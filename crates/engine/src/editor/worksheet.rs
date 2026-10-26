//! The worksheet (`cWorksheetPane`): hit tests, hover selection, the
//! wires' and joints' drawing, the background and its pipe frame
//! (docs/editor-interaction.md 3, 4, 9; docs/editor-layout.md 4.3).
//!
//! The document's lists are kept in the port's order: index 0 is the
//! original list's tail, the last index its head. So drawing walks the
//! lists forwards and hit tests walk them backwards (the head first).

use crate::doc::Document;
use crate::gadget::Core;
use crate::gfx::{IRect, Surface};

/// A knot is under the pointer within this squared distance (`0x423ec3`,
/// `0x425909`: `d² < 0xc5`).
pub const KNOT_D2: i32 = 197;
/// A joint is under the pointer within this squared distance (`0x43c30a`).
pub const JOINT_D2: i32 = 37;
/// A wire is under the pointer nearer than this (`DAT_00496620`).
pub const WIRE_DIST: f64 = 5.0;

/// Palette indices the worksheet draws with (all exact in palette 101).
pub mod color {
    /// RGB(0,128,0): a connected wire, the selection frames.
    pub const GREEN: u8 = 2;
    /// RGB(128,0,0): a wire with a loose end.
    pub const RED: u8 = 1;
    /// RGB(0,0,128): a joint.
    pub const JOINT: u8 = 4;
    pub const WHITE: u8 = 255;
    pub const BLACK: u8 = 0;
}

/// What the pointer hovers over: the selection (at most one thing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sel {
    #[default]
    None,
    Gadget(usize),
    /// A wire and its segment.
    Wire(usize, usize),
    /// A wire and the index of its joint.
    Joint(usize, usize),
}

/// A wire's end held by the pointer: drawn at `at` instead of its knot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loose {
    pub wire: usize,
    /// The source (output) end, else the destination.
    pub source: bool,
    pub at: (i32, i32),
    /// Over a free knot it would connect to (drawn green).
    pub snapped: bool,
}

/// The topmost gadget whose bounds hold `p` (`0x42391d`, slot 0x18 =
/// `0x41dac5` for every class).
#[must_use]
pub fn gadget_at(doc: &Document, p: (i32, i32)) -> Option<usize> {
    doc.gadgets.iter().rposition(|g| g.core.rect().contains(p.0, p.1))
}

fn d2(a: (i32, i32), b: (i32, i32)) -> i32 {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    dx * dx + dy * dy
}

/// The knot of `core` nearest `p` and its squared distance (`0x41ebd7`).
#[must_use]
pub fn nearest_knot(core: &Core, p: (i32, i32)) -> Option<(usize, i32)> {
    let mut best: Option<(usize, i32)> = None;
    for k in 0..core.knots.len() {
        let Some(at) = core.knot_at(k) else { continue };
        let d = d2(p, at);
        if best.is_none_or(|(_, b)| d < b) {
            best = Some((k, d));
        }
    }
    best
}

/// The free knot of `core` nearest `p` that is an input (or an output),
/// at any distance (`0x41ddb5`, `0x41df70`).
#[must_use]
pub fn nearest_free_knot(core: &Core, p: (i32, i32), input: bool) -> Option<usize> {
    let mut best: Option<(usize, i32)> = None;
    for (k, knot) in core.knots.iter().enumerate() {
        if knot.wire.is_some() || knot.is_input() != input {
            continue;
        }
        let Some(at) = core.knot_at(k) else { continue };
        let d = d2(p, at);
        if best.is_none_or(|(_, b)| d < b) {
            best = Some((k, d));
        }
    }
    best.map(|(k, _)| k)
}

/// Wire `w`'s points: its source knot, its joints, its destination knot;
/// a loose end is where the pointer holds it.
#[must_use]
pub fn wire_points(doc: &Document, w: usize, loose: Option<Loose>) -> Vec<(i32, i32)> {
    let wire = &doc.wires[w];
    let end = |g: usize, k: usize, source: bool| match loose {
        Some(l) if l.wire == w && l.source == source => l.at,
        _ => doc.gadgets[g].core.knot_at(k).unwrap_or_default(),
    };
    let mut pts = Vec::with_capacity(wire.points.len() + 2);
    pts.push(end(wire.from, wire.from_knot, true));
    pts.extend_from_slice(&wire.points);
    pts.push(end(wire.to, wire.to_knot, false));
    pts
}

