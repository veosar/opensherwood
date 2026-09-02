"""Walk STAT polygons (u8 R,u32 flags,u8 tag,u16 npts,pts) from an offset until it breaks; show break context."""
import struct, sys, collections
from rhp_chunks import load_chunks, rhp_path, map_size, hexdump
m = sys.argv[1]; start = int(sys.argv[2], 0)
ver, b = load_chunks(rhp_path(m))["STAT"]; W, H = map_size(m)
pos = start; n = 0; flags = collections.Counter(); Rs = []
while pos + 8 <= len(b):
    R = b[pos]; f = struct.unpack_from("<I", b, pos + 1)[0]; tag = b[pos + 5]; npts = struct.unpack_from("<H", b, pos + 6)[0]
    if f > 0xff or npts == 0 or npts > 1000 or pos + 8 + 4 * npts > len(b): break
    pts = [struct.unpack_from("<HH", b, pos + 8 + 4 * k) for k in range(npts)]
    if any(x > W + 4 or y > H + 4 for x, y in pts): break
    flags[f] += 1; Rs.append(R); pos += 8 + 4 * npts; n += 1
print(m, "polys", n, "from", hex(start), "to", hex(pos), "of", hex(len(b)), "flags", dict(flags))
print("R seq", bytes(Rs[:40]).hex(' '))
print(hexdump(b, max(0, pos - 32), 96))
