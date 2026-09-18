# Codex spec re-review 48: docs/original/spec-navigation.md (navigation, revision 6), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`. To be answered in the spec's next revision. Includes the revision 5 to 6 change table for rebuild batch 2.

Reviewed navigation revision 6, commit `ef288bc`, blob `16f5391dc1471e2b4c64037f4cd3c154211c2062`, against the executable evidence and navigation notes. This is an ADR-0009 spec review only.

1. **High — NAV-090/057/146; §8.1–8.2; test 13(g): the restore conclusion incorrectly excludes patches before the consumer slot.**  
   **Addresses:** `0x004c6ef0`, `0x00570e30`, `0x00570e50`, `0x00550150`, `0x00550220`, `0x0054fe50`, `0x004d28e0`.  
   The revised continuation is correct **for the specified P3 patch**: restored P delivers the lower path at `t+2`; Q can be recomputed for upper-path delivery at `t+3`. However, “no patch can reach it first” and “a patch after a restore can only revise requests dispatched after the restore” are too broad. Script callbacks run before the consumer slot, as the pinned ANIM-101 also states. Patch natives have paths that apply availability changes immediately within those callbacks.

   **Correction:** retain the P3 continuation, but restrict its conclusion to that phase. Define how a patch invoked by a pre-consumer callback affects a restored completed request, including its delivery tick. Add that continuation fixture. Alternatively, explicitly record deferral of those patch effects to P3 as an additional deviation; the present scheduling proposal does not establish it.

2. **Medium — NAV-112/141; §8.5(v): the stated corridor rounding boundary is incorrect.**  
   **Addresses:** `0x0060aaa4`, `0x0060aaca`, `0x0060ab3a`, `0x0060ab5f`; arithmetic helper `0x005fc680`.  
   The segment-intersection routine stores its cross-product results to single precision before comparing them with zero. It does **not** compare all these results without storing, as §8.5 says. Other corridor-side tests do retain extended intermediates, so the distinction matters.

   **Correction:** distinguish rounding the completed cross product from rounding its individual products. Fixture 6 remains a valid discriminator: the wider product evaluation yields 2, which survives storage; separately rounded `f32` products yield 0. Also qualify “original” numeric expectations by the assumed wider precision while the runtime precision-control setting remains unestablished.

3. **Medium — NAV-041/121; §8.5 fixture 5: the door costs require an explicit test override.**  
   **Addresses:** `0x0051b2a0`, `0x004f9c26–0x004f9c34`, `0x004f9ff5–0x004fa01f`.  
   The stated arithmetic and D2-versus-D1 outcome check out **with injected costs of 70**. But NAV-041 derives costs from the supplied door points: D2 costs 120 and D1 costs 130. With those constructed doors, their scores are approximately 250 and 260, and both policies select D2.

   **Correction:** explicitly identify the crossing costs as injected search-kernel inputs that bypass NAV-041, or supply geometrically consistent doors producing the intended rounding-sensitive outcome. The fixture currently names only corridor/quadrant stubs.

4. **Medium — NAV-141; §8.5 fixture 1: the claimed equality of delivered waypoints is not established.**  
   **Address:** `0x00554997–0x005549a2`.  
   The corrected examination order **N, M, N, A** and predecessor chains are right. However, smoothing runs only when the reconstructed list contains more than three points. A policy result containing start, one candidate of A, and goal remains three points even with every corridor test passing. The fixture does not specify candidate sets and edge records sufficient to guarantee the asserted collapse of both lists to start, goal.

   **Correction:** keep this as a predecessor-chain fixture and remove the waypoint-equality assertion, or specify the missing reconstruction inputs and give the resulting waypoint expectations.

The remaining answers to review 37 pass within their stated scope:

- **NAV-111 passes the expression filter.** The reference-corner relation specifies a final geometric selection without prescribing successive box mutations. Test 15(e)’s final box is approximately `(87.37389,96.31724)–(99.37389,104.31724)`; its deepest corner finishes about `0.30275` px from the wall.
- **NAV-058 and F4 agree with the executable.** Unavailable owners retain both edge sequences. Toggling the edge while T0 remains unavailable preserves the lower route; toggling it while T0 is available produces the upper route. Canonical file order produces the lower route in both cases.
- **NAV-150(b) is corrected.** The comparison excludes coincident positions; the directional threshold uses the normalised direction. Movement revision 5 at `002314d` agrees.
- **NAV-121’s initial door-list order and `cost + h`, then `+ g`, are supported at both cited score sites.**
- **NAV-046’s count-only fallback is implementable.** Chunk framing consumes all nine maps exactly, with the stated counts. No patch-record or name parsing is required.

