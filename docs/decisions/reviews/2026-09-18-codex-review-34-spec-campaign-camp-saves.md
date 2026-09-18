# Codex spec re-review 34: docs/original/spec-campaign-camp-saves.md (revision 5), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`.

Reviewed [revision 5 at `50d52ec`](C:/Users/przem/source/repos/opensherwood/docs/original/spec-campaign-camp-saves.md) against review 29, the listings, decompilation and campaign notes. Remaining findings:

1. **Blocker — CAMP-091/300/320; §8.6; 0x0050F710, 0x00511340, 0x00580D20.** The departure contract still explicitly permits resuming an imported camp save by re-entering camp. This contradicts §8.3 and `OriginalSaveResume`, and reintroduces duplicated production, training and healing. Replace that paragraph with extraction-only support. Also qualify §8.3’s “new campaign started from it”: that cannot implicitly authorize the same unresolved reconstruction.

2. **Blocker — CAMP-271/272, snapshot contract; §§8.4–8.5, B60/B60a; 0x0050E7B0, 0x0050E800, 0x00512310.** The sixty-fourths arithmetic is now exact, but §8.5 still hashes `playtime_residue_frames` and omits `interval_active`. Specify the same four fields consistently. Define when an interval closes—currently it becomes active and never becomes inactive—and how transition checkpoints differ from continuing-save checkpoints. Resolve whether a snapshot request creates a checkpoint or is refused outside one; §8.4 and B60a currently prescribe both. B60 now has the required canonical-continuation structure, but depends on this correction.

3. **High — CAMP-339; §3.11; 0x00581020, 0x005810C0, 0x00581180, 0x00642B7C.** The conversion is not directly to signed 32-bit. The helper truncates toward zero to **signed 64-bit**, and callers retain the relevant low bits. This matters outside the signed-32-bit range. Also, a double-width constant operand does not establish that the multiplication itself rounds to binary64: establish the effective x87 precision/rounding mode, or demonstrate equivalence over the supported input domain. Preserve the verified single-precision store of `capacity × S`, factor order and subsequent masks.

4. **High — CAMP-338; §3.11, B71/B72; 0x00580EC8–0x00580EDD, 0x00581220.** Kind 12 stops when the extras list is exhausted. It places **the smaller of the spot count and extras count**, without reporting an ordinary short-list bounds error. An empty list places nothing silently. The defensive bounds helper throws; it does not report and return to an out-of-bounds read. Correct the claim and both tests. Empty fixtures also do not establish that every stock campaign always has an empty list.

5. **High — CAMP-321/328/331a; §3.11; 0x00580D20, 0x004FC590, 0x004FC660.** The asserted complete workshop effects omit the post-placement clearing and rebuilding of location membership. This membership feeds the zone-member lists later used by assignment capture. Specify the resulting membership, observable ordering and relevant predicates, reference a cleared specification that establishes them, or explicitly exclude this integration. Production arithmetic alone does not clear the whole workshop pass.

6. **High — CAMP-203/212; B63; 0x00451B70, 0x00452A50.** The proposed failed attempt cannot occur as described: restoring both candidates makes the attempt non-empty and therefore successful. Their initial zero ages also cannot demonstrate a persisted reset. Use a real failure—for example, two distinct story candidates initially at age 2 with maximum age 2, no obligatory/recent constraints, distinct locations and probability 100. Expiry empties the first attempt and resets their ages; the second attempt must observe those resets.

7. **Medium — CAMP-211; B14b; 0x00452C90.** The new test again stops at two candidates. The actual condition is **fewer than two**. Four distinct candidates containing three recent entries lose all three recent entries and leave **one**. B14a’s revised two-survivor expectation is correct.

8. **High — CAMP-301/303; §6 message table; 0x0050F710.** The interface table still says message 1001 occurs “once, at camp level start.” Replace it with the two occurrence conditions adopted in §3.9/CAMP-301 from pinned VM-113. Keep submission frequency distinct from the still-excluded completion ordering; repeated registration through native 200 makes this consequential.

