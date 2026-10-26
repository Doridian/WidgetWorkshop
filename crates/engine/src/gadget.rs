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
        let be = |at: usize| {
            b.get(at..at + 4)
                .map_or(0, |x| u32::from_be_bytes([x[0], x[1], x[2], x[3]]))
        };
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
    /// A sound or a picture, passed by reference (slot 0x38's pointer);
    /// `None` is the null pointer of a silent part.
    Object(Option<Object>),
}

impl Value {
    /// As the input reader (`0x41e56e`) turns a wired source's output into
    /// a number: a boolean or an object is 1 or 0, nothing is -1.
    #[must_use]
    pub fn number(&self) -> f64 {
        match self {
            Value::Bool(b) => f64::from(u8::from(*b)),
            Value::Number(n) => *n,
            Value::Object(o) => f64::from(u8::from(o.is_some())),
            Value::None => -1.0,
        }
    }

    /// The sound, if it's one.
    #[must_use]
    pub fn sound(&self) -> Option<&SoundData> {
        match self {
            Value::Object(Some(Object::Sound(s))) => Some(s),
            _ => None,
        }
    }
}

/// An object travelling through a wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Object {
    /// A sound descriptor (`{u32 len, u16 rate, u8* data}`, at `+0x197`
    /// in the parts that have one).
    Sound(SoundData),
    /// A picture, by the sheet it's frame 1 of.
    Picture(u32),
}

/// A sound: 8-bit unsigned mono samples, to play at `rate` Hz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundData {
    pub rate: u16,
    pub samples: std::sync::Arc<[u8]>,
}

/// The "no value" a cached input starts at (`0xADDC4A11FF61B241`).
pub const NONE: f64 = f64::from_bits(0xADDC_4A11_FF61_B241);

/// What every gadget has (`cGadget`).
#[allow(clippy::struct_excessive_bools)]
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
    /// Locked (`+0x0c`, saved at 0x110): a puzzle's part, which can't be
    /// selected, moved, opened or deleted.
    pub locked: bool,
    pub settings: Settings,
    /// One long text instead of four labels (`+0x34`).
    pub long_text: bool,
    pub knots: Vec<Knot>,
    /// The boolean state (`+0x13e`), what a boolean output gives.
    pub state: bool,
    /// The number (`+0x146`), what a number output gives.
    pub number: f64,
    /// The object (`+0x14e`), what an object output gives.
    pub object: Option<Object>,
    /// Broken (`+0x140`): drawn with the broken picture until reset.
    pub broken: bool,
    /// An animation phase classes keep (`+0x141`), zeroed by reset.
    pub phase: u8,
    /// Whether a run starts with every input's cached value "none" (`+0x13d`),
    /// so the first look sees them changed; otherwise it caches what they are.
    pub empty_cache: bool,
    /// The inputs' last values (`+0x156`), for [`Io::value_changed`].
    pub cache: [f64; 8],
    /// When [`Io::throttle`] last passed (`+0x152`), in run ticks.
    pub throttled_at: u32,
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
            locked: false,
            settings: Settings::default(),
            long_text: false,
            knots: Vec::new(),
            state: false,
            number: 0.0,
            object: None,
            broken: false,
            phase: 0,
            empty_cache: false,
            cache: [NONE; 8],
            throttled_at: 0,
            raw_base: Vec::new(),
        }
    }

    /// The base class's reset (`0x41cef4`): the outputs cleared, not broken.
    pub fn base_reset(&mut self) {
        self.state = false;
        self.number = 0.0;
        self.object = None;
        self.phase = 0;
        self.broken = false;
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
        if let Some(img) = self
            .pictures
            .frame(self.assets, crate::assets::Module::Gadget, sheet, n)
        {
            self.surface.blit(&img, x, y);
        }
    }

    /// Frame `n` placed by its own rectangle relative to `x`, `y`.
    pub fn frame_at_rect(&mut self, sheet: u32, n: u16, x: i32, y: i32) {
        let m = crate::assets::Module::Gadget;
        if let (Some(img), Some(r)) = (
            self.pictures.frame(self.assets, m, sheet, n),
            self.pictures.rect(self.assets, m, sheet, n),
        ) {
            self.surface.blit(&img, x + i32::from(r.left), y + i32::from(r.top));
        }
    }

    /// Point list `id` (type 1010 in `WWGADGET.DLL`, `0x41141f`).
    #[must_use]
    pub fn points(&self, id: u32) -> Vec<formats::tables::Point> {
        self.assets
            .get(crate::assets::Module::Gadget, crate::assets::rt::POINTS, id)
            .map(formats::tables::points)
            .unwrap_or_default()
    }

    /// The palette index nearest `rgb`, as Windows maps a pen or text
    /// colour on the 8-bit display.
    #[must_use]
    pub fn color(&self, rgb: [u8; 3]) -> u8 {
        nearest(&self.assets.palette(), rgb)
    }

    /// `text` (Windows-1252) in the bitmap font `face` nearest `height`
    /// pixels, top left at `x`, `y`; returns its width (0 without the font).
    #[allow(clippy::too_many_arguments)]
    pub fn text(&mut self, face: &str, height: u16, bold: bool, x: i32, y: i32, text: &[u8], c: u8) -> i32 {
        match self.assets.font(face, height, bold) {
            Some(f) => self.surface.text(f, x, y, text, c),
            None => 0,
        }
    }

    /// The width `text` would take in that font.
    #[must_use]
    pub fn text_width(&self, face: &str, height: u16, bold: bool, text: &[u8]) -> i32 {
        self.assets
            .font(face, height, bold)
            .map_or(0, |f| i32::try_from(f.width(text)).unwrap_or(0))
    }
}

/// The index of the colour in `palette` nearest `rgb`.
#[must_use]
pub fn nearest(palette: &[[u8; 3]; 256], rgb: [u8; 3]) -> u8 {
    let d = |p: &[u8; 3]| -> u32 { p.iter().zip(rgb).map(|(&a, b)| u32::from(a.abs_diff(b)).pow(2)).sum() };
    let i = (0..256).min_by_key(|&i| d(&palette[i])).unwrap_or(0);
    u8::try_from(i).unwrap_or(0)
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

    /// Slot 0x1c: back to the state before running (on creation and when
    /// a run stops).
    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
    }

    /// Slot 0x20: the widget starts running.
    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
    }

    /// Slots 0x24/0x28: whether output `knot` gives a boolean, and which.
    fn has_bool(&self, _core: &Core, _knot: usize) -> bool {
        false
    }

    fn bool_out(&self, core: &Core, _knot: usize) -> bool {
        core.state
    }

    /// Slots 0x2c/0x30: whether output `knot` gives a number, and which.
    fn has_number(&self, _core: &Core, _knot: usize) -> bool {
        false
    }

    fn number_out(&self, core: &Core, _knot: usize) -> f64 {
        core.number
    }

    /// Slots 0x34/0x38: whether output `knot` gives an object, and which.
    fn has_object(&self, _core: &Core, _knot: usize) -> bool {
        false
    }

    fn object_out(&self, core: &Core, _knot: usize) -> Option<Object> {
        core.object.clone()
    }

    /// What output `knot` gives, as a reader asks (`0x41e56e`): an object
    /// first, then a number, then a boolean. Not overridden.
    fn output(&self, core: &Core, knot: usize) -> Value {
        if self.has_object(core, knot) {
            Value::Object(self.object_out(core, knot))
        } else if self.has_number(core, knot) {
            Value::Number(self.number_out(core, knot))
        } else if self.has_bool(core, knot) {
            Value::Bool(self.bool_out(core, knot))
        } else {
            Value::None
        }
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
