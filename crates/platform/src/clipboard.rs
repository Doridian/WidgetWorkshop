//! The system's clipboard, for text (the widget links, `engine::link`):
//! arboard's natively (X11, Wayland where the compositor offers data
//! control, Windows, macOS), `navigator.clipboard` in the browser, which
//! may ask the player first. What [`request_text`] reads comes to
//! [`crate::App::pasted`] on the next turn of the event loop: the browser
//! can't wait for it.

use std::cell::RefCell;

thread_local! {
    /// What was read, for [`take_pasted`].
    static PASTED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// Natively the clipboard, kept: on X11 what's copied is served from
    /// it until another program copies.
    #[cfg(not(target_arch = "wasm32"))]
    static CLIPBOARD: RefCell<Option<arboard::Clipboard>> = const { RefCell::new(None) };
}

#[cfg(not(target_arch = "wasm32"))]
fn with<T>(f: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>) -> Option<T> {
    CLIPBOARD.with_borrow_mut(|c| {
        if c.is_none() {
            *c = arboard::Clipboard::new()
                .map_err(|e| eprintln!("the clipboard: {e}"))
                .ok();
        }
        f(c.as_mut()?).map_err(|e| eprintln!("the clipboard: {e}")).ok()
    })
}

/// Puts `text` on the clipboard; false if it can't. In the browser the
/// copy finishes later, and a refusal shows on the page's status line.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn set_text(text: &str) -> bool {
    with(|c| c.set_text(text)).is_some()
}

#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn set_text(text: &str) -> bool {
    let Some(clipboard) = crate::web::clipboard() else {
        return false;
    };
    let copy = clipboard.write_text(text);
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = wasm_bindgen_futures::JsFuture::from(copy).await {
            web_sys::console::error_2(&"copying:".into(), &e);
            crate::web::show_error("The browser didn't let the page use the clipboard.");
        }
    });
    true
}

/// Reads the clipboard's text for [`crate::App::pasted`]: an empty text
/// if it holds none or can't be read.
#[cfg(not(target_arch = "wasm32"))]
pub fn request_text() {
    let text = with(arboard::Clipboard::get_text).unwrap_or_default();
    PASTED.with_borrow_mut(|p| p.push(text));
}

#[cfg(target_arch = "wasm32")]
pub fn request_text() {
    let Some(clipboard) = crate::web::clipboard() else {
        PASTED.with_borrow_mut(|p| p.push(String::new()));
        return;
    };
    let read = clipboard.read_text();
    wasm_bindgen_futures::spawn_local(async move {
        let text = match wasm_bindgen_futures::JsFuture::from(read).await {
            Ok(t) => t.as_string().unwrap_or_default(),
            Err(e) => {
                web_sys::console::error_2(&"pasting:".into(), &e);
                String::new()
            }
        };
        PASTED.with_borrow_mut(|p| p.push(text));
    });
}

/// What was read since last time.
pub(crate) fn take_pasted() -> Vec<String> {
    PASTED.with_borrow_mut(std::mem::take)
}
