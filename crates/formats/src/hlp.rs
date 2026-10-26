//! Windows Help 3.1 `.HLP` files, as far as `WIDGET.HLP` uses them.
//!
//! A help file is a small file system: a header points at a B+ tree
//! directory of internal files. The ones read here:
//!
//! - `|SYSTEM`: version, title, copyright, contents topic, startup macros
//!   and secondary-window definitions.
//! - `|TOPIC`: every topic's text, in 4 KiB blocks. Each block is a 12-byte
//!   header followed by LZ77 data that unpacks to up to 16 KiB. Inside is a
//!   linked list of `TOPICLINK` records that may run across block ends:
//!   topic headers (type 2) and paragraphs of text (type `0x20`).
//! - `|Phrases`: the phrase table the text and titles are compressed with.
//! - `|FONT`: face names and font descriptors (size, style, colour).
//! - `|CONTEXT`: context-string hash to topic offset (what hotspots jump to).
//! - `|CTXOMAP`: `[MAP]` ids to topic offsets.
//! - `|KWBTREE`/`|KWDATA`: the keyword index (Index button).
//!
//! Internal files whose names don't start with `|` are baggage. In
//! `WIDGET.HLP` they're the six screenshots, plain 8-bit BMPs that topics
//! show through Doc-To-Help's `D2HTools` embedded window (`{ewc}`), not
//! the help compiler's own `|bm` pictures, so those aren't decoded here.
//!
//! The text model keeps the bytes the help compiler stored (Windows-1252, or
//! the Symbol font's own code points for runs in that face), so the engine
//! can draw them with its bitmap fonts. Reference: Manfred Winterhoff's
//! `helpfile.txt` (helpdeco); field meanings below were checked against the
//! shipped file.

use std::collections::BTreeMap;
use std::fmt;

use crate::bmp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// No help-file signature.
    NotHelp,
    /// A required internal file is missing.
    Missing(&'static str),
    /// Data ends early or an offset points outside its file.
    Truncated,
    /// Inconsistent data (bad LZ77 back-reference, B+ tree loop, ...).
    Corrupt(&'static str),
    /// A feature `WIDGET.HLP` doesn't use and this parser doesn't read.
    Unsupported(String),
    /// A baggage bitmap failed to decode.
    Bitmap(bmp::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotHelp => write!(f, "not a WinHelp file"),
            Error::Missing(name) => write!(f, "help file has no {name}"),
            Error::Truncated => write!(f, "help file is truncated"),
            Error::Corrupt(what) => write!(f, "help file is corrupt: {what}"),
            Error::Unsupported(what) => write!(f, "unsupported help feature: {what}"),
            Error::Bitmap(e) => write!(f, "help bitmap: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// A little-endian reader over one internal file or record.
struct Cur<'a> {
    d: &'a [u8],
    at: usize,
}

impl<'a> Cur<'a> {
    fn new(d: &'a [u8]) -> Self {
        Cur { d, at: 0 }
    }

    fn at(d: &'a [u8], at: usize) -> Self {
        Cur { d, at }
    }

    fn done(&self) -> bool {
        self.at >= self.d.len()
    }

    fn bytes(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let b = self.d.get(self.at..self.at + n).ok_or(Error::Truncated)?;
        self.at += n;
        Ok(b)
    }

    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.bytes(1)?[0])
    }

    fn peek(&self) -> Result<u8, Error> {
        self.d.get(self.at).copied().ok_or(Error::Truncated)
    }

    fn u16(&mut self) -> Result<u16, Error> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn i16(&mut self) -> Result<i16, Error> {
        Ok(self.u16()?.cast_signed())
    }

    fn u32(&mut self) -> Result<u32, Error> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn i32(&mut self) -> Result<i32, Error> {
        Ok(self.u32()?.cast_signed())
    }

    /// A NUL-terminated string (the NUL is consumed; a missing one ends at
    /// the end of the data).
    fn cstr(&mut self) -> &'a [u8] {
        let rest = self.d.get(self.at..).unwrap_or_default();
        let n = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        self.at += (n + 1).min(rest.len());
        &rest[..n]
    }

    /// A fixed-size string field, cut at its first NUL.
    fn fixed_str(&mut self, n: usize) -> Result<String, Error> {
        let b = self.bytes(n)?;
        Ok(ansi(b.split(|&c| c == 0).next().unwrap_or_default()))
    }

    /// "Compressed signed long": two bytes if bit 0 is clear, else four;
    /// the value is that shifted right by one, biased by half the range.
    fn comp_long(&mut self) -> Result<i32, Error> {
        if self.peek()? & 1 == 0 {
            Ok(i32::from(self.u16()? >> 1) - 0x4000)
        } else {
            Ok((self.u32()? >> 1).cast_signed() - 0x4000_0000)
        }
    }

    /// "Compressed signed short": one byte if bit 0 is clear, else two.
    fn comp_short(&mut self) -> Result<i16, Error> {
        if self.peek()? & 1 == 0 {
            Ok(i16::from(self.u8()? >> 1) - 0x40)
        } else {
            Ok((self.u16()? >> 1).cast_signed() - 0x4000)
        }
    }

    /// "Compressed unsigned short": as above, without the bias.
    fn comp_ushort(&mut self) -> Result<u16, Error> {
        if self.peek()? & 1 == 0 {
            Ok(u16::from(self.u8()? >> 1))
        } else {
            Ok(self.u16()? >> 1)
        }
    }
}

