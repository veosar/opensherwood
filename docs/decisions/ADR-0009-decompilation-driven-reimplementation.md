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

1. **The executable is analysed with a decompiler (Ghidra), as far as interoperability requires.** Analysts
   read the decompiled code of a subsystem to the extent needed to make the player's own files (maps, mission
   scripts, profiles, saves) behave as the game expects, and write what it does into behaviour specifications
   under `docs/`. Every specification opens with a **necessity record**: the interoperability target, the
   information that data observation and black-box testing could not settle, the scope read and the stopping
   condition. The legal basis and its limits are in `docs/legal.md` (Directive 2009/24/EC art. 5-6, the Polish
   act art. 75-76); the maintainer keeps the acquisition record and the store terms privately; the workflow is
   to be assessed by counsel before a release.
2. **Two-tier wall, enforced by session separation.**
   - *Analysts* work in the git-ignored `re/` workspace (Ghidra project, exported decompilation, notes). Their
     only deliverable is a specification: behaviour in prose and mathematics, state machines, tables of format
     layouts, constants as facts, the semantics of each native by id, the algorithms in the analyst's own words.
     Provenance names the function addresses and the build hash. **Never committed:** decompiler output or its
     paraphrase line by line, pseudocode transcribed from a function, the binary's identifiers, its string table,
     tables of values copied from its memory or data segments, game text.
   - *Implementers* write original Rust from the specifications. They never open `re/`. A session that has read
     decompilation for a subsystem does not implement that subsystem.
   - *Reviewers* are two distinct roles. A *spec reviewer* may read `re/` and checks a specification (evidence,
     transcription, completeness, the expression filter); its output is corrections to the specification only,
     never code, patches or translated algorithms. An *implementation reviewer* checks code against the cleared
     specification and never reads `re/`. One session does not hold both roles for the same subsystem.
   - *Handoffs.* Implementer sessions are started fresh (no inherited analyst context, tool output, notes or
     memory carrying decompilation), receive only cleared specifications, and route every question back through
     a specification amendment. Exposure is tracked per subsystem in the spec's identity block; `.gitignore` is
     not access control, so implementer briefs forbid `re/` explicitly and exposure through helper routines
     counts.
3. **Language: Rust stays** (ADR-0001). The original is MSVC 6 C++; matching its language buys nothing: fidelity
   comes from the specifications, and the existing harness, formats, determinism model and CI are kept.
4. **What changes in the code base.** The harness (JSON-RPC, replays, snapshots, hashes, the pytest suites),
   the decoded formats, the UI screens matched to captures and the CI are kept. Every subsystem that rests on a
   hypothesis is rebuilt from its verified specification, in the order of the roadmap (ADR-0007 amended):
   script VM and natives, navigation (path graph, layers, doors, lifts), AI and combat, movement and camera,
   campaign and camp, HUD and menus, saves. The `Assumption` registry (ADR-0008) becomes the list of what is
   still unverified; a verified rule removes its variant. The end state is the full campaign playable through
   canonical input, with a recorded replay per mission, and no assumption left in a winning path.
5. **Expression filter** (conservative project policy, not a claim about what is copyrightable):

   | Material | Boundary |
   |---|---|
   | Line-by-line paraphrase | Refused, including one instruction or basic block rewritten as one sentence. |
   | Internal structure | Recovered class layouts, function decomposition, call graphs and branch structure are not prescribed unless a functional or interface requirement demands it. |
   | Identifiers | Ours are chosen independently; a compatibility token whose exact spelling a file format requires is allowed individually and marked. |
   | Constants | Individual functional facts with provenance are allowed; reconstructing a copied table as many "constants" is not. |
   | Constant and string tables | Copied content, its ordering and encoded substitutes are refused; a formula that merely re-encodes a curated table does not pass. Tables loaded from data files are named by file and field. |
   | Algorithms | Required results, state transitions and observable orderings are described; the implementer chooses the organisation. |

   There is no safe numerical granularity; the reviewer judges each specification against this table.

   *Clarification (2026-09-13, after the campaign review):* a mapping from a data-file field, flag or id to its
   meaning (which flag byte selects which capability, which workshop kind produces which item kind, which
   native id has which semantics) is an **interface fact** the player's files require and is allowed, entry
   by entry, with its data-file provenance, exactly like the native table. What stays refused is a table of
   tuned values that the program carries as content (balance numbers, colour lists, curves) unless the rule
   that generates it is described or the data file it is loaded from is named.
6. **Gate.** `scripts/check_no_assets.py` inspects the index (what a commit records), the outgoing commits of a
   push (`scripts/hooks/pre-push`, installed by `git config core.hooksPath scripts/hooks`) and named drafts
   (`--paths`); it refuses Ghidra project files and directories, exporter signatures and decompiler idioms in
   UTF-8 or UTF-16 text, and reports what it cannot inspect. It is a heuristic; the spec reviewer is the rule.
   The export scripts write only under the analysis root (`re/`) and refuse other destinations. Every
   specification follows `docs/original/SPEC-TEMPLATE.md` (claims with ids, evidence and confidence; execution
   semantics; acceptance tests; the identity block with the review and publication approvals) and is reviewed
   before implementation starts on it.

## Consequences

- Faster, exact progress on everything the binary decides; the oracle recordings remain for timing and
  presentation checks.
- Higher care in reviews: the risk moved from "wrong" to "copied"; the wall and the gate are the mitigation.
- The reviews 8 to 12 findings that concern hypothesis handling are closed by construction once each
  subsystem is rebuilt from its spec; they stay listed until then.
