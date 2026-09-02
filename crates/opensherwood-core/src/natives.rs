//! Engine functions the scripts call by number (`docs/formats/scb.md`, "Native call table").
//!
//! Every arm cites its row of the table with the spec's confidence. Three classes of natives:
//! *implemented* (act on the world or the VM state), *stub* (documented effect not modelled yet:
//! recorded per id in `counters.stub_natives`, arguments ignored, result 0 unless the stub policy
//! table of the spec, "Natives at load per mission", gives the value that keeps the scripts sane:
//! [`STUB_POLICY_VALUES`]) and *unknown* (no row with an effect). An unknown native is a
//! deterministic trap by default: its id is counted, the
//! running callback stops at that instruction and the script is marked `faulted`. With
//! `MissionSpec::lenient_natives` it is a recorded no-op instead (result 0) and every call is
//! appended with its arguments to `VmState::unknown_calls`, which is hashed. Inside a sequence
//! (between natives 30 and 31) the natives listed in [`SEQUENCE_ELEMENTS`] are collected as
//! elements instead of running at once; everything else runs immediately. Native 32 is the
//! sequence barrier (`vm.rs`, [`crate::vm::SeqToken`]); natives 202 (non-blocking) and 203 (a
//! page that holds its sequence) both queue a `TextRequest` whose `blocking` flag tells them apart.
//!
//! Handles. Elements, locations, paths, doors and patches are their table indices (`NONE_HANDLE`
//! = none); a location value with [`LOCATION_POINT_BIT`] set packs an actor position (native 95).

use crate::fixed::Fixed;
use crate::geom::point_in_polygon;
use crate::vm::{
    Element, LOCATION_POINT_BIT, Location, MAX_QUEUE, MISSION_VARIABLES, Message, NONE_HANDLE,
    Objective, Program, SeqElement, UnknownCall, charge_budget, location_of_point,
};

/// Saturating increment of a per-id diagnostic counter.
fn count(map: &mut std::collections::BTreeMap<u32, u64>, id: u32) {
    let c = map.entry(id).or_insert(0);
    *c = c.saturating_add(1);
}
use crate::world::{EntityKind, Gait, World};

/// Natives that are elements of a sequence when called between natives 30 and 31 (observed:
/// these ids are followed by the sync native 32 in the retail scripts; `docs/formats/scb.md`).
pub const SEQUENCE_ELEMENTS: &[u32] = &[
    32, 33, 34, 35, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 59,
    62, 64, 69, 70, 72, 73, 203, 212, 226, 243,
];

/// Natives with a documented effect that the engine records without acting on (see the spec rows
/// and the stub policy table: "0-stub safe" rows, the sequence stubs that sit before a barrier,
/// and the ids whose recorded result is a policy value, [`STUB_POLICY_VALUES`]).
pub const STUB_NATIVES: &[u32] = &[
    7, 18, 20, 24, 29, 35, 38, 39, 41, 42, 46, 47, 49, 50, 51, 52, 53, 54, 55, 59, 62, 69, 70, 72,
    73, 80, 81, 88, 89, 92, 99, 101, 102, 103, 112, 119, 125, 126, 130, 137, 143, 149, 150, 152,
    156, 163, 164, 172, 173, 177, 178, 180, 182, 186, 187, 188, 189, 191, 195, 197, 198, 199, 200,
    205, 210, 212, 213, 214, 215, 218, 219, 220, 221, 222, 223, 224, 226, 228, 229, 231, 232, 234,
    235, 243, 244, 246, 247, 248, 253, 254, 255, 256, 258, 261, 264,
];

/// Stub natives whose recorded result is not 0: the value the stub policy table of the spec
/// ("Natives at load per mission") requires so the scripts branch sanely, each pinned by
/// `policy_values_of_the_stub_table_are_pinned`. 253 / 255 (campaign character alive / present,
/// medium-low) return 1: with 0 every `CheckVictoryCondition` that tests them loses at tick 1.
/// 205 (i-th actor inside a zone, medium) returns -1 (no actor): 0 would be a map element handed
/// to 80 / 81 / 99 / 243. (128 and 240 read the real states since the stealth layer exists.)
pub const STUB_POLICY_VALUES: &[(u32, i32)] = &[(205, -1), (253, 1), (255, 1)];

