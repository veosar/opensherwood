"""Main menu and mission start, the way a player does it (needs OPENSHERWOOD_GAME_DIR).

Geometry from ``docs/original/ui-flow.md`` / ``campaign-flow.md``: Play! at (748,364) starts mission 1
(H01_Lin_VL, Lincoln) behind a three page briefing confirmed with Enter or the V seal.
"""

from __future__ import annotations

from opensherwood_harness import Engine, pointer_click, pointer_move


def key(name: str, kind: str = "key_down", seq: int = 0) -> dict:
    return {"tick_offset": 0, "sequence": seq, "kind": kind, "key": name}


def test_menu_renders_and_hover_changes_the_frame(binary, game_dir, tmp_path):
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path) as e:
        e.reset({"menu": "main"}, seed=0)
        obs = e.observe(entities=False)
        ui = obs["ui"]
        assert ui["screen"] == "main_menu"
        labels = [it["label"] for it in ui["items"]]
        actions = [it["action"] for it in ui["items"]]
        assert actions == ["play", "load", "select_player", "options", "show_movies", "credits", "exit"]
        # Labels are read from the player's string table, so they are not the fallback identifiers.
        assert len(set(labels)) == 7 and all(labels) and labels != actions
        assert ui["items"][0]["rect"] == [664, 339, 168, 39]
        c0 = e.capture(path="menu_idle.png")
        assert c0["width"] == 1024 and c0["height"] == 768
        e.step(1, [pointer_move(748, 364, 0, 0)])
        assert e.observe(entities=False)["ui"]["hovered"] == 0
        c1 = e.capture(path="menu_hover_play.png")
        assert c1["hash"] != c0["hash"], "hovering Play! must change the plate"
        # Load opens the load screen (empty list here); Escape returns. Entries that are not
        # implemented yet (select player, row 2) keep the menu open.
        e.step(1, pointer_click(748, 405, "left"))
        assert e.observe(entities=False)["ui"]["screen"] == "load"
        e.step(1, [key("escape")])
        assert e.observe(entities=False)["ui"]["screen"] == "main_menu"
        e.step(1, pointer_click(748, 440, "left"))
        assert e.observe(entities=False)["ui"]["screen"] == "select_player"
        e.step(1, [key("escape")])
        assert e.observe(entities=False)["ui"]["screen"] == "main_menu"


def test_play_starts_the_first_mission_behind_the_briefing(binary, game_dir, tmp_path):
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=120) as e:
        e.reset({"menu": "main"}, seed=0)
        e.step(1, pointer_click(748, 364, "left"))
        obs = e.observe()
        ui = obs["ui"]
        assert ui["screen"] == "briefing"
        assert ui["page"] == [1, 1]  # the script shows one page per sequence element
        # The mission is loaded and paused behind the parchment: Lincoln, Robin alone as player.
        assert obs["map_size"][0] > 1024
        players = [x for x in obs["entities"] if x["kind"] == "player"]
        assert len(players) == 1 and players[0]["anim"]["set"] == "RobinHood"
        tick0 = obs["tick"]
        e.capture(path="briefing_page1.png")
        e.step(5, [key("enter")])
        assert e.observe(entities=False)["ui"]["screen"] == "briefing"  # page 2
        assert e.observe(entities=False)["tick"] == tick0, "the world must stay paused"
        e.step(1, [key("enter")])
        # Last page: clicking the V seal at (508,552) starts the game.
        e.step(1, pointer_click(508, 552, "left"))
        obs = e.observe(entities=False)
        assert "ui" not in obs
        e.step(10)
        assert e.observe(entities=False)["tick"] == tick0 + 10
        e.capture(path="mission_started.png")


def test_keyboard_reaches_exit(binary, game_dir):
    with Engine(binary=binary, game_dir=game_dir) as e:
        e.reset({"menu": "main"}, seed=0)
        e.step(1, [key("up")])
        assert e.observe(entities=False)["ui"]["hovered"] == 6


