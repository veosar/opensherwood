"""Test page layout hypotheses: pages of (max_symbol+1) entries of 8 bytes back to back."""
import sys, os
sys.path.insert(0, os.path.dirname(__file__))
import numpy as np
from spritebank import Bank

b = Bank()
pp = np.load(os.path.join(os.environ["RE_SCRATCH"], "page_maxsym.npy"))
counts = pp[:, 2]
print("pages with <4096 symbols:", [(int(p), int(c)) for p, c in zip(pp[:, 0], counts) if c != 4096])
reg = b.region
u16 = np.frombuffer(reg, dtype="<u2")

def show(off, n=4):
    return " ".join(f"{v:04x}" for v in np.frombuffer(reg[off:off + 2 * n], dtype="<u2"))

# most frequent symbol per page (first 6 pages)
starts_h1 = np.concatenate([[0], np.cumsum(counts * 8)])
for p in range(6):
    idx = b.frames_of_page(p)
    hist = np.zeros(4096, dtype=np.int64)
    for i in idx[:50]:
        hist += np.bincount(b.stream(i), minlength=4096)
    top = np.argsort(hist)[::-1][:3]
    print(f"page {p} start_h1 {starts_h1[p]} top symbols {[(int(t), int(hist[t])) for t in top]}")
    for t in top[:2]:
        print("   H1 entry:", show(starts_h1[p] + 8 * t), "| H2 (+2/page):", show(starts_h1[p] + 2 * (p + 1) + 8 * t))
# tail of region
print("region tail 326 bytes at", len(reg) - 326, ":", show(len(reg) - 326, 40))
print("region head:", show(0, 24))
