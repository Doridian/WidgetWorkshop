//! The resource table of a 16-bit Windows (NE) executable. `HOLIDAY.EXE`
//! keeps its form layouts here as `RT_RCDATA` resources named after the form
//! classes (see [`crate::dfm`]).

use std::fmt;

/// `RT_RCDATA`.
pub const RT_RCDATA: u16 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not an MZ file with an NE header.
    NotNe,
    /// A table or resource lies outside the file.
    OutOfBounds(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotNe => write!(f, "not an NE executable"),
            Error::OutOfBounds(at) => write!(f, "data at {at:#x} lies outside the file"),
        }
    }
}

impl std::error::Error for Error {}

/// A resource type or name: a number, or a string (upper case in the file).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResId {
    Id(u16),
    Name(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource<'a> {
    pub kind: ResId,
    pub name: ResId,
    /// The resource's bytes, padded to the file's alignment unit.
    pub data: &'a [u8],
}

fn u16_at(d: &[u8], at: usize) -> Result<u16, Error> {
    d.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or(Error::OutOfBounds(at))
}

fn slice(d: &[u8], at: usize, len: usize) -> Result<&[u8], Error> {
    d.get(at..at + len).ok_or(Error::OutOfBounds(at))
}

/// All resources, in table order.
///
/// # Errors
/// If the file isn't NE or a table entry points outside it.
pub fn resources(exe: &[u8]) -> Result<Vec<Resource<'_>>, Error> {
    if exe.get(..2) != Some(b"MZ") {
        return Err(Error::NotNe);
    }
    let ne = usize::from(u16_at(exe, 0x3c)?) | usize::from(u16_at(exe, 0x3e)?) << 16;
    if exe.get(ne..ne + 2) != Some(b"NE") {
        return Err(Error::NotNe);
    }
    let table = ne + usize::from(u16_at(exe, ne + 0x24)?);
    let shift = u16_at(exe, table)?;
    if shift > 16 {
        return Err(Error::OutOfBounds(table));
    }
    let id = |raw: u16| -> Result<ResId, Error> {
        if raw & 0x8000 != 0 {
            return Ok(ResId::Id(raw & 0x7fff));
        }
        let at = table + usize::from(raw);
        let len = usize::from(*exe.get(at).ok_or(Error::OutOfBounds(at))?);
        Ok(ResId::Name(slice(exe, at + 1, len)?.to_vec()))
    };
    let mut out = Vec::new();
    let mut p = table + 2;
    loop {
        let kind = u16_at(exe, p)?;
        if kind == 0 {
            return Ok(out);
        }
        let count = u16_at(exe, p + 2)?;
        p += 8;
        let kind = id(kind)?;
        for _ in 0..count {
            let offset = usize::from(u16_at(exe, p)?) << shift;
            let len = usize::from(u16_at(exe, p + 2)?) << shift;
            let name = id(u16_at(exe, p + 6)?)?;
            out.push(Resource {
                kind: kind.clone(),
                name,
                data: slice(exe, offset, len)?,
            });
            p += 12;
        }
    }
}
