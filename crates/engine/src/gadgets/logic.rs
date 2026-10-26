//! The logic parts: the gates (`AndG`, `OrGa`, `XorG`, `NotG`), the
//! Comparer (`Comp`), the splitters (`Spl2`/`Spl3`/`Spl4`), the Switcher
//! (`Mplx`) and the Win / Lose Checker (`Stop`). docs/gadgets/logic.md has
//! the original's code. None of them saves more than the base chunk.

use super::common::{Kinds, any, copy_input, point, scan_f64, src_bool, truthy};
use crate::gadget::{Behavior, Core, DrawCx};
use crate::sim::{Io, RunEnd};

// ---------------------------------------------------------------------------
// The gates

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GateOp {
    And,
    Or,
    Xor,
}

/// cGate (vtable `0x499a88`): a two-input gate, its output recomputed every
/// tick and flagged changed when an input's wire was.
#[derive(Debug)]
pub struct Gate {
    op: GateOp,
}

/// cAndGate.
#[must_use]
pub fn and_gate() -> Box<dyn Behavior> {
    Box::new(Gate { op: GateOp::And })
}

/// cOrGate.
#[must_use]
pub fn or_gate() -> Box<dyn Behavior> {
    Box::new(Gate { op: GateOp::Or })
}

/// cXorGate.
#[must_use]
pub fn xor_gate() -> Box<dyn Behavior> {
    Box::new(Gate { op: GateOp::Xor })
}

impl Behavior for Gate {
    /// `0x4411aa` / `0x44e6e2` / `0x45cfee`.
    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        let a = truthy(io.number(core, 0));
        let b = truthy(io.number(core, 1));
        core.state = match self.op {
            GateOp::And => a && b,
            GateOp::Or => a || b,
            GateOp::Xor => a != b,
        };
        let c0 = io.changed(core, 0);
        let c1 = io.changed(core, 1);
        if c0 || c1 {
            io.fire(core, 2);
        }
    }

    fn has_bool(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

/// cNotGate (not a cGate): reads its input the boolean way (`0x43adec`),
/// and an unwired one gives false.
#[derive(Debug, Default)]
pub struct NotGate;

#[must_use]
pub fn not_gate() -> Box<dyn Behavior> {
    Box::new(NotGate)
}

impl Behavior for NotGate {
    /// `0x44c54b`.
    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        if !io.wired(core, 0) {
            core.state = false;
            return;
        }
        core.state = !src_bool(io, core, 0);
        if io.changed(core, 0) {
            io.fire(core, 1);
        }
    }

    fn has_bool(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    any!();
}

// ---------------------------------------------------------------------------
// The Comparer

/// Each mode's picture (`0x4992d8`): <, >, =, ≠, ≤, ≥.
const COMPARE_FRAMES: [u16; 6] = [2, 1, 4, 3, 6, 5];

/// cComparison: whether the first number (input 0) is less, greater,
/// equal... (radio group 0) than the second (input 1).
#[derive(Debug, Default)]
pub struct Comparison;

#[must_use]
pub fn comparison() -> Box<dyn Behavior> {
    Box::new(Comparison)
}

impl Behavior for Comparison {
    /// `0x442bf2`.
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        // Modes past 5 aren't checked in the original.
        if let Some(&n) = COMPARE_FRAMES.get(usize::from(core.settings.radios[0])) {
            cx.frame(core.sheet, n, core.h, core.v);
        }
    }

    /// `0x442ac3`.
    #[allow(clippy::float_cmp)]
    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        let a = io.number(core, 0);
        let b = io.number(core, 1);
        // An unordered compare (a NaN) sets CF and ZF on the x87.
        let nan = a.is_nan() || b.is_nan();
        core.state = match core.settings.radios[0] {
            0 => a < b || nan,
            1 => a > b,
            2 => a == b || nan,
            3 => a != b && !nan,
            4 => a <= b || nan,
            5 => a >= b,
            _ => core.state,
        };
        let c0 = io.changed(core, 0);
        let c1 = io.changed(core, 1);
        if c0 || c1 {
            io.fire(core, 2);
        }
    }

    fn has_bool(&self, _core: &Core, _knot: usize) -> bool {
        true
    }

    /// `0x4428c0`: Equal to.
    fn init(&mut self, core: &mut Core) {
        core.settings.radios[0] = 2;
    }

    any!();
}

// ---------------------------------------------------------------------------
// The splitters

