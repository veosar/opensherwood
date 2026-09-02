"""For each FACE record i: gap = bytes between rows end of i and the kind byte of i+1 (first valid split).
Group gaps by (kind_i & 0x10) to test 'refs trailer belongs to kinds with bit 4'."""
import struct, sys, collections
from rhp_chunks import load_chunks, rhp_path
from probe_face_walk import walk, decode_row
from probe_face_pre import polylines_ok

for m in sys.argv[1:] or ["derby"]:
    b = load_chunks(rhp_path(m))["FACE"][1]; count, recs, pos = walk(b)
    info = []
    for i, (off, L, hdr, w, h, rows, tail) in enumerate(recs):
        js = [j for j in range(min(L, 60)) if polylines_ok(hdr, j)]
        kind = hdr[js[0]] if js else None
        p = off + L + 6
        for r in range(h): p = decode_row(b, p, (w + 7) // 8)[1]
        info.append((off, kind, js[0] if js else None, p))
    stats = collections.Counter(); ex = {}
    for i in range(len(info) - 1):
        off, kind, j, rows_end = info[i]; noff, nkind, nj, _ = info[i + 1]
        gap = b[rows_end:noff + nj]
        key = (kind is not None and bool(kind & 0x10), len(gap))
        stats[key] += 1; ex.setdefault(key, (i, hex(off), kind, gap.hex(' ')))
    print(m, "records", len(recs), "last gap:", b[info[-1][3]:].hex(' '), "last kind", info[-1][1])
    for k in sorted(stats): print(f"  bit4={k[0]!s:5} gaplen={k[1]:3d} x{stats[k]:4d}  e.g. {ex[k]}")
