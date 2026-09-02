@AGENTS.md

# Claude Code adapter

- `.claude/skills/` is generated from `.agents/skills/` by `python scripts/sync_skills.py --write`. Edit the
  canonical copy only.
- Codex is available on the maintainer's machine as `codex exec`; see the `cross-agent-review` skill for how to
  request an adversarial review.
- Game data for local tests lives outside the repository; ask for `OPENSHERWOOD_GAME_DIR` if it is not set.
