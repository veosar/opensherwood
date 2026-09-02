"""Parse SBSCRIPT 1.5 files under the class/function/quad hypothesis and print statistics (generic; no game bytes).

Usage: python scb_probe.py <dir-or-file> [--dump CLASS] [--hist]
"""
import collections
import glob
import os
import struct
import sys

QUAD = 9  # bytes per instruction in version 1.5 (Desperados 1.0 uses 10)


def pstr32(d, p):
    n = struct.unpack_from("<I", d, p)[0]
    return d[p + 4 : p + 4 + n].decode("latin-1"), p + 4 + n


def parse(d):
    assert d[:8] == b"SBSCRIPT"
    ver = struct.unpack_from("<f", d, 8)[0]
    nclasses = struct.unpack_from("<I", d, 12)[0]
    p = 16
    classes = []
    for _ in range(nclasses):
        path, p = pstr32(d, p)
        name, p = pstr32(d, p)
        nvars, sizevars = struct.unpack_from("<II", d, p)
        p += 8
        vars_ = []
        for _ in range(nvars):
            t, tl = d[p], d[p + 1]
            tname = d[p + 2 : p + 2 + tl].decode("latin-1")
            p += 2 + tl
            vn, p = pstr32(d, p)
            off = struct.unpack_from("<I", d, p)[0]
            p += 4
            vars_.append((t, tname, vn, off))
        nfuncs = struct.unpack_from("<I", d, p)[0]
        p += 4
        funcs = []
        for _ in range(nfuncs):
            fn, p = pstr32(d, p)
            f = struct.unpack_from("<6I", d, p)
            p += 24
            funcs.append((fn,) + f)
        nquads = struct.unpack_from("<I", d, p)[0]
        p += 4
        quads = d[p : p + QUAD * nquads]
        p += QUAD * nquads
        classes.append(dict(path=path, name=name, nvars=nvars, sizevars=sizevars, vars=vars_, funcs=funcs, nquads=nquads, quads=quads))
    return ver, classes, p


def main():
    target = sys.argv[1]
    files = sorted(glob.glob(os.path.join(target, "*.scb"))) if os.path.isdir(target) else [target]
    hist = collections.Counter()
    fields = collections.defaultdict(collections.Counter)
    vartypes = collections.Counter()
    natives = collections.Counter()
    for f in files:
        d = open(f, "rb").read()
        ver, classes, end = parse(d)
        ok = "OK" if end == len(d) else f"LEFT {len(d)-end}"
        print(f"{os.path.basename(f)}: v{ver} {len(classes)} classes, {sum(c['nquads'] for c in classes)} quads, {ok}")
        for c in classes:
            for t, tname, vn, off in c["vars"]:
                vartypes[(t, tname)] += 1
            for fn in c["funcs"]:
                for i, v in enumerate(fn[2:]):
                    fields[i][v] += 1
            q = c["quads"]
            for i in range(0, len(q), QUAD):
                hist[q[i]] += 1
                if q[i] == 0x0C:
                    natives[struct.unpack_from("<I", q, i + 1)[0]] += 1
            if "--dump" in sys.argv and c["name"] == sys.argv[sys.argv.index("--dump") + 1]:
                print(c["path"], c["name"], c["vars"])
                for fn in c["funcs"]:
                    print("  ", fn)
                for i in range(0, len(q), QUAD):
                    print(f"  {i//QUAD:5d}: " + " ".join(f"{b:02x}" for b in q[i : i + QUAD]))
    print("var types:", dict(vartypes))
    for i in range(6):
        print(f"func field {i}:", sorted(fields[i].items())[:20])
    print("opcodes:", sorted(hist.items()))
    print("native ids:", sorted(natives.items()))


if __name__ == "__main__":
    main()
