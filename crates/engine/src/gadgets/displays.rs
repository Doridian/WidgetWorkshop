//! The parts that show what they're given: the Light Bulb (`bulb`), the
//! Three-Digit and Ten-Digit Displays (`disp`, `di10`), the Color Mixer
//! (`rgbd`), the Strip Chart (`StCh`), the Graphic Projector (`GrPr`), and
//! the stickers (`Stk1`..`Stk9`, `St10`). docs/gadgets/io.md has the
//! original's code.

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

use super::common::{any, format_number, scan_f64, src_bool, trunc, trunc_sat, truthy};
use super::inputs::{draw_broken, ext_floor, ext_ftol, point_or, rects};
use super::x87::Ext;
use crate::assets::Module;
use crate::gadget::{Behavior, Core, DrawCx};
use crate::gfx::IRect;
use crate::sim::Io;

/// The digit sheet the displays share: frames 1..10 are `0`..`9`, 11 `-`,
/// 12 `+`, 13 `.`, 14 `E`.
const DIGITS: u32 = 100;

// ---------------------------------------------------------------------------
// The Light Bulb

/// The Light Bulb (`bulb`, io.md 11): lit while its input is non-zero.
/// No outputs; its state is kept in `+0x13e` all the same.
#[derive(Debug, Default)]
pub struct LightBulb;

impl Behavior for LightBulb {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame_at_rect(core.sheet, if core.state { 2 } else { 1 }, core.h, core.v);
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        core.state = truthy(io.number(core, 0));
    }

    any!();
}

#[must_use]
pub fn light_bulb() -> Box<dyn Behavior> {
    Box::new(LightBulb)
}

// ---------------------------------------------------------------------------
// The Three-Digit Display

/// The digit cells' places (1010/1013), hundreds to ones.
const DISPLAY3_POINTS: [(i32, i32); 3] = [(26, 9), (48, 9), (70, 9)];

/// What the Three-Digit Display shows for `n`: its three cells' frames of
/// the digit sheet, blank cells `None`, or `None` overall for "broken"
/// (outside -99..999). No leading zeros.
#[must_use]
pub fn display3_cells(n: i32) -> Option<[Option<u16>; 3]> {
    if !(n < 1000 && n > -100) {
        return None;
    }
    let d = |x: i32| Some((x % 10 + 1) as u16);
    Some(if n < 0 {
        let a = -n;
        [Some(11), if a > 9 { d(a / 10) } else { None }, d(a)]
    } else {
        [
            if n > 99 { d(n / 100) } else { None },
            if n > 9 { d(n / 10) } else { None },
            d(n),
        ]
    })
}

/// The Three-Digit Display (`disp`, cDisplay, io.md 12.1): its input
/// rounded half up, -99 to 999, while running.
#[derive(Debug, Default)]
pub struct Display3 {
    /// The integer shown (`+0x142`, cleared by reset).
    shown: i32,
}

impl Behavior for Display3 {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame_at_rect(core.sheet, 1, core.h, core.v);
        if !cx.running {
            return;
        }
        let Some(cells) = display3_cells(self.shown) else {
            draw_broken(core, cx);
            return;
        };
        for (i, f) in cells.into_iter().enumerate() {
            if let Some(f) = f {
                let (h, v) = point_or(cx.assets, core.sheet, i, DISPLAY3_POINTS[i]);
                cx.frame_at_rect(DIGITS, f, core.h + h, core.v + v);
            }
        }
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        if io.value_changed(core, 0) {
            // 0.5 is a float; `floor` rounds down (RC=down).
            self.shown = trunc_sat(ext_floor(Ext::from(io.number(core, 0)).add(0.5)));
        }
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        // `+0x13d`: the first look after GO always sees a wired input.
        core.empty_cache = true;
        io.base_start(core);
    }

    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        self.shown = 0;
    }

    any!();
}

#[must_use]
pub fn display3() -> Box<dyn Behavior> {
    Box::new(Display3::default())
}

// ---------------------------------------------------------------------------
// The Ten-Digit Display

