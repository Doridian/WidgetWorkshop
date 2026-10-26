//! The number parts: the number crunchers (`Addr`, `Subr`, `Mulr`, `Divr`),
//! Math Functions (`1ncr`), the Converter (`Conv`), the Counter (`Cntr`),
//! the Totaler (`Ttlr`), the Random Number Generator (`rand`), the Coin
//! Toss (`Coin`) and the Three-Digit Numbers (`cnst`, `MiCo`).
//! docs/gadgets/logic.md has the original's code. None of them saves more
//! than the base chunk. Arithmetic goes through [`Ext`], as the x87 does it.

use std::cell::Cell;

use super::common::{any, draw_broken, format_number, point, random_int, scan_int, trunc_sat, truthy};
use super::x87::Ext;
use crate::gadget::{Behavior, Core, DrawCx};
use crate::sim::Io;

/// The largest number the parts multiply or divide to (`0x44cf71`), a
/// little short of `f64::MAX`.
const DBLMAX: f64 = f64::from_bits(0x7FEF_FFFF_D7B9_609A);

/// The digits' positions, ones last: frames `first + d` at points `at`.
fn draw_digits(core: &Core, cx: &mut DrawCx<'_>, value: i32, first: u16, at: [usize; 3]) {
    for (i, d) in [value / 100 % 10, value / 10 % 10, value % 10].into_iter().enumerate() {
        let (h, v) = point(cx, core.sheet, at[i]);
        let d = u16::try_from(d).unwrap_or(0);
        cx.frame(core.sheet, first + d, core.h + h, core.v + v);
    }
}

// ---------------------------------------------------------------------------
// The number crunchers

/// The body's frames by step (`0x49a278`).
const CRUNCH_FRAMES: [u16; 8] = [10, 11, 12, 13, 14, 5, 6, 7];

/// cNumberCruncher: adds, subtracts, multiplies or divides its inputs
/// (first = 0, second = 1) after an animation of seven throttled steps.
#[derive(Debug)]
pub struct NumberCruncher {
    /// 0 to 3: + − × ÷ (`+0x198`).
    op: u8,
    /// An input's wire was flagged changed when the crunching began
    /// (`+0x197`): only then is the output flagged.
    wire_changed: bool,
}

fn cruncher(op: u8) -> Box<dyn Behavior> {
    Box::new(NumberCruncher { op, wire_changed: true })
}

/// cAdder.
#[must_use]
pub fn adder() -> Box<dyn Behavior> {
    cruncher(0)
}

/// cSubtracter.
#[must_use]
pub fn subtracter() -> Box<dyn Behavior> {
    cruncher(1)
}

/// cMultiplier.
#[must_use]
pub fn multiplier() -> Box<dyn Behavior> {
    cruncher(2)
}

/// cDivider.
#[must_use]
pub fn divider() -> Box<dyn Behavior> {
    cruncher(3)
}

