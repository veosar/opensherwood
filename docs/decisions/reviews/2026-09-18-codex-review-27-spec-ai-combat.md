# Codex spec re-review 27: docs/original/spec-ai-combat.md (revision 3), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`. Answered in the spec's next revision.

Reviewed revision 3 at commit `e7b2cd45ec1dd170101be64e68f2d5517efbdbfd`, blob `c9040a2a406e8525fbd6aae32241b81fe0c72a6c`. The executable SHA-256 matches. Reviewer session: `01a0b406-a088-76b0-b80e-f5d4a5abd73d`, spec-reviewer role only.

1. **Blocker — AI-008, AI-066, AI-176: the arithmetic contract still omits consequential rounding stages.**
   **Addresses:** `0048996f–0048999a`, `00489e30`, `0047b947–0047b99e`. [Specification](/C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:133).

   “Every intermediate held in a precision of at least 64 significant bits,” AI-066’s “All arithmetic is single precision,” and section 8’s `f64` policy are different contracts. The corrected 15/159 sight contributions and 54/108/60 reaction delays are supported, but do not establish general equivalence.

   Aim validity stores the scaled y displacement, tangent result, and squared reach as singles before subsequent operations. For example, same-height reach **204**, dx **0**, dy **153.38345336914062**: the unrounded scaled y is approximately **203.99999956346073**, but its stored single is **204**, so the original rejects the shot at equality. The current formula can accept it.

   **Correction:** specify the consequential rounding boundaries, reconcile AI-066, and distinguish the chosen deterministic arithmetic from demonstrated original equivalence. Add boundary fixtures; withhold exact arithmetic-dependent behaviour until settled.

2. **Medium — AI-068, AI-069: acceptance test 1’s observer-ID variant has an incorrect initial condition.**
   **Addresses:** `0048c700`, `00488f60`, `00487d00`. [Test](/C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:1557).

   “A soldier with id 1 evaluates on odd frames but the contributions and frames above are unchanged because the cached value is identical” assumes a populated cache. A newly added entry starts with zero.

   **Correction:** explicitly prepopulate the cache, or expect the first ordinary evaluation on frame 1, shadow on frame **8**, and sighting on frame **67**, under the other stated conditions and without forced reevaluation.

3. **High — AI-151, AI-040: figure scoring and penalty decay remain wrong.**
   **Addresses:** `00485324–00485517`, particularly `00485496–004854c6`.

   The score subtracts **three times** the stored usage penalty, not the penalty itself. The nonzero-hesitation circle exception receives its +500 bonus **without that subtraction**. Penalties decay when the scoring pass is entered, not on every chooser invocation; failed attack gates can return without decay.

   **Correction:** state these distinctions and the equal-score preference for the earlier eligible row. The revised circular eligibility exception and conditional reactive skill roll themselves are correct.

4. **High — AI-166, AI-007: the conditional experience contract still misidentifies outcomes.**
   **Addresses:** `0047e806–0047e951`, `0047e90c–0047e944`, `004734a1–00473505`, `00471e50`, `00471ed0`. [Resolution](/C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:1121).

   A victim without a class block returns the same result value used for damage-and-stun; the generic handler can therefore consume the trailing experience roll for a qualifying row figure. This is **one decision draw**, with no defence or stun draw. Section 9.2(15) need not leave it unresolved. Review 22’s blanket statement about a “rejected result” was also insufficiently precise.

   Furthermore, qualifying stun sets the resolution’s stun indication even if health-zero or immunity prevents an actual stun increase. “No stun applied” therefore does not reliably imply that experience is skipped. “No effect … no reaction” is also too broad: certain displacement reactions occur before that exit.

   **Correction:** define draw counts from the supported outcome conditions, distinguish attempted effects from state changes, remove the invented separate no-class rejection outcome, and replace the contradictory “finished” explanation. Keep intervening sound/reaction draws separately accounted for.

5. **High — AI-082: the fit-again state-domain error remains verbatim.**
   **Address:** `00410620`. [Passage](/C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:575).

   The specification still says **“while its current action id is 7”**, although the guard reads **order state 7**.

   **Correction:** change the domain in the operative claim, not only in the revision-history summary.

6. **High — AI-178: principal-enemy immunity bypasses the penetration roll.**
   **Addresses:** `004a690e–004a6930`, `004a5690`, `004a18c0`. [Passage](/C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:1260).

   “An immune … target goes through case (b)’s roll like any soldier” is false. The projectile checks the principal-enemy predicate and exits before calling the penetration predicate.

   **Correction:** specify **zero penetration draws** for this immunity path and correct acceptance test 12. The civilian permission-only path and ordinary class-based penetration comparison are supported.

7. **High — AI-050, AI-003: the revised execution order and local-pause description remain incomplete.**
   **Addresses:** `0048a980`, `0040dbe0`. [Execution contract](/C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:344).

   The pre-tick hook is itself suppressed by the local-pause flag. The door-wait/reset check occurs **after** perception, the AI update, and deafness processing—not before them. The civilian periodic routine also remains outside the later pause/lock guard and can consume randomness and produce a remark.

   **Correction:** describe those observable orderings and exceptions. The timer check ordering, deadline advancement, and half-open lock example are supported, but do not justify saying all AI work stops during an actor-local pause.

