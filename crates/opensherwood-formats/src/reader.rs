//! A bounded little-endian reader that reports offsets in its errors.

use thiserror::Error;

/// Error produced by every parser in this crate.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// The input ended before a field could be read.
    #[error("unexpected end of data at offset {offset:#x} while reading {what} ({needed} bytes)")]
    Eof {
        /// Offset at which the read started.
        offset: usize,
        /// Bytes required.
        needed: usize,
        /// What was being read.
        what: &'static str,
    },
    /// A magic number or tag did not match.
    #[error("bad magic at offset {offset:#x}: expected {expected}, found {found}")]
    BadMagic {
        /// Offset of the magic.
        offset: usize,
        /// What was expected.
        expected: String,
        /// What was found (escaped).
        found: String,
    },
    /// A field had a value outside the allowed range.
    #[error("invalid value for {what} at offset {offset:#x}: {value}")]
    Invalid {
        /// Offset of the field.
        offset: usize,
        /// Field name.
        what: &'static str,
        /// Human readable value.
        value: String,
    },
    /// A compressed payload could not be decoded.
    #[error("decompression failed ({codec}) at offset {offset:#x}: {message}")]
    Decompress {
        /// Codec name.
        codec: &'static str,
        /// Offset of the payload.
        offset: usize,
        /// Underlying message.
        message: String,
    },
    /// The file was parsed but bytes remained.
    #[error("{remaining} trailing bytes after the last {what} at offset {offset:#x}")]
    Trailing {
        /// Where the parser stopped.
        offset: usize,
        /// Number of remaining bytes.
        remaining: usize,
        /// What was parsed last.
        what: &'static str,
    },
}

/// Cursor over a byte slice with bounds-checked little-endian reads.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// Create a reader positioned at the start of `data`.
    #[must_use]
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Create a reader positioned at `pos`.
    pub fn at(data: &'a [u8], pos: usize) -> Result<Self, FormatError> {
        if pos > data.len() {
            return Err(FormatError::Eof {
                offset: pos,
                needed: 0,
                what: "seek",
            });
        }
        Ok(Self { data, pos })
    }

    /// Current offset.
    #[must_use]
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Bytes remaining.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    /// Whole underlying slice.
    #[must_use]
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    /// Move to an absolute offset.
    pub fn seek(&mut self, pos: usize) -> Result<(), FormatError> {
        if pos > self.data.len() {
            return Err(FormatError::Eof {
                offset: pos,
                needed: 0,
                what: "seek",
            });
        }
        self.pos = pos;
        Ok(())
    }

    /// Skip `n` bytes.
    pub fn skip(&mut self, n: usize, what: &'static str) -> Result<(), FormatError> {
        self.bytes(n, what).map(|_| ())
    }

    /// Read `n` raw bytes.
    pub fn bytes(&mut self, n: usize, what: &'static str) -> Result<&'a [u8], FormatError> {
        if self.remaining() < n {
            return Err(FormatError::Eof {
                offset: self.pos,
                needed: n,
                what,
            });
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    /// Read a fixed-size array.
    pub fn array<const N: usize>(&mut self, what: &'static str) -> Result<[u8; N], FormatError> {
        let s = self.bytes(N, what)?;
        let mut out = [0u8; N];
        out.copy_from_slice(s);
        Ok(out)
    }

    /// Read an unsigned byte.
    pub fn u8(&mut self, what: &'static str) -> Result<u8, FormatError> {
        Ok(self.array::<1>(what)?[0])
    }

    /// Read a little-endian `u16`.
    pub fn u16(&mut self, what: &'static str) -> Result<u16, FormatError> {
        Ok(u16::from_le_bytes(self.array(what)?))
    }

    /// Read a little-endian `i16`.
    pub fn i16(&mut self, what: &'static str) -> Result<i16, FormatError> {
        Ok(i16::from_le_bytes(self.array(what)?))
    }

    /// Read a little-endian `u32`.
    pub fn u32(&mut self, what: &'static str) -> Result<u32, FormatError> {
        Ok(u32::from_le_bytes(self.array(what)?))
    }

    /// Read a little-endian `i32`.
    pub fn i32(&mut self, what: &'static str) -> Result<i32, FormatError> {
        Ok(i32::from_le_bytes(self.array(what)?))
    }

    /// Read a little-endian `f32`.
    pub fn f32(&mut self, what: &'static str) -> Result<f32, FormatError> {
        Ok(f32::from_le_bytes(self.array(what)?))
    }

    /// Read a 4-byte tag as a string (Latin-1, control characters escaped).
    pub fn tag(&mut self, what: &'static str) -> Result<[u8; 4], FormatError> {
        self.array::<4>(what)
    }

    /// Expect an exact byte sequence.
    pub fn expect(&mut self, magic: &[u8], what: &'static str) -> Result<(), FormatError> {
        let offset = self.pos;
        let found = self.bytes(magic.len(), what)?;
        if found != magic {
            return Err(FormatError::BadMagic {
                offset,
                expected: escape(magic),
                found: escape(found),
            });
        }
        Ok(())
    }

    /// Read a `u16` length followed by that many Latin-1 bytes.
    pub fn pstring16(&mut self, what: &'static str) -> Result<String, FormatError> {
        let len = usize::from(self.u16(what)?);
        let bytes = self.bytes(len, what)?;
        Ok(latin1(bytes))
    }

    /// Read a `u16` length followed by that many UTF-16LE code units.
    pub fn pstring16_utf16(&mut self, what: &'static str) -> Result<String, FormatError> {
        let len = usize::from(self.u16(what)?);
        let bytes = self.bytes(len * 2, what)?;
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Ok(String::from_utf16_lossy(&units))
    }

    /// Read a fixed-size NUL-padded Latin-1 string.
    pub fn fixed_string(&mut self, n: usize, what: &'static str) -> Result<String, FormatError> {
        let bytes = self.bytes(n, what)?;
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(n);
        Ok(latin1(&bytes[..end]))
    }

    /// Fail if bytes remain.
    pub fn expect_end(&self, what: &'static str) -> Result<(), FormatError> {
        if self.remaining() != 0 {
            return Err(FormatError::Trailing {
                offset: self.pos,
                remaining: self.remaining(),
                what,
            });
        }
        Ok(())
    }
}

/// Decode Latin-1 bytes.
#[must_use]
pub fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

/// Escape bytes for error messages.
#[must_use]
pub fn escape(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    for &b in bytes {
        if (0x20..0x7f).contains(&b) {
            s.push(char::from(b));
        } else {
            let _ = write!(s, "\\x{b:02x}");
        }
    }
    s
}

/// Render a 4-byte tag for display.
#[must_use]
pub fn tag_string(tag: [u8; 4]) -> String {
    escape(&tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_reports_eof() {
        let data = [1u8, 0, 2, 0, 0, 0];
        let mut r = Reader::new(&data);
        assert_eq!(r.u16("a").unwrap(), 1);
        assert_eq!(r.u32("b").unwrap(), 2);
        let err = r.u8("c").unwrap_err();
        assert!(matches!(
            err,
            FormatError::Eof {
                offset: 6,
                needed: 1,
                ..
            }
        ));
    }

    #[test]
    fn pstrings() {
        let data = [3u8, 0, b'a', b'b', b'c', 1, 0, 0x41, 0];
        let mut r = Reader::new(&data);
        assert_eq!(r.pstring16("s").unwrap(), "abc");
        assert_eq!(r.pstring16_utf16("t").unwrap(), "A");
        r.expect_end("x").unwrap();
    }
}
