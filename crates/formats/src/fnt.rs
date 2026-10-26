//! Windows bitmap fonts: the `FNT` format (versions 2 and 3), as `RT_FONT`
//! resources in a `.FON` (a resource-only NE file). The game asks Windows
//! for "MS Sans Serif" and "Small Fonts"; the CD carries both in its
//! Windows 3.1 folder (`\WIN31\SVGA\SSERIFE.FON` and `SMALLE.FON`, the
//! VGA sizes).

use std::fmt;

use crate::ne;

/// `RT_FONT`.
pub const RT_FONT: u16 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    NotFnt,
    OutOfBounds(usize),
    Ne(ne::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFnt => f.write_str("not a Windows bitmap font"),
            Error::OutOfBounds(at) => write!(f, "data at {at:#x} lies outside the font"),
            Error::Ne(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

/// One glyph: `width` × the font's height, row-major, `true` for ink.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Glyph {
    pub width: u16,
    pub bits: Vec<bool>,
}

/// A bitmap font of one size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Font {
    pub face: String,
    pub points: u16,
    pub height: u16,
    pub ascent: u16,
    pub weight: u16,
    pub italic: bool,
    pub first: u8,
    pub default_char: u8,
    /// From `first` on.
    pub glyphs: Vec<Glyph>,
}

fn u16_at(d: &[u8], at: usize) -> Result<u16, Error> {
    d.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or(Error::OutOfBounds(at))
}

fn u32_at(d: &[u8], at: usize) -> Result<u32, Error> {
    d.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(Error::OutOfBounds(at))
}

impl Font {
    /// Reads one `FNT` resource.
    ///
    /// # Errors
    /// If it isn't version 2 or 3, or its tables lie outside it.
    pub fn parse(d: &[u8]) -> Result<Font, Error> {
        let version = u16_at(d, 0)?;
        if version != 0x200 && version != 0x300 {
            return Err(Error::NotFnt);
        }
        let byte = |at: usize| d.get(at).copied().ok_or(Error::OutOfBounds(at));
        let points = u16_at(d, 68)?;
        let ascent = u16_at(d, 74)?;
        let italic = byte(80)? != 0;
        let weight = u16_at(d, 83)?;
        let height = u16_at(d, 88)?;
        let first = byte(95)?;
        let last = byte(96)?;
        let default_char = byte(97)?.wrapping_add(first);
        let face_at = u32_at(d, 105)? as usize;
        let face_end = d[face_at.min(d.len())..]
            .iter()
            .position(|&c| c == 0)
            .map_or(d.len(), |p| face_at + p);
        let face = String::from_utf8_lossy(d.get(face_at..face_end).unwrap_or_default()).into_owned();
        let (table, entry) = if version == 0x200 { (118, 4) } else { (148, 6) };
        let count = usize::from(last.saturating_sub(first)) + 1;
        let rows = usize::from(height);
        let mut glyphs = Vec::with_capacity(count);
        for i in 0..count {
            let at = table + i * entry;
            let width = u16_at(d, at)?;
            let off = if version == 0x200 {
                usize::from(u16_at(d, at + 2)?)
            } else {
                u32_at(d, at + 2)? as usize
            };
            let w = usize::from(width);
            let mut bits = vec![false; w * rows];
            // Column-major in strips 8 pixels wide, each strip `height` bytes.
            for strip in 0..w.div_ceil(8) {
                for y in 0..rows {
                    let b = byte(off + strip * rows + y)?;
                    for bit in 0..8 {
                        let x = strip * 8 + bit;
                        if x < w && b & (0x80 >> bit) != 0 {
                            bits[y * w + x] = true;
                        }
                    }
                }
            }
            glyphs.push(Glyph { width, bits });
        }
        Ok(Font {
            face,
            points,
            height,
            ascent,
            weight,
            italic,
            first,
            default_char,
            glyphs,
        })
    }

    /// Every font in the `.FON` file `fon`.
    ///
    /// # Errors
    /// If it isn't an NE file or a font in it doesn't parse.
    pub fn parse_fon(fon: &[u8]) -> Result<Vec<Font>, Error> {
        ne::resources(fon)
            .map_err(Error::Ne)?
            .into_iter()
            .filter(|r| r.kind == ne::ResId::Id(RT_FONT))
            .map(|r| Font::parse(r.data))
            .collect()
    }

    /// The glyph for byte `c` (Windows-1252), or the default one.
    #[must_use]
    pub fn glyph(&self, c: u8) -> Option<&Glyph> {
        let i = c.checked_sub(self.first).map(usize::from);
        i.and_then(|i| self.glyphs.get(i)).or_else(|| {
            self.glyphs
                .get(usize::from(self.default_char.saturating_sub(self.first)))
        })
    }

    /// The width of `text` in pixels.
    #[must_use]
    pub fn width(&self, text: &[u8]) -> u32 {
        text.iter().filter_map(|&c| self.glyph(c)).map(|g| u32::from(g.width)).sum()
    }
}
