# Codex spec re-review 41: docs/original/spec-campaign-camp-saves.md (campaign, revision 6), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`. To be answered in the spec's next revision.

Reviewed [revision 6 at `3d21293`](C:/Users/przem/source/repos/opensherwood/docs/original/spec-campaign-camp-saves.md) against review 34, the decompilation, instruction listings, campaign notes and conversion-helper bytes. Remaining findings:

1. **High — CAMP-271/272; §§3.7, 8.4–8.5, B60a; 0x0050E7B0, 0x0050E800, 0x00512310.** B60a still requires refusing a snapshot away from a checkpoint, directly contradicting §8.4’s new checkpoint-on-request policy. Change its expectation to successful accrual followed by serialization and equivalent continuation. Initialize `interval_start_frame` explicitly, since it is hashed even while inactive. Also synchronize the transition description: §3.7 still says “not at level end,” and 0x00512310 closes and reopens an interval around a menu without ending the level. Define those suspension/resumption checkpoints alongside level transitions.

2. **High — CAMP-335/339; §3.11, B66a, §§0, 9–12; 0x00581020, 0x005810C0, 0x00581180, 0x00642B7C.** `ProductionPrecisionMode` correctly preserves the uncertainty, but surrounding clearance still exceeds it. The formula retains “computed in double”; §9 says nothing remains to guess; §§10–12 declare daily output settled. B66a is also conditional on precision: its narrowed product is approximately **0.9999999776482582**, but rounding that multiplication to a 24-bit significand produces **1.0**, hence output **1**, despite the narrowing. Clear the verified operands, factor order, single-precision store, signed-64-bit truncation and retained bits individually. Keep exact output and mode-dependent acceptance results conditional until an analyst amendment establishes the mode, proves equivalence over a stated domain, or specifies a deviation. The claimed exactness of the intermediate product needs the same qualification.

3. **Medium — CAMP-321/328/333a; §3.11 and §11; 0x00456A40, 0x00580D20, 0x004FC590, 0x004FC660.** The new wording places membership rebuilding at the end of the overall workshop pass. It actually follows **each workshop’s** work and character placement, before the next workshop proceeds. Correct that frequency and ordering. Retain `LocationMembershipRebuild` for the unresolved membership predicates, resulting member order and dependent integration.

4. **Medium — CAMP-338; §11, `CampExtrasSource`; 0x00580EC8–0x00580EDD.** The exclusion still says an empty extras list “is what a stock campaign has.” That contradicts the corrected §3.11 and exceeds the fixture evidence. Replace it with “empty in the inspected fixtures”; allow empty-list behavior without assuming all stock campaigns remain empty. The corrected minimum-count placement and silent exhaustion are supported.

5. **Medium — CAMP-300/301; claim table; 0x0050F710, submission sites 0x0050F8BB and 0x0050FD58.** Although §6 now records both occurrence conditions correctly, CAMP-300 still unconditionally couples camp level start with message submission and production. Qualify it with the mission-selection entry exception. Preserve the distinction that an accepted selection of the current node submits the second message **without another workshop pass**. Handler completion ordering remains excluded.

6. **Low — clearance/provenance contract, CAMP-336; §§0, 3.11, 13; 0x005803C0, 0x00642B7C.** Section 11 contains **19 exclusions**, not seventeen. Section 13 still introduces “two reviews” before listing four. The teardown paragraph still cites CAMP-332, which describes the campaign-map status line; use CAMP-336. Explicitly include instruction-listing and conversion-helper exposure in the reviewer record as well as the analyst record.

The expression filter passes this revision: no new prohibited expression was identified. The production formulas and required observable orderings are admissible; the palette remains withheld. The heuristic asset check also passes.

| Component | Cleared for implementation | Still waits |
|---|---|---|
| Campaign graph and offers | Graph, accessibility, reducer including retries and persistent mutations, selection/aging, established outcome and blazon rules; corrected B63 and B14b | Excluded recruitment, initialization and mission-end dependencies for complete campaign execution |
| Save and profile grammar | Documented profile/configuration reading and campaign-prefix extraction, versions 28–48 with stated defaults and mismatch rejection | Playable original-save restoration, reconstruction from extracted state, complete level payload, original-readable exports and donor validation |
| Deployment | Membership/capability checks, T1/T2/T3, established list and selection effects | Spawn assignment, native 165 actor effects, native 173 state |
| Production | Constants, inputs, supervisor factors, narrowing, factor order, 64-bit truncation, masks, stock addition/wrapping, victory gates, established placement/capture/teardown facts and kind-12 exhaustion | Precision-dependent output, membership integration, capture scheduling, extras source/class, original health behavior beyond the established cap comparison |
| Statistics | Counter identities, established score/progress/map-percentage arithmetic, delta propagation and corrected revive ordering | Time-contract corrections above, hostile qualification, complete revive effects, profile percentage and update scheduling |
| Camp screen | Documented campaign-map presentation and message occurrence conditions | Complete interactive execution, message completion ordering, send action, recruitment arithmetic and initialization |
| HUD and mini-map | Money/clover counters, established blazon/selection behavior, native 24 validation and override preservation | Clover producers, portrait/action/tooltip behavior, mini-map palette, state mapping and visibility |

`HealthCapOverflow` appropriately withholds the untraced original setter result; the declared clamp deviation does not clear that original behavior. Publication approval remains separately pending.

**Verdict: fix-then-clear.**