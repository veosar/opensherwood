"""Walk FACE in all maps; record header stats."""
import struct, collections
from rhp_chunks import load_chunks, rhp_path, map_size, MAPS
from probe_face_walk import walk
for m in MAPS:
    ver, b = load_chunks(rhp_path(m))["FACE"]; W, H = map_size(m)
    count, recs, pos = walk(b)
    first = collections.Counter(h[0] for (_, _, h, *_ ) in recs)
    tails = collections.Counter(t.hex() for (*_, t) in recs)
    Ls = collections.Counter(L for (_, L, *_ ) in recs)
    wmax = max(w for (_, _, _, w, h, _, _) in recs); hmax = max(h for (_, _, _, w, h, _, _) in recs)
    print(m, (W, H), "count", count, "parsed", len(recs), "end", pos, "len", len(b), "first byte", dict(first), "tails", tails.most_common(3), "wmax", wmax, "hmax", hmax, "L", sorted(Ls)[:6], "...", max(Ls))
