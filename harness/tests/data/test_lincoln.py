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
        assert nav["backend"] == "layered", nav
        assert nav["navigation"]["point"]["valid"], nav
        assert nav["navigation"]["nodes"] > 0 and nav["navigation"]["layers"] > 1
        e.step(2, pointer_click(sx, sy, "left"))
        assert e.observe(entities=False)["selected"] is not None
        # The old flat-grid target is outside the sector; rejecting it is part of this test.
        invalid = e.call("debug.nav", {"x": robin["x"] // 256 - 150, "y": robin["y"] // 256 + 30})
        assert not invalid["navigation"]["point"]["valid"]
        e.step(2, pointer_click(sx - 150, sy + 30, "left"))
        p = next(x for x in e.observe()["entities"] if x["kind"] == "player")
        assert p["walk"] is None and (p["x"], p["y"]) == (robin["x"], robin["y"])
        e.step(2, pointer_click(sx + 80, sy, "left"))
        for _ in range(60):
            e.step(1)
            p = next(x for x in e.observe()["entities"] if x["kind"] == "player")
            if p["x"] != robin["x"]:
                break
        assert p["walk"] is not None and p["x"] != robin["x"], "snapshot must be taken during real movement"
        snap = e.snapshot()
        straight = e.step(100, hash_every_tick=True)["per_tick"]
        p2 = next(x for x in e.observe()["entities"] if x["kind"] == "player")
        assert abs(p2["x"] - robin["x"]) > 50 * 256, "Robin did not move"
        e.restore(snapshot=snap["snapshot"])
        assert e.step(100, hash_every_tick=True)["per_tick"] == straight
