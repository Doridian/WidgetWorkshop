//! Win32 dialog and menu templates (`RT_DIALOG`, `RT_MENU`) as the
//! resource compiler wrote them into `WIDGET.EXE`. Every gadget has a
//! settings dialog numbered like the gadget (1000 to 1019, 2000 to 2046);
//! the game fills and reads its controls by their ids (`cDialog`,
//! `0x4266xx`).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    OutOfBounds(usize),
    /// `DLGTEMPLATEEX`, which the game doesn't use.
    Extended,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::OutOfBounds(at) => write!(f, "data at {at:#x} lies outside the template"),
            Error::Extended => f.write_str("extended dialog templates aren't supported"),
        }
    }
}

impl std::error::Error for Error {}

/// A class, menu or title field: absent, an ordinal, or a string.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Field {
    #[default]
    None,
    Ordinal(u16),
    Text(String),
}

impl Field {
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Field::Text(s) => s,
            _ => "",
        }
    }
}

/// The predefined control classes' ordinals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Button,
    Edit,
    Static,
    ListBox,
    ScrollBar,
    ComboBox,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub style: u32,
    pub ex_style: u32,
    /// In dialog units.
    pub x: i16,
    pub y: i16,
    pub cx: i16,
    pub cy: i16,
    pub id: u16,
    pub class: Field,
    pub title: Field,
}

impl Item {
    #[must_use]
    pub fn class(&self) -> Class {
        match &self.class {
            Field::Ordinal(0x80) => Class::Button,
            Field::Ordinal(0x81) => Class::Edit,
            Field::Ordinal(0x82) => Class::Static,
            Field::Ordinal(0x83) => Class::ListBox,
            Field::Ordinal(0x84) => Class::ScrollBar,
            Field::Ordinal(0x85) => Class::ComboBox,
            Field::Text(t) => match t.to_ascii_lowercase().as_str() {
                "button" => Class::Button,
                "edit" => Class::Edit,
                "static" => Class::Static,
                "listbox" => Class::ListBox,
                "scrollbar" => Class::ScrollBar,
                "combobox" => Class::ComboBox,
                _ => Class::Other,
            },
            _ => Class::Other,
        }
    }

    /// `WS_VISIBLE`.
    #[must_use]
    pub fn visible(&self) -> bool {
        self.style & 0x1000_0000 != 0
    }

    /// `WS_DISABLED`.
    #[must_use]
    pub fn disabled(&self) -> bool {
        self.style & 0x0800_0000 != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dialog {
    pub style: u32,
    pub ex_style: u32,
    pub x: i16,
    pub y: i16,
    pub cx: i16,
    pub cy: i16,
    pub menu: Field,
    pub class: Field,
    pub title: String,
    /// With `DS_SETFONT`: the point size and the face.
    pub font: Option<(u16, String)>,
    pub items: Vec<Item>,
}

struct Reader<'a> {
    d: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn u16(&mut self) -> Result<u16, Error> {
        let v = self
            .d
            .get(self.at..self.at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .ok_or(Error::OutOfBounds(self.at))?;
        self.at += 2;
        Ok(v)
    }

    #[allow(clippy::cast_possible_wrap)]
    fn i16(&mut self) -> Result<i16, Error> {
        self.u16().map(|v| v as i16)
    }

    fn u32(&mut self) -> Result<u32, Error> {
        let lo = u32::from(self.u16()?);
        Ok(lo | u32::from(self.u16()?) << 16)
    }

    fn string(&mut self) -> Result<String, Error> {
        let mut units = Vec::new();
        loop {
            let u = self.u16()?;
            if u == 0 {
                break;
            }
            units.push(u);
        }
        Ok(String::from_utf16_lossy(&units))
    }

    fn field(&mut self) -> Result<Field, Error> {
        match self.u16()? {
            0 => Ok(Field::None),
            0xffff => Ok(Field::Ordinal(self.u16()?)),
            _ => {
                self.at -= 2;
                Ok(Field::Text(self.string()?))
            }
        }
    }

    fn align4(&mut self) {
        self.at = (self.at + 3) & !3;
    }
}

impl Dialog {
    /// Reads a `DLGTEMPLATE` and its items.
    ///
    /// # Errors
    /// If it's cut short or extended.
    pub fn parse(d: &[u8]) -> Result<Dialog, Error> {
        let mut r = Reader { d, at: 0 };
        let style = r.u32()?;
        if style == 0xffff_0001 {
            return Err(Error::Extended);
        }
        let ex_style = r.u32()?;
        let count = r.u16()?;
        let (x, y, cx, cy) = (r.i16()?, r.i16()?, r.i16()?, r.i16()?);
        let menu = r.field()?;
        let class = r.field()?;
        let title = r.string()?;
        // DS_SETFONT
        let font = if style & 0x40 != 0 {
            let size = r.u16()?;
            Some((size, r.string()?))
        } else {
            None
        };
        let mut items = Vec::with_capacity(usize::from(count));
        for _ in 0..count {
            r.align4();
            let style = r.u32()?;
            let ex_style = r.u32()?;
            let (x, y, cx, cy) = (r.i16()?, r.i16()?, r.i16()?, r.i16()?);
            let id = r.u16()?;
            let class = r.field()?;
            let title = r.field()?;
            let extra = usize::from(r.u16()?);
            r.at += extra;
            items.push(Item {
                style,
                ex_style,
                x,
                y,
                cx,
                cy,
                id,
                class,
                title,
            });
        }
        Ok(Dialog {
            style,
            ex_style,
            x,
            y,
            cx,
            cy,
            menu,
            class,
            title,
            font,
            items,
        })
    }

    #[must_use]
    pub fn item(&self, id: u16) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }
}

/// A menu item: a command with its id, a separator, or a popup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuItem {
    Command { id: u16, text: String, flags: u16 },
    Separator,
    Popup { text: String, flags: u16, items: Vec<MenuItem> },
}

/// `MF_POPUP`, `MF_END`, `MF_GRAYED`, `MF_CHECKED`.
const MF_POPUP: u16 = 0x10;
const MF_END: u16 = 0x80;
pub const MF_GRAYED: u16 = 0x01;
pub const MF_CHECKED: u16 = 0x08;

/// Reads a standard `MENUITEMTEMPLATE` list (`RT_MENU`): the menu bar's
/// popups.
///
/// # Errors
/// If it's cut short.
pub fn parse_menu(d: &[u8]) -> Result<Vec<MenuItem>, Error> {
    let mut r = Reader { d, at: 4 };
    menu_items(&mut r)
}

fn menu_items(r: &mut Reader<'_>) -> Result<Vec<MenuItem>, Error> {
    let mut out = Vec::new();
    loop {
        let flags = r.u16()?;
        if flags & MF_POPUP != 0 {
            let text = r.string()?;
            let items = menu_items(r)?;
            out.push(MenuItem::Popup { text, flags, items });
        } else {
            let id = r.u16()?;
            let text = r.string()?;
            if id == 0 && text.is_empty() {
                out.push(MenuItem::Separator);
            } else {
                out.push(MenuItem::Command { id, text, flags });
            }
        }
        if flags & MF_END != 0 {
            return Ok(out);
        }
    }
}
