//! Data-backed tests: run only when `OPENSHERWOOD_GAME_DIR` points at a copy of the game.
//! They assert the invariants recorded in `docs/formats/*.md` over every file of each kind.

use std::path::{Path, PathBuf};

use opensherwood_formats::{
    FileKind, chunk, cpf, detect, dic, image_blob, rhs, scb, sprite_decode, sres,
};

fn game_dir() -> Option<PathBuf> {
    let p = std::env::var_os("OPENSHERWOOD_GAME_DIR")?;
    let p = PathBuf::from(p);
    p.join("DATA").is_dir().then_some(p)
}

fn files_with_ext(root: &Path, ext: &str) -> Vec<PathBuf> {
    fn walk(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, ext, out);
            } else if p
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(ext))
            {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, ext, &mut out);
    out.sort();
    out
}

macro_rules! need_data {
    () => {
        match game_dir() {
            Some(d) => d,
            None => {
                eprintln!("OPENSHERWOOD_GAME_DIR not set; skipping");
                return;
            }
        }
    };
}

#[test]
fn every_map_and_minimap_decodes_to_16bpp() {
    let dir = need_data!();
    let mut count = 0;
    for ext in ["map", "min", "pak", "sxt"] {
        for f in files_with_ext(&dir, ext) {
            let data = std::fs::read(&f).unwrap();
            assert_eq!(detect(&data), FileKind::ImageBlob, "{}", f.display());
            let imgs = image_blob::parse_sequence(&data)
                .unwrap_or_else(|e| panic!("{}: {e}", f.display()));
            for img in &imgs {
                assert_eq!(
                    img.pixels.len(),
                    usize::from(img.width) * usize::from(img.height)
                );
            }
            if ext == "map" || ext == "min" {
                assert_eq!(imgs.len(), 1, "{}", f.display());
            }
            count += 1;
        }
    }
    assert!(
        count >= 48,
        "expected at least 48 image blobs, found {count}"
    );
}

#[test]
fn every_sres_archive_parses_to_the_end() {
    let dir = need_data!();
    let mut total = 0;
    for f in files_with_ext(&dir, "res") {
        let data = std::fs::read(&f).unwrap();
        let a = sres::parse(&data).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        assert_eq!(a.version, sres::VERSION);
        assert!(!a.entries.is_empty());
        assert!(
            a.offsets.is_empty() || a.offsets.len() == a.entries.len() + 1,
            "{}",
            f.display()
        );
        for e in &a.entries {
            assert!(
                e.unknown_0x08 <= 1,
                "{}: entry {} unknown_0x08 = {}",
                f.display(),
                e.id,
                e.unknown_0x08
            );
        }
        total += a.entries.len();
    }
    assert!(
        total > 800,
        "expected > 800 entries across archives, found {total}"
    );
}

#[test]
fn every_rhp_and_rhm_container_walks_cleanly() {
    let dir = need_data!();
    let rhp = files_with_ext(&dir, "rhp");
    assert_eq!(rhp.len(), 9);
    for f in rhp {
        let data = std::fs::read(&f).unwrap();
        let c = chunk::parse_container(&data, b"MEUH")
            .unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        assert_eq!(c.version, 2);
        for tag in [b"SPOK", b"STAT", b"FACE", b"WOAW"] {
            assert!(
                c.child(tag).is_some(),
                "{}: missing {}",
                f.display(),
                String::from_utf8_lossy(tag)
            );
        }
    }
    let rhm = files_with_ext(&dir, "rhm");
    assert_eq!(rhm.len(), 39);
    for f in rhm {
        let data = std::fs::read(&f).unwrap();
        let c = chunk::parse_container(&data, b"DUTY")
            .unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        assert_eq!(c.version, 2);
        assert!(
            c.child(b"FOOT").is_some() && c.child(b"BOYZ").is_some(),
            "{}",
            f.display()
        );
    }
}

#[test]
fn every_scb_has_a_source_path() {
    let dir = need_data!();
    let files = files_with_ext(&dir, "scb");
    assert_eq!(files.len(), 39);
    for f in files {
        let data = std::fs::read(&f).unwrap();
        let h = scb::parse_header(&data).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        assert!((h.version - 1.5).abs() < 1e-6, "{}", f.display());
        assert!(
            h.source_path.to_lowercase().ends_with(".scs"),
            "{}",
            f.display()
        );
    }
}

