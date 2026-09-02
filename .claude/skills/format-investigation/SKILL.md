---
name: format-investigation
description: How to reverse engineer a game data file format legally (observation-based) and turn it into a spec in docs/formats plus a parser in locksley-formats. Use when a format is stub/partial, when a parser fails on a real file, or when a new file type is found.
---

# Format investigation (clean-room, observation-based)

Applies to data files only. Analysing the executable is a different role: see ADR-0003 and the `oracle-capture`
skill. If you find yourself needing disassembly to understand a data file, stop and hand the question to an
analyst session; write down the exact open question in the spec.

## Procedure

1. **Inventory**: list all files of the type, sizes, and where the executable's strings reference them
   (`docs/original/executable-notes.md`). Note related files (same base name, other extension).
2. **Header**: hexdump the first 256 bytes of several files. Look for magics, `u32` sizes equal to file size minus
   header, versions, counts, dimensions. Compare small vs large files of the same type.
3. **Containers first**: if you see 4-char tags + `u32` size, write a chunk walker and confirm it consumes the whole
   file with no leftover bytes. Record tag order, versions and sizes per file in the spec table.
4. **Compression**: look for `78 DA`/`78 9C` (zlib), `BZh` (bzip2), and check decompressed sizes against
   width x height x bytes-per-pixel guesses.
5. **Hypothesis testing**: write a throwaway Python script (keep it in `harness/tools/re/` if reusable, no game bytes
   inside) that parses *every* file of the type under the hypothesis and prints: files fully consumed, field ranges,
   histograms. A hypothesis is accepted only if it consumes all files exactly and the field ranges make sense.
6. **Cross-reference**: ids found in one format must resolve in another (e.g. RHS frame ids into the DIC table,
   `.red` ids into `Level.res`). Do the join and report the hit rate.
7. **Render or listen**: for pixel or audio data, produce a candidate image/sound locally and look at it (Read the PNG).
   Wrong pixel formats look wrong; do not commit the image.
8. **Spec**: update `docs/formats/<name>.md`: status, layout tables with offsets/types, every unknown as
   `unknown_<offset>`, examples of values (counts, dimensions), and the Provenance section describing exactly how
   each claim was established. Do not paste game strings beyond a few short identifiers needed to explain a field.
9. **Parser**: implement in `crates/locksley-formats/src/<name>.rs` with a bounded reader; return typed structures;
   keep `unknown_*` fields; add a data-backed test (skipped without `LOCKSLEY_GAME_DIR`) that parses every file
   of the type and asserts the invariants you found; add a fuzz-safe test with truncated/garbage input.
10. **Tools**: expose it in `locksley-tools inspect` so the next investigator can see it.

## Statistics helpers

Entropy per block, `u16` histograms, monotonic `u32` run detection, alignment search, cross-file diffing of
same-name Day/Night/Fog variants. Keep scripts generic; they must not embed game data.
