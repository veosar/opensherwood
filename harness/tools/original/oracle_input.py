"""Closed-loop cursor control and timestamped frame recording for the original game (analyst helper).

Generic: contains no game bytes. Complements rhcap.py.

Observed on the analyst machine (2026-09-03, 3840x2160 desktop at 200 % DPI, cnc-ddraw windowed,
1024x768 window at the screen origin, Windows pointer acceleration on): the game's own cursor is not the
OS cursor. It only follows *relative* mouse motion (DirectInput-style; SetCursorPos alone does nothing)
and its client position equals the OS cursor's physical position divided by the DPI factor (2 here),
clamped to the game screen. Pointer acceleration makes raw deltas non-linear (a raw 200 arrives as 294..784
physical pixels depending on the step size), so an open-loop scale (rhcap.MOUSE_SCALE) is unreliable.
`gmove` therefore drives the OS cursor with small raw deltas in a closed loop on GetCursorPos until the
physical position is exactly `DPI * (x, y)`; because both cursors integrate the same deltas from the same
clamped origin, the game cursor lands on (x, y).

Usage (python oracle_input.py ...):
  probe                     print the DPI factor guess and the physical / game cursor relation
  move x y                  put the in-game cursor at client (x, y)
  click x y [left|right] [n]  move then click n times
  mmove x y / mclick x y [button] [n]  in-mission cursor control with screenshot feedback
  rec <prefix> <secs> <dt>  save every frame with its timestamp (no change detection) to captures/original/
"""
import ctypes
import ctypes.wintypes
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rhcap  # noqa: E402

DPI = float(os.environ.get("RHCAP_DPI", "2"))
u = ctypes.windll.user32


def phys():
    pt = ctypes.wintypes.POINT()
    u.GetCursorPos(ctypes.byref(pt))
    return pt.x, pt.y


def clamp_origin():
    """Clamp both the OS cursor and the game cursor to the top-left corner."""
    for _ in range(3):
        rhcap.raw_rel(-8000, -8000)
        time.sleep(0.02)


def gmove(x, y, tol=0):
    """Put the in-game cursor at client (x, y) (see the module docstring)."""
    tx, ty = int(round(x * DPI)), int(round(y * DPI))
    clamp_origin()
    for _ in range(4000):
        px, py = phys()
        dx, dy = tx - px, ty - py
        if abs(dx) <= tol and abs(dy) <= tol:
            break
        # coarse while far (acceleration is harmless as long as we do not overshoot much),
        # single pixels when close (no acceleration at 1 px per event)
        sx = max(-8, min(8, dx)) if abs(dx) > 24 else max(-1, min(1, dx))
        sy = max(-8, min(8, dy)) if abs(dy) > 24 else max(-1, min(1, dy))
        rhcap.raw_rel(sx, sy)
        time.sleep(0.001)
    time.sleep(0.03)
    return phys()


def gclick(x, y, button="left", n=1, gap=0.08):
    gmove(x, y)
    down, up = (0x0002, 0x0004) if button == "left" else (0x0008, 0x0010)
    for _ in range(n):
        rhcap.raw_rel(0, 0, down)
        time.sleep(0.05)
        rhcap.raw_rel(0, 0, up)
        time.sleep(gap)


def record(prefix, secs, dt, h=None, t0=None):
    """Save every frame with its wall-clock timestamp: <prefix>_<i>_<t>.png; returns [(t, path)]."""
    if h is None:
        h, _ = rhcap.find_window()
    os.makedirs(rhcap.CAPDIR, exist_ok=True)
    t0 = t0 or time.time()
    out = []
    i = 0
    while time.time() - t0 < secs:
        t = time.time() - t0
        img = rhcap.grab(h)
        p = os.path.join(rhcap.CAPDIR, "%s_%03d_%06.2fs.png" % (prefix, i, t))
        img.save(p)
        out.append((t, p))
        i += 1
        rest = dt - (time.time() - t0 - t)
        if rest > 0:
            time.sleep(rest)
    return out


