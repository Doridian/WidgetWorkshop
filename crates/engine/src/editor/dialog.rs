//! The gadgets' settings dialogs (`cDialog`, docs/editor-interaction.md 6)
//! and the squeeze questions (Fit Work Area's dialog 10014, the save's
//! Mac size check 10010), drawn from their
//! `RT_DIALOG` templates in `WIDGET.EXE` in Windows 95's style: radio
//! groups, check boxes, edit fields, drop-down lists, the gadget's
//! picture and Doc's balloon (`cDialogDocItem`, docs/editor-layout.md
//! 7.4).
//!
//! The settings block maps onto the controls by kind and order
//! (`0x41ea4d` fills, `0x41e851` reads back): the radio groups in the order
//! they're made, the drop-down lists as the four numbers (their selection),
//! the check boxes as the 32 bits, the edit fields as the texts.
//!
//! On top of that (docs/dialogs.md): the links the classes' slot 0x48
//! makes (working a control selects another, `0x417c7b`), the edit
//! fields' validation (`0x419ba9`), the parts' own push buttons ("Clear
//! Text", "Reset Number Range", the Recorder's two) and the Recorder's
//! progress bar (control 160).

use formats::dialog::{Class, Dialog as Template};
use formats::pe::RT_DIALOG;

use crate::assets::{Assets, Module, rt};
use crate::gadget::{DrawCx, Gadget, Settings, Tag};
use crate::gfx::{IRect, Pictures, Surface};

use super::menu::BAR_H;
use super::ui;

/// What a dialog's input left it as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Result {
    Open,
    Ok,
    Cancel,
    /// The Recorder's "Record a Sound" (`0x459db3`): the editor records.
    Record,
    /// Its "Import .WAV file" (`0x459fe8`): the editor asks for the file.
    Import,
}

/// Fit Work Area's sizes (dialog 10014, radio buttons 101..105).
pub const FIT_SIZES: [(i32, i32); 5] = [(640, 480), (720, 540), (800, 600), (1024, 768), (1280, 1024)];

/// Size `i` of [`FIT_SIZES`].
#[must_use]
pub fn fit_size(i: usize) -> (i32, i32) {
    FIT_SIZES.get(i).copied().unwrap_or(FIT_SIZES[0])
}

pub(super) const CAPTION_H: i32 = 18;
pub(super) const FRAME: i32 = 3;
/// The Doc balloon (103), its button (150), the picture (102).
const DOC: u16 = 103;
const DOC_BUTTON: u16 = 150;
const PIC: u16 = 102;
/// The Recorder's progress bar (owner-drawn, `0x41a912`).
const PROGRESS: u16 = 160;
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
        filter: Filter,
        multiline: bool,
        /// `ES_CENTER`.
        center: bool,
        /// The text as last validated (`cEditText+0x114`).
        last_good: String,
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
    /// The control working this one selects (`+0x0a`, `0x417c7b`).
    link: Option<usize>,
    /// `EnableWindow(FALSE)` (only the Recorder's error does it).
    disabled: bool,
}

/// What an edit field takes, from its template text (`0x419c1a`): digits
/// the most characters, `#` a whole number, `.` a real one, `-` a sign,
/// `A` letters and digits.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Filter {
    max: usize,
    number: bool,
    real: bool,
    minus: bool,
    alnum: bool,
}

impl Filter {
    fn parse(t: &str) -> Filter {
        let mut f = Filter::default();
        for c in t.chars() {
            match c {
                '0'..='9' => f.max = f.max * 10 + c.to_digit(10).map_or(0, |d| d as usize),
                '#' => f.number = true,
                '.' => f.real = true,
                '-' => f.minus = true,
                'A' => f.alnum = true,
                _ => {}
            }
        }
        f
    }

    /// `0x419ad8`: whether `t` may stand.
    fn ok(self, t: &str) -> bool {
        let r = if self.alnum {
            // `ctype & 0xe`: digits, upper and lower case.
            t.chars().all(|c| c.is_ascii_alphanumeric())
        } else if self.real {
            // `0x419904`, `0x419828`.
            t.is_empty() || t == "." || (self.minus && t == "-") || scan_real(t)
        } else if self.number {
            // `0x4199e1`, `0x419a59`.
            t.is_empty() || (self.minus && t == "-") || scan_int(t).is_some_and(|v| self.minus || v >= 0)
        } else {
            true
        };
        r && (self.max == 0 || crate::text::encode(t).len() <= self.max)
    }
}

