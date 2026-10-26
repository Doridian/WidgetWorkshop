//! Stand-alone widgets (docs/editor-interaction.md 8.5): File ▸ Save Into
//! a Stand-Alone Widget writes a Windows program that runs one widget,
//! and File ▸ Import a Stand-Alone Widget reads the widget back out of
//! one.
//!
//! The program is the CD's `WIDGET.SLF` (the runtime, an `SZDD`-packed
//! Win32 `.EXE`, `formats::lz`) with resources added
//! (`formats::pe::add_resources`): the widget, saved as a `.wgt` is, as
//! type 1023 id 1; the pictures and sounds its parts need (`0x4909a8`,
//! [`NEEDS`]); the background picture. The port makes the same file the
//! original did: it runs on Windows (or Wine) like one made in 1995, and
//! Import reads either.

use std::path::Path;

use formats::pe;

use crate::assets::{Assets, Module, rt};
use crate::doc::Document;

use super::file_dialog::{IMPORT_EXE, SAVE_EXE};
use super::prompt::Answer;
use super::{Editor, Then, UNTITLED};

/// What each class needs in a stand-alone (`0x4909a8`, 9-byte entries
/// {tag, resource type, id, used} up to `Done`): picture sheets (1004,
/// `WWGADGET.DLL`'s) and sounds (1006, `WWSOUND.DLL`'s).
const NEEDS: [([u8; 4], u32, u32); 115] = [
    (*b"disp", 1004, 100),
    (*b"di10", 1004, 100),
    (*b"GrPr", 1004, 200),
    (*b"GrPr", 1004, 201),
    (*b"GrPr", 1004, 202),
    (*b"GrPr", 1004, 203),
    (*b"GrPr", 1004, 204),
    (*b"GrPr", 1004, 205),
    (*b"Pend", 1004, 1000),
    (*b"Drop", 1004, 1001),
    (*b"Cano", 1004, 1002),
    (*b"Wtch", 1004, 1004),
    (*b"Ttlr", 1004, 1005),
    (*b"Coin", 1004, 1006),
    (*b"rand", 1004, 1007),
    (*b"Fork", 1004, 1008),
    (*b"DClk", 1004, 1009),
    (*b"Key_", 1004, 1010),
    (*b"Metr", 1004, 1011),
    (*b"swit", 1004, 1012),
    (*b"disp", 1004, 1013),
    (*b"di10", 1004, 1014),
    (*b"Sldr", 1004, 1015),
    (*b"bulb", 1004, 1016),
    (*b"hart", 1004, 1017),
    (*b"rgbd", 1004, 1018),
    (*b"Spl2", 1004, 1019),
    (*b"Spl3", 1004, 1019),
    (*b"Spl4", 1004, 1019),
    (*b"Addr", 1004, 1020),
    (*b"Subr", 1004, 1020),
    (*b"Mulr", 1004, 1020),
    (*b"Divr", 1004, 1020),
    (*b"cnst", 1004, 1021),
    (*b"Plnt", 1004, 1022),
    (*b"1ncr", 1004, 1024),
    (*b"ArVo", 1004, 1025),
    (*b"Conv", 1004, 1026),
    (*b"NmKP", 1004, 1027),
    (*b"Comp", 1004, 1028),
    (*b"AndG", 1004, 1029),
    (*b"OrGa", 1004, 1030),
    (*b"XorG", 1004, 1031),
    (*b"NotG", 1004, 1032),
    (*b"Mplx", 1004, 1036),
    (*b"Tape", 1004, 1037),
    (*b"Spkr", 1004, 1038),
    (*b"MsMv", 1004, 1039),
    (*b"MsCl", 1004, 1040),
    (*b"Cntr", 1004, 1041),
    (*b"AnSd", 1004, 1042),
    (*b"MsSd", 1004, 1043),
    (*b"SdTr", 1004, 1044),
    (*b"SdSw", 1004, 1045),
    (*b"TxLn", 1004, 1046),
    (*b"TxBx", 1004, 1047),
    (*b"DnPl", 1004, 1048),
    (*b"StCh", 1004, 1049),
    (*b"GrPr", 1004, 1050),
    (*b"GrPr", 1004, 200),
    (*b"GrPr", 1004, 201),
    (*b"GrPr", 1004, 202),
    (*b"GrPr", 1004, 203),
    (*b"GrPr", 1004, 204),
    (*b"GrPr", 1004, 205),
    (*b"Stop", 1004, 1051),
    (*b"Stk1", 1004, 1052),
    (*b"Stk2", 1004, 1053),
    (*b"Stk3", 1004, 1054),
    (*b"Stk4", 1004, 1055),
    (*b"Stk5", 1004, 1056),
    (*b"Stk6", 1004, 1057),
    (*b"Stk7", 1004, 1058),
    (*b"Stk8", 1004, 1059),
    (*b"Stk9", 1004, 1060),
    (*b"St10", 1004, 1061),
    (*b"MiCo", 1004, 1063),
    (*b"Pend", 1006, 1000),
    (*b"Drop", 1006, 1001),
    (*b"Drop", 1006, 1101),
    (*b"Cano", 1006, 1002),
    (*b"Coin", 1006, 1006),
    (*b"rand", 1006, 1007),
    (*b"Fork", 1006, 1008),
    (*b"Metr", 1006, 1011),
    (*b"swit", 1006, 1012),
    (*b"hart", 1006, 1017),
    (*b"Addr", 1006, 1020),
    (*b"Subr", 1006, 1020),
    (*b"Mulr", 1006, 1020),
    (*b"Divr", 1006, 1020),
    (*b"1ncr", 1006, 1020),
    (*b"ArVo", 1006, 1020),
    (*b"Conv", 1006, 1020),
    (*b"NmKP", 1006, 1027),
    (*b"Mplx", 1006, 1036),
    (*b"AnSd", 1006, 200),
    (*b"AnSd", 1006, 201),
    (*b"AnSd", 1006, 202),
    (*b"AnSd", 1006, 203),
    (*b"MsSd", 1006, 300),
    (*b"MsSd", 1006, 301),
    (*b"MsSd", 1006, 302),
    (*b"MsSd", 1006, 303),
    (*b"MsSd", 1006, 304),
    (*b"MsSd", 1006, 305),
    (*b"MsSd", 1006, 306),
    (*b"MsSd", 1006, 307),
    (*b"MsSd", 1006, 308),
    (*b"MsSd", 1006, 309),
    (*b"DnPl", 1006, 1048),
    (*b"Stop", 1006, 1051),
    (*b"Stop", 1006, 1151),
    (*b"Spkr", 1006, 220),
    (*b"Spkr", 1006, 224),
];

