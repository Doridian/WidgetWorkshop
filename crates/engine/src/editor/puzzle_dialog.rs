//! "Select a puzzle" (`cPuzzleDlg`, docs/puzzles.md 2.3): dialog 10000 of
//! `WIDGET.EXE`, a list of the puzzles with `+` before the solved ones,
//! Ok, Cancel and "Reset Completed Puzzles". Drawn from its `RT_DIALOG`
//! template in Windows 95's style, like the settings dialogs
//! ([`super::dialog`]).
//!
//! The original shows no description: a puzzle's instructions are text
//! parts on its own worksheet.

use formats::dialog::{Class, Dialog as Template};
use formats::pe::RT_DIALOG;

use crate::assets::{Assets, Module};
use crate::gfx::{IRect, Surface};

use super::dialog::{CAPTION_H, FRAME, base_units, close_box, draw_window, mul_div};
use super::menu::BAR_H;
use super::ui;

/// The template.
pub const TEMPLATE: u32 = 10000;
/// The controls' ids.
const OK: u16 = 1;
const CANCEL: u16 = 2;
const RESET: u16 = 0x69;
const LIST: u16 = 0x6b;
/// A scroll bar's width and its arrows' height (`SM_CXVSCROLL`).
const SB: i32 = 16;
/// The smallest thumb (`SM_CYVTHUMB` is the arrow's height, but Windows
/// shrinks it to 8 on a short track).
const MIN_THUMB: i32 = 8;

/// The confirmation "Reset Completed Puzzles" asks for (`0x45f99f(msg,
/// MB_OKCANCEL)`, docs/puzzles.md 2.3).
pub const RESET_QUESTION: &str = "Do you really want to mark all puzzles as unsolved?\nThis is a permanent change\n";

/// What an input left the dialog as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Open,
    /// Ok with list line `n` selected: puzzle `n + 1`.
    Ok(usize),
    Cancel,
    /// "Reset Completed Puzzles" clicked: the question comes next.
    Reset,
}

#[derive(Debug, Clone)]
struct Button {
    id: u16,
    rect: IRect,
    text: String,
    enabled: bool,
}

/// What the mouse holds down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Press {
    Button(usize),
    Close,
    Up,
    Down,
    PageUp,
    PageDown,
    /// The thumb, grabbed `grab` pixels below its top.
    Thumb {
        grab: i32,
    },
    List,
}

/// The open dialog.
#[derive(Debug, Clone)]
pub struct PuzzleDialog {
    title: String,
    /// The window in frame coordinates, and its client area.
    window: IRect,
    client: IRect,
    buttons: Vec<Button>,
    /// The list box (client coordinates), its border included.
    list: IRect,
    /// The lines: `"  title"`, or `"+ title"` when solved.
    pub lines: Vec<String>,
    /// The selected line (`LB_GETCURSEL`).
    pub sel: Option<usize>,
    /// The first line shown.
    top: usize,
    item_h: i32,
    pressed: Option<Press>,
}

/// List line of a puzzle (`0x42cef8`): two spaces, the title; a `+` for
/// the first if solved.
#[must_use]
pub fn line(title: &str, solved: bool) -> String {
    format!("{}{title}", if solved { "+ " } else { "  " })
}