/// `sscanf(t, "%d%c") == 1`: a whole number and nothing after it.
fn scan_int(t: &str) -> Option<i64> {
    let s = t.trim_start_matches([' ', '\t', '\n', '\r']);
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let v: i64 = digits.parse().unwrap_or(i64::MAX);
    Some(if s.starts_with('-') { -v } else { v })
}

/// The real numbers' check: one trailing `.`, `e` or `E` dropped, then
/// `sscanf(t, "%lf%c") == 1`.
fn scan_real(t: &str) -> bool {
    let t = t.strip_suffix(['.', 'e', 'E']).unwrap_or(t);
    let s = t.trim_start_matches([' ', '\t', '\n', '\r']).as_bytes();
    let mut i = usize::from(matches!(s.first(), Some(b'+' | b'-')));
    let digits = |i: &mut usize| {
        let at = *i;
        while s.get(*i).is_some_and(u8::is_ascii_digit) {
            *i += 1;
        }
        *i - at
    };
    let mut n = digits(&mut i);
    if s.get(i) == Some(&b'.') {
        i += 1;
        n += digits(&mut i);
    }
    if n == 0 {
        return false;
    }
    if matches!(s.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(s.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        if digits(&mut i) == 0 {
            return false;
        }
    }
    i == s.len()
}

/// A control by kind and number (docs/dialogs.md 1.1): what a link joins.
#[derive(Debug, Clone, Copy)]
enum At {
    Check(usize),
    Combo(usize),
    Edit(usize),
    /// Button `k` of radio group `g`.
    Radio(usize, usize),
}

const PRESET: At = At::Radio(0, 0);
const CUSTOM: At = At::Radio(0, 1);

/// The links a class's slot 0x48 makes (docs/dialogs.md 3): working the
/// first control selects the second.
fn links(tag: Tag) -> &'static [(At, At)] {
    match &tag.0 {
        // `0x44f393`, `0x45cca9`.
        b"Pend" | b"Fork" => &[(At::Combo(0), PRESET), (At::Edit(0), CUSTOM)],
        // `0x44706c`: the height list, not the gravity or the thing.
        b"Drop" => &[(At::Combo(1), PRESET), (At::Edit(0), CUSTOM)],
        // `0x44fe21`: the height list, not the speed or the gravity.
        b"Cano" => &[(At::Combo(2), PRESET), (At::Edit(0), CUSTOM)],
        // `0x450cdb`, `0x45262d`: from and to.
        b"rand" | b"Sldr" => &[(At::Combo(0), PRESET), (At::Edit(0), CUSTOM), (At::Edit(1), CUSTOM)],
        // `0x459615`: the hours; From and To aren't linked.
        b"StCh" => &[(At::Combo(0), PRESET), (At::Edit(2), CUSTOM)],
        // `0x457e95`: a comparison or the number selects "Win / Lose if
        // input is:", True or False "Win if input is:".
        b"Stop" => &[
            (At::Radio(2, 0), CUSTOM),
            (At::Radio(2, 1), CUSTOM),
            (At::Radio(2, 2), CUSTOM),
            (At::Radio(2, 3), CUSTOM),
            (At::Radio(2, 4), CUSTOM),
            (At::Radio(2, 5), CUSTOM),
            (At::Radio(1, 0), PRESET),
            (At::Radio(1, 1), PRESET),
            (At::Edit(0), CUSTOM),
        ],
        // `0x4536ea`: the pitch list ticks "Change Pitch:", which (like
        // "Backwards") selects "Transform Sound:".
        b"SdTr" => &[
            (At::Combo(0), At::Check(0)),
            (At::Check(0), CUSTOM),
            (At::Check(1), CUSTOM),
        ],
        _ => &[],
    }
}

/// A part's push-button handler (`+0x2f`, docs/dialogs.md 1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Handler {
    /// Edits `0..n` emptied: "Clear Text" (`0x45a7e8`, `0x45b645`), "Clear
    /// All Text" (`0x45b6e0`).
    Clear(usize),
    /// "Reset Number Range" (`0x458b74`): From "0", To "100".
    ResetRange,
    Record,
    Import,
}

/// The handler of push button `n` (counting OK and Cancel) of class `tag`.
fn handler(tag: Tag, n: usize) -> Option<Handler> {
    Some(match (&tag.0, n) {
        (b"TxBx" | b"TxLb", 2) => Handler::Clear(1),
        (b"TxLn", 2) => Handler::Clear(4),
        (b"StCh", 2) => Handler::ResetRange,
        // The init's (`0x45a057`) and the fill's (`0x45a72e`).
        (b"Tape", 2) => Handler::Record,
        (b"Tape", 3) => Handler::Import,
        _ => return None,
    })
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
    /// Dialog 10010 (`CWrksFitDialog`).
    MacFit,
}

