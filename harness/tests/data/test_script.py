"""The mission script VM (needs OPENSHERWOOD_GAME_DIR): the retail scripts run in the engine.

`docs/formats/scb.md` ("First mission script walkthrough", "Engine") and ADR-0008. The first mission's
`PostInitialize` adds the primary objective 0 and plays one sequence: text pages 0, 1, 2, then the camera
returns to the hero. Every player action here is canonical input: pages are dismissed with Enter through
the briefing screen (`Engine.skip_briefing`), walks are left clicks on the ground, the pause menu is Escape. `debug.vm`
only inspects (`{"element": i}` describes one entry of the element table); the end-of-mission flows are
driven through play in `test_win.py`.
"""

from __future__ import annotations

import json

from opensherwood_harness import Engine, key_press, pointer_click

FIRST_MISSION = "H01_Lin_VL"

# Ticks every mission runs in strict mode without a fault (`docs/formats/scb.md`, "Natives at load per
# mission": the lenient run of 2026-09-02 reached tick 500 everywhere; the strict run of 2026-09-03 too, and
# the strict run of 2026-09-05 over all 39 files under the corrected element table).
EARLY_TICKS = 300

# Script state right after loading each mission with seed 1 (`PostInitialize` ran): whether an unknown
# native trapped (`faulted`), how many traps, and the stub natives called (`{id: calls}`). Derived from the
# engine on 2026-09-03 (ruleset 7, hash schema 9: every native the load-time closure reaches is implemented
# or a recorded stub, so no loadable script traps) and re-recorded on 2026-09-05 with the corrected element
# table (`docs/formats/sherwood-hub.md`, section 4: the player slots at the tail, the map prefix from the
# `.rhp`): the forest missions now report their hidden player slots through stub 244 (slot empty), the
# "every element" loops of H05 / Str01 / Str02 (stubs 80 / 81) cover the `ZORG` / `TING` entries, and the two
# Sherwood missions load strictly. A change here is a deliberate edit that goes with the native or binding
# that caused it, never a silent drift.
EXPECTED_AT_LOAD: dict[str, tuple[bool, int, dict[str, int]]] = {
    "Emb01_FoA_EC": (False, 0, {"20": 1, "51": 1, "54": 1, "73": 1, "224": 3, "244": 4}),
    "Emb02_FoC_MK": (False, 0, {"54": 1, "224": 3}),
    "Emb03_FoC_MP": (False, 0, {"51": 1, "54": 1, "73": 1, "224": 3, "244": 4}),
    "Emb04_FoA_MP": (False, 0, {"51": 1, "54": 1, "73": 1, "224": 3, "244": 5}),
    "Emb05_FoB_MP": (False, 0, {"20": 1, "54": 1, "73": 1, "224": 4}),
    "Emb06_FoC_EC": (False, 0, {"20": 1, "51": 1, "54": 1, "73": 1, "224": 4, "244": 5}),
    "Emb07_FoB_JMS": (False, 0, {"54": 1, "188": 1, "224": 4}),
    "Emb08_FoA_JMS": (False, 0, {"51": 1, "54": 1, "180": 1, "224": 4, "228": 2}),
    "Emb09_FoB_JMS": (False, 0, {"54": 1, "73": 2, "195": 1, "224": 4}),
    "EmbTut_FoC_EC": (False, 0, {"20": 1, "51": 1, "54": 1, "73": 1, "224": 3, "244": 4}),
    "H01_Lin_VL": (False, 0, {"186": 2, "191": 6, "198": 5}),
    "H02_Not_EC": (False, 0, {"24": 1, "186": 10, "187": 2, "188": 9, "189": 9, "191": 8, "198": 17, "264": 3}),
    "H03_Der_MK": (False, 0, {"42": 2, "50": 1, "51": 1, "53": 7, "54": 1, "99": 17, "186": 6, "188": 1, "189": 4, "191": 1, "195": 1, "218": 6}),
    "H04_Lei_VL": (False, 0, {"38": 1, "51": 2, "191": 2, "195": 1, "198": 17, "254": 1}),
    "H05_Lin_EC": (False, 0, {"24": 2, "80": 198, "99": 1, "177": 2, "186": 9, "191": 7, "198": 48}),
    "H07_Not_MK": (False, 0, {"20": 1, "50": 1, "80": 2, "92": 1, "99": 1, "177": 3, "180": 5, "186": 15, "187": 15, "188": 15, "189": 15, "191": 4, "195": 1, "205": 2, "218": 3, "244": 1, "247": 1, "264": 5}),
    "H09_Not_VL": (False, 0, {"186": 4, "187": 4, "188": 1, "189": 1, "191": 8, "198": 8}),
    "H10_Yor_VL": (False, 0, {"24": 2, "35": 1, "51": 1, "54": 1, "99": 1, "186": 3, "187": 3, "191": 4, "195": 2, "198": 8}),
    "H12_Not_MP": (False, 0, {"20": 1, "50": 1, "52": 1, "156": 32, "177": 7, "186": 16, "187": 8, "188": 5, "189": 3, "191": 8}),
    "S01_Not_VL": (False, 0, {"24": 1, "35": 1, "54": 1, "186": 9, "187": 9, "188": 1, "189": 3, "191": 8, "198": 9}),
    "S02_Lei_MP": (False, 0, {"20": 1, "24": 1, "50": 1, "51": 2, "99": 1, "156": 24, "177": 3, "186": 3, "187": 1, "189": 2, "191": 5, "254": 1, "264": 1}),
    "S03_FoB_MP": (False, 0, {"20": 1, "38": 1, "54": 1, "125": 3, "156": 1, "177": 14, "232": 1}),
    "S04_Der_EC": (False, 0, {"186": 5, "187": 1, "188": 5, "189": 5, "191": 3, "198": 7}),
    "S05_Yrk_EC": (False, 0, {"20": 1, "24": 3, "51": 1, "54": 1, "99": 3, "156": 1, "186": 7, "188": 4, "189": 6, "191": 4, "198": 10, "218": 7, "264": 3}),
    "SherwoodOutro": (False, 0, {"54": 1, "180": 11}),
    "Str01_Lin_EC": (False, 0, {"80": 241, "99": 12, "186": 1, "188": 1, "189": 1, "191": 7, "198": 42}),
    "Str02_Der_MP": (False, 0, {"20": 1, "81": 182, "189": 4, "191": 1, "195": 2, "198": 11}),
    "Str03_Yor_MK": (False, 0, {"51": 1, "99": 4, "143": 14, "186": 7, "188": 1, "189": 3, "191": 4, "195": 1}),
    "Tac01_FoA_MP": (False, 0, {"20": 1, "54": 1, "224": 6, "244": 5}),
    "Tac02_FoB_EC": (False, 0, {"20": 1, "54": 1, "198": 55, "224": 9, "244": 5}),
    "Tac03_FoC_MP": (False, 0, {"39": 1, "52": 1, "54": 1, "224": 2, "244": 5}),
    "Tac04_FoA_EC": (False, 0, {"20": 1, "54": 1, "224": 5, "244": 5}),
    "Tac05_FoC_MP": (False, 0, {"177": 8, "198": 8}),
    "Tac06_FoB_EC": (False, 0, {"20": 1, "54": 1, "224": 3}),
    "Tac17_FoC_EC": (False, 0, {"20": 1, "51": 1, "54": 1, "73": 1, "224": 4, "244": 5}),
    "Tac18_FoA_EC": (False, 0, {"20": 1, "51": 1, "54": 1, "73": 1, "224": 3}),
    "Tac19_FoB_EC": (False, 0, {"20": 1, "51": 1, "54": 1, "73": 1, "224": 2, "244": 4}),
    "Tac21_FoB_EC": (False, 0, {"20": 1, "186": 1, "188": 1, "189": 1, "224": 4, "244": 5}),
    "sherwood": (False, 0, {"7": 1, "150": 1, "195": 25, "210": 10, "214": 1, "215": 14, "256": 50, "261": 1}),
}


