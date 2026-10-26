//! Puzzle mode's bookkeeping (`cPuzzle`, docs/puzzles.md 2): which
//! puzzle is loaded, and the solved flags.
//!
//! The original keeps the flags in two places and ORs them on reading:
//! the `Done` resource of `WIDGET.PZL` (rewritten in place only when the
//! file is writable) and `Done=` in `%WINDIR%\WIDGET.INI`. The port reads
//! the puzzle file from the CD (or the overrides) and never writes it,
//! as the original does with a read-only install; the INI's part goes to
//! a [`Profile`], which the `widget` crate keeps as a `WIDGET.INI` in the
//! port's settings folder.

use std::collections::BTreeMap;

use formats::pzl::{self, Puzzles};

use crate::assets::Assets;

/// The profile section the game reads and writes (`0x4100e6`).
pub const SECTION: &str = "Widget Workshop";
/// The key of the solved flags (`0x42d82d`, `0x42d915`).
pub const DONE_KEY: &str = "Done";
/// The document's name while a puzzle is loaded (`0x42d5b3`, whatever
/// the number).
pub const DOC_NAME: &str = "puzzle";

/// The `[Widget Workshop]` section of the game's profile
/// (`GetPrivateProfileString`, `WritePrivateProfileString`).
pub trait Profile {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&mut self, key: &str, value: &str);
}

/// A profile that lives as long as the editor (the default, and tests').
#[derive(Debug, Clone, Default)]
pub struct MemoryProfile(pub BTreeMap<String, String>);

impl Profile for MemoryProfile {
    fn get(&self, key: &str) -> Option<String> {
        self.0.get(&key.to_ascii_lowercase()).cloned()
    }

    fn set(&mut self, key: &str, value: &str) {
        self.0.insert(key.to_ascii_lowercase(), value.to_owned());
    }
}

/// `cPuzzle`'s state: the puzzle's number (`+0x12`) and whether the
/// worksheet holds it (`+0x14`; New, Open, a save and Import clear it).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Current {
    pub number: i16,
    pub active: bool,
}

/// The puzzle file (`0x413e78`), or `None` where the game shows alert 5
/// ("Cannot find puzzle file...").
#[must_use]
pub fn open(assets: &Assets) -> Option<Puzzles> {
    Puzzles::parse(assets.puzzles.as_deref()?).ok()
}

/// The solved flags (`0x42d82d`): the `Done` resource's, with the
/// profile's added. `done[n - 1]` is puzzle `n`.
#[must_use]
pub fn read_solved(file: &Puzzles, profile: &dyn Profile) -> Vec<bool> {
    let mut done = file.done.clone();
    if let Some(s) = profile.get(DONE_KEY) {
        pzl::parse_done_ini(&s, &mut done);
    }
    done
}

/// Writes the solved flags (`0x42d915`): into the profile. The puzzle
/// file's `Done` resource stays as it is: the port treats the file as
/// read-only, as the original does on a CD.
pub fn write_solved(done: &[bool], profile: &mut dyn Profile) {
    if let Some(s) = pzl::format_done_ini(done) {
        profile.set(DONE_KEY, &s);
    }
}

/// `cPuzzle::MarkSolved` (`0x42d4b1`): nothing unless a puzzle is loaded
/// and its file can be read; else its flag set and written back. Returns
/// whether it marked one.
pub fn mark_solved(current: Current, file: Option<&Puzzles>, profile: &mut dyn Profile) -> bool {
    if !current.active {
        return false;
    }
    let Some(file) = file else { return false };
    let mut done = read_solved(file, profile);
    // The original stores at `flag[number - 1]` whatever the resource's
    // size; a number past it would write past the buffer. Dropped here.
    let Some(flag) = usize::try_from(current.number)
        .ok()
        .and_then(|n| n.checked_sub(1))
        .and_then(|i| done.get_mut(i))
    else {
        return false;
    };
    *flag = true;
    write_solved(&done, profile);
    true
}

/// "Reset Completed Puzzles" (`0x42d01e`, button 0x69): every flag
/// cleared in both stores (here: the profile).
pub fn reset_solved(file: &Puzzles, profile: &mut dyn Profile) {
    let done = vec![false; file.done.len()];
    write_solved(&done, profile);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(n: usize) -> Puzzles {
        Puzzles {
            puzzles: Vec::new(),
            done: vec![false; n],
        }
    }

    #[test]
    fn solved_flags_go_to_the_profile() {
        let f = file(25);
        let mut p = MemoryProfile::default();
        assert_eq!(read_solved(&f, &p), vec![false; 25]);
        // Not active: nothing.
        assert!(!mark_solved(
            Current {
                number: 8,
                active: false
            },
            Some(&f),
            &mut p
        ));
        assert_eq!(p.get("Done"), None);
        assert!(mark_solved(
            Current {
                number: 8,
                active: true
            },
            Some(&f),
            &mut p
        ));
        assert_eq!(p.get("Done").as_deref(), Some("0800000"));
        assert!(mark_solved(
            Current {
                number: 1,
                active: true
            },
            Some(&f),
            &mut p
        ));
        assert_eq!(p.get("DONE").as_deref(), Some("1800000"));
        let d = read_solved(&f, &p);
        assert!(d[0] && d[7] && d.iter().filter(|&&x| x).count() == 2);
        // No file, or a number out of range: nothing.
        assert!(!mark_solved(
            Current {
                number: 2,
                active: true
            },
            None,
            &mut p
        ));
        assert!(!mark_solved(
            Current {
                number: 26,
                active: true
            },
            Some(&f),
            &mut p
        ));
        assert!(!mark_solved(
            Current {
                number: 0,
                active: true
            },
            Some(&f),
            &mut p
        ));
        // The file's own flags count too (ORed), and reset clears the
        // profile's.
        let mut f2 = file(25);
        f2.done[24] = true;
        assert!(read_solved(&f2, &p)[24]);
        reset_solved(&f2, &mut p);
        assert_eq!(p.get("Done").as_deref(), Some("0000000"));
        assert_eq!(read_solved(&f, &p), vec![false; 25]);
    }
}
