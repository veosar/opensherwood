//! The mission script VM (ADR-0008): a typed instruction set, the program representation, the
//! run-time state and a deterministic interpreter, all owned by [`World`].
//!
//! Semantics come from `docs/formats/scb.md` ("Opcode hypotheses", "Calling convention",
//! "Native call table"); `opensherwood-script` translates the retail bytecode into this IR, the
//! core only executes it. Every value is an `i32`: integers, element / location / path handles,
//! and floats as [`Fixed`] (24.8) bits. Natives are dispatched by number in `natives.rs`.
//!
//! Time. The world ticks at the app's rate (60 Hz); the scripts count in their own unit (native 56
//! waits `n` script ticks, 25 per second is the hypothesis of the spec). A program carries the
//! conversion as a rational `wait_scale` chosen by the translator. `Hourglass` receives the world
//! tick as its time parameter (hypothesis: the scripts only compare differences of it).
//!
//! Work. Everything the VM does in one tick is charged to one deterministic budget
//! ([`WORK_BUDGET_PER_TICK`]): instruction dispatch, native and call argument transfers, zone and
//! scroll scans (one unit per entity looked at, one per polygon edge tested), the polygon natives
//! 97 / 204, sequence elements, and the path searches of the walks it issues (`nav.rs` charges
//! the search initialisation, node expansions, unwinding, line-clear cells and the smoothed
//! output; `world.rs` the conversion of the final path). The budget is granted exactly once per
//! world tick, at the start of [`World::vm_tick`]; the load-time run of `attach_script` has its
//! own, [`WORK_BUDGET_AT_LOAD`]. Every other entry point (the event hooks such as `IsTaken`, and
//! `vm_dismiss_text`, which the app calls between ticks) draws from whatever the current tick
//! left: a dismissal after an exhausted tick removes the page but the sequence behind it only
//! continues next tick. When the budget is exhausted the tick stops where it is (the running
//! callback is aborted, the remaining phases are skipped until the next tick, messages not yet
//! delivered stay queued) and `counters.budget_aborts` counts it; nothing panics and nothing
//! loops on.
//!
//! Sequences. Elements that take time issue *tokens* ([`SeqToken`]): a walk (natives 45 / 48 /
//! 64) completes when the entity arrived, gave up or was ordered elsewhere; an animation
//! (natives 49..=53, not modelled) completes at once. Native 32 is a [`SeqElement::Barrier`] that
//! holds the sequence until every token issued since the previous barrier completed. Text pages
//! (native 203) and waits (native 56) hold the sequence directly. Camera moves (33 / 34) are
//! instant. Native 202 texts are never blocking.
//!
//! Action changes. Every change of an actor's reported action id is queued
//! (`VmState::pending_action_changes`, snapshotted and hashed) and delivered to the class bound
//! to the actor as `ActionChange(previous, new)` exactly once: a change whose class has no
//! handler is dropped as undeliverable, one whose handler returned (or trapped) is removed, and
//! one the budget cut short stays at the front of the queue for the next tick. A queued handler
//! runs as a *transaction* ([`Transaction`]): the script-visible state it may mutate (the VM's
//! variables, queues, money, RNG; the entities a native touches; the selection and the camera)
//! is captured before it runs, charged to the budget one unit per value copied, and put back
//! when the budget cuts the handler short, so the retry on the next tick starts the handler
//! from the state it saw the first time and no effect is applied twice; one that fails
//! deterministically (a trap, a fault) is rolled back too and consumed, since it would fail
//! the same way again. A full queue is a deterministic fault ([`Fault::ActionQueueOverflow`]),
//! never a silent drop, and a call that would exceed [`MAX_FRAMES`] is the sticky
//! [`Fault::CallStackOverflow`] that aborts the callback where it stands (Codex review 10,
//! finding 3: the call's destination is never left untouched behind a fabricated value).
//!
//! Hypotheses and taint (ADR-0008, "Hypotheses and taint"). The engine runs the retail scripts
//! over stubs and over hypotheses about the original. The taint is *dependency-closed by
//! construction*: every source of a hypothesis is named by an [`Assumption`] variant (the
//! registry) and recorded at the point where the engine takes the hypothesis, whether or not
//! the script reads a value there: an opcode of low-confidence meaning executed
//! ([`Assumption::Opcode`], [`Assumption::UnresolvedJump`]), a native whose reading is a policy
//! ([`Assumption::Policy`], `natives::NATIVE_TAINT`), a recorded stub invoked or its fabricated
//! result consumed ([`Assumption::StubResult`]; only the presentation-only stubs of
//! `natives::NATIVE_TAINT` record nothing on the call), a lenient unknown native
//! ([`Assumption::UnknownNative`]), and the engine's own hypotheses, each recorded by the rule
//! itself at the point where it first mutates authoritative state, independent of any callback
//! or later consumer (Codex review 9, finding 1): the unmeasured part of the sight (the rear
//! radius and the crouch divisor, [`Assumption::SightCone`]; the cone itself is measured), the
//! unmeasured part of the noise radius ([`Assumption::NoiseRadius`]), the alert sequence a
//! sighting starts ([`Assumption::AlertPolicy`]), the alert timeout and the return to the post
//! ([`Assumption::AlertTimeout`]), the attack policy ([`Assumption::AttackPolicy`]), the knock-out
//! policy ([`Assumption::KnockOut`]), the profile stats, the tick rate, a scroll's fate after
//! its reading ([`Assumption::ScrollPickup`]; the order, the approach and the pause are
//! measured), a purse's amount or an unknown item kind's effect ([`Assumption::ItemPickup`];
//! the order, the stoop and the arrows' stack are measured), the
//! zone presence at load, a walk that completed without arriving, the `ActionChange` parameter
//! order, the campaign graph, the lenient asset fallbacks. The set is snapshotted, hashed,
//! validated and exposed as `ScriptObservation::assumptions` / `tainted`; it only grows (a
//! rolled-back transaction keeps what it recorded). `mission_won` / `mission_lost` are still
//! recorded, but an outcome reached with a non-empty set is not authoritative; one reached with
//! an empty set depended on no hypothesis the engine knows of.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::fixed::Fixed;
use crate::geom::point_in_polygon;
use crate::hash::Encoder;
use crate::rng::Rng;
use crate::world::{EntityKind, World};

/// Work units the VM may spend in one tick (all callbacks, zone and scroll scans, event hooks,
/// sequences, dismissals and path searches together), granted once at the start of `vm_tick`
/// and never replenished before the next tick. A unit is one instruction, one transferred
/// argument, one entity looked at by a scan, one polygon edge test, one sequence element, one A*
/// node expansion, one unwound or converted path cell, one line-clear cell or 64 search cells
/// initialised.
pub const WORK_BUDGET_PER_TICK: u64 = 1 << 22;
/// Work units of the load-time run (`Initialize` on every class, `PostInitialize`, the first
/// sequence elements), granted once by `attach_script`; what it leaves serves the dismissals of
/// the briefing pages until the first tick grants [`WORK_BUDGET_PER_TICK`].
pub const WORK_BUDGET_AT_LOAD: u64 = 1 << 22;
/// Deepest argument / parameter stack; no `argc` may exceed it.
pub const MAX_STACK: usize = 1 << 12;
/// Largest total number of instructions over all classes of a program.
pub const MAX_PROGRAM_CODE: usize = 1 << 22;
/// Largest number of vertices of one location polygon.
pub const MAX_POLYGON_VERTICES: usize = 1 << 12;
/// Largest total vertex count over all locations of a program.
pub const MAX_LOCATION_VERTICES: usize = 1 << 20;
/// Largest magnitude of a location or element coordinate (the geometry's range).
pub const MAX_LOCATION_COORD: i32 = crate::geom::MAX_COORD;
/// Largest total number of elements over all active sequences.
pub const MAX_SEQUENCE_ELEMENTS: usize = 1 << 16;
/// Deepest call stack accepted.
pub const MAX_FRAMES: usize = 64;
/// Number of mission variables (native 0 / 1 / 2 index them).
pub const MISSION_VARIABLES: usize = 64;
/// Largest number of pending texts, messages, sequence elements or objectives kept.
pub const MAX_QUEUE: usize = 1 << 12;
/// Largest number of classes, elements and locations accepted in a program.
pub const MAX_TABLE: usize = 1 << 14;
/// Largest number of instructions in one class.
pub const MAX_CODE: usize = 1 << 20;
/// Handle value meaning "none" (element, location, path): `n6(-1)`, `n3(-1)` in the scripts.
pub const NONE_HANDLE: i32 = -1;

/// Bit set in a location value that packs an actor position (see [`location_of_point`]).
pub const LOCATION_POINT_BIT: i32 = 1 << 30;
/// RNG stream id of the `script` stream (the gameplay stream is 1).
pub const SCRIPT_RNG_STREAM: u64 = 2;

/// Storage class of an operand slot (`docs/formats/scb.md`, "Instruction").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Space {
    /// Class variable block.
    Class,
    /// Function-local ("volatile") block.
    Local,
    /// Temporary block.
    Temp,
}

impl Space {
    fn tag(self) -> u8 {
        match self {
            Space::Class => 1,
            Space::Local => 2,
            Space::Temp => 3,
        }
    }
}

/// An operand: a 4-byte slot in one of the three storage spaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slot {
    /// Storage class.
    pub space: Space,
    /// Slot index (byte offset / 4).
    pub index: u32,
}

/// Binary operators of the three-operand instructions (`docs/formats/scb.md`, opcode table; the
/// low-confidence rows are pinned by the translator's tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinOp {
    /// `0x19`: integer add (wrapping).
    Add,
    /// `0x1a`: integer subtract.
    Sub,
    /// `0x1b`: integer multiply.
    Mul,
    /// `0x1d`: bitwise or.
    Or,
    /// `0x1e`: bitwise and.
    And,
    /// `0x22`: fixed-point multiply.
    FixedMul,
    /// `0x25`: `a < b` (integers).
    Lt,
    /// `0x26`: `a >= b` (medium).
    Ge,
    /// `0x24`: `a >= b` by its Desperados name (**low**: the direction is not determinable
    /// from the data); executing one records [`Assumption::Opcode`]`(0x24)`.
    GeLow,
    /// `0x27`: `a > b`.
    Gt,
    /// `0x28`: `a != b` (**low**: `>=` is the alternative); executing one records
    /// [`Assumption::Opcode`]`(0x28)`.
    Ne,
    /// `0x29`: `a == b` (integers or handles).
    Eq,
    /// `0x2b`: `a < b` on fixed-point values (**low**, one use); executing one records
    /// [`Assumption::Opcode`]`(0x2b)`.
    FixedLt,
}

impl BinOp {
    fn tag(self) -> u8 {
        match self {
            BinOp::Add => 1,
            BinOp::Sub => 2,
            BinOp::Mul => 3,
            BinOp::Or => 4,
            BinOp::And => 5,
            BinOp::FixedMul => 6,
            BinOp::Lt => 7,
            BinOp::Ge => 8,
            BinOp::Gt => 9,
            BinOp::Ne => 10,
            BinOp::Eq => 11,
            BinOp::FixedLt => 12,
            BinOp::GeLow => 13,
        }
    }

    /// The bytecode opcode this operator reads with **low** confidence, if any: the hypothesis
    /// source [`Assumption::Opcode`] names (`docs/formats/scb.md`, "Opcode hypotheses").
    #[must_use]
    pub fn low_confidence_opcode(self) -> Option<u8> {
        match self {
            BinOp::GeLow => Some(0x24),
            BinOp::Ne => Some(0x28),
            BinOp::FixedLt => Some(0x2b),
            _ => None,
        }
    }

    fn apply(self, a: i32, b: i32) -> i32 {
        match self {
            BinOp::Add => a.wrapping_add(b),
            BinOp::Sub => a.wrapping_sub(b),
            BinOp::Mul => a.wrapping_mul(b),
            BinOp::Or => a | b,
            BinOp::And => a & b,
            BinOp::FixedMul => (Fixed::from_raw(a) * Fixed::from_raw(b)).raw(),
            BinOp::Lt => i32::from(a < b),
            BinOp::Ge | BinOp::GeLow => i32::from(a >= b),
            BinOp::Gt => i32::from(a > b),
            BinOp::Ne => i32::from(a != b),
            BinOp::Eq => i32::from(a == b),
            BinOp::FixedLt => i32::from(Fixed::from_raw(a) < Fixed::from_raw(b)),
        }
    }
}

/// The bytecode opcodes whose reading is of low confidence and whose execution records
/// [`Assumption::Opcode`]: `0x24` / `0x28` / `0x2b` ([`BinOp::low_confidence_opcode`]) and
/// `0x14` (a float immediate rounded to 24.8 fixed point: the original computes in `f32`).
/// A jump to `0xffff` (`0x0e`, two occurrences) is [`Assumption::UnresolvedJump`].
pub const LOW_CONFIDENCE_OPCODES: [u8; 4] = [0x14, 0x24, 0x28, 0x2b];

/// One instruction. The translator keeps one instruction per bytecode quad so that jump targets
/// are the original quad indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Instr {
    /// Does nothing (`0x01`, and the filled-in slots of unknown quads).
    Nop,
    /// Function prologue (`0x03`); the sizes are those of the function table, kept for checking.
    Enter {
        /// Local slots.
        locals: u32,
        /// Temporary slots.
        temps: u32,
    },
    /// End of function (`0x04`) or return (`0x06`).
    Return,
    /// A jump to the unresolved label `0xffff` (`0x0e`, two occurrences in the corpus): leaves
    /// the function like [`Instr::Return`] (**low** confidence) and records
    /// [`Assumption::UnresolvedJump`].
    LeaveUnresolved,
    /// Set the return value of the current function (`0x07`).
    SetResult {
        /// Value.
        src: Slot,
    },
    /// Read parameter `index` of the current function (`0x08`).
    LoadParam {
        /// Destination.
        dst: Slot,
        /// Parameter number.
        index: u32,
    },
    /// Push an argument for the next [`Instr::Call`] (`0x02`).
    PushParam {
        /// Value.
        src: Slot,
    },
    /// Call a function of the same class (`0x05`); `argc` parameters were pushed. When the
    /// bytecode reads the return value (`0x0a` directly after the call) the translator fuses the
    /// two quads into this one instruction and leaves a [`Instr::Nop`] in the reader's place, so
    /// no control flow can reach a result read without the call that produces it (Codex review
    /// 9, finding 3); `Program::validate` refuses a `dst` on a callee that leaves no value
    /// (`Function::has_result`). The callee's result is written to `dst` when it returns.
    Call {
        /// Function index in the class table.
        function: u32,
        /// Parameters to pop.
        argc: u32,
        /// Where the return value goes, if the script reads it.
        dst: Option<Slot>,
    },
    /// Push an argument for the next [`Instr::Native`] (`0x0b`).
    PushArg {
        /// Value.
        src: Slot,
    },
    /// Call engine function `id` with `argc` pushed arguments (`0x0c`) and store its result in
    /// `dst` when the bytecode reads one (`0x0d` directly after the call: the translator fuses
    /// the two quads into this one instruction and leaves a [`Instr::Nop`] in the reader's
    /// place, so there is no instruction that consumes a result its own call did not produce).
    /// `Program::validate` refuses a `dst` on a native whose signature leaves no value.
    Native {
        /// Native id.
        id: u32,
        /// Arguments to pop.
        argc: u32,
        /// Where the result goes, if the script reads it.
        dst: Option<Slot>,
    },
    /// Unconditional jump (`0x0e`).
    Jump {
        /// Target instruction.
        target: u32,
    },
    /// Jump if the value is non-zero (`0x0f`).
    JumpIf {
        /// Condition.
        cond: Slot,
        /// Target instruction.
        target: u32,
    },
    /// Copy a slot (`0x11`, `0x12`).
    Move {
        /// Destination.
        dst: Slot,
        /// Source.
        src: Slot,
    },
    /// Load an integer immediate (`0x13`).
    LoadInt {
        /// Destination.
        dst: Slot,
        /// Value.
        value: i32,
    },
    /// Load a fixed-point immediate (`0x14`, converted from the file's `f32`).
    LoadFixed {
        /// Destination.
        dst: Slot,
        /// Value.
        value: Fixed,
    },
    /// Integer negation (`0x15`).
    Neg {
        /// Destination.
        dst: Slot,
        /// Source.
        src: Slot,
    },
    /// Integer to fixed point (`0x18`).
    IntToFixed {
        /// Destination.
        dst: Slot,
        /// Source.
        src: Slot,
    },
    /// Three-operand arithmetic or comparison (`0x19..=0x2b`).
    Binary {
        /// Operator.
        op: BinOp,
        /// Destination.
        dst: Slot,
        /// Left operand.
        a: Slot,
        /// Right operand.
        b: Slot,
    },
}

impl Instr {
    /// Stable tag for canonical encodings.
    #[must_use]
    pub fn tag(&self) -> u8 {
        match self {
            Instr::Nop => 1,
            Instr::Enter { .. } => 2,
            Instr::Return => 3,
            Instr::SetResult { .. } => 4,
            Instr::LoadParam { .. } => 5,
            Instr::PushParam { .. } => 6,
            Instr::Call { .. } => 7,
            Instr::PushArg { .. } => 9,
            Instr::Native { .. } => 10,
            Instr::LeaveUnresolved => 11,
            Instr::Jump { .. } => 12,
            Instr::JumpIf { .. } => 13,
            Instr::Move { .. } => 14,
            Instr::LoadInt { .. } => 15,
            Instr::LoadFixed { .. } => 16,
            Instr::Neg { .. } => 17,
            Instr::IntToFixed { .. } => 18,
            Instr::Binary { .. } => 19,
        }
    }
}

/// A function of a class (the file's function table entry, decoded per the calling convention).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Function {
    /// Name (`Initialize`, `Hourglass`, a designer helper, ...).
    pub name: String,
    /// Index of the first instruction in the class code.
    pub address: u32,
    /// Number of parameters (`(unknown_2 - unknown_1) / 4`).
    pub param_count: u32,
    /// Whether the function returns a value (`unknown_1 == 4`).
    pub has_result: bool,
    /// Local slots.
    pub locals: u32,
    /// Temporary slots.
    pub temps: u32,
}

/// A script class: variables, functions, code and its binding to the mission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Class {
    /// Name (`StartUp` for the level class, a mission element name otherwise).
    pub name: String,
    /// Number of class variable slots.
    pub variable_count: u32,
    /// Functions in table order.
    pub functions: Vec<Function>,
    /// Instructions.
    pub code: Vec<Instr>,
    /// Element of the mission this class is bound to (index into [`Program::elements`]).
    pub element: Option<u32>,
    /// Script polygon (location index) whose `EnterZone` / `ExitZone` this class handles.
    pub zone: Option<u32>,
    /// Named rail point `(rail, point)` whose `ReachPoint` this class handles.
    pub rail_point: Option<(u32, u32)>,
}

impl Class {
    /// Index of the function named `name`.
    #[must_use]
    pub fn function(&self, name: &str) -> Option<u32> {
        self.functions
            .iter()
            .position(|f| f.name == name)
            .map(|i| i as u32)
    }
}

/// An entry of the level's flat element table (native 3; `docs/formats/scb.md`, "Index spaces").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Element {
    /// An animated map element or proto-level entry (index within the map's part of the table).
    Map(u32),
    /// An entry the engine does not model yet (state and attributes still work on it).
    Unmodelled(u32),
    /// A mission actor: index into [`World::entities`].
    Actor(u32),
    /// A mission object (`BOOM`) at a map position.
    Object {
        /// Map x.
        x: i32,
        /// Map y.
        y: i32,
    },
    /// A scroll (`SKRO`) at a map position.
    Scroll {
        /// Map x.
        x: i32,
        /// Map y.
        y: i32,
    },
    /// A pick-up item (`ZORG`, the executable's "Bonus" chunk; `docs/formats/rhm.md`) at a map
    /// position (the sprite's base point): a purse, a bundle of arrows or another kind, with
    /// its stack size. Taken by a player character ordered onto it by a click on the item
    /// (`World::resolve_pickups`: the walk, the stoop, the take); native 235 reads whether it
    /// was taken, 113 / 114 hide and show it like any other non-actor element.
    Item {
        /// Map x.
        x: i32,
        /// Map y.
        y: i32,
        /// What the item is (the record's `unknown_a`).
        kind: ItemKind,
        /// Stack size (the record's `unknown_b`, 1..=5): observed as the digit of the hand
        /// pointer's badge over the item and, for arrows, as what the counter receives
        /// (`docs/original/h01-measurements-2.md` 1.1 / 1.3).
        stack: u16,
    },
    /// A script polygon: location index.
    Polygon(u32),
}

/// The kind of a pick-up item, read from the `ZORG` record's `unknown_a` (`docs/formats/rhm.md`,
/// "`ZORG`": the value pairs the first mission's items with the tutorial scrolls that hand them
/// out; medium confidence for the two named kinds, everything else stays unknown and is kept by
/// its raw value).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// Arrows (`unknown_a` 0): taking the pile adds its stack to the character's arrows.
    Arrows,
    /// A purse with money (`unknown_a` 9): taking it adds [`PURSE_MONEY_PER_STACK`] times the
    /// stack to the mission's money and one purse to the character's purses.
    Purse,
    /// A kind the engine does not read yet (`unknown_a` value): taking it only removes it.
    #[serde(rename = "unknown_a")]
    Unknown(u16),
}

/// Money a purse item holds per stack unit (`Element::Item` of kind [`ItemKind::Purse`]): a
/// policy, not a measurement (the corpus never states a purse's worth; its only money increment
/// is the +25 of one ambush handler; no purse with money was taken in the oracle sessions,
/// `docs/original/h01-measurements-2.md` 1.3). Taking a purse records
/// [`Assumption::ItemPickup`].
pub const PURSE_MONEY_PER_STACK: i32 = 25;

impl ItemKind {
    /// The kind of a `ZORG` record with this `unknown_a`.
    #[must_use]
    pub fn from_field(unknown_a: u16) -> Self {
        match unknown_a {
            0 => ItemKind::Arrows,
            9 => ItemKind::Purse,
            other => ItemKind::Unknown(other),
        }
    }

    fn encode(self, e: &mut Encoder) {
        match self {
            ItemKind::Arrows => e.u8(1),
            ItemKind::Purse => e.u8(2),
            ItemKind::Unknown(a) => e.u8(3).u32(u32::from(a)),
        };
    }
}

impl Element {
    fn encode(self, e: &mut Encoder) {
        match self {
            Element::Map(i) => e.u8(1).u32(i),
            Element::Unmodelled(i) => e.u8(2).u32(i),
            Element::Actor(i) => e.u8(3).u32(i),
            Element::Object { x, y } => e.u8(4).i32(x).i32(y),
            Element::Scroll { x, y } => e.u8(5).i32(x).i32(y),
            Element::Polygon(i) => e.u8(6).u32(i),
            Element::Item { x, y, kind, stack } => {
                e.u8(7).i32(x).i32(y).u32(u32::from(stack));
                kind.encode(e);
                e
            }
        };
    }
}

/// A location of the mission (native 6: `GULP` points then polygons).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Location {
    /// A point in map pixels.
    Point {
        /// Map x.
        x: i32,
        /// Map y.
        y: i32,
    },
    /// A polygon in map pixels.
    Polygon(Vec<(i32, i32)>),
}

impl Location {
    /// A representative position: the point itself or the vertex average of a polygon.
    #[must_use]
    pub fn position(&self) -> (i32, i32) {
        match self {
            Location::Point { x, y } => (*x, *y),
            Location::Polygon(pts) => {
                if pts.is_empty() {
                    return (0, 0);
                }
                let n = pts.len() as i64;
                let sx: i64 = pts.iter().map(|p| i64::from(p.0)).sum();
                let sy: i64 = pts.iter().map(|p| i64::from(p.1)).sum();
                ((sx / n) as i32, (sy / n) as i32)
            }
        }
    }
}

/// Pack a map position into a location value (bit 30 set, 15 bits per coordinate). Positions
/// outside `0..32768` are clamped.
#[must_use]
pub fn location_of_point(x: i32, y: i32) -> i32 {
    let (x, y) = (x.clamp(0, 0x7fff), y.clamp(0, 0x7fff));
    LOCATION_POINT_BIT | (x << 15) | y
}

/// A translated script: what the translator hands to the core at mission load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    /// Classes in file order; class 0 is the level class.
    pub classes: Vec<Class>,
    /// The flat element table.
    pub elements: Vec<Element>,
    /// Locations: `GULP` points then polygons.
    pub locations: Vec<Location>,
    /// World ticks per script tick as a rational `(num, den)`: native 56's argument is multiplied
    /// by `num / den`.
    pub wait_scale: (u32, u32),
}

impl Program {
    /// Check every internal reference and bound a translated script or a snapshot must satisfy:
    /// table sizes (per class and aggregate), functions laid out in table order from address 0
    /// each starting with the `Enter` of its frame sizes, jumps inside their function, parameter
    /// reads and call arities against the function table, slot indices inside their blocks,
    /// native / call arities within [`MAX_STACK`], every native call of a known id with the
    /// argument count of its signature and a result slot only on a native whose signature
    /// leaves a value (`natives::NATIVE_SIGNATURES`; an unknown id is unconstrained, it traps
    /// or is recorded at run time; the call and its result read are one instruction, so no
    /// control flow can reach a result read without its call), the same for script calls (a
    /// result slot only on a callee with `has_result`), balanced parameter and argument stacks
    /// in every function
    /// ([`check_stack_balance`]), bindings inside the tables, and element and location
    /// coordinates within `+-MAX_LOCATION_COORD`. The translator performs the same checks
    /// earlier for diagnostics; this is the trust boundary (a snapshot embeds the program).
    pub fn validate(&self) -> Result<(), String> {
        if self.classes.is_empty() {
            return Err("program has no classes".into());
        }
        if self.classes.len() > MAX_TABLE
            || self.elements.len() > MAX_TABLE
            || self.locations.len() > MAX_TABLE
        {
            return Err("program tables too large".into());
        }
        if self.wait_scale.0 == 0 || self.wait_scale.1 == 0 {
            return Err("wait scale must be a positive rational".into());
        }
        let mut total_code = 0usize;
        for (ci, c) in self.classes.iter().enumerate() {
            if c.code.len() > MAX_CODE || c.functions.len() > MAX_TABLE {
                return Err(format!("class {ci} too large"));
            }
            total_code = total_code.saturating_add(c.code.len());
            if total_code > MAX_PROGRAM_CODE {
                return Err("program code too large".into());
            }
            if c.variable_count as usize > MAX_TABLE {
                return Err(format!("class {ci} has too many variables"));
            }
            if let Some(e) = c.element
                && e as usize >= self.elements.len()
            {
                return Err(format!("class {ci} bound to element {e} out of range"));
            }
            if let Some(z) = c.zone
                && !matches!(self.locations.get(z as usize), Some(Location::Polygon(_)))
            {
                return Err(format!(
                    "class {ci} bound to zone {z} which is not a polygon"
                ));
            }
            if c.functions.is_empty() != c.code.is_empty() {
                return Err(format!("class {ci} functions and code disagree"));
            }
            if c.functions.first().is_some_and(|f| f.address != 0) {
                return Err(format!("class {ci} code does not start with a function"));
            }
            for (fi, f) in c.functions.iter().enumerate() {
                if f.address as usize >= c.code.len() {
                    return Err(format!("class {ci} function {fi} address out of range"));
                }
                if fi > 0 && f.address <= c.functions[fi - 1].address {
                    return Err(format!(
                        "class {ci} functions are not laid out in table order"
                    ));
                }
                if f.locals as usize > MAX_TABLE
                    || f.temps as usize > MAX_TABLE
                    || f.param_count as usize > MAX_STACK
                {
                    return Err(format!("class {ci} function {fi} frame too large"));
                }
                match c.code[f.address as usize] {
                    Instr::Enter { locals, temps } if locals == f.locals && temps == f.temps => {}
                    _ => {
                        return Err(format!(
                            "class {ci} function {fi} does not start with its prologue"
                        ));
                    }
                }
            }
            // Every instruction belongs to the function whose range holds it; functions are laid
            // out in table order.
            let mut fi = 0usize;
            for (pc, ins) in c.code.iter().enumerate() {
                while fi + 1 < c.functions.len() && c.functions[fi + 1].address as usize <= pc {
                    fi += 1;
                }
                let Some(f) = c.functions.get(fi) else {
                    return Err(format!(
                        "class {ci} instruction {pc} outside every function"
                    ));
                };
                let end = c
                    .functions
                    .get(fi + 1)
                    .map_or(c.code.len(), |n| n.address as usize);
                let slot_ok = |s: Slot| match s.space {
                    Space::Class => s.index < c.variable_count,
                    Space::Local => s.index < f.locals,
                    Space::Temp => s.index < f.temps,
                };
                let target_ok = |t: u32| (f.address as usize..end).contains(&(t as usize));
                let ok = match *ins {
                    Instr::Nop | Instr::Return | Instr::LeaveUnresolved => true,
                    Instr::Enter { locals, temps } => locals == f.locals && temps == f.temps,
                    Instr::SetResult { src }
                    | Instr::PushParam { src }
                    | Instr::PushArg { src } => slot_ok(src),
                    Instr::LoadParam { dst, index } => slot_ok(dst) && index < f.param_count,
                    Instr::LoadInt { dst, .. } | Instr::LoadFixed { dst, .. } => slot_ok(dst),
                    Instr::Call {
                        function,
                        argc,
                        dst,
                    } => {
                        let Some(callee) = c.functions.get(function as usize) else {
                            return Err(format!(
                                "class {ci} instruction {pc} calls function {function}, which does not exist"
                            ));
                        };
                        if dst.is_some() && !callee.has_result {
                            return Err(format!(
                                "class {ci} instruction {pc} reads the result of function {function} ({}), which has none",
                                callee.name
                            ));
                        }
                        argc as usize <= MAX_STACK
                            && callee.param_count == argc
                            && dst.is_none_or(slot_ok)
                    }
                    Instr::Native { id, argc, dst } => {
                        if let Some(sig) = crate::natives::native_signature(id) {
                            if sig.arity != argc {
                                return Err(format!(
                                    "class {ci} instruction {pc} calls native {id} with {argc} arguments; its signature takes {}",
                                    sig.arity
                                ));
                            }
                            if dst.is_some() && !sig.returns_value {
                                return Err(format!(
                                    "class {ci} instruction {pc} reads the result of native {id}, which has none"
                                ));
                            }
                        }
                        argc as usize <= MAX_STACK && dst.is_none_or(slot_ok)
                    }
                    Instr::Jump { target } => target_ok(target),
                    Instr::JumpIf { cond, target } => slot_ok(cond) && target_ok(target),
                    Instr::Move { dst, src }
                    | Instr::Neg { dst, src }
                    | Instr::IntToFixed { dst, src } => slot_ok(dst) && slot_ok(src),
                    Instr::Binary { dst, a, b, .. } => slot_ok(dst) && slot_ok(a) && slot_ok(b),
                };
                if !ok {
                    return Err(format!("class {ci} instruction {pc} out of range"));
                }
            }
            for (fi, f) in c.functions.iter().enumerate() {
                let end = c
                    .functions
                    .get(fi + 1)
                    .map_or(c.code.len(), |n| n.address as usize);
                if let Err(pc) = check_stack_balance(&c.code, f.address as usize, end) {
                    return Err(format!(
                        "class {ci} function {fi} stacks are not balanced at instruction {pc}"
                    ));
                }
            }
        }
        let coord_ok = |v: i32| v.unsigned_abs() <= MAX_LOCATION_COORD as u32;
        for (i, el) in self.elements.iter().enumerate() {
            match *el {
                Element::Object { x, y }
                | Element::Scroll { x, y }
                | Element::Item { x, y, .. } => {
                    if !(coord_ok(x) && coord_ok(y)) {
                        return Err(format!("element {i} position out of range"));
                    }
                }
                Element::Polygon(l) => {
                    if !matches!(self.locations.get(l as usize), Some(Location::Polygon(_))) {
                        return Err(format!("element {i} polygon out of range"));
                    }
                }
                Element::Map(_) | Element::Unmodelled(_) | Element::Actor(_) => {}
            }
        }
        let mut vertices = 0usize;
        for (i, l) in self.locations.iter().enumerate() {
            match l {
                Location::Point { x, y } => {
                    if !(coord_ok(*x) && coord_ok(*y)) {
                        return Err(format!("location {i} out of range"));
                    }
                }
                Location::Polygon(pts) => {
                    if pts.len() > MAX_POLYGON_VERTICES {
                        return Err(format!("location {i} has too many vertices"));
                    }
                    vertices = vertices.saturating_add(pts.len());
                    if vertices > MAX_LOCATION_VERTICES {
                        return Err("program locations have too many vertices".into());
                    }
                    if pts.iter().any(|&(x, y)| !(coord_ok(x) && coord_ok(y))) {
                        return Err(format!("location {i} out of range"));
                    }
                }
            }
        }
        Ok(())
    }

