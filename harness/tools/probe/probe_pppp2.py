"""PPPP second section after the polygons."""
import struct, sys
from rhp_chunks import load_chunks, rhp_path, map_size, hexdump, MAPS
from overlay import pppp
for m in sys.argv[1:] or MAPS:
    ver, b = load_chunks(rhp_path(m))["PPPP"]; W, H = map_size(m)
    polys, pos = pppp(b)
    n2 = struct.unpack_from("<H", b, pos)[0]
    print(m, "polys", len(polys), "section2 @", hex(pos), "n2", n2, "rest", len(b) - pos - 2)
    print(hexdump(b, pos, 64))
