#!/usr/bin/env python3
"""End-of-session numbers: LOC, tests, scenarios, benches and wall time.

Usage:
    python -I scripts/session_report.py [base-rev]

Compares the working tree (including untracked files) with `base-rev`, which
defaults to the merge base of HEAD and `main`. Bench numbers are the last
`just bench` results in target/criterion against the committed baselines; this
script never runs benches itself.
"""
import json
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CODE = (".rs", ".wgsl")
TEST = re.compile(r"#\[test\]")


def git(*args, check=True):
    """stdout of a git command; with check=False a failure (no match, no path) is empty."""
    proc = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True,
                          encoding="utf-8", errors="replace")
    if check and proc.returncode != 0:
        sys.exit(proc.stderr.strip())
    return proc.stdout if proc.returncode == 0 else ""


def loc_delta(base):
    added = removed = 0
    for line in git("diff", "--numstat", base, "--", "crates").splitlines():
        a, r, path = line.split("\t", 2)
        if path.endswith(CODE) and a != "-":
            added, removed = added + int(a), removed + int(r)
    for path in git("ls-files", "--others", "--exclude-standard", "--", "crates").splitlines():
        if path.endswith(CODE):
            added += len((ROOT / path).read_text(encoding="utf-8", errors="replace").splitlines())
    return added, removed


def tests_at(base):
    if base is None:
        files = [p for p in ROOT.glob("crates/**/*.rs") if "target" not in p.parts]
        return sum(len(TEST.findall(p.read_text(encoding="utf-8", errors="replace"))) for p in files)
    counts = git("grep", "-c", r"#\[test\]", base, "--", "crates/*.rs", check=False).splitlines()
    return sum(int(line.rsplit(":", 1)[1]) for line in counts)


def scenarios_at(base):
    if base is None:
        return len(list((ROOT / "assets" / "scenarios").glob("*.*")))
    return len(git("ls-tree", "--name-only", f"{base}:assets/scenarios", check=False).split())


def criterion_dir():
    """The same directory bench.py reads: CRITERION_HOME, else cargo's target directory."""
    import importlib.util

    spec = importlib.util.spec_from_file_location("bench", ROOT / "scripts" / "bench.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.criterion_dir()


def bench_lines():
    lines = []
    results = criterion_dir()
    for baselines in sorted(ROOT.glob("crates/*/benches/baselines.json")):
        base = json.loads(baselines.read_text(encoding="utf-8"))
        for bench, ns in sorted(base.items()):
            est = results / bench / "new" / "estimates.json"
            if not est.is_file():
                lines.append(f"  {bench}: not run this session (baseline {ns / 1e6:.3f} ms)")
                continue
            now = json.loads(est.read_text(encoding="utf-8"))["median"]["point_estimate"]
            age = (time.time() - est.stat().st_mtime) / 60
            lines.append(f"  {bench}: {now / 1e6:.3f} ms vs {ns / 1e6:.3f} ms ({now / ns - 1:+.1%}, run {age:.0f} min ago)")
    return lines or ["  no benches"]


def worktree_summary():
    sys.path.insert(0, str(ROOT / "scripts"))
    import worktrees

    return worktrees.summary()


def main():
    base = sys.argv[1] if len(sys.argv) > 1 else git("merge-base", "HEAD", "main").strip()
    started = int(git("log", "-1", "--format=%ct", base).strip())
    added, removed = loc_delta(base)
    t0, t1 = tests_at(base), tests_at(None)
    s0, s1 = scenarios_at(base), scenarios_at(None)
    print(f"session report vs {base[:10]}")
    print(f"  LOC (rs+wgsl): +{added} -{removed} = {added - removed:+d}")
    print(f"  #[test] fns:   {t0} -> {t1} ({t1 - t0:+d})")
    print(f"  scenarios:     {s0} -> {s1} ({s1 - s0:+d})")
    print(f"  wall time:     {(time.time() - started) / 3600:.1f} h since {base[:10]}")
    print(f"  worktrees:     {worktree_summary()}")
    print("benches (last run vs baseline):")
    print("\n".join(bench_lines()))


if __name__ == "__main__":
    main()
