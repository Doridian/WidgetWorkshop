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
`.iso` (the one with `WIN31/SVGA`, where the fonts are). **Start** reads
the image into memory and opens the editor. On later starts the page
shows the image it kept and **Start** goes straight on; picking another
file replaces it. The page never fetches or sends game data: the image
comes from the player.

The Start button is also the click the browser wants before a page may
play sound: the `AudioContext` is resumed on it and on every later click
or key press (`platform::audio::resume`).

## Storage

`web.js` keeps everything in the browser's IndexedDB (database
`widget-workshop`): the CD image as the `Blob` the file input gave
(IndexedDB keeps it without reading it into memory), and the port's own
files, mounted at `/storage` (`platform::storage::WEB_STORAGE`): the
widgets the player saves, `WIDGET.INI` (the solved puzzles) and
`settings.ini`. The editor's file dialogs start and stay there.

## Options

The page's query string takes the command line's options without their
`--`: `?no-audio`, `?ui-scale=2`. A `.WGT` name opens that widget from
the storage (`?MYWIDGET.WGT`). `--overrides`, `--export` and the window's
size mean nothing there. A bad option stops the page with the reason
shown at the bottom. What the port would print on the terminal goes to
the browser's console.
