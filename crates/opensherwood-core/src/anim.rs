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

/// The animations of one character profile plus which of them play for each posture and state,
/// per 8-way direction (`docs/formats/sprite-animations.md`: action ids 0, 6, 7, 14, 16; the alert
/// set 140 / 141 / 142 / 143 / 151; the fall set 41 / 44 / 47 / 48 / 49; the knock-out blow 123;
/// `docs/original/stealth-and-combat.md`, "Engine"). Every array always resolves: a profile without
/// a block names the documented fallback (crouch -> standing, run -> walk, alert idle / noticed /
/// alarm -> idle, alert walk / run -> walk / run, knocked down -> idle, lying -> knocked down,
/// get up -> idle, punch -> idle), so a soldier without a sneak block sneaks with its walk and a
/// civilian without an alert set stands. `has_punch` records whether the knock-out blow exists,
/// because the order model must not fake it (`docs/original/stealth-and-combat.md` 3.2: Robin
/// and the big man only).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AnimSet {
    /// Animations in profile order.
    pub animations: Vec<Vec<FrameSpec>>,
    /// Idle animation index per 8-way direction (see [`direction_of`]).
    pub idle: [u32; 8],
    /// Walk animation index per direction.
    pub walk: [u32; 8],
    /// Run animation index per direction (action 7; the walk block when the profile has none).
    #[serde(default)]
    pub run: [u32; 8],
    /// Crouched idle per direction (action 14; the idle block when the profile has none).
    #[serde(default)]
    pub crouch_idle: [u32; 8],
    /// Crouched walk ("sneak") per direction (action 16; the walk block when the profile has none).
    #[serde(default)]
    pub crouch_walk: [u32; 8],
    /// Alert idle, weapon ready (action 140; fallback idle).
    #[serde(default)]
    pub alert_idle: [u32; 8],
    /// Noticed something (action 141; fallback idle).
    #[serde(default)]
    pub noticed: [u32; 8],
    /// Raises the alarm (action 142; fallback idle).
    #[serde(default)]
    pub alarm: [u32; 8],
    /// Alert walk (action 143; fallback walk).
    #[serde(default)]
    pub alert_walk: [u32; 8],
    /// Alert run (action 151; fallback run).
    #[serde(default)]
    pub alert_run: [u32; 8],
    /// Knocked down forward, ends face down (action 41; fallback idle).
    #[serde(default)]
    pub knocked_down: [u32; 8],
    /// Knocked down backward, ends on the back (action 44; fallback `knocked_down`).
    #[serde(default)]
    pub knocked_down_back: [u32; 8],
    /// Lying face down (action 47; fallback `knocked_down`).
    #[serde(default)]
    pub lying: [u32; 8],
    /// Lying on the back (action 48; fallback `lying`).
    #[serde(default)]
    pub lying_back: [u32; 8],
    /// Gets up from the ground (action 49; fallback idle).
    #[serde(default)]
    pub get_up: [u32; 8],
    /// The knock-out blow (action 123; fallback idle, see `has_punch`).
    #[serde(default)]
    pub punch: [u32; 8],
    /// Whether the profile has the knock-out blow (action 123).
    #[serde(default)]
    pub has_punch: bool,
}

impl AnimSet {
    /// A set whose run, crouched, alert, fall and punch blocks are the standing ones (synthetic
    /// worlds and tests). Such a set can punch: synthetic units exercise the knock-out rules.
    #[must_use]
    pub fn standing_only(animations: Vec<Vec<FrameSpec>>, idle: [u32; 8], walk: [u32; 8]) -> Self {
        Self {
            animations,
            idle,
            walk,
            run: walk,
            crouch_idle: idle,
            crouch_walk: walk,
            alert_idle: idle,
            noticed: idle,
            alarm: idle,
            alert_walk: walk,
            alert_run: walk,
            knocked_down: idle,
            knocked_down_back: idle,
            lying: idle,
            lying_back: idle,
            get_up: idle,
            punch: idle,
            has_punch: true,
        }
    }

    /// Length of one loop of animation `index` in ticks (the sum of its frame durations, each at
    /// least 1); `None` when the index does not exist or the animation has no frames.
    #[must_use]
    pub fn length(&self, index: u32) -> Option<u32> {
        let frames = self.animations.get(index as usize)?;
        if frames.is_empty() {
            return None;
        }
        Some(
            frames
                .iter()
                .fold(0u32, |acc, f| acc.saturating_add(f.duration.max(1))),
        )
    }
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
            AnimSet::standing_only(
                vec![vec![f(10, 2), f(11, 1)], vec![f(20, 1)]],
                [0; 8],
                [1; 8],
            ),
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
    fn length_sums_the_frame_durations() {
        let c = catalog();
        let set = &c.sets["hero"];
        assert_eq!(set.length(0), Some(3));
        assert_eq!(set.length(1), Some(1));
        assert_eq!(set.length(2), None);
        assert!(set.has_punch);
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
