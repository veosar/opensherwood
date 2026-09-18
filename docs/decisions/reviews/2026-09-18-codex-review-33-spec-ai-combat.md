# Codex spec re-review 33: docs/original/spec-ai-combat.md (revision 4), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`.

Reviewed revision 4 at commit `392e4a3a9a1452d3824bece1c8be8827b0cd715d`, blob `58639d3662664b1654895783afed961d130b480b`, against all 13 findings, the AI notes, decompilation and targeted disassembly. The executable SHA-256 matches. All addresses below are virtual addresses, image base `0x400000`.

1. **High — AI-008, AI-066, AI-068, AI-176: the rounding contract remains incorrect.**
   **Addresses:** `0x00489e30`, `0x00489974–0x0048999a`, `0x0047b90c–0x0047ba3d`. [Specification](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:144).

   The graded perception result is rounded to single before posture scaling; it does not remain unrounded until entry storage. Conversely, downhill aiming reach remains extended through its squaring: the **square**, not reach itself, is stored as single. The contract also omits the stored horizontal displacement and height difference.

   **Correction:** reconcile all three perception passages and specify the actual aim boundaries. The supplied same-height equality fixture is correct. A distinguishing downhill fixture is R = 204, dz = 1, dx = 204, dy = 8.450092315673828: original squared reach is **41742.3046875**, while prematurely storing reach produces **41742.30859375**; D² is approximately **41742.30666085133**. The original rejects; the proposed extra rounding accepts. Withholding unspecified paths does not cure these affirmative misstatements.

2. **High — AI-151: corrected penalties do not make the complete scoring rule correct.**
   **Addresses:** `0x00482240–0x00482385`, `0x00485590`, `0x00485240`. [Specification](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:1084).

   The threefold penalty, circle exception, decay timing and earlier-row tie preference are supported. However, the “expected damage plus expected stun” formula is not what the scoring calculation produces. Damage is capped by current health; its weighted, truncated result can also replace the stun contribution. For capped damage 10, defence 0 and stun 0, the contribution is **18**, then the nonzero-contribution bonus makes **48**.

   A friend ordinarily **disqualifies** the candidate rather than subtracting one; the soldier/nonzero-hesitation exception matters. Only nonzero contributions count toward opponent requirements, and a winner must exceed an initial score of zero.

   **Correction:** replace the remaining scoring formula and eligibility description with supported outcomes and distinguishing fixtures. Keep the admissibility thresholds excluded.

3. **High — AI-166: lying-victim outcomes still describe effects the resolver does not perform.**
   **Addresses:** `0x0047e954–0x0047e98e`, `0x00473481–0x004734a6`, `0x00474d60`. [Specification](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:1173).

   “Finishing-off figure reports ‘finished’” followed by “plays the reaction” is unsupported. The lying paths bypass ordinary damage/stun resolution, and the corresponding reaction handler returns without installing a new reaction for lying postures. The document also says three special outcomes “carry no indications” immediately before correctly assigning both indications to the class-less outcome.

   **Correction:** remove the invented finishing-effect explanation and specify finishing effects separately from this resolver. Preserve the supported zero decision draws for lying victims, the class-less row figure’s one experience draw, and the distinction between attempted effects and actual state changes.

4. **High — AI-081, AI-190: the behavioural rewrite introduces incorrect callback values.**
   **Addresses:** `0x00410620`, `0x0040dcb0`. [Specification](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:591).

   Arrow launched, stone landed, adversary weak and after-combat injury must each map to **−2**, not 53–56. The call mappings must be: hey **36**, hint **39**, instruction **40**, look there **41**, coordinate **42**, report **43**, go to officer **44**, officer returned **45**, colleague returned **46**, patrol coordinate **47**, tower alert **37**, tower summons **38**, finish brawl **35**.

   **Correction:** repair the direct behavioural-to-script mapping and test it. Scope “values not listed … are not produced” to this dispatcher: the separate dispatcher produces **100–106**, already documented by VM-108. Replace the registry’s stale “event kinds 0..70” description.

5. **High — AI-093, AI-097, section 6.1: several purportedly completed native contracts remain wrong.**
   **Addresses:** `0x005790f0`, `0x00444440`, `0x004a5230`, `0x00413640`, `0x0057aa70`, `0x0041ac50`, `0x0057ad30`, `0x0044cae0`, `0x005790b0`. [Specification](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:1579).

   Native **177** does not rerun perception when there is no target. It updates the attentive setting and readiness; the conditional status update tests existing alert status and requires frame > 1. Native **218** rejects a proposed chief who already has a chief; “chief is on a patrol” is not that condition. Native **220** resets only an actor already in the default/on-duty state, not every actor regardless of state.

   **Correction:** state these effects and guards individually. Preserve the corrected soldier-only tests and native 176’s 16-bit storage. Do not extend the common validated-handle failure contract to 176: its body lacks that validation. Keep the ten explicitly excluded natives outside factual clearance; applying an assumed common failure rule does not clear them.

