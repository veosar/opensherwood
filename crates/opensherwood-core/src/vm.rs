//! The mission script VM (ADR-0008): the instruction set, the program representation, the
//! run-time state and a deterministic interpreter, all owned by [`World`].
//!
//! The semantics are `docs/original/spec-script-vm.md` (revision 9): section 3.1 for the
//! instructions, 3.2 for the calling convention, 3.3 for the native protocol, 3.4 for running a
//! callback, 3.6 for messages, 8.1 for the deliberate departures and 8.3 for the snapshot set.
//! `docs/formats/scb.md` describes only the container the translator reads.
//! `opensherwood-script` maps each bytecode quad to one [`Instr`]; the core executes them.
//! A cell is four bytes shared by integers and floats (VM-004): the float opcodes read and
//! write the cell's bits as an IEEE-754 single, everything else as a 32-bit two's complement
//! integer. Natives are dispatched by number in `natives.rs`.
//!
//! **Instances.** One [`Instance`] per class (VM-010): its class-variable block, its current
//! script-parameter buffer, its 12-cell native argument buffer, its native result register and
//! its callback return register. Both buffers and both registers persist across callbacks
//! (VM-088) and belong to the snapshot set (VM-014, 8.3); a callback that leaves a residue
//! records [`Assumption::SnapshotSet`], because that persistence is inferred rather than
//! observed. A frame ([`Frame`]) captures the instance's parameter buffer at entry and installs
//! a fresh one; popping it discards the captured buffer, so the callee's stays current.
//!
//! **Time.** One logic frame of 46.875 ms drives everything (ADR-0010, [`crate::TICK_RATE`]):
//! a timer of native 56 counts `n` frames (VM-221), and the animation and AI timers count the
//! same frames. There is no second clock and no conversion.
//!
//! **Faults.** The failure classes of VM-089 get the deterministic outcomes of 8.1: a [`Fault`]
//! is appended to the fault log with the tick and the [`Provenance`] of the work that raised
//! it, and the *site* decides whether the running callback terminates - an unchecked read
//! answers null and goes on, an unchecked write, a trap, a container range error, a bad opcode
//! and the barrier wrap end the callback where they stand. A terminated callback keeps
//! everything it did (nothing is rolled back) and the engine continues its tick. Three faults
//! are recorded once and carry a suppression set that is snapshotted with them:
//! [`Fault::MissingCallback`] per class and name, [`Fault::SentinelJump`] per class and
//! address, [`Fault::ScrollOverlap`] per scroll.
//!
//! **Work.** Everything the VM does in one tick is charged to one deterministic budget
//! ([`WORK_BUDGET_PER_TICK`]): instruction dispatch, the arity a native call transfers, zone and
//! scroll scans (one unit per entity looked at, one per polygon edge tested), the polygon natives
//! 97 / 204, sequence elements, and the path searches of the walks it issues (`nav.rs` charges
//! the search initialisation, node expansions, unwinding, line-clear cells and the smoothed
//! output; `world.rs` the conversion of the final path). The budget is granted exactly once per
//! logic frame, at the start of [`World::vm_tick`]; the load-time run of `attach_script` has its
//! own, [`WORK_BUDGET_AT_LOAD`]. Every other entry point (the event hooks such as `IsTaken`, and
//! `vm_dismiss_text`, which the app calls between frames) draws from whatever the current frame
//! left. When the budget is exhausted the tick stops where it is and `counters.budget_aborts`
//! counts it; nothing panics and nothing loops on. The budget is an engine safeguard, not a rule
//! of the original.
//!
//! **What is not cleared yet.** The scheduler (specification 3.5) and the sequence machinery
//! (3.7) keep this engine's own behaviour behind the new protocols: `Hourglass` runs on every
//! class every frame with the frame counter, `CheckVictoryCondition` follows it, and a sequence
//! runs element by element with [`SeqToken`] completions and a [`SeqElement::Barrier`] instead
//! of the specification's levels. Every decision the original's actors would make is recorded
//! as [`Assumption::ElementAdmission`] or [`Assumption::ElementDuration`], and the ids whose
//! effect the specification leaves open are [`Assumption::UnresolvedEffect`] (4.3) or
//! [`Assumption::UnknownNative`] (4.2).
//!
//! **Sequences.** Elements that take time issue *tokens* ([`SeqToken`]): a walk completes when
//! the entity arrived, gave up or was ordered elsewhere; an animation completes at once. Native
//! 32 is a [`SeqElement::Barrier`] that holds the sequence until every token issued since the
//! previous barrier completed. Text pages (native 203) and timers (native 56) hold the sequence
//! directly. Camera moves are instant. Native 202 texts are never blocking.
//!
//! **Action changes.** Every change of an actor's reported action id is queued
//! (`VmState::pending_action_changes`, snapshotted and hashed) and delivered to the class bound
//! to the actor as `ActionChange(current, previous)` (VM-091) exactly once, with that actor as
//! the current actor (VM-093): a change whose class has no handler is dropped as undeliverable,
//! one whose handler returned or terminated is removed, and one the budget cut short is put
//! back (its transaction rolled back) and retried whole on the next frame. A full queue is a
//! deterministic fault ([`Fault::ActionQueueOverflow`]), never a silent drop.
//!
//! **Hypotheses and taint** (ADR-0008, "Hypotheses and taint"). The taint is *dependency-closed
//! by construction*: every source is named by an [`Assumption`] variant and recorded where the
//! engine takes it, whether or not the script reads a value there - a partly modelled native
//! ([`Assumption::Policy`]), a stub ([`Assumption::StubResult`]), one of the specification's
//! unresolved effects ([`Assumption::UnresolvedEffect`]) or excluded ids
//! ([`Assumption::UnknownNative`]), the departures of 8.1 that the specification maps to an
//! assumption ([`Assumption::UnresolvedJump`], [`Assumption::UnwrittenResultSlot`],
//! [`Assumption::SnapshotSet`]), and the engine's own hypotheses about the world: the unmeasured
//! part of the sight ([`Assumption::SightCone`]) and of the noise radius
//! ([`Assumption::NoiseRadius`]), the alert sequence ([`Assumption::AlertPolicy`]) and its
//! timeout ([`Assumption::AlertTimeout`]), the attack policy ([`Assumption::AttackPolicy`]), the
//! knock-out policy ([`Assumption::KnockOut`]), the profile stats, a scroll's fate after its
//! reading ([`Assumption::ScrollPickup`]), a purse's amount ([`Assumption::ItemPickup`]), the
//! zone presence at load, a walk that completed without arriving, the campaign graph and the
//! lenient asset fallbacks. The set is snapshotted, hashed, validated and exposed as
//! `ScriptObservation::assumptions` / `tainted`; it only grows. `mission_won` / `mission_lost`
//! are still recorded, but an outcome reached with a non-empty set is not authoritative.

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
/// Deepest script-parameter buffer the engine keeps: the original grows it without a limit
/// (VM-010), so this is the engine's bound; a push past it is [`Fault::UncheckedWrite`].
pub const MAX_STACK: usize = 1 << 12;
/// Cells of the native argument buffer (`spec-script-vm.md` constant table: 12 cells, 48
/// bytes). A `0x0B` past it is an unchecked write in the original (VM-051, 8.1).
pub const NATIVE_ARG_CELLS: usize = 12;
/// Entries of the native call table (VM-085): ids 0..=264, all populated.
pub const NATIVE_TABLE_SIZE: usize = 265;
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
/// Cells of the program-wide global block (storage class `00`, VM-010 / VM-011). The original's
/// block size is not in the specification and the retail files never address the class; the
/// engine allocates this many cells and treats anything beyond as an unchecked access.
pub const GLOBAL_CELLS: usize = 1 << 10;
/// Entries native 0 adds beyond the index it declares (`spec-script-vm.md` VM-020: the array
/// grows to `k + 16`).
pub const MISSION_VARIABLE_GROWTH: usize = 16;
/// Largest mission-variable array the engine keeps (the original grows without a limit).
pub const MAX_MISSION_VARIABLES: usize = 1 << 16;
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

/// Storage class of a symbol operand (`docs/original/spec-script-vm.md` VM-011: bits 15..14 of
/// the `u16` select it, bits 13..0 are a byte offset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Space {
    /// Storage class `00`: the program-wide global block (never addressed by the retail files).
    Global,
    /// Storage class `01`: the class-variable block of the instance.
    Class,
    /// Storage class `10`: the frame's locals block ("volatile").
    Local,
    /// Storage class `11`: the frame's temporaries block.
    Temp,
}

impl Space {
    fn tag(self) -> u8 {
        match self {
            Space::Global => 0,
            Space::Class => 1,
            Space::Local => 2,
            Space::Temp => 3,
        }
    }
}

/// An operand: a 4-byte cell in one of the four storage classes (VM-011).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slot {
    /// Storage class.
    pub space: Space,
    /// Cell index (the operand's byte offset / 4).
    pub index: u32,
}

/// The operator of a three-operand instruction (`spec-script-vm.md` 3.1, opcodes `0x19`..`0x2F`;
/// VM-063 - VM-068). Integer arithmetic wraps and integer comparisons are signed; the float
/// operations read both operands as IEEE-754 singles stored in the 4-byte cell and the float
/// comparisons *answer* `1.0f` / `0.0f`, with the unordered outcomes of VM-068.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinOp {
    /// `0x19`: integer add (wrapping).
    AddInt,
    /// `0x1A`: integer subtract (wrapping).
    SubInt,
    /// `0x1B`: integer multiply (wrapping).
    MulInt,
    /// `0x1C`: signed integer divide, truncating; `/ 0` and `INT_MIN / -1` are traps (VM-064).
    DivInt,
    /// `0x1D`: bitwise or.
    Or,
    /// `0x1E`: bitwise and.
    And,
    /// `0x1F`: bitwise exclusive or.
    Xor,
    /// `0x20`: float add.
    AddFloat,
    /// `0x21`: float subtract.
    SubFloat,
    /// `0x22`: float multiply.
    MulFloat,
    /// `0x23`: float divide.
    DivFloat,
    /// `0x24`: `a <= b` (signed integers) -> 1 / 0.
    LeInt,
    /// `0x25`: `a < b`.
    LtInt,
    /// `0x26`: `a >= b`.
    GeInt,
    /// `0x27`: `a > b`.
    GtInt,
    /// `0x28`: `a != b`.
    NeInt,
    /// `0x29`: `a == b`.
    EqInt,
    /// `0x2A`: float `a <= b` -> `1.0f` / `0.0f`; unordered -> `1.0f`.
    LeFloat,
    /// `0x2B`: float `a < b`; unordered -> `1.0f`.
    LtFloat,
    /// `0x2C`: float `a >= b`; unordered -> `0.0f`.
    GeFloat,
    /// `0x2D`: float `a > b`; unordered -> `0.0f`.
    GtFloat,
    /// `0x2E`: float `a != b`; unordered -> `0.0f`.
    NeFloat,
    /// `0x2F`: float `a == b`; unordered -> `1.0f`.
    EqFloat,
}

impl BinOp {
    fn tag(self) -> u8 {
        match self {
            BinOp::AddInt => 1,
            BinOp::SubInt => 2,
            BinOp::MulInt => 3,
            BinOp::DivInt => 4,
            BinOp::Or => 5,
            BinOp::And => 6,
            BinOp::Xor => 7,
            BinOp::AddFloat => 8,
            BinOp::SubFloat => 9,
            BinOp::MulFloat => 10,
            BinOp::DivFloat => 11,
            BinOp::LeInt => 12,
            BinOp::LtInt => 13,
            BinOp::GeInt => 14,
            BinOp::GtInt => 15,
            BinOp::NeInt => 16,
            BinOp::EqInt => 17,
            BinOp::LeFloat => 18,
            BinOp::LtFloat => 19,
            BinOp::GeFloat => 20,
            BinOp::GtFloat => 21,
            BinOp::NeFloat => 22,
            BinOp::EqFloat => 23,
        }
    }

    /// The bytecode opcode of this operator.
    #[must_use]
    pub fn opcode(self) -> u8 {
        0x19 + self.tag() - 1
    }

    /// The operator of a three-operand opcode (`0x19`..=`0x2F`), `None` otherwise.
    #[must_use]
    pub fn of_opcode(opcode: u8) -> Option<Self> {
        Some(match opcode {
            0x19 => BinOp::AddInt,
            0x1a => BinOp::SubInt,
            0x1b => BinOp::MulInt,
            0x1c => BinOp::DivInt,
            0x1d => BinOp::Or,
            0x1e => BinOp::And,
            0x1f => BinOp::Xor,
            0x20 => BinOp::AddFloat,
            0x21 => BinOp::SubFloat,
            0x22 => BinOp::MulFloat,
            0x23 => BinOp::DivFloat,
            0x24 => BinOp::LeInt,
            0x25 => BinOp::LtInt,
            0x26 => BinOp::GeInt,
            0x27 => BinOp::GtInt,
            0x28 => BinOp::NeInt,
            0x29 => BinOp::EqInt,
            0x2a => BinOp::LeFloat,
            0x2b => BinOp::LtFloat,
            0x2c => BinOp::GeFloat,
            0x2d => BinOp::GtFloat,
            0x2e => BinOp::NeFloat,
            0x2f => BinOp::EqFloat,
            _ => return None,
        })
    }

    /// Apply the operator. `Err` is the arithmetic trap of VM-064 (`0x1C`); every other
    /// operator is total. The float comparisons are written as the x87's condition codes
    /// leave them, so the unordered outcomes of VM-068 fall out of the negations.
    #[allow(clippy::neg_cmp_op_on_partial_ord, clippy::nonminimal_bool)]
    fn apply(self, a: i32, b: i32) -> Result<i32, ()> {
        let fa = f32::from_bits(a as u32);
        let fb = f32::from_bits(b as u32);
        let flt = |v: bool| if v { 1.0f32 } else { 0.0f32 }.to_bits() as i32;
        Ok(match self {
            BinOp::AddInt => a.wrapping_add(b),
            BinOp::SubInt => a.wrapping_sub(b),
            BinOp::MulInt => a.wrapping_mul(b),
            BinOp::DivInt => {
                if b == 0 || (a == i32::MIN && b == -1) {
                    return Err(());
                }
                a / b
            }
            BinOp::Or => a | b,
            BinOp::And => a & b,
            BinOp::Xor => a ^ b,
            BinOp::AddFloat => (fa + fb).to_bits() as i32,
            BinOp::SubFloat => (fa - fb).to_bits() as i32,
            BinOp::MulFloat => (fa * fb).to_bits() as i32,
            BinOp::DivFloat => (fa / fb).to_bits() as i32,
            BinOp::LeInt => i32::from(a <= b),
            BinOp::LtInt => i32::from(a < b),
            BinOp::GeInt => i32::from(a >= b),
            BinOp::GtInt => i32::from(a > b),
            BinOp::NeInt => i32::from(a != b),
            BinOp::EqInt => i32::from(a == b),
            // VM-068: an unordered comparison (a NaN operand) answers as the x87's
            // condition codes leave it - `<=`, `<` and `==` true, `>=`, `>` and `!=` false.
            BinOp::LeFloat => flt(!(fa > fb)),
            BinOp::LtFloat => flt(!(fa >= fb)),
            BinOp::GeFloat => flt(fa >= fb),
            BinOp::GtFloat => flt(fa > fb),
            BinOp::NeFloat => flt(fa.partial_cmp(&fb).is_some_and(std::cmp::Ordering::is_ne)),
            BinOp::EqFloat => flt(!fa.partial_cmp(&fb).is_some_and(std::cmp::Ordering::is_ne)),
        })
    }
}

/// One instruction: one bytecode quad, so a quad index is an instruction index and the jump,
/// call and native-call targets of VM-002 address this vector directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Instr {
    /// `0x00` / `0x04` / any opcode >= `0x30`: an instruction the original reports as an error.
    /// `0x04` continues at the next instruction; the other two are [`Fault::BadOpcode`] (8.1:
    /// the unchecked fetch and the endless loop become a deterministic fault).
    Bad {
        /// The opcode as the file spells it.
        opcode: u8,
    },
    /// `0x01`: no operation.
    Nop,
    /// `0x02`: append `S(a)` to the current script-parameter buffer (VM-042).
    PushParam {
        /// Value.
        src: Slot,
    },
    /// `0x03`: prologue; allocate the locals and temporaries blocks zero-filled (VM-043).
    Enter {
        /// Local cells.
        locals: u32,
        /// Temporary cells.
        temps: u32,
    },
    /// `0x05`: call; push a frame with return `pc + 1` and jump to `target` (VM-045).
    Call {
        /// Instruction index `a | (b << 16)`.
        target: u32,
    },
    /// `0x06`: return; pop the frame (VM-046).
    Return,
    /// `0x07`: return with value; the callback return register and, at depth > 1, the caller
    /// frame's result slot take `S(a)`, then the frame is popped (VM-047).
    ReturnValue {
        /// Value.
        src: Slot,
    },
    /// `0x08`: read the frame's saved parameter buffer at byte offset `offset` (VM-048).
    LoadParam {
        /// Destination.
        dst: Slot,
        /// Byte offset into the saved buffer (parameter `k` at `4k`).
        offset: u32,
    },
    /// `0x09`: the inverse write (never emitted by the retail files, VM-049).
    StoreParam {
        /// Value.
        src: Slot,
        /// Byte offset into the saved buffer.
        offset: u32,
    },
    /// `0x0A`: read the frame's result slot (VM-050).
    LoadResult {
        /// Destination.
        dst: Slot,
    },
    /// `0x0B`: append `S(a)` to the native argument buffer (VM-051).
    PushArg {
        /// Value.
        src: Slot,
    },
    /// `0x0C`: native call `id = a | (b << 16)` (VM-052).
    Native {
        /// Native id.
        id: u32,
    },
    /// `0x0D`: read the native result register (VM-053).
    LoadNativeResult {
        /// Destination.
        dst: Slot,
    },
    /// `0x0E`: `pc := a | (b << 16)` (VM-054). The two retail jumps to `0xFFFFFFFF` end the
    /// callback and record [`Assumption::UnresolvedJump`] (VM-070, 8.1).
    Jump {
        /// Instruction index.
        target: u32,
    },
    /// `0x0F`: jump to `c` if `S(a) != 0` (VM-055).
    JumpIfNonZero {
        /// Condition.
        cond: Slot,
        /// Instruction index.
        target: u32,
    },
    /// `0x10`: jump to `c` if `S(a) == 0` (VM-056; never emitted).
    JumpIfZero {
        /// Condition.
        cond: Slot,
        /// Instruction index.
        target: u32,
    },
    /// `0x11` / `0x12`: `S(a) := S(b)` (VM-057).
    Move {
        /// Destination.
        dst: Slot,
        /// Source.
        src: Slot,
    },
    /// `0x13` / `0x14`: `S(a) := c`, the 32-bit operand verbatim (VM-058; an integer or the
    /// bit pattern of a float immediate - the cell holds either).
    LoadImm {
        /// Destination.
        dst: Slot,
        /// Value (the operand's bits).
        value: i32,
    },
    /// `0x15`: `S(a) := -S(b)` as an integer, `INT_MIN` unchanged (VM-059).
    NegInt {
        /// Destination.
        dst: Slot,
        /// Source.
        src: Slot,
    },
    /// `0x16`: float sign flip (VM-060; never emitted).
    NegFloat {
        /// Destination.
        dst: Slot,
        /// Source.
        src: Slot,
    },
    /// `0x17`: float -> int, truncating toward zero into 64 bits and keeping the low 32
    /// (VM-061; never emitted).
    FloatToInt {
        /// Destination.
        dst: Slot,
        /// Source.
        src: Slot,
    },
    /// `0x18`: int -> float, round to nearest even (VM-062).
    IntToFloat {
        /// Destination.
        dst: Slot,
        /// Source.
        src: Slot,
    },
    /// `0x19`..=`0x2F`: three-operand arithmetic or comparison (VM-063 - VM-068).
    Binary {
        /// Operator.
        op: BinOp,
        /// Destination `S(a)`.
        dst: Slot,
        /// Left operand `S(b)`.
        a: Slot,
        /// Right operand `S(c16)`.
        b: Slot,
    },
}

impl Instr {
    /// Stable tag for canonical encodings.
    #[must_use]
    pub fn tag(&self) -> u8 {
        match self {
            Instr::Bad { .. } => 1,
            Instr::Nop => 2,
            Instr::PushParam { .. } => 3,
            Instr::Enter { .. } => 4,
            Instr::Call { .. } => 5,
            Instr::Return => 6,
            Instr::ReturnValue { .. } => 7,
            Instr::LoadParam { .. } => 8,
            Instr::StoreParam { .. } => 9,
            Instr::LoadResult { .. } => 10,
            Instr::PushArg { .. } => 11,
            Instr::Native { .. } => 12,
            Instr::LoadNativeResult { .. } => 13,
            Instr::Jump { .. } => 14,
            Instr::JumpIfNonZero { .. } => 15,
            Instr::JumpIfZero { .. } => 16,
            Instr::Move { .. } => 17,
            Instr::LoadImm { .. } => 18,
            Instr::NegInt { .. } => 19,
            Instr::NegFloat { .. } => 20,
            Instr::FloatToInt { .. } => 21,
            Instr::IntToFloat { .. } => 22,
            Instr::Binary { .. } => 23,
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
/// out; medium confidence for the three named kinds, everything else stays unknown and is kept
/// by its raw value).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// Arrows (`unknown_a` 0): taking the pile adds its stack to the character's arrows.
    Arrows,
    /// A purse with money (`unknown_a` 9): taking it adds [`PURSE_MONEY_PER_STACK`] times the
    /// stack to the mission's money and one purse to the character's purses.
    Purse,
    /// A pouch (`unknown_a` 8: the item the first mission's pick-up tutorial sends the player
    /// to, observed as a small pouch with the badge counting 1, `docs/original/h01-measurements-2.md`
    /// 1.1 / 1.5; the empty purse of the throw is the hypothesis of `rhm.md`). Taking it only
    /// removes it: whether it feeds the purse counter is not measured
    /// ([`Assumption::ItemPickup`]).
    Pouch,
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
            8 => ItemKind::Pouch,
            9 => ItemKind::Purse,
            other => ItemKind::Unknown(other),
        }
    }

