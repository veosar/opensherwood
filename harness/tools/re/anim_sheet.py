"""Contact sheets of `.rhs` animations, for identifying actions and directions by eye.

usage:
  anim_sheet.py <file.rhs> <out_dir> [--seq I] [--first A] [--last B] [--cols 16] [--scale S]
  anim_sheet.py <file.rhs> <out_dir> --strip N [N ...] [--scale S]

Sheet mode writes `<out_dir>/<stem>_sheet_<A>_<B>.png`: the first frame of animations [A, B) in a grid of
`cols` columns (16 = one direction per column, one block per row); each cell is labelled with the animation
index (tiny 3x5 digits) and a cross marks the animation origin. Strip mode writes
`<out_dir>/<stem>_anim_<N>.png` with every frame of animation N (label = frame position and low half of the
duration word). Layout facts used (docs/formats/sprite-animations.md): a frame is drawn with its top-left at
`origin + (anchor - 150)`; colour 0x07C0 is transparent and 0x001F a shadow.

Generic: reads only the files named on the command line and OPENSHERWOOD_GAME_DIR; embeds no game bytes;
never writes into the repository unless told to.
"""
import argparse
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
import numpy as np
from spritebank import Bank, rgb565_to_rgb8, write_png
from sprite_render import decode_frame, load_pages

KEY = 0x07C0
SHADOW = 0x001F
ORIGIN = 150
BG = (96, 96, 96, 255)
GRID = (40, 40, 40, 255)
INK = (255, 255, 0, 255)
CROSS = (255, 64, 64, 255)

# 3x5 digit glyphs, rows top to bottom, 1 = ink.
DIGITS = {
    "0": ["111", "101", "101", "101", "111"],
    "1": ["010", "110", "010", "010", "111"],
    "2": ["111", "001", "111", "100", "111"],
    "3": ["111", "001", "111", "001", "111"],
    "4": ["101", "101", "111", "001", "001"],
    "5": ["111", "100", "111", "001", "111"],
    "6": ["111", "100", "111", "101", "111"],
    "7": ["111", "001", "001", "001", "001"],
    "8": ["111", "101", "111", "101", "111"],
    "9": ["111", "101", "111", "001", "111"],
    "-": ["000", "000", "111", "000", "000"],
    ":": ["000", "010", "000", "010", "000"],
    " ": ["000", "000", "000", "000", "000"],
}


def parse_rhs(path):
    """Minimal `.rhs` reader (docs/formats/sprites.md). Returns a list of sequences."""
    d = open(path, "rb").read()
    _gen, nseq = struct.unpack_from("<IH", d, 0)
    p = 6
    seqs = []
    for _ in range(nseq):
        name = d[p:p + 32].split(b"\0")[0].decode("latin-1")
        nanim, w, h, u26, u2a = struct.unpack_from("<HHHII", d, p + 32)
        p += 46
        anims = []
        for _ in range(nanim):
            n, u02, u04, u08, u0c = struct.unpack_from("<HHIIH", d, p)
            p += 14
            frames = []
            for _ in range(n):
                fi, dur, ax, ay, u = struct.unpack_from("<IIHHH", d, p)
                p += 14
                frames.append(dict(frame=fi, duration=dur, anchor=(ax, ay), unknown=u))
            anims.append(dict(u02=u02, u04=u04, u08=u08, u0c=u0c, frames=frames))
        seqs.append(dict(name=name, w=w, h=h, u26=u26, u2a=u2a, anims=anims))
    if p != len(d):
        sys.exit(f"{path}: {len(d) - p} trailing bytes")
    return seqs


def text(img, x, y, s, scale=1):
    for ch in s:
        g = DIGITS.get(ch, DIGITS[" "])
        for r, row in enumerate(g):
            for c, bit in enumerate(row):
                if bit == "1":
                    img[y + r * scale:y + (r + 1) * scale, x + c * scale:x + (c + 1) * scale] = INK
        x += 4 * scale


def frame_rgba(px):
    rgb = rgb565_to_rgb8(px)
    a = np.where(px == KEY, 0, 255).astype(np.uint8)
    shadow = px == SHADOW
    rgb[shadow] = 0
    a[shadow] = 110
    return np.concatenate([rgb, a[..., None]], axis=-1)


