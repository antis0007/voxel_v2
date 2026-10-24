#!/usr/bin/env python3
"""Tests for the task gate (scripts/workflow.py, scripts/workflow_hooks.py).

Each test copies both scripts and the policy into a throwaway git repo, so the real checkout and its
session state are never touched. Sessions are driven the way Claude Code drives them: hook events as JSON
on stdin, commands with CLAUDE_CODE_SESSION_ID set.

    python -I scripts/test_workflow.py
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

# A checkout to take the scripts and policy from; override to run these tests against a mutated copy.
SOURCE = Path(os.environ.get("WORKFLOW_TEST_SOURCE") or Path(__file__).resolve().parents[1])
# A caller's GIT_INDEX_FILE / GIT_DIR (the temp-index commit flow) must not leak into the throwaway repos.
CLEAN_ENV = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}


class Repo:
    def __init__(self):
        self.root = Path(tempfile.mkdtemp(prefix="workflow-test-"))
        (self.root / "scripts").mkdir()
        (self.root / ".claude" / "verify").mkdir(parents=True)
        for name in ("workflow.py", "workflow_hooks.py"):
            shutil.copy(SOURCE / "scripts" / name, self.root / "scripts" / name)
        shutil.copy(SOURCE / ".claude" / "verify" / "policy.json", self.root / ".claude" / "verify" / "policy.json")
        (self.root / ".gitignore").write_text("target/\n", encoding="utf-8")
        (self.root / "docs").mkdir()
        for name in ("a.md", "b.md", "c.md"):
            (self.root / "docs" / name).write_text(name + "\n", encoding="utf-8")
        (self.root / "crates" / "core" / "src").mkdir(parents=True)
        (self.root / "crates" / "core" / "src" / "tracked.rs").write_text("fn base() {}\n", encoding="utf-8")
        self.git("init", "-q")
        self.git("add", "-A")
        self.git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "base")

    def git(self, *args):
        subprocess.run(["git", *args], cwd=self.root, check=True, capture_output=True, env=CLEAN_ENV)

    def write(self, rel, text):
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def hook(self, event_name, **event):
        out = subprocess.run([sys.executable, "-I", "scripts/workflow.py", "hook", event_name],
                             input=json.dumps(event), capture_output=True, text=True, cwd=self.root, env=CLEAN_ENV)
        assert out.returncode == 0, out.stderr
        result = json.loads(out.stdout) if out.stdout.strip() else None
        message = (result or {}).get("systemMessage", "")
        assert not message.startswith("workflow hook"), message  # run() swallowed an exception
        return result

    def cmd(self, session, *args):
        env = dict(CLEAN_ENV, CLAUDE_CODE_SESSION_ID=session)
        out = subprocess.run([sys.executable, "-I", "scripts/workflow.py", *args], capture_output=True, text=True,
                             cwd=self.root, env=env)
        return out.returncode, out.stdout

    def prompt(self, session):
        self.hook("prompt", session_id=session, prompt="docs")

    def shell(self, session, tid, action):
        """A Bash call: pre snapshot, the call's writes, post attribution."""
        self.hook("pre", session_id=session, tool_name="Bash", tool_use_id=tid, tool_input={})
        action()
        self.hook("post", session_id=session, tool_name="Bash", tool_use_id=tid, tool_input={})

    def edit(self, session, tid, rel, text):
        tool_input = {"file_path": str(self.root / rel)}
        self.hook("pre", session_id=session, tool_name="Edit", tool_use_id=tid, tool_input=tool_input)
        self.write(rel, text)
        self.hook("post", session_id=session, tool_name="Edit", tool_use_id=tid, tool_input=tool_input)

    def task(self, session):
        """The session's task as the gate computes it, from the copied scripts (their ROOT is this repo)."""
        code = ("import json, sys; sys.path.insert(0, 'scripts'); import workflow as wf; "
                f"print(json.dumps(wf.changed_files(wf.load_state({session!r}))))")
        out = subprocess.run([sys.executable, "-I", "-c", code], capture_output=True, text=True, cwd=self.root,
                             env=CLEAN_ENV)
        assert out.returncode == 0, out.stderr
        return set(json.loads(out.stdout))

    def state(self, session):
        return json.loads((self.root / "target" / "workflow" / "sessions" / f"{session}.json").read_text())

    def make_legacy(self, session):
        path = self.root / "target" / "workflow" / "sessions" / f"{session}.json"
        state = json.loads(path.read_text())
        state.pop("owned", None)
        path.write_text(json.dumps(state))

    def close(self):
        shutil.rmtree(self.root, ignore_errors=True)


