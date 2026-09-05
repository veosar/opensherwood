//! The authoritative world. M0: a synthetic scenario with a player unit, a patrolling guard and
//! rectangular obstacles, driven only by canonical input events. M2 groundwork: a scrollable camera
//! over a map of arbitrary size and sprite animation state.
//!
//! Every field of [`World`] except `catalog` is authoritative: it is serialised in snapshots,
//! encoded in the canonical hash (ADR-0004) and validated on restore.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::ai::{
    AiState, DAMAGE_NUMBER_TICKS, ENERGY_MAX, FIGURE_MIN_STROKE, FightPose, Figure, action_id,
    wanted_animation,
};
use crate::anim::{AnimState, Catalog, UNITS_PER_TABLE_TICK, direction_of};
use crate::fixed::Fixed;
use crate::geom::Geometry;
use crate::hash::{Encoder, HASH_SCHEMA_VERSION, Hashes, total};
use crate::input::{Button, InputEvent, Key, button_tag, encode_key};
use crate::nav::{DEFAULT_SEARCH_WORK, NavError, NavGrid};
use crate::rng::Rng;
use crate::vm::{
    Assumption, Program, SCRIPT_RNG_STREAM, ScriptObservation, VmState, charge_budget,
};

/// Stable entity identifier (index + generation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityId {
    /// Slot index.
    pub index: u32,
    /// Generation; incremented when a slot is reused.
    pub generation: u32,
}

/// What an entity is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    /// Player-controlled character.
    Player,
    /// Patrolling guard.
    Guard,
    /// Static obstacle (axis-aligned box centred on the position with half extents in `patrol[0]`).
    Obstacle,
}

impl EntityKind {
    /// Stable tag for canonical encodings (never derived from declaration order).
    #[must_use]
    pub fn tag(self) -> u8 {
        match self {
            EntityKind::Player => 1,
            EntityKind::Guard => 2,
            EntityKind::Obstacle => 3,
        }
    }
}

/// Movement mode of an actor's current order (`docs/original/ui-flow.md` 9.4: a click on the
/// ground walks, a double click runs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gait {
    /// Walking (action 6 of the animation table).
    #[default]
    Walk,
    /// Running (action 7): the speed of the profile's run cycle (measured: hero 106.7 px/s
    /// predicted, 101 +- 10 measured; `docs/original/stealth-and-combat.md` 8.3), or the
    /// walking speed times [`FALLBACK_RUN_SPEED_RATIO`] without a cycle.
    Run,
}

impl Gait {
    /// Stable tag for canonical encodings (never derived from declaration order).
    #[must_use]
    pub fn tag(self) -> u8 {
        match self {
            Gait::Walk => 1,
            Gait::Run => 2,
        }
    }
}

/// Body posture of an actor (`c` crouches, `s` stands; the kneel / stand icons of the HUD).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Posture {
    /// Upright.
    #[default]
    Standing,
    /// Crouched: the sneak animation (action 16) and the crouched idle (14), moving at the sneak
    /// cycle's speed whatever the gait (measured: 17.8 px/s for the hero, 8.2), or the walking
    /// speed times [`FALLBACK_SNEAK_SPEED_RATIO`] without a cycle.
    Crouched,
}

impl Posture {
    /// Stable tag for canonical encodings (never derived from declaration order).
    #[must_use]
    pub fn tag(self) -> u8 {
        match self {
            Posture::Standing => 1,
            Posture::Crouched => 2,
        }
    }
}

/// An entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
    /// Id.
    pub id: EntityId,
    /// Kind.
    pub kind: EntityKind,
    /// Position in map pixels (24.8).
    pub x: Fixed,
    /// Position.
    pub y: Fixed,
    /// Half extents for obstacles; selection radius for actors.
    pub size: Fixed,
    /// Walking speed in map pixels per world tick (24.8), used when the entity's profile has no
    /// movement cycle to read the speed from (synthetic units, profiles without a table):
    /// [`Entity::effective_speed`].
    pub speed: Fixed,
    /// Current movement target (the final destination).
    pub target: Option<(Fixed, Fixed)>,
    /// Remaining path waypoints towards the target (map pixels, 24.8), first = next.
    #[serde(default)]
    pub path: Vec<(Fixed, Fixed)>,
    /// Patrol waypoints (guards); half extents for obstacles (exactly one entry).
    pub patrol: Vec<(Fixed, Fixed)>,
    /// Index of the next patrol waypoint.
    pub patrol_index: u32,
    /// Ticks to wait before moving on.
    pub wait_ticks: u32,
    /// Facing in 1/256 turns (0 = +x, increasing clockwise on screen), always in `0..256`.
    pub facing256: i32,
    /// Alive.
    pub alive: bool,
    /// Sprite animation state, if the entity is drawn with a sprite.
    pub anim: Option<AnimState>,
    /// Waypoint program this entity executes (index into [`World::programs`]); `None` = the
    /// legacy patrol over `patrol` (synthetic guards) or idle.
    #[serde(default)]
    pub program: Option<u32>,
    /// Program counter: index of the next instruction to execute.
    #[serde(default)]
    pub pc: u32,
    /// Active (script natives 113 / 114): inactive entities are not drawn, not stepped and not
    /// pickable.
    #[serde(default = "default_true")]
    pub active: bool,
    /// The script locked this entity's AI (natives 134 / 135): its waypoint program is paused.
    #[serde(default)]
    pub ai_locked: bool,
    /// Movement mode of the current order: every order walks unless a double click made it a
    /// run; reset to walking when the order ends.
    #[serde(default)]
    pub gait: Gait,
    /// Standing or crouched (player characters, keys `c` / `s`).
    #[serde(default)]
    pub posture: Posture,
    /// Side (players are `Player`, soldiers `Enemy`, civilians `Civilian`; obstacles carry the
    /// default). Only enemy soldiers perceive (`ai.rs`).
    #[serde(default = "default_team")]
    pub team: Team,
    /// Behaviour state (`ai.rs`): normal, the alert states, the knock-out states.
    #[serde(default)]
    pub ai_state: AiState,
    /// Ticks left in the current timed state (`ai_state`); 0 in untimed states.
    #[serde(default)]
    pub state_ticks: u32,
    /// Where the soldier last perceived a player character (map pixels, 24.8), while alerted.
    #[serde(default)]
    pub last_seen: Option<(Fixed, Fixed)>,
    /// Where the alert or the knock-out took the soldier from: he returns there afterwards.
    #[serde(default)]
    pub alert_origin: Option<(Fixed, Fixed)>,
    /// The enemy this player character was ordered to attack (walk into reach, then punch).
    #[serde(default)]
    pub attack_target: Option<EntityId>,
    /// The sprite action id the entity reported last tick (`ai::action_id`); a change fires the
    /// script's `ActionChange`.
    #[serde(default)]
    pub action: u32,
    /// Hit points left (`combat-measurements.md` 1.2: the red bar, never regenerating); 0
    /// exactly when the entity is dead. Obstacles carry 0.
    #[serde(default = "default_hit_points")]
    pub hp: i32,
    /// Full hit points: the hero's measured 100, a soldier's SD `pre[0]` (80 for a blue
    /// halberdier, confirmed), [`crate::ai::DEFAULT_HIT_POINTS`] for everyone without a
    /// profile value (the other heroes and the civilians: hypothesis).
    #[serde(default = "default_hit_points")]
    pub hp_max: i32,
    /// Energy units left, 0..=[`crate::ai::ENERGY_MAX`] (the blue bar): a landed soldier hit
    /// costs the soldier one, the hero's powerful blow two; regained one unit per interval.
    #[serde(default = "default_energy")]
    pub energy: i32,
    /// Ticks until the next unit of energy is regained; 0 exactly when the energy is full.
    #[serde(default)]
    pub energy_ticks: u32,
    /// The opponent of the current melee (`Fighting` state, exactly then): a player character's
    /// enemy soldier or a soldier's player character.
    #[serde(default)]
    pub foe: Option<EntityId>,
    /// What the fighter is doing in the stance (`crate::ai::FightPose`); `Idle` outside a fight.
    #[serde(default)]
    pub pose: FightPose,
    /// Ticks left in the current pose; 0 exactly in the idle pose.
    #[serde(default)]
    pub pose_ticks: u32,
    /// Ticks until the next automatic swing while fighting (0 when due or outside a fight).
    #[serde(default)]
    pub swing_ticks: u32,
    /// The figure a player character drew (`crate::ai::Figure`), pending until he fights the
    /// target of the order; only with an attack order or in a fight.
    #[serde(default)]
    pub figure: Option<Figure>,
    /// Knock-out resistance (`profile.md`, SD `p4`: hypothesis, 0..100): scales the knock-out
    /// timer, 100 makes the blow fail.
    #[serde(default)]
    pub knockout_resistance: i32,
    /// Gait of the walks this NPC's waypoint program issues (script native 140: hypothesis
    /// 0 walk / 1 run / 2 sprint, the last played as a run).
    #[serde(default)]
    pub npc_gait: Gait,
    /// Fell backward (struck from the front: actions 44 / 48) rather than forward (41 / 47).
    #[serde(default)]
    pub fell_backward: bool,
    /// The current alert came through the noise channel (a running character heard: measured,
    /// `docs/original/stealth-and-combat.md` 8.6) rather than the view cone (hypothesis), so the
    /// alert action ids it produces record no `Assumption::Perception` (`vm.rs`). Only set in
    /// an alert state.
    #[serde(default)]
    pub heard: bool,
}

impl Entity {
    /// Distance covered per world tick: the speed of the movement cycle the entity plays in its
    /// state, posture and gait (`ai::wanted_animation`: walk 6, run 7, sneak 16, the alert walk
    /// 143 and run 151 of an alerted soldier, or the documented fallback block), read from the
    /// catalog as the cycle's advance over its duration on the animation clock
    /// (`AnimSet::cycle_speed`: hero walk 85.3 px/s, run 106.7, sneak 18.0; soldier walk 42.7,
    /// run 64, alert walk 64, alert run 85.3; measured / derived, `docs/original/stealth-and-combat.md`
    /// 8.8). Without a catalog, a set or a forward-moving cycle: the walking `speed`, times
    /// [`FALLBACK_RUN_SPEED_RATIO`] when running, times [`FALLBACK_SNEAK_SPEED_RATIO`] when
    /// crouched (a crouched actor never runs: the table has one crouched movement, the sneak).
    #[must_use]
    pub fn effective_speed(&self, catalog: &Catalog) -> Fixed {
        let from_table = self
            .anim
            .as_ref()
            .and_then(|a| catalog.sets.get(&a.set))
            .and_then(|set| set.cycle_speed(wanted_animation(self, set)));
        from_table.unwrap_or_else(|| {
            let (num, den) = match (self.posture, self.gait) {
                (Posture::Crouched, _) => FALLBACK_SNEAK_SPEED_RATIO,
                (Posture::Standing, Gait::Run) => FALLBACK_RUN_SPEED_RATIO,
                (Posture::Standing, Gait::Walk) => (1, 1),
            };
            let raw = (i64::from(self.speed.raw()) * i64::from(num) + i64::from(den) / 2)
                / i64::from(den);
            Fixed::from_raw(raw.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
        })
    }
}

fn default_true() -> bool {
    true
}

fn default_team() -> Team {
    Team::Civilian
}

fn default_hit_points() -> i32 {
    crate::ai::DEFAULT_HIT_POINTS
}

fn default_energy() -> i32 {
    crate::ai::ENERGY_MAX
}

/// A damage number rising over a victim's head (`combat-measurements.md` 1.2: cream digits
/// climbing about 50 px in 1.5 s). Presentation only: kept in the snapshot so a restored world
/// draws the same picture, but neither hashed nor validated beyond its bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageNumber {
    /// Feet position of the victim when hit (map pixels).
    pub x: i32,
    /// Feet position.
    pub y: i32,
    /// Hit points taken.
    pub amount: i32,
    /// Ticks since the hit (below [`crate::ai::DAMAGE_NUMBER_TICKS`]).
    pub age: u32,
}

/// Most damage numbers kept at once (the oldest is dropped beyond it).
pub const MAX_DAMAGE_NUMBERS: usize = 256;

/// One entity as `observe` reports it: every field of [`Entity`] plus the derived `in_combat`
/// (the entity fights: `foe` is set).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityObservation {
    /// The entity.
    #[serde(flatten)]
    pub entity: Entity,
    /// In a melee (`ai_state` is `fighting`): the bars are drawn.
    pub in_combat: bool,
}

/// The last left click on the ground, remembered for double-click detection (authoritative: it
/// decides whether the next click walks or runs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroundClick {
    /// Tick the click was applied on.
    pub tick: u64,
    /// Map position of the click (24.8).
    pub x: Fixed,
    /// Map position.
    pub y: Fixed,
}

/// One instruction of an NPC waypoint program. Programs are plain data translated by the app
/// from the mission's rail programs (`docs/formats/rhm.md`, "Rail programs"); the core only
/// executes them. Everything is integer: positions in map pixels, durations in ticks, facings in
/// 1/256 turns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Instruction {
    /// Walk to a map point using pathfinding. Blocks until the entity arrives (or the walk fails,
    /// which skips the instruction after a short pause).
    GoTo {
        /// Target x in map pixels.
        x: i32,
        /// Target y in map pixels.
        y: i32,
    },
    /// Stand still for this many ticks.
    Wait {
        /// Ticks.
        ticks: u32,
    },
    /// Face an absolute direction.
    Face {
        /// Facing in 1/256 turns.
        facing256: i32,
    },
    /// Turn relative to the current facing.
    Turn {
        /// Delta in 1/256 turns (positive = clockwise on screen).
        delta256: i32,
    },
    /// Continue at another instruction.
    Jump {
        /// Target instruction index.
        pc: u32,
    },
    /// Roll a percentage (0..100) on the gameplay RNG and jump to the first arm whose cumulative
    /// percentage exceeds the roll; when no arm matches, fall through to the next instruction.
    Choose {
        /// `(percent, pc)` arms, in file order.
        arms: Vec<(u8, u32)>,
    },
    /// End of program: the entity stands where it is (idle) forever.
    Stop,
    /// A command of the original whose meaning is not established: does nothing.
    Nop {
        /// The original opcode, for inspection.
        opcode: u8,
    },
}

impl Instruction {
    /// Stable tag for canonical encodings (never derived from declaration order).
    #[must_use]
    pub fn tag(&self) -> u8 {
        match self {
            Instruction::GoTo { .. } => 1,
            Instruction::Wait { .. } => 2,
            Instruction::Face { .. } => 3,
            Instruction::Turn { .. } => 4,
            Instruction::Jump { .. } => 5,
            Instruction::Choose { .. } => 6,
            Instruction::Stop => 7,
            Instruction::Nop { .. } => 8,
        }
    }
}

/// Most instructions a program may execute in one tick before it yields (guards against
/// programs made only of jumps).
pub const PROGRAM_STEPS_PER_TICK: u32 = 32;
/// Largest number of instructions in one program.
pub const MAX_PROGRAM_LEN: usize = 1 << 16;

/// Scenario selection for `reset`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scenario {
    /// Synthetic scenario by name (`corridor`).
    Synthetic(String),
    /// A retail map background with synthetic units on it (`map`, `ambiance` = Day / Night / Fog).
    MapView {
        /// Map base name, e.g. `sherwood`.
        map: String,
        /// Ambiance directory name.
        ambiance: String,
    },
    /// Retail mission by base name.
    Mission(String),
    /// A menu screen handled by the app (`main`); the core has no world for it.
    Menu(String),
}

/// Data the app must supply for map-backed scenarios (core does no I/O).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapInfo {
    /// Map width in pixels.
    pub width: u32,
    /// Map height in pixels.
    pub height: u32,
}

/// Which side an actor is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Team {
    /// Player-controlled.
    Player,
    /// Hostile.
    Enemy,
    /// Neutral civilian.
    Civilian,
}

impl Team {
    /// Stable tag for canonical encodings (never derived from declaration order).
    #[must_use]
    pub fn tag(self) -> u8 {
        match self {
            Team::Player => 1,
            Team::Enemy => 2,
            Team::Civilian => 3,
        }
    }
}

/// One actor of a mission, as decoded by the app from the mission file (plain data; core does no I/O).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorSpec {
    /// Sprite profile name (`RobinHood`, `Soldier A00`, ...); must exist in the attached catalog to be drawn.
    pub profile: String,
    /// Team.
    pub team: Team,
    /// Start position in map pixels.
    pub x: i32,
    /// Start position.
    pub y: i32,
    /// Facing in 1/256 turns.
    pub facing256: i32,
    /// Patrol waypoints in map pixels (guards without a program walk between them; retail
    /// missions leave this empty and use `program`).
    pub patrol: Vec<(i32, i32)>,
    /// Waypoint program (empty = none: the actor stands idle unless it has a `patrol`).
    #[serde(default)]
    pub program: Vec<Instruction>,
    /// Active at start (`false` for the mission's hidden player characters, which a script
    /// activates later; `docs/formats/rhm.md`, `SCOT` placement flag `0x88`).
    #[serde(default = "default_true")]
    pub active: bool,
    /// Hit points (`profile.md`, SD `p0`; the default for actors without a profile value).
    #[serde(default = "default_hit_points")]
    pub hit_points: i32,
    /// Knock-out resistance (`profile.md`, SD `p4`; 0 for actors without a profile value).
    #[serde(default)]
    pub knockout_resistance: i32,
}

