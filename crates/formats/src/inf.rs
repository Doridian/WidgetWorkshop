//! OS/2 Information Presentation Facility `.INF` files (`VIEW.EXE`'s
//! books, and `.HLP` help libraries), as far as the OS/2 Warp CD's
//! `WIDGET.INF` uses them, plus the parts of the format around that.
//!
//! The 155-byte header points at everything else:
//!
//! - the table of contents: one entry per panel (topic), with its nesting
//!   level, its title and the "slots" that hold its text;
//! - the slots: compressed text. Every word is a byte indexing the slot's
//!   local dictionary, whose entries index the file's global dictionary of
//!   words. Bytes `0xFA..=0xFF` are paragraph ends, line breaks, spaces and
//!   escape sequences (styles, margins, links, pictures, fonts);
//! - the index (`:i1.`), the resource-id and name tables (link targets),
//!   the extended header's font table;
//! - the pictures (`:artwork.`): OS/2 1.x bitmaps whose bits are cut into
//!   blocks, each stored plain or LZW-compressed.
//!
//! [`InfFile::to_help`] turns a book into [`hlp::HelpFile`]'s topic model,
//! so the help viewer shows it like `WIDGET.HLP`. The text is in the book's
//! code page (437 here); [`oem`] decodes it. The full-text search table and
//! the NLS table aren't read. Reference: the "INF/HLP file format" notes
//! (inf02a.doc) and `NewView`'s reader; the parts `WIDGET.INF` uses were
//! checked against it.

use std::collections::BTreeMap;
use std::fmt;

use crate::bmp;
use crate::hlp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// No `HSP` signature.
    NotInf,
    /// Data ends early or an offset points outside the file.
    Truncated,
    /// Inconsistent data (a bad LZW code, a word outside a dictionary, ...).
    Corrupt(&'static str),
    /// A feature this parser doesn't read.
    Unsupported(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotInf => write!(f, "not an OS/2 INF file"),
            Error::Truncated => write!(f, "INF file is truncated"),
            Error::Corrupt(what) => write!(f, "INF file is corrupt: {what}"),
            Error::Unsupported(what) => write!(f, "unsupported INF feature: {what}"),
        }
    }
}

impl std::error::Error for Error {}

/// Whether `data` starts like an INF or OS/2 `.HLP` file.
#[must_use]
pub fn is_inf(data: &[u8]) -> bool {
    data.starts_with(b"HSP")
}

fn bytes(d: &[u8], at: usize, n: usize) -> Result<&[u8], Error> {
    d.get(at..at.checked_add(n).ok_or(Error::Truncated)?)
        .ok_or(Error::Truncated)
}

fn u8_at(d: &[u8], at: usize) -> Result<u8, Error> {
    d.get(at).copied().ok_or(Error::Truncated)
}

fn u16_at(d: &[u8], at: usize) -> Result<u16, Error> {
    let b = bytes(d, at, 2)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

fn u32_at(d: &[u8], at: usize) -> Result<usize, Error> {
    let b = bytes(d, at, 4)?;
    usize::try_from(u32::from_le_bytes([b[0], b[1], b[2], b[3]])).map_err(|_| Error::Truncated)
}

/// Code page 437's upper half.
const CP437: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', 'É', 'æ', 'Æ', 'ô', 'ö', 'ò', 'û',
    'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', 'á', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡',
    '«', '»', '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐', '└', '┴', '┬', '├', '─',
    '┼', '╞', '╟', '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█',
    '▄', '▌', '▐', '▀', 'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', '≡', '±', '≥',
    '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{a0}',
];

/// Decodes the book's text (code page 437, what `WIDGET.INF`'s fonts and
/// country table name). 0x07 is the list bullet (`:ul.`), as code page
/// 437 draws it.
#[must_use]
pub fn oem(b: &[u8]) -> String {
    b.iter()
        .map(|&c| match c {
            BULLET => '•',
            0x80.. => CP437[usize::from(c - 0x80)],
            _ => char::from(c),
        })
        .collect()
}

/// The word IPF's compiler writes for a `:ul.` item's bullet.
const BULLET: u8 = 0x07;

/// Code page 437 to Windows-1252 (the help viewer's fonts), `?` for what
/// Windows-1252 doesn't have.
fn to_ansi(b: &[u8]) -> Vec<u8> {
    oem(b)
        .chars()
        .map(|c| match u32::from(c) {
            n @ (0..0x80 | 0xa0..0x100) => u8::try_from(n).unwrap_or(b'?'),
            _ => (0x80..=0x9f).find(|&b| hlp::ansi(&[b]).starts_with(c)).unwrap_or(b'?'),
        })
        .collect()
}

/// One font of the extended header's table (`:font.`), what
/// [`Escape::Font`] selects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Font {
    pub face: String,
    /// IPF's `size=HxW`, in points; 0 × 0 is the default font.
    pub height: u16,
    pub width: u16,
    pub codepage: u16,
}

/// A table-of-contents entry: one panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocEntry {
    /// Nesting level, 1 for the top (`:h1.`).
    pub level: u8,
    /// The entries after it with a deeper level are its children.
    pub has_children: bool,
    /// Not shown in the contents (`hide`); reachable through links.
    pub hidden: bool,
    /// The slots its text is in, in order.
    pub slots: Vec<u16>,
    pub title: String,
}

/// An index entry (`:i1.`, `:i2.`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    pub word: String,
    /// The panel it leads to (a [`InfFile::toc`] index).
    pub topic: u16,
    /// `:i2.`, a second-level entry under the last `:i1.`.
    pub second_level: bool,
    pub global: bool,
}

/// A slot: a piece of a panel's text and the words it uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// The local dictionary: global dictionary indices.
    pub words: Vec<u16>,
    pub text: Vec<u8>,
}

