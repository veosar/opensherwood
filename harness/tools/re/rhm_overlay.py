"""Draw a parsed mission (JSON from rhm_full.py --json) over a map PNG for visual validation (generic; no game bytes).

Usage: python rhm_overlay.py <mission.json> <map.png> <out.png>
Colours: PCs green, NPC humans red, civilians yellow, VIPs magenta, objects cyan, waypoints (HOLE) white,
beam-me points (POW) orange, bushes blue, rails (patrol paths) lime lines, script polygons (GULP) purple,
scrolls gold, ZORG entries pink.
"""
import json
import sys

from PIL import Image, ImageDraw


def main():
    m = json.load(open(sys.argv[1]))
    img = Image.open(sys.argv[2]).convert("RGBA")
    d = ImageDraw.Draw(img)

    def dot(x, y, col, r=4, label=None):
        d.ellipse((x - r, y - r, x + r, y + r), outline=col, width=2)
        if label:
            d.text((x + r + 1, y - 6), label, fill=col)

    colours = dict(SCOT="lime", BORG="red", OILE="yellow", TOTO="magenta", BOOM="cyan")
    for g in m["BOYZ"]["data"]:
        col = colours.get(g["tag"], "white")
        for rec in g["records"]:
            label = (rec.get("name") or "")[:12]
            dot(rec["x"], rec["y"], col, 5, label or None)
            if "dir" in rec:
                import math

                a = rec["dir"] * math.pi / 8
                d.line((rec["x"], rec["y"], rec["x"] + 10 * math.cos(a), rec["y"] - 10 * math.sin(a)), fill=col)
    for w in m["HIRN"]["data"].get("HOLE", {}).get("records", []):
        dot(w["x"], w["y"], "white", 3)
    for w in m["HIRN"]["data"].get("POW ", {}).get("records", []):
        dot(w["x"], w["y"], "orange", 6, "POW")
    for w in m["HIRN"]["data"].get("BUSH", {}).get("records", []):
        dot(w["x"], w["y"], "blue", 3)
    for i, pts in enumerate(m["RAIL"]["data"]):
        xy = [(p["x"], p["y"]) for p in pts]
        if len(xy) > 1:
            d.line(xy, fill="lime", width=2)
        for p in pts:
            dot(p["x"], p["y"], "lime", 2, p.get("name"))
        if xy:
            d.text((xy[0][0] + 4, xy[0][1] + 4), f"rail{i}", fill="lime")
    for poly in m["GULP"]["data"]["polygons"]:
        pts = [tuple(p) for p in poly["points"]]
        if len(pts) > 1:
            d.polygon(pts, outline="purple")
        if pts and poly.get("name"):
            d.text(pts[0], poly["name"][:14], fill="purple")
    for p in m["GULP"]["data"]["points"]:
        dot(p["x"], p["y"], "purple", 2)
    for s in m["SKRO"]["data"]:
        dot(s["x"], s["y"], "gold", 6, (s.get("name") or "")[:10])
    for z in m["ZORG"]["data"]:
        dot(z["x"], z["y"], "pink", 5, f"Z{z['a']}/{z['b']}")
    for e in m["TING"]["data"]:
        dot(e["x"], e["y"], "cyan", 8, "TING")
        pts = [tuple(p) for p in e["poly"][1]]
        if len(pts) > 1:
            d.polygon(pts, outline="cyan")
    img.save(sys.argv[3])
    print("wrote", sys.argv[3])


if __name__ == "__main__":
    main()
