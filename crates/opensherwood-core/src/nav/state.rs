//! The dynamic navigation state: the per-sector availability words, the door and building
//! state, the path-request queue with its deterministic schedule, and the walk sequence a walk
//! order turns into.
//!
//! Specification: `docs/original/spec-navigation.md` 2.9 (NAV-055 to NAV-058), 3.0 (NAV-090),
//! 3.3 to 3.5 (NAV-121, NAV-130, NAV-140, NAV-146, NAV-147), 3.8 to 3.10 (NAV-170 to NAV-200)
//! and the implementation choices 8.1 to 8.4. Everything in [`NavState`] is authoritative:
//! snapshot state, hashed and validated (8.2).

use serde::{Deserialize, Serialize};

use crate::fixed::Fixed;
use crate::hash::Encoder;

use super::map::{BUILDING_CAPACITY, DoorSide, NavMap};
use super::search::{self, Crossing, PathOutcome, SearchInput};

/// The state word every sector starts with after the map is loaded (`spec-navigation.md`
/// NAV-055): sixteen 2-bit fields, each `01`.
pub const INITIAL_SECTOR_STATE: u32 = 0x5555_5555;
/// Tolerance the walk pipeline gives a door-approach test (`spec-navigation.md` section 5).
pub const DOOR_APPROACH_TOLERANCE: i32 = 10;
/// Ticks a failed-pending request waits before the element fails (NAV-146).
pub const PATH_FAILURE_WINDOW: u64 = 100;
/// Work units one search may spend (8.1: a bound, not a measurement).
pub const SEARCH_BUDGET: u64 = 1_000_000;
/// Longest route the door search accepts (NAV-121).
pub use super::search::MAX_ROUTE_DOORS;
/// Frames a character waits before entering a building, unless the door is the first element
/// of the sequence (NAV-130).
pub const BUILDING_ENTRY_WAIT: u32 = 50;
/// Largest number of steps a walk sequence may hold (a route of [`MAX_ROUTE_DOORS`] doors
/// produces at most five steps each, plus the first and last walks).
pub const MAX_WALK_STEPS: usize = MAX_ROUTE_DOORS * 6 + 4;
/// Largest number of requests the queue may hold at once.
pub const MAX_REQUESTS: usize = 1 << 12;
/// Largest number of waypoints a delivered result may carry.
pub const MAX_WAYPOINTS: usize = 1 << 12;

/// The classes the door rules distinguish (`spec-navigation.md` NAV-170, AI section 2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ActorClass {
    /// A player character.
    Player,
    /// A plain soldier.
    #[default]
    PlainSoldier,
    /// A civilian.
    Civilian,
    /// A member of the family that is not the soldier family.
    Class0,
    /// The special class 0x103.
    Special103,
}

impl ActorClass {
    /// Whether the class belongs to the soldier family (player characters, plain soldiers and
    /// civilians).
    #[must_use]
    pub fn soldier_family(self) -> bool {
        matches!(
            self,
            ActorClass::Player | ActorClass::PlainSoldier | ActorClass::Civilian
        )
    }

    /// A stable tag for the canonical hash.
    #[must_use]
    pub fn tag(self) -> u8 {
        match self {
            ActorClass::Player => 0,
            ActorClass::PlainSoldier => 1,
            ActorClass::Civilian => 2,
            ActorClass::Class0 => 3,
            ActorClass::Special103 => 4,
        }
    }
}

/// What the door rules need to know about the character asking to pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Passer {
    /// The class.
    pub class: ActorClass,
    /// A class-0-family character's state, which types 0/3/7/8 and stairs consult.
    pub state: u8,
    /// The lock-picking ability 0x1c.
    pub ability_lockpick: bool,
    /// The climbing ability 0x15.
    pub ability_climb: bool,
    /// The soldier-family "cannot pass" test.
    pub cannot_pass: bool,
}

/// The two ordered sequences a collection of NAV-058 keeps: what the search and the grid see,
/// and what they do not. Both are authoritative state (8.2): the next toggle's outcome depends
/// on the unavailable order as much as on the available one, and the sequences of an
/// unavailable node keep their history until the node is re-enabled.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Sequences {
    /// What the search sees, in order.
    pub available: Vec<u32>,
    /// What it does not, in order.
    pub unavailable: Vec<u32>,
}

impl Sequences {
    fn check_partition(&self, expected: &[u32]) -> Result<(), String> {
        if self.total() != expected.len() {
            return Err("navigation collection has the wrong length".into());
        }
        let mut actual: Vec<_> = self
            .available
            .iter()
            .chain(&self.unavailable)
            .copied()
            .collect();
        actual.sort_unstable();
        let mut expected = expected.to_vec();
        expected.sort_unstable();
        if actual != expected {
            return Err("navigation collection has foreign or duplicate indices".into());
        }
        Ok(())
    }
    /// Everything the pair holds, for a validity check.
    fn total(&self) -> usize {
        self.available.len() + self.unavailable.len()
    }

    /// One **recomputation** (`spec-navigation.md` NAV-058): the available members that are no
    /// longer available move to the end of the unavailable sequence, then the unavailable
    /// members that are available again move to the end of the available sequence. Nodes make
    /// both moves in reverse order, edges and obstacles in forward order.
    fn recompute(&mut self, reverse: bool, ok: &dyn Fn(u32) -> bool) {
        let mut out: Vec<u32> = self.available.iter().copied().filter(|&x| !ok(x)).collect();
        self.available.retain(|&x| ok(x));
        if reverse {
            out.reverse();
        }
        self.unavailable.extend(out);
        let mut back: Vec<u32> = self
            .unavailable
            .iter()
            .copied()
            .filter(|&x| ok(x))
            .collect();
        self.unavailable.retain(|&x| !ok(x));
        if reverse {
            back.reverse();
        }
        self.available.extend(back);
    }
}

