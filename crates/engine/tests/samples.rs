//! The CD's sample widgets load and save back byte for byte (with the CD
//! extracted to `orig`, as `tools/` leaves it; skipped without).

use engine::assets::{Assets, Dir};
use engine::doc::Document;

#[test]
fn samples_round_trip() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../orig");
    if !root.exists() {
        eprintln!("no orig: skipped");
        return;
    }
    let assets = Assets::load(Some(&Dir(root)), None).unwrap();
    assert!(assets.samples.iter().any(|(n, _)| n.eq_ignore_ascii_case("CLOCK.WGT")));
    for (name, bytes) in &assets.samples {
        let doc = Document::load(&assets, bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(&doc.save(), bytes, "{name}");
    }
}
