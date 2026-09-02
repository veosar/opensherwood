"""FACE deterministic model: u16 count; record = u16 nrefs, nrefs x u16, u16 z, u8 kind, popcount(kind&3) polylines
(u8 id, u16 n, n x (u16,u16), u8 id2), u16 x, u16 y, u16 w, u16 h, u16 packed, RLE rows."""
import struct, sys, collections
from rhp_chunks import load_chunks, rhp_path, map_size, MAPS

def decode_rows(b, pos, w, h):
    rowbytes = (w + 7) // 8; rows = []
    for r in range(h):
        n = b[pos]; p = pos + 1; end = p + n; out = bytearray()
        while p < end:
            c = b[p]; p += 1
            if c & 0x80: out += bytes([b[p]]) * (c & 0x7f); p += 1
            else: out += b[p:p + c]; p += c
        if p != end or len(out) != rowbytes: raise ValueError(f"row {r} bad at {pos:#x}")
        rows.append(bytes(out)); pos = end
    return rows, pos

def parse_face(b, W=None, H=None):
    count = struct.unpack_from("<H", b, 0)[0]; pos = 2; recs = []
    for i in range(count):
        nrefs = struct.unpack_from("<H", b, pos)[0]; pos += 2
        refs = struct.unpack_from("<%dH" % nrefs, b, pos); pos += 2 * nrefs
        z = struct.unpack_from("<H", b, pos)[0]; pos += 2
        kind = b[pos]; pos += 1
        polys = []
        for k in range(bin(kind & 3).count("1")):
            id1 = b[pos]; n = struct.unpack_from("<H", b, pos + 1)[0]; pos += 3
            pts = [struct.unpack_from("<HH", b, pos + 4 * j) for j in range(n)]; pos += 4 * n
            id2 = b[pos]; pos += 1; polys.append((id1, pts, id2))
        x, y, w, h, packed = struct.unpack_from("<HHHHH", b, pos); pos += 10
        rows, end = decode_rows(b, pos, w, h)
        if end != pos + packed: raise ValueError(f"rec {i} packed mismatch")
        pos = end; recs.append((nrefs, refs, z, kind, polys, x, y, w, h, rows))
    return recs, pos

if __name__ == "__main__":
    for m in sys.argv[1:] or MAPS:
        b = load_chunks(rhp_path(m))["FACE"][1]; W, H = map_size(m)
        try:
            recs, pos = parse_face(b)
        except Exception as e:
            print(m, "ERR", e); continue
        kinds = collections.Counter(r[3] for r in recs); zs = collections.Counter(r[2] for r in recs); nrefs = collections.Counter(r[0] for r in recs)
        oob = sum(1 for r in recs if r[5] + r[7] > W or r[6] + r[8] > H)
        refmax = max((max(r[1]) for r in recs if r[1]), default=0)
        print(f"{m:13s} n={len(recs)} {'OK' if pos == len(b) else 'MISMATCH'} kinds={dict(sorted(kinds.items()))} z={dict(sorted(zs.items()))} nrefs={dict(sorted(nrefs.items()))} refmax={refmax} mask-oob={oob}")
