# Codex spec re-review 47: docs/original/spec-ai-combat.md (AI and combat, revision 7), 2026-09-18

Spec reviewer role (ADR-0009), with access to `re/`. To be answered in the spec's next revision.

Reviewed revision 7 at `9d34ab6`, blob `959c434e349b9ed92f205887eb4c1cbcd7d09dc2`, against the decompilation, available listings, AI notes and targeted disassembly.

**Review 42’s two findings are closed.** AI-166 and its four fixtures now distinguish posture 10 correctly. Section 8 correctly separates script execution, element updates and queued processing, including native 102’s zero decision draws. RNG-policy ratification remains explicitly outstanding.

1. **High — AI-187, AI-186, AI-082(f), AI-174(b), native 89: tied and carried states are reversed.**  
   **Addresses:** `0049bb20`, `00498f89–00498f9e`, `0049a694–0049a6d7`, `0048af40`, `00579b10`, `00471c00`.  
   Order state **7 is carried; 18 is tied**. The cited `0049bb20` releases a carried body and restores its saved state; its literal 7 is not an order-state assignment. The actual tying completion assigns 18. Consequently, native 89 tests tied, and the state-7 search and fit-again exclusions concern carrying.  
   **Correction:** repair every occurrence, including exclusions and clearance claims. Specify tying from its executor. Add separate tied, carried and put-down fixtures, covering native 89, search eligibility and stun-floor behavior.

2. **High — AI-187: player searching uses the wrong money recipient.**  
   **Addresses:** `0048d510`, `00497cd1–00497d20`, `00450b60`.  
   `0048d510` supplies the NPC search effect. Player search credits the shared player money accounting through `00450b60`, then empties the victim’s purse; it does not add the amount to the searching character’s NPC purse field. Both standing and crouched search reach this effect.  
   **Correction:** distinguish NPC and player search. Test the shared money total, emptied victim purse, crouched search and a repeated search.

3. **High — AI-189: planned actions are misidentified as automatic actions on arrival.**  
   **Addresses:** `004df070`, `004ccad0`, `00490da0`, `00490cd0`, `004904c0`, `005460c0`.  
   The modified bow path stores a shot in a selected action slot; it does not construct the asserted movement-then-shot sequence. The context path likewise stores a deferred target action. A separate operation executes the selected slot. This contradicts both “on arrival” and “no other queueing.” The cited messages also do not establish the claimed key-press/release contract.  
   **Correction:** specify preparation, slot replacement, execution trigger, cancellation and target revalidation separately. Include pending plans in snapshot state. Keep the physical key binding unresolved until traced.

4. **High — AI-188: cancellation priority and state meanings are wrong.**  
   **Addresses:** `004de840`, `004cd400`, `00485e00`.  
   Right-click first cancels action-planning mode, before testing combat. It therefore does not invariably issue block when a selected character is fighting. The ordinary cancellation handler distinguishes order state from posture; its branches do not support the stated crouched/aiming/turning descriptions. Bow and shield modes also have specific cancellation behavior beyond clearing the icon.  
   **Correction:** replace the guessed labels with verified behavioral cases. Include planning-during-combat, carrying, shield and bow cancellation fixtures, and distinguish cancelling preparation from stopping an executing order.

5. **High — AI-191: circle geometry, crossing fallback and jab targeting are incorrect.**  
   **Addresses:** `00547600`, `00547f00`, `00548050`, `00548120`, `00548280`, `004e1a40`.  
   The circle test compares the start and end directions from the stroke’s bounding-box centre, not the chord against character facing. A self-crossing stroke that fails the finishing and circle tests does **not** proceed through half-circle, straight and lateral recognition. A drawn backward jab targets the first adversary; pointer targeting belongs to the click path.  
   **Correction:** state these observable distinctions and the strict threshold comparisons. Preserve the supported one-sided 0.3 deviation rule, but add boundary fixtures around its stored floating value and the 45°/135° boundaries. Add crossing-fallback, facing-independent circle and click-versus-drawn-jab fixtures.

