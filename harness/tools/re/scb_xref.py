"""Cross-reference native-call immediates of each .scb with the element counts of the paired .rhm and the
text counts of the level's .red index (generic; no game bytes embedded).

Usage: python scb_xref.py <levels-dir> <text-dir> <opensherwood-tools.exe> [native ...]

For every mission: the number of actors (SCOT+OILE+TOTO+BORG), objects (BOOM), scrolls (SKRO), script polygons
and points (GULP), rails, waypoints (HOLE), beam-me points (POW) from the `rhm` tool output, the text-list size
of the matching RHLevel??.red (the mapping file-name -> code follows docs/original/campaign-flow.md and is a
hypothesis that this probe tests), and the maximum immediate passed to each candidate native (default: the ids
whose immediates look like element or text indices).
"""
import collections
import os
import re
import struct
import subprocess
import sys

from scb_probe import parse
from scb_semantics import Flow, functions

# Mission file base name -> RHLevel code (hypothesis from the campaign-flow notes; tested here).
RED_CODE = {
    "H01_Lin_VL": "HA", "H02_Not_EC": "HB", "H03_Der_MK": "HC", "H04_Lei_VL": "HD", "H05_Lin_EC": "HE",
    "H07_Not_MK": "HF", "H09_Not_VL": "HG", "H10_Yor_VL": "HH", "H12_Not_MP": "HI", "sherwood": "HQ",
    "S01_Not_VL": "SA", "S02_Lei_MP": "SB", "S03_FoB_MP": "SC", "S04_Der_EC": "SD", "S05_Yrk_EC": "SE",
    "Str01_Lin_EC": "AA", "Str02_Der_MP": "AB", "Str03_Yor_MK": "AC",
    "Emb01_FoA_EC": "EA", "Emb02_FoC_MK": "EB", "Emb03_FoC_MP": "EC", "Emb04_FoA_MP": "ED", "Emb05_FoB_MP": "EE",
    "Emb06_FoC_EC": "EF", "Emb07_FoB_JMS": "EG", "Emb08_FoA_JMS": "EH", "Emb09_FoB_JMS": "EI", "EmbTut_FoC_EC": "ET",
    "Tac01_FoA_MP": "TA", "Tac02_FoB_EC": "TB", "Tac03_FoC_MP": "TC", "Tac04_FoA_EC": "TD", "Tac05_FoC_MP": "TE",
    "Tac06_FoB_EC": "TF", "Tac17_FoC_EC": "TQ", "Tac18_FoA_EC": "TR", "Tac19_FoB_EC": "TS", "Tac21_FoB_EC": "TT",
    "SherwoodOutro": "VO",
}


def red_text_count(path):
    """Size of the text list: the count preceding the list id (u32 layout, see campaign-flow.md)."""
    d = open(path, "rb").read()
    v = struct.unpack("<%dI" % (len(d) // 4), d)
    # The file ends with n_won, id, n_lost, id, n_short, id; the text list count is the value before the
    # first id of the block that precedes those, i.e. v[-7 - n] where n is the list length: scan for it.
    tail = 6
    for n in range(1, 60):
        k = len(v) - tail - n - 2
        if k >= 0 and v[k] == n and all(x >= 1000000 for x in v[k + 1 : k + 2 + n]):
            return n, v[k + 1], v[-2]
    return None, None, v[-2]


def rhm_counts(tool, path):
    out = subprocess.run([tool, "rhm", path], capture_output=True, text=True, errors="replace").stdout
    c = {}
    for key, rx in {
        "scot": r"SCOT v\d+: (\d+)", "oile": r"OILE v\d+: (\d+)", "toto": r"TOTO v\d+: (\d+)", "borg": r"BORG v\d+: (\d+)",
        "boom": r"BOOM v\d+: (\d+)", "skro": r"SKRO: (\d+)", "gulp_pts": r"GULP: (\d+) points", "gulp_poly": r"(\d+) script polygons",
        "rail": r"RAIL: (\d+)", "hole": r"HOLE[^\n]*?(\d+)", "pow": r"POW[^\n]*?(\d+)", "bush": r"BUSH[^\n]*?(\d+)",
        "zorg": r"ZORG: (\d+)", "pouf": r"POUF: (\d+)",
    }.items():
        m = re.search(rx, out)
        c[key] = int(m.group(1)) if m else -1
    c["named_rail_pts"] = len(re.findall(r'"[^"]+__\d+___8[0-9a-f]{7}"', out))
    c["actors"] = c["scot"] + c["oile"] + c["toto"] + c["borg"]
    return c


def main():
    levels, textdir, tool = sys.argv[1:4]
    natives = [int(x) for x in sys.argv[4:]] or [3, 6, 9, 4, 5, 202, 203, 26, 27, 195]
    for base in sorted(RED_CODE):
        scb = os.path.join(levels, base + ".scb")
        rhm = os.path.join(levels, ("Sherwood" if base == "sherwood" else base) + ".rhm")
        if not os.path.exists(scb):
            continue
        counts = rhm_counts(tool, rhm) if os.path.exists(rhm) else {}
        red = os.path.join(textdir, f"RHLevel{RED_CODE[base]}.red")
        ntext, list_id, short_id = red_text_count(red) if os.path.exists(red) else (None, None, None)
        maxes = collections.defaultdict(lambda: -1)
        distinct = collections.defaultdict(set)
        _, classes, _ = parse(open(scb, "rb").read())
        nclasses = len(classes) - 1
        for cls in classes:
            for fname, start, end, fn in functions(cls):
                flow = Flow(cls, start, end)

                def on_native(i, nid, args, ret):
                    if nid in natives:
                        for k, src in enumerate(args):
                            if src.startswith("imm:"):
                                v = int(src[4:])
                                maxes[(nid, k)] = max(maxes[(nid, k)], v)
                                distinct[(nid, k)].add(v)

                flow.walk(on_native, None)
        print(f"{base} ({RED_CODE[base]}): classes={nclasses} actors={counts.get('actors')} (scot {counts.get('scot')} oile {counts.get('oile')} "
              f"toto {counts.get('toto')} borg {counts.get('borg')}) boom={counts.get('boom')} skro={counts.get('skro')} "
              f"gulp={counts.get('gulp_pts')}/{counts.get('gulp_poly')} rail={counts.get('rail')} named_pts={counts.get('named_rail_pts')} "
              f"hole={counts.get('hole')} pow={counts.get('pow')} zorg={counts.get('zorg')} pouf={counts.get('pouf')} texts={ntext}")
        print("    " + "  ".join(f"n{nid}[{k}]<={maxes[(nid, k)]}({len(distinct[(nid, k)])})" for (nid, k) in sorted(maxes)))


if __name__ == "__main__":
    main()