6. **High — AI-007, section 8: entry-point draw ordering remains materially false.**
   **Addresses:** `0x004801d0`, `0x00586270`, `0x00582560`, `0x0058bb80`, `0x0058ba60`, `0x004c6ef0`, `0x004b82e0`, `0x00577500`, `0x00472070`. [Specification](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:1730).

   Melee strike effects are queued; their resolution draws do not universally nest inside the striker’s update. Projectiles update in the element pass, not a separately specified projectile phase. Native 102 queues direct damage/stun processing; it does not invoke the ordinary defence/stun/experience-roll resolver. Section 6.1 also retains “no native below consumes a random number,” contradicting native 140’s verified transitive draw.

   **Correction:** distinguish synchronous delivery from queued effects, bind queued resolution to VM-215, and specify element-order interleaving and creation timing. Retain the proposed distribution-level deviation and the explicit withholding of integrated seeded/replay/hash clearance.

7. **High — AI-040–AI-042, AI-095, AI-164: snapshot ownership still omits consequential state and ordering.**
   **Addresses:** `0x004801d0`, `0x004808f0`, `0x00481050`, `0x00439080`, `0x00410de0`, `0x0040dbe0`. [Inventory](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:275).

   The additions address the previously named omissions, but the inventory still lacks the persistent strike-target list and sweep progress, the **registration order of multiple synchronisation waiters**, and the civilian periodic routine’s countdown. Per-waiter partner/waypoint data alone cannot reconstruct registration order.

   **Correction:** assign ownership or demonstrate reconstruction, including ownership of queued combat effects under the VM contract. Add restore cases for an unfinished sweep and multiple waiters, alongside the already requested fixtures. Integrated snapshot/replay clearance remains withheld.

8. **High — AI-170: recovery tests movement during the tick, not arrival at the target.**
   **Addresses:** `0x00471b00`, `0x00464230`. [Specification](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:1228).

   “Position equals the target position” misidentifies the comparison. It compares current position with the position saved before that tick’s update. An actor making no progress can satisfy it while still away from its destination.

   **Correction:** describe absence of movement during the tick and add a stationary-but-not-arrived fixture. The 64-frame stagger, no-adversary guard and recovery amount remain supported.

9. **High — AI-174, AI-040: the stun-floor description conflates coma with other restrictions.**
   **Addresses:** `0x0049b9c0–0x0049b9fa`, `0x0049f510`, `0x00471c00`. [Specification](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:1258).

   The player-coma override supplies stun **300** to the common setter; it is not merely a floor of 30. The common setter also has a conditional floor for an NPC marked “out,” dependent on the previous level and setter flag, which the specification omits.

   **Correction:** separate coma, tied/carried restrictions and the NPC condition. Preserve the supported consciousness thresholds, decay period and residual-counter phase; withhold the incomplete restricted-recovery contract.

10. **Medium — identity block: recoverable reader identities remain deliberately unrecorded.**
    **Address:** document-wide. [Identity block](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:44).

    Review 27 is now correctly bound to its session, commit and blob. However, saying the six fork identifiers are recoverable but “not reproduced here” does not resolve their individual exposure records.

    **Correction:** recover and bind each identity to its exposure, or reference an auditable private identity record with stable individual labels. Reserve “historical identity gap” for identities actually unrecoverable. Publication approval remains separate.

The automated asset check passes. The rewritten behavioural event wording and continued exclusion of the admissibility tables pass the manual expression filter; the stale private-number registry description should be removed.

**Implementation clearance is limited to individual contracts:**

| Area | Cleared | Still waiting |
|---|---|---|
| Perception | Category lifetimes, caching/staggering, corrected observer-ID fixture and fixed-value accumulation | General rounding contract, callback corrections and excluded geometry details |
| Hearing | Previously cleared noise, margin, deafness and staggering rules | Explicitly excluded oracle discrepancies |
| State machine | Reviewed individual transitions, reaction delays, fit-again order-state guard, corrected pause/pre-hook/door ordering | Corrected event/native contracts and section 9.2 exclusions |
| Rails | Reviewed cursor/opcode rules and cached-roll semantics; native 140 styles 0/1 | Snapshot ordering, excluded behaviour and ratification of invalid-style deviation |
| Orders | AI-185’s bounded movement-order contract | Context dispatch, cancellation, Ctrl queueing and gesture recognition |
| Combat rolls | Standing-victim attempt indications and conditional decision counts; class-less and lying-victim draw counts | Scoring, lying-effect description and integrated delivery/draw ordering |
| Energy | Costs, block recovery, recovery amount and cadence | Correct stationary guard and unresolved oracle timing |
| Knock-out | Punch scaling, resistance bypass, thresholds and residual decay phase | Coma/out restrictions, body effects and excluded scripted phase |
| Bow | Same-height equality fixture, ordinary penetration comparison and zero-draw principal-enemy immunity | Correct general aim rounding, exact interpolation boundaries and excluded target behaviour |

The ten natives **85, 87, 88, 90, 99, 128, 130, 228, 235 and 240** remain excluded from complete contract clearance. No clearance is granted for integrated snapshot/replay/hash expectations, exact figure admissibility or mission readiness.

**Verdict: fix-then-clear.**