/// cSplitter: passes its input on to two, three or four outputs, a tick
/// late, copying the value when the input changes.
#[derive(Debug)]
pub struct Splitter {
    /// Its frame of sheet 1019 (1 to 3 for 2 to 4 ways).
    frame: u16,
    /// What the source gave at the last copy (`+0x197..0x199`); reset
    /// keeps them.
    kinds: Kinds,
}

fn splitter(frame: u16) -> Box<dyn Behavior> {
    Box::new(Splitter {
        frame,
        kinds: (true, false, false),
    })
}

/// cSplitter2.
#[must_use]
pub fn splitter2() -> Box<dyn Behavior> {
    splitter(1)
}

/// cSplitter3.
#[must_use]
pub fn splitter3() -> Box<dyn Behavior> {
    splitter(2)
}

/// cSplitter4.
#[must_use]
pub fn splitter4() -> Box<dyn Behavior> {
    splitter(3)
}

impl Behavior for Splitter {
    /// `0x4540f0`.
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, self.frame, core.h, core.v);
    }

    fn bounds_frame(&self) -> u16 {
        self.frame
    }

    /// `0x45402a`.
    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        let c = io.changed(core, 0);
        let v = io.value_changed(core, 0);
        if !(c || v) {
            return;
        }
        if c {
            for k in 1..=4 {
                io.fire(core, k);
            }
        }
        if let Some(kinds) = copy_input(io, core, 0) {
            self.kinds = kinds;
        }
    }

    fn has_bool(&self, _core: &Core, _knot: usize) -> bool {
        self.kinds.0
    }

    fn has_number(&self, _core: &Core, _knot: usize) -> bool {
        self.kinds.1
    }

    fn has_object(&self, _core: &Core, _knot: usize) -> bool {
        self.kinds.2
    }

    any!();
}

// ---------------------------------------------------------------------------
// The Switcher

/// cMultiplexer: passes on input 1 or input 2, as the selector (input 0) is
/// false or true, with a train running along the track.
#[derive(Debug)]
pub struct Multiplexer {
    /// The input passed on, 1 or 2 (`+0x197`).
    selected: usize,
    /// What it gave at the last copy (`+0x198..0x19a`).
    kinds: Kinds,
}

impl Default for Multiplexer {
    fn default() -> Multiplexer {
        Multiplexer {
            selected: 1,
            kinds: (true, false, false),
        }
    }
}

#[must_use]
pub fn multiplexer() -> Box<dyn Behavior> {
    Box::new(Multiplexer::default())
}

impl Behavior for Multiplexer {
    /// `0x44b7b8`.
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        let n = if cx.running {
            (if self.selected == 2 { 8 } else { 3 }) + u16::from(core.phase)
        } else {
            1
        };
        cx.frame(core.sheet, n, core.h, core.v);
    }

    /// `0x44b9cf`.
    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        let mut dirty = false;
        let c = io.changed(core, 0);
        if c | io.value_changed(core, 0) {
            let sel = if truthy(io.number(core, 0)) { 2 } else { 1 };
            if self.selected != sel {
                io.play(1036);
                self.selected = sel;
                dirty = true;
            }
        }
        // Only the selected input's flag and cache are taken.
        let c = io.changed(core, self.selected);
        let v = io.value_changed(core, self.selected);
        if c || v {
            dirty = true;
        }
        if dirty {
            if c {
                io.fire(core, 3);
            }
            core.phase = 1;
            // Unwired: a false boolean.
            self.kinds = copy_input(io, core, self.selected).unwrap_or((true, false, false));
        }
        if core.phase != 0 && io.throttle(core, 15) {
            core.phase += 1;
            if core.phase == 5 {
                core.phase = 0;
            }
        }
    }

    /// `0x44b96f`.
    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        core.empty_cache = true;
        *self = Multiplexer::default();
    }

    fn has_bool(&self, _core: &Core, _knot: usize) -> bool {
        self.kinds.0
    }

    fn has_number(&self, _core: &Core, _knot: usize) -> bool {
        self.kinds.1
    }

    fn has_object(&self, _core: &Core, _knot: usize) -> bool {
        self.kinds.2
    }

    any!();
}

// ---------------------------------------------------------------------------
// The Win / Lose Checker

/// cStopper: ends the run when its input is true or false (mode 0), or
/// compares as set against a number (mode 1), with a flag animation and
/// the win or lose sound; a win solves the puzzle.
#[derive(Debug, Default)]
pub struct Stopper {
    /// Triggered (`+0x197`).
    triggered: bool,
    /// When, in run ticks (`+0x198`, the original's absolute clock).
    triggered_at: u32,
}

