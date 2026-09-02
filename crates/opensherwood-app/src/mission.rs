//! Turn a retail mission file into a `MissionSpec` for the core (`docs/formats/rhm.md`).
//!
//! What is verified: actor placements (map pixels), 16-way directions, the actor groups. The
//! sprite of every non-player actor comes from `Configuration/profile.cpf`
//! (`docs/formats/profile.md`): `BORG.profile` indexes the SD table, `OILE.profile` the CV table
//! and `TOTO.profile` the PC table, each record naming `Characters/<sprite>.rhs`. A missing or
//! malformed table, or a referenced sprite whose profile cannot be loaded from the bank, fails
//! the mission load ([`load`]); only with `OPENSHERWOOD_LENIENT_ASSETS=1` do they, like an index
//! outside its table, fall back to a default of the actor's kind with a logged warning. `SCOT`
//! records carry no profile: which hero stands in a slot is campaign state, so the heroes are
//! still assigned in file order
//! (Robin first; correct for `H01_Lin_VL` and `S01_Not_VL`, where Robin is alone). Whether the
//! original honours `SCOT.unknown_0x16` for that choice is an open question of the spec.
//!
//! NPC patrols come from the rail programs (`RAIL`): each rail is translated into a core
//! [`Instruction`] program by [`compile_rail`]; the opcode meanings used are the documented
//! inferences of `docs/formats/rhm.md` ("Rail programs") and every command whose meaning is not
//! established becomes a no-op and is counted in the load-time summary.
//!
//! The mission script (`Data/Levels/<name>.scb`, `docs/formats/scb.md`) is translated by
//! `opensherwood-script` into the core VM's program (ADR-0008) with the mission's element table
//! built here from the same actor order as the entities; a missing or untranslatable script
//! leaves the mission without a VM, logged. The per-mission counts of natives the engine does
//! not implement are part of the load-time summary.

use std::collections::{BTreeMap, BTreeSet};

use opensherwood_assets::{GameDir, SpriteBank};
use opensherwood_core::natives::{NativeStatus, native_status};
use opensherwood_core::{ActorSpec, Geometry, Instruction, MapInfo, MissionSpec, Team};
use opensherwood_formats::cpf::{self, ProfileTable};
use opensherwood_formats::rhm::{self, ActorGroup, Command, CommandTable, Mission, RailPoint};
use opensherwood_formats::scb;
use opensherwood_script::{MissionBinding, map_element_count, translate_with_report};

/// Hero profiles in the order player characters appear in mission files (placeholder: the
/// campaign decides the team; see the module documentation).
const HERO_PROFILES: [&str; 6] = [
    "RobinHood",
    "LittleJohn",
    "WillScarlet",
    "Stuteley",
    "Friar Tuck",
    "LadyMarian",
];

/// Fallback NPC profile when the SD table entry is unavailable.
const NPC_PROFILE: &str = "Soldier A00";
/// Fallback civilian / VIP profile when the CV or PC table entry is unavailable.
const CIVILIAN_PROFILE: &str = "ManCivilianPoor";

/// Logical path of the profile table.
const PROFILE_TABLE_PATH: &str = "Data/Configuration/profile.cpf";

/// `SCOT` placement flag of a hidden player character (`docs/formats/rhm.md`): the actor starts
/// inactive and a script activates it (native 114).
const HIDDEN_PC_FLAG: u32 = 0x88;

/// Opcodes `compile_block` gives a meaning to (see its documentation); every other opcode is a no-op.
const TRANSLATED_OPCODES: [u8; 6] = [0x02, 0x03, 0x04, 0x07, 0x0b, 0x0c];

/// Glance angle of the look-left / look-right commands in 1/256 turns (45 degrees). Inferred:
/// the commands come in pairs around a facing command; the exact angle is not established.
const LOOK_DELTA256: i32 = 32;

/// Convert the file's 16-way direction (0 = east, counter-clockwise) to 1/256 turns clockwise.
#[must_use]
pub fn facing256_from_direction(direction: u32) -> i32 {
    let d = (direction % 16) as i32;
    ((16 - d) % 16) * 16
}

/// Ticks for a waypoint wait operand. Inferred: the operand values (10, 12, 25, 50, 75, 100,
/// 150 ... 500) read as hundredths of a second; the unit is not verified against the original.
#[must_use]
pub fn wait_ticks(hundredths: u16) -> u32 {
    let (num, den) = crate::engine::TICK_RATE;
    u32::from(hundredths) * num / (100 * den)
}

