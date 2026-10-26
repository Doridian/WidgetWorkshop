//! The science lab: the Pendulum (`Pend`), the Gravity Chamber (`Drop`),
//! the Cannon (`Cano`), the Solar System (`Plnt`) and Area / Volume
//! (`ArVo`). docs/gadgets/science.md has the original's code; the physics
//! is closed-form, timed by the run clock in Mac ticks (1/60 s), in feet
//! and seconds.

use std::any::Any;

use formats::hr::Rect;

use crate::assets::Module;
use crate::gadget::{Behavior, Core, DrawCx};
use crate::sim::Io;

/// Preset heights in feet (`0x4903bc`): Mt. Everest, Mt. Vesuvius, the
/// Empire State Building, the Eiffel Tower, a T. rex, a man.
const HEIGHTS_FT: [f64; 6] = [
    f64::from_bits(0x40dc_5940_0000_0000),
    f64::from_bits(0x40b0_5e00_0000_0000),
    f64::from_bits(0x4097_0000_0000_0000),
    f64::from_bits(0x408e_c000_0000_0000),
    f64::from_bits(0x4034_0000_0000_0000),
    f64::from_bits(0x4018_0000_0000_0000),
];

/// Gravity in ft/s² (`0x4903ec`): the Sun, Jupiter, the Earth, the Moon.
const GRAVITY_FT_S2: [f64; 4] = [
    f64::from_bits(0x408c_2970_a3d7_0a3e),
    f64::from_bits(0x4052_fd38_ef34_d6a1),
    f64::from_bits(0x4040_17ae_147a_e148),
    f64::from_bits(0x4015_e2c3_c9ee_cbfc),
];

/// The cannon's launch speeds in ft/s (`0x49ab88`): Apollo 10, a rifle
/// bullet, a jet, a baseball pitch, none.
const CANNON_SPEED_FT_S: [f64; 5] = [
    f64::from_bits(0x40e1_c104_4444_4444),
    f64::from_bits(0x4096_eaaa_aaaa_aaaa),
    f64::from_bits(0x4089_b666_6666_6666),
    f64::from_bits(0x4060_8000_0000_0000),
    0.0,
];

/// 2π as the pendulum stores it (`0x44f1af`, 6.283185307178).
const TWO_PI: f64 = f64::from_bits(0x4019_21fb_5444_261e);
/// 3π/2 (`0x49aaa4`): the pendulum's phase before release.
const THREE_HALF_PI: f64 = f64::from_bits(0x4012_d97c_7f33_1c96);
/// The swing's amplitude in radians (`0x44f04f`, 36°).
const AMPLITUDE: f64 = f64::from_bits(0x3fe4_1b2f_769c_eb4c);
/// 1/2π (`0x44f117`, an x87 extended constant).
const INV_TWO_PI: f64 = 0.159_154_943_091_895_35;
/// ft/s to mph (`0x44fe15`).
const MPH: f64 = 0.681_818_181_818_181_8;

fn table(t: &[f64], i: u32) -> f64 {
    usize::try_from(i).ok().and_then(|i| t.get(i)).copied().unwrap_or(0.0)
}

/// Seconds since `since` on the run clock (`(u32)(now - since) * 1/60`).
fn seconds(io: &Io<'_>, since: u32) -> f64 {
    f64::from(io.elapsed().wrapping_sub(since)) / 60.0
}

/// `_ftol` (`0x480ec4`): truncation toward zero.
#[allow(clippy::cast_possible_truncation)]
fn trunc(x: f64) -> i32 {
    x as i32
}

/// `0x439e3d`: truncation, saturating short of the extremes.
fn trunc_sat(x: f64) -> i32 {
    if x >= 2_147_483_647.0 {
        0x7fff_fffe
    } else if x <= -2_147_483_646.0 {
        -0x7fff_fffe
    } else {
        trunc(x)
    }
}

/// What `sscanf(s, "%lf")` reads from the start of `s`, if anything.
fn scan_f64(s: &[u8]) -> Option<f64> {
    let s = std::str::from_utf8(s).ok()?.trim_start();
    let b = s.as_bytes();
    let mut end = 0;
    if matches!(b.first(), Some(b'+' | b'-')) {
        end = 1;
    }
    let digits = |from: usize| b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let int = digits(end);
    end += int;
    let mut frac = 0;
    if b.get(end) == Some(&b'.') {
        frac = digits(end + 1);
        end += 1 + frac;
    }
    if int + frac == 0 {
        return None;
    }
    if matches!(b.get(end), Some(b'e' | b'E')) {
        let mut e = end + 1;
        if matches!(b.get(e), Some(b'+' | b'-')) {
            e += 1;
        }
        let n = digits(e);
        if n > 0 {
            end = e + n;
        }
    }
    s[..end].parse().ok()
}

