//! What several families of parts share: reading inputs the ways the
//! original's helpers do, number parsing and formatting, the random
//! generator and drawing bits. docs/gadgets/logic.md section 1.

use crate::gadget::{Core, DrawCx, Object};
use crate::sim::{Env, Io};

macro_rules! any {
    () => {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    };
}
pub(crate) use any;

/// `_ftol` (`0x480ec4`): truncation toward zero.
#[allow(clippy::cast_possible_truncation)]
pub fn trunc(x: f64) -> i32 {
    x as i32
}

/// `0x439e3d`: truncation, saturating short of the extremes.
pub fn trunc_sat(x: f64) -> i32 {
    if x >= 2_147_483_647.0 {
        0x7fff_fffe
    } else if x <= -2_147_483_646.0 {
        -0x7fff_fffe
    } else {
        trunc(x)
    }
}

/// Whether `x` counts as true: `fcomp` against 0 sets ZF for a NaN too,
/// so a NaN is false.
pub fn truthy(x: f64) -> bool {
    !(x == 0.0 || x.is_nan())
}

/// What `sscanf(s, "%lf")` reads from the start of `s`, if anything.
pub fn scan_f64(s: &[u8]) -> Option<f64> {
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

/// What `sscanf(s, "%d")` (or `"%u"`, stored to an int) reads from the
/// start of `s`, if anything; wraps like the RTL's.
pub fn scan_int(s: &[u8]) -> Option<i32> {
    let s = s.trim_ascii_start();
    let (neg, s) = match s.first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let n = s.iter().take_while(|c| c.is_ascii_digit()).count();
    if n == 0 {
        return None;
    }
    let v = s[..n]
        .iter()
        .fold(0i32, |v, &c| v.wrapping_mul(10).wrapping_add(i32::from(c - b'0')));
    Some(if neg { v.wrapping_neg() } else { v })
}

/// A number typed in a text field (`0x4110c3`): 0 when unreadable, and
/// kept off zero and infinity.
pub fn parse_num(s: &[u8]) -> f64 {
    let d = scan_f64(s).unwrap_or(0.0);
    if d > 0.0 {
        d.clamp(1e-300, 1e300)
    } else if d < 0.0 {
        d.clamp(-1e300, -1e-300)
    } else {
        d
    }
}

/// The point `i` of table `id`, as (`h`, `v`).
pub fn point(cx: &DrawCx<'_>, id: u32, i: usize) -> (i32, i32) {
    cx.points(id)
        .get(i)
        .map_or((0, 0), |p| (i32::from(p.h), i32::from(p.v)))
}

/// The broken picture (slot 0x10, `0x41ed9d`).
// TODO: draw the broken sheet (`DAT_004a2694`) centred once it's
// identified; frame 1 stands in for it.
pub fn draw_broken(core: &Core, cx: &mut DrawCx<'_>) {
    cx.frame(core.sheet, 1, core.h, core.v);
}

// ---------------------------------------------------------------------------
// Reading a source the other ways (docs/gadgets/logic.md 1.2)

/// `0x43adec`: input `knot`'s source as a boolean, a boolean first, then
/// a number, then an object; false unwired or with none.
pub fn src_bool(io: &Io<'_>, core: &Core, knot: usize) -> bool {
    let Some((g, k)) = io.source(core, knot) else {
        return false;
    };
    let (b, c) = (&g.behavior, &g.core);
    if b.has_bool(c, k) {
        b.bool_out(c, k)
    } else if b.has_number(c, k) {
        truthy(b.number_out(c, k))
    } else if b.has_object(c, k) {
        b.object_out(c, k).is_some()
    } else {
        false
    }
}

/// `0x43c13c`: input `knot`'s source as a number, a number first, then a
/// boolean or an object as 1 or 0; 0 with none.
pub fn src_number(io: &Io<'_>, core: &Core, knot: usize) -> f64 {
    let Some((g, k)) = io.source(core, knot) else {
        return 0.0;
    };
    let (b, c) = (&g.behavior, &g.core);
    if b.has_number(c, k) {
        b.number_out(c, k)
    } else if b.has_bool(c, k) {
        f64::from(u8::from(b.bool_out(c, k)))
    } else if b.has_object(c, k) {
        f64::from(u8::from(b.object_out(c, k).is_some()))
    } else {
        0.0
    }
}

/// `0x43aef3`: input `knot`'s source's object, if it gives one.
pub fn src_object(io: &Io<'_>, core: &Core, knot: usize) -> Option<Object> {
    let (g, k) = io.source(core, knot)?;
    if g.behavior.has_object(&g.core, k) {
        g.behavior.object_out(&g.core, k)
    } else {
        None
    }
}

/// What a source gives: a boolean, a number, an object.
pub type Kinds = (bool, bool, bool);

/// `0x41e79c`: copies input `knot`'s source's boolean, number and object to
/// `core`'s outputs (all cleared unwired); returns which of them the
/// source gives (`has_bool` etc.), `None` unwired.
pub fn copy_input(io: &Io<'_>, core: &mut Core, knot: usize) -> Option<Kinds> {
    let Some((g, k)) = io.source(core, knot) else {
        core.state = false;
        core.number = 0.0;
        core.object = None;
        return None;
    };
    let kinds = (
        g.behavior.has_bool(&g.core, k),
        g.behavior.has_number(&g.core, k),
        g.behavior.has_object(&g.core, k),
    );
    core.state = src_bool(io, core, knot);
    core.number = src_number(io, core, knot);
    core.object = src_object(io, core, knot);
    Some(kinds)
}

// ---------------------------------------------------------------------------
// The random generator (docs/gadgets/logic.md 1.9)

/// The Borland RTL's `lrand` (`0x481530`): a 64-bit LCG, 31 bits out.
pub fn lrand(state: &mut u64) -> u32 {
    *state = state.wrapping_mul(0x0000_15A4_0000_4E35).wrapping_add(1);
    u32::try_from((*state >> 32) & 0x7fff_ffff).unwrap_or(0)
}

/// `0x415008`: a random number from 0 to `0x7ffffffe`.
pub fn random_int(env: &mut Env) -> i32 {
    i32::try_from(lrand(&mut env.rng) % 0x7fff_ffff).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Number formatting (docs/gadgets/logic.md 1.7)

/// `0x410f93`: `x` as the Totaler, the Display and the Slider show it, ten
/// characters or more. `sticky` is the gadget's count of decimals
/// (`+0x196`), which only grows until reset.
pub fn format_number(x: f64, sticky: &mut u8) -> String {
    if x == 0.0 || (x > 1e-6 && x < 1e7) || (x < -1e-5 && x > -1e7) {
        let s = format!("{x:10.8}");
        let zeros = s.bytes().rev().take_while(|&c| c == b'0').count().min(8);
        let d = 8 - u8::try_from(zeros).unwrap_or(8);
        if *sticky < d {
            *sticky = d;
        }
        format!("{x:10.prec$}", prec = usize::from(*sticky))
    } else {
        let s = format_g(x, 4);
        if s.len() > 10 { format_g(x, 3) } else { s }
    }
}

/// `sprintf("%#10.<p>g", x)`, with two-digit exponents.
fn format_g(x: f64, p: usize) -> String {
    if !x.is_finite() {
        let s = if x.is_nan() {
            "+NAN"
        } else if x < 0.0 {
            "-INF"
        } else {
            "+INF"
        };
        return format!("{s:>10}");
    }
    let e = format!("{x:.prec$e}", prec = p - 1);
    let (mant, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let p = i32::try_from(p).unwrap_or(4);
    if exp < -4 || exp >= p {
        let sign = if exp < 0 { '-' } else { '+' };
        let s = format!("{mant}e{sign}{:02}", exp.unsigned_abs());
        format!("{s:>10}")
    } else {
        let prec = usize::try_from(p - 1 - exp).unwrap_or(0);
        let mut s = format!("{x:.prec$}");
        if prec == 0 {
            s.push('.');
        }
        format!("{s:>10}")
    }
}

#[cfg(test)]
pub mod bench {
    //! A test bench: a gadget under test with each knot wired to a source
    //! or a sink of its own.

    use crate::assets::Assets;
    use crate::gadget::{Behavior, Core, Gadget, Knot, Object, Tag, Unknown};
    use crate::sim::{Env, Io, Wire};
    use formats::tables::{KnotDef, KnotKind, Point};

    /// A source giving what it's told.
    #[derive(Default)]
    pub struct Source {
        pub b: Option<bool>,
        pub n: Option<f64>,
        /// Whether it gives an object, and which (maybe the null one).
        #[allow(clippy::option_option)]
        pub o: Option<Option<Object>>,
    }

    impl Source {
        pub fn number(x: f64) -> Source {
            Source {
                n: Some(x),
                ..Source::default()
            }
        }

        pub fn boolean(b: bool) -> Source {
            Source {
                b: Some(b),
                ..Source::default()
            }
        }

        pub fn object(o: Option<Object>) -> Source {
            Source {
                o: Some(o),
                ..Source::default()
            }
        }
    }

    impl Behavior for Source {
        fn has_bool(&self, _core: &Core, _knot: usize) -> bool {
            self.b.is_some()
        }

        fn bool_out(&self, _core: &Core, _knot: usize) -> bool {
            self.b.unwrap_or(false)
        }

        fn has_number(&self, _core: &Core, _knot: usize) -> bool {
            self.n.is_some()
        }

        fn number_out(&self, _core: &Core, _knot: usize) -> f64 {
            self.n.unwrap_or(0.0)
        }

        fn has_object(&self, _core: &Core, _knot: usize) -> bool {
            self.o.is_some()
        }

        fn object_out(&self, _core: &Core, _knot: usize) -> Option<Object> {
            self.o.clone().flatten()
        }

        super::any!();
    }

    /// The gadget under test (`core`, id 1, a placeholder at index 0) with
    /// knot `k`'s partner at index `k + 1`, wired with [`Bench::wire`].
    pub struct Bench {
        pub core: Core,
        pub gadgets: Vec<Gadget>,
        pub wires: Vec<Wire>,
        pub env: Env,
        pub assets: Assets,
    }

    impl Bench {
        pub fn with_knots(kinds: &[KnotKind]) -> Bench {
            let mut core = Core::new(Tag::new(b"test"), 0, 0);
            core.id = 1;
            core.knots = kinds
                .iter()
                .map(|&kind| Knot {
                    def: KnotDef {
                        at: Point { v: 0, h: 0 },
                        kind,
                    },
                    wire: None,
                })
                .collect();
            let mut gadgets = vec![Gadget {
                core: core.clone(),
                behavior: Box::new(Unknown::default()),
            }];
            for k in 0..kinds.len() {
                let mut c = Core::new(Tag::new(b"part"), 0, 0);
                c.id = u32::try_from(k).unwrap() + 2;
                gadgets.push(Gadget {
                    core: c,
                    behavior: Box::new(Source::default()),
                });
            }
            Bench {
                core,
                gadgets,
                wires: Vec::new(),
                env: Env::default(),
                assets: Assets::default(),
            }
        }

        /// One input (0) wired to a number, one output (1) wired.
        pub fn new(input: f64) -> Bench {
            let mut b = Bench::with_knots(&[KnotKind::Input, KnotKind::Output]);
            b.source(0, Source::number(input));
            b.wire(1);
            b
        }

        /// Wires knot `k` to its partner.
        pub fn wire(&mut self, k: usize) {
            if self.core.knots[k].wire.is_some() {
                return;
            }
            let (from, from_knot, to, to_knot) = if self.core.knots[k].is_input() {
                (k + 1, 0, 0, k)
            } else {
                (0, k, k + 1, 0)
            };
            self.wires.push(Wire {
                from,
                from_knot,
                to,
                to_knot,
                flag: 0,
                points: Vec::new(),
                changed: false,
            });
            self.core.knots[k].wire = Some(self.wires.len() - 1);
        }

        /// Input `k` wired to `s`, its flag as it was.
        pub fn source(&mut self, k: usize, s: Source) {
            self.wire(k);
            self.gadgets[k + 1].behavior = Box::new(s);
        }

        /// Input `k` wired to `s`, flagged changed.
        pub fn set(&mut self, k: usize, s: Source) {
            self.source(k, s);
            self.raise(k);
        }

        pub fn set_input(&mut self, x: f64) {
            self.set(0, Source::number(x));
        }

        /// Flags knot `k`'s wire changed.
        pub fn raise(&mut self, k: usize) {
            let w = self.core.knots[k].wire.unwrap();
            self.wires[w].changed = true;
        }

        /// Whether knot `k`'s wire is flagged changed (and clears it).
        pub fn fired(&mut self, k: usize) -> bool {
            self.core.knots[k]
                .wire
                .is_some_and(|w| std::mem::take(&mut self.wires[w].changed))
        }

        pub fn run<R>(&mut self, f: impl FnOnce(&mut Core, &mut Io<'_>) -> R) -> R {
            let mut io = Io {
                assets: &self.assets,
                gadgets: &self.gadgets,
                wires: &mut self.wires,
                env: &mut self.env,
            };
            f(&mut self.core, &mut io)
        }

        /// A new part's constructor: `init` then `reset`.
        pub fn fresh(&mut self, b: &mut dyn Behavior) {
            b.init(&mut self.core);
            b.reset(&mut self.core);
            b.settings_changed(&mut self.core);
        }

        /// Reset and start, as a run begins after a stop.
        pub fn start(&mut self, b: &mut dyn Behavior) {
            b.reset(&mut self.core);
            self.run(|c, io| b.start(c, io));
        }

        /// Ticks once at run tick `t`.
        pub fn tick_at(&mut self, b: &mut dyn Behavior, t: u32) {
            self.env.ticks = t;
            self.run(|c, io| b.tick(c, io));
        }

        /// Ticks at `t`, `t + step`, ... for `n` ticks; returns the next.
        pub fn ticks(&mut self, b: &mut dyn Behavior, t: u32, step: u32, n: u32) -> u32 {
            let mut t = t;
            for _ in 0..n {
                self.tick_at(b, t);
                t += step;
            }
            t
        }

        /// What the part gives on output `k`.
        pub fn out(&self, b: &dyn Behavior, k: usize) -> crate::gadget::Value {
            b.output(&self.core, k)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numbers_like_scanf() {
        assert_eq!(scan_f64(b" 12.5ft"), Some(12.5));
        assert_eq!(scan_f64(b"-3e2"), Some(-300.0));
        assert_eq!(scan_f64(b"abc"), None);
        assert_eq!(scan_f64(b""), None);
        assert_eq!(parse_num(b"1e-400"), 0.0);
        assert_eq!(parse_num(b"0"), 0.0);
        assert_eq!(parse_num(b"5"), 5.0);
        assert_eq!(scan_int(b" 42x"), Some(42));
        assert_eq!(scan_int(b"-7"), Some(-7));
        assert_eq!(scan_int(b"x"), None);
        assert_eq!(scan_int(b"4294967295"), Some(-1));
    }

    #[test]
    fn truncates_like_ftol() {
        assert_eq!(trunc(-2.7), -2);
        assert_eq!(trunc_sat(3e9), 0x7fff_fffe);
        assert_eq!(trunc_sat(-3e9), -0x7fff_fffe);
        assert!(!truthy(f64::NAN));
        assert!(truthy(-1.0));
    }

    #[test]
    fn random_is_the_rtl_lcg() {
        let mut s = 1u64;
        let a = lrand(&mut s);
        assert_eq!(s, 0x0000_15A4_0000_4E36);
        assert_eq!(a, 0x15A4);
        let mut env = Env::default();
        env.rng = 12345;
        let mut again = env.clone();
        let xs: Vec<i32> = (0..100).map(|_| random_int(&mut env)).collect();
        assert!(xs.iter().all(|&x| (0..0x7fff_ffff).contains(&x)));
        assert_eq!(xs, (0..100).map(|_| random_int(&mut again)).collect::<Vec<_>>());
    }

    #[test]
    fn formats_numbers() {
        let mut d = 0;
        assert_eq!(format_number(0.0, &mut d), "         0");
        assert_eq!(d, 0);
        assert_eq!(format_number(12.5, &mut d), "      12.5");
        assert_eq!(d, 1);
        // The decimals never go back down.
        assert_eq!(format_number(3.0, &mut d), "       3.0");
        assert_eq!(format_number(-0.25, &mut d), "     -0.25");
        assert_eq!(d, 2);
        assert_eq!(format_number(1.0 / 3.0, &mut d), "0.33333333");
        assert_eq!(d, 8);
        // Longer than ten: the Totaler shows the first ten.
        assert_eq!(format_number(1_234_567.5, &mut d), "1234567.50000000");
        let mut d = 0;
        assert_eq!(format_number(12_345_678.0, &mut d), " 1.235e+07");
        assert_eq!(format_number(-12_345_678.0, &mut d), "-1.235e+07");
        assert_eq!(format_number(1e-7, &mut d), " 1.000e-07");
        assert_eq!(format_number(-5e-6, &mut d), "-5.000e-06");
        assert_eq!(format_number(1e7, &mut d), " 1.000e+07");
        assert_eq!(format_number(-1.5e300, &mut d), "-1.50e+300");
        assert_eq!(format_number(1.5e300, &mut d), "1.500e+300");
        assert_eq!(d, 0);
        assert_eq!(format_g(1234.0, 4), "     1234.");
        assert_eq!(format_g(0.5, 4), "    0.5000");
    }
}
