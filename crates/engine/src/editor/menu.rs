//! The menu bar (`RT_MENU` 3) and the accelerators (`RT_ACCELERATOR` 2),
//! drawn in Windows 95's style at the top of the frame, since the platform
//! has no native menus (docs/editor-layout.md 5).

use formats::dialog::{MF_CHECKED, MF_GRAYED, MenuItem, parse_menu};
use formats::pe::{RT_ACCELERATOR, RT_MENU};

use crate::assets::{Assets, Module};
use crate::gfx::{IRect, Surface};

use super::ui;

/// The menu bar's height (`SM_CYMENU`).
pub const BAR_H: i32 = 19;
const ITEM_H: i32 = 17;
const SEP_H: i32 = 9;
/// Padding either side of a bar item's text.
const BAR_PAD: i32 = 6;
/// A popup's border and its left margin (the check mark's column).
const BORDER: i32 = 3;
const CHECK_W: i32 = 15;

/// Command ids (menu 3).
pub mod cmd {
    pub const NEW: u16 = 0xe100;
    pub const OPEN: u16 = 0xe101;
    pub const SAVE: u16 = 0xe103;
    pub const SAVE_AS: u16 = 0xe104;
    pub const PRINT_SETUP: u16 = 0xe106;
    pub const PRINT: u16 = 0xe107;
    pub const PRINT_PREVIEW: u16 = 0xe108;
    pub const EXIT: u16 = 0xe141;
    pub const UNDO: u16 = 0xe12b;
    pub const ABOUT: u16 = 0xe140;
    pub const LOAD_PUZZLE: u16 = 0x8003;
    pub const REVERT: u16 = 0x8004;
    pub const IMPORT: u16 = 0x8005;
    pub const SAVE_INTO: u16 = 0x8006;
    pub const CLEAR: u16 = 0x8009;
    pub const REMOVE_WIRES: u16 = 0x800a;
    pub const REMOVE_JOINTS: u16 = 0x800b;
    pub const PLAIN: u16 = 0x800c;
    pub const METAL: u16 = 0x800d;
    pub const NOISE: u16 = 0x800e;
    pub const STEEL: u16 = 0x800f;
    pub const CONTENTS: u16 = 0x8010;
    pub const FIT: u16 = 0x8011;
}

/// The items `Menu_UpdateForRunState` (`0x414b7e`) greys while running.
pub const RUN_DISABLED: [u16; 17] = [
    cmd::ABOUT,
    cmd::NEW,
    cmd::OPEN,
    cmd::LOAD_PUZZLE,
    cmd::SAVE,
    cmd::SAVE_AS,
    cmd::REVERT,
    cmd::IMPORT,
    cmd::SAVE_INTO,
    cmd::PRINT_SETUP,
    cmd::PRINT,
    cmd::PRINT_PREVIEW,
    cmd::FIT,
    cmd::UNDO,
    cmd::CLEAR,
    cmd::REMOVE_WIRES,
    cmd::REMOVE_JOINTS,
];

/// An accelerator: virtual key, Shift, Ctrl, Alt, command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accel {
    pub vk: u16,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub cmd: u16,
}

/// Reads an `RT_ACCELERATOR` table: `{u16 flags, u16 key, u16 cmd, u16}`,
/// the last with flag 0x80.
#[must_use]
pub fn parse_accels(d: &[u8]) -> Vec<Accel> {
    d.as_chunks::<8>()
        .0
        .iter()
        .map(|e| {
            let w = |i: usize| u16::from_le_bytes([e[i], e[i + 1]]);
            let flags = w(0);
            Accel {
                vk: w(2),
                shift: flags & 4 != 0,
                ctrl: flags & 8 != 0,
                alt: flags & 0x10 != 0,
                cmd: w(4),
            }
        })
        .collect()
}

