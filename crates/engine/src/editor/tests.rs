//! The editor driven by mouse and key events, with the CD's resources
//! (extracted to `orig`, as `tools/` leaves it; skipped without).

use super::*;
use crate::assets::Dir;
use crate::gadget::Tag;

fn editor() -> Option<Editor> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../orig");
    if !root.exists() {
        eprintln!("no orig: skipped");
        return None;
    }
    let assets = Assets::load(Some(&Dir(root)), None).ok()?;
    Some(Editor::new(assets, Document::new(), 800, 600))
}

/// Adds a part of class `tag` at `h`, `v`; returns its index.
fn add(e: &mut Editor, tag: [u8; 4], h: i32, v: i32) -> usize {
    let id = e.doc.new_id();
    let g = doc::new_gadget(&e.assets, Tag(tag), id, h, v);
    e.doc.gadgets.push(g);
    e.doc.gadgets.len() - 1
}

/// Knot `k` of gadget `g` in frame coordinates.
fn knot(e: &Editor, g: usize, input: bool) -> (usize, (i32, i32)) {
    let core = &e.doc.gadgets[g].core;
    let k = core.knots.iter().position(|k| k.is_input() == input).unwrap();
    let (x, y) = core.knot_at(k).unwrap();
    (k, (x, y + menu::BAR_H))
}

fn centre(e: &Editor, g: usize) -> (i32, i32) {
    let r = e.doc.gadgets[g].core.rect();
    (i32::midpoint(r.x0, r.x1), i32::midpoint(r.y0, r.y1) + menu::BAR_H)
}

fn drag(e: &mut Editor, from: (i32, i32), to: (i32, i32)) {
    e.mouse_move(from.0, from.1);
    e.mouse_down(from.0, from.1, Button::Left);
    e.mouse_move(i32::midpoint(from.0, to.0), i32::midpoint(from.1, to.1));
    e.mouse_move(to.0, to.1);
    e.mouse_up(to.0, to.1, Button::Left);
    // Past the double-click time.
    e.update(600, (0, 0, 0));
}

#[test]
fn wiring_snaps_moves_springs_back_and_undoes() {
    let Some(mut e) = editor() else { return };
    let and = add(&mut e, *b"AndG", 100, 200);
    let bulb = add(&mut e, *b"bulb", 400, 200);
    let (ko, out) = knot(&e, and, false);
    // Released over the bulb's bounds: the wire snaps to its free input.
    let to = centre(&e, bulb);
    drag(&mut e, out, to);
    assert_eq!(e.doc.wires.len(), 1);
    // The AND gate was raised to the head when pressed.
    let (and, bulb) = (bulb, and);
    let w = &e.doc.wires[0];
    assert_eq!(e.doc.gadgets[and].core.tag, Tag(*b"AndG"));
    assert_eq!((w.from, w.from_knot, w.to), (and, ko, bulb));
    assert_eq!(e.doc.gadgets[and].core.knots[ko].wire, Some(0));
    assert!(e.dirty);
    let n = e.doc.gadgets.len();
    let (_, inp) = knot(&e, bulb, true);
    // Its input is wired: pressing there picks the wire's end up; dropped
    // in empty space it springs back.
    drag(&mut e, inp, (600, 500));
    assert_eq!(e.doc.wires.len(), 1);
    let w = &e.doc.wires[0];
    assert_eq!(e.doc.gadgets[w.to].core.tag, Tag(*b"bulb"));
    assert_eq!(e.doc.gadgets[w.to].core.knots[w.to_knot].wire, Some(0));
    assert_eq!(e.doc.gadgets.len(), n);
    // Undo is one step and a toggle.
    e.undo();
    assert_eq!(e.doc.wires.len(), 1);
    e.command(cmd::REMOVE_WIRES);
    assert_eq!(e.doc.wires, []);
    e.undo();
    assert_eq!(e.doc.wires.len(), 1);
    e.undo();
    assert_eq!(e.doc.wires, []);
}