impl Behavior for NumberCruncher {
    /// `0x44ca43`.
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        let n = CRUNCH_FRAMES[usize::from(core.phase) % 8];
        cx.frame(core.sheet, n, core.h, core.v);
        let (h, v) = point(cx, core.sheet, 0);
        cx.frame(core.sheet, u16::from(self.op) + 1, core.h + h, core.v + v);
        if core.broken {
            draw_broken(core, cx);
        }
    }

    fn bounds_frame(&self) -> u16 {
        5
    }

    /// `0x44cc85`.
    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        if core.phase == 0 {
            let v0 = io.value_changed(core, 0);
            let v1 = io.value_changed(core, 1);
            let c0 = io.changed(core, 0);
            let c1 = io.changed(core, 1);
            if v0 || v1 || c0 || c1 {
                self.wire_changed = c0 || c1;
                core.phase = 1;
                io.play(1020);
            }
            return;
        }
        if !io.throttle(core, 9) {
            return;
        }
        core.phase += 1;
        if core.phase == 8 {
            let a = io.number(core, 0);
            let b = io.number(core, 1);
            core.phase = 0;
            match self.op {
                0 => core.number = Ext::from(a).add(b).to_f64(),
                1 => core.number = Ext::from(a).sub(b).to_f64(),
                2 => {
                    // The error sticks: only a good division clears it.
                    if b.abs() > 1.0 && Ext::from(a.abs()) > Ext::from(DBLMAX).div(b.abs()) {
                        core.broken = true;
                    } else {
                        core.number = Ext::from(a).mul(b).to_f64();
                    }
                }
                3 => {
                    if b == 0.0 || b.is_nan() {
                        core.number = 0.0;
                        core.broken = true;
                    } else if b.abs() < 1.0 && Ext::from(a.abs()) > Ext::from(b.abs()).mul(DBLMAX) {
                        core.broken = true;
                    } else {
                        core.number = Ext::from(a).div(b).to_f64();
                        core.broken = false;
                    }
                }
                _ => {}
            }
            if self.wire_changed {
                io.fire(core, 2);
            }
        }
        if core.broken {
            core.number = 0.0;
        }
    }

    /// `0x44ca1a`.
    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        core.empty_cache = true;
    }

    fn has_number(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

// ---------------------------------------------------------------------------
// Math Functions

/// The functions' symbols by number field 0 (`0x49a954`): √, x², !, Sin,
/// Cos, Tan, Int.
const FUNCTION_FRAMES: [u16; 7] = [10, 11, 9, 13, 12, 14, 15];

/// Degrees to radians (`0x44e18d`), not quite π/180.
const DEGREES: f64 = f64::from_bits(0x3F91_DF46_A252_9843);

/// The factorials 0! to 170! (`0x49a3fc`), as the original has them: exact
/// to 12!, then to about seven digits.
pub const FACTORIALS: [u64; 171] = [
    0x3ff0_0000_0000_0000,
    0x3ff0_0000_0000_0000,
    0x4000_0000_0000_0000,
    0x4018_0000_0000_0000,
    0x4038_0000_0000_0000,
    0x405e_0000_0000_0000,
    0x4086_8000_0000_0000,
    0x40b3_b000_0000_0000,
    0x40e3_b000_0000_0000,
    0x4116_2600_0000_0000,
    0x414b_af80_0000_0000,
    0x4183_08a8_0000_0000,
    0x41bc_8cfc_0000_0000,
    0x41f7_328c_cc80_0000,
    0x4234_4c3b_2350_0000,
    0x4273_0777_1ba8_0000,
    0x42b3_0777_7735_8000,
    0x42f4_37ee_d20c_a000,
    0x4336_beec_dbfd_3c00,
    0x437b_02b9_2ee2_ab80,
    0x43c0_e1b3_bd4d_ab30,
    0x4406_283b_d9e8_a65e,
    0x444e_7752_dd24_cbeb,
    0x4495_e5c3_6450_82d8,
    0x44e0_6c52_67b5_6fb2,
    0x4529_a940_c20b_7e86,
    0x4574_d984_c05b_1f8e,
    0x45c1_9787_f4bf_7035,
    0x460e_c92d_86aa_2a35,
    0x465b_e651_86dc_7f4b,
    0x46aa_27ec_b09b_6007,
    0x46f9_56ad_1c8f_1e6a,
    0x4749_56ac_cf02_c8bb,
    0x479a_2162_8642_62ed,
    0x47eb_c378_9bb7_c631,
    0x483e_5dcc_4cec_8d4d,
    0x4891_14c2_9e3b_89b1,
    0x48e3_c000_d466_9767,
    0x4937_7401_4775_2073,
    0x498c_9561_6e2c_ce02,
    0x49e1_dd5d_0997_f530,
    0x4a36_e39f_5349_4862,
    0x4a8e_0ac0_c00a_f460,
    0x4ae4_2f39_8c3d_3829,
    0x4b3b_c0ef_82eb_71f4,
    0x4b93_83a7_fa93_93fd,
    0x4bec_0d41_bca0_5fd0,
    0x4c44_99bc_1915_e720,
    0x4c9e_e69b_30e7_a17d,
    0x4cf7_a88e_5bfc_bd48,
    0x4d52_7baf_04e2_d7fd,
    0x4dad_751f_726b_b7b0,
    0x4e07_ef29_6523_bb76,
    0x4e63_d20e_1dc0_386b,
    0x4ec0_b93b_fefa_e653,
    0x4f1c_be5e_994f_1e7b,
    0x4f79_2693_3cd7_0bc4,
    0x4fd6_665b_2054_bf37,
    0x5034_4cc2_6123_a8e6,
    0x5092_b6c3_33d0_6ff3,
    0x50f1_8b57_23f3_16cc,
    0x5150_b8cf_0a65_3ab7,
    0x51b0_3308_7d58_0104,
    0x520f_e478_9915_6df3,
    0x526f_e478_667b_2d3a,
    0x52d0_3205_7666_f250,
    0x5330_b395_8020_d9ae,
    0x5391_7c00_8fce_bab5,
    0x53f2_93c0_da25_364b,
    0x5454_074c_0a9b_2d91,
    0x54b5_e7fa_9230_fa6d,
    0x5518_4d5a_3880_e394,
    0x557b_5705_858f_1780,
    0x55df_2f42_0874_d4da,
    0x5642_0752_23be_b47c,
    0x56a5_2094_659b_61ae,
    0x5709_16b0_8906_49e6,
    0x576e_2f4c_3089_2ebb,
    0x57d2_64d2_3c86_5c70,
    0x5836_b473_a4c6_275b,
    0x589c_6190_a894_6d8c,
    0x5901_f5bd_7d2a_55ac,
    0x5967_02da_ae61_2dd5,
    0x59cd_d7b3_c17c_9510,
    0x5a33_958d_e71d_0cd9,
    0x5a9a_02a0_76f2_950f,
    0x5b01_79c4_13b0_325d,
    0x5b67_c186_0666_5694,
    0x5bd0_550c_0cc3_6ec1,
    0x5c36_b645_8842_167c,
    0x5c9f_f051_3759_0582,
    0x5d06_b4da_3953_a719,
    0x5d70_51fc_204b_13ab,
    0x5dd7_b723_8a2a_cf90,
    0x5e41_6a7d_72dc_fcc3,
    0x5ea9_da12_b475_d55a,
    0x5f13_638d_cc4d_2760,
    0x5f7d_62e2_fc9b_a6c2,
    0x5fe6_7fb5_b62e_d9e7,
    0x6051_66c6_a70f_bc51,
    0x60bb_3096_6508_963d,
    0x6125_7456_a352_19b3,
    0x6191_18b5_0b56_eada,
    0x61fb_83c3_5dc8_8c99,
    0x6266_5b0f_27fe_ce94,
    0x62d2_56b2_525b_7c46,
    0x633e_5f97_a86c_77a8,
    0x63a9_63e7_de18_bccc,
    0x6415_6c4c_0fed_5b4b,
    0x6482_3e38_f432_c507,
    0x64ef_5af1_e3b7_42a4,
    0x655b_30dd_b3f2_5195,
    0x65c7_cac1_b4ad_148a,
    0x6635_00ff_0fb7_a790,
    0x66a2_b4e3_2787_d240,
    0x6710_ce84_1584_06e6,
    0x677e_764f_60f8_f33b,
    0x67eb_d824_5045_d63d,
    0x6859_ab41_9942_1162,
    0x68c7_dd37_00ca_98a6,
    0x6936_5f63_8585_15ee,
    0x69a5_262c_246f_f89e,
    0x6a14_2861_f804_4700,
    0x6a83_5ecd_b618_909d,
    0x6af2_c3d7_efa0_92d7,
    0x6b62_5340_8e5c_165a,
    0x6bd2_09f3_8995_7039,
    0x6c41_e5df_bf69_ea99,
    0x6cb1_e5df_cefc_cb5d,
    0x6d22_09ab_733a_95dd,
    0x6d92_51d2_1786_1a85,
    0x6e02_bfbd_2759_6e7c,
    0x6e73_55bb_69ec_bbf7,
    0x6ee4_1714_9589_2411,
    0x6f55_0829_73c1_fdde,
    0x6fc6_2e9b_b12c_1233,
    0x7037_9185_4dfc_c24e,
    0x70a9_39c0_b511_b164,
    0x711b_3243_8d21_4e32,
    0x718d_8895_72f6_3ebe,
    0x7200_26b1_8e96_3e52,
    0x7271_ca9f_f3d9_3d17,
    0x72e3_bcc9_378c_8e83,
    0x7356_0ce8_fa21_1fb2,
    0x73c8_ce86_0766_3218,
    0x743c_19f3_c2ce_ad98,
    0x74b0_06cc_d83f_d140,
    0x7522_67cf_89a5_ceaa,
    0x7595_4807_ebfb_451e,
    0x7608_c5d9_52e2_a943,
    0x767d_07da_8281_e35c,
    0x76f1_1fa1_e84e_947b,
    0x7764_5590_58ad_4ea0,
    0x77d8_4e46_79bc_a94e,
    0x784d_3e2c_f092_d499,
    0x78c1_b4a5_16a5_928a,
    0x7935_9429_2fb9_6013,
    0x79aa_77ba_adcd_2db9,
    0x7a20_55e5_2e65_2c68,
    0x7a94_4ab2_782d_0cd9,
    0x7b09_5d5f_5e6d_80cd,
    0x7b7f_e771_c7a4_7d8c,
    0x7bf4_3075_c67d_db42,
    0x7c69_b5b6_a29e_9870,
    0x7ce0_7868_e8a4_1d2a,
    0x7d55_3b37_24e7_4a05,
    0x7dcb_88cb_7684_fa7e,
    0x7e41_f63d_11b2_7927,
    0x7eb7_932f_cf8b_8f7e,
    0x7f2f_2054_ea33_9b25,
    0x7fa4_ab78_76a3_4240,
];

/// cOneNumberCruncher: a function of its input (number field 0) after an
/// animation of seven throttled steps.
#[derive(Debug)]
pub struct OneNumberCruncher {
    /// The input's wire was flagged changed when it began (`+0x197`).
    wire_changed: bool,
}

#[must_use]
pub fn one_number_cruncher() -> Box<dyn Behavior> {
    Box::new(OneNumberCruncher { wire_changed: true })
}

/// Function `f` of `x` (`0x44def5`): `Err` on a domain error, `Ok(None)`
/// for an unknown function.
pub fn math_function(f: u32, x: f64) -> Result<Option<f64>, ()> {
    let r = match f {
        0 => (x >= 0.0).then(|| Ext::from(x).sqrt().to_f64()),
        1 => (Ext::from(x.abs()) < Ext::from(DBLMAX).sqrt()).then(|| Ext::from(x).mul(x).to_f64()),
        // `(u16)ftol_clamped(x)`: 65536 + 5 is 5.
        2 => usize::try_from(trunc_sat(x) & 0xffff)
            .ok()
            .and_then(|n| FACTORIALS.get(n))
            .map(|&b| f64::from_bits(b)),
        3 => Some(Ext::from(x).mul(DEGREES).to_f64().sin()),
        4 => Some(Ext::from(x).mul(DEGREES).to_f64().cos()),
        5 => Some(Ext::from(x).mul(DEGREES).to_f64().tan()),
        6 => Some(x.floor()),
        _ => return Ok(None),
    };
    r.map(Some).ok_or(())
}

impl Behavior for OneNumberCruncher {
    /// `0x44e195`.
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, u16::from(core.phase) + 1, core.h, core.v);
        // Past 6 the original reads past the table.
        let f = usize::try_from(core.settings.numbers[0]).unwrap_or(usize::MAX);
        if let Some(&n) = FUNCTION_FRAMES.get(f) {
            let (h, v) = point(cx, core.sheet, 0);
            cx.frame(core.sheet, n, core.h + h, core.v + v);
        }
        if core.broken {
            draw_broken(core, cx);
        }
    }

    /// `0x44def5`.
    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        if core.phase == 0 {
            let v = io.value_changed(core, 0);
            self.wire_changed = io.changed(core, 0);
            if v || self.wire_changed {
                core.phase = 1;
                io.play(1020);
            }
            return;
        }
        if !io.throttle(core, 9) {
            return;
        }
        core.phase += 1;
        if core.phase == 8 {
            let x = io.number(core, 0);
            core.phase = 0;
            core.broken = false;
            match math_function(core.settings.numbers[0], x) {
                Ok(Some(y)) => core.number = y,
                Ok(None) => {}
                Err(()) => core.broken = true,
            }
            if core.broken {
                core.number = 0.0;
            }
            if self.wire_changed {
                io.fire(core, 1);
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

    any!();
}

// ---------------------------------------------------------------------------
// The Converter

/// −459.4 °F (`0x443e63`).
const FAHRENHEIT_MIN: f64 = f64::from_bits(0xC07C_B666_6666_6667);
/// 5/9 (`0x443e57`, an extended constant holding a double).
const FIVE_NINTHS: f64 = f64::from_bits(0x3FE1_C71C_71C7_1C72);
/// Feet per meter (`0x443e6b`, the same).
const FEET_PER_METER: f64 = f64::from_bits(0x400A_3F28_FCA3_F28F);
/// Pounds per kilogram (`0x443e7f`, the same).
const POUNDS_PER_KG: f64 = f64::from_bits(0x4001_A312_20C5_37E2);

/// Conversion `c` of `x` (`0x443c3b`): the result (`None` for an unknown
/// one) and whether `x` is out of range.
pub fn convert(c: u32, x: f64) -> (Option<f64>, bool) {
    let e = Ext::from(x);
    match c {
        0 => (Some(e.mul(1.8).add(32.0).to_f64()), x < -273.0),
        1 => (Some(e.sub(32.0).mul(FIVE_NINTHS).to_f64()), x < FAHRENHEIT_MIN),
        2 => (Some(e.mul(FEET_PER_METER).to_f64()), x < 0.0),
        3 => (Some(e.mul(0.3048).to_f64()), x < 0.0),
        4 => (Some(e.mul(POUNDS_PER_KG).to_f64()), x < 0.0),
        5 => (Some(e.mul(0.453_592).to_f64()), x < 0.0),
        _ => (None, x < 0.0),
    }
}

/// cConversion: converts its input (number field 0: °C to °F, °F to °C, m
/// to ft, ft to m, kg to lb, lb to kg) after an animation.
#[derive(Debug)]
pub struct Conversion {
    /// The input's wire was flagged changed when it began (`+0x197`).
    wire_changed: bool,
}

#[must_use]
pub fn conversion() -> Box<dyn Behavior> {
    Box::new(Conversion { wire_changed: true })
}

impl Behavior for Conversion {
    /// `0x443e93`: the units, C° M Kg on the left of F° Ft Lb.
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, u16::from(core.phase) + 1, core.h, core.v);
        let c = core.settings.numbers[0];
        let half = c / 2;
        let (from, to) = if c % 2 == 1 { (half + 3, half) } else { (half, half + 3) };
        for (i, unit) in [from, to].into_iter().enumerate() {
            let (h, v) = point(cx, core.sheet, i);
            let n = u16::try_from(unit + 10).unwrap_or(0);
            cx.frame(core.sheet, n, core.h + h, core.v + v);
        }
        if core.broken {
            draw_broken(core, cx);
        }
    }

    /// `0x443c3b`.
    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        if core.phase == 0 {
            let v = io.value_changed(core, 0);
            self.wire_changed = io.changed(core, 0);
            if v || self.wire_changed {
                core.phase = 1;
                io.play(1020);
            }
            return;
        }
        if !io.throttle(core, 9) {
            return;
        }
        core.phase += 1;
        if core.phase == 8 {
            let x = io.number(core, 0);
            core.phase = 0;
            let (y, bad) = convert(core.settings.numbers[0], x);
            if let Some(y) = y {
                core.number = y;
            }
            core.broken = bad;
            if core.broken {
                core.number = 0.0;
            }
            if self.wire_changed {
                io.fire(core, 1);
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

    /// `0x443a5b`: °F to °C.
    fn init(&mut self, core: &mut Core) {
        core.settings.numbers[0] = 1;
    }

    any!();
}

// ---------------------------------------------------------------------------
// The Counter

/// cCounter: counts its input's changes, both ways for a boolean.
#[derive(Debug, Default)]
pub struct Counter {
    /// The count (`+0x142`).
    count: i32,
    /// Show the digits (`+0x13f`, always set).
    digits: bool,
}

#[must_use]
pub fn counter() -> Box<dyn Behavior> {
    Box::new(Counter::default())
}

impl Behavior for Counter {
    /// `0x44455e`: lit (frame 1) just after a count; broken from 1000.
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, u16::from(core.phase), core.h, core.v);
        if self.count < 1000 {
            if self.digits && self.count >= 0 {
                draw_digits(core, cx, self.count, 4, [0, 1, 2]);
            }
        } else {
            draw_broken(core, cx);
        }
    }

    fn bounds_frame(&self) -> u16 {
        2
    }

    /// `0x44489f`. A boolean or object source counts only its flagged
    /// changes; a number counts value changes too.
    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        let Some(flag_only) = io
            .source(core, 0)
            .map(|(g, k)| g.behavior.has_bool(&g.core, k) || g.behavior.has_object(&g.core, k))
        else {
            return;
        };
        let c = io.changed(core, 0);
        let v = io.value_changed(core, 0);
        if c || (v && !flag_only) {
            // `0x444869`.
            core.phase = 1;
            self.count = self.count.wrapping_add(1);
            core.number = f64::from(self.count);
        }
        if core.phase == 1 && io.throttle(core, 6) {
            core.phase = 2;
            io.fire(core, 1);
        }
    }

    /// `0x444531`.
    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        self.digits = true;
        self.count = 0;
        core.phase = 2;
    }

    fn has_number(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    fn number_out(&self, _core: &Core, _knot: usize) -> f64 {
        f64::from(self.count)
    }

    any!();
}

