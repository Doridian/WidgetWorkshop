//! The OS/2 build's Presentation Manager resources turned into the Win32
//! ones the Win95 build has, so that the rest reads either CD the same
//! way ([`to_win`]): dialogs, the menu, accelerators, strings and the
//! editor's pointers. The game's own types (1003 and up) are the same
//! bytes in both and pass through.
//!
//! Where the numbers differ they're mapped:
//!
//! - the puzzle dialog is 260 (Win95: 10000), the menu bar 1 (3);
//! - the menu's commands are numbered anew (`~New` 0x8000, ...): they get
//!   the Win95 build's ([`MENU_IDS`]), as the accelerators already have;
//! - the editor's pointers 4135 to 4141 are cursor groups 5007 to 5013
//!   (in another order, [`POINTERS`]), each with one `RT_CURSOR`
//!   numbered like the pointer;
//! - Presentation Manager's own types (pointers other than those, the
//!   frame's accelerators, help tables, ...) are left out.

use crate::{lx, pe};

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*d.get(at)?, *d.get(at + 1)?]))
}

#[allow(clippy::cast_possible_wrap)]
fn i16_at(d: &[u8], at: usize) -> Option<i16> {
    u16_at(d, at).map(|v| v as i16)
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

/// Code page 437's upper half (the templates' code page, and the combo
/// lists') as Windows-1252, `?` where it has no match.
const CP437: [u8; 128] = [
    0xc7, 0xfc, 0xe9, 0xe2, 0xe4, 0xe0, 0xe5, 0xe7, 0xea, 0xeb, 0xe8, 0xef, 0xee, 0xec, 0xc4, 0xc5, 0xc9, 0xe6, 0xc6,
    0xf4, 0xf6, 0xf2, 0xfb, 0xf9, 0xff, 0xd6, 0xdc, 0xa2, 0xa3, 0xa5, 0x3f, 0x83, 0xe1, 0xed, 0xf3, 0xfa, 0xf1, 0xd1,
    0xaa, 0xba, 0xbf, 0x3f, 0xac, 0xbd, 0xbc, 0xa1, 0xab, 0xbb, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f,
    0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f,
    0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f,
    0x3f, 0x3f, 0xdf, 0x3f, 0x3f, 0x3f, 0x3f, 0xb5, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0x3f, 0xb1,
    0x3f, 0x3f, 0x3f, 0x3f, 0xf7, 0x3f, 0xb0, 0x3f, 0xb7, 0x3f, 0x3f, 0xb2, 0x3f, 0xa0,
];

/// Byte `c` of code page 437 as Windows-1252.
fn ansi(c: u8) -> u8 {
    if c < 0x80 { c } else { CP437[usize::from(c - 0x80)] }
}

/// Byte `c` of code page 437 as a character (by way of Windows-1252, read
/// as Latin-1: the game's texts are ASCII but for a degree sign).
fn char437(c: u8) -> char {
    char::from(ansi(c))
}

/// A NUL-terminated string at `at` (code page 437, ASCII in the game's).
fn c_string(d: &[u8], at: usize) -> Option<(String, usize)> {
    let s = d.get(at..)?;
    let n = s.iter().position(|&c| c == 0)?;
    Some((s[..n].iter().map(|&c| char437(c)).collect(), at + n + 1))
}

fn put_utf16(out: &mut Vec<u8>, s: &str) {
    for u in s.encode_utf16() {
        out.extend(u.to_le_bytes());
    }
    out.extend([0, 0]);
}

/// A caption with Presentation Manager's mnemonic (`~`) as Windows' (`&`).
fn mnemonic(s: &str) -> String {
    s.replace('&', "&&").replace('~', "&")
}

/// The parent's height in dialog units dialogs are placed against: OS/2
/// counts `y` up from its bottom; this puts the message boxes 10011,
/// 10012 and MFC's 30721 to 30723 where the Win95 templates have them.
const PARENT_H: i16 = 160;

/// How far an entry field's border (`ES_MARGIN`) reaches past its
/// rectangle, in dialog units: Presentation Manager draws it outside,
/// Windows inside.
const MARGIN: i16 = 2;

// Win32's window styles.
const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_DISABLED: u32 = 0x0800_0000;
const WS_BORDER: u32 = 0x0080_0000;
const WS_VSCROLL: u32 = 0x0020_0000;
const WS_HSCROLL: u32 = 0x0010_0000;
const WS_GROUP: u32 = 0x0002_0000;
const WS_TABSTOP: u32 = 0x0001_0000;

/// A control of a Presentation Manager dialog as Win32's: its class
/// ordinal (or a name, for the classes Windows lacks), style and caption.
/// `class` is the `WC_*` number, `s` the style.
fn control(class: u16, s: u32, text: &str) -> (Result<u16, String>, u32, String) {
    let mut w = WS_CHILD;
    for (os2, win) in [
        (0x8000_0000, WS_VISIBLE),
        (0x4000_0000, WS_DISABLED),
        (0x0001_0000, WS_GROUP),
        (0x0002_0000, WS_TABSTOP),
    ] {
        if s & os2 != 0 {
            w |= win;
        }
    }
    let flag = |bit: u32, win: u32| if s & bit != 0 { win } else { 0 };
    match class {
        // WC_BUTTON: BS_PUSHBUTTON (BS_DEFAULT), BS_CHECKBOX,
        // BS_AUTOCHECKBOX, BS_RADIOBUTTON, BS_AUTORADIOBUTTON, BS_3STATE,
        // BS_AUTO3STATE, BS_USERBUTTON.
        3 => {
            let kind = match s & 0xf {
                0 if s & 0x400 != 0 => 1,
                0 => 0,
                1 => 2,
                2 => 3,
                3 => 4,
                4 => 9,
                5 => 5,
                6 => 6,
                _ => 0xb,
            };
            (Ok(0x80), w | kind, mnemonic(text))
        }
        // WC_STATIC: SS_TEXT (aligned by DT_CENTER, DT_RIGHT), SS_GROUPBOX,
        // SS_ICON, SS_BITMAP, then the rectangles and frames in Windows'
        // order.
        5 => match s & 0x7f {
            1 => {
                let align = match s & 0x300 {
                    0x100 => 1,
                    0x200 => 2,
                    _ => 0,
                };
                (Ok(0x82), w | align, mnemonic(text))
            }
            2 => (Ok(0x80), w | 7, mnemonic(text)),
            3 => (Ok(0x82), w | 3, text.into()),
            4 => (Ok(0x82), w | 0xe, text.into()),
            k @ 5..=0xa => (Ok(0x82), w | (k - 1), String::new()),
            _ => (Ok(0x82), w, mnemonic(text)),
        },
        // WC_ENTRYFIELD: ES_CENTER, ES_RIGHT, ES_AUTOSCROLL, ES_MARGIN,
        // ES_READONLY, ES_UNREADABLE.
        6 => {
            let st = (s & 3) | flag(4, 0x80) | flag(8, WS_BORDER) | flag(0x20, 0x800) | flag(0x80, 0x20);
            (Ok(0x81), w | st, text.into())
        }
        // WC_MLE: an edit with ES_MULTILINE and ES_AUTOVSCROLL; MLS_BORDER,
        // MLS_VSCROLL, MLS_HSCROLL, MLS_READONLY.
        10 => {
            let st = 0x44 | flag(2, WS_BORDER) | flag(4, WS_VSCROLL) | flag(8, WS_HSCROLL) | flag(0x10, 0x800);
            (Ok(0x81), w | st, text.into())
        }
        // WC_LISTBOX: LBS_NOTIFY, a border and a scroll bar; LS_MULTIPLESEL,
        // LS_NOADJUSTPOS (LBS_NOINTEGRALHEIGHT), LS_HORZSCROLL.
        7 => {
            let st = 1 | WS_BORDER | WS_VSCROLL | flag(1, 8) | flag(4, 0x100) | flag(8, WS_HSCROLL);
            (Ok(0x83), w | st, String::new())
        }
        // WC_SCROLLBAR: SBS_VERT.
        8 => (Ok(0x84), w | (s & 1), String::new()),
        // WC_COMBOBOX: CBS_SIMPLE, CBS_DROPDOWN, CBS_DROPDOWNLIST.
        2 => {
            let kind = match s & 7 {
                1 => 1,
                2 => 2,
                _ => 3,
            };
            (Ok(0x85), w | WS_VSCROLL | kind, String::new())
        }
        _ => (Err(format!("PM#{class}")), w, text.into()),
    }
}

/// One item of a Win32 `DLGTEMPLATE`.
struct WinItem {
    style: u32,
    x: i16,
    y: i16,
    cx: i16,
    cy: i16,
    id: u16,
    class: Result<u16, String>,
    text: String,
}

/// A Presentation Manager dialog template (`DLGTEMPLATE`, `pmwin.h`) as a
/// Win32 one ([`crate::dialog::Dialog`] reads it): the frame window's size
/// and title, its controls' classes and styles mapped, in MS Sans Serif 8
/// as the Win95 build's.
///
/// The header is 14 bytes, the offset of the items at 6; an item is 30
/// bytes: status, children, the class (a name's length and offset, or
/// with no length a `WC_*` number), the text's length and offset, the
/// style, `x`, `y`, `cx`, `cy`, id, presentation parameters and control
/// data. The first is the frame, the rest its children (with theirs
/// following each). `y` counts up from the parent's bottom; the frame's
/// size is the client area's, as in the Win95 build.
#[must_use]
pub fn dialog(d: &[u8]) -> Option<Vec<u8>> {
    let first = usize::from(u16_at(d, 6)?);
    // Children, id, class, text, style, rectangle.
    let item = |i: usize| -> Option<(u16, u16, Result<u16, String>, String, u32, [i16; 4])> {
        let at = first + 30 * i;
        let children = u16_at(d, at + 2)?;
        let (ccls, ocls) = (u16_at(d, at + 4)?, usize::from(u16_at(d, at + 6)?));
        let class = if ccls == 0 {
            Ok(u16::try_from(ocls).ok()?)
        } else {
            Err(d
                .get(ocls..ocls + usize::from(ccls))?
                .iter()
                .map(|&c| char437(c))
                .collect())
        };
        let (ctxt, otxt) = (usize::from(u16_at(d, at + 8)?), usize::from(u16_at(d, at + 10)?));
        let text = if ctxt == 0 {
            String::new()
        } else {
            let t = d.get(otxt..otxt + ctxt)?;
            let t = t.split(|&c| c == 0).next().unwrap_or_default();
            t.iter().map(|&c| char437(c)).collect()
        };
        let style = u32_at(d, at + 12)?;
        let r = [
            i16_at(d, at + 16)?,
            i16_at(d, at + 18)?,
            i16_at(d, at + 20)?,
            i16_at(d, at + 22)?,
        ];
        Some((children, u16_at(d, at + 24)?, class, text, style, r))
    };
    let (children, _, _, title, fstyle, [fx, fy, fcx, fcy]) = item(0)?;
    let mut items = Vec::new();
    // The items follow in tree order: per level, how many are left and
    // their parent's origin.
    let mut stack: Vec<(u16, i16, i16)> = vec![(children, 0, 0)];
    let mut i = 1;
    while let Some(top) = stack.last_mut() {
        if top.0 == 0 {
            stack.pop();
            continue;
        }
        top.0 -= 1;
        let (ox, oy) = (top.1, top.2);
        let (n, id, class, text, style, [x, y, cx, cy]) = item(i)?;
        i += 1;
        let (x, y) = (ox.saturating_add(x), oy.saturating_add(y));
        // An entry field with ES_MARGIN.
        let m = if class == Ok(6) && style & 8 != 0 { MARGIN } else { 0 };
        let (wclass, wstyle, wtext) = match class {
            Ok(c) => control(c, style, &text),
            Err(name) => (Err(name), WS_CHILD | (style & 0x8000_0000) >> 3, text),
        };
        let (cx, cy) = (cx.saturating_add(2 * m), cy.saturating_add(2 * m));
        items.push(WinItem {
            style: wstyle,
            x: x.saturating_sub(m),
            y: fcy.saturating_sub(y.saturating_sub(m)).saturating_sub(cy),
            cx,
            cy,
            id,
            class: wclass,
            text: wtext,
        });
        if n > 0 {
            stack.push((n, x, y));
        }
    }
    // WS_POPUP, WS_CAPTION, WS_SYSMENU, DS_MODALFRAME, DS_SETFONT; visible
    // as the frame is.
    let style = 0x80c8_00c0 | (fstyle & 0x8000_0000) >> 3;
    let mut out = Vec::new();
    out.extend(style.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(u16::try_from(items.len()).ok()?.to_le_bytes());
    let y = PARENT_H.saturating_sub(fy).saturating_sub(fcy).max(0);
    for v in [fx, y, fcx, fcy] {
        out.extend(v.to_le_bytes());
    }
    // No menu, the standard class.
    out.extend([0, 0, 0, 0]);
    put_utf16(&mut out, &title);
    out.extend(8u16.to_le_bytes());
    put_utf16(&mut out, "MS Sans Serif");
    for it in items {
        out.resize(out.len().next_multiple_of(4), 0);
        out.extend(it.style.to_le_bytes());
        out.extend(0u32.to_le_bytes());
        for v in [it.x, it.y, it.cx, it.cy] {
            out.extend(v.to_le_bytes());
        }
        out.extend(it.id.to_le_bytes());
        match &it.class {
            Ok(c) => {
                out.extend(0xffffu16.to_le_bytes());
                out.extend(c.to_le_bytes());
            }
            Err(name) => put_utf16(&mut out, name),
        }
        put_utf16(&mut out, &it.text);
        out.extend(0u16.to_le_bytes());
    }
    Some(out)
}

/// The OS/2 menu's commands and the Win95 build's (docs/os2.md). Cut,
/// Copy, Paste and Clear, which the Win95 menu lacks, get MFC's
/// `ID_EDIT_*`, as the accelerators have them.
pub const MENU_IDS: [(u16, u16); 24] = [
    (0x8000, 0xe100), // New
    (0x8001, 0xe101), // Open
    (0x8007, 0x8003), // Load Puzzle
    (0x8006, 0xe103), // Save
    (0x8002, 0xe104), // Save As
    (0x8008, 0x8004), // Revert
    (0x8009, 0x8005), // Import a Stand Alone Widget
    (0x800a, 0x8006), // Export a Stand Alone Widget (Save Into)
    (0x8003, 0xe106), // Page Setup (Print Setup)
    (0x8004, 0xe107), // Print
    (0xf008, 0xe141), // Exit (SC_CLOSE)
    (0x8015, 0xe12b), // Undo
    (0x8018, 0xe123), // Cut
    (0x8019, 0xe122), // Copy
    (0x801a, 0xe125), // Paste
    (0x8020, 0xe120), // Clear
    (0x800c, 0x8009), // Clear work area
    (0x800d, 0x800a), // Remove all connections
    (0x800e, 0x800b), // Remove all joints
    (0x800f, 0x800c), // No Background
    (0x8010, 0x800d), // Metal waves
    (0x8011, 0x800e), // Noise
    (0x8012, 0x800f), // Steel
    (0x8017, 0xe140), // About
];

/// The Win95 menu's Contents (F1), the OS/2 one's User's Guide.
const MENU_HELP: (u16, u16) = (0xf180, 0x8010);

fn menu_id(id: u16) -> u16 {
    MENU_IDS
        .iter()
        .chain([&MENU_HELP])
        .find(|(o, _)| *o == id)
        .map_or(id, |(_, w)| *w)
}

/// A Presentation Manager menu template as a Win32 `MENUITEMTEMPLATE`
/// list ([`crate::dialog::parse_menu`]), the commands numbered as the
/// Win95 build's ([`MENU_IDS`]).
///
/// A template is its size (32 bits), the code page, a reserved word and
/// the item count; an item is its style (`MIS_TEXT` 1, `MIS_SEPARATOR` 4,
/// `MIS_SUBMENU` 0x10), attributes (`MIA_CHECKED` 0x2000, `MIA_DISABLED`
/// 0x4000), id, the text with `MIS_TEXT`, then with `MIS_SUBMENU` the
/// submenu's template.
#[must_use]
pub fn menu(d: &[u8]) -> Option<Vec<u8>> {
    let mut out = vec![0, 0, 0, 0];
    menu_items(d, 0, &mut out)?;
    Some(out)
}

fn menu_items(d: &[u8], at: usize, out: &mut Vec<u8>) -> Option<usize> {
    let size = u32_at(d, at)? as usize;
    let n = u16_at(d, at + 8)?;
    let mut i = at + 10;
    for k in 0..n {
        let style = u16_at(d, i)?;
        let attr = u16_at(d, i + 2)?;
        let id = u16_at(d, i + 4)?;
        i += 6;
        let mut text = String::new();
        if style & 1 != 0 {
            let (t, next) = c_string(d, i)?;
            text = mnemonic(&t);
            i = next;
        }
        // MF_GRAYED, MF_CHECKED, MF_POPUP, MF_END.
        let mut flags = 0u16;
        if attr & 0x4000 != 0 && style & 4 == 0 {
            flags |= 1;
        }
        if attr & 0x2000 != 0 {
            flags |= 8;
        }
        if k + 1 == n {
            flags |= 0x80;
        }
        if style & 0x10 != 0 {
            out.extend((flags | 0x10).to_le_bytes());
            put_utf16(out, &text);
            i = menu_items(d, i, out)?;
        } else if style & 4 != 0 {
            out.extend(flags.to_le_bytes());
            out.extend([0, 0, 0, 0]);
        } else {
            out.extend(flags.to_le_bytes());
            out.extend(menu_id(id).to_le_bytes());
            put_utf16(out, &text);
        }
    }
    Some(at + size)
}

/// Presentation Manager's virtual key `vk` as Windows'.
fn vk(vk: u16) -> Option<u16> {
    Some(match vk {
        // VK_BUTTON1 to 3.
        1 => 1,
        2 => 2,
        3 => 4,
        // VK_BREAK, VK_BACKSPACE, VK_TAB, VK_BACKTAB, VK_NEWLINE.
        4 => 3,
        5 => 8,
        6 | 7 => 9,
        8 | 0x1e => 0xd,
        // VK_SHIFT, VK_CTRL, VK_ALT, VK_ALTGRAF, VK_PAUSE, VK_CAPSLOCK,
        // VK_ESC, VK_SPACE.
        9 => 0x10,
        0xa => 0x11,
        0xb | 0xc => 0x12,
        0xd => 0x13,
        0xe => 0x14,
        0xf => 0x1b,
        0x10 => 0x20,
        // VK_PAGEUP to VK_DOWN, in Windows' order.
        0x11..=0x18 => vk + 0x10,
        // VK_PRINTSCRN, VK_INSERT, VK_DELETE, VK_SCRLLOCK, VK_NUMLOCK,
        // VK_SYSRQ.
        0x19 | 0x1f => 0x2c,
        0x1a => 0x2d,
        0x1b => 0x2e,
        0x1c => 0x91,
        0x1d => 0x90,
        // VK_F1 to VK_F24.
        0x20..=0x37 => vk + 0x50,
        _ => return None,
    })
}

/// A Presentation Manager accelerator table as a Win32 `RT_ACCELERATOR`:
/// a count, the code page, then `{fs, key, cmd}`, `fs` with `AF_CHAR` 1,
/// `AF_VIRTUALKEY` 2, `AF_SCANCODE` 4, `AF_SHIFT` 8, `AF_CONTROL` 0x10,
/// `AF_ALT` 0x20. Characters become virtual keys (upper case), as the
/// Win95 table has them; scan codes are left out.
#[must_use]
pub fn accelerators(d: &[u8]) -> Option<Vec<u8>> {
    let n = usize::from(u16_at(d, 0)?);
    let mut out = Vec::new();
    for i in 0..n {
        let at = 4 + 6 * i;
        let fs = u16_at(d, at)?;
        let key = u16_at(d, at + 2)?;
        let key = if fs & 2 != 0 {
            vk(key)
        } else if fs & 1 != 0 {
            u8::try_from(key).ok().map(|c| u16::from(c.to_ascii_uppercase()))
        } else {
            None
        };
        let Some(key) = key else { continue };
        // FVIRTKEY, FSHIFT, FCONTROL, FALT.
        let flags = 1 | (fs & 0x38) >> 1;
        out.extend(flags.to_le_bytes());
        out.extend(key.to_le_bytes());
        out.extend(u16_at(d, at + 4)?.to_le_bytes());
        out.extend([0, 0]);
    }
    // FLAST.
    let last = out.len().checked_sub(8)?;
    out[last] |= 0x80;
    Some(out)
}

/// A Presentation Manager string table block (the code page, then 16
/// strings: a length counting the NUL, the bytes, the NUL) as a Win32
/// one (16 lengths and UTF-16 strings), numbered alike (`id / 16 + 1`).
#[must_use]
pub fn strings(d: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut at = 2;
    for _ in 0..16 {
        let n = usize::from(d.get(at).copied().unwrap_or(0));
        let s = d.get(at + 1..at + 1 + n).unwrap_or_default();
        let s = s.split(|&c| c == 0).next().unwrap_or_default();
        let s: Vec<u16> = s.iter().map(|&c| u16::from(ansi(c))).collect();
        out.extend(u16::try_from(s.len()).ok()?.to_le_bytes());
        for u in s {
            out.extend(u.to_le_bytes());
        }
        at += 1 + n;
    }
    Some(out)
}

/// A Presentation Manager bitmap header at `at`: the 14-byte file header
/// (type, size, hot spot from the bottom left, the bits' offset), then
/// `BITMAPINFOHEADER` (12 bytes, 16-bit sizes, 3-byte colours) or
/// `BITMAPINFOHEADER2` (32-bit sizes, 4-byte colours).
struct Bmp {
    hot: (u16, u16),
    bits: usize,
    w: usize,
    h: usize,
    bpp: usize,
    /// The colour table's offset and the size of an entry.
    colors: usize,
    entry: usize,
    /// Just past the colour table.
    end: usize,
}

fn bmp(d: &[u8], at: usize) -> Option<Bmp> {
    let fix = u32_at(d, at + 14)? as usize;
    let i = at + 14;
    let (w, h, bpp, entry) = if fix == 12 {
        (
            usize::from(u16_at(d, i + 4)?),
            usize::from(u16_at(d, i + 6)?),
            usize::from(u16_at(d, i + 10)?),
            3,
        )
    } else {
        if u32_at(d, i + 16).unwrap_or(0) != 0 {
            return None;
        }
        (
            u32_at(d, i + 4)? as usize,
            u32_at(d, i + 8)? as usize,
            usize::from(u16_at(d, i + 14)?),
            4,
        )
    };
    if !matches!(bpp, 1 | 4 | 8) || w == 0 || w > 256 || h == 0 || h > 512 {
        return None;
    }
    let colors = i + fix;
    Some(Bmp {
        hot: (u16_at(d, at + 6)?, u16_at(d, at + 8)?),
        bits: u32_at(d, at + 10)? as usize,
        w,
        h,
        bpp,
        colors,
        entry,
        end: colors + entry * (1 << bpp),
    })
}

/// A Presentation Manager pointer (`RT_POINTER`) as a Win32 `RT_CURSOR`
/// ([`crate::cursor::Cursor::parse`]) and its group entry's width,
/// height and bits a pixel. A pointer is a monochrome mask twice as high
/// as the pointer (`PT`: the XOR half, then the AND half, bottom up),
/// followed for a colour one (`CP`) by the colour image, which takes the
/// XOR half's place; a bitmap array (`BA`) holds them, the first is used.
/// The hot spot counts from the bottom.
#[must_use]
pub fn pointer(d: &[u8]) -> Option<(Vec<u8>, u16, u16, u16)> {
    let at = if d.get(..2)? == b"BA" { 14 } else { 0 };
    let kind = d.get(at..at + 2)?;
    if kind != b"CP" && kind != b"PT" {
        return None;
    }
    let mask = bmp(d, at)?;
    if mask.bpp != 1 {
        return None;
    }
    let h = mask.h / 2;
    let mstride = mask.w.div_ceil(32) * 4;
    let and = d.get(mask.bits + h * mstride..mask.bits + 2 * h * mstride)?;
    let (img, xor) = if kind == b"CP" {
        let c = bmp(d, mask.end)?;
        let stride = (c.w * c.bpp).div_ceil(32) * 4;
        let xor = d.get(c.bits..c.bits + h * stride)?;
        (c, xor)
    } else {
        let xor = d.get(mask.bits..mask.bits + h * mstride)?;
        (bmp(d, at)?, xor)
    };
    let mut out = Vec::new();
    out.extend(mask.hot.0.to_le_bytes());
    out.extend(
        u16::try_from(h.checked_sub(1 + usize::from(mask.hot.1))?)
            .ok()?
            .to_le_bytes(),
    );
    // BITMAPINFOHEADER: 40 bytes, the size, one plane, the depth.
    out.extend(40u32.to_le_bytes());
    out.extend(u32::try_from(mask.w).ok()?.to_le_bytes());
    out.extend(u32::try_from(2 * h).ok()?.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(u16::try_from(img.bpp).ok()?.to_le_bytes());
    out.extend([0; 24]);
    for k in 0..1 << img.bpp {
        let c = img.colors + k * img.entry;
        out.extend(d.get(c..c + 3)?);
        out.push(0);
    }
    out.extend(xor);
    out.extend(and);
    let w = u16::try_from(mask.w).ok()?;
    Some((out, w, u16::try_from(2 * h).ok()?, u16::try_from(img.bpp).ok()?))
}

/// A combo box's strings (type 1022, [`crate::tables::strings`]) in
/// Windows-1252: OS/2's are in code page 437 (the Converter's degree
/// signs).
#[must_use]
pub fn combo(d: &[u8]) -> Vec<u8> {
    let mut out = d.to_vec();
    let mut at = 0;
    while u16_at(d, at).is_some_and(|id| id != 0) {
        let Some(len) = u32_at(d, at + 4) else { break };
        let end = (at + 8 + len as usize).min(d.len());
        for c in out.get_mut(at + 8..end).unwrap_or_default() {
            *c = ansi(*c);
        }
        at = end;
    }
    out
}

/// The OS/2 pointers the editor uses and the Win95 build's cursor groups
/// (`0x439b88`, 5007 to 5013: glove, scissors, plug on a knot, grabbing
/// hand, open hand, plug on a wire, pointing hand). OS/2 has them in
/// another order and drawn anew, the plugs twice as Win95, and a pointing
/// hand of its own where Win95 shows the grabbing one again.
pub const POINTERS: [(u32, u32); 7] = [
    (4135, 5007),
    (4140, 5008),
    (4137, 5009),
    (4136, 5010),
    (4139, 5011),
    (4138, 5012),
    (4141, 5013),
];

/// The OS/2 dialogs numbered differently: OS/2's, Win95's.
pub const DIALOG_IDS: [(u32, u32); 1] = [(260, 10000)];

/// The OS/2 menu bar's id and the Win95 one's.
pub const MENU_BAR: (u32, u32) = (1, 3);

/// The main window's accelerators, numbered as the Win95 build's (OS/2's
/// table 1 holds the frame's own keys: Esc, F1, F3, Alt+X, Ctrl+X).
pub const ACCELERATORS: u32 = 2;

/// The resources of an OS/2 module as the Win95 build's would be: the
/// game's own types as they are, Presentation Manager's converted and
/// renumbered (see the module's documentation). Resources that don't
/// convert are left out.
#[must_use]
pub fn to_win(r: &lx::Resources) -> pe::Resources {
    let mut out = pe::Resources::default();
    for (&(typ, id), d) in &r.entries {
        let conv = match typ {
            lx::RT_DIALOG => {
                let id = DIALOG_IDS.iter().find(|(o, _)| *o == id).map_or(id, |(_, w)| *w);
                dialog(d).map(|b| (pe::RT_DIALOG, id, b))
            }
            lx::RT_MENU => {
                let id = if id == MENU_BAR.0 { MENU_BAR.1 } else { id };
                menu(d).map(|b| (pe::RT_MENU, id, b))
            }
            lx::RT_STRING => strings(d).map(|b| (pe::RT_STRING, id, b)),
            lx::RT_ACCELTABLE if id == ACCELERATORS => accelerators(d).map(|b| (pe::RT_ACCELERATOR, id, b)),
            lx::RT_POINTER => {
                let Some(&(_, group_id)) = POINTERS.iter().find(|(o, _)| *o == id) else {
                    continue;
                };
                let Some((cursor, w, h, bpp)) = pointer(d) else {
                    continue;
                };
                let mut group = vec![0, 0, 2, 0, 1, 0];
                for v in [w, h, 1, bpp] {
                    group.extend(v.to_le_bytes());
                }
                group.extend(u32::try_from(cursor.len()).unwrap_or(0).to_le_bytes());
                group.extend(u16::try_from(id).unwrap_or(0).to_le_bytes());
                out.entries.insert((pe::RT_CURSOR, id), cursor);
                Some((pe::RT_GROUP_CURSOR, group_id, group))
            }
            1022 => Some((typ, id, combo(d))),
            t if t >= 1000 => Some((typ, id, d.clone())),
            _ => None,
        };
        if let Some((t, i, b)) = conv {
            out.entries.insert((t, i), b);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialog::{Class, Dialog, MenuItem, parse_menu};

    /// An OS/2 dialog: a 100×80 frame titled "T" at (10, 20) with a push
    /// button (`BS_DEFAULT`) at (5, 6) 30×14 "~Ok" and an entry field with
    /// `ES_MARGIN` at (40, 50) 20×8.
    fn os2_dialog() -> Vec<u8> {
        let mut d = vec![0u8; 14];
        d[6] = 14;
        let strings_at = 14 + 3 * 30;
        let mut strings = Vec::new();
        let mut item = |d: &mut Vec<u8>, children: u16, class: u16, text: &[u8], style: u32, r: [i16; 4], id: u16| {
            let off = strings_at + strings.len();
            strings.extend_from_slice(text);
            strings.push(0);
            d.extend([0, 0]);
            d.extend(children.to_le_bytes());
            d.extend([0, 0]);
            d.extend(class.to_le_bytes());
            d.extend(u16::try_from(text.len() + 1).unwrap().to_le_bytes());
            d.extend(u16::try_from(off).unwrap().to_le_bytes());
            d.extend(style.to_le_bytes());
            for v in r {
                d.extend(v.to_le_bytes());
            }
            d.extend(id.to_le_bytes());
            d.extend([0xff; 4]);
        };
        item(&mut d, 2, 1, b"T", 0x8000_0000, [10, 20, 100, 80], 500);
        item(&mut d, 0, 3, b"~Ok", 0x8002_0400, [5, 6, 30, 14], 1);
        item(&mut d, 0, 6, b"5#", 0x8000_0008, [40, 50, 20, 8], 115);
        d.extend(strings);
        d
    }

    #[test]
    fn converts_dialogs() {
        let w = Dialog::parse(&dialog(&os2_dialog()).unwrap()).unwrap();
        assert_eq!(w.title, "T");
        assert_eq!((w.x, w.y, w.cx, w.cy), (10, PARENT_H - 20 - 80, 100, 80));
        assert_eq!(w.font, Some((8, "MS Sans Serif".into())));
        let ok = w.item(1).unwrap();
        assert_eq!(ok.class(), Class::Button);
        assert_eq!(ok.style & 0xf, 1);
        assert!(ok.visible() && !ok.disabled());
        assert_eq!(ok.title.text(), "&Ok");
        assert_eq!((ok.x, ok.y, ok.cx, ok.cy), (5, 80 - 6 - 14, 30, 14));
        let e = w.item(115).unwrap();
        assert_eq!(e.class(), Class::Edit);
        assert_eq!(e.title.text(), "5#");
        assert_eq!((e.x, e.y, e.cx, e.cy), (38, 80 - 48 - 12, 24, 12));
    }

    #[test]
    fn converts_menus() {
        let item = |d: &mut Vec<u8>, style: u16, attr: u16, id: u16, text: &[u8]| {
            for v in [style, attr, id] {
                d.extend(v.to_le_bytes());
            }
            if style & 1 != 0 {
                d.extend_from_slice(text);
                d.push(0);
            }
        };
        let header = |d: &mut Vec<u8>, n: u16| {
            d.extend([0, 0, 0, 0, 0xb5, 1, 4, 0]);
            d.extend(n.to_le_bytes());
        };
        let mut sub = Vec::new();
        header(&mut sub, 3);
        item(&mut sub, 1, 0, 0x8000, b"~New...");
        item(&mut sub, 4, 0x4000, 0xffff, b"");
        item(&mut sub, 1, 0x2000, 0x800f, b"Plain");
        let n = u32::try_from(sub.len()).unwrap();
        sub[..4].copy_from_slice(&n.to_le_bytes());
        let mut d = Vec::new();
        header(&mut d, 1);
        item(&mut d, 0x11, 0, 0xf000, b"~File");
        d.extend(sub);
        let n = u32::try_from(d.len()).unwrap();
        d[..4].copy_from_slice(&n.to_le_bytes());
        let m = parse_menu(&menu(&d).unwrap()).unwrap();
        assert_eq!(
            m,
            [MenuItem::Popup {
                text: "&File".into(),
                flags: 0x90,
                items: vec![
                    MenuItem::Command {
                        id: 0xe100,
                        text: "&New...".into(),
                        flags: 0
                    },
                    MenuItem::Separator,
                    MenuItem::Command {
                        id: 0x800c,
                        text: "Plain".into(),
                        flags: 0x88
                    },
                ],
            }]
        );
    }

    #[test]
    fn converts_accelerators_and_strings() {
        // Ctrl+N (a character), Alt+Backspace, Shift+F6.
        let mut d = vec![3, 0, 0xb5, 1];
        for (fs, key, cmd) in [
            (0x11u16, u16::from(b'n'), 0xe100u16),
            (0x22, 5, 0xe12b),
            (0x0a, 0x25, 0xe151),
        ] {
            for v in [fs, key, cmd] {
                d.extend(v.to_le_bytes());
            }
        }
        let w = accelerators(&d).unwrap();
        assert_eq!(
            w,
            [
                9, 0, b'N', 0, 0x00, 0xe1, 0, 0, 0x11, 0, 8, 0, 0x2b, 0xe1, 0, 0, 0x85, 0, 0x75, 0, 0x51, 0xe1, 0, 0
            ]
        );
        // Block 501: 8000 empty, 8001 "Hi", the rest missing.
        let s = strings(&[0xb5, 1, 1, 0, 3, b'H', b'i', 0]).unwrap();
        let mut r = pe::Resources::default();
        r.entries.insert((pe::RT_STRING, 501), s);
        assert_eq!(r.string(8000).as_deref(), Some(""));
        assert_eq!(r.string(8001).as_deref(), Some("Hi"));
        assert_eq!(r.string(8015).as_deref(), Some(""));
        // A combo list's "°F" (code page 437's 0xf8), its id 0xf8 kept.
        let d = [0xf8, 0, 3, 4, 3, 0, 0, 0, 0xf8, b'F', 0, 0, 0];
        assert_eq!(crate::tables::strings(&combo(&d)), [(0xf8, "\u{b0}F".to_owned())]);
    }

    #[test]
    fn converts_pointers() {
        // A 2×2 colour pointer, hot spot (1, 1) from the bottom: the mask
        // (XOR rows, then AND rows: the top right transparent), then 4 bpp.
        let mut d = Vec::new();
        let head = |d: &mut Vec<u8>, kind: &[u8], bits: u32, h: u16, bpp: u16, colors: &[[u8; 3]]| {
            d.extend_from_slice(kind);
            d.extend(26u32.to_le_bytes());
            d.extend([1, 0, 1, 0]);
            d.extend(bits.to_le_bytes());
            d.extend(12u32.to_le_bytes());
            for v in [2, h, 1, bpp] {
                d.extend(v.to_le_bytes());
            }
            for c in colors {
                d.extend(c);
            }
        };
        let mut pal = vec![[0u8; 3]; 16];
        pal[1] = [0, 0, 0xff];
        let bits = 2 * 26 + 6 + 48;
        head(&mut d, b"CP", bits, 4, 1, &[[0; 3], [0xff; 3]]);
        head(&mut d, b"CP", bits + 16, 2, 4, &pal);
        d.extend([0; 8]);
        d.extend([0, 0, 0, 0, 0x40, 0, 0, 0]);
        // Colour rows, bottom up: index 1 then 0; then 0, 0.
        d.extend([0x10, 0, 0, 0, 0, 0, 0, 0]);
        let (c, w, h, bpp) = pointer(&d).unwrap();
        assert_eq!((w, h, bpp), (2, 4, 4));
        let c = crate::cursor::Cursor::parse(&c).unwrap();
        assert_eq!(c.hot, (1, 0));
        assert_eq!(&c.rgba[..8], [0, 0, 0, 255, 0, 0, 0, 0]);
        assert_eq!(&c.rgba[8..], [255, 0, 0, 255, 0, 0, 0, 255]);
    }
}
