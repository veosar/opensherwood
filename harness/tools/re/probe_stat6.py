"""STAT: parse boundary, segments, obstacles (tag,npts,pts,R,flags) and show what follows."""
import struct, sys
from rhp_chunks import load_chunks, rhp_path, map_size, hexdump, MAPS
def parse(b):
    top = struct.unpack_from("<HH", b, 0); pos = 4
    assert b[pos:pos+5] == b"\0\0\0\0\0"; pos += 5
    tag = b[pos]; n = struct.unpack_from("<H", b, pos + 1)[0]; pos += 3
    boundary = [struct.unpack_from("<HH", b, pos + 4 * k) for k in range(n)]; pos += 4 * n
    assert b[pos] == 0x82; nseg = struct.unpack_from("<H", b, pos + 1)[0]; pos += 3
    segs = [struct.unpack_from("<HHHH", b, pos + 8 * k) for k in range(nseg)]; pos += 8 * nseg
    z1 = struct.unpack_from("<I", b, pos)[0]; nob = struct.unpack_from("<H", b, pos + 4)[0]; z2 = struct.unpack_from("<I", b, pos + 6)[0]; pos += 10
    obst = []
    for i in range(nob):
        tag = b[pos]; n = struct.unpack_from("<H", b, pos + 1)[0]; pos += 3
        pts = [struct.unpack_from("<HH", b, pos + 4 * k) for k in range(n)]; pos += 4 * n
        R = b[pos]; flags = struct.unpack_from("<I", b, pos + 1)[0]; pos += 5
        obst.append((tag, pts, R, flags))
    return top, boundary, segs, (z1, z2), obst, pos
if __name__ == "__main__":
    for m in sys.argv[1:] or MAPS:
        ver, b = load_chunks(rhp_path(m))["STAT"]; W, H = map_size(m)
        top, boundary, segs, zz, obst, pos = parse(b)
        oob = sum(1 for _, pts, _, _ in obst for x, y in pts if x > W or y > H)
        print(f"{m:13s} top={top} boundary={len(boundary)} segs={len(segs)} zz={zz} obst={len(obst)} oob={oob} flags={sorted(set(f for *_, f in obst))} end@{pos:06x}/{len(b):06x}")
        print("   next:", b[pos:pos + 48].hex(' '))
