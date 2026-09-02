//! Data-backed tests: run only when `OPENSHERWOOD_GAME_DIR` points at a copy of the game.
//! They assert the invariants recorded in `docs/formats/*.md` over every file of each kind.

use std::path::{Path, PathBuf};

use opensherwood_formats::{FileKind, chunk, detect, dic, image_blob, rhs, scb, sres};

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
