"""STAT: locate polygon bodies (u16 npts then npts in-range points, npts>=3) followed within 40 bytes by another
body; print the header strings between bodies grouped by length."""
import struct, sys, collections
from rhp_chunks import load_chunks, rhp_path, map_size
m = sys.argv[1]; show = int(sys.argv[2]) if len(sys.argv) > 2 else 6
ver, b = load_chunks(rhp_path(m))["STAT"]; W, H = map_size(m)
def body_at(p):
    if p + 2 > len(b): return None
    n = struct.unpack_from("<H", b, p)[0]
    if n < 3 or n > 3000 or p + 2 + 4 * n > len(b): return None
    for k in range(n):
        x, y = struct.unpack_from("<HH", b, p + 2 + 4 * k)
        if x > W + 2 or y > H + 2: return None
    return n
def next_body_within(p, lim):
    for q in range(p, min(p + lim, len(b))):
        if body_at(q): return q
    return None
pos = 2; hdrs = collections.defaultdict(list); last_end = 2; bodies = 0
while pos < len(b):
    n = body_at(pos)
    if n and (next_body_within(pos + 2 + 4 * n, 40) is not None or pos + 2 + 4 * n >= len(b) - 40):
        hdrs[pos - last_end].append(b[last_end:pos].hex(' ')); pos += 2 + 4 * n; last_end = pos; bodies += 1
    else:
        pos += 1
print(m, "bodies", bodies, "trailing", len(b) - last_end, b[last_end:last_end + 48].hex(' '))
for L in sorted(hdrs):
    print(f"len {L:3d} x{len(hdrs[L]):4d}:")
    for h in hdrs[L][:show]: print("     ", h)
