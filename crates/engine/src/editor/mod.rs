//! The editor: the main window (`CMainFrame`, `CWidgetView`) with the
//! worksheet (`cWorksheetPane`), its two floating toolbars, the menu bar,
//! and the widget running in it. docs/editor-layout.md and
//! docs/editor-interaction.md describe the original.
//!
//! The original runs Mac-style modal loops while the mouse is down; here
//! the same steps are a [`Drag`] advanced by the mouse events. Everything
//! is drawn into one 8-bit surface: the menu bar on top, the worksheet
//! (the view's client area) below it.

pub mod about;
pub mod dialog;
mod file;
pub mod file_dialog;
pub mod menu;
pub mod prompt;
mod puzzle;
pub mod puzzle_dialog;
mod recorder;
mod standalone;
pub mod toolbar;
pub mod tray;
pub mod ui;
pub mod worksheet;

use formats::cursor::Cursor as CursorPic;
use formats::pe::{RT_CURSOR, RT_GROUP_CURSOR};

use crate::assets::{Assets, Module};
use crate::doc::{self, Document};
use crate::files::{Files, StdFiles};
use crate::gadget::{DrawCx, Gadget};
use crate::gfx::{IRect, Pictures, Surface};
use crate::puzzle::{Current, MemoryProfile, Profile};
use crate::sim::{Env, Mic, NoMic, SoundCmd};

use self::dialog::Dialog;
use self::file::Step;
use self::file_dialog::FileDialog;
use self::menu::{Menu, Outcome, cmd};
use self::prompt::{Answer, Prompt};
use self::puzzle_dialog::PuzzleDialog;
use self::toolbar::{Bar, MOVE_BAR, id};
use self::tray::{Tool, Tray};
use self::worksheet::{Loose, Sel};

/// Mac ticks per second: the original's clock (`TickCount`).
pub const TICKS_PER_SECOND: u64 = 60;
/// Windows' double-click time and rectangle (`GetDoubleClickTime`,
/// `SM_CXDOUBLECLK`).
const DOUBLE_CLICK_MS: u64 = 500;
const DOUBLE_CLICK_D: i32 = 2;
/// The title of message boxes (`AfxMessageBox`).
const APP: &str = "Widget Workshop";
/// The document's default name (string 2).
const UNTITLED: &str = "untitled.wgt";

/// The Label's tag (`TxLb`).
const LABEL: crate::gadget::Tag = crate::gadget::Tag::new(b"TxLb");

const VK_SHIFT: u16 = 0x10;
const VK_CONTROL: u16 = 0x11;
const VK_MENU: u16 = 0x12;
const VK_RETURN: u16 = 0x0d;
const VK_ESCAPE: u16 = 0x1b;
const VK_DELETE: u16 = 0x2e;
const VK_ADD: u16 = 0x6b;
const VK_SUBTRACT: u16 = 0x6d;
const VK_SEPARATOR: u16 = 0x6c;
const VK_F10: u16 = 0x79;

/// UI sounds (`0x4402cc`; `WWSOUND` type 1006).
mod snd {
    pub const CLICK: u32 = 0x80;
    pub const WIRE_PICK: u32 = 0x81;
    pub const WIRE: u32 = 0x82;
    pub const WIRE_SNAP: u32 = 0x83;
    pub const SELECT: u32 = 0x84;
    pub const PICK: u32 = 0x85;
    pub const DROP: u32 = 0x86;
    pub const DEL_GADGET: u32 = 0x87;
    pub const DEL_WIRE: u32 = 0x88;
    pub const DEL_JOINT: u32 = 0x89;
    pub const DIALOG_OPEN: u32 = 0x8a;
    pub const DIALOG_CLOSE: u32 = 0x8b;
    pub const LIGHT_ON: u32 = 0x8c;
    pub const LIGHT_OFF: u32 = 0x8d;
    pub const GO: u32 = 0x8e;
    pub const STOP: u32 = 0x8f;
    pub const SOUND_ON: u32 = 0x90;
    pub const SOUND_OFF: u32 = 0x91;
    pub const PLACED: u32 = 0x92;
    pub const TAKEN: u32 = 0x93;
    pub const ARROW: u32 = 0x94;
    pub const SLIDE: u32 = 0x95;
    pub const MOVING: u32 = 0x96;
}

/// Mouse buttons the editor tells apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
}

/// The mouse pointer: the system's arrow (over the toolbars, the menu and
/// boxes: the `AfxWnd` class cursor) or one of the editor's seven
/// (`DAT_004a2d60`, groups 5007..5013).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pointer {
    Arrow,
    /// 0 glove, 1 scissors, 2 plug (knot), 3 grabbing hand, 4 open hand
    /// (Move), 5 plug (wire), 6 pointing hand.
    Game(u8),
}

/// Which toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Main,
    Tray,
}

/// What the mouse is doing while its button is down (the original's
/// tracking loops and input tools).
enum Drag {
    /// A gadget follows the pointer (`0x426878`).
    Gadget {
        g: usize,
        last: (i32, i32),
        moved: bool,
        voice: Option<u64>,
    },
    /// A joint follows the pointer (`0x426b94`).
    Joint { w: usize, j: usize },
    /// A new wire from knot `k` of gadget `g` (`0x426df7`).
    NewWire {
        g: usize,
        k: usize,
        from_output: bool,
        free: (i32, i32),
        snap: Option<(usize, usize)>,
        hit: Option<(usize, usize)>,
    },
    /// Wire `w`'s end picked up from knot `orig` (`0x4272f7`).
    WireEnd {
        w: usize,
        source: bool,
        orig: (usize, usize),
        free: (i32, i32),
        snap: Option<(usize, usize)>,
        hit: Option<(usize, usize)>,
    },
    /// The Move tool pans the widget (`0x426313`).
    Pan { last: (i32, i32) },
    /// A toolbar dragged by its move bar; the outline's place.
    Bar {
        which: Which,
        grab: (i32, i32),
        at: (i32, i32),
    },
    /// A toolbar's push button held.
    Press(Which),
    /// A new part from the tray (`CWPlaceGadgetTool`).
    Place {
        gadget: Box<Gadget>,
        index: u8,
        hot: (i32, i32),
    },
    /// The mouse went down on the menu.
    Menu,
    /// The mouse went down on a box or dialog.
    Modal,
}

/// What follows a message box's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Then {
    /// Just a message.
    Nothing,
    /// "Save changes to %s?", then this.
    SaveChanges(Next),
    /// The name of the file to open.
    OpenPath,
    /// The name to save as, then this.
    SaveAsPath(Option<Next>),
    /// "Revert to last saved version?"
    Revert,
    /// Dialog 10014's question (without its template), then the save.
    Fit(Option<Step>),
    /// Dialog 10010's question (without its template), then the save.
    MacFit(Step),
    /// The stand-alone to write.
    SaveInto,
    /// The stand-alone to import.
    ImportExe,
    /// "Reset Completed Puzzles"' question.
    ResetPuzzles,
    /// The wave file to import into Recorder `g`.
    ImportWav(usize),
}

/// What comes after the "Save changes?" question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Next {
    New,
    Open,
    LoadPuzzle,
    Import,
    Exit,
}

/// The editor, drawn into an 8-bit surface.
#[allow(clippy::struct_excessive_bools)]
pub struct Editor {
    pub assets: Assets,
    pub doc: Document,
    /// The file the widget came from or was saved to.
    pub path: Option<String>,
    pictures: Pictures,
    pub surface: Surface,
    /// Running (`DAT_00493dde == 0`).
    pub running: bool,
    pub env: Env,
    /// Milliseconds not yet made into ticks.
    ms_rest: u64,
    /// The editor's clock in milliseconds.
    now: u64,
    finished: bool,
    /// The Light and Sound buttons (`DAT_00493ddc`, `DAT_00493ddd`).
    pub light: bool,
    pub sound: bool,
    main_bar: Bar,
    tray: Tray,
    /// The toolbars' order, the top one last.
    bars: [Which; 2],
    pub sel: Sel,
    drag: Option<Drag>,
    /// The mouse in frame coordinates.
    mouse: (i32, i32),
    last_click: Option<(u64, (i32, i32))>,
    shift: bool,
    ctrl: bool,
    alt: bool,
    alt_alone: bool,
    /// The one undo step (`cUndo`): the worksheet as a saved widget.
    undo: Option<Vec<u8>>,
    /// Changed since saved (`cFile+0x100`).
    pub dirty: bool,
    menu: Menu,
    prompt: Option<(Prompt, Then)>,
    dialog: Option<Dialog>,
    about: Option<about::About>,
    cursors: Vec<Option<CursorPic>>,
    title: String,
    /// The puzzle loaded (`cPuzzle`) and the Load Puzzle dialog.
    puzzle: Current,
    puzzle_dialog: Option<PuzzleDialog>,
    /// Where the solved puzzles are kept (`WIDGET.INI`'s section).
    pub profile: Box<dyn Profile>,
    /// The sound input (the Recorder's and the Sound Switch's).
    pub mic: Box<dyn Mic>,
    recording: Option<recorder::Recording>,
    /// The folder the last wave file came from (`DAT_004a2d90`).
    wav_dir: Option<std::path::PathBuf>,
    /// The player's files (opened, saved, browsed).
    pub files: Box<dyn Files>,
    /// The open file dialog and what its answer is for.
    file_dialog: Option<(FileDialog, Then)>,
    /// The folder the widget dialogs start in (`DAT_00491076`), once one
    /// was chosen.
    pub file_dir: Option<std::path::PathBuf>,
    /// The widget came from a stand-alone `.EXE` (`cFile+0x101`): Revert
    /// reads it from there.
    from_exe: bool,
    /// What follows dialog 10014 or 10010: a save.
    fit_then: Option<Step>,
}

