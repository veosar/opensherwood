# <Subsystem> (behaviour specification)

Status: `draft` | `reviewed` (Codex review N, spec revision <git blob or commit>) | `implemented` (ruleset N).
Build: GOG English edition, `Robin Hood.exe` SHA-256 `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`,
image base 0x00400000; every address below is a virtual address in that image. Analyst: <date, session id>.
Reviewer: <session id>. Publication approval: <who, when> (separate from factual approval).

This file describes what the original program does, in the analyst's own words, so that an implementer who has
never seen the program can build it. It contains no decompiler output, no transcribed pseudocode, none of the
binary's identifiers or strings, no tables copied from its data, no game text, and no prescribed internal
structure (ADR-0009, "expression filter"). It describes required results and orderings; the implementer chooses
the organisation.

## 0. Necessity record

- Interoperability target: <which of the player's files / scripts / saves this subsystem must interoperate with>.
- Information not otherwise available: <what the data files, the manual and black-box observation could not
  settle, with pointers to the earlier attempts>.
- Scope read: <function address ranges / count read>, and the stopping condition that was reached.
- Analyst authorisation: on behalf of the maintainer, on the maintainer's lawfully acquired copy.

## 1. Scope

What the subsystem covers, what it hands to and takes from the others (inputs, outputs, shared state).

## 2. Data model

The state the subsystem keeps (fields, types, widths, signedness, ranges, units, initial values, reset
behaviour, what a save / snapshot must carry), with the file fields it is loaded from (link the format spec).
Units are explicit (map pixels, which clock, fixed-point scale, coordinate conventions).

## 3. Behaviour

Per operation: preconditions, the algorithm in prose and mathematics (formulas with every constant), the
evaluation order and tick phase when it matters for determinism (simultaneous events, RNG consumption order),
rounding and overflow rules, edge cases and error paths (invalid ids / handles, failure behaviour). State
machines as tables (state, event, guard, action, next state). Timing in the program's own clock, converted to
ours.

## 4. Claims

| Id | Claim (one sentence) | Status | Evidence (address / experiment) | Confidence | Contradictions / notes |

Every statement of sections 2, 3 and 5 that an implementer relies on has a row here (`<PREFIX>-NNN`). Status is
`observed` (read in the program), `inferred` (from usage or data), `unknown`. Evidence names the function
address or the reproducible experiment (data probe, oracle recording). A claim that stays unknown maps to an
`Assumption` variant (ADR-0008) in the implementation.

## 5. Constants

| Name (ours) | Value | Unit | Source (address / data file field) | Confidence |

Constants are individual functional facts. A curated table of values is not reproduced: the rule that generates
it is described, or the data file and field it is loaded from is named.

## 6. Interfaces to the script VM

Natives by id: arity, argument coercion, meaning, return value, side effects, blocking / yielding, failure
behaviour, callback order.

## 7. Acceptance tests

Synthetic inputs with expected outputs and state changes; boundary and negative cases; oracle procedures with
tolerances (which recording, which measurement). These become the implementer's tests.

## 8. Implementation choices

What is the original's behaviour and what is an OpenSherwood decision or a deliberate deviation (with the reason).

## 9. Open questions

What the analyst could not settle, with the addresses to look at next.

## 10. Provenance

Ghidra project `re/ghidra/robinhood` (never committed), the export scripts used (`scripts/ghidra/`), the files
read, oracle recordings that confirmed timing (file names in the analyst workspace), tests that depend on this
spec. Instruction bytes, disassembly, recovered symbols and comprehensive address maps stay in `re/`.