/// Decodes Windows-1252 text (the help compiler's character set).
#[must_use]
pub fn ansi(b: &[u8]) -> String {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}', '\u{90}', '‘',
        '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
    ];
    b.iter()
        .map(|&c| match c {
            0x80..=0x9f => HIGH[usize::from(c - 0x80)],
            _ => char::from(c),
        })
        .collect()
}

/// Unpacks the help compiler's LZ77: a flag byte, then eight items, each a literal
/// byte (flag bit clear) or a 16-bit back-reference (set): low 12 bits are
/// the distance − 1, high 4 the length − 3.
fn lz77(src: &[u8]) -> Result<Vec<u8>, Error> {
    let mut out = Vec::with_capacity(src.len() * 3);
    let mut i = 0;
    while i < src.len() {
        let flags = src[i];
        i += 1;
        for bit in 0..8 {
            if i >= src.len() {
                break;
            }
            if flags & (1 << bit) == 0 {
                out.push(src[i]);
                i += 1;
            } else {
                let v = usize::from(u16::from_le_bytes([src[i], *src.get(i + 1).ok_or(Error::Truncated)?]));
                i += 2;
                let back = (v & 0xfff) + 1;
                let from = out.len().checked_sub(back).ok_or(Error::Corrupt("LZ77 reference"))?;
                for k in 0..(v >> 12) + 3 {
                    out.push(out[from + k]);
                }
            }
        }
    }
    Ok(out)
}

/// The leaf entries of a B+ tree internal file: each leaf page's entry
/// count and bytes, in key order. Index pages are only walked through.
fn btree_leaves(file: &[u8]) -> Result<Vec<(u16, &[u8])>, Error> {
    let mut h = Cur::new(file);
    if h.u16()? != 0x293b {
        return Err(Error::Corrupt("B+ tree signature"));
    }
    h.u16()?; // flags
    let page_size = usize::from(h.u16()?);
    h.bytes(16 + 2 + 2)?; // structure, must-be-zero, page splits
    let root = h.i16()?;
    h.i16()?; // must be -1
    let total_pages = h.u16()?;
    let levels = h.u16()?;
    let page = |n: i16| -> Result<&[u8], Error> {
        let n = usize::try_from(n).map_err(|_| Error::Corrupt("B+ tree page"))?;
        file.get(38 + n * page_size..38 + (n + 1) * page_size)
            .ok_or(Error::Truncated)
    };
    // Index pages start {unused, count, first child}; descend leftmost.
    let mut n = root;
    for _ in 1..levels {
        n = Cur::at(page(n)?, 4).i16()?;
    }
    let mut leaves = Vec::new();
    while n != -1 {
        if leaves.len() > usize::from(total_pages) {
            return Err(Error::Corrupt("B+ tree leaf loop"));
        }
        let p = page(n)?;
        let mut c = Cur::at(p, 2);
        let count = c.u16()?;
        c.i16()?; // previous leaf
        n = c.i16()?;
        leaves.push((count, &p[8..]));
    }
    Ok(leaves)
}

/// A secondary-window definition from `|SYSTEM` (record type 6). Each field
/// is only present if the help project set it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Window {
    pub kind: Option<String>,
    pub name: Option<String>,
    pub caption: Option<String>,
    pub x: Option<i16>,
    pub y: Option<i16>,
    pub width: Option<i16>,
    pub height: Option<i16>,
    /// `SW_*`-style show state (1 = maximized).
    pub maximize: Option<i16>,
    pub background: Option<bmp::Rgb>,
    pub non_scroll_background: Option<bmp::Rgb>,
    pub on_top: bool,
}

impl Window {
    fn parse(d: &[u8]) -> Result<Self, Error> {
        let mut c = Cur::new(d);
        let flags = c.u16()?;
        let has = |bit: u16| flags & bit != 0;
        let kind = c.fixed_str(10)?;
        let name = c.fixed_str(9)?;
        let caption = c.fixed_str(51)?;
        let mut nums = [0i16; 5];
        for n in &mut nums {
            *n = c.i16()?;
        }
        let mut rgb = || -> Result<bmp::Rgb, Error> {
            let b = c.bytes(4)?;
            Ok(bmp::Rgb {
                r: b[0],
                g: b[1],
                b: b[2],
            })
        };
        let background = rgb()?;
        let non_scroll = rgb()?;
        Ok(Window {
            kind: has(0x01).then_some(kind),
            name: has(0x02).then_some(name),
            caption: has(0x04).then_some(caption),
            x: has(0x08).then_some(nums[0]),
            y: has(0x10).then_some(nums[1]),
            width: has(0x20).then_some(nums[2]),
            height: has(0x40).then_some(nums[3]),
            maximize: has(0x80).then_some(nums[4]),
            background: has(0x100).then_some(background),
            non_scroll_background: has(0x200).then_some(non_scroll),
            on_top: has(0x400),
        })
    }
}

/// One `|FONT` descriptor: what a text run's font number selects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Font {
    pub face: String,
    /// Size in half points.
    pub half_points: u8,
    /// `FF_*` family: 1 modern, 2 roman, 3 swiss, 4 script, 5 decorative.
    pub family: u8,
    /// Style bits; see the methods.
    pub attributes: u8,
    /// Text colour; `None` for the default (stored as 01 01 00, which is
    /// what all of `WIDGET.HLP`'s fonts have).
    pub colour: Option<bmp::Rgb>,
    pub background: Option<bmp::Rgb>,
}

