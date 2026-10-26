//! Whether the system is in dark mode, and when that changes: the XDG
//! desktop portal's `color-scheme` on Linux and the BSDs, the system's
//! setting on Windows and macOS, `prefers-color-scheme` in the browser.

use dark_light::{Mode, Watcher};

/// The system's dark mode, watched for changes.
pub struct SystemTheme {
    dark: bool,
    watcher: Option<Watcher>,
}

impl SystemTheme {
    /// The system's dark mode now; light where there's no telling.
    #[must_use]
    pub fn new() -> SystemTheme {
        SystemTheme {
            dark: dark_light::detect().is_ok_and(|m| m == Mode::Dark),
            watcher: dark_light::subscribe().ok(),
        }
    }

    #[must_use]
    pub fn dark(&self) -> bool {
        self.dark
    }

    /// The new dark mode if it changed since last asked.
    pub fn changed(&mut self) -> Option<bool> {
        let dark = self.watcher.as_ref()?.try_iter().last()? == Mode::Dark;
        (dark != self.dark).then(|| {
            self.dark = dark;
            dark
        })
    }
}

impl Default for SystemTheme {
    fn default() -> Self {
        SystemTheme::new()
    }
}
