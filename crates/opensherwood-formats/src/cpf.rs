//! Character / level profile table (`Configuration/profile.cpf`), see `docs/formats/profile.md`.
//!
//! The file has no magic. It is a sequence of six counted tables: two numeric tables of unknown
//! meaning (kept raw), the player characters (PC), the armed non-player humans (SD), the campaign
//! levels and the civilians (CV). Mission files index the three actor tables by 0-based position
//! (`BORG.profile` -> SD, `OILE.profile` -> CV, `TOTO.profile` -> PC; `docs/formats/rhm.md`,
//! "Actor profile mapping"). Every actor record names the sprite profile `Characters/<sprite>.rhs`,
//! the sequence inside it, a designer label and a voice-set code; the numeric fields are kept as
//! `unknown_*` bytes exactly as the spec lists them.

use crate::reader::{FormatError, Reader};

/// Upper bound on the entries of any table (retail: 27, 4, 10, 68, 63, 24).
pub const MAX_ENTRIES: usize = 4096;
/// Upper bound on the length of any string (retail maximum: 26).
pub const MAX_STRING: usize = 256;
/// Upper bound on the level prerequisite lists (retail: 0..=2).
pub const MAX_CODES: usize = 64;
/// Upper bound on the per-level `k x u32` list (retail: 0 or 1).
pub const MAX_LEVEL_WORDS: usize = 64;

/// Size of a table A block head.
pub const TABLE_A_HEAD: usize = 28;
/// Records per table A block.
pub const TABLE_A_RECORDS: usize = 10;
/// Size of a table A record.
pub const TABLE_A_RECORD: usize = 32;
/// Size of a table B record.
pub const TABLE_B_RECORD: usize = 81;

/// One block of table A: a 28-byte head and ten 32-byte records (meaning unknown; one parameter
/// set per player character for 27 actions / skills is the hypothesis).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableABlock {
    /// `u16[14]`, raw.
    pub unknown_head: [u8; TABLE_A_HEAD],
    /// Ten `u16[16]` records, raw.
    pub unknown_records: Vec<[u8; TABLE_A_RECORD]>,
}

/// A player character (PC table, indexed by `TOTO.profile`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerProfile {
    /// Base name of the sprite profile: `Characters/<sprite>.rhs`.
    pub sprite: String,
    /// Sequence name inside that `.rhs` (French).
    pub sequence: String,
    /// Designer label (empty for player characters).
    pub label: String,
    /// `u16[4]` ability values; which is which is unknown.
    pub unknown_pre: [u8; 8],
    /// Voice-set code (`PCRH`, ...): `Sounds/Exclamations/actor<code>.dat`.
    pub voice: String,
    /// Id, small words, six `f32` and two trailing `u16` (see the spec).
    pub unknown_post: [u8; 82],
}

/// An armed non-player human (SD table, indexed by `BORG.profile`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoldierProfile {
    /// Base name of the sprite profile: `Characters/<sprite>.rhs`.
    pub sprite: String,
    /// Sequence name inside that `.rhs` (French).
    pub sequence: String,
    /// Designer label, French: unit type + colour (`Lancier Bleu`).
    pub label: String,
    /// `u16[5], u8, u16[5]`: hit points and combat skills is the reading; unverified.
    pub unknown_pre: [u8; 21],
    /// Voice-set code (`SDHL`, `VPGG`, ...).
    pub voice: String,
    /// Tier word, flag byte, ranges, class id, sprite canvas centre ... (see the spec).
    pub unknown_post: [u8; 55],
}

/// A civilian (CV table, indexed by `OILE.profile`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CivilianProfile {
    /// Base name of the sprite profile: `Characters/<sprite>.rhs`.
    pub sprite: String,
    /// Sequence name inside that `.rhs` (French).
    pub sequence: String,
    /// Designer label.
    pub label: String,
    /// `u32, u32`: civilian kind (0 man, 1 woman, 3 child, 4 beggar, 5 notable) and a 0/1 word.
    pub unknown_pre: [u8; 8],
    /// Voice-set code, empty for seven entries.
    pub voice: String,
}

