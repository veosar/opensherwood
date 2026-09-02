import sys
from rhp_chunks import load_chunks, rhp_path
from probe_face_walk import walk
for m in sys.argv[1:]:
    ver, b = load_chunks(rhp_path(m))["FACE"]
    count, recs, pos = walk(b)
    shown = 0
    for (off, L, hdr, w, h, rows, tail) in recs:
        if L <= 25 and shown < 14:
            shown += 1
            print(f"{m[:6]} {off:07x} L={L:3d} w={w:4d} h={h:4d} tail={tail.hex()} hdr={hdr.hex(' ')}")
