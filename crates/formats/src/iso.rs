//! ISO 9660, as far as the game's CD needs it: the primary volume
//! descriptor and the directories' records. The CD is a single MODE1
//! data track; an image is either its 2048-byte sectors (`.iso`) or the
//! raw 2352-byte ones (a `.bin`), which [`Layout`] tells apart. The
//! callers read the sectors; this only parses them (`tools/extract_cd.py`
//! does the same in Python).

use std::fmt;

/// A sector's user data.
pub const SECTOR: u64 = 2048;
/// A raw MODE1 sector: sync, header, data, error correction.
const RAW_SECTOR: u64 = 2352;
/// Where the data starts in a raw sector.
const RAW_DATA: u64 = 16;
/// The primary volume descriptor's sector.
pub const PVD_SECTOR: u64 = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Sector 16 isn't a primary volume descriptor.
    NotIso,
    /// A record runs past its directory.
    Truncated,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotIso => f.write_str("not an ISO 9660 CD image"),
            Error::Truncated => f.write_str("a directory record is cut off"),
        }
    }
}

impl std::error::Error for Error {}

/// How an image's bytes hold the sectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// 2048 bytes each (`.iso`, `MODE1/2048`).
    Cooked,
    /// 2352 bytes each, the data at 16 (`.bin`, `MODE1/2352`).
    Raw,
}

impl Layout {
    /// The layout an image of `size` bytes has: raw if it's a whole
    /// number of raw sectors and not of cooked ones.
    #[must_use]
    pub fn of_size(size: u64) -> Layout {
        if size.is_multiple_of(RAW_SECTOR) && !size.is_multiple_of(SECTOR) {
            Layout::Raw
        } else {
            Layout::Cooked
        }
    }

    /// Where sector `lba`'s data starts in the image.
    #[must_use]
    pub fn offset(self, lba: u64) -> u64 {
        match self {
            Layout::Cooked => lba * SECTOR,
            Layout::Raw => lba * RAW_SECTOR + RAW_DATA,
        }
    }
}

/// Where a file's or a directory's bytes are: `size` bytes from sector
/// `lba` on, in consecutive sectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extent {
    pub lba: u32,
    pub size: u32,
}

/// The root directory's extent, from the primary volume descriptor
/// (sector [`PVD_SECTOR`]'s 2048 bytes).
///
/// # Errors
/// [`Error::NotIso`] if it isn't one.
pub fn root(pvd: &[u8]) -> Result<Extent, Error> {
    if pvd.len() < 190 || pvd[0] != 1 || &pvd[1..6] != b"CD001" {
        return Err(Error::NotIso);
    }
    Ok(extent(&pvd[156..190]))
}

/// The volume's name, from the primary volume descriptor.
#[must_use]
pub fn volume_name(pvd: &[u8]) -> String {
    pvd.get(40..72)
        .map(|n| String::from_utf8_lossy(n).trim_matches([' ', '\0']).to_owned())
        .unwrap_or_default()
}

fn extent(record: &[u8]) -> Extent {
    let u32_at = |p: usize| u32::from_le_bytes([record[p], record[p + 1], record[p + 2], record[p + 3]]);
    Extent {
        lba: u32_at(2),
        size: u32_at(10),
    }
}

/// A directory's entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// The name without the version (`;1`) and a trailing dot.
    pub name: String,
    pub extent: Extent,
    pub is_dir: bool,
}

/// The entries of a directory whose extent's bytes are `data`, without
/// `.` and `..`. Records don't cross sectors: a zero length skips to the
/// next one.
///
/// # Errors
/// [`Error::Truncated`] if a record runs past `data`.
pub fn records(data: &[u8]) -> Result<Vec<Record>, Error> {
    let sector = usize::try_from(SECTOR).unwrap_or(2048);
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        let len = usize::from(data[pos]);
        if len == 0 {
            pos = (pos / sector + 1) * sector;
            continue;
        }
        let record = data.get(pos..pos + len).ok_or(Error::Truncated)?;
        if record.len() < 33 {
            return Err(Error::Truncated);
        }
        let name_len = usize::from(record[32]);
        let name = record.get(33..33 + name_len).ok_or(Error::Truncated)?;
        pos += len;
        if name == [0] || name == [1] {
            continue;
        }
        let name = String::from_utf8_lossy(name);
        let name = name.split(';').next().unwrap_or_default().trim_end_matches('.');
        out.push(Record {
            name: name.to_owned(),
            extent: extent(record),
            is_dir: record[25] & 2 != 0,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(name: &[u8], lba: u32, size: u32, dir: bool) -> Vec<u8> {
        let mut r = vec![0u8; 33];
        r[2..6].copy_from_slice(&lba.to_le_bytes());
        r[10..14].copy_from_slice(&size.to_le_bytes());
        r[25] = if dir { 2 } else { 0 };
        r[32] = u8::try_from(name.len()).unwrap();
        r.extend_from_slice(name);
        if r.len() % 2 == 1 {
            r.push(0);
        }
        r[0] = u8::try_from(r.len()).unwrap();
        r
    }

    #[test]
    fn directory_records() {
        let mut dir = record(&[0], 20, 2048, true);
        dir.extend(record(&[1], 20, 2048, true));
        dir.extend(record(b"GRAFIK", 30, 4096, true));
        dir.extend(record(b"HOLIDAY.EXE;1", 40, 1234, false));
        dir.resize(2048, 0);
        // A record in the next sector, after the zero padding.
        dir.extend(record(b"README.;1", 50, 99, false));
        let r = records(&dir).unwrap();
        assert_eq!(
            r,
            vec![
                Record {
                    name: "GRAFIK".into(),
                    extent: Extent { lba: 30, size: 4096 },
                    is_dir: true
                },
                Record {
                    name: "HOLIDAY.EXE".into(),
                    extent: Extent { lba: 40, size: 1234 },
                    is_dir: false
                },
                Record {
                    name: "README".into(),
                    extent: Extent { lba: 50, size: 99 },
                    is_dir: false
                },
            ]
        );
        assert_eq!(records(&dir[..40]), Err(Error::Truncated));
    }

    #[test]
    fn volume_descriptor() {
        let mut pvd = vec![0u8; 2048];
        assert_eq!(root(&pvd), Err(Error::NotIso));
        pvd[0] = 1;
        pvd[1..6].copy_from_slice(b"CD001");
        pvd[40..47].copy_from_slice(b"HOLIDAY");
        pvd[156..190].copy_from_slice(&record(&[0], 18, 2048, true)[..34]);
        assert_eq!(root(&pvd), Ok(Extent { lba: 18, size: 2048 }));
        assert_eq!(volume_name(&pvd), "HOLIDAY");
    }

    #[test]
    fn layouts() {
        assert_eq!(Layout::of_size(2048 * 100), Layout::Cooked);
        assert_eq!(Layout::of_size(2352 * 101), Layout::Raw);
        assert_eq!(Layout::Raw.offset(16), 16 * 2352 + 16);
        assert_eq!(Layout::Cooked.offset(16), 16 * 2048);
    }
}
