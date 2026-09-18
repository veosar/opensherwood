# Codex spec re-review 39: docs/original/spec-script-vm.md (revision 9), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`. Verdict: cleared for implementation (the interpreter core, the scheduler-independent natives, the messages and the snapshot / fault contract, for scheduler-independent use).

Reviewed revision 9, commit `a014e38cec7dc731309e6bea0503fc130a85f105`, against review 36, the spec’s rules and the relevant decompilation.

No remaining findings. The three separate fixtures resolve the recording conflict and cover preservation of opener provenance, pending-fault attribution, and scroll-overlap suppression across restores. Manual expression-filter review found no blocker; `check_no_assets.py` passes.

**Clearance:** the snapshot / fault contract is now cleared. The interpreter core, scheduler-independent individually settled native effects, messages, and snapshot / fault contract are **cleared in full for scheduler-independent use**.

Scheduler integration, deferred execution, camera integration and sibling-gated effects remain excluded. Publication approval remains separate. This reviewer context must not implement the exposed subsystems.

**Verdict: cleared for implementation.**