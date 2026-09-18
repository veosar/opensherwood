# Codex spec re-review 42: docs/original/spec-ai-combat.md (AI and combat, revision 6), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`. To be answered in the spec's next revision.

Reviewed revision 6 at commit `0347564160cfbf585eb934a6074c1f6502ec39cb`, blob `0cd519412b987b8e726a9f2c028a7ecf14da5ce2`, against the decompilation, AI notes and targeted disassembly. The executable SHA-256 matches. Addresses below are virtual addresses, image base `0x400000`.

Review 40 findings **1, 2, 4, 5, 6 and 8 are closed**. The corrected arithmetic reproduces **15 / 159**; friendly-target scoring reproduces **96 with two counted targets**. Two findings remain:

1. **Medium — AI-166: posture 10’s outcome condition is reversed.**  
   **Addresses:** `0x0047e954–0x0047e98e`, `0x00473481–0x004734a6`, `0x00474d60`.  
   (spec line 1238).  
   Posture 11 always reports the special “lying” outcome. Posture 10 reports **“nothing” for the two finishing-off figures and “lying” for other figures**, contrary to revision 6.  
   **Correction:** reverse the posture-10 condition here and in section 9.1. Add distinguishing outcome fixtures for an ordinary row figure and a finishing-off figure. Preserve the supported zero decision draws, absence of damage/stun resolution and absence of an ordinary hit reaction.

2. **High — AI-007, section 8: the obsolete draw-order and native-102 claims survive.**  
   **Addresses:** `0x004c6ef0`, `0x00577500`, `0x00472070`, `0x00582560`, `0x0058ba60`.  
   (spec line 1799).  
   Section 8 still opens with “drawn in actor-list order” and still says “native 102’s hit resolves with the strike rolls.” These contradict its later corrected account and section 6.1.  
   **Correction:** replace the opening ordering claim with the script-phase, element-pass and queue-processing account; remove native 102 from the strike-roll examples. Its queued direct step does not perform AI-166’s defence, stun or experience decisions. The engagement-generated attack order is now correctly classified as queued; newly created elements’ first-update timing remains explicitly unresolved.

The automated asset check and manual **expression filter pass**. The excluded admissibility tables remain excluded; the new corrections prescribe behavioral results and necessary restore state.

| Area | Cleared | Still waiting |
|---|---|---|
| Perception | Reviewed lifetimes, caching, staggering, posture rounding and fixed-value fixtures | Excluded geometry, unidentified gates and unverified arithmetic boundaries |
| Hearing | Noise, margin, deafness and staggering rules | Excluded oracle discrepancies |
| State machine | Reviewed transitions and ordering; corrected callback payload/value contract and native guards | Section 9.2 exclusions, including reset details, menacing, tower-guard and civilian branches |
| Rails | Reviewed cursor/opcode rules, cached rolls, waiter order; native 140 styles 0/1 | Restore fixtures, excluded behavior and invalid-style deviation ratification |
| Orders | AI-185’s bounded movement-order contract | Context dispatch, cancellation, Ctrl queueing and gestures |
| Combat rolls | Contribution and friendly-target scoring; reviewed indications/counts; queued engagement and pending sweep-candidate ownership | Findings 1–2; exact admissibility remains excluded; cadence unresolved |
| Energy | Costs, recovery amount/cadence and movement-based guard, including blocked actors | Oracle timing discrepancy |
| Knock-out | Punch scaling, thresholds, decay phase, coma 300 and conditional NPC-floor predicate | Setter-flag callers, body effects and scripted phase |
| Bow | Aim boundaries and fixtures; penetration comparison and immunity draw count | Exact interpolation boundaries, record/flag selection and excluded target behavior |

Natives **85, 87, 88, 90, 99, 128, 130, 228, 235 and 240** remain excluded from complete contract clearance. Integrated snapshot/replay/hash behavior awaits the restore fixtures and RNG-policy ratification.

**The cleared areas do not yet form an implementable whole for playing a mission.** They support individual contracts, but the excluded state transitions, order dispatch and combat behavior still interrupt that whole. Mission readiness also requires integration with navigation/scripts and a canonical-input completion trace. Publication approval remains separate.

**Verdict: fix-then-clear.**