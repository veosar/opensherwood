//! The authoritative world. M0: a synthetic scenario with a player unit, a patrolling guard and
//! rectangular obstacles, driven only by canonical input events.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::fixed::Fixed;
use crate::hash::{Encoder, Hashes, total};
use crate::input::{Button, InputEvent};
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
    /// Static obstacle (axis-aligned box centred on `pos` with half extents `size`).
    Obstacle,
}

/// An entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
    /// Id.
    pub id: EntityId,
    /// Kind.
    pub kind: EntityKind,
    /// Position (logical pixels, 24.8).
    pub x: Fixed,
    /// Position.
    pub y: Fixed,
    /// Half extents for obstacles; selection radius for actors.
    pub size: Fixed,
    /// Movement speed per tick.
    pub speed: Fixed,
    /// Current movement target.
    pub target: Option<(Fixed, Fixed)>,
    /// Patrol waypoints (guards).
    pub patrol: Vec<(Fixed, Fixed)>,
    /// Index of the next patrol waypoint.
    pub patrol_index: u32,
    /// Ticks to wait before moving on.
    pub wait_ticks: u32,
    /// Facing in 1/256 turns (0 = +x, increasing clockwise on screen).
    pub facing256: i32,
    /// Alive.
    pub alive: bool,
}

/// Scenario selection for `reset`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scenario {
    /// Synthetic scenario by name (`corridor`).
    Synthetic(String),
    /// Retail mission by base name (not available until milestone M2).
    Mission(String),
}

/// Serialisable snapshot of the whole authoritative state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Snapshot schema version.
    pub version: u32,
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
    /// Pointer position (24.8).
    pub pointer: (i32, i32),
    /// Selected entity.
    pub selected: Option<EntityId>,
    /// Entities.
    pub entities: Vec<Entity>,
    /// RNG draws so far.
    pub rng_draws: u64,
    /// Objective state for the synthetic scenario.
    pub objective_reached: bool,
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
    /// Pointer position in 24.8.
    pub pointer: (i32, i32),
    /// Buttons currently held.
    pub buttons_down: Vec<Button>,
    /// Selected entity.
    pub selected: Option<EntityId>,
    /// Entities by slot.
    pub entities: Vec<Entity>,
    /// Gameplay RNG stream.
    pub rng: Rng,
    /// Goal position for the synthetic objective.
    pub goal: (Fixed, Fixed),
    /// Whether the player reached the goal.
    pub objective_reached: bool,
}

/// Snapshot schema version.
pub const SNAPSHOT_VERSION: u32 = 1;

impl World {
    /// Create a world for a scenario.
    pub fn new(scenario: Scenario, seed: u64) -> Result<Self, String> {
        match scenario {
            Scenario::Synthetic(ref name) if name == "corridor" => {
                Ok(Self::corridor(scenario, seed))
            }
            Scenario::Synthetic(name) => Err(format!("unknown synthetic scenario '{name}'")),
            Scenario::Mission(name) => Err(format!(
                "mission '{name}' cannot be loaded yet (milestone M2)"
            )),
        }
    }

