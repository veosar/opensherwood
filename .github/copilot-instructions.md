# OpenSherwood: instructions for Copilot code review

OpenSherwood is a GPLv3, asset-free Rust reimplementation of the engine of *Robin Hood: The Legend of Sherwood*
(2002). It reads the player's own game data and is built from behaviour specifications under a clean-room wall
(ADR-0009). The full rules are in `AGENTS.md` (also loaded via `CLAUDE.md`), `docs/legal.md` and `docs/decisions/`;
read them instead of guessing. The review checklist is the skill `code-review`. Write review comments in English.

## Stack and layout
Cargo workspace (Rust 1.95, edition 2024) plus a Python harness (`harness/`, JSON-RPC, pytest) and `scripts/`.
Dependencies point down only: `formats` <- `assets` <- `core` <- `script` <- `render` <- `protocol` <- `app` / `tools`.
Detail: `docs/architecture.md`. CI (`.github/workflows/ci.yml`): policy checks, fmt, clippy `-D warnings`, tests on
Linux/Windows/macOS, synthetic harness and a cross-platform determinism digest.

## What to flag (bugs and rule violations, not style)
1. Game content in the diff: any original asset, game text or string, screenshot with game art, savegame, replay or
   snapshot with a retail `content_fingerprint`, or a file type blocked by `scripts/check_no_assets.py`.
2. Decompiler leakage (ADR-0009): decompiler output, auto-generated names (`FUN_`, `DAT_`, `param_N`, `local_`,
   `undefined`), pseudocode or a line-by-line paraphrase of the executable, the binary's identifiers or string
   table, value tables copied from it. Never suggest reading `re/` or "translating" original code.
3. Determinism in simulation code: `HashMap`/`HashSet` iteration order, wall clock, threads, floats or platform
   behaviour affecting state, randomness outside the named seeded RNG streams in `opensherwood-core`.
4. Authoritative state not covered by `snapshot()`/`restore()`/canonical hash; simulation semantics changed
   without bumping `RULESET_VERSION`/`HASH_SCHEMA_VERSION`/`SNAPSHOT_VERSION` and the golden digest.
5. Dependency direction broken, or `unsafe`/FFI/platform code outside `opensherwood-app` (`unsafe_code = "forbid"`).
6. Guessed semantics in code without a spec claim; unknown fields not named `unknown_*`; panics on malformed
   input in parsers.
7. Behaviour, flag, RPC method or format changed without the matching doc (`docs/`, `README.md`, `docs/harness.md`).
8. A disabled or weakened test, a force-added ignored path (`re/`, `harness/goldens`, `harness/out`), edits to
   `.claude/skills/` (generated from `.agents/skills/` by `scripts/sync_skills.py`), a dependency that is not
   GPLv3-compatible.

## Do not flag
Clippy-pedantic lints already allowed in the root `Cargo.toml`, naming or formatting (rustfmt decides), long
specs and ADRs, or the absence of `re/` (it is git-ignored on purpose).
