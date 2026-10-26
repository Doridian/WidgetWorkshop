//! A gadget: what every part has (`cGadget`'s fields, [`Core`]) and what
//! each class does ([`Behavior`], `cGadget`'s vtable). docs/gadget-model.md
//! has the original's layout.

use std::any::Any;
use std::fmt;

use formats::hr::Rect;
use formats::tables::{KnotDef, KnotKind};

use crate::assets::Assets;
use crate::gfx::{IRect, Pictures, Surface};
use crate::sim::Io;

/// A gadget class's four-character tag, as saved widgets name it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tag(pub [u8; 4]);

impl Tag {
    #[must_use]
    pub const fn new(s: &[u8; 4]) -> Tag {
        Tag(*s)
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&String::from_utf8_lossy(&self.0))
    }
}

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tag({self})")
    }
}

/// The settings block (`+0x35`, 0x108 bytes): what the gadget's settings
/// dialog edits, saved as is (`0x41ef34` swaps it from the Mac's order).
#[derive(Clone, PartialEq, Eq)]
pub struct Settings {
    /// 32 check boxes.
    pub checks: u32,
    /// 4 radio groups' choices.
    pub radios: [u8; 4],
    /// 4 number fields.
    pub numbers: [u32; 4],
    /// 4 texts of 60 bytes each, or one of 240 (NUL-terminated,
    /// Windows-1252).
    pub text: [u8; 240],
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            checks: 0,
            radios: [0; 4],
            numbers: [0; 4],
            text: [0; 240],
        }
    }
}

impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Settings")
            .field("checks", &format_args!("{:#x}", self.checks))
            .field("radios", &self.radios)
            .field("numbers", &self.numbers)
            .field("labels", &(0..4).map(|i| self.label_str(i)).collect::<Vec<_>>())
            .finish()
    }
}

pub const SETTINGS_SIZE: usize = 0x108;
pub const LABEL_SIZE: usize = 60;

impl Settings {
    /// Reads the block as saved (big-endian).
    #[must_use]
    pub fn from_be(b: &[u8]) -> Settings {
        let mut s = Settings::default();
        let be = |at: usize| b.get(at..at + 4).map_or(0, |x| u32::from_be_bytes([x[0], x[1], x[2], x[3]]));
        s.checks = be(0);
        for i in 0..4 {
            s.radios[i] = b.get(4 + i).copied().unwrap_or(0);
            s.numbers[i] = be(8 + 4 * i);
        }
        if let Some(t) = b.get(24..24 + 240) {
            s.text.copy_from_slice(t);
        }
        s
    }

    /// The block as saved (big-endian), 0x108 bytes.
    #[must_use]
    pub fn to_be(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(SETTINGS_SIZE);
        out.extend_from_slice(&self.checks.to_be_bytes());
        out.extend_from_slice(&self.radios);
        for n in self.numbers {
            out.extend_from_slice(&n.to_be_bytes());
        }
        out.extend_from_slice(&self.text);
        out
    }

    #[must_use]
    pub fn check(&self, i: u32) -> bool {
        self.checks & (1 << i) != 0
    }

    pub fn set_check(&mut self, i: u32, on: bool) {
        if on {
            self.checks |= 1 << i;
        } else {
            self.checks &= !(1 << i);
        }
    }

    /// Text field `i` (0 to 3), up to its NUL.
    #[must_use]
    pub fn label(&self, i: usize) -> &[u8] {
        let t = &self.text[i * LABEL_SIZE..(i + 1) * LABEL_SIZE];
        &t[..t.iter().position(|&c| c == 0).unwrap_or(LABEL_SIZE)]
    }

    #[must_use]
    pub fn label_str(&self, i: usize) -> String {
        crate::text::decode(self.label(i))
    }

