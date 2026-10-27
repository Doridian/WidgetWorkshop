//! The port's own Settings window (Options ▸ Settings...), in the look of
//! the original's dialogs: the UI scale, which the window follows at once,
//! and the one switch for all of the port's enhancements of the original
//! ([`super::tray`]'s whole-folder tray, windows dragged whole). The host keeps both
//! ([`super::Editor::take_ui_scale`], [`super::Editor::take_enhanced`]).

use crate::assets::Assets;
use crate::gfx::{IRect, Surface};

use super::dialog::{CAPTION_H, FRAME, check_mark, close_box, draw_window};
use super::ui;

/// The UI scale the player asks for: the display's, or a fixed one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scale {
    Auto,
    Fixed(f64),
}

/// The smallest and largest UI scale, and the step of − and +.
pub const MIN_SCALE: f64 = 1.0;
pub const MAX_SCALE: f64 = 4.0;
const STEP: f64 = 0.25;

const W: i32 = 252;
const MARGIN: i32 = 10;
const BUTTON_W: i32 = 75;
const BUTTON_H: i32 = 23;
const SMALL: i32 = 23;
const CHECK_H: i32 = 16;
/// The enhancements' note under their switch.
const NOTE: &str = "The parts tray shows a whole folder at once; windows drag whole.";

/// What an input did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    None,
    Scale(Scale),
    Enhanced(bool),
    Close,
}

/// The parts of the window that take clicks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Close,
    CloseBox,
    Auto,
    Smaller,
    Bigger,
    Enhanced,
}

const PARTS: [Part; 6] = [
    Part::Close,
    Part::CloseBox,
    Part::Auto,
    Part::Smaller,
    Part::Bigger,
    Part::Enhanced,
];

/// The window.
#[derive(Debug, Clone)]
pub struct SettingsBox {
    /// The scale asked for and the one it gives now.
    pub scale: Scale,
    pub current: f64,
    pub enhanced: bool,
    window: IRect,
    /// The part the mouse holds down.
    pressed: Option<Part>,
}

/// `s` one step bigger (`up`) or smaller, in whole steps from 1 to 4.
#[must_use]
pub fn step(s: f64, up: bool) -> f64 {
    let n = (s / STEP).round() + if up { 1.0 } else { -1.0 };
    (n * STEP).clamp(MIN_SCALE, MAX_SCALE)
}

/// `s` as the window shows it: "1.5×".
#[must_use]
pub fn label(s: f64) -> String {
    format!("{s}×")
}

impl SettingsBox {
    /// The window centred in `area`.
    #[must_use]
    pub fn new(assets: &Assets, area: IRect, scale: Scale, current: f64, enhanced: bool) -> SettingsBox {
        let h = Self::height(assets);
        let window = IRect::sized(
            area.x0 + (area.width() - W) / 2,
            area.y0 + (area.height() - h) / 2,
            W,
            h,
        );
        SettingsBox {
            scale,
            current,
            enhanced,
            window,
            pressed: None,
        }
    }

    fn note(assets: &Assets) -> Vec<String> {
        ui::font(assets).map_or_else(Vec::new, |f| ui::wrap(f, NOTE, W - 2 * FRAME - 2 * MARGIN - 17))
    }

    fn height(assets: &Assets) -> i32 {
        let lh = ui::font(assets).map_or(13, |f| i32::from(f.height));
        let note = i32::try_from(Self::note(assets).len()).unwrap_or(0) * lh;
        2 * FRAME
            + CAPTION_H
            + 3 * MARGIN
            + lh
            + 4
            + CHECK_H
            + 6
            + SMALL
            + 2 * MARGIN
            + CHECK_H
            + note
            + MARGIN
            + BUTTON_H
            + MARGIN
    }

    /// The client area's top left.
    fn client(&self) -> (i32, i32) {
        (
            self.window.x0 + FRAME + MARGIN,
            self.window.y0 + FRAME + CAPTION_H + MARGIN,
        )
    }

