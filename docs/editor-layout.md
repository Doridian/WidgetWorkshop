# The editor: main window, toolbars, parts tray, worksheet frame

What the player sees around the gadgets in `WIDGET.EXE` (Win95 build), and
what each control does. Addresses are in `WIDGET.EXE`. Picture "sheets" are
`WWGADGET.DLL` type 1004 (`HR`, see `crates/formats/src/hr.rs`), rendered in
`ghidra/png/WWGADGET/<sheet>.png` (frames from 1; the PNG pads each frame
with a 1 px border). Strings are `WIDGET.EXE` RT_STRING ids.

Rects below are Windows `RECT`s, `(left, top)-(right, bottom)`, right/bottom
exclusive, unless written as a Mac `Rect` (top, left, bottom, right).

Out of scope here (other docs): what the tools do on the worksheet (select,
drag, wire, cut, undo, settings dialogs), puzzles (`cPuzzleDlg`, `numG`),
the intro and main menu (`CIntroDialog`, `0x42b07c`), the gadgets'
own drawing.

## 1. Class map (names from the RTTI type descriptors)

Each vtable's slot −3 (`vtable − 12`) points to a Borland type descriptor
whose first dword is the object size and which carries the class name, so
these names are certain (the earlier guesses from the string order are not
needed):

| class | vtable | size | what it is |
|---|---|---|---|
| `CWidgetApp` | 0x494a48 | 0xd8 | the app |
| `CMainFrame` | 0x495480 | | the top-level `CFrameWnd` |
| `CWidgetView` | 0x495180 | | the `CView` = the whole worksheet window; owns both toolbars and the offscreen |
| `CWidgetDoc` | 0x494bcc | 0x60 | the MFC document (file I/O) |
| `cWorksheetPane` (base `cAbstractPane`, 0x492fc4) | 0x49326c | 0x56 | the worksheet model (`DAT_004a2848`): gadget list, wires, work-area rect, background, dirty rect |
| `CWTbar` | 0x49452c | 0x4c4 | base of the floating toolbars (draggable child window with its own offscreen) |
| `CMainTbar` | 0x495100 | | GO / STOP / Light / Sound toolbar (`DAT_004a2868`) |
| `CWidgetTbar` | 0x4944ac | 0xc58 | **the parts tray** window (`DAT_004a2864`) |
| `CWToolbarElement` | 0x4945e8 | | abstract element (pure virtuals) |
| `CWBmpButton` | 0x4945cc | 0x70 | element that **toggles** on each press (Light, Sound) |
| `CWBmpPushButton` | 0x49448c | 0x70 | momentary element, fires on release (Previous/Next tray page) |
| `CWBmpRadioButton` | 0x4950e4 | 0x74 | radio element in a `CGroupBox` (GO/STOP, the 3 tools, the 7 folder tabs) |
| `CGroupBox` | 0x4950c4 | | radio group: remembers the selected element |
| `CWStaticText` | 0x494440 | | the tray's help line |
| `CWScrollNumber` | 0x494474 | | the tray's page number digit |
| `CWTrayToolbar` | 0x49445c | 0x38 | the **slot area** element inside the tray; owns the 7 folders |
| `CWTrayFolder` | (none, 0x1c bytes) | 0x1c | one folder: list of pages + current page |
| `CWTraySlot` | 0x4945bc | 0x34 | one *page* of a folder (a row of parts) |
| `CWGrayTraySlot` | 0x4945ac | | blank gray page used in the folder-switch animation |
| `CWTraySlotInfo` | (none, 0x12 bytes) | 0x12 | one part on a page: rect + count |
| `CWPlaceGadgetTool` | 0x496528 | 0x14 | the "drag a new part from the tray" tool |
| `CWorksheetDragTool` / `CWInputTool` | 0x4964c0 / 0x4964f4 | 8 | tool bases |
| `cOffscreenUpdateList` & `c…UpdateElement` | 0x4935a8 … | | sprite save/restore lists used while dragging |
| `CWinGDib` | 0x4962d4 | 0x44c | 8-bit WinG/DIB offscreen |
| `CWResPic` | 0x496438 | | a loaded `HR` picture sheet |
| `CAboutWnd` | 0x4938cc | | the About window (credits) |
| `CAboutDlg` | 0x4949b8 | 0x44 | MFC About dialog (template 100); constructed by 0x4331ff but **never shown** |
| `CStdWinFitRectDialog` | 0x493650 | | dialog 10014, "squeeze to screen size" (Fit Work Area) |
| `CWrksFitDialog` | 0x4936e0 | | dialog 10010, "too big to share with Mac users" (on save) |
| `cDialogDocItem` | (none, 0x6e bytes) | | Doc's balloon in a gadget settings dialog |

`CNewTypeDlg` (0x45e763) is MFC's own "New" type picker (template 30721);
there is one document template, so it never appears.

## 2. Startup and the main window

- `CWidgetApp::InitInstance` (0x43308b): on Win 3.x only,
  `SetDialogBkColor(0xc0c0c0)`; CTL3D unless profile `No3DControlsPlease`;
  **`m_nCmdShow = 3` (SW_SHOWMAXIMIZED)**: the frame opens maximized. One
  `CSingleDocTemplate(2, CWidgetDoc, CMainFrame, CWidgetView)`. The frame's
  `PreCreateWindow` (0x43586e) is the default, so the restored size is
  `CW_USEDEFAULT`. No MFC toolbar, no status bar.
- Title: `"Widget Workshop - %s"` (0x491440, set by 0x417092) with the
  file's title; the doc string (string 2) gives `untitled.wgt`.
- Menu: the frame is created with menu 2 (a single blank item, id 32784);
  the startup handler 0x43480e (`WM_USER+0x65` = 0x465, posted by the
  first `OnDraw`) runs the intro (skipped in the port), then `LoadMenu(3)`
  and `SetMenu`, shows the two toolbars (`ShowWindow(…, SW_RESTORE)`),
  and invalidates the whole worksheet.
- `CWidgetView` window class (0x433dfa): `CS_DBLCLKS`, **no class cursor**
  (cursor set in `WM_SETCURSOR`), null background brush; `WM_ERASEBKGND`
  returns 1 (0x4340c9).
- `CWidgetView::OnCreate` (0x4340f0): creates `CMainTbar` at view client
  **(0, 0)** and the tray at **(54, 0)** (main toolbar width 53 + 1);
  `DragAcceptFiles`; `SetTimer(1, 1000 ms)`. Both toolbars are created
  hidden (`WS_CHILD|WS_CLIPSIBLINGS`, 0x44000000) and shown by 0x43480e.
- The worksheet is the **whole view client area** (= frame client area).
  The toolbars are child windows floating over it. There is no scrolling
  (plain `CView`, no scroll bars).

### Window messages

`CMainFrame` (message map 0x495281): `WM_NCCREATE`→0x4358b1 (default),
`WM_NCPAINT` 0x4358dd (draws a black `FrameRect` around the window when
`+0xc0` is set; the setters 0x4359ab/0x4359d1 are unreferenced, so in
practice default), `WM_SIZE` 0x4358bf (default), `WM_CREATE` 0x435894
(default), `WM_INITMENUPOPUP` 0x4358d8 (**empty**: this disables MFC's
automatic command-UI greying; menus are enabled by hand, §5),
`WM_ACTIVATEAPP` 0x4359f7, `WM_QUERYENDSESSION` 0x435b12 (save prompt),
`0x466` 0x435b2b (open a file passed from a second instance).

`WM_ACTIVATEAPP` (0x4359f7): deactivate → `app+0xd4 = 0` (cursor
updates off), all sound stopped (0x44083d), 0x424944(pane). Activate →
`app+0xd4 = 1`, sound resumed if Sound is on, palette realized, the view
and both toolbars invalidated, 0x424944(pane).

`CWidgetView` (map 0x494c95): `WM_PAINT` (MFC → `OnDraw` 0x433e52),
`WM_CREATE` 0x4340f0, `WM_DESTROY` 0x4341c0, `WM_SETCURSOR` 0x4345ed,
mouse 0x434531/0x4343f2/0x4344d2/0x4343b4/0x4343d3 (to the pane),
`WM_KEYDOWN` 0x434451, `WM_KEYUP` 0x43449b, `WM_CHAR` 0x43448d (ignored),
`WM_SIZE` 0x43424e, print/preview (MFC), `0x464` 0x43473a (delete a
finished tool), `0x465` 0x43480e (startup), `WM_DROPFILES` 0x434ac8 (open
the dropped file), `WM_TIMER` 0x434b0d.

