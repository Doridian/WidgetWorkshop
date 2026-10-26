//! The browser's side of the platform (`wasm32`): the console for what
//! the port would print on the terminal, and the page's status line for
//! what the player has to see.

use wasm_bindgen::JsCast;

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