/// The availability of a whole map: the per-sector state words and the ordered sequences the
/// search consumes (`spec-navigation.md` NAV-055 to NAV-058). OpenSherwood reproduces the
/// historical orders rather than the canonical file order proposed in 8.3. Review 48 cleared
/// that proposal for maintainer consideration; it did not approve or reject it on their behalf.
/// Both sequences of every collection are snapshot state in this implementation.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Availability {
    /// One 32-bit state word per sector, by flat sector index (NAV-055).
    pub sector_state: Vec<u32>,
    /// `nodes[sector][region]`: the region's nodes, by flat node index.
    pub nodes: Vec<Vec<Sequences>>,
    /// `edges[node]`: the edges entering that node, by edge index.
    pub edges: Vec<Sequences>,
    /// `obstacles[sector]`: the sector's obstacles, by their position in the sector.
    pub obstacles: Vec<Sequences>,
}

impl Availability {
    /// The state a freshly loaded map starts in: every sequence in file order, then the initial
    /// recomputation of every sector against `0x55555555` (NAV-055, NAV-058).
    #[must_use]
    pub fn new(map: &NavMap) -> Self {
        let mut a = Availability {
            sector_state: vec![INITIAL_SECTOR_STATE; map.sectors.len()],
            nodes: map
                .sectors
                .iter()
                .map(|s| {
                    map.node_index[s.layer as usize][s.position as usize]
                        .iter()
                        .map(|region| Sequences {
                            available: region.clone(),
                            unavailable: Vec::new(),
                        })
                        .collect()
                })
                .collect(),
            edges: map
                .nodes
                .iter()
                .map(|n| Sequences {
                    available: n.entering_edges.clone(),
                    unavailable: Vec::new(),
                })
                .collect(),
            obstacles: map
                .sectors
                .iter()
                .map(|s| Sequences {
                    available: (0..s.obstacles.len() as u32).collect(),
                    unavailable: Vec::new(),
                })
                .collect(),
        };
        for sector in 0..map.sectors.len() as u32 {
            a.recompute(map, sector);
        }
        a
    }

    /// The state word of a sector.
    #[must_use]
    pub fn state(&self, sector: u32) -> u32 {
        self.sector_state.get(sector as usize).copied().unwrap_or(0)
    }

    /// Whether an object with this mask is available in a sector (NAV-056).
    #[must_use]
    pub fn available(&self, sector: u32, mask: u32) -> bool {
        super::search::available(mask, self.state(sector))
    }

    /// The nodes of a region the search sees, in order (NAV-058).
    #[must_use]
    pub fn region_nodes(&self, sector: u32, region: u32) -> &[u32] {
        self.nodes
            .get(sector as usize)
            .and_then(|s| s.get(region as usize))
            .map_or(&[][..], |q| &q.available[..])
    }

    /// How many regions a sector has.
    #[must_use]
    pub fn region_count(&self, sector: u32) -> usize {
        self.nodes.get(sector as usize).map_or(0, Vec::len)
    }

    /// The edges entering a node that the search sees, in order (NAV-058).
    #[must_use]
    pub fn node_edges(&self, node: u32) -> &[u32] {
        self.edges
            .get(node as usize)
            .map_or(&[][..], |q| &q.available[..])
    }

    /// The enabled obstacles of a sector, in order (NAV-056: a disabled obstacle's polygon
    /// leaves the cells and its wall segments stop blocking).
    #[must_use]
    pub fn sector_obstacles(&self, sector: u32) -> &[u32] {
        self.obstacles
            .get(sector as usize)
            .map_or(&[][..], |q| &q.available[..])
    }

    /// Recompute one sector's sequences against its state word (`spec-navigation.md` NAV-058).
    /// A node's edge sequences are transformed only in a recomputation under which the node
    /// itself is available; while it is unavailable both of its sequences keep their history.
    pub fn recompute(&mut self, map: &NavMap, sector: u32) {
        let s = self.state(sector);
        let node_ok = |n: u32| {
            map.nodes
                .get(n as usize)
                .is_some_and(|x| super::search::available(x.mask, s))
        };
        if let Some(regions) = self.nodes.get_mut(sector as usize) {
            for region in regions.iter_mut() {
                region.recompute(true, &node_ok);
            }
        }
        let all_nodes: Vec<u32> = self
            .nodes
            .get(sector as usize)
            .map(|regions| {
                regions
                    .iter()
                    .flat_map(|q| q.available.iter().chain(q.unavailable.iter()).copied())
                    .collect()
            })
            .unwrap_or_default();
        let edge_ok = |e: u32| {
            map.edges
                .get(e as usize)
                .is_some_and(|x| super::search::available(x.mask, s))
        };
        for n in all_nodes {
            if !node_ok(n) {
                continue;
            }
            if let Some(q) = self.edges.get_mut(n as usize) {
                q.recompute(false, &edge_ok);
            }
        }
        let obstacle_ok = |o: u32| {
            map.sectors
                .get(sector as usize)
                .and_then(|x| x.obstacles.get(o as usize))
                .is_some_and(|x| super::search::available(x.mask, s))
        };
        if let Some(q) = self.obstacles.get_mut(sector as usize) {
            q.recompute(false, &obstacle_ok);
        }
    }

    /// Whether the sequences match the map (`World::validate`).
    pub fn check(&self, map: &NavMap) -> Result<(), String> {
        if self.sector_state.len() != map.sectors.len()
            || self.nodes.len() != map.sectors.len()
            || self.obstacles.len() != map.sectors.len()
            || self.edges.len() != map.nodes.len()
        {
            return Err("navigation availability does not match the map".into());
        }
        for (si, regions) in self.nodes.iter().enumerate() {
            let sec = &map.sectors[si];
            let want = map.node_index[sec.layer as usize][sec.position as usize].len();
            if regions.len() != want {
                return Err(format!(
                    "sector {si} has {} region sequences",
                    regions.len()
                ));
            }
            for (ri, q) in regions.iter().enumerate() {
                q.check_partition(&map.node_index[sec.layer as usize][sec.position as usize][ri])?;
                if q.available
                    .iter()
                    .any(|&n| !self.available(si as u32, map.nodes[n as usize].mask))
                    || q.unavailable
                        .iter()
                        .any(|&n| self.available(si as u32, map.nodes[n as usize].mask))
                {
                    return Err("navigation node partition contradicts its mask".into());
                }
            }
        }
        for (ni, q) in self.edges.iter().enumerate() {
            q.check_partition(&map.nodes[ni].entering_edges)?;
        }
        for (si, q) in self.obstacles.iter().enumerate() {
            q.check_partition(&(0..map.sectors[si].obstacles.len() as u32).collect::<Vec<_>>())?;
            if q.available
                .iter()
                .any(|&o| !self.available(si as u32, map.sectors[si].obstacles[o as usize].mask))
                || q.unavailable
                    .iter()
                    .any(|&o| self.available(si as u32, map.sectors[si].obstacles[o as usize].mask))
            {
                return Err("navigation obstacle partition contradicts its mask".into());
            }
        }
        Ok(())
    }
}

