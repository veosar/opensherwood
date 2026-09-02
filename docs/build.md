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
cargo run -p locksley-app -- --rpc stdio --headless

# game window (M2+)
cargo run -p locksley-app --release -- --game-dir "C:\GOG Games\Robin Hood - The Legend of Sherwood"

# inspect game files
cargo run -p locksley-tools -- inspect "C:\GOG Games\Robin Hood - The Legend of Sherwood\DATA\Interface\DEFAULT.RES"
```

`--game-dir` may be omitted if `LOCKSLEY_GAME_DIR` is set or the GOG / Steam installation is found automatically.

## Test

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace          # or: cargo test --workspace
python -m pytest harness/tests/synthetic
LOCKSLEY_GAME_DIR=... python -m pytest harness/tests/data     # local only
python scripts/sync_skills.py --check
python scripts/check_no_assets.py
```

`scripts/ci_local.sh` (or `.ps1`) runs the same sequence CI runs.

## Environment variables

| Variable | Meaning |
|---|---|
| `LOCKSLEY_GAME_DIR` | path to the game installation used by data-backed tests and tools |
| `LOCKSLEY_ARTIFACTS` | where `capture` writes PNGs (default `harness/out`) |
| `RUST_LOG` | tracing filter, e.g. `locksley_core=debug` |
