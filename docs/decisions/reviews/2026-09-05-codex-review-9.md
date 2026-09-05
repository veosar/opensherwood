# Codex adversarial review 9: commits dc9f006..842407e (2026-09-05)

Disposition in `2026-09-05-codex-review-9-disposition.md`.

Verdict: **redesign**. The ruleset-12 taint and simulation-budget contracts remain unsound, and the clean-room history gate is explicitly unresolved.

I reviewed committed objects only, treating `dc9f006` as inclusive because the requested protocol/UI work is in that commit, through `842407e`.

## Findings

1. **[CRITICAL] AI hypotheses can still produce an untainted win.** The exact 350 px noise radius is explicitly an engine choice, not measured ([ai.rs:184](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/ai.rs:184)), yet noise is declared assumption-free ([vm.rs:1132](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/vm.rs:1132)). More generally, `Perception` is recorded only after an `ActionChange` handler is found; a class without one is skipped ([vm.rs:2252](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/vm.rs:2252)). The existing test even asserts that hypothesis-driven state changes leave `assumptions` empty without a handler ([vm.rs:5558](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/vm.rs:5558)).

   Constructed counterexample: place a running hero in the unmeasured part of the noise radius, let the guard’s resulting charge move it into a polygon, and have `CheckVictoryCondition` read native 97—classified `Observed` ([natives.rs:201](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/natives.rs:201))—and return 1. No `ActionChange` handler is needed, so the mission wins with an empty assumption set. Sight, alert timing and attack-order movement have equivalent dependency gaps.

   **Fix:** record assumptions where a hypothetical rule first mutates authoritative state, independent of callbacks or later consumers. Give sight, exact noise radius, alert policy, attack policy and knockout policy distinct sources. Add no-handler, position-to-victory tests through snapshot/restore and replay.

2. **[CRITICAL] Review-8’s asset-history violation is still open.** The new disposition directly acknowledges that the reachable history remains non-compliant ([disposition:17](/C:/Users/przem/source/repos/opensherwood/docs/decisions/reviews/2026-09-05-codex-review-8-disposition.md:17)), contrary to the hard prohibition on asset-derived tables ([ADR-0003:39](/C:/Users/przem/source/repos/opensherwood/docs/decisions/ADR-0003-clean-room-roles.md:39)).

   **Fix:** purge the named table commits and equivalent objects from every reachable branch/tag, then verify all refs with a history-aware asset scan. This requires the maintainer-approved history rewrite already identified by the disposition.

3. **[HIGH] The result-reader validator bypass survives for ordinary function calls.** The translator protects only native result opcode `0x0d` ([script/lib.rs:385](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-script/src/lib.rs:385)); ordinary `0x0a` remains a standalone `GetCallResult` ([script/lib.rs:550](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-script/src/lib.rs:550)). Validation checks only its destination slot and does not require a preceding value-returning call ([vm.rs:650](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/vm.rs:650)). A frame initializes `call_result` to zero ([vm.rs:3041](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/vm.rs:3041)), so a direct jump into the reader—or reading after a void call—produces a fabricated branch value.

   **Fix:** fuse `Call` and its result destination, validate `callee.has_result`, and reject direct/divergent/loop entries into the old reader. Test hostile translated SCB and restored programs.

4. **[HIGH] The shared simulation budget can permanently starve later phases.** Only per-phase entity cursors are stored ([world.rs:681](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/world.rs:681)); every tick nevertheless starts with perception, then transitions, attacks and programs ([ai.rs:622](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/ai.rs:622)). A valid 65,536-entity snapshot split between players and guards yields over one billion perception pairs against a `2^24` budget. Perception consumes every tick’s budget, so timers, attacks and programs never run. The perception cursor cannot provide fairness between phases.

   **Fix:** snapshot a global current phase and the inner player cursor, or allocate deterministic per-phase quotas. Add a maximum-size hostile snapshot test proving bounded progress for every phase.

5. **[HIGH] Large parts of simulation remain outside the advertised budget.** After the budgeted phases finish, movement scans every obstacle for every moving entity without charging work ([world.rs:1671](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/world.rs:1671), [world.rs:1695](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/world.rs:1695)). A valid half-movers/half-obstacles snapshot performs roughly 1.07 billion AABB checks per tick. Animation and action-change scans are also outside the claimed “everything besides script” budget.

   **Fix:** use a deterministic spatial obstacle index or bring movement, animation and action scanning under persisted phase cursors and the shared budget. Add maximum accepted snapshot regressions.

6. **[HIGH] Restart breaks the canonical starting-money/replay contract.** `current` stores only scenario and seed ([engine.rs:230](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/engine.rs:230)); mission loading consumes `money_override` ([engine.rs:1916](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/engine.rs:1916)), while both restart paths call `reset` without restoring it ([engine.rs:1235](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/engine.rs:1235), [engine.rs:1262](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/engine.rs:1262)). Playback applies the header value only to its initial reset ([engine.rs:2410](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/engine.rs:2410)). A replay containing Restart therefore depends on the profile present during playback.

   The “one signed money type” claim is also false: `HudState.money` remains `u32` ([ui.rs:2165](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/ui.rs:2165)), and negative VM/profile values are rendered as zero.

   **Fix:** store the complete reset descriptor, including actual starting money, and make Restart reuse it. Either keep HUD money signed or reject negative money at every input boundary. Test replay → Restart after changing/reloading the profile.

