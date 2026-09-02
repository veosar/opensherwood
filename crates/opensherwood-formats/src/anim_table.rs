//! Animation index layout of character profiles (`docs/formats/sprite-animations.md`).
//!
//! A character sequence lists its animations in blocks of 16, one per facing direction. Every
//! animation of a block carries the same value in [`Animation::unknown_0x0c`], and that value is a
//! global *action id* shared by all profiles (0 = stand idle, 6 = walk, 7 = run, ...). Which ids a
//! profile has, and in which order, depends on its family (heroes, soldiers, civilians, objects), so
//! lookups go through the id, never through a fixed index.
//!
//! Pure data helper: no I/O, no game logic.

use crate::rhs::{Animation, Profile, Sequence};

/// Number of facing directions per action block.
pub const DIRECTIONS: usize = 16;

/// A facing direction in screen space, in the order the sprite bank stores them: index 0 faces
/// screen-up (the character is seen from behind), increasing clockwise, index 4 faces screen-right,
/// 8 faces the viewer, 12 faces screen-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Direction {
    /// Screen-up (away from the viewer).
    N = 0,
    /// Up, slightly right.
    NNE = 1,
    /// Up-right.
    NE = 2,
    /// Right, slightly up.
    ENE = 3,
    /// Screen-right.
    E = 4,
    /// Right, slightly down.
    ESE = 5,
    /// Down-right.
    SE = 6,
    /// Down, slightly right.
    SSE = 7,
    /// Screen-down (facing the viewer).
    S = 8,
    /// Down, slightly left.
    SSW = 9,
    /// Down-left.
    SW = 10,
    /// Left, slightly down.
    WSW = 11,
    /// Screen-left.
    W = 12,
    /// Left, slightly up.
    WNW = 13,
    /// Up-left.
    NW = 14,
    /// Up, slightly left.
    NNW = 15,
}

impl Direction {
    /// All directions in sprite order.
    pub const ALL: [Direction; DIRECTIONS] = [
        Direction::N,
        Direction::NNE,
        Direction::NE,
        Direction::ENE,
        Direction::E,
        Direction::ESE,
        Direction::SE,
        Direction::SSE,
        Direction::S,
        Direction::SSW,
        Direction::SW,
        Direction::WSW,
        Direction::W,
        Direction::WNW,
        Direction::NW,
        Direction::NNW,
    ];

    /// Sprite index 0..16 of this direction.
    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    /// Direction from a sprite index (taken modulo 16).
    #[must_use]
    pub fn from_index(index: usize) -> Direction {
        Direction::ALL[index % DIRECTIONS]
    }

    /// Nearest direction to a facing in 1/256 turns, `0` = screen-right (+x), increasing clockwise
    /// on screen (the `Entity::facing256` convention of `opensherwood-core`). Facing 0 maps to
    /// [`Direction::E`], 64 to [`Direction::S`], 192 to [`Direction::N`]; sectors are centred on the
    /// direction (facing 7 is still E, facing 8 is ESE).
    #[must_use]
    pub fn from_facing256(facing256: i32) -> Direction {
        let sector = ((facing256.rem_euclid(256) + 8) / 16) as usize;
        Direction::from_index(sector + Direction::E.index())
    }

    /// Centre of this direction's sector in 1/256 turns (inverse of [`Direction::from_facing256`]).
    #[must_use]
    pub fn to_facing256(self) -> i32 {
        (((self.index() + DIRECTIONS - Direction::E.index()) % DIRECTIONS) * 16) as i32
    }

    /// Direction from an 8-way octant in the `opensherwood-core` order (0 = E, 1 = SE, 2 = S, ...,
    /// 7 = NE), i.e. `direction_of(facing256)`.
    #[must_use]
    pub fn from_octant(octant: usize) -> Direction {
        Direction::from_index((octant % 8) * 2 + Direction::E.index())
    }

    /// Screen-space unit vector of this direction scaled by 256 (x right, y down); the sprite bank's
    /// own motion vectors follow a 2:1 ellipse, this does not.
    #[must_use]
    pub fn vector256(self) -> (i32, i32) {
        // sin/cos of k * 22.5 degrees, times 256, rounded.
        const S: [i32; 16] = [
            0, 98, 181, 237, 256, 237, 181, 98, 0, -98, -181, -237, -256, -237, -181, -98,
        ];
        let i = self.index();
        (S[i], -S[(i + 4) % 16])
    }
}

