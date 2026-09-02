# Codex adversarial review 3: menu, briefing and mission start (2026-09-02)

Scope: commit `3279445`. Requested by the lead with the cross-agent-review skill; disposition in
`2026-09-02-codex-review-3-disposition.md`.

Review baseline: committed tree at `3279445`; concurrent uncommitted workspace changes were excluded.

1. **Critical — P0 legal containment is not complete.**

   The policy forbids all original text (`docs/legal.md:10-12,20-24`), but current `HEAD` still contains:

   - Verbatim manual prose and an in-game message in `docs/original/campaign-flow.md:22-26`.
   - Briefing fragments and the full mission-title catalog in `docs/original/campaign-flow.md:69-81`.
   - Exact option/shortcut tables in `docs/original/ui-flow.md:116-176`.
   - Credits wording in `docs/original/ui-flow.md:199-200`.
   - An objective, quit-dialog wording, and a manual instruction in `docs/original/ui-flow.md:260-272`.
   - Long executable/help strings in `docs/original/executable-notes.md:21-27` and `docs/original/console-commands.md:62-63`.
   - A mission title copied into `harness/tests/data/test_menu.py:3-4`, plus `docs/roadmap.md:44` and `docs/status/2026-09-02.md:8`.

   The generic fallback labels in `ui.rs:69-75` are reasonable functional identifiers. Individual labels such as “OK” or “Load” are also defensible; wholesale option tables, narrative prose, dialog sentences, creative title catalogs, and credits are not under this repository’s stricter policy.

   This directly contradicts the “Done” dispositions at `docs/decisions/reviews/2026-09-02-codex-full-review-disposition.md:11-13`. Historical containment also remains explicitly open at line 14.

   **Fix:** replace copied text with resource IDs, indices, lengths/hashes, and original-worded paraphrases; remove titles from tests/status prose; audit all tracked documentation again. Resolve the already-pushed historical leak separately with the maintainer/counsel.

2. **High — Briefing state breaks snapshot/restore determinism.**

   `Screen::Briefing` decides whether the world advances (`engine.rs:110-126,249-286`), so it is authoritative session state. Nevertheless, `snapshot` stores only `World`, and `restore` does not restore `screen` or the current page (`engine.rs:769-817`).

   Concrete failure:

   1. Take a snapshot on briefing page 1.
   2. Dismiss the briefing and advance the world.
   3. Restore the snapshot.
   4. `screen` remains `World`; the identical Enter/click suffix now advances and mutates the world instead of paging the briefing.

   This violates the invariant in `docs/decisions/ADR-0004-protocol.md:51-55`. Replay recording is likewise allowed during a briefing (`engine.rs:852-877`), but briefing events never reach `step_recorded`; playback starts a direct mission world at `engine.rs:941-957`.

   The existing test correctly confirms that the world tick is paused (`test_menu.py:51-62`), but does not test hash equality, snapshot/restore, or replay.

   **Fix:** create a versioned session snapshot containing world plus modal/page state, while keeping UI out of world hashes; alternatively reject snapshot/replay operations outside `Screen::World`. Add restore/replay tests from every briefing page using identical suffixes.

3. **High — UI rendering turns an explicitly unknown format semantic into code.**

   `ui_assets.rs:44-54` applies the sprite preview converter to every SRES picture—including opaque backgrounds and parchments—and claims `0x07C0` is their transparency key. The format specification explicitly says SRES color-key behavior is unknown (`docs/formats/image-blob.md:17-20`). The converter also maps the sprite shadow key to half-transparent black despite documenting that as an unverified preview choice (`sprite_decode.rs:213-222`).

   The green pause tint is similarly an invented per-channel transform (`ui.rs:486-490`); the observation only establishes that the scene appears green-tinted.

   This can erase legitimate green pixels and render shadows incorrectly, and it violates the rule against guessed semantics.

   **Fix:** use opaque RGB565 for resources known to be opaque; establish per-resource transparency and tint behavior through black-box pixel comparisons before implementing it. Do not reuse the sprite preview converter as a UI decoder.

