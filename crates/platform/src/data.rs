//! Where the game's files and the player's come from, as [`fs::Source`]s
//! to mount ([`fs::mount`]):
//!
//! - [`CdImage`]: the game's CD as an image (ISO 9660, `.iso` or a raw
//!   `.bin`). The browser build reads the game from one; natively it's
//!   one of the places the game's files can come from.
//! - [`MemoryFiles`]: files kept in memory, each change also handed to a
//!   callback: the browser's storage for the saves and the settings.
//!
//! Names on the CD are upper case; [`CdImage`] finds them in any case,
//! as the game's own lookups do.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::sync::{Mutex, PoisonError};

use formats::iso;

use crate::fs::{self, Kind};

/// An image's bytes, read at any offset.
pub trait ReadAt: Send + Sync {
    /// The image's size.
    fn size(&self) -> u64;

    /// Fills `buf` from `offset` on.
    ///
    /// # Errors
    /// If that's past the end or can't be read.
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()>;
}

impl ReadAt for Vec<u8> {
    fn size(&self) -> u64 {
        self.len() as u64
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        let start = usize::try_from(offset).map_err(|_| past_end())?;
        let bytes = self.get(start..start + buf.len()).ok_or_else(past_end)?;
        buf.copy_from_slice(bytes);
        Ok(())
    }
}

fn past_end() -> io::Error {
    io::Error::new(io::ErrorKind::UnexpectedEof, "past the image's end")
}

/// An image in a file.
#[cfg(not(target_arch = "wasm32"))]
pub struct FileImage {
    file: Mutex<std::fs::File>,
    size: u64,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileImage {
    /// Opens the image at `path`.
    ///
    /// # Errors
    /// If it can't be opened.
    pub fn open(path: &std::path::Path) -> io::Result<FileImage> {
        let file = std::fs::File::open(path)?;
        let size = file.metadata()?.len();
        Ok(FileImage {
            file: Mutex::new(file),
            size,
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ReadAt for FileImage {
    fn size(&self) -> u64 {
        self.size
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        use std::io::{Read, Seek, SeekFrom};
        let mut f = self.file.lock().unwrap_or_else(PoisonError::into_inner);
        f.seek(SeekFrom::Start(offset))?;
        f.read_exact(buf)
    }
}

/// A file of a [`CdImage`]: where it is on the CD.
#[derive(Clone)]
enum Content {
    Dir,
    Extent(iso::Extent),
}

struct Node {
    /// The name as the CD or the archive has it.
    name: String,
    content: Content,
}

/// The game's CD from an image.
pub struct CdImage {
    image: Box<dyn ReadAt>,
    layout: iso::Layout,
    /// Every path, upper case, `/` between the parts; the root is "".
    nodes: BTreeMap<String, Node>,
    /// The upper-case paths in each directory.
    children: BTreeMap<String, BTreeSet<String>>,
    /// The volume's name.
    pub volume: String,
}

/// The parent of `path` (`a/b` → `a`, `a` → "").
fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(p, _)| p)
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_owned()
    } else {
        format!("{dir}/{name}")
    }
}

impl CdImage {
    /// Reads the image's directories.
    ///
    /// # Errors
    /// If it isn't an ISO 9660 image, or a directory can't be read.
    pub fn open(image: Box<dyn ReadAt>) -> Result<CdImage, String> {
        let layout = iso::Layout::of_size(image.size());
        let mut cd = CdImage {
            image,
            layout,
            nodes: BTreeMap::new(),
            children: BTreeMap::new(),
            volume: String::new(),
        };
        let pvd = cd
            .extent(iso::Extent {
                lba: u32::try_from(iso::PVD_SECTOR).unwrap_or(16),
                size: 2048,
            })
            .map_err(|_| iso::Error::NotIso.to_string())?;
        let root = iso::root(&pvd).map_err(|e| e.to_string())?;
        cd.volume = iso::volume_name(&pvd);
        cd.add(String::new(), String::new(), Content::Dir);
        let mut dirs = vec![(String::new(), root)];
        // Each directory once, in case of a loop.
        let mut seen = BTreeSet::new();
        while let Some((path, extent)) = dirs.pop() {
            if !seen.insert(extent.lba) {
                continue;
            }
            let data = cd.extent(extent).map_err(|e| format!("{path}: {e}"))?;
            for r in iso::records(&data).map_err(|e| format!("{path}: {e}"))? {
                let child = join(&path, &r.name.to_ascii_uppercase());
                if r.is_dir {
                    cd.add(child.clone(), r.name, Content::Dir);
                    dirs.push((child, r.extent));
                } else {
                    cd.add(child, r.name, Content::Extent(r.extent));
                }
            }
        }
        Ok(cd)
    }

    /// Opens the image file at `path`.
    ///
    /// # Errors
    /// As [`CdImage::open`], and if the file can't be opened.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_file(path: &std::path::Path) -> Result<CdImage, String> {
        let image = FileImage::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        CdImage::open(Box::new(image))
    }

    fn add(&mut self, path: String, name: String, content: Content) {
        if !path.is_empty() {
            self.children
                .entry(parent(&path).to_owned())
                .or_default()
                .insert(path.clone());
        }
        self.nodes.insert(path, Node { name, content });
    }

    fn extent(&self, extent: iso::Extent) -> io::Result<Vec<u8>> {
        let size = usize::try_from(extent.size).map_err(|_| past_end())?;
        let mut out = vec![0; size];
        match self.layout {
            iso::Layout::Cooked => self
                .image
                .read_at(self.layout.offset(u64::from(extent.lba)), &mut out)?,
            iso::Layout::Raw => {
                // A sector at a time, past each one's header and error
                // correction.
                for (i, chunk) in out.chunks_mut(2048).enumerate() {
                    let lba = u64::from(extent.lba) + i as u64;
                    self.image.read_at(self.layout.offset(lba), chunk)?;
                }
            }
        }
        Ok(out)
    }

    fn node(&self, path: &str) -> Option<&Node> {
        self.nodes.get(&path.to_ascii_uppercase())
    }

    /// The paths of every file, `/` between the parts, as the CD names
    /// them.
    #[must_use]
    pub fn files(&self) -> Vec<String> {
        self.nodes
            .iter()
            .filter(|(_, n)| !matches!(n.content, Content::Dir))
            .map(|(p, _)| p.clone())
            .collect()
    }
}

fn not_found(path: &str) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("{path}: not found"))
}

