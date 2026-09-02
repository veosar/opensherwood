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
//! ([`WORK_BUDGET_PER_TICK`]): instruction dispatch, native and call argument transfers, zone edge
//! tests, scroll range checks, sequence elements, and the path searches and smoothing of the walks
//! it issues (`nav.rs` charges node expansions and line-clear cells). When the budget is exhausted
//! the tick stops where it is (the running callback is aborted, the remaining phases are skipped
//! until the next tick, messages not yet delivered stay queued) and `counters.budget_aborts`
//! counts it; nothing panics and nothing loops on.
//!
//! Sequences. Elements that take time issue *tokens* ([`SeqToken`]): a walk (natives 45 / 48 /
//! 64) completes when the entity arrived, gave up or was ordered elsewhere; an animation
//! (natives 49..=53, not modelled) completes at once. Native 32 is a [`SeqElement::Barrier`] that
//! holds the sequence until every token issued since the previous barrier completed. Text pages
//! (native 203) and waits (native 56) hold the sequence directly. Camera moves (33 / 34) are
//! instant. Native 202 texts are never blocking.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::fixed::Fixed;
use crate::geom::point_in_polygon;
use crate::hash::Encoder;
use crate::rng::Rng;
use crate::world::{EntityKind, World};

/// Work units the VM may spend in one tick (all callbacks, zone tests, sequences and path
/// searches together); reset for every load-time, event and dismissal-time run as well. A unit is
/// one instruction, one transferred argument, one polygon edge test, one scroll range check, one
/// sequence element, one A* node expansion or one line-clear cell.
pub const WORK_BUDGET_PER_TICK: u64 = 1 << 22;
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

