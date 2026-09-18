# Codex spec re-review 45: docs/original/spec-script-vm.md (script VM, revision 10: the seven implementer answers, the scheduler and the sequences), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`. To be answered in the spec's next revision.

Reviewed revision 10, commit `1c7aecf`, blob `35459de492c79e00c4559de73c28a597b9560bd5`. Only remaining defects and implementation gates follow.

1. **High — §11.5, §4.1 handle equivalence, VM-030. Addresses: `005716a0`, `00571700`, `00579c90`, `00579d00`, `00579e80`, `00579ef0`, `00570a60`.**  
   The three conditions do **not** establish equivalence for literal zero-based position handles. Position 0 collides with null; equal positions in different lists collide under native 86. The original distinguishes those objects.

   **Correction:** require identity preservation across every handle-producing interface, with null distinct from every valid object and unrelated objects distinct across lists. Indices may identify objects internally, but script-visible handles and inverse results cannot simply be the same integer. Preserve absent-handle and range behavior separately. Add first-entry/null and cross-list equality fixtures.

2. **Medium — VM-010/011/014, §§8.1, 8.3, 11.2; expression filter. Addresses: `00639960`, `00639a00`, `0063a1e0`.**  
   The absence of allocated global storage is supported, but VM-014 and §8.3 still require the global block to be snapshotted and hashed. Section 11 permits retaining the allocation while §8.1 says none exists.

   **Correction:** remove global storage from the authoritative snapshot contract and consistently retain the chosen read-zero/write-terminate fault policy. Remove the recovered descriptor’s pointer/size layout and factory/shutdown organization from the public explanation; “no allocated global storage; class-00 accesses are invalid” supplies the necessary behavior. The heuristic asset check passes, but does not resolve this expression-filter issue.

3. **High — native 118, §11.6. Addresses: `00571fb0`, specifically `00571fe4–00571fed`; setter `00572300`; loaders `004a1e90`, `004705f3`.**  
   Native 118 does **not** expose property 12. Its accepted property range ends at 11; reading 12 takes the error path and returns −1. Property 12 belongs to native 117’s setter contract.

   **Correction:** distinguish the twelve readable properties, 0–11, from the setter’s additional property. The money-field correction itself is right: soldier `BORG + 0x23`, civilian `OILE + 0x1c`, both u32. **Emb02’s single civilian carries 4000**, and its victory callback queries element 40, property 1. Replace stale references to the old `rhm.md` layout, already corrected in this commit. Restrict “only looting can change it” to a demonstrated mission-specific statement; native 117 can also write money.

4. **Medium — §11.7, §4.2, native 224. Addresses: `005795b0`, `0057adf0`, `004ec910`.**  
   Native 173 occurs **fourteen times in one mission**, not fourteen missions. Also, §4.2’s placeholder description says the repulsive point “is created,” whereas §11.7 says the placeholder drops that effect.

   **Correction:** fix the corpus count and distinguish the original’s creation request from the interim implementation’s omitted effect. State consistently whether the default placeholder both returns 0 and omits creation. The default-placeholder/opt-in-strict decision itself is acceptable; its justification does not establish campaign playability.

5. **High — scheduler VM-101–104, §9.3. Addresses: `0050f710`, `004c6ef0`, `004e3220`, `004e3260`, `0050eca0`.**  
   Scheduler control inputs remain insufficiently specified: the three transition flags’ sources and clearing conditions; the captured-character reference; the civilian death exemption; and the interface transition for campaign node types 3/6. The first-loop modal guard remains unresolved.

   **Correction:** supply behavioral producers, reset conditions and effects on subsequent phases, or explicitly exclude the associated feature. Establish the complete first-iteration ordering, including camera processing before `PostInitialize`, and distinguish an attempted tick from an executed tick. The 25/75-tick cadence and ADR-0010’s chosen frame length do not require re-investigation.

6. **High — scheduler VM-030, VM-103 step 8, VM-105, VM-107–110. Addresses: `004c2720`, `004d8460`, `004c6ef0`, `004b9f40`, `00464230`, `0040dcb0`, `004105d0`, `004abfe0`, `0057f8c0`, `0057fcc0`, `0057fdb0`.**  
   The callback schedule still lacks settled claims for script-zone registration/order, target-family `ActionChange`, events 100–106 and their unidentified payload, and the patrol variant of `ReachPoint`. Membership-dispatch behavior does not specify the physical conditions and phase that generate zone entry/exit.

   **Correction:** settle those sources and their ordering, including effects of registration/removal during element updates. Close the scroll-source question: its update is reached through the ordinary element-update dispatch. Specify counter initialization, retention while inactive, and resumption; visible/hidden status must not silently substitute for the update eligibility condition.