/// A number typed in a text field (`0x4110c3`): 0 when unreadable, and
/// kept off zero and infinity.
fn parse_num(s: &[u8]) -> f64 {
    let d = scan_f64(s).unwrap_or(0.0);
    if d > 0.0 {
        d.clamp(1e-300, 1e300)
    } else if d < 0.0 {
        d.clamp(-1e300, -1e-300)
    } else {
        d
    }
}

/// Frame `n`'s half width and height (for centring), as C divides.
fn half_size(cx: &DrawCx<'_>, sheet: u32, n: u16) -> (i32, i32) {
    cx.pictures.rect(cx.assets, Module::Gadget, sheet, n).map_or((0, 0), |r| {
        (
            (i32::from(r.right) - i32::from(r.left)) / 2,
            (i32::from(r.bottom) - i32::from(r.top)) / 2,
        )
    })
}

/// The point `i` of table `id`, as (`h`, `v`).
fn point(cx: &DrawCx<'_>, id: u32, i: usize) -> (i32, i32) {
    cx.points(id)
        .get(i)
        .map_or((0, 0), |p| (i32::from(p.h), i32::from(p.v)))
}

/// The broken picture (slot 0x10, `0x41ed9d`).
// TODO: draw the broken sheet (`DAT_004a2694`) centred once it's
// identified; frame 1 stands in for it.
fn draw_broken(core: &Core, cx: &mut DrawCx<'_>) {
    cx.frame(core.sheet, 1, core.h, core.v);
}

macro_rules! any {
    () => {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    };
}

// ---------------------------------------------------------------------------
// The Pendulum

/// The phase angle `t` seconds after release (`0x44f1b7`).
#[must_use]
pub fn pendulum_phase(t: f64, period: f64) -> f64 {
    if period == 0.0 {
        THREE_HALF_PI
    } else {
        TWO_PI * t / period + THREE_HALF_PI
    }
}

/// The period of a pendulum `length` feet long under `gravity` ft/s²
/// (`0x44f137`).
#[must_use]
pub fn pendulum_period(length: f64, gravity: f64) -> f64 {
    (length / gravity).sqrt() * TWO_PI
}

/// `0x44f0d7`: +1 for the first half of each turn of `x`, −1 for the
/// second (the sign of `sin x` for `x >= 0`).
#[must_use]
pub fn half_turn_sign(x: f64) -> f64 {
    if (x * INV_TWO_PI).fract() < 0.5 { 1.0 } else { -1.0 }
}

/// cPendulum: swings once released (input 0), with the period set by its
/// length and gravity; outputs the period, or a pulse each time it passes
/// the centre.
#[derive(Debug)]
pub struct Pendulum {
    /// The phase at the last look (`+0x197`).
    prev: f64,
    /// The phase (`+0x19f`).
    phase: f64,
    /// When it was released, in run ticks; 0 not yet (`+0x1a7`).
    released: u32,
    /// The period in seconds (`+0x1ab`).
    period: f64,
}

impl Default for Pendulum {
    fn default() -> Pendulum {
        Pendulum {
            prev: THREE_HALF_PI,
            phase: THREE_HALF_PI,
            released: 0,
            period: 0.0,
        }
    }
}

#[must_use]
pub fn pendulum() -> Box<dyn Behavior> {
    Box::new(Pendulum::default())
}

impl Behavior for Pendulum {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, 1, core.h, core.v);
        #[allow(clippy::cast_possible_truncation)]
        let a = f64::from((self.phase.sin() * AMPLITUDE) as f32);
        #[allow(clippy::cast_possible_truncation)]
        let (dx, dy) = (trunc(a.sin() * 74.0), trunc(a.cos() * 74.0));
        let (ph, pv) = point(cx, core.sheet, 0);
        let (px, py) = (core.h + ph, core.v + pv);
        let c = cx.color([255, 255, 0]);
        cx.surface.line((px, py), (px + dx, py + dy), c);
        let (hw, hh) = half_size(cx, core.sheet, 2);
        cx.frame(core.sheet, 2, px + dx - hw, py + dy - hh);
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        if !io.throttle(core, 6) {
            return;
        }
        if self.released == 0 {
            if io.number(core, 0) != 0.0 {
                self.released = io.elapsed();
            }
            return;
        }
        self.phase = pendulum_phase(seconds(io, self.released), self.period);
        let crossed = half_turn_sign(self.prev) * half_turn_sign(self.phase) < 0.0;
        match core.settings.radios[1] {
            0 => {
                core.number = self.period;
                core.state = false;
            }
            1 => {
                core.state = crossed;
                core.number = 0.0;
            }
            _ => {}
        }
        self.prev = self.phase;
        if crossed {
            io.play(1000);
            io.fire(core, 1);
        }
    }

    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        self.prev = THREE_HALF_PI;
        self.phase = THREE_HALF_PI;
        self.released = 0;
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        let s = &core.settings;
        let length = if s.radios[0] == 0 {
            table(&HEIGHTS_FT, s.numbers[0])
        } else {
            parse_num(s.label(0))
        };
        self.period = pendulum_period(length, table(&GRAVITY_FT_S2, s.numbers[1]));
    }

    fn has_bool(&self, core: &Core, _knot: usize) -> bool {
        core.settings.radios[1] != 0
    }

    fn has_number(&self, core: &Core, knot: usize) -> bool {
        !self.has_bool(core, knot)
    }

    fn init(&mut self, core: &mut Core) {
        // On at Center; 1,472 ft.
        core.settings.radios[1] = 1;
        core.settings.numbers[0] = 2;
    }

    any!();
}

