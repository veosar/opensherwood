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
//! Everything is bounds-checked; malformed input yields a [`FormatError`], never a panic. Every
//! public decode route validates the frame record against a [`DecodeLimits`] policy before it
//! allocates anything, and allocations go through `try_reserve`, so a hostile record can neither
//! abort the process nor make it allocate more than the policy allows.

use crate::dic::{Dictionary, FrameRecord, NO_PAGE};
use crate::image_blob::{Image16, RgbaBudget, rgb565_to_rgb8};
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

/// Allocation policy for decoding one frame. Shared by every public decode route of this module,
/// by `opensherwood-assets`' sprite bank and by the `export-frame` tool, so a `FrameRecord` from an
/// untrusted `.dic` is checked the same way everywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeLimits {
    /// Largest width or height accepted.
    pub max_dimension: u16,
    /// Largest decoded frame accepted, in bytes of 16-bit pixels (`width * height * 2`). The RGBA8
    /// preview of [`to_rgba8_keyed`] is twice that.
    pub max_decoded_bytes: usize,
    /// Largest frame stream (`FrameRecord::length`, the bytes read from `robinhood.bks`) accepted.
    pub max_stream_bytes: u32,
}

impl DecodeLimits {
    /// Retail-safe policy: 4096 per dimension (retail maximum 674x583), 32 MiB decoded
    /// (4096 x 4096 x 2) and 64 MiB per stream (a 4096x4096 span frame is `2*w*h + 4*h` bytes,
    /// just over 32 MiB).
    pub const RETAIL: Self = Self {
        max_dimension: 4096,
        max_decoded_bytes: 4096 * 4096 * 2,
        max_stream_bytes: 64 * 1024 * 1024,
    };

    /// Validate a frame record against the policy. Returns the number of pixels of the frame.
    pub fn check_record(&self, rec: &FrameRecord) -> Result<usize, FormatError> {
        if rec.width > self.max_dimension || rec.height > self.max_dimension {
            return Err(FormatError::Invalid {
                offset: 0,
                what: "bks frame dimensions",
                value: format!(
                    "{}x{} (limit {} per side)",
                    rec.width, rec.height, self.max_dimension
                ),
            });
        }
        let pixels = usize::from(rec.width)
            .checked_mul(usize::from(rec.height))
            .ok_or_else(|| too_big(rec))?;
        let bytes = pixels.checked_mul(2).ok_or_else(|| too_big(rec))?;
        if bytes > self.max_decoded_bytes {
            return Err(too_big(rec));
        }
        if rec.length > self.max_stream_bytes {
            return Err(FormatError::Invalid {
                offset: 0,
                what: "bks frame stream length",
                value: format!("{} bytes (limit {})", rec.length, self.max_stream_bytes),
            });
        }
        Ok(pixels)
    }

    /// Validate the length of a stream that is about to be decoded.
    fn check_stream(&self, stream: &[u8]) -> Result<(), FormatError> {
        if u64::try_from(stream.len()).unwrap_or(u64::MAX) > u64::from(self.max_stream_bytes) {
            return Err(FormatError::Invalid {
                offset: 0,
                what: "bks frame stream length",
                value: format!("{} bytes (limit {})", stream.len(), self.max_stream_bytes),
            });
        }
        Ok(())
    }
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self::RETAIL
    }
}

fn too_big(rec: &FrameRecord) -> FormatError {
    FormatError::Invalid {
        offset: 0,
        what: "bks frame decoded size",
        value: format!(
            "{}x{} pixels exceeds the decode budget",
            rec.width, rec.height
        ),
    }
}

/// An empty pixel buffer with room for `pixels` entries obtained with `try_reserve`: an allocation
/// failure is a [`FormatError`], not an abort.
fn alloc_pixels(pixels: usize, rec: &FrameRecord) -> Result<Vec<u16>, FormatError> {
    let mut v = Vec::new();
    v.try_reserve_exact(pixels)
        .map_err(|_| FormatError::Invalid {
            offset: 0,
            what: "bks frame allocation",
            value: format!("{}x{} ({pixels} pixels)", rec.width, rec.height),
        })?;
    Ok(v)
}

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

