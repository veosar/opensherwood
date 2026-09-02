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
        # Entries that are not implemented yet keep the menu open.
        e.step(1, pointer_click(748, 405, "left"))
        assert e.observe(entities=False)["ui"]["screen"] == "main_menu"


def test_play_starts_the_first_mission_behind_the_briefing(binary, game_dir, tmp_path):
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=120) as e:
        e.reset({"menu": "main"}, seed=0)
        e.step(1, pointer_click(748, 364, "left"))
        obs = e.observe()
        ui = obs["ui"]
        assert ui["screen"] == "briefing"
        assert ui["page"] == [1, 3]
        # The mission is loaded and paused behind the parchment: Lincoln, Robin alone as player.
        assert obs["map_size"][0] > 1024
        players = [x for x in obs["entities"] if x["kind"] == "player"]
        assert len(players) == 1 and players[0]["anim"]["set"] == "RobinHood"
        tick0 = obs["tick"]
        e.capture(path="briefing_page1.png")
        e.step(5, [key("enter")])
        assert e.observe(entities=False)["ui"]["page"] == [2, 3]
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


def test_exit_needs_confirmation(binary, game_dir):
    with Engine(binary=binary, game_dir=game_dir) as e:
        e.reset({"menu": "main"}, seed=0)
        e.step(1, [key("escape")])
        assert e.observe(entities=False)["ui"]["screen"] == "dialog"
        e.step(1, [key("escape")])
        assert e.observe(entities=False)["ui"]["screen"] == "main_menu"