def _hero(engine: Engine) -> dict:
    return next(x for x in engine.observe()["entities"] if x["kind"] == "player")


def _select_hero(e: Engine) -> tuple[int, int]:
    """Left-click the hero (on screen after the briefing); returns its screen position."""
    obs = e.observe()
    robin = next(x for x in obs["entities"] if x["kind"] == "player")
    cam = obs["camera"]
    sx, sy = robin["x"] // 256 - cam[0], robin["y"] // 256 - cam[1]
    assert 0 <= sx < 1024 and 0 <= sy < 768, (sx, sy)
    e.step(2, pointer_click(sx, sy, "left"))
    assert e.observe(entities=False)["selected"] is not None
    return sx, sy


def _run_first_mission(binary, game_dir, artifacts, seed: int, ticks: int = 600) -> tuple[list[dict], dict, dict]:
    """Dismiss the briefing pages with Enter, step `ticks` with per-tick hashes; return the per-tick
    hashes, the VM report and the observation."""
    with Engine(binary=binary, game_dir=game_dir, artifacts=artifacts, timeout=300) as e:
        e.reset({"mission": FIRST_MISSION}, seed=seed)
        assert e.skip_briefing() == 3
        r = e.step(ticks, hash_every_tick=True)
        return r["per_tick"], e.call("debug.vm"), e.observe(entities=False)


