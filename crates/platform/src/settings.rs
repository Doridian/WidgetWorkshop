//! The port's own settings file, separate from anything the original
//! reads: the window's size and position, whether it was fullscreen and
//! the UI scale,
//! remembered from one run to the next. It lives in the platform's
//! config directory ([`crate::storage::config_dir`]) as `settings.ini`,
//! one `key=value` per line:
//!
//! ```text
//! window_width=800
//! window_height=600
//! window_x=120
//! window_y=80
//! fullscreen=false
//! ui_scale=auto
//! ```
//!
//! `ui_scale` is `auto` (the display's scale factor) or how many window
//! pixels make one of the game's, such as `2` or `1.5` ([`UiScale`]).
//! Sizes are in logical pixels (the desktop's points, before the
//! display's scale factor), positions in physical pixels as the window
//! system reports them. Unknown keys and lines that don't parse are
//! skipped, missing ones keep their defaults.
//!
//! The enhancements' switches ([`EnhanceSettings`]) are the player's,
//! written by hand or by the game's Settings window
//! ([`EnhanceSettings::save`]), which has the one for all of them,
//! `original`:
//!
//! ```text
//! original=false
//! enhance.window_stacking=off
//! ```
//!
//! The player's language (`game::lang`, `docs/language.md`), written by
//! hand: `auto` (the default) follows the game folder's.
//!
//! ```text
//! language=en
//! ```
//!
//! Writing the window's state keeps every other line of the file, and
//! writing the switches keeps every line but theirs. A UI scale chosen in
//! the game is written at once ([`save_value`]).

use std::fmt;
use std::path::{Path, PathBuf};

/// How big the game is drawn: how many window pixels make one of its
/// desktop's (`crate::ui_scale`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum UiScale {
    /// The display's scale factor.
    #[default]
    Auto,
    /// This scale whatever the display.
    Fixed(f64),
}

impl UiScale {
    /// `auto` or a positive number; `None` for anything else.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.eq_ignore_ascii_case("auto") {
            return Some(UiScale::Auto);
        }
        text.parse::<f64>()
            .ok()
            .filter(|n| n.is_finite() && *n > 0.0)
            .map(UiScale::Fixed)
    }
}

impl fmt::Display for UiScale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UiScale::Auto => write!(f, "auto"),
            UiScale::Fixed(n) => write!(f, "{n}"),
        }
    }
}

/// What the settings file remembers about the window.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WindowSettings {
    /// The windowed client area's size, in logical pixels.
    pub size: Option<(u32, u32)>,
    /// The window's outer top left, where the window system has one.
    pub position: Option<(i32, i32)>,
    pub fullscreen: bool,
    pub ui_scale: Option<UiScale>,
}

impl WindowSettings {
    /// The settings in `text`.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut s = WindowSettings::default();
        let (mut width, mut height, mut x, mut y) = (None, None, None, None);
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "window_width" => width = value.parse::<u32>().ok().filter(|&w| w > 0),
                "window_height" => height = value.parse::<u32>().ok().filter(|&h| h > 0),
                "window_x" => x = value.parse::<i32>().ok(),
                "window_y" => y = value.parse::<i32>().ok(),
                "fullscreen" => s.fullscreen = value == "true",
                "ui_scale" => s.ui_scale = UiScale::parse(value),
                _ => {}
            }
        }
        s.size = width.zip(height);
        s.position = x.zip(y);
        s
    }

    /// The file's text.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut lines = Vec::new();
        if let Some((w, h)) = self.size {
            lines.push(format!("window_width={w}"));
            lines.push(format!("window_height={h}"));
        }
        if let Some((x, y)) = self.position {
            lines.push(format!("window_x={x}"));
            lines.push(format!("window_y={y}"));
        }
        lines.push(format!("fullscreen={}", self.fullscreen));
        if let Some(scale) = self.ui_scale {
            lines.push(format!("ui_scale={scale}"));
        }
        lines.join("\n") + "\n"
    }

    /// The settings in the file at `path`; the defaults if there's none
    /// or it can't be read.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        crate::fs::read_to_string(path).map_or_else(|_| WindowSettings::default(), |t| WindowSettings::parse(&t))
    }

    /// Writes the settings to `path`, making its directory.
    ///
    /// # Errors
    /// If the directory or the file can't be written.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            crate::fs::create_dir_all(dir)?;
        }
        // The other lines (the enhancements, comments) stay as they are.
        let others: String = crate::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.split_once('=').is_some_and(|(k, _)| WINDOW_KEYS.contains(&k.trim())))
            .flat_map(|l| [l, "\n"])
            .collect();
        let temp = path.with_extension("ini.tmp");
        crate::fs::write(&temp, others + &self.to_text())?;
        crate::fs::rename(&temp, path)
    }
}