/// What an escape sequence (`0xFF`, length, code, data) does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Escape {
    /// `0x02`: the left margin, in character widths (`:lm.`, list indents
    /// and the columns of hand-made tables). Text continues there if the
    /// line hasn't reached it yet.
    LeftMargin(u8),
    /// `0x11`: as [`Escape::LeftMargin`], on a new line.
    LeftMarginBreak(u8),
    /// `0x12`: as [`Escape::LeftMargin`], or a new line if the text is
    /// already past it (`:dl. tsize`).
    LeftMarginFit(u8),
    /// `0x1C`: the left margin where the text is now.
    LeftMarginHere,
    /// `0x03`: the right margin, in character widths.
    RightMargin(u8),
    /// `0x04`: `:hp1.`..`:hp7.`: 0 plain, 1 italic, 2 bold, 3 bold italic,
    /// 5 underlined, 6 underlined italic, 7 underlined bold.
    Style(u8),
    /// `0x0D`: `:hp4.`, `:hp8.`, `:hp9.`: 1 blue, 2 red, 3 pink; 0 plain.
    Colour(u8),
    /// `0x05`: the start of a link to a panel (a [`InfFile::toc`] index).
    /// Links that open a window of their own carry more data, not read.
    Link(u16),
    /// `0x07`: the start of a link to a footnote panel (a popup).
    FootnoteLink(u16),
    /// `0x08`: the end of a link.
    LinkEnd,
    /// `0x0B`/`0x0C`: monospaced text (`:xmp.`, `:cgraphic.`) begins or
    /// ends. Words in it aren't followed by spaces.
    Monospace(bool),
    /// `0x0E`: a picture (`:artwork.`): flags (0x01 left, 0x02 right, 0x04
    /// centred, 0x08 fit to the window, 0x10 in the line, `runin`) and its
    /// offset from the header's image start ([`InfFile::bitmap`]).
    Bitmap { flags: u8, offset: u32 },
    /// `0x19`: font [`InfFile::fonts`]`[n]`.
    Font(u8),
    /// `0x1A`/`0x1B`: `:lines.` begins (alignment: 1 left, 2 right, 4
    /// centred) or ends.
    Lines(Option<u8>),
    /// Anything else: its code and data.
    Other { code: u8, data: Vec<u8> },
}

impl Escape {
    fn parse(code: u8, data: &[u8]) -> Escape {
        let b = |i: usize| data.get(i).copied();
        let w = |i: usize| Some(u16::from_le_bytes([b(i)?, b(i + 1)?]));
        let known = match code {
            0x02 => b(0).map(Escape::LeftMargin),
            0x03 => b(0).map(Escape::RightMargin),
            0x04 => b(0).map(Escape::Style),
            0x05 => w(0).map(Escape::Link),
            0x07 => w(0).map(Escape::FootnoteLink),
            0x08 => Some(Escape::LinkEnd),
            0x0b => Some(Escape::Monospace(true)),
            0x0c => Some(Escape::Monospace(false)),
            0x0d => b(0).map(Escape::Colour),
            0x0e => data.get(1..5).map(|o| Escape::Bitmap {
                flags: data[0],
                offset: u32::from_le_bytes([o[0], o[1], o[2], o[3]]),
            }),
            0x11 => b(0).map(Escape::LeftMarginBreak),
            0x12 => b(0).map(Escape::LeftMarginFit),
            0x19 => b(0).map(Escape::Font),
            0x1a => Some(Escape::Lines(Some(b(0).unwrap_or(1)))),
            0x1b => Some(Escape::Lines(None)),
            0x1c => Some(Escape::LeftMarginHere),
            _ => None,
        };
        known.unwrap_or_else(|| Escape::Other {
            code,
            data: data.to_vec(),
        })
    }
}

/// A panel's text, decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// A word, punctuation mark or run of characters (code page 437).
    Word(Vec<u8>),
    /// A space: after a word while spacing is on, or `0xFE`.
    Space,
    /// `0xFA`: the end of a paragraph.
    Paragraph,
    /// `0xFD`: a line break.
    LineBreak,
    /// `0xFB`: centre the rest of the paragraph.
    Centre,
    Escape(Escape),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfFile {
    /// The flags byte: 0x01 a book (`.INF`), 0x10 a help library.
    pub flags: u8,
    pub title: String,
    pub toc: Vec<TocEntry>,
    pub fonts: Vec<Font>,
    pub index: Vec<IndexEntry>,
    /// `res=` numbers and the panels they name.
    pub resources: Vec<(u16, u16)>,
    /// `name=`/`id=` strings and the panels they name.
    pub names: Vec<(String, u16)>,
    /// The global dictionary.
    pub dictionary: Vec<Vec<u8>>,
    pub slots: Vec<Slot>,
    /// The file from the header's image start on (offsets in
    /// [`Escape::Bitmap`] count from there).
    images: Vec<u8>,
}

impl InfFile {
    /// Parses a book.
    ///
    /// # Errors
    /// See [`Error`].
    #[allow(clippy::too_many_lines)]
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        if !is_inf(data) {
            return Err(Error::NotInf);
        }
        let h = bytes(data, 0, 155)?;
        let u16h = |at| u16_at(h, at).map(usize::from);
        let u32h = |at| u32_at(h, at);
        let ntoc = u16h(8)?;
        let toc_at = u32h(18)?;
        let (nres, res_at) = (u16h(22)?, u32h(24)?);
        let (nname, name_at) = (u16h(28)?, u32h(30)?);
        let (nindex, index_at) = (u16h(34)?, u32h(36)?);
        let (nslots, slots_at) = (u16h(62)?, u32h(64)?);
        let (ndict, dict_at) = (u16h(72)?, u32h(74)?);
        let image_at = u32h(78)?;
        let ext_at = u32h(91)?;

        let mut dictionary = Vec::with_capacity(ndict);
        let mut at = dict_at;
        for _ in 0..ndict {
            let n = usize::from(u8_at(data, at)?);
            dictionary.push(bytes(data, at + 1, n.saturating_sub(1))?.to_vec());
            at += n.max(1);
        }

        let mut toc = Vec::with_capacity(ntoc);
        for i in 0..ntoc {
            let at = u32_at(data, toc_at + 4 * i)?;
            let len = usize::from(u8_at(data, at)?);
            let e = bytes(data, at, len)?;
            let flags = u8_at(e, 1)?;
            let nslots = usize::from(u8_at(e, 2)?);
            let mut p = 3;
            if flags & 0x20 != 0 {
                // Window placement: position, size, controls, a group.
                let (w1, w2) = (u8_at(e, p)?, u8_at(e, p + 1)?);
                p += 2;
                for (bit, n) in [(0x01, 5), (0x02, 5), (0x08, 2), (0x40, 2)] {
                    if w1 & bit != 0 {
                        p += n;
                    }
                }
                if w2 & 0x04 != 0 {
                    p += 2;
                }
            }
            let slots = (0..nslots).map(|k| u16_at(e, p + 2 * k)).collect::<Result<_, _>>()?;
            p += 2 * nslots;
            toc.push(TocEntry {
                level: flags & 0x0f,
                has_children: flags & 0x80 != 0,
                hidden: flags & 0x40 != 0,
                slots,
                title: oem(e.get(p..).ok_or(Error::Truncated)?),
            });
        }

