//! Engine functions the scripts call by number (`docs/formats/scb.md`, "Native call table").
//!
//! Every arm cites its row of the table with the spec's confidence. Three classes of natives:
//! *implemented* (act on the world or the VM state), *stub* (documented effect not modelled yet:
//! recorded per id in `counters.stub_natives`, arguments ignored, result 0) and *unknown* (no row
//! with an effect). An unknown native is a deterministic trap by default: its id is counted, the
//! running callback stops at that instruction and the script is marked `faulted`. With
//! `MissionSpec::lenient_natives` it is a recorded no-op instead (result 0) and every call is
//! appended with its arguments to `VmState::unknown_calls`, which is hashed. Inside a sequence
//! (between natives 30 and 31) the natives listed in [`SEQUENCE_ELEMENTS`] are collected as
//! elements instead of running at once; everything else runs immediately.
//!
//! Handles. Elements, locations, paths, doors and patches are their table indices (`NONE_HANDLE`
//! = none); a location value with [`LOCATION_POINT_BIT`] set packs an actor position (native 95).

use crate::fixed::Fixed;
use crate::geom::point_in_polygon;
use crate::vm::{
    Element, LOCATION_POINT_BIT, Location, MAX_QUEUE, MISSION_VARIABLES, Message, NONE_HANDLE,
    Objective, SeqElement, UnknownCall, location_of_point,
};
use crate::world::{EntityKind, World};

/// Natives that are elements of a sequence when called between natives 30 and 31 (observed:
/// these ids are followed by the sync native 32 in the retail scripts; `docs/formats/scb.md`).
pub const SEQUENCE_ELEMENTS: &[u32] = &[
    33, 34, 35, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 59, 62,
    64, 69, 70, 73, 203, 226, 243,
];

/// Natives with a documented effect that the engine records without acting on (see the spec rows).
pub const STUB_NATIVES: &[u32] = &[
    35, 49, 50, 51, 52, 53, 54, 55, 59, 69, 80, 81, 87, 88, 89, 99, 102, 103, 130, 137, 140, 186,
    187, 188, 189, 191, 195, 197, 198, 218, 224, 235, 243,
];

/// Natives the engine implements (acting on the world or the VM state).
pub const IMPLEMENTED_NATIVES: &[u32] = &[
    0, 1, 2, 3, 4, 5, 6, 9, 10, 26, 27, 28, 30, 31, 32, 33, 34, 43, 44, 45, 48, 56, 64, 74, 75, 79,
    85, 90, 95, 96, 97, 109, 110, 111, 113, 114, 117, 118, 132, 134, 135, 144, 145, 159, 160, 161,
    193, 194, 196, 202, 203, 204, 211, 216, 217, 233,
];

/// Status of a native id in this engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeStatus {
    /// Acts on the world.
    Implemented,
    /// Recorded no-op with a documented effect.
    Stub,
    /// No implementation; recorded.
    Unknown,
}

/// Classify a native id.
#[must_use]
pub fn native_status(id: u32) -> NativeStatus {
    if IMPLEMENTED_NATIVES.contains(&id) {
        NativeStatus::Implemented
    } else if STUB_NATIVES.contains(&id) {
        NativeStatus::Stub
    } else {
        NativeStatus::Unknown
    }
}

/// Decode a packed actor position from a location value.
#[must_use]
pub fn unpack_point(v: i32) -> Option<(i32, i32)> {
    if v >= 0 && v & LOCATION_POINT_BIT != 0 {
        Some(((v >> 15) & 0x7fff, v & 0x7fff))
    } else {
        None
    }
}

fn arg(args: &[i32], i: usize) -> i32 {
    args.get(i).copied().unwrap_or(0)
}

impl World {
    /// Dispatch native `id` with `args`; returns its result (0 when it has none), or `None`
    /// when an unknown native traps (strict mode).
    pub(crate) fn native_call(&mut self, id: u32, args: &[i32]) -> Option<i32> {
        if native_status(id) == NativeStatus::Unknown {
            let vm = self.vm.as_mut()?;
            *vm.counters.unknown_natives.entry(id).or_insert(0) += 1;
            if !vm.lenient {
                vm.faulted = true;
                return None;
            }
            if vm.unknown_calls.len() < MAX_QUEUE {
                vm.unknown_calls.push(UnknownCall {
                    id,
                    args: args.to_vec(),
                });
            }
            return Some(0);
        }
        let collecting = self.vm.as_ref().is_some_and(|vm| vm.collecting.is_some());
        if collecting
            && SEQUENCE_ELEMENTS.contains(&id)
            && let Some(el) = self.sequence_element(id, args)
        {
            if let Some(vm) = self.vm.as_mut()
                && let Some(c) = vm.collecting.as_mut()
                && c.len() < MAX_QUEUE
            {
                c.push(el);
            }
            return Some(0);
        }
        Some(self.native_known(id, args))
    }

