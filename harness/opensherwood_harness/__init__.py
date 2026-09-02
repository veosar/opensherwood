"""Python client and helpers for driving the OpenSherwood engine over JSON-RPC (docs/harness.md)."""

from .rpc import Engine, EngineError, find_binary, key_press, pointer_click, pointer_move

__all__ = ["Engine", "EngineError", "find_binary", "key_press", "pointer_click", "pointer_move"]
