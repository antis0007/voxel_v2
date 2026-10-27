#!/usr/bin/env python3
"""Tests for scripts/tracked_deps.py against throwaway git repos.

    python -I scripts/test_tracked_deps.py
"""
import contextlib
import io
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import tracked_deps  # noqa: E402

CLEAN_ENV = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}


def git(cwd, *args):
    subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", "-C", str(cwd), *args],
                   check=True, capture_output=True, env=CLEAN_ENV)


class TrackedDepsTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name).resolve()
        git(self.root, "init", "-q", "-b", "main")
        self.write("Cargo.toml", '[workspace]\nmembers = ["crates/*"]\n')
        self.write("crates/app/Cargo.toml", '[package]\nname = "app"\n')
        self.write("crates/app/src/main.rs", "mod world;\nfn main() {}\n")
        self.write("crates/app/src/world.rs", "pub fn f() {}\n")
        self.commit()

    def tearDown(self):
        self.tmp.cleanup()

    def write(self, rel, text):
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def commit(self):
        git(self.root, "add", "-A")
        git(self.root, "commit", "-q", "-m", "c")

    def blockers(self):
        repo = tracked_deps.Repo(self.root)
        return {(src, kind, target) for src, _, kind, target, _, _ in
                tracked_deps.build_blockers(repo, repo.edges())}

    def test_clean_tree_has_no_blockers(self):
        self.assertEqual(self.blockers(), set())
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(tracked_deps.cmd_check(tracked_deps.Repo(self.root)), 0)

    def test_mod_on_untracked_file_blocks_and_keeps_cfg(self):
        self.write("crates/app/src/world.rs", '#[cfg(feature = "x")]\nmod ca;\nmod lab;\n')
        self.write("crates/app/src/world/ca.rs", "")
        self.assertEqual(self.blockers(), {
            ("crates/app/src/world.rs", "mod", "crates/app/src/world/ca.rs"),
            ("crates/app/src/world.rs", "mod", "crates/app/src/world/lab.rs"),
        })
        edges = tracked_deps.Repo(self.root).edges()
        cfgs = {e[3]: (e[4], e[5]) for e in edges}
        self.assertEqual(cfgs["crates/app/src/world/ca.rs"], ("untracked", 'feature = "x"'))
        self.assertEqual(cfgs["crates/app/src/world/lab.rs"][0], "missing")

    def test_path_attribute_and_mod_rs_resolve(self):
        self.write("crates/app/src/main.rs", 'mod world;\n#[path = "extra/t.rs"]\nmod t;\nfn main() {}\n')
        self.write("crates/app/src/world.rs", "mod deep;\n")
        self.write("crates/app/src/extra/t.rs", "")
        self.write("crates/app/src/world/deep/mod.rs", "")
        self.commit()
        self.assertEqual(self.blockers(), set())

    def test_include_and_workspace_glob_crate(self):
        self.write("crates/app/src/world.rs", 'const A: &[u8] = include_bytes!("../../../assets/a.bin");\n')
        self.write("assets/a.bin", "x")
        self.write("crates/net/Cargo.toml", '[package]\nname = "net"\n')
        self.assertEqual(self.blockers(), {
            ("crates/app/src/world.rs", "include", "assets/a.bin"),
            ("Cargo.toml", "workspace", "crates/net"),
        })

    def test_rev_mode_ignores_files_that_exist_only_on_disk(self):
        self.write("crates/app/src/world.rs", "mod ca;\n")
        git(self.root, "add", "crates/app/src/world.rs")
        git(self.root, "commit", "-q", "-m", "half")
        self.write("crates/app/src/world/ca.rs", "")
        repo = tracked_deps.Repo(self.root, "HEAD")
        blockers = tracked_deps.build_blockers(repo, repo.edges())
        self.assertEqual([(b[0], b[3], b[4]) for b in blockers],
                         [("crates/app/src/world.rs", "crates/app/src/world/ca.rs", "missing")])
        clean = tracked_deps.Repo(self.root, "HEAD~1")
        self.assertEqual(tracked_deps.build_blockers(clean, clean.edges()), [])

    def test_commented_mod_is_ignored(self):
        self.write("crates/app/src/world.rs", "// mod ghost;\n")
        self.assertEqual(self.blockers(), set())

    def test_untracked_files_linked_by_crate_paths_share_a_commit_set(self):
        self.write("crates/app/src/world.rs", "mod ca;\nmod tools;\n")
        self.write("crates/app/src/world/ca.rs", "use crate::world::tools::dig;\n")
        self.write("crates/app/src/world/tools.rs", "pub fn dig() {}\n")
        self.write("crates/app/src/lonely.rs", "")
        repo = tracked_deps.Repo(self.root)
        sets = tracked_deps.commit_sets(repo, repo.edges())
        self.assertEqual(len(sets), 1)
        files, refs = sets[0]
        self.assertEqual(files, {"crates/app/src/world/ca.rs", "crates/app/src/world/tools.rs"})
        self.assertEqual(refs, {"crates/app/src/world.rs"})


if __name__ == "__main__":
    unittest.main(verbosity=1)
