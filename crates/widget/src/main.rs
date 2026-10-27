// A window, not a console, on Windows (the release build).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

//! Widget Workshop. Opens straight into the editor (the original's main
//! menu is left out), with a new widget or the one given.
//!
//! `widget [--overrides=DIR] [--export=DIR] [--no-audio]
//! [--window-size=WxH] [--fullscreen|--windowed] [--ui-scale=N|auto] [--help]
//! [CD] [WIDGET.WGT|LINK]` ([`USAGE`]), where `CD` is the game's CD: an
//! ISO image or a folder with its files (default: the one `.iso` in the
//! current directory, else `orig`), and `LINK` a widget link
//! (`engine::link`). `--overrides` names a folder whose
//! files replace or add to the CD's resources (`engine::assets`);
//! `--export` writes every resource there in that layout and quits.
//! Started without a CD and with none here (a packaged app started from
//! its icon), it asks for one (`pick`). In the browser (`web`,
//! `docs/web.md`) the page asks for the CD image and its query is the
//! command line.

/// In the browser `std`'s standard output and error go nowhere: what the
/// port says goes to the console (`platform::web::log`).
#[cfg(target_arch = "wasm32")]
macro_rules! eprintln {
    ($($t:tt)*) => {
        platform::web::log(&format!($($t)*))
    };
}
#[cfg(target_arch = "wasm32")]
macro_rules! println {
    ($($t:tt)*) => {
        platform::web::log(&format!($($t)*))
    };
}

mod cd;
mod files;
#[cfg(not(target_arch = "wasm32"))]
mod pick;
mod profile;
mod sound;
#[cfg(target_arch = "wasm32")]
mod web;

use std::path::PathBuf;

use web_time::Duration;

use engine::assets::{Assets, Source};
use engine::doc::Document;
use engine::editor::{self, Editor};
use platform::{App, Button, Cursor, CursorImage, Frame, Input, UiScale, WindowOptions};

use crate::sound::Sound;

/// The page widget links open natively: the browser build on the
/// project's site (`.github/workflows/build.yml`). In the browser they
/// open the page they were made on.
#[cfg(not(target_arch = "wasm32"))]
const LINK_PAGE: &str = "https://doridian.github.io/WidgetWorkshop/widget.html";

