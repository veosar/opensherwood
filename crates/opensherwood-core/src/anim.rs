//! Sprite animation state. The core does not read files: the app builds a [`Catalog`] from the
//! parsed `.rhs` profiles and attaches it to the world. Animation *state* is authoritative (it is
//! part of the snapshot and of the hash); the catalog is static data rebuilt on load.
//!
//! # The animation clock is the logic frame (ADR-0010)
//!
//! One logic frame of 46.875 ms drives everything ([`crate::TICK_RATE`]), so the animation
//! tables' timing word counts engine ticks one to one: a frame of the table is displayed for
//! `(timing low half + 1)` logic frames and the frame changes when the timer reaches zero after
//! stepping (`docs/original/spec-movement-animation-camera.md` ANIM-030 - ANIM-035;
//! `docs/formats/sprite-animations.md`, "Reading rules": the "table tick" of rule 2 is one
//! frame). The measured walking frame of 46.9 ms is one such frame, and the 32-frame sneak cycle
//! spans exactly 1.5 s. [`AnimState::elapsed`] is that timer, counted in frames, and the 64 Hz
//! sub-clock with its conversion into world ticks is gone.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::fixed::Fixed;

/// Logic frames a run of `table_ticks` table ticks takes: one to one since ADR-0010 (a table
/// tick *is* a logic frame), which is when the player leaves the last frame of an animation
/// started with an empty timer.
#[must_use]
pub const fn world_ticks(table_ticks: u32) -> u32 {
    table_ticks
}

/// One frame of an animation: bank frame index, duration in table ticks, movement along the
/// facing, anchor inside the sequence box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameSpec {
    /// Frame index in the sprite bank.
    pub frame: u32,
    /// Display duration in logic frames: the tick half of the profile's timing word plus one
    /// (at least 1; ANIM-031).
    pub duration: u32,
    /// Movement along the facing while the frame is displayed, in map pixels (the signed high
    /// half of the timing word; 0 for static frames). Only cycles use it ([`AnimSet::cycle_speed`]).
    #[serde(default)]
    pub advance: i32,
    /// Offset of the frame's left edge from the entity position (anchor minus sequence origin).
    pub offset_x: i32,
    /// Offset of the frame's top edge from the entity position.
    pub offset_y: i32,
}

