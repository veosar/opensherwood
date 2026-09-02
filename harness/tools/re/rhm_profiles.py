"""Cross-check mission actor profile indices against `profile.cpf` (generic; no game bytes).

Usage: python rhm_profiles.py <Levels dir> <profile.cpf> [--cast MISSION ...] [--scot]

Prints, per actor group, every profile index used across the missions with its record count, the
number of missions using it, the designer name prefixes seen on named records, and the sprite/label the
index resolves to when read as a 0-based position in the matching `profile.cpf` section
(BORG -> SD, OILE -> CV, TOTO -> PC). `--cast` lists the resolved cast of the named missions; `--scot`
dumps the player-character records (which carry no profile index).
"""
import collections
import glob
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cpf_probe import parse as parse_cpf  # noqa: E402
from rhm_full import parse_file  # noqa: E402

SECTION = {"BORG": "sd", "OILE": "cv", "TOTO": "pc"}


def prefix(name):
    if not name:
        return "-"
    n = re.sub(r"_8[0-9a-f]{7}$", "", name)
    return re.sub(r"\d+", "#", n)


def main(argv):
    levels, cpf_path = argv[0], argv[1]
    cpf = parse_cpf(open(cpf_path, "rb").read())
    missions = {}
    for f in sorted(glob.glob(os.path.join(levels, "*.rhm"))):
        missions[os.path.basename(f)[:-4]] = parse_file(open(f, "rb").read())

    def resolve(tag, idx):
        sec = cpf[SECTION[tag]]
        if 0 <= idx < len(sec):
            d = sec[idx]
            return f"{d['sprite']} [{d['label']}] {d['voice']}"
        return "OUT OF RANGE"

    if "--cast" in argv:
        wanted = argv[argv.index("--cast") + 1 :]
        for name in wanted:
            m = missions[name]
            print(f"== {name}: map {m['FOOT']['data']['map']} mission_id {m['FOOT']['data']['mission_id']}")
            for g in m["BOYZ"]["data"]:
                if g["tag"] not in SECTION:
                    continue
                counts = collections.Counter(rec["profile"] for rec in g["records"])
                for idx, n in sorted(counts.items()):
                    names = [rec["name"] for rec in g["records"] if rec["profile"] == idx and rec["name"]]
                    print(f"  {g['tag']} {idx:3d} x{n:<3d} -> {resolve(g['tag'], idx):45s} {names[:4]}")
        return

    if "--scot" in argv:
        for name, m in missions.items():
            for g in m["BOYZ"]["data"]:
                if g["tag"] != "SCOT":
                    continue
                print(name, len(g["records"]))
                for rec in g["records"]:
                    fl = bytes.fromhex(rec["flags10"])
                    bits = [i for i, v in enumerate(fl) if v]
                    print(f"    u12={rec['B']} flagbytes={bits} u08={rec['flags']:#x} trailer={rec['trailer']} {rec['name'] or '-'}")
        return

    tally = {t: collections.defaultdict(collections.Counter) for t in SECTION}
    per_mission = {t: collections.defaultdict(set) for t in SECTION}
    for name, m in missions.items():
        for g in m["BOYZ"]["data"]:
            if g["tag"] not in SECTION:
                continue
            for rec in g["records"]:
                tally[g["tag"]][rec["profile"]][prefix(rec["name"])] += 1
                per_mission[g["tag"]][rec["profile"]].add(name)
    for tag, sec in SECTION.items():
        print(f"== {tag} -> cpf section {sec} ({len(cpf[sec])} entries); {len(tally[tag])} distinct indices used")
        for idx in range(max(len(cpf[sec]), max(tally[tag]) + 1)):
            c = tally[tag].get(idx)
            used = sum(c.values()) if c else 0
            named = {k: v for k, v in c.most_common(6) if k != "-"} if c else {}
            print(f"  {idx:3d} x{used:<4d} in {len(per_mission[tag].get(idx, ())):2d} missions -> {resolve(tag, idx):48s} {named}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    main(sys.argv[1:])