6. **Medium — AI-155: tower-alert recipients, distance and sequencing are overstated.**  
   **Addresses:** `0043c1a0`, `005fc700`, `005fc6d0`.  
   Candidate soldiers must be **rank 0**. The routine does not apply the claimed same-side filter at candidate enumeration. Both distance helpers receive unit scaling: the radius test is strictly inside 500 px without the asserted elliptical scaling. Calls precede the queued turning/animation sequence, and the arm-raise portion is conditional.  
   **Correction:** distinguish candidate selection from receiver acceptance, restore the rank restriction and metric, and describe the conditional animation. Test rank-1 exclusion, exactly-500 exclusion and recipient acceptance.

7. **High — AI-156: rally direction, allocation order and the twenty-soldier limit are wrong or incomplete.**  
   **Addresses:** `0043b1b0`, `0041a010`, `00443fa0`.  
   Recruitment uses the same unscaled radius test. Twenty limits accepted recruits, not necessarily attempted calls. Accepted soldiers are ordered by decreasing distance before competing for points. The outdoor starting direction derives from the aggregate directions toward recruited soldiers, not the officer’s facing. Indoor route failure also has outcomes missing from the claim.  
   **Correction:** specify these allocation results and failure cases. Replace the “cost order” fixture with one where two recruits compete for the same nearest point; distinguish facing from recruit distribution and refused calls from accepted recruits.

8. **Medium — AI-154: the principal-enemy exception includes holder registration.**  
   **Addresses:** `00424d50`, `00447030`, `0049fbb0`.  
   Holder registration occurs on the ordinary guard branch. The principal-enemy branch takes a different action path without that registration. The claim currently exempts only the remark/stance, incorrectly extending the holder and shadow-suppression effects to both branches.  
   **Correction:** separate ordinary guards, principal enemies and an already-held player. Restrict the existing fixture to the ordinary guard case and add a distinguishing holder-registration fixture.

9. **Medium — AI-129: two colleague-found outcomes are incorrect.**  
   **Address:** `00444bb0`, particularly `00444ebc`.  
   With a pending synchronization check and an active program, a partner outside the default state causes immediate program continuation. It does not take the stated waiter/fallback path. An officer already lecturing, or encountering an already-sent colleague, can return without the claimed remark and synchronization work.  
   **Correction:** document both outcomes and test that they neither register a waiter nor issue an approach order.

10. **Medium — AI-128: civilian memory and zone conditions need qualification.**  
    **Address:** `0040d440`.  
    The zone test concerns the observing civilian’s sector. The enemy sighting replaces remembered position with priority 5 only when the existing priority is below 6. The reaction also issues its initial approach before the later reporting timer; the prose omits that movement.  
    **Correction:** distinguish current target position from priority-controlled remembered position, identify whose zone is tested, and include the immediate approach. Add a fixture preserving an existing priority-6 memory.

11. **Medium — AI-102b: placement conflates soldier/civilian initialization with hostile/friendly sides.**  
    **Addresses:** `00435c80`, `0040de30`, `00419e10`, `00414550`, `00435bd0`.  
    The two initializers have different actor-class rules. Soldier registration filters opposing-side non-civilians; it does not universally register every player. Civilian registration has its own side-dependent rule. The extra 20–59-frame timer draw is conditional on eligible initial state and absence of a usable rail. The glance also has additional placed-state guards.  
    **Correction:** separate actor class from allegiance and state precisely when initialization consumes the draw. Cover friendly soldiers, both civilian attitudes, usable rails and excluded initial states.

12. **High — AI-101: the reset preservation requirement is unsupported.**  
    **Addresses:** reset call sites in `00424d50`, `00444bb0`, `0043a220`, `00435c80`, `0040de30`.  
    Inspecting callers does not establish that an unresolved callee preserves targets, remembered position or priority. “Implementer must therefore not clear” turns an acknowledged unknown into a mandatory behavior.  
    **Correction:** remove that requirement until the reset implementation or a distinguishing observation establishes it. Specify verified entry/exit effects separately from `Assumption::ResetBody`; reopen the claimed memory-preservation clearance.

