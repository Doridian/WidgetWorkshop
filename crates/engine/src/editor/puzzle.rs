//! Puzzle mode in the editor (docs/puzzles.md 2): File ▸ Load Puzzle...
//! and its dialog, loading and reverting a puzzle, and marking it solved.
//! The locks and tray limits a puzzle brings are the editor's own rules
//! for any widget (docs/puzzles.md 2.6, 3).

use crate::doc::Document;
use crate::puzzle::{self, Current};

use super::prompt::Answer;
use super::puzzle_dialog::{self, Outcome, PuzzleDialog};
use super::tray::Tool;
use super::worksheet::Sel;
use super::{Editor, Then};

impl Editor {
    /// The puzzle loaded, if the worksheet holds one (`cPuzzle +0x12`,
    /// `+0x14`).
    #[must_use]
    pub fn puzzle(&self) -> Option<i16> {
        self.puzzle.active.then_some(self.puzzle.number)
    }

    /// `cPuzzle::Choose` (`0x42d570` → `0x42d284`): the dialog, or alert 5
    /// without a puzzle file.
    pub(super) fn choose_puzzle(&mut self) {
        let Some(file) = puzzle::open(&self.assets) else {
            self.alert(5);
            return;
        };
        let done = puzzle::read_solved(&file, self.profile.as_ref());
        // Line `i` is puzzle `i + 1`, named by its ID (`0x414781`; a
        // missing one has no name), `CountResources('Puzl')` of them.
        let lines = (1..=file.puzzles.len())
            .map(|n| {
                let title = file
                    .puzzles
                    .iter()
                    .find(|p| usize::try_from(p.id).ok() == Some(n))
                    .map(formats::pzl::Puzzle::title)
                    .unwrap_or_default();
                puzzle_dialog::line(&title, done.get(n - 1).copied().unwrap_or(false))
            })
            .collect();
        match PuzzleDialog::new(&self.assets, lines) {
            Some(d) => self.puzzle_dialog = Some(d),
            None => self.alert(5),
        }
    }

    /// What the dialog's input led to.
    pub(super) fn puzzle_outcome(&mut self, o: Outcome) {
        match o {
            Outcome::Open => {}
            Outcome::Cancel => self.puzzle_dialog = None,
            Outcome::Ok(line) => {
                self.puzzle_dialog = None;
                if let Ok(n) = i16::try_from(line + 1) {
                    self.load_puzzle(n);
                }
            }
            Outcome::Reset => self.message(
                puzzle_dialog::RESET_QUESTION,
                &[Answer::Ok, Answer::Cancel],
                Then::ResetPuzzles,
            ),
        }
    }

    /// The reset question's answer: on OK both stores cleared, the marks
    /// gone, the button disabled.
    pub(super) fn reset_puzzles(&mut self, a: Answer) {
        if a != Answer::Ok {
            return;
        }
        if let Some(file) = puzzle::open(&self.assets) {
            puzzle::reset_solved(&file, self.profile.as_mut());
        }
        if let Some(d) = &mut self.puzzle_dialog {
            d.clear_marks();
        }
    }

    /// The puzzle's widget, or alert 5 (no file) / 7 (not a widget).
    fn puzzle_doc(&mut self, number: i16) -> Option<Document> {
        let Some(file) = puzzle::open(&self.assets) else {
            self.alert(5);
            return None;
        };
        let doc = file
            .puzzles
            .iter()
            .find(|p| p.id == number)
            .and_then(|p| Document::load(&self.assets, &p.widget).ok());
        if doc.is_none() {
            self.alert(7);
        }
        doc
    }

    /// Puts puzzle `number`'s widget on the worksheet (`0x416f1a`): the
    /// tray's counts reset then set from its `numG`, the Pointer. Unlike
    /// Open it keeps the undo step and the modified flag, as the original.
    pub fn load_puzzle(&mut self, number: i16) -> bool {
        let Some(doc) = self.puzzle_doc(number) else {
            return false;
        };
        self.puzzle = Current { number, active: true };
        self.put_puzzle(doc);
        true
    }

