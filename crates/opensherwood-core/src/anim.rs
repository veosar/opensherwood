//! Sprite animation state. The core does not read files: the app builds a [`Catalog`] from the
//! parsed `.rhs` profiles and attaches it to the world. Animation *state* is authoritative (it is
//! part of the snapshot and of the hash); the catalog is static data rebuilt on load.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One frame of an animation: bank frame index, duration in ticks, anchor inside the sequence box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameSpec {
    /// Frame index in the sprite bank.
    pub frame: u32,
    /// Display duration in ticks (at least 1).
    pub duration: u32,
    /// Offset of the frame's left edge from the entity position (anchor minus sequence origin).
    pub offset_x: i32,
    /// Offset of the frame's top edge from the entity position.
    pub offset_y: i32,
}

/// The animations of one character profile plus which of them are idle / walk per direction.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AnimSet {
    /// Animations in profile order.
    pub animations: Vec<Vec<FrameSpec>>,
    /// Idle animation index per 8-way direction (see [`direction_of`]).
    pub idle: [u32; 8],
    /// Walk animation index per direction.
    pub walk: [u32; 8],
}

/// All animation sets known to the world, by profile name (`RobinHood`, `Soldier A00`, ...).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Catalog {
    /// Sets by name.
    pub sets: BTreeMap<String, AnimSet>,
}

/// Authoritative animation state of an entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimState {
    /// Profile name in the catalog.
    pub set: String,
    /// Current animation index.
    pub animation: u32,
    /// Current frame within the animation.
    pub frame: u32,
    /// Ticks spent on the current frame.
    pub elapsed: u32,
}

impl AnimState {
    /// Start an animation set at its first idle animation.
    #[must_use]
    pub fn new(set: impl Into<String>, animation: u32) -> Self {
        Self {
            set: set.into(),
            animation,
            frame: 0,
            elapsed: 0,
        }
    }

    /// The frame currently displayed, if the catalog knows the set.
    #[must_use]
    pub fn current(&self, catalog: &Catalog) -> Option<FrameSpec> {
        let set = catalog.sets.get(&self.set)?;
        let anim = set.animations.get(self.animation as usize)?;
        anim.get(self.frame as usize).copied()
    }

    /// Switch to `animation` if it differs, then advance one tick (looping).
    pub fn advance(&mut self, catalog: &Catalog, animation: u32) {
        let Some(set) = catalog.sets.get(&self.set) else {
            return;
        };
        if animation != self.animation {
            self.animation = animation;
            self.frame = 0;
            self.elapsed = 0;
            return;
        }
        let Some(anim) = set.animations.get(self.animation as usize) else {
            return;
        };
        if anim.is_empty() {
            return;
        }
        let len = anim.len() as u32;
        let duration = anim[(self.frame % len) as usize].duration.max(1);
        self.elapsed = self.elapsed.saturating_add(1);
        if self.elapsed >= duration {
            self.elapsed = 0;
            self.frame = (self.frame % len).saturating_add(1) % len;
        }
    }
}

/// 8-way direction index from a facing in 1/256 turns (0 = screen right, clockwise): 0 = E, 1 = SE,
/// 2 = S, 3 = SW, 4 = W, 5 = NW, 6 = N, 7 = NE.
#[must_use]
pub fn direction_of(facing256: i32) -> usize {
    (((facing256.rem_euclid(256)) + 16) / 32 % 8) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Catalog {
        let f = |frame, duration| FrameSpec {
            frame,
            duration,
            offset_x: 0,
            offset_y: 0,
        };
        let mut sets = BTreeMap::new();
        sets.insert(
            "hero".into(),
            AnimSet {
                animations: vec![vec![f(10, 2), f(11, 1)], vec![f(20, 1)]],
                idle: [0; 8],
                walk: [1; 8],
            },
        );
        Catalog { sets }
    }

    #[test]
    fn advances_and_loops() {
        let c = catalog();
        let mut s = AnimState::new("hero", 0);
        assert_eq!(s.current(&c).unwrap().frame, 10);
        s.advance(&c, 0);
        assert_eq!((s.frame, s.elapsed), (0, 1));
        s.advance(&c, 0);
        assert_eq!((s.frame, s.elapsed), (1, 0));
        s.advance(&c, 0);
        assert_eq!((s.frame, s.elapsed), (0, 0));
        s.advance(&c, 1);
        assert_eq!((s.animation, s.frame), (1, 0));
        assert_eq!(s.current(&c).unwrap().frame, 20);
    }

    #[test]
    fn directions() {
        assert_eq!(direction_of(0), 0);
        assert_eq!(direction_of(32), 1);
        assert_eq!(direction_of(64), 2);
        assert_eq!(direction_of(224), 7);
        assert_eq!(direction_of(-32), 7);
        assert_eq!(direction_of(15), 0);
        assert_eq!(direction_of(16), 1);
    }
}
