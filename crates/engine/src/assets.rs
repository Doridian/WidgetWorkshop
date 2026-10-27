//! Where the game's data comes from. The original keeps nearly all of it
//! in the resources of four modules ([`Module`]); the port reads them from
//! the CD (an ISO image or its files, through a [`Source`]) and lets a
//! directory of loose files override or add any of them. The Win95 build's
//! modules are PE files; the OS/2 CD's are LX files, whose dialogs, menu,
//! strings and pointers are turned into the Win95 build's on loading
//! (`formats::os2`, docs/os2.md), so the rest sees one kind:
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
//! `WIDGET.PZL` or `WIDGET.HLP` (or `WIDGET.INF`) at the top replaces the
//! CD's puzzles or help.

use std::collections::BTreeMap;
use std::fmt;

use formats::{fnt, lx, os2, pe, pzl};

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
/// the Win95 build, the Windows 3.1 one, the OS/2 CD's, or the folder
/// itself.
pub const GAME_DIRS: [&str; 4] = ["WIN95/WIDGET", "WIN31/WIDGET", "WIDGET", ""];
/// The puzzle file, next to `WIDGET.EXE` (`0x42d3fc`), or at the top of
/// the overrides.
pub const PUZZLE_FILE: &str = "WIDGET.PZL";
/// The help file, next to `WIDGET.EXE` with its base name (`0x4333f5`),
/// or at the top of the overrides.
pub const HELP_FILE: &str = "WIDGET.HLP";
/// The OS/2 Warp CD's help instead: an IPF book (`formats::inf`), read
/// where there's no [`HELP_FILE`]. The viewer tells them apart by content.
pub const INF_FILE: &str = "WIDGET.INF";
/// The stand-alone widgets' runtime, beside the game (`0x4165a7`).
pub const RUNTIME_FILE: &str = "WIDGET.SLF";
/// Where the sample widgets are, in the game's folder: the Win95 CD's,
/// the OS/2 CD's.
pub const SAMPLE_DIRS: [&str; 2] = ["FUNWIDGE", "WIDGETS"];
/// The Windows 3.1 bitmap fonts the CD carries for its own setup.
pub const FONT_DIR: &str = "WIN31/SVGA";

/// The port's own fonts, behind the CD's: Wine's free MS Sans Serif and
/// Small Fonts (LGPL 2.1 or later, the same metrics as Windows'), and
/// "Widget Sans" (`tools/make-sans-font.py`, Liberation Sans under the SIL
/// OFL 1.1) for the gadgets' "Helvetica". crates/engine/fonts/README.md.
const BUNDLED_FON: [&[u8]; 2] = [
    include_bytes!("../fonts/SSERIFE.FON"),
    include_bytes!("../fonts/SMALLE.FON"),
];
const BUNDLED_FNT: [&[u8]; 3] = [
    include_bytes!("../fonts/WSANS16.FNT"),
    include_bytes!("../fonts/WSANS16B.FNT"),
    include_bytes!("../fonts/WSANS24.FNT"),
];

/// The port's own fonts ([`BUNDLED_FON`], [`BUNDLED_FNT`]).
#[must_use]
pub fn bundled_fonts() -> Vec<fnt::Font> {
    let mut fonts: Vec<fnt::Font> = BUNDLED_FON
        .iter()
        .flat_map(|b| fnt::Font::parse_fon(b).unwrap_or_default())
        .collect();
    fonts.extend(BUNDLED_FNT.iter().filter_map(|b| fnt::Font::parse(b).ok()));
    fonts
}

