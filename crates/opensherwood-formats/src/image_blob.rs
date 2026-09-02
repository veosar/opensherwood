//! Compressed 16-bit pictures: `.map`, `.min`, `.pak`, `.sxt`, save thumbnails and SRES pictures.
//! Spec: `docs/formats/image-blob.md`.

use std::io::Read;

use crate::reader::{FormatError, Reader};

/// Compression codec of an image blob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// zlib / deflate (`flag = 1`).
    Zlib,
    /// bzip2 (`flag = 2`).
    Bzip2,
}

/// Header of an image blob (12 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageHeader {
    /// Width in pixels.
    pub width: u16,
    /// Height in pixels.
    pub height: u16,
    /// Codec.
    pub compression: Compression,
    /// Size of the compressed payload.
    pub compressed_size: u32,
}

/// Header size in bytes.
pub const HEADER_SIZE: usize = 12;

/// Largest width or height accepted (the biggest retail background is 2304x3520).
pub const MAX_DIMENSION: u16 = 8192;

/// Largest decoded picture accepted, in bytes (8192 x 8192 x 2).
pub const MAX_DECODED_BYTES: usize = 8192 * 8192 * 2;

/// A decoded picture: 16 bits per pixel, row-major, `width * height` entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image16 {
    /// Width in pixels.
    pub width: u16,
    /// Height in pixels.
    pub height: u16,
    /// Raw 16-bit pixels as stored (little-endian words).
    pub pixels: Vec<u16>,
}

/// Cumulative budget of RGBA8 bytes materialised from decoded pictures: every conversion through
/// [`Image16::try_to_rgba8_with`] is charged its final size before it allocates, so a caller that
/// converts many pictures (a sprite cache, the menu assets) bounds the total, not only each one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbaBudget {
    limit: usize,
    used: usize,
}

impl RgbaBudget {
    /// A budget that never refuses (single conversions whose size the decode limits already bound).
    pub const UNBOUNDED: Self = Self::new(usize::MAX);

    /// A budget of `limit` bytes.
    #[must_use]
    pub const fn new(limit: usize) -> Self {
        Self { limit, used: 0 }
    }

    /// Bytes the budget allows in total.
    #[must_use]
    pub const fn limit(&self) -> usize {
        self.limit
    }

    /// Bytes charged so far.
    #[must_use]
    pub const fn used(&self) -> usize {
        self.used
    }

    /// Bytes still available.
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.limit.saturating_sub(self.used)
    }

    /// Charge `bytes`; refused (and unchanged) when they do not fit.
    pub fn charge(&mut self, bytes: usize) -> Result<(), FormatError> {
        if bytes > self.remaining() {
            return Err(FormatError::Invalid {
                offset: 0,
                what: "rgba budget",
                value: format!(
                    "{bytes} bytes requested, {} of {} left",
                    self.remaining(),
                    self.limit
                ),
            });
        }
        self.used += bytes;
        Ok(())
    }

    /// Give `bytes` back (a cache dropped a picture); saturating.
    pub fn release(&mut self, bytes: usize) {
        self.used = self.used.saturating_sub(bytes);
    }

    /// Give everything back (a cache was cleared).
    pub fn reset(&mut self) {
        self.used = 0;
    }
}

impl Image16 {
    /// Bytes the RGBA8 form of this picture takes: `width * height * 4`, refused when the pixel
    /// buffer does not hold exactly `width * height` entries or the product overflows.
    pub fn rgba8_len(&self) -> Result<usize, FormatError> {
        let pixels = usize::from(self.width) * usize::from(self.height);
        if pixels != self.pixels.len() {
            return Err(FormatError::Invalid {
                offset: 0,
                what: "image pixel count",
                value: format!(
                    "{}x{} needs {pixels} pixels, {} present",
                    self.width,
                    self.height,
                    self.pixels.len()
                ),
            });
        }
        pixels.checked_mul(4).ok_or_else(|| FormatError::Invalid {
            offset: 0,
            what: "image rgba size",
            value: format!("{}x{} overflows", self.width, self.height),
        })
    }

    /// Convert to RGBA8 with `map` deciding every pixel's colour and alpha. The output size is
    /// checked ([`Image16::rgba8_len`]) and charged to `budget` before a fallible allocation
    /// (`try_reserve_exact`); an allocation failure is an error, not an abort.
    pub fn try_to_rgba8_with(
        &self,
        budget: &mut RgbaBudget,
        map: impl Fn(u16) -> [u8; 4],
    ) -> Result<Vec<u8>, FormatError> {
        let bytes = self.rgba8_len()?;
        budget.charge(bytes)?;
        let mut out = Vec::new();
        if let Err(e) = out.try_reserve_exact(bytes) {
            budget.release(bytes);
            let _ = e;
            return Err(FormatError::Invalid {
                offset: 0,
                what: "image rgba allocation",
                value: format!("{}x{} ({bytes} bytes)", self.width, self.height),
            });
        }
        for &p in &self.pixels {
            out.extend_from_slice(&map(p));
        }
        Ok(out)
    }

