//! Turn a retail mission file into a `MissionSpec` for the core (`docs/formats/rhm.md`).
//!
//! What is verified: actor placements (map pixels), 16-way directions, the actor groups. What is
//! still a placeholder: the profile-index -> sprite profile mapping (the profile table is not
//! located yet), so NPCs are drawn with a default soldier sprite and player characters get the
//! hero profiles in file order; and patrol routes, which follow the rail points literally.

use opensherwood_assets::GameDir;
use opensherwood_core::{ActorSpec, MapInfo, MissionSpec, Team};
use opensherwood_formats::rhm::{self, ActorGroup, Mission};

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

/// Convert the file's 16-way direction (0 = east, counter-clockwise) to 1/256 turns clockwise.
#[must_use]
pub fn facing256_from_direction(direction: u32) -> i32 {
    let d = (direction % 16) as i32;
    ((16 - d) % 16) * 16
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

/// Build the spec from a decoded mission and the background size.
#[must_use]
pub fn build_spec(mission: &Mission, map: MapInfo) -> (MissionSpec, Vec<String>) {
    let mut actors = Vec::new();
    let mut profiles: Vec<String> = Vec::new();
    let mut use_profile = |p: &str| {
        if !profiles.iter().any(|x| x == p) {
            profiles.push(p.to_string());
        }
        p.to_string()
    };
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
                    });
                }
            }
            ActorGroup::Npcs { records, .. } => {
                for npc in records {
                    let patrol = usize::try_from(npc.rail)
                        .ok()
                        .and_then(|r| mission.rails.get(r))
                        .map(|rail| {
                            rail.iter()
                                .map(|p| (i32::from(p.point.x), i32::from(p.point.y)))
                                .collect()
                        })
                        .unwrap_or_default();
                    actors.push(ActorSpec {
                        profile: use_profile(NPC_PROFILE),
                        team: Team::Enemy,
                        x: i32::from(npc.placement.x),
                        y: i32::from(npc.placement.y),
                        facing256: facing256_from_direction(npc.placement.direction),
                        patrol,
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
                    });
                }
            }
            ActorGroup::Meow { .. } | ActorGroup::Objects { .. } | ActorGroup::Unknown { .. } => {}
        }
    }
    (MissionSpec { map, actors }, profiles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_conversion() {
        assert_eq!(facing256_from_direction(0), 0);
        assert_eq!(facing256_from_direction(4), 192); // north (counter-clockwise 90) -> our 270
        assert_eq!(facing256_from_direction(8), 128);
        assert_eq!(facing256_from_direction(12), 64);
        assert_eq!(facing256_from_direction(16), 0);
    }
}
