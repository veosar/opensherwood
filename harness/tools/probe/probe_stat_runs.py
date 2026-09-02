"""STAT: find runs of plausible (u16 x, u16 y) pairs and print the gap bytes between runs."""
import struct, sys
from rhp_chunks import load_chunks, rhp_path, map_size
m = sys.argv[1]; maxgaps = int(sys.argv[2]) if len(sys.argv) > 2 else 30
ver, b = load_chunks(rhp_path(m))["STAT"]; W, H = map_size(m)
def ok(p):
    if p + 4 > len(b): return False
    x, y = struct.unpack_from("<HH", b, p); return x <= W + 8 and y <= H + 8 and (x, y) != (0, 0)
pos = 2; gaps = 0
while pos < len(b) and gaps < maxgaps:
    # find run start
    start = pos
    while pos < len(b) and not (ok(pos) and ok(pos + 4) and ok(pos + 8)): pos += 1
    gap = b[start:pos]
    runstart = pos
    while ok(pos): pos += 4
    print(f"gap@{start:06x} [{len(gap):3d}] {gap.hex(' ')}  -> run@{runstart:06x} {(pos-runstart)//4} pts")
    gaps += 1