        let slots = (0..nslots)
            .map(|i| {
                let at = u32_at(data, slots_at + 4 * i)?;
                let local_at = u32_at(data, at + 1)?;
                let nlocal = usize::from(u8_at(data, at + 5)?);
                let ntext = usize::from(u16_at(data, at + 6)?);
                Ok(Slot {
                    words: (0..nlocal)
                        .map(|k| u16_at(data, local_at + 2 * k))
                        .collect::<Result<_, _>>()?,
                    text: bytes(data, at + 8, ntext)?.to_vec(),
                })
            })
            .collect::<Result<_, Error>>()?;

        let mut index = Vec::with_capacity(nindex);
        let mut at = index_at;
        for _ in 0..nindex {
            let n = usize::from(u8_at(data, at)?);
            let flags = u8_at(data, at + 1)?;
            index.push(IndexEntry {
                word: oem(bytes(data, at + 5, n)?),
                topic: u16_at(data, at + 3)?,
                second_level: flags & 0x40 != 0,
                global: flags & 0x02 != 0,
            });
            at += 5 + n;
        }

        // Both tables: all the keys, then all the panels.
        let resources = (0..nres)
            .map(|i| Ok((u16_at(data, res_at + 2 * i)?, u16_at(data, res_at + 2 * (nres + i))?)))
            .collect::<Result<_, Error>>()?;
        let names = (0..nname)
            .map(|i| {
                let word = usize::from(u16_at(data, name_at + 2 * i)?);
                let topic = u16_at(data, name_at + 2 * (nname + i))?;
                Ok((oem(dictionary.get(word).ok_or(Error::Corrupt("panel name"))?), topic))
            })
            .collect::<Result<_, Error>>()?;

        let mut fonts = Vec::new();
        if ext_at != 0 {
            let n = usize::from(u16_at(data, ext_at)?);
            let fonts_at = u32_at(data, ext_at + 2)?;
            for i in 0..n {
                let f = bytes(data, fonts_at + 39 * i, 39)?;
                fonts.push(Font {
                    face: oem(f[..33].split(|&c| c == 0).next().unwrap_or_default()),
                    height: u16_at(f, 33)?,
                    width: u16_at(f, 35)?,
                    codepage: u16_at(f, 37)?,
                });
            }
        }

        Ok(InfFile {
            flags: h[3],
            title: oem(h[107..155].split(|&c| c == 0).next().unwrap_or_default()),
            toc,
            fonts,
            index,
            resources,
            names,
            dictionary,
            slots,
            images: data.get(image_at..).unwrap_or_default().to_vec(),
        })
    }

    /// Panel `topic`'s text: its slots decoded, with the spaces the
    /// spacing rule implies. Spacing starts on; `0xFC` flips it; paragraph
    /// ends and line breaks turn it back on outside monospaced text, which
    /// turns it off.
    ///
    /// # Errors
    /// [`Error::Truncated`] for an escape that runs past its slot,
    /// [`Error::Corrupt`] for a word outside the dictionaries.
    pub fn text(&self, topic: usize) -> Result<Vec<Item>, Error> {
        let entry = self.toc.get(topic).ok_or(Error::Corrupt("panel number"))?;
        let mut out = Vec::new();
        let (mut spacing, mut mono) = (true, false);
        for &s in &entry.slots {
            let slot = self.slots.get(usize::from(s)).ok_or(Error::Corrupt("slot number"))?;
            let t = &slot.text;
            let mut i = 0;
            while i < t.len() {
                match t[i] {
                    0xfa | 0xfd => {
                        out.push(if t[i] == 0xfa { Item::Paragraph } else { Item::LineBreak });
                        spacing |= !mono;
                    }
                    0xfb => out.push(Item::Centre),
                    0xfc => spacing = !spacing,
                    0xfe => out.push(Item::Space),
                    0xff => {
                        let n = usize::from(u8_at(t, i + 1)?);
                        let code = u8_at(t, i + 2)?;
                        let data = t.get(i + 3..i + 1 + n.max(2)).ok_or(Error::Truncated)?;
                        let e = Escape::parse(code, data);
                        if let Escape::Monospace(on) = e {
                            mono = on;
                            spacing = !on;
                        }
                        out.push(Item::Escape(e));
                        i += n.max(2);
                    }
                    w => {
                        let g = slot.words.get(usize::from(w)).ok_or(Error::Corrupt("local word"))?;
                        let word = self.dictionary.get(usize::from(*g)).ok_or(Error::Corrupt("word"))?;
                        out.push(Item::Word(word.clone()));
                        if spacing {
                            out.push(Item::Space);
                        }
                    }
                }
                i += 1;
            }
        }
        Ok(out)
    }

    /// Decodes the picture at `offset` from the image start (an
    /// [`Escape::Bitmap`]'s): an OS/2 bitmap file header (`bM`), a 1.x
    /// (12-byte) or 2.x info header and its palette, then the bits: their
    /// total size, the size each block unpacks to, and the blocks, each a
    /// length, a type (0 plain, 2 LZW) and the data. The rows are stored
    /// bottom-up, 4-byte aligned. Depths of 1, 4 and 8 bits come out as
    /// 8-bit indices.
    ///
    /// # Errors
    /// [`Error::Unsupported`] for other depths or block types; see
    /// [`Error`].
    pub fn bitmap(&self, offset: u32) -> Result<bmp::Bitmap, Error> {
        let d = self
            .images
            .get(usize::try_from(offset).map_err(|_| Error::Truncated)?..)
            .ok_or(Error::Truncated)?;
        if !d.starts_with(b"bM") {
            return Err(Error::Corrupt("bitmap signature"));
        }
        let bits_at = u32_at(d, 10)?;
        let info = u32_at(d, 14)?;
        let (width, height, depth, quad, colours) = if info == 12 {
            let bits = u16_at(d, 24)?;
            (usize::from(u16_at(d, 18)?), usize::from(u16_at(d, 20)?), bits, false, 0)
        } else {
            let colours = if info >= 36 { u32_at(d, 14 + 32)? } else { 0 };
            (u32_at(d, 18)?, u32_at(d, 22)?, u16_at(d, 28)?, true, colours)
        };
        if !matches!(depth, 1 | 4 | 8) {
            return Err(Error::Unsupported(format!("{depth}-bit picture")));
        }
        let mut palette = [bmp::Rgb::default(); 256];
        let n = if colours == 0 { 1 << depth } else { colours.min(256) };
        let step = if quad { 4 } else { 3 };
        for (i, entry) in palette.iter_mut().take(n).enumerate() {
            let c = bytes(d, 14 + info + step * i, 3)?;
            *entry = bmp::Rgb {
                r: c[2],
                g: c[1],
                b: c[0],
            };
        }

        let total = u32_at(d, bits_at)?;
        let block = usize::from(u16_at(d, bits_at + 4)?);
        let end = bits_at + 4 + total;
        let stride = width
            .checked_mul(usize::from(depth))
            .ok_or(Error::Corrupt("picture size"))?
            .div_ceil(32)
            * 4;
        let size = stride.checked_mul(height).ok_or(Error::Corrupt("picture size"))?;
        let mut raw = Vec::new();
        let mut at = bits_at + 6;
        while at < end {
            let n = usize::from(u16_at(d, at)?);
            let kind = u8_at(d, at + 2)?;
            let data = bytes(d, at + 3, n.saturating_sub(1))?;
            match kind {
                0 => raw.extend_from_slice(data),
                2 => raw.extend(lzw(data, block)?),
                _ => return Err(Error::Unsupported(format!("picture block type {kind}"))),
            }
            at += 2 + n;
        }
        if raw.len() < size {
            return Err(Error::Truncated);
        }
        let per = 8 / usize::from(depth);
        let mut pixels = Vec::with_capacity(width * height);
        for row in (0..height).rev() {
            let r = &raw[row * stride..(row + 1) * stride];
            pixels.extend((0..width).map(|x| {
                let b = r[x / per];
                let shift = 8 - usize::from(depth) * (x % per + 1);
                (b >> shift) & (0xff >> (8 - depth))
            }));
        }
        Ok(bmp::Bitmap {
            width,
            height,
            palette,
            pixels,
        })
    }
}

