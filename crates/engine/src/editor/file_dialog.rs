//! The common file dialogs (`GetOpenFileNameA`, `GetSaveFileNameA`,
//! docs/editor-interaction.md 8.6): Windows 95's Explorer-style Open and
//! Save As boxes, drawn like the other dialogs so they look the same
//! everywhere. "Look in" with the folder's drives and parents, the Up One
//! Level button, the file list (Windows 95's List view: small icons in
//! columns, a horizontal scroll bar), "File name", "Files of type", Open
//! or Save, and Cancel.
//!
//! What the program passes is a [`Spec`] (the title, the one filter, the
//! default extension, and whether it's a Save box, whose
//! `OFN_OVERWRITEPROMPT` asks before replacing a file); where it starts and
//! the name in the field are the caller's (`lpstrInitialDir`,
//! `lpstrFile`). The layout is the Explorer-style template's (`comdlg32`
//! 1547) without the hidden "Open as read-only" (`OFN_HIDEREADONLY`).

use std::path::{Path, PathBuf};

use crate::assets::Assets;
use crate::files::{self, Fs};
use crate::gfx::{IRect, Surface};

use super::dialog::{CAPTION_H, FRAME, arrow, base_units, close_box, draw_window, mul_div};
use super::prompt::{Answer, Prompt, typed};
use super::puzzle_dialog::focus_rect;
use super::ui;

/// What a caller passes (`OPENFILENAME`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spec {
    /// `lpstrTitle`.
    pub title: &'static str,
    /// `lpstrFilter`: the type's description and pattern.
    pub filter: &'static [(&'static str, &'static str)],
    /// `lpstrDefExt`: added to a name without an extension.
    pub def_ext: &'static str,
    /// `GetSaveFileNameA` with `Flags` 0x806 (`OFN_OVERWRITEPROMPT |
    /// OFN_HIDEREADONLY | OFN_PATHMUSTEXIST`, `0x41732c`); else
    /// `GetOpenFileNameA` with 0x1804 (`OFN_FILEMUSTEXIST` instead of the
    /// prompt, `0x417160`, `0x43dd54`).
    pub save: bool,
}

/// File ▸ Open (`0x415936`).
pub const OPEN_WIDGET: Spec = Spec {
    title: "Load widget",
    filter: &[("Saved widgets (*.wgt)", "*.wgt")],
    def_ext: "wgt",
    save: false,
};
/// File ▸ Save As (`0x415c1f`).
pub const SAVE_WIDGET: Spec = Spec {
    title: "Save Widget as",
    filter: &[("Saved widgets (*.wgt)", "*.wgt")],
    def_ext: "wgt",
    save: true,
};
/// File ▸ Import a Stand-Alone Widget (`0x416bf5`).
pub const IMPORT_EXE: Spec = Spec {
    title: "Import from self-running widget",
    filter: &[("Self running widgets (*.exe)", "*.exe")],
    def_ext: "exe",
    save: false,
};
/// File ▸ Save Into a Stand-Alone Widget (`0x4165a7`).
pub const SAVE_EXE: Spec = Spec {
    title: "Save Into Stand Alone Widget",
    filter: &[("Self running widgets (*.exe)", "*.exe")],
    def_ext: "exe",
    save: true,
};
/// The Recorder's "Import .WAV file" (`0x43dd54`).
pub const OPEN_WAV: Spec = Spec {
    title: "Import from Wave format file",
    filter: &[("Wave files (*.wav)", "*.wav")],
    def_ext: "WAV",
    save: false,
};

/// The template's controls (dialog units: x, y, width, height).
const LOOK_LABEL: (i16, i16, i16, i16) = (7, 6, 41, 8);
const LOOK_IN: (i16, i16, i16, i16) = (49, 3, 132, 12);
const LIST: (i16, i16, i16, i16) = (4, 20, 272, 85);
const NAME_LABEL: (i16, i16, i16, i16) = (5, 112, 46, 8);
const NAME: (i16, i16, i16, i16) = (54, 110, 155, 12);
const TYPE_LABEL: (i16, i16, i16, i16) = (5, 128, 46, 8);
const TYPES: (i16, i16, i16, i16) = (54, 126, 155, 12);
const OK: (i16, i16, i16, i16) = (222, 110, 50, 14);
const CANCEL: (i16, i16, i16, i16) = (222, 128, 50, 14);
/// The dialog's size.
const SIZE: (i16, i16) = (280, 146);
/// A toolbar button (pixels).
const TOOL: (i32, i32) = (23, 22);
/// A scroll bar's height and its arrows' width.
const SB: i32 = 16;
/// A list row: the small icon's height and a pixel.
const ROW_H: i32 = 17;
/// The pixels around a name in the list: the icon, the gaps.
const ICON_W: i32 = 16;

const VK_BACK: u16 = 0x08;
const VK_TAB: u16 = 0x09;
const VK_RETURN: u16 = 0x0d;
const VK_ESCAPE: u16 = 0x1b;
const VK_END: u16 = 0x23;
const VK_HOME: u16 = 0x24;
const VK_LEFT: u16 = 0x25;
const VK_UP: u16 = 0x26;
const VK_RIGHT: u16 = 0x27;
const VK_DOWN: u16 = 0x28;

/// A list entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub name: String,
    pub dir: bool,
    /// The name's width in pixels.
    w: i32,
}

/// Small icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Icon {
    Folder,
    File,
    Drive,
    Cd,
}

/// An entry of "Look in": a drive or a folder on the way to the current
/// one, indented by its depth.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Place {
    path: PathBuf,
    depth: i32,
    label: String,
    icon: Icon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Name,
    List,
}

/// What the mouse holds down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Press {
    Ok,
    Cancel,
    Close,
    Up,
    Left,
    Right,
    PageLeft,
    PageRight,
    /// The thumb, grabbed `grab` pixels right of its left edge.
    Thumb {
        grab: i32,
    },
}

/// The drop-down list open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dropped {
    LookIn,
    Types,
}

/// What an inner message box leads to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Asked {
    /// Back to the dialog.
    Back,
    /// "... already exists. Do you want to replace it?": Yes takes it.
    Replace(PathBuf),
}

/// What an input left the dialog as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Open,
    /// Open or Save with this file.
    Done(PathBuf),
    Cancel,
}

/// The controls' rectangles (client coordinates).
#[derive(Debug, Clone, Copy, Default)]
struct Rects {
    look_label: IRect,
    look_in: IRect,
    up: IRect,
    list: IRect,
    name_label: IRect,
    name: IRect,
    type_label: IRect,
    types: IRect,
    ok: IRect,
    cancel: IRect,
}