#[test]
fn sprite_profiles_reference_frames_inside_the_dictionary_table() {
    let dir = need_data!();
    let dic_data = std::fs::read(dir.join("DATA/robinhood.dic")).unwrap();
    let d = dic::parse(&dic_data).unwrap();
    assert_eq!(d.page_count, 134);
    assert_eq!(d.symbols_per_page, 4096);
    assert_eq!(d.frames.len(), 404_855);
    let bks_len = std::fs::metadata(dir.join("DATA/robinhood.bks"))
        .unwrap()
        .len();
    let last = d.frames.last().unwrap();
    assert_eq!(u64::from(last.offset) + u64::from(last.length), bks_len);
    assert!(
        d.frames
            .iter()
            .all(|f| f.page == dic::NO_PAGE || usize::from(f.page) < usize::from(d.page_count))
    );
    assert_eq!(
        d.frames.iter().filter(|f| f.page == dic::NO_PAGE).count(),
        10_134
    );

    let files = files_with_ext(&dir, "rhs");
    assert_eq!(files.len(), 233);
    let mut refs = 0usize;
    for f in files {
        let data = std::fs::read(&f).unwrap();
        let p = rhs::parse(&data).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        assert_eq!(p.bank_generation, d.bank_generation);
        for idx in p.frame_indices() {
            let rec = d
                .frame(idx)
                .unwrap_or_else(|| panic!("{}: frame {idx} out of table", f.display()));
            assert!(
                rec.length.is_multiple_of(2),
                "odd symbol stream length for frame {idx}"
            );
            refs += 1;
        }
    }
    assert!(
        refs > 400_000,
        "expected > 400k unique frame references, found {refs}"
    );
}

#[test]
fn sprite_frames_decode_to_their_dimensions() {
    use std::io::{Read, Seek, SeekFrom};

    let dir = need_data!();
    let dic_data = std::fs::read(dir.join("DATA/robinhood.dic")).unwrap();
    let d = dic::parse(&dic_data).unwrap();
    let pages = sprite_decode::parse_pages(&d).unwrap();
    assert_eq!(pages.pages.len(), 134);
    assert_eq!(pages.pages[0].entries.len(), 4096);
    assert_eq!(pages.frame_count as usize, d.frames.len());
    assert!(pages.pages.iter().all(|p| p.entries.len() <= 4096));

    let mut bks = std::fs::File::open(dir.join("DATA/robinhood.bks")).unwrap();
    let mut decode = |index: usize| {
        let rec = &d.frames[index];
        let mut stream = vec![0u8; rec.length as usize];
        bks.seek(SeekFrom::Start(u64::from(rec.offset))).unwrap();
        bks.read_exact(&mut stream).unwrap();
        let img = sprite_decode::decode_frame(rec, &stream, &pages)
            .unwrap_or_else(|e| panic!("frame {index}: {e}"));
        assert_eq!(img.width, rec.width);
        assert_eq!(img.height, rec.height);
        assert_eq!(
            img.pixels.len(),
            usize::from(rec.width) * usize::from(rec.height)
        );
        img
    };
    // Frame 0 is the 4x1 placeholder: one symbol expanding to four transparent pixels.
    let first = decode(0);
    assert!(first.pixels.iter().all(|&p| p == sprite_decode::COLOR_KEY));
    // A sample across the whole table (both encodings), plus every page-less frame in the
    // first block of them.
    let mut count = 1;
    let mut pageless = 0;
    for i in (1..d.frames.len()).step_by(97) {
        decode(i);
        count += 1;
    }
    for (i, rec) in d.frames.iter().enumerate() {
        if rec.page == dic::NO_PAGE {
            decode(i);
            pageless += 1;
            if pageless == 500 {
                break;
            }
        }
    }
    assert!(count > 4000, "decoded {count} sampled frames");
    assert_eq!(pageless, 500);
}

