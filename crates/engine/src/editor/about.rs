//! The About window (`CAboutWnd`, `0x429ca5`; docs/editor-layout.md 7.1):
//! sheet 10001's picture with the title (sheet 10004), the version and the
//! credits scrolling up through a window in it, until a click, Space or
//! Esc.

use crate::assets::{Assets, Module};
use crate::gadget::nearest;
use crate::gfx::{IRect, Pictures, Surface};

use super::ui;

const SIZE: (i32, i32) = (344, 271);
/// The scrolling window.
const SCROLL: IRect = IRect::new(36, 97, 310, 210);
/// The title picture's height (sheet 10004 frame 1, 274×232).
const TITLE_H: i32 = 232;
/// Milliseconds per pixel: every more than 5 ticks.
const STEP_MS: u64 = 100;

/// The open About window.
#[derive(Debug, Clone)]
pub struct About {
    started: u64,
}

impl About {
    #[must_use]
    pub fn new(now: u64) -> About {
        About { started: now }
    }

    /// Draws it centred in `area` at time `now`.
    pub fn draw(&self, s: &mut Surface, assets: &Assets, pictures: &Pictures, area: IRect, now: u64) {
        let r = IRect::sized(
            area.x0 + (area.width() - SIZE.0) / 2,
            area.y0 + (area.height() - SIZE.1) / 2,
            SIZE.0,
            SIZE.1,
        );
        let pic = |id: u32| {
            pictures
                .frame(assets, Module::Gadget, id, 1)
                .or_else(|| pictures.frame(assets, Module::Intro, id, 1))
        };
        s.fill(r, ui::BLACK);
        if let Some(img) = pic(10001) {
            s.blit(&img, r.x0, r.y0);
        }
        // Helvetica 16 bold, which Windows has as Arial; the nearest here.
        let Some(font) = assets.font("MS Sans Serif", 16, false).or_else(|| ui::font(assets)) else {
            return;
        };
        let credits = assets.text(Module::Exe, 10000).unwrap_or_default();
        let lines = ui::wrap(font, &credits, SCROLL.width());
        let lh = i32::from(font.height);
        let text_h = lh * i32::try_from(lines.len()).unwrap_or(0);
        let total = text_h + TITLE_H + SCROLL.y1;
        let n = i64::try_from(now.saturating_sub(self.started) / STEP_MS).unwrap_or(0);
        let (total, back) = (i64::from(total), i64::from(SCROLL.height()));
        // From 0 to `total`, then again from −113 (back in from below).
        let off = if n <= total {
            n
        } else {
            -back + (n - total - 1) % (total + back + 1)
        };
        let off = i32::try_from(off).unwrap_or(0);
        let sc = SCROLL.offset(r.x0, r.y0);
        s.fill(sc, ui::BLACK);
        let old = s.clip();
        s.set_clip(sc.intersect(old));
        if off < TITLE_H
            && let Some(img) = pic(10004)
        {
            s.blit(&img, sc.x0, sc.y0 - off);
        }
        let color = nearest(&assets.palette(), [0x8f, 0xc7, 0xf7]);
        let centred = |s: &mut Surface, y: i32, line: &str| {
            let w = ui::width(font, line);
            s.text(font, sc.x0 + (sc.width() - w) / 2, y, &crate::text::encode(line), color);
        };
        if off < 200 {
            let v = assets.string(100).unwrap_or_default();
            centred(s, sc.y0 + 66 - off, &v);
        }
        let y0 = sc.y0 + TITLE_H - off;
        if y0 < sc.y1 {
            for (i, l) in lines.iter().enumerate() {
                let y = y0 + lh * i32::try_from(i).unwrap_or(0);
                if y >= sc.y1 {
                    break;
                }
                centred(s, y, l);
            }
        }
        s.set_clip(old);
    }
}
