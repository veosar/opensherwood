//! The walk pipeline: what a walk order does between the click (or the script's native) and the
//! move actions the movement carries out.
//!
//! This is the world-facing half of `docs/original/spec-navigation.md`: click resolution
//! (NAV-100 to NAV-102), the walk sequence (NAV-130), the walk element and its path request
//! (NAV-140), the schedule of 8.1, the delivery of a result (NAV-146), the door and lift
//! crossings (NAV-171, NAV-200) and the building entry (3.9). The pure geometry, the search and
//! the route live in [`crate::nav`]; the per-frame displacement and the arrival are the movement
//! specification's and stay in `world.rs` and `ai.rs`.
//!
//! A world without a [`crate::nav::map::NavMap`] - the synthetic scenarios, which have no map -
//! keeps the cell grid of [`crate::nav::grid`]; every map-backed mission goes through this
//! module.

use std::sync::Arc;

use crate::fixed::Fixed;
use crate::nav::map::{ClickAnswer, ClickTarget, DoorSide, NavContent, NavMap};
use crate::nav::search;
use crate::nav::state::{
    ActorClass, DOOR_APPROACH_TOLERANCE, NavRequest, NavState, NavWalk, NavWalkStep, Passer,
    RequestState, build_walk_sequence,
};
use crate::vm::Assumption;
use crate::world::{EntityKind, Gait, NO_AREA, Team, World};

/// The ability id the door rules call "lock-picking" (`spec-navigation.md` NAV-170 rule 4).
pub const ABILITY_LOCKPICK: u32 = 0x1c;
/// The ability id NAV-192 kind 3 (a climb) requires.
pub const ABILITY_CLIMB: u32 = 0x15;

/// Work units one walk order may spend on its route search and its geometric tests before the
/// order is refused. The route is bounded by the door count and the 102-door limit; this bounds
/// the tests it pays for.
pub const ORDER_NAV_WORK: u64 = 1 << 20;

/// Reserve the capped search plus request scans and result delivery before the phase runs.
pub(crate) const NAV_TICK_WORK: u64 = crate::nav::state::SEARCH_BUDGET
    + 4 * crate::nav::state::MAX_REQUESTS as u64
    + crate::nav::state::MAX_WAYPOINTS as u64;

/// What a click resolved to (`spec-navigation.md` NAV-100, NAV-102).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavClick {
    /// The layer the answer came from, or the selected character's layer when no layer
    /// answered.
    pub layer: u32,
    /// The flat sector index the walk would run to, when the cursor is valid.
    pub sector: Option<u32>,
    /// The door the click routes to (a leaf hit; NAV-122).
    pub door: Option<u32>,
    /// Whether the cursor is the walk cursor (NAV-102).
    pub valid: bool,
    /// The point the walk aims at.
    pub point: (Fixed, Fixed),
}

