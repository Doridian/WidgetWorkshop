//! The parts tray (`CWidgetTbar`, docs/editor-layout.md 3.4–3.7): the
//! Pointer/Cut/Move tools, the seven folder tabs, the page arrows and
//! number, the help line and the slot area with the parts and their
//! counts (`CWTrayToolbar`, `CWTrayFolder`, `CWTraySlot`).
//!
//! With the port's enhancements on ([`Tray::set_enhanced`]) the slot area
//! shows the whole folder, its pages one under the other, and the page
//! arrows and number are gone. The tools keep their height, so the tray
//! hangs down below them; its frame is put together from the original's
//! face. A folder switch rolls the area up into the gray and down into
//! the new folder, as the original's slides do.

use formats::hr::Image;

use crate::assets::{Assets, Module};
use crate::gadget::Tag;
use crate::gfx::{IRect, Pictures, Surface};

use super::toolbar::{Bar, Element, Kind, MOVE_BAR, id};

/// The tray index → tag table (`0x49552c`, `0x435c60`): part `i` is
/// `TAGS[i - 1]`.
pub const TAGS: [&[u8; 4]; 64] = [
    b"Pend", b"Drop", b"Cano", b"Wtch", b"Ttlr", b"Coin", b"rand", b"Fork", b"DClk", b"Key_", b"Metr", b"swit",
    b"disp", b"di10", b"Sldr", b"bulb", b"hart", b"rgbd", b"Spl2", b"Spl3", b"Spl4", b"Addr", b"Subr", b"Mulr",
    b"Divr", b"cnst", b"Plnt", b"1ncr", b"ArVo", b"Conv", b"NmKP", b"Comp", b"AndG", b"OrGa", b"XorG", b"NotG",
    b"Mplx", b"Tape", b"Spkr", b"MsMv", b"MsCl", b"Cntr", b"AnSd", b"MsSd", b"SdTr", b"SdSw", b"TxLn", b"TxBx",
    b"DnPl", b"StCh", b"GrPr", b"Stop", b"Stk1", b"Stk2", b"Stk3", b"Stk4", b"Stk5", b"Stk6", b"Stk7", b"Stk8",
    b"Stk9", b"St10", b"MiCo", b"TxLb",
];

/// The folders' contents (`0x49428c`): tray indexes, −2 a page break.
pub const FOLDERS: [&[i8]; 7] = [
    &[16, 18, 50, 51, -2, 13, 14],
    &[9, 4, 11],
    &[46, 43, 44, 39, -2, 8, 38, 45],
    &[
        31, 26, 42, 63, 15, -2, 22, 23, 24, 25, -2, 6, 7, 5, 32, -2, 28, 29, 30, -2, 33, 34, 35, 36,
    ],
    &[12, 10, 41, 40, 49, -2, 37, 19, 20, 21],
    &[64, 47, 48, -2, 53, 54, 55, 56, -2, 57, 58, 59, -2, 60, 61, 62],
    &[52, 1, 2, 3, -2, 17, 27],
];

/// The slot area in the tray (`+0xb5c`): 350×40 at (67, 51).
pub const AREA: IRect = IRect::new(67, 51, 417, 91);
const AREA_W: i32 = 350;
const AREA_H: i32 = 40;
/// The help line (`+0xb94`, `CWStaticText`).
pub const HELP: IRect = IRect::new(4, 3, 445, 15);
/// The page number (`+0xa6c`, `CWScrollNumber`) and its digits "1".."6"
/// in the sheet (`0x49438c`).
const NUMBER: (i32, i32) = (427, 48);
const DIGITS: [IRect; 6] = [
    IRect::new(511, 0, 523, 10),
    IRect::new(511, 11, 523, 22),
    IRect::new(511, 22, 523, 33),
    IRect::new(511, 33, 523, 44),
    IRect::new(511, 44, 523, 55),
    IRect::new(511, 55, 523, 66),
];
/// One slide of the slot area, in milliseconds (`0x42f592`).
pub const SLIDE_MS: u64 = 180;
/// Sheet of the gray stipple (frame 1) and the count digits (2..11).
const COUNT_SHEET: u32 = 801;
/// The enhanced tray: the face's columns up to the slot area's right edge
/// and the last three (the window's own edge), 423 wide; the tools'
/// height, and where the part under the slot area starts.
const KEEP_W: i32 = 420;
const EDGE: IRect = IRect::new(446, 0, 449, 93);
const ENHANCED_W: i32 = KEEP_W + 3;
const TOOLS_H: i32 = 93;
const LOWER_X: i32 = 61;
/// A row of the face the frame's sides are repeated from.
const SIDE_ROW: i32 = 60;

