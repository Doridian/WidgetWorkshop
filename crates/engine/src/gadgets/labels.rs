//! The text parts: the Message List (`TxLn`), the Billboard (`TxBx`) and
//! the Label (`TxLb`), and the gadgets' text drawing (`DrawText` in the
//! fonts `0x41783b` makes). docs/gadgets/io.md 1.3, 14 to 16.

#![allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]

use formats::fnt::Font;
use formats::hr::Rect;

use super::common::{any, trunc, truthy};
use crate::assets::Assets;
use crate::gadget::{Behavior, Core, DrawCx, Settings};
use crate::gfx::IRect;
use crate::sim::Io;

// ---------------------------------------------------------------------------
// Text drawing

/// The gadgets' fonts (`0x41783b`): `CreateFont` with a cell height in
/// pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GadgetFont {
    /// "Helvetica" 16 (`0x417931`).
    Helv16,
    /// "Helvetica" 16, weight 700 (`0x41794d`).
    Helv16Bold,
    /// "Helvetica" 24 (`0x417969`).
    Helv24,
}

/// "Helvetica" isn't a Windows font: GDI substitutes Arial, which is
/// TrueType. The port draws bitmap fonts only, so it takes the first of
/// these it has (an override can add one; the game's own fonts have MS
/// Sans Serif).
const FACES: [&str; 3] = ["Helvetica", "Arial", "MS Sans Serif"];

/// The font for `f`, and whether its bold must be made up (as GDI does for
/// a raster font: each glyph drawn twice, a pixel apart, one pixel wider).
fn pick(assets: &Assets, f: GadgetFont) -> Option<(&Font, bool)> {
    let (height, bold) = match f {
        GadgetFont::Helv16 => (16, false),
        GadgetFont::Helv16Bold => (16, true),
        GadgetFont::Helv24 => (24, false),
    };
    let font = FACES.iter().find_map(|face| assets.font(face, height, bold))?;
    Some((font, bold && font.weight < 600))
}

/// `text`'s extent in `f` (`GetTextExtentPoint`): width and cell height.
#[must_use]
pub fn text_extent(assets: &Assets, f: GadgetFont, text: &[u8]) -> (i32, i32) {
    match pick(assets, f) {
        Some((font, fake)) => (line_width(font, fake, text), i32::from(font.height)),
        None => (0, 0),
    }
}

fn line_width(font: &Font, fake_bold: bool, text: &[u8]) -> i32 {
    let w = i32::try_from(font.width(text)).unwrap_or(0);
    if fake_bold {
        w + text.iter().filter(|&&c| font.glyph(c).is_some()).count() as i32
    } else {
        w
    }
}

fn draw_line(cx: &mut DrawCx<'_>, font: &Font, fake_bold: bool, x: i32, y: i32, text: &[u8], c: u8) {
    if !fake_bold {
        cx.surface.text(font, x, y, text, c);
        return;
    }
    let mut at = x;
    for ch in text.chunks(1) {
        if font.glyph(ch[0]).is_none() {
            continue;
        }
        let w = cx.surface.text(font, at, y, ch, c);
        cx.surface.text(font, at + 1, y, ch, c);
        at += w + 1;
    }
}

/// The `DrawText` formats the gadgets use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// `0x825`: `DT_CENTER|DT_VCENTER|DT_SINGLELINE|DT_NOCLIP`.
    Line,
    /// `0x811`: `DT_CENTER|DT_WORDBREAK|DT_NOCLIP`.
    Wrapped,
}

