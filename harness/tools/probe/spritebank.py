"""Loader for the sprite bank (`robinhood.dic` + `robinhood.bks`) used by the RE scripts.

Generic: reads the files named by OPENSHERWOOD_GAME_DIR, embeds no game bytes.
"""
import os
import struct
import sys

import numpy as np

REC = struct.Struct("<HHIIH")
REC_SIZE = REC.size  # 14


def game_dir():
    d = os.environ.get("OPENSHERWOOD_GAME_DIR")
    if not d:
        sys.exit("OPENSHERWOOD_GAME_DIR not set")
    return d


class Bank:
    def __init__(self, root=None):
        root = root or game_dir()
        self.dic_path = os.path.join(root, "DATA", "robinhood.dic")
        self.bks_path = os.path.join(root, "DATA", "robinhood.bks")
        with open(self.dic_path, "rb") as f:
            self.dic = f.read()
        self.generation, self.page_count, self.symbols_per_page = struct.unpack_from("<IHH", self.dic, 0)
        self.table_start = self._find_table_start()
        n = (len(self.dic) - self.table_start) // REC_SIZE
        raw = np.frombuffer(self.dic, dtype=np.uint8, count=n * REC_SIZE, offset=self.table_start)
        rec = raw.reshape(n, REC_SIZE)
        self.width = rec[:, 0:2].copy().view("<u2").ravel()
        self.height = rec[:, 2:4].copy().view("<u2").ravel()
        self.offset = rec[:, 4:8].copy().view("<u4").ravel()
        self.length = rec[:, 8:12].copy().view("<u4").ravel()
        self.page = rec[:, 12:14].copy().view("<u2").ravel()
        self.region = self.dic[8:self.table_start]
        self.bks = open(self.bks_path, "rb")

    def _find_table_start(self):
        total = len(self.dic)
        start = total - REC_SIZE
        while start - REC_SIZE >= 8:
            p = REC.unpack_from(self.dic, start - REC_SIZE)
            c = REC.unpack_from(self.dic, start)
            if p[2] + p[3] != c[2]:
                break
            start -= REC_SIZE
        return start

    def stream(self, i):
        """Symbol stream of frame i as a uint16 numpy array."""
        self.bks.seek(int(self.offset[i]))
        b = self.bks.read(int(self.length[i]))
        return np.frombuffer(b, dtype="<u2")

    def frames_of_page(self, p):
        return np.nonzero(self.page == p)[0]


def rgb565_to_rgb8(v):
    v = np.asarray(v, dtype=np.uint32)
    r = (v >> 11) & 0x1F
    g = (v >> 5) & 0x3F
    b = v & 0x1F
    return np.stack([(r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2)], axis=-1).astype(np.uint8)


def write_png(path, rgba):
    """Write an HxWx4 uint8 array as PNG without external deps."""
    import zlib
    h, w, _ = rgba.shape
    raw = b"".join(b"\x00" + rgba[y].tobytes() for y in range(h))

    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)
