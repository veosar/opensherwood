# Building, running, testing

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Rust | pinned in `rust-toolchain.toml` (1.95) | `rustup` installs it automatically |
| C compiler | any | needed by vendored C crates (Lua, compression). Windows: MSVC Build Tools 2022 with the "Desktop C++" workload; Linux: `gcc`/`clang`; macOS: Xcode CLT |
| Python | 3.12+ | harness; `pip install -r harness/requirements.txt` |
| cargo-nextest | latest | `cargo install cargo-nextest` (optional; `cargo test` works too) |

Windows one-liner for the C toolchain (admin shell):

```
winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

## Build

```
cargo build --workspace            # debug
cargo build --workspace --release  # release
```

## Run

```
# headless RPC server (what the harness uses)
cargo run -p opensherwood-app -- --rpc stdio --headless

# game window: synthetic scenario (no game data needed)
cargo run -p opensherwood-app --release -- --game-dir "C:\GOG Games\Robin Hood - The Legend of Sherwood"   # the original main menu
cargo run -p opensherwood-app --release -- --scenario corridor --scale 2

# game window: scroll around a retail map background with synthetic units on it
cargo run -p opensherwood-app --release -- --scenario map:sherwood:Day --game-dir "C:\GOG Games\Robin Hood - The Legend of Sherwood"

# game window that also accepts JSON-RPC on stdin (agents drive and screenshot the real window)
cargo run -p opensherwood-app --release -- --rpc stdio --scenario map:nottingham
python harness/tools/drive.py --scenario map:sherwood --window --out harness/out/drive

# inspect game files
cargo run -p opensherwood-tools -- inspect "C:\GOG Games\Robin Hood - The Legend of Sherwood\DATA\Interface\DEFAULT.RES"
```

`--game-dir` may be omitted if `OPENSHERWOOD_GAME_DIR` is set or the GOG / Steam installation is found automatically.

## Test

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace          # or: cargo test --workspace
python -m pytest harness/tests/synthetic
OPENSHERWOOD_GAME_DIR=... python -m pytest harness/tests/data     # local only
python scripts/sync_skills.py --check
python scripts/check_no_assets.py
```

`scripts/ci_local.sh` (or `.ps1`) runs the same sequence CI runs.

## Window flags

`--scenario menu | corridor | map:<name>[:<ambiance>] | mission:<name>` (default `menu`, the original main menu), `--windowed` (resizable window; the default is
borderless fullscreen at the desktop resolution, letterboxed), `--scale N` (integer window scale when windowed),
`--mute` (no audio device), `--rpc stdio` (accept harness commands; the simulation then advances only through
`step`; implies `--windowed`). F11 toggles fullscreen at runtime.

## Window controls (current)

Left click selects a unit, right click orders the selected unit to walk there, arrow keys or the pointer at the
window edge scroll the camera. The window is letterboxed to the logical 640x480 viewport (integer `--scale` on
start; resizable). Simulation runs at 60 ticks per second in window mode.

## Environment variables

| Variable | Meaning |
|---|---|
| `OPENSHERWOOD_GAME_DIR` | path to the game installation used by data-backed tests and tools |
| `OPENSHERWOOD_ARTIFACTS` | where `capture` writes PNGs (default `harness/out`) |
| `OPENSHERWOOD_LENIENT_ASSETS` | `1` makes a retail scenario load with logged defaults when a required dependency is missing or malformed (the map's `.rhp` geometry: everything walkable, no occluders; `Configuration/profile.cpf` or a sprite profile it references: default sprites; the sprite bank or a profile: no sprites; the mission's `.scb` script missing, malformed, untranslatable, with classes that bind to no mission element, or without a known element index space: the mission runs without a script or with the unbound classes inert). Off by default: the `reset` of a `map:` or `mission:` scenario fails with the file and the parser's message instead, because a world built without them (no geometry, empty catalog) would silently differ from the original. Diagnostic use only; never for replays or goldens |
| `OPENSHERWOOD_BIN` | engine binary used by the Python harness (default: `target/release` then `target/debug`) |
| `RUST_LOG` | tracing filter, e.g. `opensherwood_core=debug` |