/// Distance (map pixels) within which a player character picks up a scroll (hypothesis).
pub const SCROLL_PICKUP_RADIUS: i64 = 24;
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
    /// `0x26` and `0x24`: `a >= b`.
    Ge,
    /// `0x27`: `a > b`.
    Gt,
    /// `0x28`: `a != b`.
    Ne,
    /// `0x29`: `a == b` (integers or handles).
    Eq,
    /// `0x2b`: `a < b` on fixed-point values.
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
            BinOp::Ge => i32::from(a >= b),
            BinOp::Gt => i32::from(a > b),
            BinOp::Ne => i32::from(a != b),
            BinOp::Eq => i32::from(a == b),
            BinOp::FixedLt => i32::from(Fixed::from_raw(a) < Fixed::from_raw(b)),
        }
    }
}

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
    /// Call a function of the same class (`0x05`); `argc` parameters were pushed.
    Call {
        /// Function index in the class table.
        function: u32,
        /// Parameters to pop.
        argc: u32,
    },
    /// Read the return value of the preceding call (`0x0a`).
    GetCallResult {
        /// Destination.
        dst: Slot,
    },
    /// Push an argument for the next [`Instr::Native`] (`0x0b`).
    PushArg {
        /// Value.
        src: Slot,
    },
    /// Call engine function `id` with `argc` pushed arguments (`0x0c`).
    Native {
        /// Native id.
        id: u32,
        /// Arguments to pop.
        argc: u32,
    },
    /// Read the result of the preceding native call (`0x0d`).
    GetNativeResult {
        /// Destination.
        dst: Slot,
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
            Instr::GetCallResult { .. } => 8,
            Instr::PushArg { .. } => 9,
            Instr::Native { .. } => 10,
            Instr::GetNativeResult { .. } => 11,
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
    /// A script polygon: location index.
    Polygon(u32),
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
    /// native / call arities within [`MAX_STACK`], bindings inside the tables, and element and
    /// location coordinates within `+-MAX_LOCATION_COORD`. The translator performs the same
    /// checks earlier for diagnostics; this is the trust boundary (a snapshot embeds the program).
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
                    Instr::Nop | Instr::Return => true,
                    Instr::Enter { locals, temps } => locals == f.locals && temps == f.temps,
                    Instr::SetResult { src }
                    | Instr::PushParam { src }
                    | Instr::PushArg { src } => slot_ok(src),
                    Instr::LoadParam { dst, index } => slot_ok(dst) && index < f.param_count,
                    Instr::GetCallResult { dst }
                    | Instr::GetNativeResult { dst }
                    | Instr::LoadInt { dst, .. }
                    | Instr::LoadFixed { dst, .. } => slot_ok(dst),
                    Instr::Call { function, argc } => {
                        argc as usize <= MAX_STACK
                            && c.functions
                                .get(function as usize)
                                .is_some_and(|callee| callee.param_count == argc)
                    }
                    Instr::Native { argc, .. } => argc as usize <= MAX_STACK,
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
        }
        let coord_ok = |v: i32| v.unsigned_abs() <= MAX_LOCATION_COORD as u32;
        for (i, el) in self.elements.iter().enumerate() {
            match *el {
                Element::Object { x, y } | Element::Scroll { x, y } => {
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

fn encode_slot(e: &mut Encoder, s: Slot) {
    e.u8(s.space.tag()).u32(s.index);
}

fn encode_instr(e: &mut Encoder, ins: &Instr) {
    e.u8(ins.tag());
    match *ins {
        Instr::Nop | Instr::Return => {}
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
        Instr::Call { function, argc } => {
            e.u32(function).u32(argc);
        }
        Instr::GetCallResult { dst } | Instr::GetNativeResult { dst } => encode_slot(e, dst),
        Instr::Native { id, argc } => {
            e.u32(id).u32(argc);
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
    /// Return value set by `SetResult`.
    pub result: i32,
    /// Return value of the last call made from this frame.
    pub call_result: i32,
    /// Result of the last native call made from this frame.
    pub native_result: i32,
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
    /// `(class, entity)` pairs currently within pickup range of the class's scroll (so `IsTaken`
    /// fires once per approach when the handler declines the pickup).
    #[serde(default)]
    pub scroll_presence: BTreeSet<(u32, u32)>,
    /// Program index (into `World::programs`) per `RAIL` index (native 9 / 132).
    pub paths: Vec<Option<u32>>,
    /// Lenient natives (`MissionSpec::lenient_natives`): an unknown native is a recorded no-op
    /// instead of a trap.
    pub lenient: bool,
    /// An unknown native was called in strict mode: the callback was stopped (sticky).
    pub faulted: bool,
    /// Unknown native calls in lenient mode, in order, with their arguments (bounded).
    pub unknown_calls: Vec<UnknownCall>,
    /// The `script` RNG stream (native 161).
    pub rng: Rng,
    /// Call stack (empty between callbacks; a snapshot must be quiescent).
    pub frames: Vec<Frame>,
    /// Native argument stack (empty between callbacks).
    pub arg_stack: Vec<i32>,
    /// Script call parameter stack (empty between callbacks).
    pub param_stack: Vec<i32>,
    /// Work units left in the current tick (not serialised: reset at the start of every tick and
    /// of every load-time, event and dismissal run).
    #[serde(skip)]
    pub budget: u64,
    /// Diagnostics (not serialised, not hashed).
    #[serde(skip)]
    pub counters: Counters,
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
            patches: BTreeSet::new(),
            actions: BTreeMap::new(),
            attributes: Vec::new(),
            states: BTreeMap::new(),
            inactive_elements: BTreeSet::new(),
            zone_presence: BTreeSet::new(),
            scroll_presence: BTreeSet::new(),
            paths,
            lenient,
            faulted: false,
            unknown_calls: Vec::new(),
            rng: Rng::new(seed, SCRIPT_RNG_STREAM),
            frames: Vec::new(),
            arg_stack: Vec::new(),
            param_stack: Vec::new(),
            budget: WORK_BUDGET_PER_TICK,
            counters: Counters::default(),
        }
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
            || self.scroll_presence.len() > MAX_QUEUE * 16
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
        for &(c, e) in &self.scroll_presence {
            if c as usize >= self.program.classes.len() || e as usize >= entity_count {
                return Err("vm scroll presence out of range".into());
            }
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
        self.rng.validate()
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
        e.u32(self.paths.len() as u32);
        for p in &self.paths {
            match p {
                Some(p) => e.u8(1).u32(*p),
                None => e.u8(0),
            };
        }
        e.u8(u8::from(self.lenient)).u8(u8::from(self.faulted));
        e.u32(self.unknown_calls.len() as u32);
        for c in &self.unknown_calls {
            e.u32(c.id).u32(c.args.len() as u32);
            for a in &c.args {
                e.i32(*a);
            }
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
        e.u32(self.scroll_presence.len() as u32);
        for (c, en) in &self.scroll_presence {
            e.u32(*c).u32(*en);
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
    if vm.budget < units {
        vm.budget = 0;
        false
    } else {
        vm.budget -= units;
        true
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
}

/// Outcome of one callback invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallOutcome {
    /// Ran to completion with this return value.
    Returned(i32),
    /// Aborted (budget, fault); the frames were discarded.
    Aborted,
}

/// Names of the engine callbacks the core invokes (`docs/formats/scb.md`, "Calling convention").
pub mod callbacks {
    /// Every class, at load.
    pub const INITIALIZE: &str = "Initialize";
    /// Level class, after every `Initialize`.
    pub const POST_INITIALIZE: &str = "PostInitialize";
    /// Every tick, `(time)`.
    pub const HOURGLASS: &str = "Hourglass";
    /// Level class, every tick; 1 = won.
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
    /// unknown-native policy (see `natives.rs`).
    pub fn attach_script(
        &mut self,
        program: Program,
        paths: Vec<Option<u32>>,
        lenient: bool,
    ) -> Result<(), String> {
        program.validate()?;
        let vm = VmState::new(program, paths, self.seed, lenient);
        vm.validate(self.programs.len(), self.entities.len())?;
        self.vm = Some(vm);
        self.vm_reset_budget();
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
    /// multi-page presentation can dismiss page after page without ticking the world. Returns
    /// whether a text was pending.
    pub fn vm_dismiss_text(&mut self) -> bool {
        let Some(vm) = self.vm.as_mut() else {
            return false;
        };
        if vm.texts.is_empty() {
            return false;
        }
        vm.texts.remove(0);
        self.vm_reset_budget();
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
            unknown_natives: vm.counters.unknown_natives.clone(),
            sequence_active: !vm.sequences.is_empty(),
            camera_target: vm.camera_target,
            debriefing: vm.debriefing,
            faulted: vm.faulted,
            lenient: vm.lenient,
            unknown_calls: vm.unknown_calls.len(),
        })
    }

    /// Hook: a scroll bound to a class was taken by `actor` (element handle). Not triggered by
    /// the engine yet.
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

    /// Hook: the actor of `class` changed its action state. Not triggered by the engine yet.
    pub fn vm_action_change(&mut self, class: u32, a: i32, b: i32) -> Option<i32> {
        self.vm_event(class, callbacks::ACTION_CHANGE, &[a, b])
    }

    fn vm_event(&mut self, class: u32, name: &str, params: &[i32]) -> Option<i32> {
        self.vm_reset_budget();
        match self.vm_callback(class, name, params) {
            Some(CallOutcome::Returned(v)) => Some(v),
            _ => None,
        }
    }

    fn vm_reset_budget(&mut self) {
        if let Some(vm) = self.vm.as_mut() {
            vm.budget = WORK_BUDGET_PER_TICK;
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
    /// the messages queued before this tick, `Hourglass(tick)` on every class, zone transitions
    /// of the player characters, scroll pickups, the active sequences, then
    /// `CheckVictoryCondition`. Every phase stops when the work budget is spent; undelivered
    /// messages stay queued (ahead of those sent this tick) for the next tick.
    pub(crate) fn vm_tick(&mut self) {
        if self.vm.is_none() {
            return;
        }
        self.vm_reset_budget();
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
        self.vm_scrolls();
        if self.vm_out_of_work() {
            return;
        }
        self.vm_advance_sequences();
        if self.vm_out_of_work() {
            return;
        }
        if let Some(CallOutcome::Returned(1)) = self.vm_callback(0, callbacks::CHECK_VICTORY, &[])
            && let Some(vm) = self.vm.as_mut()
        {
            vm.mission_won = true;
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
                if e.kind != EntityKind::Player || !e.alive || !e.active {
                    continue;
                }
                let cost = poly.len().max(1) as u64;
                if budget < cost {
                    budget = 0;
                    exhausted = true;
                    break 'scan;
                }
                budget -= cost;
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

    /// `IsTaken` for every player character coming within pickup range of an active scroll bound to
    /// a class. A handler that returns non-zero takes the scroll (it becomes inactive); one that
    /// returns zero leaves it, and the character has to walk away and back to try again. The pickup
    /// radius is a hypothesis (`SCROLL_PICKUP_RADIUS`); the original's rule is not observed yet.
    fn vm_scrolls(&mut self) {
        let Some(vm) = self.vm.as_ref() else { return };
        let mut budget = vm.budget;
        let mut exhausted = false;
        let mut events: Vec<(u32, u32, i32, bool)> = Vec::new();
        'scan: for (ci, c) in vm.program.classes.iter().enumerate() {
            let Some(handle) = c.element else { continue };
            let Some(Element::Scroll { x, y }) = vm.program.elements.get(handle as usize) else {
                continue;
            };
            let active = !vm.inactive_elements.contains(&(handle as i32));
            for (ei, e) in self.entities.iter().enumerate() {
                if e.kind != EntityKind::Player || !e.alive || !e.active {
                    continue;
                }
                if budget == 0 {
                    exhausted = true;
                    break 'scan;
                }
                budget -= 1;
                let dx = i64::from(e.x.round()) - i64::from(*x);
                let dy = i64::from(e.y.round()) - i64::from(*y);
                let near =
                    active && dx * dx + dy * dy <= SCROLL_PICKUP_RADIUS * SCROLL_PICKUP_RADIUS;
                let was = vm.scroll_presence.contains(&(ci as u32, ei as u32));
                if near != was {
                    events.push((ci as u32, ei as u32, handle as i32, near));
                }
            }
        }
        if let Some(vm) = self.vm.as_mut() {
            vm.budget = budget;
            if exhausted {
                inc(&mut vm.counters.budget_aborts);
            }
        }
        for (class, entity, handle, near) in events {
            if self.vm_out_of_work() {
                return;
            }
            let actor = self
                .vm
                .as_ref()
                .map_or(NONE_HANDLE, |vm| vm.program.element_of_entity(entity));
            if let Some(vm) = self.vm.as_mut() {
                if near {
                    vm.scroll_presence.insert((class, entity));
                } else {
                    vm.scroll_presence.remove(&(class, entity));
                }
            }
            if !near {
                continue;
            }
            let taken = matches!(self.vm_is_taken(class, actor), Some(v) if v != 0);
            if taken && let Some(vm) = self.vm.as_mut() {
                vm.inactive_elements.insert(handle);
            }
        }
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
            let el = {
                let Some(vm) = self.vm.as_mut() else {
                    return true;
                };
                if !charge(vm, 1) {
                    return false;
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
    /// unit per instruction plus one per argument transferred by a call or a native.
    pub(crate) fn vm_invoke(&mut self, class: u32, function: u32, params: &[i32]) -> CallOutcome {
        let Some(vm) = self.vm.as_mut() else {
            return CallOutcome::Aborted;
        };
        inc(&mut vm.counters.callbacks);
        if !vm.frames.is_empty() {
            // Callbacks never nest (natives queue events instead of invoking scripts).
            inc(&mut vm.counters.faults);
            vm.frames.clear();
        }
        vm.arg_stack.clear();
        vm.param_stack.clear();
        vm.collecting = None;
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
                vm.frames.clear();
                vm.collecting = None;
                return CallOutcome::Aborted;
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
                        vm.collecting = None;
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
                Instr::Call { function, argc } => {
                    let params = pop_n(&mut vm.param_stack, argc as usize, &mut vm.counters);
                    // The caller stays on the `Call`; `pop_frame` steps past it.
                    let class = vm.frames.last().map_or(0, |f| f.class);
                    if !push_frame(vm, class, function, params) {
                        advance(vm);
                    }
                }
                Instr::GetCallResult { dst } => {
                    let v = vm.frames.last().map_or(0, |f| f.call_result);
                    write(vm, dst, v);
                    advance(vm);
                }
                Instr::PushArg { src } => {
                    let v = read(vm, src);
                    if vm.arg_stack.len() < MAX_STACK {
                        vm.arg_stack.push(v);
                    }
                    advance(vm);
                }
                Instr::Native { id, argc } => {
                    let args = pop_n(&mut vm.arg_stack, argc as usize, &mut vm.counters);
                    advance(vm);
                    let Some(r) = self.native_call(id, &args) else {
                        // Unknown native in strict mode: a deterministic trap ends the
                        // callback here (its frames are discarded, the script is marked
                        // faulted).
                        if let Some(vm) = self.vm.as_mut() {
                            vm.frames.clear();
                            vm.collecting = None;
                            inc(&mut vm.counters.traps);
                        }
                        return CallOutcome::Aborted;
                    };
                    if let Some(f) = self.vm.as_mut().and_then(|vm| vm.frames.last_mut()) {
                        f.native_result = r;
                    }
                }
                Instr::GetNativeResult { dst } => {
                    let v = vm.frames.last().map_or(0, |f| f.native_result);
                    write(vm, dst, v);
                    advance(vm);
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
        call_result: 0,
        native_result: 0,
    });
    true
}

/// Pop the current frame; returns the value when the outermost frame returned.
fn pop_frame(vm: &mut VmState) -> Option<i32> {
    let done = vm.frames.pop()?;
    match vm.frames.last_mut() {
        Some(parent) => {
            parent.call_result = done.result;
            parent.pc = parent.pc.saturating_add(1);
            None
        }
        None => Some(done.result),
    }
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
        });
        if let Some(dst) = result {
            v.push(Instr::GetNativeResult { dst });
        }
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
            },
            Instr::GetCallResult { dst: cv(0) },
            Instr::LoadInt {
                dst: tv(0),
                value: 5,
            },
            Instr::PushArg { src: tv(0) },
            Instr::PushArg { src: cv(0) },
            Instr::Native { id: 1, argc: 2 },
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
        post.push(Instr::Native { id: 95, argc: 1 });
        post.push(Instr::GetNativeResult { dst: tv(1) });
        post.push(Instr::PushArg { src: tv(1) });
        post.push(Instr::Native { id: 34, argc: 1 });
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

    #[test]
    fn scrolls_fire_is_taken_once_per_approach_and_vanish_when_taken() {
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
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().class_vars[1], vec![0, 0]);
        // Walk in: the handler runs once and declines; standing still does not repeat it.
        w.entities[0].x = Fixed::from_int(700);
        w.entities[0].y = Fixed::from_int(710);
        w.step(&[]);
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().class_vars[1][0], 1);
        assert!(!w.vm.as_ref().unwrap().inactive_elements.contains(&2));
        // Leave, accept next time: the scroll is taken and inactive.
        w.entities[0].x = Fixed::from_int(100);
        w.step(&[]);
        w.vm.as_mut().unwrap().class_vars[1][1] = 1;
        w.entities[0].x = Fixed::from_int(700);
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().class_vars[1][0], 2);
        assert!(w.vm.as_ref().unwrap().inactive_elements.contains(&2));
        // An inactive scroll never fires again.
        w.entities[0].x = Fixed::from_int(100);
        w.step(&[]);
        w.entities[0].x = Fixed::from_int(700);
        w.step(&[]);
        assert_eq!(w.vm.as_ref().unwrap().class_vars[1][0], 2);
        w.validate().unwrap();
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
        assert!(!vm.faulted && vm.lenient);
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
        assert!(vm.faulted && !vm.lenient);
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
            },
            Instr::GetCallResult { dst: cv(0) },
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
                }
            },
            "instruction 3 out of range",
        );
        reject(
            |p| p.classes[0].code[1] = Instr::Jump { target: 7 },
            "instruction 1 out of range",
        );
        reject(
            |p| {
                p.classes[0].code[1] = Instr::Native {
                    id: 1,
                    argc: MAX_STACK as u32 + 1,
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
                    call_result: 0,
                    native_result: 0,
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

    #[test]
    fn location_values_pack_points() {
        let v = location_of_point(1234, 567);
        assert!(v & LOCATION_POINT_BIT != 0);
        assert_eq!(crate::natives::unpack_point(v), Some((1234, 567)));
        assert_eq!(crate::natives::unpack_point(5), None);
        assert_eq!(location_of_point(-5, 40000), location_of_point(0, 0x7fff));
    }
}
