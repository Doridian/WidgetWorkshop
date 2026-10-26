//! Sound output: the [`Engine`] renders what the game plays, and a `cpal`
//! stream (the `audio-device` feature) pulls it to the sound card. With no
//! device (or without the feature) [`Audio`] is silent and every call does
//! nothing, so the game runs the same headless. [`input`] captures the
//! microphone the same way.

pub mod engine;
pub mod input;

use std::sync::{Arc, Mutex, PoisonError};

pub use engine::{Engine, Play, Sound, VoiceId};
pub use input::Capture;

/// The game's sound output.
pub struct Audio {
    engine: Option<Arc<Mutex<Engine>>>,
    #[cfg(feature = "audio-device")]
    _stream: Option<Arc<cpal::Stream>>,
}

impl Audio {
    /// No output: every call does nothing.
    #[must_use]
    pub fn silent() -> Audio {
        Audio {
            engine: None,
            #[cfg(feature = "audio-device")]
            _stream: None,
        }
    }

    /// The default output device. Without one it's silent, which logs
    /// one line on stderr.
    #[must_use]
    pub fn open() -> Audio {
        Audio::with_device().unwrap_or_else(|e| {
            eprintln!("sound off: {e}");
            Audio::silent()
        })
    }

    #[cfg(feature = "audio-device")]
    fn with_device() -> Result<Audio, String> {
        with_devices(true, Audio::on)
    }