// ---------------------------------------------------------------------------
// The Totaler

/// The labels by mode (`0x49c334`): TOTAL, AVERAGE, MAXIMUM, MINIMUM.
const TOTALER_LABELS: [u16; 4] = [4, 3, 5, 6];

/// cTotaler: the total, average, maximum or minimum (radio group 0) of the
/// values its input takes.
#[derive(Debug)]
pub struct Totaler {
    /// How many (`+0x142`).
    count: i32,
    average: f64,
    sum: f64,
    min: f64,
    max: f64,
    /// No value yet this run (`+0x1bb`): the number isn't shown.
    waiting: bool,
    /// The decimals shown (`+0x196`), grown as the number is drawn.
    decimals: Cell<u8>,
}

impl Default for Totaler {
    fn default() -> Totaler {
        Totaler {
            count: 0,
            average: 0.0,
            sum: 0.0,
            min: 1e300,
            max: -1e300,
            waiting: false,
            decimals: Cell::new(0),
        }
    }
}

#[must_use]
pub fn totaler() -> Box<dyn Behavior> {
    Box::new(Totaler::default())
}

impl Behavior for Totaler {
    /// `0x45c0b4`.
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, 1, core.h, core.v);
        if let Some(&n) = TOTALER_LABELS.get(usize::from(core.settings.radios[0])) {
            let (h, v) = point(cx, core.sheet, 0);
            cx.frame(core.sheet, n, core.h + h, core.v + v);
        }
        if !cx.running || self.waiting {
            return;
        }
        let mut decimals = self.decimals.get();
        let s = format_number(core.number, &mut decimals);
        self.decimals.set(decimals);
        for (i, c) in s.bytes().take(10).enumerate() {
            let n = match c {
                b' ' => continue,
                b'.' => 7,
                b'-' => 8,
                b'E' | b'e' => 9,
                b'+' => 10,
                b'0'..=b'9' => u16::from(c - b'0') + 11,
                _ => {
                    draw_broken(core, cx);
                    return;
                }
            };
            let (h, v) = point(cx, core.sheet, i + 1);
            cx.frame(core.sheet, n, core.h + h, core.v + v);
        }
    }

    /// `0x45c4ac`. The cache is primed at start, so the value there then
    /// isn't counted until it changes or its wire is flagged.
    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        let c = io.changed(core, 0);
        let v = io.value_changed(core, 0);
        if !(c || v) {
            return;
        }
        self.waiting = false;
        let x = io.number(core, 0);
        self.count = self.count.wrapping_add(1);
        self.sum = Ext::from(self.sum).add(x).to_f64();
        self.average = Ext::from(self.sum).div(Ext::from(self.count)).to_f64();
        if self.max < x {
            self.max = x;
        }
        if x < self.min {
            self.min = x;
        }
        match core.settings.radios[0] {
            0 => core.number = self.sum,
            1 => core.number = self.average,
            2 => core.number = self.max,
            3 => core.number = self.min,
            _ => {}
        }
        if c {
            io.fire(core, 1);
        }
    }

    /// `0x45c3b4`.
    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        *self = Totaler {
            waiting: self.waiting,
            ..Totaler::default()
        };
    }

    /// `0x45c494`.
    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        self.waiting = true;
    }

    fn has_number(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

// ---------------------------------------------------------------------------
// The Random Number Generator

/// 1/16383 (`0x450cc7`, an extended constant holding a double).
const INV_16383: f64 = f64::from_bits(0x3F10_0040_0100_0400);

/// cRandomNumberGenerator: while its input is true, a new random number
/// every four throttled steps: 1 to 10 or 1 to 100 (preset), or between
/// two typed numbers (custom).
#[derive(Debug, Default)]
pub struct RandomNumber;

#[must_use]
pub fn random_number() -> Box<dyn Behavior> {
    Box::new(RandomNumber)
}

/// The number `n` (from `0x415008`) gives for the custom range `lo`, `hi`,
/// both included; the ends come up half as often.
pub fn random_in(n: i32, lo: i32, hi: i32) -> f64 {
    let (a, b) = if hi > lo { (hi, lo) } else { (lo, hi) };
    let r = Ext::from(n & 0x3fff).mul(INV_16383).to_f64();
    let t = Ext::from(b.wrapping_sub(a)).mul(r).add(Ext::from(a)).to_f64();
    Ext::from(t).add(0.5).to_f64().floor()
}

impl Behavior for RandomNumber {
    /// `0x450972`.
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, u16::from(core.phase) + 1, core.h, core.v);
    }

    /// `0x450aff`.
    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        if core.phase == 0 {
            // While true, not on a change.
            if truthy(io.number(core, 0)) {
                core.phase = 1;
                io.play(1007);
            }
            return;
        }
        if !io.throttle(core, 15) {
            return;
        }
        core.phase += 1;
        if core.phase == 4 {
            core.phase = 0;
            io.fire(core, 1);
            let n = random_int(io.env);
            let s = &core.settings;
            if s.radios[0] == 0 {
                match s.numbers[0] {
                    0 => core.number = f64::from(n % 10 + 1),
                    1 => core.number = f64::from(n % 100 + 1),
                    _ => {}
                }
            } else {
                let lo = scan_int(s.label(0)).unwrap_or(0);
                let hi = scan_int(s.label(1)).unwrap_or(0);
                core.number = random_in(n, lo, hi);
            }
        }
    }

    fn has_number(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

// ---------------------------------------------------------------------------
// The Coin Toss

/// cCoinToss: while its input is true, tosses a coin: 1 heads, 0 tails.
#[derive(Debug, Default)]
pub struct CoinToss;

#[must_use]
pub fn coin_toss() -> Box<dyn Behavior> {
    Box::new(CoinToss)
}

impl Behavior for CoinToss {
    /// `0x4423da`.
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, u16::from(core.phase) + 1, core.h, core.v);
        if core.phase == 0 {
            let (h, v) = point(cx, core.sheet, 0);
            let n = if core.number == 0.0 { 8 } else { 7 };
            cx.frame(core.sheet, n, core.h + h, core.v + v);
        }
    }

    /// `0x4426a8`.
    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        if core.phase == 0 {
            if truthy(io.number(core, 0)) {
                core.phase = 1;
            }
            return;
        }
        if !io.throttle(core, 12) {
            return;
        }
        if core.phase == 1 {
            io.play(1006);
        }
        core.phase += 1;
        if core.phase == 5 {
            core.phase = 0;
            core.number = if random_int(io.env) & 1 == 1 { 1.0 } else { 0.0 };
            io.fire(core, 1);
        }
    }

    fn has_number(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

// ---------------------------------------------------------------------------
// The Three-Digit Numbers

/// `0x442dc4` / `0x449b24`: the number typed (text 0); 0 when empty or
/// out of 0 to 999, `old` when unreadable.
fn typed_number(core: &Core, old: i32) -> i32 {
    let t = core.settings.label(0);
    if t.is_empty() {
        return 0;
    }
    let v = scan_int(t).unwrap_or(old);
    if (0..=999).contains(&v) { v } else { 0 }
}

/// The Constant's arrows (points 1021 in `WWGADGET.DLL`, `v`, `h`): up for
/// hundreds, tens, ones; the digits; down for hundreds, tens, ones. The
/// class's init makes its click rectangles from them (`0x442e22`).
const CONSTANT_POINTS: [(i32, i32); 9] = [
    (0, 6),
    (0, 31),
    (0, 55),
    (24, 13),
    (24, 35),
    (24, 57),
    (55, 6),
    (55, 31),
    (55, 55),
];

/// An arrow's size (frame 1 of sheet 1021): 25 × 14.
const ARROW: (i32, i32) = (14, 25);

/// cConstant: a number from 0 to 999, typed in its settings and changed by
/// clicking its arrows while running; kept when the run stops.
#[derive(Debug, Default)]
pub struct Constant {
    /// The number (`+0x142`).
    value: i32,
    /// An arrow is held (`+0x197`): its frame and point (`+0x198`,
    /// `+0x199`).
    pressed: bool,
    pressed_frame: u16,
    pressed_at: usize,
}

#[must_use]
pub fn constant() -> Box<dyn Behavior> {
    Box::new(Constant::default())
}

impl Behavior for Constant {
    /// `0x44326f`.
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, 4, core.h, core.v);
        if !core.settings.check(1) {
            draw_digits(core, cx, self.value, 8, [3, 4, 5]);
        }
        if self.pressed {
            let (h, v) = point(cx, core.sheet, self.pressed_at);
            cx.frame(core.sheet, self.pressed_frame, core.h + h, core.v + v);
        }
    }

    fn bounds_frame(&self) -> u16 {
        4
    }

    /// `0x443885`: the arrow comes back up with the mouse button.
    fn tick(&mut self, _core: &mut Core, io: &mut Io<'_>) {
        if self.pressed && !io.env.mouse_down {
            self.pressed = false;
        }
    }

    /// `0x44369b`: an arrow adds or takes 1, 10 or 100, unless Lock Number
    /// (check 0); out of 0 to 999 it buzzes and goes back, though the
    /// output was flagged.
    fn click(&mut self, core: &mut Core, io: &mut Io<'_>, x: i32, y: i32) {
        if core.settings.check(0) {
            return;
        }
        let hit = |p: usize| {
            let (v, h) = CONSTANT_POINTS[p];
            (v..v + ARROW.0).contains(&y) && (h..h + ARROW.1).contains(&x)
        };
        let old = self.value;
        for (i, w) in [1, 10, 100].into_iter().enumerate() {
            let (up, down) = (2 - i, 8 - i);
            let delta = if hit(up) {
                self.pressed_at = up;
                w
            } else if hit(down) {
                self.pressed_at = down;
                -w
            } else {
                continue;
            };
            self.value += delta;
            self.pressed = true;
            // Up arrows are frames 3, 2, 1; down 7, 6, 5.
            self.pressed_frame = u16::try_from(if delta > 0 { 3 - i } else { 7 - i }).unwrap_or(0);
            io.fire(core, 0);
        }
        if !(0..=999).contains(&self.value) {
            self.value = old;
            io.play(1027);
        }
    }

    /// `0x4435ed`: not the base's, so the number stays, unless Auto Reset
    /// (check 2) puts back the one typed.
    fn reset(&mut self, core: &mut Core) {
        self.pressed = false;
        if core.settings.check(2) {
            self.settings_changed(core);
        }
    }

    /// `0x44385e`.
    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        if self.value != 0 {
            io.fire(core, 0);
        }
    }

    fn has_number(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    fn number_out(&self, _core: &Core, _knot: usize) -> f64 {
        f64::from(self.value)
    }

    fn settings_changed(&mut self, core: &mut Core) {
        self.value = typed_number(core, self.value);
    }

    any!();
}