/// The tag of tray index `i` (`0x435c60`).
#[must_use]
pub fn tag_of(i: u8) -> Option<Tag> {
    TAGS.get(usize::from(i).checked_sub(1)?).map(|t| Tag(**t))
}

/// The tray index of `tag` (`0x435c93`).
#[must_use]
pub fn index_of(tag: Tag) -> Option<u8> {
    TAGS.iter()
        .position(|t| **t == tag.0)
        .and_then(|i| u8::try_from(i + 1).ok())
}

/// Tray index `i`'s picture: sheet 898 frame `i` for 1..30, else sheet 899
/// frame `i − 30` (`0x432229`).
#[must_use]
pub fn picture(i: u8) -> (u32, u16) {
    if i <= 30 {
        (898, u16::from(i))
    } else {
        (899, u16::from(i) - 30)
    }
}

/// The tools (the radio group at `+0x620`; `0x432683`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Pointer,
    Cut,
    Move,
}

impl Tool {
    fn id(self) -> i32 {
        match self {
            Tool::Pointer => id::POINTER,
            Tool::Cut => id::CUT,
            Tool::Move => id::MOVE,
        }
    }
}

/// A part on a page (`CWTraySlotInfo`): its tray index and rectangle in
/// the slot area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub index: u8,
    pub rect: IRect,
}

/// Splits a folder's list into pages and lays them out (`0x42ecea`,
/// `0x42e0ff`): a part `w` wide takes `w + 3` of the 350 pixels and starts
/// a new page where it doesn't fit; the room left is shared out as equal
/// gaps (C division), each part centred vertically.
#[must_use]
pub fn layout(list: &[i8], size: impl Fn(u8) -> (i32, i32)) -> Vec<Vec<Slot>> {
    let mut pages: Vec<Vec<(u8, (i32, i32))>> = vec![Vec::new()];
    let mut free = AREA_W;
    for &v in list {
        let Ok(i) = u8::try_from(v) else {
            pages.push(Vec::new());
            free = AREA_W;
            continue;
        };
        let (w, h) = size(i);
        if w + 3 > free {
            pages.push(Vec::new());
            free = AREA_W;
        }
        free -= w + 3;
        if let Some(p) = pages.last_mut() {
            p.push((i, (w, h)));
        }
    }
    pages
        .into_iter()
        .map(|parts| {
            let n = i32::try_from(parts.len()).unwrap_or(0);
            let rem = AREA_W - parts.iter().map(|(_, (w, _))| w + 3).sum::<i32>();
            let gap = (3 * n + rem) / (n + 1);
            let mut x = gap;
            parts
                .into_iter()
                .map(|(index, (w, h))| {
                    let y = (AREA_H - h) / 2;
                    let rect = IRect::sized(x, y, w, h);
                    x += w + gap;
                    Slot { index, rect }
                })
                .collect()
        })
        .collect()
}

/// A folder's pages one under the other, as the enhanced tray shows
/// them.
fn folder_slots(pages: &[Vec<Slot>]) -> impl Iterator<Item = Slot> + '_ {
    (0..).zip(pages).flat_map(|(row, page)| {
        page.iter().map(move |s| Slot {
            index: s.index,
            rect: s.rect.offset(0, row * AREA_H),
        })
    })
}

/// What a page slide shows: a folder's page or the blank gray one
/// (`CWGrayTraySlot`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shown {
    Page(usize, usize),
    Gray,
}

/// The tray.
#[derive(Debug, Clone)]
pub struct Tray {
    pub bar: Bar,
    /// Each folder's pages.
    pub folders: Vec<Vec<Vec<Slot>>>,
    /// Each folder's current page.
    pub pages: Vec<usize>,
    pub folder: usize,
    /// Each tray index's count (−1 unlimited); index 0 unused.
    pub counts: [i16; 65],
    /// The help line's current string id (`+0xc0c`) and text.
    pub help_id: i32,
    pub help: String,
    /// The tool to go back to when Shift or Ctrl is released (`+0xc24`).
    pub saved_tool: Option<Tool>,
    /// The slides under way: from, to, upwards; and when they started.
    slides: Vec<(Shown, Shown, bool)>,
    slide_start: u64,
    /// The port's whole-folder tray.
    pub enhanced: bool,
}

