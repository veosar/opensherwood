"""Characterize ASCII record names and Unicode trailing whitespace."""

import importlib.util
from pathlib import Path
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import patch


class RecordNamesTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        folder = Path(__file__).resolve().parents[2] / "harness/tools/probe"
        spec = importlib.util.spec_from_file_location("record_names", folder / "scb_elements.py")
        cls.module = importlib.util.module_from_spec(spec)
        with patch.object(sys, "path", [str(folder), *sys.path]):
            spec.loader.exec_module(cls.module)

    def records(self, lines):
        output = "SCOT v1: " + str(len(lines)) + "\n" + "\n".join("  " + line for line in lines)
        with patch.object(self.module.subprocess, "run", return_value=SimpleNamespace(stdout=output)) as run:
            result = self.module.rhm_records("unused-tool", "synthetic-input")
        run.assert_called_once_with(
            ["unused-tool", "rhm", "synthetic-input"], capture_output=True, text=True, errors="replace",
        )
        return result

    def test_ascii_suffixes_and_unicode_separators(self):
        lines = ["alpha_8abcdef0", "a_8abcdef0_8abcdef1", "éalpha_8abcdef0",
                 "ż_8abcdef2", "éa_8abcdef3\u00a0\u2003", "Z9__8abcdef4\t ",
                 "_8abcdef5", "name_8ABCDEF0", "name_8abcdef0 extra", "name_8abcdef0é"]
        names, counts = self.records(lines)
        self.assertEqual(names, {
            "alpha_8abcdef0": ("SCOT", 2), "a_8abcdef0_8abcdef1": ("SCOT", 1),
            "a_8abcdef3": ("SCOT", 4), "Z9__8abcdef4": ("SCOT", 5),
        })
        self.assertEqual(counts, {"SCOT": 10})

    def test_failed_long_name_does_not_create_a_record(self):
        names, counts = self.records(["a" * 1000 + "!", "a" * 1000 + "_8abcdef0"])
        self.assertEqual(names, {"a" * 1000 + "_8abcdef0": ("SCOT", 1)})
        self.assertEqual(counts, {"SCOT": 2})


if __name__ == "__main__":
    unittest.main()
