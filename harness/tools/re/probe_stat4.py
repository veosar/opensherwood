import struct, sys
from rhp_chunks import load_chunks, rhp_path, map_size, hexdump
m = sys.argv[1]
ver, b = load_chunks(rhp_path(m))["STAT"]; W, H = map_size(m)
ng = struct.unpack_from("<H", b, 0)[0]; pos = 2
print(m, "ng", ng, "len", len(b))
for g in range(ng):
    cnt = struct.unpack_from("<H", b, pos)[0]; print(f" group {g} @{pos:06x} count {cnt}"); pos += 2
    for i in range(cnt):
        if pos + 8 > len(b): print("  EOF at", hex(pos)); break
        R = b[pos]; flags = struct.unpack_from("<I", b, pos + 1)[0]; tag = b[pos + 5]; npts = struct.unpack_from("<H", b, pos + 6)[0]
        if npts > 1000 or flags > 16:
            print(f"  poly {i} @{pos:06x} suspicious: {b[pos:pos+24].hex(' ')}"); print(hexdump(b, pos - 16, 64)); sys.exit()
        pos += 8 + 4 * npts
    print(f"  ended @{pos:06x}")
print("final", hex(pos), "remaining", len(b) - pos, b[pos:].hex(' '))
