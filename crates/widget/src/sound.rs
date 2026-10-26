//! What the running widget asks to hear ([`SoundCmd`]), played through the
//! platform's mixer.

use std::collections::HashMap;

use engine::assets::{Assets, Module, rt};
use engine::sim::{SoundCmd, SoundRef};
use formats::tables::Wave;
use platform::audio::{Audio, Play, VoiceId};

pub struct Sound {
    audio: Audio,
    /// The waves played so far, by resource id.
    waves: HashMap<u32, Option<platform::audio::Sound>>,
    /// The widget's voices to the mixer's.
    voices: HashMap<u64, VoiceId>,
}

impl Sound {
    pub fn new(audio: Audio) -> Sound {
        Sound {
            audio,
            waves: HashMap::new(),
            voices: HashMap::new(),
        }
    }

    fn wave(&mut self, assets: &Assets, id: u32) -> Option<platform::audio::Sound> {
        self.waves
            .entry(id)
            .or_insert_with(|| {
                let w = Wave::parse(assets.get(Module::Sound, rt::SOUND, id)?)?;
                Some(platform::audio::Sound::new(w.rate, w.samples))
            })
            .clone()
    }

    pub fn play(&mut self, assets: &Assets, cmds: Vec<SoundCmd>) {
        for cmd in cmds {
            match cmd {
                SoundCmd::Play {
                    voice,
                    sound,
                    looping,
                    speed,
                    volume,
                } => {
                    let wave = match sound {
                        SoundRef::Resource(id) => self.wave(assets, id),
                        // Recordings aren't kept yet.
                        SoundRef::Recorded(_) => None,
                    };
                    let Some(wave) = wave else { continue };
                    let mut id = None;
                    self.audio.with(|e| {
                        id = Some(e.play(&wave, Play { looping, speed, volume }));
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