- `WM_SIZE` (0x43424e), when not minimized and `0 < cx, cy < 0x7fff`:
  resize the view offscreen to `cx × cy` (on failure: alert 9 "running
  out of memory" and the background is set to Plain); `view+0x1348 =
  (offscreen size == client size)`; the pane's work-area rect and the
  background's rect become `(0, 0)-(cx, cy)` (0x426629 / 0x43d182); full
  redraw. Before the first `WM_SIZE` the pane rect is Mac `(0, 0, 540,
  720)`, i.e. 720×540 (0x423268).
- `WM_TIMER` (0x434b0d), only while editing: refresh the tray's "gray"
  state (0x432c05, §3.6), and if the offscreen is not full size
  (`view+0x1348 == 0`) retry allocating it.
- `PreTranslateMessage` (0x434600): while a tool object is active
  (`view+0x1344`), all mouse messages (0x200..0x209) and
  `WM_KEYDOWN/KEYUP/CHAR` go to the tool's slot 0x1c (dispatch 0x4390d5:
  0x200→slot 0, 0x201→4, 0x202→8, 0x203→0xc, 0x204→0x10, 0x205→0x14,
  0x206→0x18).

## 3. The toolbars

### 3.1 `CWTbar`, the common toolbar window

Fields: `+0x1c` HWND, `+0x30` `CWinGDib` offscreen (window-sized),
`+0x47c` dragging flag, `+0x480/+0x484` grab offset, `+0x488/+0x48c` last
outline position, `+0x490` **move-bar height = 15** (ctor 0x42fa33),
`+0x494` RECT clip for the background, `+0x4a4` the `CWResPic`, `+0x4a8`
`CObArray` of elements (`+0x4ac` data, `+0x4b0` count), `+0x4bc` parent
(the view), `+0x4c0` element holding the capture.

- `Create` (0x42fb3b)`(this, parent, x, y, RECT r, short pic)`: loads
  sheet `pic`, `+0x494 = r`, creates class `"AfxWnd"` child at `(x, y)`
  of size `r`, background brush null, offscreen `r.w × r.h`.
- Drawing (0x4304b5): sheet frame 1 drawn at (0, 0) clipped to `+0x494`,
  then each element's slot 4, in creation order, all into the offscreen.
  `WM_PAINT` (0x430536) blits the offscreen at (0, 0) (palette selected).
  `WM_ERASEBKGND` returns 1. The first paint sets `DAT_00493de4 = 1`
  (enables the Enter/Esc keys, §3.2).
- `WM_LBUTTONDOWN` (0x42fccc) and `WM_LBUTTONDBLCLK` (0x42fe99, same):
  bring the toolbar to the top of the z-order. If `y > 15`: call slot 8
  ("press") of **every** element whose rect contains the point. Else
  (`y ≤ 15`, the **move bar**): start dragging the toolbar: capture,
  `ClipCursor` to the parent's client rect, `app+0xd4 = 0`, draw an XOR
  outline (0x430172: four 2 px `PatBlt(PATINVERT)` strips on the screen
  DC, the toolbar's size, at cursor − grab offset).
- `WM_MOUSEMOVE` (0x42febc): if dragging, move the outline. Else, the hover
  text (§3.4): for the first element containing the point, `id =
  slot0()`; if `id ≥ 1` show string `id + 2500` and stop; if `id < 1`
  stop without changing the text. If no element contains the point: show
  string 4000 (does not exist → empty) when `y > 15`, string 4017 "Move
  Bar" when `y ≤ 15`.
- `WM_LBUTTONUP` (0x4302f7): if dragging: erase the outline, release,
  unclip, `SetWindowPos` to the new place (parent client coords = cursor −
  grab offset), `ShowWindow(SW_RESTORE)`. Else if an element holds the
  capture (`+0x4c0`), call its slot 0x10 with "released inside".
- Element command → slot 0x70 of the toolbar: `CWTbar` default 0x4305c4
  posts `WM_COMMAND(id)` to the view; `CMainTbar` 0x430d2c and
  `CWidgetTbar` 0x4324c8 handle their own ids. Positions are not saved;
  toolbars are not moved when the window resizes.

### 3.2 Toolbar elements (`CWToolbarElement`, 0x70 bytes)

| off | what |
|---|---|
| +0x00 | RECT, the element in toolbar coords |
| +0x10 | vtable |
| +0x14 | command id (slot 0 returns it, 0x434de5) |
| +0x18 | owner toolbar |
| +0x1c | state: 0 normal, 1 pressed/on/selected |
| +0x20, +0x24 | normal: sheet, source RECT |
| +0x34, +0x38 | on/pressed: sheet, source RECT |
| +0x48, +0x4c | disabled: sheet, source RECT |
| +0x6c | enabled (1 by default, 0x42db7c) |
| +0x70 | `CGroupBox*` (radio buttons only) |

`Init` (0x42dbe6)`(elem, owner, id, rect, normal, on, disabled)`; a null
`on`/`disabled` copies `normal`. Draw (0x42dcb0): `!enabled` → disabled
image, else `state` 0 → normal, 1 → on; the source RECT of the sheet's
frame 1 is copied to the element's top-left (0x422f2f). Every state
change redraws the element into the offscreen and invalidates its rect.

Vtable slots: 0 id/hit, 4 draw, 8 press, 0xc dtor, 0x10 release.

| class | press (slot 8) | release (slot 0x10) |
|---|---|---|
| `CWBmpButton` (toggle) | 0x42dd17: if enabled, `state = !state`, notify owner `(id, state)` | – |
| `CWBmpRadioButton` | 0x42dd8c: if enabled and not selected: `CGroupBox::Select` (0x42ddd4: old one's state 0, new one's 1), notify owner `(id)` | – |
| `CWBmpPushButton` | 0x42de8d: if enabled: capture, `state = 1` | 0x42df32: release, `state = 0`; notify owner only if released inside |

### 3.3 `CMainTbar`: GO / STOP / Light / Sound

Created by 0x4305f2 at view (0, 0), window **53×93**, sheet **1** (one
106×93 frame: left half 0..53 the "off" art, right half 53..106 the
"on" art; the on-source rect is the normal one shifted right by 53).
Rows 0..15 are the move bar.

| element | id | class | rect (normal source = dest) | on source | tooltip | initial |
|---|---|---|---|---|---|---|
| +0x5a4 GO | 1501 (0x5dd) | radio, group +0x68c | (5,19)-(47,38) | (58,19)-(100,38) | 4001 "GO Button - starts experiments!" | off |
| +0x618 STOP | 1502 (0x5de) | radio, group +0x68c | (5,40)-(47,59) | (58,40)-(100,59) | 4002 "STOP Button - stops experiments!" | **selected** |
| +0x4c4 Light | 1503 (0x5df) | `CWBmpButton` toggle | (5,64)-(25,88) | (58,64)-(78,88) | 4003 "Light On/Off" | **on** |
| +0x534 Sound | 1504 (0x5e0) | `CWBmpButton` toggle | (27,64)-(47,88) | (80,64)-(100,88) | 4004 "Sound On/Off" | **on** |

(Creation order, which is also the draw order: STOP, GO, Light, Sound.)
Disabled images = normal (never disabled). Initial globals: `DAT_00493dde
= 1` (editing), `DAT_00493ddc = 1` (light on), `DAT_00493ddd = 1` (sound
on).

Handler 0x430d2c `(this, id, state)`:

- **GO** (1501): UI sound 0x80, then **start** (0x430e70), if editing:
  `DAT_00493dde = 0`, sound 0x8e, **hide the tray** (`ShowWindow(tray,
  SW_HIDE)`), pane start 0x424a12 (switch the tray to the pointer tool,
  deselect all, reset the run clock `pane+0x4b`, every gadget's slot 0x20
  "start" and slot 0xc "invalidate"), press GO (keeps the radio in sync
  when started from the keyboard), menu update 0x414b7e.
- **STOP** (1502): sound 0x80, then **stop** (0x430ee7), if running:
  `DAT_00493dde = 1`, stop all sounds if Sound on (0x440823), pane stop
  0x424a94 (every gadget's slot 0x1c "reset", every wire reset 0x43ca73,
  `pane+0x4f = 0`, full redraw), sound 0x8f, **show the tray**
  (`SW_RESTORE`), press STOP, menu update.
- **Light** (1503): sound 0x80, then 0x8c if the light was off (turning
  on) or 0x8d if it was on, sound 0x80; `DAT_00493ddc = state`; invalidate
  the whole worksheet and redraw it; rebuild the background tile (§4.3).
- **Sound** (1504): sound 0x80; if it was on, sound 0x91 (flag 1) before
  turning off; `DAT_00493ddd = state`; when turned on: resume sounds
  (0x440837) and play 0x90. All UI sounds (0x4402cc) are silent while
  `DAT_00493ddd == 0`.
- **Keys** (0x430f63, from the view's key handler, only after the main
  toolbar has painted once): **Enter** (VK 0x0d) while editing = GO;
  **Esc** (0x1b) while running = STOP.

While running, the tray is hidden, the menu items of §5 are disabled,
clicks on the worksheet go to the gadgets' "click while running" (or move
the part when the Move tool is selected, 0x423e3f); the pane keys (Delete,
L, numpad ±) do nothing.

### 3.4 The help line (`CWStaticText`)

The tray's element `+0xb94` (`DAT_00493dd8`), rect **(4,3)-(445,15)**
inside the tray (on the tray's move bar), id −1 (so it has no hover text
itself). Draw (0x42da30): its normal source (same rect of sheet 2, the
plain bar) then `TextOut(4, 3, text)` with **"Small Fonts", height 12**
(rect height; `CreateFont(12,0,0,0,FW_NORMAL,…,PROOF_QUALITY,…)`,
0x42db12), `SetTextAlign(0)` (top-left), **white** text (`+0x74 =
0xffffff`), `SetBkMode(TRANSPARENT)`. Starts empty.

It shows the hover text of **either** toolbar (§3.1): string `2500 + id`
for buttons (4001..4016), `2500 + tray index` (2501..2564, the part's
name) over a part in the tray, 4000 (empty) over a toolbar but not on an
element, 4017 "Move Bar" over a move bar. It only repaints when the id
changes (`+0xc0c` holds the current id, `+0xc10` the text).

### 3.5 `CWidgetTbar`: the parts tray

Created by 0x431281 at view (54, 0), window **449×93**, sheet **2**
(540×107; x 0..449 is the tray's face, x ≥ 450 holds the "on/pressed"
art and the digit strip). Background drawn from the sheet at (0,0).

| element | id | class | dest = normal source | on source | disabled source | tooltip |
|---|---|---|---|---|---|---|
| +0x4c4 Pointer tool | 1505 | radio, group +0x620 | (5,18)-(63,52) | (450,0)-(508,34) | = normal | 4005 "Pointer Tool" |
| +0x538 Cut tool | 1506 | radio, group +0x620 | (5,54)-(33,88) | (450,36)-(478,70) | = normal | 4006 "Cut Tool" |
| +0x5ac Move tool | 1507 | radio, group +0x620 | (35,54)-(63,88) | (480,36)-(508,70) | = normal | 4007 "Move Tool" |
| +0xb94 help line | −1 | `CWStaticText` | (4,3)-(445,15) | | | |
| +0x640+0x74·i tab i (i=0..6) | 1508+i | radio, group +0x96c | (68+50i,18)-(117+50i,49) | (68+50i,53)-(117+50i,84) | = normal | 4008+i |
| +0x98c Previous page | 1515 | `CWBmpPushButton` | (423,27)-(444,43) | (450,74)-(471,90) | (471,74)-(492,90) | 4015 "Previous Tray" |
| +0x9fc Next page | 1516 | `CWBmpPushButton` | (423,65)-(444,81) | (450,90)-(471,106) | (471,90)-(492,106) | 4016 "Next Tray" |
| +0xa6c page number | −1 | `CWScrollNumber` | (427,48)-(439,59) | | | |
| +0xb5c slot area | (part under cursor) | `CWTrayToolbar` | (67,51)-(417,91), fill white | | | part name |

(Draw order = this table's order.) Tab rects come from the table at
0x4942ac (8 ints per tab: dest, on-source); the tabs are Display, Timer,
Sound, Number, Switch, Text, Super Parts (4008..4014). Initial: Pointer
tool and tab 0 selected, page 0.

`CWScrollNumber` (draw 0x42dfac): copies source rect `table[value]` of the
sheet to its top-left. Table at 0x49438c, 6 entries: (511,0)-(523,10),
(511,11)-(523,22), (511,22)-(523,33), (511,33)-(523,44), (511,44)-(523,55),
(511,55)-(523,66): the digits "1".."6" on the sheet's right edge. `value`
= current page (0-based); values outside 0..5 are not shown.

Arrow/number refresh (0x4322e5, after every tray command and at
creation): Previous enabled iff `page != 0`; Next enabled iff `page !=
pages − 1`; number = page.

Handler 0x4324c8:

- **Pointer** (1505): cancel the view's active tool (0x43469e(view, 0)),
  sound 0x80. **Cut**/**Move** (1506/1507): sound 0x80. The current tool
  is read back from the group (0x432683: 0 pointer, 1 cut, 2 move).
  `0x4325e0(tray, n)` selects tool `n` by code and refreshes the cursor.
- **Tab** (1508..1514): sound 0x80, `UpdateWindow`, folder switch
  animation (0x42f419, §3.6).
- **Previous** (1515) / **Next** (1516): sound 0x94, slide to the previous
  / next page (0x42ee6e / 0x42edc5) of the current folder.
- Keys (0x4326e0, from the view's key handler): **Shift** held while
  editing → temporarily the Cut tool; **Ctrl** held → temporarily the Move
  tool; releasing (0x432824 on `WM_KEYUP`) restores the previous tool
  (`+0xc24`, 3 = none saved). **Numpad +** (VK 0x6b) / **numpad −** (0x6d)
  while editing with the cursor over a part in the tray: that part's count
  +1 / −1 (puzzle authoring; see §3.6 for the count rules).

### 3.6 Folders, pages and slots

The slot area `CWTrayToolbar` (+0xb5c, 350×40 at tray (67,51)) holds 7
`CWTrayFolder`s (+0xa98, 0x1c each: `CObArray` of pages, `+0x14` current
page, `+0x18` the area). Each page (`CWTraySlot`, 0x34 bytes: `+8`
word-array of tray indexes, `+0xc` count, `+0x1c` array of
`CWTraySlotInfo*`, `+0x2c` folder, `+0x30` free width) holds a row of
parts. `CWTraySlotInfo`: RECT relative to the area's top-left, `+0x10`
i16 **count** (−1 = unlimited).

**Tray index**: parts are numbered 1..64 in the tray. Index → tag table at
0x49552c (`{u32 index, u32 tag}` pairs, 0x435c60; reverse 0x435c93);
index → name = string `2500 + index`; index → tray picture = **sheet 898
frame `index`** for 1..30, **sheet 899 frame `index − 30`** for 31..64
(0x432229 / 0x432267).

Folder contents (0x49428c → per-folder zero-terminated lists, −2 = page
break) are added with 0x42ecea; a part that doesn't fit the page's free
width starts a new page too. Layout of a page (0x42e0ff, re-run on every
add): free width starts at 350; a part of picture size `w×h` uses `w + 3`
(new page if `w + 3 > free`); with `n` parts and `rem` free, `gap = (3n +
rem) / (n + 1)` (C int division), part 0 at `x = gap`, then `x += w +
gap`; `y = (40 − h) / 2`. Resulting layout (area-relative, x and y
ranges):

| folder (tab) | page | parts: index tag "name" (picture) x-range, y-range |
|---|---|---|
| 1 Display | 1 | 16 `bulb` Light Bulb (898/16) 38..67, 3..37; 18 `rgbd` Color Mixer (898/18) 105..150, 2..37; 50 `StCh` Strip Chart (899/20) 188..238, 3..37; 51 `GrPr` Graphic Projector (899/21) 276..310, 3..37 |
| | 2 | 13 `disp` Three-Digit Display (898/13) 33..107, 3..37; 14 `di10` Ten-Digit Display (898/14) 140..315, 5..35 |
| 2 Timer | 1 | 9 `DClk` Digital Clock (898/9) 60..113, 3..37; 4 `Wtch` Stopwatch (898/4) 173..202, 3..37; 11 `Metr` Metronome (898/11) 262..288, 0..40 |
| 3 Sound | 1 | 46 `SdSw` Sound Switch (899/16) 35..84, 3..37; 43 `AnSd` Animal Sounds (899/13) 119..150, 3..37; 44 `MsSd` Assorted Sounds (899/14) 185..220, 3..37; 39 `Spkr` Speaker (899/9) 255..312, 3..37 |
| | 2 | 8 `Fork` Tuning Fork (898/8) 43..79, 2..38; 38 `Tape` Recorder (899/8) 122..181, 3..37; 45 `SdTr` Sound Transformer (899/15) 224..304, 5..35 |
| 4 Number | 1 | 31 `NmKP` Numeric Key Pad (899/1) 15..41, 3..37; 26 `cnst` Three-Digit Number (898/26) 56..99, 3..37; 42 `Cntr` Counter (899/12) 114..159, 3..37; 63 `MiCo` Mini Three-Digit Number (899/33) 174..229, 6..33; 15 `Sldr` Slider (898/15) 244..331, 3..37 |
| | 2 | 22 `Addr` Adder 26..80; 23 `Subr` Subtractor 106..160; 24 `Mulr` Multiplier 186..240; 25 `Divr` Divider 266..320 (898/22..25, all 3..37) |
| | 3 | 6 `Coin` Coin Toss (898/6) 32..69; 7 `rand` Random Number Generator (898/7) 101..147; 5 `Ttlr` Totaler (898/5) 179..251; 32 `Comp` Comparer (899/2) 283..317 (all 3..37) |
| | 4 | 28 `1ncr` Math Functions (898/28) 46..100; 29 `ArVo` Area/Volume (898/29) 146..201; 30 `Conv` Conversion (898/30) 247..303 (all 3..37) |
| | 5 | 33 `AndG` And Gate (899/3) 31..80, 2..37; 34 `OrGa` Or Gate (899/4) 111..156, 3..37; 35 `XorG` Xor Gate (899/5) 187..236, 2..37; 36 `NotG` Not Gate (899/6) 267..316, 2..37 |
| 5 Switch | 1 | 12 `swit` Light Switch (898/12) 29..54, 3..37; 10 `Key_` Key Press (898/10) 83..119, 3..37; 41 `MsCl` Mouse Button (899/11) 148..196, 2..37; 40 `MsMv` Mouse Distance Measurer (899/10) 225..262, 3..37; 49 `DnPl` Dynamite Plunger (899/19) 291..317, 3..36 |
| | 2 | 37 `Mplx` Switcher (899/7) 40..84, 3..37; 19 `Spl2` Two Way Splitter (898/19) 124..169, 1..38; 20 `Spl3` Three Way (898/20) 209..244, 2..38; 21 `Spl4` Four Way (898/21) 284..310, 2..38 |
| 6 Text | 1 | 64 `TxLb` Label (899/34) 45..76, 15..24; 47 `TxLn` Message List (899/17) 121..200, 8..32; 48 `TxBx` Billboard (899/18) 245..304, 3..37 |
| | 2 | 53..56 `Stk1..Stk4` Sticker (899/23..26) 25..78, 103..163, 188..242, 267..321 (3..37) |
| | 3 | 57..59 `Stk5..Stk7` (899/27..29) 27..88, 115..163, 190..322 (3..37) |
| | 4 | 60..62 `Stk8, Stk9, St10` (899/30..32) 18..99, 117..213, 231..331 (3..37) |
| 7 Super | 1 | 52 `Stop` Win / Lose Checker (899/22) 42..77, 3..36; 1 `Pend` Pendulum (898/1) 119..159, 1..38; 2 `Drop` Gravity Chamber (898/2) 201..214, 1..38; 3 `Cano` Cannon (898/3) 256..308, 3..37 |
| | 2 | 17 `hart` Heart (898/17) 85..108, 3..37; 27 `Plnt` Solar System (898/27) 193..265, 3..37 |

(Number of pages: 2, 1, 2, 5, 2, 4, 2 → the arrows and the "1".."5"
digit.)

**Drawing a page** (0x42e360): fill the area with white (palette index
255), then for each part: its tray picture at area origin + slot top-left,
clipped to the slot; then, with **sheet 801**:

- if `count == 0`, or the tray is grayed (`DAT_00493de0`, or
  `view+0x1348 == 0`, i.e. memory trouble), the stipple **frame 1**
  (350×40) drawn at the slot's top-left clipped to the slot rect (grays
  the part out);
- else if `count != −1` (a puzzle limit): the count in **frames 2..11**
  (digits 0..9, 8×9), right-aligned at the slot's bottom-right: first
  digit at `(right − 8, bottom − 9)`, each next (more significant) one 8 px
  further left.

`count` comes from the puzzle (`numG` chunk, 0x432b3c: `u16 n`, then `n`
× `{u32 tag, u16 count}`, big-endian; 0x42f14d resets all to −1).
Taking a part decrements a limited count; dropping it back on the tray
increments it (0x432867); numpad ± changes it by ±1 (0x42f060: from −1,
+1 gives 0; −1 is only applied when the count isn't already −1).

**Slide animations** (0x42f592, the area as rect, `H = 40`): sound 0x95;
both pages rendered to temporary offscreens `A` (from) and `B` (to); busy
loop on `GetTickCount`, `k = elapsed_ms · H / 180` (clamped to `H`), each
step blitting straight to the screen DC:
- direction 0 ("down"): `A` rows `0..H−k` at area y + k, `B` rows
  `H−k..H` at area y (content slides down, new page enters from the top);
- direction 1 ("up"): `A` rows `k..H` at area y, `B` rows `0..k` at
  area y + H − k (content slides up, new page enters from the bottom).
Next page = direction 1, previous page = direction 0. Folder switch
(0x42f419): the old folder's current page → a `CWGrayTraySlot` (fill
0x808080 → palette 248) in direction 1, then gray → the new folder's
**page 0** in direction 0. Total ≈ 0.36 s.

**Gray state** `DAT_00493de0` (0x432c05, every second while editing):
`1` if memory is low (0x4109d4) or the view offscreen isn't full size;
then every slot shows the stipple and picking parts is refused (alert 9).

### 3.7 Picking up and placing a part (`CWPlaceGadgetTool`)

Mouse down on a slot (`CWTrayToolbar` press 0x432f16 → 0x42e880), only if
the slot's `count != 0`:

1. Select the Pointer tool; `UpdateWindow(tray)`; deselect all (0x4253c8);
   flush the redraw.
2. If memory is low: alert 9, stop. Else push an undo step (0x4154b4);
   if the count is limited, decrement it and redraw the area.
3. Create the tool (0x439237, `view, index, point in view coords`):
   `SetCapture(view)`, **`ClipCursor` to the view's client rect**, cursor
   **6** (§6), sound 0x93; create the gadget with the factory
   (`0x41f194(pane, tag, (−2000,−2000), fresh)`); wrap it in a drag
   sprite whose hot spot is the **center** of the gadget's bounds:
   `(⌊w/2⌋, ⌊h/2⌋)` of its bounds rect `+0x18`. Copy both toolbars'
   offscreens into the view's offscreen at their window positions and
   **hide both toolbars** (`SW_HIDE` + `ValidateRect`), so the part can be
   dragged over them while they still appear on screen.
4. Mouse move (slot 0, 0x439b64): move the sprite: gadget position =
   cursor − hot spot (no snapping, no grid).
5. Mouse up (slot 8, 0x4395f5): show both toolbars again. If the cursor is
   inside the tray's (hidden) window rect: the part goes back: its count
   +1, the sprite and the gadget are destroyed. Else: sound 0x92, mark
   the document dirty (0x416142), final position = cursor − hot spot,
   add the gadget to the pane (0x42450c) and invalidate it. Either way the
   toolbar rects are redrawn and the tool deletes itself (0x4346f3 → `0x464`).

The button is still down from the tray click; the drag needs no second
click.

## 4. The worksheet window

### 4.1 Offscreen and palette

- One `CWinGDib` (`view+0x135c`, 8-bit top-down DIB section via WinG when
  available) the size of the view's client area. Everything on the
  worksheet is drawn into it; `OnDraw` (0x433e52) blits it at (0, 0). If
  the client is bigger than the offscreen (allocation failed), the client
  is first filled with `WHITE_BRUSH` (light on) or `BLACK_BRUSH` (light
  off). Printing scales the offscreen to the page, centered, aspect kept.
- The palette (0x436519) is built from `WIDGET.EXE` type 1003 id 101 (256
  `RGBQUAD`s; `0x41082a(…, 0x3eb, 0x65)`, created at 0x436747). On a
  palette device the 20 system colors keep slots 0..9 and 246..255 (the
  resource already has the standard system colors there: 0 black, 7
  (192,192,192), 248 (128,128,128), 255 white, …), entries 10..245 come
  from the resource with `PC_NOCOLLAPSE`. For the port: use the resource
  as the palette as is. Colors given as RGB (fills) map to the nearest
  index (0x43746e, `GetNearestPaletteIndex`): white → 255, black → 0,
  0x808080 → 248.
- Each toolbar and each `cDialogDocItem` has its own `CWinGDib` too.

### 4.2 Redraw scheme

The pane keeps a dirty Mac rect (`+0x2d`) and a flag (`+0x2c`).
`0x4236ed(pane, rect)` unions a rect into it (every gadget's slot 0xc,
"invalidate", does this). `0x4234b8(pane, mode)` flushes: if dirty: for
modes 0, 1, 2 `InvalidateRect(view, dirty)`; clear; unless mode 2,
re-render the dirty rect into the offscreen:

- running (0x423b59): background (0x43d209), every gadget's draw (slot 4),
  every wire (0x43a489), the frame (0x43d39b);
- editing (0x42397d, clipped to the dirty rect with a region): background,
  gadgets (twice over, the selected one with its selection frame, slot 8),
  wires (the selected wire highlighted 0x43bb96), the selected joint
  (0x43cdfd), the frame.

Modes 1 and 2 also `UpdateWindow(view)`; mode −1 renders without
invalidating (used before sprite drags). `0x425316(pane)` = dirty the
whole work area and flush (mode 0). `0x434794(view, rect|NULL)` /
`0x4347ef(view, mode)` are the view-side wrappers.

During drags (moving parts, wires, the new-part tool) a
`cOffscreenUpdateList` (0x4935a8) of `cGadgetUpdateElement` /
`cConnectionSegmentUpdateElement` / `cKnotUpdateElement` sprites is used:
the dragged objects are hidden from the normal render, the rest is drawn
once, then each step restores the saved background (slot 0xc), moves the
object, saves the new background and draws the object (slot 8), and
reports the dirty rect (slot 0x10), which is invalidated and
`UpdateWindow`ed. A port can simply re-render the frame.

### 4.3 Background (`back` chunk, Options menu)

Background object (`pane+0x24`, ctor 0x43d0c4): `+0` i16 type, `+2` Mac
rect (= the work area), `+0xa` the type's sheet, `+0xe` sheet **704** (the
frame), `+0x12` a `CWinGDib` tile.

| type (`back` value, u16 BE) | menu item | sheet | tile |
|---|---|---|---|
| 0 | Plain Background (32780) | – | none: fill white (light on) / black (light off) |
| 701 | Metal Waves (32781) | 701 (96×96) | frame 1 (light on) / 2 (light off) |
| 700 | Noise (32782) | 700 (122×112) | frame 1 / 2 |
| 702 | Steel (32783) | 702 (128×128) | frame 1 / 2 |

The tile is pre-rendered (0x43dc4a) into the `CWinGDib` and redone when
the light toggles. Drawing (0x43d209, clip rect): plain → fill the clip;
else tile from the work area's top-left in steps of the tile size, each
copy clipped. A new document has type 0 (the pane reset 0x424b27, called from the file code at 0x415xxx,
deletes everything and sets type 0); "Clear Work Area" keeps the
background. Loading a file sets it from the `back` chunk (0x42503e). Changing it: undo step,
replace the picture, re-check the menu (0x414dcc), mark dirty, full
redraw.

**The frame** (0x43d39b) is drawn last, over the gadgets and wires, and
**only for types ≠ 0**. Sheet 704 (pipes): frames 1, 3, 5, 7 are the
84×54 corners, 2 and 6 the 192×11 horizontal pieces, 4 and 8 the 11×158
vertical pieces. With the work area `(L, T)-(R, B)` and frame `n`'s size
`wn×hn`:

- frame 2 tiled along the top: at `(x, T)` for `x = L, L+192, …` while
  `x < R` (only if the clip's top < h2);
- frame 6 along the bottom: at `(x, B − h6)` (only if `B − h6 < clip.bottom`);
- frame 8 along the left: at `(L, y)` for `y = T, T+158, …` while `y < B`
  (only if `clip.left < w8`);
- frame 4 along the right: at `(R − w4, y)` (only if `R − w4 < clip.right`);
- corners: 1 at `(L, T)`, 3 at `(R − w3, T)`, 7 at `(L, B − h7)`, 5 at
  `(R − w5, B − h5)`.

All pieces are clipped to the clip rect; the sheet's magenta is
transparent. Since the work area is the client area, the pipes run along
the window edges, under the floating toolbars.

### 4.4 Light off

`DAT_00493ddc == 0`: plain background black, textured backgrounds use
frame 2 (dark), the area outside the offscreen is black; the gadgets draw
their own dark variants (gadget docs). Toolbars are unchanged.

## 5. Menus (`RT_MENU` 3) and accelerators

Menu 2 is a placeholder (one item " ", 32784). Menu 3:

| menu | item | id | initial flags | handler | does |
|---|---|---|---|---|---|
| File | &New | 57600 (0xe100) | | app 0x43327a | MFC `OnFileNew`, then 0x415865 (reset document) |
| | &Open... | 57601 | | doc 0x4337c4 → 0x415936 | open a `.wgt` |
| | Load Puzzle... | 32771 (0x8003) | | app 0x4332b6 → 0x416f1a | puzzle dialog (puzzles doc) |
| | &Save | 57603 (0xe103) | grayed | doc 0x4337d9 → 0x415a3b | save (may offer the 10010 squeeze, §7) |
| | Save &As... | 57604 | | doc 0x4337ea → 0x415c1f | |
| | Revert | 32772 (0x8004) | grayed | doc 0x4337fb → 0x415ece | alert 1/2 ("Revert to last saved version?" / "…original puzzle?") |
| | Import a Stand-Alone Widget... | 32773 | | app 0x433294 → 0x416bf5 | |
| | Save Into a Stand-Alone Widget | 32774 | | app 0x4332a5 → 0x4165a7 | (offers the 10014 squeeze) |
| | Print Preview | 57608 | | view, MFC | |
| | Print Setup... | 57606 | | app, MFC | |
| | Print | 57607 | | view, MFC | |
| | E&xit | 57665 | | MFC | save prompt "Save changes to %s?" (0x410b24, Yes/No/Cancel) |
| Edit | &Undo | 57643 (0xe12b) | grayed | app 0x4332e9 → 0x415546 | undo (other doc) |
| | Clear Work Area | 32777 | | app 0x43335a → 0x425cda | undo step, deselect, delete every unlocked part and wire (all if `pane+0x55`), redraw. No confirmation. Background kept. |
| | Remove All Connections | 32778 | | 0x43336b → 0x425dbe | undo step, delete every unlocked wire |
| | Remove All Joints | 32779 | | 0x43337c → 0x426407 | undo step, remove the bend points of every unlocked wire (0x43c8c6) |
| | Fit Work Area ... | 32785 | | 0x43338d → 0x4294fe | §7 |
| Options | Plain Background | 32780 | checked | 0x4332fa → 0x43d8c8 | §4.3 |
| | Metal Waves | 32781 | | 0x433312 → 0x43d946 | |
| | Noise | 32782 | | 0x43332a → 0x43da08 | |
| | Steel | 32783 | | 0x433342 → 0x43daca | |
| &Help | &About Widget... | 57664 (0xe140) | | app 0x433235 → 0x429ca5 | §7.1 |
| | Contents F1 | 32784 | | app 0x4333f5 | `WinHelp(<exe dir>\<exe name>.HLP, HELP_CONTENTS)`; "Can't find help file" retry loop |

Options items are radio-checked by 0x414dcc (only the current background
checked). There are no `ON_UPDATE_COMMAND_UI` entries and
`OnInitMenuPopup` is empty; instead **0x414b7e** runs on GO/STOP and
document changes:

- running: disable About, New, Open, Load Puzzle, Save, Save As, Revert,
  Import, Save Into, Print Setup, Print, Print Preview, Fit Work Area,
  Undo, Clear, Remove All Connections, Remove All Joints. (Exit, Options
  and Contents stay enabled.)
- editing: enable all of them except: **Undo** iff there is something to
  undo (0x414d0d / 0x41566d), **Save** iff the document is dirty, **Revert**
  iff dirty and (it has a file or a puzzle is loaded) (0x414d47).

Accelerators (`RT_ACCELERATOR` 2): Ctrl+N New, Ctrl+O Open, Ctrl+S Save,
Ctrl+Z Undo, Ctrl+X 57635 (Cut, no handler → ignored), Ctrl+C 57634 /
Ctrl+V 57637 (ignored), Ctrl+P Print, F1 Contents, Alt+Backspace Undo,
Shift+Del 57635, Ctrl+Ins 57634, Shift+Ins 57637 (ignored), F6 / Shift+F6
next/previous pane (MFC, no effect with one pane).

Other keys (view `WM_KEYDOWN` 0x434451 → pane 0x42385d, virtual-key
codes): editing: **Delete** (0x2e) deletes the selection (0x423c2d), **L**
(0x4c) or numpad separator (0x6c) toggles the lock of the selected part /
wire, **numpad +/−** change the tray count of the selected part's type;
then Enter/Esc (§3.3), then the tray's Shift/Ctrl/numpad ± (§3.5).

## 6. Cursors

`DAT_004a2d60` holds 7 cursors (0x439b88): on Win95/NT the 8-bit group
cursors **5007..5013** (falling back to 5000+i), on Win 3.x the
monochrome **5000..5006**; slot 8 holds `IDC_WAIT`. `0x439d44(mgr, i)`
sets cursor `i`. The view's `WM_SETCURSOR` (0x434550): if a tool is
active, its slot 0x20 sets it (the new-part tool: 6; the base input tool:
0); else, when cursor updates are on (`app+0xd4`), `i = 0x425909(pane,
cursor)`:

| i | group (Win95 / 3.x) | hot spot | picture | used when (0x425909) |
|---|---|---|---|---|
| 0 | 5007 / 5000 | (0,0) | white glove pointing up-left | default; running unless Move tool; locked objects |
| 1 | 5008 / 5001 | (4,4) | scissors | Cut tool (editing) |
| 2 | 5009 / 5002 | (2,8) / (3,9) | hand holding a green plug | Pointer tool over a knot (within 14 px, `d² < 197`): start a wire |
| 3 | 5010 / 5003 | (12,13) | pointing hand | while dragging parts / a joint (0x426878, 0x426b94) |
| 4 | 5011 / 5004 | (7,7) | open hand | Move tool (editing or running) |
| 5 | 5012 / 5005 | (2,8) / (3,9) | hand holding a blue plug | Pointer tool over a wire joint (bend point) |
| 6 | 5013 / 5006 | (12,13) | pointing finger | Pointer tool over a part or a wire; the new-part tool |

Over a window other than the view (the toolbars) the class cursor of
`"AfxWnd"` (the arrow) is used. Group cursors 5000..5013 → `RT_CURSOR`
8, 25..30 and 31..37.

## 7. Dialogs and pop-ups from the editor

### 7.1 About (`CAboutWnd`, 0x429ca5; command 57664)

Not a dialog: a borderless child window (`WS_CHILD|WS_VISIBLE|
WS_CLIPSIBLINGS`) the size of **sheet 10001 frame 1 (344×271)**, centered
in the view's client area, put above the toolbars (both toolbars pushed
to the bottom of the z-order and restored after). It captures the mouse
and runs its own loop until the left or right button, Space or Esc is
down (0x415300). Its message map (0x493764) only records clicks/keys.

- Offscreen 344×271 filled black, sheet 10001 frame 1 drawn at (0,0),
  blitted.
- Scroll rect **(36,97)-(310,210)** (274×113). Font: Helvetica 16 bold
  (CWFontSet 0x41794d), text color **RGB(0x8f,0xc7,0xf7)**, background
  color black (opaque).
- `total = textHeight + 232 + 210`, where `textHeight` = `DrawText(credits,
  DT_CALCRECT|DT_CENTER|DT_WORDBREAK|DT_NOPREFIX)` in the scroll rect;
  credits = `WIDGET.EXE` type 1020 id 10000 (whole resource as text).
- Every **> 5 ticks** (1/60 s) the offset `s` (from 0) advances by 1:
  fill the scroll rect black; if `s < 232` draw **sheet 10004 frame 1**
  (274×232, title/copyright) at `(36, 97 − s)` clipped to the scroll rect;
  if `s < 200` draw string 100 "Version 1.0" in `(36, 97+66−s)-(310,210)`
  with `DT_CENTER|DT_NOPREFIX`; if `97 + 232 − s < 210` draw the credits
  in `(36, 97+232−s)-(310,210)` with `DT_CENTER|DT_WORDBREAK|DT_NOPREFIX`;
  blit the scroll rect; `s += 1`; when `s > total`, `s = −113` (it comes
  back in from below).
- Easter eggs: holding **Shift+V** shows string 101 ("Internal notes:
  Release version Win'95 1.23") in the scroll rect (refreshed every 50
  ticks); holding **V+E+I+K** shows sheet 901 frame 1 (174×182) centered
  in the window. Releasing either redraws the whole picture.
- Calls the sound module on the first loop (0x44084b/0x440862 →
  0x4400e2) and on exit (0x440871 → 0x4400f3); probably about music, not
  checked.

### 7.2 Fit Work Area (32785, 0x4294fe)

`bbox` = union of every part's bounds (position + bounds rect) and every
wire's extent (0x4291d9). Then:

- empty → message box **"Nothing to fit"** (0x493604);
- `bbox.left ≥ 15 && bbox.top ≥ 15 && bbox.right ≤ 625 && bbox.bottom ≤
  465` → **"Your widget is already fit to any Windows screen"** (0x493613);
- else dialog **10014** (`CStdWinFitRectDialog`, 0x429591): "Do you want to
  automatically squeeze the parts together so they'll all fit on your (or
  a smaller) monitor?", radio buttons 101..105 = 640×480 ("Best
  Macintosh-compatible size"), 720×540 ("Maximum Macintosh-compatible
  size"), 800×600, 1024×768, 1280×1024; the choice is remembered in the
  profile key **`FitSize`** (0..4, 0x429ad6 checks it on open); Yes (6) /
  No (7) / Cancel (2). On Yes with size `W×H`: squeeze into `(14, 14)-(W −
  14, H − 14)` (0x428f1c: `sx = min(1, (W−28)/bbox.w)`, `sy` likewise;
  offsets so the box lands inside; 0x429060 moves every part to `pos·s +
  o` and rescales the wires' joints), with an undo step and dirty mark.

On **save** (0x42932b) the same test against the Mac size `(14,14)-(706,
526)` opens dialog **10010** (`CWrksFitDialog`: "This widget is too big to
share with friends who have the Macintosh version… squeeze…?", Yes/No/
Cancel, "Remember my answer", profile key `Fit`); Yes squeezes to that
rect. (Dialog 10012, "fit worksheet elements to the rectangle W x H
pixels", is unused.)

(The profile details, No vs Cancel, and the port: editor-interaction.md
5.5; the file dialogs and stand-alone widgets: 8.5, 8.6.)

### 7.3 Alerts (`cAlert`, `DAT_004a2668`)

`0x410b5b(alert, n)` / `0x410bb8` (returns the answer): message box with
string `8000 + n`:

| n | text |
|---|---|
| 1 | Revert to last saved version? |
| 2 | Revert to original puzzle? |
| 3 | An error occurred while reading the file. |
| 4 | An error occurred while writing to the file. Your disk is probably full |
| 5 | Cannot find puzzle file. … WIDGET.PZL. |
| 6 | Cannot find resource file. … WIDGET.SLF |
| 7 | This file is not a Widget Workshop saved widget |
| 8 | This file is not a Widget Workshop Stand Alone Widget |
| 9 | Widgets is running out of memory, and has become unstable. Please save your work now. … |
| 10 | Insufficient memory to perform operation |

Plus MFC message boxes with literal strings: "Save changes to %s?"
(Yes/No/Cancel; Yes while running first presses STOP), "Nothing to fit",
"Your widget is already fit…", "Can't find help file", "Can't launch a
second copy of Widget Workshop", wave import errors (0x43deda).

### 7.4 Doc, the help character (`cDialogDocItem`, 0x417df8)

Every gadget settings dialog (templates 1000.., 2000..) has, besides the
gadget picture (button 102, `"Pic…"`, drawn by 0x418cd0 with sheet 504's
knot arrows), two owner-drawn controls:

- **103** "Doc": the balloon area, e.g. (107,14,124,70) DLU;
- **150**: the Doc button, (201,65,31,19) DLU, overlapping the balloon's
  bottom-right corner.

The dialog's init (0x41acb9, case control 0x67) reads the gadget's
**type 1021** info (`id, kind, text1, text2`, u16 each) and builds the
item with texts **1020/`text1`** (the description, e.g. 1000 "The
Pendulum lets you…") and **1020/`text2`** ("Construction Tip: …", e.g.
3000), from `WWGADGET.DLL`.

Construction: one offscreen covering the union of control 103's client
rect and control 150's rect (in 103's coordinates); filled with the
dialog's background brush (`WM_CTLCOLORDLG`). Balloon rect `B` = 103's
client rect; text rect `T` = `B` inset 4 px; with `H = T.height`, three
fonts **MS Sans Serif, heights H/9, H/12, H/12** (normal weight, PROOF
quality; the third equals the second, a quirk). Doc's face = **sheet 505**
(4 frames, 32×30), placed at `(off.right − 33, off.bottom − 31)` of the
offscreen.

Render (0x4186f9, `which` = 0 text1 / 1 text2): erase `B` (+1 px) with the
dialog brush and a null pen; **white `RoundRect(B, 20, 20)`** without
outline; select font 1; measure with `DrawText(DT_CALCRECT|DT_WORDBREAK|
DT_NOPREFIX)` in `T` (bottom +100); if taller than `T`, font 2, measure
again; if still taller, font 3; draw with `DT_WORDBREAK|DT_NOPREFIX`
(black on white); draw the face; invalidate both controls. The string
"Doc text height=%hi, Avail height=%hi" (0x491646) is an unused debug
format.

Face frame (0x418c05): not pressed → **1** (showing text1) / **3**
(showing text2); pressed (button `ODS_SELECTED`) → **2** / **4**.
Clicking button 150 (`WM_COMMAND` 0x96 in 0x41a686 → 0x418ca0) toggles between text1
and text2. `WM_DRAWITEM` (0x41a912): control 103 blits the offscreen at
(0,0) (0x418ae2); control 150 redraws the face and blits its part of the
offscreen (0x418b22).

## 8. Fonts

| where | face | height | weight | created at |
|---|---|---|---|---|
| tray help line | Small Fonts (0x494420) | 12 | normal | 0x42db12 |
| CWFontSet #1 (0x4a2715, getter 0x417915) | Small Fonts (0x4914e0) | 11 | normal | 0x41783b |
| CWFontSet #2 (0x4a2725, 0x417931) | Helvetica (0x4914ec) | 16 | normal | 0x41783b |
| CWFontSet #3 (0x4a271d, 0x41794d) | Helvetica (0x4914f6) | 16 | **bold** (700) | 0x41783b |
| CWFontSet #4 (0x4a272d, 0x417969) | Helvetica (0x491500) | 24 | normal | 0x41783b |
| Doc balloon | MS Sans Serif (0x49161c / 0x49162a / 0x491638) | H/9, H/12, H/12 | normal | 0x417df8 |
| dialogs | MS Sans Serif 8 pt (templates) | | | |

All via `CreateFontA(h, 0, 0, 0, weight, 0, 0, 0, ANSI_CHARSET,
OUT_DEFAULT, CLIP_DEFAULT, PROOF_QUALITY, DEFAULT_PITCH, face)`; heights
are cell heights in pixels. The editor itself uses only the help-line font
and Helvetica 16 bold (About); the CWFontSet fonts are otherwise used by
gadgets (users in 0x448xxx, 0x450xxx, 0x45axxx, 0x45bxxx). "Helvetica"
is not a Windows font, so GDI substitutes (usually Arial).

## 9. UI sound ids

`0x4402cc(id, flag)` (silent when Sound is off): 0x80 button click, 0x8c
light on, 0x8d light off, 0x8e GO, 0x8f STOP, 0x90 sound on, 0x91 sound
off, 0x92 part dropped, 0x93 part picked up, 0x94 tray arrow, 0x95 tray
slide, 0x82/0x83 wiring, 0x85/0x86 + loop 0x96 moving parts. Which
resource each plays is in the sound code (not checked here).

## 10. Globals

| addr | what |
|---|---|
| `DAT_00493dde` (u16) | 1 editing (STOP), 0 running |
| `DAT_00493ddc` (u8) | light on |
| `DAT_00493ddd` (u8) | sound on |
| `DAT_00493de0` | tray grayed |
| `DAT_00493de4` | main toolbar has painted (Enter/Esc allowed) |
| `DAT_00493dd8` | the help-line element |
| `DAT_004a2848` | `cWorksheetPane` |
| `DAT_004a2864` | `CWidgetTbar` (tray) |
| `DAT_004a2868` | `CMainTbar` |
| `DAT_004a2944` | `CWidgetView` |
| `DAT_004a2d4e` | the HPALETTE |
| `DAT_004a2d60` | cursor set |
| `DAT_004a2668` | `cAlert` |
| `DAT_00490170` | `WIDGET.EXE` module (cursors, palette, credits) |
| `DAT_0049016c` | `WWGADGET.DLL` |

View fields: `+0x34` `CMainTbar`, `+0x50` its HWND, `+0x64` its offscreen,
`+0x6e0` the tray, `+0x6fc` its HWND, `+0x710` its offscreen, `+0x1344`
active tool, `+0x1348` offscreen full size, `+0x135c` the offscreen.

## 11. Proposed names

| address | name |
|---|---|
| 0x41783b | CWFontSet_CreateFonts |
| 0x417915 / 0x417931 / 0x41794d / 0x417969 | FontSet_SmallFonts11 / Helvetica16 / Helvetica16Bold / Helvetica24 |
| 0x417092 | Doc_UpdateWindowTitle |
| 0x417df8 | cDialogDocItem_ctor |
| 0x4186f9 | cDialogDocItem_RenderText |
| 0x418ae2 | cDialogDocItem_PaintBalloon |
| 0x418b22 | cDialogDocItem_PaintButton |
| 0x418c05 | cDialogDocItem_DrawFace |
| 0x418ca0 | cDialogDocItem_ToggleText |
| 0x418cd0 | cDialogGadgetPic_ctor |
| 0x410b5b / 0x410bb8 | cAlert_Show / cAlert_Ask |
| 0x410b24 | AskSaveChanges |
| 0x410600 | GetWindowsKind (0 Win3.x, 1 Win95, 2 NT) |
| 0x4234b8 | Pane_FlushRedraw |
| 0x4236ed | Pane_InvalidateRect |
| 0x42397d | Pane_RenderEditing |
| 0x423b59 | Pane_RenderRunning |
| 0x423c2d | Pane_EditKey |
| 0x42385d | Pane_KeyDown |
| 0x424a12 / 0x424a94 | Pane_StartRun / Pane_StopRun |
| 0x425316 | Pane_RedrawAll |
| 0x425909 | Pane_CursorForPoint |
| 0x425cda / 0x425dbe / 0x426407 | Pane_ClearWorkArea / Pane_RemoveAllConnections / Pane_RemoveAllJoints |
| 0x4263f1 | Pane_GetBackground |
| 0x426629 | Pane_SetWorkAreaSize |
| 0x4277d9 / 0x4279e2 | DragSprite_ctor / DragSprite_MoveTo |
| 0x428f1c | Fit_ComputeTransform |
| 0x429060 | Fit_ApplyTransform |
| 0x4291d9 | Pane_ContentBounds |
| 0x42932b | Fit_OfferMacSqueezeOnSave |
| 0x4294fe | OnFitWorkArea |
| 0x429591 | Fit_AskScreenSizeAndSqueeze |
| 0x4298b9 / 0x429ad6 | CStdWinFitRectDialog_OnCommand / _OnInitDialog |
| 0x429ca5 | CAboutWnd_Run |
| 0x42d01e…0x42d570 | cPuzzleDlg (puzzles doc) |
| 0x42db12 | CWStaticText_Init |
| 0x42db7c | CWToolbarElement_ctor |
| 0x42dbe6 | CWToolbarElement_Init |
| 0x42dcb0 | CWToolbarElement_Draw |
| 0x42dd17 | CWBmpButton_Press (toggle) |
| 0x42dd8c | CWBmpRadioButton_Press |
| 0x42ddd4 | CGroupBox_Select |
| 0x42de8d / 0x42df32 | CWBmpPushButton_Press / _Release |
| 0x42da30 | CWStaticText_Draw |
| 0x42dfac | CWScrollNumber_Draw |
| 0x42e001 | CWTraySlot_ctor (page) |
| 0x42e0ff | CWTraySlot_AddPart (layout) |
| 0x42e360 | CWTraySlot_Draw |
| 0x42e7da | CWTraySlot_HitTest |
| 0x42e880 | CWTraySlot_PickUp |
| 0x42ebc5 | CWTrayFolder_ctor |
| 0x42ecea | CWTrayFolder_AddPart |
| 0x42edc5 / 0x42ee6e | CWTrayFolder_NextPage / _PrevPage |
| 0x42ef7e / 0x42f060 | TrayFolder_ReturnPart / _AdjustCount |
| 0x42ef11 / 0x42f1c0 / 0x42f2c8 / 0x42f14d | TrayFolder_CountLimited / _SaveCounts / _LoadCount / _ResetCounts |
| 0x42f3a7 | CWTrayToolbar_Init |
| 0x42f3e2 | CWTrayToolbar_AddFolder |
| 0x42f419 | CWTrayToolbar_SwitchFolder |
| 0x42f592 | Tray_SlideAnimation |
| 0x42fa33 | CWTbar_ctor |
| 0x42fb3b | CWTbar_Create |
| 0x42fccc / 0x42febc / 0x4302f7 | CWTbar_OnLButtonDown / _OnMouseMove / _OnLButtonUp |
| 0x430172 | CWTbar_DrawDragOutline |
| 0x4304b5 | CWTbar_RenderAll |
| 0x430536 | CWTbar_OnPaint |
| 0x4305c4 | CWTbar_OnElementCommand |
| 0x4305f2 | CMainTbar_Create |
| 0x430d2c | CMainTbar_OnElementCommand |
| 0x430e70 / 0x430ee7 | Editor_Go / Editor_Stop |
| 0x430f63 | CMainTbar_OnKey |
| 0x430fb2 | CWidgetTbar_ctor |
| 0x431281 | CWidgetTbar_Create |
| 0x432229 / 0x432267 | Tray_PartPictureRect / Tray_DrawPartPicture |
| 0x4322e5 | CWidgetTbar_UpdatePageControls |
| 0x4324c8 | CWidgetTbar_OnElementCommand |
| 0x4325e0 | CWidgetTbar_SelectTool |
| 0x432683 | CWidgetTbar_GetTool |
| 0x4326e0 / 0x432824 | CWidgetTbar_OnKeyDown / _OnKeyUp |
| 0x432867 / 0x432921 | CWidgetTbar_ReturnPart / _AdjustCount |
| 0x4329e0 / 0x432a57 / 0x432aa5 / 0x432b3c | CWidgetTbar_ResetCounts / _CountsSize / _SaveCounts (`numG`) / _LoadCounts |
| 0x432c05 / 0x432c92 | CWidgetTbar_UpdateGray / Tray_IsGray |
| 0x432d65 | CWBmpRadioButton_ctor |
| 0x432e4a / 0x432ea9 / 0x432f16 | CWTrayToolbar_HitTest / _Draw / _Press |
| 0x432fbe | CWGrayTraySlot_Draw |
| 0x43308b | CWidgetApp_InitInstance |
| 0x433235 | OnAbout |
| 0x4333f5 | OnHelpContents |
| 0x4338e6 / 0x433afa | CWidgetView_ctor / _dtor |
| 0x433dfa | CWidgetView_PreCreateWindow |
| 0x433e52 | CWidgetView_OnDraw |
| 0x4340f0 | CWidgetView_OnCreate |
| 0x4341c0 | CWidgetView_OnDestroy |
| 0x43424e | CWidgetView_OnSize |
| 0x434550 | CWidgetView_UpdateCursor |
| 0x434600 | CWidgetView_PreTranslateMessage |
| 0x43469e / 0x4346f3 | CWidgetView_SetTool / _EndTool |
| 0x434794 / 0x4347ef | CWidgetView_Invalidate / _Flush |
| 0x43480e | CWidgetView_OnStartup |
| 0x434ac8 | CWidgetView_OnDropFiles |
| 0x434b0d | CWidgetView_OnTimer |
| 0x435796 | CMainFrame_ctor |
| 0x4358dd | CMainFrame_OnNcPaint |
| 0x4359f7 | CMainFrame_OnActivateApp |
| 0x435c60 / 0x435c93 | TrayIndexToTag / TagToTrayIndex |
| 0x436519 / 0x436747 | BuildLogPalette / CreateGamePalette |
| 0x436dd4 / 0x436e58 | CWinGDib_ctor / CWinGDib_Resize |
| 0x437117 / 0x43718f | CWinGDib_BlitAll / CWinGDib_BlitRect |
| 0x43746e / 0x437481 | NearestPaletteIndex / CWinGDib_FillRect |
| 0x439237 / 0x439580 | CWPlaceGadgetTool_ctor / _dtor |
| 0x439b64 / 0x4395f5 | CWPlaceGadgetTool_OnMouseMove / _OnLButtonUp |
| 0x4390d5 | CWInputTool_Dispatch |
| 0x439b88 / 0x439d44 | Cursors_Load / Cursors_Set |
| 0x43d0c4 | Background_ctor |
| 0x43d209 | Background_Draw |
| 0x43d39b | Background_DrawFrame |
| 0x43d8c8 / 0x43d946 / 0x43da08 / 0x43daca | Background_SetPlain / _SetMetalWaves / _SetNoise / _SetSteel |
| 0x43db97 | Background_SetType |
| 0x43dc4a | Background_RebuildTile |
| 0x414b7e | Menu_UpdateForRunState |
| 0x414d0d / 0x414d47 | Menu_UpdateUndo / Menu_UpdateSaveRevert |
| 0x414dcc | Menu_CheckBackground |
| 0x4149c8 / 0x414a26 / 0x414aa8 | Menu_Enable / Menu_Disable / Menu_Check |
| 0x4402cc | PlayUISound |
| 0x415300 | IsClickOrKeyDown |

## Open questions

- The exact resources behind the UI sound ids (0x80..0x96) and the About
  music were not traced.
- `0x4359ab`/`0x4359d1` (frame black border) look unused; not verified
  against every vtable.
- Cursor 3's exact trigger set (it's set by the part-move and joint-drag
  loops) belongs to the worksheet-interaction doc.
- "Light off" rendering of each gadget is the gadgets' business; only the
  background/frame side is covered here.
- The menu-update calls after document changes (who calls 0x414d0d /
  0x414d47 besides 0x414b7e) were not enumerated.