#[must_use]
pub fn stopper() -> Box<dyn Behavior> {
    Box::new(Stopper::default())
}

impl Behavior for Stopper {
    /// `0x4578c8`.
    fn draw(&self, core: &Core, cx: &mut DrawCx<'_>) {
        cx.frame(core.sheet, 1, core.h, core.v);
        let (h, v) = point(cx, core.sheet, 0);
        let n = if core.phase == 0 { 2 } else { u16::from(core.phase) + 2 };
        cx.frame(core.sheet, n, core.h + h, core.v + v);
    }

    /// `0x457ba2`. The original plays the end animation in a loop inside
    /// the tick, which holds up every other part and the editor until the
    /// run ends; here each tick is one pass of that loop, with
    /// [`crate::sim::Env::exclusive`] holding up the rest.
    // TODO: the cheat (the mouse at the screen's top left with W, I and N
    // held) triggers it too; the editor doesn't report held keys yet.
    #[allow(clippy::float_cmp)]
    fn tick(&mut self, core: &mut Core, io: &mut Io<'_>) {
        let was = self.triggered;
        if !io.wired(core, 0) {
            return;
        }
        let radios = core.settings.radios;
        if self.triggered {
            io.env.exclusive = Some(core.id);
            if io.elapsed().wrapping_sub(self.triggered_at) > u32::from(core.phase) * 25 {
                core.phase += 1;
                if core.phase == 8 {
                    core.phase = 0;
                    self.triggered = false;
                    io.env.exclusive = None;
                    io.env.end_run = Some(RunEnd {
                        solved: radios[3] == 0,
                        sound: 128,
                    });
                }
            }
        } else if radios[0] == 0 {
            // "True" fires on true, "False" on false.
            self.triggered = src_bool(io, core, 0) == (radios[1] == 0);
        } else {
            let (x, n) = (io.number(core, 0), core.number);
            self.triggered = match radios[2] {
                0 => x == n,
                1 => x != n,
                2 => x > n,
                3 => x < n,
                4 => x >= n,
                5 => x <= n,
                _ => false,
            };
        }
        if self.triggered && !was {
            self.triggered_at = io.elapsed();
            io.play(if radios[3] == 0 { 1051 } else { 1151 });
            core.phase = 1;
        }
    }

    /// `0x4578b0`.
    fn reset(&mut self, core: &mut Core) {
        core.base_reset();
        self.triggered = false;
    }

    /// `0x457b77`: the number to compare with, kept 0 if unreadable.
    fn start(&mut self, core: &mut Core, io: &mut Io<'_>) {
        io.base_start(core);
        if let Some(n) = scan_f64(core.settings.label(0)) {
            core.number = n;
        }
    }

    /// `0x4576e0`.
    fn init(&mut self, core: &mut Core) {
        core.settings.set_label(0, b"1");
    }

    any!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gadget::{Object, Value};
    use crate::gadgets::common::bench::{Bench, Source};
    use formats::tables::KnotKind::{Input, Output};

    fn gate_bench() -> Bench {
        let mut b = Bench::with_knots(&[Input, Input, Output]);
        b.wire(2);
        b
    }

    fn gate_truth(make: fn() -> Box<dyn Behavior>) -> Vec<bool> {
        let mut g = make();
        let mut out = Vec::new();
        for (a, c) in [(0.0, 0.0), (0.0, 1.0), (1.0, 0.0), (1.0, 1.0)] {
            let mut b = gate_bench();
            b.source(0, Source::number(a));
            b.source(1, Source::number(c));
            b.tick_at(g.as_mut(), 0);
            out.push(b.core.state);
        }
        out
    }

    #[test]
    fn gate_truth_tables() {
        assert_eq!(gate_truth(and_gate), [false, false, false, true]);
        assert_eq!(gate_truth(or_gate), [false, true, true, true]);
        assert_eq!(gate_truth(xor_gate), [false, true, true, false]);
    }

