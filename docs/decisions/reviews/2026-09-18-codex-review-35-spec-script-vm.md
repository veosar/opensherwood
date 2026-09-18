# Codex spec re-review 35: docs/original/spec-script-vm.md (revision 7), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`.

Reviewed revision 7, commit `5187321b7bcb683f9914c72d657c90c33db10bdc`, blob `2ec3f7743145b63acd2c3fdd99d4243dff61f2e8`, against the decompilation and VM notes. Addresses below are virtual addresses, image base `0x400000`.

1. **Medium — VM-070/095, acceptance case 23: the return-register setup is not executable as stated.**
   **Addresses:** `00634de0`, `00639370`, `00639300`.
   An earlier `0x07` **in the active Hourglass invocation** ends that invocation; it cannot then call native 109.
   **Correction:** explicitly seed 9 through a previous completed callback, or through a script helper that returns 9 before the message call. The revised saved-return-address sentinel rule itself is correct.

2. **Medium — VM-089/215/217, §8.1 deferred-fault row: timing and propagated state contradict the referenced contracts.**
   **Addresses:** `004c6ef0`, `00582560`, `00582620`, `0058ba60`, `00585570`, `00585320`.
   Hand-over need not occur in the tick that dispatches the level: timer completion can dispatch a seek **after** that tick’s drain. Also, VM-217 propagates the **same state**; refusal therefore makes later levels refused, whereas the new row says cancelled.
   **Correction:** attribute the fault to the actual hand-over tick, which may be later. Specify refusal throughout the next-level chain, or explicitly declare a different departure. Add a timer-before-seek fixture covering this boundary.

3. **Medium — VM-014/089/094/200, §§8.1–8.3: new fault state is not fully incorporated into the snapshot contract.**
   **Addresses:** `004ba760`, `004ba4e0`, `00585570` (trigger provenance; preservation is OpenSherwood policy).
   The new once-per-scroll suppression is absent from the enumerated suppression sets. Deferred faults now require recording-class and launch-tick attribution, but the recording/live-sequence snapshot inventory does not include that provenance. A recording can span callbacks, making “the class that recorded it” ambiguous.
   **Correction:** explicitly preserve and hash scroll-overlap suppression; define whose recording identity is retained and preserve the attribution needed by pending elements. Add restore coverage, or explicitly keep deferred attribution outside the cleared snapshot contract.

The interpreter behavior **including the sentinel departure**, scheduler-independent settled native effects, and message contracts are cleared. The take-overlap outcome and corrected case 19 are also supported. **The requested bundle is not cleared in full: the complete snapshot/fault contract still needs finding 3 resolved**, and case 23 needs its setup corrected. Deferred execution, scheduler integration, camera integration and sibling-gated effects remain excluded; 235 remains an accounting total, not blanket clearance.

Manual expression-filter review found no blocker; the asset check passes. This reviewer context must not implement the exposed subsystems.

**Verdict: fix-then-clear.**