    fn encode(self, e: &mut Encoder) {
        match self {
            ItemKind::Arrows => e.u8(1),
            ItemKind::Purse => e.u8(2),
            ItemKind::Unknown(a) => e.u8(3).u32(u32::from(a)),
            ItemKind::Pouch => e.u8(4),
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
}

impl Program {
    /// Check every internal reference and bound a translated script or a snapshot must satisfy:
    /// table sizes (per class and aggregate), functions laid out in table order from address 0
    /// each starting with the `Enter` of its frame sizes, jump and call targets inside the
    /// class code (the two retail jumps to [`END_OF_CALLBACK`] excepted, VM-070), slot indices
    /// inside their blocks, bindings inside the tables, and element and location coordinates
    /// within `+-MAX_LOCATION_COORD`. The translator performs the same checks earlier for
    /// diagnostics; this is the trust boundary (a snapshot embeds the program).
    ///
    /// What it deliberately does **not** check is what the original does not check either
    /// (VM-011, VM-048, VM-051, VM-080, VM-087): argument counts, parameter offsets and the
    /// fill of the native buffer. Those are run-time conditions with the deterministic
    /// outcomes of `spec-script-vm.md` 8.1 ([`Fault::UncheckedRead`] / [`Fault::UncheckedWrite`]).
    #[allow(clippy::match_same_arms)]
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
                let _ = end;
                let slot_ok = |s: Slot| match s.space {
                    Space::Global => s.index < GLOBAL_CELLS as u32,
                    Space::Class => s.index < c.variable_count,
                    Space::Local => s.index < f.locals,
                    Space::Temp => s.index < f.temps,
                };
                // A jump or call target addresses the class code (VM-002); the retail
                // `0xFFFFFFFF` of VM-070 is accepted and ends the callback at run time.
                let target_ok = |t: u32| t as usize <= c.code.len() || t == END_OF_CALLBACK;
                let ok = match *ins {
                    Instr::Nop | Instr::Return | Instr::Bad { .. } => true,
                    Instr::Enter { locals, temps } => locals == f.locals && temps == f.temps,
                    Instr::ReturnValue { src }
                    | Instr::PushParam { src }
                    | Instr::PushArg { src } => slot_ok(src),
                    Instr::StoreParam { src, .. } => slot_ok(src),
                    Instr::LoadParam { dst, .. }
                    | Instr::LoadResult { dst }
                    | Instr::LoadNativeResult { dst }
                    | Instr::LoadImm { dst, .. } => slot_ok(dst),
                    Instr::Call { target } | Instr::Jump { target } => target_ok(target),
                    Instr::Native { id } => {
                        if id as usize >= NATIVE_TABLE_SIZE {
                            return Err(format!(
                                "class {ci} instruction {pc} calls native {id}, beyond the table of {NATIVE_TABLE_SIZE}"
                            ));
                        }
                        true
                    }
                    Instr::JumpIfNonZero { cond, target } | Instr::JumpIfZero { cond, target } => {
                        slot_ok(cond) && target_ok(target)
                    }
                    Instr::Move { dst, src }
                    | Instr::NegInt { dst, src }
                    | Instr::NegFloat { dst, src }
                    | Instr::FloatToInt { dst, src }
                    | Instr::IntToFloat { dst, src } => slot_ok(dst) && slot_ok(src),
                    Instr::Binary { dst, a, b, .. } => slot_ok(dst) && slot_ok(a) && slot_ok(b),
                };
                if !ok {
                    return Err(format!("class {ci} instruction {pc} out of range"));
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

fn encode_slot(e: &mut Encoder, s: Slot) {
    e.u8(s.space.tag()).u32(s.index);
}

fn encode_instr(e: &mut Encoder, ins: &Instr) {
    e.u8(ins.tag());
    match *ins {
        Instr::Nop | Instr::Return => {}
        Instr::Bad { opcode } => {
            e.u8(opcode);
        }
        Instr::Enter { locals, temps } => {
            e.u32(locals).u32(temps);
        }
        Instr::ReturnValue { src } | Instr::PushParam { src } | Instr::PushArg { src } => {
            encode_slot(e, src);
        }
        Instr::LoadParam { dst, offset } => {
            encode_slot(e, dst);
            e.u32(offset);
        }
        Instr::StoreParam { src, offset } => {
            encode_slot(e, src);
            e.u32(offset);
        }
        Instr::LoadResult { dst } | Instr::LoadNativeResult { dst } => {
            encode_slot(e, dst);
        }
        Instr::Call { target } | Instr::Jump { target } => {
            e.u32(target);
        }
        Instr::Native { id } => {
            e.u32(id);
        }
        Instr::JumpIfNonZero { cond, target } | Instr::JumpIfZero { cond, target } => {
            encode_slot(e, cond);
            e.u32(target);
        }
        Instr::Move { dst, src }
        | Instr::NegInt { dst, src }
        | Instr::NegFloat { dst, src }
        | Instr::FloatToInt { dst, src }
        | Instr::IntToFloat { dst, src } => {
            encode_slot(e, dst);
            encode_slot(e, src);
        }
        Instr::LoadImm { dst, value } => {
            encode_slot(e, dst);
            e.i32(value);
        }
        Instr::Binary { op, dst, a, b } => {
            e.u8(op.tag());
            encode_slot(e, dst);
            encode_slot(e, a);
            encode_slot(e, b);
        }
    }
}

/// A call frame (`spec-script-vm.md` VM-012 / VM-013). A frame is created by `0x05` and by a
/// callback entry; it captures the instance's current script-parameter buffer, gets a fresh
/// empty one, and allocates its locals and temporaries when the prologue `0x03` runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    /// The instance (class index) the frame runs on: a nested callback (VM-095) pushes a frame
    /// of another instance on top of this one.
    pub class: u32,
    /// Where `0x06` returns; [`END_OF_CALLBACK`] for the frame the engine pushed.
    pub return_pc: u32,
    /// The 4-byte result slot read by `0x0A`: the value of the most recent `0x07` executed one
    /// frame deeper (VM-050). Zero until one wrote it (VM-071: the original's value is
    /// undefined, OpenSherwood reads 0 and records [`Assumption::UnwrittenResultSlot`]).
    pub result: i32,
    /// Whether a `0x07` wrote [`Frame::result`].
    pub result_written: bool,
    /// The caller's script-parameter buffer, saved at entry and read by `0x08`.
    pub params: Vec<i32>,
    /// Locals block ("volatile"), allocated zero-filled by `0x03`.
    pub locals: Vec<i32>,
    /// Temporaries block, allocated zero-filled by `0x03`.
    pub temps: Vec<i32>,
}

/// The return program counter of the frame the engine pushes for a callback: popping it ends
/// the callback (VM-046, VM-090).
pub const END_OF_CALLBACK: u32 = u32::MAX;

/// One interpreter instance: the per-element state of `spec-script-vm.md` VM-010, one per
/// class (the retail files bind one class to one element). Snapshotted whole (VM-014).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instance {
    /// The class-variable block, zero-filled at construction (VM-004).
    pub vars: Vec<i32>,
    /// The current script-parameter buffer (`0x02` appends, a frame captures it, `0x08` reads
    /// the captured one). It persists across callbacks (VM-088).
    #[serde(default)]
    pub params: Vec<i32>,
    /// The native argument buffer, at most [`NATIVE_ARG_CELLS`] cells (`0x0B` appends, a
    /// native wrapper pops its arity). It persists across callbacks (VM-087, VM-088).
    #[serde(default)]
    pub args: Vec<i32>,
    /// The native result register (`0x0C` writes it, `0x0D` reads it).
    #[serde(default)]
    pub native_result: i32,
    /// The callback return register: `0x07` writes it at any depth, the engine reads it after
    /// a callback, and nothing ever resets it (VM-010, VM-072).
    #[serde(default)]
    pub callback_return: i32,
}

impl Instance {
    /// A fresh instance of a class with `variables` cells.
    #[must_use]
    pub fn new(variables: usize) -> Self {
        Instance {
            vars: vec![0; variables],
            params: Vec::new(),
            args: Vec::new(),
            native_result: 0,
            callback_return: 0,
        }
    }
}

/// A `ProcessMessage` (natives 43 / 44 / 109 / 110), delivered synchronously (VM-120).
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
    /// Native 56: wait this many logic frames.
    Wait(u32),
    /// Natives 33 / 34: move the camera to a location value.
    Camera(i32),
    /// Natives 43 / 44: record a message; it is delivered when the sequence reaches it
    /// (natives 109 / 110 deliver synchronously outside a recording, VM-095).
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
    /// The provenance native 31 copied from the recording (8.3).
    #[serde(default)]
    pub provenance: Provenance,
}

/// The global recording state of `spec-script-vm.md` VM-200: at most one recording is open at
/// a time, it persists across callbacks, and it belongs to the snapshot set (8.3). The
/// sequence *semantics* around it stay the engine's own until section 3.7 is cleared
/// ([`Assumption::ElementAdmission`], [`Assumption::ElementDuration`]); the level counter and
/// the entered lists follow the specification so that natives 30 / 31 / 32 / 46 / 47 / 63
/// answer as they must.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Recording {
    /// Elements recorded so far, in order.
    pub elements: Vec<SeqElement>,
    /// The barrier level counter (16 bits; 1 after native 30, 0 = not recording).
    pub level: u16,
    /// The level the last recorded element was tagged with (VM-201: a barrier only counts when
    /// an element carries the current level).
    pub last_level: u16,
    /// The "entered actors" lists of natives 46 / 47 / 63, merged (the two lists of the
    /// original are not distinguished by any cleared claim).
    pub entered: BTreeSet<i32>,
    /// Where the recording came from (8.3): fixed when native 30 opened it.
    #[serde(default)]
    pub provenance: Provenance,
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
/// every place in the core that departs from `docs/original/spec-script-vm.md`, calls a native
/// whose row it models only in part or not at all, or takes one of the engine's own hypotheses
/// records its variant, so a VM whose set is empty took no hypothesis the engine knows of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Assumption {
    /// Native `id` was called and this engine does not model its effect at all
    /// (`natives::Kind::Stub`): the call is counted and the row's neutral value answered.
    StubResult(u32),
    /// Native `id` was called and its settled row reaches a subsystem this engine models only
    /// in part - the camera, animation, the AI, navigation, the campaign
    /// (`natives::Kind::Partial`): the engine's own behaviour is kept behind the new calling
    /// convention.
    Policy(u32),
    /// A jump to `0xFFFFFFFF` ended its callback instead of fetching before the instruction
    /// array (`spec-script-vm.md` VM-070, 8.1: the script's evident intent).
    UnresolvedJump,
    /// The result slot of a frame was read by `0x0A` without a `0x07` having written it: the
    /// original reads whatever the allocation left there, OpenSherwood reads 0 (VM-071, 8.1).
    UnwrittenResultSlot,
    /// The snapshot set of an instance (VM-014 / VM-088) was exercised: a callback ended with
    /// a non-empty native-argument or script-parameter buffer, so the residue the next
    /// callback consumes rests on the inferred persistence rule rather than on an observation.
    SnapshotSet,
    /// The element table's shape was used where the spec leaves it open (VM-030: whether and
    /// where the script zones occupy table entries).
    ElementTableOrder,
    /// An `ActionChange` was dispatched for a target-family object rather than an actor: the
    /// object dispatcher was not read (VM-107).
    ObjectActionChange,
    /// `FilterAIEvent` ran for one of the seven events 100..106 of the second dispatcher,
    /// whose meaning is open (VM-108 b).
    AiEventCode(u32),
    /// `ReachPoint` fired from the patrol-node variant, which is inferred (VM-109).
    ReachPointSource,
    /// A sequence element was offered to an actor and the engine's own admission / priority
    /// rules decided, the original's predicates being open (VM-216).
    ElementAdmission,
    /// A sequence element of this category completed on a duration the spec leaves open
    /// (VM-231).
    ElementDuration(ElementCategory),
    /// A native whose effect is settled only up to an unresolved code, flag or consumer
    /// (`spec-script-vm.md` 4.3) was called: the value is passed through verbatim and no
    /// fidelity is claimed for what the consumer does with it.
    UnresolvedEffect(u32),
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
    /// waits for the arrival (`spec-script-vm.md` VM-231, not cleared).
    WalkCompletion,
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

/// The sequence-element category whose completion rule [`Assumption::ElementDuration`] names
/// (`spec-script-vm.md` VM-231: the actor-side durations the sibling specifications have not
/// settled).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementCategory {
    /// A walk (natives 45 / 46 / 47 / 64 / 212): completion on arrival, refusal on a failure.
    Walk,
    /// A seek (57 / 70 / 71).
    Seek,
    /// An animation (49 / 50 / 51).
    Animation,
    /// Speech (62 / 69).
    Speak,
    /// An action of native 59.
    Action,
    /// A corpse element (63 / 65).
    Corpse,
}

impl ElementCategory {
    fn tag(self) -> u8 {
        match self {
            ElementCategory::Walk => 1,
            ElementCategory::Seek => 2,
            ElementCategory::Animation => 3,
            ElementCategory::Speak => 4,
            ElementCategory::Action => 5,
            ElementCategory::Corpse => 6,
        }
    }
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
            Assumption::UnwrittenResultSlot => e.u8(5),
            Assumption::CampaignGraph => e.u8(6),
            Assumption::LenientAssets => e.u8(7),
            Assumption::Policy(id) => e.u8(8).u32(id),
            Assumption::SnapshotSet => e.u8(9),
            Assumption::UnresolvedJump => e.u8(10),
            Assumption::UnknownNative(id) => e.u8(11).u32(id),
            Assumption::ScrollPickup => e.u8(12),
            Assumption::ZoneAtLoad => e.u8(13),
            Assumption::WalkCompletion => e.u8(14),
            Assumption::AttackPolicy(rule) => e.u8(16).u8(rule.tag()),
            Assumption::NoiseRadius => e.u8(17),
            Assumption::AlertPolicy => e.u8(18),
            Assumption::CombatActions => e.u8(19),
            Assumption::HeroDeathLoss => e.u8(20),
            Assumption::ItemPickup => e.u8(21),
            Assumption::AlertTimeout => e.u8(22),
            Assumption::ElementTableOrder => e.u8(23),
            Assumption::ObjectActionChange => e.u8(24),
            Assumption::AiEventCode(event) => e.u8(25).u32(event),
            Assumption::ReachPointSource => e.u8(26),
            Assumption::ElementAdmission => e.u8(27),
            Assumption::ElementDuration(c) => e.u8(28).u8(c.tag()),
            Assumption::UnresolvedEffect(id) => e.u8(29).u32(id),
        };
    }

    /// Whether a snapshot may carry this assumption: a `StubResult` names a stub, a `Policy` a
    /// policy native, an `Opcode` a low-confidence opcode, an `UnknownNative` an id without a
    /// row (and only in lenient mode).
    fn well_formed(self, _lenient: bool) -> Result<(), String> {
        use crate::natives::{Kind, native_kind};
        match self {
            Assumption::StubResult(id) if native_kind(id) != Some(Kind::Stub) => Err(format!(
                "vm assumption names native {id}, which is not a stub"
            )),
            Assumption::Policy(id) if native_kind(id) != Some(Kind::Partial) => Err(format!(
                "vm assumption names native {id}, which is not a partly modelled native"
            )),
            Assumption::UnknownNative(id) if native_kind(id) != Some(Kind::Unknown) => {
                Err(format!("vm assumption names native {id}, which is known"))
            }
            Assumption::UnresolvedEffect(id)
                if !crate::natives::UNRESOLVED_EFFECT.contains(&id) =>
            {
                Err(format!(
                    "vm assumption names native {id}, whose effect is not an unresolved one"
                ))
            }
            Assumption::AiEventCode(event) if !(100..=106).contains(&event) => Err(format!(
                "vm assumption names AI event {event}, which is not one of 100..106"
            )),
            _ => Ok(()),
        }
    }
}

/// The deterministic outcome the engine gives a failure the original leaves undefined
/// (`spec-script-vm.md` VM-089 and the departure table of 8.1). Every fault is appended to the
/// fault log (`VmState::faults`, hashed and restored) with the tick and the provenance of the
/// work that raised it; whether the running callback ends there is decided by the *site*, per
/// the departure row, not by the variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fault {
    /// Class **U**: an unchecked access. The payload is the native's id, or the opcode for an
    /// access the instruction itself makes (`0x08` past the parameters, a `0x0B` past the
    /// twelve cells, a symbol outside its block: [`SYMBOL_FAULT`]). Reads answer null / 0 and
    /// the callback continues; writes write nothing and terminate it (8.1).
    UncheckedAccess(u32),
    /// Class **T**: an arithmetic trap (native 161 with `n = 0`, opcode `0x1C` by zero or
    /// `INT_MIN / -1`). The callback terminates.
    Trap,
    /// Class **X**: a bounds-checked container refused an index (native 168). The callback
    /// terminates.
    Range(u32),
    /// Class **F**: a scroll callback was attempted while another scroll's callback was
    /// running (VM-094). The attempted callback is **not run** and nothing terminates; the
    /// take proceeds as after a zero result (8.1). Recorded once per scroll.
    ScrollOverlap(i32),
    /// Class **C**: a callback name the class lacks (`(class, `[`callback_index`]`)`). The
    /// engine's call is a no-op that leaves both registers and the parameter buffer as they
    /// were; nothing terminates. Recorded once per class and name.
    MissingCallback(u32, u32),
    /// The jump to `0xFFFFFFFF` of VM-070 (`(class, address)`): the current frame is popped as
    /// by `0x06` and the popped frame's saved return address decides what follows. Nothing
    /// terminates. Recorded once per class and address.
    SentinelJump(u32, u32),
    /// Class **N** and the `0x00` fetch: an opcode the interpreter reports as an error
    /// (`0x00`, `>= 0x30`). The callback terminates.
    BadOpcode,
    /// The level counter of an open recording would turn from 65,535 to 0 (VM-201, 8.1). The
    /// callback terminates.
    BarrierOverflow,
    /// The deferred execution fault of 8.1: a seek element of 57 / 70 / 71 whose `actor` is
    /// not an actor-family element, observed when the element is handed over. The element is
    /// refused and the rest of its sequence with it; nothing is unwound.
    DeferredTarget(u32, i32),
    /// An id excluded from clearance (4.2) was called in strict mode: the callback terminates.
    UnknownNative(u32),
    /// The action change queue was full when a change arrived: the exactly-once delivery can
    /// no longer be honoured, so the change is recorded as a fault rather than dropped.
    ActionQueueOverflow,
    /// A script call would have pushed the frame beyond [`MAX_FRAMES`] (unbounded recursion):
    /// the callback terminates at the call, its result slot untouched.
    CallStackOverflow,
}

impl Fault {
    fn encode(self, e: &mut Encoder) {
        match self {
            Fault::UnknownNative(id) => e.u8(1).u32(id),
            Fault::ActionQueueOverflow => e.u8(3),
            Fault::CallStackOverflow => e.u8(4),
            Fault::UncheckedAccess(id) => e.u8(5).u32(id),
            Fault::Trap => e.u8(7),
            Fault::Range(id) => e.u8(8).u32(id),
            Fault::ScrollOverlap(h) => e.u8(9).i32(h),
            Fault::MissingCallback(class, name) => e.u8(10).u32(class).u32(name),
            Fault::BadOpcode => e.u8(11),
            Fault::BarrierOverflow => e.u8(12),
            Fault::SentinelJump(class, addr) => e.u8(13).u32(class).u32(addr),
            Fault::DeferredTarget(id, handle) => e.u8(14).u32(id).i32(handle),
        };
    }
}

/// Where a unit of script work came from (`spec-script-vm.md` 8.3, "Provenance"): OpenSherwood
/// bookkeeping the original does not keep, carried by a recording, by the sequence native 31
/// launches from it and by every element of that sequence, so a fault raised long after the
/// recording native returned can be attributed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    /// Nothing is known (the engine started before any callback ran).
    #[default]
    None,
    /// The callback that executed native 30: its instance's class, the callback's name
    /// ([`callback_index`]) and the tick at which it ran.
    Callback {
        /// Class of the instance.
        class: u32,
        /// Index of the callback's name.
        name: u32,
        /// Tick at which native 30 ran.
        tick: u64,
    },
    /// An engine-originated sequence: the kind of order and the tick.
    Engine {
        /// Kind of the originating order (an engine-internal code).
        kind: u32,
        /// Tick at which it was originated.
        tick: u64,
    },
}

impl Provenance {
    fn encode(self, e: &mut Encoder) {
        match self {
            Provenance::None => e.u8(0),
            Provenance::Callback { class, name, tick } => e.u8(1).u32(class).u32(name).u64(tick),
            Provenance::Engine { kind, tick } => e.u8(2).u32(kind).u64(tick),
        };
    }
}

/// One entry of the fault log (8.1, "Fault state"): the fault, the tick at which it was
/// recorded and the provenance of the work that raised it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaultEntry {
    /// What happened.
    pub fault: Fault,
    /// The tick counter when it was recorded.
    pub tick: u64,
    /// Where the work came from.
    pub provenance: Provenance,
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
    /// Native calls by id.
    pub native_calls: BTreeMap<u32, u64>,
    /// Errors of the failure class E: the native reported and answered its failure value.
    pub native_errors: u64,
    /// Recording natives whose element was dropped because no recording was open (VM-203).
    pub recording_dropped: u64,
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
    /// Callbacks rolled back: every one that aborted (a trap, a fault, the frame limit) and
    /// every queued handler the budget cut short (retried whole on the next tick).
    pub transactions_rolled_back: u64,
}