const USAGE: &str = "usage: widget [--overrides=DIR] [--export=DIR] [--no-audio] [--window-size=WxH] \
[--fullscreen|--windowed] [--ui-scale=N|auto] [--help] [CD] [WIDGET.WGT|LINK]

  CD               the game's CD: an ISO image or a folder with its files
                   (default: the .iso in the current directory, else orig)
  WIDGET.WGT       a widget to open
  LINK             a widget link to open (File > Copy Link to Widget's)
  --overrides=DIR  resources that replace or add to the CD's:
                   DIR/<MODULE>/<type>/<id>.<ext>, fonts in DIR/fonts,
                   puzzles in DIR/WIDGET.PZL, help in DIR/WIDGET.HLP
  --export=DIR     write every resource to DIR in that layout and quit
  --no-audio       no sound
  --window-size=WxH, --fullscreen, --windowed
                   the window (remembered in the settings file)
  --ui-scale=N|auto
                   window pixels per game pixel, such as 2 or 1.5, in
                   quarters; auto follows the display's scaling
                   (remembered, ui_scale in the settings file)";

struct Args {
    cd: Option<PathBuf>,
    widget: Option<PathBuf>,
    link: Option<String>,
    overrides: Option<PathBuf>,
    export: Option<PathBuf>,
    no_audio: bool,
    window: WindowOptions,
}

/// Ends the program with `code`. The browser build can't: the page
/// shows that it stopped, and the module stops by panicking.
fn exit(code: i32) -> ! {
    #[cfg(target_arch = "wasm32")]
    {
        if code != 0 {
            platform::web::show_error("Widget Workshop stopped; the console (F12) says why.");
        }
        panic!("Widget Workshop stopped ({code})");
    }
    #[cfg(not(target_arch = "wasm32"))]
    std::process::exit(code)
}

/// `msg` where the player sees it (`pick::fatal`, in the browser the
/// page), and the end.
fn fail(msg: &str) -> ! {
    #[cfg(not(target_arch = "wasm32"))]
    pick::fatal(msg);
    #[cfg(target_arch = "wasm32")]
    {
        eprintln!("{msg}");
        platform::web::show_error(msg);
    }
    exit(1);
}

fn usage_error(msg: &str) -> ! {
    #[cfg(not(target_arch = "wasm32"))]
    pick::fatal(&format!("{msg}\n{USAGE}"));
    #[cfg(target_arch = "wasm32")]
    platform::web::show_error(msg);
    exit(2);
}

fn args(mut it: impl Iterator<Item = std::ffi::OsString>) -> Args {
    let mut a = Args {
        cd: None,
        widget: None,
        link: None,
        overrides: None,
        export: None,
        no_audio: false,
        window: WindowOptions {
            settings_file: platform::settings::settings_file(),
            ..WindowOptions::default()
        },
    };
    while let Some(arg) = it.next() {
        let s = arg.to_string_lossy().into_owned();
        let (key, value) = match s.split_once('=') {
            Some((k, v)) if k.starts_with("--") => (k.to_owned(), Some(v.to_owned())),
            _ => (s.clone(), None),
        };
        let mut value = || {
            value
                .clone()
                .or_else(|| it.next().map(|v| v.to_string_lossy().into_owned()))
        };
        match key.as_str() {
            "--help" | "-h" => {
                println!("{USAGE}");
                exit(0);
            }
            "--overrides" => a.overrides = value().map(PathBuf::from),
            "--export" => a.export = value().map(PathBuf::from),
            "--no-audio" => a.no_audio = true,
            "--fullscreen" => a.window.fullscreen = Some(true),
            "--windowed" => a.window.fullscreen = Some(false),
            "--window-size" => {
                let v = value().unwrap_or_default();
                let size = v
                    .split_once('x')
                    .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
                a.window.size = Some(size.unwrap_or_else(|| usage_error(&format!("bad window size {v}"))));
            }
            "--ui-scale" => {
                let v = value().unwrap_or_default();
                let scale = platform::UiScale::parse(&v);
                a.window.ui_scale = Some(scale.unwrap_or_else(|| usage_error(&format!("bad UI scale {v}"))));
            }
            k if k.starts_with('-') => usage_error(&format!("unknown option {k}")),
            _ => {
                let p = PathBuf::from(&s);
                let is_widget = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("wgt"));
                let is_link = engine::link::decode(&s).is_some();
                if (is_widget || is_link) && a.widget.is_none() && a.link.is_none() {
                    if is_link {
                        a.link = Some(s);
                    } else {
                        a.widget = Some(p);
                    }
                } else if a.cd.is_none() {
                    a.cd = Some(p);
                } else {
                    usage_error(&format!("too many arguments: {s}"));
                }
            }
        }
    }
    a
}

/// The game in the window.
struct Workshop {
    editor: Editor,
    sound: Sound,
    palette: [[u8; 3]; 256],
    theme: platform::theme::SystemTheme,
    /// Where the enhancements' switch is kept (`original=`).
    settings_file: Option<PathBuf>,
}

impl App for Workshop {
    fn update(&mut self, elapsed: Duration) {
        let ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        if let Some(dark) = self.theme.changed() {
            self.editor.set_system_dark(dark);
        }
        #[cfg(target_arch = "wasm32")]
        for l in web::take_links() {
            self.editor.open_link(&l);
        }
        self.editor.update(ms, sound::local_time());
        if let Some(on) = self.editor.take_enhanced()
            && let Some(path) = &self.settings_file
            && let Err(e) = platform::settings::EnhanceSettings::save(path, Some(!on), &[], &[])
        {
            eprintln!("{}: {e}", path.display());
        }
        let cmds = self.editor.take_sounds();
        self.sound.play(cmds);
        self.editor.draw();
    }

