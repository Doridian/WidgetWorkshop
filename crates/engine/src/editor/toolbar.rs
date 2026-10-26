//! The floating toolbars (`CWTbar`, docs/editor-layout.md 3.1–3.3): a
//! child window of the worksheet drawn from a picture sheet, with a move
//! bar on top and bitmap buttons (`CWToolbarElement`). The main toolbar
//! (`CMainTbar`) holds GO, STOP, Light and Sound.

use crate::assets::{Assets, Module};
use crate::gfx::{IRect, Pictures, Surface};

/// The move bar's height (`+0x490`, ctor `0x42fa33`).
pub const MOVE_BAR: i32 = 15;

/// Element ids (commands) of the main toolbar and the tray.
pub mod id {
    pub const GO: i32 = 1501;
    pub const STOP: i32 = 1502;
    pub const LIGHT: i32 = 1503;
    pub const SOUND: i32 = 1504;
    pub const POINTER: i32 = 1505;
    pub const CUT: i32 = 1506;
    pub const MOVE: i32 = 1507;
    pub const TAB0: i32 = 1508;
    pub const PREV: i32 = 1515;
    pub const NEXT: i32 = 1516;
}

/// How an element reacts to a press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `CWBmpButton`: toggles on each press.
    Toggle,
    /// `CWBmpRadioButton` in group `n`.
    Radio(u8),
    /// `CWBmpPushButton`: down while held, fires on release inside.
    Push,
}

/// A bitmap button (`CWToolbarElement`, 0x70 bytes).
#[derive(Debug, Clone)]
pub struct Element {
    pub id: i32,
    /// In toolbar coordinates; also the normal image's place in the sheet.
    pub rect: IRect,
    pub kind: Kind,
    /// Pressed, on or selected (`+0x1c`).
    pub on: bool,
    pub enabled: bool,
    /// The on and disabled images' places in the sheet's frame 1.
    pub on_src: IRect,
    pub off_src: IRect,
}

impl Element {
    #[must_use]
    pub fn new(id: i32, rect: IRect, kind: Kind, on_src: IRect, off_src: Option<IRect>) -> Element {
        Element {
            id,
            rect,
            kind,
            on: false,
            enabled: true,
            on_src,
            off_src: off_src.unwrap_or(rect),
        }
    }

    /// The image's place in the sheet (`0x42dcb0`).
    #[must_use]
    pub fn source(&self) -> IRect {
        if !self.enabled {
            self.off_src
        } else if self.on {
            self.on_src
        } else {
            self.rect
        }
    }
}

/// A toolbar window: where it is in the worksheet, its sheet and buttons.
#[derive(Debug, Clone)]
pub struct Bar {
    /// Its top left in the worksheet's (the view's) coordinates.
    pub pos: (i32, i32),
    pub size: (i32, i32),
    /// Picture sheet (`WWGADGET` type 1004); frame 1 is its face.
    pub sheet: u32,
    pub elements: Vec<Element>,
    pub visible: bool,
    /// The push button holding the mouse (`+0x4c0`).
    pub capture: Option<usize>,
}

impl Bar {
    #[must_use]
    pub fn rect(&self) -> IRect {
        IRect::sized(self.pos.0, self.pos.1, self.size.0, self.size.1)
    }

    /// The element `id`.
    #[must_use]
    pub fn get(&self, id: i32) -> Option<&Element> {
        self.elements.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: i32) -> Option<&mut Element> {
        self.elements.iter_mut().find(|e| e.id == id)
    }

    /// Selects radio `id` in its group (`CGroupBox::Select`, `0x42ddd4`).
    pub fn select(&mut self, id: i32) {
        let Some(Kind::Radio(g)) = self.get(id).map(|e| e.kind) else {
            return;
        };
        for e in &mut self.elements {
            if e.kind == Kind::Radio(g) {
                e.on = e.id == id;
            }
        }
    }

    /// The selected radio of group `g`.
    #[must_use]
    pub fn selected(&self, g: u8) -> Option<i32> {
        self.elements
            .iter()
            .find(|e| e.kind == Kind::Radio(g) && e.on)
            .map(|e| e.id)
    }

    /// The first element at `p` (toolbar coordinates).
    #[must_use]
    pub fn element_at(&self, p: (i32, i32)) -> Option<usize> {
        self.elements.iter().position(|e| e.rect.contains(p.0, p.1))
    }

