//! Help ▸ Contents (`0x4333f5`, docs/help.md): the original calls
//! `WinHelp(HELP_CONTENTS)` on `WIDGET.HLP`, and Windows 95's `WINHLP32`
//! shows its contents topic in a window of its own. This is a look-alike of
//! that window drawn into the editor's surface: the caption, the button bar
//! (`Contents`, `Index`, `Back`, `Print`, and `<<`/`>>` from the file's
//! `BrowseButtons()` macro), the topic's non-scrolling region and its
//! scrolling text, with the jump hotspots, the Back history and the Index
//! (`WinHelp` 4's "Help Topics" dialog, Index tab).
//!
//! The OS/2 Warp CD's `WIDGET.INF` (an IPF book) shows in the same window:
//! [`load`] tells the two apart by their first bytes and turns the book's
//! panels into the same topics (`formats::inf::InfFile::to_help`).
//!
//! The text is laid out from `formats::hlp`'s paragraphs with the CD's
//! MS Sans Serif bitmap fonts standing in for Arial: the nearest point
//! size, bold over-struck one pixel to the right when there's no bold face,
//! italic sheared. Distances are the help compiler's units, ⅔ of a pixel
//! at 96 dpi. The pictures are the Doc-To-Help `{ewc}` windows' baggage
//! BMPs, mapped to the game's palette.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use formats::fnt::Font;
use formats::hlp::{Align, HelpFile, Inline, Link, LinkAction, Paragraph};
use formats::hr::Image;

use crate::assets::Assets;
use crate::gadget::nearest;
use crate::gfx::{IRect, Surface};

use super::Pointer;
use super::puzzle_dialog::focus_rect;
use super::ui;

/// The message the original shows while `WIDGET.HLP` isn't there
/// (`0x4333f5`, through `0x45f99f` with `MB_RETRYCANCEL`).
pub const MISSING: &str =
    "Can't find help file\nIt should be in the same folder as Widget Workshop and be named WIDGET.HLP";

/// Reads a help file: `WIDGET.HLP` (`WinHelp`), or an OS/2 book
/// (`WIDGET.INF`, by its `HSP` signature) as the same topics. `None` if
/// it doesn't parse.
#[must_use]
pub fn load(data: &[u8]) -> Option<HelpFile> {
    if formats::inf::is_inf(data) {
        formats::inf::InfFile::parse(data).ok()?.to_help().ok()
    } else {
        HelpFile::parse(data).ok()
    }
}

/// A sizing frame (`SM_CXFRAME`) and the caption (`SM_CYCAPTION` − 1).
const FRAME: i32 = 4;
const CAPTION_H: i32 = 18;
/// The caption's boxes (`SM_CXSIZE` − 2 × `SM_CYSIZE` − 4).
const BOX_W: i32 = 16;
const BOX_H: i32 = 14;
/// The button bar: buttons a dialog button high, 2 pixels apart.
const BUTTON_H: i32 = 23;
const BUTTON_GAP: i32 = 2;
const BAR_H: i32 = BUTTON_H + 2 * 3;
/// A scroll bar's width and its arrows' size (`SM_CXVSCROLL`).
const SB: i32 = 16;
/// The smallest thumb.
const MIN_THUMB: i32 = 8;
/// The topic's margins (`WinHelp`'s left margin is about an eighth of an
/// inch).
const MARGIN: i32 = 8;
/// Default tab stops: every half inch.
const DEFAULT_TAB: i32 = 48;
/// The space a border keeps from the text.
const BORDER_PAD: i32 = 3;
/// A line of the arrows and the wheel (a notch is three).
const LINE: i32 = 16;
/// The smallest window.
const MIN_SIZE: (i32, i32) = (240, 180);

/// The button bar's buttons.
const BUTTONS: [&str; 6] = ["&Contents", "&Index", "&Back", "&Print", "<&<", "&>>"];
pub const B_CONTENTS: usize = 0;
pub const B_INDEX: usize = 1;
pub const B_BACK: usize = 2;
pub const B_PRINT: usize = 3;
pub const B_PREV: usize = 4;
pub const B_NEXT: usize = 5;

const VK_BACK: u16 = 0x08;
const VK_TAB: u16 = 0x09;
const VK_RETURN: u16 = 0x0d;
const VK_ESCAPE: u16 = 0x1b;
const VK_PRIOR: u16 = 0x21;
const VK_NEXT: u16 = 0x22;
const VK_END: u16 = 0x23;
const VK_HOME: u16 = 0x24;
const VK_LEFT: u16 = 0x25;
const VK_UP: u16 = 0x26;
const VK_RIGHT: u16 = 0x27;
const VK_DOWN: u16 = 0x28;

/// Help-compiler units to pixels: ×10 is twips, so a unit is ⅔ pixel.
fn px(u: i16) -> i32 {
    let u = i32::from(u) * 2;
    if u >= 0 { (u + 1) / 3 } else { (u - 1) / 3 }
}

fn len(n: usize) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

// ---- fonts ----------------------------------------------------------

/// How a run of text is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Face {
    points: u16,
    bold: bool,
    italic: bool,
    underline: Underline,
    /// The Symbol font: its bytes are its own characters.
    symbol: bool,
    colour: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Underline {
    None,
    Solid,
    /// A popup hotspot.
    Dotted,
}

/// The bitmap font standing in for `points` (MS Sans Serif's nearest size,
/// the smaller on a tie; a bold face if there is one) and how many pixels
/// to over-strike to fake bold (two from 20 points: Arial Bold's strokes
/// are that heavy).
fn font_for(assets: &Assets, points: u16, bold: bool) -> Option<(&Font, i32)> {
    let f = assets
        .fonts
        .iter()
        .filter(|f| f.face.eq_ignore_ascii_case("MS Sans Serif"))
        .min_by_key(|f| {
            (
                f.points.abs_diff(points) * 2 + u16::from(f.points > points),
                u16::from((f.weight >= 600) != bold),
            )
        })
        .or_else(|| ui::font(assets))?;
    let extra = if bold && f.weight < 600 {
        1 + i32::from(f.points >= 20)
    } else {
        0
    };
    Some((f, extra))
}

/// The width of `text` in `font`, wider by the over-strike a character
/// when bold is faked (GDI's emboldening widens every character).
fn text_width(font: &Font, bold: i32, text: &[u8]) -> i32 {
    i32::try_from(font.width(text)).unwrap_or(0) + bold * len(text.len())
}

/// Draws `text` with its top left at `x`, `y`: over-struck `bold` pixels
/// to the right for a faked bold, sheared for italic.
#[allow(clippy::too_many_arguments)]
fn draw_text(s: &mut Surface, font: &Font, x: i32, y: i32, text: &[u8], c: u8, bold: i32, italic: bool) {
    let asc = i32::from(font.ascent);
    let mut at = x;
    for &ch in text {
        let Some(g) = font.glyph(ch) else { continue };
        let w = i32::from(g.width);
        for gy in 0..i32::from(font.height) {
            let shear = if italic { (asc - gy).div_euclid(4) } else { 0 };
            for gx in 0..w {
                if g.bits.get(usize::try_from(gy * w + gx).unwrap_or(usize::MAX)) == Some(&true) {
                    for b in 0..=bold {
                        s.pset(at + gx + shear + b, y + gy, c);
                    }
                }
            }
        }
        at += w + bold;
    }
}

// ---- layout ---------------------------------------------------------

/// One indivisible piece of a paragraph.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Atom {
    /// Characters without a space, or one space.
    Text {
        bytes: Vec<u8>,
        face: Face,
        link: Option<usize>,
    },
    /// A Symbol-font character: `0xB7` a bullet, `0xBE` a horizontal bar
    /// (used as an em dash). Neither is in the bitmap fonts.
    Symbol {
        ch: u8,
        face: Face,
        link: Option<usize>,
    },
    Picture {
        name: String,
        w: i32,
        h: i32,
        link: Option<usize>,
    },
    Tab,
    Break,
}

impl Atom {
    fn is_space(&self) -> bool {
        matches!(self, Atom::Text { bytes, .. } if bytes == b" ")
    }

    fn link(&self) -> Option<usize> {
        match self {
            Atom::Text { link, .. } | Atom::Symbol { link, .. } | Atom::Picture { link, .. } => *link,
            Atom::Tab | Atom::Break => None,
        }
    }

    /// Width, ascent and descent.
    fn metrics(&self, assets: &Assets) -> (i32, i32, i32) {
        match self {
            Atom::Text { bytes, face, .. } => font_for(assets, face.points, face.bold).map_or((0, 0, 0), |(f, fb)| {
                let asc = i32::from(f.ascent);
                (text_width(f, fb, bytes), asc, i32::from(f.height) - asc)
            }),
            Atom::Symbol { ch, face, .. } => font_for(assets, face.points, face.bold).map_or((0, 0, 0), |(f, fb)| {
                let asc = i32::from(f.ascent);
                let em = i32::from(face.points) * 4 / 3;
                let w = match ch {
                    0xb7 => em * 46 / 100,
                    0xbe => em,
                    _ => text_width(f, fb, &[*ch]),
                };
                (w, asc, i32::from(f.height) - asc)
            }),
            Atom::Picture { w, h, .. } => (*w, *h, 0),
            Atom::Tab | Atom::Break => (0, 0, 0),
        }
    }
}

