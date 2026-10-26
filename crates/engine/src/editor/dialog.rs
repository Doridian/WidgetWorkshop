//! The gadgets' settings dialogs (`cDialog`, docs/editor-interaction.md 6)
//! and the Fit Work Area question (dialog 10014), drawn from their
//! `RT_DIALOG` templates in `WIDGET.EXE` in Windows 95's style: radio
//! groups, check boxes, edit fields, drop-down lists, the gadget's
//! picture and Doc's balloon (`cDialogDocItem`, docs/editor-layout.md
//! 7.4).
//!
//! The settings block maps onto the controls by kind and order
//! (`0x41ea4d` fills, `0x41e851` reads back): the radio groups in the order
//! they're made, the drop-down lists as the four numbers (their selection),
//! the check boxes as the 32 bits, the edit fields as the texts.

use formats::dialog::{Class, Dialog as Template};
use formats::pe::RT_DIALOG;

use crate::assets::{Assets, Module, rt};
use crate::gadget::{DrawCx, Gadget, Settings};
use crate::gfx::{IRect, Pictures, Surface};

use super::menu::BAR_H;
use super::ui;

/// What a dialog's input left it as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Result {
    Open,
    Ok,
    Cancel,
}

/// Fit Work Area's sizes (dialog 10014, radio buttons 101..105).
pub const FIT_SIZES: [(i32, i32); 5] = [(640, 480), (720, 540), (800, 600), (1024, 768), (1280, 1024)];

/// Size `i` of [`FIT_SIZES`].
#[must_use]
pub fn fit_size(i: usize) -> (i32, i32) {
    FIT_SIZES.get(i).copied().unwrap_or(FIT_SIZES[0])
}

const CAPTION_H: i32 = 18;
const FRAME: i32 = 3;
/// The Doc balloon (103), its button (150), the picture (102).
const DOC: u16 = 103;
const DOC_BUTTON: u16 = 150;
const PIC: u16 = 102;
/// Doc's face (sheet 505: 1/2 the description, 3/4 the tip; even pressed).
const DOC_FACE: u32 = 505;

#[derive(Debug, Clone, PartialEq)]
enum Ctl {
    Push,
    Check,
    Radio(usize),
    /// Owner-drawn (the picture, Doc, its button, a bar).
    Owner,
    /// `SS_*` style.
    Static(u32),
    Edit {
        numeric: bool,
        minus: bool,
        dot: bool,
        max: usize,
        multiline: bool,
    },
    Combo {
        items: Vec<String>,
        sel: Option<usize>,
        /// The list's height when dropped (pixels).
        list_h: i32,
    },
    Other,
}

#[derive(Debug, Clone)]
struct Control {
    id: u16,
    /// In the dialog's client coordinates.
    rect: IRect,
    text: String,
    ctl: Ctl,
    checked: bool,
}

/// Doc's balloon: the two texts and which shows.
#[derive(Debug, Clone)]
struct Doc {
    texts: [String; 2],
    page: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Settings,
    Fit,
}

/// An open dialog.
#[derive(Debug, Clone)]
pub struct Dialog {
    /// The gadget whose settings these are.
    pub gadget: Option<usize>,
    kind: Kind,
    title: String,
    /// The window in frame coordinates, and its client area.
    window: IRect,
    client: IRect,
    controls: Vec<Control>,
    /// The edit field with the caret.
    focus: Option<usize>,
    /// The button the mouse holds.
    pressed: Option<usize>,
    /// The drop-down list open.
    dropped: Option<usize>,
    /// The picture's sheet (`Pic%d`), else the gadget drawn live.
    pic_sheet: Option<u32>,
    doc: Option<Doc>,
    long_text: bool,
    /// A control changed since the last preview.
    pub changed: bool,
    /// Fit's answer: Yes.
    yes: bool,
    /// The settings as they were when it opened.
    original: Settings,
}

/// The dialog base units of the template's font (`GetDialogBaseUnits` for
/// MS Sans Serif 8: the average letter width and the height).
fn base_units(assets: &Assets) -> (i32, i32) {
    match ui::font(assets) {
        Some(f) => {
            let w = ui::width(f, "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz");
            ((w / 26 + 1) / 2, i32::from(f.height))
        }
        None => (6, 13),
    }
}

fn mul_div(a: i32, b: i32, c: i32) -> i32 {
    let n = i64::from(a) * i64::from(b);
    let c = i64::from(c);
    let r = if n >= 0 { (n + c / 2) / c } else { (n - c / 2) / c };
    i32::try_from(r).unwrap_or(0)
}

