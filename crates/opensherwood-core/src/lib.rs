//! Deterministic simulation core.
//!
//! No I/O, no rendering, no platform types. Everything authoritative lives in [`World`], advances by
//! [`World::step`], is captured by [`World::snapshot`] and hashed by [`World::hashes`]
//! (see `docs/architecture.md`, ADR-0004).
//!
//! Milestone M0 ships a *synthetic* world (no game data) that exercises the whole determinism contract:
//! canonical input, RNG streams, movement, selection, snapshot/restore and hashing.

pub mod fixed;
pub mod hash;
pub mod input;
pub mod rng;
pub mod world;

pub use fixed::Fixed;
pub use hash::Hashes;
pub use input::{Button, InputEvent, Key};
pub use world::{Entity, EntityId, EntityKind, Observation, Scenario, Snapshot, World};

/// Ruleset version: bump when simulation semantics change so old replays/hashes are not compared.
pub const RULESET_VERSION: u32 = 1;