// ---------------------------------------------------------------------------
// The Gravity Chamber

/// How far down the picture is, in pixels, after falling `t` seconds from
/// `height` feet under `gravity` toward a floor `floor` pixels down
/// (`0x446f00`).
#[must_use]
pub fn drop_pixels(t: f64, gravity: f64, height: f64, floor: i16) -> i32 {
    let d = t * t * gravity * 0.5;
    if height == 0.0 {
        0x7fff_ffff
    } else {
        trunc(d / height * f64::from(floor))
    }
}

/// Each object's floor, in pixels below its falling picture's start
/// (`0x499868`): feather, elephant, baseball.
const FLOOR_PX: [i16; 3] = [79, 77, 94];
/// Each object's first frame (`0x49986e`) and first point (`0x499874`).
const FRAME_BASE: [u16; 3] = [4, 10, 16];
const POINT_BASE: [usize; 3] = [1, 7, 13];

/// cDropChamber: a claw opens and drops a feather, an elephant or a
/// baseball; the output goes on when it hits the floor.
#[derive(Debug)]
pub struct DropChamber {
    /// The height in feet (`+0x197`); kept when the custom height can't
    /// be read.
    height: f64,
    /// When the fall began, in run ticks (`+0x19f`).
    fell_at: u32,
    /// How far it has fallen, in pixels (`+0x1a3`), and as last drawn
    /// (`+0x1a7`).
    y: i32,
    prev_y: i32,
}

impl Default for DropChamber {
    fn default() -> DropChamber {
        DropChamber {
            height: 0.0,
            fell_at: 0,
            y: 0,
            prev_y: 0,
        }
    }
}

#[must_use]
pub fn drop_chamber() -> Box<dyn Behavior> {
    Box::new(DropChamber::default())
}

fn thing(core: &Core) -> usize {
    usize::try_from(core.settings.numbers[2]).map_or(0, |i| i.min(2))
}

impl Behavior for DropChamber {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        let s = core.sheet;
        cx.frame(s, 1, core.h, core.v);
        let th = thing(core);
        let at = |cx: &DrawCx<'_>, i: usize| {
            let (h, v) = point(cx, s, i);
            (core.h + h, core.v + v)
        };
        if core.phase < 4 {
            let (x, y) = at(cx, POINT_BASE[th] + usize::from(core.phase));
            cx.frame(s, FRAME_BASE[th] + u16::from(core.phase), x, y);
        } else {
            let (x, y) = at(cx, 0);
            cx.frame(s, 3, x, y);
            if i32::from(FLOOR_PX[th]) == self.y {
                let (x, y) = at(cx, POINT_BASE[th] + 5);
                cx.frame(s, FRAME_BASE[th] + 5, x, y);
            } else {
                let (x, y) = at(cx, POINT_BASE[th] + 4);
                // The original adds the pixels as a 16-bit coordinate.
                #[allow(clippy::cast_possible_truncation)]
                let dy = i32::from(self.y as i16);
                cx.frame(s, FRAME_BASE[th] + 4, x, y + dy);
            }
        }
    }

    #[allow(clippy::cast_sign_loss)]
    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        match core.phase {
            0 => {
                if io.number(core, 0) != 0.0 {
                    core.phase = 1;
                }
            }
            1 => {
                if io.throttle(core, 5) {
                    core.phase = 2;
                }
            }
            2 => {
                if io.throttle(core, 5) {
                    core.phase = 3;
                    io.play(1101);
                }
            }
            3 => {
                if io.throttle(core, 5) {
                    core.phase = 4;
                    self.fell_at = io.elapsed();
                }
            }
            4 => {
                let g = table(&GRAVITY_FT_S2, core.settings.numbers[0]);
                let t = seconds(io, self.fell_at);
                let floor = FLOOR_PX[thing(core)];
                self.y = drop_pixels(t, g, self.height, floor);
                core.number = t * g;
                if self.y as u32 > self.prev_y as u32 {
                    if self.y as u32 >= i32::from(floor) as u32 {
                        self.y = i32::from(floor);
                        core.number = 0.0;
                        core.state = true;
                        io.play(1001);
                        core.phase = 5;
                        io.fire(core, 1);
                    }
                    self.prev_y = self.y;
                }
            }
            _ => {}
        }
    }

    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        self.prev_y = 0;
        self.y = 0;
        self.fell_at = 0;
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        let s = &core.settings;
        if s.radios[0] == 0 {
            self.height = table(&HEIGHTS_FT, s.numbers[1]);
        } else if let Some(h) = scan_f64(s.label(0)) {
            self.height = h;
        }
    }

    fn has_bool(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    fn init(&mut self, core: &mut Core) {
        // An elephant, on Earth, from 1,472 ft.
        core.settings.numbers = [2, 2, 1, 0];
    }

    any!();
}

