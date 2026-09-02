"""The first mission's map (Lincoln, layers): walking must work from the start position."""

from __future__ import annotations

from opensherwood_harness import Engine, pointer_click


def test_robin_can_walk_from_the_first_mission_start(binary, game_dir):
    with Engine(binary=binary, game_dir=game_dir, timeout=120) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=0)
        e.skip_briefing()
        obs = e.observe()
        robin = next(x for x in obs["entities"] if x["kind"] == "player")
        cam = obs["camera"]
        sx, sy = robin["x"] // 256 - cam[0], robin["y"] // 256 - cam[1]
        nav = e.call("debug.nav", {"x": robin["x"] // 256, "y": robin["y"] // 256})
        assert nav["geometry_walkable"] and nav["cell_walkable"], nav
        assert nav["areas"] > 100, "Lincoln's projection areas must be loaded"
        e.step(2, pointer_click(sx, sy, "left"))
        assert e.observe(entities=False)["selected"] is not None
        e.step(2, pointer_click(sx - 150, sy + 30, "right"))
        p = next(x for x in e.observe()["entities"] if x["kind"] == "player")
        assert p["target"] is not None and len(p["path"]) >= 1
        e.step(300)
        p2 = next(x for x in e.observe()["entities"] if x["kind"] == "player")
        assert abs(p2["x"] - robin["x"]) > 50 * 256, "Robin did not move"
