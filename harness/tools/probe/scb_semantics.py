"""Data-flow statistics and a folded pseudo-listing for SBSCRIPT 1.5 quads (generic; no game bytes embedded).

Everything here is observation over the compiled scripts only. The probe knows the container layout from
``scb_probe.parse`` and the operand encoding established in docs/formats/scb.md; it assigns *no* semantics of
its own beyond the labels used for reporting (push / native / return / jump), which are hypotheses under test.

Usage:
  python scb_semantics.py <dir> --natives [--id N ...]   per native id: arity, argument sources, return consumers
  python scb_semantics.py <dir> --ops                    contexts of the unnamed opcodes, jump directions, call arity
  python scb_semantics.py <dir> --imm                    which immediates feed which native argument (text/actor ids)
  python scb_semantics.py <file> --pseudo [--class NAME] [--fn NAME]   folded expression listing
  python scb_semantics.py <dir> --handlers               native ids per callback name (Initialize, EnterZone, ...)
  python scb_semantics.py <dir> --params                 parameter reads and comparisons per callback name
  python scb_semantics.py <dir> --messages               message ids sent (natives 43/44/109/110) vs handled
  python scb_semantics.py <dir> --find 25 26 ...         folded lines using the given opcodes (hex), with context
"""
import collections
import glob
import os
import struct
import sys

from scb_probe import QUAD, parse

# Storage classes of a u16 operand (top two bits), see scb.md.
NONE, CV, LV, TV = 0, 1, 2, 3

OP_PUSH_ARG = 0x02  # push before 0x05 (script call)
OP_ENTER = 0x03
OP_END = 0x04
OP_CALL = 0x05
OP_RET = 0x06
OP_RETVAL = 0x07
OP_GETPARAM = 0x08
OP_GETRET = 0x0A
OP_NPUSH = 0x0B
OP_NATIVE = 0x0C
OP_NRET = 0x0D
OP_JMP = 0x0E
OP_JZ = 0x0F
OP_MOV = 0x11
OP_MOVI = 0x13
OP_MOVF = 0x14
THREE_OP = set(range(0x19, 0x2D))
TWO_OP = {0x11, 0x12, 0x15, 0x16, 0x18}


def decode(quads, i):
    op = quads[i * QUAD]
    a, b = struct.unpack_from("<HH", quads, i * QUAD + 1)
    c = struct.unpack_from("<I", quads, i * QUAD + 5)[0]
    return op, a, b, c


def storage(v):
    return v >> 14, v & 0x3FFF


def files_of(target):
    if os.path.isdir(target):
        return sorted(glob.glob(os.path.join(target, "*.scb")))
    return [target]


def functions(cls):
    """Yield (name, start, end) over the function table (functions are laid out in table order)."""
    fns = cls["funcs"]
    for k, fn in enumerate(fns):
        start = fn[1]
        end = fns[k + 1][1] if k + 1 < len(fns) else cls["nquads"]
        yield fn[0], start, end, fn


def var_name(cls, v):
    s, off = storage(v)
    if s == CV:
        for t, tname, vn, voff in cls["vars"]:
            if voff == off:
                return vn + (f":{tname}" if tname else "")
        return f"cv{off}"
    if s == LV:
        return f"local{off}"
    if s == TV:
        return f"t{off}"
    return f"#{v}"


def var_type(cls, v):
    """Type label of an operand: 'imm', 'Actor', 'Location', 'int' (tag-2 class var), 'local', 'temp'."""
    s, off = storage(v)
    if s == CV:
        for t, tname, vn, voff in cls["vars"]:
            if voff == off:
                return tname if tname else "cvar"
        return "cvar"
    return {LV: "local", TV: "temp"}.get(s, "none")


