"""Smoke gates: the engine boots headless, answers hello, loads the synthetic scenario and survives ticks."""

from __future__ import annotations

import pytest

from opensherwood_harness import EngineError


def test_hello_reports_protocol(engine):
    h = engine.hello()
    assert h["protocol"] == 6
    assert "synthetic" in h["capabilities"]
    assert h["ruleset"] >= 1


def test_reset_and_100_ticks(engine):
    r = engine.reset("corridor", seed=1)
    assert r["tick"] == 0
    assert len(r["hashes"]["total"]) == 64
    s = engine.step(100)
    assert s["tick"] == 100
    obs = engine.observe()
    assert obs["tick"] == 100
    kinds = {e["kind"] for e in obs["entities"]}
    assert {"player", "guard", "obstacle"} <= kinds


def test_step_before_reset_is_an_error(engine):
    with pytest.raises(EngineError) as ei:
        engine.step(1)
    assert ei.value.code == -32000


def test_unknown_method_and_bad_params(engine):
    with pytest.raises(EngineError) as ei:
        engine.call("nope")
    assert ei.value.code == -32601
    engine.reset()
    with pytest.raises(EngineError) as ei:
        engine.call("step", {"ticks": 0})
    assert ei.value.code == -32602


def test_shutdown_is_clean(engine):
    engine.hello()
    engine.shutdown()
    assert engine.proc.returncode == 0
