# Codex spec re-review 40: docs/original/spec-ai-combat.md (AI and combat, revision 5), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`. To be answered in the spec's next revision.

Reviewed revision 5 at commit `5349b4f49a8e743ddbcbf816fa0d4dd4715300df`, blob `b78704dd5afebdd6122888e689d24eb9ca22a0b4`, against the decompilation, AI notes and targeted disassembly. The executable SHA-256 matches. Addresses below are virtual addresses, image base `0x400000`.

1. **High — AI-008, AI-066, AI-068: perception still omits a rounding boundary.**
   **Addresses:** `0x00489984–0x00489a90`. [Specification](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:485).
   The posture-scaled product is also stored as single precision before returning. It does **not** remain extended until entry storage.
   **Correction:** reconcile all three arithmetic passages. Distinguishing fixture: stored fade `0.3333333432674408`, posture factor `1.5`, W = 80. Rounded posture result gives the documented **15 / 159** contributions; retaining the extended product gives **16 / 160**. The corrected aim boundaries and downhill fixture pass.

2. **High — AI-151: the friendly-target exception counts friends rather than skipping them.**
   **Addresses:** `0x00485646–0x00485686`. [Specification](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:1115).
   For a soldier with nonzero hesitation, the exception suppresses disqualification. The friend then contributes to the score, receives the nonzero-contribution bonus and counts toward the required target count.
   **Correction:** replace “friends are simply skipped” and “remaining opponents.” Add a fixture with one enemy and one friend, each contributing 18: the unpenalised sum is **96**, with two counted targets. The corrected individual contribution rule and **18 / 48** fixture pass.

3. **Medium — AI-166: the lying-victim description remains contradictory.**
   **Addresses:** `0x0047e954–0x0047e98e`, `0x00473481–0x004734a6`, `0x00474d60`. [Specification](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:1201).
   Step 1 still says finishing figures affect a lying victim and report “finished.” Step 5’s “reports whether” description is also inaccurate: posture 11 returns the same special outcome regardless of figure.
   **Correction:** remove the surviving finishing-effect claim and the figure-discriminator interpretation. Retain the supported absence of ordinary damage/stun resolution, zero decision draws and no installed ordinary hit reaction; keep finishing effects separately excluded.

4. **High — AI-081, AI-190: the new callback fixture supplies the wrong source.**
   **Addresses:** `0x0047bb60`, `0x00410620`. [Specification](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:1760).
   The arrow-launched event carries the shooting actor as an element payload. Its callback is therefore **(shooting actor, −2)**, not `(null, −2)`. An unmapped event value does not erase its payload.
   **Correction:** repair acceptance test 15 and distinguish payload type from event-value mapping. The revised numeric mappings, including calls 35–47 and the separate 100–106 dispatcher, pass.

5. **High — section 6.1, native 102: the accepted-target contract still invents sequence membership.**
   **Addresses:** `0x00577500`, `0x00570850`, `0x004d8460`. [Specification](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:1612).
   Validation searches the level’s registered element table and checks the actor-family type. It does not require the target to be declared in a current sequence.
   **Correction:** remove that prerequisite; describe the level-table/type guard. Preserve queued direct processing without AI-166’s decision rolls.

6. **Medium — AI-097, native 218: duplicate membership does not trigger re-linking.**
   **Addresses:** `0x0057ac3d–0x0057acae`, `0x0041ac50`. [Specification](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:1624).
   Once the guards pass, an actor already present in the membership list is neither appended nor re-linked; the rebuilding operation runs only after a new insertion. An actor already assigned a chief fails an earlier guard.
   **Correction:** replace “re-linked rather than duplicated.” The corrected proposed-chief guard passes, as do natives 177 and 220 and native 176’s validation exception.

7. **High — AI-007, section 8: delivery and draw-order claims remain inconsistent.**
   **Addresses:** `0x0047c600`, `0x005866a0`, `0x0058a940`, `0x00582560`, `0x0058ba60`, `0x00577500`, `0x00472070`. [Specification](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:1780).
   Section 8 still says native 102 resolves with strike rolls, contradicting its corrected contract. It also classifies the engagement-generated attack order as synchronous, although that order is queued through the sequence machinery.
   **Correction:** remove native 102’s strike-roll claim and place the generated attack order under VM-215 delivery. Reconcile section 2.2’s blanket actor-list ordering with the corrected element-pass/queue-drain account. Keep newly created elements’ first-update timing explicitly unresolved.

8. **High — AI-040–AI-042, AI-164: the snapshot addition misidentifies sweep state.**
   **Addresses:** `0x004801d0`, `0x004808f0`, `0x00481050`. [Specification](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md:326).
   The sweeping strike retains candidates **still awaiting a strike**, removing each when its effect is queued. A list of already-struck targets plus the current angle cannot reconstruct that initial candidate selection.
   **Correction:** require preservation of pending candidate membership and delivery order, plus sufficient sweep state to reproduce its remaining sector and termination. Update the unfinished-sweep restore case accordingly. The waiter-registration order, civilian countdown and queued-effect ownership additions pass.

The automated asset check and manual expression filter pass. The admissibility tables remain properly excluded. The six delegated-reader identities now have individual exposure bindings.

Clearance remains limited to individual contracts:

| Area | Cleared | Still waiting |
|---|---|---|
| Perception | Previously cleared lifetimes, caching, staggering and fixed-value fixtures | Posture-result rounding; excluded geometry |
| Hearing | Previously cleared noise, margin, deafness and staggering rules | Excluded oracle discrepancies |
| State machine | Reviewed transitions and update ordering; corrected callback values; natives 177/220 | Callback source fixture, remaining native corrections and section 9.2 exclusions |
| Rails | Reviewed cursor/opcode rules, cached rolls, waiter-registration ordering; native 140 styles 0/1 | Restore fixtures, excluded behaviour and invalid-style deviation ratification |
| Orders | AI-185’s bounded movement-order contract | Context dispatch, cancellation, Ctrl queueing and gestures |
| Combat rolls | Individual contribution rule and 18/48 fixture; reviewed indications and decision counts | Friendly-target scoring, lying-victim wording, delivery ordering and sweep ownership |
| Energy | Costs, recovery amount/cadence and movement-based guard, including blocked actors | Unresolved oracle timing |
| Knock-out | Punch scaling, thresholds, decay phase, coma 300 and stated conditional NPC floor | Setter-flag callers, body effects and scripted phase |
| Bow | Corrected aim boundaries and both fixtures; penetration comparison and immunity draw count | Exact interpolation boundaries and excluded target behaviour |

Natives **85, 87, 88, 90, 99, 128, 130, 228, 235 and 240** remain excluded from complete contract clearance. Integrated snapshot/replay/hash behaviour, exact figure admissibility and mission readiness remain uncleared. Publication approval remains separate.

**Verdict: fix-then-clear.**