/// Live state of one door (`spec-navigation.md` NAV-040, NAV-172).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoorState {
    /// Whether the door is open.
    pub open: bool,
    /// The active lock set: player, lock-pick, civilian, soldier (file order).
    pub lock: [u8; 4],
    /// The alternate lock set a patch swaps in.
    pub alt: [u8; 4],
    /// The run-time "player barred" byte (NAV-170 rule 2).
    pub player_barred: bool,
    /// Whether the leaf's click target is enabled (native 191).
    pub leaf_enabled: bool,
}

/// Live state of one building (`spec-navigation.md` 3.9).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BuildingState {
    /// The occupants, in the order they entered (entity slot indices).
    pub occupants: Vec<u32>,
    /// The occupants whose "displayed" mark is set (NAV-190 / NAV-191).
    pub displayed: Vec<u32>,
}

/// The state of a path request (`spec-navigation.md` NAV-090, 8.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RequestState {
    /// Waiting for a dispatch slot.
    Queued,
    /// Searched; the waypoints are delivered at `delivery`.
    Completed {
        /// The waypoints, `start` first and `goal` last (NAV-145).
        waypoints: Vec<(Fixed, Fixed)>,
        /// The tick at which the consumer slot takes them.
        delivery: u64,
    },
    /// An empty result: the element fails after the failure window (NAV-146).
    FailedPending {
        /// The tick the empty result arrived at.
        arrival: u64,
    },
}

/// One path request (`spec-navigation.md` NAV-140, 8.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavRequest {
    /// Identity, unique within a mission.
    pub id: u64,
    /// The element that owns the request (an entity slot index).
    pub owner: u32,
    /// The layer the search runs on.
    pub layer: u32,
    /// The polygon number the request names (NAV-014).
    pub sector_number: u32,
    /// The flat sector index the number resolved to.
    pub sector: u32,
    /// Where the walk starts (the corrected start when `unstuck`).
    pub start: (Fixed, Fixed),
    /// Where it must arrive.
    pub goal: (Fixed, Fixed),
    /// The gait tag the delivered move actions carry.
    pub gait: u8,
    /// The run flag of the move actions.
    pub run: bool,
    /// Priority 0 for player characters, 1 for others (NAV-140); `NavReferenceBox` keeps it.
    pub priority: u8,
    /// The unstuck flag (NAV-140 step 3, NAV-145).
    pub unstuck: bool,
    /// The tick the request was submitted at.
    pub submitted: u64,
    /// Its state.
    pub state: RequestState,
}

/// A stable key for ordering the queue: priority first, then submission order
/// (`spec-navigation.md` NAV-147, stable for equal priority).
pub type RequestKey = (u8, u64);

impl NavRequest {
    /// The ordering key of NAV-147.
    #[must_use]
    pub fn key(&self) -> RequestKey {
        (self.priority, self.id)
    }
}

/// One step of the walk sequence a walk order turns into (`spec-navigation.md` NAV-130).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavWalkStep {
    /// A walk to a point on a sector of a layer (NAV-140).
    Walk {
        /// Where to.
        to: (Fixed, Fixed),
        /// The flat sector index the walk runs in.
        sector: u32,
        /// The layer.
        layer: u32,
    },
    /// The door-approach test in its point form (NAV-150a).
    ApproachPoint {
        /// The point.
        at: (Fixed, Fixed),
        /// The tolerance; the test passes below `tolerance + 5` px by the Chebyshev rule.
        tolerance: i32,
    },
    /// The door-approach test in its sector form.
    ApproachSector {
        /// The flat sector index the character must be in.
        sector: u32,
    },
    /// A wait of a number of frames (the building entry of NAV-130).
    Wait {
        /// Frames left.
        frames: u32,
    },
    /// Entering a building at a point, facing the door (NAV-130, 3.9).
    Enter {
        /// The door.
        door: u32,
        /// The near point.
        at: (Fixed, Fixed),
    },
    /// Crossing a door or a lift end (NAV-171, 3.10).
    PassDoor {
        /// The door.
        door: u32,
        /// The side the character comes from.
        from: DoorSide,
    },
    /// Picking a lock: the sequence ends here (NAV-130 step 3).
    PickLock {
        /// The door.
        door: u32,
        /// The far point the element plays at.
        at: (Fixed, Fixed),
    },
}

/// The walk sequence of one character (`spec-navigation.md` NAV-130), with the step it is on.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NavWalk {
    /// The steps, in order.
    pub steps: Vec<NavWalkStep>,
    /// The step being executed.
    pub cursor: u32,
    /// The gait tag the walks use.
    pub gait: u8,
    /// The run flag.
    pub run: bool,
    /// Whether the current `Walk` step has already submitted its request.
    pub submitted: bool,
}

impl NavWalk {
    /// The step being executed, if any.
    #[must_use]
    pub fn current(&self) -> Option<NavWalkStep> {
        self.steps.get(self.cursor as usize).copied()
    }

    /// Whether the sequence has run out of steps.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.cursor as usize >= self.steps.len()
    }
}

/// The whole dynamic navigation state of a mission.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NavState {
    /// The availability of every sector: its state word and the ordered sequences the search
    /// consumes (NAV-055 to NAV-058).
    pub avail: Availability,
    /// One entry per door of the flat door list.
    pub doors: Vec<DoorState>,
    /// One entry per building.
    pub buildings: Vec<BuildingState>,
    /// The request queue, in submission order (NAV-147 orders it when the head is taken).
    pub queue: Vec<NavRequest>,
    /// The request that has been searched and not yet consumed (at most one: 8.1 dispatches
    /// one per tick and never while one is completed).
    pub completed: Option<NavRequest>,
    /// The requests whose result was empty, waiting out the failure window (NAV-146).
    pub failed: Vec<NavRequest>,
    /// The id the next request takes.
    pub next_id: u64,
    /// The largest number of work units one search of this mission consumed (8.1: the harness
    /// asserts that no retail map exhausts the budget). Not hashed.
    pub max_search_work: u64,
    /// The work budget one search may spend; [`SEARCH_BUDGET`] unless a test lowers it.
    pub search_budget: u64,
    /// Entity index at which the bounded sequence phase resumes.
    pub drive_cursor: u32,
}

