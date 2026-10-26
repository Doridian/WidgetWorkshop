//! The CD's puzzles (`WIDGET.PZL`): 25 of them, each a widget that loads and
//! saves back byte for byte (with the CD extracted to `orig`, as `tools/`
//! leaves it; skipped without).

use engine::assets::{Assets, Dir};
use engine::doc::Document;
use formats::pzl::Puzzles;

#[test]
fn puzzles_round_trip() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../orig");
    if !root.exists() {
        eprintln!("no orig: skipped");
        return;
    }
    let assets = Assets::load(Some(&Dir(root)), None).unwrap();
    let file = assets.puzzles.as_ref().expect("WIDGET.PZL");
    let puzzles = Puzzles::parse(file).unwrap();
    assert_eq!(puzzles.puzzles.len(), 25);
    let ids: Vec<i16> = puzzles.puzzles.iter().map(|p| p.id).collect();
    assert_eq!(ids, (1..=25).collect::<Vec<i16>>());
    assert_eq!(puzzles.done, vec![false; 25]);
    assert_eq!(puzzles.puzzles[7].title(), "Count To Ten!");
    for p in &puzzles.puzzles {
        let name = p.title();
        let doc = Document::load(&assets, &p.widget).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!doc.tray_limits.is_empty(), "{name}: no numG");
        let out = doc.save();
        if p.id == 17 {
            // "Gravity Chamber Test" was stored with a FORM size that leaves
            // out its trailing numG chunk (7856 + 8 + 0x182 = the real 8250);
            // the loader ignores the FORM size, and the game writes the true
            // one. Everything after the size matches.
            assert_eq!(&p.widget[4..8], &7848u32.to_be_bytes(), "{name}");
            assert_eq!(&out[4..8], &8242u32.to_be_bytes(), "{name}");
            assert_eq!(out[..4], p.widget[..4], "{name}");
            assert_eq!(out[8..], p.widget[8..], "{name}");
        } else {
            assert_eq!(out, p.widget, "{name}");
        }
    }
}