/// The first of [`GAME_DIRS`] on `cd` that holds `WWGADGET.DLL`.
#[must_use]
pub fn game_dir(cd: &dyn Source) -> Option<&'static str> {
    GAME_DIRS
        .into_iter()
        .find(|d| cd.list(d).iter().any(|n| n.eq_ignore_ascii_case(Module::Gadget.file())))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// None of [`GAME_DIRS`] holds `WWGADGET.DLL`.
    NoGame,
    Module(&'static str, pe::Error),
    /// An OS/2 module that doesn't parse.
    Os2Module(&'static str, lx::Error),
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
            Error::Os2Module(file, e) => write!(f, "{file}: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// The resources of module `m` from its file `bytes`: a PE file's as they
/// are, an LX file's (the OS/2 build) as the Win95 build's would be.
fn parse_module(m: Module, bytes: &[u8]) -> Result<pe::Resources, Error> {
    if lx::is_lx(bytes) {
        let r = lx::Resources::parse(bytes).map_err(|e| Error::Os2Module(m.file(), e))?;
        Ok(os2::to_win(&r))
    } else {
        pe::Resources::parse(bytes).map_err(|e| Error::Module(m.file(), e))
    }
}

/// The game's resources: the overrides' on top of the original modules'.
#[derive(Default)]
pub struct Assets {
    modules: BTreeMap<Module, pe::Resources>,
    overrides: BTreeMap<(Module, u32, u32), Vec<u8>>,
    /// The bitmap fonts: the overrides', the CD's, then
    /// [`bundled_fonts`].
    pub fonts: Vec<fnt::Font>,
    /// The sample widgets on the CD (`FUNWIDGE/*.WGT`, the OS/2 CD's
    /// `WIDGETS/*.WGT`): name and bytes.
    pub samples: Vec<(String, Vec<u8>)>,
    /// The puzzle file (`WIDGET.PZL`, `formats::pzl`), if found: the
    /// overrides', else the CD's.
    pub puzzles: Option<Vec<u8>>,
    /// The help file (`WIDGET.HLP`, `formats::hlp`, or the OS/2 CD's
    /// `WIDGET.INF`, `formats::inf`), if found: the overrides', else the
    /// CD's. Parsed when Help opens.
    pub help: Option<Vec<u8>>,
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
    /// them), its fonts and samples, then the files of `overrides`; the
    /// port's own fonts last.
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
        // After the CD's and the overrides', which win a tie in `font`.
        a.fonts.extend(bundled_fonts());
        Ok(a)
    }

    fn load_cd(&mut self, cd: &dyn Source) -> Result<(), Error> {
        let dir = game_dir(cd).ok_or(Error::NoGame)?;
        let mut os2 = false;
        for m in Module::ALL {
            if let Some(bytes) = cd.read(&join(dir, m.file())) {
                os2 |= lx::is_lx(&bytes);
                self.modules.insert(m, parse_module(m, &bytes)?);
            }
        }
        for fon in ["SSERIFE.FON", "SMALLE.FON"] {
            for d in [FONT_DIR, dir, ""] {
                if let Some(b) = cd.read(&join(d, fon)) {
                    self.fonts.extend(fnt::Font::parse_fon(&b).unwrap_or_default());
                    break;
                }
            }
        }
        let (samples, mut names) = SAMPLE_DIRS
            .iter()
            .map(|s| {
                let s = join(dir, s);
                let names = cd.list(&s);
                (s, names)
            })
            .find(|(_, names)| !names.is_empty())
            .unwrap_or_default();
        names.sort();
        for name in names {
            if name.to_ascii_uppercase().ends_with(".WGT")
                && let Some(b) = cd.read(&join(&samples, &name))
            {
                self.samples.push((name, b));
            }
        }
        self.puzzles = cd.read(&join(dir, PUZZLE_FILE));
        // The OS/2 CD's file was shipped with every puzzle marked solved.
        if os2
            && let Some(p) = &mut self.puzzles
            && let Ok(old) = pzl::Puzzles::parse(p)
        {
            let _ = pzl::write_done(p, &vec![false; old.done.len()]);
        }
        self.help = cd.read(&join(dir, HELP_FILE)).or_else(|| cd.read(&join(dir, INF_FILE)));
        self.runtime = cd.read(&join(dir, RUNTIME_FILE));
        Ok(())
    }

    fn load_overrides(&mut self, o: &dyn Source) {
        if let Some(b) = o.read(PUZZLE_FILE) {
            self.puzzles = Some(b);
        }
        if let Some(b) = o.read(HELP_FILE).or_else(|| o.read(INF_FILE)) {
            self.help = Some(b);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_fonts_have_the_games_faces() {
        let a = Assets {
            fonts: bundled_fonts(),
            ..Assets::default()
        };
        for (face, height) in [
            ("MS Sans Serif", 13),
            ("MS Sans Serif", 16),
            ("Small Fonts", 11),
            ("Widget Sans", 24),
        ] {
            let f = a.font(face, height, false).unwrap();
            assert_eq!((f.face.as_str(), f.height), (face, height));
        }
        let bold = a.font("Widget Sans", 16, true).unwrap();
        assert_eq!((bold.height, bold.weight), (16, 700));
        // Wine's MS Sans Serif has Windows' widths.
        assert_eq!(a.font("MS Sans Serif", 13, false).unwrap().width(b"Cancel"), 33);
    }

    /// A CD image in memory, through `formats::iso`.
    struct Iso(Vec<u8>);

    impl Source for Iso {
        fn read(&self, path: &str) -> Option<Vec<u8>> {
            formats::iso::read_file(&self.0, path)
        }

        fn list(&self, path: &str) -> Vec<String> {
            self.read(path)
                .and_then(|d| formats::iso::records(&d).ok())
                .map(|r| r.into_iter().map(|r| r.name).collect())
                .unwrap_or_default()
        }
    }

    /// The OS/2 Warp CD, from the image `WIDGET_OS2_ISO` names, else
    /// `WW1_01A.ISO` at the top of the repository (skipped without it):
    /// it loads, and its puzzle file comes up with nothing solved.
    #[test]
    fn os2_cd_loads_with_no_puzzle_solved() {
        let iso = std::env::var_os("WIDGET_OS2_ISO").map_or_else(
            || std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../WW1_01A.ISO"),
            std::path::PathBuf::from,
        );
        let Ok(image) = std::fs::read(iso) else {
            eprintln!("no OS/2 CD: skipped");
            return;
        };
        let a = Assets::load(Some(&Iso(image)), None).unwrap();
        assert_eq!(a.samples.len(), 10);
        let p = pzl::Puzzles::parse(a.puzzles.as_ref().unwrap()).unwrap();
        assert_eq!(p.done.len(), 25);
        assert!(p.done.iter().all(|d| !d));
    }
}