/// An atom placed: `x` and its line's baseline.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Piece {
    x: i32,
    base: i32,
    w: i32,
    atom: Atom,
}

/// A hotspot's area, and its link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Hot {
    rect: IRect,
    link: usize,
}

/// Paragraphs laid out at one width, from 0, 0.
#[derive(Debug, Clone, Default)]
pub struct Laid {
    pieces: Vec<Piece>,
    /// Border lines.
    rules: Vec<IRect>,
    hots: Vec<Hot>,
    /// The hotspots' targets, by link number.
    links: Vec<LinkAction>,
    /// The right end of the widest line, and the bottom.
    pub width: i32,
    pub height: i32,
}

/// The pictures, mapped to the game's palette as they're first laid out.
#[derive(Default)]
struct Pictures {
    cache: RefCell<HashMap<String, Option<Rc<Image>>>>,
}

impl Pictures {
    fn get(&self, file: &HelpFile, palette: &[[u8; 3]; 256], name: &str) -> Option<Rc<Image>> {
        let key = name.to_ascii_lowercase();
        if let Some(i) = self.cache.borrow().get(&key) {
            return i.clone();
        }
        let img = file.bitmap(name).ok().and_then(|b| {
            let lut: Vec<u8> = b.palette.iter().map(|c| nearest(palette, [c.r, c.g, c.b])).collect();
            let mut img = Image::new(u16::try_from(b.width).ok()?, u16::try_from(b.height).ok()?);
            for y in 0..b.height {
                for x in 0..b.width {
                    img.pixels[y * b.width + x] = lut[usize::from(b.pixel(x, y))];
                }
            }
            img.opaque.fill(true);
            Some(Rc::new(img))
        });
        self.cache.borrow_mut().insert(key, img.clone());
        img
    }
}

/// What a layout needs to look up.
struct Cx<'a> {
    assets: &'a Assets,
    file: &'a HelpFile,
    palette: &'a [[u8; 3]; 256],
    pictures: &'a Pictures,
    green: u8,
}

impl Cx<'_> {
    fn face(&self, font: u16, link: Option<&Link>) -> Face {
        let f = self.file.fonts.get(usize::from(font));
        let mut face = Face {
            points: f.map_or(10, |f| u16::from(f.half_points.max(2)) / 2),
            bold: f.is_some_and(formats::hlp::Font::bold),
            italic: f.is_some_and(formats::hlp::Font::italic),
            underline: if f.is_some_and(|f| f.underline() || f.double_underline()) {
                Underline::Solid
            } else {
                Underline::None
            },
            symbol: f.is_some_and(|f| f.face.eq_ignore_ascii_case("Symbol")),
            colour: f
                .and_then(|f| f.colour)
                .map_or(ui::BLACK, |c| nearest(self.palette, [c.r, c.g, c.b])),
        };
        // Visible hotspots: green, underlined (dotted for popups).
        if let Some(l) = link.filter(|l| l.visible) {
            face.colour = self.green;
            face.underline = if matches!(l.action, LinkAction::Popup(_)) {
                Underline::Dotted
            } else {
                Underline::Solid
            };
        }
        face
    }

    /// `para`'s atoms; links get numbers in `links`.
    fn atoms(&self, para: &Paragraph, links: &mut Vec<LinkAction>) -> Vec<Atom> {
        let mut out = Vec::new();
        let mut last: Option<(Link, usize)> = None;
        let mut number = |link: Option<&Link>, links: &mut Vec<LinkAction>| -> Option<usize> {
            let l = link?;
            if let Some((prev, n)) = &last
                && prev == l
            {
                return Some(*n);
            }
            links.push(l.action.clone());
            let n = links.len() - 1;
            last = Some((l.clone(), n));
            Some(n)
        };
        for item in &para.items {
            match item {
                Inline::Text { text, font, link } => {
                    let face = self.face(*font, link.as_ref());
                    let link = number(link.as_ref(), links);
                    if face.symbol {
                        for &ch in text {
                            out.push(if ch == b' ' {
                                Atom::Text {
                                    bytes: vec![ch],
                                    face: Face { symbol: false, ..face },
                                    link,
                                }
                            } else {
                                Atom::Symbol { ch, face, link }
                            });
                        }
                        continue;
                    }
                    let mut word = Vec::new();
                    for &ch in text {
                        if ch == b' ' {
                            if !word.is_empty() {
                                out.push(Atom::Text {
                                    bytes: std::mem::take(&mut word),
                                    face,
                                    link,
                                });
                            }
                            out.push(Atom::Text {
                                bytes: vec![b' '],
                                face,
                                link,
                            });
                        } else {
                            word.push(ch);
                        }
                    }
                    if !word.is_empty() {
                        out.push(Atom::Text {
                            bytes: word,
                            face,
                            link,
                        });
                    }
                }
                Inline::Tab => out.push(Atom::Tab),
                Inline::LineBreak => out.push(Atom::Break),
                Inline::Picture { picture, link, .. } => {
                    let link = number(link.as_ref(), links);
                    let name = picture.baggage_bitmap().unwrap_or_default();
                    let (w, h) = self
                        .pictures
                        .get(self.file, self.palette, name)
                        .map_or((0, 0), |i| (i32::from(i.width), i32::from(i.height)));
                    out.push(Atom::Picture {
                        name: name.to_owned(),
                        w,
                        h,
                        link,
                    });
                }
            }
        }
        out
    }

    /// Lays `paras` out `width` pixels wide.
    fn lay_out(&self, paras: &[Paragraph], width: i32) -> Laid {
        let mut laid = Laid::default();
        let mut y = 0;
        for para in paras {
            y = self.paragraph(para, width, y, &mut laid);
        }
        // A little room under the last line.
        laid.height = if paras.is_empty() { 0 } else { y + BORDER_PAD };
        laid
    }

    /// One paragraph from `y`; returns the bottom.
    #[allow(clippy::too_many_lines)]
    fn paragraph(&self, para: &Paragraph, width: i32, mut y: i32, laid: &mut Laid) -> i32 {
        let f = &para.format;
        let left = MARGIN + px(f.left_indent);
        let right = (width - MARGIN - px(f.right_indent)).max(left + 1);
        let first = left + px(f.first_line_indent);
        let border = f.border.map_or(0, |b| b.sides);
        let thick = if border & 0x20 != 0 { 2 } else { 1 };
        let double = border & 0x40 != 0;
        let rule_h = if double { 3 } else { thick };
        let (top, bottom) = (border & 0x03 != 0, border & 0x09 != 0);
        let (bx0, bx1) = (first.min(left) - BORDER_PAD, right + BORDER_PAD);
        y += px(f.space_above);
        let box_top = y;
        if top {
            y += rule_h + BORDER_PAD;
        }
        let atoms = self.atoms(para, &mut laid.links);
        let assets = self.assets;
        // Lines of (x, width, atom).
        let mut lines: Vec<Vec<(i32, i32, Atom)>> = vec![Vec::new()];
        let mut pen = first;
        let new_line = |lines: &mut Vec<Vec<(i32, i32, Atom)>>, pen: &mut i32| {
            // Spaces at the end of a line aren't drawn.
            if let Some(l) = lines.last_mut() {
                while l.last().is_some_and(|(_, _, a)| a.is_space()) {
                    l.pop();
                }
            }
            lines.push(Vec::new());
            *pen = left;
        };
        let mut i = 0;
        while i < atoms.len() {
            match &atoms[i] {
                Atom::Break => {
                    new_line(&mut lines, &mut pen);
                    i += 1;
                }
                Atom::Tab => {
                    // The next stop past the pen: a set one (from the
                    // margin), the hanging indent, or a default one.
                    let rel = pen - MARGIN;
                    let hanging = (px(f.left_indent) > rel).then(|| px(f.left_indent));
                    let set = f
                        .tabs
                        .iter()
                        .map(|t| px(t.position.try_into().unwrap_or(i16::MAX)))
                        .find(|&t| t > rel);
                    let stop = match (set, hanging) {
                        (Some(a), Some(b)) => a.min(b),
                        (a, b) => a.or(b).unwrap_or((rel / DEFAULT_TAB + 1) * DEFAULT_TAB),
                    };
                    let x = MARGIN + stop;
                    if x > right && lines.last().is_some_and(|l| !l.is_empty()) {
                        new_line(&mut lines, &mut pen);
                    } else {
                        pen = x;
                    }
                    i += 1;
                }
                a if a.is_space() => {
                    let (w, _, _) = a.metrics(assets);
                    if let Some(l) = lines.last_mut()
                        && !l.is_empty()
                    {
                        l.push((pen, w, a.clone()));
                    }
                    pen += w;
                    i += 1;
                }
                _ => {
                    // A word: up to the next space, tab or break.
                    let end = (i..atoms.len())
                        .find(|&j| atoms[j].is_space() || matches!(atoms[j], Atom::Tab | Atom::Break))
                        .unwrap_or(atoms.len());
                    let widths: Vec<i32> = atoms[i..end].iter().map(|a| a.metrics(assets).0).collect();
                    let w: i32 = widths.iter().sum();
                    let has_content = lines.last().is_some_and(|l| l.iter().any(|(_, _, a)| !a.is_space()));
                    if pen + w > right && has_content {
                        new_line(&mut lines, &mut pen);
                    }
                    for (a, aw) in atoms[i..end].iter().zip(widths) {
                        if let Some(l) = lines.last_mut() {
                            l.push((pen, aw, a.clone()));
                        }
                        pen += aw;
                    }
                    i = end;
                }
            }
        }
        if let Some(l) = lines.last_mut() {
            while l.last().is_some_and(|(_, _, a)| a.is_space()) {
                l.pop();
            }
        }
        // The empty-line height: the paragraph's first font.
        let default_metrics = atoms
            .iter()
            .find(|a| matches!(a, Atom::Text { .. } | Atom::Symbol { .. }))
            .map_or_else(
                || {
                    font_for(assets, 10, false).map_or((0, 13, 3), |(f, _)| {
                        (0, i32::from(f.ascent), i32::from(f.height - f.ascent))
                    })
                },
                |a| a.metrics(assets),
            );
        let spacing = px(f.line_spacing);
        for line in lines {
            let (mut asc, mut desc) = if line.is_empty() {
                (default_metrics.1, default_metrics.2)
            } else {
                (0, 0)
            };
            for (_, _, a) in &line {
                let (_, a_asc, a_desc) = a.metrics(assets);
                asc = asc.max(a_asc);
                desc = desc.max(a_desc);
            }
            // Positive: at least; negative: exactly.
            let natural = asc + desc;
            let h = match spacing {
                s if s > 0 => natural.max(s),
                s if s < 0 => -s,
                _ => natural,
            };
            let base = y + h - desc;
            let line_w = line.last().map_or(0, |(x, w, _)| x + w);
            let shift = match f.align {
                Align::Left => 0,
                Align::Right => (right - line_w).max(0),
                Align::Centre => ((right - line_w) / 2).max(0),
            };
            for (x, w, a) in line {
                let x = x + shift;
                laid.width = laid.width.max(x + w + MARGIN);
                if let Some(link) = a.link() {
                    let r = IRect::new(x, y, x + w, y + h);
                    match laid.hots.last_mut() {
                        Some(last) if last.link == link && last.rect.y0 == r.y0 && last.rect.x1 == r.x0 => {
                            last.rect.x1 = r.x1;
                        }
                        _ => laid.hots.push(Hot { rect: r, link }),
                    }
                }
                laid.pieces.push(Piece { x, base, w, atom: a });
            }
            y += h;
        }
        if bottom {
            y += BORDER_PAD;
        }
        let rule = |laid: &mut Laid, r: IRect| {
            if double {
                laid.rules.push(IRect::new(r.x0, r.y0, r.x1, r.y0 + 1));
                laid.rules.push(IRect::new(r.x0, r.y1 - 1, r.x1, r.y1));
            } else {
                laid.rules.push(r);
            }
        };
        if top {
            rule(laid, IRect::new(bx0, box_top, bx1, box_top + rule_h));
        }
        if bottom {
            rule(laid, IRect::new(bx0, y, bx1, y + rule_h));
            y += rule_h;
        }
        if border & 0x05 != 0 {
            laid.rules.push(IRect::new(bx0, box_top, bx0 + thick, y));
        }
        if border & 0x11 != 0 {
            laid.rules.push(IRect::new(bx1 - thick, box_top, bx1, y));
        }
        if border != 0 {
            laid.width = laid.width.max(bx1);
        }
        y + px(f.space_below)
    }
}

