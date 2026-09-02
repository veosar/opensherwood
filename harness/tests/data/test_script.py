"""The mission script VM (needs OPENSHERWOOD_GAME_DIR): the retail scripts run in the engine.

`docs/formats/scb.md` ("First mission script walkthrough", "Engine") and ADR-0008. The first mission's
`PostInitialize` adds the primary objective 0 and plays one sequence: text pages 0, 1, 2, then the camera
returns to the hero. Texts are dismissed through `debug.vm {dismiss_text}` here (the window dismisses them
from the briefing parchment); nothing else about the script is privileged: `Hourglass` and
`CheckVictoryCondition` run inside `step`.
"""

from __future__ import annotations

import re

from opensherwood_harness import Engine

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