/// The tray's buttons (`0x431281`): the tools, the tabs and, unless
/// `enhanced`, the page arrows.
fn elements(enhanced: bool) -> Vec<Element> {
    let mut elements = vec![
        Element::new(
            id::POINTER,
            IRect::new(5, 18, 63, 52),
            Kind::Radio(1),
            IRect::new(450, 0, 508, 34),
            None,
        ),
        Element::new(
            id::CUT,
            IRect::new(5, 54, 33, 88),
            Kind::Radio(1),
            IRect::new(450, 36, 478, 70),
            None,
        ),
        Element::new(
            id::MOVE,
            IRect::new(35, 54, 63, 88),
            Kind::Radio(1),
            IRect::new(480, 36, 508, 70),
            None,
        ),
    ];
    for i in 0..7 {
        let x = 68 + 50 * i;
        elements.push(Element::new(
            id::TAB0 + i,
            IRect::new(x, 18, x + 49, 49),
            Kind::Radio(2),
            IRect::new(x, 53, x + 49, 84),
            None,
        ));
    }
    if enhanced {
        return elements;
    }
    elements.push(Element::new(
        id::PREV,
        IRect::new(423, 27, 444, 43),
        Kind::Push,
        IRect::new(450, 74, 471, 90),
        Some(IRect::new(471, 74, 492, 90)),
    ));
    elements.push(Element::new(
        id::NEXT,
        IRect::new(423, 65, 444, 81),
        Kind::Push,
        IRect::new(450, 90, 471, 106),
        Some(IRect::new(471, 90, 492, 106)),
    ));
    elements
}

impl Tray {
    /// The tray (`0x431281`): at (54, 0), 449×93, sheet 2; the Pointer,
    /// the first folder and its first page.
    #[must_use]
    pub fn new(assets: &Assets, pictures: &Pictures) -> Tray {
        let size = |i: u8| {
            let (sheet, n) = picture(i);
            pictures
                .rect(assets, Module::Gadget, sheet, n)
                .map_or((0, 0), |r| (i32::from(r.width()), i32::from(r.height())))
        };
        let mut t = Tray {
            bar: Bar {
                pos: (54, 0),
                size: (449, 93),
                sheet: 2,
                elements: elements(false),
                visible: true,
                capture: None,
            },
            folders: FOLDERS.iter().map(|l| layout(l, size)).collect(),
            pages: vec![0; 7],
            folder: 0,
            counts: [-1; 65],
            help_id: 0,
            help: String::new(),
            saved_tool: None,
            slides: Vec::new(),
            slide_start: 0,
            enhanced: false,
        };
        t.bar.select(id::POINTER);
        t.bar.select(id::TAB0);
        t.update_arrows();
        t
    }

    #[must_use]
    pub fn tool(&self) -> Tool {
        match self.bar.selected(1) {
            Some(id::CUT) => Tool::Cut,
            Some(id::MOVE) => Tool::Move,
            _ => Tool::Pointer,
        }
    }

    /// Selects tool `t` (`0x4325e0`).
    pub fn select_tool(&mut self, t: Tool) {
        self.bar.select(t.id());
    }

    /// The current page's parts.
    #[must_use]
    pub fn page(&self) -> &[Slot] {
        self.folders[self.folder]
            .get(self.pages[self.folder])
            .map_or(&[], Vec::as_slice)
    }

    /// The parts shown in the slot area, in its coordinates: the current
    /// page's, or the enhanced tray's whole folder.
    #[must_use]
    pub fn slots(&self) -> Vec<Slot> {
        if self.enhanced {
            folder_slots(&self.folders[self.folder]).collect()
        } else {
            self.page().to_vec()
        }
    }

