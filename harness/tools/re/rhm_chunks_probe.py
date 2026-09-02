"""Parse the non-actor chunks of DUTY missions under the current hypotheses (generic; no game bytes).

Usage: python rhm_chunks_probe.py <dir> [TAG ...]
"""
import collections
import glob
import os
import struct
import sys

from chunkdump import children
from rhm_probe import R, header


def hexs(b):
    return b.hex()


def parse_rail(body):
    r = R(body)
    n = r.u16()
    rails = []
    for _ in range(n):
        npts = r.u16()
        pts = []
        for _ in range(npts):
            x, y = r.u16(), r.u16()
            five = r.raw(5)
            blen = r.u16()
            blob = r.raw(blen)
            pts.append((x, y, five.hex(), blob.hex()))
        rails.append(pts)
    assert r.done(), f"rail left {len(body)-r.p}"
    return rails


def parse_gulp(body):
    r = R(body)
    n = r.u16()
    pts = [(r.u16(), r.u16(), r.u16(), r.u16()) for _ in range(n)]
    m = r.u16()
    polys = []
    for _ in range(m):
        a = r.u8()
        k = r.u16()
        poly = [(r.u16(), r.u16()) for _ in range(k)]
        tail = r.raw(5)
        name = r.pstr() if r.u8() else None
        polys.append((a, poly, tail.hex(), name))
    assert r.done(), f"gulp left {len(body)-r.p}"
    return pts, polys


def parse_skro(body):
    r = R(body)
    n = r.u16()
    out = []
    for _ in range(n):
        h = header(r)
        six = r.raw(6)
        h["six"] = six.hex()
        h["name"] = r.pstr()
        out.append(h)
    assert r.done(), f"skro left {len(body)-r.p}"
    return out


def parse_zorg(body):
    r = R(body)
    n = r.u16()
    out = []
    for _ in range(n):
        a, b = r.u16(), r.u16()
        h = header(r)
        h["a"], h["b"] = a, b
        out.append(h)
    assert r.done(), f"zorg left {len(body)-r.p}"
    return out


def parse_hirn(body):
    r = R(body)
    n = r.u16()
    groups = {}
    for _ in range(n):
        tag = r.raw(4).decode("latin-1")
        size = r.u32()
        ver = r.u32()
        gb = r.raw(size - 4)
        groups[tag] = (ver, gb)
    assert r.done()
    return groups


def main():
    files = sorted(glob.glob(os.path.join(sys.argv[1], "*.rhm")))
    want = set(sys.argv[2:])
    cmdstats = collections.Counter()
    prefixes = collections.Counter()
    fives = collections.Counter()
    for f in files:
        name = os.path.basename(f)
        data = open(f, "rb").read()
        _, _, ch = children(data)
        bodies = {t: (v, b) for t, v, o, b in ch}
        if "RAIL" in want:
            try:
                rails = parse_rail(bodies["RAIL"][1])
                for pts in rails:
                    for x, y, five, blob in pts:
                        fives[five] += 1
                        prefixes[blob[:20]] += 1
                        if blob:
                            cmdstats[blob[20:]] += 1
                print(name, "RAIL OK", len(rails), "rails; sizes", [len(p) for p in rails][:20])
            except Exception as e:  # noqa: BLE001
                print(name, "RAIL FAIL", e)
        if "GULP" in want:
            try:
                pts, polys = parse_gulp(bodies["GULP"][1])
                print(name, "GULP OK", len(pts), "points,", len(polys), "polys:", [(a, len(p), t, nm) for a, p, t, nm in polys][:6])
            except Exception as e:  # noqa: BLE001
                print(name, "GULP FAIL", e)
        if "SKRO" in want:
            try:
                out = parse_skro(bodies["SKRO"][1])
                print(name, "SKRO OK", [(o["x"], o["y"], o["dir"], o["flags"], o["P"], o["Q"], o["R"], o["six"], o["name"]) for o in out][:4])
            except Exception as e:  # noqa: BLE001
                print(name, "SKRO FAIL", e)
        if "ZORG" in want:
            try:
                out = parse_zorg(bodies["ZORG"][1])
                print(name, "ZORG OK", [(o["a"], o["b"], o["x"], o["y"], o["dir"], o["flags"], o["P"], o["Q"], o["R"]) for o in out][:8])
            except Exception as e:  # noqa: BLE001
                print(name, "ZORG FAIL", e)
        if "CAVE" in want:
            b = bodies["CAVE"][1]
            print(name, "CAVE", struct.unpack_from("<H", b)[0], len(b), hexs(b[2:]))
        if "HIRN" in want:
            g = parse_hirn(bodies["HIRN"][1])
            print(name, "HIRN", {k: (v[0], len(v[1]), struct.unpack_from("<H", v[1])[0] if len(v[1]) >= 2 else None) for k, v in g.items()})
            for k in ("BUSH", "NLIP"):
                if k in g and len(g[k][1]) > 2:
                    print("   ", k, hexs(g[k][1][:200]))
        if "TING" in want:
            b = bodies["TING"][1]
            if len(b) > 2:
                print(name, "TING", len(b), hexs(b))
        if "FOOT" in want:
            b = bodies["FOOT"][1]
            print(name, "FOOT", hexs(b))
    if "RAIL" in want:
        print("five-byte fields:", fives.most_common(12))
        print("blob prefixes:", prefixes.most_common(12))
        print("command streams:", len(cmdstats), cmdstats.most_common(40))


if __name__ == "__main__":
    main()
