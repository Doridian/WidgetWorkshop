//! The game's text is Windows-1252, as the resources and saved widgets
//! hold it (the Mac's text was converted for the Windows build, the saved
//! widgets' too as far as their labels go: both versions share them).

/// The characters 0x80..=0x9f of Windows-1252 (undefined ones as `?`).
const HIGH: [char; 32] = [
    '€', '?', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '?', 'Ž', '?', '?', '‘', '’', '“', '”', '•', '–',
    '—', '˜', '™', 'š', '›', 'œ', '?', 'ž', 'Ÿ',
];

#[must_use]
pub fn decode(b: &[u8]) -> String {
    b.iter()
        .map(|&c| match c {
            0x80..=0x9f => HIGH[usize::from(c - 0x80)],
            _ => char::from(c),
        })
        .collect()
}

/// `s` as Windows-1252 bytes; what it can't hold becomes `?`.
#[must_use]
pub fn encode(s: &str) -> Vec<u8> {
    s.chars()
        .map(|ch| match u32::from(ch) {
            c @ (0..0x80 | 0xa0..0x100) => u8::try_from(c).unwrap_or(b'?'),
            _ => HIGH
                .iter()
                .position(|&h| h == ch && h != '?')
                .and_then(|i| u8::try_from(0x80 + i).ok())
                .unwrap_or(b'?'),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let b = b"\xb0Celsius \x93to\x94 \x80";
        assert_eq!(decode(b), "°Celsius “to” €");
        assert_eq!(encode(&decode(b)), b);
    }
}