impl Laid {
    /// Draws it with its origin at `x`, `y`.
    fn draw(&self, s: &mut Surface, cx: &Cx<'_>, x: i32, y: i32) {
        let assets = cx.assets;
        let clip = s.clip();
        for r in &self.rules {
            s.fill(r.offset(x, y), ui::BLACK);
        }
        for p in &self.pieces {
            let (px0, base) = (x + p.x, y + p.base);
            // Text far outside the clip isn't drawn (a picture may reach in).
            if (base - 64 > clip.y1 || base + 64 < clip.y0) && !matches!(p.atom, Atom::Picture { .. }) {
                continue;
            }
            match &p.atom {
                Atom::Text { bytes, face, .. } => {
                    let Some((f, fb)) = font_for(assets, face.points, face.bold) else {
                        continue;
                    };
                    let top = base - i32::from(f.ascent);
                    if bytes != b" " {
                        draw_text(s, f, px0, top, bytes, face.colour, fb, face.italic);
                    }
                    underline(s, *face, px0, px0 + p.w, base);
                }
                Atom::Symbol { ch, face, .. } => {
                    let Some((f, fb)) = font_for(assets, face.points, face.bold) else {
                        continue;
                    };
                    let asc = i32::from(f.ascent);
                    match ch {
                        0xb7 => {
                            // A round bullet a third of the ascent across,
                            // centred on the x-height.
                            let d = (asc / 3).max(3);
                            let cy = base - asc * 2 / 5;
                            let cx = px0 + (p.w - d) / 2;
                            for yy in 0..d {
                                for xx in 0..d {
                                    let (dx, dy) = (2 * xx + 1 - d, 2 * yy + 1 - d);
                                    if dx * dx + dy * dy <= d * d {
                                        s.pset(cx + xx, cy - d / 2 + yy, face.colour);
                                    }
                                }
                            }
                        }
                        0xbe => {
                            let my = base - asc * 2 / 5;
                            s.fill(IRect::new(px0, my, px0 + p.w, my + 1), face.colour);
                        }
                        _ => {
                            draw_text(s, f, px0, base - asc, &[*ch], face.colour, fb, face.italic);
                        }
                    }
                    underline(s, *face, px0, px0 + p.w, base);
                }
                Atom::Picture { name, h, .. } => {
                    if let Some(img) = cx.pictures.get(cx.file, cx.palette, name) {
                        s.blit(&img, px0, base - h);
                    }
                }
                Atom::Tab | Atom::Break => {}
            }
        }
    }

    /// The hotspot at `p` (in the layout's coordinates).
    fn hot_at(&self, p: (i32, i32)) -> Option<usize> {
        self.hots.iter().find(|h| h.rect.contains(p.0, p.1)).map(|h| h.link)
    }
}

fn underline(s: &mut Surface, face: Face, x0: i32, x1: i32, base: i32) {
    match face.underline {
        Underline::None => {}
        Underline::Solid => s.fill(IRect::new(x0, base + 1, x1, base + 2), face.colour),
        Underline::Dotted => {
            for x in (x0..x1).filter(|x| x % 2 == 0) {
                s.pset(x, base + 1, face.colour);
            }
        }
    }
}

// ---- scroll bars ------------------------------------------------------

/// A scroll bar's parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Less,
    More,
    PageLess,
    PageMore,
    Thumb,
}

/// A scroll bar: its rectangle, which way, and the range it shows.
#[derive(Debug, Clone, Copy)]
struct ScrollBar {
    rect: IRect,
    vertical: bool,
    pos: i32,
    /// The whole length and the part shown.
    total: i32,
    page: i32,
}

impl ScrollBar {
    fn max(&self) -> i32 {
        (self.total - self.page).max(0)
    }

    fn track(&self) -> (i32, i32) {
        if self.vertical {
            (self.rect.y0 + SB, self.rect.y1 - SB)
        } else {
            (self.rect.x0 + SB, self.rect.x1 - SB)
        }
    }

    fn thumb(&self) -> IRect {
        let (t0, t1) = self.track();
        let room = t1 - t0;
        let size = (room * self.page / self.total.max(1)).max(MIN_THUMB).min(room);
        let at = t0 + (room - size) * self.pos / self.max().max(1);
        if self.vertical {
            IRect::new(self.rect.x0, at, self.rect.x1, at + size)
        } else {
            IRect::new(at, self.rect.y0, at + size, self.rect.y1)
        }
    }

    fn part_at(&self, p: (i32, i32)) -> Option<Part> {
        if !self.rect.contains(p.0, p.1) {
            return None;
        }
        let (t0, t1) = self.track();
        let t = self.thumb();
        let (v, a, b) = if self.vertical {
            (p.1, t.y0, t.y1)
        } else {
            (p.0, t.x0, t.x1)
        };
        Some(if v < t0 {
            Part::Less
        } else if v >= t1 {
            Part::More
        } else if v < a {
            Part::PageLess
        } else if v >= b {
            Part::PageMore
        } else {
            Part::Thumb
        })
    }

    /// The position for the thumb dragged so that its start is at `v`.
    fn pos_for(&self, v: i32) -> i32 {
        let (t0, t1) = self.track();
        let t = self.thumb();
        let size = if self.vertical { t.height() } else { t.width() };
        let room = t1 - t0 - size;
        if room <= 0 {
            return 0;
        }
        ((v - t0).clamp(0, room) * self.max() + room / 2) / room
    }