def test_escape_opens_the_pause_menu_and_quit_returns_to_the_main_menu(binary, game_dir, tmp_path):
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=120) as e:
        e.reset({"menu": "main"}, seed=0)
        e.step(1, pointer_click(748, 364, "left"))
        for _ in range(3):
            e.step(1, [key("enter")])
        assert "ui" not in e.observe(entities=False)
        e.step(5)
        tick0 = e.observe(entities=False)["tick"]
        hashes0 = e.observe(entities=False)["hashes"]["total"]
        e.capture(path="mission_hud.png")
        # Escape pauses: the world stops, the pause column shows six entries starting one row down.
        e.step(1, [key("escape")])
        ui = e.observe(entities=False)["ui"]
        assert ui["screen"] == "pause_menu"
        assert [it["action"] for it in ui["items"]] == ["continue", "load", "save", "options", "restart", "quit"]
        assert ui["items"][0]["rect"][1] == 380
        e.step(20, [pointer_move(748, 400, 0, 0)])
        assert e.observe(entities=False)["tick"] == tick0
        assert e.observe(entities=False)["hashes"]["total"] == hashes0, "menus must not touch the world"
        e.capture(path="pause_menu.png")
        # Escape again continues; the world ticks.
        e.step(1, [key("escape")])
        assert "ui" not in e.observe(entities=False)
        e.step(3)
        assert e.observe(entities=False)["tick"] == tick0 + 3
        # Quit asks for confirmation; the red X cancels, the blue V leaves for the main menu.
        e.step(1, [key("escape")])
        e.step(1, pointer_click(748, 604, "left"))
        assert e.observe(entities=False)["ui"]["screen"] == "dialog"
        e.capture(path="quit_dialog.png")
        e.step(1, pointer_click(541, 433, "left"))
        assert e.observe(entities=False)["ui"]["screen"] == "pause_menu"
        e.step(1, pointer_click(748, 604, "left"))
        e.step(1, pointer_click(483, 433, "left"))
        assert e.observe(entities=False)["ui"]["screen"] == "main_menu"


def test_credits_scroll_and_escape_returns(binary, game_dir, tmp_path):
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path) as e:
        e.reset({"menu": "main"}, seed=0)
        e.step(1, pointer_click(748, 563, "left"))  # Credits row (k = 5)
        ui = e.observe(entities=False)["ui"]
        assert ui["screen"] == "credits"
        c0 = e.capture(path="credits_0.png")
        e.step(120)
        assert e.observe(entities=False)["ui"]["page"][0] == 40  # 2 s at 20 px/s
        c1 = e.capture(path="credits_2s.png")
        assert c0["hash"] != c1["hash"]
        e.step(1, [key("escape")])
        assert e.observe(entities=False)["ui"]["screen"] == "main_menu"


def test_exit_needs_confirmation(binary, game_dir):
    with Engine(binary=binary, game_dir=game_dir) as e:
        e.reset({"menu": "main"}, seed=0)
        e.step(1, [key("escape")])
        assert e.observe(entities=False)["ui"]["screen"] == "dialog"
        e.step(1, [key("escape")])
        assert e.observe(entities=False)["ui"]["screen"] == "main_menu"


