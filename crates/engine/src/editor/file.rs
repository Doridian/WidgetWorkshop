//! File ▸ Save and Save As with the save-time Mac size check, the common
//! file dialogs' plumbing (where they start, the folder remembered) and
//! the answers of the two squeeze questions, dialogs 10014 and 10010
//! (docs/editor-interaction.md 5.5, 8.4, 8.6).

use std::path::{Path, PathBuf};

use crate::files::{self, Fs};
use crate::gfx::IRect;

use super::dialog::{self, Dialog};
use super::file_dialog::{FileDialog, OPEN_WAV, Outcome, SAVE_WIDGET, Spec};
use super::prompt::Answer;
use super::{Editor, Next, Then, UNTITLED, worksheet};

/// The Mac version's largest screen (`0x42932b` squeezes into it less 14
/// pixels a side, `(14, 14)-(706, 526)`).
pub const MAC: (i32, i32) = (720, 540);
/// The profile's keys: dialog 10010's remembered answer (0 ask, 1 No, 2
/// Yes; `0x4935b0`), Fit Work Area's size (0..4, `0x4935b4`) and the
/// folder the dialogs first start in (`0x49147c`).
pub const FIT_KEY: &str = "Fit";
pub const FIT_SIZE_KEY: &str = "FitSize";
pub const HOME_KEY: &str = "WidgetHome";

/// What a save goes on to after the Mac check or the Fit dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Write the file, then `Next`.
    Save(Option<Next>),
    /// The Save As box, then `Next`.
    SaveAs(Option<Next>),
    /// Save Into a Stand-Alone Widget's box.
    Into,
}

/// `sscanf(s, "%ld")`: a whole number at the start, after blanks.
fn scan_long(s: &str) -> Option<i64> {
    let t = s.trim_start();
    let (neg, rest) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let v: i64 = digits.parse().ok()?;
    Some(if neg { -v } else { v })
}

