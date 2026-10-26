//! The resources of a 32-bit OS/2 (LX) executable, as the OS/2 Warp CD
//! keeps `WIDGET.EXE`, `WWGADGET.DLL`, `WWSOUND.DLL` and `WWINTRO.DLL`.
//! The game's own types (1003 and up) hold the same bytes as the Win95
//! build's; the system's (dialogs, menus, pointers, strings, accelerators)
//! are Presentation Manager's, which [`crate::os2`] turns into the Win32
//! ones.
//!
//! An LX file keeps its resources in objects (segments) cut into pages; a
//! resource is a slice of an object. Pages are stored as they are, zero,
//! or packed: `EXEPACK1` (iterated records, [`unpack1`]) or `EXEPACK2`
//! ([`unpack2`]), as `WIDGET.EXE`'s are.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt;

/// Presentation Manager's resource types (`RT_*`, `pmwin.h`).
pub const RT_POINTER: u32 = 1;
pub const RT_BITMAP: u32 = 2;
pub const RT_MENU: u32 = 3;
pub const RT_DIALOG: u32 = 4;
pub const RT_STRING: u32 = 5;
pub const RT_ACCELTABLE: u32 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// No MZ header, or no LX header behind it.
    NotLx,
    /// A table, page or resource lies outside the file.
    OutOfBounds(usize),
    /// A page packed in a way this doesn't read (its flags).
    Page(u16),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotLx => f.write_str("not an LX executable"),
            Error::OutOfBounds(at) => write!(f, "data at {at:#x} lies outside the file"),
            Error::Page(flags) => write!(f, "a page of type {flags} can't be read"),
        }
    }
}

impl std::error::Error for Error {}

fn u16_at(b: &[u8], at: usize) -> Result<u16, Error> {
    b.get(at..at + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or(Error::OutOfBounds(at))
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, Error> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or(Error::OutOfBounds(at))
}

/// The offset of the LX header of `exe`, if it is one.
fn header(exe: &[u8]) -> Option<usize> {
    if exe.get(..2)? != b"MZ" {
        return None;
    }
    let lx = u32_at(exe, 0x3c).ok()? as usize;
    (exe.get(lx..lx + 2)? == b"LX").then_some(lx)
}

/// Whether `exe` is an LX executable.
#[must_use]
pub fn is_lx(exe: &[u8]) -> bool {
    header(exe).is_some()
}

/// Every resource of an LX file: `(type, id)` to its bytes, in OS/2's
/// numbering.
#[derive(Debug, Clone, Default)]
pub struct Resources {
    pub entries: BTreeMap<(u32, u32), Vec<u8>>,
}

impl Resources {
    /// Reads the resource table of the LX file `exe`, unpacking the
    /// objects that hold resources.
    ///
    /// # Errors
    /// If it isn't an LX file, a table, page or resource lies outside it,
    /// or a page is packed in an unknown way.
    pub fn parse(exe: &[u8]) -> Result<Resources, Error> {
        let lx = header(exe).ok_or(Error::NotLx)?;
        let h = |off: usize| u32_at(exe, lx + off).map(|v| v as usize);
        let (page_size, shift) = (h(0x28)?, h(0x2c)?);
        let (objects, pages) = (lx + h(0x40)?, lx + h(0x48)?);
        let (table, count) = (lx + h(0x50)?, h(0x54)?);
        let data = h(0x80)?;
        let mut loaded: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
        let mut out = Resources::default();
        for i in 0..count {
            let e = table + 14 * i;
            let typ = u32::from(u16_at(exe, e)?);
            let id = u32::from(u16_at(exe, e + 2)?);
            let size = u32_at(exe, e + 4)? as usize;
            let obj = usize::from(u16_at(exe, e + 8)?);
            let off = u32_at(exe, e + 10)? as usize;
            if let Entry::Vacant(slot) = loaded.entry(obj) {
                // The object: virtual size, base, flags, first page (from 1),
                // page count.
                let o = objects + 24 * obj.checked_sub(1).ok_or(Error::OutOfBounds(e))?;
                let first = (u32_at(exe, o + 12)? as usize).max(1);
                let n = u32_at(exe, o + 16)? as usize;
                let mut buf = Vec::with_capacity(n * page_size);
                for p in first..first + n {
                    let pe = pages + 8 * (p - 1);
                    let at = data + ((u32_at(exe, pe)? as usize) << shift);
                    let len = usize::from(u16_at(exe, pe + 4)?);
                    let flags = u16_at(exe, pe + 6)?;
                    let raw = exe.get(at..at + len).ok_or(Error::OutOfBounds(at));
                    let mut page = match flags {
                        0 => raw?.to_vec(),
                        1 => unpack1(raw?, page_size),
                        3 => Vec::new(),
                        5 => unpack2(raw?, page_size),
                        _ => return Err(Error::Page(flags)),
                    };
                    page.resize(page_size, 0);
                    buf.extend(page);
                }
                slot.insert(buf);
            }
            let buf = &loaded[&obj];
            let res = buf.get(off..off + size).ok_or(Error::OutOfBounds(off))?;
            out.entries.insert((typ, id), res.to_vec());
        }
        Ok(out)
    }

    #[must_use]
    pub fn get(&self, typ: u32, id: u32) -> Option<&[u8]> {
        self.entries.get(&(typ, id)).map(Vec::as_slice)
    }

    /// The ids of type `typ`, ascending.
    pub fn ids(&self, typ: u32) -> impl Iterator<Item = u32> + '_ {
        self.entries.range((typ, 0)..=(typ, u32::MAX)).map(|((_, id), _)| *id)
    }
}

