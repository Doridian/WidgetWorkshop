//! Windows 95's look, for what the platform has no native version of: the
//! menu bar, message boxes and the settings dialogs' controls. Drawn with
//! the system colours of the 8-bit palette (the 20 static entries).

use formats::fnt::Font;

use crate::assets::Assets;
use crate::gfx::{IRect, Surface};

pub const BLACK: u8 = 0;
/// `COLOR_3DFACE`, `COLOR_MENU`: RGB(192,192,192).
pub const FACE: u8 = 7;
/// `COLOR_3DSHADOW`, `COLOR_GRAYTEXT`: RGB(128,128,128).
pub const SHADOW: u8 = 248;
/// `COLOR_3DHILIGHT`: white.
pub const LIGHT: u8 = 255;
pub const WHITE: u8 = 255;
/// `COLOR_HIGHLIGHT`, `COLOR_ACTIVECAPTION`: RGB(0,0,128).
pub const NAVY: u8 = 4;

/// The system font of dialogs and menus: MS Sans Serif 8 pt (13 px).
#[must_use]
pub fn font(assets: &Assets) -> Option<&Font> {
    assets.font("MS Sans Serif", 13, false)
}

/// A raised edge (`DrawEdge(EDGE_RAISED)`): light grey and black outside,
/// white and grey inside.
pub fn raised(s: &mut Surface, r: IRect) {
    edge(s, r, FACE, BLACK);
    edge(s, r.inset(1), LIGHT, SHADOW);
}

/// A sunken edge (`EDGE_SUNKEN`): grey and white outside, black and light
/// grey inside.
pub fn sunken(s: &mut Surface, r: IRect) {
    edge(s, r, SHADOW, LIGHT);
    edge(s, r.inset(1), BLACK, FACE);
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

/// A greyed label: white one pixel down and right, then grey (Win95's
/// embossed disabled text).
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

/// A push button: raised (or pressed) with its caption centred; a default
/// button has a black frame around.
pub fn button(s: &mut Surface, font: &Font, r: IRect, text: &str, pressed: bool, default: bool, enabled: bool) {
    s.fill(r, FACE);
    let mut b = r;
    if default {
        s.frame(r, BLACK);
        b = r.inset(1);
    }
    if pressed {
        s.frame(b, SHADOW);
    } else {
        edge(s, b, LIGHT, BLACK);
        edge(s, b.inset(1), FACE, SHADOW);
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
