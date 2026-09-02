"""Determinism contract (ADR-0004): same inputs -> same hashes; snapshot/restore is transparent;
the player acts only through canonical input events."""

from __future__ import annotations

from opensherwood_harness import Engine, pointer_click


def play_corridor(engine: Engine, seed: int = 11, hash_every_tick: bool = False) -> list[dict]:
    """Select the player, order it to the goal, run 300 ticks. Returns per-tick hashes (or final only)."""
    engine.reset("corridor", seed=seed)
    per_tick: list[dict] = []
    r = engine.step(1, pointer_click(80, 240, "left"), hash_every_tick=hash_every_tick)
    per_tick += r.get("per_tick", []) or [r["hashes"]]
    r = engine.step(1, pointer_click(600, 240, "right"), hash_every_tick=hash_every_tick)
    per_tick += r.get("per_tick", []) or [r["hashes"]]
    r = engine.step(398, hash_every_tick=hash_every_tick)
    per_tick += r.get("per_tick", []) or [r["hashes"]]
    return per_tick


def test_same_inputs_same_hashes_across_processes(binary):
    with Engine(binary=binary) as a, Engine(binary=binary) as b:
        ha = play_corridor(a, hash_every_tick=True)
        hb = play_corridor(b, hash_every_tick=True)
    assert len(ha) == 400
    first_diff = next((i for i, (x, y) in enumerate(zip(ha, hb)) if x != y), None)
    assert first_diff is None, f"first divergence at tick {first_diff}: {ha[first_diff]} vs {hb[first_diff]}"


def test_different_seed_changes_rng_hash_only_until_it_matters(engine):
    engine.reset("corridor", seed=1)
    h1 = engine.step(1)["hashes"]
    engine.reset("corridor", seed=2)
    h2 = engine.step(1)["hashes"]
    assert h1["rng"] != h2["rng"]
    assert h1["actors"] == h2["actors"]


def test_player_reaches_goal_through_input_only(engine):
    play_corridor(engine)
    obs = engine.observe()
    player = next(e for e in obs["entities"] if e["kind"] == "player")
    assert obs["objective_reached"], f"player at {player['x'] / 256:.1f},{player['y'] / 256:.1f}"
    assert player["target"] is None


def test_order_without_selection_does_nothing(engine):
    engine.reset("corridor", seed=5)
    before = engine.observe()
    engine.step(1, pointer_click(300, 300, "right"))
    engine.step(50)
    after = engine.observe()
    p0 = next(e for e in before["entities"] if e["kind"] == "player")
    p1 = next(e for e in after["entities"] if e["kind"] == "player")
    assert (p0["x"], p0["y"]) == (p1["x"], p1["y"])


def test_snapshot_restore_is_transparent(engine):
    engine.reset("corridor", seed=9)
    engine.step(1, pointer_click(80, 240, "left"))
    engine.step(1, pointer_click(500, 400, "right"))
    engine.step(40)
    snap = engine.snapshot()
    suffix_a = engine.step(120, hash_every_tick=True)["per_tick"]
    r = engine.restore(snapshot_id=snap["id"])
    assert r["hashes"] == snap["hashes"]
    suffix_b = engine.step(120, hash_every_tick=True)["per_tick"]
    assert suffix_a == suffix_b
    # Inline snapshot round trip through JSON.
    engine.restore(snapshot=snap["snapshot"])
    suffix_c = engine.step(120, hash_every_tick=True)["per_tick"]
    assert suffix_a == suffix_c


def test_restore_fuzz_every_10_ticks(engine):
    """Snapshot at many points; restoring and replaying the tail must reproduce the straight run."""
    engine.reset("corridor", seed=21)
    engine.step(1, pointer_click(80, 240, "left"))
    engine.step(1, pointer_click(600, 240, "right"))
    straight = engine.step(200, hash_every_tick=True)["per_tick"]
    for at in range(0, 200, 10):
        engine.reset("corridor", seed=21)
        engine.step(1, pointer_click(80, 240, "left"))
        engine.step(1, pointer_click(600, 240, "right"))
        if at:
            engine.step(at)
        snap = engine.snapshot()
        engine.step(37)
        engine.restore(snapshot_id=snap["id"])
        tail = engine.step(200 - at, hash_every_tick=True)["per_tick"]
        assert tail == straight[at:], f"divergence after restore at tick {at}"
