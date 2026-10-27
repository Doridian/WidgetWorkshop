//! The look of what the platform has no native version of: the menu bar,
//! message boxes and the settings dialogs' controls. Drawn like the game's
//! own toolbars (sheet 1, `WWGADGET` 1004) rather than like Windows: their
//! darker grey face and bevels, near-black wells, and the recessed blue of
//! their move bars for captions and selections.

use formats::fnt::Font;

use crate::assets::Assets;
use crate::gfx::{IRect, Surface};

pub const BLACK: u8 = 0;
/// The toolbars' face: RGB(159,159,159).
pub const FACE: u8 = 16;
/// Their lit edge: RGB(207,207,207).
pub const LIGHT: u8 = 13;
/// Their shaded edge, and greyed text: RGB(115,115,115).
pub const SHADOW: u8 = 34;
/// The inner rim of their wells: RGB(23,23,23).
pub const DARK: u8 = 26;
pub const WHITE: u8 = 255;
/// The move bars' blue, also the selection's: RGB(0,67,171).
pub const BLUE: u8 = 17;
/// The move bars' lit bottom and right edge: RGB(0,99,254).
pub const BLUE_LIT: u8 = 40;

/// The system font of dialogs and menus: MS Sans Serif 8 pt (13 px).
#[must_use]
pub fn font(assets: &Assets) -> Option<&Font> {
    assets.font("MS Sans Serif", 13, false)
}

/// A window's edge, as round a toolbar: lit on the top and left, shaded
/// on the bottom and right, with a dark outline so it stands off the
/// worksheet.
pub fn raised(s: &mut Surface, r: IRect) {
    s.frame(r, DARK);
    edge(s, r.inset(1), LIGHT, SHADOW);
}

/// A well, as the toolbars' buttons sit in: grey and lit outside, near
/// black and face inside.
pub fn sunken(s: &mut Surface, r: IRect) {
    edge(s, r, SHADOW, LIGHT);
    edge(s, r.inset(1), DARK, FACE);
}

/// A caption, drawn like a toolbar's move bar: a recessed blue strip,
/// black on the top and left, bright blue on the bottom and right. An
/// inactive one is grey.
pub fn caption(s: &mut Surface, r: IRect, active: bool) {
    let (fill, lit) = if active { (BLUE, BLUE_LIT) } else { (SHADOW, FACE) };
    s.fill(r, fill);
    edge(s, r, BLACK, lit);
}

/// A one-pixel edge: `tl` on the top and left, `br` on the bottom and right.
pub fn edge(s: &mut Surface, r: IRect, tl: u8, br: u8) {
    if r.is_empty() {
        return;
    }
    s.fill(IRect::new(r.x0, r.y0, r.x1, r.y0 + 1), tl);
    s.fill(IRect::new(r.x0, r.y0, r.x0 + 1, r.y1), tl);
    s.fill(IRect::new(r.x0, r.y1 - 1, r.x1, r.y1), br);
    s.fill(IRect::new(r.x1 - 1, r.y0, r.x1, r.y1), br);
}

/// `text` without its `&` mnemonic markers, and the index of the
/// underlined character.
#[must_use]
pub fn strip_mnemonic(text: &str) -> (String, Option<usize>) {
    let mut out = String::new();
    let mut under = None;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '&' {
            match chars.next() {
                Some('&') => out.push('&'),
                Some(n) => {
                    under = Some(out.chars().count());
                    out.push(n);
                }
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    (out, under)
}

/// The mnemonic letter of `text`, upper case.
#[must_use]
pub fn mnemonic(text: &str) -> Option<char> {
    let (s, u) = strip_mnemonic(text);
    s.chars().nth(u?).map(|c| c.to_ascii_uppercase())
}

/// The width of `text` in `font`.
#[must_use]
pub fn width(font: &Font, text: &str) -> i32 {
    i32::try_from(font.width(&crate::text::encode(text))).unwrap_or(0)
}

/// Draws `text` with its mnemonic underlined; returns its width.
pub fn label(s: &mut Surface, font: &Font, x: i32, y: i32, text: &str, c: u8) -> i32 {
    let (plain, under) = strip_mnemonic(text);
    let bytes = crate::text::encode(&plain);
    let w = s.text(font, x, y, &bytes, c);
    if let Some(u) = under
        && u < bytes.len()
    {
        let x0 = x + i32::try_from(font.width(&bytes[..u])).unwrap_or(0);
        let x1 = x + i32::try_from(font.width(&bytes[..=u])).unwrap_or(0);
        let base = y + i32::from(font.ascent) + 1;
        s.fill(IRect::new(x0, base, x1, base + 1), c);
    }
    w
}

/// A greyed label: the lit edge's grey one pixel down and right, then the
/// shaded one (embossed disabled text).
pub fn grey_label(s: &mut Surface, font: &Font, x: i32, y: i32, text: &str) {
    label(s, font, x + 1, y + 1, text, LIGHT);
    label(s, font, x, y, text, SHADOW);
}

/// Splits `text` into lines that fit `w` pixels, breaking at spaces and
/// at newlines (`DrawText(DT_WORDBREAK)`).
#[must_use]
pub fn wrap(font: &Font, text: &str, w: i32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let para = para.trim_end_matches('\r');
        let mut line = String::new();
        for word in para.split(' ') {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if width(font, &candidate) > w && !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                word.clone_into(&mut line);
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines
}

/// A push button, like the toolbars' tiles: a near-black outline round a
/// bevel, pressed in when held, with its caption centred; a default button
/// has a blue ring inside its outline.
pub fn button(s: &mut Surface, font: &Font, r: IRect, text: &str, pressed: bool, default: bool, enabled: bool) {
    s.fill(r, FACE);
    s.frame(r, DARK);
    let mut b = r.inset(1);
    if default {
        s.frame(b, BLUE);
        b = b.inset(1);
    }
    if pressed {
        edge(s, b, SHADOW, FACE);
    } else {
        edge(s, b, LIGHT, SHADOW);
    }
    let (plain, _) = strip_mnemonic(text);
    let tw = width(font, &plain);
    let off = i32::from(pressed);
    let x = r.x0 + (r.width() - tw) / 2 + off;
    let y = r.y0 + (r.height() - i32::from(font.height)) / 2 + off;
    if enabled {
        label(s, font, x, y, text, BLACK);
    } else {
        grey_label(s, font, x, y, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mnemonics() {
        assert_eq!(strip_mnemonic("E&xit"), ("Exit".to_owned(), Some(1)));
        assert_eq!(strip_mnemonic("A && B"), ("A & B".to_owned(), None));
        assert_eq!(mnemonic("&New"), Some('N'));
        assert_eq!(mnemonic("Revert"), None);
    }
}
