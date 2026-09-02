"""Reference parser for DUTY missions under the layouts recorded in docs/formats/rhm.md (generic; no game bytes).

Usage: python rhm_full.py <dir-or-file> [--stats] [--json FILE]
Parses every chunk of every file, asserts exact consumption and prints a summary or failures.
"""
import collections
import glob
import json
import os
import struct
import sys

from chunkdump import children
from rhm_probe import R, header, opt_name, polygon


def sub_chunks(r):
    """Read `u16 count` then that many tag/size/version chunks; returns [(tag, version, body)]."""
    n = r.u16()
    out = []
    for _ in range(n):
        tag = r.raw(4).decode("latin-1")
        size = r.u32()
        ver = r.u32()
        out.append((tag, ver, r.raw(size - 4)))
    return out


def parse_foot(b):
    r = R(b)
    d = dict(map_id=r.u32(), variant=r.u32(), map=r.pstr(), mission_id=r.u32())
    assert r.done()
    return d


def pouf_entry_start(b, p):
    """True when a plausible entry (two printable pstrings) starts at p."""
    if p + 4 > len(b):
        return False
    n1 = struct.unpack_from("<H", b, p)[0]
    if not 1 <= n1 <= 64 or p + 2 + n1 + 2 > len(b):
        return False
    s1 = b[p + 2 : p + 2 + n1]
    if not all(32 <= c < 127 for c in s1):
        return False
    q = p + 2 + n1
    n2 = struct.unpack_from("<H", b, q)[0]
    if not 1 <= n2 <= 64 or q + 2 + n2 > len(b):
        return False
    return all(32 <= c < 127 for c in b[q + 2 : q + 2 + n2])


def parse_pouf(b):
    r = R(b)
    n = r.u16()
    out = []
    for i in range(n):
        assert pouf_entry_start(b, r.p), f"pouf entry {i} at {r.p:#x}"
        sprite = r.pstr()
        label = r.pstr()
        start = r.p
        end = len(b)
        if i + 1 < n:
            p = start
            while p < len(b) and not pouf_entry_start(b, p):
                p += 1
            end = p
        body = b[start:end]
        r.p = end
        out.append(dict(sprite=sprite, label=label, body=body.hex()))
    assert r.done()
    return out


def rec_scot(r):
    d = header(r)
    d["B"] = r.u32()
    d["flags10"] = r.raw(10).hex()
    d["name"] = opt_name(r)
    d["trailer"] = r.u8()
    return d


def rec_borg(r):
    d = header(r)
    d["B"] = r.u32()
    d["profile"] = r.u32()
    d["u8a"] = r.u8()
    d["u32c"] = r.u32()
    d["u32d"] = r.u32()
    d["u32e"] = r.u32()
    n = r.u16()
    d["members"] = [r.u16() for _ in range(n)]
    d["rail"] = r.i16()
    d["i16b"] = r.i16()
    d["name"] = opt_name(r)
    return d


def rec_oile(r):
    d = header(r)
    d["B"] = r.u32()
    d["profile"] = r.u32()
    d["i16a"] = r.i16()
    d["i16b"] = r.i16()
    d["u16c"] = r.u16()
    if d["profile"] == 1:
        lists = []
        for _ in range(10):
            m = r.u16()
            lists.append([r.u16() for _ in range(m)])
        d["lists"] = lists
    d["name"] = opt_name(r)
    return d


def rec_toto(r):
    d = header(r)
    d["B"] = r.u32()
    d["profile"] = r.u32()
    d["i16a"] = r.i16()
    d["i16b"] = r.i16()
    d["name"] = opt_name(r)
    return d


def rec_boom(r):
    d = dict(x=r.u16(), y=r.u16(), i16a=r.i16(), u16b=r.u16(), u16c=r.u16(), u16d=r.u16(), u16e=r.u16(), P=r.i16(), Q=r.u16(), R=r.u16())
    d["sprite"] = r.pstr()
    d["label"] = r.pstr()
    d["X"] = r.u32()
    d["x2"], d["y2"], d["Q2"], d["R2"] = r.u16(), r.u16(), r.u16(), r.u16()
    d["poly"] = polygon(r)
    d["u8e"] = r.u8()
    d["name"] = opt_name(r)
    return d


RECS = {"SCOT": rec_scot, "BORG": rec_borg, "OILE": rec_oile, "TOTO": rec_toto, "BOOM": rec_boom}


def parse_boyz(b):
    r = R(b)
    groups = []
    for tag, ver, gb in sub_chunks(r):
        g = R(gb)
        n = g.u16()
        recs = []
        if tag != "MEOW":
            fn = RECS[tag]
            recs = [fn(g) for _ in range(n)]
        assert g.done(), f"{tag} left {len(gb)-g.p}"
        groups.append(dict(tag=tag, version=ver, count=n, records=recs))
    assert r.done()
    return groups