/// A mission ready to be simulated: map size, walkable geometry and actors. Built by the app from
/// the retail files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionSpec {
    /// Map size in pixels.
    pub map: MapInfo,
    /// Walkable ground and obstacles.
    pub geometry: Geometry,
    /// Actors in file order (order is authoritative).
    pub actors: Vec<ActorSpec>,
    /// The mission script translated to the core IR (`None` = no script: the mission runs
    /// without objectives, texts or scripted events).
    #[serde(default)]
    pub script: Option<Program>,
    /// Every rail of the mission as a compiled program, by `RAIL` index (script native 132
    /// assigns them; empty rails give empty programs).
    #[serde(default)]
    pub rails: Vec<Vec<Instruction>>,
    /// Unknown-native policy of the script VM: `false` (default) traps, `true` records no-ops
    /// (`opensherwood_core::natives`).
    #[serde(default)]
    pub lenient_natives: bool,
    /// The player's money when the mission starts (natives 236 / 237): campaign state the app
    /// seeds ([`DEFAULT_STARTING_MONEY`] by default), applied to the VM before `Initialize`
    /// runs so a script that sets it (H10's 100000) wins and nothing overwrites it afterwards.
    #[serde(default = "default_starting_money")]
    pub starting_money: i32,
    /// Assumptions the app recorded while building the spec (`Assumption::LenientAssets` when
    /// an actor fell back to a default profile), seeded into the VM's set at load.
    #[serde(default)]
    pub assumptions: BTreeSet<Assumption>,
}

/// The player's money at the start of a mission when the campaign supplies none.
pub const DEFAULT_STARTING_MONEY: i32 = 100;

fn default_starting_money() -> i32 {
    DEFAULT_STARTING_MONEY
}

/// Serialisable snapshot of the whole authoritative state, with the versions it was made under
/// and the identity of the content it was built from (the envelope, ADR-0004).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Snapshot schema version.
    pub version: u32,
    /// Ruleset the state was produced by.
    pub ruleset: u32,
    /// Hash schema in force.
    pub hash_schema: u32,
    /// Fingerprint of the game content the world was built from (`GameDir::fingerprint` in the
    /// assets crate); `None` for synthetic scenarios. The catalog and background are not part of
    /// the snapshot, so a restore must run on the same content: see [`Snapshot::check_content`].
    pub content: Option<String>,
    /// World state.
    pub world: World,
}

impl Snapshot {
    /// Check the envelope versions against this build: snapshot schema, ruleset and hash schema
    /// must all match exactly (a snapshot is never migrated).
    pub fn check_versions(&self) -> Result<(), String> {
        if self.version != SNAPSHOT_VERSION {
            return Err(format!(
                "snapshot version {} not supported (expected {SNAPSHOT_VERSION})",
                self.version
            ));
        }
        if self.ruleset != crate::RULESET_VERSION {
            return Err(format!(
                "snapshot ruleset {} does not match {}",
                self.ruleset,
                crate::RULESET_VERSION
            ));
        }
        if self.hash_schema != HASH_SCHEMA_VERSION {
            return Err(format!(
                "snapshot hash schema {} does not match {HASH_SCHEMA_VERSION}",
                self.hash_schema
            ));
        }
        Ok(())
    }

    /// Check content identity: `expected` is the fingerprint of the content the restoring session
    /// would rebuild the scenario from (`None` for synthetic scenarios, which need no content).
    /// The core cannot compute fingerprints (no I/O), so the app calls this before
    /// [`World::restore`].
    pub fn check_content(&self, expected: Option<&str>) -> Result<(), String> {
        match (self.content.as_deref(), expected) {
            (None, None) => Ok(()),
            (Some(a), Some(b)) if a == b => Ok(()),
            (None, Some(_)) => Err(
                "snapshot carries no content fingerprint but the scenario needs game content"
                    .into(),
            ),
            (Some(_), None) => Err(
                "snapshot carries a content fingerprint but the scenario uses no game content"
                    .into(),
            ),
            (Some(_), Some(_)) => Err("snapshot was taken with different game content".into()),
        }
    }
}

/// Filtered view for `observe`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    /// Current tick.
    pub tick: u64,
    /// Scenario.
    pub scenario: Scenario,
    /// Viewport size.
    pub viewport: (u32, u32),
    /// Map size.
    pub map_size: (u32, u32),
    /// Camera offset in map pixels (top-left of the viewport).
    pub camera: (i32, i32),
    /// Pointer position in viewport coordinates (24.8).
    pub pointer: (i32, i32),
    /// Selected entity.
    pub selected: Option<EntityId>,
    /// Entities (empty when the caller asked to omit them).
    pub entities: Vec<EntityObservation>,
    /// RNG draws so far.
    pub rng_draws: u64,
    /// Objective state for the synthetic scenario.
    pub objective_reached: bool,
    /// A player character died (`World::hero_dead`): the app shows the lost page.
    #[serde(default)]
    pub hero_dead: bool,
    /// Script state (objectives, pending texts, victory, unknown natives), if the world runs a
    /// script.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<ScriptObservation>,
}

/// Scroll speed in pixels per tick for keyboard and edge scrolling.
pub const SCROLL_SPEED: i32 = 8;
/// Edge-scroll margin in logical pixels.
pub const EDGE_MARGIN: i32 = 6;
/// Largest map dimension accepted.
pub const MAX_MAP_SIZE: u32 = 1 << 15;
/// Largest viewport dimension accepted (must fit the renderer's framebuffer budget).
pub const MAX_VIEWPORT: u32 = 4096;
/// Largest number of entities accepted in a snapshot.
pub const MAX_ENTITIES: usize = 1 << 16;
/// Pointer coordinates (24.8) are clamped to this magnitude.
pub const MAX_POINTER_RAW: i32 = 1 << 24;
/// Largest total vertex count of the walkable geometry.
pub const MAX_GEOMETRY_VERTICES: usize = 1 << 20;
/// Largest magnitude of a geometry vertex coordinate; see [`crate::geom::MAX_COORD`].
pub const MAX_GEOMETRY_COORD: i32 = crate::geom::MAX_COORD;
/// Largest magnitude of an entity position (map pixels); positions are clamped to the map when
/// entities move, snapshots may not exceed the geometry range.
pub const MAX_ENTITY_COORD: i32 = crate::geom::MAX_COORD;
/// RNG stream id of the gameplay stream (the script stream is [`SCRIPT_RNG_STREAM`]).
pub const GAMEPLAY_RNG_STREAM: u64 = 1;
/// Work budget of one movement order issued by the player (A* node expansions and smoothing
/// cells, `nav.rs`), and the cap of one search issued by the simulation (an NPC's waypoint
/// program, the stealth layer), which otherwise draws from the tick's [`SIM_WORK_PER_TICK`];
/// orders issued by the script are charged to the VM's per-tick budget instead.
pub const ORDER_SEARCH_WORK: u64 = DEFAULT_SEARCH_WORK;
/// A second left click on the ground within this many ticks of the first (20 at 60 Hz, a third
/// of a second) ...
pub const DOUBLE_CLICK_TICKS: u64 = 20;
/// ... and within this many map pixels of it is a double click: the order becomes a run
/// (`docs/original/ui-flow.md` 9.4).
pub const DOUBLE_CLICK_DISTANCE: i32 = 8;
/// Running speed over the walking speed, as a ratio, for an entity whose profile has no run
/// cycle to read it from (synthetic units): the hero table's run over walk (60 px per 12 frames
/// over 88 px per 22 frames = 5 / 4; measured 1.2, `docs/original/stealth-and-combat.md` 8.3).
pub const FALLBACK_RUN_SPEED_RATIO: (i32, i32) = (5, 4);
/// Crouched (sneaking) speed over the walking speed for an entity without a sneak cycle: the
/// hero table's sneak (27 px per 32 table ticks) over its walk (88 px per 22) = 27 / 128 = 0.21
/// (measured 0.21, `docs/original/stealth-and-combat.md` 8.2).
pub const FALLBACK_SNEAK_SPEED_RATIO: (i32, i32) = (27, 128);
/// Walking speed of a synthetic player character in map pixels per world tick (24.8): the
/// hero's measured 85.3 px/s (`docs/original/stealth-and-combat.md` 8.1), 364 / 256 = 1.42 px
/// per tick, the value the hero's walk cycle gives ([`crate::anim::AnimSet::cycle_speed`]).
pub const SYNTHETIC_PLAYER_SPEED: Fixed = Fixed::from_raw(364);
/// Walking speed of a synthetic guard: the soldier's 42.7 px/s derived from the same clock
/// (182 / 256 = 0.71 px per tick).
pub const SYNTHETIC_GUARD_SPEED: Fixed = Fixed::from_raw(182);
/// Largest state timer a snapshot may carry (`Entity::state_ticks`).
pub const MAX_STATE_TICKS: u32 = 1 << 24;

/// Every check a walkable geometry must pass before it may drive movement: vertex budget and
/// coordinate range (`-MAX_GEOMETRY_COORD..=MAX_GEOMETRY_COORD`).
fn check_geometry(geometry: &Geometry, map_size: (u32, u32)) -> Result<(), String> {
    if geometry.vertex_count() > MAX_GEOMETRY_VERTICES {
        return Err("geometry has too many vertices".into());
    }
    geometry
        .check_bounds()
        .map_err(|e| format!("geometry {e}"))?;
    // The navigation grid must be buildable within its work budget before the geometry is accepted.
    NavGrid::check_budget(geometry, map_size.0, map_size.1)
        .map(|_| ())
        .map_err(|e| format!("geometry navigation budget: {e}"))
}

/// The world.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct World {
    /// Scenario.
    pub scenario: Scenario,
    /// Seed used at reset.
    pub seed: u64,
    /// Current tick.
    pub tick: u64,
    /// Logical viewport.
    pub viewport: (u32, u32),
    /// Map size in pixels (viewport size for synthetic scenarios).
    pub map_size: (u32, u32),
    /// Camera offset in map pixels.
    pub camera: (i32, i32),
    /// Pointer position in viewport coordinates, 24.8.
    pub pointer: (i32, i32),
    /// Whether a pointer position has been received since reset (edge scrolling needs a real pointer).
    pub pointer_seen: bool,
    /// Buttons currently held (a set; order carries no meaning).
    pub buttons_down: BTreeSet<Button>,
    /// Keys currently held (a set).
    pub keys_down: BTreeSet<Key>,
    /// Selected entity.
    pub selected: Option<EntityId>,
    /// The last left click that ordered a walk, for double-click detection (`None` once a double
    /// click was consumed or a click did something else).
    #[serde(default)]
    pub last_ground_click: Option<GroundClick>,
    /// Entities by slot (order is authoritative: it is the simulation and draw order).
    pub entities: Vec<Entity>,
    /// Gameplay RNG stream.
    pub rng: Rng,
    /// Goal position for the synthetic objective (map pixels).
    pub goal: (Fixed, Fixed),
    /// Whether the player reached the goal.
    pub objective_reached: bool,
    /// Walkable geometry (authoritative: it decides movement).
    pub geometry: Geometry,
    /// Waypoint programs referenced by `Entity::program` (authoritative; deduplicated at load,
    /// in first-use order).
    #[serde(default)]
    pub programs: Vec<Vec<Instruction>>,
    /// The mission script VM (ADR-0008): program, class variables, scheduler queues, sequences;
    /// `None` for worlds without a script.
    #[serde(default)]
    pub vm: Option<VmState>,
    /// Where each phase of the simulation resumes when the tick's budget ran out (`ai.rs`,
    /// "Work": round robin per phase; 0 after a completed walk).
    #[serde(default)]
    pub cursors: SimCursors,
    /// Where the left button went down on the map (24.8), until it comes up: the release
    /// decides between a click and a drawn figure (`combat-measurements.md` 1.4).
    #[serde(default)]
    pub press: Option<(Fixed, Fixed)>,
    /// A player character died (`ai::World::kill`): the mission is lost (measured for a lone
    /// hero, `combat-measurements.md` 4); sticky, read by the app for the lost page.
    #[serde(default)]
    pub hero_dead: bool,
    /// Damage numbers in flight, oldest first (presentation: not hashed).
    #[serde(default)]
    pub damage_numbers: Vec<DamageNumber>,
    /// Navigation grid derived from `geometry` and `map_size`: not serialised, built by every
    /// constructor, `set_geometry` and `restore` before the world is committed (a world the core
    /// hands out always has it; movement orders are refused, never unbounded, without it).
    #[serde(skip)]
    pub nav: Option<NavGrid>,
    /// Static animation data attached by the app (not part of the snapshot; re-attached on load).
    #[serde(skip)]
    pub catalog: Catalog,
}

/// The entity index each simulation phase resumes from when the tick's budget ran out
/// (`ai.rs`, "Work"): authoritative (snapshot, `validate`: every cursor names an entity or is
/// 0, the `world` hash).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimCursors {
    /// The perception walk over the soldiers.
    pub perception: u32,
    /// The state transitions over the humans.
    pub states: u32,
    /// The attack orders over the attacking player characters.
    pub attacks: u32,
    /// The waypoint programs and legacy patrols over the idle guards.
    pub programs: u32,
}

impl SimCursors {
    fn all(self) -> [u32; 4] {
        [self.perception, self.states, self.attacks, self.programs]
    }
}

/// Work units the simulation may spend in one tick besides the script (`ai.rs`, "Work"): the
/// pre-index pass, perception, the state transitions, the attack orders, the waypoint programs
/// and every path search any of them issues (the units of `nav.rs`, capped per search at
/// [`ORDER_SEARCH_WORK`]), granted at the start of `simulate` and nowhere else. 2^24: a
/// `MAX_ENTITIES` world of idle soldiers costs 2^18 a tick before any search, and a search over
/// the largest accepted grid (`nav::MAX_CELLS`) fits three times.
pub const SIM_WORK_PER_TICK: u64 = 1 << 24;

/// Snapshot schema version (9: sequence barrier tokens, VM counters and budget no longer
/// serialised, snapshots must be quiescent; 11: entity `gait` / `posture`, the world's
/// `last_ground_click`; 12: the stealth layer: entity `team`, `ai_state`, `state_ticks`,
/// `last_seen`, `alert_origin`, `attack_target`, `action`, `hit_points`,
/// `knockout_resistance`, `npc_gait`, `fell_backward`; actor specs carry `hit_points` and
/// `knockout_resistance`; 13: the VM's `assumptions` and `pending_action_changes`, the
/// world's `ai_cursor`, mission specs carry `starting_money` and `assumptions`; 14: entity
/// `heard`, animation `elapsed` in clock units of the measured animation clock; 15: the
/// world's `cursors` (one per simulation phase) replace `ai_cursor`, the VM's `fault`
/// replaces `faulted`, native calls carry their result slot, frames no longer hold a native
/// result, the assumption registry grew; 16: the melee: entity `hp` / `hp_max` (replacing
/// `hit_points`) / `energy` / `energy_ticks` / `foe` / `pose` / `pose_ticks` / `swing_ticks`
/// / `figure`, the world's `press`, `hero_dead` and `damage_numbers`, the assumption
/// registry grew).
pub const SNAPSHOT_VERSION: u32 = 16;

impl World {
    /// Create a world for a scenario that needs no external data.
    pub fn new(scenario: Scenario, seed: u64) -> Result<Self, String> {
        match scenario {
            Scenario::Synthetic(ref name) if name == "corridor" => {
                Self::build(scenario, seed, None)
            }
            Scenario::Synthetic(name) => Err(format!("unknown synthetic scenario '{name}'")),
            Scenario::MapView { .. } => {
                Err("map view scenarios need MapInfo (World::new_map_view)".into())
            }
            Scenario::Mission(name) => {
                Err(format!("mission '{name}' needs the app's mission loader"))
            }
            Scenario::Menu(name) => Err(format!("menu '{name}' has no world")),
        }
    }

