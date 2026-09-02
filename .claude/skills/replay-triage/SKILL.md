---
name: replay-triage
description: Diagnose a determinism failure - a replay whose state hash differs between runs, platforms, or after snapshot/restore. Use whenever a harness test reports a hash mismatch or a desync.
---

# Replay / determinism triage

1. **Reproduce twice** on the same machine with the same build: `python -m pytest harness/tests/synthetic -k <test> -p no:randomly`.
   If it is flaky on one machine, the cause is unordered iteration, uninitialised state, threads or wall clock.
2. **Find the first divergent tick**: run the replay with `--hash-every-tick` on both sides and diff the per-tick
   subsystem hashes (`harness/tools/hashdiff.py a.jsonl b.jsonl`). The first tick and the first subsystem that
   differ are the bug's address.
3. **Narrow the subsystem**: `observe` at tick T-1 and T on both sides with the subsystem filter, diff the JSON.
4. **Snapshot/restore failures**: if the plain replay is stable but restore diverges, some authoritative field is
   missing from the snapshot or a cache is not rebuilt. Compare `observe` after restore with `observe` before.
5. **Cross-platform failures**: suspect float formatting, `f32` vs `f64` promotion, `sort` stability, `HashMap`,
   path separators, line endings in scenario files, and platform-dependent RNG seeding.
6. **Fix at the source**, add a regression test with the minimal replay, and note the class of bug in
   `docs/architecture.md` (Determinism contract) if it is new.
7. Never "fix" by regenerating the expected hash unless the change is an intended ruleset change, in which case
   bump the ruleset version in `opensherwood-protocol` and say so in the commit.