impl Editor {
    #[must_use]
    pub fn new(assets: Assets, doc: Document, width: u32, height: u32) -> Editor {
        let pictures = Pictures::default();
        let tray = Tray::new(&assets, &pictures);
        let menu = Menu::load(&assets);
        let cursors = (0..7)
            .map(|i| {
                let group = assets.get(Module::Exe, RT_GROUP_CURSOR, 5007 + i)?;
                let id = formats::cursor::group_first(group)?;
                CursorPic::parse(assets.get(Module::Exe, RT_CURSOR, u32::from(id))?)
            })
            .collect();
        let mut e = Editor {
            assets,
            doc,
            path: None,
            pictures,
            surface: Surface::new(width, height),
            running: false,
            // Off the frame until the mouse first moves, so the Win / Lose
            // Checker's cheat doesn't see (0, 0) before it's been there.
            env: {
                let mut env = Env::default();
                env.cursor_screen = (-1, -1);
                env
            },
            ms_rest: 0,
            now: 0,
            finished: false,
            light: true,
            sound: true,
            main_bar: toolbar::main_toolbar(),
            tray,
            bars: [Which::Main, Which::Tray],
            sel: Sel::None,
            drag: None,
            mouse: (-1, -1),
            last_click: None,
            shift: false,
            ctrl: false,
            alt: false,
            alt_alone: false,
            undo: None,
            dirty: false,
            menu,
            prompt: None,
            dialog: None,
            about: None,
            cursors,
            title: String::new(),
            puzzle: Current::default(),
            puzzle_dialog: None,
            profile: Box::new(MemoryProfile::default()),
            mic: Box::new(NoMic),
            recording: None,
            wav_dir: None,
            files: Box::new(StdFiles),
            file_dialog: None,
            file_dir: None,
            from_exe: false,
            fit_then: None,
        };
        let limits = e.doc.tray_limits.clone();
        e.tray.apply_limits(&limits);
        e.update_menu();
        e
    }