impl NavState {
    /// Work charged before a callback journal copies this state.
    pub(crate) fn capture_cost(&self) -> u64 {
        let sequences = self
            .avail
            .nodes
            .iter()
            .flatten()
            .chain(self.avail.edges.iter())
            .chain(self.avail.obstacles.iter());
        let ordered: usize = sequences.map(|s| 1 + s.total()).sum();
        let occupants: usize = self
            .buildings
            .iter()
            .map(|b| 1 + b.occupants.len() + b.displayed.len())
            .sum();
        let requests: usize = self
            .queue
            .iter()
            .chain(&self.completed)
            .chain(&self.failed)
            .map(|r| {
                16 + match &r.state {
                    RequestState::Completed { waypoints, .. } => waypoints.len() * 2,
                    _ => 0,
                }
            })
            .sum();
        (ordered + occupants + requests + self.doors.len() * 12 + self.avail.sector_state.len() + 8)
            as u64
    }

    /// The state a freshly loaded map starts in (NAV-055, NAV-040).
    #[must_use]
    pub fn new(map: &NavMap) -> Self {
        NavState {
            avail: Availability::new(map),
            doors: map
                .doors
                .iter()
                .map(|d| DoorState {
                    open: d.initially_open,
                    lock: d.lock,
                    alt: d.alt,
                    player_barred: false,
                    leaf_enabled: !d.leaf.is_empty(),
                })
                .collect(),
            buildings: map
                .buildings
                .iter()
                .map(|b| BuildingState {
                    occupants: b.tenants.clone(),
                    displayed: Vec::new(),
                })
                .collect(),
            queue: Vec::new(),
            completed: None,
            failed: Vec::new(),
            next_id: 1,
            max_search_work: 0,
            search_budget: SEARCH_BUDGET,
            drive_cursor: 0,
        }
    }

