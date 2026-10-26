# Widget Workshop

A decompilation and native port of **Widget Workshop: A Mad Scientist's
Laboratory** (Maxis, 1995) to Rust, for Linux, Windows, macOS and
WebAssembly.

**Play it in your browser:** <https://doridian.github.io/WidgetWorkshop/> (the
page asks for the CD image, [docs/web.md](docs/web.md)), or download it for
Linux, Windows or macOS from the
[releases](https://github.com/Doridian/WidgetWorkshop/releases)
([Packages](#packages)).

The port opens straight into the editor (the original's intro and main
menu are left out): the parts tray, wiring, the worksheets, undo and the
parts' own settings dialogs; the logic, number, science, sound and time,
input and display parts; puzzle mode; widgets loaded and saved in the
original's `.WGT` format; recording and WAV import; and the help viewer
for `WIDGET.HLP`. It's a non-matching decompilation of the Win95 build's
`WIDGET.EXE`: the code behaves like the original, but doesn't compile to
the same bytes. The stack is winit, wgpu and cpal, with a resizable
window, fullscreen and UI scaling.

This repository contains **no game data files or original code**. You
need your own copy of the game's CD.

## Getting the game

The port reads the **Mac/PC CD** (volume `WIDGETS`), as an `.iso` or as
a folder with its files. The Internet
Archive has it as a zip of the `.iso`:
[widget-workshop-a-mad-scientists-laboratory-1996-maxis-mac-pc](https://archive.org/details/widget-workshop-a-mad-scientists-laboratory-1996-maxis-mac-pc).

It also reads the much smaller **OS/2 Warp CD** (`WW1_01A.ISO`, game folder
`WIDGET/`): its modules are OS/2 executables whose dialogs, menu, strings
and pointers the port turns into the Win95 build's
([docs/os2.md](docs/os2.md)).

The OS/2 CD has no Windows fonts, so the port bundles free stand-ins
(`crates/engine/fonts`): Wine's MS Sans Serif and Small Fonts, and Widget
Sans, rendered from Liberation Sans, for the gadgets' Arial. When the
Mac/PC CD's own fonts (`WIN31/SVGA`) are there, they win.

## Packages

Every push builds the port as one file per platform
(`.github/workflows/build.yml`); a `v*` tag publishes them on its release:

- `widget-workshop-linux-x86_64.AppImage`, `widget-workshop-linux-arm64.AppImage`
- `widget-workshop-windows-x86_64.exe` (the C runtime linked in)
- `widget-workshop-macos.dmg`: `Widget Workshop.app`, for Apple silicon and
  Intel (signed ad hoc only: the first start needs a right click, Open)
- `widget-workshop-web.zip`: the browser build, as on the Pages site

None of them carries game data. Started without a CD (from its icon) and
with no `.iso` or `orig` in the working directory, the port asks for the
CD image with the system's file dialog and keeps its path in its data
directory (`cd-path.txt` in `~/.local/share/widget-workshop`,
`%APPDATA%\Widget Workshop`, `~/Library/Application Support/Widget Workshop`);
it doesn't ask again while the file is there. On Linux the dialog is the
desktop's (xdg-desktop-portal, else zenity). The icon is the port's own
(`tools/make-icon.py`).

## Running

```sh
cargo run --release -p widget                       # the .iso here, else orig/
cargo run --release -p widget -- CD.iso MY.WGT      # a CD and a widget to open
cargo run --release -p widget -- --help
```

Options: `--no-audio`, `--window-size=WxH`, `--fullscreen`, `--windowed`,
`--ui-scale=N|auto` (remembered in the settings file), `--overrides=DIR`
(resources that replace or add to the CD's) and `--export=DIR` (writes
every resource there in that layout and quits). On Linux the sound needs
ALSA's headers to build (`libasound2-dev`).

The browser build: `tools/build-web.sh`, [docs/web.md](docs/web.md).

## Tools

- `tools/gh`, `tools/ghidra/`: headless Ghidra setup and CLI for the decompilation.
- `tools/build-web.sh`: builds the browser version into `web/`.
- `tools/make-icon.py`: draws the port's icon.

## Docs

- [docs/web.md](docs/web.md): the browser build
- [docs/gadget-model.md](docs/gadget-model.md): the gadget model
- [docs/editor-layout.md](docs/editor-layout.md): the editor: main window, toolbars, parts tray, worksheet frame
- [docs/editor-interaction.md](docs/editor-interaction.md): editor interaction: the worksheet
- [docs/dialogs.md](docs/dialogs.md): the parts' settings dialogs
- [docs/puzzles.md](docs/puzzles.md): puzzles
- [docs/help.md](docs/help.md): `WIDGET.HLP` and the Help menu
- [docs/os2.md](docs/os2.md): the OS/2 Warp CD and how its modules differ
- [docs/gadgets/logic.md](docs/gadgets/logic.md): the logic and number parts
- [docs/gadgets/science.md](docs/gadgets/science.md): the science lab parts
- [docs/gadgets/sound-time.md](docs/gadgets/sound-time.md): time and sound parts
- [docs/gadgets/io.md](docs/gadgets/io.md): input and display parts

## Legal

The port's code is under the AGPL-3.0-or-later ([LICENSE](LICENSE)).
Widget Workshop is © Maxis; this is an unofficial port, and no game data
is included.
