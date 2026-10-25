#!/usr/bin/env python3
"""Tests for scripts/worktrees.py against throwaway git repos.

    python -I scripts/test_worktrees.py
"""
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import worktrees  # noqa: E402

CLEAN_ENV = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}


def run(cwd, *args):
    subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", "-C", str(cwd), *args],
                   check=True, capture_output=True, env=CLEAN_ENV)


class SurveyTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        base = Path(self.tmp.name).resolve()
        self.main = base / "main"
        self.main.mkdir()
        run(self.main, "init", "-q", "-b", "main")
        (self.main / ".gitignore").write_text("target/\nsaves/\n", encoding="utf-8")
        (self.main / "a.txt").write_text("a\n", encoding="utf-8")
        run(self.main, "add", ".")
        run(self.main, "commit", "-q", "-m", "base")
        self.base = base

    def tearDown(self):
        self.tmp.cleanup()

    def add(self, name, *flags):
        path = self.base / name
        run(self.main, "worktree", "add", "-q", *flags, str(path))
        return path

    def verdicts(self, hours=0):
        return {Path(f["path"]).name: (v, r) for f, v, r in worktrees.survey(hours, self.main)}

    def test_primary_is_never_removed(self):
        self.assertEqual(self.verdicts()["main"][0], "KEEP")

    def test_clean_saved_idle_worktree_is_removable(self):
        self.add("wt", "-b", "feature")
        self.assertEqual(self.verdicts()["wt"][0], "REMOVE")

    def test_recent_activity_keeps_it(self):
        self.add("wt", "-b", "feature")
        verdict, reason = self.verdicts(hours=24)["wt"]
        self.assertEqual(verdict, "KEEP")
        self.assertIn("active", reason)

    def test_untracked_file_keeps_it(self):
        path = self.add("wt", "-b", "feature")
        (path / "new.txt").write_text("x\n", encoding="utf-8")
        self.assertIn("changed", self.verdicts()["wt"][1])

    def test_commit_on_no_ref_keeps_it(self):
        path = self.add("wt", "--detach")
        (path / "a.txt").write_text("b\n", encoding="utf-8")
        run(path, "commit", "-q", "-am", "orphan")
        self.assertEqual(self.verdicts()["wt"], ("KEEP", "HEAD is on no branch or tag"))

    def test_review_evidence_in_target_keeps_it(self):
        path = self.add("wt", "-b", "feature")
        (path / "target" / "debug").mkdir(parents=True)
        (path / "target" / "debug" / "x.o").write_text("x", encoding="utf-8")
        (path / "target" / "review-captures").mkdir()
        (path / "target" / "review-captures" / "a.png").write_text("x", encoding="utf-8")
        verdict, reason = self.verdicts()["wt"]
        self.assertEqual(verdict, "KEEP")
        self.assertIn("target/review-captures", reason)

    def test_build_output_alone_does_not_keep_it(self):
        path = self.add("wt", "-b", "feature")
        (path / "target" / "debug").mkdir(parents=True)
        (path / "target" / "debug" / "x.o").write_text("x", encoding="utf-8")
        self.assertEqual(self.verdicts()["wt"][0], "REMOVE")

    def test_saves_keep_it(self):
        path = self.add("wt", "-b", "feature")
        (path / "saves").mkdir()
        (path / "saves" / "w.vxw").write_text("x", encoding="utf-8")
        self.assertIn("saves", self.verdicts()["wt"][1])

    def test_locked_keeps_it(self):
        path = self.add("wt", "-b", "feature")
        run(self.main, "worktree", "lock", str(path))
        self.assertEqual(self.verdicts()["wt"], ("KEEP", "locked"))

    def test_deleted_directory_is_pruned(self):
        path = self.add("wt", "-b", "feature")
        for child in sorted(path.rglob("*"), reverse=True):
            child.unlink() if child.is_file() else child.rmdir()
        path.rmdir()
        self.assertEqual(self.verdicts()["wt"][0], "PRUNE")

    def test_scratchpad_owner_activity_keeps_it(self):
        facts = {"primary": False, "exists": True, "locked": False, "dirty": 0, "saved": True,
                 "evidence": [], "idle_h": 99.0, "owner_idle_h": 1.0}
        verdict, reason = worktrees.classify(facts, 24)
        self.assertEqual(verdict, "KEEP")
        self.assertIn("owner session", reason)


if __name__ == "__main__":
    unittest.main()