/// A campaign level (LEVEL table).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelRecord {
    /// Two-letter level code (`Text/RHLevel<code>.red`).
    pub code: String,
    /// Proto-level (map) name, as in the `.rhp` and the mission `FOOT`.
    pub map: String,
    /// `.rhm` base name, or `Impossible_mission` for placeholders.
    pub mission_file: String,
    /// Internal working title.
    pub title: String,
    /// 0 story, 1 assault, 3 ambush, 4 Sherwood, 5 defend, 6 tactical (observed).
    pub unknown_a: u32,
    /// 1 Croisement01 .. 9 York (observed to match `map`).
    pub location: u32,
    /// 0 or 1.
    pub unknown_c: u8,
    /// 1..=6.
    pub unknown_d: u8,
    /// 0 or 256.
    pub unknown_e: u16,
    /// 0, 200, 300, 600..800.
    pub unknown_f: u16,
    /// 0, 1, 5000..100000.
    pub unknown_g: u32,
    /// 0, 20000..200000.
    pub unknown_h: u32,
    /// Level codes this level is available after (campaign graph reading).
    pub after: Vec<String>,
    /// Level codes after which this level is removed.
    pub until: Vec<String>,
    /// `u16 0, u16 10000` in every retail record.
    pub unknown_fixed: [u16; 2],
    /// 0, 40, 50, 60, 100.
    pub unknown_i: u16,
    /// A per-kind serial.
    pub unknown_j: u16,
    /// `k x u32` (0 or 1 entries).
    pub unknown_k: Vec<u32>,
    /// Twelve bytes, zero apart from a few `01`.
    pub unknown_l: [u8; 12],
    /// Three `i8` slots (`-1 -1 -1` mostly).
    pub unknown_slots: [i8; 3],
    /// Seven `u16` values.
    pub unknown_m: [u16; 7],
    /// Ambient track base name in `Musics/`.
    pub music_ambient: String,
    /// Alarm track.
    pub music_alarm: String,
    /// Fight track.
    pub music_fight: String,
}

/// The decoded profile table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProfileTable {
    /// Table A (unknown), raw.
    pub table_a: Vec<TableABlock>,
    /// Table B (unknown; four difficulty presets is the hypothesis), raw 81-byte records.
    pub table_b: Vec<[u8; TABLE_B_RECORD]>,
    /// PC table.
    pub player_characters: Vec<PlayerProfile>,
    /// SD table.
    pub soldiers: Vec<SoldierProfile>,
    /// LEVEL table.
    pub levels: Vec<LevelRecord>,
    /// CV table.
    pub civilians: Vec<CivilianProfile>,
}

impl ProfileTable {
    /// Level record by its two-letter code.
    #[must_use]
    pub fn level(&self, code: &str) -> Option<&LevelRecord> {
        self.levels.iter().find(|l| l.code == code)
    }
}

fn count(r: &mut Reader<'_>, what: &'static str) -> Result<usize, FormatError> {
    let offset = r.pos();
    let n = r.u32(what)?;
    let n = usize::try_from(n).unwrap_or(usize::MAX);
    if n > MAX_ENTRIES {
        return Err(FormatError::Invalid {
            offset,
            what,
            value: n.to_string(),
        });
    }
    Ok(n)
}

fn string(r: &mut Reader<'_>, what: &'static str) -> Result<String, FormatError> {
    let offset = r.pos();
    let len = usize::from(r.u16(what)?);
    if len > MAX_STRING {
        return Err(FormatError::Invalid {
            offset,
            what,
            value: format!("string length {len}"),
        });
    }
    let bytes = r.bytes(len, what)?;
    Ok(crate::reader::latin1(bytes))
}

fn code(r: &mut Reader<'_>, what: &'static str) -> Result<String, FormatError> {
    r.fixed_string(4, what)
}

fn code_list(r: &mut Reader<'_>, what: &'static str) -> Result<Vec<String>, FormatError> {
    let offset = r.pos();
    let n = r.u32(what)?;
    let n = usize::try_from(n).unwrap_or(usize::MAX);
    if n > MAX_CODES {
        return Err(FormatError::Invalid {
            offset,
            what,
            value: n.to_string(),
        });
    }
    (0..n).map(|_| code(r, what)).collect()
}

fn player(r: &mut Reader<'_>) -> Result<PlayerProfile, FormatError> {
    Ok(PlayerProfile {
        sprite: string(r, "PC sprite")?,
        sequence: string(r, "PC sequence")?,
        label: string(r, "PC label")?,
        unknown_pre: r.array("PC unknown_pre")?,
        voice: code(r, "PC voice")?,
        unknown_post: r.array("PC unknown_post")?,
    })
}

