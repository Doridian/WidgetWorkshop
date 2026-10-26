//! The browser's side of the platform (`wasm32`): the console for what
//! the port would print on the terminal, the page's status line for
//! what the player has to see, files dropped onto the page and files
//! handed to the player as downloads.

use std::cell::RefCell;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;

use crate::DroppedFile;

/// Writes `text` to the browser's console, where `eprintln!` goes in the
/// browser build.
pub fn log(text: &str) {
    web_sys::console::log_1(&text.into());
}

/// Writes `text` to the console as an error and shows it on the page, in
/// the element with the id `status` if there is one.
pub fn show_error(text: &str) {
    web_sys::console::error_1(&text.into());
    show_status(text, true);
}

/// Shows `text` on the page's status line (`#status`), as an error or
/// not; an empty text hides it.
pub fn show_status(text: &str, error: bool) {
    let Some(el) = element("status") else { return };
    el.set_text_content(Some(text));
    let _ = el.set_attribute("class", if error { "error" } else { "" });
    let _ = el.set_attribute("style", if text.is_empty() { "display:none" } else { "" });
}

/// The page's element with the id `id`.
#[must_use]
pub fn element(id: &str) -> Option<web_sys::Element> {
    web_sys::window()?.document()?.get_element_by_id(id)
}

/// The page's `<canvas id="widget">`, if it has one.
pub(crate) fn canvas() -> Option<web_sys::HtmlCanvasElement> {
    element("widget")?.dyn_into().ok()
}

thread_local! {
    /// The files dropped and read, for [`take_drops`]; `None` until
    /// [`watch_drops`].
    static DROPS: RefCell<Option<Vec<DroppedFile>>> = const { RefCell::new(None) };
}

/// Takes the files dropped onto the page from then on (once): the
/// browser would open one in place of the page. Dropped files are read
/// and kept for [`take_drops`].
pub(crate) fn watch_drops() {
    if DROPS.with_borrow(Option::is_some) {
        return;
    }
    DROPS.set(Some(Vec::new()));
    let Some(window) = web_sys::window() else { return };
    let over = Closure::<dyn FnMut(web_sys::DragEvent)>::new(|e: web_sys::DragEvent| {
        e.prevent_default();
        if let Some(dt) = e.data_transfer() {
            dt.set_drop_effect("copy");
        }
    });
    let on_drop = Closure::<dyn FnMut(web_sys::DragEvent)>::new(|e: web_sys::DragEvent| {
        e.prevent_default();
        let Some(files) = e.data_transfer().and_then(|dt| dt.files()) else {
            return;
        };
        for i in 0..files.length() {
            let Some(file) = files.get(i) else { continue };
            wasm_bindgen_futures::spawn_local(async move {
                match wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await {
                    Ok(buf) => {
                        let data = js_sys::Uint8Array::new(&buf).to_vec();
                        let f = DroppedFile {
                            name: file.name(),
                            path: None,
                            data,
                        };
                        DROPS.with_borrow_mut(|d| d.get_or_insert_default().push(f));
                    }
                    Err(e) => web_sys::console::error_2(&"reading the dropped file:".into(), &e),
                }
            });
        }
    });
    let _ = window.add_event_listener_with_callback("dragover", over.as_ref().unchecked_ref());
    let _ = window.add_event_listener_with_callback("drop", on_drop.as_ref().unchecked_ref());
    // The page's for good.
    over.forget();
    on_drop.forget();
}

/// The files dropped and read since last time.
pub(crate) fn take_drops() -> Vec<DroppedFile> {
    DROPS.with_borrow_mut(|d| d.as_mut().map(std::mem::take).unwrap_or_default())
}

/// Hands `data` to the player as a download named `name` (an `<a
/// download>` on a `Blob` of it); false if the browser wouldn't.
#[must_use]
pub fn download(name: &str, data: &[u8]) -> bool {
    let go = || -> Option<()> {
        let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(data));
        let opts = web_sys::BlobPropertyBag::new();
        opts.set_type("application/octet-stream");
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).ok()?;
        let url = web_sys::Url::create_object_url_with_blob(&blob).ok()?;
        let document = web_sys::window()?.document()?;
        let a: web_sys::HtmlAnchorElement = document.create_element("a").ok()?.dyn_into().ok()?;
        a.set_href(&url);
        a.set_download(name);
        let body = document.body()?;
        let _ = body.append_child(&a);
        a.click();
        a.remove();
        // After the browser has started the download.
        let revoke = Closure::once_into_js(move || {
            let _ = web_sys::Url::revoke_object_url(&url);
        });
        let _ =
            web_sys::window()?.set_timeout_with_callback_and_timeout_and_arguments_0(revoke.unchecked_ref(), 60_000);
        Some(())
    };
    go().is_some()
}