# --- in-mission cursor control with screenshot feedback ---------------------------------
# In a mission the game recentres the OS cursor (GetCursorPos returns the client centre most of
# the time) and integrates the relative deltas with its own gain, so the OS cursor is useless as
# feedback; the game cursor sprite is located in a screenshot instead (OpenCV template matching).
# Keep away from the screen edges: the cursor at an edge scrolls the camera.
# The pointer changes shape (arrow on free ground, a cross where the character cannot go, ...);
# one template per shape, cropped from earlier screenshots (captures/original/, git-ignored), with
# the hotspot of each shape inside its template. Add a shape by saving `_cursor_<name>_template.png`
# and its hotspot here.
CURSOR_TEMPLATES = {
    "arrow": ("_cursor_template.png", (2, 2)),  # tip of the arrow
    "cross": ("_cursor_cross_template.png", (16, 13)),  # centre of the cross
}
CURSOR_TEMPLATE = os.path.join(rhcap.CAPDIR, CURSOR_TEMPLATES["arrow"][0])
CURSOR_HOTSPOT = CURSOR_TEMPLATES["arrow"][1]
_TPL_CACHE = {}


def find_cursor(img, template=None):
    """Best match over all known pointer shapes: ((x, y) of the hotspot, score)."""
    import cv2
    import numpy as np

    frame = cv2.cvtColor(np.array(img), cv2.COLOR_RGB2BGR)
    if template is not None:
        shapes = [(template, CURSOR_HOTSPOT)]
    else:
        shapes = []
        for name, (fn, hs) in CURSOR_TEMPLATES.items():
            path = os.path.join(rhcap.CAPDIR, fn)
            if name not in _TPL_CACHE and os.path.exists(path):
                _TPL_CACHE[name] = cv2.imread(path)
            if name in _TPL_CACHE:
                shapes.append((_TPL_CACHE[name], hs))
    best = ((0, 0), 0.0)
    for tpl, hs in shapes:
        _, score, _, loc = cv2.minMaxLoc(cv2.matchTemplate(frame, tpl, cv2.TM_CCOEFF_NORMED))
        if score > best[1]:
            best = ((loc[0] + hs[0], loc[1] + hs[1]), float(score))
    return best


