"""Overlay FACE masks on the map at candidate positions. python probe_face_place.py <map> <out.png> <mode>"""
import sys, struct, numpy as np
from PIL import Image
from rhp_chunks import load_chunks, rhp_path, map_path
from probe_face_walk import walk
from map_png import load_map
m, out, mode = sys.argv[1], sys.argv[2], sys.argv[3]
ver, b = load_chunks(rhp_path(m))["FACE"]
count, recs, pos = walk(b)
img = load_map(map_path(m)).convert("RGBA")
ov = np.zeros((img.height, img.width, 4), dtype=np.uint8)
for i, (off, L, hdr, w, h, rows, tail) in enumerate(recs):
    arr = np.frombuffer(b"".join(rows), dtype=np.uint8).reshape(h, (w + 7) // 8)
    bits = np.unpackbits(arr, axis=1)[:, :w]
    A = struct.unpack_from("<H", hdr, 1)[0]; B = struct.unpack_from("<H", hdr, 4)[0]
    if mode == "AB": x0, y0 = A, B
    elif mode == "BA": x0, y0 = B, A
    elif mode == "A-w": x0, y0 = A - w, B
    elif mode == "AB-h": x0, y0 = A, B - h
    else: x0, y0 = 0, 0
    x1, y1 = min(img.width, x0 + w), min(img.height, y0 + h)
    if x1 <= max(x0, 0) or y1 <= max(y0, 0): continue
    sx, sy = max(0, -x0), max(0, -y0)
    sub = bits[sy:sy + (y1 - max(y0, 0)), sx:sx + (x1 - max(x0, 0))]
    col = [(255, 0, 0), (0, 255, 0), (0, 128, 255), (255, 255, 0)][i % 4]
    region = ov[max(y0, 0):y1, max(x0, 0):x1]
    region[sub == 1] = (*col, 140)
res = Image.alpha_composite(img, Image.fromarray(ov)); res.save(out); print("wrote", out)
