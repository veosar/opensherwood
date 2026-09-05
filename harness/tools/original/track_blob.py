"""Track a moving sprite in a sequence of static-camera screenshots by differencing (analyst helper, generic).

Usage: python track_blob.py <prefix> <ref_index> x0 y0 x1 y1 [thresh]
Frames are captures/original/<prefix>_<i>_<t>s.png; ref_index -1 = differ from both the first and the
last frame (start and end poses cancel out), otherwise that frame is the single reference.
For each frame prints: t, bbox of the changed pixels inside the region, centroid, bottom-centre (feet).
"""
import glob
import os
import re
import sys

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
CAPDIR = os.path.normpath(os.path.join(HERE, "..", "..", "captures", "original"))


def frames(prefix):
    """(index, t, path) sorted; when an index exists twice (an older run with the same prefix)
    the earliest time wins."""
    best = {}
    for p in glob.glob(os.path.join(CAPDIR, prefix + "_*.png")):
        m = re.search(r"_(\d+)_([\d.]+)s\.png$", p)
        if m:
            i, t = int(m.group(1)), float(m.group(2))
            if i not in best or t < best[i][0]:
                best[i] = (t, p)
    return sorted((i, t, p) for i, (t, p) in best.items())


def track(prefix, ref_index, region, thresh=40, min_pixels=30):
    fr = frames(prefix)
    x0, y0, x1, y1 = region
    # pixels must differ from BOTH the first and the last frame: the sprite's start and end
    # positions (present in one of the references) are excluded, only the moving sprite remains
    refs = [np.asarray(Image.open(fr[k][2]).convert("L"), dtype=np.int16)[y0:y1, x0:x1] for k in (0, -1)]
    if ref_index >= 0:
        refs = [np.asarray(Image.open(fr[ref_index][2]).convert("L"), dtype=np.int16)[y0:y1, x0:x1]]
    rows = []
    for i, t, p in fr:
        cur = np.asarray(Image.open(p).convert("L"), dtype=np.int16)[y0:y1, x0:x1]
        mask = np.ones_like(cur, dtype=bool)
        for ref in refs:
            mask &= np.abs(cur - ref) > thresh
        ys, xs = np.nonzero(mask)
        if len(xs) < min_pixels:
            rows.append((i, t, None))
            continue
        bx0, bx1, by0, by1 = xs.min() + x0, xs.max() + x0, ys.min() + y0, ys.max() + y0
        cx, cy = xs.mean() + x0, ys.mean() + y0
        # feet: median x of the lowest 6 rows of the blob
        low = ys >= ys.max() - 6
        fx = np.median(xs[low]) + x0
        rows.append((i, t, (bx0, by0, bx1, by1, cx, cy, fx, by1, len(xs))))
    return rows


def main(argv):
    prefix, ref = argv[0], int(argv[1])
    region = tuple(int(v) for v in argv[2:6])
    thresh = int(argv[6]) if len(argv) > 6 else 40
    for i, t, r in track(prefix, ref, region, thresh):
        if r is None:
            print("%3d %6.2f -" % (i, t))
        else:
            bx0, by0, bx1, by1, cx, cy, fx, fy, n = r
            print("%3d %6.2f bbox (%d,%d)-(%d,%d) centroid (%.1f,%.1f) feet (%.1f,%d) n=%d" % (i, t, bx0, by0, bx1, by1, cx, cy, fx, fy, n))


if __name__ == "__main__":
    main(sys.argv[1:])
