"""Quick save / quick load (F1 / F5) and rolling auto saves: modern additions over the snapshot envelope."""

from __future__ import annotations

from opensherwood_harness import Engine, pointer_click


def fkey(n: int) -> list[dict]:
    return [
        {"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": {"function": n}},
        {"tick_offset": 0, "sequence": 1, "kind": "key_up", "key": {"function": n}},
    ]


def test_quick_save_and_load_restore_the_world(binary, game_dir, tmp_path):
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=300) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=0)
        e.skip_briefing()
        obs = e.observe()
        robin = next(x for x in obs["entities"] if x["kind"] == "player")
        cam = obs["camera"]
        rx, ry = robin["x"] // 256, robin["y"] // 256
        e.step(1, fkey(1))
        assert (tmp_path / "saves" / "quick.json").is_file()
        saved = e.observe(entities=False)["hashes"]["total"]
        saved_tick = e.observe(entities=False)["tick"]
        e.step(2, pointer_click(rx - cam[0], ry - cam[1], "left"))
        e.step(2, pointer_click(rx - cam[0] - 120, ry - cam[1] + 30, "left"))
        e.step(200)
        moved = next(x for x in e.observe()["entities"] if x["kind"] == "player")
        assert abs(moved["x"] // 256 - rx) > 40
        e.step(1, fkey(5))
        after = e.observe()
        back = next(x for x in after["entities"] if x["kind"] == "player")
        assert (back["x"] // 256, back["y"] // 256) == (rx, ry)
        assert after["tick"] == saved_tick and after["hashes"]["total"] == saved


def test_auto_saves_roll_over_five_slots(binary, game_dir, tmp_path):
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=600) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=0)
        e.skip_briefing()
        e.step(3600)
        assert (tmp_path / "saves" / "auto-0.json").is_file()
        e.step(3600)
        names = sorted(p.name for p in (tmp_path / "saves").glob("auto-*.json"))
        assert names == ["auto-0.json", "auto-1.json"]
