# Codex spec re-review 22: docs/original/spec-ai-combat.md (AI and combat, revision 2), 2026-09-13

Spec reviewer role (ADR-0009), with access to `re/`. Answered in the spec's next revision.

Reviewed revision 2 at commit `e966b05`, blob `e10208494c95caf92c2b356357329a61cf014c13`, against the decompilation, private notes, and instruction bytes where exports were incomplete. The executable hash matches.

**Both analyst disputes are upheld.** The serialized stimulus order is **purse / apple / beer / whistle** (`0056a7d0`, `00438cb0`, `004468a0`, `0044cb40`, `00424d50`). Native 126 returns **4 for menacing and 5 for fleeing**, with sleeping returning 0 (`00575e20`, state-setting paths in `00424d50`). Review 16 was wrong on those two points.

The following findings remain in [revision 2](C:/Users/przem/source/repos/opensherwood/docs/original/spec-ai-combat.md).

1. **Blocker — AI-068, AI-069; acceptance test 1 — `004890d5–00489124`, `00488231–00488263`.**
   The multipliers are corrected, but the example still loses the rounding stages required by finding 1. With the stated `W = 80` and `g = 0.5`, the recovered calculation stores approximately **0.7999999523162842**, producing contributions **15 normally and 159 in wide mode**, not 16/160. Under the test’s assumed first evaluation on frame 0, ordinary sighting occurs on **frame 66**, not 62. Specify the intermediate precision and storage boundaries, then recalculate the examples. Also specify the global frame and observer ID: reevaluation is staggered, not universally “every 2nd frame from creation.”

2. **Blocker — AI-100; acceptance test 5 — `004386d5–004386eb`, `00642b7c`.**
   The multiplier correction is valid; the numerical examples are not. The passage **“the product is evaluated in single precision”** contradicts the arithmetic retained in floating-point registers before truncation. With the recovered single-precision `0.01` constant and that arithmetic, base 60/initiative 10 gives **54 on medium and 108 on easy/hard**; initiative 0 on medium gives **60**. Define the arithmetic contract explicitly and correct both the prose example and test 5.

3. **High — AI-151, AI-152 — `00485240`, especially `004853a0–004854ed`.**
   The circular-figure exception is reversed. The offending factual statement is **“the circles are always admissible for hesitation-zero … and then earn a +500 score bonus.”** Zero hesitation retains the experience requirement. Nonzero hesitation takes the exceptional eligibility path and receives the bonus, subject to the remaining timing, opponent-count and scoring conditions. Reactive selection also has an additional conditional skill roll for non-NPC actors after the initial attack gate fails; the revision still omits it. Correct these outcomes without restoring the excluded threshold tables.

4. **High — AI-166, AI-007; acceptance test 9 — `0047e7d0`, `00472070`.**
   The later experience roll is conditional, not consumed by every strike reaching the handler. A resolution with no effect exits before it; the rejected-result case and figure identifiers outside the qualifying range also bypass it. Consequently, **“a strike that does not land consumes the same three”** is false: a defended strike with no stun effect skips that later roll. Specify separate cases for damage, stun-only, no effect and rejected targets. Distinguish these particular decision draws from randomness consumed by intervening reactions or sounds.

5. **High — AI-190; section 6.2 — `00410620`, `004032a0`.**
   **“Internal 8 … is never offered”** is not supported. Internal event 8, and other unmapped events, receive mapped value **−2**; the subsequent script-callback invocation has no exclusion for that value. Correct this inherited review-16 error and document the default mapping. Add callback tests covering −2, source-element versus null payloads, implicit receiver context and zero-return suppression.

6. **High — AI-082 and section 6.1 natives 89, 134/135 — `00410620`, `00579b10`, `00576d80`, `00576e00`, `00418b80`.**
   Distinct state domains remain conflated. The fit-again guard compares **order state 7**, and native 89 compares **order state 18**, not sprite-action identifiers. Native 134/135 also **reject player characters**; the branch setting a lock byte belongs to a different actor category. Finally, locking preserves sprite action **127**, rather than generically preserving “idle.” Correct these domains and accepted actor types.

7. **High — AI-176; acceptance test 13 — `0047b8a0`, particularly `0047b947–0047ba43`.**
   Aim geometry remains incomplete and partly wrong:
   - The horizontal test scales the y displacement by approximately **1.33**; it is not ordinary horizontal Euclidean distance.
   - Both height branches use a **strict** reach comparison.
   - In long mode, the downhill reach doubles **the base reach plus the height extension**, not just the base reach.
   - “tan(a fixed angle)” leaves a required constant unspecified; the recovered angle is approximately **0.30000001192092896 radians**.

   Correct the formula and give numeric boundary cases for horizontal direction, equality and downhill long mode.

8. **High — AI-178; acceptance test 12 — `004859b0`, `004a18c0`, `004a68e0`.**
   The corrected deflection comparison is supported, but **“otherwise one roll … the target’s class head word 10”** still overgeneralises. Civilian targets use the target-permission predicate without the class-based penetration roll. This matters because the same section permits player arrows to affect civilians on hard. Specify target eligibility, principal-enemy immunity and class-based deflection as distinct cases, including their RNG consumption.

