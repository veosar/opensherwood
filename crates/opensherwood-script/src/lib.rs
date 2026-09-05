//! Translate a compiled mission script (`.scb`, `opensherwood_formats::scb`) into the core VM's
//! instruction set (`opensherwood_core::vm`, ADR-0008). This crate holds no execution logic and
//! no state: it maps opcodes, applies the calling convention, resolves the mission's index spaces
//! and validates every reference (`docs/formats/scb.md`).
//!
//! Choices for the low-confidence rows of the spec (each pinned by a test below, and each a
//! hypothesis source the VM records when the instruction executes: `vm::Assumption::Opcode`,
//! `UnresolvedJump`): `0x24` is `>=` (its Desperados name; `BinOp::GeLow`, distinct from the
//! medium-confidence `0x26`), `0x28` is `!=`, `0x2b` is a fixed-point `<`, a jump to `0xffff`
//! (two occurrences, an unresolved `break` in a switch) leaves the function
//! (`Instr::LeaveUnresolved`), and `0x14` immediates are rounded to 24.8 fixed point.
//!
//! Native call sites are checked against the core's signature table
//! (`opensherwood_core::natives::NATIVE_SIGNATURES`): a known native called with another number
//! of pushes than its arity, or a `0x0d` after a native that leaves no value (or after anything
//! but a `0x0c`), is a translation error. A `0x0c` followed by its `0x0d` is fused into one
//! `Instr::Native` carrying the result slot, and the `0x0d` quad becomes a `Nop` so that the
//! quad indices stay the instruction indices; a jump whose target is a `0x0d` quad is a
//! translation error (the corpus never does it, and the fused instruction has no separate
//! reader to land on: such a jump would skip the call). The ordinary call and its result read
//! are fused the same way (Codex review 9, finding 3): a `0x05` followed by its `0x0a` becomes
//! one `Instr::Call` carrying the destination, the `0x0a` quad becomes a `Nop`, a `0x0a` after
//! anything but a `0x05` or after a call of a function that returns no value is an error, and a
//! jump whose target is a `0x0a` quad is refused. `Program::validate` repeats the signature
//! checks in the core; here they name the class and quad.

use std::collections::BTreeMap;

use opensherwood_core::natives::native_signature;
use opensherwood_core::vm::{
    BinOp, Class, Element, Function, Instr, Location, Program, Slot, Space,
};
use opensherwood_formats::rhm::{ActorGroup, Mission};
use opensherwood_formats::rhp::Rhp;
use opensherwood_formats::scb::{self, Quad, Script, Storage};

/// Script ticks per second assumed for native 56 (`docs/formats/scb.md`, row 56).
pub const SCRIPT_TICKS_PER_SECOND: u32 = 25;

/// Why a script could not be translated.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TranslateError {
    /// No classes.
    #[error("script has no classes")]
    Empty,
    /// A structural problem in a class.
    #[error("class {class} ({name}): {what}")]
    Class {
        /// Class index.
        class: usize,
        /// Class name.
        name: String,
        /// Problem.
        what: String,
    },
    /// A problem at one instruction.
    #[error("class {class} ({name}) quad {quad}: {what}")]
    Quad {
        /// Class index.
        class: usize,
        /// Class name.
        name: String,
        /// Instruction index.
        quad: usize,
        /// Problem.
        what: String,
    },
    /// The binding's tick rate is zero or its script-tick scaling does not fit `u32`.
    #[error("tick rate {0}/{1} cannot be scaled to script ticks")]
    TickRate(u32, u32),
}

/// What the translator learnt about a script besides the program.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranslateReport {
    /// Classes whose name matched no mission element, polygon or rail point.
    pub unbound_classes: Vec<String>,
    /// Native call sites per id.
    pub native_calls: BTreeMap<u32, usize>,
    /// Call sites that read the result of a native the retail corpus never reads a result of
    /// (`NativeSignature::read_in_corpus` false while `returns_value` holds), per id: a
    /// diagnostic, not an error (the value is the native's contract).
    pub unobserved_result_reads: BTreeMap<u32, usize>,
    /// Largest immediate passed to native 3 (element by index), if any.
    pub max_element_immediate: Option<i32>,
}

/// The mission side of a translation: the index spaces of `docs/formats/scb.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissionBinding {
    /// The flat element table with the class name of every entry that has one.
    pub elements: Vec<(Option<String>, Element)>,
    /// Locations (`GULP` points then polygons) with the class names of the polygons.
    pub locations: Vec<(Option<String>, Location)>,
    /// Named rail points `(name, rail, point)`.
    pub rail_points: Vec<(String, u32, u32)>,
    /// World tick rate as a rational (Hz).
    pub tick_rate: (u32, u32),
}

/// Number of map elements that precede the mission's own records in the flat element table of
/// native 3: the map's `FLIM` entries (animated elements) followed by its `TUPO` entries (patches),
/// both counted from the parsed `.rhp` (`docs/formats/scb.md`, "Index spaces";
/// `docs/formats/sherwood-hub.md`, section 4).
#[must_use]
pub fn map_element_count(map: &Rhp) -> u32 {
    map.flims.len() as u32 + u32::from(map.tupo_count())
}

/// The per-map prefix of the nine retail maps as the mission records' self-references place it
/// (`docs/formats/sherwood-hub.md`, 4.1 and 4.2): a cross-check for [`map_element_count`] (the
/// data-backed test `tests/gamedata.rs` asserts the two agree), never an input of the binding.
/// `None` for a map that is not one of the nine.
#[must_use]
pub fn known_map_element_count(map: &str) -> Option<u32> {
    match map.to_ascii_lowercase().as_str() {
        "croisement01" => Some(19),
        "croisement02" | "croisement03" => Some(24),
        "derby" | "sherwood" => Some(20),
        "nottingham" => Some(59),
        "leicester" => Some(63),
        "lincoln" => Some(50),
        "york" => Some(70),
        _ => None,
    }
}

