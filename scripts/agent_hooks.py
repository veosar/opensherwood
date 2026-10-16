#!/usr/bin/env python3
"""Claude Code project hooks: Rust baseline on SessionStart, compile guard on Stop."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

import sync_skills

ROOT = Path(__file__).resolve().parent.parent
CHECK = ["cargo", "check", "--workspace", "--all-targets", "--locked"]


def fingerprint(root: Path) -> str:
    files = (
        subprocess.check_output(
            [
                "git",
                "-C",
                str(root),
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "-z",
            ],
            timeout=10,
        )
        .decode("utf-8")
        .split("\0")
    )
    digest = hashlib.sha256()
    for name in sorted(set(files)):
        path = Path(name)
        relevant = (
            name in {"Cargo.toml", "Cargo.lock", "rust-toolchain", "rust-toolchain.toml"}
            or (path.parts and path.parts[0] == "crates")
            or name in {".cargo/config", ".cargo/config.toml"}
        )
        if relevant:
            digest.update(name.encode("utf-8") + b"\0")
            file = root / name
            digest.update(file.read_bytes() if file.is_file() else b"<deleted>")
    # A toolchain change must invalidate a previously successful check too.
    digest.update(
        json.dumps(
            {
                k: os.environ.get(k)
                for k in (
                    "RUSTUP_TOOLCHAIN",
                    "RUSTFLAGS",
                    "CARGO_ENCODED_RUSTFLAGS",
                    "CARGO_BUILD_TARGET",
                )
            },
            sort_keys=True,
        ).encode()
    )
    return digest.hexdigest()


def state_path(root: Path, data: dict) -> Path:
    sid = hashlib.sha256(str(data.get("session_id") or "manual").encode()).hexdigest()[:24]
    return root / ".local" / "agent-hooks" / f"{sid}.json"


def read_state(path: Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}


def write_state(path: Path, state: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(f".{os.getpid()}.tmp")
    temporary.write_text(json.dumps(state), encoding="utf-8")
    os.replace(temporary, path)


def mirror_problem(root: Path) -> str:
    source, target = root / ".agents" / "skills", root / ".claude" / "skills"
    if sync_skills.file_map(source) != sync_skills.file_map(target) or not (target / sync_skills.MARKER).is_file():
        return "Skills differ: run python scripts/sync_skills.py --write, then --check."
    return ""


def start(root: Path, data: dict) -> dict:
    path = state_path(root, data)
    if not path.exists():
        write_state(path, {"baseline": fingerprint(root)})
    problem = mirror_problem(root)
    if not problem:
        return {}
    return {
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": problem,
        }
    }


def stop(root: Path, data: dict) -> dict:
    if data.get("stop_hook_active"):
        return {}
    problem = mirror_problem(root)
    if problem:
        return {"decision": "block", "reason": problem}
    path = state_path(root, data)
    state = read_state(path)
    current = fingerprint(root)
    if current == state.get("checked"):
        if not state.get("ok"):
            return {
                "systemMessage": "The last automatic cargo check failed; report the failure or a subsequent manual verification. "
                "An unchanged failure is not retried automatically."
            }
        return {}
    if current == state.get("baseline"):
        return {}
    try:
        result = compile_check(root)
        ok = result.returncode == 0
        output = result.stdout + result.stderr
    except (OSError, subprocess.TimeoutExpired) as error:
        ok, output = False, str(error)
    # Do not certify code that changed while cargo was running.
    if fingerprint(root) != current:
        return {
            "decision": "block",
            "reason": "Rust inputs changed during cargo check. Verify the current version.",
        }
    write_state(path, {**state, "checked": current, "ok": ok})
    if ok:
        return {}
    return {
        "decision": "block",
        "reason": (
            "cargo check --workspace --all-targets --locked failed or could not finish. "
            "Fix failures caused by this task; report pre-existing work without editing it. "
            "For a cold build or lock timeout, run the command manually and report its result.\n"
            + "\n".join(output.splitlines()[-25:])
        ),
    }


def compile_check(root: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        CHECK,
        check=False,
        cwd=root,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=45,
    )


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8")
    try:
        data = json.load(sys.stdin)
        event = data.get("hook_event_name")
        if event == "SessionStart":
            result = start(ROOT, data)
        elif event == "Stop":
            result = stop(ROOT, data)
        else:
            result = {}
        print(json.dumps(result, ensure_ascii=False))
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(
            json.dumps(
                {
                    "systemMessage": f"OpenSherwood hook could not verify state: {error}. "
                    "Run the documented checks manually."
                }
            )
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
