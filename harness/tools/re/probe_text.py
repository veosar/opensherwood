"""TEXT: u16 n, entries (u8 kind?, u16 npts, pts...)?"""
import struct, sys
from rhp_chunks import load_chunks, rhp_path, map_size, hexdump, MAPS
for m in sys.argv[1:] or MAPS:
    ver, b = load_chunks(rhp_path(m))["TEXT"]; W, H = map_size(m)
    n = struct.unpack_from("<H", b, 0)[0]; pos = 2; ok = True; ents = []
    for i in range(n):
        try:
            k = b[pos]; npts = struct.unpack_from("<H", b, pos + 1)[0]; pos += 3
            pts = [struct.unpack_from("<HH", b, pos + 4 * j) for j in range(npts)]; pos += 4 * npts
            oob = sum(1 for x, y in pts if x > W or y > H)
            ents.append((k, npts, oob, pts[:3]))
        except Exception as e:
            ok = False; break
    print(m, "n", n, "consumed", pos, "of", len(b), "OK" if ok and pos == len(b) else "MISMATCH")
    for e in ents[:6]: print("   ", e)
