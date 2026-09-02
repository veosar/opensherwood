"""For each FACE record: find the kind byte inside the brute-forced header (the suffix after it must parse as
popcount(kind&3) polylines + x,y); print histogram of the 'pre' bytes = prev tail + hdr[:j]."""
import struct, sys, collections
from rhp_chunks import load_chunks, rhp_path
from probe_face_walk import walk

def polylines_ok(hdr, j):
    kind = hdr[j]; pos = j + 1
    for k in range(bin(kind & 3).count("1")):
        if pos + 3 > len(hdr): return False
        n = struct.unpack_from("<H", hdr, pos + 1)[0]; pos += 3 + 4 * n + 1
    return pos + 4 == len(hdr)

for m in sys.argv[1:] or ["derby"]:
    b = load_chunks(rhp_path(m))["FACE"][1]; count, recs, pos = walk(b)
    pats = collections.Counter(); examples = {}
    prev_tail = b[2:4].hex(" ")  # bytes after u16 count, before first record
    for i, (off, L, hdr, w, h, rows, tail) in enumerate(recs):
        js = [j for j in range(min(L, 40)) if polylines_ok(hdr, j)]
        j = js[-1] if js else None
        if j is None:
            pats["??"] += 1; continue
        pre = (prev_tail + " | " + hdr[:j].hex(" ")).strip()
        # normalise: replace ref values by 'R'
        key = pre; pats[key] += 1; examples.setdefault(key, (i, hex(off), hdr[j]))
        prev_tail = tail.hex(" ")
    print(m, "records", len(recs))
    for k, c in sorted(pats.items(), key=lambda kv: -kv[1])[:40]:
        print(f"  {c:4d}  {k}   e.g. {examples.get(k)}")
