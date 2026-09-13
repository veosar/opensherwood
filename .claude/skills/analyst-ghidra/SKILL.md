---
name: analyst-ghidra
description: How an analyst session turns the decompiled executable into a behaviour specification under docs/ without leaking decompiler output into the repository (ADR-0009). Use for any subsystem whose rules live in the executable (script VM and natives, navigation, AI, combat, campaign, HUD layout).
---

# Analyst with Ghidra (ADR-0009)

You are an **analyst**. Your deliverable is one specification file following `docs/original/SPEC-TEMPLATE.md`,
opened by its necessity record and confined to what the interoperability target requires. You do not write
engine code, patches or translated algorithms, not even as review comments; the session that implements the
subsystem must not be yours and must not inherit your context.

## Workspace

- `re/` is git-ignored: `re/ghidra/` holds the project (`robinhood`), `re/out/` the exports, `re/notes/` your
  working notes. Nothing under `re/` is ever committed (`scripts/check_no_assets.py` refuses it).
- Headless Ghidra: `C:\Users\przem\source\tools\ghidra_12.1.3_PUBLIC\support\analyzeHeadless.bat`, JDK 21 at
  `C:\Program Files\Eclipse Adoptium\jdk-21.0.12.101-hotspot`. The project already exists (imported and
  auto-analysed); do not re-import. Run scripts with `-process "Robin Hood.exe" -noanalysis -postScript <script>`.
- Export scripts live in `scripts/ghidra/` (generic, asset-free, committed): function inventory with sizes and
  call counts, string references, call graph neighbourhoods, decompilation of a list of addresses into `re/out/`.
- The original may be run only from `C:\Users\przem\source\gamedata\robinhood_oracle` (never the GOG install)
  with the tools in `harness/tools/original/` when a timing or presentation fact needs confirmation.

## Procedure

1. **Anchor**: find the subsystem from what is already known: the file formats it loads (`docs/formats/`), the
   strings the executable references (file names, format tags, error messages), the natives' dispatch table
   (the script VM), the harness measurements (`docs/original/`). Record the anchor addresses in `re/notes/`.
2. **Read what the target requires**: the functions that decide the behaviour the player's files depend on,
   following the data structures (your own layouts in the notes) until the required results and orderings are
   settled; record the scope and the stopping condition in the necessity record. Rename in Ghidra freely;
   those names stay in `re/`. The export scripts refuse any output directory outside `re/`; keep logs there too.
3. **Write the spec** in your own words: data model, behaviour with formulas and state tables, constants with
   their source addresses, native semantics by id, open questions, provenance. Test the spec against the data
   files and the harness where possible (a claim about a format field is checked on all 39 missions; a claim
   about timing against a recording).
4. **Self-check against the expression filter** (ADR-0009 section 5) before saving: no line reads like
   decompiler output (no `FUN_`, `DAT_`, `param_N`, `uVar`, `local_`, `undefined`), no identifiers, class
   layouts, function decomposition or strings taken from the binary, no value tables copied from its data
   segment (describe the rule that generates them, or name the data file and field they are loaded from), no
   game text; results and orderings, not organisation. Run `python scripts/check_no_assets.py --paths <spec>`.
5. **Hand over**: the lead requests a Codex spec review (skill `cross-agent-review`, task B); only a reviewed spec
   is implemented.

## What may be committed

Behaviour in prose and mathematics, state machines, layouts of file formats, constants as facts with addresses,
native semantics, the generic export scripts. Nothing else from the analysis.
