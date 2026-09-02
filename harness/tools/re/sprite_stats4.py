"""Walk the dictionary region as [u16 count][count * 8 bytes] pages, starting at .dic offset 6."""
import sys, os, struct
sys.path.insert(0, os.path.dirname(__file__))
import numpy as np
from spritebank import Bank

b = Bank()
pp = np.load(os.path.join(os.environ["RE_SCRATCH"], "page_maxsym.npy"))
maxs = pp[:, 2]
dic = b.dic
pos = 6
pages = []
for p in range(b.page_count):
    if pos + 2 > b.table_start:
        print("ran out at page", p); break
    cnt = struct.unpack_from("<H", dic, pos)[0]
    pages.append((p, pos, cnt, int(maxs[p]) if p < len(maxs) else -1))
    pos += 2 + cnt * 8
print("end pos", pos, "table_start", b.table_start, "leftover", b.table_start - pos)
bad = [(p, pos, cnt, mx) for p, pos, cnt, mx in pages if cnt != mx]
print("pages where count != max_sym+1:", bad)
print("first pages:", pages[:5])
if pos < b.table_start:
    print("leftover bytes:", dic[pos:b.table_start].hex())
# verify: most frequent symbol of each page decodes to 4 equal pixels
starts = {p: s + 2 for p, s, c, m in pages}
nonuniform = []
for p, s, cnt, mx in pages:
    idx = b.frames_of_page(p)
    hist = np.zeros(4096, dtype=np.int64)
    for i in idx[:30]:
        hist += np.bincount(b.stream(i), minlength=4096)
    t = int(np.argmax(hist))
    e = np.frombuffer(dic[starts[p] + 8 * t: starts[p] + 8 * t + 8], dtype="<u2")
    if not (e == e[0]).all():
        nonuniform.append((p, t, [hex(v) for v in e]))
    elif p < 3 or p > 130:
        print("page", p, "top symbol", t, "->", [hex(v) for v in e])
print("pages whose top symbol is not 4 equal pixels:", nonuniform)
np.save(os.path.join(os.environ["RE_SCRATCH"], "page_starts.npy"), np.array([(p, s + 2, cnt) for p, s, cnt, m in pages]))
