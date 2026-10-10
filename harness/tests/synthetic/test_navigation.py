"""Exercise the map navigation pipeline through canonical player input, without game assets.

The navigation scenario uses spec-navigation F1 and a second floor joined by a passage.
Snapshots and debug observations only inspect state; every order is a mouse event.
"""

import copy

import pytest

from opensherwood_harness import Engine, EngineError, pointer_click


def player(engine):
    return engine.observe()["entities"][0]


def start(engine, destination=(300, 100)):
    engine.reset("navigation", seed=17)
    engine.step(1, pointer_click(100, 100))
    engine.step(1, pointer_click(*destination))


def test_click_queues_a_path_and_walks_around_the_obstacle(engine):
    start(engine)
    queued = engine.snapshot()["snapshot"]["world"]["nav_state"]
    assert len(queued["queue"]) == 1
    assert (player(engine)["x"], player(engine)["y"]) == (100 * 256, 100 * 256)
    engine.step(1)
    dispatched = engine.snapshot()["snapshot"]["world"]["nav_state"]
    assert dispatched["completed"] is not None and not dispatched["queue"]
    positions = []
    for _ in range(110):
        engine.step(1)
        p = player(engine)
        positions.append((p["x"] / 256, p["y"] / 256))
        assert not (180 <= p["x"] / 256 <= 220 and 80 <= p["y"] / 256 <= 120)
    assert min(y for _, y in positions) <= 76
    assert positions[-1] == (300, 100)
    assert player(engine)["walk"] is None


def test_navigation_diagnostics_report_the_active_backend_without_mutation(engine):
    start(engine)
    before = engine.snapshot()
    nav = engine.call("debug.nav", {"x": 100, "y": 100, "to": [200, 100]})
    assert nav["backend"] == "layered"
    assert nav["navigation"]["point"]["valid"]
    assert not nav["navigation"]["destination"]["valid"]
    assert nav["navigation"]["queue"] == before["snapshot"]["world"]["nav_state"]["queue"]
    assert engine.snapshot()["hashes"] == before["hashes"]


@pytest.mark.parametrize("delay", [0, 1, 2, 20, 75])
def test_inline_restore_preserves_queued_completed_and_moving_walks(engine, delay):
    start(engine)
    if delay:
        engine.step(delay)
    snap = engine.snapshot()
    suffix = engine.step(130, hash_every_tick=True)["per_tick"]
    restored = engine.restore(snapshot=snap["snapshot"])
    assert restored["hashes"] == snap["hashes"]
    assert engine.step(130, hash_every_tick=True)["per_tick"] == suffix
    assert player(engine)["x"] == 300 * 256


def test_right_click_cancels_a_pending_request(engine):
    start(engine)
    engine.step(1, pointer_click(100, 100, "right"))
    engine.step(120)
    p = player(engine)
    assert p["walk"] is None and p["target"] is None
    assert (p["x"], p["y"]) == (100 * 256, 100 * 256)
    nav = engine.snapshot()["snapshot"]["world"]["nav_state"]
    assert not nav["queue"] and nav["completed"] is None and not nav["failed"]


def test_passage_changes_layer_at_the_crossing_and_replays(engine, binary):
    engine.reset("navigation", seed=17)
    engine.replay_start(checkpoint_every=10)
    engine.step(1, pointer_click(100, 100))
    engine.step(1, pointer_click(560, 100))
    seen_crossing = False
    for _ in range(220):
        engine.step(1)
        p = player(engine)
        seen_crossing |= p["crossing"] is not None
        if p["layer"] == 1:
            assert p["x"] >= 410 * 256
    assert seen_crossing
    assert (p["x"], p["y"], p["layer"], p["sector"]) == (560 * 256, 100 * 256, 1, 2)
    rec = engine.replay_stop()
    with Engine(binary=binary) as fresh:
        played = fresh.replay_play(jsonl=rec["jsonl"])
        assert played["first_divergence"] is None
        assert played["checkpoints_ok"] == rec["checkpoints"]


def test_click_inside_obstacle_cannot_start_a_walk(engine):
    start(engine, (200, 100))
    engine.step(40)
    p = player(engine)
    assert p["walk"] is None and p["target"] is None
    assert (p["x"], p["y"]) == (100 * 256, 100 * 256)


@pytest.mark.parametrize("path,value", [
    (("nav_state", "queue", 0, "owner"), 9999),
    (("nav_state", "queue", 0, "id"), 0),
    (("nav_state", "queue", 0, "submitted"), 9999),
    (("nav_state", "queue", 0, "goal"), [2147483647, 0]),
    (("nav_state", "queue", 0, "state"), {"FailedPending": {"arrival": 2}}),
    (("nav_state", "drive_cursor"), 9999),
    (("nav_state", "search_budget"), 1000001),
    (("nav_state", "avail", "nodes", 0, 0, "available"), [0, 0, 0, 0]),
    (("entities", 0, "walk", "gait"), 255),
    (("entities", 0, "walk", "cursor"), 9999),
    (("entities", 0, "walk", "submitted"), False),
    (("entities", 0, "walk", "steps", 0, "Walk", "sector"), 9999),
])
def test_invalid_navigation_restore_is_transactional(engine, path, value):
    start(engine)
    snapshot = engine.snapshot()
    bad = copy.deepcopy(snapshot["snapshot"])
    field = bad["world"]
    for key in path[:-1]:
        field = field[key]
    field[path[-1]] = value
    with pytest.raises(EngineError):
        engine.restore(snapshot=bad)
    assert engine.snapshot()["hashes"] == snapshot["hashes"]
    expected = engine.step(130, hash_every_tick=True)["per_tick"]
    engine.restore(snapshot=snapshot["snapshot"])
    assert engine.step(130, hash_every_tick=True)["per_tick"] == expected


def test_each_crossing_phase_survives_inline_restore(engine):
    start(engine, (560, 100))
    snapshots = {}
    for _ in range(210):
        engine.step(1)
        crossing = player(engine)["crossing"]
        if crossing is not None:
            snapshots.setdefault(crossing[2], engine.snapshot())
    assert set(snapshots) == {False, True}
    final_hashes = engine.snapshot()["hashes"]
    final_tick = engine.observe()["tick"]
    for snapshot in snapshots.values():
        engine.restore(snapshot=snapshot["snapshot"])
        engine.step(final_tick - engine.observe()["tick"])
        assert engine.snapshot()["hashes"] == final_hashes
