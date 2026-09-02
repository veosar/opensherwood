"""First look: header, region stats, smallest frames and their streams."""
import sys, os
sys.path.insert(0, os.path.dirname(__file__))
import numpy as np
from spritebank import Bank

b = Bank()
print("pages", b.page_count, "sym/page", b.symbols_per_page, "region", len(b.region), "frames", len(b.width))
reg16 = np.frombuffer(b.region, dtype="<u2")
reg32 = np.frombuffer(b.region[: len(b.region) // 4 * 4], dtype="<u4")
print("region u16 count", len(reg16))
# frame 0
print("frame0", b.width[0], b.height[0], b.length[0], b.page[0], b.stream(0))
# smallest frames
area = b.width.astype(np.int64) * b.height
order = np.argsort(area, kind="stable")
for i in order[:40]:
    s = b.stream(i)
    print(f"frame {i:6d} {b.width[i]:3d}x{b.height[i]:3d} page {b.page[i]:5d} len {b.length[i]:4d} syms {len(s):3d} :", " ".join(f"{v:03x}" for v in s[:24]))
# ratio pixels/symbols by page
for p in [0, 1, 2, 133]:
    idx = b.frames_of_page(p)
    px = area[idx].sum(); sy = (b.length[idx] // 2).sum()
    print("page", p, "frames", len(idx), "pixels", px, "symbols", sy, "px/sym", px / sy, "min area", area[idx].min(), "max area", area[idx].max())
idx = b.frames_of_page(0xFFFF)
print("page FFFF frames", len(idx), "pixels", area[idx].sum(), "bytes", b.length[idx].sum(), "bytes/px", b.length[idx].sum() / area[idx].sum())
# search for a monotonic run of 134 u32s anywhere in region (any alignment)
for align in range(4):
    a = np.frombuffer(b.region[align: align + (len(b.region) - align) // 4 * 4], dtype="<u4")
    d = np.diff(a.astype(np.int64))
    inc = d > 0
    # longest run of increasing values
    best = 0; cur = 0; bestpos = 0
    for k, v in enumerate(inc):
        if v:
            cur += 1
            if cur > best:
                best = cur; bestpos = k
        else:
            cur = 0
    print("align", align, "longest increasing u32 run", best + 1, "ending at u32 index", bestpos + 1, "byte", align + 4 * (bestpos + 1))
