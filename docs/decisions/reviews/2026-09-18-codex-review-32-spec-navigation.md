# Codex spec re-review 32: docs/original/spec-navigation.md (revision 4), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`.

Reviewed [revision 4](C:/Users/przem/source/repos/opensherwood/docs/original/spec-navigation.md), commit `5c380b6`, blob `ec30e08cfcde7b6620cd3c3d2879095c702f0f25`, against `re/out/decomp_all` and `re/notes/nav/`. Addresses below are virtual addresses, image base `0x400000`.

Review-event identity: **2026-09-18 / navigation revision 4 / 5c380b6**. Session identifier unavailable. Exposure: navigation decompilation, its helpers, navigation notes and pinned sibling specifications; this session must not implement those subsystems.

1. **Blocker — NAV-111; test 15 — expression filter still fails.**
   **Address:** `0x004f5890`.
   The “composition” and “sum” formulation still specifies the recovered corner order and successive translations: each distance incorporates preceding corner updates. Test 15(a) repeats that execution trace. Mathematical notation has not removed the procedural transcription identified in review 26.
   **Correction:** express the required separation geometrically, preserving necessary observable wall-order effects, termination and failure outcomes. Give test inputs and final outputs without intermediate corner mutations. The revised NAV-141 smoothing statement resolves its former loop-mechanics problem.

2. **High — NAV-058; §§8.2–8.3; test 14 — re-enablement order is still incorrect, and conditional snapshot coverage remains insufficient.**
   **Addresses:** `0x00555ade–0x00555b2b`, `0x00555d1f–0x00555d70`, `0x00556260`, `0x00556470`, `0x0055aa60`.
   Node removal and node re-enablement traverse their respective orders backwards. Consequently, re-enabled nodes are **not appended in removal order**. For file order `A,B,C`, with A and B initially unavailable and C surviving, removal order is B,A, but enabling both appends A,B: the result is `C,A,B`. Test 14 enables only one node and cannot distinguish these rules. The stated load result also needs to exclude initially available objects that the saved state disables.

   If §8.3 is rejected, storing only available-node/edge order does not preserve the historical ordering of unavailable objects that determines a later re-enablement.
   **Correction:** specify node and edge ordering separately as observable outcomes; add simultaneous and repeated-toggle cases. Require snapshots to retain enough ordering history to reproduce subsequent toggles, including currently unavailable objects, without prescribing the original containers.

3. **Medium — NAV-150(b); §5 — proximity constant contradicts the corrected claim.**
   **Addresses:** `0x0056117c–0x00561269`, `0x00608400`.
   Section 5 still says “radius field + 60, **along the movement direction**.” The executable constructs equal extents on both coordinate axes around the proposed next position.
   **Correction:** state “half-extent `radius + 60` on each coordinate axis.” NAV-150(b)’s square and restored common exclusions clear the corresponding geometric finding.

4. **High — NAV-150/210; §8.7; movement hand-off — pinned dependencies do not supply the claimed contracts.**
   **Addresses:** `0x00561040`, especially `0x005613a6–0x00561497`; `0x00563e90`; documentary references to the pinned siblings.
   Movement revision 2 at `b0cd053` describes a centre-distance proximity test and an additional projection-area restriction. It does not provide the character eligibility list that NAV-150(b) references. Its `CharacterPush` and `CollisionSlide` entries identify unknowns, but do not define executable fallback outcomes. Thus the claim that §8.7 completes the implementation contract is unsupported.

   Likewise, pinned AI revision 3 §3.2.2 supplies **AI-065**, not the unnamed “assumption for unread geometry” described by `NavLineOfSight`.
   **Correction:** pin reviewed, mutually consistent contracts or state explicit precedence and the missing eligibility requirements here. Give concrete movement fallback outcomes or withhold that integration. Refer to AI-065 accurately and preserve its independent clearance boundary.

