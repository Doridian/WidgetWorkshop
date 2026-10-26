//! Parses everything the port reads from the OS/2 Warp CD's game folder
//! (`WIDGET/`, extracted), its modules turned into the Win95 build's:
//! `cargo run -p formats --example check_os2 DIR`.

use std::path::PathBuf;

use formats::{cursor, dialog, hr, lx, os2, pe, pzl, tables, wgt};

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("orig-os2/WIDGET"), PathBuf::from);
    let read = |p: PathBuf| std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let module = |f: &str| {
        let raw = lx::Resources::parse(&read(dir.join(f))).unwrap_or_else(|e| panic!("{f}: {e}"));
        let types: std::collections::BTreeSet<u32> = raw.entries.keys().map(|(t, _)| *t).collect();
        println!("{f}: {} resources, types {types:?}", raw.entries.len());
        os2::to_win(&raw)
    };
    let exe = module("WIDGET.EXE");
    let gad = module("WWGADGET.DLL");
    let snd = module("WWSOUND.DLL");
    let intro = module("WWINTRO.DLL");

    let mut frames = 0;
    for (name, res) in [("WWGADGET", &gad), ("WWINTRO", &intro)] {
        for id in res.ids(1004).collect::<Vec<_>>() {
            let sheet = hr::Sheet::parse(res.get(1004, id).unwrap()).unwrap();
            for n in 1..=u16::try_from(sheet.len()).unwrap() {
                sheet
                    .decode(n)
                    .unwrap_or_else(|e| panic!("{name} sheet {id} frame {n}: {e}"));
                frames += 1;
            }
        }
    }
    println!("{frames} frames decoded");
    let waves = snd
        .ids(1006)
        .filter(|&id| tables::Wave::parse(snd.get(1006, id).unwrap()).is_some())
        .count();
    println!("{waves} of {} sounds parsed", snd.ids(1006).count());

    let mut dialogs = 0;
    for id in exe.ids(pe::RT_DIALOG).collect::<Vec<_>>() {
        dialog::Dialog::parse(exe.get(pe::RT_DIALOG, id).unwrap()).unwrap_or_else(|e| panic!("dialog {id}: {e}"));
        dialogs += 1;
    }
    println!("{dialogs} dialogs converted");
    for id in exe.ids(pe::RT_MENU).collect::<Vec<_>>() {
        let m = dialog::parse_menu(exe.get(pe::RT_MENU, id).unwrap()).unwrap();
        println!("menu {id}: {m:?}");
    }
    for id in 5007..=5013 {
        let group = exe.get(pe::RT_GROUP_CURSOR, id).unwrap();
        let c = cursor::group_first(group).unwrap();
        let c = cursor::Cursor::parse(exe.get(pe::RT_CURSOR, u32::from(c)).unwrap()).unwrap();
        println!("cursor {id}: {}x{} hot {:?}", c.width, c.height, c.hot);
    }
    for id in [1, 7, 100, 157, 251, 501, 8001, 8002, 8008] {
        println!("string {id}: {:?}", exe.string(id));
    }

    let p = pzl::Puzzles::parse(&read(dir.join("WIDGET.PZL"))).unwrap();
    println!(
        "WIDGET.PZL: {} puzzles, {} marked solved",
        p.puzzles.len(),
        p.done.iter().filter(|&&d| d).count()
    );

    for e in std::fs::read_dir(dir.join("WIDGETS")).unwrap().flatten() {
        let b = read(e.path());
        if let Ok(form) = wgt::Form::parse(&b) {
            let v = form.version().unwrap_or(0);
            println!("{}: v{v} {} chunks", e.file_name().display(), form.chunks.len());
        }
    }
}
