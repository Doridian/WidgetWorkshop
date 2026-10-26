//! The File menu through its dialogs, with the CD's resources (extracted
//! to `orig`; skipped without): Open and Save As, the CD's samples, Fit
//! Work Area's remembered size, the save's Mac size check, stand-alone
//! widgets.

use std::path::{Path, PathBuf};

use super::file::{FIT_KEY, FIT_SIZE_KEY, HOME_KEY, MAC};
use super::menu::cmd;
use super::*;
use crate::assets::Dir;
use crate::files;
use crate::gadget::Tag;

fn editor() -> Option<Editor> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../orig");
    if !root.exists() {
        eprintln!("no orig: skipped");
        return None;
    }
    let assets = Assets::load(Some(&Dir(root)), None).ok()?;
    Some(Editor::new(assets, Document::new(), 800, 600))
}

fn temp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ww-file-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn add(e: &mut Editor, tag: [u8; 4], h: i32, v: i32) -> usize {
    let id = e.doc.new_id();
    let g = doc::new_gadget(&e.assets, Tag(tag), id, h, v);
    e.doc.gadgets.push(g);
    e.doc.gadgets.len() - 1
}

/// Types `name` into the open file dialog and presses Enter.
fn choose(e: &mut Editor, name: &Path) {
    e.file_dialog.as_mut().unwrap().0.name = name.display().to_string();
    e.key_down(0x0d);
}

/// Every part's place inside `(14, 14)-(w − 14, h − 14)`: a squeeze moves
/// the parts, their sizes stay (`0x429060`).
fn placed_within(e: &Editor, (w, h): (i32, i32)) -> bool {
    e.doc
        .gadgets
        .iter()
        .all(|g| (14..=w - 14).contains(&g.core.h) && (14..=h - 14).contains(&g.core.v))
}

fn click_control(e: &mut Editor, id: u16) {
    let (x, y) = e.dialog.as_ref().unwrap().centre_of(id).unwrap();
    e.mouse_move(x, y);
    e.mouse_down(x, y, Button::Left);
    e.mouse_up(x, y, Button::Left);
    e.update(600, (0, 0, 0));
}

