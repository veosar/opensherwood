# Codex spec review 17: docs/original/spec-script-vm (revision 2).md (script VM), 2026-09-13

Spec reviewer role (ADR-0009), with access to `re/`. Answered in the spec's next revision. Class and source-file names quoted by the reviewer as offending passages are redacted here.

1. **High — VM-100; §§5, 8.4, 10: clock conversion remains wrong.**
   **Addresses:** `0050f710`, `004c6ef0`.
   Retain 40/400 ms as guarded wait thresholds, not guaranteed frame durations. VM-100 must incorporate the movement spec’s counter-granularity conclusion: **46.875 ms per realised normal frame**, approximately 21⅓ frames/s, on the measured host; 25 executed ticks therefore span **1.171875 s**, and 75 span **3.515625 s**, absent skipped or delayed updates. Remove the conversion to an independent 64 Hz animation clock. Distinguish executable evidence, host-dependent timing inference, and OpenSherwood’s chosen fixed timestep.

2. **High — VM-219; native rows 18–21: camera semantics still need correction, including a contradiction in the movement spec.**
   **Addresses:** `00571270`, `00571330`, `005713f0`, `00571200`, `005758d0`, `004cdfc0`.
   The rows must say:
   - **18:** requests scrolling to the location and sets the scroll step length to **2.0 map pixels per camera update**.
   - **19:** the same, with step length supplied by its float argument.
   - **20:** immediately sets the clamped camera corner and invalidates the cached view; the scroll step length is unchanged.
   - **21:** sets a **zoom request**, whose visual change occurs through subsequent camera updates.

   “Movement scale” lacks the required units, and row 20 incorrectly calls its operation a scroll. The movement spec’s assertion that **18/19 also jump immediately** conflicts with the executable: they update the scroll target, whereas 20 updates the camera corner. Record and resolve that sibling contradiction rather than importing it. Also distinguish cached-view invalidation from actor-follow locking.

3. **High — VM-108(b): the first callback argument is still misidentified.**
   **Address:** `0040dcb0`.
   For events **100–103**, the first argument is the **NPC receiving the callback**, also installed as current actor—not its target. Events 104–106 use a different stored reference whose meaning remains unresolved. The seven event meanings requested in review 14 are still unavailable; their assumption mapping records the gap but does not resolve it.

4. **High — VM-108, VM-216, VM-231; §1.1: dependencies are falsely described as reviewed.**
   **Addresses:** `00410620`, `004646e0`, `0046b210`, `00467a50`; review status is documentary.
   Both navigation and AI specifications still identify themselves as **drafts awaiting review**. Replace “reviewed there” with their actual status and identify the revisions on which clearance depends. The three contradictions listed in §9.4 remain unresolved dependencies. In particular, the reference to **AI-066 for posture codes is wrong**: that claim describes visibility testing. “100 clock units” also needs an established unit before it can define movement-failure timing.

5. **High — VM-231; natives 49–51; §§1.1, 9.1: animation completion remains contradictory and stale.**
   **Addresses:** `00464b20`, `00467a50`, `0046bd40`.
   “Animation / AI specs (not written)” is obsolete. Reference the existing movement/animation specification with its actual review status. Replace “animations at the end of the clip / loop” with separate contracts: **49 completes after the clip; 50’s looping animation does not complete; 51 completes after the clip and leaves the actor holding its final frame**. Speech, action completion, and admission/priority rules still require explicit unresolved boundaries.

6. **High — VM-014, VM-088; §8.3: the snapshot contract discards persistent state without justification.**
   **Addresses:** `00570f00`, `00571020`, `006390b0`, `00635210`, `00635240`.
   The statement “recordings never span a native return” is false: beginning a recording returns while that recording remains open. Nothing shown prevents a recording from spanning callbacks. Preserve recording state at permitted snapshot boundaries, or explicitly define and justify a restriction that forbids such boundaries. Likewise, native results and unconsumed argument cells persist; retail adjacency/balance observations do not establish that this state is generally disposable. Include a restore-equivalence acceptance case.

