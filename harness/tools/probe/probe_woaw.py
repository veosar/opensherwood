"""WOAW hypothesis: u16 nlayers, nlayers x u16, u16 nareas; area = u16 npts, npts x (4 f32), 6 f32 bbox,
u8 nlinks, nlinks x (u16,u16), u8[4], u8, u16 nl2, nl2 x u16."""
import struct, sys, collections
from rhp_chunks import load_chunks, rhp_path, map_size, MAPS
for m in sys.argv[1:] or MAPS:
    ver, b = load_chunks(rhp_path(m))["WOAW"]; W, H = map_size(m)
    nl = struct.unpack_from("<H", b, 0)[0]; layers = struct.unpack_from("<%dH" % nl, b, 2); pos = 2 + 2 * nl
    na = struct.unpack_from("<H", b, pos)[0]; pos += 2
    f4s = collections.Counter(); xs = collections.Counter(); nl2s = collections.Counter(); links = collections.Counter(); oob = 0; bad_bbox = 0; npts_min = 99
    for i in range(na):
        npts = struct.unpack_from("<H", b, pos)[0]; pos += 2
        pts = [struct.unpack_from("<4f", b, pos + 16 * k) for k in range(npts)]; pos += 16 * npts
        bbox = struct.unpack_from("<6f", b, pos); pos += 24
        k = b[pos]; pos += 1; lk = [struct.unpack_from("<HH", b, pos + 4 * j) for j in range(k)]; pos += 4 * k
        f4 = b[pos:pos + 4]; pos += 4; x = b[pos]; pos += 1
        n2 = struct.unpack_from("<H", b, pos)[0]; pos += 2; l2 = struct.unpack_from("<%dH" % n2, b, pos); pos += 2 * n2
        f4s[f4.hex()] += 1; xs[x] += 1; nl2s[n2] += 1; npts_min = min(npts_min, npts)
        for a, l in lk: links[l] += 1
        oob += sum(1 for p in pts if not (-100 <= p[0] <= W + 100 and -100 <= p[1] <= H + 100))
        px = [p[0] for p in pts]; py = [p[1] for p in pts]
        if pts and (abs(min(px) - bbox[0]) > 0.01 or abs(max(px) - bbox[3]) > 0.01 or abs(min(py) - bbox[1]) > 0.01 or abs(max(py) - bbox[4]) > 0.01): bad_bbox += 1
        if n2 and sorted(l2) != list(range(n2)) and i < 3: print("   l2", l2)
    print(f"{m:13s} layers={nl} ids=0..{max(layers)} areas={na} {'OK' if pos == len(b) else 'MISMATCH %d/%d' % (pos, len(b))} npts_min={npts_min} oob={oob} bad_bbox={bad_bbox} f4={dict(f4s)} x={dict(xs)} nl2={dict(nl2s)} link-layers={dict(links)}")