    #[test]
    fn gates_read_numbers_and_pass_on_flags() {
        let mut g = and_gate();
        let mut b = gate_bench();
        // Any non-zero number is true, a NaN is false, unwired is 0.
        b.source(0, Source::number(-3.5));
        b.source(1, Source::number(0.25));
        b.tick_at(g.as_mut(), 0);
        assert!(b.core.state);
        assert!(!b.fired(2));
        b.set(1, Source::number(f64::NAN));
        b.tick_at(g.as_mut(), 1);
        assert!(!b.core.state);
        assert!(b.fired(2));
        // A source with no typed output reads -1: true.
        b.set(1, Source::default());
        b.tick_at(g.as_mut(), 2);
        assert!(b.core.state);
        // Both flags are taken.
        b.raise(0);
        b.raise(1);
        b.tick_at(g.as_mut(), 3);
        assert!(b.fired(2));
        b.tick_at(g.as_mut(), 4);
        assert!(!b.fired(2));
        let mut b = gate_bench();
        let mut o = or_gate();
        b.tick_at(o.as_mut(), 0);
        assert!(!b.core.state);
    }

    #[test]
    fn not_gate_reads_booleans_first() {
        let mut g = not_gate();
        let mut b = Bench::with_knots(&[Input, Output]);
        b.wire(1);
        // Unwired: false.
        b.tick_at(g.as_mut(), 0);
        assert!(!b.core.state);
        b.set(0, Source::boolean(false));
        b.tick_at(g.as_mut(), 1);
        assert!(b.core.state);
        assert!(b.fired(1));
        // Nothing typed: false, so the NOT gives true.
        b.source(0, Source::default());
        b.tick_at(g.as_mut(), 2);
        assert!(b.core.state);
        assert!(!b.fired(1));
        // A boolean beats a number.
        b.source(
            0,
            Source {
                b: Some(true),
                n: Some(0.0),
                o: None,
            },
        );
        b.tick_at(g.as_mut(), 3);
        assert!(!b.core.state);
        b.source(0, Source::number(2.0));
        b.tick_at(g.as_mut(), 4);
        assert!(!b.core.state);
        b.source(0, Source::object(Some(Object::Picture(1))));
        b.tick_at(g.as_mut(), 5);
        assert!(!b.core.state);
    }

    #[test]
    fn comparer_modes() {
        let mut c = comparison();
        let mut b = gate_bench();
        b.fresh(c.as_mut());
        assert_eq!(b.core.settings.radios[0], 2);
        let cases: [(u8, f64, f64, bool); 8] = [
            (0, 1.0, 2.0, true),
            (1, 1.0, 2.0, false),
            (2, 2.0, 2.0, true),
            (3, 2.0, 2.0, false),
            (4, 2.0, 2.0, true),
            (5, 1.0, 2.0, false),
            (2, f64::NAN, 2.0, true),
            (5, f64::NAN, 2.0, false),
        ];
        for (mode, x, y, want) in cases {
            b.core.settings.radios[0] = mode;
            b.source(0, Source::number(x));
            b.source(1, Source::number(y));
            b.tick_at(c.as_mut(), 0);
            assert_eq!(b.core.state, want, "mode {mode}: {x} vs {y}");
        }
        b.raise(1);
        b.tick_at(c.as_mut(), 1);
        assert!(b.fired(2));
    }

    #[test]
    fn splitter_copies_on_change() {
        let mut s = splitter3();
        let mut b = Bench::with_knots(&[Input, Output, Output, Output]);
        for k in 1..4 {
            b.wire(k);
        }
        b.source(0, Source::number(4.0));
        b.start(s.as_mut());
        // Primed at start: no change yet, and the copy is empty.
        b.tick_at(s.as_mut(), 0);
        assert_eq!(b.out(s.as_ref(), 1), Value::Bool(false));
        // A value change copies without flagging.
        b.source(0, Source::number(5.0));
        b.tick_at(s.as_mut(), 1);
        assert_eq!(b.out(s.as_ref(), 2), Value::Number(5.0));
        assert!(!b.fired(1));
        // A flag passes to every output.
        b.raise(0);
        b.tick_at(s.as_mut(), 2);
        assert!(b.fired(1) && b.fired(2) && b.fired(3));
        // The kinds survive a reset, the values don't.
        s.reset(&mut b.core);
        assert_eq!(b.out(s.as_ref(), 1), Value::Number(0.0));
        assert_eq!(s.bounds_frame(), 2);
    }