/// An open file dialog.
#[derive(Debug, Clone)]
pub struct FileDialog {
    pub spec: Spec,
    /// The folder shown.
    pub dir: PathBuf,
    /// "File name".
    pub name: String,
    /// The name is selected (as it starts): typing replaces it.
    name_sel: bool,
    /// "Files of type": the filter chosen.
    filter: usize,
    /// The list's patterns (`;` between): the filter's, or typed.
    pattern: String,
    pub items: Vec<Item>,
    pub sel: Option<usize>,
    /// The first column shown.
    first: usize,
    focus: Focus,
    pressed: Option<Press>,
    dropped: Option<Dropped>,
    /// "Look in"'s entries, made when it drops.
    places: Vec<Place>,
    asked: Option<(Prompt, Asked)>,
    /// Where it's centred (the frame) and its window and client area.
    area: IRect,
    window: IRect,
    client: IRect,
    r: Rects,
}

/// `pattern` (`*`, `?`) matches `name`, without regard to case.
fn glob(pattern: &[u8], name: &[u8]) -> bool {
    match (pattern.first(), name.first()) {
        (None, None) => true,
        (Some(b'*'), _) => glob(&pattern[1..], name) || !name.is_empty() && glob(pattern, &name[1..]),
        (Some(b'?'), Some(_)) => glob(&pattern[1..], &name[1..]),
        (Some(p), Some(n)) => p.eq_ignore_ascii_case(n) && glob(&pattern[1..], &name[1..]),
        _ => false,
    }
}

/// Whether `name` matches one of `patterns` (`;` between). `*.*` takes
/// every name, and `*.` the ones without an extension, as in Windows.
#[must_use]
pub fn matches(patterns: &str, name: &str) -> bool {
    patterns.split(';').map(str::trim).any(|p| match p {
        "*.*" | "*" => true,
        "*." => !name.contains('.'),
        _ => glob(p.as_bytes(), name.as_bytes()),
    })
}

/// A drive's label: "(C:)" for a Windows drive, "Widget Workshop CD" for
/// the samples' (`files::CD`), else the path.
fn root_label(p: &Path) -> String {
    if files::on_cd(p).is_some() {
        return "Widget Workshop CD".to_owned();
    }
    let s = p.display().to_string();
    match s.trim_end_matches(['\\', '/']) {
        d if d.len() == 2 && d.ends_with(':') => format!("({d})"),
        _ => s,
    }
}

impl FileDialog {
    /// The dialog showing `dir` with `name` in the field (selected),
    /// centred in `area`.
    #[must_use]
    pub fn new(assets: &Assets, fs: Fs, spec: Spec, dir: &Path, name: &str, area: IRect) -> FileDialog {
        let (bx, by) = base_units(assets);
        let px = |(x, y, w, h): (i16, i16, i16, i16)| {
            IRect::sized(
                mul_div(i32::from(x), bx, 4),
                mul_div(i32::from(y), by, 8),
                mul_div(i32::from(w), bx, 4),
                mul_div(i32::from(h), by, 8),
            )
        };
        let (cw, ch) = (mul_div(i32::from(SIZE.0), bx, 4), mul_div(i32::from(SIZE.1), by, 8));
        let (ww, wh) = (cw + 2 * FRAME, ch + 2 * FRAME + CAPTION_H);
        let window = IRect::sized(
            area.x0 + (area.width() - ww) / 2,
            area.y0 + (area.height() - wh) / 2,
            ww,
            wh,
        );
        let client = IRect::sized(window.x0 + FRAME, window.y0 + FRAME + CAPTION_H, cw, ch);
        // Combo boxes and the edit field are a line high plus their edges
        // (`dialog.rs` does the same).
        let line = |r: IRect| IRect::sized(r.x0, r.y0, r.width(), by + 8);
        let look_in = line(px(LOOK_IN));
        let r = Rects {
            look_label: px(LOOK_LABEL),
            look_in,
            up: IRect::sized(look_in.x1 + 8, look_in.y0 - 1, TOOL.0, TOOL.1),
            list: px(LIST),
            name_label: px(NAME_LABEL),
            name: line(px(NAME)),
            type_label: px(TYPE_LABEL),
            types: line(px(TYPES)),
            ok: px(OK),
            cancel: px(CANCEL),
        };
        let mut d = FileDialog {
            spec,
            dir: files::normalize(dir),
            name: name.to_owned(),
            name_sel: !name.is_empty(),
            filter: 0,
            pattern: spec.filter.first().map_or("*.*", |f| f.1).to_owned(),
            items: Vec::new(),
            sel: None,
            first: 0,
            focus: Focus::Name,
            pressed: None,
            dropped: None,
            places: Vec::new(),
            asked: None,
            area,
            window,
            client,
            r,
        };
        d.relist(assets, fs);
        d
    }