def test_first_mission_briefing_sequence_then_camera_on_the_hero(binary, game_dir, tmp_path):
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=300) as e:
        e.reset({"mission": FIRST_MISSION}, seed=1)
        obs = e.observe(entities=False)
        sc = obs["script"]
        # PostInitialize ran at load: objective 0 (primary), the first page pending, the sequence active,
        # the page on the briefing parchment.
        assert sc["objectives"] == [{"index": 0, "primary": True, "done": False}]
        assert sc["texts"] == [0]
        assert sc["sequence_active"] and not sc["mission_won"] and not sc["faulted"]
        assert obs["ui"]["screen"] == "briefing"
        vm = e.call("debug.vm")
        assert vm["present"] and vm["classes"] == 47
        assert vm["counters"]["faults"] == 0 and vm["counters"]["traps"] == 0
        # Enter dismisses a page (one session tick, the world does not tick); pages 1 and 2 follow each
        # dismissal, the third dismissal ends the sequence and closes the parchment.
        for expected in ([1], [2], []):
            e.step(1, key_press("enter"))
            obs = e.observe(entities=False)
            assert obs["script"]["texts"] == expected
            assert (obs.get("ui") or {}).get("screen") == ("briefing" if expected else None)
            assert obs["tick"] == 0, "screens do not tick the world"
        vm = e.call("debug.vm")
        assert not vm["sequence_active"]
        hero = _hero(e)
        assert vm["camera_target"] == [(hero["x"] + 128) // 256, (hero["y"] + 128) // 256]
        # The taint is dependency-closed (ADR-0008, "Hypotheses and taint"; Codex review 8): the
        # level's `Initialize` already took hypotheses at load, before any tick, so the mission is
        # tainted from the start: it locks doors (effect stubs 186 / 191), hides an actor (198),
        # locks AI (policy native 134: the halting is a low-confidence reading) and sets action
        # availability (policy native 196: stored, not modelled). The briefing pages themselves
        # (natives 26 / 30 / 203 / 32 / 34 / 95 / 211 / 31) add nothing.
        AT_LOAD = [{"stub_result": 186}, {"stub_result": 191}, {"stub_result": 198}, {"policy": 134}, {"policy": 196}]
        sc = e.observe(entities=False)["script"]
        assert sc["tainted"] is True and sc["assumptions"] == AT_LOAD, sc["assumptions"]
        # Nothing left to dismiss: Enter in the world is not a page dismissal.
        e.step(1, key_press("enter"))
        assert e.observe(entities=False).get("ui") is None
        # Hourglass on every class and CheckVictoryCondition, 600 ticks, without a fault.
        e.step(600)
        vm = e.call("debug.vm")
        assert vm["counters"]["callbacks"] >= 600
        assert vm["counters"]["budget_aborts"] == 0 and vm["counters"]["faults"] == 0
        assert not vm["faulted"] and vm["counters"]["traps"] == 0
        assert vm["counters"]["arity_mismatches"] == {}
        assert not vm["mission_won"]
        sc = e.observe(entities=False)["script"]
        assert sc["objectives"][0]["done"] is False
        # The taint of a normal run: the archery training plays animations (49 / 51) and shoots (59),
        # the steward objective polls the purse item's "taken" predicate (235: implemented on the
        # pick-up items, a policy reading of a low row), the scroll
        # states are read and written (193 / 194: low-confidence rows), a wait / the Hourglass time
        # was consumed under the 25-versus-60 tick reading, a sequence walk completed without
        # arriving (the sergeant walks to an archer's spot) and `ActionChange` handlers ran (the
        # parameter order is a hypothesis); neither perception nor a knock-out reached the script.
        assert sc["tainted"] is True
        assert sc["assumptions"] == [
            {"stub_result": 49}, {"stub_result": 51}, {"stub_result": 59},
            {"stub_result": 186}, {"stub_result": 191}, {"stub_result": 198},
            {"policy": 134}, {"policy": 193}, {"policy": 194}, {"policy": 196}, {"policy": 235},
            "tick_rate", "walk_completion", "action_change_order",
        ], sc["assumptions"]
        assert "sight_cone" not in sc["assumptions"] and "knock_out" not in sc["assumptions"]
        assert vm["fault"] is None and vm["counters"]["transactions_rolled_back"] == 0


def test_first_mission_script_is_deterministic_across_processes(binary, game_dir, tmp_path):
    """Two processes, same seed: equal hashes after every one of 600 ticks, equal VM counters."""
    a, va, _ = _run_first_mission(binary, game_dir, tmp_path, seed=4)
    b, vb, _ = _run_first_mission(binary, game_dir, tmp_path, seed=4)
    assert len(a) == 600
    first_diff = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), None)
    assert first_diff is None, f"first divergence at tick {first_diff + 1}: {a[first_diff]} vs {b[first_diff]}"
    assert va["counters"] == vb["counters"]


