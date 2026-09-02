"""Parse DUTY mission chunks under the current layout hypotheses and report consumption (generic; no game bytes).

Usage: python rhm_probe.py <dir-or-file> [--dump TAG] [--verbose]
"""
import collections
import glob
import os
import struct
import sys

from chunkdump import children


class R:
    def __init__(self, b, pos=0):
        self.b, self.p = b, pos

    def u8(self):
        v = self.b[self.p]
        self.p += 1
        return v

    def u16(self):
        v = struct.unpack_from("<H", self.b, self.p)[0]
        self.p += 2
        return v

    def i16(self):
        v = struct.unpack_from("<h", self.b, self.p)[0]
        self.p += 2
        return v

    def u32(self):
        v = struct.unpack_from("<I", self.b, self.p)[0]
        self.p += 4
        return v

    def raw(self, n):
        v = self.b[self.p : self.p + n]
        if len(v) != n:
            raise EOFError(f"need {n} at {self.p:#x}")
        self.p += n
        return v

    def pstr(self):
        n = self.u16()
        return self.raw(n).decode("latin-1")

    def done(self):
        return self.p >= len(self.b)


def header(r):
    """Common element header: x, y, dir, flags, P, Q, R."""
    return dict(x=r.u16(), y=r.u16(), dir=r.u32(), flags=r.u32(), P=r.i16(), Q=r.u16(), R=r.u16())


def opt_name(r):
    return r.pstr() if r.u8() else None


def rec_scot(r):
    d = header(r)
    d["B"] = r.u32()
    d["u32a"] = r.u32()
    d["u32b"] = r.u32()
    d["u16c"] = r.u16()
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
    d["list"] = [r.u16() for _ in range(n)]
    d["rail"] = r.i16()
    d["i16b"] = r.i16()
    d["name"] = opt_name(r)
    return d


def rec_oile(r):
    d = header(r)
    d["B"] = r.u32()
    d["u32b"] = r.u32()
    d["i16a"] = r.i16()
    d["i16b"] = r.i16()
    d["name"] = opt_name(r)
    return d


RECS = {"SCOT": rec_scot, "BORG": rec_borg, "OILE": rec_oile}


def parse_group(tag, body, verbose):
    r = R(body)
    n = r.u16()
    recs = []
    fn = RECS.get(tag)
    if fn is None:
        return None, f"no grammar ({n} records, {len(body)} bytes)"
    try:
        for i in range(n):
            start = r.p
            d = fn(r)
            d["_off"] = start
            recs.append(d)
            if verbose:
                print("   ", tag, i, d)
    except Exception as e:  # noqa: BLE001
        return recs, f"error {e} after {len(recs)} records at {r.p:#x}: {body[r.p:r.p+48].hex()}"
    if r.p != len(body):
        return recs, f"left {len(body)-r.p} bytes after {n} records at {r.p:#x}: {body[r.p:r.p+64].hex()}"
    return recs, "OK"


def main():
    target = sys.argv[1]
    files = sorted(glob.glob(os.path.join(target, "*.rhm"))) if os.path.isdir(target) else [target]
    verbose = "--verbose" in sys.argv
    dump = sys.argv[sys.argv.index("--dump") + 1] if "--dump" in sys.argv else None
    stats = collections.defaultdict(collections.Counter)
    for f in files:
        data = open(f, "rb").read()
        _, _, ch = children(data)
        boyz = next(b for t, v, o, b in ch if t == "BOYZ")
        pos = 2
        while pos < len(boyz):
            ct, cs, cv = struct.unpack_from("<4sII", boyz, pos)
            ct = ct.decode("latin-1")
            gb = boyz[pos + 12 : pos + 8 + cs]
            pos += 8 + cs
            recs, status = parse_group(ct, gb, verbose or dump == ct)
            if status != "OK" and recs is not None:
                print(os.path.basename(f), ct, status)
            for d in recs or []:
                for k, v in d.items():
                    if k in ("x", "y", "_off", "name", "list"):
                        continue
                    stats[(ct, k)][v] += 1
    for k in sorted(stats):
        vals = stats[k]
        print(k, len(vals), "distinct:", sorted(vals.items(), key=lambda kv: -kv[1])[:12])


if __name__ == "__main__":
    main()