7. **[HIGH] New documentation does not pass the clean-room/provenance gate.** `movies.md` contains original-file measurements and community-codec claims but has no Provenance section or URLs ([movies.md:1](/C:/Users/przem/source/repos/opensherwood/docs/formats/movies.md:1)). The hub provenance depends on uncommitted scratch probes and omits required build hash and test dependencies ([sherwood-hub.md:310](/C:/Users/przem/source/repos/opensherwood/docs/formats/sherwood-hub.md:310)); ADR-0003 requires those fields ([ADR-0003:33](/C:/Users/przem/source/repos/opensherwood/docs/decisions/ADR-0003-clean-room-roles.md:33)). The combat report also includes verbatim manual/UI wording despite the explicit no-verbatim request ([combat-measurements.md:40](/C:/Users/przem/source/repos/opensherwood/docs/original/combat-measurements.md:40), [combat-measurements.md:129](/C:/Users/przem/source/repos/opensherwood/docs/original/combat-measurements.md:129)). I found no designer names.

   **Fix:** paraphrase the quoted text, add complete provenance and direct public-documentation citations, and commit reusable probe tooling or fully reproducible replacement commands.

8. **[MEDIUM] Persistence format validation is bypassable.** Profiles accept missing or non-numeric `format` as version 1 through `unwrap_or(1)` ([engine.rs:809](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/engine.rs:809)). Settings accept any numeric format and immediately rewrite it to 1 during sanitation ([ui.rs:1152](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/ui.rs:1152), [engine.rs:878](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/engine.rs:878)). This contradicts “malformed or unknown-format documents are ignored.”

   **Fix:** parse a required version envelope first and reject anything except integer `1`. Test missing, string and future numeric versions for both files.

9. **[MEDIUM] Protocol-6 UI metadata remains internally false.** `enabled` means clickable and `selected` means current selection ([protocol/lib.rs:231](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-protocol/src/lib.rs:231)), but option bars emit `enabled = on`, `selected = false` even though off bars accept clicks ([ui.rs:1438](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/ui.rs:1438)). Select Player reports `hovered = selected`, although rows follow the button items and selection is not pointer hover ([ui.rs:1020](/C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/ui.rs:1020)). Stored-only option controls also remain enabled, so finding 10 is still user-visible.

   **Fix:** make interactive bars enabled, use `selected` for their state, calculate hover against the complete item array, and disable or implement controls that currently only persist values.

10. **[MEDIUM] Documentation and tests overstate completion.** The stealth document contains a duplicate obsolete section with 200/150 radii immediately after the current 250/350 section ([stealth-and-combat.md:442](/C:/Users/przem/source/repos/opensherwood/docs/original/stealth-and-combat.md:442), [stealth-and-combat.md:477](/C:/Users/przem/source/repos/opensherwood/docs/original/stealth-and-combat.md:477)). The harness example still advertises protocol/ruleset 5 ([harness.md:27](/C:/Users/przem/source/repos/opensherwood/docs/harness.md:27)); the roadmap simultaneously says the `;` minimap key is missing and marks incomplete Options as done ([roadmap.md:47](/C:/Users/przem/source/repos/opensherwood/docs/roadmap.md:47), [roadmap.md:49](/C:/Users/przem/source/repos/opensherwood/docs/roadmap.md:49)). Campaign/lost-page tests force outcomes with `debug.vm` ([test_script.py:410](/C:/Users/przem/source/repos/opensherwood/harness/tests/data/test_script.py:410)), contrary to the canonical-input test rule ([AGENTS.md:31](/C:/Users/przem/source/repos/opensherwood/AGENTS.md:31)).

   **Fix:** remove the obsolete block, update version examples and roadmap states, and drive these flows from a synthetic script/canonical events or a lower-level UI unit fixture.

## Review-8 closure

| Finding | Status |
|---|---|
| 1 taint closure | Open |
| 2 campaign graph successor | Closed |
| 3 transactional callbacks/overflow | Closed |
| 4 AI work budget | Open |
| 5 campaign money/replay | Open |
| 6 native result validation | Native half closed; equivalent ordinary-call bypass remains |
| 7 persistence | Partial |
| 8 asset-derived history | Open |
| 9 Select Player protocol | Partial |
| 10 Options | Partial, as admitted |
| 11 disposition accuracy | Open because multiple “Done” claims remain false |

`git diff --check dc9f006^..HEAD` passed. I did not run the full suite because the live worktree acquired concurrent uncommitted edits, which would test code outside the requested committed range. The independent review required by [cross-agent-review/SKILL.md](/C:/Users/przem/source/repos/opensherwood/.agents/skills/cross-agent-review/SKILL.md) was attempted, but the Claude process could not connect through the environment firewall.