/// Natives the engine implements (acting on the world or the VM state).
pub const IMPLEMENTED_NATIVES: &[u32] = &[
    0, 1, 2, 3, 4, 5, 6, 8, 9, 10, 12, 13, 26, 27, 28, 30, 31, 32, 33, 34, 43, 44, 45, 48, 56, 64,
    74, 75, 79, 85, 86, 87, 90, 93, 94, 95, 96, 97, 98, 109, 110, 111, 113, 114, 117, 118, 128,
    132, 133, 134, 135, 140, 144, 145, 159, 160, 161, 192, 193, 194, 196, 202, 203, 204, 211, 216,
    217, 233, 236, 237, 240, 245, 250,
];

/// Facing units per sixteenth of a turn: the scripts' sixteen directions (natives 93 / 94 / 133,
/// 0..=15) on the entities' 256-unit facing. Which direction is 0 is not in the spec; the engine
/// takes direction 0 as facing 0 (the `+x` axis, `world::facing_of`) and counts the same way, a
/// choice of **low** confidence pinned by `facing_natives_map_sixteen_directions_onto_facing256`.
pub const FACING_UNITS_PER_DIRECTION: i32 = 16;

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

/// Polygon of a location value (zones) borrowed from the program, if it is one.
fn polygon_in(program: &Program, value: i32) -> Option<&[(i32, i32)]> {
    if value < 0 {
        return None;
    }
    match program.locations.get(value as usize)? {
        Location::Polygon(p) => Some(p.as_slice()),
        Location::Point { .. } => None,
    }
}