class Flow:
    """Per-function forward walk that records where temps come from (last writer) and where they go."""

    def __init__(self, cls, start, end):
        self.cls = cls
        self.quads = cls["quads"]
        self.start, self.end = start, end
        self.writer = {}  # temp operand -> description of last writer ("imm:5", "nat:3", "cvar:Actor", ...)

    def describe_source(self, v):
        s, off = storage(v)
        if s == TV:
            return self.writer.get(v, "temp:?")
        if s == CV:
            return "cvar:" + var_type(self.cls, v)
        if s == LV:
            return "local"
        return "none"

    def walk(self, on_native=None, on_use=None):
        """Walk the function. on_native(i, nid, arg_sources, ret_temp_or_None); on_use(temp_writer, use_desc)."""
        q = self.quads
        pending = []  # sources pushed by 0x0b since the last native
        pending_args = []  # sources pushed by 0x02 since the last call
        i = self.start
        while i < self.end:
            op, a, b, c = decode(q, i)
            if op == OP_MOVI:
                self.writer[a] = f"imm:{struct.unpack('<i', struct.pack('<I', c))[0]}"
            elif op == OP_MOVF:
                self.writer[a] = f"flt:{struct.unpack('<f', struct.pack('<I', c))[0]:g}"
            elif op == OP_MOV:
                src = self.describe_source(b)
                if on_use:
                    on_use(src, "mov->" + var_type(self.cls, a))
                self.writer[a] = "mov:" + src
            elif op == OP_GETPARAM:
                self.writer[a] = f"param:{c}"
            elif op == OP_NPUSH:
                pending.append(self.describe_source(a))
            elif op == OP_PUSH_ARG:
                pending_args.append(self.describe_source(a))
            elif op == OP_NATIVE:
                nid = a
                nxt = decode(q, i + 1) if i + 1 < self.end else (0, 0, 0, 0)
                ret = nxt[1] if nxt[0] == OP_NRET else None
                if on_native:
                    on_native(i, nid, list(pending), ret)
                for k, src in enumerate(pending):
                    if on_use:
                        on_use(src, f"nat{nid}[{k}]")
                pending = []
                if ret is not None:
                    self.writer[ret] = f"nat:{nid}"
                    i += 1
            elif op == OP_CALL:
                for k, src in enumerate(pending_args):
                    if on_use:
                        on_use(src, f"call[{k}]")
                pending_args = []
                nxt = decode(q, i + 1) if i + 1 < self.end else (0, 0, 0, 0)
                if nxt[0] == OP_GETRET:
                    self.writer[nxt[1]] = "callret"
                    i += 1
            elif op == OP_JZ:
                if on_use:
                    on_use(self.describe_source(a), "jz")
            elif op == OP_RETVAL:
                if on_use:
                    on_use(self.describe_source(a), "retval")
            elif op in THREE_OP:
                sb, sc = self.describe_source(b), self.describe_source(c & 0xFFFF)
                if on_use:
                    on_use(sb, f"op{op:02x}.b")
                    on_use(sc, f"op{op:02x}.c")
                self.writer[a] = f"op{op:02x}"
            elif op in TWO_OP and op != OP_MOV:
                if on_use:
                    on_use(self.describe_source(b), f"op{op:02x}.b")
                self.writer[a] = f"op{op:02x}"
            i += 1


def bucket(src):
    """Coarse class of a source description for histograms."""
    if src.startswith("imm:"):
        return "imm"
    if src.startswith("flt:"):
        return "float"
    if src.startswith("nat:"):
        return src
    if src.startswith("mov:"):
        return "mov:" + bucket(src[4:])
    return src


