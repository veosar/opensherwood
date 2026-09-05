"""Retail mission loading (needs OPENSHERWOOD_GAME_DIR): actors from the .rhm on the right map."""

from __future__ import annotations

from opensherwood_harness import Engine, key_press, pointer_click


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
            e.step(1, pointer_click(sx - 100, sy, "left"))
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
        e.step(1, pointer_click(sx - 260, sy + 40, "left"))
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


def _hero_on_screen(e):
    """Select the first player character with a left click; returns his viewport position."""
    obs = e.observe()
    robin = next(x for x in obs["entities"] if x["kind"] == "player")
    cam = obs["camera"]
    sx, sy = robin["x"] // 256 - cam[0], robin["y"] // 256 - cam[1]
    e.step(1, pointer_click(sx, sy, "left"))
    assert e.observe(entities=False)["selected"] is not None
    return sx, sy


def _hero(e):
    return next(x for x in e.observe()["entities"] if x["kind"] == "player")


def test_double_click_runs_and_c_s_crouch_and_stand(binary, game_dir):
    """H01 (Lincoln), `docs/original/ui-flow.md` 9.4: a left click on the ground walks, a double
    click runs (twice as far in the same ticks, the run animation block), `c` crouches Robin (the
    crouched idle / sneak blocks, half speed) and `s` stands him up. Every action is canonical input."""
    covered = {}
    for mode in ("walk", "run"):
        with Engine(binary=binary, game_dir=game_dir, timeout=120) as e:
            e.reset({"mission": "H01_Lin_VL"}, seed=0)
            e.skip_briefing()
            sx, sy = _hero_on_screen(e)
            start = _hero(e)
            walk_anim = None
            e.step(1, pointer_click(sx - 150, sy + 30, "left"))
            if mode == "run":
                e.step(1, pointer_click(sx - 150, sy + 30, "left"))
            p = _hero(e)
            assert p["target"] is not None and p["gait"] == mode and p["posture"] == "standing"
            e.step(20)
            p = _hero(e)
            covered[mode] = abs(p["x"] - start["x"]) + abs(p["y"] - start["y"])
            covered[mode + "_anim"] = p["anim"]["animation"]
    assert covered["run"] > covered["walk"] * 3 // 2, covered
    assert covered["run_anim"] != covered["walk_anim"], "running uses another animation block"

    with Engine(binary=binary, game_dir=game_dir, timeout=120) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=0)
        e.skip_briefing()
        sx, sy = _hero_on_screen(e)
        standing_idle = _hero(e)["anim"]["animation"]
        e.step(1, key_press({"letter": "c"}))
        p = _hero(e)
        assert p["posture"] == "crouched"
        e.step(5)
        crouched_idle = _hero(e)["anim"]["animation"]
        assert crouched_idle != standing_idle, "Robin has a crouched idle block (action 14)"
        start = _hero(e)
        e.step(1, pointer_click(sx - 150, sy + 30, "left"))
        e.step(20)
        p = _hero(e)
        sneaked = abs(p["x"] - start["x"]) + abs(p["y"] - start["y"])
        assert p["anim"]["animation"] not in (crouched_idle, standing_idle, covered["walk_anim"])
        assert 0 < sneaked < covered["walk"], (sneaked, covered)
        e.step(1, key_press({"letter": "s"}))
        p = _hero(e)
        assert p["posture"] == "standing"
        # Right click on the selected character cancels his order; on the ground it deselects.
        cam = e.observe(entities=False)["camera"]
        e.step(1, pointer_click(p["x"] // 256 - cam[0], p["y"] // 256 - cam[1], "right"))
        p = _hero(e)
        assert p["target"] is None and e.observe(entities=False)["selected"] is not None
        e.step(1, pointer_click(sx + 200, sy, "right"))
        assert e.observe(entities=False)["selected"] is None


def _facing_vector(facing256):
    """Unit vector of a facing in 1/256 turns (0 = +x, clockwise on screen)."""
    import math

    a = facing256 / 256 * 2 * math.pi
    return math.cos(a), math.sin(a)


def _scroll_to(e, x, y):
    """Scroll the camera so that map point (x, y) is near the viewport centre; returns the camera."""
    cam = e.observe(entities=False)["camera"]
    tx, ty = max(0, x - 512), max(0, y - 384)
    for key, d in (("right", tx - cam[0]), ("left", cam[0] - tx), ("down", ty - cam[1]), ("up", cam[1] - ty)):
        n = d // 8
        if n > 0:
            e.step(n, [{"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": key}])
            e.step(1, [{"tick_offset": 0, "sequence": 0, "kind": "key_up", "key": key}])
    return e.observe(entities=False)["camera"]


def _click_map(e, x, y, button="left"):
    """Left / right click on a map point through the pointer (scrolls there first)."""
    cam = _scroll_to(e, x, y)
    e.step(1, pointer_click(x - cam[0], y - cam[1], button))


def _entity(e, index):
    return e.observe()["entities"][index]


def _pos(x):
    return x["x"] // 256, x["y"] // 256


def _nearest_soldier(e):
    """The living, unlocked enemy soldier closest to the first player character (index, entity)."""
    import math

    obs = e.observe()
    robin = next(x for x in obs["entities"] if x["kind"] == "player")
    rx, ry = _pos(robin)
    soldiers = [
        x
        for x in obs["entities"]
        if x["kind"] == "guard" and x["team"] == "enemy" and x["alive"] and x["active"] and not x["ai_locked"]
    ]
    guard = min(soldiers, key=lambda x: math.hypot(_pos(x)[0] - rx, _pos(x)[1] - ry))
    return guard["id"]["index"], guard


def _walk_until_arrived(e, index_of_player, max_ticks, watch=None):
    """Step until the player character's order ends; returns the set of `ai_state` values the
    watched entity showed meanwhile."""
    seen = set()
    for _ in range(max_ticks):
        e.step(1)
        obs = e.observe()
        if watch is not None:
            seen.add(obs["entities"][watch]["ai_state"])
        if obs["entities"][index_of_player]["target"] is None:
            return seen
    raise AssertionError("the order never ended")


def test_running_past_a_soldier_is_noticed_then_the_alarm(binary, game_dir):
    """H01 (Lincoln), `docs/original/stealth-and-combat.md` "Engine": a running Robin inside a
    soldier's noise radius (or view cone) is noticed (`ai_state` `noticed`, action 141) and the
    soldier raises the alarm (`alarm`, 142), then searches (`alerted`). The nearest soldier to the
    start is an archer of the training scene, whose `ActionChange(_, 141)` ends the archery
    training loop (`docs/formats/scb.md`, H01)."""
    with Engine(binary=binary, game_dir=game_dir, timeout=300) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=0)
        e.skip_briefing()
        obs = e.observe()
        robin_index = next(i for i, x in enumerate(obs["entities"]) if x["kind"] == "player")
        gi, guard = _nearest_soldier(e)
        assert guard["ai_state"] == "patrol" and guard["action"] == 0
        rx, ry = _pos(obs["entities"][robin_index])
        gx, gy = _pos(guard)
        # Run to the point 60 px short of the soldier on the straight line from Robin: well inside
        # the 150 px noise radius whatever the soldier faces.
        import math

        d = math.hypot(gx - rx, gy - ry)
        tx, ty = round(gx - (gx - rx) / d * 60), round(gy - (gy - ry) / d * 60)
        _click_map(e, rx, ry)
        assert e.observe(entities=False)["selected"] is not None
        _click_map(e, tx, ty)
        cam = e.observe(entities=False)["camera"]
        e.step(1, pointer_click(tx - cam[0], ty - cam[1], "left"))
        assert _entity(e, robin_index)["gait"] == "run"
        states = []
        for _ in range(600):
            e.step(1)
            g = _entity(e, gi)
            if not states or states[-1] != g["ai_state"]:
                states.append(g["ai_state"])
            if g["ai_state"] == "alerted":
                break
        assert states[:4] == ["patrol", "noticed", "alarm", "alerted"], states
        g = _entity(e, gi)
        assert g["last_seen"] is not None and g["alert_origin"] is not None
        assert g["action"] in (140, 143, 151), g["action"]
        vm = e.call("debug.vm")
        assert not vm["faulted"] and vm["counters"]["traps"] == 0
        # The taint (ADR-0008, "Hypotheses and taint"): the run's first tick already recorded the
        # steward objective's stub and the tick rate; `perception` is recorded only when an alert
        # action id reaches an `ActionChange` handler, and the class of this soldier (the last
        # element of the soldier range, not one of the archery-training classes) has none, so the
        # script never saw the alert and no `perception` / `knock_out` entry appears.
        sc = e.observe(entities=False)["script"]
        assert sc["tainted"], sc
        assert {"stub_result": 235} in sc["assumptions"] and "tick_rate" in sc["assumptions"]
        assert "perception" not in sc["assumptions"] and "knock_out" not in sc["assumptions"]


def test_a_crouched_approach_from_behind_is_not_noticed(binary, game_dir):
    """H01: Robin sneaks (crouched, `c`) to a point 60 px behind the nearest soldier, outside
    his view cone all the way: the soldier stays on patrol."""
    with Engine(binary=binary, game_dir=game_dir, timeout=300) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=0)
        e.skip_briefing()
        obs = e.observe()
        robin_index = next(i for i, x in enumerate(obs["entities"]) if x["kind"] == "player")
        gi, guard = _nearest_soldier(e)
        gx, gy = _pos(guard)
        c, s = _facing_vector(guard["facing256"])
        behind = (round(gx - 60 * c), round(gy - 60 * s))
        rx, ry = _pos(obs["entities"][robin_index])
        _click_map(e, rx, ry)
        e.step(1, key_press({"letter": "c"}))
        assert _entity(e, robin_index)["posture"] == "crouched"
        _click_map(e, *behind)
        assert _entity(e, robin_index)["target"] is not None
        seen = _walk_until_arrived(e, robin_index, 1500, watch=gi)
        assert seen == {"patrol"}, seen
        p = _entity(e, robin_index)
        assert abs(_pos(p)[0] - behind[0]) + abs(_pos(p)[1] - behind[1]) < 16
        g = _entity(e, gi)
        assert g["ai_state"] == "patrol" and g["last_seen"] is None


