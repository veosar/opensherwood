#!/usr/bin/env python3
"""Fail if anything that is (or is about to be) committed looks like game data or decompiler output.

See docs/legal.md and ADR-0009. This is a heuristic safety net, never the rule: it cannot detect copied prose or
non-literal copying of program structure; reviewers must check that.

What is inspected, by mode:
  default          the blobs in the git INDEX (what `git commit` would record), not the working tree
  --range A..B     every blob added or changed by each commit in the range (what `git push` would publish)
  --paths P ...    the given working-tree files (drafts not yet staged)
  --selftest       synthetic fragments through the detectors; exits non-zero if any detector is broken

Checks: forbidden extensions (game formats, derived images, Ghidra project / archive / type files), paths under
Ghidra project directories (`*.rep/`, `*.gpr`), the private roots (`re/`, goldens, harness output), magics of the
game's formats, retail content fingerprints in replays / snapshots, large binaries, and text (UTF-8 or UTF-16)
carrying decompiler idioms or exporter signatures. A file too large to inspect is reported, never skipped.
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
    # derived images: screenshots and decoded sheets of game data stay local
    ".png", ".jpg", ".jpeg", ".gif", ".webp", ".tif", ".tiff",
    # Ghidra projects, archives, exports and type libraries (ADR-0009: analysis stays in the ignored `re/`)
    ".gpr", ".gzf", ".gar", ".gdt", ".fidb", ".prp", ".rep",
}
FORBIDDEN_ROOTS = ("re/", "harness/goldens/", "harness/captures/", "goldens/", "harness/out/", "harness/harness/")
# Replays and snapshots produced from a retail run carry the content fingerprint of the player's data.
RETAIL_MARKERS = (b'"content_fingerprint":"opensherwood', b'"content":"opensherwood')
FORBIDDEN_NAMES = {"continue", "continue_t", "restart", "restart_t", "profiles", "campaign.bck"}
MAGICS = [b"SRES", b"MEUH", b"DUTY", b"SBSCRIPT", b"SBFONT", b"SBTTFT", b"FXBK", b"SFPK", b"NEUF", b"GSHR",
          b"FORP", b"BIKi", bytes.fromhex("c9eb0300")]
MAX_BINARY_BYTES = 512 * 1024
MAX_INSPECT_BYTES = 32 * 1024 * 1024

# Decompiler idioms (Ghidra's default naming and helper macros). A text file with two or more distinct families
# is refused; one family alone is reported as a warning in the output but does not fail (ordinary prose can
# mention one token). Files that describe the rule itself are listed by exact path.
IDIOM_FAMILIES = {
    "auto-named function / data / label": re.compile(r"\b(?:FUN|DAT|LAB|PTR|thunk_FUN|switchD)_[0-9a-fA-F]{6,8}\b"),
    "auto-named parameter / local": re.compile(r"\b(?:param_\d+|local_[0-9a-f]{1,4}|[a-z]{1,2}Var\d+|[a-z]{1,2}Stack\d*|auStack_?\w*|in_stack_\w+|unaff_\w+|extraout_\w+|in_[A-Z]{2,3})\b"),
    "undefined types": re.compile(r"\bundefined(?:[1248])?\b"),
    "calling conventions": re.compile(r"\b__(?:thiscall|fastcall|cdecl|stdcall|regparm)\b"),
    "p-code helper macros": re.compile(r"\b(?:CONCAT\d\d|SUB\d\d|ZEXT\d\d|SEXT\d\d|SBORROW\d|CARRY\d|SCARRY\d)\("),
    "exporter signatures": re.compile(r"(?:^|\n)(?:// callers: |// callees: |address\tsize\tname\tcallers|address\tlength\tfunctions\ttext)"),
}
IDIOM_ALLOWED = {
    "scripts/check_no_assets.py",
    ".agents/skills/analyst-ghidra/SKILL.md",
    ".claude/skills/analyst-ghidra/SKILL.md",
    "docs/decisions/ADR-0009-decompilation-driven-reimplementation.md",
}
# The exporters' own source names their output columns; they are generic tooling, listed exactly.
IDIOM_ALLOWED |= {f"scripts/ghidra/{n}" for n in ("ExportInventory.java", "ExportStrings.java", "DecompileList.java", "DecompileAll.java")}


def git(*args: str) -> bytes:
    return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, check=True).stdout


def index_blobs() -> list[tuple[str, str]]:
    """(path, blob sha) for every entry of the index."""
    out = git("ls-files", "-s", "-z").decode("utf-8")
    entries = []
    for rec in out.split("\0"):
        if not rec:
            continue
        meta, path = rec.split("\t", 1)
        sha = meta.split(" ")[1]
        entries.append((path, sha))
    return entries


def range_blobs(rng: str) -> list[tuple[str, str]]:
    """(path, blob sha) for every blob added or modified by the commits in `rng` (A..B)."""
    commits = git("rev-list", rng).decode().split()
    seen: set[tuple[str, str]] = set()
    entries = []
    for c in commits:
        out = git("diff-tree", "--no-commit-id", "-r", "--diff-filter=AMCR", "-z", c).decode("utf-8", "replace")
        parts = out.split("\0")
        i = 0
        while i + 1 < len(parts):
            meta = parts[i]
            if not meta.startswith(":"):
                i += 1
                continue
            fields = meta.split(" ")
            sha = fields[3]
            status = fields[4]
            if status.startswith(("R", "C")):
                path = parts[i + 2]
                i += 3
            else:
                path = parts[i + 1]
                i += 2
            if sha == "0" * 40 or (path, sha) in seen:
                continue
            seen.add((path, sha))
            entries.append((path, sha))
    return entries


def blob_size(sha: str) -> int:
    return int(git("cat-file", "-s", sha).decode().strip())


def blob_bytes(sha: str, limit: int) -> bytes:
    p = subprocess.Popen(["git", "cat-file", "blob", sha], cwd=ROOT, stdout=subprocess.PIPE)
    assert p.stdout is not None
    data = p.stdout.read(limit)
    p.stdout.close()
    p.wait()
    return data


def is_binary(data: bytes) -> bool:
    return b"\0" in data[:8192] and not looks_utf16(data)


def looks_utf16(data: bytes) -> bool:
    return data[:2] in (b"\xff\xfe", b"\xfe\xff") or (len(data) >= 8 and data[1] == 0 and data[3] == 0 and data[0] != 0 and data[2] != 0)


def as_text(data: bytes) -> str:
    if data[:2] == b"\xff\xfe" or (looks_utf16(data) and data[:2] != b"\xfe\xff"):
        return data.decode("utf-16-le", "replace")
    if data[:2] == b"\xfe\xff":
        return data.decode("utf-16-be", "replace")
    return data.decode("utf-8", "replace")


def check_content(rel: str, data: bytes, size: int) -> list[str]:
    """Problems of one file's content; `size` is the full size, `data` at most MAX_INSPECT_BYTES of it."""
    problems: list[str] = []
    for magic in MAGICS:
        if data.startswith(magic):
            problems.append(f"game format magic {magic!r}: {rel}")
    if Path(rel).suffix.lower() in {".jsonl", ".json"} and any(m in data[:4096] for m in RETAIL_MARKERS) \
            and not rel.startswith("harness/fixtures/"):
        problems.append(f"replay or snapshot with a retail content fingerprint: {rel}")
    if size > MAX_INSPECT_BYTES:
        problems.append(f"too large to inspect ({size} bytes), review by hand: {rel}")
        return problems
    if is_binary(data):
        if size > MAX_BINARY_BYTES:
            problems.append(f"large binary file ({size} bytes): {rel}")
        return problems
    if rel in IDIOM_ALLOWED:
        return problems
    text = as_text(data)
    hits = [name for name, rx in IDIOM_FAMILIES.items() if rx.search(text)]
    if len(hits) >= 2 or "exporter signatures" in hits:
        problems.append(f"looks like decompiler output ({', '.join(hits)}): {rel}")
    return problems


