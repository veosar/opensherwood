//! `SRES` resource archives. Spec: `docs/formats/sres.md`.
//!
//! Pictures are decompressed eagerly, so the parser enforces archive-wide budgets ([`Limits`]) on
//! top of the per-picture caps of `image_blob`: a hostile archive cannot amplify a few KiB of input
//! into gigabytes of decoded pixels.

use crate::image_blob::{self, Image16};
use crate::reader::{FormatError, Reader, tag_string};

/// Archive header version seen in retail data.
pub const VERSION: u32 = 0x100;

/// Archive-wide budgets. The retail maxima, measured over the four retail archives (GOG build,
/// 2026-09-02): 508 entries (`Level.res`), 1,134 pictures and 21.7 MiB of decoded pixels
/// (`DEFAULT.RES`), 128 pictures in one collection. [`Limits::RETAIL`] leaves generous headroom
/// while keeping the worst case a hostile archive can request at a few hundred MiB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Most top-level entries.
    pub max_entries: u32,
    /// Most pictures in one `PICC` collection or `CUR ` cursor.
    pub max_pictures_per_entry: u32,
    /// Most pictures in the whole archive (every `PIC `, collection, widget and cursor frame).
    pub max_images: usize,
    /// Most decoded picture bytes (16-bit pixels, `width * height * 2` per picture) in the whole
    /// archive. Checked from the picture header before anything is decompressed.
    pub max_decoded_bytes: usize,
}

impl Limits {
    /// Retail-safe policy: 65,536 entries, 4,096 pictures per entry, 16,384 pictures and 256 MiB
    /// of decoded pixels per archive.
    pub const RETAIL: Self = Self {
        max_entries: 65_536,
        max_pictures_per_entry: 4096,
        max_images: 16_384,
        max_decoded_bytes: 256 * 1024 * 1024,
    };
}

impl Default for Limits {
    fn default() -> Self {
        Self::RETAIL
    }
}

/// Running totals of one parse, checked against [`Limits`] before each picture is decompressed.
struct Budget<'l> {
    limits: &'l Limits,
    images: usize,
    decoded_bytes: usize,
}

impl Budget<'_> {
    /// Parse one picture at the reader's position, after charging its header to the budget.
    fn picture(&mut self, r: &mut Reader<'_>) -> Result<Image16, FormatError> {
        let offset = r.pos();
        let header = image_blob::parse_header(&mut r.clone())?;
        let bytes = usize::from(header.width) * usize::from(header.height) * 2;
        self.images += 1;
        if self.images > self.limits.max_images {
            return Err(FormatError::Invalid {
                offset,
                what: "SRES picture count",
                value: format!(
                    "more than {} pictures in the archive",
                    self.limits.max_images
                ),
            });
        }
        self.decoded_bytes = self.decoded_bytes.saturating_add(bytes);
        if self.decoded_bytes > self.limits.max_decoded_bytes {
            return Err(FormatError::Invalid {
                offset,
                what: "SRES decoded size",
                value: format!(
                    "{} bytes exceeds the archive budget of {}",
                    self.decoded_bytes, self.limits.max_decoded_bytes
                ),
            });
        }
        image_blob::parse(r)
    }

    /// Check a per-entry picture count against the policy.
    fn count(&self, n: u32, offset: usize, what: &'static str) -> Result<(), FormatError> {
        if n > self.limits.max_pictures_per_entry {
            return Err(FormatError::Invalid {
                offset,
                what,
                value: n.to_string(),
            });
        }
        Ok(())
    }
}

/// One entry of an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Four-character tag.
    pub tag: [u8; 4],
    /// Global resource id.
    pub id: u32,
    /// Always 0 in retail data; purpose unknown.
    pub unknown_0x08: u32,
    /// Offset of the entry in the file.
    pub offset: usize,
    /// Payload.
    pub body: Body,
}

/// UI widget kinds stored as picture sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetKind {
    /// `BTTN`: push button (3 or 4 states).
    Button,
    /// `RDO `: radio button (7 states).
    Radio,
    /// `NPTF`: text input field (6 pictures: backgrounds and caret).
    InputField,
    /// `SLID`: slider (6 pictures: knobs and track).
    Slider,
}

