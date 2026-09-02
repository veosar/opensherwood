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
| 10-17 | Decode limits, SRES budgets, RPC drain, replay quotas, geometry bounds, full content digest, transactional restore, snapshot validation | **Open.** Next hardening batch; ordered as in the review. |

## P2-P4

Open. The roadmap lists them under the hardening and verification milestones; the oracle gate (P2 18-19) is the
first item after the hardening batch. The `codex_full_review.ps1` script no longer pushes on its own (P4 41):
it writes the report and leaves committing to a human or the lead agent.

## Format-status corrections requested by the review

Applied to `docs/formats/README.md`: RHM downgraded to partial (POUF raw, semantics incomplete), fonts split into
"pixels verified / layout partial", sprites labelled as a dated corpus observation for the all-frame claim,
video marked stub. Provenance blocks per ADR-0003 remain to be filled in (P2 25).
