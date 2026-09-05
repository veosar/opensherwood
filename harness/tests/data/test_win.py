"""The first mission won through play with canonical input (`docs/original/h01-win-path.md` 4.1).

The tutorial's victory test is one mission variable, set by the pick-up of one scroll; the walk to it
crosses no zone with a handler. The outcome is tainted (the scroll-pickup radius and the door stubs
are hypotheses, ADR-0008), so the test asserts the taint as well as the win.
"""

from __future__ import annotations

from opensherwood_harness import Engine, pointer_click


def hold(key: str, ticks: int) -> list[dict]:
    return [
        {"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": key},
        {"tick_offset": ticks - 1, "sequence": 0, "kind": "key_up", "key": key},
    ]


def test_first_mission_is_won_by_walking_to_the_sons_scroll(binary, game_dir, tmp_path):
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=600) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=1)
        e.skip_briefing()
        e.step(1, pointer_click(512, 384, "left"))  # select Robin
        e.step(150, hold("left", 150))  # camera to the west edge
        e.step(93, hold("up", 93))  # and north
        e.step(1, pointer_click(28, 100, "left"))  # walk order to the scroll at map (253,380)
        won = False
        for _ in range(12):
            e.step(300)
            obs = e.observe(entities=False)
            if obs.get("ui") and obs["ui"]["screen"] == "debriefing":
                won = True
                break
        assert won, "no won page within 3600 ticks"
        vm = e.call("debug.vm", {})
        assert vm["mission_won"] and not vm["mission_lost"]
        assert vm["tainted"] and any("scroll" in str(a) for a in vm["assumptions"]), vm["assumptions"]
