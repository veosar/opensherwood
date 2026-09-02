# Instructions for AI coding agents (and humans)

This file is the single source of truth for how work is done in this repository. Claude Code loads it through
`CLAUDE.md`; Codex loads it directly. Skills with step-by-step procedures live in `.agents/skills/` (mirrored to
`.claude/skills/` by `scripts/sync_skills.py`; never edit the mirror).

## What this project is

OpenSherwood: a clean-room, GPLv3, asset-free reimplementation of the engine of *Robin Hood: The Legend of Sherwood*
(2002). Goal order: (1) play the full original campaign from the player's own data, (2) modern platform and QoL,
(3) modding: maps, missions, campaigns, characters, skins, scripts, (4) new modes, co-op, other games of the same
engine family. Non-goals: shipping any original content; matching the original's binary layout; a general-purpose
game engine.

## Hard rules (read `docs/legal.md` and `docs/decisions/ADR-0003-clean-room-roles.md`)

1. Never add game data to the repository in any form. `scripts/check_no_assets.py` runs in CI; it is a safety net,
   not the rule.
2. Clean room: decompiler or disassembler output never enters the repository, and a session that has looked at it
   does not implement the corresponding subsystem. Static analysis is an *analyst* task that ends in a spec
   document under `docs/`. Implementation works from specs, data-file observation and black-box behaviour.
3. Unknown fields are named `unknown_*` in specs and code. No guessed semantics in code without a spec claim.
4. Every fact about the original goes into `docs/formats/` or `docs/original/` with its Provenance.

## Determinism rules (see `docs/architecture.md`, ADR-0004)

- Simulation advances in fixed ticks; all randomness through named seeded RNG streams in `opensherwood-core`.
- No `HashMap` iteration order, wall clock, thread timing, float formatting or platform behaviour may influence
  simulation state.
- Everything authoritative is in `snapshot()`, restored by `restore()`, and covered by the canonical hash.
- Player actions in tests are canonical input events, never privileged commands (`debug.*` is for inspection).

## Layout and dependency direction

`opensherwood-formats` <- `opensherwood-assets` <- `opensherwood-core` <- `opensherwood-script` <- `opensherwood-render` <-
`opensherwood-protocol` <- `opensherwood-app` / `opensherwood-tools`. Lower crates never depend on higher ones. Platform,
FFI and `unsafe` code only in `opensherwood-app`. Python harness in `harness/`, helper scripts in `scripts/`.

## Approved commands

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace            # or cargo test --workspace
python -m pytest harness/tests/synthetic
python -m pytest harness/tests/data      # only with OPENSHERWOOD_GAME_DIR set; local
python scripts/sync_skills.py --check    # --write after editing .agents/skills
python scripts/check_no_assets.py
```

Game data for local tests: set `OPENSHERWOOD_GAME_DIR` to a *copy* of the installation, never to the store install.

## Definition of done

A change is done when: it builds on all targets; fmt / clippy / tests are green; the synthetic harness passes;
data-backed tests that touch the changed area pass locally; docs describing the changed behaviour (`docs/`,
`README.md`, `docs/build.md`, `docs/harness.md`, format specs) are updated in the same commit; the commit message
says why. "Implemented but not verified" is not done; say so explicitly in the commit or PR.

## Working style

- Small commits, pushed to `main` often (the human follows progress on GitHub). Feature branches for anything
  that breaks the build for more than one commit.
- Prefer boring code: explicit readers, plain structs, no macros that hide offsets, no premature abstraction.
- Read the relevant spec before touching a parser; update the spec when the parser teaches you something.
- When a decision is architectural, write an ADR in `docs/decisions/` and ask the other agent for review
  (skill `cross-agent-review`).
- Roadmap checkboxes in `docs/roadmap.md` are updated when a milestone item is verified, not when code lands.

## Forbidden

- `git add -f` on ignored paths; committing anything under `harness/goldens`, `harness/out`, `re/`.
- Uploading data-derived artifacts from local or self-hosted runs to CI or issues.
- Editing `.claude/skills/` directly (generated).
- Disabling a failing test to make CI green.
