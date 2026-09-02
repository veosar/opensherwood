"""Walk `DATA/Configuration/profile.cpf` (character / level profile table) and print its tables.

Generic reader written from observation; embeds no game bytes. Spec: docs/formats/profile.md.

Usage: python cpf_probe.py <profile.cpf> [--rhs <Characters dir>] [--hex] [--json FILE]

Layout (all little-endian, pstring = u16 length + Latin-1 bytes, code = char[4] NUL padded):

    u32 n_a; n_a x block_a        block_a = 28-byte header + 10 x 32-byte records   (unknown tables)
    u32 n_b; n_b x 81 bytes       (unknown; four records, one per difficulty is the guess)
    u32 n_pc; n_pc x PC           PC  = pstring sprite, pstring sequence, pstring label, 8 bytes, code, 82 bytes
    u32 n_sd; n_sd x SD           SD  = pstring sprite, pstring sequence, pstring label, 21 bytes, code, 55 bytes
    u32 n_lv; n_lv x LEVEL        LEVEL = code, pstring map, pstring mission, pstring title, variable part
                                          (see spec), pstring music_ambient, pstring music_alarm,
                                          pstring music_fight
    u32 n_cv; n_cv x CV           CV  = pstring sprite, pstring sequence, pstring label, 8 bytes, code

The script asserts that this grammar consumes the file exactly.
"""
import json
import os
import struct
import sys


class R:
    def __init__(self, b):
        self.b, self.p = b, 0

    def u8(self):
        v = self.b[self.p]
        self.p += 1
        return v

    def u16(self):
        v = struct.unpack_from("<H", self.b, self.p)[0]
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

    def code(self):
        return self.raw(4).rstrip(b"\0").decode("latin-1")

    def done(self):
        return self.p >= len(self.b)


PC_PRE, PC_POST = 8, 82
SD_PRE, SD_POST = 21, 55
CV_PRE = 8


def actor(r, pre, post):
    d = {"at": r.p}
    d["sprite"] = r.pstr()
    d["sequence"] = r.pstr()
    d["label"] = r.pstr()
    d["pre"] = r.raw(pre).hex()
    d["voice"] = r.code()
    d["post"] = r.raw(post).hex() if post else ""
    return d


def pstr_at(b, p, lo=1, hi=48):
    """Length of a plausible printable pstring at p, or -1."""
    if p + 2 > len(b):
        return -1
    n = struct.unpack_from("<H", b, p)[0]
    if not lo <= n <= hi or p + 2 + n > len(b):
        return -1
    return n if all(32 <= c < 127 for c in b[p + 2 : p + 2 + n]) else -1


def is_code(b, p):
    """True when a level code (two capitals + two NULs) sits at p."""
    c = b[p : p + 4]
    return len(c) == 4 and c[2] == 0 and c[3] == 0 and all(65 <= x <= 90 for x in c[:2])


def find_music(b, start):
    """Offset of the three consecutive printable pstrings (music names) that end a level record."""
    p = start
    while p < len(b):
        q = p
        ok = True
        for _ in range(3):
            n = pstr_at(b, q)
            if n < 0:
                ok = False
                break
            q += 2 + n
        # what follows is the next level code, or the civilian section (u32 count then a pstring)
        if ok and (is_code(b, q) or pstr_at(b, q + 4) >= 0):
            return p
        p += 1
    raise ValueError(f"no music triple after {start:#x}")


def level(r):
    d = {"at": r.p}
    d["code"] = r.code()
    d["map"] = r.pstr()
    d["mission"] = r.pstr()
    d["title"] = r.pstr()
    end = find_music(r.b, r.p)
    d["mid"] = r.raw(end - r.p).hex()
    d["music"] = [r.pstr(), r.pstr(), r.pstr()]
    return d


def parse(b):
    r = R(b)
    out = {}
    n_a = r.u32()
    out["table_a"] = []
    for _ in range(n_a):
        head = r.raw(28).hex()
        recs = [r.raw(32).hex() for _ in range(10)]
        out["table_a"].append({"head": head, "records": recs})
    n_b = r.u32()
    out["table_b"] = [r.raw(81).hex() for _ in range(n_b)]
    n = r.u32()
    out["pc"] = [actor(r, PC_PRE, PC_POST) for _ in range(n)]
    n = r.u32()
    out["sd"] = [actor(r, SD_PRE, SD_POST) for _ in range(n)]
    n = r.u32()
    out["level"] = [level(r) for _ in range(n)]
    n = r.u32()
    out["cv"] = [actor(r, CV_PRE, 0) for _ in range(n)]
    assert r.done(), f"{len(b) - r.p} bytes left at {r.p:#x}"
    return out


def main(argv):
    path = argv[0]
    b = open(path, "rb").read()
    t = parse(b)
    show_hex = "--hex" in argv
    print(f"{os.path.basename(path)}: {len(b)} bytes; table_a {len(t['table_a'])} blocks, table_b {len(t['table_b'])}, "
          f"pc {len(t['pc'])}, sd {len(t['sd'])}, level {len(t['level'])}, cv {len(t['cv'])}")
    rhs_dir = argv[argv.index("--rhs") + 1] if "--rhs" in argv else None
    for sec in ("pc", "sd", "cv"):
        print(f"== {sec.upper()} ({len(t[sec])})")
        for i, d in enumerate(t[sec]):
            check = ""
            if rhs_dir:
                p = os.path.join(rhs_dir, d["sprite"] + ".rhs")
                if os.path.exists(p):
                    with open(p, "rb") as f:
                        f.seek(6)
                        seq = f.read(32).split(b"\0")[0].decode("latin-1")
                    check = "rhs-seq=OK" if seq == d["sequence"] else f"rhs-seq={seq!r}"
                else:
                    check = "rhs MISSING"
            line = f"{i:3d} {d['sprite']:20s} seq={d['sequence']!r:22s} label={d['label']!r:30s} voice={d['voice']:4s} {check}"
            if show_hex:
                line += f"\n     pre={d['pre']} post={d['post']}"
            print(line)
    print(f"== LEVEL ({len(t['level'])})")
    for i, d in enumerate(t["level"]):
        line = f"{i:3d} {d['code']:3s} map={d['map']:13s} mission={d['mission']:19s} title={d['title']!r:28s} music={d['music']}"
        if show_hex:
            line += f"\n     mid[{len(d['mid']) // 2}]={d['mid']}"
        print(line)
    if show_hex:
        print("== table_a heads")
        for i, blk in enumerate(t["table_a"]):
            print(i, blk["head"])
            for rec in blk["records"]:
                print("   ", rec)
        print("== table_b")
        for rec in t["table_b"]:
            print("   ", rec)
    if "--json" in argv:
        json.dump(t, open(argv[argv.index("--json") + 1], "w"), indent=1)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    main(sys.argv[1:])
