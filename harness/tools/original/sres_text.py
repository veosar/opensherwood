"""Dump TEXT entries of an SRES archive (generic reader written from docs/formats/sres.md).

Usage: python sres_text.py <file.res> [id ...]   (no ids: all TEXT entries, first string only)
"""
import struct
import sys


def read_blob_len(data, pos):
    raise NotImplementedError


def entries(data):
    """Yield (tag, id, body_offset, next_offset) using the trailer when present."""
    count = struct.unpack_from("<I", data, 8)[0]
    # trailer: count+1 u32 offsets; the last is the trailer offset itself
    tail = data[-4 * (count + 1):]
    offs = struct.unpack("<%dI" % (count + 1), tail)
    if offs[0] == 12 and offs[-1] == len(data) - 4 * (count + 1):
        for i in range(count):
            tag = data[offs[i]:offs[i] + 4]
            eid = struct.unpack_from("<I", data, offs[i] + 4)[0]
            yield tag, eid, offs[i] + 12, offs[i + 1]
        return
    raise SystemExit("no trailer; only trailer archives are supported by this helper")


def read_text(data, pos):
    n = struct.unpack_from("<H", data, pos)[0]
    pos += 2
    out = []
    for _ in range(n):
        ln = struct.unpack_from("<H", data, pos)[0]
        pos += 2
        out.append(data[pos:pos + 2 * ln].decode("utf-16-le"))
        pos += 2 * ln
    return out


def main(argv):
    data = open(argv[0], "rb").read()
    want = set(int(a) for a in argv[1:])
    for tag, eid, body, nxt in entries(data):
        if tag != b"TEXT":
            continue
        if want and eid not in want:
            continue
        strings = read_text(data, body)
        if want:
            print("== id %d (%d strings)" % (eid, len(strings)))
            for i, s in enumerate(strings):
                print("  [%d] %s" % (i, s))
        else:
            print("%d\t%d\t%s" % (eid, len(strings), strings[0][:70] if strings else ""))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    main(sys.argv[1:])