#[test]
fn every_bitmap_font_parses_with_two_equal_strips() {
    use opensherwood_formats::font;

    let dir = need_data!();
    let fonts = dir.join("DATA/Interface/Fonts");
    let mut files = files_with_ext(&fonts, "bfn");
    files.extend(files_with_ext(&fonts, "fnt"));
    assert_eq!(files.len(), 11, "expected 10 .bfn + dialog.fnt");
    for f in &files {
        let data = std::fs::read(f).unwrap();
        assert_eq!(detect(&data), FileKind::BitmapFont, "{}", f.display());
        let font = font::parse_bitmap(&data).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        assert_eq!(font.version, 0x200, "{}", f.display());
        assert!(
            font.glyphs.len() == 161 || font.glyphs.len() == 162,
            "{}: {} glyphs",
            f.display(),
            font.glyphs.len()
        );
        assert_eq!(font.cell_height, u32::from(font.colour.height));
        assert_eq!(font.colour.width, font.mask.width);
        assert_eq!(font.colour.height, font.mask.height);
        assert!((13..=30).contains(&font.cell_height), "{}", f.display());
        assert!(
            font.glyphs.windows(2).all(|w| w[0].code < w[1].code),
            "{}: codes not ascending",
            f.display()
        );
        for c in 0x20u16..=0x7e {
            assert!(
                font.glyphs.iter().any(|g| g.code == c),
                "{}: missing glyph {c:#x}",
                f.display()
            );
        }
        assert!(font.glyphs.iter().any(|g| g.code == 0x2026));
        for g in &font.glyphs {
            assert!(
                g.width <= 25,
                "{}: glyph {:#x} width {}",
                f.display(),
                g.code,
                g.width
            );
            assert!((-2..=4).contains(&g.x_adjust));
            assert!((-8..=5).contains(&g.advance_adjust));
            let img = font.glyph_rgba(g);
            assert_eq!(img.pixels.len(), (g.width * font.cell_height * 4) as usize);
        }
        // Every mask word is a (near) grey (r, 2r, r); the EditFields/MenuButton mask has 15 pixels
        // off by up to 2 units in blue and 4 in green.
        assert!(font.mask.pixels.iter().all(|&m| {
            let r = i32::from((m >> 11) & 0x1f);
            let g = i32::from((m >> 5) & 0x3f);
            let b = i32::from(m & 0x1f);
            (r - b).abs() <= 2 && (g - 2 * r).abs() <= 4
        }));
        // 'H' has ink: at least one opaque mask pixel inside its cell.
        let h = font.glyph('H').unwrap();
        assert!(font.glyph_rgba(h).pixels.chunks(4).any(|p| p[3] == 255));
    }
}

#[test]
fn every_truetype_descriptor_and_manager_cfg_parse() {
    use opensherwood_formats::font;

    let dir = need_data!();
    let fonts = dir.join("DATA/Interface/Fonts");
    let files = files_with_ext(&fonts, "tfn");
    assert_eq!(files.len(), 16);
    for f in &files {
        let data = std::fs::read(f).unwrap();
        assert_eq!(detect(&data), FileKind::TrueTypeFont, "{}", f.display());
        let t = font::parse_truetype(&data).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        assert_eq!(t.version, 0x100);
        assert!(
            (11..=34).contains(&t.size),
            "{}: size {}",
            f.display(),
            t.size
        );
        assert!(
            t.face == "SimSun" || t.face == "Arial",
            "{}: {:?}",
            f.display(),
            t.face
        );
        assert!(t.unknown_2e <= 1);
        assert_eq!(t.unknown_59, 0);
    }
    let cfg = std::fs::read(fonts.join("manager.cfg")).unwrap();
    let roles = font::parse_manager_cfg(&String::from_utf8_lossy(&cfg));
    assert_eq!(roles.len(), 20);
    let names: Vec<String> = std::fs::read_dir(&fonts)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_ascii_lowercase())
        .collect();
    for r in &roles {
        assert!(r.bitmap.is_some() || r.truetype.is_some(), "{}", r.role);
        for name in r.bitmap.iter().chain(r.truetype.iter()) {
            assert!(
                names.contains(&name.to_ascii_lowercase()),
                "{}: {name} missing",
                r.role
            );
        }
    }
}