/// The script-visible state a callback may mutate, captured before it runs and put back when
/// it aborts (a trap, a fault, the frame-limit overflow: every callback, Codex review 11,
/// finding 2) or, for a queued handler, when the budget cuts it short (module documentation,
/// "Action changes"). The VM part is a copy of every mutable field but the program, the
/// digest, the path table, the presence sets, the taken set, the fault and the queue itself;
/// the world part is the entities the callback's natives touched (captured lazily through
/// [`World::vm_touch_entity`]), the selection and the camera. Never serialised: a snapshot is
/// quiescent, no transaction is open between callbacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    instances: Vec<Instance>,
    globals: Vec<i32>,
    mission_vars: Vec<i32>,
    objectives: Vec<Objective>,
    debriefing: Option<i32>,
    sequences: Vec<Sequence>,
    texts: Vec<TextRequest>,
    next_text_id: u64,
    camera_target: Option<(i32, i32)>,
    money: i32,
    patches: BTreeSet<i32>,
    pc_actions: Vec<(i32, i32, i32)>,
    campaign_values: Vec<i32>,
    npc_values: Vec<(i32, i32, i32)>,
    door_bytes: Vec<(i32, i32, i32)>,
    banner_count: i32,
    collecting: Option<Recording>,
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

/// `serde` default for the two context handles: null.
fn none_handle() -> i32 {
    NONE_HANDLE
}

/// Run-time state of the VM (part of [`World`], of the snapshot and of the hash).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmState {
    /// The program.
    pub program: Program,
    /// [`Program::digest`] of `program`, computed at load and checked by `validate`.
    pub program_digest: String,
    /// One interpreter instance per class (VM-010), in class order.
    pub instances: Vec<Instance>,
    /// The program-wide global block, storage class `00` ([`GLOBAL_CELLS`] cells).
    #[serde(default)]
    pub globals: Vec<i32>,
    /// Mission variables (natives 0 / 1 / 2): empty at level start, grown by native 0 to
    /// `k + `[`MISSION_VARIABLE_GROWTH`] (VM-020).
    pub mission_vars: Vec<i32>,
    /// Objectives in the order they were added.
    pub objectives: Vec<Objective>,
    /// Debriefing variant chosen by native 28.
    pub debriefing: Option<i32>,
    /// Active sequences, first is running.
    pub sequences: Vec<Sequence>,
    /// The open recording of natives 30 / 31 / 32 (VM-200); `None` = none open.
    pub collecting: Option<Recording>,
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
    /// Active patches (natives 144 / 145 / 146).
    pub patches: BTreeSet<i32>,
    /// Availability of a player character's actions as `(pc, action, value)` sorted by the
    /// first two (natives 115 / 116). A list rather than a map: JSON has no tuple keys.
    #[serde(default)]
    pub pc_actions: Vec<(i32, i32, i32)>,
    /// The twenty script-visible campaign values of natives 195 / 196.
    #[serde(default)]
    pub campaign_values: Vec<i32>,
    /// The ten custom values a script keeps per NPC as `(npc, k, value)`, sorted (197 / 198).
    #[serde(default)]
    pub npc_values: Vec<(i32, i32, i32)>,
    /// The door bytes of natives 182 - 189 as `(door, byte, value)`, sorted, plus the
    /// click-target flag of native 191 as byte 4.
    #[serde(default)]
    pub door_bytes: Vec<(i32, i32, i32)>,
    /// Banners captured so far (native 178 raises it, 179 lowers it, 234 compares it).
    #[serde(default)]
    pub banner_count: i32,
    /// The current actor of native 74 (VM-093), a handle with dynamic scope; null at a tick
    /// boundary.
    #[serde(default = "none_handle")]
    pub current_actor: i32,
    /// The current scroll of native 192 (VM-094); null at a tick boundary.
    #[serde(default = "none_handle")]
    pub current_scroll: i32,
    /// The force flag of native 29, consumed by the next victory check (VM-242).
    #[serde(default)]
    pub force_victory: bool,
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
    /// departed from the original ([`Fault`]); `None` while it runs as written.
    #[serde(default)]
    pub fault: Option<Fault>,
    /// The fault log of 8.1: every recorded fault in order, with its tick and provenance.
    #[serde(default)]
    pub faults: Vec<FaultEntry>,
    /// The `(class, name)` pairs whose [`Fault::MissingCallback`] is already recorded (8.1).
    #[serde(default)]
    pub seen_missing: BTreeSet<(u32, u32)>,
    /// The `(class, address)` pairs whose [`Fault::SentinelJump`] is already recorded.
    #[serde(default)]
    pub seen_sentinel: BTreeSet<(u32, u32)>,
    /// The scroll handles whose [`Fault::ScrollOverlap`] is already recorded.
    #[serde(default)]
    pub seen_overlap: BTreeSet<i32>,
    /// The tick the fault log stamps on new entries (the world's tick counter, mirrored here
    /// so the VM can record without reaching back into the world).
    #[serde(skip)]
    pub tick: u64,
    /// The callbacks the interpreter is inside, innermost last (`(class, name index)`): the
    /// provenance a native 30 gives its recording. Empty at a tick boundary, so not serialised.
    #[serde(skip)]
    pub callback_stack: Vec<(u32, u32)>,
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
    /// Program counter of the running frame (meaningless while `frames` is empty).
    #[serde(default)]
    pub pc: u32,
    /// Work units left in the current tick (not serialised: granted at the start of every tick
    /// and once at load; events and dismissals draw from what is left).
    #[serde(skip)]
    pub budget: u64,
    /// Diagnostics (not serialised, not hashed).
    #[serde(skip)]
    pub counters: Counters,
    /// The open transaction of the running callback (only while one runs; never serialised).
    #[serde(skip)]
    pub transaction: Option<Transaction>,
}

