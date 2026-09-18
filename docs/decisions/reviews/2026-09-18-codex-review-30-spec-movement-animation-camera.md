# Codex spec re-review 30: docs/original/spec-movement-animation-camera.md (revision 3), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`.

Reviewed revision 3 at `03b413e`, blob `1b65f0bafaa59f6ac4dd704fe7a0c6e4d841df03`, against the matching executable, decompilation and animation notes. Remaining findings:

1. **Blocker — expression filter — ANIM-134, ANIM-212 — 00464b20, 005b86b0.** “Must not decide what plays next” and “must therefore drive turning from the same place as the play call” still prescribe implementation organisation. Describe the required action transitions and ordering/count of turn operations instead. The removed cache and placement details are resolved; the heuristic asset check passes, but these prescriptions prevent expression clearance.

2. **Blocker — clearance boundary — §4.2–4.3; ANIM-003, 012, 034, 043, 020–023 — 005b7820, 005b86b0, 005bdcd0, 004ab7e0.** There are now exactly **26 exclusions**, but the boundary remains inconsistent. Section 4.2 again clears “one timer step per play call” without its documented exceptions, and clears the entire mode table despite marker-dependent exclusions. Separate arithmetic/state-transition facts, tested with supplied synthetic marker values, from their unresolved file-field dependencies. Cart program/progress and rattle initialisation still have neither established defaults nor an explicit initialisation exclusion. Include these in the boundary and snapshot requirements.

3. **High — freeze-all scope and recorded-element ordering — ANIM-005; §6, §11 — 00576c30, 00577df0, 004ca410, 004ab720, 004ab7e0, 00464b20.** The animation-player gate and NPC AI gate are real, but “stops every movement” is too broad: cart displacement and executor-side turning are not stopped by those player-entry checks. Limit the claim to the operations actually gated. Also document that the recorded freeze element is marked completed **before** its flag write; native 139 writes immediately. Do not imply identical ordering around sequence advancement.

4. **High — admitting-update completion — ANIM-035, ANIM-120; test 2 — 005b86b0, 005fc0b0, 00560460.** Immediate completion is not the general arrival predicate already being satisfied. It requires **movement mode other than 5 and exact equality of the supplied destination and current screen position**. A merely within-tolerance destination does not establish this case. Arrival after displacement can also override started status on the admitting update. Specify these conditions and distinguish ordinary movement arrival from mode-5 completion handling.

5. **High — mode boundaries — ANIM-036, ANIM-042, ANIM-043; tests 6 and 8 — 005b7300, 005b7820, 005bdbf0.** Three errors remain:
   - Mode 7’s rotation-ending step leaves `m + 1` and returns without wrapping. A terminal marker therefore produces a one-past-end state; the following ordinary step can read its hold before wrapping.
   - Mode 14 cannot leave a valid clip merely because its entry frame is odd. Its guard keeps every initially valid frame in range; an already-invalid entry is a different case.
   - Mode 10 preserves the frame when entered without an action reset; it does not invariably leave frame zero.

   Correct the descriptions, traces and out-of-range exclusion.

6. **High — failure-counter reset state — ANIM-021, ANIM-241; test 13 — 00563ea0, 00607d90, 00563ed0, 00608400.** Both resets are confirmed, but the new-action reset also **invalidates the progress region**. The next progress check establishes a region and leaves the counter at zero, rather than counting its first in-region failure. Specify region validity, the approximately 0.49-pixel half-extents and inclusive containment boundaries; preserve validity in snapshots. Otherwise the refusal update remains ambiguous despite naming both resets.

7. **High — proximity vector and eligibility — ANIM-240 — 00561040, 0055fc20, 005fc7c0, 005fc660.** The directional-product comparison is corrected, but the added assertion that it depends on the mover’s **speed** is unsupported. The operand is the movement direction, ordinarily normalised, not the published speed-scaled velocity. Remove that inference. The pinned NAV-150 also does not contain the claimed detailed query rectangle and eligibility specification; either supply the missing guards or clear only the comparison itself.

8. **High — native 19 ramp trace — ANIM-331; §6, test 19 — 004cdfc0.** The prose correctly says the **first** ramp refresh resets the index for step length 1.0, but the interface table and test incorrectly require resetting on every later update. That refresh replaces the length with ramp entry zero; the following refresh advances normally. Correct both contradictory statements. Native 18’s asserted first 2-pixel step also still needs an explicit zero-leftover-speed precondition.

9. **High — camera interaction precedence — ANIM-330, ANIM-331, ANIM-333; tests 18–21 — 004cdfc0, 004db8e0.** Preserving a lock or zoom request does not guarantee concurrent scroll progress. Follow processing precedes them and can bypass the remaining camera work; zoom completion can complete the installed camera element before scroll arrival. State these interactions and add the requested contested-state cases. Test 21 must also separate key scrolling from an active script destination: that destination gates key-scroll commands.

10. **Medium — small-map acceptance contract — ANIM-320, ANIM-329; test 16 — 005758d0.** ANIM-329’s five-step description matches the instructions, but the unconditional interval-clamp shorthand elsewhere does not. On an undersized map, a **non-negative** truncated component can become a negative upper-bound result without triggering fallback. Test negative, zero and positive truncated components separately, on both axes; preserve the conditional fallback instead of substituting a conventional clamp.

11. **Medium — camera initialisation exclusion — ANIM-023, ANIM-323; §4.3 — 004bef2f, 004bef35 within 004be6d0.** The claim that the constructor does not initialise the x direction latch is false. It clears **both** direction latches. Remove that item from `AnimInitialState`; the stated fresh-level right/down versus left/up asymmetry is supported.

12. **Medium — ground-mark draw count — ANIM-370 — 0051c0d0.** The final sentence still unconditionally says “12 draws.” Starting with an eligible even-counter draw removes the mark on draw **11**; starting odd removes it on draw **12**. Keep six eligible ageing events as the invariant and correct this remaining fixed-duration statement.

13. **High — sibling amendments falsely marked absorbed — §12; ANIM-120, ANIM-240, ANIM-242, ANIM-329 — 00464b20, 00561040, 00560460, 005758d0.** Against the explicitly pinned commits:
   - VM-231 still says walks arrive within “radius + 5” and does not specify native 51’s deferred freeze admission.
   - VM-219’s small-map fallback omits the negative-component condition.
   - NAV-150 still describes intersecting boxes and references obsolete movement claim IDs.

   Restore these as outstanding amendments or pin revisions that actually contain the corrections. “Already absorbed — no action” is inaccurate.

14. **Medium — exposure traceability — identity block — document-wide.** The delegated readers’ missing identities are now acknowledged, but they remain untraceable. Record each delegate separately with any retained handle or archival reference. If recovery is impossible, retain this explicitly as unresolved provenance; attributing their exposure to the parent does not establish their identities.

**Cleared as facts:** the unaffected portions of §4.2, subject to §4.3; specifically ANIM-329’s ordered conversion, the gated player/NPC operations, play-only status precedence, the existence of both failure-counter resets, and ANIM-340’s **1.0 eight-byte double**, inclusive burst threshold and strict speed-replacement threshold.

**Waiting:** the corrections above and §4.3’s excluded behavior, with the x-latch exclusion removed and cart initialisation and mode-7 out-of-range handling accounted for. Publication approval remains separate.

**Verdict: fix-then-clear.**