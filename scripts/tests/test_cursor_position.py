"""Characterize the Win32 POINT adapter without moving the real cursor."""

import ctypes
from ctypes.wintypes import BOOL, POINT
import importlib.util
from pathlib import Path
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import patch


class CursorPositionTests(unittest.TestCase):
    def call_phys(self, callback):
        source = Path(__file__).resolve().parents[2] / "harness/tools/original/oracle_input.py"
        spec = importlib.util.spec_from_file_location("cursor_adapter", source)
        module = importlib.util.module_from_spec(spec)
        user32 = SimpleNamespace(GetCursorPos=callback)
        with patch.dict(sys.modules, {"rhcap": SimpleNamespace(CAPDIR="unused")}), \
                patch.object(sys, "path", list(sys.path)), \
                patch.object(ctypes, "windll", SimpleNamespace(user32=user32), create=True):
            spec.loader.exec_module(module)
            return module.phys()

    def test_signed_coordinates_are_returned_from_point(self):
        calls = []

        def get_cursor(pointer):
            point = ctypes.cast(pointer, ctypes.POINTER(POINT)).contents
            calls.append((point.x, point.y))
            point.x = -123
            point.y = 456
            return 1

        self.assertEqual(self.call_phys(get_cursor), (-123, 456))
        self.assertEqual(calls, [(0, 0)])

    def test_failed_call_leaves_zero_coordinates(self):
        self.assertEqual(self.call_phys(lambda _pointer: 0), (0, 0))

    def test_native_pointer_writes_survive_failed_result(self):
        callback_type = getattr(ctypes, "WINFUNCTYPE", ctypes.CFUNCTYPE)(BOOL, ctypes.POINTER(POINT))

        @callback_type
        def get_cursor(pointer):
            pointer.contents.x = -2147483648
            pointer.contents.y = 2147483647
            return 0

        self.assertEqual(self.call_phys(get_cursor), (-2147483648, 2147483647))

    def test_exception_is_propagated(self):
        def get_cursor(_pointer):
            raise OSError("synthetic failure")

        with self.assertRaisesRegex(OSError, "synthetic failure"):
            self.call_phys(get_cursor)


if __name__ == "__main__":
    unittest.main()