    /// Turns the port's enhanced tray on or off: its buttons, its size,
    /// the current folder's first page.
    pub fn set_enhanced(&mut self, on: bool) {
        if on == self.enhanced {
            return;
        }
        let (tool, tab) = (self.tool(), self.bar.selected(2));
        self.enhanced = on;
        self.bar.elements = elements(on);
        self.bar.capture = None;
        self.select_tool(tool);
        if let Some(t) = tab {
            self.bar.select(t);
        }
        self.pages[self.folder] = 0;
        self.slides.clear();
        self.update_arrows();
        self.animate(self.slide_start);
    }

    /// The enhanced slot area's height for folder `f` (gray: none).
    fn full_height(&self, shown: Shown) -> i32 {
        match shown {
            Shown::Page(f, _) => AREA_H * i32::try_from(self.folders[f].len()).unwrap_or(0),
            Shown::Gray => 0,
        }
    }

    /// The enhanced slot area at `now`: its height and the folder rolled
    /// into it. A folder switch rolls the old folder up into the gray,
    /// then the new one down out of it.
    fn rolled(&self, now: u64) -> (i32, Shown) {
        let elapsed = now.saturating_sub(self.slide_start);
        let step = usize::try_from(elapsed / SLIDE_MS).unwrap_or(usize::MAX);
        let current = Shown::Page(self.folder, 0);
        let Some(&(from, to, up)) = self.slides.get(step) else {
            return (self.full_height(current), current);
        };
        let ms = i32::try_from(elapsed % SLIDE_MS).unwrap_or(0);
        let k = |h: i32| h * ms / i32::try_from(SLIDE_MS).unwrap_or(1);
        if up {
            let h = self.full_height(from);
            (h - k(h), from)
        } else {
            (k(self.full_height(to)), to)
        }
    }

    /// The tray's size at `now` (the enhanced tray's follows its slot
    /// area, which shows at least one row).
    pub fn animate(&mut self, now: u64) {
        self.bar.size = if self.enhanced {
            let (h, _) = self.rolled(now);
            (ENHANCED_W, AREA.y0 + h.max(AREA_H) + 2)
        } else {
            (449, 93)
        };
    }

    /// Whether `p` (worksheet coordinates) is on the tray: the enhanced
    /// tray leaves out the corner under the tools.
    #[must_use]
    pub fn contains(&self, p: (i32, i32)) -> bool {
        let q = (p.0 - self.bar.pos.0, p.1 - self.bar.pos.1);
        self.bar.rect().contains(p.0, p.1) && !(self.enhanced && q.1 >= TOOLS_H && q.0 < LOWER_X)
    }

    /// The outline drawn while the tray is dragged, at `at`: 2-pixel
    /// strips around its shape.
    #[must_use]
    pub fn outline(&self, at: (i32, i32)) -> Vec<IRect> {
        let (w, h) = self.bar.size;
        let strips = if self.enhanced && h > TOOLS_H {
            vec![
                IRect::new(0, 0, w, 2),
                IRect::new(w - 2, 2, w, h),
                IRect::new(0, 2, 2, TOOLS_H),
                IRect::new(2, TOOLS_H - 2, LOWER_X + 2, TOOLS_H),
                IRect::new(LOWER_X, TOOLS_H, LOWER_X + 2, h - 2),
                IRect::new(LOWER_X, h - 2, w - 2, h),
            ]
        } else {
            // The rectangle's (`0x430172`).
            vec![
                IRect::new(0, 0, w, 2),
                IRect::new(0, h - 2, w, h),
                IRect::new(0, 2, 2, h - 2),
                IRect::new(w - 2, 2, w, h - 2),
            ]
        };
        strips.into_iter().map(|r| r.offset(at.0, at.1)).collect()
    }

    /// The help line: the enhanced tray's ends at its narrower edge.
    fn help_rect(&self) -> IRect {
        if self.enhanced {
            IRect::new(HELP.x0, HELP.y0, ENHANCED_W - 4, HELP.y1)
        } else {
            HELP
        }
    }

    /// The slot area now (tray coordinates).
    fn area(&self) -> IRect {
        IRect::new(AREA.x0, AREA.y0, AREA.x1, self.bar.size.1 - 2)
    }

    /// The arrows and the number (`0x4322e5`).
    fn update_arrows(&mut self) {
        let (page, n) = (self.pages[self.folder], self.folders[self.folder].len());
        if let Some(e) = self.bar.get_mut(id::PREV) {
            e.enabled = page != 0;
        }
        if let Some(e) = self.bar.get_mut(id::NEXT) {
            e.enabled = page + 1 != n;
        }
    }