    /// Revert for a puzzle (`0x415ece` → `0x42d530`): reloaded, active
    /// again, not modified.
    pub(super) fn revert_puzzle(&mut self) {
        let number = self.puzzle.number;
        if let Some(doc) = self.puzzle_doc(number) {
            self.puzzle.active = true;
            self.put_puzzle(doc);
            self.dirty = false;
        }
    }

    fn put_puzzle(&mut self, doc: Document) {
        self.stop();
        self.sel = Sel::None;
        self.doc = doc;
        self.tray.reset_counts();
        let limits = self.doc.tray_limits.clone();
        self.tray.apply_limits(&limits);
        self.tray.select_tool(Tool::Pointer);
        // The title says "puzzle" at once (`0x417092`).
        self.update_menu();
    }

    /// A Win / Lose Checker set to Win ended the run: `cPuzzle::MarkSolved`
    /// (`0x42d4b1`), nothing unless a puzzle is loaded.
    pub(super) fn mark_solved(&mut self) {
        let file = puzzle::open(&self.assets);
        puzzle::mark_solved(self.puzzle, file.as_ref(), self.profile.as_mut());
    }

    /// Save As while a puzzle is loaded (`0x415c1f`), once a name is
    /// chosen: unless Shift is held, every gadget and wire is unlocked and
    /// the tray unlimited, so the file is an ordinary widget.
    pub(super) fn unlock_for_save(&mut self) {
        if !self.puzzle.active || self.shift {
            return;
        }
        for g in &mut self.doc.gadgets {
            g.core.locked = false;
        }
        for w in &mut self.doc.wires {
            w.flag = 0;
        }
        self.tray.reset_counts();
    }
}

#[cfg(test)]
mod tests {
    use super::super::menu::{BAR_H, cmd};
    use super::super::tray;
    use super::super::{Button, Editor};
    use super::*;
    use crate::assets::{Assets, Dir};
    use crate::gadget::Tag;
    use crate::puzzle::{MemoryProfile, Profile};