/// The keys [`WindowSettings`] writes.
const WINDOW_KEYS: [&str; 6] = [
    "window_width",
    "window_height",
    "window_x",
    "window_y",
    "fullscreen",
    "ui_scale",
];

/// The enhancements' switches in the settings file: `original=true` (all
/// off) and `enhance.<key>=on|off` (also `1`/`0`, `true`/`false`,
/// `yes`/`no`), in the file's order. The keys mean nothing here; the game
/// resolves them (`game::enhance::Config::resolve`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnhanceSettings {
    pub original: bool,
    pub overrides: Vec<(String, bool)>,
    /// Lines with an `enhance.` key whose value isn't a switch.
    pub bad: Vec<String>,
}

impl EnhanceSettings {
    /// The switches in `text`.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut s = EnhanceSettings::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            if key == "original" {
                s.original = switch(value).unwrap_or(false);
            } else if let Some(k) = key.strip_prefix("enhance.") {
                match switch(value) {
                    Some(on) => s.overrides.push((k.to_owned(), on)),
                    None => s.bad.push(line.to_owned()),
                }
            }
        }
        s
    }

    /// The switches in the file at `path`; none if there's no file.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        crate::fs::read_to_string(path).map_or_else(|_| EnhanceSettings::default(), |t| EnhanceSettings::parse(&t))
    }

    /// `text` with switches changed: `original=true|false` if `original`
    /// says, `enhance.<key>=on|off` for each of `set`, and no
    /// `enhance.<key>` line for each of `remove`. A switch's first line
    /// takes the new value where it is and its other lines go; a switch
    /// without a line gets one at the end. Every other line stays as it
    /// is.
    #[must_use]
    pub fn edit(text: &str, original: Option<bool>, set: &[(&str, bool)], remove: &[&str]) -> String {
        let on_off = |on: bool| if on { "on" } else { "off" };
        // Each switch's new line, until it's written.
        let mut pending: Vec<(String, Option<String>)> = Vec::new();
        if let Some(o) = original {
            pending.push(("original".to_owned(), Some(format!("original={o}"))));
        }
        for &(key, on) in set {
            let k = format!("enhance.{key}");
            let line = format!("{k}={}", on_off(on));
            pending.retain(|(p, _)| *p != k);
            pending.push((k, Some(line)));
        }
        for &key in remove {
            let k = format!("enhance.{key}");
            if !pending.iter().any(|(p, _)| *p == k) {
                pending.push((k, None));
            }
        }
        let mut done: Vec<String> = Vec::new();
        let mut out = String::new();
        for line in text.lines() {
            let key = line.split_once('=').map(|(k, _)| k.trim().to_owned());
            let Some((k, new)) = key.and_then(|k| pending.iter().find(|(p, _)| *p == k).cloned()) else {
                out.push_str(line);
                out.push('\n');
                continue;
            };
            if !done.contains(&k) {
                if let Some(new) = new {
                    out.push_str(&new);
                    out.push('\n');
                }
                done.push(k);
            }
        }
        for (k, new) in pending {
            if let Some(new) = new.filter(|_| !done.contains(&k)) {
                out.push_str(&new);
                out.push('\n');
            }
        }
        out
    }

    /// Writes switches into the file at `path` ([`EnhanceSettings::edit`]),
    /// making its directory.
    ///
    /// # Errors
    /// If the directory or the file can't be written.
    pub fn save(path: &Path, original: Option<bool>, set: &[(&str, bool)], remove: &[&str]) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            crate::fs::create_dir_all(dir)?;
        }
        let text = crate::fs::read_to_string(path).unwrap_or_default();
        let temp = path.with_extension("ini.tmp");
        crate::fs::write(&temp, Self::edit(&text, original, set, remove))?;
        crate::fs::rename(&temp, path)
    }
}

/// `text` with `key=value`: the key's first line takes it where it is and
/// its other lines go, or it's added at the end. Every other line stays.
#[must_use]
pub fn edit_value(text: &str, key: &str, value: &str) -> String {
    let new = format!("{key}={value}");
    let mut lines: Vec<&str> = Vec::new();
    let mut done = false;
    for line in text.lines() {
        if line.split_once('=').is_none_or(|(k, _)| k.trim() != key) {
            lines.push(line);
        } else if !done {
            lines.push(&new);
            done = true;
        }
    }
    if !done {
        lines.push(&new);
    }
    lines.join("\n") + "\n"
}

/// Writes `key=value` into the file at `path` ([`edit_value`]), making its
/// directory.
///
/// # Errors
/// If the directory or the file can't be written.
pub fn save_value(path: &Path, key: &str, value: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        crate::fs::create_dir_all(dir)?;
    }
    let text = crate::fs::read_to_string(path).unwrap_or_default();
    let temp = path.with_extension("ini.tmp");
    crate::fs::write(&temp, edit_value(&text, key, value))?;
    crate::fs::rename(&temp, path)
}

