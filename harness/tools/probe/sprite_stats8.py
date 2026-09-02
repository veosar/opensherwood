"""Colour statistics: most frequent RGB565 values in dictionary entries and in page-less spans."""
import sys, os
sys.path.insert(0, os.path.dirname(__file__))
import numpy as np
from spritebank import Bank
from sprite_render import load_pages, decode_span_frame

b = Bank()
pages = load_pages(b)
hist = np.zeros(65536, dtype=np.int64)
for ent in pages:
    hist += np.bincount(ent.ravel(), minlength=65536)
top = np.argsort(hist)[::-1][:12]
print("dictionary entry pixels: total", hist.sum())
for t in top:
    print(f"  {t:04x} {hist[t]:9d} {hist[t]/hist.sum():.3%}")
for c in (0x07C0, 0x07E0, 0x001F, 0xF81F, 0x0000, 0xFFFF):
    print(f"  colour {c:04x}: {hist[c]}")
# entries that are 4x the same colour
uniform = 0
for ent in pages:
    uniform += int((ent == ent[:, :1]).all(axis=1).sum())
print("uniform entries", uniform, "of", sum(len(e) for e in pages))
# page-less frames
idx = b.frames_of_page(0xFFFF)
h2 = np.zeros(65536, dtype=np.int64)
rng = np.random.default_rng(1)
for i in rng.choice(idx, 400, replace=False):
    px = decode_span_frame(b, i)
    h2 += np.bincount(px.ravel(), minlength=65536)
print("page-less sample pixels (400 frames, incl. outside-span key):", h2.sum())
for t in np.argsort(h2)[::-1][:8]:
    print(f"  {t:04x} {h2[t]:9d} {h2[t]/h2.sum():.3%}")