9. **Medium — CAMP-071/295; §2.3 and CAMP-071; 0x0055BF00, 0x0055BDF0, 0x004565C0, 0x00456670.** Both still declare the retry exit unresolved. Replace those statements with the established history test, append-on-success rule and ten-collision fallback without history insertion. The corrected recruit-stream counts and even/new versus odd/recovery polarity are supported.

10. **Medium — CAMP-263; §2.2; 0x00455A70, 0x005F8030.** The field table still says zero `men_per_blazon` is diagnosed and the routine returns. State original-process termination there too; returning zero belongs exclusively to OpenSherwood’s declared deviation.

11. **Medium — CAMP-253; §3.5 and claim row; 0x004A05A0, 0x00464630.** The revised text invents an ordering: health becomes 50 and “three further operations follow.” Two unidentified operations precede the health write; the activity-scheduling operation follows it. Correct that observable ordering while retaining `RevivePassEffects`. Removing the untranslated argument tuples resolves the earlier expression-filter defect.

12. **High — CAMP-322/324/339; §3.11, B66a/B73; 0x00581020, 0x00581180, 0x00484360.** Numerical acceptance remains incomplete and partly wrong. B66a supplies no distinguishing input: capacity 5, yield 200, one non-supervisor assignment produces **0** with the narrowed scale versus **1** without narrowing. B73 must assert that stock **adds** the masked output and wraps to 16 bits; it does not replace stock with output. Also reconcile the unconditional “health capped at 100” claim with the signed-16-bit sum comparison: sufficiently large outputs can bypass that cap. Specify the resulting behavior or a bounded domain/deviation, and test it.

13. **Medium — clearance/provenance contract; §§0, 2.7, 11–13; CAMP-327/335/336/338.** The stopping statement still excludes production arithmetic, and §2.7 still calls the supervisor contribution unknown. Section 11 contains **16 exclusions**, not fourteen or twelve. Its blanket “whole workshop pass” clearance exceeds the unresolved effects above. Synchronize the scope statements and fix the teardown’s mistaken CAMP-332 reference to CAMP-336. Update the identity block’s review count and explicitly include instruction-listing/helper exposure.

The expression filter’s earlier palette assignments and raw revive arguments have been removed; the functional production formula and individual constants are admissible. Replace kind 12’s reference to using the same placement “calls” with required placement results. The heuristic asset check passes. The identity block preserves analyst/reviewer separation; publication approval remains separately pending.

The implementation clearance is limited as follows:

| Component | Cleared | Still waits |
|---|---|---|
| Campaign graph and offers | Graph interpretation, accessibility, selection/aging, established outcome/blazon rules, corrected deferred-list and forced-path behavior | Complete reducer signoff pending corrected acceptance cases and contradictory fatal-error text |
| Save and profile grammar | Documented campaign-prefix extraction and profile/configuration reading; versions 28–48 with stated defaults and mismatched-version rejection | Playable original-save restoration; complete level payload; original-readable exports |
| Deployment | Team membership/capability checks, T1, T2 and corrected T3, established list/HUD effects | Spawn assignment, native 165’s unresolved effects, native 173’s state |
| Production | Five constants, yield-field interpretation, supervisor factors, assignment scaling only for kinds 0–8, victory gates, established placement/capture/teardown facts | Final numerical contract, corrected kind-12 boundary behavior, non-empty extras semantics and complete workshop integration |
| Statistics | Counter identities, established score/progress/map-percentage arithmetic and delta propagation | Authoritative time contract, hostile qualification, complete revive effects, profile percentage and update scheduling |
| Camp screen | Documented campaign-map presentation facts, HUD counters and settled interfaces | Complete interactive camp execution, message ordering/send action, capture scheduling, recruitment arithmetic, initialization and mini-map palette/state mapping |

The donor design now keeps original-compatible writing behind validation and a separate clearance gate. That gate remains closed; donor eligibility/build trust and actual original-load validation are not established merely by parsing a file.

**Verdict: fix-then-clear.**