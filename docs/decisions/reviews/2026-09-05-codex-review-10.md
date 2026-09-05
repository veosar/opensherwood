# Codex adversarial review 10: commits 842407e..1d4d428 (2026-09-05)

Disposition in `2026-09-05-codex-review-10-disposition.md`.

The range is not mergeable. Review-9 findings 1–5 and 10 remain open in adversarial cases; the clean-room history gate is still explicitly unresolved.

1. **[CRITICAL] The committed test demonstrates an untainted alert-policy win.** `ALERT_TIMEOUT_TICKS` is explicitly a hypothesis ([ai.rs:295](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/ai.rs:295)), yet `charge` writes that timeout, the alert state, origin and destination without recording `AlertPolicy` ([ai.rs:1057](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/ai.rs:1057)). The constructed native-97 test then asserts that the 320-pixel charge wins with an empty assumption set ([vm.rs:5811](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/vm.rs:5811)). Whether hearing at that distance is measured is irrelevant: the same transition stores the hypothesized five-second policy. This directly contradicts the “recorded where the rule first mutates state” contract and review-9 disposition row 1.

   **Fix:** record `AlertPolicy` before `charge` mutates state. If immediate charging is intended to remain measured, split the source into measured `NoiseCharge` and hypothesized `AlertTimeout`/return policy; the 320-pixel test must no longer expect `[]`.

2. **[CRITICAL] The clean-room history gate is still open.** The disposition itself acknowledges that reachable history remains non-compliant ([review-9 disposition:9](/C:/Users/przem/source/repos/opensherwood/docs/decisions/reviews/2026-09-05-codex-review-9-disposition.md:9)), while ADR-0003 forbids asset-derived lookup tables in commits ([ADR-0003:39](/C:/Users/przem/source/repos/opensherwood/docs/decisions/ADR-0003-clean-room-roles.md:39)). No commit in this range changes that fact.

   **Fix:** maintainer-approved history rewrite removing the previously named blobs from every reachable branch and tag, followed by a history-aware asset scan. Until then, this cannot pass the repository’s definition of mergeable.

3. **[HIGH] Fused calls still have a result-fabrication bypass on stack overflow.** Validation checks only that `dst` names a value-returning callee ([vm.rs:657](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/vm.rs:657)); recursion is accepted. At runtime, when the 64-frame limit is reached, `push_frame` merely increments a diagnostic and returns false ([vm.rs:3120](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/vm.rs:3120)). The caller then advances without writing `dst` ([vm.rs:3001](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/vm.rs:3001)).

   A validated `CheckVictoryCondition` can therefore do `t0 = 1; t0 = recurse(); return t0`. The deepest failed call leaves `t0 == 1`; that value unwinds and sets `mission_won`. The old standalone-reader bypass is gone, but its fabricated-result effect survives through a different runtime path.

   **Fix:** stack overflow must produce a sticky `CallStackOverflow` fault and abort/roll back the callback. Alternatively reject recursive call graphs. Add a validated recursive fused-call test that attempts to win, including snapshot restore.

4. **[HIGH] A partially funded state search is not retried and can permanently lose an AI action.** `walk_to` documents `Walk::Exhausted` as “state unchanged; retry next tick” ([ai.rs:964](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/ai.rs:964)), but:

   - `charge` mutates the alert state and ignores the result ([ai.rs:1057](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/ai.rs:1057)).
   - `Alarm → Alerted` mutates first and ignores the result ([ai.rs:1135](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/ai.rs:1135)).
   - `end_fight` treats an unpaid search as unreachable and resumes patrol ([ai.rs:1531](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/ai.rs:1531)).

   Construct a guard late in the states walk with less than `SIM_SEARCH_WORK` remaining, `Alarm`, `state_ticks = 1`, and a distant `last_seen`. It becomes `Alerted` with no target; the cursor moves on, and with no new stimulus the search is never retried.

   **Fix:** make transition-plus-path-planning transactional, or propagate `Exhausted` to the phase loop and keep the cursor on that entity. Retain a retry state for post-fight returns.

5. **[HIGH] A valid movement query can cost more than the entire movement quota and therefore make no progress forever.** The geometry limit and movement quota are both exactly `2^20` ([world.rs:632](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/world.rs:632), [world.rs:820](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/world.rs:820)). Geometry consumes one unit per edge ([geom.rs:55](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/geom.rs:55)), but movement has already charged the entity and at least one obstacle-index cell before reaching it ([world.rs:2355](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/world.rs:2355)). An obstacle query may independently traverse up to `2^22` accepted index entries.

   When earlier phases consume their carry, an accepted worst-case mover cannot finish its atomic query. `resume_at` rotates past it but stores no inner cell/candidate/edge cursor ([ai.rs:788](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/ai.rs:788)); when its turn returns, the query restarts from zero. The maximum-size test covers many cheap queries, not one maximum-cost query.

   **Fix:** ensure every accepted atomic query is strictly below the minimum movement grant, or persist inner query progress. Test maximum geometry and maximum index occupancy together with earlier phases consuming all carry.

6. **[HIGH] Negative obstacle extents are a validator bypass that makes obstacles fail open.** Validation requires exactly one extent entry but does not validate either extent ([world.rs:1660](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/world.rs:1660)). Index construction applies `abs()` when choosing cells ([world.rs:940](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/world.rs:940)), but collision testing uses the original signed values in `hw + size` and `hh + size` ([world.rs:1042](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/world.rs:1042)). Thus a restored snapshot containing negative half-extents validates and indexes the obstacle, but movement passes through it.

   **Fix:** validate non-negative, bounded obstacle half-extents and normalize them before storage. Add hostile JSON tests for negative and maximum raw extents.

