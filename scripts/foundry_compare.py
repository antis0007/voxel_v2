#!/usr/bin/env python3
"""Compare every recorded pass; insufficient evidence never permits integration."""
import argparse
import csv
import json
import math
import random
import statistics
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from foundry import PROTOCOL, SCHEMA, canonical, digest, frame_bench

METRICS = ("p95_frame_ms", "p99_frame_ms")


def validate(path):
    path = Path(path).resolve()
    r = json.loads(path.read_text(encoding="utf-8"))
    reasons = []
    if r.get("schema") != SCHEMA:
        reasons.append("unsupported receipt schema")
    if canonical(r.get("protocol")) != r.get("protocol_sha256"):
        reasons.append("protocol hash mismatch")
    if r.get("protocol_sha256") != canonical(PROTOCOL):
        reasons.append("receipt does not use this verifier's protocol")
    artifacts = r.get("artifacts", {})
    if not artifacts:
        reasons.append("missing artifact manifest")
    for name, expected in artifacts.items():
        p = (path.parent / name).resolve()
        if not p.is_relative_to(path.parent) or not p.is_file() or digest(p) != expected:
            reasons.append(f"missing or changed artifact: {name}")
    runs = r.get("runs", [])
    if len(runs) < 3:
        reasons.append("fewer than three independent passes")
    for run in runs:
        prefix = f"pass-{run.get('index', -1):03d}/"
        required = [prefix + "stdout.log", prefix + "stderr.log", prefix + "frames.csv"]
        if any(name not in artifacts for name in required) or not any(
                name.startswith(prefix) and name.endswith(".png") for name in artifacts):
            reasons.append("pass lacks raw timing, logs or images")
        if run.get("status") != "captured" or run.get("exit_code") != 0:
            reasons.append("failed or incomplete pass")
        log = path.parent / prefix / "stdout.log"
        frames = path.parent / prefix / "frames.csv"
        try:
            parsed = frame_bench.parse_report(log.read_text(encoding="utf-8"))
            if parsed != run.get("metrics"):
                reasons.append("summary does not match raw log")
            if parsed.get("res") != "1600x900" or not parsed.get("adapter"):
                reasons.append("unexpected resolution or missing adapter")
            with frames.open(encoding="utf-8", newline="") as stream:
                rows = list(csv.DictReader(stream))
            if not rows or any(not math.isfinite(float(row["total_ms"])) or
                               float(row["total_ms"]) < 0 for row in rows):
                reasons.append("invalid per-frame timing evidence")
        except (OSError, ValueError, KeyError, frame_bench.BenchError):
            reasons.append("unreadable raw timing evidence")
        if run.get("contention_before") or run.get("contention_after"):
            reasons.append("observed competing engine/build process")
        for key in METRICS:
            value = run.get("metrics", {}).get(key)
            if not isinstance(value, (int, float)) or not math.isfinite(value) or value <= 0:
                reasons.append(f"invalid metric: {key}")
    if not r.get("source_unchanged") or not r.get("binary_unchanged"):
        reasons.append("source or executable changed during collection")
    if not r.get("hardware", {}).get("complete"):
        reasons.append("active adapter could not be matched in every pass" if
                       r.get("hardware", {}).get("active_adapter_matched") is False else
                       "hardware/driver fingerprint unavailable")
    adapters = {run.get("metrics", {}).get("adapter") for run in runs
                if run.get("metrics", {}).get("adapter")}
    if any(not run.get("metrics", {}).get("adapter") for run in runs):
        reasons.append("active adapter identity missing from an incomplete pass")
    if len(adapters) > 1:
        reasons.append("adapter changed between passes")
    return r, reasons


def interval(a, b):
    """Seeded pass-level bootstrap; frames within a pass are correlated."""
    rng = random.Random(74)
    samples = sorted(statistics.mean(rng.choices(b, k=len(b))) /
                     statistics.mean(rng.choices(a, k=len(a))) - 1 for _ in range(4000))
    return [samples[100], samples[3899]]


def compare(a, b, reasons=()):
    reasons = list(reasons)
    for key in ("protocol_sha256", "backend", "runner_sha256"):
        if a.get(key) != b.get(key):
            reasons.append(f"incomparable {key}")
    if a.get("hardware", {}).get("fingerprint") != b.get("hardware", {}).get("fingerprint"):
        reasons.append("different hardware or driver")
    if {r.get("metrics", {}).get("adapter") for r in a.get("runs", []) if r.get("metrics", {}).get("adapter")} != {
            r.get("metrics", {}).get("adapter") for r in b.get("runs", []) if r.get("metrics", {}).get("adapter")}:
        reasons.append("different active adapter")
    if reasons:
        return {"status": "unverified", "reasons": sorted(set(reasons)), "merge_authorized": False}
    metrics = {}
    for key in METRICS:
        av = [r["metrics"][key] for r in a["runs"]]
        bv = [r["metrics"][key] for r in b["runs"]]
        metrics[key] = {"baseline_passes": av, "candidate_passes": bv,
                        "mean_relative_change": statistics.mean(bv) / statistics.mean(av) - 1,
                        "bootstrap_95_interval": interval(av, bv)}
    regression = any(m["bootstrap_95_interval"][0] > 0.05 for m in metrics.values())
    improvement = all(m["bootstrap_95_interval"][1] < 0 for m in metrics.values())
    return {"status": "timing-regression" if regression else "timing-improvement-candidate" if improvement else "inconclusive",
            "metrics": metrics, "merge_authorized": False,
            "reasons": ["Timing alone cannot establish visual/physics correctness or Pareto dominance.",
                        "No independent verifier, hidden scenes or clean-build attestation in this slice."]}


def calibrate(a, b, reasons=()):
    reasons = list(reasons)
    if not a.get("build", {}).get("sha256") or a.get("build", {}).get("sha256") != b.get("build", {}).get("sha256"):
        reasons.append("calibration requires the identical executable hash")
    result = compare(a, b, reasons)
    result["mode"] = "same-binary-calibration"
    if result["status"] != "unverified":
        intervals = [m["bootstrap_95_interval"] for m in result["metrics"].values()]
        stable = all(-0.05 <= low <= high <= 0.05 for low, high in intervals)
        unstable = any(low > 0.05 or high < -0.05 for low, high in intervals)
        result["status"] = "calibration-stable" if stable else "calibration-unstable" if unstable else "calibration-inconclusive"
        result["reasons"] = ["Identical binary, protocol and hardware context; observed differences are measurement variability.",
                             "This calibration does not measure an engine improvement or establish correctness."]
    return result


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("baseline", type=Path)
    p.add_argument("candidate", type=Path)
    p.add_argument("--calibration", action="store_true", help="Compare repeated batches of the identical executable")
    args = p.parse_args()
    a, ar = validate(args.baseline)
    b, br = validate(args.candidate)
    result = (calibrate if args.calibration else compare)(a, b, ar + br)
    print(json.dumps(result, indent=2, allow_nan=False))
    return 2 if result["status"] == "unverified" else 1 if result["status"] in {"timing-regression", "calibration-unstable"} else 0


if __name__ == "__main__":
    sys.exit(main())
