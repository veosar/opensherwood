//! The authoritative world. M0: a synthetic scenario with a player unit, a patrolling guard and
//! rectangular obstacles, driven only by canonical input events. M2 groundwork: a scrollable camera
//! over a map of arbitrary size and sprite animation state.
//!
//! Every field of [`World`] except `catalog` is authoritative: it is serialised in snapshots,
//! encoded in the canonical hash (ADR-0004) and validated on restore.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::anim::{AnimState, Catalog, direction_of};
use crate::fixed::Fixed;
use crate::geom::Geometry;
use crate::hash::{Encoder, HASH_SCHEMA_VERSION, Hashes, total};
use crate::input::{Button, InputEvent, Key, button_tag, encode_key};
use crate::rng::Rng;

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
    /// Movement speed per tick.
    pub speed: Fixed,
    /// Current movement target.
    pub target: Option<(Fixed, Fixed)>,
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
}

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
    /// Retail mission by base name (not available until milestone M2).
    Mission(String),
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
    /// Patrol waypoints in map pixels (guards).
    pub patrol: Vec<(i32, i32)>,
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
}

/// Serialisable snapshot of the whole authoritative state, with the versions it was made under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Snapshot schema version.
    pub version: u32,
    /// Ruleset the state was produced by.
    pub ruleset: u32,
    /// Hash schema in force.
    pub hash_schema: u32,
    /// World state.
    pub world: World,
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
    pub entities: Vec<Entity>,
    /// RNG draws so far.
    pub rng_draws: u64,
    /// Objective state for the synthetic scenario.
    pub objective_reached: bool,
}

/// Scroll speed in pixels per tick for keyboard and edge scrolling.
pub const SCROLL_SPEED: i32 = 8;
/// Edge-scroll margin in logical pixels.
pub const EDGE_MARGIN: i32 = 6;
/// Largest map dimension accepted.
pub const MAX_MAP_SIZE: u32 = 1 << 15;
/// Largest number of entities accepted in a snapshot.
pub const MAX_ENTITIES: usize = 1 << 16;
/// Pointer coordinates (24.8) are clamped to this magnitude.
pub const MAX_POINTER_RAW: i32 = 1 << 24;
/// Largest total vertex count of the walkable geometry.
pub const MAX_GEOMETRY_VERTICES: usize = 1 << 20;

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
    /// Static animation data attached by the app (not part of the snapshot; re-attached on load).
    #[serde(skip)]
    pub catalog: Catalog,
}

/// Snapshot schema version.
pub const SNAPSHOT_VERSION: u32 = 3;

