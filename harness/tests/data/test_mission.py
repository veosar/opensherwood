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
    # All 39 load strictly, the Sherwood hub and its outro included (`docs/formats/sherwood-hub.md`).
    assert not failures, "\n".join(failures)


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
    click runs (the run animation block, 5 / 4 of the walking speed: `docs/original/stealth-and-combat.md`
    8.3), `c` crouches Robin (the crouched idle / sneak blocks, a fifth of the walking speed, 8.2)
    and `s` stands him up. Every action is canonical input."""
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
    assert covered["walk"] * 11 // 10 < covered["run"] < covered["walk"] * 14 // 10, covered
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


def test_running_near_a_soldier_alerts_him_at_once_from_afar(binary, game_dir):
    """H01 (Lincoln), `docs/original/stealth-and-combat.md` 8.6 (measured): a running Robin is
    heard by soldiers not facing him from 330 px and more, and they charge at once (`ai_state`
    `alerted`, the alert run 151, no `noticed` / `alarm` pause). The nearest soldier to the
    start, an archer of the training scene about 270 px away, hears the first running tick and
    charges; a soldier 370 px away, beyond the noise radius for the whole run, stays on patrol."""
    import math

    with Engine(binary=binary, game_dir=game_dir, timeout=300) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=0)
        e.skip_briefing()
        obs = e.observe()
        robin_index = next(i for i, x in enumerate(obs["entities"]) if x["kind"] == "player")
        rx, ry = _pos(obs["entities"][robin_index])
        gi, guard = _nearest_soldier(e)
        assert guard["ai_state"] == "patrol" and guard["action"] == 0
        gx, gy = _pos(guard)
        near = math.hypot(gx - rx, gy - ry)
        assert 250 < near < 300, near
        # A soldier out of earshot: more than 350 px from Robin's start and from every point of a
        # 150 px run west of it.
        soldiers = [
            (i, x)
            for i, x in enumerate(obs["entities"])
            if x["kind"] == "guard" and x["team"] == "enemy" and x["alive"] and x["active"] and i != gi
        ]
        far = [(i, x) for i, x in soldiers if 360 < math.hypot(_pos(x)[0] - rx, _pos(x)[1] - ry) < 400]
        fi, far_guard = min(far, key=lambda ix: math.hypot(_pos(ix[1])[0] - rx, _pos(ix[1])[1] - ry))
        tx, ty = rx - 150, ry
        assert math.hypot(_pos(far_guard)[0] - tx, _pos(far_guard)[1] - ty) > 360
        _click_map(e, rx, ry)
        assert e.observe(entities=False)["selected"] is not None
        _click_map(e, tx, ty)
        cam = e.observe(entities=False)["camera"]
        e.step(1, pointer_click(tx - cam[0], ty - cam[1], "left"))
        assert _entity(e, robin_index)["gait"] == "run"
        # The tick of the double click: the archer heard the run and is already charging.
        g = _entity(e, gi)
        assert g["ai_state"] == "alerted" and g["heard"] is True, g
        assert g["gait"] == "run" and g["target"] is not None
        assert g["last_seen"] is not None and g["alert_origin"] is not None
        assert g["action"] == 151, g["action"]
        states = {"alerted"}
        for _ in range(120):
            e.step(1)
            states.add(_entity(e, gi)["ai_state"])
            assert _entity(e, fi)["ai_state"] == "patrol", "out of earshot"
        assert "noticed" not in states and "alarm" not in states, states
        vm = e.call("debug.vm")
        assert not vm["faulted"] and vm["counters"]["traps"] == 0
        # The taint (ADR-0008, "Hypotheses and taint"): the run's first tick already recorded the
        # steward objective's stub and the tick rate; the alert action ids of a soldier alerted
        # through the measured noise channel (`heard`) record no `perception`, whether or not
        # his class handles `ActionChange`.
        sc = e.observe(entities=False)["script"]
        assert sc["tainted"], sc
        assert {"stub_result": 235} in sc["assumptions"] and "tick_rate" in sc["assumptions"]
        assert "perception" not in sc["assumptions"] and "knock_out" not in sc["assumptions"]


def _path_length(e, index, ticks, every=10):
    """Distance a character covers over `ticks` ticks, sampled every `every` ticks (map px)."""
    import math

    total = 0.0
    last = _pos(_entity(e, index))
    for _ in range(ticks // every):
        e.step(every)
        p = _pos(_entity(e, index))
        total += math.hypot(p[0] - last[0], p[1] - last[1])
        last = p
    return total


def test_walk_run_and_sneak_speeds_match_the_measurements(binary, game_dir):
    """H01 (Lincoln), `docs/original/stealth-and-combat.md` 8.1-8.3 (measured): Robin walks at
    85.3 px/s, runs (double click, action 7) at 101 +- 10 px/s (106.7 from the table) and sneaks
    at 17.8 px/s, along the analyst's line from the start (down-left across the courtyard).
    Asserted within 5 % over 120 ticks (2 s at 60 Hz) against the table-derived values."""
    expected = {"walk": 85.33, "run": 106.67, "sneak": 18.0}
    for mode, px_per_s in expected.items():
        with Engine(binary=binary, game_dir=game_dir, timeout=300) as e:
            e.reset({"mission": "H01_Lin_VL"}, seed=0)
            e.skip_briefing()
            obs = e.observe()
            robin_index = next(i for i, x in enumerate(obs["entities"]) if x["kind"] == "player")
            rx, ry = _pos(obs["entities"][robin_index])
            tx, ty = rx - 326, ry + 185
            _click_map(e, rx, ry)
            if mode == "sneak":
                e.step(1, key_press({"letter": "c"}))
            _click_map(e, tx, ty)
            if mode == "run":
                cam = e.observe(entities=False)["camera"]
                e.step(1, pointer_click(tx - cam[0], ty - cam[1], "left"))
            p = _entity(e, robin_index)
            assert p["target"] is not None
            assert p["gait"] == ("run" if mode == "run" else "walk")
            assert p["posture"] == ("crouched" if mode == "sneak" else "standing")
            covered = _path_length(e, robin_index, 120)
            assert _entity(e, robin_index)["target"] is not None, "arrived before the 120 ticks were up"
            want = px_per_s * 2.0
            assert abs(covered - want) <= want * 0.05, (mode, covered, want)


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
    ordered onto him with a left click (the approach walks: a run would be heard from 350 px,
    `docs/original/stealth-and-combat.md` 8.6): Robin plays the knock-out blow (123), the soldier goes
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
        assert victim["hp"] > 0 and victim["knockout_resistance"] == 0
        vx, vy = _pos(victim)
        c, s = _facing_vector(victim["facing256"])
        # Staging point: 240 px down the corridor (south-west), then the spot 60 px behind him.
        staging = (vx - 170, vy + 170)
        behind = (round(vx - 60 * c), round(vy - 60 * s))
        assert e.call("debug.nav", {"x": vx, "y": vy, "to": list(staging)})["path_cells"]
        rx, ry = _pos(obs["entities"][robin_index])
        _click_map(e, rx, ry)
        _click_map(e, *staging)
        assert _entity(e, robin_index)["gait"] == "walk", "a walk makes no noise"
        for _ in range(120):
            e.step(50)
            if _entity(e, robin_index)["target"] is None:
                break
        assert _entity(e, robin_index)["target"] is None, "never reached the staging point"
        assert _entity(e, vi)["ai_state"] == "patrol", "the walk stayed out of his cone"
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
        # Getting up (action 49) lasts 24 table ticks = 68 world ticks on the animation clock.
        e.step(80)
        v = _entity(e, vi)
        assert v["ai_state"] in ("patrol", "returning", "noticed", "alarm", "alerted"), v["ai_state"]
        assert e.call("debug.vm")["counters"]["out_of_action_true"] >= before + 1


def _halberdier(e):
    """The halberdier at the arch of the right-hand wall, the one soldier a walking hero can reach
    from the start (`docs/original/combat-measurements.md`: feet at screen (932, 347) with Robin at
    (513, 392) under the start camera): the enemy soldier nearest to that offset from Robin, with the
    profile's 80 hit points (`pre[0]`, confirmed there). Returns `(index, entity)`."""
    import math

    obs = e.observe()
    robin = next(x for x in obs["entities"] if x["kind"] == "player")
    rx, ry = _pos(robin)
    want = (rx + 932 - 513, ry + 347 - 392)
    soldiers = [
        (i, x)
        for i, x in enumerate(obs["entities"])
        if x["kind"] == "guard" and x["team"] == "enemy" and x["alive"] and x["active"]
    ]
    gi, guard = min(soldiers, key=lambda ix: math.hypot(_pos(ix[1])[0] - want[0], _pos(ix[1])[1] - want[1]))
    assert math.hypot(_pos(guard)[0] - want[0], _pos(guard)[1] - want[1]) < 40, (_pos(guard), want)
    assert guard["hp_max"] == 80 and guard["hp"] == 80, guard
    return gi, guard


