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
  python rhcap.py seq "cmd a b; sleep 1; cmd" run several commands in one process
"""
import ctypes
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

try:
    ctypes.windll.shcore.SetProcessDpiAwareness(2)
except Exception:  # noqa: BLE001
    pass
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
pyautogui.FAILSAFE = False
pyautogui.PAUSE = 0.05

HERE = os.path.dirname(os.path.abspath(__file__))
CAPDIR = os.path.normpath(os.path.join(HERE, "..", "..", "captures", "original"))
TITLE_EXACT = "robin hood - legend of sherwood"


def find_window():
    found = []

    def cb(h, _):
        if not win32gui.IsWindowVisible(h):
            return
        t = win32gui.GetWindowText(h)
        if t and t.strip().lower() == TITLE_EXACT:
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
    """Bring the game to the foreground (needed for keyboard input). Windows only
    lets the process that produced the last input steal focus, hence the ALT tap."""
    if win32gui.GetForegroundWindow() == h:
        return True
    for attempt in range(3):
        try:
            fg = win32gui.GetForegroundWindow()
            fg_tid, _ = win32process.GetWindowThreadProcessId(fg)
            my_tid = ctypes.windll.kernel32.GetCurrentThreadId()
            attached = fg_tid != my_tid and ctypes.windll.user32.AttachThreadInput(my_tid, fg_tid, True)
            if attempt > 0:
                pyautogui.keyDown("alt")
                pyautogui.keyUp("alt")
            win32gui.SetForegroundWindow(h)
            win32gui.BringWindowToTop(h)
            if attached:
                ctypes.windll.user32.AttachThreadInput(my_tid, fg_tid, False)
        except Exception as e:  # noqa: BLE001
            print("focus failed:", e, file=sys.stderr)
        time.sleep(0.25)
        if win32gui.GetForegroundWindow() == h:
            return True
    print("WARNING: game not in foreground; keys not sent", file=sys.stderr)
    return False


# --- raw (relative) mouse input: the game reads DirectInput deltas, so SetCursorPos
# (pyautogui.moveTo) does not move the in-game cursor. We clamp the in-game cursor to
# the client's top-left corner with a huge negative delta, then move by (x, y).
class _MI(ctypes.Structure):
    _fields_ = [("dx", ctypes.c_long), ("dy", ctypes.c_long), ("mouseData", ctypes.c_ulong),
                ("dwFlags", ctypes.c_ulong), ("time", ctypes.c_ulong), ("dwExtraInfo", ctypes.c_void_p)]


class _KI(ctypes.Structure):
    _fields_ = [("wVk", ctypes.c_ushort), ("wScan", ctypes.c_ushort), ("dwFlags", ctypes.c_ulong),
                ("time", ctypes.c_ulong), ("dwExtraInfo", ctypes.c_void_p)]


class _IU(ctypes.Union):
    _fields_ = [("mi", _MI), ("ki", _KI)]


class _INPUT(ctypes.Structure):
    _fields_ = [("type", ctypes.c_ulong), ("u", _IU)]


def raw_rel(dx, dy, flags=0x0001):
    inp = _INPUT()
    inp.type = 0
    inp.u.mi = _MI(int(dx), int(dy), 0, flags, 0, None)
    n = ctypes.windll.user32.SendInput(1, ctypes.byref(inp), ctypes.sizeof(_INPUT))
    if n != 1:
        print("SendInput failed", ctypes.GetLastError(), file=sys.stderr)


MOUSE_SCALE = float(os.environ.get("RHCAP_MOUSE_SCALE", "0.775"))


def rmove(h, x, y):
    """Put the in-game cursor at client (x, y): clamp to (0, 0), then move by (x, y)."""
    sx, sy, w, hh = client_rect(h)
    for _ in range(3):
        raw_rel(-4000, -4000)
        time.sleep(0.02)
    # physical cursor is now at the screen corner; the client origin is at (sx, sy)
    tx, ty = int(x * MOUSE_SCALE), int(y * MOUSE_SCALE)
    steps = max(1, (abs(tx) + abs(ty)) // 200)
    ax = ay = 0
    for i in range(1, steps + 1):
        nx, ny = tx * i // steps, ty * i // steps
        raw_rel(nx - ax, ny - ay)
        ax, ay = nx, ny
        time.sleep(0.01)
    time.sleep(0.05)


def amove(h, x, y):
    """Absolute move (SetCursorPos) plus a 1-pixel raw jiggle so the game notices."""
    sx, sy, w, hh = client_rect(h)
    ctypes.windll.user32.SetCursorPos(sx + int(x), sy + int(y))
    time.sleep(0.02)
    raw_rel(1, 1)
    time.sleep(0.02)
    raw_rel(-1, -1)
    time.sleep(0.03)


def rclick(h, x, y, button="left", n=1):
    focus(h)
    rmove(h, x, y)
    down, up = (0x0002, 0x0004) if button == "left" else (0x0008, 0x0010)
    for _ in range(n):
        raw_rel(0, 0, down)
        time.sleep(0.06)
        raw_rel(0, 0, up)
        time.sleep(0.08)


def grab(h):
    """Capture the client area even when other windows overlap it (PrintWindow
    with PW_RENDERFULLCONTENT, which DWM supports for D3D windows)."""
    import win32ui
    from PIL import Image

    wl, wt, wr, wb = win32gui.GetWindowRect(h)
    ww, wh = wr - wl, wb - wt
    sx, sy, w, hh = client_rect(h)
    hwnd_dc = win32gui.GetWindowDC(h)
    mfc_dc = win32ui.CreateDCFromHandle(hwnd_dc)
    save_dc = mfc_dc.CreateCompatibleDC()
    bmp = win32ui.CreateBitmap()
    bmp.CreateCompatibleBitmap(mfc_dc, ww, wh)
    save_dc.SelectObject(bmp)
    ok = ctypes.windll.user32.PrintWindow(h, save_dc.GetSafeHdc(), 2)
    info = bmp.GetInfo()
    data = bmp.GetBitmapBits(True)
    img = Image.frombuffer("RGB", (info["bmWidth"], info["bmHeight"]), data, "raw", "BGRX", 0, 1)
    win32gui.DeleteObject(bmp.GetHandle())
    save_dc.DeleteDC()
    mfc_dc.DeleteDC()
    win32gui.ReleaseDC(h, hwnd_dc)
    if not ok:
        return ImageGrab.grab(bbox=(sx, sy, sx + w, sy + hh), all_screens=True)
    ox, oy = sx - wl, sy - wt
    return img.crop((ox, oy, ox + w, oy + hh))


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
    if cmd == "seq":
        # rhcap.py seq "click 10 20; sleep 1; shot name; key escape"
        for step in " ".join(args).split(";"):
            step = step.strip()
            if not step:
                continue
            parts = step.split()
            if parts[0] == "sleep":
                time.sleep(float(parts[1]))
            else:
                main(parts)
        return
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
        rmove(h, int(args[0]), int(args[1]))
        return
    if cmd == "rclick" or cmd == "click":
        btn = args[2] if len(args) > 2 else "left"
        n = int(args[3]) if len(args) > 3 else 1
        rclick(h, int(args[0]), int(args[1]), btn, n)
        return
    if cmd == "focus":
        focus(h)
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
        if not focus(h):
            return
        if len(args) > 1:
            pyautogui.keyDown(args[0])
            time.sleep(float(args[1]))
            pyautogui.keyUp(args[0])
        else:
            pyautogui.press(args[0])
        return
    if cmd == "type":
        if not focus(h):
            return
        pyautogui.typewrite(args[0], interval=0.05)
        return
    if cmd == "hover":
        focus(h)
        rmove(h, int(args[0]), int(args[1]))
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
