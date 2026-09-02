//! Pixel decoding of sprite bank frames: the dictionary pages of `robinhood.dic` and the symbol /
//! span streams of `robinhood.bks`. Spec: `docs/formats/sprites.md` (section "Pixel data").
//!
//! Two encodings exist, selected by [`FrameRecord::page`]:
//!
//! * page `0..page_count`: the stream is `ceil(width / 4) * height` little-endian `u16` symbols, each
//!   naming a 4-pixel horizontal run in that page's dictionary (rows are padded to a multiple of 4
//!   pixels and the padding is discarded);
//! * page [`NO_PAGE`]: per row `u16 first_x`, `u16 last_x` (inclusive) and the RGB565 pixels in that
//!   span; `last_x == 0xFFFF` is an empty row. Pixels outside spans are [`COLOR_KEY`].
//!
//! Everything is bounds-checked; malformed input yields a [`FormatError`], never a panic.

use crate::dic::{Dictionary, FrameRecord, NO_PAGE};
use crate::image_blob::{Image16, rgb565_to_rgb8};
use crate::reader::{FormatError, Reader};

/// RGB565 value that is transparent in every frame (bright green, `0x07C0`).
pub const COLOR_KEY: u16 = 0x07C0;

/// RGB565 value observed only where a drop shadow would be drawn (pure blue, `0x001F`). The
/// original's treatment of it is not verified; see the spec before relying on its semantics.
pub const SHADOW_KEY: u16 = 0x001F;

/// Pixels per dictionary symbol (one 4x1 horizontal run).
pub const PIXELS_PER_SYMBOL: usize = 4;

/// Bytes per dictionary entry (`PIXELS_PER_SYMBOL` RGB565 pixels).
pub const ENTRY_SIZE: usize = PIXELS_PER_SYMBOL * 2;

/// `last_x` value marking an empty row in span-encoded (page-less) frames.
pub const EMPTY_ROW: u16 = 0xFFFF;

/// One dictionary page: a codebook of 4-pixel runs indexed by stream symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// Entries in symbol order; `entries[symbol]` is one run of 4 RGB565 pixels.
    pub entries: Vec<[u16; PIXELS_PER_SYMBOL]>,
}

/// All dictionary pages plus the frame count that terminates the dictionary region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pages {
    /// Pages in index order (`frame.page` indexes this).
    pub pages: Vec<Page>,
    /// The `u32` between the last page and the frame table; equals the number of frame records.
    pub frame_count: u32,
}

impl Pages {
    /// Page by index, if it exists.
    #[must_use]
    pub fn page(&self, index: u16) -> Option<&Page> {
        self.pages.get(usize::from(index))
    }
}

/// Parse the dictionary pages out of a parsed `.dic`.
///
/// The region between the header and the frame table is `page_count` pages of
/// `u16 entry_count` + `entry_count * 8` bytes followed by `u32 frame_count`. The header's third
/// field (`Dictionary::symbols_per_page`) is really the entry count of page 0, so the region slice
/// starts directly with page 0's entries. Error offsets are relative to `dictionary_region`.
pub fn parse_pages(dic: &Dictionary<'_>) -> Result<Pages, FormatError> {
    let mut r = Reader::new(dic.dictionary_region);
    let mut pages = Vec::with_capacity(usize::from(dic.page_count));
    for index in 0..dic.page_count {
        let count = if index == 0 {
            dic.symbols_per_page
        } else {
            r.u16("dic page entry count")?
        };
        let raw = r.bytes(usize::from(count) * ENTRY_SIZE, "dic page entries")?;
        let entries = raw
            .chunks_exact(ENTRY_SIZE)
            .map(|c| {
                [
                    u16::from_le_bytes([c[0], c[1]]),
                    u16::from_le_bytes([c[2], c[3]]),
                    u16::from_le_bytes([c[4], c[5]]),
                    u16::from_le_bytes([c[6], c[7]]),
                ]
            })
            .collect();
        pages.push(Page { entries });
    }
    let frame_count = r.u32("dic frame count")?;
    r.expect_end("dic frame count")?;
    Ok(Pages { pages, frame_count })
}

/// The bytes of one frame's stream inside a loaded `robinhood.bks`.
pub fn frame_stream<'a>(bks: &'a [u8], rec: &FrameRecord) -> Result<&'a [u8], FormatError> {
    let start = usize::try_from(rec.offset).map_err(|_| bad_range(rec))?;
    let len = usize::try_from(rec.length).map_err(|_| bad_range(rec))?;
    let end = start.checked_add(len).ok_or_else(|| bad_range(rec))?;
    bks.get(start..end).ok_or(FormatError::Eof {
        offset: start,
        needed: len,
        what: "bks frame stream",
    })
}

