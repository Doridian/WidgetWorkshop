# Input and display parts

This file covers the parts a person clicks, presses or reads: `swit`, `Sldr`, `NmKP`,
`Key_`, `MsCl`, `MsMv`, `DnPl`, `hart`, `bulb`, `disp`, `di10`, `rgbd`,
`TxLn`, `TxBx`, `TxLb`, `StCh`, `GrPr`, and the stickers `Stk1`..`Stk9`/`St10`.
It describes them as `WIDGET.EXE` (Win95) implements them. All addresses are in
`WIDGET.EXE`.

Read [`../gadget-model.md`](../gadget-model.md) first, then §1 of
[`logic.md`](logic.md). That section defines the shared notation
(`input(k)`, `value_changed(k)`, `fire(k)`, `changed(k)`, `play(id)`,
`invalidate()`, `draw_broken()`, `draw_frame(sheet, n, pt)`, `P[i]`,
`ftol_clamped`, `trunc`, `floor`) and the shared mechanics: reading
inputs, the knot value cache, reset/start, the dialog-to-settings
mapping, `format_number`, and drawing. They are not repeated here. This
file adds the input-routing path (§1) and the helpers only these parts use.

Points are Mac `(v, h)`: `v` is down, `h` is across. `pos` is the gadget's
top left (`+0x14` v, `+0x16` h). Every `draw_frame` puts the frame's own
rect, offset by the given point. All frames here have rect top-left
`(0,0)` unless noted, so the point is the frame's top-left.

**Saved data, all parts in this file.** No class here overrides slot 0x3c
(size, 0x150). Only `TxBx` overrides slot 0x40 (save), and only to
transform its text temporarily (§15). Every load constructor reads only
the 0x150-byte base (`0x41d166`). Every part's persistent configuration
is the settings block. All other state is runtime only and is lost on
save/load. This was checked against every use in `orig/WIN95/WIDGET/FUNWIDGE/*.WGT`:
133 `TxLb`, 31 `bulb`, 17 `St10`, 14 `Key_`, 12 `swit`, 11 `GrPr`, 8
`TxLn`, 6 `disp`, 5 `StCh`, 2 `TxBx`, 1 each of `NmKP`, `DnPl`, `Stk1`,
`Stk4`, `Stk6`, `Stk7`. All of them are exactly 0x150 bytes, with settings
consistent with the field meanings below. Many unused text fields in the
saved files contain garbage (`"@\x81"`, uninitialised Mac memory), so do
not assume they are zero.

---

## 1. Shared mechanics specific to these parts

### 1.1 How mouse clicks reach a gadget (run mode)

The keyboard never reaches gadgets. Every part that reacts to keys or to the
mouse button state polls Windows itself (§1.2). The only push path is
"mouse button pressed on a gadget while running":

```
view WM_LBUTTONDOWN-type handler   0x4343f2(view, nFlags, CPoint pt)
    -> worksheet.slot4(ws, Point{v=pt.y, h=pt.x}, flags=0, dbl=0)
view double-click handler          0x4344d2(view, nFlags, CPoint pt)
    -> worksheet.slot4(ws, Point{v=pt.y, h=pt.x}, flags=0, dbl=1)

worksheet = DAT_004a2848 (vtable 0x49326c; RTTI name probably cWorksheetPane)
worksheet slot 4 = 0x423756 mouse_down(ws, pt, flags, dbl):
    if run_mode (DAT_00493dde == 0):  0x423e3f(ws, pt, flags, dbl)
    elif edit mode (== 1):            capture + clip cursor, 0x423dc4 (editing; not covered here)

0x423e3f run_click(ws, pt, flags, dbl):
    if tool_mode(DAT_004a2864) == 2:  0x426313(ws, pt)           // a tool mode (probably the Doc/help cursor); no gadget click
    else:
        node = 0x42391d(ws, pt)       // first gadget in the worksheet list (ws+0x10), in list order, whose slot 0x18 returns true
        if node: gadget.slot14(gadget, pt, flags, dbl)
```

* The point is in worksheet coordinates, which are the view's client
  coordinates (`0x425e44` converts screen→worksheet with plain
  `ScreenToClient`).
* Slot 0x18 (`0x41dac5`) is a **hit test**, not "move to". It returns
  `PtInRect(bounds(+0x18), (pt.v - pos.v, pt.h - pos.h))`. `PtInRect` is
  `0x411e03`, Mac semantics: `top <= v < bottom && left <= h < right`.
* A double-click reaches slot 0x14 a second time with `dbl = 1`. No part
  here looks at `flags` or `dbl`, so a double-click acts as two clicks.
  For example, it toggles a switch twice. (The view handlers were not
  found in an MFC message map, so their mapping to `WM_LBUTTONDOWN` /
  `WM_LBUTTONDBLCLK` is inferred from their shape. See open questions.)
* Key presses go to worksheet slot 8 (`0x42385d`, via `0x434451`). In run
  mode that returns `0x423dbd() = 0`, so gadgets get nothing. The key then
  goes to `0x430f63`, where Return = GO, and to the tool palette.

### 1.2 Polled input helpers

| name | address | what |
|---|---|---|
| `mouse_down()` | `0x415208` | `true` if the left **or right** button is down. If `DAT_00490924 == 0`: `GetAsyncKeyState(VK_LBUTTON/VK_RBUTTON) < 0`. Else: first `PeekMessage(0x200..0x209, PM_REMOVE)`, `Translate`+`Dispatch` one mouse message, then `GetKeyState(1/2) < 0`. |
| `mouse_down2()` | `0x415284` | Identical code to `0x415208` (a duplicate). Used by `MsCl`. |
| `key_down(vk)` | `0x41507c(kbd, vk)` | `GetKeyState(vk) < 0`. `kbd` is `DAT_004a26f8` (RTTI `cKeyboard`) and is unused. |
| `toupper(c)` | `0x480bc4` | Borland `toupper`: `c - 0x20` if ctype(c) has the lower-case bit (`DAT_004a1751[c] & 8`). |
| `is_digit(c)` | `DAT_004a1751[c] & 2` | Borland ctype. `& 0xe` = alphanumeric (digit, upper or lower). |
| `cursor_screen()` | `0x4153ac` | `GetCursorPos`, returned as `Point{v=y, h=x}` in **screen** pixels. |
| `cursor_ws()` | `0x4264b6(&pt, ws)` | `GetCursorPos` + `ScreenToClient(view)`, as a `Point` in worksheet coordinates. |
| `parse_number(s)` | `0x4110c3` | `v = 0.0; sscanf(s, "%lf", &v)`. If `v == 0` return 0. If `v > 0`, clamp to `[1e-300, 1e300]`. If `v < 0`, clamp to `[-1e300, -1e-300]`. |
| `now()` | `0x424a89(DAT_004a2848)` | `doc.+0x4b`, Mac ticks (1/60 s) since the run's first tick (u32). |
| `run_mode` | `DAT_00493dde == 0` | 0 = running, 1 = editing. |
| `lights_on` | `DAT_00493ddc` | Toggled by toolbar command `0x5df` (sounds `0x8c`/`0x8d`). Default 1. It selects light/dark backgrounds (dialog picture background is `WHITE_BRUSH` when 1, `BLACK_BRUSH` when 0). Used here only by `TxLb`. |

Mouse buttons and keys are read through `GetKeyState` /
`GetAsyncKeyState`, so a press only counts if it is still down when a tick
looks at it. Nothing is queued.

### 1.3 Text drawing

The view object passed to `draw` holds an MFC `CDC` at `view+0x438`, whose
`HDC` is at `view+0x43c`. The text calls are:

| call | meaning |
|---|---|
| `cdc.vt[0x30](font)` | `SelectObject(font)`, which returns the old font |
| `cdc.vt[0x38](colorref)` | `SetTextColor`, which returns the old colour |
| `0x478893(cdc, 1)` | `SetBkMode(TRANSPARENT)` |
| `0x4790a9(cdc, 0)` | `SetTextAlign(TA_LEFT\|TA_TOP)` |
| `cdc.vt[0x70](str, len, &RECT, fmt)` | `DrawText` |

Everything is restored afterwards. The fonts are created once by
`0x41783b`. The arguments are `CreateFontA(h, 0, 0, 0, weight, 0, 0, 0,
charset 0, 0, 0, quality 2 = PROOF_QUALITY, pitch/family 0, face)`. A
positive `h` is the cell height in pixels.

| getter | object | face | height | weight |
|---|---|---|---|---|
| `0x417915` | `0x4a2715` | "Small Fonts" | 11 | normal |
| `0x417931` | `0x4a2725` | "Helvetica" | 16 | normal |
| `0x41794d` | `0x4a271d` | "Helvetica" | 16 | 700 (bold) |
| `0x417969` | `0x4a272d` | "Helvetica" | 24 | normal |

DrawText format values used: `0x825` = `DT_CENTER|DT_VCENTER|DT_SINGLELINE|DT_NOCLIP`.
`0x811` = `DT_CENTER|DT_WORDBREAK|DT_NOCLIP`.

### 1.4 Picture sheets and per-class caches

Every class loads its sheet (WWGADGET type 1004) and point/rect lists
once, when its first instance is constructed, using a class instance
counter. It frees them when the last instance is destroyed. Point lists
are type 1010, loaded by `0x41141f`. Rect lists are type 1012, loaded by
`0x411f9a` and indexed by `0x412032(&table, i)`. Knots are type 1014,
loaded by `0x41db6c`. **The resource number used for sheet, points and
knots is not the gadget id** (`+0x28`). It is a per-class number, given in
the table below as "res".

### 1.5 Settings-dialog "Clear"/"Reset" buttons

`+0x2f` (code pointer) and `+0x33` (u8) name a dialog push button and the
callback it runs: fill (`0x41ea4d`) calls `0x41ca72(dlg, +0x33, +0x2f)`.
The push buttons are numbered by control id, with ids 1 (OK) and 2
(Cancel) first, so index 2 is the dialog's extra push button. The callback
gets the dialog and sets edit fields with `0x41c728(dlg, n, text)`.

### 1.6 The dialog's live preview (`+0x2b`)

The base init `0x41cf8a` sets `+0x2b = 0x41eb78`, and the dialog stores it
(`0x41baac(dlg, gadget, hook)`). As logic.md §1.6 says, the dialog calls
it on control changes. What it does:

```
0x41eb78 preview(dlg, gadget):
    saved = copy of gadget settings block (0x108 bytes)
    0x41e851(dlg, gadget)        // read the dialog into the block
    gadget.slot50()              // re-derive state from the settings
    0x41c660(dlg)                // repaint the dialog's live "Pic" control (0x418e92) if the dialog has one
    gadget settings block = saved
```

