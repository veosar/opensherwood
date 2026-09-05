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
//! Signatures. [`NATIVE_SIGNATURES`] is the one table of `id -> (arity, returns_value,
//! read_in_corpus)` for every implemented and stub native: the arity from the arity column of
//! the spec's rows (the corpus has exactly one arity per id), `returns_value` from the row's
//! `-> result` notation (the semantic contract: the value the call leaves, which the dispatcher
//! honours) and `read_in_corpus` from the observation that a `0x0d` follows the call in the
//! retail scripts (a corpus fact, kept apart from the contract; today the two agree for every
//! id). The translator checks the table for diagnostics, `Program::validate` refuses a program
//! whose call sites disagree with it (the trust boundary: a wrong argument count, or a result
//! slot on a native that leaves no value), and the dispatcher checks the arity again: a call
//! whose argument count differs from the signature is a deterministic trap like an unknown
//! native (counted in `counters.arity_mismatches`), so no required argument ever defaults to 0.
//!
//! Hypotheses and taint (ADR-0008, "Hypotheses and taint"; `vm.rs`, [`Assumption`]).
//! [`NATIVE_TAINT`] classifies every known native ([`Taint`]): an *observed* native records
//! nothing on the call (its value is read from modelled state or its effect goes through the
//! same code paths the player's orders use); a *policy* native records
//! `Assumption::Policy(id)` on every call (the engine's reading of the row is a choice the spec
//! does not settle: 98, 128, 140, 245 and the rest of the policy list); a *branch* native
//! records `Policy(id)` from its own arm on the branch that is a policy; an *effect* stub
//! records `Assumption::StubResult(id)` on every call (its documented effect is not modelled),
//! and a *presentation* stub records nothing on the call, each with its one-line proof in the
//! table. A lenient unknown call records `Assumption::UnknownNative(id)`. Reading any stub's
//! fabricated result records `StubResult(id)` (`vm.rs`). Natives 90 / 128 reporting a knock-out
//! record `Assumption::KnockOut`, native 56 outside a sequence `Assumption::TickRate`.
//!
//! Handles. Elements, locations, paths, doors and patches are their table indices (`NONE_HANDLE`
//! = none); a location value with [`LOCATION_POINT_BIT`] set packs an actor position (native 95).

use crate::ai::ActorStatus;
use crate::fixed::Fixed;
use crate::geom::point_in_polygon;
use crate::vm::{
    Assumption, Element, Fault, LOCATION_POINT_BIT, Location, MAX_QUEUE, MISSION_VARIABLES,
    Message, NONE_HANDLE, Objective, Program, SeqElement, UnknownCall, charge_budget,
    location_of_point,
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
    156, 163, 164, 165, 166, 170, 172, 173, 174, 177, 178, 180, 182, 186, 187, 188, 189, 191, 195,
    197, 198, 199, 200, 205, 210, 212, 213, 214, 215, 218, 219, 220, 221, 222, 223, 224, 226, 228,
    229, 231, 232, 234, 239, 243, 244, 246, 247, 248, 249, 253, 254, 255, 256, 258, 261, 264,
];

/// Stub natives whose recorded result is not 0: the value the stub policy table of the spec
/// ("Natives at load per mission") requires so the scripts branch sanely, each pinned by
/// `policy_values_of_the_stub_table_are_pinned`. 253 / 255 (campaign character alive / present,
/// medium-low) return 1: with 0 every `CheckVictoryCondition` that tests them loses at tick 1.
/// 205 (i-th actor inside a zone, medium) returns -1 (no actor): 0 would be a map element handed
/// to 80 / 81 / 99 / 243. 174 (the mission team's size limit, `docs/formats/sherwood-hub.md`)
/// returns 5, a positive limit, so the deployment zone admits a character (the team size 163
/// stays 0). (128 and 240 read the real states since the stealth layer exists.)
pub const STUB_POLICY_VALUES: &[(u32, i32)] = &[(174, 5), (205, -1), (253, 1), (255, 1)];

