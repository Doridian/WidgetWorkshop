# Editor interaction: the worksheet

How `WIDGET.EXE` (Win95 build) lets the player build and run a widget on
the worksheet: tools, hover-selection, wiring, joints, moving, cutting,
undo, Fit Work Area, the settings dialogs, run/stop, and the file flow.
Read [gadget-model.md](gadget-model.md) first; this doc uses its terms and
corrects a few of its field names (see "Corrections to gadget-model.md"
at the end).

All addresses are in `WIDGET.EXE`. "Point" is a Mac `Point {i16 v (y), i16
h (x)}`, "Rect" a Mac `Rect {top, left, bottom, right}` (half-open:
`left <= x < right`, `top <= y < bottom`). `SetRect(r, l, t, r, b)` below is
`0x41195a` (it takes Windows order and stores Mac order). All worksheet
coordinates are client pixels of the view window (`CWidgetView`), origin
top left; there is no scrolling and no zoom.

Terminology. The binary's own class names (RTTI strings) differ from
gadget-model.md in one place, so to be explicit:

| this doc | what | binary's name |
|---|---|---|
| **knot** | a gadget's connection point (10 bytes, from resource 1014) | no RTTI (plain struct) |
| **wire** | `cConnection`, 0x2b bytes | `cConnection` |
| **joint** | a bend point in a wire (9 bytes); the Edit menu calls them "Joints" | **`cKnot`** (RTTI string at 0x43cb2a; its update element is `cKnotUpdateElement`) |
| **worksheet** | the object holding gadgets and wires, global `DAT_004a2848` | **`cWorksheetPane`** (0x56 bytes, vtable 0x49326c, base `cAbstractPane` vtable 0x492fc4) |

The task brief calls the worksheet "CWidgetDoc"; the MFC `CWidgetDoc`
(vtable 0x494bcc) is only a shell whose four commands forward to the
`cFile` object. Every field below (+0x10 gadget list, +0x4b timing,
+0x53 version...) is on `cWorksheetPane`.

---

## 1. Objects and globals

| global | object | notes |
|---|---|---|
| `DAT_004a2848` | `cWorksheetPane*` | created by the view ctor (0x4338e6 → 0x423268) |
| `DAT_004a2944` | `CWidgetView*` | the main view; its client area is the worksheet |
| `DAT_004a2864` | tray + tool pane (`CWTrayToolbar`, view+0x6e0, HWND at view+0x6fc) | holds the Pointer/Cut/Move radio buttons and the parts tray with counts; **hidden while running** |
| `DAT_004a2868` | control panel (view+0x34, HWND at view+0x50) | GO/STOP, Light, Sound buttons; stays visible while running |
| `DAT_00493dde` | u8 mode: **1 = edit, 0 = running** | starts at 1 (0x4305f2) |
| `DAT_004a2704` | `cUndo*` (9 bytes) | one-level undo |
| `DAT_004a270c` | `cFile*` | file name, dirty flag (below) |
| `DAT_004a26ec` | `cMenuBar*` (5 bytes) | enables/disables menu items |
| `DAT_004a26f8` | `cKeyboard*` (1 byte) | `IsKeyDown(vk)` = `GetKeyState(vk) < 0` (0x41507c) |
| `DAT_004a2d60` | `CWCursor` cursor set | 7 cursors, see 2.4 |
| `DAT_004a2858` | `cPuzzle*` | `0x42d5a3` = "a puzzle is loaded" |
| `DAT_004a3624+0xd4` | app "active" flag | 0 while the app is in the background or inside a modal mouse loop; idle work is skipped then |

### 1.1 `cWorksheetPane` (0x56 bytes, ctor 0x423268, dtor 0x4233dc)

| offset | type | what |
|---|---|---|
| +0x00 | ptr | vtable 0x49326c |
| +0x04 | ptr | the view's offscreen DIB (view+0x135c), set by 0x423901 |
| +0x08 | Rect | work area: `SetRect(0,0,720,540)` at creation; replaced by `(0,0,cx,cy)` of the view's client size on every WM_SIZE (0x43424e → 0x426629). Nothing clamps gadgets to it. |
| +0x10 | list | **gadgets** (`cPtrList`: +0 head, +4 tail, +8 u16 count; node = {+0 next, +4 prev, +8 data}) |
| +0x1a | list | **wires** (same list type) |
| +0x24 | ptr | `cBackground` (0x45e bytes; +0 u16 background id 0/700/701/702) |
| +0x28 | ptr | `CWidgetView*` |
| +0x2c | u8 | "dirty rect is non-empty" |
| +0x2d | Rect | dirty rect (union of everything invalidated since the last redraw) |
| +0x35 | ptr | selected (hovered) wire |
| +0x39 | i16 | selected wire's segment index |
| +0x3b | ptr | selected (hovered) gadget |
| +0x3f | ptr | selected (hovered) joint |
| +0x43, +0x47 | u32 | zeroed in ctor, unused in my area |
| +0x4b | u32 | run clock: Mac ticks since the first tick of this run |
| +0x4f | u32 | run clock: tick time of the first tick (0 = not started) |
| +0x53 | u16 | `fver` of the last loaded file/undo snapshot |
| +0x55 | u8 | "may edit locked parts". Always 0: its only toggle (0x426488) is unreferenced. Every lock check is `locked && !ws+0x55`. |

vtable 0x49326c: +0x00 dtor 0x4233dc, **+0x04 MouseDown 0x423756**,
**+0x08 KeyDown 0x42385d**, +0x0c 0x423170 (calls +0x08), +0x1c
GlobalToLocal 0x4231f6 (base).

### 1.2 List order = draw order = hit order

`cPtrList` push_front is `0x4115a8`, push_back `0x4115f6`, insert-before
`0x4116bb`, unlink `0x41172e`.

- New gadgets (placed from the tray, or loaded) are **pushed to the
  front** (0x42450c). New wires likewise (0x424571).
- Hit tests walk **head → tail** (front first): the topmost item wins.
- Drawing walks **tail → head** (0x42397d / 0x423b59), so the head is
  drawn last = on top. All gadgets are drawn first, then all wires, so
  **wires are always above every gadget**.
- Ticking (0x42498f) walks head → tail (newest gadget ticks first).
- Saving (0x424d0a) walks tail → head; loading pushes each chunk to the
  front, so the order survives a save/load round trip.
- **Bring to front:** a Pointer-tool mouse-down on a gadget that isn't the
  head moves it to the head (0x423ec3, before anything else, also for
  locked gadgets and double-clicks); it then invalidates the gadget and the
  wires on its **output** knots (knot kind 1). Clicking a wire (to add a
  joint) moves the wire to the head of the wire list the same way. There
  is no menu command for ordering.

---

## 2. Mouse and keyboard plumbing

### 2.1 Message routing

`CWidgetView` message map (0x494c95) relevant entries:

| message | handler | does |
|---|---|---|
| WM_LBUTTONDOWN | 0x4343f2 | `ws->MouseDown(Point{y,x}, 0, dbl=0)` (slot 4) |
| WM_LBUTTONDBLCLK | 0x4344d2 | `ws->MouseDown(Point{y,x}, 0, dbl=1)` (the class has CS_DBLCLKS, 0x433dfa) |
| WM_LBUTTONUP, WM_RBUTTONUP, WM_MOUSEMOVE | 0x4343b4, 0x4343d3, 0x434531 | nothing (default) |
| WM_KEYDOWN | 0x434451 | if view+0x1340 (keys enabled; 0 only during the intro): `ws->KeyDown((u8)vk, 0)` (slot 8) |
| WM_KEYUP | 0x43449b | if view+0x1340: `0x4238e5(ws, vk)` → `0x432824` (end a temporary tool) |
| WM_SETCURSOR | 0x4345ed | `0x434550`: cursor (2.4) |
| WM_SIZE | 0x43424e | resize offscreen, set ws+0x08, redraw all |
| WM_TIMER (id 1, 1000 ms) | 0x434b0d | edit mode only: low-memory housekeeping |
| WM_DROPFILES | 0x434ac8 | open the dropped file (0x415936) |
| 0x464 (posted) | 0x43473a | delete a finished input tool |
| 0x465 (posted on first paint) | 0x43480e | intro dialog / command-line file (section 9) |

The view's `WindowProc` override **0x434600**: while an *input tool* is
active (view+0x1344 ≠ 0), every mouse message 0x200..0x209 and
WM_KEYDOWN/KEYUP/CHAR goes to the tool's slot 0x1c (`0x4390d5`, which maps
0x200→slot 0x00 move, 0x201→0x04 L-down, 0x202→0x08 L-up, 0x203→0x0c
L-dblclk, 0x204→0x10, 0x205→0x14, 0x206→0x18; keys fall through and are
swallowed) and nothing reaches the worksheet.

Most editing is **not** event driven: `MouseDown` runs a Mac-style modal
tracking loop that polls the mouse until the button is released:

- `StillDown()` = **0x415208**: true while the left *or right* button is
  down (`GetAsyncKeyState(VK_LBUTTON/VK_RBUTTON)`, or `GetKeyState` after
  pumping mouse messages when `DAT_00490924` is set).
- `GetMouse()` = **0x4153ac**: `GetCursorPos` (screen) as a Point;
  `0x425e44(ws, pt)` converts screen→view client, `0x425eaa` the reverse.
- `0x44081d` is called every loop iteration (sound/idle pump; not mine).

### 2.2 `MouseDown` (0x423756)

