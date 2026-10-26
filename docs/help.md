# Help: `WIDGET.HLP` and the Help menu

What the original does with on-line help, what the help file contains,
and a plan for an in-game viewer the engine draws in the Win95 style it
uses for the rest of the UI. The parser is `crates/formats/src/hlp.rs`
(`formats::hlp::HelpFile`).

---

## 1. What `WIDGET.EXE` does

There's one way into help: **Help ▸ Contents** (command `0x8010`, also on
the F1 accelerator). Its handler is `0x4333f5`, reached from the
`WM_COMMAND` message-map entry at file offset `0x7a030`
(`{0x111, 0, 0x8010, 0x8010, sig 0x0a, 0x4333f5}`). It:

1. builds `<drive><dir><exe name>.HLP` from the module path with
   `_splitpath`/`_makepath` (`0x47f2ac`, `0x47e090`). The help file has to
   sit next to the EXE and share its base name, so it's `WIDGET.HLP`;
2. switches to that drive and folder (`_getdrive`/`_chdrive`/`_chdir`,
   `0x480410`/`0x480420`/`0x480164`);
3. while `_access(path, 0)` fails, shows "Can't find help file\nIt should
   be in the same folder as Widget Workshop and be named WIDGET.HLP"
   through `0x45f99f` (an `AfxMessageBox` with Retry/Cancel). Cancel (`2`)
   gives up silently;
4. if the file exists, calls `WinHelp(mainWnd, path, HELP_CONTENTS (3), 0)`;
5. restores the previous drive and folder.

**No context-sensitive help.** `WinHelpA` is called from only three places:

| address | what |
|---|---|
| `0x4333f5` | the Help ▸ Contents handler above: `HELP_CONTENTS` |
| `0x4733b1` | MFC's `CWinApp::WinHelp` (help path at app `+0x80`, error `0xf107` = `AFX_IDP_FAILED_TO_LAUNCH_HELP`). It's only a vtable slot. No message map has an `ID_HELP`, `ID_HELP_INDEX` or `ID_CONTEXT_HELP` command handler, so nothing reaches it |
| `0x4766e6` | MFC's `CFrameWnd::OnDestroy`: `WinHelp(hwnd, NULL, HELP_QUIT, 0)` |

`HELP_CONTEXT` is never used, Shift+F1 does nothing, and the gadget
settings dialogs' **Doc** button isn't WinHelp. It's the in-game Doc
balloon, with text from the EXE's resources (see
[dialogs.md](dialogs.md)). The `|CTXOMAP` ids 1–11 below are there only
because Doc-To-Help writes a `[MAP]` section.

