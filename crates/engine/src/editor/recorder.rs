//! The Recorder's own buttons (docs/dialogs.md 4): "Record a Sound"
//! (`0x459db3`) and "Import .WAV file" (`0x459fe8`). Both write straight
//! into the part's buffer, so Cancel doesn't take them back, and OK's undo
//! snapshot comes after them.
//!
//! The original records blocking (`0x43e170`: `waveIn` busy-polled, the
//! progress bar repainted on each poll); here the recording is a modal
//! state that [`Editor::update`] advances while the samples come in.

use crate::gadgets::sound_time::{TAPE_LEN, TapePlayer, WavError};
use crate::sim::SoundCmd;

use super::prompt::Answer;
use super::{Editor, Then};

/// The catch handler's message (`0x459f9f`, literal at `0x49c04a`).
const RECORD_ERROR: &str = "There was an error while recording sound";
/// Milliseconds without a sample before the recording counts as failed
/// (the original waits for ever on a driver that doesn't advance).
const STALL_MS: u64 = 3000;

/// A recording under way.
pub(super) struct Recording {
    /// The Recorder's index.
    g: usize,
    buf: Vec<u8>,
    /// When samples last came.
    last: u64,
}

impl Editor {
    /// "Record a Sound" (`0x459db3`) in the dialog of Recorder `g`.
    pub(super) fn start_recording(&mut self, g: usize) {
        self.doc.gadgets[g].core.broken = false;
        // `SoundClose` (`0x43f39f`, `waveOutReset`): nothing plays on.
        self.env.sounds.push(SoundCmd::StopAll);
        match self.mic.start() {
            Ok(()) => {
                self.recording = Some(Recording {
                    g,
                    buf: Vec::with_capacity(TAPE_LEN),
                    last: self.now,
                });
            }
            Err(_) => self.record_failed(g),
        }
    }

    /// Whether a recording holds up the editor.
    #[must_use]
    pub fn recording(&self) -> bool {
        self.recording.is_some()
    }

    /// The recording's poll: the samples so far and the bar (`0x459d98`,
    /// `pos * 105 / 40000`); at 40000 bytes, into the Recorder.
    pub(super) fn poll_recording(&mut self) {
        let Some(mut rec) = self.recording.take() else { return };
        let had = rec.buf.len();
        if self.mic.read(&mut rec.buf).is_err() {
            self.mic.stop();
            self.record_failed(rec.g);
            return;
        }
        if rec.buf.len() > had {
            rec.last = self.now;
        } else if self.now.saturating_sub(rec.last) > STALL_MS {
            self.mic.stop();
            self.record_failed(rec.g);
            return;
        }
        let pos = rec.buf.len().min(TAPE_LEN);
        if let Some(d) = &mut self.dialog {
            d.set_progress(u16::try_from(pos * 105 / TAPE_LEN).unwrap_or(105));
        }
        if rec.buf.len() < TAPE_LEN {
            self.recording = Some(rec);
            return;
        }
        self.mic.stop();
        if let Some(d) = &mut self.dialog {
            d.set_progress(100);
            d.changed = true;
        }
        if let Some(t) = self.doc.gadgets[rec.g]
            .behavior
            .as_any_mut()
            .downcast_mut::<TapePlayer>()
        {
            t.record(&rec.buf);
        }
    }

    /// The catch handler (`0x459f9f`): Record disabled for this dialog, the
    /// part broken, the message.
    fn record_failed(&mut self, g: usize) {
        if let Some(d) = &mut self.dialog {
            // `progress(100)` runs before the error is noticed.
            d.set_progress(100);
            d.disable_button(2);
            d.changed = true;
        }
        self.doc.gadgets[g].core.broken = true;
        self.message(RECORD_ERROR, &[Answer::Ok], Then::Nothing);
    }

    /// The Sound Switches' listening fed from the microphone, opened while
    /// they listen (`0x43e2db`) and closed when they stop (`0x43e411`).
    pub(super) fn poll_listen(&mut self) {
        let l = &mut self.env.listen;
        if !l.on {
            if std::mem::take(&mut l.capturing) {
                self.mic.stop();
            }
            return;
        }
        if !l.capturing {
            l.capturing = true;
            if self.mic.start().is_err() {
                l.pos = None;
                return;
            }
        }
        let mut got = Vec::new();
        if self.mic.read(&mut got).is_err() {
            l.pos = None;
        } else if !std::mem::take(&mut l.fresh) {
            l.feed(&got);
        }
    }

    /// `0x43deda` on the file `path` into Recorder `g`; its messages on
    /// failure. Success clears the broken flag.
    pub(super) fn import_wav_path(&mut self, g: usize, path: &str) {
        let error = match self.fs().read(std::path::Path::new(path)) {
            // The backtick is the binary's (`0x496a37`).
            Err(_) => Some("Can`t open file"),
            Ok(bytes) => {
                let gadget = &mut self.doc.gadgets[g];
                match gadget.behavior.as_any_mut().downcast_mut::<TapePlayer>() {
                    Some(t) => match t.import(&mut gadget.core, &bytes) {
                        Ok(()) => None,
                        Err(WavError::BadFormat) => Some("Bad file format"),
                        Err(WavError::Not8Bit) => Some("Can't import non-8 bit sound"),
                    },
                    None => None,
                }
            }
        };
        if let Some(d) = &mut self.dialog {
            d.changed = true;
        }
        if let Some(e) = error {
            self.message(&format!("{path}\n{e}"), &[Answer::Ok], Then::Nothing);
        }
    }
}