impl MissionBinding {
    /// Build the binding from a decoded mission. The table order is the one the retail scripts'
    /// self-references establish (`docs/formats/sherwood-hub.md`, 4.1 and 4.3) with the order of
    /// the `ZORG` and `SKRO` blocks fixed by `docs/original/h01-win-path.md` section 2 (the
    /// file's chunk order; every scroll-state call of the corpus lands in the scroll range only
    /// with `ZORG` first): `map_elements` map entries, then `POUF`, `OILE`, `TOTO`, `BORG`
    /// (actors), `BOOM` (objects), `ZORG` (pick-up items, inert: [`Element::Unmodelled`]),
    /// `SKRO` (scrolls), `TING` (inert), the `SCOT` player-character slots at the tail, and the
    /// script polygons after them (their position is not observable; no retail script addresses
    /// one through native 3). The entity numbering stays the app's
    /// actor list (actor groups in file order: `SCOT` first, then `OILE`, `TOTO`, `BORG`; objects
    /// skipped), so each group's entity ids are assigned in file order and the groups are then
    /// laid out in table order.
    #[must_use]
    pub fn from_mission(mission: &Mission, map_elements: u32, tick_rate: (u32, u32)) -> Self {
        let mut elements: Vec<(Option<String>, Element)> = Vec::new();
        for i in 0..map_elements {
            elements.push((None, Element::Map(i)));
        }
        let unmodelled = |elements: &mut Vec<(Option<String>, Element)>| {
            let index = elements.len() as u32;
            elements.push((None, Element::Unmodelled(index)));
        };
        for _ in &mission.tenants {
            unmodelled(&mut elements);
        }
        // Entity ids in file order; the groups are emitted in table order below.
        let mut entity = 0u32;
        let mut actor = |name: &Option<String>| {
            let e = (name.clone(), Element::Actor(entity));
            entity += 1;
            e
        };
        let mut player_characters = Vec::new();
        let mut civilians = Vec::new();
        let mut vips = Vec::new();
        let mut npcs = Vec::new();
        let mut objects = Vec::new();
        for group in &mission.actor_groups {
            match group {
                ActorGroup::PlayerCharacters { records, .. } => {
                    player_characters.extend(records.iter().map(|r| actor(&r.name)));
                }
                ActorGroup::Civilians { records, .. } => {
                    civilians.extend(records.iter().map(|r| actor(&r.name)));
                }
                ActorGroup::Vips { records, .. } => {
                    vips.extend(records.iter().map(|r| actor(&r.name)));
                }
                ActorGroup::Npcs { records, .. } => {
                    npcs.extend(records.iter().map(|r| actor(&r.name)));
                }
                ActorGroup::Objects { records, .. } => {
                    objects.extend(records.iter().map(|r| {
                        (
                            r.name.clone(),
                            Element::Object {
                                x: i32::from(r.x),
                                y: i32::from(r.y),
                            },
                        )
                    }));
                }
                ActorGroup::Meow { .. } | ActorGroup::Unknown { .. } => {}
            }
        }
        elements.extend(civilians);
        elements.extend(vips);
        elements.extend(npcs);
        elements.extend(objects);
        for _ in &mission.zorg {
            unmodelled(&mut elements);
        }
        elements.extend(mission.scrolls.iter().map(|s| {
            (
                s.name.clone(),
                Element::Scroll {
                    x: i32::from(s.placement.x),
                    y: i32::from(s.placement.y),
                },
            )
        }));
        for _ in &mission.mobiles {
            unmodelled(&mut elements);
        }
        elements.extend(player_characters);
        let mut locations: Vec<(Option<String>, Location)> = mission
            .script_areas
            .points
            .iter()
            .map(|p| {
                (
                    None,
                    Location::Point {
                        x: i32::from(p.x),
                        y: i32::from(p.y),
                    },
                )
            })
            .collect();
        for (i, poly) in mission.script_areas.polygons.iter().enumerate() {
            let index = (mission.script_areas.points.len() + i) as u32;
            elements.push((poly.name.clone(), Element::Polygon(index)));
            locations.push((
                poly.name.clone(),
                Location::Polygon(
                    poly.polygon
                        .points
                        .iter()
                        .map(|&(x, y)| (i32::from(x), i32::from(y)))
                        .collect(),
                ),
            ));
        }
        let mut rail_points = Vec::new();
        for (r, rail) in mission.rails.iter().enumerate() {
            for (p, point) in rail.iter().enumerate() {
                if let Some(name) = &point.name {
                    rail_points.push((name.clone(), r as u32, p as u32));
                }
            }
        }
        MissionBinding {
            elements,
            locations,
            rail_points,
            tick_rate,
        }
    }

    /// Number of actor elements (must equal the world's mission entity count).
    #[must_use]
    pub fn actor_count(&self) -> usize {
        self.elements
            .iter()
            .filter(|(_, e)| matches!(e, Element::Actor(_)))
            .count()
    }
}

/// Translate a script for a mission (see the crate documentation).
pub fn translate(script: &Script, binding: &MissionBinding) -> Result<Program, TranslateError> {
    translate_with_report(script, binding).map(|(p, _)| p)
}

/// [`translate`] returning what was learnt on the way.
pub fn translate_with_report(
    script: &Script,
    binding: &MissionBinding,
) -> Result<(Program, TranslateReport), TranslateError> {
    if script.classes.is_empty() {
        return Err(TranslateError::Empty);
    }
    let mut report = TranslateReport::default();
    let mut classes = Vec::with_capacity(script.classes.len());
    for (ci, c) in script.classes.iter().enumerate() {
        let mut class = translate_class(ci, c, &mut report)?;
        if ci > 0 {
            bind_class(&mut class, binding, &mut report);
        }
        classes.push(class);
    }
    let (num, den) = binding.tick_rate;
    let scaled = den
        .checked_mul(SCRIPT_TICKS_PER_SECOND)
        .filter(|&d| d > 0 && num > 0)
        .ok_or(TranslateError::TickRate(num, den))?;
    let program = Program {
        classes,
        elements: binding.elements.iter().map(|(_, e)| *e).collect(),
        locations: binding.locations.iter().map(|(_, l)| l.clone()).collect(),
        wait_scale: (num, scaled),
    };
    program.validate().map_err(|what| TranslateError::Class {
        class: 0,
        name: script.classes[0].name.clone(),
        what,
    })?;
    Ok((program, report))
}

fn bind_class(class: &mut Class, binding: &MissionBinding, report: &mut TranslateReport) {
    let name = class.name.as_str();
    class.element = binding
        .elements
        .iter()
        .position(|(n, _)| n.as_deref() == Some(name))
        .map(|i| i as u32);
    class.zone = binding
        .locations
        .iter()
        .position(|(n, l)| n.as_deref() == Some(name) && matches!(l, Location::Polygon(_)))
        .map(|i| i as u32);
    class.rail_point = binding
        .rail_points
        .iter()
        .find(|(n, _, _)| n == name)
        .map(|(_, r, p)| (*r, *p));
    if class.element.is_none() && class.zone.is_none() && class.rail_point.is_none() {
        report.unbound_classes.push(class.name.clone());
    }
}

fn class_err(ci: usize, c: &scb::Class, what: impl Into<String>) -> TranslateError {
    TranslateError::Class {
        class: ci,
        name: c.name.clone(),
        what: what.into(),
    }
}

fn quad_err(ci: usize, c: &scb::Class, quad: usize, what: impl Into<String>) -> TranslateError {
    TranslateError::Quad {
        class: ci,
        name: c.name.clone(),
        quad,
        what: what.into(),
    }
}

