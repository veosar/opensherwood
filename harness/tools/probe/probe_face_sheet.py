"""Render FACE masks of a map as a contact sheet PNG: python probe_face_sheet.py <map> <out.png> [n]"""
import sys, numpy as np
from PIL import Image
from rhp_chunks import load_chunks, rhp_path
from probe_face_walk import walk
m, out = sys.argv[1], sys.argv[2]; n = int(sys.argv[3]) if len(sys.argv) > 3 else 40
ver, b = load_chunks(rhp_path(m))["FACE"]
count, recs, pos = walk(b, n)
tiles = []
for (off, L, hdr, w, h, rows, tail) in recs:
    arr = np.frombuffer(b"".join(rows), dtype=np.uint8).reshape(h, (w + 7) // 8)
    bits = np.unpackbits(arr, axis=1)[:, :w]
    tiles.append(bits)
tw = max(t.shape[1] for t in tiles) + 4; th = max(t.shape[0] for t in tiles) + 4
cols = 10; rows_n = (len(tiles) + cols - 1) // cols
sheet = np.full((rows_n * th, cols * tw), 64, dtype=np.uint8)
for i, t in enumerate(tiles):
    r, c = divmod(i, cols)
    sheet[r * th + 2:r * th + 2 + t.shape[0], c * tw + 2:c * tw + 2 + t.shape[1]] = t * 255
img = Image.fromarray(sheet)
if img.width > 1600: img = img.resize((1600, int(img.height * 1600 / img.width)))
img.save(out); print("wrote", out, img.size, "tiles", len(tiles))
