---
name: docs-sync
description: Checklist of which documents must change with which kind of code change, so README, build, harness, formats, roadmap and ADRs never drift from the code. Use at the end of every task before committing.
---

# Documentation sync checklist

| If you changed... | Update |
|---|---|
| a CLI flag, env var, binary name | `docs/build.md`, `README.md` (short version), `AGENTS.md` approved commands |
| an RPC method, event kind, replay field, hash component | `docs/harness.md`, ADR-0004 (if the contract changes), `harness/locksley_harness/rpc.py` docstrings |
| a parser or a format finding | `docs/formats/<format>.md` (status, tables, Provenance), `docs/formats/README.md` status column |
| a fact about the original game's behaviour | `docs/original/*.md` |
| crate layout or dependency direction | `docs/architecture.md`, `AGENTS.md` layout section |
| a milestone item verified | `docs/roadmap.md` checkbox |
| an architectural decision | new `docs/decisions/ADR-NNNN-*.md` + index |
| a skill | `.agents/skills/<name>/SKILL.md`, then `python scripts/sync_skills.py --write` |
| a dependency with a new license | `docs/licenses.md` (third-party table) |

Before committing: `git diff --stat` and ask "would a reader of the docs be misled by this change?" If yes, fix it
in the same commit.