    /// The rectangles: the UI scale's group, its check box, −, the value,
    /// +, the enhancements' check box, Close.
    fn layout(&self, assets: &Assets) -> Layout {
        let lh = ui::font(assets).map_or(13, |f| i32::from(f.height));
        let (x, y) = self.client();
        let inner = W - 2 * FRAME - 2 * MARGIN;
        let group = IRect::sized(x, y + lh / 2, inner, lh / 2 + MARGIN + CHECK_H + 6 + SMALL + MARGIN);
        let auto = IRect::sized(x + MARGIN, y + lh + 4, inner - 2 * MARGIN, CHECK_H);
        let row = auto.y1 + 6;
        let smaller = IRect::sized(x + MARGIN + 17, row, SMALL, SMALL);
        let value = IRect::sized(smaller.x1 + 4, row, 56, SMALL);
        let bigger = IRect::sized(value.x1 + 4, row, SMALL, SMALL);
        let enhanced = IRect::sized(x, group.y1 + MARGIN, inner, CHECK_H);
        let close = IRect::sized(
            self.window.x1 - FRAME - MARGIN - BUTTON_W,
            self.window.y1 - FRAME - MARGIN - BUTTON_H,
            BUTTON_W,
            BUTTON_H,
        );
        Layout {
            group,
            auto,
            smaller,
            value,
            bigger,
            enhanced,
            close,
        }
    }

    fn rect(&self, l: &Layout, p: Part) -> IRect {
        match p {
            Part::Close => l.close,
            Part::CloseBox => close_box(self.window),
            Part::Auto => l.auto,
            Part::Smaller => l.smaller,
            Part::Bigger => l.bigger,
            Part::Enhanced => l.enhanced,
        }
    }

    /// The scale − and + start from: the fixed one, or what Automatic
    /// gives now.
    fn shown(&self) -> f64 {
        match self.scale {
            Scale::Auto => self.current,
            Scale::Fixed(s) => s,
        }
    }

    fn enabled(&self, p: Part) -> bool {
        match p {
            Part::Smaller => self.shown() > MIN_SCALE,
            Part::Bigger => self.shown() < MAX_SCALE,
            _ => true,
        }
    }

    /// What pressing `p` does.
    fn act(&mut self, p: Part) -> Outcome {
        if !self.enabled(p) {
            return Outcome::None;
        }
        match p {
            Part::Close | Part::CloseBox => Outcome::Close,
            Part::Auto => {
                self.scale = match self.scale {
                    Scale::Auto => Scale::Fixed(self.current),
                    Scale::Fixed(_) => Scale::Auto,
                };
                Outcome::Scale(self.scale)
            }
            Part::Smaller | Part::Bigger => {
                self.scale = Scale::Fixed(step(self.shown(), p == Part::Bigger));
                Outcome::Scale(self.scale)
            }
            Part::Enhanced => {
                self.enhanced = !self.enhanced;
                Outcome::Enhanced(self.enhanced)
            }
        }
    }

    /// The window in frame coordinates.
    pub(super) fn window(&self) -> IRect {
        self.window
    }

    /// The window moved by its caption, its corner to `to`.
    pub(super) fn move_to(&mut self, to: (i32, i32)) {
        self.window = self.window.offset(to.0 - self.window.x0, to.1 - self.window.y0);
    }

    /// Whether `p` (frame coordinates) is on the window.
    #[must_use]
    pub fn contains(&self, p: (i32, i32)) -> bool {
        self.window.contains(p.0, p.1)
    }

    pub fn mouse_down(&mut self, assets: &Assets, p: (i32, i32)) {
        let l = self.layout(assets);
        self.pressed = PARTS.into_iter().find(|&k| self.rect(&l, k).contains(p.0, p.1));
    }

    /// The mouse went up at `p`: the pressed part acts if it's still under
    /// it.
    pub fn mouse_up(&mut self, assets: &Assets, p: (i32, i32)) -> Outcome {
        let l = self.layout(assets);
        match self.pressed.take() {
            Some(k) if self.rect(&l, k).contains(p.0, p.1) => self.act(k),
            _ => Outcome::None,
        }
    }

    /// A key: Enter and Esc close, A and E are the check boxes, − and +
    /// the scale.
    pub fn key(&mut self, vk: u16) -> Outcome {
        let part = match vk {
            0x0d | 0x1b => Part::Close,
            0x41 => Part::Auto,
            0x45 => Part::Enhanced,
            0x6d | 0xbd => Part::Smaller,
            0x6b | 0xbb => Part::Bigger,
            _ => return Outcome::None,
        };
        self.act(part)
    }