impl Font {
    #[must_use]
    pub fn bold(&self) -> bool {
        self.attributes & 0x01 != 0
    }
    #[must_use]
    pub fn italic(&self) -> bool {
        self.attributes & 0x02 != 0
    }
    #[must_use]
    pub fn underline(&self) -> bool {
        self.attributes & 0x04 != 0
    }
    #[must_use]
    pub fn strikeout(&self) -> bool {
        self.attributes & 0x08 != 0
    }
    /// The help compiler's hotspot marker in RTF; in `WIDGET.HLP` only
    /// font 7, which carries no text.
    #[must_use]
    pub fn double_underline(&self) -> bool {
        self.attributes & 0x10 != 0
    }
    #[must_use]
    pub fn small_caps(&self) -> bool {
        self.attributes & 0x20 != 0
    }
}

/// A descriptor colour, or `None` for the "automatic" marker.
fn font_colour(b: &[u8]) -> Option<bmp::Rgb> {
    (b != [1, 1, 0]).then(|| bmp::Rgb {
        r: b[0],
        g: b[1],
        b: b[2],
    })
}

fn parse_fonts(d: &[u8]) -> Result<Vec<Font>, Error> {
    let mut c = Cur::new(d);
    let faces = usize::from(c.u16()?);
    let descriptors = c.u16()?;
    let face_at = usize::from(c.u16()?);
    let desc_at = usize::from(c.u16()?);
    if face_at >= 12 {
        return Err(Error::Unsupported("WinHelp 4 font table".into()));
    }
    let face_len = desc_at.saturating_sub(face_at) / faces.max(1);
    let face_names: Vec<String> = (0..faces)
        .map(|i| Cur::at(d, face_at + i * face_len).fixed_str(face_len))
        .collect::<Result<_, _>>()?;
    let mut c = Cur::at(d, desc_at);
    (0..descriptors)
        .map(|_| {
            let attr = c.u8()?;
            let half_points = c.u8()?;
            let family = c.u8()?;
            let face = usize::from(c.u16()?);
            let fg = c.bytes(3)?;
            let bg = c.bytes(3)?;
            Ok(Font {
                face: face_names.get(face).cloned().unwrap_or_default(),
                half_points,
                family,
                attributes: attr,
                colour: font_colour(fg),
                background: font_colour(bg),
            })
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Right,
    Centre,
}

/// A paragraph border (`bits & 0x100`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Border {
    /// 0x01 box, 0x02 top, 0x04 left, 0x08 bottom, 0x10 right, 0x20 thick,
    /// 0x40 double, 0x80 dotted.
    pub sides: u8,
    /// helpdeco's "width". Its meaning is unclear: `WIDGET.HLP` has 252
    /// and 79 on otherwise identical borders.
    pub width: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabStop {
    pub position: u16,
    /// 0 left, 1 right, 2 centre.
    pub kind: u16,
}

/// Paragraph formatting, shared by every paragraph of one `TOPICLINK`.
///
/// Distances are in the help compiler's units; helpdeco multiplies them by
/// 10 to get twips (so 72 is half an inch, 48 pixels at 96 dpi).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParaFormat {
    pub space_above: i16,
    pub space_below: i16,
    /// Line spacing (`\sl`); 0 means automatic.
    pub line_spacing: i16,
    pub left_indent: i16,
    pub right_indent: i16,
    /// Relative to `left_indent`; negative for hanging (bulleted) lines.
    pub first_line_indent: i16,
    pub align: Align,
    pub border: Option<Border>,
    pub tabs: Vec<TabStop>,
    /// The raw `bits` word (0x1000 keep lines together, 0x2000 keep with
    /// next are the ones not decoded above).
    pub bits: u16,
}

/// What a hotspot does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkAction {
    /// Jump to the topic for this context hash ([`HelpFile::topic_for_hash`]).
    Jump(u32),
    /// Show the topic for this context hash in a popup.
    Popup(u32),
    /// Run a help macro (`JumpID(...)` and so on).
    Macro(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub action: LinkAction,
    /// Visible hotspots are drawn green and underlined (solid for jumps,
    /// dotted for popups); invisible ones look like the surrounding text.
    pub visible: bool,
}

/// Where a picture sits: in the line, or floated to a margin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Inline,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Picture {
    /// An embedded window (`{ewc dll, class, data}`). `WIDGET.HLP` uses
    /// `D2HTools, D2H_256Color, helpNNNN.bmp`: Doc-To-Help's window that
    /// shows a baggage DIB ([`HelpFile::bitmap`]).
    Window {
        dll: String,
        class: String,
        data: String,
        /// The size the help compiler recorded (0 = ask the window).
        width: i16,
        height: i16,
    },
    /// A baggage bitmap shown as it is (an OS/2 book's picture,
    /// [`crate::inf::InfFile::to_help`]).
    Baggage(String),
}