def mmove(x, y, h=None, gain=0.6, tol=2, iters=16, verbose=False):
    """Move the in-game cursor to client (x, y) using screenshot feedback; returns (pos, score, gain)."""
    if h is None:
        h, _ = rhcap.find_window()
    pos = None
    score = 0.0
    for _ in range(iters):
        time.sleep(0.12)
        img = rhcap.grab(h)
        pos, score = find_cursor(img)
        if score < 0.5:
            img.save(os.path.join(rhcap.CAPDIR, "_cursor_unknown_%d.png" % int(time.time() * 10 % 100000)))
            # the pointer changes shape over characters and HUD widgets: nudge it upwards and
            # towards the middle of the screen (raw 60 px) and look again
            if verbose:
                print("cursor not found (score %.2f), nudging" % score)
            for _ in range(20):
                rhcap.raw_rel(3 if pos[0] < 512 else -3, -3)
                time.sleep(0.002)
            continue
        dx, dy = x - pos[0], y - pos[1]
        if verbose:
            print("cursor at", pos, "score %.2f" % score, "delta", (dx, dy), "gain %.2f" % gain)
        if abs(dx) <= tol and abs(dy) <= tol:
            break
        # bounded step, sent as 1..3 px raw events (no pointer acceleration at that speed)
        sx, sy = max(-300, min(300, dx)), max(-300, min(300, dy))
        rx, ry = int(round(sx * gain)), int(round(sy * gain))
        n = max(1, max(abs(rx), abs(ry)) // 3)
        ax = ay = 0
        for i in range(1, n + 1):
            nx, ny = rx * i // n, ry * i // n
            rhcap.raw_rel(nx - ax, ny - ay)
            ax, ay = nx, ny
            time.sleep(0.002)
        time.sleep(0.1)
        npos, nscore = find_cursor(rhcap.grab(h))
        if nscore >= 0.5:
            moved = max(abs(npos[0] - pos[0]), abs(npos[1] - pos[1]))
            sent = max(abs(rx), abs(ry))
            if moved >= 8 and sent >= 8:
                gain = max(0.2, min(5.0, gain * sent / moved))
    return pos, score, gain


def mclick(x, y, button="left", n=1, gap=0.08, h=None, gain=0.6):
    pos, score, gain = mmove(x, y, h, gain)
    down, up = (0x0002, 0x0004) if button == "left" else (0x0008, 0x0010)
    for _ in range(n):
        rhcap.raw_rel(0, 0, down)
        time.sleep(0.05)
        rhcap.raw_rel(0, 0, up)
        time.sleep(gap)
    return pos, score, gain


def burst(secs, region, h=None, t0=None):
    """Grab frames as fast as possible into memory for `secs` seconds and return
    [(t, changed_pixel_count_vs_previous_frame_in_region, frame)] - to find out when the picture
    actually changes (animation frame rate) without the cost of saving every frame."""
    import numpy as np

    if h is None:
        h, _ = rhcap.find_window()
    x0, y0, x1, y1 = region
    t0 = t0 or time.time()
    out = []
    prev = None
    while time.time() - t0 < secs:
        t = time.time() - t0
        img = rhcap.grab(h)
        cur = np.asarray(img.convert("L"), dtype=np.int16)[y0:y1, x0:x1]
        n = int((np.abs(cur - prev) > 40).sum()) if prev is not None else -1
        out.append((t, n, img))
        prev = cur
    return out


def fast_burst(prefix, secs, region, save_every=4, h=None, t0=None, thresh=40):
    """Screen-DC burst with mss (~10 ms per frame; the game window must be unobstructed at the
    screen origin). Keeps only a grey crop of `region` per frame, saves every `save_every`-th
    full frame, and returns [(t, changed_vs_previous, crop)]."""
    import mss
    import numpy as np
    from PIL import Image

    if h is None:
        h, _ = rhcap.find_window()
    sx, sy, w, hh = rhcap.client_rect(h)
    x0, y0, x1, y1 = region
    t0 = t0 or time.time()
    out = []
    prev = None
    os.makedirs(rhcap.CAPDIR, exist_ok=True)
    with mss.mss() as s:
        mon = {"left": sx, "top": sy, "width": w, "height": hh}
        i = 0
        while time.time() - t0 < secs:
            t = time.time() - t0
            shot = s.grab(mon)
            img = Image.frombytes("RGB", shot.size, shot.bgra, "raw", "BGRX")
            cur = np.asarray(img.convert("L"), dtype=np.int16)[y0:y1, x0:x1]
            n = int((np.abs(cur - prev) > thresh).sum()) if prev is not None else -1
            out.append((t, n, cur))
            if i % save_every == 0:
                img.save(os.path.join(rhcap.CAPDIR, "%s_%03d_%06.2fs.png" % (prefix, i, t)))
            prev = cur
            i += 1
    return out


def track_crops(crops, thresh=40, min_pixels=30):
    """Dual-reference blob tracking on the crops of fast_burst: [(t, feet_x, feet_y, cx, cy, n)]."""
    import numpy as np

    refs = [crops[0][2], crops[-1][2]]
    rows = []
    for t, _, cur in crops:
        mask = (np.abs(cur - refs[0]) > thresh) & (np.abs(cur - refs[1]) > thresh)
        ys, xs = np.nonzero(mask)
        if len(xs) < min_pixels:
            rows.append((t, None))
            continue
        low = ys >= ys.max() - 6
        rows.append((t, (float(np.median(xs[low])), int(ys.max()), float(xs.mean()), float(ys.mean()), len(xs))))
    return rows


def main(argv):
    if not argv:
        print(__doc__)
        return
    cmd, a = argv[0], argv[1:]
    h, _ = rhcap.find_window()
    if cmd == "probe":
        rhcap.focus(h)
        clamp_origin()
        for step in (1, 5, 50):
            clamp_origin()
            for _ in range(200 // step):
                rhcap.raw_rel(step, step)
                time.sleep(0.002)
            print("raw 200 in steps of %d -> physical %s" % (step, phys()))
        return
    if cmd == "move":
        rhcap.focus(h)
        print("physical", gmove(int(a[0]), int(a[1])))
        return
    if cmd == "click":
        rhcap.focus(h)
        gclick(int(a[0]), int(a[1]), a[2] if len(a) > 2 else "left", int(a[3]) if len(a) > 3 else 1)
        return
    if cmd == "mmove":
        rhcap.focus(h)
        print(mmove(int(a[0]), int(a[1]), h, verbose=True))
        return
    if cmd == "mclick":
        rhcap.focus(h)
        print(mclick(int(a[0]), int(a[1]), a[2] if len(a) > 2 else "left", int(a[3]) if len(a) > 3 else 1, h=h))
        return
    if cmd == "rec":
        for t, p in record(a[0], float(a[1]), float(a[2]), h):
            print("%.2f %s" % (t, os.path.basename(p)))
        return
    print("unknown command", cmd)


if __name__ == "__main__":
    main(sys.argv[1:])
