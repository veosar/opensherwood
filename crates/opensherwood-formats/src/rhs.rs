//! Sprite profiles (`.rhs`): named sequences of animations made of frame references.
//! Spec: `docs/formats/sprites.md`. The pixel data lives in the bank (`dic.rs`).

use crate::dic::BANK_GENERATION_ID;
use crate::reader::{FormatError, Reader};

/// One frame reference inside an animation (14 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRef {
    /// Index into the bank frame table.
    pub frame: u32,
    /// Display duration in ticks (values 1..15 seen).
    pub duration: u32,
    /// Hotspot / anchor x, relative to the sequence box.
    pub anchor_x: u16,
    /// Hotspot / anchor y.
    pub anchor_y: u16,
    /// Mostly 0; non-zero on some cart animations. Unknown.
    pub unknown_0x0c: u16,
}

/// One animation (a direction / action) inside a sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Animation {
    /// Observed to be `frames.len() - 1`; probably the loop / key frame index.
    pub unknown_0x02: u16,
    /// Typically 150 or 0; unknown (speed? scale?).
    pub unknown_0x04: u32,
    /// Typically 150, 136, 10; unknown.
    pub unknown_0x08: u32,
    /// Small value, often 0 or 195; unknown.
    pub unknown_0x0c: u16,
    /// Frame references in playback order.
    pub frames: Vec<FrameRef>,
}

/// A named sequence: a character, object or map animation with a bounding box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sequence {
    /// Name as authored (French, e.g. "Robin des bois").
    pub name: String,
    /// Bounding box width of all frames.
    pub width: u16,
    /// Bounding box height.
    pub height: u16,
    /// Typically 150; unknown.
    pub unknown_0x26: u32,
    /// Typically 150; unknown.
    pub unknown_0x2a: u32,
    /// Animations in file order (for characters: 2048 = actions x 8 directions x variants).
    pub animations: Vec<Animation>,
}

/// A parsed `.rhs` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// Bank generation id; must match the `.dic`.
    pub bank_generation: u32,
    /// Sequences in file order (usually 1; map animation files hold several).
    pub sequences: Vec<Sequence>,
}

impl Profile {
    /// Every frame index referenced by this profile.
    #[must_use]
    pub fn frame_indices(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self
            .sequences
            .iter()
            .flat_map(|s| s.animations.iter())
            .flat_map(|a| a.frames.iter().map(|f| f.frame))
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }
}

/// Parse a `.rhs` file.
pub fn parse(data: &[u8]) -> Result<Profile, FormatError> {
    let mut r = Reader::new(data);
    let bank_generation = r.u32("rhs bank generation id")?;
    if bank_generation != BANK_GENERATION_ID {
        return Err(FormatError::BadMagic {
            offset: 0,
            expected: format!("{BANK_GENERATION_ID:#x}"),
            found: format!("{bank_generation:#x}"),
        });
    }
    let seq_count = r.u16("rhs sequence count")?;
    let mut sequences = Vec::with_capacity(usize::from(seq_count));
    for _ in 0..seq_count {
        let name = r.fixed_string(32, "rhs sequence name")?;
        let anim_count = r.u16("rhs animation count")?;
        let width = r.u16("rhs width")?;
        let height = r.u16("rhs height")?;
        let unknown_0x26 = r.u32("rhs unknown_0x26")?;
        let unknown_0x2a = r.u32("rhs unknown_0x2a")?;
        let mut animations = Vec::with_capacity(usize::from(anim_count));
        for _ in 0..anim_count {
            let n = r.u16("rhs frame count")?;
            let unknown_0x02 = r.u16("rhs anim unknown_0x02")?;
            let unknown_0x04 = r.u32("rhs anim unknown_0x04")?;
            let unknown_0x08 = r.u32("rhs anim unknown_0x08")?;
            let unknown_0x0c = r.u16("rhs anim unknown_0x0c")?;
            let mut frames = Vec::with_capacity(usize::from(n));
            for _ in 0..n {
                frames.push(FrameRef {
                    frame: r.u32("rhs frame index")?,
                    duration: r.u32("rhs frame duration")?,
                    anchor_x: r.u16("rhs frame anchor x")?,
                    anchor_y: r.u16("rhs frame anchor y")?,
                    unknown_0x0c: r.u16("rhs frame unknown_0x0c")?,
                });
            }
            animations.push(Animation {
                unknown_0x02,
                unknown_0x04,
                unknown_0x08,
                unknown_0x0c,
                frames,
            });
        }
        sequences.push(Sequence {
            name,
            width,
            height,
            unknown_0x26,
            unknown_0x2a,
            animations,
        });
    }
    r.expect_end("rhs sequences")?;
    Ok(Profile {
        bank_generation,
        sequences,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build() -> Vec<u8> {
        let mut f = Vec::new();
        f.extend_from_slice(&BANK_GENERATION_ID.to_le_bytes());
        f.extend_from_slice(&1u16.to_le_bytes());
        let mut name = b"Coin".to_vec();
        name.resize(32, 0);
        f.extend_from_slice(&name);
        f.extend_from_slice(&1u16.to_le_bytes()); // anim count
        f.extend_from_slice(&6u16.to_le_bytes());
        f.extend_from_slice(&6u16.to_le_bytes());
        f.extend_from_slice(&150u32.to_le_bytes());
        f.extend_from_slice(&150u32.to_le_bytes());
        // animation with 2 frames
        f.extend_from_slice(&2u16.to_le_bytes());
        f.extend_from_slice(&1u16.to_le_bytes());
        f.extend_from_slice(&150u32.to_le_bytes());
        f.extend_from_slice(&150u32.to_le_bytes());
        f.extend_from_slice(&0u16.to_le_bytes());
        for (frame, dur) in [(10u32, 3u32), (11, 4)] {
            f.extend_from_slice(&frame.to_le_bytes());
            f.extend_from_slice(&dur.to_le_bytes());
            f.extend_from_slice(&5u16.to_le_bytes());
            f.extend_from_slice(&6u16.to_le_bytes());
            f.extend_from_slice(&0u16.to_le_bytes());
        }
        f
    }

    #[test]
    fn parses_profile() {
        let p = parse(&build()).unwrap();
        assert_eq!(p.sequences.len(), 1);
        let s = &p.sequences[0];
        assert_eq!(s.name, "Coin");
        assert_eq!((s.width, s.height), (6, 6));
        assert_eq!(s.animations[0].frames[1].frame, 11);
        assert_eq!(p.frame_indices(), vec![10, 11]);
    }

    #[test]
    fn truncated_does_not_panic() {
        let f = build();
        for n in 0..f.len() {
            assert!(parse(&f[..n]).is_err());
        }
    }
}