impl World {
    /// Validate navigation against the attached immutable map before accepting a snapshot.
    pub(crate) fn check_navigation(&self) -> Result<(), String> {
        let Some(map) = self.navmap.as_ref() else {
            if self.nav_state != NavState::default()
                || self
                    .entities
                    .iter()
                    .any(|e| e.walk.is_some() || e.crossing.is_some())
            {
                return Err("navigation state without map content".into());
            }
            return Ok(());
        };
        self.nav_state.check(map)?;
        let bounded = |p: (Fixed, Fixed)| {
            let limit = crate::world::MAX_MAP_SIZE * 256;
            p.0.0.unsigned_abs() <= limit && p.1.0.unsigned_abs() <= limit
        };
        if self.nav_state.drive_cursor as usize >= self.entities.len().max(1) {
            return Err("navigation drive cursor outside entity table".into());
        }
        for (i, e) in self.entities.iter().enumerate() {
            if e.layer > map.layer_count {
                return Err(format!(
                    "entity {i} is on invalid navigation layer {}",
                    e.layer
                ));
            }
            if let Some(w) = &e.walk {
                if w.steps.is_empty()
                    || w.steps.len() > crate::nav::state::MAX_WALK_STEPS
                    || w.cursor as usize > w.steps.len()
                    || w.gait > 1
                {
                    return Err(format!("entity {i} has a malformed navigation walk"));
                }
                for step in &w.steps {
                    let valid = match *step {
                        NavWalkStep::Walk { to, sector, layer } => {
                            bounded(to)
                                && map
                                    .sectors
                                    .get(sector as usize)
                                    .is_some_and(|s| s.layer == layer)
                        }
                        NavWalkStep::ApproachPoint { at, tolerance } => {
                            bounded(at)
                                && (0..=crate::world::MAX_MAP_SIZE as i32).contains(&tolerance)
                        }
                        NavWalkStep::ApproachSector { sector } => {
                            (sector as usize) < map.sectors.len()
                        }
                        NavWalkStep::Wait { .. } => true,
                        NavWalkStep::Enter { door, at } => {
                            bounded(at)
                                && map
                                    .doors
                                    .get(door as usize)
                                    .is_some_and(|d| d.building.is_some())
                        }
                        NavWalkStep::PassDoor { door, .. } => (door as usize) < map.doors.len(),
                        NavWalkStep::PickLock { door, at } => {
                            bounded(at) && (door as usize) < map.doors.len()
                        }
                    };
                    if !valid {
                        return Err(format!("entity {i} has an invalid navigation step"));
                    }
                }
            }
            if let Some((door, from, _)) = e.crossing
                && ((door as usize) >= map.doors.len()
                    || e.walk.as_ref().and_then(NavWalk::current)
                        != Some(NavWalkStep::PassDoor { door, from }))
            {
                return Err(format!("entity {i} has an orphaned navigation crossing"));
            }
        }
        for r in self
            .nav_state
            .queue
            .iter()
            .chain(self.nav_state.completed.iter())
            .chain(self.nav_state.failed.iter())
        {
            let Some(e) = self.entities.get(r.owner as usize) else {
                return Err("navigation request names an absent owner".into());
            };
            let owns = e.walk.as_ref().is_some_and(|w| {
                w.submitted
                    && w.current()
                        == Some(NavWalkStep::Walk {
                            to: r.goal,
                            sector: r.sector,
                            layer: r.layer,
                        })
            });
            if !owns
                || e.target.is_some()
                || e.crossing.is_some()
                || r.submitted > self.tick
                || !bounded(r.start)
                || !bounded(r.goal)
            {
                return Err("navigation request disagrees with its owner or clock".into());
            }
            let valid = match &r.state {
                RequestState::Queued => true,
                RequestState::Completed {
                    waypoints,
                    delivery,
                } => {
                    *delivery >= r.submitted
                        && *delivery <= self.tick.saturating_add(1)
                        && waypoints.iter().all(|p| bounded(*p))
                }
                RequestState::FailedPending { arrival } => {
                    *arrival >= r.submitted && *arrival <= self.tick.saturating_add(1)
                }
            };
            if !valid {
                return Err("navigation request has invalid delivery state".into());
            }
        }
        let mut occupants = std::collections::BTreeSet::new();
        for b in &self.nav_state.buildings {
            if b.occupants.len() > self.entities.len()
                || b.displayed.len() > b.occupants.len()
                || b.occupants
                    .iter()
                    .any(|&i| i as usize >= self.entities.len() || !occupants.insert(i))
                || b.displayed
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != b.displayed.len()
                || b.displayed.iter().any(|i| !b.occupants.contains(i))
            {
                return Err("navigation building occupants are invalid".into());
            }
        }
        Ok(())
    }

    /// Attach a map's navigation content (`spec-navigation.md` 2.2, 2.3, 2.6) and start its
    /// dynamic state. The app calls this once per mission, before the script is attached.
    pub fn attach_navmap(&mut self, content: &NavContent) -> Result<(), String> {
        let map = NavMap::build(content).map_err(|e| format!("map navigation: {e}"))?;
        let map = Arc::new(map);
        self.nav_state = NavState::new(&map);
        self.navmap = Some(map);
        self.nav_place_entities();
        Ok(())
    }

    /// Re-attach a navigation map after a restore (content is not part of the snapshot, exactly
    /// like the animation catalog). The state must already match the map.
    pub fn reattach_navmap(&mut self, map: Arc<NavMap>) -> Result<(), String> {
        self.nav_state.check(&map)?;
        self.navmap = Some(map);
        Ok(())
    }

    /// Give every entity whose placement named no sector the sector its position falls on
    /// (NAV-011: the level's default sector when it falls on none).
    fn nav_place_entities(&mut self) {
        let Some(map) = self.navmap.clone() else {
            return;
        };
        let default_sector = map
            .sectors
            .last()
            .map_or(0, |s| s.number + s.obstacles.len() as u32 + 1);
        for i in 0..self.entities.len() {
            let e = &self.entities[i];
            if map.sector_of_number(e.sector).is_some() {
                continue;
            }
            let p = (e.x.to_f32(), e.y.to_f32());
            let found = (0..map.layer_count).rev().find_map(|layer| {
                match map.resolve_on_layer(layer, p, &|_, _| false, &|_| false, p) {
                    ClickAnswer::Hit(ClickTarget::Sector(s)) => Some((layer, s)),
                    _ => None,
                }
            });
            let e = &mut self.entities[i];
            match found {
                Some((layer, s)) => {
                    e.layer = layer;
                    e.sector = map.sectors[s as usize].number;
                }
                None => e.sector = default_sector,
            }
        }
    }

