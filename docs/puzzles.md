# Puzzles

How `WIDGET.EXE` (Win95 build) stores, lists, loads, restricts and scores
the 25 built-in puzzles. Addresses are in `WIDGET.EXE`. Read
[gadget-model.md](gadget-model.md) first.

The short version:

- A puzzle is an ordinary saved widget (`FORM Gadf`, the same format as a
  `.WGT` file) stored as a `Puzl` resource in `WIDGET.PZL`. The resource
  ID is the puzzle number and the resource name is its title.
- The puzzle's instructions are text gadgets (`TxLb`, `TxBx`) placed on the
  worksheet itself. There is no separate instructions resource.
- Restrictions come from two things a normal widget can also hold: a
  **lock** byte on each gadget and on each wire, and a **`numG` chunk** that
  limits how many of each part the tray hands out.
- The game decides a puzzle is solved. Each puzzle contains a
  "Win / Lose Checker" gadget (`Stop`, `cStopper`). When its condition
  holds, it plays an animation and stops the widget. If it is set to "Win",
  it also marks the current puzzle as solved.
- Solved flags are stored in two places: the `Done` resource inside
  `WIDGET.PZL` (written back when the file is writable) and the `Done=` key
  of `%WINDIR%\WIDGET.INI`. On reading, the two are ORed together.

---

## 1. `WIDGET.PZL`

`WIDGET.PZL` (242671 bytes) is a classic Mac resource fork stored as a
plain file. All numbers are big-endian.

### 1.1 Header (file offset 0)

| off | value | what |
|-----|-------|------|
| 0x00 | `0x00000100` | data area offset |
| 0x04 | `0x0003b0fc` | map offset |
| 0x08 | `0x0003affc` | data area length |
| 0x0c | `0x000002f3` | map length |
| 0x10..0xff | junk (`kiknpstv...`, `\x17Widget Workshop`...) | reserved/app area, ignored |

### 1.2 Map (file offset 0x3b0fc, 0x2f3 bytes)

| map off | value | what |
|---------|-------|------|
| 0x00 | copy of the 16-byte header | |
| 0x10 | `0028b748 0fcc` | handle/refnum placeholders (ignored) |
| 0x16 | `0x0000` | map attributes |
| 0x18 | `0x001c` | type list offset (from map start) |
| 0x1a | `0x017a` | name list offset (from map start) |
| 0x1c | `0x0002` | number of types − 1 (3 types) |
| 0x1e | type list entries, 8 bytes each: `type[4], count−1 (u16), ref list offset (u16, from the type list at map+0x1c)` |

| type | count | ref list at (type list +) |
|------|-------|---------------------------|
| `TMPL` | 1 | 0x1a |
| `Puzl` | 25 | 0x26 |
| `Done` | 1 | 0x152 |

Each reference-list entry is 12 bytes:
`id (i16), name offset (u16, from the name list; 0xffff = no name), attributes (u8), data offset (u24, from the data area), handle (u32, ignored)`.
Resource data at `data_off + offset` is `u32 length` followed by the bytes.
Names are Pascal strings (length byte + Mac Roman text; `0xD5` is the curly
apostrophe ’).

### 1.3 How the game reads it (its own minimal Resource Manager)

The EXE does not use any Mac API. Its reader (`0x413e78` to `0x414781`)
uses one global open file:

- `0x413e78 OpenResFile(path)` checks `access(path, 6)` (read and write).
  If that succeeds it opens the file with `fopen("r+b")` and sets the
  global `g_resWritable` (`DAT_00490834`) to 1. Otherwise it uses `"rb"`.
  It reads the first u32. If that is not `0x100`, it assumes a 128-byte
  MacBinary header and uses base `0x80`, otherwise base 0 (`DAT_00490824`).
  It reads the map offset at `base+4`. It sets the type list start to
  `map + base + 0x1c` (it ignores the stored type-list offset) and reads the
  type count and all 8-byte type entries into memory. It then computes the
  name list start as
  `typelist + last_type.ref_off + (last_type.count−1+1) * 12`, which assumes
  the name list follows the last reference list. It ignores the stored
  name-list offset too. Both shortcuts hold for the shipped file.
- Data offsets are always taken as `base + 0x100 + offset`. The stored
  data-area offset is not used.
- `0x4140d2 CloseResFile()` closes the file and frees the type table.
- `0x414102 GetResource(type, id, u32 *outSize, char *outName)` finds the
  type, reads its reference list, and finds the entry by **id**. It
  optionally copies the name (as a C string) into `outName`. It returns a
  `malloc`'d copy of the data and stores the length in `*outSize`. It
  returns NULL if the type or id is not found.
- `0x414492 WriteResource(type, id, buf)`: only if `g_resWritable`. It
  finds the entry, reads the stored length, and `fwrite`s that many bytes
  of `buf` over the resource data in place. The size never changes.
- `0x414729 CountResources(type)` returns the stored *count − 1*. Callers
  add 1.
- `0x414781 GetResourceName(type, id, char *out)` returns the name as a
  C string (an empty string if there is no name).

Type codes are compared as 4 raw bytes (`'Puzl'` appears in the code as
`0x6c7a7550`, `'Done'` as `0x656e6f44`).

### 1.4 `TMPL` 128 "Done" (50 bytes)

This is a ResEdit template for the `Done` resource and is not used by the
code:

