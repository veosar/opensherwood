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


def key(name: str) -> list[dict]:
    return [
        {"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": name},
        {"tick_offset": 0, "sequence": 1, "kind": "key_up", "key": name},
    ]


def test_save_and_load_screens_round_trip(binary, game_dir, tmp_path):
    """Pause -> Save (default name) -> Quit -> main menu Load -> first row -> Load restores the mission."""
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=300) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=0)
        e.skip_briefing()
        e.step(30)
        tick0 = e.observe(entities=False)["tick"]
        total0 = e.observe(entities=False)["hashes"]["total"]
        e.step(1, key("escape"))
        assert e.observe(entities=False)["ui"]["screen"] == "pause_menu"
        e.step(1, pointer_click(748, 481, "left"))  # Save (row 3: 339 + 41 * 3 = 462)
        ui = e.observe(entities=False)["ui"]
        assert ui["screen"] == "save", ui
        e.step(1, pointer_click(748, 522, "left"))  # Save button (row 4)
        assert e.observe(entities=False)["ui"]["screen"] == "pause_menu"
        assert (tmp_path / "saves" / "save-1.json").is_file()
        # Quit to the menu, then Load from the main menu.
        e.step(1, pointer_click(748, 604, "left"))
        e.step(1, pointer_click(483, 433, "left"))
        assert e.observe(entities=False)["ui"]["screen"] == "main_menu"
        e.step(1, pointer_click(748, 399, "left"))  # Load (row 1)
        ui = e.observe(entities=False)["ui"]
        assert ui["screen"] == "load", ui
        rows = [it for it in ui["items"] if it["action"].startswith("row:")]
        assert rows and rows[0]["action"] == "row:save-1"
        e.step(1, pointer_click(300, 185, "left"))  # first row
        e.step(1, pointer_click(748, 522, "left"))  # Load button
        obs = e.observe(entities=False)
        assert obs.get("ui") is None, obs.get("ui")
        assert obs["scenario"] == {"mission": "H01_Lin_VL"}
        assert obs["tick"] == tick0 and obs["hashes"]["total"] == total0

