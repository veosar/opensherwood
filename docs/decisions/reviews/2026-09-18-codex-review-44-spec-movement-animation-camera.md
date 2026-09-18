# Codex spec re-review 44: docs/original/spec-movement-animation-camera.md (movement, animation and camera, revision 5), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`. To be answered in the spec's next revision.

Reviewed revision 5 at `002314d`, blob `d3f6c8e346b160571e51bd6c9ae714780188c176`. Checked the decompilation and notes, with direct instruction checks against the matching executable where the requested listings were absent.

1. **High — ANIM-209 — `005b86b0`, specifically `005b8e78–005b8e80`.** The newly described mode-5 look-ahead compares **action IDs**, not targets or destinations. Correct “the same target” and the explanation that the successor continues to the same place. Keep the full completion contract excluded through `WaypointRecords`.

2. **Medium — ANIM-212; test 14 — `00464b20`, `0055f0f0`, `0055f210`.** The 296–299 execution count and stop-on-status-3 rule are correct, but the asserted facing changes are not unconditional. A turn call changes nothing when already aligned; the damped variant can also consume calls without changing facing. Specify **one or two turn/play executions**. Require the ordinary variant and sufficient remaining angular distance in fixtures asserting one or two facing changes.

3. **Medium — ANIM-003, ANIM-212; §4.2 — `00464b20`, `00466b75–00466bf7`.** The repeated-execution inventory still omits action **294**. Its ordinary movement path performs two turn/play operations without stopping when the first answers 3. Document that count separately from 296–299, or explicitly exclude it; the present accounting treats it as an ordinary single execution.

4. **Medium — ANIM-003, ANIM-031; §4.2 and §5 — `005b7820`.** The revised `hold + 1` qualification remains too broad. Modes 4/5 can wait at their stochastic start, mode 7 holds its marker according to the rotation counter, and modes 8/9/14 hold their stopping frame indefinitely. Restrict the duration rule to **ordinary hold-driven advancement**, with those state-specific exceptions; a list of modes alone is insufficient.

5. **High — ANIM-005; test 9 — `004ca410`, `00585320`, `00582560`, `00585b70`, `0058ba60`.** Test 9 retains the old assertion that a next level dispatched in the same drain observes the flag already set. Delete it. Also replace “deferred queue … later drain”: a deferred successor can execute **later in the same drain**, after the write. The distinction is synchronous execution before the write versus queued execution after it. Specify the initial flag value when asserting “clear” and “set.”

6. **Medium — ANIM-331, ANIM-334; §6 — `004cdfc0`, `004ce74c–004ce79e`, `00571330`.** The detailed camera correction is supported, but summaries still contradict it. ANIM-331(d) and native 18’s row unconditionally describe an active lock as suspending scroll; carry through the clipped-zero-delta exception. Native 19’s row still calls the next update “a zero step”: the **ramp length** becomes zero, while retained nonzero speed can still move the camera.

7. **Medium — ANIM-330/331; tests 18–19 — `004cdfc0`, particularly `004cea9d–004ceb14`.** Test 19’s new isolation conditions do not establish its exact 2/7/10-pixel steps. Remaining-distance limiting and per-component truncation still apply. Require an axis-aligned destination sufficiently far away for every asserted step. Likewise, test 18’s clipped-lock fall-through establishes that the scroll stage **runs**, not necessarily that it moves; require a nonzero effective, unclipped displacement for that assertion.

8. **Medium — ANIM-001–003; §11.1 — `0050f710`, `005b7820`; implementation commit `ef45066`.** The header correctly acknowledges batch 1, but §11 still describes the “current engine” as using the removed 60/64 Hz conversion and fractional remainder. Mark that comparison historical or update it: the 46.875 ms logic frame is already implemented.

The expression filter **passes**: the revision describes behavioral rules and observable orderings without prescribing recovered implementation structure. The heuristic asset check also passed.

The header’s Assumption handoff is right **for unresolved behavior the next batch uses**. Keep all exclusions visible, record assumptions where a fallback is taken, and do not equate that with clearance. An omitted feature—such as slow motion—does not require inventing a runtime assumption.

| Component | Updated clearance | Remaining boundary |
|---|---|---|
| Frame timer | Core arithmetic, resets and entry rules cleared for supplied values; mode-7 read confirmed | Finding 4; marker mapping, initial state, overrides and out-of-range consequences |
| Idle chain | Next-update start and wait-element RNG exclusion cleared | Other substitutions remain excluded |
| Movement modes 0–3 and 6 | Per-call displacement arithmetic and conditional extra steps cleared | Execution accounting; mode-3 caller, arrival and collision dependencies |
| Turning | Turn variants and ordering cleared | Findings 2–3; unresolved initial counters |
| Collision | Failure-region accounting and established comparisons cleared | Sliding, rollback and remaining arrival geometry |
| Proximity | Layer/sector guards, inclusive containment and character-only directional product cleared | Eligibility, query geometry and reactions |
| Camera | Conversion, native writes, clipped-zero fall-through and zoom completion preserving scroll cleared | Findings 6–7; ramp, edge scrolling and full follow behavior |
| Zoom | Request/refusal rules and established transition facts cleared | Accepted-request completion timing |
| Drawing order | Established pass order, actor comparison and ground-mark ageing cleared | Scenery boundaries, family coverage and mask integration |
| Carts | Previously cleared facts retained | Initial state, program and sub-sprite exclusions |

These components are **not yet cleared as an implementable whole**. Individual cleared rules can be implemented within the stated boundaries.

The table above is the review update; no repository files were changed because this session is read-only.

**Verdict: fix-then-clear.**