# Disposition of the Codex full review (2026-09-02)

Review: `2026-09-02-codex-full-review.md` (baseline `277af51` plus the then-uncommitted menu work). This file
records what was done about each item, by the implementer, on the same day. Items not listed under "done" are
open and tracked in `docs/roadmap.md`.

## P0 (legal containment)

| # | Finding | Disposition |
|---|---|---|
| 1 | Copied briefing/debriefing prose in `docs/original/campaign-flow.md` | **Done.** Replaced with text ids, string indices and one-line paraphrases. |
| 2 | Copied UI strings in `docs/original/ui-flow.md` | **Done.** Menu labels, dialog wording, the profile summary block and options wording replaced with lowercase functional descriptions and text ids. Row geometry kept. |
| 3 | Hardcoded retail labels in `ui.rs` | **Done.** Labels come from `Level.res` TEXT 1000507 at run time (indices documented in `ui_assets.rs`); without game data the menu shows neutral action identifiers. |
| 4 | History rewrite of `277af51` | **Open, maintainer's decision.** The text is removed from `main`; the pushed history still contains it. Rewriting public history is destructive and was not done without the maintainer's approval. |
| 5 | `.gitignore` / policy checker coverage | **Done.** Derived image extensions, `harness/goldens/`, `harness/captures/`, root `re/` are now forbidden by both. |
| 6 | Checker success wording | **Done.** It states what it checks and that copied text is not detected. |

## P1 (build and security integrity)

| # | Finding | Disposition |
|---|---|---|
| 7 | Menu integration incomplete | **Done.** `Scenario::Menu` is parsed and reset; `observe`/`reset` return `ui`; `step`/`capture` work without a world while a menu is open; end-to-end menu tests in `harness/tests/data/test_menu.py`. |
| 8 | Version bumps | **Done.** Ruleset 3 (viewport 1024x768 for retail scenarios, camera on the hero), snapshot 4 (new scenario variant), protocol 3 (`ui` field, `menu` scenario). Fixture regenerated. |
| 9 | Green from a fresh build | **Done** locally (fmt, clippy `-D warnings`, Rust tests, 28 harness tests with game data). CI runs on push. |
| 10 | Shared sprite decode limits | **Done.** `sprite_decode::DecodeLimits` (4096 per side, 32 MiB decoded, 64 MiB stream; checked arithmetic, `try_reserve`) is applied by every public decode function, by `SpriteBank` and by `export-frame`. |
| 11 | SRES cumulative budgets | **Done.** `sres::Limits` (65,536 entries, 4,096 pictures per entry, 16,384 pictures and 256 MiB decoded per archive, charged from the picture header before decompression); retail archives still parse. |
| 12 | Oversized RPC line drain | **Done.** The remainder of an oversized line is skipped through the `BufRead` buffer (`discard_rest_of_line`), nothing is allocated for it. |
| 13 | Replay resource limits | **Done.** `replay.play` checks the file size against the 64 MiB cap before reading and refuses replays past 1,000,000 ticks before resetting; recording enforces the format quotas (2^20 events, 2^16 checkpoints, tick 2^24) cumulatively, refusing a `step` up front and discarding a window-mode recording that crosses them. |
| 14 | Geometry bounds / overflow | **Done.** Vertices are bounded to `+-2^20` map pixels (`geom::MAX_COORD`) in `World::validate` and `set_geometry` (now fallible); `point_in_polygon` and the nav scan conversion use `i128`, `line_clear`, the A* heuristic and cell arithmetic `i64` / saturating, so any `i32` input is total and identical in debug and release. Extreme-coordinate tests through JSON restore, run in both build modes. |
| 16 | Transactional restore | **Done.** `restore` checks the envelope, builds a cross-scenario world in a temporary (`load_scenario`) and touches the session only in `install`; a failed restore or reset leaves world, background, screen, snapshot handles and queued input untouched (session test `restore_is_transactional`). |
| 17 | Snapshot validation | **Done.** `Snapshot::check_versions` (snapshot, ruleset and hash schema), `content` fingerprint in the envelope (snapshot 6) compared by `Snapshot::check_content` against the session's `GameDir::fingerprint` (`null` for synthetic), animation state validated against the attached catalog (profile, animation, frame, elapsed; rejected, never fallen back). Data test `test_snapshot_envelope_checks_content_identity_and_catalog`. |
| 15 | Full content digest | **Done.** `GameDir::fingerprint` streams every indexed file through BLAKE3 (v3 tag), caches per-file digests by path, size and mtime, and returns an error when any file cannot be read; about 0.8 s for the 1 GiB retail install when cached by the OS. |

## P2-P4

Mostly open. P2 19 (local-only image comparison) is started: `opensherwood_harness.compare` and the menu oracle
test (`docs/harness.md`, "Oracle comparison"); the trace schema (P2 18) is not. The roadmap lists the rest under
the hardening and verification milestones. The `codex_full_review.ps1` script no longer pushes on its own (P4 41):
it writes the report and leaves committing to a human or the lead agent.

## Format-status corrections requested by the review

Applied to `docs/formats/README.md`: RHM downgraded to partial (POUF raw, semantics incomplete), fonts split into
"pixels verified / layout partial", sprites labelled as a dated corpus observation for the all-frame claim,
video marked stub. Provenance blocks per ADR-0003 remain to be filled in (P2 25).