impl fs::Source for CdImage {
    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        match &self.node(path).ok_or_else(|| not_found(path))?.content {
            Content::Dir => Err(io::Error::new(io::ErrorKind::IsADirectory, path.to_owned())),
            Content::Extent(e) => self.extent(*e),
        }
    }

    fn kind(&self, path: &str) -> Option<Kind> {
        self.node(path).map(|n| match n.content {
            Content::Dir => Kind::Dir,
            Content::Extent(_) => Kind::File,
        })
    }

    fn list(&self, path: &str) -> io::Result<Vec<(String, Kind)>> {
        if self.kind(path) != Some(Kind::Dir) {
            return Err(not_found(path));
        }
        Ok(self
            .children
            .get(&path.to_ascii_uppercase())
            .into_iter()
            .flatten()
            .filter_map(|p| self.nodes.get(p))
            .map(|n| {
                let kind = if matches!(n.content, Content::Dir) {
                    Kind::Dir
                } else {
                    Kind::File
                };
                (n.name.clone(), kind)
            })
            .collect())
    }
}

/// What [`MemoryFiles`] calls on each change: the path and the new
/// bytes, or none for a removed file.
pub type Persist = Box<dyn Fn(&str, Option<&[u8]>) + Send + Sync>;

/// Files in memory, every change handed to a [`Persist`] too.
pub struct MemoryFiles {
    files: Mutex<BTreeMap<String, Vec<u8>>>,
    /// Directories made, besides those the files' paths imply.
    dirs: Mutex<BTreeSet<String>>,
    persist: Option<Persist>,
}

impl MemoryFiles {
    /// The files `files` (path, bytes), each change passed to `persist`.
    #[must_use]
    pub fn new(files: Vec<(String, Vec<u8>)>, persist: Option<Persist>) -> MemoryFiles {
        MemoryFiles {
            files: Mutex::new(files.into_iter().collect()),
            dirs: Mutex::new(BTreeSet::new()),
            persist,
        }
    }