7. **[HIGH] Combat permits non-reciprocal live fights and multiple attackers through a pairwise `foe` API.** When a second hero attacks an already engaged guard, `start_fight` overwrites the guard’s foe but leaves the first hero fighting that guard ([ai.rs:1276](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/ai.rs:1276)). `fight_tick` checks only whether the referenced foe is alive, standing and close—not whether it points back ([ai.rs:1325](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/ai.rs:1325)). Validation likewise checks only opposing kinds/teams ([world.rs:1893](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/world.rs:1893)). Both heroes can consequently damage the guard while it attacks only the most recent one, and the state survives validation/snapshot.

   **Fix:** either detach the old reciprocal pair before reassignment or introduce an explicit multi-attacker model. Require reciprocity for two living engaged actors and add two-hero/one-guard snapshot and replay tests. Multi-party policy also needs its own assumption because the measurements explicitly did not cover it.

8. **[MEDIUM] The drawn-figure target is selected on release, not locked while held as measured.** The observation says the nearest enemy is outlined and locked while the button is down ([combat-measurements.md:76](/C:/Users/przem/source/repos/opensherwood/docs/original/combat-measurements.md:76), [combat-measurements.md:186](/C:/Users/przem/source/repos/opensherwood/docs/original/combat-measurements.md:186)). The engine stores only the press position and calls `figure_order` on release, where it scans for the then-nearest enemy ([world.rs:1999](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/world.rs:1999), [world.rs:2224](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/world.rs:2224)). If targets move or exchange distance during the gesture, the wrong guard is attacked; no held-target outline exists.

   The “combat as measured” roadmap claim also omits the observed occasional 25-hp event while tests enforce only 5-hp damage ([combat-measurements.md:106](/C:/Users/przem/source/repos/opensherwood/docs/original/combat-measurements.md:106), [ai.rs:365](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/ai.rs:365)).

   **Fix:** snapshot/hash a held figure target and render its outline, or explicitly leave lock-on unchecked. Keep the melee milestone qualified until the larger blow is measured and modeled.

9. **[MEDIUM] Replay coverage promised by the disposition is absent, and review-9 finding 10 still violates AGENTS.md.** The taint test calls parallel snapshot stepping “replay” ([vm.rs:5784](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/vm.rs:5784)); the combat data test does the same for one restored tick ([test_mission.py:640](/C:/Users/przem/source/repos/opensherwood/harness/tests/data/test_mission.py:640)). Neither records and plays a `Replay`. `test_win.py` also performs only the live run.

   Separately, the won/lost flow tests still mutate outcomes using `debug.vm` ([test_script.py:465](/C:/Users/przem/source/repos/opensherwood/harness/tests/data/test_script.py:465), [test_script.py:498](/C:/Users/przem/source/repos/opensherwood/harness/tests/data/test_script.py:498)), contrary to the canonical-input rule ([AGENTS.md:31](/C:/Users/przem/source/repos/opensherwood/AGENTS.md:31)). Their docstring is now demonstrably stale: it says no mission can be won through play.

   **Fix:** add actual `replay.start`/`stop`/`play` regressions for the hypothesis win and combat; cover cursor/index behavior with deterministic input reproduction. Drive UI flow tests through the new canonical win/loss paths or a synthetic script activated by canonical events; keep `debug.*` read-only in behavioral tests.

10. **[MEDIUM] The documentation/provenance closure remains inaccurate.** Direct manual/UI wording remains quoted in the combat report ([combat-measurements.md:40](/C:/Users/przem/source/repos/opensherwood/docs/original/combat-measurements.md:40), [combat-measurements.md:76](/C:/Users/przem/source/repos/opensherwood/docs/original/combat-measurements.md:76)) and H01 report ([h01-win-path.md:111](/C:/Users/przem/source/repos/opensherwood/docs/original/h01-win-path.md:111)). The Sherwood provenance records method/build/tests but not an explicit “who” attribution ([sherwood-hub.md:321](/C:/Users/przem/source/repos/opensherwood/docs/formats/sherwood-hub.md:321)). `movies.md` makes third-party licensing/relicensing claims without citing the upstream license or offer ([movies.md:23](/C:/Users/przem/source/repos/opensherwood/docs/formats/movies.md:23)); its provenance links only the codec description. I found no designer names.

   **Fix:** paraphrase the remaining quoted wording, add an explicit analyst/session attribution to the hub provenance, and cite or remove the NihAV licensing/relicensing assertions.

The protocol-6 restart-money, strict format-envelope, signed HUD, option-bar and hover changes appear to close review-9 findings 6, 8 and 9. The new combat fields and all seven phase cursors are serialized and hashed; the obstacle index is correctly treated as deterministic derived state. The remaining problems are provenance/liveness/semantic gaps, not simple missing hash fields.

`git diff --check 842407e..1d4d428` passed. I did not run the suite because the live worktree acquired concurrent uncommitted edits in core files after the range was frozen, so it would not have tested the requested committed endpoint. The required cross-agent review was attempted but could not connect through the environment firewall.

**Verdict: redesign.**