impl PuzzleDialog {
    /// The dialog (`0x42d284`, `OnInitDialog` `0x42cef8`) with `lines`,
    /// the first selected; Reset disabled when no line is marked.
    #[must_use]
    pub fn new(assets: &Assets, lines: Vec<String>) -> Option<PuzzleDialog> {
        let t = Template::parse(assets.get(Module::Exe, RT_DIALOG, TEMPLATE)?).ok()?;
        let (bx, by) = base_units(assets);
        let px = |x: i16, y: i16| (mul_div(i32::from(x), bx, 4), mul_div(i32::from(y), by, 8));
        let (cw, ch) = px(t.cx, t.cy);
        let (x, y) = px(t.x, t.y);
        let window = IRect::sized(x, y + BAR_H, cw + 2 * FRAME, ch + 2 * FRAME + CAPTION_H);
        let client = IRect::sized(window.x0 + FRAME, window.y0 + FRAME + CAPTION_H, cw, ch);
        let item_h = ui::font(assets).map_or(13, |f| i32::from(f.height));
        let mut buttons = Vec::new();
        let mut list = IRect::default();
        for it in t.items.iter().filter(|i| i.visible()) {
            let (x0, y0) = px(it.x, it.y);
            let (w, h) = px(it.cx, it.cy);
            match it.class() {
                Class::Button => buttons.push(Button {
                    id: it.id,
                    rect: IRect::sized(x0, y0, w, h),
                    text: it.title.text().to_owned(),
                    enabled: true,
                }),
                Class::ListBox if it.id == LIST => {
                    // No `LBS_NOINTEGRALHEIGHT`: the box shrinks to whole
                    // lines inside its 2-pixel edge.
                    let rows = ((h - 4) / item_h).max(1);
                    list = IRect::sized(x0, y0, w, rows * item_h + 4);
                }
                _ => {}
            }
        }
        let any = lines.iter().any(|l| !l.starts_with(' '));
        let mut d = PuzzleDialog {
            title: t.title.clone(),
            window,
            client,
            buttons,
            list,
            sel: (!lines.is_empty()).then_some(0),
            lines,
            top: 0,
            item_h,
            pressed: None,
        };
        d.set_enabled(RESET, any);
        Some(d)
    }

    fn set_enabled(&mut self, id: u16, on: bool) {
        for b in self.buttons.iter_mut().filter(|b| b.id == id) {
            b.enabled = on;
        }
    }

    /// Whether button `id` is enabled.
    #[must_use]
    pub fn enabled(&self, id: u16) -> bool {
        self.buttons.iter().any(|b| b.id == id && b.enabled)
    }

    /// After a reset: every line's mark blanked, Reset disabled.
    pub fn clear_marks(&mut self) {
        for l in &mut self.lines {
            if !l.starts_with(' ') && !l.is_empty() {
                l.replace_range(..1, " ");
            }
        }
        self.set_enabled(RESET, false);
    }

    fn rows(&self) -> usize {
        usize::try_from((self.list.height() - 4) / self.item_h)
            .unwrap_or(1)
            .max(1)
    }

    fn max_top(&self) -> usize {
        self.lines.len().saturating_sub(self.rows())
    }

    fn has_bar(&self) -> bool {
        self.lines.len() > self.rows()
    }

    /// Inside the list's edge, without the scroll bar.
    fn items_rect(&self) -> IRect {
        let r = self.list.inset(2);
        if self.has_bar() {
            IRect::new(r.x0, r.y0, r.x1 - SB, r.y1)
        } else {
            r
        }
    }

    fn bar_rect(&self) -> IRect {
        let r = self.list.inset(2);
        IRect::new(r.x1 - SB, r.y0, r.x1, r.y1)
    }

    /// The thumb (client coordinates).
    fn thumb(&self) -> IRect {
        let b = self.bar_rect();
        let track = b.height() - 2 * SB;
        let n = i32::try_from(self.lines.len()).unwrap_or(1).max(1);
        let rows = i32::try_from(self.rows()).unwrap_or(1);
        let h = (track * rows / n).max(MIN_THUMB).min(track);
        let max = i32::try_from(self.max_top()).unwrap_or(0).max(1);
        let top = i32::try_from(self.top).unwrap_or(0);
        let y = b.y0 + SB + (track - h) * top / max;
        IRect::sized(b.x0, y, SB, h)
    }

    fn scroll_to(&mut self, top: usize) {
        self.top = top.min(self.max_top());
    }

    /// Selects line `i` and scrolls it into view.
    fn select(&mut self, i: usize) {
        if self.lines.is_empty() {
            return;
        }
        let i = i.min(self.lines.len() - 1);
        self.sel = Some(i);
        let rows = self.rows();
        if i < self.top {
            self.top = i;
        } else if i >= self.top + rows {
            self.top = i + 1 - rows;
        }
    }

