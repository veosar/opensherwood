"""Numeric stat fields of `profile.cpf` actor records, laid out as columns (generic; no game bytes).

Usage: python cpf_stats.py <profile.cpf> [--tiers] [--tables]

Reads the file with `cpf_probe.parse` (docs/formats/profile.md) and prints the `unknown_pre` /
`unknown_post` bytes of every SD (soldier) and PC (player character) record split into the words that
docs/original/stealth-and-combat.md discusses. Column labels are the *hypotheses* of that document
(`hp?`, `purse?` ...), not established names; the reader of this output is meant to judge them.

`--tiers` prints, per soldier family (six consecutive records = the colour tiers 00..05), which columns
change from tier to tier and by how much, and which are constant across the family.
`--tables` prints table A (27 blocks: a 14-word head and ten 16-word records) and table B (four
41-word records) as integers.
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cpf_probe import parse  # noqa: E402

SD_PRE = ["hp?", "p1", "p2", "p3", "p4", "pole", "ranged?", "q1", "q2", "rank?", "q4"]
SD_POST = ["w0", "flags", "purse?", "apple?", "beer?", "whistle?", "class", "z0", "rkind", "z1", "z2", "u16", "weapon?", "armour?"]
FAMILIES = ["halberdier", "swordsman", "archer", "officer", "knight-foot", "lancer", "crossbowman"]


def sd_fields(rec):
    pre = struct.unpack("<5HB5H", bytes.fromhex(rec["pre"]))
    p = bytes.fromhex(rec["post"])
    w0, flags = struct.unpack_from("<HB", p, 0)
    stim = struct.unpack_from("<4H", p, 3)
    cls, z0, rkind, z1, z2 = struct.unpack_from("<HHHHB", p, 11)
    floats = struct.unpack_from("<4f", p, 20)
    one = p[36]
    centre = struct.unpack_from("<2f", p, 37)
    u16, weapon, armour = struct.unpack_from("<HII", p, 45)
    assert one == 1 and 45 + 10 == len(p), len(p)
    post = (w0, flags) + stim + (cls, z0, rkind, z1, z2, u16, weapon, armour)
    return dict(zip(SD_PRE, pre)), dict(zip(SD_POST, post)), floats, centre


def pc_fields(rec):
    pre = struct.unpack("<4H", bytes.fromhex(rec["pre"]))
    p = bytes.fromhex(rec["post"])
    words = struct.unpack_from("<%dH" % (len(p) // 2), p, 0)
    return pre, words


def main(argv):
    t = parse(open(argv[0], "rb").read())
    print("== SD records: pre = " + " ".join(SD_PRE) + " | post = " + " ".join(SD_POST))
    rows = []
    for i, rec in enumerate(t["sd"]):
        pre, post, floats, centre = sd_fields(rec)
        rows.append((pre, post))
        print(f"{i:2d} {rec['sprite']:14s} "
              + " ".join(f"{pre[k]:3d}" for k in SD_PRE) + " | "
              + " ".join((f"{post[k]:#04x}" if k == "flags" or k == "class" else f"{post[k]:3d}") for k in SD_POST)
              + f"  f32={tuple(int(x) for x in floats)} centre={tuple(int(x) for x in centre)}")
    if "--tiers" in argv:
        print("\n== per family: column deltas between consecutive tiers 00..04 (05 = the green repeat)")
        for f, name in enumerate(FAMILIES):
            fam = rows[f * 6:f * 6 + 6]
            for label, keys, part in (("pre", SD_PRE, 0), ("post", SD_POST, 1)):
                out = []
                for k in keys:
                    vals = [r[part][k] for r in fam[:5]]
                    deltas = {vals[j + 1] - vals[j] for j in range(4)}
                    if deltas == {0}:
                        out.append(f"{k}={vals[0]}")
                    elif len(deltas) == 1:
                        out.append(f"{k}={vals[0]}..{vals[4]}({'+' if vals[4] > vals[0] else ''}{deltas.pop()}/tier)")
                    else:
                        out.append(f"{k}={vals}")
                print(f"{name:12s} {label:4s} " + "  ".join(out))
    print("\n== PC records: pre = u16[4]; post as u16 words")
    for i, rec in enumerate(t["pc"]):
        pre, words = pc_fields(rec)
        print(f"{i:2d} {rec['sprite']:14s} pre={pre} post={words[:24]} ... tail={words[-2:]}")
    if "--tables" in argv:
        print("\n== table A (block k = combat class id k+1: PC ids 1..10, soldier class ids 0x0b..0x1b)")
        for i, blk in enumerate(t["table_a"]):
            print(i, struct.unpack("<14H", bytes.fromhex(blk["head"])))
            for rec in blk["records"]:
                print("   ", struct.unpack("<16H", bytes.fromhex(rec)))
        print("\n== table B")
        for rec in t["table_b"]:
            b = bytes.fromhex(rec)
            print(struct.unpack("<20H", b[:40]), b[40], struct.unpack("<20H", b[41:81]))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    main(sys.argv[1:])
