//! Interface fonts: `SBFONT` bitmap fonts (`.bfn`, `dialog.fnt`), `SBTTFT` TrueType descriptors (`.tfn`)
//! and the role table `Interface/Fonts/manager.cfg`. Spec: `docs/formats/fonts.md`.
//!
//! A bitmap font is a glyph table plus two RGB565 strips of equal size: a colour (texture) layer and a
//! greyscale coverage mask. Glyph `i` occupies columns `x .. x + width` of both strips over the full strip
//! height. The mask is the alpha used to blit the colour layer.

use crate::image_blob::{self, Image16, rgb565_to_rgb8};
use crate::reader::{FormatError, Reader};

/// Magic of a bitmap font.
pub const BITMAP_MAGIC: &[u8; 6] = b"SBFONT";
/// Magic of a TrueType descriptor.
pub const TRUETYPE_MAGIC: &[u8; 6] = b"SBTTFT";
/// Size of the fixed name field in both headers.
pub const NAME_SIZE: usize = 36;
/// Size of one glyph record.
pub const GLYPH_RECORD_SIZE: usize = 18;
/// Offset of the first glyph record.
pub const GLYPH_TABLE_OFFSET: usize = 0x46;
/// Size of a `.tfn` file.
pub const TRUETYPE_SIZE: usize = 90;
/// Largest glyph table accepted (codes are `u16`).
pub const MAX_GLYPHS: usize = 65536;

/// One glyph record (18 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    /// Character code: Unicode scalar value (Latin-1 plus U+0152/U+0153/U+2026/U+20AC in retail fonts).
    pub code: u16,
    /// First column of the glyph in both strips.
    pub x: u32,
    /// Width in pixels (0 for a blank glyph such as the Title space).
    pub width: u32,
    /// Small signed horizontal adjustment applied before the glyph (left-side bearing; inferred).
    pub x_adjust: i32,
    /// Small signed adjustment of the advance after the glyph (inferred; `Q` in the Scroll face is -8).
    pub advance_adjust: i32,
}

/// A parsed bitmap font.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitmapFont {
    /// Version word at offset 6 (`0x200` in every retail file).
    pub version: u32,
    /// Face / project name from the fixed 36-byte field ("Lydian", "Scroll", "Title", "ShortBriefing").
    pub name: String,
    /// `0` or `2` in retail files (2 for the "Lydian" fonts); meaning not established.
    pub unknown_2e: u32,
    /// Height of a glyph cell; equals the strip height in every retail file.
    pub cell_height: u32,
    /// `0`, `15` or `25` in retail files; meaning not established.
    pub unknown_36: u32,
    /// `11 .. 25` in retail files; not the baseline row; meaning not established.
    pub unknown_3a: u32,
    /// Global signed spacing added to every advance (`-4 .. 1` in retail files; inferred).
    pub spacing: i32,
    /// Glyph table in file order (ascending code in every retail file).
    pub glyphs: Vec<Glyph>,
    /// Colour layer: RGB565 strip, black where the glyph has no colour.
    pub colour: Image16,
    /// Coverage mask: greyscale RGB565 strip of the same size (`0x0000` = transparent, `0xFFDF` = opaque).
    pub mask: Image16,
}

/// A decoded RGBA8 picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes, row-major RGBA.
    pub pixels: Vec<u8>,
}

impl RgbaImage {
    /// A fully transparent image.
    #[must_use]
    pub fn transparent(width: u32, height: u32) -> Self {
        RgbaImage {
            width,
            height,
            pixels: vec![0; (width as usize) * (height as usize) * 4],
        }
    }
}

/// Alpha of one mask word: the 5-bit red channel expanded to 8 bits (`0xFFDF` -> 255, `0` -> 0).
#[must_use]
pub fn mask_alpha(word: u16) -> u8 {
    rgb565_to_rgb8(word).0
}

impl BitmapFont {
    /// Height of every glyph in pixels.
    #[must_use]
    pub fn height(&self) -> u32 {
        u32::from(self.colour.height)
    }