/// Global action id of an animation block (the value of [`Animation::unknown_0x0c`], constant over
/// the 16 animations of a block). The named constants are the ids identified by looking at the
/// rendered animations (see the spec for what was seen); any other value is still a valid id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ActionId(pub u16);

impl ActionId {
    /// Standing idle, 6-frame ping-pong breathing loop.
    pub const IDLE: ActionId = ActionId(0);
    /// Standing fidget (looks around, scratches).
    pub const IDLE_FIDGET: ActionId = ActionId(1);
    /// Walk cycle (22 frames).
    pub const WALK: ActionId = ActionId(6);
    /// Run cycle (12 frames), upright.
    pub const RUN: ActionId = ActionId(7);
    /// Fast crouched run (16 or 32 frames).
    pub const SPRINT: ActionId = ActionId(10);
    /// Decelerating stop after a sprint (7 -> 2 pixels per frame).
    pub const SPRINT_STOP: ActionId = ActionId(12);
    /// Crouch down from standing (heroes only).
    pub const CROUCH_DOWN: ActionId = ActionId(13);
    /// Crouched idle (heroes only).
    pub const CROUCH_IDLE: ActionId = ActionId(14);
    /// Crouched walk / sneak cycle (heroes only).
    pub const SNEAK: ActionId = ActionId(16);
    /// Stand up from the crouch (heroes only).
    pub const STAND_UP: ActionId = ActionId(18);
    /// Climb a wall or ladder upwards (climbers only).
    pub const CLIMB_UP: ActionId = ActionId(20);
    /// Climb a wall or ladder downwards (climbers only).
    pub const CLIMB_DOWN: ActionId = ActionId(21);
    /// Draw the weapon (sword out of the scabbard / staff readied).
    pub const DRAW_WEAPON: ActionId = ActionId(40);
    /// Hit, staggers and falls flat on the back.
    pub const KNOCKED_DOWN: ActionId = ActionId(41);
    /// Hit, collapses (second fall variant, moves backwards).
    pub const KNOCKED_DOWN_BACK: ActionId = ActionId(44);
    /// Lying on the ground, one frame (unconscious / dead pose).
    pub const LYING: ActionId = ActionId(47);
    /// Get up from the ground.
    pub const GET_UP: ActionId = ActionId(49);
    /// Melee attack (first strike of the combat set).
    pub const ATTACK: ActionId = ActionId(52);
    /// Combat stance idle, weapon held ready.
    pub const FIGHT_IDLE: ActionId = ActionId(54);
}

/// Per-frame timing word of a frame reference split into its two halves. The `.rhs` field parsed
/// as `duration: u32` is really `u16 ticks` (low half) followed by `i16 unknown_0x06` (high half),
/// which the spec infers to be the movement along the facing during that frame (walk 2..4, run
/// 3..5, sprint 5..7, stops 7,6,5,4,3,2, negative for backward steps). Idle frames have ticks and
/// no advance; walk/run/sprint frames have an advance and zero ticks, i.e. they are distance-timed.
#[must_use]
pub fn split_duration(word: u32) -> (u16, i16) {
    ((word & 0xFFFF) as u16, (word >> 16) as i16)
}

/// The action-to-block table of one character sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationTable {
    /// `(action id, index of the block's first animation)` in file order.
    blocks: Vec<(ActionId, usize)>,
    animation_count: usize,
}

impl AnimationTable {
    /// Build the table of a sequence. Returns `None` when the animation count is not a positive
    /// multiple of 16 (objects such as nets and flags) or when a block mixes several ids.
    #[must_use]
    pub fn from_sequence(seq: &Sequence) -> Option<Self> {
        let n = seq.animations.len();
        if n == 0 || !n.is_multiple_of(DIRECTIONS) {
            return None;
        }
        let mut blocks = Vec::with_capacity(n / DIRECTIONS);
        for (b, block) in seq.animations.chunks(DIRECTIONS).enumerate() {
            let id = block[0].unknown_0x0c;
            if block.iter().any(|a| a.unknown_0x0c != id) {
                return None;
            }
            blocks.push((ActionId(id), b * DIRECTIONS));
        }
        Some(Self {
            blocks,
            animation_count: n,
        })
    }

