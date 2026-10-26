//! The parts a person works: the Light Switch (`swit`), the Slider
//! (`Sldr`), the Numeric Keypad (`NmKP`), the Keypress (`Key_`), the Mouse
//! Button (`MsCl`), the Mouse Distance Measurer (`MsMv`), the Dynamite
//! Plunger (`DnPl`) and the Heart (`hart`). docs/gadgets/io.md has the
//! original's code. The keyboard never reaches a gadget and only a press of
//! the mouse on one does (slot 0x14); the rest is polled each tick from
//! [`crate::sim::Env`], as the original polls `GetKeyState` (io.md 1.2).

// The original's integer and float arithmetic, reproduced on purpose.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::float_cmp
)]

use std::cell::Cell;

use formats::hr::Rect;
use formats::tables::Point;

use super::common::{any, format_number, parse_num, scan_f64, trunc_sat};
use super::x87::Ext;
use crate::assets::{Assets, Module, rt};
use crate::gadget::{Behavior, Core, DrawCx};
use crate::sim::Io;

// ---------------------------------------------------------------------------
// Shared helpers (also the displays' and the text parts')

/// Point list `id` (type 1010, `0x41141f`).
pub(super) fn points(assets: &Assets, id: u32) -> Vec<Point> {
    assets
        .get(Module::Gadget, rt::POINTS, id)
        .map(formats::tables::points)
        .unwrap_or_default()
}

/// Point `i` of list `id` as (`h`, `v`), or `default` if there's none.
pub(super) fn point_or(assets: &Assets, id: u32, i: usize, default: (i32, i32)) -> (i32, i32) {
    points(assets, id)
        .get(i)
        .map_or(default, |p| (i32::from(p.h), i32::from(p.v)))
}

/// Rect list `id` (type 1012, `0x411f9a`).
pub(super) fn rects(assets: &Assets, id: u32) -> Vec<Rect> {
    assets
        .get(Module::Gadget, rt::RECTS, id)
        .map(formats::tables::rects)
        .unwrap_or_default()
}

/// `PtInRect` (`0x411e03`), Mac semantics: `top <= v < bottom && left <=
/// h < right`.
pub(super) fn in_rect(r: Rect, h: i32, v: i32) -> bool {
    i32::from(r.top) <= v && v < i32::from(r.bottom) && i32::from(r.left) <= h && h < i32::from(r.right)
}

/// Slot 0x10 (`0x41ed9d`): the broken picture (sheet 101), frame 1, or
/// frame 2 where frame 1 doesn't fit, centred on the gadget.
pub(super) fn draw_broken(core: &Core, cx: &mut DrawCx<'_>) {
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
    cx.frame(101, n, core.h + (gw - fw) / 2, core.v + (gh - fh) / 2);
}

/// `floor` (`0x480e90`, `frndint` with RC=down) of an x87 value, as a
/// double: the double nearest `x` may have rounded up onto an integer.
pub(super) fn ext_floor(x: Ext) -> f64 {
    let f = x.to_f64().floor();
    if Ext::from(f) > x { f - 1.0 } else { f }
}

/// `ftol_clamped` (`0x439e3d`) of an x87 value: truncated toward zero
/// before any rounding to a double.
pub(super) fn ext_ftol(x: Ext) -> i32 {
    let f = x.to_f64();
    let mut t = f.trunc();
    if t == f {
        if f > 0.0 && Ext::from(t) > x {
            t -= 1.0;
        } else if f < 0.0 && Ext::from(t) < x {
            t += 1.0;
        }
    }
    trunc_sat(t)
}

/// Borland's `toupper` (`0x480bc4`) in the C locale.
pub(super) fn toupper(c: u8) -> u8 {
    c.to_ascii_uppercase()
}

// ---------------------------------------------------------------------------
// The Light Switch

/// The switch's click sound (WWSOUND 1006/1012).
const SWITCH_SOUND: u32 = 1012;

/// The Light Switch (`swit`, io.md 3): a click toggles it; "On at Start"
/// (radio group 0) is its position after every stop.
#[derive(Debug, Default)]
pub struct Switch;