/// Natives the engine implements (acting on the world or the VM state).
pub const IMPLEMENTED_NATIVES: &[u32] = &[
    0, 1, 2, 3, 4, 5, 6, 8, 9, 10, 12, 13, 26, 27, 28, 30, 31, 32, 33, 34, 43, 44, 45, 48, 56, 64,
    74, 75, 79, 85, 86, 87, 90, 93, 94, 95, 96, 97, 98, 109, 110, 111, 113, 114, 117, 118, 128,
    132, 133, 134, 135, 140, 144, 145, 159, 160, 161, 192, 193, 194, 196, 202, 203, 204, 211, 216,
    217, 233, 235, 236, 237, 240, 245, 250,
];

/// What calling a known native records in the taint model (module documentation, "Hypotheses
/// and taint"; [`NATIVE_TAINT`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taint {
    /// Implemented; the value is read from modelled state or the effect goes through the code
    /// paths the player's orders use: nothing is recorded on the call.
    Observed,
    /// Implemented; the engine's reading of the row is a policy the spec does not settle:
    /// every call records `Assumption::Policy(id)`.
    Policy,
    /// Implemented; one of its branches is a policy: the arm records `Assumption::Policy(id)`
    /// itself when it takes that branch.
    Branch,
    /// A recorded stub with a documented effect the engine does not model: every call records
    /// `Assumption::StubResult(id)`.
    Effect,
    /// A recorded stub proven presentation-only (the justification is in the table): the call
    /// records nothing; reading its result, if it has one, still records `StubResult(id)`.
    Presentation,
}