/// `text`'s lines as `DT_WORDBREAK` breaks them to fit `width`: at line
/// breaks (CR LF, CR or LF), and before a word that would cross the right
/// edge (a word too long for a line overflows it). Spaces where a line
/// was broken are dropped.
#[must_use]
pub fn wrap(text: &[u8], width: i32, measure: impl Fn(&[u8]) -> i32) -> Vec<Vec<u8>> {
    let mut lines = Vec::new();
    let mut rest = text;
    loop {
        let stop = rest
            .iter()
            .position(|&c| c == b'\r' || c == b'\n')
            .unwrap_or(rest.len());
        let para = &rest[..stop];
        // The line is `para[start..end]`; `i` is where the next word's
        // spaces begin.
        let (mut start, mut end, mut i) = (0, 0, 0);
        while i < para.len() {
            while i < para.len() && para[i] == b' ' {
                i += 1;
            }
            let word = i;
            while i < para.len() && para[i] != b' ' {
                i += 1;
            }
            if word == i {
                break;
            }
            if end > start && measure(&para[start..i]) > width {
                lines.push(para[start..end].to_vec());
                start = word;
            }
            end = i;
        }
        lines.push(para[start..end].to_vec());
        if stop >= rest.len() {
            break;
        }
        let skip = if rest[stop] == b'\r' && rest.get(stop + 1) == Some(&b'\n') {
            2
        } else {
            1
        };
        rest = &rest[stop + skip..];
    }
    lines
}