impl WidgetKind {
    /// Tag of the kind.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            WidgetKind::Button => "BTTN",
            WidgetKind::Radio => "RDO ",
            WidgetKind::InputField => "NPTF",
            WidgetKind::Slider => "SLID",
        }
    }
}

/// Entry payload by tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// `PIC `: one picture.
    Picture(Image16),
    /// `PICC`: a picture collection (icon set / animation frames).
    PictureCollection(Vec<Image16>),
    /// `BTTN` / `RDO ` / `NPTF` / `SLID`: a UI widget with one picture per set bit of a state mask
    /// (which bit is which visual state is not confirmed; see spec).
    Widget {
        /// Widget kind.
        kind: WidgetKind,
        /// State mask; the number of set bits is the number of pictures.
        states: u32,
        /// Pictures in ascending bit order.
        pictures: Vec<Image16>,
    },
    /// `CUR `: an animated mouse cursor.
    Cursor {
        /// Always 2 in retail data.
        unknown_0x0c: u16,
        /// Hotspot x.
        hotspot_x: u16,
        /// Hotspot y.
        hotspot_y: u16,
        /// 0 or 2; unknown (animation speed?).
        unknown_0x12: u16,
        /// Frames.
        frames: Vec<Image16>,
    },
    /// `TEXT`: UTF-16 strings.
    Text(Vec<String>),
    /// `WAVE`: Latin-1 paths relative to `Data\Sounds`.
    Wave(Vec<String>),
}

impl Body {
    /// Tag name for display.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Body::Picture(_) => "PIC ",
            Body::PictureCollection(_) => "PICC",
            Body::Widget { kind, .. } => kind.tag(),
            Body::Cursor { .. } => "CUR ",
            Body::Text(_) => "TEXT",
            Body::Wave(_) => "WAVE",
        }
    }
}

/// A parsed archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archive {
    /// Header version.
    pub version: u32,
    /// Entries in file order.
    pub entries: Vec<Entry>,
    /// Offset table from the trailer: one file offset per entry, in entry order, plus the trailer offset.
    pub offsets: Vec<u32>,
}

impl Archive {
    /// Find an entry by id.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }
}

/// Parse an archive under [`Limits::RETAIL`]. Pictures are decompressed eagerly.
pub fn parse(data: &[u8]) -> Result<Archive, FormatError> {
    parse_with(data, &Limits::RETAIL)
}