#[test]
fn a_new_wire_dropped_on_nothing_disappears() {
    let Some(mut e) = editor() else { return };
    let and = add(&mut e, *b"AndG", 100, 200);
    let (_, out) = knot(&e, and, false);
    drag(&mut e, out, (700, 550));
    assert_eq!(e.doc.wires, []);
}

#[test]
fn joints_are_added_dragged_and_cut() {
    let Some(mut e) = editor() else { return };
    let and = add(&mut e, *b"AndG", 100, 300);
    let bulb = add(&mut e, *b"bulb", 500, 100);
    let (ko, _) = knot(&e, and, false);
    let (ki, _) = knot(&e, bulb, true);
    e.doc.connect(and, ko, bulb, ki, Vec::new(), 0);
    let pts = worksheet::wire_points(&e.doc, 0, None);
    let mid = (i32::midpoint(pts[0].0, pts[1].0), i32::midpoint(pts[0].1, pts[1].1));
    // A click on the wire puts a joint there and drags it.
    drag(&mut e, (mid.0, mid.1 + menu::BAR_H), (mid.0, mid.1 + menu::BAR_H + 40));
    assert_eq!(e.doc.wires[0].points, [(mid.0, mid.1 + 40)]);
    // Shift is the Cut tool while held; it cuts the joint.
    e.key_down(0x10);
    let j = (mid.0, mid.1 + 40 + menu::BAR_H);
    e.mouse_move(j.0, j.1);
    e.mouse_down(j.0, j.1, Button::Left);
    e.mouse_up(j.0, j.1, Button::Left);
    e.key_up(0x10);
    assert_eq!(e.doc.wires[0].points, []);
    assert_eq!(e.tray.tool(), Tool::Pointer);
}

#[test]
fn dragging_raises_and_moves_a_gadget() {
    let Some(mut e) = editor() else { return };
    let a = add(&mut e, *b"Coin", 100, 100);
    let _b = add(&mut e, *b"Coin", 400, 400);
    let c = centre(&e, a);
    drag(&mut e, c, (c.0 + 30, c.1 + 20));
    // Moved to the head of the list (the end of the port's).
    let g = &e.doc.gadgets[1];
    assert_eq!(g.core.tag, Tag(*b"Coin"));
    assert_eq!((g.core.h, g.core.v), (130, 120));
}

#[test]
fn hover_selects_and_delete_returns_the_part() {
    let Some(mut e) = editor() else { return };
    let g = add(&mut e, *b"Coin", 300, 300);
    let i = tray::index_of(Tag(*b"Coin")).unwrap();
    e.tray.counts[usize::from(i)] = 2;
    let c = centre(&e, g);
    e.mouse_move(c.0, c.1);
    e.update(16, (0, 0, 0));
    assert_eq!(e.sel, Sel::Gadget(g));
    e.key_down(0x2e);
    assert_eq!(e.doc.gadgets.len(), 0);
    assert_eq!(e.tray.counts[usize::from(i)], 3);
    assert!(e.can_undo());
}

#[test]
fn running_takes_clicks_and_keys_only() {
    let Some(mut e) = editor() else { return };
    let g = add(&mut e, *b"Coin", 300, 300);
    e.key_down(0x0d);
    e.key_up(0x0d);
    assert!(e.running);
    assert!(!e.tray.bar.visible);
    assert!(e.menu.grayed.contains(&cmd::NEW));
    assert!(!e.menu.grayed.contains(&cmd::EXIT));
    let c = centre(&e, g);
    drag(&mut e, c, (c.0 + 50, c.1));
    assert_eq!(e.doc.gadgets[g].core.h, 300, "no editing while running");
    e.key_down(0x2e);
    assert_eq!(e.doc.gadgets.len(), 1);
    // Enter doesn't stop; Esc does.
    e.key_down(0x0d);
    assert!(e.running);
    e.key_down(0x1b);
    assert!(!e.running);
    assert!(e.tray.bar.visible);
}

