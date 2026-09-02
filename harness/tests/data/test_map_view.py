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


def test_snapshot_envelope_checks_content_identity_and_catalog(binary, game_dir):
    """A retail snapshot names the content it was taken on; restore refuses another content, a
    missing fingerprint, or animation state the sprite catalog cannot resolve, and leaves the world
    untouched each time."""
    from opensherwood_harness import EngineError
    import pytest

    with Engine(binary=binary, game_dir=game_dir) as e:
        fingerprint = e.hello()["content_fingerprint"]
        e.reset({"map_view": {"map": "sherwood", "ambiance": "Day"}}, seed=4)
        e.step(20)
        snap = e.snapshot()
        assert snap["snapshot"]["content"] == fingerprint
        e.step(5)
        before = e.observe()["hashes"]

        def refused(edit, needle):
            bad = dict(snap["snapshot"])
            bad["world"] = dict(bad["world"])
            edit(bad)
            with pytest.raises(EngineError) as ei:
                e.restore(snapshot=bad)
            assert ei.value.code == -32000 and needle in ei.value.message, ei.value.message
            assert e.observe()["hashes"] == before, f"{needle}: the failed restore changed the world"

        refused(lambda b: b.__setitem__("content", "0" * 64), "different game content")
        refused(lambda b: b.__setitem__("content", None), "no content fingerprint")
        refused(lambda b: b.__setitem__("hash_schema", 999), "hash schema")

        def bad_anim(b):
            entities = [dict(x) for x in b["world"]["entities"]]
            player = next(x for x in entities if x["kind"] == "player")
            player["anim"] = dict(player["anim"], animation=1 << 19)
            b["world"]["entities"] = entities

        refused(bad_anim, "does not exist in profile")
        r = e.restore(snapshot=snap["snapshot"])
        assert r["hashes"] == snap["hashes"]
        # Across scenarios the world is rebuilt from the snapshot's content before it replaces
        # the session's; the synthetic session then shows the map view at the snapshot's tick.
        e.reset("corridor", seed=1)
        r = e.restore(snapshot=snap["snapshot"])
        assert r["tick"] == 20 and r["hashes"] == snap["hashes"]
        obs = e.observe()
        assert obs["scenario"] == {"map_view": {"map": "sherwood", "ambiance": "Day"}}
        assert obs["map_size"] == [1920, 1088]
        assert e.capture()["width"] == 1024


def test_unknown_map_is_a_clean_error(binary, game_dir):
    from opensherwood_harness import EngineError
    import pytest

    with Engine(binary=binary, game_dir=game_dir) as e:
        with pytest.raises(EngineError) as ei:
            e.reset({"map_view": {"map": "atlantis", "ambiance": "Day"}})
        assert ei.value.code == -32000
