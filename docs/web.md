# The browser build

The port runs in a browser as WebAssembly (`wasm32-unknown-unknown`),
without Emscripten: winit draws into a `<canvas>`, wgpu renders with
WebGPU or WebGL2, cpal plays through WebAudio, and wasm-bindgen glues it
to the page. The editor is the same code as natively; only the start,
the files and the clock differ.

## Building

Needs the `wasm32-unknown-unknown` target and the `wasm-bindgen` CLI in
the version the crates pin (`wasm-bindgen = "=0.2.129"` in
`crates/widget/Cargo.toml` and `crates/platform/Cargo.toml`; the CLI
refuses a module from another version):

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129
tools/build-web.sh            # release build into web/
tools/build-web.sh --debug    # quicker to build, slow to run
```

The script runs `cargo build -p widget --target wasm32-unknown-unknown`
and `wasm-bindgen --target web`, and copies the page
(`crates/widget/web/index.html`) and the icon next to the output: `web/`
holds `index.html`, `icon.png`, `widget.js`, `widget_bg.wasm` and
`snippets/` (the storage code, `crates/widget/src/web.js`). `web/` is
ignored by git.

Upgrading wasm-bindgen means changing the pin in both crates and
installing the matching CLI.

## GitHub Pages

`.github/workflows/build.yml` publishes the build on every push to
`main` at <https://doridian.github.io/WidgetWorkshop/>, without game data
(the workflow fails if anything but the pages, the icon, the glue and
the module is in the build). `crates/widget/web/front.html` is the
site's front page: where to get the CD image, the downloads, and a link
to the game, `widget.html` (the build's `index.html`).

## Serving and running

Any static HTTP server does; the module can't load from `file://`:

```sh
python3 -m http.server -d web 8000    # then open http://localhost:8000/
```

On the first start the page asks for the game's CD image: the Mac/PC CD's
`.iso` or the OS/2 CD's `WW1_01A.ISO`. **Start** reads
the image into memory and opens the editor. Later starts skip the form
and open the editor on the image it kept. Options ▸ Settings ▸ **Change
CD...** (after "Save changes?") reloads the page into the form, which
shows the kept image: **Start** goes on with it, picking another file
replaces it. If the kept image no longer reads or opens, the form asks
for one again. The page never fetches or sends game data: the image
comes from the player.

A browser won't let a page play sound before a click or key press: the
`AudioContext` is resumed on every click or key press
(`platform::audio::resume`), so the sound starts with the first.

## Storage

`web.js` keeps everything in the browser's IndexedDB (database
`widget-workshop`): the CD image as the `Blob` the file input gave
(IndexedDB keeps it without reading it into memory), and the port's own
files, mounted at `/storage` (`platform::storage::WEB_STORAGE`): the
widgets the player saves, `WIDGET.INI` (the solved puzzles) and
`settings.ini`. The editor's file dialogs start and stay there.

Widgets also come and go as files of the player's (`editor::download`):

- A file dropped onto the page opens like File ▸ Open, after "Save
  changes?" (`platform::web::watch_drops` reads it). It has no path, so
  the title shows its name and Save goes to Save As with that name, into
  the storage. Natively a file dropped onto the window opens the same
  way, and Save writes back to it.
- File ▸ **Save to Download** hands the widget to the browser as a
  download under its name (`WIDGET.WGT` while untitled), as Save would
  write it; it doesn't count as saved, and the storage is left alone.
  Greyed for a puzzle, which only Save As makes a widget of.
- File ▸ **Download a Widget...** picks a widget in the storage, or one
  of the CD's samples, and downloads it.

These two items are the port's own (commands `0x9001`, `0x9002`), added
after Save Into a Stand-Alone Widget only in the browser.

## Widget links

A widget fits in a link (`engine::link`): the page with the widget in
its fragment, which the browser never sends to the server, so the site
still never sees a widget.

```
https://doridian.github.io/WidgetWorkshop/widget.html#n=CLOCK.WGT&w=Ae1a…
```

`n` is the widget's name (optional, percent-encoded); `w` is base64url,
without padding, of a format byte and the `.WGT` as Save writes it.
Format 1 is raw deflate (`miniz_oxide`; `zlib` with `wbits=-15` makes
the same). A widget's pictures and sounds are the CD's, which whoever
opens the link has too, so the CD's samples are links of 2 to 23 KB,
most 3 to 5; each Recorder adds its 40000-sample recording, which
barely compresses. Browsers take that; chats with a message limit may
not.

- File ▸ **Copy Link to Widget** puts the link to the widget on the
  clipboard, as Save would write it, and says how long it is. In the
  browser the link points at the page it was made on, natively at the
  site's. It doesn't count as saved; greyed for a puzzle, as Save to
  Download is.
- File ▸ **Open Widget Link** (Ctrl+V, Shift+Insert) opens the link on
  the clipboard as a dropped widget (Save goes to Save As with its
  name), after "Save changes?". Its command is MFC's `ID_EDIT_PASTE`
  (`0xe125`), which the CD's accelerators already give those keys and
  the original left unused. The browser may ask before letting the page
  read the clipboard; white space in the link (a chat's line breaks) is
  ignored.
- Opened with a link as its address, the page opens the widget once the
  CD is in, as `?MYWIDGET.WGT` does; a link pasted into the address bar
  of the open page opens too. Natively a link is an argument like a
  widget's file (`widget CD.iso 'https://…#w=…'`).

The clipboard is `platform::clipboard`: arboard natively (X11, Wayland
where the compositor offers data control, Windows, macOS),
`navigator.clipboard` in the browser, which the page only has over
HTTPS or from `localhost`. Both items are the port's own, on every
platform, after the download items (natively after Save Into a
Stand-Alone Widget).

## Options

The page's query string takes the command line's options without their
`--`: `?no-audio`, `?ui-scale=2`. A `.WGT` name opens that widget from
the storage (`?MYWIDGET.WGT`). `--overrides`, `--export` and the window's
size mean nothing there. A bad option stops the page with the reason
shown at the bottom. What the port would print on the terminal goes to
the browser's console.
