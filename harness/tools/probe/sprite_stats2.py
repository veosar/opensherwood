"""Symbol count vs dimensions per frame; per-page symbol usage vs dictionary region size."""
import sys, os
sys.path.insert(0, os.path.dirname(__file__))
import numpy as np
from spritebank import Bank

b = Bank()
w = b.width.astype(np.int64); h = b.height.astype(np.int64)
syms = b.length // 2
pg = b.page != 0xFFFF
cands = {
    "w*h/4 ceil": (w * h + 3) // 4,
    "ceil(w/2)*ceil(h/2)": ((w + 1) // 2) * ((h + 1) // 2),
    "ceil(w/4)*h": ((w + 3) // 4) * h,
    "w*ceil(h/4)": w * ((h + 3) // 4),
    "ceil(w/2)*h/2..": ((w + 1) // 2) * h // 2,
}
for name, c in cands.items():
    ok = (c[pg] == syms[pg]).sum()
    print(f"{name:24s} matches {ok}/{pg.sum()}")
odd = pg & ((w % 2 == 1) | (h % 2 == 1))
print("page frames with odd w or h:", odd.sum())
for i in np.nonzero(odd)[0][:10]:
    print(i, w[i], h[i], syms[i])
# per page: max symbol, distinct count
tot_max = 0; tot_distinct = 0
per_page = []
for p in range(b.page_count):
    idx = b.frames_of_page(p)
    seen = np.zeros(4096, dtype=bool)
    for i in idx:
        s = b.stream(i)
        seen[s] = True
    mx = np.nonzero(seen)[0].max() + 1
    per_page.append((p, len(idx), mx, seen.sum()))
    tot_max += mx; tot_distinct += seen.sum()
for row in per_page[:8] + per_page[-4:]:
    print("page %3d frames %5d max_sym+1 %4d distinct %4d" % row)
print("sum(max+1)", tot_max, "sum distinct", tot_distinct)
R = len(b.region)
for entry in [6, 8, 10, 12, 16]:
    rem = R - tot_max * entry
    print(f"entry {entry}: region - sum(max+1)*entry = {rem}, /134 = {rem/134:.2f}")
np.save(os.path.join(os.environ.get("RE_SCRATCH", "."), "page_maxsym.npy"), np.array(per_page))