    /// Look up a glyph by character.
    #[must_use]
    pub fn glyph(&self, ch: char) -> Option<&Glyph> {
        let code = u16::try_from(u32::from(ch)).ok()?;
        self.glyphs.iter().find(|g| g.code == code)
    }

    /// Pen advance of a glyph: `x_adjust + width + advance_adjust + spacing` (inferred layout rule).
    #[must_use]
    pub fn advance(&self, g: &Glyph) -> i32 {
        g.x_adjust
            .saturating_add(i32::try_from(g.width).unwrap_or(i32::MAX))
            .saturating_add(g.advance_adjust)
            .saturating_add(self.spacing)
    }

    /// Decode one glyph as RGBA8 (`width x height`), colour from the colour layer and alpha from the mask.
    #[must_use]
    pub fn glyph_rgba(&self, g: &Glyph) -> RgbaImage {
        let stride = usize::from(self.colour.width);
        let height = usize::from(self.colour.height);
        let width = usize::try_from(g.width).unwrap_or(0);
        let mut out = RgbaImage::transparent(g.width, self.height());
        for y in 0..height {
            for x in 0..width {
                let col = usize::try_from(g.x).unwrap_or(usize::MAX).saturating_add(x);
                if col >= stride {
                    continue;
                }
                let i = y * stride + col;
                let (r, gr, b) = rgb565_to_rgb8(self.colour.pixels[i]);
                let a = mask_alpha(self.mask.pixels[i]);
                let o = (y * width + x) * 4;
                out.pixels[o..o + 4].copy_from_slice(&[r, gr, b, a]);
            }
        }
        out
    }
}

/// Parse a whole `SBFONT` file.
pub fn parse_bitmap(data: &[u8]) -> Result<BitmapFont, FormatError> {
    let mut r = Reader::new(data);
    r.expect(BITMAP_MAGIC, "SBFONT magic")?;
    let version = r.u32("font version")?;
    let name = r.fixed_string(NAME_SIZE, "font name")?;
    let unknown_2e = r.u32("font unknown_2e")?;
    let cell_height = r.u32("font cell height")?;
    let unknown_36 = r.u32("font unknown_36")?;
    let unknown_3a = r.u32("font unknown_3a")?;
    let glyph_count = r.u32("font glyph count")?;
    let spacing = r.i32("font spacing")?;
    debug_assert_eq!(r.pos(), GLYPH_TABLE_OFFSET);
    let count = glyph_count as usize;
    if count > MAX_GLYPHS || count.saturating_mul(GLYPH_RECORD_SIZE) > r.remaining() {
        return Err(FormatError::Invalid {
            offset: 0x3e,
            what: "font glyph count",
            value: glyph_count.to_string(),
        });
    }
    let mut glyphs = Vec::with_capacity(count);
    for _ in 0..count {
        glyphs.push(Glyph {
            code: r.u16("glyph code")?,
            x: r.u32("glyph x")?,
            width: r.u32("glyph width")?,
            x_adjust: r.i32("glyph x adjust")?,
            advance_adjust: r.i32("glyph advance adjust")?,
        });
    }
    let colour = image_blob::parse(&mut r)?;
    let mask_offset = r.pos();
    let mask = image_blob::parse(&mut r)?;
    if mask.width != colour.width || mask.height != colour.height {
        return Err(FormatError::Invalid {
            offset: mask_offset,
            what: "font mask dimensions",
            value: format!(
                "{}x{} (colour layer is {}x{})",
                mask.width, mask.height, colour.width, colour.height
            ),
        });
    }
    r.expect_end("font mask layer")?;
    for (i, g) in glyphs.iter().enumerate() {
        let end = u64::from(g.x) + u64::from(g.width);
        if end > u64::from(colour.width) {
            return Err(FormatError::Invalid {
                offset: GLYPH_TABLE_OFFSET + i * GLYPH_RECORD_SIZE + 2,
                what: "glyph extent",
                value: format!("{end} > strip width {}", colour.width),
            });
        }
    }
    Ok(BitmapFont {
        version,
        name,
        unknown_2e,
        cell_height,
        unknown_36,
        unknown_3a,
        spacing,
        glyphs,
        colour,
        mask,
    })
}

