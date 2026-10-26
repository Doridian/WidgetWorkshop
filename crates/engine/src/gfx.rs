//! Drawing into an 8-bit surface in the game's palette, as the original
//! draws into its WinG/DIB offscreen buffer (`CWinGDib`): the picture
//! sheets' frames with their transparent runs, lines, filled rectangles and
//! bitmap-font text.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use formats::fnt::Font;
use formats::hr::{Image, Rect, Sheet};

use crate::assets::{Assets, Module, rt};

/// A rectangle in pixels: `x0..x1` × `y0..y1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct IRect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl IRect {
    #[must_use]
    pub const fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> IRect {
        IRect { x0, y0, x1, y1 }
    }

    #[must_use]
    pub const fn sized(x: i32, y: i32, w: i32, h: i32) -> IRect {
        IRect::new(x, y, x + w, y + h)
    }

    /// A Mac `Rect` moved by `dx`, `dy`.
    #[must_use]
    pub fn from_mac(r: Rect, dx: i32, dy: i32) -> IRect {
        IRect::new(
            i32::from(r.left) + dx,
            i32::from(r.top) + dy,
            i32::from(r.right) + dx,
            i32::from(r.bottom) + dy,
        )
    }

    #[must_use]
    pub fn width(self) -> i32 {
        self.x1 - self.x0
    }

    #[must_use]
    pub fn height(self) -> i32 {
        self.y1 - self.y0
    }

    #[must_use]
    pub fn is_empty(self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }

    #[must_use]
    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
    }

    #[must_use]
    pub fn intersect(self, o: IRect) -> IRect {
        IRect::new(self.x0.max(o.x0), self.y0.max(o.y0), self.x1.min(o.x1), self.y1.min(o.y1))
    }

    #[must_use]
    pub fn union(self, o: IRect) -> IRect {
        if self.is_empty() {
            return o;
        }
        if o.is_empty() {
            return self;
        }
        IRect::new(self.x0.min(o.x0), self.y0.min(o.y0), self.x1.max(o.x1), self.y1.max(o.y1))
    }

    #[must_use]
    pub fn offset(self, dx: i32, dy: i32) -> IRect {
        IRect::new(self.x0 + dx, self.y0 + dy, self.x1 + dx, self.y1 + dy)
    }

    #[must_use]
    pub fn inset(self, d: i32) -> IRect {
        IRect::new(self.x0 + d, self.y0 + d, self.x1 - d, self.y1 - d)
    }
}

/// An 8-bit picture to draw into, with a clip rectangle and an origin
/// that drawing positions are relative to.
pub struct Surface {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u8>,
    clip: IRect,
    /// Added to every position drawn at.
    pub origin: (i32, i32),
}

