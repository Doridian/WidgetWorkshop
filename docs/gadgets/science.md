# The science lab parts

cPendulum (`Pend`), cDropChamber (`Drop`, the "Gravity Chamber"),
cPhysicsCanon (`Cano`, "Cannon"), cPlanet (`Plnt`, "Solar System") and
cAreaVolume (`ArVo`, "Area / Volume"), as `WIDGET.EXE` (Win95) implements
them. Read [`../gadget-model.md`](../gadget-model.md) first. All addresses
are in `WIDGET.EXE`; all constants below were read from the binary
(doubles unless said otherwise; "x87 ext" = 80-bit constant).

Notation used in the pseudocode:

- `elapsed()` = `doc+0x4b` via `0x424a89(DAT_004a2848)`: Mac ticks
  (1/60 s) since the run's first tick (the first tick of a run sees 0).
  The tick count is `trunc(timeGetTime() * 0.06)` (`0x4113a8`).
- `throttle(n)` = `0x41e762(this, n)`:
  `now = elapsed(); if (u32)(now - this.+0x152) > n { this.+0x152 = now; true } else false`.
  So `throttle(6)` passes at most every 7 ticks. `+0x152` is a base field,
  zeroed by the base init and never by reset.
- `input(k)` = `0x41e56e(this, k)`: the value at input knot `k` (0.0 when
  unwired; an object output reads as 1.0/0.0).
- `changed(k)` = `0x41e713`: the wire's changed flag at knot `k`, read
  and cleared.
- `value_changed(k)` = `0x41e62a`: re-reads `input(k)`; if it differs
  from the cached `+0x156 + 8k`, stores it and returns true when wired.
- `fire(k)` = `0x41e6cf(this, k)`: set the output wire's changed flag.
- `play(id)` = `0x4402cc(id, 0)`: plays WWSOUND type 1006 resource `id`
  once (no-op when sound is off).
- `invalidate()` = vtable slot 0x0c.
- `parse_num(s)` = `0x4110c3`: `sscanf(s, "%lf")` into a 0.0-initialised
  double, then: `0` stays `0`; positive values clamped to
  `[1e-300, 1e300]`; negative values clamped to `[-1e300, -1e-300]`.
- `trunc()` = `0x480ec4` (`fistp` with rounding mode "chop"): truncation
  toward zero. `0x439e3d(x)` = `trunc` with saturation:
  `x >= 2147483647.0 → 0x7ffffffe`, `x <= -2147483646.0 → 0x80000002`.
- `sq(x)` = `0x44f0d7`: `f = modf(x * (1/2π as x87 ext 0.15915494309193554), &ip); return f < 0.5 ? 1.0 : -1.0`
  (sign of `sin x` for `x >= 0`; the product is rounded to a double before `modf`).
- `sin` = `0x4811e4`, `cos` = `0x480c00`, `sqrt` = `0x481228`, `modf` =
  `0x480f54`.
- `draw_frame(sheet, n, pt)` = `0x422d32`: draws frame `n` (from 1) of an
  HR sheet with its rect offset by `pt` (`pt` is a Mac Point: `v` = y,
  `h` = x; every frame rect used here has top-left `(0,0)`, so `pt` is the
  frame's top-left).
- `pos` = `+0x14` (`pos.v` at +0x14, `pos.h` at +0x16): the gadget's
  top-left on the worksheet.

Shared physics tables (`WIDGET.EXE` .data):

| address | name | values (index: value) |
|---|---|---|
| `0x4903bc` | `HEIGHTS_FT[6]` | 0: 29029.0, 1: 4190.0, 2: 1472.0, 3: 984.0, 4: 20.0, 5: 6.0 |
| `0x4903ec` | `GRAVITY_FT_S2[4]` | 0: 901.1800000000001 (Sun), 1: 75.9566 (Jupiter), 2: 32.185 (Earth), 3: 5.471450000000001 (Moon) |
| `0x49ab88` | `CANNON_SPEED_FT_S[5]` | 0: 36360.13333333333 (24 791 mph), 1: 1466.6666666666665 (1000 mph), 2: 822.8 (561 mph), 3: 132.0 (90 mph), 4: 0.0 ("None") |

Units are feet and seconds throughout. `HEIGHTS_FT` is immediately
followed by `GRAVITY_FT_S2` (0x4903bc + 6·8 = 0x4903ec).

## Gadget ids 1000–1019 and 2000–2046

From the `+0x28` store in each class's init function (the function both
constructors call), the tag stored at `+4`, and the resources. "Res" lists
which of WWGADGET 1021 (info) / 1020 (Doc text) / WIDGET RT_DIALOG exist
for the id. 1021/<id> is `id, kind, dialog, doc` with `dialog == id` and
`doc = id + 2000` for every entry (Doc text 1020/<id> holds the help text;
1020/<id+2000> also exists).

