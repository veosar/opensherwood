//! Compiled mission scripts (`.scb`). Spec: `docs/formats/scb.md` (partial: header only).

use crate::reader::{FormatError, Reader};

/// Header of a compiled script.
#[derive(Debug, Clone, PartialEq)]
pub struct ScbHeader {
    /// Format version as a float (1.5 in retail data).
    pub version: f32,
    /// Count at offset 12; meaning unknown (23 in the tutorial).
    pub unknown_0x0c: u32,
    /// Path of the source file on the developer's machine.
    pub source_path: String,
    /// Offset of the first byte after the header.
    pub body_offset: usize,
}

/// Parse the header of a `.scb` file.
pub fn parse_header(data: &[u8]) -> Result<ScbHeader, FormatError> {
    let mut r = Reader::new(data);
    r.expect(b"SBSCRIPT", "SBSCRIPT magic")?;
    let version = r.f32("scb version")?;
    let unknown_0x0c = r.u32("scb unknown_0x0c")?;
    let path_len = r.u32("scb source path length")? as usize;
    if path_len > 1024 {
        return Err(FormatError::Invalid {
            offset: 16,
            what: "scb source path length",
            value: path_len.to_string(),
        });
    }
    let source_path = crate::reader::latin1(r.bytes(path_len, "scb source path")?);
    Ok(ScbHeader {
        version,
        unknown_0x0c,
        source_path,
        body_offset: r.pos(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_header() {
        let mut f = b"SBSCRIPT".to_vec();
        f.extend_from_slice(&1.5f32.to_le_bytes());
        f.extend_from_slice(&23u32.to_le_bytes());
        f.extend_from_slice(&3u32.to_le_bytes());
        f.extend_from_slice(b"a.b");
        let h = parse_header(&f).unwrap();
        assert!((h.version - 1.5).abs() < f32::EPSILON);
        assert_eq!(h.source_path, "a.b");
        assert_eq!(h.body_offset, f.len());
    }
}
