"""FACE: decode RLE rows from a given header end and report row widths + bytes after the mask.
python probe_face.py <map> <start_of_rows> <width> <height>"""
import sys
from rhp_chunks import load_chunks, rhp_path, hexdump
m, start, w, h = sys.argv[1], int(sys.argv[2], 0), int(sys.argv[3]), int(sys.argv[4])
ver, b = load_chunks(rhp_path(m))["FACE"]
pos = start; rowbytes = (w + 7) // 8; bad = 0
for r in range(h):
    n = b[pos]; p = pos + 1; end = p + n; out = bytearray()
    while p < end:
        c = b[p]; p += 1
        if c & 0x80:
            out += bytes([b[p]]) * (c & 0x7f); p += 1
        else:
            out += b[p:p + c]; p += c
    if len(out) != rowbytes or p != end:
        bad += 1
        if bad < 4: print("row", r, "decoded", len(out), "expected", rowbytes, "p-end", p - end)
    pos = end
print("rows done at", hex(pos), "bad rows", bad, "packed total", pos - start)
print(hexdump(b, pos, 96))
