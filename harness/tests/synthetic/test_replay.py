"""Replay recording and playback through the RPC (ReplayV1, ADR-0004).

Replay time is the session tick: one unit per `step` tick, whether a screen consumed the events or
the world stepped. The corridor has no screens, so here the session tick equals the world tick; the
mission replays in `harness/tests/data/test_script.py` cover screens in the timeline.
"""

from __future__ import annotations

import json

import pytest

from opensherwood_harness import Engine, EngineError, pointer_click


def record(engine: Engine, seed: int = 11) -> dict:
    engine.reset("corridor", seed=seed)
    engine.replay_start(checkpoint_every=25)
    engine.step(1, pointer_click(80, 240, "left"))
    engine.step(1, pointer_click(600, 240, "right"))
    engine.step(1, [{"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": "right"}])
    engine.step(200)
    return engine.replay_stop(path="replays/corridor.jsonl")


def lines_of(rec: dict) -> list[dict]:
    return [json.loads(l) for l in rec["jsonl"].splitlines() if l.strip()]


def with_edited(rec: dict, edit) -> str:
    """The replay text with `edit(obj)` applied to every line's object."""
    out = []
    for obj in lines_of(rec):
        edit(obj)
        out.append(json.dumps(obj))
    return "\n".join(out) + "\n"


def test_record_then_play_matches_every_checkpoint(engine):
    rec = record(engine)
    assert rec["events"] == 7
    # The initial state (tick 0), every 25th session tick up to 200, and the final tick 203.
    assert rec["checkpoints"] == 1 + 8 + 1
    lines = lines_of(rec)
    header = lines[0]
    assert header["type"] == "header"
    assert header["time"] == "session"
    assert header["tick_rate"] == [60, 1] and header["viewport"] == [640, 480]
    assert header["rng_streams"]["gameplay"]["algorithm"] == "pcg32"
    checkpoints = [l for l in lines if l["type"] == "checkpoint"]
    assert [c["tick"] for c in checkpoints] == [0] + list(range(25, 201, 25)) + [203]
    assert all(c["world_tick"] == c["tick"] for c in checkpoints), "no screen: the world keeps up"
    # Every checkpoint carries the session digest and the framebuffer hash of its tick.
    assert all(len(c["session"]) == 64 and len(c["frame"]) == 64 for c in checkpoints)
    assert len({c["session"] for c in checkpoints}) == 1, "no screen, no notice: one session state"
    assert len({c["frame"] for c in checkpoints}) > 1, "the actors move: the frames differ"
    assert checkpoints[-1]["frame"] == engine.capture()["hash"]
    events = [l for l in lines if l["type"] == "event"]
    assert [e["tick"] for e in events] == [0, 0, 0, 1, 1, 1, 2]
    final_hash = engine.observe(entities=False)["hashes"]["total"]

    played = engine.replay_play(jsonl=rec["jsonl"])
    assert played["first_divergence"] is None
    assert played["checkpoints_ok"] == rec["checkpoints"]
    assert played["ticks"] == 203
    assert played["hashes"]["total"] == final_hash
    assert engine.capture()["hash"] == checkpoints[-1]["frame"]

    played_from_file = engine.replay_play(path="replays/corridor.jsonl")
    assert played_from_file["hashes"]["total"] == final_hash
    assert played_from_file["checkpoints_ok"] == rec["checkpoints"]


def test_tampered_replay_reports_first_divergence(engine):
    rec = record(engine)

    # Move the right click 100 px: the actor hashes must diverge at the first checkpoint after it.
    def move_click(obj):
        if obj["type"] == "event" and obj["kind"] == "pointer_move" and obj["tick"] == 1:
            obj["x256"] += 100 * 256

    played = engine.replay_play(jsonl=with_edited(rec, move_click))
    assert played["first_divergence"] is not None
    tick, parts = played["first_divergence"]
    assert tick == 25
    assert "orders" in parts or "actors" in parts
    assert played["checkpoints_ok"] == 1, "only the initial checkpoint matched"