def check_path(rel: str) -> list[str]:
    problems: list[str] = []
    low = rel.lower()
    parts = low.split("/")
    if low.startswith(FORBIDDEN_ROOTS) or "/harness/out/" in low or "/out/replays/" in low:
        problems.append(f"file under a local-only root: {rel}")
    if any(p.endswith(".rep") or p.endswith(".gpr") for p in parts[:-1]):
        problems.append(f"file inside a Ghidra project directory: {rel}")
    if Path(rel).suffix.lower() in FORBIDDEN_EXT:
        problems.append(f"forbidden extension: {rel}")
    if Path(rel).name.lower() in FORBIDDEN_NAMES:
        problems.append(f"forbidden file name: {rel}")
    return problems


def run(entries: list[tuple[str, str | None]]) -> list[str]:
    problems: list[str] = []
    for rel, sha in entries:
        problems.extend(check_path(rel))
        if sha is None:
            p = ROOT / rel
            if not p.exists():
                continue
            size = p.stat().st_size
            with p.open("rb") as f:
                data = f.read(MAX_INSPECT_BYTES)
        else:
            size = blob_size(sha)
            data = blob_bytes(sha, MAX_INSPECT_BYTES)
        problems.extend(check_content(rel, data, size))
    return problems


SELFTEST = [
    ("a.c", b"undefined4 FUN_00401000(int param_1) { return param_1; }", True),
    ("b.md", b"The dispatcher at FUN_00570a90 switches on the opcode; uVar3 holds the operand.", True),
    ("c.md", b"The rule is measured; see the function at virtual address 0x00570a90.", False),
    ("d.tsv", b"address\tsize\tname\tcallers\tcallees\tstring_refs\tfirst_string\n00401000\t12\tx\t1\t0\t0\t", True),
    ("e.txt", "undefined4 FUN_00401000(int param_1)".encode("utf-16-le"), True),
    ("f.md", b"One mention of param_1 in prose is tolerated.", False),
    ("re/notes/x.md", b"anything", True),
    ("analysis/demo.rep/idata/00/x.db", b"\0\0\0", True),
    ("proj.gar", b"PK", True),
    ("docs/x.png", b"\x89PNG", True),
]


