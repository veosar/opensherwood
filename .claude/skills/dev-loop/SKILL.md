---
name: dev-loop
description: The mandatory build -> run -> verify -> document loop for any code change in OpenSherwood. Use before claiming any task is done, after every non-trivial edit, and when setting up a fresh machine.
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
python scripts/check_no_assets.py
```

If the change touches parsers, rendering, simulation or assets and `OPENSHERWOOD_GAME_DIR` is set:

```
python -m pytest harness/tests/data -q -k <area>
```

For rendering changes also run `capture` on the affected scene and look at the PNG yourself (Read the image),
then compare with the golden (`harness/tools/compare.py`).

## 4. Report

- Say exactly what ran and what passed or failed, with the failing output. Never summarise a failure as "minor".
- If something could not be verified (no data, no display), say so and leave the roadmap checkbox unticked.

## 5. Commit and push

- Imperative subject, body with the why and the verification performed.
- Push to `main` when green; otherwise a branch.

## Fresh machine checklist

`rustup` (reads `rust-toolchain.toml`), a C compiler, Python 3.12 + `pip install -r harness/requirements.txt`,
`cargo install cargo-nextest`. Windows: MSVC Build Tools with the C++ workload. See `docs/build.md`.