    /// The class and abilities the door rules ask about (`spec-navigation.md` NAV-170).
    ///
    /// No reviewed specification says where a character's abilities come from, so this engine
    /// gives every player character the climbing ability 0x15 and no other
    /// ([`Assumption::NavAbilities`]): NAV-192 kind 3 would otherwise make every climb lift
    /// impassable, and without 0x1c the lock-pick branch of NAV-130, which this engine does not
    /// animate, never arises.
    fn nav_passer(&self, i: usize) -> Passer {
        let e = &self.entities[i];
        let class = match (e.kind, e.team) {
            (EntityKind::Player, _) => ActorClass::Player,
            (_, Team::Civilian) => ActorClass::Civilian,
            _ => ActorClass::PlainSoldier,
        };
        Passer {
            class,
            state: 0,
            ability_lockpick: false,
            ability_climb: class == ActorClass::Player,
            cannot_pass: false,
        }
    }

    /// Whether this world runs on a map's navigation content.
    #[must_use]
    pub fn has_navmap(&self) -> bool {
        self.navmap.is_some()
    }

    /// Record a navigation assumption when the world runs a script (the taint registry lives in
    /// the VM; a world without one takes the same decisions but has nowhere to record them).
    fn nav_assume(&mut self, a: Assumption) {
        if let Some(vm) = self.vm.as_mut() {
            vm.assume(a);
        }
    }

    // ----------------------------------------------------------------------------------------
    // Click resolution
    // ----------------------------------------------------------------------------------------

    /// Resolve a map point to a layer, a sector and a target (`spec-navigation.md` NAV-100 and
    /// NAV-102). `reference` is the selected character's position, which the jump-zone pass
    /// measures against, and whose layer answers when no layer does.
    #[must_use]
    pub fn nav_resolve(&self, point: (Fixed, Fixed), reference_entity: Option<usize>) -> NavClick {
        let default_layer = reference_entity.map_or(0, |i| self.entities[i].layer);
        let reference = reference_entity.map_or((0.0, 0.0), |i| {
            (self.entities[i].x.to_f32(), self.entities[i].y.to_f32())
        });
        let mut out = NavClick {
            layer: default_layer,
            sector: None,
            door: None,
            valid: false,
            point,
        };
        let Some(map) = self.navmap.as_ref() else {
            return out;
        };
        let state = &self.nav_state;
        let enabled = |s: u32, o: u32| state.obstacle_enabled(map, s, o);
        let leaf = |d: u32| state.doors.get(d as usize).is_some_and(|x| x.leaf_enabled);
        let (layer, answer) = map.resolve(
            (point.0.to_f32(), point.1.to_f32()),
            &enabled,
            &leaf,
            reference,
        );
        if let Some(l) = layer {
            out.layer = l;
        }
        match answer {
            ClickAnswer::Nothing | ClickAnswer::Blocked => out,
            ClickAnswer::Hit(ClickTarget::Sector(s)) => {
                out.sector = Some(s);
                out.valid = true;
                out
            }
            ClickAnswer::Hit(ClickTarget::JumpZone(_, s)) => {
                // `NavJumpZones`: the jump itself does not exist; the walk goes to the clicked
                // point on the candidate sector and the cursor is valid.
                out.sector = Some(s);
                out.valid = true;
                out
            }
            ClickAnswer::Hit(ClickTarget::DoorLeaf(d)) => {
                out.door = Some(d);
                out.valid = true;
                out
            }
            ClickAnswer::Hit(ClickTarget::LiftShape(k)) => {
                let lift = map.lifts.get(k as usize);
                out.sector = lift.and_then(|l| map.sector_of_number(l.sector));
                out.layer = lift.map_or(out.layer, |l| l.layer);
                out.valid = out.sector.is_some();
                out
            }
        }
    }

    // ----------------------------------------------------------------------------------------
    // Ordering a walk
    // ----------------------------------------------------------------------------------------