/// The ten cells' places (1010/1014): 22 pixels apart but 21 between the
/// sixth and seventh.
const DISPLAY10_POINTS: [(i32, i32); 10] = [
    (26, 9),
    (48, 9),
    (70, 9),
    (92, 9),
    (114, 9),
    (136, 9),
    (157, 9),
    (179, 9),
    (201, 9),
    (223, 9),
];

/// The digit sheet's frame for each of `s`'s first ten characters (`None`
/// for a blank), or `None` overall for "broken".
#[must_use]
pub fn display10_cells(s: &str) -> Option<Vec<Option<u16>>> {
    s.bytes()
        .take(10)
        .map(|c| match c {
            b'.' => Some(Some(13)),
            b' ' => Some(None),
            b'+' => Some(Some(12)),
            b'-' => Some(Some(11)),
            b'E' | b'e' => Some(Some(14)),
            b'0'..=b'9' => Some(Some(u16::from(c - b'/'))),
            _ => None,
        })
        .collect()
}

/// The Ten-Digit Display (`di10`, cDisplay10, io.md 12.2): its input as
/// [`format_number`] writes it, cut to ten characters, while running.
#[derive(Debug, Default)]
pub struct Display10 {
    /// Sticky decimals (`+0x196`, cleared by reset); the original updates
    /// it as it draws.
    decimals: Cell<u8>,
}

impl Behavior for Display10 {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame_at_rect(core.sheet, 1, core.h, core.v);
        if !cx.running {
            return;
        }
        let mut d = self.decimals.get();
        let s = format_number(core.number, &mut d);
        self.decimals.set(d);
        let Some(cells) = display10_cells(&s) else {
            draw_broken(core, cx);
            return;
        };
        for (i, f) in cells.into_iter().enumerate() {
            if let Some(f) = f {
                let (h, v) = point_or(cx.assets, core.sheet, i, DISPLAY10_POINTS[i]);
                cx.frame_at_rect(DIGITS, f, core.h + h, core.v + v);
            }
        }
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        if io.value_changed(core, 0) {
            core.number = io.number(core, 0);
        }
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        core.empty_cache = true;
        io.base_start(core);
    }

    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        self.decimals.set(0);
    }

    any!();
}

#[must_use]
pub fn display10() -> Box<dyn Behavior> {
    Box::new(Display10::default())
}

// ---------------------------------------------------------------------------
// The Color Mixer and CDitheredBrush

/// `0x439f8a`: the 8×8 pattern (palette indexes, row-major, top row
/// first) `CDitheredBrush` (`0x43a226`) paints `rgb` with: the nearest
/// colour first, then greedily the nearest colour of each octant around
/// `rgb` that keeps the running mean closest to it.
#[must_use]
pub fn dither(palette: &[[u8; 3]; 256], rgb: [u8; 3]) -> [u8; 64] {
    let t = rgb.map(i32::from);
    let pal = |i: usize| palette[i].map(i32::from);
    // `0x439e88`.
    let dist2 = |p: [i32; 3]| -> i32 { (0..3).map(|c| (p[c] - t[c]) * (p[c] - t[c])).sum() };
    let mut best = [(0usize, 1_000_000i32); 8];
    let mut overall = (0usize, 1_000_000i32);
    for i in 0..256 {
        let p = pal(i);
        // `0x439f58`: the octant around the target.
        let o = usize::from(p[0] > t[0]) | usize::from(p[1] > t[1]) << 1 | usize::from(p[2] > t[2]) << 2;
        let d = dist2(p);
        if d < best[o].1 {
            best[o] = (i, d);
        }
        if d < overall.1 {
            overall = (i, d);
        }
    }
    let mut out = [0u8; 64];
    out[0] = overall.0 as u8;
    let mut acc = pal(overall.0);
    for k in 1..64i32 {
        let m = k + 1;
        let mut best_e = 1_000_000u32;
        let mut pick = 0;
        let mut next = acc;
        for (j, &(idx, _)) in best.iter().enumerate() {
            let p = pal(idx);
            let c = [acc[0] + p[0], acc[1] + p[1], acc[2] + p[2]];
            // `0x439ee3`: the squared distance of the mean from the target.
            let e = (0..3).map(|i| (c[i] - t[i] * m).pow(2)).sum::<i32>() as u32 / (m * m) as u32;
            if e < best_e {
                best_e = e;
                pick = j;
                next = c;
            }
        }
        acc = next;
        out[k as usize] = best[pick].0 as u8;
    }
    out
}