    /// Whether a slide is under way at `now`.
    #[must_use]
    pub fn sliding(&self, now: u64) -> bool {
        !self.slides.is_empty() && now < self.slide_start + SLIDE_MS * self.slides.len() as u64
    }

    /// Next (`up`) or previous page (`0x42edc5`, `0x42ee6e`).
    pub fn turn_page(&mut self, up: bool, now: u64) {
        let f = self.folder;
        let from = self.pages[f];
        let to = if up {
            (from + 1).min(self.folders[f].len().saturating_sub(1))
        } else {
            from.saturating_sub(1)
        };
        if to == from || self.enhanced {
            return;
        }
        self.pages[f] = to;
        self.slides = vec![(Shown::Page(f, from), Shown::Page(f, to), up)];
        self.slide_start = now;
        self.update_arrows();
    }

    /// Switches to folder `f` (`0x42f419`): the old page slides up into
    /// gray, gray down into the new folder's first page.
    pub fn switch_folder(&mut self, f: usize, now: u64) {
        if f == self.folder || f >= self.folders.len() {
            return;
        }
        let from = Shown::Page(self.folder, self.pages[self.folder]);
        self.folder = f;
        self.pages[f] = 0;
        self.slides = vec![(from, Shown::Gray, true), (Shown::Gray, Shown::Page(f, 0), false)];
        self.slide_start = now;
        self.update_arrows();
    }

    /// The part in the slot area at `p` (tray coordinates).
    #[must_use]
    pub fn slot_at(&self, p: (i32, i32)) -> Option<Slot> {
        if !self.area().contains(p.0, p.1) {
            return None;
        }
        let q = (p.0 - AREA.x0, p.1 - AREA.y0);
        self.slots().into_iter().find(|s| s.rect.contains(q.0, q.1))
    }

    /// The help line's string for the pointer at `p` over the tray
    /// (`0x42febc`); `None` leaves it as it is.
    #[must_use]
    pub fn hover_id(&self, p: (i32, i32)) -> Option<i32> {
        let elem = |q: (i32, i32)| {
            if self.help_rect().contains(q.0, q.1) {
                return Some(-1);
            }
            if let Some(i) = self.bar.element_at(q) {
                return Some(self.bar.elements[i].id);
            }
            if !self.enhanced && NUMBER_RECT.contains(q.0, q.1) {
                return Some(-1);
            }
            if self.area().contains(q.0, q.1) {
                return Some(self.slot_at(q).map_or(0, |s| i32::from(s.index)));
            }
            None
        };
        hover_string(elem(p), p)
    }

    /// Shows string `id` on the help line (if it changed).
    pub fn set_help(&mut self, sid: i32, assets: &Assets) {
        if sid != self.help_id {
            self.help_id = sid;
            self.help = u32::try_from(sid)
                .ok()
                .and_then(|s| assets.string(s))
                .unwrap_or_default();
        }
    }

    /// A count changes by one when a part is taken (`−1`) or given back
    /// (`+1`), unless it's unlimited (`0x42ef7e`).
    pub fn take(&mut self, i: u8) -> bool {
        let c = &mut self.counts[usize::from(i)];
        if *c == 0 {
            return false;
        }
        if *c > 0 {
            *c -= 1;
        }
        true
    }

    pub fn give_back(&mut self, i: u8) {
        let c = &mut self.counts[usize::from(i)];
        if *c != -1 {
            *c = c.saturating_add(1);
        }
    }

    /// Numpad ± (`0x42f060`): from −1, +1 gives 0; −1 isn't applied to −1.
    pub fn adjust(&mut self, i: u8, up: bool) {
        let c = &mut self.counts[usize::from(i)];
        if up {
            *c = if *c == -1 { 0 } else { c.saturating_add(1) };
        } else if *c != -1 {
            *c -= 1;
        }
    }

    /// Every count unlimited (`0x4329e0`).
    pub fn reset_counts(&mut self) {
        self.counts = [-1; 65];
    }

    /// Applies a document's limits (`numG`, `0x432b3c`); unknown tags are
    /// skipped.
    pub fn apply_limits(&mut self, limits: &[(Tag, i16)]) {
        for &(tag, n) in limits {
            if let Some(i) = index_of(tag) {
                self.counts[usize::from(i)] = n;
            }
        }
    }