fn translate_class(
    ci: usize,
    c: &scb::Class,
    report: &mut TranslateReport,
) -> Result<Class, TranslateError> {
    if !c.size_of_variables.is_multiple_of(4) {
        return Err(class_err(
            ci,
            c,
            "variable block size is not a multiple of 4",
        ));
    }
    let variable_count = c.size_of_variables / 4;
    for v in &c.variables {
        if !v.offset.is_multiple_of(4) || v.offset / 4 >= variable_count {
            return Err(class_err(
                ci,
                c,
                format!("variable {} offset {} outside the block", v.name, v.offset),
            ));
        }
    }
    if c.functions.is_empty() {
        return Err(class_err(ci, c, "no functions"));
    }
    // Function table -> calling convention (`docs/formats/scb.md`): unknown_1 = return size,
    // unknown_2 = parameter block including the return slot, sizes verified against the
    // prologue quad at the address.
    let mut functions = Vec::with_capacity(c.functions.len());
    let mut last_address = 0u32;
    for (fi, f) in c.functions.iter().enumerate() {
        let Some(q) = c.quads.get(f.address as usize) else {
            return Err(class_err(
                ci,
                c,
                format!("function {} address {} out of range", f.name, f.address),
            ));
        };
        if fi > 0 && f.address <= last_address {
            return Err(class_err(
                ci,
                c,
                "functions are not laid out in table order",
            ));
        }
        last_address = f.address;
        if q.opcode != 0x03
            || u32::from(q.a) != f.size_of_volatile
            || u32::from(q.b) != f.size_of_tempor
        {
            return Err(class_err(
                ci,
                c,
                format!("function {} prologue does not match its sizes", f.name),
            ));
        }
        if !matches!(f.unknown_1, 0 | 4)
            || f.unknown_2 < f.unknown_1
            || !f.unknown_2.is_multiple_of(4)
        {
            return Err(class_err(
                ci,
                c,
                format!("function {} has an unexpected parameter layout", f.name),
            ));
        }
        if !f.size_of_volatile.is_multiple_of(4) || !f.size_of_tempor.is_multiple_of(4) {
            return Err(class_err(ci, c, format!("function {} frame sizes", f.name)));
        }
        functions.push(Function {
            name: f.name.clone(),
            address: f.address,
            param_count: (f.unknown_2 - f.unknown_1) / 4,
            has_result: f.unknown_1 == 4,
            locals: f.size_of_volatile / 4,
            temps: f.size_of_tempor / 4,
        });
    }
    // Jump targets, to check that no argument push straddles one and that none lands on a
    // result read (`0x0d` of a native, `0x0a` of a call), which is fused into the call before
    // it.
    let mut targets = vec![false; c.quads.len()];
    for (pc, q) in c.quads.iter().enumerate() {
        let target = match q.opcode {
            0x0e if q.a != 0xffff => Some(usize::from(q.a)),
            0x0f => Some(q.c as usize),
            _ => None,
        };
        if let Some(t) = target {
            if c.quads.get(t).is_some_and(|q| q.opcode == 0x0d) {
                return Err(quad_err(
                    ci,
                    c,
                    pc,
                    format!("jump target {t} is a native result read"),
                ));
            }
            if c.quads.get(t).is_some_and(|q| q.opcode == 0x0a) {
                return Err(quad_err(
                    ci,
                    c,
                    pc,
                    format!("jump target {t} is a call result read"),
                ));
            }
            if let Some(flag) = targets.get_mut(t) {
                *flag = true;
            }
        }
    }
    let mut code = Vec::with_capacity(c.quads.len());
    let mut fi = 0usize;
    let mut pushed_args = 0u32;
    let mut pushed_params = 0u32;
    for (pc, q) in c.quads.iter().enumerate() {
        while fi + 1 < functions.len() && functions[fi + 1].address as usize <= pc {
            fi += 1;
            if pushed_args != 0 || pushed_params != 0 {
                return Err(quad_err(
                    ci,
                    c,
                    pc,
                    "pushed arguments cross a function start",
                ));
            }
        }
        let f = &functions[fi];
        let range = f.address as usize
            ..functions
                .get(fi + 1)
                .map_or(c.quads.len(), |n| n.address as usize);
        if targets[pc] && (pushed_args != 0 || pushed_params != 0) {
            return Err(quad_err(
                ci,
                c,
                pc,
                "pushed arguments straddle a jump target",
            ));
        }
        let slot = |v: u16| -> Result<Slot, TranslateError> {
            let (storage, offset) = scb::operand(v);
            let space = match storage {
                Storage::ClassVar => Space::Class,
                Storage::Local => Space::Local,
                Storage::Temp => Space::Temp,
                Storage::None => {
                    return Err(quad_err(ci, c, pc, "operand is not a variable reference"));
                }
            };
            if !offset.is_multiple_of(4) {
                return Err(quad_err(ci, c, pc, "operand offset is not a multiple of 4"));
            }
            let index = u32::from(offset / 4);
            let limit = match space {
                Space::Class => variable_count,
                Space::Local => f.locals,
                Space::Temp => f.temps,
            };
            if index >= limit {
                return Err(quad_err(
                    ci,
                    c,
                    pc,
                    format!("slot {v:#06x} outside its block"),
                ));
            }
            Ok(Slot { space, index })
        };
        let target = |t: u32| -> Result<u32, TranslateError> {
            if range.contains(&(t as usize)) {
                Ok(t)
            } else {
                Err(quad_err(
                    ci,
                    c,
                    pc,
                    format!("jump target {t} outside the function"),
                ))
            }
        };
        let ins = match q.opcode {
            // 0x01: no-op (high).
            0x01 => Instr::Nop,
            // 0x02: push argument for the next 0x05 (high).
            0x02 => {
                pushed_params += 1;
                Instr::PushParam { src: slot(q.a)? }
            }
            // 0x03: prologue with the frame sizes (verified above).
            0x03 => Instr::Enter {
                locals: u32::from(q.a) / 4,
                temps: u32::from(q.b) / 4,
            },
            // 0x04: end of function; 0x06: return (high).
            0x04 | 0x06 => Instr::Return,
            // 0x05: call function at address `a` of the same class (high); the pushes since the
            // last call are its parameters and must match its table entry. A 0x0a directly
            // after it is fused into the call as its result slot; that function must return a
            // value.
            0x05 => {
                let Some(function) = functions.iter().position(|f| f.address == u32::from(q.a))
                else {
                    return Err(quad_err(
                        ci,
                        c,
                        pc,
                        format!("call target {} is not a function", q.a),
                    ));
                };
                let callee = &functions[function];
                if callee.param_count != pushed_params {
                    return Err(quad_err(
                        ci,
                        c,
                        pc,
                        format!(
                            "{} pushes for {} which takes {}",
                            pushed_params, callee.name, callee.param_count
                        ),
                    ));
                }
                let argc = pushed_params;
                pushed_params = 0;
                let read = c.quads.get(pc + 1).filter(|next| next.opcode == 0x0a);
                let dst = match read {
                    Some(next) => {
                        if !callee.has_result {
                            return Err(quad_err(
                                ci,
                                c,
                                pc + 1,
                                format!("reads the result of {}, which returns none", callee.name),
                            ));
                        }
                        Some(slot(next.a)?)
                    }
                    None => None,
                };
                Instr::Call {
                    function: function as u32,
                    argc,
                    dst,
                }
            }
            // 0x07: set the return value (high).
            0x07 => Instr::SetResult { src: slot(q.a)? },
            // 0x08: read parameter at byte offset c (high).
            0x08 => {
                if !q.c.is_multiple_of(4) {
                    return Err(quad_err(
                        ci,
                        c,
                        pc,
                        "parameter offset is not a multiple of 4",
                    ));
                }
                let index = q.c / 4;
                if index >= f.param_count {
                    return Err(quad_err(
                        ci,
                        c,
                        pc,
                        format!("parameter {index} beyond {} of {}", f.param_count, f.name),
                    ));
                }
                Instr::LoadParam {
                    dst: slot(q.a)?,
                    index,
                }
            }
            // 0x0a: read the return value of the preceding call (high): fused into the 0x05
            // before it, which must exist; the quad itself becomes a no-op (no jump targets it,
            // checked above).
            0x0a => {
                let call = pc.checked_sub(1).and_then(|p| c.quads.get(p));
                if call.is_none_or(|prev| prev.opcode != 0x05) {
                    return Err(quad_err(
                        ci,
                        c,
                        pc,
                        "reads a call result without a call before it",
                    ));
                }
                Instr::Nop
            }
            // 0x0b: push native argument (high).
            0x0b => {
                pushed_args += 1;
                Instr::PushArg { src: slot(q.a)? }
            }
            // 0x0c: native call `a` (high); arity = pushes since the last native call. A 0x0d
            // directly after it (every read of the corpus is) is fused into the call as its
            // result slot; that native must leave a value.
            0x0c => {
                let id = u32::from(q.a);
                *report.native_calls.entry(id).or_insert(0) += 1;
                if id == 3
                    && let Some(imm) = element_immediate(&c.quads, pc)
                {
                    report.max_element_immediate =
                        Some(report.max_element_immediate.map_or(imm, |m| m.max(imm)));
                }
                let argc = pushed_args;
                pushed_args = 0;
                let sig = native_signature(id);
                if let Some(sig) = sig
                    && sig.arity != argc
                {
                    return Err(quad_err(
                        ci,
                        c,
                        pc,
                        format!(
                            "native {id} called with {argc} arguments; its signature takes {}",
                            sig.arity
                        ),
                    ));
                }
                let read = c.quads.get(pc + 1).filter(|next| next.opcode == 0x0d);
                let dst = match read {
                    Some(next) => {
                        if let Some(sig) = sig {
                            if !sig.returns_value {
                                return Err(quad_err(
                                    ci,
                                    c,
                                    pc + 1,
                                    format!("reads the result of native {id}, which has none"),
                                ));
                            }
                            if !sig.read_in_corpus {
                                *report.unobserved_result_reads.entry(id).or_insert(0) += 1;
                            }
                        }
                        Some(slot(next.a)?)
                    }
                    None => None,
                };
                Instr::Native { id, argc, dst }
            }
            // 0x0d: read the native result (high): fused into the 0x0c before it, which must
            // exist; the quad itself becomes a no-op (no jump targets it, checked above).
            0x0d => {
                let call = pc.checked_sub(1).and_then(|p| c.quads.get(p));
                if call.is_none_or(|prev| prev.opcode != 0x0c) {
                    return Err(quad_err(
                        ci,
                        c,
                        pc,
                        "reads a native result without a native call before it",
                    ));
                }
                Instr::Nop
            }
            // 0x0e: jump to quad `a` (high); `0xffff` is an unresolved label: leave the function
            // (low; the VM records `Assumption::UnresolvedJump`).
            0x0e => {
                if q.a == 0xffff {
                    Instr::LeaveUnresolved
                } else {
                    Instr::Jump {
                        target: target(u32::from(q.a))?,
                    }
                }
            }
            // 0x0f: jump to `c` if `a` is non-zero (high).
            0x0f => Instr::JumpIf {
                cond: slot(q.a)?,
                target: target(q.c)?,
            },
            // 0x11 / 0x12: move (high / medium).
            0x11 | 0x12 => Instr::Move {
                dst: slot(q.a)?,
                src: slot(q.b)?,
            },
            // 0x13: int immediate (high).
            0x13 => Instr::LoadInt {
                dst: slot(q.a)?,
                value: q.c as i32,
            },
            // 0x14: float immediate (high), rounded to 24.8.
            0x14 => Instr::LoadFixed {
                dst: slot(q.a)?,
                value: fixed_of_f32(f32::from_bits(q.c)),
            },
            // 0x15: negate (high).
            0x15 => Instr::Neg {
                dst: slot(q.a)?,
                src: slot(q.b)?,
            },
            // 0x18: int to float (medium).
            0x18 => Instr::IntToFixed {
                dst: slot(q.a)?,
                src: slot(q.b)?,
            },
            // Three-operand arithmetic and comparisons; see the crate documentation for the
            // low-confidence rows.
            0x19..=0x2b => {
                let op = match q.opcode {
                    0x19 => BinOp::Add,
                    0x1a => BinOp::Sub,
                    0x1b => BinOp::Mul,
                    0x1d => BinOp::Or,
                    0x1e => BinOp::And,
                    0x22 => BinOp::FixedMul,
                    0x24 => BinOp::GeLow,
                    0x26 => BinOp::Ge,
                    0x25 => BinOp::Lt,
                    0x27 => BinOp::Gt,
                    0x28 => BinOp::Ne,
                    0x29 => BinOp::Eq,
                    0x2b => BinOp::FixedLt,
                    other => {
                        return Err(quad_err(ci, c, pc, format!("opcode {other:#04x} unknown")));
                    }
                };
                if q.c >> 16 != 0 {
                    return Err(quad_err(ci, c, pc, "third operand has high bits set"));
                }
                Instr::Binary {
                    op,
                    dst: slot(q.a)?,
                    a: slot(q.b)?,
                    b: slot((q.c & 0xffff) as u16)?,
                }
            }
            other => {
                return Err(quad_err(ci, c, pc, format!("opcode {other:#04x} unknown")));
            }
        };
        code.push(ins);
    }
    if pushed_args != 0 || pushed_params != 0 {
        return Err(class_err(ci, c, "pushed arguments at the end of the class"));
    }
    Ok(Class {
        name: c.name.clone(),
        variable_count,
        functions,
        code,
        element: None,
        zone: None,
        rail_point: None,
    })
}