/// The message when the runtime isn't there (`0x491326`).
pub const NO_RUNTIME: &str = "Can't find file: WIDGET.SLF\nIt must be in the same directory as Widget Workshop\n";

/// The resources a stand-alone of `doc` carries besides the widget
/// (`0x42659d` marks the classes present; each type and id once, in the
/// table's order), then the background picture unless it's plain.
#[must_use]
pub fn needs(doc: &Document) -> Vec<(u32, u32)> {
    let mut v: Vec<(u32, u32)> = Vec::new();
    for (tag, typ, id) in NEEDS {
        if doc.gadgets.iter().any(|g| g.core.tag.0 == tag) && !v.contains(&(typ, id)) {
            v.push((typ, id));
        }
    }
    if doc.background != 0 {
        v.push((rt::PICTURES, u32::from(doc.background)));
    }
    v
}

/// The stand-alone program for `doc` (saved as `widget`) from the
/// packed runtime `slf`; `None` if the runtime doesn't unpack or take
/// resources. Resources the assets lack are left out (the original asks
/// to retry, `0x41082a`).
#[must_use]
pub fn build(assets: &Assets, slf: &[u8], doc: &Document, widget: &[u8]) -> Option<Vec<u8>> {
    let exe = formats::lz::expand(slf).unwrap_or_else(|| slf.to_vec());
    let found: Vec<(u32, u32, &[u8])> = needs(doc)
        .into_iter()
        .filter_map(|(typ, id)| {
            let m = if typ == rt::SOUND {
                Module::Sound
            } else {
                Module::Gadget
            };
            assets.get(m, typ, id).map(|b| (typ, id, b))
        })
        .collect();
    let mut add = vec![(rt::WIDGET, 1, widget)];
    add.extend(found);
    pe::add_resources(&exe, &add).ok()
}

