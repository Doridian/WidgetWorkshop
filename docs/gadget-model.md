# The gadget model

How `WIDGET.EXE` (Win95 build, Borland C++ 4.x with MFC 3) represents a
widget: gadgets, their knots, the wires between them, and how values move.
Addresses are in `WIDGET.EXE`.

Widget Workshop was written on the Mac first. The Windows port kept the
Mac's data: saved widgets are big-endian Mac structs, rectangles are Mac
`Rect`s (`top, left, bottom, right`), points are Mac `Point`s (`v, h`),
and the clock counts Mac ticks (1/60 s). Expect byte swaps everywhere.

## Classes

The RTTI names survive in the binary. The base class is `cGadget`
(vtable `0x491e58`); every part is a subclass, created by the factory
`0x41f194` from its four-character tag. Each tag has two constructors: a
fresh one (dragged from the tray) and one that reads a saved chunk (the
second argument is the chunk's data).

| tag | class | fresh ctor | load ctor | size |
|-----|-------|-----------|-----------|------|
| `MsCl` | cMouseClick | 0x44a92d | 0x44aaee | 0x198 |
| `NotG` | cNotGate | 0x44c37f | 0x44c4eb | 0x197 |
| `Spl2` / `Spl3` / `Spl4` | cSplitter2/3/4 | 0x45429c / 0x454540 / 0x4547e4 | 0x454441 / 0x4546e5 / 0x454989 | 0x19a |
| `di10` | cDisplay10 | 0x445613 | 0x445790 | 0x197 |
| `Stk1`..`Stk9`, `St10` | cSticker1..10 | 0x455077 .. 0x454c3f | 0x4551ef .. 0x454db7 | 0x197 |
| `AndG` | cAndGate | 0x440fd7 | 0x44115f | 0x197 |
| `XorG` | cXorGate | 0x45ce1b | 0x45cfa3 | 0x197 |
| `rgbd` | cRGBDisplay | 0x450e9c | 0x451019 | 0x19b |
| `TxLb` | cTextLabel | 0x45b126 | 0x45b2d3 | 0x197 |
| `NmKP` | cNumericKeyPad | 0x44d1a3 | 0x44d30f | 0x1a5 |
| `Key_` | (keyboard key? see `cGraphicProjector`?) | 0x4488fa | 0x448a85 | 0x198 |
| `OrGa` | cOrGate | 0x44e50f | 0x44e697 | 0x197 |
| `bulb` | cLightBulb | 0x449093 | 0x449210 | 0x197 |
| `AnSd` | cAnimalSound | 0x4414e1 | 0x441679 | 0x1ad |
| `MsSd` | cMiscSound | 0x44a31d | 0x44a4ab | 0x1ad |
| `StCh` | cStripChart | 0x458ca6 | 0x458e5c | 0x2a9 |
| `rand` | cRandomNumberGenerator | 0x450792 | 0x45091b | 0x197 |
| `Pend` | cPendulum | 0x44ea05 | 0x44eb8e | 0x1b3 |
| `Tape` | cTapePlayer | 0x45a1a0 | 0x45a30d | 0x1a1 |
| `Wtch` | cStopWatch | 0x458220 | 0x45838c | 0x1a0 |
| `DClk` | cDigitalClock | 0x444ac5 | 0x444c49 | 0x199 |
| `Fork` | cTuningFork | 0x45c79b | 0x45cb34 | 0x1a1 |
| `Sldr` | cSlider | 0x4519a3 | 0x451b49 | 0x1a6 |
| `Drop` | cDropChamber | 0x4466e7 | 0x44687d | 0x1ab |
| `MiCo` | cMiniConstant | 0x449c4c | 0x449db8 | 0x197 |
| `DnPl` | cDynamitePlunger | 0x44722a | 0x4473a7 | 0x197 |
| `TxLn` | cTextLine | 0x45b7d6 | 0x45b9a5 | 0x199 |
| `Coin` | cCoinToss | 0x442223 | 0x44261a | 0x197 |
| `ArVo` | cAreaVolume | 0x441b53 | 0x441cd2 | 0x198 |
| `Cano` | cPhysicsCanon | 0x44f520 | 0x44f6de | 0x1b3 |
| `Comp` | cComparison | 0x4428c0 | 0x442a33 | 0x197 |
| `SdTr` | cSoundTransformer | 0x45301e | 0x45318b | 0x1aa |
| `Stop` | cStopper | 0x4576e0 | 0x45786b | 0x19c |
| `disp` | cDisplay | 0x445c9f | 0x445e1c | 0x197 |
| `GrPr` | cGraphicProjector | 0x447ea2 | 0x44801f | 0x198 |
| `Subr` | cSubtracter | 0x459718 | 0x459761 | 0x199 |
| `1ncr` | cOneNumberCruncher | 0x44dd1f | 0x44de97 | 0x198 |
| `Addr` | cAdder | 0x440e20 | 0x440e69 | 0x199 |
| `Plnt` | cPlanet | 0x44ffa0 | 0x450117 | 0x197 |
| `Metr` | cMetronome | 0x4495ac | 0x449722 | 0x1a3 |
| `Spkr` | cSpeaker | 0x4539dd | 0x453d05 | 0x197 |
| `Ttlr` | cTotaler | 0x45bee5 | 0x45c05d | 0x1bc |
| `Mulr` | cMultiplier | 0x44bc00 | 0x44bc49 | 0x199 |
| `Cntr` | cCounter | 0x4442e3 | 0x444460 | 0x197 |
| `Divr` | cDivider | 0x446504 | 0x44654d | 0x199 |
| `swit` | cSwitch | 0x45995a | 0x459ad7 | 0x197 |
| `Conv` | cConversion | 0x443a5b | 0x443bdd | 0x198 |
| `hart` | cHeart | 0x44092f | 0x440ab3 | 0x197 |
| `cnst` | cConstant | 0x443098 | 0x443214 | 0x19a |
| `MsMv` | cMouseMove | 0x44aea5 | 0x44b074 | 0x1a7 |
| `SdSw` | cSoundSwitch | 0x452889 | 0x452a06 | 0x197 |
| `TxBx` | cTextBox | 0x45a8d4 | 0x45ab9b | 0x197 |
| `Mplx` | cMultiplexer | 0x44b610 | 0x44b77d | 0x19b |

(64 tags; the factory's table is the authority. `cFirstThru`,
`cNoiseMaker`, `cGate`, `cSplitter`, `cSticker`, `cNumberCruncher` are
base classes. Some tags above were read off the factory's compares with
the bytes reversed; check against `0x41f194` when in doubt.)

## The gadget object (`cGadget`, packed, no alignment)

| offset | type | what |
|--------|------|------|
| +0x00 | ptr | vtable |
| +0x04 | u32 | tag (e.g. `0x74736e63` = `cnst`) |
| +0x08 | u32 | id (the saved chunk's first u32; wires refer to gadgets by it) |
| +0x0c | u8 | selected |
| +0x0e | ptr | knots: array of 10-byte knots |
| +0x12 | u16 | knot count |
| +0x14 | Point (i16 v, i16 h... see note) | position on the worksheet |
| +0x18 | Rect | bounds, relative to the position (usually frame 1's rect) |
| +0x20 | Rect | |
| +0x28 | u16 | the gadget's resource id (2004 for cConstant, 2008 for cAndGate) |
| +0x2a | u8 | |
| +0x2f | u32 | |
| +0x33 | u8 | |
| +0x34 | u8 | one text (240 chars) instead of four labels |
| +0x35 | 0x108 bytes | the settings block, exactly as saved (below) |
| +0x13d | u8 | |
| +0x13e | u8 | boolean state (what `GetBool` returns by default) |
| +0x13f | u8 | |
| +0x141 | u8 | |
| +0x142 | i32/... | class state (cConstant: its value 0..999) |
| +0x14e | u32 | what slot 0x38 returns by default |
| +0x152 | u32 | |
| +0x156 | f64 × knots | the knots' last values; `0xADDC4A11FF61B241` means none yet |

Note on +0x14: the code adds `[+0x14]` to a rect's `left`/`right` and
`[+0x16]` to `top`/`bottom` in places and the other way round in others;
check per use. On the worksheet it is the gadget's top left.

### The settings block (+0x35, 0x108 bytes, saved as is)

Filled from and into the gadget's settings dialog (`0x41ea4d` fills,
`0x41e851` reads back):

| offset in block | type | what |
|-----|------|------|
| 0x00 | u32 | 32 check boxes as bits (`0x41c4c7` per box) |
| 0x04 | u8 × 4 | 4 radio groups' choices (`0x41c344`) |
| 0x08 | u32 × 4 | 4 number fields (`0x41c976`) |
| 0x18 | char × 60 × 4 | 4 text fields (`0x41c7d1`); or, when +0x34 is set, one 240-char text |

cConstant reads its value from the first text field with `%d`
(`0x442dc4`).

## A knot (10 bytes)

`0x43d068` builds them from resource type 1014 (`0x3f6`) of the gadget's
id: `+0` the wire (a `cConnection*`, null if unwired), `+4` Point, `+8`
u16 kind (0 in, 1 out). **A knot takes one wire.** That's why the
Splitters exist.

## Saved form (the chunk)

The base part is 0x150 bytes (slot 0x3c returns the size; slot 0x40
writes it, `0x41d396`; the load constructor `0x41d166` reads it):

| offset | what |
|--------|------|
| 0x000 | u32 id |
| 0x004 | i16, i16: position (swapped into +0x14/+0x16) |
| 0x008 | the 0x108-byte settings block (converted by `0x41ef34`, swapping the numbers) |
| 0x110 | u8: selected? (+0x0c) |
| 0x111.. | (pad to 0x150) |

Subclasses append their own state after 0x150.

## Wires (`cConnection`, 0x2b bytes)

| offset | what |
|--------|------|
| +0x00 | `cGadget*` the gadget with the output |
| +0x04 | `cGadget*` the gadget with the input |
| +0x08 | u16 the output's knot |
| +0x0a | u16 the input's knot |
| +0x0c | u16 |
| +0x0e | u16 |
| +0x11 | list of bend points |
| +0x25 | u8 flag (saved from format 3 on) |
| +0x2a | u8 **changed**: set by the sender, cleared by the reader |

## The vtable

| slot | base | what |
|------|------|------|
| 0x00 | 0x41d2bd | destructor |
| 0x04 | pure | draw(dc, clip) |
| 0x08 | 0x41d4b5 | draw the selection frame |
| 0x0c | 0x41e359 | invalidate (add the bounds to the update list) |
| 0x10 | 0x41ed9d | draw the "broken" picture (sheet `DAT_004a2694`) centered |
| 0x14 | nop | click while running (point) |
| 0x18 | 0x41dac5 | move to (point) |
| 0x1c | 0x41cef4 | reset: clear state, +0x13f = 1 |
| 0x20 | 0x41e50a | start: every knot's value to "none" and its wire's changed flag cleared |
| 0x24 | false | has a boolean output |
| 0x28 | +0x13e | the boolean output |
| 0x2c | false | has a number output |
| 0x30 | nop | the number output (in ST0) |
| 0x34 | false | has an object output (sound, picture) |
| 0x38 | +0x14e | the object output |
| 0x3c | 0x150 | saved size |
| 0x40 | 0x41d396 | save |
| 0x44 | nop | **tick** (every idle loop while running) |
| 0x48 | 0x41ea4d | fill the settings dialog |
| 0x4c | 0x41ea2e | read the settings dialog back |
| 0x50 | nop | settings changed |

## How values move

Pull, not push. Each tick (`0x42498f`, from the app's idle loop through
`0x424778`, only in run mode: `DAT_00493dde == 0`) calls slot 0x44 on
every gadget in list order.

- `0x41e56e(gadget, knot)` reads input `knot`: through its wire to the
  source gadget, it asks the source's vtable: object (0x34/0x38) first,
  then number (0x2c/0x30), then boolean (0x24/0x28, as 1.0/0.0). The
  result comes back in ST0.
- `0x41e713(gadget, knot)` is "did input `knot` change": the wire's
  changed flag, cleared as it's read.
- `0x41e6cf(gadget, knot)` is "output `knot` changed": sets its wire's
  flag.

Example, cAndGate's tick (`0x4411aa`):

```c
a = input(0); b = input(1);
this->bool_state = (a != 0 && b != 0);
if (changed(0) | changed(1)) output_changed(2);
```

## Time

`0x4113a8` is `timeGetTime() * DAT_004113cc` truncated: the clock in Mac
ticks. The tick loop also keeps `+0x4b` (time since the first tick) and
`+0x4f` (the first tick's time) on the document.
