//! The game's CD when the command line names none (a packaged app
//! started from its icon): an `.iso` or `orig` in the working directory,
//! as always; else the CD the player picked before; else the system's
//! file dialog asks for the CD image. The choice is kept in the port's
//! data directory (`platform::storage::data_dir`, `cd-path.txt`).
//! Natively only; the browser build's page asks for the image itself
//! (`web`).

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

/// The game's CD image or folder: `default` if it's there, else the one
/// picked before, else the one the player picks now. `None` if the
/// player cancels.
pub fn cd(default: &Path) -> Option<PathBuf> {
    if default.exists() {
        return Some(default.to_path_buf());
    }
    let kept = platform::storage::data_dir().map(|d| d.join("cd-path.txt"));
    let before = kept
        .as_deref()
        .and_then(|k| std::fs::read_to_string(k).ok())
        .map(|s| PathBuf::from(s.trim_end_matches(['\r', '\n'])))
        .filter(|p| p.exists());
    if before.is_some() {
        return before;
    }
    let picked = loop {
        let path = rfd::FileDialog::new()
            .set_title("Choose the Widget Workshop CD image")
            .add_filter("CD image", &["iso", "ISO"])
            .add_filter("All files", &["*"])
            .pick_file()?;
        match check(&path) {
            Ok(()) => break path,
            Err(e) => error_box(&format!("That isn't Widget Workshop's CD image ({e}). Choose another.")),
        }
    };
    if let Some(kept) = kept {
        let saved = kept
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&kept, format!("{}\n", picked.display())));
        if let Err(e) = saved {
            eprintln!("{}: {e}", kept.display());
        }
    }
    Some(picked)
}

/// Whether `path` is the game's CD image; why not if it isn't.
fn check(path: &Path) -> Result<(), String> {
    let cd = crate::cd::open(path)?;
    if crate::cd::has_fonts(cd.as_ref()) {
        Ok(())
    } else {
        Err(format!("no {} on it", engine::assets::FONT_DIR))
    }
}

/// `message` in a message box, as an error.
pub fn error_box(message: &str) {
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Widget Workshop")
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

/// `message` on the standard error, and in a message box too when that
/// goes nowhere to be read (started from the icon).
pub fn fatal(message: &str) {
    eprintln!("{message}");
    if !std::io::stderr().is_terminal() {
        error_box(message);
    }
}
