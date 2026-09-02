"""Derive the element index space of native 3 (element by index) from self-references (generic; no game bytes).

For every class of a .scb whose name is a named record of the paired .rhm (actor, object, scroll, script polygon,
named rail point), the immediates passed to native 3 inside that class are compared with the record's index in
its chunk. The most frequent difference per chunk is the chunk's base in the flat element table; printed with the
chunk counts so the layout hypothesis (which chunks precede which) can be tested across all missions.

Usage: python scb_elements.py <levels-dir> <opensherwood-tools.exe> [mission-base ...]
"""
import collections
import os
import re
import subprocess
import sys

from scb_probe import parse
from scb_semantics import Flow, functions


def rhm_records(tool, path):
    """Return (name -> (chunk, index)) and chunk counts from the `rhm` tool text output."""
    out = subprocess.run([tool, "rhm", path], capture_output=True, text=True, errors="replace").stdout
    names, counts = {}, {}
    section = None
    idx = 0
    for line in out.splitlines():
        m = re.match(r"^(SCOT|OILE|TOTO|BORG|BOOM|MEOW) v\d+: (\d+)", line)
        if m:
            section, idx = m.group(1), 0
            counts[section] = int(m.group(2))
            continue
        m = re.match(r"^(SKRO): (\d+)", line)
        if m:
            section, idx = "SKRO", 0
            counts["SKRO"] = int(m.group(2))
            continue
        m = re.match(r"^GULP: (\d+) points, (\d+) script polygons", line)
        if m:
            section, idx = "GULP", 0
            counts["GULP_pts"], counts["GULP_poly"] = int(m.group(1)), int(m.group(2))
            continue
        m = re.match(r"^RAIL: (\d+)", line)
        if m:
            section, idx = "RAIL", 0
            counts["RAIL"] = int(m.group(1))
            continue
        m = re.match(r"^(ZORG|CAVE): (\d+)", line)
        if m:
            section, idx = m.group(1), 0
            counts[m.group(1)] = int(m.group(2))
            continue
        m = re.match(r"^HIRN: (\d+) waypoints, (\d+) bushes, (\d+) beam-me points", line)
        if m:
            section = None
            counts["HOLE"], counts["BUSH"], counts["POW"] = int(m.group(1)), int(m.group(2)), int(m.group(3))
            continue
        m = re.match(r"^POUF: (\d+)", line)
        if m:
            section = None
            counts["POUF"] = int(m.group(1))
            continue
        m = re.match(r"^TING: (\d+)", line)
        if m:
            section = None
            counts["TING"] = int(m.group(1))
            continue
        if not line.startswith("  ") or section is None:
            continue
        if section == "RAIL":
            m = re.match(r"^  rail (\d+):", line)
            if m:
                for pt_i, name in enumerate(re.findall(r'"([^"]+__\d+___8[0-9a-f]{7})"', line)):
                    names[name] = ("RAILPT", int(m.group(1)))
            continue
        m = re.search(r"([A-Za-z0-9_]+_8[0-9a-f]{7})\s*$", line)
        if m:
            names[m.group(1)] = (section, idx)
        if section in ("SCOT", "OILE", "TOTO", "BORG", "BOOM", "SKRO", "MEOW"):
            idx += 1
        elif section == "GULP" and re.match(r"^  \d+ pts", line):
            idx += 1
    return names, counts


def main():
    levels, tool = sys.argv[1:3]
    only = set(sys.argv[3:])
    for f in sorted(os.listdir(levels)):
        if not f.endswith(".scb"):
            continue
        base = f[:-4]
        if only and base not in only:
            continue
        rhm = os.path.join(levels, ("Sherwood" if base == "sherwood" else base) + ".rhm")
        if not os.path.exists(rhm):
            continue
        names, counts = rhm_records(tool, rhm)
        _, classes, _ = parse(open(os.path.join(levels, f), "rb").read())
        diffs = collections.defaultdict(collections.Counter)
        unresolved = 0
        for cls in classes:
            if cls["name"] not in names:
                if cls["name"] != "StartUp":
                    unresolved += 1
                continue
            chunk, index = names[cls["name"]]
            used = collections.Counter()
            for fname, start, end, fn in functions(cls):
                flow = Flow(cls, start, end)

                def on_native(i, nid, args, ret):
                    if nid == 3 and args and args[0].startswith("imm:"):
                        used[int(args[0][4:])] += 1

                flow.walk(on_native, None)
            for k in used:
                diffs[chunk][k - index] += 1
        bases = {chunk: c.most_common(2) for chunk, c in diffs.items()}
        print(f"{base}: counts={counts} unresolved_classes={unresolved}")
        print(f"    base candidates (offset: hits): {bases}")


if __name__ == "__main__":
    main()
