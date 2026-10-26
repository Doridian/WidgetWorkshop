//! Time and sound: the Stopwatch (`Wtch`), the Digital Clock (`DClk`), the
//! Metronome (`Metr`), the Tuning Fork (`Fork`), the Recorder (`Tape`),
//! the Speaker (`Spkr`), Animal Sounds (`AnSd`), Assorted Sounds (`MsSd`),
//! the Sound Transformer (`SdTr`) and the Sound Switch (`SdSw`).
//! docs/gadgets/sound-time.md has the original's code. Times are run ticks
//! (1/60 s); sounds are 8-bit unsigned mono, passed as
//! [`Object::Sound`] (the original's `{len, rate, data}` at `+0x197`).

// The original's integer and float arithmetic, reproduced on purpose.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::float_cmp
)]

use std::any::Any;
use std::sync::Arc;

use crate::assets::{Module, rt};
use crate::gadget::{Behavior, Core, DrawCx, Object, SoundData, Tag, Value};
use crate::sim::{Io, LISTEN_LEN, SoundCmd};

macro_rules! any {
    () => {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    };
}

/// The rate the game plays everything at.
const RATE: u16 = 11025;

// ---------------------------------------------------------------------------
// Shared helpers

/// `0x439e3d`: a double to an int, saturating near the ends, else
/// truncated (`__ftol`).
fn clamp_int(x: f64) -> i32 {
    if x >= 2_147_483_647.0 {
        0x7fff_fffe
    } else if x <= -2_147_483_646.0 {
        -0x7fff_fffe
    } else {
        x as i32
    }
}

/// A run tick count as the original's signed arithmetic sees it.
fn now(io: &Io<'_>) -> u32 {
    io.elapsed()
}

/// `0x41e4c8`: every knot's value into the input cache.
fn snapshot_inputs(core: &mut Core, io: &Io<'_>) {
    for k in 0..core.knots.len().min(core.cache.len()) {
        core.cache[k] = io.number(core, k);
    }
}

/// Point `i` of the gadget's point list (type 1010, same number as its
/// sheet), on the worksheet.
fn point(core: &Core, cx: &DrawCx<'_>, i: usize) -> (i32, i32) {
    let p = cx.points(core.sheet).get(i).copied().unwrap_or_default();
    (core.h + i32::from(p.h), core.v + i32::from(p.v))
}

/// Slot 0x10 (`0x41ed9d`): the broken picture (sheet 101), frame 1, or
/// frame 2 where frame 1 doesn't fit, centred on the gadget.
fn draw_broken(core: &Core, cx: &mut DrawCx<'_>) {
    let gw = i32::from(core.bounds.right) - i32::from(core.bounds.left);
    let gh = i32::from(core.bounds.bottom) - i32::from(core.bounds.top);
    let size = |cx: &DrawCx<'_>, n| {
        cx.pictures.rect(cx.assets, Module::Gadget, 101, n).map(|r| {
            (
                i32::from(r.right) - i32::from(r.left),
                i32::from(r.bottom) - i32::from(r.top),
            )
        })
    };
    let Some((mut fw, mut fh)) = size(cx, 1) else {
        return;
    };
    let mut n = 1;
    if gw < fw || gh < fh {
        n = 2;
        (fw, fh) = size(cx, 2).unwrap_or((fw, fh));
    }
    // C division: truncates toward zero.
    cx.frame(101, n, core.h + (gw - fw) / 2, core.v + (gh - fh) / 2);
}

/// The C library's `sscanf("%lf")` as `0x4110c3` uses it: the longest
/// leading number, 0 if none.
fn parse_double(text: &[u8]) -> f64 {
    let s = String::from_utf8_lossy(text);
    let s = s.trim_start();
    let mut best = 0.0;
    for end in (1..=s.len()).rev() {
        if !s.is_char_boundary(end) {
            continue;
        }
        let t = &s[..end];
        // Rust takes "inf" and "nan", the C library of 1995 wouldn't.
        if t.chars().any(char::is_alphabetic) && !t.contains(['e', 'E']) {
            continue;
        }
        if let Ok(v) = t.parse::<f64>() {
            best = v;
            break;
        }
    }
    best
}

// ---------------------------------------------------------------------------
// The Stopwatch

/// The Stopwatch (`Wtch`, docs/gadgets/sound-time.md 3).
#[derive(Debug, Default)]
pub struct StopWatch {
    /// The ticks shown (`+0x142`).
    shown: i32,
    /// When it last stopped (`+0x197`).
    stopped_at: u32,
    /// Elapsed is `now - offset` (`+0x19b`).
    offset: u32,
    /// 0 running, 1 stopped, 2 reset (`+0x19f`).
    state: u8,
}

const WATCH_START: usize = 0;
const WATCH_STOP: usize = 1;
const WATCH_RESET: usize = 2;
const WATCH_OUT: usize = 3;

impl StopWatch {
    fn clear(&mut self, core: &mut Core) {
        core.base_reset();
        self.shown = 0;
        self.state = 2;
        self.stopped_at = 0;
        self.offset = 0;
    }
}

