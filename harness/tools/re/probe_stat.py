"""STAT: u16 n; then records. Explore the record separators by scanning for the pattern after a polygon."""
import struct, sys
from rhp_chunks import load_chunks, rhp_path, map_size, hexdump
m = sys.argv[1]
ver, b = load_chunks(rhp_path(m))["STAT"]; W, H = map_size(m)
n = struct.unpack_from("<H", b, 0)[0]
print(m, (W, H), "n", n, "len", len(b))
print(hexdump(b, 0, 16))
# hypothesis: record = u16 id?, u32, u8, u8 tag?, ... find all occurrences of b'\x82\x00\x00\x00\x00\x00\x00'
import re
for mm in re.finditer(rb"\x82\x00\x00\x00\x00\x00\x00", b):
    p = mm.start()
    print(f"{p:06x}: {b[p-8:p+24].hex(' ')}")
    if p > 3000: break
