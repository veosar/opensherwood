"""Action tables of `.rhs` character profiles: id, frames, ticks, advance, displacement (generic; no game bytes).

Usage:
  anim_actions.py <Characters dir> --table NAME [NAME ...] [--ids 41 44 ...]
      one line per 16-direction block of the named profiles (direction 4 = screen-right):
      action id, frame count, sum and list of the per-frame tick halves, sum and list of the per-frame
      advance halves (signed), the block displacement of direction 4 and `unknown_0x02`
  anim_actions.py <Characters dir> --families
      groups every 16-block profile by its exact action-id sequence and prints the groups
  anim_actions.py <Characters dir> --matrix NAME [NAME ...] [--ids ...]
      one row per action id, one column per profile: frames/ticks/advance/displacement or `-`
  anim_actions.py <Characters dir> --sheet NAME OUT.png SCALE ID [ID ...]
      renders every frame of direction 4 of the given action ids of one profile into one picture
      (needs OPENSHERWOOD_GAME_DIR for the pixel bank; the picture is game art: never commit it)

Layout facts used: docs/formats/sprite-animations.md (blocks of 16, `unknown_0x0c` = action id, the
timing word split into a tick half and an advance half, the per-animation displacement).
"""
import collections
import glob
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from anim_sheet import ORIGIN, parse_rhs  # noqa: E402

DIRECTION = 4


def blocks(path):
    seq = parse_rhs(path)[0]
    an = seq["anims"]
    if len(an) % 16:
        return None
    out = []
    for b in range(len(an) // 16):
        a = an[b * 16 + DIRECTION]
        fr = a["frames"]
        ticks = [f["duration"] & 0xFFFF for f in fr]
        adv = [(f["duration"] >> 16) for f in fr]
        adv = [x - 65536 if x >= 32768 else x for x in adv]
        out.append(dict(id=a["u0c"], n=len(fr), ticks=ticks, adv=adv,
                        disp=(a["u04"] - ORIGIN, a["u08"] - ORIGIN), u02=a["u02"]))
    return out


def table(d, names, ids):
    for name in names:
        print("=====", name)
        for b in blocks(os.path.join(d, name + ".rhs")):
            if ids and b["id"] not in ids:
                continue
            print(f"id={b['id']:3d} n={b['n']:2d} ticks={sum(b['ticks']):3d} {b['ticks']} "
                  f"adv={sum(b['adv']):4d} {b['adv']} disp={b['disp']} u02={b['u02']}")


def families(d):
    fam = collections.defaultdict(list)
    for p in sorted(glob.glob(os.path.join(d, "*.rhs"))):
        bl = blocks(p)
        if bl is None:
            continue
        fam[tuple(b["id"] for b in bl)].append(os.path.basename(p)[:-4])
    for ids, names in sorted(fam.items(), key=lambda kv: -len(kv[0])):
        print(f"## {len(ids)} blocks: {names}")
        print("   ids:", " ".join(map(str, ids)))


def matrix(d, names, ids):
    rows = {n: {b["id"]: b for b in blocks(os.path.join(d, n + ".rhs"))} for n in names}
    allids = sorted(set(i for n in names for i in rows[n]))
    print("id | " + " | ".join(names))
    for i in allids:
        if ids and i not in ids:
            continue
        cells = []
        for n in names:
            b = rows[n].get(i)
            cells.append("-" if b is None else f"{b['n']}/{sum(b['ticks'])}/{sum(b['adv']):+d}/{b['disp'][0]}")
        print(f"{i:3d} | " + " | ".join(cells))


def sheet(d, name, out, scale, ids):
    from anim_sheet import render_cells
    from sprite_render import load_pages
    from spritebank import Bank, write_png

    an = parse_rhs(os.path.join(d, name + ".rhs"))[0]["anims"]
    byid = {an[b * 16 + DIRECTION]["u0c"]: an[b * 16 + DIRECTION] for b in range(len(an) // 16)}
    ids = [i for i in ids if i in byid]
    cols = max(len(byid[i]["frames"]) for i in ids)
    cells, labels = [], []
    for i in ids:
        fr = byid[i]["frames"]
        for k in range(cols):
            if k < len(fr):
                cells.append(fr[k])
                labels.append(str(i) if k == 0 else f"{k}:{fr[k]['duration'] & 0xFFFF}")
            else:
                cells.append(dict(frame=0, anchor=(ORIGIN, ORIGIN), duration=0))
                labels.append("")
    bank = Bank()
    img = render_cells(bank, load_pages(bank), cells, cols, scale, labels)
    write_png(out, img)
    print(out, img.shape)


def main(argv):
    d = argv[0]
    ids = set()
    if "--ids" in argv:
        k = argv.index("--ids")
        ids = {int(x) for x in argv[k + 1:]}
        argv = argv[:k]
    if "--table" in argv:
        table(d, argv[argv.index("--table") + 1:], ids)
    elif "--families" in argv:
        families(d)
    elif "--matrix" in argv:
        matrix(d, argv[argv.index("--matrix") + 1:], ids)
    elif "--sheet" in argv:
        k = argv.index("--sheet")
        sheet(d, argv[k + 1], argv[k + 2], int(argv[k + 3]), [int(x) for x in argv[k + 4:]])
    else:
        print(__doc__)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    main(sys.argv[1:])
