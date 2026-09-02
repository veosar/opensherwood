"""Decode dictionary-page frames to PNG: usage sprite_render.py out_dir frame_index [frame_index...]

Layout (docs/formats/sprites.md): page = u16 count + count * 4 RGB565 pixels; a frame stream is
ceil(w/4)*h u16 symbols, each a 4x1 run; colour 0x07C0 is transparent.
"""
import sys, os, struct
sys.path.insert(0, os.path.dirname(__file__))
import numpy as np
from spritebank import Bank, rgb565_to_rgb8, write_png

KEY = 0x07C0


def load_pages(b):
    pos = 6
    pages = []
    for _ in range(b.page_count):
        cnt = struct.unpack_from("<H", b.dic, pos)[0]
        ent = np.frombuffer(b.dic, dtype="<u2", count=cnt * 4, offset=pos + 2).reshape(cnt, 4)
        pages.append(ent)
        pos += 2 + cnt * 8
    return pages


def decode_page_frame(b, pages, i):
    w, h = int(b.width[i]), int(b.height[i])
    s = b.stream(i)
    ent = pages[int(b.page[i])]
    stride = (w + 3) // 4
    px = ent[s].reshape(h, stride * 4)[:, :w]
    return px


def decode_span_frame(b, i):
    """Page-less frame: per row [first_x u16][last_x u16][pixels first..last]; last_x == 0xFFFF = empty row."""
    w, h = int(b.width[i]), int(b.height[i])
    s = b.stream(i)
    px = np.full((h, w), KEY, dtype=np.uint16)
    p = 0
    for y in range(h):
        a, e = int(s[p]), int(s[p + 1]); p += 2
        if e == 0xFFFF:
            continue
        px[y, a:e + 1] = s[p:p + e - a + 1]
        p += e - a + 1
    assert p == len(s)
    return px


def decode_frame(b, pages, i):
    if b.page[i] == 0xFFFF:
        return decode_span_frame(b, i)
    return decode_page_frame(b, pages, i)


def to_rgba(px, scale=1):
    rgb = rgb565_to_rgb8(px)
    a = np.where(px == KEY, 0, 255).astype(np.uint8)[..., None]
    rgba = np.concatenate([rgb, a], axis=-1)
    if scale > 1:
        rgba = rgba.repeat(scale, axis=0).repeat(scale, axis=1)
    return rgba


if __name__ == "__main__":
    b = Bank()
    pages = load_pages(b)
    out = sys.argv[1]
    scale = int(os.environ.get("SCALE", "1"))
    for a in sys.argv[2:]:
        i = int(a)
        px = decode_frame(b, pages, i)
        p = os.path.join(out, f"frame_{i}.png")
        write_png(p, to_rgba(px, scale))
        print("frame", i, px.shape, "transparent", (px == KEY).mean().round(3), "->", p)
        if px.size <= 200:
            for row in px:
                print("   ", " ".join(f"{v:04x}" for v in row))
