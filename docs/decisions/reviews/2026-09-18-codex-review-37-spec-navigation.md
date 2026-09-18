# Codex spec re-review 37: docs/original/spec-navigation.md (revision 5), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`.

Reviewed [revision 5](C:/Users/przem/source/repos/opensherwood/docs/original/spec-navigation.md), commit `4e35271`, blob `0ad05e9f43ea15b463951f03e0b466575ec15798`, against the decompilation and navigation notes. Addresses are virtual addresses, image base `0x400000`. This is a spec review only.

1. **Blocker — NAV-111; test 15 — expression filter still fails.**
   **Address:** `0x004f5890`.
   The new `T_i` recurrence still encodes the recovered four corner updates and their conditional branches one for one. Calling it a function does not remove the procedural transcription. Test 15(e) also describes an intermediate corner after translation, despite promising inputs and final outputs only. Its final geometric result checks out.

   **Correction:** characterize the final separation geometrically, retaining the demonstrated order-sensitive outcome without prescribing successive corner updates. Remove the intermediate execution commentary from test 15(e).

2. **High — NAV-058; §§8.2–8.3 — edge-order history omits the owning node’s availability.**
   **Addresses:** `0x00555afd–0x00555b39`, `0x00555b39–0x00555c79`, `0x00555d35–0x00555d79`.
   The reverse node orders are now correct. However, the executable updates an edge collection only when its owning node remains available or becomes available. While that node stays unavailable, its edge orders retain their history. The specification instead applies the edge transformation on every recomputation.

   Disabling and re-enabling an edge while its owner remains unavailable can therefore reorder it under the specified rule while leaving its original order unchanged in the executable.

   **Correction:** specify that conditional preservation and add a continuation fixture covering it. Retaining both sequences in snapshots resolves the earlier snapshot omission.

3. **High — NAV-150(b); movement hand-off — the new eligibility list tests the wrong quantity.**
   **Addresses:** `0x0056133c–0x00561347`, `0x005fc0e0`; directional comparison at `0x005613be–0x005613e0`.
   The purported “mover’s displacement this frame is nonzero” check actually compares the candidate’s position with the mover’s position. It excludes coincident positions; it does not inspect this frame’s displacement.

   The pinned ANIM-240 also still describes the directional product as speed-dependent, whereas NAV-150 specifies a unit direction. The new pin alone does not resolve that disagreement.

   **Correction:** replace the displacement condition with distinct candidate/mover positions. Establish an explicitly consistent directional-product contract and precedence against ANIM-240. The class/action exclusions, common exclusions and square extents otherwise agree with the inspected checks.

4. **High — NAV-121/141; §8.5 — numeric acceptance evidence remains incorrect and incomplete.**
   **Addresses:** `0x00556490`, `0x00556839–0x0055688c`, `0x00554360`, `0x00556990`, `0x00554a14–0x00554a55`, `0x004f9ff5–0x004fa01f`.
   The improvement-comparison boundary is now correctly distinguished from the stored cost. Remaining problems:

   - In the new fixture, N initially scores approximately **224.80664**, below M’s **229.31712**. Examination begins **N, M, N**, because M improves and reopens N. The stated final predecessor alternatives remain possible, but “M is examined first” is false.
   - Integer coordinates do not guarantee exact `f32` cross products. For example, `4097×4097 − 4096×4098` is exactly 1, but separately rounded `f32` products subtract to 0. Runtime starts and smoothing endpoints also need not be integral.
   - The 20,000-pixel shrink assertion is false: shrinking the one-pixel interval from 4096 to 4097 by the stated fraction rounds both endpoints back to their original values in `f32`.
   - Exact ties and widely separated scores still do not supply the requested rounding-sensitive score-order and geometric fixtures.

   **Correction:** correct the examination order and distinguish predecessor-chain expectations from delivered waypoints. Remove the unsupported retail geometric-equivalence claim. Supply complete score-order and geometric boundary fixtures with explicit policy outcomes.

5. **High — NAV-046/057/172; `NavPatches` — the inert loader still depends on unestablished record framing.**
   **Addresses:** `0x004c3930`, `0x0054eea0`; native contracts at `0x00571700`, `0x00579d00`, `0x00570e20–0x00570e50`.
   Handles, element slots, query/setter behavior and snapshot flags now have concrete fallback semantics. But requiring patch **names** to be read from `rhp.md` is not implementable from its claimed two strings plus 100-byte record layout. Applying that layout fails on **all eight nonempty retail `TUPO` chunks**; the empty Sherwood chunk alone consumes exactly.

   **Correction:** either clear sufficient variable record framing to locate every required name, or define the fallback using the established count and inert indexed slots without requiring unread names. Withhold any name-dependent binding until specified.

6. **Medium — NAV-090/146; §8.1; test 13(g) — the completed-result restore case remains untested.**
   **Addresses:** `0x005532a0`, `0x005546e0`, `0x004d23d0`, `0x004d28e0`.
   Case (g) restores snapshot **(a)**, containing queued work, then computes a fresh result. It never restores a completed result.

   There is also a phase distinction to resolve: restoring snapshot **(b)** at the end of `t+1` makes P’s result deliver at P1 of `t+2`, before a patch during P3. That patch cannot revise P’s already-delivered path.

   **Correction:** add the actual continuation from (b). Under the present schedule, P receives the lower path at `t+2`; a subsequent patch can recompute Q for upper-path delivery at `t+3`. If invalidating restored P before delivery is intended, specify a supported phase for that event. The repeated-patch and cancellation/submission cases otherwise address their earlier omissions.

The asset check passes. Existing probes consume all nine maps’ `STAT`, `FARM` and lift chunks exactly. Review 32’s proximity-constant correction is complete; the concrete movement fallbacks and accurate AI-065 reference also resolve those portions of its dependency finding.

| Component | Updated clearance |
|---|---|
| Formats | Established layouts cleared. `TUPO` name/record parsing waits on finding 5; unread `TUPO`/`PPPP` semantics remain excluded. |
| Corner graph and search | Graph decoding, availability predicate, quadrant rules, corner walks and waypoint ordering remain cleared. Full integration waits on findings 1, 2, 4 and 6, plus applicable deviation approvals. |
| Click resolution | Established resolution and cursor rules cleared. Patch/jump behavior remains provisional. |
| Doors | Admission, native state changes and passage action ordering cleared. Original leaf interaction remains unread; execution inherits the movement boundary. |
| Buildings | Established admission, placement, occupancy and removal semantics cleared. Full execution inherits the movement boundary. |
| Stairs | Gait ordering, endpoints and run-flag rule cleared. Execution timing inherits the movement boundary. Ladder/climb substitution remains a fallback. |
| Movement hand-off | Waypoint delivery, approach tests and navigation-owned crossing state cleared. Proximity integration waits on finding 3; faithful reactions and sliding remain unread. |

Dynamic patches and faithful ladder/climb execution remain deferred. Scheduling, canonical availability order and numeric-policy approvals remain separate from factual clearance, as does publication approval.

**Verdict: fix-then-clear.**