fn soldier(r: &mut Reader<'_>) -> Result<SoldierProfile, FormatError> {
    Ok(SoldierProfile {
        sprite: string(r, "SD sprite")?,
        sequence: string(r, "SD sequence")?,
        label: string(r, "SD label")?,
        unknown_pre: r.array("SD unknown_pre")?,
        voice: code(r, "SD voice")?,
        unknown_post: r.array("SD unknown_post")?,
    })
}

fn civilian(r: &mut Reader<'_>) -> Result<CivilianProfile, FormatError> {
    Ok(CivilianProfile {
        sprite: string(r, "CV sprite")?,
        sequence: string(r, "CV sequence")?,
        label: string(r, "CV label")?,
        unknown_pre: r.array("CV unknown_pre")?,
        voice: code(r, "CV voice")?,
    })
}

fn level(r: &mut Reader<'_>) -> Result<LevelRecord, FormatError> {
    let code_ = code(r, "LEVEL code")?;
    let map = string(r, "LEVEL map")?;
    let mission_file = string(r, "LEVEL mission_file")?;
    let title = string(r, "LEVEL title")?;
    let unknown_a = r.u32("LEVEL unknown_a")?;
    let location = r.u32("LEVEL location")?;
    let unknown_c = r.u8("LEVEL unknown_c")?;
    let unknown_d = r.u8("LEVEL unknown_d")?;
    let unknown_e = r.u16("LEVEL unknown_e")?;
    let unknown_f = r.u16("LEVEL unknown_f")?;
    let unknown_g = r.u32("LEVEL unknown_g")?;
    let unknown_h = r.u32("LEVEL unknown_h")?;
    let after = code_list(r, "LEVEL after")?;
    let until = code_list(r, "LEVEL until")?;
    let unknown_fixed = [r.u16("LEVEL fixed")?, r.u16("LEVEL fixed")?];
    let unknown_i = r.u16("LEVEL unknown_i")?;
    let unknown_j = r.u16("LEVEL unknown_j")?;
    let k_offset = r.pos();
    let k = usize::from(r.u16("LEVEL k")?);
    if k > MAX_LEVEL_WORDS {
        return Err(FormatError::Invalid {
            offset: k_offset,
            what: "LEVEL k",
            value: k.to_string(),
        });
    }
    let unknown_k = (0..k)
        .map(|_| r.u32("LEVEL unknown_k"))
        .collect::<Result<Vec<_>, _>>()?;
    let unknown_l = r.array("LEVEL unknown_l")?;
    let slots = r.array::<3>("LEVEL unknown_slots")?;
    let unknown_slots = [slots[0] as i8, slots[1] as i8, slots[2] as i8];
    let mut unknown_m = [0u16; 7];
    for m in &mut unknown_m {
        *m = r.u16("LEVEL unknown_m")?;
    }
    Ok(LevelRecord {
        code: code_,
        map,
        mission_file,
        title,
        unknown_a,
        location,
        unknown_c,
        unknown_d,
        unknown_e,
        unknown_f,
        unknown_g,
        unknown_h,
        after,
        until,
        unknown_fixed,
        unknown_i,
        unknown_j,
        unknown_k,
        unknown_l,
        unknown_slots,
        unknown_m,
        music_ambient: string(r, "LEVEL music_ambient")?,
        music_alarm: string(r, "LEVEL music_alarm")?,
        music_fight: string(r, "LEVEL music_fight")?,
    })
}

/// Parse a whole `profile.cpf`. The file must be consumed exactly.
pub fn parse(data: &[u8]) -> Result<ProfileTable, FormatError> {
    let mut r = Reader::new(data);
    let mut t = ProfileTable::default();
    let n_a = count(&mut r, "table A count")?;
    for _ in 0..n_a {
        let unknown_head = r.array("table A head")?;
        let mut unknown_records = Vec::with_capacity(TABLE_A_RECORDS);
        for _ in 0..TABLE_A_RECORDS {
            unknown_records.push(r.array("table A record")?);
        }
        t.table_a.push(TableABlock {
            unknown_head,
            unknown_records,
        });
    }
    let n_b = count(&mut r, "table B count")?;
    for _ in 0..n_b {
        t.table_b.push(r.array("table B record")?);
    }
    let n_pc = count(&mut r, "PC count")?;
    for _ in 0..n_pc {
        t.player_characters.push(player(&mut r)?);
    }
    let n_sd = count(&mut r, "SD count")?;
    for _ in 0..n_sd {
        t.soldiers.push(soldier(&mut r)?);
    }
    let n_lv = count(&mut r, "LEVEL count")?;
    for _ in 0..n_lv {
        t.levels.push(level(&mut r)?);
    }
    let n_cv = count(&mut r, "CV count")?;
    for _ in 0..n_cv {
        t.civilians.push(civilian(&mut r)?);
    }
    r.expect_end("CV record")?;
    Ok(t)
}

