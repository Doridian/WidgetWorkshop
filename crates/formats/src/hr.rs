//! The gadgets' picture sheets: resource type 1004 (`0x3ec`) in
//! `WWGADGET.DLL` and `WWINTRO.DLL`, magic `HR`. A sheet holds numbered
//! frames (from 1), each a rectangle and run-length coded 8-bit pixels in
//! the game's palette, with transparent runs.
//!
//! Layout (little-endian, as the Windows port converted it):
//!
//! ```text
//! 0  "HR"
//! 2  u16 frame count
//! 4  per frame, 12 bytes: i16 top, left, bottom, right (a Mac Rect), u32
//!    offset of its pixels from the sheet's start
//! ```
//!
//! The pixels, as the blitter reads them (`0x4228c0`, and clipped
//! `0x422a81`): a row begins with two bytes `Fx nn` (`x` and `nn` its coded
//! length, which the blitter ignores), then runs, each a byte `c` with
//! `n = (c & 0x3f) + 1`:
//!
//! - `00xxxxxx`: `n` literal pixels follow.
//! - `01xxxxxx`: skip `n` pixels (transparent).
//! - `10xxxxxx`: the next byte, `n` times.
//! - `11xxxxxx`: the row ends; this byte is the next row's header (or the
//!   end).
//!
//! Between rows, `Ex xx` skips `0xxxx & 0xfff` (big-endian) rows. Any
//! other byte ends the picture.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    NotHr,
    OutOfBounds(usize),
    /// Frame numbers start at 1.
    NoFrame(u16),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotHr => f.write_str("not an HR picture sheet"),
            Error::OutOfBounds(at) => write!(f, "data at {at:#x} lies outside the sheet"),
            Error::NoFrame(n) => write!(f, "no frame {n}"),
        }
    }
}

impl std::error::Error for Error {}

/// A Mac `Rect`: `top`, `left`, `bottom`, `right`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Rect {
    pub top: i16,
    pub left: i16,
    pub bottom: i16,
    pub right: i16,
}

impl Rect {
    #[must_use]
    pub const fn new(top: i16, left: i16, bottom: i16, right: i16) -> Rect {
        Rect {
            top,
            left,
            bottom,
            right,
        }
    }

    #[must_use]
    pub fn width(self) -> i16 {
        self.right - self.left
    }

    #[must_use]
    pub fn height(self) -> i16 {
        self.bottom - self.top
    }
}

/// A decoded frame: `width` × `height` palette indices, top row first,
/// and which of them are drawn (`false` is transparent).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Image {
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<u8>,
    pub opaque: Vec<bool>,
}

impl Image {
    #[must_use]
    pub fn new(width: u16, height: u16) -> Image {
        let n = usize::from(width) * usize::from(height);
        Image {
            width,
            height,
            pixels: vec![0; n],
            opaque: vec![false; n],
        }
    }

