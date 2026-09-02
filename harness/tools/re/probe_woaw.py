"""WOAW hypothesis: u32 nlayers, (nlayers-1) u16, u16 narea, areas: u16 npts, npts*(4 f32), 6 f32 bbox,
u8 n, n*u32, 4 bytes, u8, u16."""
import struct, sys, collections
from rhp_chunks import load_chunks, rhp_path, map_size, MAPS
maps = sys.argv[1:] or MAPS
for m in maps:
    ver, b = load_chunks(rhp_path(m))["WOAW"]; W, H = map_size(m)
    nl = struct.unpack_from("<I", b, 0)[0]; pos = 4
    layers = struct.unpack_from("<%dH" % (nl - 1), b, pos); pos += 2 * (nl - 1)
    na = struct.unpack_from("<H", b, pos)[0]; pos += 2
    trailers = collections.Counter(); zs = collections.Counter(); c3 = collections.Counter(); npts_hist = collections.Counter()
    ok = True; oob = 0
    for i in range(na):
        try:
            npts = struct.unpack_from("<H", b, pos)[0]; pos += 2
            pts = [struct.unpack_from("<4f", b, pos + 16 * k) for k in range(npts)]; pos += 16 * npts
            bbox = struct.unpack_from("<6f", b, pos); pos += 24
            n = b[pos]; pos += 1
            lst = struct.unpack_from("<%dI" % n, b, pos); pos += 4 * n
            f4 = b[pos:pos + 4]; pos += 4
            x = b[pos]; pos += 1
            y = struct.unpack_from("<H", b, pos)[0]; pos += 2
            trailers[(n, lst[:3], f4.hex(), x, y)] += 1
            npts_hist[npts] += 1
            for p in pts:
                zs[round(p[3], 3)] += 1; c3[round(p[2], 4)] += 1
                if not (-50 <= p[0] <= W + 50 and -50 <= p[1] <= H + 50): oob += 1
            xs = [p[0] for p in pts]; ys = [p[1] for p in pts]
            if abs(min(xs) - bbox[0]) > 0.01 or abs(max(xs) - bbox[3]) > 0.01 or abs(min(ys) - bbox[1]) > 0.01 or abs(max(ys) - bbox[4]) > 0.01:
                print("  bbox mismatch area", i, bbox, min(xs), max(xs))
        except Exception as e:
            print("  ERR at area", i, e); ok = False; break
    print(m, "layers", nl, layers, "areas", na, "consumed", pos, "of", len(b), "OK" if pos == len(b) and ok else "MISMATCH", "oob pts", oob)
    print("  npts", sorted(npts_hist.items())[:12])
    print("  z(4th)", sorted(zs.items())[:10], "3rd", sorted(c3.items())[:6])
    print("  trailers", trailers.most_common(8))
