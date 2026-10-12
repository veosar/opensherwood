---
name: code-review
description: "Review checklist for pull requests to OpenSherwood (Rust game-engine reimplementation under a clean-room wall). Use on every PR review diff: game assets and decompiler leakage (ADR-0009), determinism contract, snapshot/hash/ruleset bumps, crate dependency direction, unsafe code, parser robustness, spec and docs sync, generated skills mirror, CI policy."
---

# Reviewing a pull request in OpenSherwood

Comments in English, only for things you can point to in the diff: a bug, a rule violation or a concrete risk.
Not style (rustfmt and clippy decide). Rules live in `AGENTS.md`, `docs/legal.md`, `docs/architecture.md` and
`docs/decisions/` (ADR-0003, 0004, 0008, 0009, 0010); cite the rule you rely on. You are a reviewer of the
diff: never ask the author to read `re/` or to reproduce original code, and never paste or reconstruct decompiled
code in a comment.

## 1. Legal and clean-room (blocking)
- [ ] No game content in any form: binaries, graphics, audio, video, text or strings, maps, scripts, savegames,
      screenshots with game art, hex dumps of game files, tables copied from assets. Extensions and magics that
      `scripts/check_no_assets.py` blocks, including `.png`/`.jpg`, `.exe`/`.dll`.
- [ ] No replay, snapshot or fixture recorded from retail data (`content_fingerprint` of a real installation);
      nothing added under `re/`, `harness/goldens/`, `harness/captures/`, `harness/out/`.
- [ ] No decompiler output or traces: Ghidra project files, exporter output, auto-generated names (`FUN_`,
      `DAT_`, `param_N`, `local_`, `undefined`, p-code helper macros), pseudocode transcribed from the executable,
      the binary's identifiers or string table, line-by-line paraphrase in code, comments or specs.
- [ ] Code implementing a subsystem comes from a spec in `docs/`; unsettled rules are `Assumption` variants
      (ADR-0008), unknown fields are `unknown_*`, no guessed semantics presented as fact.
- [ ] New dependencies are GPLv3-compatible and their license is recorded as the `docs-sync` checklist requires; `Cargo.lock` is committed and the
      build uses `--locked`. No project name or trademark use beyond descriptive reference to the game.

## 2. Determinism (simulation crates)
- [ ] No `HashMap`/`HashSet` iteration order, wall clock, threads, environment, file I/O or float formatting
      affecting state, hashes, snapshots or event order.
- [ ] All randomness from named seeded RNG streams in `opensherwood-core`; draw order unchanged or deliberately
      changed together with the ruleset bump.
- [ ] Positions and timers stay fixed-point; no new `f32`/`f64` in hashed or authoritative state.
- [ ] Every authoritative field is in `snapshot()`, validated and restored in `restore()`/`validate`, and in the
      canonical hash; caches are rebuilt on restore.
- [ ] Changed simulation outcome: `RULESET_VERSION` bumped (and `HASH_SCHEMA_VERSION`/`SNAPSHOT_VERSION` if the
      encoding changed), golden test constant and `harness/fixtures/synthetic_corridor.json` updated on purpose,
      `docs/harness.md` updated.
- [ ] Rendering and audio read simulation state and never write it; tests inject canonical input events, not
      privileged commands.

## 3. Architecture and safety
- [ ] Crate dependency direction (`formats` <- `assets` <- `core` <- `script` <- `render` <- `protocol` <-
      `app`/`tools`) is not inverted; `core` has no I/O, rendering or platform types.
- [ ] `unsafe`, FFI and platform code only in `opensherwood-app` (the workspace forbids `unsafe_code`).
- [ ] Parsers in `opensherwood-formats` do not panic on malformed input: no `unwrap`/`expect`, unchecked index,
      unchecked arithmetic or allocation sized by file contents; errors carry offsets.
- [ ] Protocol types stay versioned serde types; an RPC or replay change is documented in `docs/harness.md`
      (ADR-0004).

## 4. Tests and docs
- [ ] No test disabled, skipped, loosened or made to pass by weakening an assertion. Bug fix has a regression test.
- [ ] Tests that need real game data live in `harness/tests/data` (local only), not in CI paths; synthetic
      fixtures have an origin manifest.
- [ ] Docs match the change in the same PR (`docs-sync` table in `.agents/skills/docs-sync/SKILL.md`): format spec
      and its Provenance, `docs/original/`, `docs/build.md`, `README.md`, roadmap checkbox only when verified.
- [ ] An architectural decision is a new ADR plus a row in `docs/decisions/README.md`; existing ADRs are not
      rewritten.
- [ ] "Implemented but not verified" is stated in the PR instead of ticking a roadmap item.

## 5. Specs (`docs/original/spec-*.md`, `docs/formats/`)
- [ ] Follows `docs/original/SPEC-TEMPLATE.md`: necessity record, claims with id/evidence/confidence, constants
      with source, acceptance tests, identity block with review and publication approvals.
- [ ] Expression filter of ADR-0009: behaviour in prose and mathematics, no copied value tables, no internal
      structure prescribed beyond interface needs; data-file mappings are allowed entry by entry with provenance.
- [ ] Handoffs and notes in the diff contain no private analysis from `re/`.

## 6. Repository hygiene and CI
- [ ] Skills are edited only in `.agents/skills/`; `.claude/skills/` is the generated mirror and must match
      (`python scripts/sync_skills.py --check`).
- [ ] CI is not weakened: no removed or skipped job, no `continue-on-error`, no lowered lint level, no `allow`
      added to hide a warning; new workflows use least-privilege `permissions` and do not upload data-derived
      artifacts or run data-backed tests on untrusted PRs.
- [ ] No absolute local user paths, secrets or machine settings committed.

## Reporting
Order findings: section 1, then determinism or data loss, then other bugs, then docs drift. Give file, line and the
consequence; say "not verified" for anything you could not confirm from the diff.