The block is restored, but whatever slot 0x50 derived is **not** restored.
For example, a switch's on/off state follows the dialog's radio buttons
live. OK then reads the dialog for real (slot 0x4c). Cancel calls slot
0x50 again with the restored block, which reverts the derived state.
`MsMv` sets `+0x2b = 0`, which disables the hook.

In a dialog template, owner-draw button 102 with caption `"Pic"` shows the
live gadget (`0x418cd0`). With caption `"Pic<n>"` it shows frame 1 of
WWGADGET sheet `n` (for example `"Pic4015"`). Button 103 `"Doc"` shows
Doc's text.

---

## 2. Summary table

Constructor size, vtable and "res" (sheet/points/knots number) per class.
`+0x28` is the gadget id (WWGADGET type 1021 info `(id, kind, dialog, doc)`).
The dialog is WIDGET RT_DIALOG `id`. Doc text is WWGADGET type 1020 `id+2000`.

| tag | class | fresh / load ctor | size | vtable | `+0x28` id | kind | res (sheet, knots) | `+0x13d` |
|---|---|---|---|---|---|---|---|---|
| `swit` | cSwitch | 0x45995a / 0x459ad7 | 0x197 | 0x49bef6 | 1012 | 501 | 1012 | 0 |
| `Sldr` | cSlider | 0x4519a3 / 0x451b49 | 0x1a6 | 0x49b120 | 1015 | 501 | 1015 | 0 |
| `NmKP` | cNumericKeyPad | 0x44d1a3 / 0x44d30f | 0x1a5 | 0x49a3a8 | 2007 | 501 | 1027 | 0 |
| `Key_` | cKey ("Keypress") | 0x4488fa / 0x448a85 | 0x198 | 0x499c2c | 1010 | 501 | 1010 | 0 |
| `MsCl` | cMouseClick ("Mouse Button") | 0x44a92d / 0x44aaee | 0x198 | 0x499f4e | 2032 | 503 | 1040 | 0 |
| `MsMv` | cMouseMove ("Mouse Distance Measurer") | 0x44aea5 / 0x44b074 | 0x1a7 | 0x499fec | 2029 | 503 | 1039 | 0 |
| `DnPl` | cDynamitePlunger | 0x44722a / 0x4473a7 | 0x197 | 0x499972 | 2030 | 503 | 1048 | 0 |
| `hart` | cHeart | 0x44092f / 0x440ab3 | 0x197 | 0x498f8e | 1017 | 501 | 1017 | 0 |
| `bulb` | cLightBulb | 0x449093 / 0x449210 | 0x197 | 0x499cbe | 1016 | 502 | 1016 | 0 |
| `disp` | cDisplay ("Three-Digit Display") | 0x445c9f / 0x445e1c | 0x197 | 0x4997b2 | 2002 | 502 | 1013 (+ digits sheet 100) | 1 |
| `di10` | cDisplay10 ("Ten-Digit Display") | 0x445613 / 0x445790 | 0x197 | 0x4996d2 | 1014 | 502 | 1014 (+ digits sheet 100) | 1 |
| `rgbd` | cRGBDisplay ("Color Mixer") | 0x450e9c / 0x451019 | 0x19b | 0x49b06c | 1018 | 502 | 1018 | 1 |
| `TxLn` | cTextLine ("Message List") | 0x45b7d6 / 0x45b9a5 | 0x199 | 0x49c2e0 | 2028 | 500 | 1046 | 0 |
| `TxBx` | cTextBox ("Billboard") | 0x45a8d4 / 0x45ab9b | 0x197 | 0x49c18c | 2027 | 500 | 1047 | 0 |
| `TxLb` | cTextLabel ("Label") | 0x45b126 / 0x45b2d3 | 0x197 | 0x49c218 | 2045 | 500 | none (no sheet, no knots) | 0 |
| `StCh` | cStripChart | 0x458ca6 / 0x458e5c | 0x2a9 | 0x49be04 | 2044 | 500 | 1049 | 0 |
| `GrPr` | cGraphicProjector | 0x447ea2 / 0x44801f | 0x198 | 0x499b74 | 2035 | 500 | 1050 (+ picture sheets 200..205) | 0 |
| `Stk1`..`Stk9` | cSticker1..9 | see §19 | 0x197 | see §19 | 2034 | 502 | 1052..1060 | 0 |
| `St10` | cSticker10 | 0x454c3f / 0x454db7 | 0x197 | 0x49b634 | 2034 | 502 | 1061 | 0 |

`Key_` is **cKey**. The RTTI string `cKey` is the only class name in the
binary not otherwise assigned. The dialog title is "Keypress", gadget id
1010, Doc text 3010: "Set the Keypress to any letter or number…".
`+0x2a` is 1 for every gadget (the base init sets it). Some inits set it
again, which changes nothing.

Knots (type 1014 of "res", positions relative to `pos`):

| tag | knots |
|---|---|
| `swit` | 0 out (20,33): bool |
| `Sldr` | 0 out (53,87): number |
| `NmKP` | 0 out (15,90): number |
| `Key_` | 0 out (20,43): bool |
| `MsCl` | 0 out (5,51): bool |
| `MsMv` | 0 out (5,51): number (inches) |
| `DnPl` | 0 out (55,55): bool |
| `hart` | 0 out (10,37): bool or number (settings) |
| `bulb` | 0 in (55,20) |
| `disp` | 0 in (21,4) |
| `di10` | 0 in (21,4) |
| `rgbd` | 0 in (14,3) red, 1 in (41,3) green, 2 in (67,3) blue |
| `TxLn` | 0 in (11,4): message number |
| `TxBx` | 0 in (42,4): show |
| `TxLb` | none |
| `StCh` | 0 in (96,4): value; 1 in (4,17): reset |
| `GrPr` | 0 in (87,4): picture number |
| stickers | none |