#[test]
fn every_mission_parses_and_its_references_resolve() {
    use opensherwood_formats::rhm;
    let dir = need_data!();
    let files = files_with_ext(&dir, "rhm");
    assert_eq!(files.len(), 39);
    let mut npcs_total = 0usize;
    for f in &files {
        let data = std::fs::read(f).unwrap();
        let m = rhm::parse(&data).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        assert_eq!(m.version, rhm::VERSION, "{}", f.display());
        assert_eq!(m.header.version, 4);
        assert!(m.unknown_chunks.is_empty(), "{}", f.display());
        let tags: Vec<[u8; 4]> = m.chunk_versions.iter().map(|(t, _)| *t).collect();
        assert_eq!(
            tags,
            [
                *b"FOOT", *b"POUF", *b"BOYZ", *b"ZORG", *b"HIRN", *b"RAIL", *b"SKRO", *b"TING",
                *b"GULP", *b"CAVE"
            ],
            "{}",
            f.display()
        );
        // Actor groups always come in the same order with the same versions.
        let groups: Vec<(&str, u32)> = m
            .actor_groups
            .iter()
            .map(|g| match g {
                rhm::ActorGroup::Meow { version, .. } => ("MEOW", *version),
                rhm::ActorGroup::PlayerCharacters { version, .. } => ("SCOT", *version),
                rhm::ActorGroup::Civilians { version, .. } => ("OILE", *version),
                rhm::ActorGroup::Vips { version, .. } => ("TOTO", *version),
                rhm::ActorGroup::Npcs { version, .. } => ("BORG", *version),
                rhm::ActorGroup::Objects { version, .. } => ("BOOM", *version),
                rhm::ActorGroup::Unknown { .. } => ("?", 0),
            })
            .collect();
        assert_eq!(
            groups,
            [
                ("MEOW", 2),
                ("SCOT", 4),
                ("OILE", 3),
                ("TOTO", 2),
                ("BORG", 4),
                ("BOOM", 5)
            ],
            "{}",
            f.display()
        );
        let npcs = m.npcs();
        npcs_total += npcs.len();
        for n in npcs {
            assert!(n.placement.direction < 16);
            assert!(
                n.rail == -1 || (n.rail as usize) < m.rails.len(),
                "{}",
                f.display()
            );
            assert!(n.members.iter().all(|&i| usize::from(i) < npcs.len()));
        }
        for pc in m.player_characters() {
            assert!(pc.placement.direction < 16);
        }
        for s in &m.scrolls {
            assert_eq!(s.placement.unknown_0x08, 190);
        }
        for z in &m.zorg {
            assert_eq!(z.placement.unknown_0x08, 189 + u32::from(z.unknown_b));
        }
        for rail in &m.rails {
            for p in rail {
                for t in &p.tables {
                    let sum: u32 = t.blocks.iter().map(|b| u32::from(b.percent)).sum();
                    assert!(sum <= 100, "{}", f.display());
                }
            }
        }
        // Every script class (except the level's StartUp) is a mission element and vice versa.
        let stem = f.file_stem().unwrap().to_str().unwrap();
        let scb_path = ["scb", "SCB"]
            .iter()
            .map(|_| f.with_extension("scb"))
            .find(|p| p.exists())
            .unwrap_or_else(|| f.with_file_name("sherwood.scb"));
        let script = scb::parse(&std::fs::read(&scb_path).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", scb_path.display()));
        let names = m.script_names();
        for c in &script.classes[1..] {
            assert!(
                names.contains(&c.name.as_str()),
                "{stem}: class {} has no mission element",
                c.name
            );
        }
        assert_eq!(script.classes[0].name, "StartUp", "{stem}");
    }
    assert_eq!(npcs_total, 2463);

    // The tutorial in detail.
    let m = rhm::parse(&std::fs::read(dir.join("DATA/Levels/EmbTut_FoC_EC.rhm")).unwrap()).unwrap();
    assert_eq!(m.header.map, "Croisement03");
    assert_eq!(
        (m.header.map_id, m.header.variant, m.header.mission_id),
        (101, 1, 20)
    );
    assert_eq!(m.player_characters().len(), 5);
    assert_eq!(m.npcs().len(), 22);
    assert_eq!(m.objects().len(), 14);
    assert_eq!(m.rails.len(), 18);
    assert_eq!(m.brains.waypoints.len(), 25);
    assert_eq!(m.brains.beam_points.len(), 10);
    assert_eq!(m.scrolls.len(), 6);
    assert_eq!(m.script_areas.polygons.len(), 6);
    assert_eq!(m.script_areas.points.len(), 42);
    assert_eq!(m.mobiles.len(), 1);
    assert_eq!(m.mobiles[0].animations[0].sprite, "chariot05");
    let pcs = m.player_characters();
    assert_eq!(pcs[0].name, None);
    assert!(pcs[1].name.as_deref().is_some_and(|n| n.len() > 9 && n[n.len() - 8..].chars().all(|c| c.is_ascii_hexdigit())));
    assert_eq!((pcs[0].placement.x, pcs[0].placement.y), (1228, 462));
    // Beam-me points sit on the map edges (1408x960).
    for p in &m.brains.beam_points {
        assert!(
            p.x < 60 || p.x > 1348 || p.y < 60 || p.y > 900,
            "({}, {})",
            p.x,
            p.y
        );
    }
}

#[test]
fn every_script_parses_and_its_code_is_consistent() {
    let dir = need_data!();
    let files = files_with_ext(&dir, "scb");
    assert_eq!(files.len(), 39);
    let mut quads = 0usize;
    for f in files {
        let data = std::fs::read(&f).unwrap();
        let s = scb::parse(&data).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        assert!((s.version - scb::VERSION).abs() < 1e-6);
        for c in &s.classes {
            assert!(c.source_path.to_lowercase().ends_with(".scs"));
            let addrs: Vec<u32> = c.functions.iter().map(|f| f.address).collect();
            for (i, func) in c.functions.iter().enumerate() {
                let body = c.function_quads(i);
                let first = body
                    .first()
                    .unwrap_or_else(|| panic!("{}: empty function", c.name));
                assert_eq!(first.opcode, 0x03, "{}: {}", c.name, func.name);
                assert_eq!(u32::from(first.a), func.size_of_volatile);
                assert_eq!(u32::from(first.b), func.size_of_tempor);
            }
            let n = c.quads.len() as u32;
            for q in &c.quads {
                match q.opcode {
                    0x05 => assert!(addrs.contains(&u32::from(q.a)), "{}: call {}", c.name, q.a),
                    0x0c => assert!(q.a < 300 && q.b == 0 && q.c == 0),
                    0x0f => assert!(q.c < n, "{}: jump target {}", c.name, q.c),
                    _ => {}
                }
                assert!(
                    scb::layout(q.opcode) != scb::Layout::Unknown,
                    "opcode {:#x}",
                    q.opcode
                );
            }
            quads += c.quads.len();
        }
    }
    assert_eq!(quads, 208_679);
}

/// Width and height of the Day background of a map, from the image blob header.
fn day_map_size(dir: &Path, stem: &str) -> Option<(u16, u16)> {
    for e in std::fs::read_dir(dir.join("DATA/Levels/Day"))
        .ok()?
        .flatten()
    {
        let p = e.path();
        let is_map = p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("map"));
        let same_stem = p
            .file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case(stem));
        if is_map && same_stem {
            let data = std::fs::read(&p).ok()?;
            let h = data.get(..4)?;
            return Some((
                u16::from_le_bytes([h[0], h[1]]),
                u16::from_le_bytes([h[2], h[3]]),
            ));
        }
    }
    None
}

