"""Collect distinct STAT polygon 'middle' strings (bytes between id byte and tag byte) over all maps."""
import struct, collections
from rhp_chunks import load_chunks, rhp_path, map_size, MAPS
mids = collections.Counter(); examples = {}
for m in MAPS:
    ver, b = load_chunks(rhp_path(m))["STAT"]; W, H = map_size(m)
    def body_at(p):
        if p + 2 > len(b): return None
        n = struct.unpack_from("<H", b, p)[0]
        if n < 3 or n > 3000 or p + 2 + 4 * n > len(b): return None
        for k in range(n):
            x, y = struct.unpack_from("<HH", b, p + 2 + 4 * k)
            if x > W + 2 or y > H + 2: return None
        return n
    def nxt(p, lim):
        for q in range(p, min(p + lim, len(b))):
            if body_at(q): return q
        return None
    pos = 2; last_end = 2
    while pos < len(b):
        n = body_at(pos)
        if n and (nxt(pos + 2 + 4 * n, 40) is not None):
            hdr = b[last_end:pos]
            if 6 <= len(hdr) <= 40:
                mid = hdr[1:-1].hex(' '); mids[mid] += 1; examples.setdefault(mid, (m, hex(last_end)))
            pos += 2 + 4 * n; last_end = pos
        else:
            pos += 1
for mid, c in sorted(mids.items(), key=lambda kv: (len(kv[0]), kv[0])):
    print(f"{c:5d} [{(len(mid)+1)//3:2d}] {mid}   e.g. {examples[mid]}")
