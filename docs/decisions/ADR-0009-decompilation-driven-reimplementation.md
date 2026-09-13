# ADR-0009: Decompilation-driven reimplementation behind a two-tier wall

Date: 2026-09-13. Status: accepted (the maintainer's decision; Codex review requested on this text). Supersedes
the "data observation only" practice that grew under ADR-0003 and amends ADR-0001 (the language stays Rust).

## Problem

Eleven adversarial reviews returned the same root cause: the engine is built from hypotheses. Data observation
and oracle recordings established file formats and screen layouts, but the behaviour that lives only in the
executable (the script VM's opcodes and 168 natives, of which about 95 are recorded stubs; the path graph; layers,
doors and lifts; perception, alarms and combat rules; campaign flow; the camp) was guessed, tested for internal
consistency and marked as an assumption. A playable-to-the-end, faithful engine cannot be reached that way in
any reasonable time, and every review said so.

## Decision

1. **The executable is analysed with a decompiler (Ghidra).** Analysts read the decompiled code until they
   understand each subsystem completely and write what it does into behaviour specifications under `docs/`.
   The legal basis is the interoperability exception (`docs/legal.md`): the aim is a program that interoperates
   with the player's own data files.
2. **Two-tier wall, enforced by session separation.**
   - *Analysts* work in the git-ignored `re/` workspace (Ghidra project, exported decompilation, notes). Their
     only deliverable is a specification: behaviour in prose and mathematics, state machines, tables of format
     layouts, constants as facts, the semantics of each native by id, the algorithms in the analyst's own words.
     Provenance names the function addresses and the build hash. **Never committed:** decompiler output or its
     paraphrase line by line, pseudocode transcribed from a function, the binary's identifiers, its string table,
     tables of values copied from its memory or data segments, game text.
   - *Implementers* write original Rust from the specifications. They never open `re/`. A session that has read
     decompilation for a subsystem does not implement that subsystem.
   - *Codex* reviews both sides: a spec against `re/` (allowed for the reviewer: is every claim backed by an
     address, is anything transcribed, what is missing) and code against the spec.
3. **Language: Rust stays** (ADR-0001). The original is MSVC 6 C++; matching its language buys nothing: fidelity
   comes from the specifications, and the existing harness, formats, determinism model and CI are kept.
4. **What changes in the code base.** The harness (JSON-RPC, replays, snapshots, hashes, the pytest suites),
   the decoded formats, the UI screens matched to captures and the CI are kept. Every subsystem that rests on a
   hypothesis is rebuilt from its verified specification, in the order of the roadmap (ADR-0007 amended):
   script VM and natives, navigation (path graph, layers, doors, lifts), AI and combat, movement and camera,
   campaign and camp, HUD and menus, saves. The `Assumption` registry (ADR-0008) becomes the list of what is
   still unverified; a verified rule removes its variant. The end state is the full campaign playable through
   canonical input, with a recorded replay per mission, and no assumption left in a winning path.
5. **Gate.** `scripts/check_no_assets.py` refuses Ghidra project files and text that looks like decompiler
   output; Codex reviews every specification before implementation starts on it; the specification template
   (`docs/original/SPEC-TEMPLATE.md`) is mandatory.

## Consequences

- Faster, exact progress on everything the binary decides; the oracle recordings remain for timing and
  presentation checks.
- Higher care in reviews: the risk moved from "wrong" to "copied"; the wall and the gate are the mitigation.
- The reviews 8 to 12 findings that concern hypothesis handling are closed by construction once each
  subsystem is rebuilt from its spec; they stay listed until then.