/// The taint class of every implemented and stub native, one row per id in ascending order
/// (the same ids as [`NATIVE_SIGNATURES`]; pinned by `taint_table_covers_the_known_natives`).
/// Policy rows, with the choice the engine made: 8 (a building index without interiors), 44 /
/// 110 (the third / fourth message arguments are ignored: unknown, perhaps a delay), 45 (the
/// mode is ignored), 64 (a walk; the row is low), 93 / 94 / 133 (the direction encoding
/// `FACING_UNITS_PER_DIRECTION` is low), 98 (every actor is outdoors), 128 (the able-to-act
/// reading, medium-low; non-actors always can act), 134 / 135 (locking halts the walk: low),
/// 140 (0 walk / else run: low), 159 (the off-map location: low), 161 (the engine's generator,
/// not the original's), 193 / 194 / 196 (stored values whose meaning is low), 204 (the count
/// of player characters in the polygon: low), 235 (an item is "taken" once a player character
/// picked it up, `VmState::taken_items`; the row is low and the pickup itself a hypothesis),
/// 245 (the number of live player characters).
/// Branch rows: 111 / 211 / 250 record when more than one player character exists (which one
/// is "main" is observed only with a single one), 240 for a non-actor element (present unless
/// deactivated: the policy table's 1). Presentation rows, each proven by its row of
/// `docs/formats/scb.md`: 62 (an expression on the actor's face: nothing reads it), 69 (a
/// remark / gesture before a dialogue line: a voice line, nothing reads it), 149 / 150 (a level
/// sound played once / at start: audio only), 243 (a highlight on the actor a cutscene text
/// talks about, always inside a sequence: a HUD effect nothing reads). The Sherwood hub's team
/// natives (`docs/formats/sherwood-hub.md`, effect stubs until the team logic exists): 165 /
/// 166 add / remove a character to / from the mission team (no value), 170 "the team satisfies
/// the mission's requirements" (0), 172 the selected level code (0x4248 = the two ASCII letters
/// of a level code; 0 = none: the stub's 0), 173 a 0 / 1 state gating the team limit and the
/// camp helpers (0), 174 the team size limit (the policy value 5), 239 the deployment scroll's
/// `IsTaken`-style helper (no value), 249 the size of the team to send (0: message 1000's loop
/// runs zero times).
pub const NATIVE_TAINT: &[(u32, Taint)] = &[
    (0, Taint::Observed),
    (1, Taint::Observed),
    (2, Taint::Observed),
    (3, Taint::Observed),
    (4, Taint::Observed),
    (5, Taint::Observed),
    (6, Taint::Observed),
    (7, Taint::Effect),
    (8, Taint::Policy),
    (9, Taint::Observed),
    (10, Taint::Observed),
    (12, Taint::Observed),
    (13, Taint::Observed),
    (18, Taint::Effect),
    (20, Taint::Effect),
    (24, Taint::Effect),
    (26, Taint::Observed),
    (27, Taint::Observed),
    (28, Taint::Observed),
    (29, Taint::Effect),
    (30, Taint::Observed),
    (31, Taint::Observed),
    (32, Taint::Observed),
    (33, Taint::Observed),
    (34, Taint::Observed),
    (35, Taint::Effect),
    (38, Taint::Effect),
    (39, Taint::Effect),
    (41, Taint::Effect),
    (42, Taint::Effect),
    (43, Taint::Observed),
    (44, Taint::Policy),
    (45, Taint::Policy),
    (46, Taint::Effect),
    (47, Taint::Effect),
    (48, Taint::Observed),
    (49, Taint::Effect),
    (50, Taint::Effect),
    (51, Taint::Effect),
    (52, Taint::Effect),
    (53, Taint::Effect),
    (54, Taint::Effect),
    (55, Taint::Effect),
    (56, Taint::Observed),
    (59, Taint::Effect),
    (62, Taint::Presentation),
    (64, Taint::Policy),
    (69, Taint::Presentation),
    (70, Taint::Effect),
    (72, Taint::Effect),
    (73, Taint::Effect),
    (74, Taint::Observed),
    (75, Taint::Observed),
    (79, Taint::Observed),
    (80, Taint::Effect),
    (81, Taint::Effect),
    (85, Taint::Observed),
    (86, Taint::Observed),
    (87, Taint::Observed),
    (88, Taint::Effect),
    (89, Taint::Effect),
    (90, Taint::Observed),
    (92, Taint::Effect),
    (93, Taint::Policy),
    (94, Taint::Policy),
    (95, Taint::Observed),
    (96, Taint::Observed),
    (97, Taint::Observed),
    (98, Taint::Policy),
    (99, Taint::Effect),
    (101, Taint::Effect),
    (102, Taint::Effect),
    (103, Taint::Effect),
    (109, Taint::Observed),
    (110, Taint::Policy),
    (111, Taint::Branch),
    (112, Taint::Effect),
    (113, Taint::Observed),
    (114, Taint::Observed),
    (117, Taint::Observed),
    (118, Taint::Observed),
    (119, Taint::Effect),
    (125, Taint::Effect),
    (126, Taint::Effect),
    (128, Taint::Policy),
    (130, Taint::Effect),
    (132, Taint::Observed),
    (133, Taint::Policy),
    (134, Taint::Policy),
    (135, Taint::Policy),
    (137, Taint::Effect),
    (140, Taint::Policy),
    (143, Taint::Effect),
    (144, Taint::Observed),
    (145, Taint::Observed),
    (149, Taint::Presentation),
    (150, Taint::Presentation),
    (152, Taint::Effect),
    (156, Taint::Effect),
    (159, Taint::Policy),
    (160, Taint::Observed),
    (161, Taint::Policy),
    (163, Taint::Effect),
    (164, Taint::Effect),
    (165, Taint::Effect),
    (166, Taint::Effect),
    (170, Taint::Effect),
    (172, Taint::Effect),
    (173, Taint::Effect),
    (174, Taint::Effect),
    (177, Taint::Effect),
    (178, Taint::Effect),
    (180, Taint::Effect),
    (182, Taint::Effect),
    (186, Taint::Effect),
    (187, Taint::Effect),
    (188, Taint::Effect),
    (189, Taint::Effect),
    (191, Taint::Effect),
    (192, Taint::Observed),
    (193, Taint::Policy),
    (194, Taint::Policy),
    (195, Taint::Effect),
    (196, Taint::Policy),
    (197, Taint::Effect),
    (198, Taint::Effect),
    (199, Taint::Effect),
    (200, Taint::Effect),
    (202, Taint::Observed),
    (203, Taint::Observed),
    (204, Taint::Policy),
    (205, Taint::Effect),
    (210, Taint::Effect),
    (211, Taint::Branch),
    (212, Taint::Effect),
    (213, Taint::Effect),
    (214, Taint::Effect),
    (215, Taint::Effect),
    (216, Taint::Observed),
    (217, Taint::Observed),
    (218, Taint::Effect),
    (219, Taint::Effect),
    (220, Taint::Effect),
    (221, Taint::Effect),
    (222, Taint::Effect),
    (223, Taint::Effect),
    (224, Taint::Effect),
    (226, Taint::Effect),
    (228, Taint::Effect),
    (229, Taint::Effect),
    (231, Taint::Effect),
    (232, Taint::Effect),
    (233, Taint::Observed),
    (234, Taint::Effect),
    (235, Taint::Policy),
    (236, Taint::Observed),
    (237, Taint::Observed),
    (239, Taint::Effect),
    (240, Taint::Branch),
    (243, Taint::Presentation),
    (244, Taint::Effect),
    (245, Taint::Policy),
    (246, Taint::Effect),
    (247, Taint::Effect),
    (248, Taint::Effect),
    (249, Taint::Effect),
    (250, Taint::Branch),
    (253, Taint::Effect),
    (254, Taint::Effect),
    (255, Taint::Effect),
    (256, Taint::Effect),
    (258, Taint::Effect),
    (261, Taint::Effect),
    (264, Taint::Effect),
];