class Ownership(unittest.TestCase):
    def setUp(self):
        self.repo = Repo()

    def tearDown(self):
        self.repo.close()

    def test_a_peer_edit_is_not_in_my_task(self):
        r = self.repo
        r.prompt("me")
        r.prompt("peer")
        r.edit("me", "t1", "docs/a.md", "mine\n")
        r.shell("peer", "t2", lambda: r.write("docs/b.md", "theirs\n"))
        self.assertEqual(r.task("me"), {"docs/a.md"})
        self.assertEqual(r.task("peer"), {"docs/b.md"})

    def test_a_file_i_own_stays_mine_after_a_peer_edit(self):
        r = self.repo
        r.prompt("me")
        r.prompt("peer")
        r.edit("me", "t1", "docs/a.md", "mine\n")
        r.edit("peer", "t2", "docs/a.md", "theirs on top\n")
        self.assertIn("docs/a.md", r.task("me"))

    def test_a_shell_write_and_a_deletion_are_attributed(self):
        r = self.repo
        r.prompt("me")
        r.shell("me", "t1", lambda: (r.write("docs/new.md", "x\n"), (r.root / "docs" / "c.md").unlink()))
        self.assertEqual(r.task("me"), {"docs/new.md", "docs/c.md"})

    def test_a_legacy_session_still_counts_every_change(self):
        r = self.repo
        r.prompt("old")
        r.make_legacy("old")
        r.shell("peer", "t1", lambda: r.write("docs/b.md", "theirs\n"))
        r.shell("old", "t2", lambda: r.write("docs/a.md", "legacy write\n"))
        self.assertEqual(r.task("old"), {"docs/a.md", "docs/b.md"})
        pending = list((r.root / "target" / "workflow" / "pending").glob("old-*.json"))
        self.assertEqual(pending, [], "legacy post must remove its pre snapshot")

    def test_adopting_inside_a_call_does_not_claim_peer_edits(self):
        r = self.repo
        r.prompt("me")
        r.make_legacy("me")
        r.edit("me", "t0", "docs/a.md", "mine\n")  # written while legacy: not attributed by hooks
        r.shell("peer", "t1", lambda: r.write("docs/b.md", "theirs\n"))
        # One shell call claims and adopts, as an agent would.
        r.hook("pre", session_id="me", tool_name="Bash", tool_use_id="t2", tool_input={})
        self.assertEqual(r.cmd("me", "claim", "docs/a.md", "--note", "pre-hook edit")[0], 0)
        code, out = r.cmd("me", "adopt-ownership")
        self.assertEqual(code, 0, out)
        self.assertIn("dropped docs/b.md", out)
        r.hook("post", session_id="me", tool_name="Bash", tool_use_id="t2", tool_input={})
        self.assertEqual(r.task("me"), {"docs/a.md"})

    def test_claim_only_adds(self):
        r = self.repo
        r.prompt("me")
        r.edit("me", "t1", "docs/a.md", "mine\n")
        r.write("docs/b.md", "written by a detached process\n")
        self.assertEqual(r.cmd("me", "claim", "docs/b.md", "--note", "background job")[0], 0)
        self.assertEqual(r.task("me"), {"docs/a.md", "docs/b.md"})

    def test_an_orphaned_call_is_reported_not_attributed(self):
        r = self.repo
        r.prompt("me")
        r.hook("pre", session_id="me", tool_name="Bash", tool_use_id="lost", tool_input={})
        r.write("docs/a.md", "written by a call whose post hook timed out\n")
        pfile = r.root / "target" / "workflow" / "pending" / "me-lost.json"
        old = time.time() - 3600
        os.utime(pfile, (old, old))  # the call started an hour ago...
        os.utime(r.root / "docs" / "a.md", (old + 30, old + 30))  # ...and touched this 30 s in
        out = json.dumps(r.hook("stop", session_id="me"))
        self.assertEqual(r.task("me"), set(), "orphans are never swept into the task")
        self.assertIn("docs/a.md", out)
        self.assertIn("claim", out)
        self.assertFalse(pfile.exists(), "each orphan is reported once")

    def test_disown_drops_a_peer_write_until_this_session_writes_it_again(self):
        r = self.repo
        r.prompt("me")
        r.shell("me", "t1", lambda: (r.write("docs/a.md", "mine\n"), r.write("docs/b.md", "a peer's, same moment\n")))
        self.assertEqual(r.task("me"), {"docs/a.md", "docs/b.md"})
        code, out = r.cmd("me", "disown", "docs/b.md", "--note", "peer session wrote it")
        self.assertEqual(code, 0, out)
        self.assertEqual(r.task("me"), {"docs/a.md"})
        self.assertIn("disowned", json.dumps(r.hook("stop", session_id="me")))
        self.assertNotIn("disowned", json.dumps(r.hook("stop", session_id="me")), "announced once")
        r.edit("me", "t2", "docs/b.md", "now mine\n")
        self.assertEqual(r.task("me"), {"docs/a.md", "docs/b.md"})

    def test_disown_refuses_files_this_session_named_or_claimed(self):
        r = self.repo
        r.prompt("me")
        r.edit("me", "t1", "crates/app/src/render/x.rs", "fn mine() {}\n")
        r.write("docs/b.md", "changed\n")
        self.assertEqual(r.cmd("me", "claim", "docs/b.md", "--note", "mine")[0], 0)
        for path in ("crates/app/src/render/x.rs", "docs/b.md"):
            code, out = r.cmd("me", "disown", path, "--note", "dodge")
            self.assertEqual(code, 2, out)
        self.assertEqual(r.task("me"), {"crates/app/src/render/x.rs", "docs/b.md"})

    def test_disowned_paths_are_listed_and_a_later_change_is_flagged(self):
        r = self.repo
        r.prompt("me")
        r.shell("me", "t1", lambda: r.write("docs/b.md", "a peer's\n"))
        self.assertEqual(r.cmd("me", "disown", "docs/b.md", "--note", "peer")[0], 0)
        self.assertIn("disowned       docs/b.md: peer", r.cmd("me", "status")[1])
        r.write("docs/b.md", "changed again by someone\n")
        self.assertIn("CHANGED since disown", r.cmd("me", "status")[1])
        self.assertIn("changed since", json.dumps(r.hook("stop", session_id="me")))

    def test_a_young_pending_call_is_reported_but_kept(self):
        r = self.repo
        r.prompt("me")
        r.hook("pre", session_id="me", tool_name="Bash", tool_use_id="young", tool_input={})
        r.write("docs/a.md", "written by a call that failed a moment ago\n")
        out = json.dumps(r.hook("stop", session_id="me"))
        self.assertIn("docs/a.md", out)
        self.assertTrue((r.root / "target" / "workflow" / "pending" / "me-young.json").exists())

    def test_disown_refuses_a_path_the_session_does_not_own(self):
        r = self.repo
        r.prompt("me")
        code, out = r.cmd("me", "disown", "docs/c.md", "--note", "x")
        self.assertEqual(code, 2, out)

    def test_nothing_the_game_embeds_matches_the_doc_patterns(self):
        """Manual evidence survives edits to doc-pattern files; that is only safe if the game never reads them."""
        crates = SOURCE / "crates"
        if not crates.is_dir():
            self.skipTest("no crates in this source tree")
        import fnmatch
        import re
        pol = json.loads((SOURCE / ".claude" / "verify" / "policy.json").read_text(encoding="utf-8"))
        docs = pol.get("inert_paths") or next(r["only_paths"] for r in pol["routes"] if r["id"] == "docs_only")
        embedded = re.compile(r'include_(?:str|bytes)!\(\s*"([^"]+)"')
        hits = []
        for source in crates.rglob("*.rs"):
            for target in embedded.findall(source.read_text(encoding="utf-8", errors="replace")):
                rel = (source.parent / target).resolve()
                try:
                    rel = rel.relative_to(SOURCE.resolve()).as_posix()
                except ValueError:
                    continue
                if any(fnmatch.fnmatch(rel, pat) for pat in docs) and not source.name == "repo_layout.rs":
                    hits.append(f"{source.relative_to(SOURCE).as_posix()} -> {rel}")
        self.assertEqual(hits, [], "an inert-pattern file is embedded in the game; tighten inert_paths in the policy")
        # Runtime reads (fs::read, include!, concat!) name their path in a string literal somewhere.
        literal = re.compile(r'"(?:\.\./)*(?:docs|\.claude)/')
        readers = [source.relative_to(SOURCE).as_posix() for source in crates.rglob("*.rs")
                   if source.name != "repo_layout.rs"  # the agent-docs test reads docs on purpose
                   and literal.search(source.read_text(encoding="utf-8", errors="replace"))]
        self.assertEqual(readers, [], "game code names a docs/ or .claude/ path; is that file still inert?")

    def test_a_gate_script_edit_does_not_stale_a_capture(self):
        r = self.repo
        r.prompt("me")
        r.edit("me", "t1", "crates/app/src/render/x.rs", "fn a() {}\n")
        for name in ("before.png", "after.png"):
            r.write(f"target/ev/{name}", "png")
        self.assertEqual(r.cmd("me", "attest", "capture_ab", "--note", "i", "--evidence", "target/ev")[0], 0)
        r.edit("me", "t2", ".claude/rules/x.md", "a rule\n")
        self.assertEqual(self.check_states("me")["capture_ab"], "ok")

    def test_a_malformed_pending_file_does_not_disable_the_stop_gate(self):
        r = self.repo
        r.prompt("me")
        r.edit("me", "t1", "crates/app/src/render/x.rs", "fn a() {}\n")
        pfile = r.root / "target" / "workflow" / "pending" / "me-junk.json"
        pfile.parent.mkdir(parents=True, exist_ok=True)
        pfile.write_text("[1, 2]", encoding="utf-8")
        old = time.time() - 3600
        os.utime(pfile, (old, old))
        out = r.hook("stop", session_id="me")
        self.assertNotEqual((out or {}).get("decision"), "block", "stop is advisory")
        self.assertIn("Required checks", (out or {}).get("systemMessage", ""), "missing checks are still reported")
        self.assertFalse(pfile.exists())

    def test_a_failed_tool_call_is_attributed(self):
        """PostToolUseFailure runs the same post handler."""
        r = self.repo
        r.prompt("me")
        r.hook("pre", session_id="me", tool_name="Bash", tool_use_id="f1", tool_input={})
        r.write("docs/a.md", "written before the command failed\n")
        r.hook("post", session_id="me", hook_event_name="PostToolUseFailure", tool_name="Bash",
               tool_use_id="f1", tool_input={}, error="exit 1")
        self.assertEqual(r.task("me"), {"docs/a.md"})

    def test_a_recent_pending_call_is_left_alone_at_stop(self):
        r = self.repo
        r.prompt("me")
        r.hook("pre", session_id="me", tool_name="Bash", tool_use_id="live", tool_input={})
        r.hook("stop", session_id="me")
        self.assertTrue((r.root / "target" / "workflow" / "pending" / "me-live.json").exists())

    def test_adoption_is_announced_once_at_stop(self):
        r = self.repo
        r.prompt("me")
        r.make_legacy("me")
        r.write("docs/b.md", "peer\n")
        r.write("docs/a.md", "mine\n")
        r.cmd("me", "claim", "docs/a.md", "--note", "mine")
        r.cmd("me", "adopt-ownership")
        first = json.dumps(r.hook("stop", session_id="me"))
        second = json.dumps(r.hook("stop", session_id="me"))
        self.assertIn("docs/b.md", first)
        self.assertNotIn("Ownership adopted", second)

    def check_states(self, session):
        code, out = self.repo.cmd(session, "status")
        self.assertEqual(code, 0, out)
        return {ln.split()[1]: ln.split()[0] for ln in out.splitlines()[1:] if ln.strip()}

    def test_capture_evidence_survives_an_unrelated_edit_but_not_a_render_edit(self):
        r = self.repo
        r.prompt("me")
        r.edit("me", "t1", "crates/app/src/render/x.rs", "fn a() {}\n")
        r.edit("me", "t2", "docs/a.md", "notes\n")
        for name in ("before.png", "after.png"):
            r.write(f"target/ev/{name}", "png")
        code, out = r.cmd("me", "attest", "capture_ab", "--note", "inspected", "--evidence", "target/ev")
        self.assertEqual(code, 0, out)
        self.assertEqual(self.check_states("me")["capture_ab"], "ok")
        r.edit("me", "t3", "docs/a.md", "more notes\n")
        self.assertEqual(self.check_states("me")["capture_ab"], "ok", "a docs edit must not stale a render capture")
        r.edit("me", "t4", "crates/app/src/render/x.rs", "fn b() {}\n")
        self.assertEqual(self.check_states("me")["capture_ab"], "MISSING/STALE")

    def test_any_code_edit_stales_a_capture_even_outside_its_routes(self):
        r = self.repo
        r.prompt("me")
        r.edit("me", "t1", "crates/app/src/render/x.rs", "fn a() {}\n")
        r.edit("me", "t2", "crates/app/src/lighting.rs", "fn l() {}\n")  # no capture route matches this file
        for name in ("before.png", "after.png"):
            r.write(f"target/ev/{name}", "png")
        self.assertEqual(r.cmd("me", "attest", "capture_ab", "--note", "i", "--evidence", "target/ev")[0], 0)
        r.edit("me", "t3", "crates/app/src/lighting.rs", "fn l2() {}\n")
        self.assertEqual(self.check_states("me")["capture_ab"], "MISSING/STALE")

    def test_a_review_survives_a_docs_edit_only(self):
        r = self.repo
        r.prompt("me")
        r.edit("me", "t1", "crates/core/src/a.rs", "use std::sync::Mutex<u8>;\n")  # concurrency_unsafe -> review_contract
        r.edit("me", "t2", "docs/a.md", "notes\n")
        r.write("target/review.md", "x" * 300)
        code, out = r.cmd("me", "attest", "review_contract", "--note", "reviewed", "--evidence", "target/review.md")
        self.assertEqual(code, 0, out)
        r.edit("me", "t3", "docs/a.md", "more notes\n")
        self.assertEqual(self.check_states("me")["review_contract"], "ok")
        r.edit("me", "t4", "crates/core/src/b.rs", "fn other() {}\n")
        self.assertEqual(self.check_states("me")["review_contract"], "MISSING/STALE")

    def test_a_diff_line_containing_the_block_marker_does_not_hide_later_hunks(self):
        r = self.repo
        r.prompt("me")
        marker = "diff " + "--git "  # split so this file's own diff stays clean
        r.edit("me", "t1", "crates/core/src/tracked.rs",
               f'fn base() {{}}\nconst NOTE: &str = "{marker}a/x b/x";\nuse std::sync::Mutex<u8>;\n')
        self.assertIn("review_contract", self.check_states("me"))

    def test_committed_work_leaves_the_task(self):
        r = self.repo
        r.prompt("me")
        r.edit("me", "t1", "docs/a.md", "mine\n")
        r.edit("me", "t2", "docs/b.md", "mine too\n")
        r.git("add", "docs/a.md")
        r.git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "a")
        self.assertEqual(r.task("me"), {"docs/b.md"})

    def test_a_peer_commit_during_my_call_is_not_my_write(self):
        r = self.repo
        r.prompt("me")
        r.prompt("peer")
        r.edit("peer", "t1", "docs/c.md", "peer work\n")
        r.hook("pre", session_id="me", tool_name="Bash", tool_use_id="t2", tool_input={})
        r.git("add", "docs/c.md")
        r.git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "peer")
        r.write("docs/a.md", "mine\n")
        r.hook("post", session_id="me", tool_name="Bash", tool_use_id="t2", tool_input={})
        self.assertEqual(r.task("me"), {"docs/a.md"})

    def test_verify_runs_only_named_required_auto_checks(self):
        r = self.repo
        r.prompt("me")
        r.edit("me", "t1", "docs/a.md", "notes\n")  # docs_only -> tests_layout (auto)
        code, out = r.cmd("me", "verify", "review_policy")
        self.assertEqual(code, 2, out)  # manual, and not required here
        code, out = r.cmd("me", "verify", "tests_workflow")
        self.assertEqual(code, 2, out)  # automatic, but not required for a docs-only diff

    def commit(self, *paths):
        self.repo.git("add", *paths)
        self.repo.git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "c")

    def test_committing_owned_work_is_announced_once(self):
        r = self.repo
        r.prompt("me")
        r.edit("me", "t1", "crates/app/src/render/x.rs", "fn a() {}\n")
        self.commit("crates/app/src/render/x.rs")
        first = json.dumps(r.hook("stop", session_id="me"))
        self.assertIn("committed since the task began", first)
        self.assertIn("crates/app/src/render/x.rs", first)
        self.assertNotIn("committed since", json.dumps(r.hook("stop", session_id="me")))

    def test_soft_reset_puts_committed_work_back_in_the_task(self):
        r = self.repo
        r.prompt("me")
        r.edit("me", "t1", "docs/a.md", "mine\n")
        self.commit("docs/a.md")
        self.assertEqual(r.task("me"), set())
        r.git("reset", "-q", "--soft", "HEAD~1")
        self.assertEqual(r.task("me"), {"docs/a.md"})

    def test_stop_reports_but_never_blocks_when_the_gate_itself_fails(self):
        r = self.repo
        r.prompt("me")
        r.write(".claude/verify/policy.json", "{ not json")
        out = subprocess.run([sys.executable, "-I", "scripts/workflow.py", "hook", "stop"],
                             input=json.dumps({"session_id": "me"}), capture_output=True, text=True,
                             cwd=r.root, env=CLEAN_ENV)
        report = json.loads(out.stdout)
        self.assertNotEqual(report.get("decision"), "block")
        self.assertIn("unverified", report.get("systemMessage", ""))
        again = subprocess.run([sys.executable, "-I", "scripts/workflow.py", "hook", "stop"],
                               input=json.dumps({"session_id": "me", "stop_hook_active": True}),
                               capture_output=True, text=True, cwd=r.root, env=CLEAN_ENV)
        self.assertNotIn("block", again.stdout, "never loop: a continued Stop only reports")

    def test_named_verify_exits_5_while_other_auto_checks_remain(self):
        r = self.repo
        pol = json.loads((r.root / ".claude" / "verify" / "policy.json").read_text(encoding="utf-8"))
        ok = {"kind": "auto", "argv": [sys.executable, "-c", "pass"]}
        pol["checks"].update({"quick_a": ok, "quick_b": ok})
        pol["routes"] = [{"id": "everything", "paths": ["*"], "checks": ["quick_a", "quick_b"]}]
        (r.root / ".claude" / "verify" / "policy.json").write_text(json.dumps(pol), encoding="utf-8")
        r.prompt("me")
        r.edit("me", "t1", "docs/a.md", "x\n")
        code, out = r.cmd("me", "verify", "quick_a")
        self.assertEqual(code, 5, out)
        self.assertIn("PENDING auto quick_b", out)
        code, out = r.cmd("me", "verify")
        self.assertEqual(code, 0, out)

    def test_malformed_disown_keeps_the_path_owned(self):
        r = self.repo
        r.prompt("me")
        r.shell("me", "t1", lambda: r.write("docs/b.md", "peer's\n"))
        path = r.root / "target" / "workflow" / "sessions" / "me.json"
        state = json.loads(path.read_text())
        state["disowned"] = {"docs/b.md": {"seq": "garbage"}}
        path.write_text(json.dumps(state))
        self.assertEqual(r.task("me"), {"docs/b.md"})

    def test_a_legacy_log_line_cannot_be_disowned(self):
        r = self.repo
        r.prompt("me")
        r.write("docs/b.md", "written before attribution had sources\n")
        log = r.root / "target" / "workflow" / "sessions" / "me.owned.txt"
        log.write_text("docs/b.md\n", encoding="utf-8")
        self.assertEqual(r.task("me"), {"docs/b.md"})
        self.assertEqual(r.cmd("me", "disown", "docs/b.md", "--note", "x")[0], 2)

    def test_owners_lists_every_session_that_wrote_a_path(self):
        r = self.repo
        r.prompt("me")
        r.prompt("peer")
        r.edit("me", "t1", "docs/a.md", "mine\n")
        r.edit("peer", "t2", "docs/a.md", "theirs\n")
        code, out = r.cmd("me", "owners", "docs/a.md", "docs/b.md")
        self.assertEqual(code, 0, out)
        first, second = out.splitlines()
        self.assertIn("me", first)
        self.assertIn("peer", first)
        self.assertIn("no ownership session", second)

    def test_peer_edits_do_not_make_the_stop_hook_block(self):
        r = self.repo
        r.prompt("me")
        r.prompt("peer")
        r.shell("peer", "t1", lambda: r.write("crates/x.rs", "fn main() {}\n"))
        self.assertIsNone(r.hook("stop", session_id="me"))


if __name__ == "__main__":
    unittest.main(verbosity=1)
