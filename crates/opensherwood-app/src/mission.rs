//! Turn a retail mission file into a `MissionSpec` for the core (`docs/formats/rhm.md`).
//!
//! What is verified: actor placements (map pixels), 16-way directions, the actor groups. What is
//! still a placeholder: the profile-index -> sprite profile mapping (the profile table is not
//! located yet), so NPCs are drawn with a default soldier sprite and player characters get the
//! hero profiles in file order. NPC patrols come from the rail programs (`RAIL`): each rail is
//! translated into a core [`Instruction`] program by [`compile_rail`]; the opcode meanings used
//! are the documented inferences of `docs/formats/rhm.md` ("Rail programs") and every command
//! whose meaning is not established becomes a no-op and is counted in the load-time summary.

use std::collections::BTreeMap;

use opensherwood_assets::GameDir;
use opensherwood_core::{ActorSpec, Geometry, Instruction, MapInfo, MissionSpec, Team};
use opensherwood_formats::rhm::{self, ActorGroup, Command, CommandTable, Mission, RailPoint};

/// Hero profiles in the order player characters appear in mission files (placeholder).
const HERO_PROFILES: [&str; 6] = [
    "RobinHood",
    "LittleJohn",
    "WillScarlet",
    "Stuteley",
    "Friar Tuck",
    "LadyMarian",
];

/// Default NPC profile until the profile table is decoded.
const NPC_PROFILE: &str = "Soldier A00";
/// Default civilian profile.
const CIVILIAN_PROFILE: &str = "ManCivilianPoor";

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

/// Counts from translating a mission's rail programs.
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
        let mut s = format!(
            "{} rails in use, {} waypoint commands: {} translated, {} unknown (no-op)",
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
        s
    }
}

/// Load `Data/Levels/<name>.rhm` and build the spec. `map_size` must be supplied by the caller
/// after decoding the background (core cannot read files).
pub fn load(game: &GameDir, name: &str) -> Result<(Mission, String), String> {
    let logical = format!("Data/Levels/{name}.rhm");
    let data = game.read(&logical).map_err(|e| e.to_string())?;
    let mission = rhm::parse(&data).map_err(|e| format!("{logical}: {e}"))?;
    let map = mission.header.map.clone();
    Ok((mission, map))
}

/// Build the spec from a decoded mission and the background size. Logs one line with the rail
/// program translation summary.
#[must_use]
pub fn build_spec(
    mission: &Mission,
    map: MapInfo,
    geometry: Geometry,
) -> (MissionSpec, Vec<String>) {
    let (spec, profiles, stats) = build_spec_with_stats(mission, map, geometry);
    eprintln!(
        "opensherwood: mission {} on {}: {}",
        mission.header.mission_id,
        mission.header.map,
        stats.summary()
    );
    (spec, profiles)
}

/// [`build_spec`] returning the translation counts instead of logging them.
#[must_use]
pub fn build_spec_with_stats(
    mission: &Mission,
    map: MapInfo,
    geometry: Geometry,
) -> (MissionSpec, Vec<String>, ProgramStats) {
    let mut actors = Vec::new();
    let mut profiles: Vec<String> = Vec::new();
    let mut use_profile = |p: &str| {
        if !profiles.iter().any(|x| x == p) {
            profiles.push(p.to_string());
        }
        p.to_string()
    };
    let mut stats = ProgramStats::default();
    // Each rail is translated once (in first-use order, so the counts are deterministic).
    let mut compiled: BTreeMap<usize, Vec<Instruction>> = BTreeMap::new();
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
                    });
                }
            }
            ActorGroup::Npcs { records, .. } => {
                for npc in records {
                    let program = usize::try_from(npc.rail)
                        .ok()
                        .and_then(|r| mission.rails.get(r).map(|rail| (r, rail)))
                        .map(|(r, rail)| {
                            compiled
                                .entry(r)
                                .or_insert_with(|| compile_rail(rail, &mut stats))
                                .clone()
                        })
                        .unwrap_or_default();
                    actors.push(ActorSpec {
                        profile: use_profile(NPC_PROFILE),
                        team: Team::Enemy,
                        x: i32::from(npc.placement.x),
                        y: i32::from(npc.placement.y),
                        facing256: facing256_from_direction(npc.placement.direction),
                        patrol: Vec::new(),
                        program,
                    });
                }
            }
            ActorGroup::Civilians { records, .. } => {
                for c in records {
                    actors.push(ActorSpec {
                        profile: use_profile(CIVILIAN_PROFILE),
                        team: Team::Civilian,
                        x: i32::from(c.placement.x),
                        y: i32::from(c.placement.y),
                        facing256: facing256_from_direction(c.placement.direction),
                        patrol: Vec::new(),
                        program: Vec::new(),
                    });
                }
            }
            ActorGroup::Vips { records, .. } => {
                for v in records {
                    actors.push(ActorSpec {
                        profile: use_profile(CIVILIAN_PROFILE),
                        team: Team::Civilian,
                        x: i32::from(v.placement.x),
                        y: i32::from(v.placement.y),
                        facing256: facing256_from_direction(v.placement.direction),
                        patrol: Vec::new(),
                        program: Vec::new(),
                    });
                }
            }
            ActorGroup::Meow { .. } | ActorGroup::Objects { .. } | ActorGroup::Unknown { .. } => {}
        }
    }
    (
        MissionSpec {
            map,
            geometry,
            actors,
        },
        profiles,
        stats,
    )
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
        out.push(Instruction::GoTo {
            x: i32::from(p.point.x),
            y: i32::from(p.point.y),
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
}
