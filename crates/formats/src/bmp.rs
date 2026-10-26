//! Windows 3.x BMP files. Every BMP on the CD is 8-bit, uncompressed and
//! bottom-up with a `BITMAPINFOHEADER`, so that's all this reads.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// No `BM` signature, or a header that isn't `BITMAPINFOHEADER`.
    NotBmp,
    /// A depth or compression other than 8-bit `BI_RGB`.
    Unsupported { bits: u16, compression: u32 },
    /// The palette or pixels lie outside the file.
    Truncated,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotBmp => write!(f, "not a Windows 3.x BMP"),
            Error::Unsupported { bits, compression } => {
                write!(f, "unsupported BMP: {bits} bits, compression {compression}")
            }
            Error::Truncated => write!(f, "BMP file is truncated"),
        }
    }
}

impl std::error::Error for Error {}

/// One palette entry, as `RGBQUAD` minus the reserved byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    /// 256 entries; those past the file's colour count are black.
    pub palette: [Rgb; 256],
    /// Palette indices, top row first, `width` bytes per row.
    pub pixels: Vec<u8>,
}

fn u16_at(d: &[u8], at: usize) -> Result<u16, Error> {
    d.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or(Error::Truncated)
}

fn u32_at(d: &[u8], at: usize) -> Result<u32, Error> {
    d.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(Error::Truncated)
}

fn size(n: u32) -> Result<usize, Error> {
    usize::try_from(n).map_err(|_| Error::Truncated)
}

impl Bitmap {
    /// Parses a BMP file.
    ///
    /// # Errors
    /// See [`Error`].
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        if data.get(..2) != Some(b"BM") || u32_at(data, 14)? != 40 {
            return Err(Error::NotBmp);
        }
        let bits_offset = size(u32_at(data, 10)?)?;
        let width = u32_at(data, 18)?.cast_signed();
        let height = u32_at(data, 22)?.cast_signed();
        let bits = u16_at(data, 28)?;
        let compression = u32_at(data, 30)?;
        if bits != 8 || compression != 0 || width < 0 {
            return Err(Error::Unsupported { bits, compression });
        }
        let colours = match u32_at(data, 46)? {
            0 => 256,
            n => size(n)?.min(256),
        };
        let mut palette = [Rgb::default(); 256];
        let table = data.get(54..54 + 4 * colours).ok_or(Error::Truncated)?;
        for (entry, q) in palette.iter_mut().zip(table.as_chunks::<4>().0) {
            *entry = Rgb {
                r: q[2],
                g: q[1],
                b: q[0],
            };
        }
        let width = size(width.unsigned_abs())?;
        let rows = size(height.unsigned_abs())?;
        let stride = width.div_ceil(4) * 4;
        let mut pixels = Vec::with_capacity(width * rows);
        for row in 0..rows {
            // Positive heights are stored bottom-up.
            let stored = if height > 0 { rows - 1 - row } else { row };
            let at = bits_offset + stored * stride;
            pixels.extend_from_slice(data.get(at..at + width).ok_or(Error::Truncated)?);
        }
        Ok(Bitmap {
            width,
            height: rows,
            palette,
            pixels,
        })
    }

    /// The palette index at `x`, `y` (0, 0 is the top left).
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> u8 {
        self.pixels[y * self.width + x]
    }

    /// The bitmap as a BMP file of the kind [`Bitmap::parse`] reads.
    #[must_use]
    pub fn to_bmp(&self) -> Vec<u8> {
        let stride = self.width.div_ceil(4) * 4;
        let bits_offset = 54 + 4 * 256;
        let size = bits_offset + stride * self.height;
        let u32_of = |n: usize| u32::try_from(n).unwrap_or(u32::MAX).to_le_bytes();
        let mut d = b"BM".to_vec();
        d.extend_from_slice(&u32_of(size));
        d.extend_from_slice(&[0; 4]);
        d.extend_from_slice(&u32_of(bits_offset));
        d.extend_from_slice(&40u32.to_le_bytes());
        d.extend_from_slice(&u32_of(self.width));
        d.extend_from_slice(&u32_of(self.height));
        d.extend_from_slice(&[1, 0, 8, 0]);
        d.extend_from_slice(&[0; 24]);
        for c in &self.palette {
            d.extend_from_slice(&[c.b, c.g, c.r, 0]);
        }
        for row in self.pixels.chunks(self.width.max(1)).rev() {
            d.extend_from_slice(row);
            d.resize(d.len() + stride - row.len(), 0);
        }
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bmp(width: u32, height: i32, rows: &[&[u8]]) -> Vec<u8> {
        let mut d = b"BM".to_vec();
        d.extend_from_slice(&[0; 8]);
        d.extend_from_slice(&(54u32 + 8).to_le_bytes());
        d.extend_from_slice(&40u32.to_le_bytes());
        d.extend_from_slice(&width.to_le_bytes());
        d.extend_from_slice(&height.to_le_bytes());
        d.extend_from_slice(&[1, 0, 8, 0]);
        d.extend_from_slice(&[0; 16]);
        d.extend_from_slice(&2u32.to_le_bytes());
        d.extend_from_slice(&[0; 4]);
        d.extend_from_slice(&[0, 0, 0xff, 0, 0xff, 0, 0, 0]);
        for row in rows {
            d.extend_from_slice(row);
        }
        d
    }

    #[test]
    fn parse() {
        // Two rows of 3 pixels, padded to 4 bytes, bottom row first.
        let b = Bitmap::parse(&bmp(3, 2, &[&[1, 1, 0, 9], &[0, 1, 0, 9]])).unwrap();
        assert_eq!((b.width, b.height), (3, 2));
        assert_eq!(b.pixels, [0, 1, 0, 1, 1, 0]);
        assert_eq!(b.palette[0], Rgb { r: 0xff, g: 0, b: 0 });
        assert_eq!(b.palette[1], Rgb { r: 0, g: 0, b: 0xff });
        assert_eq!(b.palette[2], Rgb::default());
        let top_down = Bitmap::parse(&bmp(3, -2, &[&[1, 1, 0, 9], &[0, 1, 0, 9]])).unwrap();
        assert_eq!(top_down.pixel(0, 0), 1);
        assert_eq!(Bitmap::parse(&bmp(3, 3, &[&[0; 4]])), Err(Error::Truncated));
        assert_eq!(Bitmap::parse(&b.to_bmp()), Ok(b));
    }
}
