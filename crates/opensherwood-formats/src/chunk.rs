//! Tagged chunk containers used by `.rhp` (`MEUH`) and `.rhm` (`DUTY`).
//! Spec: `docs/formats/rhp.md`, `docs/formats/rhm.md`.
//!
//! Layout: `tag: [u8;4]`, `size: u32` (bytes that follow), `version: u32`, then either child chunks or
//! opaque data. The root chunk's children are themselves chunks; grandchildren are not walked
//! automatically because some chunk bodies contain tag-like data.

use crate::reader::{FormatError, Reader, tag_string};

/// A chunk with its body still undecoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawChunk<'a> {
    /// Tag.
    pub tag: [u8; 4],
    /// Offset of the tag in the file.
    pub offset: usize,
    /// Version word that starts the body.
    pub version: u32,
    /// Body after the version word.
    pub body: &'a [u8],
}

impl RawChunk<'_> {
    /// Tag as printable text.
    #[must_use]
    pub fn tag_str(&self) -> String {
        tag_string(self.tag)
    }
}

/// A root container: its tag, version and immediate children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Container<'a> {
    /// Root tag (`MEUH` or `DUTY`).
    pub tag: [u8; 4],
    /// Root version.
    pub version: u32,
    /// Children in file order.
    pub children: Vec<RawChunk<'a>>,
}

impl<'a> Container<'a> {
    /// First child with the given tag.
    #[must_use]
    pub fn child(&self, tag: &[u8; 4]) -> Option<&RawChunk<'a>> {
        self.children.iter().find(|c| &c.tag == tag)
    }
}

fn read_chunk<'a>(r: &mut Reader<'a>) -> Result<RawChunk<'a>, FormatError> {
    let offset = r.pos();
    let tag = r.tag("chunk tag")?;
    let size = r.u32("chunk size")? as usize;
    if size < 4 {
        return Err(FormatError::Invalid {
            offset: offset + 4,
            what: "chunk size",
            value: size.to_string(),
        });
    }
    let version = r.u32("chunk version")?;
    let body = r.bytes(size - 4, "chunk body")?;
    Ok(RawChunk {
        tag,
        offset,
        version,
        body,
    })
}

/// Parse a file consisting of one root chunk whose body is a sequence of child chunks.
pub fn parse_container<'a>(
    data: &'a [u8],
    expected_root: &[u8; 4],
) -> Result<Container<'a>, FormatError> {
    let mut r = Reader::new(data);
    let root = read_chunk(&mut r)?;
    if &root.tag != expected_root {
        return Err(FormatError::BadMagic {
            offset: 0,
            expected: tag_string(*expected_root),
            found: root.tag_str(),
        });
    }
    r.expect_end("root chunk")?;
    let mut body = Reader::new(root.body);
    let mut children = Vec::new();
    while body.remaining() > 0 {
        let mut child = read_chunk(&mut body)?;
        child.offset += 12; // relative to the file: root header is 12 bytes
        children.push(child);
    }
    Ok(Container {
        tag: root.tag,
        version: root.version,
        children,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(tag: [u8; 4], version: u32, body: &[u8]) -> Vec<u8> {
        let mut v = tag.to_vec();
        v.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
        v.extend_from_slice(&version.to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn walks_children() {
        let mut inner = chunk(*b"FOOT", 4, &[1, 2, 3]);
        inner.extend(chunk(*b"POUF", 3, &[]));
        let file = chunk(*b"DUTY", 2, &inner);
        let c = parse_container(&file, b"DUTY").unwrap();
        assert_eq!(c.version, 2);
        assert_eq!(c.children.len(), 2);
        assert_eq!(c.child(b"FOOT").unwrap().body, &[1, 2, 3]);
        assert_eq!(c.child(b"POUF").unwrap().version, 3);
        assert_eq!(c.children[0].offset, 12);
    }

    #[test]
    fn rejects_wrong_root_and_garbage() {
        let file = chunk(*b"MEUH", 2, &[]);
        assert!(parse_container(&file, b"DUTY").is_err());
        for n in 0..64usize {
            let data: Vec<u8> = (0..n).map(|i| (i * 91 % 251) as u8).collect();
            let _ = parse_container(&data, b"DUTY");
        }
    }
}
