"""Hypothesis test: PPPP = u16 n, entries (u8 id, u16 npts, npts*(u16 x,u16 y), 6 bytes); 007 = u16 n, n*14 bytes."""
import struct
from rhp_chunks import load_chunks, rhp_path, map_size, MAPS
for m in MAPS:
    ch = load_chunks(rhp_path(m)); W, H = map_size(m)
    ver, b = ch["PPPP"]
    n = struct.unpack_from("<H", b, 0)[0]; pos = 2; ok = True; info = []
    for i in range(n):
        try:
            id_ = b[pos]; npts = struct.unpack_from("<H", b, pos + 1)[0]; pos += 3
            pts = [struct.unpack_from("<HH", b, pos + 4 * k) for k in range(npts)]; pos += 4 * npts
            tail = b[pos:pos + 6]; pos += 6
            inb = all(0 <= x <= W and 0 <= y <= H for x, y in pts)
            info.append((id_, npts, tail.hex(), inb))
        except Exception as e:
            ok = False; info.append(("ERR", str(e))); break
    print(m, "PPPP", "n=", n, "consumed", pos, "of", len(b), "ok" if pos == len(b) and ok else "MISMATCH")
    for x in info[:14]: print("   ", x)
    ver, b = ch["007 "]
    n = struct.unpack_from("<H", b, 0)[0]
    print(m, "007", "n=", n, "size", len(b), "rec", (len(b) - 2) / n if n else None)
    for i in range(min(n, 6)):
        r = struct.unpack_from("<hhhhHHH", b, 2 + 14 * i); print("   ", r)
    recs = [struct.unpack_from("<hhhhHHH", b, 2 + 14 * i) for i in range(n)]
    print("   a range", min(r[4] for r in recs), max(r[4] for r in recs), "b set", sorted(set(r[5] for r in recs))[:12], "c set", sorted(set(r[6] for r in recs)))