```
"Puzzle Count"       OCNT
                     LSTC
"Puzzle Completed?"  BOOL
                     LSTE
```

### 1.5 `Done` 1 (no name, 52 bytes)

```
u16  count            = 0x0001 in the shipped file (stale; the code ignores it)
u16  flag[25]         Mac BOOL: 0x0100 = solved, 0x0000 = not; any non-zero
                      value is read as solved
```

The code takes the number of flags as `size/2 − 1` (= 25), not from the
count word. Puzzle *N* (1-based) is the u16 at byte offset `2*N` of the
resource, which is `flag[N−1]`. The shipped file has every flag at 0.

### 1.6 `Puzl` 1..25: one saved widget each

Each resource's data is a complete `FORM Gadf` file, byte for byte what
`CWidgetDoc`'s loader `0x42503e` accepts (see
`crates/formats/src/wgt.rs`). All 25 are `fver` 4 and `back` 0. The
resource name is the title shown in the list. The resource ID is the
puzzle number (`cPuzzle +0x12`) and the list position.

Puzzle-specific content inside the FORM:

- **Gadget lock byte**: byte `0x110` of every gadget chunk. The gadget
  model doc calls it "selected?". It is loaded into `cGadget +0x0c`
  (`0x41d166`) and saved from there (`0x41d396`). It is a **lock flag**
  (getter `0x41ed78`, setter `0x41ed83`), see section 3. Bytes
  `0x111..0x14f` of the base chunk are uninitialised padding (`0xa3` fill
  in the shipped files).
- **Wire lock byte**: byte 12 of a `conn` chunk (format 3 and later; the
  `flag` field in `wgt.rs`). It is loaded into `cConnection +0x25`
  (getter `0x43c8a4`, setter `0x43c8af`). It is the wire's lock flag.
- **`numG` chunk** (the tray limits, section 2.6). The loader compares the
  chunk tag as a little-endian int `0x476d756e`. That is where the name
  "`Gmun`" in `wgt.rs`/the task notes comes from, but **the bytes in the
  file are `n u m G`**, the same way `conn` is `0x6e6e6f63` and `back` is
  `0x6b636162` in the code. Layout:

  ```
  u16 n                       number of entries
  n × { char tag[4];          gadget class tag, e.g. "swit"
        i16  count; }         parts left in the tray; -1 = unlimited
  ```

  Every puzzle's `numG` has 64 entries, one per tray part. Most counts are 0.
- **Win / Lose Checker** (`Stop`) gadget: every puzzle has exactly one.
- **Instruction text**: `TxLb` labels (and in some puzzles `TxBx`) placed
  on the worksheet, all locked.

There is no puzzle-number chunk. The puzzle number exists only in memory
(`cPuzzle +0x12`) and is never written into a saved widget.

### 1.7 The puzzles

`ghidra/pzl_dump.py` prints all of this (`-v` lists every gadget, wire and
`numG` entry, `-n N` limits the output to one puzzle). Its run on the
shipped file:

```
header: data_off=0x100 map_off=0x3b0fc data_len=0x3affc map_len=0x2f3
map: attrs=0x0 typelist_off=0x1c namelist_off=0x17a
type TMPL: 1 resource(s), refs at typelist+0x1a
type Puzl: 25 resource(s), refs at typelist+0x26
type Done: 1 resource(s), refs at typelist+0x152
Done 1 (52 bytes): count word=1, 25 flags: 0000000000000000000000000
```

In the table, "Checker" is the `Stop` gadget's condition on its input knot
0. "Tray" lists the `numG` entries that are not 0; all others are 0 (no
puzzle uses −1/unlimited). "Free parts" are the gadgets whose lock byte is
0. Every other gadget, and almost every wire, is locked.