/// Structural detection (the file has no magic): the first three counts chain through the fixed-size
/// tables A and B to a PC count, and the first PC record starts with a short printable string.
#[must_use]
pub fn looks_like_profile_table(data: &[u8]) -> bool {
    let mut r = Reader::new(data);
    let Ok(n_a) = r.u32("n_a") else {
        return false;
    };
    if n_a > 256 {
        return false;
    }
    let a_size = n_a as usize * (TABLE_A_HEAD + TABLE_A_RECORDS * TABLE_A_RECORD);
    if r.skip(a_size, "table A").is_err() {
        return false;
    }
    let Ok(n_b) = r.u32("n_b") else {
        return false;
    };
    if n_b > 256 || r.skip(n_b as usize * TABLE_B_RECORD, "table B").is_err() {
        return false;
    }
    let Ok(n_pc) = r.u32("n_pc") else {
        return false;
    };
    if n_pc == 0 || n_pc > 256 {
        return false;
    }
    let Ok(len) = r.u16("sprite length") else {
        return false;
    };
    let Ok(s) = r.bytes(usize::from(len), "sprite") else {
        return false;
    };
    (1..=64).contains(&s.len()) && s.iter().all(|&b| (0x20..0x7f).contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pstr(out: &mut Vec<u8>, s: &str) {
        out.extend_from_slice(&(s.len() as u16).to_le_bytes());
        out.extend_from_slice(s.as_bytes());
    }

    fn code4(out: &mut Vec<u8>, s: &str) {
        let mut c = [0u8; 4];
        c[..s.len()].copy_from_slice(s.as_bytes());
        out.extend_from_slice(&c);
    }

    fn actor(out: &mut Vec<u8>, sprite: &str, label: &str, pre: usize, voice: &str, post: usize) {
        pstr(out, sprite);
        pstr(out, "seq");
        pstr(out, label);
        out.extend(std::iter::repeat_n(0xaa, pre));
        code4(out, voice);
        out.extend(std::iter::repeat_n(0xbb, post));
    }

    fn level_bytes(out: &mut Vec<u8>, code: &str, after: &[&str], k: u16) {
        code4(out, code);
        pstr(out, "Lincoln");
        pstr(out, "H01_Lin_VL");
        pstr(out, "title");
        out.extend_from_slice(&0u32.to_le_bytes()); // unknown_a
        out.extend_from_slice(&6u32.to_le_bytes()); // location
        out.push(1); // unknown_c
        out.push(6); // unknown_d
        out.extend_from_slice(&256u16.to_le_bytes());
        out.extend_from_slice(&700u16.to_le_bytes());
        out.extend_from_slice(&5000u32.to_le_bytes());
        out.extend_from_slice(&200_000u32.to_le_bytes());
        out.extend_from_slice(&(after.len() as u32).to_le_bytes());
        for a in after {
            code4(out, a);
        }
        out.extend_from_slice(&1u32.to_le_bytes());
        code4(out, code);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&10000u16.to_le_bytes());
        out.extend_from_slice(&100u16.to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&k.to_le_bytes());
        for _ in 0..k {
            out.extend_from_slice(&1u32.to_le_bytes());
        }
        out.extend(std::iter::repeat_n(0, 12));
        out.extend_from_slice(&[0xff, 0xff, 0xff]);
        for v in [0u16, 0, 0, 0, 10, 2, 3] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        pstr(out, "Lincoln_D");
        pstr(out, "Cast_Alarm");
        pstr(out, "Cast_Fight");
    }

    fn sample() -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend(std::iter::repeat_n(0x11, TABLE_A_HEAD));
        out.extend(std::iter::repeat_n(0x22, TABLE_A_RECORDS * TABLE_A_RECORD));
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend(std::iter::repeat_n(0x33, 2 * TABLE_B_RECORD));
        out.extend_from_slice(&2u32.to_le_bytes());
        actor(&mut out, "RobinHood", "", 8, "PCRH", 82);
        actor(&mut out, "LittleJohn", "", 8, "PCLJ", 82);
        out.extend_from_slice(&1u32.to_le_bytes());
        actor(&mut out, "Guard A00", "Hallebardier Bleu", 21, "SDHL", 55);
        out.extend_from_slice(&2u32.to_le_bytes());
        level_bytes(&mut out, "HA", &[], 0);
        level_bytes(&mut out, "SA", &["HA"], 1);
        out.extend_from_slice(&2u32.to_le_bytes());
        actor(&mut out, "Mendicant", "Mendiant", 8, "CVMT", 0);
        actor(&mut out, "Priest", "Pretre", 8, "", 0);
        out
    }

    #[test]
    fn parses_synthetic_table() {
        let data = sample();
        assert!(looks_like_profile_table(&data));
        let t = parse(&data).unwrap();
        assert_eq!(t.table_a.len(), 1);
        assert_eq!(t.table_a[0].unknown_records.len(), TABLE_A_RECORDS);
        assert_eq!(t.table_b.len(), 2);
        assert_eq!(t.player_characters.len(), 2);
        assert_eq!(t.player_characters[1].sprite, "LittleJohn");
        assert_eq!(t.player_characters[1].voice, "PCLJ");
        assert_eq!(t.soldiers.len(), 1);
        assert_eq!(t.soldiers[0].sprite, "Guard A00");
        assert_eq!(t.soldiers[0].label, "Hallebardier Bleu");
        assert_eq!(t.soldiers[0].voice, "SDHL");
        assert_eq!(t.soldiers[0].unknown_pre, [0xaa; 21]);
        assert_eq!(t.soldiers[0].unknown_post, [0xbb; 55]);
        assert_eq!(t.levels.len(), 2);
        let sa = t.level("SA").unwrap();
        assert_eq!(sa.map, "Lincoln");
        assert_eq!(sa.mission_file, "H01_Lin_VL");
        assert_eq!(sa.location, 6);
        assert_eq!(sa.after, vec!["HA".to_string()]);
        assert_eq!(sa.until, vec!["SA".to_string()]);
        assert_eq!(sa.unknown_fixed, [0, 10000]);
        assert_eq!(sa.unknown_k, vec![1]);
        assert_eq!(sa.unknown_slots, [-1, -1, -1]);
        assert_eq!(sa.unknown_m, [0, 0, 0, 0, 10, 2, 3]);
        assert_eq!(sa.music_fight, "Cast_Fight");
        assert!(t.level("HA").unwrap().unknown_k.is_empty());
        assert_eq!(t.civilians.len(), 2);
        assert_eq!(t.civilians[0].sprite, "Mendicant");
        assert_eq!(t.civilians[0].voice, "CVMT");
        assert_eq!(t.civilians[1].voice, "");
        assert!(t.level("ZZ").is_none());
    }

    #[test]
    fn truncation_and_trailing_bytes_are_errors() {
        let data = sample();
        for cut in [3, 40, 400, data.len() - 1] {
            assert!(parse(&data[..cut]).is_err(), "cut at {cut}");
        }
        let mut longer = data.clone();
        longer.push(0);
        assert!(matches!(
            parse(&longer),
            Err(FormatError::Trailing { remaining: 1, .. })
        ));
    }

    #[test]
    fn oversized_counts_are_rejected_early() {
        let mut data = vec![0u8; 16];
        data[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            parse(&data),
            Err(FormatError::Invalid {
                what: "table A count",
                ..
            })
        ));
        assert!(!looks_like_profile_table(&data));
        assert!(!looks_like_profile_table(b"SRES"));
        assert!(!looks_like_profile_table(&[]));
        // A string longer than the limit.
        let mut s = sample();
        let pos = 4 + TABLE_A_HEAD + TABLE_A_RECORDS * TABLE_A_RECORD + 4 + 2 * TABLE_B_RECORD + 4;
        s[pos..pos + 2].copy_from_slice(&(MAX_STRING as u16 + 1).to_le_bytes());
        assert!(matches!(
            parse(&s),
            Err(FormatError::Invalid {
                what: "PC sprite",
                ..
            })
        ));
    }
}