def cmd_natives(files, only=None):
    per = collections.defaultdict(lambda: dict(n=0, arity=collections.Counter(), ret=0, args=collections.defaultdict(collections.Counter),
                                              imm=collections.defaultdict(collections.Counter), uses=collections.Counter(), fns=collections.Counter(),
                                              classes=collections.Counter()))
    for f in files:
        _, classes, _ = parse(open(f, "rb").read())
        for cls in classes:
            kind = "level" if cls["name"] == "StartUp" else "element"
            for fname, start, end, fn in functions(cls):
                flow = Flow(cls, start, end)

                def on_native(i, nid, args, ret, fname=fname, kind=kind):
                    p = per[nid]
                    p["n"] += 1
                    p["arity"][len(args)] += 1
                    p["ret"] += ret is not None
                    p["fns"][fname] += 1
                    p["classes"][kind] += 1
                    for k, src in enumerate(args):
                        p["args"][k][bucket(src)] += 1
                        if src.startswith("imm:"):
                            p["imm"][k][int(src[4:])] += 1

                def on_use(src, use):
                    if src.startswith("nat:"):
                        per[int(src[4:])]["uses"][use] += 1

                flow.walk(on_native, on_use)
    for nid in sorted(per):
        if only and nid not in only:
            continue
        p = per[nid]
        print(f"native {nid}: n={p['n']} arity={dict(p['arity'])} ret_used={p['ret']}/{p['n']} classes={dict(p['classes'])}")
        print(f"   in: {p['fns'].most_common(6)}")
        for k in sorted(p["args"]):
            top = p["args"][k].most_common(6)
            line = f"   arg{k}: {top}"
            if p["imm"][k]:
                vals = p["imm"][k]
                line += f"  imm: distinct={len(vals)} min={min(vals)} max={max(vals)} top={vals.most_common(8)}"
            print(line)
        if p["uses"]:
            print(f"   ret used as: {p['uses'].most_common(8)}")


def cmd_ops(files):
    prev = collections.defaultdict(collections.Counter)
    nxt = collections.defaultdict(collections.Counter)
    jz_dir = collections.Counter()
    jmp_dir = collections.Counter()
    loop_shapes = collections.Counter()
    call_arity = collections.Counter()
    getparam = collections.Counter()
    ret_pos = collections.Counter()
    src_of = collections.defaultdict(collections.Counter)
    cmp_imm = collections.defaultdict(collections.Counter)
    jz_src = collections.Counter()
    for f in files:
        _, classes, _ = parse(open(f, "rb").read())
        for cls in classes:
            q = cls["quads"]
            addr_to_fn = {fn[1]: fn for fn in cls["funcs"]}
            for fname, start, end, fn in functions(cls):
                params = set()
                npush = 0
                for i in range(start, end):
                    op, a, b, c = decode(q, i)
                    if i > start:
                        prev[op][decode(q, i - 1)[0]] += 1
                    if i + 1 < end:
                        nxt[op][decode(q, i + 1)[0]] += 1
                    if op == OP_JZ:
                        jz_dir["forward" if c > i else "backward"] += 1
                        # shape: is the instruction before the target an unconditional backward jump (while loop)?
                        if c > i and c - 1 < end:
                            pop, pa, _, _ = decode(q, c - 1)
                            if pop == OP_JMP and pa <= i:
                                loop_shapes["jz_forward_over_backjump(while)"] += 1
                            elif pop == OP_JMP:
                                loop_shapes["jz_forward_target_after_forward_jmp(if/else)"] += 1
                            else:
                                loop_shapes["jz_forward_plain(if)"] += 1
                    if op == OP_JMP:
                        if storage(a)[0] == NONE:
                            jmp_dir["forward" if a > i else "backward"] += 1
                    if op == OP_PUSH_ARG:
                        npush += 1
                    if op == OP_CALL:
                        callee = addr_to_fn.get(a)
                        if callee:
                            call_arity[(npush, callee[2], callee[3], callee[4])] += 1
                        npush = 0
                    if op == OP_GETPARAM:
                        params.add(c)
                    if op == OP_RETVAL:
                        nop = decode(q, i + 1)[0] if i + 1 < end else None
                        ret_pos[f"retval->{nop:#04x}" if nop is not None else "retval->end"] += 1
                if params:
                    getparam[(fn[2], fn[3], fn[4], tuple(sorted(params)))] += 1
                # operand sources of the compare / arithmetic ops and jz
                flow = Flow(cls, start, end)

                def on_use(src, use):
                    if use.startswith("op"):
                        src_of[use][bucket(src)] += 1
                        if src.startswith("imm:"):
                            cmp_imm[use[:4]][int(src[4:])] += 1
                    if use == "jz":
                        jz_src[bucket(src)] += 1

                flow.walk(None, on_use)
    print("0x0f (cond jump) direction:", dict(jz_dir))
    print("0x0f shapes:", dict(loop_shapes))
    print("0x0e (jump) direction:", dict(jmp_dir))
    print("0x0f operand sources:", jz_src.most_common(12))
    print("0x07 followed by:", dict(ret_pos))
    print("script call: (0x02 pushes, callee unknown_0, unknown_1, unknown_2) ->", sorted(call_arity.items()))
    print("0x08 offsets per function (unknown_0, unknown_1, unknown_2, offsets) ->", sorted(getparam.items())[:40])
    for op in sorted(prev):
        if op in (0x12, 0x15, 0x18, 0x07, 0x0A, 0x02, 0x11) or op in THREE_OP:
            print(f"op {op:#04x}: prev={prev[op].most_common(5)} next={nxt[op].most_common(5)}")
    for use in sorted(src_of):
        print(f"{use}: {src_of[use].most_common(8)}")
    for op in sorted(cmp_imm):
        vals = cmp_imm[op]
        print(f"{op} immediates: distinct={len(vals)} top={vals.most_common(10)}")


