//! Sprite bank dictionary (`robinhood.dic`): dictionary pages plus the global frame table.
//! Spec: `docs/formats/sprites.md`. Pixel decoding of `.bks` symbol streams is still unknown.

use crate::reader::{FormatError, Reader};

/// Shared first word of `.rhs` and `.dic` files in the retail data set: a bank generation id.
pub const BANK_GENERATION_ID: u32 = 0x0003_EBC9;

/// Size of one frame record in bytes.
pub const FRAME_RECORD_SIZE: usize = 14;

/// Page value of frames that do not use a dictionary page (the largest frames).
pub const NO_PAGE: u16 = 0xFFFF;

/// One entry of the frame table (14 bytes): where a frame's symbol stream lives in `.bks`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRecord {
    /// Frame width in pixels.
    pub width: u16,
    /// Frame height in pixels.
    pub height: u16,
    /// Byte offset of the symbol stream in `robinhood.bks`.
    pub offset: u32,
    /// Byte length of the symbol stream (`u16` symbols, values `0..4096`).
    pub length: u32,
    /// Dictionary page used to decode this frame (`0..page_count`), or [`NO_PAGE`].
    pub page: u16,
}

/// Header and tables of the dictionary file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dictionary<'a> {
    /// Bank generation id (matches every `.rhs`).
    pub bank_generation: u32,
    /// Number of dictionary pages (134 in retail data).
    pub page_count: u16,
    /// Symbols per page (4096 in retail data).
    pub symbols_per_page: u16,
    /// Undecoded dictionary region between the header and the frame table.
    pub dictionary_region: &'a [u8],
    /// Frame records in index order.
    pub frames: Vec<FrameRecord>,
}

impl Dictionary<'_> {
    /// Frame record by index.
    #[must_use]
    pub fn frame(&self, index: u32) -> Option<&FrameRecord> {
        self.frames.get(index as usize)
    }
}

/// Parse `robinhood.dic`.
///
/// The frame table is located from the end of the file: it consists of `n` records of 14 bytes
/// whose `offset`/`length` chain contiguously through `.bks`. We find the table start by walking
/// backwards from the end while the chain property holds.
pub fn parse(data: &[u8]) -> Result<Dictionary<'_>, FormatError> {
    let mut r = Reader::new(data);
    let bank_generation = r.u32("dic bank generation id")?;
    if bank_generation != BANK_GENERATION_ID {
        return Err(FormatError::BadMagic {
            offset: 0,
            expected: format!("{BANK_GENERATION_ID:#x}"),
            found: format!("{bank_generation:#x}"),
        });
    }
    let page_count = r.u16("dic page count")?;
    let symbols_per_page = r.u16("dic symbols per page")?;
    let header_end = r.pos();

    let table_start = find_table_start(data, header_end)?;
    let mut frames = Vec::with_capacity((data.len() - table_start) / FRAME_RECORD_SIZE);
    let mut t = Reader::at(data, table_start)?;
    while t.remaining() >= FRAME_RECORD_SIZE {
        frames.push(read_record(&mut t)?);
    }
    t.expect_end("dic frame table")?;
    Ok(Dictionary {
        bank_generation,
        page_count,
        symbols_per_page,
        dictionary_region: &data[header_end..table_start],
        frames,
    })
}

fn read_record(r: &mut Reader<'_>) -> Result<FrameRecord, FormatError> {
    Ok(FrameRecord {
        width: r.u16("dic frame width")?,
        height: r.u16("dic frame height")?,
        offset: r.u32("dic frame offset")?,
        length: r.u32("dic frame length")?,
        page: r.u16("dic frame page")?,
    })
}

/// Walk backwards from the end of the file while consecutive records chain (`offset + length ==
/// next.offset`). The first record of the table is the first one whose predecessor breaks the chain.
fn find_table_start(data: &[u8], header_end: usize) -> Result<usize, FormatError> {
    let total = data.len();
    if total < header_end + FRAME_RECORD_SIZE {
        return Err(FormatError::Eof {
            offset: header_end,
            needed: FRAME_RECORD_SIZE,
            what: "dic frame table",
        });
    }
    // The table ends exactly at EOF; find the phase from the last record.
    let mut start = total - FRAME_RECORD_SIZE;
    loop {
        if start < header_end + FRAME_RECORD_SIZE {
            break;
        }
        let prev_start = start - FRAME_RECORD_SIZE;
        let mut a = Reader::at(data, prev_start)?;
        let prev = read_record(&mut a)?;
        let mut b = Reader::at(data, start)?;
        let cur = read_record(&mut b)?;
        let chains = u64::from(prev.offset) + u64::from(prev.length) == u64::from(cur.offset);
        if !chains {
            break;
        }
        start = prev_start;
    }
    if total - start < FRAME_RECORD_SIZE * 2 {
        return Err(FormatError::Invalid {
            offset: start,
            what: "dic frame table",
            value: "no chained frame records found".into(),
        });
    }
    Ok(start)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(w: u16, h: u16, off: u32, len: u32, page: u16) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.extend_from_slice(&off.to_le_bytes());
        v.extend_from_slice(&len.to_le_bytes());
        v.extend_from_slice(&page.to_le_bytes());
        v
    }

    #[test]
    fn finds_table_at_end() {
        let mut f = Vec::new();
        f.extend_from_slice(&BANK_GENERATION_ID.to_le_bytes());
        f.extend_from_slice(&2u16.to_le_bytes());
        f.extend_from_slice(&4096u16.to_le_bytes());
        f.extend_from_slice(&[0xAA; 37]); // dictionary region of odd length
        f.extend(rec(4, 4, 0, 10, 0));
        f.extend(rec(5, 5, 10, 20, 0));
        f.extend(rec(6, 6, 30, 5, 1));
        let d = parse(&f).unwrap();
        assert_eq!(d.page_count, 2);
        assert_eq!(d.dictionary_region.len(), 37);
        assert_eq!(d.frames.len(), 3);
        assert_eq!(d.frame(2).unwrap().page, 1);
    }

    #[test]
    fn garbage_does_not_panic() {
        for n in 0..80usize {
            let mut data = BANK_GENERATION_ID.to_le_bytes().to_vec();
            data.extend((0..n).map(|i| (i * 17 % 251) as u8));
            let _ = parse(&data);
        }
    }
}
