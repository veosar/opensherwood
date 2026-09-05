//! Data-backed tests: run only when `OPENSHERWOOD_GAME_DIR` points at a copy of the game. They
//! check the script element index space of `docs/formats/scb.md` ("Index spaces") and
//! `docs/formats/sherwood-hub.md` (section 4) against the retail files: the per-map prefix
//! computed from the `.rhp` equals the value the mission records' self-references derive, every
//! mission binds with no unbound class, and the player-character slots sit at the tail of the
//! table where the four self-referencing `SCOT` classes and H01's `Initialize` address them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use opensherwood_core::vm::{Element, ItemKind};
use opensherwood_formats::{rhm, rhp, scb};
use opensherwood_script::{
    MissionBinding, known_map_element_count, map_element_count, translate_with_report,
};

/// World tick rate the app binds with (`opensherwood_app::engine::TICK_RATE`).
const TICK_RATE: (u32, u32) = (60, 1);

fn levels_dir() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var_os("OPENSHERWOOD_GAME_DIR")?);
    let levels = p.join("DATA").join("Levels");
    levels.is_dir().then_some(levels)
}

/// Files of one extension in `dir`, keyed by lower-case stem.
fn files(dir: &Path, ext: &str) -> BTreeMap<String, PathBuf> {
    let mut out = BTreeMap::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_file()
            && p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(ext))
        {
            let stem = p
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .to_ascii_lowercase();
            out.insert(stem, p);
        }
    }
    out
}

macro_rules! need_data {
    () => {
        match levels_dir() {
            Some(d) => d,
            None => {
                eprintln!("OPENSHERWOOD_GAME_DIR not set; skipping");
                return;
            }
        }
    };
}

/// The nine maps: `FLIM` + `TUPO` of the `.rhp` is the prefix the self-references establish
/// (`sherwood-hub.md` 4.1, the "K (new)" column).
#[test]
fn map_element_counts_match_the_known_prefixes() {
    let dir = need_data!();
    let maps = files(&dir, "rhp");
    assert_eq!(maps.len(), 9, "{maps:?}");
    for (stem, path) in &maps {
        let map = rhp::parse(&std::fs::read(path).unwrap()).unwrap();
        let known = known_map_element_count(stem).unwrap_or_else(|| panic!("{stem}: not known"));
        assert_eq!(
            map_element_count(&map),
            known,
            "{stem}: FLIM {} + TUPO {}",
            map.flims.len(),
            map.tupo_count()
        );
    }
    assert_eq!(
        maps.get("sherwood")
            .map(|p| rhp::parse(&std::fs::read(p).unwrap()).unwrap().tupo_count()),
        Some(0)
    );
}

/// Immediate handed to native 3 at the call `pc` (`0x13 load; 0x0b push; 0x0c 3`), as the
/// translator reads it.
fn element_immediate(quads: &[scb::Quad], pc: usize) -> Option<i32> {
    let push = quads.get(pc.checked_sub(1)?)?;
    let load = quads.get(pc.checked_sub(2)?)?;
    (push.opcode == 0x0b && load.opcode == 0x13 && load.a == push.a).then_some(load.c as i32)
}

/// Every immediate a class passes to native 3.
fn element_immediates(class: &scb::Class) -> Vec<i32> {
    class
        .quads
        .iter()
        .enumerate()
        .filter(|(_, q)| q.opcode == 0x0c && q.a == 3)
        .filter_map(|(pc, _)| element_immediate(&class.quads, pc))
        .collect()
}

