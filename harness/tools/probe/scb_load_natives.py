"""Natives reachable at load time per mission script (generic; no game bytes embedded).

The engine runs `Initialize` on every class (level first, then the elements in table order), then
`PostInitialize` on the level, then the first elements of the sequences those callbacks opened. This probe
computes, per file, the native ids reachable from those callbacks through same-class script calls (0x05),
in static order (an over-approximation: every branch is assumed taken), and compares them with a set of
*known* ids (implemented or stubbed by the engine) to report which ids would trap a strict VM at load.

Usage:
  python scb_load_natives.py <dir> [--known-rs <natives.rs>] [--known 1,2,3] [--callbacks Initialize,PostInitialize]
        --missions            per file: unknown ids at load in first-encounter order (the first one traps first)
        --ids                 per unknown id: files blocked, callbacks and classes it is reached from
        --context [--id N ...] [--limit K]   folded lines calling the given ids, over the whole corpus
        --all-unknown         also list unknown ids that occur only outside the load path, by frequency
"""
import collections
import os
import re
import sys

from scb_probe import parse
from scb_semantics import OP_CALL, OP_NATIVE, decode, files_of, functions, pseudo

DEFAULT_CALLBACKS = ("Initialize", "PostInitialize")


def known_from_rs(path):
    """Read the `IMPLEMENTED_NATIVES` and `STUB_NATIVES` arrays of the engine's natives.rs."""
    text = open(path, encoding="utf-8").read()
    ids = set()
    for name in ("IMPLEMENTED_NATIVES", "STUB_NATIVES"):
        m = re.search(name + r"[^=]*=\s*&\[([^\]]*)\]", text)
        if m:
            ids.update(int(x) for x in re.findall(r"\d+", m.group(1)))
    return ids


def call_graph(cls):
    """Function address -> (name, start, end, natives in order, callees in order)."""
    graph = {}
    q = cls["quads"]
    for fname, start, end, fn in functions(cls):
        natives, callees = [], []
        for i in range(start, end):
            op, a, b, c = decode(q, i)
            if op == OP_NATIVE:
                natives.append((i, a))
            elif op == OP_CALL:
                callees.append((i, a))
        graph[start] = (fname, start, end, natives, callees)
    return graph


def reachable(graph, start, seen=None):
    """Natives reachable from function `start`, in static order, following calls once each."""
    seen = set() if seen is None else seen
    out = []
    if start in seen or start not in graph:
        return out
    seen.add(start)
    fname, _, _, natives, callees = graph[start]
    events = sorted([(i, "n", nid) for i, nid in natives] + [(i, "c", addr) for i, addr in callees])
    for _, kind, v in events:
        if kind == "n":
            out.append((v, fname))
        else:
            out.extend(reachable(graph, v, seen))
    return out


def load_natives(classes, callbacks):
    """Yield (native id, class index, class name, root callback, function name) in engine load order."""
    order = []
    level = classes[0]
    for ci, cls in enumerate(classes):
        graph = call_graph(cls)
        by_name = {g[0]: addr for addr, g in graph.items()}
        for cb in callbacks:
            if cb == "PostInitialize":
                continue
            if cb in by_name:
                for nid, fname in reachable(graph, by_name[cb]):
                    order.append((nid, ci, cls["name"], cb, fname))
    if "PostInitialize" in callbacks:
        graph = call_graph(level)
        by_name = {g[0]: addr for addr, g in graph.items()}
        if "PostInitialize" in by_name:
            for nid, fname in reachable(graph, by_name["PostInitialize"]):
                order.append((nid, 0, level["name"], "PostInitialize", fname))
    return order


def main():
    args = sys.argv[1:]
    if not args or args[0].startswith("--"):
        print(__doc__)
        return
    files = files_of(args[0])
    known = set()
    if "--known-rs" in args:
        known |= known_from_rs(args[args.index("--known-rs") + 1])
    if "--known" in args:
        known |= {int(x) for x in args[args.index("--known") + 1].split(",") if x}
    callbacks = DEFAULT_CALLBACKS
    if "--callbacks" in args:
        callbacks = tuple(args[args.index("--callbacks") + 1].split(","))
    limit = int(args[args.index("--limit") + 1]) if "--limit" in args else 12
    only = {int(x) for x in args[args.index("--id") + 1 :] if x.isdigit()} if "--id" in args else set()

    per_file = {}
    per_id = collections.defaultdict(lambda: dict(files=set(), cbs=collections.Counter(), fns=collections.Counter(),
                                                  level=0, element=0, n=0))
    all_counts = collections.Counter()
    for f in files:
        base = os.path.basename(f)[:-4]
        _, classes, _ = parse(open(f, "rb").read())
        for cls in classes:
            for i in range(cls["nquads"]):
                op, a, b, c = decode(cls["quads"], i)
                if op == OP_NATIVE:
                    all_counts[a] += 1
        order = load_natives(classes, callbacks)
        unknown_order = []
        for nid, ci, cname, cb, fname in order:
            p = per_id[nid]
            p["files"].add(base)
            p["cbs"][cb] += 1
            p["fns"][fname] += 1
            p["n"] += 1
            p["level" if ci == 0 else "element"] += 1
            if nid not in known and nid not in unknown_order:
                unknown_order.append(nid)
        per_file[base] = (unknown_order, {nid for nid, *_ in order})

    if "--missions" in args:
        for base in sorted(per_file):
            unknown_order, allset = per_file[base]
            print(f"{base}: load natives={len(allset)} unknown at load={unknown_order or 'none'}")
        blocked = sum(1 for v in per_file.values() if v[0])
        print(f"{blocked} of {len(per_file)} files reach an unknown native at load")
    if "--ids" in args:
        for nid in sorted(per_id, key=lambda k: (-len(per_id[k]["files"]), k)):
            if nid in known and not only:
                continue
            if only and nid not in only:
                continue
            p = per_id[nid]
            print(f"native {nid}: files={len(p['files'])} calls_at_load={p['n']} level={p['level']} element={p['element']} "
                  f"callbacks={dict(p['cbs'])} corpus_calls={all_counts[nid]}")
            print(f"   via functions: {p['fns'].most_common(8)}")
            print(f"   files: {sorted(p['files'])}")
    if "--all-unknown" in args:
        load_ids = set(per_id)
        print("unknown ids not reached at load, by corpus frequency:")
        for nid, n in sorted(all_counts.items(), key=lambda kv: (-kv[1], kv[0])):
            if nid not in known and nid not in load_ids:
                print(f"   native {nid}: {n} calls")
    if "--context" in args:
        seen = collections.Counter()
        for f in files:
            _, classes, _ = parse(open(f, "rb").read())
            for cls in classes:
                fn_names = {fn[1]: fn[0] for fn in cls["funcs"]}
                for fname, start, end, fn in functions(cls):
                    lines = pseudo(cls, start, end, fn_names)
                    for k, line in enumerate(lines):
                        for nid in only:
                            tag = f"n{nid}("
                            if tag in line and seen[nid] < limit:
                                seen[nid] += 1
                                kind = "level" if cls["name"] == "StartUp" else "element"
                                print(f"[n{nid}] {os.path.basename(f)} {kind}.{fname}")
                                for m in range(max(0, k - 2), min(len(lines), k + 3)):
                                    print("      " + lines[m])
    if not any(x in args for x in ("--missions", "--ids", "--context", "--all-unknown")):
        print(__doc__)


if __name__ == "__main__":
    main()
