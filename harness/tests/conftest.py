"""Shared fixtures for the harness test suites."""

from __future__ import annotations

import os
import sys
from pathlib import Path

import pytest

HARNESS = Path(__file__).resolve().parents[1]
if str(HARNESS) not in sys.path:
    sys.path.insert(0, str(HARNESS))

from opensherwood_harness import Engine, find_binary  # noqa: E402

OUT = HARNESS / "out" / "synthetic"


@pytest.fixture(scope="session")
def binary() -> Path:
    return find_binary()


@pytest.fixture
def engine(binary: Path):
    OUT.mkdir(parents=True, exist_ok=True)
    e = Engine(binary=binary, artifacts=OUT)
    try:
        yield e
    finally:
        e.close()


@pytest.fixture(scope="session")
def game_dir() -> Path:
    p = os.environ.get("OPENSHERWOOD_GAME_DIR")
    if not p:
        pytest.skip("OPENSHERWOOD_GAME_DIR not set")
    path = Path(p)
    if not (path / "DATA").is_dir():
        pytest.skip(f"{path} has no DATA directory")
    return path