/// An open dialog.
#[derive(Debug, Clone)]
pub struct Dialog {
    /// The gadget whose settings these are.
    pub gadget: Option<usize>,
    /// Its class, for the links and push buttons.
    tag: Option<Tag>,
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
    /// Fit's answer: Yes (No and Cancel leave it).
    yes: bool,
    /// The settings as they were when it opened.
    original: Settings,
    /// The progress bar's percentage (`dlg+0x9c`).
    progress: u16,
}

/// The dialog base units of the template's font (`GetDialogBaseUnits` for
/// MS Sans Serif 8: the average letter width and the height).
pub(super) fn base_units(assets: &Assets) -> (i32, i32) {
    match ui::font(assets) {
        Some(f) => {
            let w = ui::width(f, "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz");
            ((w / 26 + 1) / 2, i32::from(f.height))
        }
        None => (6, 13),
    }
}

pub(super) fn mul_div(a: i32, b: i32, c: i32) -> i32 {
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
                    // The caption says what it takes (`0x419c1a`).
                    let e = Ctl::Edit {
                        filter: Filter::parse(&text),
                        multiline: it.style & 4 != 0,
                        center: it.style & 3 == 1,
                        last_good: String::new(),
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
                link: None,
                disabled: false,
            });
        }
        let focus = controls.iter().position(|c| matches!(c.ctl, Ctl::Edit { .. }));
        Dialog {
            gadget: None,
            tag: None,
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
            progress: 0,
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
        d.tag = Some(gadget.core.tag);
        d.link(gadget.core.tag);
        d.fill(&gadget.core.settings);
        Some(d)
    }

    /// Dialog 10014, "squeeze to screen size", with size `chosen` (the
    /// profile's `FitSize`, `0x429ad6`; none past the five).
    #[must_use]
    pub fn fit(assets: &Assets, chosen: Option<usize>) -> Option<Dialog> {
        let t = Template::parse(assets.get(Module::Exe, RT_DIALOG, 10014)?).ok()?;
        let mut d = Dialog::build(assets, &t, Kind::Fit);
        if let Some(i) = chosen
            && let Some(c) = d
                .controls
                .iter_mut()
                .find(|c| c.id == 101 + u16::try_from(i).unwrap_or(0))
        {
            c.checked = true;
        }
        Some(d)
    }

    /// Dialog 10010, the save's "too big to share with friends who have
    /// the Macintosh version" (`0x42932b`), "Remember my answer" clear.
    #[must_use]
    pub fn mac_fit(assets: &Assets) -> Option<Dialog> {
        let t = Template::parse(assets.get(Module::Exe, RT_DIALOG, 10010)?).ok()?;
        Some(Dialog::build(assets, &t, Kind::MacFit))
    }

    /// Whether this is 10014 or 10010 (a squeeze question).
    #[must_use]
    pub fn is_fit(&self) -> bool {
        self.kind != Kind::Settings
    }

    /// Whether this is 10010.
    #[must_use]
    pub fn is_mac_fit(&self) -> bool {
        self.kind == Kind::MacFit
    }

    /// A squeeze question's answer was Yes.
    #[must_use]
    pub fn said_yes(&self) -> bool {
        self.yes
    }

    /// 10014's size chosen (radio buttons 101..105).
    #[must_use]
    pub fn fit_choice(&self) -> Option<usize> {
        if self.kind != Kind::Fit {
            return None;
        }
        self.controls
            .iter()
            .filter(|c| matches!(c.ctl, Ctl::Radio(_)))
            .position(|c| c.checked)
    }

    /// 10010's "Remember my answer" (check box 13).
    #[must_use]
    pub fn remember(&self) -> bool {
        self.controls.iter().any(|c| c.id == 13 && c.checked)
    }

    /// Yes on a squeeze question: 10014 takes it only with a size chosen
    /// (`0x4298b9` falls through without one).
    fn fit_yes(&mut self) -> Result {
        if self.kind == Kind::Fit && self.fit_choice().is_none() {
            return Result::Open;
        }
        self.yes = true;
        Result::Ok
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

    /// The `n`-th control of a kind, in id order (`0x41c27b`, `0x41c25b`,
    /// `0x41c1f9`; a group's button `0x41c23b` + `0x41a396`).
    fn at(&self, a: At) -> Option<usize> {
        match a {
            At::Check(n) => self.of_kind(|c| *c == Ctl::Check).nth(n),
            At::Combo(n) => self.of_kind(|c| matches!(c, Ctl::Combo { .. })).nth(n),
            At::Edit(n) => self.of_kind(|c| matches!(c, Ctl::Edit { .. })).nth(n),
            At::Radio(g, k) => self.group(g).get(k).copied(),
        }
    }

    /// Class `tag`'s slot 0x48 links (`0x417c7b`).
    fn link(&mut self, tag: Tag) {
        for &(from, to) in links(tag) {
            if let (Some(i), Some(j)) = (self.at(from), self.at(to)) {
                self.controls[i].link = Some(j);
            }
        }
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
            let c = &mut self.controls[i];
            c.text = crate::text::decode(t);
            // The original leaves it unset (docs/dialogs.md 6).
            if let Ctl::Edit { last_good, .. } = &mut c.ctl {
                last_good.clone_from(&c.text);
            }
        }
    }

    /// Reads the controls into `s` (`0x41e851`); `font` wraps a multi-line
    /// field as it shows.
    fn read(&self, s: &mut Settings, font: Option<&formats::fnt::Font>) {
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
            let c = &self.controls[i];
            let t = match (&c.ctl, font) {
                (Ctl::Edit { multiline: true, .. }, Some(f)) => {
                    crate::text::encode(&hardened(f, &c.text, c.rect.width() - 6))
                }
                _ => crate::text::encode(&c.text),
            };
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
    pub fn preview(&mut self, g: &mut Gadget, assets: &Assets) {
        if !self.changed {
            return;
        }
        self.changed = false;
        let saved = g.core.settings.clone();
        self.read(&mut g.core.settings, ui::font(assets));
        g.behavior.settings_changed(&mut g.core);
        g.core.settings = saved;
    }

    /// OK (`0x41ea2e`): the dialog read back into the settings.
    pub fn apply(&mut self, g: &mut Gadget, assets: &Assets) {
        self.read(&mut g.core.settings, ui::font(assets));
    }

    /// Control `id`'s centre in frame coordinates (for the tests' clicks).
    #[cfg(test)]
    pub(super) fn centre_of(&self, id: u16) -> Option<(i32, i32)> {
        let c = self.controls.iter().find(|c| c.id == id)?;
        let r = c.rect.offset(self.client.x0, self.client.y0);
        Some((i32::midpoint(r.x0, r.x1), i32::midpoint(r.y0, r.y1)))
    }

    /// `0x41caca`: the progress bar at `pct` percent.
    pub fn set_progress(&mut self, pct: u16) {
        self.progress = pct;
    }

    /// The progress bar's percentage.
    #[must_use]
    pub fn progress(&self) -> u16 {
        self.progress
    }

    /// `0x41c3ea`: push button `n` (OK is 0) disabled.
    pub fn disable_button(&mut self, n: usize) {
        let i = self.of_kind(|c| *c == Ctl::Push).nth(n);
        if let Some(i) = i {
            self.controls[i].disabled = true;
        }
    }

    /// Whether push button `n` is enabled.
    #[must_use]
    pub fn button_enabled(&self, n: usize) -> bool {
        self.of_kind(|c| *c == Ctl::Push)
            .nth(n)
            .is_some_and(|i| !self.controls[i].disabled)
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
        close_box(self.window)
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
        if self.controls[i].disabled {
            return;
        }
        match &self.controls[i].ctl {
            Ctl::Push | Ctl::Check | Ctl::Radio(_) => self.pressed = Some(i),
            Ctl::Owner if self.controls[i].id == DOC_BUTTON => self.pressed = Some(i),
            Ctl::Edit { .. } => self.focus = Some(i),
            Ctl::Combo { .. } => {
                // `CBN_SETFOCUS`: the links, before anything is chosen.
                self.dropped = Some(i);
                self.follow(i);
            }
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

    /// Radio button `i` selected, the others of its group cleared (`0x41a0be`).
    fn select(&mut self, i: usize) {
        if let Ctl::Radio(g) = self.controls[i].ctl {
            for k in self.group(g) {
                self.controls[k].checked = k == i;
            }
        }
    }

    /// `OnCommand`'s walk (`0x41a686`) from control `src`, just worked: a
    /// push button runs its handler, then each link in turn is selected (a
    /// check box checked, a radio button chosen in its group, an edit field
    /// focused), the source itself left as it is. A radio button links as
    /// its group, through the button chosen in it (`0x41a3b6`). Then the
    /// preview (`+0x2b`).
    fn follow(&mut self, src: usize) -> Result {
        let mut r = Result::Open;
        let mut c = Some(src);
        // The tables have no cycles; a bound all the same.
        for step in 0..=self.controls.len() {
            let Some(i) = c else { break };
            let first = step == 0;
            match self.controls[i].ctl {
                Ctl::Check if !first => self.controls[i].checked = true,
                Ctl::Radio(_) if !first => self.select(i),
                Ctl::Push => r = self.run_handler(i),
                Ctl::Edit { .. } => self.focus = Some(i),
                _ => {}
            }
            c = match self.controls[i].ctl {
                Ctl::Radio(g) => self
                    .group(g)
                    .into_iter()
                    .find(|&k| self.controls[k].checked)
                    .and_then(|k| self.controls[k].link),
                _ => self.controls[i].link,
            };
        }
        self.changed = true;
        r
    }

    /// Push button `i`'s handler (`0x417dc3`); none does nothing.
    fn run_handler(&mut self, i: usize) -> Result {
        let n = self.of_kind(|c| *c == Ctl::Push).position(|k| k == i).unwrap_or(0);
        match self.tag.and_then(|t| handler(t, n)) {
            Some(Handler::Clear(k)) => {
                for e in 0..k {
                    self.set_edit(e, "");
                }
            }
            Some(Handler::ResetRange) => {
                self.set_edit(0, "0");
                self.set_edit(1, "100");
            }
            Some(Handler::Record) => return Result::Record,
            Some(Handler::Import) => return Result::Import,
            None => {}
        }
        Result::Open
    }

    /// `0x41c728`: edit `n`'s text set (`WM_SETTEXT`), which is a change
    /// like typing; no such field does nothing.
    fn set_edit(&mut self, n: usize, text: &str) {
        if let Some(i) = self.at(At::Edit(n)) {
            text.clone_into(&mut self.controls[i].text);
            self.edit_changed(i);
        }
    }

    /// `EN_CHANGE` on edit `i`: validated (`0x419ba9`: kept if it passes,
    /// else the last good text back), then its links.
    fn edit_changed(&mut self, i: usize) {
        let c = &mut self.controls[i];
        if let Ctl::Edit { filter, last_good, .. } = &mut c.ctl {
            if filter.ok(&c.text) {
                last_good.clone_from(&c.text);
            } else {
                c.text.clone_from(last_good);
            }
        }
        self.follow(i);
    }

    /// A button clicked (`0x41a686`).
    fn click(&mut self, i: usize) -> Result {
        let id = self.controls[i].id;
        match self.controls[i].ctl {
            Ctl::Check => {
                let c = &mut self.controls[i];
                c.checked = !c.checked;
                return self.follow(i);
            }
            Ctl::Radio(_) => {
                self.select(i);
                return self.follow(i);
            }
            Ctl::Owner if id == DOC_BUTTON => {
                if let Some(d) = &mut self.doc {
                    d.page ^= 1;
                }
            }
            Ctl::Push => match (self.kind, id) {
                // A squeeze question's No: on with the save, unsqueezed
                // (`0x429591` returns `answer != IDCANCEL`).
                (Kind::Settings, 1) | (Kind::Fit | Kind::MacFit, 7) => return Result::Ok,
                (Kind::Fit | Kind::MacFit, 6) => return self.fit_yes(),
                (_, 2) => return Result::Cancel,
                // The parts' own buttons ("Clear Text", "Record a
                // Sound"...): the handler (`+0x2f`).
                (Kind::Settings, _) => return self.follow(i),
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
                    Kind::Fit | Kind::MacFit => self.fit_yes(),
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
        let Ctl::Edit { filter, .. } = self.controls[f].ctl else {
            return Result::Open;
        };
        let text = &mut self.controls[f].text;
        if vk == 0x08 {
            if text.ends_with("\r\n") {
                text.pop();
            }
            if text.pop().is_none() {
                return Result::Open;
            }
        } else {
            let c = if vk == 0x0d {
                // A multi-line field's Enter (`ES_WANTRETURN`).
                "\r\n".to_owned()
            } else {
                let Some(c) = super::prompt::typed(vk, shift) else {
                    return Result::Open;
                };
                c.to_string()
            };
            // Unfiltered, the field holds what the settings block can.
            let limit = if self.long_text { 239 } else { 59 };
            if filter.max == 0 && text.len() + c.len() > limit {
                return Result::Open;
            }
            text.push_str(&c);
        }
        self.edit_changed(f);
        Result::Open
    }

    /// Draws the dialog; `gadget` is the one whose settings these are.
    #[allow(clippy::too_many_lines)]
    pub fn draw(&self, s: &mut Surface, assets: &Assets, pictures: &Pictures, gadget: Option<&Gadget>, lights: bool) {
        let Some(font) = ui::font(assets) else { return };
        draw_window(s, font, self.window, &self.title, self.pressed == Some(usize::MAX));
        let old = s.clip();
        s.set_clip(self.client.intersect(old));
        let (ox, oy) = (self.client.x0, self.client.y0);
        for (i, c) in self.controls.iter().enumerate() {
            let r = c.rect.offset(ox, oy);
            match &c.ctl {
                Ctl::Push => {
                    let default = self.kind == Kind::Settings && c.id == 1 || self.kind != Kind::Settings && c.id == 6;
                    ui::button(s, font, r, &c.text, self.pressed == Some(i), default, !c.disabled);
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
                Ctl::Edit { multiline, center, .. } => {
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
                        let x = if *center && *multiline {
                            r.x0 + (r.width() - ui::width(font, line)) / 2
                        } else {
                            r.x0 + 3
                        };
                        let tw = s.text(font, x, y, &crate::text::encode(line), ui::BLACK);
                        end = (x + tw, y);
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
            PROGRESS => {
                // Split at `MulDiv(pct, right, 100)`: blue done, yellow to go.
                let pal = assets.palette();
                let x = r.x0 + mul_div(i32::from(self.progress), r.width(), 100).min(r.width());
                if self.progress != 0 {
                    s.fill(
                        IRect::new(r.x0, r.y0, x, r.y1),
                        crate::gadget::nearest(&pal, [0, 0, 255]),
                    );
                }
                if self.progress < 100 {
                    s.fill(
                        IRect::new(x, r.y0, r.x1, r.y1),
                        crate::gadget::nearest(&pal, [255, 255, 0]),
                    );
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

/// `text` as read back with `EM_FMTLINES` (`0x41a49b`, `0x41a44e`): each
/// soft line break of a field `w` pixels wide becomes a hard `"\r\n"`
/// after the space it broke at.
fn hardened(font: &formats::fnt::Font, text: &str, w: i32) -> String {
    text.split("\r\n")
        .map(|para| {
            if para.contains('\n') {
                para.to_owned()
            } else {
                ui::wrap(font, para, w).join(" \r\n")
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// The close box of a dialog window at `window` (frame coordinates).
pub(super) fn close_box(window: IRect) -> IRect {
    IRect::sized(window.x1 - FRAME - 2 - 16, window.y0 + FRAME + 2, 16, 14)
}

/// A dialog's window: the raised frame, the caption with `title` and the
/// close box (held down if `close_pressed`).
pub(super) fn draw_window(s: &mut Surface, font: &formats::fnt::Font, w: IRect, title: &str, close_pressed: bool) {
    s.fill(w, ui::FACE);
    ui::raised(s, w);
    let cap = IRect::new(w.x0 + FRAME, w.y0 + FRAME, w.x1 - FRAME, w.y0 + FRAME + CAPTION_H);
    s.fill(cap, ui::NAVY);
    s.text(font, cap.x0 + 3, cap.y0 + 2, &crate::text::encode(title), ui::WHITE);
    let cb = close_box(w);
    ui::button(s, font, cb, "", close_pressed, false, true);
    // The close box's ×.
    for d in 0..6 {
        for t in 0..2 {
            s.pset(cb.x0 + 4 + d + t, cb.y0 + 3 + d, ui::BLACK);
            s.pset(cb.x0 + 9 - d + t, cb.y0 + 3 + d, ui::BLACK);
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
pub(super) fn arrow(s: &mut Surface, x: i32, y: i32) {
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
        d.read(&mut back, None);
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
        d.read(&mut back, None);
        assert_eq!(back.label(0), b"427");
        assert!(d.changed);
        assert_eq!(d.key(0x0d, false), Result::Ok);
    }

    #[test]
    fn edit_filters() {
        let f = Filter::parse("5#");
        assert_eq!((f.max, f.number, f.real, f.minus), (5, true, false, false));
        assert!(f.ok("") && f.ok("123") && f.ok(" 12") && f.ok("-0"));
        assert!(!f.ok("12a") && !f.ok("-") && !f.ok("-1") && !f.ok("123456") && !f.ok("1 "));
        let f = Filter::parse("#.-6");
        assert!(f.ok("-") && f.ok(".") && f.ok("-1.5") && f.ok("1e") && f.ok("1.e") && f.ok("2E-3"));
        assert!(!f.ok("-.") && !f.ok("1e-") && !f.ok("1x") && !f.ok("-1.5e10"));
        // Without `-`, a minus can't come first but a whole number can.
        let f = Filter::parse("3.#");
        assert!(f.ok("-5") && !f.ok("-"));
        let f = Filter::parse("#-");
        assert!(f.ok("-") && f.ok("-12") && !f.ok("1.5"));
        let f = Filter::parse("A");
        assert!(f.ok("ab1") && !f.ok("a b"));
        // The Cannon's "#5 ", the Slider's "5", the Label's "30".
        assert_eq!(Filter::parse("#5 ").max, 5);
        assert!(Filter::parse("5").ok("abcde") && !Filter::parse("5").ok("abcdef"));
        assert!(Filter::parse("").ok("anything at all"));
    }

    fn assets() -> Option<Assets> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../orig");
        if !root.exists() {
            eprintln!("no orig: skipped");
            return None;
        }
        Assets::load(Some(&crate::assets::Dir(root)), None).ok()
    }

    fn open(assets: &Assets, tag: [u8; 4]) -> Dialog {
        let g = crate::doc::new_gadget(assets, Tag(tag), 1, 100, 100);
        Dialog::settings(assets, &Pictures::default(), &g, 0).unwrap()
    }

    fn ix(d: &Dialog, id: u16) -> usize {
        d.controls.iter().position(|c| c.id == id).unwrap()
    }

    fn on(d: &Dialog, id: u16) -> bool {
        d.controls[ix(d, id)].checked
    }

    fn text(d: &Dialog, id: u16) -> &str {
        &d.controls[ix(d, id)].text
    }

    /// Works control `id` with the mouse: a click, the list opened (and
    /// closed again), or the field focused and `1` typed.
    fn work(d: &mut Dialog, a: &Assets, id: u16) -> Result {
        let p = d.centre_of(id).unwrap();
        let q = d.to_client(p);
        assert_eq!(
            d.controls.iter().rposition(|c| c.rect.contains(q.0, q.1)),
            Some(ix(d, id))
        );
        d.mouse_down(a, p, false);
        match d.controls[ix(d, id)].ctl {
            Ctl::Combo { .. } => {
                d.dropped = None;
                Result::Open
            }
            Ctl::Edit { .. } => d.key(0x31, false),
            _ => d.mouse_up(a, p),
        }
    }

    #[test]
    fn links_select_per_class() {
        let Some(a) = assets() else { return };
        // (class, [(clicked first, worked, then on)]).
        let cases: [(&[u8; 4], &[(u16, u16, u16)]); 8] = [
            (
                b"Pend",
                &[(114, 107, 106), (106, 115, 114), (114, 109, 114), (114, 111, 114)],
            ),
            (
                b"Drop",
                &[(112, 109, 108), (108, 113, 112), (112, 107, 112), (112, 111, 112)],
            ),
            (
                b"Cano",
                &[(112, 111, 110), (110, 113, 112), (112, 107, 112), (112, 109, 112)],
            ),
            (b"rand", &[(109, 108, 107), (107, 111, 109), (107, 113, 109)]),
            (b"Fork", &[(107, 109, 106), (106, 110, 107)]),
            (b"Sldr", &[(109, 108, 107), (107, 111, 109), (107, 113, 109)]),
            (
                b"StCh",
                &[(112, 113, 111), (111, 114, 112), (111, 108, 111), (111, 109, 111)],
            ),
            (
                b"Stop",
                &[
                    (105, 109, 106),
                    (105, 110, 106),
                    (105, 114, 106),
                    (106, 107, 105),
                    (106, 108, 105),
                    (105, 115, 106),
                    (105, 117, 105),
                    (106, 118, 106),
                ],
            ),
        ];
        for (tag, steps) in cases {
            let mut d = open(&a, *tag);
            for &(first, worked, then) in steps {
                work(&mut d, &a, first);
                assert!(on(&d, first));
                assert_eq!(work(&mut d, &a, worked), Result::Open);
                assert!(on(&d, then), "{}: {worked} selects {then}", Tag(*tag));
                assert!(d.changed);
            }
        }
        // The Sound Transformer: the pitch list ticks "Change Pitch:",
        // which selects "Transform Sound:"; either box selects it, checked
        // or not.
        let mut d = open(&a, *b"SdTr");
        if on(&d, 108) {
            work(&mut d, &a, 108);
        }
        work(&mut d, &a, 106);
        assert!(!on(&d, 108) && on(&d, 106));
        work(&mut d, &a, 109);
        assert!(on(&d, 108) && on(&d, 107));
        for _ in 0..2 {
            work(&mut d, &a, 106);
            let was = on(&d, 110);
            work(&mut d, &a, 110);
            assert!(on(&d, 110) != was && on(&d, 107));
        }
        work(&mut d, &a, 106);
        assert!(on(&d, 108), "nothing unticks the boxes");
    }

    #[test]
    fn typing_reverts_what_the_filter_refuses() {
        let Some(a) = assets() else { return };
        let mut d = open(&a, *b"Pend");
        work(&mut d, &a, 115);
        while !text(&d, 115).is_empty() {
            d.key(0x08, false);
        }
        for vk in [0x31, 0x32, 0x41, 0xbd, 0x33, 0x34, 0x35, 0x36] {
            d.key(vk, false);
        }
        assert_eq!(text(&d, 115), "12345");
        // A signed real number field: a minus first, an exponent.
        let mut d = open(&a, *b"Stop");
        work(&mut d, &a, 115);
        while !text(&d, 115).is_empty() {
            d.key(0x08, false);
        }
        for vk in [0xbd, 0xbe, 0x31, 0xbe, 0x45, 0x35, 0x36] {
            d.key(vk, false);
        }
        assert_eq!(text(&d, 115), "-1.e56");
        d.key(0x37, false);
        assert_eq!(text(&d, 115), "-1.e56");
        // Read back as typed.
        let mut s = Settings::default();
        d.read(&mut s, ui::font(&a));
        assert_eq!(s.label(0), b"-1.e56");
    }

    #[test]
    fn push_buttons_run_their_handlers() {
        let Some(a) = assets() else { return };
        // Reset Number Range: From and To only; it's a change like typing.
        let mut d = open(&a, *b"StCh");
        work(&mut d, &a, 112);
        work(&mut d, &a, 108);
        work(&mut d, &a, 109);
        work(&mut d, &a, 114);
        let hours = text(&d, 114).to_owned();
        work(&mut d, &a, 111);
        d.changed = false;
        assert_eq!(work(&mut d, &a, 115), Result::Open);
        assert_eq!((text(&d, 108), text(&d, 109), text(&d, 114)), ("0", "100", &*hours));
        assert!(on(&d, 111) && d.changed);
        assert_eq!(d.focus, Some(ix(&d, 109)));
        // Clear Text, Clear All Text.
        for (tag, button, edits) in [
            (b"TxBx", 107, &[106][..]),
            (b"TxLb", 109, &[106]),
            (b"TxLn", 113, &[106, 108, 110, 112]),
        ] {
            let mut d = open(&a, *tag);
            for &e in edits {
                work(&mut d, &a, e);
            }
            work(&mut d, &a, button);
            assert!(edits.iter().all(|&e| text(&d, e).is_empty()), "{}", Tag(*tag));
        }
        // The Recorder's two; a disabled button does nothing.
        let mut d = open(&a, *b"Tape");
        assert_eq!(work(&mut d, &a, 106), Result::Record);
        assert_eq!(work(&mut d, &a, 107), Result::Import);
        d.disable_button(2);
        assert!(!d.button_enabled(2) && d.button_enabled(3));
        assert_eq!(work(&mut d, &a, 106), Result::Open);
        // The progress bar: blue then yellow (drawn with the CD's fonts).
        if ui::font(&a).is_none() {
            return;
        }
        d.set_progress(50);
        let mut s = Surface::new(800, 600);
        d.draw(&mut s, &a, &Pictures::default(), None, true);
        let r = d.controls[ix(&d, PROGRESS)].rect.offset(d.client.x0, d.client.y0);
        let pal = a.palette();
        let (blue, yellow) = (
            crate::gadget::nearest(&pal, [0, 0, 255]),
            crate::gadget::nearest(&pal, [255, 255, 0]),
        );
        assert_eq!(s.get(r.x0 + 1, r.y0 + 1), Some(blue));
        assert_eq!(s.get(r.x1 - 2, r.y0 + 1), Some(yellow));
    }

    #[test]
    fn billboard_soft_breaks_read_back_hard() {
        let Some(a) = assets() else { return };
        // The CD's fonts (not always extracted).
        let Some(f) = ui::font(&a) else { return };
        let w = ui::width(f, "aaaa bbbb");
        assert_eq!(hardened(f, "aaaa bbbb cccc\r\ndd", w), "aaaa bbbb \r\ncccc\r\ndd");
        let mut d = open(&a, *b"TxBx");
        let i = ix(&d, 106);
        let long = "word ".repeat(20);
        d.controls[i].text.clone_from(&long);
        let mut s = Settings::default();
        d.read(&mut s, Some(f));
        let back = crate::text::decode(s.long_text());
        assert!(back.contains(" \r\n"));
        assert_eq!(back.replace("\r\n", ""), long);
    }
}