/// Counts from translating a mission's rail programs and resolving its actor profiles.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgramStats {
    /// Rails translated (those assigned to actors).
    pub rails: usize,
    /// Commands seen.
    pub commands: usize,
    /// Commands translated with a documented (inferred) meaning.
    pub translated: usize,
    /// Commands emitted as no-ops, by opcode.
    pub unknown: BTreeMap<u8, usize>,
    /// Non-player actors drawn with the default sprite of their kind because their profile
    /// table entry was unavailable.
    pub profile_fallbacks: usize,
    /// Script translation counts (`None` when the mission has no script).
    pub script: Option<ScriptStats>,
    /// Why the script was not attached, if it exists but could not be used.
    pub script_error: Option<String>,
}

/// Counts from translating the mission script.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptStats {
    /// Classes.
    pub classes: usize,
    /// Native call sites whose native the engine implements.
    pub implemented_calls: usize,
    /// Native call sites of recorded no-op natives, by id.
    pub stub_calls: BTreeMap<u32, usize>,
    /// Native call sites of natives the engine knows nothing about, by id.
    pub unknown_calls: BTreeMap<u32, usize>,
    /// Classes whose name matched no mission element.
    pub unbound_classes: Vec<String>,
    /// Size of the element table built for native 3.
    pub elements: usize,
    /// Largest immediate element index used with native 3 in the script.
    pub max_element_immediate: Option<i32>,
}

impl ProgramStats {
    /// Total no-op commands.
    #[must_use]
    pub fn unknown_total(&self) -> usize {
        self.unknown.values().sum()
    }

    /// One-line summary for the log.
    #[must_use]
    pub fn summary(&self) -> String {
        use std::fmt::Write as _;
        let mut s = format!(
            "{} rails, {} waypoint commands: {} translated, {} unknown (no-op)",
            self.rails,
            self.commands,
            self.translated,
            self.unknown_total()
        );
        if !self.unknown.is_empty() {
            let list: Vec<String> = self
                .unknown
                .iter()
                .map(|(op, n)| format!("{op:#04x} x{n}"))
                .collect();
            s.push_str(" [");
            s.push_str(&list.join(", "));
            s.push(']');
        }
        if self.profile_fallbacks > 0 {
            let _ = write!(
                s,
                "; {} actors on a default sprite (profile unavailable)",
                self.profile_fallbacks
            );
        }
        match (&self.script, &self.script_error) {
            (Some(sc), _) => {
                let _ = write!(
                    s,
                    "; script: {} classes, {} elements (max index used {}), native calls: {} implemented, {} stub [{}], {} unknown [{}]",
                    sc.classes,
                    sc.elements,
                    sc.max_element_immediate
                        .map_or("-".to_string(), |m| m.to_string()),
                    sc.implemented_calls,
                    sc.stub_calls.values().sum::<usize>(),
                    join_counts(&sc.stub_calls),
                    sc.unknown_calls.values().sum::<usize>(),
                    join_counts(&sc.unknown_calls),
                );
                if !sc.unbound_classes.is_empty() {
                    let _ = write!(s, "; unbound classes: {}", sc.unbound_classes.join(", "));
                }
            }
            (None, Some(e)) => {
                let _ = write!(s, "; script not attached: {e}");
            }
            (None, None) => s.push_str("; no script"),
        }
        s
    }
}

