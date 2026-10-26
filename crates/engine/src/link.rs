//! Links that carry a widget (the port's own, docs/web.md): the browser
//! build's page with the widget in the fragment, which the browser never
//! sends to the server. `…/widget.html#n=CLOCK.WGT&w=A…`: `n`, the name
//! (optional, percent-encoded), and `w`, the widget, base64url without
//! padding of a format byte and the `.WGT` as Save writes it. Format 1:
//! raw deflate. The widget's pictures and sounds are the CD's, which the
//! player opening the link has too; a few kilobytes of link are a widget.

/// Format 1 of `w`: the `.WGT` deflated.
const DEFLATE: u8 = 1;

/// The most a link may inflate to: a widget is kilobytes, each of its
/// Recorders 40000 bytes.
const MAX_SIZE: usize = 16 << 20;

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// The link to `wgt` (a saved widget) on the page `base`, named `name`.
#[must_use]
pub fn encode(base: &str, name: Option<&str>, wgt: &[u8]) -> String {
    let mut payload = vec![DEFLATE];
    payload.extend(miniz_oxide::deflate::compress_to_vec(wgt, 10));
    let mut s = format!("{base}#");
    if let Some(n) = name {
        s += "n=";
        s += &percent_encode(n);
        s += "&";
    }
    s += "w=";
    s += &base64_encode(&payload);
    s
}

/// The widget in a link, and its name if it has one: `text` is the link
/// or its fragment, white space in it (a chat's line breaks) ignored.
/// `None` if it holds no widget.
#[must_use]
pub fn decode(text: &str) -> Option<(Option<String>, Vec<u8>)> {
    let text: String = text.split_whitespace().collect();
    let fragment = text.split_once('#').map_or(text.as_str(), |(_, f)| f);
    let mut name = None;
    let mut widget = None;
    for p in fragment.split('&') {
        match p.split_once('=') {
            Some(("n", v)) => name = percent_decode(v).filter(|n| !n.is_empty()),
            Some(("w", v)) => widget = Some(v),
            _ => {}
        }
    }
    let payload = base64_decode(widget?)?;
    let (&format, data) = payload.split_first()?;
    if format != DEFLATE {
        return None;
    }
    let wgt = miniz_oxide::inflate::decompress_to_vec_with_limit(data, MAX_SIZE).ok()?;
    Some((name, wgt))
}

fn base64_encode(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = c
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..=c.len() {
            s.push(char::from(B64[(n >> (18 - 6 * i)) as usize & 63]));
        }
    }
    s
}

/// Also takes standard base64 and padding.
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut n, mut bits) = (0u32, 0);
    for c in s.trim_end_matches('=').bytes() {
        let v = match c {
            b'+' => 62,
            b'/' => 63,
            _ => B64.iter().position(|&b| b == c)?,
        };
        n = n << 6 | u32::try_from(v).ok()?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from(n >> bits & 0xff).ok()?);
        }
    }
    Some(out)
}

fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b'_' => char::from(b).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn percent_decode(s: &str) -> Option<String> {
    let mut out = Vec::with_capacity(s.len());
    let mut b = s.bytes();
    while let Some(c) = b.next() {
        out.push(if c == b'%' {
            let hex = [b.next()?, b.next()?];
            u8::from_str_radix(std::str::from_utf8(&hex).ok()?, 16).ok()?
        } else {
            c
        });
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let wgt: Vec<u8> = (0..5000u32).map(|i| (i * i % 251) as u8).collect();
        let url = encode("https://example.org/widget.html", Some("MY WIDGET.WGT"), &wgt);
        assert!(url.starts_with("https://example.org/widget.html#n=MY%20WIDGET.WGT&w=A"));
        assert_eq!(decode(&url), Some((Some("MY WIDGET.WGT".to_owned()), wgt.clone())));
        // Without a name, as the fragment alone, broken over lines.
        let url = encode("x", None, &wgt);
        let (_, frag) = url.split_once('#').unwrap();
        let wrapped: String = frag
            .as_bytes()
            .chunks(70)
            .map(|c| std::str::from_utf8(c).unwrap())
            .collect::<Vec<_>>()
            .join("\n ");
        assert_eq!(decode(&wrapped), Some((None, wgt)));
    }

    #[test]
    fn base64_every_length() {
        for n in 0..8 {
            let d: Vec<u8> = (0..n).map(|i| 0xf0 ^ i).collect();
            let s = base64_encode(&d);
            assert_eq!(s.len(), (n as usize * 4).div_ceil(3));
            assert_eq!(base64_decode(&s), Some(d.clone()));
        }
        assert_eq!(base64_encode(b"\xfb\xff"), "-_8");
        assert_eq!(base64_decode("+/8="), Some(b"\xfb\xff".to_vec()));
    }

    #[test]
    fn not_a_widget() {
        assert_eq!(decode("hello"), None);
        assert_eq!(decode("https://example.org/#w="), None);
        assert_eq!(decode("#w=AQ!!"), None);
        // Another format.
        assert_eq!(decode(&format!("#w={}", base64_encode(&[2, 0, 0]))), None);
        // Not deflate.
        assert_eq!(decode(&format!("#w={}", base64_encode(&[1, 0xff, 0xff]))), None);
    }
}