/// The taint class of a known native; `None` for an unknown id.
#[must_use]
pub fn native_taint(id: u32) -> Option<Taint> {
    NATIVE_TAINT
        .binary_search_by_key(&id, |&(i, _)| i)
        .ok()
        .map(|k| NATIVE_TAINT[k].1)
}

/// Signature of a native: the number of arguments its call sites push, whether the call leaves
/// a value (the semantic contract) and whether the retail corpus reads one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeSignature {
    /// Arguments popped by the call.
    pub arity: u32,
    /// The call leaves a value the script may read (the row's `-> result`); a result slot on a
    /// native without one is refused by `Program::validate`.
    pub returns_value: bool,
    /// A `0x0d` follows the call somewhere in the retail corpus (the observation, kept apart
    /// from the contract).
    pub read_in_corpus: bool,
}

/// The signature of every implemented and stub native (`id, arity, returns_value,
/// read_in_corpus`), one row per id in ascending order, from the arity and result columns of
/// the native call table of `docs/formats/scb.md` (130 and 137 have arity-only rows: their
/// effect is unknown, the arity is the corpus observation). Pinned by
/// `signature_table_covers_the_known_natives`.
pub const NATIVE_SIGNATURES: &[(u32, u32, bool, bool)] = &[
    (0, 2, false, false),
    (1, 2, false, false),
    (2, 1, true, true),
    (3, 1, true, true),
    (4, 1, true, true),
    (5, 1, true, true),
    (6, 1, true, true),
    (7, 1, true, true),
    (8, 1, true, true),
    (9, 1, true, true),
    (10, 1, true, true),
    (12, 1, true, true),
    (13, 1, true, true),
    (18, 1, false, false),
    (20, 1, false, false),
    (24, 2, false, false),
    (26, 2, false, false),
    (27, 1, false, false),
    (28, 1, false, false),
    (29, 0, false, false),
    (30, 0, false, false),
    (31, 0, false, false),
    (32, 0, false, false),
    (33, 1, false, false),
    (34, 1, false, false),
    (35, 1, false, false),
    (38, 2, false, false),
    (39, 1, false, false),
    (41, 1, false, false),
    (42, 2, false, false),
    (43, 2, false, false),
    (44, 4, false, false),
    (45, 3, false, false),
    (46, 4, false, false),
    (47, 4, false, false),
    (48, 2, false, false),
    (49, 2, false, false),
    (50, 2, false, false),
    (51, 2, false, false),
    (52, 1, false, false),
    (53, 1, false, false),
    (54, 0, false, false),
    (55, 0, false, false),
    (56, 1, false, false),
    (59, 3, false, false),
    (62, 3, false, false),
    (64, 3, false, false),
    (69, 2, false, false),
    (70, 6, false, false),
    (72, 1, false, false),
    (73, 1, false, false),
    (74, 0, true, true),
    (75, 0, true, true),
    (79, 1, true, true),
    (80, 1, true, true),
    (81, 1, true, true),
    (85, 1, true, true),
    (86, 2, true, true),
    (87, 1, true, true),
    (88, 1, true, true),
    (89, 1, true, true),
    (90, 1, true, true),
    (92, 2, false, false),
    (93, 1, true, true),
    (94, 2, false, false),
    (95, 1, true, true),
    (96, 2, false, false),
    (97, 2, true, true),
    (98, 2, true, true),
    (99, 1, false, false),
    (101, 1, true, true),
    (102, 3, false, false),
    (103, 1, false, false),
    (109, 2, false, false),
    (110, 4, false, false),
    (111, 0, true, true),
    (112, 1, false, false),
    (113, 1, false, false),
    (114, 1, false, false),
    (117, 3, false, false),
    (118, 2, true, true),
    (119, 0, true, true),
    (125, 2, false, false),
    (126, 1, true, true),
    (128, 1, true, true),
    (130, 3, false, false),
    (132, 2, false, false),
    (133, 3, false, false),
    (134, 2, false, false),
    (135, 1, false, false),
    (137, 2, false, false),
    (140, 2, false, false),
    (143, 2, false, false),
    (144, 1, true, true),
    (145, 1, false, false),
    (149, 1, false, false),
    (150, 1, false, false),
    (152, 1, false, false),
    (156, 2, false, false),
    (159, 0, true, true),
    (160, 2, true, true),
    (161, 1, true, true),
    (163, 0, true, true),
    (164, 1, true, true),
    (165, 1, false, false),
    (166, 1, false, false),
    (170, 0, true, true),
    (172, 0, true, true),
    (173, 0, true, true),
    (174, 0, true, true),
    (177, 2, false, false),
    (178, 1, false, false),
    (180, 2, false, false),
    (182, 1, true, true),
    (186, 2, false, false),
    (187, 2, false, false),
    (188, 2, false, false),
    (189, 2, false, false),
    (191, 2, false, false),
    (192, 0, true, true),
    (193, 1, true, true),
    (194, 2, false, false),
    (195, 1, true, true),
    (196, 2, false, false),
    (197, 2, true, true),
    (198, 3, false, false),
    (199, 3, false, false),
    (200, 2, false, false),
    (202, 1, false, false),
    (203, 1, false, false),
    (204, 1, true, true),
    (205, 2, true, true),
    (210, 1, true, true),
    (211, 0, true, true),
    (212, 4, false, false),
    (213, 3, true, true),
    (214, 1, false, false),
    (215, 1, true, true),
    (216, 0, true, true),
    (217, 1, true, true),
    (218, 2, false, false),
    (219, 1, false, false),
    (220, 1, false, false),
    (221, 1, true, true),
    (222, 1, true, true),
    (223, 1, true, true),
    (224, 4, false, false),
    (226, 1, false, false),
    (228, 3, false, false),
    (229, 1, false, false),
    (231, 1, true, true),
    (232, 1, false, false),
    (233, 2, false, false),
    (234, 0, true, true),
    (235, 1, true, true),
    (236, 0, true, true),
    (237, 1, false, false),
    (239, 0, false, false),
    (240, 1, true, true),
    (243, 1, false, false),
    (244, 2, false, false),
    (245, 0, true, true),
    (246, 1, true, true),
    (247, 1, false, false),
    (248, 1, true, true),
    (249, 0, true, true),
    (250, 1, true, true),
    (253, 1, true, true),
    (254, 2, false, false),
    (255, 1, true, true),
    (256, 1, true, true),
    (258, 1, true, true),
    (261, 0, true, true),
    (264, 3, false, false),
];