```
MouseDown(ws, pt, mods /*always 0*/, dbl):
  if mode == RUNNING:  RunClick(ws, pt, mods, dbl)                       // 0x423e3f
  else:  // edit
     ClipCursor(view client rect in screen coords); SetCapture(view)
     app.active = 0                     // stops idle hover/tick during the loop
     EditClick(ws, pt, mods, dbl)                                         // 0x423dc4
     ClipCursor(NULL); ReleaseCapture(); app.active = 1

EditClick:  switch tool():           // 0x432683 on the tray pane
     0 Pointer -> PointerDown(ws, pt, mods, dbl)                          // 0x423ec3
     1 Cut     -> CutDown(ws, pt)                                         // 0x42566c
     2 Move    -> PanWidget(ws, pt)                                       // 0x426313

RunClick:   if tool() == 2 (Move): PanWidget(ws, pt)
            else g = topmost gadget whose bounds contain pt (0x42391d)
                 if g: g->vtbl[0x14](g, pt, mods=0, dbl)     // the gadget's "click"
```

While running, the mouse only reaches gadgets through slot 0x14 on
button-down (and again with `dbl=1` on a double-click). There is no
mouse-up or mouse-move dispatch: gadgets that track the mouse (sliders,
switches, Mouse Move...) poll `StillDown()`/`GetCursorPos` inside their
slot 0x14 or tick. The cursor is not clipped while running.

### 2.3 `KeyDown` (0x42385d) and the temporary tools

```
KeyDown(ws, vk):
  handled = (mode == EDIT) ? EditKey(ws, vk) /*0x423c2d*/ : false   // 0x423dbd returns 0
  if !handled: handled = PanelKey(controlPanel, vk)                  // 0x430f63
  if !handled: handled = TrayKey(trayPane, vk)                       // 0x4326e0
```

`vk` is the WM_KEYDOWN virtual-key byte (auto-repeat included):