    /// Create a mission world from a decoded mission spec. Actors become player / guard entities
    /// with a walking speed and selection radius matching the synthetic ones until the real
    /// movement rules are specified.
    pub fn new_mission(scenario: Scenario, seed: u64, spec: &MissionSpec) -> Result<Self, String> {
        if !matches!(scenario, Scenario::Mission(_)) {
            return Err("not a mission scenario".into());
        }
        if spec.map.width == 0
            || spec.map.height == 0
            || spec.map.width > MAX_MAP_SIZE
            || spec.map.height > MAX_MAP_SIZE
        {
            return Err(format!(
                "map size {}x{} out of range",
                spec.map.width, spec.map.height
            ));
        }
        if spec.actors.len() > MAX_ENTITIES {
            return Err(format!("{} actors exceed the limit", spec.actors.len()));
        }
        let mut world = Self::build(scenario, seed, Some(spec.map))?;
        world.set_geometry(spec.geometry.clone())?;
        world.entities.clear();
        world.goal = (Fixed::from_int(-1000), Fixed::from_int(-1000));
        let f = Fixed::from_int;
        for (i, a) in spec.actors.iter().enumerate() {
            let kind = match a.team {
                Team::Player => EntityKind::Player,
                Team::Enemy | Team::Civilian => EntityKind::Guard,
            };
            // Programs are shared: identical rails give one entry, in first-use order.
            let program = if kind == EntityKind::Guard && !a.program.is_empty() {
                if a.program.len() > MAX_PROGRAM_LEN {
                    return Err(format!("actor {i} program too long"));
                }
                let idx = world
                    .programs
                    .iter()
                    .position(|p| *p == a.program)
                    .unwrap_or_else(|| {
                        world.programs.push(a.program.clone());
                        world.programs.len() - 1
                    });
                Some(idx as u32)
            } else {
                None
            };
            world.entities.push(Entity {
                id: EntityId {
                    index: i as u32,
                    generation: 1,
                },
                kind,
                x: f(a.x),
                y: f(a.y),
                size: f(12),
                speed: if kind == EntityKind::Player {
                    SYNTHETIC_PLAYER_SPEED
                } else {
                    SYNTHETIC_GUARD_SPEED
                },
                target: None,
                path: Vec::new(),
                patrol: a.patrol.iter().map(|&(x, y)| (f(x), f(y))).collect(),
                patrol_index: 0,
                wait_ticks: 0,
                facing256: a.facing256.rem_euclid(256),
                alive: true,
                anim: Some(AnimState::new(a.profile.clone(), 0)),
                program,
                pc: 0,
                active: a.active,
                ai_locked: false,
                gait: Gait::Walk,
                posture: Posture::Standing,
                team: a.team,
                ai_state: AiState::Patrol,
                state_ticks: 0,
                last_seen: None,
                alert_origin: None,
                attack_target: None,
                action: 0,
                hp: a.hit_points,
                hp_max: a.hit_points,
                energy: ENERGY_MAX,
                energy_ticks: 0,
                foe: None,
                pose: FightPose::Idle,
                pose_ticks: 0,
                swing_ticks: 0,
                figure: None,
                knockout_resistance: a.knockout_resistance,
                npc_gait: Gait::Walk,
                fell_backward: false,
                heard: false,
            });
        }
        // The original opens a mission with the camera on the hero.
        if let Some(hero) = world.entities.iter().find(|e| e.kind == EntityKind::Player) {
            let (cx, cy) = (hero.x.round(), hero.y.round());
            world.center_camera_on(cx, cy);
        }
        // Every rail becomes a program the script can assign (native 132), shared like the
        // actors' own programs.
        let mut paths = Vec::with_capacity(spec.rails.len());
        for (i, rail) in spec.rails.iter().enumerate() {
            if rail.is_empty() {
                paths.push(None);
                continue;
            }
            if rail.len() > MAX_PROGRAM_LEN {
                return Err(format!("rail {i} program too long"));
            }
            let idx = world
                .programs
                .iter()
                .position(|p| p == rail)
                .unwrap_or_else(|| {
                    world.programs.push(rail.clone());
                    world.programs.len() - 1
                });
            paths.push(Some(idx as u32));
        }
        world.validate()?;
        if let Some(program) = &spec.script {
            world.attach_script(
                program.clone(),
                paths,
                spec.lenient_natives,
                spec.starting_money,
                &spec.assumptions,
            )?;
            world.validate()?;
        }
        Ok(world)
    }

    /// Centre the camera on a map point, clamped to the map. Any `i32` point is accepted: the
    /// arithmetic is done in `i64`, so a hostile position lands at a map edge instead of wrapping.
    pub fn center_camera_on(&mut self, x: i32, y: i32) {
        let max_x = (i64::from(self.map_size.0) - i64::from(self.viewport.0)).max(0);
        let max_y = (i64::from(self.map_size.1) - i64::from(self.viewport.1)).max(0);
        // Both bounds are below `MAX_MAP_SIZE`, so the clamped values fit `i32`.
        self.camera = (
            (i64::from(x) - i64::from(self.viewport.0 / 2)).clamp(0, max_x) as i32,
            (i64::from(y) - i64::from(self.viewport.1 / 2)).clamp(0, max_y) as i32,
        );
    }

    /// Attach walkable geometry (map view and missions) and rebuild the navigation grid. The
    /// grid is built first, into a temporary; geometry over the vertex budget, outside
    /// `+-MAX_GEOMETRY_COORD`, over the navigation budgets or whose grid cannot be allocated is
    /// refused and the world is left unchanged.
    pub fn set_geometry(&mut self, geometry: Geometry) -> Result<(), String> {
        check_geometry(&geometry, self.map_size)?;
        let nav = NavGrid::try_build(&geometry, self.map_size.0, self.map_size.1)
            .map_err(|e| format!("navigation grid: {e}"))?;
        self.geometry = geometry;
        self.nav = Some(nav);
        Ok(())
    }

    /// Build the navigation grid if it is missing (every constructor, `set_geometry` and
    /// `restore` build it, so this only acts on a world whose `nav` was cleared by hand). A
    /// failed build leaves `nav` empty and is reported; there is no degraded grid and no
    /// infallible wrapper: every caller handles the error.
    pub fn try_ensure_nav(&mut self) -> Result<(), NavError> {
        if self.nav.is_none() {
            self.nav = Some(NavGrid::try_build(
                &self.geometry,
                self.map_size.0,
                self.map_size.1,
            )?);
        }
        Ok(())
    }

    /// Plan a path for entity `index` to `target` (map pixels) within [`ORDER_SEARCH_WORK`]
    /// (the budget of a player's order). Targets on unwalkable ground are moved to the
    /// nearest walkable cell; unreachable targets and an exhausted budget clear the order.
    pub(crate) fn plan_path(&mut self, index: usize, target: (Fixed, Fixed)) {
        let mut budget = ORDER_SEARCH_WORK;
        let _ = self.plan_path_with(index, target, &mut budget);
    }

    /// [`World::plan_path`] charging the search (initialisation, expansions, unwinding), the
    /// smoothing (line-clear cells, output points) and the conversion of the final path (one unit
    /// per point) to `budget`; an exhausted budget (or a refused allocation) clears the order and
    /// is returned. Without a navigation grid the order is refused (`Ok`, no target).
    pub(crate) fn plan_path_with(
        &mut self,
        index: usize,
        target: (Fixed, Fixed),
        budget: &mut u64,
    ) -> Result<(), NavError> {
        let Some(nav) = self.nav.as_ref() else {
            self.entities[index].target = None;
            self.entities[index].path.clear();
            return Ok(());
        };
        let e = &self.entities[index];
        let from = nav.cell_of(e.x.round(), e.y.round());
        let from = nav.nearest_walkable(from, 8).unwrap_or(from);
        let goal = nav.cell_of(target.0.round(), target.1.round());
        // Like the original, an order on water or behind a wall walks to the reachable cell
        // closest to it.
        let planned = nav
            .find_path_with(from, goal, true, budget)
            .and_then(|cells| match cells {
                Some(cells) => nav
                    .smooth_with(from, &cells, budget)
                    .map(|s| Some((cells, s))),
                None => Ok(None),
            });
        let Ok(Some((cells, smooth))) = planned else {
            self.entities[index].target = None;
            self.entities[index].path.clear();
            return planned.map(|_| ());
        };
        // The conversion to map coordinates is charged like the unwinding (one unit per point,
        // one more for the exact target) and the final path is allocated fallibly, both before
        // any point is produced.
        let points = smooth.len().saturating_add(1);
        let mut path: Vec<(Fixed, Fixed)> = Vec::new();
        let converted = if *budget < points as u64 {
            *budget = 0;
            Err(NavError::WorkExhausted)
        } else {
            *budget -= points as u64;
            path.try_reserve_exact(points)
                .map_err(|_| NavError::Allocation { cells: points })
        };
        if let Err(e) = converted {
            self.entities[index].target = None;
            self.entities[index].path.clear();
            return Err(e);
        }
        path.extend(smooth.iter().map(|&c| {
            let (x, y) = nav.centre(c);
            (Fixed::from_int(x), Fixed::from_int(y))
        }));
        // Walk to the exact target when it is itself walkable and was reached by the path.
        if self
            .geometry
            .is_walkable(target.0.round(), target.1.round())
            && cells.last() == Some(&goal)
        {
            if let Some(last) = path.last_mut() {
                *last = target;
            } else {
                path.push(target);
            }
        }
        let e = &mut self.entities[index];
        e.target = Some(target);
        e.path = path;
        // Every order walks; the player's double click upgrades its own order afterwards.
        e.gait = Gait::Walk;
        Ok(())
    }

    /// Create a map-view world; the app resolved and decoded the background already.
    pub fn new_map_view(scenario: Scenario, seed: u64, info: MapInfo) -> Result<Self, String> {
        if info.width == 0
            || info.height == 0
            || info.width > MAX_MAP_SIZE
            || info.height > MAX_MAP_SIZE
        {
            return Err(format!(
                "map size {}x{} out of range",
                info.width, info.height
            ));
        }
        match scenario {
            Scenario::MapView { .. } => Self::build(scenario, seed, Some(info)),
            _ => Err("not a map view scenario".into()),
        }
    }

    /// The common part of every constructor; builds the navigation grid of the (empty) geometry
    /// so that a world always carries one.
    fn build(scenario: Scenario, seed: u64, map: Option<MapInfo>) -> Result<Self, String> {
        let f = Fixed::from_int;
        // Synthetic scenarios keep the small viewport of the determinism fixtures; retail maps and
        // missions use the original's 1024x768 frame (`docs/original/ui-flow.md`).
        let viewport = if map.is_some() {
            (1024u32, 768u32)
        } else {
            (640u32, 480u32)
        };
        let map_size = map.map_or(viewport, |m| (m.width, m.height));
        let mut entities = Vec::new();
        let id = |index: u32| EntityId {
            index,
            generation: 1,
        };
        entities.push(Entity {
            id: id(0),
            kind: EntityKind::Player,
            x: f(80),
            y: f(240),
            size: f(12),
            speed: SYNTHETIC_PLAYER_SPEED,
            target: None,
            path: Vec::new(),
            patrol: Vec::new(),
            patrol_index: 0,
            wait_ticks: 0,
            facing256: 0,
            alive: true,
            anim: None,
            program: None,
            pc: 0,
            active: true,
            ai_locked: false,
            gait: Gait::Walk,
            posture: Posture::Standing,
            team: Team::Player,
            ai_state: AiState::Patrol,
            state_ticks: 0,
            last_seen: None,
            alert_origin: None,
            attack_target: None,
            action: 0,
            hp: crate::ai::DEFAULT_HIT_POINTS,
            hp_max: crate::ai::DEFAULT_HIT_POINTS,
            energy: ENERGY_MAX,
            energy_ticks: 0,
            foe: None,
            pose: FightPose::Idle,
            pose_ticks: 0,
            swing_ticks: 0,
            figure: None,
            knockout_resistance: 0,
            npc_gait: Gait::Walk,
            fell_backward: false,
            heard: false,
        });
        entities.push(Entity {
            id: id(1),
            kind: EntityKind::Guard,
            x: f(400),
            y: f(120),
            size: f(12),
            speed: SYNTHETIC_GUARD_SPEED,
            target: None,
            path: Vec::new(),
            patrol: vec![(f(400), f(120)), (f(400), f(360))],
            patrol_index: 1,
            wait_ticks: 0,
            facing256: 64,
            alive: true,
            anim: None,
            program: None,
            pc: 0,
            active: true,
            ai_locked: false,
            gait: Gait::Walk,
            posture: Posture::Standing,
            team: Team::Enemy,
            ai_state: AiState::Patrol,
            state_ticks: 0,
            last_seen: None,
            alert_origin: None,
            attack_target: None,
            action: 0,
            hp: crate::ai::DEFAULT_HIT_POINTS,
            hp_max: crate::ai::DEFAULT_HIT_POINTS,
            energy: ENERGY_MAX,
            energy_ticks: 0,
            foe: None,
            pose: FightPose::Idle,
            pose_ticks: 0,
            swing_ticks: 0,
            figure: None,
            knockout_resistance: 0,
            npc_gait: Gait::Walk,
            fell_backward: false,
            heard: false,
        });
        let obstacles: &[(i32, i32, i32, i32)] = if map.is_some() {
            &[]
        } else {
            &[(320, 60, 20, 100), (320, 420, 20, 100), (520, 400, 20, 60)]
        };
        for (i, &(x, y, w, h)) in obstacles.iter().enumerate() {
            entities.push(Entity {
                id: id(2 + i as u32),
                kind: EntityKind::Obstacle,
                x: f(x),
                y: f(y),
                size: f(w.max(h)),
                speed: Fixed::ZERO,
                target: None,
                path: Vec::new(),
                patrol: vec![(f(w), f(h))],
                patrol_index: 0,
                wait_ticks: 0,
                facing256: 0,
                alive: true,
                anim: None,
                program: None,
                pc: 0,
                active: true,
                ai_locked: false,
                gait: Gait::Walk,
                posture: Posture::Standing,
                team: Team::Civilian,
                ai_state: AiState::Patrol,
                state_ticks: 0,
                last_seen: None,
                alert_origin: None,
                attack_target: None,
                action: 0,
                hp: 0,
                hp_max: 0,
                energy: 0,
                energy_ticks: 0,
                foe: None,
                pose: FightPose::Idle,
                pose_ticks: 0,
                swing_ticks: 0,
                figure: None,
                knockout_resistance: 0,
                npc_gait: Gait::Walk,
                fell_backward: false,
                heard: false,
            });
        }
        let geometry = Geometry::default();
        let nav = NavGrid::try_build(&geometry, map_size.0, map_size.1)
            .map_err(|e| format!("navigation grid: {e}"))?;
        Ok(World {
            scenario,
            seed,
            tick: 0,
            viewport,
            map_size,
            camera: (0, 0),
            pointer: (0, 0),
            pointer_seen: false,
            buttons_down: BTreeSet::new(),
            keys_down: BTreeSet::new(),
            selected: None,
            last_ground_click: None,
            entities,
            rng: Rng::new(seed, GAMEPLAY_RNG_STREAM),
            goal: (f(600), f(240)),
            objective_reached: false,
            geometry,
            programs: Vec::new(),
            vm: None,
            cursors: SimCursors::default(),
            press: None,
            hero_dead: false,
            damage_numbers: Vec::new(),
            nav: Some(nav),
            catalog: Catalog::default(),
        })
    }

    /// Attach animation data and give every player / guard the named set (idle, facing direction).
    pub fn attach_catalog(
        &mut self,
        catalog: Catalog,
        player_set: Option<&str>,
        guard_set: Option<&str>,
    ) {
        self.catalog = catalog;
        for e in &mut self.entities {
            let default = match e.kind {
                EntityKind::Player => player_set,
                EntityKind::Guard => guard_set,
                EntityKind::Obstacle => None,
            };
            // A mission actor already names its profile; synthetic units get the defaults.
            let name = e
                .anim
                .as_ref()
                .map(|a| a.set.clone())
                .or_else(|| default.map(str::to_string));
            e.anim = name.and_then(|name| {
                let s = self.catalog.sets.get(&name)?;
                Some(AnimState::new(name, s.idle[direction_of(e.facing256)]))
            });
        }
    }

    /// Check every invariant a snapshot must satisfy before it may become the world, using the
    /// attached catalog for the animation checks (see [`World::validate_with`]).
    pub fn validate(&self) -> Result<(), String> {
        self.validate_with(&self.catalog)
    }

