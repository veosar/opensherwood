//! Deterministic simulation core.
//!
//! No I/O, no rendering, no platform types. Everything authoritative lives in [`World`], advances by
//! [`World::step`], is captured by [`World::snapshot`] and hashed by [`World::hashes`]
//! (see `docs/architecture.md`, ADR-0004).
//!
//! Milestone M0 ships a *synthetic* world (no game data) that exercises the whole determinism contract:
//! canonical input, RNG streams, movement, selection, snapshot/restore and hashing. Missions add the
//! script VM ([`vm`], natives in [`natives`]; ADR-0008) and the stealth and combat layer ([`ai`]:
//! perception, alert states, the knock-out, the melee), which are part of the same contract.

pub mod ai;
pub mod anim;
pub mod fixed;
pub mod geom;
pub mod hash;
pub mod input;
pub mod natives;
pub mod nav;
pub mod rng;
pub mod vm;
pub mod world;

pub use ai::{AiState, FightPose, Figure};
pub use anim::{AnimSet, AnimState, Catalog, FrameSpec, direction_of};
pub use fixed::Fixed;
pub use geom::Geometry;
pub use hash::Hashes;
pub use input::{Button, InputEvent, Key};
pub use nav::{NavError, NavGrid};
pub use vm::{Program, VmState};
pub use world::{
    ActorSpec, DamageNumber, Entity, EntityId, EntityKind, EntityObservation, Gait, GroundClick,
    Instruction, MapInfo, MissionSpec, Observation, Posture, Scenario, Snapshot, Team, World,
};

/// Ruleset version: bump when simulation semantics change so old replays/hashes are not compared.
pub const RULESET_VERSION: u32 = 19;

/// The simulation's tick rate as a rational in Hz: one **logic frame** of 46.875 ms (ADR-0010,
/// `docs/original/spec-script-vm.md` VM-100). 64 frames span exactly 3 s. Everything the
/// behaviour specifications count in frames - script ticks, AI timers, animation timers, camera
/// updates, sequence timers - counts engine ticks one to one; the 64 Hz animation sub-clock and
/// the 25 Hz script sub-clock of the earlier engine are gone, and so are their conversions.
pub const TICK_RATE: (u32, u32) = (64, 3);
