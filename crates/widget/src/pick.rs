//! The game's CD when the command line names none (a packaged app
//! started from its icon): an `.iso` or `orig` in the working directory,
//! as always; else the CD the player picked before; else the system's
//! file dialog asks for the CD image. The choice is kept in the port's
//! data directory (`platform::storage::data_dir`, `cd-path.txt`).
//! Settings ▸ Change CD... asks again ([`choose`]).
//!
//! Where there's no file dialog to ask with ([`has_dialog`]: on Linux and
//! the BSDs neither xdg-desktop-portal's `FileChooser` nor zenity, as on a
//! minimal system or in the `AppImage` catalog's sandbox), a window of the
//! port's own asks for the CD image to be dropped on it ([`DropScreen`],
//! its text in the bundled fonts); Change CD... shows the same in the
//! game's window.
//!
//! Natively only; the browser build's page asks for the image itself
//! (`web`).

use std::cell::RefCell;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use engine::gfx::{IRect, Surface};
use formats::fnt::Font;
use platform::{App, Cursor, Frame, Input, WindowOptions};

/// The game's CD image or folder: `default` if it's there, else the one
/// picked before, else the one the player picks now (in the file dialog,
/// or dropped on a window opened as `window` says where there's none).
/// `None` if the player cancels.
pub fn cd(default: &Path, window: &WindowOptions) -> Option<PathBuf> {
    if default.exists() {
        return Some(default.to_path_buf());
    }
    let before = kept()
        .and_then(|k| std::fs::read_to_string(k).ok())
        .map(|s| PathBuf::from(s.trim_end_matches(['\r', '\n'])))
        .filter(|p| p.exists());
    if before.is_some() {
        return before;
    }
    if has_dialog() { choose() } else { dropped(window) }
}

/// Where the picked CD is kept.
fn kept() -> Option<PathBuf> {
    platform::storage::data_dir().map(|d| d.join("cd-path.txt"))
}

/// Keeps `picked` for next time.
fn keep(picked: &Path) {
    if let Some(kept) = kept() {
        let saved = kept
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&kept, format!("{}\n", picked.display())));
        if let Err(e) = saved {
            eprintln!("{}: {e}", kept.display());
        }
    }
}

/// The CD image the player picks in the system's file dialog, kept for
/// next time. `None` if the player cancels.
pub fn choose() -> Option<PathBuf> {
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
    keep(&picked);
    Some(picked)
}

/// Whether `path` is the game's CD image; why not if it isn't.
fn check(path: &Path) -> Result<(), String> {
    let cd = crate::cd::open(path)?;
    if crate::cd::has_game(cd.as_ref()) {
        Ok(())
    } else {
        Err("no WWGADGET.DLL on it".into())
    }
}

/// Whether rfd has a file dialog (and a message box) to show. On Linux
/// and the BSDs it asks xdg-desktop-portal's `FileChooser` on the session
/// bus, else runs zenity; with neither, `pick_file` returns as if
/// cancelled and the message box doesn't show. Elsewhere the system's
/// own dialogs are always there.
pub fn has_dialog() -> bool {
    static HAS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *HAS.get_or_init(|| portal() || on_path("zenity"))
}

/// Whether the session bus has the portal's `FileChooser` (its `version`,
/// asked as rfd's call would: starting the portal if it isn't running).
#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd"
))]
fn portal() -> bool {
    zbus::block_on(async {
        let Ok(conn) = zbus::connection::Builder::session()
            .method_timeout(std::time::Duration::from_secs(10))
            .build()
            .await
        else {
            return false;
        };
        conn.call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("org.freedesktop.portal.FileChooser", "version"),
        )
        .await
        .is_ok()
    })
}

/// The system's own dialog.
#[cfg(not(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd"
)))]
fn portal() -> bool {
    true
}

