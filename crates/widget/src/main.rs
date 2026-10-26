//! Widget Workshop. Opens straight into the editor (the original's main
//! menu is left out), with a new widget or the one given.
//!
//! `widget [--overrides=DIR] [--export=DIR] [--no-audio]
//! [--window-size=WxH] [--fullscreen|--windowed] [--ui-scale=N|auto] [--help]
//! [CD] [WIDGET.WGT]` ([`USAGE`]), where `CD` is the game's CD: an ISO
//! image or a folder with its files (default: the one `.iso` in the
//! current directory, else `orig`). `--overrides` names a folder whose
//! files replace or add to the CD's resources (`engine::assets`);
//! `--export` writes every resource there in that layout and quits.

mod cd;
mod files;
mod profile;
mod sound;

use std::path::PathBuf;
use std::process::exit;

use web_time::Duration;

use engine::assets::{Assets, Source};
use engine::doc::Document;
use engine::editor::{self, Editor};
use platform::{App, Button, Cursor, CursorImage, Frame, Input, WindowOptions};

use crate::sound::Sound;

const USAGE: &str = "usage: widget [--overrides=DIR] [--export=DIR] [--no-audio] [--window-size=WxH] \
[--fullscreen|--windowed] [--ui-scale=N|auto] [--help] [CD] [WIDGET.WGT]

  CD               the game's CD: an ISO image or a folder with its files
                   (default: the .iso in the current directory, else orig)
  WIDGET.WGT       a widget to open
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
    overrides: Option<PathBuf>,
    export: Option<PathBuf>,
    no_audio: bool,
    window: WindowOptions,
}

fn usage_error(msg: &str) -> ! {
    eprintln!("{msg}\n{USAGE}");
    exit(2);
}

fn args(mut it: impl Iterator<Item = std::ffi::OsString>) -> Args {
    let mut a = Args {
        cd: None,
        widget: None,
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
                if is_widget && a.widget.is_none() {
                    a.widget = Some(p);
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
}

impl App for Workshop {
    fn update(&mut self, elapsed: Duration) {
        let ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        self.editor.update(ms, sound::local_time());
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
}

/// The CD to read when none is given: of the `.iso`s here, the first with
/// the fonts ([`engine::assets::FONT_DIR`]), else the first that opens,
/// else `orig`.
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
    let has_fonts = |cd: &dyn Source| !cd.list(engine::assets::FONT_DIR).is_empty();
    opened
        .iter()
        .find(|(_, cd)| has_fonts(cd.as_ref()))
        .or(opened.first())
        .map_or_else(|| PathBuf::from("orig"), |(p, _)| (*p).clone())
}

fn main() {
    let args = args(std::env::args_os().skip(1));
    let cd_path = args.cd.clone().unwrap_or_else(default_cd);
    let cd = match cd::open(&cd_path) {
        Ok(cd) => Some(cd),
        Err(e) if args.overrides.is_some() => {
            eprintln!("{}: {e}; using the overrides alone", cd_path.display());
            None
        }
        Err(e) => {
            eprintln!("{}: {e}", cd_path.display());
            exit(1);
        }
    };
    let overrides = args.overrides.clone().map(engine::assets::Dir);
    let assets = match Assets::load(cd.as_deref(), overrides.as_ref().map(|o| o as &dyn Source)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{}: {e}", cd_path.display());
            exit(1);
        }
    };
    if assets.fonts.is_empty() {
        eprintln!(
            "{}: no fonts in {} (or the overrides' fonts folder); text won't be drawn",
            cd_path.display(),
            engine::assets::FONT_DIR
        );
    }
    if let Some(dir) = &args.export {
        match cd::export(&assets, dir) {
            Ok(n) => println!("{n} files written to {}", dir.display()),
            Err(e) => {
                eprintln!("{}: {e}", dir.display());
                exit(1);
            }
        }
        return;
    }
    let mut doc = Document::new();
    let mut path = None;
    if let Some(p) = &args.widget {
        match std::fs::read(p)
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
    // `0x415018`: the random generator starts at `(u32)time(NULL)`.
    editor.env.rng = chrono::Utc::now().timestamp().rem_euclid(1 << 32).unsigned_abs();
    let audio = if args.no_audio {
        platform::audio::Audio::silent()
    } else {
        // Without sound there's no microphone either (`NoMic`).
        editor.mic = Box::new(sound::Microphone::default());
        platform::audio::Audio::open()
    };
    let app = Workshop {
        editor,
        sound: Sound::new(audio),
        palette,
    };
    if let Err(e) = platform::run("Widget Workshop", args.window, app) {
        eprintln!("{e}");
        exit(1);
    }
}
