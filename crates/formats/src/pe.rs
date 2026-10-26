//! The resource directory of a 32-bit Windows (PE) executable. Widget
//! Workshop keeps nearly everything in resources: `WIDGET.EXE` its dialogs,
//! menus, cursors, strings and palette; `WWGADGET.DLL` the gadgets'
//! pictures and tables; `WWSOUND.DLL` the sounds. The game's own loader
//! (`CWResourceFile`, 0x4106xx) reads them by number only ("Only ID entries
//! are supported"), as this does; named entries are skipped.

use std::collections::BTreeMap;
use std::fmt;

pub const RT_CURSOR: u32 = 1;
pub const RT_BITMAP: u32 = 2;
pub const RT_ICON: u32 = 3;
pub const RT_MENU: u32 = 4;
pub const RT_DIALOG: u32 = 5;
pub const RT_STRING: u32 = 6;
pub const RT_ACCELERATOR: u32 = 9;
pub const RT_GROUP_CURSOR: u32 = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// No MZ header, or no PE header behind it.
    NotPe,
    /// No `.rsrc` data directory.
    NoResources,
    /// A table or resource lies outside the file.
    OutOfBounds(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotPe => f.write_str("not a PE executable"),
            Error::NoResources => f.write_str("no resources"),
            Error::OutOfBounds(at) => write!(f, "data at {at:#x} lies outside the file"),
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

struct Section {
    va: u32,
    size: u32,
    raw: u32,
    raw_size: u32,
}

/// Every numbered resource of a PE file: `(type, id)` to its bytes (the
/// first language of each).
#[derive(Debug, Clone, Default)]
pub struct Resources {
    pub entries: BTreeMap<(u32, u32), Vec<u8>>,
}

impl Resources {
    /// Reads the resource directory of the PE file `exe`.
    ///
    /// # Errors
    /// If it isn't a PE file, has no resources, or a table lies outside it.
    pub fn parse(exe: &[u8]) -> Result<Resources, Error> {
        if exe.get(..2) != Some(b"MZ") {
            return Err(Error::NotPe);
        }
        let pe = u32_at(exe, 0x3c)? as usize;
        if exe.get(pe..pe + 4) != Some(b"PE\0\0") {
            return Err(Error::NotPe);
        }
        let coff = pe + 4;
        let nsect = usize::from(u16_at(exe, coff + 2)?);
        let opt = coff + 20;
        let opt_size = usize::from(u16_at(exe, coff + 16)?);
        // PE32: the data directories start at 96; the resources are the third.
        let rsrc_rva = u32_at(exe, opt + 96 + 2 * 8)?;
        if rsrc_rva == 0 {
            return Err(Error::NoResources);
        }
        let mut sections = Vec::new();
        for i in 0..nsect {
            let s = opt + opt_size + i * 40;
            sections.push(Section {
                size: u32_at(exe, s + 8)?,
                va: u32_at(exe, s + 12)?,
                raw_size: u32_at(exe, s + 16)?,
                raw: u32_at(exe, s + 20)?,
            });
        }
        let file_offset = |rva: u32| -> Result<usize, Error> {
            sections
                .iter()
                .find(|s| rva >= s.va && rva < s.va + s.size.max(s.raw_size))
                .map(|s| (rva - s.va + s.raw) as usize)
                .ok_or(Error::OutOfBounds(rva as usize))
        };
        let root = file_offset(rsrc_rva)?;
        let mut out = Resources::default();
        for (typ, type_dir) in Self::dir(exe, root, root)? {
            let Some(type_dir) = type_dir else { continue };
            for (id, id_dir) in Self::dir(exe, root, type_dir)? {
                let Some(id_dir) = id_dir else { continue };
                // The first language.
                let Some((_, None)) = Self::dir(exe, root, id_dir)?.into_iter().next() else {
                    continue;
                };
                let leaf_at = root + (u32_at(exe, id_dir + 16 + 4)? & 0x7fff_ffff) as usize;
                let data_rva = u32_at(exe, leaf_at)?;
                let size = u32_at(exe, leaf_at + 4)? as usize;
                let at = file_offset(data_rva)?;
                let data = exe.get(at..at + size).ok_or(Error::OutOfBounds(at))?;
                out.entries.insert((typ, id), data.to_vec());
            }
        }
        Ok(out)
    }

    /// A directory's numbered entries: the id and, for a subdirectory, its
    /// offset in the file (`None` for a leaf).
    fn dir(exe: &[u8], root: usize, at: usize) -> Result<Vec<(u32, Option<usize>)>, Error> {
        let named = usize::from(u16_at(exe, at + 12)?);
        let ids = usize::from(u16_at(exe, at + 14)?);
        let mut out = Vec::new();
        for i in named..named + ids {
            let e = at + 16 + i * 8;
            let id = u32_at(exe, e)?;
            let off = u32_at(exe, e + 4)?;
            let sub = (off & 0x8000_0000 != 0).then_some(root + (off & 0x7fff_ffff) as usize);
            out.push((id, sub));
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

    /// String `id` of the `RT_STRING` tables (16 to a block, UTF-16).
    #[must_use]
    pub fn string(&self, id: u32) -> Option<String> {
        let block = self.get(RT_STRING, id / 16 + 1)?;
        let mut at = 0;
        for i in 0..16 {
            let len = usize::from(u16::from_le_bytes([*block.get(at)?, *block.get(at + 1)?]));
            at += 2;
            if i == id % 16 {
                let units: Vec<u16> = block
                    .get(at..at + 2 * len)?
                    .as_chunks::<2>().0.iter()
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                return Some(String::from_utf16_lossy(&units));
            }
            at += 2 * len;
        }
        None
    }
}
