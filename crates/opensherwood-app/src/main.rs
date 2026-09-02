//! `opensherwood` binary. Milestone M0: headless JSON-RPC server over stdio (`docs/harness.md`).

mod session;

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
    match args.rpc.as_deref() {
        Some("stdio") => session::serve_stdio(game, artifacts),
        Some(other) => anyhow::bail!("unsupported --rpc transport '{other}' (only 'stdio')"),
        None => {
            if args.headless {
                anyhow::bail!("--headless without --rpc stdio does nothing");
            }
            anyhow::bail!(
                "the interactive window is not implemented yet (milestone M2); use --rpc stdio"
            )
        }
    }
}
