#!/usr/bin/env python3
"""Dump bytes of a PE image at a virtual address (analyst tooling, ADR-0009).

Maps the virtual address through the section table to a file offset and prints a hex dump plus the same bytes
as little-endian u16 / u32 / i32 / f32 columns, so an analyst reading decompiled code can look at the data a
function references without holding the Ghidra project open. Nothing is written; the output goes to stdout
and must stay in the analyst's workspace (never in the repository).

Usage: python scripts/ghidra/peek.py <exe> <hex address> [length] [--u16|--u32|--i32|--f32|--str]
"""
from __future__ import annotations

import struct
import sys
from pathlib import Path


def sections(data: bytes):
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    assert data[pe:pe + 4] == b"PE\0\0", "not a PE image"
    nsec = struct.unpack_from("<H", data, pe + 6)[0]
    opt_size = struct.unpack_from("<H", data, pe + 20)[0]
    base = struct.unpack_from("<I", data, pe + 24 + 28)[0]
    off = pe + 24 + opt_size
    out = []
    for i in range(nsec):
        s = off + i * 40
        name = data[s:s + 8].rstrip(b"\0").decode("ascii", "replace")
        vsize, vaddr, rsize, raw = struct.unpack_from("<IIII", data, s + 8)
        out.append((name, base + vaddr, vsize, raw, rsize))
    return base, out


def to_offset(data: bytes, va: int) -> int | None:
    _, secs = sections(data)
    for _name, start, vsize, raw, rsize in secs:
        if start <= va < start + max(vsize, rsize):
            rel = va - start
            return raw + rel if rel < rsize else None
    return None


def main() -> int:
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    data = Path(sys.argv[1]).read_bytes()
    va = int(sys.argv[2], 16)
    length = int(sys.argv[3], 0) if len(sys.argv) > 3 and not sys.argv[3].startswith("--") else 64
    mode = next((a for a in sys.argv[3:] if a.startswith("--")), "--hex")
    off = to_offset(data, va)
    if off is None:
        print(f"{va:#x}: not backed by file bytes (uninitialised data or outside the image)")
        return 1
    chunk = data[off:off + length]
    if mode == "--str":
        print(chunk.split(b"\0")[0].decode("latin-1", "replace"))
        return 0
    fmt = {"--u16": ("<H", 2), "--u32": ("<I", 4), "--i32": ("<i", 4), "--f32": ("<f", 4)}.get(mode)
    if fmt:
        code, size = fmt
        vals = [struct.unpack_from(code, chunk, i)[0] for i in range(0, len(chunk) - size + 1, size)]
        for i in range(0, len(vals), 8):
            row = vals[i:i + 8]
            print(f"{va + i * size:08x}: " + " ".join(f"{v:>12}" if not isinstance(v, float) else f"{v:>12.5g}" for v in row))
        return 0
    for i in range(0, len(chunk), 16):
        row = chunk[i:i + 16]
        print(f"{va + i:08x}: {row.hex(' '):<48} {''.join(chr(b) if 32 <= b < 127 else '.' for b in row)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