    pub fn draw(&self, s: &mut Surface, assets: &Assets) {
        let Some(font) = ui::font(assets) else { return };
        let l = self.layout(assets);
        let pressed = |k: Part| self.pressed == Some(k);
        draw_window(s, font, self.window, "Settings", pressed(Part::CloseBox));
        // The group box: an etched frame with its title on it.
        let g = l.group;
        ui::edge(s, g, ui::SHADOW, ui::LIGHT);
        ui::edge(s, g.inset(1), ui::LIGHT, ui::SHADOW);
        let title = "UI scale";
        let tr = IRect::sized(
            g.x0 + 8,
            g.y0 - i32::from(font.height) / 2,
            ui::width(font, title) + 4,
            i32::from(font.height),
        );
        s.fill(tr, ui::FACE);
        ui::label(s, font, tr.x0 + 2, tr.y0, title, ui::BLACK);
        let auto = self.scale == Scale::Auto;
        check(s, font, l.auto, "&Automatic (the display's)", auto, pressed(Part::Auto));
        for (k, r, text) in [(Part::Smaller, l.smaller, "-"), (Part::Bigger, l.bigger, "+")] {
            ui::button(s, font, r, text, pressed(k), false, self.enabled(k));
        }
        s.fill(l.value, ui::WHITE);
        ui::sunken(s, l.value);
        let v = label(self.shown());
        let tx = l.value.x0 + (l.value.width() - ui::width(font, &v)) / 2;
        let ty = l.value.y0 + (l.value.height() - i32::from(font.height)) / 2;
        if auto {
            ui::label(s, font, tx, ty, &v, ui::SHADOW);
        } else {
            ui::label(s, font, tx, ty, &v, ui::BLACK);
        }
        check(
            s,
            font,
            l.enhanced,
            "&Enhancements",
            self.enhanced,
            pressed(Part::Enhanced),
        );
        let mut y = l.enhanced.y1 + 2;
        for line in Self::note(assets) {
            ui::label(s, font, l.enhanced.x0 + 17, y, &line, ui::BLACK);
            y += i32::from(font.height);
        }
        ui::button(s, font, l.close, "Close", pressed(Part::Close), true, true);
    }
}

struct Layout {
    group: IRect,
    auto: IRect,
    smaller: IRect,
    value: IRect,
    bigger: IRect,
    enhanced: IRect,
    close: IRect,
}

/// A check box with its label, as the dialogs draw theirs.
fn check(s: &mut Surface, font: &formats::fnt::Font, r: IRect, text: &str, on: bool, pressed: bool) {
    let b = IRect::sized(r.x0, r.y0 + (r.height() - 13) / 2, 13, 13);
    s.fill(b, if pressed { ui::FACE } else { ui::WHITE });
    ui::sunken(s, b);
    if on {
        check_mark(s, b.x0 + 3, b.y0 + 3);
    }
    let ty = r.y0 + (r.height() - i32::from(font.height)) / 2;
    ui::label(s, font, r.x0 + 17, ty, text, ui::BLACK);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps() {
        assert!((step(1.0, true) - 1.25).abs() < f64::EPSILON);
        assert!((step(1.0, false) - 1.0).abs() < f64::EPSILON);
        assert!((step(4.0, true) - 4.0).abs() < f64::EPSILON);
        // An odd scale snaps to the steps.
        assert!((step(1.6, true) - 1.75).abs() < f64::EPSILON);
        assert_eq!(label(1.5), "1.5×");
        assert_eq!(label(2.0), "2×");
    }

    #[test]
    fn keys_and_switches() {
        let mut b = SettingsBox {
            scale: Scale::Auto,
            current: 2.0,
            enhanced: true,
            window: IRect::default(),
            pressed: None,
        };
        // + from Automatic starts from what it gives now.
        assert_eq!(b.key(0x6b), Outcome::Scale(Scale::Fixed(2.25)));
        assert_eq!(b.key(0x41), Outcome::Scale(Scale::Auto));
        assert_eq!(b.key(0x41), Outcome::Scale(Scale::Fixed(2.0)));
        assert_eq!(b.key(0x45), Outcome::Enhanced(false));
        b.scale = Scale::Fixed(1.0);
        assert_eq!(b.key(0x6d), Outcome::None, "− is greyed at 1×");
        assert_eq!(b.key(0x1b), Outcome::Close);
    }
}