impl Picture {
    /// The baggage file a `D2H_256Color` window (or a [`Picture::Baggage`])
    /// shows.
    #[must_use]
    pub fn baggage_bitmap(&self) -> Option<&str> {
        match self {
            Picture::Window { class, data, .. } if class.eq_ignore_ascii_case("D2H_256Color") => Some(data),
            Picture::Window { .. } => None,
            Picture::Baggage(name) => Some(name),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    /// Text in one font. `text` is in the font's own character set (see the
    /// module docs); 0xA0 is a non-breaking space.
    Text {
        text: Vec<u8>,
        font: u16,
        link: Option<Link>,
    },
    Tab,
    LineBreak,
    Picture {
        picture: Picture,
        placement: Placement,
        link: Option<Link>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Paragraph {
    pub format: ParaFormat,
    pub items: Vec<Inline>,
}

impl Paragraph {
    /// The paragraph's text as a `String` (tabs and breaks as `\t`/`\n`,
    /// pictures dropped), for tests and debugging.
    #[must_use]
    pub fn plain_text(&self) -> String {
        let mut s = String::new();
        for item in &self.items {
            match item {
                Inline::Text { text, .. } => s.push_str(&ansi(text)),
                Inline::Tab => s.push('\t'),
                Inline::LineBreak => s.push('\n'),
                Inline::Picture { .. } => {}
            }
        }
        s
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topic {
    /// Sequence number (0 is the first topic).
    pub number: u32,
    /// The topic's `TOPICOFFSET`, what `|CONTEXT` and the browse links hold.
    pub offset: u32,
    pub title: String,
    /// Entry macros (`!` footnotes).
    pub macros: Vec<String>,
    /// Browse sequence (the `<<`/`>>` buttons), as topic offsets.
    pub browse_prev: Option<u32>,
    pub browse_next: Option<u32>,
    /// The non-scrolling region at the top (the topic's heading band).
    pub non_scroll: Vec<Paragraph>,
    pub scroll: Vec<Paragraph>,
}

/// One keyword of the Index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyword {
    pub word: String,
    /// The topic offsets it leads to.
    pub topics: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpFile {
    /// `|SYSTEM` major and minor version (1, 21 for HC 3.1).
    pub version: (u16, u16),
    pub title: String,
    pub copyright: String,
    /// Topic offset of the contents topic (`HELP_CONTENTS` shows it).
    pub contents: u32,
    /// Macros run when the file is opened.
    pub macros: Vec<String>,
    pub windows: Vec<Window>,
    pub fonts: Vec<Font>,
    pub topics: Vec<Topic>,
    /// Context hash → topic offset, sorted by hash.
    pub contexts: Vec<(u32, u32)>,
    /// `[MAP]` id → topic offset (`HELP_CONTEXT` ids), in file order.
    pub map: Vec<(u32, u32)>,
    pub keywords: Vec<Keyword>,
    /// Baggage files by name.
    pub baggage: BTreeMap<String, Vec<u8>>,
}

/// One internal file's data.
fn internal(data: &[u8], at: usize) -> Result<&[u8], Error> {
    let mut c = Cur::at(data, at);
    c.u32()?; // reserved space
    let used = usize::try_from(c.u32()?).map_err(|_| Error::Truncated)?;
    c.u8()?; // flags
    c.bytes(used)
}

impl HelpFile {
    /// Parses a help file.
    ///
    /// # Errors
    /// See [`Error`].
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        let mut c = Cur::new(data);
        if c.u32()? != 0x0003_5f3f {
            return Err(Error::NotHelp);
        }
        let dir_at = usize::try_from(c.u32()?).map_err(|_| Error::Truncated)?;
        let mut files = BTreeMap::new();
        for (count, page) in btree_leaves(internal(data, dir_at)?)? {
            let mut e = Cur::new(page);
            for _ in 0..count {
                let name = ansi(e.cstr());
                let at = usize::try_from(e.u32()?).map_err(|_| Error::Truncated)?;
                files.insert(name, internal(data, at)?);
            }
        }
        let get = |name: &'static str| files.get(name).copied().ok_or(Error::Missing(name));

        let mut help = HelpFile {
            version: (0, 0),
            title: String::new(),
            copyright: String::new(),
            contents: 0,
            macros: Vec::new(),
            windows: Vec::new(),
            fonts: parse_fonts(get("|FONT")?)?,
            topics: Vec::new(),
            contexts: Vec::new(),
            map: Vec::new(),
            keywords: Vec::new(),
            baggage: files
                .iter()
                .filter(|(n, _)| !n.starts_with('|'))
                .map(|(n, d)| (n.clone(), d.to_vec()))
                .collect(),
        };
        let flags = help.parse_system(get("|SYSTEM")?)?;
        if files.contains_key("|PhrIndex") {
            return Err(Error::Unsupported("Hall phrase compression".into()));
        }
        let phrases = match files.get("|Phrases") {
            Some(p) => parse_phrases(p, flags != 0)?,
            None => Vec::new(),
        };
        help.topics = parse_topics(get("|TOPIC")?, flags, &phrases)?;

        if let Some(ctx) = files.get("|CONTEXT") {
            for (count, page) in btree_leaves(ctx)? {
                let mut e = Cur::new(page);
                for _ in 0..count {
                    help.contexts.push((e.u32()?, e.u32()?));
                }
            }
            help.contexts.sort_unstable();
        }
        if let Some(map) = files.get("|CTXOMAP") {
            let mut e = Cur::new(map);
            for _ in 0..e.u16()? {
                help.map.push((e.u32()?, e.u32()?));
            }
        }
        if let (Some(tree), Some(kw_data)) = (files.get("|KWBTREE"), files.get("|KWDATA")) {
            for (count, page) in btree_leaves(tree)? {
                let mut e = Cur::new(page);
                for _ in 0..count {
                    let word = ansi(e.cstr());
                    let n = e.u16()?;
                    let mut d = Cur::at(kw_data, usize::try_from(e.u32()?).map_err(|_| Error::Truncated)?);
                    let topics = (0..n).map(|_| d.u32()).collect::<Result<_, _>>()?;
                    help.keywords.push(Keyword { word, topics });
                }
            }
        }
        Ok(help)
    }

    /// Reads `|SYSTEM`; returns its compression flags.
    fn parse_system(&mut self, d: &[u8]) -> Result<u16, Error> {
        let mut c = Cur::new(d);
        if c.u16()? != 0x036c {
            return Err(Error::Corrupt("|SYSTEM signature"));
        }
        let minor = c.u16()?;
        let major = c.u16()?;
        self.version = (major, minor);
        c.u32()?; // build date
        let flags = c.u16()?;
        if minor <= 16 {
            return Err(Error::Unsupported("WinHelp 3.0 file".into()));
        }
        while !c.done() {
            let kind = c.u16()?;
            let len = usize::from(c.u16()?);
            let rec = c.bytes(len)?;
            let text = || ansi(rec.split(|&b| b == 0).next().unwrap_or_default());
            match kind {
                1 => self.title = text(),
                2 => self.copyright = text(),
                3 => self.contents = Cur::new(rec).u32()?,
                4 => self.macros.push(text()),
                6 => self.windows.push(Window::parse(rec)?),
                _ => {}
            }
        }
        Ok(flags)
    }

    /// The topic containing topic offset `offset`.
    #[must_use]
    pub fn topic_at(&self, offset: u32) -> Option<&Topic> {
        let i = self.topics.partition_point(|t| t.offset <= offset);
        self.topics.get(i.checked_sub(1)?)
    }

    /// The topic for a context-string hash (a hotspot's target).
    #[must_use]
    pub fn topic_for_hash(&self, hash: u32) -> Option<&Topic> {
        let i = self.contexts.binary_search_by_key(&hash, |&(h, _)| h).ok()?;
        self.topic_at(self.contexts[i].1)
    }

    /// The topic for a `[MAP]` id (`HELP_CONTEXT`'s argument).
    #[must_use]
    pub fn topic_for_map_id(&self, id: u32) -> Option<&Topic> {
        let &(_, offset) = self.map.iter().find(|&&(i, _)| i == id)?;
        self.topic_at(offset)
    }

    /// The topic `HELP_CONTENTS` opens.
    #[must_use]
    pub fn contents_topic(&self) -> Option<&Topic> {
        self.topic_at(self.contents)
    }

    /// Decodes a baggage bitmap by its (case-insensitive) file name.
    ///
    /// # Errors
    /// [`Error::Missing`] if there's no such file, [`Error::Bitmap`] if it
    /// doesn't decode.
    pub fn bitmap(&self, name: &str) -> Result<bmp::Bitmap, Error> {
        let data = self
            .baggage
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name.trim()))
            .map(|(_, d)| d)
            .ok_or(Error::Missing("baggage bitmap"))?;
        bmp::Bitmap::parse(data).map_err(Error::Bitmap)
    }
}

/// Reads `|Phrases` (HC 3.1: LZ77-compressed when the file is).
fn parse_phrases(d: &[u8], compressed: bool) -> Result<Vec<Vec<u8>>, Error> {
    let mut c = Cur::new(d);
    let n = usize::from(c.u16()?);
    if c.u16()? != 0x0100 {
        return Err(Error::Corrupt("|Phrases signature"));
    }
    if compressed {
        c.u32()?; // unpacked size
    }
    let offsets: Vec<usize> = (0..=n).map(|_| c.u16().map(usize::from)).collect::<Result<_, _>>()?;
    let rest = &d[c.at..];
    let text = if compressed { lz77(rest)? } else { rest.to_vec() };
    // Offsets count from the start of the offset table.
    let base = offsets[0];
    offsets
        .windows(2)
        .map(|w| {
            text.get(w[0].wrapping_sub(base)..w[1].wrapping_sub(base))
                .map(<[u8]>::to_vec)
                .ok_or(Error::Truncated)
        })
        .collect()
}

/// Replaces phrase references: a byte 1..=15 and the next form
/// `n = (b1 − 1) × 256 + b2`; phrase `n / 2`, plus a space if `n` is odd.
fn expand_phrases(d: &[u8], phrases: &[Vec<u8>]) -> Result<Vec<u8>, Error> {
    let mut out = Vec::with_capacity(d.len() * 2);
    let mut i = 0;
    while i < d.len() {
        let b = d[i];
        if (1..0x10).contains(&b) {
            let n = (usize::from(b) - 1) * 256 + usize::from(*d.get(i + 1).ok_or(Error::Truncated)?);
            out.extend_from_slice(phrases.get(n / 2).ok_or(Error::Corrupt("phrase number"))?);
            if n & 1 != 0 {
                out.push(b' ');
            }
            i += 2;
        } else {
            out.push(b);
            i += 1;
        }
    }
    Ok(out)
}

/// `|TOPIC`'s blocks, unpacked, with positions (`TOPICPOS`) counting the
/// 12-byte block header: position `p` is byte `p % span` of block
/// `p / span`.
struct TopicBlocks {
    blocks: Vec<Vec<u8>>,
    span: usize,
}

impl TopicBlocks {
    fn new(d: &[u8], flags: u16) -> Result<Self, Error> {
        let (size, compressed) = match flags {
            0 => (4096, false),
            4 => (4096, true),
            8 => (2048, true),
            _ => return Err(Error::Unsupported(format!("|SYSTEM flags {flags:#x}"))),
        };
        let blocks = d
            .chunks(size)
            .map(|b| {
                let body = b.get(12..).ok_or(Error::Truncated)?;
                let mut block = b[..12].to_vec();
                if compressed {
                    block.extend(lz77(body)?);
                } else {
                    block.extend_from_slice(body);
                }
                Ok(block)
            })
            .collect::<Result<_, Error>>()?;
        Ok(TopicBlocks {
            blocks,
            span: if compressed { 0x4000 } else { size },
        })
    }

    fn block_of(&self, pos: usize) -> usize {
        pos / self.span
    }

    /// `n` bytes from `pos`, continuing past a block's end after the next
    /// block's header.
    fn read(&self, pos: usize, n: usize) -> Result<Vec<u8>, Error> {
        let mut out = Vec::with_capacity(n);
        let (mut block, mut at) = (pos / self.span, pos % self.span);
        while out.len() < n {
            let b = self.blocks.get(block).ok_or(Error::Truncated)?;
            let take = (n - out.len()).min(b.len().saturating_sub(at));
            out.extend_from_slice(&b[at..at + take]);
            block += 1;
            at = 12;
        }
        Ok(out)
    }
}

/// Converts a `TOPICOFFSET`/`TOPICPOS` field, where −1 means none.
fn optional(v: i32) -> Option<u32> {
    (v != -1).then_some(v.cast_unsigned())
}

fn parse_topics(d: &[u8], flags: u16, phrases: &[Vec<u8>]) -> Result<Vec<Topic>, Error> {
    let blocks = TopicBlocks::new(d, flags)?;
    let first = Cur::at(blocks.blocks.first().ok_or(Error::Truncated)?, 4).i32()?;
    let mut topics: Vec<Topic> = Vec::new();
    // Where the current topic's scrolling region starts, if it has both.
    let mut scroll_from = None;
    // Topic offsets: block number × 0x8000 + characters of text since the
    // first record that starts in that block.
    let (mut offset_block, mut chars) = (usize::MAX, 0u32);
    let mut pos = first;
    let mut guard = 0;
    while pos != -1 {
        let at = usize::try_from(pos).map_err(|_| Error::Corrupt("topic link"))?;
        if blocks.block_of(at) >= blocks.blocks.len() {
            break;
        }
        guard += 1;
        if guard > 1_000_000 {
            return Err(Error::Corrupt("topic link loop"));
        }
        let header = blocks.read(at, 21)?;
        let mut h = Cur::new(&header);
        let size = usize::try_from(h.i32()?).map_err(|_| Error::Corrupt("topic link size"))?;
        let len2 = usize::try_from(h.i32()?).map_err(|_| Error::Corrupt("topic link size"))?;
        h.i32()?; // previous link
        let next = h.i32()?;
        let len1 = usize::try_from(h.i32()?).map_err(|_| Error::Corrupt("topic link size"))?;
        let kind = h.u8()?;
        let rec = blocks.read(at, size)?;
        let data1 = rec.get(21..len1).ok_or(Error::Truncated)?;
        let raw2 = rec.get(len1..).ok_or(Error::Truncated)?;
        let data2 = if len2 > raw2.len() {
            expand_phrases(raw2, phrases)?
        } else {
            raw2.to_vec()
        };

        let block = blocks.block_of(at);
        if block != offset_block {
            (offset_block, chars) = (block, 0);
        }
        let offset = u32::try_from(offset_block).map_err(|_| Error::Truncated)? * 0x8000 + chars;
        match kind {
            2 => {
                let mut c = Cur::new(data1);
                c.i32()?; // size of the topic's records
                let browse_prev = optional(c.i32()?);
                let browse_next = optional(c.i32()?);
                let number = c.u32()?;
                let non_scroll = c.i32()?;
                let scroll = c.i32()?;
                scroll_from = (non_scroll != -1).then_some(scroll);
                let mut parts = data2.split(|&b| b == 0);
                let title = ansi(parts.next().unwrap_or_default());
                topics.push(Topic {
                    number,
                    offset,
                    title,
                    macros: parts.filter(|m| !m.is_empty()).map(ansi).collect(),
                    browse_prev,
                    browse_next,
                    non_scroll: Vec::new(),
                    scroll: Vec::new(),
                });
            }
            0x20 => {
                let topic = topics.last_mut().ok_or(Error::Corrupt("text before the first topic"))?;
                let (length, paragraphs) = parse_paragraphs(data1, &data2)?;
                chars += u32::from(length);
                // Before the scroll start (or with no scrolling region at all),
                // records belong to the non-scrolling region.
                let in_non_scroll = match scroll_from {
                    Some(-1) => true,
                    Some(s) => pos < s,
                    None => false,
                };
                if in_non_scroll {
                    topic.non_scroll.extend(paragraphs);
                } else {
                    topic.scroll.extend(paragraphs);
                }
            }
            0x23 => return Err(Error::Unsupported("tables".into())),
            _ => return Err(Error::Unsupported(format!("topic record type {kind:#x}"))),
        }
        pos = next;
    }
    // The help compiler ends |TOPIC with an empty, untitled topic.
    if topics
        .last()
        .is_some_and(|t| t.title.is_empty() && t.non_scroll.is_empty() && t.scroll.is_empty())
    {
        topics.pop();
    }
    Ok(topics)
}

/// Reads a text record's paragraph formatting.
fn parse_format(c: &mut Cur) -> Result<ParaFormat, Error> {
    c.bytes(4)?; // unknown byte, biased byte, paragraph id
    let bits = c.u16()?;
    let mut format = ParaFormat {
        bits,
        ..ParaFormat::default()
    };
    if bits & 0x0001 != 0 {
        c.comp_long()?;
    }
    for (bit, field) in [
        (0x0002, &mut format.space_above),
        (0x0004, &mut format.space_below),
        (0x0008, &mut format.line_spacing),
        (0x0010, &mut format.left_indent),
        (0x0020, &mut format.right_indent),
        (0x0040, &mut format.first_line_indent),
    ] {
        if bits & bit != 0 {
            *field = c.comp_short()?;
        }
    }
    if bits & 0x0100 != 0 {
        let sides = c.u8()?;
        format.border = Some(Border { sides, width: c.i16()? });
    }
    if bits & 0x0200 != 0 {
        for _ in 0..c.comp_short()?.max(0) {
            let position = c.comp_ushort()?;
            let kind = if position & 0x4000 != 0 { c.comp_ushort()? } else { 0 };
            format.tabs.push(TabStop {
                position: position & 0x3fff,
                kind,
            });
        }
    }
    format.align = if bits & 0x0800 != 0 {
        Align::Centre
    } else if bits & 0x0400 != 0 {
        Align::Right
    } else {
        Align::Left
    };
    Ok(format)
}

/// Reads a picture command's data (after the 0x86..0x88 byte).
fn parse_picture(c: &mut Cur) -> Result<Picture, Error> {
    let kind = c.u8()?;
    let size = usize::try_from(c.comp_long()?).map_err(|_| Error::Corrupt("picture size"))?;
    let data = c.bytes(size)?;
    if kind != 5 {
        return Err(Error::Unsupported(format!("picture type {kind:#x}")));
    }
    let mut p = Cur::new(data);
    p.i16()?;
    let width = p.i16()?;
    let height = p.i16()?;
    let spec = ansi(p.cstr());
    let mut parts = spec.splitn(3, ',').map(|s| s.trim().to_string());
    Ok(Picture::Window {
        dll: parts.next().unwrap_or_default(),
        class: parts.next().unwrap_or_default(),
        data: parts.next().unwrap_or_default(),
        width,
        height,
    })
}

/// Reads a text record: its character count (for topic offsets) and its
/// paragraphs. `data1` holds formatting and commands, `data2` the text: one
/// NUL-terminated string before each command.
fn parse_paragraphs(data1: &[u8], data2: &[u8]) -> Result<(u16, Vec<Paragraph>), Error> {
    let mut c = Cur::new(data1);
    c.comp_long()?; // size of the rest of data1
    let length = c.comp_ushort()?;
    let format = parse_format(&mut c)?;

    let mut text = Cur::new(data2);
    let mut paragraphs = Vec::new();
    let mut items: Vec<Inline> = Vec::new();
    let (mut font, mut link) = (0u16, None::<Link>);
    // Appends to the last run if it has the same font and link.
    let push_text = |items: &mut Vec<Inline>, bytes: &[u8], font: u16, link: &Option<Link>| {
        if bytes.is_empty() {
            return;
        }
        if let Some(Inline::Text { text, font: f, link: l }) = items.last_mut()
            && *f == font
            && l == link
        {
            text.extend_from_slice(bytes);
            return;
        }
        items.push(Inline::Text {
            text: bytes.to_vec(),
            font,
            link: link.clone(),
        });
    };
    loop {
        push_text(&mut items, text.cstr(), font, &link);
        let cmd = c.u8()?;
        match cmd {
            0xff => break,
            0x20 => {
                c.bytes(4)?;
            }
            0x21 => {
                c.bytes(2)?;
            }
            0x80 => font = c.u16()?,
            0x81 => items.push(Inline::LineBreak),
            0x82 => paragraphs.push(Paragraph {
                format: format.clone(),
                items: std::mem::take(&mut items),
            }),
            0x83 => items.push(Inline::Tab),
            0x86..=0x88 => {
                let placement = match cmd {
                    0x86 => Placement::Inline,
                    0x87 => Placement::Left,
                    _ => Placement::Right,
                };
                items.push(Inline::Picture {
                    picture: parse_picture(&mut c)?,
                    placement,
                    link: link.clone(),
                });
            }
            0x89 => link = None,
            0x8b => push_text(&mut items, &[0xa0], font, &link),
            0x8c => push_text(&mut items, b"-", font, &link),
            0xc8 | 0xcc => {
                let n = usize::from(c.u16()?);
                let mut m = Cur::new(c.bytes(n)?);
                link = Some(Link {
                    action: LinkAction::Macro(ansi(m.cstr())),
                    visible: cmd == 0xc8,
                });
            }
            0xe2 | 0xe3 | 0xe6 | 0xe7 => {
                let hash = c.u32()?;
                link = Some(Link {
                    action: if cmd & 1 == 0 {
                        LinkAction::Popup(hash)
                    } else {
                        LinkAction::Jump(hash)
                    },
                    visible: cmd & 4 == 0,
                });
            }
            _ => return Err(Error::Unsupported(format!("text command {cmd:#x}"))),
        }
    }
    if !items.is_empty() {
        paragraphs.push(Paragraph { format, items });
    }
    Ok((length, paragraphs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lz77_back_references() {
        // "abc" literal, then a 6-byte copy from 3 back: "abcabcabc".
        let packed = [0b1000, b'a', b'b', b'c', 0x02, 0x30];
        assert_eq!(lz77(&packed).unwrap(), b"abcabcabc");
        assert_eq!(lz77(&[1, 0, 0]), Err(Error::Corrupt("LZ77 reference")));
    }

    #[test]
    fn compressed_numbers() {
        let mut c = Cur::new(&[0x1a, 0x80, 0x7b, 0x01, 0x3a, 0x91, 0x80]);
        assert_eq!(c.comp_long().unwrap(), 13);
        assert_eq!(c.comp_ushort().unwrap(), 189);
        assert_eq!(c.comp_short().unwrap(), -35);
        assert_eq!(c.comp_short().unwrap(), 72);
    }

    #[test]
    fn phrase_expansion() {
        let phrases = [b"Widget".to_vec(), b"Box".to_vec()];
        // Phrase 0, phrase 1 + space, a literal.
        assert_eq!(expand_phrases(&[1, 0, 1, 3, b'!'], &phrases).unwrap(), b"WidgetBox !");
    }

    /// The shipped `WIDGET.HLP` (extracted to `orig`; skipped without).
    fn shipped() -> Option<Vec<u8>> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../orig/WIN95/WIDGET/WIDGET.HLP");
        std::fs::read(p).ok()
    }

    #[test]
    fn reads_the_shipped_file() {
        let Some(file) = shipped() else {
            eprintln!("no orig: skipped");
            return;
        };
        let h = HelpFile::parse(&file).unwrap();
        assert_eq!(h.version, (1, 21));
        assert_eq!(h.title, "Widget Workshop");
        assert_eq!(h.copyright, "\u{ae} 1995 Maxis, Inc.");
        assert_eq!(h.macros.len(), 4);
        assert_eq!(h.macros[3], "BrowseButtons()");

        let titles: Vec<&str> = h.topics.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Help Contents",
                "What Is Widget Workshop, Anyway?",
                "What's a Widget?",
                "What Can I Do with Widget Workshop?",
                "Hey! Wait a Minute! Fun with Science???",
                "What's all that Other Stuff in the Box?",
                "The Widget Screen",
                "The Control Box",
                "The Tooltray",
                "The Menus",
                "Widget Troubleshooting",
            ]
        );
        let contents = h.contents_topic().unwrap();
        assert_eq!(contents.number, 0);
        assert_eq!(contents.non_scroll[0].plain_text(), "Contents");
        assert_eq!(contents.scroll[0].plain_text(), "Welcome to Widget Workshop");

        // Every context and map id lands exactly on a topic start, and the
        // map ids are the topics in order.
        assert_eq!(h.contexts.len(), 11);
        for &(_, off) in h.contexts.iter().chain(&h.map) {
            assert_eq!(h.topic_at(off).unwrap().offset, off);
        }
        for (id, t) in (1..).zip(&h.topics) {
            assert_eq!(h.topic_for_map_id(id).unwrap().title, t.title);
        }
        // Browse links point at topic starts too.
        for t in &h.topics {
            for off in t.browse_prev.into_iter().chain(t.browse_next) {
                assert_eq!(h.topic_at(off).unwrap().offset, off);
            }
        }

        // The contents page's first hotspot jumps to topic 1.
        let first_link = contents.scroll[1]
            .items
            .iter()
            .find_map(|i| match i {
                Inline::Text { link: Some(l), .. } => Some(l.clone()),
                _ => None,
            })
            .unwrap();
        assert!(first_link.visible);
        assert_eq!(first_link.action, LinkAction::Jump(0x584b_af94));
        assert_eq!(
            h.topic_for_hash(0x584b_af94).unwrap().title,
            "What Is Widget Workshop, Anyway?"
        );
        assert!(h.topic_for_hash(0x1234_5678).is_none());

        // Fonts: link text is font 5, Arial 11 pt bold; the dashes and
        // bullets are in Symbol.
        assert_eq!(h.fonts.len(), 15);
        assert_eq!(
            (h.fonts[5].face.as_str(), h.fonts[5].half_points, h.fonts[5].bold()),
            ("Arial", 22, true)
        );
        assert_eq!(h.fonts[11].face, "Symbol");
        assert!(h.fonts.iter().all(|f| f.colour.is_none()));
        assert_eq!(h.windows[0].caption.as_deref(), Some(" Widget Workshop"));

        // Pictures: six D2H windows showing baggage bitmaps.
        let pictures: Vec<&str> = h
            .topics
            .iter()
            .flat_map(|t| &t.scroll)
            .flat_map(|p| &p.items)
            .filter_map(|i| match i {
                Inline::Picture { picture, .. } => picture.baggage_bitmap(),
                _ => None,
            })
            .collect();
        assert_eq!(
            pictures,
            [
                "help0001.bmp",
                "help0002.bmp",
                "help0003.bmp",
                "help0004.bmp",
                "help0005.bmp",
                "help0006.bmp"
            ]
        );
        let screen = h.bitmap("help0001.bmp").unwrap();
        assert_eq!((screen.width, screen.height), (574, 431));
        assert_eq!(h.keywords.len(), 11);
        assert_eq!(h.keywords[0].word, "Help Contents");
        assert_eq!(h.keywords[0].topics, [0]);
    }
}
