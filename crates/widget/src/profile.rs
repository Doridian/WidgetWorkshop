//! The game's profile (`%WINDIR%\WIDGET.INI`, `[Widget Workshop]`) as a
//! `WIDGET.INI` in the port's settings folder
//! (`platform::storage::config_dir`). Only the solved puzzles (`Done=`)
//! are kept there; the CD's own `WIDGET.INI` is never read or written.

use std::path::PathBuf;

use engine::puzzle::{Profile, SECTION};

/// The profile's file name.
pub const FILE: &str = "WIDGET.INI";

/// The profile in a file.
pub struct IniProfile(pub PathBuf);

impl IniProfile {
    /// `WIDGET.INI` in the settings folder, if there is one.
    #[must_use]
    pub fn default_file() -> Option<IniProfile> {
        platform::storage::config_dir().map(|d| IniProfile(d.join(FILE)))
    }
}

impl Profile for IniProfile {
    fn get(&self, key: &str) -> Option<String> {
        let text = platform::fs::read_to_string(&self.0).ok()?;
        formats::ini::get(&text, SECTION, key)
    }

    fn set(&mut self, key: &str, value: &str) {
        let text = platform::fs::read_to_string(&self.0).unwrap_or_default();
        let text = formats::ini::set(&text, SECTION, key, value);
        let written = self
            .0
            .parent()
            .map_or(Ok(()), platform::fs::create_dir_all)
            .and_then(|()| platform::fs::write(&self.0, text));
        if let Err(e) = written {
            eprintln!("{}: {e}", self.0.display());
        }
    }
}
