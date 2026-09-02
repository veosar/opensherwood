#!/usr/bin/env python3
"""Drive a running engine (headless or windowed) with a short scripted session and capture PNGs.

Example:
    python harness/tools/drive.py --scenario map:sherwood --window --out harness/out/drive
    python harness/tools/drive.py --scenario corridor --out harness/out/drive

Writes capture PNGs under --out and prints the observation summary. Used by agents to look at what
the engine shows after a sequence of canonical inputs (see docs/harness.md).
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

HARNESS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS))

from opensherwood_harness import Engine, pointer_click, pointer_move  # noqa: E402


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--scenario", default="corridor")
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--window", action="store_true", help="drive the real window instead of headless")
    ap.add_argument("--game-dir", default=os.environ.get("OPENSHERWOOD_GAME_DIR"))
    ap.add_argument("--out", default=str(HARNESS / "out" / "drive"))
    ap.add_argument("--ticks", type=int, default=120)
    args = ap.parse_args()

    out = Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    extra = ["--scenario", args.scenario] if args.window else []
    eng = Engine(
        game_dir=Path(args.game_dir) if args.game_dir else None,
        artifacts=out,
        extra_args=extra,
        headless=not args.window,
    )
    try:
        print(json.dumps(eng.hello()))
        scenario: dict = {"synthetic": args.scenario}
        if args.scenario.startswith("map:"):
            parts = args.scenario.split(":")
            scenario = {"map_view": {"map": parts[1], "ambiance": parts[2] if len(parts) > 2 else "Day"}}
        r = eng.reset(scenario, seed=args.seed)
        print("reset tick", r["tick"])
        eng.capture("00_start.png")
        eng.step(1, pointer_click(80, 240, "left"))
        eng.step(1, pointer_click(400, 300, "left"))
        eng.step(args.ticks // 2)
        eng.capture("01_moving.png")
        # scroll right with the keyboard for a while, then release
        eng.step(1, [{"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": "right"}])
        eng.step(30)
        eng.step(1, [{"tick_offset": 0, "sequence": 0, "kind": "key_up", "key": "right"}])
        eng.step(args.ticks // 2, [pointer_move(320, 240)])
        c = eng.capture("02_scrolled.png")
        obs = eng.observe()
        player = next(e for e in obs["entities"] if e["kind"] == "player")
        print(
            f"tick={obs['tick']} camera={obs['camera']} map={obs['map_size']} "
            f"player=({player['x'] / 256:.1f},{player['y'] / 256:.1f}) selected={obs['selected']} "
            f"capture={c['path']}"
        )
        eng.shutdown()
    finally:
        eng.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
