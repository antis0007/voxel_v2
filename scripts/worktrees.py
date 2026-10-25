#!/usr/bin/env python3
"""Classify linked git worktrees and remove the ones that are safe to drop.

Usage:
    python -I scripts/worktrees.py [report|remove] [--hours N]

`report` (default) prints one line per worktree. `remove` runs
`git worktree remove` (never --force) on every REMOVE row and prunes
registrations whose directory is gone.

A worktree is REMOVE only when all hold: it is not the primary checkout, not
locked, has no tracked or untracked changes, its HEAD is reachable from a branch
or tag, its ignored files are build outputs only, and neither it nor the
session that owns its scratchpad has been active for N hours (default 24).
Anything else is kept with the reason that kept it.
"""
import os
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_HOURS = 24
# Ignored entries that cargo or tools regenerate. Anything else under an ignored
# path (review captures, saves, downloads) is evidence and blocks removal.
BUILD_OUTPUTS = {"debug", "release", "release-fast", ".rustc_info.json", "CACHEDIR.TAG", ".cargo-lock", "doc"}
SCRATCH_DIRS = {"__pycache__", ".superpowers", ".code-advisor"}


def git(cwd, *args):
    # No optional locks: a report must not contend with another session's git commands.
    proc = subprocess.run(["git", "--no-optional-locks", "-C", str(cwd), *args], capture_output=True,
                          text=True, encoding="utf-8", errors="replace")
    return proc.stdout.strip() if proc.returncode == 0 else ""


def listed(root=ROOT):
    """Worktree records from `git worktree list --porcelain`, primary first."""
    records, cur = [], {}
    for line in git(root, "worktree", "list", "--porcelain").splitlines() + [""]:
        if line:
            key, _, value = line.partition(" ")
            cur[key] = value
        elif cur:
            records.append(cur)
            cur = {}
    return records


def newest_mtime(*paths):
    return max((os.path.getmtime(p) for p in paths if os.path.exists(p)), default=0.0)


def target_activity(target):
    """Newest directory mtime in target/ down to depth 3; a build in progress touches these."""
    newest, level = [os.path.getmtime(target)], [target]
    for _ in range(3):
        level = [e.path for d in level for e in os.scandir(d) if e.is_dir(follow_symlinks=False)]
        newest += [os.path.getmtime(d) for d in level]
    return max(newest)


def owner_transcript(path):
    """Claude transcript of the session whose scratchpad holds `path`, if it does."""
    parts = Path(path).parts
    if "scratchpad" not in parts or parts.index("scratchpad") < 2:
        return None
    i = parts.index("scratchpad")
    return Path.home() / ".claude" / "projects" / parts[i - 2] / (parts[i - 1] + ".jsonl")


def evidence_of(path):
    """Ignored entries that are not regenerable build output."""
    found = []
    for line in git(path, "status", "--porcelain", "--ignored").splitlines():
        if not line.startswith("!!"):
            continue
        entry = line[3:].rstrip("/")
        if entry == "target":
            found += [f"target/{e.name}" for e in os.scandir(Path(path) / "target") if e.name not in BUILD_OUTPUTS]
        elif entry.split("/")[-1] not in SCRATCH_DIRS:
            found.append(entry)
    return found


def facts(record, primary, now):
    path = record["worktree"]
    out = {"path": path, "primary": primary, "locked": "locked" in record,
           "branch": record.get("branch", "").removeprefix("refs/heads/") or "(detached)",
           "head": record["HEAD"][:8], "exists": os.path.isdir(path) and "prunable" not in record}
    if not out["exists"]:
        return out
    out["dirty"] = len(git(path, "status", "--porcelain").splitlines())
    out["saved"] = bool(git(path, "for-each-ref", "--contains", "HEAD", "--count=1", "refs/heads", "refs/tags"))
    out["evidence"] = evidence_of(path)
    gitdir = Path(git(path, "rev-parse", "--absolute-git-dir"))
    target = Path(path) / "target"
    last = max(newest_mtime(gitdir / "index", gitdir / "HEAD"),
               float(git(path, "log", "-1", "--format=%ct") or 0),
               target_activity(target) if target.is_dir() else 0.0)
    out["idle_h"] = max(0.0, (now - last) / 3600)
    transcript = owner_transcript(path)
    out["owner_idle_h"] = max(0.0, (now - transcript.stat().st_mtime) / 3600) if transcript and transcript.exists() else None
    return out


def classify(f, hours=DEFAULT_HOURS):
    """(verdict, reason) for one worktree's facts."""
    if f["primary"]:
        return "KEEP", "primary checkout"
    if not f["exists"]:
        return "PRUNE", "directory is gone"
    if f["locked"]:
        return "KEEP", "locked"
    if f["dirty"]:
        return "KEEP", f"{f['dirty']} changed files"
    if not f["saved"]:
        return "KEEP", "HEAD is on no branch or tag"
    if f["evidence"]:
        return "KEEP", "evidence: " + ", ".join(f["evidence"][:3])
    if f["idle_h"] < hours:
        return "KEEP", f"active {f['idle_h']:.0f} h ago"
    if f["owner_idle_h"] is not None and f["owner_idle_h"] < hours:
        return "KEEP", f"owner session active {f['owner_idle_h']:.0f} h ago"
    return "REMOVE", f"idle {f['idle_h']:.0f} h, clean, saved on a ref"


def survey(hours=DEFAULT_HOURS, root=ROOT):
    now = time.time()
    return [(f, *classify(f, hours)) for f in (facts(r, i == 0, now) for i, r in enumerate(listed(root)))]


def summary(hours=DEFAULT_HOURS):
    rows = survey(hours)
    removable = sum(1 for _, v, _ in rows if v in ("REMOVE", "PRUNE"))
    return f"{len(rows)} registered, {removable} removable (just worktrees)"


def main(argv):
    hours = DEFAULT_HOURS
    if "--hours" in argv:
        hours = float(argv[argv.index("--hours") + 1])
    rows = survey(hours)
    for f, verdict, reason in rows:
        print(f"{verdict:6} {f['branch'][:28]:28} {f['head']} {f['path']}  [{reason}]")
    print(summary(hours))
    if "remove" not in argv:
        return 0
    status = 0
    for f, verdict, _ in rows:
        if verdict == "REMOVE":
            proc = subprocess.run(["git", "-C", str(ROOT), "worktree", "remove", f["path"]],
                                  capture_output=True, text=True)
            print(("removed " if proc.returncode == 0 else "failed  ") + f["path"] + " " + proc.stderr.strip())
            status |= proc.returncode
        elif verdict == "PRUNE":
            git(ROOT, "worktree", "prune")
    return status


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