impl World {
    /// Create a world for a scenario that needs no external data.
    pub fn new(scenario: Scenario, seed: u64) -> Result<Self, String> {
        match scenario {
            Scenario::Synthetic(ref name) if name == "corridor" => {
                Ok(Self::build(scenario, seed, None))
            }
            Scenario::Synthetic(name) => Err(format!("unknown synthetic scenario '{name}'")),
            Scenario::MapView { .. } => {
                Err("map view scenarios need MapInfo (World::new_map_view)".into())
            }
            Scenario::Mission(name) => Err(format!(
                "mission '{name}' cannot be loaded yet (milestone M2)"
            )),
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
        let mut world = Self::build(scenario, seed, Some(spec.map));
        world.geometry = spec.geometry.clone();
        world.entities.clear();
        world.goal = (Fixed::from_int(-1000), Fixed::from_int(-1000));
        let f = Fixed::from_int;
        for (i, a) in spec.actors.iter().enumerate() {
            let kind = match a.team {
                Team::Player => EntityKind::Player,
                Team::Enemy | Team::Civilian => EntityKind::Guard,
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
                    Fixed::from_raw(3 * 256 / 2)
                } else {
                    Fixed::from_int(1)
                },
                target: None,
                patrol: a.patrol.iter().map(|&(x, y)| (f(x), f(y))).collect(),
                patrol_index: 0,
                wait_ticks: 0,
                facing256: a.facing256.rem_euclid(256),
                alive: true,
                anim: Some(AnimState::new(a.profile.clone(), 0)),
            });
        }
        world.validate()?;
        Ok(world)
    }

    /// Attach walkable geometry (map view and missions).
    pub fn set_geometry(&mut self, geometry: Geometry) {
        self.geometry = geometry;
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
            Scenario::MapView { .. } => Ok(Self::build(scenario, seed, Some(info))),
            _ => Err("not a map view scenario".into()),
        }
    }

    fn build(scenario: Scenario, seed: u64, map: Option<MapInfo>) -> Self {
        let f = Fixed::from_int;
        let viewport = (640u32, 480u32);
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
            speed: Fixed::from_raw(3 * 256 / 2),
            target: None,
            patrol: Vec::new(),
            patrol_index: 0,
            wait_ticks: 0,
            facing256: 0,
            alive: true,
            anim: None,
        });
        entities.push(Entity {
            id: id(1),
            kind: EntityKind::Guard,
            x: f(400),
            y: f(120),
            size: f(12),
            speed: Fixed::from_int(1),
            target: None,
            patrol: vec![(f(400), f(120)), (f(400), f(360))],
            patrol_index: 1,
            wait_ticks: 0,
            facing256: 64,
            alive: true,
            anim: None,
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
                patrol: vec![(f(w), f(h))],
                patrol_index: 0,
                wait_ticks: 0,
                facing256: 0,
                alive: true,
                anim: None,
            });
        }
        World {
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
            entities,
            rng: Rng::new(seed, 1),
            goal: (f(600), f(240)),
            objective_reached: false,
            geometry: Geometry::default(),
            catalog: Catalog::default(),
        }
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

    /// Check every invariant a snapshot must satisfy before it may become the world.
    pub fn validate(&self) -> Result<(), String> {
        if self.viewport.0 == 0
            || self.viewport.1 == 0
            || self.viewport.0 > MAX_MAP_SIZE
            || self.viewport.1 > MAX_MAP_SIZE
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
        if self.pointer.0.abs() > MAX_POINTER_RAW || self.pointer.1.abs() > MAX_POINTER_RAW {
            return Err(format!("pointer {:?} out of range", self.pointer));
        }
        if self.entities.len() > MAX_ENTITIES {
            return Err(format!("{} entities exceed the limit", self.entities.len()));
        }
        if self.geometry.vertex_count() > MAX_GEOMETRY_VERTICES {
            return Err("geometry has too many vertices".into());
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
            if e.speed < Fixed::ZERO || e.size < Fixed::ZERO {
                return Err(format!("entity {:?} has a negative speed or size", e.id));
            }
        }
        if let Some(sel) = self.selected
            && !ids.contains(&sel)
        {
            return Err(format!("selected entity {sel:?} does not exist"));
        }
        self.rng.validate()
    }

    /// Apply the events of one tick (in order) and advance the simulation by one tick.
    pub fn step(&mut self, events: &[InputEvent]) {
        for e in events {
            self.apply(*e);
        }
        self.scroll();
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
                    Button::Left => self.select_at_pointer(),
                    Button::Right => self.order_move_to_pointer(),
                    Button::Middle => {}
                }
            }
            InputEvent::PointerUp { button } => {
                self.buttons_down.remove(&button);
            }
            InputEvent::KeyDown { key } => {
                self.keys_down.insert(key);
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

    fn select_at_pointer(&mut self) {
        let (px, py) = self.pointer_in_map();
        // First match in slot order: order is authoritative (and hashed).
        let hit = self
            .entities
            .iter()
            .filter(|e| e.alive && matches!(e.kind, EntityKind::Player | EntityKind::Guard))
            .find(|e| Fixed::length(e.x - px, e.y - py) <= e.size)
            .map(|e| e.id);
        self.selected = hit;
    }

    fn order_move_to_pointer(&mut self) {
        let Some(sel) = self.selected else { return };
        let target = self.pointer_in_map();
        if let Some(e) = self
            .entities
            .iter_mut()
            .find(|e| e.id == sel && e.kind == EntityKind::Player)
        {
            e.target = Some(target);
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
        for e in &mut self.entities {
            if !e.alive || e.kind == EntityKind::Obstacle {
                continue;
            }
            if e.kind == EntityKind::Guard && e.target.is_none() {
                if e.wait_ticks > 0 {
                    e.wait_ticks -= 1;
                } else if !e.patrol.is_empty() {
                    e.target = Some(e.patrol[e.patrol_index as usize % e.patrol.len()]);
                }
            }
            let Some((tx, ty)) = e.target else { continue };
            let dx = tx - e.x;
            let dy = ty - e.y;
            let dist = Fixed::length(dx, dy);
            let (nx, ny) = if dist <= e.speed {
                (tx, ty)
            } else {
                (e.x + dx * e.speed / dist, e.y + dy * e.speed / dist)
            };
            let blocked = obstacles.iter().any(|&(ox, oy, hw, hh)| {
                (nx - ox).abs() <= hw + e.size && (ny - oy).abs() <= hh + e.size
            }) || !self.geometry.is_walkable(nx.round(), ny.round());
            if dx.0 != 0 || dy.0 != 0 {
                e.facing256 = facing_of(dx, dy);
            }
            if blocked {
                e.target = None;
                if e.kind == EntityKind::Guard {
                    e.wait_ticks = 10;
                }
                continue;
            }
            e.x = nx.clamp(Fixed::ZERO, w);
            e.y = ny.clamp(Fixed::ZERO, h);
            if (e.x, e.y) == (tx, ty) {
                e.target = None;
                if e.kind == EntityKind::Guard {
                    e.patrol_index = (e.patrol_index + 1) % e.patrol.len().max(1) as u32;
                    e.wait_ticks = 20 + self.rng.below(20);
                }
            }
        }
        for e in &mut self.entities {
            let Some(anim) = e.anim.as_mut() else {
                continue;
            };
            let Some(set) = self.catalog.sets.get(&anim.set) else {
                continue;
            };
            let dir = direction_of(e.facing256);
            let wanted = if e.target.is_some() {
                set.walk[dir]
            } else {
                set.idle[dir]
            };
            anim.advance(&self.catalog, wanted);
        }
        if let Some(p) = self.entities.iter().find(|e| e.kind == EntityKind::Player)
            && Fixed::length(p.x - self.goal.0, p.y - self.goal.1) <= Fixed::from_int(16)
        {
            self.objective_reached = true;
        }
    }

    /// Snapshot everything authoritative.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            version: SNAPSHOT_VERSION,
            ruleset: crate::RULESET_VERSION,
            hash_schema: HASH_SCHEMA_VERSION,
            world: self.clone(),
        }
    }

    /// Validate a snapshot and, only if it is acceptable, make it the current state. The catalog
    /// is kept (it is static data). On error the world is unchanged.
    pub fn restore(&mut self, snap: &Snapshot) -> Result<(), String> {
        if snap.version != SNAPSHOT_VERSION {
            return Err(format!(
                "snapshot version {} not supported (expected {SNAPSHOT_VERSION})",
                snap.version
            ));
        }
        if snap.ruleset != crate::RULESET_VERSION {
            return Err(format!(
                "snapshot ruleset {} does not match {}",
                snap.ruleset,
                crate::RULESET_VERSION
            ));
        }
        snap.world.validate()?;
        let catalog = std::mem::take(&mut self.catalog);
        *self = snap.world.clone();
        self.catalog = catalog;
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
                self.entities.clone()
            } else {
                Vec::new()
            },
            rng_draws: self.rng.draws,
            objective_reached: self.objective_reached,
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
            .i32(self.goal.1.raw());
        match &self.scenario {
            Scenario::Synthetic(n) => w.u8(1).str(n),
            Scenario::Mission(n) => w.u8(2).str(n),
            Scenario::MapView { map, ambiance } => w.u8(3).str(map).str(ambiance),
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
        }
        parts.insert("orders".into(), o.finish());

        let mut r = Encoder::new("rng");
        r.str(Rng::ALGORITHM)
            .u64(self.rng.seed)
            .u64(self.rng.stream)
            .u64(self.rng.state())
            .u64(self.rng.draws);
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
        parts.insert("pathfinding".into(), g.finish());

        // Subsystems that do not exist yet hash to a versioned constant so the set of parts is
        // stable across milestones and their appearance is a visible ruleset change.
        for name in ["scripts", "scheduler", "campaign"] {
            let mut e = Encoder::new(name);
            e.u8(0);
            parts.insert(name.into(), e.finish());
        }

        let t = total(&parts);
        parts.insert("total".into(), t);
        Hashes { parts }
    }
}

