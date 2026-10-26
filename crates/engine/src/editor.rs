//! The editor: the main window with the worksheet and its widget
//! (`CWidgetView`), where gadgets are placed and wired and the widget run.
//! This is the first cut: the worksheet shows a widget, the space bar (or
//! Ctrl+R) starts and stops it and a click on a running gadget is its
//! click. The trays, tools, menus and dialogs come from
//! `docs/editor-layout.md` and `docs/editor-interaction.md`.

use crate::assets::Assets;
use crate::doc::Document;
use crate::gadget::DrawCx;
use crate::gfx::{IRect, Pictures, Surface};
use crate::sim::{Env, SoundCmd};

/// Mac ticks per second: the original's clock (`TickCount`).
pub const TICKS_PER_SECOND: u64 = 60;

/// The worksheet's background colour (a palette index).
const WORKSHEET: u8 = 0xff;
/// Wires' colour.
const WIRE: u8 = 0;

const VK_SPACE: u16 = 0x20;
const VK_CONTROL: u16 = 0x11;
const VK_R: u16 = b'R' as u16;

/// Mouse buttons the editor tells apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
}

/// The editor, drawn into an 8-bit surface.
pub struct Editor {
    pub assets: Assets,
    pub doc: Document,
    /// The file the widget came from or was saved to.
    pub path: Option<String>,
    pictures: Pictures,
    pub surface: Surface,
    pub running: bool,
    pub env: Env,
    /// The worksheet's scroll position: the worksheet point at the top
    /// left of its view.
    pub scroll: (i32, i32),
    /// Milliseconds not yet made into ticks.
    ms_rest: u64,
    ctrl: bool,
    finished: bool,
}

impl Editor {
    #[must_use]
    pub fn new(assets: Assets, doc: Document, width: u32, height: u32) -> Editor {
        Editor {
            assets,
            doc,
            path: None,
            pictures: Pictures::default(),
            surface: Surface::new(width, height),
            running: false,
            env: Env::default(),
            scroll: (0, 0),
            ms_rest: 0,
            ctrl: false,
            finished: false,
        }
    }

    /// The desktop is now `width` × `height`.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface.resize(width, height);
    }

    #[must_use]
    pub fn finished(&self) -> bool {
        self.finished
    }

    pub fn quit(&mut self) {
        self.stop();
        self.finished = true;
    }

    /// The worksheet's view on the surface.
    #[must_use]
    pub fn worksheet(&self) -> IRect {
        self.surface.bounds()
    }

    /// Starts the widget (Run).
    pub fn run(&mut self) {
        if self.running {
            return;
        }
        self.running = true;
        self.env.ticks = 0;
        self.env.ms = 0;
        self.ms_rest = 0;
        self.doc.start(&mut self.env);
    }

    /// Stops the widget (back to editing).
    pub fn stop(&mut self) {
        if !self.running {
            return;
        }
        self.running = false;
        self.doc.stop();
        self.env.sounds.push(SoundCmd::StopAll);
    }

    /// Advances the clock by `ms`, ticking the running widget once per Mac
    /// tick (the original ticks it from `OnIdle`, as often as it can; once
    /// a tick is what its gadgets' timing assumes).
    pub fn update(&mut self, ms: u64, local_time: (u32, u32, u32)) {
        self.env.local_time = local_time;
        if !self.running {
            return;
        }
        self.ms_rest += ms * TICKS_PER_SECOND;
        // At most a few ticks to catch up after a stall.
        let mut n = 0;
        while self.ms_rest >= 1000 && n < 8 {
            self.ms_rest -= 1000;
            self.doc.tick(&mut self.env);
            self.env.keys.clear();
            self.env.ticks = self.env.ticks.wrapping_add(1);
            self.env.ms += 1000 / TICKS_PER_SECOND;
            n += 1;
        }
        if n == 8 {
            self.ms_rest = 0;
        }
    }

    /// The sounds asked for since the last call.
    pub fn take_sounds(&mut self) -> Vec<SoundCmd> {
        std::mem::take(&mut self.env.sounds)
    }

    fn to_sheet(&self, x: i32, y: i32) -> (i32, i32) {
        let w = self.worksheet();
        (x - w.x0 + self.scroll.0, y - w.y0 + self.scroll.1)
    }

    pub fn mouse_move(&mut self, x: i32, y: i32) {
        self.env.mouse = self.to_sheet(x, y);
    }

    pub fn mouse_down(&mut self, x: i32, y: i32, button: Button) {
        let (h, v) = self.to_sheet(x, y);
        self.env.mouse = (h, v);
        if button != Button::Left {
            return;
        }
        self.env.mouse_down = true;
        if self.running
            && let Some(i) = self.doc.gadget_at(h, v)
        {
            let (gh, gv) = (self.doc.gadgets[i].core.h, self.doc.gadgets[i].core.v);
            self.doc.with_gadget(i, &mut self.env, |g, io| {
                g.behavior.click(&mut g.core, io, h - gh, v - gv);
            });
        }
    }

    pub fn mouse_up(&mut self, x: i32, y: i32, button: Button) {
        self.env.mouse = self.to_sheet(x, y);
        if button == Button::Left {
            self.env.mouse_down = false;
        }
    }

    /// The wheel scrolls the worksheet (a port extension).
    pub fn wheel(&mut self, dx: i32, dy: i32) {
        self.scroll.0 = (self.scroll.0 + dx * 32).max(0);
        self.scroll.1 = (self.scroll.1 - dy * 32).max(0);
    }

    pub fn key_down(&mut self, vk: u16) {
        match vk {
            VK_CONTROL => self.ctrl = true,
            VK_SPACE => self.toggle_run(),
            VK_R if self.ctrl => self.toggle_run(),
            _ if self.running => self.env.keys.push(vk),
            _ => {}
        }
    }

    pub fn key_up(&mut self, vk: u16) {
        if vk == VK_CONTROL {
            self.ctrl = false;
        }
    }

    fn toggle_run(&mut self) {
        if self.running {
            self.stop();
        } else {
            self.run();
        }
    }

    /// Draws everything.
    pub fn draw(&mut self) {
        let view = self.worksheet();
        let s = &mut self.surface;
        s.fill(view, WORKSHEET);
        let old = s.set_clip(view);
        s.origin = (view.x0 - self.scroll.0, view.y0 - self.scroll.1);
        let mut cx = DrawCx {
            surface: s,
            assets: &self.assets,
            pictures: &self.pictures,
            running: self.running,
            ticks: self.env.ticks,
        };
        self.doc.draw_gadgets(&mut cx);
        for wire in &self.doc.wires {
            let ends = (
                self.doc.gadgets[wire.from].core.knot_at(wire.from_knot),
                self.doc.gadgets[wire.to].core.knot_at(wire.to_knot),
            );
            let (Some(a), Some(b)) = ends else { continue };
            let mut at = a;
            for &p in wire.points.iter().chain(std::iter::once(&b)) {
                cx.surface.line(at, p, WIRE);
                at = p;
            }
        }
        let s = &mut self.surface;
        s.origin = (0, 0);
        s.set_clip(old);
    }
}