#[test]
fn parts_come_from_the_tray_and_go_back() {
    let Some(mut e) = editor() else { return };
    let slot = e.tray.page()[0];
    let i = usize::from(slot.index);
    e.tray.counts[i] = 1;
    let at = (
        e.tray.bar.pos.0 + tray::AREA.x0 + slot.rect.x0 + 2,
        e.tray.bar.pos.1 + tray::AREA.y0 + slot.rect.y0 + 2 + menu::BAR_H,
    );
    drag(&mut e, at, (400, 450));
    assert_eq!(e.doc.gadgets.len(), 1);
    assert_eq!(e.tray.counts[i], 0);
    // An empty slot gives nothing.
    drag(&mut e, at, (400, 300));
    assert_eq!(e.doc.gadgets.len(), 1);
    // Dropped back on the tray, it returns.
    e.tray.counts[i] = 1;
    drag(&mut e, at, (at.0 + 40, at.1));
    assert_eq!(e.doc.gadgets.len(), 1);
    assert_eq!(e.tray.counts[i], 1);
    // The limits are saved with the widget.
    e.snapshot();
    assert!(e.doc.tray_limits.contains(&(tray::tag_of(slot.index).unwrap(), 1)));
}

#[test]
fn menu_rules() {
    let Some(mut e) = editor() else { return };
    e.update(16, (0, 0, 0));
    assert!(e.menu.grayed.contains(&cmd::UNDO));
    assert!(e.menu.grayed.contains(&cmd::SAVE));
    assert!(e.menu.grayed.contains(&cmd::REVERT));
    add(&mut e, *b"Coin", 300, 300);
    e.command(cmd::STEEL);
    assert_eq!(e.doc.background, 702);
    assert_eq!(e.menu.checked, [cmd::STEEL]);
    assert!(!e.menu.grayed.contains(&cmd::UNDO));
    assert!(!e.menu.grayed.contains(&cmd::SAVE));
    // Clear Work Area keeps the background and every locked part.
    let locked = add(&mut e, *b"Coin", 100, 100);
    e.doc.gadgets[locked].core.locked = true;
    e.command(cmd::CLEAR);
    assert_eq!(e.doc.gadgets.len(), 1);
    assert!(e.doc.gadgets[0].core.locked);
    assert_eq!(e.doc.background, 702);
    // Ctrl+Z is Undo.
    e.key_down(0x11);
    e.key_down(u16::from(b'Z'));
    e.key_up(u16::from(b'Z'));
    e.key_up(0x11);
    assert_eq!(e.doc.gadgets.len(), 2);
}

#[test]
fn the_tray_is_laid_out_as_the_original() {
    let Some(e) = editor() else { return };
    let pages: Vec<usize> = e.tray.folders.iter().map(Vec::len).collect();
    assert_eq!(pages, [2, 1, 2, 5, 2, 4, 2]);
    let xs = |f: usize, p: usize| -> Vec<(u8, i32, i32)> {
        e.tray.folders[f][p]
            .iter()
            .map(|s| (s.index, s.rect.x0, s.rect.x1))
            .collect()
    };
    // docs/editor-layout.md 3.6.
    assert_eq!(xs(0, 0), [(16, 38, 67), (18, 105, 150), (50, 188, 238), (51, 276, 310)]);
    assert_eq!(xs(1, 0), [(9, 60, 113), (4, 173, 202), (11, 262, 288)]);
    assert_eq!(
        xs(3, 0).iter().map(|s| s.1).collect::<Vec<_>>(),
        [15, 56, 114, 174, 244]
    );
    let label = e.tray.folders[5][0][0];
    assert_eq!((label.index, label.rect.y0, label.rect.y1), (64, 15, 24));
}
