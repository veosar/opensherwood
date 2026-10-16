//! Local characterization of CLI text and PNG output; no game content is stored here.
use std::path::Path;
use std::process::Command;

fn digest(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn output(root: &Path, args: &[&str]) -> Vec<u8> {
    let result = Command::new(env!("CARGO_BIN_EXE_opensherwood-tools"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty(), "{args:?}");
    result.stdout
}

#[test]
fn retail_cli_text_and_overlays_are_stable() {
    let Some(root) = std::env::var_os("OPENSHERWOOD_GAME_DIR") else {
        return;
    };
    let root = Path::new(&root);
    let commands: &[&[&str]] = &[
        &["inspect", "DATA/Levels/EmbTut_FoC_EC.rhm"],
        &["sres", "DATA/Interface/DEFAULT.RES"],
        &["chunks", "DATA/Levels/Croisement01.rhp"],
        &["cpf", "DATA/Configuration/profile.cpf", "--hex"],
        &["rhm", "DATA/Levels/EmbTut_FoC_EC.rhm", "--programs"],
        &["rhp", "DATA/Levels/Croisement01.rhp"],
    ];
    let observed: Vec<u64> = commands
        .iter()
        .map(|args| digest(&output(root, args)))
        .collect();
    assert_eq!(
        observed,
        vec![
            6_273_942_253_363_411_711,
            7_254_066_248_619_692_263,
            16_063_743_139_653_247_749,
            13_612_986_343_322_970_565,
            2_383_075_233_930_951_158,
            17_149_257_797_542_501_374
        ]
    );
    let out = std::env::temp_dir().join(format!("opensherwood-cli-{}.png", std::process::id()));
    let out_arg = out.to_str().unwrap();
    let mut images = Vec::new();
    for args in [
        [
            "rhm-overlay",
            "DATA/Levels/EmbTut_FoC_EC.rhm",
            "DATA/Levels/Day/Croisement03.map",
            out_arg,
        ],
        [
            "rhp-overlay",
            "DATA/Levels/Croisement01.rhp",
            "DATA/Levels/Day/Croisement01.map",
            out_arg,
        ],
    ] {
        output(root, &args);
        images.push(digest(&std::fs::read(&out).unwrap()));
    }
    std::fs::remove_file(out).unwrap();
    assert_eq!(
        images,
        vec![4_057_812_516_551_272_859, 14_359_063_250_409_098_468]
    );
}