impl Dialog {
    /// Builds the dialog of `template` (`0x41acb9`), placed as the
    /// template says relative to the frame's client area.
    fn build(assets: &Assets, t: &Template, kind: Kind) -> Dialog {
        let (bx, by) = base_units(assets);
        let px = |x: i16, y: i16| (mul_div(i32::from(x), bx, 4), mul_div(i32::from(y), by, 8));
        let (cw, ch) = px(t.cx, t.cy);
        let (x, y) = px(t.x, t.y);
        let window = IRect::sized(x, y + BAR_H, cw + 2 * FRAME, ch + 2 * FRAME + CAPTION_H);
        let client = IRect::sized(window.x0 + FRAME, window.y0 + FRAME + CAPTION_H, cw, ch);
        let mut groups: Vec<char> = Vec::new();
        let mut items: Vec<_> = t.items.iter().collect();
        if kind == Kind::Settings {
            // Wrapped in id order, 1 to 0xa9.
            items.retain(|i| (1..=0xa9).contains(&i.id));
            items.sort_by_key(|i| i.id);
        }
        let mut controls = Vec::new();
        for it in items {
            if !it.visible() {
                continue;
            }
            let (x0, y0) = px(it.x, it.y);
            let (w, h) = px(it.cx, it.cy);
            let rect = IRect::sized(x0, y0, w, h);
            let mut text = it.title.text().to_owned();
            let ctl = match it.class() {
                Class::Button => match it.style & 0xf {
                    0 | 1 => Ctl::Push,
                    2 | 3 => Ctl::Check,
                    4 | 9 => {
                        // Grouped by the caption's last character, which is
                        // cut off (the Fit dialog's radios are one group).
                        let key = if kind == Kind::Settings {
                            text.pop().unwrap_or(' ')
                        } else {
                            ' '
                        };
                        let g = groups.iter().position(|&k| k == key).unwrap_or_else(|| {
                            groups.push(key);
                            groups.len() - 1
                        });
                        Ctl::Radio(g)
                    }
                    0xb => Ctl::Owner,
                    _ => Ctl::Other,
                },
                Class::Static => Ctl::Static(it.style & 0x1f),
                Class::Edit => {
                    // The caption says what it takes (`0x419c1a`): `#`
                    // digits, `-` a sign, `.` a point, a number the length.
                    let mut max = 0usize;
                    for c in text.chars() {
                        if let Some(d) = c.to_digit(10) {
                            max = max * 10 + d as usize;
                        }
                    }
                    let e = Ctl::Edit {
                        numeric: text.contains('#'),
                        minus: text.contains('-'),
                        dot: text.contains('.'),
                        max,
                        multiline: it.style & 4 != 0,
                    };
                    text.clear();
                    e
                }
                Class::ComboBox => Ctl::Combo {
                    items: Vec::new(),
                    sel: None,
                    list_h: h,
                },
                _ => Ctl::Other,
            };
            let rect = match ctl {
                // A drop-down list is as high as one line closed.
                Ctl::Combo { .. } => IRect::sized(x0, y0, w, by + 8),
                _ => rect,
            };
            controls.push(Control {
                id: it.id,
                rect,
                text,
                ctl,
                checked: false,
            });
        }
        let focus = controls.iter().position(|c| matches!(c.ctl, Ctl::Edit { .. }));
        Dialog {
            gadget: None,
            kind,
            title: t.title.clone(),
            window,
            client,
            controls,
            focus,
            pressed: None,
            dropped: None,
            pic_sheet: None,
            doc: None,
            long_text: false,
            changed: false,
            yes: false,
            original: Settings::default(),
        }
    }

    /// The settings dialog of gadget `g` (template = its gadget id).
    #[must_use]
    pub fn settings(assets: &Assets, _pictures: &Pictures, gadget: &Gadget, g: usize) -> Option<Dialog> {
        let id = u32::from(gadget.core.gadget_id);
        let t = Template::parse(assets.get(Module::Exe, RT_DIALOG, id)?).ok()?;
        let mut d = Dialog::build(assets, &t, Kind::Settings);
        d.gadget = Some(g);
        d.long_text = gadget.core.long_text;
        d.original = gadget.core.settings.clone();
        // The picture: `Pic%d` names its sheet.
        if let Some(c) = d.controls.iter().find(|c| c.id == PIC) {
            d.pic_sheet = c.text.strip_prefix("Pic").and_then(|n| n.parse().ok());
        }
        // The lists' entries (type 1022 of the dialog's id, `0x41abc2`).
        if let Some(data) = assets.get(Module::Exe, rt::COMBO, id) {
            for (cid, s) in formats::tables::strings(data) {
                if let Some(Ctl::Combo { items, .. }) = d.controls.iter_mut().find(|c| c.id == cid).map(|c| &mut c.ctl)
                {
                    items.push(s);
                }
            }
        }
        // Doc's texts (type 1021 of the gadget id: words 4 and 6).
        if d.controls.iter().any(|c| c.id == DOC)
            && let Some(info) = assets
                .get(Module::Gadget, rt::INFO, id)
                .and_then(formats::tables::gadget_info)
        {
            let text = |n: u16| assets.text(Module::Gadget, u32::from(n)).unwrap_or_default();
            d.doc = Some(Doc {
                texts: [text(info.dialog), text(info.doc)],
                page: 0,
            });
        }
        d.fill(&gadget.core.settings);
        Some(d)
    }