def test_seed_changes_the_gameplay_rng_stream(binary, game_dir, tmp_path):
    """Two seeds, same input: the gameplay stream is drawn from during the first 600 ticks (rail
    guards), so the `rng` hashes differ and the draw counts are non-zero for both. The first mission's
    script draws nothing from the `script` stream in these 600 ticks (asserted, so a script that starts
    drawing shows up here); the `script` stream's seed dependence is covered by the core VM tests and the
    synthetic corridor's `rng` test (`harness/tests/synthetic/test_determinism.py`)."""
    _, va, oa = _run_first_mission(binary, game_dir, tmp_path, seed=1)
    _, vb, ob = _run_first_mission(binary, game_dir, tmp_path, seed=2)
    assert oa["rng_draws"] > 0 and ob["rng_draws"] > 0, (oa["rng_draws"], ob["rng_draws"])
    assert oa["hashes"]["rng"] != ob["hashes"]["rng"]
    assert va["rng_draws"] == 0 and vb["rng_draws"] == 0, "H01 draws no script randomness in 600 ticks"


def test_mission_replay_round_trip_from_the_first_page(binary, game_dir, tmp_path):
    """Recording starts right after `reset`, before the first page is dismissed: the three Enter
    presses, the selection, a walk order and a pause / continue through the pause menu are recorded as
    the session ticks they were played at, and playback reproduces every checkpoint (world hashes,
    world tick, session digest of the screen and framebuffer hash) from the tick-0 state on. The replay
    file goes under `tmp_path` (retail-derived, never under the repository)."""
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=300) as e:
        e.reset({"mission": FIRST_MISSION}, seed=7)
        assert e.observe(entities=False)["ui"]["screen"] == "briefing"
        e.replay_start(checkpoint_every=10)
        assert e.skip_briefing() == 3
        sx, sy = _select_hero(e)
        before = _hero(e)
        e.step(1, pointer_click(sx + 60, sy, "left"))
        e.step(60)
        hero = _hero(e)
        assert (hero["x"], hero["y"]) != (before["x"], before["y"]), "the walk order took"
        e.step(1, key_press("escape"))
        assert e.observe(entities=False)["ui"]["screen"] == "pause_menu"
        e.step(3)
        paused_frame = e.capture()["hash"]
        e.step(1, key_press("escape"))
        assert e.observe(entities=False).get("ui") is None
        e.step(40)
        rec = e.replay_stop(path="replays/h01_from_first_page.jsonl")
        assert rec["path"].startswith(str(tmp_path))
        final = e.observe(entities=False)
        final_frame = e.capture()["hash"]
        # 3 pages + 2 (selection) + 1 (order) + 60 + 1 (escape) + 3 + 1 (escape) + 40 session ticks;
        # the world ticked on all but the 3 page frames and the 5 pause frames.
        session_ticks = 3 + 2 + 1 + 60 + 1 + 3 + 1 + 40
        assert final["tick"] == session_ticks - 8
        lines = [json.loads(l) for l in rec["jsonl"].splitlines() if l.strip()]
        header = lines[0]
        assert header["time"] == "session" and header["scenario"] == {"mission": FIRST_MISSION}
        assert header["seed"] == 7 and header["viewport"] == [1024, 768]
        assert set(header["rng_streams"]) == {"gameplay", "script"}
        assert header["content_fingerprint"]
        events = [l for l in lines if l["type"] == "event"]
        assert [x["tick"] for x in events[:6]] == [0, 0, 1, 1, 2, 2], "Enter presses at ticks 0, 1, 2"
        assert events[0]["kind"] == "key_down" and events[0]["key"] == "enter"
        checkpoints = [l for l in lines if l["type"] == "checkpoint"]
        assert checkpoints[0]["tick"] == 0 and checkpoints[0]["world_tick"] == 0
        assert checkpoints[-1]["tick"] == session_ticks and checkpoints[-1]["world_tick"] == final["tick"]
        assert checkpoints[1]["tick"] == 10 and checkpoints[1]["world_tick"] == 7, "the pages lag the world"
        assert rec["checkpoints"] == 1 + session_ticks // 10 + 1
        # Screens are in the checkpoints: tick 0 (briefing page) and tick 70 (pause menu, 3 frames in)
        # have session digests and frames of their own; the world-screen checkpoints share one digest.
        by_tick = {c["tick"]: c for c in checkpoints}
        assert by_tick[70]["frame"] == paused_frame and by_tick[70]["world_tick"] == 63
        assert checkpoints[-1]["frame"] == final_frame
        world_digests = {c["session"] for c in checkpoints if c["tick"] not in (0, 70)}
        assert len(world_digests) == 1 and by_tick[0]["session"] not in world_digests
        assert by_tick[70]["session"] not in world_digests and by_tick[70]["session"] != by_tick[0]["session"]

        played = e.replay_play(jsonl=rec["jsonl"])
        assert played["first_divergence"] is None, played
        assert played["checkpoints_ok"] == rec["checkpoints"]
        assert played["ticks"] == session_ticks
        assert played["hashes"] == final["hashes"]
        after = e.observe(entities=False)
        assert after["tick"] == final["tick"] and after.get("ui") is None
        assert e.capture()["hash"] == final_frame
        # The taint travels with the state: the assumptions are in the `scripts` hash the checkpoints
        # compare, and playback ends with the recording's set (tainted from load, see
        # `test_first_mission_briefing_sequence_then_camera_on_the_hero`).
        assert final["script"]["tainted"] is True
        assert after["script"]["assumptions"] == final["script"]["assumptions"]
        # The same replay from the file, with divergence reporting off, still matches everything.
        played = e.replay_play(path="replays/h01_from_first_page.jsonl", stop_on_divergence=False)
        assert played["first_divergence"] is None and played["checkpoints_ok"] == rec["checkpoints"]