// ---------------------------------------------------------------------------
// The Cannon

/// A cannon shot's flight time in seconds from `height` feet
/// (`0x44fa0d`).
#[must_use]
pub fn flight_time(height: f64, gravity: f64) -> f64 {
    (2.0 * height / gravity).sqrt()
}

/// cPhysicsCanon: fires a ball level from a height; outputs the distance,
/// the height or the speed while it flies, or goes on when it lands.
#[derive(Debug)]
pub struct PhysicsCanon {
    /// Ready to fire (`+0x13f`); cleared when the ball lands.
    armed: bool,
    /// When it fired, in run ticks; 0 not yet (`+0x197`).
    fired_at: u32,
    /// The ball's offset in pixels (`+0x19f` across, `+0x19b` down).
    ball_x: i32,
    ball_y: i32,
    /// The height in feet and the flight time in seconds (`+0x1a3`,
    /// `+0x1ab`).
    height: f64,
    flight: f64,
}

impl Default for PhysicsCanon {
    fn default() -> PhysicsCanon {
        PhysicsCanon {
            armed: true,
            fired_at: 0,
            ball_x: 0,
            ball_y: 0,
            height: 0.0,
            flight: 0.0,
        }
    }
}

#[must_use]
pub fn physics_canon() -> Box<dyn Behavior> {
    Box::new(PhysicsCanon::default())
}

impl Behavior for PhysicsCanon {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, 1, core.h, core.v);
        // The original adds the offsets as 16-bit coordinates.
        #[allow(clippy::cast_possible_truncation)]
        let (dx, dy) = (i32::from(self.ball_x as i16), i32::from(self.ball_y as i16));
        cx.frame(core.sheet, 3, core.h + 19 + dx, core.v + 5 + dy);
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        if !self.armed {
            return;
        }
        if self.fired_at == 0 {
            if io.number(core, 0) != 0.0 {
                self.fired_at = io.elapsed();
            }
            return;
        }
        let s = &core.settings;
        let g = table(&GRAVITY_FT_S2, s.numbers[1]);
        let v = table(&CANNON_SPEED_FT_S, s.numbers[0]);
        let mode = s.radios[1];
        let t = seconds(io, self.fired_at);
        let vy = g * t;
        let d = vy * t * 0.5;
        if self.height < 1e-100 || self.flight < 1e-100 {
            self.ball_y = 66;
            self.ball_x = 158;
        } else {
            self.ball_y = trunc_sat(d / self.height * 66.0);
            self.ball_x = trunc_sat(t / self.flight * 158.0);
        }
        match mode {
            0 => core.number = t * v,
            1 => core.number = self.height - d,
            3 => core.number = (v * v + vy * vy).sqrt() * MPH,
            _ => {}
        }
        if self.ball_x >= 158 {
            self.armed = false;
            self.ball_y = 66;
            self.ball_x = 158;
            io.play(1002);
            self.fired_at = 0;
            io.fire(core, 1);
            match mode {
                0 => core.number = v * self.flight,
                1 => core.number = 0.0,
                2 => core.state = true,
                3 => {
                    let vy = g * self.flight;
                    core.number = (vy * vy + v * v).sqrt() * MPH;
                }
                _ => {}
            }
        }
    }

    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        self.armed = true;
        self.ball_x = 0;
        self.ball_y = 0;
        self.fired_at = 0;
    }

    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        let s = &core.settings;
        self.height = if s.radios[0] == 0 {
            table(&HEIGHTS_FT, s.numbers[2])
        } else {
            parse_num(s.label(0))
        };
        self.flight = flight_time(self.height, table(&GRAVITY_FT_S2, s.numbers[1]));
        match s.radios[1] {
            0 | 3 => core.number = 0.0,
            1 => core.number = self.height,
            2 => core.state = false,
            _ => {}
        }
        self.armed = true;
    }

    fn has_bool(&self, core: &Core, _knot: usize) -> bool {
        core.settings.radios[1] == 2
    }

    fn has_number(&self, core: &Core, knot: usize) -> bool {
        !self.has_bool(core, knot)
    }

    fn init(&mut self, core: &mut Core) {
        // On When Hits.
        core.settings.radios[1] = 2;
    }

    any!();
}