#[test]
fn open_and_save_through_the_dialogs() {
    let Some(mut e) = editor() else { return };
    if e.assets.samples.is_empty() {
        return;
    }
    let dir = temp("open");
    // The first dialog starts in `WidgetHome` (its separator dropped).
    e.profile.set(HOME_KEY, &format!("{}/", dir.display()));
    e.command(cmd::OPEN);
    let (d, then) = e.file_dialog.as_ref().unwrap();
    assert_eq!(
        (d.dir.as_path(), d.name.as_str(), *then),
        (dir.as_path(), "", Then::OpenPath)
    );
    // The samples are in the CD's folder.
    let clock = PathBuf::from(files::CD).join(files::SAMPLES).join("CLOCK.WGT");
    choose(&mut e, &clock);
    assert!(e.file_dialog.is_none() && e.prompt.is_none());
    assert_eq!(e.path.as_deref(), Some(clock.display().to_string().as_str()));
    assert!(!e.doc.gadgets.is_empty());
    // Save As starts where the widget is, with its name.
    e.command(cmd::SAVE_AS);
    let (d, _) = e.file_dialog.as_ref().unwrap();
    assert_eq!(
        (d.dir.as_path(), d.name.as_str()),
        (clock.parent().unwrap(), "CLOCK.WGT")
    );
    let mine = dir.join("mine");
    choose(&mut e, &mine);
    let saved = dir.join("mine.wgt");
    assert_eq!(e.path.as_deref(), Some(saved.display().to_string().as_str()));
    assert_eq!(
        Document::load(&e.assets, &std::fs::read(&saved).unwrap())
            .unwrap()
            .gadgets
            .len(),
        e.doc.gadgets.len()
    );
    // Remembered: the next dialog starts there.
    assert_eq!(e.file_dir.as_deref(), Some(dir.as_path()));
    e.command(cmd::NEW);
    e.command(cmd::OPEN);
    assert_eq!(e.file_dialog.as_ref().unwrap().0.dir, dir);
    // Cancel leaves everything.
    e.key_down(0x1b);
    assert!(e.file_dialog.is_none() && e.path.is_none());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn fit_size_is_remembered() {
    let Some(mut e) = editor() else { return };
    add(&mut e, *b"bulb", 100, 100);
    add(&mut e, *b"bulb", 1100, 900);
    e.profile.set(FIT_SIZE_KEY, "3");
    e.command(cmd::FIT);
    assert_eq!(e.dialog.as_ref().unwrap().fit_choice(), Some(3));
    // 720×540 chosen: Yes squeezes into it and keeps the choice.
    click_control(&mut e, 102);
    click_control(&mut e, 6);
    assert!(e.dialog.is_none());
    assert_eq!(e.profile.get(FIT_SIZE_KEY).as_deref(), Some("1"));
    assert!(placed_within(&e, (720, 540)));
    // Next time it's chosen; No changes nothing, Cancel neither.
    add(&mut e, *b"bulb", 1100, 900);
    e.command(cmd::FIT);
    assert_eq!(e.dialog.as_ref().unwrap().fit_choice(), Some(1));
    click_control(&mut e, 7);
    assert!(e.dialog.is_none());
    assert!(worksheet::extents(&e.doc).x1 > 1000);
    // A size past the five: none chosen, and Yes waits for one.
    e.profile.set(FIT_SIZE_KEY, "7");
    e.command(cmd::FIT);
    assert_eq!(e.dialog.as_ref().unwrap().fit_choice(), None);
    click_control(&mut e, 6);
    assert!(e.dialog.is_some());
    click_control(&mut e, 2);
    assert!(e.dialog.is_none());
    assert_eq!(e.profile.get(FIT_SIZE_KEY).as_deref(), Some("7"));
}

#[test]
fn saving_checks_the_mac_size() {
    let Some(mut e) = editor() else { return };
    let dir = temp("mac");
    add(&mut e, *b"bulb", 100, 100);
    let far = add(&mut e, *b"bulb", 760, 300);
    let path = dir.join("big.wgt");
    e.path = Some(path.display().to_string());
    e.dirty = true;
    // Too big: dialog 10010. Cancel saves nothing.
    e.command(cmd::SAVE);
    assert!(e.dialog.as_ref().is_some_and(Dialog::is_mac_fit));
    click_control(&mut e, 2);
    assert!(!path.exists() && e.dirty);
    assert_eq!(e.profile.get(FIT_KEY).as_deref(), Some("0"));
    // No: saved as it is.
    e.command(cmd::SAVE);
    click_control(&mut e, 7);
    assert!(path.exists() && !e.dirty);
    assert_eq!(e.doc.gadgets[far].core.h, 760);
    std::fs::remove_file(&path).unwrap();
    // Yes, remembered: squeezed into the Mac's screen, and from now on
    // without asking.
    e.dirty = true;
    e.command(cmd::SAVE);
    click_control(&mut e, 13);
    click_control(&mut e, 6);
    assert!(path.exists() && e.dialog.is_none());
    assert_eq!(e.profile.get(FIT_KEY).as_deref(), Some("2"));
    assert!(placed_within(&e, MAC));
    e.doc.gadgets[far].core.h = 900;
    e.command(cmd::SAVE_AS);
    assert!(e.dialog.is_none() && e.file_dialog.is_some());
    assert!(placed_within(&e, MAC));
    e.key_down(0x1b);
    // 1: never squeezed, never asked.
    e.profile.set(FIT_KEY, "1");
    e.doc.gadgets[far].core.h = 900;
    e.command(cmd::SAVE);
    assert!(e.dialog.is_none() && e.file_dialog.is_none());
    assert_eq!(e.doc.gadgets[far].core.h, 900);
    // A widget that fits is never asked about.
    e.profile.set(FIT_KEY, "0");
    e.doc.gadgets[far].core.h = 300;
    e.command(cmd::SAVE);
    assert!(e.dialog.is_none());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn stand_alone_widgets_round_trip() {
    let Some(mut e) = editor() else { return };
    if e.assets.runtime.is_none() {
        return;
    }
    let dir = temp("exe");
    add(&mut e, *b"bulb", 100, 100);
    add(&mut e, *b"Spkr", 200, 100);
    e.doc.background = 701;
    e.path = Some(dir.join("lamp.wgt").display().to_string());
    // Save Into is switched off (standalone::SAVE_ENABLED); go round it.
    assert!(!e.menu.enabled(cmd::SAVE_INTO));
    e.save_into();
    // The default name is the widget's with .EXE, in its folder.
    let (d, then) = e.file_dialog.as_ref().unwrap();
    assert_eq!(
        (d.dir.as_path(), d.name.as_str(), *then),
        (dir.as_path(), "lamp.EXE", Then::SaveInto)
    );
    e.key_down(0x0d);
    let exe = dir.join("lamp.EXE");
    let bytes = std::fs::read(&exe).unwrap();
    let res = formats::pe::Resources::parse(&bytes).unwrap();
    assert!(res.get(crate::assets::rt::WIDGET, 1).is_some());
    // The bulb's and the speaker's pictures and sounds, the background.
    assert!(res.get(1004, 1016).is_some() && res.get(1006, 220).is_some() && res.get(1004, 701).is_some());
    assert_eq!(
        e.path.as_deref(),
        Some(dir.join("lamp.wgt").display().to_string().as_str())
    );
    // Import reads it back, named .WGT beside it.
    e.command(cmd::NEW);
    e.command(cmd::IMPORT);
    choose(&mut e, &exe);
    assert_eq!(e.doc.gadgets.len(), 2);
    assert_eq!(e.doc.background, 701);
    assert_eq!(
        e.path.as_deref(),
        Some(dir.join("lamp.WGT").display().to_string().as_str())
    );
    // Revert reads the .EXE again.
    e.doc.gadgets.clear();
    e.dirty = true;
    e.command(cmd::REVERT);
    e.key_down(u16::from(b'Y'));
    assert_eq!(e.doc.gadgets.len(), 2);
    assert!(!e.dirty);
    // A file that isn't a program: alert 8.
    let txt = dir.join("x.exe");
    std::fs::write(&txt, b"not a program").unwrap();
    e.command(cmd::IMPORT);
    choose(&mut e, &txt);
    assert_eq!(e.prompt.as_ref().map(|(p, _)| p.text.clone()), e.assets.string(8008));
    std::fs::remove_dir_all(&dir).unwrap();
}