7. **High — VM-215, deferred-fault row §8.1, cases 9/25. Addresses: `0058ba60`, `004c6ef0`, `004904c0`, specifically `0049083b`.**  
   Step 9 is not the only queue-drain call. An actor order-execution path also drains the manager. Consequently, unconditional statements that work queued after step 9 must wait until the next tick are not yet established.

   **Correction:** identify the additional drain’s behavioral trigger and scheduling phase. Specify ordering and re-entrancy relative to the regular drain, and qualify the deferred-fault fixtures accordingly. Any excluded path needs an explicit scope boundary.

8. **High — VM-218, VM-213; level-executor completion ordering. Addresses: `004ca410`, `00585320`, `00582560`.**  
   “At once” omits observable ordering. Freeze-all elements complete **before** writing the flag, so a synchronous successor observes the previous value. Camera jump/lock/clear elements also complete an existing camera element before applying their own camera changes; recorded camera jump releases the actor lock.

   **Correction:** state these effect-versus-completion orderings and their synchronous successor consequences. Cover immediate and queued successors separately. Preserve the distinction between recorded camera jump and direct native 20.

9. **High — VM-203, entered-actor state, §9.1. Addresses: `00582640`, `00583630`, `00577650`, `005779a0`, `0057c950`.**  
   Full recording remains gated by the precise compound-walk construction: which functional elements are produced, their level grouping, modes 2/3 of native 45 and native 212, and entered-actor behavior beyond initial placement.

   **Correction:** provide reviewed navigation claims for those observable results, including duplicate entry, placement failure, partial effects and level advancement. “Several elements” plus an obsolete navigation pin is insufficient to build the recording contract.

10. **High — VM-216/217/231; deferred-target policy. Addresses: `004646e0`, `0046b210`, `0046ad00`, `00467a50`, `00464b20`, `00467230`.**  
    Actor admission, busy-actor replacement/merge/queue rules and cancellation consequences remain open. Completion is still unsettled for speech, native-59 actions, corpse elements, complete movement arrival/failure behavior, and seeks with an invalid target. Native 51’s deferred hold also depends on those admission rules.

    **Correction:** supply per-category admission and completion contracts, including refusal, interruption and successor timing. Pin the actual reviewed sibling claims. Preserve the existing isolated facts—such as animation 50 not completing—without treating them as clearance of the actor executor.

11. **High — VM-218/219/222; §1.1 dependency pins. Addresses: `004cdfc0`, `004cf610`, `004cfce0`, `004c8120`, `005758d0`.**  
    Complete camera-sequence timing still lacks accepted-zoom completion timing, complete lock/follow interference, and the remaining scroll-ramp behavior. However, the VM’s old pins also incorrectly leave already-reviewed facts looking wholly unresolved: review 44 clears conversion and several native-write/interference facts individually; review 37 likewise grants navigation partial clearances.

    **Correction:** refresh pins and reconcile the exact dependent claims. Separate already-cleared conversion/write facts from the genuinely outstanding timing and execution rules. Add completion-order fixtures at the tick/camera boundary.

12. **Medium — VM-102/105, §8.3; scheduler/sequence integration snapshots. Addresses: `004c6ef0`, `004b9f40`, `0050f710`.**  
    The integration contract does not explicitly assign snapshot ownership to scroll cadence counters and the one-time `PostInitialize` state. “Tick boundary” also leaves the boundary relative to the following camera update unclear.

    **Correction:** identify those authoritative states and their owners; define the snapshot phase, tick-counter width/wrap, and whether fault/provenance timestamps use the tick’s entry counter or the incremented counter. Provide continuation fixtures spanning skipped ticks, scroll resumption, first initialization and camera-triggered sequence advancement.

**Seven answers — verdict: fix-then-clear.** Answers **1, 3 and 4 are cleared for implementation** within their stated scope; answers **2, 5, 6 and 7 require the corrections above**. The finite normal-result rounding argument is consistent with the established double-rounding result; it should retain its explicit scope. [Roux, *Innocuous Double Rounding*](https://jfr.unibo.it/article/view/4359)

**Section 3.5 — verdict: fix-then-clear.**

**Section 3.7 — verdict: fix-then-clear.**