/// A wire's box: its ends and joints, 3 pixels larger (`0x43beac`).
#[must_use]
pub fn points_bbox(pts: &[(i32, i32)]) -> IRect {
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for &(x, y) in pts {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    IRect::new(x0 - 3, y0 - 3, x1 + 3, y1 + 3)
}

/// Whether the polyline `pts` passes under `p`, and the nearest segment
/// (`0x43b7ca`): within the box, each segment whose end points' rectangle
/// grown by 5 holds `p` is measured by its distance from the line through
/// it; the nearest under 5 pixels wins.
#[must_use]
pub fn polyline_hit(pts: &[(i32, i32)], p: (i32, i32)) -> Option<usize> {
    if !points_bbox(pts).contains(p.0, p.1) {
        return None;
    }
    let mut best = WIRE_DIST * 2.0;
    let mut seg = None;
    for (i, s) in pts.windows(2).enumerate() {
        let (a, b) = (s[0], s[1]);
        let r = IRect::new(a.0.min(b.0) - 5, a.1.min(b.1) - 5, a.0.max(b.0) + 5, a.1.max(b.1) + 5);
        if !r.contains(p.0, p.1) {
            continue;
        }
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len2 = dx * dx + dy * dy;
        let d = if len2 == 0 {
            0.0
        } else {
            f64::from((dy * (p.0 - a.0) - dx * (p.1 - a.1)).abs()) / f64::from(len2).sqrt()
        };
        if d < best {
            best = d;
            seg = Some(i);
        }
    }
    (best < WIRE_DIST).then_some(seg).flatten()
}

/// The topmost wire under `p` and its segment (`0x42533a`).
#[must_use]
pub fn wire_at(doc: &Document, p: (i32, i32)) -> Option<(usize, usize)> {
    (0..doc.wires.len())
        .rev()
        .find_map(|w| polyline_hit(&wire_points(doc, w, None), p).map(|s| (w, s)))
}

/// The joint under `p`: the first wire (topmost) whose box holds `p` and
/// that has a joint within 6 pixels; its nearest (`0x425c6d`, `0x43c30a`).
#[must_use]
pub fn joint_at(doc: &Document, p: (i32, i32)) -> Option<(usize, usize)> {
    for w in (0..doc.wires.len()).rev() {
        if !points_bbox(&wire_points(doc, w, None)).contains(p.0, p.1) {
            continue;
        }
        if let Some(j) = nearest_joint(&doc.wires[w].points, p) {
            return Some((w, j));
        }
    }
    None
}

/// The joint in `joints` nearest `p` within 6 pixels.
#[must_use]
pub fn nearest_joint(joints: &[(i32, i32)], p: (i32, i32)) -> Option<usize> {
    let mut best: Option<(usize, i32)> = None;
    for (j, &at) in joints.iter().enumerate() {
        let d = d2(p, at);
        if d < JOINT_D2 && best.is_none_or(|(_, b)| d < b) {
            best = Some((j, d));
        }
    }
    best.map(|(j, _)| j)
}

/// Where a click on a wire puts a new joint (`0x43c470`): before the joint
/// that ends the segment nearest `p` (the same measure as the hit test),
/// or last for its last segment. `None` if `p` isn't on it.
#[must_use]
pub fn joint_insert_index(pts: &[(i32, i32)], p: (i32, i32)) -> Option<usize> {
    polyline_hit(pts, p)
}

/// Whether wire `w` is locked (`+0x25`).
#[must_use]
pub fn wire_locked(doc: &Document, w: usize) -> bool {
    doc.wires.get(w).is_some_and(|w| w.flag != 0)
}

/// Draws a line as the editor's DIB code does (`0x43849e`): both ends
/// inclusive, stepping the longer axis, rounding the other.
pub fn dib_line(s: &mut Surface, a: (i32, i32), b: (i32, i32), c: u8) {
    if a == b {
        return;
    }
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    if dy.abs() > dx.abs() {
        let (a, b) = if a.1 <= b.1 { (a, b) } else { (b, a) };
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        for y in a.1..=b.1 {
            s.pset(a.0 + ((y - a.1) * dx + dy / 2) / dy, y, c);
        }
    } else {
        let (a, b) = if a.0 <= b.0 { (a, b) } else { (b, a) };
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        for x in a.0..=b.0 {
            s.pset(x, a.1 + ((x - a.0) * dy + dx / 2) / dx, c);
        }
    }
}

/// Draws a line as GDI's `MoveTo`/`LineTo` with a one-pixel pen does: the
/// last pixel left out (Bresenham).
pub fn gdi_line(s: &mut Surface, a: (i32, i32), b: (i32, i32), c: u8, width: i32) {
    let (mut x, mut y) = a;
    let dx = (b.0 - x).abs();
    let dy = -(b.1 - y).abs();
    let sx = if x < b.0 { 1 } else { -1 };
    let sy = if y < b.1 { 1 } else { -1 };
    let mut err = dx + dy;
    let steep = -dy > dx;
    while (x, y) != b {
        s.pset(x, y, c);
        if width > 1 {
            // A two-pixel pen: one more pixel across the line.
            if steep {
                s.pset(x - 1, y, c);
            } else {
                s.pset(x, y - 1, c);
            }
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

/// A joint (`0x43cb82`): a 4×4 square, `x−2..x+1`.
pub fn draw_joint(s: &mut Surface, p: (i32, i32)) {
    s.fill(IRect::new(p.0 - 2, p.1 - 2, p.0 + 2, p.1 + 2), color::JOINT);
}

/// The selected joint's frame (`0x43cc9b`): `(x−3, y−3)-(x+3, y+3)`.
pub fn draw_joint_frame(s: &mut Surface, p: (i32, i32)) {
    s.frame(IRect::new(p.0 - 3, p.1 - 3, p.0 + 3, p.1 + 3), color::GREEN);
}

/// Draws a wire's polyline and joints (`0x43a489`): green when both ends
/// are connected or the loose one is snapped, else red; segment `skip`
/// left out (it floats with a dragged gadget, drawn later).
pub fn draw_polyline(s: &mut Surface, pts: &[(i32, i32)], green: bool, running: bool, skip: Option<usize>) {
    let c = if green { color::GREEN } else { color::RED };
    for (i, seg) in pts.windows(2).enumerate() {
        if Some(i) == skip {
            continue;
        }
        if running {
            gdi_line(s, seg[0], seg[1], c, 1);
        } else {
            dib_line(s, seg[0], seg[1], c);
        }
    }
    if pts.len() > 2 {
        for &p in &pts[1..pts.len() - 1] {
            draw_joint(s, p);
        }
    }
}

/// The selected segment again with a two-pixel pen (`0x43bb96`).
pub fn draw_selected_segment(s: &mut Surface, pts: &[(i32, i32)], seg: usize, locked: bool) {
    if let (Some(&a), Some(&b)) = (pts.get(seg), pts.get(seg + 1)) {
        gdi_line(s, a, b, if locked { color::RED } else { color::GREEN }, 2);
    }
}

/// Moves gadget `i` to the head of the list (the end of the port's), so it
/// is drawn last and hit first (`0x423ec3`). Returns its new index.
pub fn raise_gadget(doc: &mut Document, i: usize) -> usize {
    let last = doc.gadgets.len() - 1;
    if i == last {
        return i;
    }
    let g = doc.gadgets.remove(i);
    doc.gadgets.push(g);
    let fix = |x: usize| match x {
        x if x == i => last,
        x if x > i => x - 1,
        x => x,
    };
    for w in &mut doc.wires {
        w.from = fix(w.from);
        w.to = fix(w.to);
    }
    last
}

/// Moves wire `w` to the head of the wire list. Returns its new index.
pub fn raise_wire(doc: &mut Document, w: usize) -> usize {
    let last = doc.wires.len() - 1;
    if w == last {
        return w;
    }
    let wire = doc.wires.remove(w);
    doc.wires.push(wire);
    let fix = |x: usize| match x {
        x if x == w => last,
        x if x > w => x - 1,
        x => x,
    };
    for g in &mut doc.gadgets {
        for k in &mut g.core.knots {
            k.wire = k.wire.map(fix);
        }
    }
    last
}

/// Every gadget's and wire's extent: the gadgets' screen bounds and the
/// wires' boxes (`0x4291d9`); empty if there is nothing.
#[must_use]
pub fn extents(doc: &Document) -> IRect {
    let mut r = IRect::default();
    for g in &doc.gadgets {
        r = r.union(g.core.rect());
    }
    for w in 0..doc.wires.len() {
        r = r.union(points_bbox(&wire_points(doc, w, None)));
    }
    r
}

/// Whether panning by `d` is allowed (`0x425f10`): the widget must keep a
/// gadget in the middle of the view (the view shrunk by a tenth on each
/// side), unless none is there already.
#[must_use]
pub fn may_offset(doc: &Document, view: IRect, d: (i32, i32)) -> bool {
    let (w, h) = (view.width() / 10, view.height() / 10);
    let inner = IRect::new(view.x0 + w, view.y0 + h, view.x1 - w, view.y1 - h);
    let meets = |r: IRect| !r.intersect(inner).is_empty();
    let now = doc.gadgets.iter().any(|g| meets(g.core.rect()));
    let after = doc.gadgets.iter().any(|g| meets(g.core.rect().offset(d.0, d.1)));
    !now || after
}

/// Moves every gadget and joint by `d`.
pub fn offset_all(doc: &mut Document, d: (i32, i32)) {
    for g in &mut doc.gadgets {
        g.core.h += d.0;
        g.core.v += d.1;
    }
    for w in &mut doc.wires {
        for p in &mut w.points {
            p.0 += d.0;
            p.1 += d.1;
        }
    }
}

/// The squeeze of Fit Work Area (`0x428f1c`): scales and offsets, or
/// `None` when `e` is inside `t` already.
#[must_use]
pub fn fit_params(e: IRect, t: IRect) -> Option<(f64, f64, f64, f64)> {
    if e.intersect(t) == e {
        return None;
    }
    let (mut sx, mut sy, mut ox, mut oy) = (1.0, 1.0, 0.0, 0.0);
    if t.width() < e.width() {
        sx = f64::from(t.width()) / f64::from(e.width());
    }
    if t.height() < e.height() {
        sy = f64::from(t.height()) / f64::from(e.height());
    }
    if f64::from(e.x0) * sx < f64::from(t.x0) || f64::from(e.x1) * sx > f64::from(t.x1) {
        ox = f64::from(t.x0) - f64::from(e.x0) * sx;
    }
    if f64::from(e.y0) * sy < f64::from(t.y0) || f64::from(e.y1) * sy > f64::from(t.y1) {
        oy = f64::from(t.y0) - f64::from(e.y0) * sy;
    }
    Some((sx, sy, ox, oy))
}

/// Applies [`fit_params`] to every gadget and joint (`0x429060`),
/// truncating.
#[allow(clippy::cast_possible_truncation)]
pub fn squeeze(doc: &mut Document, (sx, sy, ox, oy): (f64, f64, f64, f64)) {
    let f = |p: (i32, i32)| ((f64::from(p.0) * sx + ox) as i32, (f64::from(p.1) * sy + oy) as i32);
    for g in &mut doc.gadgets {
        (g.core.h, g.core.v) = f((g.core.h, g.core.v));
    }
    for w in &mut doc.wires {
        for p in &mut w.points {
            *p = f(*p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_hit_measures_the_line() {
        let pts = [(10, 10), (50, 10), (50, 60)];
        assert_eq!(polyline_hit(&pts, (30, 12)), Some(0));
        assert_eq!(polyline_hit(&pts, (30, 15)), None);
        assert_eq!(polyline_hit(&pts, (30, 14)), Some(0));
        assert_eq!(polyline_hit(&pts, (52, 40)), Some(1));
        // Near the corner both segments are in range: the nearer wins.
        assert_eq!(polyline_hit(&pts, (49, 12)), Some(1));
        // Outside the box (3 pixels around the points).
        assert_eq!(polyline_hit(&pts, (5, 10)), None);
        // The line through a segment counts only inside its rectangle + 5.
        assert_eq!(polyline_hit(&[(0, 0), (10, 0)], (12, 0)), Some(0));
    }

    #[test]
    fn joints_within_six_pixels() {
        let j = [(10, 10), (14, 10)];
        assert_eq!(nearest_joint(&j, (13, 10)), Some(1));
        assert_eq!(nearest_joint(&j, (10, 17)), None);
        assert_eq!(nearest_joint(&j, (10, 16)), Some(0));
        assert_eq!(nearest_joint(&j, (10, 15)), Some(0));
        assert_eq!(nearest_joint(&j, (5, 14)), None);
    }

    #[test]
    fn dib_lines_round_like_the_original() {
        let mut s = Surface::new(8, 8);
        dib_line(&mut s, (0, 0), (4, 1), 1);
        let row = |s: &Surface, y: i32| (0..8).map(|x| s.get(x, y).unwrap()).collect::<Vec<_>>();
        assert_eq!(row(&s, 0), [1, 1, 0, 0, 0, 0, 0, 0]);
        assert_eq!(row(&s, 1), [0, 0, 1, 1, 1, 0, 0, 0]);
    }

    #[test]
    fn fit_squeezes_independently() {
        let e = IRect::new(0, 0, 1000, 100);
        let (sx, sy, ox, oy) = fit_params(e, IRect::new(14, 14, 626, 466)).unwrap();
        assert!((sx - 0.612).abs() < 1e-9);
        assert!((sy - 1.0).abs() < 1e-9);
        assert!((ox - 14.0).abs() < 1e-9 && (oy - 14.0).abs() < 1e-9);
        assert!(fit_params(IRect::new(20, 20, 100, 100), IRect::new(14, 14, 626, 466)).is_none());
    }
}