Overridden vtable slots (all others are cGadget's):

| tag | 0x00 dtor | 0x04 draw | 0x14 click | 0x1c reset | 0x20 start | 0x24 has_bool | 0x2c has_num | 0x40 save | 0x44 tick | 0x48 fill dlg | 0x4c read dlg | 0x50 settings |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| swit | 459cd8 | 459b22 | 459c3d | 459cb5 | 459c8e | 459d3e | | | | | | 459880 |
| Sldr | 452043 | 451bc4 | 4520bb | 45202a | 452459 | | 4520b4 | | 4524ad | 45262d | | 4516c0 (nop) |
| NmKP | 44d36a | 44d53e | 44d86b | 44d766 | 44dae5 | | 44d797 | | 44db2d | | | 44cfb8 |
| Key_ | 448ee2 | 448c57 | 448dc6 | | 448e04 | 448f53 | | | 448e2b | | | |
| MsCl | 44acb2 | 44ab97 | | 44ab7f | 44ad1f | 44ad18 | | | 44ad46 | | | |
| MsMv | 44b26f | 44b113 | 44b26a (nop) | 44b227 | | | 44b2d5 | | 44b2dc | | | |
| DnPl | 447550 | 4473f2 | 44750d | | | 4475c1 | | | | | | |
| hart | 440c19 | 440afe | | | | 440daa | 440dbc | | 440c8a | | | |
| bulb | 44925b | 4492c1 | | 44941e | | | | | 449436 | | | |
| disp | 445e67 | 445ef6 | | | | | | | 446410 | | | |
| di10 | 4457db | 44586a | | | | | | | 445ace | | | |
| rgbd | 45155d | 4511f2 | | 451114 | | | | | 45112d | | | |
| TxLn | 45bcf1 | 45bb9b | | | 45bd57 | | | | 45bd72 | | | |
| TxBx | 45ae1b | 45acd5 | | | | | | 45aed5 | 45ae81 | 45af0a | | |
| TxLb | 45b6b2 | 45b51e | | | | | | | | | 45b611 | |
| StCh | 459226 | 458ea7 | | | 4592f5 | | | | 459469 | 459615 | | |
| GrPr | 44806a | 448122 | | | 448692 | | | | 448728 | | | |
| stickers | per class | per class | | | | | | | 454b7a (nop) | | | |

Getters 0x28 (bool value = `+0x13e`), 0x30 (number value = `fld [+0x146]`),
0x34/0x38 (no object) are never overridden here. Note that base slot 0x30
`0x41db39` is `return +0x146 as f64`, not a no-op.

---
## 3. cSwitch `swit` ("Light Switch", id 1012, Doc 3012)

**Sheet 1012**: frame 1 = lever up "ON", frame 2 = lever down "OFF" (both
38×42), frame 3 = 1×1. `bounds` = frame **2**'s rect. There is no point
list.

**State**: `+0x13e` u8 on/off (the bool output).

**Settings** (dialog 1012 "Light Switch"):

| control | field | meaning |
|---|---|---|
| radio 107 "Off at Start" (tag `1`) | radio group 0 = 0 | |
| radio 108 "On at Start" (tag `1`) | radio group 0 = 1 | |

```
init 0x45989a (both ctors):
    load sheet 1012 (class cache DAT_004a33b4, count DAT_0049beb8)
    tag = 'swit'; bounds = frame2.rect; +0x28 = 1012
    on = (radio0 != 0)
fresh ctor 0x45995a: centre on drop point using frame 2; load knots 1012; invalidate
load ctor 0x459ad7:  base load; init; knots; invalidate

draw 0x459b22(view, clip):
    if bounds+pos ∩ clip empty: return
    draw_frame(1012, on ? 1 : 2, pos)

click 0x459c3d (slot 0x14, run mode only):
    on = !on
    if on: fire(0)            // only the off→on edge is marked; on→off is not
    invalidate()
    play(1012)                // WWSOUND 1006/1012

start 0x459c8e:   gadget_start(); if on: fire(0)
reset 0x459cb5:   gadget_reset(); on = (radio0 != 0)
settings 0x459880 (slot 0x50): on = (radio0 != 0)
has_bool 0x459d3e: true      // value = +0x13e
```

There is no tick. The state only changes on clicks, on Stop (reset),
through the dialog preview (§1.6), and on Cancel. The starting position
therefore takes effect when the dialog is changed and on every Stop.
Saved examples: `radios [1,…]` = On at Start (DOGYEAR, ESPTEST, HIMOM,
SHOTMOON, SINCOS) and `[0,…]` (OSCILLA).

---

## 4. cSlider `Sldr` ("Slider", id 1015, Doc 3015)

**Sheet 1015** (19 frames):

| frame | size | what |
|---|---|---|
| 1 | 9×15 | `.` |
| 2 | 9×15 | `-` |
| 3 | 9×15 | `+` |
| 4 | 9×15 | `E` |
| 5..14 | 9×15 | digits `0`..`9` (frame = char − `'+'`) |
| 15 | 15×14 | right (increase) arrow button, pressed |
| 16 | 175×59 | body (`bounds`) |
| 17 | 15×14 | left (decrease) arrow button, pressed |
| 18 | 18×43 | knob |
| 19 | 1×1 | |

**Points 1010/1015**: `P0=(3,29)` first digit, `P1=(0,12)` digit step,
`P2=(3,3)` left button, `P3=(3,157)` right button, `P4=(20,29)` track
start, `P5=(20,129)` track end.

Class rects, computed once at class init (`0x4516c5`), relative to `pos`
(`0x41198a` = `Pt2Rect`, min/max of two points):

```
btn = frame15.size = (v 14, h 15)
UP_RECT   (DAT_004a32ce) = Pt2Rect(P3, P3+btn) = {top 3, left 157, bottom 17, right 172}
DOWN_RECT (DAT_004a32d6) = Pt2Rect(P2, P2+btn) = {top 3, left 3,   bottom 17, right 18}
TRACK     (DAT_004a32de) = Pt2Rect(P4, P5)     = {top 20, left 29, bottom 20, right 129}
TRACK_W = TRACK.right - TRACK.left = 100   (DAT_004a32e4 - DAT_004a32e0, as i16)
```

**State**:

| offset | type | what |
|---|---|---|
| `+0x146` | f64 | value (number output) |
| `+0x196` | u8 | sticky decimals for `format_number` |
| `+0x197` | i16 | knob position, 0..TRACK_W. Set to 0 only by the constructors; not reset, not saved. |
| `+0x199` | u8 | tick counter (wraps) |
| `+0x19a` | u8 | dragging the knob |
| `+0x19b`, `+0x19d`, `+0x19f`, `+0x1a1` | i16 | zeroed by the constructors, otherwise unused |
| `+0x1a3` | u8 | an arrow button is shown pressed |
| `+0x1a4` | u8 | that button's frame (15 or 17) |
| `+0x1a5` | u8 | the point index for it (3 or 2) |

**Settings** (dialog 1015 "Slider"):

| control | field | meaning |
|---|---|---|
| radio 107 "Preset:" (tag `1`) | radio0 = 0 | use the combo |
| radio 109 "Custom:" (tag `1`) | radio0 = 1 | use the two edit fields |
| combo 108 (strings `0-10`, `0-100`) | num0 | preset range; fresh = 1 (`0-100`) |
| edit 111 "from" (filter `"5"`: max 5 chars) | text0 | custom low |
| edit 113 "to" (filter `"5"`) | text1 | custom high |

Fill `0x45262d` links combo 0 to radio button 0 and edits 0 and 1 to
radio button 1 (`0x417c7b(ctrl, radiobutton)`, presumably enabled only
while that button is checked). It then calls the base fill.

```
value_of(this) 0x45238c -> f64:          // computed in x87 extended, stored as f64
    t = (f64)(i16)knob / (f64)(i16)TRACK_W
    if radio0 != 0:
        lo = parse_number(text0); hi = parse_number(text1)
        if hi < lo: swap(lo, hi)
    else switch num0:
        0: lo = 0, hi = 10
        1: lo = 0, hi = 100
        else: lo, hi uninitialised stack (cannot happen from the dialog)
    return (hi - lo) * t + lo

draw 0x451bc4(view, clip):
    if bounds+pos ∩ clip empty: return
    draw_frame(1015, 16, pos)
    draw_frame(1015, 18, (pos.v + P4.v, pos.h + P4.h + knob))       // drawn in edit mode too
    if run_mode:
        s = format_number(+0x146, &+0x196)
        for i in 0..10:                                             // the first 10 chars only
            c = s[i]
            if c == ' ': continue
            f = '.'→1, '-'→2, '+'→3, 'E'/'e'→4, digit→c-'+' (5..14)
            otherwise: draw_broken(); return
            draw_frame(1015, f, (pos.v + P0.v + i*P1.v, pos.h + P0.h + i*P1.h))
        if +0x1a3: draw_frame(1015, +0x1a4, pos + P[+0x1a5])

click 0x4520bb(pt):                       // run mode only (§1.1)
    q = pt - pos
    if PtInRect(UP_RECT, q):
        if knob < TRACK_W: knob += 1; +0x1a3=1; +0x1a5=3; +0x1a4=15; invalidate()
    elif PtInRect(DOWN_RECT, q):
        if knob > 0:       knob -= 1; +0x1a3=1; +0x1a5=2; +0x1a4=17; invalidate()
    else:
        knob_rect = frame18.rect offset by P4 and by (0, knob)   // {20, 29+knob, 63, 47+knob}
        if PtInRect(knob_rect, q): dragging = 1        // (cursor_ws() is called; result unused)

tick 0x4524ad:
    counter += 1                                  // u8
    if !dragging:
        if counter & 8:                           // ticks 8..15 of every 16
            +0x146 = value_of(this)
            if +0x1a3 and !mouse_down():
                +0x1a3 = 0; fire(0); invalidate()
    else:
        if !mouse_down(): dragging = 0
        else:
            m = cursor_ws()
            x = (i16)(m.h - pos.h) - TRACK.left - 9       // centre of the 18-wide knob on the cursor
            x = clamp(x, 0, TRACK_W)                      // checks 0 first, then the maximum
            if knob != x:
                knob = x; +0x146 = value_of(this); fire(0); invalidate()

start 0x452459:  gadget_start(); +0x146 = value_of(this); if +0x146 != 0: fire(0); invalidate()
reset 0x45202a:  +0x196 = 0; +0x1a3 = 0           // does NOT call gadget_reset; the value, knob and bool are kept
has_number 0x4520b4: true                        // value = +0x146
settings 0x4516c0: nop
```

Faithfulness notes:

* An arrow click moves the knob at once, but the value only updates on a
  tick with `counter & 8`. `fire(0)` comes only when such a tick sees the
  mouse released.
* Dragging always fires on every change of position.
* The knob is not saved. A loaded slider starts at the left end (value =
  low end of the range).

---

## 5. cNumericKeyPad `NmKP` ("Numeric Keypad", id 2007, Doc 4007)

**Sheet 1027** (30 frames): 1..10 = LCD digits `0`..`9` (6×13), 11 = `.`,
12 = `-`, 13 = `E`, 14 = one more LCD glyph (unused), 15 = body 96×138
(`bounds`), 16..25 = buttons `0`..`9` pressed (20×18), 26 = `.` pressed,
27 = `—` (minus) pressed, 28 = `C` pressed, 29 = `ENTER` pressed (43×18),
30 = 1×1.

**Points 1010/1027**: `P0=(9,8)` the LCD's first character. Characters
step 7 px across.

**Rects 1012/1027** (button hit rects, relative to `pos`, `{top,left,bottom,right}`):

| idx | button | rect |
|---|---|---|
| 0 | `0` | (92,10,108,29) |
| 1 | `1` | (73,10,89,29) |
| 2 | `2` | (73,33,89,52) |
| 3 | `3` | (73,56,89,75) |
| 4 | `4` | (54,10,70,29) |
| 5 | `5` | (54,33,70,52) |
| 6 | `6` | (54,56,70,75) |
| 7 | `7` | (35,10,51,29) |
| 8 | `8` | (35,33,51,52) |
| 9 | `9` | (35,56,51,75) |
| 10 | `.` | (92,33,108,52) |
| 11 | `—` (sign) | (111,10,127,29) |
| 12 | `C` | (92,56,108,75) |
| 13 | `ENTER` | (111,33,127,75) |

**State**:

| offset | type | what |
|---|---|---|
| `+0x146` | f64 | value (number output) |
| `+0x197` | char[10] + NUL at `+0x1a1` | the LCD text, right-aligned, space-padded |
| `+0x1a2` | u8 | a button is shown pressed |
| `+0x1a3` | u8 | that button's pressed frame (16..29). 29 also means "ENTER was last". |
| `+0x1a4` | u8 | that button's rect index |

**Settings** (dialog 2007):

| control | field | meaning |
|---|---|---|
| edit 107 "Enter Beginning Number:" (filter `"10.-#"`: numeric, `.`, `-`, max 10) | text0 | starting LCD text |
| check 108 "Lock Number" | bit 0 | clicks are ignored |
| check 109 "Hide Number" | bit 1 | the LCD digits are not drawn |
| check 110 "Auto Reset" | bit 2 | on Stop (reset), the LCD is reloaded from text0 |

```
clear 0x44d726:  +0x197..+0x19f = ' ' ×9; +0x1a0 = '0'; +0x1a1 = 0       // "         0"

valid 0x44cff6(s) -> bool:            // the whole LCD must match
    skip ' '*; optional '-'; digit*; optional ('.' digit*);
    optional ( ('e'|'E') optional '-' digit [digit] );   // e needs ≥1 digit, max 2
    must be at the NUL

append 0x44d79e(c) -> rejected:bool
    saved = copy of the 11 bytes
    if lcd[0] == ' ':                                   // room left
        if lcd[9] != '0' or (lcd[8] != ' ' and lcd[8] != '-') or c == '.':
            shift lcd[1..10] left by one into lcd[0..9]  // otherwise a lone "0" is replaced
        lcd[9] = c
        if valid(lcd): return false
    lcd = saved; return true                            // full, or the result is invalid

enter 0x44da9d:
    p = first non-space of lcd
    sscanf(p, "%10lf", &+0x146)                        // unchanged if it fails
    fire(0)

draw 0x44d53e(view, clip):                             // NOT gated on run mode
    if bounds+pos ∩ clip empty: return
    draw_frame(1027, 15, pos)
    if !(bits & 2):
        for i in 0..10:
            c = lcd[i]
            f = '.'→11, '-'→12, 'E'/'e'→13, digit→c-'/' (1..10), ' '→skip
            any other char: f keeps its value from the previous character (uninitialised for i=0). It cannot happen, because valid() admits no other characters.
            draw_frame(1027, f, (pos.v + P0.v, pos.h + P0.h + 7*i))
    if +0x1a2: draw_frame(1027, +0x1a3, (pos.v + rect[+0x1a4].top, pos.h + rect[+0x1a4].left))

click 0x44d86b(pt):
    if bits & 1: return                                // Lock Number
    for i in 0..14:
        if !PtInRect(rect[i], pt - pos): continue
        +0x1a2 = 1; +0x1a4 = i; invalidate()
        switch i:
          10: +0x1a3 = 26; if append('.'): play(1027)
          11: +0x1a3 = 27
              if lcd[0] == ' ' or lcd[0] == '-':
                  k = number of leading ' '
                  if lcd[k] == '-': lcd[k] = ' '        // remove sign
                  else:            lcd[k-1] = '-'      // add sign (k >= 1 here)
              else: if append('!'): play(1027)         // full: '!' is always rejected → error beep
          12: +0x1a3 = 28; clear()                     // the value is unchanged
          13: +0x1a3 = 29; enter()
          0..9: if +0x1a3 == 29: clear()               // first digit after ENTER starts a new number
                +0x1a3 = 16 + i
                if append('0' + i): play(1027)          // WWSOUND 1006/1027 = reject beep

tick 0x44db2d:    if +0x1a2 and !mouse_down(): +0x1a2 = 0; invalidate()
start 0x44dae5:   gadget_start(); enter(); if +0x146 == 0.0: changed(0)   // takes the flag back: a zero start is not an event
                  +0x1a3 = 29                          // behave as if ENTER was last
reset 0x44d766:   gadget_reset(); +0x1a2 = 0; if bits & 4: slot50()      // gadget_reset zeroes +0x146
settings 0x44cfb8 (slot 0x50): clear(); for c in text0 until NUL: if append(c): break
has_number 0x44d797: true
```

* The load constructor calls slot 0x50 (LCD = text0). The fresh constructor
  only clears the LCD (text0 is empty).
* Sounds play only on rejection. A sign click never beeps unless the LCD is full.
* SLOTMACH.WGT example: `bits 0x4` (Auto Reset), text0 `"0"`.

---
## 6. cKey `Key_` ("Keypress", id 1010, Doc 3010)

**Sheet 1010**: frame 1 = key up (off), frame 2 = key pressed (on), both
49×40. Frame 3 = 1×1. `bounds` = frame 1. Points 1010/1010 (`(24,16)`,
`(25,16)`) are loaded but never used.

**State**: `+0x13e` u8 on (bool output). `+0x197` u8 "turned on by a
mouse click" (otherwise by the key).

**Settings** (dialog 1010 "Keypress"): edit 107 "Letter:" (filter `"A1"`:
max 1 char, `A` flag) → text0. Only `text0[0]` is used (`+0x4d`). A fresh
gadget gets `+0x4d = 'A'`, `+0x4e = 0`. Saved examples: `A R s g C O D 1
2 3 4 5 6 H`; lower case works, through `toupper`. `radio0` (`+0x39`)
picks the font. The dialog has no radio, so it is always 0 unless a saved
file says otherwise.

```
K = toupper(text0[0])          // also the Windows virtual-key code: right for A–Z, 0–9, space

draw 0x448c57(view, clip):     // NOT gated on run mode
    if bounds+pos ∩ clip empty: return
    draw_frame(1010, on ? 2 : 1, pos)
    draw_letter 0x448ad0(K, pos):
        if !(ctype(K) & 0xe): return                  // only letters and digits are drawn
        r = bounds + pos                               // Mac rect → RECT{left, top, right-10, bottom-4}
        font = (radio0 == 0) ? Helvetica 16 bold : Helvetica 24
        SetTextColor(0x000000); SetBkMode(TRANSPARENT); SetTextAlign(0)
        DrawText(one char K (static DAT_00499bfa), 1, r, 0x825)

click 0x448dc6:
    if !on: on = 1; by_mouse(+0x197) = 1; fire(0); invalidate()

tick 0x448e2b:
    if on == 1:
        if by_mouse:
            if mouse_down(): return                    // stays on while the button is held (anywhere)
            goto off
        if !key_down(K): goto off
        return
    else:
        if key_down(K): on = 1; by_mouse = 0; fire(0); invalidate()
    return
  off: on = 0; invalidate()                            // the release is not marked with fire()

start 0x448e04:  gadget_start(); if on: fire(0)
has_bool 0x448f53: true
reset:           base (on = 0)
```

`key_down` is `GetKeyState`, so the key must have focus in the game's
window. Holding the key repeats nothing, because the output just stays on.

---

## 7. cMouseClick `MsCl` ("Mouse Button", id 2032, Doc 4032)

**Sheet 1040**: frame 1 = hand pressing (on), frame 2 = hand raised
(off), both 57×46, frame 3 = 1×1. `bounds` = frame 2. It is not
clickable as a gadget (no slot 0x14). It watches the mouse buttons
globally, anywhere on the screen.

**State**: `+0x13e` u8 on. `+0x197` u8 armed (the buttons have been seen
released since the last reset).

**Settings**: dialog 2032 has none (it is just info).

```
draw 0x44ab97:   draw_frame(1040, on ? 1 : 2, pos)
tick 0x44ad46:
    d = mouse_down2()                     // left or right, anywhere
    if !d: armed = 1
    if armed and d != on:
        on = d; invalidate()
        if on: fire(0)                    // only the press is marked
start 0x44ad1f:  gadget_start(); if on: fire(0)
reset 0x44ab7f:  gadget_reset(); armed = 0
has_bool 0x44ad18: true
ctors: both call slot 0x1c (own reset) after init, then set armed = 0, then invalidate
```

Because of `armed`, a button already held when the run starts (for example
the click on GO) is ignored until it is released. Doc's tip (4032) says
the GO click *does* activate it. From the code this can only happen if the
run starts on mouse-down and a tick sees the button still down after it
was first seen up. See open questions.

---

## 8. cMouseMove `MsMv` ("Mouse Distance Measurer", id 2029, Doc 4029)

This is the "Mouse Distance Measurer". It outputs the total distance the
mouse has travelled **while a mouse button is held**, in inches (72 px =
1 inch). The picture shows how fast it is going.

**Sheet 1039**: frames 1..3 = mouse picture with 0/1/2 pairs of speed
arrows (57×46), frame 4 = 1×1. `bounds` = frame 1.

**State**:

| offset | type | what |
|---|---|---|
| `+0x146` | f64 | distance, inches (number output) |
| `+0x141` | u8 | picture frame 1..3 |
| `+0x197` | i16 | last cursor v (screen y) |
| `+0x199` | i16 | last cursor h (screen x) |
| `+0x19b` | f64 | distance at the last speed sample |
| `+0x1a3` | u32 | tick of the next speed sample |
| `+0x2b` | | 0 (no dialog preview hook) |

**Settings**: none (dialog 2029 is info only).

```
INCH = ext 1/72 (80-bit constant at 0x44b4c4 = 0.013888…)

tick 0x44b2dc:
    c = cursor_screen()
    if c != last:
        d = mouse_down() ? sqrt((c.h-last.h)² + (c.v-last.v)²) : 0.0   // ints → f64, x87
        last = c
    else: d = 0.0
    +0x146 = INCH * d + +0x146                     // extended, stored f64
    if d != 0.0: fire(0)
    t = now()
    if t > next (u32):
        speed = (+0x146 - +0x19b) / (f64)(u64)(t - next + 5) * 30.0   // f32 30; inches per 30 ticks
        DAT_00499fd8 = speed                       // a global (shared by all MsMv)
        next = t + 5; +0x19b = +0x146
        f = speed > 5.0 ? 3 : (speed < 1.5 ? 1 : 2)
        if +0x141 != f: +0x141 = f; invalidate()

draw 0x44b113:   draw_frame(1039, +0x141, pos)
reset 0x44b227:  gadget_reset(); last = cursor_screen(); +0x141 = 1; +0x19b = 0; next = 0
click 0x44b26a:  nop
has_number 0x44b2d5: true
```

`t - next + 5` is the number of ticks since the last sample, because
`next` was set to `t_prev + 5`. The speed is sampled at most every 6
ticks: the next sample is the first tick with `t > t_prev + 5`. The
distance is not cleared on start, only on Stop (reset). Movement is
measured in screen pixels and counts anywhere on screen.

---

## 9. cDynamitePlunger `DnPl` ("Dynamite Plunger", id 2030, Doc 4030)

**Sheet 1048**: frame 1 = handle up (off), frame 2 = pushed (on), 65×69,
frame 3 = 1×1. `bounds` = frame 1. Settings: none.

```
draw 0x4473f2:  draw_frame(1048, on ? 2 : 1, pos)
click 0x44750d: if !on: play(1048); on = 1; fire(0); invalidate()
has_bool 0x4475c1: true
reset/start: base                    // latched on until Stop
```

---

## 10. cHeart `hart` ("Heart", id 1017, Doc 3017)

**Sheet 1017**: frame 1 = heart 43×52 (resting), frame 2 = beating
(rect `(0,1)-(49,43)`), frame 3 = 1×1. `bounds` = frame 1.

**Settings** (dialog 1017 "Heart"):

| control | field | meaning |
|---|---|---|
| combo 107 "Whose Heart:" | num0 | animal 0..7, table below |
| radio 109 "Pulse Rate" (tag `1`) | radio0 = 0 | number output = beats per minute |
| radio 110 "Heart Weight (lbs.)" | radio0 = 1 | number output = weight |
| radio 111 "On During Heart Beat" | radio0 = 2 | bool output (fresh default) |

Table `DAT_00498ed2`, 8 × `{f64 bpm, f64 weight}`:

| num0 | combo text | bpm | weight (lbs) |
|---|---|---|---|
| 0 | Human Adult | 70 | 0.63 |
| 1 | Human Infant | 120 | 0.06 |
| 2 | Rabbit | 60 | 0.02 |
| 3 | Tortoise | 11 | 0.01 |
| 4 | Elephant | 49 | 44.0 |
| 5 | Cat | 120 | 0.02 |
| 6 | Dog | 100 | 0.24 |
| 7 | Gerbil | 280 | 0.003 |

```
tick 0x440c8a:
    period = ftol_clamped(3600.0f / bpm[num0])      // ticks per beat: 51, 30, 60, 327, 73, 30, 36, 12
    ph = now() % period                              // u32
    if !on:
        if ph <= period >> 2: play(1017); invalidate(); on = 1; fire(0)
    else:
        if ph > period >> 2:  play(1017); invalidate(); on = 0     // the sound plays on both edges
    if radio0 == 0: +0x146 = bpm[num0]
    elif radio0 == 1: +0x146 = weight[num0]
draw 0x440afe:  draw_frame(1017, on ? 2 : 1, pos)
has_bool 0x440daa:   radio0 == 2
has_number 0x440dbc: radio0 != 2
fresh ctor: radio0 = 2
```

Ticks only run in run mode. `now()` is the run's elapsed time, so the
first beat starts at the first tick (`ph = 0`).
The number output is rewritten every tick but never fired.

---
## 11. cLightBulb `bulb` ("Light Bulb", id 1016, Doc 3016)

**Sheet 1016**: frame 1 = bulb off, frame 2 = bulb lit (42×60), frame 3 =
1×1. `bounds` = frame 1. Settings: none (dialog 1016 has only OK).

```
draw 0x4492c1:  draw_frame(1016, on ? 2 : 1, pos)        // on = +0x13e; drawn in both modes
tick 0x449436:
    x = input(0) != 0.0                                  // NaN counts as 0, see logic.md §1.2
    if x != on: invalidate(); on = x; invalidate()
reset 0x44941e: gadget_reset(); on = 0
```

There are no outputs: the bool getters are not overridden, and slot 0x24
is false.

---

## 12. Number displays: cDisplay `disp`, cDisplay10 `di10`

Both use the digit sheet **100**: frames 1..10 = `0`..`9`, 11 = `-`,
12 = `+`, 13 = `.`, 14 = `E`. All are 13×25 except `E`, which is 12×25.
Both set `+0x13d = 1`. Start therefore marks the input cache "none", and
the first tick after GO always reads a wired input.

### 12.1 cDisplay `disp` ("Three-Digit Display", id 2002, Doc 4002)

**Sheet 1013** frame 1 = body 98×43 (`bounds`). **Points 1010/1013**:
`P0=(9,26)`, `P1=(9,48)`, `P2=(9,70)` (hundreds, tens, ones). Settings:
none.

**State**: `+0x142` i32 the shown integer (cleared by base reset).

```
tick 0x446410:
    if value_changed(0):
        +0x142 = ftol_clamped(floor(input(0) + 0.5))     // input is read twice; 0.5 is f32; round half up
        invalidate()

draw 0x445ef6:
    draw_frame(1013, 1, pos)
    if !run_mode: return                                 // digits only while running
    n = +0x142
    if !(n < 1000 and n > -100): draw_broken(); return
    if n < 0:
        a = -n
        draw_frame(100, 11, pos + P0)                    // '-'
        if a > 9: draw_frame(100, (a/10)%10 + 1, pos + P1)
        draw_frame(100, a%10 + 1, pos + P2)
    else:
        if n > 99: draw_frame(100, (n/100)%10 + 1, pos + P0)
        if n > 9:  draw_frame(100, (n/10)%10 + 1,  pos + P1)
        draw_frame(100, n%10 + 1, pos + P2)
```

The range shown is −99..999. There are no leading zeros: blank positions
are simply not drawn. An unwired display shows `0` while running, because
`value_changed` on an unwired knot is always false and `+0x142` stays 0
after reset.

### 12.2 cDisplay10 `di10` ("Ten-Digit Display", id 1014, Doc 3014)

**Sheet 1014** frame 1 = body 251×43 (`bounds`). **Points 1010/1014**: ten
character cells `P0..P9` = `(9,26) (9,48) (9,70) (9,92) (9,114) (9,136)
(9,157) (9,179) (9,201) (9,223)`. The spacing is 22 px except 21 between
cells 5 and 6. Settings: none.

```
tick 0x445ace:  if value_changed(0): +0x146 = input(0); invalidate()

draw 0x44586a:
    draw_frame(1014, 1, pos)
    if !run_mode: return
    s = format_number(+0x146, &+0x196)                   // logic.md §1.7; +0x196 = sticky decimals, cleared by reset
    for i in 0..10:
        c = s[i]
        f = '.'→13, ' '→skip, '+'→12, '-'→11, 'E'/'e'→14, digit→c-'/' (1..10)
        other: draw_broken(); return
        draw_frame(100, f, pos + P[i])
```

Only the first 10 characters of the formatted string are shown, so wide
fixed-point results are cut on the right. The sticky decimals mean that
once a value with 3 decimals has been shown, later values keep 3 decimals
until Stop.

---

## 13. cRGBDisplay `rgbd` ("Color Mixer", id 1018, Doc 3018) and CDitheredBrush

**Sheet 1018** frame 1 = body 117×82 (`bounds`). Settings: none.
`+0x13d = 1`.

**State**: `+0x197` u32 colour as a Windows `COLORREF` `0x00BBGGRR`
(0 = black). Cleared by the init and by reset (`0x451114`: gadget_reset +
colour = 0).

```
channel 0x451064(k) -> u16:
    w = wire(k); if !w: return 0
    if src.has_number(k'):                              // 0x43c1e8
        v = input(k)
        if v > 100.0: v = 100.0
        if v < 0.0:   v = 0.0
        return trunc(655.35 * v)                         // f64 constant; 0..65535
    if src_bool(w): return 0xffff                        // logic.md §1.2 (bool > object here, because number was ruled out)
    return 0

tick 0x45112d:
    if value_changed(0) || value_changed(1) || value_changed(2):    // short-circuit! When 0 changed, 1 and 2 are not re-cached this tick
        r = channel(0) >> 8; g = channel(1) >> 8; b = channel(2) >> 8    // each & 0xff
        +0x197 = r | g << 8 | b << 16
        invalidate()

draw 0x4511f2:
    draw_frame(1018, 1, pos)
    if !run_mode: return
    o = bounds + pos
    fill = Pt2Rect((o.top+6, o.left+17), (o.bottom-5, o.right-4))   // = {top+6, left+17, bottom-5, right-4}
    brush = CDitheredBrush(+0x197, 8, 8)                           // 0x43a226
    FillRect(hdc, RECT{fill.left, fill.top, fill.right, fill.bottom}, brush)
```

Inputs are percentages: 0..100 → 0..255 per channel
(`(trunc(655.35*v)) >> 8`, so 100 → 65535 → 255 and 50 → 32767 → 127).
A boolean source gives 0 or 255.

### CDitheredBrush `0x43a226(this, colorref, w=8, h=8)`

This builds an 8×8 **8-bit palette-index pattern** that dithers the colour
with the game palette. It is turned into a pattern brush
(`CreateBitmap(8, 8, 1, 8, bits)` + `CreatePatternBrush`). The target is
first converted to `t = 0x00RRGGBB` (bytes swapped from the COLORREF).
The palette is the 256-entry `RGBQUAD` table at `DAT_004a294e`, the game
palette copied into the offscreen DIB (`0x436xxx`). Each entry is read as
a u32 `0x00RRGGBB`.

```
dither 0x439f8a(t, n = 64, out[]):
    dist2(a, b) = Σ over R,G,B of (a.c - b.c)²                     // 0x439e88, i32
    best[8] = 8 × (idx 0, d 1_000_000)                            // table 0x496574
    overall = (idx 0, d 1_000_000)                                // 0x4965b4
    for i in 0..256:
        p = pal[i]
        o = (p.R > t.R) | (p.G > t.G) << 1 | (p.B > t.B) << 2      // 0x439f58: octant around t
        d = dist2(p, t)
        if d < best[o].d: best[o] = (i, d)                       // strict <: the lowest index wins ties
        if d < overall.d: overall = (i, d)
    out[0] = overall.idx
    acc = pal[overall.idx]                                        // running sums per channel (ints)
    for k in 1..n:
        bestE = 1_000_000; pick = 0
        for j in 0..8:                                            // all 8 octants, including empty ones (idx 0)
            c = acc + pal[best[j].idx]                            // per channel
            m = k + 1
            e = ((c.R - t.R*m)² + (c.G - t.G*m)² + (c.B - t.B*m)²) / (m*m)   // 0x439ee3; i32 terms, u32 division
            if e < bestE: bestE = e; pick = j; accNext = c
        acc = accNext; out[k] = best[pick].idx
```

`out` is filled row-major, 8 bytes per row. Each pixel is a palette index.
The pattern is a greedy "error diffusion over the running mean". Pixel
`k`'s choice depends on all earlier pixels, so the result is a fixed
function of `t` and the palette. A port can call `dither()` once per
colour change and tile the 8×8 result.

---
## 14. cTextLine `TxLn` ("Message List", id 2028, Doc 4028)

**Sheet 1046** frame 1 = strip 280×24 (`bounds`).

**Settings** (dialog 2028):

| control | field |
|---|---|
| edit 106 "1:" (filter `"33"`: max 33) | text0 |
| edit 108 "2:" | text1 |
| edit 110 "3:" | text2 |
| edit 112 "4:" | text3 |
| push 113 "Clear All Text" | button index 2 → `0x45b6e0`: sets text0..3 to `""` |

The fresh constructor sets text0..3 to `"Message One"`, `"Message Two"`,
`"Message Three"`, `"Message Four"`.

**State**: `+0x142` i32 the selected message, 0..3, or −1 for none.
`+0x197` i16 the message last "executed" (see `@` below). Both
constructors set it to −1.

```
tick 0x45bd72:
    n = trunc(floor(input(0))) - 1                       // input 1..4 → message 0..3; floor first (RC=down), then fistp (chop). No clamping.
    if n != +0x142: +0x142 = n; invalidate()

start 0x45bd57: gadget_start(); +0x142 = -1
reset: base (+0x142 = 0, so message 1 is selected after Stop, but it is only drawn in run mode)

draw 0x45bb9b:
    draw_frame(1046, 1, pos)
    if run_mode and 0 <= +0x142 < 4: draw_message 0x45b9ec(+0x142, pos)

draw_message 0x45b9ec(i, pos):
    buf = first 34 bytes of text[i] (strncpy 0x22), buf[34] = 0
    if run_mode:
        if buf[0] == '@' and +0x197 != i:
            WinExec(buf + 1, SW_SHOW)                   // easter egg: launch a program
        +0x197 = i                                       // i16
    SetTextColor(0x00ff00)                               // green
    SetBkMode(TRANSPARENT); font = Helvetica 16 bold
    r = bounds + pos → RECT{left+17, top, right-6, bottom}
    DrawText(buf, strlen(buf), r, 0x825)                 // single line, centred; the '@' is drawn too
```

* A message number outside 1..4 (including an unwired input = 0) shows
  nothing.
* The `@` behaviour runs **at draw time**, once each time a different
  message is drawn. `+0x197` is not reset between runs, so if a run
  begins on the same `@` message as the last one, nothing is launched. A
  port should probably not implement `WinExec`, but if it does, the
  trigger is as above. No FUNWIDGE file uses `@`.
* RIDDLE.WGT uses 5 lists. Each shows line *n* of a riddle, which is why
  the messages are short.

---

## 15. cTextBox `TxBx` ("Billboard", id 2027, Doc 4027)

**Sheet 1047** frame 1 = board 183×85 (`bounds`). `+0x34 = 1`: the
settings use **one 240-char text** (block `+0x18`..`+0x107`, gadget
`+0x4d`) instead of four fields.

**Settings** (dialog 2027): edit 106 "Enter Text:" (multi-line,
`ES_CENTER|ES_MULTILINE|ES_WANTRETURN`, no filter) is the text. Read-back
takes up to 255 chars, collapses `"\r\r"` to `"\r"` (`0x41a44e`),
compares the first 239 and stores 240 with `+0x13c = 0`. The text keeps
its `"\r\n"` line breaks. Push 107 "Clear Text" (button index 2 →
`0x45a7e8`) sets text0 to `""`. Fresh text: `"You can enter up to\r\nfour
lines of static text\r\nto be displayed\r\non this Billboard."`

**State**: `+0x13e` u8 shown.

```
tick 0x45ae81:
    x = input(0) != 0.0
    if on != x: on = x; invalidate()

draw 0x45acd5:
    draw_frame(1047, 1, pos)
    if run_mode and on: draw_text 0x45abe3(pos)

draw_text 0x45abe3(pos):
    crlf_restore(block)                                  // 0x45ab08, see below
    r = RECT{left = pos.h+15, top = pos.v+8, right = pos.h+178, bottom = pos.v+80}
    SetTextColor(0x000000); SetBkMode(TRANSPARENT); font = Helvetica 16 (normal)
    DrawText(text, strlen(text), r, 0x811)              // centred, word-wrapped, no clipping

reset: base (on = 0)
```

**Save (slot 0x40, `0x45aed5`)** hides the line breaks while the base
writes the block, then puts them back:

```
crlf_hide 0x45aa55(block):            // block = gadget+0x35
    block.num0 (u32 at +0x08) = 0x02611500                // magic
    n = 0
    for i in 0..239:                                     // i+1 < 0xf0
        if text[i] == '\r' and text[i+1] == '\n':
            if n < 4: block.radio[n] = i; n += 1          // u8 positions, in the radio bytes +0x04..+0x07
            text[i] = text[i+1] = ' '                     // every CRLF is blanked, even past the 4th
    block.radio[n..4] = 0xff
crlf_restore 0x45ab08(block):
    if block.num0 == 0x02611500:
        block.num0 = 0
        for each of the 4 position bytes p:
            if p < 0xef and text[p] == ' ' and text[p+1] == ' ': text[p] = '\r'; text[p+1] = '\n'
save = crlf_hide; base save 0x41d396; crlf_restore
```

On disk, a Win95-saved billboard therefore has its first 4 CRLFs as
spaces, `num0 = 0x02611500` (big-endian in the file, as all block numbers
are) and the positions in the radio bytes. CRLFs after the 4th are lost
(they stay spaces). The load constructor `0x45ab9b` runs `crlf_restore`.
The shipped files were written differently. SLOTMACH.WGT stores raw
`"\r\n"` with `num0 = 0`. ESPTEST.WGT's text has no line breaks. Both
load correctly, because `crlf_restore` does nothing without the magic.
`0x45af0a` (slot 0x48) only calls the base fill.

---

## 16. cTextLabel `TxLb` ("Label", id 2045, Doc 4045)

This has no sheet and no knots. It is free text on the worksheet, drawn in
both modes.

**Settings** (dialog 2045):

| control | field | meaning |
|---|---|---|
| edit 106 "Type your label here:" (filter `"30"`) | text0 | the label |
| radio 107 "Small letters" (tag `1`) | radio0 = 0 | Helvetica 16 **bold** |
| radio 108 "Large letters" | radio0 = 1 | Helvetica 24 normal |
| push 109 "Clear Text" | button index 2 → `0x45b645`: text0 = `""` | |

Fresh text0 = `"Label"`. Saved examples: 133 labels, mostly radio0 0,
some 1 (CLOCK.WGT "Grandfather Clock").

```
measure 0x45af79:                     // sets bounds from the text
    dc = GetDC(NULL) with font(radio0) selected
    (cx, cy) = GetTextExtentPoint(text0, strlen)
    bounds = {top -2, left -6, bottom cy+2, right cx+6}      // Pt2Rect((0,0),(cy,cx)), then grown by 6 across and 2 down

fresh ctor 0x45b126: base; init; text0 = "Label"; measure
    pos.v = d.v - (top + (bottom-top)/2)
    pos.h = d.h - (left + (right-left)/2) - 17           // note the extra -17
load ctor 0x45b2d3: base load; init; measure; invalidate
init 0x45b0fd: tag; +0x28 = 2045; +0x2f = 0x45b645; +0x33 = 2

draw 0x45b51e(view, clip):
    if bounds+pos ∩ clip empty: return
    draw_label 0x45b31c(pos, lights_on)

draw_label(pos, lit):
    measure()                                            // re-measured on every draw
    font(radio0); SetBkMode(TRANSPARENT); SetTextAlign(0)
    r = bounds + pos (as RECT)
    SetTextColor(lit ? 0xffffff : 0x000000)
    DrawText(text0, strlen, OffsetRect(r, +1, +1), 0x825)     // shadow, 1 px down-right
    SetTextColor(lit ? 0x000000 : 0xffffff)
    DrawText(text0, strlen, r, 0x825)                         // text
read dialog 0x45b611 (slot 0x4c): 0x41e851 read back; measure(); slot 0x1c (base reset); 0x425316(doc) (refresh)
```

With the lights on (the default), a label is black with a white shadow.
With the lights off, it is white with a black shadow.

---
## 17. cStripChart `StCh` ("Strip Chart", id 2044, Doc 4044)

**Sheet 1049**: frame 1 = chart 148×103 (`bounds`; a grid with a "RESET"
strip on the left), frame 2 = 1×1 (copied to `+0x20`), frame 3 = 4×5
pen marker. **Points 1010/1049**: `P0=(96,24)` is the plot origin
(bottom-left). The plot is **120 columns** (h = 24..143) by **80 rows**
(value 0 at v = 96, up to v = 17).

**State**:

| offset | type | what |
|---|---|---|
| `+0x197` | f64 | range low (From) |
| `+0x19f` | f64 | range high (To) |
| `+0x1a7` | u32 | sweep length in ticks |
| `+0x1ab` | i32 | last column written (−1 = none) |
| `+0x1af` | i32 | the y of the value that started column `+0x1ab` (marker) |
| `+0x1b3` | 120 × {i8 min, i8 max} | per column: lowest/highest y. Empty = {127, −128}. |
| `+0x2a5` | u32 | `now()` when the sweep started |

**Settings** (dialog 2044):

| control | field | meaning |
|---|---|---|
| edit 108 "From:" (filter `"5.#-"`) | text0 | low (fresh `"0"`) |
| edit 109 "To:" (filter `"5.#-"`) | text1 | high (fresh `"100"`) |
| radio 111 "Preset:" (tag `1`) | radio0 = 0 | use the combo |
| radio 112 "Custom:" | radio0 = 1 | use the hours field |
| combo 113 "Make chart this long:" | num0 | 0 = 10 seconds, 1 = 1 minute (fresh), 2 = 10 minutes, 3 = 1 hour |
| edit 114 (filter `"3.#"`) "hrs." | text2 | custom length in hours (fresh `"1"`) |
| push 115 "Reset Number Range" | button index 2 → `0x458b74`: text0 = `"0"`, text1 = `"100"` | |

Fill `0x459615` links combo 0 to radio button 0 and edit 2 to radio
button 1, then does the base fill. Preset table `DAT_0049bd5c` (u32
ticks): `600, 3600, 36000, 216000`.

```
clear 0x459297:
    for c in 0..120: col[c] = {min 127, max -128}
    +0x1af = -1; +0x1ab = -1; +0x2a5 = now()

start 0x4592f5:
    gadget_start()
    sscanf(text0, "%lf", &lo); sscanf(text1, "%lf", &hi)      // on failure the old +0x197/+0x19f is kept (never initialised by the ctors; else the last run's)
    if radio0 != 0:
        sscanf(text2, "%lf", &h)                            // h is uninitialised if this fails
        len = ftol_clamped(216000.0f * h)                   // hours → ticks, truncated
    else: len = table[num0]
    if lo == hi: lo = -2e-300; hi = 1e-300                  // exact constants 0x81b56e1f_c2f8f359 / 0x01a56e1f_c2f8f359
    if lo > hi: swap(lo, hi)
    if len < 60 (u32): len = 60
    clear()

tick 0x459469:
    if input(1) != 0.0:                                     // RESET input held: clear every tick
        if +0x1ab != -1: invalidate()
        clear(); return
    v = input(0)
    col = (u32)((now() - +0x2a5) * 120) / len               // u32 multiply wraps after ~35.8M ticks
    y = ftol_clamped((v - lo) / (hi - lo) * 80.0f)          // truncation toward 0, NOT rounding
    if (i32)col < 0 or col >= 120: return                   // the chart stops when full: no scrolling
    c = &col[col]
    if c.min > y: c.min = (y < -128) ? -128 : (i8)y
    if c.max < y: c.max = (y > 127) ? 127 : (i8)y
    if col > 0:
        p = &col[col-1]
        if p.min < 80 and p.max >= 0:                      // previous column has something visible
            if p.max < c.min:   c.min = p.max               // join to the previous column
            elif p.min > c.max: c.max = p.min
    if col != +0x1ab: +0x1ab = col; +0x1af = y; invalidate()   // the marker y is only taken at a column change

draw 0x458ea7(view, clip):
    if bounds+pos ∩ clip empty: return
    o = pos + P0                                            // (ov, oh)
    draw_frame(1049, 1, pos)
    if run_mode:
        pen = CPen(PS_SOLID, 0, RGB(0,0,0)) selected        // black, 1 px
        for c in 0..=+0x1ab:                                // nothing when +0x1ab == -1
            if col[c].min < 80 and col[c].max >= 0:
                y1 = ov - max(col[c].min, 0)
                y2 = ov - min(col[c].max, 79)
                if y1 == y2: y2 += 1
                MoveTo(oh + c, y1); LineTo(oh + c, y2)       // GDI: the end pixel is not drawn
    m = +0x1af; if m > 80: m = 79; if m < 0: m = 0          // note: 80 itself passes unclamped
    draw_frame(1049, 3, (ov - m, oh + +0x1ab))              // drawn in edit mode too (at oh-1, ov when empty)
```

* Values are mapped linearly, From → row 0 (bottom) and To → row 80
  (above the top row, so the top line is at y = 79). Out-of-range values
  are clipped per column by the `min < 80 && max >= 0` test and the
  clamps.
* A column's line runs from `max(min,0)` up to `min(max,79)`, excluding
  the top pixel. When min = max it is one pixel at `min`.
* The sweep lasts `len` ticks over 120 columns. When it is full, the chart
  freezes until RESET (input 1) or the next GO.
* The marker (frame 3) is placed at the column's first value. Because of
  that and the clamping above, a value exactly at To puts the marker 80
  rows up.
* Examples: HIMOM.WGT `num0 = 1` (1 minute), range 0..10. SINCOS.WGT
  `num0 = 2`, `"1.2"`/`"-1.2"` (swapped at start to −1.2..1.2), custom
  hours `"1"`, `".06"` with radio0 = 0 (unused).

---

## 18. cGraphicProjector `GrPr` ("Graphic Projector", id 2035, Doc 4035)

**Sheet 1050** (body, 131×98): frame 1 = black screen, frame 2 = blue
gradient screen, frame 3 = test-pattern screen, frame 4 = 1×1. `bounds` =
frame 1. **Rects 1012/1050**: `R0 = (11,23,82,119)`, the screen area.

**Picture sets**: WWGADGET sheet `200 + set`. Frame counts are in table
`DAT_00499b0e` (u8):

| num0 | combo text | sheet | frames | frame 1 size | body frame |
|---|---|---|---|---|---|
| 0 | Phases of the Moon | 200 | 8 | 42×43 | 1 |
| 1 | ESP Cards | 201 | 5 | 68×46 | 1 |
| 2 | Astrological Signs | 202 | 12 | 85×52 | 1 |
| 3 | Fortune Teller | 203 | 8 | 96×71 | 1 |
| 4 | Numbers (frames = digits `0`..`9`) | 204 | 10 | 42×53 | 3 |
| 5 | Rock Star | 205 | 7 | 56×63 | 2 |

Body frame: `num0 < 4` → 1, `== 4` → 3, `== 5` → 2. Any other value
draws no body (it cannot happen from the dialog). The picture sheets are
cached in a global array `DAT_004a3200[6]`, shared by all projectors.
They are loaded on start and freed when the last projector is destroyed.

**Settings** (dialog 2035):

| control | field | meaning |
|---|---|---|
| combo 107 "Picture Set:" | num0 | set 0..5 |
| radio 108 "Once Thru Set" (tag `1`) | radio0 = 0 | numbers above the count show nothing |
| radio 109 "Wrap Around Set" | radio0 = 1 | numbers wrap |

**State**: `+0x142` i32 the current picture (1-based, 0 = none).
`+0x197` u8 the frame count of the set (set at start).

```
start 0x448692:
    gadget_start()
    +0x197 = count[num0]
    if DAT_004a3200[num0] == null: DAT_004a3200[num0] = load sheet (200 + num0)

tick 0x448728:
    n = ftol_clamped(floor(input(0)))
    if radio0 != 0 and n > 0: n = (n - 1) % +0x197 + 1
    if n != +0x142: +0x142 = n; invalidate()

draw 0x448122(view, clip):
    if bounds+pos ∩ clip empty: return
    draw_frame(1050, body frame, pos)
    if !run_mode: return
    sheet = DAT_004a3200[num0]
    if sheet == null: draw_broken(); return               // e.g. drawn in run mode before this projector's first start
    f1 = sheet.frame(1).rect
    c(r) = (r.top + (r.bottom-r.top)/2, r.left + (r.right-r.left)/2)   // C division, truncation toward 0
    off = c(R0) - c(f1)
    if 0 < +0x142 <= +0x197:
        draw_frame(sheet, +0x142, pos + off)              // every frame is placed using frame 1's centre
reset: base (+0x142 = 0)
```

Input 1 shows the first picture. 0 or a negative number shows an empty
screen. Saved examples: ESPTEST.WGT `num0 = 1` (ESP cards). SLOTMACH.WGT
`num0 = 1`, radio0 = 1 (wrap). SHOTMOON.WGT `num0 = 0` (moon).

---

## 19. Stickers `Stk1`..`Stk9`, `St10` (cSticker1..10, base cSticker)

Pure decoration. There are no knots, no state and no settings (dialog
2034 "Sticker" is info only). The ten classes differ **only** in their
picture sheet. All share gadget id 2034 (the base ctor `0x454adc` /
`0x454b0d` sets `+0x28 = 2034` and vtable `0x49b5a4`).

| tag | class | fresh / load ctor | vtable | init | sheet | frame 1 size |
|---|---|---|---|---|---|---|
| `Stk1` | cSticker1 | 0x455077 / 0x4551ef | 0x49b6c4 | 0x454fb8 | 1052 | 82×85 |
| `Stk2` | cSticker2 | 0x4554af / 0x455627 | 0x49b754 | 0x4553f0 | 1053 | 110×75 |
| `Stk3` | cSticker3 | 0x4558e7 / 0x455a5f | 0x49b7e4 | 0x455828 | 1054 | 94×80 (rect left = 1) |
| `Stk4` | cSticker4 | 0x455d1f / 0x455e97 | 0x49b874 | 0x455c60 | 1055 | 94×80 (rect left = 1) |
| `Stk5` | cSticker5 | 0x456157 / 0x4562cf | 0x49b904 | 0x456098 | 1056 | 81×60 |
| `Stk6` | cSticker6 | 0x45658f / 0x456707 | 0x49b994 | 0x4564d0 | 1057 | 66×79 |
| `Stk7` | cSticker7 | 0x4569c7 / 0x456b3f | 0x49ba24 | 0x456908 | 1058 | 157×44 |
| `Stk8` | cSticker8 | 0x456dff / 0x456f77 | 0x49bab4 | 0x456d40 | 1059 | 118×51 |
| `Stk9` | cSticker9 | 0x457237 / 0x4573af | 0x49bb44 | 0x457178 | 1060 | 126×80 (rect left = 2) |
| `St10` | cSticker10 | 0x454c3f / 0x454db7 | 0x49b634 | 0x454b80 | 1061 | 144×85 |

```
init:  load sheet; tag; bounds = frame1.rect; +0x20 = frame2.rect (1×1)
draw:  draw_frame(sheet, 1, pos)          // drawn in both modes
tick:  0x454b7a (nop)
```

Draw functions: `0x455290` (1), `0x4556c8` (2), `0x455b00` (3),
`0x455f38` (4), `0x456370` (5), `0x4567a8` (6), `0x456be0` (7),
`0x457018` (8), `0x457450` (9), `0x454e58` (10). Destructors are at
draw − 0x66 (for example `0x45522a`, `0x454df2`). Sticker frames whose
rect left is not 0 (3, 4, 9) are drawn 1–2 px right of `pos`, as their
rect says. The tray/drop centring uses the rect too.

---
## 20. Proposed names

Shared (input, text, dialog):

| address | proposed name |
|---|---|
| 0x4343f2 | `WorksheetView_OnLButtonDown` (inferred) |
| 0x4344d2 | `WorksheetView_OnLButtonDblClk` (inferred) |
| 0x434451 | `WorksheetView_OnChar` (inferred) |
| 0x423756 | `Worksheet_MouseDown` (vtable 0x49326c slot 4) |
| 0x42385d | `Worksheet_Key` (slot 8) |
| 0x423dbd | `Worksheet_RunKey` (returns 0) |
| 0x423e3f | `Worksheet_RunClick` |
| 0x42391d | `Worksheet_FindGadgetAt` |
| 0x423ec3 | `Worksheet_EditMouseDown` (opens the settings dialog on double-click) |
| 0x41dac5 | `cGadget_ContainsPoint` (slot 0x18) |
| 0x411e03 | `PtInMacRect` |
| 0x41198a | `Pt2Rect` |
| 0x41195a | `SetMacRect(r, left, top, right, bottom)` |
| 0x411c6f | `SectMacRect` (returns by value) |
| 0x411d3e | `SectMacRectInPlace` |
| 0x41141f | `LoadPointList` (type 1010) |
| 0x4114da | `PointList_At` |
| 0x411f9a | `LoadRectList` (type 1012) |
| 0x412032 | `RectList_At` |
| 0x415208 | `MouseButtonDown` |
| 0x415284 | `MouseButtonDown2` (duplicate) |
| 0x41507c | `cKeyboard_IsKeyDown` |
| 0x4153ac | `GetCursorScreenPoint` |
| 0x4264b6 | `GetCursorWorksheetPoint` |
| 0x425e44 | `ScreenToWorksheet` |
| 0x480bc4 | `toupper` |
| 0x4110c3 | `ParseNumberClamped` |
| 0x41783b | `CreateGadgetFonts` |
| 0x417915 / 0x417931 / 0x41794d / 0x417969 | `FontSmall11` / `FontHelv16` / `FontHelv16Bold` / `FontHelv24` |
| 0x478893 | `CDC_SetBkMode` |
| 0x4790a9 | `CDC_SetTextAlign` |
| 0x479c0a | `CPen_Construct` |
| 0x41eb78 | `cGadget_DialogPreview` (`+0x2b` hook) |
| 0x41eb68 | `cGadget_GetPreviewHook` |
| 0x41baac | `cGadgetDialog_Construct` |
| 0x41acb9 | `cGadgetDialog_OnInitDialog` |
| 0x41a686 | `cGadgetDialog_OnCommand` |
| 0x41ca72 | `cGadgetDialog_SetButtonCallback` |
| 0x41c728 | `cGadgetDialog_SetText` |
| 0x41a396 | `cRadioGroup_Button` |
| 0x417c7b | `cDialogControl_LinkToRadio` |
| 0x41a44e | `CollapseDoubleCR` |
| 0x4402cc | `PlayGadgetSound` |
| 0x43a226 | `CDitheredBrush_Construct` |
| 0x43a3d2 | `CDitheredBrush_Destroy` |
| 0x439f8a | `DitherColor` |
| 0x439f58 | `ColorOctant` |
| 0x439e88 | `ColorDist2` |
| 0x439ee3 | `ColorMeanDist2` |

Per part:

| address | proposed name |
|---|---|
| 0x45989a / 0x45995a / 0x459ad7 | `cSwitch_Init` / `_Construct` / `_Load` |
| 0x459b22 / 0x459c3d / 0x459c8e / 0x459cb5 / 0x459880 / 0x459d3e / 0x459cd8 | `cSwitch_Draw` / `_Click` / `_Start` / `_Reset` / `_SettingsChanged` / `_HasBool` / `_Destroy` |
| 0x4516c5 / 0x4519a3 / 0x451b49 | `cSlider_Init` / `_Construct` / `_Load` |
| 0x451bc4 / 0x4520bb / 0x4524ad / 0x452459 / 0x45202a / 0x45238c / 0x45262d / 0x4520b4 / 0x452043 / 0x4516c0 | `cSlider_Draw` / `_Click` / `_Tick` / `_Start` / `_Reset` / `_ComputeValue` / `_FillDialog` / `_HasNumber` / `_Destroy` / `_SettingsChanged` (nop) |
| 0x4526cd | `cSlider_StaticInit` (clears the class rects) |
| 0x44d0b6 / 0x44d1a3 / 0x44d30f | `cNumericKeyPad_Init` / `_Construct` / `_Load` |
| 0x44d53e / 0x44d3f1 / 0x44d86b / 0x44db2d / 0x44dae5 / 0x44d766 / 0x44cfb8 / 0x44d797 / 0x44d36a | `cNumericKeyPad_Draw` / `_DrawLCD` / `_Click` / `_Tick` / `_Start` / `_Reset` / `_SettingsChanged` / `_HasNumber` / `_Destroy` |
| 0x44d726 / 0x44d79e / 0x44cff6 / 0x44da9d | `cNumericKeyPad_Clear` / `_AppendChar` / `IsValidNumberText` / `_Enter` |
| 0x448844 / 0x4488fa / 0x448a85 | `cKey_Init` / `_Construct` / `_Load` |
| 0x448c57 / 0x448ad0 / 0x448dc6 / 0x448e2b / 0x448e04 / 0x448f53 / 0x448da7 / 0x448ee2 | `cKey_Draw` / `_DrawLetter` / `_Click` / `_Tick` / `_Start` / `_HasBool` / `_IsKeyDown` / `_Destroy` |
| 0x44a8b0 / 0x44a92d / 0x44aaee | `cMouseClick_Init` / `_Construct` / `_Load` |
| 0x44ab97 / 0x44ad46 / 0x44ad1f / 0x44ab7f / 0x44ad18 / 0x44acb2 | `cMouseClick_Draw` / `_Tick` / `_Start` / `_Reset` / `_HasBool` / `_Destroy` |
| 0x44ae04 / 0x44aea5 / 0x44b074 | `cMouseMove_Init` / `_Construct` / `_Load` |
| 0x44b113 / 0x44b2dc / 0x44b227 / 0x44b26a / 0x44b2d5 / 0x44b26f | `cMouseMove_Draw` / `_Tick` / `_Reset` / `_Click` (nop) / `_HasNumber` / `_Destroy` |
| 0x447174 / 0x44722a / 0x4473a7 | `cDynamitePlunger_Init` / `_Construct` / `_Load` |
| 0x4473f2 / 0x44750d / 0x4475c1 / 0x447550 | `cDynamitePlunger_Draw` / `_Click` / `_HasBool` / `_Destroy` |
| 0x440880 / 0x44092f / 0x440ab3 | `cHeart_Init` / `_Construct` / `_Load` |
| 0x440afe / 0x440c8a / 0x440daa / 0x440dbc / 0x440c19 | `cHeart_Draw` / `_Tick` / `_HasBool` / `_HasNumber` / `_Destroy` |
| 0x448fe8 / 0x449093 / 0x449210 | `cLightBulb_Init` / `_Construct` / `_Load` |
| 0x4492c1 / 0x449436 / 0x44941e / 0x44925b | `cLightBulb_Draw` / `_Tick` / `_Reset` / `_Destroy` |
| 0x445b98 / 0x445c9f / 0x445e1c | `cDisplay_Init` / `_Construct` / `_Load` |
| 0x445ef6 / 0x446410 / 0x445e67 | `cDisplay_Draw` / `_Tick` / `_Destroy` |
| 0x44550c / 0x445613 / 0x445790 | `cDisplay10_Init` / `_Construct` / `_Load` |
| 0x44586a / 0x445ace / 0x4457db | `cDisplay10_Draw` / `_Tick` / `_Destroy` |
| 0x450ddc / 0x450e9c / 0x451019 | `cRGBDisplay_Init` / `_Construct` / `_Load` |
| 0x4511f2 / 0x45112d / 0x451064 / 0x451114 / 0x45155d | `cRGBDisplay_Draw` / `_Tick` / `_ReadChannel` / `_Reset` / `_Destroy` |
| 0x45b70a / 0x45b7d6 / 0x45b9a5 | `cTextLine_Init` / `_Construct` / `_Load` |
| 0x45bb9b / 0x45b9ec / 0x45bd72 / 0x45bd57 / 0x45b6e0 / 0x45bcf1 | `cTextLine_Draw` / `_DrawMessage` / `_Tick` / `_Start` / `_ClearAllText` (dialog button) / `_Destroy` |
| 0x45a801 / 0x45a8d4 / 0x45ab9b | `cTextBox_Init` / `_Construct` / `_Load` |
| 0x45acd5 / 0x45abe3 / 0x45ae81 / 0x45aed5 / 0x45af0a / 0x45a7e8 / 0x45ae1b | `cTextBox_Draw` / `_DrawText` / `_Tick` / `_Save` / `_FillDialog` / `_ClearText` (dialog button) / `_Destroy` |
| 0x45aa55 / 0x45ab08 | `cTextBox_HideCRLF` / `_RestoreCRLF` |
| 0x45b0fd / 0x45b126 / 0x45b2d3 | `cTextLabel_Init` / `_Construct` / `_Load` |
| 0x45b51e / 0x45b31c / 0x45af79 / 0x45b611 / 0x45b645 / 0x45b6b2 | `cTextLabel_Draw` / `_DrawText` / `_MeasureBounds` / `_ReadDialog` / `_ClearText` (dialog button) / `_Destroy` |
| 0x458b9f / 0x458ca6 / 0x458e5c | `cStripChart_Init` / `_Construct` / `_Load` |
| 0x458ea7 / 0x459469 / 0x4592f5 / 0x459297 / 0x459615 / 0x458b74 / 0x459226 | `cStripChart_Draw` / `_Tick` / `_Start` / `_ClearTrace` / `_FillDialog` / `_ResetRange` (dialog button) / `_Destroy` |
| 0x447dcc / 0x447ea2 / 0x44801f | `cGraphicProjector_Init` / `_Construct` / `_Load` |
| 0x448122 / 0x448728 / 0x448692 / 0x44806a | `cGraphicProjector_Draw` / `_Tick` / `_Start` / `_Destroy` |
| 0x454adc / 0x454b0d / 0x454b43 / 0x454b7a | `cSticker_Construct` / `_Load` / `_Destroy` / `_Tick` (nop) |
| 0x454fb8, 0x4553f0, 0x455828, 0x455c60, 0x456098, 0x4564d0, 0x456908, 0x456d40, 0x457178, 0x454b80 | `cSticker1_Init` .. `cSticker9_Init`, `cSticker10_Init` |

---

## 21. Corrections to gadget-model.md

1. **Slot 0x18 is a hit test**, not "move to (point)". `0x41dac5(this, Point
   pt)` returns `PtInRect(bounds, pt − pos)`. The worksheet uses it to find
   the clicked gadget (`0x42391d`).
2. **Slot 0x14 (click)** is called as `(this, Point pt, u16 flags=0, u8
   dbl)`. A double-click calls it a second time with `dbl = 1`. Only the
   run-mode mouse-down path calls it (§1.1). The keyboard never reaches
   gadgets.
3. **Slot 0x30's base (`0x41db39`) returns `+0x146` as f64**. It is not a
   no-op. Most number parts only override slot 0x2c and keep their value
   in `+0x146`.
4. **The "4 number fields" of the settings block are combo-box selection
   indices** (`CB_GETCURSEL`, read back as u16). Combos are numbered in
   control-id order. Radio groups are tagged by the last character of
   each button's caption. Edit templates are input filters. (logic.md
   §1.6 has the details. This file adds: push buttons are numbered in id
   order including OK = 0 and Cancel = 1, and `+0x2f`/`+0x33` attach a
   callback to one of them, §1.5.)
