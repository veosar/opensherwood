# Codex spec re-review 26: docs/original/spec-navigation.md (revision 3), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`. Answered in the spec's next revision.

Reviewed [revision 3](C:/Users/przem/source/repos/opensherwood/docs/original/spec-navigation.md), commit `d280c1f`, blob `710e01793e83a327d31451ae1f5c9ba740a4c04a`, against the decompilation and navigation notes. Addresses below are virtual addresses, image base `0x400000`.

**Finding 9 is withdrawn.** The analyst’s boundary ownership is correct. The call site `0x005541db–0x005541e5`, subtraction `0x00553a72–0x00553a81`, and both cited flag-test paths establish revision 3’s inequalities. For bit 1 at `(94,96)`, `(94,120)` passes and `(50,96)` fails.

Review 20 findings **4, 10, 11, 12, 14, 15, 17 and 20 are resolved**. The remaining defects follow.

1. **Blocker — NAV-111/141 — expression filter still fails.**
   **Addresses:** `0x004f5890`, `0x00556490`, `0x005547c0`.
   Offending passages include “examines the box’s corners in the order …”, followed by per-corner mutations and “later corners are measured after that translation”; “its recorded predecessor the edge that produced that least cost”; “makes `A` open again”; and “`p[i-1]` is removed and the same `i` tested again”. These still prescribe recovered bookkeeping and procedural organisation.
   **Correction:** specify separation outcomes, path-selection requirements and observable waypoint ordering without prescribing those mutations or loop mechanics. NAV-144’s revised forward sequences resolve the earlier direction error; that does not clear these other passages.

2. **High — NAV-150(b), §5, §8.7 — proximity rectangle remains incorrect.**
   **Addresses:** `0x0056117c–0x00561269`, `0x005613a6–0x00561497`, `0x00608400`.
   The query is a square centred on the proposed next position, with half-extent `radius + 60` on **each coordinate axis**. It is not the rectangle between `position ± r·dir`, which degenerates to a line for axis-aligned movement. The object branch also retains the common displayed, carried-element and layer/sector exclusions surrounding both branches.
   **Correction:** describe the actual centre and axis extents, retain the common exclusions, and then distinguish character-only eligibility and dot-product tests. Replace the dangling `ANIM-205a` reference: that claim does not exist in the pinned movement revision.

3. **High — NAV-058; §§8.2–8.3; test 14 — original restore order is unsupported.**
   **Addresses:** `0x00555a10`, `0x00556260`, `0x00556470`, `0x0055aa60`.
   The restore routines reapply availability to existing lists; they do not reconstruct canonical file order. Even initial filtering followed by restoration can append a previously unavailable node after surviving nodes. Consequently “the original’s own order after a load” is not established.
   **Correction:** remove that asserted equivalence and derive the actual load transition. Canonical file order may remain an explicitly approved OpenSherwood deviation. If that deviation is rejected, restoration must preserve the required ordering state; rebuilding from state words alone is insufficient.

4. **High — NAV-090/147; §8.1 — proposed scheduling contract is not yet determinate.**
   **Addresses:** `0x005532a0`, `0x005546e0`, `0x004d28e0`.
   The budget has neither a value nor defined accounting units. “Large enough that no retail map reaches it” has no supporting measurement. Patch delivery at the original tick “or later” leaves timing discretionary. Cancellation followed by another request or patch also lacks an exact dispatch rule. Section 11’s “one search per tick” contradicts immediate patch-triggered recomputation.
   **Correction:** fix budget accounting and exhaustion timing; specify exact dispatch/delivery results for cancellation, repeated patches and patches with only queued work. Reconcile section 11. Keep this policy pending maintainer approval.

5. **High — NAV-140/146/171/200; §8.2 — snapshot coverage still omits crossing state.**
   **Addresses:** `0x00467a50`, `0x0046a900`, `0x00469770`, `0x0046a000`, `0x004d23d0`.
   NAV-171/200 record an active door and crossing side, but §8.2 does not include them. Its reference to `ANIM-021` covers position state, not those crossing associations or the action queue.
   **Correction:** explicitly cover active crossing identity/side and the queued/current movement actions with their progress, or cite a cleared contract covering them. State that completed requests retain all inputs needed for patch recomputation and delivery. Serializing completed waypoint results, rather than recomputing them on restore, is otherwise the correct repair.

6. **High — NAV-121/141/143; §8.5 — precision claims and deviation bounds remain unsound.**
   **Addresses:** `0x004f9ff5–0x004fa01f`, `0x004fa6f0`, `0x00554360`, `0x00556839–0x0055688c`, `0x005581a0`.
   Queue ordering compares stored single-precision scores; not every named comparison uses an unrounded x87 intermediate. The blanket “80-bit intermediates” claim also needs the relevant precision-control evidence. “Results may differ … only” within approximately `1e-6` relative is unproved and overlooks geometric decisions and accumulated effects.
   **Correction:** describe the evidenced rounding boundaries accurately, remove the unsupported bound, and supply near-tie and geometric-boundary acceptance cases for the proposed `f32` policy. Exact symmetric ties do not validate that deviation. F1 also contradicts the stated greater-than-1-pixel separation: its competing scores differ by approximately `0.807`.

