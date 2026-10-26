//! What the sound card plays, rendered without a device: the voices the
//! game starts, mixed. The original's "Widget Sound System" mixes 8-bit
//! mono waves at 11.025 kHz into one wave-out stream; here each voice is
//! resampled to the device's rate on the fly, so a voice can also play
//! faster or slower than its wave was recorded (the Sound Transformer's
//! pitch shifts). The game decides what plays when; this only renders it,
//! so tests can render to a buffer.

use std::sync::Arc;

/// The 8-bit PCM zero.
pub const SILENCE: u8 = 0x80;

/// A playing voice's handle, unique for the engine's life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VoiceId(pub u64);

/// A wave to play: 8-bit unsigned mono samples at `rate` Hz.
#[derive(Debug, Clone)]
pub struct Sound {
    pub rate: u32,
    pub samples: Arc<[u8]>,
}

impl Sound {
    #[must_use]
    pub fn new(rate: u32, samples: impl Into<Arc<[u8]>>) -> Sound {
        Sound {
            rate,
            samples: samples.into(),
        }
    }

    /// The length in seconds.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn seconds(&self) -> f64 {
        self.samples.len() as f64 / f64::from(self.rate.max(1))
    }
}

/// How a voice plays.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Play {
    /// Start over at the end, until stopped.
    pub looping: bool,
    /// The playback rate as a factor of the wave's own: 2.0 is an octave up.
    pub speed: f64,
    /// 0.0 to 1.0.
    pub volume: f32,
}

impl Default for Play {
    fn default() -> Play {
        Play {
            looping: false,
            speed: 1.0,
            volume: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
struct Voice {
    id: VoiceId,
    sound: Sound,
    play: Play,
    /// Where in the wave, in samples.
    at: f64,
}

/// An 8-bit sample as -1.0..1.0.
fn unsigned8(s: u8) -> f32 {
    (f32::from(s) - 128.0) / 128.0
}

/// The mixer behind the output device.
pub struct Engine {
    rate: u32,
    voices: Vec<Voice>,
    next_id: u64,
    volume: f32,
}

impl Engine {
    /// An engine rendering at `rate` Hz.
    #[must_use]
    pub fn new(rate: u32) -> Engine {
        Engine {
            rate: rate.max(1),
            voices: Vec::new(),
            next_id: 1,
            volume: 1.0,
        }
    }

    #[must_use]
    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Starts `sound`; it plays until its end (or [`Engine::stop`]).
    pub fn play(&mut self, sound: &Sound, play: Play) -> VoiceId {
        let id = VoiceId(self.next_id);
        self.next_id += 1;
        if !sound.samples.is_empty() {
            self.voices.push(Voice {
                id,
                sound: sound.clone(),
                play,
                at: 0.0,
            });
        }
        id
    }

    pub fn stop(&mut self, id: VoiceId) {
        self.voices.retain(|v| v.id != id);
    }

    pub fn stop_all(&mut self) {
        self.voices.clear();
    }

    #[must_use]
    pub fn is_playing(&self, id: VoiceId) -> bool {
        self.voices.iter().any(|v| v.id == id)
    }

    /// Changes a playing voice's speed and volume.
    pub fn adjust(&mut self, id: VoiceId, speed: f64, volume: f32) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.id == id) {
            v.play.speed = speed;
            v.play.volume = volume;
        }
    }

    /// The master volume, 0.0 to 1.0.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Renders the next `out.len() / channels` frames, interleaved, as
    /// -1.0..1.0, every channel the same.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let rate = f64::from(self.rate);
        for frame in out.chunks_exact_mut(channels) {
            let mut sum = 0.0f32;
            for v in &mut self.voices {
                let len = v.sound.samples.len();
                if v.at >= len as f64 {
                    continue;
                }
                let i = v.at as usize;
                let t = (v.at - i as f64) as f32;
                let a = unsigned8(v.sound.samples[i]);
                let b = if i + 1 < len {
                    unsigned8(v.sound.samples[i + 1])
                } else if v.play.looping {
                    unsigned8(v.sound.samples[0])
                } else {
                    0.0
                };
                sum += (a + (b - a) * t) * v.play.volume;
                v.at += f64::from(v.sound.rate) * v.play.speed.max(0.0) / rate;
                if v.play.looping && v.at >= len as f64 {
                    v.at %= len as f64;
                }
            }
            frame.fill((sum * self.volume).clamp(-1.0, 1.0));
        }
        self.voices.retain(|v| v.at < v.sound.samples.len() as f64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plays_to_its_end_and_stops() {
        let mut e = Engine::new(11025);
        let id = e.play(&Sound::new(11025, vec![0x40u8; 3]), Play::default());
        assert!(e.is_playing(id));
        let mut out = vec![0.0; 5];
        e.render(&mut out, 1);
        assert_eq!(out, [-0.5, -0.5, -0.5, 0.0, 0.0]);
        assert!(!e.is_playing(id));
    }

    #[test]
    fn mixes_and_resamples() {
        let mut e = Engine::new(22050);
        e.play(&Sound::new(11025, vec![0xc0u8; 4]), Play::default());
        e.play(&Sound::new(11025, vec![0xc0u8; 4]), Play::default());
        let mut out = vec![0.0; 2 * 4];
        e.render(&mut out, 2);
        assert!(out.iter().all(|s| (*s - 1.0).abs() < 1e-6), "{out:?}");
    }

    #[test]
    fn loops_until_stopped() {
        let mut e = Engine::new(8000);
        let id = e.play(
            &Sound::new(8000, vec![0xffu8, 0x00]),
            Play {
                looping: true,
                ..Play::default()
            },
        );
        let mut out = vec![0.0; 100];
        e.render(&mut out, 1);
        assert!(e.is_playing(id));
        e.stop(id);
        assert!(!e.is_playing(id));
    }
}
