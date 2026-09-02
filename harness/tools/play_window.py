#!/usr/bin/env python3
"""Play the real window like a human: OS-level mouse and keyboard input against the engine window,
verified through the RPC (which, with --rpc stdio, only observes and steps; window input is queued
into the next step). This exercises the winit input mapping, letterboxing and focus handling that
the canonical-event tests bypass.

    python harness/tools/play_window.py --scenario map:sherwood --out harness/out/play

Requires pyautogui, pygetwindow and pywin32 (Windows only for now). Moves the real mouse: do not run while
a person or another automation (the original-game analyst) uses the machine; the tool refuses to click if
another window covers the engine window.
"""

from __future__ import annotations

import argparse
import os
import sys
import time
from pathlib import Path

HARNESS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS))

from opensherwood_harness import Engine  # noqa: E402

LOGICAL = (640, 480)


def find_window(title: str, timeout: float = 20.0):
    import pygetwindow as gw

    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        wins = [w for w in gw.getWindowsWithTitle(title) if w.title == title]
        if wins:
            return wins[0]
        time.sleep(0.2)
    raise RuntimeError(f"window '{title}' not found")


def letterbox(win_w: int, win_h: int) -> tuple[float, float, float]:
    """Scale and offsets of the logical viewport inside the client area (same rule as window.rs)."""
    s = min(win_w / LOGICAL[0], win_h / LOGICAL[1])
    dw, dh = LOGICAL[0] * s, LOGICAL[1] * s
    return s, (win_w - dw) / 2, (win_h - dh) / 2


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--scenario", default="map:sherwood")
    ap.add_argument("--game-dir", default=os.environ.get("OPENSHERWOOD_GAME_DIR"))
    ap.add_argument("--out", default=str(HARNESS / "out" / "play"))
    args = ap.parse_args()

    import pyautogui

    pyautogui.FAILSAFE = True
    out = Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    eng = Engine(
        game_dir=Path(args.game_dir) if args.game_dir else None,
        artifacts=out,
        extra_args=["--scenario", args.scenario, "--windowed", "--scale", "1", "--mute"],
        headless=False,
    )
    ok = True
    try:
        eng.hello()
        win = find_window("OpenSherwood")
        try:
            win.activate()
        except Exception:  # pygetwindow raises a spurious error 0 on Windows
            pass
        time.sleep(0.5)
        # Exact client rectangle in physical screen pixels (DPI scaling makes it larger than 640x480).
        import win32gui

        hwnd = win._hWnd
        _, _, client_w, client_h = win32gui.GetClientRect(hwnd)
        left, top = win32gui.ClientToScreen(hwnd, (0, 0))
        s, ox, oy = letterbox(client_w, client_h)
        # Refuse to click into another window (e.g. the original game run by the analyst).
        probe = (left + client_w // 2, top + client_h // 2)
        under = win32gui.WindowFromPoint(probe)
        if under != hwnd and win32gui.GetAncestor(under, 2) != hwnd:
            raise RuntimeError(
                f"another window covers the engine window at {probe}: "
                f"'{win32gui.GetWindowText(under)}'. Close it and retry."
            )

        def to_screen(lx: float, ly: float) -> tuple[int, int]:
            return int(left + ox + lx * s), int(top + oy + ly * s)

        # 1. Real left click on the player (map view: player at logical (80,240) with camera at 0,0).
        pyautogui.moveTo(*to_screen(80, 240), duration=0.2)
        pyautogui.click()
        time.sleep(0.1)
        eng.step(2)
        obs = eng.observe()
        sel = obs["selected"]
        print("selected after real click:", sel)
        ok &= sel is not None

        # 2. Real right click: order a move; verify a target is set.
        pyautogui.moveTo(*to_screen(300, 300), duration=0.2)
        pyautogui.rightClick()
        time.sleep(0.1)
        eng.step(2)
        player = next(e for e in eng.observe()["entities"] if e["kind"] == "player")
        print("target after real right click:", player["target"])
        ok &= player["target"] is not None

        # 3. Hold the right arrow for a moment: camera must scroll.
        pyautogui.keyDown("right")
        time.sleep(0.05)
        eng.step(20)
        pyautogui.keyUp("right")
        time.sleep(0.05)
        eng.step(1)
        cam = eng.observe()["camera"]
        print("camera after real key:", cam)
        ok &= cam[0] > 0
        eng.capture("play_window.png")
        eng.shutdown()
    finally:
        eng.close()
    print("PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