/// Decode a frame from its record and its stream bytes, choosing the encoding by `rec.page`,
/// under the [`DecodeLimits::RETAIL`] policy.
pub fn decode_frame(
    rec: &FrameRecord,
    stream: &[u8],
    pages: &Pages,
) -> Result<Image16, FormatError> {
    decode_frame_with(rec, stream, pages, &DecodeLimits::RETAIL)
}

/// [`decode_frame`] under an explicit policy.
pub fn decode_frame_with(
    rec: &FrameRecord,
    stream: &[u8],
    pages: &Pages,
    limits: &DecodeLimits,
) -> Result<Image16, FormatError> {
    if rec.page == NO_PAGE {
        return decode_span_frame_with(rec, stream, limits);
    }
    let page = pages.page(rec.page).ok_or_else(|| FormatError::Invalid {
        offset: 0,
        what: "dic frame page",
        value: format!("page {} of {}", rec.page, pages.pages.len()),
    })?;
    decode_page_frame_with(rec, stream, page, limits)
}

/// Decode a dictionary-page frame: `ceil(width / 4) * height` symbols of 4 pixels each, under the
/// [`DecodeLimits::RETAIL`] policy.
pub fn decode_page_frame(
    rec: &FrameRecord,
    stream: &[u8],
    page: &Page,
) -> Result<Image16, FormatError> {
    decode_page_frame_with(rec, stream, page, &DecodeLimits::RETAIL)
}