    /// Opaque RGBA8 assuming RGB565 (channel order still `partial` in the spec), charged to
    /// `budget`.
    pub fn try_to_rgba8_565(&self, budget: &mut RgbaBudget) -> Result<Vec<u8>, FormatError> {
        self.try_to_rgba8_with(budget, |p| {
            let (r, g, b) = rgb565_to_rgb8(p);
            [r, g, b, 255]
        })
    }

    /// Opaque RGBA8 assuming RGB565, one picture on its own (size checks and fallible allocation,
    /// no cumulative budget).
    pub fn to_rgba8_565(&self) -> Result<Vec<u8>, FormatError> {
        let mut budget = RgbaBudget::UNBOUNDED;
        self.try_to_rgba8_565(&mut budget)
    }
}

/// Expand an RGB565 word to 8-bit channels (with bit replication).
#[must_use]
pub fn rgb565_to_rgb8(p: u16) -> (u8, u8, u8) {
    let r5 = (p >> 11) & 0x1f;
    let g6 = (p >> 5) & 0x3f;
    let b5 = p & 0x1f;
    let r = ((r5 << 3) | (r5 >> 2)) as u8;
    let g = ((g6 << 2) | (g6 >> 4)) as u8;
    let b = ((b5 << 3) | (b5 >> 2)) as u8;
    (r, g, b)
}

/// Parse only the header at the reader's position.
pub fn parse_header(r: &mut Reader<'_>) -> Result<ImageHeader, FormatError> {
    let start = r.pos();
    let width = r.u16("image width")?;
    let height = r.u16("image height")?;
    let flag = r.u32("image compression flag")?;
    let compressed_size = r.u32("image compressed size")?;
    let compression = match flag {
        1 => Compression::Zlib,
        2 => Compression::Bzip2,
        other => {
            return Err(FormatError::Invalid {
                offset: start + 4,
                what: "image compression flag",
                value: other.to_string(),
            });
        }
    };
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(FormatError::Invalid {
            offset: start,
            what: "image dimensions",
            value: format!("{width}x{height}"),
        });
    }
    Ok(ImageHeader {
        width,
        height,
        compression,
        compressed_size,
    })
}

/// Cheap check used by [`crate::detect`]: header fields plausible and payload size matches.
#[must_use]
pub fn looks_like_image_blob(data: &[u8]) -> bool {
    let mut r = Reader::new(data);
    let Ok(h) = parse_header(&mut r) else {
        return false;
    };
    if data.len() < HEADER_SIZE + h.compressed_size as usize {
        return false;
    }
    let payload = &data[HEADER_SIZE..HEADER_SIZE + h.compressed_size as usize];
    match h.compression {
        Compression::Zlib => payload.len() >= 2 && payload[0] == 0x78,
        Compression::Bzip2 => payload.starts_with(b"BZh"),
    }
}

/// Parse and decompress a blob at the reader's position; advances past the payload.
pub fn parse(r: &mut Reader<'_>) -> Result<Image16, FormatError> {
    let start = r.pos();
    let header = parse_header(r)?;
    let payload_offset = r.pos();
    let payload = r.bytes(header.compressed_size as usize, "image payload")?;
    let expected = usize::from(header.width) * usize::from(header.height) * 2;
    if expected > MAX_DECODED_BYTES {
        return Err(FormatError::Invalid {
            offset: start,
            what: "decoded image size",
            value: expected.to_string(),
        });
    }
    let raw = decompress(header.compression, payload, expected, payload_offset)?;
    if raw.len() != expected {
        return Err(FormatError::Invalid {
            offset: start,
            what: "decompressed image size",
            value: format!("{} (expected {expected})", raw.len()),
        });
    }
    let pixels = raw
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    Ok(Image16 {
        width: header.width,
        height: header.height,
        pixels,
    })
}

/// Parse a whole file holding exactly one blob.
pub fn parse_file(data: &[u8]) -> Result<Image16, FormatError> {
    let mut r = Reader::new(data);
    let img = parse(&mut r)?;
    r.expect_end("image blob")?;
    Ok(img)
}

/// Parse a file holding one or more blobs back to back (`.pak` slide shows and loading screens).
pub fn parse_sequence(data: &[u8]) -> Result<Vec<Image16>, FormatError> {
    let mut r = Reader::new(data);
    let mut out = Vec::new();
    while r.remaining() > 0 {
        out.push(parse(&mut r)?);
        if out.len() > 4096 {
            return Err(FormatError::Invalid {
                offset: r.pos(),
                what: "image count",
                value: "> 4096".into(),
            });
        }
    }
    if out.is_empty() {
        return Err(FormatError::Eof {
            offset: 0,
            needed: HEADER_SIZE,
            what: "image blob",
        });
    }
    Ok(out)
}

