# The OS/2 Warp CD

Besides the Mac/PC CD, the port reads the OS/2 Warp CD (`WW1_01A.ISO`,
12 MB). Its game folder is `WIDGET/`, with the same four modules,
`WIDGET.PZL`, `WIDGET.SLF`, help as `WIDGET.INF` (OS/2's own format, not
`WIDGET.HLP`) and the sample widgets in `WIDGET/WIDGETS/` (the Win95 CD's
`FUNWIDGE/`; the same files also lie loose in `WIDGET/`). It has no
Windows fonts.

## Modules

The modules are 32-bit OS/2 executables (LX, `formats::lx`). A resource
is a slice of an object, whose pages are stored plain, zero-filled or
packed; the DLLs' pages are plain, `WIDGET.EXE`'s are `EXEPACK2`.
`assets` turns each module into the Win95 build's resources on loading
(`formats::os2::to_win`), so the rest of the port reads one kind:

- The game's own types (1003 palette, 1004 pictures, 1006 sounds, 1010 to
  1014 tables, 1020 texts, 1021, 1022 combo lists, 1023, 20000) are the
  same format. 1022's text is in code page 437 on OS/2 (the Converter's
  degree signs) and becomes Windows-1252.
- Dialogs (`RT_DIALOG` 4) become Win32 templates (`RT_DIALOG` 5), same
  ids but the puzzle dialog (260 → 10000). Presentation Manager's
  template is a 14-byte header and 30-byte items, the first the frame,
  the controls its children; classes are `WC_*` numbers (frame 1,
  combo box 2, button 3, static 5, entry field 6, list box 7, scroll bar
  8, MLE 10) and the styles are PM's, mapped to the Win32 ones the
  dialogs check. `y` counts up from the bottom: `y_win = cy - y - cy_item`
  (the frame's size is the client area's, as in Win95). An entry field's
  `ES_MARGIN` border lies outside its rectangle, so it grows 2 units each
  way. Mnemonics are `~`, made `&`. The layouts are OS/2's own (narrower:
  they were made for Helv 8), drawn in MS Sans Serif like the Win95 ones;
  the hidden title statics (105) are missing.
- The menu bar (`RT_MENU` 3, id 1) becomes `RT_MENU` 4 id 3. Its commands
  are numbered anew (`~New` 0x8000, ...) and get the Win95 build's
  (`os2::MENU_IDS`). It reads "Export a Stand Alone Widget", "Page Setup"
  and "About Widget Workshop for OS/2", has Cut, Copy, Paste and Clear
  (MFC's `ID_EDIT_*`, which the port doesn't handle) and lacks Print
  Preview and Fit Work Area.
- Accelerator table 2 (`RT_ACCELTABLE` 8) becomes `RT_ACCELERATOR` 9 id 2,
  PM's virtual keys mapped to Windows' (it comes out byte for byte as the
  Win95 one). Table 1, the frame's own keys, is left out.
- String tables (`RT_STRING` 5: a code page, then 16 strings with a
  length byte counting the NUL) become UTF-16 blocks (`RT_STRING` 6), the
  same block ids. OS/2's strings have no line breaks; string 100 reads
  "Release version 1.16".
- The editor's seven pointers (`RT_POINTER` 1, ids 4135 to 4141; `CP`
  colour pointers: a 1-bit AND/XOR mask twice as high, then a 4-bit
  image, the hot spot from the bottom) become cursor groups 5007 to 5013,
  each with an `RT_CURSOR` numbered like the pointer. They're drawn anew
  and in another order: 4135 glove, 4140 scissors, 4137 and 4138 plug,
  4136 grabbing hand, 4139 open hand, 4141 a pointing hand of its own
  (Win95's 5013 repeats the grabbing hand).

## Data that differs

- Pictures 1024, 1029 to 1032, 204 and 899 of `WWGADGET.DLL` differ in a
  few bytes; sound 1282 of `WWSOUND.DLL` is 20 bytes shorter; one palette
  entry differs.
- The credits (`WIDGET.EXE` 1020/10000) add "OS/2 Port by WinWare
  Corporation".
- `WIDGET.PZL` has the same layout but differs in places (puzzle 2 has 8
  parts, not 10), and its `Done` resource marks every puzzle solved;
  `Assets::load` clears those flags when the modules are OS/2's.
- `WIDGET.SLF` is OS/2's runtime; Save Into a Stand-Alone Widget needs the
  Win95 one.