    /// The editor on the CD's files (extracted to `orig`; skipped without).
    fn editor() -> Option<Editor> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../orig");
        if !root.exists() {
            eprintln!("no orig: skipped");
            return None;
        }
        let assets = Assets::load(Some(&Dir(root)), None).ok()?;
        Some(Editor::new(assets, Document::new(), 800, 600))
    }

    fn centre(e: &Editor, g: usize) -> (i32, i32) {
        let r = e.doc.gadgets[g].core.rect();
        (i32::midpoint(r.x0, r.x1), i32::midpoint(r.y0, r.y1) + BAR_H)
    }

    fn click(e: &mut Editor, p: (i32, i32)) {
        e.mouse_move(p.0, p.1);
        e.mouse_down(p.0, p.1, Button::Left);
        e.mouse_up(p.0, p.1, Button::Left);
        // Past the double-click time.
        e.update(600, (0, 0, 0));
    }

    fn drag(e: &mut Editor, from: (i32, i32), to: (i32, i32)) {
        e.mouse_move(from.0, from.1);
        e.mouse_down(from.0, from.1, Button::Left);
        e.mouse_move(to.0, to.1);
        e.mouse_up(to.0, to.1, Button::Left);
        e.update(600, (0, 0, 0));
    }

    fn count(e: &Editor, tag: [u8; 4]) -> i16 {
        e.tray.counts[usize::from(tray::index_of(Tag(tag)).unwrap())]
    }

    fn find(e: &Editor, tag: [u8; 4]) -> Vec<usize> {
        (0..e.doc.gadgets.len())
            .filter(|&i| e.doc.gadgets[i].core.tag == Tag(tag))
            .collect()
    }

    #[test]
    fn every_puzzle_loads() {
        let Some(e) = editor() else { return };
        let file = puzzle::open(&e.assets).unwrap();
        assert_eq!(file.puzzles.len(), 25);
        // Gadgets (locked), wires (locked): docs/puzzles.md 1.7.
        let table: [(usize, usize, usize, usize); 25] = [
            (17, 17, 5, 5),
            (14, 14, 10, 10),
            (21, 21, 11, 10),
            (16, 16, 11, 11),
            (43, 28, 38, 22),
            (34, 34, 29, 29),
            (33, 33, 28, 28),
            (12, 8, 2, 2),
            (20, 20, 9, 9),
            (18, 18, 8, 8),
            (23, 23, 20, 20),
            (57, 57, 45, 45),
            (22, 22, 7, 6),
            (44, 43, 22, 21),
            (28, 28, 14, 14),
            (19, 19, 9, 9),
            (22, 22, 10, 10),
            (17, 14, 1, 1),
            (11, 10, 3, 3),
            (27, 27, 21, 21),
            (45, 20, 39, 27),
            (16, 16, 6, 6),
            (14, 14, 5, 5),
            (36, 36, 3, 3),
            (33, 33, 20, 20),
        ];
        for (p, &(g, gl, w, wl)) in file.puzzles.iter().zip(&table) {
            let d = Document::load(&e.assets, &p.widget).unwrap();
            let n = p.id;
            assert_eq!(d.gadgets.len(), g, "puzzle {n}");
            assert_eq!(d.gadgets.iter().filter(|g| g.core.locked).count(), gl, "puzzle {n}");
            assert_eq!(d.wires.len(), w, "puzzle {n}");
            assert_eq!(d.wires.iter().filter(|w| w.flag != 0).count(), wl, "puzzle {n}");
            assert_eq!(d.gadgets.iter().filter(|g| g.core.tag == Tag(*b"Stop")).count(), 1);
            assert_eq!(d.tray_limits.len(), 64, "puzzle {n}");
            // Every checker is set to Win.
            let stop = d.gadgets.iter().find(|g| g.core.tag == Tag(*b"Stop")).unwrap();
            assert_eq!(stop.core.settings.radios[3], 0, "puzzle {n}");
        }
    }

    #[test]
    fn the_dialog_lists_marks_and_resets() {
        let Some(mut e) = editor() else { return };
        let mut p = MemoryProfile::default();
        p.set("Done", "0800000");
        e.profile = Box::new(p);
        e.command(cmd::LOAD_PUZZLE);
        let d = e.puzzle_dialog.as_ref().unwrap();
        assert_eq!(d.lines.len(), 25);
        assert_eq!(d.lines[0], "  White Color");
        assert_eq!(d.lines[7], "+ Count To Ten!");
        assert_eq!(d.lines[6], "  Doc's Diabolical Adder");
        assert_eq!(d.sel, Some(0));
        assert!(d.enabled(0x69));
        // Reset asks first (OK / Cancel); Cancel keeps the marks.
        e.puzzle_outcome(Outcome::Reset);
        assert!(e.prompt.is_some());
        e.key_down(0x1b);
        assert!(e.prompt.is_none());
        assert_eq!(e.profile.get("Done").as_deref(), Some("0800000"));
        e.puzzle_outcome(Outcome::Reset);
        e.key_down(0x0d);
        assert_eq!(e.profile.get("Done").as_deref(), Some("0000000"));
        let d = e.puzzle_dialog.as_ref().unwrap();
        assert_eq!(d.lines[7], "  Count To Ten!");
        assert!(!d.enabled(0x69));
        // End scrolls to the last line, Home back.
        e.key_down(0x23);
        assert_eq!(e.puzzle_dialog.as_ref().unwrap().sel, Some(24));
        e.key_down(0x24);
        assert_eq!(e.puzzle_dialog.as_ref().unwrap().sel, Some(0));
        // Down twice, Enter: puzzle 3.
        e.key_down(0x28);
        e.key_down(0x28);
        e.key_down(0x0d);
        assert!(e.puzzle_dialog.is_none());
        assert_eq!(e.puzzle(), Some(3));
        assert_eq!(e.title(), "Widget Workshop - puzzle");
        // Esc cancels: nothing changes.
        e.command(cmd::LOAD_PUZZLE);
        e.key_down(0x1b);
        assert!(e.puzzle_dialog.is_none());
        assert_eq!(e.puzzle(), Some(3));
    }

    #[test]
    fn the_tray_hands_out_what_the_puzzle_allows() {
        let Some(mut e) = editor() else { return };
        assert!(e.load_puzzle(2));
        assert_eq!(count(&e, *b"AndG"), 1);
        assert_eq!(count(&e, *b"XorG"), 1);
        assert_eq!(count(&e, *b"swit"), 4);
        assert_eq!(count(&e, *b"bulb"), 0);
        // A part at 0 can't be taken; one with a count can, down to 0.
        let slot = |e: &Editor, tag: &[u8; 4]| {
            let i = tray::index_of(Tag(*tag)).unwrap();
            let f = e
                .tray
                .folders
                .iter()
                .position(|f| f.iter().flatten().any(|s| s.index == i))
                .unwrap();
            (f, i)
        };
        let n = e.doc.gadgets.len();
        // The bulb is on the first folder's first page.
        let (f, i) = slot(&e, b"bulb");
        assert_eq!(f, 0);
        let s = e.tray.page().iter().find(|s| s.index == i).copied().unwrap();
        let at = (
            e.tray.bar.pos.0 + tray::AREA.x0 + s.rect.x0 + 2,
            e.tray.bar.pos.1 + tray::AREA.y0 + s.rect.y0 + 2 + BAR_H,
        );
        drag(&mut e, at, (600, 500));
        assert_eq!(e.doc.gadgets.len(), n);
        e.tray.counts[usize::from(i)] = 1;
        drag(&mut e, at, (600, 500));
        assert_eq!(e.doc.gadgets.len(), n + 1);
        assert_eq!(e.tray.counts[usize::from(i)], 0);
        // Deleted, it goes back.
        let b = e.doc.gadgets.len() - 1;
        assert!(!e.doc.gadgets[b].core.locked);
        let c = centre(&e, b);
        e.mouse_move(c.0, c.1);
        e.update(16, (0, 0, 0));
        e.key_down(0x2e);
        assert_eq!(e.tray.counts[usize::from(i)], 1);
        // A new widget unlimits the tray.
        e.command(cmd::NEW);
        assert_eq!(e.puzzle(), None);
        assert_eq!(count(&e, *b"bulb"), -1);
    }

    #[test]
    fn locks_block_editing() {
        let Some(mut e) = editor() else { return };
        // Puzzle 2: every gadget and wire locked.
        assert!(e.load_puzzle(2));
        let before = e.doc.save();
        let xor = find(&e, *b"XorG")[0];
        let id = e.doc.gadgets[xor].core.id;
        let c = centre(&e, xor);
        let (h, v) = (e.doc.gadgets[xor].core.h, e.doc.gadgets[xor].core.v);
        // Not selected on hover; can't be dragged.
        e.mouse_move(c.0, c.1);
        e.update(16, (0, 0, 0));
        assert_eq!(e.sel, Sel::None);
        drag(&mut e, c, (c.0 + 40, c.1 + 40));
        let xor = by_id(&e, id);
        assert_eq!((e.doc.gadgets[xor].core.h, e.doc.gadgets[xor].core.v), (h, v));
        // Nor opened.
        e.mouse_move(c.0, c.1);
        e.mouse_down(c.0, c.1, Button::Left);
        e.mouse_up(c.0, c.1, Button::Left);
        e.mouse_down(c.0, c.1, Button::Left);
        e.mouse_up(c.0, c.1, Button::Left);
        assert!(e.dialog.is_none());
        e.update(600, (0, 0, 0));
        // Nor deleted: Delete, the Cut tool, Clear Work Area.
        e.key_down(0x2e);
        e.key_down(0x10);
        click(&mut e, c);
        e.key_up(0x10);
        e.command(cmd::CLEAR);
        e.command(cmd::REMOVE_WIRES);
        e.command(cmd::REMOVE_JOINTS);
        assert_eq!(e.doc.gadgets.len(), 14);
        assert_eq!(e.doc.wires.len(), 10);
        // A locked wire: its end can't be picked up from its knot, and a
        // click on it adds no joint.
        let w = (0..e.doc.wires.len()).find(|&w| e.doc.wires[w].flag != 0).unwrap();
        let wire = e.doc.wires[w].clone();
        let k = e.doc.gadgets[wire.to].core.knot_at(wire.to_knot).unwrap();
        drag(&mut e, (k.0, k.1 + BAR_H), (700, 550));
        assert_eq!(e.doc.wires.len(), 10);
        assert_eq!(e.doc.wires[w].points, wire.points);
        // A free knot of a locked gadget still takes a new wire.
        let and = find(&e, *b"AndG");
        let (g, free) = and
            .iter()
            .find_map(|&g| {
                let core = &e.doc.gadgets[g].core;
                (0..core.knots.len())
                    .find(|&k| core.knots[k].wire.is_none())
                    .map(|k| (g, k))
            })
            .unwrap();
        let output = !e.doc.gadgets[g].core.knots[free].is_input();
        let target = (0..e.doc.gadgets.len()).find_map(|t| {
            let core = &e.doc.gadgets[t].core;
            (t != g)
                .then(|| {
                    (0..core.knots.len()).find(|&k| core.knots[k].wire.is_none() && core.knots[k].is_input() == output)
                })
                .flatten()
                .map(|k| (t, k))
        });
        let from = e.doc.gadgets[g].core.knot_at(free).unwrap();
        if let Some((t, tk)) = target {
            let to = e.doc.gadgets[t].core.knot_at(tk).unwrap();
            drag(&mut e, (from.0, from.1 + BAR_H), (to.0, to.1 + BAR_H));
            assert_eq!(e.doc.wires.len(), 11);
            // The new wire isn't locked: it can be cut.
            assert_eq!(e.doc.wires.iter().filter(|w| w.flag == 0).count(), 1);
            e.command(cmd::REMOVE_WIRES);
            assert_eq!(e.doc.wires.len(), 10);
        }
        // In run mode a locked part works as usual.
        let _ = before;
    }

    /// The index of the gadget with id `id`.
    fn by_id(e: &Editor, id: u32) -> usize {
        e.doc.gadgets.iter().position(|g| g.core.id == id).unwrap()
    }

    /// Drags a new wire from free knot `k` of gadget `from` (by id) onto
    /// gadget `to`, where it snaps to the nearest free knot.
    fn wire(e: &mut Editor, from: u32, k: usize, to: u32) {
        let n = e.doc.wires.len();
        let g = by_id(e, from);
        let a = e.doc.gadgets[g].core.knot_at(k).unwrap();
        let b = centre(e, by_id(e, to));
        drag(e, (a.0, a.1 + BAR_H), b);
        assert_eq!(e.doc.wires.len(), n + 1);
    }

    /// The first free knot of gadget `id` that is an input (or an output).
    fn free_knot(e: &Editor, id: u32, input: bool) -> usize {
        let core = &e.doc.gadgets[by_id(e, id)].core;
        (0..core.knots.len())
            .find(|&k| core.knots[k].wire.is_none() && core.knots[k].is_input() == input)
            .unwrap()
    }

    #[test]
    fn a_win_marks_the_puzzle_solved() {
        let Some(mut e) = editor() else { return };
        // Puzzle 1, "White Color": every part locked, the switches not
        // wired. Wires go from them to the splitters and from those to the
        // AND gates (free knots of locked parts), then the three colours
        // switched on make the checker win.
        assert!(e.load_puzzle(1));
        let ids = |e: &Editor, tag: &[u8; 4]| -> Vec<u32> {
            find(e, *tag).iter().map(|&g| e.doc.gadgets[g].core.id).collect()
        };
        let (switches, splitters, ands) = (ids(&e, b"swit"), ids(&e, b"Spl2"), ids(&e, b"AndG"));
        assert!(e.doc.gadgets.iter().all(|g| g.core.locked));
        for (&sw, &sp) in switches.iter().zip(&splitters) {
            let k = free_knot(&e, sw, false);
            wire(&mut e, sw, k, sp);
        }
        for &sp in &splitters {
            let and = *ands
                .iter()
                .find(|&&a| {
                    e.doc.gadgets[by_id(&e, a)]
                        .core
                        .knots
                        .iter()
                        .any(|k| k.is_input() && k.wire.is_none())
                })
                .unwrap();
            let k = free_knot(&e, sp, false);
            wire(&mut e, sp, k, and);
        }
        assert_eq!(e.doc.wires.len(), 11);
        e.run();
        e.update(100, (0, 0, 0));
        for &sw in &switches {
            let c = centre(&e, by_id(&e, sw));
            e.mouse_down(c.0, c.1, Button::Left);
            e.mouse_up(c.0, c.1, Button::Left);
            e.update(100, (0, 0, 0));
        }
        let mut t = 0;
        while e.running && t < 200 {
            e.update(50, (0, 0, 0));
            t += 1;
        }
        assert!(!e.running, "the checker stops the run");
        assert_eq!(e.profile.get("Done").as_deref(), Some("1000000"));
        e.command(cmd::LOAD_PUZZLE);
        assert_eq!(e.puzzle_dialog.as_ref().unwrap().lines[0], "+ White Color");
    }

    #[test]
    fn saving_makes_an_ordinary_widget() {
        let Some(mut e) = editor() else { return };
        assert!(e.load_puzzle(4));
        e.dirty = true;
        // Save goes to Save As, named "puzzle".
        e.command(cmd::SAVE);
        let (d, _) = e.file_dialog.as_mut().unwrap();
        assert_eq!(d.name, "puzzle");
        let path = std::env::temp_dir().join(format!("widget-puzzle-{}.wgt", std::process::id()));
        d.name = path.display().to_string();
        e.key_down(0x0d);
        let saved = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(e.puzzle(), None);
        let d = Document::load(&e.assets, &saved).unwrap();
        assert!(d.gadgets.iter().all(|g| !g.core.locked));
        assert!(d.wires.iter().all(|w| w.flag == 0));
        assert_eq!(d.tray_limits, []);
        assert_eq!(count(&e, *b"swit"), -1);
        // A win now marks nothing.
        e.mark_solved();
        assert_eq!(e.profile.get("Done"), None);
    }

    #[test]
    fn puzzles_drop_changes_and_revert_to_the_original() {
        let Some(mut e) = editor() else { return };
        assert!(e.load_puzzle(8));
        let n = e.doc.gadgets.len();
        let free = (0..n).find(|&g| !e.doc.gadgets[g].core.locked).unwrap();
        let c = centre(&e, free);
        e.mouse_move(c.0, c.1);
        e.update(16, (0, 0, 0));
        e.key_down(0x2e);
        assert_eq!(e.doc.gadgets.len(), n - 1);
        assert!(e.dirty);
        e.update(16, (0, 0, 0));
        assert!(e.menu.enabled(cmd::REVERT));
        // "Revert to original puzzle?"
        e.command(cmd::REVERT);
        let text = e.assets.string(8002).unwrap();
        assert_eq!(e.prompt.as_ref().unwrap().0.text, text);
        e.key_down(u16::from(b'Y'));
        assert_eq!(e.doc.gadgets.len(), n);
        assert!(!e.dirty);
        assert_eq!(e.puzzle(), Some(8));
        // New drops a puzzle's changes without asking.
        e.mouse_move(c.0, c.1);
        e.update(16, (0, 0, 0));
        e.key_down(0x2e);
        assert!(e.dirty);
        e.command(cmd::NEW);
        assert!(e.prompt.is_none());
        assert!(e.doc.gadgets.is_empty());
        assert_eq!(e.puzzle(), None);
    }
}
