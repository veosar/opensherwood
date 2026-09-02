//! Translate a compiled mission script (`.scb`, `opensherwood_formats::scb`) into the core VM's
//! instruction set (`opensherwood_core::vm`, ADR-0008). This crate holds no execution logic and
//! no state: it maps opcodes, applies the calling convention, resolves the mission's index spaces
//! and validates every reference (`docs/formats/scb.md`).
//!
//! Choices for the low-confidence rows of the spec (each pinned by a test below):
//! `0x24` is `>=` (its Desperados name), `0x28` is `!=`, `0x2b` is a fixed-point `<`, a jump to
//! `0xffff` (two occurrences, an unresolved `break` in a switch) returns from the function, and
//! `0x14` immediates are rounded to 24.8 fixed point.

use std::collections::BTreeMap;

use opensherwood_core::vm::{
    BinOp, Class, Element, Function, Instr, Location, Program, Slot, Space,
};
use opensherwood_formats::rhm::{ActorGroup, Mission};
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

/// Number of map elements (`FLIM` entries plus the unidentified proto-level entries) that precede
/// the mission's own elements in the flat table, per map (`docs/formats/scb.md`, "Index spaces").
/// `None` for maps the model does not fit (`sherwood`).
#[must_use]
pub fn map_element_count(map: &str) -> Option<u32> {
    match map.to_ascii_lowercase().as_str() {
        "croisement01" => Some(14),
        "croisement02" | "croisement03" | "derby" => Some(19),
        "nottingham" => Some(58),
        "leicester" => Some(59),
        "lincoln" => Some(49),
        "york" => Some(67),
        _ => None,
    }
}

impl MissionBinding {
    /// Build the binding from a decoded mission: `map_elements` map entries first, then
    /// `POUF` entries, then the actor groups in file order (`SCOT`, `OILE`, `TOTO`, `BORG` as
    /// actors numbered like the world's entities, `BOOM` as objects), then the scrolls, then the
    /// script polygons. The entity numbering must match the app's actor list (actor groups in
    /// file order, objects skipped).
    #[must_use]
    pub fn from_mission(mission: &Mission, map_elements: u32, tick_rate: (u32, u32)) -> Self {
        let mut elements: Vec<(Option<String>, Element)> = Vec::new();
        for i in 0..map_elements {
            elements.push((None, Element::Map(i)));
        }
        for i in 0..mission.tenants.len() {
            elements.push((
                None,
                Element::Unmodelled((map_elements as usize + i) as u32),
            ));
        }
        let mut entity = 0u32;
        let mut actor = |name: &Option<String>| {
            let e = (name.clone(), Element::Actor(entity));
            entity += 1;
            e
        };
        for group in &mission.actor_groups {
            match group {
                ActorGroup::PlayerCharacters { records, .. } => {
                    elements.extend(records.iter().map(|r| actor(&r.name)));
                }
                ActorGroup::Civilians { records, .. } => {
                    elements.extend(records.iter().map(|r| actor(&r.name)));
                }
                ActorGroup::Vips { records, .. } => {
                    elements.extend(records.iter().map(|r| actor(&r.name)));
                }
                ActorGroup::Npcs { records, .. } => {
                    elements.extend(records.iter().map(|r| actor(&r.name)));
                }
                ActorGroup::Objects { records, .. } => {
                    elements.extend(records.iter().map(|r| {
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
        elements.extend(mission.scrolls.iter().map(|s| {
            (
                s.name.clone(),
                Element::Scroll {
                    x: i32::from(s.placement.x),
                    y: i32::from(s.placement.y),
                },
            )
        }));
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
    // Jump targets, to check that no argument push straddles one.
    let mut targets = vec![false; c.quads.len()];
    for q in &c.quads {
        match q.opcode {
            0x0e if q.a != 0xffff => {
                if let Some(t) = targets.get_mut(usize::from(q.a)) {
                    *t = true;
                }
            }
            0x0f => {
                if let Some(t) = targets.get_mut(q.c as usize) {
                    *t = true;
                }
            }
            _ => {}
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
            // last call are its parameters and must match its table entry.
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
                Instr::Call {
                    function: function as u32,
                    argc,
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
            // 0x0a: read the return value of the preceding call (high).
            0x0a => Instr::GetCallResult { dst: slot(q.a)? },
            // 0x0b: push native argument (high).
            0x0b => {
                pushed_args += 1;
                Instr::PushArg { src: slot(q.a)? }
            }
            // 0x0c: native call `a` (high); arity = pushes since the last native call.
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
                Instr::Native { id, argc }
            }
            // 0x0d: read the native result (high).
            0x0d => Instr::GetNativeResult { dst: slot(q.a)? },
            // 0x0e: jump to quad `a` (high); `0xffff` is an unresolved label: leave the function.
            0x0e => {
                if q.a == 0xffff {
                    Instr::Return
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
                    0x24 | 0x26 => BinOp::Ge,
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
    use opensherwood_core::vm::{SeqWait, callbacks};
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
            Instr::Native { id: 132, argc: 2 }
        );
        assert_eq!(program.classes[1].code[7], Instr::Native { id: 3, argc: 1 });
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
            Instr::Return,
            "0x0e 0xffff returns"
        );
        let w = world(program);
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
        let mut bad = ok;
        bad.quads[1] = q(0x08, TV, 0, 4);
        assert!(translate(&script(bad), &binding()).is_err());
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
    fn map_table() {
        assert_eq!(map_element_count("Lincoln"), Some(49));
        assert_eq!(map_element_count("Croisement03"), Some(19));
        assert_eq!(map_element_count("sherwood"), None);
    }
}
