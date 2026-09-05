"""Timestamped frame recorder and static-camera motion tracker for the original game (analyst helper).

Generic: contains no game bytes. Records the game window's client area with mss (screen DC, ~10 ms per
frame; the window must be unobstructed) as 8-bit grey frames with wall-clock timestamps into one .npz,
then tracks moving sprites against the per-pixel median background of the recording (a static camera
makes the median the empty scene; anything that moves shows up as a blob).

Usage (python frame_rec.py ...):
  rec <out.npz> <secs> [fps [x0 y0 x1 y1]]       record (default fps 30; 0 = as fast as possible; optional crop)
  blobs <in.npz> [x0 y0 x1 y1] [thresh] [minpx]  per frame: every blob's bbox, feet (bottom-centre), pixels
  track <in.npz> x0 y0 x1 y1 [thresh]            one blob per frame inside the region: t, feet x, feet y
  changes <in.npz> x0 y0 x1 y1 [thresh]          per frame: changed pixels vs the previous frame in the
                                                 region (animation frame boundaries)
  save <in.npz> <index> <out.png>                write one frame as an image (inspection only)

Positions are client pixels at the game's resolution (1024x768 with the maintainer's profile).
"""
import os
import sys
import time

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rhcap  # noqa: E402


def record(path, secs, fps=30.0, h=None, t0=None, on_frame=None, region=None):
    """Grab grey frames for `secs` seconds; returns (times, frames) and writes the npz.
    `region` = (x0, y0, x1, y1) keeps only that crop (positions in the npz are then relative to it)."""
    import mss
    from PIL import Image

    if h is None:
        h, _ = rhcap.find_window()
    sx, sy, w, hh = rhcap.client_rect(h)
    dt = 1.0 / fps if fps else 0.0
    times, frames = [], []
    t0 = t0 or time.time()
    with mss.mss() as s:
        mon = {"left": sx, "top": sy, "width": w, "height": hh}
        nxt = 0.0
        while True:
            t = time.time() - t0
            if t >= secs:
                break
            if t < nxt:
                time.sleep(min(0.002, nxt - t))
                continue
            nxt = t + dt
            shot = s.grab(mon)
            img = Image.frombytes("RGB", shot.size, shot.bgra, "raw", "BGRX")
            g = np.asarray(img.convert("L"), dtype=np.uint8)
            if region:
                g = g[region[1]:region[3], region[0]:region[2]]
            frames.append(g)
            times.append(time.time() - t0)
            if on_frame:
                on_frame(times[-1], frames[-1])
    times = np.array(times, dtype=np.float64)
    frames = np.stack(frames)
    np.savez_compressed(path, t=times, frames=frames)
    return times, frames


def load(path):
    d = np.load(path)
    return d["t"], d["frames"]


def background(frames):
    return np.median(frames.astype(np.int16), axis=0)


def blobs(frames, times, region=None, thresh=40, minpx=40, bg=None):
    """[(t, [(x0, y0, x1, y1, feet_x, feet_y, n), ...])] with connected components of |frame - median|."""
    import cv2

    if bg is None:
        bg = background(frames)
    x0, y0, x1, y1 = region or (0, 0, frames.shape[2], frames.shape[1])
    out = []
    for t, fr in zip(times, frames):
        d = (np.abs(fr.astype(np.int16) - bg) > thresh).astype(np.uint8)
        d = d[y0:y1, x0:x1]
        # close small gaps so a sprite is one blob
        d = cv2.morphologyEx(d, cv2.MORPH_CLOSE, np.ones((5, 5), np.uint8))
        n, lab, stats, _ = cv2.connectedComponentsWithStats(d, connectivity=8)
        bl = []
        for i in range(1, n):
            bx, by, bw, bh, area = stats[i]
            if area < minpx:
                continue
            ys, xs = np.nonzero(lab == i)
            low = ys >= ys.max() - 4
            bl.append((bx + x0, by + y0, bx + bw + x0, by + bh + y0, float(np.median(xs[low])) + x0, int(ys.max()) + y0, int(area)))
        out.append((t, bl))
    return out


def track(frames, times, region, thresh=40, minpx=40, bg=None):
    """Largest blob per frame inside the region: [(t, feet_x, feet_y, x0, y0, x1, y1, n) or (t, None)]."""
    rows = []
    for t, bl in blobs(frames, times, region, thresh, minpx, bg):
        if not bl:
            rows.append((t, None))
            continue
        b = max(bl, key=lambda b: b[6])
        rows.append((t, b[4], b[5], b[0], b[1], b[2], b[3], b[6]))
    return rows


def changes(frames, times, region, thresh=40):
    x0, y0, x1, y1 = region
    prev = None
    out = []
    for t, fr in zip(times, frames):
        cur = fr[y0:y1, x0:x1].astype(np.int16)
        out.append((t, int((np.abs(cur - prev) > thresh).sum()) if prev is not None else -1))
        prev = cur
    return out


def fit_speed(rows):
    """Least-squares px/s of the feet position over the frames where a blob was found."""
    pts = [(t, x, y) for t, *r in rows if r and r[0] is not None for x, y in [(r[0], r[1])]]
    if len(pts) < 3:
        return None
    t = np.array([p[0] for p in pts])
    x = np.array([p[1] for p in pts])
    y = np.array([p[2] for p in pts])
    vx = np.polyfit(t, x, 1)[0]
    vy = np.polyfit(t, y, 1)[0]
    return vx, vy, float(np.hypot(vx, vy)), len(pts), t[0], t[-1]


def main(argv):
    if not argv:
        print(__doc__)
        return
    cmd, a = argv[0], argv[1:]
    if cmd == "rec":
        fps = float(a[2]) if len(a) > 2 else 30.0
        region = tuple(int(v) for v in a[3:7]) if len(a) >= 7 else None
        t, f = record(a[0], float(a[1]), fps, region=region)
        print("frames", len(t), "span %.2f s" % (t[-1] - t[0]), "mean dt %.4f s" % np.diff(t).mean())
        return
    t, f = load(a[0])
    if cmd == "save":
        from PIL import Image

        Image.fromarray(f[int(a[1])]).save(a[2])
        return
    if cmd == "blobs":
        region = tuple(int(v) for v in a[1:5]) if len(a) >= 5 else None
        thresh = int(a[5]) if len(a) > 5 else 40
        minpx = int(a[6]) if len(a) > 6 else 40
        for tt, bl in blobs(f, t, region, thresh, minpx):
            print("%6.3f " % tt + " | ".join("(%d,%d)-(%d,%d) feet (%.1f,%d) n=%d" % b for b in bl))
        return
    if cmd == "track":
        region = tuple(int(v) for v in a[1:5])
        thresh = int(a[5]) if len(a) > 5 else 40
        rows = track(f, t, region, thresh)
        for r in rows:
            if r[1] is None:
                print("%6.3f -" % r[0])
            else:
                print("%6.3f feet (%.1f,%d) bbox (%d,%d)-(%d,%d) n=%d" % r)
        print("fit", fit_speed(rows))
        return
    if cmd == "changes":
        region = tuple(int(v) for v in a[1:5])
        thresh = int(a[5]) if len(a) > 5 else 40
        for tt, n in changes(f, t, region, thresh):
            print("%6.3f %d" % (tt, n))
        return
    print("unknown command", cmd)


if __name__ == "__main__":
    main(sys.argv[1:])
