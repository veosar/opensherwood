"""Probe SBFONT bitmap fonts and SBTTFT descriptors (docs/formats/fonts.md).

Usage:
    python font_probe.py <font dir>                 # header, glyph table and layer statistics per file
    python font_probe.py <font dir> --png <outdir>  # also write <name>_layers.png (colour / mask / product)

Generic: reads only what is given on the command line, embeds no game bytes.
"""
import bz2
import collections
import glob
import os
import struct
import sys
import zlib

GLYPH_TABLE_OFFSET = 0x46
GLYPH_RECORD = "<HIIii"  # code, x, width, x_adjust, advance_adjust


def write_png(path, w, h, rgb):
    raw = b"".join(b"\0" + bytes(rgb[y * w * 3:(y + 1) * w * 3]) for y in range(h))

    def chunk(tag, body):
        return struct.pack(">I", len(body)) + tag + body + struct.pack(">I", zlib.crc32(tag + body) & 0xFFFFFFFF)

    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
                + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


def rgb565(p):
    r, g, b = (p >> 11) & 31, (p >> 5) & 63, p & 31
    return ((r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2))


def read_blob(b, off):
    w, h, comp, size = struct.unpack_from("<HHII", b, off)
    stream = b[off + 12:off + 12 + size]
    raw = bz2.decompress(stream) if comp == 2 else zlib.decompress(stream)
    assert len(raw) == w * h * 2, (len(raw), w, h)
    return w, h, struct.unpack("<%dH" % (w * h), raw), off + 12 + size


def parse_bfn(b):
    assert b[:6] == b"SBFONT"
    version = struct.unpack_from("<I", b, 6)[0]
    name = b[0x0A:0x2E].split(b"\0")[0].decode("latin-1")
    u2e, cell_h, u36, u3a, count, spacing = struct.unpack_from("<IIIIIi", b, 0x2E)
    off = GLYPH_TABLE_OFFSET
    glyphs = []
    for _ in range(count):
        glyphs.append(struct.unpack_from(GLYPH_RECORD, b, off))
        off += struct.calcsize(GLYPH_RECORD)
    colour = read_blob(b, off)
    mask = read_blob(b, colour[3])
    return dict(version=version, name=name, unknown_2e=u2e, cell_height=cell_h, unknown_36=u36,
                unknown_3a=u3a, spacing=spacing, glyphs=glyphs, colour=colour[:3], mask=mask[:3],
                consumed=mask[3] == len(b))


def parse_tfn(b):
    assert b[:6] == b"SBTTFT" and len(b) == 90
    version = struct.unpack_from("<I", b, 6)[0]
    name = b[0x0A:0x2E].split(b"\0")[0].decode("latin-1")
    u2e, size = struct.unpack_from("<II", b, 0x2E)
    face = b[0x36:0x56].split(b"\0")[0].decode("latin-1")
    r, g, bl, hi = b[0x56:0x5A]
    return dict(version=version, name=name, unknown_2e=u2e, size=size, face=face, colour=(r, g, bl), unknown_59=hi)


def layers_png(path, font, cols=520, scale=2):
    (w, h, colour), (_, _, mask) = font["colour"], font["mask"]
    cols = min(cols, w)
    out = bytearray()
    for layer in range(3):
        for y in range(h):
            row = bytearray()
            for x in range(cols):
                p, q = colour[y * w + x], mask[y * w + x]
                if layer == 0:
                    c = rgb565(p)
                elif layer == 1:
                    c = rgb565(q)
                else:
                    a = rgb565(q)[0]
                    c = tuple(v * a // 255 for v in rgb565(p))
                row += bytes(c) * scale
            out += row * scale
        out += bytes((255, 0, 255)) * cols * scale * 2
    write_png(path, cols * scale, (h * scale + 2) * 3, out)


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return
    d = sys.argv[1]
    png_dir = sys.argv[sys.argv.index("--png") + 1] if "--png" in sys.argv else None
    files = sorted(glob.glob(os.path.join(d, "*.bfn")) + glob.glob(os.path.join(d, "*.fnt")))
    for f in files:
        b = open(f, "rb").read()
        font = parse_bfn(b)
        g = font["glyphs"]
        summary = {k: v for k, v in font.items() if k not in ("glyphs", "colour", "mask")}
        summary["strip"] = font["colour"][:2]
        print(os.path.basename(f), len(b), "bytes", summary)
        print("  glyphs", len(g), "codes ascending", all(g[i][0] < g[i + 1][0] for i in range(len(g) - 1)),
              "max x+w", max(e[1] + e[2] for e in g), "x_adjust", collections.Counter(e[3] for e in g),
              "advance_adjust", collections.Counter(e[4] for e in g))
        print("  x pitch", collections.Counter(g[i + 1][1] - g[i][1] for i in range(1, len(g) - 1)).most_common(3))
        mask_hist = collections.Counter(font["mask"][2])
        print("  mask distinct", len(mask_hist), "top", [(hex(k), v) for k, v in mask_hist.most_common(4)],
              "colour distinct", len(set(font["colour"][2])))
        if png_dir:
            layers_png(os.path.join(png_dir, os.path.basename(f).split(".")[0] + "_layers.png"), font)
    for f in sorted(glob.glob(os.path.join(d, "*.tfn"))):
        print(os.path.basename(f), parse_tfn(open(f, "rb").read()))


if __name__ == "__main__":
    main()