def test_hud_kneel_and_stand_icons_change_the_posture(binary, game_dir):
    """Clicks on the HUD figures act like the c / s keys; clicks on other widgets do not reach the map."""
    with Engine(binary=binary, game_dir=game_dir, timeout=120) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=0)
        e.skip_briefing()
        obs = e.observe()
        robin = next(x for x in obs["entities"] if x["kind"] == "player")
        cam = obs["camera"]
        e.step(2, pointer_click(robin["x"] // 256 - cam[0], robin["y"] // 256 - cam[1], "left"))

        def posture():
            return next(x for x in e.observe()["entities"] if x["kind"] == "player")["posture"]

        assert posture() == "standing"
        e.step(2, pointer_click(20, 740, "left"))  # kneeling figure
        assert posture() == "crouched"
        e.step(2, pointer_click(20, 690, "left"))  # standing figure
        assert posture() == "standing"
        e.step(2, pointer_click(960, 60, "left"))  # map scroll: consumed, no walk order
        p = next(x for x in e.observe()["entities"] if x["kind"] == "player")
        assert p["target"] is None and e.observe(entities=False)["selected"] is not None


def test_options_sound_sliders_apply_and_persist(binary, game_dir, tmp_path):
    """Options -> Sounds: a click on a slider cell sets the value, OK applies and writes settings.json,
    Back returns to the main menu; Cancel discards."""
    import json

    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path) as e:
        e.reset({"menu": "main"}, seed=0)
        e.step(1, pointer_click(748, 481, "left"))  # Options (row 3)
        assert e.observe(entities=False)["ui"]["screen"] == "options"
        e.step(1, pointer_click(748, 522, "left"))  # Sounds (row 4)
        ui = e.observe(entities=False)["ui"]
        assert ui["screen"] == "options_sounds"
        assert any(it["action"] == "slider:2" and it["label"].endswith(" 10") for it in ui["items"])
        e.step(1, pointer_click(226 - 42 + 10, 520, "left"))  # left of the first cell: mute (0)
        ui = e.observe(entities=False)["ui"]
        assert any(it["action"] == "slider:2" and it["label"].endswith(" 0") for it in ui["items"]), ui
        e.step(1, pointer_click(226 + 42 * 4 + 10, 520, "left"))  # music slider, fifth cell
        ui = e.observe(entities=False)["ui"]
        assert any(it["action"] == "slider:2" and it["label"].endswith(" 5") for it in ui["items"]), ui
        e.step(1, pointer_click(748, 563, "left"))  # OK (row 5)
        assert e.observe(entities=False)["ui"]["screen"] == "options"
        assert json.loads((tmp_path / "settings.json").read_text())["volumes"] == [10, 10, 5, 10]
        # Cancel discards an edit.
        e.step(1, pointer_click(748, 522, "left"))
        e.step(1, pointer_click(226 + 10, 520, "left"))  # music to 1
        e.step(1, pointer_click(748, 604, "left"))  # Cancel (row 6)
        e.step(1, pointer_click(748, 522, "left"))
        ui = e.observe(entities=False)["ui"]
        assert any(it["action"] == "slider:2" and it["label"].endswith(" 5") for it in ui["items"]), ui
        e.step(1, pointer_click(748, 604, "left"))
        e.step(1, pointer_click(748, 604, "left"))  # Back (row 6)
        assert e.observe(entities=False)["ui"]["screen"] == "main_menu"


def letters(word: str) -> list[dict]:
    events = []
    for i, ch in enumerate(word):
        events.append({"tick_offset": 0, "sequence": 2 * i, "kind": "key_down", "key": {"letter": ch}})
        events.append({"tick_offset": 0, "sequence": 2 * i + 1, "kind": "key_up", "key": {"letter": ch}})
    return events


def test_select_player_new_profile_and_selection(binary, game_dir, tmp_path):
    """Select player -> New -> type a name, pick Hard, V seal -> the row appears selected; Select returns to
    the menu showing that profile; the list persists in profiles.json."""
    import json

    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path) as e:
        e.reset({"menu": "main"}, seed=0)
        e.step(1, pointer_click(748, 440, "left"))  # Select player (row 2)
        ui = e.observe(entities=False)["ui"]
        assert ui["screen"] == "select_player"
        assert [it["action"] for it in ui["items"] if it["action"].startswith("row:")] == ["row:0"]
        e.step(1, pointer_click(748, 522, "left"))  # New (row 4)
        assert e.observe(entities=False)["ui"]["screen"] == "new_player"
        e.step(1, letters("marian"))
        e.step(1, pointer_click(580, 428, "left"))  # Hard seal
        e.step(1, pointer_click(480, 542, "left"))  # V seal
        ui = e.observe(entities=False)["ui"]
        assert ui["screen"] == "select_player"
        rows = [it for it in ui["items"] if it["action"].startswith("row:")]
        assert [r["label"] for r in rows] == ["Player", "marian"]
        assert rows[1]["selected"] and all(r["enabled"] for r in rows), "the new profile is selected"
        e.step(1, pointer_click(748, 481, "left"))  # Select (row 3)
        assert e.observe(entities=False)["ui"]["screen"] == "main_menu"
        doc = json.loads((tmp_path / "profiles.json").read_text())
        assert doc["selected"] == 1 and doc["profiles"][1]["name"] == "marian"
        assert doc["profiles"][1]["difficulty"] == 2
        # The menu now shows the selected profile: its name changes the frame.
        e.capture(path="menu_marian.png")