    fn draw(&self, s: &mut Surface, font: &Font, pressed: Option<Part>) {
        let b = self.rect;
        for y in b.y0..b.y1 {
            for x in b.x0..b.x1 {
                s.pset(x, y, if (x + y) % 2 == 0 { ui::WHITE } else { ui::FACE });
            }
        }
        let (less, more) = if self.vertical {
            (IRect::sized(b.x0, b.y0, SB, SB), IRect::sized(b.x0, b.y1 - SB, SB, SB))
        } else {
            (IRect::sized(b.x0, b.y0, SB, SB), IRect::sized(b.x1 - SB, b.y0, SB, SB))
        };
        let active = self.max() > 0;
        for (r, part) in [(less, Part::Less), (more, Part::More)] {
            let down = pressed == Some(part);
            ui::button(s, font, r, "", down, false, true);
            let off = i32::from(down);
            let (cx, cy) = (r.x0 + 4 + off, r.y0 + 4 + off);
            let c = if active { ui::BLACK } else { ui::SHADOW };
            // A 7×4 triangle (4×7 sideways).
            for d in 0..4 {
                let (a, b0, b1) = match (self.vertical, part) {
                    (true, Part::Less) => (cy + 2 + d, cx + 3 - d, cx + 4 + d),
                    (true, _) => (cy + 5 - d, cx + 3 - d, cx + 4 + d),
                    (false, Part::Less) => (cx + 2 + d, cy + 3 - d, cy + 4 + d),
                    (false, _) => (cx + 5 - d, cy + 3 - d, cy + 4 + d),
                };
                if self.vertical {
                    s.fill(IRect::new(b0, a, b1, a + 1), c);
                } else {
                    s.fill(IRect::new(a, b0, a + 1, b1), c);
                }
            }
        }
        if active {
            ui::button(s, font, self.thumb(), "", false, false, true);
        } else {
            s.fill(IRect::new(less.x0, less.y0, more.x1, more.y1).intersect(b), ui::FACE);
            ui::button(s, font, less, "", false, false, true);
            ui::button(s, font, more, "", false, false, true);
        }
    }
}

// ---- the window -------------------------------------------------------

/// What an input left the window as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Open,
    Closed,
}

/// What the mouse holds down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Press {
    Close,
    Max,
    /// The caption, grabbed at this offset from the window's corner.
    Caption((i32, i32)),
    /// A sizing edge: left, top, right, bottom flags; where the mouse went
    /// down and the window then.
    Size(u8, (i32, i32), IRect),
    Button(usize),
    /// A scroll bar's part; for the thumb, the grab offset.
    Bar(bool, Part, i32),
    /// A hotspot, by link.
    Hot(usize),
    /// Elsewhere in the window.
    Other,
}

/// The current view's layout.
struct View {
    key: (usize, i32, i32),
    nsr: Laid,
    body: Laid,
    vbar: bool,
    hbar: bool,
}

/// The rectangles of the window, in frame coordinates.
struct Rects {
    caption: IRect,
    close: IRect,
    max: IRect,
    min: IRect,
    buttons: Vec<IRect>,
    nsr: IRect,
    pane: IRect,
    vbar: Option<IRect>,
    hbar: Option<IRect>,
}

/// The open help window.
pub struct Help {
    file: HelpFile,
    title: String,
    pub window: IRect,
    /// The window before it was maximized.
    restore: Option<IRect>,
    /// Has the keyboard (the game's window doesn't).
    pub active: bool,
    /// The topic shown (an index of `file.topics`) and how far it's
    /// scrolled.
    topic: usize,
    scroll: (i32, i32),
    /// Topics to go Back to.
    history: Vec<usize>,
    /// The hotspot with the focus rectangle (Tab).
    focus: Option<usize>,
    pressed: Option<Press>,
    /// Where a held scroll arrow repeats next (ms).
    repeat_at: u64,
    mouse: (i32, i32),
    index: Option<IndexDialog>,
    palette: [[u8; 3]; 256],
    green: u8,
    pictures: Pictures,
    view: RefCell<Option<Rc<View>>>,
}