    /// [`World::validate`] against an explicit catalog: a deserialised snapshot carries none, so
    /// restore checks it against the catalog of the session. When `catalog` has any set, every
    /// animation state must name an existing profile with its animation and frame indices in
    /// range and `elapsed` below the frame duration; nothing falls back silently. Without a
    /// catalog (synthetic worlds, no sprite bank) only the size bounds apply.
    pub fn validate_with(&self, catalog: &Catalog) -> Result<(), String> {
        if self.viewport.0 == 0
            || self.viewport.1 == 0
            || self.viewport.0 > MAX_VIEWPORT
            || self.viewport.1 > MAX_VIEWPORT
        {
            return Err(format!("viewport {:?} out of range", self.viewport));
        }
        if self.map_size.0 == 0
            || self.map_size.1 == 0
            || self.map_size.0 > MAX_MAP_SIZE
            || self.map_size.1 > MAX_MAP_SIZE
        {
            return Err(format!("map size {:?} out of range", self.map_size));
        }
        let max_x = (self.map_size.0 as i32 - self.viewport.0 as i32).max(0);
        let max_y = (self.map_size.1 as i32 - self.viewport.1 as i32).max(0);
        if !(0..=max_x).contains(&self.camera.0) || !(0..=max_y).contains(&self.camera.1) {
            return Err(format!("camera {:?} outside the map", self.camera));
        }
        if self.pointer.0.unsigned_abs() > MAX_POINTER_RAW as u32
            || self.pointer.1.unsigned_abs() > MAX_POINTER_RAW as u32
        {
            return Err(format!("pointer {:?} out of range", self.pointer));
        }
        if self.entities.len() > MAX_ENTITIES {
            return Err(format!("{} entities exceed the limit", self.entities.len()));
        }
        check_geometry(&self.geometry, self.map_size)?;
        if self.programs.len() > MAX_ENTITIES {
            return Err(format!("{} programs exceed the limit", self.programs.len()));
        }
        for (i, p) in self.programs.iter().enumerate() {
            if p.len() > MAX_PROGRAM_LEN {
                return Err(format!("program {i} too long"));
            }
            let in_range = |pc: u32| (pc as usize) < p.len();
            for (j, ins) in p.iter().enumerate() {
                let ok = match ins {
                    Instruction::Jump { pc } => in_range(*pc),
                    Instruction::Choose { arms } => {
                        arms.len() <= 256 && arms.iter().all(|&(_, pc)| in_range(pc))
                    }
                    Instruction::Face { facing256 } => (0..256).contains(facing256),
                    Instruction::Turn { delta256 } => delta256.unsigned_abs() < 256,
                    Instruction::GoTo { x, y } => {
                        x.unsigned_abs() <= MAX_MAP_SIZE && y.unsigned_abs() <= MAX_MAP_SIZE
                    }
                    Instruction::Wait { .. } | Instruction::Stop | Instruction::Nop { .. } => true,
                };
                if !ok {
                    return Err(format!("program {i} instruction {j} out of range"));
                }
            }
        }
        let mut ids = BTreeSet::new();
        for e in &self.entities {
            if !ids.insert(e.id) {
                return Err(format!("duplicate entity id {:?}", e.id));
            }
            if !(0..256).contains(&e.facing256) {
                return Err(format!(
                    "entity {:?} facing {} out of range",
                    e.id, e.facing256
                ));
            }
            match e.kind {
                EntityKind::Obstacle => {
                    if e.patrol.len() != 1 {
                        return Err(format!(
                            "obstacle {:?} needs exactly one extent entry",
                            e.id
                        ));
                    }
                }
                EntityKind::Player | EntityKind::Guard => {
                    if e.patrol.len() > MAX_ENTITIES {
                        return Err(format!("entity {:?} has too many waypoints", e.id));
                    }
                    if !e.patrol.is_empty() && e.patrol_index as usize >= e.patrol.len() {
                        return Err(format!("entity {:?} patrol index out of range", e.id));
                    }
                }
            }
            if let Some(p) = e.program {
                let Some(program) = self.programs.get(p as usize) else {
                    return Err(format!("entity {:?} program {p} does not exist", e.id));
                };
                if e.pc as usize > program.len() {
                    return Err(format!("entity {:?} pc out of range", e.id));
                }
            }
            if e.path.len() > MAX_ENTITIES {
                return Err(format!("entity {:?} has too many path points", e.id));
            }
            if let Some(a) = &e.anim {
                if a.set.len() > 256 || a.animation > 1 << 20 || a.frame > 1 << 20 {
                    return Err(format!(
                        "entity {:?} has an out-of-range animation state",
                        e.id
                    ));
                }
                if !catalog.sets.is_empty() {
                    let Some(set) = catalog.sets.get(&a.set) else {
                        return Err(format!(
                            "entity {:?} animation profile '{}' is not in the catalog",
                            e.id, a.set
                        ));
                    };
                    let Some(frames) = set.animations.get(a.animation as usize) else {
                        return Err(format!(
                            "entity {:?} animation {} does not exist in profile '{}'",
                            e.id, a.animation, a.set
                        ));
                    };
                    // An empty animation holds frame 0 (nothing advances it).
                    if a.frame as usize >= frames.len().max(1) {
                        return Err(format!(
                            "entity {:?} frame {} out of range for animation {} of '{}'",
                            e.id, a.frame, a.animation, a.set
                        ));
                    }
                    let units = frames
                        .get(a.frame as usize)
                        .map_or(1, |f| f.duration.max(1))
                        .saturating_mul(UNITS_PER_TABLE_TICK);
                    if a.elapsed >= units {
                        return Err(format!(
                            "entity {:?} animation elapsed {} exceeds the frame's {units} clock units",
                            e.id, a.elapsed
                        ));
                    }
                }
            }
            if e.speed < Fixed::ZERO || e.size < Fixed::ZERO {
                return Err(format!("entity {:?} has a negative speed or size", e.id));
            }
            let bound = Fixed::from_int(MAX_ENTITY_COORD);
            if e.x.abs() > bound || e.y.abs() > bound {
                return Err(format!("entity {:?} position out of range", e.id));
            }
            if (e.kind == EntityKind::Player) != (e.team == Team::Player) {
                return Err(format!(
                    "entity {:?} kind {:?} does not match team {:?}",
                    e.id, e.kind, e.team
                ));
            }
            if e.state_ticks > MAX_STATE_TICKS {
                return Err(format!("entity {:?} state ticks out of range", e.id));
            }
            for (name, p) in [("last seen", e.last_seen), ("alert origin", e.alert_origin)] {
                if let Some((x, y)) = p
                    && (x.abs() > bound || y.abs() > bound)
                {
                    return Err(format!("entity {:?} {name} position out of range", e.id));
                }
            }
            // The stealth layer's invariants (`ai.rs`): dead is one state, a timed state
            // carries its timer and an untimed one none, the alert states belong to enemy
            // soldiers and the blow to player characters, a returning soldier knows where to,
            // a patrolling one remembers nothing.
            if e.ai_state.dead() == e.alive {
                return Err(format!(
                    "entity {:?} dead state {:?} disagrees with alive = {}",
                    e.id, e.ai_state, e.alive
                ));
            }
            // The melee's invariants (`ai.rs`): hit points within the maximum and 0 exactly
            // when dead, energy within its bar with its regain timer running exactly while
            // below full, a foe exactly in the fighting state (an opponent of the other side),
            // a timed pose exactly outside the idle one and only in a fight, the swing timer
            // only in a fight, a figure only on a player character who attacks or fights.
            if e.kind != EntityKind::Obstacle {
                if e.hp < 0 || e.hp > e.hp_max {
                    return Err(format!(
                        "entity {:?} hit points {} outside 0..={}",
                        e.id, e.hp, e.hp_max
                    ));
                }
                if (e.hp > 0) != e.alive {
                    return Err(format!(
                        "entity {:?} hit points {} disagree with alive = {}",
                        e.id, e.hp, e.alive
                    ));
                }
                if !(0..=ENERGY_MAX).contains(&e.energy) {
                    return Err(format!(
                        "entity {:?} energy {} out of range",
                        e.id, e.energy
                    ));
                }
                if (e.energy < ENERGY_MAX) != (e.energy_ticks > 0)
                    || e.energy_ticks > MAX_STATE_TICKS
                {
                    return Err(format!(
                        "entity {:?} energy timer {} inconsistent with energy {}",
                        e.id, e.energy_ticks, e.energy
                    ));
                }
            }
            if (e.ai_state == AiState::Fighting) != e.foe.is_some() {
                return Err(format!(
                    "entity {:?} in the {:?} state with foe {:?}",
                    e.id, e.ai_state, e.foe
                ));
            }
            if (e.pose != FightPose::Idle) != (e.pose_ticks > 0)
                || e.pose_ticks > MAX_STATE_TICKS
                || (e.pose != FightPose::Idle && e.ai_state != AiState::Fighting)
            {
                return Err(format!(
                    "entity {:?} pose {:?} with {} pose ticks in the {:?} state",
                    e.id, e.pose, e.pose_ticks, e.ai_state
                ));
            }
            if e.swing_ticks > MAX_STATE_TICKS
                || (e.swing_ticks > 0 && e.ai_state != AiState::Fighting)
            {
                return Err(format!(
                    "entity {:?} swing timer {} outside a fight",
                    e.id, e.swing_ticks
                ));
            }
            if e.figure.is_some()
                && !(e.kind == EntityKind::Player
                    && (e.attack_target.is_some() || e.ai_state == AiState::Fighting))
            {
                return Err(format!(
                    "entity {:?} holds a figure without an attack order or a fight",
                    e.id
                ));
            }
            if e.ai_state == AiState::Fighting
                && !(e.kind == EntityKind::Player
                    || (e.kind == EntityKind::Guard && e.team == Team::Enemy))
            {
                return Err(format!(
                    "entity {:?} fights but is neither a player character nor an enemy soldier",
                    e.id
                ));
            }
            if e.ai_state.timed() != (e.state_ticks > 0) {
                return Err(format!(
                    "entity {:?} state ticks {} inconsistent with the {} state {:?}",
                    e.id,
                    e.state_ticks,
                    if e.ai_state.timed() {
                        "timed"
                    } else {
                        "untimed"
                    },
                    e.ai_state
                ));
            }
            if e.ai_state.alert() && !(e.kind == EntityKind::Guard && e.team == Team::Enemy) {
                return Err(format!(
                    "entity {:?} in the alert state {:?} is not an enemy soldier",
                    e.id, e.ai_state
                ));
            }
            if e.heard && !e.ai_state.alert() {
                return Err(format!(
                    "entity {:?} was heard but is in the {:?} state, not an alert state",
                    e.id, e.ai_state
                ));
            }
            if e.ai_state == AiState::Punching && e.kind != EntityKind::Player {
                return Err(format!(
                    "entity {:?} delivers the punch but is not a player character",
                    e.id
                ));
            }
            if e.ai_state == AiState::Returning && e.alert_origin.is_none() {
                return Err(format!(
                    "entity {:?} is returning without an alert origin",
                    e.id
                ));
            }
            if e.ai_state == AiState::Patrol && (e.last_seen.is_some() || e.alert_origin.is_some())
            {
                return Err(format!(
                    "entity {:?} patrols with a last seen position or an alert origin",
                    e.id
                ));
            }
        }
        for e in &self.entities {
            if let Some(t) = e.attack_target {
                let victim = self.entities.iter().find(|v| v.id == t);
                let ok = e.kind == EntityKind::Player
                    && t != e.id
                    && victim.is_some_and(|v| v.kind == EntityKind::Guard && v.team == Team::Enemy);
                if !ok {
                    return Err(format!(
                        "entity {:?} attack target {t:?} is invalid: the order goes from a player character to an enemy soldier",
                        e.id
                    ));
                }
            }
            if let Some(f) = e.foe {
                let foe = self.entities.iter().find(|v| v.id == f);
                let ok = f != e.id
                    && foe.is_some_and(|v| match e.kind {
                        EntityKind::Player => v.kind == EntityKind::Guard && v.team == Team::Enemy,
                        _ => v.kind == EntityKind::Player,
                    });
                if !ok {
                    return Err(format!(
                        "entity {:?} foe {f:?} is invalid: a fight is between a player character and an enemy soldier",
                        e.id
                    ));
                }
            }
        }
        if let Some((x, y)) = self.press {
            let bound = Fixed::from_int(MAX_ENTITY_COORD);
            if x.abs() > bound || y.abs() > bound {
                return Err(format!("press position ({x:?}, {y:?}) out of range"));
            }
        }
        if self.damage_numbers.len() > MAX_DAMAGE_NUMBERS {
            return Err(format!(
                "{} damage numbers exceed the limit",
                self.damage_numbers.len()
            ));
        }
        if let Some(d) = self
            .damage_numbers
            .iter()
            .find(|d| d.age >= DAMAGE_NUMBER_TICKS)
        {
            return Err(format!("damage number {d:?} outlived its rise"));
        }
        for cursor in self.cursors.all() {
            if !self.entities.is_empty() && cursor as usize >= self.entities.len() {
                return Err(format!(
                    "simulation cursor {cursor} beyond the {} entities",
                    self.entities.len()
                ));
            }
            if self.entities.is_empty() && cursor != 0 {
                return Err("simulation cursor without entities".into());
            }
        }
        if let Some(sel) = self.selected
            && !ids.contains(&sel)
        {
            return Err(format!("selected entity {sel:?} does not exist"));
        }
        if let Some(c) = self.last_ground_click {
            let bound = Fixed::from_int(MAX_ENTITY_COORD);
            if c.tick > self.tick || c.x.abs() > bound || c.y.abs() > bound {
                return Err(format!("last ground click {c:?} out of range"));
            }
        }
        if let Some(vm) = &self.vm {
            vm.validate(self.programs.len(), self.entities.len())?;
            if vm.rng.seed != self.seed || vm.rng.stream != SCRIPT_RNG_STREAM {
                return Err(format!(
                    "script rng stream identity ({}, {}) does not derive from the world seed {} and stream {SCRIPT_RNG_STREAM}",
                    vm.rng.seed, vm.rng.stream, self.seed
                ));
            }
        }
        // Every stream derives from the world seed with its assigned id: a snapshot cannot
        // smuggle in another generator.
        if self.rng.seed != self.seed || self.rng.stream != GAMEPLAY_RNG_STREAM {
            return Err(format!(
                "gameplay rng stream identity ({}, {}) does not derive from the world seed {} and stream {GAMEPLAY_RNG_STREAM}",
                self.rng.seed, self.rng.stream, self.seed
            ));
        }
        self.rng.validate()
    }

    /// Apply the events of one tick (in order) and advance the simulation by one tick.
    pub fn step(&mut self, events: &[InputEvent]) {
        for e in events {
            self.apply(*e);
        }
        self.scroll();
        self.vm_tick();
        self.simulate();
        self.tick = self.tick.saturating_add(1);
    }

    /// Pointer position in map pixels (24.8).
    #[must_use]
    pub fn pointer_in_map(&self) -> (Fixed, Fixed) {
        (
            Fixed::from_raw(self.pointer.0) + Fixed::from_int(self.camera.0),
            Fixed::from_raw(self.pointer.1) + Fixed::from_int(self.camera.1),
        )
    }

    fn apply(&mut self, event: InputEvent) {
        match event {
            InputEvent::PointerMove { x256, y256 } => {
                self.pointer = (
                    x256.clamp(-MAX_POINTER_RAW, MAX_POINTER_RAW),
                    y256.clamp(-MAX_POINTER_RAW, MAX_POINTER_RAW),
                );
                self.pointer_seen = true;
            }
            InputEvent::PointerDown { button } => {
                self.buttons_down.insert(button);
                match button {
                    // The left button acts on its release: a press and a release on the same
                    // spot is the click, a stroke between them a drawn figure
                    // (`combat-measurements.md` 1.4).
                    Button::Left => self.press = Some(self.pointer_in_map()),
                    Button::Right => self.right_click(),
                    Button::Middle => {}
                }
            }
            InputEvent::PointerUp { button } => {
                self.buttons_down.remove(&button);
                if button == Button::Left
                    && let Some(from) = self.press.take()
                {
                    let to = self.pointer_in_map();
                    match figure_of(from, to) {
                        Ok(Some(figure)) => self.figure_order(figure),
                        // A stroke the engine does not read as a figure orders nothing.
                        Ok(None) => {}
                        Err(()) => self.left_click(),
                    }
                }
            }
            InputEvent::KeyDown { key } => {
                self.keys_down.insert(key);
                match key {
                    Key::Letter('c') => self.set_posture(Posture::Crouched),
                    Key::Letter('s') => self.set_posture(Posture::Standing),
                    _ => {}
                }
            }
            InputEvent::KeyUp { key } => {
                self.keys_down.remove(&key);
            }
            InputEvent::Wheel { .. } => {}
        }
    }

    fn scroll(&mut self) {
        let (mut dx, mut dy) = (0i32, 0i32);
        for k in &self.keys_down {
            match k {
                Key::Left => dx -= SCROLL_SPEED,
                Key::Right => dx += SCROLL_SPEED,
                Key::Up => dy -= SCROLL_SPEED,
                Key::Down => dy += SCROLL_SPEED,
                _ => {}
            }
        }
        let (px, py) = (
            Fixed::from_raw(self.pointer.0).round(),
            Fixed::from_raw(self.pointer.1).round(),
        );
        let (vw, vh) = (self.viewport.0 as i32, self.viewport.1 as i32);
        if self.pointer_seen && (0..vw).contains(&px) && (0..vh).contains(&py) {
            if px < EDGE_MARGIN {
                dx -= SCROLL_SPEED;
            } else if px >= vw - EDGE_MARGIN {
                dx += SCROLL_SPEED;
            }
            if py < EDGE_MARGIN {
                dy -= SCROLL_SPEED;
            } else if py >= vh - EDGE_MARGIN {
                dy += SCROLL_SPEED;
            }
        }
        let max_x = (self.map_size.0 as i32 - vw).max(0);
        let max_y = (self.map_size.1 as i32 - vh).max(0);
        self.camera.0 = self.camera.0.saturating_add(dx).clamp(0, max_x);
        self.camera.1 = self.camera.1.saturating_add(dy).clamp(0, max_y);
    }

    /// The actor under the pointer: first match in slot order (order is authoritative and
    /// hashed). The renderer draws the bars of a hovered actor with it.
    #[must_use]
    pub fn actor_at_pointer(&self) -> Option<EntityId> {
        let (px, py) = self.pointer_in_map();
        self.entities
            .iter()
            .filter(|e| {
                e.alive && e.active && matches!(e.kind, EntityKind::Player | EntityKind::Guard)
            })
            .find(|e| Fixed::length(e.x - px, e.y - py) <= e.size)
            .map(|e| e.id)
    }

    /// Slot of the selected player character if it takes orders: only a living, active one does
    /// (a deactivated actor is deselected, but a snapshot may still name one).
    fn commanded_player(&self) -> Option<usize> {
        let sel = self.selected?;
        self.entities
            .iter()
            .position(|e| e.id == sel && e.kind == EntityKind::Player && e.alive && e.active)
    }

    /// Left click (`docs/original/ui-flow.md` 9.4; `combat-measurements.md` 1.1): on an enemy
    /// while a player character is selected it orders the attack (`crate::ai`: walk to the
    /// fighting distance and fight, or the knock-out blow from behind); on any other character
    /// it selects him; on the ground it orders the selected player character to walk there
    /// (leaving a fight he is in), and a second click within [`DOUBLE_CLICK_TICKS`] and
    /// [`DOUBLE_CLICK_DISTANCE`] of the first makes the order a run. A click on the ground
    /// with nothing selected does nothing.
    fn left_click(&mut self) {
        if let Some(hit) = self.actor_at_pointer() {
            self.last_ground_click = None;
            let enemy = self
                .entities
                .iter()
                .position(|e| e.id == hit && e.kind == EntityKind::Guard && e.team == Team::Enemy);
            match (enemy, self.commanded_player()) {
                (Some(t), Some(i)) if self.entities[i].ai_state == AiState::Fighting => {
                    // Already fighting him: the order stands; another soldier: leave this
                    // fight and go for him.
                    if self.entities[i].foe != Some(hit) {
                        self.leave_fight(i);
                        self.order_attack(i, t, None);
                    }
                }
                (Some(t), Some(i)) if self.entities[i].ai_state == AiState::Patrol => {
                    self.order_attack(i, t, None);
                }
                _ => self.selected = Some(hit),
            }
            return;
        }
        let Some(i) = self.commanded_player() else {
            self.last_ground_click = None;
            return;
        };
        let target = self.pointer_in_map();
        if self.entities[i].ai_state == AiState::Fighting {
            self.leave_fight(i);
        }
        if self.entities[i].ai_state != AiState::Patrol {
            // A character delivering a blow finishes it first.
            self.last_ground_click = None;
            return;
        }
        self.entities[i].attack_target = None;
        self.entities[i].figure = None;
        let double = self.last_ground_click.is_some_and(|c| {
            self.tick.saturating_sub(c.tick) <= DOUBLE_CLICK_TICKS
                && Fixed::length(target.0 - c.x, target.1 - c.y)
                    <= Fixed::from_int(DOUBLE_CLICK_DISTANCE)
        });
        self.plan_path(i, target);
        if double {
            // The run replaces the walk the first click ordered; a third click starts over.
            self.last_ground_click = None;
            let e = &mut self.entities[i];
            if e.target.is_some() {
                e.gait = Gait::Run;
            }
        } else {
            self.last_ground_click = Some(GroundClick {
                tick: self.tick,
                x: target.0,
                y: target.1,
            });
        }
    }