fn bad_range(rec: &FrameRecord) -> FormatError {
    FormatError::Invalid {
        offset: 0,
        what: "bks frame stream range",
        value: format!("offset {} length {}", rec.offset, rec.length),
    }
}

/// Decode a frame from its record and its stream bytes, choosing the encoding by `rec.page`.
pub fn decode_frame(
    rec: &FrameRecord,
    stream: &[u8],
    pages: &Pages,
) -> Result<Image16, FormatError> {
    if rec.page == NO_PAGE {
        return decode_span_frame(rec, stream);
    }
    let page = pages.page(rec.page).ok_or_else(|| FormatError::Invalid {
        offset: 0,
        what: "dic frame page",
        value: format!("page {} of {}", rec.page, pages.pages.len()),
    })?;
    decode_page_frame(rec, stream, page)
}

/// Decode a dictionary-page frame: `ceil(width / 4) * height` symbols of 4 pixels each.
pub fn decode_page_frame(
    rec: &FrameRecord,
    stream: &[u8],
    page: &Page,
) -> Result<Image16, FormatError> {
    let width = usize::from(rec.width);
    let height = usize::from(rec.height);
    let symbols_per_row = width.div_ceil(PIXELS_PER_SYMBOL);
    let expected = symbols_per_row * height * 2;
    if stream.len() != expected {
        return Err(FormatError::Invalid {
            offset: 0,
            what: "bks page frame stream length",
            value: format!(
                "{} bytes for {}x{} (expected {expected})",
                stream.len(),
                rec.width,
                rec.height
            ),
        });
    }
    let mut pixels = Vec::with_capacity(width * height);
    let mut r = Reader::new(stream);
    for _ in 0..height {
        let mut remaining = width;
        for _ in 0..symbols_per_row {
            let offset = r.pos();
            let symbol = r.u16("bks symbol")?;
            let run =
                page.entries
                    .get(usize::from(symbol))
                    .ok_or_else(|| FormatError::Invalid {
                        offset,
                        what: "bks symbol",
                        value: format!("{symbol} >= page size {}", page.entries.len()),
                    })?;
            let take = remaining.min(PIXELS_PER_SYMBOL);
            pixels.extend_from_slice(&run[..take]);
            remaining -= take;
        }
    }
    Ok(Image16 {
        width: rec.width,
        height: rec.height,
        pixels,
    })
}

/// Decode a page-less frame: per row `first_x`, `last_x` and the pixels of that span.
pub fn decode_span_frame(rec: &FrameRecord, stream: &[u8]) -> Result<Image16, FormatError> {
    let width = usize::from(rec.width);
    let height = usize::from(rec.height);
    let mut pixels = vec![COLOR_KEY; width * height];
    let mut r = Reader::new(stream);
    for y in 0..height {
        let offset = r.pos();
        let first = usize::from(r.u16("bks span first x")?);
        let last = r.u16("bks span last x")?;
        if last == EMPTY_ROW {
            continue;
        }
        let last = usize::from(last);
        if first > last || last >= width {
            return Err(FormatError::Invalid {
                offset,
                what: "bks span",
                value: format!("x {first}..={last} in row {y} of width {width}"),
            });
        }
        let raw = r.bytes((last - first + 1) * 2, "bks span pixels")?;
        let row = &mut pixels[y * width + first..=y * width + last];
        for (dst, src) in row.iter_mut().zip(raw.chunks_exact(2)) {
            *dst = u16::from_le_bytes([src[0], src[1]]);
        }
    }
    r.expect_end("bks span rows")?;
    Ok(Image16 {
        width: rec.width,
        height: rec.height,
        pixels,
    })
}