def selftest() -> int:
    bad = 0
    for rel, data, expect in SELFTEST:
        problems = check_path(rel) + check_content(rel, data, len(data))
        if bool(problems) != expect:
            bad += 1
            print(f"selftest FAILED for {rel}: expected {'a problem' if expect else 'no problem'}, got {problems}")
    for rx in IDIOM_FAMILIES.values():
        if any(ord(ch) < 32 and ch not in "\t\n" for ch in rx.pattern):
            bad += 1
            print("selftest FAILED: a control character in an idiom pattern")
    print("selftest passed" if not bad else f"selftest: {bad} failures")
    return 1 if bad else 0


def main() -> int:
    args = sys.argv[1:]
    if args[:1] == ["--selftest"]:
        return selftest()
    if args[:1] == ["--range"] and len(args) >= 2:
        entries: list[tuple[str, str | None]] = list(range_blobs(args[1]))
        what = f"blobs published by {args[1]}"
    elif args[:1] == ["--paths"]:
        entries = [(Path(p).resolve().relative_to(ROOT).as_posix(), None) for p in args[1:]]
        what = "the given working-tree files"
    else:
        entries = list(index_blobs())
        what = "the index"
    problems = run(entries)
    for p in problems:
        print(p)
    if problems:
        return 1
    print(f"policy check passed on {what}: no forbidden extensions, magics, roots, large binaries or decompiler "
          "idioms (copied prose and non-literal copying are not detected by this script)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