def cmd_imm(files):
    """For each (native, arg) fed by immediates: per file max, to compare with mission tables (text lists, actors)."""
    per_file = collections.defaultdict(lambda: collections.defaultdict(set))
    for f in files:
        base = os.path.basename(f)
        _, classes, _ = parse(open(f, "rb").read())
        for cls in classes:
            for fname, start, end, fn in functions(cls):
                flow = Flow(cls, start, end)

                def on_native(i, nid, args, ret, base=base):
                    for k, src in enumerate(args):
                        if src.startswith("imm:"):
                            per_file[(nid, k)][base].add(int(src[4:]))

                flow.walk(on_native, None)
    for key in sorted(per_file):
        rows = per_file[key]
        allv = set().union(*rows.values())
        if len(allv) < 3:
            continue
        print(f"native {key[0]} arg{key[1]}: files={len(rows)} distinct={len(allv)} range={min(allv)}..{max(allv)}")
        for base in sorted(rows):
            v = sorted(rows[base])
            print(f"    {base}: n={len(v)} max={v[-1]} {v[:24]}{'...' if len(v) > 24 else ''}")


def cmd_handlers(files):
    by_fn = collections.defaultdict(collections.Counter)
    for f in files:
        _, classes, _ = parse(open(f, "rb").read())
        for cls in classes:
            for fname, start, end, fn in functions(cls):
                for i in range(start, end):
                    op, a, b, c = decode(cls["quads"], i)
                    if op == OP_NATIVE:
                        by_fn[fname][a] += 1
    for fname in sorted(by_fn):
        print(f"{fname}: {by_fn[fname].most_common(15)}")


