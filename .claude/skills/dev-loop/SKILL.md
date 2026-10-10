---
name: dev-loop
description: The mandatory build, run, verify and document loop for any code change in OpenSherwood. Use before claiming any task is done, after every non-trivial edit, and when setting up a fresh machine.
---

# Development loop

## 1. Before editing

- Read the spec or doc for the area (`docs/architecture.md`, `docs/formats/<format>.md`, `docs/harness.md`).
- State the verification you will run at the end (which tests, which replay, which screenshot comparison).

## 2. Edit

- Keep the change small enough to verify in one loop.
- Update the doc in the same change (format spec, protocol doc, build doc, roadmap checkbox).

## 3. Verify (all of it, in order)

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace          # cargo test --workspace if nextest is missing
python -m pytest harness/tests/synthetic -q
python scripts/sync_skills.py --check
python -m unittest discover -s scripts/tests
python scripts/check_no_assets.py
```

If the change touches parsers, rendering, simulation or assets and `OPENSHERWOOD_GAME_DIR` is set:

```
python -m pytest harness/tests/data -q -k <area>
```

For rendering changes also run `capture` on the affected scene and look at the PNG yourself (Read the image),
then compare with the golden (`harness/tools/compare.py`).

For agent-tooling changes, also exercise the configured hook commands with JSON on stdin. SessionStart
only records the Rust baseline; Stop must report a failing compile check without looping.
Its quick `cargo check` does not replace the checks above. With unrelated dirty Rust files, use the
non-mutating `cargo fmt --all -- --check` and report existing failures without formatting another task's work.

## 4. Report

- Say exactly what ran and what passed or failed, with the failing output. Never summarise a failure as "minor".
- If something could not be verified (no data, no display), say so and leave the roadmap checkbox unticked.

## 5. Commit and push

- Imperative subject, body with the why and the verification performed.
- Push to `main` when green; otherwise a branch.

## Parallel sessions

- Files written from Python on Windows use `newline="\n"` (otherwise git warns about CRLF), as
  `harness/tools/golden_digest.py --write` does.
- While other agents edit the core crates the tree may not build. Meanwhile do work that needs no build (docs,
  tests); integrate, run `cargo fmt`, clippy and the tests after the agent reports.
- To verify your own change while someone else's edits leave the tree unbuildable: `git worktree add
  <scratch>/old HEAD`, copy only your files into it and build there with `CARGO_TARGET_DIR` inside the worktree.
  The harness takes any binary through `OPENSHERWOOD_BIN=<exe>`, so a pre-change binary built the same way gives
  an old/new comparison over RPC without touching the main `target/`. Remove the worktree with
  `cmd /c rmdir /s /q <scratch>\old` (`git worktree remove` fails on long `target` paths), then `git worktree prune`.
- A release binary rebuilt while an agent has uncommitted edits contains those edits: verify the lead's changes
  in a worktree.

## Fresh machine checklist

`rustup` (reads `rust-toolchain.toml`), a C compiler, Python 3.12 + `pip install -r harness/requirements.txt`,
`cargo install cargo-nextest`. Windows: MSVC Build Tools with the C++ workload. See `docs/build.md`.