    fn input(&mut self, input: Input) {
        let button = |b| match b {
            Button::Right => Some(editor::Button::Right),
            Button::Left => Some(editor::Button::Left),
            Button::Middle => None,
        };
        match input {
            Input::MouseMove { x, y } => self.editor.mouse_move(x, y),
            Input::MouseDown { x, y, button: b } => {
                if let Some(b) = button(b) {
                    self.editor.mouse_down(x, y, b);
                }
            }
            Input::MouseUp { x, y, button: b } => {
                if let Some(b) = button(b) {
                    self.editor.mouse_up(x, y, b);
                }
            }
            Input::Wheel { dx, dy, .. } => self.editor.wheel(dx, dy),
            Input::KeyDown(vk) => self.editor.key_down(vk),
            Input::KeyUp(vk) => self.editor.key_up(vk),
            Input::Close => self.editor.quit(),
        }
    }

    fn frame(&self) -> Frame<'_> {
        let s = &self.editor.surface;
        Frame {
            width: u32::try_from(s.width).unwrap_or(0),
            height: u32::try_from(s.height).unwrap_or(0),
            pixels: &s.pixels,
            palette: &self.palette,
            overlay: None,
            cursor: Cursor::Default,
            cursor_image: match self.editor.pointer() {
                editor::Pointer::Game(i) => self.editor.cursor_picture(i).map(|c| CursorImage {
                    key: u32::from(i),
                    width: c.width,
                    height: c.height,
                    hot: c.hot,
                    rgba: &c.rgba,
                }),
                editor::Pointer::Arrow => None,
            },
            title: Some(self.editor.title()),
        }
    }

    fn finished(&self) -> bool {
        self.editor.finished()
    }

    fn resize(&mut self, width: u32, height: u32) {
        let (w, h) = (width.max(platform::MIN_DESKTOP.0), height.max(platform::MIN_DESKTOP.1));
        self.editor.resize(w, h);
        self.editor.draw();
    }

    fn dropped(&mut self, file: platform::DroppedFile) {
        self.editor.drop_file(editor::Dropped {
            name: file.name,
            path: file.path.map(|p| p.display().to_string()),
            data: file.data,
        });
    }

    fn pasted(&mut self, text: String) {
        self.editor.paste_link(&text);
    }

    fn take_ui_scale(&mut self) -> Option<UiScale> {
        self.editor.take_ui_scale().map(|s| match s {
            editor::settings::Scale::Auto => UiScale::Auto,
            editor::settings::Scale::Fixed(n) => UiScale::Fixed(n),
        })
    }

    fn ui_scale_changed(&mut self, asked: UiScale, scale: f64) {
        let asked = match asked {
            UiScale::Auto => editor::settings::Scale::Auto,
            UiScale::Fixed(n) => editor::settings::Scale::Fixed(n),
        };
        self.editor.set_ui_scale(asked, scale);
    }
}

/// The CD to read when none is given: of the `.iso`s here, the first with
/// the original fonts ([`engine::assets::FONT_DIR`]: the Mac/PC CD, which
/// has the help too), else the first with the game, else the first that
/// opens, else `orig`.
#[cfg(not(target_arch = "wasm32"))]
fn default_cd() -> PathBuf {
    let mut isos: Vec<PathBuf> = std::fs::read_dir(".")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("iso")))
        .collect();
    isos.sort();
    let opened: Vec<(&PathBuf, Box<dyn Source>)> = isos.iter().filter_map(|p| Some((p, cd::open(p).ok()?))).collect();
    opened
        .iter()
        .find(|(_, cd)| cd::has_fonts(cd.as_ref()))
        .or_else(|| opened.iter().find(|(_, cd)| cd::has_game(cd.as_ref())))
        .or(opened.first())
        .map_or_else(|| PathBuf::from("orig"), |(p, _)| (*p).clone())
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    // A panic's message in a box too when started from the icon.
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        report(info);
        if !std::io::IsTerminal::is_terminal(&std::io::stderr()) {
            pick::error_box(&format!("Widget Workshop stopped: {info}"));
        }
    }));
    let args = args(std::env::args_os().skip(1));
    let cd_path = match args.cd.clone() {
        Some(p) => p,
        // Started without a CD, and there's none here: the one picked
        // before, or the player picks it (`pick`).
        None if args.overrides.is_none() && args.export.is_none() => pick::cd(&default_cd()).unwrap_or_else(|| {
            fail("no CD: start it with the game's CD image or folder as its argument, or pick it when it asks")
        }),
        None => default_cd(),
    };
    let cd = match cd::open(&cd_path) {
        Ok(cd) => Some(cd),
        Err(e) if args.overrides.is_some() => {
            eprintln!("{}: {e}; using the overrides alone", cd_path.display());
            None
        }
        Err(e) => fail(&format!("{}: {e}", cd_path.display())),
    };
    let overrides = args
        .overrides
        .clone()
        .map(|d| Box::new(engine::assets::Dir(d)) as Box<dyn Source>);
    let assets = load(&cd_path, cd.as_deref(), overrides.as_deref());
    if let Some(dir) = &args.export {
        match cd::export(&assets, dir) {
            Ok(n) => println!("{n} files written to {}", dir.display()),
            Err(e) => fail(&format!("{}: {e}", dir.display())),
        }
        return;
    }
    if let Err(e) = start(&args, assets) {
        fail(&e.to_string());
    }
}