5. **High — NAV-121/141; §8.5 — precision boundaries and acceptance evidence remain incomplete.**
   **Addresses:** `0x004f9ff5–0x004fa01f`, `0x00556839–0x0055688c`, `0x005581a0`, `0x00554360`.
   Stored-score ordering is now described correctly, but the improvement comparison is missing: at `0x00556839–0x00556843`, the proposed accumulated cost is stored to single precision **without discarding the unrounded intermediate**, which is then compared against the previous stored cost. An all-`f32` implementation can therefore change whether a reaching replaces the selected path, before queue ordering occurs.

   The proposed “near tie” differs by about 0.42 at a score near 204—roughly 27,000 single-precision representable steps. It does not exercise a rounding-sensitive tie. The accumulated-cost example lacks a complete graph and differs by roughly 26 pixels. The final assertion restricting acceptable differences to “such near ties” remains unsupported.
   **Correction:** document the improvement-comparison boundary, distinguish original arithmetic from the proposed evaluation order, and remove the remaining confinement claim. Supply complete rounding-sensitive score, accumulated-cost and geometric fixtures with explicit expected outcomes for the proposed policy.

6. **High — NAV-046/057/172; `NavPatches` — disabling patches leaves script identity and native behavior unspecified.**
   **Addresses:** `0x004c3930`, `0x0054eea0`, `0x00571700`, `0x00570e20`, `0x00570e30`, `0x00570e50`.
   “Patches are not loaded” defines the absence of geometry and toggles, but not patch handles, their registered elements, or natives 5 and 144–146. Omitting their element slots would shift subsequent mission indices; returning null patch handles also conflicts with the pinned native contract, including native 144’s unchecked argument.
   **Correction:** preserve the required index spaces and define inert patch-handle/query/setter behavior, or explicitly withhold that loader/VM integration. Geometry omission alone is insufficient.

7. **Medium — NAV-090/146; §8.1; test 13 — scheduling acceptance coverage still misses the interacting cases.**
   **Addresses:** `0x005532a0`, `0x005546e0`, `0x004d28e0`, `0x004d23d0`.
   The work budget now has a value and accounting units, and the policy supplies concrete delivery ticks. Test 13 nevertheless tests restoration and patch recomputation separately. It does not test a patch **after restoring a completed result**, repeated patches in one update phase, or cancellation followed by submission and patch dispatch. Its toggling cases also need an explicit distinction from the active `NavPatches` fallback, which prohibits toggles.
   **Correction:** add exact queue, delivery and waypoint expectations for those continuations, identifying whether they test the future patch contract or the currently enabled fallback.

The active-crossing identity/side, action queue and completed-request inputs added to §8.2 resolve review 26’s crossing-state omission. The strict failure boundary is correct: pending at `arrival + 100`, expiry at `arrival + 101`. Tests 2 and 6 are corrected; test 14’s newer-candidate explanation is corrected. Tests 16 and 17 agree with the quadrant and corner-walk routines. Test 15’s geometric outcomes check out, subject to finding 1. The historical reviewer-identity repair is sufficient.

The asset check passes. Existing probes consume all nine maps’ `STAT`, `FARM` and lift chunks exactly; that does not clear the expression finding.

**Implementation clearance:**

| Area | Clearance |
|---|---|
| Static formats | Cleared for the established layouts; unread `TUPO`/`PPPP` semantics remain excluded. |
| Corner graph and search | Graph decoding, availability predicate, quadrant rules, corner walks and waypoint ordering cleared independently. Full search integration waits on findings 1, 2 and 5 and the applicable deviation approvals. |
| Click resolution | Established resolution and cursor rules cleared; deferred patch/jump behavior remains explicitly provisional. |
| Doors | Admission, native state changes and passage action ordering cleared. The opening waiver fixes the former fallback circularity; original leaf interaction remains unread. |
| Buildings | Established admission, placement, occupancy and removal semantics cleared. Full execution inherits the movement boundary. |
| Stairs | Revised gait ordering, endpoints and run-flag rule cleared. Precise execution timing inherits the movement boundary. Original ladder/climb execution remains withheld; stairs substitution is only a fallback. |
| Movement hand-off | Waypoint delivery, approach-test semantics and navigation-owned crossing state cleared. Complete collision/proximity execution waits on finding 4. |

Dynamic patches and faithful ladder/climb behavior remain deferred. Scheduling, canonical availability order and numeric-policy approvals remain separate from factual review, as does publication approval.

**Verdict: fix-then-clear.**