/// Whether `name` is an executable on `PATH`.
#[cfg(unix)]
fn on_path(name: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::env::var_os("PATH").is_some_and(|p| {
        std::env::split_paths(&p).any(|d| {
            d.join(name)
                .metadata()
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
    })
}

#[cfg(not(unix))]
fn on_path(_name: &str) -> bool {
    false
}

/// `message` in a message box, as an error (nowhere without a dialog).
pub fn error_box(message: &str) {
    if !has_dialog() {
        return;
    }
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

/// The CD image dropped on a window of its own, opened as `window` says,
/// kept for next time. `None` if the window is closed first.
fn dropped(window: &WindowOptions) -> Option<PathBuf> {
    let picked = Rc::new(RefCell::new(None));
    let app = DropWindow {
        screen: DropScreen::new(false, platform::theme::SystemTheme::new().dark()),
        picked: Rc::clone(&picked),
        closed: false,
    };
    if let Err(e) = platform::run("Widget Workshop", window.clone(), app) {
        eprintln!("{e}");
    }
    picked.take()
}

/// The window [`dropped`] asks with.
struct DropWindow {
    screen: DropScreen,
    picked: Rc<RefCell<Option<PathBuf>>>,
    closed: bool,
}

impl App for DropWindow {
    fn update(&mut self, _elapsed: web_time::Duration) {}

    fn input(&mut self, input: Input) {
        if input == Input::Close {
            self.closed = true;
        }
    }

    fn frame(&self) -> Frame<'_> {
        self.screen.frame()
    }

    fn finished(&self) -> bool {
        self.closed || self.picked.borrow().is_some()
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.screen.resize(width, height);
    }

    fn dropped_path(&mut self, path: &Path) -> bool {
        if self.screen.take(path) {
            *self.picked.borrow_mut() = Some(path.to_path_buf());
        }
        true
    }
}

/// The colours of [`DropScreen`]'s palette.
const BACK: u8 = 0;
const TEXT: u8 = 1;
const DIM: u8 = 2;
const ERROR: u8 = 3;

/// Asks for the CD image to be dropped on the window, where there's no
/// file dialog: the port's own picture of it, its text in the bundled
/// fonts (`engine::assets::bundled_fonts`), with why the last one dropped
/// isn't the game's.
pub struct DropScreen {
    surface: Surface,
    palette: [[u8; 3]; 256],
    fonts: Vec<Font>,
    /// Change CD..., in the game's window: Esc goes back to the game.
    in_game: bool,
    error: Option<String>,
}

impl DropScreen {
    #[must_use]
    pub fn new(in_game: bool, dark: bool) -> DropScreen {
        let mut palette = [[0; 3]; 256];
        let colours = if dark {
            [
                [0x20, 0x20, 0x24],
                [0xf0, 0xf0, 0xf0],
                [0xa8, 0xa8, 0xb0],
                [0xff, 0x70, 0x70],
            ]
        } else {
            [
                [0xf4, 0xf4, 0xf0],
                [0x10, 0x10, 0x10],
                [0x60, 0x60, 0x68],
                [0xc0, 0x10, 0x10],
            ]
        };
        palette[..4].copy_from_slice(&colours);
        let mut s = DropScreen {
            surface: Surface::new(1, 1),
            palette,
            fonts: engine::assets::bundled_fonts(),
            in_game,
            error: None,
        };
        s.draw();
        s
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface.resize(width.max(1), height.max(1));
        self.draw();
    }

    /// `path` dropped: whether it's the game's CD image, kept for next
    /// time if it is; if not, the screen says why.
    pub fn take(&mut self, path: &Path) -> bool {
        match check(path) {
            Ok(()) => {
                keep(path);
                true
            }
            Err(e) => {
                let name = path
                    .file_name()
                    .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
                self.set_error(format!("{name} isn't Widget Workshop's CD image ({e}). Drop another."));
                false
            }
        }
    }

    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.draw();
    }

    #[must_use]
    pub fn frame(&self) -> Frame<'_> {
        Frame {
            width: u32::try_from(self.surface.width).unwrap_or(0),
            height: u32::try_from(self.surface.height).unwrap_or(0),
            pixels: &self.surface.pixels,
            palette: &self.palette,
            overlay: None,
            cursor: Cursor::Default,
            cursor_image: None,
            title: Some("Widget Workshop"),
        }
    }

    /// Widget Sans, the size closest to `height` pixels.
    fn font(&self, height: u16) -> Option<Font> {
        self.fonts
            .iter()
            .filter(|f| f.face == "Widget Sans" && f.weight < 600)
            .min_by_key(|f| f.height.abs_diff(height))
            .cloned()
    }

    fn draw(&mut self) {
        let (w, h) = (self.surface.width, self.surface.height);
        self.surface.fill(IRect::new(0, 0, w, h), BACK);
        let (Some(big), Some(small)) = (self.font(24), self.font(16)) else {
            return;
        };
        let width = (w - 64).clamp(1, 560);
        let mut lines: Vec<(&Font, u8, Vec<u8>)> = Vec::new();
        let mut add = |font, colour, text: &str| {
            for l in wrap(font, text, width) {
                lines.push((font, colour, l));
            }
        };
        add(&big, TEXT, "Widget Workshop");
        add(&small, TEXT, "");
        add(
            &small,
            TEXT,
            "Drop the game's CD image (.iso) on this window, or a folder with its files.",
        );
        add(&small, TEXT, "");
        add(
            &small,
            DIM,
            "There's no file dialog here to choose it with (no xdg-desktop-portal, no zenity).",
        );
        add(
            &small,
            DIM,
            if self.in_game {
                "Esc goes back to the game."
            } else {
                "It can be given on the command line too. Closing the window quits."
            },
        );
        if let Some(e) = &self.error {
            add(&small, TEXT, "");
            add(&small, ERROR, e);
        }
        let step = |f: &Font| i32::from(f.height) + 2;
        let total: i32 = lines.iter().map(|(f, ..)| step(f)).sum();
        let mut y = ((h - total) / 2).max(24);
        for (font, colour, text) in &lines {
            let x = (w - i32::try_from(font.width(text)).unwrap_or(0)) / 2;
            self.surface.text(font, x, y, text, *colour);
            y += step(font);
        }
        // The drop zone, dashed.
        let r = IRect::new(12, 12, w - 12, h - 12);
        if !r.is_empty() {
            for x in (r.x0..r.x1).filter(|x| (x / 6) % 2 == 0) {
                self.surface.pset(x, r.y0, DIM);
                self.surface.pset(x, r.y1 - 1, DIM);
            }
            for y in (r.y0..r.y1).filter(|y| (y / 6) % 2 == 0) {
                self.surface.pset(r.x0, y, DIM);
                self.surface.pset(r.x1 - 1, y, DIM);
            }
        }
    }
}

