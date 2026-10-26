# The logic and number parts

The gates (`AndG`, `OrGa`, `XorG`, `NotG`), the number crunchers (`Addr`,
`Subr`, `Mulr`, `Divr`, `1ncr`), `Comp`, `Conv`, the splitters
(`Spl2`/`Spl3`/`Spl4`), `Mplx`, `Cntr`, `Ttlr`, `Stop`, `rand`, `Coin`,
`cnst` and `MiCo`, as `WIDGET.EXE` (Win95) implements them. The dead class
`cFirstThru` (`1thr`) is at the end. Read
[`../gadget-model.md`](../gadget-model.md) first; this file corrects it in
places (last section). All addresses are in `WIDGET.EXE`. Constants were
read from the binary (doubles unless said otherwise; "ext" means an 80-bit
x87 constant).

**Saved data.** None of these classes overrides slot 0x3c (size) or 0x40
(save). Every one saves exactly the 0x150-byte base chunk, and every load
constructor reads only that base (`0x41d166`). All per-part state is
runtime state. This was checked against every use in
`orig/WIN95/WIDGET/FUNWIDGE/*.WGT`: 52 `AndG`, 14 `OrGa`, 3 `NotG`, 6
`Addr`, 7 `Subr`, 18 `Mulr`, 1 `Divr`, 2 `1ncr`, 52 `Comp`, 68 `Spl2`, 9
`Spl3`, 24 `Spl4`, 19 `Mplx`, 3 `Cntr`, 5 `Ttlr`, 3 `Stop`, 5 `rand`, 7
`cnst` and 66 `MiCo` chunks, all of size `0x150`. Their settings blocks
agree with the field meanings below. For example, SINCOS.WGT's `1ncr`s
have number field 0 = 3 (Sin) and 4 (Cos), SLOTMACH.WGT's `Stop` has
radios `[1,0,4,0]` and text `"10000"`, its `rand`s have radio 0 = 1
(custom) with texts `"1"`/`"5"` and `"11"`/`"15"`, RIDDLE.WGT's `cnst`
has check bits `0x4` (Auto Reset), and the `Comp`s have radio 0 = 2 (=) or
5 (>=). The chunk layout is in gadget-model.md ("Saved form"). The settings
block is at chunk offset 8 and is stored big-endian.

---

## 1. Shared mechanics

### 1.1 Notation used in all pseudocode

| name | address | meaning |
|---|---|---|
| `wire(k)` | `0x43d0a9(knots + 10k)` | the `cConnection*` at knot `k`, or null |
| `input(k)` | `0x41e56e(this, k)` | read input knot `k` as a double (§1.2) |
| `changed(k)` | `0x41e713(this, k)` | read **and clear** the wire changed flag at knot `k` (§1.4) |
| `value_changed(k)` | `0x41e62a(this, k)` | re-read `input(k)` and compare it with the knot cache (§1.3) |
| `fire(k)` | `0x41e6cf(this, k)` | set the changed flag of the wire at knot `k` (§1.4) |
| `throttle(n)` | `0x41e762(this, n)` | `now = doc.+0x4b; if (u32)(now - this.+0x152) > n { this.+0x152 = now; return true } return false` |
| `clock()` | `0x4113a8` | absolute Mac ticks: `trunc(timeGetTime() * 0.06)` |
| `play(id)` | `0x4402cc(id, 0)` | play WWSOUND type-1006 sound `id` once (no-op if sound is off) |
| `invalidate()` | slot 0x0c (`0x41e359`) | queue the gadget's bounds for redraw |
| `draw_broken()` | slot 0x10 (`0x41ed9d`) | draw the "broken" overlay picture, centred |
| `draw_frame(sheet, n, pt)` | `0x422d32` | blit frame `n` (from 1) of an HR sheet at Mac point `pt` |
| `pos` | `+0x14` (i16 v) / `+0x16` (i16 h) | the gadget's top left on the worksheet |
| `P[i]` | `0x4114da(&table, i)` | point `i` of the gadget's type-1010 point list (resource id = the sheet id), as `(v, h)` relative to `pos` |

`doc.+0x4b` is the number of Mac ticks since the run's first tick (set by
the tick loop `0x42498f`). `+0x152` is zeroed by the base init `0x41cf8a`
and is **never** reset. On a second run, `now - +0x152` therefore wraps
(u32) and the first `throttle` passes at once. The comparison is strict
and unsigned, so `throttle(9)` passes when 10 or more ticks have passed.

All the getters (slots 0x24 to 0x38) are called as `slot(this, u16 knot)`.
The knot is the source's output knot (`conn+8`). Every class in this file
ignores it and returns the same value for every knot.

### 1.2 Reading an input: `0x41e56e` (read_input)

The result is returned in ST0. `src` is the gadget at the wire's `+0`, and
`k` is the wire's `+8` (the source's output knot).

```
fn read_input(this, knot) -> f64:
    w = wire(knot)
    if w == null:                     return 0.0        // unwired
    if src.has_object(k):             // slot 0x34
        return src.object(k) != 0 ? 1.0 : 0.0          // slot 0x38
    if src.has_number(k):             // slot 0x2c
        return src.number(k)          // slot 0x30
    if src.has_bool(k):               // slot 0x24
        return src.bool(k) ? 1.0 : 0.0                  // slot 0x28
    return -1.0                       // source with no typed output
```

The constants are at `0x41e612` (0.0), `0x41e61a` (1.0) and `0x41e622`
(-1.0). The priority is **object > number > bool**. The number branch is
`0x43c13c`. It cannot fall through, because it is only taken when
`has_number` is true.

Three related helpers have different priorities:

* `src_bool` `0x43adec(w)`: **bool > number > object**. Returns
  `bool(k)` if `has_bool`, else `number(k) != 0.0` if `has_number`, else
  `object(k) != 0` if `has_object`, else false. Used by NotG and by
  the bool mode of Stop.
* `src_number` `0x43c13c(w)`: number > bool (1.0/0.0) > object (1.0 if
  non-null, else 0.0) > 0.0.
* `src_object` `0x43aef3(w)`: `has_object ? object(k) : 0`.

`copy_input` `0x41e79c(this, knot)` is used by the splitters and Mplx:

```
fn copy_input(this, knot) -> bool:
    w = wire(knot)
    if w == null:
        this.+0x13e = 0; this.+0x146 = 0.0; this.+0x14e = 0
        return false
    this.+0x13e = src_bool(w)       // u8
    this.+0x146 = src_number(w)     // f64
    this.+0x14e = src_object(w)     // u32 handle/pointer
    return true
```

Comparisons of ST0 against 0.0 use `fcomp` with `sahf; je`. An unordered
result (NaN) sets ZF, so **NaN counts as zero/false** wherever the code
tests "!= 0". No part in this file produces a NaN on purpose.

### 1.3 The per-knot value cache (`+0x156`, f64 per knot) and `0x41e62a`

```
const NONE = f64::from_bits(0xADDC4A11FF61B241)   // = -8.888e-88, an ordinary negative double, not a NaN

fn value_changed(this, knot) -> bool:     // 0x41e62a
    v = read_input(this, knot)
    if cache[knot] == v: return false      // exact compare, after rounding v to f64
    cache[knot] = v
    return wire(knot) != null
```

An unwired input is read as 0.0. The cache is updated, but the function
returns false: an unwired knot never reports a change.

`NONE` is written only by the start routine below, and only for gadgets
whose `+0x13d` flag is 1. After that, the first `value_changed` on a wired
input returns true, because a real value is almost never exactly
-8.888e-88.

### 1.4 Wire changed flags: `0x41e6cf` / `0x41e713` / `0x41e697`

The flag is the wire's `+0x2a`. A knot holds at most one wire, so an
output feeds exactly one consumer. That is why the splitters exist.

```
fn fire(this, k) -> bool:            // 0x41e6cf  mark_output_changed
    if k >= knot_count: return false
    w = wire(k); if w == null: return false
    w.changed = 1; return true

fn changed(this, k) -> bool:         // 0x41e713  take_input_changed
    if k >= knot_count: return false
    w = wire(k); if w == null: return false
    r = w.changed; w.changed = 0; return r

fn clear_flag(this, k):              // 0x41e697
    if k < knot_count and wire(k): wire(k).changed = 0
```

The flag is set by the producer and cleared by the consumer when it reads
it. A producer does not need to `fire` for its value to be seen, because
values are always pulled live. `fire` is only a "this is a new event"
signal, which the consumers below treat in different ways.

### 1.5 Reset (slot 0x1c, base `0x41cef4`) and start (slot 0x20, base `0x41e50a`)

```
fn gadget_reset(this):               // 0x41cef4
    +0x13e = 0                        // bool output
    +0x142 = +0x146 = +0x14a = +0x14e = 0   // i32 / f64 number output / object
    +0x141 = 0                        // animation step
    +0x13f = 1
    if +0x140 != 0: +0x140 = 0; invalidate()   // error flag ("broken")
    +0x196 = 0                        // sticky decimals for format_number (§1.7)

fn gadget_start(this):               // 0x41e50a
    if +0x13d == 0:                   // default
        prime_cache(this)             // 0x41e4c8
    else:
        for k in 0..knot_count: cache[k] = NONE; clear_flag(this, k)

fn prime_cache(this):                // 0x41e4c8
    for k in 0..knot_count: cache[k] = read_input(this, k)   // wire flags NOT touched
```

`prime_cache` also "reads" output knots. Those go through the gadget's own
wire, whose source is the gadget itself, so they call its own getters. The
result is harmless.

When each is called:

* **Run** (`0x430e70` → `0x424a12`): for each gadget in list order:
  `start()`, then `invalidate()`. Reset is not called here.
* **Stop** (`0x430ee7` → `0x424a94`): for each gadget, `reset()`. Then
  every wire's changed flag is cleared (`0x43ca73`), and `doc.+0x4f`
  (the first-tick time) is set to 0.