7. **High — VM-095: nested callbacks do not automatically restore both result registers.**
   **Addresses:** `00635210`, `00634de0`, `00635090`.
   The outer native instruction overwrites the **native-result register**, but does not restore the previous **callback-return register**. A nested value-returning callback can leave its result visible when the outer callback subsequently returns without a value. On the same instance it can also overwrite the enclosing frame’s result slot. Correct “re-written by the outer instruction that follows” and add acceptance coverage for these observable effects.

8. **High — VM-201: barrier overflow remains unresolved.**
   **Addresses:** `00571100`, `00570f00`, `00571020`, `0057a020`.
   Static class size cannot prove overflow unreachable: loops can repeat recording and barrier calls. At the 16-bit wrap, native 32 returns zero; natives 31/32 then treat the level counter as inactive while the outstanding recording still prevents native 30 from opening another. Specify this boundary and its chosen departure explicitly. The claimed maximum of 2,336 instructions per class is also false.

9. **High — VM-203; native 46: “outside recording means no effect” remains overbroad.**
   **Addresses:** `00577650`, `0057a020`.
   Native 46 performs placement and entered-list changes **before** its final recording check. Failure to append the element therefore does not undo all effects. Describe the immediate effects separately from successful recording, classify the failure accordingly, and test the outside-recording case.

10. **High — VM-210–VM-217: sequence reentrancy is still underspecified.**
    **Addresses:** `00582560`, `00582620`, `00585320`, `0058a3d0`.
    The revision explains synchronous completion but still lacks the requested contract for callbacks that launch another sequence or alter pending work during dispatch. State the observable ordering of same-level immediate elements, nested launches, and subsequent siblings. Also specify that assigning an element its existing state does not repeat completion or propagation. Add acceptance cases for nested launch and cancellation before a sibling’s dispatch opportunity.

11. **Medium — VM-020, VM-103, VM-212; acceptance cases 3, 7, 9, 13: several expected outcomes are invalid or lack preconditions.**
    **Addresses:** `004e3e30`, `004c6ef0`, `00582560`, `00585b70`.
    Case 3 uses native 1 on an initially empty mission-variable array; declare index 0 first. In case 7, forcing during `Hourglass(1)` causes **`CheckVictoryCondition(1)` in that same tick**, not `(2)` at T=50. In case 9, a recorded page opens during **queue draining**, not initial sequence launch. Case 13 must establish table and variable-array sizes before treating indices 999 and 99 as invalid.

12. **Medium — VM-221; acceptance case 13: zero-duration timers still have conflicting outcomes.**
    **Addresses:** `004c6ef0`, `00575350`, `00586f10`, `00587160`.
    Remove the acceptance expectation that native 56 with zero “never completes.” A recorded zero counter completes on its **2³²-th visited timer pass**. Negative counters likewise have finite wraparound lifetimes determined by their unsigned bit patterns; “about 2³²” is not accurate across the entire negative range. If OpenSherwood deliberately treats these as indefinite, document that departure.

13. **Medium — VM-092, VM-102, VM-103, VM-103b; acceptance case 8: end-state corrections are inconsistent across the text.**
    **Addresses:** `004c6ef0`, `004e3220`, `004e3260`, `004e3cd0`.
    `CheckVictoryCondition` result 2 requests end bookkeeping; it does **not necessarily mean failure** after victory was declared. Case 8’s “only when both PCs have died or left” omits the other end triggers already listed in VM-103. The departure notice is processed **before** the terminal-flag checks, contrary to the numbered order. VM-102 must reference increment step **5**, not step 4.

