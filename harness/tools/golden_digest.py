#!/usr/bin/env python3
"""Cross-platform determinism digest for the synthetic corridor scenario.

Runs a fixed input script against the engine and records the per-tick total hashes plus the final
framebuffer hash. The committed fixture (harness/fixtures/synthetic_corridor.json, asset-free) must
match on every platform; a mismatch means the simulation or the renderer is platform dependent, or
the ruleset changed without regenerating the fixture on purpose.

    python harness/tools/golden_digest.py --write harness/fixtures/synthetic_corridor.json
    python harness/tools/golden_digest.py --check harness/fixtures/synthetic_corridor.json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

HARNESS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS))

from opensherwood_harness import Engine, pointer_click  # noqa: E402


def run() -> dict:
    with Engine() as e:
        hello = e.hello()
        e.reset("corridor", seed=11)
        r1 = e.step(1, pointer_click(80, 240, "left"), hash_every_tick=True)
        r2 = e.step(1, pointer_click(600, 240, "left"), hash_every_tick=True)
        r3 = e.step(1, [{"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": "right"}], hash_every_tick=True)
        r4 = e.step(397, hash_every_tick=True)
        per_tick = [h["total"] for r in (r1, r2, r3, r4) for h in r["per_tick"]]
        cap = e.capture()
        return {
            "protocol": hello["protocol"],
            "ruleset": hello["ruleset"],
            "ticks": len(per_tick),
            "every_50th_total": per_tick[::50],
            "final_total": per_tick[-1],
            "framebuffer": cap["hash"],
        }


def main() -> int:
    ap = argparse.ArgumentParser()
    g = ap.add_mutually_exclusive_group(required=True)
    g.add_argument("--write", metavar="FILE")
    g.add_argument("--check", metavar="FILE")
    args = ap.parse_args()
    digest = run()
    if args.write:
        Path(args.write).parent.mkdir(parents=True, exist_ok=True)
        with open(args.write, "w", encoding="utf-8", newline="\n") as f:
            f.write(json.dumps(digest, indent=2) + "\n")
        print(f"wrote {args.write}")
        return 0
    expected = json.loads(Path(args.check).read_text(encoding="utf-8"))
    if digest != expected:
        print("determinism digest mismatch:")
        for k in sorted(set(digest) | set(expected)):
            if digest.get(k) != expected.get(k):
                print(f"  {k}: got {digest.get(k)!r}, expected {expected.get(k)!r}")
        return 1
    print("determinism digest matches")
    return 0


if __name__ == "__main__":
    sys.exit(main())