/// [`decode_page_frame`] under an explicit policy.
pub fn decode_page_frame_with(
    rec: &FrameRecord,
    stream: &[u8],
    page: &Page,
    limits: &DecodeLimits,
) -> Result<Image16, FormatError> {
    let pixels_len = limits.check_record(rec)?;
    limits.check_stream(stream)?;
    let width = usize::from(rec.width);
    let height = usize::from(rec.height);
    let symbols_per_row = width.div_ceil(PIXELS_PER_SYMBOL);
    // Cannot overflow: both factors are bounded by `max_dimension`, a `u16`.
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
    let mut pixels = alloc_pixels(pixels_len, rec)?;
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

/// Decode a page-less frame: per row `first_x`, `last_x` and the pixels of that span, under the
/// [`DecodeLimits::RETAIL`] policy.
pub fn decode_span_frame(rec: &FrameRecord, stream: &[u8]) -> Result<Image16, FormatError> {
    decode_span_frame_with(rec, stream, &DecodeLimits::RETAIL)
}

/// [`decode_span_frame`] under an explicit policy.
pub fn decode_span_frame_with(
    rec: &FrameRecord,
    stream: &[u8],
    limits: &DecodeLimits,
) -> Result<Image16, FormatError> {
    let pixels_len = limits.check_record(rec)?;
    limits.check_stream(stream)?;
    let width = usize::from(rec.width);
    let height = usize::from(rec.height);
    let mut pixels = alloc_pixels(pixels_len, rec)?;
    pixels.resize(pixels_len, COLOR_KEY);
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

/// RGBA8 of one sprite pixel: [`COLOR_KEY`] fully transparent, [`SHADOW_KEY`] a half-transparent
/// black (a preview choice, not verified original behaviour), anything else opaque RGB565.
#[must_use]
pub fn keyed_rgba(p: u16) -> [u8; 4] {
    match p {
        COLOR_KEY => [0, 0, 0, 0],
        SHADOW_KEY => [0, 0, 0, 128],
        _ => {
            let (r, g, b) = rgb565_to_rgb8(p);
            [r, g, b, 255]
        }
    }
}

/// Convert a decoded frame to RGBA8 for previewing ([`keyed_rgba`] per pixel), charging the
/// frame's final size to `budget` before allocating (`Image16::try_to_rgba8_with`: the pixel
/// count is checked against the dimensions, the allocation is fallible).
pub fn to_rgba8_keyed_budgeted(
    img: &Image16,
    budget: &mut RgbaBudget,
) -> Result<Vec<u8>, FormatError> {
    img.try_to_rgba8_with(budget, keyed_rgba)
}

/// [`to_rgba8_keyed_budgeted`] for one frame on its own (no cumulative budget).
pub fn to_rgba8_keyed(img: &Image16) -> Result<Vec<u8>, FormatError> {
    let mut budget = RgbaBudget::UNBOUNDED;
    to_rgba8_keyed_budgeted(img, &mut budget)
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
        let rgba = to_rgba8_keyed(&img).unwrap();
        assert_eq!(rgba, vec![0, 0, 0, 0]);
    }

    #[test]
    fn rgba_preview_is_checked_and_budgeted() {
        let img = Image16 {
            width: 2,
            height: 1,
            pixels: vec![COLOR_KEY, SHADOW_KEY],
        };
        assert_eq!(
            to_rgba8_keyed(&img).unwrap(),
            vec![0, 0, 0, 0, 0, 0, 0, 128]
        );
        // Dimensions and pixel count must agree.
        let torn = Image16 {
            width: 2,
            height: 2,
            pixels: vec![0; 3],
        };
        assert!(to_rgba8_keyed(&torn).is_err());
        // The cumulative budget refuses the frame that does not fit and stays unchanged.
        let mut budget = RgbaBudget::new(12);
        assert!(to_rgba8_keyed_budgeted(&img, &mut budget).is_ok());
        assert_eq!(budget.used(), 8);
        assert!(to_rgba8_keyed_budgeted(&img, &mut budget).is_err());
        assert_eq!(budget.used(), 8);
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
    fn limits_reject_huge_records_before_allocating() {
        // 65535x65535 would be 8 GiB of u16 pixels: rejected by the dimension cap without touching
        // the allocator, for both encodings and on every public route.
        let huge = FrameRecord {
            width: u16::MAX,
            height: u16::MAX,
            offset: 0,
            length: 0,
            page: NO_PAGE,
        };
        let pages = Pages {
            pages: vec![Page {
                entries: vec![[0; 4]],
            }],
            frame_count: 0,
        };
        let stream = [0u8; 8];
        assert!(decode_frame(&huge, &stream, &pages).is_err());
        assert!(decode_span_frame(&huge, &stream).is_err());
        let huge_page = FrameRecord { page: 0, ..huge };
        assert!(decode_frame(&huge_page, &stream, &pages).is_err());
        assert!(decode_page_frame(&huge_page, &stream, &pages.pages[0]).is_err());
        // Within the per-side cap but over the byte budget under a tighter policy.
        let tight = DecodeLimits {
            max_dimension: 4096,
            max_decoded_bytes: 1024,
            max_stream_bytes: 1024,
        };
        let medium = FrameRecord {
            width: 64,
            height: 64,
            ..huge
        };
        assert!(tight.check_record(&medium).is_err());
        assert!(decode_span_frame_with(&medium, &stream, &tight).is_err());
        assert_eq!(DecodeLimits::RETAIL.check_record(&medium).unwrap(), 64 * 64);
        // A stream longer than the policy allows is refused even for a small frame.
        let long_stream = FrameRecord {
            width: 1,
            height: 1,
            length: u32::MAX,
            ..huge
        };
        assert!(DecodeLimits::RETAIL.check_record(&long_stream).is_err());
        assert!(
            decode_span_frame_with(
                &FrameRecord {
                    length: 0,
                    ..long_stream
                },
                &[0u8; 2048],
                &tight
            )
            .is_err()
        );
        // The retail-sized worst case is accepted by the policy (4096x4096 span frame).
        let max = FrameRecord {
            width: 4096,
            height: 4096,
            length: 2 * 4096 * 4096 + 4 * 4096,
            ..huge
        };
        assert_eq!(
            DecodeLimits::RETAIL.check_record(&max).unwrap(),
            4096 * 4096
        );
        assert!(
            DecodeLimits::RETAIL
                .check_record(&FrameRecord { width: 4097, ..max })
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
