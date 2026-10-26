//! The small tables in `WWGADGET.DLL` and `WIDGET.EXE`, little-endian as
//! the Windows port converted them from the Mac's resources. Each type is
//! numbered by gadget (1000 to 1019, 2000 to 2046) or by sheet.
//!
//! - 1010 (`0x3f2`): points, `u16` count then `v`, `h` pairs.
//! - 1011 (`0x3f3`): one point.
//! - 1012 (`0x3f4`): rects, `u16` count then `top`, `left`, `bottom`, `right`.
//! - 1013 (`0x3f5`): one rect.
//! - 1014 (`0x3f6`): a gadget's knots (`0x41db6c`), `u16` count then
//!   `v`, `h` and a `u16` kind each (0 an input, 1 an output).
//! - 1021 (`0x3fd`): a gadget's info, four `u16`s (`0x41141f`).
//! - 1022 (`0x3fe`, in `WIDGET.EXE`): a combo box's strings ([`strings`]).

use crate::hr::Rect;

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}

#[allow(clippy::cast_possible_wrap)]
fn i16_at(d: &[u8], at: usize) -> Option<i16> {
    u16_at(d, at).map(|v| v as i16)
}

/// A point, `v` (down) then `h` (across), as the Mac's `Point`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Point {
    pub v: i16,
    pub h: i16,
}

/// Type 1010: a list of points.
#[must_use]
pub fn points(d: &[u8]) -> Vec<Point> {
    let n = u16_at(d, 0).unwrap_or(0);
    (0..usize::from(n))
        .map_while(|i| {
            Some(Point {
                v: i16_at(d, 2 + i * 4)?,
                h: i16_at(d, 4 + i * 4)?,
            })
        })
        .collect()
}

/// Type 1011: one point.
#[must_use]
pub fn point(d: &[u8]) -> Option<Point> {
    Some(Point {
        v: i16_at(d, 0)?,
        h: i16_at(d, 2)?,
    })
}

/// Type 1012: a list of rects.
#[must_use]
pub fn rects(d: &[u8]) -> Vec<Rect> {
    let n = u16_at(d, 0).unwrap_or(0);
    (0..usize::from(n)).map_while(|i| rect(&d[2 + i * 8..])).collect()
}

/// Type 1013: one rect.
#[must_use]
pub fn rect(d: &[u8]) -> Option<Rect> {
    Some(Rect::new(i16_at(d, 0)?, i16_at(d, 2)?, i16_at(d, 4)?, i16_at(d, 6)?))
}

/// Whether a knot takes a value in or gives one out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KnotKind {
    Input,
    Output,
}

/// A gadget's connection point, relative to its top left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KnotDef {
    pub at: Point,
    pub kind: KnotKind,
}

/// Type 1014: a gadget's knots.
#[must_use]
pub fn knots(d: &[u8]) -> Vec<KnotDef> {
    let n = u16_at(d, 0).unwrap_or(0);
    (0..usize::from(n))
        .map_while(|i| {
            let at = 2 + i * 6;
            Some(KnotDef {
                at: Point {
                    v: i16_at(d, at)?,
                    h: i16_at(d, at + 2)?,
                },
                kind: if u16_at(d, at + 4)? == 0 {
                    KnotKind::Input
                } else {
                    KnotKind::Output
                },
            })
        })
        .collect()
}

/// Type 1021: a gadget's numbers, as `0x41141f` reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GadgetInfo {
    /// The gadget's own id (its resources' number).
    pub id: u16,
    pub kind: u16,
    /// The settings dialog's number.
    pub dialog: u16,
    /// Doc's help text (type 1020).
    pub doc: u16,
}

#[must_use]
pub fn gadget_info(d: &[u8]) -> Option<GadgetInfo> {
    Some(GadgetInfo {
        id: u16_at(d, 0)?,
        kind: u16_at(d, 2)?,
        dialog: u16_at(d, 4)?,
        doc: u16_at(d, 6)?,
    })
}

/// Type 1022: a combo box's strings. Each entry is a `u16` control id, a
/// `u16` (`0x0403`), a `u32` length counting the NUL, the text and its
/// NUL; a zero id ends the list.
#[must_use]
pub fn strings(d: &[u8]) -> Vec<(u16, String)> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(id) = u16_at(d, at) {
        if id == 0 {
            break;
        }
        let Some(len) = d
            .get(at + 4..at + 8)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
        else {
            break;
        };
        let Some(text) = d.get(at + 8..at + 8 + len) else { break };
        let text = text.split(|&c| c == 0).next().unwrap_or_default();
        out.push((id, text.iter().map(|&c| char::from(c)).collect()));
        at += 8 + len;
    }
    out
}