| vk | where | mode | effect |
|---|---|---|---|
| 0x2E **Delete** | EditKey | edit | if anything is selected (hovered): undo snapshot, dirty, then delete the selected wire (sound 0x88), gadget (0x87, returns it to the tray), joint (0x89) (0x42539a, 0x4244de, 0x425bac) |
| 0x4C **L** / 0x6C (numpad Separator) | EditKey | edit | toggle the *lock* flag of the selected gadget (+0x0c) and/or wire (+0x25) (designer feature; a locked item can't be hovered afterwards, so in practice it only locks) |
| 0x6B **numpad +** / 0x6D **numpad −** | EditKey, then TrayKey | edit | +1/−1 on the tray count of the selected gadget's type (0x432921); if nothing is selected, of the tray item under the cursor (undo snapshot + dirty) |
| 0x0D **Enter** | PanelKey | edit | start running (0x430e70) — only once the panel has painted (`DAT_00493de4`) |
| 0x1B **Esc** | PanelKey | running | stop (0x430ee7) |
| 0x10 **Shift** | TrayKey | edit only | if no temp tool is active (pane+0xc24 == 3): remember the current tool in pane+0xc24, select **Cut** |
| 0x11 **Ctrl** | TrayKey | edit *and* running | same, select **Move** |

WM_KEYUP of 0x10 or 0x11 (0x432824): if pane+0xc24 ≠ 3, reselect the
remembered tool and set pane+0xc24 = 3. (README: "hold Shift for Cut, Ctrl
for Move".)

Keyboard input to gadgets while running: none is dispatched. Gadgets
that react to keys (Key Press `Key_` 0x448da7, Stopper 0x457ba2) poll
`cKeyboard::IsKeyDown` = `GetKeyState`. While an input tool is active all
keys are swallowed.

### 2.4 Tools and cursors

Tools are a radio group on the tray pane (`DAT_004a2864`): buttons at
pane+0x4c4 = **0 Pointer**, +0x538 = **1 Cut**, +0x5ac = **2 Move**
(tooltips 4005/4006/4007 "Pointer Tool"/"Cut Tool"/"Move Tool");
`tool()` = 0x432683 compares the group's current button (pane+0x63c).
`SelectTool(pane, n)` = 0x4325e0 presses button n and refreshes the
cursor (0x434550). Starting a run, New/Open/Revert, and picking a part
from the tray all force the Pointer.

Cursor set `CWCursor` (`DAT_004a2d60`, 0x439b88): 7 cursors loaded from
RT_GROUP_CURSOR **5000+i** (monochrome) or, when `GetVersion` says Win95
or NT (0x410600 ≠ 0), **5007+i** (256-colour, falling back to 5000+i).
`SetCursorIndex(i)` = 0x439d44. Hot spots (from the resources):

| i | id (mono/colour) | picture | hot spot | used for |
|---|---|---|---|---|
| 0 | 5000 / 5007 | pointing glove | (0,0) | default |
| 1 | 5001 / 5008 | scissors | (4,4) | Cut tool |
| 2 | 5002 / 5009 | hand holding a plug | (3,9) / (2,8) | over a knot |
| 3 | 5003 / 5010 | grabbing hand | (12,13) | while dragging a gadget, wire end or joint |
| 4 | 5004 / 5011 | open hand | (7,7) | Move tool |
| 5 | 5005 / 5012 | hand with plug | (3,9) / (2,8) | over a wire |
| 6 | 5006 / 5013 | grabbing hand | (12,13) | over a gadget or joint; while an input tool (tray drag) is active |

`CursorFor(ws, pt)` = **0x425909** (pt in screen coords):

```
if RUNNING: return tool()==Move ? 4 : 0
switch tool(): Cut -> 1;  Move -> 4
Pointer:
  if WindowFromPoint(pt) != view: return 0
  pt = client(pt)
  j = JointAt(pt); w = WireAt(pt); g = TopGadgetAt(pt)
  k = (g && nearest knot of g within d² < 197) ? that knot : none
  if j:  return locked(j.wire) ? 0 : 6
  if k:  return (k has a wire && locked(wire)) ? 0 : 2
  if w:  return locked(w) ? 0 : 5
  if g:  return locked(g) ? 0 : 6
  return 0
```

WM_SETCURSOR (0x434550): if an input tool is active, its slot 0x20 sets
the cursor (CWInputTool: 0; CWorksheetDragTool/CWPlaceGadgetTool: 6);
otherwise, if the app is active, `SetCursorIndex(CursorFor(ws, GetCursorPos()))`.

---

## 3. Hover = selection (edit mode)

There is **no click-selection, no rubber-band selection and no
multi-selection**. The "selection" is whatever the pointer hovers over,
updated every idle cycle, and at most one thing is selected.

Idle (`CWinApp::OnIdle` override 0x4331ad → **0x424778**, only when the app
is active, the view exists and no input tool is active; OnIdle always
returns 1, so this runs continuously):

```
Idle(ws):
  sound pump (0x44081d)
  if RUNNING: Tick(ws)                                        // 0x42498f, section 7
  else:
    pt = GetCursorPos()
    if tool() in {Pointer, Cut}:
        if WindowFromPoint(pt) == view: Hover(ws, client(pt))  // 0x425800
        else Deselect(ws)                                       // 0x4253c8
    else Deselect(ws)                                           // Move tool
    if ws.selGadget == 0 and cursor over view: doc/status text id = 4000
  if ws.dirty: Redraw(ws, 0)                                    // 0x4234b8
```

```
Hover(ws, pt):                         // priority: joint, wire, gadget
  j = JointAt(pt)                      // 0x425c6d
  if j:  if j != ws.selJoint: SelectJoint(ws, j); return            // 0x425bf2
  (w, seg) = WireAt(pt)                // 0x42533a
  if w:  if w != ws.selWire or seg != ws.selSeg: SelectWire(ws, w, seg); return  // 0x425431
  g = TopGadgetAt(pt)                  // 0x42391d
  if g:  if g != ws.selGadget: SelectGadget(ws, g)                  // 0x4254ba
  else Deselect(ws)
```

Each `Select*`: if the new item differs from the current one, play sound
**0x84** (only if the item isn't locked), `Deselect` (clears all three and
invalidates them), then — only if the item isn't locked — store it and
invalidate it. `SelectGadget` also sets the Doc/status text id to
`2500 + typeNumber` (string 2500+n is the part's name, e.g. 2501
"Pendulum"; `typeNumber` = 0x435c93(tag), table at 0x49552c of
{u32 number, u32 tag}).

Knots are not hover targets; hovering a knot selects its gadget.

### 3.1 Hit tests

| function | addr | rule |
|---|---|---|
| `TopGadgetAt(pt)` | 0x42391d | first gadget (head→tail) whose slot 0x18 returns true. Base slot 0x18 = **0x41dac5**: `PtInRect(g.bounds(+0x18), pt − g.pos(+0x14))`. **All 71 gadget vtables use the base**: hit testing is by bounds rectangle, never pixel-exact. |
| `NearestKnot(g, pt)` | 0x41ebd7 | the knot with the smallest `(pt − (g.pos + knot.at))²` over all of g's knots; the caller accepts it if **d² < 0xC5 (197)**, i.e. within 14 px |
| `NearestFreeKnot(g, pt, kind)` | 0x41ddb5 (kind 0 = input), 0x41df70 (kind 1 = output) | nearest knot of that kind with no wire; **no distance limit** |
| `WireAt(pt)` | 0x42533a → 0x43b7ca per wire | first wire (head→tail) that hits: (1) pt inside the wire's bbox (2.3 below); (2) for each segment (src→joint1→…→dst) whose endpoint rect inflated by **5** on every side contains pt, perpendicular distance to the infinite line `|dy·(x−x0) − dx·(y−y0)| / √(dx²+dy²)` (0 for a zero-length segment); keep the minimum, starting from 10.0; hit if min **< 5.0** (`DAT_00496620` = 5.0); returns the index of the best segment |
| `JointAt(pt)` | 0x425c6d → 0x43c30a per wire | first wire (head→tail) whose bbox contains pt and that has a joint with **d² < 0x25 (37)** (within 6 px); the nearest such joint of that wire |
| wire bbox | 0x43beac | union of both end points and all joints, then `SetRect(left−3, top−3, right+3, bottom+3)` |

End points of a wire: `knotPos(g, k)` = **0x41e167** = `g.pos + knot.at`
(v + v, h + h). A wire end with no gadget uses the wire's free point
(+0x0c).

---

## 4. Pointer tool (edit mode): `PointerDown` (0x423ec3)

```
PointerDown(ws, pt, mods, dbl):
  j  = JointAt(pt)
  (w, seg) = WireAt(pt)
  gn = TopGadgetAt(pt)                     // list node
  knot = none
  if gn:
     g = gn.data
     if gn != head: move gn to head; g.invalidate();
                    for each output knot of g with a wire: invalidate(wire);
                    Redraw(ws, 1)
     (k, kind) = NearestKnot(g, pt)
     if (pt − knotPos(g,k))² < 197: knot = k

  if j and !dbl:                                      // 1. joint
     if !locked(j.wire):
        SelectJoint(ws, j)
        if StillDown(): UndoSnapshot(); SetDirty(); DragJoint(ws, pt)      // 0x426b94
  elif knot and !dbl:                                  // 2. knot
     SelectGadget(ws, g)
     if knot has a wire:
        if !locked(wire): MoveWireEnd(ws, g, knot, kind, pt)              // 0x4272f7
     else: NewWire(ws, g, knot, kind, pt)                                 // 0x426df7
  elif w and !dbl:                                     // 3. wire -> add a joint
     if !locked(w):
        move w to head of the wire list
        UndoSnapshot(); SetDirty()
        InsertJoint(w, pt)                                                // 0x43c470
        j2 = nearest joint of w within 6 px of pt (the new one)          // 0x43c30a
        SelectJoint(ws, j2)
        if StillDown(): DragJoint(ws, pt)
  elif gn and !w:                                      // 4. gadget
     SelectGadget(ws, g)
     if !locked(g):
        if !dbl:
           if StillDown(): UndoSnapshot(); SetDirty(); DragGadget(ws, pt)  // 0x426878
        else if g.hasDialog (+0x2a):
           OpenSettings(g)                                                 // section 6
           Idle(ws)                                                        // 0x424778
  // empty space: nothing (no rubber band)
```

Notes:

- Knots of locked gadgets can still start a wire (the knot branch doesn't
  check the gadget's lock); a wire already on a knot can only be moved if
  the *wire* isn't locked.
- A knot is only reachable if the pointer is also inside the gadget's
  bounds (the gadget hit test comes first).
- A double-click skips 1–3: it only opens dialogs, and only when no wire
  passes under the pointer.
- "Drag" only starts if the button is still down when the handler gets
  there; a quick click just selects.

### 4.1 Dragging a gadget (0x426878)

```
DragGadget(ws, start):
  Redraw(ws, 1)
  list = new cOffscreenUpdateList
  list.add(GadgetElement(g))               // 0x427cc9: hides g (+0x0d = 0)
  for each knot k of g with a wire w:
      list.add(SegmentElement(w, SegmentAt(w, g, k)))   // 0x427e91, 0x43a976
  Redraw(ws, -1)                           // rebuild offscreen without g, don't present
  list.saveUnder()                         // elements' slot 0x08
  SetCursorIndex(3)
  last = start; moved = false
  while StillDown():
     p = client(GetMouse())
     if p != last:
        if !moved: moved = true; Sound(0x85)
        list.restoreUnder()                // slot 0x0c
        g.pos += p − last                  // 0x41e1d3, raw, no clamping, no snapping
        list.saveUnder(); list.draw()      // slot 0x10
        InvalidateRect(view, ws.dirtyRect); UpdateWindow(view); ws.dirtyRect = empty
     if moved and !loopSoundStarted: StartLoop(0x96)    // 0x44073f
     last = p
     sound pump
  if loopSoundStarted: StopLoop(0x96)      // 0x440352
  if moved: Sound(0x86)
  delete list                              // g visible again, everything invalidated
  Redraw(ws, 0)
```

`SegmentAt(w, g, k)` (0x43a976): 0 if g is w's source (and k is its source
knot), the joint count (= last segment) if g is the destination, else
0xFFFF. Only the wire segment touching the dragged gadget is redrawn live;
its end point is recomputed from the gadget's new position on every draw
because wire ends are never stored, only `(gadget, knot)`. Joints never
move with a gadget.

**Bounds:** nothing clamps the gadget. The only limit is that the cursor is
clipped to the view's client rect during the drag, so the grab point stays
inside the window while the gadget itself can hang off any edge. No
snapping or grid.

The dragged gadget is drawn by its element as slot 0x04 (draw) **plus
slot 0x08 (the selection frame)**, so it shows the green frame while
moving.

### 4.2 Starting a wire (0x426df7)

Wires are made by pressing on a free knot and dragging; there is no
click-click mode.

```
NewWire(ws, g, k, kind, start):
  Sound(0x82); Deselect(ws); Redraw(ws,1); UndoSnapshot(); SetDirty()
  if kind == 1 /*output*/: w = new cConnection(src=g, srcKnot=k, dst=0, dstKnot=0xFFFF); want = INPUT
  else:                    w = new cConnection(src=0, 0xFFFF, dst=g, dstKnot=k);    want = OUTPUT
  // ctor 0x43a424 registers w on the non-null end's knot immediately
  push w to the front of the wire list
  w.free = start; w.snapped = 0                    // 0x43c281
  list = { SegmentElement(w, 0) }; Redraw(ws,-1); list.saveUnder()
  lastHit = none; last = start
  do:
     sound pump
     p = client(GetMouse())
     if p != last:
        list.restoreUnder()
        h = TopGadgetAt(p)
        w.free = p; w.snapped = 0
        if !h: lastHit = none
        else if (k2 = NearestFreeKnot(h, p, want)):
             w.free = knotPos(h, k2); w.snapped = 1
             if (h,k2) != lastHit: Sound(0x83); lastHit = (h,k2)
        list.saveUnder(); list.draw(); present dirty rect; last = p
  while StillDown()
  invalidate(w)
  if w.snapped: Sound(0x82); connect the open end to (h, k2)   // 0x43c24f dst / 0x43c21f src
  else: DeleteWire(ws, w)                                       // 0x4246be
```

**What may connect:**

- Always output → input (the search only looks at the opposite kind).
- Only to a **free** knot: a knot holds one wire; an input that is already
  wired is skipped, so to rewire it you must first move or delete the
  old wire. (One output can feed one input; that's what the Splitters
  are for.)
- **The same gadget is allowed** (its own free inputs/outputs are found
  like any other's); no loop check.
- Snapping: hovering **anywhere over a gadget's bounds** snaps the wire to
  that gadget's *nearest free knot of the right kind, at any distance*.
  Over empty space the free end follows the pointer.
- Releasing over nothing deletes the new wire silently (no extra sound).

### 4.3 Moving a wire end (0x4272f7)

Pressing on a knot that already has an (unlocked) wire picks that end up:

```
MoveWireEnd(ws, g, k, kind, start):
  Sound(0x81); Deselect(ws); Redraw(ws,1); SetDirty(); UndoSnapshot()
  w = k.wire; seg = SegmentAt(w, g, k)
  if kind == 1: detach w's source end (0x43c2a3); want = OUTPUT
  else:         detach w's destination end (0x43c2d5); want = INPUT
  w.free = start; w.snapped = 0
  track exactly like NewWire (same snapping rule, same 0x83 sound),
  but searching knots of the SAME kind as the one picked up; whenever the
  snapped state flips, invalidate and redraw the whole wire (its colour changes)
  on release:
     if w.snapped: Sound(0x82); attach the end to the new (gadget, knot)
     else:         re-attach the end to the original (g, k)      // no sound
```

A wire can't be deleted by dragging its end into empty space; the end
springs back. The original knot is free during the drag, so dropping on it
is a normal reconnect (with sound 0x82).

### 4.4 Joints (bend points)

Joint = `cKnot` (9 bytes, ctor 0x43cb40): +0 Point, +4 owning
`cConnection*`, +8 u8 visible (1). A wire's joints are an ordered list at
wire+0x11; segment i runs from point i to point i+1 where point 0 is the
source knot, then the joints in list order, then the destination knot.

- **Adding** (0x43c470): clicking on a wire (Pointer tool) inserts a joint
  *at the click point* into the segment nearest to it (same distance rule
  as `WireAt`): inserted before the joint that ends that segment, or
  appended when it is the last segment. The new joint is selected and, if
  the button is still down, dragged. Every click on a wire adds a joint.
- **Dragging** (0x426b94): elements = the two segments touching the joint
  (indices i and i+1, where i is the joint's index) + the joint itself;
  cursor 3; the joint follows the pointer exactly (no snapping/clamping);
  no sounds. While dragging the wire draws green (`snapped` forced to 1).
- **Deleting**: Cut tool click on it (sound 0x89), Delete key while
  hovered, or Edit ▸ Remove All Joints.
- Gadget moves don't touch joints; panning and Fit Work Area move them.

### 4.5 Placing a gadget from the tray (`CWPlaceGadgetTool`)

The tray is another agent's area; the hand-off is:

1. L-button down on a tray slot (0x42e880): if its count is 0, nothing.
   Else: select the Pointer tool, deselect, redraw, **undo snapshot**,
   decrement the count unless it is −1 (unlimited) and redraw the tray,
   then `tool = new CWPlaceGadgetTool(view, typeNumber, x, y)` (0x439237)
   and make it the view's input tool (0x43469e; the old tool, if any, is
   ended).
2. The ctor (`CWorksheetDragTool` part): `SetCapture(view)`,
   `ClipCursor(view client)`. Then: sound **0x93**; create the gadget with
   the factory at position (−2000, −2000) (`0x41f194(ws, tag, 0xF830F830,
   NULL, fresh)`); create a `CWGadgetDragger` (0x4277d9, 0x1c bytes) with
   hot spot = `(width/2, height/2)` of the gadget's bounds rect (C
   division); copy the images of the control panel and the tray into the
   view's offscreen and **hide both pane windows**, so the dragged part
   can be drawn over where they were; move the dragger to (x, y).
3. Mouse move (slot 0x00 = 0x439b64): `0x4279e2`: restore under, set
   `g.pos = (y − hot.y, x − hot.x)`, save under, draw, present (Redraw mode
   2). The part's bounds-centre follows the pointer (the centre of its
   *size*: if the bounds rect doesn't start at 0,0 it is offset).
4. L-button up (slot 0x08 = 0x4395f5): show both panes again; move the
   dragger to the release point; then
   - if the point is **not** inside the tray pane's rect: sound **0x92**,
     `g.pos = (y − hot.y, x − hot.x)` via 0x41e21c, push g to the front of
     the gadget list, invalidate it, `SetDirty()`.
   - else (dropped back on the tray): give the part back to the tray
     (0x432867, +1 unless unlimited), delete the dragger and the gadget.
     (The undo snapshot from step 1 stays.)
   Then invalidate both panes' rects, redraw, and end the tool (posts 0x464;
   its destructor releases the capture and the cursor clip).

No clamping: a part can be dropped half outside the window or under the
control panel.

---

## 5. Cut and Move tools, Delete, menu edits

### 5.1 Cut tool (0x42566c), edit mode only

Acts on button-down, no drag; knots are not considered:

```
if j = JointAt(pt):                      // joints first
    if !locked(j.wire): Sound(0x89); UndoSnapshot(); SetDirty()
                        if j == ws.selJoint: Deselect(ws)
                        RemoveJoint(j.wire, j)                     // 0x43c3f0
elif (w,_) = WireAt(pt):
    if !locked(w): Sound(0x88); UndoSnapshot(); SetDirty(); DeleteWire(ws, w)
elif g = TopGadgetAt(pt):
    if !locked(g): Sound(0x87); UndoSnapshot(); SetDirty(); DeleteGadget(ws, g)
```

The Cut tool still hover-highlights (section 3); only the cursor differs.

`DeleteGadget(ws, g)` = **0x4245d6**: return one of g's type to the tray
(0x432867: +1 unless the count is −1), deselect if it was selected,
unlink, destroy. The gadget destructor (0x41d2bd) invalidates it and
**deletes every wire attached to any of its knots** (0x4246be), whatever
their lock state.

`DeleteWire(ws, w)` = **0x4246be**: deselect if selected, unlink, destroy
(0x43af27: invalidate, clear the wire pointer in both end knots, free all
joints).

### 5.2 Move tool = pan the whole widget (0x426313)

The "Move" tool moves *everything*, not one part. It works in edit mode and
while running (Ctrl held, since starting a run selects the Pointer and
hides the tray pane with the tool buttons).

```
PanWidget(ws, start):
  UndoSnapshot(); SetDirty()                    // even if nothing moves
  last = start
  while StillDown():
     sound pump
     p = client(GetMouse())
     if p != last:
        TryOffset(ws, p − last)                 // 0x425f10
        last = p                                // updated even if refused
        Redraw(ws, 1)
```

`TryOffset(ws, d)` (0x425f10): `inner` = view client rect shrunk by
`width/10` and `height/10` on each side (C truncation). Let A = "some
gadget's screen bounds intersects `inner` now" and B = "some gadget's
bounds offset by d intersects `inner`". Apply only if `!A || B`: every
gadget moves by d (0x41e21c, with invalidation) and every joint of every
wire moves by d (0x43c942). So the widget can't be pushed entirely out of
the middle 80 % of the window. Wires follow automatically.

### 5.3 Edit menu

The Edit menu (menu resource 3) is: **Undo** (0xE12B), Clear Work Area
(0x8009), Remove All Connections (0x800A), Remove All Joints (0x800B), Fit
Work Area ... (0x8011). There is **no Cut/Copy/Paste/Duplicate/Select
All** and no clipboard code anywhere in the program.

| command | handler | does |
|---|---|---|
| Clear Work Area | 0x425cda | `UndoSnapshot`, deselect, delete every unlocked gadget (their wires go with them; parts return to the tray), then every remaining unlocked wire; redraw. Does **not** set the dirty flag. |
| Remove All Connections | 0x425dbe | `UndoSnapshot`, deselect, delete every unlocked wire; redraw. No dirty flag. |
| Remove All Joints | 0x426407 | `UndoSnapshot`, deselect, remove all joints of every unlocked wire (0x43c8c6); redraw. No dirty flag. |
| Fit Work Area | 0x4294fe | section 5.5 |
| Undo | 0x415546 | section 5.4 |

The Options menu's four backgrounds (0x800C plain, 0x800D Metal Waves =
701, 0x800E Noise = 700, 0x800F Steel = 702; handlers 0x43d8c8, 0x43d946,
0x43da08, 0x43daca) each take an undo snapshot and set the dirty flag when
the background actually changes.