    /// Sets text field `i`, cut to 59 bytes.
    pub fn set_label(&mut self, i: usize, text: &[u8]) {
        let t = &mut self.text[i * LABEL_SIZE..(i + 1) * LABEL_SIZE];
        t.fill(0);
        let n = text.len().min(LABEL_SIZE - 1);
        t[..n].copy_from_slice(&text[..n]);
    }

    /// The one long text (when the gadget keeps one instead of four).
    #[must_use]
    pub fn long_text(&self) -> &[u8] {
        &self.text[..self.text.iter().position(|&c| c == 0).unwrap_or(self.text.len())]
    }

    pub fn set_long_text(&mut self, text: &[u8]) {
        self.text.fill(0);
        let n = text.len().min(self.text.len() - 1);
        self.text[..n].copy_from_slice(&text[..n]);
    }
}

/// A knot: where a wire attaches. One wire each.
#[derive(Debug, Clone, PartialEq)]
pub struct Knot {
    pub def: KnotDef,
    /// The wire's index in the document.
    pub wire: Option<usize>,
}

impl Knot {
    #[must_use]
    pub fn is_input(&self) -> bool {
        self.def.kind == KnotKind::Input
    }
}

/// What an output gives a wire's other end (slots 0x24 to 0x38).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Value {
    /// The gadget gives nothing on this knot.
    #[default]
    None,
    Bool(bool),
    Number(f64),
    /// A sound or a picture, passed by reference.
    Object(Object),
}

impl Value {
    /// As the input reader (`0x41e56e`) turns it into a number: a boolean
    /// is 1 or 0.
    #[must_use]
    pub fn number(&self) -> Option<f64> {
        match self {
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Value::Number(n) => Some(*n),
            Value::Object(o) => Some(f64::from(o.handle)),
            Value::None => None,
        }
    }
}

/// An object travelling through a wire: a sound (or picture) by handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    pub kind: ObjectKind,
    pub handle: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Sound,
    Picture,
}

/// What every gadget has (`cGadget`).
#[derive(Debug, Clone)]
pub struct Core {
    pub tag: Tag,
    /// The id saved widgets and their wires know it by (`+0x08`).
    pub id: u32,
    /// The top left on the worksheet: `h` across, `v` down (`+0x16`,
    /// `+0x14`).
    pub h: i32,
    pub v: i32,
    /// The gadget's extent relative to `h`, `v` (`+0x18`).
    pub bounds: Rect,
    /// The gadget's number: its dialog, Doc text and info (`+0x28`).
    pub gadget_id: u16,
    /// Its picture sheet and knot table (type 1004/1014).
    pub sheet: u32,
    pub selected: bool,
    pub settings: Settings,
    /// One long text instead of four labels (`+0x34`).
    pub long_text: bool,
    pub knots: Vec<Knot>,
    /// The boolean state (`+0x13e`), what a boolean output gives.
    pub state: bool,
    /// The saved base part (0x150 bytes) as loaded, written back as is
    /// where the port doesn't model a field.
    pub raw_base: Vec<u8>,
}

impl Core {
    #[must_use]
    pub fn new(tag: Tag, gadget_id: u16, sheet: u32) -> Core {
        Core {
            tag,
            id: 0,
            h: 0,
            v: 0,
            bounds: Rect::default(),
            gadget_id,
            sheet,
            selected: false,
            settings: Settings::default(),
            long_text: false,
            knots: Vec::new(),
            state: false,
            raw_base: Vec::new(),
        }
    }

    /// The gadget's rectangle on the worksheet.
    #[must_use]
    pub fn rect(&self) -> IRect {
        IRect::from_mac(self.bounds, self.h, self.v)
    }

    /// Knot `i`'s point on the worksheet.
    #[must_use]
    pub fn knot_at(&self, i: usize) -> Option<(i32, i32)> {
        let k = self.knots.get(i)?;
        Some((self.h + i32::from(k.def.at.h), self.v + i32::from(k.def.at.v)))
    }
}