/// `text` as Windows-1252 lines of `font` no wider than `width`, broken
/// at spaces.
fn wrap(font: &Font, text: &str, width: i32) -> Vec<Vec<u8>> {
    let fits = |l: &[u8]| i64::from(font.width(l)) <= i64::from(width);
    let mut lines = Vec::new();
    let mut line: Vec<u8> = Vec::new();
    for word in engine::text::encode(text).split(|&c| c == b' ') {
        let mut longer = line.clone();
        if !longer.is_empty() {
            longer.push(b' ');
        }
        longer.extend_from_slice(word);
        if line.is_empty() || fits(&longer) {
            line = longer;
        } else {
            lines.push(std::mem::replace(&mut line, word.to_vec()));
        }
    }
    lines.push(line);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_at_spaces() {
        let s = DropScreen::new(false, false);
        let font = s.font(16).unwrap();
        let one = wrap(&font, "Drop the game's CD image", 10_000);
        assert_eq!(one, vec![b"Drop the game's CD image".to_vec()]);
        let narrow = i32::try_from(font.width(b"Drop the game's")).unwrap();
        let lines = wrap(&font, "Drop the game's CD image", narrow);
        assert_eq!(lines, vec![b"Drop the game's".to_vec(), b"CD image".to_vec()]);
        // A word wider than the line has one of its own.
        assert_eq!(wrap(&font, "Drop", 1), vec![b"Drop".to_vec()]);
    }
}