    /// Order entity `i` to walk to a map point, through the pipeline of NAV-130. Returns
    /// whether a sequence was issued; a refused order leaves the entity without a target.
    pub(crate) fn nav_plan(&mut self, i: usize, target: (Fixed, Fixed)) -> bool {
        let Some(map) = self.navmap.clone() else {
            return false;
        };
        self.nav_cancel_walk(i);
        let click = self.nav_resolve(target, Some(i));
        // A click on a door leaf routes to the door (NAV-122) with the admission of
        // `NavDoorInteraction`: rule 3 (the door must be open) is waived.
        let (target_sector, aim) = match (click.door, click.sector) {
            (Some(d), _) => {
                self.nav_assume(Assumption::NavDoorInteraction);
                let door = &map.doors[d as usize];
                let side = if door.layer_a == self.entities[i].layer {
                    DoorSide::A
                } else {
                    DoorSide::B
                };
                let p = door.point(side);
                let sector = map.sector_of_number(door.sector(side));
                let Some(sector) = sector else {
                    return false;
                };
                (sector, (Fixed::from_f32(p.0), Fixed::from_f32(p.1)))
            }
            (None, Some(s)) => (s, target),
            (None, None) => return false,
        };
        // NAV-014 with 8.4: a polygon that is not a sector cannot own a request.
        let Some(from_sector) = map.sector_of_number(self.entities[i].sector) else {
            return false;
        };
        let from_layer = self.entities[i].layer;
        let who = self.nav_passer(i);
        let waived = click.door;
        let state = &self.nav_state;
        let mapref = &map;
        let admits = move |door: u32, side: DoorSide| {
            state.admits_with_open(mapref, door, side, &who, false, Some(door) == waived)
        };
        let mut budget = ORDER_NAV_WORK;
        let lift_end = self.nav_lift_end(i);
        let from = (self.entities[i].x, self.entities[i].y);
        // The two draws of the building-entry wait come off the gameplay stream in the order
        // the sequence is built (NAV-130 step 3); taking the stream out of `self` keeps the
        // borrow simple and the draw order identical.
        let mut rng = std::mem::replace(&mut self.rng, crate::rng::Rng::new(0, 0));
        let mut draws = || rng.below(16);
        let steps = build_walk_sequence(
            &map,
            from,
            from_sector,
            from_layer,
            aim,
            target_sector,
            lift_end,
            None,
            &admits,
            &mut draws,
            &mut budget,
        );
        self.rng = rng;
        let Some(steps) = steps else {
            let e = &mut self.entities[i];
            e.target = None;
            e.path.clear();
            return false;
        };
        let gait = self.entities[i].gait;
        let e = &mut self.entities[i];
        e.walk = Some(NavWalk {
            steps,
            cursor: 0,
            gait: gait.tag(),
            run: gait == Gait::Run,
            submitted: false,
        });
        e.target = None;
        e.path.clear();
        true
    }

    /// The lift end the character is standing on, if any (NAV-130 step 1): a lift end whose
    /// point on the character's side is within the door-approach tolerance.
    fn nav_lift_end(&self, i: usize) -> Option<(u32, DoorSide)> {
        let map = self.navmap.as_ref()?;
        let e = &self.entities[i];
        let p = (e.x.to_f32(), e.y.to_f32());
        for (di, d) in map.doors.iter().enumerate() {
            if !matches!(d.door_type, 4..=6) {
                continue;
            }
            for side in [DoorSide::A, DoorSide::B] {
                let q = d.point(side);
                if (q.0 - p.0).abs() < 1.0 && (q.1 - p.1).abs() < 1.0 {
                    return Some((di as u32, side));
                }
            }
        }
        None
    }

    /// Drop the walk sequence and any request of entity `i` (a new order, a cancellation, a
    /// right click).
    pub(crate) fn nav_cancel_walk(&mut self, i: usize) {
        self.nav_state.cancel(i as u32);
        let e = &mut self.entities[i];
        e.walk = None;
        e.crossing = None;
        e.target = None;
        e.path.clear();
    }

    // ----------------------------------------------------------------------------------------
    // The tick: P1 (consume), P2 (dispatch), P3 (the sequences)
    // ----------------------------------------------------------------------------------------

    /// P1 and P2 of the schedule of `spec-navigation.md` 8.1: consume the completed result whose
    /// delivery tick has come, fail the requests whose window has run out, then dispatch the
    /// queue head.
    pub(crate) fn nav_tick(&mut self) {
        let Some(map) = self.navmap.clone() else {
            return;
        };
        // P1: the consumer slot.
        if let Some(done) = self.nav_state.consume(self.tick) {
            self.nav_deliver(&done);
        }
        for failed in self.nav_state.take_expired_failures(self.tick) {
            // NAV-146: the element fails; a player character would play its refusal reaction.
            let i = failed.owner as usize;
            if i < self.entities.len() {
                self.nav_fail_walk(i);
            }
        }
        // P2: the dispatch.
        self.nav_state.dispatch(&map, self.tick);
    }

    /// Turn a delivered result into the entity's move actions (`spec-navigation.md` NAV-146).
    fn nav_deliver(&mut self, r: &NavRequest) {
        let RequestState::Completed { waypoints, .. } = &r.state else {
            return;
        };
        let i = r.owner as usize;
        if i >= self.entities.len() || waypoints.is_empty() {
            return;
        }
        let gait = if r.gait == Gait::Run.tag() {
            Gait::Run
        } else {
            Gait::Walk
        };
        let e = &mut self.entities[i];
        e.path.clone_from(waypoints);
        e.target = waypoints.last().copied();
        e.gait = gait;
        if let Some(w) = e.walk.as_mut() {
            w.submitted = true;
        }
    }

    /// The walk element failed: the order ends (a player character plays its refusal reaction,
    /// which this engine does not animate).
    pub(crate) fn nav_fail_walk(&mut self, i: usize) {
        self.nav_state.cancel(i as u32);
        self.nav_finish_program(i, false);
        let e = &mut self.entities[i];
        e.walk = None;
        e.target = None;
        e.path.clear();
        e.crossing = None;
        e.gait = Gait::Walk;
    }

