# Codex spec re-review 21: docs/original/spec-script-vm.md (script VM, revision 3), 2026-09-13

Spec reviewer role (ADR-0009), with access to `re/`. Answered in the spec's next revision.

Reviewed [revision 3](C:/Users/przem/source/repos/opensherwood/docs/original/spec-script-vm.md) at commit `25b6dbe8aee71ebca4d78fb5e2382b4ff37b22a9`, checking all 20 review-17 findings against the matching executable, decompilation and VM notes. Only remaining defects follow. Addresses are virtual addresses, image base `0x400000`.

1. **Medium — VM-100; §§5, 8.1: the three-layer clock still overstates the evidence.**
   **Address:** `0050f710`.
   The wait’s reference counter is sampled again inside the active-window path; it does not measure the entire iteration. “On another host the realised length is the smallest multiple of the granularity ≥ 40 ms” also ignores counter phase, workload and scheduling.
   **Correction:** retain 40/400 ms as reported-counter thresholds, 46.875 ms as the measured reference cadence, and 46.875 ms as the explicit OpenSherwood choice. Remove the unsupported rationale that “the shipped data was authored at the realised rate.”

2. **High — VM-219; native rows 18–21, 42, 145–146; acceptance case 19: camera semantics remain partly wrong.**
   **Addresses:** `00571270`, `00571330`, `005713f0`, `005758d0`, `004cdfc0`, `004ca410`, `00570e30`, `00570e50`.
   **Confirmed:** native 18 writes the raw scroll target, the converted/clamped destination, and step length **2.0**; native 19 substitutes its float argument. Neither writes the camera corner. However:
   - The destination conversion centres the viewport on the point, applies truncation and zoom-dependent bounds/alignment; it is not merely clamping the supplied point.
   - Natives 18/19 preserve the existing scroll-speed override and ramp position. Consequently, 2.0 or the float argument is not an unconditional displacement on subsequent updates.
   - Native 20 invalidates the cached view but **does not clear actor-follow locking**. Scroll completion and natives 145/146 also conflate cache invalidation with that lock.
   - Arrival completion is tested at the beginning of a camera update; landing exactly on the destination completes on the following update.

   **Correction:** distinguish these behaviors, amend case 19, and reference an exact reviewed camera-conversion contract. Its unconditional “zoom is unchanged” also needs to exclude the conversion’s zoom-reset fallback.

3. **High — VM-014, VM-088; §8.3; acceptance case 17: the snapshot set remains incomplete.**
   **Addresses:** `00634c30`, `006392e0`, `0063a250`, `0063a320`, `0063a370`, `0063a400`.
   The **current script-parameter buffer**, distinct from the native-argument buffer, can survive a callback return with unconsumed contents. Subsequent callback arguments append to it before frame creation captures it. VM-013 already describes the underlying persistence, but VM-014 and §8.3 omit this state.
   **Correction:** preserve its contents and logical length, or explicitly restrict supported boundary states. Strengthen case 17 so execution after restore actually consumes residual script parameters and native arguments, reads the saved native result, and continues/closes the saved recording. Merely running tick 11 does not establish those properties.

4. **High — VM-201: overflow behavior is incorrectly generalized to every recording native.**
   **Addresses:** `00571100`, `00571020`, `00570f00`, `0057a020`, `00575350`, `00578cb0`.
   At counter wrap, natives 31/32 reject the zero level and native 30 refuses the still-open recording. But the common append operation checks the **recording’s existence**, not its level counter. Native 56 and recorded messages can therefore still append at level zero.
   **Correction:** distinguish those guards; remove “every recording native” and the unsupported universal claim that recovery is impossible through other recording operations. Keep the deliberate OpenSherwood fault at wrap, clearly separated from the original behavior.

5. **High — VM-203; native rows 46/47/63; acceptance case 5: the outside-recording correction introduces new errors.**
   **Addresses:** `00577650`, `005779a0`, `00574a70`, `00582640`, `00585500`, `004fc8f0`.
   Native 46’s immediate placement survives failure to record, and its result is correctly zero. However, placement uses a derived entry position; case 5 cannot assert that the actor simply remains at `p`. Native 47 is not the same placement operation and can reach an unchecked append through a null recording after modifying its entered lists. Native 63 checks the zero recording level **before** registering its taker, so its alleged outside-recording E+ effect is false.
   **Correction:** specify each operation separately, correct its failure class, and replace the placement expectation with a grounded fixture. Also remove VM-203’s universal “appends one element”: compound recording operations can create multiple elements.

