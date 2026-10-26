//! Widget links (the port's own, `crate::link`, docs/web.md): File ▸
//! Copy Link to Widget puts a link to the browser build's page with the
//! widget in it on the clipboard, and File ▸ Open Widget Link (Ctrl+V)
//! opens the one on the clipboard as a dropped widget without a path. A
//! link given at the start (the page's address in the browser, an
//! argument natively) opens the same way.

use crate::link;

use super::{Answer, Editor, Then, UNTITLED, download::Dropped};

/// How the editor reaches the clipboard, and where its links point.
pub struct Links {
    /// The page the links open, without a fragment.
    pub base: String,
    /// Puts the text on the clipboard; false if it failed.
    pub copy: Box<dyn FnMut(&str) -> bool>,
    /// Asks for the clipboard's text, which comes back to
    /// [`Editor::paste_link`].
    pub paste: Box<dyn FnMut()>,
}

impl Editor {
    /// Links go to and come from the clipboard through `links`, and the
    /// File menu offers them.
    pub fn set_links(&mut self, links: Links) {
        if self.links.is_none() {
            self.menu.add_links();
        }
        self.links = Some(links);
        self.update_menu();
    }

    /// File ▸ Copy Link to Widget: the widget as Save writes it, under
    /// its name. It doesn't count as saved.
    pub(super) fn copy_link(&mut self) {
        self.doc.tray_limits = self.tray.limits();
        let bytes = self.doc.save();
        let name = Some(self.display_name()).filter(|n| n != UNTITLED);
        let Some(l) = &mut self.links else { return };
        let url = link::encode(&l.base, name.as_deref(), &bytes);
        let text = if (l.copy)(&url) {
            format!(
                "A link to {} is on the clipboard ({} characters).",
                name.as_deref().unwrap_or("the widget"),
                url.len()
            )
        } else {
            "The link could not be copied.".to_owned()
        };
        self.message(&text, &[Answer::Ok], Then::Nothing);
    }

    /// File ▸ Open Widget Link: asks for the clipboard's text.
    pub(super) fn request_paste(&mut self) {
        if let Some(l) = &mut self.links {
            (l.paste)();
        }
    }

    /// The clipboard's text came: the widget in it opens as a dropped
    /// one ([`Editor::drop_file`]), else the player hears there's none.
    pub fn paste_link(&mut self, text: &str) {
        self.open_text(text, "The clipboard holds no link to a widget.");
    }

    /// A link given at the start opens as [`Editor::paste_link`]'s.
    pub fn open_link(&mut self, text: &str) {
        self.open_text(text, "That link holds no widget.");
    }

    fn open_text(&mut self, text: &str, no_link: &str) {
        match link::decode(text) {
            Some((name, data)) => self.drop_file(Dropped {
                name: name.unwrap_or_else(|| "WIDGET.WGT".to_owned()),
                path: None,
                data,
            }),
            None if !self.busy() => self.message(no_link, &[Answer::Ok], Then::Nothing),
            None => {}
        }
    }
}