9. **High — AI-170; acceptance test 14(d) — `00471b00`, `00475bd0`.**
   **“A stationary duel … expect one pixel per 64 frames … for both fighters”** contradicts the revised rule twice. Passive recovery requires **no adversary**, and the cited soldier’s rate gives one pixel per **320 frames**, versus 64 for the hero. Keep an out-of-combat recovery test separate from an exploratory stationary-duel recording. The observed recovery discrepancies remain unresolved evidence, not acceptance expectations.

10. **Medium — AI-040, AI-174; acceptance test 11 — `00471b00`, `00471c00`.**
    The initial-phase description is too strong. Becoming unconscious reloads the counter only when it is zero; a nonzero residual counter is preserved. During decay, an already-zero counter triggers the decrement and reload, which explains the `t + 1` period. Specify that distinction and initialize the counter explicitly in test 11 before asserting exactly 4,615 frames.

11. **High — AI-003, AI-050, AI-082; acceptance test 4 — `0048a980`, `00471b00`, `00410620`, `004c6ef0`.**
    The execution contract still compresses different pause mechanisms into one rule. Section 3.1 presents perception and AI processing as unconditional, although their pause guard also controls rescanning, cone updates and deafness processing. Section 3.3.3 mentions deadline advancement only while locked/out, although the enclosing alternate path also covers the local pause condition. Distinguish a pause that prevents level ticks from one encountered during an actor tick. Specify timer-arm/check phases and a half-open lock interval so test 4’s expected deadline is unambiguous.

12. **High — AI-007, AI-040–AI-042, AI-050; sections 2 and 8 — `00485240`, `00419c50`, `00419c80`, `00487d00`, `00472070`.**
    The snapshot and determinism contract remains incomplete. Examples include the per-figure usage penalties and cached rail roll, which affect later choices but are missing from the declared snapshot inventory. “Single-precision (or 80-bit intermediate)” does not define reproducible arithmetic. Section 8 identifies alternative RNG policies but does not select stream ownership, draw ordering or the intended equivalence target. Complete these contracts—or explicitly withhold the dependent implementation scope. The original clock facts can be cleared independently of the fixed-tick decision.

13. **High — section 6.1; AI-190 and missing native claims — `005794b0`, `00579520`, `00577500`, `00573450`.**
    The native table remains a summary rather than complete contracts. Several entries lack return/failure behaviour, argument coercion, bounds and direct evidence. For example, native 197 rejects indices outside 0–9 and returns **−1** on failure; native 198 rejects invalid inputs without writing. Native 59 still has an ellipsis for its arguments. Supply complete contracts or list the affected natives as excluded; “none blocks” alone is insufficient.

14. **High — claims registry and exclusions — AI-101, AI-151, AI-173, AI-185; `00424d50`, `00485240`, `00475bd0`, `0046bcb0`, `004de290`.**
    The registry does not consistently distinguish supported claims from excluded dependencies. Examples:
    - AI-185 prescribes cancellation and Ctrl queueing while section 9.2 excludes them.
    - AI-173 states hard-punch scaling while section 9.2 says its expression remains unsettled.
    - AI-101 describes reset-body effects while declaring that body unidentified.
    - Grouped registry rows obscure which individual claims are observed, inferred or unknown.

    Split the grouped claims, use the template’s status vocabulary, and give each excluded dependency an explicit handoff boundary. Remove the unsupported necessity assertion that none of the exclusions decides the first mission’s win path.

15. **High — expression-filter policy; AI-151, section 9.3 — `00485240`, threshold data at `006779b0`, `006779bc`.**
    The copied numeric tables have been removed. However, section 9.3(c) proposes to **“decide that these particular constants are functional facts and admit them individually.”** That is precisely the reconstruction of a tuned table as individual constants that ADR-0009 still refuses. The interface-mapping clarification does not authorize it. Remove that option under the current policy. Any observational alternative must describe necessary observable outcomes rather than reconstruct the same excluded table.

    A smaller structural residue remains in AI-140: **“a ranged handle and a hostile-kind AI.”** Replace the recovered handle requirement with the relevant weapon capability and behavioural condition.

16. **Medium — identity block; address: document-wide.**
    Edition, build, analyst authorization and separate pending publication approval are now recorded. The six delegated readers still lack individually identifiable session records, however, and the reviewer entry identifies a model/review number rather than a session. Record each reader’s identity and exposure, and bind this review to the exact revision/blob. Pending publication approval should remain separate from factual clearance.

The corrected SD/PC/ranged **serialized layouts**, stimulus meanings, category notification lifetimes, hearing action/material corrections, nominal-versus-measured clock distinction, difficulty existence, rail double advancement, mutual-duel gate, and core recovery/stun/deflection boundaries can be cleared as individual facts. The ANIM-523 contradiction is corrected in the sibling specification. Removal of the copied threshold tables and the corrected callback compatibility tokens also pass.

The affected contracts above, section 9.2’s excluded behaviours, exact figure selection under section 9.3, and mission readiness must wait. The 14 acceptance entries need the corrections and explicit initial conditions identified above. The automated asset check passes, but does not settle the expression review.

**Verdict: fix-then-clear.**