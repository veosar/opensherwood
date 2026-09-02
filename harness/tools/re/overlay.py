"""Draw decoded RHP geometry over the map background.
python overlay.py <map> <out.png> <layers comma list: woaw,bond,stat,pppp> [crop x0 y0 x1 y1]"""
import struct, sys
from PIL import Image, ImageDraw
from rhp_chunks import load_chunks, rhp_path, map_path
from map_png import load_map

def woaw_areas(b):
    nl = struct.unpack_from("<I", b, 0)[0]; pos = 4 + 2 * (nl - 1)
    na = struct.unpack_from("<H", b, pos)[0]; pos += 2; areas = []
    for i in range(na):
        npts = struct.unpack_from("<H", b, pos)[0]; pos += 2
        pts = [struct.unpack_from("<4f", b, pos + 16 * k) for k in range(npts)]; pos += 16 * npts
        bbox = struct.unpack_from("<6f", b, pos); pos += 24
        n = b[pos]; pos += 1; lst = struct.unpack_from("<%dI" % n, b, pos); pos += 4 * n
        f4 = b[pos:pos + 4]; pos += 4; x = b[pos]; pos += 1; y = struct.unpack_from("<H", b, pos)[0]; pos += 2
        areas.append((pts, bbox, lst, f4, x, y))
    return areas

def bonds(b):
    n = struct.unpack_from("<H", b, 0)[0]
    return [struct.unpack_from("<hhhhHHH", b, 2 + 14 * i) for i in range(n)]

def pppp(b):
    n = struct.unpack_from("<H", b, 0)[0]; pos = 2; out = []
    for i in range(n):
        id_ = b[pos]; npts = struct.unpack_from("<H", b, pos + 1)[0]; pos += 3
        pts = [struct.unpack_from("<HH", b, pos + 4 * k) for k in range(npts)]; pos += 4 * npts
        tail = b[pos:pos + 6]; pos += 6; out.append((id_, pts, tail))
    return out, pos

if __name__ == "__main__":
    m, out, layers = sys.argv[1], sys.argv[2], sys.argv[3].split(",")
    ch = load_chunks(rhp_path(m)); img = load_map(map_path(m)).convert("RGBA")
    d = ImageDraw.Draw(img, "RGBA")
    if "stat" in layers:
        from probe_stat3 import parse_stat
        groups, _ = parse_stat(ch["STAT"][1])
        cols = [(255, 255, 0), (255, 0, 0), (0, 255, 255), (255, 0, 255), (0, 255, 0), (255, 128, 0), (128, 128, 255), (255, 255, 255), (0, 128, 0), (128, 0, 0), (0, 0, 128), (128, 128, 0), (255, 0, 128), (0, 255, 128)]
        for gi, polys in enumerate(groups):
            c = cols[gi % len(cols)]
            for R, f, tag, pts in polys:
                if len(pts) >= 2: d.line(pts + [pts[0]], fill=c + (255,), width=2 if gi else 3)
                if f: d.text(pts[0], f"f{f}", fill=c)
    if "woaw" in layers:
        for i, (pts, bbox, lst, f4, x, y) in enumerate(woaw_areas(ch["WOAW"][1])):
            poly = [(p[0], p[1]) for p in pts]
            d.polygon(poly, fill=(0, 255, 0, 40), outline=(0, 255, 0, 255))
            cx = sum(p[0] for p in poly) / len(poly); cy = sum(p[1] for p in poly) / len(poly)
            d.text((cx, cy), f"{i}", fill=(255, 255, 255, 255))
    if "bond" in layers:
        for (x1, y1, x2, y2, a, bb, c) in bonds(ch["007 "][1]):
            d.line([(x1, y1), (x2, y2)], fill=(255, 0, 0, 255), width=3)
            d.text(((x1 + x2) / 2, (y1 + y2) / 2), f"{a}-{bb}/{c}", fill=(255, 200, 200, 255))
    if "pppp" in layers:
        polys, _ = pppp(ch["PPPP"][1])
        for id_, pts, tail in polys:
            d.line(pts + [pts[0]], fill=(255, 0, 255, 255), width=2); d.text(pts[0], f"P{id_}", fill=(255, 0, 255, 255))
    if len(sys.argv) > 4:
        img = img.crop(tuple(map(int, sys.argv[4:8])))
    img.save(out); print("wrote", out, img.size)