    /// Canonical digest of the whole program (part of the `scripts` hash).
    #[must_use]
    pub fn digest(&self) -> String {
        let mut e = Encoder::new("program");
        e.u32(self.wait_scale.0).u32(self.wait_scale.1);
        e.u32(self.classes.len() as u32);
        for c in &self.classes {
            e.str(&c.name).u32(c.variable_count);
            match c.element {
                Some(x) => e.u8(1).u32(x),
                None => e.u8(0),
            };
            match c.zone {
                Some(x) => e.u8(1).u32(x),
                None => e.u8(0),
            };
            match c.rail_point {
                Some((r, p)) => e.u8(1).u32(r).u32(p),
                None => e.u8(0),
            };
            e.u32(c.functions.len() as u32);
            for f in &c.functions {
                e.str(&f.name)
                    .u32(f.address)
                    .u32(f.param_count)
                    .u8(u8::from(f.has_result))
                    .u32(f.locals)
                    .u32(f.temps);
            }
            e.u32(c.code.len() as u32);
            for ins in &c.code {
                encode_instr(&mut e, ins);
            }
        }
        e.u32(self.elements.len() as u32);
        for el in &self.elements {
            el.encode(&mut e);
        }
        e.u32(self.locations.len() as u32);
        for l in &self.locations {
            match l {
                Location::Point { x, y } => {
                    e.u8(1).i32(*x).i32(*y);
                }
                Location::Polygon(pts) => {
                    e.u8(2).u32(pts.len() as u32);
                    for (x, y) in pts {
                        e.i32(*x).i32(*y);
                    }
                }
            }
        }
        e.finish()
    }

    /// Element handle of the first entity of `kind` in entity order; `NONE_HANDLE` when absent.
    #[must_use]
    pub fn element_of_entity(&self, entity: u32) -> i32 {
        self.elements
            .iter()
            .position(|e| *e == Element::Actor(entity))
            .map_or(NONE_HANDLE, |i| i as i32)
    }
}

/// Stack balance of one function (`code[start..end]`, jump targets already checked to lie in
/// it): a worklist walk assigns every reachable instruction the depths of the parameter and
/// argument stacks. A `Call` / `Native` needs at least its `argc` values, a `Return` (and falling
/// off the end of the function) needs both stacks empty, a join point must agree, and neither
/// depth may exceed [`MAX_STACK`]. Unreachable code is not walked. `Err(pc)` names the offending
/// instruction. The interpreter's teardown clears the stacks after every callback anyway; this
/// makes a program that would rely on it invalid instead of merely tolerated.
fn check_stack_balance(code: &[Instr], start: usize, end: usize) -> Result<(), usize> {
    let mut depth: Vec<Option<(u32, u32)>> = vec![None; end.saturating_sub(start)];
    let mut work = vec![(start, (0u32, 0u32))];
    while let Some((pc, d)) = work.pop() {
        let Some(slot) = pc.checked_sub(start).and_then(|i| depth.get_mut(i)) else {
            return Err(pc);
        };
        match *slot {
            Some(seen) if seen == d => continue,
            Some(_) => return Err(pc),
            None => *slot = Some(d),
        }
        let (mut params, mut args) = d;
        match code[pc] {
            Instr::PushParam { .. } => params += 1,
            Instr::Call { argc, .. } => {
                if params < argc {
                    return Err(pc);
                }
                params -= argc;
            }
            Instr::PushArg { .. } => args += 1,
            Instr::Native { argc, .. } => {
                if args < argc {
                    return Err(pc);
                }
                args -= argc;
            }
            Instr::Return | Instr::LeaveUnresolved => {
                if (params, args) != (0, 0) {
                    return Err(pc);
                }
                continue;
            }
            Instr::Jump { target } => {
                work.push((target as usize, (params, args)));
                continue;
            }
            Instr::JumpIf { target, .. } => work.push((target as usize, (params, args))),
            _ => {}
        }
        if params as usize > MAX_STACK || args as usize > MAX_STACK {
            return Err(pc);
        }
        if pc + 1 >= end {
            // Falling off the end of the function is a return.
            if (params, args) != (0, 0) {
                return Err(pc);
            }
            continue;
        }
        work.push((pc + 1, (params, args)));
    }
    Ok(())
}

fn encode_slot(e: &mut Encoder, s: Slot) {
    e.u8(s.space.tag()).u32(s.index);
}

fn encode_instr(e: &mut Encoder, ins: &Instr) {
    e.u8(ins.tag());
    match *ins {
        Instr::Nop | Instr::Return | Instr::LeaveUnresolved => {}
        Instr::Enter { locals, temps } => {
            e.u32(locals).u32(temps);
        }
        Instr::SetResult { src } | Instr::PushParam { src } | Instr::PushArg { src } => {
            encode_slot(e, src);
        }
        Instr::LoadParam { dst, index } => {
            encode_slot(e, dst);
            e.u32(index);
        }
        Instr::Call {
            function,
            argc,
            dst,
        } => {
            e.u32(function).u32(argc);
            match dst {
                Some(d) => {
                    e.u8(1);
                    encode_slot(e, d);
                }
                None => {
                    e.u8(0);
                }
            }
        }
        Instr::Native { id, argc, dst } => {
            e.u32(id).u32(argc);
            match dst {
                Some(d) => {
                    e.u8(1);
                    encode_slot(e, d);
                }
                None => {
                    e.u8(0);
                }
            }
        }
        Instr::Jump { target } => {
            e.u32(target);
        }
        Instr::JumpIf { cond, target } => {
            encode_slot(e, cond);
            e.u32(target);
        }
        Instr::Move { dst, src } | Instr::Neg { dst, src } | Instr::IntToFixed { dst, src } => {
            encode_slot(e, dst);
            encode_slot(e, src);
        }
        Instr::LoadInt { dst, value } => {
            encode_slot(e, dst);
            e.i32(value);
        }
        Instr::LoadFixed { dst, value } => {
            encode_slot(e, dst);
            e.i32(value.raw());
        }
        Instr::Binary { op, dst, a, b } => {
            e.u8(op.tag());
            encode_slot(e, dst);
            encode_slot(e, a);
            encode_slot(e, b);
        }
    }
}

/// A call frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    /// Class.
    pub class: u32,
    /// Function index.
    pub function: u32,
    /// Next instruction.
    pub pc: u32,
    /// Local slots.
    pub locals: Vec<i32>,
    /// Temporary slots.
    pub temps: Vec<i32>,
    /// Parameters.
    pub params: Vec<i32>,
    /// Return value set by `SetResult` (written to the caller's [`Instr::Call`] destination
    /// when the frame returns; a frame holds no result of a call it made).
    pub result: i32,
}

/// A queued `ProcessMessage` (natives 43 / 44 / 109 / 110), delivered on the next tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Target element handle.
    pub target: i32,
    /// Message id.
    pub id: i32,
    /// First argument.
    pub arg: i32,
    /// Second argument (native 110).
    pub arg2: i32,
}

/// An element of a sequence (natives 30 / 31 collect them; see `docs/formats/scb.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeqElement {
    /// Native 203: show a text page and wait until it is dismissed.
    Text(i32),
    /// Native 56: wait this many world ticks.
    Wait(u32),
    /// Natives 33 / 34: move the camera to a location value.
    Camera(i32),
    /// Natives 43 / 44 / 109 / 110: queue a message.
    Message(Message),
    /// Natives 45 / 48 / 64 / 233: walk an entity to a map point.
    Walk {
        /// Entity index.
        entity: u32,
        /// Target x.
        x: i32,
        /// Target y.
        y: i32,
    },
    /// Native 96: teleport an entity to a map point (`None` = off the map: deactivated).
    Teleport {
        /// Entity index.
        entity: u32,
        /// Target, or off map.
        to: Option<(i32, i32)>,
    },
    /// Natives 49..=53: an animation on an actor (not modelled: recorded like a stub) whose
    /// completion token completes at once.
    Animation {
        /// Native id.
        id: u32,
        /// Actor element handle.
        actor: i32,
        /// Animation number (0 for the natives without one).
        anim: i32,
    },
    /// Native 32: hold the sequence until every token issued since the previous barrier completed.
    Barrier,
    /// A recorded no-op element (remarks, presentation).
    Stub {
        /// Native id.
        id: u32,
    },
}

/// A completion token issued by a sequence element that takes time; a [`SeqElement::Barrier`]
/// waits for all of them (`docs/formats/scb.md`, native 32).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeqToken {
    /// A walk of `entity` to `(x, y)`: complete when the entity is no longer walking to that
    /// point (arrived, gave up, was ordered elsewhere, deactivated or died). Hypothesis: the
    /// original waits for the arrival of the actor; walk failure is treated as completion so a
    /// blocked cutscene cannot stall a mission.
    Walk {
        /// Entity index.
        entity: u32,
        /// Target x.
        x: i32,
        /// Target y.
        y: i32,
    },
    /// An animation (natives 49..=53): complete at once, the engine has no animation model yet.
    Animation {
        /// Native id.
        id: u32,
    },
}

/// What a sequence is waiting for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeqWait {
    /// Nothing: the next element runs.
    None,
    /// This many more ticks.
    Ticks(u32),
    /// The text request with this id to be dismissed.
    Text(u64),
    /// Every token of the sequence to complete (native 32).
    Barrier,
}

/// An active sequence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sequence {
    /// Elements in order.
    pub elements: Vec<SeqElement>,
    /// Next element to run.
    pub next: u32,
    /// Current wait.
    pub wait: SeqWait,
    /// Tokens issued since the previous barrier.
    #[serde(default)]
    pub tokens: Vec<SeqToken>,
}

/// A text the script asked to show (natives 202 / 203).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextRequest {
    /// Request id (monotonic).
    pub id: u64,
    /// Text index in the level's text list.
    pub text: i32,
    /// Whether a sequence waits for its dismissal.
    pub blocking: bool,
}

/// An objective (natives 26 / 27).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Objective {
    /// Index into the level's short-briefing list.
    pub index: i32,
    /// Primary (`1`) or secondary (`0`) objective.
    pub primary: bool,
    /// Accomplished.
    pub done: bool,
}

/// An element attribute (natives 117 / 118).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attribute {
    /// Element handle.
    pub element: i32,
    /// Attribute number.
    pub attr: i32,
    /// Value.
    pub value: i32,
}

/// One call of a native the engine does not know (lenient mode).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownCall {
    /// Native id.
    pub id: u32,
    /// Arguments as pushed.
    pub args: Vec<i32>,
}

/// The registry of hypothesis sources (module documentation, "Hypotheses and taint";
/// ADR-0008): one variant per source, recorded in `VmState::assumptions` at the point where the
/// engine takes the hypothesis (once per kind and id). The set is complete by construction:
/// every place in the core that reads a low-confidence opcode, a policy native, a stub, a
/// lenient unknown native or one of the engine's own hypotheses records its variant, so a VM
/// whose set is empty took no hypothesis the engine knows of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Assumption {
    /// The script depended on the recorded stub `id`: an effectful stub was invoked (its
    /// documented effect is not modelled; `natives::NATIVE_TAINT`, `Taint::Effect`) or a
    /// stub's fabricated result (0 or a policy value) was consumed. The presentation-only
    /// stubs (`Taint::Presentation`) record nothing on the call; reading their result, if any,
    /// still records this.
    StubResult(u32),
    /// The implemented native `id` was called, and the engine's reading of its row is a policy
    /// rather than an observation (`natives::NATIVE_TAINT`, `Taint::Policy`: 98, 128, 140, 245
    /// and the rest of the policy list).
    Policy(u32),
    /// An instruction of a low-confidence opcode was executed ([`LOW_CONFIDENCE_OPCODES`]:
    /// `0x24` read as `>=`, `0x28` as `!=`, `0x2b` as a fixed-point `<`, `0x14` rounded to
    /// 24.8).
    Opcode(u8),
    /// A jump to the unresolved label `0xffff` left its function ([`Instr::LeaveUnresolved`]).
    UnresolvedJump,
    /// An unknown native was called in lenient mode and answered with a fabricated 0
    /// (`MissionSpec::lenient_natives`).
    UnknownNative(u32),
    /// The unmeasured part of the sight decided that a soldier saw a player character and his
    /// state changed on it (he noticed him, or an alert of his was refreshed by the sighting):
    /// the rear radius (`ai::REAR_SIGHT_RADIUS`, a hypothesis from one event) or the crouch
    /// divisor (`ai::CROUCH_VIEW_DIVISOR`, a hypothesis). The cone itself (half angle, the
    /// elliptical reach, the binding to the facing: `ai::VIEW_CONE_HALF_ANGLE_256`,
    /// `ai::VIEW_RANGE`, `ai::VIEW_Y_COMPRESSION`) is measured
    /// (`docs/original/h01-measurements-2.md` 6) and a standing character seen inside it
    /// records nothing. Recorded by the stealth layer where the sighting first mutates the
    /// state, whether or not any script handler exists.
    SightCone,
    /// A running player character was heard from beyond the measured bound of the noise
    /// radius (`ai::NOISE_MEASURED_RADIUS`, 330 px: soldiers detected a run from at least that
    /// far) and within the engine's chosen radius (`ai::RUN_NOISE_RADIUS`, 350 px), and the
    /// soldier's state changed on it. A run heard within the measured bound records nothing.
    NoiseRadius,
    /// The alert sequence (hypotheses: the noticed -> alarm -> search sequence a sighting
    /// starts, the re-plan distance while searching) mutated a soldier's state. The
    /// immediate charge on a heard run is measured and records nothing of its own
    /// (`docs/original/stealth-and-combat.md` 8.6); what it stores besides is the timeout
    /// ([`Assumption::AlertTimeout`]).
    AlertPolicy,
    /// The alert timeout and the return policy (hypotheses: the five seconds of
    /// `ai::ALERT_TIMEOUT_TICKS` an alerted soldier keeps searching, the return to the post
    /// afterwards or after a knock-out) mutated a soldier's state: recorded before the charge
    /// on a heard run stores the timeout, when an alarm or a sighting (re)starts it and when
    /// the return begins (Codex review 10, finding 1: the charge itself is measured, the
    /// timeout it stores is not).
    AlertTimeout,
    /// The attack policy mutated state; the rule names which part ([`AttackRule`]).
    AttackPolicy(AttackRule),
    /// The knock-out policy (hypotheses: the blow always fells a victim below the immune
    /// resistance, the base duration and its scaling by `p4`, the immune threshold) mutated
    /// state: a victim fell or shrugged the blow off; also native 90 reporting a knocked-out
    /// actor, native 128 refusing one, or a knock-out action id reaching an `ActionChange`.
    KnockOut,
    /// The profile stat hypotheses (`p0` hit points, `p4` knock-out resistance) were consulted.
    ProfileStats,
    /// A script wait (native 56) or the `Hourglass` time was consumed under the 25-versus-60
    /// tick reading of the scripts' time unit. The animation clock is measured (`anim.rs`);
    /// the scripts' unit is not, so the reading stays a hypothesis.
    TickRate,
    /// `IsTaken` returned non-zero and the scroll was deactivated (the take-on-non-zero rule,
    /// `World::resolve_pickups`): what makes a scroll vanish after its reading is a
    /// hypothesis (observed: the tutorial scrolls stay, the training-start scroll vanishes;
    /// the `SKRO` record's `flags5` bit 0 as "stays after reading" is the analyst's
    /// hypothesis, not modelled). The reading itself (a click on the scroll orders the walk,
    /// the stop about 18 px short, the pause before the page) is measured
    /// (`docs/original/h01-measurements-2.md` 1.2 / 1.4) and records nothing.
    ScrollPickup,
    /// A zone callback fired on the first scan for a character standing inside the zone at
    /// load (presence starts empty: hypothesis).
    ZoneAtLoad,
    /// A sequence barrier was released by a walk that completed without arriving (the path
    /// failed, the actor was ordered elsewhere, deactivated or died): the original presumably
    /// waits for the arrival (`docs/formats/scb.md`, "Engine notes").
    WalkCompletion,
    /// An `ActionChange(previous, new)` was delivered: the parameter order is a hypothesis
    /// (the actor classes compare the second parameter with 141).
    ActionChangeOrder,
    /// The campaign graph hypothesis chose a successor mission (recorded by the app).
    CampaignGraph,
    /// A profile index or sprite fell back to a default under `OPENSHERWOOD_LENIENT_ASSETS`
    /// (recorded by the app through `MissionSpec::assumptions`).
    LenientAssets,
    /// A melee action id (the stance 54, the strike 59, the powerful blow 75, the flinch 104)
    /// or a death's fall / lying id (41 / 44 / 47 / 48 of a dead actor) reached an
    /// `ActionChange` handler: which id the original plays in each case is inferred by eye
    /// (`sprite-animations.md`).
    CombatActions,
    /// A player character died while another one was still alive and present, and the world
    /// raised `hero_dead` (the loss): measured for a lone hero only (`combat-measurements.md`
    /// 4).
    HeroDeathLoss,
    /// A purse or an item of an unknown kind was taken (`World::resolve_pickups`): the money a
    /// purse holds ([`PURSE_MONEY_PER_STACK`] per stack unit) and the purse counter's rise,
    /// and an unknown kind's effect (it only disappears), are hypotheses (no such item was
    /// taken in the oracle sessions, `docs/original/h01-measurements-2.md` 1.3 / 8). The
    /// gesture (a click on the item orders the walk), the take on arrival after the stoop and
    /// an arrow pile adding its stack (`unknown_b`) to the arrows are measured and record
    /// nothing; native 235 reading the taken flag records `Policy(235)` itself.
    ItemPickup,
}

/// Which part of the attack policy [`Assumption::AttackPolicy`] names (`crate::ai`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttackRule {
    /// The reach bands of the attack order: an approach from behind (`ai::BACK_ARC_HALF_ANGLE_256`)
    /// ends in the knock-out blow at `ai::PUNCH_REACH` rather than the measured fight at
    /// `ai::FIGHT_RANGE`, or a drawn figure / a profile without the blow turns such an
    /// approach into the fight. Recorded when an attack order resolved with the victim's back
    /// to the attacker.
    Reach,
    /// A player character's automatic strike against a soldier never lands: the reading of
    /// `combat-measurements.md` 1.3 (225 s of click attacks against a pole arm at 52 px never
    /// hurt him: the pole arm's reach band or a block) is inferred from one fighter pair and
    /// applied to every soldier. Recorded when such a strike starts or resolves.
    Block,
    /// A chance or a cadence the engine draws from the RNG: the soldier's swing interval
    /// (`ai::SOLDIER_SWING_TICKS` with `ai::SWING_JITTER_TICKS`, the engine's spread within
    /// the measured mean), his two-in-three hits (derived from the cadence) and the hero's
    /// powerful blow landing one time in three (from 2 of 6 strokes, `combat-measurements.md`
    /// 1.4). Recorded when a swing is timed or a blow is resolved by a roll.
    HitChance,
    /// A soldier's foe left the fight alive and the soldier stood his ground rather than
    /// chasing: measured for the halberdier (`combat-measurements.md` 3), a hypothesis for
    /// every other kind.
    PostBound,
    /// Several player characters attack one soldier: the engine lets him fight one at a time
    /// while the others wait at reach (Codex review 10, finding 7; the measurements of
    /// `combat-measurements.md` were one-on-one). Recorded when an attacker in reach waits
    /// because his victim is engaged with another.
    MultiParty,
}

impl AttackRule {
    fn tag(self) -> u8 {
        match self {
            AttackRule::Reach => 1,
            AttackRule::Block => 2,
            AttackRule::HitChance => 3,
            AttackRule::PostBound => 4,
            AttackRule::MultiParty => 5,
        }
    }
}

impl Assumption {
    fn encode(self, e: &mut Encoder) {
        match self {
            Assumption::StubResult(id) => e.u8(1).u32(id),
            Assumption::SightCone => e.u8(2),
            Assumption::KnockOut => e.u8(3),
            Assumption::ProfileStats => e.u8(4),
            Assumption::TickRate => e.u8(5),
            Assumption::CampaignGraph => e.u8(6),
            Assumption::LenientAssets => e.u8(7),
            Assumption::Policy(id) => e.u8(8).u32(id),
            Assumption::Opcode(op) => e.u8(9).u8(op),
            Assumption::UnresolvedJump => e.u8(10),
            Assumption::UnknownNative(id) => e.u8(11).u32(id),
            Assumption::ScrollPickup => e.u8(12),
            Assumption::ZoneAtLoad => e.u8(13),
            Assumption::WalkCompletion => e.u8(14),
            Assumption::ActionChangeOrder => e.u8(15),
            Assumption::AttackPolicy(rule) => e.u8(16).u8(rule.tag()),
            Assumption::NoiseRadius => e.u8(17),
            Assumption::AlertPolicy => e.u8(18),
            Assumption::CombatActions => e.u8(19),
            Assumption::HeroDeathLoss => e.u8(20),
            Assumption::ItemPickup => e.u8(21),
            Assumption::AlertTimeout => e.u8(22),
        };
    }

    /// Whether a snapshot may carry this assumption: a `StubResult` names a stub, a `Policy` a
    /// policy native, an `Opcode` a low-confidence opcode, an `UnknownNative` an id without a
    /// row (and only in lenient mode).
    fn well_formed(self, lenient: bool) -> Result<(), String> {
        use crate::natives::{NativeStatus, Taint, native_status, native_taint};
        match self {
            Assumption::StubResult(id) if native_status(id) != NativeStatus::Stub => Err(format!(
                "vm assumption names native {id}, which is not a stub"
            )),
            Assumption::Policy(id)
                if !matches!(native_taint(id), Some(Taint::Policy | Taint::Branch)) =>
            {
                Err(format!(
                    "vm assumption names native {id}, which is not a policy native"
                ))
            }
            Assumption::Opcode(op) if !LOW_CONFIDENCE_OPCODES.contains(&op) => Err(format!(
                "vm assumption names opcode {op:#04x}, which is not of low confidence"
            )),
            Assumption::UnknownNative(id) if native_status(id) != NativeStatus::Unknown => {
                Err(format!("vm assumption names native {id}, which is known"))
            }
            Assumption::UnknownNative(_) if !lenient => {
                Err("vm assumption names an unknown native without lenient mode".into())
            }
            _ => Ok(()),
        }
    }
}

/// Why the script is faulted (`VmState::fault`, sticky, hashed): a deterministic condition
/// under which the engine stopped running the script as written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fault {
    /// An unknown native was called in strict mode (the callback stopped there).
    UnknownNative(u32),
    /// A native was called with an argument count that differs from its signature (the
    /// callback stopped there).
    ArityMismatch(u32),
    /// The action change queue was full when a change arrived: the exactly-once delivery can
    /// no longer be honoured, so the script is faulted rather than the change dropped.
    ActionQueueOverflow,
    /// A script call would have pushed the frame beyond [`MAX_FRAMES`] (unbounded recursion):
    /// the callback stopped at the call, its destination untouched and its transaction, if
    /// any, rolled back (Codex review 10, finding 3).
    CallStackOverflow,
}

impl Fault {
    fn encode(self, e: &mut Encoder) {
        match self {
            Fault::UnknownNative(id) => e.u8(1).u32(id),
            Fault::ArityMismatch(id) => e.u8(2).u32(id),
            Fault::ActionQueueOverflow => e.u8(3),
            Fault::CallStackOverflow => e.u8(4),
        };
    }
}

/// A queued `ActionChange(previous, new)` for the class bound to an actor (module
/// documentation, "Action changes").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionChange {
    /// Class bound to the actor.
    pub class: u32,
    /// The action id reported before the change.
    pub previous: i32,
    /// The action id reported now.
    pub new: i32,
}

/// Action ids of the knock-out (`crate::ai::actions`) whose delivery records
/// [`Assumption::KnockOut`] (of a living actor; a dead one fell by a blow and records
/// [`Assumption::CombatActions`] instead).
const KNOCK_OUT_ACTIONS: [u32; 6] = [41, 44, 47, 48, 49, 123];
/// Action ids of the melee (`crate::ai::actions`) whose delivery records
/// [`Assumption::CombatActions`].
const COMBAT_ACTIONS: [u32; 4] = [54, 59, 75, 104];

/// Diagnostic counters: neither in the snapshot nor in the hash (a restored world counts afresh;
/// ADR-0008). Every counter saturates.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counters {
    /// Instructions executed.
    pub instructions: u64,
    /// Callbacks invoked.
    pub callbacks: u64,
    /// Callbacks, tick phases, sequences and walks stopped by the work budget.
    pub budget_aborts: u64,
    /// Run-time faults (bad slot, missing parameter, stack underflow, deep recursion).
    pub faults: u64,
    /// Callbacks stopped by an unknown native (strict mode).
    pub traps: u64,
    /// Messages delivered.
    pub messages_delivered: u64,
    /// Messages dropped because the queue was full.
    pub messages_dropped: u64,
    /// Text requests dropped because the queue was full or the id counter saturated.
    pub texts_dropped: u64,
    /// Calls of natives with no implementation, by id.
    pub unknown_natives: BTreeMap<u32, u64>,
    /// Calls of natives implemented as recorded no-ops, by id.
    pub stub_natives: BTreeMap<u32, u64>,
    /// Objective completions for objectives that were never added.
    pub objective_done_before_added: u64,
    /// Calls of native 90 that reported an actor out of action (knocked out or dead).
    pub out_of_action_true: u64,
    /// Native calls whose argument count differed from the signature (a trap), by id.
    pub arity_mismatches: BTreeMap<u32, u64>,
    /// Queued callbacks rolled back because the budget cut them short (each is retried whole
    /// on the next tick).
    pub transactions_rolled_back: u64,
}

/// The script-visible state a queued callback may mutate, captured before it runs and put back
/// when the budget cuts it short (module documentation, "Action changes"). The VM part is a
/// copy of every mutable field but the program, the digest, the path table, the presence sets
/// and the queue itself; the world part is the entities the callback's natives touched (captured
/// lazily through [`World::vm_touch_entity`]), the selection and the camera. Never serialised:
/// a snapshot is quiescent, no transaction is open between callbacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    class_vars: Vec<Vec<i32>>,
    mission_vars: Vec<i32>,
    objectives: Vec<Objective>,
    debriefing: Option<i32>,
    messages: Vec<Message>,
    sequences: Vec<Sequence>,
    texts: Vec<TextRequest>,
    next_text_id: u64,
    camera_target: Option<(i32, i32)>,
    money: i32,
    patches: BTreeSet<i32>,
    actions: BTreeMap<i32, i32>,
    attributes: Vec<Attribute>,
    states: BTreeMap<i32, i32>,
    inactive_elements: BTreeSet<i32>,
    unknown_calls: Vec<UnknownCall>,
    rng: Rng,
    /// Entities touched by the callback, by index, as they were before it ran.
    entities: BTreeMap<usize, crate::world::Entity>,
    selected: Option<crate::world::EntityId>,
    camera: (i32, i32),
}

/// Run-time state of the VM (part of [`World`], of the snapshot and of the hash).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmState {
    /// The program.
    pub program: Program,
    /// [`Program::digest`] of `program`, computed at load and checked by `validate`.
    pub program_digest: String,
    /// Class variable storage, one block per class.
    pub class_vars: Vec<Vec<i32>>,
    /// Mission variables (natives 0 / 1 / 2), exactly [`MISSION_VARIABLES`] entries.
    pub mission_vars: Vec<i32>,
    /// Objectives in the order they were added.
    pub objectives: Vec<Objective>,
    /// Debriefing variant chosen by native 28.
    pub debriefing: Option<i32>,
    /// Messages queued for the next tick.
    pub messages: Vec<Message>,
    /// Active sequences, first is running.
    pub sequences: Vec<Sequence>,
    /// Elements collected between natives 30 and 31 of the running callback.
    pub collecting: Option<Vec<SeqElement>>,
    /// Pending texts, first is shown.
    pub texts: Vec<TextRequest>,
    /// Next text request id.
    pub next_text_id: u64,
    /// Last camera target set by natives 33 / 34 (map pixels).
    pub camera_target: Option<(i32, i32)>,
    /// `CheckVictoryCondition` returned 1.
    pub mission_won: bool,
    /// `CheckVictoryCondition` returned 2 (sticky, like `mission_won`).
    #[serde(default)]
    pub mission_lost: bool,
    /// The player's money (natives 236 / 237).
    #[serde(default)]
    pub money: i32,
    /// Active patches (natives 144 / 145).
    pub patches: BTreeSet<i32>,
    /// Player action availability flags (native 196).
    pub actions: BTreeMap<i32, i32>,
    /// Element attributes, sorted by `(element, attr)`.
    pub attributes: Vec<Attribute>,
    /// Element states (natives 193 / 194).
    pub states: BTreeMap<i32, i32>,
    /// Elements that are not entities and were deactivated (native 113).
    pub inactive_elements: BTreeSet<i32>,
    /// `(class, entity)` pairs currently inside the class's zone.
    pub zone_presence: BTreeSet<(u32, u32)>,
    /// Handles of the pick-up items ([`Element::Item`]) a player character took (sticky: native
    /// 235 reads it; the item is also deactivated, so a later 114 shows a taken item again
    /// without un-taking it).
    #[serde(default)]
    pub taken_items: BTreeSet<i32>,
    /// Program index (into `World::programs`) per `RAIL` index (native 9 / 132).
    pub paths: Vec<Option<u32>>,
    /// Lenient natives (`MissionSpec::lenient_natives`): an unknown native is a recorded no-op
    /// instead of a trap.
    pub lenient: bool,
    /// The script is faulted (sticky): the first deterministic condition under which the engine
    /// stopped running it as written ([`Fault`]); `None` while it runs as written.
    #[serde(default)]
    pub fault: Option<Fault>,
    /// Unknown native calls in lenient mode, in order, with their arguments (bounded).
    pub unknown_calls: Vec<UnknownCall>,
    /// The hypotheses and stub values the script-visible state depended on so far (module
    /// documentation, "Hypotheses and taint"); non-empty = tainted.
    #[serde(default)]
    pub assumptions: BTreeSet<Assumption>,
    /// Action changes not yet delivered to their `ActionChange` handler, in order (module
    /// documentation, "Action changes").
    #[serde(default)]
    pub pending_action_changes: Vec<ActionChange>,
    /// The `script` RNG stream (native 161).
    pub rng: Rng,
    /// Call stack (empty between callbacks; a snapshot must be quiescent).
    pub frames: Vec<Frame>,
    /// Native argument stack (empty between callbacks).
    pub arg_stack: Vec<i32>,
    /// Script call parameter stack (empty between callbacks).
    pub param_stack: Vec<i32>,
    /// Work units left in the current tick (not serialised: granted at the start of every tick
    /// and once at load; events and dismissals draw from what is left).
    #[serde(skip)]
    pub budget: u64,
    /// Diagnostics (not serialised, not hashed).
    #[serde(skip)]
    pub counters: Counters,
    /// The open transaction of a queued callback (only while one runs; never serialised).
    #[serde(skip)]
    pub transaction: Option<Transaction>,
}

impl VmState {
    /// Fresh state for a program.
    #[must_use]
    pub fn new(program: Program, paths: Vec<Option<u32>>, seed: u64, lenient: bool) -> Self {
        let class_vars = program
            .classes
            .iter()
            .map(|c| vec![0; c.variable_count as usize])
            .collect();
        let program_digest = program.digest();
        VmState {
            program,
            program_digest,
            class_vars,
            mission_vars: vec![0; MISSION_VARIABLES],
            objectives: Vec::new(),
            debriefing: None,
            messages: Vec::new(),
            sequences: Vec::new(),
            collecting: None,
            texts: Vec::new(),
            next_text_id: 1,
            camera_target: None,
            mission_won: false,
            mission_lost: false,
            money: 0,
            patches: BTreeSet::new(),
            actions: BTreeMap::new(),
            attributes: Vec::new(),
            states: BTreeMap::new(),
            inactive_elements: BTreeSet::new(),
            zone_presence: BTreeSet::new(),
            taken_items: BTreeSet::new(),
            paths,
            lenient,
            fault: None,
            unknown_calls: Vec::new(),
            assumptions: BTreeSet::new(),
            pending_action_changes: Vec::new(),
            rng: Rng::new(seed, SCRIPT_RNG_STREAM),
            frames: Vec::new(),
            arg_stack: Vec::new(),
            param_stack: Vec::new(),
            budget: WORK_BUDGET_PER_TICK,
            counters: Counters::default(),
            transaction: None,
        }
    }

    /// The script is faulted (`fault` is set).
    #[must_use]
    pub fn faulted(&self) -> bool {
        self.fault.is_some()
    }

    /// Mark the script faulted with `fault` unless it already is (the first fault is kept).
    pub fn set_fault(&mut self, fault: Fault) {
        if self.fault.is_none() {
            self.fault = Some(fault);
        }
    }

    /// Record a hypothesis source (module documentation, "Hypotheses and taint").
    pub fn assume(&mut self, assumption: Assumption) {
        self.assumptions.insert(assumption);
    }