/// Convert a decoded frame to RGBA8 for previewing: [`COLOR_KEY`] becomes fully transparent and
/// [`SHADOW_KEY`] a half-transparent black (a preview choice, not verified original behaviour).
#[must_use]
pub fn to_rgba8_keyed(img: &Image16) -> Vec<u8> {
    let mut out = Vec::with_capacity(img.pixels.len() * 4);
    for &p in &img.pixels {
        match p {
            COLOR_KEY => out.extend_from_slice(&[0, 0, 0, 0]),
            SHADOW_KEY => out.extend_from_slice(&[0, 0, 0, 128]),
            _ => {
                let (r, g, b) = rgb565_to_rgb8(p);
                out.extend_from_slice(&[r, g, b, 255]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dic;

    fn le16(v: &[u16]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    fn dic_with_pages(
        page0: &[[u16; 4]],
        page1: &[[u16; 4]],
        frames: &[(u16, u16, u32, u32, u16)],
    ) -> Vec<u8> {
        let mut f = Vec::new();
        f.extend_from_slice(&dic::BANK_GENERATION_ID.to_le_bytes());
        f.extend_from_slice(&2u16.to_le_bytes());
        f.extend_from_slice(&(page0.len() as u16).to_le_bytes());
        for e in page0 {
            f.extend(le16(e));
        }
        f.extend_from_slice(&(page1.len() as u16).to_le_bytes());
        for e in page1 {
            f.extend(le16(e));
        }
        f.extend_from_slice(&(frames.len() as u32).to_le_bytes());
        for &(w, h, off, len, page) in frames {
            f.extend_from_slice(&w.to_le_bytes());
            f.extend_from_slice(&h.to_le_bytes());
            f.extend_from_slice(&off.to_le_bytes());
            f.extend_from_slice(&len.to_le_bytes());
            f.extend_from_slice(&page.to_le_bytes());
        }
        f
    }

    #[test]
    fn pages_and_page_frame_decode() {
        let page0 = [[COLOR_KEY; 4], [1, 2, 3, 4]];
        let page1 = [[9, 9, 9, 9]];
        // frame 0: 6x2 on page 0 = 2 symbols per row, 4 symbols; frame 1: 4x1 on page 1.
        let data = dic_with_pages(
            &page0,
            &page1,
            &[(6, 2, 0, 8, 0), (4, 1, 8, 2, 1), (1, 1, 10, 4, NO_PAGE)],
        );
        let d = dic::parse(&data).unwrap();
        let pages = parse_pages(&d).unwrap();
        assert_eq!(pages.pages.len(), 2);
        assert_eq!(pages.frame_count, 3);
        let bks = le16(&[1, 0, 0, 1, 0, 0, EMPTY_ROW]);
        let img = decode_frame(
            &d.frames[0],
            frame_stream(&bks, &d.frames[0]).unwrap(),
            &pages,
        )
        .unwrap();
        assert_eq!(img.width, 6);
        assert_eq!(
            img.pixels,
            vec![
                1, 2, 3, 4, COLOR_KEY, COLOR_KEY, COLOR_KEY, COLOR_KEY, COLOR_KEY, COLOR_KEY, 1, 2
            ]
        );
        let img = decode_frame(
            &d.frames[1],
            frame_stream(&bks, &d.frames[1]).unwrap(),
            &pages,
        )
        .unwrap();
        assert_eq!(img.pixels, vec![9, 9, 9, 9]);
        let img = decode_frame(
            &d.frames[2],
            frame_stream(&bks, &d.frames[2]).unwrap(),
            &pages,
        )
        .unwrap();
        assert_eq!(img.pixels, vec![COLOR_KEY]);
        let rgba = to_rgba8_keyed(&img);
        assert_eq!(rgba, vec![0, 0, 0, 0]);
    }

    #[test]
    fn span_frame_decode_and_errors() {
        let rec = FrameRecord {
            width: 4,
            height: 3,
            offset: 0,
            length: 0,
            page: NO_PAGE,
        };
        let stream = le16(&[0, EMPTY_ROW, 1, 2, 0xAAAA, 0xBBBB, 3, 3, SHADOW_KEY]);
        let img = decode_span_frame(&rec, &stream).unwrap();
        let k = COLOR_KEY;
        assert_eq!(
            img.pixels,
            vec![k, k, k, k, k, 0xAAAA, 0xBBBB, k, k, k, k, SHADOW_KEY]
        );
        // last_x beyond width, first > last, truncated pixels, trailing bytes: all errors.
        assert!(decode_span_frame(&rec, &le16(&[0, 4])).is_err());
        assert!(decode_span_frame(&rec, &le16(&[2, 1])).is_err());
        assert!(decode_span_frame(&rec, &le16(&[0, 3, 1, 2])).is_err());
        let mut extra = stream.clone();
        extra.push(0);
        assert!(decode_span_frame(&rec, &extra).is_err());
        // Page frame with an out-of-range symbol and a wrong length.
        let page = Page {
            entries: vec![[0; 4]],
        };
        let prec = FrameRecord { page: 0, ..rec };
        assert!(decode_page_frame(&prec, &le16(&[1, 0, 0]), &page).is_err());
        assert!(decode_page_frame(&prec, &le16(&[0, 0]), &page).is_err());
        assert!(
            frame_stream(
                &[0u8; 4],
                &FrameRecord {
                    offset: 2,
                    length: 4,
                    ..rec
                }
            )
            .is_err()
        );
    }

    #[test]
    fn garbage_does_not_panic() {
        for n in 0..200usize {
            let data: Vec<u8> = (0..n).map(|i| (i * 31 % 253) as u8).collect();
            let rec = FrameRecord {
                width: (n % 7) as u16,
                height: (n % 5) as u16,
                offset: 0,
                length: n as u32,
                page: if n % 2 == 0 { NO_PAGE } else { 0 },
            };
            let pages = Pages {
                pages: vec![Page {
                    entries: vec![[0; 4]; n % 9],
                }],
                frame_count: 0,
            };
            let _ = decode_frame(&rec, &data, &pages);
        }
    }
}