fn decompress(
    codec: Compression,
    payload: &[u8],
    expected: usize,
    offset: usize,
) -> Result<Vec<u8>, FormatError> {
    // Cap the output at the expected size plus one byte so a hostile stream cannot allocate unboundedly.
    let limit = expected as u64 + 1;
    let mut out = Vec::with_capacity(expected);
    let result = match codec {
        Compression::Zlib => flate2::read::ZlibDecoder::new(payload)
            .take(limit)
            .read_to_end(&mut out)
            .map_err(|e| e.to_string()),
        Compression::Bzip2 => bzip2::read::BzDecoder::new(payload)
            .take(limit)
            .read_to_end(&mut out)
            .map_err(|e| e.to_string()),
    };
    match result {
        Ok(_) => Ok(out),
        Err(message) => Err(FormatError::Decompress {
            codec: match codec {
                Compression::Zlib => "zlib",
                Compression::Bzip2 => "bzip2",
            },
            offset,
            message,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    #[test]
    fn round_trip_zlib_blob() {
        let pixels: Vec<u16> = (0..6u16).collect();
        let raw: Vec<u8> = pixels.iter().flat_map(|p| p.to_le_bytes()).collect();
        let payload = zlib(&raw);
        let mut file = Vec::new();
        file.extend_from_slice(&3u16.to_le_bytes());
        file.extend_from_slice(&2u16.to_le_bytes());
        file.extend_from_slice(&1u32.to_le_bytes());
        file.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        file.extend_from_slice(&payload);
        assert!(looks_like_image_blob(&file));
        let img = parse_file(&file).unwrap();
        assert_eq!(img.width, 3);
        assert_eq!(img.height, 2);
        assert_eq!(img.pixels, pixels);
    }

    #[test]
    fn rejects_wrong_size() {
        let payload = zlib(&[0u8; 4]);
        let mut file = Vec::new();
        file.extend_from_slice(&3u16.to_le_bytes());
        file.extend_from_slice(&2u16.to_le_bytes());
        file.extend_from_slice(&1u32.to_le_bytes());
        file.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        file.extend_from_slice(&payload);
        assert!(parse_file(&file).is_err());
    }

    #[test]
    fn garbage_does_not_panic() {
        for n in 0..64usize {
            let data: Vec<u8> = (0..n).map(|i| (i * 37 % 251) as u8).collect();
            let _ = parse_file(&data);
            let _ = looks_like_image_blob(&data);
        }
    }

    #[test]
    fn rgba_conversion_checks_sizes_and_charges_the_budget() {
        let img = Image16 {
            width: 3,
            height: 2,
            pixels: vec![0, 0xffff, 0x07e0, 0, 0, 0],
        };
        assert_eq!(img.rgba8_len().unwrap(), 24);
        let rgba = img.to_rgba8_565().unwrap();
        assert_eq!(rgba.len(), 24);
        assert_eq!(&rgba[4..8], &[255, 255, 255, 255]);
        assert_eq!(&rgba[8..12], &[0, 255, 0, 255]);
        // A pixel buffer that does not match the dimensions is refused, never indexed short.
        let short = Image16 {
            width: 3,
            height: 2,
            pixels: vec![0; 5],
        };
        assert!(short.rgba8_len().is_err());
        assert!(short.to_rgba8_565().is_err());
        let long = Image16 {
            width: 1,
            height: 1,
            pixels: vec![0; 2],
        };
        assert!(long.to_rgba8_565().is_err());
        // The budget is cumulative: two pictures fit, the third is refused and charges nothing.
        let mut budget = RgbaBudget::new(50);
        assert!(img.try_to_rgba8_565(&mut budget).is_ok());
        assert!(img.try_to_rgba8_565(&mut budget).is_ok());
        assert_eq!(budget.used(), 48);
        assert!(img.try_to_rgba8_565(&mut budget).is_err());
        assert_eq!(budget.used(), 48);
        budget.release(24);
        assert!(img.try_to_rgba8_with(&mut budget, |_| [1, 2, 3, 4]).is_ok());
        assert_eq!(budget.remaining(), 2);
        budget.reset();
        assert_eq!(budget.remaining(), 50);
        // A single picture larger than the budget is refused up front.
        assert!(img.try_to_rgba8_565(&mut RgbaBudget::new(23)).is_err());
        // The unbounded budget still validates the pixel count.
        let mut unbounded = RgbaBudget::UNBOUNDED;
        assert!(short.try_to_rgba8_565(&mut unbounded).is_err());
    }

    #[test]
    fn rgb565_expands_extremes() {
        assert_eq!(rgb565_to_rgb8(0), (0, 0, 0));
        assert_eq!(rgb565_to_rgb8(0xffff), (255, 255, 255));
        assert_eq!(rgb565_to_rgb8(0x07e0), (0, 255, 0));
    }
}
