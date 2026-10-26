//! Prints an OS/2 book's contents, text and pictures, and how the help
//! viewer gets it: `cargo run -p formats --example dump_inf FILE [BMP_DIR]`.
//! `FILE` is an `.INF`, or a CD image to read `WIDGET/WIDGET.INF` from;
//! with `BMP_DIR` the pictures are written there as BMPs.

use std::fmt::Write;

use formats::{inf, iso};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: dump_inf FILE [BMP_DIR]");
    let out = args.next();
    let data = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let data = if inf::is_inf(&data) {
        data
    } else {
        iso::read_file(&data, "WIDGET/WIDGET.INF").expect("no WIDGET/WIDGET.INF on the image")
    };
    let book = inf::InfFile::parse(&data).unwrap();
    println!(
        "{:?}: {} panels, {} slots, {} words, {} index entries, fonts {:?}",
        book.title,
        book.toc.len(),
        book.slots.len(),
        book.dictionary.len(),
        book.index.len(),
        book.fonts
    );
    for (i, e) in book.toc.iter().enumerate() {
        println!(
            "\n=== {i}: {}{} (level {}, slots {:?})",
            "  ".repeat(usize::from(e.level.saturating_sub(1))),
            e.title,
            e.level,
            e.slots
        );
        let mut line = String::new();
        for item in book.text(i).unwrap() {
            match item {
                inf::Item::Word(w) => line.push_str(&inf::oem(&w)),
                inf::Item::Space => line.push(' '),
                inf::Item::Paragraph => line.push_str("\n\n"),
                inf::Item::LineBreak => line.push('\n'),
                inf::Item::Centre => line.push_str("[centre]"),
                inf::Item::Escape(e) => {
                    if let inf::Escape::Bitmap { offset, .. } = e {
                        let b = book.bitmap(offset).unwrap();
                        let _ = write!(line, "[{}x{} picture]", b.width, b.height);
                        if let Some(dir) = &out {
                            std::fs::create_dir_all(dir).unwrap();
                            std::fs::write(format!("{dir}/{offset:08x}.bmp"), b.to_bmp()).unwrap();
                        }
                    }
                    let _ = write!(line, "{{{e:?}}}");
                }
            }
        }
        println!("{}", line.trim());
    }

    let help = book.to_help().unwrap();
    println!("\n--- as help topics ---");
    for t in &help.topics {
        println!(
            "{} {:?} browse {:?}/{:?}: {} paragraphs",
            t.offset,
            t.title,
            t.browse_prev,
            t.browse_next,
            t.scroll.len()
        );
        for p in &t.scroll {
            println!(
                "    [left {} gap {} tabs {:?} border {:?}] {:?}",
                p.format.left_indent,
                p.format.space_above,
                p.format.tabs.iter().map(|t| t.position).collect::<Vec<_>>(),
                p.format.border.map(|b| b.sides),
                p.plain_text()
            );
        }
    }
    for (i, f) in help.fonts.iter().enumerate() {
        println!("font {i}: {f:?}");
    }
}