/// Unpacks a picture block's LZW: codes MSB first, 9 bits wide and one
/// wider each time the table is one entry short of the next power of two
/// (as TIFF's), up to 12; 256 clears the table and 257 ends.
fn lzw(src: &[u8], size: usize) -> Result<Vec<u8>, Error> {
    const CLEAR: usize = 256;
    const END: usize = 257;
    // Each entry: the entry it extends, its last byte, its length.
    let mut prefix = vec![0u16; 4096];
    let mut last: Vec<u8> = (0..=255).chain(std::iter::repeat_n(0, 4096 - 256)).collect();
    let mut length = vec![1u16; 4096];
    let mut out = Vec::with_capacity(size);
    let (mut next, mut width, mut prev) = (258, 9, None::<usize>);
    let (mut acc, mut have, mut i) = (0u32, 0, 0);
    let mut chain = Vec::new();
    loop {
        while have < width {
            let Some(&b) = src.get(i) else { return Ok(out) };
            acc = (acc << 8) | u32::from(b);
            have += 8;
            i += 1;
        }
        let code = usize::try_from((acc >> (have - width)) & ((1 << width) - 1)).unwrap_or(0);
        have -= width;
        if code == CLEAR {
            (next, width, prev) = (258, 9, None);
            continue;
        }
        if code == END {
            return Ok(out);
        }
        // The new string: the code's, or (code not there yet) the
        // previous one's and its first byte.
        let known = code < next;
        let from = match (known, prev) {
            (true, _) => code,
            (false, Some(p)) if code == next => p,
            _ => return Err(Error::Corrupt("LZW code")),
        };
        chain.clear();
        let mut c = from;
        loop {
            chain.push(last[c]);
            if length[c] == 1 {
                break;
            }
            c = usize::from(prefix[c]);
        }
        let first = chain[chain.len() - 1];
        out.extend(chain.iter().rev());
        if !known {
            out.push(first);
        }
        if let Some(p) = prev
            && next < 4096
        {
            prefix[next] = u16::try_from(p).unwrap_or(0);
            last[next] = first;
            length[next] = length[p].saturating_add(1);
            next += 1;
        }
        prev = Some(code);
        if next + 1 >= 1 << width && width < 12 {
            width += 1;
        }
    }
}

// ---- the help viewer's model --------------------------------------------

/// One character width of a margin, in help-compiler units (⅔ pixel):
/// System Proportional's average width at 96 dpi.
const SPACE: i16 = 10;
/// The blank line before a paragraph.
const PARA_GAP: i16 = 18;
/// Each contents level's indent.
const LEVEL_INDENT: i16 = 36;

impl InfFile {
    /// The book as a help file for the help viewer.
    ///
    /// Panel `i` is topic `i + 1`, at topic offset `i + 1`; context hash
    /// `i` leads there, so links are [`hlp::LinkAction::Jump`]`(i)`
    /// (footnotes are popups). Topic 0, at offset 0, is a made-up contents
    /// page: the visible entries as links, indented by level, which is
    /// what `VIEW.EXE` opens on. Each panel's title is its non-scrolling
    /// heading (`VIEW` shows it in the window's caption); a panel that
    /// only heads others lists them. `<<`/`>>` step through the contents
    /// in order, as `VIEW`'s Previous and Next. The Index is the book's
    /// index, or the panel titles if it has none.
    ///
    /// Formatting: paragraphs get a blank line above, margins become
    /// indents (or tab stops mid-line), the hp styles and colours become
    /// fonts, and a monospaced line of `─`/`═` (how `WIDGET.INF` draws the
    /// rules around its notes) becomes a rule. Pictures are decoded into
    /// BMP baggage named `bitmapNNNNNNNN.bmp` after their offset.
    ///
    /// # Errors
    /// A panel's text or a picture that doesn't decode.
    pub fn to_help(&self) -> Result<hlp::HelpFile, Error> {
        let mut b = Builder {
            inf: self,
            fonts: Vec::new(),
            baggage: BTreeMap::new(),
        };
        let shown: Vec<usize> = (0..self.toc.len()).filter(|&i| !self.toc[i].hidden).collect();
        let offset = |i: usize| u32::try_from(i + 1).unwrap_or(u32::MAX);

        let mut topics = vec![hlp::Topic {
            number: 0,
            offset: 0,
            title: "Contents".into(),
            macros: Vec::new(),
            browse_prev: None,
            browse_next: None,
            non_scroll: vec![b.heading("Contents")],
            scroll: b.list(&shown, 1),
        }];
        for (i, entry) in self.toc.iter().enumerate() {
            let mut scroll = b.panel(i)?;
            if scroll.is_empty() && entry.has_children {
                let children: Vec<usize> = shown
                    .iter()
                    .copied()
                    .skip_while(|&j| j <= i)
                    .take_while(|&j| self.toc[j].level > entry.level)
                    .filter(|&j| self.toc[j].level == entry.level + 1)
                    .collect();
                scroll = b.list(&children, entry.level + 1);
            }
            let at = shown.iter().position(|&j| j == i);
            topics.push(hlp::Topic {
                number: offset(i),
                offset: offset(i),
                title: entry.title.clone(),
                macros: Vec::new(),
                browse_prev: at.and_then(|k| k.checked_sub(1)).map(|k| offset(shown[k])),
                browse_next: at.and_then(|k| shown.get(k + 1)).map(|&j| offset(j)),
                non_scroll: vec![b.heading(&entry.title)],
                scroll,
            });
        }
        let keywords = if self.index.is_empty() {
            shown
                .iter()
                .map(|&i| hlp::Keyword {
                    word: self.toc[i].title.clone(),
                    topics: vec![offset(i)],
                })
                .collect()
        } else {
            self.index
                .iter()
                .map(|e| hlp::Keyword {
                    word: e.word.clone(),
                    topics: vec![offset(usize::from(e.topic))],
                })
                .collect()
        };
        Ok(hlp::HelpFile {
            version: (0, 0),
            title: self.title.clone(),
            copyright: String::new(),
            contents: 0,
            macros: Vec::new(),
            windows: Vec::new(),
            fonts: b.fonts,
            topics,
            contexts: (0..self.toc.len())
                .map(|i| (u32::try_from(i).unwrap_or(0), offset(i)))
                .collect(),
            map: Vec::new(),
            keywords,
            baggage: b.baggage,
        })
    }
}