8. **High — AI-040–AI-042: the snapshot inventory is still insufficient.**
   **Addresses:** `00410620`, `0048a980`, `00410010`, `00486fa0`, `005794b0`, `00579520`. [Inventory](/C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:276).

   Adding usage penalties and the cached rail roll does not cover the pending/deferred events and their ordering, lock/queue flags, rail execution position and suspension state, creation-frame stagger, custom native values, or the perception collapse increment and tracking target.

   **Correction:** give each authoritative quantity explicit snapshot ownership, or a demonstrated reconstruction rule. Distinguish the cached roll’s value from whether it remains available. Withhold integrated snapshot/replay clearance until the inventory and restore fixtures cover these cases.

9. **High — AI-007, section 8: the proposed RNG policy contains a false exemption and does not establish complete draw ordering.**
   **Addresses:** `005780d0`, `0041bd20`, `00419cc0`, `00419c80`. [Policy](/C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:1660).

   “The natives of section 6 (none draw)” is false transitively: native 140 can inspect a rail program, causing its cached roll to be generated. Actor-list order alone also does not define interleaving with script-native calls, synchronous cross-actor events, player combat, and projectile processing.

   **Correction:** specify stream ownership and ordering across those entry points, including nested draws. Retain distribution-level equivalence as a proposed deliberate deviation. Replace “nothing … depends on the choice except the exact sequence of outcomes” with an explicit withholding of integrated seeded behaviour, replay, and hash expectations until ratification.

10. **High — section 6.1, AI-093, AI-097: native contracts remain materially incorrect or incomplete.**
    **Addresses:** `00579b10`, `005790b0`, `005790f0`, `0057ad30`, `0044cae0`, `00571b80`, `0057aa70`.

    Native **89 admits player characters**: its human-type test is broader than the NPC-only test used by 134/135. Natives **176, 177, and 220 require soldiers**, rather than arbitrary NPCs. Native 176 stores a 16-bit company value. Native 220 can reset an actor already on duty even when no alert rail exists, contradicting “no alert rail … no effect.” Native 160 still says only “distance,” without its location requirements, metric, or truncation contract. Native 218 omits rejection conditions, including self-subordination.

    **Correction:** provide per-native accepted types, coercions, results, and failure effects with direct provenance, or exclude the unfinished entries. A common inferred contract cannot replace these distinctions.

11. **High — AI-093, native 140: out-of-range walking styles are not passed through verbatim.**
    **Addresses:** `00578122–0057814e`, `0041bd20`.

    Both passages asserting **“other values verbatim”** are contradicted by the instructions. Outside styles 0 and 1, the written word is derived from the actor-address argument, not the supplied style.

    **Correction:** clear only styles 0/1 as currently understood. Document the address-dependent invalid-input behaviour and select an explicit deterministic deviation before supporting other values.

12. **High — expression filter, AI-081: the internal event-name catalogue still crosses the boundary.**
    **Addresses:** `0041f540`, `00410620`. [Catalogue](/C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:554).

    The passage **“the event kinds, numbered as the AI uses them internally”**, including **“59..61 my talk 1..3, 62..64 your talk 1..3”** and **“68 my talk 0, 69 your talk 0”**, reproduces the organisation and naming pattern of the diagnostic event catalogue. These private numbers are not required by the callback interface; those events are exposed as −2.

    **Correction:** retain independently named behavioural events and map them directly to script-visible values. Keep unnecessary private numbering and diagnostic naming in `re/`. Section 9.3’s removal of the individual-constants escape hatch and AI-140’s capability wording pass.

13. **Medium — identity block: exposure is described, but individual identities remain missing.**
    **Address:** document-wide. [Identity block](/C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:39).

    “Their session ids were not recorded separately” and “the reviewer’s own session id is not known” acknowledge rather than resolve finding 16.

    **Correction:** record recoverable fork task/session identifiers and bind them to their exposure records; explicitly retain any unrecoverable historical identity gap. Bind this review to the session, commit, and blob stated above. Publication approval remains separate.

**Cleared for implementation as individual contracts:** the previously cleared serialized layouts and profile meanings; the corrected event-to-callback mapping, including −2 delivery, payload/context and suppression; AI-100’s reaction-delay calculation and corrected examples; native 134/135’s player rejection and action-127 preservation; AI-170’s out-of-combat recovery timing; AI-173’s player-punch difficulty scaling; AI-174’s residual-counter phase; and AI-176’s geometric rules subject to the arithmetic correction above.

**Waiting:** the affected contracts in findings 1–13; complete snapshot/replay integration and RNG-policy adoption; section 9.2’s remaining exclusions; exact figure admissibility excluded by section 9.3; and mission-readiness claims. An explicitly recorded qualitative figure-choice assumption does not constitute original-equivalent clearance.

The automated asset check passes. Manual expression clearance remains blocked by finding 12.

**Verdict: fix-then-clear.**