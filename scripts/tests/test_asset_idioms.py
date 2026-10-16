"""Synthetic boundary cases for the asset policy's local-name detector."""

import importlib.util
from pathlib import Path
import unittest


SOURCE = Path(__file__).parents[1] / "check_no_assets.py"
SPEC = importlib.util.spec_from_file_location("asset_policy", SOURCE)
POLICY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(POLICY)


class LocalNameDetectorTests(unittest.TestCase):
    def check_local(self, text, expected):
        data = ("undefined " + text).encode()
        problems = POLICY.check_content("example.txt", data, len(data))
        self.assertEqual(bool(problems), expected, text)
        if expected:
            self.assertEqual(
                problems, ["looks like decompiler output (auto-named parameter / local, undefined types): example.txt"]
            )

    def test_all_alternatives_and_word_boundaries(self):
        words = [
            "param_" + "1",
            "param_" + "１２",
            "local_" + "a",
            "local_" + "abcd",
            "x" + "Var2",
            "ab" + "Var12",
            "x" + "Stack",
            "ab" + "Stack012",
            "au" + "Stack",
            "au" + "Stack_",
            "au" + "Stack_name",
            "au" + "Stacké",
            "in_stack_" + "a",
            "unaff_" + "a",
            "extraout_" + "é",
            "in_" + "AB",
            "in_" + "ABC",
        ]
        for word in words:
            for prefix, suffix in (("", ""), ("(", ")"), (" ", "\n")):
                with self.subTest(word=word, prefix=prefix):
                    self.check_local(prefix + word + suffix, True)
            with self.subTest(word=word, prefix="é"):
                self.check_local("é" + word, False)

    def test_rejected_names(self):
        for word in (
            "param_",
            "local_",
            "local_" + "abcde",
            "local_" + "AB",
            "abc" + "Var1",
            "Var1",
            "abc" + "Stack1",
            "in_stack_",
            "unaff_",
            "extraout_",
            "in_A",
            "in_ABCD",
        ):
            with self.subTest(word=word):
                self.check_local(word, False)

    def test_multiple_local_alternatives_still_count_as_one_family(self):
        data = ("param_" + "1 " + "local_" + "ab " + "x" + "Var2").encode()
        self.assertEqual(POLICY.check_content("example.txt", data, len(data)), [])


if __name__ == "__main__":
    unittest.main()
