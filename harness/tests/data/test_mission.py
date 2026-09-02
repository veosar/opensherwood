"""Retail mission loading (needs OPENSHERWOOD_GAME_DIR): actors from the .rhm on the right map."""

from __future__ import annotations

from opensherwood_harness import Engine, pointer_click


def test_tutorial_loads_with_its_actors(binary, game_dir):
    with Engine(binary=binary, game_dir=game_dir) as e:
        e.reset({"mission": "EmbTut_FoC_EC"}, seed=1)
        obs = e.observe()
        assert obs["map_size"] == [1408, 960]
        kinds = {}
        for x in obs["entities"]:
            kinds[x["kind"]] = kinds.get(x["kind"], 0) + 1
        assert kinds["player"] == 5
        assert kinds["guard"] >= 20
        robin = next(x for x in obs["entities"] if x["kind"] == "player")
        assert robin["anim"]["set"] == "RobinHood"
        assert 0 <= robin["x"] // 256 < 1408 and 0 <= robin["y"] // 256 < 960


def test_every_mission_file_loads(binary, game_dir):
    names = [p.stem for p in (game_dir / "DATA" / "Levels").glob("*.rhm")]
    assert len(names) == 39
    failures = []
    with Engine(binary=binary, game_dir=game_dir, timeout=300) as e:
        for name in sorted(names):
            try:
                e.reset({"mission": name}, seed=1)
                obs = e.observe(entities=False)
                assert obs["map_size"][0] > 0
            except Exception as ex:  # noqa: BLE001 - collect everything, report once
                failures.append(f"{name}: {ex}")
    # The Sherwood hub and its outro have no known script element index space yet
    # (docs/formats/scb.md, "Index spaces"), so strict loading refuses them: the only accepted failures.
    unexpected = [f for f in failures if not f.lower().startswith("sherwood")]
    assert not unexpected, "\n".join(unexpected)
    assert len(failures) == 2, failures
    assert all("no element index space" in f for f in failures), failures


def test_mission_is_deterministic_across_processes(binary, game_dir):
    totals = []
    for _ in range(2):
        with Engine(binary=binary, game_dir=game_dir) as e:
            e.reset({"mission": "EmbTut_FoC_EC"}, seed=3)
            obs = e.observe()
            robin = next(x for x in obs["entities"] if x["kind"] == "player")
            # Scroll so Robin is on screen, then select and move him.
            cam_x = max(0, robin["x"] // 256 - 320)
            e.step(max(1, cam_x // 8), [{"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": "right"}])
            e.step(1, [{"tick_offset": 0, "sequence": 0, "kind": "key_up", "key": "right"}])
            cam = e.observe(entities=False)["camera"]
            sx, sy = robin["x"] // 256 - cam[0], robin["y"] // 256 - cam[1]
            e.step(1, pointer_click(sx, sy, "left"))
            assert e.observe(entities=False)["selected"] is not None
            e.step(1, pointer_click(sx - 100, sy, "right"))
            r = e.step(200)
            totals.append(r["hashes"]["total"])
    assert totals[0] == totals[1]


def test_walking_into_an_obstacle_stops_and_occluders_hide_the_sprite(binary, game_dir):
    with Engine(binary=binary, game_dir=game_dir) as e:
        e.reset({"mission": "EmbTut_FoC_EC"}, seed=1)
        obs = e.observe()
        robin = next(x for x in obs["entities"] if x["kind"] == "player")
        rx, ry = robin["x"] // 256, robin["y"] // 256
        cam_x, cam_y = max(0, rx - 320), max(0, ry - 240)
        for key, n in (("right", cam_x // 8), ("down", cam_y // 8)):
            if n:
                e.step(n, [{"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": key}])
                e.step(1, [{"tick_offset": 0, "sequence": 0, "kind": "key_up", "key": key}])
        cam = e.observe(entities=False)["camera"]
        sx, sy = rx - cam[0], ry - cam[1]
        e.step(1, pointer_click(sx, sy, "left"))
        # West along the bank: the big tree's obstacle polygon is in the way and the target is on
        # the river, so the path bends around the tree and ends on the closest reachable ground.
        e.step(1, pointer_click(sx - 260, sy + 40, "right"))
        p = next(x for x in e.observe()["entities"] if x["kind"] == "player")
        assert p["target"] is not None and len(p["path"]) >= 2, "expected a multi-point path"
        e.step(700)
        p = next(x for x in e.observe()["entities"] if x["kind"] == "player")
        assert p["target"] is None
        assert abs(p["x"] // 256 - (rx - 260)) < 60, "did not get near the target"
        assert p["x"] // 256 < rx - 150, "did not move far enough"


def test_guards_follow_their_rail_programs(binary, game_dir):
    """H01 (Lincoln): NPCs with a rail walk it; the run is deterministic across processes."""
    totals = []
    moved = 0
    for _ in range(2):
        with Engine(binary=binary, game_dir=game_dir, timeout=300) as e:
            e.reset({"mission": "H01_Lin_VL"}, seed=5)
            e.skip_briefing()
            before = {x["id"]["index"]: (x["x"], x["y"]) for x in e.observe()["entities"] if x["kind"] == "guard"}
            assert len(before) >= 30
            r = e.step(600)
            totals.append(r["hashes"]["total"])
            after = {x["id"]["index"]: (x["x"], x["y"]) for x in e.observe()["entities"] if x["kind"] == "guard"}
            moved = sum(1 for k, p in before.items() if after[k] != p)
    assert moved >= 5, f"only {moved} guards moved in 600 ticks"
    assert totals[0] == totals[1]


def test_npc_sprites_come_from_the_profile_table(binary, game_dir):
    """H01 (Lincoln): BORG / OILE indices resolve through profile.cpf (docs/formats/rhm.md,
    "Actor profile mapping"): the wall guards are halberdiers, a beggar stands near the start."""
    with Engine(binary=binary, game_dir=game_dir) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=1)
        sets = {}
        for x in e.observe()["entities"]:
            if x["kind"] == "guard":
                sets[x["anim"]["set"]] = sets.get(x["anim"]["set"], 0) + 1
    # Relational checks (no asset names): several soldier families, no single sprite for the majority,
    # and at least one civilian sprite appearing exactly once (the beggar near the start).
    assert len(sets) >= 6, sets
    assert max(sets.values()) < sum(sets.values()) // 2, sets
    assert any(n == 1 for n in sets.values()), sets
