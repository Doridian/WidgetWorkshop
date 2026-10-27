//! The port's own way in and out for widgets, beside the file dialogs: a
//! widget file dropped onto the window opens like File ▸ Open, and in
//! the browser, where the dialogs only see the browser's storage, the
//! File menu's Save to Download and Download a Widget hand a widget to
//! the player as a download (docs/web.md).

use std::path::Path;

use crate::doc::Document;

use super::{Editor, Next, UNTITLED};

/// Gives the player the file `name` with these bytes; false if it failed.
pub type Download = Box<dyn FnMut(&str, &[u8]) -> bool>;

/// A file dropped onto the window.
#[derive(Debug, Clone)]
pub struct Dropped {
    /// Its name.
    pub name: String,
    /// Where it is, if the editor can save back there (natively); none
    /// in the browser, which gives only the bytes.
    pub path: Option<String>,
    pub data: Vec<u8>,
}

impl Editor {
    /// Downloads go through `download`, and the File menu offers them.
    pub fn set_download(&mut self, download: Download) {
        if self.download.is_none() {
            self.menu.add_downloads();
        }
        self.download = Some(download);
        self.update_menu();
    }

    /// A file was dropped onto the window: "Save changes?" as File ▸
    /// Open asks, then it opens. A drop while a box is up is ignored;
    /// one while the widget runs stops it.
    pub fn drop_file(&mut self, file: Dropped) {
        if self.busy() {
            return;
        }
        self.menu.close();
        self.stop();
        self.dropped = Some(file);
        self.confirm_save(Next::Dropped);
        self.update_menu();
    }

    /// Whether a box is up or a drag going on, which a drop waits for.
    pub(super) fn busy(&self) -> bool {
        self.prompt.is_some()
            || self.settings.is_some()
            || self.dialog.is_some()
            || self.file_dialog.is_some()
            || self.puzzle_dialog.is_some()
            || self.recording.is_some()
            || self.drag.is_some()
    }

    /// After "Save changes?": the dropped file as the widget. Without a
    /// path its name is kept for the title and the next save.
    pub(super) fn open_dropped(&mut self) {
        let Some(f) = self.dropped.take() else { return };
        let Ok(d) = Document::load(&self.assets, &f.data) else {
            self.alert(7);
            return;
        };
        self.install(d, f.path);
        if self.path.is_none() {
            self.drop_name = Some(f.name);
        }
    }

    fn give(&mut self, name: &str, data: &[u8]) {
        if !self.download.as_mut().is_some_and(|d| d(name, data)) {
            self.message(
                "The widget could not be downloaded.",
                &[super::Answer::Ok],
                super::Then::Nothing,
            );
        }
    }

    /// File ▸ Save to Download: the widget as Save writes it, under its
    /// name. It doesn't count as saved; the storage is left alone.
    pub(super) fn save_download(&mut self) {
        self.doc.tray_limits = self.tray.limits();
        let bytes = self.doc.save();
        let name = Some(self.display_name())
            .filter(|n| n != UNTITLED)
            .unwrap_or_else(|| "WIDGET.WGT".to_owned());
        self.give(&name, &bytes);
    }

    /// File ▸ Download a Widget's file: the player's or a CD sample.
    pub(super) fn download_path(&mut self, path: &str) {
        let Ok(bytes) = self.fs().read(Path::new(path)) else {
            self.alert(3);
            return;
        };
        let name = Path::new(path)
            .file_name()
            .map_or_else(|| "WIDGET.WGT".to_owned(), |n| n.to_string_lossy().into_owned());
        self.give(&name, &bytes);
    }
}
