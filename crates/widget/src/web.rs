//! The browser build's start (`docs/web.md`). The browser can't block,
//! so this runs as a future on the page's event loop: it asks for the
//! game's CD image (once; the browser keeps it), reads it into memory,
//! mounts the browser's storage where the widgets and the settings go
//! (`platform::storage::WEB_STORAGE`), takes the options from the
//! page's query, and then [`crate::start`]s the editor as natively.
//!
//! The page (`web/index.html`, written by `tools/build-web.sh`) has the
//! form that picks the CD image and the canvas; `web.js` beside this
//! keeps the files in the browser's `IndexedDB`.

use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;

use platform::data::{CdImage, MemoryFiles};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

/// How much of the image is read at a time.
const CHUNK: f64 = 16.0 * 1024.0 * 1024.0;

#[wasm_bindgen(module = "/src/web.js")]
extern "C" {
    /// Shows the start form and resolves to the CD image (a `Blob`) once
    /// the player starts with one; a new one is stored first.
    #[wasm_bindgen(js_name = chooseCd)]
    fn choose_cd(error: &str) -> js_sys::Promise;
    /// Forgets the stored CD image.
    #[wasm_bindgen(js_name = forgetCd)]
    fn forget_cd() -> js_sys::Promise;
    /// `blob`'s bytes from `start` to `end`, a `Uint8Array`.
    #[wasm_bindgen(js_name = readBlob)]
    fn read_blob(blob: &web_sys::Blob, start: f64, end: f64) -> js_sys::Promise;
    /// The stored files: an array of `[path, Uint8Array]`.
    #[wasm_bindgen(js_name = storedFiles)]
    fn stored_files() -> js_sys::Promise;
    /// Stores the file `path`.
    #[wasm_bindgen(js_name = putFile)]
    fn put_file(path: &str, data: &[u8]);
    /// Removes the stored file `path`.
    #[wasm_bindgen(js_name = deleteFile)]
    fn delete_file(path: &str);
}

/// The module's start: shows a panic on the page and starts [`run`].
pub(crate) fn main() {
    std::panic::set_hook(Box::new(|info| platform::web::show_error(&info.to_string())));
    wasm_bindgen_futures::spawn_local(async {
        if let Err(e) = run().await {
            platform::web::show_error(&e);
        }
    });
}

fn js_error(e: &JsValue) -> String {
    e.as_string()
        .or_else(|| e.dyn_ref::<js_sys::Error>().map(|e| String::from(e.message())))
        .unwrap_or_else(|| format!("{e:?}"))
}

async fn wait(p: js_sys::Promise) -> Result<JsValue, String> {
    JsFuture::from(p).await.map_err(|e| js_error(&e))
}

/// The CD image the player picks (or picked before), read and opened;
/// asks again while the image doesn't open or isn't the game's.
async fn cd_image() -> Result<Box<dyn engine::assets::Source>, String> {
    let mut error = String::new();
    loop {
        let blob: web_sys::Blob = wait(choose_cd(&error)).await?.dyn_into().map_err(|e| js_error(&e))?;
        platform::web::show_status("Reading the CD image…", false);
        let size = blob.size();
        // A file's size, whole and below 2^53.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let mut image = Vec::with_capacity(size as usize);
        let mut at = 0.0;
        while at < size {
            let end = (at + CHUNK).min(size);
            let chunk: js_sys::Uint8Array = wait(read_blob(&blob, at, end)).await?.unchecked_into();
            let start = image.len();
            image.resize(start + chunk.length() as usize, 0);
            chunk.copy_to(&mut image[start..]);
            at = end;
        }
        error = match CdImage::open(Box::new(image)) {
            Ok(cd) => {
                let volume = cd.volume.clone();
                let cd = crate::cd::image(cd);
                if crate::cd::has_fonts(cd.as_ref()) {
                    platform::web::log(&format!("the CD image: {volume}"));
                    return Ok(cd);
                }
                format!(
                    "That CD image isn't Widget Workshop's Mac/PC CD: it has no {}.",
                    engine::assets::FONT_DIR
                )
            }
            Err(e) => format!("That file doesn't open as a CD image: {e}."),
        };
        platform::web::show_status("", false);
        wait(forget_cd()).await?;
    }
}

/// The page's query as command-line options: `?no-audio&ui-scale=2` is
/// `--no-audio --ui-scale=2`. A value without a key names a widget in the
/// storage (`?MYWIDGET.WGT`).
fn query_args() -> Vec<OsString> {
    let search = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .unwrap_or_default();
    let decode = |s: &str| {
        let s = s.replace('+', " ");
        js_sys::decode_uri_component(&s).map_or(s, String::from)
    };
    search
        .trim_start_matches('?')
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) if !v.is_empty() => OsString::from(format!("--{}={}", decode(k), decode(v))),
            Some((k, _)) => OsString::from(format!("--{}", decode(k))),
            None if Path::new(&decode(p))
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("wgt")) =>
            {
                OsString::from(format!("{}/{}", platform::storage::WEB_STORAGE, decode(p)))
            }
            None => OsString::from(format!("--{}", decode(p))),
        })
        .collect()
}

async fn run() -> Result<(), String> {
    let cd = cd_image().await?;
    let files: js_sys::Array = wait(stored_files()).await?.unchecked_into();
    let files = files
        .iter()
        .filter_map(|entry| {
            let entry: js_sys::Array = entry.unchecked_into();
            let path = entry.get(0).as_string()?;
            let data: js_sys::Uint8Array = entry.get(1).dyn_into().ok()?;
            Some((path, data.to_vec()))
        })
        .collect();
    let persist: platform::data::Persist = Box::new(|path, data| match data {
        Some(d) => put_file(path, d),
        None => delete_file(path),
    });
    platform::fs::mount(
        platform::storage::WEB_STORAGE,
        Arc::new(MemoryFiles::new(files, Some(persist))),
    );
    platform::web::show_status("", false);
    let args = crate::args(query_args().into_iter());
    for (given, what) in [
        (args.cd.is_some(), "a CD"),
        (args.overrides.is_some(), "--overrides"),
        (args.export.is_some(), "--export"),
    ] {
        if given {
            eprintln!("{what} means nothing in the browser");
        }
    }
    let assets = crate::load(Path::new("CD"), Some(cd.as_ref()), None);
    crate::start(&args, assets).map_err(|e| e.to_string())
}
