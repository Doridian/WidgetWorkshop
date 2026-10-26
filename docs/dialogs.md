# Settings dialogs: the gadget-specific parts

What `WIDGET.EXE` (Win95) does in a gadget's settings dialog beyond the
generic mapping of controls to the settings block. That generic part
(`0x41acb9` builds the control lists, `0x41ea4d` fills them, `0x41e851`
reads them back) is described in [`gadgets/logic.md`](gadgets/logic.md)
§1.6, [`gadgets/io.md`](gadgets/io.md) §1.5–1.6 and
[`editor-interaction.md`](editor-interaction.md) §6. The port
(`crates/engine/src/editor/dialog.rs`) already implements it.

This file specifies what the port is still missing:

* the gadget overrides of vtable slot **0x48** (fill the dialog) and
  slot **0x4c** (read it back);
* the push buttons other than OK, Cancel and Doc ("Clear Text",
  "Reset Number Range", "Record a Sound", "Import .WAV file"...);
* the control **links** that the slot 0x48 overrides set up (typing in
  "Custom" selects the "Custom" radio button, and so on), the edit-field
  validation, and the progress bar;
* the Recorder's recording and import, and how the original uses the
  microphone.

All addresses are in `WIDGET.EXE`. Nothing here touches `WWGADGET.DLL`
or `WWSOUND.DLL`: the dialog code and the waveIn code are all in the EXE.
The dialog templates are WIDGET `RT_DIALOG <gadget id>`; their combo
strings are WIDGET type 1022 `<gadget id>`.

---

## 1. The dialog machinery the overrides use

### 1.1 Control lists and accessors

`cDialog` (vtable `0x491d70`, built by `0x41baac(dlg, gadget, hook)`)
keeps one list per kind of control. `OnInitDialog` (`0x41acb9`) fills
them in control-id order (ids 1..0xa9):

| dlg offset | list | template control | wrapper (vtable, kind) | accessor `(dlg, n)` |
|---|---|---|---|---|
| `+0x50` | check boxes | `Button` style 2/3 | `cBoolControl` `0x417ca0` (`0x491540`, kind **1**) | `0x41c27b` |
| `+0x5a` | push buttons | `Button` style 0/1 (OK and Cancel included) | `cButton` `0x417d54` (`0x491570`, kind **2**) | `0x41c1d9` |
| `+0x64` | radio groups | `Button` style 4/9, grouped by the caption's last character | `cRadioGroup` `0x419ecc` (`0x491828`, kind **8**); its buttons are `cBoolControl`s (kind 1) | `0x41c23b`; button `k` of a group: `0x41a396(group, k)` |
| `+0x6e` | combo boxes | `ComboBox` | `cIntControl` `0x419e18` (`0x491770`, kind **3**) | `0x41c25b` |
| `+0x78` | edit fields | `Edit` | `cEditText` `0x419c1a` (`0x491748`, kind **4**) | `0x41c1f9` |
| `+0x82` | statics | `Static` | `cTextControl` `0x41a3ec` (`0x491858`, kind 5) | — |

So `n` always counts controls **of that kind, in control-id order**.
For radio groups, `n` is the group number, which is the order in which
each group's first button appears by control id. `k` is the button's
index within its group, also in control-id order. These are the same
numbers that the settings block uses (`+0x04 + group`, the combo
`+0x08 + 4n`, and so on).

Note: [`editor-interaction.md`](editor-interaction.md) §6 lists `+0x50`
as "check boxes/plain buttons" and `+0x5a` as "radio buttons". In the
builder, push buttons go to `+0x5a`, check boxes to `+0x50`, and radio
buttons only into the groups at `+0x64`.

Every wrapper (`cDialogControl`, vtable `0x491518` and descendants) has:

| offset / slot | what |
|---|---|
| `+0x04` | `HWND` |
| `+0x08` | control id (`0x417c64`) |
| `+0x0a` | **link**: another control, or null (set by `0x417c7b`, read by slot 0x08 `0x417c70`) |
| slot 0x0c | kind (table above) |
| slot 0x10 / 0x14 | `EnableWindow(TRUE)` (`0x417c52`) / `EnableWindow(FALSE)` (`0x417bf0`) |
| slot 0x18 | "is control id x" (`0x417c02`; for a radio group `0x41a057`: "has a button with id x") |
| slot 0x1c / 0x20 | get / set the value (check state `BM_GETCHECK`/`BM_SETCHECK`; combo `CB_GETCURSEL`/`CB_SETCURSEL`; radio group: the selected index `+0x19`, set by `0x41a2ae`) |
| `cButton +0x0e` | the push-button handler `fn(dlg, gadget)` (set by `0x417de0`, run by `0x417dc3`) |

For a radio group, slot 0x08 (`0x41a3b6`) returns **the link of its
currently selected button**, not a link of its own.

### 1.2 `0x417c7b(control, target)`: the link

All the slot 0x48 overrides except the Recorder's do only one thing:
they call `0x417c7b(src, target)`, which stores `target` at `src+0x0a`.
They then call the base fill `0x41ea4d`. The target is always a radio
button or a check box.

The link does **not** enable or disable anything. Earlier docs guessed
that it did (io.md §4, logic.md §11/§12 and the open question at the end
of logic.md, sound-time.md §6/§10, science.md "Settings dialogs"). No
code in the dialog ever disables a settings control. The only
`EnableWindow(FALSE)` that gadget code calls is the Recorder's error path
(§4.2). What the link does is **select the target when the user works
the source control**. This is done by the command handler (§1.3), so
typing a custom value selects "Custom:", picking a preset selects
"Preset:", and so on.

### 1.3 `OnCommand` (`0x41a686`) in full

`OnCommand(dlg, wParam = (notify << 16) | id, lParam)`:

```
if id == 0xfd: ready (DAT_004918bc) = 1          // posted by OnInitDialog after the fill
0x44081d()                                       // sound housekeeping (0x43f799), unrelated
if id == 1:   UndoSnapshot(0x4154b4); gadget.slot4c(gadget, dlg); EndDialog(1)
elif id == 2: gadget.slot50(gadget); EndDialog(2)
elif id == 150 (0x96): Doc balloon page toggle (0x418ca0)
elif id < 0x100:
    c = find(dlg, id)                            // 0x41a58a: push, combo, edit, static, check lists;
                                                 // else the radio group that has the button: re-read
                                                 // which button is checked (0x41a0be(group, -1)) and use the GROUP
    if c and ready:
        kind = c.kind
        if notify == EN_CHANGE (0x300) and kind == 4 and !reentered (DAT_004918c0):
            reentered = 1; validate(c) (0x419ba9, §1.5); reentered = 0
        if (notify == BN_CLICKED (0) and kind in {1, 2, 6, 8})
           or (notify == EN_CHANGE and kind == 4)
           or (notify == CBN_SETFOCUS (3) and kind == 3):
            first = true
            while c:                                 // walk the link chain
                switch c.kind:
                  1 (check box / radio button):
                      if !first: c.set(1)             // BM_SETCHECK 1
                      for each radio group g: if g has c.id: g.select(c.id) (0x41a0be); break
                  2 (push button): c.handler(dlg, gadget)  (0x417dc3)
                  4 (edit): SetFocus(c)
                  3, 8: nothing
                first = false
                c = c.link                            // slot 0x08; for a group: its selected button's link
        if dlg.hook (+0x48, = gadget+0x2b): hook(dlg, gadget)   // live preview, 0x41eb78
CDialog::OnCommand(...)                         // 0x473f7a
```

Points for the implementation:

* The chain is followed only for these notifications: a click on a
  button or a radio button, a text change in an edit field, or a combo box
  **getting the focus** (`CBN_SETFOCUS`, not `CBN_SELCHANGE`). So
  clicking a preset list selects "Preset:" before anything is chosen in
  it. A port without focus events can do this on opening the list or on
  any selection in it.
* The source control itself is never re-checked (`first`). If the
  source is a check box (Sound Transformer), the chain runs whether the
  click checked it or unchecked it.
* The preview hook runs after **any** notification from a known control
  once `ready` is set, including `CBN_SELCHANGE`, focus changes and the
  `EN_CHANGE` caused by a push-button handler's `WM_SETTEXT`. The hook is
  `0x41eb78` unless the class set `+0x2b = 0` (see the table, column
  "+0x2b").
* During `OnInitDialog`, `ready` is 0 (it is cleared at the start of
  `0x41acb9` and set only by the posted command `0xfd`). So the fill's
  own `WM_SETTEXT`/`BM_SETCHECK` neither validate, follow links nor
  preview.
* A push button that has no handler (`+0x0e == 0`) does nothing.

### 1.4 Push-button handlers (`+0x2f` / `+0x33`)