| # | Title | Gadgets (locked) | Wires (locked) | Checker | Tray | Free parts | What the text says |
|---|-------|------|------|---------|------|------------|------|
| 1 | White Color | 17 (17) | 5 (5) | input is True | – | – | Did mixing red, green and blue give white? (switch the colors on) |
| 2 | Turn It On | 14 (14) | 10 (10) | input is True | AndG 1, XorG 1, swit 4 | – | Turn on the light with only four switches |
| 3 | The Millionaire | 21 (21) | 11 (10) | input == 20 | – | – | $1 doubled daily: how many days to a million? |
| 4 | Lightbulbs | 16 (16) | 11 (11) | input is True | DClk, Wtch, Metr, swit, Key_, MsCl, MsMv, DnPl, hart 1 each; Spl2/3/4 2 each | – | Light all the bulbs with four different switches |
| 5 | Guess the Note | 43 (28) | 38 (22) | input == 10 | – | Fork×3 Comp×2 MiCo Spl2×2 AndG×3 OrGa Spkr×3 | Name the played note; get 10 right |
| 6 | More Lightbulbs | 34 (34) | 29 (29) | input is True | – | – | A mess of switches: turn on the light |
| 7 | Doc’s Diabolical Adder | 33 (33) | 28 (28) | input == 5 | – | – | Type sums; get 5 right |
| 8 | Count To Ten! | 12 (8) | 2 (2) | input == 1 | – | Wtch Key_ Cntr Metr | Build a widget that counts to ten |
| 9 | Barking Dogs | 20 (20) | 9 (9) | input is True | – | – | Fix the connections: 30 barks in under a minute |
| 10 | Wake the Chickens | 18 (18) | 8 (8) | input is True | disp, Metr, MiCo, Ttlr, XorG, swit, Key_, MsCl, MsMv, DnPl, hart 1 each; Spl2/3/4 2 each | – | Rooster must crow 40 times in one minute |
| 11 | Clues | 23 (23) | 20 (20) | input is True | cnst 1, MiCo 2, Mulr 1, swit 1, MsMv 1 | – | Chain of clues, finally enter 746 |
| 12 | Still More Lightbulbs | 57 (57) | 45 (45) | input == 1 | – | – | Rewired lab: make lights 1–4 follow switches 1–4 |
| 13 | Moon Baseball | 22 (22) | 7 (6) | input == 195 | Metr, swit, Key_, MsCl, MsMv, DnPl, Mplx, Spl2, Spl3, Spl4, Pend, Cano, hart 1 each | – | How far can a ball be thrown on the moon |
| 14 | Marquee | 44 (43) | 22 (21) | input == 5 | – | Cntr | Reconnect so the lights cycle once through |
| 15 | Mad Multiplier | 28 (28) | 14 (14) | input == 1 | disp 1, MiCo 2, NotG 1, Key_ 1, Spl3 1, hart 1 | – | 6 right products in a minute to switch it off |
| 16 | Heads or Tails | 19 (19) | 9 (9) | input == 100 | Coin 1, Comp 2, Spl2 1 | – | Flip a coin 100 times, count heads |
| 17 | Gravity Chamber Test | 22 (22) | 10 (10) | input is True | XorG, NotG, swit, Key_, MsCl, MsMv, DnPl, Mplx, Spl2, Spl3, hart 1 each; Spl4 2 | – | Light one bulb while the elephant falls, one on impact |
| 18 | Fast Counter | 17 (14) | 1 (1) | input > 300 | – | Mplx MiCo Spl2 | Three pieces broke off a counter: count fast to 300 |
| 19 | Elephant Fall | 11 (10) | 3 (3) | input >= 1 | Wtch 1, cnst 3, MsCl 1, Spl2 1, Drop 1 | cnst | Time for an elephant to fall 1300 ft on the moon |
| 20 | Beat the Clock | 27 (27) | 21 (21) | input is True | – | – | Click, move the mouse, make the cow moo within 23 s |
| 21 | Bank Vault | 45 (20) | 39 (27) | input is True | – | 25 logic parts (AndG, XorG, NotG, Mplx, MiCo, Spl2, Spl3) | Find three 2-key combinations |
| 22 | Projector Puzzle | 16 (16) | 6 (6) | input == 7 | Sldr 1 | – | Add a slider; which sign is the lion? |
| 23 | How Many Earths | 14 (14) | 5 (5) | input == 55 | MiCo 2, Divr 3, ArVo 2, Plnt 2 | – | How many Earths fit in Neptune |
| 24 | Doc’s Animal Clock | 36 (36) | 3 (3) | input is True | – | – | Rebuild the clock: a sound every 15 s |
| 25 | A Rose is a Rose | 33 (33) | 20 (20) | input == 5 | – | – | Finish the automatic five-sentence poet |

All checkers are set to "Win". None uses "Lose".

---

## 2. How the game uses it

### 2.1 Objects and globals

| global | what |
|--------|------|
| `DAT_004a2858` | `cPuzzle *g_puzzle`, created at startup by `cPuzzleInit` (`0x42d5dd`, run from the static-init table at `0x4a20f8`) and destroyed by `0x42d642` |
| `DAT_004a2848` | the document (`CWidgetDoc`; owns gadget list `+0x10`, wire list `+0x1a`, selection `+0x35/+0x3b/+0x3f`, author mode `+0x55`) |
| `DAT_004a2864` | the parts tray window (7 drawers at `+0xa98`, 0x1c bytes each) |
| `DAT_004a270c` | the file-state object (name `+0`, modified `+0x100`, untitled/imported `+0x101`, has-name `+0x102`, alternate name `+0x107`, puzzle name `+0x207`) |
| `DAT_00493dde` | 1 = design mode, 0 = run mode |

`cPuzzle` (size 0x115, vtable `0x493dbc`, which holds only the destructor
`0x42d487`). It is built by `0x42d3fc`:

| off | type | what |
|-----|------|------|
| +0x00 | ptr | vtable |
| +0x06, +0x0a, +0x0e | u32 | set to 0, never used |
| +0x12 | i16 | current puzzle number (resource ID) |
| +0x14 | u8 | **puzzle active**: the worksheet holds an unsaved puzzle |
| +0x15 | char[256] | path of the puzzle file: `_splitpath(argv[0])` drive and dir, then `_makepath(.., "widget", ".pzl")`. That is the EXE's folder plus `widget.pzl`. |

### 2.2 "Load Puzzle..." (File menu, command `0x8003`)

The menu item comes from menu resource 3 (`&File`): `Load Puzzle...` =
`0x8003`, `Revert` = `0x8004`. The message map entry at `0x494670` sends
it to `0x4332b6`, which calls **`0x416f1a FileLoadPuzzle(file)`**:

1. If the document is modified (`file+0x100`) **and no puzzle is active**,
   it asks to save (`0x410b24`, Yes/No/Cancel). Cancel (2) aborts. Yes (6)
   runs Save (`0x415a3b`) and ignores its result. An active puzzle's
   changes are discarded without asking.
2. `0x42d570 cPuzzle::Choose(&data, &size)` calls
   `0x42d284 RunPuzzleDialog(path, &data, &size)`, stores the returned
   number in `+0x12`, and sets `+0x14 = 1` if it is non-zero. If it is 0
   (cancelled or error), nothing else happens.
3. It clears the document (`0x424b27`) and resets the tray: every slot's
   count is set to −1, unlimited (`0x4329e0`).
4. It copies `"puzzle"` into `file+0x207` (`0x42d5b3 cPuzzle::GetDocName`
   ignores the number) and updates the title (`0x417092`).
5. It loads the FORM: `0x42503e(doc, data, size, 0)`. Gadgets keep their
   lock bytes, wires keep theirs, and `numG` sets the tray counts. It then
   frees the data.
6. It switches the tray to its normal view (`0x4325e0(tray, 0)`) and sets
   `file+0x102 = 1` and `file+0x101 = 0`. The modified flag is not cleared
   here.

### 2.3 The dialog (`cPuzzleDlg`)

`0x42d284 RunPuzzleDialog(path, void **outData, u32 *outSize)`:

- Opens the PZL (`0x413e78`). On failure it shows alert 8005 (`0x410b5b`
  shows string `8000+n`): *"Cannot find puzzle file. It should be in the
  same folder as Widget Workshop and be named WIDGET.PZL."* and returns 0.
- Builds a `CDialog` on the stack with template **10000** (`0x466afe(this,
  10000, NULL)`), then sets the vtable to `0x493c76`. Fields:
  `+0x44` i16 chosen number, `+0x46` i32 count (`CountResources('Puzl')+1`
  = 25), `+0x4a` path, `+0x4e` `outData`, `+0x52` `outSize`.
- Runs `DoModal` (vtable `+0x70`), closes the PZL, and returns `+0x44` if
  the result was IDOK (1), otherwise 0.

Dialog template 10000 (`ghidra/res/WIDGET/5/10000.bin`), caption
"Select a puzzle ", MS Sans Serif 8, at 57,16 and 162×203 DLUs:

| id | control | text | rect (x,y,w,h) |
|----|---------|------|----------------|
| 107 (`0x6b`) | LISTBOX, `LBS_NOTIFY`, `WS_VSCROLL`, `WS_BORDER`, not sorted | | 7,7,149,148 |
| 2 | button | Cancel | 111,161,40,14 |
| 1 | default button | Ok | 111,182,40,14 |
| 105 (`0x69`) | button | Reset Completed Puzzles | 9,182,93,14 |

`OnInitDialog` (`0x42cef8`, vtable `+0x74`):

- Reads the solved flags (`0x42d82d`, section 2.5).
- For id = 1..count, the list line is a 0x102-byte buffer that starts as
  `"  "` (template `0x493af4`), followed by the resource name from
  `GetResourceName('Puzl', id)` at offset 2. Every `0xD5` byte (Mac ’) is
  replaced by `'` (`0x42ced8`). If flag[id] is set, the first character
  becomes `'+'`. Lines are added with `LB_ADDSTRING`, so line *i* is puzzle
  *i+1*, for example `"+ Count To Ten!"` and `"  Lightbulbs"`.
- Selects line 0 (`LB_SETCURSEL 0`).
- If no puzzle is solved, it disables "Reset Completed Puzzles".

`OnCommand` (`0x42d01e`, vtable `+0x48`):

- **IDOK (1)**: `sel = LB_GETCURSEL`. If `sel == −1`, it ends with 2.
  Otherwise it sets `+0x44 = sel+1`, sets
  `*outData = GetResource('Puzl', sel+1, outSize)`, and ends with 1.
- **IDCANCEL (2)**: ends with 2.
- **0x69 Reset**: asks *"Do you really want to mark all puzzles as
  unsolved?\nThis is a permanent change\n"* (`0x45f99f(msg, 1, 0)`, Yes
  = 1). On Yes it reads the flags, zeroes everything after the count word,
  and writes them back (`0x42d915`, which updates both the `Done` resource
  and the INI). For every list line whose first character is not `' '`,
  it sets that character to `' '` and replaces the line
  (`LB_GETTEXT`/`LB_INSERTSTRING`/`LB_DELETESTRING`). It then restores the
  selection and disables the button.
- **0x6b with `LBN_DBLCLK` (2)**: posts `WM_COMMAND IDOK`.

### 2.4 Solved: who decides

The game decides, through the **Win / Lose Checker** (`Stop`,
`cStopper`), vtable `0x49bc00`. Its gadget resource id is 2043 (`0x7fb`),
its picture/sound resource is 1051 (`0x41b`), and its settings dialog is
2043 ("Win / Lose Checker").

Settings (standard settings block, see gadget-model.md):

| block off | gadget off | dialog group | meaning |
|-----------|-----------|--------------|---------|
| +0x04 | +0x39 | 1 | 0 = "Win if input is" (boolean), 1 = "Win / Lose if input is" (numeric) |
| +0x05 | +0x3a | 2 | boolean mode: 0 = True, 1 = False |
| +0x06 | +0x3b | 3 | numeric mode: 0 equal to, 1 not equal to, 2 greater than, 3 less than, 4 greater than or equal to, 5 less than or equal to |
| +0x07 | +0x3c | 4 | 0 = "Win sound", 1 = "Lose sound" (also decides whether the puzzle is marked solved) |
| +0x18 | +0x4d | text 0 | "this number:" (a fresh gadget gets `"1"`) |