/// The signature of a known (implemented or stub) native; `None` for an unknown id, which has no
/// signature to check (it traps in strict mode, is recorded in lenient mode).
#[must_use]
pub fn native_signature(id: u32) -> Option<NativeSignature> {
    NATIVE_SIGNATURES
        .binary_search_by_key(&id, |&(i, _, _, _)| i)
        .ok()
        .map(|k| {
            let (_, arity, returns_value, read_in_corpus) = NATIVE_SIGNATURES[k];
            NativeSignature {
                arity,
                returns_value,
                read_in_corpus,
            }
        })
}

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

/// Argument `i` of a call. `native_call` checks the argument count against the signature before
/// dispatching, so every index an arm uses is present; the fallback is unreachable and exists
/// only to keep the accessor total.
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
    /// when the call traps: an unknown native in strict mode, or an argument count that differs
    /// from the native's signature (in either mode: a required argument never defaults).
    pub(crate) fn native_call(&mut self, id: u32, args: &[i32]) -> Option<i32> {
        if native_status(id) == NativeStatus::Unknown {
            let vm = self.vm.as_mut()?;
            count(&mut vm.counters.unknown_natives, id);
            if !vm.lenient {
                vm.set_fault(Fault::UnknownNative(id));
                return None;
            }
            // The fabricated 0 and the omitted effect are a hypothesis on every call.
            vm.assume(Assumption::UnknownNative(id));
            if vm.unknown_calls.len() < MAX_QUEUE {
                vm.unknown_calls.push(UnknownCall {
                    id,
                    args: args.to_vec(),
                });
            }
            return Some(0);
        }
        if native_signature(id).is_none_or(|s| s.arity as usize != args.len()) {
            let vm = self.vm.as_mut()?;
            count(&mut vm.counters.arity_mismatches, id);
            vm.set_fault(Fault::ArityMismatch(id));
            return None;
        }
        // The taint of the call itself (`NATIVE_TAINT`), recorded before the native runs or is
        // collected as a sequence element.
        if let Some(vm) = self.vm.as_mut() {
            match native_taint(id) {
                Some(Taint::Policy) => vm.assume(Assumption::Policy(id)),
                Some(Taint::Effect) => vm.assume(Assumption::StubResult(id)),
                Some(Taint::Observed | Taint::Branch | Taint::Presentation) | None => {}
            }
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
            // sequence there is nothing to wait for either, which rests on the same reading.
            32 => 0,
            56 => {
                if let Some(vm) = self.vm.as_mut() {
                    vm.assume(Assumption::TickRate);
                }
                0
            }
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
                    self.vm_touch_entity(entity);
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
            // The status predicates 85 / 87 / 90 / 128 / 240 all derive from one state function,
            // [`ActorStatus::of`] (`ai.rs`), so they cannot contradict each other.
            // 85 (actor) -> bool: unusable, dead or removed (medium): dead or deactivated.
            85 => match self.entity_of(arg(args, 0)) {
                Some(i) => {
                    let s = ActorStatus::of(&self.entities[i]);
                    i32::from(s.dead || !s.present)
                }
                None => 0,
            },
            // 87 (actor) -> bool: dead (medium): killed in the melee (`ai.rs`, hit points at
            // 0, from the tick of the blow on). 88 / 89 (tied up, netted / captured: unknown /
            // low) stay stubs returning 0: no such state exists.
            87 => match self.entity_of(arg(args, 0)) {
                Some(i) => i32::from(ActorStatus::of(&self.entities[i]).dead),
                None => 0,
            },
            // 90 (actor) -> bool: out of action (medium): dead, or knocked down / lying knocked
            // out (a soldier getting up is back: hypothesis). Counted in
            // `counters.out_of_action_true` when it reports 1 (diagnostic); a knock-out reported
            // here reaches the script, so it records `Assumption::KnockOut`.
            90 => match self.entity_of(arg(args, 0)) {
                Some(i) => {
                    let s = ActorStatus::of(&self.entities[i]);
                    if s.out_of_action
                        && let Some(vm) = self.vm.as_mut()
                    {
                        vm.counters.out_of_action_true =
                            vm.counters.out_of_action_true.saturating_add(1);
                        if s.knocked_out {
                            vm.assumptions.insert(Assumption::KnockOut);
                        }
                    }
                    i32::from(s.out_of_action)
                }
                None => 0,
            },
            // 128 (actor) -> bool: able to act (medium-low): alive, active and on its feet;
            // elements that are not actors can act (the policy table's 1: with 0 no zone would
            // react). A policy row: every call records `Assumption::Policy(128)`
            // (`native_call`); a 0 caused by a knock-out records `Assumption::KnockOut` too.
            128 => match self.entity_of(arg(args, 0)) {
                Some(i) => {
                    let s = ActorStatus::of(&self.entities[i]);
                    if !s.can_act
                        && s.knocked_out
                        && let Some(vm) = self.vm.as_mut()
                    {
                        vm.assume(Assumption::KnockOut);
                    }
                    i32::from(s.can_act)
                }
                None => 1,
            },
            // 240 (actor) -> bool: present on the map (medium-low): the entity's `active` flag;
            // other elements are present unless deactivated (113): that branch is the policy
            // table's value and records `Assumption::Policy(240)`.
            240 => {
                if let Some(i) = self.entity_of(arg(args, 0)) {
                    i32::from(ActorStatus::of(&self.entities[i]).present)
                } else {
                    let handle = arg(args, 0);
                    let Some(vm) = self.vm.as_mut() else {
                        return 1;
                    };
                    vm.assume(Assumption::Policy(240));
                    i32::from(!vm.inactive_elements.contains(&handle))
                }
            }
            // 140 (actor, 0 / 1 / 2): the gait of the actor's patrol walks (low; the reading
            // 0 walk / 1 run / 2 sprint is the hypothesis of `stealth-and-combat.md` 2.5; the
            // engine plays a sprint as a run). Applied to the walks the waypoint program issues
            // from now on; a walk under way keeps its gait.
            140 => {
                if let Some(i) = self.entity_of(arg(args, 0)) {
                    self.vm_touch_entity(i);
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
            // requires 211's value (0 would be element 0). Which character is "main" is observed
            // only with a single one: with several, the choice records `Assumption::Policy`.
            111 | 211 | 250 => {
                let players = self
                    .entities
                    .iter()
                    .filter(|e| e.kind == EntityKind::Player)
                    .count();
                if players > 1
                    && let Some(vm) = self.vm.as_mut()
                {
                    vm.assume(Assumption::Policy(id));
                }
                self.player_element(0)
            }
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
                    self.vm_touch_entity(i);
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
                    self.vm_touch_entity(i);
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
            // 235 (element) -> bool: element taken (low; the corpus compares every result with
            // 1 from `Hourglass`, `CheckVictoryCondition` and one `IsTaken`, always on a pick-up
            // item): 1 once a player character picked the item up (`World::resolve_pickups`,
            // `VmState::taken_items`), 0 for an item still lying there and for every other
            // element. The reading is a policy (`Assumption::Policy(235)`, recorded by
            // `native_call`).
            235 => i32::from(
                self.vm
                    .as_ref()
                    .is_some_and(|vm| vm.taken_items.contains(&arg(args, 0))),
            ),
            // Stub natives: recorded per id (see `STUB_NATIVES`), result 0 or the policy value
            // of `STUB_POLICY_VALUES`. The call's taint was recorded by `native_call`
            // (`NATIVE_TAINT`); a result read taints in `vm.rs`.
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

    /// Map position of an element (actors, objects, scrolls, items, polygons).
    fn element_position(&self, handle: i32) -> Option<(i32, i32)> {
        let vm = self.vm.as_ref()?;
        match vm.element(handle)? {
            Element::Actor(i) => {
                let e = self.entities.get(i as usize)?;
                Some((e.x.round(), e.y.round()))
            }
            Element::Object { x, y } | Element::Scroll { x, y } | Element::Item { x, y, .. } => {
                Some((x, y))
            }
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
                self.vm_touch_entity(i);
                let e = &mut self.entities[i];
                e.active = active;
                if !active {
                    e.target = None;
                    e.path.clear();
                    e.pickup = None;
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
        self.vm_touch_entity(i);
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
        self.vm_touch_entity(entity as usize);
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

/// Message of natives 43 / 44 / 109 / 110: `(target, msg[, arg[, arg2]])`. The two-argument
/// forms (43 / 109) carry no arguments for the handler, which reads them as 0: the only place an
/// absent argument has a meaning.
fn message_of(args: &[i32]) -> Message {
    Message {
        target: arg(args, 0),
        id: arg(args, 1),
        arg: arg(args, 2),
        arg2: arg(args, 3),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The signature table names exactly the implemented and stub natives, once each, in
    /// ascending order (a binary search relies on it), and the natives the engine implements
    /// agree with it on what they consume.
    #[test]
    fn signature_table_covers_the_known_natives() {
        let mut ids: Vec<u32> = NATIVE_SIGNATURES.iter().map(|&(id, _, _, _)| id).collect();
        assert!(ids.windows(2).all(|w| w[0] < w[1]), "ascending, unique");
        let mut known: Vec<u32> = IMPLEMENTED_NATIVES
            .iter()
            .chain(STUB_NATIVES)
            .copied()
            .collect();
        known.sort_unstable();
        ids.sort_unstable();
        assert_eq!(ids, known);
        for id in IMPLEMENTED_NATIVES {
            assert!(
                !STUB_NATIVES.contains(id),
                "{id} is both implemented and a stub"
            );
        }
        assert_eq!(
            native_signature(237),
            Some(NativeSignature {
                arity: 1,
                returns_value: false,
                read_in_corpus: false,
            })
        );
        assert_eq!(
            native_signature(236),
            Some(NativeSignature {
                arity: 0,
                returns_value: true,
                read_in_corpus: true,
            })
        );
        assert_eq!(native_signature(999), None);
        // The corpus never reads a value a native does not leave.
        for &(id, _, returns_value, read_in_corpus) in NATIVE_SIGNATURES {
            assert!(returns_value || !read_in_corpus, "{id}");
        }
        for (id, _) in STUB_POLICY_VALUES {
            assert!(
                native_signature(*id).is_some_and(|s| s.returns_value),
                "{id}"
            );
        }
    }

    /// The taint table names exactly the known natives, in order: implemented ones are
    /// observed, policy or branch rows, stubs are effect or presentation rows; the ids the
    /// review named are policy rows and the presentation list is the documented one.
    #[test]
    fn taint_table_covers_the_known_natives() {
        let ids: Vec<u32> = NATIVE_TAINT.iter().map(|&(id, _)| id).collect();
        let sig_ids: Vec<u32> = NATIVE_SIGNATURES.iter().map(|&(id, _, _, _)| id).collect();
        assert_eq!(ids, sig_ids, "one taint row per signature row, same order");
        for &(id, taint) in NATIVE_TAINT {
            let status = native_status(id);
            let consistent = match taint {
                Taint::Observed | Taint::Policy | Taint::Branch => {
                    status == NativeStatus::Implemented
                }
                Taint::Effect | Taint::Presentation => status == NativeStatus::Stub,
            };
            assert!(consistent, "{id}: {taint:?} versus {status:?}");
        }
        for id in [98, 128, 140, 245] {
            assert_eq!(native_taint(id), Some(Taint::Policy), "{id}");
        }
        let presentation: Vec<u32> = NATIVE_TAINT
            .iter()
            .filter(|(_, t)| *t == Taint::Presentation)
            .map(|&(id, _)| id)
            .collect();
        assert_eq!(presentation, vec![62, 69, 149, 150, 243]);
        assert_eq!(native_taint(999), None);
    }
}