/// A parsed `SBTTFT` descriptor (90 bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrueTypeFont {
    /// Version word at offset 6 (`0x100` in every retail file).
    pub version: u32,
    /// Descriptor name ("New font", "List Default").
    pub name: String,
    /// `0` or `1` in retail files; `1` on the heavier roles (Title, Dialog, buttons); meaning not established.
    pub unknown_2e: u32,
    /// Point size (`11 .. 34` in retail files).
    pub size: u32,
    /// Windows face name ("SimSun", "Arial"); the 32-byte field may hold junk after the NUL.
    pub face: String,
    /// Text colour as R, G, B (stored as a Windows `COLORREF`, `00BBGGRR`).
    pub colour: [u8; 3],
    /// High byte of the colour word (`0` in every retail file).
    pub unknown_59: u8,
}

/// Parse a whole `SBTTFT` file.
pub fn parse_truetype(data: &[u8]) -> Result<TrueTypeFont, FormatError> {
    let mut r = Reader::new(data);
    r.expect(TRUETYPE_MAGIC, "SBTTFT magic")?;
    let version = r.u32("truetype version")?;
    let name = r.fixed_string(NAME_SIZE, "truetype name")?;
    let unknown_2e = r.u32("truetype unknown_2e")?;
    let size = r.u32("truetype size")?;
    let face = r.fixed_string(32, "truetype face")?;
    let c: [u8; 4] = r.array("truetype colour")?;
    r.expect_end("truetype descriptor")?;
    Ok(TrueTypeFont {
        version,
        name,
        unknown_2e,
        size,
        face,
        colour: [c[0], c[1], c[2]],
        unknown_59: c[3],
    })
}

/// One line of `manager.cfg`: a UI role and the fonts that render it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontRole {
    /// Role name (e.g. `Tooltips`, `MissionTitle`).
    pub role: String,
    /// Bitmap font file name (first column), if any. Case may differ from the file on disk.
    pub bitmap: Option<String>,
    /// TrueType descriptor file name (second column), if any.
    pub truetype: Option<String>,
}