/// A RIFF `WAVE` file's PCM, as 8-bit unsigned mono: the game's sounds are
/// all 8-bit mono at 11.025 kHz; 16-bit or stereo files (an imported wave)
/// are mixed down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wave {
    pub rate: u32,
    pub samples: Vec<u8>,
}

impl Wave {
    /// Reads a RIFF `WAVE` file's `fmt ` and `data`.
    #[must_use]
    pub fn parse(d: &[u8]) -> Option<Wave> {
        if d.get(..4) != Some(b"RIFF") || d.get(8..12) != Some(b"WAVE") {
            return None;
        }
        let mut at = 12;
        let (mut channels, mut rate, mut bits) = (0u16, 0u32, 0u16);
        let mut data = None;
        while at + 8 <= d.len() {
            let tag = &d[at..at + 4];
            let size = u32::from_le_bytes([d[at + 4], d[at + 5], d[at + 6], d[at + 7]]) as usize;
            let body = &d[at + 8..(at + 8 + size).min(d.len())];
            match tag {
                b"fmt " => {
                    if u16_at(body, 0)? != 1 {
                        return None;
                    }
                    channels = u16_at(body, 2)?;
                    rate = u32::from(u16_at(body, 4)?) | u32::from(u16_at(body, 6)?) << 16;
                    bits = u16_at(body, 14)?;
                }
                b"data" => data = Some(body),
                _ => {}
            }
            at += 8 + size + (size & 1);
        }
        let data = data?;
        let channels = usize::from(channels.max(1));
        let samples = match bits {
            8 => data
                .chunks_exact(channels)
                .map(|f| {
                    let sum: usize = f.iter().map(|&s| usize::from(s)).sum();
                    u8::try_from(sum / channels).unwrap_or(0x80)
                })
                .collect(),
            16 => data
                .chunks_exact(2 * channels)
                .map(|f| {
                    let sum: i32 = f
                        .as_chunks::<2>().0.iter()
                        .map(|s| i32::from(i16::from_le_bytes([s[0], s[1]])))
                        .sum();
                    let avg = sum / i32::try_from(channels).unwrap_or(1);
                    u8::try_from((avg >> 8) + 128).unwrap_or(0x80)
                })
                .collect(),
            _ => return None,
        };
        Some(Wave { rate, samples })
    }

    /// The wave as an 8-bit mono RIFF file.
    #[must_use]
    pub fn write(&self) -> Vec<u8> {
        let n = u32::try_from(self.samples.len()).unwrap_or(u32::MAX);
        let mut out = Vec::with_capacity(self.samples.len() + 44);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + n).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&self.rate.to_le_bytes());
        out.extend_from_slice(&self.rate.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&8u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(&self.samples);
        out
    }
}

/// `RGBQUAD`s (blue, green, red, reserved), as the palette resource
/// (`WIDGET.EXE` type 1003, id 101) holds them, as RGB.
#[must_use]
pub fn palette(d: &[u8]) -> [[u8; 3]; 256] {
    let mut out = [[0; 3]; 256];
    for (i, q) in d.as_chunks::<4>().0.iter().take(256).enumerate() {
        out[i] = [q[2], q[1], q[0]];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_knots_and_combo_strings() {
        let k = knots(&[2, 0, 0x51, 0, 4, 0, 0, 0, 0x51, 0, 0xa1, 0, 1, 0]);
        assert_eq!(k.len(), 2);
        assert_eq!(k[1].at, Point { v: 0x51, h: 0xa1 });
        assert_eq!(k[1].kind, KnotKind::Output);
        let mut d = vec![0x6b, 0, 3, 4, 4, 0, 0, 0];
        d.extend_from_slice(b"Dog\0");
        d.extend_from_slice(&[0, 0]);
        assert_eq!(strings(&d), [(0x6b, "Dog".to_owned())]);
    }

    #[test]
    fn wave_round_trips() {
        let w = Wave {
            rate: 11025,
            samples: vec![1, 2, 3],
        };
        assert_eq!(Wave::parse(&w.write()), Some(w));
    }
}
