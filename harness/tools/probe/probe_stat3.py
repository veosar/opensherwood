"""STAT hypothesis: u16 ngroups; group = u16 count, count x polygon(u8 R, u32 flags, u8 tag, u16 npts, npts x (u16,u16))."""
import struct, sys, collections
from rhp_chunks import load_chunks, rhp_path, map_size, MAPS

def parse_stat(b):
    ng = struct.unpack_from("<H", b, 0)[0]; pos = 2; groups = []
    for g in range(ng):
        cnt = struct.unpack_from("<H", b, pos)[0]; pos += 2; polys = []
        for i in range(cnt):
            R = b[pos]; flags = struct.unpack_from("<I", b, pos + 1)[0]; tag = b[pos + 5]
            npts = struct.unpack_from("<H", b, pos + 6)[0]; pos += 8
            pts = [struct.unpack_from("<HH", b, pos + 4 * k) for k in range(npts)]; pos += 4 * npts
            polys.append((R, flags, tag, pts))
        groups.append(polys)
    return groups, pos

if __name__ == "__main__":
    for m in sys.argv[1:] or MAPS:
        ver, b = load_chunks(rhp_path(m))["STAT"]; W, H = map_size(m)
        try:
            groups, pos = parse_stat(b)
        except Exception as e:
            print(m, "ERR", e); continue
        flags = collections.Counter(); oob = 0; npts = collections.Counter(); total = 0
        for polys in groups:
            for R, f, tag, pts in polys:
                flags[f] += 1; total += 1; npts[len(pts)] += 1
                oob += sum(1 for x, y in pts if x > W or y > H)
        print(m, (W, H), "groups", [len(g) for g in groups], "consumed", pos, "of", len(b), "OK" if pos == len(b) else "MISMATCH",
              "polys", total, "oob pts", oob, "flags", dict(flags), "npts min/max", min(npts), max(npts))
        print("   first R/tag per group:", [(g[0][0], g[0][2], len(g[0][3])) if g else None for g in groups][:12])