/// The menu bar and its state.
#[derive(Debug, Clone, Default)]
pub struct Menu {
    pub items: Vec<MenuItem>,
    pub accels: Vec<Accel>,
    /// The commands greyed and checked now.
    pub grayed: Vec<u16>,
    pub checked: Vec<u16>,
    /// The bar item whose popup is open, or that has the keyboard.
    pub open: Option<usize>,
    /// Whether the popup is shown (else only the bar item is selected).
    pub dropped: bool,
    /// The highlighted popup item.
    pub hot: Option<usize>,
    /// The mouse went down on the bar this time.
    tracking: bool,
}

/// What the menu did with an input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Not for the menu.
    Ignored,
    /// Used (the menu stays or closed).
    Used,
    /// A command was chosen.
    Command(u16),
}

impl Menu {
    /// Menu 3 and accelerators 2 of `WIDGET.EXE`, with the initial flags.
    #[must_use]
    pub fn load(assets: &Assets) -> Menu {
        let items = assets
            .get(Module::Exe, RT_MENU, 3)
            .and_then(|d| parse_menu(d).ok())
            .unwrap_or_default();
        let accels = assets
            .get(Module::Exe, RT_ACCELERATOR, 2)
            .map(parse_accels)
            .unwrap_or_default();
        let mut m = Menu {
            items,
            accels,
            ..Menu::default()
        };
        for p in &m.items.clone() {
            if let MenuItem::Popup { items, .. } = p {
                for i in items {
                    if let MenuItem::Command { id, flags, .. } = i {
                        if flags & MF_GRAYED != 0 {
                            m.grayed.push(*id);
                        }
                        if flags & MF_CHECKED != 0 {
                            m.checked.push(*id);
                        }
                    }
                }
            }
        }
        m
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn close(&mut self) {
        self.open = None;
        self.dropped = false;
        self.hot = None;
        self.tracking = false;
    }

    #[must_use]
    pub fn enabled(&self, id: u16) -> bool {
        !self.grayed.contains(&id)
    }

    /// The command of accelerator `vk` with these modifiers.
    #[must_use]
    pub fn accel(&self, vk: u16, shift: bool, ctrl: bool, alt: bool) -> Option<u16> {
        self.accels
            .iter()
            .find(|a| a.vk == vk && a.shift == shift && a.ctrl == ctrl && a.alt == alt)
            .map(|a| a.cmd)
    }

    fn title(&self, i: usize) -> &str {
        match &self.items[i] {
            MenuItem::Popup { text, .. } | MenuItem::Command { text, .. } => text,
            MenuItem::Separator => "",
        }
    }

    fn popup(&self, i: usize) -> &[MenuItem] {
        match self.items.get(i) {
            Some(MenuItem::Popup { items, .. }) => items,
            _ => &[],
        }
    }

    /// The bar items' rectangles.
    #[must_use]
    pub fn bar_rects(&self, assets: &Assets) -> Vec<IRect> {
        let Some(font) = ui::font(assets) else {
            return Vec::new();
        };
        let mut x = 0;
        (0..self.items.len())
            .map(|i| {
                let (t, _) = ui::strip_mnemonic(self.title(i));
                let w = ui::width(font, &t) + 2 * BAR_PAD;
                let r = IRect::new(x, 1, x + w, BAR_H - 1);
                x += w;
                r
            })
            .collect()
    }

    /// Popup `i`'s rectangle and its items' rectangles (frame coordinates),
    /// and where the shortcut column starts.
    fn popup_layout(&self, assets: &Assets, i: usize) -> (IRect, Vec<IRect>, i32) {
        let Some(font) = ui::font(assets) else {
            return (IRect::default(), Vec::new(), 0);
        };
        let x = self.bar_rects(assets).get(i).map_or(0, |r| r.x0);
        let items = self.popup(i);
        let (mut tw, mut aw) = (0, 0);
        let mut h = 0;
        for it in items {
            match it {
                MenuItem::Command { text, .. } | MenuItem::Popup { text, .. } => {
                    let (t, a) = text.split_once('\t').unwrap_or((text, ""));
                    tw = tw.max(ui::width(font, &ui::strip_mnemonic(t).0));
                    aw = aw.max(ui::width(font, a));
                    h += ITEM_H;
                }
                MenuItem::Separator => h += SEP_H,
            }
        }
        let gap = if aw > 0 { 14 } else { 0 };
        let w = 2 * BORDER + CHECK_W + tw + gap + aw + CHECK_W;
        let r = IRect::sized(x, BAR_H, w, h + 2 * BORDER);
        let mut y = r.y0 + BORDER;
        let rects = items
            .iter()
            .map(|it| {
                let ih = if matches!(it, MenuItem::Separator) {
                    SEP_H
                } else {
                    ITEM_H
                };
                let ir = IRect::new(r.x0 + BORDER, y, r.x1 - BORDER, y + ih);
                y += ih;
                ir
            })
            .collect();
        (r, rects, r.x0 + BORDER + CHECK_W + tw + gap)
    }

    fn item_enabled(&self, it: &MenuItem) -> bool {
        match it {
            MenuItem::Command { id, .. } => self.enabled(*id),
            MenuItem::Popup { .. } => true,
            MenuItem::Separator => false,
        }
    }

    /// The mouse went down at `p` (frame coordinates).
    pub fn mouse_down(&mut self, assets: &Assets, p: (i32, i32)) -> Outcome {
        let bars = self.bar_rects(assets);
        if p.1 >= 0
            && p.1 < BAR_H
            && let Some(i) = bars.iter().position(|r| r.x0 <= p.0 && p.0 < r.x1)
        {
            if self.open == Some(i) && self.dropped {
                self.close();
            } else {
                self.open = Some(i);
                self.dropped = true;
                self.hot = None;
                self.tracking = true;
            }
            return Outcome::Used;
        }
        if let Some(i) = self.open.filter(|_| self.dropped) {
            let (r, _, _) = self.popup_layout(assets, i);
            if r.contains(p.0, p.1) {
                self.tracking = true;
                return Outcome::Used;
            }
            self.close();
            // The click that closes a menu goes nowhere else.
            return Outcome::Used;
        }
        if self.open.is_some() {
            self.close();
            return Outcome::Used;
        }
        Outcome::Ignored
    }

    /// The mouse moved to `p`.
    pub fn mouse_move(&mut self, assets: &Assets, p: (i32, i32)) {
        let Some(open) = self.open.filter(|_| self.dropped) else {
            return;
        };
        if p.1 >= 0 && p.1 < BAR_H {
            if let Some(i) = self.bar_rects(assets).iter().position(|r| r.x0 <= p.0 && p.0 < r.x1)
                && i != open
            {
                self.open = Some(i);
                self.hot = None;
            }
            return;
        }
        let (r, rects, _) = self.popup_layout(assets, open);
        if r.contains(p.0, p.1) {
            self.hot = rects
                .iter()
                .position(|ir| ir.contains(p.0, p.1))
                .filter(|&k| !matches!(self.popup(open)[k], MenuItem::Separator));
        } else if self.tracking {
            self.hot = None;
        }
    }

    /// The mouse went up at `p`: a command if over an enabled item.
    pub fn mouse_up(&mut self, assets: &Assets, p: (i32, i32)) -> Outcome {
        let Some(open) = self.open.filter(|_| self.dropped) else {
            return Outcome::Ignored;
        };
        self.tracking = false;
        let (r, rects, _) = self.popup_layout(assets, open);
        if !r.contains(p.0, p.1) {
            return Outcome::Used;
        }
        let Some(k) = rects.iter().position(|ir| ir.contains(p.0, p.1)) else {
            return Outcome::Used;
        };
        self.choose(open, k)
    }

    fn choose(&mut self, open: usize, k: usize) -> Outcome {
        let it = &self.popup(open)[k];
        if let MenuItem::Command { id, .. } = it
            && self.enabled(*id)
        {
            let id = *id;
            self.close();
            return Outcome::Command(id);
        }
        Outcome::Used
    }

    /// Opens the bar item with mnemonic `c` (Alt+letter), if any.
    pub fn open_mnemonic(&mut self, c: char) -> bool {
        let Some(i) = (0..self.items.len()).find(|&i| ui::mnemonic(self.title(i)) == Some(c)) else {
            return false;
        };
        self.open = Some(i);
        self.dropped = true;
        self.hot = self.first_enabled(i, 0, true);
        true
    }

    /// Alt (or F10) alone: the bar takes the keyboard.
    pub fn activate(&mut self) {
        if self.open.is_some() {
            self.close();
        } else if !self.items.is_empty() {
            self.open = Some(0);
            self.dropped = false;
        }
    }

    fn first_enabled(&self, i: usize, from: usize, down: bool) -> Option<usize> {
        let items = self.popup(i);
        let n = items.len();
        (0..n)
            .map(|d| if down { (from + d) % n } else { (from + n - d) % n })
            .find(|&k| !matches!(items[k], MenuItem::Separator))
    }

    /// A key while the menu has the keyboard.
    pub fn key(&mut self, vk: u16) -> Outcome {
        let Some(open) = self.open else {
            return Outcome::Ignored;
        };
        let n = self.items.len();
        match vk {
            0x1b => {
                if self.dropped {
                    self.dropped = false;
                    self.hot = None;
                } else {
                    self.close();
                }
            }
            0x25 | 0x27 => {
                let next = if vk == 0x25 { (open + n - 1) % n } else { (open + 1) % n };
                self.open = Some(next);
                if self.dropped {
                    self.hot = self.first_enabled(next, 0, true);
                }
            }
            0x26 | 0x28 => {
                if self.dropped {
                    let len = self.popup(open).len();
                    if len > 0 {
                        let down = vk == 0x28;
                        let from = match self.hot {
                            Some(h) if down => (h + 1) % len,
                            Some(h) => (h + len - 1) % len,
                            None => 0,
                        };
                        self.hot = self.first_enabled(open, from, down);
                    }
                } else {
                    self.dropped = true;
                    self.hot = self.first_enabled(open, 0, true);
                }
            }
            0x0d => {
                if self.dropped {
                    if let Some(h) = self.hot {
                        return self.choose(open, h);
                    }
                } else {
                    self.dropped = true;
                    self.hot = self.first_enabled(open, 0, true);
                }
            }
            0x12 | 0x79 => self.close(),
            k if (0x30..=0x5a).contains(&k) => {
                let c = char::from(u8::try_from(k).unwrap_or(0));
                if self.dropped {
                    let items = self.popup(open);
                    if let Some(h) = items.iter().position(|it| match it {
                        MenuItem::Command { text, .. } => ui::mnemonic(text) == Some(c),
                        _ => false,
                    }) {
                        return self.choose(open, h);
                    }
                } else {
                    self.open_mnemonic(c);
                }
            }
            _ => {}
        }
        Outcome::Used
    }

    /// Draws the bar across `width` and the open popup.
    pub fn draw(&self, s: &mut Surface, assets: &Assets, width: i32) {
        s.fill(IRect::new(0, 0, width, BAR_H), ui::FACE);
        let Some(font) = ui::font(assets) else { return };
        let ty = (BAR_H - i32::from(font.height)) / 2;
        for (i, r) in self.bar_rects(assets).into_iter().enumerate() {
            let sel = self.open == Some(i);
            let off = i32::from(sel && self.dropped);
            if sel {
                if self.dropped {
                    ui::edge(s, r, ui::SHADOW, ui::LIGHT);
                } else {
                    ui::edge(s, r, ui::LIGHT, ui::SHADOW);
                }
            }
            ui::label(s, font, r.x0 + BAR_PAD + off, ty + off, self.title(i), ui::BLACK);
        }
        let Some(open) = self.open.filter(|_| self.dropped) else {
            return;
        };
        let (r, rects, acc_x) = self.popup_layout(assets, open);
        s.fill(r, ui::FACE);
        ui::raised(s, r);
        for (k, (it, ir)) in self.popup(open).iter().zip(rects).enumerate() {
            match it {
                MenuItem::Separator => {
                    let y = ir.y0 + SEP_H / 2 - 1;
                    s.fill(IRect::new(ir.x0 + 1, y, ir.x1 - 1, y + 1), ui::SHADOW);
                    s.fill(IRect::new(ir.x0 + 1, y + 1, ir.x1 - 1, y + 2), ui::LIGHT);
                }
                MenuItem::Command { text, .. } | MenuItem::Popup { text, .. } => {
                    let enabled = self.item_enabled(it);
                    let hot = self.hot == Some(k);
                    if hot {
                        s.fill(ir, ui::NAVY);
                    }
                    let (t, a) = text.split_once('\t').unwrap_or((text, ""));
                    let ty = ir.y0 + (ITEM_H - i32::from(font.height)) / 2;
                    let tx = ir.x0 + CHECK_W;
                    let c = if hot { ui::WHITE } else { ui::BLACK };
                    if !enabled && !hot {
                        ui::grey_label(s, font, tx, ty, t);
                        ui::grey_label(s, font, acc_x, ty, a);
                    } else {
                        let c = if enabled { c } else { ui::SHADOW };
                        ui::label(s, font, tx, ty, t, c);
                        ui::label(s, font, acc_x, ty, a, c);
                    }
                    if let MenuItem::Command { id, .. } = it
                        && self.checked.contains(id)
                    {
                        draw_check(s, ir.x0 + 4, ir.y0 + (ITEM_H - 7) / 2, c);
                    }
                }
            }
        }
    }

    /// Whether `p` is over the menu (the bar or the open popup).
    #[must_use]
    pub fn covers(&self, assets: &Assets, p: (i32, i32)) -> bool {
        if p.1 >= 0 && p.1 < BAR_H {
            return true;
        }
        self.open
            .filter(|_| self.dropped)
            .is_some_and(|i| self.popup_layout(assets, i).0.contains(p.0, p.1))
    }
}

/// The menu check mark (7×7).
fn draw_check(s: &mut Surface, x: i32, y: i32, c: u8) {
    const ROWS: [u8; 7] = [0x02, 0x06, 0x8e, 0xdc, 0xf8, 0x70, 0x20];
    for (dy, bits) in (0..).zip(ROWS) {
        for dx in 0..8 {
            if bits & (0x80 >> dx) != 0 {
                s.pset(x + dx, y + dy, c);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accelerators() {
        let d = [
            0x09, 0, b'N', 0, 0x00, 0xe1, 0, 0, 0x11, 0, 0x08, 0, 0x2b, 0xe1, 0, 0, 0x81, 0, 0x70, 0, 0x10, 0x80, 0, 0,
        ];
        let a = parse_accels(&d);
        let m = Menu {
            accels: a,
            ..Menu::default()
        };
        assert_eq!(m.accel(u16::from(b'N'), false, true, false), Some(cmd::NEW));
        assert_eq!(m.accel(u16::from(b'N'), false, false, false), None);
        assert_eq!(m.accel(8, false, false, true), Some(cmd::UNDO));
        assert_eq!(m.accel(0x70, false, false, false), Some(cmd::CONTENTS));
    }

    #[test]
    fn keyboard_navigation() {
        let pop = |t: &str, ids: &[u16]| MenuItem::Popup {
            text: t.into(),
            flags: 0x10,
            items: ids
                .iter()
                .map(|&id| {
                    if id == 0 {
                        MenuItem::Separator
                    } else {
                        MenuItem::Command {
                            id,
                            text: format!("&{id}"),
                            flags: 0,
                        }
                    }
                })
                .collect(),
        };
        let mut m = Menu {
            items: vec![pop("&File", &[1, 0, 2]), pop("&Edit", &[3])],
            ..Menu::default()
        };
        m.activate();
        assert_eq!((m.open, m.dropped), (Some(0), false));
        assert_eq!(m.key(0x28), Outcome::Used);
        assert_eq!(m.hot, Some(0));
        m.key(0x28);
        assert_eq!(m.hot, Some(2), "the separator is skipped");
        m.grayed.push(2);
        assert_eq!(m.key(0x0d), Outcome::Used, "greyed items do nothing");
        m.key(0x27);
        assert_eq!((m.open, m.hot), (Some(1), Some(0)));
        assert_eq!(m.key(0x0d), Outcome::Command(3));
        assert!(!m.is_open());
        assert!(m.open_mnemonic('F'));
        assert_eq!(m.open, Some(0));
    }
}