    fn line_at(&self, q: (i32, i32)) -> Option<usize> {
        let r = self.items_rect();
        let row = usize::try_from((q.1 - r.y0).max(0) / self.item_h).ok()?;
        let i = self.top + row.min(self.rows() - 1);
        (i < self.lines.len()).then_some(i)
    }

    fn to_client(&self, p: (i32, i32)) -> (i32, i32) {
        (p.0 - self.client.x0, p.1 - self.client.y0)
    }

    /// Ok: the selected line, or Cancel when there's none (`0x42d01e`).
    fn ok(&self) -> Outcome {
        self.sel.map_or(Outcome::Cancel, Outcome::Ok)
    }

    pub fn mouse_down(&mut self, p: (i32, i32), dbl: bool) -> Outcome {
        if close_box(self.window).contains(p.0, p.1) {
            self.pressed = Some(Press::Close);
            return Outcome::Open;
        }
        let q = self.to_client(p);
        if let Some(i) = self.buttons.iter().position(|b| b.rect.contains(q.0, q.1)) {
            if self.buttons[i].enabled {
                self.pressed = Some(Press::Button(i));
            }
            return Outcome::Open;
        }
        if self.has_bar() && self.bar_rect().contains(q.0, q.1) {
            let b = self.bar_rect();
            let t = self.thumb();
            let press = if q.1 < b.y0 + SB {
                self.scroll_to(self.top.saturating_sub(1));
                Press::Up
            } else if q.1 >= b.y1 - SB {
                self.scroll_to(self.top + 1);
                Press::Down
            } else if q.1 < t.y0 {
                self.scroll_to(self.top.saturating_sub(self.rows()));
                Press::PageUp
            } else if q.1 >= t.y1 {
                self.scroll_to(self.top + self.rows());
                Press::PageDown
            } else {
                Press::Thumb { grab: q.1 - t.y0 }
            };
            self.pressed = Some(press);
            return Outcome::Open;
        }
        if self.items_rect().contains(q.0, q.1)
            && let Some(i) = self.line_at(q)
        {
            self.sel = Some(i);
            // `LBN_DBLCLK` posts IDOK.
            if dbl {
                return self.ok();
            }
            self.pressed = Some(Press::List);
        }
        Outcome::Open
    }

    pub fn mouse_move(&mut self, p: (i32, i32)) {
        let q = self.to_client(p);
        match self.pressed {
            Some(Press::Thumb { grab }) => {
                let b = self.bar_rect();
                let t = self.thumb();
                let room = b.height() - 2 * SB - t.height();
                if room > 0 {
                    let y = (q.1 - grab - b.y0 - SB).clamp(0, room);
                    let max = i32::try_from(self.max_top()).unwrap_or(0);
                    let top = (y * max + room / 2) / room;
                    self.scroll_to(usize::try_from(top).unwrap_or(0));
                }
            }
            Some(Press::List) => {
                // Dragging along the list moves the selection (within the
                // lines shown).
                if let Some(i) = self.line_at(q) {
                    self.sel = Some(i);
                }
            }
            _ => {}
        }
    }

    pub fn mouse_up(&mut self, p: (i32, i32)) -> Outcome {
        let Some(press) = self.pressed.take() else {
            return Outcome::Open;
        };
        let q = self.to_client(p);
        match press {
            Press::Close if close_box(self.window).contains(p.0, p.1) => Outcome::Cancel,
            Press::Button(i) if self.buttons[i].rect.contains(q.0, q.1) => match self.buttons[i].id {
                OK => self.ok(),
                CANCEL => Outcome::Cancel,
                RESET => Outcome::Reset,
                _ => Outcome::Open,
            },
            _ => Outcome::Open,
        }
    }

    /// A key: the list's arrows and pages, Enter the default button (Ok),
    /// Esc Cancel.
    pub fn key(&mut self, vk: u16) -> Outcome {
        let rows = self.rows();
        let sel = self.sel.unwrap_or(0);
        let last = self.lines.len().saturating_sub(1);
        match vk {
            0x0d => return self.ok(),
            0x1b => return Outcome::Cancel,
            0x26 => self.select(sel.saturating_sub(1)),
            0x28 => self.select(if self.sel.is_some() { sel + 1 } else { 0 }),
            0x21 => self.select(sel.saturating_sub(rows - 1)),
            0x22 => self.select(sel + rows - 1),
            0x24 => self.select(0),
            0x23 => self.select(last),
            _ => {}
        }
        Outcome::Open
    }