    /// The desktop is now `width` × `height`.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface.resize(width, height);
    }

    #[must_use]
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// The window's close button: Exit.
    pub fn quit(&mut self) {
        if self.prompt.is_some()
            || self.dialog.is_some()
            || self.file_dialog.is_some()
            || self.puzzle_dialog.is_some()
            || self.recording.is_some()
        {
            return;
        }
        self.confirm_save(Next::Exit);
    }

    /// The window's title (`0x417092`): "Widget Workshop - name".
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The worksheet (the view's client area) in the frame.
    #[must_use]
    pub fn worksheet(&self) -> IRect {
        IRect::new(0, menu::BAR_H, self.surface.width, self.surface.height)
    }

    /// The worksheet's own rectangle (its coordinates start at 0, 0).
    fn view(&self) -> IRect {
        let w = self.worksheet();
        IRect::new(0, 0, w.width(), w.height())
    }

    /// Frame to worksheet coordinates.
    fn to_view(p: (i32, i32)) -> (i32, i32) {
        (p.0, p.1 - menu::BAR_H)
    }

    /// `p` kept in the worksheet (`ClipCursor` to the view while editing).
    fn clip(&self, p: (i32, i32)) -> (i32, i32) {
        let v = self.view();
        (p.0.clamp(0, (v.x1 - 1).max(0)), p.1.clamp(0, (v.y1 - 1).max(0)))
    }

    fn bar(&self, w: Which) -> &Bar {
        match w {
            Which::Main => &self.main_bar,
            Which::Tray => &self.tray.bar,
        }
    }

    fn bar_mut(&mut self, w: Which) -> &mut Bar {
        match w {
            Which::Main => &mut self.main_bar,
            Which::Tray => &mut self.tray.bar,
        }
    }

    /// The topmost visible toolbar at `p` (worksheet coordinates).
    fn bar_at(&self, p: (i32, i32)) -> Option<Which> {
        self.bars
            .iter()
            .rev()
            .copied()
            .find(|&w| self.bar(w).visible && self.bar(w).rect().contains(p.0, p.1))
    }

    fn raise_bar(&mut self, w: Which) {
        if self.bars[1] != w {
            self.bars.swap(0, 1);
        }
    }

    /// Whether `p` (frame coordinates) is over the worksheet itself, not a
    /// toolbar, the menu or a box (`WindowFromPoint == view`).
    fn over_view(&self, p: (i32, i32)) -> bool {
        if self.prompt.is_some()
            || self.dialog.is_some()
            || self.file_dialog.is_some()
            || self.puzzle_dialog.is_some()
            || self.about.is_some()
            || self.menu.covers(&self.assets, p)
        {
            return false;
        }
        let q = Self::to_view(p);
        self.view().contains(q.0, q.1) && self.bar_at(q).is_none()
    }

    // ---- sound ------------------------------------------------------

    /// A UI sound (`0x4402cc`), silent while Sound is off.
    fn ui_sound(&mut self, id: u32) {
        if self.sound
            && let Some(s) = self.assets.sound(id)
        {
            let voice = self.env.voice();
            self.env.sounds.push(SoundCmd::Play {
                voice,
                sound: s,
                looping: false,
            });
        }
    }

    fn loop_sound(&mut self, id: u32) -> Option<u64> {
        if !self.sound {
            return None;
        }
        let s = self.assets.sound(id)?;
        let voice = self.env.voice();
        self.env.sounds.push(SoundCmd::Play {
            voice,
            sound: s,
            looping: true,
        });
        Some(voice)
    }

    /// The sounds asked for since the last call; none while Sound is off
    /// (every play wrapper checks `DAT_00493ddd`).
    pub fn take_sounds(&mut self) -> Vec<SoundCmd> {
        let mut v = std::mem::take(&mut self.env.sounds);
        if !self.sound {
            v.retain(|c| !matches!(c, SoundCmd::Play { .. }));
        }
        v
    }

    // ---- running ----------------------------------------------------

    /// GO (`0x430e70`).
    pub fn run(&mut self) {
        if self.running {
            return;
        }
        self.running = true;
        self.ui_sound(snd::GO);
        self.tray.bar.visible = false;
        self.tray.select_tool(Tool::Pointer);
        self.sel = Sel::None;
        self.env.ticks = 0;
        self.env.ms = 0;
        self.env.exclusive = None;
        self.ms_rest = 0;
        self.doc.start(&self.assets, &mut self.env);
        self.main_bar.select(id::GO);
        self.update_menu();
    }

    /// STOP (`0x430ee7`): every sound stops, every gadget resets.
    pub fn stop(&mut self) {
        if !self.running {
            return;
        }
        self.running = false;
        self.env.sounds.push(SoundCmd::StopAll);
        self.doc.stop();
        // The Sound Switches' reset (`0x452c52`): `listen_stop`.
        self.env.listen.stop();
        self.poll_listen();
        self.env.exclusive = None;
        self.env.mouse_down = false;
        self.ui_sound(snd::STOP);
        self.tray.bar.visible = true;
        self.main_bar.select(id::STOP);
        self.update_menu();
    }

    /// Advances the clock by `ms`: ticks the running widget once per Mac
    /// tick, or (editing) the hover selection (`0x424778`).
    pub fn update(&mut self, ms: u64, local_time: (u32, u32, u32)) {
        self.now += ms;
        self.env.local_time = local_time;
        if self.recording.is_some() {
            self.poll_recording();
        }
        if !self.running {
            self.idle();
            self.update_menu();
            return;
        }
        self.poll_listen();
        self.ms_rest += ms * TICKS_PER_SECOND;
        // At most a few ticks to catch up after a stall.
        let mut n = 0;
        while self.ms_rest >= 1000 && n < 8 {
            self.ms_rest -= 1000;
            self.doc.tick(&self.assets, &mut self.env);
            if let Some(end) = self.env.end_run.take() {
                // A Win / Lose Checker ended the run: a win marks the
                // puzzle solved (`0x42d4b1`), then the run stops
                // (`0x430ee7`) and its sound plays.
                if end.solved {
                    self.mark_solved();
                }
                self.stop();
                if let Some(sound) = self.assets.sound(end.sound) {
                    let voice = self.env.voice();
                    self.env.sounds.push(SoundCmd::Play {
                        voice,
                        sound,
                        looping: false,
                    });
                }
                return;
            }
            self.env.keys.clear();
            self.env.ticks = self.env.ticks.wrapping_add(1);
            self.env.ms += 1000 / TICKS_PER_SECOND;
            n += 1;
        }
        if n == 8 {
            self.ms_rest = 0;
        }
        self.update_menu();
    }

    // ---- selection --------------------------------------------------

    /// The idle hover (`0x424778`): with the Pointer or Cut over the
    /// worksheet, what is under the pointer is selected.
    fn idle(&mut self) {
        if self.drag.is_some()
            || self.prompt.is_some()
            || self.dialog.is_some()
            || self.file_dialog.is_some()
            || self.puzzle_dialog.is_some()
            || self.about.is_some()
            || self.menu.is_open()
        {
            return;
        }
        let over = self.over_view(self.mouse);
        if matches!(self.tray.tool(), Tool::Pointer | Tool::Cut) && over {
            self.hover(Self::to_view(self.mouse));
        } else {
            self.sel = Sel::None;
        }
        if !matches!(self.sel, Sel::Gadget(_)) && over {
            self.tray.set_help(4000, &self.assets);
        }
    }

    /// `0x425800`: a joint, else a wire, else a gadget.
    fn hover(&mut self, p: (i32, i32)) {
        if let Some((w, j)) = worksheet::joint_at(&self.doc, p) {
            self.select(Sel::Joint(w, j));
        } else if let Some((w, s)) = worksheet::wire_at(&self.doc, p) {
            self.select(Sel::Wire(w, s));
        } else if let Some(g) = worksheet::gadget_at(&self.doc, p) {
            self.select(Sel::Gadget(g));
        } else {
            self.sel = Sel::None;
        }
    }

    fn locked(&self, s: Sel) -> bool {
        match s {
            Sel::None => false,
            Sel::Gadget(g) => self.doc.gadgets.get(g).is_some_and(|g| g.core.locked),
            Sel::Wire(w, _) | Sel::Joint(w, _) => worksheet::wire_locked(&self.doc, w),
        }
    }

    /// `SelectJoint`/`SelectWire`/`SelectGadget` (`0x425bf2`, `0x425431`,
    /// `0x4254ba`): a change plays 0x84; locked things aren't kept.
    fn select(&mut self, s: Sel) {
        if s == self.sel {
            return;
        }
        let locked = self.locked(s);
        if !locked {
            self.ui_sound(snd::SELECT);
        }
        self.sel = Sel::None;
        if !locked {
            self.sel = s;
            if let Sel::Gadget(g) = s
                && let Some(i) = tray::index_of(self.doc.gadgets[g].core.tag)
            {
                self.tray.set_help(2500 + i32::from(i), &self.assets);
            }
        }
    }

    // ---- undo and the document ------------------------------------

    /// `UndoSnapshot` (`0x4154b4`): the worksheet as it is now.
    fn snapshot(&mut self) {
        self.doc.tray_limits = self.tray.limits();
        self.undo = Some(self.doc.save());
    }

    fn set_dirty(&mut self) {
        self.dirty = true;
    }

    /// Edit ▸ Undo (`0x415546`): a toggle between the snapshot and now.
    pub fn undo(&mut self) {
        let Some(buf) = self.undo.take() else {
            self.clear_worksheet();
            return;
        };
        self.snapshot();
        match Document::load(&self.assets, &buf) {
            Ok(d) => {
                self.doc = d;
                let limits = self.doc.tray_limits.clone();
                self.tray.apply_limits(&limits);
            }
            Err(_) => {
                self.undo = None;
            }
        }
        self.sel = Sel::None;
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.undo.is_some()
    }

    /// `0x424b27`: everything gone, plain background.
    fn clear_worksheet(&mut self) {
        self.sel = Sel::None;
        self.doc.gadgets.clear();
        self.doc.wires.clear();
        self.doc.background = 0;
    }

    /// Removes gadget `g`, giving it back to the tray (`0x4245d6`).
    fn delete_gadget(&mut self, g: usize) {
        self.sel = Sel::None;
        let gadget = self.doc.remove_gadget(g);
        if let Some(i) = tray::index_of(gadget.core.tag) {
            self.tray.give_back(i);
        }
    }

    /// Puts a widget in the editor: the tray reset and given its limits,
    /// the Pointer, no undo (`0x416348`).
    fn install(&mut self, doc: Document, path: Option<String>) {
        self.stop();
        self.doc = doc;
        self.undo = None;
        self.tray.reset_counts();
        let limits = self.doc.tray_limits.clone();
        self.tray.apply_limits(&limits);
        self.tray.select_tool(Tool::Pointer);
        self.sel = Sel::None;
        self.path = path;
        self.dirty = false;
        self.puzzle.active = false;
        self.from_exe = false;
    }

    /// Opens the widget file `path` (the player's, one of the CD's samples
    /// in its folder, `files::CD`, or one of those by name).
    pub fn open_path(&mut self, path: &str) -> bool {
        let bytes = self.fs().read(std::path::Path::new(path)).ok().or_else(|| {
            let name = std::path::Path::new(path).file_name()?.to_string_lossy().into_owned();
            self.assets
                .samples
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(&name))
                .map(|(_, b)| b.clone())
        });
        let Some(bytes) = bytes else {
            self.alert(3);
            return false;
        };
        if let Ok(d) = Document::load(&self.assets, &bytes) {
            self.install(d, Some(path.to_owned()));
            true
        } else {
            self.alert(7);
            false
        }
    }

    /// Writes the widget to `path`.
    fn write(&mut self, path: &str) -> bool {
        self.doc.tray_limits = self.tray.limits();
        let bytes = self.doc.save();
        if self.files.write(std::path::Path::new(path), &bytes).is_err() {
            self.alert(4);
            return false;
        }
        self.path = Some(path.to_owned());
        self.dirty = false;
        // Saved, it's an ordinary widget (`cPuzzle::Clear`).
        self.puzzle.active = false;
        true
    }

    /// The name shown in the title and the save question (`0x416ee2`):
    /// "puzzle" for a puzzle.
    fn display_name(&self) -> String {
        if self.puzzle.active {
            return crate::puzzle::DOC_NAME.to_owned();
        }
        self.path
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_name())
            .map_or_else(|| UNTITLED.to_owned(), |n| n.to_string_lossy().into_owned())
    }

    /// Alert `n` (string `8000 + n`, `0x410b5b`).
    fn alert(&mut self, n: u32) {
        let text = self.assets.string(8000 + n).unwrap_or_default();
        self.message(&text, &[Answer::Ok], Then::Nothing);
    }

    fn message(&mut self, text: &str, buttons: &[Answer], then: Then) {
        self.prompt = Some((Prompt::new(APP, text, buttons), then));
    }

    /// "Save changes to %s?" when dirty (`0x410b24`), then `next`. New,
    /// Open and Load Puzzle drop a puzzle's changes without asking; Exit
    /// asks (`0x417027`).
    fn confirm_save(&mut self, next: Next) {
        if self.dirty && (next == Next::Exit || !self.puzzle.active) {
            let text = format!("Save changes to {}?", self.display_name());
            self.message(
                &text,
                &[Answer::Yes, Answer::No, Answer::Cancel],
                Then::SaveChanges(next),
            );
        } else {
            self.after_save(next);
        }
    }

    fn after_save(&mut self, next: Next) {
        match next {
            Next::New => {
                self.install(Document::new(), None);
            }
            // The field starts empty (`DAT_0049117a`).
            Next::Open => self.ask_file(file_dialog::OPEN_WIDGET, "", Then::OpenPath),
            Next::LoadPuzzle => self.choose_puzzle(),
            Next::Import => self.import_ask(),
            Next::Exit => self.finished = true,
        }
    }

    /// A Yes / No / Cancel answer as a squeeze question's.
    fn yes_no(a: Answer) -> Option<bool> {
        match a {
            Answer::Yes => Some(true),
            Answer::No => Some(false),
            _ => None,
        }
    }

    fn answer(&mut self, a: Answer, then: Then, input: Option<String>) {
        match then {
            Then::Nothing => {}
            Then::SaveChanges(next) => match a {
                Answer::Yes => {
                    self.stop();
                    // `saved` goes on.
                    self.save(Some(next));
                }
                Answer::No => self.after_save(next),
                _ => {}
            },
            Then::OpenPath => {
                if a == Answer::Ok
                    && let Some(p) = input.filter(|p| !p.is_empty())
                {
                    self.open_path(&p);
                }
            }
            Then::SaveAsPath(next) => {
                let saved = match input.filter(|p| !p.is_empty()) {
                    Some(p) if a == Answer::Ok => {
                        self.unlock_for_save();
                        self.write(&p)
                    }
                    _ => false,
                };
                self.saved(next, saved);
            }
            Then::Revert => {
                if a == Answer::Yes {
                    if self.puzzle.active {
                        self.revert_puzzle();
                    } else if self.from_exe {
                        self.revert_exe();
                    } else if let Some(p) = self.path.clone() {
                        self.open_path(&p);
                    }
                }
            }
            Then::ResetPuzzles => self.reset_puzzles(a),
            Then::ImportWav(g) => {
                if a == Answer::Ok
                    && let Some(p) = input.filter(|p| !p.is_empty())
                {
                    self.import_wav_path(g, &p);
                }
            }
            Then::Fit(step) => {
                self.fit_then = step;
                let size = self.fit_size_index().or(Some(0));
                self.fit_answered(false, Self::yes_no(a), size, false);
            }
            Then::MacFit(step) => {
                self.fit_then = Some(step);
                self.fit_answered(true, Self::yes_no(a), None, false);
            }
            Then::SaveInto => {
                if a == Answer::Ok
                    && let Some(p) = input.filter(|p| !p.is_empty())
                {
                    self.save_into_path(&p);
                }
            }
            Then::ImportExe => {
                if a == Answer::Ok
                    && let Some(p) = input.filter(|p| !p.is_empty())
                {
                    self.import_path(&p);
                }
            }
        }
    }

    fn fit_to(&mut self, (w, h): (i32, i32)) {
        let e = worksheet::extents(&self.doc);
        if let Some(params) = worksheet::fit_params(e, IRect::new(14, 14, w - 14, h - 14)) {
            self.snapshot();
            self.set_dirty();
            worksheet::squeeze(&mut self.doc, params);
        }
    }

    // ---- menu -------------------------------------------------------

    /// The menu's enabled and checked items (`0x414b7e`, `0x414d0d`,
    /// `0x414d47`, `0x414dcc`).
    fn update_menu(&mut self) {
        let mut g = Vec::new();
        if self.running {
            g.extend_from_slice(&menu::RUN_DISABLED);
        } else {
            if !self.can_undo() {
                g.push(cmd::UNDO);
            }
            if !self.dirty {
                g.push(cmd::SAVE);
            }
            if !(self.dirty && (self.path.is_some() || self.puzzle.active)) {
                g.push(cmd::REVERT);
            }
        }
        self.menu.grayed = g;
        self.menu.checked = vec![match self.doc.background {
            701 => cmd::METAL,
            700 => cmd::NOISE,
            702 => cmd::STEEL,
            _ => cmd::PLAIN,
        }];
        let name = self.display_name();
        self.title = format!("Widget Workshop - {name}");
    }

    /// Runs menu command `id`.
    pub fn command(&mut self, id: u16) {
        self.update_menu();
        if !self.menu.enabled(id) {
            return;
        }
        match id {
            cmd::NEW => self.confirm_save(Next::New),
            cmd::OPEN => self.confirm_save(Next::Open),
            cmd::SAVE => self.save(None),
            cmd::SAVE_AS => self.save_as(None),
            cmd::REVERT => {
                // "Revert to original puzzle?" for a puzzle.
                let text = self
                    .assets
                    .string(if self.puzzle.active { 8002 } else { 8001 })
                    .unwrap_or_default();
                self.message(&text, &[Answer::Yes, Answer::No], Then::Revert);
            }
            cmd::EXIT => self.confirm_save(Next::Exit),
            cmd::UNDO => self.undo(),
            cmd::CLEAR => self.clear_work_area(),
            cmd::REMOVE_WIRES => {
                self.snapshot();
                self.sel = Sel::None;
                for w in (0..self.doc.wires.len()).rev() {
                    if !worksheet::wire_locked(&self.doc, w) {
                        self.doc.remove_wire(w);
                    }
                }
            }
            cmd::REMOVE_JOINTS => {
                self.snapshot();
                self.sel = Sel::None;
                for w in &mut self.doc.wires {
                    if w.flag == 0 {
                        w.points.clear();
                    }
                }
            }
            cmd::PLAIN => self.set_background(0),
            cmd::METAL => self.set_background(701),
            cmd::NOISE => self.set_background(700),
            cmd::STEEL => self.set_background(702),
            cmd::FIT => self.fit_work_area(),
            cmd::ABOUT => self.about = Some(about::About::new(self.now)),
            cmd::LOAD_PUZZLE => self.confirm_save(Next::LoadPuzzle),
            cmd::IMPORT => self.confirm_save(Next::Import),
            cmd::SAVE_INTO => self.save_into(),
            cmd::CONTENTS => {
                self.message("Not available in this version yet.", &[Answer::Ok], Then::Nothing);
            }
            // Printing: nothing to print to.
            _ => {}
        }
        self.update_menu();
    }

    /// Edit ▸ Clear Work Area (`0x425cda`); no dirty flag, as the original.
    fn clear_work_area(&mut self) {
        self.snapshot();
        self.sel = Sel::None;
        for g in (0..self.doc.gadgets.len()).rev() {
            if !self.doc.gadgets[g].core.locked {
                self.delete_gadget(g);
            }
        }
        for w in (0..self.doc.wires.len()).rev() {
            if !worksheet::wire_locked(&self.doc, w) {
                self.doc.remove_wire(w);
            }
        }
    }

    /// Options ▸ a background (`0x43d8c8`...).
    fn set_background(&mut self, b: u16) {
        if self.doc.background != b {
            self.snapshot();
            self.doc.background = b;
            self.set_dirty();
        }
    }

    /// Edit ▸ Fit Work Area (`0x4294fe`).
    fn fit_work_area(&mut self) {
        let e = worksheet::extents(&self.doc);
        if e.is_empty() {
            self.message("Nothing to fit", &[Answer::Ok], Then::Nothing);
        } else if e.x0 >= 15 && e.y0 >= 15 && e.x1 <= 625 && e.y1 <= 465 {
            self.message(
                "Your widget is already fit to any Windows screen",
                &[Answer::Ok],
                Then::Nothing,
            );
        } else {
            self.fit_dialog(None);
        }
    }

    // ---- mouse ------------------------------------------------------

    pub fn mouse_move(&mut self, x: i32, y: i32) {
        self.mouse = (x, y);
        let p = Self::to_view((x, y));
        self.env.mouse = p;
        self.env.cursor_screen = (x, y);
        if let Some(d) = &mut self.dialog {
            d.mouse_move(&self.assets, (x, y));
        }
        if let Some((d, _)) = &mut self.file_dialog {
            d.mouse_move((x, y));
        }
        if let Some(d) = &mut self.puzzle_dialog {
            d.mouse_move((x, y));
        }
        let Some(drag) = self.drag.take() else {
            if self.menu.is_open() {
                self.menu.mouse_move(&self.assets, (x, y));
            } else if let Some(w) = self.bar_at(p) {
                self.toolbar_hover(w, p);
            }
            return;
        };
        let drag = self.drag_to(drag, p);
        self.drag = Some(drag);
    }

    /// The hover text over a toolbar (`0x42febc`).
    fn toolbar_hover(&mut self, w: Which, p: (i32, i32)) {
        let bar = self.bar(w);
        let q = (p.0 - bar.pos.0, p.1 - bar.pos.1);
        let sid = match w {
            Which::Tray => self.tray.hover_id(q),
            Which::Main => tray::hover_string(bar.element_at(q).map(|i| bar.elements[i].id), q),
        };
        if let Some(sid) = sid {
            self.tray.set_help(sid, &self.assets);
        }
    }

    fn drag_to(&mut self, drag: Drag, p: (i32, i32)) -> Drag {
        match drag {
            Drag::Gadget { g, last, moved, voice } => {
                let p = self.clip(p);
                let mut d = Drag::Gadget { g, last, moved, voice };
                if p != last {
                    let first = !moved;
                    if first {
                        self.ui_sound(snd::PICK);
                    }
                    let core = &mut self.doc.gadgets[g].core;
                    core.h += p.0 - last.0;
                    core.v += p.1 - last.1;
                    let voice = if first { self.loop_sound(snd::MOVING) } else { voice };
                    d = Drag::Gadget {
                        g,
                        last: p,
                        moved: true,
                        voice,
                    };
                }
                d
            }
            Drag::Joint { w, j } => {
                let p = self.clip(p);
                if let Some(pt) = self.doc.wires[w].points.get_mut(j) {
                    *pt = p;
                }
                Drag::Joint { w, j }
            }
            Drag::NewWire {
                g, k, from_output, hit, ..
            } => {
                let p = self.clip(p);
                let (free, snap, hit) = self.track_wire(p, from_output, hit);
                Drag::NewWire {
                    g,
                    k,
                    from_output,
                    free,
                    snap,
                    hit,
                }
            }
            Drag::WireEnd {
                w, source, orig, hit, ..
            } => {
                let p = self.clip(p);
                let (free, snap, hit) = self.track_wire(p, !source, hit);
                Drag::WireEnd {
                    w,
                    source,
                    orig,
                    free,
                    snap,
                    hit,
                }
            }
            Drag::Pan { last } => {
                let p = if self.running { p } else { self.clip(p) };
                if p != last {
                    let d = (p.0 - last.0, p.1 - last.1);
                    if worksheet::may_offset(&self.doc, self.view(), d) {
                        worksheet::offset_all(&mut self.doc, d);
                    }
                }
                Drag::Pan { last: p }
            }
            Drag::Bar { which, grab, .. } => {
                let p = self.clip(p);
                Drag::Bar {
                    which,
                    grab,
                    at: (p.0 - grab.0, p.1 - grab.1),
                }
            }
            Drag::Place { mut gadget, index, hot } => {
                let p = self.clip(p);
                gadget.core.h = p.0 - hot.0;
                gadget.core.v = p.1 - hot.1;
                Drag::Place { gadget, index, hot }
            }
            Drag::Menu => {
                self.menu.mouse_move(&self.assets, self.mouse);
                Drag::Menu
            }
            d @ (Drag::Press(_) | Drag::Modal) => d,
        }
    }

    /// The free end of a wire being made or moved at `p`: over a gadget it
    /// snaps to its nearest free knot of the wanted kind (inputs when
    /// `input`), sounding 0x83 on each new one (`0x426df7`).
    fn track_wire(
        &mut self,
        p: (i32, i32),
        input: bool,
        hit: Option<(usize, usize)>,
    ) -> ((i32, i32), Option<(usize, usize)>, Option<(usize, usize)>) {
        let Some(h) = worksheet::gadget_at(&self.doc, p) else {
            return (p, None, None);
        };
        let core = &self.doc.gadgets[h].core;
        let Some(k) = worksheet::nearest_free_knot(core, p, input) else {
            return (p, None, hit);
        };
        let at = core.knot_at(k).unwrap_or(p);
        if hit != Some((h, k)) {
            self.ui_sound(snd::WIRE_SNAP);
        }
        (at, Some((h, k)), Some((h, k)))
    }

    pub fn mouse_down(&mut self, x: i32, y: i32, button: Button) {
        self.mouse = (x, y);
        let p = Self::to_view((x, y));
        self.env.mouse = p;
        self.env.cursor_screen = (x, y);
        if self.recording.is_some() {
            // The original's recording blocks the program.
            return;
        }
        if self.about.take().is_some() {
            // Any button closes the About window (`0x415300`).
            return;
        }
        if button != Button::Left {
            self.env.right_down = true;
            return;
        }
        let dbl = self.last_click.is_some_and(|(t, q)| {
            self.now.saturating_sub(t) <= DOUBLE_CLICK_MS
                && (q.0 - x).abs() <= DOUBLE_CLICK_D
                && (q.1 - y).abs() <= DOUBLE_CLICK_D
        });
        self.last_click = if dbl { None } else { Some((self.now, (x, y))) };
        if self.drag.is_some() {
            return;
        }
        if let Some((pr, _)) = &mut self.prompt {
            pr.mouse_down(&self.assets, self.surface.bounds(), (x, y));
            self.drag = Some(Drag::Modal);
            return;
        }
        if self.file_dialog.is_some() {
            self.file_mouse_down((x, y), dbl);
            self.drag = Some(Drag::Modal);
            return;
        }
        if let Some(d) = &mut self.dialog {
            d.mouse_down(&self.assets, (x, y), dbl);
            self.drag = Some(Drag::Modal);
            return;
        }
        if let Some(d) = &mut self.puzzle_dialog {
            let o = d.mouse_down((x, y), dbl);
            self.drag = Some(Drag::Modal);
            self.puzzle_outcome(o);
            return;
        }
        if self.menu.mouse_down(&self.assets, (x, y)) != Outcome::Ignored {
            self.drag = Some(Drag::Menu);
            return;
        }
        if let Some(w) = self.bar_at(p) {
            self.toolbar_down(w, p);
            return;
        }
        if !self.view().contains(p.0, p.1) {
            return;
        }
        self.env.mouse_down = true;
        if self.running {
            self.run_click(p);
        } else {
            match self.tray.tool() {
                Tool::Pointer => self.pointer_down(p, dbl),
                Tool::Cut => self.cut_down(p),
                Tool::Move => self.pan_down(p),
            }
        }
    }

    /// A click while running (`0x423e3f`): the Move tool pans, else the
    /// topmost gadget under it gets it.
    fn run_click(&mut self, p: (i32, i32)) {
        if self.tray.tool() == Tool::Move {
            self.drag = Some(Drag::Pan { last: p });
            return;
        }
        if self.env.exclusive.is_none()
            && let Some(i) = worksheet::gadget_at(&self.doc, p)
        {
            let (gh, gv) = (self.doc.gadgets[i].core.h, self.doc.gadgets[i].core.v);
            self.doc.with_gadget(i, &self.assets, &mut self.env, |g, io| {
                g.behavior.click(&mut g.core, io, p.0 - gh, p.1 - gv);
            });
        }
    }

    /// The Pointer tool's mouse-down (`0x423ec3`).
    fn pointer_down(&mut self, p: (i32, i32), dbl: bool) {
        let joint = worksheet::joint_at(&self.doc, p);
        let wire = worksheet::wire_at(&self.doc, p);
        let mut gadget = worksheet::gadget_at(&self.doc, p);
        let mut knot = None;
        if let Some(g) = gadget {
            let g = worksheet::raise_gadget(&mut self.doc, g);
            gadget = Some(g);
            if let Some((k, d)) = worksheet::nearest_knot(&self.doc.gadgets[g].core, p)
                && d < worksheet::KNOT_D2
            {
                knot = Some(k);
            }
        }
        if let (Some((w, j)), false) = (joint, dbl) {
            if !worksheet::wire_locked(&self.doc, w) {
                self.select(Sel::Joint(w, j));
                self.snapshot();
                self.set_dirty();
                self.drag = Some(Drag::Joint { w, j });
            }
        } else if let (Some(g), Some(k), false) = (gadget, knot, dbl) {
            self.select(Sel::Gadget(g));
            let knot = &self.doc.gadgets[g].core.knots[k];
            let output = !knot.is_input();
            match knot.wire {
                Some(w) => {
                    if !worksheet::wire_locked(&self.doc, w) {
                        self.move_wire_end(g, k, w, output, p);
                    }
                }
                None => self.new_wire(g, k, output, p),
            }
        } else if let (Some((w, _)), false) = (wire, dbl) {
            if !worksheet::wire_locked(&self.doc, w) {
                let w = worksheet::raise_wire(&mut self.doc, w);
                self.snapshot();
                self.set_dirty();
                let pts = worksheet::wire_points(&self.doc, w, None);
                if let Some(seg) = worksheet::joint_insert_index(&pts, p) {
                    self.doc.wires[w].points.insert(seg, p);
                    if let Some(j) = worksheet::nearest_joint(&self.doc.wires[w].points, p) {
                        self.select(Sel::Joint(w, j));
                        self.drag = Some(Drag::Joint { w, j });
                    }
                }
            }
        } else if let (Some(g), None) = (gadget, wire) {
            self.select(Sel::Gadget(g));
            if self.doc.gadgets[g].core.locked {
                return;
            }
            if dbl {
                self.open_settings(g);
            } else {
                self.snapshot();
                self.set_dirty();
                self.drag = Some(Drag::Gadget {
                    g,
                    last: p,
                    moved: false,
                    voice: None,
                });
            }
        }
    }

    /// `0x426df7`.
    fn new_wire(&mut self, g: usize, k: usize, output: bool, p: (i32, i32)) {
        self.ui_sound(snd::WIRE);
        self.sel = Sel::None;
        self.snapshot();
        self.set_dirty();
        self.drag = Some(Drag::NewWire {
            g,
            k,
            from_output: output,
            free: p,
            snap: None,
            hit: None,
        });
    }

    /// `0x4272f7`: the end comes off its knot while it's held.
    fn move_wire_end(&mut self, g: usize, k: usize, w: usize, output: bool, p: (i32, i32)) {
        self.ui_sound(snd::WIRE_PICK);
        self.sel = Sel::None;
        self.set_dirty();
        self.snapshot();
        self.doc.gadgets[g].core.knots[k].wire = None;
        self.drag = Some(Drag::WireEnd {
            w,
            source: output,
            orig: (g, k),
            free: p,
            snap: None,
            hit: None,
        });
    }

    /// The Cut tool (`0x42566c`): a joint, else a wire, else a gadget.
    fn cut_down(&mut self, p: (i32, i32)) {
        if let Some((w, j)) = worksheet::joint_at(&self.doc, p) {
            if !worksheet::wire_locked(&self.doc, w) {
                self.ui_sound(snd::DEL_JOINT);
                self.snapshot();
                self.set_dirty();
                self.sel = Sel::None;
                self.doc.wires[w].points.remove(j);
            }
        } else if let Some((w, _)) = worksheet::wire_at(&self.doc, p) {
            if !worksheet::wire_locked(&self.doc, w) {
                self.ui_sound(snd::DEL_WIRE);
                self.snapshot();
                self.set_dirty();
                self.sel = Sel::None;
                self.doc.remove_wire(w);
            }
        } else if let Some(g) = worksheet::gadget_at(&self.doc, p)
            && !self.doc.gadgets[g].core.locked
        {
            self.ui_sound(snd::DEL_GADGET);
            self.snapshot();
            self.set_dirty();
            self.delete_gadget(g);
        }
    }

    /// The Move tool (`0x426313`): an undo step even if nothing moves.
    fn pan_down(&mut self, p: (i32, i32)) {
        self.snapshot();
        self.set_dirty();
        self.drag = Some(Drag::Pan { last: p });
    }

    /// A toolbar's mouse-down (`0x42fccc`).
    fn toolbar_down(&mut self, w: Which, p: (i32, i32)) {
        self.raise_bar(w);
        if w == Which::Tray && self.tray.sliding(self.now) {
            return;
        }
        let bar = self.bar(w);
        let q = (p.0 - bar.pos.0, p.1 - bar.pos.1);
        if q.1 <= MOVE_BAR {
            self.drag = Some(Drag::Bar {
                which: w,
                grab: q,
                at: bar.pos,
            });
            return;
        }
        if w == Which::Tray
            && let Some(slot) = self.tray.slot_at(q)
        {
            self.pick_up(slot.index, p);
            return;
        }
        let fired = self.bar_mut(w).press(q);
        if self.bar(w).capture.is_some() {
            self.drag = Some(Drag::Press(w));
        }
        for (id, on) in fired {
            self.toolbar_command(id, on);
        }
    }

    /// The commands of both toolbars (`0x430d2c`, `0x4324c8`).
    fn toolbar_command(&mut self, cid: i32, on: bool) {
        match cid {
            id::GO => {
                self.ui_sound(snd::CLICK);
                self.run();
            }
            id::STOP => {
                self.ui_sound(snd::CLICK);
                self.stop();
            }
            id::LIGHT => {
                self.ui_sound(snd::CLICK);
                self.ui_sound(if on { snd::LIGHT_ON } else { snd::LIGHT_OFF });
                self.ui_sound(snd::CLICK);
                self.light = on;
            }
            id::SOUND => {
                self.ui_sound(snd::CLICK);
                if on {
                    self.sound = true;
                    self.ui_sound(snd::SOUND_ON);
                } else {
                    self.ui_sound(snd::SOUND_OFF);
                    self.env.sounds.push(SoundCmd::StopAll);
                    self.sound = false;
                }
            }
            id::POINTER | id::CUT | id::MOVE => self.ui_sound(snd::CLICK),
            t if (id::TAB0..id::TAB0 + 7).contains(&t) => {
                self.ui_sound(snd::CLICK);
                self.ui_sound(snd::SLIDE);
                let f = usize::try_from(t - id::TAB0).unwrap_or(0);
                self.tray.switch_folder(f, self.now);
            }
            id::PREV | id::NEXT => {
                self.ui_sound(snd::ARROW);
                self.ui_sound(snd::SLIDE);
                self.tray.turn_page(cid == id::NEXT, self.now);
            }
            _ => {}
        }
    }

    /// A part taken from the tray (`0x42e880`, `0x439237`).
    fn pick_up(&mut self, index: u8, p: (i32, i32)) {
        if self.tray.counts[usize::from(index)] == 0 {
            return;
        }
        self.tray.select_tool(Tool::Pointer);
        self.sel = Sel::None;
        self.snapshot();
        self.tray.take(index);
        let Some(tag) = tray::tag_of(index) else { return };
        self.ui_sound(snd::TAKEN);
        let id = self.doc.new_id();
        let mut gadget = doc::new_gadget(&self.assets, tag, id, -2000, -2000);
        gadget.behavior.layout(&mut gadget.core, &self.assets);
        let b = gadget.core.bounds;
        let mut hot = (i32::from(b.width()) / 2, i32::from(b.height()) / 2);
        if tag == LABEL {
            // A new Label lands 17 pixels further across (io.md 16).
            hot.0 -= 17;
        }
        gadget.core.h = p.0 - hot.0;
        gadget.core.v = p.1 - hot.1;
        self.drag = Some(Drag::Place {
            gadget: Box::new(gadget),
            index,
            hot,
        });
    }

    pub fn mouse_up(&mut self, x: i32, y: i32, button: Button) {
        self.mouse = (x, y);
        let p = Self::to_view((x, y));
        self.env.mouse = p;
        self.env.cursor_screen = (x, y);
        if button != Button::Left {
            self.env.right_down = false;
            return;
        }
        self.env.mouse_down = false;
        let Some(drag) = self.drag.take() else { return };
        let drag = self.drag_to(drag, p);
        match drag {
            Drag::Gadget { moved, voice, .. } => {
                if let Some(v) = voice {
                    self.env.sounds.push(SoundCmd::Stop { voice: v });
                }
                if moved {
                    self.ui_sound(snd::DROP);
                }
            }
            Drag::Joint { .. } | Drag::Pan { .. } => {}
            Drag::NewWire {
                g,
                k,
                from_output,
                snap,
                ..
            } => {
                if let Some((h, k2)) = snap {
                    self.ui_sound(snd::WIRE);
                    if from_output {
                        self.doc.connect(g, k, h, k2, Vec::new(), 0);
                    } else {
                        self.doc.connect(h, k2, g, k, Vec::new(), 0);
                    }
                }
            }
            Drag::WireEnd {
                w, source, orig, snap, ..
            } => {
                let (g, k) = match snap {
                    Some(t) => {
                        self.ui_sound(snd::WIRE);
                        t
                    }
                    None => orig,
                };
                let wire = &mut self.doc.wires[w];
                if source {
                    wire.from = g;
                    wire.from_knot = k;
                } else {
                    wire.to = g;
                    wire.to_knot = k;
                }
                self.doc.gadgets[g].core.knots[k].wire = Some(w);
            }
            Drag::Bar { which, at, .. } => {
                self.bar_mut(which).pos = at;
            }
            Drag::Press(w) => {
                let bar = self.bar(w);
                let q = (p.0 - bar.pos.0, p.1 - bar.pos.1);
                if let Some(id) = self.bar_mut(w).release(q) {
                    self.toolbar_command(id, false);
                }
            }
            Drag::Place { gadget, index, hot } => {
                let p = self.clip(p);
                if self.tray.bar.rect().contains(p.0, p.1) {
                    self.tray.give_back(index);
                } else {
                    self.ui_sound(snd::PLACED);
                    self.set_dirty();
                    let mut g = *gadget;
                    g.core.h = p.0 - hot.0;
                    g.core.v = p.1 - hot.1;
                    self.doc.gadgets.push(g);
                }
            }
            Drag::Menu => {
                if let Outcome::Command(c) = self.menu.mouse_up(&self.assets, (x, y)) {
                    self.command(c);
                }
            }
            Drag::Modal => self.modal_up(x, y),
        }
    }

    /// The mouse went up after going down on a box or dialog.
    fn modal_up(&mut self, x: i32, y: i32) {
        if let Some((mut pr, then)) = self.prompt.take() {
            match pr.mouse_up(&self.assets, self.surface.bounds(), (x, y)) {
                Some(a) => self.answer(a, then, None),
                None => self.prompt = Some((pr, then)),
            }
        } else if self.file_dialog.is_some() {
            self.file_mouse_up((x, y));
        } else if let Some(mut d) = self.dialog.take() {
            let r = d.mouse_up(&self.assets, (x, y));
            self.dialog_result(d, r);
        } else if let Some(d) = &mut self.puzzle_dialog {
            let o = d.mouse_up((x, y));
            self.puzzle_outcome(o);
        }
    }

    /// The wheel scrolls the puzzle list; the worksheet doesn't scroll.
    pub fn wheel(&mut self, _dx: i32, dy: i32) {
        if self.prompt.is_some() {
            return;
        }
        if let Some((d, _)) = &mut self.file_dialog {
            d.wheel(dy);
        } else if let Some(d) = &mut self.puzzle_dialog {
            d.wheel(dy);
        }
    }

    // ---- settings dialogs ---------------------------------------------

    /// Double-click on a gadget with a dialog (`0x41c43b`).
    fn open_settings(&mut self, g: usize) {
        if let Some(d) = Dialog::settings(&self.assets, &self.pictures, &self.doc.gadgets[g], g) {
            self.ui_sound(snd::DIALOG_OPEN);
            self.dialog = Some(d);
        }
    }

    /// A dialog's state after an input.
    fn dialog_result(&mut self, mut d: Dialog, r: dialog::Result) {
        match r {
            dialog::Result::Open | dialog::Result::Record | dialog::Result::Import => {
                if let Some(g) = d.gadget {
                    d.preview(&mut self.doc.gadgets[g], &self.assets);
                }
                let g = d.gadget;
                self.dialog = Some(d);
                match (r, g) {
                    (dialog::Result::Record, Some(g)) => self.start_recording(g),
                    (dialog::Result::Import, Some(g)) => self.ask_wav(g),
                    _ => {}
                }
            }
            dialog::Result::Ok => {
                if let Some(g) = d.gadget {
                    self.snapshot();
                    let gadget = &mut self.doc.gadgets[g];
                    d.apply(gadget, &self.assets);
                    gadget.behavior.layout(&mut gadget.core, &self.assets);
                    if gadget.core.tag == LABEL {
                        // The Label's read-back lays it out again and resets.
                        gadget.behavior.reset(&mut gadget.core);
                    }
                    self.ui_sound(snd::DIALOG_CLOSE);
                } else if d.is_fit() {
                    self.fit_answered(d.is_mac_fit(), Some(d.said_yes()), d.fit_choice(), d.remember());
                }
            }
            dialog::Result::Cancel => {
                if let Some(g) = d.gadget {
                    d.cancel(&mut self.doc.gadgets[g]);
                    self.ui_sound(snd::DIALOG_CLOSE);
                } else if d.is_fit() {
                    self.fit_answered(d.is_mac_fit(), None, None, d.remember());
                }
            }
        }
    }

    // ---- keys -------------------------------------------------------

    pub fn key_down(&mut self, vk: u16) {
        if !self.env.keys_held.contains(&vk) {
            self.env.keys_held.push(vk);
        }
        match vk {
            VK_SHIFT => self.shift = true,
            VK_CONTROL => self.ctrl = true,
            VK_MENU => {
                self.alt = true;
                self.alt_alone = true;
                return;
            }
            _ => self.alt_alone = false,
        }
        if self.recording.is_some() {
            return;
        }
        if self.about.is_some() {
            if matches!(vk, 0x20 | VK_ESCAPE) {
                self.about = None;
            }
            return;
        }
        if let Some((pr, then)) = &mut self.prompt {
            if let Some(a) = pr.key(vk) {
                let then = *then;
                self.prompt = None;
                self.answer(a, then, None);
            }
            return;
        }
        if self.file_dialog.is_some() {
            self.file_key(vk);
            return;
        }
        if let Some(mut d) = self.dialog.take() {
            let r = d.key(vk, self.shift);
            self.dialog_result(d, r);
            return;
        }
        if let Some(d) = &mut self.puzzle_dialog {
            let o = d.key(vk);
            self.puzzle_outcome(o);
            return;
        }
        if self.menu.is_open() {
            if let Outcome::Command(c) = self.menu.key(vk) {
                self.command(c);
            }
            return;
        }
        if vk == VK_F10 && !self.shift && !self.ctrl {
            self.menu.activate();
            return;
        }
        if self.alt && (0x41..=0x5a).contains(&vk) && self.menu.open_mnemonic(char::from(u8::try_from(vk).unwrap_or(0)))
        {
            return;
        }
        if let Some(c) = self.menu.accel(vk, self.shift, self.ctrl, self.alt) {
            self.command(c);
            return;
        }
        if self.running {
            self.env.keys.push(vk);
        }
        // `0x42385d`: the worksheet's keys, the main toolbar's, the tray's.
        if !self.running && self.edit_key(vk) {
            return;
        }
        match vk {
            VK_RETURN if !self.running => {
                self.run();
                return;
            }
            VK_ESCAPE if self.running => {
                self.stop();
                return;
            }
            _ => {}
        }
        self.tray_key(vk);
    }

    /// `0x423c2d`: Delete, L, numpad ± on the selection.
    fn edit_key(&mut self, vk: u16) -> bool {
        match vk {
            VK_DELETE => {
                if self.sel == Sel::None || self.drag.is_some() {
                    return true;
                }
                self.snapshot();
                self.set_dirty();
                match self.sel {
                    Sel::Wire(w, _) => {
                        self.ui_sound(snd::DEL_WIRE);
                        self.sel = Sel::None;
                        self.doc.remove_wire(w);
                    }
                    Sel::Gadget(g) => {
                        self.ui_sound(snd::DEL_GADGET);
                        self.delete_gadget(g);
                    }
                    Sel::Joint(w, j) => {
                        self.ui_sound(snd::DEL_JOINT);
                        self.sel = Sel::None;
                        self.doc.wires[w].points.remove(j);
                    }
                    Sel::None => {}
                }
                true
            }
            0x4c | VK_SEPARATOR => {
                if self.sel == Sel::None || self.drag.is_some() {
                    return true;
                }
                self.snapshot();
                self.set_dirty();
                match self.sel {
                    Sel::Gadget(g) => {
                        let c = &mut self.doc.gadgets[g].core;
                        c.locked = !c.locked;
                    }
                    Sel::Wire(w, _) | Sel::Joint(w, _) => {
                        let f = &mut self.doc.wires[w].flag;
                        *f = u8::from(*f == 0);
                    }
                    Sel::None => {}
                }
                true
            }
            VK_ADD | VK_SUBTRACT => {
                let Sel::Gadget(g) = self.sel else { return false };
                if let Some(i) = tray::index_of(self.doc.gadgets[g].core.tag) {
                    self.snapshot();
                    self.set_dirty();
                    self.tray.adjust(i, vk == VK_ADD);
                }
                true
            }
            _ => false,
        }
    }

    /// `0x4326e0`: Shift (editing) for the Cut tool, Ctrl for Move while
    /// held; numpad ± over a part in the tray.
    fn tray_key(&mut self, vk: u16) {
        match vk {
            VK_SHIFT | VK_CONTROL => {
                if (vk == VK_SHIFT && self.running) || self.tray.saved_tool.is_some() || self.drag.is_some() {
                    return;
                }
                self.tray.saved_tool = Some(self.tray.tool());
                self.tray
                    .select_tool(if vk == VK_SHIFT { Tool::Cut } else { Tool::Move });
            }
            VK_ADD | VK_SUBTRACT if !self.running => {
                let p = Self::to_view(self.mouse);
                if self.bar_at(p) != Some(Which::Tray) {
                    return;
                }
                let q = (p.0 - self.tray.bar.pos.0, p.1 - self.tray.bar.pos.1);
                if let Some(s) = self.tray.slot_at(q) {
                    self.snapshot();
                    self.set_dirty();
                    self.tray.adjust(s.index, vk == VK_ADD);
                }
            }
            _ => {}
        }
    }

    pub fn key_up(&mut self, vk: u16) {
        self.env.keys_held.retain(|&k| k != vk);
        match vk {
            VK_SHIFT => self.shift = false,
            VK_CONTROL => self.ctrl = false,
            VK_MENU => {
                self.alt = false;
                if self.alt_alone
                    && self.prompt.is_none()
                    && self.dialog.is_none()
                    && self.file_dialog.is_none()
                    && self.puzzle_dialog.is_none()
                    && self.drag.is_none()
                {
                    self.menu.activate();
                }
                self.alt_alone = false;
                return;
            }
            _ => {}
        }
        // `0x432824`.
        if matches!(vk, VK_SHIFT | VK_CONTROL)
            && let Some(t) = self.tray.saved_tool.take()
        {
            self.tray.select_tool(t);
        }
    }

    // ---- pointer ----------------------------------------------------

    /// The pointer to show (`0x434550`, `0x425909`).
    #[must_use]
    pub fn pointer(&self) -> Pointer {
        if let Some(d) = &self.drag {
            return match d {
                Drag::Gadget { .. } | Drag::Joint { .. } => Pointer::Game(3),
                Drag::NewWire { .. } | Drag::WireEnd { .. } => Pointer::Game(2),
                Drag::Pan { .. } => Pointer::Game(4),
                Drag::Place { .. } => Pointer::Game(6),
                Drag::Bar { .. } | Drag::Press(_) | Drag::Menu | Drag::Modal => Pointer::Arrow,
            };
        }
        if !self.over_view(self.mouse) {
            return Pointer::Arrow;
        }
        let p = Self::to_view(self.mouse);
        if self.running {
            return Pointer::Game(if self.tray.tool() == Tool::Move { 4 } else { 0 });
        }
        Pointer::Game(match self.tray.tool() {
            Tool::Cut => 1,
            Tool::Move => 4,
            Tool::Pointer => self.cursor_for(p),
        })
    }

    fn cursor_for(&self, p: (i32, i32)) -> u8 {
        let d = &self.doc;
        let lock = |w: usize| worksheet::wire_locked(d, w);
        if let Some((w, _)) = worksheet::joint_at(d, p) {
            return if lock(w) { 0 } else { 6 };
        }
        let g = worksheet::gadget_at(d, p);
        if let Some(g) = g
            && let Some((k, dist)) = worksheet::nearest_knot(&d.gadgets[g].core, p)
            && dist < worksheet::KNOT_D2
        {
            return match d.gadgets[g].core.knots[k].wire {
                Some(w) if lock(w) => 0,
                _ => 2,
            };
        }
        if let Some((w, _)) = worksheet::wire_at(d, p) {
            return if lock(w) { 0 } else { 5 };
        }
        match g {
            Some(g) if d.gadgets[g].core.locked => 0,
            Some(_) => 6,
            None => 0,
        }
    }

    /// Cursor `i`'s picture (`WIDGET.EXE` group `5007 + i`).
    #[must_use]
    pub fn cursor_picture(&self, i: u8) -> Option<&CursorPic> {
        self.cursors.get(usize::from(i))?.as_ref()
    }

    // ---- drawing ----------------------------------------------------

    /// Draws everything.
    pub fn draw(&mut self) {
        let frame = self.surface.bounds();
        let view = self.worksheet();
        self.surface.set_clip(view);
        self.surface.origin = (view.x0, view.y0);
        self.draw_worksheet();
        let outline = match &self.drag {
            Some(Drag::Bar { which, at, .. }) => {
                let size = self.bar(*which).size;
                Some(IRect::sized(at.0, at.1, size.0, size.1))
            }
            _ => None,
        };
        let s = &mut self.surface;
        // While a part is placed the toolbars hide, but their pictures stay
        // under it (`0x439237`): drawn as usual.
        for w in self.bars {
            match w {
                Which::Main if self.main_bar.visible => self.main_bar.draw(s, &self.assets, &self.pictures),
                Which::Tray if self.tray.bar.visible => self.tray.draw(s, &self.assets, &self.pictures, self.now),
                _ => {}
            }
        }
        match &self.drag {
            Some(Drag::Place { gadget, .. }) => {
                let mut cx = DrawCx {
                    surface: s,
                    assets: &self.assets,
                    pictures: &self.pictures,
                    running: false,
                    ticks: 0,
                    lights: self.light,
                };
                gadget.behavior.draw(&gadget.core, &mut cx);
            }
            Some(Drag::Bar { .. }) => {
                // The outline (`0x430172`): four 2-pixel inverted strips.
                let r = outline.unwrap_or_default();
                s.invert(IRect::new(r.x0, r.y0, r.x1, r.y0 + 2));
                s.invert(IRect::new(r.x0, r.y1 - 2, r.x1, r.y1));
                s.invert(IRect::new(r.x0, r.y0 + 2, r.x0 + 2, r.y1 - 2));
                s.invert(IRect::new(r.x1 - 2, r.y0 + 2, r.x1, r.y1 - 2));
            }
            _ => {}
        }
        s.origin = (0, 0);
        s.set_clip(frame);
        self.menu.draw(s, &self.assets, frame.width());
        if let Some(d) = &self.dialog {
            let g = d.gadget.and_then(|g| self.doc.gadgets.get(g));
            d.draw(s, &self.assets, &self.pictures, g, self.light);
        }
        if let Some(d) = &self.puzzle_dialog {
            d.draw(s, &self.assets);
        }
        if let Some(a) = &self.about {
            a.draw(s, &self.assets, &self.pictures, view, self.now);
        }
        if let Some((d, _)) = &self.file_dialog {
            let fs = crate::files::Fs {
                files: self.files.as_ref(),
                samples: &self.assets.samples,
            };
            d.draw(s, &self.assets, fs);
        }
        if let Some((p, _)) = &self.prompt {
            p.draw(s, &self.assets, frame);
        }
    }

    /// The worksheet (`0x42397d`, `0x423b59`): background, gadgets, wires,
    /// the frame; and what's being dragged on top.
    fn draw_worksheet(&mut self) {
        let view = self.view();
        let running = self.running;
        let loose = match &self.drag {
            Some(Drag::WireEnd {
                w, source, free, snap, ..
            }) => Some(Loose {
                wire: *w,
                source: *source,
                at: *free,
                snapped: snap.is_some(),
            }),
            _ => None,
        };
        let dragged = match &self.drag {
            Some(Drag::Gadget { g, .. }) => Some(*g),
            _ => None,
        };
        self.draw_background(view);
        let s = &mut self.surface;
        let mut cx = DrawCx {
            surface: s,
            assets: &self.assets,
            pictures: &self.pictures,
            running,
            ticks: self.env.ticks,
            lights: self.light,
        };
        for (i, g) in self.doc.gadgets.iter().enumerate() {
            if Some(i) == dragged {
                continue;
            }
            g.behavior.draw(&g.core, &mut cx);
            if !running && self.sel == Sel::Gadget(i) {
                cx.surface.frame(g.core.rect(), worksheet::color::GREEN);
            }
        }
        let doc = &self.doc;
        for w in 0..doc.wires.len() {
            let pts = worksheet::wire_points(doc, w, loose);
            let green = loose.is_none_or(|l| l.wire != w || l.snapped);
            worksheet::draw_polyline(s, &pts, green, running, floating(doc, dragged, w));
        }
        if !running {
            match self.sel {
                Sel::Wire(w, seg) if w < doc.wires.len() => {
                    let pts = worksheet::wire_points(doc, w, loose);
                    worksheet::draw_selected_segment(s, &pts, seg, worksheet::wire_locked(doc, w));
                }
                Sel::Joint(w, j) => {
                    if let Some(&p) = doc.wires.get(w).and_then(|x| x.points.get(j)) {
                        worksheet::draw_joint_frame(s, p);
                    }
                }
                _ => {}
            }
        }
        if let Some(Drag::NewWire {
            g,
            k,
            from_output,
            free,
            snap,
            ..
        }) = &self.drag
        {
            let at = doc.gadgets[*g].core.knot_at(*k).unwrap_or(*free);
            let pts = if *from_output { [at, *free] } else { [*free, at] };
            worksheet::draw_polyline(s, &pts, snap.is_some(), false, None);
        }
        self.draw_frame(view);
        if let Some(g) = dragged {
            let s = &mut self.surface;
            let gadget = &self.doc.gadgets[g];
            let mut cx = DrawCx {
                surface: s,
                assets: &self.assets,
                pictures: &self.pictures,
                running,
                ticks: self.env.ticks,
                lights: self.light,
            };
            gadget.behavior.draw(&gadget.core, &mut cx);
            s.frame(gadget.core.rect(), worksheet::color::GREEN);
            for w in 0..self.doc.wires.len() {
                if let Some(seg) = floating(&self.doc, dragged, w) {
                    let pts = worksheet::wire_points(&self.doc, w, None);
                    worksheet::draw_polyline(s, &pts[seg..seg + 2], true, false, None);
                }
            }
        }
    }

    /// The background (`0x43d209`): plain white (black with the light
    /// off) or a picture tiled from the top left.
    fn draw_background(&mut self, view: IRect) {
        let s = &mut self.surface;
        let b = u32::from(self.doc.background);
        let frame = if self.light { 1 } else { 2 };
        match self.pictures.frame(&self.assets, Module::Gadget, b, frame) {
            Some(tile) if b != 0 && tile.width > 0 && tile.height > 0 => {
                let (tw, th) = (i32::from(tile.width), i32::from(tile.height));
                let mut y = view.y0;
                while y < view.y1 {
                    let mut x = view.x0;
                    while x < view.x1 {
                        s.blit(&tile, x, y);
                        x += tw;
                    }
                    y += th;
                }
            }
            _ => s.fill(view, if self.light { 255 } else { 0 }),
        }
    }

    /// The pipe frame of the textured backgrounds (`0x43d39b`, sheet 704).
    fn draw_frame(&mut self, v: IRect) {
        if self.doc.background == 0 {
            return;
        }
        let s = &mut self.surface;
        let pic = |n: u16| self.pictures.frame(&self.assets, Module::Gadget, 704, n);
        let size = |n: u16| pic(n).map_or((0, 0), |i| (i32::from(i.width), i32::from(i.height)));
        let (w2, _) = size(2);
        if let Some(p) = pic(2).filter(|_| w2 > 0) {
            let mut x = v.x0;
            while x < v.x1 {
                s.blit(&p, x, v.y0);
                x += w2;
            }
        }
        let (w6, h6) = size(6);
        if let Some(p) = pic(6).filter(|_| w6 > 0) {
            let mut x = v.x0;
            while x < v.x1 {
                s.blit(&p, x, v.y1 - h6);
                x += w6;
            }
        }
        let (_, h8) = size(8);
        if let Some(p) = pic(8).filter(|_| h8 > 0) {
            let mut y = v.y0;
            while y < v.y1 {
                s.blit(&p, v.x0, y);
                y += h8;
            }
        }
        let (w4, h4) = size(4);
        if let Some(p) = pic(4).filter(|_| h4 > 0) {
            let mut y = v.y0;
            while y < v.y1 {
                s.blit(&p, v.x1 - w4, y);
                y += h4;
            }
        }
        let corner = |s: &mut Surface, n: u16, x: i32, y: i32| {
            if let Some(p) = pic(n) {
                s.blit(&p, x, y);
            }
        };
        let (w3, _) = size(3);
        let (_, h7) = size(7);
        let (w5, h5) = size(5);
        corner(s, 1, v.x0, v.y0);
        corner(s, 3, v.x1 - w3, v.y0);
        corner(s, 7, v.x0, v.y1 - h7);
        corner(s, 5, v.x1 - w5, v.y1 - h5);
    }
}

/// The segment of wire `w` touching the dragged gadget, which floats
/// with it (`0x43a976`).
fn floating(doc: &Document, dragged: Option<usize>, w: usize) -> Option<usize> {
    let g = dragged?;
    let wire = &doc.wires[w];
    if wire.from == g {
        Some(0)
    } else if wire.to == g {
        Some(wire.points.len())
    } else {
        None
    }
}

#[cfg(test)]
mod file_tests;
#[cfg(test)]
mod tests;