    /// Right click: on the selected character it cancels his order (and the fight he is in);
    /// anywhere else it deselects.
    fn right_click(&mut self) {
        self.last_ground_click = None;
        let hit = self.actor_at_pointer();
        match (hit, self.commanded_player()) {
            (Some(h), Some(i)) if self.entities[i].id == h => {
                if self.entities[i].ai_state == AiState::Fighting {
                    self.leave_fight(i);
                }
                let e = &mut self.entities[i];
                e.target = None;
                e.path.clear();
                e.gait = Gait::Walk;
                e.attack_target = None;
                e.figure = None;
            }
            _ => self.selected = None,
        }
    }

    /// The attack order: player character `i` walks towards soldier `t` (`attack_target`; the
    /// stealth layer stops him in reach), carrying the figure he drew, if any.
    fn order_attack(&mut self, i: usize, t: usize, figure: Option<Figure>) {
        let to = (self.entities[t].x, self.entities[t].y);
        let id = self.entities[t].id;
        self.plan_path(i, to);
        let e = &mut self.entities[i];
        e.attack_target = Some(id);
        e.figure = figure;
    }

    /// Player character `i` leaves his fight on the player's order; his foe stands his ground
    /// (`ai::World::end_fight`, with a per-order search budget for the soldier's way back).
    fn leave_fight(&mut self, i: usize) {
        let foe = self.entities[i].foe;
        let mut budget = ORDER_SEARCH_WORK;
        self.end_fight(i, &mut budget);
        if let Some(t) = foe.and_then(|id| self.entities.iter().position(|e| e.id == id))
            && self.entities[t].ai_state == AiState::Fighting
            && self.entities[t].foe == Some(self.entities[i].id)
        {
            let mut budget = ORDER_SEARCH_WORK;
            self.end_fight(t, &mut budget);
        }
    }

    /// A drawn figure (`combat-measurements.md` 1.4): the selected player character locks it
    /// onto the nearest enemy soldier and, once he fights him, delivers the blow (the forward
    /// stroke: the powerful blow). A figure while he already fights that soldier is delivered
    /// in the fight; while he fights another, he leaves that fight.
    fn figure_order(&mut self, figure: Figure) {
        self.last_ground_click = None;
        let Some(i) = self.commanded_player() else {
            return;
        };
        let e = &self.entities[i];
        if !matches!(e.ai_state, AiState::Patrol | AiState::Fighting) {
            return;
        }
        let Some(t) = self.nearest_fightable(e.x, e.y) else {
            return;
        };
        let target = self.entities[t].id;
        if self.entities[i].ai_state == AiState::Fighting {
            if self.entities[i].foe == Some(target) {
                self.entities[i].figure = Some(figure);
                return;
            }
            self.leave_fight(i);
        }
        self.order_attack(i, t, Some(figure));
    }

    /// Keys `c` / `s`: the selected player character crouches / stands up. Orders and the gait
    /// are kept: a crouched character continues at the sneaking speed.
    fn set_posture(&mut self, posture: Posture) {
        if let Some(i) = self.commanded_player() {
            self.entities[i].posture = posture;
        }
    }

    fn simulate(&mut self) {
        let obstacles: Vec<(Fixed, Fixed, Fixed, Fixed)> = self
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Obstacle)
            .filter_map(|e| e.patrol.first().map(|&(hw, hh)| (e.x, e.y, hw, hh)))
            .collect();
        let (w, h) = (
            Fixed::from_int(self.map_size.0 as i32),
            Fixed::from_int(self.map_size.1 as i32),
        );
        // One simulation budget (`SIM_WORK_PER_TICK`) for the pre-index pass, the stealth
        // layer (`ai.rs`: perception, alert and knock-out states, attack orders; an alerted
        // or knocked-out guard does not run its program below) and the waypoint programs.
        let mut left = SIM_WORK_PER_TICK;
        if let Some(index) = self.sim_index(&mut left) {
            self.stealth_tick(&index, &mut left);
            self.program_walks(&index.idle_guards, &mut left);
        }
        for e in &mut self.entities {
            if !e.alive || !e.active || e.kind == EntityKind::Obstacle {
                continue;
            }
            let Some((fx, fy)) = e.target else { continue };
            // Next waypoint (or the final target when the path is exhausted).
            let (tx, ty) = e.path.first().copied().unwrap_or((fx, fy));
            let dx = tx - e.x;
            let dy = ty - e.y;
            let dist = Fixed::length(dx, dy);
            let speed = e.effective_speed(&self.catalog);
            let (nx, ny) = if dist <= speed {
                (tx, ty)
            } else {
                (e.x + dx * speed / dist, e.y + dy * speed / dist)
            };
            let blocked = obstacles.iter().any(|&(ox, oy, hw, hh)| {
                (nx - ox).abs() <= hw + e.size && (ny - oy).abs() <= hh + e.size
            }) || !self.geometry.is_walkable(nx.round(), ny.round());
            if dx.0 != 0 || dy.0 != 0 {
                e.facing256 = facing_of(dx, dy);
            }
            if blocked {
                e.target = None;
                e.path.clear();
                e.gait = Gait::Walk;
                if e.kind == EntityKind::Guard {
                    e.wait_ticks = 10;
                    if e.program.is_some() {
                        // The walk failed: the program moves on after the pause.
                        e.pc = e.pc.saturating_add(1);
                    }
                }
                continue;
            }
            e.x = nx.clamp(Fixed::ZERO, w);
            e.y = ny.clamp(Fixed::ZERO, h);
            if (e.x, e.y) == (tx, ty) {
                if !e.path.is_empty() {
                    e.path.remove(0);
                }
                if e.path.is_empty() && (e.x, e.y) == (fx, fy) {
                    e.target = None;
                    e.gait = Gait::Walk;
                    if e.kind == EntityKind::Guard {
                        if e.program.is_some() {
                            // Arrived: the `GoTo` is complete.
                            e.pc = e.pc.saturating_add(1);
                        } else {
                            e.patrol_index = (e.patrol_index + 1) % e.patrol.len().max(1) as u32;
                            e.wait_ticks = 20 + self.rng.below(20);
                        }
                    }
                }
            }
        }
        for e in &mut self.entities {
            if !e.active {
                continue;
            }
            let Some(set) = e.anim.as_ref().and_then(|a| self.catalog.sets.get(&a.set)) else {
                continue;
            };
            let wanted = wanted_animation(e, set);
            if let Some(anim) = e.anim.as_mut() {
                anim.advance(&self.catalog, wanted);
            }
        }
        // The action id every human reports (`ai::action_id`); a change is queued for the
        // script's `ActionChange` of the class bound to the actor with `(previous, new)`
        // (hypothesis on the parameter order: the actor classes compare the second parameter
        // with 141, `docs/formats/scb.md`) and delivered within what the tick's budget left;
        // what it cannot deliver waits for the next tick (`vm.rs`, "Action changes").
        let mut changes: Vec<(u32, i32, i32)> = Vec::new();
        for (i, e) in self.entities.iter_mut().enumerate() {
            if !e.active || e.kind == EntityKind::Obstacle {
                continue;
            }
            let now = action_id(e);
            if now != e.action {
                let previous = e.action;
                e.action = now;
                if let Some(class) = self.vm.as_ref().and_then(|vm| {
                    let handle = vm.program.element_of_entity(i as u32);
                    (handle >= 0)
                        .then(|| {
                            vm.program
                                .classes
                                .iter()
                                .position(|c| c.element == Some(handle as u32))
                        })
                        .flatten()
                }) {
                    changes.push((class as u32, previous as i32, now as i32));
                }
            }
        }
        for (class, previous, now) in changes {
            self.vm_queue_action_change(class, previous, now);
        }
        self.vm_deliver_action_changes();
        // The damage numbers rise and vanish (presentation).
        for d in &mut self.damage_numbers {
            d.age += 1;
        }
        self.damage_numbers.retain(|d| d.age < DAMAGE_NUMBER_TICKS);
        if let Some(p) = self.entities.iter().find(|e| e.kind == EntityKind::Player)
            && Fixed::length(p.x - self.goal.0, p.y - self.goal.1) <= Fixed::from_int(16)
        {
            self.objective_reached = true;
        }
    }

    /// The waypoint programs and legacy patrols of the idle guards (`idle`, from the tick's
    /// pre-index), one unit per guard from `cursors.programs`, round robin: a guard whose
    /// program or patrol wants a walk gets a path search within what the budget has left,
    /// capped at [`ORDER_SEARCH_WORK`]. An unreachable point is skipped after a pause; a search
    /// the budget cut short leaves the guard where he is with his instruction unchanged, the
    /// cursor on him, so the next tick plans him first.
    pub(crate) fn program_walks(&mut self, idle: &[usize], left: &mut u64) {
        let start = idle.partition_point(|&i| i < self.cursors.programs as usize);
        let order: Vec<usize> = idle[start..]
            .iter()
            .chain(&idle[..start])
            .copied()
            .collect();
        let mut cursor = 0u32;
        for i in order {
            if !charge_budget(left, 1) {
                cursor = i as u32;
                break;
            }
            let programs = &self.programs;
            let rng = &mut self.rng;
            let e = &mut self.entities[i];
            if !(e.alive
                && e.active
                && e.kind == EntityKind::Guard
                && e.target.is_none()
                && e.ai_state == AiState::Patrol)
            {
                continue;
            }
            if e.wait_ticks > 0 {
                e.wait_ticks -= 1;
                continue;
            }
            // A locked AI (script natives 134 / 135) holds its program or patrol where it is.
            if e.ai_locked {
                continue;
            }
            let target = match e.program.and_then(|p| programs.get(p as usize)) {
                Some(program) => run_program(e, program, rng),
                None if !e.patrol.is_empty() => {
                    Some(e.patrol[e.patrol_index as usize % e.patrol.len()])
                }
                None => None,
            };
            let Some(t) = target else {
                continue;
            };
            let granted = (*left).min(ORDER_SEARCH_WORK);
            let mut search = granted;
            let planned = self.plan_path_with(i, t, &mut search);
            *left -= granted - search;
            if planned == Err(NavError::WorkExhausted) {
                // Not unreachable, unpaid: the same walk is planned first next tick.
                cursor = i as u32;
                break;
            }
            let e = &mut self.entities[i];
            if e.target.is_some() {
                // Program walks use the gait the script set (native 140).
                e.gait = e.npc_gait;
            } else {
                // Unreachable point: skip it.
                if e.program.is_some() {
                    e.pc = e.pc.saturating_add(1);
                } else {
                    e.patrol_index = (e.patrol_index + 1) % e.patrol.len().max(1) as u32;
                }
                e.wait_ticks = 10;
            }
        }
        self.cursors.programs = cursor;
    }

    /// A blow of `amount` hit points landed on a victim whose feet are at `at`: a damage
    /// number starts rising there (the oldest is dropped beyond [`MAX_DAMAGE_NUMBERS`]).
    pub(crate) fn push_damage_number(&mut self, at: (i32, i32), amount: i32) {
        if self.damage_numbers.len() >= MAX_DAMAGE_NUMBERS {
            self.damage_numbers.remove(0);
        }
        self.damage_numbers.push(DamageNumber {
            x: at.0,
            y: at.1,
            amount,
            age: 0,
        });
    }

    /// Snapshot everything authoritative. `content` is the fingerprint of the game content the
    /// world was built from (`None` for synthetic scenarios); the core has no I/O, so the app
    /// supplies it.
    #[must_use]
    pub fn snapshot(&self, content: Option<String>) -> Snapshot {
        Snapshot {
            version: SNAPSHOT_VERSION,
            ruleset: crate::RULESET_VERSION,
            hash_schema: HASH_SCHEMA_VERSION,
            content,
            world: self.clone(),
        }
    }

    /// Validate a snapshot (envelope versions, every world invariant, animation state against
    /// the attached catalog), build the navigation grid of its geometry, and only if all of that
    /// succeeded make it the current state. The catalog is kept (it is static data). On error
    /// the world is unchanged, its grid included. Content identity ([`Snapshot::check_content`])
    /// is the caller's check: the core cannot fingerprint content.
    pub fn restore(&mut self, snap: &Snapshot) -> Result<(), String> {
        snap.check_versions()?;
        snap.world.validate_with(&self.catalog)?;
        let same_geometry = self.nav.is_some()
            && self.geometry == snap.world.geometry
            && self.map_size == snap.world.map_size;
        let built = if same_geometry {
            None
        } else {
            Some(
                NavGrid::try_build(
                    &snap.world.geometry,
                    snap.world.map_size.0,
                    snap.world.map_size.1,
                )
                .map_err(|e| format!("navigation grid: {e}"))?,
            )
        };
        let catalog = std::mem::take(&mut self.catalog);
        let nav = built.or_else(|| self.nav.take());
        *self = snap.world.clone();
        self.catalog = catalog;
        self.nav = nav;
        Ok(())
    }

    /// Observation for the harness.
    #[must_use]
    pub fn observe(&self, with_entities: bool) -> Observation {
        Observation {
            tick: self.tick,
            scenario: self.scenario.clone(),
            viewport: self.viewport,
            map_size: self.map_size,
            camera: self.camera,
            pointer: self.pointer,
            selected: self.selected,
            entities: if with_entities {
                self.entities
                    .iter()
                    .map(|e| EntityObservation {
                        entity: e.clone(),
                        in_combat: e.foe.is_some(),
                    })
                    .collect()
            } else {
                Vec::new()
            },
            rng_draws: self.rng.draws,
            objective_reached: self.objective_reached,
            hero_dead: self.hero_dead,
            script: self.script_observation(),
        }
    }

    /// Canonical hashes (ADR-0004). Every authoritative field is encoded, in a fixed order, with
    /// explicit tags and lengths.
    #[must_use]
    pub fn hashes(&self) -> Hashes {
        let mut parts = BTreeMap::new();

        let mut w = Encoder::new("world");
        w.u64(self.tick)
            .u32(self.viewport.0)
            .u32(self.viewport.1)
            .u32(self.map_size.0)
            .u32(self.map_size.1)
            .i32(self.camera.0)
            .i32(self.camera.1)
            .i32(self.pointer.0)
            .i32(self.pointer.1)
            .u8(u8::from(self.pointer_seen))
            .u64(self.seed)
            .u8(u8::from(self.objective_reached))
            .i32(self.goal.0.raw())
            .i32(self.goal.1.raw())
            .u8(u8::from(self.hero_dead));
        match self.last_ground_click {
            Some(c) => w.u8(1).u64(c.tick).i32(c.x.raw()).i32(c.y.raw()),
            None => w.u8(0),
        };
        match self.press {
            Some((x, y)) => w.u8(1).i32(x.raw()).i32(y.raw()),
            None => w.u8(0),
        };
        match &self.scenario {
            Scenario::Synthetic(n) => w.u8(1).str(n),
            Scenario::Mission(n) => w.u8(2).str(n),
            Scenario::MapView { map, ambiance } => w.u8(3).str(map).str(ambiance),
            Scenario::Menu(n) => w.u8(4).str(n),
        };
        w.u32(self.buttons_down.len() as u32);
        for b in &self.buttons_down {
            w.u8(button_tag(*b));
        }
        let mut keys = Vec::new();
        for k in &self.keys_down {
            encode_key(*k, &mut keys);
        }
        w.u32(self.keys_down.len() as u32).bytes(&keys);
        for cursor in self.cursors.all() {
            w.u32(cursor);
        }
        parts.insert("world".into(), w.finish());

        let mut a = Encoder::new("actors");
        a.u32(self.entities.len() as u32);
        for e in &self.entities {
            a.u32(e.id.index).u32(e.id.generation).u8(e.kind.tag());
            a.i32(e.x.raw())
                .i32(e.y.raw())
                .i32(e.size.raw())
                .i32(e.speed.raw())
                .i32(e.facing256)
                .u8(u8::from(e.alive))
                .u8(u8::from(e.active))
                .u8(u8::from(e.ai_locked))
                .u8(e.gait.tag())
                .u8(e.posture.tag())
                .u8(e.team.tag())
                .u8(e.ai_state.tag())
                .u32(e.state_ticks)
                .u32(e.action)
                .i32(e.hp)
                .i32(e.hp_max)
                .i32(e.energy)
                .u32(e.energy_ticks)
                .u8(e.pose.tag())
                .u32(e.pose_ticks)
                .u32(e.swing_ticks)
                .u8(e.figure.map_or(0, Figure::tag))
                .i32(e.knockout_resistance)
                .u8(e.npc_gait.tag())
                .u8(u8::from(e.fell_backward))
                .u8(u8::from(e.heard))
                .u32(e.wait_ticks)
                .u32(e.patrol_index)
                .u32(e.patrol.len() as u32);
            for (x, y) in &e.patrol {
                a.i32(x.raw()).i32(y.raw());
            }
            match &e.anim {
                Some(st) => a
                    .u8(1)
                    .str(&st.set)
                    .u32(st.animation)
                    .u32(st.frame)
                    .u32(st.elapsed),
                None => a.u8(0),
            };
            match e.program {
                Some(p) => a.u8(1).u32(p),
                None => a.u8(0),
            };
            a.u32(e.pc);
            for p in [e.last_seen, e.alert_origin] {
                match p {
                    Some((x, y)) => a.u8(1).i32(x.raw()).i32(y.raw()),
                    None => a.u8(0),
                };
            }
            for id in [e.attack_target, e.foe] {
                match id {
                    Some(t) => a.u8(1).u32(t.index).u32(t.generation),
                    None => a.u8(0),
                };
            }
        }
        a.u32(self.programs.len() as u32);
        for p in &self.programs {
            a.u32(p.len() as u32);
            for ins in p {
                a.u8(ins.tag());
                match ins {
                    Instruction::GoTo { x, y } => a.i32(*x).i32(*y),
                    Instruction::Wait { ticks } => a.u32(*ticks),
                    Instruction::Face { facing256 } => a.i32(*facing256),
                    Instruction::Turn { delta256 } => a.i32(*delta256),
                    Instruction::Jump { pc } => a.u32(*pc),
                    Instruction::Choose { arms } => {
                        a.u32(arms.len() as u32);
                        for &(percent, pc) in arms {
                            a.u8(percent).u32(pc);
                        }
                        &mut a
                    }
                    Instruction::Stop => &mut a,
                    Instruction::Nop { opcode } => a.u8(*opcode),
                };
            }
        }
        parts.insert("actors".into(), a.finish());

        let mut o = Encoder::new("orders");
        match self.selected {
            Some(id) => o.u8(1).u32(id.index).u32(id.generation),
            None => o.u8(0),
        };
        o.u32(self.entities.len() as u32);
        for e in &self.entities {
            match e.target {
                Some((x, y)) => o.u8(1).i32(x.raw()).i32(y.raw()),
                None => o.u8(0),
            };
            o.u32(e.path.len() as u32);
            for (x, y) in &e.path {
                o.i32(x.raw()).i32(y.raw());
            }
        }
        parts.insert("orders".into(), o.finish());

        let mut r = Encoder::new("rng");
        r.str(Rng::ALGORITHM)
            .u64(self.rng.seed)
            .u64(self.rng.stream)
            .u64(self.rng.state())
            .u64(self.rng.draws);
        match &self.vm {
            Some(vm) => r
                .u8(1)
                .str("script")
                .u64(vm.rng.seed)
                .u64(vm.rng.stream)
                .u64(vm.rng.state())
                .u64(vm.rng.draws),
            None => r.u8(0),
        };
        parts.insert("rng".into(), r.finish());

        let mut g = Encoder::new("pathfinding");
        g.u32(self.geometry.boundary.len() as u32);
        for (x, y) in &self.geometry.boundary {
            g.i32(*x).i32(*y);
        }
        g.u32(self.geometry.obstacles.len() as u32);
        for o in &self.geometry.obstacles {
            g.u32(o.len() as u32);
            for (x, y) in o {
                g.i32(*x).i32(*y);
            }
        }
        g.u32(self.geometry.areas.len() as u32);
        for a in &self.geometry.areas {
            g.u32(a.len() as u32);
            for (x, y) in a {
                g.i32(*x).i32(*y);
            }
        }
        parts.insert("pathfinding".into(), g.finish());

        // Script VM (ADR-0008): program identity and script-visible state under `scripts`, the
        // queues, sequences and pending texts under `scheduler`; a world without a script
        // encodes the absence.
        let mut s = Encoder::new("scripts");
        let mut q = Encoder::new("scheduler");
        if let Some(vm) = &self.vm {
            s.u8(1);
            vm.encode_scripts(&mut s);
            q.u8(1);
            vm.encode_scheduler(&mut q);
        } else {
            s.u8(0);
            q.u8(0);
        }
        parts.insert("scripts".into(), s.finish());
        parts.insert("scheduler".into(), q.finish());

        // The campaign subsystem does not exist yet: it hashes to a versioned constant so the
        // set of parts is stable across milestones and its appearance is a visible ruleset change.
        let mut e = Encoder::new("campaign");
        e.u8(0);
        parts.insert("campaign".into(), e.finish());

        let t = total(&parts);
        parts.insert("total".into(), t);
        Hashes { parts }
    }
}

