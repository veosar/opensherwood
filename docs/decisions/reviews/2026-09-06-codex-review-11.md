# Codex adversarial review 11: commits 1d4d428..bce71e5 (2026-09-06)

Disposition in `2026-09-06-codex-review-11-disposition.md`.

Review frozen at `bce71e5`; later uncommitted edits in six core files were excluded. All references below are to that committed endpoint.

1. **[CRITICAL] The clean-room history gate remains open, and this range adds new prohibited material.** The disposition explicitly leaves the earlier asset-derived history unresolved (`docs/decisions/reviews/2026-09-05-codex-review-10-disposition.md:10`). The new `H01_ITEMS` constant reproduces the mission’s complete 11-record item table, including coordinates, kinds, stacks, and activation state (`harness/tests/data/test_mission.py:855-922`), despite ADR-0003 forbidding lookup tables copied from assets (`docs/decisions/ADR-0003-clean-room-roles.md:39`). The analyst report also commits verbatim UI/tool-tip wording at `docs/original/h01-measurements-2.md:31`, `:56`, and `:145`, contrary to the no-text rule in `docs/legal.md:20-24`; another quoted mission-text fragment appears at `docs/formats/rhm.md:183`.

   **Fix:** perform the already-required maintainer-approved history rewrite across every reachable branch and tag; include this newly added table and verbatim text. Replace the table with synthetic fixtures or narrowly scoped structural/aggregate assertions against the player-owned file, and paraphrase all displayed wording. The provenance section exists and I found no newly introduced designer names, but provenance does not make copied assets permissible.

2. **[CRITICAL] Stack overflow still permits an untainted mission win because ordinary callbacks are not rolled back.** `vm_tick` ignores the outcome of each `Hourglass` callback and continues to victory evaluation (`crates/opensherwood-core/src/vm.rs:2704-2709`, `:2725-2738`). `vm_invoke` only tears down frames (`vm.rs:3092-3097`); rollback is installed solely around queued `ActionChange` handlers (`vm.rs:2555-2586`). Consequently, an `Hourglass` can set `cv0 = 1`, recurse until `push_frame` faults at `vm.rs:3320-3323`, and abort while leaving `cv0` set. The same tick’s `CheckVictoryCondition` can return `cv0`, setting `mission_won` with an empty assumption set. The existing test at `vm.rs:6938-7037` covers untouched call destinations and the transactional action-handler case, not persistent effects of an ordinary callback.

   **Fix:** make every callback transactional on `Aborted`, including class/mission variables, native effects, entities, selection, camera, queues, and sequences; alternatively make the first VM fault terminal before any later callback, though that does not satisfy the disposition’s rollback promise. Add an `Hourglass → overflow → CheckVictoryCondition` regression covering the same tick, later ticks, snapshot/restore, and native side effects.

3. **[HIGH] A one-tick noise stimulus is still lost when `charge` cannot fund its path search.** Perception stores the stimulus only in the current tick’s local vector (`crates/opensherwood-core/src/ai.rs:937-956`), which is passed to the transition once at `ai.rs:965-983`. When `charge` returns `false`, it stores no pending stimulus (`ai.rs:1112-1117`). The test at `ai.rs:2803-2827` succeeds only because the hero remains running during the next tick. If the detection occurs on the final tick of a run, movement completes later that tick and changes the gait; the next perception pass sees no noise, so the guard remains in `Patrol` permanently despite the cursor retry.

   **Fix:** snapshot/hash a pending transition containing the position and channel, or otherwise reserve enough work before consuming the stimulus. Add a test where the run completes or is cancelled immediately after the underfunded transition, including snapshot and ReplayV1 continuation.