### 5.4 Undo (`cUndo`, 0x415400)

`cUndo` = {+0 buffer, +4 size, +8 u8 can-undo}. **One level**, and it is a
**toggle**: undoing twice redoes.

- `UndoSnapshot()` = **0x4154b4**: serialize the whole worksheet with the
  file writer (0x424d0a, section 8; buffer = size + 63000) into the undo
  buffer, replacing the previous one; can-undo = 1; update the menu.
- `Undo()` = **0x415546** (Edit ▸ Undo): if there is no buffer, clear the
  worksheet. Else copy the buffer, `UndoSnapshot()` (current state becomes
  the new undo state), clear the worksheet (0x424b27), load the copy
  (0x42503e); if that fails, drop the undo state and retry; redraw. Undo
  doesn't touch the dirty flag or the tray reset, but the snapshot's
  `Gmun` chunk restores limited tray counts.
- `UndoClear()` = **0x415471**: free the buffer, can-undo = 0 (New, Open,
  Load; Save only when out of memory).

Snapshot sites = everything undoable: gadget drag, joint drag (each only
if the button was still down), joint insert (click on a wire), new wire,
moving a wire end, Cut-tool deletes, Delete key, L (lock), numpad ± tray
counts, panning, picking a part from the tray (covers the placement),
settings dialog OK (before reading the dialog back), Clear Work Area,
Remove All Connections/Joints, Fit (inside 0x429060), background change.
Menu: Undo is enabled iff can-undo (0x414d0d), and disabled while running.

### 5.5 Fit Work Area (0x4294fe) and the save-time Mac check

`Extents(ws)` = **0x4291d9**: union of every gadget's screen bounds
(`pos + bounds`) and every wire's bbox (incl. ±3 and joints); empty if the
worksheet is empty.

```
FitWorkArea(ws):                                     // menu 0x8011
  e = Extents(ws)
  if e empty: MessageBox("Nothing to fit"); return
  if e.left > 14 and e.top > 14 and e.right < 626 and e.bottom < 466:
     MessageBox("Your widget is already fit to any Windows screen"); return
  FitDialog(ws)                                      // 0x429591

FitDialog(ws):  (returns "continue", false only on Cancel)
  e = Extents(ws)
  if e nonempty and (e.left < 15 or e.top < 15 or e.right > 625 or e.bottom > 465):
     dialog 10014 ("Do you want to automatically squeeze the parts together so they'll
       all fit on your (or a smaller) monitor?"), radio buttons 0x65..0x69 =
       640×480, 720×540, 800×600, 1024×768, 1280×1024; initial choice = INI
       [Widget Workshop] FitSize (0..4); the choice is written back to FitSize
     if result == 6 (Yes):
        (sx, sy, ox, oy, needed) = FitParams(e, target = (14, 14, W−14, H−14))
        if needed: Squeeze(ws, sx, sy, ox, oy)
     return result != 2
  return true
```

`FitParams` = **0x428f1c**(e, T):

```
sx = sy = 1.0; ox = oy = 0.0
if IntersectRect(e, T) == e: return not needed        // already inside
if T.w < e.w: sx = T.w / e.w                            // x and y independent:
if T.h < e.h: sy = T.h / e.h                            // aspect is NOT kept
if e.left*sx < T.left or e.right*sx > T.right:  ox = T.left − e.left*sx
if e.top*sy  < T.top  or e.bottom*sy > T.bottom: oy = T.top − e.top*sy
return needed
```