/// The value of the last `key=value` line for `key` in `text`, trimmed.
#[must_use]
pub fn value(text: &str, key: &str) -> Option<String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.trim().to_owned())
        .next_back()
}

/// [`value`] in the file at `path`; none if there's no file.
#[must_use]
pub fn load_value(path: &Path, key: &str) -> Option<String> {
    value(&crate::fs::read_to_string(path).ok()?, key)
}

/// A switch's value: `on`/`off`, `1`/`0`, `true`/`false`, `yes`/`no`.
#[must_use]
pub fn switch(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "on" | "1" | "true" | "yes" => Some(true),
        "off" | "0" | "false" | "no" => Some(false),
        _ => None,
    }
}

/// Where the settings file is: `settings.ini` in the config directory.
#[must_use]
pub fn settings_file() -> Option<PathBuf> {
    crate::storage::config_dir().map(|d| d.join("settings.ini"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let s = WindowSettings {
            size: Some((1024, 700)),
            position: Some((-8, 30)),
            fullscreen: true,
            ui_scale: Some(UiScale::Fixed(1.5)),
        };
        assert_eq!(WindowSettings::parse(&s.to_text()), s);
        let auto = WindowSettings::parse("ui_scale=auto\n");
        assert_eq!(auto.ui_scale, Some(UiScale::Auto));
        assert_eq!(auto.to_text(), "fullscreen=false\nui_scale=auto\n");
        assert_eq!(WindowSettings::parse(""), WindowSettings::default());
        let odd = WindowSettings::parse("window_width=0\nwindow_height=5\n# x\nfullscreen=false\nwindow_x=3\n");
        assert_eq!(odd, WindowSettings::default());
        for bad in ["0", "-2", "big", "NaN", "inf"] {
            assert_eq!(UiScale::parse(bad), None, "{bad}");
        }
        assert_eq!(UiScale::parse(" 2 "), Some(UiScale::Fixed(2.0)));
    }

    #[test]
    fn values() {
        let text = "language=de\nwindow_width=3\n language = en \nnope\n";
        assert_eq!(value(text, "language").as_deref(), Some("en"));
        assert_eq!(value(text, "fullscreen"), None);
        assert_eq!(load_value(Path::new("/nonexistent/settings.ini"), "language"), None);
        let edited = edit_value("# x\nui_scale=2\nfullscreen=true\nui_scale=3\n", "ui_scale", "auto");
        assert_eq!(edited, "# x\nui_scale=auto\nfullscreen=true\n");
        assert_eq!(edit_value("a=1", "ui_scale", "1.5"), "a=1\nui_scale=1.5\n");
    }

    #[test]
    fn enhancements() {
        let e = EnhanceSettings::parse("window_width=3\noriginal=1\nenhance.window_stacking=on\nenhance.x=maybe\n");
        assert!(e.original);
        assert_eq!(e.overrides, [("window_stacking".to_owned(), true)]);
        assert_eq!(e.bad, ["enhance.x=maybe"]);
        assert_eq!(EnhanceSettings::parse(""), EnhanceSettings::default());
    }

    #[test]
    fn writing_switches() {
        let text = "# mine\nwindow_width=3\nenhance.big_map=off\nenhance.x=on\nenhance.big_map=on\noriginal=1\n";
        let edited = EnhanceSettings::edit(text, None, &[("big_map", true), ("music", false)], &[]);
        assert_eq!(
            edited,
            "# mine\nwindow_width=3\nenhance.big_map=on\nenhance.x=on\noriginal=1\nenhance.music=off\n"
        );
        let e = EnhanceSettings::parse(&edited);
        assert!(e.original);
        assert_eq!(e.overrides.last(), Some(&("music".to_owned(), false)));
        // Everything on: the master switch off, the keys' lines gone, the
        // others kept.
        let all = EnhanceSettings::edit(&edited, Some(false), &[], &["big_map", "music"]);
        assert_eq!(all, "# mine\nwindow_width=3\nenhance.x=on\noriginal=false\n");
        assert_eq!(EnhanceSettings::edit("", Some(true), &[], &["a"]), "original=true\n");
    }

    #[test]
    fn saving_keeps_the_other_lines() {
        let dir = std::env::temp_dir().join(format!("widget-settings-{}", std::process::id()));
        let path = dir.join("settings.ini");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            &path,
            "# mine\nwindow_width=1\nenhance.window_stacking=off\nfullscreen=true\n",
        )
        .unwrap();
        let s = WindowSettings {
            size: Some((800, 600)),
            ..WindowSettings::default()
        };
        s.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(WindowSettings::parse(&text), s);
        assert!(text.starts_with("# mine\nenhance.window_stacking=off\n"));
        assert_eq!(text.matches("window_width").count(), 1);
    }
}
