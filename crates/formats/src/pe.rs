//! The resource directory of a 32-bit Windows (PE) executable. Widget
//! Workshop keeps nearly everything in resources: `WIDGET.EXE` its dialogs,
//! menus, cursors, strings and palette; `WWGADGET.DLL` the gadgets'
//! pictures and tables; `WWSOUND.DLL` the sounds. The game's own loader
//! (`CWResourceFile`, 0x4106xx) reads them by number only ("Only ID entries
//! are supported"), as this does; named entries are skipped.
//!
//! [`add_resources`] is the other half of `CWResourceFile` (`0x41218a`,
//! `0x412297`, `0x412d27`): resources added to an executable whose
//! `.rsrc` is its last section, the directory written anew in place. Save
//! Into a Stand-Alone Widget makes its `.EXE`s so.

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
    /// The resources aren't the start of the last section, where
    /// [`add_resources`] rewrites them.
    NotLast,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotPe => f.write_str("not a PE executable"),
            Error::NoResources => f.write_str("no resources"),
            Error::OutOfBounds(at) => write!(f, "data at {at:#x} lies outside the file"),
            Error::NotLast => f.write_str("the resources aren't the last section"),
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
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                return Some(String::from_utf16_lossy(&units));
            }
            at += 2 * len;
        }
        None
    }
}

/// A directory entry's name: a string (UTF-16) or a number. Named entries
/// come first, as the format wants.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Name(Vec<u16>),
    Id(u32),
}

/// A resource: its bytes and code page.
type Leaf = (Vec<u8>, u32);
/// type → name → language → resource.
type Tree = BTreeMap<Key, BTreeMap<Key, BTreeMap<Key, Leaf>>>;

/// The PE headers [`add_resources`] patches: where they are and the
/// `.rsrc` section's.
struct Layout {
    opt: usize,
    /// The section header of `.rsrc`.
    header: usize,
    va: u32,
    raw: usize,
    file_align: u32,
    section_align: u32,
}

fn layout(exe: &[u8]) -> Result<Layout, Error> {
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
    let first = opt + usize::from(u16_at(exe, coff + 16)?);
    let rva = u32_at(exe, opt + 96 + 2 * 8)?;
    if rva == 0 {
        return Err(Error::NoResources);
    }
    let header = (0..nsect)
        .map(|i| first + i * 40)
        .find(|&s| u32_at(exe, s + 12).is_ok_and(|va| va == rva))
        .ok_or(Error::NotLast)?;
    let raw = u32_at(exe, header + 20)? as usize;
    for i in 0..nsect {
        let s = first + i * 40;
        if s != header && (u32_at(exe, s + 12)? > rva || u32_at(exe, s + 20)? as usize > raw) {
            return Err(Error::NotLast);
        }
    }
    Ok(Layout {
        opt,
        header,
        va: rva,
        raw,
        section_align: u32_at(exe, opt + 32)?.max(1),
        file_align: u32_at(exe, opt + 36)?.max(1),
    })
}

/// Every resource of the section at `root` (a file offset), names and
/// languages kept.
fn tree(exe: &[u8], root: usize, va: u32, raw: usize) -> Result<Tree, Error> {
    let entries = |at: usize| -> Result<Vec<(Key, u32)>, Error> {
        let n = usize::from(u16_at(exe, at + 12)?) + usize::from(u16_at(exe, at + 14)?);
        (0..n)
            .map(|i| {
                let e = at + 16 + i * 8;
                let name = u32_at(exe, e)?;
                let key = if name & 0x8000_0000 == 0 {
                    Key::Id(name)
                } else {
                    let s = root + (name & 0x7fff_ffff) as usize;
                    let len = usize::from(u16_at(exe, s)?);
                    Key::Name((0..len).map(|k| u16_at(exe, s + 2 + 2 * k)).collect::<Result<_, _>>()?)
                };
                Ok((key, u32_at(exe, e + 4)?))
            })
            .collect()
    };
    let sub = |off: u32| (off & 0x8000_0000 != 0).then_some(root + (off & 0x7fff_ffff) as usize);
    let mut t = Tree::new();
    for (typ, off) in entries(root)? {
        let Some(td) = sub(off) else { continue };
        for (name, off) in entries(td)? {
            let Some(nd) = sub(off) else { continue };
            for (lang, off) in entries(nd)? {
                if sub(off).is_some() {
                    continue;
                }
                let leaf = root + off as usize;
                let rva = u32_at(exe, leaf)?;
                let size = u32_at(exe, leaf + 4)? as usize;
                let at = (rva.checked_sub(va).ok_or(Error::OutOfBounds(leaf))? as usize) + raw;
                let data = exe.get(at..at + size).ok_or(Error::OutOfBounds(at))?;
                t.entry(typ.clone())
                    .or_default()
                    .entry(name.clone())
                    .or_default()
                    .insert(lang, (data.to_vec(), u32_at(exe, leaf + 8)?));
            }
        }
    }
    Ok(t)
}

