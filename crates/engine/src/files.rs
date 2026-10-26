//! The files the editor opens and saves: the player's own, through a
//! [`Files`] the `widget` crate gives it (`platform::fs`, so the browser's
//! storage too), and the CD's sample widgets (`FUNWIDGE`, read with the
//! rest of the CD into [`crate::assets::Assets::samples`]), which the file
//! dialogs show as the folder [`SAMPLES`] of a drive of their own, [`CD`].
//! The original ran from the CD, where `FUNWIDGE` was a subfolder of the
//! folder its dialogs start in.

use std::io;
use std::path::{Component, Path, PathBuf};

/// The CD's drive in the file dialogs (not a real path).
pub const CD: &str = "CD:";
/// The sample widgets' folder on it.
pub const SAMPLES: &str = "FUNWIDGE";

/// The file system as the editor uses it.
pub trait Files {
    /// The file `path`'s bytes.
    ///
    /// # Errors
    /// If there's no such file or it can't be read.
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;

    /// Writes the file `path`, replacing it.
    ///
    /// # Errors
    /// If it can't be written.
    fn write(&mut self, path: &Path, data: &[u8]) -> io::Result<()>;

    /// The folder `path`'s entries: each name and whether it's a folder.
    ///
    /// # Errors
    /// If there's no such folder.
    fn list(&self, path: &Path) -> io::Result<Vec<(String, bool)>>;

    fn is_dir(&self, path: &Path) -> bool;

    fn is_file(&self, path: &Path) -> bool;

    /// The current folder (`_getcwd`): where the dialogs start when the
    /// profile has no `WidgetHome`.
    fn current_dir(&self) -> PathBuf;

    /// The tops of the folder tree (the drives), for "Look in".
    fn roots(&self) -> Vec<PathBuf>;
}

/// `std::fs`: the default, and the tests'.
#[derive(Debug, Clone, Copy, Default)]
pub struct StdFiles;

impl Files for StdFiles {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    fn write(&mut self, path: &Path, data: &[u8]) -> io::Result<()> {
        std::fs::write(path, data)
    }

    fn list(&self, path: &Path) -> io::Result<Vec<(String, bool)>> {
        Ok(std::fs::read_dir(path)?
            .filter_map(Result::ok)
            .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path().is_dir()))
            .collect())
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn current_dir(&self) -> PathBuf {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
    }

    fn roots(&self) -> Vec<PathBuf> {
        if cfg!(windows) {
            (b'A'..=b'Z')
                .map(|d| PathBuf::from(format!("{}:\\", char::from(d))))
                .filter(|p| p.is_dir())
                .collect()
        } else {
            vec![PathBuf::from("/")]
        }
    }
}

/// The parts of `path` after [`CD`], if it's on the CD.
#[must_use]
pub fn on_cd(path: &Path) -> Option<Vec<String>> {
    let mut c = path.components();
    match c.next()? {
        Component::Normal(d) if d.eq_ignore_ascii_case(CD) => {}
        _ => return None,
    }
    Some(c.map(|p| p.as_os_str().to_string_lossy().into_owned()).collect())
}

/// `path` with `.` and `..` worked out (`..` at the top stays there).
#[must_use]
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if out.file_name().is_some() {
                    out.pop();
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// Whether `path` names its drive and folder (Borland's `fnsplit` flags
/// `DRIVE | DIRECTORY`, what the dialogs check before remembering a
/// folder): an absolute path, or one on the CD.
#[must_use]
pub fn has_folder(path: &Path) -> bool {
    path.is_absolute() || on_cd(path).is_some_and(|p| !p.is_empty())
}

/// The files the dialogs browse: `files` and the CD's `samples`.
#[derive(Clone, Copy)]
pub struct Fs<'a> {
    pub files: &'a dyn Files,
    pub samples: &'a [(String, Vec<u8>)],
}

impl Fs<'_> {
    /// Folder `path`'s entries.
    ///
    /// # Errors
    /// If there's no such folder.
    pub fn list(&self, path: &Path) -> io::Result<Vec<(String, bool)>> {
        match on_cd(path).as_deref() {
            Some([]) if !self.samples.is_empty() => Ok(vec![(SAMPLES.to_owned(), true)]),
            Some([d]) if d.eq_ignore_ascii_case(SAMPLES) && !self.samples.is_empty() => {
                Ok(self.samples.iter().map(|(n, _)| (n.clone(), false)).collect())
            }
            Some(_) => Err(io::ErrorKind::NotFound.into()),
            None => self.files.list(path),
        }
    }

    #[must_use]
    pub fn is_dir(&self, path: &Path) -> bool {
        match on_cd(path).as_deref() {
            Some([]) => !self.samples.is_empty(),
            Some([d]) => d.eq_ignore_ascii_case(SAMPLES) && !self.samples.is_empty(),
            Some(_) => false,
            None => self.files.is_dir(path),
        }
    }

    #[must_use]
    pub fn is_file(&self, path: &Path) -> bool {
        self.sample(path).is_some() || on_cd(path).is_none() && self.files.is_file(path)
    }

    /// The sample at `path`, if that's where one is.
    #[must_use]
    pub fn sample(&self, path: &Path) -> Option<&[u8]> {
        match on_cd(path).as_deref() {
            Some([d, name]) if d.eq_ignore_ascii_case(SAMPLES) => self
                .samples
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .map(|(_, b)| b.as_slice()),
            _ => None,
        }
    }

    /// The file `path`'s bytes, a sample's from memory.
    ///
    /// # Errors
    /// If there's no such file or it can't be read.
    pub fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        match (self.sample(path), on_cd(path)) {
            (Some(b), _) => Ok(b.to_vec()),
            (None, Some(_)) => Err(io::ErrorKind::NotFound.into()),
            (None, None) => self.files.read(path),
        }
    }

    /// The drives "Look in" lists: the file system's, then the CD's if it
    /// has samples.
    #[must_use]
    pub fn roots(&self) -> Vec<PathBuf> {
        let mut r = self.files.roots();
        if !self.samples.is_empty() {
            r.push(PathBuf::from(CD));
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cd_folder() {
        let samples = vec![("CLOCK.WGT".to_owned(), b"FORM".to_vec())];
        let fs = Fs {
            files: &StdFiles,
            samples: &samples,
        };
        let cd = PathBuf::from(CD);
        assert!(fs.is_dir(&cd) && fs.is_dir(&cd.join("funwidge")));
        assert_eq!(fs.list(&cd).unwrap(), [("FUNWIDGE".to_owned(), true)]);
        let clock = cd.join(SAMPLES).join("clock.wgt");
        assert!(fs.is_file(&clock) && !fs.is_dir(&clock));
        assert_eq!(fs.read(&clock).unwrap(), b"FORM");
        assert!(fs.read(&cd.join(SAMPLES).join("NONE.WGT")).is_err());
        assert!(has_folder(&clock) && !has_folder(&cd) && !has_folder(Path::new("a.wgt")));
        assert_eq!(normalize(Path::new("/a/b/../c/./d")), PathBuf::from("/a/c/d"));
        assert_eq!(normalize(&cd.join(SAMPLES).join("..")), cd);
    }
}