    /// Dialog 10014, "squeeze to screen size", with 640×480 chosen.
    #[must_use]
    pub fn fit(assets: &Assets) -> Option<Dialog> {
        let t = Template::parse(assets.get(Module::Exe, RT_DIALOG, 10014)?).ok()?;
        let mut d = Dialog::build(assets, &t, Kind::Fit);
        if let Some(c) = d.controls.iter_mut().find(|c| c.id == 101) {
            c.checked = true;
        }
        // Centred over the frame, as the message boxes are.
        Some(d)
    }

    /// Fit's chosen size, if the answer was Yes.
    #[must_use]
    pub fn fit_choice(&self) -> Option<(i32, i32)> {
        if self.kind != Kind::Fit || !self.yes {
            return None;
        }
        let i = self
            .controls
            .iter()
            .filter(|c| matches!(c.ctl, Ctl::Radio(_)))
            .position(|c| c.checked)
            .unwrap_or(0);
        Some(fit_size(i))
    }

    fn of_kind<'a>(&'a self, f: impl Fn(&Ctl) -> bool + 'a) -> impl Iterator<Item = usize> + 'a {
        self.controls
            .iter()
            .enumerate()
            .filter(move |(_, c)| f(&c.ctl))
            .map(|(i, _)| i)
    }

    fn group(&self, g: usize) -> Vec<usize> {
        self.of_kind(move |c| *c == Ctl::Radio(g)).collect()
    }

    fn groups(&self) -> usize {
        self.controls
            .iter()
            .filter_map(|c| match c.ctl {
                Ctl::Radio(g) => Some(g + 1),
                _ => None,
            })
            .max()
            .unwrap_or(0)
    }

    /// Fills the controls from `s` (the base slot 0x48, `0x41ea4d`).
    fn fill(&mut self, s: &Settings) {
        for g in 0..self.groups().min(4) {
            let members = self.group(g);
            let v = usize::from(s.radios[g]);
            for (k, &i) in members.iter().enumerate() {
                self.controls[i].checked = k == v.min(members.len().saturating_sub(1));
            }
        }
        let combos: Vec<usize> = self.of_kind(|c| matches!(c, Ctl::Combo { .. })).collect();
        for (n, &i) in combos.iter().take(4).enumerate() {
            if let Ctl::Combo { items, sel, .. } = &mut self.controls[i].ctl {
                let v = usize::try_from(s.numbers[n] & 0xffff).unwrap_or(usize::MAX);
                *sel = (v < items.len()).then_some(v);
            }
        }
        let checks: Vec<usize> = self.of_kind(|c| *c == Ctl::Check).collect();
        for (n, &i) in checks.iter().take(32).enumerate() {
            self.controls[i].checked = s.check(u32::try_from(n).unwrap_or(0));
        }
        let edits: Vec<usize> = self.of_kind(|c| matches!(c, Ctl::Edit { .. })).collect();
        for (n, &i) in edits.iter().take(4).enumerate() {
            let t = if self.long_text {
                if n > 0 {
                    break;
                }
                s.long_text()
            } else {
                s.label(n)
            };
            self.controls[i].text = crate::text::decode(t);
        }
    }

