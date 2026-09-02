"""Dump a slice of one chunk: python probe_dump.py <map> <TAG> [start] [n]"""
import sys
from rhp_chunks import load_chunks, rhp_path, hexdump
name, tag = sys.argv[1], sys.argv[2]
start = int(sys.argv[3], 0) if len(sys.argv) > 3 else 0
n = int(sys.argv[4], 0) if len(sys.argv) > 4 else 256
ver, body = load_chunks(rhp_path(name))[tag]
print(f"{name} {tag!r} v{ver} len={len(body)}")
print(hexdump(body, start, n))