/// The animations of one character profile plus which of them play for each posture and state,
/// per 8-way direction (`docs/formats/sprite-animations.md`: action ids 0, 6, 7, 14, 16; the alert
/// set 140 / 141 / 142 / 143 / 151; the fall set 41 / 44 / 47 / 48 / 49; the knock-out blow 123;
/// the melee set 54 / 59 / 75 / 104; `docs/original/stealth-and-combat.md`, "Engine"). Every
/// array always resolves: a profile without a block names the documented fallback (crouch ->
/// standing, run -> walk, alert idle / noticed / alarm -> idle, alert walk / run -> walk / run,
/// knocked down -> idle, lying -> knocked down, get up -> idle, punch -> idle, fight idle ->
/// idle, strike / flinch -> fight idle, powerful blow -> strike), so a soldier without a sneak block sneaks with its walk and a
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
    /// Fight idle: the stance with the weapon held level (action 54; fallback idle).
    #[serde(default)]
    pub fight_idle: [u32; 8],
    /// A quick strike (action 59; fallback `fight_idle`).
    #[serde(default)]
    pub strike: [u32; 8],
    /// The powerful blow of the forward-stroke figure (action 75, the over-the-head finishing
    /// blow; fallback `strike`).
    #[serde(default)]
    pub powerful_blow: [u32; 8],
    /// Hit in the fighting stance, stumbles back a step (action 104; fallback `fight_idle`).
    #[serde(default)]
    pub flinch: [u32; 8],
    /// Bends and picks something up (action 126, the stoop over a pick-up item or scroll,
    /// `docs/formats/sprite-animations.md`; fallback idle for a profile without the block).
    #[serde(default)]
    pub pick_up: [u32; 8],
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
            fight_idle: idle,
            strike: idle,
            powerful_blow: idle,
            flinch: idle,
            pick_up: idle,
        }
    }

    /// Length of one loop of animation `index` in table ticks (the sum of its frame durations,
    /// each at least 1); `None` when the index does not exist or the animation has no frames.
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

    /// Logic frames one loop of animation `index` takes (its [`Self::length`], ADR-0010).
    #[must_use]
    pub fn world_ticks(&self, index: u32) -> Option<u32> {
        self.length(index).map(world_ticks)
    }

    /// Speed of a movement cycle in map pixels per logic frame (24.8): the cycle's advance (the
    /// sum of its frames' `advance`) over its length in frames. The hero's walk (22 frames of
    /// 4 px, one frame each) gives 4 px per frame = 85.3 px/s at 21.333 frames per second, the
    /// measured value (ADR-0010); the entity moves at this constant speed rather than in
    /// per-frame steps (`docs/original/stealth-and-combat.md` 8.8).
    /// `None` when the animation does not exist, has no frames or does not move forward.
    #[must_use]
    pub fn cycle_speed(&self, index: u32) -> Option<Fixed> {
        let frames = self.animations.get(index as usize)?;
        let ticks = self.length(index)?;
        let advance: i64 = frames.iter().map(|f| i64::from(f.advance)).sum();
        if advance <= 0 {
            return None;
        }
        // raw = round(advance * 256 / ticks)
        let num = advance * i64::from(Fixed::ONE.raw());
        let den = i64::from(ticks);
        let raw = (num + den / 2) / den;
        Some(Fixed::from_raw(raw.clamp(0, i64::from(i32::MAX)) as i32))
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
    /// The frame timer of ANIM-031: logic frames spent on the current frame, below the frame's
    /// `duration`.
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

    /// Switch to `animation` if it differs, then step the frame timer by one logic frame
    /// (looping): the frame changes when the timer reaches the current frame's `duration`
    /// (ANIM-031, a frame lasting `hold + 1` frames) and the remainder carries over.
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
        let frames = anim[(self.frame % len) as usize].duration.max(1);
        self.elapsed = self.elapsed.saturating_add(1);
        if self.elapsed >= frames {
            // A frame lasts at least one logic frame, so at most one change per step.
            self.elapsed -= frames;
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

    fn frame(frame: u32, duration: u32) -> FrameSpec {
        FrameSpec {
            frame,
            duration,
            advance: 0,
            offset_x: 0,
            offset_y: 0,
        }
    }

    fn catalog() -> Catalog {
        let mut sets = BTreeMap::new();
        sets.insert(
            "hero".into(),
            AnimSet::standing_only(
                vec![vec![frame(10, 2), frame(11, 1)], vec![frame(20, 1)]],
                [0; 8],
                [1; 8],
            ),
        );
        Catalog { sets }
    }

    #[test]
    fn advances_on_the_frame_timer_and_loops() {
        let c = catalog();
        let mut s = AnimState::new("hero", 0);
        assert_eq!(s.current(&c).unwrap().frame, 10);
        // Frame 10 lasts two logic frames (hold 1): the first step leaves the timer at 1, the
        // second reaches the duration and makes frame 11 current (ANIM-031).
        s.advance(&c, 0);
        assert_eq!((s.frame, s.elapsed), (0, 1));
        s.advance(&c, 0);
        assert_eq!((s.frame, s.elapsed), (1, 0));
        // Frame 11 lasts one logic frame: the next step wraps to frame 10.
        s.advance(&c, 0);
        assert_eq!((s.frame, s.elapsed), (0, 0));
        s.advance(&c, 1);
        assert_eq!((s.animation, s.frame, s.elapsed), (1, 0, 0));
        assert_eq!(s.current(&c).unwrap().frame, 20);
    }

    #[test]
    fn a_walking_frame_lasts_one_logic_frame() {
        // 22 frames of one table tick (the hero's walk): one logic frame each since ADR-0010,
        // so a loop is 22 frames = 1.031 s and 16 loops change the frame 16 x 22 times.
        let mut sets = BTreeMap::new();
        let walk: Vec<FrameSpec> = (0..22).map(|i| frame(i, 1)).collect();
        sets.insert(
            "hero".into(),
            AnimSet::standing_only(vec![walk], [0; 8], [0; 8]),
        );
        let c = Catalog { sets };
        let mut s = AnimState::new("hero", 0);
        let mut changes = 0;
        let mut last = 0;
        for _ in 0..16 * 22 {
            s.advance(&c, 0);
            if s.frame != last {
                changes += 1;
                last = s.frame;
            }
        }
        assert_eq!(changes, 16 * 22);
        assert_eq!((s.frame, s.elapsed), (0, 0));
        assert_eq!(world_ticks(1), 1);
        assert_eq!(world_ticks(22), 22);
        assert_eq!(world_ticks(32), 32);
        assert_eq!(world_ticks(0), 0);
        assert_eq!(world_ticks(u32::MAX), u32::MAX);
    }

    #[test]
    fn length_sums_the_frame_durations() {
        let c = catalog();
        let set = &c.sets["hero"];
        assert_eq!(set.length(0), Some(3));
        assert_eq!(set.length(1), Some(1));
        assert_eq!(set.length(2), None);
        assert_eq!(set.world_ticks(0), Some(3));
        assert!(set.has_punch);
    }

    /// The measured hero values (`docs/original/stealth-and-combat.md` 8.8; ADR-0010): walk
    /// 85.3 px/s = 4 px per logic frame, run 106.7 (predicted; 101 +- 10 measured), crouched
    /// walk 18.0 px/s; the soldier's walk 42.7 and alert run 85.3 from the same frame.
    #[test]
    fn cycle_speeds_match_the_measured_hero_values() {
        let cycle = |n: u32, ticks: &[u32], adv: &[i32]| -> Vec<FrameSpec> {
            (0..n as usize)
                .map(|i| FrameSpec {
                    frame: i as u32,
                    duration: ticks[i % ticks.len()],
                    advance: adv[i % adv.len()],
                    offset_x: 0,
                    offset_y: 0,
                })
                .collect()
        };
        let mut sneak_ticks = vec![3, 3, 3, 3];
        sneak_ticks.extend([2; 10]);
        let mut sneak_adv = vec![1];
        sneak_adv.extend([2; 13]);
        let set = AnimSet::standing_only(
            vec![
                cycle(22, &[1], &[4]),                // hero walk 6
                cycle(12, &[1], &[5]),                // hero run 7
                cycle(14, &sneak_ticks, &sneak_adv),  // hero sneak 16
                cycle(22, &[1], &[2]),                // soldier walk
                cycle(12, &[1], &[4]),                // soldier alert run 151
                cycle(6, &[7, 3, 3, 16, 5, 5], &[0]), // an idle: no movement
                cycle(7, &[1], &[-7]),                // a backward fall
                Vec::new(),
            ],
            [0; 8],
            [0; 8],
        );
        let px_per_s = |f: Fixed| f64::from(f.raw()) / 256.0 * f64::from(crate::TICK_RATE.0) / 3.0;
        let walk = set.cycle_speed(0).unwrap();
        assert!((px_per_s(walk) - 85.3).abs() < 0.1, "{}", px_per_s(walk));
        assert_eq!(walk.raw(), 4 * 256);
        let run = set.cycle_speed(1).unwrap();
        assert!((px_per_s(run) - 106.7).abs() < 0.1, "{}", px_per_s(run));
        assert_eq!(run.raw(), 5 * 256);
        let sneak = set.cycle_speed(2).unwrap();
        assert!((px_per_s(sneak) - 18.0).abs() < 0.3, "{}", px_per_s(sneak));
        assert_eq!(sneak.raw(), 216);
        assert_eq!(
            set.length(2),
            Some(32),
            "the sneak cycle is 32 logic frames = 1.5 s"
        );
        assert_eq!(set.world_ticks(2), Some(32));
        assert_eq!(set.cycle_speed(3).unwrap().raw(), 2 * 256);
        assert_eq!(set.cycle_speed(4).unwrap().raw(), 4 * 256);
        assert_eq!(set.cycle_speed(5), None);
        assert_eq!(set.cycle_speed(6), None);
        assert_eq!(set.cycle_speed(7), None);
        assert_eq!(set.cycle_speed(8), None);
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
