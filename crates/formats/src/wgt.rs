//! Saved widgets (`*.WGT`, the Mac's file type `Gadf`): an IFF-style
//! `FORM` of type `Gadf`, big-endian like the Mac version that wrote the
//! format. `CWidgetDoc`'s loader (`0x42503e`) reads:
//!
//! - `fver`: the format version (`u16`, 1 to 4; the shipped widgets are 4).
//! - `back`: the worksheet's background (`u16`).
//! - `Gmun`: which puzzle the widget is (`0x432b3c`).
//! - `conn`: a wire between two gadgets ([`Connection`]).
//! - anything else: a gadget, the chunk's tag its class (`cnst`, `AndG`,
//!   ...), its data the object as the Mac wrote it; the gadget's class reads
//!   it.
//!
//! The loader steps from chunk to chunk by the size plus 8, with no IFF
//! padding to an even size, as does this.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    NotWidget,
    OutOfBounds(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotWidget => f.write_str("not a saved widget (FORM Gadf)"),
            Error::OutOfBounds(at) => write!(f, "chunk at {at:#x} runs past the file's end"),
        }
    }
}

impl std::error::Error for Error {}

/// A chunk: its four-character tag and its data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub tag: [u8; 4],
    pub data: Vec<u8>,
}

impl Chunk {
    #[must_use]
    pub fn tag_str(&self) -> String {
        String::from_utf8_lossy(&self.tag).into_owned()
    }
}

/// A file's chunks, in order, after `FORM` and `Gadf`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Form {
    pub chunks: Vec<Chunk>,
}

impl Form {
    /// Splits the widget file `data` into its chunks.
    ///
    /// # Errors
    /// If it isn't a `FORM Gadf`, or a chunk runs past its end.
    pub fn parse(data: &[u8]) -> Result<Form, Error> {
        if data.get(..4) != Some(b"FORM") || data.get(8..12) != Some(b"Gadf") {
            return Err(Error::NotWidget);
        }
        let mut chunks = Vec::new();
        let mut at = 12;
        // The loader reads up to the end of the file it's given, not to the
        // FORM's size (which some files get wrong).
        while at + 8 <= data.len() {
            let tag = [data[at], data[at + 1], data[at + 2], data[at + 3]];
            let size = be32(data, at + 4)? as usize;
            let body = data.get(at + 8..at + 8 + size).ok_or(Error::OutOfBounds(at))?;
            chunks.push(Chunk {
                tag,
                data: body.to_vec(),
            });
            at += 8 + size;
        }
        Ok(Form { chunks })
    }

    /// Writes the chunks back as a widget file.
    #[must_use]
    pub fn write(&self) -> Vec<u8> {
        let mut body = b"Gadf".to_vec();
        for c in &self.chunks {
            body.extend_from_slice(&c.tag);
            body.extend_from_slice(&u32::try_from(c.data.len()).unwrap_or(u32::MAX).to_be_bytes());
            body.extend_from_slice(&c.data);
        }
        let mut out = b"FORM".to_vec();
        out.extend_from_slice(&u32::try_from(body.len()).unwrap_or(u32::MAX).to_be_bytes());
        out.extend_from_slice(&body);
        out
    }

    /// The first chunk tagged `tag`.
    #[must_use]
    pub fn find(&self, tag: &[u8; 4]) -> Option<&Chunk> {
        self.chunks.iter().find(|c| &c.tag == tag)
    }

    /// `fver`'s version, if there is one.
    #[must_use]
    pub fn version(&self) -> Option<u16> {
        let c = self.find(b"fver")?;
        be16(&c.data, 0).ok()
    }
}

/// A big-endian `u16` at `at`.
///
/// # Errors
/// If it lies past the end.
pub fn be16(b: &[u8], at: usize) -> Result<u16, Error> {
    b.get(at..at + 2)
        .map(|s| u16::from_be_bytes([s[0], s[1]]))
        .ok_or(Error::OutOfBounds(at))
}

/// A big-endian `u32` at `at`.
///
/// # Errors
/// If it lies past the end.
pub fn be32(b: &[u8], at: usize) -> Result<u32, Error> {
    b.get(at..at + 4)
        .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or(Error::OutOfBounds(at))
}

/// A `conn` chunk: a wire from gadget `from`'s knot `from_knot` to gadget
/// `to`'s knot `to_knot` (gadgets by the id their chunks carry), bent at
/// `points` (`v`, `h` each). `cConnection`'s reader (`0x43b010`): format
/// versions up to 2 end after the knots (straight wires); from 3 on a
/// byte (`+0x25` in the object) and a pad byte follow, then the count of
/// points and the points.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Connection {
    pub from: u32,
    pub to: u32,
    pub from_knot: u16,
    pub to_knot: u16,
    pub flag: u8,
    pub points: Vec<(i16, i16)>,
}

impl Connection {
    /// Reads a `conn` chunk's data as format `version` wrote it.
    ///
    /// # Errors
    /// If it's cut short.
    #[allow(clippy::cast_possible_wrap)]
    pub fn parse(data: &[u8], version: u16) -> Result<Connection, Error> {
        let mut c = Connection {
            from: be32(data, 0)?,
            to: be32(data, 4)?,
            from_knot: be16(data, 8)?,
            to_knot: be16(data, 10)?,
            ..Connection::default()
        };
        if version >= 3 {
            c.flag = *data.get(12).ok_or(Error::OutOfBounds(12))?;
            let mut at = 14;
            let n = be16(data, at)?;
            at += 2;
            for _ in 0..n {
                c.points.push((be16(data, at)? as i16, be16(data, at + 2)? as i16));
                at += 4;
            }
        }
        Ok(c)
    }

    /// The chunk data, as version 4 writes it.
    #[must_use]
    pub fn write(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.from.to_be_bytes());
        out.extend_from_slice(&self.to.to_be_bytes());
        out.extend_from_slice(&self.from_knot.to_be_bytes());
        out.extend_from_slice(&self.to_knot.to_be_bytes());
        out.push(self.flag);
        out.push(0);
        out.extend_from_slice(&u16::try_from(self.points.len()).unwrap_or(u16::MAX).to_be_bytes());
        for (v, h) in &self.points {
            out.extend_from_slice(&v.to_be_bytes());
            out.extend_from_slice(&h.to_be_bytes());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_chunks_and_connections() {
        let conn = Connection {
            from: 0x0381_8810,
            to: 7,
            from_knot: 2,
            to_knot: 0,
            flag: 1,
            points: vec![(10, 20), (-3, 40)],
        };
        let form = Form {
            chunks: vec![
                Chunk {
                    tag: *b"fver",
                    data: vec![0, 4],
                },
                Chunk {
                    tag: *b"conn",
                    data: conn.write(),
                },
            ],
        };
        let bytes = form.write();
        let back = Form::parse(&bytes).unwrap();
        assert_eq!(back, form);
        assert_eq!(back.version(), Some(4));
        assert_eq!(Connection::parse(&back.chunks[1].data, 4).unwrap(), conn);
    }
}
