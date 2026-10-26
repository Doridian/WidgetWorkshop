//! What the running widget asks to hear ([`SoundCmd`]), played through the
//! platform's mixer at 11025 Hz, as the original's (`docs/gadgets/sound-time.md`).

use std::collections::HashMap;

use engine::gadget::SoundData;
use engine::sim::SoundCmd;
use platform::audio::{Audio, Play, VoiceId};

/// The mixer's rate.
const RATE: u32 = 11025;
/// The lowest rate a sound plays at.
const MIN_RATE: u16 = 172;

pub struct Sound {
    audio: Audio,
    /// The widget's voices to the mixer's.
    voices: HashMap<u64, VoiceId>,
}

/// `MulDiv`: `a * b / c`, rounded to nearest.
fn mul_div(a: usize, b: usize, c: usize) -> usize {
    (a * b + c / 2) / c.max(1)
}

/// `sound` at 11025 Hz (`PlayBuffer`, `0x4404a8`): resampled by picking
/// the nearest sample; none below 172 Hz.
fn at_mixer_rate(sound: &SoundData) -> Option<platform::audio::Sound> {
    let data = &sound.samples;
    if u32::from(sound.rate) == RATE {
        return Some(platform::audio::Sound::new(RATE, data.clone()));
    }
    if sound.rate < MIN_RATE || data.is_empty() {
        return None;
    }
    let len = data.len();
    let n = mul_div(len, RATE as usize, usize::from(sound.rate));
    let buf: Vec<u8> = (0..n).map(|i| data[mul_div(i, len, n).min(len - 1)]).collect();
    Some(platform::audio::Sound::new(RATE, buf))
}

impl Sound {
    pub fn new(audio: Audio) -> Sound {
        Sound {
            audio,
            voices: HashMap::new(),
        }
    }

    pub fn play(&mut self, cmds: Vec<SoundCmd>) {
        for cmd in cmds {
            match cmd {
                SoundCmd::Play { voice, sound, looping } => {
                    let Some(wave) = at_mixer_rate(&sound) else { continue };
                    let mut id = None;
                    self.audio.with(|e| {
                        id = Some(e.play(
                            &wave,
                            Play {
                                looping,
                                ..Play::default()
                            },
                        ));
                    });
                    if let Some(id) = id {
                        self.voices.insert(voice, id);
                    }
                }
                SoundCmd::Stop { voice } => {
                    if let Some(id) = self.voices.remove(&voice) {
                        self.audio.with(|e| e.stop(id));
                    }
                }
                SoundCmd::StopAll => {
                    self.voices.clear();
                    self.audio.with(platform::audio::Engine::stop_all);
                }
            }
        }
    }
}

/// The local time of day: hours, minutes, seconds.
pub fn local_time() -> (u32, u32, u32) {
    use chrono::Timelike;
    let t = chrono::Local::now();
    (t.hour(), t.minute(), t.second())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resamples_by_nearest_sample() {
        let s = SoundData {
            rate: 22050,
            samples: vec![1, 2, 3, 4].into(),
        };
        assert_eq!(&*at_mixer_rate(&s).unwrap().samples, [1, 3]);
        let low = SoundData {
            rate: 171,
            samples: vec![1].into(),
        };
        assert!(at_mixer_rate(&low).is_none());
    }
}