    /// Reads the controls into `s` (`0x41e851`).
    fn read(&self, s: &mut Settings) {
        for g in 0..self.groups().min(4) {
            if let Some(k) = self.group(g).iter().position(|&i| self.controls[i].checked) {
                s.radios[g] = u8::try_from(k).unwrap_or(0);
            }
        }
        let combos: Vec<usize> = self.of_kind(|c| matches!(c, Ctl::Combo { .. })).collect();
        for (n, &i) in combos.iter().take(4).enumerate() {
            if let Ctl::Combo { sel, .. } = &self.controls[i].ctl {
                // `CB_GETCURSEL`, −1 (none) as 0xffff.
                s.numbers[n] = sel.map_or(0xffff, |v| u32::try_from(v).unwrap_or(0) & 0xffff);
            }
        }
        let mut bits = 0u32;
        for (n, i) in self.of_kind(|c| *c == Ctl::Check).take(32).enumerate() {
            if self.controls[i].checked {
                bits |= 1 << n;
            }
        }
        s.checks = bits;
        let edits: Vec<usize> = self.of_kind(|c| matches!(c, Ctl::Edit { .. })).collect();
        for (n, &i) in edits.iter().take(4).enumerate() {
            let t = crate::text::encode(&self.controls[i].text);
            if self.long_text {
                if n == 0 {
                    s.set_long_text(&t);
                }
            } else {
                s.set_label(n, &t);
            }
        }
    }

    /// The live preview after a change (`0x41eb78`): the dialog read into
    /// the settings, slot 0x50, the settings put back.
    pub fn preview(&mut self, g: &mut Gadget) {
        if !self.changed {
            return;
        }
        self.changed = false;
        let saved = g.core.settings.clone();
        self.read(&mut g.core.settings);
        g.behavior.settings_changed(&mut g.core);
        g.core.settings = saved;
    }

    /// OK (`0x41ea2e`): the dialog read back into the settings.
    pub fn apply(&mut self, g: &mut Gadget) {
        self.read(&mut g.core.settings);
    }

    /// Cancel: slot 0x50 on the unchanged settings, undoing the preview.
    pub fn cancel(&mut self, g: &mut Gadget) {
        g.core.settings = self.original.clone();
        g.behavior.settings_changed(&mut g.core);
    }

    fn to_client(&self, p: (i32, i32)) -> (i32, i32) {
        (p.0 - self.client.x0, p.1 - self.client.y0)
    }

    fn close_box(&self) -> IRect {
        IRect::sized(self.window.x1 - FRAME - 2 - 16, self.window.y0 + FRAME + 2, 16, 14)
    }

    /// The open list's rectangle (client coordinates) and its items' height.
    fn list_rect(&self, i: usize, assets: &Assets) -> Option<(IRect, i32)> {
        let c = &self.controls[i];
        let Ctl::Combo { items, list_h, .. } = &c.ctl else {
            return None;
        };
        let ih = ui::font(assets).map_or(13, |f| i32::from(f.height));
        let n = i32::try_from(items.len()).unwrap_or(0);
        let h = (n * ih + 2).min((list_h - c.rect.height()).max(ih + 2));
        Some((IRect::sized(c.rect.x0, c.rect.y1, c.rect.width(), h), ih))
    }

    pub fn mouse_move(&mut self, _assets: &Assets, _p: (i32, i32)) {}

    pub fn mouse_down(&mut self, assets: &Assets, p: (i32, i32), _dbl: bool) {
        let q = self.to_client(p);
        if let Some(i) = self.dropped.take() {
            if let Some((r, ih)) = self.list_rect(i, assets)
                && r.contains(q.0, q.1)
            {
                let k = usize::try_from((q.1 - r.y0 - 1) / ih).unwrap_or(0);
                if let Ctl::Combo { items, sel, .. } = &mut self.controls[i].ctl
                    && k < items.len()
                {
                    *sel = Some(k);
                    self.changed = true;
                }
            }
            return;
        }
        if self.close_box().contains(p.0, p.1) {
            self.pressed = Some(usize::MAX);
            return;
        }
        let Some(i) = self.controls.iter().rposition(|c| c.rect.contains(q.0, q.1)) else {
            return;
        };
        match &self.controls[i].ctl {
            Ctl::Push | Ctl::Check | Ctl::Radio(_) => self.pressed = Some(i),
            Ctl::Owner if self.controls[i].id == DOC_BUTTON => self.pressed = Some(i),
            Ctl::Edit { .. } => self.focus = Some(i),
            Ctl::Combo { .. } => self.dropped = Some(i),
            _ => {}
        }
    }

    pub fn mouse_up(&mut self, _assets: &Assets, p: (i32, i32)) -> Result {
        let Some(i) = self.pressed.take() else {
            return Result::Open;
        };
        if i == usize::MAX {
            return if self.close_box().contains(p.0, p.1) {
                Result::Cancel
            } else {
                Result::Open
            };
        }
        let q = self.to_client(p);
        if !self.controls[i].rect.contains(q.0, q.1) {
            return Result::Open;
        }
        self.click(i)
    }