/// `0x451064`'s scale: a percentage, kept to 0..100, to 0..65535 (`655.35`,
/// a double).
fn channel_level(v: f64) -> u16 {
    let v = v.clamp(0.0, 100.0);
    trunc(655.35 * v) as u16
}

/// The Color Mixer (`rgbd`, cRGBDisplay, io.md 13): red, green and blue
/// percentages, shown dithered with the game's palette while running.
#[derive(Debug, Default)]
pub struct RgbDisplay {
    /// The colour as a `COLORREF`, `0x00BBGGRR` (`+0x197`).
    color: u32,
}

impl RgbDisplay {
    /// `0x451064`: channel `k`, 0..65535: a number source's percentage, or a
    /// boolean source's 0 or all.
    fn channel(core: &Core, io: &Io<'_>, k: usize) -> u16 {
        let Some((g, gk)) = io.source(core, k) else {
            return 0;
        };
        if g.behavior.has_number(&g.core, gk) {
            // A NaN stays a NaN; the cast makes it 0.
            channel_level(io.number(core, k))
        } else if src_bool(io, core, k) {
            0xffff
        } else {
            0
        }
    }

    /// The colour as `[r, g, b]`.
    #[must_use]
    pub fn rgb(&self) -> [u8; 3] {
        [self.color as u8, (self.color >> 8) as u8, (self.color >> 16) as u8]
    }
}

impl Behavior for RgbDisplay {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame_at_rect(core.sheet, 1, core.h, core.v);
        if !cx.running {
            return;
        }
        let o = core.rect();
        let fill = IRect::new(o.x0 + 17, o.y0 + 6, o.x1 - 4, o.y1 - 5);
        let pattern = dither(&cx.assets.palette(), self.rgb());
        // A pattern brush lines up with the device's origin; the port
        // takes the worksheet's.
        for y in fill.y0..fill.y1 {
            for x in fill.x0..fill.x1 {
                let c = pattern[(y.rem_euclid(8) * 8 + x.rem_euclid(8)) as usize];
                cx.surface.pset(x, y, c);
            }
        }
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        // Short-circuit: when input 0 changed, 1 and 2 aren't re-cached.
        if io.value_changed(core, 0) || io.value_changed(core, 1) || io.value_changed(core, 2) {
            let r = u32::from(Self::channel(core, io, 0) >> 8);
            let g = u32::from(Self::channel(core, io, 1) >> 8);
            let b = u32::from(Self::channel(core, io, 2) >> 8);
            self.color = r | g << 8 | b << 16;
        }
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        core.empty_cache = true;
        io.base_start(core);
    }

    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        self.color = 0;
    }

    any!();
}

#[must_use]
pub fn rgb_display() -> Box<dyn Behavior> {
    Box::new(RgbDisplay::default())
}

// ---------------------------------------------------------------------------
// The Strip Chart

/// The chart's columns.
pub const CHART_COLUMNS: usize = 120;
/// Its rows: values from From to To map to 0..80.
const CHART_ROWS: f64 = 80.0;
/// The preset sweep lengths in ticks (`0x49bd5c`): 10 seconds, 1 minute,
/// 10 minutes, 1 hour.
const CHART_LENGTHS: [u32; 4] = [600, 3600, 36000, 216_000];
/// The plot's origin, bottom left (1010/1049 `P0`), `(h, v)`.
const CHART_ORIGIN: (i32, i32) = (24, 96);

/// The Strip Chart (`StCh`, io.md 17): its input over one sweep of 120
/// columns, each the range of the values seen in its time; it stops when
/// full, until RESET (input 1) or the next GO.
#[derive(Debug)]
pub struct StripChart {
    /// The range (`+0x197`, `+0x19f`).
    lo: f64,
    hi: f64,
    /// The custom length in hours last read (the original's stack slot,
    /// read uninitialised when the text doesn't parse).
    hours: f64,
    /// The sweep's length in ticks (`+0x1a7`).
    len: u32,
    /// The last column written, -1 for none (`+0x1ab`).
    last_col: i32,
    /// The row of the value that began it, for the pen marker (`+0x1af`).
    marker: i32,
    /// Each column's lowest and highest row (`+0x1b3`); empty is
    /// `(127, -128)`.
    cols: [(i8, i8); CHART_COLUMNS],
    /// When the sweep began (`+0x2a5`).
    began: u32,
}

