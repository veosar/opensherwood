"""FACE deterministic model (Model T): u32 count; record = u8 kind, popcount(kind&3) polylines
(u8 id, u16 n, n x (u16,u16), u8 id2), u16 x, u16 y, u16 w, u16 h, u16 packed, RLE rows, then trailer
(absent for the last record): if kind & 0x10: u16 nrefs, nrefs x u16, u16 z; else u16 z."""
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

def parse_face(b):
    count = struct.unpack_from("<I", b, 0)[0]; pos = 4; recs = []
    for i in range(count):
        kind = b[pos]; pos += 1; polys = []
        for k in range(bin(kind & 3).count("1")):
            id1 = b[pos]; n = struct.unpack_from("<H", b, pos + 1)[0]; pos += 3
            pts = [struct.unpack_from("<HH", b, pos + 4 * j) for j in range(n)]; pos += 4 * n
            id2 = b[pos]; pos += 1; polys.append((id1, pts, id2))
        x, y, w, h, packed = struct.unpack_from("<HHHHH", b, pos); pos += 10
        rows, end = decode_rows(b, pos, w, h)
        if end != pos + packed: raise ValueError(f"rec {i} packed mismatch")
        pos = end; refs = (); z = None
        if i + 1 < count:
            if kind & 0x10:
                n = struct.unpack_from("<H", b, pos)[0]; pos += 2
                refs = struct.unpack_from("<%dH" % n, b, pos); pos += 2 * n
            z = struct.unpack_from("<H", b, pos)[0]; pos += 2
        recs.append((kind, polys, x, y, w, h, rows, refs, z))
    return recs, pos

if __name__ == "__main__":
    for m in sys.argv[1:] or MAPS:
        b = load_chunks(rhp_path(m))["FACE"][1]; W, H = map_size(m)
        try:
            recs, pos = parse_face(b)
        except Exception as e:
            print(m, "ERR", e); continue
        kinds = collections.Counter(r[0] for r in recs); zs = collections.Counter(r[8] for r in recs); nrefs = collections.Counter(len(r[7]) for r in recs)
        oob = sum(1 for r in recs if r[2] + r[4] > W or r[3] + r[5] > H)
        refmax = max((max(r[7]) for r in recs if r[7]), default=0)
        poly_oob = sum(1 for r in recs for _, pts, _ in r[1] for x, y in pts if x > W or y > H)
        print(f"{m:13s} n={len(recs)} {'OK' if pos == len(b) else 'MISMATCH'} kinds={dict(sorted(kinds.items()))} z={dict(sorted(zs.items(), key=lambda kv: (kv[0] is None, kv[0])))} nrefs={dict(sorted(nrefs.items()))} refmax={refmax} mask-oob={oob} poly-oob={poly_oob}")