    /// Capture the mutable VM part of a [`Transaction`], charging one work unit per value
    /// copied first; `None` (nothing copied, the budget zero) when the copy does not fit.
    fn capture(&mut self) -> Option<Transaction> {
        let sequences: usize = self
            .sequences
            .iter()
            .map(|s| 1 + s.elements.len() + s.tokens.len())
            .sum();
        let unknown: usize = self.unknown_calls.iter().map(|c| 1 + c.args.len()).sum();
        let cost = self.class_vars.iter().map(Vec::len).sum::<usize>()
            + self.mission_vars.len()
            + self.objectives.len()
            + self.messages.len()
            + sequences
            + self.texts.len()
            + self.patches.len()
            + self.actions.len()
            + self.attributes.len()
            + self.states.len()
            + self.inactive_elements.len()
            + unknown
            + 8;
        if !charge(self, cost as u64) {
            return None;
        }
        Some(Transaction {
            class_vars: self.class_vars.clone(),
            mission_vars: self.mission_vars.clone(),
            objectives: self.objectives.clone(),
            debriefing: self.debriefing,
            messages: self.messages.clone(),
            sequences: self.sequences.clone(),
            texts: self.texts.clone(),
            next_text_id: self.next_text_id,
            camera_target: self.camera_target,
            money: self.money,
            patches: self.patches.clone(),
            actions: self.actions.clone(),
            attributes: self.attributes.clone(),
            states: self.states.clone(),
            inactive_elements: self.inactive_elements.clone(),
            unknown_calls: self.unknown_calls.clone(),
            rng: self.rng.clone(),
            entities: BTreeMap::new(),
            selected: None,
            camera: (0, 0),
        })
    }

    /// Put the VM part of a transaction back (the assumptions recorded meanwhile stay: the
    /// taint only grows).
    fn roll_back(&mut self, t: &Transaction) {
        self.class_vars.clone_from(&t.class_vars);
        self.mission_vars.clone_from(&t.mission_vars);
        self.objectives.clone_from(&t.objectives);
        self.debriefing = t.debriefing;
        self.messages.clone_from(&t.messages);
        self.sequences.clone_from(&t.sequences);
        self.texts.clone_from(&t.texts);
        self.next_text_id = t.next_text_id;
        self.camera_target = t.camera_target;
        self.money = t.money;
        self.patches.clone_from(&t.patches);
        self.actions.clone_from(&t.actions);
        self.attributes.clone_from(&t.attributes);
        self.states.clone_from(&t.states);
        self.inactive_elements.clone_from(&t.inactive_elements);
        self.unknown_calls.clone_from(&t.unknown_calls);
        self.rng = t.rng.clone();
    }

    /// Check every invariant a snapshot must satisfy (`program_count` is the number of rail
    /// programs of the world, `entity_count` its entities).
    pub fn validate(&self, program_count: usize, entity_count: usize) -> Result<(), String> {
        self.program.validate()?;
        if self.program_digest != self.program.digest() {
            return Err("vm program digest does not match the program".into());
        }
        // Callbacks never yield: between ticks there is no frame, no pushed argument and no
        // sequence being collected. A snapshot that says otherwise is not a state this VM can
        // produce, and resuming it is not defined.
        if !self.frames.is_empty()
            || !self.arg_stack.is_empty()
            || !self.param_stack.is_empty()
            || self.collecting.is_some()
        {
            return Err(
                "vm snapshot is not quiescent (frames, stacks or a collecting sequence)".into(),
            );
        }
        if self.next_text_id == 0 {
            return Err("vm text id counter must be at least 1".into());
        }
        if self
            .program
            .elements
            .iter()
            .any(|e| matches!(e, Element::Actor(i) if *i as usize >= entity_count))
        {
            return Err("vm element table names an entity that does not exist".into());
        }
        if self.class_vars.len() != self.program.classes.len() {
            return Err("vm class variable blocks do not match the classes".into());
        }
        for (i, (vars, c)) in self
            .class_vars
            .iter()
            .zip(&self.program.classes)
            .enumerate()
        {
            if vars.len() != c.variable_count as usize {
                return Err(format!("vm class {i} variable block has the wrong size"));
            }
        }
        if self.mission_vars.len() != MISSION_VARIABLES {
            return Err("vm mission variables must be exactly 64".into());
        }
        if self.objectives.len() > MAX_QUEUE
            || self.messages.len() > MAX_QUEUE
            || self.sequences.len() > MAX_QUEUE
            || self.texts.len() > MAX_QUEUE
            || self.attributes.len() > MAX_QUEUE * 16
            || self.states.len() > MAX_QUEUE * 16
            || self.inactive_elements.len() > MAX_QUEUE * 16
            || self.patches.len() > MAX_QUEUE
            || self.actions.len() > MAX_QUEUE
            || self.zone_presence.len() > MAX_QUEUE * 16
            || self.taken_items.len() > MAX_QUEUE * 16
            || self.arg_stack.len() > MAX_QUEUE
            || self.param_stack.len() > MAX_QUEUE
        {
            return Err("vm queue too long".into());
        }
        if self
            .collecting
            .as_ref()
            .is_some_and(|c| c.len() > MAX_QUEUE)
        {
            return Err("vm collected sequence too long".into());
        }
        let mut total_elements = 0usize;
        for s in &self.sequences {
            if s.elements.len() > MAX_QUEUE || s.next as usize > s.elements.len() {
                return Err("vm sequence out of range".into());
            }
            total_elements = total_elements.saturating_add(s.elements.len());
            if total_elements > MAX_SEQUENCE_ELEMENTS {
                return Err("vm sequences hold too many elements".into());
            }
            if s.tokens.len() > MAX_QUEUE {
                return Err("vm sequence has too many tokens".into());
            }
            let entity_ok = |e: u32| (e as usize) < entity_count;
            let coord_ok = |v: i32| v.unsigned_abs() <= MAX_LOCATION_COORD as u32;
            for el in &s.elements {
                let ok = match *el {
                    SeqElement::Walk { entity, x, y } => {
                        entity_ok(entity) && coord_ok(x) && coord_ok(y)
                    }
                    SeqElement::Teleport { entity, to } => {
                        entity_ok(entity) && to.is_none_or(|(x, y)| coord_ok(x) && coord_ok(y))
                    }
                    _ => true,
                };
                if !ok {
                    return Err("vm sequence element out of range".into());
                }
            }
            for t in &s.tokens {
                if let SeqToken::Walk { entity, x, y } = *t
                    && !(entity_ok(entity) && coord_ok(x) && coord_ok(y))
                {
                    return Err("vm sequence token out of range".into());
                }
            }
            if let SeqWait::Text(id) = s.wait
                && id >= self.next_text_id
            {
                return Err("vm sequence waits for a text id beyond the counter".into());
            }
        }
        if self.texts.iter().any(|t| t.id >= self.next_text_id) {
            return Err("vm text id beyond the counter".into());
        }
        if self.texts.windows(2).any(|w| w[0].id >= w[1].id) {
            return Err("vm text ids are not increasing".into());
        }
        if self
            .attributes
            .windows(2)
            .any(|w| (w[0].element, w[0].attr) >= (w[1].element, w[1].attr))
        {
            return Err("vm attributes are not sorted".into());
        }
        for &(c, e) in &self.zone_presence {
            if c as usize >= self.program.classes.len() || e as usize >= entity_count {
                return Err("vm zone presence out of range".into());
            }
        }
        if self
            .taken_items
            .iter()
            .any(|&h| !matches!(self.element(h), Some(Element::Item { .. })))
        {
            return Err("vm taken item is not a pick-up item of the table".into());
        }
        if self.paths.len() > MAX_TABLE
            || self
                .paths
                .iter()
                .flatten()
                .any(|&p| p as usize >= program_count)
        {
            return Err("vm path table out of range".into());
        }
        if self.unknown_calls.len() > MAX_QUEUE
            || self.unknown_calls.iter().any(|c| c.args.len() > MAX_TABLE)
        {
            return Err("vm unknown call log too long".into());
        }
        if !self.lenient && !self.unknown_calls.is_empty() {
            return Err("vm unknown call log without lenient mode".into());
        }
        if self.assumptions.len() > MAX_QUEUE {
            return Err("vm assumption set too large".into());
        }
        for a in &self.assumptions {
            a.well_formed(self.lenient)?;
        }
        if self.transaction.is_some() {
            return Err("vm snapshot is not quiescent (a transaction is open)".into());
        }
        if self.pending_action_changes.len() > MAX_QUEUE {
            return Err("vm action change queue too long".into());
        }
        if self
            .pending_action_changes
            .iter()
            .any(|c| c.class as usize >= self.program.classes.len())
        {
            return Err("vm action change names a class that does not exist".into());
        }
        self.rng.validate()
    }

    /// Whether the script executed over any hypothesis source of the registry ([`Assumption`]):
    /// a won or lost mission of a tainted VM is not authoritative (ADR-0008). The set is
    /// complete by construction, so `false` means no known hypothesis was taken.
    #[must_use]
    pub fn tainted(&self) -> bool {
        !self.assumptions.is_empty()
    }

    /// Encode the `scripts` hash part (program identity and script-visible state).
    pub fn encode_scripts(&self, e: &mut Encoder) {
        e.str(&self.program_digest);
        e.u32(self.class_vars.len() as u32);
        for vars in &self.class_vars {
            e.u32(vars.len() as u32);
            for v in vars {
                e.i32(*v);
            }
        }
        e.u32(self.mission_vars.len() as u32);
        for v in &self.mission_vars {
            e.i32(*v);
        }
        e.u32(self.objectives.len() as u32);
        for o in &self.objectives {
            e.i32(o.index).u8(u8::from(o.primary)).u8(u8::from(o.done));
        }
        match self.debriefing {
            Some(d) => e.u8(1).i32(d),
            None => e.u8(0),
        };
        e.u8(u8::from(self.mission_won));
        e.u8(u8::from(self.mission_lost)).i32(self.money);
        e.u32(self.patches.len() as u32);
        for p in &self.patches {
            e.i32(*p);
        }
        e.u32(self.actions.len() as u32);
        for (k, v) in &self.actions {
            e.i32(*k).i32(*v);
        }
        e.u32(self.attributes.len() as u32);
        for a in &self.attributes {
            e.i32(a.element).i32(a.attr).i32(a.value);
        }
        e.u32(self.states.len() as u32);
        for (k, v) in &self.states {
            e.i32(*k).i32(*v);
        }
        e.u32(self.inactive_elements.len() as u32);
        for k in &self.inactive_elements {
            e.i32(*k);
        }
        e.u32(self.taken_items.len() as u32);
        for k in &self.taken_items {
            e.i32(*k);
        }
        e.u32(self.paths.len() as u32);
        for p in &self.paths {
            match p {
                Some(p) => e.u8(1).u32(*p),
                None => e.u8(0),
            };
        }
        e.u8(u8::from(self.lenient));
        match self.fault {
            Some(f) => {
                e.u8(1);
                f.encode(e);
            }
            None => {
                e.u8(0);
            }
        }
        e.u32(self.unknown_calls.len() as u32);
        for c in &self.unknown_calls {
            e.u32(c.id).u32(c.args.len() as u32);
            for a in &c.args {
                e.i32(*a);
            }
        }
        e.u32(self.assumptions.len() as u32);
        for a in &self.assumptions {
            a.encode(e);
        }
    }

    /// Encode the `scheduler` hash part (queues, sequences with their tokens, texts, presence).
    /// Frames and stacks are not encoded: they are empty whenever a hash is taken (`validate`
    /// refuses a snapshot where they are not).
    pub fn encode_scheduler(&self, e: &mut Encoder) {
        e.u32(self.messages.len() as u32);
        for m in &self.messages {
            encode_message(e, m);
        }
        e.u32(self.sequences.len() as u32);
        for s in &self.sequences {
            e.u32(s.next);
            match s.wait {
                SeqWait::None => e.u8(0),
                SeqWait::Ticks(n) => e.u8(1).u32(n),
                SeqWait::Text(id) => e.u8(2).u64(id),
                SeqWait::Barrier => e.u8(3),
            };
            e.u32(s.elements.len() as u32);
            for el in &s.elements {
                encode_element(e, el);
            }
            e.u32(s.tokens.len() as u32);
            for t in &s.tokens {
                match *t {
                    SeqToken::Walk { entity, x, y } => e.u8(1).u32(entity).i32(x).i32(y),
                    SeqToken::Animation { id } => e.u8(2).u32(id),
                };
            }
        }
        match &self.collecting {
            Some(els) => {
                e.u8(1).u32(els.len() as u32);
                for el in els {
                    encode_element(e, el);
                }
            }
            None => {
                e.u8(0);
            }
        }
        e.u32(self.texts.len() as u32);
        for t in &self.texts {
            e.u64(t.id).i32(t.text).u8(u8::from(t.blocking));
        }
        e.u64(self.next_text_id);
        match self.camera_target {
            Some((x, y)) => e.u8(1).i32(x).i32(y),
            None => e.u8(0),
        };
        e.u32(self.zone_presence.len() as u32);
        for (c, en) in &self.zone_presence {
            e.u32(*c).u32(*en);
        }
        e.u32(self.pending_action_changes.len() as u32);
        for c in &self.pending_action_changes {
            e.u32(c.class).i32(c.previous).i32(c.new);
        }
    }

    fn attribute_index(&self, element: i32, attr: i32) -> Result<usize, usize> {
        self.attributes
            .binary_search_by_key(&(element, attr), |a| (a.element, a.attr))
    }

    /// Attribute value (0 when unset).
    #[must_use]
    pub fn attribute(&self, element: i32, attr: i32) -> i32 {
        self.attribute_index(element, attr)
            .map_or(0, |i| self.attributes[i].value)
    }

    /// Set an attribute.
    pub fn set_attribute(&mut self, element: i32, attr: i32, value: i32) {
        match self.attribute_index(element, attr) {
            Ok(i) => self.attributes[i].value = value,
            Err(i) => {
                if self.attributes.len() < MAX_QUEUE * 16 {
                    self.attributes.insert(
                        i,
                        Attribute {
                            element,
                            attr,
                            value,
                        },
                    );
                }
            }
        }
    }

    /// Queue a message for the next tick.
    pub fn send(&mut self, m: Message) {
        if self.messages.len() < MAX_QUEUE {
            self.messages.push(m);
        } else {
            inc(&mut self.counters.messages_dropped);
        }
    }

    /// Ask the app to show a text; returns the request id, or `None` when the request was dropped
    /// (queue full, or the id counter saturated; counted in `texts_dropped`).
    pub fn show_text(&mut self, text: i32, blocking: bool) -> Option<u64> {
        let id = self.next_text_id;
        if self.texts.len() >= MAX_QUEUE || id == u64::MAX {
            inc(&mut self.counters.texts_dropped);
            return None;
        }
        self.next_text_id = id.saturating_add(1);
        self.texts.push(TextRequest { id, text, blocking });
        Some(id)
    }

    /// Element by handle; out-of-table handles are [`Element::Unmodelled`], negative ones `None`.
    #[must_use]
    pub fn element(&self, handle: i32) -> Option<Element> {
        if handle < 0 {
            return None;
        }
        Some(
            self.program
                .elements
                .get(handle as usize)
                .copied()
                .unwrap_or(Element::Unmodelled(handle as u32)),
        )
    }

    /// Whether the element `handle` is active: not deactivated by native 113 (entities keep
    /// their own flag; this answers for the non-actor elements).
    #[must_use]
    pub fn element_active(&self, handle: i32) -> bool {
        !self.inactive_elements.contains(&handle)
    }

    /// The scroll `handle`, if the table holds one: its position.
    #[must_use]
    pub fn scroll(&self, handle: i32) -> Option<(i32, i32)> {
        match self.element(handle)? {
            Element::Scroll { x, y } => Some((x, y)),
            _ => None,
        }
    }

    /// The first class bound to the scroll `handle` (the class whose `IsTaken` a reading
    /// calls), if any.
    #[must_use]
    pub fn scroll_class(&self, handle: i32) -> Option<u32> {
        self.scroll(handle)?;
        self.program
            .classes
            .iter()
            .position(|c| c.element == Some(handle as u32))
            .map(|i| i as u32)
    }

    /// The pick-up item `handle`, if the table holds one: `(x, y, kind, stack)`.
    #[must_use]
    pub fn item(&self, handle: i32) -> Option<(i32, i32, ItemKind, u16)> {
        match self.element(handle)? {
            Element::Item { x, y, kind, stack } => Some((x, y, kind, stack)),
            _ => None,
        }
    }

    /// Every pick-up item of the table with its state (`observe`, `debug.vm`, the renderer).
    #[must_use]
    pub fn items(&self) -> Vec<ItemObservation> {
        self.program
            .elements
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match *e {
                Element::Item { x, y, kind, stack } => {
                    let handle = i as i32;
                    let taken = self.taken_items.contains(&handle);
                    Some(ItemObservation {
                        element: handle,
                        kind,
                        stack,
                        x,
                        y,
                        active: self.element_active(handle) && !taken,
                        taken,
                    })
                }
                _ => None,
            })
            .collect()
    }

    /// Pending text indices in order (see [`VmState::pending_text_requests`] for the blocking
    /// flag of each: a native 202 text is shown without pausing anything, a native 203 page holds
    /// its sequence until it is dismissed).
    #[must_use]
    pub fn pending_texts(&self) -> Vec<i32> {
        self.texts.iter().map(|t| t.text).collect()
    }

    /// Pending text requests in order, first is shown; `blocking` tells a native 203 page (a
    /// sequence waits for its dismissal) from a native 202 text (nothing waits).
    #[must_use]
    pub fn pending_text_requests(&self) -> &[TextRequest] {
        &self.texts
    }
}

/// Saturating increment of a diagnostic counter.
fn inc(c: &mut u64) {
    *c = c.saturating_add(1);
}

/// Saturating increment of a per-id counter.
fn inc_id(map: &mut BTreeMap<u32, u64>, id: u32) {
    let c = map.entry(id).or_insert(0);
    *c = c.saturating_add(1);
}

/// Charge `units` of work; `false` (and a zero budget) when it does not fit.
fn charge(vm: &mut VmState, units: u64) -> bool {
    charge_budget(&mut vm.budget, units)
}

/// Charge `units` of work to a budget; `false` (and a zero budget) when it does not fit. Every
/// charge is made before the work (or the allocation) it pays for.
pub(crate) fn charge_budget(budget: &mut u64, units: u64) -> bool {
    if *budget < units {
        *budget = 0;
        false
    } else {
        *budget -= units;
        true
    }
}

/// The one teardown path of a callback: frames, both stacks and a sequence still being collected
/// are dropped, so the VM is quiescent between callbacks whatever the program did (returned with
/// surplus values, was aborted by the budget, faulted or trapped). `Program::validate` rejects
/// programs whose stacks are not balanced; this holds even for one that got past it.
fn teardown(vm: &mut VmState) {
    vm.frames.clear();
    vm.arg_stack.clear();
    vm.param_stack.clear();
    vm.collecting = None;
}

fn encode_message(e: &mut Encoder, m: &Message) {
    e.i32(m.target).i32(m.id).i32(m.arg).i32(m.arg2);
}

fn encode_element(e: &mut Encoder, el: &SeqElement) {
    match el {
        SeqElement::Text(t) => e.u8(1).i32(*t),
        SeqElement::Wait(n) => e.u8(2).u32(*n),
        SeqElement::Camera(l) => e.u8(3).i32(*l),
        SeqElement::Message(m) => {
            e.u8(4);
            encode_message(e, m);
            e
        }
        SeqElement::Walk { entity, x, y } => e.u8(5).u32(*entity).i32(*x).i32(*y),
        SeqElement::Teleport { entity, to } => {
            e.u8(6).u32(*entity);
            match to {
                Some((x, y)) => e.u8(1).i32(*x).i32(*y),
                None => e.u8(0),
            }
        }
        SeqElement::Stub { id } => e.u8(7).u32(*id),
        SeqElement::Animation { id, actor, anim } => e.u8(8).u32(*id).i32(*actor).i32(*anim),
        SeqElement::Barrier => e.u8(9),
    };
}

/// Script state for `observe`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptObservation {
    /// Objectives in the order they were added.
    pub objectives: Vec<Objective>,
    /// Pending text indices, first is shown.
    pub texts: Vec<i32>,
    /// The same requests with their blocking flag (native 203 pages block a sequence, native 202
    /// texts do not).
    #[serde(default)]
    pub text_requests: Vec<TextRequest>,
    /// `CheckVictoryCondition` returned 1.
    pub mission_won: bool,
    /// `CheckVictoryCondition` returned 2.
    #[serde(default)]
    pub mission_lost: bool,
    /// Unknown native calls by id.
    pub unknown_natives: BTreeMap<u32, u64>,
    /// A sequence is running.
    pub sequence_active: bool,
    /// Last camera target set by the script.
    pub camera_target: Option<(i32, i32)>,
    /// Debriefing variant chosen.
    pub debriefing: Option<i32>,
    /// An unknown native stopped a callback (strict mode).
    pub faulted: bool,
    /// Unknown natives are recorded no-ops (`MissionSpec::lenient_natives`).
    pub lenient: bool,
    /// Unknown native calls recorded in lenient mode.
    pub unknown_calls: usize,
    /// Element handle of every entity, by entity index (`NONE_HANDLE` for entities the script
    /// cannot address): what native 3 returns for the actors, for tests that aim at the actor a
    /// script polls.
    #[serde(default)]
    pub actor_elements: Vec<i32>,
    /// A script-visible outcome depended on a hypothesis or a stub value: `mission_won` /
    /// `mission_lost` are not authoritative (ADR-0008, "Hypotheses and taint").
    #[serde(default)]
    pub tainted: bool,
    /// The assumptions recorded so far, in canonical order.
    #[serde(default)]
    pub assumptions: Vec<Assumption>,
    /// The pick-up items of the element table ([`Element::Item`]) with their state.
    #[serde(default)]
    pub items: Vec<ItemObservation>,
}

/// One pick-up item as `observe` reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemObservation {
    /// Element handle (what native 3 returns for it).
    pub element: i32,
    /// Kind.
    pub kind: ItemKind,
    /// Stack size.
    pub stack: u16,
    /// Map x.
    pub x: i32,
    /// Map y.
    pub y: i32,
    /// Shown on the map and pickable (not deactivated by native 113, not taken).
    pub active: bool,
    /// A player character took it (native 235 reads 1).
    pub taken: bool,
}

/// Outcome of one callback invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallOutcome {
    /// Ran to completion with this return value.
    Returned(i32),
    /// Aborted by a fault or a trap: the callback ran and failed deterministically (it would
    /// fail the same way again); the frames were discarded.
    Aborted,
    /// Cut short by the tick's work budget; the frames were discarded and the callback did not
    /// run to its end.
    Exhausted,
}

/// Names of the engine callbacks the core invokes (`docs/formats/scb.md`, "Calling convention").
pub mod callbacks {
    /// Every class, at load.
    pub const INITIALIZE: &str = "Initialize";
    /// Level class, after every `Initialize`.
    pub const POST_INITIALIZE: &str = "PostInitialize";
    /// Every tick, `(time)`.
    pub const HOURGLASS: &str = "Hourglass";
    /// Level class, every tick; 1 = won, 2 = lost.
    pub const CHECK_VICTORY: &str = "CheckVictoryCondition";
    /// `(msg, arg, arg2)`.
    pub const PROCESS_MESSAGE: &str = "ProcessMessage";
    /// `(actor)`.
    pub const ENTER_ZONE: &str = "EnterZone";
    /// `(actor)`.
    pub const EXIT_ZONE: &str = "ExitZone";
    /// `(actor)`: scroll picked up.
    pub const IS_TAKEN: &str = "IsTaken";
    /// `(actor)`: actor reached a named rail point.
    pub const REACH_POINT: &str = "ReachPoint";
    /// `(a, b)`: actor changed action state.
    pub const ACTION_CHANGE: &str = "ActionChange";
}

impl World {
    /// Attach a translated script to a freshly built mission world and run its load-time
    /// callbacks: `Initialize` on every class (level first, then elements in table order),
    /// `PostInitialize` on the level, then the first sequence elements. `lenient` selects the
    /// unknown-native policy (see `natives.rs`); `starting_money` seeds natives 236 / 237
    /// before `Initialize` runs (a script that sets it, e.g. H10's 100000, wins; nothing
    /// overwrites it afterwards); `assumptions` are the app's load-time assumptions
    /// (`Assumption::LenientAssets`).
    pub fn attach_script(
        &mut self,
        program: Program,
        paths: Vec<Option<u32>>,
        lenient: bool,
        starting_money: i32,
        assumptions: &BTreeSet<Assumption>,
    ) -> Result<(), String> {
        program.validate()?;
        let mut vm = VmState::new(program, paths, self.seed, lenient);
        vm.money = starting_money;
        vm.assumptions.clone_from(assumptions);
        vm.validate(self.programs.len(), self.entities.len())?;
        self.vm = Some(vm);
        self.vm_grant_budget(WORK_BUDGET_AT_LOAD);
        let n = self.vm.as_ref().map_or(0, |v| v.program.classes.len());
        for class in 0..n as u32 {
            self.vm_callback(class, callbacks::INITIALIZE, &[]);
        }
        self.vm_callback(0, callbacks::POST_INITIALIZE, &[]);
        self.vm_advance_sequences();
        Ok(())
    }

    /// The app dismissed the text at the front of the queue (a briefing page, a popup). The
    /// sequence waiting for it continues at once (up to its next blocking element), so a
    /// multi-page presentation can dismiss page after page without ticking the world. The
    /// continuation draws from the work the current tick (or the load-time run) left: no new
    /// budget is granted between ticks, so after an exhausted tick the page is removed but the
    /// sequence continues at the next tick. Returns whether a text was pending.
    pub fn vm_dismiss_text(&mut self) -> bool {
        let Some(vm) = self.vm.as_mut() else {
            return false;
        };
        if vm.texts.is_empty() {
            return false;
        }
        vm.texts.remove(0);
        self.vm_advance_sequences();
        true
    }

    /// Script state for `observe`.
    #[must_use]
    pub fn script_observation(&self) -> Option<ScriptObservation> {
        let vm = self.vm.as_ref()?;
        Some(ScriptObservation {
            objectives: vm.objectives.clone(),
            texts: vm.pending_texts(),
            text_requests: vm.texts.clone(),
            mission_won: vm.mission_won,
            mission_lost: vm.mission_lost,
            unknown_natives: vm.counters.unknown_natives.clone(),
            sequence_active: !vm.sequences.is_empty(),
            camera_target: vm.camera_target,
            debriefing: vm.debriefing,
            faulted: vm.faulted(),
            lenient: vm.lenient,
            unknown_calls: vm.unknown_calls.len(),
            actor_elements: (0..self.entities.len() as u32)
                .map(|i| vm.program.element_of_entity(i))
                .collect(),
            tainted: vm.tainted(),
            assumptions: vm.assumptions.iter().copied().collect(),
            items: vm.items(),
        })
    }

    /// Record that a script-visible outcome depends on a hypothesis (module documentation,
    /// "Hypotheses and taint"); the app records `Assumption::CampaignGraph` when its campaign
    /// graph picks the next mission. A world without a script has no outcome to taint.
    pub fn record_assumption(&mut self, assumption: Assumption) {
        if let Some(vm) = self.vm.as_mut() {
            vm.assumptions.insert(assumption);
        }
    }

    /// Hook: a scroll bound to a class was read by `actor` (element handle); triggered by
    /// [`World::vm_read_scroll`] at the end of the reading pause of a pick-up order.
    pub fn vm_is_taken(&mut self, class: u32, actor: i32) -> Option<i32> {
        self.vm_event(class, callbacks::IS_TAKEN, &[actor])
    }

    /// Hook: `actor` reached the named rail point `(rail, point)`. Not triggered by the engine yet.
    pub fn vm_reach_point(&mut self, rail: u32, point: u32, actor: i32) -> Option<i32> {
        let class = self
            .vm
            .as_ref()?
            .program
            .classes
            .iter()
            .position(|c| c.rail_point == Some((rail, point)))?;
        self.vm_event(class as u32, callbacks::REACH_POINT, &[actor])
    }

    /// Hook: an object bound to `class` was activated (`ActivatedByArrow`, `ActivatedBySword`,
    /// ...; `handler` is the full callback name) by `actor`. Not triggered by the engine yet.
    pub fn vm_activated(&mut self, class: u32, handler: &str, actor: i32) -> Option<i32> {
        if !handler.starts_with("ActivatedBy") {
            return None;
        }
        self.vm_event(class, handler, &[actor])
    }

    /// Queue an action change of the actor of `class`: `(previous, new)` sprite action ids
    /// (`crate::ai::action_id`; the parameter order is a hypothesis, `docs/formats/scb.md`).
    /// `World::simulate` queues every change it detects, then calls
    /// [`World::vm_deliver_action_changes`]; a full queue faults the script
    /// ([`Fault::ActionQueueOverflow`]): the change is not delivered and the fault is sticky.
    pub(crate) fn vm_queue_action_change(&mut self, class: u32, previous: i32, new: i32) {
        if let Some(vm) = self.vm.as_mut() {
            if vm.pending_action_changes.len() < MAX_QUEUE {
                vm.pending_action_changes.push(ActionChange {
                    class,
                    previous,
                    new,
                });
            } else {
                vm.set_fault(Fault::ActionQueueOverflow);
            }
        }
    }

    /// Deliver the queued action changes in order within what the tick's budget has left, each
    /// exactly once: a change whose class has no `ActionChange` is dropped as undeliverable,
    /// one whose handler returned is removed, one whose handler trapped or faulted is rolled
    /// back and removed (it would fail the same way again), and one the budget cut short is rolled
    /// back ([`Transaction`]) and stays at the front for the next tick (`vm_tick` delivers the
    /// leftovers before `Hourglass`), where the handler runs again from the start over the
    /// state it saw the first time. A knock-out or melee action reaching a handler records the
    /// corresponding assumption (the stealth layer recorded its own sources when the state
    /// changed, handler or not); every delivery records the parameter-order hypothesis.
    pub(crate) fn vm_deliver_action_changes(&mut self) {
        let mut done = 0usize;
        loop {
            let Some(vm) = self.vm.as_ref() else {
                return;
            };
            let Some(change) = vm.pending_action_changes.get(done).copied() else {
                break;
            };
            let handler = vm
                .program
                .classes
                .get(change.class as usize)
                .and_then(|c| c.function(callbacks::ACTION_CHANGE));
            let Some(function) = handler else {
                done += 1;
                continue;
            };
            if self.vm_out_of_work() {
                break;
            }
            // The actor bound to the class: a dead actor's fall is the melee's, not the
            // knock-out's.
            let dead = self
                .vm
                .as_ref()
                .and_then(|vm| {
                    let handle = vm.program.classes.get(change.class as usize)?.element?;
                    match vm.program.elements.get(handle as usize)? {
                        Element::Actor(i) => self.entities.get(*i as usize).map(|e| !e.alive),
                        _ => None,
                    }
                })
                .unwrap_or(false);
            // Open the transaction (the capture is charged; when it does not fit the delivery
            // waits for the next tick like an exhausted handler).
            let captured = self.vm.as_mut().and_then(VmState::capture);
            let Some(mut txn) = captured else {
                if let Some(vm) = self.vm.as_mut() {
                    inc(&mut vm.counters.budget_aborts);
                }
                break;
            };
            txn.selected = self.selected;
            txn.camera = self.camera;
            if let Some(vm) = self.vm.as_mut() {
                vm.transaction = Some(txn);
                let ids = [change.previous, change.new];
                let fallen = ids.iter().any(|&a| KNOCK_OUT_ACTIONS.contains(&(a as u32)));
                if fallen && !dead {
                    vm.assume(Assumption::KnockOut);
                }
                if (fallen && dead) || ids.iter().any(|&a| COMBAT_ACTIONS.contains(&(a as u32))) {
                    vm.assume(Assumption::CombatActions);
                }
                vm.assume(Assumption::ActionChangeOrder);
            }
            match self.vm_invoke(change.class, function, &[change.previous, change.new]) {
                CallOutcome::Exhausted => {
                    self.vm_roll_back();
                    break;
                }
                CallOutcome::Aborted => {
                    // A deterministic failure (a trap, a fault such as the frame limit):
                    // the handler's partial effects are put back and the change is
                    // consumed, since it would fail the same way again.
                    self.vm_roll_back();
                    done += 1;
                }
                CallOutcome::Returned(_) => {
                    if let Some(vm) = self.vm.as_mut() {
                        vm.transaction = None;
                    }
                    done += 1;
                }
            }
        }
        if let Some(vm) = self.vm.as_mut() {
            vm.pending_action_changes.drain(..done);
        }
    }

