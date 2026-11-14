"""Compare compatible native workload reports; frames are not independent runs."""
import argparse
import json
import statistics
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from foundry import canonical, digest
from foundry_compare import interval
from foundry_perf import SCHEMA, analyse, load


def checked_report(path):
    path = path.resolve(strict=True)
    r = load(path)
    if r.get("schema") != SCHEMA:
        raise ValueError("Unsupported performance schema")
    if r.get("analyzer_sha256") != digest(Path(__file__).with_name("foundry_perf.py")) or r.get("metrics_analyzer_sha256") != digest(Path(__file__).with_name("foundry_perf_metrics.py")):
        raise ValueError("Analyzer version differs; retain this report and re-analyse into a new output")
    if r.get("headless_analyzer_sha256") != digest(Path(__file__).with_name("foundry_perf_headless.py")):
        raise ValueError("Headless analyzer version differs")
    if not r.get("raw_artifacts"):
        raise ValueError("Raw artifact manifest missing")
    for name, expected in r["raw_artifacts"].items():
        p = (path.parent / name).resolve()
        if not p.is_relative_to(path.parent) or not p.is_file() or digest(p) != expected:
            raise ValueError("Changed or unavailable raw artifact: " + name)
    raw = path.parent / "raw"
    rebuilt = analyse(raw, load(raw / "context.json"), r["frame_budget_ms"])
    if any(r.get(key) != value for key, value in rebuilt.items()):
        raise ValueError("Report does not agree with raw evidence")
    return r


def compare_reports(baseline, candidate, tolerance=.05):
    if any("headless" in r for r in baseline + candidate):
        from foundry_perf_headless import compare
        return compare(baseline, candidate, tolerance)
    all_reports = baseline + candidate
    reasons = []
    if not baseline or not candidate:
        reasons.append("Both baseline and candidate runs are required")
    for key in ("workload", "hardware_fingerprint", "frame_budget_ms"):
        values = [canonical(r.get(key)) for r in all_reports]
        if len(set(values)) != 1 or any(r.get(key) is None for r in all_reports):
            reasons.append("Different or missing " + key)
    adapters = {r.get("native_summary", {}).get("adapter") for r in all_reports}
    resolutions = {r.get("native_summary", {}).get("res") for r in all_reports}
    configs = {r.get("build", {}).get("configuration_hash") for r in all_reports}
    if None in configs or len(configs) != 1:
        reasons.append("Build configuration differs or is missing")
    native_configs = {canonical({k: r.get("native_summary", {}).get(k)
                                for k in ("res", "view_chunks", "lod_start", "lod_levels", "motion")}) for r in all_reports}
    if len(native_configs) != 1:
        reasons.append("Native render settings differ")
    if None in adapters or len(adapters) != 1:
        reasons.append("Active adapter differs or is missing")
    if None in resolutions or len(resolutions) != 1:
        reasons.append("Native resolution differs or is missing")
    for group in (baseline, candidate):
        if len({r.get("binary_sha256") for r in group}) != 1 or any(not r.get("binary_sha256") for r in group):
            reasons.append("Each variant needs one exact executable identity")
        if len({r.get("source_capture") for r in group}) != len(group):
            reasons.append("Duplicate source captures cannot count as independent passes")
    for r in all_reports:
        t = r.get("telemetry")
        if not t or not t.get("frame_ms") or t.get("invalid_frame_rows"):
            reasons.append("Missing or invalid frame telemetry")
        if not r.get("native_summary", {}).get("p95_frame_ms"):
            reasons.append("No completed benchmark summary")
        if any("Native benchmark reported failure" in x or "sample count differs" in x or "disagrees with the native" in x for x in r.get("reliability_issues", [])):
            reasons.append("Failed or inconsistent capture")
    for group in (baseline, candidate):
        hashes = [r.get("raw_artifacts", {}).get("raw/frames.csv") for r in group]
        if len(group) > 1 and len(set(hashes)) != len(hashes):
            reasons.append("Duplicate frame artifacts cannot count as independent passes")
    result = {"schema": "axiom.foundry.performance-comparison.v1", "merge_authorized": False,
              "status": "incomparable" if reasons else "diagnostic-only",
              "reasons": sorted(set(reasons)), "metrics": {}, "baseline_runs": len(baseline), "candidate_runs": len(candidate)}
    if reasons:
        return result
    repeated = len(baseline) >= 3 and len(candidate) >= 3
    for key in ("p50", "p95", "p99", "max"):
        a = [r["telemetry"]["frame_ms"][key] for r in baseline]
        b = [r["telemetry"]["frame_ms"][key] for r in candidate]
        if statistics.mean(a) <= 0:
            result["reasons"].append("Zero baseline duration cannot establish a relative change")
            continue
        delta = statistics.mean(b) / statistics.mean(a) - 1
        result["metrics"][key] = {"baseline_passes_ms": a, "candidate_passes_ms": b,
                                  "mean_relative_change": delta, "observed_over_tolerance": delta > tolerance,
                                  "pass_bootstrap_95_interval": interval(a, b) if repeated else None}
    result["reasons"] = ["Repeated independent passes are required for a pass-level uncertainty estimate."] if not repeated else []
    result["reasons"] += ["Timing differences do not establish causality, visible work equivalence or independent build verification.",
                           "Reported observations cannot authorize an engine improvement or merge."]
    if repeated and any(m["pass_bootstrap_95_interval"][0] > tolerance for m in result["metrics"].values()):
        result["status"] = "timing-regression-observed"
    return result


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--baseline", action="append", required=True, type=Path)
    p.add_argument("--candidate", action="append", required=True, type=Path)
    p.add_argument("--output", required=True, type=Path)
    args = p.parse_args()
    result = compare_reports([checked_report(x) for x in args.baseline], [checked_report(x) for x in args.candidate])
    with args.output.open("x", encoding="utf-8") as stream:
        stream.write(json.dumps(result, indent=2, allow_nan=False) + "\n")
    print(result["status"])
    sys.exit(2 if result["status"] == "incomparable" else 1 if result["status"] == "timing-regression-observed" else 0)