def test_knock_out_from_behind_puts_the_soldier_out_of_action(binary, game_dir):
    """H01: the soldier the level script polls with native 90 every tick (its `Hourglass` tests
    element 87 first, `docs/formats/scb.md` H01 notes) stands at a corridor post facing away from
    the corridor. Robin runs to a staging point down the corridor, sneaks behind him and is
    ordered onto him with a left click: Robin plays the knock-out blow (123), the soldier goes
    down (41), lies knocked out (47) while native 90 reports him out of action (`debug.vm`
    counter `out_of_action_true` grows), then gets up (49) and stands again, and the script's
    reaction gives the girl her running gait (native 140)."""
    with Engine(binary=binary, game_dir=game_dir, timeout=600) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=0)
        e.skip_briefing()
        obs = e.observe()
        robin_index = next(i for i, x in enumerate(obs["entities"]) if x["kind"] == "player")
        elements = obs["script"]["actor_elements"]
        vi = elements.index(87)
        victim = obs["entities"][vi]
        assert victim["kind"] == "guard" and victim["team"] == "enemy" and victim["ai_state"] == "patrol"
        assert victim["hit_points"] > 0 and victim["knockout_resistance"] == 0
        vx, vy = _pos(victim)
        c, s = _facing_vector(victim["facing256"])
        # Staging point: 240 px down the corridor (south-west), then the spot 60 px behind him.
        staging = (vx - 170, vy + 170)
        behind = (round(vx - 60 * c), round(vy - 60 * s))
        assert e.call("debug.nav", {"x": vx, "y": vy, "to": list(staging)})["path_cells"]
        rx, ry = _pos(obs["entities"][robin_index])
        _click_map(e, rx, ry)
        _click_map(e, *staging)
        cam = e.observe(entities=False)["camera"]
        e.step(1, pointer_click(staging[0] - cam[0], staging[1] - cam[1], "left"))
        assert _entity(e, robin_index)["gait"] == "run"
        for _ in range(60):
            e.step(50)
            if _entity(e, robin_index)["target"] is None:
                break
        assert _entity(e, robin_index)["target"] is None, "never reached the staging point"
        assert _entity(e, vi)["ai_state"] == "patrol", "the run was out of his earshot"
        e.step(1, key_press({"letter": "c"}))
        _click_map(e, *behind)
        seen = _walk_until_arrived(e, robin_index, 1200, watch=vi)
        assert seen == {"patrol"}, seen
        before = e.call("debug.vm")["counters"]["out_of_action_true"]
        assert before == 0
        v = _entity(e, vi)
        _click_map(e, *_pos(v))
        p = _entity(e, robin_index)
        assert p["attack_target"] == v["id"], "a left click on an enemy is an attack order"
        states = []
        for _ in range(300):
            e.step(1)
            v = _entity(e, vi)
            if not states or states[-1] != v["ai_state"]:
                states.append(v["ai_state"])
            if v["ai_state"] == "lying":
                break
        assert states == ["patrol", "knocked_down", "lying"], states
        p = _entity(e, robin_index)
        assert p["action"] == 123 or p["ai_state"] == "patrol"
        assert v["action"] == 47 and v["fell_backward"] is False
        e.step(5)
        vm = e.call("debug.vm")
        assert vm["counters"]["out_of_action_true"] > 0, "the script's Hourglass polls native 90 on him"
        assert not vm["faulted"] and vm["counters"]["traps"] == 0
        # Native 90 reported the knock-out and the blow consulted `p4`: both hypotheses taint the outcome.
        sc = e.observe(entities=False)["script"]
        assert sc["tainted"], sc
        assert "knock_out" in sc["assumptions"] and "profile_stats" in sc["assumptions"], sc["assumptions"]
        # The script reacts: the girl gets a path and the running gait (natives 132 / 140).
        girl = next(x for x in e.observe()["entities"] if x["npc_gait"] == "run")
        assert girl["kind"] == "guard"
        # He sleeps for the knock-out timer, then gets up and is back on his feet.
        got_up = False
        for _ in range(16):
            e.step(50)
            st = _entity(e, vi)["ai_state"]
            if st in ("getting_up", "patrol", "returning"):
                got_up = True
                break
        assert got_up, _entity(e, vi)["ai_state"]
        e.step(60)
        v = _entity(e, vi)
        assert v["ai_state"] in ("patrol", "returning", "noticed", "alarm", "alerted"), v["ai_state"]
        assert e.call("debug.vm")["counters"]["out_of_action_true"] >= before + 1
