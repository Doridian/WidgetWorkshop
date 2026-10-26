//! Windows profile files (`.INI`) as `GetPrivateProfileString` and
//! `WritePrivateProfileString` treat them: `[section]` headers and
//! `key=value` lines, both names matched without regard to case, `;`
//! comments. The game keeps its solved puzzles in one (`WIDGET.INI`,
//! `[Widget Workshop] Done=`, docs/puzzles.md 2.5).

/// The section a line opens, if it's a header.
fn header(line: &str) -> Option<&str> {
    let t = line.trim();
    t.strip_prefix('[')?.split_once(']').map(|(s, _)| s.trim())
}

/// The key of a `key=value` line (not a comment).
fn key_of(line: &str) -> Option<&str> {
    let t = line.trim_start();
    if t.starts_with(';') {
        return None;
    }
    t.split_once('=').map(|(k, _)| k.trim())
}

/// The value of `key` in `section` of `text`, trimmed; the first one if
/// there are several.
#[must_use]
pub fn get(text: &str, section: &str, key: &str) -> Option<String> {
    let mut inside = false;
    for line in text.lines() {
        if let Some(s) = header(line) {
            inside = s.eq_ignore_ascii_case(section);
        } else if inside
            && key_of(line).is_some_and(|k| k.eq_ignore_ascii_case(key))
            && let Some((_, v)) = line.split_once('=')
        {
            return Some(v.trim().to_owned());
        }
    }
    None
}

/// `text` with `key=value` in `section`: the key's first line replaced,
/// else a line added at the section's end, else the section added at the
/// end of the file. Every other line stays as it is.
#[must_use]
pub fn set(text: &str, section: &str, key: &str, value: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let new = format!("{key}={value}");
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + 2);
    let mut inside = false;
    let mut done = false;
    // Where the section's last non-blank line went in `out`.
    let mut section_end = None;
    for line in &lines {
        if let Some(s) = header(line) {
            inside = s.eq_ignore_ascii_case(section);
            out.push((*line).to_owned());
            if inside {
                section_end = Some(out.len());
            }
            continue;
        }
        if inside && !done && key_of(line).is_some_and(|k| k.eq_ignore_ascii_case(key)) {
            out.push(new.clone());
            done = true;
            section_end = Some(out.len());
            continue;
        }
        out.push((*line).to_owned());
        if inside && !line.trim().is_empty() {
            section_end = Some(out.len());
        }
    }
    if !done {
        if let Some(at) = section_end {
            out.insert(at, new);
        } else {
            if out.last().is_some_and(|l| !l.trim().is_empty()) {
                out.push(String::new());
            }
            out.push(format!("[{section}]"));
            out.push(new);
        }
    }
    out.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes() {
        let text = "; mine\n[general]\nDone=1\n\n[Widget Workshop]\n;Done=X\nwidgethome=c:\\\n\n[other]\na=b\n";
        assert_eq!(get(text, "widget workshop", "DONE"), None);
        assert_eq!(get(text, "general", "done").as_deref(), Some("1"));
        let t = set(text, "Widget Workshop", "Done", "0800000");
        assert_eq!(
            t,
            "; mine\n[general]\nDone=1\n\n[Widget Workshop]\n;Done=X\nwidgethome=c:\\\nDone=0800000\n\n[other]\na=b\n"
        );
        assert_eq!(get(&t, "Widget Workshop", "Done").as_deref(), Some("0800000"));
        let t = set(&t, "widget workshop", "done", "FFFFFF1");
        assert_eq!(get(&t, "Widget Workshop", "Done").as_deref(), Some("FFFFFF1"));
        assert_eq!(t.matches("FFFFFF1").count(), 1);
        assert_eq!(t.matches("0800000").count(), 0);
        assert_eq!(set("", "S", "k", "v"), "[S]\nk=v\n");
        assert_eq!(set("x=1", "S", "k", "v"), "x=1\n\n[S]\nk=v\n");
    }
}
