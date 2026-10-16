"""Characterize the retail loader's error collection without retail data."""

import importlib.util
from pathlib import Path

import pytest


@pytest.fixture
def mission_checks(monkeypatch, tmp_path):
    source = Path(__file__).parents[1] / "data" / "test_mission.py"
    spec = importlib.util.spec_from_file_location("mission_checks", source)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    levels = tmp_path / "DATA" / "Levels"
    levels.mkdir(parents=True)
    for number in reversed(range(39)):
        (levels / f"mission{number:02}.rhm").touch()
    visits = []
    outcomes = {}

    class FakeEngine:
        def __init__(self, **kwargs):
            assert kwargs == {"binary": "unused", "game_dir": tmp_path, "timeout": 300}

        def __enter__(self):
            return self

        def __exit__(self, *_args):
            return False

        def reset(self, mission, *, seed):
            assert seed == 1
            self.name = mission["mission"]
            visits.append(self.name)
            outcome = outcomes.get(self.name)
            if isinstance(outcome, Exception):
                raise outcome

        def observe(self, *, entities):
            assert entities is False
            return {"map_size": [outcomes.get(self.name, 1), 1]}

    monkeypatch.setattr(module, "Engine", FakeEngine)
    return module, tmp_path, visits, outcomes


def test_loader_visits_all_missions_in_sorted_order(mission_checks):
    module, game_dir, visits, _outcomes = mission_checks
    module.test_every_mission_file_loads("unused", game_dir)
    assert visits == [f"mission{number:02}" for number in range(39)]


def test_loader_collects_invalid_sizes_and_engine_errors(mission_checks):
    module, game_dir, visits, outcomes = mission_checks
    outcomes.update(
        mission01=0, mission03=-1, mission05=RuntimeError("load failed"), mission07=AssertionError("engine assertion")
    )
    with pytest.raises(AssertionError) as caught:
        module.test_every_mission_file_loads("unused", game_dir)
    assert str(caught.value) == ("mission01: \nmission03: \nmission05: load failed\nmission07: engine assertion")
    assert len(visits) == 39