def test_minimap_toggles_from_the_map_scroll_and_ignores_right_clicks(binary, game_dir, tmp_path):
    """HUD element 2 (combat-measurements.md 5): the map scroll toggles the mini-map overlay, a
    right click does not close it; the world keeps running underneath (an overlay, not a screen)."""
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=300) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=0)
        e.skip_briefing()
        e.step(2)
        assert e.observe(entities=False).get("ui") is None
        e.capture("closed.png")
        e.step(1, pointer_click(970, 60, "left"))
        obs = e.observe(entities=False)
        assert obs["ui"]["screen"] == "minimap", obs.get("ui")
        e.capture("open.png")
        assert (tmp_path / "closed.png").read_bytes() != (tmp_path / "open.png").read_bytes()
        # Markers over the map area (h01-measurements-2.md 5) at their computed places: the map
        # area is 204x155 at (728,112) and a map position maps proportionally (15 map px per
        # picture px on this map). The hero's oval is green at his position, an active pick-up's
        # cross has a white centre, the camera rectangle's corner is black, and a soldier far from
        # the hero is grey.
        from PIL import Image

        full = e.observe()
        mw, mh = full["map_size"]
        to_map = lambda x, y: (728 + x * 204 // mw, 112 + y * 155 // mh)
        with Image.open(tmp_path / "open.png") as im:
            rgb = im.convert("RGB")
            robin = next(x for x in full["entities"] if x["kind"] == "player")
            hx, hy = to_map(robin["x"] // 256, robin["y"] // 256)
            assert rgb.getpixel((hx, hy)) == (164, 251, 82), "the hero's oval"
            item = next(it for it in e.call("debug.vm", {})["items"] if it["active"])
            ix, iy = to_map(item["x"], item["y"])
            # Neighbouring pick-ups overlap on the map (an arrow pile lies next to a scroll), so
            # the centre may be covered by another cross's arm: both are cross colours.
            assert rgb.getpixel((ix, iy)) in ((255, 255, 255), (255, 220, 40)), "the cross"
            assert rgb.getpixel((ix - 2, iy)) in ((255, 220, 40), (255, 255, 255)), "the cross's arm"
            cam = full["camera"]
            cx, cy = to_map(cam[0], cam[1])
            assert rgb.getpixel((cx, cy)) == (0, 0, 0), "the camera rectangle"
            far = next(
                x
                for x in full["entities"]
                if x["kind"] == "guard" and x["alive"] and x["active"]
                and abs(x["x"] // 256 - robin["x"] // 256) + abs(x["y"] // 256 - robin["y"] // 256) > 1200
            )
            fx, fy = to_map(far["x"] // 256, far["y"] // 256)
            assert rgb.getpixel((fx, fy)) in ((176, 176, 176), (0, 0, 0)), "an unidentified character"
        t0 = obs["tick"]
        e.step(5)
        assert e.observe(entities=False)["tick"] == t0 + 5
        e.step(1, pointer_click(500, 400, "right"))
        assert e.observe(entities=False)["ui"]["screen"] == "minimap"
        e.step(1, pointer_click(970, 60, "left"))
        assert e.observe(entities=False).get("ui") is None
        # The `;` key toggles it too.
        e.step(1, [key("semicolon"), key("semicolon", "key_up", 1)])
        assert e.observe(entities=False)["ui"]["screen"] == "minimap"
        e.step(1, [key("semicolon"), key("semicolon", "key_up", 1)])
        assert e.observe(entities=False).get("ui") is None


def test_profiles_persist_across_sessions_and_survive_a_fresh_or_corrupt_store(binary, game_dir, tmp_path):
    """The artifact directory is created on the first write; a second session reads the list back;
    a corrupt or oversized profiles.json is ignored (default profile) instead of failing startup;
    out-of-range values are clamped."""
    import json

    fresh = tmp_path / "fresh" / "deeper"
    with Engine(binary=binary, game_dir=game_dir, artifacts=fresh) as e:
        e.reset({"menu": "main"}, seed=0)
        e.step(1, pointer_click(748, 440, "left"))  # Select player
        e.step(1, pointer_click(748, 522, "left"))  # New
        e.step(1, letters("tuck"))
        e.step(1, pointer_click(480, 542, "left"))  # V seal
        e.step(1, pointer_click(748, 481, "left"))  # Select
        assert (fresh / "profiles.json").is_file()
    with Engine(binary=binary, game_dir=game_dir, artifacts=fresh) as e:
        e.reset({"menu": "main"}, seed=0)
        e.step(1, pointer_click(748, 440, "left"))
        rows = [it for it in e.observe(entities=False)["ui"]["items"] if it["action"].startswith("row:")]
        assert [r["label"] for r in rows] == ["Player", "tuck"] and rows[1]["selected"]
    # Clamping and a bad selection index.
    (fresh / "profiles.json").write_text(json.dumps({
        "format": 1, "selected": 99,
        "profiles": [{"name": "  x" * 20, "difficulty": 9, "money": -5, "score": 1, "spared_lives": 300,
                      "progress": 200, "game_length": "y" * 100}, {"name": "   ", "difficulty": 0, "money": 0,
                      "score": 0, "spared_lives": 0, "progress": 0, "game_length": ""}]}))
    with Engine(binary=binary, game_dir=game_dir, artifacts=fresh) as e:
        e.reset({"menu": "main"}, seed=0)
        e.step(1, pointer_click(748, 440, "left"))
        rows = [it for it in e.observe(entities=False)["ui"]["items"] if it["action"].startswith("row:")]
        assert len(rows) == 1 and len(rows[0]["label"]) <= 16 and rows[0]["selected"]
    # Corrupt JSON, a missing version envelope, a string version and a future version are all ignored.
    for bad in ("{ not json", json.dumps({"selected": 0, "profiles": [{"name": "ghost"}]}),
                json.dumps({"format": "1", "selected": 0, "profiles": [{"name": "ghost"}]}),
                json.dumps({"format": 2, "selected": 0, "profiles": [{"name": "ghost"}]})):
        (fresh / "profiles.json").write_text(bad)
        with Engine(binary=binary, game_dir=game_dir, artifacts=fresh) as e:
            e.reset({"menu": "main"}, seed=0)
            e.step(1, pointer_click(748, 440, "left"))
            rows = [it for it in e.observe(entities=False)["ui"]["items"] if it["action"].startswith("row:")]
            assert [r["label"] for r in rows] == ["Player"], bad
    # Settings: the same envelope rule.
    (fresh / "settings.json").write_text(json.dumps({"aspect": 2, "effects": [False] * 4, "sound_mode": 0,
                                                     "sound_quality": 0, "volumes": [1, 1, 1, 1],
                                                     "comment_frequency": 0, "shortcut_set": 0}))
    with Engine(binary=binary, game_dir=game_dir, artifacts=fresh) as e:
        e.reset({"menu": "main"}, seed=0)
        e.step(1, pointer_click(748, 481, "left"))  # Options
        e.step(1, pointer_click(748, 522, "left"))  # Sounds
        ui = e.observe(entities=False)["ui"]
        assert any(it["action"] == "slider:2" and it["label"].endswith(" 10") for it in ui["items"]), ui


def test_rename_edits_the_row_inline(binary, game_dir, tmp_path):
    """Rename turns the selected row into an edit field (ui-flow.md 5); Enter commits, Escape cancels."""
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path) as e:
        e.reset({"menu": "main"}, seed=0)
        e.step(1, pointer_click(748, 440, "left"))  # Select player
        e.step(1, pointer_click(748, 563, "left"))  # Rename (row 5)
        ui = e.observe(entities=False)["ui"]
        assert ui["screen"] == "rename_player", ui
        assert not any(it["action"] == "yes" for it in ui["items"]), "no parchment seals for a rename"
        e.step(1, letters("s"))
        ui = e.observe(entities=False)["ui"]
        assert [it["label"] for it in ui["items"] if it["action"].startswith("row:")] == ["Players"]
        e.step(1, [{"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": "escape"}])
        ui = e.observe(entities=False)["ui"]
        assert ui["screen"] == "select_player"
        assert [it["label"] for it in ui["items"] if it["action"].startswith("row:")] == ["Player"]
        e.step(1, pointer_click(748, 563, "left"))
        e.step(1, letters("s"))
        e.step(1, [{"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": "enter"}])
        ui = e.observe(entities=False)["ui"]
        assert ui["screen"] == "select_player"
        assert [it["label"] for it in ui["items"] if it["action"].startswith("row:")] == ["Players"]


def test_reset_starting_money_overrides_the_profile(binary, game_dir, tmp_path):
    """`reset {"starting_money": n}` seeds the mission with n (replays record the value used)."""
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=300) as e:
        e.call("reset", {"scenario": {"mission": "H01_Lin_VL"}, "seed": 0, "starting_money": 777})
        rec = e.replay_start()
        e.skip_briefing()
        assert e.call("debug.vm", {})["money"] == 777
        e.step(3)
        out = e.replay_stop()
        # Restart from the pause menu keeps the session's starting money (not the profile's); a reset
        # ends a recording, so a replay never spans a restart.
        e.step(1, [key("escape"), key("escape", "key_up", 1)])
        e.step(1, pointer_click(748, 563, "left"))  # Restart (row 5)
        e.skip_briefing()
        assert e.call("debug.vm", {})["money"] == 777
        assert '"starting_money":777' in out["jsonl"].replace(" ", ""), out["jsonl"][:400]
        # Playback resets with the recorded value, whatever the profile says: the header check
        # after the reset passes and the money is the recorded one.
        e.call("replay.play", {"jsonl": out["jsonl"]})
        assert e.call("debug.vm", {})["money"] == 777
