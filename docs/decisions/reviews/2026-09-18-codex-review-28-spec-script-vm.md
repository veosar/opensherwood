# Codex spec re-review 28: docs/original/spec-script-vm.md (revision 5), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`. Answered in the spec's next revision.

Reviewed revision 5, commit `22a1e33ccb8a2c033e96c49e6da69b048caacf64`, blob `de8e583fa937c8e3bbeb31cd0cb93589ad4b143c`, against the matching executable and `re/notes/vm/`. All 11 review-25 findings were revisited. Addresses below are virtual, image base `0x400000`.

1. **High — VM-219; natives 18–20; case 19: vertical centring remains wrong.**
   **Address:** `005758d0`, specifically `00575910–00575961`; bounds at `00575a05–00575a66`.

   `W = screen_width / zoom` and `H = (screen_height − 80) / zoom` correctly describe the bounds, but the initial vertical corner uses **half the full screen height divided by zoom**, not `H/2`. Before truncation, it is `point.y − screen_height / (2 × zoom)`. At 640×480, zoom 1, an interior point `(1000,1000)` produces `(680,760)`, not `(680,800)`.

   **Correction:** distinguish centring dimensions from clipping dimensions. Evaluate fallback using the negative **truncated** initial coordinate and the corresponding overflow test. The zoom-0.5 odd-coordinate adjustment is supported. Add vertical and alignment fixtures. Case 19’s active-camera fixture also needs its own axis-aligned, unclipped destination for an exact three-pixel assertion. Test native 20’s immediate state before camera processing; do not assume an intervening drawn frame exists.

2. **Medium — VM-201; §8.1: recorder guards and wrap coverage remain incomplete.**
   **Addresses:** `00572ab0`, `00572ba0`, `00572d50`, `00572e40`, `00572c70`, `00573190–00573370`, `005753d0`, `00575470`, `00575050`, `005730b0`, `00572ef0`, `00573d10`, `00582640`.

   Besides the four listed counter guards, natives **33–36, 42, 49–51, 54/55, 61/62 and 69** reject level zero; 64 reaches 45’s guard. Native **57 has no preliminary recording guard** and reaches the common append, so its level-zero append is established.

   Moreover, compound walk recording itself advances the level counter. Faulting only when native 32 would wrap does not establish that other recording operations cannot produce wrapped levels.

   **Correction:** complete the guard inventory and explicitly settle or exclude overflow during compound recording. The corrected 65,535th native-32 increment and the distinction between recording existence and level zero are cleared.

3. **High — VM-203; native 47: the two walk destinations are still conflated.**
   **Addresses:** `005779a0`, `00582640`, `004fc8f0`.

   Native 47 first records the compound walk toward the **supplied location**. Its final walk targets the separately derived entry geometry. The row instead directs the compound walk toward that geometry.

   VM-203 also says compound recorders append “several” elements tagged with the current level. A same-zone path can produce one walk, while longer constructions advance levels internally.

   **Correction:** distinguish the compound destination from the final destination; describe “one or more” elements and their level progression. The corrected stored-entry/current-origin distinction, native 46’s single walk, and case 5’s deferred placement assertion are supported.

4. **High — §4.3; natives 57/70/71; §0: the unresolved register is still inconsistent.**
   **Addresses:** `00573d10`, `00573da0`, `00573f00`, `00588530`, `00467a50`.

   The register contains **16 distinct ids**, and adding 100 and 106/107 fixes the previous omissions. However, 70/71 pass their fourth argument through the same seek parameter path that remains unresolved as `v` for 57. Calling it `range` does not supply the missing consumer contract. The cited movement element row does not establish that parameter’s semantics.

   **Correction:** settle the shared parameter for all three natives, or include 70/71 in the unresolved register. With the latter choice, the counts become **18 unresolved and 235 otherwise settled**, before other gates. Also replace 57’s unspecified “validation” boundary with its actual checks; it does not validate actor/target membership before constructing the request.

5. **Medium — VM-231; native 51: the native row retains the unconditional hold promise.**
   **Addresses:** `00464b20`, `0058a940`, `00582560`, `00585b70`, `0058ba60`.

   VM-231 now correctly orders the deferred hold request before animation completion and permits next-level activity before hold admission. Native 51 still says the actor “then holds the last frame” unconditionally.

   **Correction:** make the native row describe a hold request, conditional on admission and subsequent interference. The revised request/completion/admission ordering itself is cleared.

6. **High — VM-070/089; §§8.1/8.3: the complete fault/restore contract still has uncovered cases.**
   **Addresses:** `00639370`, `0063a320`, `00634eb0`, `004ba760`, `004ba4e0`; snapshot additions are OpenSherwood policy.

   The new termination paragraph resolves ordinary fault unwinding, persistent buffers, both registers, enclosing-callback continuation and surviving recording state. Remaining gaps are:

   - The sentinel jump **“ends as by 0x06”**, but the termination contract explicitly covers rows saying “terminates.” A return from a nested script function and termination of its entire callback are different outcomes.
   - Scroll overlap is detected **before the attempted callback frame is pushed**. Unwinding “the faulting callback” does not identify the boundary or retained scroll context in that case.
   - The default out-of-range `0x08` policy describes a native return and wrapper pop, without explicitly specifying the instruction’s destination and register effects.
   - Faults are hashed and missing-callback faults are recorded once per class/name, but §8.3 omits the fault state and suppression state needed to reproduce that behavior after restore.

   **Correction:** specify those cases explicitly and add nested-fault and missing-callback restore traces. Case 17 now correctly tests normal buffer persistence, but does not cover these fault paths.

7. **Low — identity block; §§0/9.4/10: documentary reconciliation is unfinished.**
   **Address:** documentary; camera anchor `005758d0`.

   The analyst identity still covers only revisions **1–4**, leaving revision 5 unattributed. Section 9.4 still claims complete camera agreement with the movement pin, contrary to §1.1’s explicit conversion disagreement. “All drafts awaiting review” also remains in the stopping statement and provenance despite the recorded review dispositions.

   **Correction:** attribute revision 5 and align these summaries with §1.1. All four sibling commit/blob pins match. The documented maintainer-held reviewer-session mapping and individual exposure attribution address review 25’s reviewer-identity requirement; the private mapping itself was not available for independent verification.

**Cleared now:** the existing instruction/calling/native-result protocols and message contracts; VM-088, both-buffer snapshot preservation and corrected case 17; native 223’s read classification; the ordinary termination rules identified above; the corrected distinction between ordinary arrival and approach completion. Review 25’s expression-filter finding is resolved: the prescribed one-element containers are removed. Manual reapplication found no remaining expression-filter blocker, and the asset check passes.

**Still waiting:** the findings above, full recording/scheduler integration, camera conversion/interference, and the explicitly excluded or sibling-gated navigation, actor admission, speech/action, AI and campaign effects. **The interpreter core, all natives, messages and snapshot/fault contract cannot collectively be cleared in full.**

This reviewer context inspected VM/frame, native, recording/dispatch, camera, movement/actor and animation evidence and must not implement those subsystems.

**Verdict: fix-then-clear.**