The asset check passes. I found no remaining expression-filter violation in revision 6.

For **rebuild batch 2**, these are the precise revision-5-to-6 changes to check:

| Claim or contract | Meaning change |
|---|---|
| NAV-058; §8.2–8.3 | Edge-order transformations now depend on the owner being available after recomputation. Frozen sequences of unavailable owners must survive snapshots when canonical ordering is rejected. The availability predicate is unchanged. |
| NAV-150(b) | “Nonzero mover displacement” becomes “candidate and mover positions differ.” Inclusive query bounds are now explicit. Unit direction was already navigation’s rule; the new movement pin resolves the sibling contradiction. |
| Movement dependency pin | Revision 3 becomes revision 5. This imports relevant corrections beyond proximity, notably ANIM-241’s progress-region validity and reset behavior. It does not clear the sibling’s excluded execution contracts. |
| NAV-121; §8.5 | Initial doors are explicitly queued in sector door-list order. The proposed `f32` score evaluation changes from `(cost + g) + h` to `(cost + h) + g`. This can change route selection. |
| NAV-046/057/172; `NavPatches` | Named inert records become **nameless slots created from the count**, with the remaining chunk skipped by its length. Name-based binding resolves to none; index and flag behavior remains. |
| NAV-090/146; §8.1–8.2 | The specified P1/P2/P3 order is unchanged. Test 13(g) now restores the completed-result snapshot and preserves P’s delivered path. Its broader phase conclusion needs finding 1 fixed. |
| NAV-111; test 15(e) | Expression-only replacement; no intended geometric behavior change. |
| NAV-141; numeric fixtures | N, M, N, A corrects the fixture’s expectation, not the search rule. Predecessor chains become the asserted observation. Unsupported geometric-equivalence claims are withdrawn. |

The updated component clearance is:

| Component | Clearance |
|---|---|
| Formats and sector polygons | Established layouts cleared. Count-framed inert `TUPO` slots cleared; variable patch records and name binding remain withheld. |
| Corner graph | Decoding, candidates, quadrant rules, edge records, corner walks and reconstruction cleared. NAV-111’s expression blocker is removed. |
| Search | Established search rules remain cleared. Numeric documentation and acceptance details need findings 2 and 4; scheduling integration needs finding 1. |
| Cross-sector routing over doors | Admission, link exclusion, initial queue order and score summation order cleared. Fixture 5 needs finding 3. |
| Availability masks | Predicate and historical node/edge/obstacle ordering cleared, including F4. Dynamic patch-to-mask bindings remain unread. |
| Click resolution | Established resolution and cursor rules cleared. Patch, jump and alternate-resolution behavior remains bounded by the recorded fallbacks. |
| Layers and bonds | Established identities, projection, bond transitions and navigation-owned crossing state remain cleared. |
| Lifts | Stairs’ action order, gaits, endpoints and run flag cleared. Faithful ladder/climb execution remains withheld; stairs substitution is a fallback. |
| Buildings | Established admission, placement, occupancy and removal semantics cleared. Full traversal inherits movement execution limits. |
| Movement hand-off | Waypoint delivery, approach tests, crossing state and corrected proximity eligibility cleared. Full movement execution remains subject to the sibling’s arrival/mode-5 gaps and the reaction/sliding fallbacks. |

**These components are not yet cleared as a complete, faithful mission A-to-Z walking implementation.** The implementer has substantial cleared machinery and concrete fallback behavior, but still lacks patch-driven geometry and bindings, faithful jumps and ladder/climb playback, original leaf interaction, complete movement arrival/record completion, and proximity/sliding behavior. Goal-check and node-reset tails also remain assumptions. Inert patches and stop-only proximity can prevent progress; an implementable fallback does not establish mission completion.

Canonical availability order is sufficiently specified for maintainer approval as a route-changing deviation. Scheduling needs finding 1 resolved before approval as recorded. Numeric policy needs findings 2–4 corrected before its evidence is approval-ready. None of these approvals would clear the withheld original behavior or publication.

The table above is the updated review result; the read-only workspace prevented saving an archive.

**Verdict: fix-then-clear.**