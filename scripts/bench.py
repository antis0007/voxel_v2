#!/usr/bin/env python3
"""Criterion benches with committed baselines and a perf gate.

Usage:
    python -I scripts/bench.py run [filter]    one run; print median vs baseline
    python -I scripts/bench.py check           the gate: median of RUNS runs, fail above THRESHOLD
    python -I scripts/bench.py save [filter]   median of RUNS runs written as the new baseline

Every crate with a `benches/` directory is benched. Its baselines live in
`crates/<crate>/benches/baselines.json` as {bench id: median nanoseconds}.
`check` fails on a bench slower than baseline by more than THRESHOLD and on a
bench with no baseline (record one with `just bench-save`), after one
re-measurement, because concurrent builds skew a pass. Runs are whole
passes over every bench, so A/B noise is interleaved rather than batched
(docs/adr/0006-benches-and-perf-gate.md).
"""
import json
import statistics
import subprocess
import sys
import time
from pathlib import Path

RUNS = 3
THRESHOLD = 0.08
ROOT = Path(__file__).resolve().parent.parent
CRITERION = ROOT / "target" / "criterion"


def bench_crates():
    """(package name, crate dir) for every crate that has benches."""
    out = []
    for manifest in sorted(ROOT.glob("crates/*/Cargo.toml")):
        if (manifest.parent / "benches").is_dir():
            for line in manifest.read_text(encoding="utf-8").splitlines():
                if line.startswith("name = "):
                    out.append((line.split('"')[1], manifest.parent))
                    break
    return out


def run_once(package, flt):
    """Run one crate's benches; return {bench id: median ns} for benches this run wrote."""
    started = time.time()
    cmd = ["cargo", "bench", "-q", "-p", package, "--benches", "--"]
    if flt:
        cmd.append(flt)
    proc = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace")
    if proc.returncode != 0:
        sys.stdout.write(proc.stdout[-3000:] + proc.stderr[-3000:])
        sys.exit(proc.returncode)
    results = {}
    for meta in CRITERION.glob("**/new/benchmark.json"):
        if meta.stat().st_mtime < started:
            continue
        estimates = json.loads((meta.parent / "estimates.json").read_text(encoding="utf-8"))
        results[json.loads(meta.read_text(encoding="utf-8"))["full_id"]] = estimates["median"]["point_estimate"]
    return results


def measure(runs, flt):
    """{package: (crate dir, {bench id: [median ns per run]})}, runs interleaved across crates."""
    samples = {pkg: (path, {}) for pkg, path in bench_crates()}
    for _ in range(runs):
        for pkg, (_, benches) in samples.items():
            for bench, ns in run_once(pkg, flt).items():
                benches.setdefault(bench, []).append(ns)
    return samples


def load_baseline(path):
    file = path / "benches" / "baselines.json"
    return json.loads(file.read_text(encoding="utf-8")) if file.is_file() else {}


def fmt(ns):
    return f"{ns / 1e6:9.3f} ms" if ns >= 1e5 else f"{ns / 1e3:9.3f} us"


def report(samples):
    """Print one line per bench; return the number of gate failures."""
    failures = 0
    for pkg, (path, benches) in samples.items():
        base = load_baseline(path)
        for bench, runs in sorted(benches.items()):
            now = statistics.median(runs)
            if bench not in base:
                failures += 1
                print(f"FAIL {pkg}/{bench}: {fmt(now)}  no baseline (just bench-save)")
                continue
            delta = now / base[bench] - 1.0
            bad = delta > THRESHOLD
            failures += bad
            print(f"{'FAIL' if bad else 'ok  '} {pkg}/{bench}: {fmt(now)} vs {fmt(base[bench])} ({delta:+.1%}, n={len(runs)})")
    return failures


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "run"
    flt = sys.argv[2] if len(sys.argv) > 2 else ""
    if mode == "run":
        report(measure(1, flt))
    elif mode == "check":
        failures = report(measure(RUNS, ""))
        if failures:
            # Peer builds on a shared workstation skew whole passes; a real regression fails twice.
            print("over threshold; measuring once more before failing")
            failures = report(measure(RUNS, ""))
        print(f"bench gate: {failures} failure(s), median of {RUNS} runs, threshold +{THRESHOLD:.0%}")
        sys.exit(1 if failures else 0)
    elif mode == "save":
        for pkg, (path, benches) in measure(RUNS, flt).items():
            base = load_baseline(path)
            base.update({bench: round(statistics.median(runs)) for bench, runs in benches.items()})
            file = path / "benches" / "baselines.json"
            file.write_text(json.dumps(base, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
            print(f"saved {len(benches)} baseline(s) for {pkg} -> {file.relative_to(ROOT).as_posix()}")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
