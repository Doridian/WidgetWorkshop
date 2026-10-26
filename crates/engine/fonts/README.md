# The port's fonts

Fallbacks for the game's bitmap fonts, used behind any the CD carries
(only the Mac/PC CD has Windows' own, in `WIN31/SVGA`), so that a CD
without them, such as the OS/2 one, still draws its text.

- `SSERIFE.FON` (MS Sans Serif 8, 10 and 12 pt at 96 dpi) and `SMALLE.FON`
  (Small Fonts 7 pt): Wine's free replacements for Windows' fonts of the
  same names, with the same metrics (every ASCII advance width matches
  Windows'). © 2004, 2006 Huw D M Davies, Dmitry Timoshkov; from Wine
  (`fonts/sserife.sfd`, `fonts/smalle.sfd` in its source,
  <https://gitlab.winehq.org/wine/wine>), under the GNU LGPL 2.1 or later
  ([LICENSE-LGPL-2.1.txt](LICENSE-LGPL-2.1.txt)).
- `WSANS16.FNT`, `WSANS16B.FNT`, `WSANS24.FNT` ("Widget Sans" 16, 16 bold,
  24 pixels): Liberation Sans 2.1, which has Arial's metrics, drawn as
  bitmaps by `tools/make-sans-font.py` for the gadgets' "Helvetica" (Arial
  under Windows). © 2010 Google Corporation, © 2012 Red Hat, Inc., under
  the SIL Open Font License 1.1
  ([LICENSE-OFL-1.1.txt](LICENSE-OFL-1.1.txt)); being a modified version,
  it doesn't carry the Reserved Font Names "Liberation" or "Arimo".