    fn nav_finish_program(&mut self, i: usize, reached: bool) {
        let e = &mut self.entities[i];
        if e.walk.is_none()
            || e.kind != EntityKind::Guard
            || e.ai_state != crate::ai::AiState::Patrol
            || e.ai_locked
        {
            return;
        }
        if e.program.is_some() {
            e.pc = e.pc.saturating_add(1);
        } else {
            e.patrol_index = (e.patrol_index + 1) % e.patrol.len().max(1) as u32;
            if reached {
                e.wait_ticks = 20 + self.rng.below(20);
            }
        }
        if !reached {
            e.wait_ticks = 10;
        }
    }

    /// P3 of 8.1 for the walk sequences: advance every character's sequence by the step it is
    /// on. Runs in entity order, which is the simulation order.
    pub(crate) fn nav_drive(&mut self, mut budget: u64) {
        let Some(map) = self.navmap.clone() else {
            return;
        };
        let count = self.entities.len();
        if count == 0 {
            return;
        }
        let start = self.nav_state.drive_cursor as usize % count;
        for offset in 0..count {
            let i = (start + offset) % count;
            if !crate::vm::charge_budget(&mut budget, 1) {
                self.nav_state.drive_cursor = i as u32;
                return;
            }
            if self.entities[i].walk.is_none() {
                continue;
            }
            if !self.entities[i].alive || !self.entities[i].active {
                self.nav_fail_walk(i);
                continue;
            }
            if self.entities[i].target.is_some() || self.nav_state.has_request(i as u32) {
                continue;
            }
            if !crate::vm::charge_budget(&mut budget, ORDER_NAV_WORK) {
                self.nav_state.drive_cursor = i as u32;
                return;
            }
            self.nav_step(i, &map);
        }
        self.nav_state.drive_cursor = 0;
    }

    /// One character's sequence, advanced as far as it goes this tick.
    fn nav_step(&mut self, i: usize, map: &NavMap) {
        // A sequence never advances more steps in one tick than it has.
        let limit = self.entities[i]
            .walk
            .as_ref()
            .map_or(0, |w| w.steps.len() + 1);
        for _ in 0..limit {
            let Some(step) = self.entities[i].walk.as_ref().and_then(NavWalk::current) else {
                // The sequence is done.
                self.nav_finish_program(i, true);
                self.entities[i].walk = None;
                return;
            };
            let advance = match step {
                NavWalkStep::Walk { to, sector, layer } => {
                    self.nav_walk_step(i, map, to, sector, layer)
                }
                NavWalkStep::ApproachPoint { at, tolerance } => {
                    let e = &self.entities[i];
                    let dx = (e.x - at.0).abs();
                    let dy = (e.y - at.1).abs();
                    let bound = Fixed::from_int(tolerance + 5);
                    if dx.max(dy) < bound {
                        StepResult::Done
                    } else {
                        StepResult::Cancel
                    }
                }
                NavWalkStep::ApproachSector { sector } => {
                    let number = map
                        .sectors
                        .get(sector as usize)
                        .map_or(u32::MAX, |s| s.number);
                    if self.entities[i].sector == number {
                        StepResult::Done
                    } else {
                        StepResult::Cancel
                    }
                }
                NavWalkStep::Wait { frames } => {
                    if frames == 0 {
                        StepResult::Done
                    } else {
                        if let Some(w) = self.entities[i].walk.as_mut() {
                            let c = w.cursor as usize;
                            if let Some(NavWalkStep::Wait { frames }) = w.steps.get_mut(c) {
                                *frames -= 1;
                            }
                        }
                        StepResult::Wait
                    }
                }
                NavWalkStep::Enter { door, at } => self.nav_enter_step(i, map, door, at),
                NavWalkStep::PassDoor { door, from } => self.nav_cross_step(i, map, door, from),
                NavWalkStep::PickLock { .. } => StepResult::End,
            };
            match advance {
                StepResult::Done => {
                    if let Some(w) = self.entities[i].walk.as_mut() {
                        w.cursor += 1;
                        w.submitted = false;
                    }
                }
                StepResult::Wait => return,
                StepResult::Cancel => {
                    self.nav_fail_walk(i);
                    return;
                }
                StepResult::End => {
                    let e = &mut self.entities[i];
                    e.walk = None;
                    e.target = None;
                    e.path.clear();
                    return;
                }
            }
        }
    }