    #[test]
    fn switcher_selects_and_passes_on() {
        let mut m = multiplexer();
        let mut b = Bench::with_knots(&[Input, Input, Input, Output]);
        b.wire(3);
        b.source(0, Source::boolean(false));
        b.source(1, Source::number(10.0));
        b.source(2, Source::number(20.0));
        b.start(m.as_mut());
        // The cache starts "none": the selected input reads as changed.
        b.tick_at(m.as_mut(), 0);
        assert_eq!(b.out(m.as_ref(), 3), Value::Number(10.0));
        assert_eq!(b.core.phase, 1);
        assert!(!b.fired(3));
        // Switch to input 2: copied, but not flagged unless its wire is.
        b.set(0, Source::boolean(true));
        b.tick_at(m.as_mut(), 1);
        assert_eq!(b.out(m.as_ref(), 3), Value::Number(20.0));
        assert!(!b.fired(3));
        // Input 1 isn't watched now.
        b.set(1, Source::number(11.0));
        b.tick_at(m.as_mut(), 2);
        assert_eq!(b.out(m.as_ref(), 3), Value::Number(20.0));
        assert!(!b.fired(3));
        b.set(2, Source::number(21.0));
        b.tick_at(m.as_mut(), 3);
        assert_eq!(b.out(m.as_ref(), 3), Value::Number(21.0));
        assert!(b.fired(3));
        // The train: a step every 16 ticks, round to 0 after 4.
        let mut b2 = Bench::with_knots(&[Input, Input, Input, Output]);
        b2.source(1, Source::number(1.0));
        b2.start(m.as_mut());
        b2.tick_at(m.as_mut(), 100);
        assert_eq!(b2.core.phase, 2);
        for t in [116, 132, 148] {
            b2.tick_at(m.as_mut(), t);
        }
        assert_eq!(b2.core.phase, 0);
    }

    #[test]
    fn switcher_unwired_input_is_false() {
        let mut m = multiplexer();
        let mut b = Bench::with_knots(&[Input, Input, Input, Output]);
        b.source(0, Source::number(1.0));
        b.start(m.as_mut());
        b.tick_at(m.as_mut(), 0);
        assert_eq!(b.out(m.as_ref(), 3), Value::Bool(false));
    }

    fn stopper_bench(src: Source) -> (Bench, Box<dyn Behavior>) {
        let mut s = stopper();
        let mut b = Bench::with_knots(&[Input]);
        b.fresh(s.as_mut());
        b.source(0, src);
        (b, s)
    }

    #[test]
    fn stopper_wins_on_true() {
        let (mut b, mut s) = stopper_bench(Source::boolean(false));
        b.start(s.as_mut());
        b.tick_at(s.as_mut(), 0);
        assert_eq!(b.core.phase, 0);
        b.source(0, Source::boolean(true));
        b.tick_at(s.as_mut(), 10);
        assert_eq!(b.core.phase, 1);
        assert_eq!(b.env.exclusive, None);
        // The next tick starts the end, holding up the rest.
        b.tick_at(s.as_mut(), 11);
        assert_eq!(b.env.exclusive, Some(1));
        let mut t = 11;
        while b.env.end_run.is_none() {
            t += 1;
            b.tick_at(s.as_mut(), t);
        }
        // A frame past 25, 50, ... 175 ticks.
        assert_eq!(t, 10 + 176);
        assert_eq!(b.env.exclusive, None);
        assert_eq!(
            b.env.end_run,
            Some(RunEnd {
                solved: true,
                sound: 128
            })
        );
    }

    #[test]
    fn stopper_false_and_numbers() {
        // "False" fires at once on a false input.
        let (mut b, mut s) = stopper_bench(Source::number(0.0));
        b.core.settings.radios[1] = 1;
        b.start(s.as_mut());
        b.tick_at(s.as_mut(), 0);
        assert_eq!(b.core.phase, 1);

        // Number mode: greater than 10000, a lose.
        let (mut b, mut s) = stopper_bench(Source::number(10000.0));
        b.core.settings.radios = [1, 0, 2, 1];
        b.core.settings.set_label(0, b"10000");
        b.start(s.as_mut());
        assert_eq!(b.core.number, 10000.0);
        b.tick_at(s.as_mut(), 0);
        assert_eq!(b.core.phase, 0);
        b.source(0, Source::number(10001.0));
        b.tick_at(s.as_mut(), 1);
        assert_eq!(b.core.phase, 1);
        let mut t = 1;
        while b.env.end_run.is_none() {
            t += 1;
            b.tick_at(s.as_mut(), t);
        }
        assert_eq!(
            b.env.end_run,
            Some(RunEnd {
                solved: false,
                sound: 128
            })
        );

        // Unwired: nothing.
        let mut s = stopper();
        let mut b = Bench::with_knots(&[Input]);
        b.fresh(s.as_mut());
        b.core.settings.radios[1] = 1;
        b.start(s.as_mut());
        b.tick_at(s.as_mut(), 0);
        assert_eq!(b.core.phase, 0);
    }
}
