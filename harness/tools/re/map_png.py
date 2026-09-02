"""Decode a .map/.min image blob (u16 w, u16 h, u32, u32 csize, bzip2 RGB565) to PNG.
python map_png.py <in.map> <out.png>; also usable as library: load_map(path) -> PIL RGB image."""
import bz2, struct, sys, numpy as np
from PIL import Image

def load_map(path):
    d = open(path, "rb").read()
    w, h, _k, csize = struct.unpack_from("<HHII", d, 0)
    raw = bz2.decompress(d[12:12 + csize])
    px = np.frombuffer(raw, dtype="<u2")[: w * h].reshape(h, w)
    r = ((px >> 11) & 31) * 255 // 31; g = ((px >> 5) & 63) * 255 // 63; b = (px & 31) * 255 // 31
    return Image.fromarray(np.dstack([r, g, b]).astype(np.uint8), "RGB")

if __name__ == "__main__":
    img = load_map(sys.argv[1]); img.save(sys.argv[2]); print("wrote", sys.argv[2], img.size)
