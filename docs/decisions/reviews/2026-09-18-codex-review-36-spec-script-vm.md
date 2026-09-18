# Codex spec re-review 36: docs/original/spec-script-vm.md (revision 8), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`.

Reviewed revision 8, commit `f35eebb15ddaf0ccdf5ecf8e21da128df7055fd5`, blob `a745790b539c6656a2719f08cbe754f53e9db847`.

1. **Medium — VM-200/202, §8.3, acceptance case 26: the combined restore fixture is unreachable.**
   **Addresses:** `00570f00`, `00571020`, `0057a020`.
   [Case 26](C:/Users/przem/source/repos/opensherwood/docs/original/spec-script-vm.md:696) keeps the recording opened by `PostInitialize` open through tick 26, while also requiring case 25’s separate recording to open and launch at tick 25. There is only one global recording: native 30 refuses the second opening; subsequent recording natives append to the existing recording; native 31 closes and launches that recording. Its provenance would remain the `PostInitialize` opener, and it would no longer be open at the snapshot.

   **Correction:** split the open-recording restore check into a separate run, or open the second recording only after case 25’s sequence has launched, extend it in a later callback, and update its expected opener triple. Preserve the pending-sequence attribution and scroll-overlap suppression restore assertions.

Case 23’s helper-return trace is supported. Case 25’s next-drain hand-over and refusal propagation are supported. All three suppression sets are now explicitly hashed and restored; the opener-based sequence provenance is clearly identified as OpenSherwood policy and included in snapshots.

**Clearance:** the interpreter core, scheduler-independent individually settled native effects, and messages remain cleared. The snapshot/fault rules now resolve review 35’s missing-state concerns, but **the complete snapshot/fault contract—and therefore the requested bundle—cannot yet be cleared in full**, because case 26 supplies an impossible acceptance fixture. Scheduler integration, deferred execution, camera integration and sibling-gated effects remain excluded.

Manual expression-filter review found no blocker; the asset check passes. This reviewer context must not implement the exposed subsystems.

**Verdict: fix-then-clear.**