5. **Slot 0x50** is not just "settings changed". It runs from the dialog
   preview hook `+0x2b` (`0x41eb78`) on control changes, and that hook
   then restores the settings block but not the state slot 0x50 derived
   from it. It also runs on Cancel (`0x41a686`) and from some of the
   gadgets' own code (`NmKP` load and reset). OK runs slot 0x4c only.
6. **Start** (`0x41e50a`) sets the knot cache to "none" and clears the
   wire flags only when `+0x13d` is set. Otherwise (`0x41e4c8`) it primes
   the cache with the current input values and leaves wire flags alone.
   (logic.md §1.5 agrees.)
7. **Reset (slot 0x1c) runs on Stop** (`0x430ee7` → `0x424a94`), and in the
   constructors. **Start (slot 0x20) runs on GO** (`0x430e70` →
   `0x424a12`). Base reset also zeroes `+0x142..+0x151` and `+0x141`, sets
   `+0x13f = 1`, clears `+0x140` (with an invalidate if it was set) and
   clears `+0x196`.
8. **`+0x2b`** is a code pointer (the dialog preview hook, default
   `0x41eb78`). `+0x2f` (code pointer) and `+0x33` (u8 index) are a dialog
   push-button callback. `+0x2a` is set to 1 by the base init for every
   gadget.
