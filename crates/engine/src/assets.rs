//! Where the game's data comes from. The original keeps nearly all of it
//! in the resources of four modules ([`Module`]); the port reads them from
//! the CD (an ISO image or its files, through a [`Source`]) and lets a
//! directory of loose files override or add any of them:
//!
//! ```text
//! <overrides>/<MODULE>/<type>/<id>.<ext>
//! ```
//!
//! `MODULE` is `WIDGET`, `WWGADGET`, `WWSOUND` or `WWINTRO`; `type` and
//! `id` are the resource's numbers (1004 picture sheets, 1006 sounds, 1014
//! knots, 1020 texts, ...); the extension doesn't matter (`.hr`, `.wav`,
//! `.txt`, `.bin`). `widget --export DIR` writes every resource in this
//! layout, a starting point for new art. Fonts are `fonts/*.FON`, and a
//! `WIDGET.PZL` at the top replaces the CD's puzzles.

use std::collections::BTreeMap;
use std::fmt;

use formats::{fnt, pe};

/// A tree of files: the CD (an image or a folder), or a folder of
/// overrides. Paths use `/` and match without regard to case.
pub trait Source {
    fn read(&self, path: &str) -> Option<Vec<u8>>;
    /// The names in directory `path` (files and directories).
    fn list(&self, path: &str) -> Vec<String>;
}

/// The modules whose resources the game reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Module {
    /// `WIDGET.EXE`: dialogs, menus, cursors, strings, the palette.
    Exe,
    /// `WWGADGET.DLL`: the gadgets' pictures and tables.
    Gadget,
    /// `WWSOUND.DLL`: the sounds.
    Sound,
    /// `WWINTRO.DLL`: the intro's and the about box's pictures.
    Intro,
}

impl Module {
    pub const ALL: [Module; 4] = [Module::Exe, Module::Gadget, Module::Sound, Module::Intro];

    #[must_use]
    pub fn file(self) -> &'static str {
        match self {
            Module::Exe => "WIDGET.EXE",
            Module::Gadget => "WWGADGET.DLL",
            Module::Sound => "WWSOUND.DLL",
            Module::Intro => "WWINTRO.DLL",
        }
    }

    /// The directory name of its overrides.
    #[must_use]
    pub fn dir(self) -> &'static str {
        match self {
            Module::Exe => "WIDGET",
            Module::Gadget => "WWGADGET",
            Module::Sound => "WWSOUND",
            Module::Intro => "WWINTRO",
        }
    }
}

/// Resource types the game defines.
pub mod rt {
    pub const PICTURES: u32 = 1004;
    pub const PALETTE: u32 = 1003;
    pub const SOUND: u32 = 1006;
    pub const POINTS: u32 = 1010;
    pub const POINT: u32 = 1011;
    pub const RECTS: u32 = 1012;
    pub const RECT: u32 = 1013;
    pub const KNOTS: u32 = 1014;
    pub const TEXT: u32 = 1020;
    pub const INFO: u32 = 1021;
    pub const COMBO: u32 = 1022;
    pub const WIDGET: u32 = 1023;
}

