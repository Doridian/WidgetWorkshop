//! Windows cursors (`RT_GROUP_CURSOR`, `RT_CURSOR`), as `WIDGET.EXE`
//! keeps the editor's pointers (groups 5000 to 5013, `0x439b88`).

/// The `RT_CURSOR` id of a group's first image: a `NEWHEADER` (6 bytes)
/// then 14-byte entries ending in the image's id.
#[must_use]
pub fn group_first(d: &[u8]) -> Option<u16> {
    let n = u16::from_le_bytes([*d.get(4)?, *d.get(5)?]);
    if n == 0 {
        return None;
    }
    Some(u16::from_le_bytes([*d.get(6 + 12)?, *d.get(6 + 13)?]))
}

/// A cursor image in true colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub width: u16,
    pub height: u16,
    /// The hot spot, from the top left.
    pub hot: (u16, u16),
    /// RGBA, top row first; the AND mask's pixels are transparent.
    pub rgba: Vec<u8>,
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*d.get(at)?, *d.get(at + 1)?]))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *d.get(at)?,
        *d.get(at + 1)?,
        *d.get(at + 2)?,
        *d.get(at + 3)?,
    ]))
}

impl Cursor {
    /// Reads an `RT_CURSOR`: the hot spot, then a DIB twice as high as the
    /// cursor (the XOR image, then the AND mask), 1, 4 or 8 bits a pixel.
    #[must_use]
    pub fn parse(d: &[u8]) -> Option<Cursor> {
        let hot = (u16_at(d, 0)?, u16_at(d, 2)?);
        let b = d.get(4..)?;
        let hdr = u32_at(b, 0)? as usize;
        let w = u32_at(b, 4)? as usize;
        let h = (u32_at(b, 8)? / 2) as usize;
        let bpp = usize::from(u16_at(b, 14)?);
        if !matches!(bpp, 1 | 4 | 8) || w == 0 || h == 0 || w > 256 || h > 256 {
            return None;
        }
        let used = u32_at(b, 32)? as usize;
        let colors = if used == 0 { 1 << bpp } else { used };
        let pal = hdr;
        let xor = pal + colors * 4;
        let stride = (w * bpp).div_ceil(32) * 4;
        let and = xor + stride * h;
        let and_stride = w.div_ceil(32) * 4;
        let mut rgba = vec![0; w * h * 4];
        for y in 0..h {
            let row = h - 1 - y;
            for x in 0..w {
                let bit = x * bpp;
                let byte = *b.get(xor + row * stride + bit / 8)?;
                let i = usize::from(byte >> (8 - bpp - bit % 8)) & ((1 << bpp) - 1);
                let masked = b.get(and + row * and_stride + x / 8)? & (0x80 >> (x % 8)) != 0;
                let at = (y * w + x) * 4;
                if !masked {
                    let c = pal + i * 4;
                    rgba[at] = *b.get(c + 2)?;
                    rgba[at + 1] = *b.get(c + 1)?;
                    rgba[at + 2] = *b.get(c)?;
                    rgba[at + 3] = 0xff;
                }
            }
        }
        Some(Cursor {
            width: u16::try_from(w).ok()?,
            height: u16::try_from(h).ok()?,
            hot,
            rgba,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_one_bit_cursor() {
        // 2×2 (stride 4), 1 bpp: XOR rows then AND rows, bottom up.
        let mut d = vec![1, 0, 0, 0];
        let mut h = vec![0u8; 40];
        h[0] = 40;
        h[4] = 2;
        h[8] = 4;
        h[12] = 1;
        h[14] = 1;
        d.extend(h);
        d.extend([0, 0, 0, 0, 255, 255, 255, 0]);
        // XOR: bottom row white|black, top row black|black.
        d.extend([0x80, 0, 0, 0, 0, 0, 0, 0]);
        // AND: bottom row opaque, top row: right pixel transparent.
        d.extend([0, 0, 0, 0, 0x40, 0, 0, 0]);
        let c = Cursor::parse(&d).unwrap();
        assert_eq!(c.hot, (1, 0));
        assert_eq!((c.width, c.height), (2, 2));
        assert_eq!(&c.rgba[..8], [0, 0, 0, 255, 0, 0, 0, 0]);
        assert_eq!(&c.rgba[8..], [255, 255, 255, 255, 0, 0, 0, 255]);
    }
}
