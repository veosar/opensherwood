"""Oracle comparison: the engine's main menu against a local capture of the original's main menu.

The capture (`harness/captures/original/menu_main.png`, git-ignored, produced by the analyst with
`harness/tools/original/rhcap.py` from the player's own copy) is never committed; the test skips when it is
absent. Masked: the profile summary (name and values differ per profile), the button column (the original's
capture has the cursor over it and a different hover state), the cursor.
"""

from __future__ import annotations

from pathlib import Path

import pytest

from opensherwood_harness import Engine
from opensherwood_harness.compare import compare

HARNESS = Path(__file__).resolve().parents[2]
ORIGINAL = HARNESS / "captures" / "original" / "menu_main.png"

MASKS = [
    (280, 230, 310, 170),  # profile summary text
    (660, 330, 180, 300),  # button column (hover state / cursor in the original capture)
    (0, 0, 1024, 128),  # letterbox band (identical, but keeps the metric about the picture)
]


def test_main_menu_matches_the_original_outside_the_masked_regions(binary, game_dir, tmp_path):
    if not ORIGINAL.is_file():
        pytest.skip("no local capture of the original main menu")
    with Engine(binary=binary, game_dir=game_dir, artifacts=tmp_path) as e:
        e.reset({"menu": "main"}, seed=0)
        # Park the pointer where the original's cursor is not, inside a masked region.
        e.step(1, [{"tick_offset": 0, "sequence": 0, "kind": "pointer_move", "x256": 700 * 256, "y256": 600 * 256}])
        e.capture(path="menu_for_oracle.png")
    result = compare(tmp_path / "menu_for_oracle.png", ORIGINAL, MASKS, diff_out=tmp_path / "menu_diff.png")
    print("menu vs original:", result)
    assert result.ssim > 0.97, str(result)
    assert result.fraction_over_32 < 0.01, str(result)