    /// The **walk element** of `spec-navigation.md` NAV-140.
    fn nav_walk_step(
        &mut self,
        i: usize,
        map: &NavMap,
        to: (Fixed, Fixed),
        sector: u32,
        layer: u32,
    ) -> StepResult {
        // Already moving: the element ends when the move actions are exhausted.
        if self.entities[i].walk.as_ref().is_some_and(|w| w.submitted) {
            if self.nav_state.has_request(i as u32) {
                return StepResult::Wait;
            }
            if self.entities[i].target.is_some() {
                return StepResult::Wait;
            }
            return StepResult::Done;
        }
        let half = map.half_size(0);
        let mut budget = ORDER_NAV_WORK;
        let target = (to.0.to_f32(), to.1.to_f32());
        // The geometric part of NAV-140 runs against the availability, which the borrow checker
        // will not let us hold while the entity moves: take it, decide, put it back.
        let avail = std::mem::take(&mut self.nav_state.avail);
        let decision = {
            // (1) the walker box grown by 0.5 px on every side.
            let (x, y) = (self.entities[i].x.to_f32(), self.entities[i].y.to_f32());
            let grown = (
                x - half.0 - 0.5,
                y - half.1 - 0.5,
                x + half.0 + 0.5,
                y + half.1 + 0.5,
            );
            let corrected = if search::clearance(map, &avail, layer, grown, &mut budget) {
                Some((x, y))
            } else {
                // NAV-140 step 1 keeps the corrected centre even after all 50 rounds.
                // Only the smaller box in step 3 must finish clear to submit a request.
                let (bx, _) = search::unstick(map, &avail, layer, grown, &mut budget);
                Some((f32::midpoint(bx.0, bx.2), f32::midpoint(bx.1, bx.3)))
            };
            match corrected {
                None => WalkDecision::Fail,
                Some((x, y)) => {
                    // (2) a free corridor to the target: one move action, no request.
                    if search::corridor_free(map, &avail, layer, (x, y), target, half, &mut budget)
                    {
                        WalkDecision::Direct((x, y))
                    } else {
                        // (3) a path request; the walker box decides the unstuck flag
                        // (NAV-140 step 3, NAV-145).
                        let box_now = (x - half.0, y - half.1, x + half.0, y + half.1);
                        if search::clearance(map, &avail, layer, box_now, &mut budget) {
                            WalkDecision::Request((x, y), (x, y), false)
                        } else {
                            let (bx, ok) =
                                search::unstick(map, &avail, layer, box_now, &mut budget);
                            if ok {
                                WalkDecision::Request(
                                    (x, y),
                                    (f32::midpoint(bx.0, bx.2), f32::midpoint(bx.1, bx.3)),
                                    true,
                                )
                            } else {
                                WalkDecision::Fail
                            }
                        }
                    }
                }
            }
        };
        self.nav_state.avail = avail;
        if budget == 0 {
            return StepResult::Cancel;
        }
        let (start, unstuck) = match decision {
            WalkDecision::Fail => return StepResult::Cancel,
            WalkDecision::Direct((x, y)) => {
                let e = &mut self.entities[i];
                e.x = Fixed::from_f32(x);
                e.y = Fixed::from_f32(y);
                e.target = Some(to);
                e.path = vec![to];
                if let Some(walk) = e.walk.as_mut() {
                    walk.submitted = true;
                }
                return StepResult::Wait;
            }
            WalkDecision::Request((x, y), start, unstuck) => {
                let e = &mut self.entities[i];
                e.x = Fixed::from_f32(x);
                e.y = Fixed::from_f32(y);
                (start, unstuck)
            }
        };
        let number = map
            .sectors
            .get(sector as usize)
            .map_or(u32::MAX, |s| s.number);
        let (gait, run) = self.entities[i]
            .walk
            .as_ref()
            .map_or((0, false), |w| (w.gait, w.run));
        let priority = u8::from(self.entities[i].kind != EntityKind::Player);
        let request = NavRequest {
            id: 0,
            owner: i as u32,
            layer,
            sector_number: number,
            sector,
            start: (Fixed::from_f32(start.0), Fixed::from_f32(start.1)),
            goal: to,
            gait,
            run,
            priority,
            unstuck,
            submitted: self.tick,
            state: RequestState::Queued,
        };
        let tick = self.tick;
        if self.nav_state.submit(request, tick).is_none() {
            return StepResult::Cancel;
        }
        if let Some(w) = self.entities[i].walk.as_mut() {
            w.submitted = true;
        }
        self.entities[i].target = None;
        self.entities[i].path.clear();
        StepResult::Wait
    }

