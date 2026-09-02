"""Walk FACE records: header of unknown length L ending in (u16 w, u16 h, u16 packed), then packed RLE rows,
then 2 bytes. Search L in a window; report header bytes."""
import struct, sys
from rhp_chunks import load_chunks, rhp_path, map_size

def decode_row(b, pos, rowbytes):
    n = b[pos]; p = pos + 1; end = p + n; out = bytearray()
    while p < end:
        c = b[p]; p += 1
        if c & 0x80:
            out += bytes([b[p]]) * (c & 0x7f); p += 1
        else:
            out += b[p:p + c]; p += c
    if p != end or len(out) != rowbytes: return None
    return bytes(out), end

def try_record(b, start, L):
    if start + L + 6 > len(b): return None
    w, h, packed = struct.unpack_from("<HHH", b, start + L)
    if w == 0 or h == 0 or packed == 0 or w > 4000 or h > 4000: return None
    pos = start + L + 6; rowbytes = (w + 7) // 8; rows = []
    if pos + packed > len(b): return None
    for r in range(h):
        d = decode_row(b, pos, rowbytes)
        if d is None: return None
        rows.append(d[0]); pos = d[1]
    if pos != start + L + 6 + packed: return None
    return w, h, packed, rows, pos

def walk(b, maxrec=None):
    count = struct.unpack_from("<I", b, 0)[0]; pos = 4; recs = []
    for i in range(count):
        found = None
        for L in range(4, 200):
            r = try_record(b, pos, L)
            if r: found = (L, r); break
        if not found:
            print("record", i, "not found at", hex(pos)); break
        L, (w, h, packed, rows, end) = found
        recs.append((pos, L, b[pos:pos + L], w, h, rows, b[end:end + 2]))
        pos = end + 2
        if maxrec and len(recs) >= maxrec: break
    return count, recs, pos

if __name__ == "__main__":
    m = sys.argv[1]; nshow = int(sys.argv[2]) if len(sys.argv) > 2 else 12
    ver, b = load_chunks(rhp_path(m))["FACE"]
    count, recs, pos = walk(b)
    print(m, map_size(m), "count", count, "parsed", len(recs), "end", pos, "of", len(b))
    for (off, L, hdr, w, h, rows, tail) in recs[:nshow]:
        print(f"{off:07x} L={L:3d} w={w:4d} h={h:4d} tail={tail.hex()} hdr={hdr.hex(' ')}")
