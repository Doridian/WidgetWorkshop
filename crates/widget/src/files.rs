//! The editor's files ([`engine::files::Files`]) through `platform::fs`:
//! `std::fs` natively, what's mounted in the browser (its storage, where
//! the dialogs start and stay).

use std::io;
use std::path::{Path, PathBuf};

/// `platform::fs` as the editor's file system.
pub struct PlatformFiles;

impl engine::files::Files for PlatformFiles {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        platform::fs::read(path)
    }

    fn write(&mut self, path: &Path, data: &[u8]) -> io::Result<()> {
        platform::fs::write(path, data)
    }

    fn list(&self, path: &Path) -> io::Result<Vec<(String, bool)>> {
        // Symbolic links followed: what they point at.
        Ok(platform::fs::read_dir(path)?
            .into_iter()
            .filter_map(|e| {
                let p = e.path();
                let dir = e.is_dir() || !e.is_file() && platform::fs::is_dir(&p);
                (dir || e.is_file() || platform::fs::is_file(&p)).then(|| (e.file_name(), dir))
            })
            .collect())
    }

    fn is_dir(&self, path: &Path) -> bool {
        platform::fs::is_dir(path)
    }

    fn is_file(&self, path: &Path) -> bool {
        platform::fs::is_file(path)
    }

    fn current_dir(&self) -> PathBuf {
        if cfg!(target_arch = "wasm32") {
            return PathBuf::from(platform::storage::WEB_STORAGE);
        }
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
    }

    fn roots(&self) -> Vec<PathBuf> {
        if cfg!(target_arch = "wasm32") {
            vec![PathBuf::from(platform::storage::WEB_STORAGE)]
        } else {
            engine::files::StdFiles.roots()
        }
    }
}