/// Execute an idle entity's program until it blocks: on a `GoTo` the target is returned for path
/// planning (the pc stays on the `GoTo` until the walk ends), on `Wait` / `Stop` / end of program
/// or after [`PROGRAM_STEPS_PER_TICK`] instructions nothing is returned.
fn run_program(e: &mut Entity, program: &[Instruction], rng: &mut Rng) -> Option<(Fixed, Fixed)> {
    for _ in 0..PROGRAM_STEPS_PER_TICK {
        let ins = program.get(e.pc as usize)?;
        match ins {
            Instruction::GoTo { x, y } => {
                return Some((Fixed::from_int(*x), Fixed::from_int(*y)));
            }
            Instruction::Wait { ticks } => {
                e.pc += 1;
                if *ticks > 0 {
                    e.wait_ticks = ticks - 1;
                    return None;
                }
            }
            Instruction::Face { facing256 } => {
                e.facing256 = facing256.rem_euclid(256);
                e.pc += 1;
            }
            Instruction::Turn { delta256 } => {
                e.facing256 = (e.facing256 + delta256).rem_euclid(256);
                e.pc += 1;
            }
            Instruction::Jump { pc } => e.pc = *pc,
            Instruction::Choose { arms } => {
                let roll = rng.below(100);
                let mut acc = 0u32;
                let mut next = e.pc + 1;
                for &(percent, pc) in arms {
                    acc += u32::from(percent);
                    if roll < acc {
                        next = pc;
                        break;
                    }
                }
                e.pc = next;
            }
            Instruction::Stop => return None,
            Instruction::Nop { .. } => e.pc += 1,
        }
    }
    None
}

/// What a left-button stroke from `from` to `to` (map pixels) is: `Err(())` for a click (shorter
/// than [`FIGURE_MIN_STROKE`]), `Ok(Some(figure))` for a stroke the engine reads as a figure
/// (the forward stroke: at least the minimum to the right, within 45 degrees of horizontal;
/// `combat-measurements.md` 1.4 drew it 80 px right and 20 px up), `Ok(None)` for any other
/// stroke (the other eight figures are not modelled).
fn figure_of(from: (Fixed, Fixed), to: (Fixed, Fixed)) -> Result<Option<Figure>, ()> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    if Fixed::length(dx, dy) < Fixed::from_int(FIGURE_MIN_STROKE) {
        return Err(());
    }
    Ok((dx >= Fixed::from_int(FIGURE_MIN_STROKE) && dy.abs() <= dx)
        .then_some(Figure::ForwardStroke))
}