impl Default for StripChart {
    fn default() -> StripChart {
        StripChart {
            lo: 0.0,
            hi: 0.0,
            hours: 0.0,
            len: 60,
            last_col: -1,
            marker: -1,
            cols: [(127, -128); CHART_COLUMNS],
            began: 0,
        }
    }
}

impl StripChart {
    /// `0x459297`.
    fn clear(&mut self, now: u32) {
        self.cols = [(127, -128); CHART_COLUMNS];
        self.marker = -1;
        self.last_col = -1;
        self.began = now;
    }

    /// The columns so far: each one's lowest and highest row.
    #[must_use]
    pub fn columns(&self) -> &[(i8, i8)] {
        let n = usize::try_from(self.last_col + 1).unwrap_or(0).min(CHART_COLUMNS);
        &self.cols[..n]
    }

    /// The sweep's length in ticks.
    #[must_use]
    pub fn sweep(&self) -> u32 {
        self.len
    }
}

impl Behavior for StripChart {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        let (ph, pv) = point_or(cx.assets, core.sheet, 0, CHART_ORIGIN);
        let (oh, ov) = (core.h + ph, core.v + pv);
        cx.frame_at_rect(core.sheet, 1, core.h, core.v);
        if cx.running {
            let black = cx.color([0, 0, 0]);
            for (c, &(min, max)) in self.columns().iter().enumerate() {
                if min < 80 && max >= 0 {
                    let y1 = ov - i32::from(min.max(0));
                    let y2 = ov - i32::from(max.min(79));
                    // `MoveTo(y1)`, `LineTo(y2)`: the end isn't drawn.
                    let (top, bottom) = if y1 == y2 { (y1, y1 + 1) } else { (y2 + 1, y1 + 1) };
                    let x = oh + c as i32;
                    cx.surface.fill(IRect::new(x, top, x + 1, bottom), black);
                }
            }
        }
        // The pen, in edit mode too (80 itself isn't clamped).
        let mut m = self.marker;
        if m > 80 {
            m = 79;
        }
        if m < 0 {
            m = 0;
        }
        cx.frame_at_rect(core.sheet, 3, oh + self.last_col, ov - m);
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        let s = &core.settings;
        // A text that doesn't read keeps the old value.
        if let Some(lo) = scan_f64(s.label(0)) {
            self.lo = lo;
        }
        if let Some(hi) = scan_f64(s.label(1)) {
            self.hi = hi;
        }
        self.len = if s.radios[0] != 0 {
            if let Some(h) = scan_f64(s.label(2)) {
                self.hours = h;
            }
            ext_ftol(Ext::from(216_000.0).mul(self.hours)) as u32
        } else {
            // Past the table reads garbage in the original.
            CHART_LENGTHS
                .get(s.numbers[0] as usize)
                .copied()
                .unwrap_or(CHART_LENGTHS[1])
        };
        if self.lo == self.hi {
            self.lo = f64::from_bits(0x81b5_6e1f_c2f8_f359);
            self.hi = f64::from_bits(0x01a5_6e1f_c2f8_f359);
        }
        if self.lo > self.hi {
            std::mem::swap(&mut self.lo, &mut self.hi);
        }
        if self.len < 60 {
            self.len = 60;
        }
        self.clear(io.elapsed());
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        let now = io.elapsed();
        // RESET held: cleared every tick.
        if truthy(io.number(core, 1)) {
            self.clear(now);
            return;
        }
        let v = io.number(core, 0);
        let col = now.wrapping_sub(self.began).wrapping_mul(120) / self.len.max(1);
        let y = ext_ftol(
            Ext::from(v)
                .sub(self.lo)
                .div(Ext::from(self.hi).sub(self.lo))
                .mul(CHART_ROWS),
        );
        // Full: it stops; there's no scrolling.
        if col as i32 >= 0 && (col as usize) < CHART_COLUMNS {
            let col = col as usize;
            let c = &mut self.cols[col];
            if i32::from(c.0) > y {
                c.0 = y.max(-128) as i8;
            }
            if i32::from(c.1) < y {
                c.1 = y.min(127) as i8;
            }
            if col > 0 {
                let p = self.cols[col - 1];
                let c = &mut self.cols[col];
                // Joined to the previous column where it shows.
                if p.0 < 80 && p.1 >= 0 {
                    if p.1 < c.0 {
                        c.0 = p.1;
                    } else if p.0 > c.1 {
                        c.1 = p.0;
                    }
                }
            }
            if col as i32 != self.last_col {
                self.last_col = col as i32;
                // The marker's row is only taken at a new column.
                self.marker = y;
            }
        }
    }

    fn init(&mut self, core: &mut Core) {
        let s = &mut core.settings;
        s.set_label(0, b"0");
        s.set_label(1, b"100");
        s.set_label(2, b"1");
        s.numbers[0] = 1;
    }

    any!();
}