def pseudo(cls, start, end, fn_names):
    """Fold the push / native / return idiom into expressions; print labels for jump targets."""
    q = cls["quads"]
    targets = set()
    for i in range(start, end):
        op, a, b, c = decode(q, i)
        if op == OP_JZ:
            targets.add(c)
        if op == OP_JMP and storage(a)[0] == NONE:
            targets.add(a)
    expr = {}
    pending, pending_args = [], []

    def val(v):
        s, off = storage(v)
        if s == TV and v in expr:
            return expr[v]
        return var_name(cls, v)

    out = []
    i = start
    while i < end:
        op, a, b, c = decode(q, i)
        label = f"L{i}:" if i in targets else ""
        line = None
        if op == OP_ENTER:
            line = f"enter volatile={a} tempor={b}"
        elif op == OP_END:
            line = "end"
        elif op == OP_RET:
            line = "return"
        elif op == OP_RETVAL:
            line = f"return_value {val(a)}"
        elif op == OP_GETPARAM:
            expr[a] = f"param{c // 4}"
        elif op == OP_MOVI:
            expr[a] = str(struct.unpack("<i", struct.pack("<I", c))[0])
            if storage(a)[0] != TV:
                line = f"{var_name(cls, a)} = {expr[a]}"
        elif op == OP_MOVF:
            expr[a] = f"{struct.unpack('<f', struct.pack('<I', c))[0]:g}f"
            if storage(a)[0] != TV:
                line = f"{var_name(cls, a)} = {expr[a]}"
        elif op == OP_MOV:
            if storage(a)[0] == TV:
                expr[a] = val(b)
            else:
                line = f"{var_name(cls, a)} = {val(b)}"
        elif op == OP_NPUSH:
            pending.append(val(a))
        elif op == OP_PUSH_ARG:
            pending_args.append(val(a))
        elif op == OP_NATIVE:
            call = f"n{a}({', '.join(pending)})"
            pending = []
            nxt = decode(q, i + 1) if i + 1 < end else (0, 0, 0, 0)
            if nxt[0] == OP_NRET:
                expr[nxt[1]] = call
                i += 1
                # show the call only when its value is consumed later by a statement; keep listing compact
            else:
                line = call
        elif op == OP_CALL:
            call = f"{fn_names.get(a, f'fn@{a}')}({', '.join(pending_args)})"
            pending_args = []
            nxt = decode(q, i + 1) if i + 1 < end else (0, 0, 0, 0)
            if nxt[0] == OP_GETRET:
                expr[nxt[1]] = call
                i += 1
            else:
                line = call
        elif op == OP_JMP:
            line = f"goto L{a}"
        elif op == OP_JZ:
            line = f"jz {val(a)} -> L{c}"
        elif op in THREE_OP:
            expr[a] = f"({val(b)} op{op:02x} {val(c & 0xFFFF)})"
            if storage(a)[0] != TV:
                line = f"{var_name(cls, a)} = {expr[a]}"
        elif op in TWO_OP:
            expr[a] = f"op{op:02x}({val(b)})"
            if storage(a)[0] != TV:
                line = f"{var_name(cls, a)} = {expr[a]}"
        elif op == 0x01:
            pass
        else:
            line = f"op{op:02x} a={a:#x} b={b:#x} c={c:#x}"
        if line is not None or label:
            out.append(f"{i:5d} {label:<8}{line or ''}")
        i += 1
    return out


def cmd_pseudo(path, only_class=None, only_fn=None):
    _, classes, _ = parse(open(path, "rb").read())
    for cls in classes:
        if only_class and cls["name"] != only_class:
            continue
        print(f"class {cls['name']}: vars {[(v[2], v[1] or 'int') for v in cls['vars']]}")
        fn_names = {fn[1]: fn[0] for fn in cls["funcs"]}
        for fname, start, end, fn in functions(cls):
            if only_fn and fname != only_fn:
                continue
            print(f"  fn {fname} @{start} ({end - start} quads) u0={fn[2]} u1={fn[3]} u2={fn[4]}")
            for line in pseudo(cls, start, end, fn_names):
                print("    " + line)


def cmd_find(files, opcodes, limit=20):
    """Print folded lines that use the given opcodes (as opXX), with the next line, over the corpus."""
    seen = collections.Counter()
    for f in files:
        _, classes, _ = parse(open(f, "rb").read())
        for cls in classes:
            fn_names = {fn[1]: fn[0] for fn in cls["funcs"]}
            for fname, start, end, fn in functions(cls):
                lines = pseudo(cls, start, end, fn_names)
                for k, line in enumerate(lines):
                    for op in opcodes:
                        tag = f"op{op:02x}"
                        if tag in line and seen[op] < limit:
                            seen[op] += 1
                            print(f"[{tag}] {os.path.basename(f)} {cls['name']}.{fname}")
                            for m in range(max(0, k - 1), min(len(lines), k + 3)):
                                print("      " + lines[m])