14. **Medium — VM-113: the second engine-message trigger is too broad.**
    **Address:** `0050f710`, particularly the path through `0050fd58`.
    Message 1001 is not sent on every return from mission selection. The return must take the accepted-selection path, retain the current mission, and satisfy the hub guard. Cancellation and choosing another mission follow different paths. State these conditions explicitly.

15. **Medium — VM-089, VM-090; §8.1: failure classes still misdescribe distinct mechanisms.**
    **Addresses:** `00579350`, `0057c710`, `00639300`, `00639370`, `00634bf0`.
    Native 168 checks its index as unsigned: negative inputs also enter its out-of-range **exception** path, rather than an unchecked negative access. That path throws an exception; it is not the engine fatal-error routine. Missing callback lookup produces a null dereference, also distinct from that routine. Opcode 0x00 proceeds to an unchecked instruction fetch; an inevitable hang is not established. Correct these classifications and make the corresponding OpenSherwood fault outcomes consistent.

16. **Medium — VM-086; native 46: its result is settled, not unspecified.**
    **Addresses:** `00577650`, particularly `00577984`; wrapper `00405180`.
    Native 46 explicitly clears its low result byte before returning. The wrapper therefore exposes **zero**, including successful recording. Replace the row’s “unspecified” result; acceptance case 15 already expects the correct value.

17. **High — §4: the assumption register and exclusions remain incomplete.**
    **Addresses:** claim-dependent, including `00573450`, `00576eb0`, `0057ba50`, `005766f0`, `00576830`.
    VM-071 and VM-088 are marked inferred but absent from the mapping. Native 45’s unresolved variant is also omitted. Unsettled descriptions such as native 59’s “four fixed actions,” native 136’s “seventeen decisions in order,” native 254’s unidentified flag, and natives 259/260’s unnamed action states are not covered by the claimed complete semantics or exclusions. Separate status from confidence—“medium” is not an allowed claim status. Several excluded integer-returning rows also lack the placeholder promised by §§4.1–4.2; define those as explicit implementation choices or require a deterministic unresolved-operation fault.

18. **High — VM-217; natives 103, 173, 241; §9.3: the expression filter still fails.**
    **Addresses:** `00585320`, `00571b30`, `005795b0`, `0057b2b0`, `004c6ef0`.
    Remaining offending text includes:

    > “level offsets 0x4790, 0x4791 and 0xD0”

    > “the fifth entry of the options store”

    > “its ‘stop’ with reason 6”

    > “*refused* (5) or *cancelled* (6)”

    > “a random-pick routine on entry `k − 1` … then two values set on an object not identified”

    These retain recovered layout, internal state numbering, or function decomposition without establishing a required interface. Keep physical offsets and unidentified internal operations in analyst notes. Use independently named unknown state, observable effects, and required ordering in the specification. Exclusion from implementation clearance does not exempt text from the expression filter.

19. **Medium — VM-030b, VM-201; §§4.2, 6, 10: remaining width and corpus inaccuracies.**
    **Addresses:** `005714d0`, `00571590`, `0057bfe0`; corpus evidence: all 39 SCBs.
    Native 75 returns the **full table count**, whereas native 3 truncates its table boundary to 16 bits; they cannot share the currently defined truncated `N`. Fresh corpus counts show **one** native-261 call, not two, and a largest class of **6,948** instructions. The reported 0x07 successors also omit one case: the “other” category contains **five**, not four, successors.

20. **Medium — necessity record and identity block: handoff metadata is still incomplete.**
    **Address:** documentary; none.
    Preserve the newly supplied analyst identifier and separate pending publication approval. Add a unique reviewer-session reference and an explicit exposure record covering the VM and inspected helper subsystems; “analyst—this session; implementer—none yet” does not record reviewer exposure. Identify the revision covered by review 14 separately from this re-review. Finally, “semantics of 254 ids” contradicts the **12** listed exclusions, even before the additional unresolved rows above, and “scheduler’s order and clock” cannot be presented as completed while these corrections remain.

**Verdict: fix-then-clear.**