    /// Crossing a door or a lift end (`spec-navigation.md` NAV-171 and NAV-200): a move to the
    /// threshold, the marker at which the layer and the sector change (ANIM-311), a move to the
    /// far point, the closing marker.
    fn nav_cross_step(&mut self, i: usize, map: &NavMap, door: u32, from: DoorSide) -> StepResult {
        let Some(d) = map.doors.get(door as usize) else {
            return StepResult::Cancel;
        };
        match self.entities[i].crossing {
            None => {
                // The crossing admission is re-checked; failure fails the element.
                let who = self.nav_passer(i);
                if !self.nav_state.admits(map, door, from, &who, true) {
                    return StepResult::Cancel;
                }
                if d.lift.is_some() {
                    // Ladders and climbs are executed as stairs (`NavLadderClimb`).
                    let lift_type = d
                        .lift
                        .and_then(|l| map.lifts.get(l as usize))
                        .map_or(1, |l| l.lift_type);
                    if lift_type != 1 {
                        self.nav_assume(Assumption::NavLadderClimb);
                    }
                }
                if map
                    .sector_of_number(d.sector(from))
                    .and_then(|s| map.sectors.get(s as usize))
                    .is_some_and(|s| s.unknown_flag != 0)
                    && self.entities[i].kind == EntityKind::Player
                {
                    self.nav_assume(Assumption::NavUnknownFlagActions);
                }
                self.entities[i].crossing = Some((door, from, false));
                let t = d.threshold;
                let to = (Fixed::from_int(t.0), Fixed::from_int(t.1));
                let e = &mut self.entities[i];
                e.target = Some(to);
                e.path = vec![to];
                // The run flag of the two moves is set iff the current gait is a run
                // (`spec-navigation.md` NAV-200; the original tests gait 0xc9).
                if let Some(w) = e.walk.as_mut() {
                    w.run = w.gait == Gait::Run.tag();
                }
                StepResult::Wait
            }
            Some((_, side, exiting)) => {
                if self.entities[i].target.is_some() {
                    return StepResult::Wait;
                }
                if exiting {
                    // The closing marker: the crossing is over.
                    self.entities[i].crossing = None;
                    StepResult::Done
                } else {
                    // The marker: the layer and the sector change here (ANIM-311).
                    let far = side.other();
                    let (number, layer) = (d.sector(far), d.layer(far));
                    let end = Self::nav_cross_endpoint(map, door, far);
                    let e = &mut self.entities[i];
                    e.sector = number;
                    e.layer = layer;
                    e.crossing = Some((door, side, true));
                    e.target = Some(end);
                    e.path = vec![end];
                    StepResult::Wait
                }
            }
        }
    }

    /// Where the second move of a crossing ends (`spec-navigation.md` NAV-200): a lift's lower
    /// end (type 5) ends at the door's A point, an upper end (types 4 and 6) at its B point; an
    /// ordinary passage door ends at the far side's point.
    fn nav_cross_endpoint(map: &NavMap, door: u32, far: DoorSide) -> (Fixed, Fixed) {
        let d = &map.doors[door as usize];
        let p = match d.door_type {
            5 => d.point(DoorSide::A),
            4 | 6 => d.point(DoorSide::B),
            _ => d.point(far),
        };
        (Fixed::from_f32(p.0), Fixed::from_f32(p.1))
    }

    /// The building entry (`spec-navigation.md` NAV-130 step 3, 3.9): a move to the near point,
    /// then the character is put inside.
    fn nav_enter_step(
        &mut self,
        i: usize,
        map: &NavMap,
        door: u32,
        at: (Fixed, Fixed),
    ) -> StepResult {
        let Some(d) = map.doors.get(door as usize) else {
            return StepResult::Cancel;
        };
        let Some(building) = d.building else {
            return StepResult::Cancel;
        };
        if self.entities[i].target.is_some() {
            return StepResult::Wait;
        }
        let reached = {
            let e = &self.entities[i];
            (e.x - at.0).abs().max((e.y - at.1).abs())
                < Fixed::from_int(DOOR_APPROACH_TOLERANCE + 5)
        };
        if !reached {
            let e = &mut self.entities[i];
            e.target = Some(at);
            e.path = vec![at];
            return StepResult::Wait;
        }
        let who = self.nav_passer(i);
        if !self.nav_state.admits(map, door, DoorSide::A, &who, true) {
            return StepResult::Cancel;
        }
        self.nav_put_inside(i, building);
        StepResult::End
    }

    /// Put a character inside a building (`spec-navigation.md` NAV-190): the extra layer index,
    /// the building as its sector, the B point of the building's first door, not displayed.
    pub(crate) fn nav_put_inside(&mut self, i: usize, building: u32) {
        let Some(map) = self.navmap.clone() else {
            return;
        };
        let Some(b) = map.buildings.get(building as usize) else {
            return;
        };
        let first = b.doors.first().copied();
        let (number, point) = match first.and_then(|d| map.doors.get(d as usize)) {
            Some(d) => (d.sector_b, d.point(DoorSide::B)),
            None => (u32::MAX, (0.0, 0.0)),
        };
        self.nav_state.put_inside(building, i as u32);
        let e = &mut self.entities[i];
        e.layer = map.layer_count;
        e.sector = number;
        e.x = Fixed::from_f32(point.0);
        e.y = Fixed::from_f32(point.1);
        e.active = false;
        e.target = None;
        e.path.clear();
        e.walk = None;
    }

    /// Take a character out of its building (`spec-navigation.md` NAV-191). Returns whether it
    /// was inside one.
    pub(crate) fn nav_take_out(&mut self, i: usize) -> bool {
        let Some(building) = self.nav_state.building_of(i as u32) else {
            return false;
        };
        self.nav_state.take_out(building, i as u32);
        let e = &mut self.entities[i];
        e.active = true;
        true
    }