def blit(img, rgba, x, y):
    h, w, _ = rgba.shape
    H, W, _ = img.shape
    x0, y0 = max(x, 0), max(y, 0)
    x1, y1 = min(x + w, W), min(y + h, H)
    if x1 <= x0 or y1 <= y0:
        return
    src = rgba[y0 - y:y1 - y, x0 - x:x1 - x]
    dst = img[y0:y1, x0:x1]
    a = src[..., 3:4].astype(np.uint16)
    dst[..., :3] = ((src[..., :3] * a + dst[..., :3] * (255 - a)) // 255).astype(np.uint8)


def cell_extent(bank, refs):
    """Bounding box of frame rectangles placed at anchor-150, as (minx, miny, maxx, maxy)."""
    minx = miny = 10**9
    maxx = maxy = -(10**9)
    for r in refs:
        w, h = int(bank.width[r["frame"]]), int(bank.height[r["frame"]])
        ax, ay = r["anchor"][0] - ORIGIN, r["anchor"][1] - ORIGIN
        minx, miny = min(minx, ax), min(miny, ay)
        maxx, maxy = max(maxx, ax + w), max(maxy, ay + h)
    return minx, miny, maxx, maxy


def render_cells(bank, pages, cells, cols, scale, labels):
    """cells: list of frame refs (one per cell). Returns the RGBA sheet."""
    minx, miny, maxx, maxy = cell_extent(bank, cells)
    cw, ch = maxx - minx + 2, maxy - miny + 8
    rows = (len(cells) + cols - 1) // cols
    img = np.zeros((rows * ch * scale + 1, cols * cw * scale + 1, 4), dtype=np.uint8)
    img[...] = BG
    for i, r in enumerate(cells):
        cx, cy = (i % cols) * cw * scale, (i // cols) * ch * scale
        img[cy, :] = GRID
        img[:, cx] = GRID
        rgba = frame_rgba(decode_frame(bank, pages, r["frame"]))
        if scale > 1:
            rgba = rgba.repeat(scale, axis=0).repeat(scale, axis=1)
        ox, oy = cx + (-minx + 1) * scale, cy + (-miny + 7) * scale
        blit(img, rgba, ox + (r["anchor"][0] - ORIGIN) * scale, oy + (r["anchor"][1] - ORIGIN) * scale)
        img[oy, ox - 3:ox + 4] = CROSS
        img[oy - 3:oy + 4, ox] = CROSS
        text(img, cx + 2, cy + 1, labels[i])
    img[-1, :] = GRID
    img[:, -1] = GRID
    return img


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("rhs")
    ap.add_argument("out_dir")
    ap.add_argument("--seq", type=int, default=0)
    ap.add_argument("--first", type=int, default=0)
    ap.add_argument("--last", type=int, default=None)
    ap.add_argument("--cols", type=int, default=16)
    ap.add_argument("--scale", type=int, default=1)
    ap.add_argument("--strip", type=int, nargs="*")
    ap.add_argument("--blocks", help="A:B = one row per 16-animation block A..B-1 (all frames of one direction)")
    ap.add_argument("--dir", type=int, default=4, help="direction column 0..15 used with --blocks")
    a = ap.parse_args()

    seq = parse_rhs(a.rhs)[a.seq]
    anims = seq["anims"]
    stem = os.path.splitext(os.path.basename(a.rhs))[0].replace(" ", "_")
    os.makedirs(a.out_dir, exist_ok=True)
    bank = Bank()
    pages = load_pages(bank)

    if a.strip:
        for n in a.strip:
            refs = anims[n]["frames"]
            labels = [f"{i}:{r['duration'] & 0xFFFF}" for i, r in enumerate(refs)]
            img = render_cells(bank, pages, refs, len(refs), a.scale, labels)
            p = os.path.join(a.out_dir, f"{stem}_anim_{n}.png")
            write_png(p, img)
            print(f"anim {n}: {len(refs)} frames u0c={anims[n]['u0c']} u04/u08={anims[n]['u04']}/{anims[n]['u08']} -> {p}")
        return

    if a.blocks:
        b0, b1 = (int(x) for x in a.blocks.split(":"))
        b1 = min(b1, len(anims) // 16)
        rows = [anims[b * 16 + a.dir] for b in range(b0, b1)]
        cols = max(len(r["frames"]) for r in rows)
        cells, labels = [], []
        for b, an in zip(range(b0, b1), rows):
            for i in range(cols):
                if i < len(an["frames"]):
                    cells.append(an["frames"][i])
                    labels.append(f"{b}:{an['u0c']}" if i == 0 else f"{i}:{an['frames'][i]['duration'] & 0xFFFF}")
                else:
                    cells.append(dict(frame=0, anchor=(ORIGIN, ORIGIN), duration=0))
                    labels.append("")
        img = render_cells(bank, pages, cells, cols, a.scale, labels)
        p = os.path.join(a.out_dir, f"{stem}_blocks_{b0}_{b1}_dir{a.dir}.png")
        write_png(p, img)
        print(f"{seq['name']!r}: blocks {b0}..{b1} direction {a.dir}, {img.shape[1]}x{img.shape[0]} -> {p}")
        return

    last = len(anims) if a.last is None else min(a.last, len(anims))
    cells = [anims[i]["frames"][0] for i in range(a.first, last)]
    labels = [str(i) for i in range(a.first, last)]
    img = render_cells(bank, pages, cells, a.cols, a.scale, labels)
    p = os.path.join(a.out_dir, f"{stem}_sheet_{a.first}_{last}.png")
    write_png(p, img)
    print(f"{seq['name']!r}: animations {a.first}..{last} of {len(anims)}, {img.shape[1]}x{img.shape[0]} -> {p}")


if __name__ == "__main__":
    main()