Class state: `+0x141` u8 animation frame, `+0x146` f64 compare value,
`+0x197` u8 triggered, `+0x198` u32 trigger time (ticks).

- **start** (slot 0x20, `0x457b77`): base start, then
  `sscanf(text0, "%lf", &+0x146)`.
- **reset** (slot 0x1c, `0x4578b0`): base reset and `+0x197 = 0`.
- **tick** (slot 0x44, `0x457ba2`), each idle loop in run mode:
  1. **Cheat**: if the mouse cursor is at screen (0,0) (`GetCursorPos`) and
     the **W, I and N** keys are all held (`GetKeyState < 0`), it sets
     `+0x197 = 1`.
  2. `was = +0x197`. If input knot 0 has no wire, it stops here and does
     nothing more, even if the cheat fired.
  3. If not triggered: in boolean mode it reads the source's boolean
     through the wire (`0x43adec`: the bool output, else number ≠ 0, else
     object ≠ null) and triggers when `value == (group2 == True)`. In
     numeric mode it reads input 0 (`0x41e56e`) and triggers on
     `value OP +0x146`.
  4. If it was already triggered when the tick began (`+0x197` set on an
     earlier tick, or by the cheat on this one), it runs a **blocking
     animation loop**. Each pass it services sound (`0x44081d`), reads the
     time `t` (`0x4113a8`, Mac ticks), and if
     `frame*25 < t − +0x198` it advances `frame` and invalidates the
     gadget. When `frame` reaches 8 it sets `frame = 0` and
     `+0x197 = 0`, and then:
     - if **Win** (`+0x3c == 0`): `0x42d4b1 cPuzzle::MarkSolved(g_puzzle)`;
     - always: `0x430ee7` stops the widget (`DAT_00493dde = 1`, design
       mode, plays sound `0x8f`), and plays UI sound `0x80`.

     After each pass it redraws the gadget and the worksheet
     (`0x4234b8(doc, 1)`). The loop runs until `+0x197` clears, about
     7×25 ticks (≈2.9 s). Other gadgets do not tick during it.
  5. If it just triggered (`+0x197` set and `was == 0`), it records
     `+0x198 = now`, plays sound **0x47f** (1151) for Lose or **0x41b**
     (1051) for Win, sets `frame = 1`, and invalidates. The animation
     starts on the next tick.
- **draw** (slot 0x04, `0x4578c8`): it draws frame 1 of sheet 1051 as the
  body, then frame 2 when `+0x141 == 0`, else frame `+0x141 + 2`.

`0x42d4b1 cPuzzle::MarkSolved(this)` does nothing unless `+0x14` (puzzle
active) is set. Otherwise it opens the PZL, reads the flags (`0x42d82d`),
sets `buf[2*number] = 0x0100`, writes them back (`0x42d915`), and closes
the file. A Win checker in an ordinary widget, or in a puzzle that has
been saved under a name, marks nothing.

The string `"Done puzzle %hi\n"` (`0x493dc6`) has no references. It is a
leftover debug format.

### 2.5 Where solved state lives

There are two stores, and both are always written:

1. The `Done` resource in `WIDGET.PZL`. It is rewritten in place by
   `WriteResource` (`0x414492`) only if `access(path, 6)` succeeded, so a
   read-only install (CD) keeps the shipped all-zero flags.
2. `%WINDIR%\WIDGET.INI`, section `[Widget Workshop]`, key **`Done`**. The
   INI path `DAT_004a2450` is built at startup by `0x4101dc`
   (`GetWindowsDirectoryA`, then `"\"` if needed, then `"WIDGET.INI"`). The
   profile helpers are `0x4100e6` (get, default section
   `"Widget Workshop"`) and `0x410074` (write).

The value is a hex string with one digit per 4 puzzles: digit `k`
(0-based, leftmost first) holds puzzles `4k+1 .. 4k+4`, and bit `(N−1)&3`
of the digit is puzzle *N*.

**Read: `0x42d82d ReadSolvedFlags(u32 *outSize)`.** It returns a
malloc'd copy of the `Done` resource, `u16 count, u16 flag[n]` with
`n = size/2 − 1`:

```
buf  = GetResource('Done', 1, outSize)            // PZL must already be open
str  = GetPrivateProfileString("Widget Workshop", "Done", "", 256, ini)
for i in 0..n-1:
    if str[i>>2] == '\0': break
    if hexval(str[i>>2]) & (1 << (i & 3)):        // hexval: 0-9, A-F, a-f; else 0
        flag[i] = 0x0100                          // OR only, never clears
```

**Write: `0x42d915 WriteSolvedFlags(buf, size)`.**

```
WriteResource('Done', 1, buf)                     // no-op if PZL read-only
n = size/2 - 1
if (n>>2) + 1 < 256:
    s = '0' * ((n>>2) + 1)                        // 7 chars for 25 puzzles
    for i in 0..n-1:
        if flag[i] != 0: s[i>>2] = HEX[hexval(s[i>>2]) + (1 << (i&3))]   // uppercase
    WritePrivateProfileString("Widget Workshop", "Done", s, ini)
```