    /// Whether the state matches the map (`World::validate`).
    pub fn check(&self, map: &NavMap) -> Result<(), String> {
        self.avail.check(map)?;
        if self.search_budget > SEARCH_BUDGET || self.max_search_work > SEARCH_BUDGET {
            return Err("navigation search budget exceeds the limit".into());
        }
        if self.doors.len() != map.doors.len() {
            return Err(format!(
                "navigation state has {} doors, the map has {}",
                self.doors.len(),
                map.doors.len()
            ));
        }
        if self.buildings.len() != map.buildings.len() {
            return Err(format!(
                "navigation state has {} buildings, the map has {}",
                self.buildings.len(),
                map.buildings.len()
            ));
        }
        if self.queue.len() + self.failed.len() + usize::from(self.completed.is_some())
            > MAX_REQUESTS
        {
            return Err("navigation request queue too long".into());
        }
        if self.queue.iter().any(|r| r.state != RequestState::Queued)
            || self
                .failed
                .iter()
                .any(|r| !matches!(r.state, RequestState::FailedPending { .. }))
            || self
                .completed
                .as_ref()
                .is_some_and(|r| !matches!(r.state, RequestState::Completed { .. }))
        {
            return Err("navigation request is in the wrong queue".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut owners = std::collections::BTreeSet::new();
        for r in self
            .queue
            .iter()
            .chain(self.completed.iter())
            .chain(self.failed.iter())
        {
            if !ids.insert(r.id) || !owners.insert(r.owner) || r.id == 0 || r.id >= self.next_id {
                return Err("navigation request identity is invalid".into());
            }
            if r.sector as usize >= map.sectors.len() {
                return Err(format!("navigation request names sector {}", r.sector));
            }
            let sector = &map.sectors[r.sector as usize];
            if r.layer != sector.layer
                || r.sector_number != sector.number
                || r.gait > 1
                || r.priority > 1
            {
                return Err("navigation request qualifiers are invalid".into());
            }
            if let RequestState::Completed { waypoints, .. } = &r.state
                && (waypoints.is_empty() || waypoints.len() > MAX_WAYPOINTS)
            {
                return Err("navigation result too long".into());
            }
        }
        Ok(())
    }

    /// Whether the object with this mask is available in the sector (NAV-056).
    #[must_use]
    pub fn available(&self, sector: u32, mask: u32) -> bool {
        self.avail.available(sector, mask)
    }

    /// Whether an obstacle is enabled (NAV-056): the predicate the click resolution and the
    /// wall tests use.
    #[must_use]
    pub fn obstacle_enabled(&self, map: &NavMap, sector: u32, obstacle: u32) -> bool {
        map.sectors
            .get(sector as usize)
            .and_then(|s| s.obstacles.get(obstacle as usize))
            .is_some_and(|o| self.available(sector, o.mask))
    }

    /// Toggle field `k` of a sector between `01` and `10` and recompute that sector's
    /// availability (`spec-navigation.md` NAV-057, NAV-058).
    pub fn toggle_field(&mut self, map: &NavMap, sector: u32, field: u32) {
        if field >= 16 {
            return;
        }
        if let Some(s) = self.avail.sector_state.get_mut(sector as usize) {
            let shift = field * 2;
            let cur = (*s >> shift) & 0b11;
            let next = match cur {
                0b01 => 0b10,
                0b10 => 0b01,
                other => other,
            };
            *s = (*s & !(0b11 << shift)) | (next << shift);
        }
        self.avail.recompute(map, sector);
    }

    // ----------------------------------------------------------------------------------------
    // Doors
    // ----------------------------------------------------------------------------------------

    /// The lock byte a native id addresses: 182 / 186 the player lock, 183 / 187 the lock-pick
    /// byte, 184 / 188 the soldier lock, 185 / 189 the civilian lock. The file order is
    /// player, lock-pick, civilian, soldier (`spec-navigation.md` 2.6), so the two orders
    /// differ in the last two entries.
    #[must_use]
    pub fn lock_slot(offset: u32) -> usize {
        match offset {
            0 => 0,
            1 => 1,
            2 => 3,
            _ => 2,
        }
    }

    /// Set a lock byte; clearing the player, soldier or civilian lock also **opens** the door
    /// (`spec-navigation.md` NAV-172).
    pub fn set_lock(&mut self, door: u32, offset: u32, value: u8) {
        let slot = Self::lock_slot(offset);
        if let Some(d) = self.doors.get_mut(door as usize) {
            d.lock[slot] = value;
            if value == 0 && offset != 1 {
                d.open = true;
            }
        }
    }

    /// Swap the active lock set with the alternate one (NAV-172; only a patch does this, and
    /// patches are inert under `NavPatches`).
    pub fn swap_lock_set(&mut self, door: u32) {
        if let Some(d) = self.doors.get_mut(door as usize) {
            std::mem::swap(&mut d.lock, &mut d.alt);
        }
    }

    /// **Admission** (`spec-navigation.md` NAV-170): whether `who` may pass `door` from `side`.
    /// `crossing` selects the crossing form, which additionally refuses type 8.
    #[must_use]
    pub fn admits(
        &self,
        map: &NavMap,
        door: u32,
        side: DoorSide,
        who: &Passer,
        crossing: bool,
    ) -> bool {
        self.admits_with_open(map, door, side, who, crossing, false)
    }

    /// Planning a leaf click may waive only the open flag; no state clone or mutation is needed.
    #[must_use]
    pub fn admits_with_open(
        &self,
        map: &NavMap,
        door: u32,
        side: DoorSide,
        who: &Passer,
        crossing: bool,
        waive_open: bool,
    ) -> bool {
        let (Some(d), Some(s)) = (map.doors.get(door as usize), self.doors.get(door as usize))
        else {
            return false;
        };
        if crossing && d.door_type == 8 {
            return false;
        }
        if s.lock[0] != 0 && s.player_barred && who.class == ActorClass::Player {
            return false;
        }
        if !s.open && !waive_open {
            return false;
        }
        let (lock_pc, lock_x, lock_civilian, lock_soldier) =
            (s.lock[0], s.lock[1], s.lock[2], s.lock[3]);
        match d.door_type {
            0 | 3 | 7 | 8 => match who.class {
                ActorClass::Player => lock_pc == 0 || (who.ability_lockpick && lock_x != 0),
                ActorClass::PlainSoldier | ActorClass::Special103 => lock_soldier == 0,
                ActorClass::Civilian => lock_civilian == 0,
                ActorClass::Class0 => who.state == 7 && lock_soldier == 0,
            },
            1 | 2 => {
                if who.class == ActorClass::Player {
                    return lock_pc == 0 || (who.ability_lockpick && lock_x != 0);
                }
                if !who.class.soldier_family() {
                    return false;
                }
                if side == DoorSide::A {
                    let full = d
                        .building
                        .and_then(|b| self.buildings.get(b as usize))
                        .is_some_and(|b| b.occupants.len() as u32 >= BUILDING_CAPACITY);
                    if full {
                        return false;
                    }
                }
                match who.class {
                    // A type-2 door always refuses a plain soldier.
                    ActorClass::PlainSoldier => d.door_type == 1 && lock_soldier == 0,
                    ActorClass::Civilian => !who.cannot_pass && lock_civilian == 0,
                    _ => false,
                }
            }
            4..=6 => {
                if who.class.soldier_family() && who.cannot_pass {
                    return false;
                }
                let kind = d
                    .lift
                    .and_then(|l| map.lifts.get(l as usize))
                    .map_or(BUILDING_CAPACITY, |l| u32::from(l.lift_type));
                lift_admits(kind, who)
            }
            _ => false,
        }
    }

    // ----------------------------------------------------------------------------------------
    // Buildings
    // ----------------------------------------------------------------------------------------

    /// Put an entity inside a building (`spec-navigation.md` NAV-190): the caller has already
    /// moved it; this records the occupancy.
    pub fn put_inside(&mut self, building: u32, entity: u32) {
        if let Some(b) = self.buildings.get_mut(building as usize)
            && !b.occupants.contains(&entity)
        {
            b.occupants.push(entity);
        }
    }

    /// Take an entity out of a building (NAV-191); returns whether it was inside.
    pub fn take_out(&mut self, building: u32, entity: u32) -> bool {
        let Some(b) = self.buildings.get_mut(building as usize) else {
            return false;
        };
        let before = b.occupants.len();
        b.occupants.retain(|&e| e != entity);
        b.displayed.retain(|&e| e != entity);
        b.occupants.len() != before
    }

    /// The building an entity is inside, if any.
    #[must_use]
    pub fn building_of(&self, entity: u32) -> Option<u32> {
        self.buildings
            .iter()
            .position(|b| b.occupants.contains(&entity))
            .map(|i| i as u32)
    }

    // ----------------------------------------------------------------------------------------
    // Requests and the schedule (8.1)
    // ----------------------------------------------------------------------------------------

    /// Submit a request (P3 of tick `tick`); it is dispatched at P2 of `tick + 1` at the
    /// earliest. Returns its id, or `None` when the queue is full.
    pub fn submit(&mut self, mut r: NavRequest, tick: u64) -> Option<u64> {
        if self.queue.len() + self.failed.len() + usize::from(self.completed.is_some())
            >= MAX_REQUESTS
            || self.has_request(r.owner)
            || self.next_id == u64::MAX
        {
            return None;
        }
        r.id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        r.submitted = tick;
        r.state = RequestState::Queued;
        let id = r.id;
        self.queue.push(r);
        Some(id)
    }

    /// Remove every request of an owner, whatever its state (`spec-navigation.md` NAV-090:
    /// cancelling an element removes its request).
    pub fn cancel(&mut self, owner: u32) {
        self.queue.retain(|r| r.owner != owner);
        self.failed.retain(|r| r.owner != owner);
        if self.completed.as_ref().is_some_and(|r| r.owner == owner) {
            self.completed = None;
        }
    }

    /// Whether an owner has a request in any state.
    #[must_use]
    pub fn has_request(&self, owner: u32) -> bool {
        self.queue.iter().any(|r| r.owner == owner)
            || self.failed.iter().any(|r| r.owner == owner)
            || self.completed.as_ref().is_some_and(|r| r.owner == owner)
    }

    /// **P1, the consumer slot** (8.1): the completed request whose delivery tick is at most
    /// `tick`, removed from the state. The caller turns its waypoints into move actions
    /// (NAV-146).
    pub fn consume(&mut self, tick: u64) -> Option<NavRequest> {
        let ready = matches!(
            self.completed.as_ref().map(|r| &r.state),
            Some(RequestState::Completed { delivery, .. }) if *delivery <= tick
        );
        ready.then(|| self.completed.take()).flatten()
    }

    /// The failed-pending requests whose window has run out at `tick` (NAV-146: the element
    /// fails at the first consumer slot of a tick `t > arrival + 100`), removed from the state.
    pub fn take_expired_failures(&mut self, tick: u64) -> Vec<NavRequest> {
        let mut out = Vec::new();
        self.failed.retain(|r| {
            let expired = matches!(
                r.state,
                RequestState::FailedPending { arrival } if tick > arrival.saturating_add(PATH_FAILURE_WINDOW)
            );
            if expired {
                out.push(r.clone());
            }
            !expired
        });
        out
    }

    /// **P2, the dispatch** (8.1): with no completed request pending and a non-empty queue,
    /// order the queue (NAV-147), take the head, run its search to completion under the work
    /// budget and mark it completed with delivery tick `tick + 1`.
    pub fn dispatch(&mut self, map: &NavMap, tick: u64) {
        if self.completed.is_some() || self.queue.is_empty() {
            return;
        }
        self.dispatch_head(map, tick.saturating_add(1));
    }

    /// Dispatch the head at once with the given delivery tick: what a patch does (8.1) and what
    /// [`NavState::dispatch`] does with `tick + 1`.
    pub fn dispatch_head(&mut self, map: &NavMap, delivery: u64) {
        if self.queue.is_empty() {
            return;
        }
        // NAV-147: priority 0 first, stable for equal priority. `NavReferenceBox` keeps every
        // non-player request at priority 1, so no promotion or demotion happens.
        let head = self
            .queue
            .iter()
            .enumerate()
            .min_by_key(|(_, r)| r.key())
            .map_or(0, |(i, _)| i);
        let mut r = self.queue.remove(head);
        let mut budget = self.search_budget;
        let outcome = search::search_sector(
            map,
            &self.avail,
            SearchInput {
                layer: r.layer,
                sector: r.sector,
                start: (r.start.0.to_f32(), r.start.1.to_f32()),
                goal: (r.goal.0.to_f32(), r.goal.1.to_f32()),
                class: 0,
                unstuck: r.unstuck,
            },
            &mut budget,
        );
        let spent = self.search_budget.saturating_sub(budget);
        self.max_search_work = self.max_search_work.max(spent);
        match outcome {
            PathOutcome::Path(points) if !points.is_empty() && points.len() <= MAX_WAYPOINTS => {
                // NAV-145: the consumer skips `start` when the unstuck flag is 0.
                let skip = usize::from(!r.unstuck);
                let waypoints: Vec<(Fixed, Fixed)> = points
                    .iter()
                    .skip(skip)
                    .map(|&(x, y)| (Fixed::from_f32(x), Fixed::from_f32(y)))
                    .collect();
                if waypoints.is_empty() {
                    r.state = RequestState::FailedPending { arrival: delivery };
                    self.failed.push(r);
                } else {
                    r.state = RequestState::Completed {
                        waypoints,
                        delivery,
                    };
                    self.completed = Some(r);
                }
            }
            _ => {
                r.state = RequestState::FailedPending { arrival: delivery };
                self.failed.push(r);
            }
        }
    }

    /// **Applying a patch** (`spec-navigation.md` NAV-057 and 8.1): toggle the named field,
    /// recompute the availability and re-run the search. A completed request returns to the
    /// queue head and is dispatched again at once with its delivery tick unchanged; with only
    /// queued work the head is dispatched at once with delivery `tick + 1`.
    ///
    /// Patch **records** are inert (`Assumption::NavPatches`), so nothing in a retail mission
    /// reaches this; the machinery exists so that the save format and the schedule do not
    /// change when `TUPO` is read.
    pub fn apply_patch(&mut self, map: &NavMap, sector: u32, field: u32, tick: u64) {
        self.toggle_field(map, sector, field);
        if let Some(mut r) = self.completed.take() {
            let delivery = match r.state {
                RequestState::Completed { delivery, .. } => delivery,
                _ => tick.saturating_add(1),
            };
            r.state = RequestState::Queued;
            self.queue.insert(0, r);
            self.dispatch_head(map, delivery);
        } else if !self.queue.is_empty() {
            self.dispatch_head(map, tick.saturating_add(1));
        }
    }
}

impl NavWalkStep {
    /// A stable tag for the canonical hash.
    #[must_use]
    pub fn tag(&self) -> u8 {
        match self {
            NavWalkStep::Walk { .. } => 0,
            NavWalkStep::ApproachPoint { .. } => 1,
            NavWalkStep::ApproachSector { .. } => 2,
            NavWalkStep::Wait { .. } => 3,
            NavWalkStep::Enter { .. } => 4,
            NavWalkStep::PassDoor { .. } => 5,
            NavWalkStep::PickLock { .. } => 6,
        }
    }

    /// Encode the step for the canonical hash.
    pub fn encode(&self, e: &mut Encoder) {
        e.u8(self.tag());
        match *self {
            NavWalkStep::Walk { to, sector, layer } => {
                e.i32(to.0.raw()).i32(to.1.raw()).u32(sector).u32(layer)
            }
            NavWalkStep::ApproachPoint { at, tolerance } => {
                e.i32(at.0.raw()).i32(at.1.raw()).i32(tolerance)
            }
            NavWalkStep::ApproachSector { sector } => e.u32(sector),
            NavWalkStep::Wait { frames } => e.u32(frames),
            NavWalkStep::Enter { door, at } | NavWalkStep::PickLock { door, at } => {
                e.u32(door).i32(at.0.raw()).i32(at.1.raw())
            }
            NavWalkStep::PassDoor { door, from } => e.u32(door).u8(from.tag()),
        };
    }
}

impl NavRequest {
    /// Encode the request for the canonical hash (8.2: inputs, state and the delivered list).
    pub fn encode(&self, e: &mut Encoder) {
        e.u64(self.id)
            .u32(self.owner)
            .u32(self.layer)
            .u32(self.sector_number)
            .u32(self.sector)
            .i32(self.start.0.raw())
            .i32(self.start.1.raw())
            .i32(self.goal.0.raw())
            .i32(self.goal.1.raw())
            .u8(self.gait)
            .u8(u8::from(self.run))
            .u8(self.priority)
            .u8(u8::from(self.unstuck))
            .u64(self.submitted);
        match &self.state {
            RequestState::Queued => e.u8(0),
            RequestState::Completed {
                waypoints,
                delivery,
            } => {
                e.u8(1).u64(*delivery).u32(waypoints.len() as u32);
                for (x, y) in waypoints {
                    e.i32(x.raw()).i32(y.raw());
                }
                e
            }
            RequestState::FailedPending { arrival } => e.u8(2).u64(*arrival),
        };
    }
}

impl Sequences {
    /// Encode both sequences for the canonical hash: the unavailable order decides the next
    /// toggle's outcome as much as the available one (8.2).
    pub fn encode(&self, e: &mut Encoder) {
        for seq in [&self.available, &self.unavailable] {
            e.u32(seq.len() as u32);
            for &v in seq {
                e.u32(v);
            }
        }
    }
}

impl NavState {
    /// Encode the whole navigation state for the canonical hash (`spec-navigation.md` 8.2).
    pub fn encode(&self, e: &mut Encoder) {
        e.u32(self.avail.sector_state.len() as u32);
        for &w in &self.avail.sector_state {
            e.u32(w);
        }
        e.u32(self.avail.nodes.len() as u32);
        for sector in &self.avail.nodes {
            e.u32(sector.len() as u32);
            for region in sector {
                region.encode(e);
            }
        }
        e.u32(self.avail.edges.len() as u32);
        for q in &self.avail.edges {
            q.encode(e);
        }
        e.u32(self.avail.obstacles.len() as u32);
        for q in &self.avail.obstacles {
            q.encode(e);
        }
        e.u32(self.doors.len() as u32);
        for d in &self.doors {
            e.u8(u8::from(d.open));
            for b in d.lock.iter().chain(d.alt.iter()) {
                e.u8(*b);
            }
            e.u8(u8::from(d.player_barred)).u8(u8::from(d.leaf_enabled));
        }
        e.u32(self.buildings.len() as u32);
        for b in &self.buildings {
            e.u32(b.occupants.len() as u32);
            for &o in &b.occupants {
                e.u32(o);
            }
            e.u32(b.displayed.len() as u32);
            for &o in &b.displayed {
                e.u32(o);
            }
        }
        e.u32(self.queue.len() as u32);
        for r in &self.queue {
            r.encode(e);
        }
        match &self.completed {
            Some(r) => {
                e.u8(1);
                r.encode(e);
            }
            None => {
                e.u8(0);
            }
        }
        e.u32(self.failed.len() as u32);
        for r in &self.failed {
            r.encode(e);
        }
        e.u64(self.next_id)
            .u64(self.search_budget)
            .u32(self.drive_cursor);
    }
}

/// The admission rule of a lift kind, and of a building's 65535
/// (`spec-navigation.md` NAV-192).
#[must_use]
pub fn lift_admits(kind: u32, who: &Passer) -> bool {
    match kind {
        1 => who.class != ActorClass::Class0 || who.state == 1,
        2 => who.class.soldier_family() && who.class != ActorClass::PlainSoldier,
        3 => who.class == ActorClass::Player && who.ability_climb,
        _ => true,
    }
}

/// Build the walk sequence of NAV-130 for a character standing at `from` on
/// `(from_sector, from_layer)`, walking to `target` on `(target_sector, target_layer)`.
///
/// `lift_end` is the lift end the character is standing on, if any (step 1); `admits` answers
/// the planning form of NAV-170; `waits` supplies the two random draws of the building entry
/// in the order the sequence is built. Returns `None` when no route exists.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
#[must_use]
pub fn build_walk_sequence(
    map: &NavMap,
    from: (Fixed, Fixed),
    from_sector: u32,
    from_layer: u32,
    target: (Fixed, Fixed),
    target_sector: u32,
    lift_end: Option<(u32, DoorSide)>,
    can_pick_lock: Option<&Passer>,
    admits: &dyn Fn(u32, DoorSide) -> bool,
    waits: &mut dyn FnMut() -> u32,
    budget: &mut u64,
) -> Option<Vec<NavWalkStep>> {
    let mut steps: Vec<NavWalkStep> = Vec::new();
    let point = |p: (f32, f32)| (Fixed::from_f32(p.0), Fixed::from_f32(p.1));
    // 1. Standing on a lift end: a walk to that end's point on the current side.
    if let Some((door, side)) = lift_end
        && let Some(d) = map.doors.get(door as usize)
    {
        steps.push(NavWalkStep::Walk {
            to: point(d.point(side)),
            sector: from_sector,
            layer: from_layer,
        });
    }
    if from_sector == target_sector {
        // 2. Same sector: one walk.
        steps.push(NavWalkStep::Walk {
            to: target,
            sector: target_sector,
            layer: from_layer,
        });
        return Some(steps);
    }
    // 3. Other sector: the route of NAV-121.
    let target_number = map
        .sectors
        .get(target_sector as usize)
        .map_or(u32::MAX, |s| s.number);
    let route: Vec<Crossing> = search::route(
        map,
        (from.0.to_f32(), from.1.to_f32()),
        from_sector,
        from_layer,
        target_number,
        (target.0.to_f32(), target.1.to_f32()),
        admits,
        budget,
    )?;
    let mut sector = from_sector;
    let mut layer = from_layer;
    for (i, c) in route.iter().enumerate() {
        let Some(d) = map.doors.get(c.door as usize) else {
            continue;
        };
        let near = point(d.point(c.from));
        let far = point(d.point(c.from.other()));
        let building = if c.from == DoorSide::A {
            d.building
        } else {
            None
        };
        if let Some(_b) = building {
            // A building on the far side: waits, then an enter.
            if i != 0 {
                steps.push(NavWalkStep::Wait {
                    frames: BUILDING_ENTRY_WAIT,
                });
            }
            let frames = waits().saturating_add(waits());
            steps.push(NavWalkStep::Wait { frames });
            steps.push(NavWalkStep::Enter {
                door: c.door,
                at: near,
            });
            return Some(steps);
        }
        steps.push(NavWalkStep::Walk {
            to: near,
            sector,
            layer,
        });
        steps.push(NavWalkStep::ApproachPoint {
            at: near,
            tolerance: DOOR_APPROACH_TOLERANCE,
        });
        // The lock-pick branch ends the sequence.
        if let Some(who) = can_pick_lock {
            let locked = map
                .doors
                .get(c.door as usize)
                .is_some_and(|dd| dd.lock[0] != 0);
            if who.class == ActorClass::Player && who.ability_lockpick && locked {
                steps.push(NavWalkStep::PickLock {
                    door: c.door,
                    at: far,
                });
                return Some(steps);
            }
        }
        steps.push(NavWalkStep::PassDoor {
            door: c.door,
            from: c.from,
        });
        steps.push(NavWalkStep::ApproachPoint {
            at: far,
            tolerance: DOOR_APPROACH_TOLERANCE,
        });
        let far_number = d.sector(c.from.other());
        sector = map.sector_of_number(far_number).unwrap_or(sector);
        layer = d.layer(c.from.other());
        if steps.len() > MAX_WALK_STEPS {
            return None;
        }
    }
    // 4. The final walk to the target point.
    steps.push(NavWalkStep::Walk {
        to: target,
        sector: target_sector,
        layer,
    });
    Some(steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::map::NavMap;

    fn f1() -> NavMap {
        NavMap::build(&crate::nav::fixtures::obstacle_course()).unwrap()
    }

    /// `spec-navigation.md` 7.2 test 1: availability and the toggle.
    #[test]
    fn availability_follows_the_mask_predicate() {
        let m = f1();
        let mut s = NavState::new(&m);
        assert_eq!(s.avail.sector_state[0], INITIAL_SECTOR_STATE);
        assert!(s.available(0, 0));
        assert!(s.available(0, 1));
        assert!(!s.available(0, 2));
        s.toggle_field(&m, 0, 0);
        assert_eq!(s.avail.sector_state[0], 0x5555_5556);
        assert!(!s.available(0, 1));
        assert!(s.available(0, 2));
        s.toggle_field(&m, 0, 0);
        assert_eq!(s.avail.sector_state[0], INITIAL_SECTOR_STATE);
    }

    /// `spec-navigation.md` 7.2 test 9: admission.
    #[test]
    fn admission_follows_the_type_rules() {
        use crate::nav::map::{DoorSpec, FarmSpec, NavContent};
        let mut c = crate::nav::fixtures::obstacle_course();
        c.farm = vec![FarmSpec {
            kind: 1,
            doors: vec![DoorSpec {
                door_type: 2,
                open: true,
                lock: [1, 1, 0, 0],
                a: (10, 10),
                threshold: (12, 10),
                b: (14, 10),
                ..DoorSpec::default()
            }],
        }];
        let m = NavMap::build(&c).unwrap();
        let s = NavState::new(&m);
        let who = |class: ActorClass, pick: bool| Passer {
            class,
            ability_lockpick: pick,
            ..Passer::default()
        };
        assert!(!s.admits(
            &m,
            0,
            DoorSide::A,
            &who(ActorClass::PlainSoldier, false),
            false
        ));
        assert!(s.admits(&m, 0, DoorSide::A, &who(ActorClass::Civilian, false), false));
        assert!(!s.admits(&m, 0, DoorSide::A, &who(ActorClass::Player, false), false));
        assert!(s.admits(&m, 0, DoorSide::A, &who(ActorClass::Player, true), false));
        let mut shut = s.clone();
        shut.doors[0].open = false;
        for class in [
            ActorClass::Player,
            ActorClass::PlainSoldier,
            ActorClass::Civilian,
        ] {
            assert!(!shut.admits(&m, 0, DoorSide::A, &who(class, true), false));
        }
        // A ladder lift end: the player and the civilian pass, the plain soldier does not.
        let mut c2: NavContent = crate::nav::fixtures::obstacle_course();
        c2.lifts = vec![crate::nav::map::LiftSpec {
            sector: 0,
            layer: 0,
            lift_type: 2,
            shape: Vec::new(),
            doors: vec![DoorSpec {
                door_type: 5,
                open: true,
                ..DoorSpec::default()
            }],
        }];
        let m2 = NavMap::build(&c2).unwrap();
        let s2 = NavState::new(&m2);
        assert!(s2.admits(&m2, 0, DoorSide::A, &who(ActorClass::Player, false), false));
        assert!(!s2.admits(
            &m2,
            0,
            DoorSide::A,
            &who(ActorClass::PlainSoldier, false),
            false
        ));
        assert!(s2.admits(
            &m2,
            0,
            DoorSide::A,
            &who(ActorClass::Civilian, false),
            false
        ));
    }

    /// `spec-navigation.md` 7.2 test 10: native 186 sets the player lock and clearing it opens
    /// the door.
    #[test]
    fn clearing_a_lock_opens_the_door() {
        use crate::nav::map::{DoorSpec, FarmSpec};
        let mut c = crate::nav::fixtures::obstacle_course();
        c.farm = vec![FarmSpec {
            kind: 0,
            doors: vec![DoorSpec {
                door_type: 0,
                open: false,
                ..DoorSpec::default()
            }],
        }];
        let m = NavMap::build(&c).unwrap();
        let mut s = NavState::new(&m);
        s.set_lock(0, 0, 1);
        assert_eq!(s.doors[0].lock[0], 1);
        assert!(!s.doors[0].open);
        s.set_lock(0, 0, 0);
        assert_eq!(s.doors[0].lock[0], 0);
        assert!(s.doors[0].open);
    }
}
