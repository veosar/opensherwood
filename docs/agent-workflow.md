# Claude and Codex workflow

## One source for instructions and skills

`AGENTS.md` is the shared project policy. Claude imports it through the small `CLAUDE.md` adapter;
Codex reads it directly. Procedures live in `.agents/skills/<name>/SKILL.md`. Edit that copy, then run:

```text
python scripts/sync_skills.py --write
python scripts/sync_skills.py --check
python -m unittest discover -s scripts/tests
```

The checked-in `.claude/skills/` mirror includes supporting files. CI rejects drift. Use this explicit
mirror instead of machine-specific junctions or symlinks so a clone works on Windows, Linux and macOS.
Project skills do not duplicate personal commit, PR or maintenance skills. Only the relevant procedure
and spec are loaded into a session; large files should be searched and read in excerpts.

## State without a tracker

| Information                                           | Home                                                                                   |
| ----------------------------------------------------- | -------------------------------------------------------------------------------------- |
| Rules needed every session                            | `AGENTS.md`; client-specific adapter in `CLAUDE.md`                                    |
| Repeated procedure                                    | canonical `.agents/skills/`                                                            |
| Reviewed facts, formats and architecture              | appropriate `docs/` document or ADR                                                    |
| Milestones                                            | `docs/roadmap.md`                                                                      |
| Current task, results, exact last error and next step | local handoff file per task, outside the repository (location depends on your tooling) |
| Automatic check cache                                 | local `.local/agent-hooks/` per checkout/session                                       |

All handoffs, including analyst handoffs, contain only safe progress and spec links. Private
analysis stays in `re/`. Switching clients or compacting does not erase a session's exposure to
decompilation or permit it to implement that subsystem (ADR-0003/ADR-0009).

## Claude Code project hook

Python 3.12+ and Git must be on PATH. `.claude/settings.json` invokes `scripts/agent_hooks.py`.
The launcher resolves the checkout with Git, including when the client starts in a subdirectory or a
linked worktree. No absolute user paths, models, API keys or permission bypasses are stored in it.

- **SessionStart** (startup/resume/clear/compact): records the initial Rust fingerprint, retaining it on
  resume/compaction of the same session, and reports skill-mirror drift. It emits nothing else.
- **Stop**: checks the skill mirror; after Rust/build inputs change, runs
  `cargo check --workspace --all-targets --locked`. Inputs include tracked and untracked Rust files,
  crate manifests, lockfile, toolchain and local Cargo config. Hashes detect edits and deletions.
  A per-session cache skips unchanged inputs. A failure blocks once; a repeated unchanged failure
  emits a warning. `stop_hook_active` prevents continuation loops. No formatter or handoff writer runs.
- The compile check has a 45-second limit (60 seconds for the hook). Cold builds or Cargo lock waits
  can exceed it: run the same command manually and report the result. A check cannot certify inputs
  changed while it ran. This is a quick compile guard, not runtime verification or the full dev loop.

Codex has no project hook: run the `dev-loop` checks yourself. Use a new Claude session after changing
hook settings. You can test the script directly by sending a JSON object containing `session_id`,
`hook_event_name` (`SessionStart` or `Stop`), `cwd`, and optionally `stop_hook_active` on stdin.
The Python tests exercise the configured command and a failing Rust fixture without game data.

Hook API reference, checked when implementing the adapter: [Claude hooks](https://code.claude.com/docs/en/hooks).