fn align(n: usize, a: usize) -> usize {
    n.div_ceil(a) * a
}

fn put16(b: &mut [u8], at: usize, v: u16) {
    b[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

fn put32(b: &mut [u8], at: usize, v: usize) {
    b[at..at + 4].copy_from_slice(&u32::try_from(v).unwrap_or(u32::MAX).to_le_bytes());
}

/// The section's bytes for `t`, its data at `va`: the directories, then
/// the names, the data entries and the data (8-byte aligned).
fn write_tree(t: &Tree, va: u32) -> Vec<u8> {
    let dir_size = |n: usize| 16 + 8 * n;
    // The directories: the root, the types', the names'.
    let mut at = dir_size(t.len());
    let mut type_at = Vec::new();
    for names in t.values() {
        type_at.push(at);
        at += dir_size(names.len());
    }
    let mut name_at = Vec::new();
    for names in t.values() {
        for langs in names.values() {
            name_at.push(at);
            at += dir_size(langs.len());
        }
    }
    // The strings.
    let mut strings = Vec::new();
    let mut key = |k: &Key, at: &mut usize| match k {
        Key::Id(id) => *id as usize,
        Key::Name(s) => {
            strings.push((*at, s.clone()));
            let off = *at | 0x8000_0000;
            *at += 2 + 2 * s.len();
            off
        }
    };
    let mut keys = Vec::new();
    for (typ, names) in t {
        keys.push(key(typ, &mut at));
        for (name, langs) in names {
            keys.push(key(name, &mut at));
            for lang in langs.keys() {
                keys.push(key(lang, &mut at));
            }
        }
    }
    at = align(at, 4);
    let entries_at = at;
    let n_leaves: usize = t.values().flat_map(BTreeMap::values).map(BTreeMap::len).sum();
    at += 16 * n_leaves;
    let mut data_at = Vec::new();
    for langs in t.values().flat_map(BTreeMap::values) {
        for (data, _) in langs.values() {
            at = align(at, 8);
            data_at.push(at);
            at += data.len();
        }
    }
    let mut b = vec![0u8; align(at, 8)];
    for (at, s) in strings {
        put16(&mut b, at, u16::try_from(s.len()).unwrap_or(u16::MAX));
        for (i, c) in s.iter().enumerate() {
            put16(&mut b, at + 2 + 2 * i, *c);
        }
    }
    let dir = |b: &mut [u8], at: usize, keys: &[(&Key, usize)]| {
        let named = keys.iter().filter(|(k, _)| matches!(k, Key::Name(_))).count();
        put16(b, at + 12, u16::try_from(named).unwrap_or(0));
        put16(b, at + 14, u16::try_from(keys.len() - named).unwrap_or(0));
    };
    let mut keys = keys.into_iter();
    let (mut ni, mut leaf) = (0, 0);
    let root: Vec<(&Key, usize)> = t.keys().zip(&type_at).map(|(k, &a)| (k, a)).collect();
    dir(&mut b, 0, &root);
    for (ti, (typ, names)) in t.iter().enumerate() {
        put32(&mut b, 16 + 8 * ti, keys.next().unwrap_or(0));
        put32(&mut b, 16 + 8 * ti + 4, type_at[ti] | 0x8000_0000);
        let list: Vec<(&Key, usize)> = names.keys().map(|k| (k, 0)).collect();
        dir(&mut b, type_at[ti], &list);
        let _ = typ;
        for (k, langs) in names.values().enumerate() {
            let e = type_at[ti] + 16 + 8 * k;
            put32(&mut b, e, keys.next().unwrap_or(0));
            put32(&mut b, e + 4, name_at[ni] | 0x8000_0000);
            let list: Vec<(&Key, usize)> = langs.keys().map(|k| (k, 0)).collect();
            dir(&mut b, name_at[ni], &list);
            for (l, (data, cp)) in langs.values().enumerate() {
                let e = name_at[ni] + 16 + 8 * l;
                let entry = entries_at + 16 * leaf;
                put32(&mut b, e, keys.next().unwrap_or(0));
                put32(&mut b, e + 4, entry);
                put32(&mut b, entry, va as usize + data_at[leaf]);
                put32(&mut b, entry + 4, data.len());
                put32(&mut b, entry + 8, *cp as usize);
                b[data_at[leaf]..data_at[leaf] + data.len()].copy_from_slice(data);
                leaf += 1;
            }
            ni += 1;
        }
    }
    b
}

/// The executable `exe` with the resources `add` (type, id, bytes) in it,
/// language 0, each replacing one of the same type, id and language
/// (`CWResourceFile`, docs/editor-interaction.md 8.5). The `.rsrc`
/// section, which must be the last, is written anew and the headers
/// follow its size; whatever lay after it goes.
///
/// # Errors
/// If `exe` isn't a PE file, has no resources, or they aren't the last
/// section.
pub fn add_resources(exe: &[u8], add: &[(u32, u32, &[u8])]) -> Result<Vec<u8>, Error> {
    let l = layout(exe)?;
    let mut t = tree(exe, l.raw, l.va, l.raw)?;
    for &(typ, id, data) in add {
        t.entry(Key::Id(typ))
            .or_default()
            .entry(Key::Id(id))
            .or_default()
            .insert(Key::Id(0), (data.to_vec(), 0));
    }
    let rsrc = write_tree(&t, l.va);
    let raw_size = align(rsrc.len(), l.file_align as usize);
    let mut out = exe.get(..l.raw).ok_or(Error::OutOfBounds(l.raw))?.to_vec();
    out.extend_from_slice(&rsrc);
    out.resize(l.raw + raw_size, 0);
    put32(&mut out, l.header + 8, rsrc.len());
    put32(&mut out, l.header + 16, raw_size);
    put32(&mut out, l.opt + 96 + 2 * 8 + 4, rsrc.len());
    let image = align(l.va as usize + rsrc.len(), l.section_align as usize);
    put32(&mut out, l.opt + 56, image);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small PE: two sections, `.rsrc` last with one resource (type 5,
    /// id 7) and one named (type 6, "ABC").
    fn exe() -> Vec<u8> {
        let mut t = Tree::new();
        t.entry(Key::Id(5))
            .or_default()
            .entry(Key::Id(7))
            .or_default()
            .insert(Key::Id(0x409), (b"seven".to_vec(), 1252));
        t.entry(Key::Id(6))
            .or_default()
            .entry(Key::Name(vec![u16::from(b'A'), u16::from(b'B'), u16::from(b'C')]))
            .or_default()
            .insert(Key::Id(0), (b"named".to_vec(), 0));
        let rsrc = write_tree(&t, 0x2000);
        let mut b = vec![0u8; 0x400];
        b[..2].copy_from_slice(b"MZ");
        put32(&mut b, 0x3c, 0x40);
        b[0x40..0x44].copy_from_slice(b"PE\0\0");
        put16(&mut b, 0x46, 2);
        put16(&mut b, 0x54, 0xe0);
        let opt = 0x58;
        put32(&mut b, opt + 32, 0x1000);
        put32(&mut b, opt + 36, 0x200);
        put32(&mut b, opt + 56, 0x3000);
        put32(&mut b, opt + 96 + 16, 0x2000);
        put32(&mut b, opt + 96 + 20, rsrc.len());
        let s = opt + 0xe0;
        for (i, (va, raw, size)) in [(0x1000, 0x200, 0x200), (0x2000, 0x400, rsrc.len())]
            .into_iter()
            .enumerate()
        {
            let h = s + 40 * i;
            put32(&mut b, h + 8, size);
            put32(&mut b, h + 12, va);
            put32(&mut b, h + 16, align(size, 0x200));
            put32(&mut b, h + 20, raw);
        }
        b.extend_from_slice(&rsrc);
        b.resize(0x400 + align(rsrc.len(), 0x200), 0);
        b
    }

    #[test]
    fn adds_and_keeps_resources() {
        let b = exe();
        let r = Resources::parse(&b).unwrap();
        assert_eq!(r.get(5, 7), Some(&b"seven"[..]));
        let big = vec![0xab; 5000];
        let out = add_resources(&b, &[(1023, 1, b"widget"), (1004, 100, &big), (5, 7, b"new")]).unwrap();
        let r = Resources::parse(&out).unwrap();
        assert_eq!(r.get(1023, 1), Some(&b"widget"[..]));
        assert_eq!(r.get(1004, 100), Some(&big[..]));
        // Another language: both kept, the first found first.
        assert_eq!(r.get(5, 7), Some(&b"new"[..]));
        let l = layout(&out).unwrap();
        let t = tree(&out, l.raw, l.va, l.raw).unwrap();
        assert_eq!(t[&Key::Id(5)][&Key::Id(7)][&Key::Id(0x409)], (b"seven".to_vec(), 1252));
        assert!(t[&Key::Id(6)].contains_key(&Key::Name(vec![65, 66, 67])));
        // The headers follow the new size.
        let size = u32_at(&out, l.opt + 96 + 20).unwrap() as usize;
        assert_eq!(u32_at(&out, l.header + 8).unwrap() as usize, size);
        assert_eq!(out.len(), 0x400 + align(size, 0x200));
        assert_eq!(u32_at(&out, l.opt + 56).unwrap() as usize, align(0x2000 + size, 0x1000));
        // Again on the result: the same.
        let again = add_resources(&out, &[]).unwrap();
        assert_eq!(again, out);
    }

    #[test]
    fn refuses_what_it_cant_rewrite() {
        assert_eq!(add_resources(b"nope", &[]).unwrap_err(), Error::NotPe);
        let mut b = exe();
        // The resources moved to the first section.
        put32(&mut b, 0x58 + 96 + 16, 0x1000);
        assert_eq!(add_resources(&b, &[]).unwrap_err(), Error::NotLast);
    }
}