/// A run's look: font table entry, size, hp style, hp colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Style {
    font: u8,
    /// 0: the font's own size.
    points: u16,
    hp: u8,
    colour: u8,
    mono: bool,
    /// The Symbol font (the viewer draws its 0xB7 as a bullet).
    symbol: bool,
}

struct Builder<'a> {
    inf: &'a InfFile,
    fonts: Vec<hlp::Font>,
    baggage: BTreeMap<String, Vec<u8>>,
}

impl Builder<'_> {
    /// The `hlp` font number for `s`, added if new.
    fn font(&mut self, s: Style) -> u16 {
        let f = self.inf.fonts.get(usize::from(s.font));
        let face = match f {
            _ if s.symbol => "Symbol".to_owned(),
            _ if s.mono => "System Monospaced".to_owned(),
            Some(f) if f.height != 0 => f.face.clone(),
            _ => "System Proportional".to_owned(),
        };
        let points = match (s.points, f) {
            (0, Some(f)) if f.height != 0 => f.height,
            (0, _) => 10,
            (p, _) => p,
        };
        let attributes = match s.hp {
            1 => 0x02,
            2 => 0x01,
            3 => 0x03,
            5 => 0x04,
            6 => 0x06,
            7 => 0x05,
            _ => 0,
        };
        let colour = match s.colour {
            1 => Some(bmp::Rgb { r: 0, g: 0, b: 0x80 }),
            2 => Some(bmp::Rgb { r: 0x80, g: 0, b: 0 }),
            3 => Some(bmp::Rgb { r: 0xff, g: 0, b: 0xff }),
            _ => None,
        };
        let font = hlp::Font {
            face,
            half_points: u8::try_from(points * 2).unwrap_or(u8::MAX),
            family: if s.mono { 1 } else { 3 },
            attributes,
            colour,
            background: None,
        };
        let i = self.fonts.iter().position(|f| *f == font).unwrap_or_else(|| {
            self.fonts.push(font);
            self.fonts.len() - 1
        });
        u16::try_from(i).unwrap_or(0)
    }

    /// A topic's heading: 14-point bold.
    fn heading(&mut self, title: &str) -> hlp::Paragraph {
        hlp::Paragraph {
            format: hlp::ParaFormat {
                space_above: 6,
                space_below: 6,
                ..hlp::ParaFormat::default()
            },
            items: vec![hlp::Inline::Text {
                text: to_ansi(title.as_bytes()),
                font: self.font(Style {
                    points: 14,
                    hp: 2,
                    ..Style::default()
                }),
                link: None,
            }],
        }
    }

    /// Links to `entries`, indented from level `top`.
    fn list(&mut self, entries: &[usize], top: u8) -> Vec<hlp::Paragraph> {
        let font = self.font(Style::default());
        entries
            .iter()
            .map(|&i| {
                let e = &self.inf.toc[i];
                hlp::Paragraph {
                    format: hlp::ParaFormat {
                        space_above: 4,
                        left_indent: i16::from(e.level.saturating_sub(top)) * LEVEL_INDENT,
                        ..hlp::ParaFormat::default()
                    },
                    items: vec![hlp::Inline::Text {
                        text: to_ansi(e.title.as_bytes()),
                        font,
                        link: Some(hlp::Link {
                            action: hlp::LinkAction::Jump(u32::try_from(i).unwrap_or(0)),
                            visible: true,
                        }),
                    }],
                }
            })
            .collect()
    }

    /// Panel `i`'s paragraphs.
    fn panel(&mut self, i: usize) -> Result<Vec<hlp::Paragraph>, Error> {
        let mut p = Panel::default();
        for item in self.inf.text(i)? {
            match item {
                Item::Word(w) if w == [BULLET] => {
                    let font = self.font(Style {
                        symbol: true,
                        ..p.style
                    });
                    p.text(&[0xb7], font);
                }
                Item::Word(w) => {
                    if p.style.mono && !w.is_empty() && w.iter().all(|&c| c == 0xc4 || c == 0xcd) {
                        p.rule = true;
                    } else {
                        let font = self.font(p.style);
                        p.text(&to_ansi(&w), font);
                    }
                }
                Item::Space => {
                    if !p.rule {
                        let font = self.font(p.style);
                        p.text(b" ", font);
                    }
                }
                Item::Paragraph => {
                    p.end_line();
                    p.end(true);
                }
                Item::LineBreak => p.line_break(),
                Item::Centre => p.cur.format.align = hlp::Align::Centre,
                Item::Escape(e) => match e {
                    Escape::LeftMargin(n) | Escape::LeftMarginFit(n) => p.margin = i16::from(n) * SPACE,
                    Escape::LeftMarginBreak(n) => {
                        p.margin = i16::from(n) * SPACE;
                        p.line_break();
                    }
                    Escape::RightMargin(n) => p.right = i16::from(n) * SPACE,
                    Escape::Style(s) => p.style.hp = s,
                    Escape::Colour(c) => p.style.colour = c,
                    Escape::Font(f) => p.style.font = f,
                    Escape::Monospace(on) => p.style.mono = on,
                    Escape::Link(t) | Escape::FootnoteLink(t) => {
                        p.link = Some(hlp::Link {
                            action: if matches!(e, Escape::Link(_)) {
                                hlp::LinkAction::Jump(u32::from(t))
                            } else {
                                hlp::LinkAction::Popup(u32::from(t))
                            },
                            visible: true,
                        });
                    }
                    Escape::LinkEnd => p.link = None,
                    Escape::Lines(a) => {
                        p.align = match a {
                            Some(2) => hlp::Align::Right,
                            Some(4) => hlp::Align::Centre,
                            _ => hlp::Align::Left,
                        };
                    }
                    Escape::Bitmap { flags, offset } => {
                        let name = format!("bitmap{offset:08x}.bmp");
                        if !self.baggage.contains_key(&name) {
                            self.baggage.insert(name.clone(), self.inf.bitmap(offset)?.to_bmp());
                        }
                        p.picture(name, flags);
                    }
                    Escape::LeftMarginHere | Escape::Other { .. } => {}
                },
            }
        }
        p.end_line();
        p.end(false);
        Ok(p.paras)
    }
}

