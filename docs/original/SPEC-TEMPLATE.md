# <Subsystem> (behaviour specification)

Status: `draft` | `reviewed` (Codex review N) | `implemented` (ruleset N). Build: GOG, executable SHA-256
`1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`. Analyst session: <date, agent role>.

This file describes what the original program does, in the analyst's own words, so that an implementer who has
never seen the program can build it. It contains no decompiler output, no transcribed pseudocode, no identifiers
or strings of the binary, no tables copied from its data (ADR-0009).

## 1. Scope

What the subsystem covers, what it hands to and takes from the others (inputs, outputs, shared state).

## 2. Data model

The state the subsystem keeps (fields, types, ranges, units, initial values), with the file fields it is loaded
from (link the format spec). Units are explicit (map pixels, ticks of which clock, fixed-point scale).

## 3. Behaviour

Per operation: preconditions, the algorithm in prose and mathematics (formulas with every constant), the order
of evaluation when it matters for determinism, edge cases and error paths. State machines as tables
(state, event, guard, action, next state). Timing in the program's own clock, converted to ours.

## 4. Constants

| Name (ours) | Value | Unit | Where it comes from (function address / data offset) | Confidence |

## 5. Interfaces to the script VM

Natives by id: arity, argument meaning, return value, side effects, when they read or write the state above.

## 6. Open questions

What the analyst could not settle, with the addresses to look at next.

## 7. Provenance

Ghidra project `re/ghidra/robinhood` (never committed), functions by address, the export scripts used
(`scripts/ghidra/`), oracle recordings that confirmed timing (file names in the analyst workspace), tests that
depend on this spec.