impl VmState {
    /// Fresh state for a program.
    #[must_use]
    pub fn new(program: Program, paths: Vec<Option<u32>>, seed: u64, lenient: bool) -> Self {
        let instances = program
            .classes
            .iter()
            .map(|c| Instance::new(c.variable_count as usize))
            .collect();
        let program_digest = program.digest();
        VmState {
            program,
            program_digest,
            instances,
            globals: vec![0; GLOBAL_CELLS],
            mission_vars: Vec::new(),
            objectives: Vec::new(),
            debriefing: None,
            sequences: Vec::new(),
            collecting: None,
            texts: Vec::new(),
            next_text_id: 1,
            camera_target: None,
            mission_won: false,
            mission_lost: false,
            money: 0,
            patches: BTreeSet::new(),
            pc_actions: Vec::new(),
            campaign_values: vec![0; crate::natives::CAMPAIGN_VALUES],
            npc_values: Vec::new(),
            door_bytes: Vec::new(),
            banner_count: 0,
            current_actor: NONE_HANDLE,
            current_scroll: NONE_HANDLE,
            force_victory: false,
            attributes: Vec::new(),
            states: BTreeMap::new(),
            inactive_elements: BTreeSet::new(),
            zone_presence: BTreeSet::new(),
            taken_items: BTreeSet::new(),
            paths,
            lenient,
            fault: None,
            faults: Vec::new(),
            seen_missing: BTreeSet::new(),
            seen_sentinel: BTreeSet::new(),
            seen_overlap: BTreeSet::new(),
            tick: 0,
            callback_stack: Vec::new(),
            unknown_calls: Vec::new(),
            assumptions: BTreeSet::new(),
            pending_action_changes: Vec::new(),
            rng: Rng::new(seed, SCRIPT_RNG_STREAM),
            frames: Vec::new(),
            pc: 0,
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

    /// Mark the script faulted with `fault` unless it already is (the first fault is kept)
    /// and append it to the fault log with the running work's provenance (8.1). The three
    /// "recorded once" faults consult their suppression set first and answer `false` when the
    /// entry was already there.
    pub fn set_fault(&mut self, fault: Fault) -> bool {
        let fresh = match fault {
            Fault::MissingCallback(class, name) => self.seen_missing.insert((class, name)),
            Fault::SentinelJump(class, addr) => self.seen_sentinel.insert((class, addr)),
            Fault::ScrollOverlap(handle) => self.seen_overlap.insert(handle),
            _ => true,
        };
        if !fresh {
            return false;
        }
        if self.fault.is_none() {
            self.fault = Some(fault);
        }
        if self.faults.len() < MAX_QUEUE {
            let provenance = self.provenance();
            self.faults.push(FaultEntry {
                fault,
                tick: self.tick,
                provenance,
            });
        }
        true
    }

    /// The provenance of the work the interpreter is doing now (8.3): the innermost callback,
    /// or nothing between callbacks.
    #[must_use]
    pub fn provenance(&self) -> Provenance {
        match self.callback_stack.last() {
            Some(&(class, name)) => Provenance::Callback {
                class,
                name,
                tick: self.tick,
            },
            None => Provenance::None,
        }
    }

    /// Record a hypothesis source (module documentation, "Hypotheses and taint").
    pub fn assume(&mut self, assumption: Assumption) {
        self.assumptions.insert(assumption);
    }

    /// Work units a [`Transaction`] capture costs now: one per value copied (every callback
    /// pays it before it runs).
    #[must_use]
    pub(crate) fn capture_cost(&self) -> u64 {
        let sequences: usize = self
            .sequences
            .iter()
            .map(|s| 1 + s.elements.len() + s.tokens.len())
            .sum();
        let unknown: usize = self.unknown_calls.iter().map(|c| 1 + c.args.len()).sum();
        let cost = self
            .instances
            .iter()
            .map(|i| i.vars.len() + i.params.len() + i.args.len() + 2)
            .sum::<usize>()
            + self.globals.len()
            + self.mission_vars.len()
            + self.objectives.len()
            + self.campaign_values.len()
            + self.npc_values.len()
            + self.door_bytes.len()
            + self
                .collecting
                .as_ref()
                .map_or(0, |r| r.elements.len() + r.entered.len())
            + sequences
            + self.texts.len()
            + self.patches.len()
            + self.pc_actions.len()
            + self.attributes.len()
            + self.states.len()
            + self.inactive_elements.len()
            + unknown
            + 8;
        cost as u64
    }

    /// Capture the mutable VM part of a [`Transaction`], charging one work unit per value
    /// copied first; `None` (nothing copied, the budget zero) when the copy does not fit.
    fn capture(&mut self) -> Option<Transaction> {
        let cost = self.capture_cost();
        if !charge(self, cost) {
            return None;
        }
        Some(Transaction {
            instances: self.instances.clone(),
            globals: self.globals.clone(),
            mission_vars: self.mission_vars.clone(),
            objectives: self.objectives.clone(),
            debriefing: self.debriefing,
            sequences: self.sequences.clone(),
            texts: self.texts.clone(),
            next_text_id: self.next_text_id,
            camera_target: self.camera_target,
            money: self.money,
            patches: self.patches.clone(),
            pc_actions: self.pc_actions.clone(),
            campaign_values: self.campaign_values.clone(),
            npc_values: self.npc_values.clone(),
            door_bytes: self.door_bytes.clone(),
            banner_count: self.banner_count,
            collecting: self.collecting.clone(),
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
        self.instances.clone_from(&t.instances);
        self.globals.clone_from(&t.globals);
        self.mission_vars.clone_from(&t.mission_vars);
        self.objectives.clone_from(&t.objectives);
        self.debriefing = t.debriefing;
        self.sequences.clone_from(&t.sequences);
        self.texts.clone_from(&t.texts);
        self.next_text_id = t.next_text_id;
        self.camera_target = t.camera_target;
        self.money = t.money;
        self.patches.clone_from(&t.patches);
        self.pc_actions.clone_from(&t.pc_actions);
        self.campaign_values.clone_from(&t.campaign_values);
        self.npc_values.clone_from(&t.npc_values);
        self.door_bytes.clone_from(&t.door_bytes);
        self.banner_count = t.banner_count;
        self.collecting.clone_from(&t.collecting);
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
        // Callbacks never yield, so no frame is live at a snapshot boundary (8.3). The
        // instances' buffers and an open recording *are* part of the snapshot set (VM-014,
        // VM-200) and may well be non-empty.
        if !self.frames.is_empty() {
            return Err("vm snapshot is not quiescent (a frame is live)".into());
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
        if self.instances.len() != self.program.classes.len() {
            return Err("vm instances do not match the classes".into());
        }
        for (i, (inst, c)) in self.instances.iter().zip(&self.program.classes).enumerate() {
            if inst.vars.len() != c.variable_count as usize {
                return Err(format!("vm class {i} variable block has the wrong size"));
            }
            if inst.args.len() > NATIVE_ARG_CELLS {
                return Err(format!("vm instance {i} native argument buffer overflows"));
            }
            if inst.params.len() > MAX_STACK {
                return Err(format!("vm instance {i} parameter buffer too long"));
            }
        }
        if self.globals.len() != GLOBAL_CELLS {
            return Err("vm global block has the wrong size".into());
        }
        if self.campaign_values.len() != crate::natives::CAMPAIGN_VALUES {
            return Err("vm campaign value block has the wrong size".into());
        }
        // 8.3: the current actor and the current scroll are null at a tick boundary.
        if self.current_actor != NONE_HANDLE || self.current_scroll != NONE_HANDLE {
            return Err("vm snapshot is not quiescent (a callback context is open)".into());
        }
        if self.mission_vars.len() > MAX_MISSION_VARIABLES {
            return Err("vm mission variable array too long".into());
        }
        if self.objectives.len() > MAX_QUEUE
            || self.sequences.len() > MAX_QUEUE
            || self.texts.len() > MAX_QUEUE
            || self.attributes.len() > MAX_QUEUE * 16
            || self.states.len() > MAX_QUEUE * 16
            || self.inactive_elements.len() > MAX_QUEUE * 16
            || self.patches.len() > MAX_QUEUE
            || self.pc_actions.len() > MAX_QUEUE
            || self.npc_values.len() > MAX_QUEUE * 16
            || self.door_bytes.len() > MAX_QUEUE * 16
            || self.zone_presence.len() > MAX_QUEUE * 16
            || self.taken_items.len() > MAX_QUEUE * 16
        {
            return Err("vm queue too long".into());
        }
        if let Some(r) = &self.collecting {
            if r.elements.len() > MAX_QUEUE || r.entered.len() > MAX_QUEUE {
                return Err("vm open recording too long".into());
            }
            if r.level == 0 {
                return Err("vm open recording has no level".into());
            }
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
        if self.faults.len() > MAX_QUEUE
            || self.seen_missing.len() > MAX_QUEUE
            || self.seen_sentinel.len() > MAX_QUEUE
            || self.seen_overlap.len() > MAX_QUEUE
        {
            return Err("vm fault log too long".into());
        }
        if !self.callback_stack.is_empty() {
            return Err("vm snapshot is not quiescent (a callback is on the stack)".into());
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
        e.u32(self.instances.len() as u32);
        for inst in &self.instances {
            e.u32(inst.vars.len() as u32);
            for v in &inst.vars {
                e.i32(*v);
            }
            e.u32(inst.params.len() as u32);
            for v in &inst.params {
                e.i32(*v);
            }
            e.u32(inst.args.len() as u32);
            for v in &inst.args {
                e.i32(*v);
            }
            e.i32(inst.native_result).i32(inst.callback_return);
        }
        e.u32(self.globals.len() as u32);
        for v in &self.globals {
            e.i32(*v);
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
        e.u32(self.pc_actions.len() as u32);
        for (pc, k, v) in &self.pc_actions {
            e.i32(*pc).i32(*k).i32(*v);
        }
        e.u32(self.campaign_values.len() as u32);
        for v in &self.campaign_values {
            e.i32(*v);
        }
        e.u32(self.npc_values.len() as u32);
        for (npc, k, v) in &self.npc_values {
            e.i32(*npc).i32(*k).i32(*v);
        }
        e.u32(self.door_bytes.len() as u32);
        for (door, k, v) in &self.door_bytes {
            e.i32(*door).i32(*k).i32(*v);
        }
        e.i32(self.banner_count);
        e.u8(u8::from(self.force_victory));
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
        // The fault log and the three suppression sets of 8.1 are authoritative state.
        e.u32(self.faults.len() as u32);
        for f in &self.faults {
            f.fault.encode(e);
            e.u64(f.tick);
            f.provenance.encode(e);
        }
        e.u32(self.seen_missing.len() as u32);
        for (class, name) in &self.seen_missing {
            e.u32(*class).u32(*name);
        }
        e.u32(self.seen_sentinel.len() as u32);
        for (class, addr) in &self.seen_sentinel {
            e.u32(*class).u32(*addr);
        }
        e.u32(self.seen_overlap.len() as u32);
        for h in &self.seen_overlap {
            e.i32(*h);
        }
    }

    /// Encode the `scheduler` hash part (queues, sequences with their tokens, texts, presence).
    /// Frames and stacks are not encoded: they are empty whenever a hash is taken (`validate`
    /// refuses a snapshot where they are not).
    pub fn encode_scheduler(&self, e: &mut Encoder) {
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
            s.provenance.encode(e);
            e.u32(s.tokens.len() as u32);
            for t in &s.tokens {
                match *t {
                    SeqToken::Walk { entity, x, y } => e.u8(1).u32(entity).i32(x).i32(y),
                    SeqToken::Animation { id } => e.u8(2).u32(id),
                };
            }
        }
        match &self.collecting {
            Some(r) => {
                e.u8(1).u32(u32::from(r.level)).u32(u32::from(r.last_level));
                r.provenance.encode(e);
                e.u32(r.elements.len() as u32);
                for el in &r.elements {
                    encode_element(e, el);
                }
                e.u32(r.entered.len() as u32);
                for h in &r.entered {
                    e.i32(*h);
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

    /// Read a `(a, b) -> value` table kept sorted by its first two fields; `default` when the
    /// pair is absent (natives 115 / 116, 182 - 189 / 191, 197 / 198).
    #[must_use]
    pub fn pair_table(table: &[(i32, i32, i32)], a: i32, b: i32, default: i32) -> i32 {
        table
            .binary_search_by_key(&(a, b), |&(x, y, _)| (x, y))
            .map_or(default, |i| table[i].2)
    }

    /// Write into such a table, bounded by `limit` entries.
    pub fn set_pair_table(
        table: &mut Vec<(i32, i32, i32)>,
        a: i32,
        b: i32,
        value: i32,
        limit: usize,
    ) {
        match table.binary_search_by_key(&(a, b), |&(x, y, _)| (x, y)) {
            Ok(i) => table[i].2 = value,
            Err(i) => {
                if table.len() < limit {
                    table.insert(i, (a, b, value));
                }
            }
        }
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

/// The teardown of an engine-level callback: the frames it left are dropped, so no frame is
/// live between callbacks whatever the program did (returned with surplus values, was aborted
/// by the budget, or terminated on a fault). The instances' buffers and an open recording are
/// **not** cleared: they persist across callbacks by VM-088 / VM-200 and belong to the snapshot
/// set (8.3); a callback that leaves one non-empty records [`Assumption::SnapshotSet`], because
/// that persistence is inferred rather than observed.
fn teardown(vm: &mut VmState) {
    vm.frames.clear();
    let residue = vm
        .instances
        .iter()
        .any(|i| !i.params.is_empty() || !i.args.is_empty());
    if residue {
        vm.assume(Assumption::SnapshotSet);
    }
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

/// The index of a callback name in [`callbacks::ALL`], `u32::MAX` for a name that is not one
/// of the engine's (a fault entry then names only the class).
#[must_use]
pub fn callback_index(name: &str) -> u32 {
    callbacks::ALL
        .iter()
        .position(|n| *n == name)
        .map_or(u32::MAX, |i| i as u32)
}

/// Names of the engine callbacks the core invokes (`spec-script-vm.md` VM-091).
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
    /// `(actor_or_0, event)`: an AI event offered to the script (VM-108).
    pub const FILTER_AI_EVENT: &str = "FilterAIEvent";
    /// `(0)` / `(1)`: the level ended in success / failure (VM-091).
    pub const FINALIZE: &str = "Finalize";
    /// Every name a fault entry may carry, in a fixed order (the index is hashed).
    pub const ALL: &[&str] = &[
        INITIALIZE,
        POST_INITIALIZE,
        HOURGLASS,
        CHECK_VICTORY,
        PROCESS_MESSAGE,
        ENTER_ZONE,
        EXIT_ZONE,
        IS_TAKEN,
        REACH_POINT,
        ACTION_CHANGE,
        FILTER_AI_EVENT,
        FINALIZE,
        "ActivatedByApple",
        "ActivatedByArrow",
        "ActivatedByHand",
        "ActivatedByHeal",
        "ActivatedByLever",
        "ActivatedByMoney",
        "ActivatedBySearch",
        "ActivatedByStone",
        "ActivatedBySword",
        "ActivatedByListenable",
    ];
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
            // VM-091: `Initialize` on the level takes one parameter, 0; elsewhere none.
            let params: &[i32] = if class == 0 { &[0] } else { &[] };
            self.vm_callback(class, callbacks::INITIALIZE, params);
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
        // VM-094: the current scroll is the scroll for the duration of its `IsTaken`; the
        // current actor keeps the enclosing value (VM-093).
        let scroll = self
            .vm
            .as_ref()
            .and_then(|vm| vm.program.classes.get(class as usize)?.element)
            .map_or(NONE_HANDLE, |e| e as i32);
        let mut out = None;
        self.vm_with_current_scroll(scroll, |w| {
            out = w.vm_dispatch(class, callbacks::IS_TAKEN, &[actor]);
        });
        out
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
        self.vm_dispatch(class as u32, callbacks::REACH_POINT, &[actor])
    }

    /// Hook: an object bound to `class` was activated (`ActivatedByArrow`, `ActivatedBySword`,
    /// ...; `handler` is the full callback name) by `actor`. Not triggered by the engine yet.
    pub fn vm_activated(&mut self, class: u32, handler: &str, actor: i32) -> Option<i32> {
        if !handler.starts_with("ActivatedBy") {
            return None;
        }
        // VM-093: for the nine `ActivatedBy*` the current actor is **the object itself**.
        let object = self
            .vm
            .as_ref()
            .and_then(|vm| vm.program.classes.get(class as usize)?.element)
            .map_or(NONE_HANDLE, |e| e as i32);
        let mut out = None;
        self.vm_with_current_actor(object, |w| {
            out = w.vm_dispatch(class, handler, &[actor]);
        });
        out
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
            if let Some(vm) = self.vm.as_mut() {
                let ids = [change.previous, change.new];
                let fallen = ids.iter().any(|&a| KNOCK_OUT_ACTIONS.contains(&(a as u32)));
                if fallen && !dead {
                    vm.assume(Assumption::KnockOut);
                }
                if (fallen && dead) || ids.iter().any(|&a| COMBAT_ACTIONS.contains(&(a as u32))) {
                    vm.assume(Assumption::CombatActions);
                }
            }
            // The handler runs whole or not at all: an exhausted one (or one whose capture
            // does not fit the budget) is rolled back and waits for the next tick; a
            // deterministic failure (a trap, a fault such as the frame limit) is rolled back
            // and the change consumed, since it would fail the same way again.
            // VM-093: the actor whose `ActionChange` runs is the current actor. VM-091: the
            // parameters are `(current_action, previous_action)`.
            let actor = self
                .vm
                .as_ref()
                .and_then(|vm| vm.program.classes.get(change.class as usize)?.element)
                .map_or(NONE_HANDLE, |e| e as i32);
            let mut outcome = CallOutcome::Aborted;
            self.vm_with_current_actor(actor, |w| {
                outcome =
                    w.vm_transact(change.class, function, &[change.new, change.previous], true);
            });
            match outcome {
                CallOutcome::Exhausted => break,
                CallOutcome::Aborted | CallOutcome::Returned(_) => done += 1,
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

    /// One tick of the script scheduler (called by `step` before the entities move): the
    /// action changes left over from the previous tick, `Hourglass` on every class, the zone
    /// transitions of the player characters, the active sequences, then
    /// `CheckVictoryCondition` (a scroll's `IsTaken` fires from `World::resolve_pickups`,
    /// after this, at the end of the reading pause). Messages are **not** queued any more:
    /// natives 109 / 110 deliver inside the call and a recorded message element delivers when
    /// its sequence reaches it (VM-120). The order of the phases, the `Hourglass` cadence and
    /// the victory check are the engine's own until `spec-script-vm.md` 3.5 is cleared.
    /// The tick's work budget is granted here and nowhere else; every phase stops when it is
    /// spent.
    pub(crate) fn vm_tick(&mut self) {
        if self.vm.is_none() {
            return;
        }
        let tick = self.tick;
        if let Some(vm) = self.vm.as_mut() {
            vm.tick = tick;
        }
        self.vm_grant_budget(WORK_BUDGET_PER_TICK);
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
        // `spec-script-vm.md` VM-092: 1 = declare victory, 2 = request the end bookkeeping (a
        // debriefing is
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
    pub(crate) fn vm_deliver(&mut self, m: Message) {
        let Some(vm) = self.vm.as_ref() else { return };
        // VM-120: a null target is the level class; any other target is an actor-family
        // element, whose own class handles the message.
        let class = vm
            .program
            .classes
            .iter()
            .position(|c| m.target >= 0 && c.element == Some(m.target as u32))
            .unwrap_or(0) as u32;
        if let Some(vm) = self.vm.as_mut() {
            inc(&mut vm.counters.messages_delivered);
        }
        let params = [m.id, m.arg, m.arg2];
        if m.target == NONE_HANDLE {
            // A message to the level does not change the current actor (VM-093).
            self.vm_dispatch(class, callbacks::PROCESS_MESSAGE, &params);
        } else {
            self.vm_with_current_actor(m.target, |w| {
                w.vm_dispatch(class, callbacks::PROCESS_MESSAGE, &params);
            });
        }
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
            // VM-093: the entering / leaving actor is the current actor.
            self.vm_with_current_actor(actor, |w| {
                w.vm_dispatch(class, name, &[actor]);
            });
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
                        // VM-221 with ADR-0010: a timer of `n` counts `n` logic frames.
                        if let Some(seq) = self.vm.as_mut().and_then(|vm| vm.sequences.get_mut(i)) {
                            seq.wait = SeqWait::Ticks(n);
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
                // VM-120: a message element is delivered synchronously when its sequence
                // reaches it, with the current actor set for an actor target (VM-093).
                SeqElement::Message(m) => self.vm_deliver(m),
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

    /// Invoke `name` on `class` the way the engine does when it has *committed* to the call
    /// (a message, a zone transition, a scroll reading, an activation): a class that lacks the
    /// name is the null dereference of VM-089 class C, recorded once per class and name (8.1,
    /// `Fault::MissingCallback`) and otherwise a no-op that changes nothing - in particular
    /// the parameters are never appended to the instance's buffer.
    pub(crate) fn vm_dispatch(&mut self, class: u32, name: &str, params: &[i32]) -> Option<i32> {
        let known = self
            .vm
            .as_ref()
            .and_then(|vm| vm.program.classes.get(class as usize))
            .is_some_and(|c| c.function(name).is_some());
        if !known {
            if let Some(vm) = self.vm.as_mut() {
                vm.set_fault(Fault::MissingCallback(class, callback_index(name)));
            }
            return None;
        }
        self.vm_event(class, name, params)
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

    /// Run one callback to completion (nested script calls included) within the budget as a
    /// transaction ([`Transaction`]; Codex review 11, finding 2): the script-visible state is
    /// captured first (charged; a capture that does not fit is [`CallOutcome::Exhausted`]
    /// with nothing run), one unit per instruction plus one per argument transferred by a
    /// call or a native is charged as it runs, and an aborted callback (a trap, a fault, the
    /// frame-limit overflow) is rolled back whole, so no partial effect of a callback that
    /// failed deterministically (a variable set before the recursion, a teleport) survives
    /// it. A callback the budget cut short keeps what it did so far (the tick stops there;
    /// the queued handlers of [`World::vm_deliver_action_changes`] use [`World::vm_transact`]
    /// with the rollback on exhaustion instead, since they are retried whole). Every exit (a
    /// return, a budget abort, a fault, a trap) passes through [`teardown`], so the VM is
    /// quiescent afterwards whatever the program did.
    pub(crate) fn vm_invoke(&mut self, class: u32, function: u32, params: &[i32]) -> CallOutcome {
        self.vm_transact(class, function, params, false)
    }

    /// [`World::vm_invoke`] with the exhaustion policy explicit: `roll_back_exhausted` puts
    /// the capture back when the budget cut the callback short (a queued handler, retried
    /// whole next tick); otherwise the partial effects stay. An aborted callback is always
    /// rolled back; a returned one commits (the transaction is dropped).
    fn vm_transact(
        &mut self,
        class: u32,
        function: u32,
        params: &[i32],
        roll_back_exhausted: bool,
    ) -> CallOutcome {
        // A nested callback (VM-095) runs inside the outer one's transaction and inside its
        // frame stack: it neither captures again nor tears the outer frames down.
        if self.vm.as_ref().is_some_and(|vm| !vm.frames.is_empty()) {
            return self.vm_run(class, function, params);
        }
        let captured = self.vm.as_mut().and_then(VmState::capture);
        let Some(mut txn) = captured else {
            if let Some(vm) = self.vm.as_mut() {
                inc(&mut vm.counters.budget_aborts);
            }
            return CallOutcome::Exhausted;
        };
        txn.selected = self.selected;
        txn.camera = self.camera;
        if let Some(vm) = self.vm.as_mut() {
            vm.transaction = Some(txn);
        }
        let outcome = self.vm_run(class, function, params);
        if let Some(vm) = self.vm.as_mut() {
            teardown(vm);
        }
        match outcome {
            // `spec-script-vm.md` 8.1: "a terminated callback ends the engine's call as if the
            // frame had returned with 0x06; the engine then continues its tick" - what the
            // callback did before the fault stands, as it does in the original.
            CallOutcome::Exhausted if roll_back_exhausted => self.vm_roll_back(),
            CallOutcome::Aborted | CallOutcome::Exhausted | CallOutcome::Returned(_) => {
                if let Some(vm) = self.vm.as_mut() {
                    vm.transaction = None;
                }
            }
        }
        outcome
    }

    /// Run callback `function` of `class` with `params` (VM-090): the parameters are appended
    /// to the instance's current script-parameter buffer, a frame with the return pc
    /// [`END_OF_CALLBACK`] captures it, and the interpreter runs until that frame is popped.
    /// The result is the instance's callback return register, which nothing ever resets
    /// (VM-072), so a callback that ended with `0x06` returns what an earlier one left there.
    ///
    /// The run may nest: natives 109 / 110 / 153 / 154 and the message elements of a sequence
    /// call this again from inside a native, and the inner frames are pushed on top of the
    /// outer ones (VM-095). The native-call instruction restores its own `pc` afterwards, so
    /// the outer callback continues correctly.
    fn vm_run(&mut self, class: u32, function: u32, params: &[i32]) -> CallOutcome {
        let Some(vm) = self.vm.as_mut() else {
            return CallOutcome::Aborted;
        };
        inc(&mut vm.counters.callbacks);
        let base = vm.frames.len();
        let saved_pc = vm.pc;
        let Some((address, name)) = vm
            .program
            .classes
            .get(class as usize)
            .and_then(|c| c.functions.get(function as usize))
            .map(|f| (f.address, callback_index(&f.name)))
        else {
            // Class C of VM-089: a callback name the class lacks. The engine's call is a
            // no-op (8.1); `World::vm_dispatch` records it where the engine committed to it.
            return CallOutcome::Aborted;
        };
        vm.callback_stack.push((class, name));
        if let Some(inst) = vm.instances.get_mut(class as usize) {
            for p in params {
                if inst.params.len() < MAX_STACK {
                    inst.params.push(*p);
                }
            }
        }
        if !push_frame(vm, class, address, END_OF_CALLBACK) {
            vm.pc = saved_pc;
            return CallOutcome::Aborted;
        }
        let outcome = self.vm_loop(base);
        if let Some(vm) = self.vm.as_mut() {
            // The termination contract of 8.1: only this invocation's frames are popped (each
            // pop discards its captured parameter buffer), the instance's current buffers and
            // both registers are left as the fault found them.
            vm.frames.truncate(base);
            vm.pc = saved_pc;
            vm.callback_stack.pop();
            if matches!(outcome, CallOutcome::Returned(_)) {
                let r = vm
                    .instances
                    .get(class as usize)
                    .map_or(0, |i| i.callback_return);
                return CallOutcome::Returned(r);
            }
        }
        outcome
    }

    /// The instruction loop of `spec-script-vm.md` 3.1, running until the frame stack is back
    /// at `base`.
    #[allow(clippy::too_many_lines)]
    fn vm_loop(&mut self, base: usize) -> CallOutcome {
        loop {
            let Some(vm) = self.vm.as_mut() else {
                return CallOutcome::Aborted;
            };
            if vm.frames.len() <= base {
                return CallOutcome::Returned(0);
            }
            let (ci, pc) = (vm.frames.last().map_or(0, |f| f.class as usize), vm.pc);
            let ins = vm
                .program
                .classes
                .get(ci)
                .and_then(|c| c.code.get(pc as usize))
                .copied();
            if !charge(vm, 1) {
                inc(&mut vm.counters.budget_aborts);
                return CallOutcome::Exhausted;
            }
            inc(&mut vm.counters.instructions);
            let Some(ins) = ins else {
                // Running off the end of the class code ends the frame as a `0x06` would.
                pop_frame(vm);
                continue;
            };
            let mut terminate = false;
            match ins {
                Instr::Nop => advance(vm),
                Instr::Bad { opcode } => {
                    if opcode == 0x04 {
                        // VM-044: reported, then the next instruction.
                        inc(&mut vm.counters.faults);
                        advance(vm);
                    } else {
                        // VM-040 and VM-069: the unchecked fetch and the endless loop.
                        terminate = fault(vm, Fault::BadOpcode, true);
                    }
                }
                Instr::Enter { locals, temps } => {
                    if !charge(vm, u64::from(locals) + u64::from(temps)) {
                        inc(&mut vm.counters.budget_aborts);
                        return CallOutcome::Exhausted;
                    }
                    if let Some(f) = vm.frames.last_mut() {
                        f.locals = vec![0; locals as usize];
                        f.temps = vec![0; temps as usize];
                    }
                    advance(vm);
                }
                Instr::PushParam { src } => {
                    let v = read(vm, src);
                    let full = vm
                        .instances
                        .get_mut(ci)
                        .is_none_or(|i| i.params.len() >= MAX_STACK);
                    if full {
                        terminate = fault(vm, Fault::UncheckedAccess(0x02), true);
                    } else {
                        vm.instances[ci].params.push(v);
                    }
                    advance(vm);
                }
                Instr::Call { target } => {
                    if !push_frame(vm, ci as u32, target, pc.wrapping_add(1)) {
                        terminate = true;
                    }
                }
                Instr::Return => pop_frame(vm),
                Instr::ReturnValue { src } => {
                    let v = read(vm, src);
                    if let Some(i) = vm.instances.get_mut(ci) {
                        i.callback_return = v;
                    }
                    // VM-047: at a depth greater than one the caller frame's result slot takes
                    // the value too (VM-095 (2): also across a nested callback).
                    if vm.frames.len() >= 2 {
                        let n = vm.frames.len();
                        vm.frames[n - 2].result = v;
                        vm.frames[n - 2].result_written = true;
                    }
                    pop_frame(vm);
                }
                Instr::LoadParam { dst, offset } => {
                    let v = vm
                        .frames
                        .last()
                        .filter(|_| offset.is_multiple_of(4))
                        .and_then(|f| f.params.get((offset / 4) as usize).copied());
                    let v = v.unwrap_or_else(|| {
                        // 8.1: `S(a) := 0`, the callback continues.
                        fault(vm, Fault::UncheckedAccess(0x08), false);
                        0
                    });
                    terminate = !write(vm, dst, v);
                    advance(vm);
                }
                Instr::StoreParam { src, offset } => {
                    let v = read(vm, src);
                    let ok = offset.is_multiple_of(4)
                        && vm
                            .frames
                            .last_mut()
                            .and_then(|f| f.params.get_mut((offset / 4) as usize))
                            .map(|c| *c = v)
                            .is_some();
                    if !ok {
                        terminate = fault(vm, Fault::UncheckedAccess(0x09), true);
                    }
                    advance(vm);
                }
                Instr::LoadResult { dst } => {
                    let (v, written) = vm
                        .frames
                        .last()
                        .map_or((0, false), |f| (f.result, f.result_written));
                    if !written {
                        // VM-071: the original reads whatever the allocation left there.
                        vm.assume(Assumption::UnwrittenResultSlot);
                    }
                    terminate = !write(vm, dst, v);
                    advance(vm);
                }
                Instr::PushArg { src } => {
                    let v = read(vm, src);
                    let full = vm
                        .instances
                        .get_mut(ci)
                        .is_none_or(|i| i.args.len() >= NATIVE_ARG_CELLS);
                    if full {
                        // VM-051 / VM-087: a persistent imbalance overflows the 12-cell buffer.
                        terminate = fault(vm, Fault::UncheckedAccess(0x0b), true);
                    } else {
                        vm.instances[ci].args.push(v);
                    }
                    advance(vm);
                }
                Instr::Native { id } => {
                    let arity = crate::natives::native_row(id).map_or(0, |r| u64::from(r.arity));
                    if !charge(vm, arity) {
                        inc(&mut vm.counters.budget_aborts);
                        return CallOutcome::Exhausted;
                    }
                    // VM-095: the instruction restores its own `pc` from a local copy, so a
                    // nested callback the native runs cannot move it.
                    let here = vm.pc;
                    let ok = self.native_invoke(id);
                    let Some(vm) = self.vm.as_mut() else {
                        return CallOutcome::Aborted;
                    };
                    vm.pc = here;
                    if ok {
                        advance(vm);
                    } else {
                        terminate = true;
                    }
                }
                Instr::LoadNativeResult { dst } => {
                    let v = vm.instances.get(ci).map_or(0, |i| i.native_result);
                    terminate = !write(vm, dst, v);
                    advance(vm);
                }
                Instr::Jump { target } => {
                    if target == END_OF_CALLBACK {
                        // VM-070 with 8.1: not a termination - the current frame is popped
                        // exactly as by 0x06 and its saved return address decides what
                        // follows (a script-call frame returns to its caller, an engine
                        // callback frame ends that invocation).
                        vm.assume(Assumption::UnresolvedJump);
                        fault(vm, Fault::SentinelJump(ci as u32, pc), false);
                        pop_frame(vm);
                    } else {
                        vm.pc = target;
                    }
                }
                Instr::JumpIfNonZero { cond, target } => {
                    if read(vm, cond) != 0 {
                        vm.pc = target;
                    } else {
                        advance(vm);
                    }
                }
                Instr::JumpIfZero { cond, target } => {
                    if read(vm, cond) == 0 {
                        vm.pc = target;
                    } else {
                        advance(vm);
                    }
                }
                Instr::Move { dst, src } => {
                    let v = read(vm, src);
                    terminate = !write(vm, dst, v);
                    advance(vm);
                }
                Instr::LoadImm { dst, value } => {
                    terminate = !write(vm, dst, value);
                    advance(vm);
                }
                Instr::NegInt { dst, src } => {
                    let v = read(vm, src).wrapping_neg();
                    terminate = !write(vm, dst, v);
                    advance(vm);
                }
                Instr::NegFloat { dst, src } => {
                    // VM-060: the x87 flips the sign bit, NaNs included.
                    let v = read(vm, src) ^ i32::MIN;
                    terminate = !write(vm, dst, v);
                    advance(vm);
                }
                Instr::FloatToInt { dst, src } => {
                    let v = float_to_int(read(vm, src));
                    terminate = !write(vm, dst, v);
                    advance(vm);
                }
                Instr::IntToFloat { dst, src } => {
                    let v = (read(vm, src) as f32).to_bits() as i32;
                    terminate = !write(vm, dst, v);
                    advance(vm);
                }
                Instr::Binary { op, dst, a, b } => {
                    let (x, y) = (read(vm, a), read(vm, b));
                    match op.apply(x, y) {
                        Ok(v) => terminate = !write(vm, dst, v),
                        Err(()) => terminate = fault(vm, Fault::Trap, true),
                    }
                    advance(vm);
                }
            }
            if terminate {
                return CallOutcome::Aborted;
            }
        }
    }
}

/// Step past the current instruction.
fn advance(vm: &mut VmState) {
    vm.pc = vm.pc.wrapping_add(1);
}

/// Record a fault (the first one is kept for the hash, every one is appended to the log and
/// counted) and answer `terminate`, which the *site* decides per the departure row of 8.1.
fn fault(vm: &mut VmState, f: Fault, terminate: bool) -> bool {
    inc(&mut vm.counters.faults);
    vm.set_fault(f);
    terminate
}

/// Push a frame on `class` at `target`, saving the instance's current parameter buffer and
/// installing a fresh empty one (VM-013). `false` = the frame limit was reached
/// ([`Fault::CallStackOverflow`], a departure: the original nests until memory runs out).
fn push_frame(vm: &mut VmState, class: u32, target: u32, return_pc: u32) -> bool {
    if vm.frames.len() >= MAX_FRAMES {
        fault(vm, Fault::CallStackOverflow, true);
        return false;
    }
    let params = vm
        .instances
        .get_mut(class as usize)
        .map(|i| std::mem::take(&mut i.params))
        .unwrap_or_default();
    vm.frames.push(Frame {
        class,
        return_pc,
        result: 0,
        result_written: false,
        params,
        locals: Vec::new(),
        temps: Vec::new(),
    });
    vm.pc = target;
    true
}

/// Pop the current frame (VM-013: the saved buffer is freed, the callee's stays current) and
/// continue at its return pc.
fn pop_frame(vm: &mut VmState) {
    if let Some(f) = vm.frames.pop() {
        vm.pc = f.return_pc;
    }
}

/// `S(x)` as a value. An index outside its block is the unchecked read of VM-011 / 8.1: 0,
/// recorded, and the callback continues.
fn read(vm: &mut VmState, s: Slot) -> i32 {
    let class = vm.frames.last().map(|f| f.class as usize);
    let v = match s.space {
        Space::Global => vm.globals.get(s.index as usize).copied(),
        Space::Class => class
            .and_then(|c| vm.instances.get(c))
            .and_then(|i| i.vars.get(s.index as usize))
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
        fault(vm, Fault::UncheckedAccess(SYMBOL_FAULT), false);
        0
    })
}

/// Write `S(x)`; an index outside its block is the unchecked write of 8.1 (nothing written,
/// the callback terminates) and the caller sees `false`.
#[must_use]
fn write(vm: &mut VmState, s: Slot, v: i32) -> bool {
    let class = vm.frames.last().map(|f| f.class as usize);
    let ok = match s.space {
        Space::Global => vm.globals.get_mut(s.index as usize).map(|c| *c = v),
        Space::Class => class
            .and_then(|c| vm.instances.get_mut(c))
            .and_then(|i| i.vars.get_mut(s.index as usize))
            .map(|c| *c = v),
        Space::Local => vm
            .frames
            .last_mut()
            .and_then(|f| f.locals.get_mut(s.index as usize))
            .map(|c| *c = v),
        Space::Temp => vm
            .frames
            .last_mut()
            .and_then(|f| f.temps.get_mut(s.index as usize))
            .map(|c| *c = v),
    };
    if ok.is_none() {
        return !fault(vm, Fault::UncheckedAccess(SYMBOL_FAULT), true);
    }
    true
}

/// The pseudo-id a fault of the interpreter itself (a symbol outside its block, a parameter
/// beyond the buffer, an argument-buffer overflow) carries instead of a native id.
pub const SYMBOL_FAULT: u32 = u32::MAX;

/// Opcode `0x17` (VM-061): truncate the single toward zero into a 64-bit integer and keep the
/// low 32 bits; outside the 64-bit range, or a NaN, gives the 64-bit "indefinite", whose low
/// word is 0.
fn float_to_int(bits: i32) -> i32 {
    let f = f64::from(f32::from_bits(bits as u32)).trunc();
    let wide = if f.is_nan()
        || !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&f)
    {
        0i64
    } else {
        f as i64
    };
    wide as u32 as i32
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

    /// The high bit marks a [`call`] placeholder that [`class`] resolves into an address.
    const CALL_PLACEHOLDER: u32 = 0x8000_0000;

    /// A `0x05` to the `n`-th function of the class being assembled: the target is a code
    /// address (VM-045), which only the finished layout knows, so [`class`] resolves it.
    pub(crate) fn call(function: u32) -> Instr {
        Instr::Call {
            target: CALL_PLACEHOLDER | function,
        }
    }

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
        for ins in &mut code {
            if let Instr::Call { target } = ins
                && *target & CALL_PLACEHOLDER != 0
            {
                let f = (*target & !CALL_PLACEHOLDER) as usize;
                *target = table.get(f).map_or(u32::MAX - 1, |f: &Function| f.address);
            }
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
        }
    }

    /// `0x0B` pushes for every argument, then the `0x0C`, then the `0x0D` that reads the
    /// native result register into `result` (the three instructions the original emits).
    pub(crate) fn native(
        id: u32,
        args: &[i32],
        result: Option<Slot>,
        temps_base: u32,
    ) -> Vec<Instr> {
        let mut v = Vec::new();
        for (i, a) in args.iter().enumerate() {
            v.push(Instr::LoadImm {
                dst: tv(temps_base + i as u32),
                value: *a,
            });
            v.push(Instr::PushArg {
                src: tv(temps_base + i as u32),
            });
        }
        v.push(Instr::Native { id });
        if let Some(dst) = result {
            v.push(Instr::LoadNativeResult { dst });
        }
        v
    }

    #[test]
    fn loops_branches_calls_and_natives() {
        // Level: Initialize sums 0..10 with a loop, calls `double(sum)` and stores it in cv0;
        // CheckVictoryCondition returns 1 when mission var 5 == 55.
        let init = vec![
            Instr::LoadImm {
                dst: lv(0),
                value: 0,
            }, // i
            Instr::LoadImm {
                dst: lv(1),
                value: 0,
            }, // sum
            // 3: loop: t0 = i < 11; if t0 goto 6; goto 12
            Instr::LoadImm {
                dst: tv(1),
                value: 11,
            },
            Instr::Binary {
                op: BinOp::LtInt,
                dst: tv(0),
                a: lv(0),
                b: tv(1),
            },
            Instr::JumpIfNonZero {
                cond: tv(0),
                target: 7,
            },
            Instr::Jump { target: 13 },
            // 7: sum += i; i += 1; goto 3
            Instr::Binary {
                op: BinOp::AddInt,
                dst: lv(1),
                a: lv(1),
                b: lv(0),
            },
            Instr::LoadImm {
                dst: tv(1),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::AddInt,
                dst: lv(0),
                a: lv(0),
                b: tv(1),
            },
            Instr::Nop,
            Instr::Nop,
            Instr::Jump { target: 3 },
            // 13: cv0 = double(sum); n0(5, cv0) - native 0 declares the variable (VM-020).
            Instr::PushParam { src: lv(1) },
            call(2),
            Instr::LoadResult { dst: cv(0) },
            Instr::Nop,
            Instr::LoadImm {
                dst: tv(0),
                value: 5,
            },
            Instr::PushArg { src: tv(0) },
            Instr::PushArg { src: cv(0) },
            Instr::Native { id: 0 },
        ];
        let mut victory = native(2, &[5], Some(tv(0)), 0);
        victory.push(Instr::LoadImm {
            dst: tv(1),
            value: 110,
        });
        victory.push(Instr::Binary {
            op: BinOp::EqInt,
            dst: tv(2),
            a: tv(0),
            b: tv(1),
        });
        victory.push(Instr::ReturnValue { src: tv(2) });
        let double = vec![
            Instr::LoadParam {
                dst: tv(0),
                offset: 0,
            },
            Instr::LoadImm {
                dst: tv(1),
                value: 2,
            },
            Instr::Binary {
                op: BinOp::MulInt,
                dst: tv(2),
                a: tv(0),
                b: tv(1),
            },
            Instr::ReturnValue { src: tv(2) },
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
        assert_eq!(vm.instances[0].vars[0], 110);
        assert_eq!(vm.mission_vars[5], 110);
        assert_eq!(
            vm.mission_vars.len(),
            5 + MISSION_VARIABLE_GROWTH,
            "native 0 grew the array to k + 16 (VM-020)"
        );
        assert!(!vm.mission_won);
        assert!(vm.frames.is_empty() && vm.instances[0].args.is_empty());
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
        assert!(vm.counters.instructions + vm.capture_cost() >= WORK_BUDGET_PER_TICK);
        assert_eq!(vm.budget, 0, "the tick stopped at zero");
    }

    /// Messages are synchronous (VM-120 / VM-121 / VM-095): natives 109 / 110 run the
    /// target's `ProcessMessage` inside the call, a null target is the level class, the
    /// current actor of native 74 is the target actor inside the handler and the enclosing
    /// value again afterwards, and a non-actor target is an error that delivers nothing.
    #[test]
    fn messages_are_delivered_inside_the_native_call() {
        // Level Hourglass: n110(guard, 7, 3, 0); n109(null, 8); cv1 = n74().
        let mut hourglass = native(110, &[1, 7, 3, 0], None, 0);
        hourglass.extend(native(109, &[NONE_HANDLE, 8], None, 0));
        hourglass.extend(native(74, &[], Some(cv(1)), 0));
        // The level's own handler stores the message id and the current actor it observes.
        let level_pm = vec![
            Instr::LoadParam {
                dst: cv(0),
                offset: 0,
            },
            Instr::PushArg { src: cv(0) },
            Instr::Native { id: 74 },
            Instr::LoadNativeResult { dst: cv(2) },
        ];
        // The guard's handler stores the first argument and the current actor.
        let guard_pm = vec![
            Instr::LoadParam {
                dst: cv(0),
                offset: 4,
            },
            Instr::Native { id: 74 },
            Instr::LoadNativeResult { dst: cv(1) },
        ];
        let level = class(
            "StartUp",
            3,
            &[
                ("Hourglass", 1, false, 0, 4, hourglass),
                ("ProcessMessage", 3, false, 0, 4, level_pm),
            ],
        );
        let mut guard = class("Guard", 2, &[("ProcessMessage", 3, false, 0, 4, guard_pm)]);
        guard.element = Some(1);
        let mut w = mission_world(1, Some(program(vec![level, guard], 1)));
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[1].vars[0], 3, "the guard saw arg1 at once");
        assert_eq!(vm.instances[1].vars[1], 1, "74 is the target actor");
        assert_eq!(vm.instances[0].vars[0], 8, "the level saw message 8");
        assert_eq!(
            vm.instances[0].vars[2], NONE_HANDLE,
            "a message to the level does not change the current actor"
        );
        assert_eq!(
            vm.instances[0].vars[1], NONE_HANDLE,
            "the enclosing value is restored after the nested callbacks"
        );
        assert_eq!(vm.counters.messages_delivered, 2);
        assert_eq!(vm.current_actor, NONE_HANDLE);
        w.validate().unwrap();
        // A target that is neither null nor an actor is an error and delivers nothing.
        let hourglass = native(109, &[3, 9], None, 0);
        let level_pm = vec![Instr::LoadImm {
            dst: cv(0),
            value: 1,
        }];
        let level = class(
            "StartUp",
            1,
            &[
                ("Hourglass", 1, false, 0, 4, hourglass),
                ("ProcessMessage", 3, false, 0, 4, level_pm),
            ],
        );
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[0].vars[0], 0, "the scroll target was dropped");
        assert_eq!(vm.counters.messages_delivered, 0);
        assert!(vm.counters.native_errors >= 1);
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
        post.push(Instr::Native { id: 95 });
        post.push(Instr::LoadNativeResult { dst: tv(1) });
        post.push(Instr::PushArg { src: tv(1) });
        post.push(Instr::Native { id: 34 });
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
        // A timer of 3 counts 3 logic frames (VM-221 with ADR-0010).
        assert_eq!(vm.sequences[0].wait, SeqWait::Ticks(3));
        assert_eq!(vm.camera_target, None);
        for _ in 0..3 {
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
                offset: 0,
            },
            Instr::Move {
                dst: cv(0),
                src: tv(0),
            },
            Instr::LoadImm {
                dst: tv(1),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::AddInt,
                dst: cv(1),
                a: cv(1),
                b: tv(1),
            },
        ];
        let exit = vec![
            Instr::LoadImm {
                dst: tv(1),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::AddInt,
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
        assert_eq!(w.vm.as_ref().unwrap().instances[1].vars, vec![0, 0, 0]);
        // Teleport the hero into the zone, then out (native 96 through the script would do the
        // same; here the test moves the entity directly).
        w.entities[0].x = Fixed::from_int(500);
        w.entities[0].y = Fixed::from_int(500);
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().instances[1].vars, vec![0, 1, 0]);
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().instances[1].vars, vec![0, 1, 0]);
        w.entities[0].x = Fixed::from_int(100);
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().instances[1].vars, vec![0, 1, 1]);
        // A guard inside the zone does not count.
        w.entities[1].x = Fixed::from_int(500);
        w.entities[1].y = Fixed::from_int(500);
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().instances[1].vars, vec![0, 1, 1]);
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
            Instr::LoadImm {
                dst: tv(1),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::AddInt,
                dst: cv(0),
                a: cv(0),
                b: tv(1),
            },
            Instr::ReturnValue { src: cv(1) },
        ];
        let level = class("StartUp", 0, &[("Initialize", 0, false, 0, 0, vec![])]);
        let mut scroll = class("Scroll", 2, &[("IsTaken", 1, true, 0, 4, body)]);
        scroll.element = Some(2); // the scroll at (700, 700) of `program`
        let mut w = mission_world(1, Some(program(vec![level, scroll], 1)));
        let reads = |w: &World| w.vm.as_ref().unwrap().instances[1].vars[0];
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().instances[1].vars, vec![0, 0]);
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
        w.vm.as_mut().unwrap().instances[1].vars[1] = 1;
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
        assert_eq!(w.vm.as_ref().unwrap().instances[0].vars, vec![0, 1]);
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
        assert_eq!(w.vm.as_ref().unwrap().instances[0].vars, vec![1, 1]);
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
        init.extend(native(224, &[1, 2, 3, 4], Some(cv(7)), 0));
        init.extend(native(114, &[1], None, 0));
        init.extend(native(85, &[1], Some(cv(8)), 0));
        let level = class("StartUp", 9, &[("Initialize", 0, false, 0, 4, init)]);
        let mut w = mission_world_with(2, Some(program(vec![level], 2)), true);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[0].vars[0], 42);
        assert!(vm.instances[0].vars[1] < 10);
        assert_eq!(vm.rng.draws, 1);
        assert_eq!(
            vm.instances[0].vars[2], 0,
            "85 asks whether the handle is null"
        );
        assert_eq!(vm.instances[0].vars[3], 1, "hero is a PC");
        assert_eq!(vm.instances[0].vars[4], 1, "one PC");
        assert_eq!(vm.instances[0].vars[5], 5, "5 elements");
        assert_eq!(vm.instances[0].vars[6], 0, "nobody in the zone");
        assert_eq!(
            vm.instances[0].vars[7], 0,
            "an excluded native answers its placeholder in lenient mode"
        );
        assert_eq!(vm.instances[0].vars[8], 0, "85 is still false for a handle");
        assert_eq!(vm.counters.unknown_natives.get(&224), Some(&1));
        assert!(!vm.faulted() && vm.lenient);
        assert_eq!(
            vm.unknown_calls,
            vec![UnknownCall {
                id: 224,
                args: vec![1, 2, 3, 4]
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
    fn a_terminating_fault_keeps_what_the_callback_did_and_the_tick_goes_on() {
        // Initialize: cv0 = 1; n161(0) (the arithmetic trap of VM-089 class T); cv1 = 1 -- the
        // callback terminates at the trap, so cv1 is never written; what it did before the
        // fault **stands** (8.1: nothing is rolled back), so cv0 is 1.
        // `Hourglass` keeps running afterwards (cv2 counts ticks).
        let mut init = vec![Instr::LoadImm {
            dst: cv(0),
            value: 1,
        }];
        init.extend(native(161, &[0], Some(tv(0)), 0));
        init.push(Instr::LoadImm {
            dst: cv(1),
            value: 1,
        });
        let hourglass = vec![
            Instr::LoadImm {
                dst: tv(0),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::AddInt,
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
        assert_eq!(vm.fault, Some(Fault::Trap));
        assert_eq!(
            vm.instances[0].vars,
            vec![1, 0, 0],
            "cv0 stands, cv1 never ran"
        );
        assert_eq!(vm.faults.len(), 1);
        assert!(vm.unknown_calls.is_empty());
        assert!(vm.frames.is_empty());
        w.step(&[]);
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(
            vm.instances[0].vars,
            vec![1, 0, 2],
            "later callbacks still run"
        );
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
        v.vm.as_mut().unwrap().mission_vars = vec![1];
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
            .insert(Assumption::UnwrittenResultSlot);
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
        v.vm.as_mut().unwrap().collecting = Some(Recording {
            elements: vec![SeqElement::Barrier],
            level: 2,
            last_level: 1,
            entered: BTreeSet::new(),
            provenance: Provenance::None,
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
        snap.world.vm.as_mut().unwrap().instances[0].vars.push(1);
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
                offset: 0,
            },
            Instr::LoadImm {
                dst: tv(1),
                value: 30,
            },
            Instr::Binary {
                op: BinOp::EqInt,
                dst: tv(2),
                a: tv(0),
                b: tv(1),
            },
            Instr::JumpIfNonZero {
                cond: tv(2),
                target: 9,
            },
            Instr::LoadImm {
                dst: tv(1),
                value: 60,
            },
            Instr::Binary {
                op: BinOp::EqInt,
                dst: tv(2),
                a: tv(0),
                b: tv(1),
            },
            Instr::JumpIfNonZero {
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
        // The hero stands far away: nothing but the script moves the guard.
        w.entities[0].x = Fixed::from_int(900);
        w.entities[0].y = Fixed::from_int(700);
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
        // Native 134 on a player character is an error with no effect (its row in section 6),
        // so the player's order is untouched and the character is not locked.
        w.plan_path(0, (Fixed::from_int(300), Fixed::from_int(100)));
        assert_eq!(w.native_call(134, &[0, 1]), 0);
        assert!(!w.entities[0].ai_locked && w.entities[0].target.is_some());
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
        // Hourglass: the first call sets cv1 and spins; every later one sets cv0.
        // PostInitialize: n30; n56(2); n34(0); n31. CheckVictoryCondition: 1.
        // Zone class: EnterZone: cv0 = 1; the hero stands inside the zone from the start.
        let hourglass = vec![
            Instr::JumpIfNonZero {
                cond: cv(1),
                target: 5,
            },
            Instr::LoadImm {
                dst: cv(1),
                value: 1,
            },
            Instr::Jump { target: 3 },
            Instr::Return,
            // 5 (the prologue counts): cv0 = 1.
            Instr::LoadImm {
                dst: cv(0),
                value: 1,
            },
        ];
        let zone_enter = vec![Instr::LoadImm {
            dst: cv(0),
            value: 1,
        }];
        let mut post = native(30, &[], None, 0);
        post.extend(native(56, &[2], None, 0));
        post.extend(native(34, &[0], None, 0));
        post.extend(native(31, &[], None, 0));
        let victory = vec![
            Instr::LoadImm {
                dst: tv(0),
                value: 1,
            },
            Instr::ReturnValue { src: tv(0) },
        ];
        let level = class(
            "StartUp",
            2,
            &[
                ("Hourglass", 1, false, 0, 4, hourglass),
                ("PostInitialize", 0, false, 0, 4, post),
                ("CheckVictoryCondition", 0, true, 0, 4, victory),
            ],
        );
        let mut zone = class("Zone", 1, &[("EnterZone", 1, true, 0, 4, zone_enter)]);
        zone.zone = Some(1);
        zone.element = Some(2);
        let mut w = mission_world(0, Some(program(vec![level, zone], 0)));
        w.entities[0].x = Fixed::from_int(500);
        w.entities[0].y = Fixed::from_int(500);
        assert_eq!(w.vm.as_ref().unwrap().sequences[0].wait, SeqWait::Ticks(2));
        // Tick 0: the spinning `Hourglass` takes the whole budget; every later phase of the
        // tick is skipped.
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[0].vars, vec![0, 1]);
        assert_eq!(vm.instances[1].vars, vec![0], "zone skipped");
        assert_eq!(vm.sequences[0].wait, SeqWait::Ticks(2), "sequence skipped");
        assert!(!vm.mission_won);
        assert!(vm.counters.budget_aborts >= 1);
        assert_eq!(vm.budget, 0);
        // Tick 1: the `Hourglass`, the zone, the sequence and the victory check all run.
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[0].vars, vec![1, 1]);
        assert_eq!(vm.instances[1].vars, vec![1]);
        assert_eq!(vm.sequences[0].wait, SeqWait::Ticks(1));
        assert!(vm.mission_won);
        // The wait was delayed by exactly the skipped tick: the camera element of the second
        // level runs on tick 2 instead of tick 1.
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().camera_target, Some((200, 200)));
        assert_eq!(w.tick, 3);
        w.validate().unwrap();
        assert_quiescent(&w);
    }

    #[test]
    fn program_validation_is_self_sufficient_and_snapshots_must_be_canonical() {
        // Hourglass(t): cv0 = id(t). id(x): x.
        let body = vec![
            Instr::LoadParam {
                dst: tv(0),
                offset: 0,
            },
            Instr::PushParam { src: tv(0) },
            call(1),
            Instr::LoadResult { dst: cv(0) },
            Instr::Nop,
        ];
        let callee = vec![
            Instr::LoadParam {
                dst: tv(0),
                offset: 0,
            },
            Instr::ReturnValue { src: tv(0) },
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
        // A call or jump outside the class code is refused; a `0x08` beyond the
        // parameters is **not** (VM-048: the original checks nothing, and 8.1 gives the
        // unchecked read its deterministic outcome).
        reject(
            |p| p.classes[0].code[3] = Instr::Call { target: 99 },
            "instruction 3 out of range",
        );
        reject(
            |p| p.classes[0].code[1] = Instr::Jump { target: 99 },
            "instruction 1 out of range",
        );
        reject(
            |p| p.classes[0].code[1] = Instr::Native { id: 999 },
            "beyond the table",
        );
        let mut lenient = base.clone();
        lenient.classes[0].code[1] = Instr::LoadParam {
            dst: tv(0),
            offset: 36,
        };
        lenient.validate().unwrap();
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
                    return_pc: END_OF_CALLBACK,
                    result: 0,
                    result_written: false,
                    locals: vec![],
                    temps: vec![0, 0],
                    params: vec![],
                });
            },
            "quiescent",
        );
        // The instances' buffers and an open recording are *not* refused: they belong to the
        // snapshot set (VM-014, VM-200) and may well be non-empty at a tick boundary.
        reject_snap(&mut w, |vm| vm.current_actor = 0, "quiescent");
        reject_snap(&mut w, |vm| vm.current_scroll = 0, "quiescent");
        reject_snap(
            &mut w,
            |vm| {
                vm.collecting = Some(Recording::default());
            },
            "no level",
        );
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
                    provenance: Provenance::None,
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
                    provenance: Provenance::None,
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
                    provenance: Provenance::None,
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
        // The native argument buffer holds exactly the twelve cells of VM-051.
        assert_eq!(NATIVE_ARG_CELLS, 12);
    }

    #[test]
    fn distance_and_camera_natives_are_total_at_the_coordinate_bounds() {
        // Initialize: cv0 = n160(0, 2); cv1 = n160(0, 0); cv2 = n160(0, -1); n18(2); n20(0).
        // (33 / 34 only record; 18 / 20 act at once, VM-219.)
        // Locations 0 = (-M, -M), 2 = (M, M) with M the coordinate bound.
        let m = MAX_LOCATION_COORD;
        let mut init = native(160, &[0, 2], Some(cv(0)), 0);
        init.extend(native(160, &[0, 0], Some(cv(1)), 0));
        init.extend(native(160, &[0, -1], Some(cv(2)), 0));
        init.extend(native(18, &[2], None, 0));
        init.extend(native(20, &[0], None, 0));
        let level = class("StartUp", 3, &[("Initialize", 0, false, 0, 4, init)]);
        let mut p = program(vec![level], 0);
        p.locations[0] = Location::Point { x: -m, y: -m };
        p.locations.push(Location::Point { x: m, y: m });
        let mut w = mission_world(0, Some(p));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(
            vm.instances[0].vars,
            vec![2_965_820, 0, 0],
            "160 truncates the distance and answers 0 for a non-point"
        );
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

    /// No frame is live and no callback context is open (8.3). The instances' buffers and an
    /// open recording are **not** checked: they persist across callbacks by VM-088 / VM-200.
    fn assert_quiescent(w: &World) {
        let vm = w.vm.as_ref().unwrap();
        assert!(vm.frames.is_empty(), "frames");
        assert!(vm.callback_stack.is_empty(), "callback stack");
        assert_eq!(vm.current_actor, NONE_HANDLE, "current actor");
        assert_eq!(vm.current_scroll, NONE_HANDLE, "current scroll");
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
            Instr::LoadImm {
                dst: tv(0),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::AddInt,
                dst: cv(0),
                a: cv(0),
                b: tv(0),
            },
            Instr::Jump { target: 3 },
        ];
        let hourglass = vec![
            Instr::JumpIfNonZero {
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
        };
        let mut w = mission_world(0, Some(program));
        let reads = |w: &World| w.vm.as_ref().unwrap().instances[1].vars[0];
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
        w.vm.as_mut().unwrap().instances[0].vars[0] = 1;
        for _ in 0..200 {
            w.step(&[]);
        }
        assert_eq!(w.entities[0].pickup, Some(1));
        assert_eq!(w.entities[0].pickup_ticks, 1);
        assert_eq!(reads(&w), 1);
        w.vm.as_mut().unwrap().instances[0].vars[0] = 0;
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
            Instr::JumpIfNonZero {
                cond: cv(0),
                target: 4,
            },
            Instr::LoadImm {
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
        assert_eq!(vm.instances[0].vars, vec![1, 1]);
        // One `Hourglass` on its own: the twelve instructions of the body (the prologue, six
        // for native 97, four for 204, the closing return), the four cells the prologue
        // allocates, the arity the two native calls charge (2 + 1), the edges for native 97,
        // and one per entity plus the edges for each of the four live actors that 204
        // tests (its row counts every actor inside the zone, not only player characters).
        let capture = vm.capture_cost();
        w.vm.as_mut().unwrap().budget = WORK_BUDGET_PER_TICK;
        assert_eq!(
            w.vm_callback(0, callbacks::HOURGLASS, &[1]),
            Some(CallOutcome::Returned(0))
        );
        let vm = w.vm.as_ref().unwrap();
        let used = WORK_BUDGET_PER_TICK - vm.budget;
        assert_eq!(used, capture + 12 + 4 + 3 + edges + 4 * (1 + edges));
        // Too little for native 97: nothing is scanned, the native answers 0, and the
        // callback is cut short at the `0x0D` that would have read it - so the destination
        // keeps the value it had (the read is its own instruction now, VM-053).
        let aborts = vm.counters.budget_aborts;
        let vm = w.vm.as_mut().unwrap();
        vm.instances[0].vars = vec![5, 5];
        vm.budget = capture + edges;
        assert_eq!(
            w.vm_callback(0, callbacks::HOURGLASS, &[1]),
            Some(CallOutcome::Exhausted)
        );
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.budget, 0);
        assert_eq!(vm.instances[0].vars, vec![5, 5], "neither result was read");
        assert!(vm.counters.budget_aborts > aborts);
        assert_quiescent(&w);
        // Enough for native 97 and the dispatch up to 204, not for 204's first entity.
        let vm = w.vm.as_mut().unwrap();
        vm.instances[0].vars = vec![5, 5];
        vm.budget = capture + 17 + edges + 1;
        assert_eq!(
            w.vm_callback(0, callbacks::HOURGLASS, &[1]),
            Some(CallOutcome::Exhausted)
        );
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[0].vars, vec![1, 5], "97 was read, 204 was not");
        assert_eq!(vm.budget, 0);
        assert_quiescent(&w);
        assert_round_trips(&w);
    }

    #[test]
    fn every_callback_exit_tears_down_to_a_quiescent_vm() {
        // (a) Budget abort with values pending on both stacks: the loop keeps the depths, so
        // the program is balanced and valid.
        let spin = vec![
            Instr::LoadImm {
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
        // (b) A terminating fault with cells pending and a recording open: Initialize: n30;
        // three argument cells pushed; n161(0) traps (VM-089 class T); n31 never runs.
        // 8.1: nothing is rolled back and neither buffer is cleared, so the recording is
        // still open and the cells are still there - both belong to the snapshot set.
        let mut init = native(30, &[], None, 0);
        init.push(Instr::LoadImm {
            dst: tv(0),
            value: 0,
        });
        init.extend([Instr::PushArg { src: tv(0) }; 3]);
        init.push(Instr::Native { id: 161 });
        init.extend(native(31, &[], None, 0));
        let level = class("StartUp", 0, &[("Initialize", 0, false, 0, 1, init)]);
        let w = mission_world(0, Some(program(vec![level], 0)));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.fault, Some(Fault::Trap));
        assert!(vm.sequences.is_empty(), "native 31 never ran");
        assert!(vm.collecting.is_some(), "the recording is still open");
        assert_eq!(
            vm.instances[0].args.len(),
            2,
            "the wrapper popped its arity"
        );
        assert_quiescent(&w);
        assert_round_trips(&w);
        // (c) The buffer discipline of VM-013 / VM-088: a frame captures the instance's
        // current parameter buffer and installs a fresh one; a pop **discards** the captured
        // buffer, so a second call must push its own parameters again. The native argument
        // buffer is per instance and survives the calls.
        let hourglass = vec![
            Instr::LoadImm {
                dst: tv(0),
                value: 1,
            },
            Instr::PushArg { src: tv(0) },
            Instr::PushParam { src: tv(0) },
            call(1),
            Instr::LoadResult { dst: cv(0) },
            Instr::PushParam { src: tv(0) },
            call(1),
            Instr::LoadResult { dst: cv(1) },
            Instr::Native { id: 3 },
            Instr::LoadNativeResult { dst: cv(2) },
        ];
        let inner = vec![
            Instr::LoadParam {
                dst: tv(0),
                offset: 0,
            },
            Instr::ReturnValue { src: tv(0) },
        ];
        let level = class(
            "StartUp",
            3,
            &[
                ("Hourglass", 1, false, 0, 1, hourglass),
                ("inner", 1, true, 0, 1, inner.clone()),
            ],
        );
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[0].vars, vec![1, 1, 1]);
        assert_eq!(vm.counters.faults, 0);
        assert!(vm.instances[0].params.is_empty(), "both buffers consumed");
        assert_quiescent(&w);
        assert_round_trips(&w);
        // (d) A second call without a second push reads past the fresh buffer: the unchecked
        // read of VM-048, which answers 0 and lets the callback continue (8.1).
        let hourglass = vec![
            Instr::LoadImm {
                dst: tv(0),
                value: 1,
            },
            Instr::PushParam { src: tv(0) },
            call(1),
            Instr::LoadResult { dst: cv(0) },
            call(1),
            Instr::LoadResult { dst: cv(1) },
        ];
        let level = class(
            "StartUp",
            2,
            &[
                ("Hourglass", 1, false, 0, 1, hourglass),
                ("inner", 1, true, 0, 1, inner),
            ],
        );
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[0].vars, vec![1, 0]);
        assert_eq!(vm.fault, Some(Fault::UncheckedAccess(0x08)));
        assert_quiescent(&w);
        assert_round_trips(&w);
    }

    #[test]
    fn location_values_pack_points() {
        let v = location_of_point(1234, 567);
        assert!(v & LOCATION_POINT_BIT != 0);
        assert_eq!(crate::natives::unpack_point(v), Some((1234, 567)));
        assert_eq!(crate::natives::unpack_point(5), None);
        assert_eq!(location_of_point(-5, 40000), location_of_point(0, 0x7fff));
    }

    /// The values the settled rows of `docs/original/spec-script-vm.md` section 6 answer on a
    /// fresh mission, and the neutral values of the rows this engine still stubs, in strict
    /// mode and without a fault. The excluded ids of 4.2 are not called here: in strict mode
    /// they terminate the callback (`excluded_natives_are_a_fault_in_strict_mode`).
    #[test]
    fn settled_native_values_follow_the_specification() {
        // Elements: hero 0, guards 1 / 2, scroll 3, zone 4.
        let mut init = native(128, &[1], Some(cv(0)), 0);
        init.extend(native(240, &[1], Some(cv(1)), 0));
        init.extend(native(253, &[27], Some(cv(2)), 0));
        init.extend(native(255, &[1], Some(cv(3)), 0));
        init.extend(native(205, &[4, 0], Some(cv(4)), 0));
        init.extend(native(119, &[], Some(cv(5)), 0));
        init.extend(native(231, &[4], Some(cv(6)), 0));
        init.extend(native(246, &[4], Some(cv(7)), 0));
        init.extend(native(75, &[], Some(cv(8)), 0));
        init.extend(native(98, &[0, -1], Some(cv(9)), 0));
        init.extend(native(3, &[1], Some(cv(10)), 0));
        init.extend(native(10, &[1], Some(cv(11)), 0));
        init.extend(native(6, &[1], Some(cv(12)), 0));
        init.extend(native(86, &[1, 1], Some(cv(13)), 0));
        init.extend(native(86, &[1, 2], Some(cv(14)), 0));
        init.extend(native(85, &[NONE_HANDLE], Some(cv(15)), 0));
        init.extend(native(211, &[], Some(cv(16)), 0));
        init.extend(native(245, &[], Some(cv(17)), 0));
        init.extend(native(192, &[], Some(cv(18)), 0));
        init.extend(native(174, &[], Some(cv(19)), 0));
        init.extend(native(170, &[], Some(cv(20)), 0));
        init.extend(native(249, &[], Some(cv(21)), 0));
        init.extend(native(172, &[], Some(cv(22)), 0));
        init.extend(native(2, &[0], Some(cv(23)), 0));
        let level = class("StartUp", 24, &[("Initialize", 0, false, 0, 4, init)]);
        let w = mission_world(2, Some(program(vec![level], 2)));
        let vm = w.vm.as_ref().unwrap();
        assert!(
            !vm.faulted() && vm.counters.faults == 0,
            "{:?} {:?}",
            vm.fault,
            vm.counters
        );
        assert!(vm.counters.unknown_natives.is_empty());
        let v = &vm.instances[0].vars;
        assert_eq!(v[0], 1, "128: the NPC can act");
        assert_eq!(v[1], 1, "240: present");
        assert_eq!(v[2], 1, "253: a lost character with the skill (stub)");
        assert_eq!(v[3], 1, "255: a present character with the skill (stub)");
        assert_eq!(v[4], NONE_HANDLE, "205: element 4 is not a zone location");
        assert_eq!(v[5], 0, "119: no civilian is dead");
        assert_eq!((v[6], v[7]), (0, 0), "231 / 246: not a zone location");
        assert_eq!(v[8], 5, "75: the element table's full size");
        assert_eq!(v[9], 0, "98: this engine has no building interiors");
        assert_eq!((v[10], v[11]), (1, 1), "3 / 10: the table index both ways");
        assert_eq!(v[12], 1, "6: location 1 exists");
        assert_eq!((v[13], v[14]), (1, 0), "86: handle equality");
        assert_eq!(v[15], 1, "85: the null handle");
        assert_eq!(v[16], 0, "211: the leader is the hero");
        assert_eq!(v[17], 1, "245: one live player character");
        assert_eq!(v[18], NONE_HANDLE, "192: no scroll callback is running");
        assert_eq!(v[19], 5, "174: the team size limit (stub)");
        assert_eq!((v[20], v[21], v[22]), (0, 0, 0), "170 / 249 / 172");
        assert_eq!(v[23], -1, "2: an undeclared mission variable answers -1");
        for id in [253, 255, 174, 170, 172] {
            assert_eq!(
                vm.counters.stub_natives.get(&id),
                Some(&1),
                "stub {id} recorded"
            );
        }
        for id in [2, 3, 6, 10, 75, 85, 86, 192, 245, 249] {
            assert_eq!(
                crate::natives::native_kind(id),
                Some(crate::natives::Kind::Settled)
            );
            assert!(
                !vm.counters.stub_natives.contains_key(&id),
                "{id} is settled"
            );
        }
        for (id, _) in crate::natives::STUB_POLICY_VALUES {
            assert_eq!(
                crate::natives::native_kind(*id),
                Some(crate::natives::Kind::Stub)
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
        let read = |w: &mut World, id: u32, handle: i32| w.native_call(id, &[handle]);
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
        // dead, out of action and unable to act, all from one reading. Native 85 is "is null"
        // (VM-031), not a state of the actor.
        w.entities[1].ai_state = AiState::Dead;
        w.entities[1].alive = false;
        assert_eq!(
            (
                read(&mut w, 85, 1),
                read(&mut w, 87, 1),
                read(&mut w, 90, 1),
                read(&mut w, 128, 1)
            ),
            (0, 1, 1, 0)
        );
        assert_eq!(
            read(&mut w, 85, NONE_HANDLE),
            1,
            "85 answers the null handle"
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
            (0, 0, 0)
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
            crate::natives::native_kind(89),
            Some(crate::natives::Kind::Stub),
            "no tied-up state exists"
        );
        w.validate().unwrap();
    }

    /// `ActionChange(current_action, previous_action)` (VM-091) reaches the class bound to
    /// the actor whose action id changed, with that actor as the current actor (VM-093).
    #[test]
    fn action_changes_reach_the_actors_class() {
        use crate::ai::{AiState, actions};
        // ActionChange(current, previous): cv0 = previous, cv1 = current, cv2 += 1.
        let body = vec![
            Instr::LoadParam {
                dst: cv(0),
                offset: 4,
            },
            Instr::LoadParam {
                dst: cv(1),
                offset: 0,
            },
            Instr::LoadImm {
                dst: tv(0),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::AddInt,
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
        assert_eq!(
            vm.instances[1].vars[0],
            actions::IDLE as i32,
            "previous action"
        );
        assert_eq!(
            vm.instances[1].vars[1],
            actions::NOTICED as i32,
            "current action"
        );
        assert_eq!(vm.instances[1].vars[2], 1);
        for _ in 0..crate::ai::NOTICED_TICKS {
            w.step(&[]);
        }
        assert_eq!(w.entities[1].ai_state, AiState::Alarm);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[1].vars[0], actions::NOTICED as i32);
        assert_eq!(vm.instances[1].vars[1], actions::ALARM as i32);
        assert_eq!(vm.instances[1].vars[2], 2);
        assert_eq!(vm.counters.faults, 0);
        w.validate().unwrap();
    }

    /// Native 192 is the **current scroll** of VM-094, a handle with dynamic scope: null
    /// outside a scroll callback, the scroll for the duration of its `IsTaken`, and null again
    /// afterwards. Native 74 is the current actor of VM-093, which `IsTaken` inherits from its
    /// caller (the taking actor is only a parameter).
    #[test]
    fn the_current_scroll_is_set_for_the_duration_of_is_taken() {
        let mut init = native(192, &[], Some(cv(0)), 0);
        init.extend(native(74, &[], Some(cv(1)), 0));
        let mut taken = native(192, &[], Some(cv(2)), 0);
        taken.extend(native(74, &[], Some(cv(3)), 0));
        taken.push(Instr::LoadImm {
            dst: tv(0),
            value: 1,
        });
        taken.push(Instr::ReturnValue { src: tv(0) });
        let level = class("StartUp", 0, &[]);
        let mut scroll = class(
            "Scroll",
            4,
            &[
                ("Initialize", 0, false, 0, 4, init),
                ("IsTaken", 1, true, 0, 4, taken),
            ],
        );
        // Element 2 of `program(_, 1)` is the scroll.
        scroll.element = Some(2);
        let mut w = mission_world(1, Some(program(vec![level, scroll], 1)));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(
            &vm.instances[1].vars[..2],
            &[NONE_HANDLE, NONE_HANDLE],
            "no scroll callback is running at load"
        );
        assert_eq!(w.vm_is_taken(1, 0), Some(1));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[1].vars[2], 2, "192 is the scroll itself");
        assert_eq!(
            vm.instances[1].vars[3], NONE_HANDLE,
            "74 keeps the enclosing value"
        );
        assert_eq!(vm.current_scroll, NONE_HANDLE, "cleared afterwards");
        assert!(!vm.faulted());
        // Class F of VM-089: a scroll callback attempted while another scroll's callback runs
        // is not run at all and is recorded once per scroll (8.1).
        let mut ran = false;
        w.vm_with_current_scroll(9, |w| {
            assert_eq!(w.vm_is_taken(1, 0), None, "not run");
            ran = true;
        });
        assert!(ran);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.fault, Some(Fault::ScrollOverlap(2)));
        assert_eq!(vm.faults.len(), 1);
        assert_eq!(vm.current_scroll, NONE_HANDLE);
        // A second overlap of the same scroll records nothing new.
        w.vm_with_current_scroll(9, |w| {
            assert_eq!(w.vm_is_taken(1, 0), None);
        });
        assert_eq!(w.vm.as_ref().unwrap().faults.len(), 1);
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
        assert_eq!(vm.instances[0].vars, vec![5, 12, 0, 15]);
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
        hourglass.push(Instr::LoadImm {
            dst: tv(1),
            value: 2000,
        });
        hourglass.push(Instr::Binary {
            op: BinOp::SubInt,
            dst: tv(2),
            a: tv(0),
            b: tv(1),
        });
        hourglass.push(Instr::PushArg { src: tv(2) });
        hourglass.push(Instr::Native { id: 237 });
        let level = class(
            "StartUp",
            1,
            &[
                ("Initialize", 0, false, 0, 4, init),
                ("Hourglass", 1, false, 0, 4, hourglass),
            ],
        );
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        assert_eq!(w.vm.as_ref().unwrap().instances[0].vars[0], 100_000);
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
        let init = vec![Instr::LoadImm {
            dst: cv(0),
            value: 4,
        }];
        let hourglass = vec![
            Instr::LoadImm {
                dst: tv(0),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::SubInt,
                dst: cv(0),
                a: cv(0),
                b: tv(0),
            },
        ];
        let mut victory = native(28, &[1], None, 0);
        victory.push(Instr::ReturnValue { src: cv(0) });
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

    /// The taint model (ADR-0008, "Hypotheses and taint"): every call of a stub and of a
    /// partly modelled native records its assumption, read or not; the set is observable,
    /// hashed, snapshotted and validated, and a won mission stays recorded but tainted.
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
            offset: 0,
        }];
        let victory = vec![
            Instr::LoadImm {
                dst: tv(0),
                value: 1,
            },
            Instr::ReturnValue { src: tv(0) },
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
        assert_eq!(vm.instances[0].vars, vec![0, 1, 0]);
        // After load: every stub called is recorded, read or not (222 is a stub whose result
        // was not consumed; 221 and 253 were consumed), and the camera jump 20 and the banner
        // capture 178 are partly modelled rows.
        assert_eq!(
            vm.assumptions.iter().copied().collect::<Vec<_>>(),
            vec![
                Assumption::StubResult(221),
                Assumption::StubResult(222),
                Assumption::StubResult(253),
                Assumption::Policy(20),
                Assumption::Policy(178),
            ]
        );
        let obs = w.script_observation().unwrap();
        assert!(obs.tainted);
        assert_eq!(obs.assumptions.len(), 5);
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
            json.contains("\"assumptions\":[{\"stub_result\":221},{\"stub_result\":222},"),
            "{json}"
        );
        let snap: crate::world::Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = mission_world(1, None);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.hashes(), h);
        assert!(w2.script_observation().unwrap().tainted);
        for id in [3, 10] {
            let mut bad = w.snapshot(None);
            bad.world
                .vm
                .as_mut()
                .unwrap()
                .assumptions
                .insert(Assumption::StubResult(id));
            assert!(w2.restore(&bad).unwrap_err().contains("not a stub"));
        }
        // Reading the `Hourglass` time is no hypothesis of its own any more (ADR-0010: one
        // clock), so a script that only reads it stays clean.
        let level = class("StartUp", 3, &[("Hourglass", 1, false, 0, 4, hourglass)]);
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        assert!(!w.script_observation().unwrap().tainted);
        w.step(&[]);
        assert!(w.script_observation().unwrap().assumptions.is_empty());
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

    /// The native call protocol of `spec-script-vm.md` VM-086 / VM-087: the wrapper takes the
    /// row's arity off the instance's argument buffer (the last pushed is the last argument),
    /// converts the bool arguments, and leaves the row's result convention in the native
    /// result register - `void` 0, `bool` the low 8 bits, `int` / `handle` the full word. An
    /// arity-0 native takes nothing and the pushed cells stay in the buffer.
    #[test]
    fn the_native_wrapper_pops_converts_and_leaves_its_result() {
        use crate::natives::{Kind, ResultKind, native_kind, native_row};
        // Initialize: n0(0, 5); n2(0) -> cv0; n1(0, 0x1ff); n2(0) -> cv1;
        //             n86(1, 1) -> cv2; n206(0x1ff, 0x100) -> cv3; n85(-1) -> cv4.
        let mut init = native(0, &[0, 5], None, 0);
        init.extend(native(2, &[0], Some(cv(0)), 0));
        init.extend(native(1, &[0, 0x1ff], None, 0));
        init.extend(native(2, &[0], Some(cv(1)), 0));
        init.extend(native(86, &[1, 1], Some(cv(2)), 0));
        init.extend(native(206, &[0x1ff, 0x100], Some(cv(3)), 0));
        init.extend(native(85, &[NONE_HANDLE], Some(cv(4)), 0));
        let level = class("StartUp", 5, &[("Initialize", 0, false, 0, 4, init)]);
        let w = mission_world(1, Some(program(vec![level], 1)));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(
            vm.instances[0].vars,
            vec![5, 0x1ff, 1, 0x100, 1],
            "int results keep every bit, bool results the low byte"
        );
        assert!(
            vm.instances[0].args.is_empty(),
            "the wrapper popped its arity"
        );
        // Acceptance case 16: a `bool` row truncates to its low byte while an `int` row does
        // not. Native 46 answers 0 even when it recorded; 206 keeps the full width.
        assert_eq!(native_row(206).unwrap().result, ResultKind::Int);
        assert_eq!(native_row(85).unwrap().result, ResultKind::Bool);
        assert_eq!(native_row(0).unwrap().result, ResultKind::Void);
        // VM-087: an arity-0 native leaves the cells that were pushed for it in the buffer.
        let mut body = vec![
            Instr::LoadImm {
                dst: tv(0),
                value: 42,
            },
            Instr::PushArg { src: tv(0) },
            Instr::Native { id: 75 },
            Instr::LoadNativeResult { dst: cv(0) },
        ];
        body.push(Instr::PushArg { src: tv(0) });
        let level = class("StartUp", 1, &[("Initialize", 0, false, 1, 4, body)]);
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[0].vars[0], 3, "75: the table size");
        assert_eq!(vm.instances[0].args, vec![42, 42], "the cells stayed");
        assert!(
            vm.assumptions.contains(&Assumption::SnapshotSet),
            "a callback that leaves cells in the buffer depends on VM-088"
        );
        // A bool-converted argument reaches the native as 0 or 1 (VM-086): native 26's second.
        assert_eq!(native_row(26).unwrap().bool_args, 0b10);
        w.native_call(26, &[3, 77]);
        let vm = w.vm.as_ref().unwrap();
        assert!(vm.objectives[0].primary, "77 became 1");
        // Every id of the table has a row and a kind, and 264 is the last.
        assert!(native_row(264).is_some() && native_row(265).is_none());
        assert_eq!(native_kind(13), Some(Kind::Unknown));
        assert_eq!(native_kind(45), Some(Kind::Unresolved));
    }

    /// The twelve ids excluded from clearance (`spec-script-vm.md` 4.2) answer the
    /// placeholder the specification prescribes, record `Assumption::UnknownNative(id)` and
    /// let the callback run on; with `lenient_natives` the call is also logged with its
    /// arguments. The specification's optional strict fault is not implemented.
    #[test]
    fn excluded_natives_answer_their_prescribed_placeholder() {
        let body = |id: u32, args: &[i32]| {
            let mut v = native(id, args, Some(cv(0)), 0);
            v.push(Instr::LoadImm {
                dst: cv(1),
                value: 1,
            });
            v
        };
        // 173 answers 0, the callback runs on, and no fault is recorded.
        let level = class(
            "StartUp",
            2,
            &[("Initialize", 0, false, 0, 4, body(173, &[]))],
        );
        let w = mission_world(0, Some(program(vec![level], 0)));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[0].vars, vec![0, 1]);
        assert_eq!(vm.fault, None);
        assert!(vm.assumptions.contains(&Assumption::UnknownNative(173)));
        assert!(vm.unknown_calls.is_empty(), "the log needs lenient mode");
        // Lenient: the same, and the call is logged with its arguments.
        let level = class(
            "StartUp",
            2,
            &[("Initialize", 0, false, 0, 4, body(173, &[]))],
        );
        let w = mission_world_with(0, Some(program(vec![level], 0)), true);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[0].vars, vec![0, 1]);
        assert_eq!(
            vm.unknown_calls,
            vec![UnknownCall {
                id: 173,
                args: vec![]
            }]
        );
        // 13 inverts native 6 over the location table (the scripts' evident intent).
        let level = class(
            "StartUp",
            2,
            &[("Initialize", 0, false, 0, 4, body(13, &[1]))],
        );
        let w = mission_world(0, Some(program(vec![level], 0)));
        assert_eq!(w.vm.as_ref().unwrap().instances[0].vars, vec![1, 1]);
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
            Instr::JumpIfNonZero {
                cond: cv(0),
                target: 4,
            },
            Instr::LoadImm {
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
                offset: 4,
            },
            Instr::LoadParam {
                dst: cv(1),
                offset: 0,
            },
            Instr::LoadImm {
                dst: tv(0),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::AddInt,
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
        assert_eq!(vm.instances[1].vars, vec![0, 0, 0]);
        // The alert sequence the sighting started is recorded where it changed the guard's
        // state, before any handler ran (finding 1 of Codex review 9); the sighting itself is
        // inside the measured cone (a standing hero 120 px ahead) and records nothing; the
        // delivery adds the parameter-order hypothesis.
        assert!(
            !vm.assumptions.contains(&Assumption::SightCone)
                && vm.assumptions.contains(&Assumption::AlertPolicy)
                && !vm.assumptions.contains(&Assumption::KnockOut),
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
                vm.instances[1].vars,
                vec![actions::IDLE as i32, actions::NOTICED as i32, 1]
            );
            assert!(vm.pending_action_changes.is_empty());
        }
        assert_eq!(w.hashes(), w2.hashes());
        // The alarm follows: one more delivery, never a repeat of the first.
        for _ in 0..crate::ai::NOTICED_TICKS {
            w.step(&[]);
        }
        assert_eq!(w.entities[1].ai_state, AiState::Alarm);
        assert_eq!(
            w.vm.as_ref().unwrap().instances[1].vars,
            vec![actions::NOTICED as i32, actions::ALARM as i32, 2]
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
            victory.push(Instr::ReturnValue { src: tv(0) });
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

    /// The return semantics and the persistent result slot (`spec-script-vm.md` VM-047,
    /// VM-050, VM-071, VM-081; acceptance cases 1 and 2): `0x07` returns at once and writes
    /// both the callback return register and the caller frame's result slot, `0x0A` reads that
    /// slot, the slot keeps its value until the next `0x07` one frame deeper writes it, and a
    /// slot no `0x07` wrote reads 0 and records the departure.
    #[test]
    fn the_result_slot_is_persistent_and_zero_until_written() {
        // `seven`: 0x03; t0 = 7; 0x07 t0; t0 = 9; 0x07 t0; 0x06 - control does not continue,
        // so the caller reads 7 (acceptance case 1).
        let seven = vec![
            Instr::LoadImm {
                dst: tv(0),
                value: 7,
            },
            Instr::ReturnValue { src: tv(0) },
            Instr::LoadImm {
                dst: tv(0),
                value: 9,
            },
            Instr::ReturnValue { src: tv(0) },
        ];
        let three = vec![
            Instr::LoadImm {
                dst: tv(0),
                value: 3,
            },
            Instr::ReturnValue { src: tv(0) },
        ];
        // Initialize: call seven; 0x0A cv0; cv3 = 1 (a write between the two reads); 0x0A cv1;
        // call three; 0x0A cv2.
        let init = vec![
            call(1),
            Instr::LoadResult { dst: cv(0) },
            Instr::LoadImm {
                dst: cv(3),
                value: 1,
            },
            Instr::LoadResult { dst: cv(1) },
            call(2),
            Instr::LoadResult { dst: cv(2) },
        ];
        let level = class(
            "StartUp",
            4,
            &[
                ("Initialize", 0, false, 0, 1, init),
                ("seven", 0, true, 0, 1, seven),
                ("three", 0, true, 0, 1, three),
            ],
        );
        let base = program(vec![level], 0);
        base.validate().unwrap();
        let w = mission_world(0, Some(base.clone()));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(
            vm.instances[0].vars,
            vec![7, 7, 3, 1],
            "the first 0x07 wins, the slot persists, the next callee overwrites it"
        );
        assert_eq!(vm.counters.faults, 0);
        assert!(
            !vm.assumptions.contains(&Assumption::UnwrittenResultSlot),
            "every read followed a 0x07"
        );
        assert_quiescent(&w);
        // A callback that ends with 0x06 leaves the callback return register alone: the engine
        // reads what an earlier callback of the same instance left (VM-072, case 1).
        let mut w = mission_world(0, None);
        let mut vm = VmState::new(base, vec![], 9, false);
        vm.instances[0].callback_return = 5;
        vm.budget = WORK_BUDGET_AT_LOAD;
        w.vm = Some(vm);
        assert_eq!(
            w.vm_callback(0, callbacks::INITIALIZE, &[]),
            Some(CallOutcome::Returned(3)),
            "the last 0x07 executed is what the engine reads"
        );
        // An unwritten slot reads 0 and records the departure of VM-071.
        let level = class(
            "StartUp",
            1,
            &[(
                "Initialize",
                0,
                false,
                0,
                1,
                vec![Instr::LoadResult { dst: cv(0) }],
            )],
        );
        let w = mission_world(0, Some(program(vec![level], 0)));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[0].vars, vec![0]);
        assert!(vm.assumptions.contains(&Assumption::UnwrittenResultSlot));
    }

    /// A level whose `Initialize` is `init` and whose `CheckVictoryCondition` returns 1 at
    /// once: the win depends on whatever `init` executed.
    fn win_after(init: Vec<Instr>, guards: usize, lenient: bool) -> World {
        let victory = vec![
            Instr::LoadImm {
                dst: tv(0),
                value: 1,
            },
            Instr::ReturnValue { src: tv(0) },
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
                Instr::LoadImm {
                    dst: tv(0),
                    value: 3,
                },
                Instr::LoadImm {
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
            ("0x26 is settled", binary(BinOp::GeInt), false, vec![]),
            (
                "an unwritten result slot",
                vec![Instr::LoadResult { dst: cv(0) }],
                false,
                vec![Assumption::UnwrittenResultSlot],
            ),
            (
                "0xffff",
                vec![Instr::Jump {
                    target: END_OF_CALLBACK,
                }],
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
                "policy 93",
                native(93, &[1], Some(cv(0)), 0),
                false,
                vec![Assumption::Policy(93)],
            ),
            (
                "an excluded native in lenient mode",
                native(224, &[0, 1, 2, 3], Some(cv(0)), 0),
                true,
                vec![Assumption::UnknownNative(224)],
            ),
            (
                "an unresolved effect (4.3)",
                {
                    let mut v = native(30, &[], None, 0);
                    v.extend(native(59, &[1, 0, 0], None, 0));
                    v.extend(native(31, &[], None, 0));
                    v
                },
                false,
                vec![Assumption::UnresolvedEffect(59)],
            ),
            (
                "a stub without a result",
                native(180, &[1, 1], None, 0),
                false,
                vec![Assumption::StubResult(180)],
            ),
            (
                "a stub collected in a sequence",
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
                "a stub result consumed",
                native(221, &[1], Some(cv(0)), 0),
                false,
                vec![Assumption::StubResult(221)],
            ),
            (
                "the presentation stubs are stubs too",
                {
                    let mut v = native(30, &[], None, 0);
                    v.extend(native(69, &[1, 61], None, 0));
                    v.extend(native(31, &[], None, 0));
                    v.extend(native(149, &[3], None, 0));
                    v
                },
                false,
                vec![Assumption::StubResult(69), Assumption::StubResult(149)],
            ),
            (
                "240 is settled",
                native(240, &[2], Some(cv(0)), 0),
                false,
                vec![],
            ),
            (
                "211 is the engine's reading of the leader",
                native(211, &[], Some(cv(0)), 0),
                false,
                vec![Assumption::Policy(211)],
            ),
            (
                "56 outside a recording records nothing but an error",
                native(56, &[10], None, 0),
                false,
                vec![],
            ),
            (
                "settled natives only",
                {
                    let mut v = native(0, &[3, 4], None, 0);
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
            // The sentinel jump of VM-070 is a recorded departure, not an error.
            assert!(
                vm.faults
                    .iter()
                    .all(|f| matches!(f.fault, Fault::SentinelJump(..))),
                "{name}: {:?}",
                vm.faults
            );
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
        });
        let w = World::new_mission(Scenario::Mission("T".into()), 9, &spec).unwrap();
        assert_taint_round_trips(&w, &[Assumption::Policy(211)]);
        // Malformed assumptions are refused by `validate`.
        let mut w = win_after(vec![], 0, false);
        w.step(&[]);
        let h = w.hashes();
        for (bad, needle) in [
            (Assumption::Policy(3), "not a partly modelled"),
            (Assumption::Policy(999), "not a partly modelled"),
            (Assumption::StubResult(3), "not a stub"),
            (Assumption::UnresolvedEffect(3), "not an unresolved one"),
            (Assumption::AiEventCode(7), "not one of 100..106"),
            (Assumption::UnknownNative(3), "which is known"),
            (Assumption::UnknownNative(999), "which is known"),
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
            Instr::LoadImm {
                dst: tv(0),
                value: 1,
            },
            Instr::ReturnValue { src: tv(0) },
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
                offset: 0,
            },
            Instr::LoadImm {
                dst: tv(1),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::EqInt,
                dst: tv(2),
                a: tv(0),
                b: tv(1),
            },
            Instr::JumpIfNonZero {
                cond: tv(2),
                target: 6,
            },
            Instr::Return,
            Instr::LoadImm {
                dst: tv(0),
                value: 1,
            },
            Instr::PushArg { src: tv(0) },
            Instr::Native { id: 113 },
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
        // ActionChange: cv2 += 1; n237(77); n96(hero, location 0); n0(3, 9); spin.
        let mut body = vec![
            Instr::LoadImm {
                dst: tv(0),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::AddInt,
                dst: cv(2),
                a: cv(2),
                b: tv(0),
            },
        ];
        body.extend(native(237, &[77], None, 0));
        body.extend(native(96, &[0, 0], None, 0));
        body.extend(native(0, &[3, 9], None, 0));
        let spin = 1 + body.len() as u32;
        body.push(Instr::Jump { target: spin });
        let level = class("StartUp", 0, &[]);
        let mut w = noticing_world(level, &body, 1);
        let unchanged = |w: &World, tick: u64| {
            let vm = w.vm.as_ref().unwrap();
            assert_eq!(
                vm.instances[1].vars[2], 0,
                "tick {tick}: the increment was rolled back"
            );
            assert_eq!(vm.money, 0, "tick {tick}: the money was rolled back");
            assert!(vm.mission_vars.is_empty(), "tick {tick}: never declared");
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

    /// A backlog of action changes survives a snapshot taken between the tick that produced
    /// it and the tick that delivers it: after the restore both worlds deliver every change
    /// exactly once, on the same tick, with the same hashes.
    #[test]
    fn a_backlog_of_action_changes_restores_mid_way_and_delivers_once() {
        // Two guards notice the hero on the same tick; each guard's class counts its
        // deliveries in cv0. The changes are queued by `simulate` after the tick's script
        // phase, so a snapshot at the end of that tick carries both of them.
        let level = class("StartUp", 0, &[]);
        let body = vec![
            Instr::LoadImm {
                dst: tv(0),
                value: 1,
            },
            Instr::Binary {
                op: BinOp::AddInt,
                dst: cv(0),
                a: cv(0),
                b: tv(0),
            },
        ];
        let mut w = noticing_world(level, &body, 2);
        // The hero stands far away, so nothing the world does queues a change of its own.
        w.entities[0].x = Fixed::from_int(900);
        w.entities[0].y = Fixed::from_int(700);
        w.vm_queue_action_change(1, 0, 141);
        w.vm_queue_action_change(2, 0, 141);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.pending_action_changes.len(), 2, "both changes wait");
        assert_eq!(vm.instances[1].vars[0] + vm.instances[2].vars[0], 0);
        w.validate().unwrap();
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        let snap: crate::world::Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = mission_world(0, None);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.vm.as_ref().unwrap().pending_action_changes.len(), 2);
        // The next tick delivers the backlog, once per change, in both worlds.
        for world in [&mut w, &mut w2] {
            world.step(&[]);
            let vm = world.vm.as_ref().unwrap();
            assert!(vm.pending_action_changes.is_empty());
            assert_eq!((vm.instances[1].vars[0], vm.instances[2].vars[0]), (1, 1));
        }
        assert_eq!(w.hashes(), w2.hashes());
        for _ in 0..5 {
            w.step(&[]);
            w2.step(&[]);
        }
        assert_eq!(w.hashes(), w2.hashes());
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(
            (vm.instances[1].vars[0], vm.instances[2].vars[0]),
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
        // Sticky: a later fault keeps the first one; the fault survives a snapshot.
        w.native_call(161, &[0]);
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
        // The other faults name their class: the arithmetic trap of native 161 with n = 0
        // (VM-089 class T).
        let mut w = mission_world(0, Some(program(vec![class("StartUp", 0, &[])], 0)));
        assert_eq!(w.native_call(161, &[0]), 0);
        assert_eq!(w.vm.as_ref().unwrap().fault, Some(Fault::Trap));
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
                Instr::LoadImm {
                    dst: tv(0),
                    value: 1,
                },
                call(callee),
                Instr::LoadResult { dst: tv(0) },
                Instr::ReturnValue { src: tv(0) },
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
            vm.frames.is_empty() && vm.instances[0].params.is_empty(),
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
        // The same recursion in a queued handler: 8.1 rolls nothing back, so cv0 = 1 stands,
        // and the change is consumed (it would fail the same way again).
        let level = class("StartUp", 0, &[]);
        let mut handler = vec![Instr::LoadImm {
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
        assert_eq!(
            vm.instances[1].vars,
            vec![1],
            "what it did before the fault stands"
        );
        assert!(
            vm.pending_action_changes.is_empty(),
            "consumed, not retried"
        );
        assert!(vm.transaction.is_none());
        w.validate().unwrap();
    }

    /// The termination contract of `spec-script-vm.md` 8.1: a callback that overflows the
    /// call stack ends there and **keeps what it did** - nothing is rolled back and the engine
    /// goes on with its tick. `Hourglass` sets cv0 = 1, teleports the hero (native 96) and
    /// recurses to the frame limit; `CheckVictoryCondition` then reads cv0 and wins, the
    /// teleport stands, and the fault is sticky, hashed and restored.
    #[test]
    fn an_overflowing_hourglass_keeps_what_it_did_and_faults() {
        let recurse = |callee: u32| vec![call(callee)];
        let mut hourglass = vec![Instr::LoadImm {
            dst: cv(0),
            value: 1,
        }];
        hourglass.extend(native(96, &[0, 0], None, 0));
        hourglass.extend(recurse(2));
        let victory = vec![Instr::ReturnValue { src: cv(0) }];
        let level = class(
            "StartUp",
            1,
            &[
                ("Hourglass", 1, false, 0, 2, hourglass),
                ("CheckVictoryCondition", 0, true, 0, 1, victory),
                ("Recurse", 0, false, 0, 1, recurse(2)),
            ],
        );
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        let kept = |w: &World, tick: u64| {
            let vm = w.vm.as_ref().unwrap();
            assert_eq!(vm.instances[0].vars, vec![1], "tick {tick}: cv0 stands");
            assert_eq!(
                (w.entities[0].x.round(), w.entities[0].y.round()),
                (200, 200),
                "tick {tick}: the teleport stands"
            );
            assert!(vm.mission_won, "tick {tick}: the victory check read cv0");
            assert_eq!(vm.fault, Some(Fault::CallStackOverflow), "tick {tick}");
            assert!(vm.transaction.is_none(), "tick {tick}");
            assert!(
                vm.assumptions.is_empty(),
                "tick {tick}: {:?}",
                vm.assumptions
            );
        };
        w.step(&[]);
        kept(&w, 0);
        w.validate().unwrap();
        assert_quiescent(&w);
        let snap = w.snapshot(None);
        for t in 1..5 {
            w.step(&[]);
            kept(&w, t);
        }
        let mut w2 = mission_world(0, None);
        w2.restore(&snap).unwrap();
        for _ in 1..5 {
            w2.step(&[]);
        }
        kept(&w2, 4);
        assert_eq!(w2.hashes(), w.hashes());
        // The load-time callback: cv0 = 7; n237(77); the recursion. Both effects stand.
        let mut init = vec![Instr::LoadImm {
            dst: cv(0),
            value: 7,
        }];
        init.extend(native(237, &[77], None, 0));
        init.extend(recurse(1));
        let level = class(
            "StartUp",
            1,
            &[
                ("Initialize", 0, false, 0, 1, init),
                ("Recurse", 0, false, 0, 1, recurse(1)),
            ],
        );
        let w = mission_world(0, Some(program(vec![level], 0)));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[0].vars, vec![7]);
        assert_eq!(vm.money, 77);
        assert_eq!(vm.fault, Some(Fault::CallStackOverflow));
        assert!(vm.transaction.is_none());
        w.validate().unwrap();
    }

    /// The JSON form of `Fault::CallStackOverflow` in a snapshot.
    const CALL_STACK_OVERFLOW_JSON: &str = "\"fault\":\"call_stack_overflow\"";

    /// Acceptance case 15 (VM-061 - VM-068): the arithmetic of 3.1, including the unordered
    /// float comparisons, the 64-bit float-to-int truncation and the two traps of `0x1C`.
    #[test]
    fn acceptance_15_arithmetic() {
        let f = |v: f32| v.to_bits() as i32;
        let nan = f(f32::NAN);
        // Integer comparisons are signed and answer 1 / 0.
        assert_eq!(BinOp::LeInt.apply(2, 2), Ok(1));
        assert_eq!(BinOp::GeInt.apply(-1, 0), Ok(0));
        assert_eq!(BinOp::LtInt.apply(-1, 0), Ok(1));
        // Float comparisons answer a float, and a NaN operand is unordered (VM-068).
        assert_eq!(BinOp::LtFloat.apply(nan, f(1.0)), Ok(f(1.0)));
        assert_eq!(BinOp::NeFloat.apply(nan, f(1.0)), Ok(f(0.0)));
        assert_eq!(BinOp::GeFloat.apply(nan, nan), Ok(f(0.0)));
        assert_eq!(BinOp::LeFloat.apply(nan, nan), Ok(f(1.0)));
        assert_eq!(BinOp::EqFloat.apply(nan, nan), Ok(f(1.0)));
        assert_eq!(BinOp::GtFloat.apply(f(2.0), f(1.0)), Ok(f(1.0)));
        assert_eq!(BinOp::EqFloat.apply(f(1.0), f(1.0)), Ok(f(1.0)));
        // Float arithmetic reads and writes the cell's bits.
        assert_eq!(BinOp::AddFloat.apply(f(0.5), f(0.25)), Ok(f(0.75)));
        assert_eq!(BinOp::DivFloat.apply(f(1.0), f(4.0)), Ok(f(0.25)));
        // `0x17`: truncate toward zero into 64 bits, keep the low 32.
        assert_eq!(float_to_int(f(4_294_967_296.0)), 0);
        assert_eq!(float_to_int(f(2_147_483_648.0)), i32::MIN);
        assert_eq!(float_to_int(f(-1.5)), -1);
        assert_eq!(float_to_int(nan), 0);
        assert_eq!(float_to_int(f(f32::INFINITY)), 0);
        // `0x18`: int to float, round to nearest even.
        assert_eq!(3_f32.to_bits() as i32, f(3.0));
        // Integer arithmetic wraps; `0x1C` traps by zero and on `INT_MIN / -1`.
        assert_eq!(BinOp::AddInt.apply(i32::MAX, 1), Ok(i32::MIN));
        assert_eq!(BinOp::DivInt.apply(7, 2), Ok(3));
        assert_eq!(BinOp::DivInt.apply(-7, 2), Ok(-3));
        assert_eq!(BinOp::DivInt.apply(1, 0), Err(()));
        assert_eq!(BinOp::DivInt.apply(i32::MIN, -1), Err(()));
        // `0x15` on `INT_MIN` stays, `0x16` flips the sign bit of any float.
        assert_eq!(i32::MIN.wrapping_neg(), i32::MIN);
        assert_eq!(f(-0.0) ^ i32::MIN, f(0.0));
        // The trap terminates the callback where it stands (8.1).
        let init = vec![
            Instr::LoadImm {
                dst: tv(0),
                value: i32::MIN,
            },
            Instr::LoadImm {
                dst: tv(1),
                value: -1,
            },
            Instr::Binary {
                op: BinOp::DivInt,
                dst: cv(0),
                a: tv(0),
                b: tv(1),
            },
            Instr::LoadImm {
                dst: cv(1),
                value: 7,
            },
        ];
        let level = class("StartUp", 2, &[("Initialize", 0, false, 0, 4, init)]);
        let w = mission_world(0, Some(program(vec![level], 0)));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[0].vars, vec![0, 0]);
        assert_eq!(vm.fault, Some(Fault::Trap));
    }

    /// Acceptance case 3 (VM-095): a nested callback on the same instance. The inner `0x07`
    /// leaves its value in the callback return register, which the outer callback's `0x06`
    /// does **not** restore, and writes the outer frame's result slot, which a later callee
    /// overwrites.
    #[test]
    fn acceptance_3_nested_callback_leaves_its_return_register() {
        // Hourglass: n0(0, 1); n109(null, 5); n2(0) -> cv0; 0x06.
        // ProcessMessage: n0(0, 2); return 9.
        let mut hourglass = native(0, &[0, 1], None, 0);
        hourglass.extend(native(109, &[NONE_HANDLE, 5], None, 0));
        hourglass.extend(native(2, &[0], Some(cv(0)), 0));
        let mut message = native(0, &[0, 2], None, 0);
        message.push(Instr::LoadImm {
            dst: tv(0),
            value: 9,
        });
        message.push(Instr::ReturnValue { src: tv(0) });
        let level = class(
            "StartUp",
            1,
            &[
                ("Hourglass", 1, false, 0, 4, hourglass),
                ("ProcessMessage", 3, false, 0, 4, message),
            ],
        );
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(
            vm.instances[0].vars[0], 2,
            "the Hourglass read the value the nested callback wrote"
        );
        assert_eq!(
            vm.instances[0].callback_return, 9,
            "the inner 0x07 was not restored (VM-095)"
        );
        // Variant: an 0x0A in the outer callback after the nested call reads the inner value,
        // and a later callee of the outer frame overwrites it.
        let mut hourglass = native(109, &[NONE_HANDLE, 5], None, 0);
        hourglass.push(Instr::LoadResult { dst: cv(0) });
        hourglass.push(call(2));
        hourglass.push(Instr::LoadResult { dst: cv(1) });
        let message = vec![
            Instr::LoadImm {
                dst: tv(0),
                value: 9,
            },
            Instr::ReturnValue { src: tv(0) },
        ];
        let four = vec![
            Instr::LoadImm {
                dst: tv(0),
                value: 4,
            },
            Instr::ReturnValue { src: tv(0) },
        ];
        let level = class(
            "StartUp",
            2,
            &[
                ("Hourglass", 1, false, 0, 4, hourglass),
                ("ProcessMessage", 3, false, 0, 4, message),
                ("g", 0, true, 0, 4, four),
            ],
        );
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(
            vm.instances[0].vars,
            vec![9, 4],
            "the inner 0x07 wrote the outer frame's slot; the later callee overwrote it"
        );
    }

    /// Acceptance case 4 (VM-201): the barrier counter. Two consecutive barriers count once,
    /// a second native 30 is refused with 0 and changes nothing, and the 16-bit wrap is a
    /// deterministic fault (8.1).
    #[test]
    fn acceptance_4_barrier_counter() {
        // PostInitialize: n30() -> cv0; n32() -> cv1; n32() -> cv2; n56(5); n32() -> cv3; n31().
        let mut post = native(30, &[], Some(cv(0)), 0);
        post.extend(native(32, &[], Some(cv(1)), 0));
        post.extend(native(32, &[], Some(cv(2)), 0));
        post.extend(native(56, &[5], None, 0));
        post.extend(native(32, &[], Some(cv(3)), 0));
        post.extend(native(31, &[], Some(cv(4)), 0));
        let level = class("StartUp", 5, &[("PostInitialize", 0, false, 0, 4, post)]);
        let w = mission_world(0, Some(program(vec![level], 0)));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[0].vars, vec![1, 1, 1, 2, 1]);
        assert_eq!(vm.sequences.len(), 1, "one sequence with the timer");
        assert!(vm.collecting.is_none(), "native 31 closed the recording");
        // A second native 30 while one is open is an error returning 0, and the recording is
        // unchanged.
        let mut post = native(30, &[], Some(cv(0)), 0);
        post.extend(native(56, &[5], None, 0));
        post.extend(native(30, &[], Some(cv(1)), 0));
        let level = class("StartUp", 2, &[("PostInitialize", 0, false, 0, 4, post)]);
        let w = mission_world(0, Some(program(vec![level], 0)));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[0].vars, vec![1, 0]);
        let rec = vm.collecting.as_ref().expect("the recording stays open");
        assert_eq!(rec.level, 1);
        assert_eq!(rec.elements.len(), 1, "the timer only");
        // The wrap: a recording whose counter is at 65535 faults on the next increment.
        let mut w = mission_world(0, None);
        let mut vm = VmState::new(program(vec![class("StartUp", 0, &[])], 0), vec![], 9, false);
        vm.budget = WORK_BUDGET_AT_LOAD;
        vm.collecting = Some(Recording {
            elements: vec![SeqElement::Stub { id: 56 }],
            level: u16::MAX,
            last_level: u16::MAX,
            entered: BTreeSet::new(),
            provenance: Provenance::None,
        });
        w.vm = Some(vm);
        assert_eq!(w.native_call(32, &[]), 0);
        assert_eq!(w.vm.as_ref().unwrap().fault, Some(Fault::BarrierOverflow));
    }

    /// Acceptance case 5 (VM-203): a recording native outside a recording drops its element
    /// with an error, while native 109 delivers at once.
    #[test]
    fn acceptance_5_outside_a_recording() {
        // Initialize: n56(5); n43(null, 1); n109(null, 2).
        let mut init = native(56, &[5], None, 0);
        init.extend(native(43, &[NONE_HANDLE, 1], None, 0));
        init.extend(native(109, &[NONE_HANDLE, 2], None, 0));
        let message = vec![Instr::LoadParam {
            dst: cv(0),
            offset: 0,
        }];
        let level = class(
            "StartUp",
            1,
            &[
                ("Initialize", 0, false, 0, 4, init),
                ("ProcessMessage", 3, false, 0, 4, message),
            ],
        );
        let w = mission_world(0, Some(program(vec![level], 0)));
        let vm = w.vm.as_ref().unwrap();
        assert!(vm.sequences.is_empty(), "nothing was scheduled");
        assert!(vm.collecting.is_none());
        assert_eq!(
            vm.instances[0].vars[0], 2,
            "43 delivered nothing, 109 delivered message 2 at once"
        );
        assert_eq!(vm.counters.messages_delivered, 1);
        assert!(vm.counters.recording_dropped >= 2, "56 and 43 were dropped");
    }

    /// Acceptance case 14 (VM-089): the invalid inputs of section 6 answer their failure
    /// value and the script runs on, except where 8.1 terminates the callback.
    #[test]
    fn acceptance_14_invalid_inputs() {
        // The fixture: five elements (hero, scroll, zone in `program`), no carts.
        let mut w = mission_world(1, Some(program(vec![class("StartUp", 0, &[])], 1)));
        let n = w.vm.as_ref().unwrap().program.elements.len() as i32;
        assert_eq!(w.native_call(3, &[999]), NONE_HANDLE, "beyond the table");
        assert_eq!(w.native_call(3, &[n]), NONE_HANDLE, "no cart table");
        assert_eq!(w.native_call(3, &[NONE_HANDLE]), NONE_HANDLE, "silently");
        assert_eq!(w.native_call(2, &[99]), -1, "an undeclared variable");
        w.native_call(1, &[99, 5]);
        assert!(
            w.vm.as_ref().unwrap().mission_vars.is_empty(),
            "native 1 declares nothing"
        );
        assert_eq!(w.native_call(24, &[0, 444]), 0, "an unknown minimap code");
        assert_eq!(w.native_call(161, &[0]), 0, "the arithmetic trap answers 0");
        assert_eq!(w.vm.as_ref().unwrap().fault, Some(Fault::Trap));
        assert_eq!(w.native_call(8, &[-1]), -1, "8 never checks");
        assert_eq!(w.native_call(168, &[-1]), 0, "the container range error");
        assert_eq!(w.native_call(168, &[0]), 0);
        assert_eq!(w.native_call(75, &[]), n, "the full table size");
        assert_eq!(w.native_call(10, &[n]), -1, "not in the table");
        // The sentinel jump ends the callback (8.1, VM-070).
        let init = vec![
            Instr::Jump {
                target: END_OF_CALLBACK,
            },
            Instr::LoadImm {
                dst: cv(0),
                value: 1,
            },
        ];
        let level = class("StartUp", 1, &[("Initialize", 0, false, 0, 4, init)]);
        let w = mission_world(0, Some(program(vec![level], 0)));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[0].vars, vec![0]);
        assert!(matches!(vm.fault, Some(Fault::SentinelJump(0, 1))));
    }

    /// Acceptance cases 22 and 23 (VM-070 with VM-095): the sentinel jump pops the current
    /// frame and the popped frame's saved return address decides what follows - a script-call
    /// frame returns to its caller, an engine callback frame ends only that invocation. It is
    /// recorded once per class and address.
    #[test]
    fn acceptance_22_and_23_sentinel_jump() {
        // Case 22: `EnterZone` calls `f`; `f` jumps to the sentinel; `EnterZone` continues at
        // the instruction after its 0x05 and returns 4 to the engine.
        let enter = vec![
            call(1),
            Instr::LoadImm {
                dst: tv(0),
                value: 4,
            },
            Instr::ReturnValue { src: tv(0) },
        ];
        let f = vec![Instr::Jump {
            target: END_OF_CALLBACK,
        }];
        let level = class("StartUp", 0, &[]);
        let mut zone = class(
            "Zone",
            0,
            &[
                ("EnterZone", 1, true, 0, 1, enter),
                ("f", 0, false, 0, 1, f),
            ],
        );
        zone.zone = Some(1);
        let mut w = mission_world(0, Some(program(vec![level, zone], 0)));
        assert_eq!(w.vm_dispatch(1, callbacks::ENTER_ZONE, &[0]), Some(4));
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.faults.len(), 1);
        assert!(matches!(vm.faults[0].fault, Fault::SentinelJump(1, _)));
        // A second run records nothing new (once per class and address).
        w.vm_dispatch(1, callbacks::ENTER_ZONE, &[0]);
        assert_eq!(w.vm.as_ref().unwrap().faults.len(), 1);

        // Case 23: the sentinel at a nested callback's own depth ends only that invocation,
        // and the callback return register keeps the 9 the helper left there.
        let mut hourglass = vec![call(2)];
        hourglass.extend(native(109, &[NONE_HANDLE, 6], None, 0));
        hourglass.push(Instr::LoadImm {
            dst: cv(0),
            value: 1,
        });
        let helper = vec![
            Instr::LoadImm {
                dst: tv(0),
                value: 9,
            },
            Instr::ReturnValue { src: tv(0) },
        ];
        let message = vec![Instr::Jump {
            target: END_OF_CALLBACK,
        }];
        let level = class(
            "StartUp",
            1,
            &[
                ("Hourglass", 1, false, 0, 4, hourglass),
                ("ProcessMessage", 3, false, 0, 4, message),
                ("h", 0, true, 0, 4, helper),
            ],
        );
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(
            vm.instances[0].vars,
            vec![1],
            "the Hourglass ran to its end"
        );
        assert_eq!(
            vm.instances[0].callback_return, 9,
            "the nested invocation never wrote the register"
        );
    }

    /// Acceptance case 21 (8.1 class C): a callback name the class lacks is a no-op that
    /// records `Fault::MissingCallback` once per class and name, leaves both registers and the
    /// parameter buffer alone, and survives a snapshot with its suppression set.
    #[test]
    fn acceptance_21_missing_callback() {
        let level = class("StartUp", 0, &[]);
        let mut zone = class("Zone", 0, &[("EnterZone", 1, true, 0, 1, vec![])]);
        zone.zone = Some(1);
        let mut w = mission_world(0, Some(program(vec![level, zone], 0)));
        assert_eq!(w.vm_dispatch(1, callbacks::EXIT_ZONE, &[0]), None);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(
            vm.fault,
            Some(Fault::MissingCallback(
                1,
                callback_index(callbacks::EXIT_ZONE)
            ))
        );
        assert_eq!(vm.faults.len(), 1);
        assert!(
            vm.instances[1].params.is_empty(),
            "the parameters were never appended"
        );
        assert_eq!(vm.instances[1].callback_return, 0);
        w.vm_dispatch(1, callbacks::EXIT_ZONE, &[0]);
        assert_eq!(w.vm.as_ref().unwrap().faults.len(), 1, "recorded once");
        // The suppression set is snapshotted: the restored world records nothing new either.
        w.validate().unwrap();
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        let snap: crate::world::Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = mission_world(0, None);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.hashes(), w.hashes());
        w2.vm_dispatch(1, callbacks::EXIT_ZONE, &[0]);
        assert_eq!(w2.vm.as_ref().unwrap().faults.len(), 1);
    }

    /// Acceptance case 20 (8.1, termination contract point 4): a fault inside a nested
    /// callback terminates **only** the nested callback; the native that invoked it returns
    /// normally, the current actor is restored, and the outer callback runs to its end.
    #[test]
    fn acceptance_20_a_fault_in_a_nested_callback_terminates_only_it() {
        // Hourglass: n110(guard, 5, 0, 0); n74() -> cv1; cv0 = 1.
        let mut hourglass = native(110, &[1, 5, 0, 0], None, 0);
        hourglass.extend(native(74, &[], Some(cv(1)), 0));
        hourglass.push(Instr::LoadImm {
            dst: cv(0),
            value: 1,
        });
        // The guard's handler traps at once (native 161 with n = 0) and never writes cv0.
        let mut message = native(161, &[0], Some(cv(0)), 0);
        message.push(Instr::LoadImm {
            dst: cv(0),
            value: 7,
        });
        let level = class("StartUp", 2, &[("Hourglass", 1, false, 0, 4, hourglass)]);
        let mut guard = class("Guard", 1, &[("ProcessMessage", 3, false, 0, 4, message)]);
        guard.element = Some(1);
        let mut w = mission_world(1, Some(program(vec![level, guard], 1)));
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(vm.instances[1].vars, vec![0], "the nested callback ended");
        assert_eq!(vm.fault, Some(Fault::Trap));
        assert_eq!(vm.instances[0].vars[0], 1, "the Hourglass ran to its end");
        assert_eq!(
            vm.instances[0].vars[1], NONE_HANDLE,
            "the current actor was restored on the return"
        );
        assert_eq!(vm.current_actor, NONE_HANDLE);
        assert_eq!(vm.faults.len(), 1);
        assert_eq!(vm.faults[0].tick, 0);
        // The fault log survives a snapshot and the next tick adds one more entry.
        w.validate().unwrap();
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        let snap: crate::world::Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = mission_world(1, None);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.hashes(), w.hashes());
        for world in [&mut w, &mut w2] {
            world.step(&[]);
            assert_eq!(world.vm.as_ref().unwrap().faults.len(), 2);
        }
        assert_eq!(w.hashes(), w2.hashes());
    }

    /// Acceptance case 17 (VM-014, VM-088, VM-200, 8.3): the residue of both buffers, the
    /// native result register, the open recording and the mission variables survive a
    /// snapshot, and the next callback consumes them exactly as the original does - the
    /// engine's parameters are appended **after** the script-parameter residue.
    #[test]
    fn acceptance_17_restore_equivalence_of_the_snapshot_set() {
        // Hourglass: 0x02 t(7) (a parameter pushed and never consumed); 0x0B t(3) (an argument
        // cell left in the buffer); n30(); n56(3) (the recording stays open).
        let mut hourglass = vec![
            Instr::LoadImm {
                dst: tv(0),
                value: 7,
            },
            Instr::PushParam { src: tv(0) },
            Instr::LoadImm {
                dst: tv(1),
                value: 3,
            },
            Instr::PushArg { src: tv(1) },
        ];
        hourglass.extend(native(30, &[], None, 2));
        hourglass.extend(native(56, &[3], None, 2));
        // ProcessMessage reads four parameter cells: the residue first, then (7, 8, 9).
        let message = vec![
            Instr::LoadParam {
                dst: cv(0),
                offset: 0,
            },
            Instr::LoadParam {
                dst: cv(1),
                offset: 4,
            },
            Instr::LoadParam {
                dst: cv(2),
                offset: 8,
            },
            Instr::LoadParam {
                dst: cv(3),
                offset: 12,
            },
        ];
        let level = class(
            "StartUp",
            4,
            &[
                ("Hourglass", 1, false, 0, 4, hourglass),
                ("ProcessMessage", 3, false, 0, 4, message),
            ],
        );
        let mut w = mission_world(0, Some(program(vec![level], 0)));
        w.step(&[]);
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(
            vm.instances[0].params,
            vec![7],
            "the pushed parameter stayed"
        );
        assert_eq!(vm.instances[0].args, vec![3], "the pushed cell stayed");
        assert!(vm.collecting.is_some(), "the recording spans callbacks");
        assert!(vm.assumptions.contains(&Assumption::SnapshotSet));
        w.validate().unwrap();
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        let snap: crate::world::Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = mission_world(0, None);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.hashes(), w.hashes());
        // Both worlds deliver the same message and read the same four cells.
        for world in [&mut w, &mut w2] {
            // The work budget is a diagnostic, not state: a restored world starts a fresh one.
            world.vm.as_mut().unwrap().budget = WORK_BUDGET_PER_TICK;
            world.vm_deliver(Message {
                target: NONE_HANDLE,
                id: 7,
                arg: 8,
                arg2: 9,
            });
            assert_eq!(
                world.vm.as_ref().unwrap().instances[0].vars,
                vec![7, 7, 8, 9],
                "the residue is parameter 0 and the engine's follow (VM-088)"
            );
        }
        assert_eq!(w.hashes(), w2.hashes());
    }

    /// The native result register (VM-052 / VM-053): `0x0C` writes it, `0x0D` reads it, and
    /// the two are separate instructions, so a jump may land on either. A `0x0D` that no
    /// `0x0C` preceded reads whatever the register holds - 0 on a fresh instance.
    #[test]
    fn the_native_result_register_is_read_by_its_own_instruction() {
        // Initialize: cv0 = 5; if cv1 goto L; n236(); L: 0x0D cv0.
        let init = vec![
            Instr::LoadImm {
                dst: cv(0),
                value: 5,
            },
            Instr::JumpIfNonZero {
                cond: cv(1),
                target: 4,
            },
            Instr::Native { id: 236 },
            Instr::Nop,
            Instr::LoadNativeResult { dst: cv(0) },
        ];
        let level = class("StartUp", 2, &[("Initialize", 0, false, 0, 4, init)]);
        let p = program(vec![level], 0);
        p.validate().unwrap();
        let w = mission_world(0, Some(p.clone()));
        assert_eq!(
            w.vm.as_ref().unwrap().instances[0].vars,
            vec![0, 0],
            "236 answered the money, 0"
        );
        // With the branch taken the register was never written: the read answers 0, which is
        // what a fresh instance holds.
        let mut w = mission_world(0, None);
        let mut vm = VmState::new(p, vec![], 9, false);
        vm.instances[0].vars[1] = 1;
        vm.budget = WORK_BUDGET_AT_LOAD;
        w.vm = Some(vm);
        assert_eq!(
            w.vm_callback(0, callbacks::INITIALIZE, &[]),
            Some(CallOutcome::Returned(0))
        );
        assert_eq!(w.vm.as_ref().unwrap().instances[0].vars, vec![0, 1]);
    }
}
