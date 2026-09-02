"""Black-box capture helper for the original game (analyst role, Windows only).

Generic: contains no game bytes. Finds the game window, converts logical (client)
coordinates to screen coordinates, takes screenshots of the client area and sends
mouse / keyboard input only when the game window is in the foreground.

Usage (run from anywhere):
  python rhcap.py launch [gamedir]          start "Robin Hood.exe" in gamedir (default: env OPENSHERWOOD_GAME_DIR)
  python rhcap.py win                       print window title, client rect, size
  python rhcap.py shot <name>               screenshot client area -> captures/original/<name>.png
  python rhcap.py boot <prefix> <secs> <dt> screenshot every dt seconds, only save when the picture changes
  python rhcap.py move x y                  move the mouse to client coords
  python rhcap.py click x y [left|right] [n] click at client coords (n clicks)
  python rhcap.py drag x1 y1 x2 y2 [button] drag
  python rhcap.py key <name> [hold_secs]    press a key (pyautogui names) or hold it
  python rhcap.py type <text>               type text
  python rhcap.py hover x y <name>          move mouse, wait 0.6 s, screenshot
  python rhcap.py pix x y                   print client pixel colour
  python rhcap.py kill                      terminate the game process
"""
import hashlib
import os
import subprocess
import sys
import time

import pyautogui
import win32con
import win32gui
import win32process
from PIL import ImageGrab

pyautogui.FAILSAFE = False
pyautogui.PAUSE = 0.05

HERE = os.path.dirname(os.path.abspath(__file__))
CAPDIR = os.path.normpath(os.path.join(HERE, "..", "..", "captures", "original"))
TITLE_HINTS = ("robin hood", "robinhood")


def find_window():
    found = []

    def cb(h, _):
        if not win32gui.IsWindowVisible(h):
            return
        t = win32gui.GetWindowText(h)
        if t and any(k in t.lower() for k in TITLE_HINTS) and "rhcap" not in t.lower():
            found.append((h, t))

    win32gui.EnumWindows(cb, None)
    # prefer the largest client area
    best = None
    for h, t in found:
        l, tp, r, b = win32gui.GetClientRect(h)
        area = (r - l) * (b - tp)
        if best is None or area > best[0]:
            best = (area, h, t)
    return (best[1], best[2]) if best else (None, None)


def client_rect(h):
    l, t, r, b = win32gui.GetClientRect(h)
    sx, sy = win32gui.ClientToScreen(h, (0, 0))
    return sx, sy, r - l, b - t


def need_window():
    h, t = find_window()
    if h is None:
        print("game window not found", file=sys.stderr)
        sys.exit(2)
    return h, t


def to_screen(h, x, y):
    sx, sy, w, hh = client_rect(h)
    return sx + int(x), sy + int(y)


def focus(h):
    try:
        if win32gui.GetForegroundWindow() != h:
            win32gui.SetForegroundWindow(h)
            time.sleep(0.15)
    except Exception as e:  # noqa: BLE001
        print("focus failed:", e, file=sys.stderr)


def grab(h):
    sx, sy, w, hh = client_rect(h)
    return ImageGrab.grab(bbox=(sx, sy, sx + w, sy + hh), all_screens=True)


def shot(name):
    h, t = need_window()
    os.makedirs(CAPDIR, exist_ok=True)
    img = grab(h)
    p = os.path.join(CAPDIR, name + ".png")
    img.save(p)
    print(p, img.size)
    return p


def main(argv):
    if not argv:
        print(__doc__)
        return
    cmd, args = argv[0], argv[1:]
    if cmd == "launch":
        gd = args[0] if args else os.environ.get("OPENSHERWOOD_GAME_DIR")
        exe = os.path.join(gd, "Robin Hood.exe")
        p = subprocess.Popen([exe], cwd=gd, creationflags=subprocess.DETACHED_PROCESS | subprocess.CREATE_NEW_PROCESS_GROUP)
        print("pid", p.pid, "t0", time.time())
        return
    if cmd == "kill":
        subprocess.call(["taskkill", "/IM", "Robin Hood.exe", "/F"])
        return
    if cmd == "win":
        h, t = need_window()
        sx, sy, w, hh = client_rect(h)
        _, pid = win32process.GetWindowThreadProcessId(h)
        print("title=%r hwnd=%d pid=%d client_origin=(%d,%d) size=%dx%d fg=%s" % (t, h, pid, sx, sy, w, hh, win32gui.GetForegroundWindow() == h))
        return
    if cmd == "shot":
        shot(args[0])
        return
    if cmd == "boot":
        prefix, secs, dt = args[0], float(args[1]), float(args[2])
        os.makedirs(CAPDIR, exist_ok=True)
        t0 = time.time()
        last = None
        i = 0
        h = None
        while time.time() - t0 < secs:
            if h is None:
                h, _ = find_window()
                if h is None:
                    time.sleep(0.1)
                    continue
                print("%.2f window found" % (time.time() - t0))
            try:
                img = grab(h)
            except Exception:  # noqa: BLE001
                h = None
                continue
            d = hashlib.md5(img.tobytes()).hexdigest()[:8]
            if d != last:
                p = os.path.join(CAPDIR, "%s_%03d_%05.1fs.png" % (prefix, i, time.time() - t0))
                img.save(p)
                print("%.2f %s %s %s" % (time.time() - t0, d, img.size, os.path.basename(p)))
                last = d
                i += 1
            time.sleep(dt)
        return
    h, t = need_window()
    if cmd == "move":
        focus(h)
        pyautogui.moveTo(*to_screen(h, args[0], args[1]))
        return
    if cmd == "click":
        focus(h)
        btn = args[2] if len(args) > 2 else "left"
        n = int(args[3]) if len(args) > 3 else 1
        x, y = to_screen(h, args[0], args[1])
        pyautogui.moveTo(x, y)
        time.sleep(0.1)
        pyautogui.click(x, y, clicks=n, interval=0.08, button=btn)
        return
    if cmd == "drag":
        focus(h)
        btn = args[4] if len(args) > 4 else "left"
        x1, y1 = to_screen(h, args[0], args[1])
        x2, y2 = to_screen(h, args[2], args[3])
        pyautogui.moveTo(x1, y1)
        pyautogui.mouseDown(button=btn)
        pyautogui.moveTo(x2, y2, duration=0.4)
        pyautogui.mouseUp(button=btn)
        return
    if cmd == "key":
        focus(h)
        if len(args) > 1:
            pyautogui.keyDown(args[0])
            time.sleep(float(args[1]))
            pyautogui.keyUp(args[0])
        else:
            pyautogui.press(args[0])
        return
    if cmd == "type":
        focus(h)
        pyautogui.typewrite(args[0], interval=0.05)
        return
    if cmd == "hover":
        focus(h)
        pyautogui.moveTo(*to_screen(h, args[0], args[1]))
        time.sleep(0.6)
        shot(args[2])
        return
    if cmd == "pix":
        img = grab(h)
        print(img.getpixel((int(args[0]), int(args[1]))))
        return
    print("unknown command", cmd)


if __name__ == "__main__":
    main(sys.argv[1:])
