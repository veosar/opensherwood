"""STAT: parse polygons as (u8 R, u32 zero, u8 tag, u16 npts, pts) from a start offset; report breaks."""
import struct, sys
from rhp_chunks import load_chunks, rhp_path, map_size, hexdump
m = sys.argv[1]; start = int(sys.argv[2], 0)
ver, b = load_chunks(rhp_path(m))["STAT"]; W, H = map_size(m)
print(m, (W, H), "len", len(b)); print(hexdump(b, 0, 16))
pos = start; n = 0; tags = []
while pos + 8 <= len(b):
    R = b[pos]; z = struct.unpack_from("<I", b, pos + 1)[0]; tag = b[pos + 5]; npts = struct.unpack_from("<H", b, pos + 6)[0]
    if z != 0 or npts == 0 or npts > 2000 or pos + 8 + 4 * npts > len(b):
        print(f"break at {pos:06x} after {n} polys: {b[pos:pos+40].hex(' ')}"); break
    pts = [struct.unpack_from("<HH", b, pos + 8 + 4 * k) for k in range(npts)]
    bad = sum(1 for x, y in pts if x > W or y > H)
    tags.append((R, tag, npts, bad))
    pos += 8 + 4 * npts; n += 1
print("polys", n, "end", pos, "tags", tags[:20])
