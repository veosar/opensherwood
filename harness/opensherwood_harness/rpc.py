"""JSON-RPC 2.0 client for `opensherwood --rpc stdio` (see docs/harness.md, ADR-0004).

Robustness: stderr is drained by a thread into a bounded buffer (the engine can never block on a
full pipe), every call has a deadline, and the process is always reaped.
"""

from __future__ import annotations

import collections
import json
import os
import subprocess
import sys
import threading
import time
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_TIMEOUT = float(os.environ.get("OPENSHERWOOD_RPC_TIMEOUT", "120"))


class EngineError(RuntimeError):
    """An error response from the engine (or a transport failure, code -1)."""

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


class _Reader(threading.Thread):
    """Reads lines from a pipe into a queue so the main thread can wait with a timeout."""

    def __init__(self, pipe, keep: int | None):
        super().__init__(daemon=True)
        self.pipe = pipe
        self.lines: collections.deque[str] = collections.deque(maxlen=keep)
        self.cond = threading.Condition()
        self.eof = False
        self.start()

    def run(self) -> None:
        try:
            for line in self.pipe:
                with self.cond:
                    self.lines.append(line)
                    self.cond.notify_all()
        finally:
            with self.cond:
                self.eof = True
                self.cond.notify_all()

    def pop(self, timeout: float) -> str | None:
        deadline = time.monotonic() + timeout
        with self.cond:
            while not self.lines:
                if self.eof:
                    return None
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise TimeoutError
                self.cond.wait(remaining)
            return self.lines.popleft()

    def text(self) -> str:
        with self.cond:
            return "".join(self.lines)


class Engine:
    """A running engine process speaking JSON-RPC over stdio."""

    def __init__(
        self,
        binary: Path | None = None,
        game_dir: Path | None = None,
        artifacts: Path | None = None,
        extra_args: list[str] | None = None,
        headless: bool = True,
        timeout: float = DEFAULT_TIMEOUT,
    ):
        self.binary = binary or find_binary()
        self.timeout = timeout
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
        self._out = _Reader(self.proc.stdout, keep=None)
        self._err = _Reader(self.proc.stderr, keep=2000)
        self._next_id = 0

    @property
    def stderr_text(self) -> str:
        """Recent engine log output."""
        return self._err.text()

    def call(self, method: str, params: dict[str, Any] | None = None, timeout: float | None = None) -> Any:
        """Send one request and wait for its response (raises EngineError on any failure)."""
        self._next_id += 1
        req: dict[str, Any] = {"jsonrpc": "2.0", "id": self._next_id, "method": method}
        if params is not None:
            req["params"] = params
        assert self.proc.stdin
        try:
            self.proc.stdin.write(json.dumps(req) + "\n")
            self.proc.stdin.flush()
        except (BrokenPipeError, OSError) as e:
            raise EngineError(-1, f"engine pipe closed ({e}); stderr: {self.stderr_text[-2000:]}") from e
        try:
            line = self._out.pop(timeout or self.timeout)
        except TimeoutError:
            self.kill()
            raise EngineError(-1, f"no response to {method} within {timeout or self.timeout}s; stderr: {self.stderr_text[-2000:]}")
        if line is None:
            raise EngineError(-1, f"engine closed the connection (exit={self.proc.poll()}); stderr: {self.stderr_text[-2000:]}")
        resp = json.loads(line)
        if resp.get("id") != self._next_id:
            raise EngineError(-1, f"response id mismatch: {resp}")
        if resp.get("error") is not None:
            e = resp["error"]
            raise EngineError(e.get("code", -1), e.get("message", "?"), e.get("data"))
        return resp.get("result")

    def notify(self, method: str, params: dict[str, Any] | None = None) -> None:
        """Send a notification (no id, no response)."""
        req: dict[str, Any] = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            req["params"] = params
        assert self.proc.stdin
        self.proc.stdin.write(json.dumps(req) + "\n")
        self.proc.stdin.flush()

    # Convenience wrappers -------------------------------------------------

    def hello(self) -> dict[str, Any]:
        return self.call("hello", {"client": "opensherwood_harness"})

    def reset(self, scenario: dict[str, Any] | str = "corridor", seed: int = 0) -> dict[str, Any]:
        if isinstance(scenario, str):
            scenario = {"synthetic": scenario}
        return self.call("reset", {"scenario": scenario, "seed": seed})

    def step(self, ticks: int = 1, events: list[dict[str, Any]] | None = None, hash_every_tick: bool = False):
        return self.call("step", {"ticks": ticks, "events": events or [], "hash_every_tick": hash_every_tick})

    def observe(self, entities: bool = True) -> dict[str, Any]:
        return self.call("observe", {"entities": entities})

    def skip_briefing(self, max_pages: int = 30) -> int:
        """Dismiss the script's text pages shown after a mission load (Enter, like a player); returns
        the number of pages dismissed."""
        pages = 0
        while pages < max_pages:
            ui = self.observe(entities=False).get("ui")
            if not ui or ui.get("screen") != "briefing":
                break
            self.step(1, [
                {"tick_offset": 0, "sequence": 0, "kind": "key_down", "key": "enter"},
                {"tick_offset": 0, "sequence": 1, "kind": "key_up", "key": "enter"},
            ])
            pages += 1
        return pages

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
            self.call("shutdown", timeout=10)
        finally:
            self.close()

    def kill(self) -> None:
        if self.proc.poll() is None:
            self.proc.kill()
        self.proc.wait(timeout=10)

    def close(self) -> None:
        if self.proc.poll() is None:
            try:
                assert self.proc.stdin
                self.proc.stdin.close()
            except OSError:
                pass
            try:
                self.proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.kill()
        else:
            self.proc.wait(timeout=10)

    def __enter__(self) -> Engine:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()