4. **[HIGH] The committed item model contradicts the measurements added in the same range.** The report observes sprite-bound hit testing, arrival within 0–8 px, and a 0.6–0.7 second stoop (`docs/original/h01-measurements-2.md:37-49`, `:68-78`, `:189-194`). The endpoint instead:

   - hit-tests a 12 px circle centred on the record base (`crates/opensherwood-core/src/world.rs:717-721`, `:2183-2200`);
   - takes the item immediately at the old 24 px scroll radius (`world.rs:2203-2247`);
   - has no authoritative stoop phase or timer to snapshot/hash;
   - loads only the first frame of each item block, so the measured sparkle animation is absent (`crates/opensherwood-app/src/engine.rs:52-92`);
   - renders the observed kind-8 pouch as an unknown placeholder (`crates/opensherwood-render/src/lib.rs:699-703`, `:766-772`).

   The existing item fields—kind, stack, taken set, counters, and pickup handle—are serialized and hashed, but they encode the obsolete behavior.

   **Fix:** introduce a hashed/snapshotted pickup phase and timer, use the measured item arrival and stoop duration, hit-test the sprite area above the base, and keep unmeasured purse effects tainted separately. Add mid-stoop snapshot and ReplayV1 tests and bump ruleset, snapshot, and hash versions.

5. **[HIGH] Fight reciprocity is still not an invariant, and a second attacker can bypass the wait policy.** Validation rejects mismatched foes only when the referenced opponent is also already `Fighting` (`crates/opensherwood-core/src/world.rs:2061-2074`). A living hero in `Fighting` naming a living guard in `Patrol` with `foe = None` therefore validates and survives snapshot/restore. Separately, when an engaged guard is approached from behind, the second attacker waits only when `!punching`; a punching attacker falls through and knocks the guard down (`crates/opensherwood-core/src/ai.rs:1344-1355`, `:1368-1372`). That contradicts the disposition’s “others wait at reach” rule.

   **Fix:** require every living, active `Fighting` entity’s opponent to be `Fighting` back with the reciprocal ID. Make the engaged-victim branch stop unconditionally, including behind/punch attacks, unless an explicit multi-attacker state is introduced. Add hostile snapshots for a patrol/returning opponent and a two-attacker test with the second attacker behind.

6. **[MEDIUM] Replay and end-to-end regression claims remain overstated.** The two-hero “replay” merely reapplies a `Vec<InputEvent>` directly to another `World` (`crates/opensherwood-core/src/ai.rs:3049-3054`); the harness fight and item tests only exercise snapshots (`harness/tests/data/test_mission.py:842-851`, `:981-989`). The actual ReplayV1 test at `test_mission.py:320-374` covers alert timeout and figure-target locking, while the win replay covers a scroll—not item or fight state. The successor test accepts any mission other than H01 (`harness/tests/data/test_win.py:79-95`). The minimap test merely searches the entire asset-backed area for three RGB values and does not check coordinates, red enemies, camera bounds, movement, or disappearing pickup crosses (`harness/tests/data/test_menu.py:251-260`).

   **Fix:** add real `replay.start/stop/play` cases with checkpoints during item pickup and reciprocal combat; assert the exact documented successor; test minimap pixels at calculated marker positions using a synthetic blank image, including inactive/taken markers and all colors.

7. **[MEDIUM] Documentation describes mutually incompatible endpoint states.** The H01 walkthrough still says native 235 is a stub and `ZORG` is unmodelled (`docs/original/h01-win-path.md:110`), while its work list says item pickup is done (`:222`). The format specification says pickup remains unverified and item sprites are not drawn (`docs/formats/rhm.md:192-194`). The roadmap still lists minimap markers as missing (`docs/roadmap.md:47`), and the engine documentation says campaign progression is future work (`crates/opensherwood-app/src/engine.rs:565-566`).

   **Fix:** synchronize the walkthrough, format specification, harness documentation, roadmap, and engine comments with the exact committed behavior and clearly distinguish measured behavior from the current known-mismatching implementation.

Review-10 closure status: rows 1, 5, 6, and the lock-on portion of 8 appear closed; row 2 remains explicitly open; rows 3, 4, and 7 are reopened above; row 9 is only partially closed; row 10 is reopened by newly committed verbatim text. `debug.vm` is inspection-only as claimed.

`git diff --check 1d4d428..bce71e5` passed. The asset scanner passed but explicitly cannot detect copied text. I could not run the build/data suite against the frozen endpoint because the worktree acquired concurrent changes, the filesystem is read-only, and no game-data directory is configured. The required independent Claude review was attempted but blocked by the environment firewall.

**Verdict: redesign.**