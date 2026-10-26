//! Running a widget: what a gadget's tick sees ([`Io`]). Values are
//! pulled: a gadget reads an input by asking the gadget at the wire's other
//! end for its output (`0x41e56e`). A wire carries a "changed" flag the
//! sender sets (`0x41e6cf`) and the reader clears as it reads it
//! (`0x41e713`).

use crate::assets::Assets;
use crate::gadget::{Core, Gadget, NONE, SoundData, Value};

/// A wire from an output to an input (`cConnection`).
#[derive(Debug, Clone, PartialEq)]
pub struct Wire {
    /// The gadget with the output, as an index into the document's list.
    pub from: usize,
    pub from_knot: usize,
    /// The gadget with the input.
    pub to: usize,
    pub to_knot: usize,
    /// Locked (`+0x25`, a puzzle's wire), saved from format 3 on.
    pub flag: u8,
    /// Bends on the way, on the worksheet (`h`, `v`).
    pub points: Vec<(i32, i32)>,
    /// Set by the sender, cleared by the reader (`+0x2a`).
    pub changed: bool,
}

/// What a sound request asks the platform for. The original's mixer
/// (docs/gadgets/sound-time.md, 2.3) plays 8 voices at 11025 Hz with no
/// volume; a sound at another rate is resampled to it, nearest neighbour
/// (`PlayBuffer`, `0x4404a8`), and one below 172 Hz isn't played.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoundCmd {
    /// Play `sound` as voice `voice`.
    Play {
        voice: u64,
        sound: SoundData,
        looping: bool,
    },
    Stop {
        voice: u64,
    },
    StopAll,
}

/// What the running widget shares with the outside: the clock, the mouse,
/// the keyboard, randomness and sound.
#[derive(Debug, Clone, Default)]
pub struct Env {
    /// The clock in Mac ticks (1/60 s) since the widget started.
    pub ticks: u32,
    /// Milliseconds since the widget started.
    pub ms: u64,
    /// The mouse on the worksheet, and whether its button is down.
    pub mouse: (i32, i32),
    pub mouse_down: bool,
    /// Keys pressed since the last tick (Windows virtual-key codes).
    pub keys: Vec<u16>,
    /// The random generator's state.
    pub seed: u32,
    /// Sound requests for the platform, in order.
    pub sounds: Vec<SoundCmd>,
    next_voice: u64,
    /// Wall-clock local time: hours, minutes, seconds (for the clock).
    pub local_time: (u32, u32, u32),
}

impl Env {
    /// A fresh voice number for [`SoundCmd::Play`].
    pub fn voice(&mut self) -> u64 {
        self.next_voice += 1;
        self.next_voice
    }
}

/// What a gadget's tick, click or start gets: the other gadgets (to read
/// its inputs from), the wires and the environment. The gadget itself is
/// taken out of the list while it runs.
pub struct Io<'a> {
    pub assets: &'a Assets,
    pub gadgets: &'a [Gadget],
    pub wires: &'a mut [Wire],
    pub env: &'a mut Env,
}

impl Io<'_> {
    /// Input `knot` of `core`: the value the output at its wire's other end
    /// gives, or [`Value::None`] unwired.
    #[must_use]
    pub fn input(&self, core: &Core, knot: usize) -> Value {
        let Some(w) = core.knots.get(knot).and_then(|k| k.wire) else {
            return Value::None;
        };
        let Some(wire) = self.wires.get(w) else {
            return Value::None;
        };
        // The gadget reading is `wire.to`; its source `wire.from`. A wire
        // drawn the other way round (two outputs, two inputs) reads the
        // other end.
        let (src, src_knot) = if wire.to_knot == knot && self.is_core(wire.to, core) {
            (wire.from, wire.from_knot)
        } else {
            (wire.to, wire.to_knot)
        };
        match self.gadgets.get(src) {
            Some(g) => g.behavior.output(&g.core, src_knot),
            None => Value::None,
        }
    }

    /// Whether gadget `i` is the one running (taken out of the list: its
    /// slot holds a placeholder with the same id).
    fn is_core(&self, i: usize, core: &Core) -> bool {
        self.gadgets.get(i).is_some_and(|g| g.core.id == core.id)
    }

    /// Input `knot` as a number (`0x41e56e`): a boolean or an object is 1
    /// or 0; 0 unwired.
    #[must_use]
    pub fn number(&self, core: &Core, knot: usize) -> f64 {
        if self.wired(core, knot) {
            self.input(core, knot).number()
        } else {
            0.0
        }
    }

    /// Whether input `knot`'s value changed since the last call (and clears
    /// the flag).
    pub fn changed(&mut self, core: &Core, knot: usize) -> bool {
        match core.knots.get(knot).and_then(|k| k.wire) {
            Some(w) => self.wires.get_mut(w).is_some_and(|w| std::mem::take(&mut w.changed)),
            None => false,
        }
    }

    /// Output `knot`'s value changed (`0x41e6cf`); returns whether it's wired.
    pub fn fire(&mut self, core: &Core, knot: usize) -> bool {
        match core.knots.get(knot).and_then(|k| k.wire) {
            Some(w) => match self.wires.get_mut(w) {
                Some(w) => {
                    w.changed = true;
                    true
                }
                None => false,
            },
            None => false,
        }
    }

    /// Whether input `knot` is wired.
    #[must_use]
    pub fn wired(&self, core: &Core, knot: usize) -> bool {
        core.knots.get(knot).is_some_and(|k| k.wire.is_some())
    }

    /// Ticks since the run's first tick, which sees 0 (`doc+0x4b`).
    #[must_use]
    pub fn elapsed(&self) -> u32 {
        self.env.ticks
    }

    /// Passes at most once every `n + 1` ticks (`0x41e762`).
    pub fn throttle(&self, core: &mut Core, n: u32) -> bool {
        let now = self.elapsed();
        if now.wrapping_sub(core.throttled_at) > n {
            core.throttled_at = now;
            true
        } else {
            false
        }
    }

    /// Whether input `knot`'s value differs from the one cached (and caches
    /// it), when it's wired (`0x41e62a`).
    pub fn value_changed(&self, core: &mut Core, knot: usize) -> bool {
        let v = self.number(core, knot);
        let Some(c) = core.cache.get_mut(knot) else {
            return false;
        };
        #[allow(clippy::float_cmp)]
        if *c == v {
            return false;
        }
        *c = v;
        self.wired(core, knot)
    }

    /// The base class's start (`0x41e50a`): every input's cache "none"
    /// and its wire unchanged, or (usually) every input's value cached.
    pub fn base_start(&mut self, core: &mut Core) {
        for k in 0..core.knots.len().min(core.cache.len()) {
            if core.empty_cache {
                core.cache[k] = NONE;
                self.changed(core, k);
            } else {
                core.cache[k] = self.number(core, k);
            }
        }
    }

    /// Plays `sound` (looping or once) as a new voice; returns it.
    pub fn play_sound(&mut self, sound: SoundData, looping: bool) -> u64 {
        let voice = self.env.voice();
        self.env.sounds.push(SoundCmd::Play { voice, sound, looping });
        voice
    }

    /// Plays sound resource `id` once at 11025 Hz (`0x4402cc`).
    pub fn play(&mut self, id: u32) -> Option<u64> {
        let s = self.assets.sound(id)?;
        Some(self.play_sound(s, false))
    }

    /// Stops voice `voice`.
    pub fn stop_sound(&mut self, voice: u64) {
        self.env.sounds.push(SoundCmd::Stop { voice });
    }
}