/// Where the game's folder is on a CD or in a folder, most likely first:
/// the Win95 build, the Windows 3.1 one, or the folder itself.
pub const GAME_DIRS: [&str; 4] = ["WIN95/WIDGET", "WIN31/WIDGET", "WIDGET", ""];
/// The puzzle file, next to `WIDGET.EXE` (`0x42d3fc`), or at the top of
/// the overrides.
pub const PUZZLE_FILE: &str = "WIDGET.PZL";
/// The stand-alone widgets' runtime, beside the game (`0x4165a7`).
pub const RUNTIME_FILE: &str = "WIDGET.SLF";
/// The Windows 3.1 bitmap fonts the CD carries for its own setup.
pub const FONT_DIR: &str = "WIN31/SVGA";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// None of [`GAME_DIRS`] holds `WWGADGET.DLL`.
    NoGame,
    Module(&'static str, pe::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoGame => write!(
                f,
                "no Widget Workshop data found (looked for WWGADGET.DLL in {})",
                GAME_DIRS
                    .iter()
                    .map(|d| if d.is_empty() { "the top folder" } else { d })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Error::Module(file, e) => write!(f, "{file}: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// The game's resources: the overrides' on top of the original modules'.
#[derive(Default)]
pub struct Assets {
    modules: BTreeMap<Module, pe::Resources>,
    overrides: BTreeMap<(Module, u32, u32), Vec<u8>>,
    /// The bitmap fonts found, by face.
    pub fonts: Vec<fnt::Font>,
    /// The sample widgets (`FUNWIDGE/*.WGT`) on the CD: name and bytes.
    pub samples: Vec<(String, Vec<u8>)>,
    /// The puzzle file (`WIDGET.PZL`, `formats::pzl`), if found: the
    /// overrides', else the CD's.
    pub puzzles: Option<Vec<u8>>,
    /// The stand-alone widgets' runtime (`WIDGET.SLF`, packed), if found.
    pub runtime: Option<Vec<u8>>,
    /// The sounds made from resources so far.
    sounds: std::cell::RefCell<BTreeMap<u32, crate::gadget::SoundData>>,
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_owned()
    } else {
        format!("{dir}/{name}")
    }
}

impl Assets {
    /// Reads the original modules from `cd` (where [`GAME_DIRS`] finds
    /// them), its fonts and samples, then the files of `overrides`.
    ///
    /// # Errors
    /// If the game's folder isn't on `cd`, or a module there doesn't parse.
    pub fn load(cd: Option<&dyn Source>, overrides: Option<&dyn Source>) -> Result<Assets, Error> {
        let mut a = Assets::default();
        if let Some(cd) = cd {
            a.load_cd(cd)?;
        }
        if let Some(o) = overrides {
            a.load_overrides(o);
        }
        if a.modules.is_empty() && a.overrides.is_empty() {
            return Err(Error::NoGame);
        }
        Ok(a)
    }

    fn load_cd(&mut self, cd: &dyn Source) -> Result<(), Error> {
        let dir = GAME_DIRS
            .iter()
            .find(|d| cd.read(&join(d, Module::Gadget.file())).is_some())
            .ok_or(Error::NoGame)?;
        for m in Module::ALL {
            if let Some(bytes) = cd.read(&join(dir, m.file())) {
                let res = pe::Resources::parse(&bytes).map_err(|e| Error::Module(m.file(), e))?;
                self.modules.insert(m, res);
            }
        }
        for fon in ["SSERIFE.FON", "SMALLE.FON"] {
            for d in [FONT_DIR, *dir, ""] {
                if let Some(b) = cd.read(&join(d, fon)) {
                    self.fonts.extend(fnt::Font::parse_fon(&b).unwrap_or_default());
                    break;
                }
            }
        }
        let samples = join(dir, "FUNWIDGE");
        let mut names = cd.list(&samples);
        names.sort();
        for name in names {
            if name.to_ascii_uppercase().ends_with(".WGT")
                && let Some(b) = cd.read(&join(&samples, &name))
            {
                self.samples.push((name, b));
            }
        }
        self.puzzles = cd.read(&join(dir, PUZZLE_FILE));
        self.runtime = cd.read(&join(dir, RUNTIME_FILE));
        Ok(())
    }

    fn load_overrides(&mut self, o: &dyn Source) {
        if let Some(b) = o.read(PUZZLE_FILE) {
            self.puzzles = Some(b);
        }
        for m in Module::ALL {
            for typ in o.list(m.dir()) {
                let Ok(t) = typ.parse::<u32>() else { continue };
                let tdir = join(m.dir(), &typ);
                for file in o.list(&tdir) {
                    let stem = file.split('.').next().unwrap_or_default();
                    let Ok(id) = stem.parse::<u32>() else { continue };
                    if let Some(b) = o.read(&join(&tdir, &file)) {
                        self.overrides.insert((m, t, id), b);
                    }
                }
            }
        }
        for file in o.list("fonts") {
            if let Some(b) = o.read(&join("fonts", &file)) {
                // Overriding fonts come first.
                let mut fonts = fnt::Font::parse_fon(&b).unwrap_or_default();
                fonts.append(&mut self.fonts);
                self.fonts = fonts;
            }
        }
    }

    /// Resource `id` of type `typ` in module `m`: the override's, else the
    /// original's.
    #[must_use]
    pub fn get(&self, m: Module, typ: u32, id: u32) -> Option<&[u8]> {
        self.overrides
            .get(&(m, typ, id))
            .map(Vec::as_slice)
            .or_else(|| self.modules.get(&m)?.get(typ, id))
    }

    /// The ids of type `typ` in module `m`, originals and overrides,
    /// ascending.
    #[must_use]
    pub fn ids(&self, m: Module, typ: u32) -> Vec<u32> {
        let mut ids: Vec<u32> = self.modules.get(&m).map(|r| r.ids(typ).collect()).unwrap_or_default();
        ids.extend(
            self.overrides
                .range((m, typ, 0)..=(m, typ, u32::MAX))
                .map(|((_, _, id), _)| *id),
        );
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// Every resource, for exporting: module, type, id and bytes.
    pub fn all(&self) -> impl Iterator<Item = (Module, u32, u32, &[u8])> {
        let mut keys: Vec<(Module, u32, u32)> = self
            .modules
            .iter()
            .flat_map(|(m, r)| r.entries.keys().map(move |(t, i)| (*m, *t, *i)))
            .chain(self.overrides.keys().copied())
            .collect();
        keys.sort_unstable();
        keys.dedup();
        keys.into_iter()
            .filter_map(|(m, t, i)| self.get(m, t, i).map(|b| (m, t, i, b)))
    }

    /// String `id` of `WIDGET.EXE`'s string table.
    #[must_use]
    pub fn string(&self, id: u32) -> Option<String> {
        self.modules.get(&Module::Exe)?.string(id)
    }

    /// A text resource (type 1020) as a string (Windows-1252).
    #[must_use]
    pub fn text(&self, m: Module, id: u32) -> Option<String> {
        let b = self.get(m, rt::TEXT, id)?;
        let b = b.split(|&c| c == 0).next().unwrap_or_default();
        Some(crate::text::decode(b))
    }

    /// Sound resource `id` (`WWSOUND.DLL` 1006/`id`) as the game plays it
    /// (`0x43e4a4`): every byte past the 44-byte RIFF header, at 11025 Hz
    /// (the game assumes every sound is a plain 8-bit mono 11025 Hz wave;
    /// the trailing bytes past the `data` chunk play too).
    #[must_use]
    pub fn sound(&self, id: u32) -> Option<crate::gadget::SoundData> {
        if let Some(s) = self.sounds.borrow().get(&id) {
            return Some(s.clone());
        }
        let b = self.get(Module::Sound, rt::SOUND, id)?;
        // A new sound (an override) may be any PCM wave.
        let s = match formats::tables::Wave::parse(b) {
            Some(w) if self.overrides.contains_key(&(Module::Sound, rt::SOUND, id)) => crate::gadget::SoundData {
                rate: u16::try_from(w.rate).unwrap_or(u16::MAX),
                samples: w.samples.into(),
            },
            _ => crate::gadget::SoundData {
                rate: 11025,
                samples: b.get(0x2c..).unwrap_or_default().into(),
            },
        };
        self.sounds.borrow_mut().insert(id, s.clone());
        Some(s)
    }

    /// The palette (`WIDGET.EXE` 1003/101).
    #[must_use]
    pub fn palette(&self) -> [[u8; 3]; 256] {
        self.get(Module::Exe, rt::PALETTE, 101)
            .map_or_else(default_palette, formats::tables::palette)
    }

    /// The font of `face` closest to `height` pixels, if any.
    #[must_use]
    pub fn font(&self, face: &str, height: u16, bold: bool) -> Option<&fnt::Font> {
        self.fonts
            .iter()
            .filter(|f| f.face.eq_ignore_ascii_case(face))
            .min_by_key(|f| (f.height.abs_diff(height), u16::from((f.weight >= 600) != bold)))
    }
}

/// A grey ramp, for running without the game's palette.
fn default_palette() -> [[u8; 3]; 256] {
    let mut p = [[0; 3]; 256];
    for (i, c) in p.iter_mut().enumerate() {
        let v = u8::try_from(255 - i).unwrap_or(0);
        *c = [v, v, v];
    }
    p
}

/// A folder on disk as a [`Source`] (natively).
#[cfg(not(target_arch = "wasm32"))]
pub struct Dir(pub std::path::PathBuf);

#[cfg(not(target_arch = "wasm32"))]
impl Dir {
    /// `path` under the folder, matching each part without regard to case.
    fn find(&self, path: &str) -> Option<std::path::PathBuf> {
        let mut at = self.0.clone();
        for part in path.split('/').filter(|p| !p.is_empty()) {
            let exact = at.join(part);
            if exact.exists() {
                at = exact;
                continue;
            }
            let found = std::fs::read_dir(&at)
                .ok()?
                .flatten()
                .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part))?;
            at = found.path();
        }
        Some(at)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Source for Dir {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        let p = self.find(path)?;
        p.is_file().then(|| std::fs::read(p).ok()).flatten()
    }

    fn list(&self, path: &str) -> Vec<String> {
        self.find(path)
            .and_then(|p| std::fs::read_dir(p).ok())
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    }
}