    /// Build the table of the first sequence of a profile.
    #[must_use]
    pub fn from_profile(profile: &Profile) -> Option<Self> {
        profile.sequences.first().and_then(Self::from_sequence)
    }

    /// Number of 16-animation blocks.
    #[must_use]
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Number of animations in the sequence.
    #[must_use]
    pub fn animation_count(&self) -> usize {
        self.animation_count
    }

    /// Action ids in block order (a profile may repeat an id; both blocks are listed).
    pub fn action_ids(&self) -> impl Iterator<Item = ActionId> + '_ {
        self.blocks.iter().map(|&(id, _)| id)
    }

    /// Index of the first animation of the first block with this action id.
    #[must_use]
    pub fn block_start(&self, action: ActionId) -> Option<usize> {
        self.blocks
            .iter()
            .find(|&&(id, _)| id == action)
            .map(|&(_, start)| start)
    }

    /// Whether the profile has a block for this action.
    #[must_use]
    pub fn has(&self, action: ActionId) -> bool {
        self.block_start(action).is_some()
    }

    /// Animation index of `action` facing `direction`.
    #[must_use]
    pub fn animation(&self, action: ActionId, direction: Direction) -> Option<usize> {
        self.block_start(action).map(|s| s + direction.index())
    }

    /// Standing idle.
    #[must_use]
    pub fn idle(&self, direction: Direction) -> Option<usize> {
        self.animation(ActionId::IDLE, direction)
    }

    /// Walk cycle.
    #[must_use]
    pub fn walk(&self, direction: Direction) -> Option<usize> {
        self.animation(ActionId::WALK, direction)
    }

    /// Run cycle.
    #[must_use]
    pub fn run(&self, direction: Direction) -> Option<usize> {
        self.animation(ActionId::RUN, direction)
    }

    /// Fast crouched run.
    #[must_use]
    pub fn sprint(&self, direction: Direction) -> Option<usize> {
        self.animation(ActionId::SPRINT, direction)
    }

    /// Crouched walk (heroes only).
    #[must_use]
    pub fn sneak(&self, direction: Direction) -> Option<usize> {
        self.animation(ActionId::SNEAK, direction)
    }

    /// Crouched idle (heroes only).
    #[must_use]
    pub fn crouch_idle(&self, direction: Direction) -> Option<usize> {
        self.animation(ActionId::CROUCH_IDLE, direction)
    }

    /// Combat stance idle.
    #[must_use]
    pub fn fight_idle(&self, direction: Direction) -> Option<usize> {
        self.animation(ActionId::FIGHT_IDLE, direction)
    }

    /// Lying on the ground.
    #[must_use]
    pub fn lying(&self, direction: Direction) -> Option<usize> {
        self.animation(ActionId::LYING, direction)
    }
}