/// Facing from a direction vector: 8-way quantised to 1/256 turns, exact and deterministic.
pub(crate) fn facing_of(dx: Fixed, dy: Fixed) -> i32 {
    let (ax, ay) = (i64::from(dx.abs().raw()), i64::from(dy.abs().raw()));
    let diagonal = ax * 2 > ay && ay * 2 > ax;
    let octant = match (dx.raw() >= 0, dy.raw() >= 0, diagonal, ax >= ay) {
        (true, true, true, _) => 1,
        (false, true, true, _) => 3,
        (false, false, true, _) => 5,
        (true, false, true, _) => 7,
        (true, _, false, true) => 0,
        (_, true, false, false) => 2,
        (false, _, false, true) => 4,
        (_, false, false, false) => 6,
    };
    octant * 32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click(world: &mut World, x: i32, y: i32, button: Button) {
        world.step(&[
            InputEvent::PointerMove {
                x256: x * 256,
                y256: y * 256,
            },
            InputEvent::PointerDown { button },
            InputEvent::PointerUp { button },
        ]);
    }

    fn corridor(seed: u64) -> World {
        World::new(Scenario::Synthetic("corridor".into()), seed).unwrap()
    }

    #[test]
    fn player_moves_when_selected_and_ordered() {
        let mut w = corridor(7);
        click(&mut w, 80, 240, Button::Left);
        assert!(w.selected.is_some());
        click(&mut w, 200, 240, Button::Left);
        for _ in 0..200 {
            w.step(&[]);
        }
        let p = &w.entities[0];
        assert_eq!((p.x.round(), p.y.round()), (200, 240));
        assert!(p.target.is_none());
        w.validate().unwrap();
    }

    #[test]
    fn same_inputs_same_hashes_and_restore_is_transparent() {
        let run = |snap_at: Option<u64>| {
            let mut w = corridor(3);
            click(&mut w, 80, 240, Button::Left);
            click(&mut w, 300, 200, Button::Left);
            let mut saved = None;
            for t in 0..300u64 {
                if Some(t) == snap_at {
                    saved = Some(w.snapshot(None));
                }
                w.step(&[]);
                if snap_at.is_some_and(|s| t == s + 25) {
                    w.restore(saved.as_ref().unwrap()).unwrap();
                    for _ in 0..26 {
                        w.step(&[]);
                    }
                }
            }
            w.hashes()
        };
        let a = run(None);
        let b = run(None);
        assert_eq!(a, b);
        let c = run(Some(100));
        assert_eq!(
            a.total(),
            c.total(),
            "restore changed the outcome: {:?}",
            a.diff(&c)
        );
    }

    /// Golden hash of a fixed script. Changing the encoding, the ruleset or the scenario changes
    /// this value: bump `RULESET_VERSION` / `HASH_SCHEMA_VERSION` and update the constant on purpose.
    #[test]
    fn golden_hash_of_the_corridor_script() {
        let mut w = corridor(11);
        click(&mut w, 80, 240, Button::Left);
        click(&mut w, 600, 240, Button::Left);
        w.step(&[InputEvent::KeyDown { key: Key::Right }]);
        for _ in 0..398 {
            w.step(&[]);
        }
        assert!(w.objective_reached);
        assert_eq!(
            w.hashes().total(),
            GOLDEN_CORRIDOR_TOTAL,
            "{:?}",
            w.hashes()
        );
    }

    const GOLDEN_CORRIDOR_TOTAL: &str =
        "0f026ac0a3b668f6032272475dcfd35ae55f5ed0ebbee8ab1be2c4a742f5391a";

    #[test]
    fn every_authoritative_field_changes_some_hash() {
        let base = corridor(5);
        let h0 = base.hashes();
        let mut variants: Vec<(&str, World)> = Vec::new();
        let mut w = base.clone();
        w.goal.0 += Fixed::ONE;
        variants.push(("goal", w));
        let mut w = base.clone();
        w.buttons_down.insert(Button::Middle);
        variants.push(("buttons", w));
        let mut w = base.clone();
        w.keys_down.insert(Key::Space);
        variants.push(("keys", w));
        let mut w = base.clone();
        w.entities[1].patrol[0].0 += Fixed::ONE;
        variants.push(("patrol", w));
        let mut w = base.clone();
        w.entities.swap(0, 1);
        variants.push(("order", w));
        let mut w = base.clone();
        w.camera.0 = 1;
        w.map_size.0 = 641;
        variants.push(("camera", w));
        let mut w = base.clone();
        w.entities[0].anim = Some(AnimState::new("x", 0));
        variants.push(("anim", w));
        let mut w = base.clone();
        w.entities[1].pc = 3;
        variants.push(("pc", w));
        let mut w = base.clone();
        w.entities[0].gait = Gait::Run;
        variants.push(("gait", w));
        let mut w = base.clone();
        w.entities[0].posture = Posture::Crouched;
        variants.push(("posture", w));
        let mut w = base.clone();
        w.last_ground_click = Some(GroundClick {
            tick: 0,
            x: Fixed::ONE,
            y: Fixed::ONE,
        });
        variants.push(("last_ground_click", w));
        let mut w = base.clone();
        w.entities[1].team = Team::Civilian;
        variants.push(("team", w));
        let mut w = base.clone();
        w.entities[1].ai_state = AiState::Alarm;
        variants.push(("ai_state", w));
        let mut w = base.clone();
        w.entities[1].state_ticks = 5;
        variants.push(("state_ticks", w));
        let mut w = base.clone();
        w.entities[1].last_seen = Some((Fixed::ONE, Fixed::ONE));
        variants.push(("last_seen", w));
        let mut w = base.clone();
        w.entities[1].alert_origin = Some((Fixed::ONE, Fixed::ONE));
        variants.push(("alert_origin", w));
        let mut w = base.clone();
        w.entities[0].attack_target = Some(base.entities[1].id);
        variants.push(("attack_target", w));
        let mut w = base.clone();
        w.entities[1].action = 141;
        variants.push(("action", w));
        let mut w = base.clone();
        w.entities[1].hp -= 1;
        variants.push(("hp", w));
        let mut w = base.clone();
        w.entities[1].hp_max += 1;
        variants.push(("hp_max", w));
        let mut w = base.clone();
        w.entities[1].energy -= 1;
        variants.push(("energy", w));
        let mut w = base.clone();
        w.entities[1].energy_ticks = 5;
        variants.push(("energy_ticks", w));
        let mut w = base.clone();
        w.entities[1].foe = Some(base.entities[0].id);
        variants.push(("foe", w));
        let mut w = base.clone();
        w.entities[1].pose = FightPose::Strike;
        variants.push(("pose", w));
        let mut w = base.clone();
        w.entities[1].pose_ticks = 4;
        variants.push(("pose_ticks", w));
        let mut w = base.clone();
        w.entities[1].swing_ticks = 4;
        variants.push(("swing_ticks", w));
        let mut w = base.clone();
        w.entities[0].figure = Some(Figure::ForwardStroke);
        variants.push(("figure", w));
        let mut w = base.clone();
        w.press = Some((Fixed::ONE, Fixed::ONE));
        variants.push(("press", w));
        let mut w = base.clone();
        w.hero_dead = true;
        variants.push(("hero_dead", w));
        let mut w = base.clone();
        w.entities[1].knockout_resistance = 35;
        variants.push(("knockout_resistance", w));
        let mut w = base.clone();
        w.entities[1].npc_gait = Gait::Run;
        variants.push(("npc_gait", w));
        let mut w = base.clone();
        w.entities[1].fell_backward = true;
        variants.push(("fell_backward", w));
        let mut w = base.clone();
        w.entities[1].heard = true;
        variants.push(("heard", w));
        let mut w = base.clone();
        w.cursors.perception = 1;
        variants.push(("cursors.perception", w));
        let mut w = base.clone();
        w.cursors.states = 1;
        variants.push(("cursors.states", w));
        let mut w = base.clone();
        w.cursors.attacks = 1;
        variants.push(("cursors.attacks", w));
        let mut w = base.clone();
        w.cursors.programs = 1;
        variants.push(("cursors.programs", w));
        let mut w = base.clone();
        w.programs.push(vec![Instruction::Stop]);
        variants.push(("programs", w));
        let mut w = base.clone();
        w.programs.push(vec![Instruction::Stop]);
        w.entities[1].program = Some(0);
        let mut w2 = base.clone();
        w2.programs.push(vec![Instruction::Stop]);
        assert_ne!(w.hashes().total(), w2.hashes().total(), "program ref");
        for (name, v) in variants {
            assert_ne!(
                v.hashes().total(),
                h0.total(),
                "{name} not covered by the hash"
            );
        }
    }

    #[test]
    fn camera_scrolls_with_keys_and_edges_and_affects_picking() {
        let scenario = Scenario::MapView {
            map: "test".into(),
            ambiance: "Day".into(),
        };
        let mut w = World::new_map_view(
            scenario,
            1,
            MapInfo {
                width: 2000,
                height: 1000,
            },
        )
        .unwrap();
        w.step(&[InputEvent::KeyDown { key: Key::Right }]);
        w.step(&[]);
        w.step(&[InputEvent::KeyUp { key: Key::Right }]);
        assert_eq!(w.camera, (SCROLL_SPEED * 2, 0));
        w.step(&[InputEvent::PointerMove {
            x256: 320 * 256,
            y256: 767 * 256,
        }]);
        assert_eq!(w.camera.1, SCROLL_SPEED);
        click(
            &mut w,
            80 - SCROLL_SPEED * 2,
            240 - SCROLL_SPEED,
            Button::Left,
        );
        assert!(w.selected.is_some());
        for _ in 0..1000 {
            w.step(&[
                InputEvent::KeyDown { key: Key::Right },
                InputEvent::KeyDown { key: Key::Down },
            ]);
        }
        assert_eq!(w.camera, (2000 - 1024, 1000 - 768));
        w.validate().unwrap();
    }

    #[test]
    fn hostile_input_never_panics() {
        let mut w = corridor(1);
        let extremes = [i32::MIN, i32::MAX, 0, -1, 1];
        for &x in &extremes {
            for &y in &extremes {
                w.step(&[
                    InputEvent::PointerMove { x256: x, y256: y },
                    InputEvent::PointerDown {
                        button: Button::Left,
                    },
                    InputEvent::PointerDown {
                        button: Button::Right,
                    },
                    InputEvent::Wheel { delta: x },
                ]);
                for _ in 0..3 {
                    w.step(&[]);
                }
            }
        }
        w.validate().unwrap();
    }

    #[test]
    fn invalid_snapshots_are_rejected_and_leave_the_world_untouched() {
        let mut w = corridor(2);
        let before = w.hashes();
        let mut snap = w.snapshot(None);
        snap.world.entities[1].id = snap.world.entities[0].id;
        assert!(w.restore(&snap).unwrap_err().contains("duplicate"));
        let mut snap = w.snapshot(None);
        snap.world.entities[2].patrol.clear();
        assert!(w.restore(&snap).is_err());
        let mut snap = w.snapshot(None);
        snap.world.selected = Some(EntityId {
            index: 99,
            generation: 1,
        });
        assert!(w.restore(&snap).is_err());
        let mut snap = w.snapshot(None);
        snap.world.camera = (5, 0);
        assert!(w.restore(&snap).is_err());
        let mut snap = w.snapshot(None);
        snap.ruleset += 1;
        assert!(w.restore(&snap).unwrap_err().contains("ruleset"));
        let mut snap = w.snapshot(None);
        snap.version += 1;
        assert!(w.restore(&snap).unwrap_err().contains("version"));
        let mut snap = w.snapshot(None);
        snap.hash_schema += 1;
        assert!(w.restore(&snap).unwrap_err().contains("hash schema"));
        assert_eq!(w.hashes(), before);
    }

    #[test]
    fn content_identity_is_part_of_the_envelope() {
        let w = corridor(2);
        let snap = w.snapshot(None);
        snap.check_content(None).unwrap();
        assert!(
            snap.check_content(Some("abc"))
                .unwrap_err()
                .contains("no content")
        );
        let snap = w.snapshot(Some("abc".into()));
        snap.check_content(Some("abc")).unwrap();
        assert!(
            snap.check_content(Some("xyz"))
                .unwrap_err()
                .contains("different")
        );
        assert!(
            snap.check_content(None)
                .unwrap_err()
                .contains("no game content")
        );
        // The field survives JSON and a missing field reads as "no content".
        let json = serde_json::to_value(&snap).unwrap();
        assert_eq!(json["content"], "abc");
        let mut json = json;
        json.as_object_mut().unwrap().remove("content");
        let back: Snapshot = serde_json::from_value(json).unwrap();
        assert_eq!(back.content, None);
    }

    #[test]
    fn hostile_geometry_snapshots_are_rejected_and_extremes_never_panic() {
        let mut w = corridor(4);
        assert!(w.nav.is_some(), "built by the constructor");
        let before = w.hashes();
        let extremes = [
            i32::MIN,
            i32::MIN + 1,
            -(MAX_GEOMETRY_COORD + 1),
            MAX_GEOMETRY_COORD + 1,
            i32::MAX - 1,
            i32::MAX,
        ];
        for &c in &extremes {
            // Through JSON, as a hostile client would send it.
            let mut json = serde_json::to_value(w.snapshot(None)).unwrap();
            json["world"]["geometry"]["boundary"] =
                serde_json::json!([[0, 0], [c, 0], [c, c], [0, c]]);
            let snap: Snapshot = serde_json::from_value(json).unwrap();
            let err = w.restore(&snap).unwrap_err();
            assert!(
                err.contains("geometry") && err.contains("boundary"),
                "{err}"
            );
            let mut json = serde_json::to_value(w.snapshot(None)).unwrap();
            json["world"]["geometry"]["obstacles"] = serde_json::json!([[[1, 1], [2, c], [c, 2]]]);
            let snap: Snapshot = serde_json::from_value(json).unwrap();
            let err = w.restore(&snap).unwrap_err();
            assert!(err.contains("obstacle 0"), "{err}");
            assert!(
                w.nav.is_some(),
                "the navigation grid of the live world is kept"
            );
            // `set_geometry` refuses the same input and leaves the world alone.
            let err = w
                .set_geometry(Geometry {
                    boundary: vec![(0, 0), (c, 0), (0, c)],
                    obstacles: vec![],
                    areas: Vec::new(),
                })
                .unwrap_err();
            assert!(err.contains("geometry"), "{err}");
        }
        assert_eq!(w.hashes(), before);
        assert!(w.geometry.boundary.is_empty());
        // The documented extreme is accepted and rasterises without overflow.
        let m = MAX_GEOMETRY_COORD;
        let mut json = serde_json::to_value(w.snapshot(None)).unwrap();
        json["world"]["geometry"]["boundary"] =
            serde_json::json!([[-m, -m], [m, -m], [m, m], [-m, m]]);
        json["world"]["geometry"]["obstacles"] =
            serde_json::json!([[[m, -m], [-m, m], [300, 300]]]);
        let snap: Snapshot = serde_json::from_value(json).unwrap();
        w.restore(&snap).unwrap();
        assert!(w.nav.is_some());
        // Movement over such geometry runs the point-in-polygon tests with extreme vertices and
        // the pointer at its clamp; the core must neither panic nor differ between build modes,
        // which `i128` arithmetic guarantees by construction. Same inputs, same hashes.
        let mut w2 = w.clone();
        for world in [&mut w, &mut w2] {
            click(world, 80, 240, Button::Left);
            click(world, 600, 240, Button::Left);
            world.step(&[InputEvent::PointerMove {
                x256: i32::MAX,
                y256: i32::MIN,
            }]);
            for _ in 0..30 {
                world.step(&[]);
            }
            world.validate().unwrap();
        }
        assert_eq!(w.hashes(), w2.hashes());
        // The world's own accessors tolerate any query point on any accepted geometry.
        for &x in &[i32::MIN, -1, 0, m, i32::MAX] {
            for &y in &[i32::MIN, 0, i32::MAX] {
                let _ = w.geometry.is_walkable(x, y);
            }
        }
    }

    #[test]
    fn animation_state_is_validated_against_the_catalog() {
        use crate::anim::{AnimSet, FrameSpec};
        let frame = |frame, duration| FrameSpec {
            frame,
            duration,
            advance: 0,
            offset_x: 0,
            offset_y: 0,
        };
        let mut catalog = Catalog::default();
        catalog.sets.insert(
            "hero".into(),
            AnimSet::standing_only(
                vec![vec![frame(1, 3), frame(2, 1)], vec![frame(3, 1)], vec![]],
                [0; 8],
                [1; 8],
            ),
        );
        // Without a catalog only the size bounds apply (synthetic worlds, no sprite bank).
        let mut plain = corridor(6);
        let mut snap = plain.snapshot(None);
        snap.world.entities[0].anim = Some(AnimState::new("ghost", 7));
        plain.restore(&snap).unwrap();
        // With a catalog every reference must resolve.
        let mut w = corridor(6);
        w.attach_catalog(catalog, Some("hero"), Some("hero"));
        for _ in 0..5 {
            w.step(&[]);
        }
        let good = w.snapshot(None);
        let before = w.hashes();
        let reject = |w: &mut World, edit: fn(&mut AnimState), needle: &str| {
            let mut snap = good.clone();
            edit(snap.world.entities[0].anim.as_mut().unwrap());
            let err = w.restore(&snap).unwrap_err();
            assert!(err.contains(needle), "{err} should mention {needle}");
        };
        reject(&mut w, |a| a.set = "ghost".into(), "profile 'ghost'");
        reject(&mut w, |a| a.animation = 3, "animation 3 does not exist");
        reject(&mut w, |a| a.frame = 2, "frame 2 out of range");
        // Frame 1 of animation 0 lasts 3 table ticks = 135 clock units.
        reject(&mut w, |a| a.elapsed = 135, "elapsed 135 exceeds");
        reject(
            &mut w,
            |a| {
                a.animation = 2;
                a.frame = 1;
            },
            "frame 1 out of range",
        );
        assert_eq!(w.hashes(), before);
        // In-range states, including the empty animation at frame 0, are accepted.
        let mut snap = good.clone();
        let a = snap.world.entities[0].anim.as_mut().unwrap();
        (a.animation, a.frame, a.elapsed) = (0, 0, 134);
        w.restore(&snap).unwrap();
        let mut snap = good.clone();
        let a = snap.world.entities[0].anim.as_mut().unwrap();
        (a.animation, a.frame, a.elapsed) = (2, 0, 0);
        w.restore(&snap).unwrap();
        w.restore(&good).unwrap();
        assert_eq!(w.hashes(), before);
    }

    #[test]
    fn mission_spec_builds_actors_in_order() {
        let spec = MissionSpec {
            map: MapInfo {
                width: 1000,
                height: 800,
            },
            geometry: Geometry {
                boundary: vec![(0, 0), (1000, 0), (1000, 800), (0, 800)],
                obstacles: vec![vec![(180, 150), (220, 150), (220, 250), (180, 250)]],
                areas: Vec::new(),
            },
            actors: vec![
                ActorSpec {
                    profile: "RobinHood".into(),
                    team: Team::Player,
                    x: 100,
                    y: 200,
                    facing256: 64,
                    patrol: vec![],
                    program: vec![],
                    active: true,
                    hit_points: 100,
                    knockout_resistance: 0,
                },
                ActorSpec {
                    profile: "Soldier A00".into(),
                    team: Team::Enemy,
                    x: 300,
                    y: 200,
                    facing256: -32,
                    patrol: vec![(300, 200), (300, 400)],
                    program: vec![],
                    active: true,
                    hit_points: 80,
                    knockout_resistance: 0,
                },
            ],
            script: None,
            rails: Vec::new(),
            lenient_natives: false,
            starting_money: DEFAULT_STARTING_MONEY,
            assumptions: BTreeSet::new(),
        };
        let w = World::new_mission(Scenario::Mission("EmbTut".into()), 1, &spec).unwrap();
        assert_eq!(w.entities.len(), 2);
        assert_eq!(w.entities[0].kind, EntityKind::Player);
        assert_eq!(w.entities[1].facing256, 224);
        assert_eq!(w.entities[1].patrol.len(), 2);
        assert_eq!(w.entities[0].anim.as_ref().unwrap().set, "RobinHood");
        assert!(World::new_mission(Scenario::Synthetic("x".into()), 1, &spec).is_err());
        // Walking east from (100,200) with the obstacle at x=180..220 in the way: the path goes
        // around it and reaches the target.
        let mut w = w;
        w.plan_path(0, (Fixed::from_int(400), Fixed::from_int(200)));
        assert!(
            w.entities[0].path.len() >= 2,
            "path should bend around the obstacle"
        );
        for _ in 0..600 {
            w.step(&[]);
        }
        assert_eq!(
            (w.entities[0].x.round(), w.entities[0].y.round()),
            (400, 200)
        );
        assert!(w.entities[0].target.is_none());
        // A target inside the obstacle is moved to its edge.
        w.plan_path(0, (Fixed::from_int(200), Fixed::from_int(200)));
        assert!(w.entities[0].target.is_some());
    }

    /// An open 1000x800 mission with one player and guards running the given programs.
    fn programmed_mission(programs: &[Vec<Instruction>]) -> World {
        let mut actors = vec![ActorSpec {
            profile: "RobinHood".into(),
            team: Team::Player,
            x: 100,
            y: 100,
            facing256: 0,
            patrol: vec![],
            program: vec![],
            active: true,
            hit_points: 100,
            knockout_resistance: 0,
        }];
        for (i, p) in programs.iter().enumerate() {
            actors.push(ActorSpec {
                profile: "Soldier A00".into(),
                team: Team::Enemy,
                x: 300 + 100 * i as i32,
                y: 300,
                facing256: 0,
                patrol: vec![],
                program: p.clone(),
                active: true,
                hit_points: 80,
                knockout_resistance: 0,
            });
        }
        let spec = MissionSpec {
            map: MapInfo {
                width: 1000,
                height: 800,
            },
            geometry: Geometry {
                boundary: vec![(0, 0), (1000, 0), (1000, 800), (0, 800)],
                obstacles: vec![],
                areas: Vec::new(),
            },
            actors,
            script: None,
            rails: Vec::new(),
            lenient_natives: false,
            starting_money: DEFAULT_STARTING_MONEY,
            assumptions: BTreeSet::new(),
        };
        World::new_mission(Scenario::Mission("T".into()), 9, &spec).unwrap()
    }

    #[test]
    fn program_wait_goto_face_and_loop() {
        use Instruction::*;
        let program = vec![
            Face { facing256: 128 },
            Wait { ticks: 30 },
            GoTo { x: 300, y: 400 },
            Turn { delta256: -32 },
            Wait { ticks: 10 },
            GoTo { x: 300, y: 300 },
            Jump { pc: 0 },
        ];
        let mut w = programmed_mission(&[program.clone(), vec![]]);
        assert_eq!(w.programs, vec![program]);
        assert_eq!(w.entities[1].program, Some(0));
        assert_eq!(w.entities[2].program, None, "empty program = idle guard");
        // Tick 0: Face executes, Wait starts; the guard stands for 30 ticks facing west.
        w.step(&[]);
        let g = &w.entities[1];
        assert_eq!((g.facing256, g.pc, g.wait_ticks), (128, 2, 29));
        for _ in 0..29 {
            w.step(&[]);
            assert_eq!(w.entities[1].y.round(), 300);
        }
        // Tick 30: GoTo is issued; the guard walks 100 px south at the synthetic guard speed
        // (182 / 256 = 0.71 px per tick, the soldier's 42.7 px/s: 141 moves, the first on the
        // issuing tick, one or two more for the raw units the fixed-point steps lose).
        w.step(&[]);
        let g = &w.entities[1];
        assert!(g.target.is_some());
        assert_eq!(g.pc, 2, "pc stays on the GoTo while walking");
        let mut moves = 1;
        while w.entities[1].target.is_some() {
            w.step(&[]);
            moves += 1;
            assert!(moves < 200, "never arrived");
        }
        assert!((141..=143).contains(&moves), "{moves} moves");
        let g = &w.entities[1];
        assert_eq!((g.x.round(), g.y.round()), (300, 400));
        assert_eq!(g.facing256, 64, "walking south faces south");
        // Next tick: Turn (64 - 32) and the 10-tick wait; then it walks back (another 141
        // moves or so) and loops into the 30-tick wait of the Face at pc 0.
        w.step(&[]);
        assert_eq!(w.entities[1].facing256, 32);
        for _ in 0..165 {
            w.step(&[]);
        }
        let g = &w.entities[1];
        assert_eq!((g.x.round(), g.y.round()), (300, 300));
        assert_eq!(g.facing256, 128, "looped back to the Face at pc 0");
        // The idle guard never moved and never drew from the RNG.
        let idle = &w.entities[2];
        assert_eq!((idle.x.round(), idle.y.round(), idle.pc), (400, 300, 0));
        assert_eq!(w.rng.draws, 0);
        w.validate().unwrap();
    }

    #[test]
    fn program_choose_stop_nop_and_step_budget() {
        use Instruction::*;
        // 50 % face east, 50 % face west, then stop; Nops are skipped.
        let choose = vec![
            Nop { opcode: 0x42 },
            Choose {
                arms: vec![(50, 3), (50, 5)],
            },
            Jump { pc: 7 },
            Face { facing256: 0 },
            Jump { pc: 7 },
            Face { facing256: 128 },
            Jump { pc: 7 },
            Stop,
        ];
        // A single 25 % arm that never matches falls through to the Wait.
        let fallthrough = vec![
            Choose { arms: vec![(0, 2)] },
            Wait { ticks: 5 },
            Face { facing256: 64 },
            Stop,
        ];
        // Only jumps: must yield after the per-tick budget instead of spinning.
        let spin = vec![Jump { pc: 1 }, Jump { pc: 0 }];
        let mut w = programmed_mission(&[choose, fallthrough, spin]);
        w.step(&[]);
        assert_eq!(w.rng.draws, 2, "two Choose rolls");
        assert_eq!(w.entities[1].pc, 7);
        assert!(matches!(w.entities[1].facing256, 0 | 128));
        assert_eq!((w.entities[2].pc, w.entities[2].wait_ticks), (2, 4));
        for _ in 0..10 {
            w.step(&[]);
        }
        assert_eq!((w.entities[2].pc, w.entities[2].facing256), (3, 64));
        assert_eq!(w.entities[1].pc, 7, "Stop holds the pc");
        assert_eq!(w.rng.draws, 2);
        w.validate().unwrap();
    }

    #[test]
    fn programs_survive_snapshot_restore_and_are_validated() {
        use Instruction::*;
        let program = vec![
            Choose {
                arms: vec![(50, 2), (50, 4)],
            },
            Jump { pc: 6 },
            GoTo { x: 500, y: 300 },
            Jump { pc: 6 },
            GoTo { x: 300, y: 500 },
            Jump { pc: 6 },
            Wait { ticks: 7 },
            GoTo { x: 300, y: 300 },
            Jump { pc: 0 },
        ];
        let run = |snap_at: Option<u64>| {
            let mut w = programmed_mission(&[program.clone(), program.clone()]);
            let mut saved = None;
            for t in 0..900u64 {
                if Some(t) == snap_at {
                    saved = Some(w.snapshot(None));
                }
                w.step(&[]);
                if snap_at.is_some_and(|s| t == s + 40) {
                    w.restore(saved.as_ref().unwrap()).unwrap();
                    for _ in 0..41 {
                        w.step(&[]);
                    }
                }
            }
            assert!(w.rng.draws > 0);
            w.hashes()
        };
        let a = run(None);
        assert_eq!(a, run(None));
        let c = run(Some(333));
        assert_eq!(
            a.total(),
            c.total(),
            "restore changed the outcome: {:?}",
            a.diff(&c)
        );
        // Snapshot JSON round trip keeps programs and counters.
        let mut w = programmed_mission(std::slice::from_ref(&program));
        for _ in 0..50 {
            w.step(&[]);
        }
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        let snap: Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = programmed_mission(&[]);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.programs, w.programs);
        assert_eq!(w2.entities[1].pc, w.entities[1].pc);
        assert_eq!(w2.hashes(), w.hashes());
        // Invalid programs and counters are rejected.
        let mut snap = w.snapshot(None);
        snap.world.entities[1].pc = 99;
        assert!(w.restore(&snap).unwrap_err().contains("pc"));
        let mut snap = w.snapshot(None);
        snap.world.entities[1].program = Some(5);
        assert!(w.restore(&snap).is_err());
        let mut snap = w.snapshot(None);
        snap.world.programs[0][1] = Jump { pc: 1000 };
        assert!(w.restore(&snap).is_err());
        // A `Turn` by `i32::MIN` (through JSON, as a hostile client would send it) is refused
        // without panicking in either build mode.
        let mut json = serde_json::to_value(w.snapshot(None)).unwrap();
        json["world"]["programs"][0][1] = serde_json::json!({ "turn": { "delta256": i32::MIN } });
        let snap: Snapshot = serde_json::from_value(json).unwrap();
        assert!(w.restore(&snap).unwrap_err().contains("out of range"));
        let mut json = serde_json::to_value(w.snapshot(None)).unwrap();
        json["world"]["programs"][0][1] = serde_json::json!({ "turn": { "delta256": -255 } });
        let snap: Snapshot = serde_json::from_value(json).unwrap();
        w.restore(&snap).unwrap();
    }

    #[test]
    fn deactivation_clears_selection_and_orders_need_alive_active() {
        let mut w = corridor(8);
        click(&mut w, 80, 240, Button::Left);
        assert_eq!(w.selected, Some(w.entities[0].id));
        // A snapshot may still select an inactive actor: the order is refused.
        w.entities[0].active = false;
        click(&mut w, 200, 240, Button::Left);
        assert!(w.entities[0].target.is_none());
        w.entities[0].active = true;
        w.entities[0].alive = false;
        click(&mut w, 200, 240, Button::Left);
        assert!(w.entities[0].target.is_none());
        w.entities[0].alive = true;
        // Refused clicks leave no double-click memory: this one is a plain walk.
        click(&mut w, 200, 240, Button::Left);
        assert!(w.entities[0].target.is_some());
        assert_eq!(w.entities[0].gait, Gait::Walk);
        w.validate().unwrap();
    }

    #[test]
    fn camera_centring_is_total_at_extremes() {
        let mut w = World::new_map_view(
            Scenario::MapView {
                map: "test".into(),
                ambiance: "Day".into(),
            },
            1,
            MapInfo {
                width: 2000,
                height: 1000,
            },
        )
        .unwrap();
        for &x in &[i32::MIN, -1, 0, 1000, i32::MAX] {
            for &y in &[i32::MIN, 0, 500, i32::MAX] {
                w.center_camera_on(x, y);
                w.validate().unwrap();
            }
        }
        w.center_camera_on(i32::MIN, i32::MAX);
        assert_eq!(w.camera, (0, 1000 - 768));
        w.center_camera_on(i32::MAX, i32::MIN);
        assert_eq!(w.camera, (2000 - 1024, 0));
        w.center_camera_on(1000, 500);
        assert_eq!(w.camera, (1000 - 512, 500 - 384));
    }

    #[test]
    fn rng_streams_must_derive_from_the_world_seed() {
        let mut w = corridor(21);
        let before = w.hashes();
        let mut snap = w.snapshot(None);
        snap.world.rng = Rng::new(22, GAMEPLAY_RNG_STREAM);
        assert!(w.restore(&snap).unwrap_err().contains("gameplay rng"));
        let mut snap = w.snapshot(None);
        snap.world.rng = Rng::new(21, 7);
        assert!(w.restore(&snap).unwrap_err().contains("gameplay rng"));
        let mut snap = w.snapshot(None);
        snap.world.seed = 5;
        assert!(w.restore(&snap).is_err());
        assert_eq!(w.hashes(), before);
        // Entity positions are bounded like geometry.
        let mut json = serde_json::to_value(w.snapshot(None)).unwrap();
        json["world"]["entities"][0]["x"] = serde_json::json!(i32::MAX);
        let snap: Snapshot = serde_json::from_value(json).unwrap();
        assert!(w.restore(&snap).unwrap_err().contains("position"));
    }

    #[test]
    fn plan_path_charges_its_conversion_and_needs_exactly_its_work() {
        let mut w = crate::vm::tests::mission_world(0, None);
        let target = (Fixed::from_int(900), Fixed::from_int(700));
        let mut full = ORDER_SEARCH_WORK;
        w.plan_path_with(0, target, &mut full).unwrap();
        let used = ORDER_SEARCH_WORK - full;
        let path = w.entities[0].path.clone();
        assert_eq!(w.entities[0].target, Some(target));
        assert!(!path.is_empty());
        // Every stage is charged; the conversion alone costs one unit per point of the path.
        assert!(used > path.len() as u64);
        // The exact amount plans the same path; one unit less plans nothing and clears the
        // order; a zero budget allocates nothing at all.
        let mut exact = used;
        w.plan_path_with(0, target, &mut exact).unwrap();
        assert_eq!((w.entities[0].path.clone(), exact), (path, 0));
        let mut short = used - 1;
        assert_eq!(
            w.plan_path_with(0, target, &mut short),
            Err(NavError::WorkExhausted)
        );
        assert!(w.entities[0].target.is_none() && w.entities[0].path.is_empty());
        assert_eq!(short, 0);
        let mut zero = 0;
        assert_eq!(
            w.plan_path_with(0, target, &mut zero),
            Err(NavError::WorkExhausted)
        );
        w.validate().unwrap();
    }

    #[test]
    fn restore_and_set_geometry_build_the_grid_before_committing() {
        let mut w = corridor(3);
        let before = w.hashes();
        let grid = w.nav.clone().expect("built by the constructor");
        // A geometry over the navigation budget is refused by both paths with the world and its
        // grid untouched (the vertex budget is met, the scan-conversion budget is not).
        let rows = 4096u32;
        let big: Vec<(i32, i32)> = (0..20_000)
            .map(|i| (i % 7, (i * 13) % (rows as i32 * crate::nav::CELL)))
            .collect();
        let heavy = Geometry {
            boundary: big.clone(),
            obstacles: vec![big.clone(), big],
            areas: Vec::new(),
        };
        let mut snap = w.snapshot(None);
        snap.world.map_size = (8, rows * crate::nav::CELL as u32);
        snap.world.viewport = (8, 8);
        snap.world.geometry = heavy.clone();
        let err = w.restore(&snap).unwrap_err();
        assert!(err.contains("navigation"), "{err}");
        assert_eq!(w.hashes(), before);
        assert_eq!(w.nav.as_ref(), Some(&grid));
        let mut v = w.clone();
        v.map_size = (8, rows * crate::nav::CELL as u32);
        v.viewport = (8, 8);
        let err = v.set_geometry(heavy).unwrap_err();
        assert!(err.contains("navigation"), "{err}");
        assert_eq!(v.nav.as_ref(), Some(&grid));
        assert!(v.geometry.boundary.is_empty());
        // A restore into a world without a grid builds one for the snapshot's geometry.
        let mut bare = corridor(3);
        bare.nav = None;
        bare.restore(&w.snapshot(None)).unwrap();
        assert_eq!(bare.nav.as_ref(), Some(&grid));
        bare.nav = None;
        bare.try_ensure_nav().unwrap();
        assert_eq!(bare.nav.as_ref(), Some(&grid));
    }

    #[test]
    fn unknown_scenario_is_an_error() {
        assert!(World::new(Scenario::Synthetic("nope".into()), 1).is_err());
        assert!(World::new(Scenario::Mission("H01".into()), 1).is_err());
        assert!(
            World::new(
                Scenario::MapView {
                    map: "x".into(),
                    ambiance: "Day".into()
                },
                1
            )
            .is_err()
        );
    }

    /// Ticks until the player of a fresh corridor (selected, then ordered) has arrived, and the
    /// gait it reported while under way.
    fn ticks_to_arrive(order: impl Fn(&mut World)) -> (u32, Gait) {
        let mut w = corridor(12);
        click(&mut w, 80, 240, Button::Left);
        order(&mut w);
        let gait = w.entities[0].gait;
        assert!(w.entities[0].target.is_some());
        let mut ticks = 0;
        while w.entities[0].target.is_some() {
            w.step(&[]);
            ticks += 1;
            assert!(ticks < 1000, "never arrived");
        }
        assert_eq!(w.entities[0].gait, Gait::Walk, "the gait resets on arrival");
        w.validate().unwrap();
        (ticks, gait)
    }

    #[test]
    fn double_click_runs_at_the_running_speed() {
        let (walk, g) = ticks_to_arrive(|w| click(w, 200, 240, Button::Left));
        assert_eq!(g, Gait::Walk);
        // Two clicks in consecutive ticks, 3 px apart: a double click.
        let (run, g) = ticks_to_arrive(|w| {
            click(w, 200, 240, Button::Left);
            click(w, 203, 240, Button::Left);
        });
        assert_eq!(g, Gait::Run);
        // 120 px at the synthetic walking speed (364 / 256 = 1.42 px per tick, the hero's
        // measured 85.3 px/s) is 85 moves walking (the first one on the click's own tick),
        // at the running speed (5 / 4 of it, 455 / 256 = 1.78 px per tick) 67 more moves after
        // the walk of the first click's tick, plus one for the raw units the fixed-point
        // steps lose along the way.
        assert_eq!((walk, run), (84, 68));
        let mut runner = corridor(12).entities[0].clone();
        runner.gait = Gait::Run;
        assert_eq!(runner.effective_speed(&Catalog::default()).raw(), 455);
        runner.gait = Gait::Walk;
        assert_eq!(
            runner.effective_speed(&Catalog::default()),
            SYNTHETIC_PLAYER_SPEED
        );
        // Too late (21 ticks between the presses) or too far (9 px): two walks.
        let (late, g) = ticks_to_arrive(|w| {
            click(w, 200, 240, Button::Left);
            for _ in 0..DOUBLE_CLICK_TICKS {
                w.step(&[]);
            }
            click(w, 200, 240, Button::Left);
        });
        assert_eq!(g, Gait::Walk);
        assert!(late < walk, "the second walk order continues the first");
        let (_, g) = ticks_to_arrive(|w| {
            click(w, 200, 240, Button::Left);
            click(w, 200, 240 + DOUBLE_CLICK_DISTANCE + 1, Button::Left);
        });
        assert_eq!(g, Gait::Walk);
        // Exactly at the limits it is still a double click, and a third click starts over.
        let mut w = corridor(12);
        click(&mut w, 80, 240, Button::Left);
        click(&mut w, 200, 240, Button::Left);
        for _ in 0..DOUBLE_CLICK_TICKS - 1 {
            w.step(&[]);
        }
        click(&mut w, 200 + DOUBLE_CLICK_DISTANCE, 240, Button::Left);
        assert_eq!(w.entities[0].gait, Gait::Run);
        assert!(w.last_ground_click.is_none());
        click(&mut w, 200 + DOUBLE_CLICK_DISTANCE, 240, Button::Left);
        assert_eq!(w.entities[0].gait, Gait::Walk);
        assert!(w.last_ground_click.is_some());
    }

    #[test]
    fn left_click_selects_or_orders_and_right_click_cancels_or_deselects() {
        let mut w = corridor(13);
        // A ground click with nothing selected does nothing and remembers nothing.
        click(&mut w, 200, 240, Button::Left);
        assert!(w.selected.is_none() && w.last_ground_click.is_none());
        assert!(w.entities[0].target.is_none());
        click(&mut w, 80, 240, Button::Left);
        assert_eq!(w.selected, Some(w.entities[0].id));
        click(&mut w, 200, 240, Button::Left);
        assert!(w.entities[0].target.is_some());
        // Clicking the guard with the player selected is an attack order (the selection stays,
        // the ground click is forgotten); with nothing selected it selects the guard.
        let (gx, gy) = (w.entities[1].x.round(), w.entities[1].y.round());
        click(&mut w, gx, gy, Button::Left);
        assert_eq!(w.selected, Some(w.entities[0].id));
        assert_eq!(w.entities[0].attack_target, Some(w.entities[1].id));
        assert!(w.last_ground_click.is_none());
        click(&mut w, 300, 400, Button::Right);
        assert!(w.selected.is_none());
        let (gx, gy) = (w.entities[1].x.round(), w.entities[1].y.round());
        click(&mut w, gx, gy, Button::Left);
        assert_eq!(w.selected, Some(w.entities[1].id));
        // A ground order replaces the attack.
        click(&mut w, 80, 240, Button::Left);
        assert!(w.entities[0].attack_target.is_some());
        click(&mut w, 200, 240, Button::Left);
        assert!(w.entities[0].attack_target.is_none());
        // Right click on the ground deselects; the player's order continues.
        click(&mut w, 80, 240, Button::Left);
        click(&mut w, 200, 240, Button::Left);
        click(&mut w, 203, 240, Button::Left);
        assert_eq!(w.entities[0].gait, Gait::Run);
        click(&mut w, 300, 400, Button::Right);
        assert!(w.selected.is_none());
        assert!(w.entities[0].target.is_some());
        // Right click on the selected character cancels his order.
        let (px, py) = (w.entities[0].x.round(), w.entities[0].y.round());
        click(&mut w, px, py, Button::Left);
        assert_eq!(w.selected, Some(w.entities[0].id));
        click(&mut w, px, py, Button::Right);
        assert!(w.entities[0].target.is_none() && w.entities[0].path.is_empty());
        assert_eq!(w.entities[0].gait, Gait::Walk);
        assert_eq!(
            w.selected,
            Some(w.entities[0].id),
            "cancelling keeps the selection"
        );
        w.validate().unwrap();
    }

    #[test]
    fn crouch_and_stand_keys_change_posture_speed_and_animation() {
        use crate::anim::{AnimSet, FrameSpec};
        let frame = |frame| FrameSpec {
            frame,
            duration: 1,
            advance: 0,
            offset_x: 0,
            offset_y: 0,
        };
        let mut catalog = Catalog::default();
        catalog.sets.insert(
            "hero".into(),
            AnimSet {
                run: [2; 8],
                crouch_idle: [3; 8],
                crouch_walk: [4; 8],
                ..AnimSet::standing_only((0..5).map(|i| vec![frame(i)]).collect(), [0; 8], [1; 8])
            },
        );
        let mut w = corridor(14);
        w.attach_catalog(catalog, Some("hero"), None);
        let anim = |w: &World| w.entities[0].anim.as_ref().unwrap().animation;
        w.step(&[]);
        assert_eq!(anim(&w), 0);
        // The keys act on the selected player only.
        w.step(&[InputEvent::KeyDown {
            key: Key::Letter('c'),
        }]);
        assert_eq!(w.entities[0].posture, Posture::Standing);
        click(&mut w, 80, 240, Button::Left);
        w.step(&[
            InputEvent::KeyDown {
                key: Key::Letter('c'),
            },
            InputEvent::KeyUp {
                key: Key::Letter('c'),
            },
        ]);
        assert_eq!(w.entities[0].posture, Posture::Crouched);
        assert_eq!(anim(&w), 3);
        // The blocks of this set carry no advance, so the fallback ratios apply: sneaking at
        // 27 / 128 of the walking speed (364 x 27 / 128 = 77 raw, 0.30 px per tick).
        assert_eq!(
            w.entities[0].effective_speed(&w.catalog),
            Fixed::from_raw(77)
        );
        // Sneaking: the crouched walk block; a double click does not make him run.
        click(&mut w, 200, 240, Button::Left);
        click(&mut w, 200, 240, Button::Left);
        assert_eq!(w.entities[0].gait, Gait::Run);
        assert_eq!(anim(&w), 4);
        let x0 = w.entities[0].x;
        for _ in 0..20 {
            w.step(&[]);
        }
        let moved = (w.entities[0].x - x0).raw();
        assert!((moved - 20 * 77).abs() <= 20, "{moved}");
        // Standing up mid-order: the run order resumes at the running speed and block.
        w.step(&[InputEvent::KeyDown {
            key: Key::Letter('s'),
        }]);
        assert_eq!(w.entities[0].posture, Posture::Standing);
        assert_eq!(anim(&w), 2);
        let x1 = w.entities[0].x;
        for _ in 0..10 {
            w.step(&[]);
        }
        let moved = (w.entities[0].x - x1).raw();
        assert!((moved - 10 * 455).abs() <= 10, "{moved}");
        // Walking uses the walk block.
        click(&mut w, 300, 240, Button::Left);
        assert_eq!(anim(&w), 1);
        // Posture and gait survive a snapshot round trip through JSON and are validated.
        w.step(&[InputEvent::KeyDown {
            key: Key::Letter('c'),
        }]);
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        assert!(json.contains("\"posture\":\"crouched\""));
        assert!(json.contains("\"gait\":\"walk\""));
        let snap: Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = corridor(14);
        w2.attach_catalog(w.catalog.clone(), Some("hero"), None);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.hashes(), w.hashes());
        assert_eq!(w2.entities[0].posture, Posture::Crouched);
        let mut bad = w.snapshot(None);
        bad.world.last_ground_click = Some(GroundClick {
            tick: w.tick + 1,
            x: Fixed::ZERO,
            y: Fixed::ZERO,
        });
        assert!(w2.restore(&bad).unwrap_err().contains("ground click"));
    }
}