For example, all 25 solved is `Done=FFFFFF1`, and only puzzle 8 is
`Done=0800000`. After "Reset Completed Puzzles", both stores read as
all-zero (`Done=0000000`). Because reading ORs the two stores, a puzzle
that is solved in either one shows as solved.

### 2.6 The tray limits (`numG`)

Tray structures: tray `+0xa98 + 0x1c*d` is drawer *d* (7 drawers). Each
drawer has a page array (`+4`) and a page count (`+8`). Each page has a
u16 gadget-resource-id array (`+8`), a slot count (`+0xc`) and an array of
slot-info pointers (`+0x1c`). Slot info `+0x10` is an **i16 count**: −1 is
unlimited, the normal state.

- `0x4329e0 Tray::ResetCounts` sets every slot to −1 (`0x42f14d` per
  drawer) and repaints. It is called before every load (New, Open, Load
  Puzzle, Revert, Import) and from Save As (section 2.7).
- `0x432b3c Tray::ReadNumG(tray, data)` handles `numG` on load. For each
  entry it maps the tag to a gadget id (`0x435c93`) and sets the first slot
  with that id, trying drawers 0..6 (`0x42f2c8`). Unknown tags are skipped.
- `0x432a57 Tray::NumGSize` returns `6*k + 2`, where `k` is the number of
  slots whose count is not −1, or 0 when there are none. When it is 0,
  Save writes no `numG` chunk (`0x424c45`/`0x424d0a`).
- `0x432aa5 Tray::WriteNumG` writes the count, then each such slot as
  `tag (0x435c60(id)), i16 count`, drawer by drawer.
- **Taking a part** (`0x42e360`, a click on a tray slot): if the count is
  0, nothing happens. Otherwise, if memory is fine (`0x4109d4`), it
  decrements the count unless it is −1, repaints, and starts the drag.
  When memory is low it shows alert 8009 instead.
- **Giving a part back**: deleting a gadget (`0x4245d6`), or dropping a
  dragged part back on the tray (`0x4395f5`), calls `0x432867`, which
  increments the count by 1 unless it is −1 (`0x42ef7e`).