    /// Presses the elements at `p` (`0x42fccc`: every one that holds it).
    /// Returns the commands that fire now: `(id, on)`.
    pub fn press(&mut self, p: (i32, i32)) -> Vec<(i32, bool)> {
        let mut fired = Vec::new();
        for i in 0..self.elements.len() {
            let e = &self.elements[i];
            if !e.rect.contains(p.0, p.1) || !e.enabled {
                continue;
            }
            match e.kind {
                Kind::Toggle => {
                    let e = &mut self.elements[i];
                    e.on = !e.on;
                    fired.push((e.id, e.on));
                }
                Kind::Radio(_) => {
                    if !e.on {
                        let id = e.id;
                        self.select(id);
                        fired.push((id, true));
                    }
                }
                Kind::Push => {
                    self.capture = Some(i);
                    self.elements[i].on = true;
                }
            }
        }
        fired
    }

    /// The mouse is released at `p` (`0x4302f7`): the held push button
    /// fires if it's released inside (`0x42df32`).
    pub fn release(&mut self, p: (i32, i32)) -> Option<i32> {
        let i = self.capture.take()?;
        let e = &mut self.elements[i];
        e.on = false;
        e.rect.contains(p.0, p.1).then_some(e.id)
    }

    /// Draws its face and elements (`0x4304b5`) into `s`, whose origin is
    /// the worksheet's.
    pub fn draw(&self, s: &mut Surface, assets: &Assets, pictures: &Pictures) {
        let Some(img) = pictures.frame(assets, Module::Gadget, self.sheet, 1) else {
            s.fill(self.rect(), 7);
            return;
        };
        let (x, y) = self.pos;
        s.blit_part(&img, IRect::sized(0, 0, self.size.0, self.size.1), x, y);
        for e in &self.elements {
            s.blit_part(&img, e.source(), x + e.rect.x0, y + e.rect.y0);
        }
    }
}

/// The main toolbar (`CMainTbar`, `0x4305f2`): at (0, 0), 53×93, sheet 1,
/// its "on" art 53 pixels to the right. STOP and Light and Sound start on.
#[must_use]
pub fn main_toolbar() -> Bar {
    let on = |r: IRect| r.offset(53, 0);
    let mut elements = vec![
        Element::new(
            id::STOP,
            IRect::new(5, 40, 47, 59),
            Kind::Radio(0),
            on(IRect::new(5, 40, 47, 59)),
            None,
        ),
        Element::new(
            id::GO,
            IRect::new(5, 19, 47, 38),
            Kind::Radio(0),
            on(IRect::new(5, 19, 47, 38)),
            None,
        ),
        Element::new(
            id::LIGHT,
            IRect::new(5, 64, 25, 88),
            Kind::Toggle,
            on(IRect::new(5, 64, 25, 88)),
            None,
        ),
        Element::new(
            id::SOUND,
            IRect::new(27, 64, 47, 88),
            Kind::Toggle,
            on(IRect::new(27, 64, 47, 88)),
            None,
        ),
    ];
    for e in &mut elements {
        e.on = e.id != id::GO;
    }
    Bar {
        pos: (0, 0),
        size: (53, 93),
        sheet: 1,
        elements,
        visible: true,
        capture: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radios_toggles_and_push_buttons() {
        let mut b = main_toolbar();
        assert_eq!(b.selected(0), Some(id::STOP));
        assert_eq!(b.press((10, 25)), [(id::GO, true)]);
        assert_eq!(b.selected(0), Some(id::GO));
        // Pressing the selected radio again does nothing.
        assert_eq!(b.press((10, 25)), []);
        assert_eq!(b.press((10, 70)), [(id::LIGHT, false)]);
        assert_eq!(b.press((2, 2)), []);
        b.elements.push(Element::new(
            9,
            IRect::new(0, 0, 4, 4),
            Kind::Push,
            IRect::default(),
            None,
        ));
        assert_eq!(b.press((1, 1)), []);
        assert!(b.get(9).unwrap().on);
        assert_eq!(b.release((10, 10)), None);
        assert_eq!(b.press((1, 1)), []);
        assert_eq!(b.release((2, 2)), Some(9));
    }
}
