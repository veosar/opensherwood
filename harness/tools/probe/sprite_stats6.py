"""Page-0xFFFF frames: print small non-trivial streams, then test RLE hypotheses on all of them."""
import sys, os
sys.path.insert(0, os.path.dirname(__file__))
import numpy as np
from spritebank import Bank

b = Bank()
idx = b.frames_of_page(0xFFFF)
w = b.width[idx].astype(np.int64); h = b.height[idx].astype(np.int64)
area = w * h
order = np.argsort(area, kind="stable")
shown = 0
for k in order:
    if area[k] <= 1:
        continue
    i = idx[k]
    s = b.stream(i)
    print(f"frame {i:6d} {w[k]:3d}x{h[k]:3d} len {len(s)*2:5d} :", " ".join(f"{v:04x}" for v in s[:48]))
    shown += 1
    if shown >= 25:
        break


def decode(s, w, h, count_minus_one):
    """Row = runs of [skip u16][count u16][pixels]; 0xFFFF in count position ends the row early.
    Returns (ok, words consumed)."""
    p = 0
    n = len(s)
    for y in range(h):
        x = 0
        while x < w:
            if p >= n:
                return False, p
            skip = int(s[p]); p += 1
            x += skip
            if p >= n:
                return False, p
            c = int(s[p]); p += 1
            if c == 0xFFFF:
                x = w
                break
            c = c + 1 if count_minus_one else c
            x += c
            p += c
            if x > w:
                return False, p
    return p == n, p


for cm1 in (True, False):
    ok = 0; bad = []
    for k in range(len(idx)):
        i = idx[k]
        s = b.stream(i)
        good, p = decode(s, w[k], h[k], cm1)
        if good:
            ok += 1
        elif len(bad) < 5:
            bad.append((int(i), int(w[k]), int(h[k]), len(s), p))
    print("count_minus_one", cm1, "frames fully consumed:", ok, "/", len(idx), "first failures", bad)
