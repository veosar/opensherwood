# Codex spec re-review 38: docs/original/spec-movement-animation-camera.md (movement, revision 4), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`. To be answered in the spec's next revision.

Reviewed [revision 4](C:/Users/przem/source/repos/opensherwood/docs/original/spec-movement-animation-camera.md) at `7328461`, blob `06d0fad5b45fe7106119dbf530e45e5338e89b16`, against the matching executable, decompilation and animation notes. Addresses below are virtual addresses, image base `0x400000`.

1. **High — ANIM-130, ANIM-134 — `00464b20`, `0054e730`, `00464230`, `00475bd0`.** The claimed same-update redispatch of idle/fidget is incorrect. The transition changes the action id and refreshes its identity; it does not play the replacement animation in that execution. Correct both claims’ timing. Also narrow “exactly two” and “no other action id changes by itself”: other actor-executor cases change their current action id without consuming a queued successor. The idle roll also excludes the wait-element case; §8’s unconditional draw-per-idle-cycle statement needs that qualification.

2. **Medium — ANIM-212; test 14 — `00464b20`.** “Some executor paths” still leaves the required turn count unspecified. Identify the applicable action cases and their termination condition. In particular, action cases 296–299 perform **at most two** turn/play operations, omitting the second when the first reports completion. Specify these observable counts, including a first-operation completion fixture; do not require two facing changes unconditionally.

3. **Medium — ANIM-003; §4.2 — `005b8050`, `005b86b0`, `005b7820`.** The asserted four exceptions are still incomplete. Exact-equality immediate completion performs **no timer step**; frozen and missing-animation entries also return before stepping. Movement mode 6’s extra step requires both a facing mismatch **and an already nonzero displacement**. Qualify the ordinary accounting accordingly, and restrict `hold + 1` display duration to modes that actually follow that timing rule.

4. **High — ANIM-035, ANIM-120, ANIM-209; test 2 — `005b86b0`, `00560460`.** “Mode 5 completes only by exhausting its records” is false. Its completion paths also depend on the animation completion signal and arrival/look-ahead conditions; status 3 can occur with records remaining, including on admission. Keep the corrected exclusion from exact-equality immediate completion, but remove the unconditional admitting-status-1 and exhaustion-only assertions. Either establish the remaining mode-5 completion contract or consistently exclude it through `WaypointRecords`.

5. **Medium — ANIM-042, ANIM-043; §4.3 — `005b7820`, `005bdbf0`.** ANIM-043 retains the old sentence saying terminal-marker exit takes the ordinary wrap, immediately before saying it returns without wrapping. Delete the old assertion. Also remove the blanket uncertainty over whether any consumer reads this out-of-range state: the following ordinary step demonstrably reads its hold before wrapping. What remains unknown is the value obtained and its consequences.

6. **High — ANIM-005; test 9 — `004ca410`, `00585320`, `00582620`, `00582560`, `00585b70`.** The completion-before-write correction is right, but test 9 draws the wrong unconditional consequence: the next sequence level need not observe the flag already set. Completion can dispatch immediate successors synchronously **before** the flag write; deferred successors execute later. Separate those cases in the acceptance contract.

7. **High — ANIM-240; §4.2, §12 — `00561040`, `00608400`.** The newly cleared guard list adds an unsupported **same projection area** requirement. The common scan checks layer and sector, without that additional area comparison. Remove it. Also explicitly limit the directional-product threshold to character candidates: the target-family branch uses its blocking answer and position containment without that product test. Preserve the eligibility/query exclusions and the withdrawal of the speed inference.

8. **High — ANIM-334, ANIM-331; §6, test 18 — `004cdfc0`, particularly `004ce74c–004ce79e`, and `004cec60`.** Follow processing does not invariably end the update after a burst step. When boundary clipping leaves a **zero delta**, processing falls through to zoom and scroll. Document that exception and test it. Separately, zoom completion can complete and clear the installed camera element while leaving the scroll destination active; it does not itself stop scroll progress. Correct §6’s conflation of camera-element completion with movement suspension.

9. **Medium — ANIM-331; test 19 — `00571270`, `00571330`, `004cdfc0`.** Test 19 still requires native 18’s first step to be 2 px without the zero-leftover-speed precondition. Its native-19-with-1.0 case also predicts a stationary next update unconditionally: the ramp length becomes zero, but a nonzero retained scroll speed still supplies movement. State isolated preconditions for both traces, including reaching the scroll stage without clipping or prior arrival. The **first-refresh-only reset** itself is correct.

The expression filter passes this re-review: ANIM-134/212 now state transitions and ordering rather than implementation placement. The heuristic asset check also passes. The remaining objections above concern correctness and completeness.

| Area | Clearance now | Still waiting |
|---|---|---|
| Animation clock and modes | Established timer arithmetic, reset rules, mode-10 preservation and mode-14 valid-entry bounds. Modes 6/7 and early-marker arithmetic only with supplied marker values. | Findings 1–6 as applicable; file-field mapping, initial flags, overrides, out-of-range consequences, admission and speech exclusions. |
| Movement and turning | Per-frame displacement, turning scale/floor, turn variants, exact-equality immediate-completion predicate. | Exact repeated-operation counts, mode-5 completion/records, unresolved initial state and footstep selector. |
| Collision and proximity | Directional comparison itself; failure-region invalidation, approximately 0.49-pixel half-extents, inclusive containment, resets and counter threshold. | Finding 7; full eligibility/query geometry, reactions, sliding and remaining arrival branches. |
| Carts | Previously cleared integration, program effects, conditional draws, bonds and interaction traversal; displacement remains active during freeze-all. | Explicit `CartInitialState`, sub-sprite selectors/consumer, program container/two instructions and hit-severity selector. |
| Camera | Sign-dependent small-map conversion, both cleared direction latches, native writes and first-refresh reset. | Findings 8–9; exact ramp entries, edge trigger, zoom timing and complete follow traces. |
| Drawing order | Previously cleared ordering facts and six eligible ground-mark ageing events: removal on draw 11/12 for even/odd starts. | Scenery comparison boundaries/family coverage, mask integration and wide-mode provenance. |

Section 12 correctly restores outstanding amendments against the pinned siblings. The 28 exclusions, separate delegate task references and explicitly unresolved session provenance are appropriately recorded. Publication approval remains separate.

**Verdict: fix-then-clear.**