/// A panel's paragraphs as they're built.
#[derive(Default)]
struct Panel {
    paras: Vec<hlp::Paragraph>,
    cur: hlp::Paragraph,
    /// The left margin set, and where the current line's text is.
    margin: i16,
    at: i16,
    right: i16,
    align: hlp::Align,
    style: Style,
    link: Option<hlp::Link>,
    /// Nothing on the line since the last break.
    line_empty: bool,
    /// A blank line before the next paragraph.
    gap: bool,
    /// The monospaced line so far is a rule.
    rule: bool,
}

impl Panel {
    /// Before content: starts the paragraph at the margin, a new one if a
    /// new line's margin moved, or tabs to a margin set mid-line.
    fn place(&mut self) {
        if self.cur.items.is_empty() {
            self.cur.format.left_indent = self.margin;
            self.cur.format.right_indent = self.right;
            if self.cur.format.align == hlp::Align::Left {
                self.cur.format.align = self.align;
            }
            if std::mem::take(&mut self.gap) && !self.paras.is_empty() {
                self.cur.format.space_above = PARA_GAP;
            }
            self.at = self.margin;
        } else if self.line_empty && self.margin != self.cur.format.left_indent {
            self.end(false);
            self.place();
        } else if self.margin > self.at {
            let f = &mut self.cur.format;
            let first_line = !self.cur.items.iter().any(|i| matches!(i, hlp::Inline::LineBreak));
            if first_line && f.first_line_indent == 0 {
                // The rest wraps to the new margin: a hanging indent (a
                // list item's text after its bullet, a table row).
                f.first_line_indent = f.left_indent - self.margin;
                f.left_indent = self.margin;
            } else {
                let position = u16::try_from(self.margin).unwrap_or(0);
                if !f.tabs.iter().any(|t| t.position == position) {
                    f.tabs.push(hlp::TabStop { position, kind: 0 });
                    f.tabs.sort_by_key(|t| t.position);
                }
            }
            self.cur.items.push(hlp::Inline::Tab);
            self.at = self.margin;
        }
        self.line_empty = false;
    }

    fn text(&mut self, bytes: &[u8], font: u16) {
        if bytes == b" " && (self.cur.items.is_empty() || self.line_empty) {
            return;
        }
        self.place();
        if let Some(hlp::Inline::Text { text, font: f, link }) = self.cur.items.last_mut()
            && *f == font
            && *link == self.link
        {
            text.extend_from_slice(bytes);
            return;
        }
        self.cur.items.push(hlp::Inline::Text {
            text: bytes.to_vec(),
            font,
            link: self.link.clone(),
        });
    }

    fn picture(&mut self, name: String, flags: u8) {
        let item = hlp::Inline::Picture {
            picture: hlp::Picture::Baggage(name),
            placement: hlp::Placement::Inline,
            link: self.link.clone(),
        };
        if flags & 0x10 != 0 {
            self.place();
            self.cur.items.push(item);
            return;
        }
        // On a line of its own.
        self.end(false);
        self.paras.push(hlp::Paragraph {
            format: hlp::ParaFormat {
                space_above: 6,
                space_below: 6,
                left_indent: self.margin,
                align: match flags & 0x06 {
                    0x02 => hlp::Align::Right,
                    0x04 => hlp::Align::Centre,
                    _ => hlp::Align::Left,
                },
                ..hlp::ParaFormat::default()
            },
            items: vec![item],
        });
    }

    /// The end of a line: a rule if it was one.
    fn end_line(&mut self) {
        if std::mem::take(&mut self.rule) {
            self.end(false);
            self.paras.push(hlp::Paragraph {
                format: hlp::ParaFormat {
                    space_above: 6,
                    space_below: 6,
                    // Exactly two pixels: the rule and no line of text.
                    line_spacing: -3,
                    left_indent: self.margin,
                    right_indent: self.right,
                    border: Some(hlp::Border { sides: 0x02, width: 0 }),
                    ..hlp::ParaFormat::default()
                },
                items: Vec::new(),
            });
        }
    }

    fn line_break(&mut self) {
        if self.rule {
            self.end_line();
        } else if self.cur.items.is_empty() {
            // A blank line.
            self.place();
            self.paras.push(std::mem::take(&mut self.cur));
            self.line_empty = false;
        } else {
            self.cur.items.push(hlp::Inline::LineBreak);
            self.line_empty = true;
            self.at = self.cur.format.left_indent;
        }
    }