    /// Put the open transaction back: the VM's mutable state, the entities the callback
    /// touched, the selection and the camera (counted in `transactions_rolled_back`).
    fn vm_roll_back(&mut self) {
        let Some(vm) = self.vm.as_mut() else {
            return;
        };
        let Some(t) = vm.transaction.take() else {
            return;
        };
        vm.roll_back(&t);
        inc(&mut vm.counters.transactions_rolled_back);
        for (i, e) in t.entities {
            if let Some(slot) = self.entities.get_mut(i) {
                *slot = e;
            }
        }
        self.selected = t.selected;
        self.camera = t.camera;
    }

    /// A native is about to mutate entity `i`: when a transaction is open, keep a copy of the
    /// entity as it is now so the change can be rolled back. Every native that writes an entity
    /// calls this first (`natives.rs`, `vm_walk`, `vm_teleport`).
    pub(crate) fn vm_touch_entity(&mut self, i: usize) {
        let Some(e) = self.entities.get(i) else {
            return;
        };
        if let Some(t) = self.vm.as_mut().and_then(|vm| vm.transaction.as_mut())
            && !t.entities.contains_key(&i)
        {
            t.entities.insert(i, e.clone());
        }
    }

    /// An event hook: runs the callback within what the current tick's budget has left (an
    /// exhausted budget aborts it at once; the hook fires again when its cause persists, e.g. a
    /// scroll approach whose presence was not recorded). `None` when the class has no such
    /// callback or the callback did not return.
    fn vm_event(&mut self, class: u32, name: &str, params: &[i32]) -> Option<i32> {
        match self.vm_callback(class, name, params) {
            Some(CallOutcome::Returned(v)) => Some(v),
            _ => None,
        }
    }

    /// Grant a work budget. Called from exactly two places: the start of [`World::vm_tick`]
    /// ([`WORK_BUDGET_PER_TICK`]) and once by `attach_script` ([`WORK_BUDGET_AT_LOAD`]). No
    /// other entry point replenishes the budget.
    fn vm_grant_budget(&mut self, units: u64) {
        if let Some(vm) = self.vm.as_mut() {
            vm.budget = units;
        }
    }

    /// Whether the tick's work budget is spent; counts a budget abort when it is.
    fn vm_out_of_work(&mut self) -> bool {
        match self.vm.as_mut() {
            Some(vm) if vm.budget == 0 => {
                inc(&mut vm.counters.budget_aborts);
                true
            }
            _ => false,
        }
    }

    /// One tick of the script scheduler (called by `step` before the entities move): deliver
    /// the messages queued before this tick, the action changes left over from the previous
    /// tick, `Hourglass(tick)` on every class, zone transitions of the player characters, the
    /// active sequences, then `CheckVictoryCondition` (a scroll's `IsTaken` fires from
    /// `World::resolve_pickups`, after this, at the end of the reading pause). The tick's work budget is granted here and nowhere else; every
    /// phase stops when it is spent; undelivered messages stay queued (ahead of those sent this
    /// tick) for the next tick.
    pub(crate) fn vm_tick(&mut self) {
        if self.vm.is_none() {
            return;
        }
        self.vm_grant_budget(WORK_BUDGET_PER_TICK);
        let pending = self
            .vm
            .as_mut()
            .map(|vm| std::mem::take(&mut vm.messages))
            .unwrap_or_default();
        let mut pending = pending.into_iter();
        for m in pending.by_ref() {
            if self.vm_out_of_work() {
                if let Some(vm) = self.vm.as_mut() {
                    let mut rest: Vec<Message> = std::iter::once(m).chain(pending).collect();
                    rest.append(&mut vm.messages);
                    if rest.len() > MAX_QUEUE {
                        let dropped = rest.len() - MAX_QUEUE;
                        rest.truncate(MAX_QUEUE);
                        vm.counters.messages_dropped =
                            vm.counters.messages_dropped.saturating_add(dropped as u64);
                    }
                    vm.messages = rest;
                }
                return;
            }
            self.vm_deliver(m);
        }
        // Action changes a previous tick could not deliver (its budget ran out) come first.
        self.vm_deliver_action_changes();
        let time = self.tick as i32;
        let n = self.vm.as_ref().map_or(0, |v| v.program.classes.len());
        for class in 0..n as u32 {
            if self.vm_out_of_work() {
                return;
            }
            self.vm_callback(class, callbacks::HOURGLASS, &[time]);
        }
        if self.vm_out_of_work() {
            return;
        }
        self.vm_zones();
        if self.vm_out_of_work() {
            return;
        }
        self.vm_advance_sequences();
        if self.vm_out_of_work() {
            return;
        }
        // `docs/formats/scb.md`, "Calling convention": 0 running, 1 won, 2 lost (a debriefing is
        // usually selected with native 28 first). Both outcomes are sticky.
        match self.vm_callback(0, callbacks::CHECK_VICTORY, &[]) {
            Some(CallOutcome::Returned(1)) => {
                if let Some(vm) = self.vm.as_mut() {
                    vm.mission_won = true;
                }
            }
            Some(CallOutcome::Returned(2)) => {
                if let Some(vm) = self.vm.as_mut() {
                    vm.mission_lost = true;
                }
            }
            _ => {}
        }
    }

    /// Deliver one message: the class bound to the target element, else the level class
    /// (`docs/formats/scb.md`, native 111: messages to the player character reach the level).
    fn vm_deliver(&mut self, m: Message) {
        let Some(vm) = self.vm.as_ref() else { return };
        let class = vm
            .program
            .classes
            .iter()
            .position(|c| m.target >= 0 && c.element == Some(m.target as u32))
            .unwrap_or(0) as u32;
        if let Some(vm) = self.vm.as_mut() {
            inc(&mut vm.counters.messages_delivered);
        }
        self.vm_callback(class, callbacks::PROCESS_MESSAGE, &[m.id, m.arg, m.arg2]);
    }

    /// `EnterZone` / `ExitZone` for every player character crossing a zone class's polygon.
    /// Presence starts empty, so a character standing inside a zone at load enters it on the
    /// first tick (hypothesis). Every polygon test is charged (one unit per edge); when the
    /// budget runs out the remaining pairs are tested next tick, and a transition whose callback
    /// cannot start keeps its old presence so it fires next tick.
    fn vm_zones(&mut self) {
        let Some(vm) = self.vm.as_ref() else { return };
        let mut budget = vm.budget;
        let mut exhausted = false;
        let mut events: Vec<(u32, u32, bool)> = Vec::new();
        'scan: for (ci, c) in vm.program.classes.iter().enumerate() {
            let Some(z) = c.zone else { continue };
            let Some(Location::Polygon(poly)) = vm.program.locations.get(z as usize) else {
                continue;
            };
            for (ei, e) in self.entities.iter().enumerate() {
                // One unit per entity looked at, plus one per edge for every character tested.
                let player = e.kind == EntityKind::Player && e.alive && e.active;
                let cost = if player { 1 + poly.len() as u64 } else { 1 };
                if budget < cost {
                    budget = 0;
                    exhausted = true;
                    break 'scan;
                }
                budget -= cost;
                if !player {
                    continue;
                }
                let inside = poly.len() >= 3 && point_in_polygon(e.x.round(), e.y.round(), poly);
                let was = vm.zone_presence.contains(&(ci as u32, ei as u32));
                if inside != was {
                    events.push((ci as u32, ei as u32, inside));
                }
            }
        }
        if let Some(vm) = self.vm.as_mut() {
            vm.budget = budget;
            if exhausted {
                inc(&mut vm.counters.budget_aborts);
            }
        }
        let at_load = self.tick == 0;
        for (class, entity, inside) in events {
            if self.vm_out_of_work() {
                return;
            }
            let actor = self
                .vm
                .as_ref()
                .map_or(NONE_HANDLE, |vm| vm.program.element_of_entity(entity));
            if let Some(vm) = self.vm.as_mut() {
                if inside {
                    vm.zone_presence.insert((class, entity));
                    if at_load {
                        // Presence starts empty: a character inside at load enters on the
                        // first scan (hypothesis).
                        vm.assume(Assumption::ZoneAtLoad);
                    }
                } else {
                    vm.zone_presence.remove(&(class, entity));
                }
            }
            let name = if inside {
                callbacks::ENTER_ZONE
            } else {
                callbacks::EXIT_ZONE
            };
            self.vm_callback(class, name, &[actor]);
        }
    }

    /// A player character's reading of the scroll `handle` (`World::resolve_pickups`, at the
    /// end of the pause of a pick-up order on the scroll): `IsTaken(actor)` on the first class
    /// bound to the scroll. A handler that returns non-zero takes the scroll (it becomes
    /// inactive; the take-on-non-zero rule is a hypothesis, [`Assumption::ScrollPickup`]); one
    /// that returns zero leaves it, and a new order reads it again. A scroll no class is bound
    /// to is read to no effect. `None` when the tick's budget was spent before the handler
    /// could start (the caller retries next tick); `Some(taken)` otherwise (a handler the
    /// budget cut short or that trapped is consumed, like a queued `ActionChange`).
    pub(crate) fn vm_read_scroll(&mut self, handle: i32, entity: usize) -> Option<bool> {
        let vm = self.vm.as_ref()?;
        let Some(class) = vm.scroll_class(handle) else {
            return Some(false);
        };
        if self.vm_out_of_work() {
            return None;
        }
        let actor = self.vm.as_ref().map_or(NONE_HANDLE, |vm| {
            vm.program.element_of_entity(entity as u32)
        });
        let taken = matches!(self.vm_is_taken(class, actor), Some(v) if v != 0);
        if taken && let Some(vm) = self.vm.as_mut() {
            // What makes a scroll vanish after its reading is a hypothesis.
            vm.assume(Assumption::ScrollPickup);
            vm.inactive_elements.insert(handle);
        }
        Some(taken)
    }

    /// Advance every sequence this tick: each runs until its own wait (ticks, a text page or a
    /// barrier) or its end, independently of the others (the original's sequence manager keeps
    /// one sequence per element; running them one after another would queue a scroll's popup
    /// behind unrelated timed sequences such as the archery-training loop). Finished sequences
    /// are removed. Every element executed costs one work unit; when the budget is spent the
    /// remaining sequences wait for the next tick.
    pub(crate) fn vm_advance_sequences(&mut self) {
        let mut i = 0;
        while i < self.vm.as_ref().map_or(0, |vm| vm.sequences.len()) {
            if self.vm_out_of_work() {
                return;
            }
            if self.vm_advance_sequence(i) {
                if let Some(vm) = self.vm.as_mut() {
                    vm.sequences.remove(i);
                }
            } else {
                i += 1;
            }
        }
    }

    /// Whether a walk token completed for a reason other than the arrival (the failure cases
    /// of [`SeqToken::Walk`]: hypothesis).
    fn walk_completed_without_arrival(&self, token: SeqToken) -> bool {
        match token {
            SeqToken::Walk { entity, x, y } => {
                let Some(e) = self.entities.get(entity as usize) else {
                    return true;
                };
                !e.alive || !e.active || (e.x.round(), e.y.round()) != (x, y)
            }
            SeqToken::Animation { .. } => false,
        }
    }

    /// Whether a completion token is done (see [`SeqToken`]).
    fn seq_token_done(&self, token: SeqToken) -> bool {
        match token {
            SeqToken::Walk { entity, x, y } => {
                let Some(e) = self.entities.get(entity as usize) else {
                    return true;
                };
                !e.alive || !e.active || e.target != Some((Fixed::from_int(x), Fixed::from_int(y)))
            }
            SeqToken::Animation { .. } => true,
        }
    }

    /// Run sequence `i` until it blocks; returns true when it has finished.
    fn vm_advance_sequence(&mut self, i: usize) -> bool {
        loop {
            // What the sequence waits for, checked against the world without holding it mutably.
            let Some(vm) = self.vm.as_ref() else {
                return true;
            };
            let Some(seq) = vm.sequences.get(i) else {
                return true;
            };
            match seq.wait {
                // `Wait(n)` holds the sequence for exactly n ticks: the tick that brings the
                // count to zero runs the next element.
                SeqWait::Ticks(n) if n > 1 => {
                    if let Some(seq) = self.vm.as_mut().and_then(|vm| vm.sequences.get_mut(i)) {
                        seq.wait = SeqWait::Ticks(n - 1);
                    }
                    return false;
                }
                SeqWait::Text(id) => {
                    if vm.texts.iter().any(|t| t.id == id) {
                        return false;
                    }
                }
                SeqWait::Barrier => {
                    if !seq.tokens.iter().all(|&t| self.seq_token_done(t)) {
                        return false;
                    }
                }
                SeqWait::Ticks(_) | SeqWait::None => {}
            }
            // A barrier released by a walk that did not arrive rests on the completion
            // hypothesis (`SeqToken::Walk`).
            let walk_without_arrival = seq.wait == SeqWait::Barrier
                && seq
                    .tokens
                    .iter()
                    .any(|&t| self.walk_completed_without_arrival(t));
            let el = {
                let Some(vm) = self.vm.as_mut() else {
                    return true;
                };
                if !charge(vm, 1) {
                    return false;
                }
                if walk_without_arrival {
                    vm.assume(Assumption::WalkCompletion);
                }
                let Some(seq) = vm.sequences.get_mut(i) else {
                    return true;
                };
                if seq.wait == SeqWait::Barrier {
                    seq.tokens.clear();
                }
                seq.wait = SeqWait::None;
                let Some(el) = seq.elements.get(seq.next as usize).cloned() else {
                    return true;
                };
                seq.next += 1;
                el
            };
            match el {
                SeqElement::Text(t) => {
                    if let Some(vm) = self.vm.as_mut() {
                        let id = vm.show_text(t, true);
                        if let (Some(id), Some(seq)) = (id, vm.sequences.get_mut(i)) {
                            seq.wait = SeqWait::Text(id);
                            return false;
                        }
                    }
                }
                SeqElement::Wait(n) => {
                    if n > 0 {
                        // The wait's length is the 25-versus-60 reading of native 56.
                        if let Some(vm) = self.vm.as_mut() {
                            vm.assumptions.insert(Assumption::TickRate);
                            if let Some(seq) = vm.sequences.get_mut(i) {
                                seq.wait = SeqWait::Ticks(n);
                            }
                        }
                        return false;
                    }
                }
                SeqElement::Barrier => {
                    if let Some(seq) = self.vm.as_mut().and_then(|vm| vm.sequences.get_mut(i)) {
                        seq.wait = SeqWait::Barrier;
                    }
                }
                SeqElement::Camera(loc) => self.vm_camera(loc),
                SeqElement::Message(m) => {
                    if let Some(vm) = self.vm.as_mut() {
                        vm.send(m);
                    }
                }
                SeqElement::Walk { entity, x, y } => {
                    self.vm_walk(entity, x, y);
                    self.vm_push_token(i, SeqToken::Walk { entity, x, y });
                }
                SeqElement::Teleport { entity, to } => self.vm_teleport(entity, to),
                SeqElement::Animation { id, .. } => {
                    if let Some(vm) = self.vm.as_mut() {
                        inc_id(&mut vm.counters.stub_natives, id);
                    }
                    self.vm_push_token(i, SeqToken::Animation { id });
                }
                SeqElement::Stub { id } => {
                    if let Some(vm) = self.vm.as_mut() {
                        inc_id(&mut vm.counters.stub_natives, id);
                    }
                }
            }
        }
    }

    /// Record a completion token on sequence `i` (bounded).
    fn vm_push_token(&mut self, i: usize, token: SeqToken) {
        if let Some(seq) = self.vm.as_mut().and_then(|vm| vm.sequences.get_mut(i))
            && seq.tokens.len() < MAX_QUEUE
        {
            seq.tokens.push(token);
        }
    }

    /// Invoke `name` on `class` if the class defines it.
    pub(crate) fn vm_callback(
        &mut self,
        class: u32,
        name: &str,
        params: &[i32],
    ) -> Option<CallOutcome> {
        let function = self
            .vm
            .as_ref()?
            .program
            .classes
            .get(class as usize)?
            .function(name)?;
        Some(self.vm_invoke(class, function, params))
    }

    /// Run one function to completion (nested script calls included) within the budget: one
    /// unit per instruction plus one per argument transferred by a call or a native. Every exit
    /// (a return, a budget abort, a fault, a trap) passes through [`teardown`], so the VM is
    /// quiescent afterwards whatever the program did.
    pub(crate) fn vm_invoke(&mut self, class: u32, function: u32, params: &[i32]) -> CallOutcome {
        let outcome = self.vm_run(class, function, params);
        if let Some(vm) = self.vm.as_mut() {
            teardown(vm);
        }
        outcome
    }

    /// [`World::vm_invoke`] without the teardown (its caller always tears down).
    fn vm_run(&mut self, class: u32, function: u32, params: &[i32]) -> CallOutcome {
        let Some(vm) = self.vm.as_mut() else {
            return CallOutcome::Aborted;
        };
        inc(&mut vm.counters.callbacks);
        if !vm.frames.is_empty() {
            // Callbacks never nest (natives queue events instead of invoking scripts).
            inc(&mut vm.counters.faults);
        }
        teardown(vm);
        if !push_frame(vm, class, function, params.to_vec()) {
            return CallOutcome::Aborted;
        }
        loop {
            let Some(vm) = self.vm.as_mut() else {
                return CallOutcome::Aborted;
            };
            let Some(frame) = vm.frames.last() else {
                return CallOutcome::Aborted;
            };
            let (ci, pc) = (frame.class as usize, frame.pc as usize);
            let ins = vm
                .program
                .classes
                .get(ci)
                .and_then(|c| c.code.get(pc))
                .copied();
            // One unit per instruction, plus the arguments a call or native transfers.
            let cost = 1 + match ins {
                Some(Instr::Call { argc, .. } | Instr::Native { argc, .. }) => u64::from(argc),
                _ => 0,
            };
            if !charge(vm, cost) {
                inc(&mut vm.counters.budget_aborts);
                return CallOutcome::Exhausted;
            }
            inc(&mut vm.counters.instructions);
            let Some(ins) = ins else {
                // Ran off the end of the code: treat as a return.
                if let Some(v) = pop_frame(vm) {
                    return CallOutcome::Returned(v);
                }
                continue;
            };
            match ins {
                Instr::Nop | Instr::Enter { .. } => advance(vm),
                Instr::Return => {
                    if let Some(v) = pop_frame(vm) {
                        return CallOutcome::Returned(v);
                    }
                }
                Instr::LeaveUnresolved => {
                    vm.assume(Assumption::UnresolvedJump);
                    if let Some(v) = pop_frame(vm) {
                        return CallOutcome::Returned(v);
                    }
                }
                Instr::SetResult { src } => {
                    let v = read(vm, src);
                    if let Some(f) = vm.frames.last_mut() {
                        f.result = v;
                    }
                    advance(vm);
                }
                Instr::LoadParam { dst, index } => {
                    let v = vm
                        .frames
                        .last()
                        .and_then(|f| f.params.get(index as usize).copied());
                    let v = v.unwrap_or_else(|| {
                        inc(&mut vm.counters.faults);
                        0
                    });
                    // The `Hourglass` time parameter is the world tick (hypothesis: the
                    // scripts compare differences of it, in their own tick unit).
                    if vm.frames.len() == 1 && frame_is(vm, callbacks::HOURGLASS) {
                        vm.assumptions.insert(Assumption::TickRate);
                    }
                    write(vm, dst, v);
                    advance(vm);
                }
                Instr::PushParam { src } => {
                    let v = read(vm, src);
                    if vm.param_stack.len() < MAX_STACK {
                        vm.param_stack.push(v);
                    }
                    advance(vm);
                }
                Instr::Call { function, argc, .. } => {
                    let params = pop_n(&mut vm.param_stack, argc as usize, &mut vm.counters);
                    // The caller stays on the `Call`; `pop_frame` writes the callee's result
                    // to the call's destination and steps past it. A frame that cannot be
                    // pushed (the frame limit: `Fault::CallStackOverflow`) aborts the
                    // callback here: the destination is never left untouched behind a
                    // value the caller wrote before the call (Codex review 10, finding 3).
                    let class = vm.frames.last().map_or(0, |f| f.class);
                    if !push_frame(vm, class, function, params) {
                        return CallOutcome::Aborted;
                    }
                }
                Instr::PushArg { src } => {
                    let v = read(vm, src);
                    if vm.arg_stack.len() < MAX_STACK {
                        vm.arg_stack.push(v);
                    }
                    advance(vm);
                }
                Instr::Native { id, argc, dst } => {
                    let args = pop_n(&mut vm.arg_stack, argc as usize, &mut vm.counters);
                    advance(vm);
                    let Some(r) = self.native_call(id, &args) else {
                        // Unknown native in strict mode: a deterministic trap ends the
                        // callback here (the teardown discards its frames and stacks, the
                        // script is marked faulted).
                        if let Some(vm) = self.vm.as_mut() {
                            inc(&mut vm.counters.traps);
                        }
                        return CallOutcome::Aborted;
                    };
                    if let (Some(dst), Some(vm)) = (dst, self.vm.as_mut()) {
                        // A stub's value (0 or a policy value) consumed by the script taints
                        // the outcome (a presentation-only stub included).
                        if crate::natives::native_status(id) == crate::natives::NativeStatus::Stub {
                            vm.assume(Assumption::StubResult(id));
                        }
                        write(vm, dst, r);
                    }
                }
                Instr::Jump { target } => jump(vm, target),
                Instr::JumpIf { cond, target } => {
                    if read(vm, cond) != 0 {
                        jump(vm, target);
                    } else {
                        advance(vm);
                    }
                }
                Instr::Move { dst, src } => {
                    let v = read(vm, src);
                    write(vm, dst, v);
                    advance(vm);
                }
                Instr::LoadInt { dst, value } => {
                    write(vm, dst, value);
                    advance(vm);
                }
                Instr::LoadFixed { dst, value } => {
                    // The original computes in `f32`; the 24.8 rounding is the engine's.
                    vm.assume(Assumption::Opcode(0x14));
                    write(vm, dst, value.raw());
                    advance(vm);
                }
                Instr::Neg { dst, src } => {
                    let v = read(vm, src).wrapping_neg();
                    write(vm, dst, v);
                    advance(vm);
                }
                Instr::IntToFixed { dst, src } => {
                    let v = Fixed::from_int(read(vm, src)).raw();
                    write(vm, dst, v);
                    advance(vm);
                }
                Instr::Binary { op, dst, a, b } => {
                    if let Some(opcode) = op.low_confidence_opcode() {
                        vm.assume(Assumption::Opcode(opcode));
                    }
                    let v = op.apply(read(vm, a), read(vm, b));
                    write(vm, dst, v);
                    advance(vm);
                }
            }
        }
    }
}

fn advance(vm: &mut VmState) {
    if let Some(f) = vm.frames.last_mut() {
        f.pc = f.pc.saturating_add(1);
    }
}

/// Whether the innermost frame runs the function named `name`.
fn frame_is(vm: &VmState, name: &str) -> bool {
    vm.frames.last().is_some_and(|f| {
        vm.program
            .classes
            .get(f.class as usize)
            .and_then(|c| c.functions.get(f.function as usize))
            .is_some_and(|func| func.name == name)
    })
}

fn jump(vm: &mut VmState, target: u32) {
    if let Some(f) = vm.frames.last_mut() {
        f.pc = target;
    }
}

/// Pop `n` values (validated `n <= MAX_STACK`, so the padded vector never exceeds the stack limit).
fn pop_n(stack: &mut Vec<i32>, n: usize, counters: &mut Counters) -> Vec<i32> {
    let n = n.min(MAX_STACK);
    if stack.len() < n {
        inc(&mut counters.faults);
        let mut v = std::mem::take(stack);
        v.resize(n, 0);
        return v;
    }
    stack.split_off(stack.len() - n)
}

fn push_frame(vm: &mut VmState, class: u32, function: u32, params: Vec<i32>) -> bool {
    let Some(f) = vm
        .program
        .classes
        .get(class as usize)
        .and_then(|c| c.functions.get(function as usize))
    else {
        inc(&mut vm.counters.faults);
        return false;
    };
    if vm.frames.len() >= MAX_FRAMES {
        inc(&mut vm.counters.faults);
        vm.set_fault(Fault::CallStackOverflow);
        return false;
    }
    vm.frames.push(Frame {
        class,
        function,
        pc: f.address,
        locals: vec![0; f.locals as usize],
        temps: vec![0; f.temps as usize],
        params,
        result: 0,
    });
    true
}

/// Pop the current frame; returns the value when the outermost frame returned. A frame that
/// returns to a caller writes its result to the destination of the caller's [`Instr::Call`],
/// if the call reads one (the fused `0x0a`), then steps the caller past the call.
fn pop_frame(vm: &mut VmState) -> Option<i32> {
    let done = vm.frames.pop()?;
    let Some(parent) = vm.frames.last() else {
        return Some(done.result);
    };
    let dst = match vm
        .program
        .classes
        .get(parent.class as usize)
        .and_then(|c| c.code.get(parent.pc as usize))
    {
        Some(Instr::Call { dst, .. }) => *dst,
        _ => None,
    };
    if let Some(dst) = dst {
        write(vm, dst, done.result);
    }
    advance(vm);
    None
}

fn read(vm: &mut VmState, s: Slot) -> i32 {
    let v = match s.space {
        Space::Class => vm
            .frames
            .last()
            .and_then(|f| vm.class_vars.get(f.class as usize))
            .and_then(|vars| vars.get(s.index as usize))
            .copied(),
        Space::Local => vm
            .frames
            .last()
            .and_then(|f| f.locals.get(s.index as usize))
            .copied(),
        Space::Temp => vm
            .frames
            .last()
            .and_then(|f| f.temps.get(s.index as usize))
            .copied(),
    };
    v.unwrap_or_else(|| {
        inc(&mut vm.counters.faults);
        0
    })
}

