# Codex spec re-review 46: docs/original/spec-campaign-camp-saves.md (campaign, camp and saves, revision 8), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`. To be answered in the spec's next revision.

Reviewed revision 8, commit `e5b56e6`. Files remain unchanged because this session is read-only.

1. **High — CAMP-335/339, B66/B66a; 0x00581020, 0x005810C0, 0x00581180.** Review 43 is only partly answered. Section 3.11 still calls B66 conditional and predicts potentially different integers at narrower precision; §10 question 1 still declares output settled. **Correction:** synchronize both with the unconditional **10/15** acceptance row and retain precision-dependent output as open. B66a’s integer expectations are correct, but its 53-bit upward-rounded product is approximately **1.00000007078**, not exactly 1.0. The operand-width comment and per-workshop rebuild wording are corrected.

2. **High — CAMP-074; 0x0051D480, 0x0051D5D0, 0x00565490, 0x0055BAD0.** “Two profile stat fields” does not identify which initializes each experience slot; §10 question 11 explicitly remains open despite removal of `CharacterInitialisation`. **Correction:** specify `exp[0].level` from the PC profile’s `pre[4..5]` melee experience and `exp[1].level` from `pre[2..3]` bow skill, with zero remainders. Also state `deployed = false`. Health 100, cleared stocks and flags, difficulty adjustment, and **T4’s nine wire-order mappings are supported**.

3. **High — CAMP-317; 0x005795B0, 0x0050ED10, 0x005358D0, 0x00535910, 0x00528560.** Native 173 is not established as “mission-selection screen open.” Both cited actions select a node and close their dialog; one leaves the byte set and the other clears it. The value therefore survives dialog closure. Also, 0x0050ED40 has another route to constructing the cited widget, contradicting “only when.” **Correction:** specify the actual menu-choice state and its transitions, initialization and persistence; retain the exclusion until its gameplay meaning is traced. Add its authoritative state to the snapshot contract or identify its owning sibling contract.

4. **High — CAMP-316; 0x00453E60, 0x00454C00, 0x004552F0, 0x004559F0.** The proposed spawn contract misstates more than tie-breaking. Required-character placement is driven by a numeric spawn selector matched to profile designer identities, not an arbitrary point-name comparison. Remembered camp slots follow that placement. Camp points are subsequently randomized; the two draw sites execute repeatedly, not twice overall. Remaining allocation considers per-point capabilities and prioritizes uniquely matching candidates before relaxing ambiguity. Camp initialization also clears `team`. **Correction:** specify these observable results and orderings, including unmatched-required-point behavior, allocation limits and RNG consumption. Do not promise that every unresolved named point remains empty or every remaining point is filled. Keep broader spawn clearance withheld.

5. **High — CAMP-014; 0x004E3260, 0x004A1BC0, 0x0046FFE0, 0x00579B10.** Qualification is narrower than all non-player humans: the tested category selects soldier-family actors, excluding civilian-family actors, followed by the hostile-side test. More seriously, **18 is an actor action/order state**, the same state native 89 tests—not a character-profile combat class. **Correction:** distinguish actor-family qualification, alive/dead counting and the survivor bonus. Express the latter through the established native-89 predicate plus the still-unidentified actor flag.

6. **High — CAMP-254; 0x004A05A0, 0x00464630, 0x00582560, 0x00585570.** “Element kind 161 with parameter 9” reproduces internal machinery without specifying the required revive behavior. The 9 selects immediate dispatch; it is not a documented revive-action argument. Generic scheduler references do not establish the resulting posture, admission or replacement effects. **Correction:** describe the observable actor transition and its timing, retaining unresolved effects. Remove internal construction details unless an interoperability requirement justifies them. This passage currently fails the expression filter.