impl World {
    /// Dispatch native `id` with `args`; returns its result (0 when it has none), or `None`
    /// when an unknown native traps (strict mode).
    pub(crate) fn native_call(&mut self, id: u32, args: &[i32]) -> Option<i32> {
        if native_status(id) == NativeStatus::Unknown {
            let vm = self.vm.as_mut()?;
            count(&mut vm.counters.unknown_natives, id);
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
            // 6 (index) -> location (high), 9 (index) -> path (high): same. 8 (index) ->
            // building (medium: the index itself, -1 = outdoors; the engine has no interiors,
            // see 98). 12 (patch) -> index, 13 (location) -> index (high): the inverses of 5 / 6,
            // the identity as well.
            3 | 4 | 5 | 6 | 8 | 9 | 10 | 12 | 13 => arg(args, 0),
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
                        None => {
                            vm.counters.objective_done_before_added =
                                vm.counters.objective_done_before_added.saturating_add(1);
                        }
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
            // 31 (): end a sequence (high): the collected elements become an active sequence
            // that advances independently of the others (bounded in count and total elements).
            31 => {
                if let Some(vm) = self.vm.as_mut()
                    && let Some(elements) = vm.collecting.take()
                {
                    let total: usize = vm.sequences.iter().map(|s| s.elements.len()).sum();
                    if vm.sequences.len() < MAX_QUEUE
                        && total.saturating_add(elements.len()) <= crate::vm::MAX_SEQUENCE_ELEMENTS
                    {
                        vm.sequences.push(crate::vm::Sequence {
                            elements,
                            next: 0,
                            wait: crate::vm::SeqWait::None,
                            tokens: Vec::new(),
                        });
                    }
                }
                0
            }
            // 32 (): barrier, wait for the previous elements (high): a sequence element
            // (`SeqElement::Barrier`); outside a sequence there is nothing to wait for. 56
            // (ticks): wait (high; 25 script ticks per second is the hypothesis); outside a
            // sequence there is nothing to wait for either.
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
            // 74 () -> actor: the element of this class (high). 192 () -> element: the same for
            // the non-actor classes (scrolls, objects, zones; medium): the policy table requires
            // the class's own element, since 0 would address element 0 with 193 / 194 / 113.
            74 | 192 => self
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
            // 86 (actor, actor) -> bool: the two handles are the same actor (medium): handle
            // equality.
            86 => i32::from(arg(args, 0) == arg(args, 1)),
            // 93 (element) -> dir: facing direction 0..=15 of an element (medium); a non-actor
            // element has no facing (0). 94 (actor, dir): set it (medium). 133 (actor, location,
            // dir): place the actor at the location (as 96) facing dir (medium). The direction
            // encoding is [`FACING_UNITS_PER_DIRECTION`] (low).
            93 => match self.entity_of(arg(args, 0)) {
                Some(i) => self.entities[i].facing256.rem_euclid(256) / FACING_UNITS_PER_DIRECTION,
                None => 0,
            },
            94 | 133 => {
                if let Some(entity) = self.entity_of(arg(args, 0)) {
                    let dir = if id == 133 {
                        let to = self.location_position(arg(args, 1));
                        self.vm_teleport(entity as u32, to);
                        arg(args, 2)
                    } else {
                        arg(args, 1)
                    };
                    self.entities[entity].facing256 =
                        dir.rem_euclid(16) * FACING_UNITS_PER_DIRECTION;
                }
                0
            }
            // 98 (actor, building) -> bool: actor is inside building (medium). The engine has no
            // interiors: every actor is outdoors, so the policy table's value is 1 iff the
            // building argument is the outdoors handle (-1).
            98 => i32::from(arg(args, 1) == NONE_HANDLE),
            // 85 (actor) -> bool: unusable, dead or removed (medium): dead or deactivated.
            85 => match self.entity_of(arg(args, 0)) {
                Some(i) => i32::from(!self.entities[i].alive || !self.entities[i].active),
                None => 0,
            },
            // 87 (actor) -> bool: dead (medium): the `Dead` state or `alive` cleared (no damage
            // model kills anyone yet). 88 / 89 (tied up, netted / captured: unknown / low) stay
            // stubs returning 0: no such state exists.
            87 => match self.entity_of(arg(args, 0)) {
                Some(i) => {
                    let e = &self.entities[i];
                    i32::from(!e.alive || e.ai_state == crate::ai::AiState::Dead)
                }
                None => 0,
            },
            // 90 (actor) -> bool: out of action (medium): dead, or knocked down / lying knocked
            // out (`crate::ai::AiState::out_of_action`; a soldier getting up is back: hypothesis).
            // Counted in `counters.out_of_action_true` when it reports 1 (diagnostic).
            90 => match self.entity_of(arg(args, 0)) {
                Some(i) => {
                    let e = &self.entities[i];
                    let out = !e.alive || e.ai_state.out_of_action();
                    if out && let Some(vm) = self.vm.as_mut() {
                        vm.counters.out_of_action_true =
                            vm.counters.out_of_action_true.saturating_add(1);
                    }
                    i32::from(out)
                }
                None => 0,
            },
            // 128 (actor) -> bool: able to act (medium-low): alive, active and on its feet
            // (`crate::ai::AiState::standing`); elements that are not actors can act (the policy
            // table's 1: with 0 no zone would react).
            128 => match self.entity_of(arg(args, 0)) {
                Some(i) => {
                    let e = &self.entities[i];
                    i32::from(e.alive && e.active && e.ai_state.standing())
                }
                None => 1,
            },
            // 240 (actor) -> bool: present on the map (medium-low): the entity's `active` flag;
            // other elements are present unless deactivated (113).
            240 => {
                if let Some(i) = self.entity_of(arg(args, 0)) {
                    i32::from(self.entities[i].active)
                } else {
                    let handle = arg(args, 0);
                    i32::from(
                        self.vm
                            .as_ref()
                            .is_none_or(|vm| !vm.inactive_elements.contains(&handle)),
                    )
                }
            }
            // 140 (actor, 0 / 1 / 2): the gait of the actor's patrol walks (low; the reading
            // 0 walk / 1 run / 2 sprint is the hypothesis of `stealth-and-combat.md` 2.5; the
            // engine plays a sprint as a run). Applied to the walks the waypoint program issues
            // from now on; a walk under way keeps its gait.
            140 => {
                if let Some(i) = self.entity_of(arg(args, 0)) {
                    self.entities[i].npc_gait = if arg(args, 1) == 0 {
                        Gait::Walk
                    } else {
                        Gait::Run
                    };
                }
                0
            }
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
            // 97 (actor, zone) -> bool: actor is inside zone (medium). One work unit per polygon
            // edge, charged before the test on the borrowed polygon; without the budget the
            // result is 0 and the callback aborts at its next instruction.
            97 => {
                let Some((x, y)) = self.element_position(arg(args, 0)) else {
                    return 0;
                };
                let Some(vm) = self.vm.as_mut() else {
                    return 0;
                };
                let Some(poly) = polygon_in(&vm.program, arg(args, 1)) else {
                    return 0;
                };
                if !charge_budget(&mut vm.budget, poly.len() as u64) {
                    vm.counters.budget_aborts = vm.counters.budget_aborts.saturating_add(1);
                    return 0;
                }
                i32::from(poly.len() >= 3 && point_in_polygon(x, y, poly))
            }
            // 111 () -> actor: the player's character (medium); 211 () -> actor: the main
            // player character (medium): both the first player entity. 250 (0) -> actor: player
            // character by campaign id, always 0 = the main character (medium): the policy table
            // requires 211's value (0 would be element 0).
            111 | 211 | 250 => self.player_element(0),
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
            // 134 is 0 in load-time helpers and 1 in freeze loops: both lock. Locking halts the
            // AI's current walk (low confidence, `docs/formats/scb.md` "Engine notes"): a guard
            // stops where it is, its rail program stays on the same instruction and re-issues the
            // walk when unlocked; a player character's orders are the player's and are not
            // touched.
            134 | 135 => {
                if let Some(i) = self.entity_of(arg(args, 0)) {
                    let e = &mut self.entities[i];
                    e.ai_locked = id == 134;
                    if e.ai_locked && e.kind != EntityKind::Player {
                        e.target = None;
                        e.path.clear();
                    }
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
            // 160 (location, location) -> distance (high): map pixels, rounded to nearest. The
            // differences are formed in `i64` and squared in `u128`, so any pair of positions
            // gives the same answer in debug and release; a distance beyond `i32` saturates.
            160 => match (
                self.location_position(arg(args, 0)),
                self.location_position(arg(args, 1)),
            ) {
                (Some(a), Some(b)) => {
                    let dx = u128::from((i64::from(a.0) - i64::from(b.0)).unsigned_abs());
                    let dy = u128::from((i64::from(a.1) - i64::from(b.1)).unsigned_abs());
                    // floor(sqrt(s) + 1/2) = floor((floor(sqrt(4 s)) + 1) / 2).
                    let rounded = (4 * (dx * dx + dy * dy)).isqrt().div_ceil(2);
                    i32::try_from(rounded).unwrap_or(i32::MAX)
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
            // 202 (k): show text k at once, nothing waits for it (high); 203 (k): show text k as a
            // sequence element that holds its sequence until dismissed (high). Outside a sequence
            // 203 is requested at once and still flagged blocking (the app treats it as a page).
            202 | 203 => {
                if let Some(vm) = self.vm.as_mut() {
                    let _ = vm.show_text(arg(args, 0), id == 203);
                }
                0
            }
            // 204 (zone) -> int: player actors in zone (low): count of PCs inside the polygon.
            // One work unit per entity looked at plus one per edge for every player character
            // tested, charged as the scan goes on the borrowed polygon; when the budget runs out
            // the result is 0 and the callback aborts at its next instruction.
            204 => {
                let Some(vm) = self.vm.as_mut() else {
                    return 0;
                };
                let Some(poly) = polygon_in(&vm.program, arg(args, 0)) else {
                    return 0;
                };
                if poly.len() < 3 {
                    return 0;
                }
                let edges = poly.len() as u64;
                let mut count = 0;
                for e in &self.entities {
                    let player = e.kind == EntityKind::Player && e.alive && e.active;
                    let cost = if player { 1 + edges } else { 1 };
                    if !charge_budget(&mut vm.budget, cost) {
                        vm.counters.budget_aborts = vm.counters.budget_aborts.saturating_add(1);
                        return 0;
                    }
                    if player && point_in_polygon(e.x.round(), e.y.round(), poly) {
                        count += 1;
                    }
                }
                count
            }
            // 216 () -> int: number of player characters; 217 (i) -> actor: player character i
            // (high).
            216 => self
                .entities
                .iter()
                .filter(|e| e.kind == EntityKind::Player)
                .count() as i32,
            217 => self.player_element(arg(args, 0)),
            // 236 () -> int: get the player's money; 237 (v): set it (high): one VM integer
            // (`VmState::money`, hashed and snapshotted; the HUD may read it).
            236 => self.vm.as_ref().map_or(0, |vm| vm.money),
            237 => {
                if let Some(vm) = self.vm.as_mut() {
                    vm.money = arg(args, 0);
                }
                0
            }
            // 245 () -> int: number of player characters (medium): the policy table implements
            // it as the number of live player characters (S05 starts mission variable 3 at 0 and
            // wins when it equals this value, so 0 would win at tick 1).
            245 => self
                .entities
                .iter()
                .filter(|e| e.kind == EntityKind::Player && e.alive)
                .count() as i32,
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
            // Stub natives: recorded per id (see `STUB_NATIVES`), result 0 or the policy value
            // of `STUB_POLICY_VALUES`.
            other => {
                if let Some(vm) = self.vm.as_mut() {
                    count(&mut vm.counters.stub_natives, other);
                }
                STUB_POLICY_VALUES
                    .iter()
                    .find(|(id, _)| *id == other)
                    .map_or(0, |(_, value)| *value)
            }
        }
    }

    /// The sequence element a native call collects (see [`SEQUENCE_ELEMENTS`]).
    fn sequence_element(&self, id: u32, args: &[i32]) -> Option<SeqElement> {
        let scale = self.vm.as_ref().map_or((1, 1), |vm| vm.program.wait_scale);
        Some(match id {
            // 32 (): barrier (high).
            32 => SeqElement::Barrier,
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
            // 49 / 50 / 51 (actor, anim), 52 / 53 (actor): animations (medium / low), stubs
            // whose completion token completes at once.
            49..=53 => SeqElement::Animation {
                id,
                actor: arg(args, 0),
                anim: arg(args, 1),
            },
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

    /// Natives 113 / 114: entities get their `active` flag (a deactivated entity loses its
    /// movement order and its selection); other elements are remembered.
    fn set_element_active(&mut self, handle: i32, active: bool) {
        match self.entity_of(handle) {
            Some(i) => {
                let e = &mut self.entities[i];
                e.active = active;
                if !active {
                    e.target = None;
                    e.path.clear();
                    if self.selected == Some(e.id) {
                        self.selected = None;
                    }
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

    /// Walk order for an entity through the pathfinding, charged to the VM's work budget: when
    /// the budget runs out the order is dropped (the entity stands, a barrier token completes)
    /// and `budget_aborts` counts it.
    pub(crate) fn vm_walk(&mut self, entity: u32, x: i32, y: i32) {
        let i = entity as usize;
        if i >= self.entities.len() || !self.entities[i].alive || !self.entities[i].active {
            return;
        }
        let mut budget = self.vm.as_ref().map_or(0, |vm| vm.budget);
        let planned = self.plan_path_with(i, (Fixed::from_int(x), Fixed::from_int(y)), &mut budget);
        if let Some(vm) = self.vm.as_mut() {
            vm.budget = budget;
            if planned.is_err() {
                vm.counters.budget_aborts = vm.counters.budget_aborts.saturating_add(1);
            }
        }
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