fn write(vm: &mut VmState, s: Slot, v: i32) {
    let ok = match s.space {
        Space::Class => {
            let class = vm.frames.last().map(|f| f.class as usize);
            class
                .and_then(|c| vm.class_vars.get_mut(c))
                .and_then(|vars| vars.get_mut(s.index as usize))
                .map(|slot| *slot = v)
        }
        Space::Local => vm
            .frames
            .last_mut()
            .and_then(|f| f.locals.get_mut(s.index as usize))
            .map(|slot| *slot = v),
        Space::Temp => vm
            .frames
            .last_mut()
            .and_then(|f| f.temps.get_mut(s.index as usize))
            .map(|slot| *slot = v),
    };
    if ok.is_none() {
        inc(&mut vm.counters.faults);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::geom::Geometry;
    use crate::input::{Button, InputEvent};
    use crate::world::{ActorSpec, Instruction, MapInfo, MissionSpec, Scenario, Team};

    /// Slot helpers for hand-assembled programs.
    pub(crate) fn cv(i: u32) -> Slot {
        Slot {
            space: Space::Class,
            index: i,
        }
    }
    pub(crate) fn tv(i: u32) -> Slot {
        Slot {
            space: Space::Temp,
            index: i,
        }
    }
    pub(crate) fn lv(i: u32) -> Slot {
        Slot {
            space: Space::Local,
            index: i,
        }
    }

    /// `(name, param_count, has_result, locals, temps, code)` of a test function.
    pub(crate) type FnSpec<'a> = (&'a str, u32, bool, u32, u32, Vec<Instr>);

    /// A class from [`FnSpec`] functions laid out in order.
    pub(crate) fn class(name: &str, variables: u32, functions: &[FnSpec<'_>]) -> Class {
        let mut code = Vec::new();
        let mut table = Vec::new();
        for (fname, params, has_result, locals, temps, body) in functions {
            table.push(Function {
                name: (*fname).to_string(),
                address: code.len() as u32,
                param_count: *params,
                has_result: *has_result,
                locals: *locals,
                temps: *temps,
            });
            code.push(Instr::Enter {
                locals: *locals,
                temps: *temps,
            });
            code.extend(body.iter().copied());
            code.push(Instr::Return);
        }
        Class {
            name: name.to_string(),
            variable_count: variables,
            functions: table,
            code,
            element: None,
            zone: None,
            rail_point: None,
        }
    }

    /// A 1000x800 open mission with one hero at (100,100) and `guards` soldiers at (300+100i, 300).
    pub(crate) fn mission_world(guards: usize, program: Option<Program>) -> World {
        mission_world_with(guards, program, false)
    }

    /// [`mission_world`] with the unknown-native policy.
    pub(crate) fn mission_world_with(
        guards: usize,
        program: Option<Program>,
        lenient: bool,
    ) -> World {
        let mut actors = vec![ActorSpec {
            profile: "RobinHood".into(),
            team: Team::Player,
            x: 100,
            y: 100,
            facing256: 0,
            patrol: vec![],
            program: vec![],
            active: true,
            hit_points: 100,
            knockout_resistance: 0,
        }];
        for i in 0..guards {
            actors.push(ActorSpec {
                profile: "Soldier A00".into(),
                team: Team::Enemy,
                x: 300 + 100 * i as i32,
                y: 300,
                facing256: 0,
                patrol: vec![],
                program: vec![],
                active: true,
                hit_points: 80,
                knockout_resistance: 0,
            });
        }
        let spec = MissionSpec {
            map: MapInfo {
                width: 1000,
                height: 800,
            },
            geometry: Geometry {
                boundary: vec![(0, 0), (1000, 0), (1000, 800), (0, 800)],
                obstacles: vec![],
                areas: Vec::new(),
            },
            actors,
            script: program,
            rails: vec![vec![
                Instruction::GoTo { x: 500, y: 500 },
                Instruction::Wait { ticks: 5 },
                Instruction::Jump { pc: 0 },
            ]],
            lenient_natives: lenient,
            starting_money: 0,
            assumptions: BTreeSet::new(),
        };
        World::new_mission(Scenario::Mission("T".into()), 9, &spec).unwrap()
    }

    /// Elements: hero = 0, guards 1.., then a polygon zone (index 2 + guards) at (400..600, 400..600).
    pub(crate) fn program(classes: Vec<Class>, guards: u32) -> Program {
        let mut elements = vec![Element::Actor(0)];
        for i in 0..guards {
            elements.push(Element::Actor(1 + i));
        }
        elements.push(Element::Scroll { x: 700, y: 700 });
        elements.push(Element::Polygon(1));
        Program {
            classes,
            elements,
            locations: vec![
                Location::Point { x: 200, y: 200 },
                Location::Polygon(vec![(400, 400), (600, 400), (600, 600), (400, 600)]),
            ],
            wait_scale: (2, 1),
        }
    }

    fn native(id: u32, args: &[i32], result: Option<Slot>, temps_base: u32) -> Vec<Instr> {
        let mut v = Vec::new();
        for (i, a) in args.iter().enumerate() {
            v.push(Instr::LoadInt {
                dst: tv(temps_base + i as u32),
                value: *a,
            });
            v.push(Instr::PushArg {
                src: tv(temps_base + i as u32),
            });
        }
        v.push(Instr::Native {
            id,
            argc: args.len() as u32,
            dst: result,
        });
        v
    }

    #[test]
    fn loops_branches_calls_and_natives() {
        // Level: Initialize sums 0..10 with a loop, calls `double(sum)` and stores it in cv0;
        // CheckVictoryCondition returns 1 when mission var 5 == 55.
        let init = vec![
            Instr::LoadInt {
                dst: lv(0),
                value: 0,
            }, // i
            Instr::LoadInt {
                dst: lv(1),
                value: 0,
            }, // sum
            // 3: loop: t0 = i < 11; if t0 goto 6; goto 12
            Instr::LoadInt {
                dst: tv(1),
                value: 11,
            },
            Instr::Binary {
                op: BinOp::Lt,
                dst: tv(0),
                a: lv(0),
                b: tv(1),
            },
            Instr::JumpIf {
                cond: tv(0),
                target: 7,
            },
            Instr::Jump { target: 13 },
            // 7: sum += i; i += 1; goto 3
            Instr::Binary {
                op: BinOp::Add,
                dst: lv(1),
                a: lv(1),
                b: lv(0),
            },
            Instr::LoadInt {
                dst: tv(1),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::Add,
                dst: lv(0),
                a: lv(0),
                b: tv(1),
            },
            Instr::Nop,
            Instr::Nop,
            Instr::Jump { target: 3 },
            // 13: cv0 = double(sum); n1(5, cv0)
            Instr::PushParam { src: lv(1) },
            Instr::Call {
                function: 2,
                argc: 1,
                dst: Some(cv(0)),
            },
            Instr::Nop,
            Instr::LoadInt {
                dst: tv(0),
                value: 5,
            },
            Instr::PushArg { src: tv(0) },
            Instr::PushArg { src: cv(0) },
            Instr::Native {
                id: 1,
                argc: 2,
                dst: None,
            },
        ];
        let mut victory = native(2, &[5], Some(tv(0)), 0);
        victory.push(Instr::LoadInt {
            dst: tv(1),
            value: 110,
        });
        victory.push(Instr::Binary {
            op: BinOp::Eq,
            dst: tv(2),
            a: tv(0),
            b: tv(1),
        });
        victory.push(Instr::SetResult { src: tv(2) });
        let double = vec![
            Instr::LoadParam {
                dst: tv(0),
                index: 0,
            },
            Instr::LoadInt {
                dst: tv(1),
                value: 2,
            },
            Instr::Binary {
                op: BinOp::Mul,
                dst: tv(2),
                a: tv(0),
                b: tv(1),
            },
            Instr::SetResult { src: tv(2) },
        ];
        let level = class(
            "StartUp",
            1,
            &[
                ("Initialize", 0, false, 2, 4, init),
                ("CheckVictoryCondition", 0, true, 0, 4, victory),
                ("double", 1, true, 0, 4, double),
            ],
        );
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.class_vars[0][0], 110);
        assert_eq!(vm.mission_vars[5], 110);
        assert!(!vm.mission_won);
        assert!(vm.frames.is_empty() && vm.arg_stack.is_empty());
        w.step(&[]);
        assert!(w.vm.as_ref().unwrap().mission_won);
        w.validate().unwrap();
    }

    #[test]
    fn budget_aborts_a_spinning_callback() {
        let spin = vec![Instr::Jump { target: 1 }];
        let level = class("StartUp", 0, &[("Hourglass", 1, false, 0, 0, spin)]);
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert!(vm.counters.budget_aborts >= 1);
        assert!(vm.frames.is_empty());
        assert!(vm.counters.instructions >= WORK_BUDGET_PER_TICK);
        assert_eq!(vm.budget, 0, "the tick stopped at zero");
    }

    #[test]
    fn messages_are_delivered_next_tick_in_order() {
        // Level: Hourglass sends msg 7 (arg 3) to the guard on tick 0 only (guard cv0 == 0),
        // and msg 8 to itself. Guard.ProcessMessage stores the arg in cv0; level stores msg id.
        let mut hourglass = native(2, &[0], Some(tv(0)), 0);
        hourglass.push(Instr::JumpIf {
            cond: tv(0),
            target: 100,
        });
        hourglass.extend(native(44, &[1, 7, 3, 0], None, 0));
        hourglass.extend(native(43, &[0, 8], None, 0));
        hourglass.extend(native(1, &[0, 1], None, 0));
        // Normalise the out-of-range jump above to the return.
        let end = hourglass.len() as u32 + 1;
        for ins in &mut hourglass {
            if let Instr::JumpIf { target, .. } = ins {
                *target = end;
            }
        }
        let level_pm = vec![
            Instr::LoadParam {
                dst: tv(0),
                index: 0,
            },
            Instr::Move {
                dst: cv(0),
                src: tv(0),
            },
        ];
        let guard_pm = vec![
            Instr::LoadParam {
                dst: tv(0),
                index: 1,
            },
            Instr::Move {
                dst: cv(0),
                src: tv(0),
            },
        ];
        let level = class(
            "StartUp",
            1,
            &[
                ("Hourglass", 1, false, 0, 4, hourglass),
                ("ProcessMessage", 3, false, 0, 4, level_pm),
            ],
        );
        let mut guard = class("Guard", 1, &[("ProcessMessage", 3, false, 0, 4, guard_pm)]);
        guard.element = Some(1);
        let mut w = mission_world(1, Some(program(vec![level, guard], 1)));
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.messages.len(), 2, "queued for the next tick");
        assert_eq!(vm.class_vars[1][0], 0);
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.class_vars[1][0], 3);
        assert_eq!(vm.class_vars[0][0], 8);
        assert_eq!(vm.counters.messages_delivered, 2);
        assert!(vm.messages.is_empty());
    }

    #[test]
    fn sequences_block_on_texts_and_waits_then_move_the_camera() {
        // PostInitialize: n26(0,1); n30; n203(0); n32; n203(1); n32; n56(3); n32; n34(n95(n211())); n31
        let mut post = native(26, &[0, 1], None, 0);
        post.extend(native(30, &[], None, 0));
        post.extend(native(203, &[0], None, 0));
        post.extend(native(32, &[], None, 0));
        post.extend(native(203, &[1], None, 0));
        post.extend(native(32, &[], None, 0));
        post.extend(native(56, &[3], None, 0));
        post.extend(native(32, &[], None, 0));
        post.extend(native(211, &[], Some(tv(0)), 0));
        post.push(Instr::PushArg { src: tv(0) });
        post.push(Instr::Native {
            id: 95,
            argc: 1,
            dst: Some(tv(1)),
        });
        post.push(Instr::PushArg { src: tv(1) });
        post.push(Instr::Native {
            id: 34,
            argc: 1,
            dst: None,
        });
        post.extend(native(31, &[], None, 0));
        let level = class("StartUp", 0, &[("PostInitialize", 0, false, 0, 4, post)]);
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        w.camera = (0, 0);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.objectives.len(), 1);
        assert!(vm.objectives[0].primary && !vm.objectives[0].done);
        assert_eq!(vm.pending_texts(), vec![0]);
        assert_eq!(vm.sequences.len(), 1);
        assert!(vm.collecting.is_none());
        for _ in 0..5 {
            w.step(&[]);
        }
        assert_eq!(w.vm.as_ref().unwrap().pending_texts(), vec![0], "blocked");
        assert!(w.vm_dismiss_text());
        assert_eq!(w.vm.as_ref().unwrap().pending_texts(), vec![1]);
        assert!(w.vm_dismiss_text());
        let vm = w.vm.as_ref().unwrap();
        assert!(vm.pending_texts().is_empty());
        // Wait 3 script ticks = 6 world ticks (scale 2/1).
        assert_eq!(vm.sequences[0].wait, SeqWait::Ticks(6));
        assert_eq!(vm.camera_target, None);
        for _ in 0..6 {
            w.step(&[]);
        }
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.camera_target, Some((100, 100)), "camera on the hero");
        assert!(vm.sequences.is_empty());
        assert!(!w.vm_dismiss_text());
        w.validate().unwrap();
    }

    #[test]
    fn zones_fire_enter_and_exit_for_player_characters() {
        let enter = vec![
            Instr::LoadParam {
                dst: tv(0),
                index: 0,
            },
            Instr::Move {
                dst: cv(0),
                src: tv(0),
            },
            Instr::LoadInt {
                dst: tv(1),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::Add,
                dst: cv(1),
                a: cv(1),
                b: tv(1),
            },
        ];
        let exit = vec![
            Instr::LoadInt {
                dst: tv(1),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::Add,
                dst: cv(2),
                a: cv(2),
                b: tv(1),
            },
        ];
        let level = class("StartUp", 0, &[("Initialize", 0, false, 0, 0, vec![])]);
        let mut zone = class(
            "Zone",
            3,
            &[
                ("EnterZone", 1, true, 0, 4, enter),
                ("ExitZone", 1, true, 0, 4, exit),
            ],
        );
        zone.zone = Some(1);
        zone.element = Some(2);
        let mut w = mission_world(1, Some(program(vec![level, zone], 1)));
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().class_vars[1], vec![0, 0, 0]);
        // Teleport the hero into the zone, then out (native 96 through the script would do the
        // same; here the test moves the entity directly).
        w.entities[0].x = Fixed::from_int(500);
        w.entities[0].y = Fixed::from_int(500);
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().class_vars[1], vec![0, 1, 0]);
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().class_vars[1], vec![0, 1, 0]);
        w.entities[0].x = Fixed::from_int(100);
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().class_vars[1], vec![0, 1, 1]);
        // A guard inside the zone does not count.
        w.entities[1].x = Fixed::from_int(500);
        w.entities[1].y = Fixed::from_int(500);
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().class_vars[1], vec![0, 1, 1]);
        w.validate().unwrap();
    }

    /// A left click on a pick-up (item or scroll) with the pointer at map `(x, y)` (the
    /// camera is at the origin in `mission_world`).
    pub(crate) fn click_at(w: &mut World, x: i32, y: i32) {
        w.step(&[
            InputEvent::PointerMove {
                x256: x * 256,
                y256: y * 256,
            },
            InputEvent::PointerDown {
                button: Button::Left,
            },
            InputEvent::PointerUp {
                button: Button::Left,
            },
        ]);
    }

    /// Step `w` until the hero's pick-up order is resolved; returns the steps taken.
    fn steps_until_resolved(w: &mut World, bound: u32) -> u32 {
        let mut ticks = 0;
        while w.entities[0].pickup.is_some() {
            w.step(&[]);
            ticks += 1;
            assert!(ticks < bound, "the pick-up order never resolved");
        }
        ticks
    }

    /// Scrolls are read by an order (`docs/original/h01-measurements-2.md` 1.2 / 1.4,
    /// measured): standing on a scroll or walking past it reads nothing; a click on the scroll
    /// walks the hero to about 18 px short of it, the pause of `SCROLL_PAUSE_TICKS` follows,
    /// then `IsTaken` runs once; a handler that declines leaves the scroll for another order,
    /// one that accepts takes it (the take-on-non-zero rule records `ScrollPickup`); an
    /// inactive scroll is not clickable.
    #[test]
    fn scrolls_are_read_by_an_order_after_the_pause_and_vanish_when_taken() {
        use crate::world::{SCROLL_PAUSE_TICKS, SCROLL_STOP_DISTANCE};
        // IsTaken(actor): cv0 += 1; returns cv1 (0 = leave the scroll, 1 = take it).
        let body = vec![
            Instr::LoadInt {
                dst: tv(1),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::Add,
                dst: cv(0),
                a: cv(0),
                b: tv(1),
            },
            Instr::SetResult { src: cv(1) },
        ];
        let level = class("StartUp", 0, &[("Initialize", 0, false, 0, 0, vec![])]);
        let mut scroll = class("Scroll", 2, &[("IsTaken", 1, true, 0, 4, body)]);
        scroll.element = Some(2); // the scroll at (700, 700) of `program`
        let mut w = mission_world(1, Some(program(vec![level, scroll], 1)));
        let reads = |w: &World| w.vm.as_ref().unwrap().class_vars[1][0];
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().class_vars[1], vec![0, 0]);
        // Standing on the scroll reads nothing: the reading is bound to an order.
        w.entities[0].x = Fixed::from_int(700);
        w.entities[0].y = Fixed::from_int(710);
        for _ in 0..60 {
            w.step(&[]);
        }
        assert_eq!(reads(&w), 0);
        // Select the hero; a ground order beside the scroll (20 px east of it) reads nothing
        // either, though he ends within the old approach radius.
        click_at(&mut w, 700, 710);
        assert_eq!(w.selected, Some(w.entities[0].id));
        click_at(&mut w, 720, 712);
        assert_eq!(w.entities[0].pickup, None);
        assert!(w.entities[0].target.is_some());
        for _ in 0..100 {
            w.step(&[]);
        }
        assert!(w.entities[0].target.is_none());
        assert_eq!(reads(&w), 0);
        // From 100 px north: the order on the scroll (the pointer 5 px above its base, inside
        // the sprite) walks him to 18 px short of it; the handler runs once, SCROLL_PAUSE_TICKS
        // after the arrival, and declines: the scroll stays.
        w.entities[0].x = Fixed::from_int(700);
        w.entities[0].y = Fixed::from_int(600);
        click_at(&mut w, 700, 695);
        assert_eq!(w.entities[0].pickup, Some(2));
        assert!(w.entities[0].target.is_some());
        let mut arrived_after = None;
        let mut ticks = 0u32;
        while w.entities[0].pickup.is_some() {
            w.step(&[]);
            ticks += 1;
            if arrived_after.is_none() && w.entities[0].target.is_none() {
                arrived_after = Some(ticks);
                assert_eq!(w.entities[0].pickup_ticks, SCROLL_PAUSE_TICKS);
            }
            assert!(ticks < 600, "the reading never happened");
        }
        assert_eq!(ticks, arrived_after.unwrap() + SCROLL_PAUSE_TICKS);
        assert_eq!(reads(&w), 1);
        let short = Fixed::length(
            w.entities[0].x - Fixed::from_int(700),
            w.entities[0].y - Fixed::from_int(700),
        )
        .round();
        assert!(
            (SCROLL_STOP_DISTANCE - 4..=SCROLL_STOP_DISTANCE + 6).contains(&short),
            "stopped {short} px short"
        );
        assert!(!w.vm.as_ref().unwrap().inactive_elements.contains(&2));
        assert!(
            !w.vm
                .as_ref()
                .unwrap()
                .assumptions
                .contains(&Assumption::ScrollPickup),
            "a declined reading takes no hypothesis"
        );
        // Accept next time: the scroll is taken and inactive, the rule recorded.
        w.vm.as_mut().unwrap().class_vars[1][1] = 1;
        click_at(&mut w, 700, 695);
        assert_eq!(w.entities[0].pickup, Some(2));
        steps_until_resolved(&mut w, 600);
        assert_eq!(reads(&w), 2);
        assert!(w.vm.as_ref().unwrap().inactive_elements.contains(&2));
        assert!(
            w.vm.as_ref()
                .unwrap()
                .assumptions
                .contains(&Assumption::ScrollPickup)
        );
        // An inactive scroll is not clickable: the click is a ground order.
        click_at(&mut w, 700, 695);
        assert_eq!(w.entities[0].pickup, None);
        assert!(w.entities[0].target.is_some());
        for _ in 0..60 {
            w.step(&[]);
        }
        assert_eq!(reads(&w), 2);
        w.validate().unwrap();
    }

    /// Pick-up items (`docs/original/h01-measurements-2.md` 1, measured): a left click on an
    /// active item walks the selected hero onto it, the stoop of `STOOP_TICKS` follows the
    /// arrival, then the item is taken (arrows add their stack, a purse its money and one
    /// purse, an unknown kind only disappears: the purse and the unknown kind record
    /// `ItemPickup`, the arrows nothing); a walk that ends beside an item takes nothing;
    /// native 235 reads the taken flag and records its policy; a ground order cancels a
    /// pickup under way; a deactivated item is not clickable; the state round-trips.
    #[test]
    fn items_are_taken_on_a_click_and_native_235_reads_it() {
        use crate::world::STOOP_TICKS;
        // Hourglass: cv0 = n235(1); cv1 = n235(2)
        let mut hourglass = native(235, &[1], Some(cv(0)), 0);
        hourglass.extend(native(235, &[2], Some(cv(1)), 0));
        let level = class(
            "StartUp",
            2,
            &[
                ("Initialize", 0, false, 0, 0, vec![]),
                ("Hourglass", 1, false, 0, 4, hourglass),
            ],
        );
        let program = Program {
            classes: vec![level],
            elements: vec![
                Element::Actor(0),
                Element::Item {
                    x: 300,
                    y: 300,
                    kind: ItemKind::Purse,
                    stack: 3,
                },
                Element::Item {
                    x: 100,
                    y: 130,
                    kind: ItemKind::Arrows,
                    stack: 2,
                },
                Element::Item {
                    x: 100,
                    y: 200,
                    kind: ItemKind::Unknown(8),
                    stack: 1,
                },
            ],
            locations: vec![Location::Point { x: 200, y: 200 }],
            wait_scale: (2, 1),
        };
        let mut w = mission_world(0, Some(program));
        let click = |w: &mut World, x: i32, y: i32| {
            w.step(&[
                InputEvent::PointerMove {
                    x256: x * 256,
                    y256: y * 256,
                },
                InputEvent::PointerDown {
                    button: Button::Left,
                },
                InputEvent::PointerUp {
                    button: Button::Left,
                },
            ]);
        };
        let items = w.script_observation().unwrap().items;
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|it| it.active && !it.taken));
        assert_eq!(items[1].kind, ItemKind::Arrows);
        assert_eq!((items[1].x, items[1].y, items[1].stack), (100, 130, 2));
        // Select the hero. A ground order 40 px past the arrows (outside their sprite) walks
        // him over the pile and takes nothing: the take is bound to the order.
        click(&mut w, 100, 100);
        assert_eq!(w.selected, Some(w.entities[0].id));
        click(&mut w, 100, 170);
        assert_eq!(w.entities[0].pickup, None);
        for _ in 0..80 {
            w.step(&[]);
        }
        assert!(w.entities[0].target.is_none());
        assert_eq!(w.entities[0].arrows, 0);
        assert!(w.script_observation().unwrap().items[1].active);
        // A click on the arrows' sprite (4 px above the base): the walk with the pickup
        // intent aims at the item; the stoop follows the arrival and the take ends it.
        click(&mut w, 104, 126);
        assert_eq!(w.entities[0].pickup, Some(2));
        assert_eq!(w.entities[0].pickup_ticks, 0);
        let mut arrived_after = None;
        let mut ticks = 0;
        while w.entities[0].pickup.is_some() {
            w.step(&[]);
            ticks += 1;
            if arrived_after.is_none() && w.entities[0].target.is_none() {
                arrived_after = Some(ticks);
                assert_eq!(w.entities[0].pickup_ticks, STOOP_TICKS);
                assert_eq!(w.entities[0].arrows, 0, "not yet taken");
            }
            assert!(ticks < 200, "the arrows were never taken");
        }
        assert_eq!(ticks, arrived_after.unwrap() + STOOP_TICKS);
        assert_eq!(w.entities[0].pickup_ticks, 0);
        assert_eq!(w.entities[0].arrows, 2);
        assert_eq!(w.entities[0].purses, 0);
        assert!(w.entities[0].target.is_none(), "the walk ends at the item");
        let vm = w.vm.as_ref().unwrap();
        assert!(vm.taken_items.contains(&2) && vm.inactive_elements.contains(&2));
        assert_eq!(vm.money, 0);
        // Hourglass of the next tick reads 235 = 1 for the arrows, 0 for the purse.
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().class_vars[0], vec![0, 1]);
        let items = w.script_observation().unwrap().items;
        assert!(!items[1].active && items[1].taken);
        assert!(items[0].active && !items[0].taken);
        // The arrows' take is measured: only the native's policy is recorded.
        assert_taint_round_trips(&w, &[Assumption::Policy(235)]);
        // The purse: a walk of 280 px; a ground click on the way cancels the pickup, a second
        // click on the purse renews it; the purse adds its money and one purse (the amount is
        // the hypothesis `ItemPickup` records).
        click(&mut w, 300, 300);
        assert_eq!(w.entities[0].pickup, Some(1));
        for _ in 0..20 {
            w.step(&[]);
        }
        click(&mut w, 200, 100);
        assert_eq!(w.entities[0].pickup, None);
        assert!(w.entities[0].target.is_some());
        click(&mut w, 300, 300);
        assert_eq!(w.entities[0].pickup, Some(1));
        let mut ticks = 0;
        while w.entities[0].pickup.is_some() {
            w.step(&[]);
            ticks += 1;
            assert!(ticks < 600, "the purse was never taken");
        }
        assert_eq!(w.vm.as_ref().unwrap().money, 3 * PURSE_MONEY_PER_STACK);
        assert_eq!(w.entities[0].purses, 1);
        assert_eq!(w.entities[0].arrows, 2);
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().class_vars[0], vec![1, 1]);
        assert_taint_round_trips(&w, &[Assumption::Policy(235), Assumption::ItemPickup]);
        // A deactivated item is neither drawn nor clickable; an unknown kind is taken with
        // no effect.
        w.vm.as_mut().unwrap().inactive_elements.insert(3);
        w.step(&[InputEvent::PointerMove {
            x256: 100 * 256,
            y256: 200 * 256,
        }]);
        assert_eq!(w.pickup_at_pointer(), None);
        w.vm.as_mut().unwrap().inactive_elements.remove(&3);
        assert_eq!(w.pickup_at_pointer(), Some(3));
        click(&mut w, 100, 200);
        assert_eq!(w.entities[0].pickup, Some(3));
        let mut ticks = 0;
        while w.entities[0].pickup.is_some() {
            w.step(&[]);
            ticks += 1;
            assert!(ticks < 600, "the unknown item was never taken");
        }
        assert_eq!((w.entities[0].arrows, w.entities[0].purses), (2, 1));
        assert_eq!(w.vm.as_ref().unwrap().money, 3 * PURSE_MONEY_PER_STACK);
        assert!(
            w.script_observation()
                .unwrap()
                .items
                .iter()
                .all(|it| it.taken)
        );
        w.validate().unwrap();
        // Invariants: a pickup order must name an item, the counters stay in range, a taken
        // handle must be an item.
        let mut bad = w.clone();
        bad.entities[0].pickup = Some(0);
        assert!(bad.validate().unwrap_err().contains("pick-up order"));
        let mut bad = w.clone();
        bad.entities[0].arrows = -1;
        assert!(bad.validate().unwrap_err().contains("arrows"));
        let mut bad = w.clone();
        bad.vm.as_mut().unwrap().taken_items.insert(0);
        assert!(bad.validate().unwrap_err().contains("taken item"));
    }

    #[test]
    fn natives_activation_patrol_lock_attributes_and_random() {
        // Initialize: n113(1); n132(2, 0); n134(2, 1); n117(3, 1, 42); cv0 = n118(3, 1);
        // cv1 = n161(10); cv2 = n85(1); cv3 = n79(0); cv4 = n216(); cv5 = n75(); cv6 = n204(3)
        let mut init = native(113, &[1], None, 0);
        init.extend(native(132, &[2, 0], None, 0));
        init.extend(native(134, &[2, 1], None, 0));
        init.extend(native(117, &[3, 1, 42], None, 0));
        init.extend(native(118, &[3, 1], Some(cv(0)), 0));
        init.extend(native(161, &[10], Some(cv(1)), 0));
        init.extend(native(85, &[1], Some(cv(2)), 0));
        init.extend(native(79, &[0], Some(cv(3)), 0));
        init.extend(native(216, &[], Some(cv(4)), 0));
        init.extend(native(75, &[], Some(cv(5)), 0));
        init.extend(native(204, &[4], Some(cv(6)), 0));
        init.extend(native(999, &[1, 2], Some(cv(7)), 0));
        init.extend(native(114, &[1], None, 0));
        init.extend(native(85, &[1], Some(cv(8)), 0));
        let level = class("StartUp", 9, &[("Initialize", 0, false, 0, 4, init)]);
        let mut w = mission_world_with(2, Some(program(vec![level], 2)), true);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.class_vars[0][0], 42);
        assert!(vm.class_vars[0][1] < 10);
        assert_eq!(vm.rng.draws, 1);
        assert_eq!(vm.class_vars[0][2], 1, "deactivated guard is unusable");
        assert_eq!(vm.class_vars[0][3], 1, "hero is a PC");
        assert_eq!(vm.class_vars[0][4], 1, "one PC");
        assert_eq!(vm.class_vars[0][5], 5, "5 elements");
        assert_eq!(vm.class_vars[0][6], 0, "nobody in the zone");
        assert_eq!(
            vm.class_vars[0][7], 0,
            "unknown native returns 0 in lenient mode"
        );
        assert_eq!(vm.class_vars[0][8], 0, "re-activated");
        assert_eq!(vm.counters.unknown_natives.get(&999), Some(&1));
        assert!(!vm.faulted() && vm.lenient);
        assert_eq!(
            vm.unknown_calls,
            vec![UnknownCall {
                id: 999,
                args: vec![1, 2]
            }]
        );
        assert!(w.entities[1].active, "re-activated");
        assert_eq!(w.entities[2].program, Some(0));
        assert!(w.entities[2].ai_locked);
        // The locked guard's program does not run.
        for _ in 0..30 {
            w.step(&[]);
        }
        assert_eq!(w.entities[2].pc, 0);
        assert!(w.entities[2].target.is_none());
        w.validate().unwrap();
    }

    #[test]
    fn unknown_natives_trap_in_strict_mode() {
        // Initialize: cv0 = 1; n999(5); cv1 = 1 -- the trap stops the callback before cv1.
        // Hourglass keeps running afterwards (cv2 counts ticks).
        let mut init = vec![Instr::LoadInt {
            dst: cv(0),
            value: 1,
        }];
        init.extend(native(999, &[5], Some(tv(0)), 0));
        init.push(Instr::LoadInt {
            dst: cv(1),
            value: 1,
        });
        let hourglass = vec![
            Instr::LoadInt {
                dst: tv(0),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::Add,
                dst: cv(2),
                a: cv(2),
                b: tv(0),
            },
        ];
        let level = class(
            "StartUp",
            3,
            &[
                ("Initialize", 0, false, 0, 4, init),
                ("Hourglass", 1, false, 0, 4, hourglass),
            ],
        );
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        let vm = w.vm.as_ref().unwrap();
        assert!(vm.faulted() && !vm.lenient);
        assert_eq!(vm.class_vars[0], vec![1, 0, 0]);
        assert_eq!(vm.counters.traps, 1);
        assert_eq!(vm.counters.unknown_natives.get(&999), Some(&1));
        assert!(vm.unknown_calls.is_empty());
        assert!(vm.frames.is_empty());
        w.step(&[]);
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.class_vars[0], vec![1, 0, 2], "later callbacks still run");
        assert!(w.script_observation().unwrap().faulted);
        // The policy and the log are hashed; a log without lenient mode is refused.
        let h = w.hashes();
        let mut v = w.clone();
        v.vm.as_mut().unwrap().lenient = true;
        assert_ne!(v.hashes().get("scripts"), h.get("scripts"));
        let mut snap = w.snapshot(None);
        snap.world
            .vm
            .as_mut()
            .unwrap()
            .unknown_calls
            .push(UnknownCall {
                id: 1,
                args: vec![],
            });
        assert!(w.restore(&snap).unwrap_err().contains("lenient"));
        w.validate().unwrap();
    }

    #[test]
    fn vm_state_survives_snapshot_restore_and_is_hashed() {
        let mut hourglass = native(161, &[100], Some(tv(0)), 0);
        hourglass.push(Instr::Move {
            dst: cv(0),
            src: tv(0),
        });
        hourglass.extend(native(1, &[3, 1], None, 0));
        let level = class("StartUp", 1, &[("Hourglass", 1, false, 0, 4, hourglass)]);
        let program = program(vec![level], 0);
        let run = |snap_at: Option<u64>| {
            let mut w = mission_world(0, Some(program.clone()));
            let mut saved = None;
            for t in 0..120u64 {
                if Some(t) == snap_at {
                    saved = Some(w.snapshot(None));
                }
                w.step(&[]);
                if snap_at.is_some_and(|s| t == s + 10) {
                    w.restore(saved.as_ref().unwrap()).unwrap();
                    for _ in 0..11 {
                        w.step(&[]);
                    }
                }
            }
            w.hashes()
        };
        let a = run(None);
        assert_eq!(a, run(None));
        let c = run(Some(40));
        assert_eq!(a.total(), c.total(), "{:?}", a.diff(&c));
        // JSON round trip and hash coverage.
        let mut w = mission_world(0, Some(program.clone()));
        for _ in 0..7 {
            w.step(&[]);
        }
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        let snap: crate::world::Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = mission_world(0, None);
        assert_ne!(w2.hashes().total(), w.hashes().total());
        w2.restore(&snap).unwrap();
        assert_eq!(w2.hashes(), w.hashes());
        // Counters and the budget are diagnostics: absent from the snapshot, zero after restore.
        let mut expected = w.vm.clone().unwrap();
        expected.counters = Counters::default();
        expected.budget = 0;
        assert_eq!(w2.vm, Some(expected));
        assert!(!json.contains("\"counters\"") && !json.contains("\"budget\""));
        let h0 = w.hashes();
        let mut v = w.clone();
        v.vm.as_mut().unwrap().mission_vars[9] = 1;
        assert_ne!(v.hashes().get("scripts"), h0.get("scripts"));
        let mut v = w.clone();
        v.vm.as_mut().unwrap().money = 25;
        assert_ne!(v.hashes().get("scripts"), h0.get("scripts"));
        let mut v = w.clone();
        v.vm.as_mut().unwrap().mission_lost = true;
        assert_ne!(v.hashes().get("scripts"), h0.get("scripts"));
        let mut v = w.clone();
        v.vm.as_mut()
            .unwrap()
            .assumptions
            .insert(Assumption::TickRate);
        assert_ne!(v.hashes().get("scripts"), h0.get("scripts"));
        let mut v = w.clone();
        v.vm.as_mut()
            .unwrap()
            .pending_action_changes
            .push(ActionChange {
                class: 0,
                previous: 0,
                new: 6,
            });
        assert_ne!(v.hashes().get("scheduler"), h0.get("scheduler"));
        let mut v = w.clone();
        v.vm.as_mut().unwrap().send(Message {
            target: 0,
            id: 1,
            arg: 0,
            arg2: 0,
        });
        assert_ne!(v.hashes().get("scheduler"), h0.get("scheduler"));
        let mut v = w.clone();
        v.vm.as_mut().unwrap().rng.below(3);
        assert_ne!(v.hashes().get("rng"), h0.get("rng"));
        let mut v = w.clone();
        v.entities[0].active = false;
        assert_ne!(v.hashes().get("actors"), h0.get("actors"));
        let mut v = w.clone();
        v.entities[0].ai_locked = true;
        assert_ne!(v.hashes().get("actors"), h0.get("actors"));
        // Invalid VM snapshots are rejected.
        let mut snap = w.snapshot(None);
        snap.world.vm.as_mut().unwrap().program_digest.clear();
        assert!(w.restore(&snap).unwrap_err().contains("digest"));
        let mut snap = w.snapshot(None);
        snap.world.vm.as_mut().unwrap().class_vars[0].push(1);
        assert!(w.restore(&snap).is_err());
        let mut snap = w.snapshot(None);
        snap.world.vm.as_mut().unwrap().paths = vec![Some(99)];
        assert!(w.restore(&snap).is_err());
        let mut snap = w.snapshot(None);
        snap.world.vm.as_mut().unwrap().program.classes[0].code[1] = Instr::Jump { target: 9999 };
        snap.world.vm.as_mut().unwrap().program_digest =
            snap.world.vm.as_ref().unwrap().program.digest();
        assert!(w.restore(&snap).unwrap_err().contains("out of range"));
        assert_eq!(w.hashes(), h0);
    }

    #[test]
    fn walk_then_barrier_then_text_orders_the_sequence() {
        // PostInitialize: n30; n45(guard, location 0, 0); n32; n203(0); n32; n34(0); n31. The
        // guard (element 1) walks to location 0 = (200, 200); the page shows only once it arrived.
        let mut post = native(30, &[], None, 0);
        post.extend(native(45, &[1, 0, 0], None, 0));
        post.extend(native(32, &[], None, 0));
        post.extend(native(203, &[0], None, 0));
        post.extend(native(32, &[], None, 0));
        post.extend(native(34, &[0], None, 0));
        post.extend(native(31, &[], None, 0));
        let level = class("StartUp", 0, &[("PostInitialize", 0, false, 0, 4, post)]);
        let mut w = mission_world(1, Some(program(vec![level], 1)));
        // Out of the walking guard's sight (`crate::ai`): the walk must not be interrupted.
        w.entities[0].x = Fixed::from_int(900);
        w.entities[0].y = Fixed::from_int(700);
        let vm = w.vm.as_ref().unwrap();
        assert!(
            vm.pending_texts().is_empty(),
            "the page waits behind the barrier"
        );
        assert_eq!(vm.sequences.len(), 1);
        assert_eq!(vm.sequences[0].wait, SeqWait::Barrier);
        assert_eq!(
            vm.sequences[0].tokens,
            vec![SeqToken::Walk {
                entity: 1,
                x: 200,
                y: 200
            }]
        );
        assert!(w.entities[1].target.is_some(), "the guard walks");
        for _ in 0..20 {
            w.step(&[]);
        }
        // Tokens are state: they survive a JSON round trip and are hashed.
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        let snap: crate::world::Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = mission_world(1, None);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.vm.as_ref().unwrap().sequences[0].tokens.len(), 1);
        assert_eq!(w2.hashes(), w.hashes());
        let mut v = w.clone();
        v.vm.as_mut().unwrap().sequences[0].tokens.clear();
        assert_ne!(v.hashes().get("scheduler"), w.hashes().get("scheduler"));
        let mut arrived_at = None;
        for t in 20..400 {
            w.step(&[]);
            if w.entities[1].target.is_none() {
                arrived_at = Some(t);
                break;
            }
            assert!(
                w.vm.as_ref().unwrap().pending_texts().is_empty(),
                "still walking at tick {t}"
            );
        }
        let arrived_at = arrived_at.expect("the guard arrives");
        assert!(arrived_at > 60, "{arrived_at}");
        assert_eq!(
            (w.entities[1].x.round(), w.entities[1].y.round()),
            (200, 200)
        );
        // The scheduler ran before the move on the arrival tick: the page shows on the next one.
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(
            vm.pending_text_requests(),
            &[TextRequest {
                id: 1,
                text: 0,
                blocking: true
            }]
        );
        assert_eq!(vm.sequences[0].wait, SeqWait::Text(1));
        assert!(vm.sequences[0].tokens.is_empty(), "cleared at the barrier");
        assert_eq!(vm.camera_target, None);
        assert!(w.vm_dismiss_text());
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.camera_target, Some((200, 200)));
        assert!(vm.sequences.is_empty());
        w.validate().unwrap();
    }

    #[test]
    fn locking_mid_walk_stops_the_ai_walk_and_completes_the_barrier() {
        // Initialize: n132(guard, path 0) (the rail walks to (500, 500) and loops).
        // Hourglass(t): if t == 30: n134(guard, 1); if t == 60: n135(guard).
        // PostInitialize: n30; n45(guard, location 0, 0); n32; n203(0); n31.
        let init = native(132, &[1, 0], None, 0);
        let mut hourglass = vec![
            Instr::LoadParam {
                dst: tv(0),
                index: 0,
            },
            Instr::LoadInt {
                dst: tv(1),
                value: 30,
            },
            Instr::Binary {
                op: BinOp::Eq,
                dst: tv(2),
                a: tv(0),
                b: tv(1),
            },
            Instr::JumpIf {
                cond: tv(2),
                target: 9,
            },
            Instr::LoadInt {
                dst: tv(1),
                value: 60,
            },
            Instr::Binary {
                op: BinOp::Eq,
                dst: tv(2),
                a: tv(0),
                b: tv(1),
            },
            Instr::JumpIf {
                cond: tv(2),
                target: 15,
            },
            Instr::Return,
        ];
        hourglass.extend(native(134, &[1, 1], None, 0)); // code 9..=13
        hourglass.push(Instr::Return); // code 14
        hourglass.extend(native(135, &[1], None, 0)); // code 15..=17
        let mut post = native(30, &[], None, 0);
        post.extend(native(45, &[1, 0, 0], None, 0));
        post.extend(native(32, &[], None, 0));
        post.extend(native(203, &[0], None, 0));
        post.extend(native(31, &[], None, 0));
        // `Hourglass` comes first: its jump targets are class code indices.
        let level = class(
            "StartUp",
            0,
            &[
                ("Hourglass", 1, false, 0, 4, hourglass),
                ("Initialize", 0, false, 0, 4, init),
                ("PostInitialize", 0, false, 0, 4, post),
            ],
        );
        let mut w = mission_world(1, Some(program(vec![level], 1)));
        assert_eq!(w.entities[1].program, Some(0));
        // The sequence walk (to (200, 200)) is in progress; the rail waits behind it.
        assert_eq!(
            w.entities[1].target,
            Some((Fixed::from_int(200), Fixed::from_int(200)))
        );
        for _ in 0..30 {
            w.step(&[]);
        }
        assert!(w.entities[1].target.is_some() && !w.entities[1].ai_locked);
        assert!(w.vm.as_ref().unwrap().pending_texts().is_empty());
        // Tick 30 locks the guard: its walk stops where it is, the barrier completes.
        w.step(&[]);
        let g = &w.entities[1];
        assert!(g.ai_locked && g.target.is_none() && g.path.is_empty());
        let stopped = (g.x.round(), g.y.round());
        assert_ne!(stopped, (200, 200));
        assert_eq!(w.vm.as_ref().unwrap().pending_texts(), vec![0]);
        assert!(w.vm_dismiss_text());
        for _ in 0..29 {
            w.step(&[]);
            let g = &w.entities[1];
            assert!(g.target.is_none(), "locked: the rail does not start");
            assert_eq!((g.x.round(), g.y.round()), stopped);
        }
        // Tick 60 unlocks it: the rail program issues its walk from where it stands.
        w.step(&[]);
        assert!(!w.entities[1].ai_locked);
        w.step(&[]);
        assert_eq!(
            w.entities[1].target,
            Some((Fixed::from_int(500), Fixed::from_int(500)))
        );
        assert_eq!(w.entities[1].pc, 0);
        // Locking a player character does not touch the player's order.
        w.plan_path(0, (Fixed::from_int(300), Fixed::from_int(100)));
        assert_eq!(w.native_call(134, &[0, 1]), Some(0));
        assert!(w.entities[0].ai_locked && w.entities[0].target.is_some());
        w.validate().unwrap();
    }

    #[test]
    fn text_202_never_blocks_and_203_blocks_its_sequence() {
        // Initialize: n202(5). PostInitialize: n30; n202(7); n203(8); n32; n34(0); n31 (202 is
        // not a sequence element: inside the sequence it runs at collection time).
        let init = native(202, &[5], None, 0);
        let mut post = native(30, &[], None, 0);
        post.extend(native(202, &[7], None, 0));
        post.extend(native(203, &[8], None, 0));
        post.extend(native(32, &[], None, 0));
        post.extend(native(34, &[0], None, 0));
        post.extend(native(31, &[], None, 0));
        let level = class(
            "StartUp",
            0,
            &[
                ("Initialize", 0, false, 0, 4, init),
                ("PostInitialize", 0, false, 0, 4, post),
            ],
        );
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        let req = |id, text, blocking| TextRequest { id, text, blocking };
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(
            vm.pending_text_requests(),
            &[req(1, 5, false), req(2, 7, false), req(3, 8, true)]
        );
        assert_eq!(vm.pending_texts(), vec![5, 7, 8]);
        assert_eq!(vm.sequences[0].wait, SeqWait::Text(3));
        assert_eq!(vm.camera_target, None);
        assert_eq!(
            w.script_observation().unwrap().text_requests,
            vec![req(1, 5, false), req(2, 7, false), req(3, 8, true)]
        );
        // Dismissing the non-blocking texts changes nothing for the sequence.
        assert!(w.vm_dismiss_text());
        assert!(w.vm_dismiss_text());
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.pending_text_requests(), &[req(3, 8, true)]);
        assert_eq!(vm.sequences[0].wait, SeqWait::Text(3));
        assert_eq!(vm.camera_target, None);
        for _ in 0..5 {
            w.step(&[]);
        }
        assert_eq!(w.vm.as_ref().unwrap().camera_target, None, "blocked");
        assert!(w.vm_dismiss_text());
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.camera_target, Some((200, 200)));
        assert!(vm.sequences.is_empty() && vm.texts.is_empty());
        w.validate().unwrap();
    }

    #[test]
    fn work_budget_stops_the_tick_deterministically_and_resumes() {
        // Initialize: n43(hero, 1); n43(hero, 2) (both reach the level's ProcessMessage).
        // ProcessMessage(1): cv1 = 1; spin. ProcessMessage(2): cv2 = 1. Hourglass: cv0 += 1.
        // PostInitialize: n30; n56(2); n34(0); n31. CheckVictoryCondition: 1.
        // Zone class: EnterZone: cv0 += 1; the hero stands inside the zone from the start.
        let mut init = native(43, &[0, 1], None, 0);
        init.extend(native(43, &[0, 2], None, 0));
        let pm = vec![
            Instr::LoadParam {
                dst: tv(0),
                index: 0,
            },
            Instr::LoadInt {
                dst: tv(1),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::Eq,
                dst: tv(2),
                a: tv(0),
                b: tv(1),
            },
            Instr::JumpIf {
                cond: tv(2),
                target: 7,
            },
            Instr::LoadInt {
                dst: cv(2),
                value: 1,
            },
            Instr::Return,
            // 7: cv1 = 1; spin
            Instr::LoadInt {
                dst: cv(1),
                value: 1,
            },
            Instr::Jump { target: 8 },
        ];
        let hourglass = vec![
            Instr::LoadInt {
                dst: tv(0),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::Add,
                dst: cv(0),
                a: cv(0),
                b: tv(0),
            },
        ];
        let mut post = native(30, &[], None, 0);
        post.extend(native(56, &[2], None, 0));
        post.extend(native(34, &[0], None, 0));
        post.extend(native(31, &[], None, 0));
        let victory = vec![
            Instr::LoadInt {
                dst: tv(0),
                value: 1,
            },
            Instr::SetResult { src: tv(0) },
        ];
        // `ProcessMessage` comes first: its jump targets are class code indices.
        let level = class(
            "StartUp",
            3,
            &[
                ("ProcessMessage", 3, false, 0, 4, pm),
                ("Initialize", 0, false, 0, 4, init),
                ("Hourglass", 1, false, 0, 4, hourglass.clone()),
                ("PostInitialize", 0, false, 0, 4, post),
                ("CheckVictoryCondition", 0, true, 0, 4, victory),
            ],
        );
        let mut zone = class("Zone", 1, &[("EnterZone", 1, true, 0, 4, hourglass)]);
        zone.zone = Some(1);
        zone.element = Some(2);
        let mut w = mission_world(0, Some(program(vec![level, zone], 0)));
        w.entities[0].x = Fixed::from_int(500);
        w.entities[0].y = Fixed::from_int(500);
        assert_eq!(w.vm.as_ref().unwrap().sequences[0].wait, SeqWait::Ticks(4));
        // Tick 0: message 1 spins the budget away; message 2 stays queued, every later phase
        // of the tick is skipped.
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.class_vars[0], vec![0, 1, 0]);
        assert_eq!(vm.messages.len(), 1, "requeued");
        assert_eq!(vm.class_vars[1], vec![0], "zone skipped");
        assert_eq!(vm.sequences[0].wait, SeqWait::Ticks(4), "sequence skipped");
        assert!(!vm.mission_won);
        assert!(vm.counters.budget_aborts >= 1);
        assert_eq!(vm.budget, 0);
        // Tick 1: the queued message, the Hourglass, the zone, the sequence and the victory
        // check all run.
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.class_vars[0], vec![1, 1, 1]);
        assert!(vm.messages.is_empty());
        assert_eq!(vm.class_vars[1], vec![1]);
        assert_eq!(vm.sequences[0].wait, SeqWait::Ticks(3));
        assert!(vm.mission_won);
        // The wait was delayed by exactly the skipped tick: camera on tick 4, not 3.
        for _ in 2..4 {
            w.step(&[]);
        }
        assert_eq!(w.vm.as_ref().unwrap().camera_target, None);
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().camera_target, Some((200, 200)));
        assert_eq!(w.tick, 5);
        // Same program, same inputs, same outcome: the budget is part of the ruleset.
        let mut w2 = mission_world(
            0,
            Some(program(
                vec![
                    w.vm.as_ref().unwrap().program.classes[0].clone(),
                    w.vm.as_ref().unwrap().program.classes[1].clone(),
                ],
                0,
            )),
        );
        w2.entities[0].x = Fixed::from_int(500);
        w2.entities[0].y = Fixed::from_int(500);
        for _ in 0..5 {
            w2.step(&[]);
        }
        assert_eq!(w2.hashes(), w.hashes());
        // Path searches issued by the script are charged to the same budget: with none left the
        // walk is dropped and counted; with a fresh budget it is planned.
        let aborts = w.vm.as_ref().unwrap().counters.budget_aborts;
        w.vm.as_mut().unwrap().budget = 0;
        w.vm_walk(0, 200, 200);
        assert!(w.entities[0].target.is_none());
        assert_eq!(w.vm.as_ref().unwrap().counters.budget_aborts, aborts + 1);
        w.vm.as_mut().unwrap().budget = WORK_BUDGET_PER_TICK;
        w.vm_walk(0, 200, 200);
        assert!(w.entities[0].target.is_some());
        assert!(
            w.vm.as_ref().unwrap().budget < WORK_BUDGET_PER_TICK,
            "charged"
        );
        let mut tiny = 3;
        assert!(
            w.plan_path_with(0, (Fixed::from_int(100), Fixed::from_int(100)), &mut tiny)
                .is_err()
        );
        assert!(w.entities[0].target.is_none());
        w.validate().unwrap();
    }

    #[test]
    fn program_validation_is_self_sufficient_and_snapshots_must_be_canonical() {
        // Hourglass(t): cv0 = id(t). id(x): x.
        let body = vec![
            Instr::LoadParam {
                dst: tv(0),
                index: 0,
            },
            Instr::PushParam { src: tv(0) },
            Instr::Call {
                function: 1,
                argc: 1,
                dst: Some(cv(0)),
            },
            Instr::Nop,
        ];
        let callee = vec![
            Instr::LoadParam {
                dst: tv(0),
                index: 0,
            },
            Instr::SetResult { src: tv(0) },
        ];
        let level = class(
            "StartUp",
            1,
            &[
                ("Hourglass", 1, false, 0, 2, body),
                ("id", 1, true, 0, 2, callee),
            ],
        );
        let base = program(vec![level], 0);
        base.validate().unwrap();
        let m = MAX_LOCATION_COORD;
        let reject = |edit: fn(&mut Program), needle: &str| {
            let mut p = base.clone();
            edit(&mut p);
            let err = p.validate().unwrap_err();
            assert!(err.contains(needle), "{err} should mention {needle}");
        };
        reject(
            |p| p.classes[0].functions.swap(0, 1),
            "start with a function",
        );
        reject(|p| p.classes[0].functions[1].address = 0, "table order");
        reject(|p| p.classes[0].functions.clear(), "disagree");
        reject(
            |p| {
                p.classes[0].code[0] = Instr::Enter {
                    locals: 1,
                    temps: 2,
                }
            },
            "prologue",
        );
        reject(
            |p| {
                p.classes[0].code[1] = Instr::LoadParam {
                    dst: tv(0),
                    index: 1,
                }
            },
            "instruction 1 out of range",
        );
        reject(
            |p| {
                p.classes[0].code[3] = Instr::Call {
                    function: 1,
                    argc: 0,
                    dst: None,
                }
            },
            "instruction 3 out of range",
        );
        // A result destination on a callee that leaves no value, or on a function that does
        // not exist, is refused (finding 3 of Codex review 9).
        reject(
            |p| {
                p.classes[0].functions[1].has_result = false;
            },
            "reads the result of function 1",
        );
        reject(
            |p| {
                p.classes[0].code[3] = Instr::Call {
                    function: 7,
                    argc: 1,
                    dst: None,
                }
            },
            "does not exist",
        );
        reject(
            |p| p.classes[0].code[1] = Instr::Jump { target: 7 },
            "instruction 1 out of range",
        );
        reject(
            |p| {
                p.classes[0].code[1] = Instr::Native {
                    id: 999,
                    argc: MAX_STACK as u32 + 1,
                    dst: None,
                }
            },
            "instruction 1 out of range",
        );
        reject(
            |p| {
                p.locations[0] = Location::Point {
                    x: MAX_LOCATION_COORD + 1,
                    y: 0,
                }
            },
            "location 0 out of range",
        );
        reject(
            |p| p.locations[1] = Location::Polygon(vec![(0, 0), (1, i32::MIN), (2, 2)]),
            "location 1 out of range",
        );
        reject(
            |p| p.elements[1] = Element::Scroll { x: 0, y: i32::MAX },
            "element 1 position",
        );
        reject(|p| p.elements[2] = Element::Polygon(0), "element 2 polygon");
        let mut extreme = base.clone();
        extreme.locations[0] = Location::Point { x: -m, y: m };
        extreme.locations[1] = Location::Polygon(vec![(m, m), (-m, m), (0, -m)]);
        extreme.validate().unwrap();

        let mut w = mission_world(1, Some(base));
        w.step(&[]);
        let before = w.hashes();
        let reject_snap = |w: &mut World, edit: fn(&mut VmState), needle: &str| {
            let mut snap = w.snapshot(None);
            edit(snap.world.vm.as_mut().unwrap());
            let err = w.restore(&snap).unwrap_err();
            assert!(err.contains(needle), "{err} should mention {needle}");
        };
        // Non-quiescent snapshots are refused.
        reject_snap(
            &mut w,
            |vm| {
                vm.frames.push(Frame {
                    class: 0,
                    function: 0,
                    pc: 0,
                    locals: vec![],
                    temps: vec![0, 0],
                    params: vec![],
                    result: 0,
                });
            },
            "quiescent",
        );
        reject_snap(&mut w, |vm| vm.arg_stack.push(1), "quiescent");
        reject_snap(&mut w, |vm| vm.param_stack.push(1), "quiescent");
        reject_snap(&mut w, |vm| vm.collecting = Some(Vec::new()), "quiescent");
        // Tables and counters.
        reject_snap(
            &mut w,
            |vm| {
                vm.program.elements.push(Element::Actor(99));
                vm.program_digest = vm.program.digest();
            },
            "entity that does not exist",
        );
        reject_snap(&mut w, |vm| vm.next_text_id = 0, "at least 1");
        reject_snap(
            &mut w,
            |vm| {
                vm.texts.push(TextRequest {
                    id: 3,
                    text: 0,
                    blocking: false,
                });
                vm.texts.push(TextRequest {
                    id: 3,
                    text: 1,
                    blocking: false,
                });
                vm.next_text_id = 4;
            },
            "not increasing",
        );
        reject_snap(
            &mut w,
            |vm| {
                vm.sequences.push(Sequence {
                    elements: vec![],
                    next: 0,
                    wait: SeqWait::Text(9),
                    tokens: vec![],
                });
            },
            "beyond the counter",
        );
        reject_snap(
            &mut w,
            |vm| {
                vm.sequences.push(Sequence {
                    elements: vec![],
                    next: 0,
                    wait: SeqWait::Barrier,
                    tokens: vec![SeqToken::Walk {
                        entity: 7,
                        x: 0,
                        y: 0,
                    }],
                });
            },
            "token out of range",
        );
        reject_snap(
            &mut w,
            |vm| {
                vm.sequences.push(Sequence {
                    elements: vec![SeqElement::Walk {
                        entity: 0,
                        x: i32::MIN,
                        y: 0,
                    }],
                    next: 0,
                    wait: SeqWait::None,
                    tokens: vec![],
                });
            },
            "element out of range",
        );
        // Hostile coordinates through JSON, as a client would send them: refused in either build
        // mode, the world untouched.
        for &c in &[i32::MIN, i32::MAX, -(m + 1), m + 1] {
            let mut json = serde_json::to_value(w.snapshot(None)).unwrap();
            json["world"]["vm"]["program"]["locations"][0] =
                serde_json::json!({ "point": { "x": c, "y": 0 } });
            let mut snap: crate::world::Snapshot = serde_json::from_value(json).unwrap();
            let vm = snap.world.vm.as_mut().unwrap();
            vm.program_digest = vm.program.digest();
            assert!(w.restore(&snap).unwrap_err().contains("location 0"));
        }
        assert_eq!(w.hashes(), before);
        // A saturated text counter drops further requests instead of wrapping.
        let mut v = w.clone();
        let vm = v.vm.as_mut().unwrap();
        vm.next_text_id = u64::MAX;
        assert_eq!(vm.show_text(1, false), None);
        assert_eq!(vm.counters.texts_dropped, 1);
        assert_eq!(vm.next_text_id, u64::MAX);
        v.validate().unwrap();
        // The maximum stack transfer is accepted and never resizes past the limit.
        let mut stack = Vec::new();
        let mut counters = Counters::default();
        let popped = pop_n(&mut stack, MAX_STACK * 4, &mut counters);
        assert_eq!(popped.len(), MAX_STACK);
        assert_eq!(counters.faults, 1);
    }

    #[test]
    fn distance_and_camera_natives_are_total_at_the_coordinate_bounds() {
        // Initialize: cv0 = n160(0, 2); cv1 = n160(0, 0); cv2 = n160(0, -1); n33(2); n34(0).
        // Locations 0 = (-M, -M), 2 = (M, M) with M the coordinate bound.
        let m = MAX_LOCATION_COORD;
        let mut init = native(160, &[0, 2], Some(cv(0)), 0);
        init.extend(native(160, &[0, 0], Some(cv(1)), 0));
        init.extend(native(160, &[0, -1], Some(cv(2)), 0));
        init.extend(native(33, &[2], None, 0));
        init.extend(native(34, &[0], None, 0));
        let level = class("StartUp", 3, &[("Initialize", 0, false, 0, 4, init)]);
        let mut p = program(vec![level], 0);
        p.locations[0] = Location::Point { x: -m, y: -m };
        p.locations.push(Location::Point { x: m, y: m });
        let mut w = mission_world(0, Some(p));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.class_vars[0], vec![2_965_821, 0, i32::MAX]);
        assert_eq!(vm.camera_target, Some((-m, -m)));
        assert_eq!(w.camera, (0, 0));
        w.center_camera_on(m, m);
        assert_eq!(
            w.camera,
            (0, 32),
            "clamped to a 1000x800 map under a 1024x768 viewport"
        );
        w.center_camera_on(-m, -m);
        assert_eq!(w.camera, (0, 0));
        w.validate().unwrap();
    }

    /// A square with [`MAX_POLYGON_VERTICES`] vertices (1024 points per side) covering
    /// (400..1424, 400..1424).
    fn big_square() -> Vec<(i32, i32)> {
        let n = MAX_POLYGON_VERTICES as i32 / 4;
        let mut pts = Vec::with_capacity(MAX_POLYGON_VERTICES);
        pts.extend((0..n).map(|i| (400 + i, 400)));
        pts.extend((0..n).map(|i| (400 + n, 400 + i)));
        pts.extend((0..n).map(|i| (400 + n - i, 400 + n)));
        pts.extend((0..n).map(|i| (400, 400 + n - i)));
        pts
    }

    /// Frames, both stacks and the collecting sequence are empty.
    fn assert_quiescent(w: &World) {
        let vm = w.vm.as_ref().unwrap();
        assert!(vm.frames.is_empty(), "frames");
        assert!(vm.arg_stack.is_empty(), "arg stack");
        assert!(vm.param_stack.is_empty(), "param stack");
        assert!(vm.collecting.is_none(), "collecting");
    }

    /// `World::validate`, then a JSON snapshot restored into a fresh world with equal hashes.
    fn assert_round_trips(w: &World) {
        w.validate().unwrap();
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        let snap: crate::world::Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = mission_world(0, None);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.hashes(), w.hashes());
    }

    /// A scroll's reading draws from what the tick left of the budget: a handler the budget
    /// cuts short ran once and is consumed (the scroll stays, no refill); when the budget is
    /// spent before the handler can start (an `Hourglass` that spins) the reading waits, one
    /// tick at a time, until a tick has work left.
    #[test]
    fn a_scroll_reading_draws_from_the_ticks_remaining_budget() {
        // IsTaken: counts its call and spins. Hourglass: spins while cv0 of the level is set.
        let handler = vec![
            Instr::LoadInt {
                dst: tv(0),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::Add,
                dst: cv(0),
                a: cv(0),
                b: tv(0),
            },
            Instr::Jump { target: 3 },
        ];
        let hourglass = vec![
            Instr::JumpIf {
                cond: cv(0),
                target: 2,
            },
            Instr::Return,
            Instr::Jump { target: 2 },
        ];
        let level = class(
            "StartUp",
            1,
            &[
                ("Initialize", 0, false, 0, 0, vec![]),
                ("Hourglass", 1, false, 0, 1, hourglass),
            ],
        );
        let mut scroll = class("Scroll", 1, &[("IsTaken", 1, true, 0, 1, handler)]);
        scroll.element = Some(1);
        let program = Program {
            classes: vec![level, scroll],
            elements: vec![Element::Actor(0), Element::Scroll { x: 700, y: 700 }],
            locations: vec![],
            wait_scale: (2, 1),
        };
        let mut w = mission_world(0, Some(program));
        let reads = |w: &World| w.vm.as_ref().unwrap().class_vars[1][0];
        w.entities[0].x = Fixed::from_int(700);
        w.entities[0].y = Fixed::from_int(640);
        click_at(&mut w, 700, 640);
        click_at(&mut w, 700, 695);
        assert_eq!(w.entities[0].pickup, Some(1));
        // The tick of the reading (the pause's last tick) with fresh counters.
        let mut ticks = 0;
        while w.entities[0].pickup_ticks != 1 {
            w.step(&[]);
            ticks += 1;
            assert!(ticks < 600, "the pause never ran out");
        }
        w.vm.as_mut().unwrap().counters = Counters::default();
        w.step(&[]);
        assert_eq!(w.entities[0].pickup, None);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(reads(&w), 1, "the handler ran once");
        assert!(
            vm.counters.instructions <= WORK_BUDGET_PER_TICK,
            "{} instructions: the reading did not share the tick's budget",
            vm.counters.instructions
        );
        assert_eq!(vm.budget, 0);
        assert!(!vm.inactive_elements.contains(&1), "cut short: not taken");
        assert_quiescent(&w);
        // The hooks draw from what the tick left: nothing left, nothing runs, no refill.
        assert_eq!(w.vm_is_taken(1, 0), None);
        assert_eq!(w.vm.as_ref().unwrap().budget, 0);
        assert_eq!(reads(&w), 1);
        // A second order with the Hourglass spinning: the pause runs out but the reading
        // waits (the pause stays at one tick) until the Hourglass yields the budget again.
        click_at(&mut w, 700, 695);
        assert_eq!(w.entities[0].pickup, Some(1));
        w.vm.as_mut().unwrap().class_vars[0][0] = 1;
        for _ in 0..200 {
            w.step(&[]);
        }
        assert_eq!(w.entities[0].pickup, Some(1));
        assert_eq!(w.entities[0].pickup_ticks, 1);
        assert_eq!(reads(&w), 1);
        w.vm.as_mut().unwrap().class_vars[0][0] = 0;
        w.step(&[]);
        assert_eq!(reads(&w), 2);
        assert_eq!(w.entities[0].pickup, None);
        assert_round_trips(&w);
    }

    #[test]
    fn dismissals_between_ticks_draw_from_the_ticks_remaining_budget() {
        // Hourglass: spins on its first call only (cv0 marks it). Initialize: n202(7), a
        // notice. PostInitialize: n30; n203(1); n34(0); n31 (a page, then a camera move).
        let hourglass = vec![
            Instr::JumpIf {
                cond: cv(0),
                target: 4,
            },
            Instr::LoadInt {
                dst: cv(0),
                value: 1,
            },
            Instr::Jump { target: 3 },
        ];
        let init = native(202, &[7], None, 0);
        let mut post = native(30, &[], None, 0);
        post.extend(native(203, &[1], None, 0));
        post.extend(native(34, &[0], None, 0));
        post.extend(native(31, &[], None, 0));
        let level = class(
            "StartUp",
            1,
            &[
                ("Hourglass", 1, false, 0, 1, hourglass),
                ("Initialize", 0, false, 0, 4, init),
                ("PostInitialize", 0, false, 0, 4, post),
            ],
        );
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(
            vm.texts
                .iter()
                .map(|t| (t.text, t.blocking))
                .collect::<Vec<_>>(),
            vec![(7, false), (1, true)]
        );
        assert!(
            vm.budget > 0 && vm.budget < WORK_BUDGET_AT_LOAD,
            "the load run drew from its own budget"
        );
        // A notice dismissed right after load draws from the load budget's remainder (the
        // sequence still waits for its page, so nothing is charged) and grants nothing.
        let left = vm.budget;
        assert!(w.vm_dismiss_text());
        assert_eq!(w.vm.as_ref().unwrap().budget, left);
        // Tick 0: Hourglass spins the budget away; the sequence phase is skipped.
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().budget, 0);
        // The page dismissed between ticks is removed, but its sequence gets no new budget: the
        // camera move behind it waits for the next tick.
        let aborts = w.vm.as_ref().unwrap().counters.budget_aborts;
        assert!(w.vm_dismiss_text());
        let vm = w.vm.as_ref().unwrap();
        assert!(vm.texts.is_empty());
        assert_eq!(vm.budget, 0, "no budget between ticks");
        assert_eq!(vm.camera_target, None);
        assert!(matches!(vm.sequences[0].wait, SeqWait::Text(_)));
        assert_eq!(vm.counters.budget_aborts, aborts + 1);
        assert!(!w.vm_dismiss_text(), "nothing pending");
        // Tick 1: Hourglass returns at once; the sequence continues and finishes.
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.camera_target, Some((200, 200)));
        assert!(vm.sequences.is_empty());
        assert_round_trips(&w);
    }

    #[test]
    fn polygon_natives_charge_edges_and_entities_before_scanning() {
        // Hourglass: cv0 = n97(hero, big polygon); cv1 = n204(big polygon).
        let mut body = native(97, &[0, 2], Some(cv(0)), 0);
        body.extend(native(204, &[2], Some(cv(1)), 0));
        let level = class("StartUp", 2, &[("Hourglass", 1, false, 0, 4, body)]);
        let mut program = program(vec![level], 0);
        program.locations.push(Location::Polygon(big_square()));
        program.validate().unwrap();
        let mut w = mission_world(3, Some(program));
        w.entities[0].x = Fixed::from_int(500);
        w.entities[0].y = Fixed::from_int(500);
        let edges = MAX_POLYGON_VERTICES as u64;
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.class_vars[0], vec![1, 1]);
        // 13 units of dispatch (10 instructions, 3 arguments), the edges for native 97, one per
        // entity (hero and three guards) plus the edges of the one player character for 204.
        let used = WORK_BUDGET_PER_TICK - vm.budget;
        assert_eq!(used, 13 + edges + (4 + edges));
        // Too little for native 97: nothing is scanned, its result is 0 (stored by the fused
        // call) and the callback aborts at its next instruction with the budget at zero.
        let aborts = vm.counters.budget_aborts;
        let vm = w.vm.as_mut().unwrap();
        vm.class_vars[0] = vec![5, 5];
        vm.budget = edges;
        assert_eq!(
            w.vm_callback(0, callbacks::HOURGLASS, &[1]),
            Some(CallOutcome::Exhausted)
        );
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.budget, 0);
        assert_eq!(vm.class_vars[0], vec![0, 5], "97 answered 0, 204 never ran");
        assert!(vm.counters.budget_aborts > aborts);
        assert_quiescent(&w);
        // Enough for native 97 and the dispatch up to 204, not for 204's first entity.
        let vm = w.vm.as_mut().unwrap();
        vm.class_vars[0] = vec![5, 5];
        vm.budget = 7 + edges + 5;
        assert_eq!(
            w.vm_callback(0, callbacks::HOURGLASS, &[1]),
            Some(CallOutcome::Exhausted)
        );
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.class_vars[0], vec![1, 0]);
        assert_eq!(vm.budget, 0);
        assert_quiescent(&w);
        assert_round_trips(&w);
    }

    #[test]
    fn every_callback_exit_tears_down_to_a_quiescent_vm() {
        // (a) Budget abort with values pending on both stacks: the loop keeps the depths, so
        // the program is balanced and valid.
        let spin = vec![
            Instr::LoadInt {
                dst: tv(0),
                value: 1,
            },
            Instr::PushArg { src: tv(0) },
            Instr::PushParam { src: tv(0) },
            Instr::Jump { target: 4 },
        ];
        let level = class("StartUp", 0, &[("Hourglass", 1, false, 0, 1, spin)]);
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        w.step(&[]);
        assert!(w.vm.as_ref().unwrap().counters.budget_aborts >= 1);
        assert_quiescent(&w);
        assert_round_trips(&w);
        // (b) Trap with values pending and a sequence being collected: Initialize: n30; three
        // arguments pushed; n999 takes one and traps; n86 would take the other two; n31.
        let mut init = native(30, &[], None, 0);
        init.push(Instr::LoadInt {
            dst: tv(0),
            value: 5,
        });
        init.extend([Instr::PushArg { src: tv(0) }; 3]);
        init.push(Instr::Native {
            id: 999,
            argc: 1,
            dst: None,
        });
        init.push(Instr::Native {
            id: 86,
            argc: 2,
            dst: None,
        });
        init.extend(native(31, &[], None, 0));
        let level = class("StartUp", 0, &[("Initialize", 0, false, 0, 1, init)]);
        let w = mission_world(0, Some(program(vec![level], 0)));
        let vm = w.vm.as_ref().unwrap();
        assert!(vm.faulted());
        assert_eq!(vm.counters.traps, 1);
        assert!(
            vm.sequences.is_empty(),
            "the collected sequence was dropped"
        );
        assert_quiescent(&w);
        assert_round_trips(&w);
        // (c) Returns: a nested call returns while the caller holds values on both stacks (a
        // balanced program), and the outermost return leaves everything empty.
        let hourglass = vec![
            Instr::LoadInt {
                dst: tv(0),
                value: 1,
            },
            Instr::PushArg { src: tv(0) },
            Instr::PushParam { src: tv(0) },
            Instr::PushParam { src: tv(0) },
            Instr::Call {
                function: 1,
                argc: 1,
                dst: None,
            },
            Instr::Call {
                function: 1,
                argc: 1,
                dst: None,
            },
            Instr::Native {
                id: 3,
                argc: 1,
                dst: Some(cv(0)),
            },
        ];
        let inner = vec![
            Instr::LoadParam {
                dst: tv(0),
                index: 0,
            },
            Instr::SetResult { src: tv(0) },
        ];
        let level = class(
            "StartUp",
            1,
            &[
                ("Hourglass", 1, false, 0, 1, hourglass),
                ("inner", 1, true, 0, 1, inner),
            ],
        );
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.class_vars[0], vec![1]);
        assert_eq!(vm.counters.faults, 0);
        assert_quiescent(&w);
        assert_round_trips(&w);
        // (d) A program that would return with surplus values, or call with too few pushed, is
        // invalid; injected past validation, the surplus is still torn down on return.
        let sloppy = vec![
            Instr::LoadInt {
                dst: tv(0),
                value: 1,
            },
            Instr::PushArg { src: tv(0) },
            Instr::PushParam { src: tv(0) },
        ];
        let level = class("StartUp", 0, &[("Hourglass", 1, false, 0, 1, sloppy)]);
        let p = program(vec![level], 0);
        assert!(p.validate().unwrap_err().contains("not balanced"));
        let starved = vec![Instr::Call {
            function: 1,
            argc: 1,
            dst: None,
        }];
        let level = class(
            "StartUp",
            0,
            &[
                ("Hourglass", 1, false, 0, 1, starved),
                ("inner", 1, true, 0, 1, vec![]),
            ],
        );
        assert!(
            program(vec![level], 0)
                .validate()
                .unwrap_err()
                .contains("not balanced")
        );
        let mut w = mission_world(0, None);
        w.vm = Some(VmState::new(p, vec![], 9, false));
        assert_eq!(
            w.vm_callback(0, callbacks::HOURGLASS, &[0]),
            Some(CallOutcome::Returned(0))
        );
        assert_quiescent(&w);
        assert!(
            w.validate().is_err(),
            "the unbalanced program stays invalid"
        );
    }

    #[test]
    fn location_values_pack_points() {
        let v = location_of_point(1234, 567);
        assert!(v & LOCATION_POINT_BIT != 0);
        assert_eq!(crate::natives::unpack_point(v), Some((1234, 567)));
        assert_eq!(crate::natives::unpack_point(5), None);
        assert_eq!(location_of_point(-5, 40000), location_of_point(0, 0x7fff));
    }

    /// The stub policy table of `docs/formats/scb.md` ("Natives at load per mission"): the
    /// low-confidence natives with a required return value and the index / identity natives
    /// implemented from it, in strict mode, without a trap.
    #[test]
    fn policy_values_of_the_stub_table_are_pinned() {
        // Elements: hero 0, guards 1 / 2, scroll 3, zone 4.
        let mut init = native(128, &[1], Some(cv(0)), 0);
        init.extend(native(240, &[1], Some(cv(1)), 0));
        init.extend(native(253, &[27], Some(cv(2)), 0));
        init.extend(native(255, &[1], Some(cv(3)), 0));
        init.extend(native(205, &[4, 0], Some(cv(4)), 0));
        init.extend(native(119, &[], Some(cv(5)), 0));
        init.extend(native(231, &[4], Some(cv(6)), 0));
        init.extend(native(246, &[4], Some(cv(7)), 0));
        init.extend(native(8, &[4], Some(cv(8)), 0));
        init.extend(native(98, &[0, -1], Some(cv(9)), 0));
        init.extend(native(98, &[0, 4], Some(cv(10)), 0));
        init.extend(native(12, &[7], Some(cv(11)), 0));
        init.extend(native(13, &[9], Some(cv(12)), 0));
        init.extend(native(86, &[1, 1], Some(cv(13)), 0));
        init.extend(native(86, &[1, 2], Some(cv(14)), 0));
        init.extend(native(250, &[0], Some(cv(15)), 0));
        init.extend(native(211, &[], Some(cv(16)), 0));
        init.extend(native(245, &[], Some(cv(17)), 0));
        init.extend(native(20, &[0], None, 0));
        init.extend(native(192, &[], Some(cv(18)), 0));
        // The Sherwood hub's team natives (`docs/formats/sherwood-hub.md`): recorded stubs, 174
        // with the policy limit.
        init.extend(native(174, &[], Some(cv(19)), 0));
        init.extend(native(170, &[], Some(cv(20)), 0));
        init.extend(native(249, &[], Some(cv(21)), 0));
        init.extend(native(172, &[], Some(cv(22)), 0));
        init.extend(native(173, &[], Some(cv(23)), 0));
        init.extend(native(165, &[0], None, 0));
        init.extend(native(166, &[0], None, 0));
        init.extend(native(239, &[], None, 0));
        let level = class("StartUp", 24, &[("Initialize", 0, false, 0, 4, init)]);
        let w = mission_world(2, Some(program(vec![level], 2)));
        let vm = w.vm.as_ref().unwrap();
        assert!(
            !vm.faulted() && vm.counters.traps == 0 && vm.counters.faults == 0,
            "{:?}",
            vm.counters
        );
        assert!(vm.counters.unknown_natives.is_empty());
        let v = &vm.class_vars[0];
        assert_eq!(v[0], 1, "128: able to act");
        assert_eq!(v[1], 1, "240: present");
        assert_eq!(v[2], 1, "253: campaign character alive");
        assert_eq!(v[3], 1, "255: campaign character present");
        assert_eq!(v[4], NONE_HANDLE, "205: no actor in the zone");
        assert_eq!(v[5], 0, "119: not won");
        assert_eq!((v[6], v[7]), (0, 0), "231 / 246: nobody inside");
        assert_eq!(v[8], 4, "8: the building index itself");
        assert_eq!((v[9], v[10]), (1, 0), "98: outdoors only");
        assert_eq!((v[11], v[12]), (7, 9), "12 / 13: index inverses");
        assert_eq!((v[13], v[14]), (1, 0), "86: handle equality");
        assert_eq!((v[15], v[16]), (0, 0), "250(0) is 211's value: the hero");
        assert_eq!(v[17], 1, "245: one live player character");
        assert_eq!(v[18], NONE_HANDLE, "192: the level class has no element");
        assert_eq!(v[19], 5, "174: the team size limit");
        assert_eq!(
            (v[20], v[21], v[22], v[23]),
            (0, 0, 0, 0),
            "170 / 249 / 172 / 173: no team, nobody to send, no level selected, state 0"
        );
        for id in [
            253, 255, 205, 119, 231, 246, 20, 174, 170, 249, 172, 173, 165, 166, 239,
        ] {
            assert_eq!(
                vm.counters.stub_natives.get(&id),
                Some(&1),
                "stub {id} recorded"
            );
        }
        for id in [8, 12, 13, 86, 98, 128, 240, 245, 250, 192] {
            assert_eq!(
                crate::natives::native_status(id),
                crate::natives::NativeStatus::Implemented
            );
            assert!(
                !vm.counters.stub_natives.contains_key(&id),
                "{id} is implemented"
            );
        }
        for (id, _) in crate::natives::STUB_POLICY_VALUES {
            assert_eq!(
                crate::natives::native_status(*id),
                crate::natives::NativeStatus::Stub
            );
        }
        w.validate().unwrap();
    }

    /// Natives 85 / 87 / 90 / 128 / 240 read the stealth layer's states (`crate::ai`), 140 sets
    /// the gait of an NPC's program walks.
    #[test]
    fn state_natives_read_the_stealth_layer() {
        use crate::ai::AiState;
        use crate::world::Gait;
        // Elements: hero 0, guard 1, scroll 2, zone 3.
        let level = class("StartUp", 0, &[]);
        let mut w = mission_world(1, Some(program(vec![level], 1)));
        let read = |w: &mut World, id: u32, handle: i32| w.native_call(id, &[handle]).unwrap();
        assert_eq!(
            (
                read(&mut w, 85, 1),
                read(&mut w, 87, 1),
                read(&mut w, 90, 1)
            ),
            (0, 0, 0)
        );
        assert_eq!((read(&mut w, 128, 1), read(&mut w, 240, 1)), (1, 1));
        assert_eq!(
            (read(&mut w, 128, 2), read(&mut w, 240, 2)),
            (1, 1),
            "non-actors act"
        );
        w.entities[1].ai_state = AiState::KnockedDown;
        assert_eq!((read(&mut w, 90, 1), read(&mut w, 128, 1)), (1, 0));
        w.entities[1].ai_state = AiState::Lying;
        assert_eq!(
            (
                read(&mut w, 85, 1),
                read(&mut w, 87, 1),
                read(&mut w, 90, 1)
            ),
            (0, 0, 1)
        );
        assert_eq!(
            read(&mut w, 240, 1),
            1,
            "a knocked-out soldier is still present"
        );
        w.entities[1].ai_state = AiState::GettingUp;
        assert_eq!((read(&mut w, 90, 1), read(&mut w, 128, 1)), (0, 0));
        // The knock-out reached the script through 90 / 128: the outcome is tainted.
        assert!(
            w.vm.as_ref()
                .unwrap()
                .assumptions
                .contains(&Assumption::KnockOut),
            "{:?}",
            w.vm.as_ref().unwrap().assumptions
        );
        // Dead is one state (`Dead` with `alive` cleared, the only form `validate` accepts):
        // removed, dead, out of action and unable to act, all from one reading.
        w.entities[1].ai_state = AiState::Dead;
        w.entities[1].alive = false;
        assert_eq!(
            (
                read(&mut w, 85, 1),
                read(&mut w, 87, 1),
                read(&mut w, 90, 1),
                read(&mut w, 128, 1)
            ),
            (1, 1, 1, 0)
        );
        w.entities[1].ai_state = AiState::Patrol;
        w.entities[1].alive = true;
        w.entities[1].active = false;
        assert_eq!(
            (
                read(&mut w, 85, 1),
                read(&mut w, 128, 1),
                read(&mut w, 240, 1)
            ),
            (1, 0, 0)
        );
        w.entities[1].active = true;
        assert_eq!(w.vm.as_ref().unwrap().counters.out_of_action_true, 3);
        // A deactivated scroll is not present.
        w.native_call(113, &[2]);
        assert_eq!(read(&mut w, 240, 2), 0);
        w.native_call(114, &[2]);
        assert_eq!(read(&mut w, 240, 2), 1);
        // 140: the guard's rail walks run from now on (the rail of `mission_world`).
        assert_eq!(w.entities[1].npc_gait, Gait::Walk);
        w.native_call(140, &[1, 1]);
        assert_eq!(w.entities[1].npc_gait, Gait::Run);
        w.native_call(132, &[1, 0]);
        w.step(&[]);
        assert_eq!(w.entities[1].gait, Gait::Run);
        assert!(w.entities[1].target.is_some());
        w.native_call(140, &[1, 0]);
        assert_eq!(w.entities[1].npc_gait, Gait::Walk);
        assert_eq!(
            w.entities[1].gait,
            Gait::Run,
            "the walk under way keeps its gait"
        );
        assert_eq!(
            crate::natives::native_status(88),
            crate::natives::NativeStatus::Stub,
            "no tied-up state exists"
        );
        w.validate().unwrap();
    }

    /// `ActionChange(previous, new)` reaches the class bound to the actor whose action id
    /// changed (the first mission's archers key on 141, `docs/formats/scb.md`).
    #[test]
    fn action_changes_reach_the_actors_class() {
        use crate::ai::{AiState, actions};
        // ActionChange(a, b): cv0 = b, cv1 = a, cv2 += 1.
        let body = vec![
            Instr::LoadParam {
                dst: cv(0),
                index: 1,
            },
            Instr::LoadParam {
                dst: cv(1),
                index: 0,
            },
            Instr::LoadInt {
                dst: tv(0),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::Add,
                dst: cv(2),
                a: cv(2),
                b: tv(0),
            },
        ];
        let level = class("StartUp", 0, &[]);
        let mut guard = class("Guard", 3, &[("ActionChange", 2, false, 0, 4, body)]);
        guard.element = Some(1);
        let mut w = mission_world(1, Some(program(vec![level, guard], 1)));
        // The hero steps in front of the guard (at (300, 300) facing +x): noticed.
        w.entities[0].x = Fixed::from_int(420);
        w.entities[0].y = Fixed::from_int(300);
        w.step(&[]);
        assert_eq!(w.entities[1].ai_state, AiState::Noticed);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.class_vars[1][0], actions::NOTICED as i32, "new action");
        assert_eq!(vm.class_vars[1][1], actions::IDLE as i32, "previous action");
        assert_eq!(vm.class_vars[1][2], 1);
        for _ in 0..crate::ai::NOTICED_TICKS {
            w.step(&[]);
        }
        assert_eq!(w.entities[1].ai_state, AiState::Alarm);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.class_vars[1][0], actions::ALARM as i32);
        assert_eq!(vm.class_vars[1][1], actions::NOTICED as i32);
        assert_eq!(vm.class_vars[1][2], 2);
        assert_eq!(vm.counters.faults, 0);
        w.validate().unwrap();
    }

    /// Native 192 returns the element of the calling class for non-actor classes, as 74 does.
    #[test]
    fn native_192_is_the_calling_classs_own_element() {
        let mut init = native(192, &[], Some(cv(0)), 0);
        init.extend(native(74, &[], Some(cv(1)), 0));
        let level = class("StartUp", 0, &[]);
        let mut scroll = class("Scroll", 2, &[("Initialize", 0, false, 0, 4, init)]);
        scroll.element = Some(1);
        let w = mission_world(0, Some(program(vec![level, scroll], 0)));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.class_vars[1], vec![1, 1]);
        assert!(!vm.faulted());
    }

    /// Natives 93 / 94 / 133: sixteen directions on the 256-unit facing, direction 0 = facing 0
    /// (`natives::FACING_UNITS_PER_DIRECTION`, low confidence, pinned here); 133 teleports as 96.
    #[test]
    fn facing_natives_map_sixteen_directions_onto_facing256() {
        let mut init = native(94, &[1, 5], None, 0);
        init.extend(native(93, &[1], Some(cv(0)), 0));
        init.extend(native(133, &[2, 0, 12], None, 0));
        init.extend(native(93, &[2], Some(cv(1)), 0));
        init.extend(native(93, &[3], Some(cv(2)), 0));
        init.extend(native(94, &[2, -1], None, 0));
        init.extend(native(93, &[2], Some(cv(3)), 0));
        let level = class("StartUp", 4, &[("Initialize", 0, false, 0, 4, init)]);
        let w = mission_world(2, Some(program(vec![level], 2)));
        let vm = w.vm.as_ref().unwrap();
        assert!(!vm.faulted());
        assert_eq!(vm.class_vars[0], vec![5, 12, 0, 15]);
        assert_eq!(w.entities[1].facing256, 80);
        assert_eq!(
            (w.entities[2].x, w.entities[2].y),
            (Fixed::from_int(200), Fixed::from_int(200)),
            "133 placed the guard at location 0"
        );
        assert_eq!(w.entities[2].facing256, 240, "-1 wraps to direction 15");
        w.validate().unwrap();
    }

    /// Natives 236 / 237 share one hashed integer that survives a snapshot.
    #[test]
    fn money_natives_share_one_hashed_integer() {
        let mut init = native(237, &[100_000], None, 0);
        init.extend(native(236, &[], Some(cv(0)), 0));
        // Hourglass: n237(n236() - 2000).
        let mut hourglass = native(236, &[], Some(tv(0)), 0);
        hourglass.push(Instr::LoadInt {
            dst: tv(1),
            value: 2000,
        });
        hourglass.push(Instr::Binary {
            op: BinOp::Sub,
            dst: tv(2),
            a: tv(0),
            b: tv(1),
        });
        hourglass.push(Instr::PushArg { src: tv(2) });
        hourglass.push(Instr::Native {
            id: 237,
            argc: 1,
            dst: None,
        });
        let level = class(
            "StartUp",
            1,
            &[
                ("Initialize", 0, false, 0, 4, init),
                ("Hourglass", 1, false, 0, 4, hourglass),
            ],
        );
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        assert_eq!(w.vm.as_ref().unwrap().class_vars[0][0], 100_000);
        assert_eq!(w.vm.as_ref().unwrap().money, 100_000);
        w.step(&[]);
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().money, 96_000);
        let snap = w.snapshot(None);
        let mut w2 = mission_world(0, None);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.vm.as_ref().unwrap().money, 96_000);
        assert_eq!(w2.hashes(), w.hashes());
        w.step(&[]);
        assert_ne!(w2.hashes().get("scripts"), w.hashes().get("scripts"));
    }

    /// `CheckVictoryCondition` returning 2 marks the mission lost (sticky, observable).
    #[test]
    fn check_victory_condition_two_marks_the_mission_lost() {
        // Initialize: cv0 = 4. Hourglass: cv0 = cv0 - 1. CheckVictoryCondition: n28(1); return cv0.
        let init = vec![Instr::LoadInt {
            dst: cv(0),
            value: 4,
        }];
        let hourglass = vec![
            Instr::LoadInt {
                dst: tv(0),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::Sub,
                dst: cv(0),
                a: cv(0),
                b: tv(0),
            },
        ];
        let mut victory = native(28, &[1], None, 0);
        victory.push(Instr::SetResult { src: cv(0) });
        let level = class(
            "StartUp",
            1,
            &[
                ("Initialize", 0, false, 0, 4, init),
                ("Hourglass", 1, false, 0, 4, hourglass),
                ("CheckVictoryCondition", 0, true, 0, 4, victory),
            ],
        );
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert!(!vm.mission_lost && !vm.mission_won, "3 = still running");
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert!(vm.mission_lost && !vm.mission_won, "2 = lost");
        assert_eq!(vm.debriefing, Some(1));
        let obs = w.script_observation().unwrap();
        assert!(obs.mission_lost && !obs.mission_won);
        w.step(&[]);
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert!(vm.mission_lost, "sticky");
        assert!(
            vm.mission_won,
            "1 = won is recorded independently and stays"
        );
        w.validate().unwrap();
    }

    /// The taint model (ADR-0008, "Hypotheses and taint"): a stub's result consumed by the
    /// script, an effect stub called (read or not), a wait executed and the `Hourglass` time
    /// read each record an assumption; the set is observable, hashed, snapshotted and
    /// validated, and a won mission stays recorded but tainted.
    #[test]
    fn stub_results_and_hypotheses_taint_the_outcome() {
        // Initialize: n222(3) (result ignored); cv0 = n221(1); n178(3); cv1 = n253(27);
        // n20(0) (a stub without a result). Hourglass(t): cv2 = t. CheckVictoryCondition: 1.
        // PostInitialize: n30; n56(2); n31.
        let mut init = native(222, &[3], None, 0);
        init.extend(native(221, &[1], Some(cv(0)), 0));
        init.extend(native(178, &[3], None, 0));
        init.extend(native(253, &[27], Some(cv(1)), 0));
        init.extend(native(20, &[0], None, 0));
        let hourglass = vec![Instr::LoadParam {
            dst: cv(2),
            index: 0,
        }];
        let victory = vec![
            Instr::LoadInt {
                dst: tv(0),
                value: 1,
            },
            Instr::SetResult { src: tv(0) },
        ];
        let mut post = native(30, &[], None, 0);
        post.extend(native(56, &[2], None, 0));
        post.extend(native(31, &[], None, 0));
        let level = class(
            "StartUp",
            3,
            &[
                ("Initialize", 0, false, 0, 4, init),
                ("Hourglass", 1, false, 0, 4, hourglass.clone()),
                ("CheckVictoryCondition", 0, true, 0, 4, victory),
                ("PostInitialize", 0, false, 0, 4, post),
            ],
        );
        let mut w = mission_world(1, Some(program(vec![level], 1)));
        let vm = w.vm.as_ref().unwrap();
        assert!(!vm.faulted() && vm.counters.traps == 0);
        assert_eq!(vm.class_vars[0], vec![0, 1, 0]);
        // After load: every stub called is recorded, read or not (20, 178 and 222 are effect
        // stubs whose result, if any, was not consumed; 221 and 253 were consumed); the wait
        // of PostInitialize ran at load under the tick-rate reading.
        assert_eq!(
            vm.assumptions.iter().copied().collect::<Vec<_>>(),
            vec![
                Assumption::StubResult(20),
                Assumption::StubResult(178),
                Assumption::StubResult(221),
                Assumption::StubResult(222),
                Assumption::StubResult(253),
                Assumption::TickRate
            ]
        );
        let obs = w.script_observation().unwrap();
        assert!(obs.tainted);
        assert_eq!(obs.assumptions.len(), 6);
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert!(vm.mission_won && vm.tainted(), "won, but not authoritative");
        // Hashed, snapshotted, validated; the app's own assumptions go through the world.
        let h = w.hashes();
        let mut v = w.clone();
        v.record_assumption(Assumption::CampaignGraph);
        assert!(
            v.vm.as_ref()
                .unwrap()
                .assumptions
                .contains(&Assumption::CampaignGraph)
        );
        assert_ne!(v.hashes().get("scripts"), h.get("scripts"));
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        assert!(
            json.contains("\"assumptions\":[{\"stub_result\":20},{\"stub_result\":178},"),
            "{json}"
        );
        assert!(json.contains("\"tick_rate\""));
        let snap: crate::world::Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = mission_world(1, None);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.hashes(), h);
        assert!(w2.script_observation().unwrap().tainted);
        for id in [3, 999] {
            let mut bad = w.snapshot(None);
            bad.world
                .vm
                .as_mut()
                .unwrap()
                .assumptions
                .insert(Assumption::StubResult(id));
            assert!(w2.restore(&bad).unwrap_err().contains("not a stub"));
        }
        // The Hourglass time alone taints, on the first tick that reads it; a script that reads
        // nothing of the kind stays clean.
        let level = class("StartUp", 3, &[("Hourglass", 1, false, 0, 4, hourglass)]);
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        assert!(!w.script_observation().unwrap().tainted);
        w.step(&[]);
        assert_eq!(
            w.script_observation().unwrap().assumptions,
            vec![Assumption::TickRate]
        );
        let level = class(
            "StartUp",
            1,
            &[("Hourglass", 1, false, 0, 4, native(2, &[0], Some(cv(0)), 0))],
        );
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        for _ in 0..5 {
            w.step(&[]);
        }
        assert!(!w.script_observation().unwrap().tainted);
        // A world without a script has nothing to taint.
        let mut plain = mission_world(0, None);
        plain.record_assumption(Assumption::CampaignGraph);
        assert!(plain.vm.is_none());
        plain.validate().unwrap();
    }

    /// Native signatures (`natives::NATIVE_SIGNATURES`): the trust boundary refuses a call
    /// site with the wrong argument count and a result read after a native without one, and
    /// the dispatcher traps a mismatch instead of defaulting the missing argument.
    #[test]
    fn native_arity_is_validated_and_never_defaults() {
        let with = |body: Vec<Instr>| {
            program(
                vec![class("StartUp", 1, &[("Initialize", 0, false, 0, 4, body)])],
                0,
            )
        };
        with(native(237, &[5], None, 0)).validate().unwrap();
        let err = with(vec![Instr::Native {
            id: 237,
            argc: 0,
            dst: None,
        }])
        .validate()
        .unwrap_err();
        assert!(
            err.contains("native 237") && err.contains("takes 1"),
            "{err}"
        );
        let err = with(native(3, &[1, 2], Some(cv(0)), 0))
            .validate()
            .unwrap_err();
        assert!(err.contains("native 3 with 2"), "{err}");
        let err = with(native(237, &[5], Some(cv(0)), 0))
            .validate()
            .unwrap_err();
        assert!(err.contains("237") && err.contains("has none"), "{err}");
        with(native(236, &[], Some(cv(0)), 0)).validate().unwrap();
        // The result slot is checked like any slot.
        let err = with(native(236, &[], Some(cv(7)), 0))
            .validate()
            .unwrap_err();
        assert!(err.contains("out of range"), "{err}");
        // Unknown ids carry no signature: any count passes validation (they trap or are
        // recorded at run time).
        with(native(999, &[1, 2, 3], Some(cv(0)), 0))
            .validate()
            .unwrap();
        // Dispatch: a mismatch traps and changes nothing; the right count acts.
        let mut w = mission_world(0, Some(with(native(237, &[5], None, 0))));
        assert_eq!(w.vm.as_ref().unwrap().money, 5);
        assert_eq!(w.native_call(237, &[]), None);
        assert_eq!(w.native_call(237, &[1, 2]), None);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.money, 5, "the missing argument did not default to 0");
        assert!(vm.faulted());
        assert_eq!(vm.counters.arity_mismatches.get(&237), Some(&2));
        assert_eq!(w.native_call(237, &[7]), Some(0));
        assert_eq!(w.vm.as_ref().unwrap().money, 7);
        // Injected past validation, the mismatch traps the callback where it stands.
        let mut w = mission_world(0, None);
        w.vm = Some(VmState::new(
            with(vec![
                Instr::Native {
                    id: 237,
                    argc: 0,
                    dst: None,
                },
                Instr::LoadInt {
                    dst: cv(0),
                    value: 1,
                },
            ]),
            vec![],
            9,
            false,
        ));
        assert_eq!(
            w.vm_callback(0, callbacks::INITIALIZE, &[]),
            Some(CallOutcome::Aborted)
        );
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.class_vars[0], vec![0]);
        assert_eq!(vm.counters.traps, 1);
        assert_eq!(vm.money, 0);
        assert_quiescent(&w);
    }

    /// An action change is delivered exactly once even when the tick that produced it had no
    /// budget left: it waits in the queue (snapshotted, hashed, validated) and reaches the
    /// handler on the next tick, before `Hourglass`; a class without a handler drops its
    /// changes as undeliverable.
    #[test]
    fn action_changes_survive_an_exhausted_tick_and_are_delivered_once() {
        use crate::ai::{AiState, actions};
        // Level Hourglass: spins on its first call only (cv0 marks it).
        let hourglass = vec![
            Instr::JumpIf {
                cond: cv(0),
                target: 4,
            },
            Instr::LoadInt {
                dst: cv(0),
                value: 1,
            },
            Instr::Jump { target: 3 },
        ];
        let level = class("StartUp", 1, &[("Hourglass", 1, false, 0, 1, hourglass)]);
        // Guard ActionChange(a, b): cv0 = b, cv1 = a, cv2 += 1.
        let body = vec![
            Instr::LoadParam {
                dst: cv(0),
                index: 1,
            },
            Instr::LoadParam {
                dst: cv(1),
                index: 0,
            },
            Instr::LoadInt {
                dst: tv(0),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::Add,
                dst: cv(2),
                a: cv(2),
                b: tv(0),
            },
        ];
        let mut guard = class("Guard", 3, &[("ActionChange", 2, false, 0, 4, body)]);
        guard.element = Some(1);
        let mut w = mission_world(1, Some(program(vec![level, guard], 1)));
        // The hero stands in the guard's cone (guard at (300, 300) facing +x).
        w.entities[0].x = Fixed::from_int(420);
        w.entities[0].y = Fixed::from_int(300);
        // Tick 0: Hourglass spins the budget away; the guard notices the hero in the
        // simulation and the change is queued, not delivered.
        w.step(&[]);
        assert_eq!(w.entities[1].ai_state, AiState::Noticed);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.budget, 0);
        assert_eq!(
            vm.pending_action_changes,
            vec![ActionChange {
                class: 1,
                previous: actions::IDLE as i32,
                new: actions::NOTICED as i32
            }]
        );
        assert_eq!(vm.class_vars[1], vec![0, 0, 0]);
        // The alert sequence the sighting started is recorded where it changed the guard's
        // state, before any handler ran (finding 1 of Codex review 9); the sighting itself is
        // inside the measured cone (a standing hero 120 px ahead) and records nothing; the
        // delivery adds the parameter-order hypothesis.
        assert!(
            !vm.assumptions.contains(&Assumption::SightCone)
                && vm.assumptions.contains(&Assumption::AlertPolicy)
                && !vm.assumptions.contains(&Assumption::ActionChangeOrder),
            "{:?}",
            vm.assumptions
        );
        w.validate().unwrap();
        // The queue is state: hashed, restored, validated.
        let h = w.hashes();
        let mut v = w.clone();
        v.vm.as_mut().unwrap().pending_action_changes.clear();
        assert_ne!(v.hashes().get("scheduler"), h.get("scheduler"));
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        let snap: crate::world::Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = mission_world(1, None);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.hashes(), h);
        let mut bad = w.snapshot(None);
        bad.world.vm.as_mut().unwrap().pending_action_changes[0].class = 7;
        assert!(w2.restore(&bad).unwrap_err().contains("action change"));
        // Tick 1: delivered once, before Hourglass, in both worlds.
        for world in [&mut w, &mut w2] {
            world.step(&[]);
            let vm = world.vm.as_ref().unwrap();
            assert_eq!(
                vm.class_vars[1],
                vec![actions::NOTICED as i32, actions::IDLE as i32, 1]
            );
            assert!(vm.pending_action_changes.is_empty());
            assert!(vm.assumptions.contains(&Assumption::ActionChangeOrder));
        }
        assert_eq!(w.hashes(), w2.hashes());
        // The alarm follows: one more delivery, never a repeat of the first.
        for _ in 0..crate::ai::NOTICED_TICKS {
            w.step(&[]);
        }
        assert_eq!(w.entities[1].ai_state, AiState::Alarm);
        assert_eq!(
            w.vm.as_ref().unwrap().class_vars[1],
            vec![actions::ALARM as i32, actions::NOTICED as i32, 2]
        );
        w.validate().unwrap();
        // A class without a handler: its changes are dropped as undeliverable and no handler
        // runs, but the alert sequence the sighting started is a hypothesis the engine took,
        // so the set names it and nothing else (the sighting is inside the measured cone).
        let level = class("StartUp", 0, &[]);
        let mut guard = class("Guard", 0, &[]);
        guard.element = Some(1);
        let mut w = mission_world(1, Some(program(vec![level, guard], 1)));
        w.entities[0].x = Fixed::from_int(420);
        w.entities[0].y = Fixed::from_int(300);
        w.step(&[]);
        assert_eq!(w.entities[1].ai_state, AiState::Noticed);
        let vm = w.vm.as_ref().unwrap();
        assert!(vm.pending_action_changes.is_empty());
        assert_eq!(
            vm.assumptions.iter().copied().collect::<Vec<_>>(),
            vec![Assumption::AlertPolicy]
        );
        w.validate().unwrap();
    }
    /// Finding 1 of Codex review 9: an engine hypothesis that mutates authoritative state
    /// taints a win that depends on it through no `ActionChange` handler at all. A hero runs
    /// 340 px behind a soldier: within the engine's noise radius (350) but beyond the measured
    /// bound (330); the soldier hears him and charges into the polygon zone, and the level's
    /// `CheckVictoryCondition` reads native 97 (observed) on him and returns 1. The guard's
    /// class has no handler, so nothing of the stealth layer ever reaches a script callback,
    /// yet the win is tainted by `noise_radius` from the tick of the charge on, through a JSON
    /// snapshot restored into a fresh world and a checkpoint every 50 ticks. The same scene with
    /// the soldier at 320 px (the measured band) records the hearing and the charge as measured
    /// and is tainted by `alert_timeout` alone: the five-second timeout and the return
    /// destination the charge stores are the hypothesis, recorded before the state changes
    /// (finding 1 of Codex review 10: the measured `NoiseCharge` and the hypothesised
    /// `AlertTimeout` are separate sources; no charge wins untainted). In both the hero
    /// crouches after his first running tick (by hand: the posture is state) so that the
    /// charging soldier neither hears him again nor sees him before the win (the crouched
    /// range is 125 px, the win happens 199 px away).
    #[test]
    fn a_charge_from_the_unmeasured_noise_band_taints_a_win_read_from_native_97() {
        use crate::ai::AiState;
        use crate::world::{Gait, Posture};
        let scene = |distance: i32| {
            // Elements: hero 0, guard 1; native 97 takes the zone as a location index: the
            // 400..600 square is location 1.
            let mut victory = native(97, &[1, 1], Some(tv(0)), 1);
            victory.push(Instr::SetResult { src: tv(0) });
            let level = class(
                "StartUp",
                0,
                &[("CheckVictoryCondition", 0, true, 0, 4, victory)],
            );
            let mut guard = class("Guard", 0, &[]);
            guard.element = Some(1);
            let mut w = mission_world(1, Some(program(vec![level, guard], 1)));
            // The hero at the far end of the zone running east, the guard `distance` px above
            // him facing away (up).
            w.entities[0].x = Fixed::from_int(500);
            w.entities[0].y = Fixed::from_int(599);
            w.entities[0].target = Some((Fixed::from_int(560), Fixed::from_int(599)));
            w.entities[0].gait = Gait::Run;
            w.entities[1].x = Fixed::from_int(500);
            w.entities[1].y = Fixed::from_int(599 - distance);
            w.entities[1].facing256 = 192;
            w.validate().unwrap();
            w
        };
        let mut w = scene(340);
        assert!(w.vm.as_ref().unwrap().assumptions.is_empty());
        w.step(&[]);
        let g = &w.entities[1];
        assert_eq!(g.ai_state, AiState::Alerted);
        assert!(g.heard && g.target.is_some());
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(
            vm.assumptions.iter().copied().collect::<Vec<_>>(),
            vec![Assumption::NoiseRadius, Assumption::AlertTimeout],
            "the charge itself is measured; hearing from 340 px and the timeout it stores are not"
        );
        assert!(
            vm.pending_action_changes.is_empty(),
            "no handler: nothing queued"
        );
        assert!(!vm.mission_won);
        w.entities[0].posture = Posture::Crouched;
        // The checkpoint: a JSON snapshot restored into a fresh world replays to the same win.
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        let snap: crate::world::Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = mission_world(1, None);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.hashes(), w.hashes());
        let mut ticks = 0;
        while !w.vm.as_ref().unwrap().mission_won {
            w.step(&[]);
            w2.step(&[]);
            ticks += 1;
            if ticks % 50 == 0 {
                assert_eq!(w.hashes(), w2.hashes(), "checkpoint at {ticks}");
            }
            assert!(ticks < 600, "the guard never reached the zone");
        }
        assert!(w2.vm.as_ref().unwrap().mission_won);
        assert_eq!(w.hashes(), w2.hashes());
        let obs = w.script_observation().unwrap();
        assert!(obs.mission_won && obs.tainted);
        assert_eq!(
            obs.assumptions,
            vec![Assumption::NoiseRadius, Assumption::AlertTimeout],
            "no handler ran, nothing else was taken"
        );
        assert_taint_round_trips(&w, &obs.assumptions);
        // The measured band: the same charge, the same win; the hearing records nothing, the
        // timeout the charge stores is the only hypothesis taken.
        let mut w = scene(320);
        w.step(&[]);
        assert!(w.entities[1].heard);
        assert_eq!(
            w.vm.as_ref()
                .unwrap()
                .assumptions
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![Assumption::AlertTimeout]
        );
        w.entities[0].posture = Posture::Crouched;
        let mut ticks = 0;
        while !w.vm.as_ref().unwrap().mission_won {
            w.step(&[]);
            ticks += 1;
            assert!(ticks < 600);
        }
        assert_eq!(w.entities[1].ai_state, AiState::Alerted);
        let obs = w.script_observation().unwrap();
        assert!(obs.mission_won && obs.tainted, "{:?}", obs.assumptions);
        assert_taint_round_trips(&w, &[Assumption::AlertTimeout]);
    }

    /// Finding 3 of Codex review 9: a script call and its result read are one instruction. The
    /// callee's value reaches the call's destination when it returns; a program that reads the
    /// result of a function without one, or calls a function that does not exist, fails
    /// `validate` (a snapshot embedding it is refused), and a snapshot that still carries the
    /// old standalone reader does not deserialise at all.
    #[test]
    fn call_results_are_fused_into_the_call_and_validated() {
        // Initialize: cv0 = seven(); void(): sets its result anyway (hostile), read by nobody.
        let init = vec![
            Instr::Call {
                function: 1,
                argc: 0,
                dst: Some(cv(0)),
            },
            Instr::Nop,
            Instr::Call {
                function: 2,
                argc: 0,
                dst: None,
            },
        ];
        let seven = vec![
            Instr::LoadInt {
                dst: tv(0),
                value: 7,
            },
            Instr::SetResult { src: tv(0) },
        ];
        let void = vec![
            Instr::LoadInt {
                dst: tv(0),
                value: 5,
            },
            Instr::SetResult { src: tv(0) },
        ];
        let level = class(
            "StartUp",
            2,
            &[
                ("Initialize", 0, false, 0, 1, init.clone()),
                ("seven", 0, true, 0, 1, seven.clone()),
                ("void", 0, false, 0, 1, void.clone()),
            ],
        );
        let base = program(vec![level], 0);
        base.validate().unwrap();
        let w = mission_world(0, Some(base.clone()));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.class_vars[0], vec![7, 0]);
        assert_eq!(vm.counters.faults, 0);
        assert_quiescent(&w);
        // Reading the void function's result is refused, in the program and in a snapshot.
        let mut hostile = base.clone();
        hostile.classes[0].code[3] = Instr::Call {
            function: 2,
            argc: 0,
            dst: Some(cv(1)),
        };
        let err = hostile.validate().unwrap_err();
        assert!(err.contains("reads the result of function 2"), "{err}");
        let mut w2 = mission_world(0, None);
        let mut snap = w.snapshot(None);
        snap.world.vm.as_mut().unwrap().program = hostile.clone();
        snap.world.vm.as_mut().unwrap().program_digest = hostile.digest();
        let err = w2.restore(&snap).unwrap_err();
        assert!(err.contains("reads the result"), "{err}");
        // Injected past validation the fabricated value is what the call carries, never a
        // frame's stale scratch: the trust boundary is `validate`, which the world fails.
        let mut w3 = mission_world(0, None);
        w3.vm = Some(VmState::new(hostile, vec![], 9, false));
        assert!(w3.validate().is_err());
        // The old standalone reader is not an instruction any more: a snapshot carrying it
        // does not deserialise.
        let mut json = serde_json::to_value(w.snapshot(None)).unwrap();
        json["world"]["vm"]["program"]["classes"][0]["code"][2] =
            serde_json::json!({"get_call_result": {"dst": {"space": "class", "index": 0}}});
        assert!(serde_json::from_value::<crate::world::Snapshot>(json).is_err());
        // A missing callee is refused too.
        let mut missing = base;
        missing.classes[0].code[1] = Instr::Call {
            function: 9,
            argc: 0,
            dst: None,
        };
        assert!(missing.validate().unwrap_err().contains("does not exist"));
    }

    /// A level whose `Initialize` is `init` and whose `CheckVictoryCondition` returns 1 at
    /// once: the win depends on whatever `init` executed.
    fn win_after(init: Vec<Instr>, guards: usize, lenient: bool) -> World {
        let victory = vec![
            Instr::LoadInt {
                dst: tv(0),
                value: 1,
            },
            Instr::SetResult { src: tv(0) },
        ];
        let level = class(
            "StartUp",
            2,
            &[
                ("Initialize", 0, false, 0, 4, init),
                ("CheckVictoryCondition", 0, true, 0, 4, victory),
            ],
        );
        mission_world_with(guards, Some(program(vec![level], guards as u32)), lenient)
    }

    /// The taint survives a JSON snapshot restored into a fresh world (same hashes, same
    /// observation) and a snapshot taken after the win.
    fn assert_taint_round_trips(w: &World, expected: &[Assumption]) {
        let obs = w.script_observation().unwrap();
        assert_eq!(obs.assumptions, expected);
        assert_eq!(obs.tainted, !expected.is_empty());
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        let snap: crate::world::Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = mission_world(0, None);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.hashes(), w.hashes());
        assert_eq!(w2.script_observation().unwrap().assumptions, expected);
    }

    /// Finding 1 of Codex review 8: every source of the registry taints a win that branches
    /// on it, on the point where the hypothesis is taken (the call, the instruction), whether
    /// or not a value is read; a program over observed natives and presentation stubs only
    /// wins untainted.
    #[test]
    fn every_hypothesis_source_taints_a_win_that_depends_on_it() {
        let binary = |op: BinOp| {
            vec![
                Instr::LoadInt {
                    dst: tv(0),
                    value: 3,
                },
                Instr::LoadInt {
                    dst: tv(1),
                    value: 2,
                },
                Instr::Binary {
                    op,
                    dst: cv(0),
                    a: tv(0),
                    b: tv(1),
                },
            ]
        };
        let cases: Vec<(&str, Vec<Instr>, bool, Vec<Assumption>)> = vec![
            (
                "0x24",
                binary(BinOp::GeLow),
                false,
                vec![Assumption::Opcode(0x24)],
            ),
            (
                "0x28",
                binary(BinOp::Ne),
                false,
                vec![Assumption::Opcode(0x28)],
            ),
            (
                "0x2b",
                binary(BinOp::FixedLt),
                false,
                vec![Assumption::Opcode(0x2b)],
            ),
            ("0x26 is medium", binary(BinOp::Ge), false, vec![]),
            (
                "0x14",
                vec![Instr::LoadFixed {
                    dst: cv(0),
                    value: Fixed::from_int(1),
                }],
                false,
                vec![Assumption::Opcode(0x14)],
            ),
            (
                "0xffff",
                vec![Instr::LeaveUnresolved],
                false,
                vec![Assumption::UnresolvedJump],
            ),
            (
                "policy 128",
                native(128, &[0], Some(cv(0)), 0),
                false,
                vec![Assumption::Policy(128)],
            ),
            (
                "policy 98",
                native(98, &[0, -1], Some(cv(0)), 0),
                false,
                vec![Assumption::Policy(98)],
            ),
            (
                "policy 140",
                native(140, &[1, 1], None, 0),
                false,
                vec![Assumption::Policy(140)],
            ),
            (
                "policy 245",
                native(245, &[], Some(cv(0)), 0),
                false,
                vec![Assumption::Policy(245)],
            ),
            (
                "policy 161",
                native(161, &[10], Some(cv(0)), 0),
                false,
                vec![Assumption::Policy(161)],
            ),
            (
                "lenient unknown",
                native(999, &[1], Some(cv(0)), 0),
                true,
                vec![Assumption::UnknownNative(999)],
            ),
            (
                "effect stub without a result",
                native(186, &[4, 1], None, 0),
                false,
                vec![Assumption::StubResult(186)],
            ),
            (
                "effect stub collected in a sequence",
                {
                    let mut v = native(30, &[], None, 0);
                    v.extend(native(49, &[1, 5], None, 0));
                    v.extend(native(31, &[], None, 0));
                    v
                },
                false,
                vec![Assumption::StubResult(49)],
            ),
            (
                "stub result consumed",
                native(221, &[1], Some(cv(0)), 0),
                false,
                vec![Assumption::StubResult(221)],
            ),
            (
                "presentation stubs",
                {
                    let mut v = native(149, &[3], None, 0);
                    v.extend(native(243, &[1], None, 0));
                    v.extend(native(69, &[1, 61], None, 0));
                    v
                },
                false,
                vec![],
            ),
            (
                "branch 240 on a non-actor element",
                native(240, &[2], Some(cv(0)), 0),
                false,
                vec![Assumption::Policy(240)],
            ),
            (
                "branch 240 on an actor",
                native(240, &[1], Some(cv(0)), 0),
                false,
                vec![],
            ),
            (
                "211 with a single player character",
                native(211, &[], Some(cv(0)), 0),
                false,
                vec![],
            ),
            (
                "56 outside a sequence",
                native(56, &[10], None, 0),
                false,
                vec![Assumption::TickRate],
            ),
            (
                "observed natives only",
                {
                    let mut v = native(1, &[3, 4], None, 0);
                    v.extend(native(2, &[3], Some(cv(0)), 0));
                    v.extend(native(113, &[1], None, 0));
                    v.extend(native(160, &[0, 0], Some(cv(1)), 0));
                    v
                },
                false,
                vec![],
            ),
        ];
        for (name, init, lenient, expected) in cases {
            let mut w = win_after(init, 1, lenient);
            let vm = w.vm.as_ref().unwrap();
            assert!(!vm.faulted(), "{name}");
            assert_eq!(
                vm.assumptions.iter().copied().collect::<Vec<_>>(),
                expected,
                "{name}: after load"
            );
            w.step(&[]);
            let vm = w.vm.as_ref().unwrap();
            assert!(vm.mission_won, "{name}");
            assert_eq!(vm.tainted(), !expected.is_empty(), "{name}");
            assert_taint_round_trips(&w, &expected);
        }
        // Which player character is the main one is a policy only with several of them.
        let init = native(211, &[], Some(cv(0)), 0);
        let mut spec = two_heroes_spec();
        let level = class("StartUp", 1, &[("Initialize", 0, false, 0, 4, init)]);
        spec.script = Some(Program {
            classes: vec![level],
            elements: vec![Element::Actor(0), Element::Actor(1)],
            locations: vec![Location::Point { x: 200, y: 200 }],
            wait_scale: (2, 1),
        });
        let w = World::new_mission(Scenario::Mission("T".into()), 9, &spec).unwrap();
        assert_taint_round_trips(&w, &[Assumption::Policy(211)]);
        // Malformed assumptions are refused by `validate`.
        let mut w = win_after(vec![], 0, false);
        w.step(&[]);
        let h = w.hashes();
        for (bad, needle) in [
            (Assumption::Policy(3), "not a policy"),
            (Assumption::Policy(999), "not a policy"),
            (Assumption::StubResult(3), "not a stub"),
            (Assumption::Opcode(0x19), "not of low confidence"),
            (Assumption::UnknownNative(3), "which is known"),
            (Assumption::UnknownNative(999), "without lenient"),
        ] {
            let mut snap = w.snapshot(None);
            snap.world.vm.as_mut().unwrap().assumptions.insert(bad);
            let err = w.restore(&snap).unwrap_err();
            assert!(err.contains(needle), "{bad:?}: {err}");
        }
        assert_eq!(w.hashes(), h);
    }

    /// A mission with two player characters at (100, 100) and (150, 100).
    fn two_heroes_spec() -> MissionSpec {
        let hero = |x: i32| ActorSpec {
            profile: "RobinHood".into(),
            team: Team::Player,
            x,
            y: 100,
            facing256: 0,
            patrol: vec![],
            program: vec![],
            active: true,
            hit_points: 100,
            knockout_resistance: 0,
        };
        MissionSpec {
            map: MapInfo {
                width: 1000,
                height: 800,
            },
            geometry: Geometry {
                boundary: vec![(0, 0), (1000, 0), (1000, 800), (0, 800)],
                obstacles: vec![],
                areas: Vec::new(),
            },
            actors: vec![hero(100), hero(150)],
            script: None,
            rails: Vec::new(),
            lenient_natives: false,
            starting_money: 0,
            assumptions: BTreeSet::new(),
        }
    }

    /// The engine's own hypotheses record their source when they change script-visible
    /// state: a scroll pickup, a zone entered by a character standing inside at load, a
    /// barrier released by a walk that did not arrive, and (in
    /// `action_changes_reach_the_actors_class`) the `ActionChange` parameter order.
    #[test]
    fn engine_hypotheses_record_their_source() {
        // A scroll taken by its handler's non-zero result: the hero, 60 px north of the
        // scroll (element 1 without guards), is ordered onto it.
        let taken = vec![
            Instr::LoadInt {
                dst: tv(0),
                value: 1,
            },
            Instr::SetResult { src: tv(0) },
        ];
        let level = class("StartUp", 0, &[]);
        let mut scroll = class("Scroll", 0, &[("IsTaken", 1, true, 0, 4, taken)]);
        scroll.element = Some(1);
        let mut w = mission_world(0, Some(program(vec![level, scroll], 0)));
        w.entities[0].x = Fixed::from_int(700);
        w.entities[0].y = Fixed::from_int(640);
        click_at(&mut w, 700, 640);
        click_at(&mut w, 700, 695);
        assert_eq!(w.entities[0].pickup, Some(1));
        steps_until_resolved(&mut w, 600);
        assert!(w.vm.as_ref().unwrap().inactive_elements.contains(&1));
        assert_taint_round_trips(&w, &[Assumption::ScrollPickup]);
        // A zone entered on the first scan by a character standing inside at load; the same
        // zone entered later records nothing of the kind.
        let level = class("StartUp", 0, &[]);
        let mut zone = class("Zone", 1, &[("EnterZone", 1, false, 0, 4, vec![])]);
        zone.zone = Some(1);
        let inside = |x: i32| {
            let mut w = mission_world(0, Some(program(vec![level.clone(), zone.clone()], 0)));
            w.entities[0].x = Fixed::from_int(x);
            w.entities[0].y = Fixed::from_int(500);
            w
        };
        let mut w = inside(500);
        w.step(&[]);
        assert!(w.vm.as_ref().unwrap().zone_presence.contains(&(1, 0)));
        assert_taint_round_trips(&w, &[Assumption::ZoneAtLoad]);
        let mut w = inside(100);
        for _ in 0..3 {
            w.step(&[]);
        }
        w.entities[0].x = Fixed::from_int(500);
        w.step(&[]);
        assert!(w.vm.as_ref().unwrap().zone_presence.contains(&(1, 0)));
        assert_taint_round_trips(&w, &[]);
        // A walk in a sequence: the guard is deactivated on tick 1, the barrier releases on
        // the completion hypothesis and the text after it appears.
        let mut post = native(30, &[], None, 0);
        post.extend(native(45, &[1, 0, 0], None, 0));
        post.extend(native(32, &[], None, 0));
        post.extend(native(203, &[5], None, 0));
        post.extend(native(31, &[], None, 0));
        let hourglass = vec![
            Instr::LoadParam {
                dst: tv(0),
                index: 0,
            },
            Instr::LoadInt {
                dst: tv(1),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::Eq,
                dst: tv(2),
                a: tv(0),
                b: tv(1),
            },
            Instr::JumpIf {
                cond: tv(2),
                target: 6,
            },
            Instr::Return,
            Instr::LoadInt {
                dst: tv(0),
                value: 1,
            },
            Instr::PushArg { src: tv(0) },
            Instr::Native {
                id: 113,
                argc: 1,
                dst: None,
            },
        ];
        // Hourglass first: its jump target is an index of its own code.
        let level = class(
            "StartUp",
            0,
            &[
                ("Hourglass", 1, false, 0, 4, hourglass),
                ("PostInitialize", 0, false, 0, 4, post.clone()),
            ],
        );
        let mut w = mission_world(1, Some(program(vec![level], 1)));
        // The hero stands far away: nothing but the script moves the guard.
        w.entities[0].x = Fixed::from_int(900);
        w.entities[0].y = Fixed::from_int(700);
        assert!(w.entities[1].target.is_some(), "the walk started at load");
        for _ in 0..3 {
            w.step(&[]);
        }
        let vm = w.vm.as_ref().unwrap();
        assert!(!w.entities[1].active);
        assert_eq!(vm.pending_texts(), vec![5], "the barrier released");
        assert!(vm.assumptions.contains(&Assumption::WalkCompletion));
        assert!(vm.assumptions.contains(&Assumption::TickRate), "Hourglass");
        w.validate().unwrap();
        // The same walk left to arrive records no completion hypothesis.
        let level = class("StartUp", 0, &[("PostInitialize", 0, false, 0, 4, post)]);
        let mut w = mission_world(1, Some(program(vec![level], 1)));
        w.entities[0].x = Fixed::from_int(900);
        w.entities[0].y = Fixed::from_int(700);
        let mut ticks = 0;
        while w.vm.as_ref().unwrap().pending_texts().is_empty() {
            w.step(&[]);
            ticks += 1;
            assert!(ticks < 1000, "never arrived");
        }
        assert_eq!(
            (w.entities[1].x.round(), w.entities[1].y.round()),
            (200, 200)
        );
        assert!(
            !w.vm
                .as_ref()
                .unwrap()
                .assumptions
                .contains(&Assumption::WalkCompletion)
        );
    }

    /// The guard classes and world of the transaction tests: `guards` soldiers in the hero's
    /// sight (the hero at (420, 300), the guards at (300 + 100 i, 300) facing +x), each with
    /// an `ActionChange` handler `body` on 3 class variables.
    fn noticing_world(level: Class, body: &[Instr], guards: usize) -> World {
        let mut classes = vec![level];
        for g in 0..guards {
            let mut guard = class(
                &format!("Guard{g}"),
                3,
                &[("ActionChange", 2, false, 0, 4, body.to_vec())],
            );
            guard.element = Some(1 + g as u32);
            classes.push(guard);
        }
        let mut w = mission_world(guards, Some(program(classes, guards as u32)));
        w.entities[0].x = Fixed::from_int(420);
        w.entities[0].y = Fixed::from_int(300);
        w
    }

    /// Finding 3 of Codex review 8: a queued handler the budget cuts short is rolled back
    /// (class variables, money, mission variables, the entity a native moved) and retried
    /// whole on the next tick, so no effect is applied twice; the rollback is deterministic
    /// across a snapshot.
    #[test]
    fn a_queued_handler_cut_short_is_rolled_back_and_retried_whole() {
        // ActionChange: cv2 += 1; n237(77); n96(hero, location 0); n1(3, 9); spin.
        let mut body = vec![
            Instr::LoadInt {
                dst: tv(0),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::Add,
                dst: cv(2),
                a: cv(2),
                b: tv(0),
            },
        ];
        body.extend(native(237, &[77], None, 0));
        body.extend(native(96, &[0, 0], None, 0));
        body.extend(native(1, &[3, 9], None, 0));
        let spin = 1 + body.len() as u32;
        body.push(Instr::Jump { target: spin });
        let level = class("StartUp", 0, &[]);
        let mut w = noticing_world(level, &body, 1);
        let unchanged = |w: &World, tick: u64| {
            let vm = w.vm.as_ref().unwrap();
            assert_eq!(
                vm.class_vars[1][2], 0,
                "tick {tick}: the increment was rolled back"
            );
            assert_eq!(vm.money, 0, "tick {tick}: the money was rolled back");
            assert_eq!(vm.mission_vars[3], 0, "tick {tick}");
            assert_eq!(
                (w.entities[0].x.round(), w.entities[0].y.round()),
                (420, 300),
                "tick {tick}: the teleport was rolled back"
            );
            assert_eq!(
                vm.pending_action_changes.len(),
                1,
                "tick {tick}: retried whole"
            );
            assert!(vm.transaction.is_none());
        };
        w.step(&[]);
        assert_eq!(w.entities[1].ai_state, crate::ai::AiState::Noticed);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.counters.transactions_rolled_back, 1);
        assert_eq!(vm.budget, 0);
        assert!(vm.assumptions.contains(&Assumption::ActionChangeOrder));
        unchanged(&w, 0);
        w.validate().unwrap();
        let snap = w.snapshot(None);
        for t in 1..5 {
            w.step(&[]);
            unchanged(&w, t);
            assert_eq!(
                w.vm.as_ref().unwrap().counters.transactions_rolled_back,
                1 + t
            );
        }
        // Deterministic from the snapshot.
        let mut w2 = mission_world(0, None);
        w2.restore(&snap).unwrap();
        for _ in 1..5 {
            w2.step(&[]);
        }
        assert_eq!(w2.hashes(), w.hashes());
        assert_quiescent(&w);
    }

    /// A backlog held back by spinning message handlers survives a snapshot taken in the
    /// middle of it: after the restore both worlds deliver every change exactly once, on the
    /// same tick, with the same hashes.
    #[test]
    fn a_backlog_of_action_changes_restores_mid_way_and_delivers_once() {
        // Level: Initialize sends message 1 to the hero (the level handles it);
        // ProcessMessage: while cv0 < 3 { cv0 += 1; resend; spin }.
        let init = native(43, &[0, 1], None, 0);
        let mut handler = vec![
            Instr::LoadInt {
                dst: tv(0),
                value: 3,
            },
            Instr::Binary {
                op: BinOp::Lt,
                dst: tv(1),
                a: cv(0),
                b: tv(0),
            },
            Instr::JumpIf {
                cond: tv(1),
                target: 5,
            },
            Instr::Return,
            Instr::LoadInt {
                dst: tv(0),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::Add,
                dst: cv(0),
                a: cv(0),
                b: tv(0),
            },
        ];
        handler.extend(native(43, &[0, 1], None, 0));
        let spin = 1 + handler.len() as u32;
        handler.push(Instr::Jump { target: spin });
        // The handler comes first in the table so that its jump targets are its own indices.
        let level = class(
            "StartUp",
            1,
            &[
                ("ProcessMessage", 3, false, 0, 4, handler),
                ("Initialize", 0, false, 0, 4, init),
            ],
        );
        // Guards: ActionChange: cv0 += 1.
        let body = vec![
            Instr::LoadInt {
                dst: tv(0),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::Add,
                dst: cv(0),
                a: cv(0),
                b: tv(0),
            },
        ];
        let mut w = noticing_world(level, &body, 2);
        // Ticks 0 and 1: the message spins the budget away, both guards notice on tick 0 and
        // their changes wait.
        w.step(&[]);
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.class_vars[0][0], 2);
        assert_eq!(vm.pending_action_changes.len(), 2, "both changes wait");
        assert_eq!(vm.class_vars[1][0] + vm.class_vars[2][0], 0);
        assert_eq!(vm.counters.transactions_rolled_back, 0, "never started");
        w.validate().unwrap();
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        let snap: crate::world::Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = mission_world(0, None);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.vm.as_ref().unwrap().pending_action_changes.len(), 2);
        // Tick 2 spins once more; on tick 3 the handler returns and the backlog is delivered,
        // once per change, in both worlds.
        for world in [&mut w, &mut w2] {
            world.step(&[]);
            assert_eq!(world.vm.as_ref().unwrap().pending_action_changes.len(), 2);
            world.step(&[]);
            let vm = world.vm.as_ref().unwrap();
            assert!(vm.pending_action_changes.is_empty());
            assert_eq!((vm.class_vars[1][0], vm.class_vars[2][0]), (1, 1));
        }
        assert_eq!(w.hashes(), w2.hashes());
        for _ in 0..5 {
            w.step(&[]);
            w2.step(&[]);
        }
        assert_eq!(w.hashes(), w2.hashes());
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(
            (vm.class_vars[1][0], vm.class_vars[2][0]),
            (1, 1),
            "never twice"
        );
    }

    /// A full action change queue is a deterministic, sticky, hashed fault
    /// (`Fault::ActionQueueOverflow`), never a silent drop.
    #[test]
    fn a_full_action_change_queue_faults_the_script() {
        let level = class("StartUp", 0, &[]);
        let mut guard = class("Guard", 0, &[("ActionChange", 2, false, 0, 4, vec![])]);
        guard.element = Some(1);
        let mut w = mission_world(1, Some(program(vec![level, guard], 1)));
        // Spend the tick's budget so nothing is delivered while the queue fills.
        w.vm.as_mut().unwrap().budget = 0;
        for _ in 0..MAX_QUEUE {
            w.vm_queue_action_change(1, 0, 6);
        }
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.pending_action_changes.len(), MAX_QUEUE);
        assert!(!vm.faulted());
        let before = w.hashes();
        w.vm_queue_action_change(1, 6, 0);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.fault, Some(Fault::ActionQueueOverflow));
        assert!(vm.faulted());
        assert_eq!(vm.pending_action_changes.len(), MAX_QUEUE);
        assert_ne!(w.hashes().get("scripts"), before.get("scripts"));
        assert!(w.script_observation().unwrap().faulted);
        // Sticky: a later trap keeps the first fault; the fault survives a snapshot.
        w.native_call(999, &[]);
        assert_eq!(
            w.vm.as_ref().unwrap().fault,
            Some(Fault::ActionQueueOverflow)
        );
        w.validate().unwrap();
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        assert!(
            json.contains("\"fault\":\"action_queue_overflow\""),
            "{json}"
        );
        let snap: crate::world::Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = mission_world(0, None);
        w2.restore(&snap).unwrap();
        assert_eq!(
            w2.vm.as_ref().unwrap().fault,
            Some(Fault::ActionQueueOverflow)
        );
        assert_eq!(w2.hashes(), w.hashes());
        // The other faults name their native.
        let mut w = mission_world(0, Some(program(vec![class("StartUp", 0, &[])], 0)));
        assert_eq!(w.native_call(237, &[]), None);
        assert_eq!(
            w.vm.as_ref().unwrap().fault,
            Some(Fault::ArityMismatch(237))
        );
        let mut w = mission_world(0, Some(program(vec![class("StartUp", 0, &[])], 0)));
        assert_eq!(w.native_call(999, &[]), None);
        assert_eq!(
            w.vm.as_ref().unwrap().fault,
            Some(Fault::UnknownNative(999))
        );
    }

    /// Finding 3 of Codex review 10: a validated recursive `CheckVictoryCondition` that writes
    /// 1 to a slot and then calls itself into that slot must not win through the frame limit.
    /// The deepest call that cannot push its frame faults the script
    /// (`Fault::CallStackOverflow`: sticky, hashed, restored from a snapshot) and aborts the
    /// callback where it stands, so the slot never unwinds as a result; the same recursion in
    /// a queued `ActionChange` handler is rolled back (its class variable untouched) and the
    /// change consumed rather than retried.
    #[test]
    fn a_frame_limit_overflow_faults_the_script_and_never_fabricates_a_result() {
        // f: t0 = 1; t0 = Recurse(); return t0.
        let body = |callee: u32| {
            vec![
                Instr::LoadInt {
                    dst: tv(0),
                    value: 1,
                },
                Instr::Call {
                    function: callee,
                    argc: 0,
                    dst: Some(tv(0)),
                },
                Instr::SetResult { src: tv(0) },
            ]
        };
        let level = class(
            "StartUp",
            0,
            &[
                ("CheckVictoryCondition", 0, true, 0, 1, body(1)),
                ("Recurse", 0, true, 0, 1, body(1)),
            ],
        );
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        assert!(!w.vm.as_ref().unwrap().faulted());
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert!(
            !vm.mission_won && !vm.mission_lost,
            "no fabricated 1 unwound"
        );
        assert_eq!(vm.fault, Some(Fault::CallStackOverflow));
        assert!(vm.faulted());
        assert!(
            vm.frames.is_empty() && vm.param_stack.is_empty(),
            "quiescent"
        );
        assert_eq!(vm.counters.faults, 1);
        let obs = w.script_observation().unwrap();
        assert!(obs.faulted && !obs.mission_won);
        // Hashed (under `scripts`), sticky across ticks and a later trap, restored intact.
        let mut v = w.clone();
        v.vm.as_mut().unwrap().fault = None;
        assert_ne!(v.hashes().get("scripts"), w.hashes().get("scripts"));
        w.validate().unwrap();
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        assert!(json.contains(CALL_STACK_OVERFLOW_JSON), "{json}");
        let snap: crate::world::Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = mission_world(0, None);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.hashes(), w.hashes());
        for _ in 0..5 {
            w.step(&[]);
            w2.step(&[]);
        }
        assert_eq!(w.hashes(), w2.hashes());
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.fault, Some(Fault::CallStackOverflow));
        assert!(!vm.mission_won && !w2.vm.as_ref().unwrap().mission_won);
        w.native_call(999, &[]);
        assert_eq!(
            w.vm.as_ref().unwrap().fault,
            Some(Fault::CallStackOverflow),
            "the first fault is kept"
        );
        // The same recursion in a queued handler: cv0 = 1 is rolled back with the rest of
        // the handler's effects, the change is consumed (it would fail the same way again).
        let level = class("StartUp", 0, &[]);
        let mut handler = vec![Instr::LoadInt {
            dst: cv(0),
            value: 1,
        }];
        handler.extend(body(1));
        let mut guard = class(
            "Guard",
            1,
            &[
                ("ActionChange", 2, false, 0, 1, handler),
                ("Recurse", 0, true, 0, 1, body(1)),
            ],
        );
        guard.element = Some(1);
        let mut w = mission_world(1, Some(program(vec![level, guard], 1)));
        w.vm_queue_action_change(1, 0, 6);
        w.vm_deliver_action_changes();
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.fault, Some(Fault::CallStackOverflow));
        assert_eq!(vm.class_vars[1], vec![0], "rolled back");
        assert!(
            vm.pending_action_changes.is_empty(),
            "consumed, not retried"
        );
        assert_eq!(vm.counters.transactions_rolled_back, 1);
        assert!(vm.transaction.is_none());
        w.validate().unwrap();
    }

    /// The JSON form of `Fault::CallStackOverflow` in a snapshot.
    const CALL_STACK_OVERFLOW_JSON: &str = "\"fault\":\"call_stack_overflow\"";

    /// Finding 6 of Codex review 8: the native call and its result read are one instruction,
    /// so no jump can reach a result read without its call: a jump that lands after a fused
    /// native leaves the destination slot untouched, and a result slot is validated like any
    /// slot and refused on a native that leaves no value (`native_arity_is_validated_and_never_defaults`).
    #[test]
    fn a_jump_past_a_fused_native_reads_no_result() {
        // Initialize: cv0 = 5; if cv1 goto L; cv0 = n236(); L: (the Nop of the fused read).
        let init = vec![
            Instr::LoadInt {
                dst: cv(0),
                value: 5,
            },
            Instr::JumpIf {
                cond: cv(1),
                target: 4,
            },
            Instr::Native {
                id: 236,
                argc: 0,
                dst: Some(cv(0)),
            },
            Instr::Nop,
        ];
        let level = class("StartUp", 2, &[("Initialize", 0, false, 0, 4, init)]);
        let mut p = program(vec![level], 0);
        p.validate().unwrap();
        let w = mission_world(0, Some(p.clone()));
        assert_eq!(
            w.vm.as_ref().unwrap().class_vars[0],
            vec![0, 0],
            "the call ran: money 0"
        );
        // With the branch taken (cv1 set before Initialize by hand), the slot keeps its value.
        let mut w = mission_world(0, None);
        let mut vm = VmState::new(p.clone(), vec![], 9, false);
        vm.class_vars[0][1] = 1;
        vm.budget = WORK_BUDGET_AT_LOAD;
        w.vm = Some(vm);
        assert_eq!(
            w.vm_callback(0, callbacks::INITIALIZE, &[]),
            Some(CallOutcome::Returned(0))
        );
        assert_eq!(w.vm.as_ref().unwrap().class_vars[0], vec![5, 1]);
        // A stub's result is a taint on the fused instruction, an observed native's is not.
        p.classes[0].code[3] = Instr::Native {
            id: 221,
            argc: 0,
            dst: Some(cv(0)),
        };
        assert!(p.validate().unwrap_err().contains("takes 1"));
        p.classes[0].code[3] = Instr::Native {
            id: 236,
            argc: 0,
            dst: Some(cv(0)),
        };
        p.classes[0].code[4] = Instr::Native {
            id: 221,
            argc: 1,
            dst: Some(cv(1)),
        };
        assert!(
            p.validate()
                .unwrap_err()
                .contains("stacks are not balanced")
        );
    }
}