impl Behavior for StopWatch {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, 1, core.h, core.v);
        let m = self.shown / 3600;
        let s = (self.shown / 60) % 60;
        if m < 100 {
            for (i, d) in [m / 10, m % 10, s / 10, s % 10].into_iter().enumerate() {
                let (x, y) = point(core, cx, i);
                cx.frame(core.sheet, (d + 3) as u16, x, y);
            }
        } else {
            draw_broken(core, cx);
        }
    }

    fn reset(&mut self, core: &mut Core) {
        self.clear(core);
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        // (The original also redraws when the seconds shown change; the
        // port redraws anyway.)
        let t = now(io);
        let reset = io.number(core, WATCH_RESET) != 0.0;
        match self.state {
            _ if reset => {
                self.clear(core);
                snapshot_inputs(core, io);
            }
            0 if io.number(core, WATCH_STOP) == 0.0 => {
                self.shown = t.wrapping_sub(self.offset) as i32;
            }
            0 => {
                snapshot_inputs(core, io);
                self.state = 1;
                self.stopped_at = t;
            }
            1 | 2 if io.number(core, WATCH_START) != 0.0 => {
                snapshot_inputs(core, io);
                self.state = 0;
                self.offset = self.offset.wrapping_add(t.wrapping_sub(self.stopped_at));
                self.stopped_at = t;
            }
            _ => {}
        }
        // Read after any reset: a reset never fires the output.
        let (old_number, old_state) = (core.number, core.state);
        match core.settings.radios[0] {
            0 | 1 => {
                let per = if core.settings.radios[0] == 0 { 60 } else { 3600 };
                core.number = f64::from(self.shown / per);
                if core.number != old_number {
                    io.fire(core, WATCH_OUT);
                }
            }
            2 => {
                core.state = self.state == 0;
                if core.state != old_state {
                    io.fire(core, WATCH_OUT);
                }
            }
            _ => {}
        }
    }

    fn has_bool(&self, core: &Core, _knot: usize) -> bool {
        core.settings.radios[0] == 2
    }

    fn has_number(&self, core: &Core, _knot: usize) -> bool {
        core.settings.radios[0] < 2
    }

    any!();
}

#[must_use]
pub fn stop_watch() -> Box<dyn Behavior> {
    Box::new(StopWatch {
        state: 2,
        ..StopWatch::default()
    })
}

// ---------------------------------------------------------------------------
// The Digital Clock

/// The Digital Clock (`DClk`, 4): the computer's local time.
#[derive(Debug, Default)]
pub struct DigitalClock {
    /// The local time (h, m, s) at the last tick. The original reads the
    /// clock again as it draws; the port draws what the last tick saw.
    time: (u32, u32, u32),
}

impl DigitalClock {
    /// The hour digits and the AM/PM frame (3 AM, 4 PM; none in 24-hour
    /// mode) the clock shows for `hour` (`0x444cac`). Noon shows as AM.
    #[must_use]
    pub fn hour_shown(twelve: bool, hour: u32) -> (u32, Option<u16>) {
        if !twelve {
            (hour, None)
        } else if hour < 13 {
            (if hour == 0 { 12 } else { hour }, Some(3))
        } else {
            (hour - 12, Some(4))
        }
    }
}

