# Disposition of Codex review 12 (written 2026-09-13)

Review: `2026-09-06-codex-review-12.md` (commits `bce71e5..73e91d4`). Verdict there: redesign, with the same
root cause as reviews 8 to 11: rules the executable decides were implemented as hypotheses and the taint model
could not keep up with them.

On 2026-09-13 the maintainer decided the strategic answer: **ADR-0009**. The executable is decompiled by
analysts behind a two-tier wall and every hypothesis subsystem is rebuilt from a reviewed behaviour
specification. The findings below are therefore not patched one by one on the hypothesis engine; each is
carried into the specification and rebuild of its subsystem, where it is closed by construction, and it stays
listed here until that rebuild lands.

| # | Finding | Disposition |
|---|---------|-------------|
| 1 | History gate; new asset-derived tables in tests | **Open, maintainer's decision (history).** The tables named are removed or replaced by run-time reads in the rebuild of each test file; the new gate (`check_no_assets.py`: Ghidra files, decompiler idioms) is in. |
| 2 | Budget exhaustion commits ordinary-callback effects; untainted win | **Carried into the script VM rebuild** (the first specification): the VM's real callback and budget semantics come from the executable, and the transaction model is redesigned around them. |
| 3 | Fresh stimuli of skipped entities lost | **Carried into the AI rebuild**: perception order and stimulus memory as the executable does them. |
| 4 | Fight teardown not atomic | **Carried into the combat rebuild.** |
| 5 | A pickup timer takes an item without an order | **Carried into the items rebuild** (validation ties the timer to the order in the meantime if the Rust engine is touched before then). |
| 6 | Alt cone overlay wrong and non-deterministic (floating point) | **Accepted; fixed in the HUD rebuild** with the executable's drawing rule; until then the overlay is presentation only and not part of any hash. |
| 7 | Pickup hit test not sprite-bound | **Carried into the items rebuild.** |
| 8 | Test depth | **Carried**: every rebuilt subsystem lands with recorded ReplayV1 regressions per the roadmap. |
| 9 | Documentation drift | **Done with ADR-0009**: status, roadmap and README are rewritten for the new plan; per-subsystem docs are rewritten as their specs land. |
