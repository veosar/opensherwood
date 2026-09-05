"""The first mission won and lost through play with canonical input, and the flows that follow.

Win (`docs/original/h01-win-path.md` 4.1): the tutorial's victory test is one mission variable, set by
the reading of one scroll, ordered by a click on the scroll itself (ruleset 17: a scroll is read by an
order, never by walking past it); the walk to it crosses no zone with a handler. The outcome is tainted
(the door stubs and the scroll's fate after its reading are hypotheses, ADR-0008), so the tests assert the
taint as well as the win; the run is recorded and played back as a `ReplayV1` (checkpoints equal, no
divergence).
Loss (`docs/original/combat-measurements.md` 1.5 / 4): the halberdier's blows kill the hero in about
three minutes and the lost page follows; its seals lead to the briefing (restart) or the menu (OK).
"""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path

from opensherwood_harness import Engine, pointer_click

_MISSION = Path(__file__).with_name("test_mission.py")


def _mission_helpers():
    spec = importlib.util.spec_from_file_location("test_mission_helpers", _MISSION)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def hold(key: str, ticks: int) -> list[dict]:
    return [
        {"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": key},
        {"tick_offset": ticks - 1, "sequence": 0, "kind": "key_up", "key": key},
    ]


def key(name: str) -> list[dict]:
    return [
        {"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": name},
        {"tick_offset": 0, "sequence": 1, "kind": "key_up", "key": name},
    ]


def _walk_to_the_sons_scroll(e: Engine) -> None:
    e.skip_briefing()
    e.step(1, pointer_click(512, 384, "left"))  # select Robin
    e.step(150, hold("left", 150))  # camera to the west edge
    e.step(93, hold("up", 93))  # and north
    # A click on the son's scroll (element 114, its position read from the file): 3 px right of and
    # 2 px above its base, inside the sprite's hit area (a scroll is read by an order on it, ruleset 17).
    scroll = e.call("debug.vm", {"element": 114})["element"]
    assert scroll["kind"] == "scroll", scroll
    cam = e.observe(entities=False)["camera"]
    e.step(1, pointer_click(scroll["x"] - cam[0] + 3, scroll["y"] - cam[1] - 2, "left"))


def _wait_for_screen(e: Engine, screen: str, chunks: int = 12, ticks: int = 300) -> dict:
    for _ in range(chunks):
        e.step(ticks)
        obs = e.observe(entities=False)
        if obs.get("ui") and obs["ui"]["screen"] == screen:
            return obs
    raise AssertionError(f"no {screen} within {chunks * ticks} ticks")


def test_first_mission_is_won_by_walking_to_the_sons_scroll_and_the_replay_agrees(binary, game_dir, tmp_path):
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=600) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=1)
        e.replay_start(checkpoint_every=600)
        _walk_to_the_sons_scroll(e)
        _wait_for_screen(e, "debriefing")
        vm = e.call("debug.vm", {})
        assert vm["mission_won"] and not vm["mission_lost"]
        assert vm["tainted"] and any("scroll" in str(a) for a in vm["assumptions"]), vm["assumptions"]
        total = e.observe(entities=False)["hashes"]["total"]
        out = e.replay_stop(path="win.jsonl")
        # The recorded run plays back to the same state: every checkpoint equal, no divergence.
        played = e.call("replay.play", {"path": "win.jsonl"})
        assert played.get("first_divergence") is None, played
        assert e.observe(entities=False)["hashes"]["total"] == total
        assert e.observe(entities=False)["ui"]["screen"] == "debriefing"


def test_the_won_flow_launches_the_campaign_successor_with_the_money(binary, game_dir, tmp_path):
    """After the won page the next level of the campaign graph starts behind its briefing (manual,
    p. 9); the successor is tainted from tick 0 by the graph hypothesis and seeded with the money
    the won mission ended with, which the profile now holds."""
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=600) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=1)
        _walk_to_the_sons_scroll(e)
        _wait_for_screen(e, "debriefing")
        money_at_win = e.call("debug.vm", {})["money"]
        e.step(1, key("enter"))
        obs = e.observe(entities=False)
        assert obs["scenario"]["mission"].lower() != "h01_lin_vl", obs["scenario"]
        assert obs.get("ui") is None or obs["ui"]["screen"] == "briefing"
        vm = e.call("debug.vm", {})
        assert vm["present"] and not vm["faulted"] and vm["counters"]["traps"] == 0
        assert vm["tainted"] and any("ampaign" in str(a) for a in vm["assumptions"]), vm["assumptions"]
        assert vm["money"] == money_at_win
        doc = json.loads((tmp_path / "profiles.json").read_text())
        assert doc["profiles"][doc["selected"]]["money"] == money_at_win


def test_the_lost_page_after_the_heros_death_offers_restart_and_ok(binary, game_dir, tmp_path):
    tm = _mission_helpers()
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path, timeout=900) as e:
        e.reset({"mission": "H01_Lin_VL"}, seed=0)
        e.skip_briefing()
        robin_index, _ = tm._start_fight(e)
        for _ in range(600):
            e.step(30)
            obs = e.observe(entities=False)
            if obs.get("ui"):
                break
        else:
            raise AssertionError("Robin survived 300 s")
        assert obs["hero_dead"] is True
        ui = obs["ui"]
        assert ui["screen"] == "lost", ui
        assert [it["action"] for it in ui["items"]] == ["restart", "load", "ok"]
        tick = obs["tick"]
        e.step(3)
        assert e.observe(entities=False)["tick"] == tick, "the world is paused under the page"
        e.step(1, pointer_click(333, 556, "left"))  # restart seal
        assert e.observe(entities=False)["ui"]["screen"] == "briefing"
        e.skip_briefing()
        robin = e.observe()["entities"][robin_index]
        assert robin["hp"] == 100 and robin["alive"]
        # The same death again, then OK to the main menu.
        tm._start_fight(e)
        for _ in range(600):
            e.step(30)
            if e.observe(entities=False).get("ui"):
                break
        assert e.observe(entities=False)["ui"]["screen"] == "lost"
        e.step(1, pointer_click(517, 547, "left"))  # OK seal
        assert e.observe(entities=False)["ui"]["screen"] == "main_menu"