    /// The limited counts, folder by folder (`0x432aa5`).
    #[must_use]
    pub fn limits(&self) -> Vec<(Tag, i16)> {
        let mut out = Vec::new();
        for f in &self.folders {
            for p in f {
                for s in p {
                    let c = self.counts[usize::from(s.index)];
                    if c != -1
                        && let Some(t) = tag_of(s.index)
                    {
                        out.push((t, c));
                    }
                }
            }
        }
        out
    }

    /// Draws one page into a 350×40 surface (`0x42e360`).
    fn draw_page(&self, s: &mut Surface, shown: Shown, assets: &Assets, pictures: &Pictures) {
        let all = s.bounds();
        s.fill(all, 255);
        let Shown::Page(f, p) = shown else {
            // The gray page: 0x808080.
            s.fill(all, 248);
            return;
        };
        let Some(page) = self.folders.get(f).and_then(|x| x.get(p)) else {
            return;
        };
        self.draw_slots(s, page.iter().copied(), assets, pictures);
    }

    /// Draws parts with their counts.
    fn draw_slots(&self, s: &mut Surface, slots: impl Iterator<Item = Slot>, assets: &Assets, pictures: &Pictures) {
        for slot in slots {
            let old = s.set_clip(slot.rect);
            let (sheet, n) = picture(slot.index);
            if let Some(img) = pictures.frame(assets, Module::Gadget, sheet, n) {
                s.blit(&img, slot.rect.x0, slot.rect.y0);
            }
            let count = self.counts[usize::from(slot.index)];
            if count == 0 {
                if let Some(img) = pictures.frame(assets, Module::Gadget, COUNT_SHEET, 1) {
                    s.blit(&img, slot.rect.x0, slot.rect.y0);
                }
            } else if count > 0 {
                let mut n = count;
                let mut x = slot.rect.x1 - 8;
                loop {
                    let d = u16::try_from(n % 10).unwrap_or(0);
                    if let Some(img) = pictures.frame(assets, Module::Gadget, COUNT_SHEET, 2 + d) {
                        s.blit(&img, x, slot.rect.y1 - 9);
                    }
                    n /= 10;
                    x -= 8;
                    if n == 0 {
                        break;
                    }
                }
            }
            s.set_clip(old);
        }
    }

    /// The enhanced tray's face, put together from the original's: its
    /// top (the move bar, the tools, the tabs) up to the slot area's
    /// right edge, then the window's own right edge; below the tools the
    /// slot area's sides, repeated down to its height, and the bottom.
    fn draw_enhanced_face(&self, s: &mut Surface, img: &Image) {
        let (x, y) = self.bar.pos;
        let h = self.bar.size.1;
        let bottom = h - 2;
        // `src` of the face at `dx`, `dy` in the tray, and the window's
        // edge beside it.
        let mut piece = |src: IRect, dx: i32, dy: i32| s.blit_part(img, src, x + dx, y + dy);
        let row = |x0: i32, x1: i32, r: i32| IRect::new(x0, r, x1, r + 1);
        piece(IRect::new(0, 0, KEEP_W, AREA.y0), 0, 0);
        piece(IRect::new(EDGE.x0, 0, EDGE.x1, AREA.y0), KEEP_W, 0);
        piece(IRect::new(0, AREA.y0, LOWER_X, TOOLS_H), 0, AREA.y0);
        if h <= TOOLS_H {
            piece(IRect::new(LOWER_X, AREA.y0, KEEP_W, TOOLS_H), LOWER_X, AREA.y0);
            piece(IRect::new(EDGE.x0, AREA.y0, EDGE.x1, TOOLS_H), KEEP_W, AREA.y0);
            return;
        }
        let orig_bottom = TOOLS_H - 2;
        for r in AREA.y0..bottom {
            let src = if r < orig_bottom { r } else { SIDE_ROW };
            if r < orig_bottom {
                piece(row(LOWER_X, AREA.x0, r), LOWER_X, r);
            } else {
                // The window's left edge, then the slot area's.
                piece(row(0, 3, SIDE_ROW), LOWER_X, r);
                piece(row(LOWER_X + 3, AREA.x0, SIDE_ROW), LOWER_X + 3, r);
            }
            piece(row(AREA.x1, KEEP_W, src), AREA.x1, r);
            piece(row(EDGE.x0, EDGE.x1, src), KEEP_W, r);
        }
        piece(row(0, 3, SIDE_ROW), LOWER_X, bottom);
        piece(row(LOWER_X + 3, KEEP_W, orig_bottom), LOWER_X + 3, bottom);
        piece(row(LOWER_X, KEEP_W, orig_bottom + 1), LOWER_X, bottom + 1);
        piece(IRect::new(EDGE.x0, orig_bottom, EDGE.x1, TOOLS_H), KEEP_W, bottom);
    }