/// cMiniConstant: a typed number from 0 to 999, small.
#[derive(Debug, Default)]
pub struct MiniConstant {
    /// The number (`+0x142`).
    value: i32,
}

#[must_use]
pub fn mini_constant() -> Box<dyn Behavior> {
    Box::new(MiniConstant::default())
}

impl Behavior for MiniConstant {
    /// `0x449e13`.
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, 11, core.h, core.v);
        if !core.settings.check(1) {
            draw_digits(core, cx, self.value, 1, [0, 1, 2]);
        }
    }

    fn bounds_frame(&self) -> u16 {
        11
    }

    /// `0x44a0e9`: as the Constant's (the check box can only come from a
    /// file).
    fn reset(&mut self, core: &mut Core) {
        if core.settings.check(2) {
            self.settings_changed(core);
        }
    }

    /// `0x44a18d`.
    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        if self.value != 0 {
            io.fire(core, 0);
        }
    }

    fn has_number(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    fn number_out(&self, _core: &Core, _knot: usize) -> f64 {
        f64::from(self.value)
    }

    fn settings_changed(&mut self, core: &mut Core) {
        self.value = typed_number(core, self.value);
    }

    any!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gadgets::common::bench::{Bench, Source};
    use formats::tables::KnotKind::{Input, Output};

    fn crunch_bench(a: f64, b: f64) -> Bench {
        let mut bench = Bench::with_knots(&[Input, Input, Output]);
        bench.source(0, Source::number(a));
        bench.source(1, Source::number(b));
        bench.wire(2);
        bench
    }

    /// Ticks from `t` until the part is idle again, 10 ticks apart.
    fn finish(b: &mut Bench, g: &mut dyn Behavior, t: u32) -> u32 {
        let mut t = t;
        b.tick_at(g, t);
        while b.core.phase != 0 {
            t += 10;
            b.tick_at(g, t);
        }
        t + 10
    }

    #[test]
    fn crunchers_compute_after_the_animation() {
        for (make, want) in [
            (adder as fn() -> _, 9.0),
            (subtracter, 3.0),
            (multiplier, 18.0),
            (divider, 2.0),
        ] {
            let mut g = make();
            let mut b = crunch_bench(6.0, 3.0);
            b.start(g.as_mut());
            b.tick_at(g.as_mut(), 0);
            assert_eq!(b.core.phase, 1);
            // Seven steps of more than 9 ticks.
            for t in [10, 20, 30, 40, 50, 60] {
                b.tick_at(g.as_mut(), t);
                assert_eq!(b.core.number, 0.0);
            }
            b.tick_at(g.as_mut(), 65);
            assert_eq!(b.core.phase, 7);
            b.tick_at(g.as_mut(), 70);
            assert_eq!(b.core.phase, 0);
            assert_eq!(b.core.number, want);
        }
    }

    #[test]
    fn crunchers_fire_only_for_a_flagged_wire() {
        let mut g = adder();
        let mut b = crunch_bench(1.0, 2.0);
        b.start(g.as_mut());
        // A value seen for the first time: computed, not flagged.
        let t = finish(&mut b, g.as_mut(), 0);
        assert_eq!(b.core.number, 3.0);
        assert!(!b.fired(2));
        // A flagged input: flagged on.
        b.set(1, Source::number(5.0));
        let t = finish(&mut b, g.as_mut(), t);
        assert_eq!(b.core.number, 6.0);
        assert!(b.fired(2));
        // A flag alone, the same value: computed again and flagged.
        b.raise(0);
        finish(&mut b, g.as_mut(), t);
        assert!(b.fired(2));
        // Unwired, nothing ever starts.
        let mut b = Bench::with_knots(&[Input, Input, Output]);
        b.start(g.as_mut());
        b.tick_at(g.as_mut(), 0);
        assert_eq!(b.core.phase, 0);
    }

    #[test]
    fn multiplier_overflow_sticks() {
        let mut g = multiplier();
        let mut b = crunch_bench(1e200, 1e200);
        b.start(g.as_mut());
        let t = finish(&mut b, g.as_mut(), 0);
        assert!(b.core.broken);
        assert_eq!(b.core.number, 0.0);
        b.set(0, Source::number(2.0));
        b.set(1, Source::number(3.0));
        let t = finish(&mut b, g.as_mut(), t);
        assert!(b.core.broken);
        assert_eq!(b.core.number, 0.0);
        // Until reset.
        b.start(g.as_mut());
        finish(&mut b, g.as_mut(), t);
        assert!(!b.core.broken);
        assert_eq!(b.core.number, 6.0);
        // At the limit: DBLMAX / 2 * 2 is fine, a bit more isn't.
        assert_eq!(Ext::from(DBLMAX).div(2.0).mul(2.0).to_f64(), DBLMAX);
    }

    #[test]
    fn divider_by_zero_breaks_until_a_good_division() {
        let mut g = divider();
        let mut b = crunch_bench(1.0, 0.0);
        b.start(g.as_mut());
        let t = finish(&mut b, g.as_mut(), 0);
        assert!(b.core.broken);
        b.set(1, Source::number(4.0));
        finish(&mut b, g.as_mut(), t);
        assert!(!b.core.broken);
        assert_eq!(b.core.number, 0.25);
        assert_eq!(g.bounds_frame(), 5);
    }

    #[test]
    fn factorials_are_the_originals() {
        assert_eq!(FACTORIALS.len(), 171);
        let mut f = 1.0f64;
        for (n, &b) in FACTORIALS.iter().enumerate().take(13) {
            if n > 0 {
                f *= f64::from(u32::try_from(n).unwrap());
            }
            assert_eq!(f64::from_bits(b), f, "{n}!");
        }
        assert_eq!(f64::from_bits(FACTORIALS[13]), 6_227_021_000.0);
        assert_eq!(FACTORIALS[170], 0x7fa4_ab78_76a3_4240);
        assert_eq!(math_function(2, 5.9), Ok(Some(120.0)));
        assert_eq!(math_function(2, 65541.0), Ok(Some(120.0)));
        assert_eq!(math_function(2, -1.0), Err(()));
        assert_eq!(math_function(2, 171.0), Err(()));
    }

    #[test]
    fn math_functions() {
        assert_eq!(math_function(0, 16.0), Ok(Some(4.0)));
        assert_eq!(math_function(0, -1.0), Err(()));
        assert_eq!(math_function(1, -3.0), Ok(Some(9.0)));
        assert_eq!(math_function(1, 1.4e154), Err(()));
        assert_eq!(math_function(1, 1.3e154), Ok(Some(1.3e154 * 1.3e154)));
        assert_eq!(math_function(6, -1.5), Ok(Some(-2.0)));
        assert_eq!(math_function(7, 1.0), Ok(None));
        let Ok(Some(s)) = math_function(3, 30.0) else { panic!() };
        assert!((s - 0.5).abs() < 1e-12);
        // Not quite π/2: large, finite.
        let Ok(Some(t)) = math_function(5, 90.0) else { panic!() };
        assert!(t.is_finite() && t > 1e12);
    }

    #[test]
    fn math_functions_error_clears_each_time() {
        let mut g = one_number_cruncher();
        let mut b = Bench::new(-4.0);
        b.start(g.as_mut());
        let t = finish(&mut b, g.as_mut(), 0);
        assert!(b.core.broken);
        assert!(!b.fired(1));
        b.set_input(9.0);
        finish(&mut b, g.as_mut(), t);
        assert!(!b.core.broken);
        assert_eq!(b.core.number, 3.0);
        assert!(b.fired(1));
    }

    #[test]
    fn conversions() {
        assert_eq!(convert(0, 100.0), (Some(212.0), false));
        assert!(convert(0, -300.0).1);
        assert_eq!(convert(1, 212.0), (Some(100.0), false));
        assert!(!convert(1, -459.4).1);
        assert!(convert(1, -459.5).1);
        assert_eq!(convert(3, 10.0), (Some(3.048), false));
        assert!(convert(5, -1.0).1);
        assert_eq!(convert(9, -1.0), (None, true));
        let mut g = conversion();
        let mut b = Bench::new(-500.0);
        b.fresh(g.as_mut());
        assert_eq!(b.core.settings.numbers[0], 1);
        b.start(g.as_mut());
        finish(&mut b, g.as_mut(), 0);
        assert!(b.core.broken);
        assert_eq!(b.core.number, 0.0);
    }

    #[test]
    fn counter_counts_both_edges() {
        let mut g = counter();
        let mut b = Bench::with_knots(&[Input, Output]);
        b.wire(1);
        b.source(0, Source::boolean(false));
        b.start(g.as_mut());
        assert_eq!(b.core.phase, 2);
        b.tick_at(g.as_mut(), 0);
        assert_eq!(b.out(g.as_ref(), 1).number(), 0.0);
        for (t, v) in [(10, true), (20, false), (30, true)] {
            b.set(0, Source::boolean(v));
            b.tick_at(g.as_mut(), t);
        }
        assert_eq!(b.out(g.as_ref(), 1).number(), 3.0);
        // The throttle last passed 10 ticks before: flagged at once.
        assert_eq!(b.core.phase, 2);
        assert!(b.fired(1));
        // Lit until more than 6 ticks after it last passed.
        b.set(0, Source::boolean(false));
        b.tick_at(g.as_mut(), 33);
        assert_eq!(b.core.phase, 1);
        b.tick_at(g.as_mut(), 36);
        assert!(!b.fired(1));
        b.tick_at(g.as_mut(), 37);
        assert!(b.fired(1));
        assert_eq!(b.core.phase, 2);
        // A boolean that changes without a flag isn't counted.
        b.source(0, Source::boolean(true));
        b.tick_at(g.as_mut(), 40);
        assert_eq!(b.out(g.as_ref(), 1).number(), 4.0);
        // A number counts a change of value too.
        b.source(0, Source::number(7.0));
        b.tick_at(g.as_mut(), 50);
        assert_eq!(b.out(g.as_ref(), 1).number(), 5.0);
        g.reset(&mut b.core);
        assert_eq!(b.out(g.as_ref(), 1).number(), 0.0);
    }

    #[test]
    fn totaler_ignores_the_start_value() {
        let mut g = totaler();
        let mut b = Bench::new(5.0);
        b.start(g.as_mut());
        b.tick_at(g.as_mut(), 0);
        assert_eq!(b.core.number, 0.0);
        b.set_input(5.0);
        b.tick_at(g.as_mut(), 1);
        assert_eq!(b.core.number, 5.0);
        assert!(b.fired(1));
        b.source(0, Source::number(2.0));
        b.tick_at(g.as_mut(), 2);
        assert_eq!(b.core.number, 7.0);
        assert!(!b.fired(1));
        let t = g.as_any().downcast_ref::<Totaler>().unwrap();
        assert_eq!((t.count, t.average, t.max, t.min), (2, 3.5, 5.0, 2.0));
        b.core.settings.radios[0] = 3;
        b.set_input(-1.0);
        b.tick_at(g.as_mut(), 3);
        assert_eq!(b.core.number, -1.0);
        b.core.settings.radios[0] = 1;
        b.set_input(-1.0);
        b.tick_at(g.as_mut(), 4);
        assert_eq!(b.core.number, 1.25);
        g.reset(&mut b.core);
        let t = g.as_any().downcast_ref::<Totaler>().unwrap();
        assert_eq!(t.min.to_bits(), 0x7E37_E43C_8800_759C);
        assert_eq!(t.max.to_bits(), 0xFE37_E43C_8800_759C);
    }

    #[test]
    fn random_ranges() {
        // Custom: both ends included, from either order.
        for (lo, hi) in [(1, 5), (5, 1), (11, 15)] {
            let got: std::collections::BTreeSet<i64> = (0..0x4000)
                .map(|n| {
                    #[allow(clippy::cast_possible_truncation)]
                    let v = random_in(n, lo, hi) as i64;
                    v
                })
                .collect();
            let (a, b) = (i64::from(lo.min(hi)), i64::from(lo.max(hi)));
            assert_eq!(got, (a..=b).collect());
        }
        assert_eq!(random_in(0, 1, 5), 5.0);
        assert_eq!(random_in(0x3fff, 1, 5), 1.0);
        assert_eq!(random_in(0, 0, 0), 0.0);

        let mut g = random_number();
        let mut b = Bench::new(1.0);
        b.env.rng = 1;
        b.start(g.as_mut());
        b.tick_at(g.as_mut(), 0);
        assert_eq!(b.core.phase, 1);
        b.ticks(g.as_mut(), 16, 16, 3);
        assert_eq!(b.core.phase, 0);
        assert!(b.fired(1));
        let mut rng = 1;
        let n = i32::try_from(super::super::common::lrand(&mut rng) % 0x7fff_ffff).unwrap();
        assert_eq!(b.core.number, f64::from(n % 10 + 1));
        // Custom range from the texts.
        b.core.settings.radios[0] = 1;
        b.core.settings.set_label(0, b"11");
        b.core.settings.set_label(1, b"15");
        b.ticks(g.as_mut(), 100, 16, 4);
        assert!((11.0..=15.0).contains(&b.core.number));
        // An input of 0 stops it.
        b.set_input(0.0);
        b.tick_at(g.as_mut(), 200);
        assert_eq!(b.core.phase, 0);
    }

    #[test]
    fn coin_tosses() {
        let mut g = coin_toss();
        let mut b = Bench::new(1.0);
        b.env.rng = 7;
        b.start(g.as_mut());
        b.ticks(g.as_mut(), 0, 13, 5);
        assert_eq!(b.core.phase, 0);
        assert!(b.fired(1));
        let mut rng = 7;
        let n = super::super::common::lrand(&mut rng) % 0x7fff_ffff;
        assert_eq!(b.core.number, f64::from(n & 1));
    }

    fn constant_bench(text: &[u8], checks: u32) -> (Bench, Box<dyn Behavior>) {
        let mut g = constant();
        let mut b = Bench::with_knots(&[Output]);
        b.wire(0);
        b.core.settings.set_label(0, text);
        b.core.settings.checks = checks;
        b.fresh(g.as_mut());
        (b, g)
    }

    #[test]
    fn constant_survives_stop() {
        let (mut b, mut g) = constant_bench(b"42", 0);
        assert_eq!(b.out(g.as_ref(), 0).number(), 42.0);
        b.start(g.as_mut());
        assert!(b.fired(0));
        // Up on the tens, up on the ones.
        b.run(|c, io| g.click(c, io, 31 + 3, 2));
        b.run(|c, io| g.click(c, io, 55 + 24, 13));
        assert_eq!(b.out(g.as_ref(), 0).number(), 53.0);
        assert!(b.fired(0));
        // Stop: kept.
        g.reset(&mut b.core);
        assert_eq!(b.out(g.as_ref(), 0).number(), 53.0);
        // Auto Reset puts back the typed number.
        let (mut b, mut g) = constant_bench(b"42", 4);
        b.start(g.as_mut());
        b.run(|c, io| g.click(c, io, 6, 55));
        // Hundreds down from 42: out of range, back to 42.
        assert_eq!(b.out(g.as_ref(), 0).number(), 42.0);
        b.run(|c, io| g.click(c, io, 60, 0));
        assert_eq!(b.out(g.as_ref(), 0).number(), 43.0);
        // At h 55 the tens' and the ones' rectangles overlap: both.
        b.run(|c, io| g.click(c, io, 55, 0));
        assert_eq!(b.out(g.as_ref(), 0).number(), 54.0);
        g.reset(&mut b.core);
        assert_eq!(b.out(g.as_ref(), 0).number(), 42.0);
    }

    #[test]
    fn constant_arrows() {
        let (mut b, mut g) = constant_bench(b"5", 0);
        b.start(g.as_mut());
        b.fired(0);
        // Down on the hundreds: below 0, back to 5, still flagged.
        b.run(|c, io| g.click(c, io, 6, 55));
        assert_eq!(b.out(g.as_ref(), 0).number(), 5.0);
        assert!(b.fired(0));
        // Ones down: frame 7 at point 8, held while the button is.
        b.env.mouse_down = true;
        b.run(|c, io| g.click(c, io, 60, 55 + 13));
        assert_eq!(b.out(g.as_ref(), 0).number(), 4.0);
        assert!(b.fired(0));
        let c = g.as_any().downcast_ref::<Constant>().unwrap();
        assert_eq!((c.pressed, c.pressed_frame, c.pressed_at), (true, 7, 8));
        b.tick_at(g.as_mut(), 1);
        assert!(g.as_any().downcast_ref::<Constant>().unwrap().pressed);
        b.env.mouse_down = false;
        b.tick_at(g.as_mut(), 2);
        assert!(!g.as_any().downcast_ref::<Constant>().unwrap().pressed);
        // Outside every arrow: nothing; the rectangles are half-open.
        b.run(|c, io| g.click(c, io, 6 + 25, 14));
        assert_eq!(b.out(g.as_ref(), 0).number(), 4.0);
        assert!(!b.fired(0));
        // Locked: clicks do nothing.
        b.core.settings.set_check(0, true);
        b.run(|c, io| g.click(c, io, 55, 0));
        assert_eq!(b.out(g.as_ref(), 0).number(), 4.0);
        // Typed out of range or empty: 0; unreadable: unchanged.
        for (text, want) in [(&b"1000"[..], 0.0), (b"", 0.0), (b"7", 7.0), (b"x", 7.0)] {
            b.core.settings.set_label(0, text);
            g.settings_changed(&mut b.core);
            assert_eq!(b.out(g.as_ref(), 0).number(), want);
        }
        assert_eq!(g.bounds_frame(), 4);
    }

    #[test]
    fn mini_constant() {
        let mut g = super::mini_constant();
        let mut b = Bench::with_knots(&[Output]);
        b.wire(0);
        b.fresh(g.as_mut());
        b.start(g.as_mut());
        assert!(!b.fired(0));
        b.core.settings.set_label(0, b"999");
        g.settings_changed(&mut b.core);
        b.start(g.as_mut());
        assert!(b.fired(0));
        assert_eq!(b.out(g.as_ref(), 0).number(), 999.0);
        assert_eq!(g.bounds_frame(), 11);
    }
}