// ---------------------------------------------------------------------------
// The Solar System

/// A planet's numbers (`0x49ac42`, 0x44 bytes each).
#[derive(Debug, Clone, Copy)]
struct PlanetData {
    name: &'static [u8],
    /// Earth hours.
    day: f64,
    /// Earth days.
    year: f64,
    /// Million miles from the Sun.
    distance: f64,
    /// Thousand mph.
    orbit_speed: f64,
    /// Miles.
    diameter: f64,
    /// Water = 1.
    density: f64,
    /// Earth = 1.
    gravity: f64,
    /// 10¹³ gigatons.
    mass: f64,
}

macro_rules! planets {
    ($($name:literal $day:literal $year:literal $dist:literal $orbit:literal $diam:literal $dens:literal $grav:literal $mass:literal;)*) => {
        const PLANETS: [PlanetData; 9] = [$(PlanetData {
            name: $name,
            day: $day,
            year: $year,
            distance: $dist,
            orbit_speed: $orbit,
            diameter: $diam,
            density: $dens,
            gravity: $grav,
            mass: $mass,
        },)*];
    };
}

planets! {
    b"MERCURY" 59.0 87.97 36.0 106.26 3032.0 5.5 0.38 0.332;
    b"VENUS" 243.0 224.0 67.0 77.62 7523.0 5.24 0.9 4.89;
    b"EARTH" 24.0 365.26 93.0 66.0 7926.0 5.52 1.0 6.04;
    b"MARS" 24.62 687.0 142.0 53.46 4420.0 3.94 0.38 0.642;
    b"JUPITER" 9.83 4332.0 483.0 28.93 88750.0 1.3 2.87 1920.0;
    b"SATURN" 10.23 10760.0 887.0 21.35 74580.0 0.7 1.32 575.0;
    b"URANUS" 10.82 30685.0 1783.0 15.09 31600.0 1.3 0.93 88.2;
    b"NEPTUNE" 15.8 60191.0 2794.0 12.03 30200.0 1.8 1.23 103.9;
    b"PLUTO" 159.32 90475.0 3660.0 10.5 1500.0 0.7 0.03 0.06;
}

/// cPlanet: shows a planet and outputs one of its numbers.
#[derive(Debug)]
pub struct Planet;

impl Default for Planet {
    fn default() -> Planet {
        Planet
    }
}

#[must_use]
pub fn planet() -> Box<dyn Behavior> {
    Box::new(Planet)
}

fn planet_index(core: &Core) -> usize {
    usize::try_from(core.settings.numbers[0]).map_or(0, |i| i.min(8))
}

/// The bezel, frame 14 of sheet 1022, is the planet's extent (`0x44feec`).
const PLANET_BOUNDS: Rect = Rect {
    top: 0,
    left: 0,
    bottom: 109,
    right: 118,
};

impl Behavior for Planet {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        let s = core.sheet;
        cx.frame(s, 14, core.h, core.v);
        let p = planet_index(core);
        let n = u16::try_from(p + 1).unwrap_or(1);
        let (hw, hh) = cx
            .pictures
            .rect(cx.assets, Module::Gadget, s, n)
            .map_or((0, 0), |r| {
                (
                    i32::from(r.right) - i32::from(r.left),
                    i32::from(r.bottom) - i32::from(r.top),
                )
            });
        cx.frame(s, n, core.h + 7 + (94 - hw) / 2, core.v + 7 + (88 - hh) / 2);
        // Small Fonts 11, green, bottom-aligned (TA_BOTTOM) at 107; centred
        // over 88 pixels, not the picture's 94.
        let name = PLANETS[p].name;
        let w = cx.text_width("Small Fonts", 11, false, name);
        let height = cx.assets.font("Small Fonts", 11, false).map_or(11, |f| i32::from(f.height));
        let c = cx.color([0, 255, 0]);
        cx.text("Small Fonts", 11, false, core.h + 9 + (88 - w) / 2, core.v + 107 - height, name, c);
    }

    fn tick(&mut self, core: &mut Core, _io: &mut Io<'_>) {
        let p = &PLANETS[planet_index(core)];
        core.number = match core.settings.numbers[1] {
            0 => p.diameter,
            1 => p.day,
            2 => p.year,
            3 => p.distance,
            4 => p.mass,
            5 => p.density,
            6 => p.gravity,
            7 => p.orbit_speed,
            _ => 0.0,
        };
    }

    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        core.bounds = PLANET_BOUNDS;
    }

    fn has_number(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    fn init(&mut self, core: &mut Core) {
        // The Earth.
        core.settings.numbers[0] = 2;
    }

    any!();
}