impl Behavior for DigitalClock {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, 1, core.h, core.v);
        if !cx.running {
            return;
        }
        let (hour, min, sec) = self.time;
        let (h, ampm) = Self::hour_shown(core.settings.radios[0] == 0, hour);
        if let Some(f) = ampm {
            let (x, y) = point(core, cx, 0);
            cx.frame(core.sheet, f, x, y);
        }
        for (i, d) in [h / 10, h % 10, min / 10, min % 10, sec / 10, sec % 10]
            .into_iter()
            .enumerate()
        {
            let (x, y) = point(core, cx, i + 1);
            cx.frame(core.sheet, (d + 5) as u16, x, y);
        }
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        io.fire(core, 0);
        self.time = io.env.local_time;
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        self.time = io.env.local_time;
        let (h, m, s) = self.time;
        let old = core.number;
        match core.settings.radios[1] {
            0 => core.number = f64::from(s),
            1 => core.number = f64::from(m),
            2 => core.number = f64::from(h),
            _ => {}
        }
        if core.number != old {
            io.fire(core, 0);
        }
    }

    fn has_number(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

#[must_use]
pub fn digital_clock() -> Box<dyn Behavior> {
    Box::new(DigitalClock::default())
}

// ---------------------------------------------------------------------------
// The Metronome

/// The beat's period in ticks for each tempo (`0x499d48`): 1, 2, 5, 10,
/// 30, 60, 120 beats a minute.
const METRONOME_PERIODS: [u32; 7] = [3600, 1800, 720, 360, 120, 60, 30];

/// The pendulum's frame for each swing phase (`0x499d14`).
const METRONOME_FRAMES: [u16; 4] = [3, 2, 1, 2];

/// The Metronome's tick sound (`WWSOUND` 1006/1011).
const METRONOME_SOUND: u32 = 1011;

/// The Metronome (`Metr`, 5): a 15-tick pulse every beat, and a tick sound.
#[derive(Debug, Default)]
pub struct Metronome {
    /// The period in ticks (`+0x19b`).
    period: u32,
    /// When the pulse was last on (`+0x19f`).
    last_on: u32,
}

impl Behavior for Metronome {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        let f = METRONOME_FRAMES[usize::from(core.phase & 3)];
        cx.frame(core.sheet, f, core.h, core.v);
    }

    fn init(&mut self, core: &mut Core) {
        core.settings.numbers[0] = 4;
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        // An index past the table reads garbage in the original.
        self.period = METRONOME_PERIODS
            .get(core.settings.numbers[0] as usize)
            .copied()
            .unwrap_or(METRONOME_PERIODS[0]);
        self.last_on = u32::MAX;
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        let p = self.period.max(1);
        let t = now(io);
        let u = t % (2 * p);
        let old = core.state;
        core.state = u % p < 15;
        if core.state {
            self.last_on = t;
        } else if t / p != self.last_on / p {
            // A beat went by between two ticks: one pulse anyway.
            core.state = true;
            self.last_on = t;
        }
        let half = p >> 1;
        let s = if u < p {
            u8::from(u >= half)
        } else if u < p + half {
            2
        } else {
            3
        };
        if s != core.phase {
            core.phase = s;
            if s & 1 == 0 {
                io.play(METRONOME_SOUND);
            }
        }
        // Only the rising edge is signalled.
        if core.state && !old {
            io.fire(core, 0);
        }
    }

    fn has_bool(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

#[must_use]
pub fn metronome() -> Box<dyn Behavior> {
    Box::new(Metronome::default())
}

// ---------------------------------------------------------------------------
// The default tone and the Tuning Fork

/// The default tone (gDefaultSoundInit, `0x410d8c`): `WWSOUND` 1006/205,
/// a 220 Hz tone, **with** its 44-byte header, and the same an octave up
/// (every eighth byte), up three octaves.
fn default_tones(io: &Io<'_>) -> Option<(Arc<[u8]>, Arc<[u8]>)> {
    let b = io.assets.get(Module::Sound, rt::SOUND, 205)?;
    let oct: Vec<u8> = b.iter().step_by(8).take(b.len() >> 3).copied().collect();
    Some((b.into(), oct.into()))
}

/// `0x410ea3`: a sound of `seconds` of the default tone pitched to `freq`
/// Hz (the tone is 220 Hz; it plays at `freq/220 * 11025` Hz).
fn make_tone(io: &Io<'_>, freq: f64, seconds: f64) -> Option<SoundData> {
    let (tone, oct) = default_tones(io)?;
    let mut data = tone;
    let mut r = (1.0 / 220.0) * freq * 11025.0;
    if r > 65535.0 {
        // The original multiplies in single precision here.
        r = f64::from((r as f32) * 0.125);
        data = oct;
    }
    let rate = if r <= 65535.0 {
        (clamp_int(r) as u16).max(10)
    } else {
        0xffff
    };
    let len = (clamp_int(f64::from(rate) * seconds).max(0) as usize).min(data.len());
    Some(SoundData {
        rate,
        samples: data[..len].into(),
    })
}

/// The preset frequencies (`0x49c3f0`): A 440 down to A 220.
const FORK_PRESETS: [f64; 13] = [
    440.0, 415.304, 391.996, 369.995, 349.228, 329.628, 311.127, 293.665, 277.183, 261.626, 246.942, 233.082, 220.0,
];

/// The Tuning Fork (`Fork`, 6): 0.2 s of a tone while its input is on.
#[derive(Debug, Default)]
pub struct TuningFork {
    sound: Option<SoundData>,
}

impl TuningFork {
    /// The frequency the settings choose.
    #[must_use]
    pub fn frequency(core: &Core) -> f64 {
        if core.settings.radios[0] == 0 {
            FORK_PRESETS
                .get(core.settings.numbers[0] as usize)
                .copied()
                .unwrap_or(FORK_PRESETS[0])
        } else {
            parse_double(core.settings.label(0))
        }
    }
}

impl Behavior for TuningFork {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, u16::from(core.phase) + 1, core.h, core.v);
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        self.sound = make_tone(io, Self::frequency(core), 0.2);
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        if io.number(core, 0) == 0.0 {
            core.object = None;
        } else if let Some(s) = &self.sound {
            if core.object.is_none() {
                io.fire(core, 1);
                core.phase = 1;
            }
            core.object = Some(Object::Sound(s.clone()));
        }
        if core.phase != 0 && io.throttle(core, 10) {
            core.phase = (core.phase + 1) % 4;
        }
        if io.changed(core, 0) {
            io.fire(core, 1);
        }
    }

    fn has_number(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    fn has_object(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

#[must_use]
pub fn tuning_fork() -> Box<dyn Behavior> {
    Box::new(TuningFork::default())
}

// ---------------------------------------------------------------------------
// The Recorder

/// The Recorder's buffer: 40000 samples at 11025 Hz.
pub const TAPE_LEN: usize = 40000;

/// Why a wave file can't be imported (`0x43deda`'s messages).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavError {
    /// "Bad file format".
    BadFormat,
    /// "Can't import non-8 bit sound".
    Not8Bit,
}

/// A wave file as the Recorder imports it (`0x43deda`): the 44-byte
/// header checked for `RIFF`, `WAVE`, `fmt ` and 8 bits, the samples read
/// from offset 44 on whatever follows (no chunk walking). A stereo file
/// keeps its second channel; at 22000 Hz or more every second frame is
/// kept, at 30000 every third, at 44000 every fourth (the last frame of
/// each group). Up to 40000 samples, the rest silence (0x80).
///
/// # Errors
/// As the game refuses the file.
pub fn import_wav(wav: &[u8]) -> Result<Vec<u8>, WavError> {
    let h = wav.get(..44).ok_or(WavError::BadFormat)?;
    if &h[0..4] != b"RIFF" || &h[8..12] != b"WAVE" || &h[12..16] != b"fmt " {
        return Err(WavError::BadFormat);
    }
    if u16::from_le_bytes([h[0x22], h[0x23]]) != 8 {
        return Err(WavError::Not8Bit);
    }
    let channels = u16::from_le_bytes([h[0x16], h[0x17]]);
    let rate = u32::from_le_bytes([h[0x18], h[0x19], h[0x1a], h[0x1b]]);
    let frames = 1 + usize::from(rate >= 22000) + usize::from(rate >= 30000) + usize::from(rate >= 44000);
    let mut data = wav[44..].iter().copied();
    let mut frame = || {
        let b = data.next();
        if channels == 2 { data.next() } else { b }
    };
    let mut out = Vec::with_capacity(TAPE_LEN);
    'samples: while out.len() < TAPE_LEN {
        let mut b = 0;
        for _ in 0..frames {
            match frame() {
                Some(x) => b = x,
                None => break 'samples,
            }
        }
        out.push(b);
    }
    out.resize(TAPE_LEN, 0x80);
    Ok(out)
}

/// The Recorder (`Tape`, 7): plays its 40000 samples while its input is on.
#[derive(Debug)]
pub struct TapePlayer {
    samples: Arc<[u8]>,
}

impl Default for TapePlayer {
    fn default() -> TapePlayer {
        TapePlayer {
            samples: vec![0x80; TAPE_LEN].into(),
        }
    }
}

impl TapePlayer {
    /// The recorded sound.
    #[must_use]
    pub fn samples(&self) -> &[u8] {
        &self.samples
    }

    /// Replaces the recording with a wave file (the dialog's "Import .WAV
    /// file", `0x459fe8`).
    ///
    /// # Errors
    /// As [`import_wav`].
    pub fn import(&mut self, core: &mut Core, wav: &[u8]) -> Result<(), WavError> {
        self.samples = import_wav(wav)?.into();
        core.broken = false;
        Ok(())
    }

    /// Replaces the recording with `data` (the dialog's "Record a Sound",
    /// `0x459db3`): up to 40000 samples, the rest silence (0x80).
    pub fn record(&mut self, data: &[u8]) {
        let mut s = vec![0x80; TAPE_LEN];
        let n = data.len().min(TAPE_LEN);
        s[..n].copy_from_slice(&data[..n]);
        self.samples = s.into();
    }

    fn sound(&self) -> SoundData {
        SoundData {
            rate: RATE,
            samples: self.samples.clone(),
        }
    }
}

impl Behavior for TapePlayer {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        if core.phase == 0 {
            cx.frame(core.sheet, 1, core.h, core.v);
        } else {
            cx.frame(core.sheet, 2, core.h, core.v);
            if core.phase & 1 != 0 {
                let (x, y) = point(core, cx, 0);
                cx.frame(core.sheet, 4, x, y);
            }
        }
        if core.broken {
            draw_broken(core, cx);
        }
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        if io.number(core, 0) == 0.0 {
            core.object = None;
            core.phase = 0;
        } else {
            if core.object.is_none() {
                io.fire(core, 1);
            }
            core.object = Some(Object::Sound(self.sound()));
            if io.throttle(core, 30) {
                core.phase = core.phase.wrapping_add(1);
            }
        }
    }

    fn has_object(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    fn save(&self, _core: &Core) -> Vec<u8> {
        self.samples.to_vec()
    }

    fn load(&mut self, _core: &mut Core, data: &[u8]) {
        let mut s = data[..data.len().min(TAPE_LEN)].to_vec();
        s.resize(TAPE_LEN, 0x80);
        self.samples = s.into();
    }

    any!();
}

#[must_use]
pub fn tape_player() -> Box<dyn Behavior> {
    Box::new(TapePlayer::default())
}

// ---------------------------------------------------------------------------
// The Speaker

/// The Speaker (`Spkr`, 8): plays a sound part's sound once each time it
/// turns on or changes, or a continuous tone of a number input's Hz (220
/// Hz for a true boolean).
#[derive(Debug, Default)]
pub struct Speaker {
    /// The tone channel (0 to 3) this speaker last claimed, and when (a
    /// voice number, increasing): the original's four channels are global
    /// (`0x4a330c`, `0x4a331c`); the latest claim of a channel owns it.
    channel: Option<(usize, u64)>,
    /// The tone playing in it.
    voice: Option<u64>,
}

impl Speaker {
    /// Who owns tone channel `c`: the voice playing there and whether it's
    /// this speaker's.
    fn owner(&self, io: &Io<'_>, me: u32, c: usize) -> Option<(u64, bool, Option<u64>)> {
        let mine = self
            .channel
            .filter(|&(ch, _)| ch == c)
            .map(|(_, at)| (at, true, self.voice));
        io.gadgets
            .iter()
            .filter(|g| g.core.id != me)
            .filter_map(|g| g.behavior.as_any().downcast_ref::<Speaker>())
            .filter_map(|s| s.channel.filter(|&(ch, _)| ch == c).map(|(_, at)| (at, false, s.voice)))
            .chain(mine)
            .max_by_key(|&(at, _, _)| at)
    }

    /// `0x45381c`: a tone of `f` Hz, from the 220 Hz sample (`1006/220`)
    /// or above about 1308 Hz the 1760 Hz one (`1006/224`), looped.
    fn tone(&mut self, core: &Core, io: &mut Io<'_>, f: f64) {
        let ch = (0..4)
            .find(|&c| self.owner(io, core.id, c).is_some_and(|(_, mine, _)| mine))
            .or_else(|| (0..3).find(|&c| self.owner(io, core.id, c).is_none()))
            .unwrap_or(3);
        let (id, rate) = tone_rate(f);
        if let Some((_, _, Some(v))) = self.owner(io, core.id, ch) {
            io.stop_sound(v);
        }
        self.channel = None;
        self.voice = None;
        if rate < 200 || f > 5512.0 {
            return;
        }
        if let Some(s) = io.assets.sound(id) {
            let v = io.play_sound(
                SoundData {
                    rate,
                    samples: s.samples,
                },
                true,
            );
            self.channel = Some((ch, v));
            self.voice = Some(v);
        }
    }
}

/// The sample and rate the Speaker plays a tone of `f` Hz with.
#[must_use]
pub fn tone_rate(f: f64) -> (u32, u16) {
    let mut id = 220;
    let mut r = (1.0 / 220.0) * f * 11025.0;
    if r > 65535.0 {
        r = f64::from((r as f32) * 0.125);
        id = 224;
    }
    let rate = if r <= 65535.0 { clamp_int(r) as u16 } else { 0xffff };
    (id, rate)
}

impl Behavior for Speaker {
    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        self.channel = None;
        self.voice = None;
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        let c = io.changed(core, 0);
        let v = io.value_changed(core, 0);
        if !(c || v) {
            return;
        }
        match io.input(core, 0) {
            Value::Object(o) => {
                if let Some(Object::Sound(s)) = o {
                    io.play_sound(s, false);
                }
            }
            Value::Number(n) => self.tone(core, io, n.abs()),
            Value::Bool(b) => self.tone(core, io, if b { 220.0 } else { 0.0 }),
            Value::None => {}
        }
    }

    any!();
}