13. **Medium — AI-153: the cadence probability and deterministic fixture omit attack rejection conditions.**  
    **Addresses:** `0043db20`, `00485240`, `00485590`.  
    A jab is not guaranteed merely because a target lies inside its reach band. Incoming-animation timing and accumulated figure penalties can prevent selection. Fixing the three listed rolls therefore does not guarantee a jab at frame `f + 20`, and the 0.49 estimate is conditional on reaching a viable scoring result.  
    **Correction:** qualify the estimate and fully establish the fixture’s facing, class row, penalties and incoming-animation state. Add a passing-roll case where no attack is selected.

14. **Medium — AI-192: a generic activation handler does not establish straw-target behavior.**  
    **Addresses:** `004bb240`, `004baf00`, `004baf50`, `004bb3a0`.  
    `004bb3a0` chooses activation according to object capabilities. It does not establish that the particular straw target is activation-only. The object hit test has a bow-intent capability path, contradicting the categorical “not a bow target through the click.”  
    **Correction:** separate generic object activation from the actual target’s capabilities. Withdraw activation-only clearance until the placed target is identified; retain the unexplained 186 px refusal as unresolved evidence.

15. **Medium — AI-151 / §9.3: the asserted observable repertoire lacks observational support and overgeneralizes the rules.**  
    **Address:** `00485240`.  
    “Low-tier soldiers only jab and cut sideways” conflicts with the documented non-zero-hesitation circle exception. The tier progression is presented as something observed without the recordings and controlled conditions needed to support it.  
    **Correction:** label independently chosen progression as an OpenSherwood assumption, or supply actual repertoire observations with their conditions. Under the expression filter, do not replace the excluded tuned table with an asserted ordering derived from that same table.

The automated asset check passes. The order priorities, geometric recognition conditions and necessary continuation state can pass the manual expression filter as functional results; they should not prescribe recovered function decomposition or internal storage. Section 9.3 needs the correction above before its new repertoire account is cleared.

The updated clearance table is:

| Component | Cleared individual contracts | Still withheld |
|---|---|---|
| Perception | Previously reviewed lifetimes, caching, staggering, rounding and fixtures | Existing geometry/gate exclusions; placement and memory corrections |
| Hearing | Previously reviewed noise rules; child-only whistle predicate | Existing oracle discrepancies |
| State machine | Earlier reviewed transitions, callbacks and guards; beggar’s ranged-weapon predicate | Findings 6–12; remaining reset and acceptance boundaries |
| Rails | Cursor/opcode rules, cached rolls, waiter ordering; wait and frozen-timer fixture contracts | Colleague synchronization correction; existing search exclusions |
| Orders | AI-185 movement contract; supported portions of mode/intent dispatch | Findings 1–5; untraced action executors |
| Combat | Reviewed scoring, decision draws, queued delivery, sweep ownership; corrected AI-166 and four fixtures | Gesture targeting, cadence and admissibility corrections |
| Energy | Costs, recovery amount, cadence and movement guard; restore-phase fixture | Existing in-fight recovery discrepancy |
| Knock-out | Thresholds, decay phase, coma behavior and AI-174(c)’s identified allow-flag callers | Reversed body-state meanings; remaining body-action effects |
| Bow | Reviewed aim boundaries, penetration and immunity behavior | Planning and target corrections; interpolation and record-selection exclusions |
| Snapshot / replay | Test 16’s eight bounded continuation scenarios | Pending-event, unfinished-sweep and collapsing-cone coverage; consumed-roll counterpart; planned-action state |
| RNG | Corrected original execution-order account | Proposed stream policy requires ratification; integrated seeded/hash expectations remain withheld |

**These components do not yet form an implementable whole for playing a mission with the navigation and VM specifications.** The incorrect order/body contracts and unresolved action effects interrupt that whole. The eight restore fixtures improve coverage but do not establish integrated replay clearance or mission completion.

**Verdict: fix-then-clear.**