def parse_zorg(b):
    r = R(b)
    n = r.u16()
    out = []
    for _ in range(n):
        a, c = r.u16(), r.u16()
        h = header(r)
        h["a"], h["b"] = a, c
        out.append(h)
    assert r.done()
    return out


def parse_hirn(b):
    r = R(b)
    out = {}
    for tag, ver, gb in sub_chunks(r):
        g = R(gb)
        n = g.u16()
        if tag == "HOLE":
            recs = [dict(x=g.u16(), y=g.u16(), Q=g.u16(), R=g.u16(), dir=g.u16()) for _ in range(n)]
        elif tag == "BUSH":
            recs = [dict(x=g.u16(), y=g.u16(), Q=g.u16(), R=g.u16()) for _ in range(n)]
        elif tag == "POW ":
            recs = [header(g) for _ in range(n)]
        elif tag == "NLIP":
            recs = []
            for _ in range(n):
                d = dict(u32=g.u32(), poly=polygon(g))
                m = g.u16()
                d["points"] = [dict(x=g.u16(), y=g.u16(), Q=g.u16(), R=g.u16(), flag=g.u8(), val=g.u16()) for _ in range(m)]
                recs.append(d)
        else:
            raise ValueError(tag)
        assert g.done(), f"{tag} left {len(gb)-g.p}"
        out[tag] = dict(version=ver, records=recs)
    assert r.done()
    return out


CMD_ARGS = {0: 0, 1: 0, 7: 0, 9: 0, 0x0A: 0, 0x0B: 0, 0x0C: 0, 0x0E: 0, 2: 2, 3: 2, 4: 2, 8: 2, 0x0D: 2, 0x0F: 2, 5: 4, 0x81: 4, 0x82: 4}


def parse_commands(blk):
    r = R(blk)
    out = []
    while not r.done():
        c = r.u8()
        n = CMD_ARGS[c]
        out.append((c, r.raw(n).hex()))
    return out


def parse_program(blob):
    """Waypoint command program: u16 ntab, ntab*(u8 id, u16 off); table = u16 nseg, nseg*(u8 pct, u16 off); seg = u16 len, bytes."""
    r = R(blob)
    ntab = r.u16()
    tabs = [(r.u8(), r.u16()) for _ in range(ntab)]
    tables = []
    for tid, toff in tabs:
        assert toff == r.p, (toff, r.p)
        nseg = r.u16()
        segs = [(r.u8(), r.u16()) for _ in range(nseg)]
        blocks = []
        for pct, off in segs:
            assert off == r.p, (off, r.p)
            ln = r.u16()
            blk = r.raw(ln)
            blocks.append((pct, blk.hex(), parse_commands(blk)))
        tables.append(dict(id=tid, blocks=blocks))
    assert r.done(), f"program left {len(blob)-r.p}"
    return dict(tables=tables)


def parse_rail(b):
    r = R(b)
    n = r.u16()
    rails = []
    for _ in range(n):
        npts = r.u16()
        pts = []
        for _ in range(npts):
            d = dict(x=r.u16(), y=r.u16(), Q=r.u16(), R=r.u16(), kind=r.u8())
            ln = r.u16()
            payload = r.raw(ln)
            if d["kind"] == 1:
                d["name"] = payload.decode("latin-1")
            else:
                assert d["kind"] == 0
                d["program"] = parse_program(payload) if payload else None
            pts.append(d)
        rails.append(pts)
    assert r.done()
    return rails


def parse_skro(b):
    r = R(b)
    n = r.u16()
    out = []
    for _ in range(n):
        h = header(r)
        h["flags5"] = r.raw(5).hex()
        h["name"] = opt_name(r)
        out.append(h)
    assert r.done()
    return out


def parse_ting(b):
    r = R(b)
    n = r.u16()
    out = []
    for _ in range(n):
        e = {}
        tag = r.raw(4).decode("latin-1")
        assert tag == "FLIM", tag
        size, ver = r.u32(), r.u32()
        f = R(r.raw(size - 4))
        m = f.u16()
        items = []
        for _ in range(m):
            it = dict(sprite=f.pstr(), anim=f.pstr(), dx=f.i16(), dy=f.i16(), u16=f.u16(), b3=f.raw(3).hex(), poly=polygon(f))
            items.append(it)
        assert f.done()
        e["flim"] = dict(version=ver, items=items)
        tag = r.raw(4).decode("latin-1")
        assert tag == "WOAW", tag
        size, ver = r.u32(), r.u32()
        w = R(r.raw(size - 4))
        wc = w.u16()
        e["woaw"] = dict(version=ver, count=wc, rest=w.raw(len(w.b) - w.p).hex())
        e["poly"] = polygon(r)
        e["x"], e["y"] = r.u16(), r.u16()
        e["u16a"], e["u32b"], e["u16c"], e["u32d"], e["i16e"] = r.u16(), r.u32(), r.u16(), r.u32(), r.i16()
        out.append(e)
    assert r.done(), f"ting left {len(b)-r.p}"
    return out