impl Help {
    /// The window on `file`'s contents topic: at `at` (where it was last),
    /// else at the right of `area`, two thirds of it wide (at least wide
    /// enough for the 574-pixel screenshot) and all of it high.
    #[must_use]
    pub fn new(assets: &Assets, file: HelpFile, area: IRect, at: Option<IRect>) -> Help {
        let palette = assets.palette();
        let title = file
            .windows
            .iter()
            .find(|w| w.name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case("main")))
            .and_then(|w| w.caption.as_deref())
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .map_or_else(
                || {
                    if file.title.is_empty() {
                        "Windows Help".to_owned()
                    } else {
                        file.title.clone()
                    }
                },
                str::to_owned,
            );
        let w = (area.width() * 2 / 3).max(620).min(area.width());
        let window = at.unwrap_or(IRect::new(area.x1 - w, area.y0, area.x1, area.y1));
        let topic = file
            .contents_topic()
            .and_then(|t| file.topics.iter().position(|u| u.offset == t.offset))
            .unwrap_or(0);
        let mut h = Help {
            file,
            title,
            window,
            restore: None,
            active: true,
            topic,
            scroll: (0, 0),
            history: Vec::new(),
            focus: None,
            pressed: None,
            repeat_at: 0,
            mouse: (-1, -1),
            index: None,
            green: nearest(&palette, [0, 0x80, 0]),
            palette,
            pictures: Pictures::default(),
            view: RefCell::new(None),
        };
        h.fit(area);
        h
    }

    /// The caption.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The topic shown.
    #[must_use]
    pub fn topic(&self) -> &formats::hlp::Topic {
        &self.file.topics[self.topic]
    }

    /// Whether the Index is open.
    #[must_use]
    pub fn index_open(&self) -> bool {
        self.index.is_some()
    }

    /// Keeps the window in `area` (the desktop changed size).
    pub fn fit(&mut self, area: IRect) {
        if self.restore.is_some() {
            self.window = area;
            return;
        }
        let mut w = self.window;
        let (ww, wh) = (
            w.width().clamp(MIN_SIZE.0, area.width().max(MIN_SIZE.0)),
            w.height().clamp(MIN_SIZE.1, area.height().max(MIN_SIZE.1)),
        );
        w = IRect::sized(w.x0, w.y0, ww, wh);
        let dx = if w.x1 > area.x1 { area.x1 - w.x1 } else { 0 };
        let dy = if w.y1 > area.y1 { area.y1 - w.y1 } else { 0 };
        w = w.offset(dx, dy);
        let dx = (area.x0 - w.x0).max(0);
        let dy = (area.y0 - w.y0).max(0);
        self.window = w.offset(dx, dy);
    }

    /// The window's place, to open there next time (not maximized).
    #[must_use]
    pub fn placement(&self) -> IRect {
        self.restore.unwrap_or(self.window)
    }

    /// Whether `p` is on the window (or its Index dialog).
    #[must_use]
    pub fn contains(&self, p: (i32, i32)) -> bool {
        self.window.contains(p.0, p.1) || self.index.as_ref().is_some_and(|d| d.window.contains(p.0, p.1))
    }

    fn frame(&self) -> i32 {
        if self.restore.is_some() { 0 } else { FRAME }
    }

    /// The client area: below the caption, inside the frame.
    fn client(&self) -> IRect {
        let f = self.frame();
        IRect::new(
            self.window.x0 + f,
            self.window.y0 + f + CAPTION_H + 1,
            self.window.x1 - f,
            self.window.y1 - f,
        )
    }

    fn cx<'a>(&'a self, assets: &'a Assets) -> Cx<'a> {
        Cx {
            assets,
            file: &self.file,
            palette: &self.palette,
            pictures: &self.pictures,
            green: self.green,
        }
    }

    /// The topic laid out for the window's size.
    fn view(&self, assets: &Assets) -> Rc<View> {
        let c = self.client();
        let key = (self.topic, c.width(), c.height());
        if let Some(v) = self.view.borrow().as_ref()
            && v.key == key
        {
            return v.clone();
        }
        let cx = self.cx(assets);
        let topic = &self.file.topics[self.topic];
        let nsr = cx.lay_out(&topic.non_scroll, c.width());
        let nsr_h = if topic.non_scroll.is_empty() { 0 } else { nsr.height + 1 };
        let room = (c.height() - BAR_H - nsr_h).max(0);
        // With a vertical bar if it's too high, a horizontal one if too wide
        // (which may make it too high in turn).
        let mut width = c.width();
        let mut body = cx.lay_out(&topic.scroll, width);
        let mut vbar = body.height > room;
        if vbar {
            width -= SB;
            body = cx.lay_out(&topic.scroll, width);
        }
        let hbar = body.width > width;
        if hbar && !vbar && body.height > room - SB {
            vbar = true;
            width -= SB;
            body = cx.lay_out(&topic.scroll, width);
        }
        let v = Rc::new(View {
            key,
            nsr,
            body,
            vbar,
            hbar,
        });
        *self.view.borrow_mut() = Some(v.clone());
        v
    }

    fn rects(&self, v: &View) -> Rects {
        let w = self.window;
        let f = self.frame();
        let caption = IRect::new(w.x0 + f, w.y0 + f, w.x1 - f, w.y0 + f + CAPTION_H);
        let close = IRect::sized(caption.x1 - 2 - BOX_W, caption.y0 + 2, BOX_W, BOX_H);
        let max = close.offset(-BOX_W - 2, 0);
        let min = max.offset(-BOX_W, 0);
        let c = self.client();
        let bw = BUTTONS.len();
        let mut buttons = Vec::with_capacity(bw);
        let mut x = c.x0 + 3;
        for _ in 0..bw {
            buttons.push(IRect::sized(x, c.y0 + 3, 60, BUTTON_H));
            x += 60 + BUTTON_GAP;
        }
        let top = c.y0 + BAR_H;
        let nsr_h = if self.file.topics[self.topic].non_scroll.is_empty() {
            0
        } else {
            v.nsr.height + 1
        };
        let nsr = IRect::new(c.x0, top, c.x1, top + nsr_h);
        let body = IRect::new(c.x0, nsr.y1, c.x1, c.y1);
        let pane = IRect::new(
            body.x0,
            body.y0,
            body.x1 - if v.vbar { SB } else { 0 },
            body.y1 - if v.hbar { SB } else { 0 },
        );
        Rects {
            caption,
            close,
            max,
            min,
            buttons,
            nsr,
            pane,
            vbar: v.vbar.then(|| IRect::new(pane.x1, body.y0, body.x1, pane.y1)),
            hbar: v.hbar.then(|| IRect::new(body.x0, pane.y1, pane.x1, body.y1)),
        }
    }

    fn bars(&self, v: &View, r: &Rects) -> (Option<ScrollBar>, Option<ScrollBar>) {
        let vb = r.vbar.map(|rect| ScrollBar {
            rect,
            vertical: true,
            pos: self.scroll.1,
            total: v.body.height,
            page: r.pane.height(),
        });
        let hb = r.hbar.map(|rect| ScrollBar {
            rect,
            vertical: false,
            pos: self.scroll.0,
            total: v.body.width,
            page: r.pane.width(),
        });
        (vb, hb)
    }

    /// Scrolls to `(x, y)`, kept in range.
    fn scroll_to(&mut self, assets: &Assets, x: i32, y: i32) {
        let v = self.view(assets);
        let r = self.rects(&v);
        let max_y = (v.body.height - r.pane.height()).max(0);
        let max_x = if v.hbar {
            (v.body.width - r.pane.width()).max(0)
        } else {
            0
        };
        self.scroll = (x.clamp(0, max_x), y.clamp(0, max_y));
    }

    fn enabled(&self, b: usize) -> bool {
        let t = &self.file.topics[self.topic];
        match b {
            B_BACK => !self.history.is_empty(),
            B_PREV => t.browse_prev.is_some(),
            B_NEXT => t.browse_next.is_some(),
            // Nothing to print to.
            B_PRINT => false,
            _ => true,
        }
    }

    /// Shows topic `i` at its top, remembering the one shown for Back.
    fn go(&mut self, i: usize) {
        if i >= self.file.topics.len() {
            return;
        }
        if i != self.topic {
            self.history.push(self.topic);
        }
        self.show(i);
    }

    fn show(&mut self, i: usize) {
        self.topic = i;
        self.scroll = (0, 0);
        self.focus = None;
    }

    fn topic_index(&self, offset: u32) -> Option<usize> {
        let t = self.file.topic_at(offset)?;
        self.file.topics.iter().position(|u| u.offset == t.offset)
    }

    /// `HELP_CONTENTS` again: the contents topic, and the keyboard.
    pub fn contents(&mut self) {
        self.active = true;
        if let Some(i) = self.topic_index(self.file.contents) {
            self.go(i);
        }
    }

    /// Back: the last topic left.
    pub fn back(&mut self) {
        if let Some(i) = self.history.pop() {
            self.show(i);
        }
    }

    /// `<<` or `>>`.
    pub fn browse(&mut self, next: bool) {
        let t = &self.file.topics[self.topic];
        if let Some(i) = (if next { t.browse_next } else { t.browse_prev }).and_then(|o| self.topic_index(o)) {
            self.go(i);
        }
    }

    /// Follows link `n` of the topic shown.
    fn follow(&mut self, assets: &Assets, n: usize) {
        let v = self.view(assets);
        let action = v.body.links.get(n).or_else(|| v.nsr.links.get(n)).cloned();
        // Popups and macros: not in `WIDGET.HLP`; a popup shows as a jump.
        if let Some(LinkAction::Jump(h) | LinkAction::Popup(h)) = action
            && let Some(t) = self.file.topic_for_hash(h)
            && let Some(i) = self.file.topics.iter().position(|u| u.offset == t.offset)
        {
            self.go(i);
        }
    }

    fn press_button(&mut self, b: usize) {
        if !self.enabled(b) {
            return;
        }
        match b {
            B_CONTENTS => self.contents(),
            B_INDEX => self.index = Some(IndexDialog::new(&self.file, self.window)),
            B_BACK => self.back(),
            B_PREV => self.browse(false),
            B_NEXT => self.browse(true),
            _ => {}
        }
    }

    /// Whether `p` is on a sizing edge: left, top, right, bottom flags.
    fn edges_at(&self, p: (i32, i32)) -> u8 {
        if self.restore.is_some() {
            return 0;
        }
        let w = self.window;
        let corner = 16;
        let mut e = 0;
        if p.0 < w.x0 + FRAME || (p.0 < w.x0 + corner && (p.1 < w.y0 + FRAME || p.1 >= w.y1 - FRAME)) {
            e |= 1;
        }
        if p.1 < w.y0 + FRAME || (p.1 < w.y0 + corner && (p.0 < w.x0 + FRAME || p.0 >= w.x1 - FRAME)) {
            e |= 2;
        }
        if p.0 >= w.x1 - FRAME || (p.0 >= w.x1 - corner && (p.1 < w.y0 + FRAME || p.1 >= w.y1 - FRAME)) {
            e |= 4;
        }
        if p.1 >= w.y1 - FRAME || (p.1 >= w.y1 - corner && (p.0 < w.x0 + FRAME || p.0 >= w.x1 - FRAME)) {
            e |= 8;
        }
        e
    }

    fn toggle_max(&mut self, area: IRect) {
        if let Some(r) = self.restore.take() {
            self.window = r;
        } else {
            self.restore = Some(self.window);
            self.window = area;
        }
        self.fit(area);
    }

    /// The mouse went down at `p` (frame coordinates); `area` is where the
    /// window may be.
    pub fn mouse_down(&mut self, assets: &Assets, area: IRect, p: (i32, i32), dbl: bool) -> Outcome {
        self.active = true;
        self.mouse = p;
        if let Some(d) = &mut self.index {
            match d.mouse_down(assets, p, dbl) {
                IndexOutcome::Open => {}
                IndexOutcome::Cancel => self.index = None,
                IndexOutcome::Display(off) => {
                    self.index = None;
                    if let Some(i) = self.topic_index(off) {
                        self.go(i);
                    }
                }
            }
            return Outcome::Open;
        }
        let v = self.view(assets);
        let r = self.rects(&v);
        let press = if r.close.contains(p.0, p.1) {
            Press::Close
        } else if r.max.contains(p.0, p.1) {
            Press::Max
        } else if r.min.contains(p.0, p.1) {
            // Minimize is greyed: there's no task bar to restore it from.
            Press::Other
        } else if r.caption.contains(p.0, p.1) {
            if dbl {
                self.toggle_max(area);
                Press::Other
            } else if self.restore.is_some() {
                Press::Other
            } else {
                Press::Caption((p.0 - self.window.x0, p.1 - self.window.y0))
            }
        } else if let Some(i) = r.buttons.iter().position(|b| b.contains(p.0, p.1)) {
            if self.enabled(i) {
                Press::Button(i)
            } else {
                Press::Other
            }
        } else if self.edges_at(p) != 0 {
            Press::Size(self.edges_at(p), p, self.window)
        } else {
            let (vb, hb) = self.bars(&v, &r);
            if let Some((b, part)) = vb
                .iter()
                .chain(hb.iter())
                .find_map(|b| b.part_at(p).map(|part| (*b, part)))
            {
                let grab = if b.vertical {
                    p.1 - b.thumb().y0
                } else {
                    p.0 - b.thumb().x0
                };
                self.bar_step(assets, &b, part);
                self.repeat_at = 0;
                Press::Bar(b.vertical, part, grab)
            } else if r.pane.contains(p.0, p.1)
                && let Some(l) = v
                    .body
                    .hot_at((p.0 - r.pane.x0 + self.scroll.0, p.1 - r.pane.y0 + self.scroll.1))
            {
                Press::Hot(l)
            } else {
                Press::Other
            }
        };
        self.pressed = Some(press);
        Outcome::Open
    }

    /// One step of a scroll bar's part.
    fn bar_step(&mut self, assets: &Assets, b: &ScrollBar, part: Part) {
        let d = match part {
            Part::Less => -LINE,
            Part::More => LINE,
            Part::PageLess => -(b.page - LINE).max(LINE),
            Part::PageMore => (b.page - LINE).max(LINE),
            Part::Thumb => 0,
        };
        let (x, y) = self.scroll;
        if b.vertical {
            self.scroll_to(assets, x, y + d);
        } else {
            self.scroll_to(assets, x + d, y);
        }
    }

    pub fn mouse_move(&mut self, assets: &Assets, area: IRect, p: (i32, i32)) {
        self.mouse = p;
        if let Some(d) = &mut self.index {
            d.mouse_move(p);
            return;
        }
        match self.pressed {
            Some(Press::Caption(grab)) => {
                // The caption stays on the desktop.
                let w = self.window;
                let x = (p.0 - grab.0).clamp(area.x0 - w.width() + 40, area.x1 - 40);
                let y = (p.1 - grab.1).clamp(area.y0, area.y1 - CAPTION_H - FRAME);
                self.window = IRect::sized(x, y, w.width(), w.height());
            }
            Some(Press::Size(e, from, w0)) => {
                let (dx, dy) = (p.0 - from.0, p.1 - from.1);
                let mut w = w0;
                if e & 1 != 0 {
                    w.x0 = (w0.x0 + dx).min(w0.x1 - MIN_SIZE.0).max(area.x0);
                }
                if e & 2 != 0 {
                    w.y0 = (w0.y0 + dy).min(w0.y1 - MIN_SIZE.1).max(area.y0);
                }
                if e & 4 != 0 {
                    w.x1 = (w0.x1 + dx).max(w0.x0 + MIN_SIZE.0).min(area.x1);
                }
                if e & 8 != 0 {
                    w.y1 = (w0.y1 + dy).max(w0.y0 + MIN_SIZE.1).min(area.y1);
                }
                self.window = w;
                let (x, y) = self.scroll;
                self.scroll_to(assets, x, y);
            }
            Some(Press::Bar(vertical, Part::Thumb, grab)) => {
                let v = self.view(assets);
                let r = self.rects(&v);
                let (vb, hb) = self.bars(&v, &r);
                if let Some(b) = if vertical { vb } else { hb } {
                    let pos = b.pos_for(if vertical { p.1 } else { p.0 } - grab);
                    let (x, y) = self.scroll;
                    if vertical {
                        self.scroll_to(assets, x, pos);
                    } else {
                        self.scroll_to(assets, pos, y);
                    }
                }
            }
            _ => {}
        }
    }

    /// Repeats a held scroll arrow or page area, as Windows does.
    pub fn tick(&mut self, assets: &Assets, now: u64) {
        let Some(Press::Bar(vertical, part, _)) = self.pressed else {
            return;
        };
        if part == Part::Thumb {
            return;
        }
        if self.repeat_at == 0 {
            self.repeat_at = now + 400;
            return;
        }
        if now < self.repeat_at {
            return;
        }
        self.repeat_at = now + 50;
        let v = self.view(assets);
        let r = self.rects(&v);
        let (vb, hb) = self.bars(&v, &r);
        if let Some(b) = if vertical { vb } else { hb }
            && b.part_at(self.mouse) == Some(part)
        {
            self.bar_step(assets, &b, part);
        }
    }

    pub fn mouse_up(&mut self, assets: &Assets, area: IRect, p: (i32, i32)) -> Outcome {
        if let Some(d) = &mut self.index {
            match d.mouse_up(p) {
                IndexOutcome::Open => {}
                IndexOutcome::Cancel => self.index = None,
                IndexOutcome::Display(off) => {
                    self.index = None;
                    if let Some(i) = self.topic_index(off) {
                        self.go(i);
                    }
                }
            }
            return Outcome::Open;
        }
        let Some(press) = self.pressed.take() else {
            return Outcome::Open;
        };
        let v = self.view(assets);
        let r = self.rects(&v);
        match press {
            Press::Close if r.close.contains(p.0, p.1) => return Outcome::Closed,
            Press::Max if r.max.contains(p.0, p.1) => self.toggle_max(area),
            Press::Button(b) if r.buttons[b].contains(p.0, p.1) => self.press_button(b),
            Press::Hot(l)
                if r.pane.contains(p.0, p.1)
                    && v.body
                        .hot_at((p.0 - r.pane.x0 + self.scroll.0, p.1 - r.pane.y0 + self.scroll.1))
                        == Some(l) =>
            {
                self.follow(assets, l);
            }
            _ => {}
        }
        Outcome::Open
    }

    /// The wheel: three lines a notch.
    pub fn wheel(&mut self, assets: &Assets, dy: i32) {
        if let Some(d) = &mut self.index {
            d.wheel(dy);
            return;
        }
        let (x, y) = self.scroll;
        self.scroll_to(assets, x, y - dy.signum() * 3 * LINE);
    }

    /// A key while the window has the keyboard. `alt` with a letter is a
    /// button's mnemonic, as is the letter alone (the button bar takes
    /// them without Alt).
    pub fn key(&mut self, assets: &Assets, vk: u16, shift: bool) -> Outcome {
        if let Some(d) = &mut self.index {
            match d.key(vk, shift) {
                IndexOutcome::Open => {}
                IndexOutcome::Cancel => self.index = None,
                IndexOutcome::Display(off) => {
                    self.index = None;
                    if let Some(i) = self.topic_index(off) {
                        self.go(i);
                    }
                }
            }
            return Outcome::Open;
        }
        let v = self.view(assets);
        let r = self.rects(&v);
        let (x, y) = self.scroll;
        let page = (r.pane.height() - LINE).max(LINE);
        match vk {
            VK_ESCAPE => return Outcome::Closed,
            VK_UP => self.scroll_to(assets, x, y - LINE),
            VK_DOWN => self.scroll_to(assets, x, y + LINE),
            VK_LEFT => self.scroll_to(assets, x - LINE, y),
            VK_RIGHT => self.scroll_to(assets, x + LINE, y),
            VK_PRIOR => self.scroll_to(assets, x, y - page),
            VK_NEXT => self.scroll_to(assets, x, y + page),
            VK_HOME => self.scroll_to(assets, 0, 0),
            VK_END => self.scroll_to(assets, 0, i32::MAX),
            VK_BACK => self.back(),
            VK_TAB => {
                // The hotspots in turn (WinHelp 4), shown scrolled to.
                let n = v.body.links.len();
                if n > 0 {
                    self.focus = Some(match (self.focus, shift) {
                        (None, false) => 0,
                        (None, true) => n - 1,
                        (Some(f), false) => (f + 1) % n,
                        (Some(f), true) => (f + n - 1) % n,
                    });
                    if let Some(h) = v.body.hots.iter().find(|h| Some(h.link) == self.focus) {
                        let ph = r.pane.height();
                        if h.rect.y0 < y || h.rect.y1 > y + ph {
                            self.scroll_to(assets, x, h.rect.y0 - LINE);
                        }
                    }
                }
            }
            VK_RETURN => {
                if let Some(f) = self.focus {
                    self.follow(assets, f);
                }
            }
            0xbc if shift => self.press_button(B_PREV),
            0xbe if shift => self.press_button(B_NEXT),
            _ => {
                let c = char::from(u8::try_from(vk).unwrap_or(0));
                if let Some(b) = BUTTONS.iter().position(|b| ui::mnemonic(b) == Some(c)) {
                    self.press_button(b);
                }
            }
        }
        Outcome::Open
    }

    /// The pointer over `p`: the hand over a hotspot.
    #[must_use]
    pub fn pointer(&self, assets: &Assets, p: (i32, i32)) -> Pointer {
        if self.index.is_some() || self.pressed.is_some() {
            return Pointer::Arrow;
        }
        let v = self.view(assets);
        let r = self.rects(&v);
        if r.pane.contains(p.0, p.1)
            && v.body
                .hot_at((p.0 - r.pane.x0 + self.scroll.0, p.1 - r.pane.y0 + self.scroll.1))
                .is_some()
        {
            return Pointer::Game(6);
        }
        Pointer::Arrow
    }

    /// The topic's hotspots as frame rectangles with their links, for
    /// tests: those (partly) in the pane.
    #[must_use]
    pub fn hotspots(&self, assets: &Assets) -> Vec<(IRect, String)> {
        let v = self.view(assets);
        let r = self.rects(&v);
        v.body
            .hots
            .iter()
            .map(|h| {
                (
                    h.rect.offset(r.pane.x0 - self.scroll.0, r.pane.y0 - self.scroll.1),
                    v.body
                        .links
                        .get(h.link)
                        .and_then(|l| match l {
                            LinkAction::Jump(h) | LinkAction::Popup(h) => self.file.topic_for_hash(*h),
                            LinkAction::Macro(_) => None,
                        })
                        .map(|t| t.title.clone())
                        .unwrap_or_default(),
                )
            })
            .filter(|(rect, _)| !rect.intersect(r.pane).is_empty())
            .collect()
    }

    /// A button's centre (frame coordinates), for tests.
    #[must_use]
    pub fn button_centre(&self, assets: &Assets, b: usize) -> (i32, i32) {
        let v = self.view(assets);
        let r = self.rects(&v);
        let b = r.buttons[b];
        (i32::midpoint(b.x0, b.x1), i32::midpoint(b.y0, b.y1))
    }

    /// The topic's text laid out at its current width: each piece's right
    /// end, for tests.
    #[must_use]
    pub fn pane_and_extents(&self, assets: &Assets) -> (i32, Vec<i32>) {
        let v = self.view(assets);
        let r = self.rects(&v);
        let ends = v
            .body
            .pieces
            .iter()
            .filter(|p| matches!(p.atom, Atom::Text { .. }))
            .map(|p| p.x + p.w)
            .collect();
        (r.pane.width(), ends)
    }

    pub fn draw(&self, s: &mut Surface, assets: &Assets) {
        let Some(font) = ui::font(assets) else { return };
        let v = self.view(assets);
        let r = self.rects(&v);
        let w = self.window;
        let old = s.clip();
        s.set_clip(w.intersect(old));
        s.fill(w, ui::FACE);
        if self.restore.is_none() {
            ui::raised(s, w);
        }
        // The caption: active or not.
        let (bg, fg) = if self.active {
            (ui::NAVY, ui::WHITE)
        } else {
            (ui::SHADOW, ui::FACE)
        };
        s.fill(r.caption, bg);
        help_icon(s, r.caption.x0 + 1, r.caption.y0 + 1);
        let tx = r.caption.x0 + 20;
        let cap_clip = s.clip();
        s.set_clip(IRect::new(tx, r.caption.y0, r.min.x0 - 2, r.caption.y1).intersect(cap_clip));
        draw_text(
            s,
            font,
            tx,
            r.caption.y0 + 2,
            &crate::text::encode(&self.title),
            fg,
            1,
            false,
        );
        s.set_clip(cap_clip);
        caption_box(s, font, r.min, 0, false, false);
        caption_box(
            s,
            font,
            r.max,
            if self.restore.is_some() { 2 } else { 1 },
            self.pressed == Some(Press::Max),
            true,
        );
        caption_box(s, font, r.close, 3, self.pressed == Some(Press::Close), true);
        // The button bar.
        for (i, (b, label)) in r.buttons.iter().zip(BUTTONS).enumerate() {
            let down = self.pressed == Some(Press::Button(i)) && b.contains(self.mouse.0, self.mouse.1);
            ui::button(s, font, *b, label, down, false, self.enabled(i));
        }
        // The non-scrolling region and its line.
        if !r.nsr.is_empty() {
            s.fill(r.nsr, ui::WHITE);
            let c = s.clip();
            s.set_clip(r.nsr.intersect(c));
            v.nsr.draw(s, &self.cx(assets), r.nsr.x0, r.nsr.y0);
            s.set_clip(c);
            s.fill(IRect::new(r.nsr.x0, r.nsr.y1 - 1, r.nsr.x1, r.nsr.y1), ui::BLACK);
        }
        // The topic.
        s.fill(r.pane, ui::WHITE);
        let c = s.clip();
        s.set_clip(r.pane.intersect(c));
        let (ox, oy) = (r.pane.x0 - self.scroll.0, r.pane.y0 - self.scroll.1);
        v.body.draw(s, &self.cx(assets), ox, oy);
        if self.active
            && let Some(f) = self.focus
        {
            for h in v.body.hots.iter().filter(|h| h.link == f) {
                focus_rect(s, h.rect.offset(ox, oy));
            }
        }
        s.set_clip(c);
        let (vb, hb) = self.bars(&v, &r);
        let part = |vertical: bool| match self.pressed {
            Some(Press::Bar(v, p, _)) if v == vertical => Some(p),
            _ => None,
        };
        if let Some(b) = vb {
            b.draw(s, font, part(true));
        }
        if let Some(b) = hb {
            b.draw(s, font, part(false));
        }
        if let (Some(a), Some(b)) = (r.vbar, r.hbar) {
            s.fill(IRect::new(a.x0, b.y0, a.x1, b.y1), ui::FACE);
        }
        s.set_clip(old);
        if let Some(d) = &self.index {
            d.draw(s, assets, &self.title);
        }
    }
}