// ---------------------------------------------------------------------------
// Area / Volume

/// π as Area / Volume stores it (`0x441d30`), short of `f64::consts::PI`.
#[allow(clippy::approx_constant)]
const ARVO_PI: f64 = 3.141_592_653_589;
/// 4π/3 (`0x441d30`).
const FOUR_THIRDS_PI: f64 = 4.188_790_204_785_333;

/// The area or volume of a square, circle, cube or sphere of side or
/// radius `x` (`0x441d30`).
#[must_use]
pub fn area_volume(shape: u8, x: f64) -> f64 {
    match shape {
        0 => x * x,
        1 => x * x * ARVO_PI,
        2 => x * x * x,
        3 => x * x * x * FOUR_THIRDS_PI,
        _ => 0.0,
    }
}

/// cAreaVolume: crunches a new input for a moment, then outputs the area
/// or volume; a negative input breaks it.
#[derive(Debug)]
#[derive(Default)]
pub struct AreaVolume {
    /// Whether the input's wire was flagged changed when the crunching
    /// began (`+0x197`).
    wire_changed: bool,
}


#[must_use]
pub fn area_volume_gadget() -> Box<dyn Behavior> {
    Box::new(AreaVolume::default())
}

impl Behavior for AreaVolume {
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, u16::from(core.phase) + 1, core.h, core.v);
        let (h, v) = point(cx, core.sheet, 0);
        cx.frame(core.sheet, u16::from(core.settings.radios[0]) + 9, core.h + h, core.v + v);
        if core.broken {
            draw_broken(core, cx);
        }
    }

    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        if core.phase == 0 {
            let differs = io.value_changed(core, 0);
            self.wire_changed = io.changed(core, 0);
            if differs || self.wire_changed {
                core.phase = 1;
                io.play(1020);
            }
        } else if io.throttle(core, 9) {
            core.phase += 1;
            if core.phase == 8 {
                let x = io.number(core, 0);
                core.phase = 0;
                core.broken = x < 0.0;
                core.number = if core.broken { 0.0 } else { area_volume(core.settings.radios[0], x) };
                if self.wire_changed {
                    io.fire(core, 1);
                }
            }
        }
    }

    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        core.empty_cache = true;
    }

    fn has_number(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    fn init(&mut self, core: &mut Core) {
        // Volume of a sphere.
        core.settings.radios[0] = 3;
    }

    any!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::Assets;
    use crate::gadget::{Gadget, Knot, Tag, Unknown};
    use crate::sim::{Env, Wire};
    use formats::tables::{KnotDef, KnotKind, Point};

    /// A source giving a fixed number.
    struct Source(f64);

    impl Behavior for Source {
        fn has_number(&self, _core: &Core, _knot: usize) -> bool {
            true
        }

        fn number_out(&self, _core: &Core, _knot: usize) -> f64 {
            self.0
        }

        any!();
    }

    fn knot(kind: KnotKind) -> Knot {
        Knot {
            def: KnotDef {
                at: Point { v: 0, h: 0 },
                kind,
            },
            wire: None,
        }
    }

    /// A gadget under test (id 2) with input 0 wired to a source (id 1)
    /// giving `input`, and output 1 wired to nothing in particular.
    struct Bench {
        core: Core,
        gadgets: Vec<Gadget>,
        wires: Vec<Wire>,
        env: Env,
        assets: Assets,
    }

    impl Bench {
        fn new(input: f64) -> Bench {
            let mut core = Core::new(Tag::new(b"test"), 0, 0);
            core.id = 2;
            core.knots = vec![knot(KnotKind::Input), knot(KnotKind::Output)];
            core.knots[0].wire = Some(0);
            core.knots[1].wire = Some(1);
            let mut src = Core::new(Tag::new(b"src_"), 0, 0);
            src.id = 1;
            let mut me = Core::new(Tag::new(b"test"), 0, 0);
            me.id = 2;
            let wire = |from, to| Wire {
                from,
                from_knot: 0,
                to,
                to_knot: 0,
                flag: 0,
                points: Vec::new(),
                changed: false,
            };
            Bench {
                core,
                gadgets: vec![
                    Gadget {
                        core: src,
                        behavior: Box::new(Source(input)),
                    },
                    Gadget {
                        core: me,
                        behavior: Box::new(Unknown::default()),
                    },
                ],
                wires: vec![wire(0, 1), wire(1, 0)],
                env: Env::default(),
                assets: Assets::default(),
            }
        }

        fn set_input(&mut self, x: f64) {
            self.gadgets[0].behavior = Box::new(Source(x));
            self.wires[0].changed = true;
        }

        fn run<R>(&mut self, f: impl FnOnce(&mut Core, &mut Io<'_>) -> R) -> R {
            let mut io = Io {
                assets: &self.assets,
                gadgets: &self.gadgets,
                wires: &mut self.wires,
                env: &mut self.env,
            };
            f(&mut self.core, &mut io)
        }

        fn start(&mut self, b: &mut dyn Behavior) {
            b.reset(&mut self.core);
            self.run(|c, io| b.start(c, io));
        }

        /// Ticks once at run tick `t`.
        fn tick_at(&mut self, b: &mut dyn Behavior, t: u32) {
            self.env.ticks = t;
            self.run(|c, io| b.tick(c, io));
        }

        fn fired(&mut self) -> bool {
            std::mem::take(&mut self.wires[1].changed)
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
    }

    #[test]
    fn parses_numbers_like_scanf() {
        assert_eq!(scan_f64(b" 12.5ft"), Some(12.5));
        assert_eq!(scan_f64(b"-3e2"), Some(-300.0));
        assert_eq!(scan_f64(b"abc"), None);
        assert_eq!(scan_f64(b""), None);
        assert!(close(parse_num(b"1e-400"), 0.0));
        assert!(close(parse_num(b"0"), 0.0));
        assert!(close(parse_num(b"5"), 5.0));
    }

    #[test]
    fn pendulum_period_is_two_pi_root_l_over_g() {
        // 6 ft on Earth: 2π√(6/32.185) ≈ 2.7128 s.
        let p = pendulum_period(HEIGHTS_FT[5], GRAVITY_FT_S2[2]);
        assert!(close(p, TWO_PI * (6.0f64 / 32.185).sqrt()));
        assert!((p - 2.7128).abs() < 1e-3);
        // The phase starts held to one side and comes back each period.
        assert!(close(pendulum_phase(0.0, p), THREE_HALF_PI));
        assert!(close(pendulum_phase(p, p), THREE_HALF_PI + TWO_PI));
        assert!(close(pendulum_phase(5.0, 0.0), THREE_HALF_PI));
    }

    #[test]
    fn pendulum_pulses_at_the_centre() {
        let mut b = Bench::new(1.0);
        let mut p = Pendulum::default();
        b.core.settings.radios[1] = 1; // On at Center
        b.core.settings.numbers = [5, 2, 0, 0]; // 6 ft, Earth
        b.start(&mut p);
        let period = p.period;
        // Released at tick 10 (throttle passes: 10 - 0 > 6).
        b.tick_at(&mut p, 10);
        assert_eq!(p.released, 10);
        // Ticks every 7 run ticks; the bob passes the centre at P/4, 3P/4…
        let mut crossings = Vec::new();
        for t in (17..=10 + 60 * 6).step_by(7) {
            b.tick_at(&mut p, t);
            if b.core.state {
                crossings.push(f64::from(t - 10) / 60.0);
                assert!(b.fired());
            }
        }
        // Each crossing is seen within one throttle step after it happens.
        assert!(crossings.len() >= 4);
        for (k, &seen) in crossings.iter().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let at = (0.25 + 0.5 * k as f64) * period;
            assert!(seen >= at && seen < at + 7.0 / 60.0, "crossing {k} at {at}, seen {seen}");
        }
    }

    #[test]
    fn pendulum_outputs_its_period() {
        let mut b = Bench::new(1.0);
        let mut p = Pendulum::default();
        b.core.settings.radios = [1, 0, 0, 0]; // Custom length, Period Length
        b.core.settings.numbers[1] = 3; // the Moon
        b.core.settings.set_label(0, b"20");
        b.start(&mut p);
        assert!(p.has_number(&b.core, 1));
        b.tick_at(&mut p, 7);
        b.tick_at(&mut p, 14);
        assert!(close(b.core.number, TWO_PI * (20.0 / GRAVITY_FT_S2[3]).sqrt()));
    }

    #[test]
    fn drop_lands_after_root_two_h_over_g() {
        // 20 ft on Earth: √(40/32.185) ≈ 1.1148 s.
        let g = GRAVITY_FT_S2[2];
        let h = HEIGHTS_FT[4];
        assert_eq!(drop_pixels(0.0, g, h, 77), 0);
        assert_eq!(drop_pixels((2.0 * h / g).sqrt() + 1e-9, g, h, 77), 77);
        assert_eq!(drop_pixels(1.0, g, h, 77), trunc(16.0925 / 20.0 * 77.0));
        assert_eq!(drop_pixels(1.0, g, 0.0, 77), 0x7fff_ffff);

        let mut b = Bench::new(1.0);
        let mut d = DropChamber::default();
        d.init(&mut b.core); // elephant, Earth, 1,472 ft
        b.core.settings.numbers[1] = 4; // 20 ft
        b.start(&mut d);
        b.tick_at(&mut d, 0);
        assert_eq!(b.core.phase, 1);
        b.tick_at(&mut d, 6);
        b.tick_at(&mut d, 12);
        b.tick_at(&mut d, 18);
        assert_eq!(b.core.phase, 4);
        assert_eq!(d.fell_at, 18);
        let land = 18.0 + (2.0 * h / g).sqrt() * 60.0;
        let mut t = 18;
        while b.core.phase == 4 {
            t += 1;
            b.tick_at(&mut d, t);
        }
        assert_eq!(b.core.phase, 5);
        assert!(b.core.state);
        assert!(b.fired());
        assert!(f64::from(t) >= land - 1.0 && f64::from(t) <= land + 1.0);
        assert_eq!(d.y, 77);
    }

    #[test]
    fn cannon_range_is_speed_times_flight_time() {
        let mut b = Bench::new(1.0);
        let mut c = PhysicsCanon::default();
        b.core.settings.radios = [0, 0, 0, 0]; // preset, Distance
        b.core.settings.numbers = [3, 2, 4, 0]; // 90 mph, Earth, 20 ft
        b.start(&mut c);
        let tf = flight_time(20.0, GRAVITY_FT_S2[2]);
        assert!(close(c.flight, (40.0f64 / 32.185).sqrt()));
        b.tick_at(&mut c, 1);
        assert_eq!(c.fired_at, 1);
        b.tick_at(&mut c, 31);
        assert!(close(b.core.number, 0.5 * 132.0));
        assert_eq!(c.ball_x, trunc(0.5 / tf * 158.0));
        let mut t = 31;
        while c.armed {
            t += 1;
            b.tick_at(&mut c, t);
        }
        assert!(b.fired());
        assert!(close(b.core.number, 132.0 * tf));
        assert_eq!((c.ball_x, c.ball_y), (158, 66));
        // Fires once per run.
        b.tick_at(&mut c, t + 100);
        assert!(!b.fired());
    }

    #[test]
    fn cannon_speed_at_landing_is_in_mph() {
        let mut b = Bench::new(1.0);
        let mut c = PhysicsCanon::default();
        b.core.settings.radios = [0, 3, 0, 0];
        b.core.settings.numbers = [4, 2, 5, 0]; // no speed, Earth, 6 ft
        b.start(&mut c);
        b.tick_at(&mut c, 1);
        let mut t = 1;
        while c.armed {
            t += 1;
            b.tick_at(&mut c, t);
        }
        let vy = GRAVITY_FT_S2[2] * flight_time(6.0, GRAVITY_FT_S2[2]);
        assert!(close(b.core.number, vy * MPH));
    }

    #[test]
    fn area_volume_formulas() {
        assert!(close(area_volume(0, 3.0), 9.0));
        assert!(close(area_volume(1, 2.0), 4.0 * ARVO_PI));
        assert!(close(area_volume(2, 3.0), 27.0));
        assert!(close(area_volume(3, 1.0), 4.0 / 3.0 * std::f64::consts::PI));
    }

    #[test]
    fn area_volume_crunches_then_outputs() {
        let mut b = Bench::new(2.0);
        let mut a = AreaVolume::default();
        a.init(&mut b.core); // sphere
        b.start(&mut a);
        // The cache starts "none", so a wired input counts as new.
        b.tick_at(&mut a, 0);
        assert_eq!(b.core.phase, 1);
        for k in 1..=7 {
            b.tick_at(&mut a, 10 * k);
        }
        assert_eq!(b.core.phase, 0);
        assert!(close(b.core.number, 8.0 * FOUR_THIRDS_PI));
        // Only a value difference, not a flagged wire: no changed flag.
        assert!(!b.fired());

        b.set_input(-1.0);
        b.tick_at(&mut a, 100);
        for k in 1..=7 {
            b.tick_at(&mut a, 100 + 10 * k);
        }
        assert!(b.core.broken);
        assert!(close(b.core.number, 0.0));
        assert!(b.fired());
    }

    #[test]
    fn planet_outputs_its_numbers() {
        let mut b = Bench::new(0.0);
        let mut p = Planet;
        p.init(&mut b.core);
        b.core.settings.numbers[1] = 6;
        b.tick_at(&mut p, 0);
        assert!(close(b.core.number, 1.0));
        b.core.settings.numbers = [4, 0, 0, 0];
        b.tick_at(&mut p, 1);
        assert!(close(b.core.number, 88750.0));
    }
}
