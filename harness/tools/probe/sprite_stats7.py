"""Page-0xFFFF frames: test the row-span hypothesis [first_x u16][last_x u16][pixels first..last] on all."""
import sys, os
sys.path.insert(0, os.path.dirname(__file__))
import numpy as np
from spritebank import Bank

b = Bank()
idx = b.frames_of_page(0xFFFF)
w = b.width[idx].astype(np.int64); h = b.height[idx].astype(np.int64)
ok = 0; bad = []; key_in_span = 0; spans = 0; empty = 0; start_gt_end = 0; startfull = 0; hist_first = np.zeros(4, dtype=np.int64)
for k in range(len(idx)):
    s = b.stream(idx[k])
    p = 0; n = len(s); good = True
    for y in range(h[k]):
        if p + 2 > n:
            good = False; break
        a, e = int(s[p]), int(s[p + 1]); p += 2
        if e == 0xFFFF:
            empty += 1
            if a != 0:
                start_gt_end += 1
            continue
        if a > e or e >= w[k]:
            good = False; break
        cnt = e - a + 1
        spans += 1
        if a == 0 and e == w[k] - 1:
            startfull += 1
        seg = s[p:p + cnt]
        key_in_span += int((seg == 0x07C0).sum())
        p += cnt
    if good and p == n:
        ok += 1
    elif len(bad) < 5:
        bad.append((int(idx[k]), int(w[k]), int(h[k]), n, p))
print("frames fully consumed:", ok, "/", len(idx), "failures:", bad)
print("spans", spans, "empty rows", empty, "empty rows with first!=0", start_gt_end, "full-width spans", startfull, "0x07C0 pixels inside spans", key_in_span)