#[must_use]
pub fn strip_chart() -> Box<dyn Behavior> {
    Box::new(StripChart::default())
}

// ---------------------------------------------------------------------------
// The Graphic Projector

/// Each picture set's frame count (`0x499b0e`); set `n` is sheet `200 + n`.
const PICTURE_COUNTS: [u8; 6] = [8, 5, 12, 8, 10, 7];
/// The screen area (1012/1050 `R0`), when the resource is missing.
const SCREEN: Rect = Rect {
    top: 11,
    left: 23,
    bottom: 82,
    right: 119,
};

/// The Graphic Projector (`GrPr`, io.md 18): picture `n` of a set, for
/// input `n`.
#[derive(Debug, Default)]
pub struct GraphicProjector {
    /// The picture shown, from 1; 0 for none (`+0x142`).
    picture: i32,
    /// The set's frame count, as of the last start (`+0x197`).
    count: u8,
    /// The sets loaded by a start: the original keeps them in a global
    /// cache (`0x4a3200`) every projector shares, until the last one goes;
    /// the port keeps this per projector.
    loaded: [bool; 6],
}

impl GraphicProjector {
    /// The picture shown (0 for none).
    #[must_use]
    pub fn picture(&self) -> i32 {
        self.picture
    }
}

/// The centre of `r` as (`h`, `v`), C division.
fn centre(r: Rect) -> (i32, i32) {
    let (t, l, b, rr) = (
        i32::from(r.top),
        i32::from(r.left),
        i32::from(r.bottom),
        i32::from(r.right),
    );
    (l + (rr - l) / 2, t + (b - t) / 2)
}

impl Behavior for GraphicProjector {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        let set = core.settings.numbers[0];
        let body = match set {
            0..4 => 1,
            4 => 3,
            5 => 2,
            _ => 0,
        };
        if body != 0 {
            cx.frame_at_rect(core.sheet, body, core.h, core.v);
        }
        if !cx.running {
            return;
        }
        if !self.loaded.get(set as usize).copied().unwrap_or(false) {
            draw_broken(core, cx);
            return;
        }
        let sheet = 200 + set;
        let Some(f1) = cx.pictures.rect(cx.assets, Module::Gadget, sheet, 1) else {
            draw_broken(core, cx);
            return;
        };
        let screen = rects(cx.assets, core.sheet).first().copied().unwrap_or(SCREEN);
        let (sh, sv) = centre(screen);
        let (fh, fv) = centre(f1);
        // Every frame is placed by frame 1's centre.
        if self.picture > 0 && self.picture <= i32::from(self.count) {
            cx.frame_at_rect(sheet, self.picture as u16, core.h + sh - fh, core.v + sv - fv);
        }
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        let mut n = trunc_sat(io.number(core, 0).floor());
        // Wrap Around Set.
        if core.settings.radios[0] != 0 && n > 0 {
            n = (n - 1) % i32::from(self.count.max(1)) + 1;
        }
        self.picture = n;
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        let set = core.settings.numbers[0] as usize;
        self.count = PICTURE_COUNTS.get(set).copied().unwrap_or(0);
        if let Some(l) = self.loaded.get_mut(set) {
            *l = true;
        }
    }

    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        self.picture = 0;
    }

    any!();
}