def parse_gulp(b):
    r = R(b)
    n = r.u16()
    pts = [dict(x=r.u16(), y=r.u16(), Q=r.u16(), R=r.u16()) for _ in range(n)]
    m = r.u16()
    polys = []
    for _ in range(m):
        d = dict(u8a=r.u8())
        k = r.u16()
        d["points"] = [(r.u16(), r.u16()) for _ in range(k)]
        d["u8b"], d["Q"], d["R"] = r.u8(), r.u16(), r.u16()
        d["name"] = opt_name(r)
        polys.append(d)
    assert r.done()
    return dict(points=pts, polygons=polys)


def parse_cave(b):
    r = R(b)
    n = r.u16()
    out = []
    for _ in range(n):
        m = r.u16()
        out.append(dict(ids=[r.u16() for _ in range(m)], flag=r.u8()))
    assert r.done()
    return out


PARSERS = dict(FOOT=parse_foot, POUF=parse_pouf, BOYZ=parse_boyz, ZORG=parse_zorg, HIRN=parse_hirn, RAIL=parse_rail, SKRO=parse_skro, TING=parse_ting, GULP=parse_gulp, CAVE=parse_cave)


def parse_file(data):
    root, ver, ch = children(data)
    out = {}
    for tag, v, off, body in ch:
        out[tag] = dict(version=v, data=PARSERS[tag](body))
    return out


def main():
    target = sys.argv[1]
    files = sorted(glob.glob(os.path.join(target, "*.rhm"))) if os.path.isdir(target) else [target]
    stats = collections.defaultdict(collections.Counter)
    cmds = collections.Counter()
    ok = 0
    for f in files:
        data = open(f, "rb").read()
        try:
            m = parse_file(data)
            ok += 1
        except Exception as e:  # noqa: BLE001
            print(os.path.basename(f), "FAIL", repr(e))
            continue
        if "--json" in sys.argv:
            json.dump(m, open(sys.argv[sys.argv.index("--json") + 1], "w"), indent=1)
        for pts in m["RAIL"]["data"]:
            for p in pts:
                if p.get("program"):
                    stats[("RAIL", "ntab")][len(p["program"]["tables"])] += 1
                    for t in p["program"]["tables"]:
                        stats[("RAIL", "tab id")][t["id"]] += 1
                        stats[("RAIL", "pcts")][tuple(b[0] for b in t["blocks"])] += 1
                        for pct, blk, cl in t["blocks"]:
                            for c, a in cl:
                                cmds[(c, a if len(a) <= 8 else len(a))] += 1
        for g in m["BOYZ"]["data"]:
            for rec in g["records"]:
                for k, v in rec.items():
                    if isinstance(v, (int, str)) and k not in ("x", "y", "name", "sprite", "label", "flags10", "x2", "y2"):
                        stats[(g["tag"], k)][v] += 1
        for e in m["TING"]["data"]:
            for k in ("u16a", "u32b", "u16c", "u32d", "i16e"):
                stats[("TING", k)][e[k]] += 1
            stats[("TING", "woaw")][e["woaw"]["count"]] += 1
        for z in m["ZORG"]["data"]:
            stats[("ZORG", "a")][z["a"]] += 1
            stats[("ZORG", "b-flags")][(z["b"], z["flags"])] += 1
        for s in m["SKRO"]["data"]:
            stats[("SKRO", "flags5")][s["flags5"]] += 1
            stats[("SKRO", "flags")][s["flags"]] += 1
        for c in m["CAVE"]["data"]:
            stats[("CAVE", "flag")][c["flag"]] += 1
            stats[("CAVE", "len")][len(c["ids"])] += 1
        for tag, sub in m["HIRN"]["data"].items():
            stats[("HIRN", tag)][len(sub["records"])] += 1
        stats[("FOOT", "variant")][m["FOOT"]["data"]["variant"]] += 1
    print(f"{ok}/{len(files)} files parsed")
    if "--stats" in sys.argv:
        for k in sorted(stats, key=str):
            v = stats[k]
            print(k, len(v), sorted(v.items(), key=lambda kv: -kv[1])[:14])
        print("command blocks:", len(cmds), cmds.most_common(30))


if __name__ == "__main__":
    main()
