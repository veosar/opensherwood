#!/usr/bin/env python3
"""Play the real window like a human: OS-level mouse and keyboard input against the engine window,
verified through the RPC (which, with --rpc stdio, only observes and steps; window input is queued
into the next step). This exercises the winit input mapping, letterboxing and focus handling that
the canonical-event tests bypass.

    python harness/tools/play_window.py --flow map --scenario map:sherwood --out harness/out/play
    python harness/tools/play_window.py --flow menu --out harness/out/play

`--flow map`: click the player, order a walk, hold an arrow key. `--flow menu`: start from the main menu,
click Play!, page through the briefing with Enter, click Robin, order a walk, open and close the pause menu
with Escape, quit to the menu through the confirmation seal.

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


def find_window(pid: int, timeout: float = 20.0) -> int:
    """The top-level window owned by the engine process (never another process's window)."""
    import win32gui
    import win32process

    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        found: list[int] = []

        def visit(hwnd, _):
            if win32gui.IsWindowVisible(hwnd) and win32gui.GetParent(hwnd) == 0:
                _, owner = win32process.GetWindowThreadProcessId(hwnd)
                if owner == pid:
                    found.append(hwnd)
            return True

        win32gui.EnumWindows(visit, None)
        if found:
            return found[0]
        time.sleep(0.2)
    raise RuntimeError(f"no visible window owned by pid {pid}")


def letterbox(win_w: int, win_h: int, logical: tuple[int, int]) -> tuple[float, float, float]:
    """Scale and offsets of the logical viewport inside the client area (same rule as window.rs)."""
    s = min(win_w / logical[0], win_h / logical[1])
    dw, dh = logical[0] * s, logical[1] * s
    return s, (win_w - dw) / 2, (win_h - dh) / 2


def modifiers_held() -> bool:
    """Whether Ctrl or Shift is down at the OS level (Windows)."""
    try:
        import ctypes

        u = ctypes.windll.user32
        return any(u.GetAsyncKeyState(vk) & 0x8000 for vk in (0x10, 0x11))
    except Exception:  # not Windows
        return False


class Window:
    """Screen mapping of the engine window's logical frame."""

    def __init__(self, eng: Engine):
        import win32gui

        self.eng = eng
        self.degraded: list[str] = []
        self.hwnd = find_window(eng.proc.pid)
        self.bring_to_front()
        self.refresh()

    def bring_to_front(self) -> None:
        """Give the engine window the foreground: Windows refuses SetForegroundWindow from a background
        process unless an input event precedes it (the Alt trick), and a real click on the window is the
        fallback; both are harmless on the menu."""
        import ctypes

        import pyautogui
        import win32gui

        u = ctypes.windll.user32
        for attempt in range(3):
            if win32gui.GetForegroundWindow() == self.hwnd:
                return
            try:
                u.keybd_event(0x12, 0, 0, 0)  # Alt down
                u.keybd_event(0x12, 0, 2, 0)  # Alt up
                win32gui.SetForegroundWindow(self.hwnd)
            except Exception:  # noqa: BLE001
                pass
            time.sleep(0.3)
            if win32gui.GetForegroundWindow() != self.hwnd and attempt >= 1:
                _, _, w, h = win32gui.GetClientRect(self.hwnd)
                x, y = win32gui.ClientToScreen(self.hwnd, (w // 2, h // 2))
                pyautogui.click(x, y)
                time.sleep(0.3)

    def refresh(self) -> None:
        """Re-read the window geometry, check nothing covers it, and re-read the logical frame size
        (menus are 1024x768, the synthetic corridor 640x480). Called before every action."""
        import win32gui

        _, _, self.client_w, self.client_h = win32gui.GetClientRect(self.hwnd)
        self.left, self.top = win32gui.ClientToScreen(self.hwnd, (0, 0))
        if self.client_w < 2 or self.client_h < 2:
            raise RuntimeError("engine window has no client area (minimised?)")
        # Refuse to click into another window (e.g. the original game run by the analyst).
        probe = (self.left + self.client_w // 2, self.top + self.client_h // 2)
        under = win32gui.WindowFromPoint(probe)
        if under != self.hwnd and win32gui.GetAncestor(under, 2) != self.hwnd:
            raise RuntimeError(
                f"another window covers the engine window at {probe}: "
                f"'{win32gui.GetWindowText(under)}'. Close it and retry."
            )
        if win32gui.GetForegroundWindow() != self.hwnd:
            raise RuntimeError("engine window is not in the foreground; OS input would go elsewhere")
        cap = self.eng.capture()
        self.logical = (cap["width"], cap["height"])
        self.s, self.ox, self.oy = letterbox(self.client_w, self.client_h, self.logical)

    def to_screen(self, lx: float, ly: float) -> tuple[int, int]:
        return int(self.left + self.ox + lx * self.s), int(self.top + self.oy + ly * self.s)

    def click(self, lx: float, ly: float, button: str = "left") -> None:
        import pyautogui

        self.refresh()
        pyautogui.moveTo(*self.to_screen(lx, ly), duration=0.15)
        time.sleep(0.05)
        if button == "left":
            pyautogui.click()
        else:
            pyautogui.rightClick()
        time.sleep(0.1)
        self.eng.step(2)

    def press(self, key: str) -> None:
        """Press a key through the OS. When Ctrl or Shift is physically held on this machine (seen on
        2026-09-02: Ctrl+Shift+Escape opened the Task Manager over the engine), the key goes through
        the RPC instead so the test cannot trigger system chords."""
        import pyautogui

        self.refresh()
        if modifiers_held():
            print(f"warning: Ctrl/Shift held at OS level, sending '{key}' through the RPC instead")
            self.degraded.append(key)
            self.eng.step(
                2,
                [
                    {"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": key},
                    {"tick_offset": 0, "sequence": 1, "kind": "key_up", "key": key},
                ],
            )
            return
        pyautogui.press(key)
        time.sleep(0.1)
        self.eng.step(2)


def flow_map(win: Window, eng: Engine) -> bool:
    ok = True
    # 1. Real left click on the player (map view: player at logical (80,240) with camera at 0,0).
    win.click(80, 240)
    sel = eng.observe()["selected"]
    print("selected after real click:", sel)
    ok &= sel is not None
    # 2. Real left click on the ground: order a walk; verify a target is set.
    win.click(300, 300)
    player = next(e for e in eng.observe()["entities"] if e["kind"] == "player")
    print("target after real ground click:", player["target"])
    ok &= player["target"] is not None
    # 3. Hold the right arrow for a moment: camera must scroll.
    import pyautogui

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
    return ok


def flow_menu(win: Window, eng: Engine) -> bool:
    ok = True
    ui = eng.observe(entities=False).get("ui")
    print("screen at start:", ui and ui["screen"])
    ok &= bool(ui) and ui["screen"] == "main_menu"
    eng.capture("menu_window.png")
    # Play! plate centre.
    win.click(748, 358)
    ui = eng.observe(entities=False).get("ui")
    print("after Play!:", ui and (ui["screen"], ui.get("page")))
    ok &= bool(ui) and ui["screen"] == "briefing"
    for _ in range(3):
        win.press("enter")
    obs = eng.observe()
    print("after briefing: ui =", obs.get("ui"), "tick =", obs["tick"])
    ok &= obs.get("ui") is None
    eng.capture("mission_window.png")
    # Click Robin (screen position from the observation), then order a walk to the left.
    robin = next(e for e in obs["entities"] if e["kind"] == "player")
    cam = obs["camera"]
    rx, ry = robin["x"] / 256 - cam[0], robin["y"] / 256 - cam[1]
    win.click(rx, ry)
    sel = eng.observe(entities=False)["selected"]
    print("selected Robin:", sel)
    ok &= sel is not None
    win.click(rx - 150, ry + 30)
    p = next(e for e in eng.observe()["entities"] if e["kind"] == "player")
    print("walk target:", p["target"])
    ok &= p["target"] is not None
    eng.step(60)
    # Pause menu round trip.
    win.press("escape")
    ui = eng.observe(entities=False).get("ui")
    print("after Escape:", ui and ui["screen"])
    ok &= bool(ui) and ui["screen"] == "pause_menu"
    eng.capture("pause_window.png")
    win.press("escape")
    ok &= eng.observe(entities=False).get("ui") is None
    # Quit to the main menu through the confirmation seal.
    win.press("escape")
    win.click(748, 604)
    ui = eng.observe(entities=False).get("ui")
    print("quit dialog:", ui and ui["screen"])
    ok &= bool(ui) and ui["screen"] == "dialog"
    win.click(483, 433)
    ui = eng.observe(entities=False).get("ui")
    print("after confirming quit:", ui and ui["screen"])
    ok &= bool(ui) and ui["screen"] == "main_menu"
    eng.capture("menu_again_window.png")
    return ok


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--flow", choices=["map", "menu"], default="map")
    ap.add_argument("--scenario", default=None, help="default: map:sherwood for --flow map, menu otherwise")
    ap.add_argument("--game-dir", default=os.environ.get("OPENSHERWOOD_GAME_DIR"))
    ap.add_argument("--out", default=str(HARNESS / "out" / "play"))
    args = ap.parse_args()
    scenario = args.scenario or ("map:sherwood" if args.flow == "map" else "menu")

    import pyautogui

    pyautogui.FAILSAFE = True
    out = Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    eng = Engine(
        game_dir=Path(args.game_dir) if args.game_dir else None,
        artifacts=out,
        extra_args=["--scenario", scenario, "--windowed", "--scale", "1", "--mute"],
        headless=False,
        timeout=120,
    )
    ok = True
    degraded: list[str] = []
    try:
        eng.hello()
        win = Window(eng)
        ok = flow_map(win, eng) if args.flow == "map" else flow_menu(win, eng)
        degraded = win.degraded
        eng.shutdown()
    finally:
        eng.close()
    if not ok:
        print("FAIL")
        return 1
    if degraded:
        # The mouse path was exercised, the keyboard path was not: not a full OS-level pass.
        print(f"DEGRADED: {len(degraded)} key presses went through the RPC (modifiers held at OS level)")
        return 2
    print("PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