impl Editor {
    /// The files the dialogs browse.
    pub(super) fn fs(&self) -> Fs<'_> {
        Fs {
            files: self.files.as_ref(),
            samples: &self.assets.samples,
        }
    }

    /// Profile number `key` (`0x41011b`: `"%ld"`, `default` when it's
    /// missing or not a number).
    pub(super) fn profile_int(&self, key: &str, default: i64) -> i64 {
        self.profile.get(key).and_then(|v| scan_long(&v)).unwrap_or(default)
    }

    /// Fit Work Area's size, as dialog 10014 starts (`0x429ad6`: the
    /// profile's `FitSize` as a `u16`, none chosen past 4).
    pub(super) fn fit_size_index(&self) -> Option<usize> {
        let v = self.profile_int(FIT_SIZE_KEY, 0).rem_euclid(0x1_0000);
        usize::try_from(v).ok().filter(|&i| i < dialog::FIT_SIZES.len())
    }

    /// The folder the widget dialogs start in (`DAT_00491076`): the last
    /// one chosen, else the profile's `WidgetHome` (`0x417594`, a trailing
    /// separator dropped), else the current folder.
    fn start_dir(&self) -> PathBuf {
        if let Some(d) = &self.file_dir {
            return d.clone();
        }
        if let Some(h) = self.profile.get(HOME_KEY).filter(|h| !h.is_empty()) {
            let t = h.trim_end_matches(['\\', '/']);
            return PathBuf::from(if t.is_empty() { h.as_str() } else { t });
        }
        self.files.current_dir()
    }

    /// A widget file dialog (`0x417160`, `0x41732c`) with `default` in
    /// the field; a default naming its folder starts there, which is
    /// remembered at once (Save As of a saved widget opens where it is).
    pub(super) fn ask_file(&mut self, spec: Spec, default: &str, then: Then) {
        let p = Path::new(default);
        let mut name = default.to_owned();
        if files::has_folder(p) {
            self.file_dir = p.parent().map(Path::to_path_buf);
            name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
        }
        let dir = self.start_dir();
        self.show_file_dialog(spec, &dir, &name, then);
    }

    /// The Recorder's wave file dialog (`0x43dd54`): from the folder the
    /// last wave came from (`DAT_004a2d90`), else where the widget
    /// dialogs are (the current folder they set).
    pub(super) fn ask_wav(&mut self, g: usize) {
        let dir = self.wav_dir.clone().unwrap_or_else(|| self.start_dir());
        self.show_file_dialog(OPEN_WAV, &dir, "", Then::ImportWav(g));
    }

    fn show_file_dialog(&mut self, spec: Spec, dir: &Path, name: &str, then: Then) {
        let area = self.surface.bounds();
        let d = FileDialog::new(&self.assets, self.fs(), spec, dir, name, area);
        self.file_dialog = Some((d, then));
    }

    /// The file dialog's state after an input: still open, or answered
    /// (`then` with the file, or cancelled). A chosen file's folder is
    /// remembered: the wave dialog's always, the others' when the name
    /// has its drive and folder (Borland `fnsplit` flags 0x18).
    fn file_outcome(&mut self, d: FileDialog, then: Then, o: Outcome) {
        match o {
            Outcome::Open => self.file_dialog = Some((d, then)),
            Outcome::Cancel => self.answer(Answer::Cancel, then, None),
            Outcome::Done(p) => {
                let parent = p.parent().map(Path::to_path_buf);
                if matches!(then, Then::ImportWav(_)) {
                    self.wav_dir = parent;
                } else if files::has_folder(&p) {
                    self.file_dir = parent;
                }
                self.answer(Answer::Ok, then, Some(p.display().to_string()));
            }
        }
    }

    pub(super) fn file_mouse_down(&mut self, p: (i32, i32), dbl: bool) {
        if let Some((mut d, then)) = self.file_dialog.take() {
            let o = d.mouse_down(&self.assets, self.fs(), p, dbl);
            self.file_outcome(d, then, o);
        }
    }

    pub(super) fn file_mouse_up(&mut self, p: (i32, i32)) {
        if let Some((mut d, then)) = self.file_dialog.take() {
            let o = d.mouse_up(&self.assets, self.fs(), p);
            self.file_outcome(d, then, o);
        }
    }

    pub(super) fn file_key(&mut self, vk: u16) {
        if let Some((mut d, then)) = self.file_dialog.take() {
            let o = d.key(&self.assets, self.fs(), vk, self.shift);
            self.file_outcome(d, then, o);
        }
    }

    /// File ▸ Save (`0x415a3b`): Save As without a file or with a puzzle
    /// loaded; else the Mac check, then the file. `next` follows.
    pub(super) fn save(&mut self, next: Option<Next>) {
        if self.path.is_none() || self.puzzle.active {
            self.save_as(next);
        } else {
            self.mac_check(Step::Save(next));
        }
    }

    /// File ▸ Save As (`0x415c1f`): the Mac check, then the box.
    pub(super) fn save_as(&mut self, next: Option<Next>) {
        self.mac_check(Step::SaveAs(next));
    }

    /// `0x42932b`, before every Save and Save As: a widget too big for the
    /// Mac's 720×540 (`FitParams` against `(14, 14)-(706, 526)`; an empty
    /// one never is) asks dialog 10010 when the profile's `Fit` is 0,
    /// squeezes without asking when it's 2, and is left as it is
    /// otherwise.
    fn mac_check(&mut self, step: Step) {
        let e = worksheet::extents(&self.doc);
        let t = IRect::new(14, 14, MAC.0 - 14, MAC.1 - 14);
        if e.is_empty() || worksheet::fit_params(e, t).is_none() {
            self.saving(step, true);
            return;
        }
        match self.profile_int(FIT_KEY, 0) {
            0 => {
                if let Some(d) = Dialog::mac_fit(&self.assets) {
                    self.dialog = Some(d);
                    self.fit_then = Some(step);
                } else {
                    self.message(
                        "This widget is too big to share with friends who have the Macintosh version of Widget \
                         Workshop. Do you want to squeeze the widget's parts together so you can share?",
                        &[Answer::Yes, Answer::No, Answer::Cancel],
                        Then::MacFit(step),
                    );
                }
            }
            2 => {
                self.fit_to(MAC);
                self.saving(step, true);
            }
            _ => self.saving(step, true),
        }
    }

    /// `0x429591`, Save Into's squeeze question: dialog 10014 when some
    /// part is outside `(15, 15)-(625, 465)`, else straight on. Fit Work
    /// Area (`step` `None`) asks it only after its own tests.
    pub(super) fn fit_dialog(&mut self, step: Option<Step>) {
        let e = worksheet::extents(&self.doc);
        if step.is_some() && (e.is_empty() || e.x0 >= 15 && e.y0 >= 15 && e.x1 <= 625 && e.y1 <= 465) {
            if let Some(s) = step {
                self.saving(s, true);
            }
            return;
        }
        if let Some(d) = Dialog::fit(&self.assets, self.fit_size_index()) {
            self.dialog = Some(d);
            self.fit_then = step;
        } else {
            self.message(
                "Do you want to automatically squeeze the parts together so they'll all fit on your (or a \
                 smaller) monitor?",
                &[Answer::Yes, Answer::No, Answer::Cancel],
                Then::Fit(step),
            );
        }
    }

    /// Dialog 10014 or 10010 answered: `yes` / No (`Some`), Cancel
    /// (`None`). 10014's Yes keeps its size in `FitSize` (`0x4298b9`) and
    /// squeezes to it; 10010's Yes squeezes to the Mac's, and its "Remember
    /// my answer" makes `Fit` 2 for Yes, 1 for No (written each time, 0
    /// otherwise, `0x42932b`). Then the save goes on unless cancelled.
    pub(super) fn fit_answered(&mut self, mac: bool, answer: Option<bool>, size: Option<usize>, remember: bool) {
        let step = self.fit_then.take();
        if mac {
            let v = match answer {
                Some(true) if remember => 2,
                Some(false) if remember => 1,
                _ => 0,
            };
            self.profile.set(FIT_KEY, &v.to_string());
            if answer == Some(true) {
                self.fit_to(MAC);
            }
        } else if answer == Some(true)
            && let Some(i) = size
        {
            self.profile.set(FIT_SIZE_KEY, &i.to_string());
            self.fit_to(dialog::fit_size(i));
        }
        if let Some(s) = step {
            self.saving(s, answer.is_some());
        }
    }

    /// The save after the check; `go` false when it was cancelled.
    pub(super) fn saving(&mut self, step: Step, go: bool) {
        match step {
            Step::Save(next) => {
                let ok = go && self.path.clone().is_some_and(|p| self.write(&p));
                self.saved(next, ok);
            }
            Step::SaveAs(next) if go => {
                let name = if self.puzzle.active {
                    crate::puzzle::DOC_NAME.to_owned()
                } else {
                    self.path.clone().unwrap_or_else(|| UNTITLED.to_owned())
                };
                self.ask_file(SAVE_WIDGET, &name, Then::SaveAsPath(next));
            }
            Step::SaveAs(next) => self.saved(next, false),
            Step::Into if go => self.save_into_ask(),
            Step::Into => {}
        }
    }

    /// What follows a save: `next` when it worked; Load Puzzle and Import
    /// go on whatever it did (`0x416f1a`, `0x416bf5`), unless a message is
    /// up.
    pub(super) fn saved(&mut self, next: Option<Next>, ok: bool) {
        if let Some(n) = next
            && (ok || matches!(n, Next::LoadPuzzle | Next::Import) && self.prompt.is_none())
        {
            self.after_save(n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::scan_long;

    #[test]
    fn profile_numbers() {
        assert_eq!(scan_long(" 3"), Some(3));
        assert_eq!(scan_long("-1x"), Some(-1));
        assert_eq!(scan_long("x"), None);
        assert_eq!(scan_long(""), None);
    }
}