    #[cfg(feature = "audio-device")]
    fn on(device: &cpal::Device) -> Result<Audio, String> {
        use cpal::traits::DeviceTrait;

        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let engine = Arc::new(Mutex::new(Engine::new(supported.config().sample_rate)));
        // The device's default buffer can be smaller than the sound
        // server's period (PipeWire's ALSA plugin under a quantum of 1024),
        // and then every poll fails. Ask for a buffer that's more than
        // enough, and a bigger one while the stream reports errors.
        let mut last = String::new();
        for frames in BUFFER_FRAMES {
            let frames = match supported.buffer_size() {
                cpal::SupportedBufferSize::Range { min, max } => frames.clamp(*min, *max),
                cpal::SupportedBufferSize::Unknown => frames,
            };
            match open_stream(device, &supported, frames, &engine) {
                Ok(stream) => {
                    let stream = Arc::new(stream);
                    #[cfg(target_arch = "wasm32")]
                    STREAMS
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push(Arc::downgrade(&stream));
                    return Ok(Audio {
                        engine: Some(engine),
                        _stream: Some(stream),
                    });
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    #[cfg(not(feature = "audio-device"))]
    #[allow(clippy::unnecessary_wraps)]
    fn with_device() -> Result<Audio, String> {
        Err("built without the audio-device feature".into())
    }

    /// Whether anything is heard.
    #[must_use]
    pub fn has_output(&self) -> bool {
        self.engine.is_some()
    }

    /// Calls `f` on the engine, if there's output.
    pub fn with(&self, f: impl FnOnce(&mut Engine)) {
        if let Some(e) = &self.engine {
            f(&mut e.lock().unwrap_or_else(PoisonError::into_inner));
        }
    }
}

/// The sound servers' own ALSA devices, to try when the default won't
/// open: without the server's ALSA default config, `default` is the
/// card's dmix, which fails while the server holds the card.
#[cfg(feature = "audio-device")]
const SERVER_DEVICES: [&str; 2] = ["pipewire", "pulse"];

/// `open` on the default output (or input) device, and if that fails, on
/// each of [`SERVER_DEVICES`] there is. The last error if none opens.
#[cfg(feature = "audio-device")]
pub(crate) fn with_devices<T>(
    output: bool,
    mut open: impl FnMut(&cpal::Device) -> Result<T, String>,
) -> Result<T, String> {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();
    let default = if output { host.default_output_device() } else { host.default_input_device() };
    let mut last = format!("no audio {} device", if output { "output" } else { "input" });
    if let Some(device) = &default {
        match open(device) {
            Ok(t) => return Ok(t),
            Err(e) => last = e,
        }
    }
    let devices = if output { host.output_devices() } else { host.input_devices() };
    let devices: Vec<cpal::Device> = devices.into_iter().flatten().collect();
    for name in SERVER_DEVICES {
        let Some(device) = devices.iter().find(|d| d.id().is_ok_and(|id| id.id() == name)) else {
            continue;
        };
        match open(device) {
            Ok(t) => return Ok(t),
            Err(e) => last = format!("{last}; {name}: {e}"),
        }
    }
    Err(last)
}

/// The buffer sizes to try, in frames, smallest first: 2048 is about
/// 46 ms at 44.1 kHz.
#[cfg(feature = "audio-device")]
const BUFFER_FRAMES: [u32; 3] = [2048, 4096, 8192];

/// Opens a stream on `device` with a buffer of `frames` that pulls from
/// `engine`, plays it, and gives it back if it runs without errors for a
/// moment. Later errors are logged, each kind once.
#[cfg(feature = "audio-device")]
fn open_stream(
    device: &cpal::Device,
    supported: &cpal::SupportedStreamConfig,
    frames: u32,
    engine: &Arc<Mutex<Engine>>,
) -> Result<cpal::Stream, String> {
    use cpal::traits::{DeviceTrait, StreamTrait};
    use std::sync::atomic::{AtomicUsize, Ordering};

    let mut config = supported.config();
    config.buffer_size = cpal::BufferSize::Fixed(frames);
    let channels = usize::from(config.channels);
    let errors = Arc::new(AtomicUsize::new(0));
    let err = {
        let errors = Arc::clone(&errors);
        let mut logged: Vec<String> = Vec::new();
        move |e: cpal::Error| {
            errors.fetch_add(1, Ordering::Relaxed);
            let text = e.to_string();
            if !logged.contains(&text) {
                eprintln!("audio: {text}");
                logged.push(text);
            }
        }
    };
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => {
            let engine = Arc::clone(engine);
            device.build_output_stream(
                config,
                move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    engine
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .render(out, channels);
                },
                err,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let engine = Arc::clone(engine);
            let mut buf = Vec::new();
            device.build_output_stream(
                config,
                move |out: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    buf.resize(out.len(), 0.0);
                    engine
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .render(&mut buf, channels);
                    for (o, s) in out.iter_mut().zip(&buf) {
                        // -1.0..1.0, clamped by the engine.
                        #[allow(clippy::cast_possible_truncation)]
                        let v = (s * f32::from(i16::MAX)) as i16;
                        *o = v;
                    }
                },
                err,
                None,
            )
        }
        f => return Err(format!("unsupported sample format {f}")),
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    checked(stream, &errors, frames)
}

/// `stream` if it runs without errors for a moment.
#[cfg(all(feature = "audio-device", not(target_arch = "wasm32")))]
fn checked(stream: cpal::Stream, errors: &std::sync::atomic::AtomicUsize, frames: u32) -> Result<cpal::Stream, String> {
    // A broken buffer size fails at once and on every poll.
    std::thread::sleep(std::time::Duration::from_millis(150));
    match errors.load(std::sync::atomic::Ordering::Relaxed) {
        0 => Ok(stream),
        n => Err(format!("{n} stream errors with a buffer of {frames} frames")),
    }
}

/// The browser can't wait; its stream reports errors later, if at all.
#[cfg(all(feature = "audio-device", target_arch = "wasm32"))]
#[allow(clippy::unnecessary_wraps)]
fn checked(
    stream: cpal::Stream,
    _errors: &std::sync::atomic::AtomicUsize,
    _frames: u32,
) -> Result<cpal::Stream, String> {
    Ok(stream)
}

/// The browser's audio streams, to start after a user gesture.
#[cfg(all(feature = "audio-device", target_arch = "wasm32"))]
static STREAMS: Mutex<Vec<std::sync::Weak<cpal::Stream>>> = Mutex::new(Vec::new());

/// Starts the sound after the user clicked or pressed a key: a browser
/// keeps a page's `AudioContext` suspended until then, and `cpal` asks
/// for it to run when the stream plays. The platform calls this on every
/// press; natively it does nothing.
pub fn resume() {
    #[cfg(all(feature = "audio-device", target_arch = "wasm32"))]
    {
        use cpal::traits::StreamTrait;
        let mut streams = STREAMS.lock().unwrap_or_else(PoisonError::into_inner);
        streams.retain(|s| {
            let Some(s) = s.upgrade() else { return false };
            // Refused again until a gesture counts; the next press retries.
            let _ = s.play();
            true
        });
    }
}