/// What a gadget's drawing gets: where to draw and with what.
pub struct DrawCx<'a> {
    pub surface: &'a mut Surface,
    pub assets: &'a Assets,
    pub pictures: &'a Pictures,
    /// Whether the widget is running.
    pub running: bool,
    /// The clock, in Mac ticks (1/60 s).
    pub ticks: u32,
}

impl DrawCx<'_> {
    /// Frame `n` of the gadget's sheet `sheet` at `x`, `y` on the worksheet.
    pub fn frame(&mut self, sheet: u32, n: u16, x: i32, y: i32) {
        if let Some(img) = self.pictures.frame(self.assets, crate::assets::Module::Gadget, sheet, n) {
            self.surface.blit(&img, x, y);
        }
    }

    /// Frame `n` placed by its own rectangle relative to `x`, `y`.
    pub fn frame_at_rect(&mut self, sheet: u32, n: u16, x: i32, y: i32) {
        let m = crate::assets::Module::Gadget;
        if let (Some(img), Some(r)) =
            (self.pictures.frame(self.assets, m, sheet, n), self.pictures.rect(self.assets, m, sheet, n))
        {
            self.surface.blit(&img, x + i32::from(r.left), y + i32::from(r.top));
        }
    }
}

/// A gadget class's behaviour: the slots of `cGadget`'s vtable the class
/// overrides. Every method has the base class's behaviour as its default.
pub trait Behavior: Any {
    /// Slot 0x04: draws the gadget. The default draws frame 1 of its sheet.
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, 1, core.h, core.v);
    }

    /// Slot 0x44: one step of the running widget.
    fn tick(&mut self, _core: &mut Core, _io: &mut Io<'_>) {}

    /// Slot 0x14: a click on the running gadget at `x`, `y` relative to its
    /// top left.
    fn click(&mut self, _core: &mut Core, _io: &mut Io<'_>, _x: i32, _y: i32) {}

    /// Slot 0x1c: back to the state before running.
    fn reset(&mut self, core: &mut Core) {
        core.state = false;
    }

    /// Slot 0x20: the widget starts running.
    fn start(&mut self, _core: &mut Core, _io: &mut Io<'_>) {}

    /// Slots 0x24 to 0x38: what output `knot` gives.
    fn output(&self, _core: &Core, _knot: usize) -> Value {
        Value::None
    }

    /// Slot 0x50: the settings changed (after the dialog, and on load).
    fn settings_changed(&mut self, _core: &mut Core) {}

    /// The class's saved state after the 0x150-byte base (slots 0x3c,
    /// 0x40), big-endian.
    fn save(&self, _core: &Core) -> Vec<u8> {
        Vec::new()
    }

    /// Reads what [`Behavior::save`] wrote.
    fn load(&mut self, _core: &mut Core, _data: &[u8]) {}

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// A gadget on the worksheet.
pub struct Gadget {
    pub core: Core,
    pub behavior: Box<dyn Behavior>,
}

impl Gadget {
    /// The gadget's wire indexes, from its knots.
    pub fn wires(&self) -> impl Iterator<Item = usize> + '_ {
        self.core.knots.iter().filter_map(|k| k.wire)
    }
}

/// A class the port doesn't know: drawn from its sheet, does nothing, and
/// its saved data is kept as it was.
#[derive(Debug, Default)]
pub struct Unknown {
    pub tail: Vec<u8>,
}

impl Behavior for Unknown {
    fn save(&self, _core: &Core) -> Vec<u8> {
        self.tail.clone()
    }

    fn load(&mut self, _core: &mut Core, data: &[u8]) {
        self.tail = data.to_vec();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// The knots of table `sheet` (type 1014).
#[must_use]
pub fn knots(assets: &Assets, sheet: u32) -> Vec<Knot> {
    assets
        .get(crate::assets::Module::Gadget, crate::assets::rt::KNOTS, sheet)
        .map(formats::tables::knots)
        .unwrap_or_default()
        .into_iter()
        .map(|def| Knot { def, wire: None })
        .collect()
}
