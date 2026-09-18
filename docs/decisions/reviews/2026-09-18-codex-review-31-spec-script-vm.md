# Codex spec re-review 31: docs/original/spec-script-vm.md (revision 6), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`.

Reviewed [revision 6](C:/Users/przem/source/repos/opensherwood/docs/original/spec-script-vm.md), commit `c2c578a763bf34c8502e589ca751a52e59778045`, blob `35346f60c89d02c342ec8240ffbead8d5d8e92ba`. Executable SHA-256 and image base match. All seven review-28 findings were revisited against the decompilation, relevant raw instructions and VM notes.

1. **High — VM-070/095, §8.1: sentinel-return depth rule contradicts nested callback entry.**
   **Addresses:** `00639300`, `0063a250`, `00634dd0`, `0063a320`, `00635210`.

   “Exactly as by 0x06” is correct, but “at depth > 1 control returns to the caller frame’s return pc” is not. A nested engine callback on the **same instance** also receives a sentinel return address, despite its depth exceeding one. Popping that frame ends the nested interpreter invocation; the enclosing native-call instruction subsequently resumes the outer callback.

   **Correction:** define the outcome using the return address saved in the **popped frame**, distinguishing script-call frames from engine callback frames. Add both a script-function sentinel case and a same-instance nested-callback sentinel case. Case 20 uses another instance and an arithmetic trap, so it does not cover this distinction.

2. **High — VM-089; natives 57/70/71; §8.1: deferred seek faults are assigned an impossible native-return outcome.**
   **Addresses:** `00573d10`, `00573da0`, `00573f00`, `00588530`, `00585570`, `00467a50`.

   The corrected native rows establish that actor/target validation does not occur during recording. Nevertheless, §8.1 places these execution-time faults among native reads that “return null / 0,” pop arguments and continue—or terminate—the callback. The recording native can already have returned successfully, and its callback can have ended before seek execution.

   **Correction:** separate recording results from deferred execution faults. Specify the affected element’s disposition, propagation, attached-message treatment and fault attribution, or explicitly leave that execution policy behind the actor/scheduler gate. Do not retroactively change the recording native’s result or unwind a callback that no longer exists.

3. **Medium — VM-094/106; §8.1: scroll-overlap rejection still lacks a complete take outcome.**
   **Addresses:** `004ba760`, `004ba4e0`, `00404860`, `004ca410`.

   Preserving the enclosing scroll and rejecting the attempted callback before frame creation are now explicit. “Returns normally with its usual result” remains insufficient: the take operation normally derives its result and final status from `IsTaken`, which this policy skips. Status 3 and the sound occur **before** the overlap check.

   **Correction:** specify the rejected take’s result, retained status and preceding effects, whether any saved callback return value is consulted, and the take element’s completion. Add an overlap trace with a previously non-zero callback return register. Cases 20/21 exercise neither this rejection nor its bookkeeping.

4. **Medium — VM-219, acceptance case 19: the active-camera fixture overstates the three-pixel displacement.**
   **Address:** `004cdfc0`, particularly `004cea9d–004cead1`.

   The fixture travels 200 pixels at speed 3. The update limits displacement to the remaining distance: 66 moves of 3 pixels, then one move of **2** pixels, followed by the arrival/completion update. “Each update moves … exactly 3 px” is false.

   **Correction:** state the shortened terminal step, or choose a distance divisible by three. The revised centring, clipping, fallback, zoom-0.5 alignment and immediate native-20 assertions are supported by `005758d0`’s raw instructions.

**Cleared now:** review-28 findings 2, 3, 4, 5 and 7: the recorder guard inventory and compound-overflow coverage; native 47’s supplied-location versus final-entry destinations; the **18 distinct unresolved IDs / 235 otherwise-settled IDs** accounting and seek request boundaries; native 51’s conditional hold request; and documentary reconciliation. The sibling pins match; the private reviewer-session mapping remains independently unverified.

The existing interpreter instruction/calling/native-result protocols, scheduler-independent settled native effects and message contracts remain cleared. The per-instance snapshot contents, fault-log/suppression preservation and cases 20/21 are also cleared individually. **The interpreter core including its sentinel departure, and the complete snapshot/fault contract, cannot yet be cleared in full—even for scheduler-independent use.** Deferred execution and existing sibling gates remain separate exclusions; 235 is not a blanket implementation clearance.

Manual expression-filter review found no blocker; the asset check passes. This reviewer context inspected decompilation and must not implement the exposed subsystems.

**Verdict: fix-then-clear.**