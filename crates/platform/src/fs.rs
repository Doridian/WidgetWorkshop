//! The files the port reads and writes, as `std::fs` for the few calls
//! it uses, with mount points over it: a [`Source`] mounted at a path
//! answers for everything under it. Natively nothing is mounted unless
//! the game's folder is a CD image ([`crate::data`]), so these are
//! `std::fs`. In the browser there's no file system: the CD image is
//! mounted where the game's folder would be, and the browser's storage
//! where the saves and the settings go (`docs/web.md`).

use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, PoisonError, RwLock};

/// Whether a path is a file or a directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    File,
    Dir,
}

/// Files mounted at a path. Paths given to it are relative to the mount
/// point, their parts joined with `/`; the empty path is its root.
pub trait Source: Send + Sync {
    /// The file `path`'s bytes.
    ///
    /// # Errors
    /// If there's no such file.
    fn read(&self, path: &str) -> io::Result<Vec<u8>>;

    /// What `path` is, `None` if it isn't there.
    fn kind(&self, path: &str) -> Option<Kind>;

    /// The names in the directory `path` and what each is.
    ///
    /// # Errors
    /// If there's no such directory.
    fn list(&self, path: &str) -> io::Result<Vec<(String, Kind)>>;

    /// Writes the file `path`, replacing it if it's there.
    ///
    /// # Errors
    /// If the source is read-only, or the write fails.
    fn write(&self, _path: &str, _data: &[u8]) -> io::Result<()> {
        Err(read_only())
    }

    /// Removes the file `path`.
    ///
    /// # Errors
    /// If the source is read-only, or there's no such file.
    fn remove(&self, _path: &str) -> io::Result<()> {
        Err(read_only())
    }

    /// Renames the file `from` to `to`, replacing `to`.
    ///
    /// # Errors
    /// If the source is read-only, or there's no `from`.
    fn rename(&self, _from: &str, _to: &str) -> io::Result<()> {
        Err(read_only())
    }

    /// Makes the directory `path` and its parents.
    ///
    /// # Errors
    /// If the source is read-only.
    fn create_dir_all(&self, _path: &str) -> io::Result<()> {
        Err(read_only())
    }
}

fn read_only() -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, "read-only")
}

type Mounts = Vec<(PathBuf, Arc<dyn Source>)>;

static MOUNTS: RwLock<Mounts> = RwLock::new(Vec::new());

/// Mounts `source` at `at`: everything below `at` comes from it from now
/// on. A later mount at the same path replaces it.
pub fn mount(at: impl Into<PathBuf>, source: Arc<dyn Source>) {
    let at = at.into();
    let mut mounts = MOUNTS.write().unwrap_or_else(PoisonError::into_inner);
    mounts.retain(|(p, _)| *p != at);
    mounts.push((at, source));
}

/// The source `path` is under and the path in it, the longest mount
/// point first.
fn resolve(path: &Path) -> Option<(Arc<dyn Source>, String)> {
    let mounts = MOUNTS.read().unwrap_or_else(PoisonError::into_inner);
    if mounts.is_empty() {
        return None;
    }
    let (at, source) = mounts
        .iter()
        .filter(|(at, _)| path.starts_with(at))
        .max_by_key(|(at, _)| at.components().count())?;
    let rest = path.strip_prefix(at).ok()?;
    let mut parts: Vec<String> = Vec::new();
    for c in rest.components() {
        match c {
            Component::Normal(p) => parts.push(p.to_string_lossy().into_owned()),
            Component::ParentDir => {
                parts.pop();
            }
            _ => {}
        }
    }
    Some((Arc::clone(source), parts.join("/")))
}

/// A directory's entry ([`read_dir`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    path: PathBuf,
    kind: Option<Kind>,
}

impl DirEntry {
    /// The entry's full path.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    /// Its name.
    #[must_use]
    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Whether it's a file (not following a symbolic link, as
    /// `std::fs::DirEntry::file_type`).
    #[must_use]
    pub fn is_file(&self) -> bool {
        self.kind == Some(Kind::File)
    }

    /// Whether it's a directory (not following a symbolic link).
    #[must_use]
    pub fn is_dir(&self) -> bool {
        self.kind == Some(Kind::Dir)
    }
}

/// As `std::fs::read`.
///
/// # Errors
/// If there's no such file or it can't be read.
pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    match resolve(path.as_ref()) {
        Some((s, p)) => s.read(&p),
        None => std::fs::read(path),
    }
}