    /// The wheel: three lines a notch.
    pub fn wheel(&mut self, dy: i32) {
        if dy > 0 {
            self.scroll_to(self.top.saturating_sub(3));
        } else if dy < 0 {
            self.scroll_to(self.top + 3);
        }
    }

    pub fn draw(&self, s: &mut Surface, assets: &Assets) {
        let Some(font) = ui::font(assets) else { return };
        draw_window(s, font, self.window, &self.title, self.pressed == Some(Press::Close));
        let old = s.clip();
        s.set_clip(self.client.intersect(old));
        let (ox, oy) = (self.client.x0, self.client.y0);
        for (i, b) in self.buttons.iter().enumerate() {
            let pressed = self.pressed == Some(Press::Button(i));
            ui::button(s, font, b.rect.offset(ox, oy), &b.text, pressed, b.id == OK, b.enabled);
        }
        let l = self.list.offset(ox, oy);
        s.fill(l, ui::WHITE);
        ui::sunken(s, l);
        let items = self.items_rect().offset(ox, oy);
        let inner = s.clip();
        s.set_clip(items.intersect(inner));
        for (row, i) in (self.top..self.lines.len()).take(self.rows()).enumerate() {
            let y = items.y0 + i32::try_from(row).unwrap_or(0) * self.item_h;
            let r = IRect::new(items.x0, y, items.x1, y + self.item_h);
            let hot = self.sel == Some(i);
            if hot {
                s.fill(r, ui::NAVY);
            }
            let c = if hot { ui::WHITE } else { ui::BLACK };
            s.text(font, r.x0 + 2, y, &crate::text::encode(&self.lines[i]), c);
            if hot {
                focus_rect(s, r);
            }
        }
        s.set_clip(inner);
        if self.has_bar() {
            self.draw_bar(s, font);
        }
        s.set_clip(old);
    }

    fn draw_bar(&self, s: &mut Surface, font: &formats::fnt::Font) {
        let (ox, oy) = (self.client.x0, self.client.y0);
        let b = self.bar_rect().offset(ox, oy);
        // The track: white and light grey, checkered.
        for y in b.y0..b.y1 {
            for x in b.x0..b.x1 {
                s.pset(x, y, if (x + y) % 2 == 0 { ui::WHITE } else { ui::FACE });
            }
        }
        let up = IRect::sized(b.x0, b.y0, SB, SB);
        let down = IRect::sized(b.x0, b.y1 - SB, SB, SB);
        for (r, press, pointing_up) in [(up, Press::Up, true), (down, Press::Down, false)] {
            let pressed = self.pressed == Some(press);
            ui::button(s, font, r, "", pressed, false, true);
            let off = i32::from(pressed);
            let (cx, cy) = (r.x0 + 4 + off, r.y0 + 6 + off);
            for d in 0..4 {
                let row = if pointing_up { cy + d } else { cy + 3 - d };
                s.fill(IRect::new(cx + 3 - d, row, cx + 4 + d, row + 1), ui::BLACK);
            }
        }
        let t = self.thumb().offset(ox, oy);
        ui::button(s, font, t, "", false, false, true);
    }
}

/// `DrawFocusRect`: a dotted, inverted outline.
pub(super) fn focus_rect(s: &mut Surface, r: IRect) {
    for x in (r.x0..r.x1).step_by(2) {
        s.invert(IRect::sized(x, r.y0, 1, 1));
        s.invert(IRect::sized(x, r.y1 - 1, 1, 1));
    }
    for y in (r.y0 + 2..r.y1 - 1).step_by(2) {
        s.invert(IRect::sized(r.x0, y, 1, 1));
        s.invert(IRect::sized(r.x1 - 1, y, 1, 1));
    }
}
