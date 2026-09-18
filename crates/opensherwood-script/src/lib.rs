//! Translate a compiled mission script (`.scb`, `opensherwood_formats::scb`) into the core VM's
//! instruction set (`opensherwood_core::vm`, ADR-0008). This crate holds no execution logic and
//! no state: it maps opcodes one to one, resolves the mission's index spaces and validates every
//! reference (`docs/original/spec-script-vm.md` section 3.1, `docs/formats/scb.md` for the
//! container).
//!
//! One instruction per bytecode quad, so a quad index *is* an instruction index and the jump,
//! call and native-call targets of VM-002 (`a | (b << 16)`; the conditional jumps' `c`) address
//! the translated vector directly. Nothing is fused: `0x0A` reads the frame's result slot and
//! `0x0D` the native result register, both of which the interpreter keeps, exactly as the
//! original does. A jump to `0xFFFFFFFF` (the two retail occurrences of VM-070) is kept as such
//! and ends the callback at run time.
//!
//! What the translator refuses is what no retail file contains and what would otherwise become
//! an unchecked access at run time: an operand offset that is not a multiple of four, a symbol
//! outside its block, a jump or call target outside the class code, a native id beyond the
//! 265-entry call table. What it does **not** check is what the original does not check either
//! (VM-080, VM-087): argument counts and parameter offsets, whose deterministic outcomes are in
//! the specification's 8.1.

use std::collections::BTreeMap;