def cmd_messages(files):
    """Message ids: sent by natives 43 / 44 (arg 1) versus compared against param0 in ProcessMessage."""
    total_sent, total_cmp = collections.Counter(), collections.Counter()
    for f in files:
        sent, cmp_ = set(), set()
        _, classes, _ = parse(open(f, "rb").read())
        for cls in classes:
            for fname, start, end, fn in functions(cls):
                flow = Flow(cls, start, end)

                def on_native(i, nid, args, ret):
                    if nid in (43, 44, 109, 110) and len(args) > 1 and args[1].startswith("imm:"):
                        sent.add(int(args[1][4:]))

                cmp_local = []

                def on_use(src, use, fname=fname):
                    if fname == "ProcessMessage" and use == "op29.c" and src.startswith("imm:"):
                        cmp_local.append(int(src[4:]))

                flow.walk(on_native, on_use)
                # only count immediates compared against param0: approximate by pairing with the b-source
                flow2 = Flow(cls, start, end)
                pairs = []

                def on_use2(src, use, pairs=pairs):
                    if use == "op29.b":
                        pairs.append([src, None])
                    elif use == "op29.c" and pairs and pairs[-1][1] is None:
                        pairs[-1][1] = src

                flow2.walk(None, on_use2)
                if fname == "ProcessMessage":
                    for b, c in pairs:
                        if b == "param:0" and c and c.startswith("imm:"):
                            cmp_.add(int(c[4:]))
        for v in sent:
            total_sent[v] += 1
        for v in cmp_:
            total_cmp[v] += 1
        print(f"{os.path.basename(f)}: sent={sorted(sent)} compared-not-sent={sorted(cmp_ - sent)}")
    print("compared in ProcessMessage but never sent by a script (files):", sorted(total_cmp.items()))
    print("sent by scripts (files):", sorted(total_sent.items()))


def cmd_params(files):
    """Per callback name: parameter reads (0x08 offsets) and the immediates compared against each parameter."""
    reads = collections.defaultdict(collections.Counter)
    cmp_ = collections.defaultdict(collections.Counter)
    sig = collections.defaultdict(collections.Counter)
    for f in files:
        _, classes, _ = parse(open(f, "rb").read())
        for cls in classes:
            for fname, start, end, fn in functions(cls):
                sig[fname][(fn[2], fn[3], fn[4])] += 1
                offs = set()
                for i in range(start, end):
                    op, a, b, c = decode(cls["quads"], i)
                    if op == OP_GETPARAM:
                        offs.add(c)
                reads[fname][tuple(sorted(offs))] += 1
                flow = Flow(cls, start, end)
                pairs = []

                def on_use(src, use, pairs=pairs):
                    if use.endswith(".b") and use.startswith("op2"):
                        pairs.append([use[:4], src, None])
                    elif use.endswith(".c") and pairs and pairs[-1][2] is None:
                        pairs[-1][2] = src

                flow.walk(None, on_use)
                for op, b, c in pairs:
                    if b.startswith("param:") and c:
                        cmp_[fname][(b, op, c if not c.startswith("imm:") else "imm")] += 1
                    if b.startswith("param:") and c and c.startswith("imm:"):
                        cmp_[fname][(b, "value", int(c[4:]))] += 1
    for fname in sorted(sig):
        if sum(sig[fname].values()) < 5:
            continue
        print(f"{fname}: signatures={dict(sig[fname])} reads={dict(reads[fname])}")
        if cmp_[fname]:
            print(f"    compared: {cmp_[fname].most_common(16)}")


def main():
    args = sys.argv[1:]
    target = args[0]
    files = files_of(target)
    if "--find" in args:
        ops = [int(x, 16) for x in args[args.index("--find") + 1 :] if not x.startswith("--")]
        cmd_find(files, ops)
    elif "--messages" in args:
        cmd_messages(files)
    elif "--params" in args:
        cmd_params(files)
    elif "--natives" in args:
        only = {int(x) for x in args[args.index("--id") + 1 :] if x.isdigit()} if "--id" in args else None
        cmd_natives(files, only)
    elif "--ops" in args:
        cmd_ops(files)
    elif "--imm" in args:
        cmd_imm(files)
    elif "--handlers" in args:
        cmd_handlers(files)
    elif "--pseudo" in args:
        cls = args[args.index("--class") + 1] if "--class" in args else None
        fn = args[args.index("--fn") + 1] if "--fn" in args else None
        cmd_pseudo(files[0], cls, fn)
    else:
        print(__doc__)


if __name__ == "__main__":
    main()
