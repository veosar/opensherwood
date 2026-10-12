---
applyTo: "crates/opensherwood-core/**/*.rs,crates/opensherwood-script/**/*.rs"
---
# Simulation core and script translator

Source of truth: `docs/architecture.md` (determinism contract), ADR-0004, ADR-0008, ADR-0010.

- The simulation advances in fixed logic frames. Flag any use of wall clock, `Instant`/`SystemTime`, threads,
  async, environment or file I/O, rendering or platform types in `opensherwood-core`.
- Flag `HashMap`/`HashSet` whose iteration order can reach state, hashes, snapshots or event order; the contract
  requires `BTreeMap`, sorted vectors or arena order.
- Randomness only from named seeded RNG streams owned by core; the order in which a stream is consumed is part of
  the behaviour, so flag reordered or conditional draws.
- Positions and timers are fixed-point; flag `f32`/`f64` in state or in anything that is hashed, unless the float
  operations are isolated and documented.
- New authoritative field: it must be in `snapshot()`, rebuilt or validated in `restore()`/`validate`, and in the
  canonical hash. Caches must be rebuilt on restore, not snapshotted as truth.
- A change that alters simulation outcomes needs a `RULESET_VERSION` bump (and `HASH_SCHEMA_VERSION`/
  `SNAPSHOT_VERSION` when encodings change) plus the updated golden digest `harness/fixtures/synthetic_corridor.json`.
- Player actions in tests are canonical input events, never privileged commands (`debug.*` is inspection only).
- Behaviour must trace to a spec claim in `docs/original/` or `docs/formats/`. A rule the spec has not settled is an
  `Assumption` variant (ADR-0008), visible and counted, never a silent guess. Unknown natives trap deterministically.
- `opensherwood-script` only translates SCB to the core IR: no execution logic and no state.