#[must_use]
pub fn graphic_projector() -> Box<dyn Behavior> {
    Box::new(GraphicProjector::default())
}

// ---------------------------------------------------------------------------
// The stickers

/// A sticker (`Stk1`..`Stk9`, `St10`, io.md 19): frame 1 of its sheet,
/// placed by its rect (some are 1 or 2 pixels right of the gadget's top
/// left). Nothing else.
#[derive(Debug, Default)]
pub struct Sticker;

impl Behavior for Sticker {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame_at_rect(core.sheet, 1, core.h, core.v);
    }

    any!();
}

#[must_use]
pub fn sticker() -> Box<dyn Behavior> {
    Box::new(Sticker)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gadgets::common::bench::{Bench, Source};
    use formats::tables::KnotKind;

    #[test]
    fn three_digits_round_half_up_without_leading_zeros() {
        let mut b = Bench::with_knots(&[KnotKind::Input]);
        let mut d = Display3::default();
        b.fresh(&mut d);
        b.source(0, Source::number(2.5));
        b.start(&mut d);
        b.tick_at(&mut d, 0);
        assert_eq!(d.shown, 3);
        for (x, n) in [(-2.5, -2), (-2.51, -3), (998.7, 999), (0.499_999_999_999_999_94, 0)] {
            b.source(0, Source::number(x));
            b.tick_at(&mut d, 1);
            assert_eq!(d.shown, n, "{x}");
        }
        assert_eq!(display3_cells(7), Some([None, None, Some(8)]));
        assert_eq!(display3_cells(105), Some([Some(2), Some(1), Some(6)]));
        assert_eq!(display3_cells(-5), Some([Some(11), None, Some(6)]));
        assert_eq!(display3_cells(-42), Some([Some(11), Some(5), Some(3)]));
        assert_eq!(display3_cells(1000), None);
        assert_eq!(display3_cells(-100), None);
    }

    #[test]
    fn ten_digits_keep_their_decimals_and_cut_at_ten() {
        let mut sticky = 0;
        let s = format_number(1.125, &mut sticky);
        assert_eq!(s, "     1.125");
        let cells = display10_cells(&s).unwrap();
        assert_eq!(cells[5..], [Some(2), Some(13), Some(2), Some(3), Some(6)]);
        // Sticky: 2 shows with three decimals until reset.
        assert_eq!(format_number(2.0, &mut sticky), "     2.000");
        // Wider than ten: cut on the right.
        let s = format_number(1_234_567.5, &mut sticky);
        assert_eq!(display10_cells(&s).unwrap().len(), 10);
        assert_eq!(&s[..10], "1234567.50");
        assert_eq!(display10_cells("+NAN"), None);
        // A wired input is read on the first tick after GO even if it
        // didn't change (`+0x13d`).
        let mut b = Bench::with_knots(&[KnotKind::Input]);
        let mut d = Display10::default();
        b.fresh(&mut d);
        b.source(0, Source::number(4.0));
        b.start(&mut d);
        b.tick_at(&mut d, 0);
        assert_eq!(b.core.number, 4.0);
    }

    #[test]
    fn color_mixer_scales_percentages() {
        assert_eq!(channel_level(100.0) >> 8, 255);
        assert_eq!(channel_level(50.0) >> 8, 127);
        assert_eq!(channel_level(-3.0), 0);
        assert_eq!(channel_level(f64::NAN), 0);
        let mut b = Bench::with_knots(&[KnotKind::Input, KnotKind::Input, KnotKind::Input]);
        let mut d = RgbDisplay::default();
        b.fresh(&mut d);
        b.source(0, Source::number(100.0));
        b.source(1, Source::boolean(true));
        b.source(2, Source::number(150.0));
        b.start(&mut d);
        b.tick_at(&mut d, 0);
        // Input 0 changed: 1 and 2 aren't looked at for a change, but the
        // colour reads all three.
        assert_eq!(d.rgb(), [255, 255, 255]);
    }

    #[test]
    fn dither_of_a_palette_colour_is_that_colour() {
        let mut pal = [[0u8; 3]; 256];
        for (i, p) in pal.iter_mut().enumerate() {
            *p = [i as u8, (255 - i) as u8, (i / 2) as u8];
        }
        assert!(dither(&pal, pal[77]).iter().all(|&c| c == 77));
        // Halfway between two greys: a mix of the two.
        let mut grey = [[0u8; 3]; 256];
        grey[1] = [100, 100, 100];
        grey[2] = [200, 200, 200];
        let p = dither(&grey, [150, 150, 150]);
        let ones = p.iter().filter(|&&c| c == 1).map(|_| 1).sum::<usize>();
        assert_eq!(ones, 32);
        assert!(p.iter().all(|&c| c == 1 || c == 2));
    }

    fn chart() -> (Bench, StripChart) {
        let mut b = Bench::with_knots(&[KnotKind::Input, KnotKind::Input]);
        let mut c = StripChart::default();
        b.fresh(&mut c);
        // 0..80 over 10 seconds: a row per unit, a column per 5 ticks.
        b.core.settings.set_label(1, b"80");
        b.core.settings.numbers[0] = 0;
        (b, c)
    }

    #[test]
    fn strip_chart_fills_then_freezes() {
        let (mut b, mut c) = chart();
        b.source(0, Source::number(10.9));
        b.start(&mut c);
        assert_eq!(c.sweep(), 600);
        b.tick_at(&mut c, 0);
        // Truncated, not rounded.
        assert_eq!(c.columns(), [(10, 10)]);
        assert_eq!(c.marker, 10);
        b.source(0, Source::number(-3.5));
        b.tick_at(&mut c, 4);
        assert_eq!(c.columns(), [(-3, 10)]);
        // A new column joins the last one; the marker takes its first value.
        b.source(0, Source::number(20.0));
        b.tick_at(&mut c, 5);
        assert_eq!(c.columns(), [(-3, 10), (10, 20)]);
        assert_eq!(c.marker, 20);
        // Out of range clamps to the i8s.
        b.source(0, Source::number(1e6));
        b.tick_at(&mut c, 10);
        assert_eq!(c.columns()[2], (20, 127));
        // Full after 600 ticks: nothing more, no scrolling.
        b.source(0, Source::number(5.0));
        b.tick_at(&mut c, 599);
        assert_eq!(c.columns().len(), 120);
        let full = c.cols;
        b.tick_at(&mut c, 600);
        b.tick_at(&mut c, 5000);
        assert_eq!(c.cols, full);
        assert_eq!(c.last_col, 119);
        // RESET clears and restarts the sweep there.
        b.source(1, Source::boolean(true));
        b.tick_at(&mut c, 6000);
        assert_eq!(c.columns(), []);
        b.source(1, Source::boolean(false));
        b.tick_at(&mut c, 6001);
        assert_eq!(c.columns(), [(5, 5)]);
    }

    #[test]
    fn strip_chart_range_and_length() {
        let (mut b, mut c) = chart();
        b.core.settings.set_label(0, b"1.2");
        b.core.settings.set_label(1, b"-1.2");
        b.core.settings.radios[0] = 1;
        b.core.settings.set_label(2, b".0001");
        b.start(&mut c);
        assert_eq!((c.lo, c.hi), (-1.2, 1.2));
        // 21.6 ticks: at least 60.
        assert_eq!(c.sweep(), 60);
        b.core.settings.set_label(1, b"1.2");
        b.start(&mut c);
        assert!(c.lo < 0.0 && c.hi > 0.0 && c.hi < 1e-299);
    }

    #[test]
    fn projector_wraps() {
        let mut b = Bench::with_knots(&[KnotKind::Input]);
        let mut p = GraphicProjector::default();
        b.fresh(&mut p);
        b.core.settings.numbers[0] = 1;
        b.source(0, Source::number(7.9));
        b.start(&mut p);
        b.tick_at(&mut p, 0);
        assert_eq!(p.picture(), 7);
        b.core.settings.radios[0] = 1;
        b.tick_at(&mut p, 1);
        assert_eq!(p.picture(), 2);
        b.source(0, Source::number(-1.0));
        b.tick_at(&mut p, 2);
        assert_eq!(p.picture(), -1);
    }
}
