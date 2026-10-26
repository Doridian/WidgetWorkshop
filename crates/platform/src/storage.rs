//! Where the port keeps the player's files: saved games (and later the
//! high scores) in the platform's data directory, and its settings
//! ([`crate::settings`]) in the config directory. The
//! original writes its saves into the game's own `MAPS\` directory; the
//! port leaves the game's files alone and only reads from there. In the
//! browser both are [`WEB_STORAGE`], where the browser's storage is
//! mounted (`crate::fs`, `docs/web.md`).

use std::path::PathBuf;

/// Where the browser build mounts the browser's storage: its data and
/// config directory.
pub const WEB_STORAGE: &str = "/storage";

/// The directory the port's own files go in: `$XDG_DATA_HOME/widget-workshop`
/// (or `~/.local/share/widget-workshop`) on Linux and the other Unixes,
/// `~/Library/Application Support/Widget Workshop` on macOS and
/// `%APPDATA%\Widget Workshop` on Windows. `None` if the environment
/// doesn't say where the home directory is. [`WEB_STORAGE`] in the
/// browser.
#[must_use]
pub fn data_dir() -> Option<PathBuf> {
    if cfg!(target_arch = "wasm32") {
        return Some(PathBuf::from(WEB_STORAGE));
    }
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(target_os = "windows") {
        var("APPDATA").map(|d| d.join("Widget Workshop"))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library/Application Support/Widget Workshop"))
    } else {
        var("XDG_DATA_HOME")
            .or_else(|| var("HOME").map(|h| h.join(".local/share")))
            .map(|d| d.join("widget-workshop"))
    }
}

/// Where saved games go: `MAPS` in [`data_dir`], like the game's own
/// `MAPS\`.
#[must_use]
pub fn save_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join("MAPS"))
}

/// The directory the port's settings go in: `$XDG_CONFIG_HOME/widget-workshop`
/// (or `~/.config/widget-workshop`) on Linux and the other Unixes,
/// `~/Library/Preferences/Widget Workshop` on macOS and
/// `%APPDATA%\Widget Workshop` on Windows. `None` if the environment
/// doesn't say where the home directory is. [`WEB_STORAGE`] in the
/// browser.
#[must_use]
pub fn config_dir() -> Option<PathBuf> {
    if cfg!(target_arch = "wasm32") {
        return Some(PathBuf::from(WEB_STORAGE));
    }
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(target_os = "windows") {
        var("APPDATA").map(|d| d.join("Widget Workshop"))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library/Preferences/Widget Workshop"))
    } else {
        var("XDG_CONFIG_HOME")
            .or_else(|| var("HOME").map(|h| h.join(".config")))
            .map(|d| d.join("widget-workshop"))
    }
}