/// `DrawText(text, r, format)` in `f`, colour `c`, transparent.
pub fn draw_text(cx: &mut DrawCx<'_>, f: GadgetFont, r: IRect, text: &[u8], c: u8, format: Format) {
    let Some((font, fake)) = pick(cx.assets, f) else {
        return;
    };
    let height = i32::from(font.height);
    match format {
        Format::Line => {
            let w = line_width(font, fake, text);
            let x = (r.x0 + r.x1 - w) / 2;
            let y = (r.y0 + r.y1 - height) / 2;
            draw_line(cx, font, fake, x, y, text, c);
        }
        Format::Wrapped => {
            let lines = wrap(text, r.width(), |t| line_width(font, fake, t));
            let mut y = r.y0;
            for line in lines {
                let w = line_width(font, fake, &line);
                draw_line(cx, font, fake, (r.x0 + r.x1 - w) / 2, y, &line, c);
                y += height;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The Message List

/// The Message List (`TxLn`, cTextLine, io.md 14): message `n` of four for
/// input `n`, in green, while running.
#[derive(Debug, Default)]
pub struct TextLine {
    /// The message shown, 0 to 3, -1 for none (`+0x142`; reset makes it 0,
    /// start -1).
    selected: i32,
}

impl TextLine {
    /// The message shown (0 to 3), if any.
    #[must_use]
    pub fn selected(&self) -> Option<usize> {
        usize::try_from(self.selected).ok().filter(|&i| i < 4)
    }
}

impl Behavior for TextLine {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame_at_rect(core.sheet, 1, core.h, core.v);
        let Some(i) = self.selected().filter(|_| cx.running) else {
            return;
        };
        // `0x45b9ec`: the first 34 bytes.
        let text = core.settings.label(i);
        let text = &text[..text.len().min(34)];
        // A message starting with '@' runs the rest as a program
        // (`WinExec`), once each time a different message is drawn. The
        // port runs nothing; the '@' is drawn.
        let r = core.rect();
        let r = IRect::new(r.x0 + 17, r.y0, r.x1 - 6, r.y1);
        let green = cx.color([0, 255, 0]);
        draw_text(cx, GadgetFont::Helv16Bold, r, text, green, Format::Line);
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        // Input 1..4 is message 0..3; no clamping.
        let n = trunc(io.number(core, 0).floor()).wrapping_sub(1);
        self.selected = n;
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        self.selected = -1;
    }

    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        self.selected = 0;
    }

    fn init(&mut self, core: &mut Core) {
        let s = &mut core.settings;
        s.set_label(0, b"Message One");
        s.set_label(1, b"Message Two");
        s.set_label(2, b"Message Three");
        s.set_label(3, b"Message Four");
    }

    any!();
}

#[must_use]
pub fn text_line() -> Box<dyn Behavior> {
    Box::new(TextLine { selected: -1 })
}

// ---------------------------------------------------------------------------
// The Billboard

/// The Billboard's mark that its line breaks are hidden (number field 0,
/// `0x45aa55`).
pub const CRLF_MAGIC: u32 = 0x0261_1500;

/// `0x45aa55`: the block as saved: every CR LF in the text blanked, the
/// first four's places in the radio bytes (0xff for none), and the mark.
#[must_use]
pub fn crlf_hide(s: &Settings) -> Settings {
    let mut s = s.clone();
    s.numbers[0] = CRLF_MAGIC;
    let mut n = 0;
    for i in 0..s.text.len() - 1 {
        if s.text[i] == b'\r' && s.text[i + 1] == b'\n' {
            if n < 4 {
                s.radios[n] = i as u8;
                n += 1;
            }
            s.text[i] = b' ';
            s.text[i + 1] = b' ';
        }
    }
    for r in &mut s.radios[n..] {
        *r = 0xff;
    }
    s
}

/// `0x45ab08`: undoes [`crlf_hide`], if the block has the mark (the first
/// four line breaks come back; any after them stay spaces).
pub fn crlf_restore(s: &mut Settings) {
    if s.numbers[0] != CRLF_MAGIC {
        return;
    }
    s.numbers[0] = 0;
    for p in s.radios {
        let p = usize::from(p);
        if p < 0xef && s.text[p] == b' ' && s.text[p + 1] == b' ' {
            s.text[p] = b'\r';
            s.text[p + 1] = b'\n';
        }
    }
}

/// The Billboard (`TxBx`, cTextBox, io.md 15): up to 240 characters of
/// text, shown while its input is on.
#[derive(Debug, Default)]
pub struct TextBox {
    /// The block as it was read, when the widget was saved without hiding
    /// the line breaks (the Mac version's widgets, as on the CD). Written
    /// back as read while unchanged, so they load and save byte for byte;
    /// the original would hide them.
    as_read: Option<Settings>,
}

impl Behavior for TextBox {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame_at_rect(core.sheet, 1, core.h, core.v);
        if !(cx.running && core.state) {
            return;
        }
        // `0x45abe3` restores the line breaks first; here they always are.
        let r = IRect::new(core.h + 15, core.v + 8, core.h + 178, core.v + 80);
        let black = cx.color([0, 0, 0]);
        draw_text(
            cx,
            GadgetFont::Helv16,
            r,
            core.settings.long_text(),
            black,
            Format::Wrapped,
        );
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        core.state = truthy(io.number(core, 0));
    }

    fn init(&mut self, core: &mut Core) {
        core.long_text = true;
        core.settings.set_long_text(
            b"You can enter up to\r\nfour lines of static text\r\nto be displayed\r\non this Billboard.",
        );
    }

    fn load(&mut self, core: &mut Core, _data: &[u8]) {
        core.long_text = true;
        if core.settings.numbers[0] == CRLF_MAGIC {
            crlf_restore(&mut core.settings);
        } else {
            self.as_read = Some(core.settings.clone());
        }
    }

    /// `0x45aed5`: saved with the line breaks hidden.
    fn saved_settings(&self, core: &Core) -> Option<Settings> {
        if self.as_read.as_ref() == Some(&core.settings) {
            return None;
        }
        Some(crlf_hide(&core.settings))
    }

    any!();
}

#[must_use]
pub fn text_box() -> Box<dyn Behavior> {
    Box::new(TextBox::default())
}

// ---------------------------------------------------------------------------
// The Label

/// The Label's font for its size (radio group 0).
fn label_font(core: &Core) -> GadgetFont {
    if core.settings.radios[0] == 0 {
        GadgetFont::Helv16Bold
    } else {
        GadgetFont::Helv24
    }
}

/// `0x45af79`: the label's bounds from its text: the extent grown by 6
/// across and 2 down.
fn label_bounds(assets: &Assets, core: &Core) -> Rect {
    let (w, h) = text_extent(assets, label_font(core), core.settings.label(0));
    Rect {
        top: -2,
        left: -6,
        bottom: (h + 2) as i16,
        right: (w + 6) as i16,
    }
}

/// The Label (`TxLb`, cTextLabel, io.md 16): text on the worksheet, with
/// a shadow, in both modes. No sheet, no knots.
#[derive(Debug, Default)]
pub struct TextLabel;

impl Behavior for TextLabel {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        // Measured again on every draw.
        let r = IRect::from_mac(label_bounds(cx.assets, core), core.h, core.v);
        let text = core.settings.label(0);
        let font = label_font(core);
        let (white, black) = (cx.color([255, 255, 255]), cx.color([0, 0, 0]));
        let (shadow, ink) = if cx.lights { (white, black) } else { (black, white) };
        draw_text(cx, font, r.offset(1, 1), text, shadow, Format::Line);
        draw_text(cx, font, r, text, ink, Format::Line);
    }

    fn layout(&mut self, core: &mut Core, assets: &Assets) {
        core.bounds = label_bounds(assets, core);
    }

    fn init(&mut self, core: &mut Core) {
        core.settings.set_label(0, b"Label");
    }

    any!();
}