    /// The implemented and stub natives (`id` is never unknown here).
    fn native_known(&mut self, id: u32, args: &[i32]) -> i32 {
        match id {
            // 0 (k, v): declare mission variable k with initial value v (medium).
            // 1 (k, v): set mission variable k (high).
            0 | 1 => {
                let (k, v) = (arg(args, 0), arg(args, 1));
                if let Some(vm) = self.vm.as_mut()
                    && (0..MISSION_VARIABLES as i32).contains(&k)
                {
                    vm.mission_vars[k as usize] = v;
                }
                0
            }
            // 2 (k) -> int: get mission variable k (high).
            2 => {
                let k = arg(args, 0);
                self.vm
                    .as_ref()
                    .filter(|_| (0..MISSION_VARIABLES as i32).contains(&k))
                    .map_or(0, |vm| vm.mission_vars[k as usize])
            }
            // 3 (index) -> element (high), 10 (element) -> index (medium): handles are the
            // table indices, so both are the identity. 4 (index) -> door, 5 (index) -> patch,
            // 6 (index) -> location (high), 9 (index) -> path (high): same.
            3 | 4 | 5 | 6 | 9 | 10 => arg(args, 0),
            // 26 (k, main): add objective k, main = 1 for a primary one (high).
            26 => {
                let (k, main) = (arg(args, 0), arg(args, 1));
                if let Some(vm) = self.vm.as_mut() {
                    if let Some(o) = vm.objectives.iter_mut().find(|o| o.index == k) {
                        o.primary = main != 0;
                    } else if vm.objectives.len() < MAX_QUEUE {
                        vm.objectives.push(Objective {
                            index: k,
                            primary: main != 0,
                            done: false,
                        });
                    }
                }
                0
            }
            // 27 (k): objective k accomplished (high). An objective never added is counted, not
            // shown (the retail scripts complete sub-goals whose preconditions are not modelled
            // yet, e.g. the knight's purse through native 118).
            27 => {
                let k = arg(args, 0);
                if let Some(vm) = self.vm.as_mut() {
                    match vm.objectives.iter_mut().find(|o| o.index == k) {
                        Some(o) => o.done = true,
                        None => vm.counters.objective_done_before_added += 1,
                    }
                }
                0
            }
            // 28 (k): select the debriefing variant k (medium); stored.
            28 => {
                if let Some(vm) = self.vm.as_mut() {
                    vm.debriefing = Some(arg(args, 0));
                }
                0
            }
            // 30 (): begin a sequence (high): collect the following elements.
            30 => {
                if let Some(vm) = self.vm.as_mut() {
                    vm.collecting = Some(Vec::new());
                }
                0
            }
            // 31 (): end a sequence (high): the collected elements become an active sequence,
            // queued behind any running one.
            31 => {
                if let Some(vm) = self.vm.as_mut()
                    && let Some(elements) = vm.collecting.take()
                    && vm.sequences.len() < MAX_QUEUE
                {
                    vm.sequences.push(crate::vm::Sequence {
                        elements,
                        next: 0,
                        wait: crate::vm::SeqWait::None,
                    });
                }
                0
            }
            // 32 (): wait for the previous element (high): implicit, every element completes
            // before the next runs. 56 (ticks): wait (high; 25 script ticks per second is the
            // hypothesis); outside a sequence there is nothing to wait for.
            32 | 56 => 0,
            // 33 (location): camera to location; 34 (location): camera returns to location
            // (medium). Outside a sequence they act at once.
            33 | 34 => {
                self.vm_camera(arg(args, 0));
                0
            }
            // 43 (target, msg), 109 (target, msg): send a message (high).
            // 44 (target, msg, arg, x), 110 (target, msg, a, b): with arguments (high / low).
            43 | 44 | 109 | 110 => {
                if let Some(vm) = self.vm.as_mut() {
                    vm.send(message_of(args));
                }
                0
            }
            // 45 (actor, location, mode): move actor to location (medium); 48 (actor,
            // location): same (medium); 64 (actor, location, 0): place / send actor (low).
            45 | 48 | 64 => {
                if let Some((entity, x, y)) = self.walk_target(arg(args, 0), arg(args, 1)) {
                    self.vm_walk(entity, x, y);
                }
                0
            }
            // 74 () -> actor: the element of this class (high).
            74 => self
                .vm
                .as_ref()
                .and_then(|vm| {
                    let f = vm.frames.last()?;
                    vm.program.classes.get(f.class as usize)?.element
                })
                .map_or(NONE_HANDLE, |e| e as i32),
            // 75 () -> int: number of elements (high).
            75 => self
                .vm
                .as_ref()
                .map_or(0, |vm| vm.program.elements.len() as i32),
            // 79 (actor) -> bool: is a player character (high).
            79 => i32::from(
                self.entity_of(arg(args, 0))
                    .is_some_and(|i| self.entities[i].kind == EntityKind::Player),
            ),
            // 85 (actor) -> bool: unusable, dead or removed (medium): dead or deactivated.
            85 => match self.entity_of(arg(args, 0)) {
                Some(i) => i32::from(!self.entities[i].alive || !self.entities[i].active),
                None => 0,
            },
            // 90 (actor) -> bool: out of action (medium): dead or knocked out; until combat
            // exists only `alive` can say so.
            90 => match self.entity_of(arg(args, 0)) {
                Some(i) => i32::from(!self.entities[i].alive),
                None => 0,
            },
            // 95 (actor) -> location: location of an actor (high): its position, packed.
            95 => match self.element_position(arg(args, 0)) {
                Some((x, y)) => location_of_point(x, y),
                None => NONE_HANDLE,
            },
            // 96 (actor, location): set actor location, `n6(-1)` = off map (medium).
            96 => {
                if let Some(entity) = self.entity_of(arg(args, 0)) {
                    let to = self.location_position(arg(args, 1));
                    self.vm_teleport(entity as u32, to);
                }
                0
            }
            // 97 (actor, zone) -> bool: actor is inside zone (medium).
            97 => {
                let inside = match (
                    self.element_position(arg(args, 0)),
                    self.polygon_of(arg(args, 1)),
                ) {
                    (Some((x, y)), Some(poly)) => poly.len() >= 3 && point_in_polygon(x, y, &poly),
                    _ => false,
                };
                i32::from(inside)
            }
            // 111 () -> actor: the player's character (medium); 211 () -> actor: the main
            // player character (medium): both the first player entity.
            111 | 211 => self.player_element(0),
            // 113 / 114 (element): deactivate / activate an element (high).
            113 | 114 => {
                self.set_element_active(arg(args, 0), id == 114);
                0
            }
            // 117 (element, attr, value): set an attribute; 118 (element, attr) -> value (medium).
            117 => {
                if let Some(vm) = self.vm.as_mut() {
                    vm.set_attribute(arg(args, 0), arg(args, 1), arg(args, 2));
                }
                0
            }
            118 => self
                .vm
                .as_ref()
                .map_or(0, |vm| vm.attribute(arg(args, 0), arg(args, 1))),
            // 132 (actor, path): assign patrol path (high): the compiled rail program.
            132 => {
                let path = arg(args, 1);
                let program = self.vm.as_ref().and_then(|vm| {
                    usize::try_from(path)
                        .ok()
                        .and_then(|p| vm.paths.get(p).copied())
                });
                if let Some(i) = self.entity_of(arg(args, 0)) {
                    let e = &mut self.entities[i];
                    e.program = program.flatten();
                    e.pc = 0;
                    e.target = None;
                    e.path.clear();
                    e.wait_ticks = 0;
                }
                0
            }
            // 134 (actor, flag): lock the actor's AI; 135 (actor): unlock (medium). The flag of
            // 134 is 0 in load-time helpers and 1 in freeze loops: both lock.
            134 | 135 => {
                if let Some(i) = self.entity_of(arg(args, 0)) {
                    self.entities[i].ai_locked = id == 134;
                }
                0
            }
            // 144 (patch) -> bool: patch active; 145 (patch): activate (medium).
            144 => i32::from(
                self.vm
                    .as_ref()
                    .is_some_and(|vm| vm.patches.contains(&arg(args, 0))),
            ),
            145 => {
                if let Some(vm) = self.vm.as_mut()
                    && vm.patches.len() < MAX_QUEUE
                {
                    vm.patches.insert(arg(args, 0));
                }
                0
            }
            // 159 () -> location: off-map location (low).
            159 => NONE_HANDLE,
            // 160 (location, location) -> distance (high): map pixels, rounded.
            160 => match (
                self.location_position(arg(args, 0)),
                self.location_position(arg(args, 1)),
            ) {
                (Some(a), Some(b)) => {
                    Fixed::length(Fixed::from_int(a.0 - b.0), Fixed::from_int(a.1 - b.1)).round()
                }
                _ => i32::MAX,
            },
            // 161 (n) -> int: random number in 0..n (medium), `script` RNG stream.
            161 => {
                let n = arg(args, 0);
                self.vm
                    .as_mut()
                    .map_or(0, |vm| vm.rng.below(n.max(0) as u32) as i32)
            }
            // 193 (element) -> state; 194 (element, state): element state (low).
            193 => self
                .vm
                .as_ref()
                .map_or(0, |vm| vm.states.get(&arg(args, 0)).copied().unwrap_or(0)),
            194 => {
                if let Some(vm) = self.vm.as_mut()
                    && (vm.states.contains_key(&arg(args, 0)) || vm.states.len() < MAX_QUEUE * 16)
                {
                    vm.states.insert(arg(args, 0), arg(args, 1));
                }
                0
            }
            // 196 (k, flags): availability of player action k (low); stored.
            196 => {
                if let Some(vm) = self.vm.as_mut()
                    && (vm.actions.contains_key(&arg(args, 0)) || vm.actions.len() < MAX_QUEUE)
                {
                    vm.actions.insert(arg(args, 0), arg(args, 1));
                }
                0
            }
            // 202 (k): show text k at once (high); 203 (k): show text k as a sequence element
            // that waits for its dismissal (high). Outside a sequence 203 does not block.
            202 | 203 => {
                if let Some(vm) = self.vm.as_mut() {
                    vm.show_text(arg(args, 0), id == 203);
                }
                0
            }
            // 204 (zone) -> int: player actors in zone (low): count of PCs inside the polygon.
            204 => match self.polygon_of(arg(args, 0)) {
                Some(poly) if poly.len() >= 3 => self
                    .entities
                    .iter()
                    .filter(|e| e.kind == EntityKind::Player && e.alive && e.active)
                    .filter(|e| point_in_polygon(e.x.round(), e.y.round(), &poly))
                    .count() as i32,
                _ => 0,
            },
            // 216 () -> int: number of player characters; 217 (i) -> actor: player character i
            // (high).
            216 => self
                .entities
                .iter()
                .filter(|e| e.kind == EntityKind::Player)
                .count() as i32,
            217 => self.player_element(arg(args, 0)),
            // 233 (actor, element): actor goes to element (medium): a walk order to its position.
            233 => {
                if let (Some(entity), Some((x, y))) = (
                    self.entity_of(arg(args, 0)),
                    self.element_position(arg(args, 1)),
                ) {
                    self.vm_walk(entity as u32, x, y);
                }
                0
            }
            // Stub natives: recorded per id (see `STUB_NATIVES`).
            other => {
                if let Some(vm) = self.vm.as_mut() {
                    *vm.counters.stub_natives.entry(other).or_insert(0) += 1;
                }
                0
            }
        }
    }

