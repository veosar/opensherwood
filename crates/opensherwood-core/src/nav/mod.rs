//! Navigation: layers, sectors, doors, lifts and the path finder.
//!
//! Work in progress against `docs/original/spec-navigation.md` revision 6. Reviews 37 and 48
//! cleared individual components, not complete mission navigation. Outstanding integration
//! and specification gates are in `docs/decisions/reviews/2026-10-02-claude-navigation-wip.md`.
//! The map's own navigation content - the `STAT` sector polygons per layer,
//! the corner graph with its availability masks, the `FARM` doors and buildings and the ` AZ `
//! lifts - is [`map::NavMap`]; the search, the corner walks, the door route and the click
//! resolution are [`search`]; the dynamic state (the per-sector availability words, the door and
//! building state, the path-request queue and its deterministic schedule) is [`state`].
//!
//! [`grid::NavGrid`] is the **synthetic** engine's 8 px cell grid. It is not a claim about the
//! original: worlds built from a map always carry a [`map::NavMap`] and never use it, and the
//! grid exists only for the synthetic scenarios, which have no map data at all (no sectors, no
//! graph, no doors). `spec-navigation.md` section 11 item 1 requires the grid to go wherever the
//! map decides the path, and it has.
//!
//! **Units.** Positions are map pixels. The static content is integer (`i16` in the files); the
//! search scores use `f32`; geometry has a wider cross-product intermediate. Numeric policy
//! and conversion to 24.8 fixed point still require review against specification 8.5.

pub(crate) mod fixtures;
pub mod grid;
pub mod map;
pub mod search;
pub mod state;

pub use grid::{
    CELL, DEFAULT_SEARCH_WORK, INIT_CELLS_PER_UNIT, MAX_CELLS, MAX_EDGE_TESTS, MAX_POLYGONS,
    NavError, NavGrid,
};
pub use map::{
    ClickAnswer, ClickTarget, DoorSide, NavBuilding, NavContent, NavDoor, NavEdge, NavLift, NavMap,
    NavNode, NavObstacle, NavRecord, NavSector, Wall, WallOwner,
};
pub use search::{
    CORNER_TARGET, CORRIDOR_SHRINK, PathOutcome, SEARCH_WINDOW, SearchInput, SmoothInput,
    UNSTICK_ROUNDS, clearance, corner_walk, corridor_free, quadrant_passes, search_sector,
    smooth_path, unstick,
};
pub use state::{
    ActorClass, DOOR_APPROACH_TOLERANCE, INITIAL_SECTOR_STATE, MAX_ROUTE_DOORS, NavRequest,
    NavState, NavWalk, NavWalkStep, PATH_FAILURE_WINDOW, RequestKey, RequestState, SEARCH_BUDGET,
};