def test_mission_snapshot_restore_continuation(binary, game_dir, tmp_path):
    """Restore after the pages are dismissed and a walk is under way, then run the identical suffix
    (a second order at the same point, the same ticks): equal hashes after every tick and equal
    frames at two points of the suffix."""
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=300) as e:
        e.reset({"mission": FIRST_MISSION}, seed=5)
        assert e.skip_briefing() == 3
        sx, sy = _select_hero(e)
        e.step(1, pointer_click(sx + 80, sy + 20, "left"))
        e.step(20)
        snap = e.snapshot()
        assert snap["snapshot"]["content"], "retail snapshots carry the content fingerprint"

        def suffix() -> list[dict]:
            hashes = e.step(30, pointer_click(sx - 60, sy + 40, "left"), hash_every_tick=True)["per_tick"]
            hashes.append({"frame": e.capture()["hash"]})
            hashes += e.step(90, hash_every_tick=True)["per_tick"]
            hashes.append({"frame": e.capture()["hash"]})
            return hashes

        straight = suffix()
        r = e.restore(snapshot_id=snap["id"])
        assert r["hashes"] == snap["hashes"]
        again = suffix()
        first_diff = next((i for i, (x, y) in enumerate(zip(straight, again)) if x != y), None)
        assert first_diff is None, f"divergence {first_diff + 1} ticks after the restore"
        # And from the snapshot value itself, through JSON.
        e.restore(snapshot=snap["snapshot"])
        assert suffix() == straight


