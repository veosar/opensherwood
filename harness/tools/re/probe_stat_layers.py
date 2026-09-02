"""Scan the undecoded remainder of the STAT chunk for further polygons (per-layer motion areas?).

Usage: python probe_stat_layers.py [map ...] [--point X Y]

Replicates the decoded prefix framing of docs/formats/rhp.md (header, boundary 0x5a, segments 0x82,
obstacles), then walks the remainder looking for records framed like polylines: u8 id, u16 n, n x (u16 x,
u16 y), u8 id2, with every point inside the map. Prints the records, the bytes between them, and whether a
given map point lies inside each polygon. Generic, no game bytes embedded.
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from rhp_chunks import load_chunks, map_size, rhp_path  # noqa: E402


def u8(b, p):
    return b[p], p + 1


def u16(b, p):
    return struct.unpack_from("<H", b, p)[0], p + 2


def u32(b, p):
    return struct.unpack_from("<I", b, p)[0], p + 4


def point_list(b, p):
    n, p = u16(b, p)
    pts = [struct.unpack_from("<HH", b, p + 4 * i) for i in range(n)]
    return pts, p + 4 * n


def polyline(b, p):
    _id, p = u8(b, p)
    pts, p = point_list(b, p)
    id2, p = u8(b, p)
    return (_id, pts, id2), p


def decoded_prefix(body):
    p = 0
    hdr = struct.unpack_from("<HHBI", body, 0)
    p = 9
    bid, p = u8(body, p)
    boundary, p = point_list(body, p)
    sid, p = u8(body, p)
    nseg, p = u16(body, p)
    p += 8 * nseg
    _, p = u32(body, p)
    nobst, p = u16(body, p)
    _, p = u32(body, p)
    obstacles = []
    for _ in range(nobst):
        poly, p = polyline(body, p)
        flags, p = u32(body, p)
        obstacles.append((poly, flags))
    return hdr, boundary, obstacles, p


def inside(poly, x, y):
    n = len(poly)
    res = False
    for i in range(n):
        x1, y1 = poly[i]
        x2, y2 = poly[(i + 1) % n]
        if (y1 > y) != (y2 > y):
            xi = x1 + (y - y1) * (x2 - x1) / (y2 - y1)
            if x < xi:
                res = not res
    return res


def scan(body, start, w, h, limit=400):
    """Greedy scan: at each position try to read a polyline whose points are all inside the map."""
    p = start
    records = []
    gap_start = p
    while p + 4 <= len(body) and len(records) < limit:
        _id = body[p]
        n = struct.unpack_from("<H", body, p + 1)[0]
        if 3 <= n <= 2000 and p + 3 + 4 * n + 1 <= len(body):
            pts = [struct.unpack_from("<HH", body, p + 3 + 4 * i) for i in range(n)]
            if all(x <= w and y <= h for x, y in pts):
                id2 = body[p + 3 + 4 * n]
                records.append((p, _id, n, id2, pts, body[gap_start:p]))
                p = p + 3 + 4 * n + 1
                gap_start = p
                continue
        p += 1
    return records, body[gap_start:]


def main(argv):
    pt = None
    if "--point" in argv:
        i = argv.index("--point")
        pt = (int(argv[i + 1]), int(argv[i + 2]))
        argv = argv[:i] + argv[i + 3:]
    maps = argv or ["lincoln"]
    for name in maps:
        chunks = load_chunks(rhp_path(name))
        ver, body = chunks["STAT"]
        w, h = map_size(name)
        hdr, boundary, obstacles, p = decoded_prefix(body)
        print(f"== {name} {w}x{h} STAT header {hdr}, boundary {len(boundary)} pts, {len(obstacles)} obstacles, decoded prefix ends at {p} of {len(body)}")
        if pt:
            print("  point in boundary:", inside(boundary, *pt), " in obstacle:", [i for i, (poly, _) in enumerate(obstacles) if inside(poly[1], *pt)])
        records, tail = scan(body, p, w, h)
        for off, _id, n, id2, pts, gap in records[:80]:
            xs = [x for x, _ in pts]
            ys = [y for _, y in pts]
            hit = f" contains point" if pt and inside(pts, *pt) else ""
            print(f"  @{off:6d} id=0x{_id:02x} n={n:4d} id2=0x{id2:02x} bbox=({min(xs)},{min(ys)})-({max(xs)},{max(ys)}) gap={len(gap)}B {gap[:24].hex()}{hit}")
        print(f"  {len(records)} records, tail {len(tail)} bytes: {tail[:32].hex()}")


if __name__ == "__main__" and "--seq" not in sys.argv:
    main(sys.argv[1:])


def sequential(name, pt=None, verbose=True):
    """Hypothesis: after the obstacles, `layer_count - 1` records shaped like the first layer:
    u8 tag, polyline(id, n, pts, id2), u16 nseg, nseg x 8 bytes, u32 0, u16 nobst, u32 0,
    nobst x { polyline, u32 flags }. Stops at the first inconsistency."""
    chunks = load_chunks(rhp_path(name))
    _, body = chunks["STAT"]
    w, h = map_size(name)
    hdr, boundary, obstacles, p = decoded_prefix(body)
    layers = [(0, None, boundary, obstacles)]
    ok = True
    while p + 4 <= len(body) and ok:
        start = p
        try:
            tag, q = u8(body, p)
            poly, q = polyline(body, q)
            if not all(x <= w and y <= h for x, y in poly[1]) or len(poly[1]) < 3:
                raise ValueError("polygon outside map")
            nseg, q = u16(body, q)
            q += 8 * nseg
            z1, q = u32(body, q)
            nobst, q = u16(body, q)
            a, q = u16(body, q)
            b, q = u16(body, q)
            obs = []
            for _ in range(nobst):
                op, q = polyline(body, q)
                if not all(x <= w and y <= h for x, y in op[1]):
                    raise ValueError("obstacle outside map")
                flags, q = u32(body, q)
                obs.append((op, flags))
            # Tail observed: u16 a, u16 b before the obstacles? No: they precede them in the first layer
            # framing (u32 0). Here `a`/`b` are read before the obstacles; extra u16 when a >= 2 (observed).
            c = None
            if a >= 2:
                c, q = u16(body, q)
            z2 = (a, b, c)
            layers.append((tag, poly, poly[1], obs))
            if verbose:
                xs = [x for x, _ in poly[1]]; ys = [y for _, y in poly[1]]
                hit = " <- contains point" if pt and inside(poly[1], *pt) else ""
                print(f"  layer {len(layers)-1}: tag=0x{tag:02x} id=0x{poly[0]:02x}/0x{poly[2]:02x} n={len(poly[1])} nseg={nseg} z1={z1} abc={z2} nobst={nobst} bbox=({min(xs)},{min(ys)})-({max(xs)},{max(ys)}) @{start}..{q}{hit}")
            p = q
        except Exception as ex:  # noqa: BLE001
            print(f"  stop at {start} ({ex}); {len(layers)} layers parsed, header layer count {hdr[0]}; next bytes {body[start:start+24].hex()}")
            ok = False
    return layers, p, len(body)


if __name__ == "__main__" and "--seq" in sys.argv:
    args = [a for a in sys.argv[1:] if a != "--seq"]
    pt = None
    if "--point" in args:
        i = args.index("--point"); pt = (int(args[i+1]), int(args[i+2])); args = args[:i] + args[i+3:]
    for name in args or ["lincoln"]:
        print("==", name)
        layers, p, n = sequential(name, pt)
        print(f"  parsed up to {p} of {n}")