/// A caption box: 0 minimize (greyed), 1 maximize, 2 restore, 3 close.
fn caption_box(s: &mut Surface, font: &Font, r: IRect, kind: u8, pressed: bool, enabled: bool) {
    ui::button(s, font, r, "", pressed, false, true);
    let (x, y) = (r.x0 + 4 + i32::from(pressed), r.y0 + 3 + i32::from(pressed));
    let c = if enabled { ui::BLACK } else { ui::SHADOW };
    match kind {
        0 => {
            if !enabled {
                s.fill(IRect::sized(x + 1, y + 8, 6, 2), ui::WHITE);
            }
            s.fill(IRect::sized(x, y + 7, 6, 2), c);
        }
        1 => {
            s.frame(IRect::sized(x - 1, y - 1, 9, 9), c);
            s.fill(IRect::sized(x - 1, y, 9, 1), c);
        }
        2 => {
            s.frame(IRect::sized(x + 1, y - 1, 6, 5), c);
            s.fill(IRect::sized(x + 1, y, 6, 1), c);
            s.fill(IRect::sized(x - 1, y + 2, 6, 5), ui::FACE);
            s.frame(IRect::sized(x - 1, y + 2, 6, 5), c);
            s.fill(IRect::sized(x - 1, y + 3, 6, 1), c);
        }
        _ => {
            for d in 0..6 {
                for t in 0..2 {
                    s.pset(x + d + t, y + d, c);
                    s.pset(x + 5 - d + t, y + d, c);
                }
            }
        }
    }
}

