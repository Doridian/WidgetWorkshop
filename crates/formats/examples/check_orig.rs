//! Parses everything the port reads from the extracted CD in `orig/`:
//! `cargo run -p formats --example check_orig [ORIG_DIR]`.

use std::path::PathBuf;

use formats::{dialog, fnt, hr, pe, tables, wgt};

fn main() {
    let orig = std::env::args().nth(1).map_or_else(|| PathBuf::from("orig"), PathBuf::from);
    let win = orig.join("WIN95/WIDGET");
    let read = |p: PathBuf| std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let exe = pe::Resources::parse(&read(win.join("WIDGET.EXE"))).unwrap();
    let gad = pe::Resources::parse(&read(win.join("WWGADGET.DLL"))).unwrap();
    let snd = pe::Resources::parse(&read(win.join("WWSOUND.DLL"))).unwrap();
    let intro = pe::Resources::parse(&read(win.join("WWINTRO.DLL"))).unwrap();

    let mut frames = 0;
    for (name, res) in [("WWGADGET", &gad), ("WWINTRO", &intro)] {
        for id in res.ids(1004).collect::<Vec<_>>() {
            let sheet = hr::Sheet::parse(res.get(1004, id).unwrap()).unwrap();
            for n in 1..=u16::try_from(sheet.len()).unwrap() {
                sheet.decode(n).unwrap_or_else(|e| panic!("{name} sheet {id} frame {n}: {e}"));
                frames += 1;
            }
        }
    }
    println!("{frames} frames decoded");

    let waves = snd.ids(1006).filter(|&id| tables::Wave::parse(snd.get(1006, id).unwrap()).is_some()).count();
    println!("{waves} of {} sounds parsed", snd.ids(1006).count());

    let mut dialogs = 0;
    for id in exe.ids(pe::RT_DIALOG).collect::<Vec<_>>() {
        dialog::Dialog::parse(exe.get(pe::RT_DIALOG, id).unwrap()).unwrap_or_else(|e| panic!("dialog {id}: {e}"));
        dialogs += 1;
    }
    println!("{dialogs} dialogs parsed");
    for id in exe.ids(pe::RT_MENU).collect::<Vec<_>>() {
        let m = dialog::parse_menu(exe.get(pe::RT_MENU, id).unwrap()).unwrap();
        println!("menu {id}: {m:?}");
    }
    let mut knots = 0;
    for id in gad.ids(1014).collect::<Vec<_>>() {
        knots += tables::knots(gad.get(1014, id).unwrap()).len();
    }
    println!("{knots} knots in {} gadgets", gad.ids(1014).count());
    for id in [1, 7, 157, 251, 501] {
        println!("string {id}: {:?}", exe.string(id));
    }

    let svga = orig.join("WIN31/SVGA");
    for f in ["SSERIFE.FON", "SMALLE.FON"] {
        if let Ok(b) = std::fs::read(svga.join(f)) {
            for font in fnt::Font::parse_fon(&b).unwrap() {
                println!("{f}: {} {}pt h{} w{}", font.face, font.points, font.height, font.weight);
            }
        }
    }

    for e in std::fs::read_dir(win.join("FUNWIDGE")).unwrap().flatten() {
        let b = read(e.path());
        if let Ok(form) = wgt::Form::parse(&b) {
            let v = form.version().unwrap_or(0);
            let tags: Vec<String> = form.chunks.iter().map(wgt::Chunk::tag_str).collect();
            let conns = form
                .chunks
                .iter()
                .filter(|c| &c.tag == b"conn")
                .inspect(|c| drop(wgt::Connection::parse(&c.data, v).unwrap()))
                .count();
            println!("{}: v{v} {} chunks, {conns} wires: {}", e.file_name().display(), tags.len(), tags.join(" "));
        }
    }
}