* Both constructors call `reset()` (`0x41d076` / `0x41d166` call slot
  0x1c while the vtable is still cGadget's, so this is the base reset).
  Several subclasses call it again with their own vtable, noted per part.
* **Tick** (`0x42498f`): sets `doc.+0x4b`, then calls slot 0x44 on each
  gadget in list order. It stops early if the app leaves run mode
  (`DAT_00493dde == 1`), which happens when a Stop part fires.

`+0x13d = 1` is set by: the number crunchers, `1ncr`, `Conv` and `Mplx`.
All the other parts here keep 0. With 0, they prime their caches with the
current input values at start, so `value_changed` only fires on a real
change.

### 1.6 Settings dialog ↔ settings block (`+0x35`, block offsets)

The dialog is RT_DIALOG `<gadget id>` in WIDGET.EXE. The dialog class
(`0x41a5xx` init, loop at `0x41a7xx`) walks **control ids 1 to 169 in
ascending order** and wraps each control by class:

* `BUTTON` with style `BS_CHECKBOX`/`BS_AUTOCHECKBOX` (2/3): appended to
  the check-box list. The n-th check box by id is **bit n** of block
  `+0x00` (u32).
* `BUTTON` with style `BS_RADIOBUTTON`/`BS_AUTORADIOBUTTON` (4/9): the
  **last character of its caption is a group tag**. It is stripped from the
  displayed text. A group is created at the first button with a new tag, so
  group index = order of first appearance by id. The value is the index of
  the checked button within its group (buttons in id order), stored as a u8
  at block `+0x04 + group`.
* `COMBOBOX`: the n-th combo box by id is "number field" n. It holds the
  selected index as a u16, zero-extended into the u32 at block
  `+0x08 + 4n`. The combo strings come from WIDGET type 1022 `<gadget id>`.
  Fill: `0x419e93` sets `CB_SETCURSEL`. Read-back masks with `0xffff`
  (`0x41e851`).
* `EDIT`: the n-th edit box by id is text field n, `char[60]` at block
  `+0x18 + 60n` (gadget `+0x4d + 60n`). The template text of an edit box
  is an input filter (`0x419c1a`): `#` = numeric, `.` = decimal point
  allowed, `-` = minus allowed, a decimal number = maximum length, `A` = a
  flag at `+0x111` (probably upper case, not checked). The edit box is
  then cleared.
* Push buttons, statics and owner-draw buttons (ids 102 "Pic", 103
  "Doc", 150) carry no settings.

Slot 0x48 (`0x41ea4d`) fills the controls from the block. Slot 0x4c
(`0x41ea2e` → `0x41e851`) reads them back. Slot 0x50 ("settings
changed") runs from the gadget's live-update callback `+0x2b`
(`0x41eb78`, on every control change: it reads the dialog into the block,
then calls slot 0x50), from the Cancel path of the dialog
(`0x41a686`: id 1 OK → slot 0x4c; id 2 Cancel → slot 0x50; whether the
block is restored before that call was not traced), and from some
constructors.

### 1.7 Number formatting: `0x410f93` (format_number)

`format_number(x: f64, sticky: &mut u8) -> &str`. The result is in a
static buffer at `0x4a2699`, and `sprintf` is `0x47f4a0`. Of the parts in
this file, only the Totaler uses it (`sticky` = `this.+0x196`). The
Display and Slider parts also call it.

```
if x == 0.0 or (x > 1e-6 and x < 1e7) or (x < -1e-5 and x > -1e7):
    // NB: the thresholds are asymmetric: 1e-6 (double) on the positive side, -1e-5 (double) on the negative.
    // 1e7 / -1e7 are floats; exact anyway.
    s = sprintf("%10.8f", x)
    d = 8
    p = last char of s
    while *p == '0': d -= 1; p -= 1           // stops at '.', so d >= 0
    if *sticky < d: *sticky = d                // never decreases
    s = sprintf("%10.Nf", x) with N = *sticky  // the format "%10.Xf" at 0x490414, X patched to '0'+N
else:
    s = sprintf("%#10.4g", x)
    if strlen(s) > 10: s = sprintf("%#10.3g", x)
```

These are C `printf` semantics (Borland RTL). `%#g` keeps trailing zeros
and the decimal point. The exponent has at least two digits (`1.235e+07`).
Two-digit exponents for Borland are an assumption; MSVC prints three. The
result can be longer than 10 characters, for example `"%10.8f"` of
1234567.5 with sticky 8. The Totaler draws only the first 10.

### 1.8 Floating point environment

At start-up the Borland RTL sets the x87 control word to `0x1332`
(`0x480eac`, value at `0x4a1910`). That means 64-bit (extended) precision
and round-to-nearest. Invalid-operation, zero-divide and overflow
exceptions are **unmasked**; precision, underflow and denormal are
masked. As a result:

* Expressions are evaluated in 80-bit precision and rounded to f64 when
  stored to memory (every `fstp qword`). Where an expression has more than
  one operation before the store (Conv `x*1.8+32`, `(x-32)*ext(5/9)`,
  rand's scaling), the f64 result may differ by one ulp from pure f64
  arithmetic. For exact output, use an extended-precision intermediate or
  check the specific cases. The constants given as "ext" below are 80-bit.
* A real overflow or divide by zero would trap. The parts guard against
  divide by zero and multiplication overflow (Number crunchers, 1ncr
  square). Addition and subtraction overflow (for example 1e308 + 1e308) is
  not guarded, and would raise a floating-point exception in the original.
  The handling (an RTL `SIGFPE` abort) was not traced. Treat it as "never
  happens" and saturate to ±inf, or match what you decide for the rest of
  the port.
* `trunc` = `0x480ec4` (`fistp` with RC=chop). `ftol_clamped` =
  `0x439e3d(x)`: `x >= 2147483647.0 → 0x7ffffffe`;
  `x <= -2147483646.0 → 0x80000002`; else `trunc(x)`.
* `floor` = `0x480e90` (`frndint` with RC=down; |x| ≥ 2^52 is returned
  unchanged). `sqrt` = `0x481228`, `sin` = `0x4811e4`, `cos` =
  `0x480c00`, `tan` = `0x481264`. These are x87 `fsqrt`/`fsin`/`fcos`/
  `fptan`. For |x| ≥ 2^52 the RTL reports a TLOSS error through `matherr`;
  the result in that case was not traced.

### 1.9 The random number generator

```
state: u64 (lo = DAT_004a198c, hi = DAT_004a1990), initial lo=1, hi=0
seed at start-up (0x415018, registered in the RTL init table at 0x4a20d4):
    state = (u64)(u32)time(NULL)          // 0x481500: lo = seed, hi = 0

fn lrand() -> u32:                        // 0x481530 (Borland RTL)
    state = state * 0x0000_15A4_0000_4E35 + 1   (mod 2^64)
    return (u32)(state >> 32) & 0x7fffffff

fn random_int() -> i32:                   // 0x415008, used by rand, Coin, and 0x4534c2 (probably cSoundTransformer)
    return (i32)lrand() % 0x7fffffff      // range 0..0x7ffffffe, never negative
```

The asm computes the 64-bit product as `lo*0x4E35 + ((lo*0x15A4 +
hi*0x4E35) << 32)` with carry. That is exactly multiplication by the
64-bit constant `0x000015A400004E35`. There is also a separate 32-bit
`rand()` at `0x481514` (`seed = seed*0x15A4E35+1; (seed>>16)&0x7fff`)
that shares `DAT_004a198c`. No part in this file uses it.

### 1.10 Drawing conventions

Every `draw` (slot 0x04) first intersects `bounds + pos` with the clip
rect and returns if the result is empty. It then calls
`draw_frame(sheet, n, pos + offset)`. The sheets are WWGADGET type 1004.
Every frame used here has rect top-left (0,0), so the frame's top-left
lands at `pos + offset`. Offsets come from the type-1010 point list of the
same id (`P[i]`), loaded at class init with `0x41141f`. `bounds` (`+0x18`)
is set at init from the frame named per part.

A fresh constructor (dropped from the tray at point `d`) places the gadget
so that `d` is the centre of `bounds`:
`pos.v = d.v - (top + (bottom-top)/2)`, `pos.h = d.h - (left + (right-left)/2)`.
The division is C truncation.

### 1.11 Resource ids

For every part, gadget info (WWGADGET type 1021, id = `+0x28`) =
`(id, kind, dialog = id, doc = id + 2000)`. The dialog is WIDGET
RT_DIALOG `id`, and Doc's text is WWGADGET type 1020 `id + 2000`
(1007 → 3007).

---

## 2. Gates

### 2.1 cGate (base of AndG, OrGa, XorG)

Vtable `0x499a88`. Fresh constructor `0x447d38` (= cGadget's `0x41d076`
plus the vtable). Load constructor `0x447d67`. Destructor `0x447d94`.
Overrides: slot 0x24 has_bool = `0x447d8d` (true); slot 0x28 bool =
`0x447d59` (`+0x13e`). Draw is pure (`0x482cb8`). No other state.
**NotG does not derive from it** (§2.2).

The three two-input gates are identical except for the operator:

| tag | class | vtable | init | fresh / load ctor | tick | draw | dtor | id (+0x28) | sheet = knots | doc | dialog title |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `AndG` | cAndGate | `0x499080` | `0x440f2c` | `0x440fd7` / `0x44115f` | `0x4411aa` | `0x44124a` | `0x441355` | 2008 | 1029 | 4008 | "AND Logic Gate" |
| `OrGa` | cOrGate | `0x49aa4c` | `0x44e464` | `0x44e50f` / `0x44e697` | `0x44e6e2` | `0x44e782` | `0x44e88d` | 2009 | 1030 | 4009 | "OR Logic Gate" |
| `XorG` | cXorGate | `0x49c524` | `0x45cd70` | `0x45ce1b` / `0x45cfa3` | `0x45cfee` | `0x45d09a` | `0x45d1a5` | 2011 | 1031 | 4011 | "XOR Logic Gate" |

Knots (all three): `0` in at (v15,h4); `1` in at (v31,h4); `2` out at
(v23,h75), the bool output.

Picture: frame 1 of the sheet (82×47) at `pos`, always. Frame 2 is a 1×1
dummy. `bounds` = frame 1.

State: `+0x13e` = the output bool. `+0x13d` = 0.

Tick:

```
a = input(0); b = input(1)                  // unwired = 0.0; NaN = 0
AndG: +0x13e = (a != 0 && b != 0)
OrGa: +0x13e = (a != 0 || b != 0)
XorG: +0x13e = ((a != 0) != (b != 0))
c0 = changed(0); c1 = changed(1)            // both always evaluated (both flags cleared)
if c0 || c1: fire(2)
```

The output is recomputed every tick, whether or not anything changed.
Only a changed flag on an input is passed on. A source with no typed
output reads as -1.0, which counts as true.

Settings dialog (2008/2009/2011): no settings. It has only the Pic/Doc
buttons and the static "True or False".

### 2.2 cNotGate `NotG`

Vtable `0x49a224`. Base is **cGadget** (fresh ctor `0x44c37f` calls
`0x41d076`). Init `0x44c2d4`, load ctor `0x44c4eb`, draw `0x44c5b0`, dtor
`0x44c6bb`. Id 2010, sheet = knots 1032, doc 4010, dialog 2010 ("NOT
Logic Gate", no settings).

Knots: `0` in at (v23,h4); `1` out at (v23,h75).

Picture: frame 1 of sheet 1032 (81×47) at `pos`.

Slots: 0x24 `0x44c544` → true; 0x28 `0x44c536` → `+0x13e`. Tick
`0x44c54b`:

```
w = wire(0)
if w == null: +0x13e = 0; return          // an unwired NOT outputs FALSE
+0x13e = !src_bool(w)                     // bool > number(!=0) > object(!=0) > false
if changed(0): fire(1)
```

Unlike the other gates, NotG uses `src_bool` (§1.2). A source with no
typed output gives false, so the NOT outputs true. A number source counts
as true when it is ≠ 0.

---

## 3. Number crunchers: cNumberCruncher + Addr/Subr/Mulr/Divr

Base vtable `0x49a2e0`. Init `0x44c778`, fresh ctor `0x44c85c`, load ctor
`0x44c9c8`, reset `0x44ca1a`, has_number `0x44ca13` (true), draw
`0x44ca43`, tick `0x44cc85`, dtor `0x44cc09`. The subclasses only set the
vtable, tag, operator and id:

| tag | class | vtable | fresh / load ctor | dtor | op `+0x198` | id | doc | dialog title |
|---|---|---|---|---|---|---|---|---|
| `Addr` | cAdder | `0x498ff0` | `0x440e20` / `0x440e69` | `0x440efc` | 0 | 2016 | 4016 | "Adder" |
| `Subr` | cSubtracter | `0x49be64` | `0x459718` / `0x459761` | `0x459850` | 1 | 2017 | 4017 | "Subtractor" |
| `Mulr` | cMultiplier | `0x49a0e0` | `0x44bc00` / `0x44bc49` | `0x44bce0` | 2 | 2018 | 4018 | "Multiplier" |
| `Divr` | cDivider | `0x499814` | `0x446504` / `0x44654d` | `0x4465e4` | 3 | 2019 | 4019 | "Divider" |

Knots (resource 1020, shared): `0` in at (v36,h3), the first operand
`a`; `1` in at (v3,h49), the second operand `b` (on top); `2` out at
(v36,h93), the number result.

Sheet 1020 (shared). Frames 1–4 are the op symbols + − × ÷ (26×24).
Frames 5–7 and 10–14 are the body animation (100×64). Frames 8–9 are 1×1
dummies. Frames 15–18 are small op symbols (18×16), unused by this class.
Points 1020: `P[0]` = (v24,h53), the op-symbol spot. `P[1]` = (v9,h28)
is unused. `bounds` = frame 5.

State:

| offset | meaning |
|---|---|
| `+0x13d` | 1 (start sets caches to NONE and clears flags) |
| `+0x140` | error flag: draws the broken overlay and forces the output to 0 |
| `+0x141` | animation step 0..7 (0 = idle) |
| `+0x146` | f64 result (the number output, base slot 0x30) |
| `+0x197` | u8 "an input's wire flag was set": decides whether to `fire(2)` at the end. Init sets it to 1. |
| `+0x198` | u8 operator 0..3 |
| `+0x2b` | set to 0 by init (no live-update dialog callback) |

Reset `0x44ca1a`: base reset, then `+0x146 = 0.0`, `+0x141 = 0` (the
base already does both).

Tick `0x44cc85`:

```
if +0x141 == 0:
    v0 = value_changed(0); v1 = value_changed(1)
    c0 = changed(0);       c1 = changed(1)
    if !(v0 || v1 || c0 || c1): return
    +0x197 = (c0 || c1)
    +0x141 += 1                          // = 1
    invalidate(); play(1020)
    return
if !throttle(9): return
+0x141 += 1
if +0x141 == 8:
    a = input(0); b = input(1)           // fresh reads, not the cached values
    +0x141 = 0
    match +0x198:
      0: +0x146 = a + b
      1: +0x146 = a - b
      2: if |b| > 1.0 and |a| > DBLMAX / |b|:  +0x140 = 1      // result not stored
         else: +0x146 = a * b
      3: if b == 0.0: +0x146 = 0.0; +0x140 = 1
         elif |b| < 1.0 and |a| > |b| * DBLMAX: +0x140 = 1      // result not stored
         else: +0x146 = a / b; +0x140 = 0
    if +0x197: fire(2)
// reached for steps 2..7 too:
if +0x140: +0x146 = 0.0
invalidate()
```

`DBLMAX` is the double at `0x44cf71` = `0x7FEFFFFFD7B9609A` (≈
1.797693e+308, a little less than f64::MAX). `|x|` is
`x > 0 ? x : -x`. The test `|b| > DBLMAX/|b|...` is evaluated in
extended precision. Multiplication by `|b| ≤ 1` is never checked.

Consequences that matter for faithfulness:

* A computation takes 1 tick to notice plus 7 throttled steps of ≥10
  ticks each, so the result appears about 70 ticks (≈1.2 s) after the
  input changes. Input changes during the animation are not seen until
  the next idle tick. The flags and caches are only consumed in the idle
  branch.
* If only the value changed (no wire flag), the result is computed but
  output 2 is **not** fired.
* The error flag `+0x140` is cleared only by a successful **divide** or by
  reset. Add, Sub and Mul never clear it. Once a Multiplier overflows, it
  outputs 0 and shows the broken overlay until the run stops.
* With both inputs unwired, nothing ever triggers (`value_changed` is
  false for unwired knots), and the output stays 0.

Draw `0x44ca43`:

```
draw_frame(1020, BODY[+0x141], pos)            // BODY = [10,11,12,13,14,5,6,7] at 0x49a278
draw_frame(1020, +0x198 + 1, pos + P[0])       // + − × ÷
if +0x140: draw_broken()
```

Dialogs 2016–2019: no settings (static "Number").

---

## 4. cOneNumberCruncher `1ncr` ("Math Functions")

Vtable `0x49a9bb`, base cGadget. Init `0x44dc58`, fresh ctor `0x44dd1f`
(also calls reset), load ctor `0x44de97`, has_number `0x44deee` (true),
tick `0x44def5`, draw `0x44e195`, dtor `0x44e357`. Number output = base
slot 0x30 (`+0x146`). Id 2005, sheet = knots = points 1024, doc 4005.
`+0x13d` = 1.

Knots: `0` in at (v27,h4); `1` out at (v27,h100).

Sheet 1024: frames 1–8 are the body animation (103×55). Frames 9–15 are
the function symbols (26×25): 9 `!`, 10 `√`, 11 `x²`, 12 `Cos`, 13 `Sin`,
14 `Tan`, 15 `Int`. Frame 16 is a 1×1 dummy. `P[0]` = (v15,h55).
`bounds` = frame 1.

Settings: number field 0 (block `+0x08`, gadget `+0x3d`) = the function
(combo box 107, strings 2005):

| value | combo text | operation |
|---|---|---|
| 0 | Square Root | `x >= 0 ? sqrt(x) : error` |
| 1 | Square | `|x| < sqrt(DBLMAX) ? x*x : error` |
| 2 | Factorial | `n = ftol_clamped(x)` (truncates toward 0); `(u16)n < 171 ? FACT[(u16)n] : error` |
| 3 | Sin | `sin(x * 0.01745329251993889)` (degrees) |
| 4 | Cos | `cos(x * 0.01745329251993889)` |
| 5 | Tan | `tan(x * 0.01745329251993889)` |
| 6 | Integer | `floor(x)` (rounds toward −∞: Int(−1.5) = −2) |
| other | – | `+0x146` unchanged |

The default after a fresh drop is 0 (the block is zero). Notes:

* Factorial uses `(u16)n`. A negative `n` becomes ≥ 0x8000, which is an
  error, but `n = 65536 + k` with `k < 171` gives `FACT[k]`. In the
  original `FACT` is not computed: it is the table of 171 doubles at
  `0x49a3fc` (Appendix A). Entry 0 = 1, 1 = 1, and it is exact up to 12!.
  From 13! on, the entries are the factorial rounded to about 7
  significant digits (13! = 6227021000, not 6227020800), and a few
  entries differ from `%.7g` too. Copy the table bit-exactly.
* Square: `DBLMAX` is as in §3; `sqrt` is evaluated in extended
  precision. The test is `|x| < sqrt(DBLMAX)` (`jb`), so an equal value is
  an error.
* Sin/Cos/Tan: the degree factor is an f64 constant (`0x44e18d`). The
  product is rounded to f64 before the call. With `tan(90°)` the radian
  value is not exactly π/2, so the result is large but finite (no error).

Tick `0x44def5`:

```
if +0x141 == 0:
    v = value_changed(0)
    +0x197 = changed(0)
    if v || +0x197:
        +0x141 += 1; invalidate(); play(1020)
    return
if !throttle(9): return
+0x141 += 1
if +0x141 == 8:
    x = input(0)
    +0x141 = 0; +0x140 = 0
    match +0x3d { ... table above; "error" means +0x140 = 1 }
    if +0x140: +0x146 = 0.0
    if +0x197: fire(1)
invalidate()
```

Here the error flag is cleared on every computation. Unlike the number
crunchers, the overlay is not forced on steps 2..7.

Draw `0x44e195`:

```
draw_frame(1024, +0x141 + 1, pos)
draw_frame(1024, SYM[+0x3d], pos + P[0])    // SYM = [10,11,9,13,12,14,15] at 0x49a954
                                            // (+0x3d is not range-checked: values >= 7 read the next table bytes)
if +0x140: draw_broken()
```

Dialog 2005 "Math Functions": combo 107 ("Output:" label) → number field
0, as in the table.

---

## 5. cComparison `Comp` ("Comparer")

Vtable `0x49931a`, base cGadget. Init `0x44281c`, fresh ctor `0x4428c0`,
load ctor `0x442a33`, has_bool `0x442abc` (true), bool `0x442aae`
(`+0x13e`), tick `0x442ac3`, draw `0x442bf2`, dtor `0x442d0a`. Id 2021,
sheet = knots 1028, doc 4021. `+0x13d` = 0.

Knots: `0` in at (v36,h4), the first number `a`; `1` in at (v4,h39), the
second number `b` (top); `2` out at (v36,h72), the bool.

Settings: radio group 0 (block `+0x04`, gadget `+0x39`) = mode. The
fresh ctor sets it to **2** (=). Dialog 2021: "Output TRUE if first number
is: … the second number." The radio ids 106–111 all carry tag `1`:

| value | id | caption | test | frame |
|---|---|---|---|---|
| 0 | 106 | Less Than | `a < b` | 2 (`<`) |
| 1 | 107 | Greater Than | `a > b` | 1 (`>`) |
| 2 | 108 | Equal to | `a == b` | 4 (`=`) |
| 3 | 109 | Not Equal to | `a != b` | 3 (`≠`) |
| 4 | 110 | Less than or Equal to | `a <= b` | 6 (`≤`) |
| 5 | 111 | Greater than or Equal to | `a >= b` | 5 (`≥`) |

The frame column is the table at `0x4992d8` = `[2,1,4,3,6,5]`. Sheet 1028:
frames 1–6 (79×63) are the complete pictures, one per symbol. Frame 7 is a
1×1 dummy. Draw: `draw_frame(1028, FRAME[+0x39], pos)`. Mode ≥ 6 is not
range-checked.

Tick:

```
a = input(0); b = input(1)        // f64, unwired = 0.0
match +0x39 { 0: +0x13e = a<b; 1: a>b; 2: a==b; 3: a!=b; 4: a<=b; 5: a>=b; _: unchanged }
c0 = changed(0); c1 = changed(1)
if c0 || c1: fire(2)
```

The comparison is an exact f64 compare. On x87 an unordered compare (NaN)
makes `<`, `<=` and `==` true and the others false (`setb`/`setbe`/`sete`
on CF/ZF = 1). This edge case does not normally occur.

---

## 6. cConversion `Conv` ("Converter")

Vtable `0x499488`, base cGadget. Init `0x443994`, fresh ctor `0x443a5b`,
load ctor `0x443bdd`, has_number `0x443c34`, tick `0x443c3b`, draw
`0x443e93`, dtor `0x444112`. Id 2020, sheet = knots = points 1026, doc
4020. `+0x13d` = 1. Number output = `+0x146`.

Knots: `0` in at (v27,h4); `1` out at (v27,h100).

Settings: number field 0 (gadget `+0x3d`) = combo 107 (strings 2020).
The **fresh ctor sets it to 1** (after its reset).

| value | text | result (`x` = input) | error if |
|---|---|---|---|
| 0 | °Celsius to °Fahrenheit | `x * 1.8 + 32.0` | `x < -273.0` |
| 1 | °Fahrenheit to °Celsius | `(x - 32.0) * ext(0.5555555555555556)` | `x < -459.40000000000003` |
| 2 | Meters to Feet | `x * ext(3.280839895013123)` | `x < 0.0` |
| 3 | Feet to Meters | `x * 0.3048` | `x < 0.0` |
| 4 | Kilograms to Pounds | `x * ext(2.2046244201837775)` | `x < 0.0` |
| 5 | Pounds to Kilograms | `x * 0.453592` | `x < 0.0` |
| other | – | `+0x146` unchanged | `x < 0.0` |

Constants: 1.8 = f64 `0x443e47`; 32.0 = f32 `0x443e4f`; −273.0 = f32
`0x443e53`; −459.4 = f64 `0x443e63`; 0.3048 = f64 `0x443e77`; 0.453592 =
f64 `0x443e8b`. The "ext" constants are 80-bit at `0x443e57` (5/9),
`0x443e6b` and `0x443e7f`. In cases 0 and 1 the whole expression is
computed in extended precision and rounded once.

Tick `0x443c3b` follows the same pattern as 1ncr:

```
if +0x141 == 0:
    v = value_changed(0); +0x197 = changed(0)
    if v || +0x197: +0x141 += 1; invalidate(); play(1020)
    return
if !throttle(9): return
+0x141 += 1
if +0x141 == 8:
    x = input(0); +0x141 = 0
    +0x140 = (x < 0.0)
    match +0x3d { 0: +0x146 = …; +0x140 = (x < -273.0)
                  1: +0x146 = …; +0x140 = (x < -459.4…)
                  2..5: +0x146 = … }          // +0x140 stays (x < 0)
    if +0x140: +0x146 = 0.0
    if +0x197: fire(1)
invalidate()
```

Draw `0x443e93`. Sheet 1026: frames 1–8 are the body animation (103×55),
frame 9 is a 1×1 dummy, and frames 10–15 are unit labels (20×11): 10
`C°`, 11 `M`, 12 `Kg`, 13 `F°`, 14 `Ft`, 15 `Lb`. Points: `P[0]` =
(v22,h27) for the "from" unit, `P[1]` = (v23,h58) for the "to" unit.

```
draw_frame(1026, +0x141 + 1, pos)
half = (i32)+0x3d / 2                       // C division
from = (+0x3d odd) ? half + 3 : half
to   = (+0x3d odd) ? half     : half + 3
draw_frame(1026, from + 10, pos + P[0])
draw_frame(1026, to + 10,   pos + P[1])
if +0x140: draw_broken()
```

---

## 7. Splitters: cSplitter + Spl2/Spl3/Spl4

Base vtable `0x49b424`. Init `0x453f1c`, base ctor `0x453fa7`, base load
ctor `0x453fd1`, has_bool `0x454000` (→ `+0x197`), has_number `0x45400e`
(→ `+0x198`), has_object `0x45401c` (→ `+0x199`), tick `0x45402a`, draw
`0x4540f0`, base dtor `0x454234`. The value getters are the base ones:
bool `+0x13e`, number `+0x146`, object `+0x14e`. Sheet 1019 (shared).
`+0x13d` = 0.

| tag | class | vtable | fresh / load ctor | dtor | id | knots | frame | doc | dialog title |
|---|---|---|---|---|---|---|---|---|---|
| `Spl2` | cSplitter2 | `0x49b484` | `0x45429c` / `0x454441` | `0x454510` | 2022 | 1033 | 1 (45×37) | 4022 | "Two-way Splitter" |
| `Spl3` | cSplitter3 | `0x49b4e4` | `0x454540` / `0x4546e5` | `0x4547b4` | 2023 | 1034 | 2 (45×57) | 4023 | "Three-way Splitter" |
| `Spl4` | cSplitter4 | `0x49b544` | `0x4547e4` / `0x454989` | `0x454aac` | 2024 | 1035 | 3 (45×69) | 4024 | "Four-way Splitter" |

Knots: knot 0 is the input, and the rest are outputs carrying the same
value.

* Spl2: `0` in (v18,h4); `1` out (v8,h39); `2` out (v28,h39).
* Spl3: `0` in (v28,h4); outs `1`–`3` at (v8,h39), (v28,h39), (v48,h39).
* Spl4: `0` in (v34,h4); outs `1`–`4` at (v8,h39), (v25,h39), (v43,h39),
  (v60,h39).

The draw picks the frame from the tag (`Spl2`→1, `Spl3`→2, `Spl4`→3) and
draws it at `pos`. `bounds` = that frame.

State: `+0x197` / `+0x198` / `+0x199` = the copied "has bool / has
number / has object" of the source. Init sets them to `1, 0, 0`, and
**reset does not touch them**: they keep the last run's values.
`+0x13e` / `+0x146` / `+0x14e` hold the copied values (base reset
zeroes them).

Tick `0x45402a`:

```
c = changed(0)
v = value_changed(0)
if !(c || v): return
if c: fire(1); fire(2); fire(3); fire(4)        // fire() ignores knots beyond the count
copy_input(this, 0)                             // §1.2
w = wire(0)                                     // non-null here: either test can only be true when wired
+0x197 = w.src.has_bool(k); +0x198 = w.src.has_number(k); +0x199 = w.src.has_object(k)
```

The splitter only updates its copy when the input changes (flag or
value), so it lags the source by up to one tick. Consumers that are
earlier in the gadget list see the old copy on the same tick.

Dialogs 2022–2024: no settings ("Whatever came in").

---

## 8. cMultiplexer `Mplx` ("Switcher")

Vtable `0x49a07e`, base cGadget. Init `0x44b534` (also loads the knots and
calls reset with this vtable), fresh ctor `0x44b610`, load ctor
`0x44b77d`, draw `0x44b7b8`, reset `0x44b96f`, has_bool `0x44b9a5`
(→ `+0x198`), has_number `0x44b9b3` (→ `+0x199`), has_object `0x44b9c1`
(→ `+0x19a`), tick `0x44b9cf`, dtor `0x44b8fe`. Values use the base
getters (`+0x13e` / `+0x146` / `+0x14e`). Id 2025, sheet = knots 1036,
doc 4025. `+0x13d` = 1.

Knots: `0` in at (v4,h52), the selector (top); `1` in at (v29,h4), data
input 1; `2` in at (v59,h4), data input 2; `3` out at (v45,h82).

State: `+0x197` = the selected input knot (1 or 2). `+0x198..0x19a` =
the type flags of the copied value. `+0x141` = the train animation step
(0 = idle, 1..4).

Reset `0x44b96f`: base reset; `+0x19a = 0`, `+0x199 = 0`, `+0x198 = 1`,
`+0x197 = 1`.

Tick `0x44b9cf`:

```
dirty = false
if changed(0) | value_changed(0):              // both evaluated; changed() first
    sel = (input(0) != 0.0) ? 2 : 1
    if +0x197 != sel:
        play(1036); +0x197 = sel; dirty = true
c = changed(+0x197)                            // both evaluated
v = value_changed(+0x197)
if c || v: dirty = true
if dirty:
    if c: fire(3)                              // only the selected data wire's flag is passed on
    +0x141 = 1; invalidate()
    if copy_input(this, +0x197):
        w = wire(+0x197)
        +0x198 = w.src.has_bool(k); +0x199 = w.src.has_number(k); +0x19a = w.src.has_object(k)
    else:
        +0x198 = 1; +0x19a = 0; +0x199 = 0    // unwired selected input: a false bool
if +0x141 != 0 and throttle(15):
    +0x141 += 1
    if +0x141 == 5: +0x141 = 0
    invalidate()
```

Switching to the other input copies its value, but fires output 3 only if
that input's wire flag happens to be set at that moment. The unselected
input's flags and cache are not consumed.

Draw `0x44b7b8`:

```
if editing (DAT_00493dde != 0): frame = 1
else: frame = (+0x197 == 2 ? 8 : 3) + +0x141
draw_frame(1036, frame, pos)
```

Sheet 1036 (88×72): frame 1 is the edit-mode picture, frame 2 is a 1×1
dummy, frames 3–7 are track set to input 1 with the train animating, and
frames 8–12 are the same for input 2. `bounds` = frame 1.

Dialog 2025: no settings ("One of two inputs").

---

## 9. cCounter `Cntr`

Vtable `0x49953e`, base cGadget. Init `0x444218`, fresh ctor `0x4442e3`,
load ctor `0x444460`, has_number `0x44451c` (true), number `0x444523`
(`fild +0x142`, the integer count), reset `0x444531`, draw `0x44455e`,
count `0x444869`, tick `0x44489f`, dtor `0x4444ab`. Id 2033, sheet =
knots = points 1041, doc 4033. `+0x13d` = 0.

Knots: `0` in at (v33,h4); `1` out at (v33,h71).

State: `+0x142` i32 count. `+0x146` f64 copy of the count (written, but
not used by any getter). `+0x141` = frame: 1 = "just counted" (lit), 2 =
idle. `+0x13f` = show digits (always 1: set by reset, never cleared).

Reset `0x444531`: base reset; `+0x13f = 1`; `+0x142 = 0`; `+0x141 = 2`.

Tick `0x44489f`:

```
w = wire(0); if w == null: return
c = changed(0)
v = value_changed(0)                 // always evaluated (keeps the cache current)
if w.src.has_bool(k):    if c: count()
elif w.src.has_object(k): if c: count()
else:                     if v || c: count()
if +0x141 == 1 and throttle(6):
    +0x141 = 2; fire(1); invalidate()

fn count():                         // 0x444869
    +0x141 = 1; +0x142 += 1; +0x146 = (f64)+0x142; invalidate()
```

* With a bool source the counter counts **every changed event**, rising
  or falling, not just rising edges. It relies entirely on the source
  setting the wire flag.
* The output fires about 7 ticks after the last count. The timer is not
  restarted by later counts: `+0x152` is only updated when `throttle`
  passes.
* The count is not capped.

Draw `0x44455e`. Sheet 1041: frames 1–2 are the body (89×67; 1 = lit, 2 =
idle), frame 3 is a 1×1 dummy, and frames 4–13 are the digits 0–9 (7×13).
Points `P[0..2]` = (v27,h21), (v27,h35), (v27,h49). `bounds` = frame 2.

```
draw_frame(1041, +0x141, pos)
if +0x142 < 1000:
    if +0x13f:
        draw_frame(1041, 4 + +0x142/100,      pos + P[0])
        draw_frame(1041, 4 + (+0x142/10)%10,  pos + P[1])
        draw_frame(1041, 4 + +0x142%10,       pos + P[2])
else: draw_broken()                 // 1000 and above: no digits, broken overlay
```

Dialog 2033: no settings.

---

## 10. cTotaler `Ttlr`

Vtable `0x49c398`, base cGadget. Init `0x45be1c`, fresh ctor `0x45bee5`,
load ctor `0x45c05d`, draw `0x45c0b4`, reset `0x45c3b4`, has_number
`0x45c48d` (true), start `0x45c494`, tick `0x45c4ac`, dtor `0x45c41c`.
Number output = base `+0x146`. Size 0x1bc. Id 2001, sheet = knots =
points 1005, doc 4001. `+0x13d` = 0.

Knots: `0` in at (v4,h89) (top); `1` out at (v62,h89) (bottom).

State:

| offset | meaning |
|---|---|
| `+0x142` | i32 count of samples |
| `+0x146` | f64 output |
| `+0x196` | u8 sticky decimals for `format_number` (base reset zeroes it) |
| `+0x197` | f64 average |
| `+0x19f` | f64 sum |
| `+0x1a7` | f64 minimum (reset to `+1e300`, bits `0x7E37E43C8800759C`) |
| `+0x1af` | f64 maximum (reset to `-1e300`, bits `0xFE37E43C8800759C`) |
| `+0x1b7` | u32, zeroed by reset, otherwise unused |
| `+0x1bb` | u8 "no sample yet": set by start, cleared by the first sample; hides the number |

Settings: radio group 0 (`+0x39`) = output mode (dialog 2001, ids
106–109, tag `1`): 0 Total, 1 Average, 2 Maximum, 3 Minimum. The default
is 0.

Reset `0x45c3b4`: base reset, then sum = avg = 0, max = −1e300,
min = +1e300, `+0x1b7 = 0`. Start `0x45c494`: base start (primes the
cache), then `+0x1bb = 1`.

Tick `0x45c4ac`:

```
c = changed(0)
v = value_changed(0)
if !(c || v): return
+0x1bb = 0
x = input(0)
+0x142 += 1
sum += x
avg = sum / (f64)+0x142
if max < x: max = x
if x < min: min = x
match +0x39 { 0: +0x146 = sum; 1: avg; 2: max; 3: min; _: unchanged }
invalidate()
if c: fire(1)
```

Because the cache is primed at start, the value present when the run
starts is **not** counted until it changes or the source fires.

Draw `0x45c0b4`. Sheet 1005: frame 1 is the body (179×68), frame 2 a 1×1
dummy, frames 3–6 the labels AVERAGE / TOTAL / MAXIMUM / MINIMUM
(96×19), and frames 7–20 characters (16×18): 7 `.`, 8 `-`, 9 `E`, 10
`+`, 11–20 `0`–`9`. Points: `P[0]` = (v14,h40) is the label; `P[1..10]`
= (v36, h5 + 17i) for i = 0..9 are the character cells (h 5, 22, 39, …,
158).

```
draw_frame(1005, 1, pos)
draw_frame(1005, LABEL[+0x39], pos + P[0])         // LABEL = [4,3,5,6] at 0x49c334
if running (DAT_00493dde == 0) and +0x1bb == 0:
    s = format_number(+0x146, &+0x196)             // §1.7
    for i in 0..10:                                // first 10 chars only
        ch = s[i]
        frame = match ch { '.': 7, ' ': skip, '+': 10, '-': 8, 'E'|'e': 9,
                           '0'..='9': ch - 0x25,          // = 11..20
                           _: { draw_broken(); return } }  // e.g. "INF"/"NAN"
        draw_frame(1005, frame, pos + P[i + 1])
```

---

## 11. cStopper `Stop` ("Win / Lose Checker")

Vtable `0x49bc00`, base cGadget. Init `0x457604` (also loads the knots),
fresh ctor `0x4576e0`, load ctor `0x45786b`, reset `0x4578b0`, draw
`0x4578c8`, start `0x457b77`, tick `0x457ba2`, fill dialog `0x457e95`,
dtor `0x457af0`. Size 0x19c. Id 2043, sheet = knots = points 1051, doc
4043. No outputs, and no getters overridden.

Knots: `0` in at (v22,h4).

Settings (dialog 2043). Radio groups by tag:

| group (block byte, gadget) | tag | buttons (index: id caption) |
|---|---|---|
| 0 (`+0x04`, `+0x39`) mode | `1` | 0: 105 "Win if input is:"; 1: 106 "Win / Lose if input is:" |
| 1 (`+0x05`, `+0x3a`) bool target | `2` | 0: 107 "True"; 1: 108 "False" |
| 2 (`+0x06`, `+0x3b`) compare | `3` | 0: 109 equal to; 1: 110 not equal to; 2: 111 greater than; 3: 112 less than; 4: 113 greater than or equal to; 5: 114 less than or equal to |
| 3 (`+0x07`, `+0x3c`) sound | `4` | 0: 117 "Win sound"; 1: 118 "Lose sound" |

Text field 0 (edit 115, filter `#.-6`: numeric, `.` and `-`, max 6
chars) = "this number". The fresh ctor sets it to `"1"`. The overridden
fill `0x457e95` only wires up which controls each radio enables
(mode 0 → group 1; mode 1 → group 2 and the text), then calls the base
fill.

State: `+0x146` f64 target number. `+0x197` u8 triggered. `+0x198` u32
trigger time (`clock()`). `+0x141` animation step.

Reset `0x4578b0`: base reset (`+0x146 = 0`, `+0x141 = 0`); `+0x197 = 0`.
Start `0x457b77`: base start, then `sscanf(text0, "%lf", &+0x146)`
(format at `0x49bbf0`). If the text doesn't parse, `+0x146` keeps 0.0
from reset.

Tick `0x457ba2`:

```
// cheat: mouse at the screen's top-left corner while W, I and N are all held
(y, x) = GetCursorPos()                   // 0x4153ac
if x == 0 and y == 0 and key_down('W') and key_down('I') and key_down('N'):   // 0x41507c, GetKeyState < 0
    +0x197 = 1
was = +0x197
w = wire(0)
if w == null: return                       // nothing else happens while unwired (even after the cheat)
if +0x197 == 0:
    if +0x39 == 0:                         // bool mode
        if src_bool(w) == (+0x3a == 0): +0x197 = 1     // "True" → fires on true; "False" → fires on false
    else:                                  // number mode
        x = input(0); n = +0x146
        match +0x3b { 0: x == n; 1: x != n; 2: x > n; 3: x < n; 4: x >= n; 5: x <= n }  → +0x197 = 1 if true
else:
    // BLOCKING end animation: no other gadget ticks until it ends
    loop:
        0x44081d()                         // pump/idle (sound service)
        if (u32)(clock() - +0x198) > +0x141 * 25:
            +0x141 += 1; invalidate()
            if +0x141 == 8:
                +0x141 = 0; +0x197 = 0
                if +0x3c == 0: 0x42d4b1(DAT_004a2858)   // win: mark the current puzzle solved
                0x430ee7(DAT_004a2868)                   // leave run mode (resets every gadget, §1.5)
                play(128)
        invalidate(); 0x4234b8(doc, 1)                  // repaint now
        while +0x197 != 0
if +0x197 != 0 and was == 0:               // newly triggered this tick
    +0x198 = clock()
    play(+0x3c == 0 ? 1051 : 1151)         // win / lose sound
    +0x141 = 1; invalidate()
```

* Detection, the trigger sound and frame 1 happen on one tick. The
  blocking end animation starts on this gadget's **next** tick. Because the tick loop stops
  when run mode ends, gadgets after it in the list are not ticked in that
  final pass.
* Frames advance when more than 25, 50, …, 175 ticks have passed since
  the trigger, so the sequence lasts about 2.9 s. All times use the
  absolute `clock()`, not `doc.+0x4b`.
* With the cheat, `was` is already 1, so there is no trigger sound, and
  `+0x198` is stale (0 or the last trigger). Every loop pass then
  advances a frame, the 8 frames finish almost at once, and the run ends
  (solving the puzzle if "Win sound" is selected).
* "False" in bool mode fires on the first tick if the input is false
  (unwired is excluded by the early return).

Draw `0x4578c8`. Sheet 1051: frame 1 is the body (112×113), frame 2 the
idle flag panel (59×93), frames 3–9 the animation. `P[0]` = (v8,h33).

```
draw_frame(1051, 1, pos)
draw_frame(1051, +0x141 == 0 ? 2 : +0x141 + 2, pos + P[0])
```

---

## 12. cRandomNumberGenerator `rand`

Vtable `0x49afb8`, base cGadget. Init `0x4506dc`, fresh ctor `0x450792`
(also resets), load ctor `0x45091b`, draw `0x450972`, has_number
`0x450af8` (true), tick `0x450aff`, fill dialog `0x450cdb`, dtor
`0x450a87`. Number output = base `+0x146`. Id **1007** (a 1xxx id),
sheet = knots 1007, doc 3007, dialog 1007 "Random Number Generator".
`+0x13d` = 0.

Knots: `0` in at (v26,h3), the trigger; `1` out at (v27,h84).

Settings:

* Radio group 0 (`+0x39`), tag `1`: 0 = id 107 "Preset:", 1 = id 109
  "Custom:". Default 0.
* Number field 0 (`+0x3d`) = combo 108 (strings 1007): 0 `1-10`, 1
  `1-100`. Default 0.
* Text 0 (`+0x4d`, edit 111 "from", filter `4#`) and text 1 (`+0x89`,
  edit 113 "to", filter `4#`). Both are parsed with `sscanf("%d")`
  (`0x49afa6`/`0x49afa9`) into 0-initialised ints.
* `0x450cdb` only wires up which controls each radio enables, then calls
  the base fill.

Tick `0x450aff`:

```
if +0x141 == 0:
    if input(0) != 0.0:                       // level-triggered: no change detection
        +0x141 += 1; invalidate(); play(1007)
    return
if !throttle(15): return
+0x141 += 1
if +0x141 == 4:
    +0x141 = 0
    fire(1)                                   // before the value is set, same tick: harmless
    n = random_int()                          // 0..0x7ffffffe (§1.9); the code's "if n<0: n=-n" never triggers
    if +0x39 == 0:
        if   +0x3d == 0: +0x146 = (f64)(n % 10 + 1)
        elif +0x3d == 1: +0x146 = (f64)(n % 100 + 1)
        // other values: unchanged
    else:
        lo = sscanf(text0, "%d") or 0
        hi = sscanf(text1, "%d") or 0
        A = max(lo, hi); B = min(lo, hi)      // the code swaps when hi > lo
        r = (f64)((n & 0x3fff) * ext(1/16383))     // 80-bit constant at 0x450cc7, stored to f64
        t = (f64)((f64)(i32)(B - A) * r + (f64)A)  // i32 subtraction, then extended mul/add, stored to f64
        +0x146 = floor((f64)(t + 0.5f))            // 0.5 is an f32 constant (0x450cd3); sum rounded to f64; floor = 0x480e90
invalidate()
```

* A new number is produced every 4 throttled steps (≥16 ticks each, so
  ≥64 ticks) for as long as the trigger input stays non-zero.
* Custom range: `t` runs from `A` (r = 0) down to `B` (r = 1).
  Rounding with `floor(t + 0.5)` gives the two end values about half the
  probability of the inner values. The range is inclusive.

Draw `0x450972`: `draw_frame(1007, +0x141 + 1, pos)`. Sheet 1007:
frames 1–4 (90×53) are the animation, frame 5 is a 1×1 dummy.

---

## 13. cCoinToss `Coin`

Vtable `0x499282`, base cGadget. Init `0x442190`, fresh ctor `0x442223`
(also resets), load ctor `0x44261a`, draw `0x4423da`, has_number
`0x4426a1` (true), tick `0x4426a8`, dtor `0x44259e`. Number output =
base `+0x146` (0.0 or 1.0). The tag is set in the ctors. Id 2006, sheet
= knots = points 1006, doc 4006. Dialog 2006: no settings ("1 or 0").
`+0x13d` = 0.

Knots: `0` in at (v23,h4), the trigger; `1` out at (v23,h63).

Tick `0x4426a8`:

```
if +0x141 == 0:
    if input(0) != 0.0: +0x141 += 1; invalidate()       // level-triggered
    return
if !throttle(12): return
if +0x141 == 1: play(1006)
+0x141 += 1
if +0x141 == 5:
    +0x141 = 0
    +0x146 = (random_int() & 1) ? 1.0 : 0.0
    fire(1)
invalidate()
```

Draw `0x4423da`. Sheet 1006: frames 1–5 are the hand animation (69×47),
frame 6 a 1×1 dummy, frame 7 `HEADS` (28×7), frame 8 `TAILS` (26×7).
`P[0]` = (v5,h21).

```
draw_frame(1006, +0x141 + 1, pos)
if +0x141 == 0: draw_frame(1006, +0x146 == 0.0 ? 8 : 7, pos + P[0])  // 1 = HEADS, 0 = TAILS
```

The value is 0.0 after reset, so the coin shows TAILS before the first
toss.

---

## 14. cConstant `cnst` ("Three-Digit Number")

Vtable `0x4993d4`, base cGadget. Settings-changed `0x442dc4`, init
`0x442e22`, fresh ctor `0x443098`, load ctor `0x443214`, draw `0x44326f`,
reset `0x4435ed`, number `0x443686` (`fild +0x142`), has_number
`0x443694` (true), click `0x44369b`, start `0x44385e`, tick `0x443885`,
dtor `0x443615`. Id 2004, sheet = knots = points 1021, doc 4004.
`+0x13d` = 0.

Knots: `0` out at (v33,h91), the number.

State: `+0x142` i32 value 0..999. `+0x197` = an arrow is shown pressed.
`+0x198` = frame of the pressed arrow. `+0x199` = point index of the
pressed arrow.

Settings (dialog 2004):

* Text 0 (edit 107 "Enter Beginning Number:", filter `#3`) = the
  starting number.
* Check boxes by id: bit 0 = 108 "Lock Number" (clicks ignored), bit 1 =
  109 "Hide Number" (no digits drawn), bit 2 = 110 "Auto Reset" (reset
  restores the starting number).

Settings-changed (slot 0x50) `0x442dc4`, also called by both ctors:

```
if text0[0] == 0: +0x142 = 0
else:
    sscanf(text0, "%u", &+0x142)                // on parse failure +0x142 is unchanged
    if +0x142 < 0 or +0x142 > 999: +0x142 = 0
```

Because slot 0x50 also runs from the live-update callback (§1.6), editing
the dialog sets the value immediately.

Reset `0x4435ed` (**does not call the base reset**, so the value
survives Stop):

```
+0x197 = 0
if bit 2 (Auto Reset): settings_changed()
```

Start `0x44385e`: base start (primes the cache from its own output),
then `if +0x142 != 0: fire(0)`.

Click (slot 0x14, only while running) `0x44369b(this, pt)`, with
`p = pt - pos` as (v,h):

```
if bit 0 (Lock): return
old = +0x142
for i in 0..3:                                  // i = 0 ones, 1 tens, 2 hundreds; weight = 10^i
    if p in UP[i]:   +0x142 += 10^i; +0x197 = 1; +0x199 = 2 - i; +0x198 = 3 - i; fire(0); invalidate()
    elif p in DOWN[i]: +0x142 -= 10^i; +0x197 = 1; +0x199 = 8 - i; +0x198 = 7 - i; fire(0); invalidate()
if +0x142 > 999 or +0x142 < 0:
    +0x142 = old; play(1027)                    // output 0 was still fired
```

`UP[i]` = the rect with top-left `P[2-i]` and size of frame 1
(height 14, width 25). `DOWN[i]` = the same at `P[8-i]`. Rects are
half-open (`0x411e03`: `top <= v < bottom`, `left <= h < right`). The
rects are built once at class init (`0x442e22`) into `DAT_004a312a` (up)
and `DAT_004a3142` (down), indexed by `i`. Points 1021: `P[0..2]` =
(v0,h6), (v0,h31), (v0,h55) are the up arrows for hundreds/tens/ones.
`P[3..5]` = (v24,h13), (v24,h35), (v24,h57) are the digits. `P[6..8]` =
(v55,h6), (v55,h31), (v55,h55) are the down arrows. Digits do not
wrap on their own: the value is a plain integer, so ones-up on 19 gives
20. Only the 0..999 bounds are enforced.

Tick `0x443885`:

```
if +0x197 and !mouse_button_down():             // 0x415208: neither left nor right button held
    +0x197 = 0; invalidate()
```

Draw `0x44326f`. Sheet 1021: frames 1–3 are the pressed up arrows (25×14;
3 = ones, 2 = tens, 1 = hundreds), frame 4 the body (97×71), frames 5–7
the pressed down arrows (7 = ones, 6 = tens, 5 = hundreds), frames 8–17
the digits 0–9 (18×20), and frame 18 is 5×1. `bounds` = frame 4.

```
draw_frame(1021, 4, pos)
if !(bit 1 Hide):
    draw_frame(1021, 8 + v%10,       pos + P[5])
    draw_frame(1021, 8 + (v/10)%10,  pos + P[4])
    draw_frame(1021, 8 + (v/100)%10, pos + P[3])
if +0x197: draw_frame(1021, +0x198, pos + P[+0x199])
```

---

## 15. cMiniConstant `MiCo` ("Mini Three-Digit Number")

Vtable `0x499e28`, base cGadget. Settings-changed `0x449b24`, init
`0x449b82`, fresh ctor `0x449c4c`, load ctor `0x449db8` (calls slot
0x50), draw `0x449e13`, reset `0x44a0e9`, number `0x44a178`
(`fild +0x142`), has_number `0x44a186` (true), start `0x44a18d`, dtor
`0x44a107`. No tick, no click. Id 2046, sheet = knots = points 1063, doc
4046. `+0x13d` = 0.

Knots: `0` out at (v13,h59).

Settings (dialog 2046): text 0 only (edit 107, filter `3#`). Slot 0x50
`0x449b24` is the same as cConstant's: empty → 0, else `sscanf "%u"`
(format at `0x499e18`), out of 0..999 → 0. The fresh ctor does **not**
call it (the text is empty, so the value is 0 anyway).

Reset `0x44a0e9`: no base reset. `if bit 2: settings_changed()`. The
dialog has no check boxes, so the bit can only come from a file and is
always 0 in practice. The same applies to the bit 1 (Hide) test in draw.
Start `0x44a18d`: base start; `if +0x142 != 0: fire(0)`.

Draw `0x449e13`. Sheet 1063: frames 1–10 are the digits 0–9 (7×13),
frame 11 the body (65×27), frame 12 a 1×1 dummy. Points 1063: `P[0..2]` =
(v7,h10), (v7,h24), (v7,h38) for hundreds/tens/ones. `bounds` = frame 11.

```
draw_frame(1063, 11, pos)
if !(bit 1):
    draw_frame(1063, 1 + v%10,       pos + P[2])
    draw_frame(1063, 1 + (v/10)%10,  pos + P[1])
    draw_frame(1063, 1 + (v/100)%10, pos + P[0])
```

---

## 16. cFirstThru `1thr` (dead code)

The RTTI string `"cFirstThru"` is at `0x447d12`. This is a concrete
class, not a base. Vtable `0x499a28`, init `0x447624` (sheet/points
0x3ff = 1023), fresh ctor `0x4476a3`, load ctor `0x44786e`, has_bool
`0x44791d`, bool `0x447924`, tick `0x447986`, click `0x447a0d`, reset
`0x447a2a`, draw `0x447a6e`, dtor `0x447c3e`. **Nothing calls its
constructors**: the factory has no `1thr` case, and the ctor addresses
appear nowhere in the binary. WWGADGET has no sheet 1023, though it has
knots 1023 (4 inputs at (v14/30/46/62, h4) and 4 outputs at (v14/30/46/62,
h52)) and the info/doc for 2000. This is for completeness only:

```
reset:  base reset; out[0..4] = 0.0 (f64 at +0x197 + 8i); +0x13f = 1 (armed)
click:  reset(); invalidate()
bool(k): k in 4..7 → trunc(out[k-4]) != 0 (ftol of the f64)
tick:   if +0x13f: for i in 0..4: if value_changed(i): +0x13f = 0; out[i] = 1.0; invalidate()
```

The tick's inner clearing loop is buggy: it clears `out[i]` four times
instead of `out[0..4]`. All inputs that change on the same tick light up.

---

## 17. Function names (proposed)

| address | proposed name |
|---|---|
| `0x41cef4` | `cGadget::Reset` |
| `0x41cf8a` | `cGadget::InitFields` |
| `0x41e4c8` | `cGadget::PrimeKnotCache` |
| `0x41e50a` | `cGadget::Start` |
| `0x41e56e` | `cGadget::ReadInput` |
| `0x41e62a` | `cGadget::InputValueChanged` |
| `0x41e697` | `cGadget::ClearKnotFlag` |
| `0x41e6cf` | `cGadget::FireOutput` |
| `0x41e713` | `cGadget::TakeInputChanged` |
| `0x41e762` | `cGadget::Throttle` |
| `0x41e79c` | `cGadget::CopyInput` |
| `0x41ea4d` | `cGadget::FillDialog` |
| `0x41ea2e` | `cGadget::DialogOK` |
| `0x41e851` | `cGadget::ReadDialog` |
| `0x41eb78` | `cGadget::DialogLiveUpdate` |
| `0x41db1d` / `0x41db24` / `0x41db32` / `0x41db39` / `0x41db47` / `0x41db4e` | `cGadget::HasBool` / `GetBool` / `HasNumber` / `GetNumber` (`fld +0x146`) / `HasObject` / `GetObject` |
| `0x43d0a9` | `Knot_GetWire` |
| `0x43ae7c` / `0x43aef3` | `Wire_SrcHasObject` / `Wire_SrcObject` |
| `0x43c1e8` / `0x43c13c` | `Wire_SrcHasNumber` / `Wire_SrcNumber` |
| `0x43b7ab` / `0x43adec` | `Wire_SrcHasBool` / `Wire_SrcBool` |
| `0x43ca73` | `Wire_ClearChanged` |
| `0x424a12` | `CWidgetDoc::StartRun` |
| `0x424a94` | `CWidgetDoc::StopRun` |
| `0x42498f` | `CWidgetDoc::Tick` |
| `0x410f93` | `FormatNumber` |
| `0x415008` | `RandomInt` |
| `0x415018` | `SeedRandomFromTime` |
| `0x481500` | `rtl_srand64` |
| `0x481530` | `rtl_lrand` |
| `0x481514` | `rtl_rand` |
| `0x480ec4` | `rtl_ftol` |
| `0x439e3d` | `FtolClamped` |
| `0x480e90` | `rtl_floor` |
| `0x481228` / `0x4811e4` / `0x480c00` / `0x481264` | `rtl_sqrt` / `rtl_sin` / `rtl_cos` / `rtl_tan` |
| `0x4402cc` | `PlaySoundId` |
| `0x415208` | `MouseButtonDown` |
| `0x41507c` | `KeyDown` |
| `0x4153ac` | `GetCursorPosVH` |
| `0x411e03` | `MacPtInRect` |
| `0x41198a` | `MacPt2Rect` |
| `0x41141f` | `LoadPointList` |
| `0x4114da` | `PointList_At` |
| `0x419c1a` | `cEditField::cEditField` (parses the filter spec) |
| `0x447d38` / `0x447d67` / `0x447d8d` / `0x447d59` / `0x447d94` | `cGate::cGate` / `cGate::cGate(load)` / `cGate::HasBool` / `cGate::GetBool` / `cGate::~cGate` |
| `0x440f2c` / `0x4411aa` / `0x44124a` | `cAndGate::Init` / `::Tick` / `::Draw` |
| `0x44e464` / `0x44e6e2` / `0x44e782` | `cOrGate::Init` / `::Tick` / `::Draw` |
| `0x45cd70` / `0x45cfee` / `0x45d09a` | `cXorGate::Init` / `::Tick` / `::Draw` |
| `0x44c2d4` / `0x44c54b` / `0x44c5b0` | `cNotGate::Init` / `::Tick` / `::Draw` |
| `0x44c778` / `0x44c85c` / `0x44c9c8` / `0x44ca1a` / `0x44ca43` / `0x44cc85` | `cNumberCruncher::Init` / ctor / load ctor / `Reset` / `Draw` / `Tick` |
| `0x44dc58` / `0x44def5` / `0x44e195` | `cOneNumberCruncher::Init` / `::Tick` / `::Draw` |
| `0x44281c` / `0x442ac3` / `0x442bf2` | `cComparison::Init` / `::Tick` / `::Draw` |
| `0x443994` / `0x443c3b` / `0x443e93` | `cConversion::Init` / `::Tick` / `::Draw` |
| `0x453f1c` / `0x45402a` / `0x4540f0` | `cSplitter::Init` / `::Tick` / `::Draw` |
| `0x44b534` / `0x44b9cf` / `0x44b7b8` / `0x44b96f` | `cMultiplexer::Init` / `::Tick` / `::Draw` / `::Reset` |
| `0x444218` / `0x44489f` / `0x444869` / `0x44455e` / `0x444531` | `cCounter::Init` / `::Tick` / `::CountOne` / `::Draw` / `::Reset` |
| `0x45be1c` / `0x45c4ac` / `0x45c0b4` / `0x45c3b4` / `0x45c494` | `cTotaler::Init` / `::Tick` / `::Draw` / `::Reset` / `::Start` |
| `0x457604` / `0x457ba2` / `0x4578c8` / `0x4578b0` / `0x457b77` / `0x457e95` | `cStopper::Init` / `::Tick` / `::Draw` / `::Reset` / `::Start` / `::FillDialog` |
| `0x4506dc` / `0x450aff` / `0x450972` / `0x450cdb` | `cRandomNumberGenerator::Init` / `::Tick` / `::Draw` / `::FillDialog` |
| `0x442190` / `0x4426a8` / `0x4423da` | `cCoinToss::Init` / `::Tick` / `::Draw` |
| `0x442e22` / `0x442dc4` / `0x44369b` / `0x443885` / `0x44326f` / `0x4435ed` / `0x44385e` | `cConstant::Init` / `::SettingsChanged` / `::Click` / `::Tick` / `::Draw` / `::Reset` / `::Start` |
| `0x449b82` / `0x449b24` / `0x449e13` / `0x44a0e9` / `0x44a18d` | `cMiniConstant::Init` / `::SettingsChanged` / `::Draw` / `::Reset` / `::Start` |
| `0x447624` / `0x4476a3` / `0x447986` | `cFirstThru::Init` / ctor / `::Tick` |

---

## 18. Corrections to gadget-model.md

1. **Slot 0x30 base is not a no-op.** `0x41db39` returns `fld qword
   [this+0x146]`. The number output lives at `+0x146` (f64) by default,
   and most parts here rely on it.
2. **The getter slots take the knot.** They are called as
   `slot(this, u16 out_knot)` (the wire's `+8`). No class in this file
   uses the knot.
3. **`0x41e56e` returns defined values in every case**: unwired 0.0;
   object source 1.0/0.0 (truthiness of the object, not a number);
   number; bool 1.0/0.0; a source with none of the three gives
   **-1.0**. The bool-style helper `0x43adec` has the opposite priority
   (bool > number > object).
4. **Start (slot 0x20) has two modes** chosen by `+0x13d`. 0 (the
   default) primes the knot cache with the current values and does
   **not** clear wire flags. 1 writes the "none" sentinel and clears the
   flags. The sentinel `0xADDC4A11FF61B241` is −8.888e−88, not a NaN.
5. **Reset (slot 0x1c)** is called on every gadget when the run
   **stops** (`0x424a94`) and from the constructors, not at run start.
   Start (slot 0x20) is called at run start (`0x424a12`). The base reset
   also clears `+0x140` (error flag; it invalidates if it was set),
   `+0x141` (animation step) and `+0x196` (sticky decimals), and sets
   `+0x13f = 1`.
6. More base fields: `+0x13d` = start mode (above); `+0x140` = error,
   drawn with slot 0x10; `+0x141` = animation step; `+0x142` = i32 class
   value; `+0x146` = f64 number output; `+0x152` = last `0x41e762` time
   (never reset); `+0x196` = sticky decimal count for `0x410f93`; `+0x2b`
   = the dialog live-update callback (default `0x41eb78`).
7. **cNotGate is not a cGate.** It derives from cGadget directly.
   `cFirstThru` is not a base class: it is a concrete, unreachable gadget
   (`1thr`). The RTTI name of the gate base (vtable `0x499a88`) was not
   found as a string. "cGate" is only a working name.
8. **Settings block, number fields**: the four "number fields" are the
   dialog's **combo boxes** (selected index, u16 zero-extended). Edit
   boxes are the text fields, check boxes are the bits, and radio groups
   are keyed by the last character of the button captions. All are
   indexed by ascending control id (§1.6).
9. The tick loop `0x42498f` stops as soon as run mode ends during a tick
   (the Stopper).
10. `+0x14`/`+0x16`: `+0x14` is `v` (y) and `+0x16` is `h` (x), as the
    load constructor (`0x41d166`) and every draw/click use them.
11. Gadget info (type 1021) gives `dialog = id` and `doc = id + 2000`
    (exception: 2014 → 4013). Note that `rand` uses id 1007 (doc 3007).

---

## Appendix A: 1ncr factorial table (`0x49a3fc`, 171 × f64, little-endian bit patterns, index 0..170)

```
    3ff0000000000000 3ff0000000000000 4000000000000000 4018000000000000 4038000000000000 405e000000000000
    4086800000000000 40b3b00000000000 40e3b00000000000 4116260000000000 414baf8000000000 418308a800000000
    41bc8cfc00000000 41f7328ccc800000 42344c3b23500000 427307771ba80000 42b3077777358000 42f437eed20ca000
    4336beecdbfd3c00 437b02b92ee2ab80 43c0e1b3bd4dab30 4406283bd9e8a65e 444e7752dd24cbeb 4495e5c3645082d8
    44e06c5267b56fb2 4529a940c20b7e86 4574d984c05b1f8e 45c19787f4bf7035 460ec92d86aa2a35 465be65186dc7f4b
    46aa27ecb09b6007 46f956ad1c8f1e6a 474956accf02c8bb 479a2162864262ed 47ebc3789bb7c631 483e5dcc4cec8d4d
    489114c29e3b89b1 48e3c000d4669767 4937740147752073 498c95616e2cce02 49e1dd5d0997f530 4a36e39f53494862
    4a8e0ac0c00af460 4ae42f398c3d3829 4b3bc0ef82eb71f4 4b9383a7fa9393fd 4bec0d41bca05fd0 4c4499bc1915e720
    4c9ee69b30e7a17d 4cf7a88e5bfcbd48 4d527baf04e2d7fd 4dad751f726bb7b0 4e07ef296523bb76 4e63d20e1dc0386b
    4ec0b93bfefae653 4f1cbe5e994f1e7b 4f7926933cd70bc4 4fd6665b2054bf37 50344cc26123a8e6 5092b6c333d06ff3
    50f18b5723f316cc 5150b8cf0a653ab7 51b033087d580104 520fe47899156df3 526fe478667b2d3a 52d032057666f250
    5330b3958020d9ae 53917c008fcebab5 53f293c0da25364b 5454074c0a9b2d91 54b5e7fa9230fa6d 55184d5a3880e394
    557b5705858f1780 55df2f420874d4da 5642075223beb47c 56a52094659b61ae 570916b0890649e6 576e2f4c30892ebb
    57d264d23c865c70 5836b473a4c6275b 589c6190a8946d8c 5901f5bd7d2a55ac 596702daae612dd5 59cdd7b3c17c9510
    5a33958de71d0cd9 5a9a02a076f2950f 5b0179c413b0325d 5b67c18606665694 5bd0550c0cc36ec1 5c36b6458842167c
    5c9ff05137590582 5d06b4da3953a719 5d7051fc204b13ab 5dd7b7238a2acf90 5e416a7d72dcfcc3 5ea9da12b475d55a
    5f13638dcc4d2760 5f7d62e2fc9ba6c2 5fe67fb5b62ed9e7 605166c6a70fbc51 60bb30966508963d 61257456a35219b3
    619118b50b56eada 61fb83c35dc88c99 62665b0f27fece94 62d256b2525b7c46 633e5f97a86c77a8 63a963e7de18bccc
    64156c4c0fed5b4b 64823e38f432c507 64ef5af1e3b742a4 655b30ddb3f25195 65c7cac1b4ad148a 663500ff0fb7a790
    66a2b4e32787d240 6710ce84158406e6 677e764f60f8f33b 67ebd8245045d63d 6859ab4199421162 68c7dd3700ca98a6
    69365f63858515ee 69a5262c246ff89e 6a142861f8044700 6a835ecdb618909d 6af2c3d7efa092d7 6b6253408e5c165a
    6bd209f389957039 6c41e5dfbf69ea99 6cb1e5dfcefccb5d 6d2209ab733a95dd 6d9251d217861a85 6e02bfbd27596e7c
    6e7355bb69ecbbf7 6ee4171495892411 6f55082973c1fdde 6fc62e9bb12c1233 703791854dfcc24e 70a939c0b511b164
    711b32438d214e32 718d889572f63ebe 720026b18e963e52 7271ca9ff3d93d17 72e3bcc9378c8e83 73560ce8fa211fb2
    73c8ce8607663218 743c19f3c2cead98 74b006ccd83fd140 752267cf89a5ceaa 75954807ebfb451e 7608c5d952e2a943
    767d07da8281e35c 76f11fa1e84e947b 7764559058ad4ea0 77d84e4679bca94e 784d3e2cf092d499 78c1b4a516a5928a
    793594292fb96013 79aa77baadcd2db9 7a2055e52e652c68 7a944ab2782d0cd9 7b095d5f5e6d80cd 7b7fe771c7a47d8c
    7bf43075c67ddb42 7c69b5b6a29e9870 7ce07868e8a41d2a 7d553b3724e74a05 7dcb88cb7684fa7e 7e41f63d11b27927
    7eb7932fcf8b8f7e 7f2f2054ea339b25 7fa4ab7876a34240
```

## Open questions

* The behaviour on a real FP overflow or exception (Add/Sub of huge
  values, `sin`/`cos`/`tan` of |x| ≥ 2^52 → RTL `matherr` TLOSS) was not
  traced.
* Whether Borland's `printf` prints `%g` exponents with 2 or 3 digits was
  not checked in the binary (`0x4815b4`…). Two digits is assumed.
* The exact enable/disable wiring done by `0x417c7b` in the Stop and rand
  fill-dialog overrides was not traced. It is UI-only.
* The edit filter flag `A` (`+0x111` of the edit wrapper) is uninterpreted.
  No part here uses it.