    /// Ends the paragraph; `gap` puts a blank line before the next.
    fn end(&mut self, gap: bool) {
        while matches!(self.cur.items.last(), Some(hlp::Inline::LineBreak)) {
            self.cur.items.pop();
        }
        let cur = std::mem::take(&mut self.cur);
        if !cur.items.is_empty() {
            self.paras.push(cur);
        }
        self.line_empty = false;
        self.gap |= gap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lzw_codes() {
        // 9-bit codes MSB first: 'a', 'b', 258 ("ab"), 257 (end).
        let mut bits = 0u64;
        for c in [u64::from(b'a'), u64::from(b'b'), 258, 257] {
            bits = (bits << 9) | c;
        }
        let bits = bits << (40 - 36);
        let packed: Vec<u8> = (0..5)
            .rev()
            .map(|i| u8::try_from((bits >> (8 * i)) & 0xff).unwrap())
            .collect();
        assert_eq!(lzw(&packed, 4).unwrap(), b"abab");
        // A code past the table's end.
        assert_eq!(lzw(&[0x96, 0, 0], 4), Err(Error::Corrupt("LZW code")));
    }

    #[test]
    fn code_pages() {
        assert_eq!(oem(b"A\x81\xc4\x07"), "Aü─•");
        assert_eq!(to_ansi(b"A\x81\xc4\x07"), b"A\xfc?\x95");
    }

    fn put16(d: &mut [u8], at: usize, v: usize) {
        d[at..at + 2].copy_from_slice(&u16::try_from(v).unwrap().to_le_bytes());
    }

    fn put32(d: &mut [u8], at: usize, v: usize) {
        d[at..at + 4].copy_from_slice(&u32::try_from(v).unwrap().to_le_bytes());
    }

    /// A small book: "One" (with a child, "Two") and a hidden "Secret";
    /// one font, an index entry, a resource id and a 3×2 4-bit picture.
    fn book() -> Vec<u8> {
        let mut d = vec![0u8; 155];
        d[..4].copy_from_slice(b"HSP\x01");
        d[107..118].copy_from_slice(b"Little Book");
        // The picture first, as IPFC writes it.
        let here = d.len();
        put32(&mut d, 78, here);
        let mut pic = b"bM\0\0\0\0\0\0\0\0".to_vec();
        pic.extend_from_slice(&74u32.to_le_bytes());
        pic.extend_from_slice(&[12, 0, 0, 0, 3, 0, 2, 0, 1, 0, 4, 0]);
        for i in 0..16u8 {
            pic.extend_from_slice(&[i, 0, 0xff - i]);
        }
        pic.extend_from_slice(&13u32.to_le_bytes());
        pic.extend_from_slice(&[8, 0, 9, 0, 0]);
        pic.extend_from_slice(&[0x12, 0x30, 0, 0, 0x45, 0x60, 0, 0]);
        d.extend(pic);

        let words: [&[u8]; 6] = [b"Hello", b",", b"world", b"Next", b"\xc4\xc4\xc4", &[BULLET]];
        let here = d.len();
        put32(&mut d, 74, here);
        put16(&mut d, 72, words.len());
        for w in words {
            d.push(u8::try_from(w.len() + 1).unwrap());
            d.extend_from_slice(w);
        }

        let entries: [(u8, &[u8], &[u8]); 3] = [
            (0x81, b"\x00\x00", b"One"),
            (0x02, b"\x01\x00", b"Two"),
            (0x41, b"", b"Secret"),
        ];
        let mut at = Vec::new();
        for (flags, slots, title) in entries {
            at.push(d.len());
            d.push(u8::try_from(3 + slots.len() + title.len()).unwrap());
            d.extend_from_slice(&[flags, u8::try_from(slots.len() / 2).unwrap()]);
            d.extend_from_slice(slots);
            d.extend_from_slice(title);
        }
        let here = d.len();
        put32(&mut d, 18, here);
        put16(&mut d, 8, entries.len());
        for a in at {
            d.extend_from_slice(&u32::try_from(a).unwrap().to_le_bytes());
        }

        let texts: [&[u8]; 2] = [
            // Bold "Hello, world", a break; a link "Next", a picture in the
            // line; a monospaced rule.
            b"\xfa\xff\x03\x04\x02\xfc\x00\xfc\x01\x02\xff\x03\x04\x00\xfd\
              \xff\x04\x05\x01\x00\x03\xff\x02\x08\xff\x07\x0e\x10\x00\x00\x00\x00\
              \xfa\xff\x02\x0b\x04\xfd\xff\x02\x0c",
            // A bullet list item: the bullet at 2, the text at 4.
            b"\xff\x03\x02\x02\xfc\x00\xfc\xff\x03\x02\x04\x01",
        ];
        let locals: [&[u16]; 2] = [&[0, 1, 2, 3, 4], &[5, 2]];
        let mut slot_at = Vec::new();
        for (text, local) in texts.iter().zip(locals) {
            let local_at = d.len();
            for w in local {
                d.extend_from_slice(&w.to_le_bytes());
            }
            slot_at.push(d.len());
            d.push(0);
            d.extend_from_slice(&u32::try_from(local_at).unwrap().to_le_bytes());
            d.push(u8::try_from(local.len()).unwrap());
            d.extend_from_slice(&u16::try_from(text.len()).unwrap().to_le_bytes());
            d.extend_from_slice(text);
        }
        let here = d.len();
        put32(&mut d, 64, here);
        put16(&mut d, 62, slot_at.len());
        for a in slot_at {
            d.extend_from_slice(&u32::try_from(a).unwrap().to_le_bytes());
        }

        // The index: "Greeting" → "Two"; resource 42 → "Two".
        let here = d.len();
        put32(&mut d, 36, here);
        put16(&mut d, 34, 1);
        d.extend_from_slice(&[8, 0, 0, 1, 0]);
        d.extend_from_slice(b"Greeting");
        let here = d.len();
        put32(&mut d, 24, here);
        put16(&mut d, 22, 1);
        d.extend_from_slice(&[42, 0, 1, 0]);

        let fonts_at = d.len();
        let mut font = b"Helv".to_vec();
        font.resize(33, 0);
        font.extend_from_slice(&[12, 0, 8, 0, 0xb5, 0x01]);
        d.extend(font);
        let here = d.len();
        put32(&mut d, 91, here);
        let mut ext = vec![0u8; 64];
        put16(&mut ext, 0, 1);
        put32(&mut ext, 2, fonts_at);
        d.extend(ext);
        d
    }

    #[test]
    fn reads_a_book() {
        let b = InfFile::parse(&book()).unwrap();
        assert_eq!(InfFile::parse(b"HSQ"), Err(Error::NotInf));
        assert_eq!(b.title, "Little Book");
        assert_eq!(b.flags, 1);
        let toc: Vec<_> = b
            .toc
            .iter()
            .map(|e| (e.level, e.has_children, e.hidden, e.title.as_str()))
            .collect();
        assert_eq!(
            toc,
            [
                (1, true, false, "One"),
                (2, false, false, "Two"),
                (1, false, true, "Secret")
            ]
        );
        assert_eq!(
            b.fonts,
            [Font {
                face: "Helv".into(),
                height: 12,
                width: 8,
                codepage: 437
            }]
        );
        assert_eq!(b.index[0].word, "Greeting");
        assert_eq!(b.index[0].topic, 1);
        assert_eq!(b.resources, [(42, 1)]);

        let w = |s: &[u8]| Item::Word(s.to_vec());
        assert_eq!(
            b.text(0).unwrap(),
            [
                Item::Paragraph,
                Item::Escape(Escape::Style(2)),
                w(b"Hello"),
                w(b","),
                Item::Space,
                w(b"world"),
                Item::Space,
                Item::Escape(Escape::Style(0)),
                Item::LineBreak,
                Item::Escape(Escape::Link(1)),
                w(b"Next"),
                Item::Space,
                Item::Escape(Escape::LinkEnd),
                Item::Escape(Escape::Bitmap { flags: 0x10, offset: 0 }),
                Item::Paragraph,
                Item::Escape(Escape::Monospace(true)),
                w(b"\xc4\xc4\xc4"),
                Item::LineBreak,
                Item::Escape(Escape::Monospace(false)),
            ]
        );
        assert_eq!(b.text(2).unwrap(), []);

        // Rows bottom-up, two pixels a byte.
        let pic = b.bitmap(0).unwrap();
        assert_eq!((pic.width, pic.height), (3, 2));
        assert_eq!(pic.pixels, [4, 5, 6, 1, 2, 3]);
        assert_eq!(pic.palette[1], bmp::Rgb { r: 0xfe, g: 0, b: 1 });
        assert_eq!(b.bitmap(1), Err(Error::Corrupt("bitmap signature")));
    }

    #[test]
    fn a_book_as_help() {
        let h = InfFile::parse(&book()).unwrap().to_help().unwrap();
        assert_eq!(h.title, "Little Book");
        let titles: Vec<&str> = h.topics.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["Contents", "One", "Two", "Secret"]);
        // The contents: the shown entries as jumps, indented by level.
        let contents = h.contents_topic().unwrap();
        assert_eq!(contents.scroll.len(), 2);
        assert_eq!(contents.scroll[1].format.left_indent, LEVEL_INDENT);
        assert_eq!(contents.scroll[1].plain_text(), "Two");
        let hlp::Inline::Text { link: Some(l), .. } = &contents.scroll[1].items[0] else {
            panic!("no link");
        };
        assert_eq!(l.action, hlp::LinkAction::Jump(1));
        assert_eq!(h.topic_for_hash(1).unwrap().title, "Two");
        // Browsing skips the hidden panel.
        assert_eq!(h.topics[1].browse_next, Some(2));
        assert_eq!(h.topics[2].browse_next, None);
        assert_eq!(h.topics[3].browse_prev, None);
        assert_eq!(h.keywords[0].word, "Greeting");
        assert_eq!(h.keywords[0].topics, [2]);

        let one = &h.topics[1];
        assert_eq!(one.non_scroll[0].plain_text(), "One");
        assert_eq!(one.scroll.len(), 2);
        assert_eq!(one.scroll[0].plain_text(), "Hello, world \nNext ");
        let items = &one.scroll[0].items;
        let hlp::Inline::Text { font, .. } = &items[0] else {
            panic!("no text");
        };
        assert!(h.fonts[usize::from(*font)].bold());
        assert_eq!(h.fonts[usize::from(*font)].half_points, 24);
        assert!(matches!(
            &items[2],
            hlp::Inline::Text {
                link: Some(hlp::Link {
                    action: hlp::LinkAction::Jump(1),
                    ..
                }),
                ..
            }
        ));
        let hlp::Inline::Picture { picture, .. } = &items[3] else {
            panic!("no picture");
        };
        let pic = h.bitmap(picture.baggage_bitmap().unwrap()).unwrap();
        assert_eq!(pic.pixels, [4, 5, 6, 1, 2, 3]);
        // The monospaced line of `─` is a rule.
        let rule = &one.scroll[1];
        assert_eq!(rule.items, []);
        assert_eq!(rule.format.border.map(|b| b.sides), Some(0x02));

        // The bullet: Symbol's, hanging from the item's margin.
        let item = &h.topics[2].scroll[0];
        assert_eq!(
            (item.format.left_indent, item.format.first_line_indent),
            (4 * SPACE, -2 * SPACE)
        );
        let hlp::Inline::Text { text, font, .. } = &item.items[0] else {
            panic!("no bullet");
        };
        assert_eq!(
            (text.as_slice(), h.fonts[usize::from(*font)].face.as_str()),
            (&[0xb7][..], "Symbol")
        );
        assert_eq!(item.items[1], hlp::Inline::Tab);
        assert_eq!(item.plain_text(), "\u{b7}\tworld ");
    }