/// Parse `manager.cfg`: `Role : bitmap.bfn, descriptor.tfn` per line, either column may be empty.
///
/// Lines without a colon are ignored. Never fails; a malformed file yields fewer roles.
#[must_use]
pub fn parse_manager_cfg(text: &str) -> Vec<FontRole> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some((role, rest)) = line.split_once(':') else {
            continue;
        };
        let role = role.trim();
        if role.is_empty() {
            continue;
        }
        let mut cols = rest.split(',').map(str::trim);
        let bitmap = cols.next().filter(|s| !s.is_empty()).map(String::from);
        let truetype = cols.next().filter(|s| !s.is_empty()).map(String::from);
        out.push(FontRole {
            role: role.to_string(),
            bitmap,
            truetype,
        });
    }
    out
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

    fn blob(w: u16, h: u16, pixels: &[u16]) -> Vec<u8> {
        let raw: Vec<u8> = pixels.iter().flat_map(|p| p.to_le_bytes()).collect();
        let payload = zlib(&raw);
        let mut out = Vec::new();
        out.extend_from_slice(&w.to_le_bytes());
        out.extend_from_slice(&h.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&payload);
        out
    }

    /// A 2-glyph font: 'A' (2 wide, red) and 'B' (1 wide, blue), strip 4x2.
    fn sample_font() -> Vec<u8> {
        let mut f = Vec::new();
        f.extend_from_slice(BITMAP_MAGIC);
        f.extend_from_slice(&0x200u32.to_le_bytes());
        let mut name = [0u8; NAME_SIZE];
        name[..4].copy_from_slice(b"Test");
        f.extend_from_slice(&name);
        for v in [0u32, 2, 0, 1, 2] {
            f.extend_from_slice(&v.to_le_bytes());
        }
        f.extend_from_slice(&(-1i32).to_le_bytes());
        assert_eq!(f.len(), GLYPH_TABLE_OFFSET);
        for (code, x, w, xa, aa) in [(b'A', 0u32, 2u32, 0i32, 1i32), (b'B', 3, 1, 1, 0)] {
            f.extend_from_slice(&u16::from(code).to_le_bytes());
            f.extend_from_slice(&x.to_le_bytes());
            f.extend_from_slice(&w.to_le_bytes());
            f.extend_from_slice(&xa.to_le_bytes());
            f.extend_from_slice(&aa.to_le_bytes());
        }
        f.extend_from_slice(&blob(
            4,
            2,
            &[0xF800, 0xF800, 0, 0x001F, 0xF800, 0xF800, 0, 0x001F],
        ));
        f.extend_from_slice(&blob(4, 2, &[0xFFDF, 0, 0, 0xFFDF, 0, 0xFFDF, 0, 0x7BEF]));
        f
    }

    #[test]
    fn parses_sample_font() {
        let font = parse_bitmap(&sample_font()).unwrap();
        assert_eq!(font.name, "Test");
        assert_eq!(font.cell_height, 2);
        assert_eq!(font.spacing, -1);
        assert_eq!(font.glyphs.len(), 2);
        let a = font.glyph('A').unwrap();
        assert_eq!(font.advance(a), 2);
        let img = font.glyph_rgba(a);
        assert_eq!((img.width, img.height), (2, 2));
        assert_eq!(&img.pixels[..8], &[255, 0, 0, 255, 255, 0, 0, 0]);
        let b = font.glyph('B').unwrap();
        assert_eq!(font.advance(b), 1);
        let img = font.glyph_rgba(b);
        assert_eq!(img.pixels[3], 255);
        assert_eq!(img.pixels[7], mask_alpha(0x7BEF));
        assert!(font.glyph('C').is_none());
    }

    #[test]
    fn rejects_bad_glyph_count_and_truncation() {
        let f = sample_font();
        let mut bad = f.clone();
        bad[0x3e..0x42].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        assert!(parse_bitmap(&bad).is_err());
        for n in 0..f.len() {
            assert!(parse_bitmap(&f[..n]).is_err(), "prefix {n} accepted");
        }
        let mut extra = f;
        extra.push(0);
        assert!(parse_bitmap(&extra).is_err());
    }

    #[test]
    fn garbage_does_not_panic() {
        for n in 0..128usize {
            let data: Vec<u8> = (0..n).map(|i| (i * 37 % 251) as u8).collect();
            let _ = parse_bitmap(&data);
            let _ = parse_truetype(&data);
        }
    }

    #[test]
    fn parses_truetype_descriptor() {
        let mut f = Vec::new();
        f.extend_from_slice(TRUETYPE_MAGIC);
        f.extend_from_slice(&0x100u32.to_le_bytes());
        let mut name = [0u8; NAME_SIZE];
        name[..5].copy_from_slice(b"Hello");
        f.extend_from_slice(&name);
        f.extend_from_slice(&1u32.to_le_bytes());
        f.extend_from_slice(&14u32.to_le_bytes());
        let mut face = [0u8; 32];
        face[..5].copy_from_slice(b"Arial");
        face[7] = b'U';
        f.extend_from_slice(&face);
        f.extend_from_slice(&[0x10, 0x20, 0x30, 0]);
        assert_eq!(f.len(), TRUETYPE_SIZE);
        let t = parse_truetype(&f).unwrap();
        assert_eq!(t.name, "Hello");
        assert_eq!(t.face, "Arial");
        assert_eq!(t.size, 14);
        assert_eq!(t.unknown_2e, 1);
        assert_eq!(t.colour, [0x10, 0x20, 0x30]);
        f.push(0);
        assert!(parse_truetype(&f).is_err());
    }

    #[test]
    fn parses_manager_cfg() {
        let text =
            "Tooltips\t\t:\ttooltips.bfn,\nListDefault\t:\t,\t\tListDefault.tfn\n\nnocolon\n";
        let roles = parse_manager_cfg(text);
        assert_eq!(roles.len(), 2);
        assert_eq!(roles[0].role, "Tooltips");
        assert_eq!(roles[0].bitmap.as_deref(), Some("tooltips.bfn"));
        assert_eq!(roles[0].truetype, None);
        assert_eq!(roles[1].bitmap, None);
        assert_eq!(roles[1].truetype.as_deref(), Some("ListDefault.tfn"));
    }
}