def test_initial_checkpoint_is_compared_before_anything_is_applied(engine):
    rec = record(engine)

    def break_initial(obj):
        if obj["type"] == "checkpoint" and obj["tick"] == 0:
            obj["hashes"]["actors"] = "00" * 32

    played = engine.replay_play(jsonl=with_edited(rec, break_initial))
    assert played["first_divergence"] == [0, ["actors"]]
    assert played["checkpoints_ok"] == 0
    assert played["ticks"] == 0, "stopped before the first advance"

    # A wrong world tick at a checkpoint is a divergence of its own.
    def lag_world(obj):
        if obj["type"] == "checkpoint" and obj["tick"] == 50:
            obj["world_tick"] = 49

    played = engine.replay_play(jsonl=with_edited(rec, lag_world))
    assert played["first_divergence"] == [50, ["world_tick"]]
    assert played["checkpoints_ok"] == 2

    # The presentation digests are compared too: a tampered session digest or frame hash at a
    # checkpoint is a divergence of that name, with nothing else differing.
    for field in ("session", "frame"):
        def tamper(obj, field=field):
            if obj["type"] == "checkpoint" and obj["tick"] == 75:
                obj[field] = "11" * 32

        played = engine.replay_play(jsonl=with_edited(rec, tamper))
        assert played["first_divergence"] == [75, [field]], field
        assert played["checkpoints_ok"] == 3


def test_replay_needs_its_initial_and_terminal_checkpoints(engine):
    """Deleting the tick-0 checkpoint, every checkpoint or the final one makes the replay invalid:
    refused by the parser before the session is reset (the world keeps its tick). The recording
    ends with an event after its last periodic checkpoint, so the final checkpoint (after that
    event) is the only terminal one: deleting it cannot leave a shorter valid replay."""
    engine.reset("corridor", seed=11)
    engine.replay_start(checkpoint_every=25)
    engine.step(1, pointer_click(80, 240, "left"))
    engine.step(50)
    engine.step(1, [{"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": "right"}])
    rec = engine.replay_stop()
    lines = lines_of(rec)
    assert [l["tick"] for l in lines if l["type"] == "checkpoint"] == [0, 25, 50, 52]
    last_tick = 52
    engine.step(7)

    def without(pred) -> str:
        return "\n".join(json.dumps(l) for l in lines if not pred(l)) + "\n"

    cases = {
        "not 0": without(lambda l: l["type"] == "checkpoint" and l["tick"] == 0),
        "no checkpoint": without(lambda l: l["type"] == "checkpoint"),
        "terminal": without(lambda l: l["type"] == "checkpoint" and l["tick"] == last_tick),
    }
    for needle, text in cases.items():
        with pytest.raises(EngineError) as ei:
            engine.replay_play(jsonl=text)
        assert ei.value.code == -32602, needle
        assert needle in ei.value.message, (needle, ei.value.message)
        assert engine.observe(entities=False)["tick"] == 59, needle


def test_replay_validation_rejects_bad_headers(engine):
    rec = record(engine)
    lines = rec["jsonl"].splitlines()
    header = json.loads(lines[0])
    header["ruleset"] = 999
    bad = "\n".join([json.dumps(header)] + lines[1:]) + "\n"
    with pytest.raises(EngineError) as ei:
        engine.replay_play(jsonl=bad)
    assert ei.value.code == -32602
    # Only the session time model exists.
    header = json.loads(lines[0])
    header["time"] = "world"
    with pytest.raises(EngineError) as ei:
        engine.replay_play(jsonl="\n".join([json.dumps(header)] + lines[1:]) + "\n")
    assert ei.value.code == -32602
    # events before the header are rejected too
    with pytest.raises(EngineError):
        engine.replay_play(jsonl="\n".join(lines[1:] + [lines[0]]) + "\n")


def test_header_must_match_the_session_after_reset(engine):
    """A well-formed header the session would not produce (another viewport here) is refused
    after the reset, naming the field: playback never runs against different parameters."""
    rec = record(engine)
    lines = rec["jsonl"].splitlines()
    header = json.loads(lines[0])
    header["viewport"] = [800, 600]
    with pytest.raises(EngineError) as ei:
        engine.replay_play(jsonl="\n".join([json.dumps(header)] + lines[1:]) + "\n")
    assert ei.value.code == -32000
    assert "viewport" in ei.value.message
    # The session was reset to the replay's scenario before the mismatch was found.
    assert engine.observe(entities=False)["tick"] == 0


def test_recording_must_start_at_tick_zero(engine):
    engine.reset("corridor")
    engine.step(1)
    with pytest.raises(EngineError):
        engine.replay_start()


def test_restore_is_refused_while_recording(engine):
    engine.reset("corridor", seed=3)
    engine.replay_start()
    engine.step(5)
    snap = engine.snapshot()
    engine.step(5)
    with pytest.raises(EngineError) as ei:
        engine.restore(snapshot_id=snap["id"])
    assert "replay.stop" in ei.value.message
    assert engine.observe(entities=False)["tick"] == 10
    engine.replay_stop()
    r = engine.restore(snapshot_id=snap["id"])
    assert r["tick"] == 5