/// `WinHelp`'s small icon: a closed book with a question mark.
fn help_icon(s: &mut Surface, x: i32, y: i32) {
    const ROWS: [&str; 16] = [
        "................",
        "...kkkkkkkkkk...",
        "..kyyyyyyyyyk...",
        "..kyyykkkyyykw..",
        "..kyykyyykyykw..",
        "..kyyyyyykyykw..",
        "..kyyyyykyyykw..",
        "..kyyyykyyyykw..",
        "..kyyyykyyyykw..",
        "..kyyyyyyyyykw..",
        "..kyyyykyyyykw..",
        "..kyyyyyyyyykw..",
        "..kkkkkkkkkkkw..",
        "...kwwwwwwwwwk..",
        "....kkkkkkkkkk..",
        "................",
    ];
    for (dy, row) in (0..).zip(ROWS) {
        for (dx, c) in (0..).zip(row.bytes()) {
            let c = match c {
                b'k' => ui::BLACK,
                b'y' => 251,
                b'w' => ui::WHITE,
                _ => continue,
            };
            s.pset(x + dx, y + dy, c);
        }
    }
}

// ---- the Index --------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndexOutcome {
    Open,
    Cancel,
    /// Show the topic at this offset.
    Display(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IPress {
    Close,
    Display,
    Cancel,
    List,
    Bar(Part, i32),
}

/// `WinHelp` 4's "Help Topics" dialog with its Index tab: a field to type
/// the start of a keyword in, the keywords, Display and Cancel.
#[derive(Debug, Clone)]
struct IndexDialog {
    window: IRect,
    words: Vec<(String, u32)>,
    typed: String,
    sel: Option<usize>,
    top: usize,
    pressed: Option<IPress>,
}

const IDX_SIZE: (i32, i32) = (380, 400);
const ROW_H: i32 = 13;

impl IndexDialog {
    fn new(file: &HelpFile, over: IRect) -> IndexDialog {
        let mut words: Vec<(String, u32)> = file
            .keywords
            .iter()
            .filter_map(|k| Some((k.word.clone(), *k.topics.first()?)))
            .collect();
        words.sort_by_key(|(w, _)| w.to_ascii_lowercase());
        let (w, h) = IDX_SIZE;
        let window = IRect::sized(
            over.x0 + (over.width() - w) / 2,
            over.y0 + (over.height() - h) / 2,
            w,
            h,
        );
        IndexDialog {
            window,
            sel: (!words.is_empty()).then_some(0),
            words,
            typed: String::new(),
            top: 0,
            pressed: None,
        }
    }

    fn client(&self) -> IRect {
        let w = self.window;
        IRect::new(w.x0 + 3, w.y0 + 3 + CAPTION_H + 1, w.x1 - 3, w.y1 - 3)
    }

    fn close_box(&self) -> IRect {
        IRect::sized(self.window.x1 - 3 - 2 - BOX_W, self.window.y0 + 3 + 2, BOX_W, BOX_H)
    }

    /// The tab's page.
    fn page(&self) -> IRect {
        let c = self.client();
        IRect::new(c.x0 + 6, c.y0 + 6 + 20, c.x1 - 6, c.y1 - 6 - BUTTON_H - 8)
    }

    fn edit(&self) -> IRect {
        let p = self.page();
        IRect::new(p.x0 + 12, p.y0 + 30, p.x1 - 12, p.y0 + 30 + 21)
    }

    fn list(&self) -> IRect {
        let p = self.page();
        IRect::new(p.x0 + 12, p.y0 + 84, p.x1 - 12, p.y1 - 12)
    }

    fn display_button(&self) -> IRect {
        let c = self.client();
        IRect::sized(c.x1 - 6 - 2 * 75 - 6, c.y1 - 6 - BUTTON_H, 75, BUTTON_H)
    }

    fn cancel_button(&self) -> IRect {
        let c = self.client();
        IRect::sized(c.x1 - 6 - 75, c.y1 - 6 - BUTTON_H, 75, BUTTON_H)
    }

    fn rows(&self) -> usize {
        usize::try_from((self.list().height() - 4) / ROW_H).unwrap_or(1).max(1)
    }

    fn max_top(&self) -> usize {
        self.words.len().saturating_sub(self.rows())
    }

    fn bar(&self) -> Option<ScrollBar> {
        let l = self.list().inset(2);
        (self.words.len() > self.rows()).then(|| ScrollBar {
            rect: IRect::new(l.x1 - SB, l.y0, l.x1, l.y1),
            vertical: true,
            pos: len(self.top),
            total: len(self.words.len()),
            page: len(self.rows()),
        })
    }

    fn items(&self) -> IRect {
        let l = self.list().inset(2);
        if self.bar().is_some() {
            IRect::new(l.x0, l.y0, l.x1 - SB, l.y1)
        } else {
            l
        }
    }

    fn scroll_to(&mut self, top: usize) {
        self.top = top.min(self.max_top());
    }

    /// Selects line `i`, shows it and puts it in the field.
    fn select(&mut self, i: usize, fill: bool) {
        if self.words.is_empty() {
            return;
        }
        let i = i.min(self.words.len() - 1);
        self.sel = Some(i);
        if fill {
            self.typed.clone_from(&self.words[i].0);
        }
        let rows = self.rows();
        if i < self.top {
            self.top = i;
        } else if i >= self.top + rows {
            self.top = i + 1 - rows;
        }
    }

    /// Typing selects the first keyword that starts with the text.
    fn typed_changed(&mut self) {
        let t = self.typed.to_ascii_lowercase();
        let i = self
            .words
            .iter()
            .position(|(w, _)| w.to_ascii_lowercase().starts_with(&t))
            .or_else(|| self.words.iter().position(|(w, _)| w.to_ascii_lowercase() > t))
            .unwrap_or(self.words.len().saturating_sub(1));
        self.select(i, false);
        // The match shows at the top, as WinHelp's list does.
        self.scroll_to(i);
    }

    fn display(&self) -> IndexOutcome {
        self.sel
            .and_then(|i| self.words.get(i))
            .map_or(IndexOutcome::Open, |(_, off)| IndexOutcome::Display(*off))
    }

    fn line_at(&self, p: (i32, i32)) -> Option<usize> {
        let r = self.items();
        if !r.contains(p.0, p.1) {
            return None;
        }
        let i = self.top + usize::try_from((p.1 - r.y0) / ROW_H).ok()?;
        (i < self.words.len()).then_some(i)
    }

    fn mouse_down(&mut self, _assets: &Assets, p: (i32, i32), dbl: bool) -> IndexOutcome {
        if self.close_box().contains(p.0, p.1) {
            self.pressed = Some(IPress::Close);
        } else if self.display_button().contains(p.0, p.1) {
            self.pressed = Some(IPress::Display);
        } else if self.cancel_button().contains(p.0, p.1) {
            self.pressed = Some(IPress::Cancel);
        } else if let Some(b) = self.bar()
            && let Some(part) = b.part_at(p)
        {
            let rows = self.rows();
            match part {
                Part::Less => self.scroll_to(self.top.saturating_sub(1)),
                Part::More => self.scroll_to(self.top + 1),
                Part::PageLess => self.scroll_to(self.top.saturating_sub(rows)),
                Part::PageMore => self.scroll_to(self.top + rows),
                Part::Thumb => {}
            }
            self.pressed = Some(IPress::Bar(part, p.1 - b.thumb().y0));
        } else if let Some(i) = self.line_at(p) {
            self.select(i, true);
            if dbl {
                return self.display();
            }
            self.pressed = Some(IPress::List);
        }
        IndexOutcome::Open
    }

    fn mouse_move(&mut self, p: (i32, i32)) {
        match self.pressed {
            Some(IPress::List) => {
                if let Some(i) = self.line_at(p) {
                    self.select(i, true);
                }
            }
            Some(IPress::Bar(Part::Thumb, grab)) => {
                if let Some(b) = self.bar() {
                    let top = b.pos_for(p.1 - grab);
                    self.scroll_to(usize::try_from(top).unwrap_or(0));
                }
            }
            _ => {}
        }
    }

    fn mouse_up(&mut self, p: (i32, i32)) -> IndexOutcome {
        match self.pressed.take() {
            Some(IPress::Close) if self.close_box().contains(p.0, p.1) => IndexOutcome::Cancel,
            Some(IPress::Cancel) if self.cancel_button().contains(p.0, p.1) => IndexOutcome::Cancel,
            Some(IPress::Display) if self.display_button().contains(p.0, p.1) => self.display(),
            _ => IndexOutcome::Open,
        }
    }

    fn wheel(&mut self, dy: i32) {
        if dy > 0 {
            self.scroll_to(self.top.saturating_sub(3));
        } else if dy < 0 {
            self.scroll_to(self.top + 3);
        }
    }

    fn key(&mut self, vk: u16, shift: bool) -> IndexOutcome {
        let sel = self.sel.unwrap_or(0);
        let rows = self.rows();
        match vk {
            VK_RETURN => return self.display(),
            VK_ESCAPE => return IndexOutcome::Cancel,
            VK_UP => self.select(sel.saturating_sub(1), true),
            VK_DOWN => self.select(if self.sel.is_some() { sel + 1 } else { 0 }, true),
            VK_PRIOR => self.select(sel.saturating_sub(rows - 1), true),
            VK_NEXT => self.select(sel + rows - 1, true),
            VK_BACK => {
                self.typed.pop();
                self.typed_changed();
            }
            _ => {
                if let Some(c) = super::prompt::typed(vk, shift) {
                    self.typed.push(c);
                    self.typed_changed();
                }
            }
        }
        IndexOutcome::Open
    }

    fn draw(&self, s: &mut Surface, assets: &Assets, title: &str) {
        let Some(font) = ui::font(assets) else { return };
        super::dialog::draw_window(
            s,
            font,
            self.window,
            &format!("Help Topics: {title}"),
            self.pressed == Some(IPress::Close),
        );
        // The tab control: one tab, "Index", raised above its page.
        let page = self.page();
        let tab_w = ui::width(font, "Index") + 16;
        let tab = IRect::new(page.x0, page.y0 - 20, page.x0 + tab_w, page.y0 + 1);
        s.fill(tab, ui::FACE);
        s.fill(IRect::new(page.x0, page.y0, page.x1, page.y1), ui::FACE);
        ui::edge(s, page, ui::WHITE, ui::BLACK);
        ui::edge(s, page.inset(1), ui::FACE, ui::SHADOW);
        s.fill(IRect::new(tab.x0, tab.y0 + 2, tab.x0 + 1, page.y0 + 1), ui::WHITE);
        s.fill(IRect::new(tab.x0 + 2, tab.y0, tab.x1 - 2, tab.y0 + 1), ui::WHITE);
        s.pset(tab.x0 + 1, tab.y0 + 1, ui::WHITE);
        s.fill(IRect::new(tab.x1 - 1, tab.y0 + 2, tab.x1, page.y0), ui::BLACK);
        s.fill(IRect::new(tab.x1 - 2, tab.y0 + 1, tab.x1 - 1, page.y0), ui::SHADOW);
        s.fill(IRect::new(tab.x0 + 1, page.y0, tab.x1 - 2, page.y0 + 2), ui::FACE);
        ui::label(s, font, tab.x0 + 8, tab.y0 + 4, "I&ndex", ui::BLACK);
        let e = self.edit();
        ui::label(
            s,
            font,
            e.x0,
            e.y0 - 18,
            "&1  Type the first few letters of the word you're looking for.",
            ui::BLACK,
        );
        s.fill(e, ui::WHITE);
        ui::sunken(s, e);
        let tw = s.text(font, e.x0 + 4, e.y0 + 4, &crate::text::encode(&self.typed), ui::BLACK);
        s.fill(IRect::sized(e.x0 + 4 + tw, e.y0 + 3, 1, 15), ui::BLACK);
        let l = self.list();
        ui::label(
            s,
            font,
            l.x0,
            l.y0 - 18,
            "&2  Click the index entry you want, and then click Display.",
            ui::BLACK,
        );
        s.fill(l, ui::WHITE);
        ui::sunken(s, l);
        let items = self.items();
        let old = s.clip();
        s.set_clip(items.intersect(old));
        for (row, i) in (self.top..self.words.len()).take(self.rows()).enumerate() {
            let y = items.y0 + len(row) * ROW_H;
            let r = IRect::new(items.x0, y, items.x1, y + ROW_H);
            let hot = self.sel == Some(i);
            if hot {
                s.fill(
                    IRect::new(r.x0, r.y0, r.x0 + 4 + ui::width(font, &self.words[i].0) + 2, r.y1),
                    ui::NAVY,
                );
            }
            let c = if hot { ui::WHITE } else { ui::BLACK };
            s.text(font, r.x0 + 3, y, &crate::text::encode(&self.words[i].0), c);
        }
        s.set_clip(old);
        if let Some(b) = self.bar() {
            let part = match self.pressed {
                Some(IPress::Bar(p, _)) => Some(p),
                _ => None,
            };
            b.draw(s, font, part);
        }
        ui::button(
            s,
            font,
            self.display_button(),
            "&Display",
            self.pressed == Some(IPress::Display),
            true,
            self.sel.is_some(),
        );
        ui::button(
            s,
            font,
            self.cancel_button(),
            "Cancel",
            self.pressed == Some(IPress::Cancel),
            false,
            true,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_tells_the_formats_apart() {
        // Neither parses, but each goes to its own reader.
        assert!(formats::inf::is_inf(b"HSP\x01"));
        assert!(!formats::inf::is_inf(b"?_\x03\0"));
        assert_eq!(load(b"HSP\x01"), None);
        assert_eq!(load(b"?_\x03\0"), None);
    }

    #[test]
    fn units_are_two_thirds_of_a_pixel() {
        assert_eq!(px(72), 48);
        assert_eq!(px(-35), -23);
        assert_eq!(px(36), 24);
        assert_eq!(px(0), 0);
    }

    #[test]
    fn scroll_bar_parts() {
        let b = ScrollBar {
            rect: IRect::new(0, 0, SB, 200),
            vertical: true,
            pos: 0,
            total: 400,
            page: 200,
        };
        assert_eq!(b.part_at((5, 3)), Some(Part::Less));
        assert_eq!(b.part_at((5, 195)), Some(Part::More));
        assert_eq!(b.thumb(), IRect::new(0, SB, SB, SB + 84));
        assert_eq!(b.part_at((5, 20)), Some(Part::Thumb));
        assert_eq!(b.part_at((5, 150)), Some(Part::PageMore));
        assert_eq!(b.pos_for(SB + 84), 200);
    }
}