    /// Lists the folder again: its folders, then the files the pattern
    /// takes, each sorted without regard to case; hidden (dot) names left
    /// out.
    fn relist(&mut self, assets: &Assets, fs: Fs) {
        let font = ui::font(assets);
        let mut items: Vec<Item> = fs
            .list(&self.dir)
            .unwrap_or_default()
            .into_iter()
            .filter(|(n, dir)| !n.starts_with('.') && (*dir || matches(&self.pattern, n)))
            .map(|(name, dir)| Item {
                w: font.map_or(0, |f| ui::width(f, &name)),
                name,
                dir,
            })
            .collect();
        items.sort_by(|a, b| {
            b.dir
                .cmp(&a.dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        self.items = items;
        self.sel = None;
        self.first = 0;
    }

    /// Shows folder `dir`.
    fn go(&mut self, assets: &Assets, fs: Fs, dir: &Path) {
        self.dir = files::normalize(dir);
        self.relist(assets, fs);
    }

    /// The folder above, unless this is a drive.
    fn parent(&self, fs: Fs) -> Option<PathBuf> {
        if fs.roots().contains(&self.dir) {
            return None;
        }
        self.dir
            .parent()
            .map(Path::to_path_buf)
            .filter(|p| !p.as_os_str().is_empty())
    }

    /// "Look in"'s entries: each drive, the current folder's way down from
    /// its own, the samples' folder under the CD.
    fn make_places(&self, fs: Fs) -> Vec<Place> {
        let mut v = Vec::new();
        for root in fs.roots() {
            let cd = files::on_cd(&root).is_some();
            v.push(Place {
                path: root.clone(),
                depth: 0,
                label: root_label(&root),
                icon: if cd { Icon::Cd } else { Icon::Drive },
            });
            if let Ok(rest) = self.dir.strip_prefix(&root) {
                let mut p = root.clone();
                for (k, c) in rest.components().enumerate() {
                    p.push(c);
                    v.push(Place {
                        path: p.clone(),
                        depth: i32::try_from(k).unwrap_or(0) + 1,
                        label: c.as_os_str().to_string_lossy().into_owned(),
                        icon: Icon::Folder,
                    });
                }
            } else if cd {
                v.push(Place {
                    path: root.join(files::SAMPLES),
                    depth: 1,
                    label: files::SAMPLES.to_owned(),
                    icon: Icon::Folder,
                });
            }
        }
        v
    }

    /// The current folder's label and icon (the closed "Look in").
    fn here(&self, fs: Fs) -> (String, Icon) {
        if fs.roots().contains(&self.dir) {
            let cd = files::on_cd(&self.dir).is_some();
            return (root_label(&self.dir), if cd { Icon::Cd } else { Icon::Drive });
        }
        let name = self
            .dir
            .file_name()
            .map_or_else(|| self.dir.display().to_string(), |n| n.to_string_lossy().into_owned());
        (name, Icon::Folder)
    }

    // ---- the list's geometry ------------------------------------------

    fn inner(&self) -> IRect {
        self.r.list.inset(2)
    }

    /// The width of a column: the widest name, its icon and gaps.
    fn col_w(&self) -> i32 {
        let w = self.items.iter().map(|i| i.w).max().unwrap_or(0);
        (ICON_W + 3 + w + 4 + 8).min(self.inner().width().max(1))
    }

    fn rows_without_bar(&self) -> usize {
        usize::try_from(self.inner().height() / ROW_H).unwrap_or(1).max(1)
    }

    fn has_bar(&self) -> bool {
        let cols = self.items.len().div_ceil(self.rows_without_bar());
        i32::try_from(cols).unwrap_or(i32::MAX).saturating_mul(self.col_w()) > self.inner().width()
    }

    /// Rows a column holds.
    fn rows(&self) -> usize {
        if self.has_bar() {
            usize::try_from((self.inner().height() - SB) / ROW_H)
                .unwrap_or(1)
                .max(1)
        } else {
            self.rows_without_bar()
        }
    }

    fn cols(&self) -> usize {
        self.items.len().div_ceil(self.rows())
    }

    /// Columns wholly shown.
    fn shown(&self) -> usize {
        usize::try_from(self.inner().width() / self.col_w()).unwrap_or(1).max(1)
    }

    fn max_first(&self) -> usize {
        self.cols().saturating_sub(self.shown())
    }

    fn bar_rect(&self) -> IRect {
        let r = self.inner();
        IRect::new(r.x0, r.y1 - SB, r.x1, r.y1)
    }

    /// The items' area, above the scroll bar.
    fn items_rect(&self) -> IRect {
        let r = self.inner();
        if self.has_bar() {
            IRect::new(r.x0, r.y0, r.x1, r.y1 - SB)
        } else {
            r
        }
    }

    fn thumb(&self) -> IRect {
        let b = self.bar_rect();
        let track = b.width() - 2 * SB;
        let n = i32::try_from(self.cols()).unwrap_or(1).max(1);
        let shown = i32::try_from(self.shown()).unwrap_or(1);
        let w = (track * shown / n).clamp(8.min(track), track);
        let max = i32::try_from(self.max_first()).unwrap_or(0).max(1);
        let first = i32::try_from(self.first).unwrap_or(0);
        IRect::sized(b.x0 + SB + (track - w) * first / max, b.y0, w, SB)
    }

    /// Item `i`'s cell (client coordinates).
    fn cell(&self, i: usize) -> IRect {
        let rows = self.rows();
        let col = i32::try_from(i / rows).unwrap_or(0) - i32::try_from(self.first).unwrap_or(0);
        let row = i32::try_from(i % rows).unwrap_or(0);
        let r = self.items_rect();
        IRect::sized(r.x0 + col * self.col_w(), r.y0 + row * ROW_H, self.col_w(), ROW_H)
    }

    /// Item `i`'s label: the icon and the name (what a click hits).
    fn label_rect(&self, i: usize) -> IRect {
        let c = self.cell(i);
        IRect::sized(c.x0, c.y0, ICON_W + 3 + self.items[i].w + 4, ROW_H)
    }

    fn item_at(&self, q: (i32, i32)) -> Option<usize> {
        if !self.items_rect().contains(q.0, q.1) {
            return None;
        }
        let r = self.items_rect();
        let col = usize::try_from((q.0 - r.x0) / self.col_w()).ok()? + self.first;
        let row = usize::try_from((q.1 - r.y0) / ROW_H).ok()?;
        if row >= self.rows() {
            return None;
        }
        let i = col * self.rows() + row;
        (i < self.items.len() && self.label_rect(i).contains(q.0, q.1)).then_some(i)
    }

    fn scroll_to(&mut self, first: usize) {
        self.first = first.min(self.max_first());
    }

    /// Selects item `i` (clamped), scrolled into view; a file's name goes
    /// to the field.
    fn select(&mut self, i: usize) {
        if self.items.is_empty() {
            return;
        }
        let i = i.min(self.items.len() - 1);
        self.sel = Some(i);
        let col = i / self.rows();
        if col < self.first {
            self.first = col;
        } else if col >= self.first + self.shown() {
            self.first = col + 1 - self.shown();
        }
        if !self.items[i].dir {
            self.name.clone_from(&self.items[i].name);
            self.name_sel = false;
        }
    }

    // ---- input ---------------------------------------------------------

    fn to_client(&self, p: (i32, i32)) -> (i32, i32) {
        (p.0 - self.client.x0, p.1 - self.client.y0)
    }

    /// The open list's rectangle (client coordinates) and its rows.
    fn drop_rect(&self, d: Dropped) -> (IRect, usize) {
        let (c, n) = match d {
            Dropped::LookIn => (self.r.look_in, self.places.len()),
            Dropped::Types => (self.r.types, self.spec.filter.len()),
        };
        let h = i32::try_from(n).unwrap_or(0) * ROW_H + 2;
        (IRect::sized(c.x0, c.y1, c.width(), h), n)
    }

    /// Open or Save (`IDOK`): the name worked out against the folder.
    fn ok(&mut self, assets: &Assets, fs: Fs) -> Outcome {
        let text = self.name.trim().to_owned();
        if text.is_empty() {
            // Enter on a folder in the list opens it.
            if self.focus == Focus::List
                && let Some(i) = self.sel
                && self.items[i].dir
            {
                let d = self.dir.join(&self.items[i].name);
                self.go(assets, fs, &d);
            }
            return Outcome::Open;
        }
        let typed = Path::new(&text);
        let p = if typed.is_absolute() || files::on_cd(typed).is_some() {
            files::normalize(typed)
        } else {
            files::normalize(&self.dir.join(typed))
        };
        // A pattern lists what it takes.
        if text.contains(['*', '?']) {
            let pattern = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let dir = p.parent().map(Path::to_path_buf).unwrap_or_default();
            if !fs.is_dir(&dir) {
                return self.tell(&format!(
                    "{text}\nPath not found.\nPlease verify the correct path was given."
                ));
            }
            self.pattern = pattern;
            self.name = text;
            self.name_sel = true;
            self.go(assets, fs, &dir);
            return Outcome::Open;
        }
        if fs.is_dir(&p) {
            self.name.clear();
            self.go(assets, fs, &p);
            return Outcome::Open;
        }
        let dir = p.parent().map(Path::to_path_buf).unwrap_or_default();
        if !fs.is_dir(&dir) {
            return self.tell(&format!(
                "{text}\nPath not found.\nPlease verify the correct path was given."
            ));
        }
        // The default extension for a name without one; Open takes the
        // name as typed if that file is there.
        let mut p = p;
        if p.extension().is_none() && !self.spec.def_ext.is_empty() && (self.spec.save || !fs.is_file(&p)) {
            let mut s = p.into_os_string();
            s.push(".");
            s.push(self.spec.def_ext);
            p = PathBuf::from(s);
        }
        if self.spec.save {
            if fs.is_file(&p) {
                let text = format!("{} already exists.\nDo you want to replace it?", p.display());
                self.asked = Some((
                    Prompt::new(self.spec.title, &text, &[Answer::Yes, Answer::No]),
                    Asked::Replace(p),
                ));
                return Outcome::Open;
            }
        } else if !fs.is_file(&p) {
            return self.tell(&format!(
                "{text}\nFile not found.\nPlease verify the correct file name was given."
            ));
        }
        Outcome::Done(p)
    }

    /// The message box over the dialog, if one is up.
    #[must_use]
    pub fn message(&self) -> Option<&str> {
        self.asked.as_ref().map(|(p, _)| p.text.as_str())
    }

    /// A message box over the dialog, then back to it.
    fn tell(&mut self, text: &str) -> Outcome {
        self.asked = Some((Prompt::new(self.spec.title, text, &[Answer::Ok]), Asked::Back));
        Outcome::Open
    }

    /// The inner message box's answer.
    fn answered(&mut self, a: Answer, asked: Asked) -> Outcome {
        match asked {
            Asked::Replace(p) if a == Answer::Yes => Outcome::Done(p),
            _ => {
                self.focus = Focus::Name;
                self.name_sel = true;
                Outcome::Open
            }
        }
    }

    /// A list entry double-clicked or opened: a folder shows, a file is
    /// taken.
    fn open_item(&mut self, assets: &Assets, fs: Fs, i: usize) -> Outcome {
        if self.items[i].dir {
            let d = self.dir.join(&self.items[i].name);
            self.go(assets, fs, &d);
            Outcome::Open
        } else {
            self.name.clone_from(&self.items[i].name);
            self.ok(assets, fs)
        }
    }

    pub fn mouse_down(&mut self, assets: &Assets, fs: Fs, p: (i32, i32), dbl: bool) -> Outcome {
        if let Some((pr, _)) = &mut self.asked {
            pr.mouse_down(assets, self.area, p);
            return Outcome::Open;
        }
        let q = self.to_client(p);
        if let Some(d) = self.dropped.take() {
            let (r, n) = self.drop_rect(d);
            if r.contains(q.0, q.1) {
                let k = usize::try_from((q.1 - r.y0 - 1) / ROW_H)
                    .unwrap_or(0)
                    .min(n.saturating_sub(1));
                match d {
                    Dropped::LookIn => {
                        if let Some(place) = self.places.get(k) {
                            let to = place.path.clone();
                            self.go(assets, fs, &to);
                        }
                    }
                    Dropped::Types => {
                        if let Some(f) = self.spec.filter.get(k) {
                            self.filter = k;
                            f.1.clone_into(&mut self.pattern);
                            self.relist(assets, fs);
                        }
                    }
                }
            }
            return Outcome::Open;
        }
        let r = self.r;
        let press = if close_box(self.window).contains(p.0, p.1) {
            Some(Press::Close)
        } else if r.ok.contains(q.0, q.1) {
            Some(Press::Ok)
        } else if r.cancel.contains(q.0, q.1) {
            Some(Press::Cancel)
        } else if r.up.contains(q.0, q.1) {
            self.parent(fs).map(|_| Press::Up)
        } else {
            None
        };
        if press.is_some() {
            self.pressed = press;
            return Outcome::Open;
        }
        if r.look_in.contains(q.0, q.1) || r.look_label.contains(q.0, q.1) {
            self.places = self.make_places(fs);
            self.dropped = Some(Dropped::LookIn);
        } else if r.types.contains(q.0, q.1) || r.type_label.contains(q.0, q.1) {
            self.dropped = Some(Dropped::Types);
        } else if r.name.contains(q.0, q.1) || r.name_label.contains(q.0, q.1) {
            self.focus = Focus::Name;
            self.name_sel = false;
        } else if r.list.contains(q.0, q.1) {
            self.focus = Focus::List;
            if self.has_bar() && self.bar_rect().contains(q.0, q.1) {
                self.bar_down(q);
            } else if let Some(i) = self.item_at(q) {
                self.select(i);
                if dbl {
                    return self.open_item(assets, fs, i);
                }
            } else {
                self.sel = None;
            }
        }
        Outcome::Open
    }

    fn bar_down(&mut self, q: (i32, i32)) {
        let b = self.bar_rect();
        let t = self.thumb();
        let press = if q.0 < b.x0 + SB {
            self.scroll_to(self.first.saturating_sub(1));
            Press::Left
        } else if q.0 >= b.x1 - SB {
            self.scroll_to(self.first + 1);
            Press::Right
        } else if q.0 < t.x0 {
            self.scroll_to(self.first.saturating_sub(self.shown()));
            Press::PageLeft
        } else if q.0 >= t.x1 {
            self.scroll_to(self.first + self.shown());
            Press::PageRight
        } else {
            Press::Thumb { grab: q.0 - t.x0 }
        };
        self.pressed = Some(press);
    }

    pub fn mouse_move(&mut self, p: (i32, i32)) {
        let q = self.to_client(p);
        if let Some(Press::Thumb { grab }) = self.pressed {
            let b = self.bar_rect();
            let t = self.thumb();
            let room = b.width() - 2 * SB - t.width();
            if room > 0 {
                let x = (q.0 - grab - b.x0 - SB).clamp(0, room);
                let max = i32::try_from(self.max_first()).unwrap_or(0);
                self.scroll_to(usize::try_from((x * max + room / 2) / room).unwrap_or(0));
            }
        }
    }

    pub fn mouse_up(&mut self, assets: &Assets, fs: Fs, p: (i32, i32)) -> Outcome {
        if let Some((pr, asked)) = &mut self.asked {
            if let Some(a) = pr.mouse_up(assets, self.area, p) {
                let asked = asked.clone();
                self.asked = None;
                return self.answered(a, asked);
            }
            return Outcome::Open;
        }
        let Some(press) = self.pressed.take() else {
            return Outcome::Open;
        };
        let q = self.to_client(p);
        let over = |r: IRect| r.contains(q.0, q.1);
        match press {
            Press::Close if close_box(self.window).contains(p.0, p.1) => Outcome::Cancel,
            Press::Cancel if over(self.r.cancel) => Outcome::Cancel,
            Press::Ok if over(self.r.ok) => self.ok(assets, fs),
            Press::Up if over(self.r.up) => {
                if let Some(d) = self.parent(fs) {
                    self.go(assets, fs, &d);
                }
                Outcome::Open
            }
            _ => Outcome::Open,
        }
    }

    /// A key: Enter is Open (or Save), Esc Cancel, Tab between the name
    /// and the list; in the list the arrows, Home and End move, Backspace
    /// goes up a folder and a letter to the next name starting with it;
    /// in the name, typing.
    pub fn key(&mut self, assets: &Assets, fs: Fs, vk: u16, shift: bool) -> Outcome {
        if let Some((pr, asked)) = &mut self.asked {
            if let Some(a) = pr.key(vk) {
                let asked = asked.clone();
                self.asked = None;
                return self.answered(a, asked);
            }
            return Outcome::Open;
        }
        if self.dropped.is_some() {
            if matches!(vk, VK_RETURN | VK_ESCAPE) {
                self.dropped = None;
            }
            return Outcome::Open;
        }
        match vk {
            VK_RETURN => return self.ok(assets, fs),
            VK_ESCAPE => return Outcome::Cancel,
            VK_TAB => {
                self.focus = if self.focus == Focus::Name {
                    Focus::List
                } else {
                    Focus::Name
                };
                if self.focus == Focus::List && self.sel.is_none() && !self.items.is_empty() {
                    self.sel = Some(0);
                }
                self.name_sel = self.focus == Focus::Name;
                return Outcome::Open;
            }
            _ => {}
        }
        match self.focus {
            Focus::List => self.list_key(assets, fs, vk, shift),
            Focus::Name => {
                if vk == VK_BACK {
                    if std::mem::take(&mut self.name_sel) {
                        self.name.clear();
                    } else {
                        self.name.pop();
                    }
                } else if let Some(c) = typed(vk, shift) {
                    if std::mem::take(&mut self.name_sel) {
                        self.name.clear();
                    }
                    self.name.push(c);
                }
            }
        }
        Outcome::Open
    }

    fn list_key(&mut self, assets: &Assets, fs: Fs, vk: u16, shift: bool) {
        let rows = self.rows();
        let last = self.items.len().saturating_sub(1);
        let sel = self.sel;
        let at = sel.unwrap_or(0);
        match vk {
            VK_UP => self.select(at.saturating_sub(1)),
            VK_DOWN => self.select(if sel.is_some() { at + 1 } else { 0 }),
            VK_LEFT => self.select(at.saturating_sub(rows)),
            VK_RIGHT => self.select(if at + rows <= last { at + rows } else { at }),
            VK_HOME => self.select(0),
            VK_END => self.select(last),
            VK_BACK => {
                if let Some(d) = self.parent(fs) {
                    self.go(assets, fs, &d);
                }
            }
            _ => {
                let Some(c) = typed(vk, shift) else { return };
                let n = self.items.len();
                let start = sel.map_or(0, |i| i + 1);
                if let Some(i) = (0..n).map(|k| (start + k) % n).find(|&i| {
                    self.items[i]
                        .name
                        .chars()
                        .next()
                        .is_some_and(|f| f.eq_ignore_ascii_case(&c))
                }) {
                    self.select(i);
                }
            }
        }
    }

    /// The wheel scrolls the list a column a notch.
    pub fn wheel(&mut self, dy: i32) {
        if dy > 0 {
            self.scroll_to(self.first.saturating_sub(1));
        } else if dy < 0 {
            self.scroll_to(self.first + 1);
        }
    }

    // ---- drawing -------------------------------------------------------

    #[allow(clippy::too_many_lines)]
    pub fn draw(&self, s: &mut Surface, assets: &Assets, fs: Fs) {
        let Some(font) = ui::font(assets) else { return };
        draw_window(
            s,
            font,
            self.window,
            self.spec.title,
            self.pressed == Some(Press::Close),
        );
        let old = s.clip();
        s.set_clip(self.client.intersect(old));
        let (ox, oy) = (self.client.x0, self.client.y0);
        let r = |r: IRect| r.offset(ox, oy);
        let fh = i32::from(font.height);
        let save = self.spec.save;
        let label = |s: &mut Surface, rect: IRect, text: &str| {
            let rect = r(rect);
            ui::label(s, font, rect.x0, rect.y0, text, ui::BLACK);
        };
        label(s, self.r.look_label, if save { "Save &in:" } else { "Look &in:" });
        // "Look in": the folder's icon and name.
        let (here, icon) = self.here(fs);
        Self::combo(
            s,
            font,
            r(self.r.look_in),
            Some((&here, icon)),
            None,
            self.dropped == Some(Dropped::LookIn),
        );
        // Up One Level.
        let up = r(self.r.up);
        let enabled = self.parent(fs).is_some();
        let pressed = self.pressed == Some(Press::Up);
        ui::button(s, font, up, "", pressed, false, true);
        let off = i32::from(pressed);
        draw_up(s, up.x0 + 3 + off, up.y0 + 3 + off, enabled);
        // The list.
        let l = r(self.r.list);
        s.fill(l, ui::WHITE);
        ui::sunken(s, l);
        let items = r(self.items_rect());
        let inner = s.clip();
        s.set_clip(items.intersect(inner));
        let rows = self.rows();
        let start = self.first * rows;
        for i in start..self.items.len() {
            let c = r(self.cell(i));
            if c.x0 >= items.x1 {
                break;
            }
            let it = &self.items[i];
            draw_icon(s, c.x0 + 1, c.y0, if it.dir { Icon::Folder } else { Icon::File });
            let t = IRect::sized(c.x0 + ICON_W + 3, c.y0 + 1, it.w + 4, ROW_H - 2);
            let hot = self.sel == Some(i);
            if hot {
                s.fill(t, if self.focus == Focus::List { ui::NAVY } else { ui::FACE });
            }
            let color = if hot && self.focus == Focus::List {
                ui::WHITE
            } else {
                ui::BLACK
            };
            s.text(
                font,
                t.x0 + 2,
                c.y0 + (ROW_H - fh) / 2,
                &crate::text::encode(&it.name),
                color,
            );
            if hot && self.focus == Focus::List {
                focus_rect(s, t);
            }
        }
        s.set_clip(inner);
        if self.has_bar() {
            self.draw_bar(s, font);
        }
        label(s, self.r.name_label, "File &name:");
        let n = r(self.r.name);
        s.fill(n, ui::WHITE);
        ui::sunken(s, n);
        s.set_clip(n.inset(2).intersect(inner));
        let bytes = crate::text::encode(&self.name);
        let tw = i32::try_from(font.width(&bytes)).unwrap_or(0);
        let x = n.x0 + 4 - (tw - (n.width() - 10)).max(0);
        let ty = n.y0 + (n.height() - fh) / 2;
        if self.name_sel && self.focus == Focus::Name && !self.name.is_empty() {
            s.fill(IRect::sized(x - 1, ty, tw + 2, fh), ui::NAVY);
            s.text(font, x, ty, &bytes, ui::WHITE);
        } else {
            s.text(font, x, ty, &bytes, ui::BLACK);
            if self.focus == Focus::Name {
                s.fill(IRect::sized(x + tw, ty, 1, fh), ui::BLACK);
            }
        }
        s.set_clip(inner);
        label(
            s,
            self.r.type_label,
            if save { "Save as &type:" } else { "Files of &type:" },
        );
        let desc = self.spec.filter.get(self.filter).map_or("", |f| f.0);
        Self::combo(
            s,
            font,
            r(self.r.types),
            None,
            Some(desc),
            self.dropped == Some(Dropped::Types),
        );
        let ok = if save { "&Save" } else { "&Open" };
        ui::button(s, font, r(self.r.ok), ok, self.pressed == Some(Press::Ok), true, true);
        ui::button(
            s,
            font,
            r(self.r.cancel),
            "Cancel",
            self.pressed == Some(Press::Cancel),
            false,
            true,
        );
        // The open list, over everything.
        if let Some(d) = self.dropped {
            s.set_clip(old);
            let (lr, _) = self.drop_rect(d);
            let lr = r(lr);
            s.fill(lr, ui::WHITE);
            s.frame(lr, ui::BLACK);
            let inner = s.clip();
            s.set_clip(lr.inset(1).intersect(inner));
            let rows: Vec<(String, Option<Icon>, i32, bool)> = match d {
                Dropped::LookIn => self
                    .places
                    .iter()
                    .map(|p| (p.label.clone(), Some(p.icon), p.depth, p.path == self.dir))
                    .collect(),
                Dropped::Types => self
                    .spec
                    .filter
                    .iter()
                    .enumerate()
                    .map(|(k, f)| (f.0.to_owned(), None, 0, k == self.filter))
                    .collect(),
            };
            for (k, (text, icon, depth, hot)) in rows.into_iter().enumerate() {
                let y = lr.y0 + 1 + i32::try_from(k).unwrap_or(0) * ROW_H;
                let mut x = lr.x0 + 3 + depth * 10;
                if let Some(icon) = icon {
                    draw_icon(s, x, y, icon);
                    x += ICON_W + 3;
                }
                let w = ui::width(font, &text);
                if hot {
                    s.fill(IRect::sized(x - 1, y + 1, w + 3, ROW_H - 2), ui::NAVY);
                }
                s.text(
                    font,
                    x,
                    y + (ROW_H - fh) / 2,
                    &crate::text::encode(&text),
                    if hot { ui::WHITE } else { ui::BLACK },
                );
            }
            s.set_clip(inner);
        }
        s.set_clip(old);
        if let Some((pr, _)) = &self.asked {
            pr.draw(s, assets, self.area);
        }
    }

    /// A drop-down list box, closed: an icon and text, or just text.
    fn combo(
        s: &mut Surface,
        font: &formats::fnt::Font,
        r: IRect,
        with_icon: Option<(&str, Icon)>,
        text: Option<&str>,
        open: bool,
    ) {
        s.fill(r, ui::WHITE);
        ui::sunken(s, r);
        let b = IRect::new(r.x1 - 2 - 16, r.y0 + 2, r.x1 - 2, r.y1 - 2);
        ui::button(s, font, b, "", open, false, true);
        arrow(s, b.x0 + 4, b.y0 + (b.height() - 4) / 2);
        let inner = s.clip();
        s.set_clip(IRect::new(r.x0 + 2, r.y0 + 2, b.x0, r.y1 - 2).intersect(inner));
        let ty = r.y0 + (r.height() - i32::from(font.height)) / 2;
        match (with_icon, text) {
            (Some((t, icon)), _) => {
                draw_icon(s, r.x0 + 4, r.y0 + (r.height() - 16) / 2, icon);
                s.text(font, r.x0 + 4 + ICON_W + 3, ty, &crate::text::encode(t), ui::BLACK);
            }
            (None, Some(t)) => {
                s.text(font, r.x0 + 4, ty, &crate::text::encode(t), ui::BLACK);
            }
            (None, None) => {}
        }
        s.set_clip(inner);
    }

    fn draw_bar(&self, s: &mut Surface, font: &formats::fnt::Font) {
        let (ox, oy) = (self.client.x0, self.client.y0);
        let b = self.bar_rect().offset(ox, oy);
        for y in b.y0..b.y1 {
            for x in b.x0..b.x1 {
                s.pset(x, y, if (x + y) % 2 == 0 { ui::WHITE } else { ui::FACE });
            }
        }
        let left = IRect::sized(b.x0, b.y0, SB, SB);
        let right = IRect::sized(b.x1 - SB, b.y0, SB, SB);
        for (r, press, pointing_left) in [(left, Press::Left, true), (right, Press::Right, false)] {
            let pressed = self.pressed == Some(press);
            ui::button(s, font, r, "", pressed, false, true);
            let off = i32::from(pressed);
            let (cx, cy) = (r.x0 + 6 + off, r.y0 + 4 + off);
            for d in 0..4 {
                let col = if pointing_left { cx + d } else { cx + 3 - d };
                s.fill(IRect::new(col, cy + 3 - d, col + 1, cy + 4 + d), ui::BLACK);
            }
        }
        if self.max_first() > 0 {
            ui::button(s, font, self.thumb().offset(ox, oy), "", false, false, true);
        }
    }
}

/// A small icon (16×16) from rows of letters: `k` black, `y` yellow, `Y`
/// dark yellow, `w` white, `g` grey, `l` light grey, `G` green.
fn pixels(s: &mut Surface, x: i32, y: i32, rows: &[&str]) {
    for (dy, row) in (0..).zip(rows) {
        for (dx, c) in (0..).zip(row.bytes()) {
            let c = match c {
                b'k' => ui::BLACK,
                b'y' => 251,
                b'Y' => 3,
                b'w' => ui::WHITE,
                b'g' => ui::SHADOW,
                b'l' => ui::FACE,
                b'G' => 250,
                _ => continue,
            };
            s.pset(x + dx, y + dy, c);
        }
    }
}

const FOLDER: [&str; 16] = [
    "",
    "",
    "",
    "  YYYYY",
    " YwyywyY",
    "YkkkkkkkkkkkkY",
    "Ywwwwwwwwwwwwyk",
    "Ywyyyyyyyyyyyyk",
    "Ywyyyyyyyyyyyyk",
    "Ywyyyyyyyyyyyyk",
    "Ywyyyyyyyyyyyyk",
    "Ywyyyyyyyyyyyyk",
    "Ywyyyyyyyyyyyyk",
    "Ywyyyyyyyyyyyyk",
    "kkkkkkkkkkkkkkk",
    "",
];

const FILE: [&str; 16] = [
    "  kkkkkkkk",
    "  kwwwwwwkk",
    "  kwwwwwwkwk",
    "  kwwwwwwkkkk",
    "  kwwwwwwwwwk",
    "  kwwwwwwwwwk",
    "  kwwwwwwwwwk",
    "  kwwwwwwwwwk",
    "  kwwwwwwwwwk",
    "  kwwwwwwwwwk",
    "  kwwwwwwwwwk",
    "  kwwwwwwwwwk",
    "  kwwwwwwwwwk",
    "  kwwwwwwwwwk",
    "  kkkkkkkkkkk",
    "",
];

const DRIVE: [&str; 16] = [
    "",
    "",
    "",
    "",
    "",
    "",
    "  kkkkkkkkkkkk",
    " kwwwwwwwwwwwwk",
    " kllllllllllllk",
    " kllllllllllllk",
    " kllllllllllGlk",
    " kggggggggggggk",
    "  kkkkkkkkkkkk",
    "",
    "",
    "",
];

const CD: [&str; 16] = [
    "",
    "     kkkkk",
    "   kkwwwwwkk",
    "  kwwwwwwwllk",
    " kwwwwwwwllllk",
    " kwwwwwwlllllk",
    "kwwwwwwkkllllgk",
    "kwwwwwk  klllgk",
    "kwwwwwk  kllggk",
    "kwwwwwwkklllggk",
    " kwwwllllllggk",
    " kwwlllllllggk",
    "  kllllllgggk",
    "   kkllgggkk",
    "     kkkkk",
    "",
];

fn draw_icon(s: &mut Surface, x: i32, y: i32, icon: Icon) {
    let rows: &[&str] = match icon {
        Icon::Folder => &FOLDER,
        Icon::File => &FILE,
        Icon::Drive => &DRIVE,
        Icon::Cd => &CD,
    };
    pixels(s, x, y, rows);
}

/// Up One Level's picture: a folder with an arrow up and over it; grey
/// when there's nowhere up to go.
fn draw_up(s: &mut Surface, x: i32, y: i32, enabled: bool) {
    const UP: [&str; 16] = [
        "",
        "",
        "  YYYYY",
        " YwyywyY",
        "YkkkkkkkkkkkkY",
        "Ywwwwwkwwwwwwyk",
        "Ywyyykkkyyyyyyk",
        "Ywyykkkkkyyyyyk",
        "Ywyyyykyyyyyyyk",
        "Ywyyyykyyyyyyyk",
        "Ywyyyykkkkkyyyk",
        "Ywyyyyyyyyyyyyk",
        "Ywyyyyyyyyyyyyk",
        "kkkkkkkkkkkkkkk",
        "",
        "",
    ];
    if enabled {
        pixels(s, x, y, &UP);
    } else {
        let grey: Vec<String> = UP
            .iter()
            .map(|r| r.chars().map(|c| if c == ' ' { ' ' } else { 'g' }).collect())
            .collect();
        let rows: Vec<&str> = grey.iter().map(String::as_str).collect();
        pixels(s, x, y, &rows);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::StdFiles;

    /// A folder: `sub/` (with `x.wgt`), `a.wgt`, `B.WGT`, `c.txt` and a
    /// hidden `.h.wgt`.
    fn folder(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ww-dialog-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("sub")).unwrap();
        for f in ["a.wgt", "B.WGT", "c.txt", ".h.wgt", "sub/x.wgt"] {
            std::fs::write(d.join(f), b"x").unwrap();
        }
        d
    }

    fn fs() -> Fs<'static> {
        Fs {
            files: &StdFiles,
            samples: &[],
        }
    }

    fn names(d: &FileDialog) -> Vec<&str> {
        d.items.iter().map(|i| i.name.as_str()).collect()
    }

    fn typing(d: &mut FileDialog, text: &str) {
        d.name = text.to_owned();
        d.name_sel = false;
        d.focus = Focus::Name;
    }

    #[test]
    fn browses_with_keys() {
        let a = Assets::default();
        let dir = folder("keys");
        let area = IRect::new(0, 0, 800, 600);
        let mut d = FileDialog::new(&a, fs(), OPEN_WIDGET, &dir, "", area);
        // Folders first, the filter's files, no hidden ones.
        assert_eq!(names(&d), ["sub", "a.wgt", "B.WGT"]);
        // Tab to the list (its first entry), Enter opens the folder.
        assert_eq!(d.key(&a, fs(), VK_TAB, false), Outcome::Open);
        assert_eq!(d.sel, Some(0));
        d.key(&a, fs(), VK_RETURN, false);
        assert_eq!((d.dir.clone(), names(&d)), (dir.join("sub"), vec!["x.wgt"]));
        // Backspace goes up; a letter finds the name.
        d.key(&a, fs(), VK_BACK, false);
        assert_eq!(d.dir, dir);
        d.key(&a, fs(), u16::from(b'B'), false);
        assert_eq!((d.sel, d.name.as_str()), (Some(2), "B.WGT"));
        d.key(&a, fs(), VK_UP, false);
        assert_eq!(d.name, "a.wgt");
        // A pattern typed lists what it takes.
        typing(&mut d, "*.txt");
        d.key(&a, fs(), VK_RETURN, false);
        assert_eq!(names(&d), ["sub", "c.txt"]);
        // A missing file: the message, then back.
        typing(&mut d, "nope");
        assert_eq!(d.key(&a, fs(), VK_RETURN, false), Outcome::Open);
        assert_eq!(
            d.message(),
            Some("nope\nFile not found.\nPlease verify the correct file name was given.")
        );
        d.key(&a, fs(), VK_RETURN, false);
        assert_eq!(d.message(), None);
        // The default extension; a folder by name; a path.
        typing(&mut d, "a");
        assert_eq!(d.key(&a, fs(), VK_RETURN, false), Outcome::Done(dir.join("a.wgt")));
        typing(&mut d, "sub");
        assert_eq!(d.key(&a, fs(), VK_RETURN, false), Outcome::Open);
        assert_eq!((d.dir.clone(), d.name.as_str()), (dir.join("sub"), ""));
        typing(&mut d, "../c.txt");
        assert_eq!(d.key(&a, fs(), VK_RETURN, false), Outcome::Done(dir.join("c.txt")));
        typing(&mut d, "/no/such/folder/a.wgt");
        d.key(&a, fs(), VK_RETURN, false);
        assert!(d.message().unwrap().contains("Path not found"));
        d.key(&a, fs(), VK_ESCAPE, false);
        assert_eq!(d.key(&a, fs(), VK_ESCAPE, false), Outcome::Cancel);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn browses_with_the_mouse() {
        let a = Assets::default();
        let dir = folder("mouse");
        let area = IRect::new(0, 0, 800, 600);
        let mut d = FileDialog::new(&a, fs(), OPEN_WIDGET, &dir.join("sub"), "", area);
        let at = |d: &FileDialog, r: IRect| {
            (
                d.client.x0 + i32::midpoint(r.x0, r.x1),
                d.client.y0 + i32::midpoint(r.y0, r.y1),
            )
        };
        let click = |d: &mut FileDialog, p: (i32, i32), dbl: bool| {
            let o = d.mouse_down(&a, fs(), p, dbl);
            if o != Outcome::Open {
                return o;
            }
            d.mouse_up(&a, fs(), p)
        };
        // Up One Level.
        let up = at(&d, d.r.up);
        click(&mut d, up, false);
        assert_eq!(d.dir, dir);
        // A click selects and names; a double click on a folder opens it.
        let p = at(&d, d.label_rect(1));
        click(&mut d, p, false);
        assert_eq!((d.sel, d.name.as_str()), (Some(1), "a.wgt"));
        let p = at(&d, d.label_rect(0));
        click(&mut d, p, false);
        assert_eq!(click(&mut d, p, true), Outcome::Open);
        assert_eq!(d.dir, dir.join("sub"));
        // "Look in" lists the way down; its parent is there to pick.
        let look = at(&d, d.r.look_in);
        click(&mut d, look, false);
        let k = d.places.iter().position(|p| p.path == dir).unwrap();
        assert_eq!(d.places[k + 1].label, "sub");
        let (r, _) = d.drop_rect(Dropped::LookIn);
        let row = (
            d.client.x0 + r.x0 + 5,
            d.client.y0 + r.y0 + 1 + i32::try_from(k).unwrap() * ROW_H + 3,
        );
        click(&mut d, row, false);
        assert_eq!(d.dir, dir);
        // Double-clicking a file takes it; Cancel cancels.
        let p = at(&d, d.label_rect(2));
        click(&mut d, p, false);
        assert_eq!(click(&mut d, p, true), Outcome::Done(dir.join("B.WGT")));
        let c = at(&d, d.r.cancel);
        assert_eq!(click(&mut d, c, false), Outcome::Cancel);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn save_asks_before_replacing() {
        let a = Assets::default();
        let dir = folder("save");
        let area = IRect::new(0, 0, 800, 600);
        let mut d = FileDialog::new(&a, fs(), SAVE_WIDGET, &dir, "a", area);
        // The name starts selected: typing replaces it.
        d.key(&a, fs(), u16::from(b'Q'), false);
        assert_eq!(d.name, "q");
        assert_eq!(d.key(&a, fs(), VK_RETURN, false), Outcome::Done(dir.join("q.wgt")));
        typing(&mut d, "a");
        assert_eq!(d.key(&a, fs(), VK_RETURN, false), Outcome::Open);
        let want = format!(
            "{} already exists.\nDo you want to replace it?",
            dir.join("a.wgt").display()
        );
        assert_eq!(d.message(), Some(want.as_str()));
        // No: back to the dialog; Yes: that file.
        assert_eq!(d.key(&a, fs(), u16::from(b'N'), false), Outcome::Open);
        assert_eq!(d.message(), None);
        d.key(&a, fs(), VK_RETURN, false);
        assert_eq!(
            d.key(&a, fs(), u16::from(b'Y'), false),
            Outcome::Done(dir.join("a.wgt"))
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn many_files_scroll_sideways() {
        let a = Assets::default();
        let dir = std::env::temp_dir().join(format!("ww-dialog-many-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..200 {
            std::fs::write(dir.join(format!("w{i:03}.wgt")), b"x").unwrap();
        }
        let mut d = FileDialog::new(&a, fs(), OPEN_WIDGET, &dir, "", IRect::new(0, 0, 800, 600));
        assert!(d.has_bar() && d.max_first() > 0);
        d.key(&a, fs(), VK_TAB, false);
        d.key(&a, fs(), VK_END, false);
        assert_eq!((d.sel, d.first), (Some(199), d.max_first()));
        d.key(&a, fs(), VK_HOME, false);
        assert_eq!(d.first, 0);
        d.wheel(-1);
        assert_eq!(d.first, 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn patterns() {
        assert!(matches("*.wgt", "CLOCK.WGT"));
        assert!(matches("*.wgt", "a.wgt"));
        assert!(!matches("*.wgt", "a.wgt.bak"));
        assert!(!matches("*.wgt", "readme.txt"));
        assert!(matches("*.txt;*.wgt", "a.WGT"));
        assert!(matches("?.wgt", "a.wgt") && !matches("?.wgt", "ab.wgt"));
        assert!(matches("*.*", "anything") && matches("*.", "noext") && !matches("*.", "a.b"));
        assert_eq!(root_label(Path::new("C:\\")), "(C:)");
        assert_eq!(root_label(Path::new(files::CD)), "Widget Workshop CD");
    }
}