    /// The pixel at `x`, `y`, if it's drawn.
    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> Option<u8> {
        let i = y * usize::from(self.width) + x;
        (x < usize::from(self.width) && self.opaque.get(i) == Some(&true)).then(|| self.pixels[i])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameEntry {
    pub rect: Rect,
    pub offset: u32,
}

/// A parsed sheet: its frame table and the bytes to decode from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    pub frames: Vec<FrameEntry>,
    data: Vec<u8>,
}

impl Sheet {
    /// Reads the frame table of the sheet `data`.
    ///
    /// # Errors
    /// If it isn't an HR sheet or its table is cut short.
    pub fn parse(data: &[u8]) -> Result<Sheet, Error> {
        if data.get(..2) != Some(b"HR") {
            return Err(Error::NotHr);
        }
        let rd16 = |at: usize| -> Result<i16, Error> {
            data.get(at..at + 2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]))
                .ok_or(Error::OutOfBounds(at))
        };
        // The frame count is unsigned.
        #[allow(clippy::cast_sign_loss)]
        let n = rd16(2)? as u16;
        let mut frames = Vec::with_capacity(usize::from(n));
        for i in 0..usize::from(n) {
            let at = 4 + i * 12;
            let rect = Rect::new(rd16(at)?, rd16(at + 2)?, rd16(at + 4)?, rd16(at + 6)?);
            let off = data.get(at + 8..at + 12).ok_or(Error::OutOfBounds(at + 8))?;
            frames.push(FrameEntry {
                rect,
                offset: u32::from_le_bytes([off[0], off[1], off[2], off[3]]),
            });
        }
        Ok(Sheet {
            frames,
            data: data.to_vec(),
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Frame `n`'s rectangle (from 1, as the game numbers them).
    #[must_use]
    pub fn rect(&self, n: u16) -> Option<Rect> {
        self.frames.get(usize::from(n).checked_sub(1)?).map(|f| f.rect)
    }

    /// Decodes frame `n` (from 1).
    ///
    /// # Errors
    /// If there's no such frame or its pixels run past the sheet.
    pub fn decode(&self, n: u16) -> Result<Image, Error> {
        let entry = self
            .frames
            .get(usize::from(n).checked_sub(1).ok_or(Error::NoFrame(n))?)
            .ok_or(Error::NoFrame(n))?;
        let w = u16::try_from(entry.rect.width().max(0)).unwrap_or(0);
        let h = u16::try_from(entry.rect.height().max(0)).unwrap_or(0);
        let mut img = Image::new(w, h);
        if w == 0 || h == 0 {
            return Ok(img);
        }
        decode_into(&self.data, entry.offset as usize, &mut img)?;
        Ok(img)
    }
}

/// Decodes the run-length pixels at `at` into `img`; pixels past its edges
/// are dropped.
fn decode_into(data: &[u8], mut at: usize, img: &mut Image) -> Result<(), Error> {
    let byte = |i: usize| data.get(i).copied().ok_or(Error::OutOfBounds(i));
    let (w, h) = (usize::from(img.width), usize::from(img.height));
    let mut row = 0usize;
    loop {
        let b0 = byte(at)?;
        match b0 & 0xf0 {
            0xf0 => {
                at += 2;
                let mut x = 0usize;
                loop {
                    let c = byte(at)?;
                    at += 1;
                    let n = usize::from(c & 0x3f) + 1;
                    match c & 0xc0 {
                        0x00 => {
                            for k in 0..n {
                                put(img, w, h, x + k, row, byte(at + k)?);
                            }
                            at += n;
                            x += n;
                        }
                        0x40 => x += n,
                        0x80 => {
                            let v = byte(at)?;
                            at += 1;
                            for k in 0..n {
                                put(img, w, h, x + k, row, v);
                            }
                            x += n;
                        }
                        _ => {
                            at -= 1;
                            row += 1;
                            break;
                        }
                    }
                }
            }
            0xe0 => {
                let b1 = byte(at + 1)?;
                row += usize::from(u16::from_be_bytes([b0, b1]) & 0xfff);
                at += 2;
            }
            _ => return Ok(()),
        }
    }
}

fn put(img: &mut Image, w: usize, h: usize, x: usize, y: usize, v: u8) {
    if x < w && y < h {
        img.pixels[y * w + x] = v;
        img.opaque[y * w + x] = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4×3 sheet: a row of literals and a run, a skipped row, then a
    /// transparent gap.
    #[test]
    fn decodes_runs_skips_and_literals() {
        let mut d = b"HR\x01\x00".to_vec();
        d.extend_from_slice(&[0, 0, 0, 0, 3, 0, 4, 0, 16, 0, 0, 0]);
        // The byte that ends a row is the next one's header (or the end).
        d.extend_from_slice(&[0xf0, 6, 0x01, 7, 8, 0x81, 9]);
        d.extend_from_slice(&[0xe0, 0x01]);
        d.extend_from_slice(&[0xf0, 4, 0x41, 0x80, 5]);
        d.push(0xc0);
        let s = Sheet::parse(&d).unwrap();
        assert_eq!(s.rect(1), Some(Rect::new(0, 0, 3, 4)));
        let img = s.decode(1).unwrap();
        assert_eq!((img.width, img.height), (4, 3));
        let row = |y| (0..4).map(|x| img.get(x, y)).collect::<Vec<_>>();
        assert_eq!(row(0), [Some(7), Some(8), Some(9), Some(9)]);
        assert_eq!(row(1), [None; 4]);
        assert_eq!(row(2), [None, None, Some(5), None]);
        assert_eq!(s.decode(2), Err(Error::NoFrame(2)));
    }
}