9. **`+0x20`** is a second rect. The base init `0x41cf8a` loads it from
   WWGADGET type 1013 id 2 (`0x411e4b`): `{top 0, left 0, bottom 35, right
   70}`. `StCh` and the stickers overwrite it with their frame 2's rect (a
   1×1 rect at 0,0). No reader was found in the code read for this file;
   it is probably the rect used when the part is drawn small, for example
   in the tray.
10. **Resource numbers**: sheet/point/knot resources use a per-class number
    that is often not the gadget id (`+0x28`). For example `NmKP` is gadget
    2007 but res 1027, and `disp` is gadget 2002 but res 1013. The knot
    loader `0x41db6c` takes that number.
11. **`Key_` is cKey** (the "Keypress" part), not related to
    cGraphicProjector.
12. The tag table's sticker row ("`Stk1`..`Stk9`, `St10` | 0x455077 ..
    0x454c3f") is right but out of order. See §19 for each class.
13. **Getters take the source's output knot** as a second argument
    (`slot(this, u16 knot)`). None of the parts here use it.
14. `+0x13e` is "what GetBool returns". For `TxBx`, `bulb`, `Key_`,
    `swit`, `DnPl`, `MsCl` and `hart` it is also the visible on/off state.
    `bulb` and `TxBx` have no bool output (slot 0x24 is false) but still
    keep their state there.

