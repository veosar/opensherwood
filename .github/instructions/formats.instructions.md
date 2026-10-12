---
applyTo: "crates/opensherwood-formats/**/*.rs,crates/opensherwood-assets/**/*.rs"
---
# File format readers and asset discovery

Source of truth: `docs/formats/<format>.md` (each has a Provenance section) and `docs/legal.md`.

- Readers are pure functions from bytes to typed structures, with no game logic. Flag panics, `unwrap`/`expect`,
  unchecked indexing, unchecked arithmetic or unbounded allocation driven by file contents: input is untrusted.
- Errors carry the offset. Every field whose meaning is not established is named `unknown_*` (for example
  `unknown_0x24`) and is not interpreted.
- A parser change must come with the matching update of `docs/formats/<format>.md` and its status in
  `docs/formats/README.md`.
- Tests use synthetic fixtures with an origin manifest. Flag any fixture, hex dump, table or string copied from the
  player's game files; only hashes, metrics and synthetic data may be committed.
- `opensherwood-assets`: lookups are case-insensitive on every platform; the content fingerprint must never be a
  partial hash (a file that cannot be read is an error); the engine never copies or redistributes game data.