/// The widget in the stand-alone `exe` (resource 1023/1): `Err` if it
/// isn't a Windows program, `Ok(None)` if it holds no widget.
///
/// # Errors
/// If `exe` isn't a PE file with resources.
pub fn widget_of(exe: &[u8]) -> Result<Option<Vec<u8>>, pe::Error> {
    Ok(pe::Resources::parse(exe)?.get(rt::WIDGET, 1).map(<[u8]>::to_vec))
}

/// `path` with its extension (the last four characters, as `0x415ece`
/// does) replaced by `ext`.
fn swap_ext(path: &str, ext: &str) -> String {
    let p = Path::new(path);
    p.with_extension(ext.trim_start_matches('.')).display().to_string()
}

impl Editor {
    /// File ▸ Save Into a Stand-Alone Widget (`0x4165a7`): the runtime
    /// must be there; the Fit dialog (10014, Cancel stops); then the file.
    pub(super) fn save_into(&mut self) {
        if self.assets.runtime.is_none() {
            self.message(NO_RUNTIME, &[Answer::Ok], Then::Nothing);
            return;
        }
        self.fit_dialog(Some(super::file::Step::Into));
    }

    /// The save box, the default the document's name with `.EXE`.
    pub(super) fn save_into_ask(&mut self) {
        let name = if self.puzzle.active {
            crate::puzzle::DOC_NAME.to_owned()
        } else {
            self.path.clone().unwrap_or_else(|| UNTITLED.to_owned())
        };
        self.ask_file(SAVE_EXE, &swap_ext(&name, ".EXE"), Then::SaveInto);
    }

    /// Writes the stand-alone `path`; alert 4 if it can't. The document
    /// keeps its name, its puzzle and its changes.
    pub(super) fn save_into_path(&mut self, path: &str) {
        self.doc.tray_limits = self.tray.limits();
        let widget = self.doc.save();
        let exe = self
            .assets
            .runtime
            .as_deref()
            .and_then(|slf| build(&self.assets, slf, &self.doc, &widget));
        let written = exe.is_some_and(|b| self.files.write(Path::new(path), &b).is_ok());
        if !written {
            self.alert(4);
        }
    }

    /// File ▸ Import a Stand-Alone Widget, after the save question
    /// (`0x416bf5`).
    pub(super) fn import_ask(&mut self) {
        self.ask_file(IMPORT_EXE, "", Then::ImportExe);
    }

    /// Reads the widget out of `path` (`0x416bf5`): it becomes the
    /// document, named `path` with `.WGT`, remembered as from an
    /// `.EXE` for Revert (`cFile+0x101`). A file that can't be read is
    /// alert 6 (`0x4120ec`'s, which names `WIDGET.SLF`); one that isn't a
    /// program alert 8 (the original throws there); a program without a
    /// widget does nothing, as the original.
    pub(super) fn import_path(&mut self, path: &str) -> bool {
        let Ok(bytes) = self.fs().read(Path::new(path)) else {
            self.alert(6);
            return false;
        };
        let w = match widget_of(&bytes) {
            Ok(Some(w)) => w,
            Ok(None) => return false,
            Err(_) => {
                self.alert(8);
                return false;
            }
        };
        let Ok(d) = Document::load(&self.assets, &w) else {
            self.alert(7);
            return false;
        };
        // No `UndoClear` here, unlike Open.
        let undo = self.undo.take();
        self.install(d, Some(swap_ext(path, ".WGT")));
        self.undo = undo;
        self.from_exe = true;
        true
    }

    /// Revert of an imported widget (`0x415ece`): read again from the
    /// `.EXE` beside its name.
    pub(super) fn revert_exe(&mut self) {
        if let Some(p) = self.path.clone() {
            let exe = swap_ext(&p, ".EXE");
            let undo = self.undo.take();
            if self.import_path(&exe) {
                self.path = Some(p);
            }
            self.undo = undo;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(swap_ext("/a/b/clock.wgt", ".EXE"), "/a/b/clock.EXE");
        assert_eq!(swap_ext("puzzle", ".EXE"), "puzzle.EXE");
        assert_eq!(NEEDS.iter().filter(|n| n.0 == *b"GrPr").count(), 13);
    }
}