def _click_actor(e, index, button="left"):
    """Click on an actor where he stands: scroll to him, then click his position on the next
    tick (a soldier walking his rail moves between the scroll and the click otherwise)."""
    _scroll_to(e, *_pos(_entity(e, index)))
    cam = e.observe(entities=False)["camera"]
    x, y = _pos(_entity(e, index))
    e.step(1, pointer_click(x - cam[0], y - cam[1], button))


def _screen(entity, cam):
    """Viewport position of an entity's feet as the renderer places them (24.8 rounded)."""
    return round(entity["x"] / 256) - cam[0], round(entity["y"] / 256) - cam[1]


def _pixel(path, x, y):
    from PIL import Image

    with Image.open(path) as im:
        return im.convert("RGB").getpixel((x, y))


def _start_fight(e):
    """Robin (selected) attacks the halberdier with a left click on him and walks up until both
    fight; returns `(robin_index, halberdier_index)`."""
    obs = e.observe()
    robin_index = next(i for i, x in enumerate(obs["entities"]) if x["kind"] == "player")
    gi, guard = _halberdier(e)
    _click_map(e, *_pos(obs["entities"][robin_index]))
    _click_actor(e, gi)
    p = _entity(e, robin_index)
    assert p["attack_target"] == guard["id"], "a left click on the enemy is the attack order"
    assert p["target"] is not None, "closes in"
    for _ in range(900):
        e.step(1)
        if _entity(e, robin_index)["in_combat"]:
            break
    return robin_index, gi