#[test]
fn every_rhp_decodes_and_its_geometry_lies_inside_the_background() {
    use opensherwood_formats::rhp;
    let dir = need_data!();
    let files = files_with_ext(&dir, "rhp");
    assert_eq!(files.len(), 9);
    for f in files {
        let data = std::fs::read(&f).unwrap();
        let m = rhp::parse(&data).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let stem = f.file_stem().unwrap().to_str().unwrap();
        let (w, h) = day_map_size(&dir, stem).unwrap_or_else(|| panic!("no Day map for {stem}"));
        let name = f.display().to_string();
        let inside = |p: &rhp::Point| p.x <= w && p.y <= h;

        assert_eq!(m.version, rhp::VERSION, "{name}");
        assert!(m.spok.unknown_0x04 <= 1, "{name}");
        assert_eq!(m.spok.unknown_0x08, 0, "{name}");

        // STAT: boundary of the walkable ground, segments and obstacle outlines.
        assert_eq!(m.stat.boundary_id, 0x5a, "{name}");
        assert_eq!(m.stat.segments_id, 0x82, "{name}");
        assert!(m.stat.boundary.len() >= 3, "{name}");
        assert!(m.stat.boundary.iter().all(inside), "{name}: boundary");
        assert!(!m.stat.obstacles.is_empty(), "{name}");
        for o in &m.stat.obstacles {
            assert!(o.polygon.points.len() >= 3, "{name}");
            assert!(o.polygon.points.iter().all(inside), "{name}: obstacle");
        }
        for s in &m.stat.segments {
            assert!(inside(&s.a) && inside(&s.b), "{name}: segment");
        }
        assert!(!m.stat.rest.is_empty(), "{name}");

        // TEXT and DARK zones.
        for z in &m.text {
            assert!(
                z.polygon.points.len() >= 3 && z.polygon.points.iter().all(inside),
                "{name}"
            );
        }
        for d in &m.dark {
            assert!(
                d.polygon.points.len() >= 3 && d.polygon.points.iter().all(inside),
                "{name}"
            );
            assert!(matches!(d.unknown_value, 2 | 4 | 6), "{name}");
        }

        // WOAW: projection areas with a consistent bounding box.
        assert!(
            !m.woaw.layers.is_empty() && !m.woaw.areas.is_empty(),
            "{name}"
        );
        for a in &m.woaw.areas {
            assert!(a.points.len() >= 3, "{name}");
            let (mut minx, mut miny, mut maxx, mut maxy) = (
                f32::INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
            );
            for p in &a.points {
                minx = minx.min(p.x);
                miny = miny.min(p.y);
                maxx = maxx.max(p.x);
                maxy = maxy.max(p.y);
            }
            assert!(
                (minx - a.min[0]).abs() < 0.01 && (miny - a.min[1]).abs() < 0.01,
                "{name}"
            );
            assert!(
                (maxx - a.max[0]).abs() < 0.01 && (maxy - a.max[1]).abs() < 0.01,
                "{name}"
            );
        }
        // 007: bonds reference areas.
        assert!(!m.bonds.is_empty(), "{name}");
        for b in &m.bonds {
            assert!(
                usize::from(b.area_a) < m.woaw.areas.len(),
                "{name}: bond {b:?}"
            );
            assert!(
                b.area_b == rhp::NO_AREA || usize::from(b.area_b) < m.woaw.areas.len(),
                "{name}: bond {b:?}"
            );
            for (x, y) in [(b.x1, b.y1), (b.x2, b.y2)] {
                assert!(
                    x >= -64 && y >= -64 && x <= w as i16 + 64 && y <= h as i16 + 64,
                    "{name}"
                );
            }
        }

        // FACE: occluder masks inside the background.
        assert!(!m.faces.is_empty(), "{name}");
        for fc in &m.faces {
            assert!(fc.width > 0 && fc.height > 0, "{name}");
            assert!(
                u32::from(fc.x) + u32::from(fc.width) <= u32::from(w)
                    && u32::from(fc.y) + u32::from(fc.height) <= u32::from(h),
                "{name}: mask {}x{} at {},{}",
                fc.width,
                fc.height,
                fc.x,
                fc.y
            );
            assert_eq!(
                fc.mask.len(),
                fc.stride() * usize::from(fc.height),
                "{name}"
            );
            assert_eq!(
                fc.lines.len(),
                (fc.kind & 3).count_ones() as usize,
                "{name}"
            );
            assert!(
                fc.refs.is_empty() || fc.kind & rhp::FACE_HAS_REFS != 0,
                "{name}"
            );
            assert!(fc.unknown_0x00 <= 12, "{name}");
            for r in &fc.refs {
                assert!(usize::from(*r) < m.woaw.areas.len(), "{name}: face ref {r}");
            }
        }

        // PPPP: zones and jump lines.
        for z in &m.pppp.zones {
            assert!(
                z.polygon.points.len() >= 3 && z.polygon.points.iter().all(inside),
                "{name}"
            );
        }
        for j in &m.pppp.jump_lines {
            for p in j.from.iter().chain(j.to.iter()) {
                assert!(p.x <= w && p.y <= h, "{name}: jump line {p:?}");
            }
        }

        // FLIM: animated elements.
        for fl in &m.flims {
            assert!(!fl.sprite.is_empty(), "{name}");
            assert!(
                fl.line.points.is_empty() || fl.line.points.len() == 2,
                "{name}"
            );
            assert!(fl.unknown_flags.iter().all(|&b| b <= 1), "{name}");
        }

        if stem.eq_ignore_ascii_case("Croisement01") {
            assert_eq!((w, h), (1408, 960));
            assert_eq!(m.spok.unknown_0x00, 100);
            assert_eq!(m.stat.boundary.len(), 79);
            assert_eq!(m.stat.obstacles.len(), 46);
            assert_eq!(m.text.len(), 4);
            assert_eq!(m.woaw.layers, vec![0, 1, 2, 3]);
            assert_eq!(m.woaw.areas.len(), 85);
            assert_eq!(m.bonds.len(), 29);
            assert_eq!(m.faces.len(), 103);
            assert_eq!(m.flims.len(), 13);
            assert_eq!(m.pppp.zones.len(), 11);
            assert_eq!(m.pppp.jump_lines.len(), 13);
            assert!(m.dark.is_empty());
        }
    }
}