    /// The bond crossings a step from `p` to `q` performs (`spec-navigation.md` NAV-160): the
    /// character's projection area is swapped once per bond actually crossed.
    pub(crate) fn nav_cross_bonds(&mut self, i: usize, p: (Fixed, Fixed), q: (Fixed, Fixed)) {
        let Some(map) = self.navmap.clone() else {
            return;
        };
        if map.bonds.is_empty() {
            return;
        }
        let layer = self.entities[i].layer;
        let (a, b) = ((p.0.to_f32(), p.1.to_f32()), (q.0.to_f32(), q.1.to_f32()));
        let min = (a.0.min(b.0), a.1.min(b.1));
        let max = (a.0.max(b.0), a.1.max(b.1));
        let mut seen: Vec<(i32, i32, i32, i32, u32, u32)> = Vec::new();
        for bi in map.bonds_in(layer, min, max) {
            let bond = map.bonds[bi as usize];
            let key = (
                bond.a.0,
                bond.a.1,
                bond.b.0,
                bond.b.1,
                bond.area_a,
                bond.area_b,
            );
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            let s0 = (bond.a.0 as f32, bond.a.1 as f32);
            let s1 = (bond.b.0 as f32, bond.b.1 as f32);
            if !search::segments_cross(a, b, s0, s1) {
                continue;
            }
            let area = self.entities[i].area;
            let next = if area == bond.area_a {
                Some(bond.area_b)
            } else if area == bond.area_b {
                Some(bond.area_a)
            } else {
                None
            };
            if let Some(n) = next {
                self.entities[i].area = if n == NO_AREA { NO_AREA } else { n };
            }
        }
    }
}

/// What the geometric part of NAV-140 decided, before the entity is touched.
#[derive(Debug, Clone, Copy, PartialEq)]
enum WalkDecision {
    /// Unsticking failed: the element fails.
    Fail,
    /// A free corridor: one move action from the (possibly corrected) position.
    Direct((f32, f32)),
    /// A path request: the corrected position, the request's start and the unstuck flag.
    Request((f32, f32), (f32, f32), bool),
}

/// What one step of a walk sequence decided this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepResult {
    /// The step is complete: the sequence moves on within this tick.
    Done,
    /// The step is still running: the sequence rests here until the next tick.
    Wait,
    /// The step cancelled the rest of the sequence (`spec-navigation.md` NAV-150a).
    Cancel,
    /// The sequence ends here without a failure (the building entry, the lock pick).
    End,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Instruction, Scenario};

    fn navigation() -> World {
        World::new(Scenario::Synthetic("navigation".into()), 17).unwrap()
    }

    #[test]
    fn patrol_program_waits_for_the_whole_navigation_walk() {
        let mut world = navigation();
        let e = &mut world.entities[0];
        e.kind = EntityKind::Guard;
        e.team = Team::Enemy;
        e.npc_gait = Gait::Run;
        e.program = Some(0);
        world.programs = vec![vec![
            Instruction::GoTo { x: 300, y: 100 },
            Instruction::Stop,
        ]];
        world.step(&[]);
        assert!(world.entities[0].walk.is_some());
        assert!(world.entities[0].target.is_none());
        world.step(&[]);
        assert_eq!(world.entities[0].pc, 0);
        assert_eq!(world.nav_state.queue.len(), 1);
        assert_eq!(world.nav_state.queue[0].gait, Gait::Run.tag());
        for _ in 0..120 {
            if world.entities[0].walk.is_some() {
                assert_eq!(world.entities[0].pc, 0);
            }
            world.step(&[]);
        }
        assert_eq!(
            (world.entities[0].x, world.entities[0].y),
            (Fixed::from_int(300), Fixed::from_int(100))
        );
        assert!(world.entities[0].walk.is_none());
        assert_eq!(world.entities[0].pc, 1);
    }

    #[test]
    fn underfunded_navigation_order_preserves_the_previous_walk() {
        let mut world = navigation();
        assert!(world.nav_plan(0, (Fixed::from_int(300), Fixed::from_int(100))));
        let before = world.hashes();
        let mut budget = ORDER_NAV_WORK - 1;
        assert_eq!(
            world.plan_path_with(0, (Fixed::from_int(350), Fixed::from_int(100)), &mut budget),
            Err(crate::nav::NavError::WorkExhausted)
        );
        assert_eq!(world.hashes(), before);
    }

    #[test]
    fn underfunded_tick_does_not_dispatch_or_consume_navigation() {
        let mut world = navigation();
        world.nav_plan(0, (Fixed::from_int(300), Fixed::from_int(100)));
        world.step(&[]);
        assert_eq!(world.nav_state.queue.len(), 1);
        let before = world.nav_state.clone();
        world.sim_tick_with(2);
        assert_eq!(world.nav_state, before);
    }

    #[test]
    fn mapless_world_rejects_navigation_in_a_snapshot() {
        let mut world = World::new(Scenario::Synthetic("corridor".into()), 17).unwrap();
        let before = world.hashes();
        let mut snapshot = world.snapshot(None);
        snapshot.world.nav_state.next_id = 42;
        assert!(world.restore(&snapshot).is_err());
        assert_eq!(world.hashes(), before);
    }
}