`Squeeze(ws, sx, sy, ox, oy)` = **0x429060**: `UndoSnapshot`, `SetDirty`;
for every gadget: `new.v = trunc(pos.v*sy + oy)`, `new.h = trunc(pos.h*sx +
ox)` (float → int via `__ftol` 0x480ec4, truncation), moved with
invalidation; for every joint the same formula (0x43c9c8); redraw. Only
positions scale; gadgets keep their size, so parts may overlap ("it might
not look very pretty", README).

On **Save and Save As** (0x42932b, before writing): the target is fixed
to the Mac's 720×540: `T = (14, 14, 706, 526)`. If `FitParams` says it's
needed: INI `[Widget Workshop] Fit` = 0 → ask with dialog 10010 ("This
widget is too big to share with friends who have the Macintosh
version...", **Yes / No / Cancel**, check box id 13 "Remember my answer");
=1 → behave as No; =2 → behave as Yes. If the box is checked, Yes stores 2
and No stores 1. Yes → `Squeeze`; Cancel → abort the save.

---

## 6. Settings dialogs (`cDialog`)

Opened by double-clicking an unlocked gadget whose +0x2a ("has a
dialog", 1 by default from 0x41cf8a; some subclasses clear it) is set, with
the Pointer tool, in edit mode, with no wire under the pointer
(section 4). README's "double-click on the part and Doc will give you some
pointers" = this dialog's Doc panel. Not available while running.

```
dlg = new cDialog(gadget g, callback = g+0x2b)   // 0x41baac, 0x9e bytes; template = RT_DIALOG g+0x28
RunModal(dlg)                                    // 0x41c43b
delete dlg                                       // 0x41bb8b
```

`RunModal` (0x41c43b): sound **0x8a**; (constructs and destroys an unused
second `CDialog` on the same template); `CDialog::DoModal` (vtable slot
0x70); sound **0x8b**; returns `result == IDOK`. (0x423ec3 also computes the
gadget's screen rect but never uses it: the dialog is not positioned near
the part.)

`cDialog` vtable 0x491d70: +0x74 `OnInitDialog` = **0x41acb9**, +0x48
`OnCommand` = **0x41a686**, message map 0x491874: WM_DRAWITEM →
**0x41a912**. Fields: +0x48 change callback, +0x4c gadget, +0x50 list of
check boxes, +0x5a push buttons (OK and Cancel included), +0x64 radio
groups (the radio buttons live only in their groups), +0x6e combo boxes,
+0x78 edit fields, +0x82 static texts, +0x8c
`cDialogGadgetGraphic*`, +0x90 "Pic" offscreen, +0x94 Doc picture
offscreen, +0x98 `cDialogDocItem*`.

### 6.1 OnInitDialog (0x41acb9)

1. For every control id 1..0xA9 that exists: add WS_CLIPSIBLINGS and wrap
   it by class:
   - `Button` style 0/1 (push) → `cButton` list (+0x5a);
     style 2/3 (check box) → `cBoolControl` list (+0x50); style 4/9
     (radio) → a `cBoolControl` grouped into a
     `cRadioGroup` (+0x64) by the **last character of its caption**, which is
     stripped (dialog texts like `Preset:2`, `Custom:2`, `On at Center1`:
     the trailing digit is the group key; groups are numbered in the
     order of their first button's id);
   - style 0xB (owner-draw):
     - id **0x66** whose caption matches `Pic%d` → load picture sheet `%d`
       (e.g. `Pic4000`), make an offscreen the size of the control, fill it
       with the dialog background brush (WM_CTLCOLORDLG), draw frame 1 at
       offset (0,0) → +0x90. This is the part's picture.
     - id **0x66** otherwise → `cDialogGadgetGraphic` (0x418cd0, 0x1c bytes,
       uses picture 504): a *live* rendering of this gadget (0x418e92),
       refreshed after every control change → +0x8c.
     - id **0x67** → `cDialogDocItem` (0x417df8, 0x6e bytes): reads the
       gadget's info (resource type 1021, id = gadget res id) words +4 and
       +6 = two Doc texts (resource type 1020); draws a rounded-rect text
       box (MS Sans Serif, heights = box height/9 and /12) with Doc's
       pictures (sheet 505) → +0x98. Button **0x96** toggles between the
       two texts (0x418ca0); WM_DRAWITEM 0x96 draws its frames 1/2 (page 0
       up/down) or 3/4 (page 1).
     - id **0x97** → picture 705 frame 1 into +0x94 (Doc's face).
   - `ComboBox` → `cIntControl`-like list (+0x6e); when any exist, fill
     them from resource type 1022 (0x3fe) with the dialog's id (0x41abc2:
     entries {u16 control id, u16 msg, u32 length, data}, each sent with
     `SendDlgItemMessage(id, msg, 0, &data)`, msg 0x401 translated to 0x180
     LB_ADDSTRING and 0x403 to 0x143 CB_ADDSTRING; ends at control id 0).
   - `Static` → +0x82; `Edit` → `cEditText` (+0x78), whose template text
     is a filter (max length, `#` number, `.` real, `-` sign, `A`
     alphanumeric) and is then cleared; the first edit gets the focus.
2. Focus: the first edit field, else control 1 (OK).
3. **`g->vtbl[0x48](g, dlg)` — fill the dialog** (base 0x41ea4d: radio
   groups, number fields, 32 check boxes, the text(s); and push button
   number `+0x33` gets the handler `+0x2f` if set). The 11 overrides
   also set control **links** (`0x417c7b`); see
   [`dialogs.md`](dialogs.md) §2–4.
4. Select all text of the focused edit (EM_SETSEL 0,−1).
5. Post WM_COMMAND 0xFD: when it arrives, `DAT_004918bc` = 1 ("initialized").

### 6.2 OnCommand (0x41a686)

| id | does |
|---|---|
| 1 (OK) | `UndoSnapshot()`; **`g->vtbl[0x4c](g, dlg)` — read back** (base 0x41ea2e: 0x41e851 copies radios, numbers, check bits, texts into the settings block; then redraws the work area); `EndDialog(1)` |
| 2 (Cancel) | **`g->vtbl[0x50](g)`** — "settings changed/apply": recompute the gadget's state from its (unchanged) settings block, undoing any preview; `EndDialog(2)` |
| 0x96 | Doc text page toggle |
| other < 0x100, after init | find the control (a radio button is found as its group); on `EN_CHANGE` validate the edit (revert to its last good text if the filter refuses it); on a button click, an edit change or a combo's `CBN_SETFOCUS`, run a push button's handler and walk the **link** chain (each target radio button/check box gets selected, an edit gets the focus; [`dialogs.md`](dialogs.md) §1.3); then the **change callback** `g+0x2b` (default **0x41eb78**): save the settings block, read the dialog into it, call `g->vtbl[0x50]`, refresh the `cDialogGadgetGraphic`, restore the saved settings block. So controls preview live; slot 0x50 is the "apply settings" hook. |

Neither OK nor the callback sets the document's dirty flag (only the undo
snapshot is taken). WM_DRAWITEM 0xA0 draws a percentage bar (left part
RGB(0,0,255), rest RGB(255,255,0), percent at dlg+0x9c; used by the
sound recorder's dialog).

---

## 7. Run mode vs edit mode

`DAT_00493dde`: 1 = edit (initial), 0 = running.

Start (control-panel GO, command 0x5DD, or Enter in edit mode) =
**0x430e70**, if not already running:

```
mode = RUNNING; Sound(0x8e)
hide the tray pane (view+0x6fc)
StartRun(ws):                                   // 0x424a12
   SelectTool(Pointer); Deselect(ws); ws+0x4b = 0
   for each gadget head→tail: g->vtbl[0x20](g)  // start
                              g->vtbl[0x0c](g)  // invalidate
press the GO button graphic; cMenuBar update (0x414b7e)
```

Stop (STOP 0x5DE, Esc while running, a Stopper part 0x457ba2, or "Save
changes?" → Yes) = **0x430ee7**, if running:

```
mode = EDIT; 0x440823 (stop sounds)
StopRun(ws):                                    // 0x424a94
   for each gadget: g->vtbl[0x1c](g)            // reset
   for each wire: wire.changed (+0x2a) = 0
   ws+0x4f = 0 (clock restarts next run); redraw everything
Sound(0x8f); show the tray pane; press the STOP button graphic; cMenuBar update
```

`Tick(ws)` = **0x42498f** (from idle, running only, as fast as idle
allows):

```
if ws+0x4f == 0: ws+0x4f = now(); ws+0x4b = 0       // now() = 0x4113a8, Mac ticks
else: ws+0x4b = now() − ws+0x4f
for each gadget head→tail while mode == RUNNING: g->vtbl[0x44](g)
```

then idle redraws the dirty rect (0x4234b8). While the app is inactive
(WM_ACTIVATEAPP, 0x4359f7 → 0x424944) nothing ticks and the paused time is
added to +0x4f on reactivation, so +0x4b excludes it.

While running:

- The tray pane (with the Pointer/Cut/Move buttons) is hidden; hover
  selection, frames, Cut, dragging, wiring, joints and dialogs are all
  off (`MouseDown` goes to `RunClick`, idle ticks instead of hovering).
- Mouse: slot 0x14 on the topmost gadget under a left click
  (section 2.2). Ctrl temporarily gives the Move tool, which pans.
- Keys: Esc stops; Ctrl pans; nothing else.
- Menus: `cMenuBar::Update` (0x414b7e) **disables** New, Open, Load
  Puzzle, Save, Save As, Revert, Import/Save Stand-Alone, Print, Print
  Setup, Print Preview, Fit, Undo, Clear, Remove All Connections/Joints
  and even **Exit** (0xE141). In edit mode it re-enables them, Undo per
  can-undo, Save iff dirty, Revert iff dirty and (has a file or a puzzle
  is loaded).

---

## 8. Files: `cFile`, load, save

`cFile` (`DAT_004a270c`): +0x000 path (256), +0x100 u8 dirty, +0x101 u8
"came from a stand-alone .EXE", +0x102 u8 "has a path", +0x107 default name
"untitled.wgt", +0x207 puzzle name buffer. `SetDirty` = **0x416142** (sets
+0x100, updates menu), `ClearDirty` = 0x41616a, title "Widget Workshop -
%s" (0x417092).

### 8.1 Writer (0x424d0a) — also the undo format

Size (0x424c45) = 0x20 + Σ(wire chunk + 8) + Σ(gadget chunk + 8) + (Gmun
+ 8 if any). All integers big-endian:

```
'FORM' u32 size(=total−8) 'Gadf'
'fver' u32 2  u16 4                       // version written is always 4
'back' u32 2  u16 backgroundId            // 0, 700, 701, 702
per gadget, tail→head:  tag(4)  u32 size(=slot 0x3c)  slot 0x40 data
per wire,   tail→head:  'conn'  u32 size  data (below)
if any tray count is limited: 'Gmun' u32 size  u16 n, n × {u32 tag, u16 count}
```

`conn` (0x43b512), size `0x10 + 4·joints`: +0 u32 source id, +4 u32
destination id, +8 u16 source knot, +0xa u16 destination knot, +0xc u8
locked, +0xd 0, +0xe u16 joint count, +0x10 joints as {i16 v, i16 h}.

### 8.2 Gadget ids

A **fresh** gadget's +0x08 is `(u32)this | 0xFF000000` (0x41d076). But the
writer doesn't use +0x08: both the gadget chunk's id (0x41d396) and the
wire's two ids (0x43b512) are computed as `(u32)gadgetPointer |
0xFF000000` (0x41ef27) at save time. On load, the gadget's +0x08 is set
from its chunk and the `conn` loader (0x43b010) resolves each id with
`FindGadgetById` (0x424bf2: first gadget, head→tail, whose +0x08
matches; NULL if none). So ids are only meaningful inside one file; a port
can assign any unique u32 per gadget when saving (to look authentic: a
4-byte-aligned address-like value with the top byte 0xFF).

### 8.3 Loader (0x42503e)

```
Load(ws, data, size, quiet):
  require data[0..4]=='FORM' and data[8..12]=='Gadf' and data[0xc..0x10]=='fver'
     else error 8007 ("not a Widget Workshop saved widget") unless quiet; return 0
  ws+0x53 = BE16(data+0x14)
  if version > 4 or version == 0: error 8007 unless quiet   // but keeps going
  p = data + 0x14 + BE32(data+0x10)
  while p − data < size:
     if low on memory (0x4109d4): error 8010 unless quiet; clear ws; return 0
     switch tag(p):
       'Gmun': set tray counts (0x432b3c)
       'back': background = BE16(p+8)   (0x43db97)
       'conn': w = cConnection(p+8) (0x43b010); push_front wire list
       else:   g = factory(ws, tag, 0, p+8, loading) (0x41f194); push_front gadget list
     p += 8 + BE32(p+4)
  return p − data
```

`conn` with version < 3: only the first 0xc bytes are read (ids and knots),
no joints, locked = 0. Version ≥ 3: as written above.

### 8.4 Flows

| command | id | handler | flow |
|---|---|---|---|
| New | 0xE100 | 0x43327a → 0x415865 | if dirty and no puzzle: "Save changes to %s?" (0x410b24; Yes/No/Cancel). Cancel → stop; Yes → Save, stop if it fails. Then: clear path flags and dirty, `UndoClear`, clear the worksheet (0x424b27: deselect, delete all gadgets → their wires, background 0), Pointer tool, all tray counts unlimited (0x4329e0), puzzle reset, title. |
| Open | 0xE101 | 0x4337c4 → 0x415936 | same prompt; file dialog "Saved widgets (*.wgt)" / "Load widget", or a given path (drag & drop, startup); then 0x416348: `UndoClear`, reset tray, read the whole file (0x416185), clear, reset tray, `Load`, Pointer tool, path, puzzle reset, +0x101 = 0, +0x102 = 1, title, `ClearDirty`. |
| Save | 0xE103 | 0x4337d9 → 0x415a3b | no path or puzzle loaded → Save As. Else Mac check (5.5, Cancel aborts); if a puzzle is loaded and Shift isn't held: unlock every gadget and wire (0x426511) and reset tray counts; serialize; write (0x416269, CFile create); on success puzzle reset, +0x102 = 1, `ClearDirty`. |
| Save As | 0xE104 | 0x4337ea → 0x415c1f | Mac check; default name = puzzle name / path / "untitled.wgt"; dialog "Saved widgets (*.wgt)" / "Save Widget as"; unlock as above; serialize, write, set path, title. |
| Revert | 0x8004 | 0x4337fb → 0x415ece | only if dirty; confirm 8001 "Revert to last saved version?" or 8002 "Revert to original puzzle?"; reload the file, the .EXE it was imported from, or the puzzle. |
| Load Puzzle | 0x8003 | 0x4332b6 → 0x416f1a | save prompt; puzzle chooser; clear; load (puzzle agent's area). |
| close / exit | — | `CWidgetDoc::SaveModified` 0x4337a9 → 0x417027 | if dirty: "Save changes to %s?"; Yes → **stop running**, Save; Cancel → don't close. |

"Save changes to %s?" is skipped while a puzzle is loaded (New/Open/Import/
Load Puzzle) and the name shown is the puzzle name, the path, or
"untitled.wgt" (0x416ee2). Other errors are string ids 8000+n (8003 read
error, 8004 write error / disk full, 8009/8010 memory).

Startup: the first command-line argument containing ".wgt" that exists is
opened directly (0x4104c4); otherwise the intro dialog (0x42b07c) returns
0 → Load Puzzle, 1 → empty widget, 2 → Open, 3 → Exit (0x43480e).

### 8.5 Stand-alone widgets (low priority)

- **Save Into a Stand-Alone Widget** (0x8006 → 0x4165a7): needs
  `WIDGET.SLF` next to WIDGET.EXE (else "Can't find file: WIDGET.SLF...");
  runs `FitDialog` (Cancel aborts); save dialog "Self running widgets
  (*.exe)" / "Save Into Stand Alone Widget" (default: current name with
  .EXE; refuses to overwrite the running exe); `LZCopy` WIDGET.SLF (an
  LZ-compressed runtime exe) to the target; then appends resources with
  the program's own PE resource writer (`CWResourceFile`, 0x41218a/
  0x412297/0x412275): the serialized widget (same bytes as a .wgt) as
  type **1023 (0x3FF) id 1**; every sound/picture resource needed by the
  parts present (table at 0x4909a8: 9-byte entries {u32 tag, u16 res type,
  u16 res id, u8 used}, terminated by tag 'Done'; 0x42659d marks the used
  ones; duplicates skipped); and the background picture (type 1004, id =
  background) if not plain. Error 8004 on failure.
- **Import a Stand-Alone Widget** (0x8005 → 0x416bf5): save prompt; open
  "Self running widgets (*.exe)" / "Import from self running widget"; read
  resource 1023/1 from that exe; clear, `Load`, Pointer tool; path = exe
  path with ".WGT"; +0x101 = 1.
- The runtime loads its embedded widget with 0x41642c (resource 1023/1).

---

## 9. Drawing (what my area paints)

All drawing goes into the view's 8-bit offscreen DIB (view+0x135c), whose
colour table is the 1003/101 palette; the view blits it on WM_PAINT.
`Redraw(ws, mode)` = **0x4234b8**: if the dirty flag is set: modes 0/1/2
`InvalidateRect(view, dirty)`; clear the dirty rect; unless mode == 2,
repaint the dirty area into the offscreen: running → 0x423b59, editing →
set the offscreen DC's clip region to the dirty rect and 0x42397d; modes 1
and 2 also `UpdateWindow`. Mode −1 repaints the offscreen without
invalidating. `Invalidate(rect)` = 0x4236ed (union into +0x2d).

Paint order (edit, 0x42397d; run 0x423b59 is the same without frames):

1. background (`cBackground` 0x43d209: plain = fill white when the Light is
   on (`DAT_00493ddc`), black when off; else the 700–702 picture tiled,
   frame 1 lit / frame 2 dark);
2. gadgets tail→head with `+0x0d` (visible) set: slot 0x04; the selected
   gadget also slot 0x08. (Edit mode runs this loop **twice** in a row,
   identically — so each gadget's draw is called twice per repaint.)
3. wires tail→head: 0x43a489; the selected wire's selected segment again
   with 0x43bb96; then the selected joint's frame (0x43cdfd);
4. the background's border (picture 704 edges/corners, 0x43d39b; only for
   non-plain backgrounds).

Colours (palette 1003 indices; all exact matches):

| element | colour | index |
|---|---|---|
| gadget selection frame (slot 0x08 base 0x41d4b5) | 1-px `FrameRect` of the gadget's screen bounds, RGB(0,128,0) | **2** |
| connected wire, and a dragged wire end that is snapped to a knot | RGB(0,128,0) | **2** |
| wire with a free end (not snapped) — the rubber band | RGB(128,0,0) | **1** |
| selected segment overlay | 2-px pen, RGB(0,128,0); RGB(128,0,0) if the wire is locked (never, since locked wires can't be selected) | 2 / 1 |
| joint | filled 4×4 square: x−2..x+1, y−2..y+1, RGB(0,0,128) | **4** |
| selected joint | 1-px frame of (x−3, y−3, x+3, y+3) = 6×6 outline, RGB(0,128,0) | **2** |

Wire lines are 1 pixel wide. In **edit mode** each segment is plotted by
0x43849e (clipped to the offscreen by 0x43817d), inclusive of both
endpoints:

```
if |dy| > |dx|: order so y0 <= y1; for y = y0..y1: x = x0 + ((y−y0)*dx + dy/2) / dy   (C '/' truncates toward zero)
else:           order so x0 <= x1; for x = x0..x1: y = y0 + ((x−x0)*dy + dx/2) / dx
```

While **running**, the same polyline is drawn with GDI `MoveTo/LineTo` and
a 1-px pen (Windows excludes each segment's last pixel). In both modes
zero-length segments are skipped and segments marked "floating" (wire
+0x26/+0x28, set by a live `cConnectionSegmentUpdateElement`) are left out
because the element draws them. Joints are drawn in both modes; frames
only in edit mode.

Update elements (`cOffscreenUpdateList` 0x4935a8; element slots +0x08
save-under, +0x0c restore-under, +0x10 draw):

| class | vtable | ctor | buffer | draw |
|---|---|---|---|---|
| `cGadgetUpdateElement` | 0x493574 | 0x427cc9 | gadget bounds + 1 px each side | slot 0x04 then slot 0x08 (frame); hides the gadget (+0x0d = 0) while alive |
| `cConnectionSegmentUpdateElement` | 0x493520 | 0x427e91 | the pixels under the DDA line (0x4386d7 save / 0x43891a restore) | 0x43ad31: the segment in index 2 (green) if snapped or both ends connected, else index 1 (red) |
| `cKnotUpdateElement` (= joint) | 0x493500 | 0x4280f5 | 6×6 around the joint | the joint square + its green frame; hides the joint (+8 = 0) while alive |

---

## 10. Sounds triggered here

`Sound(id)` = `0x4402cc(id, 0)` (sound system is another agent's area).

| id | when |
|---|---|
| 0x80 | control-panel button press |
| 0x81 | pick up an existing wire end |
| 0x82 | start a new wire; wire connected |
| 0x83 | wire snaps to a new knot while dragging |
| 0x84 | hover selection changes |
| 0x85 / 0x86 | gadget picked up (first move) / dropped |
| 0x96 | loop while dragging a gadget (`0x44073f` start, `0x440352` stop) |
| 0x87 / 0x88 / 0x89 | gadget / wire / joint deleted |
| 0x8a / 0x8b | settings dialog opens / closes |
| 0x8e / 0x8f | run starts / stops |
| 0x92 / 0x93 | part placed on the worksheet / taken from the tray |

---

## 11. Corrections to gadget-model.md

- Gadget **+0x0c is "locked"** (puzzle parts that can't be moved, cut,
  edited; 0x41ed78/0x41ed83), not "selected". The selection is a pointer
  on the worksheet (+0x3b). It is saved at chunk offset 0x110.
- Gadget **+0x0d** = visible (drawn by the worksheet), cleared while a
  drag element owns it. **+0x2a** = has a settings dialog; **+0x2b** = the
  dialog change callback (default 0x41eb78); +0x152 = run time of the last
  invalidation (slot 0x0c stores `ws+0x4b` there; 0x41e762(g, n) = "n
  ticks have passed since").
- Gadget **+0x14 is v (y) and +0x16 is h (x)** in every function I read
  (hit test, knot positions, load/save, frames).
- vtable **slot 0x18 is the hit test** `bool HitTest(Point)` (base
  0x41dac5), not "move to". Moving is done by 0x41e21c (with invalidation)
  / 0x41e1d3 (raw).
- Slot 0x14 (click while running) takes `(Point pt, u16 mods = 0, u8
  doubleClick)`.
- Slot 0x50 is called on dialog **Cancel** and by the live-preview
  callback: it is "apply the settings block to the gadget's state".
- `cConnection`: +0x0c is a Point (the free end while dragging), +0x10 u8
  "free end snapped", +0x11 the joint list (10 bytes), **+0x25 u8 locked**,
  +0x26/+0x28 i16 floating segment indices (−1 = none), +0x2a changed.
- The ids at gadget +0x08 are pointer-derived and rewritten on every save
  (8.2).

---

## 12. Function names (proposed; not applied to the DB)

| address | proposed name |
|---|---|
| 0x410600 | GetWindowsFlavor |
| 0x4104c4 | FindWgtOnCommandLine |
| 0x41011b | GetIniInt |
| 0x4109d4 | CheckMemoryOrThrow |
| 0x410b24 | AskSaveChanges |
| 0x410b5b | ShowErrorString8000 |
| 0x414b7e | cMenuBar_Update |
| 0x414d0d | cMenuBar_UpdateUndo |
| 0x414d47 | cMenuBar_UpdateSaveRevert |
| 0x414dcc | cMenuBar_CheckBackground |
| 0x41507c | cKeyboard_IsKeyDown |
| 0x415208 | StillDown |
| 0x4153ac | GetMouseScreen |
| 0x415400 | cUndo_ctor |
| 0x415471 | cUndo_Clear |
| 0x4154b4 | cUndo_Snapshot |
| 0x415546 | cUndo_Undo |
| 0x41566d | cUndo_CanUndo |
| 0x415865 | cFile_New |
| 0x415936 | cFile_Open |
| 0x415a3b | cFile_Save |
| 0x415c1f | cFile_SaveAs |
| 0x415ece | cFile_Revert |
| 0x416142 | cFile_SetDirty |
| 0x41616a | cFile_ClearDirty |
| 0x416134 | cFile_IsDirty |
| 0x416185 | ReadWholeFile |
| 0x416269 | WriteWholeFile |
| 0x416348 | cFile_LoadPath |
| 0x41642c | LoadEmbeddedWidget |
| 0x4164fc | LZCopyFile |
| 0x4165a7 | cFile_SaveStandAlone |
| 0x416bf5 | cFile_ImportStandAlone |
| 0x416ee2 | cFile_DisplayName |
| 0x416f1a | cFile_LoadPuzzle |
| 0x417027 | cFile_SaveModified |
| 0x417092 | cFile_UpdateTitle |
| 0x417df8 | cDialogDocItem_ctor |
| 0x4186f9 | cDialogDocItem_Render |
| 0x418ca0 | cDialogDocItem_TogglePage |
| 0x418cd0 | cDialogGadgetGraphic_ctor |
| 0x418e92 | cDialogGadgetGraphic_Render |
| 0x41a686 | cDialog_OnCommand |
| 0x41a912 | cDialog_OnDrawItem |
| 0x41abc2 | cDialog_FillCombos |
| 0x41acb9 | cDialog_OnInitDialog |
| 0x41baac | cDialog_ctor |
| 0x41bb8b | cDialog_dtor |
| 0x41c43b | cDialog_RunModal |
| 0x41cf8a | cGadget_InitCommon |
| 0x41d076 | cGadget_ctorFresh |
| 0x41d4b5 | cGadget_DrawSelectionFrame |
| 0x41dac5 | cGadget_HitTest |
| 0x41ddb5 | cGadget_NearestFreeInput |
| 0x41df70 | cGadget_NearestFreeOutput |
| 0x41dbfd | cGadget_NearestFreeKnot |
| 0x41e12c | cGadget_SetKnotWire |
| 0x41e14c | cGadget_ClearKnotWire |
| 0x41e167 | cGadget_KnotPos |
| 0x41e1d3 | cGadget_OffsetRaw |
| 0x41e21c | cGadget_Offset |
| 0x41e4bd | cGadget_Bounds |
| 0x41e762 | cGadget_IntervalElapsed |
| 0x41e824 | cGadget_Tag |
| 0x41e82f | cGadget_ResId |
| 0x41e83b | cGadget_HasDialog |
| 0x41e846 | cGadget_Pos |
| 0x41eb68 | cGadget_DialogCallback |
| 0x41eb78 | cGadget_DialogPreview |
| 0x41ebd7 | cGadget_NearestKnot |
| 0x41ed78 | cGadget_IsLocked |
| 0x41ed83 | cGadget_SetLocked |
| 0x41ef27 | GadgetSaveId |
| 0x4115a8 | cPtrList_PushFront |
| 0x4115f6 | cPtrList_PushBack |
| 0x4116bb | cPtrList_InsertBefore |
| 0x41172e | cPtrList_Unlink |
| 0x41195a | MacSetRect |
| 0x41198a | MacRectFromPoints |
| 0x411b5f | MacUnionRect |
| 0x411c6f | MacSectRect |
| 0x411e03 | MacPtInRect |
| 0x423268 | cWorksheetPane_ctor |
| 0x4233dc | cWorksheetPane_dtor |
| 0x4234b8 | cWorksheetPane_Redraw |
| 0x4236ed | cWorksheetPane_Invalidate |
| 0x423756 | cWorksheetPane_MouseDown |
| 0x42385d | cWorksheetPane_KeyDown |
| 0x4238e5 | cWorksheetPane_KeyUp |
| 0x42391d | cWorksheetPane_GadgetAt |
| 0x42397d | cWorksheetPane_DrawEdit |
| 0x423b59 | cWorksheetPane_DrawRun |
| 0x423c2d | cWorksheetPane_EditKey |
| 0x423dc4 | cWorksheetPane_EditClick |
| 0x423e3f | cWorksheetPane_RunClick |
| 0x423ec3 | cWorksheetPane_PointerDown |
| 0x4244de | cWorksheetPane_DeleteSelectedGadget |
| 0x42450c | cWorksheetPane_AddGadget |
| 0x424571 | cWorksheetPane_AddWire |
| 0x4245d6 | cWorksheetPane_DeleteGadget |
| 0x4246be | cWorksheetPane_DeleteWire |
| 0x424778 | cWorksheetPane_Idle |
| 0x424944 | cWorksheetPane_PauseClock |
| 0x42498f | cWorksheetPane_Tick |
| 0x424a12 | cWorksheetPane_StartRun |
| 0x424a89 | cWorksheetPane_RunTime |
| 0x424a94 | cWorksheetPane_StopRun |
| 0x424b27 | cWorksheetPane_Clear |
| 0x424bf2 | cWorksheetPane_FindGadgetById |
| 0x424c45 | cWorksheetPane_SavedSize |
| 0x424d0a | cWorksheetPane_Write |
| 0x42503e | cWorksheetPane_Read |
| 0x425316 | cWorksheetPane_InvalidateAll |
| 0x42533a | cWorksheetPane_WireAt |
| 0x42539a | cWorksheetPane_DeleteSelectedWire |
| 0x4253c8 | cWorksheetPane_Deselect |
| 0x425431 | cWorksheetPane_SelectWire |
| 0x4254ba | cWorksheetPane_SelectGadget |
| 0x4255ba | cWorksheetPane_CreateGadgetSelected (unused) |
| 0x42566c | cWorksheetPane_CutDown |
| 0x425800 | cWorksheetPane_Hover |
| 0x425909 | cWorksheetPane_CursorFor |
| 0x425bac | cWorksheetPane_DeleteSelectedJoint |
| 0x425bf2 | cWorksheetPane_SelectJoint |
| 0x425c6d | cWorksheetPane_JointAt |
| 0x425cce | cWorksheetPane_FileVersion |
| 0x425cda | cWorksheetPane_ClearWorkArea |
| 0x425dbe | cWorksheetPane_RemoveAllConnections |
| 0x425e44 | ScreenToWorksheet |
| 0x425eaa | WorksheetToScreen |
| 0x425f10 | cWorksheetPane_TryOffsetAll |
| 0x426313 | cWorksheetPane_PanWidget |
| 0x4263f1 | cWorksheetPane_Background |
| 0x4263fc | cWorksheetPane_EditLocked |
| 0x426407 | cWorksheetPane_RemoveAllJoints |
| 0x426488 | cWorksheetPane_ToggleEditLocked (unused) |
| 0x426511 | cWorksheetPane_UnlockAll |
| 0x42659d | MarkUsedResources |
| 0x426629 | cWorksheetPane_SetSize |
| 0x426878 | cWorksheetPane_DragGadget |
| 0x426b94 | cWorksheetPane_DragJoint |
| 0x426df7 | cWorksheetPane_NewWire |
| 0x4272f7 | cWorksheetPane_MoveWireEnd |
| 0x4277d9 | CWGadgetDragger_ctor |
| 0x42795e | CWGadgetDragger_dtor |
| 0x4279e2 | CWGadgetDragger_MoveTo |
| 0x427b1f | CWGadgetDragger_ScreenRect |
| 0x427cc9 | cOffscreenUpdateList_AddGadget |
| 0x427e91 | cOffscreenUpdateList_AddSegment |
| 0x4280f5 | cOffscreenUpdateList_AddKnot |
| 0x428002 | cOffscreenUpdateList_SaveUnder |
| 0x428033 | cOffscreenUpdateList_RestoreUnder |
| 0x428064 | cOffscreenUpdateList_Draw |
| 0x428f1c | ComputeFitParams |
| 0x429060 | cWorksheetPane_Squeeze |
| 0x4291d9 | cWorksheetPane_Extents |
| 0x42932b | CheckMacFitOnSave |
| 0x4294fe | OnFitWorkArea |
| 0x429591 | AskFitToScreen |
| 0x4298b9 | CWrksFitDialog_OnCommand |
| 0x429b4a | CStdWinFitRectDialog_OnCommand |
| 0x42e880 | CWTrayFolder_OnLButtonDown (tray pick-up) |
| 0x42ef7e | CWTray_ReturnOne |
| 0x42f060 | CWTray_AdjustCount |
| 0x4325e0 | CWTrayToolbar_SelectTool |
| 0x432683 | CWTrayToolbar_CurrentTool |
| 0x4326e0 | CWTrayToolbar_KeyDown |
| 0x432824 | CWTrayToolbar_KeyUp |
| 0x432867 | CWTrayToolbar_ReturnGadget |
| 0x432921 | CWTrayToolbar_AdjustCount |
| 0x4329e0 | CWTrayToolbar_ResetCounts |
| 0x432a57 | CWTrayToolbar_GmunSize |
| 0x432aa5 | CWTrayToolbar_WriteGmun |
| 0x432b3c | CWTrayToolbar_ReadGmun |
| 0x430e70 | ControlPanel_Run |
| 0x430ee7 | ControlPanel_Stop |
| 0x430f63 | ControlPanel_KeyDown |
| 0x430d2c | ControlPanel_OnCommand |
| 0x4331ad | CWidgetApp_OnIdle |
| 0x4343f2 | CWidgetView_OnLButtonDown |
| 0x4344d2 | CWidgetView_OnLButtonDblClk |
| 0x434451 | CWidgetView_OnKeyDown |
| 0x43449b | CWidgetView_OnKeyUp |
| 0x434550 | CWidgetView_UpdateCursor |
| 0x434600 | CWidgetView_WindowProc |
| 0x43469e | CWidgetView_SetInputTool |
| 0x4346f3 | CWidgetView_EndInputTool |
| 0x43424e | CWidgetView_OnSize |
| 0x43480e | CWidgetView_OnFirstPaint |
| 0x435c60 | TypeNumberToTag |
| 0x435c93 | TagToTypeNumber |
| 0x4359f7 | CMainFrame_OnActivateApp |
| 0x4390b4 | CWInputTool_ctor |
| 0x4390d5 | CWInputTool_Dispatch |
| 0x439237 | CWPlaceGadgetTool_ctor |
| 0x439580 | CWPlaceGadgetTool_dtor |
| 0x4395f5 | CWPlaceGadgetTool_OnLButtonUp |
| 0x439b64 | CWPlaceGadgetTool_OnMouseMove |
| 0x439a12 | CWorksheetDragTool_dtor |
| 0x439b88 | CWCursor_Load |
| 0x439d44 | CWCursor_SetIndex |
| 0x43a424 | cConnection_ctor |
| 0x43a489 | cConnection_Draw |
| 0x43a976 | cConnection_SegmentAtEnd |
| 0x43a9d2 | cConnection_JointIndex |
| 0x43aa38 | cConnection_FloatSegment |
| 0x43aa5f | cConnection_UnfloatSegment |
| 0x43aa92 | cConnection_SegmentEnds |
| 0x43aba1 | cConnection_SaveSegment |
| 0x43ac69 | cConnection_RestoreSegment |
| 0x43ad31 | cConnection_DrawSegment |
| 0x43af27 | cConnection_dtor |
| 0x43b010 | cConnection_ctorLoad |
| 0x43b4f6 | cConnection_SavedSize |
| 0x43b512 | cConnection_Write |
| 0x43b780 | cConnection_Invalidate |
| 0x43b7ca | cConnection_HitTest |
| 0x43bb96 | cConnection_DrawSelectedSegment |
| 0x43beac | cConnection_Bounds |
| 0x43c0d4 | cConnection_Attach |
| 0x43c21f | cConnection_SetSource |
| 0x43c24f | cConnection_SetDest |
| 0x43c281 | cConnection_SetFreeEnd |
| 0x43c298 | cConnection_IsSnapped |
| 0x43c2a3 | cConnection_DetachSource |
| 0x43c2d5 | cConnection_DetachDest |
| 0x43c30a | cConnection_JointAt |
| 0x43c3f0 | cConnection_RemoveJoint |
| 0x43c470 | cConnection_InsertJoint |
| 0x43c8a4 | cConnection_IsLocked |
| 0x43c8af | cConnection_SetLocked |
| 0x43c8c6 | cConnection_RemoveAllJoints |
| 0x43c942 | cConnection_OffsetJoints |
| 0x43c9c8 | cConnection_ScaleJoints |
| 0x43cb40 | cKnot_ctor (joint) |
| 0x43cb82 | cKnot_DrawAt |
| 0x43cc6c | cKnot_Draw |
| 0x43cc9b | cKnot_DrawFrameAt |
| 0x43cdfd | cKnot_DrawFrame |
| 0x43ce2c | cKnot_Near |
| 0x43ceae | cKnot_Wire |
| 0x43ceda | cKnot_SetPos |
| 0x43cf14 | cKnot_Invalidate |
| 0x43d068 | Knot_Read (connector) |
| 0x43d0a9 | Knot_Wire |
| 0x43d0b3 | Knot_HasWire |
| 0x43d014 | Knot_Kind |
| 0x43d020 | Knot_Pos |
| 0x43d0c4 | cBackground_ctor |
| 0x43d209 | cBackground_Draw |
| 0x43d39b | cBackground_DrawBorder |
| 0x43db97 | cBackground_Set |
| 0x43849e | DibLine |
| 0x4386d7 | DibSaveLine |
| 0x43891a | DibRestoreLine |
| 0x43817d | ClipLine |
| 0x438024 | DibFillRect |
| 0x43746e | NearestPaletteIndex |

---

## 13. Open questions / uncertain points

- **Edit-mode repaint draws every gadget twice** (two identical loops in
  0x42397d). I read the decompile and it really is the same loop twice;
  harmless for pure drawing, but a gadget whose draw has side effects would
  see two calls. A port can draw once unless a gadget turns out to depend
  on it.
- The **tray-pane / control-panel identity** (view+0x6e0/HWND+0x6fc vs
  view+0x34/HWND+0x50) is inferred from: run hides +0x6fc, the place tool
  cancels over +0x6fc, the tool radio buttons and tray counts live in
  `DAT_004a2864`, GO/STOP/Light/Sound in `DAT_004a2868`. The layout agent
  should confirm.
- The `CWorksheetDragTool` / `CWPlaceGadgetTool` / `CWInputTool` vtable
  assignment (0x4964c0 / 0x496528 / 0x4964f4) is by behaviour and object
  sizes; the RTTI descriptors aren't directly tied to the vtables.
- `cKnotUpdateElement` = the joint element is by elimination (three element
  classes, the other two clearly gadget and segment) plus the `cKnot` RTTI
  name sitting next to the joint code.
- The tray-count adjust keys (numpad +/−) and L (lock) look like
  puzzle-designer features left enabled; I documented what the code does.
- 0x41a686's radio-group handling for notification codes (0x300 =
  EN_CHANGE, 3 = ?) and the combo-box resource format (0x41abc2) were only
  skimmed; the dialog controls themselves belong with the gadget/dialog
  docs.
- After `Squeeze`, gadget positions use `__ftol` (0x480ec4). I assumed
  truncation toward zero (Borland's `__ftol` semantics); not verified by
  stepping.
- Clear Work Area, Remove All Connections/Joints and the settings dialog's
  OK don't set the dirty flag (no call to 0x416142). This looks like an
  original bug; a faithful port reproduces it, a friendly one may not.
