"""Data-backed end-to-end checks of the map view: needs OPENSHERWOOD_GAME_DIR (a copy of the game)."""

from __future__ import annotations

from pathlib import Path

from PIL import Image

from opensherwood_harness import Engine, pointer_click

OUT = Path(__file__).resolve().parents[2] / "out" / "data"


def test_map_view_renders_background_and_sprites_deterministically(binary, game_dir):
    OUT.mkdir(parents=True, exist_ok=True)
    hashes = []
    for run in range(2):
        with Engine(binary=binary, game_dir=game_dir, artifacts=OUT) as e:
            h = e.hello()
            assert h["content_fingerprint"] and "map_view" in h["capabilities"]
            e.reset({"map_view": {"map": "sherwood", "ambiance": "Day"}}, seed=1)
            obs = e.observe()
            assert obs["map_size"] == [1920, 1088]
            player = next(x for x in obs["entities"] if x["kind"] == "player")
            assert player["anim"] is not None, "player should carry sprite animation state"
            e.step(1, pointer_click(80, 240, "left"))
            e.step(1, pointer_click(300, 300, "right"))
            e.step(90)
            cap = e.capture(f"map_view_{run}.png")
            hashes.append(cap["hash"])
    assert hashes[0] == hashes[1]
    img = Image.open(OUT / "map_view_0.png").convert("RGB")
    assert img.size == (1024, 768)
    # A retail background is never a flat colour: expect many distinct colours in the frame.
    assert len(set(img.getdata())) > 2000


def test_map_view_snapshot_restore_keeps_animation_state(binary, game_dir):
    with Engine(binary=binary, game_dir=game_dir) as e:
        e.reset({"map_view": {"map": "sherwood", "ambiance": "Day"}}, seed=4)
        e.step(1, pointer_click(80, 240, "left"))
        e.step(1, pointer_click(400, 260, "right"))
        e.step(30)
        snap = e.snapshot()
        a = e.step(40, hash_every_tick=True)["per_tick"]
        e.restore(snapshot_id=snap["id"])
        b = e.step(40, hash_every_tick=True)["per_tick"]
        assert a == b
        obs = e.observe()
        player = next(x for x in obs["entities"] if x["kind"] == "player")
        assert player["anim"]["set"] == "RobinHood"


def test_unknown_map_is_a_clean_error(binary, game_dir):
    from opensherwood_harness import EngineError
    import pytest

    with Engine(binary=binary, game_dir=game_dir) as e:
        with pytest.raises(EngineError) as ei:
            e.reset({"map_view": {"map": "atlantis", "ambiance": "Day"}})
        assert ei.value.code == -32000