/// Displacement of the entity over an animation, in screen pixels, as stored in the animation
/// header relative to the sequence origin (`unknown_0x04 - origin_x`, `unknown_0x08 - origin_y`).
/// Zero for most blocks; for climbs, jumps and a few held poses it traces a 2:1 ellipse around the
/// origin per direction. Inferred, see the spec.
#[must_use]
pub fn displacement(seq: &Sequence, anim: &Animation) -> (i32, i32) {
    (
        anim.unknown_0x04 as i32 - seq.origin_x as i32,
        anim.unknown_0x08 as i32 - seq.origin_y as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhs::FrameRef;

    fn animation(id: u16, frame: u32) -> Animation {
        Animation {
            unknown_0x02: 0,
            unknown_0x04: 150,
            unknown_0x08: 150,
            unknown_0x0c: id,
            frames: vec![FrameRef {
                frame,
                duration: 3 | (2 << 16),
                anchor_x: 140,
                anchor_y: 100,
                unknown_0x0c: 0,
            }],
        }
    }

    fn sequence(ids: &[u16]) -> Sequence {
        let mut animations = Vec::new();
        for (b, &id) in ids.iter().enumerate() {
            for d in 0..DIRECTIONS {
                animations.push(animation(id, (b * DIRECTIONS + d) as u32));
            }
        }
        Sequence {
            name: "test".into(),
            width: 90,
            height: 108,
            origin_x: 150,
            origin_y: 150,
            animations,
        }
    }

    #[test]
    fn direction_from_facing() {
        assert_eq!(Direction::from_facing256(0), Direction::E);
        assert_eq!(Direction::from_facing256(7), Direction::E);
        assert_eq!(Direction::from_facing256(8), Direction::ESE);
        assert_eq!(Direction::from_facing256(64), Direction::S);
        assert_eq!(Direction::from_facing256(128), Direction::W);
        assert_eq!(Direction::from_facing256(192), Direction::N);
        assert_eq!(Direction::from_facing256(-64), Direction::N);
        assert_eq!(Direction::from_facing256(255), Direction::E);
        for d in Direction::ALL {
            assert_eq!(Direction::from_facing256(d.to_facing256()), d);
            assert_eq!(Direction::from_index(d.index()), d);
        }
    }

    #[test]
    fn direction_from_octant() {
        assert_eq!(Direction::from_octant(0), Direction::E);
        assert_eq!(Direction::from_octant(1), Direction::SE);
        assert_eq!(Direction::from_octant(2), Direction::S);
        assert_eq!(Direction::from_octant(4), Direction::W);
        assert_eq!(Direction::from_octant(6), Direction::N);
        assert_eq!(Direction::from_octant(7), Direction::NE);
    }

    #[test]
    fn direction_vectors() {
        assert_eq!(Direction::N.vector256(), (0, -256));
        assert_eq!(Direction::E.vector256(), (256, 0));
        assert_eq!(Direction::S.vector256(), (0, 256));
        assert_eq!(Direction::W.vector256(), (-256, 0));
        assert_eq!(Direction::SE.vector256(), (181, 181));
    }

    #[test]
    fn table_lookups() {
        let seq = sequence(&[0, 1, 2, 4, 3, 5, 8, 6, 7, 50, 51, 12, 9, 11, 10]);
        let t = AnimationTable::from_sequence(&seq).unwrap();
        assert_eq!(t.block_count(), 15);
        assert_eq!(t.animation_count(), 240);
        assert_eq!(t.idle(Direction::N), Some(0));
        assert_eq!(t.idle(Direction::E), Some(4));
        assert_eq!(t.walk(Direction::N), Some(7 * 16));
        assert_eq!(t.walk(Direction::W), Some(7 * 16 + 12));
        assert_eq!(t.run(Direction::S), Some(8 * 16 + 8));
        assert_eq!(t.sprint(Direction::NNW), Some(14 * 16 + 15));
        assert_eq!(t.sneak(Direction::N), None);
        assert!(!t.has(ActionId::SNEAK));
        assert!(t.has(ActionId::SPRINT_STOP));
        assert_eq!(
            t.action_ids().map(|a| a.0).collect::<Vec<_>>(),
            vec![0, 1, 2, 4, 3, 5, 8, 6, 7, 50, 51, 12, 9, 11, 10]
        );
        let profile = Profile {
            bank_generation: crate::dic::BANK_GENERATION_ID,
            sequences: vec![seq],
        };
        assert_eq!(AnimationTable::from_profile(&profile), Some(t));
    }

    #[test]
    fn duplicate_id_uses_first_block() {
        let seq = sequence(&[0, 180, 6, 180]);
        let t = AnimationTable::from_sequence(&seq).unwrap();
        assert_eq!(t.block_start(ActionId(180)), Some(16));
        assert_eq!(t.action_ids().count(), 4);
    }

    #[test]
    fn rejects_irregular_sequences() {
        let mut seq = sequence(&[0]);
        seq.animations.pop();
        assert!(AnimationTable::from_sequence(&seq).is_none());
        let mut mixed = sequence(&[0]);
        mixed.animations[3].unknown_0x0c = 9;
        assert!(AnimationTable::from_sequence(&mixed).is_none());
        let empty = sequence(&[]);
        assert!(AnimationTable::from_sequence(&empty).is_none());
    }

    #[test]
    fn duration_halves_and_displacement() {
        assert_eq!(split_duration(3 | (2 << 16)), (3, 2));
        assert_eq!(split_duration(0xFFF7_0002), (2, -9));
        assert_eq!(split_duration(15), (15, 0));
        let seq = sequence(&[0]);
        let mut a = animation(0, 1);
        assert_eq!(displacement(&seq, &a), (0, 0));
        a.unknown_0x04 = 180;
        a.unknown_0x08 = 136;
        assert_eq!(displacement(&seq, &a), (30, -14));
    }

    /// Data-backed: the fixed prefix and the family differences on the retail files.
    #[test]
    fn retail_profiles_layout() {
        let Ok(dir) = std::env::var("OPENSHERWOOD_GAME_DIR") else {
            eprintln!("OPENSHERWOOD_GAME_DIR not set; skipping");
            return;
        };
        let load = |name: &str| {
            let path = std::path::Path::new(&dir)
                .join("DATA")
                .join("Characters")
                .join(name);
            let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            crate::rhs::parse(&bytes).unwrap()
        };
        let prefix = [0, 1, 2, 4, 3, 5, 8, 6, 7, 50, 51, 12, 9, 11, 10];
        for name in ["RobinHood.rhs", "Soldier A00.rhs", "Child.rhs"] {
            let profile = load(name);
            let table = AnimationTable::from_profile(&profile).expect(name);
            let ids: Vec<u16> = table.action_ids().map(|a| a.0).collect();
            assert_eq!(&ids[..15], &prefix, "{name}");
            assert_eq!(table.idle(Direction::N), Some(0), "{name}");
            assert_eq!(table.walk(Direction::N), Some(7 * 16), "{name}");
            assert_eq!(table.run(Direction::N), Some(8 * 16), "{name}");
            assert_eq!(table.sprint(Direction::N), Some(14 * 16), "{name}");
            let seq = &profile.sequences[0];
            // Idle frames are tick-timed with no advance; walk frames are distance-timed.
            let idle = &seq.animations[table.idle(Direction::E).unwrap()];
            for f in &idle.frames {
                let (ticks, advance) = split_duration(f.duration);
                assert!(ticks > 0 && advance == 0, "{name}");
            }
            let walk = &seq.animations[table.walk(Direction::E).unwrap()];
            assert_eq!(walk.frames.len(), 22, "{name}");
            for f in &walk.frames {
                let (ticks, advance) = split_duration(f.duration);
                assert!(ticks == 0 && advance > 0, "{name}");
            }
            // Blocks with a displacement trace the direction ellipse: N is straight up, E straight right.
            for b in 0..table.block_count() {
                let block = &seq.animations[b * DIRECTIONS..(b + 1) * DIRECTIONS];
                if block.iter().all(|a| displacement(seq, a) == (0, 0)) {
                    continue;
                }
                let (nx, ny) = displacement(seq, &block[Direction::N.index()]);
                let (ex, ey) = displacement(seq, &block[Direction::E.index()]);
                let (sx, sy) = displacement(seq, &block[Direction::S.index()]);
                let (wx, wy) = displacement(seq, &block[Direction::W.index()]);
                // N and S are vertical, E and W horizontal, opposite directions cancel; the vector
                // points along the facing (ny < 0) or, for a few landing blocks, against it.
                assert_eq!((nx, sx, ey, wy), (0, 0, 0, 0), "{name} block {b}");
                assert!(ny != 0 && ex != 0, "{name} block {b}");
                assert_eq!((ny, ex), (-sy, -wx), "{name} block {b}");
                assert_eq!(ny < 0, ex > 0, "{name} block {b}");
            }
        }
        let robin = AnimationTable::from_profile(&load("RobinHood.rhs")).unwrap();
        assert_eq!(robin.block_count(), 142);
        assert_eq!(robin.sneak(Direction::N), Some(20 * 16));
        assert_eq!(robin.crouch_idle(Direction::N), Some(17 * 16));
        assert_eq!(
            robin.animation(ActionId::CLIMB_UP, Direction::N),
            Some(27 * 16)
        );
        assert_eq!(robin.fight_idle(Direction::N), Some(58 * 16));
        let soldier = AnimationTable::from_profile(&load("Soldier A00.rhs")).unwrap();
        assert_eq!(soldier.block_count(), 128);
        assert!(!soldier.has(ActionId::SNEAK));
        assert_eq!(
            soldier.animation(ActionId::DRAW_WEAPON, Direction::N),
            Some(21 * 16)
        );
        assert_eq!(soldier.lying(Direction::N), Some(23 * 16));
        assert_eq!(soldier.fight_idle(Direction::N), Some(30 * 16));
        let child = AnimationTable::from_profile(&load("Child.rhs")).unwrap();
        assert_eq!(child.block_count(), 46);
        assert!(!child.has(ActionId::FIGHT_IDLE));
        assert_eq!(child.lying(Direction::N), Some(23 * 16));
    }
}
