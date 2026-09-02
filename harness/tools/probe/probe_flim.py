"""FLIM: u16 n; entry = pstring16 sprite, pstring16 name, u16 x, u16 y, u16 c, u8[3], u8 id, u16 npts, pts, u8 id2."""
import struct, collections
from rhp_chunks import load_chunks, rhp_path, map_size, MAPS
for m in MAPS:
    b = load_chunks(rhp_path(m))["FLIM"][1]; W, H = map_size(m)
    n = struct.unpack_from("<H", b, 0)[0]; pos = 2; sprites = collections.Counter(); flags = collections.Counter(); npc = collections.Counter(); oob = 0; cs = []
    for i in range(n):
        l = struct.unpack_from("<H", b, pos)[0]; sprite = b[pos + 2:pos + 2 + l].decode("latin-1"); pos += 2 + l
        l = struct.unpack_from("<H", b, pos)[0]; name = b[pos + 2:pos + 2 + l].decode("latin-1"); pos += 2 + l
        x, y, c, f0, f1, f2, r, npts = struct.unpack_from("<HHHBBBBH", b, pos); pos += 12
        pts = [struct.unpack_from("<HH", b, pos + 4 * j) for j in range(npts)]; pos += 4 * npts + 1
        sprites[sprite] += 1; flags[(f0, f1, f2)] += 1; npc[npts] += 1; cs.append(c)
        oob += (x > W or y > H) + sum(1 for px, py in pts if px > W or py > H)
    print(f"{m:13s} n={n} {'OK' if pos == len(b) else 'MISMATCH'} sprites={len(sprites)} flags={dict(flags)} npts={dict(npc)} oob={oob} c-range={min(cs)}..{max(cs)}")