    fn corridor(scenario: Scenario, seed: u64) -> Self {
        let f = Fixed::from_int;
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
        });
        for (i, (x, y, w, h)) in [(320, 60, 20, 100), (320, 420, 20, 100), (520, 400, 20, 60)]
            .into_iter()
            .enumerate()
        {
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
            });
        }
        World {
            scenario,
            seed,
            tick: 0,
            viewport: (640, 480),
            pointer: (0, 0),
            buttons_down: Vec::new(),
            selected: None,
            entities,
            rng: Rng::new(seed, 1),
            goal: (f(600), f(240)),
            objective_reached: false,
        }
    }

    /// Apply the events of one tick (in order) and advance the simulation by one tick.
    pub fn step(&mut self, events: &[InputEvent]) {
        for e in events {
            self.apply(*e);
        }
        self.simulate();
        self.tick += 1;
    }

    fn apply(&mut self, event: InputEvent) {
        match event {
            InputEvent::PointerMove { x256, y256 } => self.pointer = (x256, y256),
            InputEvent::PointerDown { button } => {
                if !self.buttons_down.contains(&button) {
                    self.buttons_down.push(button);
                }
                match button {
                    Button::Left => self.select_at_pointer(),
                    Button::Right => self.order_move_to_pointer(),
                    Button::Middle => {}
                }
            }
            InputEvent::PointerUp { button } => self.buttons_down.retain(|b| *b != button),
            InputEvent::Wheel { .. } | InputEvent::KeyDown { .. } | InputEvent::KeyUp { .. } => {}
        }
    }

    fn select_at_pointer(&mut self) {
        let (px, py) = (
            Fixed::from_raw(self.pointer.0),
            Fixed::from_raw(self.pointer.1),
        );
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
        let target = (
            Fixed::from_raw(self.pointer.0),
            Fixed::from_raw(self.pointer.1),
        );
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
            .map(|e| (e.x, e.y, e.patrol[0].0, e.patrol[0].1))
            .collect();
        let (w, h) = (
            Fixed::from_int(self.viewport.0 as i32),
            Fixed::from_int(self.viewport.1 as i32),
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
            });
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
            e.x = clamp(nx, Fixed::ZERO, w);
            e.y = clamp(ny, Fixed::ZERO, h);
            if (e.x, e.y) == (tx, ty) {
                e.target = None;
                if e.kind == EntityKind::Guard {
                    e.patrol_index = (e.patrol_index + 1) % e.patrol.len().max(1) as u32;
                    e.wait_ticks = 20 + self.rng.below(20);
                }
            }
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
            world: self.clone(),
        }
    }

    /// Restore from a snapshot.
    pub fn restore(&mut self, snap: &Snapshot) -> Result<(), String> {
        if snap.version != SNAPSHOT_VERSION {
            return Err(format!(
                "snapshot version {} not supported (expected {SNAPSHOT_VERSION})",
                snap.version
            ));
        }
        *self = snap.world.clone();
        Ok(())
    }

    /// Observation for the harness.
    #[must_use]
    pub fn observe(&self) -> Observation {
        Observation {
            tick: self.tick,
            scenario: self.scenario.clone(),
            viewport: self.viewport,
            pointer: self.pointer,
            selected: self.selected,
            entities: self.entities.clone(),
            rng_draws: self.rng.draws,
            objective_reached: self.objective_reached,
        }
    }

    /// Canonical hashes (ADR-0004).
    #[must_use]
    pub fn hashes(&self) -> Hashes {
        let mut parts = BTreeMap::new();

        let mut w = Encoder::new("world");
        w.u64(self.tick)
            .u32(self.viewport.0)
            .u32(self.viewport.1)
            .i32(self.pointer.0)
            .i32(self.pointer.1);
        w.u64(self.seed).u8(u8::from(self.objective_reached));
        match &self.scenario {
            Scenario::Synthetic(n) => w.u8(1).str(n),
            Scenario::Mission(n) => w.u8(2).str(n),
        };
        parts.insert("world".into(), w.finish());

        let mut a = Encoder::new("actors");
        let mut sorted: Vec<&Entity> = self.entities.iter().collect();
        sorted.sort_by_key(|e| e.id);
        for e in &sorted {
            a.u32(e.id.index).u32(e.id.generation).u8(e.kind as u8);
            a.i32(e.x.raw())
                .i32(e.y.raw())
                .i32(e.size.raw())
                .i32(e.speed.raw());
            a.i32(e.facing256)
                .u8(u8::from(e.alive))
                .u32(e.wait_ticks)
                .u32(e.patrol_index);
        }
        parts.insert("actors".into(), a.finish());

        let mut o = Encoder::new("orders");
        match self.selected {
            Some(id) => o.u8(1).u32(id.index).u32(id.generation),
            None => o.u8(0),
        };
        for e in &sorted {
            match e.target {
                Some((x, y)) => o.u8(1).i32(x.raw()).i32(y.raw()),
                None => o.u8(0),
            };
        }
        parts.insert("orders".into(), o.finish());

        let mut r = Encoder::new("rng");
        let (s, i) = self.rng.state();
        r.str("pcg32").u64(s).u64(i).u64(self.rng.draws);
        parts.insert("rng".into(), r.finish());

        let t = total(&parts);
        parts.insert("total".into(), t);
        Hashes { parts }
    }
}

fn clamp(v: Fixed, lo: Fixed, hi: Fixed) -> Fixed {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

/// Facing from a direction vector: 8-way quantised to 1/256 turns, exact and deterministic.
fn facing_of(dx: Fixed, dy: Fixed) -> i32 {
    let (ax, ay) = (dx.abs(), dy.abs());
    let diagonal = ax.raw() * 2 > ay.raw() && ay.raw() * 2 > ax.raw();
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

    #[test]
    fn player_moves_when_selected_and_ordered() {
        let mut w = World::new(Scenario::Synthetic("corridor".into()), 7).unwrap();
        click(&mut w, 80, 240, Button::Left);
        assert!(w.selected.is_some());
        click(&mut w, 200, 240, Button::Right);
        for _ in 0..200 {
            w.step(&[]);
        }
        let p = &w.entities[0];
        assert_eq!((p.x.round(), p.y.round()), (200, 240));
        assert!(p.target.is_none());
    }

    #[test]
    fn same_inputs_same_hashes_and_restore_is_transparent() {
        let run = |snap_at: Option<u64>| {
            let mut w = World::new(Scenario::Synthetic("corridor".into()), 3).unwrap();
            click(&mut w, 80, 240, Button::Left);
            click(&mut w, 300, 200, Button::Right);
            let mut saved = None;
            for t in 0..300u64 {
                if Some(t) == snap_at {
                    saved = Some(w.snapshot());
                }
                w.step(&[]);
                if Some(t + 50) == snap_at.map(|s| s + 50) && t == snap_at.unwrap_or(u64::MAX) + 25
                {
                    w.restore(saved.as_ref().unwrap()).unwrap();
                    for _ in 0..25 {
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

    #[test]
    fn unknown_scenario_is_an_error() {
        assert!(World::new(Scenario::Synthetic("nope".into()), 1).is_err());
        assert!(World::new(Scenario::Mission("H01".into()), 1).is_err());
    }
}