/// The browser build: `web` sets up the files and then [`start`]s.
#[cfg(target_arch = "wasm32")]
fn main() {
    web::main();
}

/// The game's resources from `cd` and `overrides`; the end if they don't
/// load.
fn load(cd_path: &std::path::Path, cd: Option<&dyn Source>, overrides: Option<&dyn Source>) -> Assets {
    Assets::load(cd, overrides).unwrap_or_else(|e| fail(&format!("{}: {e}", cd_path.display())))
}

/// Opens the editor on `assets` as `args` say: the widget, the sound,
/// the window. Natively it returns when the game ends; in the browser at
/// once, the page's events running the game from then on.
///
/// # Errors
/// If the window or the GPU can't be set up.
fn start(args: &Args, assets: Assets) -> Result<(), platform::Error> {
    let mut doc = Document::new();
    let mut path = None;
    if let Some(p) = &args.widget {
        match platform::fs::read(p)
            .map_err(|e| e.to_string())
            .and_then(|b| Document::load(&assets, &b).map_err(|e| e.to_string()))
        {
            Ok(d) => {
                doc = d;
                path = Some(p.display().to_string());
            }
            Err(e) => eprintln!("{}: {e}", p.display()),
        }
    }
    let palette = assets.palette();
    let mut editor = Editor::new(assets, doc, platform::DEFAULT_DESKTOP.0, platform::DEFAULT_DESKTOP.1);
    editor.path = path;
    // The solved puzzles (`Done=`), kept between runs.
    if let Some(p) = profile::IniProfile::default_file() {
        editor.profile = Box::new(p);
    }
    editor.files = Box::new(files::PlatformFiles);
    // In the browser the dialogs see only its storage: widgets go out as
    // downloads too.
    #[cfg(target_arch = "wasm32")]
    editor.set_download(Box::new(platform::web::download));
    #[cfg(not(target_arch = "wasm32"))]
    let base = LINK_PAGE.to_owned();
    #[cfg(target_arch = "wasm32")]
    let base = web::page();
    editor.set_links(editor::Links {
        base,
        copy: Box::new(platform::clipboard::set_text),
        paste: Box::new(platform::clipboard::request_text),
    });
    if let Some(l) = &args.link {
        editor.open_link(l);
    }
    // `0x415018`: the random generator starts at `(u32)time(NULL)`.
    editor.env.rng = chrono::Utc::now().timestamp().rem_euclid(1 << 32).unsigned_abs();
    let audio = if args.no_audio {
        platform::audio::Audio::silent()
    } else {
        // Without sound there's no microphone either (`NoMic`).
        editor.mic = Box::new(sound::Microphone::default());
        platform::audio::Audio::open()
    };
    let theme = platform::theme::SystemTheme::new();
    editor.set_system_dark(theme.dark());
    // The port's enhancements, unless the settings file says `original`.
    let settings_file = args.window.settings_file.clone();
    let original = settings_file
        .as_deref()
        .is_some_and(|p| platform::settings::EnhanceSettings::load(p).original);
    editor.set_enhanced(!original);
    let app = Workshop {
        editor,
        sound: Sound::new(audio),
        palette,
        theme,
        settings_file,
    };
    platform::run("Widget Workshop", args.window.clone(), app)
}