/// [`parse`] under an explicit budget.
pub fn parse_with(data: &[u8], limits: &Limits) -> Result<Archive, FormatError> {
    let mut r = Reader::new(data);
    r.expect(b"SRES", "SRES magic")?;
    let version = r.u32("SRES version")?;
    let count = r.u32("SRES entry count")?;
    // Every entry is at least 12 bytes, so a count the data cannot hold is rejected up front.
    if count > limits.max_entries || count as usize > r.remaining() / 12 {
        return Err(FormatError::Invalid {
            offset: r.pos() - 4,
            what: "SRES entry count",
            value: count.to_string(),
        });
    }
    let mut budget = Budget {
        limits,
        images: 0,
        decoded_bytes: 0,
    };
    let mut entries = Vec::new();
    for _ in 0..count {
        let offset = r.pos();
        let tag = r.tag("SRES entry tag")?;
        let id = r.u32("SRES entry id")?;
        let unknown_0x08 = r.u32("SRES entry unknown_0x08")?;
        let body = match &tag {
            b"PIC " => Body::Picture(budget.picture(&mut r)?),
            b"PICC" => {
                let n = r.u32("SRES picture count")?;
                budget.count(n, r.pos() - 4, "SRES picture count")?;
                let mut pics = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    pics.push(budget.picture(&mut r)?);
                }
                Body::PictureCollection(pics)
            }
            b"BTTN" | b"RDO " | b"NPTF" | b"SLID" => {
                let kind = match &tag {
                    b"BTTN" => WidgetKind::Button,
                    b"RDO " => WidgetKind::Radio,
                    b"NPTF" => WidgetKind::InputField,
                    _ => WidgetKind::Slider,
                };
                let states = r.u32("SRES widget states")?;
                let mut pictures = Vec::with_capacity(states.count_ones() as usize);
                for _ in 0..states.count_ones() {
                    pictures.push(budget.picture(&mut r)?);
                }
                Body::Widget {
                    kind,
                    states,
                    pictures,
                }
            }
            b"CUR " => {
                let unknown_0x0c = r.u16("SRES cursor unknown_0x0c")?;
                let hotspot_x = r.u16("SRES cursor hotspot x")?;
                let hotspot_y = r.u16("SRES cursor hotspot y")?;
                let unknown_0x12 = r.u16("SRES cursor unknown_0x12")?;
                let n = r.u32("SRES cursor frame count")?;
                budget.count(n, r.pos() - 4, "SRES cursor frame count")?;
                let mut frames = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    frames.push(budget.picture(&mut r)?);
                }
                Body::Cursor {
                    unknown_0x0c,
                    hotspot_x,
                    hotspot_y,
                    unknown_0x12,
                    frames,
                }
            }
            b"TEXT" => {
                let n = r.u16("SRES text count")?;
                let mut v = Vec::with_capacity(usize::from(n));
                for _ in 0..n {
                    v.push(r.pstring16_utf16("SRES text")?);
                }
                Body::Text(v)
            }
            b"WAVE" => {
                let n = r.u16("SRES wave count")?;
                let mut v = Vec::with_capacity(usize::from(n));
                for _ in 0..n {
                    v.push(r.pstring16("SRES wave path")?);
                }
                Body::Wave(v)
            }
            other => {
                return Err(FormatError::BadMagic {
                    offset,
                    expected: "PIC /PICC/BTTN/RDO /NPTF/SLID/CUR /TEXT/WAVE".into(),
                    found: tag_string(*other),
                });
            }
        };
        entries.push(Entry {
            tag,
            id,
            unknown_0x08,
            offset,
            body,
        });
    }
    // Trailer: one u32 offset per entry (the first is always 12, the end of the header) plus a final
    // sentinel equal to the offset of the trailer itself.
    let trailer_start = r.pos();
    let mut offsets = Vec::new();
    if r.remaining() == 0 {
        return Ok(Archive {
            version,
            entries,
            offsets,
        });
    }
    offsets.reserve(entries.len() + 1);
    for expected in entries
        .iter()
        .map(|e| e.offset)
        .chain(std::iter::once(trailer_start))
    {
        let off = r.u32("SRES trailer offset")?;
        if off as usize != expected {
            return Err(FormatError::Invalid {
                offset: r.pos() - 4,
                what: "SRES trailer offset",
                value: format!("{off} (expected {expected})"),
            });
        }
        offsets.push(off);
    }
    r.expect_end("SRES trailer")?;
    Ok(Archive {
        version,
        entries,
        offsets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_text_and_wave_entries() {
        let mut f = Vec::new();
        f.extend_from_slice(b"SRES");
        f.extend_from_slice(&VERSION.to_le_bytes());
        f.extend_from_slice(&2u32.to_le_bytes());
        // TEXT id 7 with two strings
        f.extend_from_slice(b"TEXT");
        f.extend_from_slice(&7u32.to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&2u16.to_le_bytes());
        for s in ["Hi", "yo"] {
            f.extend_from_slice(&(s.len() as u16).to_le_bytes());
            for u in s.encode_utf16() {
                f.extend_from_slice(&u.to_le_bytes());
            }
        }
        // WAVE id 9 with one path
        f.extend_from_slice(b"WAVE");
        f.extend_from_slice(&9u32.to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&1u16.to_le_bytes());
        f.extend_from_slice(&5u16.to_le_bytes());
        f.extend_from_slice(b"a.wav");
        // trailer: offsets of the two entries
        let trailer_start = f.len() as u32;
        f.extend_from_slice(&12u32.to_le_bytes());
        f.extend_from_slice(&38u32.to_le_bytes());
        f.extend_from_slice(&trailer_start.to_le_bytes());
        let a = parse(&f).unwrap();
        assert_eq!(a.entries.len(), 2);
        assert_eq!(
            a.get(7).unwrap().body,
            Body::Text(vec!["Hi".into(), "yo".into()])
        );
        assert_eq!(a.get(9).unwrap().body, Body::Wave(vec!["a.wav".into()]));
        assert_eq!(a.offsets.len(), 3);
        assert_eq!(&a.offsets[..2], &[12, 38]);
    }

    #[test]
    fn garbage_does_not_panic() {
        for n in 0..96usize {
            let mut data: Vec<u8> = b"SRES".to_vec();
            data.extend((0..n).map(|i| (i * 53 % 251) as u8));
            let _ = parse(&data);
        }
    }

    /// A valid zlib image blob of `w` x `h` zero pixels.
    fn blob(w: u16, h: u16) -> Vec<u8> {
        use std::io::Write as _;
        let raw = vec![0u8; usize::from(w) * usize::from(h) * 2];
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&raw).unwrap();
        let payload = enc.finish().unwrap();
        let mut b = Vec::new();
        b.extend_from_slice(&w.to_le_bytes());
        b.extend_from_slice(&h.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        b.extend_from_slice(&payload);
        b
    }

    /// An archive of `entries` x `PICC` collections, each holding `per_entry` pictures of `w` x `h`.
    fn picc_archive(entries: u32, per_entry: u32, w: u16, h: u16) -> Vec<u8> {
        let pic = blob(w, h);
        let mut f = Vec::new();
        f.extend_from_slice(b"SRES");
        f.extend_from_slice(&VERSION.to_le_bytes());
        f.extend_from_slice(&entries.to_le_bytes());
        for id in 0..entries {
            f.extend_from_slice(b"PICC");
            f.extend_from_slice(&id.to_le_bytes());
            f.extend_from_slice(&0u32.to_le_bytes());
            f.extend_from_slice(&per_entry.to_le_bytes());
            for _ in 0..per_entry {
                f.extend_from_slice(&pic);
            }
        }
        f
    }

    #[test]
    fn archive_budgets_stop_amplification() {
        let tight = Limits {
            max_entries: 4,
            max_pictures_per_entry: 8,
            max_images: 10,
            max_decoded_bytes: 10 * 4 * 4 * 2,
        };
        // Within every budget: 2 entries x 5 pictures of 4x4 = 10 pictures, 320 bytes.
        let ok = picc_archive(2, 5, 4, 4);
        assert_eq!(parse_with(&ok, &tight).unwrap().entries.len(), 2);
        assert_eq!(parse(&ok).unwrap().entries.len(), 2);
        // One picture too many across the archive, although each entry is within its own cap.
        let too_many = picc_archive(2, 6, 4, 4);
        let err = parse_with(&too_many, &tight).unwrap_err();
        assert!(err.to_string().contains("picture count"), "{err}");
        // Same picture count but bigger pictures: the decoded-byte budget trips (from the header,
        // before decompression, so the offending blob does not even need a valid payload).
        let mut too_big = picc_archive(2, 5, 4, 4);
        let last_blob = too_big.len() - blob(4, 4).len();
        too_big[last_blob..last_blob + 2].copy_from_slice(&8u16.to_le_bytes());
        let err = parse_with(&too_big, &tight).unwrap_err();
        assert!(err.to_string().contains("decoded size"), "{err}");
        // Entry count above the policy, and a count the data cannot hold.
        let err = parse_with(&picc_archive(5, 1, 4, 4), &tight).unwrap_err();
        assert!(err.to_string().contains("entry count"), "{err}");
        let mut absurd = picc_archive(1, 1, 4, 4);
        absurd[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        let err = parse(&absurd).unwrap_err();
        assert!(err.to_string().contains("entry count"), "{err}");
        // Per-entry cap.
        let err = parse_with(&picc_archive(1, 9, 4, 4), &tight).unwrap_err();
        assert!(err.to_string().contains("picture count"), "{err}");
    }
}