    /// A button clicked (`0x41a686`).
    fn click(&mut self, i: usize) -> Result {
        let id = self.controls[i].id;
        match self.controls[i].ctl {
            Ctl::Check => {
                let c = &mut self.controls[i];
                c.checked = !c.checked;
                self.changed = true;
            }
            Ctl::Radio(g) => {
                for k in self.group(g) {
                    self.controls[k].checked = k == i;
                }
                self.changed = true;
            }
            Ctl::Owner if id == DOC_BUTTON => {
                if let Some(d) = &mut self.doc {
                    d.page ^= 1;
                }
            }
            Ctl::Push => match (self.kind, id) {
                (Kind::Settings, 1) => return Result::Ok,
                (Kind::Fit, 6) => {
                    self.yes = true;
                    return Result::Ok;
                }
                (Kind::Settings, 2) | (Kind::Fit, 2 | 7) => return Result::Cancel,
                // The parts' own buttons ("Clear Text", "Record a
                // Sound"...) are the gadget's business (`+0x2f`).
                _ => {}
            },
            _ => {}
        }
        Result::Open
    }

    /// A key: typing into the focused field, Tab to the next field, Enter
    /// is the default button, Esc Cancel.
    pub fn key(&mut self, vk: u16, shift: bool) -> Result {
        if let Some(i) = self.dropped {
            if matches!(vk, 0x0d | 0x1b) {
                self.dropped = None;
            }
            let _ = i;
            return Result::Open;
        }
        let multiline = self
            .focus
            .is_some_and(|i| matches!(self.controls[i].ctl, Ctl::Edit { multiline: true, .. }));
        match vk {
            0x1b => return Result::Cancel,
            0x0d if !multiline => {
                return match self.kind {
                    Kind::Settings => Result::Ok,
                    Kind::Fit => {
                        self.yes = true;
                        Result::Ok
                    }
                };
            }
            0x09 => {
                let edits: Vec<usize> = self.of_kind(|c| matches!(c, Ctl::Edit { .. })).collect();
                if let Some(f) = self.focus
                    && let Some(k) = edits.iter().position(|&e| e == f)
                {
                    self.focus = Some(edits[(k + 1) % edits.len()]);
                }
                return Result::Open;
            }
            _ => {}
        }
        let Some(f) = self.focus else { return Result::Open };
        let Ctl::Edit {
            numeric,
            minus,
            dot,
            max,
            ..
        } = self.controls[f].ctl
        else {
            return Result::Open;
        };
        let text = &mut self.controls[f].text;
        if vk == 0x08 {
            text.pop();
            self.changed = true;
            return Result::Open;
        }
        let c = if vk == 0x0d {
            Some('\n')
        } else {
            super::prompt::typed(vk, shift)
        };
        let Some(c) = c else { return Result::Open };
        let ok = !numeric || c.is_ascii_digit() || (minus && c == '-') || (dot && c == '.');
        let limit = if max > 0 { max } else { 59 };
        if ok && text.chars().count() < limit {
            text.push(c);
            self.changed = true;
        }
        Result::Open
    }