def test_attack_order_closes_in_and_the_fight_starts_with_the_bars(binary, game_dir, tmp_path):
    """H01, `docs/original/combat-measurements.md` 1.1 / 1.2 (measured): a left click on the
    halberdier is the attack order, Robin walks up, stops about 52 px from his feet and both are
    in combat (the fighting stance, action 54) with their bars drawn under the feet: a 20 px red
    health row 8 px below the feet and a blue energy row 4 px lower, the hovered soldier's in the
    bright colours, Robin's in the dark ones; nothing of the sort while he walks."""
    import math

    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=300) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=0)
        e.skip_briefing()
        obs = e.observe()
        robin_index = next(i for i, x in enumerate(obs["entities"]) if x["kind"] == "player")
        robin = obs["entities"][robin_index]
        assert robin["hp"] == 100 and robin["hp_max"] == 100 and robin["energy"] == 20
        assert not robin["in_combat"] and robin["foe"] is None
        gi, guard = _halberdier(e)
        assert guard["energy"] == 20 and not guard["in_combat"]
        _click_map(e, *_pos(robin))
        _click_actor(e, gi)
        guard = _entity(e, gi)
        p = _entity(e, robin_index)
        assert p["attack_target"] == guard["id"] and p["target"] is not None
        # Walking: no bars under Robin's feet; the pointer rests on the soldier, whose bars show.
        e.step(5)
        cam = e.observe(entities=False)["camera"]
        sx, sy = _screen(_entity(e, robin_index), cam)
        walking = e.capture(path="walking.png")
        assert _pixel(walking["path"], sx - 10, sy + 8) not in ((123, 0, 0), (255, 0, 0))
        gsx, gsy = _screen(_entity(e, gi), cam)
        assert _pixel(walking["path"], gsx - 10, gsy + 8) == (255, 0, 0), "hovered: his bars show"
        ticks = 5
        while not _entity(e, robin_index)["in_combat"]:
            e.step(1)
            ticks += 1
            assert ticks < 900, "never reached him"
        p, g = _entity(e, robin_index), _entity(e, gi)
        assert p["ai_state"] == "fighting" and g["ai_state"] == "fighting"
        assert p["in_combat"] and g["in_combat"]
        assert p["foe"] == g["id"] and g["foe"] == p["id"]
        assert p["target"] is None and p["attack_target"] is None and g["target"] is None
        d = math.hypot(_pos(p)[0] - _pos(g)[0], _pos(p)[1] - _pos(g)[1])
        assert 47 <= d <= 53, d
        assert p["action"] == 54 and g["action"] == 54
        assert p["hp"] == 100 and g["hp"] == 80
        gx, gy = _pos(g)
        assert math.hypot(gx - _pos(guard)[0], gy - _pos(guard)[1]) < 60, "fights near his post"
        # The bars: the pointer moved onto the soldier (no click), his bars bright, Robin's dark.
        from opensherwood_harness import pointer_move

        cam = e.observe(entities=False)["camera"]
        e.step(1, [pointer_move(gx - cam[0], gy - cam[1], 0, 0)])
        cam = e.observe(entities=False)["camera"]
        fight = e.capture(path="fight.png")
        assert fight["hash"] != walking["hash"]
        sx, sy = _screen(_entity(e, robin_index), cam)
        for dx in (0, 19):
            assert _pixel(fight["path"], sx - 10 + dx, sy + 8) == (123, 0, 0), (dx, "health")
            assert _pixel(fight["path"], sx - 10 + dx, sy + 10) == (123, 0, 0)
            assert _pixel(fight["path"], sx - 10 + dx, sy + 12) == (0, 101, 123), (dx, "energy")
            assert _pixel(fight["path"], sx - 10 + dx, sy + 14) == (0, 101, 123)
        assert _pixel(fight["path"], sx - 10, sy + 11) not in ((123, 0, 0), (0, 101, 123)), "4 px apart"
        gsx, gsy = _screen(_entity(e, gi), cam)
        assert _pixel(fight["path"], gsx - 10, gsy + 8) == (255, 0, 0)
        assert _pixel(fight["path"], gsx + 9, gsy + 12) == (0, 200, 255)
        vm = e.call("debug.vm")
        assert not vm["faulted"] and vm["counters"]["traps"] == 0


