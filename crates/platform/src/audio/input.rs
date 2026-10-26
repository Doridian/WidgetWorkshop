//! Sound input: the default input device through a `cpal` stream (the
//! `audio-device` feature), mixed down to mono and brought to the rate
//! asked for as unsigned 8-bit samples (silence 0x80), the format of the
//! original's `waveIn` (11025 Hz, 8 bits, mono). Without a device, or
//! without the feature, opening fails.

use std::sync::{Arc, Mutex, PoisonError};

/// Mono samples at the device's rate to unsigned 8-bit ones at another:
/// each output sample the mean of the input it covers (a box filter, as
/// the cards' own converters did roughly).
#[derive(Debug, Clone)]
pub struct Downmix {
    from: u64,
    to: u64,
    phase: u64,
    sum: f32,
    n: u32,
}

impl Downmix {
    /// From `from` Hz to `to` Hz.
    #[must_use]
    pub fn new(from: u32, to: u32) -> Downmix {
        Downmix {
            from: u64::from(from.max(1)),
            to: u64::from(to.max(1)),
            phase: 0,
            sum: 0.0,
            n: 0,
        }
    }

    /// One input sample (-1.0 to 1.0); the output samples it completes
    /// are appended to `out`.
    pub fn push(&mut self, x: f32, out: &mut Vec<u8>) {
        self.sum += x;
        self.n += 1;
        self.phase += self.to;
        if self.phase < self.from {
            return;
        }
        #[allow(clippy::cast_precision_loss)]
        let v = self.sum / self.n as f32;
        while self.phase >= self.from {
            self.phase -= self.from;
            out.push(to_u8(v));
        }
        self.sum = 0.0;
        self.n = 0;
    }
}

/// -1.0..1.0 as unsigned 8-bit, 0 at 0x80.
#[must_use]
pub fn to_u8(x: f32) -> u8 {
    // In 0.5..255.5 after the clamp.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let v = (x.clamp(-1.0, 1.0) * 127.5 + 128.0) as u8;
    v
}

/// What the stream has captured, and its first error.
#[derive(Debug, Default)]
struct Captured {
    samples: Vec<u8>,
    error: Option<String>,
}

/// A running capture; dropping it closes the device.
pub struct Capture {
    captured: Arc<Mutex<Captured>>,
    #[cfg(feature = "audio-device")]
    _stream: cpal::Stream,
}

impl Capture {
    /// Starts capturing from the default input device at `rate` Hz (or a
    /// sound server's, if the default won't open).
    ///
    /// # Errors
    /// No input device, or it won't open.
    #[cfg(feature = "audio-device")]
    pub fn open(rate: u32) -> Result<Capture, String> {
        super::with_devices(false, |device| Capture::on(device, rate))
    }

    #[cfg(feature = "audio-device")]
    fn on(device: &cpal::Device, rate: u32) -> Result<Capture, String> {
        use cpal::traits::{DeviceTrait, StreamTrait};

        let supported = device.default_input_config().map_err(|e| e.to_string())?;
        let config = supported.config();
        let captured = Arc::new(Mutex::new(Captured::default()));
        let down = Downmix::new(config.sample_rate, rate);
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => build::<f32>(device, config, down, &captured),
            cpal::SampleFormat::I16 => build::<i16>(device, config, down, &captured),
            cpal::SampleFormat::I32 => build::<i32>(device, config, down, &captured),
            cpal::SampleFormat::U16 => build::<u16>(device, config, down, &captured),
            cpal::SampleFormat::U8 => build::<u8>(device, config, down, &captured),
            f => return Err(format!("unsupported input sample format {f}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Capture {
            captured,
            _stream: stream,
        })
    }

    /// Without the `audio-device` feature there's no input.
    ///
    /// # Errors
    /// Always.
    #[cfg(not(feature = "audio-device"))]
    pub fn open(_rate: u32) -> Result<Capture, String> {
        Err("built without the audio-device feature".into())
    }

    /// Moves what was captured since the last call to the end of `out`.
    ///
    /// # Errors
    /// The stream reported an error.
    pub fn read(&self, out: &mut Vec<u8>) -> Result<(), String> {
        let mut c = self.captured.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(e) = &c.error {
            return Err(e.clone());
        }
        out.append(&mut c.samples);
        Ok(())
    }
}

/// An input stream of samples of type `T` feeding `captured`.
#[cfg(feature = "audio-device")]
fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut down: Downmix,
    captured: &Arc<Mutex<Captured>>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    use cpal::traits::DeviceTrait;

    let channels = usize::from(config.channels.max(1));
    let data = Arc::clone(captured);
    let err = Arc::clone(captured);
    let mut buf = Vec::new();
    device
        .build_input_stream(
            config,
            move |input: &[T], _: &cpal::InputCallbackInfo| {
                buf.clear();
                for frame in input.chunks(channels) {
                    let sum: f32 = frame.iter().map(|&s| s.to_sample::<f32>()).sum();
                    #[allow(clippy::cast_precision_loss)]
                    down.push(sum / frame.len() as f32, &mut buf);
                }
                data.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .samples
                    .extend_from_slice(&buf);
            },
            move |e: cpal::Error| {
                err.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .error
                    .get_or_insert_with(|| e.to_string());
            },
            None,
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_averages_to_the_new_rate() {
        // 44100 to 11025: one sample per four, their mean.
        let mut d = Downmix::new(44100, 11025);
        let mut out = Vec::new();
        for x in [1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0] {
            d.push(x, &mut out);
        }
        assert_eq!(out, [0x80, 255]);
        // Up: each input sample repeated.
        let mut d = Downmix::new(5512, 11025);
        let mut out = Vec::new();
        d.push(-1.0, &mut out);
        d.push(-1.0, &mut out);
        assert_eq!(out.len(), 4);
        assert!(out.iter().all(|&b| b == 0));
        assert_eq!(to_u8(0.0), 0x80);
    }
}
