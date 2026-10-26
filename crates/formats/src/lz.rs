//! Microsoft's `COMPRESS.EXE` format (`SZDD`), which `LZExpand.dll`'s
//! `LZCopy` undoes: how `WIDGET.SLF`, the stand-alone widgets' runtime,
//! is kept on the CD (docs/editor-interaction.md 8.5).
//!
//! A 14-byte header (`SZDD 88 F0 27 33`, the method `'A'`, the last
//! letter of the name, the expanded size), then groups of a flag byte and
//! eight items, low bit first: a set bit is a literal byte, a clear one a
//! two-byte match (12-bit position in a 4096-byte window, 4-bit length
//! − 3). The window starts filled with spaces, written from 4096 − 16.

const MAGIC: &[u8; 8] = b"SZDD\x88\xf0\x27\x33";

/// The expanded bytes of `data`, `None` if it isn't `SZDD`. A stream that
/// ends early gives what it holds.
#[must_use]
pub fn expand(data: &[u8]) -> Option<Vec<u8>> {
    if data.get(..8)? != MAGIC || *data.get(8)? != b'A' {
        return None;
    }
    let size = u32::from_le_bytes(data.get(10..14)?.try_into().ok()?) as usize;
    let mut win = [b' '; 4096];
    let mut pos = 4096 - 16;
    let mut out = Vec::with_capacity(size);
    let mut i = 14;
    while i < data.len() && out.len() < size {
        let flags = data[i];
        i += 1;
        for bit in 0..8 {
            if i >= data.len() || out.len() >= size {
                break;
            }
            if flags & (1 << bit) != 0 {
                let c = data[i];
                i += 1;
                out.push(c);
                win[pos] = c;
                pos = (pos + 1) & 0xfff;
            } else {
                let (Some(&lo), Some(&hi)) = (data.get(i), data.get(i + 1)) else {
                    i = data.len();
                    break;
                };
                i += 2;
                let at = usize::from(lo) | (usize::from(hi & 0xf0) << 4);
                for k in 0..usize::from(hi & 0xf) + 3 {
                    let c = win[(at + k) & 0xfff];
                    out.push(c);
                    win[pos] = c;
                    pos = (pos + 1) & 0xfff;
                }
            }
        }
    }
    out.truncate(size);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_and_matches() {
        // "abcabcabc": three literals, then a match of 6 at 4080.
        let mut d = MAGIC.to_vec();
        d.extend_from_slice(&[b'A', b'x', 9, 0, 0, 0]);
        d.extend_from_slice(&[0b0000_0111, b'a', b'b', b'c', 0xf0, 0xf3]);
        assert_eq!(expand(&d).unwrap(), b"abcabcabc");
        // The window starts as spaces.
        let mut d = MAGIC.to_vec();
        d.extend_from_slice(&[b'A', b'x', 3, 0, 0, 0, 0, 0x00, 0x00]);
        assert_eq!(expand(&d).unwrap(), b"   ");
        assert_eq!(expand(b"MZ not compressed"), None);
    }

    #[test]
    fn the_cds_runtime() {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../orig/WIN95/WIDGET/WIDGET.SLF");
        let Ok(d) = std::fs::read(p) else { return };
        let exe = expand(&d).unwrap();
        assert_eq!(exe.len(), 790_528);
        assert_eq!(&exe[..2], b"MZ");
        assert!(crate::pe::Resources::parse(&exe).is_ok());
    }
}