#[must_use]
pub fn speaker() -> Box<dyn Behavior> {
    Box::new(Speaker::default())
}

// ---------------------------------------------------------------------------
// Animal Sounds and Assorted Sounds (cNoiseMaker)

/// The Animal Sounds' picture for each choice (`0x4990d6`): dog, cat,
/// rooster, cow.
const ANIMAL_FRAMES: [u16; 4] = [2, 1, 4, 3];

/// Animal Sounds (`AnSd`) and Assorted Sounds (`MsSd`), 9: a recorded
/// sound (`1006/<base + choice>`) while the input is on.
#[derive(Debug)]
pub struct NoiseMaker {
    /// 200 for the animals, 300 for the assorted sounds.
    base: u32,
    sound: Option<SoundData>,
}

impl NoiseMaker {
    fn animals(&self) -> bool {
        self.base == 200
    }
}

impl Behavior for NoiseMaker {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        let f = if self.animals() {
            ANIMAL_FRAMES
                .get(core.settings.numbers[0] as usize)
                .copied()
                .unwrap_or(1)
        } else {
            1
        };
        cx.frame(core.sheet, f, core.h, core.v);
        if core.broken {
            draw_broken(core, cx);
        }
    }

    fn init(&mut self, core: &mut Core) {
        if self.animals() {
            core.settings.numbers[0] = 1;
        }
    }

    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        self.sound = None;
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        self.sound = io.assets.sound(self.base + core.settings.numbers[0]);
        if self.sound.is_none() {
            core.broken = true;
        }
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        match &self.sound {
            Some(s) if !core.broken && io.number(core, 0) != 0.0 => {
                if core.object.is_none() {
                    io.fire(core, 1);
                }
                core.object = Some(Object::Sound(s.clone()));
            }
            _ => core.object = None,
        }
        if io.changed(core, 0) {
            io.fire(core, 1);
        }
    }

    fn has_object(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

#[must_use]
pub fn animal_sound() -> Box<dyn Behavior> {
    Box::new(NoiseMaker { base: 200, sound: None })
}

#[must_use]
pub fn misc_sound() -> Box<dyn Behavior> {
    Box::new(NoiseMaker { base: 300, sound: None })
}

// ---------------------------------------------------------------------------
// The Sound Transformer

/// The pitch factors (`0x49b2a0`): up a third (really a fourth), a fifth,
/// an octave; down a third, a fifth, an octave.
pub const TRANSFORMER_FACTORS: [f64; 6] = [1.333, 1.5, 2.0, 0.75, 0.667, 0.5];

/// The Sound Transformer (`SdTr`, 10): its input's sound, backwards
/// and/or at another pitch.
#[derive(Debug, Default)]
pub struct SoundTransformer {
    /// The reverse buffer (`+0x1a1`), kept between sounds: its first byte
    /// is never written (the original's leaves it as `malloc` did; here
    /// silence).
    reverse: Vec<u8>,
    /// The five level bars (`+0x1a5`), 0 to 6.
    levels: [i8; 5],
}

impl SoundTransformer {
    /// The sound the settings make of `s`, and whether the pitch went out
    /// of range (the part's broken flag; the rate is then kept).
    #[must_use]
    pub fn transform(&mut self, core: &Core, s: &SoundData) -> (SoundData, bool) {
        let mut out = s.clone();
        let mut bad = false;
        let transform = core.settings.radios[0] != 0;
        if transform && core.settings.check(1) {
            if self.reverse.len() != TAPE_LEN {
                self.reverse = vec![0x80; TAPE_LEN];
            }
            let src = &s.samples;
            let n = src.len().min(TAPE_LEN);
            for j in (1..n).rev() {
                self.reverse[j] = src[n - 1 - j];
            }
            // The original keeps the source's length even past 40000
            // samples, reading beyond its buffer; the port stops at 40000.
            out.samples = self.reverse[..n].into();
        }
        if transform && core.settings.check(0) {
            let factor = TRANSFORMER_FACTORS.get(core.settings.numbers[0] as usize).copied();
            match factor.map(|f| f64::from(out.rate) * f) {
                Some(r) if r > 1.0 && r < 65535.0 => out.rate = clamp_int(r) as u16,
                _ => bad = true,
            }
        }
        (out, bad)
    }
}

impl Behavior for SoundTransformer {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, 1, core.h, core.v);
        for (i, &l) in self.levels.iter().enumerate() {
            if l != 0 {
                let (x, y) = point(core, cx, i);
                cx.frame(core.sheet, (l + 2) as u16, x, y);
            }
        }
    }

    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        self.reverse = Vec::new();
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        core.broken = false;
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        let v = io.value_changed(core, 0);
        let c = io.changed(core, 0);
        if c || v {
            let Some(s) = io.input(core, 0).sound().cloned() else {
                // No sound: no output, and (as the original returns here)
                // neither a change nor the bars this tick.
                core.object = None;
                return;
            };
            let (out, bad) = self.transform(core, &s);
            if bad {
                core.broken = true;
            }
            core.object = Some(Object::Sound(out));
            if c {
                io.fire(core, 1);
            }
        }
        if io.throttle(core, 20) {
            for l in &mut self.levels {
                let r = (super::common::lrand(&mut io.env.rng) % 3) as i8;
                *l = (*l + 1 - r).clamp(0, 6);
            }
        }
    }

    fn has_object(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

#[must_use]
pub fn sound_transformer() -> Box<dyn Behavior> {
    Box::new(SoundTransformer::default())
}

// ---------------------------------------------------------------------------
// The Sound Switch

/// Its sensitivity's thresholds (`0x49b1e0`): high, medium, low.
const SWITCH_THRESHOLDS: [u8; 3] = [7, 20, 50];

/// The Sound Switch (`SdSw`, 11): turns on when the microphone hears a
/// sound, and stays on. Every Sound Switch shares the one listening: once
/// any of them hears something, all of them latch on until reset.
#[derive(Debug, Default)]
pub struct SoundSwitch {
    /// This switch ended the listening (the original's global `0x49b1dc`
    /// is "any switch has").
    done: bool,
}

impl SoundSwitch {
    /// One tick with what the microphone has: the bytes recorded so far
    /// and the 22000-byte buffer, or `None` with no device (`0x452c87`).
    pub fn listen(&mut self, core: &mut Core, io: &mut Io<'_>, heard: Option<(u32, &[u8])>) {
        let done = self.done
            || io
                .gadgets
                .iter()
                .filter(|g| g.core.id != core.id && g.core.tag == Tag::new(b"SdSw"))
                .filter_map(|g| g.behavior.as_any().downcast_ref::<SoundSwitch>())
                .any(|s| s.done);
        if done {
            if !core.state {
                core.state = true;
            }
            return;
        }
        let (on, err) = match heard {
            // The original reads the position as an i16: past 32767 it's
            // an error too (it restarts at 19800, so that doesn't happen).
            Some((pos, buf)) if pos <= 0x7fff => {
                let t = SWITCH_THRESHOLDS
                    .get(usize::from(core.settings.radios[0]))
                    .copied()
                    .unwrap_or(SWITCH_THRESHOLDS[2]);
                let n = buf
                    .iter()
                    .take(LISTEN_LEN)
                    .filter(|&&b| (i32::from(b) - 128).unsigned_abs() as u8 > t)
                    .count();
                // Started over before the buffer is full.
                if pos > 19800 {
                    io.env.listen.start();
                }
                (n > 100, false)
            }
            _ => (false, true),
        };
        if on != core.state || err != core.broken {
            core.state = on;
            self.done = true;
            io.env.listen.stop();
            if on {
                io.fire(core, 0);
            }
            core.broken = err;
        }
    }
}

impl Behavior for SoundSwitch {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, 1, core.h, core.v);
        if !core.state {
            let (x, y) = point(core, cx, 0);
            cx.frame(core.sheet, 3, x, y);
        }
        if core.broken {
            draw_broken(core, cx);
        }
    }

    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        self.done = false;
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        self.done = false;
        // `DAT_0049b1d8`: the first switch closes the output and listens.
        if !io.env.listen.on {
            io.env.sounds.push(SoundCmd::StopAll);
            io.env.listen.start();
        }
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        // `listen_pos` and the buffer; no device reads an error: broken,
        // then on.
        let l = &io.env.listen;
        let heard = l.pos.map(|p| (p, l.buf.clone()));
        self.listen(core, io, heard.as_ref().map(|(p, b)| (*p, &b[..])));
    }

    fn has_bool(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

#[must_use]
pub fn sound_switch() -> Box<dyn Behavior> {
    Box::new(SoundSwitch::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::Assets;
    use crate::gadget::{Gadget, Knot, Unknown};
    use crate::sim::{Env, Wire};
    use formats::tables::{KnotDef, KnotKind, Point};

    fn knot(kind: KnotKind, wire: Option<usize>) -> Knot {
        Knot {
            def: KnotDef {
                at: Point { v: 0, h: 0 },
                kind,
            },
            wire,
        }
    }

    fn io<'a>(assets: &'a Assets, gadgets: &'a [Gadget], wires: &'a mut [Wire], env: &'a mut Env) -> Io<'a> {
        Io {
            assets,
            gadgets,
            wires,
            env,
        }
    }

    #[test]
    fn metronome_fires_on_the_rising_edge_only() {
        let assets = Assets::default();
        let mut core = Core::new(Tag::new(b"Metr"), 1011, 1011);
        let mut wires = vec![Wire {
            from: 0,
            from_knot: 0,
            to: 1,
            to_knot: 0,
            flag: 0,
            points: Vec::new(),
            changed: false,
        }];
        core.knots = vec![knot(KnotKind::Output, Some(0))];
        let mut m = Metronome::default();
        m.init(&mut core);
        m.reset(&mut core);
        assert_eq!(core.settings.numbers[0], 4, "a fresh one is 30 a minute");
        let mut env = Env::default();
        m.start(&mut core, &mut io(&assets, &[], &mut wires, &mut env));
        let mut fired = Vec::new();
        let mut on = Vec::new();
        for t in 0..250 {
            env.ticks = t;
            m.tick(&mut core, &mut io(&assets, &[], &mut wires, &mut env));
            on.push(core.state);
            if std::mem::take(&mut wires[0].changed) {
                fired.push(t);
            }
        }
        // Period 120: on for ticks 0..15 and 120..135, fired at each start.
        assert_eq!(fired, [0, 120, 240]);
        assert!(on[14] && !on[15] && on[120] && !on[200]);
    }

    #[test]
    fn noon_shows_as_am() {
        assert_eq!(DigitalClock::hour_shown(true, 12), (12, Some(3)));
        assert_eq!(DigitalClock::hour_shown(true, 0), (12, Some(3)));
        assert_eq!(DigitalClock::hour_shown(true, 13), (1, Some(4)));
        assert_eq!(DigitalClock::hour_shown(false, 13), (13, None));
    }

    #[test]
    fn transformer_factors() {
        let mut t = SoundTransformer::default();
        let mut core = Core::new(Tag::new(b"SdTr"), 2038, 1044);
        core.settings.radios[0] = 1;
        core.settings.set_check(0, true);
        let s = SoundData {
            rate: 11025,
            samples: vec![1, 2, 3, 4].into(),
        };
        let rates: Vec<u16> = (0..6)
            .map(|i| {
                core.settings.numbers[0] = i;
                t.transform(&core, &s).0.rate
            })
            .collect();
        assert_eq!(rates, [14696, 16537, 22050, 8268, 7353, 5512]);
        // Out of range: broken, rate kept.
        let fast = SoundData {
            rate: 40000,
            ..s.clone()
        };
        core.settings.numbers[0] = 2;
        assert_eq!(t.transform(&core, &fast), (fast.clone(), true));
        // Backwards: the first byte isn't written, the last is dropped.
        core.settings.set_check(0, false);
        core.settings.set_check(1, true);
        assert_eq!(&*t.transform(&core, &s).0.samples, &[0x80, 3, 2, 1]);
        // "Play with no changes".
        core.settings.radios[0] = 0;
        assert_eq!(t.transform(&core, &s).0, s);
    }

    #[test]
    fn sound_switches_latch_together() {
        let assets = Assets::default();
        let mut env = Env::default();
        let mut a = Core::new(Tag::new(b"SdSw"), 2037, 1045);
        a.id = 1;
        let mut b = a.clone();
        b.id = 2;
        let mut sa = SoundSwitch::default();
        let mut sb = SoundSwitch::default();
        let quiet = vec![0x80; 22000];
        let mut loud = quiet.clone();
        loud[..101].fill(0xff);
        // Quiet: nothing happens.
        sa.listen(&mut a, &mut io(&assets, &[], &mut [], &mut env), Some((100, &quiet)));
        assert!(!a.state && !sa.done);
        // A sound: `a` latches on.
        sa.listen(&mut a, &mut io(&assets, &[], &mut [], &mut env), Some((200, &loud)));
        assert!(a.state && sa.done);
        // `b` (low sensitivity, quiet buffer) is forced on because `a` heard.
        b.settings.radios[0] = 2;
        let others = [Gadget {
            core: a.clone(),
            behavior: Box::new(std::mem::take(&mut sa)),
        }];
        sb.listen(
            &mut b,
            &mut io(&assets, &others, &mut [], &mut env),
            Some((300, &quiet)),
        );
        assert!(b.state);
        // And `a` stays on, quiet or not.
        let mut sa = SoundSwitch { done: true };
        sa.listen(&mut a, &mut io(&assets, &[], &mut [], &mut env), Some((400, &quiet)));
        assert!(a.state);
        // No microphone: broken first, then on.
        let mut c = Core::new(Tag::new(b"SdSw"), 2037, 1045);
        let mut sc = SoundSwitch::default();
        sc.listen(&mut c, &mut io(&assets, &[], &mut [], &mut env), None);
        assert!(c.broken && !c.state);
        sc.listen(&mut c, &mut io(&assets, &[], &mut [], &mut env), None);
        assert!(c.state);
    }

    #[test]
    fn recorder_round_trip() {
        let mut core = Core::new(Tag::new(b"Tape"), 2026, 1037);
        let mut t = TapePlayer::default();
        let data: Vec<u8> = (0..TAPE_LEN).map(|i| (i * 7) as u8).collect();
        t.load(&mut core, &data);
        let saved = t.save(&core);
        assert_eq!(saved, data);
        assert_eq!(0x150 + saved.len(), 0x9d90);
    }

    #[test]
    fn wav_import() {
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x02\0");
        wav.extend_from_slice(&22050u32.to_le_bytes());
        wav.extend_from_slice(&[0; 6]);
        wav.extend_from_slice(&8u16.to_le_bytes());
        wav.extend_from_slice(b"data\0\0\0\0");
        assert_eq!(wav.len(), 44);
        // Stereo frames (l, r) at 22050: every second frame's right channel.
        wav.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let s = import_wav(&wav).unwrap();
        assert_eq!(&s[..3], &[4, 8, 0x80]);
        assert_eq!(s.len(), TAPE_LEN);
        wav[0x22] = 16;
        assert_eq!(import_wav(&wav), Err(WavError::Not8Bit));
        assert_eq!(import_wav(b"nope"), Err(WavError::BadFormat));
    }

    /// A 44-byte wave header: `channels`, `rate`, `bits`.
    pub(crate) fn wav(channels: u16, rate: u32, bits: u16, data: &[u8]) -> Vec<u8> {
        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0");
        w.extend_from_slice(&channels.to_le_bytes());
        w.extend_from_slice(&rate.to_le_bytes());
        w.extend_from_slice(&[0; 6]);
        w.extend_from_slice(&bits.to_le_bytes());
        w.extend_from_slice(b"data\0\0\0\0");
        w.extend_from_slice(data);
        w
    }

    #[test]
    fn wav_import_rates_channels_and_errors() {
        // Mono 11025: copied as is, up to 40000, the rest silence.
        let data: Vec<u8> = (0..50000u32).map(|i| (i % 251) as u8).collect();
        let s = import_wav(&wav(1, 11025, 8, &data)).unwrap();
        assert_eq!(s, data[..TAPE_LEN]);
        let s = import_wav(&wav(1, 11025, 8, &[1, 2, 3])).unwrap();
        assert_eq!(&s[..4], &[1, 2, 3, 0x80]);
        // 32000: the last of each three; a group cut short is dropped.
        let s = import_wav(&wav(1, 32000, 8, &[1, 2, 3, 4, 5, 6, 7, 8])).unwrap();
        assert_eq!(&s[..3], &[3, 6, 0x80]);
        // 44100 stereo: the right channel of every fourth frame.
        let frames: Vec<u8> = (0..16).collect();
        let s = import_wav(&wav(2, 44100, 8, &frames)).unwrap();
        assert_eq!(&s[..3], &[7, 15, 0x80]);
        // 21999 isn't reduced; three channels count as one.
        let s = import_wav(&wav(3, 21999, 8, &[9, 8])).unwrap();
        assert_eq!(&s[..3], &[9, 8, 0x80]);
        // The header: short, not RIFF, not WAVE, no "fmt ", 16 bits.
        assert_eq!(import_wav(&wav(1, 11025, 8, &[])[..43]), Err(WavError::BadFormat));
        let mut w = wav(1, 11025, 8, &[1]);
        w[0] = b'X';
        assert_eq!(import_wav(&w), Err(WavError::BadFormat));
        let mut w = wav(1, 11025, 8, &[1]);
        w[8] = b'X';
        assert_eq!(import_wav(&w), Err(WavError::BadFormat));
        let mut w = wav(1, 11025, 8, &[1]);
        w[12] = b'X';
        assert_eq!(import_wav(&w), Err(WavError::BadFormat));
        assert_eq!(import_wav(&wav(1, 11025, 16, &[1, 2])), Err(WavError::Not8Bit));
        // An import clears the broken flag; a failed one leaves it.
        let mut core = Core::new(Tag::new(b"Tape"), 2026, 1037);
        let mut t = TapePlayer::default();
        core.broken = true;
        assert!(t.import(&mut core, b"nope").is_err());
        assert!(core.broken);
        t.import(&mut core, &wav(1, 11025, 8, &[5])).unwrap();
        assert!(!core.broken);
        assert_eq!(t.samples()[..2], [5, 0x80]);
        // A recording: what came, then silence.
        t.record(&[1, 2]);
        assert_eq!(t.samples()[..3], [1, 2, 0x80]);
        assert_eq!(t.samples().len(), TAPE_LEN);
    }

    #[test]
    fn sound_switch_listens_through_the_env() {
        let assets = Assets::default();
        let mut env = Env::default();
        let mut core = Core::new(Tag::new(b"SdSw"), 2037, 1045);
        core.id = 1;
        let mut s = SoundSwitch::default();
        // The first switch closes the output and starts the listening.
        s.start(&mut core, &mut io(&assets, &[], &mut [], &mut env));
        assert!(env.listen.on && env.listen.pos == Some(0));
        assert!(env.sounds.contains(&SoundCmd::StopAll));
        // Quiet past 19800 bytes: started over.
        env.listen.feed(&vec![0x80; 19801]);
        s.tick(&mut core, &mut io(&assets, &[], &mut [], &mut env));
        assert!(!core.state && env.listen.pos == Some(0) && env.listen.on);
        // A sound: on, and the listening stops.
        env.listen.feed(&[0xff; 101]);
        s.tick(&mut core, &mut io(&assets, &[], &mut [], &mut env));
        assert!(core.state && !env.listen.on);
        // Past the end of the buffer the samples are lost.
        env.listen.start();
        env.listen.feed(&vec![0x80; 30000]);
        assert_eq!(env.listen.pos, Some(22000));
    }

    #[test]
    fn speaker_tones() {
        assert_eq!(tone_rate(220.0), (220, 11025));
        assert_eq!(tone_rate(440.0), (220, 22050));
        assert_eq!(tone_rate(2000.0).0, 224);
        assert_eq!(parse_double(b" 261.6Hz"), 261.6);
        assert_eq!(parse_double(b"abc"), 0.0);
    }

    #[test]
    fn stopwatch_counts_and_resets_silently() {
        struct Level(f64);
        impl Behavior for Level {
            fn has_number(&self, _core: &Core, _knot: usize) -> bool {
                true
            }
            fn number_out(&self, _core: &Core, _knot: usize) -> f64 {
                self.0
            }
            any!();
        }
        let assets = Assets::default();
        let mut env = Env::default();
        // Inputs START (0), STOP (1), RESET (2) from three constant sources.
        let src = |id, v| {
            let mut c = Core::new(Tag::new(b"src_"), 0, 0);
            c.id = id;
            Gadget {
                core: c,
                behavior: Box::new(Level(v)) as Box<dyn Behavior>,
            }
        };
        let wire = |from, to_knot| Wire {
            from,
            from_knot: 0,
            to: 3,
            to_knot,
            flag: 0,
            points: Vec::new(),
            changed: false,
        };
        let mut core = Core::new(Tag::new(b"Wtch"), 1004, 1004);
        core.id = 10;
        core.knots = vec![
            knot(KnotKind::Input, Some(0)),
            knot(KnotKind::Input, Some(1)),
            knot(KnotKind::Input, Some(2)),
            knot(KnotKind::Output, Some(3)),
        ];
        let me = Gadget {
            core: core.clone(),
            behavior: Box::new(Unknown::default()),
        };
        let mut gadgets = vec![src(1, 1.0), src(2, 0.0), src(3, 0.0), me];
        let mut wires = vec![wire(0, 0), wire(1, 1), wire(2, 2), wire(3, 3)];
        let mut w = StopWatch::default();
        w.reset(&mut core);
        for t in 0..=300 {
            env.ticks = t;
            w.tick(&mut core, &mut io(&assets, &gadgets, &mut wires, &mut env));
        }
        // Started on tick 0, shown from tick 1 on: 300 ticks, 5 seconds.
        assert_eq!(core.number, 5.0);
        assert!(std::mem::take(&mut wires[3].changed));
        gadgets[2] = src(3, 1.0);
        env.ticks = 301;
        w.tick(&mut core, &mut io(&assets, &gadgets, &mut wires, &mut env));
        assert_eq!(core.number, 0.0);
        assert!(!wires[3].changed, "a reset doesn't fire");
    }
}
