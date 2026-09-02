"""Hexdump the body of one child chunk of a DUTY/MEUH container (generic; no game bytes inside).

Usage: python chunkdump.py <file> [TAG] [max_bytes]
Without TAG: list the children. With TAG: hexdump that child's body (after the version word).
"""
import struct
import sys


def children(data):
    tag, size, ver = struct.unpack_from("<4sII", data, 0)
    pos, end = 12, 8 + size
    out = []
    while pos < end:
        t, s, v = struct.unpack_from("<4sII", data, pos)
        out.append((t.decode("latin-1"), v, pos, data[pos + 12 : pos + 8 + s]))
        pos += 8 + s
    return tag.decode("latin-1"), ver, out


def hexdump(b, base=0, limit=None):
    if limit is not None:
        b = b[:limit]
    for i in range(0, len(b), 16):
        row = b[i : i + 16]
        hx = " ".join(f"{c:02x}" for c in row)
        asc = "".join(chr(c) if 32 <= c < 127 else "." for c in row)
        print(f"{base + i:06x}  {hx:<48}  {asc}")


def main():
    data = open(sys.argv[1], "rb").read()
    root, ver, ch = children(data)
    if len(sys.argv) < 3:
        print(root, ver)
        for t, v, off, body in ch:
            print(f"  {t} v{v} at {off:#x} body {len(body)}")
        return
    limit = int(sys.argv[3]) if len(sys.argv) > 3 else None
    for t, v, off, body in ch:
        if t == sys.argv[2]:
            print(f"{t} v{v} body {len(body)} bytes")
            hexdump(body, 0, limit)


if __name__ == "__main__":
    main()
