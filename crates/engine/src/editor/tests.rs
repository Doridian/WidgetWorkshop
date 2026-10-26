//! The editor driven by mouse and key events, with the CD's resources
//! (extracted to `orig`, as `tools/` leaves it; skipped without).

use super::*;
use crate::assets::Dir;
use crate::gadget::Tag;
use crate::gadgets::sound_time::TapePlayer;

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

/// The original has no Space key for GO / STOP (accelerator table 2 and
/// the view's keys, `0x42385d`, only know Enter and Esc), so a Keypress
/// set to Space just sees it, polled, while running.
#[test]
fn space_is_only_a_key_for_the_parts() {
    let Some(mut e) = editor() else { return };
    let g = add(&mut e, *b"Key_", 300, 300);
    e.doc.gadgets[g].core.settings.text[0] = b' ';
    e.key_down(0x20);
    e.key_up(0x20);
    assert!(!e.running, "Space doesn't start");
    e.key_down(0x0d);
    e.key_up(0x0d);
    assert!(e.running);
    e.key_down(0x20);
    e.update(100, (0, 0, 0));
    assert!(e.running, "Space doesn't stop");
    assert!(e.doc.gadgets[g].core.state);
    e.key_up(0x20);
    e.update(100, (0, 0, 0));
    assert!(!e.doc.gadgets[g].core.state);
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

/// A microphone that gives `chunks`, one per read.
struct FakeMic {
    start: bool,
    chunks: std::collections::VecDeque<std::result::Result<Vec<u8>, ()>>,
    stops: std::rc::Rc<std::cell::Cell<u32>>,
}

impl FakeMic {
    fn new(chunks: impl IntoIterator<Item = std::result::Result<Vec<u8>, ()>>) -> FakeMic {
        FakeMic {
            start: true,
            chunks: chunks.into_iter().collect(),
            stops: std::rc::Rc::default(),
        }
    }
}

impl Mic for FakeMic {
    fn start(&mut self) -> std::result::Result<(), String> {
        if self.start { Ok(()) } else { Err("no".into()) }
    }

    fn read(&mut self, out: &mut Vec<u8>) -> std::result::Result<(), String> {
        match self.chunks.pop_front() {
            Some(Ok(c)) => out.extend(c),
            Some(Err(())) => return Err("failed".into()),
            None => {}
        }
        Ok(())
    }

    fn stop(&mut self) {
        self.stops.set(self.stops.get() + 1);
    }
}

/// A click on control `id` of the open dialog.
fn click_control(e: &mut Editor, id: u16) {
    let (x, y) = e.dialog.as_ref().unwrap().centre_of(id).unwrap();
    e.mouse_move(x, y);
    e.mouse_down(x, y, Button::Left);
    e.mouse_up(x, y, Button::Left);
}

fn tape_samples(e: &Editor, g: usize) -> Vec<u8> {
    let t = e.doc.gadgets[g].behavior.as_any().downcast_ref::<TapePlayer>().unwrap();
    t.samples().to_vec()
}

fn prompt_text(e: &Editor) -> Option<String> {
    e.prompt.as_ref().map(|(p, _)| p.text.clone())
}

#[test]
fn recorder_records_into_the_part_for_good() {
    let Some(mut e) = editor() else { return };
    let g = add(&mut e, *b"Tape", 100, 200);
    e.doc.gadgets[g].core.broken = true;
    let data: Vec<u8> = (0..45000u32).map(|i| u8::try_from(i % 200).unwrap()).collect();
    let mic = FakeMic::new(data.chunks(10000).map(|c| Ok(c.to_vec())));
    let stops = mic.stops.clone();
    e.mic = Box::new(mic);
    e.open_settings(g);
    e.take_sounds();
    click_control(&mut e, 106);
    assert!(e.recording());
    assert!(!e.doc.gadgets[g].core.broken);
    // The output stops while recording.
    assert!(e.take_sounds().contains(&SoundCmd::StopAll));
    // Held up: Esc and clicks do nothing.
    e.key_down(0x1b);
    e.key_up(0x1b);
    click_control(&mut e, 2);
    assert!(e.dialog.is_some());
    e.update(16, (0, 0, 0));
    assert_eq!(e.dialog.as_ref().unwrap().progress(), 26);
    for _ in 0..3 {
        e.update(16, (0, 0, 0));
    }
    assert!(!e.recording());
    assert_eq!(stops.get(), 1);
    assert_eq!(e.dialog.as_ref().unwrap().progress(), 100);
    assert_eq!(tape_samples(&e, g), data[..40000]);
    // Cancel doesn't take it back.
    click_control(&mut e, 2);
    assert!(e.dialog.is_none());
    assert_eq!(tape_samples(&e, g), data[..40000]);
    // Nor does Undo after OK: the snapshot comes after the recording.
    e.open_settings(g);
    e.mic = Box::new(FakeMic::new([Ok(vec![7; 40000])]));
    click_control(&mut e, 106);
    e.update(16, (0, 0, 0));
    click_control(&mut e, 1);
    e.undo();
    let g = e.doc.gadgets.iter().position(|x| x.core.tag == Tag(*b"Tape")).unwrap();
    assert_eq!(tape_samples(&e, g), vec![7; 40000]);
}

#[test]
fn recorder_errors_break_the_part_and_disable_record() {
    let Some(mut e) = editor() else { return };
    let g = add(&mut e, *b"Tape", 100, 200);
    let before = tape_samples(&e, g);
    // No input device: at once.
    e.open_settings(g);
    click_control(&mut e, 106);
    assert!(!e.recording());
    assert!(e.doc.gadgets[g].core.broken);
    assert_eq!(
        prompt_text(&e).as_deref(),
        Some("There was an error while recording sound")
    );
    let d = e.dialog.as_ref().unwrap();
    assert!(!d.button_enabled(2) && d.button_enabled(3));
    assert_eq!(d.progress(), 100);
    e.key_down(0x0d);
    assert!(e.prompt.is_none() && e.dialog.is_some());
    // Disabled for the rest of the dialog.
    click_control(&mut e, 106);
    assert!(e.prompt.is_none());
    click_control(&mut e, 2);
    // A device that fails half way, or stops giving samples.
    for mic in [
        FakeMic::new([Ok(vec![1; 100]), Err(())]),
        FakeMic::new([Ok(vec![1; 100])]),
    ] {
        e.doc.gadgets[g].core.broken = false;
        e.mic = Box::new(mic);
        e.open_settings(g);
        click_control(&mut e, 106);
        assert!(e.recording());
        for _ in 0..5 {
            e.update(1000, (0, 0, 0));
        }
        assert!(!e.recording());
        assert!(e.doc.gadgets[g].core.broken);
        assert!(e.prompt.is_some());
        e.key_down(0x0d);
        click_control(&mut e, 2);
    }
    // The buffer keeps what it had.
    assert_eq!(tape_samples(&e, g), before);
}

#[test]
fn recorder_imports_wave_files() {
    let Some(mut e) = editor() else { return };
    let g = add(&mut e, *b"Tape", 100, 200);
    let dir = std::env::temp_dir().join(format!("ww-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let wav = |bits: u16, data: &[u8]| {
        let mut w = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0".to_vec();
        w.extend_from_slice(&11025u32.to_le_bytes());
        w.extend_from_slice(&[0; 6]);
        w.extend_from_slice(&bits.to_le_bytes());
        w.extend_from_slice(b"data\0\0\0\0");
        w.extend_from_slice(data);
        w
    };
    let good = dir.join("good.wav");
    std::fs::write(&good, wav(8, &[1, 2, 3])).unwrap();
    let deep = dir.join("deep.wav");
    std::fs::write(&deep, wav(16, &[1, 2])).unwrap();
    let bad = dir.join("bad.wav");
    std::fs::write(&bad, b"not a wave file at all, but long enough to read").unwrap();
    let missing = dir.join("missing.wav");
    let import = |e: &mut Editor, path: &std::path::Path, answer: u16| {
        click_control(e, 107);
        let (d, then) = e.file_dialog.as_mut().unwrap();
        assert_eq!(*then, Then::ImportWav(g));
        d.name = path.display().to_string();
        e.key_down(answer);
    };
    e.open_settings(g);
    e.doc.gadgets[g].core.broken = true;
    // Cancelled: nothing.
    import(&mut e, &good, 0x1b);
    assert!(e.doc.gadgets[g].core.broken && e.file_dialog.is_none());
    import(&mut e, &good, 0x0d);
    assert!(!e.doc.gadgets[g].core.broken && e.prompt.is_none() && e.file_dialog.is_none());
    assert_eq!(tape_samples(&e, g)[..4], [1, 2, 3, 0x80]);
    // The next one starts in the same folder.
    click_control(&mut e, 107);
    let (d, _) = e.file_dialog.as_ref().unwrap();
    assert_eq!((d.dir.as_path(), d.name.as_str()), (dir.as_path(), ""));
    assert_eq!(
        d.items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
        ["bad.wav", "deep.wav", "good.wav"]
    );
    e.key_down(0x1b);
    // A missing file: the dialog's own message (`OFN_FILEMUSTEXIST`).
    import(&mut e, &missing, 0x0d);
    let msg = e.file_dialog.as_ref().unwrap().0.message().unwrap().to_owned();
    assert!(
        msg.ends_with("File not found.\nPlease verify the correct file name was given."),
        "{msg}"
    );
    e.key_down(0x0d);
    e.key_down(0x1b);
    assert!(e.file_dialog.is_none());
    for (path, msg) in [(&deep, "Can't import non-8 bit sound"), (&bad, "Bad file format")] {
        import(&mut e, path, 0x0d);
        assert_eq!(prompt_text(&e), Some(format!("{}\n{msg}", path.display())));
        e.key_down(0x0d);
        assert_eq!(tape_samples(&e, g)[..4], [1, 2, 3, 0x80]);
    }
    // And Cancel keeps it.
    click_control(&mut e, 2);
    assert_eq!(tape_samples(&e, g)[..4], [1, 2, 3, 0x80]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn label_ok_measures_the_new_text() {
    let Some(mut e) = editor() else { return };
    let g = add(&mut e, *b"TxLb", 100, 200);
    let before = e.doc.gadgets[g].core.bounds;
    e.open_settings(g);
    click_control(&mut e, 106);
    for _ in 0..10 {
        e.key_down(u16::from(b'W'));
    }
    // Clear Text, then the text again; then OK.
    click_control(&mut e, 109);
    for _ in 0..20 {
        e.key_down(u16::from(b'W'));
    }
    click_control(&mut e, 1);
    assert!(e.dialog.is_none());
    let after = e.doc.gadgets[g].core.bounds;
    if ui::font(&e.assets).is_some() {
        assert!(
            after.right - after.left > before.right - before.left,
            "{before:?} {after:?}"
        );
    }
    assert_eq!(e.doc.gadgets[g].core.settings.label(0), b"wwwwwwwwwwwwwwwwwwww");
}

#[test]
fn sound_switch_hears_the_microphone() {
    let Some(mut e) = editor() else { return };
    let g = add(&mut e, *b"SdSw", 100, 200);
    let mic = FakeMic::new([Ok(vec![0x80; 1000]), Ok(vec![0xff; 200])]);
    let stops = mic.stops.clone();
    e.mic = Box::new(mic);
    e.run();
    assert!(e.take_sounds().contains(&SoundCmd::StopAll));
    e.update(20, (0, 0, 0));
    assert!(!e.doc.gadgets[g].core.state);
    for _ in 0..3 {
        e.update(20, (0, 0, 0));
    }
    assert!(e.doc.gadgets[g].core.state && !e.doc.gadgets[g].core.broken);
    // Done listening: the microphone closed.
    assert_eq!(stops.get(), 1);
    e.stop();
    assert!(!e.env.listen.on);
    // Without one: broken, then on.
    e.mic = Box::new(NoMic);
    e.run();
    e.update(20, (0, 0, 0));
    assert!(e.doc.gadgets[g].core.broken && !e.doc.gadgets[g].core.state);
    e.update(20, (0, 0, 0));
    assert!(e.doc.gadgets[g].core.state);
    e.stop();
}
