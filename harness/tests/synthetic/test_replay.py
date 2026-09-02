"""Replay recording and playback through the RPC (ReplayV1, ADR-0004)."""

from __future__ import annotations

import json

import pytest

from opensherwood_harness import Engine, EngineError, pointer_click


def record(engine: Engine, seed: int = 11) -> dict:
    engine.reset("corridor", seed=seed)
    engine.call("replay.start", {"checkpoint_every": 25})
    engine.step(1, pointer_click(80, 240, "left"))
    engine.step(1, pointer_click(600, 240, "right"))
    engine.step(1, [{"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": "right"}])
    engine.step(200)
    return engine.call("replay.stop", {"path": "replays/corridor.jsonl"})


def test_record_then_play_matches_every_checkpoint(engine):
    rec = record(engine)
    assert rec["events"] == 7
    assert rec["checkpoints"] >= 8
    lines = [json.loads(l) for l in rec["jsonl"].splitlines() if l.strip()]
    assert lines[0]["type"] == "header"
    assert lines[0]["rng_streams"]["gameplay"]["algorithm"] == "pcg32"
    final_hash = engine.observe(entities=False)["hashes"]["total"]

    played = engine.call("replay.play", {"jsonl": rec["jsonl"]})
    assert played["first_divergence"] is None
    assert played["checkpoints_ok"] == rec["checkpoints"]
    assert played["ticks"] == 203
    assert played["hashes"]["total"] == final_hash

    played_from_file = engine.call("replay.play", {"path": "replays/corridor.jsonl"})
    assert played_from_file["hashes"]["total"] == final_hash


def test_tampered_replay_reports_first_divergence(engine):
    rec = record(engine)
    lines = rec["jsonl"].splitlines()
    # Move the right click 100 px: the actor hashes must diverge at the first checkpoint after it.
    for i, line in enumerate(lines):
        obj = json.loads(line)
        if obj["type"] == "event" and obj["kind"] == "pointer_move" and obj["tick"] == 1:
            obj["x256"] += 100 * 256
            lines[i] = json.dumps(obj)
    played = engine.call("replay.play", {"jsonl": "\n".join(lines) + "\n"})
    assert played["first_divergence"] is not None
    tick, parts = played["first_divergence"]
    assert tick == 25
    assert "orders" in parts or "actors" in parts


def test_replay_validation_rejects_bad_headers(engine):
    rec = record(engine)
    lines = rec["jsonl"].splitlines()
    header = json.loads(lines[0])
    header["ruleset"] = 999
    bad = "\n".join([json.dumps(header)] + lines[1:]) + "\n"
    with pytest.raises(EngineError) as ei:
        engine.call("replay.play", {"jsonl": bad})
    assert ei.value.code == -32602
    # events before the header are rejected too
    with pytest.raises(EngineError):
        engine.call("replay.play", {"jsonl": "\n".join(lines[1:] + [lines[0]]) + "\n"})


def test_recording_must_start_at_tick_zero(engine):
    engine.reset("corridor")
    engine.step(1)
    with pytest.raises(EngineError):
        engine.call("replay.start", {})