impl Surface {
    #[must_use]
    pub fn new(width: u32, height: u32) -> Surface {
        let (w, h) = (i32::try_from(width).unwrap_or(0), i32::try_from(height).unwrap_or(0));
        Surface {
            width: w,
            height: h,
            pixels: vec![0; (width * height) as usize],
            clip: IRect::new(0, 0, w, h),
            origin: (0, 0),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        *self = Surface::new(width, height);
    }

    #[must_use]
    pub fn bounds(&self) -> IRect {
        IRect::new(0, 0, self.width, self.height)
    }

    /// The clip rectangle, in surface pixels.
    #[must_use]
    pub fn clip(&self) -> IRect {
        self.clip
    }

    /// Limits drawing to `r` (in surface pixels) within the surface; returns
    /// the previous clip.
    pub fn set_clip(&mut self, r: IRect) -> IRect {
        let b = self.bounds();
        std::mem::replace(&mut self.clip, r.intersect(b))
    }

    fn put(&mut self, x: i32, y: i32, c: u8) {
        let (x, y) = (x + self.origin.0, y + self.origin.1);
        if self.clip.contains(x, y) {
            #[allow(clippy::cast_sign_loss)]
            let i = (y * self.width + x) as usize;
            self.pixels[i] = c;
        }
    }

    #[must_use]
    pub fn get(&self, x: i32, y: i32) -> Option<u8> {
        let (x, y) = (x + self.origin.0, y + self.origin.1);
        #[allow(clippy::cast_sign_loss)]
        self.bounds().contains(x, y).then(|| self.pixels[(y * self.width + x) as usize])
    }

    pub fn fill(&mut self, r: IRect, c: u8) {
        let r = r.offset(self.origin.0, self.origin.1).intersect(self.clip);
        if r.is_empty() {
            return;
        }
        for y in r.y0..r.y1 {
            #[allow(clippy::cast_sign_loss)]
            let row = (y * self.width) as usize;
            #[allow(clippy::cast_sign_loss)]
            self.pixels[row + r.x0 as usize..row + r.x1 as usize].fill(c);
        }
    }

    /// A one-pixel outline just inside `r`.
    pub fn frame(&mut self, r: IRect, c: u8) {
        if r.is_empty() {
            return;
        }
        self.fill(IRect::new(r.x0, r.y0, r.x1, r.y0 + 1), c);
        self.fill(IRect::new(r.x0, r.y1 - 1, r.x1, r.y1), c);
        self.fill(IRect::new(r.x0, r.y0, r.x0 + 1, r.y1), c);
        self.fill(IRect::new(r.x1 - 1, r.y0, r.x1, r.y1), c);
    }

    /// A line from `a` to `b`, both ends drawn (Bresenham).
    pub fn line(&mut self, a: (i32, i32), b: (i32, i32), c: u8) {
        let (mut x, mut y) = a;
        let dx = (b.0 - x).abs();
        let dy = -(b.1 - y).abs();
        let sx = if x < b.0 { 1 } else { -1 };
        let sy = if y < b.1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.put(x, y, c);
            if (x, y) == b {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// `img` with its top left at `x`, `y`; transparent pixels are skipped.
    pub fn blit(&mut self, img: &Image, x: i32, y: i32) {
        let dst = IRect::sized(x, y, i32::from(img.width), i32::from(img.height))
            .offset(self.origin.0, self.origin.1)
            .intersect(self.clip);
        if dst.is_empty() {
            return;
        }
        let w = usize::from(img.width);
        let (ox, oy) = (x + self.origin.0, y + self.origin.1);
        for sy in dst.y0..dst.y1 {
            #[allow(clippy::cast_sign_loss)]
            let src_row = (sy - oy) as usize * w;
            #[allow(clippy::cast_sign_loss)]
            let dst_row = (sy * self.width) as usize;
            for sx in dst.x0..dst.x1 {
                #[allow(clippy::cast_sign_loss)]
                let i = src_row + (sx - ox) as usize;
                if img.opaque[i] {
                    #[allow(clippy::cast_sign_loss)]
                    let d = dst_row + sx as usize;
                    self.pixels[d] = img.pixels[i];
                }
            }
        }
    }

    /// `img` stretched or shrunk to fill `r`, nearest neighbour.
    pub fn blit_scaled(&mut self, img: &Image, r: IRect) {
        if r.is_empty() || img.width == 0 || img.height == 0 {
            return;
        }
        let (w, h) = (i64::from(img.width), i64::from(img.height));
        for y in r.y0..r.y1 {
            let sy = (i64::from(y - r.y0) * h / i64::from(r.height())).clamp(0, h - 1);
            for x in r.x0..r.x1 {
                let sx = (i64::from(x - r.x0) * w / i64::from(r.width())).clamp(0, w - 1);
                #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                let i = (sy * w + sx) as usize;
                if img.opaque[i] {
                    self.put(x, y, img.pixels[i]);
                }
            }
        }
    }

    /// `text` (Windows-1252) in `font` with its top left at `x`, `y`;
    /// returns its width.
    pub fn text(&mut self, font: &Font, x: i32, y: i32, text: &[u8], c: u8) -> i32 {
        let mut at = x;
        for &ch in text {
            let Some(g) = font.glyph(ch) else { continue };
            let w = i32::from(g.width);
            for gy in 0..i32::from(font.height) {
                for gx in 0..w {
                    #[allow(clippy::cast_sign_loss)]
                    if g.bits[(gy * w + gx) as usize] {
                        self.put(at + gx, y + gy, c);
                    }
                }
            }
            at += w;
        }
        at - x
    }
}

/// The picture sheets, decoded as they're first drawn.
#[derive(Default)]
pub struct Pictures {
    sheets: RefCell<HashMap<(Module, u32), Option<Rc<Sheet>>>>,
    frames: RefCell<HashMap<(Module, u32, u16), Option<Rc<Image>>>>,
}

impl Pictures {
    /// Sheet `id` of module `m`'s pictures (type 1004).
    #[must_use]
    pub fn sheet(&self, assets: &Assets, m: Module, id: u32) -> Option<Rc<Sheet>> {
        self.sheets
            .borrow_mut()
            .entry((m, id))
            .or_insert_with(|| {
                assets
                    .get(m, rt::PICTURES, id)
                    .and_then(|b| Sheet::parse(b).ok())
                    .map(Rc::new)
            })
            .clone()
    }

    /// Frame `n` (from 1) of sheet `id`.
    #[must_use]
    pub fn frame(&self, assets: &Assets, m: Module, id: u32, n: u16) -> Option<Rc<Image>> {
        if let Some(f) = self.frames.borrow().get(&(m, id, n)) {
            return f.clone();
        }
        let img = self
            .sheet(assets, m, id)
            .and_then(|s| s.decode(n).ok())
            .map(Rc::new);
        self.frames.borrow_mut().insert((m, id, n), img.clone());
        img
    }

    /// Frame `n`'s rectangle in its sheet (a Mac `Rect`, usually at 0, 0).
    #[must_use]
    pub fn rect(&self, assets: &Assets, m: Module, id: u32, n: u16) -> Option<Rect> {
        self.sheet(assets, m, id)?.rect(n)
    }

    /// Forgets everything decoded (after the overrides changed).
    pub fn clear(&self) {
        self.sheets.borrow_mut().clear();
        self.frames.borrow_mut().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_fills_and_lines() {
        let mut s = Surface::new(4, 3);
        s.set_clip(IRect::new(1, 0, 4, 2));
        s.fill(IRect::new(-5, -5, 10, 10), 7);
        assert_eq!(s.pixels, [0, 7, 7, 7, 0, 7, 7, 7, 0, 0, 0, 0]);
        s.set_clip(s.bounds());
        s.line((0, 2), (3, 2), 1);
        assert_eq!(&s.pixels[8..], [1, 1, 1, 1]);
    }

    #[test]
    fn blits_with_transparency() {
        let mut img = Image::new(2, 1);
        img.pixels = vec![5, 6];
        img.opaque = vec![true, false];
        let mut s = Surface::new(3, 1);
        s.pixels = vec![9, 9, 9];
        s.blit(&img, 1, 0);
        assert_eq!(s.pixels, [9, 5, 9]);
    }
}
