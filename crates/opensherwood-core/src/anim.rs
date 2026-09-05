//! Sprite animation state. The core does not read files: the app builds a [`Catalog`] from the
//! parsed `.rhs` profiles and attaches it to the world. Animation *state* is authoritative (it is
//! part of the snapshot and of the hash); the catalog is static data rebuilt on load.
//!
//! # The animation clock (measured 2026-09-05, `docs/original/stealth-and-combat.md` 8.4)
//!
//! The original displays a walking frame for 46.9 ms (hero walk: 22 frames of 4 px per 1.044 s
//! stride, 85.3 px/s) and the crouched cycle (14 frames whose tick halves sum to 18) for 1.50 s.
//! The reading that fits both is an animation clock of [`CLOCK_HZ`] = 64 Hz where a frame lasts
//! `(tick half + 1)` table ticks of [`CLOCKS_PER_TABLE_TICK`] = 3 clocks: a walking frame (tick
//! half 0) is one table tick, 46.875 ms; the sneak cycle is `18 + 14` = 32 table ticks = 96 clocks
//! = 1.500 s (`docs/formats/sprite-animations.md`, "Reading rules"). The world runs at
//! [`WORLD_TICK_HZ`] = 60 Hz, so a table tick is 2.8125 world ticks: the player keeps the
//! remainder in [`AnimState::elapsed`], counted in units of 1/15 clock ([`UNITS_PER_WORLD_TICK`]
//! = 16 per world tick, [`UNITS_PER_TABLE_TICK`] = 45 per table tick), and a frame changes when
//! the units reach `duration x 45`. Everything is integer and part of the snapshot.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::fixed::Fixed;

/// The original's animation clock in Hz (measured: 64 Hz fits the 46.9 ms walking frame, the
/// 93.75 ms idle step and the 1.50 s crouched cycle; `stealth-and-combat.md` 8.4).
pub const CLOCK_HZ: u32 = 64;
/// Clocks per tick of the animation tables' timing word (a frame lasts `(tick half + 1)` such
/// ticks: 3 clocks = 46.875 ms for a walking frame).
pub const CLOCKS_PER_TABLE_TICK: u32 = 3;
/// The world's tick rate in Hz (`docs/architecture.md`).
pub const WORLD_TICK_HZ: u32 = 60;
/// Clock sub-units per world tick: `CLOCK_HZ / WORLD_TICK_HZ` = 16 / 15 clocks per world tick,
/// so with 15 units per clock a world tick is 16 units ...
pub const UNITS_PER_WORLD_TICK: u32 = 16;
/// ... and a table tick (3 clocks) is 45 units. A frame of duration `d` is displayed for
/// `45 d` units = `2.8125 d` world ticks.
pub const UNITS_PER_TABLE_TICK: u32 = 45;

/// World ticks a run of `table_ticks` table ticks takes: the smallest number of world ticks
/// whose units reach it (`ceil(45 t / 16)`), which is when the player leaves the last frame of
/// an animation started with an empty accumulator. Saturates.
#[must_use]
pub const fn world_ticks(table_ticks: u32) -> u32 {
    let units = (table_ticks as u64) * (UNITS_PER_TABLE_TICK as u64);
    let ticks = units.div_ceil(UNITS_PER_WORLD_TICK as u64);
    if ticks > u32::MAX as u64 {
        u32::MAX
    } else {
        ticks as u32
    }
}

