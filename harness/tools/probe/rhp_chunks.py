"""Generic chunk walker for MEUH (.rhp) / DUTY (.rhm) containers. No game bytes inside.

Usage as a library:
    from rhp_chunks import load_chunks, MAP_SIZES
    chunks = load_chunks(path)   # dict tag -> (version, body_bytes)
"""
import os
import struct
import sys

GAME = os.environ.get("OPENSHERWOOD_GAME_DIR", r"C:\Users\przem\source\gamedata\robinhood")
LEVELS = os.path.join(GAME, "DATA", "Levels")
MAPS = ["Croisement01", "Croisement02", "Croisement03", "derby", "leicester", "lincoln",
        "nottingham", "sherwood", "york"]


def rhp_path(name):
    return os.path.join(LEVELS, name + ".rhp")


def map_path(name, variant="Day"):
    d = os.path.join(LEVELS, variant)
    for f in os.listdir(d):
        if f.lower() == name.lower() + ".map":
            return os.path.join(d, f)
    raise FileNotFoundError(name)


def map_size(name, variant="Day"):
    with open(map_path(name, variant), "rb") as fh:
        w, h = struct.unpack("<HH", fh.read(4))
    return w, h


def walk(body):
    """Yield (tag, version, body) for consecutive chunks in `body`."""
    pos = 0
    while pos + 8 <= len(body):
        tag = body[pos:pos + 4].decode("latin-1")
        size = struct.unpack_from("<I", body, pos + 4)[0]
        ver = struct.unpack_from("<I", body, pos + 8)[0]
        yield tag, ver, body[pos + 12:pos + 8 + size]
        pos += 8 + size
    assert pos == len(body), (pos, len(body))


def load_chunks(path):
    data = open(path, "rb").read()
    assert data[:4] == b"MEUH" or data[:4] == b"DUTY"
    size = struct.unpack_from("<I", data, 4)[0]
    assert size == len(data) - 8
    out = {}
    for tag, ver, body in walk(data[12:]):
        out[tag] = (ver, body)
    return out


def hexdump(b, start=0, n=256, width=16):
    lines = []
    for off in range(start, min(len(b), start + n), width):
        chunk = b[off:off + width]
        hx = " ".join(f"{c:02x}" for c in chunk)
        asc = "".join(chr(c) if 32 <= c < 127 else "." for c in chunk)
        lines.append(f"{off:06x}  {hx:<{width*3}}  {asc}")
    return "\n".join(lines)


if __name__ == "__main__":
    name = sys.argv[1] if len(sys.argv) > 1 else "Croisement01"
    n = int(sys.argv[2]) if len(sys.argv) > 2 else 96
    ch = load_chunks(rhp_path(name))
    print(name, map_size(name))
    for tag, (ver, body) in ch.items():
        print(f"--- {tag!r} v{ver} len={len(body)}")
        print(hexdump(body, 0, n))