The Windows 3.1 (Win32s) build has the same strings and logic. The two
`WIDGET.HLP` files (`WIN95/WIDGET`, `WIN31/WIDGET`) are byte-identical.
No `WIDGET.CNT` ships, even though `WIDGET.GID` (WinHelp 4's cache,
written on the developer's machine, `T:\WWWIN95\...`) mentions one. So
WinHelp 95 opens straight on the contents topic, with no Help Topics
dialog.

The port: `crates/engine/src/editor/mod.rs` currently answers `0x8010`
with "Not available in this version yet."

---

## 2. The file

`WIDGET.HLP` is 327,985 bytes. It was compiled by HC 3.1 (`|SYSTEM`
version 1.21, flags 4 = LZ77 with 4 KiB topic blocks), authored in
**Doc-To-Help** (`D2HTOOLS.DLL` ships next to it). 89% of the file is
one screenshot.

### 2.1 Internal files

| file | bytes | contents |
|---|---:|---|
| `help0001.bmp` | 249,334 | baggage: 574×431 8-bit BMP, a screenshot of the whole game window |
| `help0002.bmp` | 7,078 | 60×100, the Control Box |
| `help0003.bmp` | 45,766 | 455×98, the Tooltray |
| `help0004.bmp` | 3,238 | 60×36, Pointer tool button |
| `help0005.bmp` | 2,294 | 31×38, Cut tool button |
| `help0006.bmp` | 2,262 | 31×37, Move (hand) tool button |
| `\|TOPIC` | 8,360 | 3 blocks, unpacking to about 11.6 KB of records |
| `\|Phrases` | 1,427 | 194 phrases (LZ77-packed, 1,239 bytes unpacked) |
| `\|SYSTEM` | 302 | see 2.2 |
| `\|FONT` | 293 | 6 face names, 15 descriptors |
| `\|CONTEXT` | 2,086 | B+ tree: 11 hashes → topic offsets |
| `\|CTXOMAP` | 90 | 11 `[MAP]` ids (1–11 = topics 0–10 in order) |
| `\|TTLBTREE` | 2,086 | topic offset → title (same titles as the topic headers) |
| `\|KWBTREE`, `\|KWDATA`, `\|KWMAP` | | keyword index: 11 keywords, each a topic's own title, one topic each |

All bitmaps are plain uncompressed 8-bit `BI_RGB` DIBs with their own
palettes; `formats::bmp` reads them. There are no `|bm*` pictures (no
SHG/MRB), no `|PhrIndex`/`|PhrImage`, no `|VIOLA`, `|CATALOG`, tables or
popups.

### 2.2 `|SYSTEM`

- title "Widget Workshop", copyright "® 1995 Maxis, Inc.", contents topic
  offset 0;
- startup macros: `RegisterRoutine("d2htools.dll","LoadDLL","")`,
  `RegisterRoutine("d2htools.dll","JumpSecondaryID","SSS")`, `LoadDLL()`,
  `BrowseButtons()`. Only `BrowseButtons()` matters to a viewer: it adds
  `<<` and `>>`. `JumpSecondaryID` is never called;
- one window, `main`: caption `" Widget Workshop"` (leading space as
  stored), maximize flag 0, no position, size or colours.

### 2.3 Topics

11 topics. The compiler also writes an empty untitled 12th topic as an
end marker, which the parser drops. Every topic has a one-line
non-scrolling heading (Arial 18 pt bold; the contents topic's is
"Contents" in Arial 24 pt bold).

| # | title | offset | browse | content |
|---|---|---:|---|---|
| 0 | Help Contents | 0 | – | "Welcome to Widget Workshop", 10 jump hotspots in two groups ("What Is…", "Using Widget Workshop") |
| 1 | What Is Widget Workshop, Anyway? | 354 | seq A | 1 paragraph |
| 2 | What's a Widget? | 605 | A | 2 paragraphs |
| 3 | What Can I Do with Widget Workshop? | 893 | A | 2 paragraphs |
| 4 | Hey! Wait a Minute! Fun with Science??? | 1398 | A | prose and a two-column tabbed list |
| 5 | What's all that Other Stuff in the Box? | 2291 | A (end) | 1 paragraph |
| 6 | The Widget Screen | 2630 | seq B | `help0001.bmp` and 6 paragraphs |
| 7 | The Control Box | 3333 | B | `help0002.bmp` and 5 bold one-liners |
| 8 | The Tooltray | 3693 | B | `help0003.bmp`; the tool buttons `help0004–6` inline in text lines |
| 9 | The Menus | 4571 | B | the longest: File/Edit/Options/Help menus, bold item names, three boxed notes |
| 10 | Widget Troubleshooting | 35832 (block 1) | B (end) | Q&A, two bulleted items |

Browse sequences: 1→2→3→4→5 and 6→7→8→9→10, with no link between them.
The contents topic has none, so `<<`/`>>` are disabled on it. All text
totals about 13.5 KB, roughly 2,200 words, in about 120 paragraphs.

### 2.4 Formatting actually used

- **Fonts** (index → descriptor): 0–2 Helv 10 pt (unused); 3 Arial 24 b;
  4 Arial 14 b; 5 Arial 11 b (contents links); 6 Arial 11 bi; 7 Arial
  11 b double-underlined (the RTF hotspot marker; carries no text);
  8 Arial 18 b (topic headings); 9 Arial 18 bi; 10 Arial 10 (body);
  11 Symbol 10; 12 Arial 10 i; 13 Arial 10 b; 14 Arial 12 b (menu
  headings). Every colour is the "automatic" marker (`01 01 00`): black
  text on the window colour.
- **Symbol-font characters**: `0xBE` (horizontal bar, used as an em dash,
  "puzzles⎯and") and `0xB7` (bullet). Neither the CD's bitmap fonts nor
  Windows-1252 have these in that slot, so the viewer must map them:
  `0xBE` → a dash about one em wide, `0xB7` → a bullet.
- **Paragraph format**: space above/below, line spacing ("at least"),
  left indent and negative first-line indent (the bullets: left 72,
  first −35), tab stops (topic 4: 36, 324, 360), centring isn't used, and
  borders (`sides` 0x0a = top and bottom) on five boxed notes: one in
  topic 8, three in topic 9 ("Be sure to save…", "Stand-Alone widgets…",
  "Be careful…") and the closing line of topic 10. Units are tenths of a twip (helpdeco
  multiplies by 10 to get twips), so at 96 dpi a unit is ⅔ pixel: 72 → 48 px.
- **Hotspots**: only visible jumps (`0xE3` + context hash), all on the
  contents topic. No popups, no macro hotspots, no hotspots on pictures.
- **Pictures**: only `{ewc D2HTools, D2H_256Color, helpNNNN.bmp}` embedded
  windows (picture type 5), all `Placement::Inline`. Width and height are
  stored as 0: the DLL sizes the window to the bitmap. 0001–0003 are on
  their own lines; 0004–0006 sit inside a text line ("Click [icon] to
  activate the Pointer tool…"), taller than the 10 pt line around them.

### 2.5 Format notes (where helpdeco's description needed checking)

- Topic offsets (`TOPICOFFSET`, used by `|CONTEXT`, `|CTXOMAP`,
  `|KWDATA` and the browse links) are `block × 0x8000 + characters`. The
  character count is the sum of the text records' `TopicLength` since the
  first record that *starts* in that block. A record that runs across a
  block end counts toward its starting block. This was checked against
  every context, map and browse value in the file.
- A text record's `data1` is: compressed long (size of the rest), compressed
  unsigned short (character count), 4 unknown bytes (`00 80 00 00`), `u16`
  bits, then the optional fields the bits select. After that come the
  commands, each preceded by one NUL-terminated string from `data2`.
- `|CONTEXT` leaves are sorted by the hash as a *signed* 32-bit number. The
  parser re-sorts them unsigned for its binary search.
- The context strings themselves aren't in the file, only their hashes, so
  name → hash isn't needed (and isn't implemented).

---

## 3. Plan for the in-game viewer

The goal is to look like WinHelp 95 (`WINHLP32`) showing a WinHelp 3.1
file without a `.CNT`, drawn by the engine with its Win95 chrome and the
CD's bitmap fonts (`WIN31/SVGA/SSERIFE.FON`: MS Sans Serif 8, 10, 12, 14,
18, 24 pt; there's no Arial).

### 3.1 Loading

- Load `WIDGET.HLP` from the game folder (`assets::GAME_DIRS`, like
  `PUZZLE_FILE`) into `Assets`, parsed lazily the first time Help opens.
  If it's missing, show the original's message ("Can't find help
  file…", §1) with OK, not Retry/Cancel, since nothing can make it appear.
- `0x8010`/F1: open (or raise) the help window on `contents_topic()`.

### 3.2 The window

A separate top-level, movable and resizable window (in WinHelp it's a
separate application). It has the standard caption, min/max/close boxes
and frame, with the caption `Widget Workshop` (the window's `caption`,
trimmed, or `HelpFile::title`). The window record sets no position or
size. Good defaults are about 2/3 of the screen width and the full height,
placed at the right, or a remembered rectangle.

From top to bottom:

1. **Menu bar** (optional for a first cut): File (Open…, Print Topic…,
   Exit), Edit (Copy…, Annotate…), Bookmark (Define…), Options (Keep Help
   on Top, Display History Window, Font, Use System Colors), Help (Version…).
   Only Exit and maybe Keep on Top are worth wiring.
2. **Button bar**: a row of push buttons with text labels, all the same
   height and wide enough for the text (MS Sans Serif 8):
   `Contents` `Index` `Back` `Print` `<<` `>>`.
   - *Contents* → `contents_topic()`; disabled when already on it? (WinHelp
     keeps it enabled; harmless either way.)
   - *Index* → a modal dialog "Index" listing `keywords` (an edit field
     with incremental match above a list box, plus Display and Cancel).
     Each keyword has exactly one topic, so Display jumps straight there.
   - *Back* → pop a history stack of topic offsets. Disabled when it's empty.
   - *Print* → can be a no-op, disabled, or the engine's print path.
   - `<<` / `>>` → `topic_at(browse_prev/browse_next)`. Disabled when
     `None`. These exist because of `BrowseButtons()`.
3. **Non-scrolling region**: the topic's `non_scroll` paragraphs. It has
   the window background (no colour is set), a 1-px separator line below,
   and doesn't scroll.
4. **Topic pane**: `scroll` paragraphs on the window colour (white), with a
   vertical scroll bar when the content is taller than the pane and a
   horizontal one when it's wider (`help0001.bmp` is 574 px wide).

### 3.3 Layout

- Paragraph box: left margin ≈ 8 px plus `left_indent`, right margin plus
  `right_indent`. Before it, `space_above`; after it, `space_below`.
  Lines are wrapped at spaces (0xA0 doesn't break). A line's height is
  max(font line height, tallest inline picture, `line_spacing` if set).
  The first line starts at `first_line_indent` (negative = hanging; the
  bullet sits there and the tab goes to `left_indent`). Convert units with
  `px = units × 2 / 3`.
- Tabs: go to the next `tabs` stop beyond the pen (relative to the
  paragraph's left edge), else the next multiple of the default (0.5 inch,
  48 px).
- `Align::Centre`/`Right` aren't used by this file.
- Borders: `sides` 0x02 top, 0x08 bottom, drawn as 1-px lines across the
  paragraph's width, a couple of pixels outside the text.
- Pictures: `Picture::baggage_bitmap()` → `HelpFile::bitmap()`, converted
  to RGB through its own palette. It sits on the baseline as an inline box.
- Fonts: map each `Font` to the nearest MS Sans Serif size from
  `half_points / 2` (10 → 10 pt, 11 → 10 or 12, 14 → 14, 18 → 18, 24 → 24),
  and draw bold by over-striking 1 px to the right (the CD has no bold or
  italic `.FON`). Italic is a synthetic shear, or ignore it. Underline is
  drawn by hand. Symbol runs use the mapping in §2.4.

### 3.4 Hotspots

- Draw visible jump text in green (`#008000`) with a solid underline.
  Popups, not used here, get a dotted underline. While the mouse is over a
  hotspot, use the hand cursor.
- Click → `topic_for_hash(hash)`, push the current topic on the Back
  stack, show the new one scrolled to the top.
- Tab/Shift+Tab cycle a focus rectangle through hotspots; Enter follows
  (WinHelp 4 behaviour, nice to have).
- Popups (`LinkAction::Popup`), if a future file needed them: a borderless
  window with a 1-px black frame and drop shadow, sized to the wrapped
  text (max ≈ 2/3 of the help window), closed by any click or key.

### 3.5 Effort estimate

Small. There are 11 topics, about 120 paragraphs, 12 font descriptors in
use, 6 images, 10 links, no popups, no tables, and no picture hotspots.
Most of the work is the paragraph layout engine (wrap, tabs, hanging
indents, inline images) and the window chrome (button bar, two scroll
regions, Index dialog). `formats::hlp` already resolves everything else.

---

## 4. Open questions

- Window placement and size: WinHelp remembers them in `WINHELP.INI`, and
  the file gives none. Pick a default.
- The four unknown bytes per text record (`00 80 00 00` throughout) and the
  `0x0001` bit's long are ignored. They don't vary in this file.
- The 16-bit value after the border sides (helpdeco calls it the width)
  is 252 on two notes and 79 on three. Its meaning is unclear, so draw
  1-px lines.
- Whether the port should copy the original's habit of launching help
  as a separate, independently living window (closed on app exit,
  `HELP_QUIT`), or make it a child panel of the game window.