/// The immediate loaded into the single argument of the native call at `pc`, when the argument
/// was loaded by a `0x13` directly before its push (the `n3(k)` idiom).
fn element_immediate(quads: &[Quad], pc: usize) -> Option<i32> {
    let push = quads.get(pc.checked_sub(1)?)?;
    let load = quads.get(pc.checked_sub(2)?)?;
    (push.opcode == 0x0b && load.opcode == 0x13 && load.a == push.a).then_some(load.c as i32)
}

/// Round an `f32` immediate to 24.8 fixed point (the retail immediates are 0.01, 0.5, 1, 2, 10, 30).
#[must_use]
pub fn fixed_of_f32(v: f32) -> opensherwood_core::Fixed {
    let scaled = (f64::from(v) * 256.0).round();
    let raw = scaled.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
    opensherwood_core::Fixed::from_raw(raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opensherwood_core::vm::{Assumption, SeqWait, callbacks};
    use opensherwood_core::{
        ActorSpec, Fixed, Geometry, MapInfo, MissionSpec, Scenario, Team, World,
    };
    use opensherwood_formats::scb::{Class as ScbClass, Function as ScbFunction, Quad, Script};

    const TV: u16 = 0xc000;
    const LV: u16 = 0x8000;
    const CV: u16 = 0x4000;

    fn q(opcode: u8, a: u16, b: u16, c: u32) -> Quad {
        Quad { opcode, a, b, c }
    }

    /// Three-operand form: the third operand is a `u16` slot in the low half of `c`.
    fn q3(opcode: u8, a: u16, b: u16, c: u16) -> Quad {
        q(opcode, a, b, u32::from(c))
    }

    /// Assemble a class from `(name, ret_size, param_bytes, volatile, tempor, body)` functions;
    /// every body gets its prologue and `end` quad.
    /// `(name, ret_size, param_bytes, volatile, tempor, body)` of a test function.
    type FnSpec<'a> = (&'a str, u32, u32, u32, u32, Vec<Quad>);

    fn class(name: &str, nvars: u32, fns: &[FnSpec<'_>]) -> ScbClass {
        let mut quads = Vec::new();
        let mut functions = Vec::new();
        for (fname, ret, params, vol, tmp, body) in fns {
            functions.push(ScbFunction {
                name: (*fname).into(),
                address: quads.len() as u32,
                unknown_0: 2.max(params / 4 + 1),
                unknown_1: *ret,
                unknown_2: params + ret,
                size_of_volatile: *vol,
                size_of_tempor: *tmp,
            });
            quads.push(q(0x03, *vol as u16, *tmp as u16, 0));
            quads.extend(body.iter().copied());
            quads.push(q(0x04, 0, 0, 0));
        }
        ScbClass {
            source_path: "script.scs".into(),
            name: name.into(),
            size_of_variables: nvars * 4,
            variables: Vec::new(),
            functions,
            quads,
        }
    }

    fn binding() -> MissionBinding {
        MissionBinding {
            elements: vec![
                (None, Element::Actor(0)),
                (Some("Guard".into()), Element::Actor(1)),
                (Some("Zone".into()), Element::Polygon(1)),
            ],
            locations: vec![
                (None, Location::Point { x: 300, y: 300 }),
                (
                    Some("Zone".into()),
                    Location::Polygon(vec![(0, 0), (50, 0), (50, 50), (0, 50)]),
                ),
            ],
            rail_points: vec![("Post".into(), 0, 1)],
            tick_rate: (60, 1),
        }
    }

    fn world(program: Program) -> World {
        let spec = MissionSpec {
            map: MapInfo {
                width: 1000,
                height: 800,
            },
            geometry: Geometry::default(),
            actors: vec![
                ActorSpec {
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
                },
                ActorSpec {
                    profile: "Soldier A00".into(),
                    team: Team::Enemy,
                    x: 200,
                    y: 200,
                    facing256: 0,
                    patrol: vec![],
                    program: vec![],
                    active: true,
                    hit_points: 100,
                    knockout_resistance: 0,
                },
            ],
            script: Some(program),
            rails: Vec::new(),
            lenient_natives: false,
            starting_money: 0,
            assumptions: std::collections::BTreeSet::new(),
        };
        World::new_mission(Scenario::Mission("T".into()), 1, &spec).unwrap()
    }

    fn native(id: u16, args: &[u32], result: Option<u16>) -> Vec<Quad> {
        let mut v = Vec::new();
        for (i, a) in args.iter().enumerate() {
            let t = TV + 4 * i as u16;
            v.push(q(0x13, t, 0, *a));
            v.push(q(0x0b, t, 0, 0));
        }
        v.push(q(0x0c, id, 0, 0));
        if let Some(dst) = result {
            v.push(q(0x0d, dst, 0, 0));
        }
        v
    }

    #[test]
    fn loop_with_branches_and_a_call_with_return_value() {
        // Initialize: sum = 0; for (i = 0; i < 5; i++) sum = sum + twice(i); n1(7, sum); cv4 = -1
        let init = vec![
            q(0x13, LV, 0, 0),                // 1: i = 0
            q(0x13, LV + 4, 0, 0),            // 2: sum = 0
            q(0x13, TV, 0, 5),                // 3: L: t0 = 5
            q3(0x25, TV + 4, LV, TV),         // 4: t1 = i < t0
            q(0x0f, TV + 4, 0, 7),            // 5: if t1 goto 7
            q(0x0e, 15, 0, 0),                // 6: goto 15
            q(0x02, LV, 0, 0),                // 7: push i
            q(0x05, 19, 0, 0),                // 8: call twice (@19)
            q(0x0a, TV + 8, 0, 0),            // 9: t2 = result
            q3(0x19, LV + 4, LV + 4, TV + 8), // 10: sum += t2
            q(0x13, TV, 0, 1),                // 11
            q3(0x19, LV, LV, TV),             // 12: i += 1
            q(0x01, 0, 0, 0),                 // 13
            q(0x0e, 3, 0, 0),                 // 14: goto L
            q(0x13, TV, 0, 7),                // 15
            q(0x0b, TV, 0, 0),                // 16
            q(0x0b, LV + 4, 0, 0),            // 17
            q(0x0c, 1, 0, 0),                 // 18: n1(7, sum)
        ];
        // twice(x): t0 = param0; t1 = 2; t2 = t0 * t1; return t2
        let twice = vec![
            q(0x08, TV, 0, 0),
            q(0x13, TV + 4, 0, 2),
            q3(0x1b, TV + 8, TV, TV + 4),
            q(0x07, TV + 8, 0, 0),
            q(0x01, 0, 0, 0),
            q(0x06, 0, 0, 0),
        ];
        let level = class(
            "StartUp",
            0,
            &[
                ("Initialize", 0, 0, 8, 12, init),
                ("twice", 4, 4, 0, 12, twice),
            ],
        );
        assert_eq!(
            level.functions[1].address,
            19 + 1,
            "layout: prologue + 18 + end = 20"
        );
        // Fix the call address to the real one.
        let mut level = level;
        let addr = level.functions[1].address as u16;
        for quad in &mut level.quads {
            if quad.opcode == 0x05 {
                quad.a = addr;
            }
        }
        let script = Script {
            version: 1.5,
            classes: vec![level],
        };
        let (program, report) = translate_with_report(&script, &binding()).unwrap();
        assert_eq!(report.native_calls.get(&1), Some(&1));
        assert_eq!(program.classes[0].functions[1].param_count, 1);
        assert!(program.classes[0].functions[1].has_result);
        // The call and its 0x0a are one instruction; the 0x0a quad is a no-op.
        assert_eq!(
            program.classes[0].code[8],
            Instr::Call {
                function: 1,
                argc: 1,
                dst: Some(Slot {
                    space: Space::Temp,
                    index: 2
                })
            }
        );
        assert_eq!(program.classes[0].code[9], Instr::Nop, "the fused 0x0a");
        let w = world(program);
        assert_eq!(w.vm.as_ref().unwrap().mission_vars[7], 20);
        assert_eq!(w.vm.as_ref().unwrap().counters.faults, 0);
    }

    #[test]
    fn natives_with_results_nested_calls_and_bindings() {
        // Guard.Initialize: cv0 = n10(n74()); n132(n3(cv0), n9(0)) — nested pushes interleave.
        let init = vec![
            q(0x0c, 74, 0, 0),
            q(0x0d, TV, 0, 0),
            q(0x0b, TV, 0, 0),
            q(0x0c, 10, 0, 0),
            q(0x0d, CV, 0, 0),
            q(0x0b, CV, 0, 0),
            q(0x0c, 3, 0, 0),
            q(0x0d, TV + 4, 0, 0),
            q(0x13, TV, 0, 0),
            q(0x0b, TV, 0, 0),
            q(0x0c, 9, 0, 0),
            q(0x0d, TV + 8, 0, 0),
            q(0x0b, TV + 4, 0, 0),
            q(0x0b, TV + 8, 0, 0),
            q(0x0c, 132, 0, 0),
        ];
        let level = class("StartUp", 0, &[("Initialize", 0, 0, 0, 0, vec![])]);
        let guard = class("Guard", 1, &[("Initialize", 0, 0, 0, 12, init)]);
        let zone = class("Zone", 0, &[("EnterZone", 4, 4, 0, 4, vec![])]);
        let post = class("Post", 0, &[("ReachPoint", 4, 4, 0, 4, vec![])]);
        let script = Script {
            version: 1.5,
            classes: vec![level, guard, zone, post],
        };
        let (program, report) = translate_with_report(&script, &binding()).unwrap();
        assert!(report.unbound_classes.is_empty());
        assert_eq!(program.classes[1].element, Some(1));
        assert_eq!(program.classes[2].element, Some(2));
        assert_eq!(program.classes[2].zone, Some(1));
        assert_eq!(program.classes[3].rail_point, Some((0, 1)));
        assert_eq!(program.wait_scale, (60, 25));
        // Quad 0 is the prologue.
        assert_eq!(
            program.classes[1].code[15],
            Instr::Native {
                id: 132,
                argc: 2,
                dst: None
            }
        );
        assert_eq!(
            program.classes[1].code[7],
            Instr::Native {
                id: 3,
                argc: 1,
                dst: Some(Slot {
                    space: Space::Temp,
                    index: 1
                })
            }
        );
        assert_eq!(program.classes[1].code[8], Instr::Nop, "the fused 0x0d");
        let w = world(program);
        assert_eq!(
            w.vm.as_ref().unwrap().class_vars[1][0],
            1,
            "n10(n74()) = own index"
        );
        assert_eq!(
            w.entities[1].program, None,
            "path 0 does not exist: no program"
        );
        assert_eq!(w.vm.as_ref().unwrap().counters.faults, 0);
    }

    #[test]
    fn low_confidence_opcodes_are_pinned() {
        // Initialize: t0 = 3; t1 = 2; cv0 = t0 op24 t1; cv1 = t1 op24 t0; cv2 = t0 op28 t1;
        // cv3 = t0 op28 t0; f0 = 0.5; f1 = float(t1) (2.0); cv4 = f0 op2b f1; cv5 = f1 op2b f0;
        // cv6 = f0 op22 f1 (1.0 -> 256)
        let init = vec![
            q(0x13, TV, 0, 3),
            q(0x13, TV + 4, 0, 2),
            q3(0x24, CV, TV, TV + 4),
            q3(0x24, CV + 4, TV + 4, TV),
            q3(0x28, CV + 8, TV, TV + 4),
            q3(0x28, CV + 12, TV, TV),
            q(0x14, LV, 0, 0.5f32.to_bits()),
            q(0x18, LV + 4, TV + 4, 0),
            q3(0x2b, CV + 16, LV, LV + 4),
            q3(0x2b, CV + 20, LV + 4, LV),
            q3(0x22, CV + 24, LV, LV + 4),
            q(0x0e, 0xffff, 0xffff, 0),
            q(0x13, CV + 28, 0, 99),
        ];
        let level = class("StartUp", 8, &[("Initialize", 0, 0, 8, 8, init)]);
        let script = Script {
            version: 1.5,
            classes: vec![level],
        };
        let program = translate(&script, &binding()).unwrap();
        assert_eq!(
            program.classes[0].code[12],
            Instr::LeaveUnresolved,
            "0x0e 0xffff leaves the function"
        );
        let w = world(program);
        // Every low-confidence reading executed is a recorded hypothesis source.
        assert_eq!(
            w.script_observation().unwrap().assumptions,
            vec![
                Assumption::Opcode(0x14),
                Assumption::Opcode(0x24),
                Assumption::Opcode(0x28),
                Assumption::Opcode(0x2b),
                Assumption::UnresolvedJump,
            ]
        );
        let vars = &w.vm.as_ref().unwrap().class_vars[0];
        assert_eq!(vars[0], 1, "0x24 is >=: 3 >= 2");
        assert_eq!(vars[1], 0, "0x24 is >=: 2 >= 3");
        assert_eq!(vars[2], 1, "0x28 is !=: 3 != 2");
        assert_eq!(vars[3], 0, "0x28 is !=: 3 != 3");
        assert_eq!(vars[4], 1, "0x2b is <: 0.5 < 2.0");
        assert_eq!(vars[5], 0, "0x2b is <: 2.0 < 0.5");
        assert_eq!(vars[6], Fixed::from_int(1).raw(), "0x22: 0.5 * 2.0");
        assert_eq!(vars[7], 0, "the 0xffff jump left the function");
        assert_eq!(fixed_of_f32(0.01), Fixed::from_raw(3));
        assert_eq!(fixed_of_f32(30.0), Fixed::from_int(30));
    }

    #[test]
    fn sequence_with_texts_wait_and_camera_through_the_bytecode() {
        // PostInitialize: n26(0,1); n30(); n203(0); n32(); n56(25); n32(); n34(n95(n211())); n31()
        let mut post = native(26, &[0, 1], None);
        post.extend(native(30, &[], None));
        post.extend(native(203, &[0], None));
        post.extend(native(32, &[], None));
        post.extend(native(56, &[25], None));
        post.extend(native(32, &[], None));
        post.extend(native(211, &[], Some(TV)));
        post.push(q(0x0b, TV, 0, 0));
        post.push(q(0x0c, 95, 0, 0));
        post.push(q(0x0d, TV + 4, 0, 0));
        post.push(q(0x0b, TV + 4, 0, 0));
        post.push(q(0x0c, 34, 0, 0));
        post.extend(native(31, &[], None));
        let level = class(
            "StartUp",
            0,
            &[
                ("Initialize", 0, 0, 0, 0, vec![]),
                ("PostInitialize", 0, 0, 0, 8, post),
            ],
        );
        let script = Script {
            version: 1.5,
            classes: vec![level],
        };
        let program = translate(&script, &binding()).unwrap();
        let mut w = world(program);
        let obs = w.script_observation().unwrap();
        assert_eq!(obs.texts, vec![0]);
        assert!(obs.sequence_active);
        assert_eq!(obs.objectives.len(), 1);
        assert!(w.vm_dismiss_text());
        let vm = w.vm.as_ref().unwrap();
        assert_eq!(
            vm.sequences[0].wait,
            SeqWait::Ticks(60),
            "25 script ticks = 60 world ticks"
        );
        for _ in 0..60 {
            w.step(&[]);
        }
        let obs = w.script_observation().unwrap();
        assert!(!obs.sequence_active);
        assert_eq!(obs.camera_target, Some((100, 100)));
        assert!(
            w.vm.as_ref().unwrap().program.classes[0]
                .function(callbacks::POST_INITIALIZE)
                .is_some()
        );
    }

    #[test]
    fn invalid_scripts_are_refused() {
        let ok = class(
            "StartUp",
            1,
            &[("Initialize", 0, 0, 0, 4, native(2, &[1], Some(TV)))],
        );
        let script = |c: ScbClass| Script {
            version: 1.5,
            classes: vec![c],
        };
        translate(&script(ok.clone()), &binding()).unwrap();
        // Jump outside the function.
        let mut bad = ok.clone();
        bad.quads[1] = q(0x0e, 40, 0, 0);
        assert!(matches!(
            translate(&script(bad), &binding()),
            Err(TranslateError::Quad { quad: 1, .. })
        ));
        // Temp slot beyond the frame.
        let mut bad = ok.clone();
        bad.quads[1] = q(0x13, TV + 8, 0, 1);
        assert!(translate(&script(bad), &binding()).is_err());
        // Call with the wrong parameter count (one push for a function without parameters).
        let mut bad = ok.clone();
        bad.quads[1] = q(0x02, TV, 0, 0);
        bad.quads[2] = q(0x05, 0, 0, 0);
        assert!(
            translate(&script(bad), &binding())
                .unwrap_err()
                .to_string()
                .contains("pushes")
        );
        // Prologue mismatch.
        let mut bad = ok.clone();
        bad.functions[0].size_of_tempor = 8;
        assert!(
            translate(&script(bad), &binding())
                .unwrap_err()
                .to_string()
                .contains("prologue")
        );
        // Unknown opcode.
        let mut bad = ok.clone();
        bad.quads[1] = q(0x09, 0, 0, 0);
        assert!(translate(&script(bad), &binding()).is_err());
        // Parameter read beyond the count.
        let mut bad = ok.clone();
        bad.quads[1] = q(0x08, TV, 0, 4);
        assert!(translate(&script(bad), &binding()).is_err());
        // A native called with the wrong number of pushes (n237 takes one), a result read
        // after a native without one (n237 again) and a `0x0d` with no call before it.
        let mut bad = ok.clone();
        bad.quads[2] = q(0x0c, 237, 0, 0);
        bad.quads[3] = q(0x01, 0, 0, 0);
        let err = translate(&script(bad), &binding()).unwrap_err().to_string();
        assert!(err.contains("native 237 called with 0"), "{err}");
        let mut bad = ok.clone();
        bad.quads[3] = q(0x0c, 237, 0, 0);
        let err = translate(&script(bad), &binding()).unwrap_err().to_string();
        assert!(err.contains("237") && err.contains("has none"), "{err}");
        let mut bad = ok;
        bad.quads[1] = q(0x0d, TV, 0, 0);
        bad.quads[2] = q(0x01, 0, 0, 0);
        bad.quads[3] = q(0x01, 0, 0, 0);
        bad.quads[4] = q(0x01, 0, 0, 0);
        let err = translate(&script(bad), &binding()).unwrap_err().to_string();
        assert!(err.contains("without a native call"), "{err}");
        assert!(matches!(
            translate(
                &Script {
                    version: 1.5,
                    classes: vec![]
                },
                &binding()
            ),
            Err(TranslateError::Empty)
        ));
    }

    #[test]
    fn tick_rate_scaling_is_checked() {
        let level = class("StartUp", 0, &[("Initialize", 0, 0, 0, 0, vec![])]);
        let script = Script {
            version: 1.5,
            classes: vec![level],
        };
        for rate in [(60, u32::MAX), (0, 1), (60, 0), (60, u32::MAX / 25 + 1)] {
            let mut b = binding();
            b.tick_rate = rate;
            assert_eq!(
                translate(&script, &b).unwrap_err(),
                TranslateError::TickRate(rate.0, rate.1),
                "{rate:?}"
            );
        }
        let mut b = binding();
        b.tick_rate = (u32::MAX, u32::MAX / 25);
        let program = translate(&script, &b).unwrap();
        assert_eq!(program.wait_scale, (u32::MAX, (u32::MAX / 25) * 25));
    }

    #[test]
    fn known_map_table() {
        assert_eq!(known_map_element_count("Lincoln"), Some(50));
        assert_eq!(known_map_element_count("Croisement03"), Some(24));
        assert_eq!(known_map_element_count("sherwood"), Some(20));
        assert_eq!(known_map_element_count("Sherwood"), Some(20));
        assert_eq!(known_map_element_count("nowhere"), None);
    }

    /// The element table of `docs/formats/sherwood-hub.md` 4.1 with the `ZORG` / `SKRO` order of
    /// `docs/original/h01-win-path.md` 2: map entries, `POUF`, `OILE`, `TOTO`, `BORG`, `BOOM`,
    /// `ZORG`, `SKRO`, `TING`, then the `SCOT` slots and the polygons,
    /// while the entity ids keep the app's file order (`SCOT` first).
    #[test]
    fn mission_binding_puts_the_player_slots_after_the_inert_entries() {
        use opensherwood_formats::rhm::{
            Brains, Civilian, Header, Mobile, Npc, Object, Placement, PlayerCharacter, Polygon,
            ScriptAreas, ScriptPolygon, Scroll, Tenant, Vip, ZorgEntry,
        };
        let placement = Placement {
            x: 10,
            y: 20,
            ..Placement::default()
        };
        let pc = |name: Option<&str>| PlayerCharacter {
            placement,
            unknown_0x12: 0,
            unknown_0x16: [0; 10],
            name: name.map(str::to_string),
            unknown_trailer: 0,
        };
        let npc = |name: Option<&str>| Npc {
            placement,
            unknown_0x12: 0,
            profile: 0,
            unknown_0x1a: 0,
            unknown_0x1b: 0,
            unknown_0x1f: 0,
            unknown_0x23: 0,
            members: Vec::new(),
            rail: -1,
            unknown_i16: -1,
            name: name.map(str::to_string),
        };
        let mission = Mission {
            version: 2,
            header: Header {
                version: 4,
                map_id: 100,
                variant: 1,
                map: "Croisement01".into(),
                mission_id: 1,
            },
            tenants: vec![Tenant {
                sprite: "Trap".into(),
                label: String::new(),
                body: Vec::new(),
            }],
            actor_groups: vec![
                ActorGroup::PlayerCharacters {
                    version: 4,
                    records: vec![pc(Some("hero_80000001")), pc(None)],
                },
                ActorGroup::Civilians {
                    version: 3,
                    records: vec![Civilian {
                        placement,
                        unknown_0x12: 0,
                        profile: 1,
                        unknown_i16_a: -1,
                        unknown_i16_b: 0,
                        unknown_u16: 0,
                        lists: None,
                        name: None,
                    }],
                },
                ActorGroup::Vips {
                    version: 2,
                    records: vec![Vip {
                        placement,
                        unknown_0x12: 0,
                        profile: 1,
                        unknown_i16_a: 0,
                        unknown_i16_b: 0,
                        name: None,
                    }],
                },
                ActorGroup::Npcs {
                    version: 4,
                    records: vec![npc(None), npc(Some("guard_80000002"))],
                },
                ActorGroup::Objects {
                    version: 5,
                    records: vec![Object {
                        x: 5,
                        y: 6,
                        unknown_0x04: -1,
                        unknown_0x06: 0,
                        unknown_0x08: 0,
                        unknown_0x0a: 0,
                        unknown_0x0c: 0,
                        unknown_0x0e: -1,
                        unknown_0x10: 0,
                        unknown_0x12: 0,
                        sprite: "TG_x".into(),
                        label: String::new(),
                        unknown_flags: 1,
                        x2: 5,
                        y2: 6,
                        unknown_q2: 0,
                        unknown_r2: 0,
                        polygon: Polygon {
                            unknown_a: 0,
                            points: Vec::new(),
                            unknown_b: 0,
                        },
                        unknown_u8: 1,
                        name: Some("target_80000003".into()),
                    }],
                },
            ],
            zorg: vec![ZorgEntry {
                unknown_a: 12,
                unknown_b: 1,
                placement,
            }],
            brains: Brains::default(),
            rails: Vec::new(),
            scrolls: vec![Scroll {
                placement,
                unknown_flags: [1; 5],
                name: Some("scroll_80000004".into()),
            }],
            mobiles: vec![Mobile {
                flim_version: 2,
                animations: Vec::new(),
                woaw_version: 3,
                woaw_count: 0,
                woaw_rest: Vec::new(),
                polygon: Polygon {
                    unknown_a: 0,
                    points: Vec::new(),
                    unknown_b: 0,
                },
                x: 0,
                y: 0,
                unknown_a: 0,
                unknown_b: 0,
                unknown_c: 0,
                unknown_d: 0,
                unknown_e: 0,
            }],
            script_areas: ScriptAreas {
                points: vec![opensherwood_formats::rhm::Point {
                    x: 1,
                    y: 1,
                    unknown_0x04: 0,
                    unknown_0x06: 0,
                }],
                polygons: vec![ScriptPolygon {
                    polygon: Polygon {
                        unknown_a: 0,
                        points: vec![(0, 0), (9, 0), (9, 9)],
                        unknown_b: 0,
                    },
                    unknown_0x00: 0,
                    unknown_0x02: 0,
                    name: Some("zone_80000005".into()),
                }],
            },
            cave: Vec::new(),
            chunk_versions: Vec::new(),
            unknown_chunks: Vec::new(),
        };
        let b = MissionBinding::from_mission(&mission, 2, (60, 1));
        let kinds: Vec<Element> = b.elements.iter().map(|(_, e)| *e).collect();
        assert_eq!(
            kinds,
            vec![
                Element::Map(0),
                Element::Map(1),
                Element::Unmodelled(2), // POUF
                Element::Actor(2),      // OILE
                Element::Actor(3),      // TOTO
                Element::Actor(4),      // BORG
                Element::Actor(5),
                Element::Object { x: 5, y: 6 },
                Element::Unmodelled(8), // ZORG (before the scrolls: the file's chunk order)
                Element::Scroll { x: 10, y: 20 },
                Element::Unmodelled(10), // TING
                Element::Actor(0),       // SCOT
                Element::Actor(1),
                Element::Polygon(1),
            ]
        );
        let named: Vec<(usize, &str)> = b
            .elements
            .iter()
            .enumerate()
            .filter_map(|(i, (n, _))| n.as_deref().map(|n| (i, n)))
            .collect();
        assert_eq!(
            named,
            vec![
                (6, "guard_80000002"),
                (7, "target_80000003"),
                (9, "scroll_80000004"),
                (11, "hero_80000001"),
                (13, "zone_80000005"),
            ]
        );
        assert_eq!(b.actor_count(), 6);
        assert_eq!(b.locations.len(), 2);
    }
    /// A native call and its `0x0d` are one instruction (finding 6 of Codex review 8): a jump
    /// whose target is the `0x0d` quad is refused, whether it comes straight from a `0x0e`,
    /// from one arm of an `0x0f` whose other arm runs the call (divergent predecessors), or
    /// from the back edge of a loop whose entry is the read; a jump to the `0x0c` itself is
    /// fine, and the `0x0d` quad translates to a `Nop`.
    #[test]
    fn jumps_into_a_native_result_read_are_refused() {
        // Initialize: t0 = 1; t1 = n2(t0); L: ...; the call is quads 3 (push) 4 (0x0c) 5 (0x0d).
        let body = |jump: Quad| {
            vec![
                q(0x13, TV, 0, 1),     // 1
                jump,                  // 2
                q(0x0b, TV, 0, 0),     // 3
                q(0x0c, 2, 0, 0),      // 4
                q(0x0d, TV + 4, 0, 0), // 5
                q(0x01, 0, 0, 0),      // 6
            ]
        };
        let script = |jump: Quad| Script {
            version: 1.5,
            classes: vec![class(
                "StartUp",
                0,
                &[("Initialize", 0, 0, 0, 8, body(jump))],
            )],
        };
        let refused = |jump: Quad| {
            let err = translate(&script(jump), &binding()).unwrap_err();
            assert!(
                matches!(err, TranslateError::Quad { quad: 2, .. })
                    && err.to_string().contains("result read"),
                "{err}"
            );
        };
        // Direct jump into the reader.
        refused(q(0x0e, 5, 0, 0));
        // Divergent predecessors: one arm falls into the call, the other jumps to the read.
        refused(q(0x0f, TV, 0, 5));
        // A jump to the call is fine; the read becomes a no-op, the call carries the slot.
        let program = translate(&script(q(0x0e, 3, 0, 0)), &binding()).unwrap();
        assert_eq!(
            program.classes[0].code[4],
            Instr::Native {
                id: 2,
                argc: 1,
                dst: Some(Slot {
                    space: Space::Temp,
                    index: 1
                })
            }
        );
        assert_eq!(program.classes[0].code[5], Instr::Nop);
        // Loop entry: the back edge targets the read.
        let looping = vec![
            q(0x0b, TV, 0, 0),     // 1
            q(0x0c, 2, 0, 0),      // 2
            q(0x0d, TV + 4, 0, 0), // 3
            q(0x0f, TV + 4, 0, 3), // 4: while (t1) goto 3
        ];
        let script = Script {
            version: 1.5,
            classes: vec![class("StartUp", 0, &[("Initialize", 0, 0, 0, 8, looping)])],
        };
        let err = translate(&script, &binding()).unwrap_err();
        assert!(
            matches!(err, TranslateError::Quad { quad: 4, .. })
                && err.to_string().contains("result read"),
            "{err}"
        );
        // The report tells a read the corpus never made from the contract.
        let (_, report) =
            translate_with_report(&script_with(native(2, &[1], Some(TV))), &binding()).unwrap();
        assert!(report.unobserved_result_reads.is_empty());
    }

    /// A script call and its `0x0a` are one instruction (finding 3 of Codex review 9): a jump
    /// whose target is the `0x0a` quad is refused (direct, from one arm of a branch whose other
    /// arm runs the call, or as a loop's entry), a `0x0a` without a `0x05` before it or after a
    /// call of a function that returns nothing is refused, and a jump to the `0x05` itself is
    /// fine: the call carries the slot and the `0x0a` quad is a `Nop`.
    #[test]
    fn jumps_into_a_call_result_read_are_refused() {
        // Initialize (quads 0..=7): t0 = 1; t1 = seven(t0); the call is quads 3 (push) 4 (0x05)
        // 5 (0x0a); `seven` starts at quad 8. `zero(x)` returns nothing.
        let body = |jump: Quad| {
            vec![
                q(0x13, TV, 0, 1),     // 1
                jump,                  // 2
                q(0x02, TV, 0, 0),     // 3
                q(0x05, 8, 0, 0),      // 4
                q(0x0a, TV + 4, 0, 0), // 5
                q(0x01, 0, 0, 0),      // 6
            ]
        };
        let seven = vec![q(0x13, TV, 0, 7), q(0x07, TV, 0, 0)];
        let zero = vec![q(0x08, TV, 0, 0)];
        let script = |init: Vec<Quad>| Script {
            version: 1.5,
            classes: vec![class(
                "StartUp",
                1,
                &[
                    ("Initialize", 0, 0, 0, 8, init),
                    ("seven", 4, 4, 0, 4, seven.clone()),
                    ("zero", 0, 4, 0, 4, zero.clone()),
                ],
            )],
        };
        assert_eq!(
            script(body(q(0x01, 0, 0, 0))).classes[0].functions[1].address,
            8
        );
        let refused = |init: Vec<Quad>, quad: usize, what: &str| {
            let err = translate(&script(init), &binding()).unwrap_err();
            assert!(
                matches!(err, TranslateError::Quad { quad: q, .. } if q == quad)
                    && err.to_string().contains(what),
                "{err}"
            );
        };
        // Direct jump into the reader; divergent predecessors.
        refused(body(q(0x0e, 5, 0, 0)), 2, "call result read");
        refused(body(q(0x0f, TV, 0, 5)), 2, "call result read");
        // A jump to the call is fine; the read becomes a no-op, the call carries the slot.
        let program = translate(&script(body(q(0x0e, 3, 0, 0))), &binding()).unwrap();
        assert_eq!(
            program.classes[0].code[4],
            Instr::Call {
                function: 1,
                argc: 1,
                dst: Some(Slot {
                    space: Space::Temp,
                    index: 1
                })
            }
        );
        assert_eq!(program.classes[0].code[5], Instr::Nop);
        program.validate().unwrap();
        // Loop entry: the back edge targets the read.
        let looping = vec![
            q(0x13, TV, 0, 1),     // 1
            q(0x02, TV, 0, 0),     // 2
            q(0x05, 8, 0, 0),      // 3
            q(0x0a, TV + 4, 0, 0), // 4
            q(0x0f, TV + 4, 0, 4), // 5: while (t1) goto 4
            q(0x01, 0, 0, 0),      // 6
        ];
        refused(looping, 5, "call result read");
        // A reader with no call before it, and a reader after a call of a void function.
        let orphan = vec![
            q(0x13, TV, 0, 1),     // 1
            q(0x0a, TV + 4, 0, 0), // 2
            q(0x01, 0, 0, 0),      // 3
            q(0x01, 0, 0, 0),      // 4
            q(0x01, 0, 0, 0),      // 5
            q(0x01, 0, 0, 0),      // 6
        ];
        refused(orphan, 2, "without a call before it");
        let void_read = vec![
            q(0x13, TV, 0, 1),     // 1
            q(0x02, TV, 0, 0),     // 2
            q(0x05, 12, 0, 0),     // 3: zero(t0)
            q(0x0a, TV + 4, 0, 0), // 4
            q(0x01, 0, 0, 0),      // 5
            q(0x01, 0, 0, 0),      // 6
        ];
        refused(void_read, 4, "returns none");
    }

    fn script_with(body: Vec<Quad>) -> Script {
        Script {
            version: 1.5,
            classes: vec![class("StartUp", 1, &[("Initialize", 0, 0, 0, 8, body)])],
        }
    }
}