/// An `EXEPACK1` (iterated) page: records of a repeat count, a length and
/// that many bytes, repeated; at most `page_size` bytes.
#[must_use]
pub fn unpack1(src: &[u8], page_size: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(page_size);
    let mut i = 0;
    while i + 4 <= src.len() && out.len() < page_size {
        let n = usize::from(u16::from_le_bytes([src[i], src[i + 1]]));
        let len = usize::from(u16::from_le_bytes([src[i + 2], src[i + 3]]));
        let Some(rec) = src.get(i + 4..i + 4 + len) else { break };
        if n == 0 {
            break;
        }
        for _ in 0..n {
            out.extend_from_slice(rec);
        }
        i += 4 + len;
    }
    out.truncate(page_size);
    out
}

/// An `EXEPACK2` page: items told apart by their first byte's low two
/// bits:
///
/// - `00`: `n` literal bytes (`n` = byte >> 2), or with a zero byte, a
///   count and a byte to repeat (a zero count ends the page);
/// - `01`: a 16-bit word: 0 to 3 literals (bits 2-3), then a copy of 3
///   to 10 bytes (bits 4-6) from up to 511 back (bits 7-15);
/// - `10`: a 16-bit word: a copy of 3 to 6 bytes (bits 2-3) from up to
///   4095 back (bits 4-15);
/// - `11`: a 24-bit word: 0 to 15 literals (bits 2-5), then a copy of 0
///   to 63 bytes (bits 6-11) from up to 4095 back (bits 12-23).
///
/// At most `page_size` bytes; a stream that ends early gives what it holds.
#[must_use]
pub fn unpack2(src: &[u8], page_size: usize) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(page_size);
    let back = |out: &mut Vec<u8>, off: usize, len: usize| {
        if off == 0 || off > out.len() {
            return false;
        }
        for _ in 0..len {
            out.push(out[out.len() - off]);
        }
        true
    };
    let b = |i: usize| usize::from(src.get(i).copied().unwrap_or(0));
    let mut i = 0;
    while i < src.len() && out.len() < page_size {
        let c = b(i);
        let ok = match c & 3 {
            0 if c == 0 => {
                let n = b(i + 1);
                if n == 0 {
                    break;
                }
                out.extend(std::iter::repeat_n(src.get(i + 2).copied().unwrap_or(0), n));
                i += 3;
                true
            }
            0 => {
                let n = c >> 2;
                out.extend_from_slice(src.get(i + 1..i + 1 + n).unwrap_or_default());
                i += 1 + n;
                true
            }
            1 => {
                let w = c | b(i + 1) << 8;
                let lit = (w >> 2) & 3;
                out.extend_from_slice(src.get(i + 2..i + 2 + lit).unwrap_or_default());
                i += 2 + lit;
                back(&mut out, w >> 7, ((w >> 4) & 7) + 3)
            }
            2 => {
                let w = c | b(i + 1) << 8;
                i += 2;
                back(&mut out, w >> 4, ((w >> 2) & 3) + 3)
            }
            _ => {
                let w = c | b(i + 1) << 8 | b(i + 2) << 16;
                let lit = (w >> 2) & 0xf;
                out.extend_from_slice(src.get(i + 3..i + 3 + lit).unwrap_or_default());
                i += 3 + lit;
                back(&mut out, w >> 12, (w >> 6) & 0x3f)
            }
        };
        if !ok {
            break;
        }
    }
    out.truncate(page_size);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpacks_exepack2() {
        // "abc" literal; type 1: one literal 'd', copy 3 from 4 back
        // ("abc"); type 2: copy 3 from 2 back; a run of five 'z'; the end.
        // Length fields 0: three bytes.
        let w1: u16 = 1 | (1 << 2) | (4 << 7);
        let w2: u16 = 2 | (2 << 4);
        let mut src = vec![3 << 2, b'a', b'b', b'c'];
        src.extend(w1.to_le_bytes());
        src.push(b'd');
        src.extend(w2.to_le_bytes());
        src.extend([0, 5, b'z']);
        // Type 3: two literals, copy 4 from 2 back (overlapping).
        let w3: u32 = 3 | (2 << 2) | (4 << 6) | (2 << 12);
        src.extend(&w3.to_le_bytes()[..3]);
        src.extend(b"xy");
        src.extend([0, 0]);
        assert_eq!(unpack2(&src, 4096), b"abcdabcbcbzzzzzxyxyxy");
        assert_eq!(unpack2(&src, 5), b"abcda");
    }

    #[test]
    fn unpacks_iterated() {
        let mut src = vec![3, 0, 2, 0, b'a', b'b', 1, 0, 1, 0, b'c'];
        src.extend([0, 0, 0, 0]);
        assert_eq!(unpack1(&src, 4096), b"abababc");
    }

    /// A small LX: two objects; resource (1004, 7) in a plain page of the
    /// first, (5, 1) across an `EXEPACK2` page and a zero page of the
    /// second.
    #[test]
    fn reads_resources() {
        let page = 16;
        let lx = 0x40;
        let mut b = vec![0u8; 0x200];
        b[..2].copy_from_slice(b"MZ");
        b[0x3c] = 0x40;
        b[lx..lx + 2].copy_from_slice(b"LX");
        let put = |b: &mut Vec<u8>, at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        put(&mut b, lx + 0x28, page);
        put(&mut b, lx + 0x40, 0x100);
        put(&mut b, lx + 0x48, 0x140);
        put(&mut b, lx + 0x50, 0x160);
        put(&mut b, lx + 0x54, 2);
        put(&mut b, lx + 0x80, 0x1c0);
        // Objects: pages 1 and 2-3.
        put(&mut b, lx + 0x100 + 12, 1);
        put(&mut b, lx + 0x100 + 16, 1);
        put(&mut b, lx + 0x118 + 12, 2);
        put(&mut b, lx + 0x118 + 16, 2);
        // Pages: plain at 0, packed at 16, zero.
        let packed = [4 << 2, b'w', b'x', b'y', b'z', 0, 12, b'!', 0, 0];
        put(&mut b, lx + 0x140, 0);
        b[lx + 0x144] = 16;
        put(&mut b, lx + 0x148, 16);
        b[lx + 0x14c] = 10;
        b[lx + 0x14e] = 5;
        b[lx + 0x156] = 3;
        // Resources: type, id, size, object, offset.
        let res = |b: &mut Vec<u8>, i: usize, t: u16, id: u16, size: u32, obj: u16, off: u32| {
            let e = lx + 0x160 + 14 * i;
            b[e..e + 2].copy_from_slice(&t.to_le_bytes());
            b[e + 2..e + 4].copy_from_slice(&id.to_le_bytes());
            b[e + 4..e + 8].copy_from_slice(&size.to_le_bytes());
            b[e + 8..e + 10].copy_from_slice(&obj.to_le_bytes());
            b[e + 10..e + 14].copy_from_slice(&off.to_le_bytes());
        };
        res(&mut b, 0, 1004, 7, 5, 1, 2);
        res(&mut b, 1, 5, 1, 20, 2, 2);
        b[0x1c0..0x1d0].copy_from_slice(b"0123456789abcdef");
        b.extend([0u8; 0x10]);
        b[0x1d0..0x1d0 + packed.len()].copy_from_slice(&packed);
        let r = Resources::parse(&b).unwrap();
        assert_eq!(r.get(1004, 7), Some(&b"23456"[..]));
        let mut want = b"yz".to_vec();
        want.extend([b'!'; 12]);
        want.extend([0; 6]);
        assert_eq!(r.get(5, 1), Some(&want[..]));
        assert!(is_lx(&b));
        assert_eq!(Resources::parse(b"MZ").unwrap_err(), Error::NotLx);
    }
}