/// Facing from a direction vector: 8-way quantised to 1/256 turns, exact and deterministic.
fn facing_of(dx: Fixed, dy: Fixed) -> i32 {
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
        click(&mut w, 200, 240, Button::Right);
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
            click(&mut w, 300, 200, Button::Right);
            let mut saved = None;
            for t in 0..300u64 {
                if Some(t) == snap_at {
                    saved = Some(w.snapshot());
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
        click(&mut w, 600, 240, Button::Right);
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
        "6bcd5808e2eb970ef912ef26ee231e503aceea4c9d8c06dce3a714709b92b958";

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
            y256: 479 * 256,
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
        assert_eq!(w.camera, (2000 - 640, 1000 - 480));
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
        let mut snap = w.snapshot();
        snap.world.entities[1].id = snap.world.entities[0].id;
        assert!(w.restore(&snap).unwrap_err().contains("duplicate"));
        let mut snap = w.snapshot();
        snap.world.entities[2].patrol.clear();
        assert!(w.restore(&snap).is_err());
        let mut snap = w.snapshot();
        snap.world.selected = Some(EntityId {
            index: 99,
            generation: 1,
        });
        assert!(w.restore(&snap).is_err());
        let mut snap = w.snapshot();
        snap.world.camera = (5, 0);
        assert!(w.restore(&snap).is_err());
        let mut snap = w.snapshot();
        snap.ruleset += 1;
        assert!(w.restore(&snap).is_err());
        let mut snap = w.snapshot();
        snap.version += 1;
        assert!(w.restore(&snap).is_err());
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
            },
            actors: vec![
                ActorSpec {
                    profile: "RobinHood".into(),
                    team: Team::Player,
                    x: 100,
                    y: 200,
                    facing256: 64,
                    patrol: vec![],
                },
                ActorSpec {
                    profile: "Soldier A00".into(),
                    team: Team::Enemy,
                    x: 300,
                    y: 200,
                    facing256: -32,
                    patrol: vec![(300, 200), (300, 400)],
                },
            ],
        };
        let w = World::new_mission(Scenario::Mission("EmbTut".into()), 1, &spec).unwrap();
        assert_eq!(w.entities.len(), 2);
        assert_eq!(w.entities[0].kind, EntityKind::Player);
        assert_eq!(w.entities[1].facing256, 224);
        assert_eq!(w.entities[1].patrol.len(), 2);
        assert_eq!(w.entities[0].anim.as_ref().unwrap().set, "RobinHood");
        assert!(World::new_mission(Scenario::Synthetic("x".into()), 1, &spec).is_err());
        // Walking east from (100,200) into the obstacle at x=180 stops at its edge.
        let mut w = w;
        w.selected = Some(w.entities[0].id);
        w.entities[0].target = Some((Fixed::from_int(400), Fixed::from_int(200)));
        for _ in 0..400 {
            w.step(&[]);
        }
        let x = w.entities[0].x.round();
        assert!((160..=180).contains(&x), "stopped at {x}");
        assert!(w.entities[0].target.is_none());
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
}
