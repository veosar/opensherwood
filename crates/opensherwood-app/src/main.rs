//! `opensherwood` binary: headless JSON-RPC server (`--rpc stdio --headless`) or the interactive
//! window (which also accepts JSON-RPC on stdin when `--rpc stdio` is given, so the harness can
//! drive and screenshot the real window).

mod engine;
mod mission;
mod rpc;
mod ui;
mod ui_assets;
mod window;

use std::path::PathBuf;

use clap::Parser;

/// OpenSherwood: open engine for Robin Hood: The Legend of Sherwood.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    /// Serve the harness protocol on stdin/stdout ("stdio" is the only transport for now).
    #[arg(long)]
    rpc: Option<String>,
    /// Never open a window.
    #[arg(long)]
    headless: bool,
    /// Game installation directory (else OPENSHERWOOD_GAME_DIR, else auto-detect).
    #[arg(long)]
    game_dir: Option<PathBuf>,
    /// Directory for captures (else OPENSHERWOOD_ARTIFACTS, else ./harness/out).
    #[arg(long)]
    artifacts: Option<PathBuf>,
    /// Scenario to load at start in window mode: `menu`, `corridor`, `map:<name>[:<ambiance>]`,
    /// `mission:<name>`.
    #[arg(long, default_value = "menu")]
    scenario: String,
    /// Integer window scale factor.
    #[arg(long, default_value_t = 2)]
    scale: u32,
    /// Do not open an audio device.
    #[arg(long)]
    mute: bool,
    /// Start in a resizable window instead of borderless fullscreen.
    #[arg(long)]
    windowed: bool,
    /// Mission scripts: treat unknown natives as recorded no-ops instead of stopping the
    /// callback (the calls are logged in the snapshot; see docs/harness.md).
    #[arg(long)]
    lenient_natives: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let artifacts = args
        .artifacts
        .or_else(|| std::env::var_os("OPENSHERWOOD_ARTIFACTS").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("harness/out"));
    let game = match opensherwood_assets::GameDir::discover(args.game_dir.as_deref()) {
        Ok(g) => {
            eprintln!("opensherwood: game directory {}", g.root.display());
            Some(g)
        }
        Err(e) => {
            eprintln!("opensherwood: no game directory ({e}); synthetic scenarios only");
            None
        }
    };
    let rpc = match args.rpc.as_deref() {
        None => false,
        Some("stdio") => true,
        Some(other) => anyhow::bail!("unsupported --rpc transport '{other}' (only 'stdio')"),
    };
    let mut session = engine::Session::new(game, artifacts);
    session.load_settings();
    session.set_lenient_natives(args.lenient_natives);
    if args.headless {
        if !rpc {
            anyhow::bail!("--headless without --rpc stdio does nothing");
        }
        return rpc::serve_stdio(session);
    }
    window::run(
        session,
        rpc,
        &args.scenario,
        window::Presentation {
            scale: args.scale,
            mute: args.mute,
            windowed: args.windowed || rpc,
        },
    )
}