fn join_counts(m: &BTreeMap<u32, usize>) -> String {
    m.iter()
        .map(|(id, n)| format!("{id} x{n}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A mission file together with what the spec needs from the rest of the installation.
#[derive(Debug, Clone)]
pub struct LoadedMission {
    /// The decoded `.rhm`.
    pub mission: Mission,
    /// The profile table, `None` when `Configuration/profile.cpf` is missing or unreadable.
    pub profiles: Option<ProfileTable>,
    /// Sprite base names (`Characters/<name>.rhs`) referenced by this mission through the profile
    /// table whose profile loads from the sprite bank; anything else falls back.
    pub available_sprites: BTreeSet<String>,
    /// The mission script, `None` when `Data/Levels/<name>.scb` is missing or unreadable.
    pub script: Option<scb::Script>,
}

/// Load `Data/Levels/<name>.rhm` and the profile table, and check the referenced sprites against
/// the bank. Returns the loaded mission and its map name; the map size must be supplied by the
/// caller after decoding the background (core cannot read files).
///
/// The profile table and every sprite profile the mission references through it are required:
/// a missing or malformed one fails the load, unless `lenient` (`crate::engine::lenient_assets`,
/// `OPENSHERWOOD_LENIENT_ASSETS=1`) is set, in which case the affected actors fall back to the
/// default sprite of their kind with a logged warning. The mission script stays optional here
/// (ADR-0008 governs missing or untranslatable scripts).
pub fn load(game: &GameDir, name: &str, lenient: bool) -> Result<(LoadedMission, String), String> {
    let logical = format!("Data/Levels/{name}.rhm");
    let data = game.read(&logical).map_err(|e| e.to_string())?;
    let mission = rhm::parse(&data).map_err(|e| format!("{logical}: {e}"))?;
    let map = mission.header.map.clone();
    let script_path = format!("Data/Levels/{name}.scb");
    // The script is the mission's authoritative logic: a missing or malformed one fails the load
    // unless `lenient` (then the mission runs without a script, logged).
    let script = match game
        .read(&script_path)
        .map_err(|e| e.to_string())
        .and_then(|bytes| scb::parse(&bytes).map_err(|e| e.to_string()))
    {
        Ok(s) => Some(s),
        Err(e) if lenient => {
            eprintln!(
                "opensherwood: {script_path}: {e}; mission runs without a script (OPENSHERWOOD_LENIENT_ASSETS)"
            );
            None
        }
        Err(e) => return Err(format!("{script_path}: {e}")),
    };
    let profiles = game
        .read(PROFILE_TABLE_PATH)
        .map_err(|e| e.to_string())
        .and_then(|bytes| cpf::parse(&bytes).map_err(|e| e.to_string()));
    let profiles = match profiles {
        Ok(t) => Some(t),
        Err(e) if lenient => {
            eprintln!(
                "opensherwood: {PROFILE_TABLE_PATH}: {e}; OPENSHERWOOD_LENIENT_ASSETS: NPCs use default sprites"
            );
            None
        }
        Err(e) => return Err(format!("{PROFILE_TABLE_PATH}: {e}")),
    };
    let mut available_sprites = BTreeSet::new();
    if let Some(table) = &profiles {
        for sprite in referenced_sprites(&mission, table) {
            match SpriteBank::load_profile(game, &sprite) {
                Ok(_) => {
                    available_sprites.insert(sprite);
                }
                Err(e) if lenient => eprintln!(
                    "opensherwood: profile table sprite {sprite:?}: {e}; OPENSHERWOOD_LENIENT_ASSETS: using a default sprite"
                ),
                Err(e) => return Err(format!("profile table sprite {sprite:?}: {e}")),
            }
        }
    }
    Ok((
        LoadedMission {
            mission,
            profiles,
            available_sprites,
            script,
        },
        map,
    ))
}

/// Distinct sprite names the mission's `BORG`, `OILE` and `TOTO` records resolve to through the
/// table (indices outside a table are skipped).
#[must_use]
pub fn referenced_sprites(mission: &Mission, table: &ProfileTable) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for group in &mission.actor_groups {
        match group {
            ActorGroup::Npcs { records, .. } => {
                for r in records {
                    if let Some(p) = index(&table.soldiers, r.profile) {
                        out.insert(p.sprite.clone());
                    }
                }
            }
            ActorGroup::Civilians { records, .. } => {
                for r in records {
                    if let Some(p) = index(&table.civilians, r.profile) {
                        out.insert(p.sprite.clone());
                    }
                }
            }
            ActorGroup::Vips { records, .. } => {
                for r in records {
                    if let Some(p) = index(&table.player_characters, r.profile) {
                        out.insert(p.sprite.clone());
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn index<T>(table: &[T], i: u32) -> Option<&T> {
    usize::try_from(i).ok().and_then(|i| table.get(i))
}

/// The sprite of a non-player actor: the table entry when it exists and its profile is
/// available, else the kind's default (counted as a fallback).
fn npc_sprite<'a>(
    loaded: &'a LoadedMission,
    entry: Option<&'a str>,
    default: &'a str,
    stats: &mut ProgramStats,
) -> &'a str {
    match entry {
        Some(s) if loaded.available_sprites.contains(s) => s,
        _ => {
            stats.profile_fallbacks += 1;
            default
        }
    }
}

/// Build the spec from a loaded mission and the background size (one log line with the rail and
/// script translation summary). Refuses a mission whose script exists but cannot be translated or bound (unknown
/// map index space, element table mismatch, invalid program) unless `lenient`; a mission file that has
/// no script at all is accepted only when `lenient` let `load` skip it.
///
/// # Errors
///
/// The script's translation or binding error, unless `lenient`.
pub fn build_spec_checked(
    loaded: &LoadedMission,
    map: MapInfo,
    geometry: Geometry,
    lenient: bool,
) -> Result<(MissionSpec, Vec<String>), String> {
    let (spec, profiles, stats) = build_spec_with_stats(loaded, map, geometry);
    eprintln!(
        "opensherwood: mission {} on {}: {}",
        loaded.mission.header.mission_id,
        loaded.mission.header.map,
        stats.summary()
    );
    if let Some(err) = &stats.script_error
        && !lenient
    {
        return Err(format!("mission script: {err}"));
    }
    Ok((spec, profiles))
}

/// [`build_spec`] returning the translation counts instead of logging them.
#[must_use]
pub fn build_spec_with_stats(
    loaded: &LoadedMission,
    map: MapInfo,
    geometry: Geometry,
) -> (MissionSpec, Vec<String>, ProgramStats) {
    let mission = &loaded.mission;
    let table = loaded.profiles.as_ref();
    let mut actors = Vec::new();
    let mut profiles: Vec<String> = Vec::new();
    let mut use_profile = |p: &str| {
        if !profiles.iter().any(|x| x == p) {
            profiles.push(p.to_string());
        }
        p.to_string()
    };
    let mut stats = ProgramStats::default();
    // Every rail is translated once, in index order: the script assigns rails by index
    // (native 132) and actors reference theirs by index.
    let rails: Vec<Vec<Instruction>> = mission
        .rails
        .iter()
        .map(|rail| compile_rail(rail, &mut stats))
        .collect();
    let mut hero = 0usize;
    for group in &mission.actor_groups {
        match group {
            ActorGroup::PlayerCharacters { records, .. } => {
                for pc in records {
                    let profile = HERO_PROFILES[hero.min(HERO_PROFILES.len() - 1)];
                    hero += 1;
                    actors.push(ActorSpec {
                        profile: use_profile(profile),
                        team: Team::Player,
                        x: i32::from(pc.placement.x),
                        y: i32::from(pc.placement.y),
                        facing256: facing256_from_direction(pc.placement.direction),
                        patrol: Vec::new(),
                        program: Vec::new(),
                        active: pc.placement.unknown_0x08 != HIDDEN_PC_FLAG,
                    });
                }
            }
            ActorGroup::Npcs { records, .. } => {
                for npc in records {
                    let program = usize::try_from(npc.rail)
                        .ok()
                        .and_then(|r| rails.get(r).cloned())
                        .unwrap_or_default();
                    let entry = table
                        .and_then(|t| index(&t.soldiers, npc.profile))
                        .map(|p| p.sprite.as_str());
                    let sprite = npc_sprite(loaded, entry, NPC_PROFILE, &mut stats);
                    actors.push(ActorSpec {
                        profile: use_profile(sprite),
                        team: Team::Enemy,
                        x: i32::from(npc.placement.x),
                        y: i32::from(npc.placement.y),
                        facing256: facing256_from_direction(npc.placement.direction),
                        patrol: Vec::new(),
                        program,
                        active: true,
                    });
                }
            }
            ActorGroup::Civilians { records, .. } => {
                for c in records {
                    let entry = table
                        .and_then(|t| index(&t.civilians, c.profile))
                        .map(|p| p.sprite.as_str());
                    let sprite = npc_sprite(loaded, entry, CIVILIAN_PROFILE, &mut stats);
                    actors.push(ActorSpec {
                        profile: use_profile(sprite),
                        team: Team::Civilian,
                        x: i32::from(c.placement.x),
                        y: i32::from(c.placement.y),
                        facing256: facing256_from_direction(c.placement.direction),
                        patrol: Vec::new(),
                        program: Vec::new(),
                        active: true,
                    });
                }
            }
            ActorGroup::Vips { records, .. } => {
                for v in records {
                    let entry = table
                        .and_then(|t| index(&t.player_characters, v.profile))
                        .map(|p| p.sprite.as_str());
                    let sprite = npc_sprite(loaded, entry, CIVILIAN_PROFILE, &mut stats);
                    actors.push(ActorSpec {
                        profile: use_profile(sprite),
                        team: Team::Civilian,
                        x: i32::from(v.placement.x),
                        y: i32::from(v.placement.y),
                        facing256: facing256_from_direction(v.placement.direction),
                        patrol: Vec::new(),
                        program: Vec::new(),
                        active: true,
                    });
                }
            }
            ActorGroup::Meow { .. } | ActorGroup::Objects { .. } | ActorGroup::Unknown { .. } => {}
        }
    }
    let script = loaded
        .script
        .as_ref()
        .and_then(|script| translate_script(script, mission, actors.len(), &mut stats));
    (
        MissionSpec {
            map,
            geometry,
            actors,
            script,
            rails,
            lenient_natives: false,
        },
        profiles,
        stats,
    )
}

/// Translate the mission script against the mission's element table; `None` (with the reason
/// in `stats.script_error`) when the table does not match the actors or the script is invalid.
fn translate_script(
    script: &scb::Script,
    mission: &Mission,
    actor_count: usize,
    stats: &mut ProgramStats,
) -> Option<opensherwood_core::Program> {
    let Some(map_elements) = map_element_count(&mission.header.map) else {
        stats.script_error = Some(format!(
            "no element index space known for map {}",
            mission.header.map
        ));
        return None;
    };
    let binding = MissionBinding::from_mission(mission, map_elements, crate::engine::TICK_RATE);
    if binding.actor_count() != actor_count {
        stats.script_error = Some(format!(
            "element table has {} actors, the mission {}",
            binding.actor_count(),
            actor_count
        ));
        return None;
    }
    match translate_with_report(script, &binding) {
        Ok((program, report)) => {
            let mut sc = ScriptStats {
                classes: program.classes.len(),
                elements: program.elements.len(),
                unbound_classes: report.unbound_classes,
                max_element_immediate: report.max_element_immediate,
                ..ScriptStats::default()
            };
            for (id, n) in report.native_calls {
                match native_status(id) {
                    NativeStatus::Implemented => sc.implemented_calls += n,
                    NativeStatus::Stub => *sc.stub_calls.entry(id).or_insert(0) += n,
                    NativeStatus::Unknown => *sc.unknown_calls.entry(id).or_insert(0) += n,
                }
            }
            stats.script = Some(sc);
            Some(program)
        }
        Err(e) => {
            stats.script_error = Some(e.to_string());
            None
        }
    }
}

/// A pending `Jump` whose target is the visit of another rail point.
struct Fixup {
    /// Index of the placeholder instruction.
    at: usize,
    /// Point the command was on.
    from: usize,
    /// Point to continue at.
    to: usize,
}

/// Translate one rail into a program (`docs/formats/rhm.md`, "Rail programs").
///
/// The rail is walked back and forth: points `0, 1, .., n-1, n-2, .., 1`, then again from 0.
/// Arriving at a point runs the point's table for that travel direction (id 1 forward, id 2
/// backward, id 0 either) as a `Choose` over its percentage blocks. Inferred from the data:
/// tables with id 2 sit on first points and id 1 on last points, so the ids select the travel
/// direction; single-point rails only ever have table 0.
#[must_use]
pub fn compile_rail(rail: &[RailPoint], stats: &mut ProgramStats) -> Vec<Instruction> {
    let n = rail.len();
    if n == 0 {
        return Vec::new();
    }
    stats.rails += 1;
    for c in rail
        .iter()
        .flat_map(|p| &p.tables)
        .flat_map(|t| &t.blocks)
        .flat_map(|b| &b.commands)
    {
        stats.commands += 1;
        if TRANSLATED_OPCODES.contains(&c.opcode) {
            stats.translated += 1;
        } else {
            *stats.unknown.entry(c.opcode).or_insert(0) += 1;
        }
    }
    // (point index, arriving while travelling forward)
    let visits: Vec<(usize, bool)> = (0..n)
        .map(|i| (i, i != 0))
        .chain((1..n.saturating_sub(1)).rev().map(|i| (i, false)))
        .collect();
    let mut out: Vec<Instruction> = Vec::new();
    let mut visit_pc: Vec<u32> = Vec::with_capacity(visits.len());
    let mut fixups: Vec<Fixup> = Vec::new();
    for &(i, forward) in &visits {
        visit_pc.push(out.len() as u32);
        let p = &rail[i];
        // Off-map points of cart paths are negative coordinates stored as two's complement
        // (`docs/formats/rhm.md`, `RAIL`).
        out.push(Instruction::GoTo {
            x: i32::from(p.point.x as i16),
            y: i32::from(p.point.y as i16),
        });
        let wanted = if forward { 1 } else { 2 };
        let table = p
            .tables
            .iter()
            .find(|t| t.id == wanted)
            .or_else(|| p.tables.iter().find(|t| t.id == 0));
        if let Some(t) = table {
            compile_table(t, i, n, &mut out, &mut fixups);
        }
    }
    // Loop for ever.
    out.push(Instruction::Jump { pc: 0 });
    for f in fixups {
        // Continue in the direction of the jump: a later point is visited going forward, an
        // earlier one going backward (point 0 has a single visit).
        let k = if f.to > f.from {
            f.to
        } else if f.to == 0 {
            0
        } else {
            2 * n - 2 - f.to
        };
        out[f.at] = Instruction::Jump { pc: visit_pc[k] };
    }
    out
}

/// One table: a single 100 % block inline, otherwise a `Choose` over the blocks (each block
/// ends by jumping past the table; a roll no block covers falls through the same way).
fn compile_table(
    table: &CommandTable,
    point: usize,
    n: usize,
    out: &mut Vec<Instruction>,
    fixups: &mut Vec<Fixup>,
) {
    let blocks = &table.blocks;
    if blocks.is_empty() {
        return;
    }
    if blocks.len() == 1 && blocks[0].percent >= 100 {
        compile_block(&blocks[0].commands, point, n, out, fixups);
        return;
    }
    let choose_at = out.len();
    out.push(Instruction::Choose { arms: Vec::new() });
    let mut arms = Vec::with_capacity(blocks.len());
    let mut end_jumps = Vec::new();
    for (k, b) in blocks.iter().enumerate() {
        arms.push((b.percent, out.len() as u32));
        compile_block(&b.commands, point, n, out, fixups);
        if k + 1 < blocks.len() {
            end_jumps.push(out.len());
            out.push(Instruction::Jump { pc: 0 });
        }
    }
    let end = out.len() as u32;
    for j in end_jumps {
        out[j] = Instruction::Jump { pc: end };
    }
    out[choose_at] = Instruction::Choose { arms };
}

fn u16_arg(c: &Command) -> u16 {
    u16::from_le_bytes([
        c.args.first().copied().unwrap_or(0),
        c.args.get(1).copied().unwrap_or(0),
    ])
}

/// One command block. Meanings used here (all inferred from the data, see rhm.md; the arms must
/// match `TRANSLATED_OPCODES`):
/// 0x03 = face direction, 0x04 = wait, 0x02 = continue at point n, 0x07 = stop here,
/// 0x0b / 0x0c = glance left / right of the last facing. Everything else is a no-op.
fn compile_block(
    commands: &[Command],
    point: usize,
    n: usize,
    out: &mut Vec<Instruction>,
    fixups: &mut Vec<Fixup>,
) {
    // Facing set by the most recent 0x03 of this block (the glance commands are relative to it).
    let mut base: Option<i32> = None;
    for c in commands {
        let ins = match c.opcode {
            // Inferred: operand 0..=15 is a 16-way direction like the placement's.
            0x03 => {
                let facing256 = facing256_from_direction(u32::from(u16_arg(c)));
                base = Some(facing256);
                Instruction::Face { facing256 }
            }
            // Inferred: a duration (see `wait_ticks`).
            0x04 => Instruction::Wait {
                ticks: wait_ticks(u16_arg(c)),
            },
            // Inferred: the operand is always a point index of the same rail (0 on last points:
            // "go back to the start", i.e. a loop instead of walking back).
            0x02 => {
                let to = usize::from(u16_arg(c));
                if to < n && to != point {
                    fixups.push(Fixup {
                        at: out.len(),
                        from: point,
                        to,
                    });
                    Instruction::Jump { pc: 0 }
                } else {
                    Instruction::Nop { opcode: c.opcode }
                }
            }
            // Inferred: only ever the last command, on single or terminal points: stop here.
            0x07 => Instruction::Stop,
            // Inferred: 0x0b / 0x0c always come as a pair around waits: glance to one side and
            // the other. Which of the two is "left" is not established.
            0x0b | 0x0c => {
                let delta256 = if c.opcode == 0x0b {
                    -LOOK_DELTA256
                } else {
                    LOOK_DELTA256
                };
                match base {
                    Some(b) => Instruction::Face {
                        facing256: (b + delta256).rem_euclid(256),
                    },
                    None => Instruction::Turn { delta256 },
                }
            }
            other => Instruction::Nop { opcode: other },
        };
        out.push(ins);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opensherwood_formats::rhm::{CommandBlock, Point};

    #[test]
    fn direction_conversion() {
        assert_eq!(facing256_from_direction(0), 0);
        assert_eq!(facing256_from_direction(4), 192); // north (counter-clockwise 90) -> our 270
        assert_eq!(facing256_from_direction(8), 128);
        assert_eq!(facing256_from_direction(12), 64);
        assert_eq!(facing256_from_direction(16), 0);
    }

    fn cmd(opcode: u8, args: &[u16]) -> Command {
        Command {
            opcode,
            args: args.iter().flat_map(|a| a.to_le_bytes()).collect(),
        }
    }

    fn point(x: u16, y: u16, tables: Vec<CommandTable>) -> RailPoint {
        RailPoint {
            point: Point {
                x,
                y,
                unknown_0x04: 0,
                unknown_0x06: 0,
            },
            kind: 0,
            name: None,
            tables,
        }
    }

    fn table(id: u8, blocks: Vec<(u8, Vec<Command>)>) -> CommandTable {
        CommandTable {
            id,
            blocks: blocks
                .into_iter()
                .map(|(percent, commands)| CommandBlock { percent, commands })
                .collect(),
        }
    }

    #[test]
    fn wait_operand_is_hundredths_of_a_second() {
        assert_eq!(
            wait_ticks(100),
            crate::engine::TICK_RATE.0 / crate::engine::TICK_RATE.1
        );
        assert_eq!(wait_ticks(0), 0);
    }

    #[test]
    fn single_point_rail_is_a_guard_post() {
        use Instruction::*;
        let rail = vec![point(
            10,
            20,
            vec![table(
                0,
                vec![(
                    100,
                    vec![
                        cmd(0x03, &[4]),
                        cmd(0x04, &[100]),
                        cmd(0x0b, &[]),
                        cmd(0x04, &[50]),
                    ],
                )],
            )],
        )];
        let mut stats = ProgramStats::default();
        let p = compile_rail(&rail, &mut stats);
        assert_eq!(
            p,
            vec![
                GoTo { x: 10, y: 20 },
                Face { facing256: 192 },
                Wait {
                    ticks: wait_ticks(100)
                },
                Face { facing256: 160 },
                Wait {
                    ticks: wait_ticks(50)
                },
                Jump { pc: 0 },
            ]
        );
        assert_eq!((stats.rails, stats.commands, stats.translated), (1, 4, 4));
        assert!(stats.unknown.is_empty());
    }

    #[test]
    fn rail_walks_back_and_forth_with_direction_tables_and_jumps() {
        use Instruction::*;
        // Three points: the first has a backward-arrival table, the last a forward one with a
        // 50/50 choice whose second arm jumps back to point 0; the middle point has both.
        let rail = vec![
            point(0, 0, vec![table(2, vec![(100, vec![cmd(0x04, &[100])])])]),
            point(
                1,
                1,
                vec![
                    table(1, vec![(100, vec![cmd(0x03, &[0])])]),
                    table(2, vec![(100, vec![cmd(0x03, &[8])])]),
                ],
            ),
            point(
                2,
                2,
                vec![table(
                    1,
                    vec![
                        (50, vec![cmd(0x09, &[]), cmd(0x07, &[])]),
                        (50, vec![cmd(0x02, &[0])]),
                    ],
                )],
            ),
        ];
        let mut stats = ProgramStats::default();
        let p = compile_rail(&rail, &mut stats);
        let expected = vec![
            GoTo { x: 0, y: 0 }, // 0: point 0 (backward arrival: table 2)
            Wait {
                ticks: wait_ticks(100),
            },
            GoTo { x: 1, y: 1 }, // 2: point 1 forward (table 1)
            Face { facing256: 0 },
            GoTo { x: 2, y: 2 }, // 4: point 2 forward (table 1)
            Choose {
                arms: vec![(50, 6), (50, 9)],
            },
            Nop { opcode: 0x09 }, // 6
            Stop,
            Jump { pc: 10 },
            Jump { pc: 0 },      // 9: 0x02(0) -> point 0
            GoTo { x: 1, y: 1 }, // 10: point 1 backward (table 2)
            Face { facing256: 128 },
            Jump { pc: 0 },
        ];
        assert_eq!(p, expected);
        assert_eq!(stats.commands, 6);
        assert_eq!(stats.translated, 5);
        assert_eq!(stats.unknown.get(&0x09), Some(&1));
        assert_eq!(stats.unknown_total(), 1);
        assert!(stats.summary().contains("1 unknown"));
    }

    #[test]
    fn forward_jumps_target_the_forward_visit() {
        use Instruction::*;
        let rail = vec![
            point(0, 0, vec![table(0, vec![(100, vec![cmd(0x02, &[2])])])]),
            point(1, 1, vec![]),
            point(2, 2, vec![]),
            point(3, 3, vec![table(0, vec![(100, vec![cmd(0x02, &[1])])])]),
        ];
        let mut stats = ProgramStats::default();
        let p = compile_rail(&rail, &mut stats);
        // Visits: 0 (pc 0), 1 (pc 2), 2 (pc 3), 3 (pc 4), 2 back (pc 6), 1 back (pc 7).
        assert_eq!(p[1], Jump { pc: 3 });
        assert_eq!(p[5], Jump { pc: 7 });
        assert_eq!(p.len(), 9);
        assert_eq!(p[8], Jump { pc: 0 });
    }

    fn synthetic_mission() -> Mission {
        use opensherwood_formats::rhm::{
            Brains, Civilian, Header, Npc, Placement, ScriptAreas, Vip,
        };
        let placement = Placement {
            x: 10,
            y: 20,
            ..Placement::default()
        };
        let npc = |profile: u32| Npc {
            placement,
            unknown_0x12: 0,
            profile,
            unknown_0x1a: 0,
            unknown_0x1b: 0,
            unknown_0x1f: 0,
            unknown_0x23: 0,
            members: Vec::new(),
            rail: -1,
            unknown_i16: -1,
            name: None,
        };
        Mission {
            version: 2,
            header: Header {
                version: 4,
                map_id: 100,
                variant: 1,
                map: "Croisement01".into(),
                mission_id: 1,
            },
            tenants: Vec::new(),
            actor_groups: vec![
                ActorGroup::Npcs {
                    version: 4,
                    records: vec![npc(0), npc(1), npc(99)],
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
            ],
            zorg: Vec::new(),
            brains: Brains::default(),
            rails: Vec::new(),
            scrolls: Vec::new(),
            mobiles: Vec::new(),
            script_areas: ScriptAreas::default(),
            cave: Vec::new(),
            chunk_versions: Vec::new(),
            unknown_chunks: Vec::new(),
        }
    }

    fn synthetic_table() -> ProfileTable {
        use opensherwood_formats::cpf::{CivilianProfile, PlayerProfile, SoldierProfile};
        let soldier = |sprite: &str| SoldierProfile {
            sprite: sprite.into(),
            sequence: String::new(),
            label: String::new(),
            unknown_pre: [0; 21],
            voice: "XX01".into(),
            unknown_post: [0; 55],
        };
        let pc = |sprite: &str| PlayerProfile {
            sprite: sprite.into(),
            sequence: String::new(),
            label: String::new(),
            unknown_pre: [0; 8],
            voice: "XX02".into(),
            unknown_post: [0; 82],
        };
        let civilian = |sprite: &str| CivilianProfile {
            sprite: sprite.into(),
            sequence: String::new(),
            label: String::new(),
            unknown_pre: [0; 8],
            voice: "XX03".into(),
        };
        ProfileTable {
            soldiers: vec![soldier("Guard A00"), soldier("Guard A01")],
            civilians: vec![civilian("TaxeCollector"), civilian("Mendicant")],
            player_characters: vec![pc("RobinHood"), pc("RobinTown")],
            ..ProfileTable::default()
        }
    }

    #[test]
    fn hidden_player_characters_start_inactive() {
        use opensherwood_formats::rhm::{Placement, PlayerCharacter};
        let mut mission = synthetic_mission();
        let pc = |flag: u32, name: Option<&str>| PlayerCharacter {
            placement: Placement {
                x: 1,
                y: 2,
                unknown_0x08: flag,
                ..Placement::default()
            },
            unknown_0x12: 0,
            unknown_0x16: [0; 10],
            name: name.map(str::to_string),
            unknown_trailer: 0,
        };
        mission.actor_groups.insert(
            0,
            ActorGroup::PlayerCharacters {
                version: 4,
                records: vec![
                    pc(0x86, None),
                    pc(HIDDEN_PC_FLAG, Some("pc_hidden_80000001")),
                    pc(3, None),
                ],
            },
        );
        let loaded = LoadedMission {
            mission,
            profiles: None,
            available_sprites: BTreeSet::new(),
            script: None,
        };
        let (spec, _, _) = build_spec_with_stats(
            &loaded,
            MapInfo {
                width: 100,
                height: 100,
            },
            Geometry::default(),
        );
        let active: Vec<bool> = spec.actors.iter().map(|a| a.active).collect();
        assert_eq!(active, [true, false, true, true, true, true, true, true]);
    }

    fn actor_profiles(loaded: &LoadedMission) -> (Vec<String>, Vec<String>, ProgramStats) {
        let (spec, profiles, stats) = build_spec_with_stats(
            loaded,
            MapInfo {
                width: 100,
                height: 100,
            },
            Geometry::default(),
        );
        let actors: Vec<String> = spec.actors.iter().map(|a| a.profile.clone()).collect();
        (actors, profiles, stats)
    }

    #[test]
    fn profiles_resolve_through_the_table_and_fall_back_when_unavailable() {
        let mission = synthetic_mission();
        let table = synthetic_table();
        assert_eq!(
            referenced_sprites(&mission, &table)
                .into_iter()
                .collect::<Vec<_>>(),
            ["Guard A00", "Guard A01", "Mendicant", "RobinTown"]
        );
        // Everything available: table sprites, and the VIP wears the PC table sprite.
        let loaded = LoadedMission {
            mission: mission.clone(),
            profiles: Some(table.clone()),
            available_sprites: referenced_sprites(&mission, &table),
            script: None,
        };
        let (actors, profiles, stats) = actor_profiles(&loaded);
        assert_eq!(
            actors,
            [
                "Guard A00",
                "Guard A01",
                NPC_PROFILE, // index 99: outside the SD table
                "Mendicant",
                "RobinTown"
            ]
        );
        assert_eq!(stats.profile_fallbacks, 1);
        assert_eq!(
            profiles,
            [
                "Guard A00",
                "Guard A01",
                NPC_PROFILE,
                "Mendicant",
                "RobinTown"
            ]
        );
        assert!(stats.summary().contains("1 actors on a default sprite"));
        // One sprite failed to load from the bank: its actors fall back.
        let mut partial = loaded.clone();
        partial.available_sprites.remove("Guard A01");
        let (actors, _, stats) = actor_profiles(&partial);
        assert_eq!(actors[1], NPC_PROFILE);
        assert_eq!(stats.profile_fallbacks, 2);
        // No table at all: the old defaults for every kind.
        let none = LoadedMission {
            mission,
            profiles: None,
            available_sprites: BTreeSet::new(),
            script: None,
        };
        let (actors, profiles, stats) = actor_profiles(&none);
        assert_eq!(
            actors,
            [
                NPC_PROFILE,
                NPC_PROFILE,
                NPC_PROFILE,
                CIVILIAN_PROFILE,
                CIVILIAN_PROFILE
            ]
        );
        assert_eq!(profiles, [NPC_PROFILE, CIVILIAN_PROFILE]);
        assert_eq!(stats.profile_fallbacks, 5);
    }
}