impl Behavior for Switch {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame_at_rect(core.sheet, if core.state { 1 } else { 2 }, core.h, core.v);
    }

    fn bounds_frame(&self) -> u16 {
        2
    }

    fn click(&mut self, core: &mut Core, io: &mut Io<'_>, _x: i32, _y: i32) {
        core.state = !core.state;
        // Only off to on is an event.
        if core.state {
            io.fire(core, 0);
        }
        io.play(SWITCH_SOUND);
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        if core.state {
            io.fire(core, 0);
        }
    }

    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        core.state = core.settings.radios[0] != 0;
    }

    fn settings_changed(&mut self, core: &mut Core) {
        core.state = core.settings.radios[0] != 0;
    }

    fn has_bool(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

#[must_use]
pub fn switch() -> Box<dyn Behavior> {
    Box::new(Switch)
}

// ---------------------------------------------------------------------------
// The Slider

/// The slider's geometry, from its points (1010/1015) and frame sizes
/// (class init `0x4516c5`).
#[derive(Debug, Clone, Copy)]
struct SliderGeom {
    /// `P0`: the first digit; `P1`: the step between digits.
    digit: (i32, i32),
    step: (i32, i32),
    /// `P2`, `P3`: the down and up buttons' top left.
    down: (i32, i32),
    up: (i32, i32),
    /// `P4`, `P5`: the track's ends (the knob's top left at 0 and at
    /// `TRACK_W`).
    track0: (i32, i32),
    track1: (i32, i32),
}

/// Frame 15's size (the arrow buttons), `(h, v)`.
const SLIDER_BUTTON: (i32, i32) = (15, 14);
/// Frame 18's size (the knob), `(h, v)`.
const SLIDER_KNOB: (i32, i32) = (18, 43);

impl SliderGeom {
    fn load(assets: &Assets, sheet: u32) -> SliderGeom {
        let p = |i, d| point_or(assets, sheet, i, d);
        SliderGeom {
            digit: p(0, (29, 3)),
            step: p(1, (12, 0)),
            down: p(2, (3, 3)),
            up: p(3, (157, 3)),
            track0: p(4, (29, 20)),
            track1: p(5, (129, 20)),
        }
    }

    /// `Pt2Rect(a, a + size)` (`0x41198a`).
    fn button(a: (i32, i32)) -> Rect {
        let (h1, v1) = (a.0 + SLIDER_BUTTON.0, a.1 + SLIDER_BUTTON.1);
        mac_rect(a.0.min(h1), a.1.min(v1), a.0.max(h1), a.1.max(v1))
    }

    /// `TRACK.left` and `TRACK_W` (`0x4a32e4 - 0x4a32e0`, as i16).
    fn track(self) -> (i32, i32) {
        let left = self.track0.0.min(self.track1.0);
        let right = self.track0.0.max(self.track1.0);
        (left, i32::from((right - left) as i16))
    }
}

fn mac_rect(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
    Rect {
        top: top as i16,
        left: left as i16,
        bottom: bottom as i16,
        right: right as i16,
    }
}

/// The Slider (`Sldr`, io.md 4): a knob over a 100-pixel track; its value
/// is the knob's place in the range (preset 0-10 or 0-100, or custom).
#[derive(Debug, Default)]
pub struct Slider {
    /// Sticky decimals for the number shown (`+0x196`); the original
    /// updates it as it draws.
    decimals: Cell<u8>,
    /// The knob, 0 to `TRACK_W` (`+0x197`): kept by reset, not saved.
    knob: i16,
    /// A tick counter (`+0x199`).
    counter: u8,
    /// Dragging the knob (`+0x19a`).
    dragging: bool,
    /// An arrow button shown pressed (`+0x1a3`), its frame (`+0x1a4`) and
    /// point (`+0x1a5`).
    pressed: bool,
    pressed_frame: u16,
    pressed_point: usize,
}

impl Slider {
    /// `0x45238c`: the value at the knob, in the x87's precision.
    fn value(&self, core: &Core, track_w: i32) -> f64 {
        let t = Ext::from(i32::from(self.knob)).div(Ext::from(track_w));
        let s = &core.settings;
        let (lo, hi) = if s.radios[0] != 0 {
            let (lo, hi) = (parse_num(s.label(0)), parse_num(s.label(1)));
            if hi < lo { (hi, lo) } else { (lo, hi) }
        } else {
            match s.numbers[0] {
                0 => (0.0, 10.0),
                // Other presets read uninitialised stack; the dialog
                // can't make one.
                _ => (0.0, 100.0),
            }
        };
        Ext::from(hi).sub(lo).mul(t).add(lo).to_f64()
    }
}

impl Behavior for Slider {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        let g = SliderGeom::load(cx.assets, core.sheet);
        let (h, v) = (core.h, core.v);
        cx.frame_at_rect(core.sheet, 16, h, v);
        // The knob shows in edit mode too.
        cx.frame_at_rect(core.sheet, 18, h + g.track0.0 + i32::from(self.knob), v + g.track0.1);
        if !cx.running {
            return;
        }
        let mut d = self.decimals.get();
        let s = format_number(core.number, &mut d);
        self.decimals.set(d);
        for (i, c) in s.bytes().take(10).enumerate() {
            let f = match c {
                b' ' => continue,
                b'.' => 1,
                b'-' => 2,
                b'+' => 3,
                b'E' | b'e' => 4,
                b'0'..=b'9' => u16::from(c - b'+'),
                _ => {
                    draw_broken(core, cx);
                    return;
                }
            };
            let i = i as i32;
            cx.frame_at_rect(
                core.sheet,
                f,
                h + g.digit.0 + i * g.step.0,
                v + g.digit.1 + i * g.step.1,
            );
        }
        if self.pressed {
            let p = if self.pressed_point == 3 { g.up } else { g.down };
            cx.frame_at_rect(core.sheet, self.pressed_frame, h + p.0, v + p.1);
        }
    }

    fn click(&mut self, core: &mut Core, io: &mut Io<'_>, x: i32, y: i32) {
        let g = SliderGeom::load(io.assets, core.sheet);
        let (_, track_w) = g.track();
        let knob = i32::from(self.knob);
        if in_rect(SliderGeom::button(g.up), x, y) {
            if knob < track_w {
                self.knob += 1;
                self.pressed = true;
                self.pressed_point = 3;
                self.pressed_frame = 15;
            }
        } else if in_rect(SliderGeom::button(g.down), x, y) {
            if knob > 0 {
                self.knob -= 1;
                self.pressed = true;
                self.pressed_point = 2;
                self.pressed_frame = 17;
            }
        } else {
            let (kh, kv) = (g.track0.0 + knob, g.track0.1);
            if in_rect(mac_rect(kh, kv, kh + SLIDER_KNOB.0, kv + SLIDER_KNOB.1), x, y) {
                self.dragging = true;
            }
        }
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        let g = SliderGeom::load(io.assets, core.sheet);
        let (left, track_w) = g.track();
        self.counter = self.counter.wrapping_add(1);
        if !self.dragging {
            // Ticks 8 to 15 of every 16: an arrow's step shows late, and
            // is an event only once the button is let go.
            if self.counter & 8 != 0 {
                core.number = self.value(core, track_w);
                if self.pressed && !io.env.any_button() {
                    self.pressed = false;
                    io.fire(core, 0);
                }
            }
        } else if !io.env.any_button() {
            self.dragging = false;
        } else {
            // The knob's middle under the cursor.
            let mut x = i32::from((io.env.mouse.0 - core.h) as i16) - left - 9;
            if x < 0 {
                x = 0;
            } else if x > track_w {
                x = track_w;
            }
            if i32::from(self.knob) != x {
                self.knob = x as i16;
                core.number = self.value(core, track_w);
                io.fire(core, 0);
            }
        }
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        let (_, track_w) = SliderGeom::load(io.assets, core.sheet).track();
        core.number = self.value(core, track_w);
        if core.number != 0.0 {
            io.fire(core, 0);
        }
    }

    /// `0x45202a`: not the base reset; the value and the knob stay.
    fn reset(&mut self, _core: &mut Core) {
        self.decimals.set(0);
        self.pressed = false;
    }

    fn init(&mut self, core: &mut Core) {
        // "0-100".
        core.settings.numbers[0] = 1;
    }

    fn has_number(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

#[must_use]
pub fn slider() -> Box<dyn Behavior> {
    Box::new(Slider::default())
}

// ---------------------------------------------------------------------------
// The Numeric Keypad

/// The keypad's reject beep (WWSOUND 1006/1027).
const KEYPAD_SOUND: u32 = 1027;
/// The LCD's width in characters.
const LCD: usize = 10;
/// The pressed-button frame meaning "ENTER was last" (`+0x1a3`).
const KEY_ENTER: u16 = 29;

/// The keypad's button rects (1012/1027), when the resource is missing.
const KEYPAD_RECTS: [(i16, i16, i16, i16); 14] = [
    (92, 10, 108, 29),
    (73, 10, 89, 29),
    (73, 33, 89, 52),
    (73, 56, 89, 75),
    (54, 10, 70, 29),
    (54, 33, 70, 52),
    (54, 56, 70, 75),
    (35, 10, 51, 29),
    (35, 33, 51, 52),
    (35, 56, 51, 75),
    (92, 33, 108, 52),
    (111, 10, 127, 29),
    (92, 56, 108, 75),
    (111, 33, 127, 75),
];

fn keypad_rects(assets: &Assets, sheet: u32) -> Vec<Rect> {
    let r = rects(assets, sheet);
    if r.len() >= KEYPAD_RECTS.len() {
        return r;
    }
    KEYPAD_RECTS
        .iter()
        .map(|&(top, left, bottom, right)| Rect {
            top,
            left,
            bottom,
            right,
        })
        .collect()
}

/// `0x44cff6`: whether the whole LCD is a number the keypad allows:
/// spaces, an optional `-`, digits, an optional `.` and digits, and an
/// optional exponent of one or two digits.
#[must_use]
pub fn keypad_valid(s: &[u8]) -> bool {
    let mut i = s.iter().take_while(|&&c| c == b' ').count();
    let digits = |i: usize| s[i..].iter().take_while(|c| c.is_ascii_digit()).count();
    if s.get(i) == Some(&b'-') {
        i += 1;
    }
    i += digits(i);
    if s.get(i) == Some(&b'.') {
        i += 1;
        i += digits(i);
    }
    if matches!(s.get(i), Some(b'e' | b'E')) {
        i += 1;
        if s.get(i) == Some(&b'-') {
            i += 1;
        }
        let n = digits(i);
        if n == 0 {
            return false;
        }
        i += n.min(2);
    }
    i == s.len()
}

/// The Numeric Keypad (`NmKP`, io.md 5): an LCD a number is typed into,
/// out on ENTER.
#[derive(Debug)]
pub struct NumericKeypad {
    /// The LCD (`+0x197`), right-aligned, space-padded.
    lcd: [u8; LCD],
    /// A button shown pressed (`+0x1a2`), its frame (`+0x1a3`, 16 to 29)
    /// and its rect (`+0x1a4`).
    pressed: bool,
    pressed_frame: u16,
    pressed_rect: usize,
}

impl Default for NumericKeypad {
    fn default() -> NumericKeypad {
        let mut k = NumericKeypad {
            lcd: [b' '; LCD],
            pressed: false,
            pressed_frame: 0,
            pressed_rect: 0,
        };
        k.clear();
        k
    }
}

impl NumericKeypad {
    /// The LCD's text.
    #[must_use]
    pub fn lcd(&self) -> &[u8; LCD] {
        &self.lcd
    }

    /// `0x44d726`: `"         0"`.
    fn clear(&mut self) {
        self.lcd = [b' '; LCD];
        self.lcd[LCD - 1] = b'0';
    }

    /// `0x44d79e`: types `c`; returns whether it was rejected (the LCD is
    /// full, or it wouldn't be a number). A lone 0 is replaced.
    fn append(&mut self, c: u8) -> bool {
        let saved = self.lcd;
        if self.lcd[0] == b' ' {
            let l = &mut self.lcd;
            if l[9] != b'0' || (l[8] != b' ' && l[8] != b'-') || c == b'.' {
                l.copy_within(1.., 0);
            }
            l[9] = c;
            if keypad_valid(&self.lcd) {
                return false;
            }
        }
        self.lcd = saved;
        true
    }

    /// `0x44da9d`: the LCD's number out (unchanged if it doesn't read).
    fn enter(&self, core: &mut Core, io: &mut Io<'_>) {
        let p = self.lcd.iter().position(|&c| c != b' ').unwrap_or(LCD);
        if let Some(x) = scan_f64(&self.lcd[p..]) {
            core.number = x;
        }
        io.fire(core, 0);
    }

    /// The sign button (rect 11).
    fn sign(&mut self, io: &mut Io<'_>) {
        if self.lcd[0] == b' ' || self.lcd[0] == b'-' {
            let k = self.lcd.iter().take_while(|&&c| c == b' ').count();
            if self.lcd.get(k) == Some(&b'-') {
                self.lcd[k] = b' ';
            } else if k > 0 {
                self.lcd[k - 1] = b'-';
            }
        } else if self.append(b'!') {
            // Full: '!' is always rejected, so this only beeps.
            io.play(KEYPAD_SOUND);
        }
    }
}

impl Behavior for NumericKeypad {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        let (h, v) = (core.h, core.v);
        cx.frame_at_rect(core.sheet, 15, h, v);
        // Not gated on running.
        if core.settings.checks & 2 == 0 {
            let (ph, pv) = point_or(cx.assets, core.sheet, 0, (8, 9));
            let mut f = 0;
            for (i, &c) in self.lcd.iter().enumerate() {
                f = match c {
                    b' ' => continue,
                    b'.' => 11,
                    b'-' => 12,
                    b'E' | b'e' => 13,
                    b'0'..=b'9' => u16::from(c - b'/'),
                    // The last frame again (valid() lets nothing else in).
                    _ => f,
                };
                cx.frame_at_rect(core.sheet, f, h + ph + 7 * i as i32, v + pv);
            }
        }
        if self.pressed
            && let Some(r) = keypad_rects(cx.assets, core.sheet).get(self.pressed_rect)
        {
            cx.frame_at_rect(
                core.sheet,
                self.pressed_frame,
                h + i32::from(r.left),
                v + i32::from(r.top),
            );
        }
    }

    fn click(&mut self, core: &mut Core, io: &mut Io<'_>, x: i32, y: i32) {
        // Lock Number.
        if core.settings.checks & 1 != 0 {
            return;
        }
        for (i, r) in keypad_rects(io.assets, core.sheet).into_iter().enumerate().take(14) {
            if !in_rect(r, x, y) {
                continue;
            }
            self.pressed = true;
            self.pressed_rect = i;
            match i {
                10 => {
                    self.pressed_frame = 26;
                    if self.append(b'.') {
                        io.play(KEYPAD_SOUND);
                    }
                }
                11 => {
                    self.pressed_frame = 27;
                    self.sign(io);
                }
                12 => {
                    // The value is kept.
                    self.pressed_frame = 28;
                    self.clear();
                }
                13 => {
                    self.pressed_frame = KEY_ENTER;
                    self.enter(core, io);
                }
                _ => {
                    // The first digit after ENTER starts a new number.
                    if self.pressed_frame == KEY_ENTER {
                        self.clear();
                    }
                    self.pressed_frame = 16 + i as u16;
                    if self.append(b'0' + i as u8) {
                        io.play(KEYPAD_SOUND);
                    }
                }
            }
        }
    }

    fn tick(&mut self, _core: &mut Core, io: &mut Io<'_>) {
        if self.pressed && !io.env.any_button() {
            self.pressed = false;
        }
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        self.enter(core, io);
        // A zero start isn't an event: the flag is taken back.
        if core.number == 0.0 {
            io.changed(core, 0);
        }
        self.pressed_frame = KEY_ENTER;
    }

    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        self.pressed = false;
        // Auto Reset.
        if core.settings.checks & 4 != 0 {
            self.settings_changed(core);
        }
    }

    /// `0x44cfb8`: the LCD typed afresh from text 0.
    fn settings_changed(&mut self, core: &mut Core) {
        self.clear();
        for &c in core.settings.label(0) {
            if self.append(c) {
                break;
            }
        }
    }

    fn has_number(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

#[must_use]
pub fn numeric_keypad() -> Box<dyn Behavior> {
    Box::new(NumericKeypad::default())
}

// ---------------------------------------------------------------------------
// The Keypress

/// The Keypress (`Key_`, cKey, io.md 6): on while its key, or the mouse
/// button after a click on it, is held.
#[derive(Debug, Default)]
pub struct Keypress {
    /// Turned on by a click rather than the key (`+0x197`).
    by_mouse: bool,
}

/// The key, `toupper(text0[0])`, which is also its virtual-key code
/// (right for letters, digits and space).
fn key_of(core: &Core) -> u8 {
    toupper(core.settings.text[0])
}

impl Behavior for Keypress {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame_at_rect(core.sheet, if core.state { 2 } else { 1 }, core.h, core.v);
        // `0x448ad0`: only letters and digits are drawn.
        let k = key_of(core);
        if !k.is_ascii_alphanumeric() {
            return;
        }
        let r = core.rect();
        let r = crate::gfx::IRect::new(r.x0, r.y0, r.x1 - 10, r.y1 - 4);
        let font = if core.settings.radios[0] == 0 {
            super::labels::GadgetFont::Helv16Bold
        } else {
            super::labels::GadgetFont::Helv24
        };
        let black = cx.color([0, 0, 0]);
        super::labels::draw_text(cx, font, r, &[k], black, super::labels::Format::Line);
    }

    fn click(&mut self, core: &mut Core, io: &mut Io<'_>, _x: i32, _y: i32) {
        if !core.state {
            core.state = true;
            self.by_mouse = true;
            io.fire(core, 0);
        }
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        let k = u16::from(key_of(core));
        if core.state {
            // On by a click: until the button (any, anywhere) is let go.
            let held = if self.by_mouse {
                io.env.any_button()
            } else {
                io.env.key_held(k)
            };
            // The release isn't an event.
            if !held {
                core.state = false;
            }
        } else if io.env.key_held(k) {
            core.state = true;
            self.by_mouse = false;
            io.fire(core, 0);
        }
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        if core.state {
            io.fire(core, 0);
        }
    }

    fn init(&mut self, core: &mut Core) {
        core.settings.text[0] = b'A';
        core.settings.text[1] = 0;
    }

    fn has_bool(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

#[must_use]
pub fn keypress() -> Box<dyn Behavior> {
    Box::new(Keypress::default())
}

// ---------------------------------------------------------------------------
// The Mouse Button

/// The Mouse Button (`MsCl`, io.md 7): on while a mouse button is down
/// anywhere, once it has been seen up since the stop.
#[derive(Debug, Default)]
pub struct MouseClick {
    /// The buttons have been seen up since the reset (`+0x197`).
    armed: bool,
}

impl Behavior for MouseClick {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame_at_rect(core.sheet, if core.state { 1 } else { 2 }, core.h, core.v);
    }

    fn bounds_frame(&self) -> u16 {
        2
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        let d = io.env.any_button();
        if !d {
            self.armed = true;
        }
        if self.armed && d != core.state {
            core.state = d;
            // Only the press is an event.
            if d {
                io.fire(core, 0);
            }
        }
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        if core.state {
            io.fire(core, 0);
        }
    }

    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        self.armed = false;
    }

    fn has_bool(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

#[must_use]
pub fn mouse_click() -> Box<dyn Behavior> {
    Box::new(MouseClick::default())
}

// ---------------------------------------------------------------------------
// The Mouse Distance Measurer

/// 1/72 inch: the 80-bit constant at `0x44b4c4`, rounded to a double (the
/// two agree to the double's last bit for this product).
const INCH: f64 = 1.0 / 72.0;

/// The Mouse Distance Measurer (`MsMv`, cMouseMove, io.md 8): how far the
/// cursor went with a button down, in inches (72 screen pixels each); its
/// picture shows how fast.
#[derive(Debug, Default)]
pub struct MouseMove {
    /// The cursor at the last tick (`+0x197`, `+0x199`), in screen pixels.
    /// The original takes it at reset; the port, which can't see the
    /// cursor there, takes it at the first tick after.
    last: Option<(i16, i16)>,
    /// The distance at the last speed sample (`+0x19b`).
    sampled: f64,
    /// The tick of the next speed sample (`+0x1a3`).
    next: u32,
    /// The last speed, inches per 30 ticks (the global `0x499fd8`).
    speed: f64,
}

impl Behavior for MouseMove {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame_at_rect(core.sheet, u16::from(core.phase.max(1)), core.h, core.v);
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        let (x, y) = io.env.cursor_screen;
        let c = (x as i16, y as i16);
        let last = *self.last.get_or_insert(c);
        let mut d = 0.0;
        if c != last {
            if io.env.any_button() {
                let dh = f64::from(i32::from(c.0) - i32::from(last.0));
                let dv = f64::from(i32::from(c.1) - i32::from(last.1));
                d = Ext::from(dh * dh).add(dv * dv).sqrt().to_f64();
            }
            self.last = Some(c);
        }
        core.number = Ext::from(INCH).mul(d).add(core.number).to_f64();
        if d != 0.0 {
            io.fire(core, 0);
        }
        let t = io.elapsed();
        if t > self.next {
            let dt = t.wrapping_sub(self.next).wrapping_add(5);
            self.speed = (core.number - self.sampled) / f64::from(dt) * 30.0;
            self.next = t.wrapping_add(5);
            self.sampled = core.number;
            let f = if self.speed > 5.0 {
                3
            } else if self.speed < 1.5 {
                1
            } else {
                2
            };
            core.phase = f;
        }
    }

    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        self.last = None;
        core.phase = 1;
        self.sampled = 0.0;
        self.next = 0;
    }

    fn has_number(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

#[must_use]
pub fn mouse_move() -> Box<dyn Behavior> {
    Box::new(MouseMove::default())
}

// ---------------------------------------------------------------------------
// The Dynamite Plunger

/// The plunger's sound (WWSOUND 1006/1048).
const PLUNGER_SOUND: u32 = 1048;

/// The Dynamite Plunger (`DnPl`, io.md 9): a click latches it on until the
/// stop.
#[derive(Debug, Default)]
pub struct DynamitePlunger;

impl Behavior for DynamitePlunger {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame_at_rect(core.sheet, if core.state { 2 } else { 1 }, core.h, core.v);
    }

    fn click(&mut self, core: &mut Core, io: &mut Io<'_>, _x: i32, _y: i32) {
        if !core.state {
            io.play(PLUNGER_SOUND);
            core.state = true;
            io.fire(core, 0);
        }
    }

    fn has_bool(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

#[must_use]
pub fn dynamite_plunger() -> Box<dyn Behavior> {
    Box::new(DynamitePlunger)
}

// ---------------------------------------------------------------------------
// The Heart

/// The heartbeat (WWSOUND 1006/1017).
const HEART_SOUND: u32 = 1017;

/// Beats a minute and heart weight in pounds, per animal (`0x498ed2`).
const HEARTS: [(f64, f64); 8] = [
    (70.0, 0.63),
    (120.0, 0.06),
    (60.0, 0.02),
    (11.0, 0.01),
    (49.0, 44.0),
    (120.0, 0.02),
    (100.0, 0.24),
    (280.0, 0.003),
];

/// The ticks a beat of animal `i` takes: `ftol(3600.0f / bpm)`.
#[must_use]
pub fn heart_period(i: usize) -> u32 {
    let bpm = HEARTS.get(i).map_or(HEARTS[0].0, |h| h.0);
    ext_ftol(Ext::from(3600.0).div(bpm)) as u32
}

/// The Heart (`hart`, io.md 10): beats on the run clock; on for the first
/// quarter of each beat, or gives the pulse rate or the heart's weight.
#[derive(Debug, Default)]
pub struct Heart;

impl Behavior for Heart {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame_at_rect(core.sheet, if core.state { 2 } else { 1 }, core.h, core.v);
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        let i = core.settings.numbers[0] as usize;
        let period = heart_period(i).max(1);
        let ph = io.elapsed() % period;
        // The sound plays on both edges; only the beat's start is an event.
        if !core.state {
            if ph <= period >> 2 {
                io.play(HEART_SOUND);
                core.state = true;
                io.fire(core, 0);
            }
        } else if ph > period >> 2 {
            io.play(HEART_SOUND);
            core.state = false;
        }
        let (bpm, weight) = HEARTS.get(i).copied().unwrap_or(HEARTS[0]);
        match core.settings.radios[0] {
            0 => core.number = bpm,
            1 => core.number = weight,
            _ => {}
        }
    }

    fn init(&mut self, core: &mut Core) {
        core.settings.radios[0] = 2;
    }

    fn has_bool(&self, core: &Core, _knot: usize) -> bool {
        core.settings.radios[0] == 2
    }

    fn has_number(&self, core: &Core, _knot: usize) -> bool {
        core.settings.radios[0] != 2
    }

    any!();
}

#[must_use]
pub fn heart() -> Box<dyn Behavior> {
    Box::new(Heart)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gadgets::common::bench::{Bench, Source};
    use formats::tables::KnotKind;

    fn out() -> Bench {
        let mut b = Bench::with_knots(&[KnotKind::Output]);
        b.wire(0);
        b
    }

    #[test]
    fn switch_fires_only_when_turned_on() {
        let mut b = out();
        let mut s = Switch;
        b.core.settings.radios[0] = 1;
        b.fresh(&mut s);
        assert!(b.core.state);
        b.run(|c, io| s.click(c, io, 0, 0));
        assert!(!b.core.state);
        assert!(!b.fired(0));
        b.run(|c, io| s.click(c, io, 0, 0));
        assert!(b.core.state && b.fired(0));
        b.core.settings.radios[0] = 0;
        s.reset(&mut b.core);
        assert!(!b.core.state);
    }

    #[test]
    fn slider_arrows_step_late_and_fire_on_release() {
        let mut b = out();
        let mut s = Slider::default();
        b.fresh(&mut s);
        b.start(&mut s);
        assert_eq!(b.core.number, 0.0);
        assert!(!b.fired(0));
        // The up arrow, held.
        b.env.mouse_down = true;
        b.run(|c, io| s.click(c, io, 160, 5));
        assert_eq!(s.knob, 1);
        // Ticks 1..7 don't look; 8 updates the value but the button's down.
        b.ticks(&mut s, 1, 1, 8);
        assert_eq!(b.core.number, 1.0);
        assert!(!b.fired(0));
        b.env.mouse_down = false;
        b.tick_at(&mut s, 9);
        assert!(b.fired(0));
        // At the top end it stops.
        s.knob = 100;
        b.run(|c, io| s.click(c, io, 160, 5));
        assert_eq!(s.knob, 100);
        // Custom 10..-10 (swapped), dragging: the knob's middle follows
        // the cursor, clamped to the track.
        b.core.settings.radios[0] = 1;
        b.core.settings.set_label(0, b"10");
        b.core.settings.set_label(1, b"-10");
        b.env.mouse_down = true;
        b.run(|c, io| s.click(c, io, 29 + 100 + 5, 30));
        assert!(s.dragging);
        b.env.mouse = (29 + 9 + 25, 0);
        b.tick_at(&mut s, 20);
        assert_eq!(s.knob, 25);
        assert_eq!(b.core.number, -5.0);
        assert!(b.fired(0));
        b.env.mouse = (-50, 0);
        b.tick_at(&mut s, 21);
        assert_eq!((s.knob, b.core.number), (0, -10.0));
        b.env.mouse_down = false;
        b.tick_at(&mut s, 22);
        assert!(!s.dragging);
        // Reset keeps the knob and the value.
        s.reset(&mut b.core);
        assert_eq!((s.knob, b.core.number), (0, -10.0));
    }

    fn press(b: &mut Bench, k: &mut NumericKeypad, i: usize) {
        let r = KEYPAD_RECTS[i];
        let (x, y) = (i32::from(r.1), i32::from(r.0));
        b.run(|c, io| k.click(c, io, x, y));
    }

    #[test]
    fn keypad_types_and_enters() {
        let mut b = out();
        let mut k = NumericKeypad::default();
        b.fresh(&mut k);
        assert_eq!(k.lcd(), b"         0");
        b.start(&mut k);
        // A zero start isn't an event.
        assert!(!b.fired(0));
        // After ENTER (start acts as one) the first digit starts afresh.
        for i in [1, 2] {
            press(&mut b, &mut k, i);
        }
        press(&mut b, &mut k, 10);
        press(&mut b, &mut k, 5);
        assert_eq!(k.lcd(), b"      12.5");
        press(&mut b, &mut k, 11);
        assert_eq!(k.lcd(), b"     -12.5");
        assert_eq!(b.core.number, 0.0);
        press(&mut b, &mut k, 13);
        assert_eq!(b.core.number, -12.5);
        assert!(b.fired(0));
        // A second '.' is rejected with a beep (no sound loaded: no voice).
        press(&mut b, &mut k, 3);
        assert_eq!(k.lcd(), b"         3");
        press(&mut b, &mut k, 10);
        press(&mut b, &mut k, 10);
        assert_eq!(k.lcd(), b"        3.");
        // C clears, keeping the value; a lone 0 is replaced.
        press(&mut b, &mut k, 12);
        assert_eq!(k.lcd(), b"         0");
        press(&mut b, &mut k, 0);
        press(&mut b, &mut k, 7);
        assert_eq!(k.lcd(), b"         7");
        assert_eq!(b.core.number, -12.5);
        // Full: ten characters.
        for _ in 0..12 {
            press(&mut b, &mut k, 9);
        }
        assert_eq!(k.lcd(), b"7999999999");
        // Lock Number.
        b.core.settings.checks = 1;
        press(&mut b, &mut k, 12);
        assert_eq!(k.lcd(), b"7999999999");
        // Auto Reset reloads text 0 on the stop.
        b.core.settings.checks = 4;
        b.core.settings.set_label(0, b"-0.5");
        k.reset(&mut b.core);
        assert_eq!(k.lcd(), b"      -0.5");
        assert_eq!(b.core.number, 0.0);
    }

    #[test]
    fn keypad_numbers() {
        assert!(keypad_valid(b"   -1.5e-12"));
        assert!(keypad_valid(b"        1."));
        assert!(!keypad_valid(b"       1e"));
        assert!(!keypad_valid(b"      1e123"));
        assert!(!keypad_valid(b"       1.2.3"));
        assert!(!keypad_valid(b"      --1"));
        assert!(!keypad_valid(b"       1!"));
    }

    #[test]
    fn keypress_holds() {
        let mut b = out();
        let mut k = Keypress::default();
        b.fresh(&mut k);
        b.core.settings.text[0] = b'g';
        b.start(&mut k);
        // A tap between two ticks isn't seen.
        b.tick_at(&mut k, 0);
        assert!(!b.core.state);
        b.env.keys_held = vec![u16::from(b'G')];
        b.tick_at(&mut k, 1);
        assert!(b.core.state && b.fired(0));
        // Held: stays on, no more events.
        b.tick_at(&mut k, 2);
        assert!(b.core.state && !b.fired(0));
        // Let go: off, not an event.
        b.env.keys_held.clear();
        b.tick_at(&mut k, 3);
        assert!(!b.core.state && !b.fired(0));
        // A click: on until the button (anywhere, either) is let go; the
        // key doesn't matter then.
        b.env.right_down = true;
        b.run(|c, io| k.click(c, io, 0, 0));
        assert!(b.core.state && b.fired(0));
        b.tick_at(&mut k, 4);
        assert!(b.core.state);
        b.env.right_down = false;
        b.env.keys_held = vec![u16::from(b'G')];
        b.tick_at(&mut k, 5);
        assert!(!b.core.state);
        // ...and the key, still held, turns it on again next tick.
        b.tick_at(&mut k, 6);
        assert!(b.core.state && b.fired(0));
    }

    #[test]
    fn mouse_button_waits_for_a_release() {
        let mut b = out();
        let mut m = MouseClick::default();
        b.fresh(&mut m);
        b.start(&mut m);
        b.env.mouse_down = true;
        b.tick_at(&mut m, 0);
        assert!(!b.core.state);
        b.env.mouse_down = false;
        b.tick_at(&mut m, 1);
        b.env.right_down = true;
        b.tick_at(&mut m, 2);
        assert!(b.core.state && b.fired(0));
        b.env.right_down = false;
        b.tick_at(&mut m, 3);
        assert!(!b.core.state && !b.fired(0));
    }

    #[test]
    fn mouse_distance_is_in_72nds_of_an_inch() {
        let mut b = out();
        let mut m = MouseMove::default();
        b.fresh(&mut m);
        b.start(&mut m);
        b.env.cursor_screen = (100, 100);
        b.tick_at(&mut m, 0);
        assert_eq!(b.core.number, 0.0);
        // Moved without a button: not counted.
        b.env.cursor_screen = (500, 100);
        b.tick_at(&mut m, 1);
        assert_eq!(b.core.number, 0.0);
        assert!(!b.fired(0));
        b.env.mouse_down = true;
        b.env.cursor_screen = (500 + 72, 100);
        b.tick_at(&mut m, 2);
        assert_eq!(b.core.number, 1.0);
        assert!(b.fired(0));
        // 3-4-5: 36 · 2 down, 27 · 2 across = 90 / 72.
        b.env.cursor_screen = (500 + 72 + 54, 100 + 72);
        b.tick_at(&mut m, 3);
        assert!((b.core.number - 2.25).abs() < 1e-12);
        // Sampled at most every 6 ticks: 2.25 inches in 6 ticks is fast,
        // the third picture.
        assert_eq!(b.core.phase, 1);
        b.tick_at(&mut m, 7);
        assert_eq!(b.core.phase, 3);
        // Stop clears it.
        m.reset(&mut b.core);
        assert_eq!((b.core.number, b.core.phase), (0.0, 1));
    }

    #[test]
    fn heart_beats() {
        let periods: Vec<u32> = (0..8).map(heart_period).collect();
        assert_eq!(periods, [51, 30, 60, 327, 73, 30, 36, 12]);
        let mut b = out();
        let mut h = Heart;
        b.fresh(&mut h);
        b.core.settings.numbers[0] = 7;
        b.start(&mut h);
        b.tick_at(&mut h, 0);
        assert!(b.core.state && b.fired(0));
        b.tick_at(&mut h, 3);
        assert!(b.core.state);
        b.tick_at(&mut h, 4);
        assert!(!b.core.state && !b.fired(0));
        b.tick_at(&mut h, 12);
        assert!(b.core.state);
        assert!(h.has_bool(&b.core, 0) && !h.has_number(&b.core, 0));
        b.core.settings.radios[0] = 1;
        b.tick_at(&mut h, 13);
        assert_eq!(b.out(&h, 0), crate::gadget::Value::Number(0.003));
        let _ = Source::default();
    }
}