    /// The enhanced slot area: the folder rolled into it, anchored at its
    /// bottom edge, over gray.
    fn draw_folder(&self, s: &mut Surface, assets: &Assets, pictures: &Pictures, now: u64) {
        let area = self.area();
        let (h, shown) = self.rolled(now);
        let h = h.min(area.height());
        let full = self.full_height(shown);
        let mut whole = Surface::new(350, u32::try_from(full.max(1)).unwrap_or(1));
        whole.fill(whole.bounds(), 255);
        if let Shown::Page(f, _) = shown {
            self.draw_slots(&mut whole, folder_slots(&self.folders[f]), assets, pictures);
        }
        let mut a = Surface::new(350, u32::try_from(area.height()).unwrap_or(0));
        a.fill(a.bounds(), 248);
        for row in 0..h {
            for col in 0..AREA_W {
                a.pset(col, row, whole.get(col, full - h + row).unwrap_or(255));
            }
        }
        let area = area.offset(self.bar.pos.0, self.bar.pos.1);
        let old = s.clip();
        s.set_clip(area.offset(s.origin.0, s.origin.1).intersect(old));
        s.copy_from(&a, area.x0, area.y0);
        s.set_clip(old);
    }

    /// Draws the tray at its place in `s` (worksheet coordinates).
    pub fn draw(&self, s: &mut Surface, assets: &Assets, pictures: &Pictures, now: u64) {
        let face = pictures.frame(assets, Module::Gadget, self.bar.sheet, 1);
        let (x, y) = self.bar.pos;
        match &face {
            Some(img) if self.enhanced => {
                self.draw_enhanced_face(s, img);
                for e in &self.bar.elements {
                    s.blit_part(img, e.source(), x + e.rect.x0, y + e.rect.y0);
                }
            }
            _ => self.bar.draw(s, assets, pictures),
        }
        if let Some(font) = assets.font("Small Fonts", 12, false) {
            let help = self.help_rect();
            let old = s.clip();
            let r = help.offset(x, y).offset(s.origin.0, s.origin.1).intersect(old);
            s.set_clip(r);
            s.text(font, x + help.x0, y + help.y0, &crate::text::encode(&self.help), 255);
            s.set_clip(old);
        }
        if self.enhanced {
            self.draw_folder(s, assets, pictures, now);
            return;
        }
        if let Some(img) = &face
            && let Some(&src) = DIGITS.get(self.pages[self.folder])
        {
            s.blit_part(img, src, x + NUMBER.0, y + NUMBER.1);
        }
        let mut a = Surface::new(350, 40);
        let area = AREA.offset(x, y);
        let elapsed = now.saturating_sub(self.slide_start);
        let step = usize::try_from(elapsed / SLIDE_MS).unwrap_or(usize::MAX);
        if let Some(&(from, to, up)) = self.slides.get(step) {
            let mut b = Surface::new(350, 40);
            self.draw_page(&mut a, from, assets, pictures);
            self.draw_page(&mut b, to, assets, pictures);
            let ms = i32::try_from(elapsed % SLIDE_MS).unwrap_or(0);
            let k = (ms * AREA_H / 180).min(AREA_H);
            let mut out = Surface::new(350, 40);
            for row in 0..AREA_H {
                let (src, sy) = if up {
                    if row < AREA_H - k {
                        (&a, row + k)
                    } else {
                        (&b, row - (AREA_H - k))
                    }
                } else if row >= k {
                    (&a, row - k)
                } else {
                    (&b, row + AREA_H - k)
                };
                for col in 0..AREA_W {
                    out.pset(col, row, src.get(col, sy).unwrap_or(255));
                }
            }
            a = out;
        } else {
            let shown = Shown::Page(self.folder, self.pages[self.folder]);
            self.draw_page(&mut a, shown, assets, pictures);
        }
        let old = s.clip();
        s.set_clip(area.offset(s.origin.0, s.origin.1).intersect(old));
        s.copy_from(&a, area.x0, area.y0);
        s.set_clip(old);
    }
}