6. **High — VM-213, VM-217; acceptance case 11: cancellation and queue expectations contradict execution.**
   **Addresses:** `00582560`, `00582620`, `00585320`, `00585b70`, `0058a3d0`.
   Cancelling B before dispatch skips B but does **not** decrement the level’s completion count. Case 11’s assertion that the level then completes is wrong. Its nested timer is also level-immediate: it enters the timer list during dispatch, not the manager’s pending FIFO. Finally, cancellation can synchronously notify an element’s executor; “unaffected until the actor processes it” is too broad.
   **Correction:** retain synchronous nested-launch ordering and same-state no-op behavior; correct cancellation’s completion consequences. Use an actually queued category to test FIFO ordering, and describe ordering by actual enqueue time.

7. **Medium — VM-089; §8.1; acceptance case 14: deterministic failure outcomes remain ambiguous.**
   **Addresses:** `00639300`, `00639370`, `00571760`, `00579350`, `0057c710`.
   VM-089 says U/T/X/F/C/N become faults, while §8.1 makes missing callbacks continuing no-ops and combines unchecked-access fallback values with `Fault::UncheckedAccess`. Case 14 expects null without establishing whether execution continues.
   **Correction:** define, per departure, whether the callback continues or terminates, what value is observable, and what happens to pending arguments. Preserve the now-correct distinction between container exceptions, fatal errors, null dereferences and unchecked instruction fetches.

8. **High — §0, §§4.1–4.3; VM-071; native rows 59, 136, 165, 254: clearance accounting still exceeds settled behavior.**
   **Addresses:** `0063a250`, `00573450`, `00576eb0`, `00579210`, `0057ba50`.
   An unidentified flag’s effect is not settled merely because its write is observed. Likewise, “pass-through” does not define the receiving subsystem’s behavior or the mapping between script codes and independently specified actions. Native 165’s “resets two of its fields” remains outside the supposedly complete gap register. VM-071 still says “none needed” despite the stated mapping requirement and supplies no deterministic outcome for an unwritten result-slot read. Native 136 also overaccepts temporary codes: subtracting 100 still leaves a validated code, rather than accepting every value ≥100.
   **Correction:** enumerate unresolved effects and consumers, supply exact dependency claims, correct native 136’s accepted range, and define the unwritten-read policy. Recalculate the “241 ids in full” assertion afterward. The explicit placeholders in §4.2 close the missing-placeholder issue; they do not establish fidelity.

9. **Medium — VM-030b; acceptance cases 14 and 16: table preconditions remain insufficient.**
   **Addresses:** `00571590`, `005714d0`.
   `N = 130` alone does not make native-3 index 999 invalid: the cart table extends the index space. Likewise, `n3(N16)` addresses a first cart only when one exists.
   **Correction:** specify the cart count in both fixtures, including an empty-cart case.

10. **High — VM-231; §5 and native rows 195/196: the manual expression filter still finds prescribed internal organization.**
    **Addresses:** `00464b20`, `00579430`, `00579470`, `00452030`.
    Remaining offending passages include:

    > “through a separate one-element sequence that itself never completes”

    > “k in 0..19 → slots 7..26”

    > “Campaign value `k + 7`”

    **Correction:** specify the final-frame hold and any required observable ordering without prescribing its sequence container. Describe twenty script-visible campaign values indexed 0–19; retain original storage numbering only if a separately established file/interface requirement needs it. The automated asset check passes, but does not clear these passages.

11. **Medium — VM-108, VM-216, VM-231; §1.1 and §9.4: dependencies remain unpinned.**
    **Addresses:** `00410620`, `004646e0`, `0046b210`, `00467a50`; revision status is documentary.
    “Draft, awaiting review” correctly replaces the former false clearance claim, but does not identify the dependency revisions. Movement and AI revisions are currently changing in the working tree; the movement draft already corrects several statements still present in its committed version.
    **Correction:** pin each dependency by revision and blob/commit, and distinguish committed contradictions from proposed resolutions. AI-event meanings, admission/priority, movement-failure units and outstanding action/speech completion rules remain implementation gates.

12. **Medium — identity block: reviewer-session references remain incomplete.**
    **Address:** documentary.
    Scratchpad names `review14.md` and `review17.md`, with session identities held elsewhere, do not provide the requested unique, retrievable reviewer-session references. The exposure summary is improved, but needs attribution to those references.
    **Correction:** link the committed reviews, identify their sessions and reviewed blobs, and add this re-review’s reference and exposure record. This reviewer inspected the VM, native wrappers/bodies, scheduler, sequences, camera, actor/AI helpers and campaign-container helpers; this context must not implement those subsystems.

**Implementation clearance now:** the interpreter’s instruction, calling and native-result protocols; core message payload, target, synchronous-delivery and dynamic-context rules; and individually settled, self-contained native contracts. This is partial clearance: the complete interpreter’s snapshot/fault contract, blanket native implementation, sequences and scheduler are not cleared.

**Must wait:** dependent camera, navigation, animation, AI, speech, admission and campaign effects require their identified sibling claims to be settled and reviewed. The §4.2 exclusions and §4.3 unresolved effects remain outside faithful implementation clearance.

**Verdict: fix-then-clear.**