use opensherwood_core::vm::{
    BinOp, Class, END_OF_CALLBACK, Element, Function, GLOBAL_CELLS, Instr, ItemKind, Location,
    NATIVE_TABLE_SIZE, Program, Slot, Space,
};
use opensherwood_formats::rhm::{ActorGroup, Mission};
use opensherwood_formats::rhp::Rhp;
use opensherwood_formats::scb::{self, Quad, Script, Storage};

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
    /// (actors), `BOOM` (objects), `ZORG` (pick-up items: [`Element::Item`] with the kind and
    /// stack of the record), `SKRO` (scrolls), `TING` (inert), the `SCOT` player-character
    /// slots at the tail, and the
    /// script polygons after them (their position is not observable; no retail script addresses
    /// one through native 3). The entity numbering stays the app's
    /// actor list (actor groups in file order: `SCOT` first, then `OILE`, `TOTO`, `BORG`; objects
    /// skipped), so each group's entity ids are assigned in file order and the groups are then
    /// laid out in table order.
    #[must_use]
    pub fn from_mission(mission: &Mission, map_elements: u32) -> Self {
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
        // Pick-up items: the record's `unknown_a` is the kind and `unknown_b` the stack
        // (`docs/formats/rhm.md`, "`ZORG`"); no class addresses them by name.
        elements.extend(mission.zorg.iter().map(|z| {
            (
                None,
                Element::Item {
                    x: i32::from(z.placement.x),
                    y: i32::from(z.placement.y),
                    kind: ItemKind::from_field(z.unknown_a),
                    stack: z.unknown_b,
                },
            )
        }));
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
    let program = Program {
        classes,
        elements: binding.elements.iter().map(|(_, e)| *e).collect(),
        locations: binding.locations.iter().map(|(_, l)| l.clone()).collect(),
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
    // Function table -> the calling convention (VM-003): the interpreter uses the name, the
    // address and `size_of_volatile`; the prologue `0x03` at the address allocates the blocks.
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
    let mut code = Vec::with_capacity(c.quads.len());
    let mut fi = 0usize;
    for (pc, q) in c.quads.iter().enumerate() {
        while fi + 1 < functions.len() && functions[fi + 1].address as usize <= pc {
            fi += 1;
        }
        let f = &functions[fi];
        // A symbol operand (VM-011): two bits of storage class over a byte offset. The
        // original checks nothing; the translator refuses what no retail file contains, so a
        // mistranslation is an error here rather than an unchecked access at run time.
        let slot = |v: u16| -> Result<Slot, TranslateError> {
            let (storage, offset) = scb::operand(v);
            let space = match storage {
                Storage::None => Space::Global,
                Storage::ClassVar => Space::Class,
                Storage::Local => Space::Local,
                Storage::Temp => Space::Temp,
            };
            if !offset.is_multiple_of(4) {
                return Err(quad_err(ci, c, pc, "operand offset is not a multiple of 4"));
            }
            let index = u32::from(offset / 4);
            let limit = match space {
                Space::Global => GLOBAL_CELLS as u32,
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
        // VM-002: the jump, call and native-call operand is `a | (b << 16)`.
        let wide = u32::from(q.a) | (u32::from(q.b) << 16);
        let target = |t: u32| -> Result<u32, TranslateError> {
            if (t as usize) < c.quads.len() || t == END_OF_CALLBACK {
                Ok(t)
            } else {
                Err(quad_err(
                    ci,
                    c,
                    pc,
                    format!("jump target {t} outside the class code"),
                ))
            }
        };
        let ins = match q.opcode {
            // VM-040 / VM-044 / VM-069: the opcodes the interpreter reports as an error.
            0x00 | 0x04 => Instr::Bad { opcode: q.opcode },
            0x01 => Instr::Nop,
            0x02 => Instr::PushParam { src: slot(q.a)? },
            0x03 => Instr::Enter {
                locals: u32::from(q.a) / 4,
                temps: u32::from(q.b) / 4,
            },
            0x05 => Instr::Call {
                target: target(wide)?,
            },
            0x06 => Instr::Return,
            0x07 => Instr::ReturnValue { src: slot(q.a)? },
            0x08 => Instr::LoadParam {
                dst: slot(q.a)?,
                offset: q.c,
            },
            0x09 => Instr::StoreParam {
                src: slot(q.a)?,
                offset: q.c,
            },
            0x0a => Instr::LoadResult { dst: slot(q.a)? },
            0x0b => Instr::PushArg { src: slot(q.a)? },
            0x0c => {
                let id = wide;
                if id as usize >= NATIVE_TABLE_SIZE {
                    return Err(quad_err(
                        ci,
                        c,
                        pc,
                        format!("native {id} is beyond the call table of {NATIVE_TABLE_SIZE}"),
                    ));
                }
                *report.native_calls.entry(id).or_insert(0) += 1;
                if id == 3
                    && let Some(imm) = element_immediate(&c.quads, pc)
                {
                    report.max_element_immediate =
                        Some(report.max_element_immediate.map_or(imm, |m| m.max(imm)));
                }
                Instr::Native { id }
            }
            0x0d => Instr::LoadNativeResult { dst: slot(q.a)? },
            0x0e => Instr::Jump {
                target: target(wide)?,
            },
            0x0f => Instr::JumpIfNonZero {
                cond: slot(q.a)?,
                target: target(q.c)?,
            },
            0x10 => Instr::JumpIfZero {
                cond: slot(q.a)?,
                target: target(q.c)?,
            },
            0x11 | 0x12 => Instr::Move {
                dst: slot(q.a)?,
                src: slot(q.b)?,
            },
            // VM-058: `0x13` and `0x14` are the same instruction; a float immediate's bits go
            // into the cell verbatim (ints and floats share the 4-byte cell, VM-004).
            0x13 | 0x14 => Instr::LoadImm {
                dst: slot(q.a)?,
                value: q.c as i32,
            },
            0x15 => Instr::NegInt {
                dst: slot(q.a)?,
                src: slot(q.b)?,
            },
            0x16 => Instr::NegFloat {
                dst: slot(q.a)?,
                src: slot(q.b)?,
            },
            0x17 => Instr::FloatToInt {
                dst: slot(q.a)?,
                src: slot(q.b)?,
            },
            0x18 => Instr::IntToFloat {
                dst: slot(q.a)?,
                src: slot(q.b)?,
            },
            // VM-063 - VM-068; the third symbol is the low 16 bits of `c` (VM-002).
            0x19..=0x2f => Instr::Binary {
                op: BinOp::of_opcode(q.opcode)
                    .ok_or_else(|| quad_err(ci, c, pc, "unknown three-operand opcode"))?,
                dst: slot(q.a)?,
                a: slot(q.b)?,
                b: slot((q.c & 0xffff) as u16)?,
            },
            other => Instr::Bad { opcode: other },
        };
        code.push(ins);
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

#[cfg(test)]
mod tests {
    use super::*;
    use opensherwood_core::vm::{Assumption, SeqWait, callbacks};
    use opensherwood_core::{ActorSpec, Geometry, MapInfo, MissionSpec, Scenario, Team, World};
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
            // A function ends with `0x06`: `0x04` is an error the interpreter *reports and
            // steps past* (VM-044), so it would fall through into the next function.
            quads.push(q(0x06, 0, 0, 0));
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
        // Initialize: sum = 0; for (i = 0; i < 5; i++) sum = sum + twice(i); n0(7, sum)
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
            q(0x0c, 0, 0, 0),                 // 18: n0(7, sum) declares and stores
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
        assert_eq!(report.native_calls.get(&0), Some(&1));
        assert_eq!(program.classes[0].functions[1].param_count, 1);
        assert!(program.classes[0].functions[1].has_result);
        // One instruction per quad: the `0x05` carries the callee's code address and the
        // `0x0A` after it reads the frame's result slot (VM-045 / VM-050).
        assert_eq!(
            program.classes[0].code[8],
            Instr::Call {
                target: program.classes[0].functions[1].address
            }
        );
        assert_eq!(
            program.classes[0].code[9],
            Instr::LoadResult {
                dst: Slot {
                    space: Space::Temp,
                    index: 2
                }
            }
        );
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
        // Quad 0 is the prologue.
        assert_eq!(program.classes[1].code[15], Instr::Native { id: 132 });
        assert_eq!(program.classes[1].code[7], Instr::Native { id: 3 });
        assert_eq!(
            program.classes[1].code[8],
            Instr::LoadNativeResult {
                dst: Slot {
                    space: Space::Temp,
                    index: 1
                }
            }
        );
        let w = world(program);
        assert_eq!(
            w.vm.as_ref().unwrap().instances[1].vars[0],
            -1,
            "74 is null outside a callback with an actor context, so 10 answers -1"
        );
        assert_eq!(
            w.entities[1].program, None,
            "path 0 does not exist: no program"
        );
        // Native 9 on an empty patrol-path list is the unchecked read of 8.1: recorded, null,
        // and the callback runs on.
        assert!(
            w.vm.as_ref()
                .unwrap()
                .faults
                .iter()
                .all(|f| matches!(f.fault, opensherwood_core::vm::Fault::UncheckedAccess(9))),
            "{:?}",
            w.vm.as_ref().unwrap().faults
        );
    }

    /// The three-operand opcodes of `spec-script-vm.md` 3.1 read through the bytecode:
    /// `0x24` is a signed `<=`, `0x28` a `!=`, `0x2B` a float `<` answering a float, `0x22` a
    /// float multiply, and `0x0E` with `a = b = 0xFFFF` ends the callback (VM-070).
    #[test]
    fn opcode_semantics_follow_the_specification() {
        // Initialize: t0 = 3; t1 = 2; cv0 = t0 <= t1; cv1 = t1 <= t0; cv2 = t0 != t1;
        // cv3 = t0 != t0; f0 = 0.5f; f1 = float(t1) (2.0f); cv4 = f0 < f1; cv5 = f1 < f0;
        // cv6 = f0 * f1; jump 0xFFFFFFFF; cv7 = 99 (never reached).
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
            Instr::Jump {
                target: END_OF_CALLBACK
            },
            "0x0E with a = b = 0xFFFF is the jump of VM-070"
        );
        let w = world(program);
        // The only hypothesis is the one the specification names for that jump.
        assert_eq!(
            w.script_observation().unwrap().assumptions,
            vec![Assumption::UnresolvedJump]
        );
        let vars = &w.vm.as_ref().unwrap().instances[0].vars;
        assert_eq!(vars[0], 0, "0x24 is <=: 3 <= 2");
        assert_eq!(vars[1], 1, "0x24 is <=: 2 <= 3");
        assert_eq!(vars[2], 1, "0x28 is !=: 3 != 2");
        assert_eq!(vars[3], 0, "0x28 is !=: 3 != 3");
        assert_eq!(vars[4], 1.0f32.to_bits() as i32, "0x2B answers a float");
        assert_eq!(vars[5], 0.0f32.to_bits() as i32);
        assert_eq!(vars[6], 1.0f32.to_bits() as i32, "0x22: 0.5f * 2.0f");
        assert_eq!(vars[7], 0, "the 0xFFFFFFFF jump ended the callback");
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
            SeqWait::Ticks(25),
            "a timer of 25 counts 25 logic frames (ADR-0010)"
        );
        for _ in 0..25 {
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
        // Jump outside the class code.
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
        // Prologue mismatch.
        let mut bad = ok.clone();
        bad.functions[0].size_of_tempor = 8;
        assert!(
            translate(&script(bad), &binding())
                .unwrap_err()
                .to_string()
                .contains("prologue")
        );
        // An opcode the interpreter reports as an error is translated, not refused: it is
        // `Instr::Bad`, whose outcome at run time is `spec-script-vm.md` 8.1.
        let mut bad = ok.clone();
        bad.quads[1] = q(0x31, 0, 0, 0);
        let p = translate(&script(bad), &binding()).unwrap();
        assert_eq!(p.classes[0].code[1], Instr::Bad { opcode: 0x31 });
        // A parameter read beyond the count is **not** refused: the original checks nothing
        // (VM-048), and the deterministic outcome is an unchecked read at run time.
        let mut bad = ok.clone();
        bad.quads[1] = q(0x08, TV, 0, 4);
        translate(&script(bad), &binding()).unwrap();
        // A native id beyond the 265-entry call table is refused (VM-085).
        let mut bad = ok;
        bad.quads[2] = q(0x0c, 300, 0, 0);
        let err = translate(&script(bad), &binding()).unwrap_err().to_string();
        assert!(err.contains("beyond the call table"), "{err}");
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
        let b = MissionBinding::from_mission(&mission, 2);
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
                // ZORG (before the scrolls: the file's chunk order): kind 12 is not read
                // yet, the stack is the record's `unknown_b`.
                Element::Item {
                    x: 10,
                    y: 20,
                    kind: ItemKind::Unknown(12),
                    stack: 1
                },
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
    /// The native call and its `0x0D` are two instructions (VM-052 / VM-053), so a jump may
    /// land on either: on the call it runs and the read follows, on the read alone it answers
    /// whatever the native result register holds.
    #[test]
    fn a_jump_may_land_on_a_result_read() {
        let body = |jump: Quad| {
            vec![
                jump,                  // 1
                q(0x13, TV, 0, 0),     // 2
                q(0x0b, TV, 0, 0),     // 3
                q(0x0c, 2, 0, 0),      // 4
                q(0x0d, TV + 4, 0, 0), // 5
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
        // A jump straight to the read is accepted now.
        let program = translate(&script(q(0x0e, 5, 0, 0)), &binding()).unwrap();
        assert_eq!(
            program.classes[0].code[5],
            Instr::LoadNativeResult {
                dst: Slot {
                    space: Space::Temp,
                    index: 1
                }
            }
        );
        // So is a jump to the call itself.
        let program = translate(&script(q(0x0e, 4, 0, 0)), &binding()).unwrap();
        assert_eq!(program.classes[0].code[4], Instr::Native { id: 2 });
        // A `0x0D` with no call before it is an ordinary read of the register.
        let program = translate(&script_with(vec![q(0x0d, TV, 0, 0)]), &binding()).unwrap();
        assert_eq!(
            program.classes[0].code[1],
            Instr::LoadNativeResult {
                dst: Slot {
                    space: Space::Temp,
                    index: 0
                }
            }
        );
    }

    /// A script call and its `0x0A` are two instructions (VM-045 / VM-050): a jump may land on
    /// either, and a `0x0A` with no call before it reads the frame's result slot, which is 0
    /// until a `0x07` one frame deeper wrote it (VM-071).
    #[test]
    fn a_jump_may_land_on_a_call_result_read() {
        let body = |jump: Quad| {
            vec![
                jump,                  // 1
                q(0x13, TV, 0, 1),     // 2
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
        let program = translate(&script(body(q(0x0e, 5, 0, 0))), &binding()).unwrap();
        assert_eq!(program.classes[0].code[4], Instr::Call { target: 8 });
        assert_eq!(
            program.classes[0].code[5],
            Instr::LoadResult {
                dst: Slot {
                    space: Space::Temp,
                    index: 1
                }
            }
        );
        program.validate().unwrap();
        // A reader with no call before it is an ordinary instruction as well.
        let orphan = vec![
            q(0x13, TV, 0, 1),     // 1
            q(0x0a, TV + 4, 0, 0), // 2
            q(0x01, 0, 0, 0),      // 3
            q(0x01, 0, 0, 0),      // 4
            q(0x01, 0, 0, 0),      // 5
            q(0x01, 0, 0, 0),      // 6
        ];
        translate(&script(orphan), &binding()).unwrap();
    }

    fn script_with(body: Vec<Quad>) -> Script {
        Script {
            version: 1.5,
            classes: vec![class("StartUp", 1, &[("Initialize", 0, 0, 0, 8, body)])],
        }
    }
}
