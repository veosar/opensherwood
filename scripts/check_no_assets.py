#!/usr/bin/env python3
"""Fail if any git-tracked file looks like game data (see docs/legal.md).

Checks: forbidden extensions (game formats and derived images), known magics of the game's formats, binaries
above a size limit, file names that exist in the retail DATA tree, and the private `re/` and `harness/goldens/`
roots. It cannot detect copied game prose: reviewers must check that (docs/legal.md). Run in CI and before every
commit.
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

FORBIDDEN_EXT = {
    ".rhs", ".rhp", ".rhm", ".scb", ".bks", ".dic", ".res", ".red", ".pak", ".sxt", ".map", ".min",
    ".bfn", ".tfn", ".fnt", ".ttc", ".sfk", ".fxg", ".cpf", ".bck", ".vid", ".bik", ".wav", ".ogg", ".mp3",
    ".bmp", ".exe", ".dll",
    # Ghidra projects, exports and archives (ADR-0009: analysis stays in the ignored `re/`)
    ".gpr", ".gzf", ".rep", ".gdt", ".fidb",
    # derived images: screenshots and decoded sheets of game data stay local
    ".png", ".jpg", ".jpeg", ".gif", ".webp", ".tif", ".tiff",
}
FORBIDDEN_ROOTS = ("re/", "harness/goldens/", "harness/captures/", "goldens/", "harness/out/", "harness/harness/")
# Replays and snapshots produced from a retail run carry the content fingerprint of the player's data.
RETAIL_MARKERS = (b'"content_fingerprint":"opensherwood', b'"content":"opensherwood')
FORBIDDEN_NAMES = {"continue", "continue_t", "restart", "restart_t", "profiles", "campaign.bck"}
MAGICS = [b"SRES", b"MEUH", b"DUTY", b"SBSCRIPT", b"SBFONT", b"SBTTFT", b"FXBK", b"SFPK", b"NEUF", b"GSHR",
          b"FORP", b"BIKi", bytes.fromhex("c9eb0300")]
MAX_BINARY_BYTES = 512 * 1024
# Decompiler idioms: a tracked text file with three or more distinct ones is refused (ADR-0009). The files that
# describe the rule itself are listed by path.
DECOMPILER_IDIOMS = [
    re.compile(rb"FUN_[0-9a-fA-F]{6,8}"), re.compile(rb"DAT_[0-9a-fA-F]{6,8}"),
    re.compile(rb"LAB_[0-9a-fA-F]{6,8}"), re.compile(rb"param_[0-9]+"),
    re.compile(rb"[iup]Var[0-9]+"), re.compile(rb"local_[0-9a-f]+"),
    re.compile(rb"undefined[1248]?"), re.compile(rb"__thiscall|__fastcall|__cdecl"),
    re.compile(rb"in_stack_[0-9a-f]+"), re.compile(rb"unaff_[A-Z]+"),
]
IDIOM_ALLOWED = {"scripts/check_no_assets.py", ".agents/skills/analyst-ghidra/SKILL.md",
                 ".claude/skills/analyst-ghidra/SKILL.md", "docs/decisions/ADR-0009-decompilation-driven-reimplementation.md"}


def tracked_files() -> list[Path]:
    out = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT, capture_output=True, check=True).stdout
    return [ROOT / p for p in out.decode("utf-8").split("\0") if p]


def is_binary(data: bytes) -> bool:
    return b"\0" in data[:8192]


def main() -> int:
    problems: list[str] = []
    for path in tracked_files():
        rel = path.relative_to(ROOT).as_posix()
        name = path.name.lower()
        if rel.lower().startswith(FORBIDDEN_ROOTS) or "/harness/out/" in rel.lower() or "/out/replays/" in rel.lower():
            problems.append(f"file under a local-only root: {rel}")
            continue
        if path.suffix.lower() in {".jsonl", ".json"} and path.exists():
            head = path.read_bytes()[:4096]
            if any(m in head for m in RETAIL_MARKERS) and not rel.startswith("harness/fixtures/"):
                problems.append(f"replay or snapshot with a retail content fingerprint: {rel}")
                continue
        if path.suffix.lower() in FORBIDDEN_EXT:
            problems.append(f"forbidden extension: {rel}")
            continue
        if name in FORBIDDEN_NAMES:
            problems.append(f"forbidden file name: {rel}")
            continue
        if not path.exists():
            continue
        head = path.read_bytes()[:8192]
        for magic in MAGICS:
            if head.startswith(magic):
                problems.append(f"game format magic {magic!r}: {rel}")
        if is_binary(head) and path.stat().st_size > MAX_BINARY_BYTES:
            problems.append(f"large binary file ({path.stat().st_size} bytes): {rel}")
        if not is_binary(head) and rel not in IDIOM_ALLOWED and path.stat().st_size < 4 * 1024 * 1024:
            text = path.read_bytes()
            hits = [i.pattern for i in DECOMPILER_IDIOMS if i.search(text)]
            if len(hits) >= 3:
                problems.append(f"looks like decompiler output ({len(hits)} idioms): {rel}")
    for p in problems:
        print(p)
    if problems:
        return 1
    print("policy check passed: no forbidden extensions, magics, roots, large binaries or decompiler idioms in "
          "tracked files (copied prose is not detected by this script)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