def test_the_soldiers_blows_wear_robin_down_to_the_lost_page(binary, game_dir, tmp_path):
    """H01, `docs/original/combat-measurements.md` 1.5 / 4 (measured): the halberdier's basic hits
    take 5 hp at a time at a median of 7.7 s between landed hits (a swing every ~5.3 s, two in
    three landing), each costing him one unit of energy regained in ~4 s, while Robin's own
    attacks never hurt him and cost no energy; health never regenerates; after about three
    minutes Robin is at 0 hp, dead for the world (`hero_dead`, `alive` false, the fall then the
    lying pose) and the app shows the lost page on the same tick. A snapshot taken mid-fight
    restores to the same run (hashes equal)."""
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=600) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=0)
        e.skip_briefing()
        robin_index, gi = _start_fight(e)
        assert _entity(e, robin_index)["in_combat"]
        hp_at = {}
        hits = []
        soldier_spent = False
        soldier_regained = False
        hp = 100
        snapshot = None
        hashes_after_snapshot = None
        for chunk in range(1, 481):
            e.step(30)
            obs = e.observe()
            p, g = obs["entities"][robin_index], obs["entities"][gi]
            assert g["hp"] == 80, "Robin's click attacks never land on the pole arm"
            assert p["energy"] == 20, "click attacks cost no energy"
            assert p["hp"] <= hp, "health never regenerates"
            if p["hp"] < hp:
                assert (hp - p["hp"]) % 5 == 0, (hp, p["hp"])
                hits.append(obs["tick"])
                hp = p["hp"]
            if g["energy"] < 20:
                soldier_spent = True
                assert g["energy"] == 19 and g["energy_ticks"] > 0
            elif soldier_spent:
                soldier_regained = True
            hp_at[chunk * 30] = p["hp"]
            if chunk == 60 and snapshot is None:
                # Determinism across a mid-fight snapshot: restore, replay one tick, same hash.
                snapshot = e.snapshot()
                hashes_after_snapshot = e.step(1)["hashes"]["total"]
                e.restore(snapshot_id=snapshot["id"])
                assert e.step(1)["hashes"]["total"] == hashes_after_snapshot
            if not p["alive"]:
                break
        else:
            raise AssertionError(f"Robin survived 240 s: {hp} hp, hits at {hits}")
        # The cadence: 100 -> about 40 hp after 100 s in the model (measured 100 -> 30 in 90 s
        # with a 25-hp blow among the hits); asserted loosely.
        assert 10 <= hp_at[6000] <= 70, hp_at[6000]
        assert soldier_spent and soldier_regained
        intervals = [b - a for a, b in zip(hits, hits[1:])]
        mean = sum(intervals) / len(intervals)
        assert 330 <= mean <= 660, (mean, intervals)
        obs = e.observe()
        p, g = obs["entities"][robin_index], obs["entities"][gi]
        assert p["hp"] == 0 and not p["alive"] and p["ai_state"] in ("dying", "dead")
        assert p["action"] in (44, 48), p["action"]
        assert obs["hero_dead"] is True
        assert obs.get("ui", {}).get("screen") == "lost", obs.get("ui")
        # The world is frozen under the page on the tick of the blow: the soldier is unhurt.
        assert g["alive"] and g["hp"] == 80
        tick = obs["tick"]
        e.step(3)
        assert e.observe(entities=False)["tick"] == tick, "paused under the lost page"
        sc = obs["script"]
        assert "melee_reach" in sc["assumptions"], sc["assumptions"]
        assert "knock_out" not in sc["assumptions"]
        assert snapshot is not None and hashes_after_snapshot is not None