def test_every_mission_script_translates_and_runs_300_ticks_strictly(binary, game_dir, tmp_path):
    """Every retail script (all 39, the Sherwood hub and outro included) loads strictly and its state
    after `PostInitialize` is the expected one (`EXPECTED_AT_LOAD`): faults, traps and stub calls are
    asserted, not printed. Then every mission runs `EARLY_TICKS` ticks in strict mode (no page dismissed: the briefing sequences stay
    on their first page, `Hourglass` / `CheckVictoryCondition` / messages / zones run) without a fault,
    a trap, a run-time fault or a budget abort, and none is won or lost by tick `EARLY_TICKS`."""
    names = sorted(p.stem for p in (game_dir / "DATA" / "Levels").glob("*.scb"))
    assert len(names) == 39
    assert set(names) == set(EXPECTED_AT_LOAD), "the expectation table lists every script"
    mismatches = []
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=900) as e:
        for name in names:
            expected = EXPECTED_AT_LOAD[name]
            try:
                e.reset({"mission": name}, seed=1)
            except Exception as ex:  # noqa: BLE001 - a refused load is a mismatch like any other
                mismatches.append(f"{name}: refused: {ex}")
                continue
            vm = e.call("debug.vm")
            if not vm["present"]:
                mismatches.append(f"{name}: script not attached")
                continue
            got = (vm["faulted"], vm["counters"]["traps"], vm["counters"]["stub_natives"])
            if got != expected:
                mismatches.append(f"{name}: (faulted, traps, stubs) {got} != {expected}")
            e.step(EARLY_TICKS)
            vm = e.call("debug.vm")
            c = vm["counters"]
            if vm["faulted"] or c["traps"] or c["faults"] or c["budget_aborts"]:
                mismatches.append(
                    f"{name}: after {EARLY_TICKS} ticks faulted={vm['faulted']} traps={c['traps']} "
                    f"faults={c['faults']} budget_aborts={c['budget_aborts']} unknown={c['unknown_natives']}"
                )
            if vm["mission_won"] or vm["mission_lost"]:
                mismatches.append(f"{name}: won={vm['mission_won']} lost={vm['mission_lost']} by tick {EARLY_TICKS}")
            if c["arity_mismatches"]:
                mismatches.append(f"{name}: native arity mismatches {c['arity_mismatches']}")
            sc = e.observe(entities=False)["script"]
            if sc["tainted"] != bool(sc["assumptions"]):
                mismatches.append(f"{name}: tainted={sc['tainted']} but assumptions={sc['assumptions']}")
    assert not mismatches, "\n".join(mismatches)


def test_first_mission_element_table_has_the_hero_at_its_tail(binary, game_dir, tmp_path):
    """The element index space of `docs/formats/scb.md` ("Index spaces"): in H01 the map's 50 entries
    (38 animated elements, 12 patches) come first, the mission's civilians from 50, the eleven `ZORG`
    pick-up items 100..=110 precede the fifteen scrolls 111..=125 (the file's chunk order,
    `docs/original/h01-win-path.md` 2), the single player slot is element 126 (the hero, entity 0: the
    element the level's `Initialize` addresses with native 117) and the script polygons follow it."""
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=300) as e:
        e.reset({"mission": FIRST_MISSION}, seed=1)
        assert e.observe()["entities"][0]["kind"] == "player", "the hero is entity 0"
        vm = e.call("debug.vm", {"element": 126})
        assert vm["element"] == {"kind": "actor", "entity": 0}
        assert e.call("debug.vm", {"element": 49})["element"] == {"kind": "map", "index": 49}
        assert e.call("debug.vm", {"element": 50})["element"] == {"kind": "actor", "entity": 1}
        for i in range(100, 111):
            assert e.call("debug.vm", {"element": i})["element"]["kind"] == "item", i
        for i in range(111, 126):
            assert e.call("debug.vm", {"element": i})["element"]["kind"] == "scroll", i
        assert e.call("debug.vm", {"element": 127})["element"]["kind"] == "polygon"
        assert e.call("debug.vm", {"element": vm["elements"]})["element"] is None