| id | tag | class | init fn | notes |
|---|---|---|---|---|
| 1000 | `Pend` | cPendulum | 0x44e944 | this doc |
| 1001 | `Drop` | cDropChamber | 0x446614 | this doc |
| 1002 | `Cano` | cPhysicsCanon | 0x44f498 | this doc |
| 1003 | — | — | — | 1021 + Doc text "The Tape Player…" exist, no dialog, no class sets 1003. The Win95 `Tape` class uses 2026. Unused. |
| 1004 | `Wtch` | cStopWatch | 0x45813c | |
| 1005, 1006 | — | — | — | no resources |
| 1007 | `rand` | cRandomNumberGenerator | 0x4506dc | |
| 1008 | `Fork` | cTuningFork | 0x45c710 | |
| 1009 | `DClk` | cDigitalClock | 0x444a08 | |
| 1010 | `Key_` | (Keypress) | 0x448844 | Doc: "The Keypress…" |
| 1011 | `Metr` | cMetronome | 0x4494ec | |
| 1012 | `swit` | cSwitch | 0x45989a | |
| 1013 | — | — | — | no resources |
| 1014 | `di10` | cDisplay10 | 0x44550c | |
| 1015 | `Sldr` | cSlider | 0x4516c5 | |
| 1016 | `bulb` | cLightBulb | 0x448fe8 | |
| 1017 | `hart` | cHeart | 0x440880 | |
| 1018 | `rgbd` | cRGBDisplay | 0x450ddc | Doc: "The Color Mixer" |
| 1019 | — | — | — | 1021 + Doc text "The Splitter…" exist, no dialog, no class. `Spl2/3/4` use 2022–2024. Unused. |
| 2000 | `Plnt` | cPlanet | 0x44feec | this doc (pictures/knots from resource 1022!) |
| 2001 | `Ttlr` | cTotaler | 0x45be1c | |
| 2002 | `disp` | cDisplay | 0x445b98 | |
| 2003 | `ArVo` | cAreaVolume | 0x441a8c | this doc (knots 1024, pictures 1025) |
| 2004 | `cnst` | cConstant | 0x442e22 | |
| 2005 | `1ncr` | cOneNumberCruncher | 0x44dc58 | |
| 2006 | `Coin` | cCoinToss | 0x442190 | |
| 2007 | `NmKP` | cNumericKeyPad | 0x44d0b6 | |
| 2008 | `AndG` | cAndGate | 0x440f2c | |
| 2009 | `OrGa` | cOrGate | 0x44e464 | |
| 2010 | `NotG` | cNotGate | 0x44c2d4 | |
| 2011 | `XorG` | cXorGate | 0x45cd70 | |
| 2012–2015 | — | — | — | 1021 + Doc ("Equal Comparitor", "Comparitor"…) only; replaced by `Comp` (2021). Unused. |
| 2016 | `Addr` | cAdder | ctor 0x440e20 | `+0x28` set in the ctors (`*(short*)(this+0x28)`, written `param_1 + 10` as int) |
| 2017 | `Subr` | cSubtracter | ctor 0x459718 | same |
| 2018 | `Mulr` | cMultiplier | ctor 0x44bc00 | same |
| 2019 | `Divr` | cDivider | ctor 0x446504 | same |
| 2020 | `Conv` | cConversion | 0x443994 | |
| 2021 | `Comp` | cComparison | 0x44281c | |
| 2022 | `Spl2` | cSplitter2 | ctor 0x45429c | |
| 2023 | `Spl3` | cSplitter3 | ctor 0x454540 | |
| 2024 | `Spl4` | cSplitter4 | ctor 0x4547e4 | |
| 2025 | `Mplx` | cMultiplexer | 0x44b534 | Doc: "The Switcher" |
| 2026 | `Tape` | cTapePlayer | 0x45a057 | Doc: "The recorder" |
| 2027 | `TxBx` | cTextBox | 0x45a801 | Doc: "The Billboard" |
| 2028 | `TxLn` | cTextLine | 0x45b70a | |
| 2029 | `MsMv` | cMouseMove | 0x44ae04 | |
| 2030 | `DnPl` | cDynamitePlunger | 0x447174 | |
| 2031 | — | — | — | 1021 only. Unused. |
| 2032 | `MsCl` | cMouseClick | 0x44a8b0 | |
| 2033 | `Cntr` | cCounter | 0x444218 | |
| 2034 | `Stk1`..`St10` | cSticker1..10 | cSticker base 0x454adc / 0x454b0d | all ten stickers share 2034 |
| 2035 | `GrPr` | cGraphicProjector | 0x447dcc | |
| 2036 | — | — | — | 1021 only. Unused. |
| 2037 | `SdSw` | cSoundSwitch | 0x45279c | |
| 2038 | `SdTr` | cSoundTransformer | 0x452f24 | |
| 2039 | — | — | — | 1021 only. Unused. |
| 2040 | `AnSd` | cAnimalSound | 0x441410 | |
| 2041 | `MsSd` | cMiscSound | 0x44a24c | |
| 2042 | `Spkr` | cSpeaker | 0x453960 | |
| 2043 | `Stop` | cStopper | 0x457604 | Doc: "Win / Lose Checker" |
| 2044 | `StCh` | cStripChart | 0x458b9f | |
| 2045 | `TxLb` | cTextLabel | 0x45b0fd | |
| 2046 | `MiCo` | cMiniConstant | 0x449b82 | |

Also: 501 (`0x1f5`) is set by `0x44c778` (vtable 0x49a2e0, ctors
0x44c85c/0x44c9c8), the cNumberCruncher base that Addr/Subr/Mulr/Divr
construct first and then overwrite with their own id.

All 64 factory tags are accounted for. Note the ids are the dialog and
Doc ids, **not always the resource ids** of the part's pictures, points
and knots: cPlanet uses 1022 for those, cAreaVolume uses 1024 (knots) and
1025 (pictures/points).

## Settings dialogs: how controls map to the settings block

From the dialog builder (`0x41acb9`, its loop over control ids
1..0xa9 in **id order**), reading the settings block via `0x41ea4d`
(fill) / `0x41e851` (read back):

- **Radio groups** (`block+4+g`, a u8 per group, gadget `+0x39+g`): a
  BS_AUTORADIOBUTTON's title ends in a group key character, which is
  stripped from the displayed text. Groups are numbered in order of the
  first button (by control id) carrying each key; within a group, the
  choice value is the button's index in control-id order.
