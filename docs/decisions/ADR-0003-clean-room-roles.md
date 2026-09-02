# ADR-0003: Analyst / implementer separation

Date: 2026-09-02. Status: accepted. Supersedes the "private `re/` directory" idea from the first brief.

## Problem

A git-ignored directory with decompiler output does not make a clean room: the same agent or person could read
disassembly and then write the corresponding engine code. Codex's review called this the single biggest risk of
the project.

## Decision

Two roles with separate context:

**Analyst** may use the original binaries, Ghidra, debuggers, Frida and mutation experiments on a private copy.
The analyst produces only: behavioural specifications, format field specifications, small factual test vectors,
reproduction procedures and provenance records (`docs/formats/`, `docs/original/`, `docs/oracle/`). The analyst
never writes engine code for the subsystem being analysed.

**Implementer** may use committed specifications, synthetic fixtures, the public trace schema, normalised oracle
results and the original game as a black box (playing it, using its console, reading its data files). The
implementer never reads decompiler output, annotated disassembly or private address maps.

For AI agents this means: static analysis of the executable is done by a *separate agent session* (a subagent or
a separate Codex/Claude run) whose only deliverable is a spec document. The implementing session works from that
document. A session that has seen decompiler output does not implement that subsystem.

Pure data-file observation (hexdumps, statistics, decompression, rendering candidates) is not decompilation and
may be done by anyone; most format work is of this kind.

## Provenance of every committed claim

Each fact in a spec carries, at least implicitly through the file's Provenance section and explicitly when
disputed: status (`observed` / `inferred` / `unknown`), game build (executable SHA-256
`1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5` for the GOG build analysed), edition / language,
file and offset or chunk, observation method, reproduction steps, confidence, who and when, and which tests depend
on it. Unknown fields are named `unknown_0x24`, never guessed into code.

Never commit: pseudocode reconstructed from the executable, large hex dumps of game files, lookup tables copied
from assets, Ghidra databases, screenshots containing game art.

## Repository policy

- No game data in commits, releases, CI artifacts, issue attachments or test snapshots.
- Tests that need real game data run only locally or on a trusted self-hosted runner, never on untrusted PRs.
- Synthetic fixtures carry an origin manifest.
- Contributors attest that no leaked source or decompiler-derived implementation was submitted.