    /// The OS/2 Warp CD's `WIDGET/WIDGET.INF`, from the image `WIDGET_OS2_ISO`
    /// names, else `WW1_01A.ISO` at the top of the repository (skipped
    /// without).
    fn shipped() -> Option<Vec<u8>> {
        let iso = std::env::var_os("WIDGET_OS2_ISO").map_or_else(
            || std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../WW1_01A.ISO"),
            std::path::PathBuf::from,
        );
        crate::iso::read_file(&std::fs::read(iso).ok()?, "WIDGET/WIDGET.INF")
    }

    #[test]
    fn reads_the_shipped_file() {
        let Some(file) = shipped() else {
            eprintln!("no OS/2 CD: skipped");
            return;
        };
        let b = InfFile::parse(&file).unwrap();
        assert_eq!(b.title, "Widget Workshop for OS/2 Warp");
        assert_eq!(b.toc.len(), 15);
        assert_eq!(b.slots.len(), 15);
        assert_eq!(b.dictionary.len(), 645);
        assert!(b.index.is_empty() && b.resources.is_empty());
        assert_eq!(b.fonts[2].face, "Helv");
        assert_eq!((b.fonts[2].height, b.fonts[2].width), (16, 10));
        assert_eq!(b.toc[9].title, "The Menus");
        assert!(b.toc[9].has_children);
        assert_eq!(b.toc[10].level, 2);
        assert_eq!(b.toc[14].slots, [13, 14]);

        // Six pictures, all LZW: the screen, the Control Box, the Tooltray
        // and its three tool buttons.
        let mut sizes = Vec::new();
        for i in 0..b.toc.len() {
            for item in b.text(i).unwrap() {
                if let Item::Escape(Escape::Bitmap { offset, .. }) = item {
                    let pic = b.bitmap(offset).unwrap();
                    sizes.push((pic.width, pic.height));
                }
            }
        }
        assert_eq!(sizes, [(552, 373), (53, 95), (448, 96), (61, 37), (33, 37), (31, 37)]);

        let h = b.to_help().unwrap();
        assert_eq!(h.topics.len(), 16);
        assert_eq!(h.baggage.len(), 6);
        assert_eq!(
            h.topics[2].scroll[0].plain_text(),
            "Widget Workshop is your very own mad scientist's laboratory. It's a special place inside your \
             computer that you can go to play with all sorts of parts and pieces and hook them all together to see \
             what happens. "
        );
        // "The Menus" has no text of its own: it lists its four.
        let menus: Vec<String> = h.topics[10].scroll.iter().map(hlp::Paragraph::plain_text).collect();
        assert_eq!(menus, ["File Menu", "Edit Menu", "Options Menu", "Help Menu"]);
        // The notes' rules.
        let rules = h
            .topics
            .iter()
            .flat_map(|t| &t.scroll)
            .filter(|p| p.format.border.is_some())
            .count();
        assert_eq!(rules, 10);
        // The keywords are the panel titles.
        assert_eq!(h.keywords.len(), 15);
    }
}