- **Drawing a slot** (`0x42e0ff`): count 0 draws frame 1 (the gray
  stipple) of sheet **801** (`WWGADGET`, tray `+0xc48`, loaded at
  `0x430fb2` as `FUN_00422940(tray+0xc48, 0x321)`) over the part. This is
  the "gray slot", class `CWGrayTraySlot`, vtable `0x4950e4`. A count
  above 0 draws the number as digits (frames `2+digit` of sheet 801,
  right to left at the slot's bottom-right). A count of −1 draws the plain
  part. The same gray overlay is also used whenever `0x432c92` reports the
  tray as disabled (`DAT_00493de0`/main window `+0x1348`), which is not
  related to puzzles.

### 2.7 Other file commands while a puzzle is active

| command | function | puzzle behaviour |
|---------|----------|------------------|
| New (`0xe100`) | `0x415865` | prompts only if not a puzzle. Clears the doc, resets the tray, `cPuzzle::Clear` (`0x42d518`, `+0x14 = 0`) |
| Open (`0xe101`) | `0x415936` → `0x416348` | same prompt rule. Loading a file clears the puzzle |
| Save (`0xe103`) | `0x415a3b` | always goes to Save As when a puzzle is active |
| Save As (`0xe104`) | `0x415c1f` | default name `"puzzle"`. **Unless doc `+0x55` (author mode) is set or Shift is held**, it first unlocks every gadget and wire (`0x426511`) and resets the tray to unlimited (`0x4329e0`). The saved `.WGT` is then an ordinary, unrestricted widget with no `numG`. With Shift held the locks and limits are saved. After a successful save it calls `cPuzzle::Clear`, so the widget is no longer a puzzle and a later Win marks nothing |
| Revert (`0x8004`) | `0x415ece` | asks 8002 *"Revert to original puzzle?"* instead of 8001. It reloads `Puzl` #`+0x12` from the PZL (`0x42d530 cPuzzle::Reload`, which reopens the file and sets `+0x14 = 1` again), resets the tray, and reloads |
| Save Into Stand-Alone (`0x8006`) | `0x4165a7` | default name `"puzzle"`. The puzzle stays active |
| Import Stand-Alone (`0x8005`) | `0x416bf5` | clears the puzzle |
| menu update | `0x414d47` | Save and Revert are enabled only when modified; Revert also needs a name or an active puzzle |
| prompt name | `0x416ee2` | for a puzzle, uses `file+0x207` (`"puzzle"`) in "Save changes to ..." |

### 2.8 What the player sees

- The list dialog, where `+` marks solved puzzles.
- The puzzle's own worksheet. Its instructions are the locked `TxLb`/`TxBx`
  gadgets in the FORM. The title bar uses the name `"puzzle"`.
- The tray, which may be emptied (gray) or show counts.
- On solving, the checker's animation and sound (1051 Win, 1151 Lose), then
  the widget stops. No dialog or message is shown.

---

## 3. Puzzle-specific gadget behaviour: locks

Lock flags: gadget `+0x0c` (saved at chunk byte `0x110`), wire `+0x25`
(saved at `conn` byte 12). Every check below has the form
`if (!locked || doc+0x55)`, so author mode (`doc+0x55`) overrides all
locks. `doc+0x55` is toggled only by `0x426488`, which has **no callers
or pointers** in the EXE. Its menu update (`0x414e6c`) targets menu id
`0xffffffff`. In the shipping build author mode is always off.

| action | where | locked behaviour |
|--------|-------|------------------|
| select gadget (click/hover) | `0x4254ba` | no "select" sound `0x84`, not selected (no selection frame, can't be opened or deleted with the Delete key) |
| select wire / joint | `0x425431`, `0x425bf2` | same |
| click on gadget body | `0x423ec3` | the gadget is still brought to the front, but it can't be dragged (`0x426878`) and its settings dialog can't be opened (the `param_4` path, `0x41baac`) |
| click on a knot that already has a wire | `0x423ec3` → `0x4272f7` | only if that **wire** is unlocked |
| click on a free knot | `0x423ec3` → `0x426df7` | **allowed even on a locked gadget**: the player can wire into or out of locked parts |
| click on a wire or joint | `0x423ec3` → `0x426b94` | only if the wire is unlocked |
| delete tool (tray mode 1) | `0x42566c` | locked gadgets, wires and joints can't be deleted |
| cursor feedback | `0x425909` | locked targets give cursor 0 (none) instead of 6 (move), 5 (wire) or 2 (knot) |
| Edit ▸ Clear Work Area | `0x425cda` | deletes only unlocked gadgets and wires |
| Edit ▸ Remove All Connections | `0x425dbe` | removes only unlocked wires |
| Edit ▸ Remove All Joints | `0x426407` | only on unlocked wires |
| selection-frame draw | `0x41d4b5` | draws a lock badge (frame 1 of sheet `DAT_004a273c`) only in author mode |

In run mode locks change nothing: locked switches, keys and so on work
normally. That is how puzzles 1, 6 and 20 (fully locked, empty tray) are
played.

**Hidden design-mode keys** (`WM_KEYDOWN`, message map entry at `0x494dbd`. The
handler `0x434451` calls doc vtable `+8` = `0x42385d(doc, vk, 0)` when
main window `+0x1340` is set, then `0x423c2d` in design mode and then the
tray's `0x4326e0`). These came from the Mac's character codes, so on
Windows they are virtual-key codes:

| VK | key | action |
|----|-----|--------|
| `0x2e` | Delete | delete the selected wire, gadget or joint (`0x42539a`, `0x4244de`, `0x425bac`) |
| `0x4c` (`'L'`) / `0x6c` (VK_SEPARATOR) | L | toggle the lock of the selected gadget and the selected wire. A locked object can't be selected, so in practice this only locks |
| `0x6b` / `0x6d` (VK_ADD / VK_SUBTRACT, the Mac's `'k'`/`'m'`) | numpad + / − | change the tray count for the selected gadget's type by +1/−1 (`0x432921` → `0x42f060`). −1 + 1 becomes 0 (limited). Going below −1 is refused. In the tray handler, the same keys over a tray slot adjust that slot's count |
| `0x10` / `0x11` | Shift / Ctrl | tray view switching (`0x4326e0`/`0x432824`, not puzzle-specific) |

These keys are always live in design mode in the shipping build, so a
player can lock gadgets (but not unlock them, since locked ones can't be
selected) and can change tray counts. They are the authoring tools Maxis
used to build the PZL.

---

## 4. Function names

| address | proposed name |
|---------|---------------|
| 0x4101dc | `InitIniPath` (static-init, `%WINDIR%\WIDGET.INI`) |
| 0x410074 | `WriteProfileStr(key, value, section=NULL)` |
| 0x4100e6 | `ReadProfileStr(key, default, buf, len, section=NULL)` |
| 0x410b5b | `AlertString(alert, n)` (string 8000+n) |
| 0x410bb8 | `ConfirmString(alert, n)` |
| 0x413e78 | `ResFile_Open(path)` |
| 0x4140d2 | `ResFile_Close()` |
| 0x414102 | `ResFile_GetResource(type, id, outSize, outName)` |
| 0x414492 | `ResFile_WriteResource(type, id, buf)` |
| 0x414729 | `ResFile_CountMinus1(type)` |
| 0x414781 | `ResFile_GetName(type, id, out)` |
| 0x4153ac | `GetMousePos(Point*)` |
| 0x41507c | `IsKeyDown(kbd, vk)` |
| 0x416f1a | `File_LoadPuzzle(file)` |
| 0x415ece | `File_Revert(file)` |
| 0x415c1f | `File_SaveAs(file)` |
| 0x415a3b | `File_Save(file)` |
| 0x415865 | `File_New(file)` |
| 0x416ee2 | `File_GetPromptName(file)` |
| 0x414d47 | `File_UpdateSaveRevertMenu()` |
| 0x42cef8 | `cPuzzleDlg::OnInitDialog` |
| 0x42ced8 | `MacApostropheToAscii(str)` |
| 0x42d01e | `cPuzzleDlg::OnCommand` |
| 0x42d284 | `RunPuzzleDialog(path, outData, outSize)` |
| 0x42d3c5 | `cPuzzleDlg::~cPuzzleDlg` (deleting) |
| 0x42d3fc | `cPuzzle::cPuzzle` |
| 0x42d487 | `cPuzzle::~cPuzzle` |
| 0x42d4b1 | `cPuzzle::MarkSolved` |
| 0x42d518 | `cPuzzle::Clear` |
| 0x42d524 / 0x42d52b / 0x42d5ae / 0x42d5c8 | `cPuzzle` stubs (return 0 / nop / nop / return 1), unreferenced |
| 0x42d530 | `cPuzzle::Reload(outSize)` |
| 0x42d570 | `cPuzzle::Choose(outData, outSize)` |
| 0x42d5a3 | `cPuzzle::IsActive` |
| 0x42d5b3 | `cPuzzle::GetDocName(num, out)` (`"puzzle"`) |
| 0x42d5d1 | `cPuzzle::GetNumber` |
| 0x42d5dd / 0x42d642 / 0x42d74d | `cPuzzleInit` ctor / dtor / deleting dtor |
| 0x42d7cc | `NibbleToHex` |
| 0x42d7ed | `HexToNibble` |
| 0x42d82d | `ReadSolvedFlags(outSize)` |
| 0x42d915 | `WriteSolvedFlags(buf, size)` |
| 0x41ed78 / 0x41ed83 | `cGadget::IsLocked` / `cGadget::SetLocked` |
| 0x43c8a4 / 0x43c8af | `cConnection::IsLocked` / `cConnection::SetLocked` |
| 0x4263fc | `CWidgetDoc::IsAuthorMode` (`+0x55`) |
| 0x426488 | `CWidgetDoc::ToggleAuthorMode` (unreferenced) |
| 0x426511 | `CWidgetDoc::UnlockAll` |
| 0x425cda | `CWidgetDoc::ClearWorkArea` |
| 0x425dbe | `CWidgetDoc::RemoveAllConnections` |
| 0x426407 | `CWidgetDoc::RemoveAllJoints` |
| 0x4254ba | `CWidgetDoc::SelectGadget` |
| 0x425431 | `CWidgetDoc::SelectWire` |
| 0x425bf2 | `CWidgetDoc::SelectJoint` |
| 0x42566c | `CWidgetDoc::DeleteToolClick` |
| 0x425909 | `CWidgetDoc::CursorFor` |
| 0x423ec3 | `CWidgetDoc::DesignMouseDown` |
| 0x42385d | `CWidgetDoc::OnKeyDown` |
| 0x423c2d | `CWidgetDoc::DesignKey` |
| 0x4245d6 | `CWidgetDoc::DeleteGadget` |
| 0x42503e | `CWidgetDoc::LoadForm(data, size, quiet)` |
| 0x424c45 / 0x424d0a | `CWidgetDoc::FormSize` / `CWidgetDoc::SaveForm` |
| 0x434451 | `CMainWnd::OnKeyDown` |
| 0x4329e0 | `Tray::ResetCounts` |
| 0x432b3c | `Tray::ReadNumG` |
| 0x432a57 | `Tray::NumGSize` |
| 0x432aa5 | `Tray::WriteNumG` |
| 0x432867 | `Tray::ReturnPart(id)` |
| 0x432921 | `Tray::AdjustCount(id, delta)` |
| 0x4326e0 | `Tray::OnKey` |
| 0x42ef11 | `Drawer::CountLimited` |
| 0x42ef7e | `Drawer::IncCount(id)` |
| 0x42f060 | `Drawer::AdjustCount(id, delta)` |
| 0x42f14d | `Drawer::ResetCounts` |
| 0x42f1c0 | `Drawer::WriteNumG` |
| 0x42f2c8 | `Drawer::SetCount(entry)` |
| 0x42e0ff | `TrayPage::Draw` |
| 0x42e360 | `TrayPage::OnClick` (take part) |
| 0x4395f5 | `PartDrag::Drop` |
| 0x432d65 | `CWGrayTraySlot::CWGrayTraySlot` |
| 0x457604 | `cStopper::InitShared` |
| 0x4576e0 / 0x45786b | `cStopper` fresh / load ctor |
| 0x4578b0 | `cStopper::Reset` |
| 0x4578c8 | `cStopper::Draw` |
| 0x457af0 | `cStopper::~cStopper` |
| 0x457b77 | `cStopper::Start` |
| 0x457ba2 | `cStopper::Tick` |
| 0x457e95 | `cStopper::FillDialog` (radio enable dependencies) |
| 0x430ee7 | `ControlPanel::StopRunning` |
| 0x4402cc | `PlayUISound(id, async)` |

## 5. Open questions / uncertainties

- `0x4101dc` (INI path) is reached only through the static-init pointer
  at `0x4a20b6`. Ghidra has no function there, and the disassembly is from
  objdump.
- I did not confirm what the `+0x1340` "keys enabled" flag on the main
  window means. It is set to 1 in `0x42ce2d`.
- The meaning of `0x432c92`'s gray-out condition (`DAT_00493de0`, main
  window `+0x1348`) is not settled. It is not used by puzzles.
- The sheet behind `DAT_004a273c` (the lock badge drawn in author mode) was
  not identified.
- `0x45f99f`'s second argument (1) is assumed to mean Yes/No, with 1
  returned for Yes. This was not checked against its body.
- Load Puzzle does not clear the document's modified flag (`file+0x100`).
  This is harmless, because puzzles never prompt.