7. **High — NAV-171/200; §8.7 — crossing requirements remain incomplete despite narrowed-clearance wording.**
   **Addresses:** `0x00467a50`, `0x0046a900`, `0x00469770`, `0x0046a000`, `0x005bddb0`.
   The stair description universally places the current gait before the stair gait, but the routine changes that ordering with traversal direction. “Mount”, “climb loop” and “dismount” still do not identify sufficient playback requirements, endpoints and completion conditions. The pinned sibling explicitly leaves the animation-field identification unresolved.
   **Correction:** specify the direction-dependent observable action sequences and timing sources, or explicitly withhold these crossings and define their fallback. §8.7 currently supplies neither a complete crossing contract nor a fallback for this gap.

8. **High — NAV-046/057/100/130/170/172/210; §8.7 — fallbacks are contradictory or incomplete.**
   **Addresses:** `0x0051ac2f–0x0051ac34`, `0x004d7880`, `0x0054eea0`, `0x004e8fc0`, `0x00583630`.
   Opening a closed door requires admission, while NAV-170 requires the door already to be open. Failing jump *elements* does not define unresolved jump-zone loading and targeting. Disabling patch toggles does not define unread patch-target resolution. “Owned by the AI specification’s assumption” identifies neither the claim nor an executable fallback.
   **Correction:** supply reachable, unambiguous fallback outcomes and named assumption mappings. Distinguish disabled patch geometry from any retained door-lock effects, and reconcile section 11’s unconditional implementation requirements with the narrowed scope.

9. **High — NAV-110/141/058; §7.2 tests 2, 6 and 14 — fixtures contain false statements.**
   **Addresses:** `0x004f5750`, `0x004ed070`, `0x00553e60`, `0x00554360`.
   Test 2’s box spans `(194,96)` to `(206,104)` and touches none of F1’s obstacle walls. Under NAV-110 it is clear, despite lying inside the obstacle. Test 6’s annotation “`g = h`” is false. Test 14 says N3 wins because it was opened *before* N2; in the stated traversal it opens **after** N2 and wins as the newer equal-score candidate.
   **Correction:** fix these expectations and explanations. Use a genuinely wall-intersecting box for the negative clearance case. Retain the distinction between clearance and polygon membership.

10. **High — NAV-090/110/111/143/144/146; §7 — required acceptance coverage remains missing.**
    **Addresses:** `0x004f5890`, `0x00553a10`, `0x00554b80`, `0x005553d0`, `0x004d23d0`, `0x0055aa60`.
    There are no determinate diagonal/multiple-wall unsticking fixtures, complete axis-boundary cases for all four candidate bits, or snapshot continuations before dispatch and before delivery. Test 13 specifies timing without geometry and expected recomputed waypoints. Test 14 checks availability, not pending-result restoration.
    **Correction:** add exact inputs and outputs for those cases, cancellation, budget exhaustion, failed-pending restoration and a patch that changes the pending path. Include multi-bit corner-set overlap. Section 7.3 also needs a defined recording/measurement procedure before its one-frame timing tolerance can establish anything.

11. **Medium — NAV-146; §8.7 — failure timeout boundary is wrong.**
    **Address:** `0x004d23d0`, especially `0x004d2610`.
    The original expires the pending failure when the current tick is **strictly greater** than arrival tick plus 100. It remains pending at `arrival + 100`; ordinary expiry is at `arrival + 101`.
    **Correction:** state that boundary and test both ticks, or declare the proposed 100-tick expiry as another deviation.

12. **Medium — identity block — reviewer identity remains incomplete.**
    **Address:** documentary requirement, ADR-0009 and `SPEC-TEMPLATE.md`.
    Revision 3 acknowledges that earlier reviewer session IDs were not recorded, but still supplies only model names and review numbers, with generic exposure wording.
    **Correction:** recover session identifiers where possible; otherwise record their unavailability and stable review-event identities with the inspected revision and subsystem exposure. Record this re-review separately. The revised necessity and stopping statements resolve the earlier overclaims.

The automated asset check passes. Existing probes also consume all nine maps’ STAT, FARM and lift chunks exactly; these checks do not clear the expression findings.

**Cleared independently:** static format work excluding unread TUPO/PPPP semantics; the availability predicate; NAV-110’s map-overlap requirement; NAV-057’s request-state distinction; NAV-123; NAV-143; the corrected reconstruction direction and test-5 sequences; NAV-150(a); the reviewed navigation-native corrections and §8.4 invalid-input decisions.

**Waiting:** full path-search implementation, scheduling/snapshot integration, proximity handling, dynamic patches, player door interaction and precise crossing execution, pending the corrections above. Proposed deviations and publication approval remain pending separately.

**Verdict: fix-then-clear.**