4. **Medium — Protocol 3 does not consistently model the new menu scenario.**

   - `ObserveResult` requires a flattened `Observation`, but menu observation returns an ad-hoc object missing `scenario`, `viewport`, `map_size`, and other required fields (`protocol/lib.rs:194-205`, `engine.rs:757-767`). A protocol client deserializing the documented type will reject a valid menu response.
   - `ui` is an unversioned `serde_json::Value`, while `MenuState` remains app-private.
   - `hello.capabilities` omits `menu` (`engine.rs:669-676`).
   - `Scenario::Menu` is placed in core even though core refuses to construct it (`world.rs:90-103,257-270`), making invalid replay/world states representable.
   - The accepted ADR still claims protocol/ruleset/snapshot versions 2/2/3 instead of 3/3/4 (`ADR-0004-protocol.md:5-6`).
   - The ADR says `step` advances exactly N ticks, but paused/menu steps may advance zero world ticks while consuming N offsets (`ADR-0004-protocol.md:21`, `engine.rs:728-750`).

   **Fix:** define typed `SessionTarget`, `UiState`, and session-observation DTOs in the protocol; make world observation optional/tagged; advertise the capability; keep presentation targets outside core `Scenario`; document whether offsets represent simulation ticks or UI frames.

5. **Medium — The advertised main menu is behaviorally misleading.**

   All seven entries are reported enabled (`ui.rs:193-203`), but five only emit a log and do nothing (`engine.rs:268-270`). Escape and Exit terminate immediately (`ui.rs:252`, `engine.rs:268`), whereas the observed UI opens a modal confirmation (`ui-flow.md:76-87,106`).

   The displayed profile is always the fabricated default `"Player"` with fixed values (`engine.rs:215-223`, `ui.rs:152-162`), despite the documented screen showing the selected profile. The briefing also omits the observed character portrait (`ui.rs:433-459`, `ui-flow.md:213-218`).

   **Fix:** implement the actions/modal/profile loading, or expose unsupported entries as disabled and describe the screen as a partial menu. Add confirm/cancel, profile-data, and briefing-portrait tests.

6. **Medium — Geometry and mission-start fidelity are being locked before verification.**

   The UI-flow specification simultaneously declares button tops at `345 + 41*k` and says the implementation uses `339 + 41*k` (`ui-flow.md:51-60`). The discrepancy exceeds the stated ±2 px measurement tolerance. Code and tests simply lock 339 (`ui.rs:15-18,200`, `test_menu.py:27`) without resolving which measurement is correct.

   `FIRST_MISSION = "H01_Lin_VL"` is also hardcoded (`engine.rs:129-130,228-239`) while its provenance explicitly says the level mapping remains inferred and unconfirmed (`campaign-flow.md:100-110`). The test checks only a large map and one Robin, not the exact map identity, camera geometry, resource association, or original pixels.

   **Fix:** remeasure the button bounds and normalize the spec; confirm the campaign mapping through black-box observation or derive it from player data; test exact initial camera/player screen position and local-only original-image metrics.

7. **Medium — Campaign transition is non-transactional.**

   `reset` switches to `Screen::World` before fallible mission/map loading (`engine.rs:554-590`). If Play fails, `start_campaign` merely logs the error (`engine.rs:263-266`), leaving the user with no menu and potentially no world.

   **Fix:** construct the new world/background/UI state in temporaries and commit them only after every load succeeds. On failure, retain the menu and expose a visible/protocol error. Add a missing/corrupt mission-resource test.

8. **Medium — Menu music continues into the briefing and mission.**

   Mission selection returns from `start_scenario_music` without stopping the current track (`engine.rs:322-348`). When entering through the menu, the already-looping menu music therefore continues; direct mission reset instead starts silent. `Audio::stop_music` already exists (`opensherwood-audio/src/lib.rs:99-102`).

   **Fix:** stop or replace the previous track during every scenario transition, then add an audio-state test seam independent of an actual output device.

`cargo fmt --check`, commit whitespace checking, and `scripts/check_no_assets.py` passed; the policy script explicitly acknowledges that it cannot detect copied text. A full build/test rerun against the exact commit was not possible in the read-only, concurrently changing worktree. The required Claude cross-review was attempted but its CLI connection was refused.

**Verdict: fix-then-merge.**