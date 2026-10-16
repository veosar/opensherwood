"""Synthetic checks for the Claude Code project hooks."""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import agent_hooks
import sync_skills


class WorkflowTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="agent workflow ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        subprocess.run(["git", "init", "-q", "-b", "main", str(self.root)], check=True)
        (self.root / ".gitignore").write_text(".local/\n.claude/handoffs/\ntarget/\n", encoding="utf-8")
        for folder in (".agents/skills/example", ".claude/skills/example"):
            directory = self.root / folder
            directory.mkdir(parents=True)
            (directory / "SKILL.md").write_text("synthetic procedure\n", encoding="utf-8")
        (self.root / ".claude/skills" / sync_skills.MARKER).write_text(sync_skills.MARKER_TEXT)
        (self.root / "Cargo.toml").write_text(
            '[workspace]\nmembers = ["crates/toy"]\nresolver = "3"\n', encoding="utf-8"
        )
        self.source = self.root / "crates/toy/src/lib.rs"
        self.source.parent.mkdir(parents=True)
        self.source.write_text("pub fn value() -> u32 { 1 }\n", encoding="utf-8")
        (self.source.parents[1] / "Cargo.toml").write_text(
            '[package]\nname = "toy"\nversion = "0.1.0"\nedition = "2024"\n',
            encoding="utf-8",
        )
        self.data = {"session_id": "synthetic-session", "cwd": str(self.root)}

    def test_start_resume_and_compaction_preserve_baseline_silently(self):
        self.assertEqual(agent_hooks.start(self.root, self.data), {})
        baseline = agent_hooks.read_state(agent_hooks.state_path(self.root, self.data))["baseline"]
        self.source.write_text("invalid rust")
        for source in ("resume", "compact"):
            agent_hooks.start(self.root, {**self.data, "source": source})
            state = agent_hooks.read_state(agent_hooks.state_path(self.root, self.data))
            self.assertEqual(state["baseline"], baseline)
        self.assertFalse((self.root / ".claude/handoffs").exists())

    def test_start_reports_skill_drift(self):
        (self.root / ".claude/skills/example/SKILL.md").write_text("stale copy")
        context = agent_hooks.start(self.root, self.data)["hookSpecificOutput"]
        self.assertIn("Skills differ", context["additionalContext"])

    def test_fingerprint_ignores_docs_and_detects_untracked_edits_and_deletions(self):
        before = agent_hooks.fingerprint(self.root)
        (self.root / "README.md").write_text("New documentation")
        self.assertEqual(agent_hooks.fingerprint(self.root), before)
        self.source.write_text("pub fn value() -> u32 { 2 }\n")
        self.assertNotEqual(agent_hooks.fingerprint(self.root), before)
        before = agent_hooks.fingerprint(self.root)
        self.source.unlink()
        self.assertNotEqual(agent_hooks.fingerprint(self.root), before)

    def test_stop_detects_skill_drift_and_does_not_loop(self):
        (self.root / ".claude/skills/example/SKILL.md").write_text("stale copy")
        self.assertEqual(agent_hooks.stop(self.root, self.data)["decision"], "block")
        self.assertEqual(agent_hooks.stop(self.root, {**self.data, "stop_hook_active": True}), {})

    def test_timeout_and_edit_during_check_are_not_successes(self):
        with patch.object(
            agent_hooks,
            "compile_check",
            side_effect=subprocess.TimeoutExpired("cargo", 45),
        ):
            result = agent_hooks.stop(self.root, self.data)
        self.assertEqual(result["decision"], "block")
        self.assertFalse(agent_hooks.read_state(agent_hooks.state_path(self.root, self.data))["ok"])
        self.source.write_text("pub fn value() -> u32 { 3 }\n")

        def concurrent_edit(*args, **kwargs):
            self.source.write_text("pub fn value() -> u32 { 4 }\n")
            return subprocess.CompletedProcess(args, 0, "", "")

        with patch.object(agent_hooks, "compile_check", side_effect=concurrent_edit):
            result = agent_hooks.stop(self.root, self.data)
        self.assertIn("changed during", result["reason"])

    @unittest.skipUnless(shutil.which("cargo"), "Rust fixture requires cargo; runs in build CI")
    def test_real_failing_compile_then_fixed_compile_and_cached_stop(self):
        subprocess.run(
            ["cargo", "generate-lockfile", "--offline"],
            cwd=self.root,
            check=True,
            capture_output=True,
            timeout=30,
        )
        agent_hooks.start(self.root, self.data)
        self.assertEqual(agent_hooks.stop(self.root, self.data), {})
        self.source.write_text("pub fn broken(\n")
        first = agent_hooks.stop(self.root, self.data)
        self.assertEqual(first["decision"], "block")
        self.assertIn("error", first["reason"])
        second = agent_hooks.stop(self.root, self.data)
        self.assertNotIn("decision", second)
        self.assertIn("failed", second["systemMessage"])
        self.source.write_text("pub fn value() -> u32 { 5 }\n")
        self.assertEqual(agent_hooks.stop(self.root, self.data), {})
        with patch.object(
            agent_hooks,
            "compile_check",
            side_effect=AssertionError("unexpected rebuild"),
        ):
            self.assertEqual(agent_hooks.stop(self.root, self.data), {})

    def test_configured_launcher_accepts_json_from_subdirectory(self):
        (self.root / "scripts").mkdir()
        for name in ("agent_hooks.py", "sync_skills.py"):
            shutil.copyfile(SCRIPTS / name, self.root / "scripts" / name)
        settings = json.loads((SCRIPTS.parent / ".claude/settings.json").read_text())
        for event in ("SessionStart", "Stop"):
            command = settings["hooks"][event][0]["hooks"][0]["command"]
            data = {**self.data, "hook_event_name": event}
            result = subprocess.run(
                command,
                check=False,
                shell=True,
                cwd=self.source.parent,
                input=json.dumps(data),
                capture_output=True,
                text=True,
                encoding="utf-8",
                timeout=30,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout), {})


if __name__ == "__main__":
    unittest.main()
