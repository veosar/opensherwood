"""Per-opcode operand statistics for SBSCRIPT 1.5 quads (generic; no game bytes).

Usage: python scb_opstats.py <dir>
Prints, for each opcode: count; for the u16 at +1 and +3: top-2-bit flag histogram and distinct low values;
for the u32 at +5: zero fraction, max, whether it fits as a quad index of the class, float plausibility.
"""
import collections
import glob
import os
import struct
import sys

from scb_probe import QUAD, parse


def main():
    files = sorted(glob.glob(os.path.join(sys.argv[1], "*.scb")))
    st = collections.defaultdict(lambda: dict(n=0, a=collections.Counter(), b=collections.Counter(), c0=0, cmax=0, cfit=0, cfloat=0, a_low=set(), b_low=set(), c_vals=collections.Counter()))
    init_ok = init_bad = 0
    call_targets = collections.Counter()
    for f in files:
        _, classes, _ = parse(open(f, "rb").read())
        for c in classes:
            q = c["quads"]
            n = c["nquads"]
            addrs = {fn[1] for fn in c["funcs"]}
            for fn in c["funcs"]:
                a = fn[1] * QUAD
                if q[a] == 3 and struct.unpack_from("<HH", q, a + 1) == (fn[4], fn[5]):
                    init_ok += 1
                else:
                    init_bad += 1
            for i in range(0, len(q), QUAD):
                op = q[i]
                a, b = struct.unpack_from("<HH", q, i + 1)
                cc = struct.unpack_from("<I", q, i + 5)[0]
                s = st[op]
                s["n"] += 1
                s["a"][a >> 14] += 1
                s["b"][b >> 14] += 1
                s["a_low"].add(a & 0x3FFF)
                s["b_low"].add(b & 0x3FFF)
                if cc == 0:
                    s["c0"] += 1
                s["cmax"] = max(s["cmax"], cc)
                if cc < n:
                    s["cfit"] += 1
                fl = struct.unpack_from("<f", q, i + 5)[0]
                if cc != 0 and 1e-3 < abs(fl) < 1e5:
                    s["cfloat"] += 1
                s["c_vals"][cc] += 1
                if op == 5:
                    call_targets["in_func_addrs" if cc in addrs else "other"] += 1
                if op == 0x0C:
                    pass
    print("InitFunction at function address matches (volatile,tempor):", init_ok, "bad:", init_bad)
    print("CALL targets:", dict(call_targets))
    for op in sorted(st):
        s = st[op]
        print(
            f"op {op:#04x} n={s['n']:6d} a.flags={dict(s['a'])} a.distinct={len(s['a_low'])} b.flags={dict(s['b'])} b.distinct={len(s['b_low'])} "
            f"c: zero={s['c0']} max={s['cmax']:#x} fitsquads={s['cfit']} floatlike={s['cfloat']} distinct={len(s['c_vals'])} top={s['c_vals'].most_common(3)}"
        )


if __name__ == "__main__":
    main()
