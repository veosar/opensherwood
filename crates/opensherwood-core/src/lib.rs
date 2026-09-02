//! Deterministic simulation core.
//!
//! No I/O, no rendering, no platform types. Everything authoritative lives in [`World`], advances by
//! [`World::step`], is captured by [`World::snapshot`] and hashed by [`World::hashes`]
//! (see `docs/architecture.md`, ADR-0004).
//!
//! Milestone M0 ships a *synthetic* world (no game data) that exercises the whole determinism contract:
//! canonical input, RNG streams, movement, selection, snapshot/restore and hashing. Missions add the
//! script VM ([`vm`], natives in [`natives`]; ADR-0008), which is part of the same contract.

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

pub use anim::{AnimSet, AnimState, Catalog, FrameSpec, direction_of};
pub use fixed::Fixed;
pub use geom::Geometry;
pub use hash::Hashes;
pub use input::{Button, InputEvent, Key};
pub use nav::{NavError, NavGrid};
pub use vm::{Program, VmState};
pub use world::{
    ActorSpec, Entity, EntityId, EntityKind, Instruction, MapInfo, MissionSpec, Observation,
    Scenario, Snapshot, Team, World,
};

/// Ruleset version: bump when simulation semantics change so old replays/hashes are not compared.
pub const RULESET_VERSION: u32 = 7;
