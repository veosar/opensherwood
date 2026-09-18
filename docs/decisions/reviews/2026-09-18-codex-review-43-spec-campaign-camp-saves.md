# Codex spec re-review 43: docs/original/spec-campaign-camp-saves.md (campaign, camp and saves, revision 7), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`. To be answered in the spec's next revision.

Reviewed revision 7, commit `95ab648`, against review 41, the decompilation, production listings, campaign notes and conversion-helper bytes. Remaining findings:

1. **High — CAMP-335/339; 0x00581020, 0x005810C0, 0x00581180, 0x00642B7C.** The production contract still contradicts `ProductionPrecisionMode`. The formula comment (spec line 1003) retains “computed in double,” and open question 1 (spec line 1724) declares daily output closed with only the extras question remaining. The listings establish operand width and narrowing, not multiplication precision. **Correction:** replace the comment with the verified double-width operand and binary32 narrowing facts; explicitly retain precision-dependent output among the unresolved questions.

2. **Medium — CAMP-335/339, B66/B66a; 0x00581043–0x0058109D, 0x00642B7C.** B66a (spec line 1462) conditions its numbers on significand width but omits rounding direction. Its stated results assume round-to-nearest: with a 24-bit significand and rounding toward zero, the product is approximately **0.9999999404**, giving **0**; with 53-bit precision and upward rounding, the narrowed scale instead produces output **1**. The helper’s truncation setting applies only to conversion. Also, B66’s prose suggests narrower precision may change its integers, while its acceptance row unconditionally requires 10/15; those values remain 10/15 across the standard 24/53/64-bit precision and rounding combinations. **Correction:** state the complete arithmetic assumptions for B66a, and reconcile B66 through a documented equivalence result or consistently conditional expectations.

3. **Medium — CAMP-321/333a; 0x00456A40, 0x00580D20, 0x004FC590, 0x004FC660.** Section 3.11 and the claim table correctly place membership rebuilding after **each workshop**, but the stopping statement (spec line 93), exclusion row (spec line 1772) and section 11’s closing paragraph still describe a rebuild ending the overall pass. **Correction:** synchronize all three with the per-workshop ordering. Retain `LocationMembershipRebuild` for unresolved predicates, resulting membership/order and dependent integration.

The expression filter passes: no new prohibited expression identified; functional formulas, interface mappings and required orderings remain admissible. The palette remains withheld. The asset-policy check passes.

| Component | Cleared scope | Still withheld |
|---|---|---|
| Campaign graph and offers | Graph, accessibility, reduction/retries and persistent mutations, selection/aging, established outcome and blazon rules | Recruitment, initialization and mission-end dependencies needed for complete execution |
| Save and profile grammar | Documented profile/configuration reading; campaign-prefix extraction, versions 28–48 and stated defaults/validation | Complete level payload, playable original-save restoration, reconstruction, original-readable exports and donor validation |
| Deployment | Membership/capability checks, T1/T2/T3, established list and selection effects | Spawn assignment, native 165 actor effects, native 173 state |
| Production | Verified operands, factor order, narrowing, truncation, retained bits, stock addition/wrapping, gates and established placement/capture/teardown facts | Findings above; exact mode-dependent output, membership integration, capture scheduling, extras source/class, unresolved original health-overflow behavior |
| Statistics | Established counters and arithmetic; corrected deterministic time contract, including suspension and snapshot-on-request | Hostile qualification, complete revive effects, profile percentage and update scheduling |
| Camp screen | Established presentation and corrected message occurrence conditions | Handler completion ordering, send action and excluded campaign dependencies |
| HUD and mini-map | Established counters, blazon/selection behavior and native 24 validation/override preservation | Clover producers, remaining UI behavior, palette, state mapping and visibility |

**These components are not cleared as an implementable whole.** The bounded clearances above stand; the nineteen exclusions still prevent complete integration. Publication approval remains separate.

**Verdict: fix-then-clear.**