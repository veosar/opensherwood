"""Overlay FACE masks on the map with (x,y) = last 4 header bytes; draw candidate polylines.
python probe_face_place.py <map> <out.png> [crop x0 y0 x1 y1]"""
import sys, struct, numpy as np
from PIL import Image, ImageDraw
from rhp_chunks import load_chunks, rhp_path, map_path
from probe_face_walk import walk
from map_png import load_map
m, out = sys.argv[1], sys.argv[2]
ver, b = load_chunks(rhp_path(m))["FACE"]
count, recs, pos = walk(b)
img = load_map(map_path(m)).convert("RGBA")
ov = np.zeros((img.height, img.width, 4), dtype=np.uint8)
lines = []
for i, (off, L, hdr, w, h, rows, tail) in enumerate(recs):
    arr = np.frombuffer(b"".join(rows), dtype=np.uint8).reshape(h, (w + 7) // 8)
    bits = np.unpackbits(arr, axis=1)[:, :w]
    x0, y0 = struct.unpack_from("<HH", hdr, L - 4)
    x1, y1 = min(img.width, x0 + w), min(img.height, y0 + h)
    if x1 <= x0 or y1 <= y0: print("rec", i, "outside", x0, y0, w, h); continue
    sub = bits[:y1 - y0, :x1 - x0]
    col = [(255, 0, 0), (0, 255, 0), (0, 128, 255), (255, 255, 0)][i % 4]
    region = ov[y0:y1, x0:x1]
    region[sub == 1] = (*col, 150)
    if hdr[0] != 4:
        pts = [struct.unpack_from("<HH", hdr, 4 + 4 * k) for k in range((L - 9) // 4)]
        lines.append((i, hdr[0], (x0, y0), pts))
res = Image.alpha_composite(img, Image.fromarray(ov))
d = ImageDraw.Draw(res)
for i, kind, (x0, y0), pts in lines:
    d.line(pts, fill=(255, 0, 255), width=2)
    d.text((x0, y0), f"{i}k{kind}", fill=(255, 255, 255))
if len(sys.argv) > 3:
    cx0, cy0, cx1, cy1 = map(int, sys.argv[3:7]); res = res.crop((cx0, cy0, cx1, cy1))
res.save(out); print("wrote", out, res.size)
