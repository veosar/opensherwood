"""Exercise our exporter control flow with synthetic Java API doubles only."""

from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
FIXTURES = Path(__file__).with_name("java")


@unittest.skipUnless(shutil.which("javac") and shutil.which("java"), "JDK required")
class ExporterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory(prefix="exporter-characterization-")
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.build = Path(cls.temporary.name)
        sources = list(FIXTURES.rglob("*.java"))
        for source in sorted((ROOT / "scripts" / "ghidra").glob("*.java")):
            # Replace API imports only; the exporter methods are compiled verbatim.
            lines = source.read_text(encoding="utf-8").splitlines()
            body = "\n".join(line for line in lines if not line.startswith("import ghidra."))
            target = cls.build / source.name
            target.write_text("import testfixture.MockTypes.*;\n" + body, encoding="utf-8", newline="\n")
            sources.append(target)
        compiled = subprocess.run(
            ["javac", "-d", str(cls.build), *map(str, sources)], capture_output=True, text=True,
        )
        if compiled.returncode:
            raise RuntimeError(compiled.stdout + compiled.stderr)

    def check_exporter(self, scenario):
        result = subprocess.run(
            ["java", "-ea", "-cp", str(self.build), "testfixture.ExporterChecks", scenario,
             str(self.build / scenario)], capture_output=True, text=True, timeout=30,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_decompile_all(self):
        self.check_exporter("all")

    def test_decompile_list(self):
        self.check_exporter("list")

    def test_inventory(self):
        self.check_exporter("inventory")

    def test_listing(self):
        self.check_exporter("listing")

    def test_strings(self):
        self.check_exporter("strings")

    def test_cancellation(self):
        self.check_exporter("cancel")

    def test_dispose_after_exception(self):
        self.check_exporter("exception")

    def test_refuse_output_outside_analysis_root(self):
        self.check_exporter("refuse")


if __name__ == "__main__":
    unittest.main()