/// Every retail mission binds under the corrected table with no unbound class and no native-3
/// immediate beyond the table; the four named player-character classes that address their own
/// slot do so at the index the tail gives them (`sherwood-hub.md` 4.3), and H01's single slot is
/// element 126 (the element its level `Initialize` zeroes two attributes of), after the eleven
/// `ZORG` entries 115..=125, with index 49 a map patch and 50 the first civilian.
#[test]
fn every_retail_mission_binds_with_the_player_slots_at_the_tail() {
    let dir = need_data!();
    let maps: BTreeMap<String, rhp::Rhp> = files(&dir, "rhp")
        .into_iter()
        .map(|(stem, p)| (stem, rhp::parse(&std::fs::read(p).unwrap()).unwrap()))
        .collect();
    let scripts = files(&dir, "scb");
    let missions = files(&dir, "rhm");
    assert_eq!(scripts.len(), 39);
    // (mission, expected index of `SCOT` slot 0, minimum self-references in its class)
    let self_referencing = [
        ("tac01_foa_mp", 107, 5),
        ("emb03_foc_mp", 107, 4),
        ("emb04_foa_mp", 93, 5),
        ("sherwoodoutro", 70, 1),
    ];
    let mut checked = 0;
    for (stem, scb_path) in &scripts {
        let mission = rhm::parse(&std::fs::read(&missions[stem]).unwrap()).unwrap();
        let script = scb::parse(&std::fs::read(scb_path).unwrap()).unwrap();
        let map = &maps[&mission.header.map.to_ascii_lowercase()];
        let binding = MissionBinding::from_mission(&mission, map_element_count(map), TICK_RATE);
        let (program, report) =
            translate_with_report(&script, &binding).unwrap_or_else(|e| panic!("{stem}: {e}"));
        assert!(
            report.unbound_classes.is_empty(),
            "{stem}: unbound {:?}",
            report.unbound_classes
        );
        if let Some(max) = report.max_element_immediate {
            assert!(
                (max as usize) < program.elements.len(),
                "{stem}: immediate {max} beyond {} elements",
                program.elements.len()
            );
        }
        // The tail: `SCOT` slots (entity 0 first) right before the polygons.
        let slots = mission.player_characters().len();
        let polygons = mission.script_areas.polygons.len();
        let first_slot = binding.elements.len() - polygons - slots;
        for (i, (_, e)) in binding.elements[first_slot..first_slot + slots]
            .iter()
            .enumerate()
        {
            assert_eq!(*e, Element::Actor(i as u32), "{stem}: slot {i}");
        }
        if let Some((_, expected, min_hits)) = self_referencing.iter().find(|(m, ..)| m == stem) {
            checked += 1;
            assert_eq!(first_slot, *expected, "{stem}: slot 0");
            // Every named slot whose class passes its own table index to native 3.
            let mut hits = 0;
            for (i, (name, _)) in binding.elements[first_slot..first_slot + slots]
                .iter()
                .enumerate()
            {
                let Some(class) = name
                    .as_deref()
                    .and_then(|n| script.classes.iter().find(|c| c.name == n))
                else {
                    continue;
                };
                hits += element_immediates(class)
                    .iter()
                    .filter(|&&x| x == (first_slot + i) as i32)
                    .count();
            }
            assert!(
                hits >= *min_hits,
                "{stem}: {hits} self-references in the slot classes, {min_hits} expected"
            );
        }
        if stem == "h01_lin_vl" {
            assert_eq!(map_element_count(map), 50);
            assert_eq!(binding.elements[49], (None, Element::Map(49)));
            assert_eq!(binding.elements[50].1, Element::Actor(1));
            // The eleven `ZORG` pick-up items precede the fifteen scrolls
            // (`docs/original/h01-win-path.md` 2): kinds 0 (arrows) and 9 (purses) are read,
            // the others kept by their raw value.
            assert!(
                binding.elements[100..=110]
                    .iter()
                    .all(|(n, e)| n.is_none() && matches!(e, Element::Item { .. }))
            );
            assert!(
                binding.elements[111..=125]
                    .iter()
                    .all(|(n, e)| n.is_some() && matches!(e, Element::Scroll { .. }))
            );
            assert_eq!(mission.zorg.len(), 11);
            assert_eq!(
                binding.elements[100].1,
                Element::Item {
                    x: 2199,
                    y: 1092,
                    kind: ItemKind::Arrows,
                    stack: 2
                }
            );
            assert_eq!(
                binding.elements[105].1,
                Element::Item {
                    x: 572,
                    y: 1388,
                    kind: ItemKind::Purse,
                    stack: 3
                }
            );
            assert!(matches!(
                binding.elements[106].1,
                Element::Item {
                    kind: ItemKind::Unknown(8),
                    stack: 1,
                    ..
                }
            ));
            // The lone visible hero's slot carries no class name: the level addresses it by
            // index (`n117(126, ..)`), which is the reference that places it.
            assert_eq!(binding.elements[126], (None, Element::Actor(0)));
            assert_eq!(report.max_element_immediate, Some(126));
            let level = &script.classes[0];
            assert!(element_immediates(level).contains(&126));
        }
    }
    assert_eq!(checked, self_referencing.len());
}