Push buttons are numbered in control-id order over **all** style 0/1
buttons. Every settings template has OK = 1 and Cancel = 2 as its two
lowest ids, so they are numbers 0 and 1, and the first extra button is
number 2. The gadget's init stores a handler at `+0x2f` and its number at
`+0x33`. The base fill attaches it with `0x41ca72(dlg, +0x33, +0x2f)`
(a no-op if that button doesn't exist). A slot 0x48 override can attach
more buttons the same way. Only the Recorder does this. A handler is
`void fn(cDialog* dlg, cGadget* g)`.

Helpers the handlers use:

| fn | what |
|---|---|
| `0x41c728(dlg, n, text)` | `WM_SETTEXT` on edit `n` (no-op if `n` is out of range). This triggers `EN_CHANGE` → validation, links, preview. |
| `0x41c3ea(dlg, n)` | disable push button `n` (wrapper slot 0x14) |
| `0x41c1d9(dlg, n)` + `0x417c89` | push button `n`; `SetFocus` on it |
| `0x41caca(dlg, pct)` | progress bar: `dlg+0x9c = pct`, then `InvalidateRect` + `UpdateWindow` on control 160 (`0xa0`) |

The only templates with extra push buttons are 2026, 2027, 2028, 2044
and 2045 (checked over every `RT_DIALOG`). Each of them has a handler.

### 1.5 Edit-field validation (`0x419ba9`, on every `EN_CHANGE` once ready)

The template text of an edit field is a filter, parsed by `0x419c1a`
into the wrapper's fields. The field is then cleared (`WM_SETTEXT ""`):

| char | field | meaning |
|---|---|---|
| digit(s) | `+0x112` u16 | maximum length (decimal; `"33"` = 33) |
| `#` | `+0x10e` | number |
| `.` | `+0x110` | real number (decimal point and exponent) |
| `-` | `+0x10f` | minus allowed |
| `A` | `+0x111` | letters and digits only |

Other characters (such as the space in the Cannon's `"#5 "`) are ignored.
On `EN_CHANGE`:

```
validate(edit) 0x419ba9:
    t = GetWindowText(edit)                       // up to 255 chars
    if ok(edit, t) (0x419ad8): edit.last_good (+0x114, 256 bytes) = t
    else: SetWindowText(edit, edit.last_good)     // revert the keystroke/paste

ok(edit, t) 0x419ad8:
    if 'A':        r = every char is alphanumeric (ctype & 0xe)          // 0x419ab1
    elif '.':
        if '-':    r = t == "" or t is "." or "-" or parse(t)            // 0x419904
        else:      r = t == "" or t == "." or parse(t)                   // 0x419828
        where parse(t): drop one trailing '.', 'e' or 'E'; sscanf(t, "%lf%c") == 1
    elif '#':
        if '-':    r = t == "" or t == "-" or sscanf(t, "%d%c") == 1      // 0x4199e1
        else:      r = t == "" or (sscanf(t, "%d%c") == 1 and value >= 0) // 0x419a59
    else:          r = true
    if max != 0 and strlen(t) > max: r = false
    return r
```

* A real-number field without `-` still accepts a pasted or completed
  negative such as `"-5"`, because `%lf` parses it. Only a lone `"-"` is
  rejected, so a minus can't be typed first.
* There are **no message boxes and no string resources** for invalid
  input. A rejected change silently restores the last good text. No
  dialog checks anything on OK.
* `last_good` is never set by the fill (it runs before `ready`), and the
  constructor doesn't initialise it (see Open questions). A port should
  start it as the filled text.

Read-back (`0x41a49b`) sends `EM_FMTLINES(TRUE)`, reads up to 255 chars,
sends `EM_FMTLINES(FALSE)`, and then turns every `"\r\r"` into `"\r"`
(`0x41a44e`). On the multi-line Billboard field, `EM_FMTLINES` inserts
`"\r\r\n"` at each soft (word-wrap) line break, so **soft breaks are
stored as hard `"\r\n"` breaks**. Single-line fields are unaffected.

### 1.6 Owner-drawn control 160: the progress bar

`WM_DRAWITEM` for id 160 (`0xa0`) (`0x41a912`) splits the item rect at
`x = MulDiv(dlg+0x9c, rc.right, 100)`. It fills `[0, x)` with a solid
`RGB(0,0,255)` brush if `dlg+0x9c != 0`, and `[x, right)` with
`RGB(255,255,0)` if `dlg+0x9c < 100`. `dlg+0x9c` is a u16, 0 when the
dialog opens, so the bar starts all yellow. Only the Recorder's template
has control 160.

---

## 2. All 71 classes: slots 0x48 / 0x4c

These are all the vtables in `WIDGET.EXE` derived from `cGadget`.
They were found by scanning the image for cGadget's slot 0x0c/0x10 pair
(`0x41e359`, `0x41ed9d`), which no class overrides: exactly 71 hits.
Base values: slot 0x48 = `0x41ea4d`, slot 0x4c = `0x41ea2e` (read back
with `0x41e851`, then refresh the worksheet with `0x425316`), slot 0x50 =
`0x41eb73` (nop). **Bold** = overridden. Slot 0x50 is listed because the
live preview (`+0x2b` hook `0x41eb78`) calls it after every change.
"Handler" is the `+0x2f` push-button callback (button number 2 unless
stated). "+0x2b" shows 0 where the init removes the preview hook.

| vtable | class | tag | dialog (`+0x28`) | slot 0x48 | slot 0x4c | slot 0x50 | handler | +0x2b |
|---|---|---|---|---|---|---|---|---|
| `0x491e58` | cGadget (base) | — | — | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x498f8e` | cHeart | `hart` | 1017 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x498ff0` | cAdder | `Addr` | 2016 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  | 0 |
| `0x499080` | cAndGate | `AndG` | 2008 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x499116` | cAnimalSound | `AnSd` | 2040 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x4991cc` | cAreaVolume | `ArVo` | 2003 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x499282` | cCoinToss | `Coin` | 2006 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49931a` | cComparison | `Comp` | 2021 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x4993d4` | cConstant | `cnst` | 2004 | 0x41ea4d | 0x41ea2e | **0x442dc4** |  |  |
| `0x499488` | cConversion | `Conv` | 2020 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49953e` | cCounter | `Cntr` | 2033 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x4995f4` | cDigitalClock | `DClk` | 1009 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x4996d2` | cDisplay10 | `di10` | 1014 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x4997b2` | cDisplay | `disp` | 2002 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x499814` | cDivider | `Divr` | 2019 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  | 0 |
| `0x4998e0` | cDropChamber | `Drop` | 1001 | **0x44706c** | 0x41ea2e | 0x41eb73 |  |  |
| `0x499972` | cDynamitePlunger | `DnPl` | 2030 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x499a28` | cFirstThru | (1thr, unregistered) | — | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x499a88` | gate base (RTTI name not found) | — (base) | — | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x499b74` | cGraphicProjector | `GrPr` | 2035 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x499c2c` | cKey | `Key_` | 1010 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x499cbe` | cLightBulb | `bulb` | 1016 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x499d70` | cMetronome | `Metr` | 1011 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x499e28` | cMiniConstant | `MiCo` | 2046 | 0x41ea4d | 0x41ea2e | **0x449b24** |  |  |
| `0x499eba` | cMiscSound | `MsSd` | 2041 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x499f4e` | cMouseClick | `MsCl` | 2032 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x499fec` | cMouseMove | `MsMv` | 2029 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  | 0 |
| `0x49a07e` | cMultiplexer | `Mplx` | 2025 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49a0e0` | cMultiplier | `Mulr` | 2018 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  | 0 |
| `0x49a194` | cNoiseMaker (probably) | (Nois, unregistered) | — | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49a224` | cNotGate | `NotG` | 2010 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49a2e0` | cNumberCruncher | — (base) | (501) | 0x41ea4d | 0x41ea2e | 0x41eb73 |  | 0 |
| `0x49a3a8` | cNumericKeyPad | `NmKP` | 2007 | 0x41ea4d | 0x41ea2e | **0x44cfb8** |  |  |
| `0x49a9bb` | cOneNumberCruncher | `1ncr` | 2005 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49aa4c` | cOrGate | `OrGa` | 2009 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49ab30` | cPendulum | `Pend` | 1000 | **0x44f393** | 0x41ea2e | 0x41eb73 |  |  |
| `0x49abec` | cPhysicsCanon | `Cano` | 1002 | **0x44fe21** | 0x41ea2e | 0x41eb73 |  |  |
| `0x49af20` | cPlanet | `Plnt` | 2000 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49afb8` | cRandomNumberGenerator | `rand` | 1007 | **0x450cdb** | 0x41ea2e | 0x41eb73 |  |  |
| `0x49b06c` | cRGBDisplay | `rgbd` | 1018 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49b120` | cSlider | `Sldr` | 1015 | **0x45262d** | 0x41ea2e | **0x4516c0** |  |  |
| `0x49b216` | cSoundSwitch | `SdSw` | 2037 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49b300` | cSoundTransformer | `SdTr` | 2038 | **0x4536ea** | 0x41ea2e | 0x41eb73 |  |  |
| `0x49b392` | cSpeaker | `Spkr` | 2042 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49b424` | cSplitter | — (base) | — | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49b484` | cSplitter2 | `Spl2` | 2022 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49b4e4` | cSplitter3 | `Spl3` | 2023 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49b544` | cSplitter4 | `Spl4` | 2024 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49b5a4` | cSticker | — (base) | 2034 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49b634` | cSticker10 | `St10` | 2034 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49b6c4` | cSticker1 | `Stk1` | 2034 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49b754` | cSticker2 | `Stk2` | 2034 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49b7e4` | cSticker3 | `Stk3` | 2034 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49b874` | cSticker4 | `Stk4` | 2034 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49b904` | cSticker5 | `Stk5` | 2034 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49b994` | cSticker6 | `Stk6` | 2034 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49ba24` | cSticker7 | `Stk7` | 2034 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49bab4` | cSticker8 | `Stk8` | 2034 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49bb44` | cSticker9 | `Stk9` | 2034 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49bc00` | cStopper | `Stop` | 2043 | **0x457e95** | 0x41ea2e | 0x41eb73 |  |  |
| `0x49bcb4` | cStopWatch | `Wtch` | 1004 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49be04` | cStripChart | `StCh` | 2044 | **0x459615** | 0x41ea2e | 0x41eb73 | 0x458b74 |  |
| `0x49be64` | cSubtracter | `Subr` | 2017 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  | 0 |
| `0x49bef6` | cSwitch | `swit` | 1012 | 0x41ea4d | 0x41ea2e | **0x459880** |  |  |
| `0x49c080` | cTapePlayer | `Tape` | 2026 | **0x45a72e** | 0x41ea2e | 0x41eb73 | 0x459db3 (button 2), 0x459fe8 (button 3, set by fill) |  |
| `0x49c18c` | cTextBox | `TxBx` | 2027 | **0x45af0a** | 0x41ea2e | 0x41eb73 | 0x45a7e8 |  |
| `0x49c218` | cTextLabel | `TxLb` | 2045 | 0x41ea4d | **0x45b611** | 0x41eb73 | 0x45b645 |  |
| `0x49c2e0` | cTextLine | `TxLn` | 2028 | 0x41ea4d | 0x41ea2e | 0x41eb73 | 0x45b6e0 |  |
| `0x49c398` | cTotaler | `Ttlr` | 2001 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |
| `0x49c494` | cTuningFork | `Fork` | 1008 | **0x45cca9** | 0x41ea2e | 0x41eb73 |  |  |
| `0x49c524` | cXorGate | `XorG` | 2011 | 0x41ea4d | 0x41ea2e | 0x41eb73 |  |  |

Notes on the table:

* "dialog" is the gadget id at `+0x28`, which is the `RT_DIALOG` id.
  The four number crunchers store 501 first (the `cNumberCruncher` init
  `0x44c778`) and then their own id.
* `cFirstThru` (`0x499a28`, tag `1thr`) and the class at `0x49a194`
  (tag `Nois`, resource 1003, fresh/load ctors `0x44bd8f`/`0x44bf35`) have
  complete vtables, but nothing references their constructors (no `call`
  and no stored pointer) and the factory has no case for them. They can't
  appear in a widget. The vtables are laid out in alphabetical class
  order, matching the RTTI string list, and `0x49a194` sits between
  cMultiplier and cNotGate. So it is most likely `cNoiseMaker`, as a
  concrete class. AnSd's and MsSd's constructors never install it, which
  agrees with sound-time.md §9/§14 ("no vtable of its own in the
  constructors").
* The gate base `0x499a88` has no RTTI string near it. Its place in the
  alphabetical vtable order (between cFirstThru and cGraphicProjector)
  fits a name like `cGate`.
* Only **13** classes do something dialog-specific: 11 slot 0x48
  overrides, 1 slot 0x4c override, and 5 push-button handlers (StCh,
  Tape, TxBx, TxLb and TxLn; StCh, Tape and TxBx also override slot
  0x48). Every other class is fully covered by the base fill and read
  back, plus its slot 0x50 for the preview.

---

## 3. Per-class specification

The notation `link(A → B)` means `0x417c7b(A, B)`: working control A
selects B (§1.3). `radio(g, k)` is button `k` of radio group `g`,
`combo(n)`/`edit(n)`/`check(n)` are the n-th control of that kind (§1.1),
with the template control id in brackets. Unless stated, the override
ends by calling the base fill `0x41ea4d`, and read-back is the base.

### 3.1 The "Preset / Custom" family

Each of these dialogs has a radio group "Preset:" (index 0) / "Custom:"
(index 1). A combo holds the presets and one or two edit fields hold the
custom value. The override links the combo to "Preset:" and the edit(s)
to "Custom:":

| class | fill | dialog | group "Preset/Custom" | links |
|---|---|---|---|---|
| cPendulum `Pend` | `0x44f393` | 1000 "Pendulum" | g0 = {106 "Preset:2", 114 "Custom:2"} (g1 = 113 "Period Length" / 117 "On at Center") | `combo(0)[107 length] → radio(0,0)`; `edit(0)[115 "5#"] → radio(0,1)` |
| cDropChamber `Drop` | `0x44706c` | 1001 "Gravity Chamber" | g0 = {108, 112} | `combo(1)[109 height] → radio(0,0)`; `edit(0)[113 "5#"] → radio(0,1)` |
| cPhysicsCanon `Cano` | `0x44fe21` | 1002 "Cannon" | g0 = {110 "Preset:1", 112 "Custom:1"} (g1 = output 116–119) | `combo(2)[111 height] → radio(0,0)`; `edit(0)[113 "#5 "] → radio(0,1)` |
| cRandomNumberGenerator `rand` | `0x450cdb` | 1007 | g0 = {107, 109} | `combo(0)[108] → radio(0,0)`; `edit(0)[111 "4#"] → radio(0,1)`; `edit(1)[113 "4#"] → radio(0,1)` |
| cTuningFork `Fork` | `0x45cca9` | 1008 | g0 = {106, 107} | `combo(0)[109] → radio(0,0)`; `edit(0)[110 "5#."] → radio(0,1)` |
| cSlider `Sldr` | `0x45262d` | 1015 | g0 = {107, 109} | `combo(0)[108] → radio(0,0)`; `edit(0)[111 "5"] → radio(0,1)`; `edit(1)[113 "5"] → radio(0,1)` |
| cStripChart `StCh` | `0x459615` | 2044 | g0 = {111, 112} | `combo(0)[113 length] → radio(0,0)`; `edit(2)[114 "3.#" hrs] → radio(0,1)` (edits 0/1 = From/To are not linked) |

Only the combo that sits next to the "Preset:" button is linked. In the
Pendulum, Gravity Chamber and Cannon dialogs, the other combos
(gravity, weight, thing, speed) are independent settings and select
nothing.

### 3.2 cStopper `Stop` (dialog 2043 "Win / Lose Checker"), fill `0x457e95`

Groups (tags 1–4): g0 = {105 "Win if input is:", 106 "Win / Lose if
input is:"}, g1 = {107 True, 108 False}, g2 = {109..114, the six
comparisons}, g3 = {117 Win sound, 118 Lose sound}. Edit 0 = 115 "this
number" (`"#.-6"`).

```
for k in 0..5: link(radio(2,k) → radio(0,1))   // a comparison selects "Win / Lose if input is:"
link(radio(1,0) → radio(0,0))                  // True selects "Win if input is:"
link(radio(1,1) → radio(0,0))                  // False likewise
link(edit(0)  → radio(0,1))                    // typing the number selects "Win / Lose if input is:"
base fill
```

The source is a radio *button*. Clicking it reaches the handler as its
**group** (§1.3), whose link is the link of the button that is now
selected, so this works for each button individually. The g3 buttons
have no link.

### 3.3 cSoundTransformer `SdTr` (dialog 2038), fill `0x4536ea`

Controls: check 0 = 108 "Change Pitch:", check 1 = 110 "Play the Sound
Backwards", g0 = {106 "Play with No Changes", 107 "Transform Sound:"},
combo 0 = 109 (pitch: Up a Third … Down an Octave).

```
link(combo(0) → check(0))          // focusing the pitch list ticks "Change Pitch:"
link(check(0) → radio(0,1))        // ...which in turn selects "Transform Sound:"
link(check(1) → radio(0,1))        // "Play the Sound Backwards" selects "Transform Sound:"
base fill
```

The chain is followed from the combo: focusing the pitch list checks
"Change Pitch:" **and** selects "Transform Sound:". Clicking either check
box selects "Transform Sound:" even when the click unchecks it. Nothing
unchecks the boxes when "Play with No Changes" is chosen.

### 3.4 cTextBox `TxBx` "Billboard" (dialog 2027)

* Slot 0x48 `0x45af0a` only calls the base fill (it behaves exactly like
  the base). `+0x34 = 1` (set by init `0x45a801`): one 240-char text, the
  multi-line edit 106 (`ES_CENTER|ES_MULTILINE|ES_WANTRETURN`, no
  filter).
* Push button 2 = 107 "Clear Text" → `0x45a7e8`: `0x41c728(dlg, 0, "")`.
* Read-back is the base: compare 239 chars, copy 240, `+0x13c = 0`.
  Soft line breaks become hard ones (§1.5).

### 3.5 cTextLabel `TxLb` "Label" (dialog 2045)

* Push button 2 = 109 "Clear Text" → `0x45b645`: `0x41c728(dlg, 0, "")`.
* **Slot 0x4c `0x45b611`** (OK):

  ```
  0x41e851(dlg, this)            // base read-back
  0x45af79(this)                 // re-measure the label: bounds from the text extent in the chosen font
  this.vtbl[0x1c](this)          // reset (base 0x41cef4)
  0x425316(DAT_004a2848)         // refresh the worksheet (same as the base 0x41ea2e)
  ```

  It differs from the base only in the re-measure and the reset, so the
  label's hit rectangle and selection frame follow the new text and size
  right away. The live preview (slot 0x50 = nop) does not re-measure,
  but the "Pic" preview draws through `0x45b31c`, which measures on every
  draw.

### 3.6 cTextLine `TxLn` "Message List" (dialog 2028)

* Push button 2 = 113 "Clear All Text" → `0x45b6e0`:
  `for n in 0..3: 0x41c728(dlg, n, "")`.
* No 0x48/0x4c override. Edits 106/108/110/112, filter `"33"` (max 33
  chars).

### 3.7 cStripChart `StCh` (dialog 2044)

* Links: §3.1.
* Push button 2 = 115 "Reset Number Range" → `0x458b74`:
  `0x41c728(dlg, 0, "0"); 0x41c728(dlg, 1, "100")` (the fresh From/To).
  It does not touch the length radio, combo or hours field.

---

## 4. cTapePlayer `Tape`: the Recorder (dialog 2026)

### 4.1 The dialog

Template 2026 "Recorder" (236×153 dialog units):

| id | control | what |
|---|---|---|
| 102 | owner-draw `"Pic"` | live gadget picture (`cDialogGadgetGraphic`) |
| 103 / 150 | owner-draw | Doc balloon / its page button |
| 104 | static | "Your recorded sound" |
| 106 | push, (19,110) 91×14 | **"Record a Sound"**: push button 2 |
| 107 | push, (124,110) 91×14 | **"Import .WAV file"**: push button 3 |
| 160 | owner-draw, (9,94) 217×8 | progress bar (§1.6) |
| 1 / 2 | push | OK / Cancel |

There are no settings controls, so the base fill and read-back do
nothing. Wiring:

* init `0x45a057`: `+0x2f = 0x459db3`, `+0x33 = 2` → Record.
* slot 0x48 `0x45a72e`: `0x41ea4d(this, dlg)` (which attaches Record),
  then `0x41ca72(dlg, 3, 0x459fe8)` → Import.

Both actions write **straight into the gadget's sample buffer**
`+0x19d`. Cancel (slot 0x50 = base nop) does not undo them, and OK's undo
snapshot (`0x4154b4`) is taken *after* them, so Undo doesn't either. The
preview hook runs after each button (§1.3). It reads nothing, calls the
nop slot 0x50 and repaints the Pic control, which shows the broken
picture if `+0x140` is set.

Gadget state involved (see sound-time.md §7): `+0x197` descriptor
`{u32 len = 40000, u16 rate = 11025, ptr data = +0x19d}`; `+0x19d` the
40000-byte buffer (u8 PCM, silence `0x80`); `+0x140` the broken flag.

### 4.2 Record a Sound: `0x459db3(dlg, gadget)`

```
g_dlg (DAT_004a33c4) = dlg                    // for the progress callback
tmp = malloc(40000)
gadget.broken (+0x140) = 0
try:
    if gadget.buf (+0x19d) == null or tmp == null: throw          // MFC exception, vtable 0x49e3b4
    SoundClose()                               // 0x44083d → 0x43f39f: waveOutReset, unprepare, waveOutClose
    n = record_once(tmp, 40000, progress)      // 0x43e170, blocking, §4.4
    progress(100)                              // bar full
    SoundOpen()                                // 0x440837 → 0x43f1a5: reopen the output (if sound is enabled)
    n16 = (u16)n
    if n16 == 0:
        SetFocus(push button 0 = OK)           // 0x41c1d9 + 0x417c89, so focus doesn't sit on the button about to be disabled
    elif n16 != 0xffff:
        gadget.rate (+0x19b) = 11025
        n16 = min(n16, 40000)
        memset(gadget.buf, 0x80, 40000)
        memcpy(gadget.buf, tmp, n16)
    free(tmp)
    if n16 == 0: throw
catch:                                         // handler at 0x459f9f (not decompiled by Ghidra)
    disable push button gadget+0x33 (= 2, "Record a Sound")      // 0x41c3ea(dlg, 2)
    gadget.broken (+0x140) = 1                 // the Recorder shows the broken picture
    AfxMessageBox("There was an error while recording sound", MB_OK)   // 0x45f99f; literal at 0x49c04a
```

`progress(pct, rem)` (`0x459d98`) is `0x41caca(g_dlg, pct); return 2`.

What this means for an implementation:

* **Length:** a recording is always one 40000-byte buffer, which is
  40000 samples at 11025 Hz = **3.628 s**. It ends only when the device
  reports 40000 bytes. There is no Stop button and no way to cancel: the
  callback always returns 2 ("go on"). The UI is frozen during the
  recording, except that the progress bar repaints synchronously (via
  `UpdateWindow`) on every poll.
* **Format stored:** unsigned 8-bit mono, 11025 Hz, exactly 40000
  bytes. The descriptor's length stays 40000. If fewer bytes come back
  (which can't happen with the loop as written), the rest is `0x80`.
  The rate is re-set to 11025 (it already is).
* **Levels / gain / monitoring:** none. There is no level meter, no
  gain, no playback after recording, and no trimming. The progress bar
  is the only feedback.
* **Errors:** if there is no input device, any `waveIn*` call fails, or
  allocation fails, the record button is disabled for the rest of this
  dialog, the gadget is marked broken, and the message box appears.
  `+0x140` stays set until a later successful record or import (each of
  which clears it). The buffer keeps its previous contents on error.
  (sound-time.md §7 says nothing sets `+0x140`. This catch handler
  does.)
* `n == 0xffff` (an aborted recording, -1 truncated) would keep the old
  buffer without an error. It can't happen because the callback never
  aborts.

### 4.3 Import .WAV file: `0x459fe8(dlg, gadget)`

```
name = CString()
if import_wav(gadget.buf, 40000, &name, dlg.hwnd) (0x43deda): gadget.broken = 0
```

`import_wav(buf, n, &name, owner)` `0x43deda`:

```
if !open_file_dialog(&name, title "Import from Wave format file" (0x4969f9),
                     filter "Wave files (*.wav)\0*.wav\0" (0x496a16), defext "WAV" (0x496a30), owner): return 0
f = fopen(name, "rb")                                              // 0x47e248
if !f: AfxMessageBox(name + "\nCan`t open file"); return 0         // 0x496a37 (the backtick is in the binary)
h = fread 44 bytes
if got != 44 or h[0..4] != "RIFF" or h[8..12] != "WAVE" or h[12..16] != "fmt ":
    fclose; AfxMessageBox(name + "\nBad file format"); return 0     // 0x496a48
if u16 h[0x22] (bits) != 8:
    fclose; AfxMessageBox(name + "\nCan't import non-8 bit sound"); return 0   // 0x496a59
ch = u16 h[0x16]; rate = u32 h[0x18]
skip = (rate >= 22000) + (rate >= 30000) + (rate >= 44000)
i = 0
while i < n:
    b = fgetc(f); if ch == 2: b = fgetc(f)       // stereo: the second (right) byte of the frame
    if b == EOF: break
    repeat skip times:                           // decimate: keep the LAST frame of each group of skip+1
        b = fgetc(f); if ch == 2: b = fgetc(f)
        if b == EOF: goto done
    buf[i++] = b
done:
memset(buf + i, 0x80, n - i)
return 1                                         // f is never closed on success (a handle leak)
```

* Sample data is assumed to start at byte 44. There is no chunk walk, so
  a `fact`/`LIST` chunk before `data`, or a `fmt ` chunk bigger than 16
  bytes, is read as samples.
* The conversions are 22050 → 1/2, 32000 → 1/3 and 44100/48000 → 1/4.
  Any other rate is copied 1:1 and plays at 11025 Hz. The descriptor rate
  is not changed. Mono and stereo are supported, and a channel count other
  than 2 is treated as mono.
* The open-file dialog (`0x43dd54`) uses `GetOpenFileNameA` with flags
  `OFN_HIDEREADONLY|OFN_PATHMUSTEXIST|OFN_FILEMUSTEXIST` (`0x1804`). Its
  initial directory is a static (`DAT_004a2d90`) that is updated to the
  chosen file's drive and directory, so the next dialog of the same kind
  starts there.
* A cancelled file dialog returns 0 and changes nothing (`+0x140` keeps
  its value).

### 4.4 How the original reaches the microphone

All in `WIDGET.EXE`, winmm, with no callbacks (`CALLBACK_NULL`) and
busy polling.

**Format** (`0x496a78`, and an identical copy at `0x496a8c` for
listening): `WAVEFORMAT {wFormatTag 1 (PCM), nChannels 1,
nSamplesPerSec 11025, nAvgBytesPerSec 11025, nBlockAlign 1}` +
`wBitsPerSample 8` (16 bytes copied, so there is no `cbSize`).

**One-shot recording**, `record_once(buf, n, cb)` `0x43e170`
(Recorder):

```
if n == 0: return 0
was_open = SoundIsOpen() (0x43f8b8, DAT_00496c50)
if waveInGetNumDevs() == 0: return 0
if was_open: SoundClose() (0x43f39f)              // the output device is closed while recording
waveInOpen(&h, WAVE_MAPPER (-1), &fmt, 0, 0, CALLBACK_NULL)        → fail: goto reopen
hdr = WAVEHDR{lpData buf, dwBufferLength n, dwBytesRecorded 0,
              dwFlags WHDR_BEGINLOOP|WHDR_ENDLOOP (0xc), dwLoops 1, lpNext 0, reserved 0}
memset(buf, 0x80, n)
waveInPrepareHeader(h, &hdr, 32)                  → fail: close
waveInAddBuffer(h, &hdr, 32); waveInStart(h)      → fail: unprepare, close
r = 2
loop:
    mmt.wType = TIME_BYTES (4); waveInGetPosition(h, &mmt, 12)
    if mmt.cb >= n: break
    if cb: r = cb(mmt.cb * 105 / n, mmt.cb * 105 % n); if r != 2: break
waveInStop(h)
result = (r == 0 or r == 2) ? mmt.cb : -1
waveInUnprepareHeader(h, &hdr, 32)
waveInClose(h)
reopen: SoundOpen() (0x43f1a5)
return result                                     // 0 if any call failed
```

The progress value goes from 0 to 105, not 100. The bar simply looks
full from 100 on, and the caller then sets it to 100. Note that
`waveInGetPosition`'s return value is not checked: a driver that never
advances the position hangs the program.

**Continuous listening** (Sound Switch `SdSw`, run mode; behaviour in
sound-time.md §11):

* `listen_start(buf, n)` `0x43e2db`: `memset(buf, 0x80, n)`. If already
  listening (`DAT_00496a88 != 0`): `waveInStop; waveInStart`, return 1.
  Otherwise, as above: close the output if open, `waveInOpen(WAVE_MAPPER,
  fmt 0x496a8c, CALLBACK_NULL)`, one global header `DAT_004a2e98`
  (`dwFlags 0xc`, `dwLoops 1`), prepare, add buffer, start. It returns 1,
  or 0 on any failure.
* `listen_pos()` `0x43e446`: `waveInGetPosition(TIME_BYTES)`, -1 on
  error.
* `listen_stop()` `0x43e411`: `waveInStop`, `waveInUnprepareHeader`,
  `waveInClose`, `SoundOpen()`, handle = 0.
* The Sound Switch uses a 22000-byte buffer (about 2 s), restarts it
  after 19800 bytes, and compares `|sample - 128|` with its threshold.

A port only needs: one mono 8-bit 11025 Hz capture of a fixed length
with a progress readout (Recorder); and a rolling capture with a
"samples so far" position (Sound Switch). The original closes its
output device during capture because many 1995 sound cards were
half-duplex. A modern port does not have to.

---

## 5. What the port needs to add

1. **Links** (§1.2–1.3, §3.1–3.3): a per-control optional target. In
   `Dialog::click` and the edit/combo handlers, follow the chain:
   radio/check targets get checked (radio targets also uncheck their
   group). Trigger on button clicks, on edit text changes, and on a combo
   being focused or opened. Build the links per gadget class from the
   tables in §3 (using control kind + index → template control).
2. **Push-button handlers** (§1.4): map
   `(gadget class, push-button index)` → action. Clear/Reset set edit
   texts and then behave like an edit change (validation, links,
   preview).
3. **Validation** (§1.5): replace the per-keystroke character filter
   with "apply, validate the whole text, revert if invalid", including
   the `%d`/`%lf` rules, the `A` flag and the max length.
4. **TxLb OK** (§3.5): after reading back, re-measure the label's
   bounds.
5. **Recorder** (§4): Record (fixed 40000-sample capture with the
   progress bar, error → disable the button + broken flag + message),
   Import (the WAV reader above), both writing to the gadget
   immediately and surviving Cancel.
6. **Progress bar** control 160 (§1.6).

---

## 6. Open questions

* **`last_good` of an edit field** (`cEditText+0x114`) is not set
  before the first valid change. `0x419c1a` doesn't initialise it, and
  the fill's `WM_SETTEXT` arrives before `ready`. If the first change
  after opening is invalid, the field reverts to whatever `new`
  (`0x4828e0`) left there (probably empty or garbage). Not tested on
  real Windows.
* **The catch handler of `0x459db3`** (`0x459f9f`) was read from raw
  disassembly. Ghidra didn't make it a function. Its first call,
  `0x46961f([ebp-4])`, is assumed to be the MFC exception cleanup. Which
  exception types it catches (the throws use the class with vtable
  `0x49e3b4`, probably `CUserException`) was not checked against the
  Borland EH tables.
* **Dirty flag:** neither OK nor the Recorder's actions were seen setting
  the document's modified flag (editor-interaction.md §6.2 says the same
  for OK). If that's true, recording into a saved widget and closing
  wouldn't prompt to save. This was not verified in the running program.
* **`dlg+0x9c` after a dialog**: the progress bar keeps its value only
  for the lifetime of the dialog object (`0x41baac` zeroes it), so every
  opening starts yellow. This was read from the constructor, not
  observed.
* Whether Windows sends `CBN_SETFOCUS` when a combo's drop-down is
  opened with the mouse while another control has the focus. It should,
  but the link chain depends on it.
* The `0x44081d` call at the top of `OnCommand` services the sound queue
  (`0x43f799`). It matters only for sounds already playing.
* The RTTI name of the gate base `0x499a88` (logic.md asks the same).

---

## 7. Function index

| address | name |
|---|---|
| 0x41acb9 | `cDialog::OnInitDialog` (builds the lists, calls slot 0x48, posts 0xfd) |
| 0x41a686 | `cDialog::OnCommand` (§1.3) |
| 0x41a58a | `cDialog::FindControl(id)` |
| 0x41a912 | `cDialog::OnDrawItem` (102 Pic, 103/150 Doc, 151 Doc face, 160 progress) |
| 0x41baac | `cDialog::cDialog(gadget, hook)` |
| 0x41c23b / 0x41c25b / 0x41c1f9 / 0x41c27b / 0x41c1d9 | `RadioGroup(n)` / `Combo(n)` / `Edit(n)` / `Check(n)` / `PushButton(n)` |
| 0x41a396 | `cRadioGroup::Button(k)` |
| 0x41a0be | `cRadioGroup::Select(id or -1 = resync)` |
| 0x41a2ae / 0x41a2a2 | `cRadioGroup::SetIndex` / `GetIndex` |
| 0x41a3b6 | `cRadioGroup::GetLink` (selected button's link) |
| 0x417c7b / 0x417c70 | `cDialogControl::SetLink` / `GetLink` |
| 0x417c52 / 0x417bf0 | `cDialogControl::Enable` / `Disable` |
| 0x417de0 / 0x417dc3 | `cButton::SetHandler` / `Run` |
| 0x41ca72 | `cDialog::SetButtonHandler(n, fn)` |
| 0x41c3ea | `cDialog::DisableButton(n)` |
| 0x41c728 | `cDialog::SetEditText(n, s)` |
| 0x41caca | `cDialog::SetProgress(pct)` |
| 0x41c660 | `cDialog::RefreshGadgetPic` |
| 0x419c1a | `cEditText::cEditText` (parses the filter) |
| 0x419ba9 / 0x419ad8 | `cEditText::Validate` / `IsValid` |
| 0x419ab1 / 0x419a59 / 0x4199e1 / 0x419828 / 0x419904 | validators: alnum / uint / int / real / signed real |
| 0x41a49b / 0x41a44e | `cTextControl::GetText` (EM_FMTLINES) / collapse `\r\r` |
| 0x41ea4d / 0x41ea2e / 0x41e851 / 0x41eb78 | `cGadget::FillDialog` / `DialogOK` / `ReadDialog` / `DialogPreview` |
| 0x44f393 / 0x44706c / 0x44fe21 / 0x450cdb / 0x45cca9 / 0x45262d / 0x459615 | Preset/Custom fills (Pend / Drop / Cano / rand / Fork / Sldr / StCh) |
| 0x457e95 | `cStopper::FillDialog` |
| 0x4536ea | `cSoundTransformer::FillDialog` |
| 0x45af0a | `cTextBox::FillDialog` (base only) |
| 0x45b611 | `cTextLabel::DialogOK` |
| 0x45a7e8 / 0x45b645 / 0x45b6e0 / 0x458b74 | Clear Text (TxBx) / Clear Text (TxLb) / Clear All Text (TxLn) / Reset Number Range (StCh) |
| 0x45a72e | `cTapePlayer::FillDialog` (attaches Import) |
| 0x459db3 / 0x459d98 | `cTapePlayer::Record` / its progress callback |
| 0x459f9f | `cTapePlayer::Record` catch handler |
| 0x459fe8 / 0x43deda / 0x43dd54 | `cTapePlayer::Import` / `ImportWav` / `OpenFileDialog` |
| 0x43e170 | `RecordOnce` (waveIn, blocking) |
| 0x43e2db / 0x43e446 / 0x43e411 | `ListenStart` / `ListenPos` / `ListenStop` |
| 0x43f39f / 0x43f1a5 / 0x43f8b8 | `SoundClose` / `SoundOpen` / `SoundIsOpen` |
| 0x45f99f | `AfxMessageBox` (through the app's vtable slot 0x6c) |
