"""TEXT: u16 n; entry = u8 kind, u8 id, u16 npts, pts, u8 id2.  DARK: u16 n; entry = u16 z, u8 id, u16 npts, pts, u8 id2, u32 v."""
import struct, collections
from rhp_chunks import load_chunks, rhp_path, map_size, MAPS
for m in MAPS:
    ch = load_chunks(rhp_path(m)); W, H = map_size(m)
    b = ch["TEXT"][1]; n = struct.unpack_from("<H", b, 0)[0]; pos = 2; kinds = collections.Counter(); oob = 0
    for i in range(n):
        k, r, npts = struct.unpack_from("<BBH", b, pos); pos += 4
        pts = [struct.unpack_from("<HH", b, pos + 4 * j) for j in range(npts)]; pos += 4 * npts + 1
        kinds[k] += 1; oob += sum(1 for x, y in pts if x > W or y > H)
    t = f"TEXT n={n} {'OK' if pos == len(b) else 'MISMATCH'} kinds={dict(kinds)} oob={oob}"
    b = ch["DARK"][1]; n = struct.unpack_from("<H", b, 0)[0]; pos = 2; vals = collections.Counter(); zs = collections.Counter(); oob = 0
    for i in range(n):
        z, r, npts = struct.unpack_from("<HBH", b, pos); pos += 5
        pts = [struct.unpack_from("<HH", b, pos + 4 * j) for j in range(npts)]; pos += 4 * npts
        r2, v = struct.unpack_from("<BI", b, pos); pos += 5; vals[v] += 1; zs[z] += 1
        oob += sum(1 for x, y in pts if x > W or y > H)
    print(f"{m:13s} {t} | DARK n={n} {'OK' if pos == len(b) else 'MISMATCH'} v={dict(vals)} z={dict(zs)} oob={oob}")