    /// The sequence element a native call collects (see [`SEQUENCE_ELEMENTS`]).
    fn sequence_element(&self, id: u32, args: &[i32]) -> Option<SeqElement> {
        let scale = self.vm.as_ref().map_or((1, 1), |vm| vm.program.wait_scale);
        Some(match id {
            // 203 (k): text page (high).
            203 => SeqElement::Text(arg(args, 0)),
            // 56 (ticks): wait, scaled from script ticks to world ticks (high).
            56 => {
                let n = u64::from(arg(args, 0).max(0) as u32);
                let ticks = n * u64::from(scale.0) / u64::from(scale.1);
                SeqElement::Wait(ticks.min(u64::from(u32::MAX)) as u32)
            }
            33 | 34 => SeqElement::Camera(arg(args, 0)),
            43 | 44 => SeqElement::Message(message_of(args)),
            45 | 48 | 64 => {
                let (entity, x, y) = self.walk_target(arg(args, 0), arg(args, 1))?;
                SeqElement::Walk { entity, x, y }
            }
            233 => {
                let entity = self.entity_of(arg(args, 0))? as u32;
                let (x, y) = self.element_position(arg(args, 1))?;
                SeqElement::Walk { entity, x, y }
            }
            other => SeqElement::Stub { id: other },
        })
    }

