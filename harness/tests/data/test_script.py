"""The mission script VM (needs OPENSHERWOOD_GAME_DIR): the retail scripts run in the engine.

`docs/formats/scb.md` ("First mission script walkthrough", "Engine") and ADR-0008. The first mission's
`PostInitialize` adds the primary objective 0 and plays one sequence: text pages 0, 1, 2, then the camera
returns to the hero. Texts are dismissed through `debug.vm {dismiss_text}` here (the window dismisses them
from the briefing parchment); nothing else about the script is privileged: `Hourglass` and
`CheckVictoryCondition` run inside `step`.
"""

from __future__ import annotations

import re

from opensherwood_harness import Engine, pointer_click

FIRST_MISSION = "H01_Lin_VL"


def _hero(engine: Engine) -> dict:
    return next(x for x in engine.observe()["entities"] if x["kind"] == "player")


def _run_first_mission(binary, game_dir, seed: int) -> tuple[str, dict]:
    """Dismiss the briefing pages, step 600 ticks; return the total hash and the VM report."""
    with Engine(binary=binary, game_dir=game_dir, timeout=300) as e:
        e.reset({"mission": FIRST_MISSION}, seed=seed)
        for _ in range(3):
            assert e.call("debug.vm", {"dismiss_text": True})["dismissed"]
        r = e.step(600)
        return r["hashes"]["total"], e.call("debug.vm")


def test_first_mission_briefing_sequence_then_camera_on_the_hero(binary, game_dir):
    with Engine(binary=binary, game_dir=game_dir, timeout=300) as e:
        e.reset({"mission": FIRST_MISSION}, seed=1)
        obs = e.observe(entities=False)
        sc = obs["script"]
        # PostInitialize ran at load: objective 0 (primary), the first page pending, the sequence active.
        assert sc["objectives"] == [{"index": 0, "primary": True, "done": False}]
        assert sc["texts"] == [0]
        assert sc["sequence_active"] and not sc["mission_won"] and not sc["faulted"]
        vm = e.call("debug.vm")
        assert vm["present"] and vm["classes"] == 47
        assert vm["counters"]["faults"] == 0 and vm["counters"]["traps"] == 0
        # Pages 1 and 2 follow each dismissal; the third dismissal ends the sequence.
        for expected in ([1], [2], []):
            vm = e.call("debug.vm", {"dismiss_text": True})
            assert vm["dismissed"]
            assert vm["texts"] == expected
        assert not vm["sequence_active"]
        hero = _hero(e)
        assert vm["camera_target"] == [(hero["x"] + 128) // 256, (hero["y"] + 128) // 256]
        assert not e.call("debug.vm", {"dismiss_text": True})["dismissed"], "nothing left to dismiss"
        # Hourglass on every class and CheckVictoryCondition, 600 ticks, without a fault.
        e.step(600)
        vm = e.call("debug.vm")
        assert vm["counters"]["callbacks"] >= 600
        assert vm["counters"]["budget_aborts"] == 0 and vm["counters"]["faults"] == 0
        assert not vm["faulted"] and vm["counters"]["traps"] == 0
        assert not vm["mission_won"]
        assert e.observe(entities=False)["script"]["objectives"][0]["done"] is False
        print(f"{FIRST_MISSION}: after 600 ticks unknown natives {vm['counters']['unknown_natives']}, "
              f"stubs {vm['counters']['stub_natives']}, messages {vm['counters']['messages_delivered']}")


def test_first_mission_script_is_deterministic_across_processes(binary, game_dir):
    a, va = _run_first_mission(binary, game_dir, seed=4)
    b, vb = _run_first_mission(binary, game_dir, seed=4)
    assert a == b
    assert va["counters"] == vb["counters"]


def test_every_mission_script_translates(binary, game_dir):
    names = sorted(p.stem for p in (game_dir / "DATA" / "Levels").glob("*.scb"))
    assert len(names) == 39
    failures = []
    summary = re.compile(r"script: .*$", re.M)
    with Engine(binary=binary, game_dir=game_dir, timeout=600) as e:
        for name in names:
            try:
                e.reset({"mission": name}, seed=1)
                vm = e.call("debug.vm")
                lines = summary.findall(e.stderr_text)
                print(f"{name}: {lines[-1] if lines else '(no summary)'}; "
                      f"at load: faulted={vm.get('faulted')} unknown={vm.get('counters', {}).get('unknown_natives')}")
                if not vm["present"]:
                    failures.append(f"{name}: script not attached")
            except Exception as ex:  # noqa: BLE001 - collect everything, report once
                failures.append(f"{name}: {ex}")
    assert not failures, "\n".join(failures)


def walk_to(e, tx, ty, max_steps=400):
    """Order the selected player to (tx, ty) with right clicks (re-issued if the camera moved the
    point off screen) and step until the order completes or the step limit is reached."""
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
                e.step(1, pointer_click(sx, sy, "right"))
            else:
                key = "right" if sx >= 1024 else "left" if sx < 0 else "down" if sy >= 768 else "up"
                e.step(40, [{"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": key}])
                e.step(1, [{"tick_offset": 0, "sequence": 0, "kind": "key_up", "key": key}])
        else:
            e.step(10)
    return e.observe()


def test_walking_onto_a_scroll_shows_its_text(binary, game_dir):
    """Scroll pickup (`IsTaken`): Robin walks to the reachable scrolls of the first mission, nearest
    first; the tutorial scrolls show a text page (native 202 / 203), so one appears within a few."""
    with Engine(binary=binary, game_dir=game_dir, timeout=300) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=0)
        e.skip_briefing()
        obs = e.observe()
        robin = next(x for x in obs["entities"] if x["kind"] == "player")
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
        taken = []
        for _, s in sorted(reachable, key=lambda t: t[0])[:5]:
            o = walk_to(e, s["x"], s["y"])
            vm = e.call("debug.vm", {})
            still = next(x for x in vm["scrolls"] if x["element"] == s["element"])
            taken.append((s["element"], not still["active"], bool(o.get("ui"))))
            if o.get("ui"):
                assert o["ui"]["screen"] == "briefing"
                print("scroll", s["element"], "showed a text page; visited:", taken)
                return
        raise AssertionError(f"no text page after visiting scrolls {taken}")


def test_mission_won_shows_the_debriefing_then_the_menu(binary, game_dir):
    """The end of a mission: the won debriefing parchment, then the main menu."""
    with Engine(binary=binary, game_dir=game_dir, timeout=300) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=0)
        e.skip_briefing()
        e.step(5)
        assert not e.call("debug.vm", {})["mission_won"]
        e.call("debug.vm", {"win": True})
        e.step(1)
        ui = e.observe(entities=False)["ui"]
        assert ui["screen"] == "debriefing", ui
        e.step(1, [{"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": "enter"}])
        assert e.observe(entities=False)["ui"]["screen"] == "main_menu"

