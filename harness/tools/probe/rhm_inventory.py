"""Inventory of DUTY mission containers: chunk sizes, counts, and BOYZ class groups (generic; no game bytes).

Usage: python rhm_inventory.py <dir-with-rhm-files>
"""
import collections
import glob
import os
import struct
import sys

from chunkdump import children


def main():
    files = sorted(glob.glob(os.path.join(sys.argv[1], "*.rhm")))
    classes = collections.defaultdict(list)
    for f in files:
        data = open(f, "rb").read()
        root, ver, ch = children(data)
        parts = []
        for t, v, off, body in ch:
            cnt = struct.unpack_from("<H", body, 0)[0] if len(body) >= 2 else -1
            parts.append(f"{t}v{v}:{cnt}/{len(body)}")
            if t == "BOYZ":
                pos = 2
                groups = []
                while pos < len(body):
                    ct, cs, cv = struct.unpack_from("<4sII", body, pos)
                    gb = body[pos + 12 : pos + 8 + cs]
                    gc = struct.unpack_from("<H", gb, 0)[0] if len(gb) >= 2 else -1
                    groups.append(f"{ct.decode('latin-1')}v{cv}:{gc}/{len(gb)}")
                    classes[(ct.decode("latin-1"), cv)].append((gc, len(gb), os.path.basename(f)))
                    pos += 8 + cs
                parts.append("[" + " ".join(groups) + "]")
        print(os.path.basename(f), " ".join(parts))
    print()
    for (ct, cv), lst in sorted(classes.items()):
        n = sum(c for c, _, _ in lst)
        b = sum(s for _, s, _ in lst)
        print(f"{ct} v{cv}: {len(lst)} groups, {n} records, {b} bytes; ", end="")
        # fixed-size hypothesis
        ratios = sorted({(s - 2) / c for c, s, _ in lst if c > 0})
        print("bytes/record:", ratios[:6], "..." if len(ratios) > 6 else "")


if __name__ == "__main__":
    main()