/// One frame of an animation: bank frame index, duration in table ticks, movement along the
/// facing, anchor inside the sequence box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameSpec {
    /// Frame index in the sprite bank.
    pub frame: u32,
    /// Display duration in table ticks of [`CLOCKS_PER_TABLE_TICK`] clocks: the tick half of the
    /// profile's timing word plus one (at least 1).
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

    /// World ticks one loop of animation `index` takes ([`world_ticks`] of its [`Self::length`]).
    #[must_use]
    pub fn world_ticks(&self, index: u32) -> Option<u32> {
        self.length(index).map(world_ticks)
    }

    /// Speed of a movement cycle in map pixels per world tick (24.8): the cycle's advance
    /// (the sum of its frames' `advance`) over its duration in world ticks, i.e.
    /// `advance x (CLOCK_HZ / CLOCKS_PER_TABLE_TICK) / WORLD_TICK_HZ` per table tick =
    /// `advance x 16 / 45`. The hero's walk (22 frames of 4 px, one table tick each) gives
    /// 1.42 px per tick = 85.3 px/s, the measured value; the entity moves at this constant
    /// speed rather than in per-frame steps (`docs/original/stealth-and-combat.md` 8.8).
    /// `None` when the animation does not exist, has no frames or does not move forward.
    #[must_use]
    pub fn cycle_speed(&self, index: u32) -> Option<Fixed> {
        let frames = self.animations.get(index as usize)?;
        let ticks = self.length(index)?;
        let advance: i64 = frames.iter().map(|f| i64::from(f.advance)).sum();
        if advance <= 0 {
            return None;
        }
        // raw = round(advance * 16 / 45 * 256 / ticks)
        let num = advance * i64::from(UNITS_PER_WORLD_TICK) * i64::from(Fixed::ONE.raw());
        let den = i64::from(UNITS_PER_TABLE_TICK) * i64::from(ticks);
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
    /// Clock units spent on the current frame ([`UNITS_PER_WORLD_TICK`] per world tick), below
    /// the frame's `duration x UNITS_PER_TABLE_TICK`.
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

    /// Switch to `animation` if it differs, then advance one world tick of the clock (looping):
    /// the frame changes when its `duration x UNITS_PER_TABLE_TICK` units are spent and the
    /// remainder carries over, so the long-run rate is exact.
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
        let units = anim[(self.frame % len) as usize]
            .duration
            .max(1)
            .saturating_mul(UNITS_PER_TABLE_TICK);
        self.elapsed = self.elapsed.saturating_add(UNITS_PER_WORLD_TICK);
        if self.elapsed >= units {
            // A frame lasts at least 45 units and a tick adds 16, so at most one change per tick.
            self.elapsed -= units;
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
    fn advances_on_the_clock_and_loops() {
        let c = catalog();
        let mut s = AnimState::new("hero", 0);
        assert_eq!(s.current(&c).unwrap().frame, 10);
        // Frame 10 lasts 2 table ticks = 90 units: five world ticks (80) are not enough, the
        // sixth (96) changes the frame and keeps the 6 units of remainder.
        for k in 1..=5 {
            s.advance(&c, 0);
            assert_eq!((s.frame, s.elapsed), (0, 16 * k));
        }
        s.advance(&c, 0);
        assert_eq!((s.frame, s.elapsed), (1, 6));
        // Frame 11 lasts 45 units: 6 + 16 * 3 = 54 >= 45 on the third tick, remainder 9.
        s.advance(&c, 0);
        s.advance(&c, 0);
        assert_eq!((s.frame, s.elapsed), (1, 38));
        s.advance(&c, 0);
        assert_eq!((s.frame, s.elapsed), (0, 9));
        s.advance(&c, 1);
        assert_eq!((s.animation, s.frame, s.elapsed), (1, 0, 0));
        assert_eq!(s.current(&c).unwrap().frame, 20);
    }

    #[test]
    fn a_walking_frame_lasts_two_point_eight_world_ticks() {
        // 22 frames of one table tick (the hero's walk): 22 x 45 = 990 units = 61.875 world
        // ticks per loop; over 16 loops (990 world ticks) the frame counter wraps exactly 16
        // times, so the frame rate is 64 / 3 Hz on average.
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
        for _ in 0..990 {
            s.advance(&c, 0);
            if s.frame != last {
                changes += 1;
                last = s.frame;
            }
        }
        assert_eq!(changes, 16 * 22);
        assert_eq!((s.frame, s.elapsed), (0, 0));
        assert_eq!(world_ticks(1), 3);
        assert_eq!(world_ticks(22), 62);
        assert_eq!(world_ticks(32), 90);
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
        assert_eq!(set.world_ticks(0), Some(9));
        assert!(set.has_punch);
    }

    /// The measured hero values (`docs/original/stealth-and-combat.md` 8.8): walk 85.3 px/s,
    /// run 106.7 (predicted; 101 +- 10 measured), crouched walk 17.8 px/s; the soldier's walk
    /// 42.7 and alert run 85.3 derived from the same clock.
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
        let px_per_s = |f: Fixed| f64::from(f.raw()) / 256.0 * 60.0;
        let walk = set.cycle_speed(0).unwrap();
        assert!((px_per_s(walk) - 85.3).abs() < 0.1, "{}", px_per_s(walk));
        assert_eq!(walk.raw(), 364);
        let run = set.cycle_speed(1).unwrap();
        assert!((px_per_s(run) - 106.7).abs() < 0.1, "{}", px_per_s(run));
        assert_eq!(run.raw(), 455);
        let sneak = set.cycle_speed(2).unwrap();
        assert!((px_per_s(sneak) - 17.8).abs() < 0.3, "{}", px_per_s(sneak));
        assert_eq!(sneak.raw(), 77);
        assert_eq!(
            set.length(2),
            Some(32),
            "the sneak cycle is 32 table ticks = 1.5 s"
        );
        assert_eq!(set.world_ticks(2), Some(90));
        assert_eq!(set.cycle_speed(3).unwrap().raw(), 182);
        assert_eq!(set.cycle_speed(4).unwrap().raw(), 364);
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