    /// Entity index of an element handle, if it is a modelled actor.
    pub(crate) fn entity_of(&self, handle: i32) -> Option<usize> {
        match self.vm.as_ref()?.element(handle)? {
            Element::Actor(i) if (i as usize) < self.entities.len() => Some(i as usize),
            _ => None,
        }
    }

    /// Map position of an element (actors, objects, scrolls, polygons).
    fn element_position(&self, handle: i32) -> Option<(i32, i32)> {
        let vm = self.vm.as_ref()?;
        match vm.element(handle)? {
            Element::Actor(i) => {
                let e = self.entities.get(i as usize)?;
                Some((e.x.round(), e.y.round()))
            }
            Element::Object { x, y } | Element::Scroll { x, y } => Some((x, y)),
            Element::Polygon(l) => Some(vm.program.locations.get(l as usize)?.position()),
            Element::Map(_) | Element::Unmodelled(_) => None,
        }
    }

    /// Map position of a location value (table index or packed point).
    pub(crate) fn location_position(&self, value: i32) -> Option<(i32, i32)> {
        if let Some(p) = unpack_point(value) {
            return Some(p);
        }
        if value < 0 {
            return None;
        }
        Some(
            self.vm
                .as_ref()?
                .program
                .locations
                .get(value as usize)?
                .position(),
        )
    }