def test_sherwood_camp_and_outro_load_strictly_and_run(binary, game_dir, tmp_path):
    """The Sherwood hub (`Sherwood.rhm` with the lower-case `sherwood.scb`) and the outro load with their
    scripts under the index space of `docs/formats/sherwood-hub.md` (map prefix 20 = 20 `FLIM` + 0 `TUPO`):
    the hub binds the trainer at 21 (entity 51: the 50 slots and the recruit come first), its 23 scrolls
    at 28..=50 and the 50 player slots at 51..=100; the outro (variant 4, no `Fog/sherwood.map`: the engine
    falls back to the day background) puts the hero at 70. Both run their load-time callbacks and 300
    strict ticks without a trap."""
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=300) as e:
        for name in ("Sherwood", "sherwood"):
            e.reset({"mission": name}, seed=1)
            vm = e.call("debug.vm")
            assert vm["present"] and vm["classes"] == 31 and vm["elements"] == 116
            assert e.call("debug.vm", {"element": 21})["element"] == {"kind": "actor", "entity": 51}
            assert e.call("debug.vm", {"element": 28})["element"]["kind"] == "scroll"
            assert e.call("debug.vm", {"element": 50})["element"]["kind"] == "scroll"
            assert e.call("debug.vm", {"element": 51})["element"] == {"kind": "actor", "entity": 0}
            assert e.call("debug.vm", {"element": 100})["element"] == {"kind": "actor", "entity": 49}
            assert e.call("debug.vm", {"element": 101})["element"]["kind"] == "polygon"
            assert len(vm["scrolls"]) == 23
        e.step(EARLY_TICKS)
        vm = e.call("debug.vm")
        assert not vm["faulted"] and vm["counters"]["traps"] == 0 and vm["counters"]["faults"] == 0
        assert not vm["mission_won"] and not vm["mission_lost"]
        e.reset({"mission": "SherwoodOutro"}, seed=1)
        vm = e.call("debug.vm")
        assert vm["present"] and vm["classes"] == 11 and vm["elements"] == 73
        assert e.call("debug.vm", {"element": 70})["element"] == {"kind": "actor", "entity": 0}
        assert e.call("debug.vm", {"element": 54})["element"]["kind"] == "object"
        assert e.observe(entities=False)["map_size"] == [1920, 1088]
        e.step(EARLY_TICKS)
        vm = e.call("debug.vm")
        assert not vm["faulted"] and vm["counters"]["traps"] == 0 and vm["counters"]["faults"] == 0


def test_starting_money_is_seeded_before_initialize(binary, game_dir, tmp_path):
    """`MissionSpec.starting_money` (the profile's money, 100 by default) reaches the VM before
    `Initialize` runs and nothing overwrites it afterwards (review 7, finding 3): H10's `Initialize`
    sets 100000 with native 237 (`docs/formats/scb.md`, "Natives at load per mission") and holds it
    right after `reset`; H01, whose script only reads the money, keeps the seed."""
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=300) as e:
        e.reset({"mission": "H10_Yor_VL"}, seed=1)
        assert e.call("debug.vm")["money"] == 100000
        e.step(5)
        assert e.call("debug.vm")["money"] == 100000
        e.reset({"mission": FIRST_MISSION}, seed=1)
        assert e.call("debug.vm")["money"] == 100
        e.skip_briefing()
        e.step(5)
        assert e.call("debug.vm")["money"] == 100