7. **High — CAMP-290/291/292; 0x004524B0, 0x004E3260, 0x00455BF0.** Splitting `RecruitArithmetic` into `RecruitCount` and `RecoveryValues` drops the still-unknown mapping from the new-character draw to a profile choice. “Every effect” is consequently not settled. The proposed documented default for recruit count also contradicts §11’s exclusion and §12’s prohibition on winning-path assumptions. **Correction:** retain an explicit new-recruit-choice exclusion and withdraw default-based clearance unless a separate project deviation is approved. Recruitment remains incomplete.

8. **High — CAMP-224; 0x004539F0, 0x0040A230, 0x0050F710.** Section 3.3.3 appends the **finished** mission to `recent`; the executable appends the newly chosen non-camp `current` mission, and appends nothing on camp entry. The summary also omits the zero-blazon defend resolutions before team reset and presents a universal next-offer path without the terminal campaign handling. “No save … is involved” contradicts its own backup/auto-save step. **Correction:** reconcile the sequence with §3.4, include terminal handling, and distinguish automatic checkpoints from any requirement for a player-initiated save.

9. **Medium — CAMP-302/303/317; 0x00578D80, 0x00585B70, 0x0050ED10.** Section 6 still says 1001 completion is unknown, 1000’s origin is unknown, and native 173 has no identified writers; §9 repeats the message exclusions. **Correction:** synchronize these summaries. Immediate completion of the 1001 handler and absence of retail 1000 submissions are supported; native 173 must reflect finding 3.

10. **Medium — §8.1a / OriginalSaveResume; 0x00511340, 0x004C4570.** Extraction is described as letting a player “carry progress in,” although §8.3 expressly excludes both resuming and seeding a playable campaign from extracted state. **Correction:** describe extraction as inspection/conversion input only. The decision to satisfy saving through OpenSherwood’s own format is valid; it does not establish playable original-save import or complete the missing authoritative-state contracts.

11. **Blocker — §§0, 11–12; CAMP-316/326/333a/339; 0x00453E60, 0x00456A70, 0x00580D20.** The stopping statement incorrectly relegates all remaining exclusions beyond its short list to presentation/interoperability. Workshop membership, assignment capture scheduling and mode-dependent production affect stock, training and healing during campaign play. Unspecified “observable contracts” do not remove those dependencies. **Correction:** withdraw whole-campaign readiness and retain explicit gameplay integration gates until these rules are settled or covered by approved deviations.

T4 passes the expression filter as a file-field/interface mapping; add its serializer provenance at **0x0055C3C0**. Section 3.3.3’s observable transition summary is admissible, subject to finding 8. The asset-policy check passes.

| Component | Cleared scope | Still withheld |
|---|---|---|
| Campaign graph | Established graph, accessibility and outcome rules | Complete campaign transition and terminal integration |
| Offers | Reduction, retries, selection, aging and persistent mutations | Recruitment/deployment dependencies; CAMP-224 corrections |
| Recruitment | Established branch structure, naming, health and stock initialization; T4 | Experience-slot specification, recruit count, profile choice, recovery values |
| Deployment | Membership/capability tests and T1–T3 | Spawn contract, native 165 actor effects, native 173 semantics |
| Camp screen | Established presentation; immediate 1001 handler; no retail 1000 send requirement | Native 173, deployment and production integration |
| Production | Verified arithmetic structure, B66 integers, established effects and per-workshop ordering | Effective arithmetic mode, membership rebuilding, capture scheduling, extras and retained overflow boundary |
| Statistics | Established arithmetic and deterministic time contract | Corrected qualification/bonus contract, complete revival, profile percentage and update scheduling |
| Own-format saves | Own-format decision; existing campaign snapshot and continuation requirements | Complete coverage of newly specified state and unresolved subsystem contracts |
| Original-save/profile reading | Previously cleared grammar and campaign-prefix extraction | Playable original-save resume and original-readable exports, excluded by decision |
| HUD / mini-map | Previously cleared counters and native-24 behavior | Existing clover, palette, mapping and visibility exclusions |

**These components are not yet cleared as an implementable whole for campaign A to Z.** Excluding original-save resume and exports does not remove the gameplay blockers.

**Verdict: fix-then-clear.**