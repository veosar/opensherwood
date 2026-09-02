"""Look at page-0xFFFF frames: sizes, stream lengths, first words."""
import sys, os
sys.path.insert(0, os.path.dirname(__file__))
import numpy as np
from spritebank import Bank

b = Bank()
idx = b.frames_of_page(0xFFFF)
w = b.width[idx].astype(np.int64); h = b.height[idx].astype(np.int64); L = b.length[idx].astype(np.int64)
print("count", len(idx), "w range", w.min(), w.max(), "h range", h.min(), h.max())
print("len - 2*w*h: min", (L - 2 * w * h).min(), "max", (L - 2 * w * h).max())
print("len - 2*w*h - 2*h == 0:", ((L - 2 * w * h - 2 * h) == 0).sum())
print("len - 2*w*h - 4*h == 0:", ((L - 2 * w * h - 4 * h) == 0).sum())
print("len - 2*(w+1)*h - 2*h ...", ((L - 2 * w * h - 2 * h - 2) == 0).sum())
area = w * h
order = np.argsort(area, kind="stable")
for k in order[:60]:
    i = idx[k]
    s = b.stream(i)
    print(f"frame {i:6d} {w[k]:3d}x{h[k]:3d} len {L[k]:5d} (2wh={2*w[k]*h[k]:5d}) :", " ".join(f"{v:04x}" for v in s[:32]))