def walk_to(e, tx, ty, max_steps=400):
    """Order the selected player to (tx, ty) with left clicks (re-issued if the camera moved the
    point off screen; a re-issue within the double-click window makes it a run, which is fine here) and step until the order completes or the step limit is reached."""
    for _ in range(max_steps):
        o = e.observe()
        if o.get("ui"):
            return o
        p = next(x for x in o["entities"] if x["kind"] == "player")
        if p["target"] is None:
            if abs(p["x"] // 256 - tx) < 6 and abs(p["y"] // 256 - ty) < 6:
                return o
            cam = o["camera"]
            sx, sy = tx - cam[0], ty - cam[1]
            if 0 <= sx < 1024 and 0 <= sy < 768:
                e.step(1, pointer_click(sx, sy, "left"))
            else:
                key = "right" if sx >= 1024 else "left" if sx < 0 else "down" if sy >= 768 else "up"
                e.step(40, [{"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": key}])
                e.step(1, [{"tick_offset": 0, "sequence": 0, "kind": "key_up", "key": key}])
        else:
            e.step(10)
    return e.observe()


def test_clicking_a_scroll_stops_short_of_it_and_pauses_before_its_text(binary, game_dir, tmp_path):
    """Scroll reading (`docs/original/h01-measurements-2.md` 1.2 / 1.4, measured): a scroll is read by an
    order on it. Walking onto the nearest reachable scroll of the first mission with a ground order reads
    nothing; a left click on the scroll's sprite walks Robin to about 18 px short of it, `IsTaken` runs
    42 ticks after the arrival, and the tutorial scrolls show a text page (native 202 / 203), so one appears
    within a few readings, nearest first."""
    import math

    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=300) as e:
        e.reset({"mission": FIRST_MISSION}, seed=0)
        e.skip_briefing()
        obs = e.observe()
        robin_index = next(i for i, x in enumerate(obs["entities"]) if x["kind"] == "player")
        robin = obs["entities"][robin_index]
        rx, ry = robin["x"] // 256, robin["y"] // 256
        cam = obs["camera"]
        e.step(2, pointer_click(rx - cam[0], ry - cam[1], "left"))
        assert e.observe(entities=False)["selected"] is not None
        scrolls = [s for s in e.call("debug.vm", {})["scrolls"] if s["active"]]
        reachable = []
        for s in scrolls:
            nav = e.call("debug.nav", {"x": rx, "y": ry, "to": [s["x"], s["y"]]})
            if nav["path_cells"]:
                reachable.append((nav["path_cells"], s))
        assert reachable, "no scroll reachable from the start"
        reachable = [s for _, s in sorted(reachable, key=lambda t: t[0])]
        # A ground order 10 px below the nearest scroll (outside its sprite): the walk ends beside it
        # and reads nothing.
        first = reachable[0]
        walk_to(e, first["x"], first["y"] + 10)
        p = e.observe()["entities"][robin_index]
        assert p["target"] is None and p["pickup"] is None and not e.observe(entities=False).get("ui")
        still = next(x for x in e.call("debug.vm", {})["scrolls"] if x["element"] == first["element"])
        assert still["active"], "a walk onto the scroll does not read it"
        taken = []
        for s in reachable[:5]:
            # The click on the scroll's sprite (3 px right of and 2 px above its base).
            cam = e.observe(entities=False)["camera"]
            sx, sy = s["x"] - cam[0] + 3, s["y"] - cam[1] - 2
            if not (0 <= sx < 1024 and 0 <= sy < 768):
                o = e.observe()
                p = o["entities"][robin_index]
                # Bring the camera over: scroll towards the scroll from the hero's screen position.
                for key, d in (("right", s["x"] - 512 - cam[0]), ("left", cam[0] - (s["x"] - 512)), ("down", s["y"] - 384 - cam[1]), ("up", cam[1] - (s["y"] - 384))):
                    n = d // 8
                    if n > 0:
                        e.step(n, [{"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": key}])
                        e.step(1, [{"tick_offset": 0, "sequence": 0, "kind": "key_up", "key": key}])
                cam = e.observe(entities=False)["camera"]
                sx, sy = s["x"] - cam[0] + 3, s["y"] - cam[1] - 2
            p = e.observe()["entities"][robin_index]
            # From farther than the stop distance the walk ends about 18 px short; a hero already
            # within it does not move.
            was_far = math.hypot(p["x"] / 256 - s["x"], p["y"] / 256 - s["y"]) > 18
            e.step(1, pointer_click(sx, sy, "left"))
            p = e.observe()["entities"][robin_index]
            assert p["pickup"] == s["element"], (s, p["pickup"])
            arrived = None
            if p["target"] is None:
                # Already within the stop distance: the pause starts on the click's tick.
                arrived = 0
                assert p["pickup_ticks"] == 42, p
            o = None
            for t in range(2000):
                e.step(1)
                o = e.observe()
                p = o["entities"][robin_index]
                if arrived is None and p["target"] is None:
                    arrived = t + 1
                    short = math.hypot(p["x"] / 256 - s["x"], p["y"] / 256 - s["y"])
                    assert (12 if was_far else 0) <= short <= 26, f"stopped {short:.1f} px short of scroll {s['element']}"
                    assert p["pickup_ticks"] == 42, p
                if p["pickup"] is None:
                    assert arrived is not None and t + 1 == arrived + 42, (t + 1, arrived)
                    break
            else:
                raise AssertionError(f"the reading of scroll {s['element']} never resolved")
            # The handler's page comes up through its sequence on the following ticks.
            for _ in range(30):
                if o.get("ui"):
                    break
                e.step(1)
                o = e.observe()
            vm = e.call("debug.vm", {})
            still = next(x for x in vm["scrolls"] if x["element"] == s["element"])
            taken.append((s["element"], not still["active"], bool(o.get("ui"))))
            if o.get("ui"):
                assert o["ui"]["screen"] == "briefing"
                print("scroll", s["element"], "showed a text page; visited:", taken)
                return
        raise AssertionError(f"no text page after reading scrolls {taken}")
