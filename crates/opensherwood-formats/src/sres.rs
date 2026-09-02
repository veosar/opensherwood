//! `SRES` resource archives. Spec: `docs/formats/sres.md`.

use crate::image_blob::{self, Image16};
use crate::reader::{FormatError, Reader, tag_string};

/// Archive header version seen in retail data.
pub const VERSION: u32 = 0x100;

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

/// Parse an archive. Pictures are decompressed eagerly.
pub fn parse(data: &[u8]) -> Result<Archive, FormatError> {
    let mut r = Reader::new(data);
    r.expect(b"SRES", "SRES magic")?;
    let version = r.u32("SRES version")?;
    let count = r.u32("SRES entry count")?;
    let mut entries = Vec::new();
    for _ in 0..count {
        let offset = r.pos();
        let tag = r.tag("SRES entry tag")?;
        let id = r.u32("SRES entry id")?;
        let unknown_0x08 = r.u32("SRES entry unknown_0x08")?;
        let body = match &tag {
            b"PIC " => Body::Picture(image_blob::parse(&mut r)?),
            b"PICC" => {
                let n = r.u32("SRES picture count")?;
                if n > 4096 {
                    return Err(FormatError::Invalid {
                        offset: r.pos() - 4,
                        what: "SRES picture count",
                        value: n.to_string(),
                    });
                }
                let mut pics = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    pics.push(image_blob::parse(&mut r)?);
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
                    pictures.push(image_blob::parse(&mut r)?);
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
                if n > 4096 {
                    return Err(FormatError::Invalid {
                        offset: r.pos() - 4,
                        what: "SRES cursor frame count",
                        value: n.to_string(),
                    });
                }
                let mut frames = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    frames.push(image_blob::parse(&mut r)?);
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
}