    fn files(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Vec<u8>>> {
        self.files.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn dirs(&self) -> std::sync::MutexGuard<'_, BTreeSet<String>> {
        self.dirs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn persist(&self, path: &str, data: Option<&[u8]>) {
        if let Some(p) = &self.persist {
            p(path, data);
        }
    }
}

impl fs::Source for MemoryFiles {
    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        self.files().get(path).cloned().ok_or_else(|| not_found(path))
    }

    fn kind(&self, path: &str) -> Option<Kind> {
        if self.files().contains_key(path) {
            return Some(Kind::File);
        }
        let prefix = format!("{path}/");
        let is_dir = path.is_empty()
            || self.dirs().contains(path)
            || self.files().keys().any(|k| k.starts_with(&prefix))
            || self.dirs().iter().any(|k| k.starts_with(&prefix));
        is_dir.then_some(Kind::Dir)
    }

    fn list(&self, path: &str) -> io::Result<Vec<(String, Kind)>> {
        if self.kind(path) != Some(Kind::Dir) {
            return Err(not_found(path));
        }
        let prefix = if path.is_empty() {
            String::new()
        } else {
            format!("{path}/")
        };
        let mut out: BTreeMap<String, Kind> = BTreeMap::new();
        let files: Vec<String> = self.files().keys().cloned().collect();
        let dirs: Vec<String> = self.dirs().iter().cloned().collect();
        for (key, is_file) in files.iter().map(|k| (k, true)).chain(dirs.iter().map(|k| (k, false))) {
            let Some(rest) = key.strip_prefix(&prefix) else {
                continue;
            };
            match rest.split_once('/') {
                Some((dir, _)) => {
                    out.insert(dir.to_owned(), Kind::Dir);
                }
                None if !rest.is_empty() => {
                    out.insert(rest.to_owned(), if is_file { Kind::File } else { Kind::Dir });
                }
                None => {}
            }
        }
        Ok(out.into_iter().collect())
    }

    fn write(&self, path: &str, data: &[u8]) -> io::Result<()> {
        if path.is_empty() || self.kind(path) == Some(Kind::Dir) {
            return Err(io::Error::new(io::ErrorKind::IsADirectory, path.to_owned()));
        }
        self.files().insert(path.to_owned(), data.to_vec());
        self.persist(path, Some(data));
        Ok(())
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        self.files().remove(path).ok_or_else(|| not_found(path))?;
        self.persist(path, None);
        Ok(())
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        let data = self.files().remove(from).ok_or_else(|| not_found(from))?;
        self.persist(to, Some(&data));
        self.persist(from, None);
        self.files().insert(to.to_owned(), data);
        Ok(())
    }

    fn create_dir_all(&self, path: &str) -> io::Result<()> {
        if self.files().contains_key(path) {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, path.to_owned()));
        }
        if !path.is_empty() {
            self.dirs().insert(path.to_owned());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::Source;
    use std::sync::Arc;

    #[test]
    fn memory_files_persist_each_change() {
        type Log = Vec<(String, Option<Vec<u8>>)>;
        let log: Arc<Mutex<Log>> = Arc::default();
        let l = Arc::clone(&log);
        let files = MemoryFiles::new(
            vec![("settings.ini".into(), b"x=1\n".to_vec())],
            Some(Box::new(move |p, d| {
                l.lock().unwrap().push((p.to_owned(), d.map(<[u8]>::to_vec)));
            })),
        );
        assert_eq!(files.kind(""), Some(Kind::Dir));
        assert_eq!(files.kind("settings.ini"), Some(Kind::File));
        files.write("MAPS/a.tmp", b"1").unwrap();
        files.rename("MAPS/a.tmp", "MAPS/a.map").unwrap();
        files.remove("settings.ini").unwrap();
        assert_eq!(files.list("").unwrap(), [("MAPS".to_owned(), Kind::Dir)]);
        assert_eq!(files.list("MAPS").unwrap(), [("a.map".to_owned(), Kind::File)]);
        assert!(files.write("MAPS", b"").is_err());
        assert_eq!(
            *log.lock().unwrap(),
            [
                ("MAPS/a.tmp".to_owned(), Some(b"1".to_vec())),
                ("MAPS/a.map".to_owned(), Some(b"1".to_vec())),
                ("MAPS/a.tmp".to_owned(), None),
                ("settings.ini".to_owned(), None),
            ]
        );
    }

    /// The CD's image, if `WIDGET_ISO` names one: its files are the
    /// extracted ones in `orig/`.
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn cd_image_is_the_extracted_cd() {
        let Some(iso) = std::env::var_os("WIDGET_ISO") else {
            eprintln!("skipped: set WIDGET_ISO to the CD's image");
            return;
        };
        let cd = CdImage::open_file(std::path::Path::new(&iso)).unwrap();
        let orig = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../orig");
        assert_eq!(cd.kind("win95/widget"), Some(Kind::Dir));
        assert_eq!(cd.kind("WIN95/WIDGET/Widget.exe"), Some(Kind::File));
        let mut compared = 0;
        for f in cd.files() {
            if let Ok(on_disk) = std::fs::read(orig.join(&f)) {
                assert!(cd.read(&f).unwrap() == on_disk, "{f} differs");
                compared += 1;
            }
        }
        assert!(compared > 20, "only {compared} compared");
    }
}