- **Combo boxes** (`block+8+4c`, a u32 per combo, gadget `+0x3d+4c`): the
  selected item index; combos are numbered in control-id order. Their
  strings come from WIDGET type 1022 resource `<dialog id>` (each entry
  tagged with the combo's control id).
- **Edit controls** (`block+0x18+60t`, gadget `+0x4d+60t`): numbered in
  control-id order. The template's title is a format spec parsed by
  `0x419c1a`, then cleared: decimal digits = maximum length, `#` = digits
  only, `.` = allows a decimal point, `-` = allows a minus sign, `A` = a
  flag at +0x111 (not used here).
- A part's slot 0x48 override can bind a radio button to a control
  (`0x417c7b(radio_button, control)`, stored at button+10); presumably the
  bound control is enabled only while that button is selected (not
  verified in the dialog code).
- Static 104 is the output-description caption; button 102 `Pic<n>`
  shows WWGADGET 1004 picture sheet `<n>` frame 1 (`Pic` without a number
  creates a different control, `0x418cd0`, given the gadget: probably a
  live preview; not traced); 103 is the Doc button.

The radio-group key quirk matters: in the Pendulum dialog the
"Preset/Custom" buttons have key `2` and the output buttons key `1`, but
the lower id (106, "Preset:2") comes first, so "Preset/Custom" is group 0.

---

## cPendulum (`Pend`)

| | |
|---|---|
| tag | `Pend` (`0x646e6550`) |
| class / vtable | cPendulum, `0x49ab30` |
| size | 0x1b3 |
| `+0x28` gadget id | 1000 (dialog 1000, Doc text 1020/1000, info 1021/1000) |
| picture sheet | WWGADGET 1004/1000 (loaded once, refcount `DAT_004a32a4`, sheet ptr `DAT_004a32a0`) |
| point table | WWGADGET 1010/1000 → `DAT_004a32a6`: one point `(v 21, h 84)` = the pivot |
| knots | 1014/1000 |
| sounds | WWSOUND 1006/1000 at each centre crossing |
| bounds `+0x18` | frame 1's rect `(0,0,119,167)` |

Knots (1014/1000, relative to `pos`):

| # | v, h | kind | meaning |
|---|---|---|---|
| 0 | 81, 4 | in | release: any non-zero value starts the swing |
| 1 | 81, 161 | out | period (s) or "at centre" pulse, per setting |

Frames (sheet 1000, png `ghidra/png/WWGADGET/1000.png`): 1 = the case
(167×119), 2 = the bob (17 wide × 18 tall), 3 = 1×1 (unused).

### State

| offset | type | meaning |
|---|---|---|
| +0x197 | f64 | previous phase angle (for crossing detection) |
| +0x19f | f64 | current phase angle φ (radians) |
| +0x1a7 | u32 | release time (`elapsed()` when released); 0 = not released |
| +0x1ab | f64 | period P (seconds) |
| +0x13e | u8 | bool output (base field) |
| +0x146 | f64 | number output (base field) |

Settings: `+0x39` radio 0 (0 Preset, 1 Custom); `+0x3a` radio 1 (0
Period Length, 1 On at Center); `+0x3d` combo 0 (length preset, index into
`HEIGHTS_FT`); `+0x41` combo 1 (gravity, index into `GRAVITY_FT_S2`);
`+0x45` combo 2 (weight: **read by nothing**; mass doesn't matter);
`+0x4d` text 0 (custom length, ft).

### Functions

```c
// 0x44e944 init (both ctors)
if (refcount++ == 0) { sheet = load_sheet(1000); load_points(&pts, 1000 /*1010*/); sound_ref(); }
tag = 'Pend'; bounds(+0x18) = frame_rect(sheet, 1); id(+0x28) = 1000;

// 0x44ea05 fresh ctor (pt = where dropped)
cGadget_ctor(); vtable = 0x49ab30; init();
pos.v = pt.v - (rect.top  + (rect.bottom - rect.top)/2);   // C division, toward 0
pos.h = pt.h - (rect.left + (rect.right - rect.left)/2);
load_knots(1000); reset();
+0x3a = 1;   // output: On at Center
+0x3d = 2;   // length: 1,472 ft (Empire State Bldg.)
// +0x39 = 0 (Preset), +0x41 = 0 (Sun!), +0x45 = 0 left from cGadget_ctor
invalidate();

// 0x44eb8e load ctor: cGadget_load(chunk); vtable; init(); load_knots(1000); reset(); invalidate();

// slot 0x1c reset, 0x44ec1a
base_reset();                       // 0x41cef4: bool=0, number=0, ...
+0x197 = +0x19f = 3π/2;             // 4.7123889803834995 (0x49aaa4)
+0x1a7 = 0;

// slot 0x20 start, 0x44f137
base_start();                       // 0x41e50a
L = (+0x39 != 0) ? parse_num(text0) : HEIGHTS_FT[+0x3d];
P = sqrt(L / GRAVITY_FT_S2[+0x41]) * 6.283185307178;   // 2π as stored at 0x44f1af
+0x1ab = P;

// slot 0x24 has_bool, 0x44ebe5:   return +0x3a != 0;
// slot 0x2c has_number, 0x44ebf6: return !has_bool();
// slots 0x28/0x30 are the base getters (+0x13e / +0x146)

// slot 0x44 tick, 0x44f1b7
if (!throttle(6)) return;
if (+0x1a7 == 0) {
    if (input(0) != 0.0) +0x1a7 = elapsed();     // (if elapsed() is 0 the release is retried)
    return;
}
t = (double)(u32)(elapsed() - +0x1a7) * (1/60 as x87 ext);   // seconds, stored as f64
if (P != 0.0) φ = 6.283185307178 * t / P + 3π/2;        // order: (2π·t)/P + 3π/2
else          φ = 3π/2;
+0x19f = φ;
crossed = sq(+0x197) * sq(φ) < 0.0;
switch (+0x3a) {
  case 0: number = P; bool = 0; break;            // Period Length
  case 1: bool = crossed; number = 0.0; break;    // On at Center (one throttled tick long)
}
+0x197 = φ;
if (crossed) { play(1000); fire(1); }
invalidate();
```

Physics: no integration; the swing is the closed form
`θ(t) = 0.6283185307178001 · sin(2πt/P + 3π/2) = −0.628·cos(2πt/P)`
(an amplitude of 36°), with the small-angle period `P = 2π√(L/g)`, L and g
in feet. Before release φ = 3π/2: the bob is held at the left extreme.
`sq` flips when φ passes a multiple of π, i.e. at t = P/4, 3P/4, … — the
bob passing the bottom. Because the check runs at most every 7 ticks, a
period shorter than ~14 ticks can miss crossings (sq is compared between
samples). The Period output is 0 until released, then P every throttled
tick; `fire(1)` happens only on crossings, in both modes, and so does the
sound.

```c
// slot 0x04 draw(dc, clip), 0x44ec4b
if (!intersects(bounds + pos, clip)) return;
draw_frame(sheet, 1, pos);
a  = (float)(sin(+0x19f) * 0.6283185307178001);   // stored as f32, reloaded
dy = trunc(cos(a) * 74.0f);                       // 74 px string
dx = trunc(sin(a) * 74.0f);
pen = CreatePen(PS_SOLID, 0, RGB(255,255,0));     // yellow, 1 px
px = pos.h + 84; py = pos.v + 21;                 // pivot from 1010/1000
MoveTo(px, py); LineTo(px + dx, py + dy);
draw_frame(sheet, 2, (v = pos.v + 21 + dy - 9, h = pos.h + 84 + dx - 8));
//   9 = (18-0)/2, 8 = (17-0)/2: the bob's half height/width (truncating)
// restore pen
```

```c
// slot 0x48 fill dialog, 0x44f393
bind(radio0.button0 /*Preset*/, combo0 /*length*/);
bind(radio0.button1 /*Custom*/, text0);
base_fill_dialog();     // 0x41ea4d
```

Destructor 0x44f05b releases the sheet and points when the refcount
reaches 0.

### Dialog 1000 "Pendulum"

| ctl id | control | block field |
|---|---|---|
| 106 "Preset:" / 114 "Custom:" | radio group 0 (key `2`) | `+0x39`: 0/1 |
| 113 "Period Length" / 117 "On at Center" | radio group 1 (key `1`) | `+0x3a`: 0/1 |
| 107 combo | combo 0 | `+0x3d`: 0 "29,029 ft.(Mt. Everest)", 1 "4,190 ft.(Mt. Vesuvius)", 2 "1,472 ft.(Empire State Bldg.)", 3 "984 ft.(Eiffel Tower)", 4 "20 ft.(Tyrann. Rex)", 5 "6 ft.(Man)" |
| 109 combo "Gravity:" | combo 1 | `+0x41`: 0 "Sun (27.9 g)", 1 "Jupiter (2.87 g)", 2 "Earth (1g)", 3 "Moon (.17g)" |
| 111 combo "Weight:" | combo 2 | `+0x45`: 0 "Earth (1.32E+25 lbs.)", 1 "Elephant (13,000 lbs.)", 2 "Human (175 lbs.)", 3 "Fly (.00031 lbs.)" — no effect |
| 115 edit `5#` | text 0 | `+0x4d`: custom length in ft, ≤5 digits |
| 102 `Pic4000` | | picture: WWGADGET 1004/4000 |

Strings: WIDGET 1022/1000.

---

## cDropChamber (`Drop`, "Gravity Chamber")

| | |
|---|---|
| tag | `Drop` (`0x706f7244`) |
| class / vtable | cDropChamber, `0x4998e0` |
| size | 0x1ab |
| `+0x28` | 1001 (dialog 1001, Doc 1020/1001) |
| sheet | WWGADGET 1004/1001 (`DAT_004a31d0`, refcount `DAT_004a31d4`) |
| points | WWGADGET 1010/1001 → `DAT_004a31d6` (19 points, below) |
| knots | 1014/1001 |
| sounds | 1006/1101 (claw opens), 1006/1001 (hits the floor) |
| bounds | frame 1 `(0,0,201,60)` |

Knots:

| # | v, h | kind | meaning |
|---|---|---|---|
| 0 | 4, 30 | in | drop: any non-zero value starts the sequence |
| 1 | 195, 33 | out | bool: on when the object hits the floor |

Frames (sheet 1001): 1 tube (60×201); 2 1×1; 3 empty claw; feather 4–7
(claw holding, 4 steps of opening), 8 falling, 9 landed; elephant 10–13,
14, 15; baseball 16–19, 20, 21.

Points (1010/1001, index from 0, `(v, h)` relative to `pos`):
0 `(43,10)` empty claw; feather 1–4 `(43,4) (43,3) (43,4) (43,3)`, 5
`(60,4)` falling start, 6 `(143,3)` landed; elephant 7–10 `(43,9) (43,9)
(43,10) (43,10)`, 11 `(60,11)`, 12 `(140,8)`; baseball 13–16 `(43,15)
(43,15) (43,11) (43,11)`, 17 `(60,20)`, 18 `(155,21)`.

Per-object tables (i16, index = thing):

| address | name | feather | elephant | baseball |
|---|---|---|---|---|
| 0x499868 | `FLOOR_PX` | 79 | 77 | 94 |
| 0x49986e | `FRAME_BASE` | 4 | 10 | 16 |
| 0x499874 | `POINT_BASE` | 1 | 7 | 13 |

### State

| offset | type | meaning |
|---|---|---|
| +0x141 | u8 | phase (base field, zeroed by base reset): 0 waiting, 1–3 claw opening, 4 falling, 5 landed |
| +0x197 | f64 | drop height H (ft) |
| +0x19f | u32 | fall start time (`elapsed()`) |
| +0x1a3 | i32 | fall distance in px (y offset of the falling picture) |
| +0x1a7 | i32 | previous fall distance in px |
| +0x13e | u8 | bool output |
| +0x146 | f64 | speed g·t (ft/s); computed but **never output** (has_number is false) |

Settings: `+0x39` radio 0 (0 Preset, 1 Custom); `+0x3d` combo 0
(gravity); `+0x41` combo 1 (height preset); `+0x45` combo 2 (thing: 0
Feather, 1 Elephant, 2 Baseball); `+0x4d` text 0 (custom height).

### Functions

```c
// 0x446614 init: sheet 1001, points 1010/1001, two sound refs; tag 'Drop';
//   bounds = frame 1; +0x2a = 1; +0x28 = 1001.
// 0x4466e7 fresh ctor: centre on pt (as Pendulum); load_knots(1001);
//   +0x45 = 1 (Elephant); +0x3d = 2 (Earth); +0x41 = 2 (1,472 ft); reset(); invalidate().
// 0x44687d load ctor: load; init; load_knots(1001); reset(); invalidate().

// slot 0x1c reset, 0x4468e2
base_reset();            // also sets phase +0x141 = 0, bool = 0
+0x1a7 = +0x1a3 = +0x19f = 0;

// slot 0x20 start, 0x44690d
base_start();
if (+0x39 != 0) sscanf(text0, "%lf", &+0x197);   // raw: no clamping; on failure +0x197 keeps its old value
else            +0x197 = HEIGHTS_FT[+0x41];

// slot 0x24 has_bool, 0x4468d4: return true
// slot 0x2c has_number, 0x4468db: return false

// slot 0x44 tick, 0x446de0
switch (+0x141) {
case 0: if (input(0) != 0.0) { +0x141 = 1; invalidate(); } break;
case 1: if (throttle(5)) { +0x141 = 2; invalidate(); } break;
case 2: if (throttle(5)) { +0x141 = 3; invalidate(); play(1101); } break;
case 3: if (throttle(5)) { +0x141 = 4; +0x19f = elapsed(); invalidate(); } break;
case 4: {
    t = (double)(u32)(elapsed() - +0x19f) * (1/60 as x87 ext);
    d = t * t * GRAVITY_FT_S2[+0x3d] * 0.5;            // ft fallen, this order
    if (+0x197 == 0.0) +0x1a3 = 0x7fffffff;
    else               +0x1a3 = trunc((d / +0x197) * (double)FLOOR_PX[+0x45]);
    +0x146 = t * GRAVITY_FT_S2[+0x3d];
    if ((u32)+0x1a3 > (u32)+0x1a7) {
        if ((u32)+0x1a3 >= (u32)FLOOR_PX[+0x45]) {    // landed
            +0x1a3 = FLOOR_PX[+0x45];
            +0x146 = 0.0; +0x13e = 1;
            play(1001);
            +0x141 = 5;
            fire(1);
        }
        +0x1a7 = +0x1a3;
        invalidate();
    }
    break; }
case 5: default: break;   // stays landed until reset
}
```

Physics: closed form `d = ½gt²` (no integration), with t from the run
clock; the picture moves `FLOOR_PX·d/H` pixels, so it lands when
`t ≥ √(2H/g)` (to the pixel). All three objects fall alike (vacuum);
only their floor offsets differ. Each phase step 1→2→3→4 needs
`throttle(5)` (≥ 6 ticks); the steps can be faster than that only if
`+0x152` was left far in the past.

```c
// slot 0x04 draw, 0x446966
if (!intersects) return;
draw_frame(sheet, 1, pos);
th = +0x45; ph = +0x141;
if (ph < 4) {
    p = pts[POINT_BASE[th] + ph];
    draw_frame(sheet, FRAME_BASE[th] + ph, (pos.v + p.v, pos.h + p.h));
} else {
    p = pts[0];  draw_frame(sheet, 3, (pos.v + p.v, pos.h + p.h));          // empty claw
    if (FLOOR_PX[th] == +0x1a3) {
        p = pts[POINT_BASE[th] + 5];
        draw_frame(sheet, FRAME_BASE[th] + 5, (pos.v + p.v, pos.h + p.h)); // landed
    } else {
        p = pts[POINT_BASE[th] + 4];
        draw_frame(sheet, FRAME_BASE[th] + 4,
                   (pos.v + p.v + (i16)+0x1a3, pos.h + p.h));              // falling
    }
}

// slot 0x48 fill dialog, 0x44706c
bind(radio0.button0 /*Preset*/, combo1 /*height*/);
bind(radio0.button1 /*Custom*/, text0);
base_fill_dialog();
```

Destructor 0x446d59.

### Dialog 1001 "Gravity Chamber"

| ctl id | control | field |
|---|---|---|
| 108 "Preset:" / 112 "Custom:" | radio group 0 (key `2`) | `+0x39` |
| 107 combo "Gravity:" | combo 0 | `+0x3d`: Sun, Jupiter, Earth, Moon (as Pendulum) |
| 109 combo "Height:" | combo 1 | `+0x41`: the six heights |
| 111 combo "Thing to drop:" | combo 2 | `+0x45`: 0 "Feather", 1 "Elephant", 2 "Baseball" |
| 113 edit `5#` | text 0 | `+0x4d` custom height (ft) |
| 104 | caption "On when hits" | |
| 102 `Pic4001` | | WWGADGET 1004/4001 |

Strings: WIDGET 1022/1001.

---

## cPhysicsCanon (`Cano`, "Cannon")

| | |
|---|---|
| tag | `Cano` (`0x6f6e6143`) |
| class / vtable | cPhysicsCanon, `0x49abec` |
| size | 0x1b3 |
| `+0x28` | 1002 (dialog 1002, Doc 1020/1002) |
| sheet | WWGADGET 1004/1002 (`DAT_004a32ac`, refcount `DAT_0049ab84`) |
| knots | 1014/1002 |
| sound | 1006/1002 when the ball lands |
| bounds | frame 1 `(0,0,87,201)` |

Knots:

| # | v, h | kind | meaning |
|---|---|---|---|
| 0 | 42, 4 | in | fire: any non-zero value |
| 1 | 43, 195 | out | distance / height / on-when-hits / speed |

Frames: 1 the grid display (201×87), 2 1×1 (unused), 3 the ball (11 wide
× 12 tall). No lines are drawn; there is no trail.

### State

| offset | type | meaning |
|---|---|---|
| +0x13f | u8 | armed (base field: base reset and start set it to 1; landing clears it) |
| +0x197 | u32 | fire time (`elapsed()`), 0 = not fired |
| +0x19b | i32 | ball y offset, px (0..66) |
| +0x19f | i32 | ball x offset, px (0..158) |
| +0x1a3 | f64 | launch height H (ft) |
| +0x1ab | f64 | flight time T = √(2H/g) (s) |

Settings: `+0x39` radio 0 (0 Preset, 1 Custom height); `+0x3a` radio 1
(0 Distance, 1 Height, 2 On When Hits, 3 Speed); `+0x3d` combo 0 (launch
speed, `CANNON_SPEED_FT_S`); `+0x41` combo 1 (gravity); `+0x45` combo 2
(height preset); `+0x4d` text 0 (custom height).

### Functions

```c
// 0x44f498 init: sheet 1002; sound ref; +0x2a = 1; +0x28 = 1002.
// 0x44f520 fresh ctor: tag 'Cano'; bounds = frame 1; centre on pt; load_knots(1002);
//   reset(); +0x3a = 2 (On When Hits); invalidate().   Other settings stay 0:
//   speed Apollo 10, gravity Sun, height Mt. Everest, Preset.
// 0x44f6de load ctor: load; init; tag; bounds; load_knots(1002); reset(); invalidate().

// slot 0x1c reset, 0x44f765
base_reset();                 // +0x13f = 1
+0x19f = +0x19b = +0x197 = 0;

// slot 0x20 start, 0x44fa0d
base_start();
H = (+0x39 != 0) ? parse_num(text0) : HEIGHTS_FT[+0x45];
+0x1a3 = H;
+0x1ab = sqrt(2.0 * H / GRAVITY_FT_S2[+0x41]);     // (2.0f * H) / g, all double
switch (+0x3a) {
  case 0: number = 0.0; break;
  case 1: number = H;   break;
  case 2: bool = 0;     break;
  case 3: number = 0.0; break;
}
+0x13f = 1;

// slot 0x24 has_bool,   0x44f801: return +0x3a == 2;
// slot 0x2c has_number, 0x44f812: return !has_bool();

// slot 0x44 tick, 0x44faf1  (no throttle: runs every idle tick)
if (!+0x13f) return;
if (+0x197 == 0) {
    if (input(0) != 0.0) +0x197 = elapsed();
    return;
}
g  = GRAVITY_FT_S2[+0x41];
v  = CANNON_SPEED_FT_S[+0x3d];
t  = (double)(u32)(elapsed() - +0x197) * (1/60 as x87 ext);
vy = g * t;
d  = vy * t * 0.5;                     // drop below the launch height
if (+0x1a3 < 1e-100 || +0x1ab < 1e-100) { +0x19b = 66; +0x19f = 158; }
else {
    +0x19b = 0x439e3d((d / +0x1a3) * 66.0);
    +0x19f = 0x439e3d((t / +0x1ab) * 158.0);
}
switch (+0x3a) {
  case 0: number = t * v; break;                                  // distance, ft
  case 1: number = +0x1a3 - d; break;                             // height, ft
  case 3: number = sqrt(v*v + vy*vy) * 0.6818181818181818; break; // speed, mph
}                                                                  // (case 2: nothing)
if (+0x19f >= 158) {                   // landed (t >= T)
    +0x13f = 0;
    +0x19b = 66; +0x19f = 158;
    play(1002);
    +0x197 = 0;
    fire(1);
    switch (+0x3a) {
      case 0: number = v * +0x1ab; break;
      case 1: number = 0.0; break;
      case 2: bool = 1; break;
      case 3: number = sqrt((g * +0x1ab)^2 + v*v) * 0.6818181818181818; break;
    }
}
invalidate();
```

Physics: a horizontal launch from height H, closed form, no integration:
x = v·t, drop = ½gt², vertical speed g·t, flight time √(2H/g). The screen
maps flight time to 158 px across and the drop to 66 px down (the ball
traces a parabola because of the time/height scaling, not the speed: the
picture is the same for every speed). The ball fires once per run (`+0x13f`
stays 0 after landing until the next reset/start). `fire(1)` happens only
on landing; the distance/height/speed values change every tick without a
changed flag. The "Speed" output is in mph (0.6818… = 3600/5280); distance
and height are in feet.

```c
// slot 0x04 draw, 0x44f836
if (!intersects) return;
draw_frame(sheet, 1, pos);
draw_frame(sheet, 3, (v = pos.v + 5 + (i16)+0x19b, h = pos.h + 19 + (i16)+0x19f));
// ball travels from (5,19) to (71,177) relative to pos

// slot 0x48 fill dialog, 0x44fe21
bind(radio0.button0 /*Preset*/, combo2 /*height*/);
bind(radio0.button1 /*Custom*/, text0);
base_fill_dialog();
```

Destructor 0x44f790.

### Dialog 1002 "Cannon"

| ctl id | control | field |
|---|---|---|
| 110 "Preset:" / 112 "Custom:" | radio group 0 (key `1`) | `+0x39` |
| 116 "Distance" / 117 "Height" / 118 "On When Hits" / 119 "Speed" | radio group 1 (key `2`) | `+0x3a`: 0/1/2/3 |
| 107 combo "Launching speed:" | combo 0 | `+0x3d`: 0 "Apollo 10 (24,791 mph)", 1 "Rifle bullet (1000 mph)", 2 "Jet Airplane (561 mph)", 3 "Baseball Pitch (90mph)", 4 "None" |
| 109 combo "Gravity:" | combo 1 | `+0x41`: Sun, Jupiter, Earth, Moon |
| 111 combo "Height:" | combo 2 | `+0x45`: the six heights |
| 113 edit `#5 ` | text 0 | `+0x4d` custom height (ft), ≤5 digits |
| 102 `Pic4002` | | WWGADGET 1004/4002 |

Strings: WIDGET 1022/1002.

---

## cPlanet (`Plnt`, "Solar System")

| | |
|---|---|
| tag | `Plnt` (`0x746e6c50`) |
| class / vtable | cPlanet, `0x49af20` |
| size | 0x197 (no own state) |
| `+0x28` | **2000** (dialog 2000, Doc 1020/2000, info 1021/2000) |
| sheet | WWGADGET 1004/**1022** (`DAT_004a32b0`, refcount `DAT_0049ac40`) |
| knots | 1014/**1022** (loaded in init, `0x41db6c(this, 0x3fe)`) |
| bounds | frame 14's rect `(0,0,109,118)` |

Knots: one, `#0 (v 54, h 112)`, out: the selected number.

Frames (sheet 1022): 1 Mercury, 2 Venus, 3 Earth, 4 Mars, 5 Jupiter, 6
Saturn, 7 Uranus, 8 Neptune, 9 Pluto (the code draws `planet + 1`);
10–13 (Moon, Sun, nebula, galaxy) are not used by this class; 14 the
bezel (118 wide × 109 tall); 15 1×1.

Settings: `+0x3d` combo 0 (planet 0..8, Mercury..Pluto); `+0x41` combo 1
(what to output, 0..7).

Planet table at `0x49ac42`, 9 records of 0x44 bytes:
`+0 char* name` (→ "MERCURY".."PLUTO" at 0x49aed6), then 8 doubles:

| planet | +0x04 day (h) | +0x0c year (d) | +0x14 dist (M mi) | +0x1c orbit vel (k mph) | +0x24 diameter (mi) | +0x2c density | +0x34 gravity | +0x3c mass |
|---|---|---|---|---|---|---|---|---|
| MERCURY | 59.0 | 87.97 | 36.0 | 106.26 | 3032.0 | 5.5 | 0.38 | 0.332 |
| VENUS | 243.0 | 224.0 | 67.0 | 77.62 | 7523.0 | 5.24 | 0.9 | 4.89 |
| EARTH | 24.0 | 365.26 | 93.0 | 66.0 | 7926.0 | 5.52 | 1.0 | 6.04 |
| MARS | 24.62 | 687.0 | 142.0 | 53.46 | 4420.0 | 3.94 | 0.38 | 0.642 |
| JUPITER | 9.83 | 4332.0 | 483.0 | 28.93 | 88750.0 | 1.3 | 2.87 | 1920.0 |
| SATURN | 10.23 | 10760.0 | 887.0 | 21.35 | 74580.0 | 0.7 | 1.32 | 575.0 |
| URANUS | 10.82 | 30685.0 | 1783.0 | 15.09 | 31600.0 | 1.3 | 0.93 | 88.2 |
| NEPTUNE | 15.8 | 60191.0 | 2794.0 | 12.03 | 30200.0 | 1.8 | 1.23 | 103.9 |
| PLUTO | 159.32 | 90475.0 | 3660.0 | 10.5 | 1500.0 | 0.7 | 0.03 | 0.06 |

(All values are exact as stored, e.g. 106.26 is the double nearest 106.26.)

```c
// 0x44feec init (both ctors)
if (refcount++ == 0) sheet = load_sheet(1022);
tag = 'Plnt'; bounds = frame_rect(sheet, 14); +0x28 = 2000;
load_knots(1022);

// 0x44ffa0 fresh ctor: cGadget_ctor; vtable; init; centre on pt using frame 14's rect;
//   +0x3d = 2 (Earth); invalidate().   (no reset call; +0x41 = 0 Diameter)
// 0x450117 load ctor: load; vtable; init; invalidate().

// slot 0x2c has_number, 0x45054c: return true
// reset/start: base (0x41cef4 / 0x41e50a)

// slot 0x44 tick, 0x450553  (every tick; never fires a changed flag)
rec = &PLANETS[+0x3d];
switch (+0x41) {
  case 0: number = rec->diameter;  break;  // "Diameter (miles)"           +0x24
  case 1: number = rec->day;       break;  // "Day (earth hours)"          +0x04
  case 2: number = rec->year;      break;  // "Year (earth days)"          +0x0c
  case 3: number = rec->distance;  break;  // "Distance from sun (million mi.)" +0x14
  case 4: number = rec->mass;      break;  // "Mass (x 10,000,000,000,000 Gigatons)" +0x3c
  case 5: number = rec->density;   break;  // "Density (water = 1)"        +0x2c
  case 6: number = rec->gravity;   break;  // "Gravity (Earth = 1)"        +0x34
  case 7: number = rec->orbit_vel; break;  // "Orbital Velocity (x1000 mph)" +0x1c
  default: number = 0.0;
}

// slot 0x04 draw, 0x4503f6 → 0x450152(this, dc, clip, pos)
if (!intersects) return;
draw_frame(sheet, 14, pos);
r = frame_rect(sheet, +0x3d + 1);
draw_frame(sheet, +0x3d + 1,
           (v = pos.v + 7 + (88 - (r.bottom - r.top)) / 2,
            h = pos.h + 7 + (94 - (r.right - r.left)) / 2));     // C division
SetTextColor(RGB(0,255,0));               // CDC vtable slot 0x38, 0x00ff00 (green)
SetBkMode(TRANSPARENT);
SelectObject(font 0x4a2715);              // "Small Fonts", height 11, PROOF_QUALITY (0x41783b)
SetTextAlign(TA_BOTTOM);                  // 8
name = PLANETS[+0x3d].name;               // upper case, e.g. "EARTH"
cx = GetTextExtentPoint(name).cx;
TextOut(x = pos.h + 9 + (88 - cx) / 2,    // note: 88, not 94 (faithful quirk)
        y = pos.v + 107, name);
// restore align, font, bk mode, text colour
```

There is no slot 0x48 override: the base fill/read handle the two combos.
Destructor 0x4504e6.

### Dialog 2000 "Solar System"

| ctl id | control | field |
|---|---|---|
| 107 combo "Planet:" | combo 0 | `+0x3d`: Mercury, Venus, Earth, Mars, Jupiter, Saturn, Uranus, Neptune, Pluto |
| 109 combo "Output:" | combo 1 | `+0x41`: the eight outputs above, in that order |
| 102 `Pic` | | live preview |

Strings: WIDGET 1022/2000.

---

## cAreaVolume (`ArVo`)

| | |
|---|---|
| tag | `ArVo` (`0x6f567241`) |
| class / vtable | cAreaVolume, `0x4991cc` |
| size | 0x198 |
| `+0x28` | **2003** (dialog 2003, Doc 1020/2003) |
| sheet | WWGADGET 1004/**1025** (`DAT_004a30fc`, refcount `DAT_004a30f8`) |
| points | 1010/1025 → `DAT_004a3100`: one point `(v 15, h 55)` (the shape icon) |
| knots | 1014/**1024** |
| sound | 1006/1020 when a calculation starts |
| bounds | frame 1 `(0,0,55,103)` |
| `+0x13d` | 1 (see corrections: start resets the input cache to "none") |

Knots: `#0 (v 27, h 4)` in: the length; `#1 (v 27, h 100)` out: the result.

Frames (sheet 1025): 1–8 the body's crunching animation (103×55 each;
frame 1 at rest), 9 square, 10 circle, 11 cube, 12 sphere (25×26), 13 an
unused dark icon, 14 1×1.

### State

| offset | type | meaning |
|---|---|---|
| +0x141 | u8 | animation phase 0..7 (base field) |
| +0x140 | u8 | "broken" (base field): input was negative |
| +0x197 | u8 | the input wire's changed flag, captured when the calculation started |
| +0x156 | f64 | cached input value (base knot cache) |

Settings: `+0x39` radio 0 (0 area of square, 1 area of circle, 2 volume of
cube, 3 volume of sphere).

```c
// 0x441a8c init: sheet 1025, points 1010/1025; tag 'ArVo'; bounds = frame 1;
//   +0x2a = 1; +0x28 = 2003; +0x13d = 1.
// 0x441b53 fresh ctor: centre on pt; load_knots(1024); reset(); +0x39 = 3 (sphere); invalidate().
// 0x441cd2 load ctor: load; init; load_knots(1024); reset(); invalidate().
// slot 0x2c has_number, 0x441d29: return true
// reset/start: base

// slot 0x44 tick, 0x441d30
if (+0x141 == 0) {
    a = value_changed(0);
    +0x197 = changed(0);
    if (a || +0x197) { +0x141 = 1; invalidate(); play(1020); }
} else if (throttle(9)) {
    +0x141++;
    if (+0x141 == 8) {
        x = input(0);
        +0x141 = 0;
        +0x140 = (x < 0.0);
        switch (+0x39) {
          case 0: number = x * x; break;
          case 1: number = x * x * 3.141592653589; break;
          case 2: number = x * x * x; break;
          case 3: number = x * x * x * 4.188790204785333; break;   // 4π/3
        }
        if (+0x140) number = 0.0;
        if (+0x197) fire(1);
    }
    invalidate();
}

// slot 0x04 draw, 0x441ecc
if (!intersects) return;
draw_frame(sheet, +0x141 + 1, pos);
draw_frame(sheet, +0x39 + 9, (pos.v + 15, pos.h + 55));
if (+0x140) draw_broken();        // slot 0x10
```

So a new input value starts a 7-step animation (≥ 10 ticks per step, ~1.2
s); the result appears at its end, computed from the input at that moment.
Because start sets the knot cache to the "none" sentinel, a wired input
always triggers one calculation at the start of a run. The output wire is
only flagged changed when the trigger included the wire's changed flag
(not when it was only a value difference).

### Dialog 2003 "Area / Volume"

| ctl id | control | field |
|---|---|---|
| 106 "Area of Square (input length of side)" | radio group 0 (key `1`), choice 0 | `+0x39` = 0 |
| 107 "Area of Circle (input length of radius)" | choice 1 | 1 |
| 108 "Volume of a Cube (input length of side)" | choice 2 | 2 |
| 109 "Volume of a Sphere (input length of radius)" | choice 3 | 3 |

(On screen the circle button sits below the cube button; the value order
is by control id.)

---

## Saved data

None of the five classes overrides slot 0x3c (size) or 0x40 (save): their
chunks are exactly the 0x150-byte base (id, position, the 0x108-byte
settings block, selected flag). Everything listed under "State" is
runtime-only and rebuilt by reset/start. None of the shipped
`FUNWIDGE/*.WGT` files contains any of these five parts (the `Pend`,
`Drop`, `Cano` hits in SHOTMOON/DOGYEAR are text inside other chunks), so
there is no sample to test a loader against.

## Proposed names

| address | proposed name |
|---|---|
| 0x44e944 | cPendulum::Init |
| 0x44ea05 | cPendulum::cPendulum(Point) |
| 0x44eb8e | cPendulum::cPendulum(chunk) |
| 0x44ebe5 | cPendulum::HasBoolOutput |
| 0x44ebf6 | cPendulum::HasNumberOutput |
| 0x44ec1a | cPendulum::Reset |
| 0x44ec4b | cPendulum::Draw |
| 0x44f05b | cPendulum::~cPendulum |
| 0x44f0d7 | SquareWaveSign (sign of sin via modf) |
| 0x44f137 | cPendulum::Start |
| 0x44f1b7 | cPendulum::Tick |
| 0x44f393 | cPendulum::FillDialog |
| 0x446614 | cDropChamber::Init |
| 0x4466e7 | cDropChamber::cDropChamber(Point) |
| 0x44687d | cDropChamber::cDropChamber(chunk) |
| 0x4468d4 | cDropChamber::HasBoolOutput |
| 0x4468db | cDropChamber::HasNumberOutput |
| 0x4468e2 | cDropChamber::Reset |
| 0x44690d | cDropChamber::Start |
| 0x446966 | cDropChamber::Draw |
| 0x446d59 | cDropChamber::~cDropChamber |
| 0x446de0 | cDropChamber::Tick (0x446e18..0x446f00 are its switch cases) |
| 0x44706c | cDropChamber::FillDialog |
| 0x44f498 | cPhysicsCanon::Init |
| 0x44f520 | cPhysicsCanon::cPhysicsCanon(Point) |
| 0x44f6de | cPhysicsCanon::cPhysicsCanon(chunk) |
| 0x44f765 | cPhysicsCanon::Reset |
| 0x44f790 | cPhysicsCanon::~cPhysicsCanon |
| 0x44f801 | cPhysicsCanon::HasBoolOutput |
| 0x44f812 | cPhysicsCanon::HasNumberOutput |
| 0x44f836 | cPhysicsCanon::Draw |
| 0x44fa0d | cPhysicsCanon::Start |
| 0x44faf1 | cPhysicsCanon::Tick |
| 0x44fe21 | cPhysicsCanon::FillDialog |
| 0x44feec | cPlanet::Init |
| 0x44ffa0 | cPlanet::cPlanet(Point) |
| 0x450117 | cPlanet::cPlanet(chunk) |
| 0x450152 | cPlanet::DrawAt |
| 0x4503f6 | cPlanet::Draw |
| 0x4504e6 | cPlanet::~cPlanet |
| 0x45054c | cPlanet::HasNumberOutput |
| 0x450553 | cPlanet::Tick |
| 0x441a8c | cAreaVolume::Init |
| 0x441b53 | cAreaVolume::cAreaVolume(Point) |
| 0x441cd2 | cAreaVolume::cAreaVolume(chunk) |
| 0x441d29 | cAreaVolume::HasNumberOutput |
| 0x441d30 | cAreaVolume::Tick |
| 0x441ecc | cAreaVolume::Draw |
| 0x44208c | cAreaVolume::~cAreaVolume |
| 0x41e762 | cGadget::Throttle(ticks) |
| 0x41e62a | cGadget::InputValueChanged(knot) |
| 0x41e697 | cGadget::ClearInputChanged(knot) |
| 0x41e4c8 | cGadget::PrimeInputCache |
| 0x4110c3 | ParseDoubleClamped |
| 0x439e3d | DoubleToIntSaturating |
| 0x4402cc | PlaySoundId |
| 0x4407eb / 0x440804 | SoundAddRef / SoundRelease |
| 0x41141f | LoadPointList (type 1010) |
| 0x4114da | PointList::At |
| 0x41db6c | cGadget::LoadKnots |
| 0x422940 | PictureSheet::Load |
| 0x422a68 | PictureSheet::FrameRect |
| 0x422d32 | PictureSheet::DrawFrame |
| 0x41783b | CreateGadgetFonts |
| 0x419c1a | cEditField::cEditField (parses the format spec) |
| 0x41acb9 | cGadgetDialog::BuildControls (OnInitDialog body) |
| 0x4811e4 / 0x480c00 / 0x481228 / 0x480f54 / 0x480ec4 | sin / cos / sqrt / modf / _ftol |
| 0x479c0a | CPen::CPen(style, width, color) |
| 0x4790a9 / 0x478893 | CDC::SetTextAlign / CDC::SetBkMode |

## Corrections to gadget-model.md

1. **Slot 0x30 is not a nop**: the base `0x41db39` returns
   `fld qword [this+0x146]`, so `+0x146` (f64) is the default number
   output, just as `+0x13e` is the default bool. Slots 0x24/0x2c/0x34
   return 0 (`0x41db1d`, `0x41db32`, `0x41db47`).
2. **Slot 0x20 (start) only sets the knots to "none" when `+0x13d` is
   set** (cAreaVolume sets it). Then it writes the sentinel into each
   `+0x156+8k` and clears each knot's wire changed flag (`0x41e697`).
   When `+0x13d` is 0 it calls `0x41e4c8` instead, which reads every knot
   (`0x41e56e`) and caches the current values, so nothing reads as changed.
3. `+0x13d` is "start with an empty input cache"; `+0x140` (u8) is the
   **broken** flag: the part's draw calls slot 0x10 when it's set, and
   base reset clears it (and invalidates). `+0x141` (u8) is zeroed by the
   base reset and used by subclasses as an animation phase (Drop,
   ArVo). `+0x152` is the last time `0x41e762` (throttle) passed.
4. The knot value cache `+0x156` is 8 doubles (to `+0x195`); `+0x196` is
   a u8 cleared by reset; subclass fields start at `+0x197`.
5. "4 number fields" (`block+0x08`) are **combo box selections**, filled
   from WIDGET type 1022 `<dialog id>`; radio groups are keyed by the last
   character of the button's title (see "Settings dialogs" above), and
   numbered by first appearance in control-id order, not by the key.
6. `+0x28` is the dialog/Doc/info id. It is not always the id of the
   part's pictures, points and knots (cPlanet: 2000 vs 1022; cAreaVolume:
   2003 vs 1024/1025). Several 2000-range classes set it in the
   constructor rather than in an init function (Addr/Subr/Mulr/Divr,
   Spl2/3/4, the stickers: all stickers are 2034).
7. `0x41e56e` returns 0.0 for an unwired knot (and 1.0/0.0 for an object
   output), so "input != 0" tests are safe on unwired knots.
8. Not gadget-model.md but related: `crates/formats/src/tables.rs` says
   type 1021 is read by `0x41141f`; `0x41141f` actually loads a **type
   1010 point list** (`0x3f2`) into a `{u16 count; Point*}` struct
   (`0x4114da` indexes it). Type 1021 is read directly by the dialog code
   (`0x41acb9`, `FUN_0041082a(.., 0x3fd, ..)` for the Doc button).

## Open questions

- Whether the run start always calls reset (slot 0x1c) before start
  (slot 0x20). The Drop chamber's phase, the Pendulum's release time and
  the Cannon's fire time are cleared only by reset; if a run can start
  without a reset, those parts would resume from where they stopped.
  Check the caller of slot 0x20.
- Whether the bound control is really enabled/disabled with its radio
  button (`button+10` set by `0x417c7b`); the use site wasn't traced.
- cDropChamber's custom height uses raw `sscanf`: an empty text leaves
  `+0x197` at its old value. On a fresh gadget nothing in the constructors
  initialises `+0x197` (unless the allocator zeroes memory; not checked); in practice the dialog only
  lets digits in, but an empty field is possible.
- Sheet 1022 frames 10–13 and sheet 1025 frame 13 are unused by these
  classes (they may be used by the dialog previews or Doc pictures).