const NUMBER_RECT: IRect = IRect::new(427, 48, 439, 59);

/// The hover text rule (`0x42febc`) for the first element's id at `p`:
/// ids from 1 show string `2500 + id`; others leave it; outside every
/// element string 4000 (none), or 4017 "Move Bar" on the move bar.
#[must_use]
pub fn hover_string(elem: Option<i32>, p: (i32, i32)) -> Option<i32> {
    match elem {
        Some(i) if i >= 1 => Some(2500 + i),
        Some(_) => None,
        None if p.1 <= MOVE_BAR => Some(4017),
        None => Some(4000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_and_indexes() {
        assert_eq!(tag_of(1), Some(Tag(*b"Pend")));
        assert_eq!(tag_of(64), Some(Tag(*b"TxLb")));
        assert_eq!(tag_of(0), None);
        assert_eq!(index_of(Tag(*b"MiCo")), Some(63));
        // Every part is in exactly one folder.
        let mut seen: Vec<i8> = FOLDERS
            .iter()
            .flat_map(|f| f.iter().copied())
            .filter(|&i| i > 0)
            .collect();
        seen.sort_unstable();
        assert_eq!(seen, (1..=64).collect::<Vec<i8>>());
    }

    #[test]
    fn pages_share_out_the_room() {
        // Display page 2: the Three-Digit (74×34) and Ten-Digit (175×30)
        // displays (docs/editor-layout.md 3.6: 33..107 and 140..315).
        let size = |i: u8| if i == 13 { (74, 34) } else { (175, 30) };
        let pages = layout(&[13, 14], size);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0][0].rect, IRect::new(33, 3, 107, 37));
        assert_eq!(pages[0][1].rect, IRect::new(140, 5, 315, 35));
        // A part that doesn't fit starts a page; −2 does too.
        let pages = layout(&[1, 2, -2, 3], |_| (150, 40));
        assert_eq!(pages.iter().map(Vec::len).collect::<Vec<_>>(), [2, 1]);
        let pages = layout(&[1, 2, 3], |_| (150, 40));
        assert_eq!(pages.iter().map(Vec::len).collect::<Vec<_>>(), [2, 1]);
        assert_eq!(pages[1][0].rect, IRect::new(100, 0, 250, 40));
    }

    #[test]
    fn counts() {
        let mut t = Tray {
            bar: super::super::toolbar::main_toolbar(),
            folders: vec![vec![vec![Slot {
                index: 5,
                rect: IRect::default(),
            }]]],
            pages: vec![0],
            folder: 0,
            counts: [-1; 65],
            help_id: 0,
            help: String::new(),
            saved_tool: None,
            slides: Vec::new(),
            slide_start: 0,
            enhanced: false,
        };
        assert!(t.take(5));
        assert_eq!(t.counts[5], -1);
        t.adjust(5, false);
        assert_eq!(t.counts[5], -1);
        t.adjust(5, true);
        assert_eq!(t.counts[5], 0);
        assert!(!t.take(5));
        t.give_back(5);
        assert_eq!(t.counts[5], 1);
        assert_eq!(t.limits(), [(Tag(*b"Ttlr"), 1)]);
        t.reset_counts();
        t.apply_limits(&[(Tag(*b"Ttlr"), 3), (Tag(*b"none"), 1)]);
        assert_eq!(t.counts[5], 3);
        assert!(t.take(5));
        assert_eq!(t.counts[5], 2);
    }

    #[test]
    fn hover_text_rule() {
        assert_eq!(hover_string(Some(1505), (10, 30)), Some(4005));
        assert_eq!(hover_string(Some(16), (100, 60)), Some(2516));
        assert_eq!(hover_string(Some(-1), (10, 5)), None);
        assert_eq!(hover_string(None, (1, 5)), Some(4017));
        assert_eq!(hover_string(None, (1, 50)), Some(4000));
    }
}
