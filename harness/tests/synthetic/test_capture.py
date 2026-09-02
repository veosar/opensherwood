"""Framebuffer capture: deterministic, PNG written under the artifact directory, path validation."""

from __future__ import annotations

from pathlib import Path

import pytest
from PIL import Image

from opensherwood_harness import Engine, EngineError, pointer_click


def test_capture_is_deterministic_across_processes(binary):
    hashes = []
    for _ in range(2):
        with Engine(binary=binary) as e:
            e.reset("corridor", seed=3)
            e.step(1, pointer_click(80, 240, "left"))
            e.step(30)
            hashes.append(e.capture()["hash"])
    assert hashes[0] == hashes[1]


def test_capture_writes_png_and_changes_with_state(engine):
    engine.reset("corridor", seed=3)
    a = engine.capture("cap_a.png")
    assert a["width"] == 640 and a["height"] == 480
    p = Path(a["path"])
    assert p.exists()
    img = Image.open(p)
    assert img.size == (640, 480)
    # Player circle is green at (80, 240)
    assert img.getpixel((80, 240))[:3] == (40, 200, 60)
    engine.step(1, pointer_click(80, 240, "left"))
    engine.step(1, pointer_click(300, 240, "right"))
    engine.step(60)
    b = engine.capture("cap_b.png")
    assert a["hash"] != b["hash"]


def test_capture_rejects_escaping_paths(engine):
    engine.reset("corridor")
    with pytest.raises(EngineError):
        engine.capture("../evil.png")