def _stroke(e, x, y):
    """The forward stroke (`combat-measurements.md` 1.4): the left button held, the pointer moved
    80 px right and 20 px up, released; two ticks of canonical events at viewport (x, y)."""
    from opensherwood_harness import pointer_move

    down = {"tick_offset": 0, "sequence": 1, "kind": "pointer_down", "button": "left"}
    up = {"tick_offset": 0, "sequence": 1, "kind": "pointer_up", "button": "left"}
    e.step(1, [pointer_move(x, y, 0, 0), down])
    e.step(1, [pointer_move(x + 80, y - 20, 0, 0), up])


def test_two_powerful_blows_kill_the_soldier_the_script_polls(binary, game_dir, tmp_path):
    """H01, `docs/original/combat-measurements.md` 1.4 (measured: 50 hp per landed blow, 2 of 6
    landing, the energy cost of two units regained one per ~0.9 s) on the corridor post soldier
    the level script polls with native 90 every tick (element 87, as in the knock-out test).
    Robin walks to the staging point down the corridor, runs a stretch so that the soldier hears
    him and charges (8.6), and is ordered onto him: they meet face to face and the fight starts
    (no knock-out from the front). Forward strokes are drawn until two land: the soldier falls
    (44), is dead for natives 85 / 87 / 90 (`out_of_action_true` grows) and lies for good (48);
    the hero survives. The hypotheses taken are recorded (`powerful_blow_chance`,
    `melee_reach`), the death is not a knock-out. A fight in flight survives a snapshot (restore
    mid-fight, same hashes after the same ticks)."""
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=600) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=0)
        e.skip_briefing()
        obs = e.observe()
        robin_index = next(i for i, x in enumerate(obs["entities"]) if x["kind"] == "player")
        vi = obs["script"]["actor_elements"].index(87)
        victim = obs["entities"][vi]
        assert victim["kind"] == "guard" and victim["team"] == "enemy"
        assert victim["hp"] == victim["hp_max"] == 80
        vx, vy = _pos(victim)
        staging = (vx - 170, vy + 170)
        _click_map(e, *_pos(obs["entities"][robin_index]))
        _click_map(e, *staging)
        for _ in range(120):
            e.step(50)
            if _entity(e, robin_index)["target"] is None:
                break
        assert _entity(e, robin_index)["target"] is None, "never reached the staging point"
        assert _entity(e, vi)["ai_state"] == "patrol"
        # A run towards him is heard: he charges. Then the attack order, so they meet face to face.
        run_to = (vx - 100, vy + 100)
        _click_map(e, *run_to)
        cam = e.observe(entities=False)["camera"]
        e.step(1, pointer_click(run_to[0] - cam[0], run_to[1] - cam[1], "left"))
        assert _entity(e, robin_index)["gait"] == "run"
        for _ in range(30):
            e.step(1)
            if _entity(e, vi)["ai_state"] == "alerted":
                break
        v = _entity(e, vi)
        assert v["ai_state"] == "alerted" and v["heard"] is True, v["ai_state"]
        # He is running: the click goes where he is.
        _click_actor(e, vi)
        assert _entity(e, robin_index)["attack_target"] == v["id"]
        for _ in range(900):
            e.step(1)
            if _entity(e, robin_index)["in_combat"]:
                break
        p, v = _entity(e, robin_index), _entity(e, vi)
        assert p["in_combat"] and v["in_combat"] and v["foe"] == p["id"], (p["ai_state"], v["ai_state"])
        assert v["hp"] == 80 and v["ai_state"] == "fighting"
        assert e.call("debug.vm")["counters"]["out_of_action_true"] == 0
        landed = 0
        strokes = 0
        for _ in range(40):
            cam = e.observe(entities=False)["camera"]
            px, py = _pos(_entity(e, robin_index))
            _stroke(e, px - cam[0] - 60, py - cam[1] + 40)
            strokes += 1
            p = _entity(e, robin_index)
            assert p["in_combat"], "the stroke is delivered in the fight"
            hp_before = _entity(e, vi)["hp"]
            energy_before = p["energy"]
            seen_blow = False
            for _ in range(160):
                e.step(1)
                p, v = _entity(e, robin_index), _entity(e, vi)
                if p["pose"] == "powerful_blow":
                    seen_blow = True
                    assert p["action"] == 75
                elif seen_blow:
                    break
            assert seen_blow, "the powerful blow was never delivered"
            assert p["energy"] <= energy_before, "the blow costs two units, landed or not"
            if v["hp"] < hp_before:
                assert hp_before - v["hp"] == min(50, hp_before), (hp_before, v["hp"])
                landed += 1
            if not v["alive"]:
                break
        v = _entity(e, vi)
        assert landed == 2 and not v["alive"] and v["hp"] == 0, (landed, strokes, v["hp"])
        assert v["ai_state"] in ("dying", "dead") and v["action"] in (44, 48)
        assert v["fell_backward"] is True, "struck from the front"
        e.step(5)
        vm = e.call("debug.vm")
        assert vm["counters"]["out_of_action_true"] > 0, "native 90 reports him out of action"
        assert not vm["faulted"] and vm["counters"]["traps"] == 0
        e.step(80)
        p, v = _entity(e, robin_index), _entity(e, vi)
        assert v["ai_state"] == "dead" and v["action"] == 48 and not v["in_combat"]
        assert p["alive"] and not p["in_combat"] and p["ai_state"] == "patrol"
        assert not e.observe(entities=False)["hero_dead"]
        sc = e.observe(entities=False)["script"]
        assert "powerful_blow_chance" in sc["assumptions"] and "melee_reach" in sc["assumptions"], sc["assumptions"]
        assert "knock_out" not in sc["assumptions"], "a death is measured, not a knock-out"
        # A fight in flight survives a snapshot: restore mid-fight and step the same ticks.
        e.reset({"mission": "H01_Lin_VL"}, seed=0)
        e.skip_briefing()
        robin_index, gi = _start_fight(e)
        e.step(100)
        snap = e.snapshot()
        h1 = e.step(200)["hashes"]["total"]
        e.restore(snapshot_id=snap["id"])
        h2 = e.step(200)["hashes"]["total"]
        assert h1 == h2
        assert _entity(e, robin_index)["in_combat"]