---

## 22. Open questions

* **View message mapping.** `0x4343f2` / `0x4344d2` / `0x434451` were
  identified by shape (`(this, nFlags, CPoint)`, a constant 0/1 for
  "double-click", and a char handler). No pointer to them was found in the
  binary's data, so the message map entries were not confirmed. They may
  go through Borland/MFC thunks. If `0x4344d2` is actually right-button
  down rather than double-click, a right click would also click gadgets.
* **MsCl and the GO click.** Doc says starting with the GO button
  activates the Mouse Button part. The code ignores a button held since
  reset (`armed`). Whether GO starts the run on button-down (so the first
  tick sees the button still down) was not traced in the tool palette
  code.
* **Dialog control linking** (`0x417c7b`, `ctrl+10 = radio button`). It
  presumably enables a control only while its radio button is checked.
  The enabling code was not traced.
* **`lights_on` (`DAT_00493ddc`)**. Its meaning (dark/light worksheet) is
  inferred from the dialog background brush and the toggle sounds. Only
  the label's colours depend on it here.
* **CDitheredBrush palette**: `DAT_004a294e` is assumed to be the active
  256-colour game palette (it is copied into the offscreen DIB's colour
  table at `0x436xxx`). Where it is filled was not traced.
* **Uninitialised paths**: Slider preset `num0 > 1` and Strip Chart custom
  hours that fail to parse use uninitialised stack values. The keypad
  draws a stale frame for unexpected characters. The Strip Chart's
  `lo`/`hi` fields are never initialised by its constructors (they are
  only read after a successful `sscanf` in start, unless the text doesn't
  parse). None of these can be reached from the dialogs.
* `+0x19b..+0x1a2` of cSlider (four i16 zeroed by both constructors) have
  no other use in the slider code. They may be leftovers.