#[test]
fn profile_table_parses_and_its_sprites_exist() {
    let dir = need_data!();
    let data = std::fs::read(dir.join("DATA/Configuration/profile.cpf")).unwrap();
    assert_eq!(detect(&data), FileKind::ProfileTable);
    let t = cpf::parse(&data).unwrap();
    assert_eq!(t.table_a.len(), 27);
    assert_eq!(t.table_b.len(), 4);
    assert_eq!(t.player_characters.len(), 10);
    assert_eq!(t.soldiers.len(), 68);
    assert_eq!(t.levels.len(), 63);
    assert_eq!(t.civilians.len(), 24);
    // Every sprite of the three actor tables is a `Characters/<sprite>.rhs` whose sequence name
    // is the record's `sequence`.
    let characters = dir.join("DATA/Characters");
    let files: Vec<(String, PathBuf)> = std::fs::read_dir(&characters)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("rhs"))
        })
        .map(|p| {
            (
                p.file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_ascii_lowercase(),
                p,
            )
        })
        .collect();
    let check = |sprite: &str, sequence: &str, voice: &str| {
        let (_, path) = files
            .iter()
            .find(|(stem, _)| *stem == sprite.to_ascii_lowercase())
            .unwrap_or_else(|| panic!("Characters/{sprite}.rhs missing"));
        let p = rhs::parse(&std::fs::read(path).unwrap()).unwrap();
        assert!(
            p.sequences.iter().any(|s| s.name == sequence),
            "{sprite}: sequence {sequence:?} not in {:?}",
            p.sequences.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        assert!(voice.is_empty() || voice.len() == 4, "{sprite}: {voice:?}");
    };
    for pc in &t.player_characters {
        check(&pc.sprite, &pc.sequence, &pc.voice);
        assert!(pc.voice.starts_with("PC"), "{:?}", pc.voice);
    }
    for sd in &t.soldiers {
        check(&sd.sprite, &sd.sequence, &sd.voice);
        assert!(
            sd.voice.starts_with("SD") || sd.voice.starts_with("VP"),
            "{:?}",
            sd.voice
        );
    }
    for cv in &t.civilians {
        check(&cv.sprite, &cv.sequence, &cv.voice);
    }
    assert_eq!(t.player_characters[0].sprite, "RobinHood");
    assert_eq!(t.soldiers[0].sprite, "Guard A00");
    assert_eq!(t.soldiers[6].sprite, "Soldier A00");
    assert_eq!(t.civilians[1].sprite, "Mendicant");
    // Level table: every retail mission file is named by exactly one record; the placeholders
    // point at `Impossible_mission`; the campaign graph codes reference existing codes.
    let missions: Vec<String> = files_with_ext(&dir.join("DATA/Levels"), "rhm")
        .iter()
        .map(|p| p.file_stem().unwrap().to_str().unwrap().to_string())
        .collect();
    let mut named = 0;
    for l in &t.levels {
        assert_eq!(l.code.len(), 2, "{:?}", l.code);
        assert_eq!(l.unknown_fixed, [0, 10000], "{}", l.code);
        assert!((1..=9).contains(&l.location), "{}", l.code);
        if l.mission_file == "Impossible_mission" {
            continue;
        }
        assert!(
            missions
                .iter()
                .any(|m| m.eq_ignore_ascii_case(&l.mission_file)),
            "{}: {} has no .rhm",
            l.code,
            l.mission_file
        );
        named += 1;
        for c in l.after.iter().chain(&l.until) {
            assert!(t.level(c).is_some(), "{}: unknown code {c}", l.code);
        }
    }
    assert_eq!(named, missions.len());
    let ha = t.level("HA").unwrap();
    assert_eq!(ha.mission_file, "H01_Lin_VL");
    assert_eq!(ha.map, "Lincoln");
    assert!(ha.after.is_empty());
    assert_eq!(ha.until, vec!["HA".to_string()]);
    assert_eq!(t.level("SA").unwrap().after, vec!["HA".to_string()]);
}