/// As `std::fs::read_to_string`.
///
/// # Errors
/// If there's no such file or it isn't UTF-8.
pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    match resolve(path.as_ref()) {
        Some((s, p)) => String::from_utf8(s.read(&p)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
        None => std::fs::read_to_string(path),
    }
}

/// As `std::fs::write`.
///
/// # Errors
/// If it can't be written.
pub fn write(path: impl AsRef<Path>, data: impl AsRef<[u8]>) -> io::Result<()> {
    match resolve(path.as_ref()) {
        Some((s, p)) => s.write(&p, data.as_ref()),
        None => std::fs::write(path, data),
    }
}

/// As `std::fs::rename`, for files.
///
/// # Errors
/// If there's no `from`, or it can't be renamed.
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    match (resolve(from.as_ref()), resolve(to.as_ref())) {
        (Some((s, f)), Some((t, p))) if Arc::ptr_eq(&s, &t) => s.rename(&f, &p),
        (None, None) => std::fs::rename(from, to),
        _ => Err(io::Error::new(io::ErrorKind::Unsupported, "a rename across mounts")),
    }
}

/// As `std::fs::remove_file`.
///
/// # Errors
/// If there's no such file, or it can't be removed.
pub fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
    match resolve(path.as_ref()) {
        Some((s, p)) => s.remove(&p),
        None => std::fs::remove_file(path),
    }
}

/// As `std::fs::create_dir_all`.
///
/// # Errors
/// If it can't be made.
pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    match resolve(path.as_ref()) {
        Some((s, p)) => s.create_dir_all(&p),
        None => std::fs::create_dir_all(path),
    }
}

/// As `std::fs::read_dir`, the entries collected.
///
/// # Errors
/// If there's no such directory.
pub fn read_dir(path: impl AsRef<Path>) -> io::Result<Vec<DirEntry>> {
    let path = path.as_ref();
    match resolve(path) {
        Some((s, p)) => Ok(s
            .list(&p)?
            .into_iter()
            .map(|(name, kind)| DirEntry {
                path: path.join(name),
                kind: Some(kind),
            })
            .collect()),
        None => Ok(std::fs::read_dir(path)?
            .filter_map(Result::ok)
            .map(|e| DirEntry {
                path: e.path(),
                kind: e.file_type().ok().and_then(|t| {
                    if t.is_file() {
                        Some(Kind::File)
                    } else if t.is_dir() {
                        Some(Kind::Dir)
                    } else {
                        None
                    }
                }),
            })
            .collect()),
    }
}

/// As `Path::is_file`.
#[must_use]
pub fn is_file(path: impl AsRef<Path>) -> bool {
    match resolve(path.as_ref()) {
        Some((s, p)) => s.kind(&p) == Some(Kind::File),
        None => path.as_ref().is_file(),
    }
}

/// As `Path::is_dir`.
#[must_use]
pub fn is_dir(path: impl AsRef<Path>) -> bool {
    match resolve(path.as_ref()) {
        Some((s, p)) => s.kind(&p) == Some(Kind::Dir),
        None => path.as_ref().is_dir(),
    }
}

/// As `Path::exists`.
#[must_use]
pub fn exists(path: impl AsRef<Path>) -> bool {
    match resolve(path.as_ref()) {
        Some((s, p)) => s.kind(&p).is_some(),
        None => path.as_ref().exists(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::MemoryFiles;

    #[test]
    fn mounted_files() {
        let at = PathBuf::from("/widget-fs-test-mount");
        mount(&at, Arc::new(MemoryFiles::new(Vec::new(), None)));
        let dir = at.join("MAPS");
        assert!(!exists(&dir));
        create_dir_all(&dir).unwrap();
        assert!(is_dir(&dir));
        write(dir.join("a.map.tmp"), b"abc").unwrap();
        rename(dir.join("a.map.tmp"), dir.join("a.map")).unwrap();
        assert_eq!(read(dir.join("a.map")).unwrap(), b"abc");
        assert!(read(dir.join("a.map.tmp")).is_err());
        let names: Vec<String> = read_dir(&dir).unwrap().iter().map(DirEntry::file_name).collect();
        assert_eq!(names, ["a.map"]);
        assert!(read_dir(&at).unwrap()[0].is_dir());
        assert_eq!(read_to_string(at.join("x/../MAPS/a.map")).unwrap(), "abc");
        remove_file(dir.join("a.map")).unwrap();
        assert!(!is_file(dir.join("a.map")));
        // Outside the mount: std::fs.
        assert!(is_dir(std::env::temp_dir()));
    }
}