    /// Draws the dialog; `gadget` is the one whose settings these are.
    #[allow(clippy::too_many_lines)]
    pub fn draw(&self, s: &mut Surface, assets: &Assets, pictures: &Pictures, gadget: Option<&Gadget>, lights: bool) {
        let Some(font) = ui::font(assets) else { return };
        let w = self.window;
        s.fill(w, ui::FACE);
        ui::raised(s, w);
        let cap = IRect::new(w.x0 + FRAME, w.y0 + FRAME, w.x1 - FRAME, w.y0 + FRAME + CAPTION_H);
        s.fill(cap, ui::NAVY);
        s.text(
            font,
            cap.x0 + 3,
            cap.y0 + 2,
            &crate::text::encode(&self.title),
            ui::WHITE,
        );
        let cb = self.close_box();
        ui::button(s, font, cb, "", self.pressed == Some(usize::MAX), false, true);
        // The close box's ×.
        for d in 0..6 {
            for t in 0..2 {
                s.pset(cb.x0 + 4 + d + t, cb.y0 + 3 + d, ui::BLACK);
                s.pset(cb.x0 + 9 - d + t, cb.y0 + 3 + d, ui::BLACK);
            }
        }
        let old = s.clip();
        s.set_clip(self.client.intersect(old));
        let (ox, oy) = (self.client.x0, self.client.y0);
        for (i, c) in self.controls.iter().enumerate() {
            let r = c.rect.offset(ox, oy);
            match &c.ctl {
                Ctl::Push => {
                    let default = self.kind == Kind::Settings && c.id == 1 || self.kind == Kind::Fit && c.id == 6;
                    ui::button(s, font, r, &c.text, self.pressed == Some(i), default, true);
                }
                Ctl::Check | Ctl::Radio(_) => {
                    let by = r.y0 + (r.height() - 13) / 2;
                    let b = IRect::sized(r.x0, by, 13, 13);
                    if matches!(c.ctl, Ctl::Check) {
                        s.fill(b, if self.pressed == Some(i) { ui::FACE } else { ui::WHITE });
                        ui::sunken(s, b);
                        if c.checked {
                            check_mark(s, b.x0 + 3, b.y0 + 3);
                        }
                    } else {
                        radio(s, b.x0, b.y0, c.checked, self.pressed == Some(i));
                    }
                    let ty = r.y0 + (r.height() - i32::from(font.height)) / 2;
                    ui::label(s, font, r.x0 + 17, ty, &c.text, ui::BLACK);
                }
                Ctl::Static(style) => match style {
                    // SS_BLACKRECT, SS_GRAYRECT, SS_WHITERECT.
                    4 => s.fill(r, ui::BLACK),
                    5 => s.fill(r, ui::SHADOW),
                    6 => s.fill(r, ui::WHITE),
                    // SS_BLACKFRAME, SS_GRAYFRAME, SS_WHITEFRAME.
                    7 => s.frame(r, ui::BLACK),
                    8 => s.frame(r, ui::SHADOW),
                    9 => s.frame(r, ui::WHITE),
                    _ => {
                        let inner = s.clip();
                        s.set_clip(r.intersect(inner));
                        let mut y = r.y0;
                        for line in ui::wrap(font, &c.text, r.width()) {
                            let x = match style {
                                1 => r.x0 + (r.width() - ui::width(font, &line)) / 2,
                                2 => r.x1 - ui::width(font, &line),
                                _ => r.x0,
                            };
                            ui::label(s, font, x, y, &line, ui::BLACK);
                            y += i32::from(font.height);
                        }
                        s.set_clip(inner);
                    }
                },
                Ctl::Edit { multiline, .. } => {
                    s.fill(r, ui::WHITE);
                    ui::sunken(s, r);
                    let inner = s.clip();
                    s.set_clip(r.inset(2).intersect(inner));
                    let lines = if *multiline {
                        ui::wrap(font, &c.text, r.width() - 6)
                    } else {
                        vec![c.text.clone()]
                    };
                    let mut y = r.y0 + 3;
                    let mut end = (r.x0 + 3, y);
                    for line in &lines {
                        let tw = s.text(font, r.x0 + 3, y, &crate::text::encode(line), ui::BLACK);
                        end = (r.x0 + 3 + tw, y);
                        y += i32::from(font.height);
                    }
                    if self.focus == Some(i) {
                        s.fill(IRect::sized(end.0, end.1, 1, i32::from(font.height)), ui::BLACK);
                    }
                    s.set_clip(inner);
                }
                Ctl::Combo { items, sel, .. } => {
                    s.fill(r, ui::WHITE);
                    ui::sunken(s, r);
                    let b = IRect::new(r.x1 - 2 - 16, r.y0 + 2, r.x1 - 2, r.y1 - 2);
                    ui::button(s, font, b, "", self.dropped == Some(i), false, true);
                    arrow(s, b.x0 + 4, b.y0 + (b.height() - 4) / 2);
                    if let Some(t) = sel.and_then(|k| items.get(k)) {
                        let inner = s.clip();
                        s.set_clip(IRect::new(r.x0 + 2, r.y0 + 2, b.x0, r.y1 - 2).intersect(inner));
                        let ty = r.y0 + (r.height() - i32::from(font.height)) / 2;
                        s.text(font, r.x0 + 4, ty, &crate::text::encode(t), ui::BLACK);
                        s.set_clip(inner);
                    }
                }
                Ctl::Owner => self.draw_owner(s, assets, pictures, c, r, gadget, lights),
                Ctl::Other => {}
            }
        }
        // The open list, over everything.
        if let Some(i) = self.dropped
            && let Some((lr, ih)) = self.list_rect(i, assets)
            && let Ctl::Combo { items, sel, .. } = &self.controls[i].ctl
        {
            s.set_clip(old);
            let lr = lr.offset(ox, oy);
            s.fill(lr, ui::WHITE);
            s.frame(lr, ui::BLACK);
            let inner = s.clip();
            s.set_clip(lr.inset(1).intersect(inner));
            for (k, t) in items.iter().enumerate() {
                let y = lr.y0 + 1 + i32::try_from(k).unwrap_or(0) * ih;
                let hot = *sel == Some(k);
                if hot {
                    s.fill(IRect::new(lr.x0 + 1, y, lr.x1 - 1, y + ih), ui::NAVY);
                }
                s.text(
                    font,
                    lr.x0 + 3,
                    y,
                    &crate::text::encode(t),
                    if hot { ui::WHITE } else { ui::BLACK },
                );
            }
            s.set_clip(inner);
        }
        s.set_clip(old);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_owner(
        &self,
        s: &mut Surface,
        assets: &Assets,
        pictures: &Pictures,
        c: &Control,
        r: IRect,
        gadget: Option<&Gadget>,
        lights: bool,
    ) {
        match c.id {
            PIC => {
                let inner = s.clip();
                s.set_clip(r.intersect(inner));
                if let Some(sheet) = self.pic_sheet {
                    if let Some(img) = pictures.frame(assets, Module::Gadget, sheet, 1) {
                        s.blit(&img, r.x0, r.y0);
                    }
                } else if let Some(g) = gadget {
                    // The gadget itself (`cDialogGadgetGraphic`), centred.
                    let b = g.core.bounds;
                    let (w, h) = (i32::from(b.width()), i32::from(b.height()));
                    let x = r.x0 + (r.width() - w) / 2 - i32::from(b.left);
                    let y = r.y0 + (r.height() - h) / 2 - i32::from(b.top);
                    let mut core = g.core.clone();
                    core.h = x;
                    core.v = y;
                    let mut cx = DrawCx {
                        surface: s,
                        assets,
                        pictures,
                        running: false,
                        ticks: 0,
                        lights,
                    };
                    g.behavior.draw(&core, &mut cx);
                }
                s.set_clip(inner);
            }
            DOC => {
                if let Some(d) = &self.doc {
                    draw_balloon(s, assets, r, &d.texts[d.page]);
                }
            }
            DOC_BUTTON => {
                let page = self.doc.as_ref().map_or(0, |d| d.page);
                let pressed = self
                    .pressed
                    .is_some_and(|p| self.controls.get(p).is_some_and(|x| x.id == DOC_BUTTON));
                let n = u16::try_from(page * 2 + 1 + usize::from(pressed)).unwrap_or(1);
                // Placed at the bottom right of the balloon and button's union.
                let doc_r = self
                    .controls
                    .iter()
                    .find(|x| x.id == DOC)
                    .map_or(r, |x| x.rect.offset(self.client.x0, self.client.y0));
                let u = doc_r.union(r);
                if let Some(img) = pictures.frame(assets, Module::Gadget, DOC_FACE, n) {
                    s.blit(&img, u.x1 - 33, u.y1 - 31);
                }
            }
            _ => {}
        }
    }
}

/// Doc's balloon (`0x4186f9`): a white rounded rectangle with the text,
/// in the largest of MS Sans Serif `H/9`, `H/12` that fits.
fn draw_balloon(s: &mut Surface, assets: &Assets, b: IRect, text: &str) {
    let rad = 10;
    for y in b.y0..b.y1 {
        for x in b.x0..b.x1 {
            let dx = (b.x0 + rad - x).max(x - (b.x1 - 1 - rad)).max(0);
            let dy = (b.y0 + rad - y).max(y - (b.y1 - 1 - rad)).max(0);
            if dx * dx + dy * dy <= rad * rad {
                s.pset(x, y, ui::WHITE);
            }
        }
    }
    let t = b.inset(4);
    let h = t.height();
    let sizes = [h / 9, h / 12, h / 12];
    let mut chosen = None;
    for size in sizes {
        let Some(f) = assets.font("MS Sans Serif", u16::try_from(size.max(1)).unwrap_or(13), false) else {
            continue;
        };
        let lines = ui::wrap(f, text, t.width());
        let fits = i32::try_from(lines.len()).unwrap_or(0) * i32::from(f.height) <= h;
        chosen = Some((f, lines));
        if fits {
            break;
        }
    }
    let Some((f, lines)) = chosen else { return };
    let old = s.clip();
    s.set_clip(t.intersect(old));
    let mut y = t.y0;
    for l in lines {
        s.text(f, t.x0, y, &crate::text::encode(&l), ui::BLACK);
        y += i32::from(f.height);
    }
    s.set_clip(old);
}

fn check_mark(s: &mut Surface, x: i32, y: i32) {
    const ROWS: [u8; 7] = [0x02, 0x06, 0x8e, 0xdc, 0xf8, 0x70, 0x20];
    for (dy, bits) in (0..).zip(ROWS) {
        for dx in 0..8 {
            if bits & (0x80 >> dx) != 0 {
                s.pset(x + dx, y + dy, ui::BLACK);
            }
        }
    }
}

/// A Win95 radio button (12×12): grey/black sunken circle, white inside.
fn radio(s: &mut Surface, x: i32, y: i32, on: bool, pressed: bool) {
    const OUTER_TL: [&str; 12] = [
        "....gggg....",
        "..gg....gg..",
        ".g........w.",
        ".g........w.",
        "g..........w",
        "g..........w",
        "g..........w",
        "g..........w",
        ".g........w.",
        ".g........w.",
        "..ww....ww..",
        "....wwww....",
    ];
    const INNER: [&str; 12] = [
        "............",
        "....bbbb....",
        "..bbiiiill..",
        "..biiiiiil..",
        ".biiiiiiiil.",
        ".biiiiiiiil.",
        ".biiiiiiiil.",
        ".biiiiiiiil.",
        "..biiiiiil..",
        "..lliiiill..",
        "....llll....",
        "............",
    ];
    for (dy, (o, i)) in (0..).zip(OUTER_TL.iter().zip(INNER)) {
        for (dx, (a, b)) in (0..).zip(o.bytes().zip(i.bytes())) {
            let c = match (a, b) {
                (b'g', _) => Some(ui::SHADOW),
                (b'w', _) => Some(ui::WHITE),
                (_, b'b') => Some(ui::BLACK),
                (_, b'l') => Some(ui::FACE),
                (_, b'i') => Some(if pressed { ui::FACE } else { ui::WHITE }),
                _ => None,
            };
            if let Some(c) = c {
                s.pset(x + dx, y + dy, c);
            }
        }
    }
    if on {
        s.fill(IRect::sized(x + 5, y + 4, 2, 4), ui::BLACK);
        s.fill(IRect::sized(x + 4, y + 5, 4, 2), ui::BLACK);
    }
}

/// The drop-down arrow (7×4).
fn arrow(s: &mut Surface, x: i32, y: i32) {
    for d in 0..4 {
        s.fill(IRect::new(x + d, y + d, x + 7 - d, y + d + 1), ui::BLACK);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats::dialog::{Field, Item};

    fn item(id: u16, class: u16, style: u32, title: &str) -> Item {
        Item {
            style: 0x5000_0000 | style,
            ex_style: 0,
            x: 0,
            y: i16::try_from(id).unwrap(),
            cx: 50,
            cy: 10,
            id,
            class: Field::Ordinal(class),
            title: if title.is_empty() {
                Field::None
            } else {
                Field::Text(title.into())
            },
        }
    }

    #[test]
    fn settings_map_onto_controls_by_kind_and_order() {
        let t = Template {
            style: 0,
            ex_style: 0,
            x: 0,
            y: 0,
            cx: 200,
            cy: 200,
            menu: Field::None,
            class: Field::None,
            title: "T".into(),
            font: None,
            items: vec![
                item(1, 0x80, 1, "OK"),
                item(110, 0x80, 9, "B one2"),
                item(106, 0x80, 9, "Preset:1"),
                item(107, 0x80, 9, "Custom:1"),
                item(111, 0x80, 9, "B two2"),
                item(108, 0x80, 3, "Lock"),
                item(109, 0x80, 3, "Hide"),
                item(112, 0x81, 0, "#3"),
                item(113, 0x85, 3, ""),
            ],
        };
        let assets = Assets::default();
        let mut d = Dialog::build(&assets, &t, Kind::Settings);
        assert_eq!(d.groups(), 2);
        // Groups in the order of their first member's id.
        assert_eq!(d.controls[d.group(0)[0]].text, "Preset:");
        let last = d.controls.len() - 1;
        if let Ctl::Combo { items, .. } = &mut d.controls[last].ctl {
            items.extend(["a".to_owned(), "b".to_owned()]);
        }
        let mut s = Settings {
            radios: [1, 1, 0, 0],
            checks: 0b10,
            numbers: [1, 0, 0, 0],
            ..Settings::default()
        };
        s.set_label(0, b"42");
        d.fill(&s);
        let mut back = Settings::default();
        d.read(&mut back);
        assert_eq!(back.radios[..2], [1, 1]);
        assert_eq!(back.checks, 0b10);
        assert_eq!(back.numbers[0], 1);
        assert_eq!(back.label(0), b"42");
        // The edit takes three digits only.
        let edit = d.of_kind(|c| matches!(c, Ctl::Edit { .. })).next();
        d.focus = edit;
        d.key(0x41, false);
        d.key(0x37, false);
        d.key(0x38, false);
        let mut back = Settings::default();
        d.read(&mut back);
        assert_eq!(back.label(0), b"427");
        assert!(d.changed);
        assert_eq!(d.key(0x0d, false), Result::Ok);
    }
}
