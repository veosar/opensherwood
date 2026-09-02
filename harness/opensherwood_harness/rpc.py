"""JSON-RPC 2.0 client for `opensherwood --rpc stdio` (see docs/harness.md, ADR-0004)."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[2]


class EngineError(RuntimeError):
    """An error response from the engine."""

    def __init__(self, code: int, message: str, data: Any = None):
        super().__init__(f"engine error {code}: {message}")
        self.code = code
        self.message = message
        self.data = data


def find_binary() -> Path:
    """Locate the engine binary: $OPENSHERWOOD_BIN, then target/{release,debug}."""
    env = os.environ.get("OPENSHERWOOD_BIN")
    if env:
        p = Path(env)
        if not p.is_absolute():
            p = REPO_ROOT / p
        if p.exists():
            return p
        raise FileNotFoundError(f"OPENSHERWOOD_BIN={env} does not exist")
    exe = "opensherwood.exe" if sys.platform == "win32" else "opensherwood"
    for profile in ("release", "debug"):
        p = REPO_ROOT / "target" / profile / exe
        if p.exists():
            return p
    raise FileNotFoundError("engine binary not built; run `cargo build -p opensherwood-app` or set OPENSHERWOOD_BIN")


def pointer_move(x: float, y: float, tick_offset: int = 0, sequence: int = 0) -> dict[str, Any]:
    """Build a pointer_move event in logical pixels."""
    return {
        "tick_offset": tick_offset,
        "sequence": sequence,
        "kind": "pointer_move",
        "x256": int(round(x * 256)),
        "y256": int(round(y * 256)),
    }


def pointer_click(x: float, y: float, button: str = "left", tick_offset: int = 0) -> list[dict[str, Any]]:
    """Move, press and release in one tick (three events)."""
    return [
        pointer_move(x, y, tick_offset, 0),
        {"tick_offset": tick_offset, "sequence": 1, "kind": "pointer_down", "button": button},
        {"tick_offset": tick_offset, "sequence": 2, "kind": "pointer_up", "button": button},
    ]


class Engine:
    """A running engine process speaking JSON-RPC over stdio."""

    def __init__(
        self,
        binary: Path | None = None,
        game_dir: Path | None = None,
        artifacts: Path | None = None,
        extra_args: list[str] | None = None,
        headless: bool = True,
    ):
        self.binary = binary or find_binary()
        args = [str(self.binary), "--rpc", "stdio"]
        if headless:
            args.append("--headless")
        if game_dir:
            args += ["--game-dir", str(game_dir)]
        if artifacts:
            args += ["--artifacts", str(artifacts)]
        if extra_args:
            args += extra_args
        self.proc = subprocess.Popen(
            args,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            bufsize=1,
        )
        self._next_id = 0

    def call(self, method: str, params: dict[str, Any] | None = None) -> Any:
        """Send one request and wait for its response."""
        self._next_id += 1
        req: dict[str, Any] = {"jsonrpc": "2.0", "id": self._next_id, "method": method}
        if params is not None:
            req["params"] = params
        assert self.proc.stdin and self.proc.stdout
        self.proc.stdin.write(json.dumps(req) + "\n")
        self.proc.stdin.flush()
        line = self.proc.stdout.readline()
        if not line:
            err = self.proc.stderr.read() if self.proc.stderr else ""
            raise EngineError(-1, f"engine closed the connection (exit={self.proc.poll()}): {err[-2000:]}")
        resp = json.loads(line)
        if resp.get("id") != self._next_id:
            raise EngineError(-1, f"response id mismatch: {resp}")
        if "error" in resp and resp["error"] is not None:
            e = resp["error"]
            raise EngineError(e.get("code", -1), e.get("message", "?"), e.get("data"))
        return resp.get("result")

    # Convenience wrappers -------------------------------------------------

    def hello(self) -> dict[str, Any]:
        return self.call("hello", {"client": "opensherwood_harness"})

    def reset(self, scenario: dict[str, Any] | str = "corridor", seed: int = 0) -> dict[str, Any]:
        if isinstance(scenario, str):
            scenario = {"synthetic": scenario}
        return self.call("reset", {"scenario": scenario, "seed": seed})

    def step(self, ticks: int = 1, events: list[dict[str, Any]] | None = None, hash_every_tick: bool = False):
        return self.call("step", {"ticks": ticks, "events": events or [], "hash_every_tick": hash_every_tick})

    def observe(self) -> dict[str, Any]:
        return self.call("observe")

    def snapshot(self) -> dict[str, Any]:
        return self.call("snapshot")

    def restore(self, snapshot_id: str | None = None, snapshot: dict[str, Any] | None = None):
        params: dict[str, Any] = {}
        if snapshot_id:
            params["id"] = snapshot_id
        if snapshot:
            params["snapshot"] = snapshot
        return self.call("restore", params)

    def capture(self, path: str | None = None) -> dict[str, Any]:
        return self.call("capture", {"path": path} if path else {})

    def shutdown(self) -> None:
        try:
            self.call("shutdown")
        finally:
            self.close()

    def close(self) -> None:
        if self.proc.poll() is None:
            try:
                assert self.proc.stdin
                self.proc.stdin.close()
            except OSError:
                pass
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()

    def __enter__(self) -> Engine:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()