#[must_use]
pub fn text_label() -> Box<dyn Behavior> {
    Box::new(TextLabel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gadgets::common::bench::{Bench, Source};
    use formats::tables::KnotKind;

    #[test]
    fn billboard_hides_line_breaks_to_save() {
        let mut s = Settings::default();
        s.set_long_text(b"a\r\nb\r\nc\r\nd\r\ne\r\nf");
        let h = crlf_hide(&s);
        assert_eq!(h.numbers[0], CRLF_MAGIC);
        assert_eq!(h.radios, [1, 4, 7, 10]);
        assert_eq!(h.long_text(), b"a  b  c  d  e  f");
        let mut r = h.clone();
        crlf_restore(&mut r);
        // The fifth is lost.
        assert_eq!(r.long_text(), b"a\r\nb\r\nc\r\nd\r\ne  f");
        assert_eq!(r.numbers[0], 0);
        // And hides again to the same bytes.
        assert_eq!(crlf_hide(&r).to_be(), h.to_be());
        let mut one = Settings::default();
        one.set_long_text(b"x\r\ny");
        assert_eq!(crlf_hide(&one).radios, [1, 0xff, 0xff, 0xff]);
    }

    #[test]
    fn billboard_saves_like_the_original_once_changed() {
        let mut t = TextBox::default();
        let mut core = Core::new(crate::gadget::Tag::new(b"TxBx"), 2027, 1047);
        // As on the CD: raw line breaks, no mark.
        core.settings.set_long_text(b"one\r\ntwo");
        t.load(&mut core, &[]);
        assert_eq!(t.saved_settings(&core), None);
        core.settings.set_long_text(b"one\r\nthree");
        assert_eq!(t.saved_settings(&core).unwrap().long_text(), b"one  three");
        // As the Win95 game saves: restored on load, hidden again to save.
        let mut t = TextBox::default();
        core.settings = crlf_hide(&core.settings);
        let saved = core.settings.to_be();
        t.load(&mut core, &[]);
        assert_eq!(core.settings.long_text(), b"one\r\nthree");
        assert_eq!(t.saved_settings(&core).unwrap().to_be(), saved);
    }

    #[test]
    fn wraps_like_drawtext() {
        let w = |t: &[u8]| i32::try_from(t.len()).unwrap();
        let lines = wrap(b"aa bb cc\r\n\r\nd  eeeeeeeee f", 5, w);
        let lines: Vec<&[u8]> = lines.iter().map(Vec::as_slice).collect();
        assert_eq!(lines, [&b"aa bb"[..], b"cc", b"", b"d", b"eeeeeeeee", b"f"]);
    }

    #[test]
    fn message_list_picks_by_number() {
        let mut b = Bench::with_knots(&[KnotKind::Input]);
        let mut t = TextLine::default();
        b.fresh(&mut t);
        assert_eq!(b.core.settings.label(2), b"Message Three");
        // Stopped: message 1 (only drawn while running).
        assert_eq!(t.selected(), Some(0));
        b.start(&mut t);
        assert_eq!(t.selected(), None);
        for (x, n) in [(4.99, Some(3)), (1.0, Some(0)), (5.0, None), (0.5, None), (-0.5, None)] {
            b.source(0, Source::number(x));
            b.tick_at(&mut t, 0);
            assert_eq!(t.selected(), n, "{x}");
        }
    }
}
