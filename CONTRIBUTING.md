# Contributing

Thank you for helping. Three rules come before everything else:

1. **Never add game assets** (not even a cropped screenshot with game art in `docs/`). Read [docs/legal.md](docs/legal.md).
2. **Clean room.** Do not paste decompiler output. Write a spec in `docs/formats/` or a behaviour note in
   `docs/original/`, then implement from the spec.
3. **Every change is verified by the harness** (`cargo test` plus `python -m pytest harness/`), and the docs that
   describe the changed behaviour are updated in the same commit.

## Workflow

- Branch from `main`, open a pull request. CI must be green.
- Small, reviewable commits. Commit messages: imperative subject line, a body that says *why*.
- Keep `docs/` truthful: if you learn something about a file format, update the spec and its `Provenance` section;
  if you change a command-line flag or RPC method, update `docs/harness.md` / `docs/build.md`.
- Rust: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`. Python: `ruff`.
- AI agents follow [AGENTS.md](AGENTS.md) and the skills in `.agents/skills/`; humans can use the same procedures.

## Reporting format discoveries

Open an issue with: the file, the offset, what you believe the bytes mean, and how you verified it (which
original behaviour or console display confirms it).