    /// Polygon of a location value (zones), if it is one.
    fn polygon_of(&self, value: i32) -> Option<Vec<(i32, i32)>> {
        if value < 0 {
            return None;
        }
        match self.vm.as_ref()?.program.locations.get(value as usize)? {
            Location::Polygon(p) => Some(p.clone()),
            Location::Point { .. } => None,
        }
    }

    /// Element handle of the `i`-th player character in entity order.
    fn player_element(&self, i: i32) -> i32 {
        let Some(vm) = self.vm.as_ref() else {
            return NONE_HANDLE;
        };
        if i < 0 {
            return NONE_HANDLE;
        }
        self.entities
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind == EntityKind::Player)
            .nth(i as usize)
            .map_or(NONE_HANDLE, |(idx, _)| {
                vm.program.element_of_entity(idx as u32)
            })
    }

    fn walk_target(&self, actor: i32, location: i32) -> Option<(u32, i32, i32)> {
        let entity = self.entity_of(actor)? as u32;
        let (x, y) = self.location_position(location)?;
        Some((entity, x, y))
    }

    /// Natives 113 / 114: entities get their `active` flag; other elements are remembered.
    fn set_element_active(&mut self, handle: i32, active: bool) {
        match self.entity_of(handle) {
            Some(i) => {
                let e = &mut self.entities[i];
                e.active = active;
                if !active {
                    e.target = None;
                    e.path.clear();
                }
            }
            None => {
                if let Some(vm) = self.vm.as_mut()
                    && handle >= 0
                {
                    if active {
                        vm.inactive_elements.remove(&handle);
                    } else if vm.inactive_elements.len() < MAX_QUEUE * 16 {
                        vm.inactive_elements.insert(handle);
                    }
                }
            }
        }
    }

    /// Natives 33 / 34: centre the camera on the location and record it for the app.
    pub(crate) fn vm_camera(&mut self, location: i32) {
        let Some((x, y)) = self.location_position(location) else {
            return;
        };
        self.center_camera_on(x, y);
        if let Some(vm) = self.vm.as_mut() {
            vm.camera_target = Some((x, y));
        }
    }

    /// Walk order for an entity through the pathfinding.
    pub(crate) fn vm_walk(&mut self, entity: u32, x: i32, y: i32) {
        let i = entity as usize;
        if i >= self.entities.len() || !self.entities[i].alive || !self.entities[i].active {
            return;
        }
        self.plan_path(i, (Fixed::from_int(x), Fixed::from_int(y)));
    }

    /// Native 96: teleport; `None` puts the entity off the map (deactivated).
    pub(crate) fn vm_teleport(&mut self, entity: u32, to: Option<(i32, i32)>) {
        let Some(e) = self.entities.get_mut(entity as usize) else {
            return;
        };
        e.target = None;
        e.path.clear();
        match to {
            Some((x, y)) => {
                let w = self.map_size.0 as i32;
                let h = self.map_size.1 as i32;
                e.x = Fixed::from_int(x.clamp(0, w));
                e.y = Fixed::from_int(y.clamp(0, h));
            }
            None => e.active = false,
        }
    }
}

/// Message of natives 43 / 44 / 109 / 110: `(target, msg[, arg[, arg2]])`.
fn message_of(args: &[i32]) -> Message {
    Message {
        target: arg(args, 0),
        id: arg(args, 1),
        arg: arg(args, 2),
        arg2: arg(args, 3),
    }
}
