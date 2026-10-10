# Navigation integration: review disposition

Status: **work in progress; do not merge or mark a milestone complete**.
Scope: the previously uncommitted navigation/RHP implementation and its integration with the
world, VM and canonical-input harness. Source: [Claude review](2026-10-02-claude-navigation-wip.md).
Implementer role was preserved; no decompilation was read. The separate analyst clarification
attempt reached its turn limit without a specification deliverable, so it clears no gate.

The following are implementation changes, not a blanket clearance of the subsystem. Deferred
findings remain tracked here as blocking work; no external issue or comment was published.

| Finding | Disposition |
|---|---|
| H1 | Open blocker: building/lift routing and exits; requires resolution of G1/G2/G5 first. |
| H2 | Fixed: pending walks count as planned, preserve NPC gait, hold the program counter and advance it only when the sequence finishes/fails. Core patrol regression added. |
| H3 | Replaced `hypot` with explicit squared-length/sqrt operations. Cross-platform verification and the separate numeric-policy gate remain. |
| H4 | Substantially hardened: exact collection partitions, request identities/states/owners/qualifiers, bounded steps, building membership, mapless-state rejection and map reattachment before validation. Negative transactional restore E2E added. General entity sector/layer validity still needs building/default-sector rules. |
| H5 | Versions explicitly bumped to ruleset 20 / snapshot 23 / hash 22; synthetic fixtures regenerated and checked. |
| H6 | Partial: bounded map registration preflight, order/phase reservations, resumed actor scan, charged wall queries and queue scans, journal capture, no per-admission state clone. Remaining hidden work and hostile-map coverage still block clearance. |
| H7 | Open: leaf interaction fallback does not yet open the clicked door; same-layer side selection remains wrong. |
| H8 | Open: script/rail targets still lose explicit sector/layer qualifiers. |
| M1 | Crossing has an explicit threshold/exit phase, with E2E restore of each phase. Degenerate endpoints and stair gait variants still need coverage. |
| M2 | Corrected: the grown box retains the corrected centre after unstick exhaustion; the smaller request box still requires clearance. Full acceptance fixture remains to be added. |
| M3 | Open: lock-picking must read live locks. |
| M4 | Partial: CAVE element indices now resolve through `MissionBinding`; all 39 missions cross-check building counts, actor references and uniqueness. Initial placement, displayed state, carried actors and exit semantics remain open. |
| M5 | Open: dependency-complete navigation assumption recording, proximity/sliding and blocked-movement fidelity. Current synthetic success is not an authoritative mission result. |
| M6 | Incorrect review/provenance comments corrected. Numerical fixtures and policy approval remain open. |
| M7 | Open: patch re-dispatch identity and failed-result timing. |
| M8 | Open: cell registration/traversal and normal conventions need a settled contract and data winding checks. |
| M9 | Open: bond endpoint touches/order and projection height/material integration. |
| M10 | Open: remaining acceptance fixtures (including F2/F4 historical order), retail search coverage and exact offset table. |
| M11 | Partial: node minimum corrected to 20 bytes, FARM/AZ malformed-byte coverage, graph endpoint/record/class validation and allocation preflight, format docs updated. NAV-013, passage kinds and unknown-field semantics remain open. |
| M12 | Open: replace geometric lift-end inference with recorded state. |
| M13 | Open: reviewed class/ability source and actor footprint mapping. |
| M14 | Unknown map-backed lock setters now do nothing. TUPO handle bounds remain open. |
| M15 | Removed silent path truncation and invented corner-walk fallback; reconstruction is charged and an invalid corner walk fails. |
| M16 | Open: default-sector placement and layer consistency. |
| Low | Removed the unused zero shift and formatted the tree. Legacy grid construction/diagnostics remain; `debug.nav` now identifies the active backend and exposes real layered state. |

All specification gates G1–G7 and G9–G11 remain open. G8 now has data evidence for CAVE's
building count and element-index identity on all 39 local missions; it does not clear the
building-state behaviour. No proposed numerical, scheduling or canonical-order departure is
represented as approved by the maintainer.

## Verification scope

The new `navigation` scenario is generated geometry: an obstacle/corner graph and an ordinary
passage between layers. Its canonical-input tests cover movement, refusal, cancellation,
queued/completed/moving snapshots, both crossing phases, corrupted snapshot rejection and a
replay in a fresh engine process. Separate core tests cover patrol completion and budget refusal.

Retail checks cover RHP parsing, CAVE references, the first mission's initial walk and snapshot
continuation. They do not establish a complete campaign, stairs or building entry/exit. The older
first-mission briefing test's exact assumption list also needs reconciliation with the new walk
lifecycle and the incomplete navigation taint registry; it must not simply be loosened to pass.

Final local checks for this step (Windows, copied game data, debug engine explicitly selected):

- `cargo build -p opensherwood-app`, fmt check and workspace Clippy with warnings denied: passed.
- `cargo test --workspace` with game data: 263 passed, including all nine RHP maps and 39 CAVE/reference checks.
- Python synthetic harness: 45 passed, including 23 navigation cases.
- Retail initial walk (including invalid-target refusal and snapshot during actual movement) and
  mission snapshot continuation: 2 passed.
- Synthetic cross-process golden digest: regenerated after explicit schema bumps, then matched.
- Script tooling: 14 passed; skill mirror check and asset-policy checks passed.
- Separate briefing audit: **1 failed** at the exact assumption-list assertion; expected
  `walk_completion` is absent after 600 ticks. Other briefing assertions passed. This is still open
  together with M5; the test remains enabled and unchanged.

These results cover this Windows checkout only. They do not replace the unresolved review gates
or the missing cross-platform and complete-campaign verification. The integration remains uncommitted.
