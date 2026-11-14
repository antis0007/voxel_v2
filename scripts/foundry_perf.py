"""Snapshot a completed native workload and emit bounded human/agent diagnostics."""
import argparse
import json
import shutil
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from foundry import canonical, digest, write
import frame_bench
from foundry_perf_metrics import CPU, GPU, auxiliary, read_frames, read_route

SCHEMA = "axiom.foundry.performance.v1"
FILES = ("frames.csv", "stdout.log", "stderr.log", "manifest.json", "route-timing.json", "route-frames.csv", "process.json")
MAX_BYTES = 64 * 1024 * 1024
HINTS = {
    "render_cpu_ms": ("crates/app/src/render.rs", "Measure dynamic resident model construction and uploads inside the CPU render span; separate this from driver waits."),
    "gameplay_ms": ("crates/app/src/world/farming/jobs", "Split job search, pathfinding and fixed-step counts; preserve cargo, paid placement and collision semantics."),
    "stream_ms": ("crates/app/src/streaming", "Measure generation, meshing and upload queues separately, alongside bytes and edited cells."),
    "horizon_ms": ("crates/app/src/render/horizon.rs", "Separate horizon CPU work from worker backlog and GPU upload waits."),
    "gpu_pass_ms": ("crates/app/src/render.rs", "Capture GPU stages at identical resolution, resident geometry and visual features before changing shading."),
    "terrain_ms": ("crates/app/src/shaders", "Profile terrain visibility and shading against identical geometry and material features; this GPU span is nested in the total."),
}


def load(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def analyse(raw, context, budget_ms):
    kind = context.get("workload_kind", "native")
    if kind != "native":
        from foundry_perf_headless import analyse as headless_analyse
        return headless_analyse(raw, context, budget_ms)
    manifest = load(raw / "manifest.json") if (raw / "manifest.json").exists() else {}
    stdout = (raw / "stdout.log").read_text(encoding="utf-8-sig", errors="replace") if (raw / "stdout.log").exists() else ""
    stderr = (raw / "stderr.log").read_text(encoding="utf-8-sig", errors="replace") if (raw / "stderr.log").exists() else ""
    telemetry = read_frames(raw / "frames.csv", budget_ms) if (raw / "frames.csv").exists() else None
    args = manifest.get("arguments", manifest.get("args", []))
    issues = []
    try:
        native_summary = frame_bench.parse_report(stdout)
    except frame_bench.BenchError:
        native_summary = {}
        issues.append("No completed native benchmark summary.")
    if not telemetry or not telemetry["rows"]:
        issues.append("No measured frame CSV; startup or scene failure cannot be reported as a fast workload.")
    elif telemetry["invalid_frame_rows"]:
        issues.append("Invalid frame timings retained in raw evidence; quantiles exclude the explicitly listed invalid rows.")
    if telemetry and native_summary.get("samples") is not None and native_summary["samples"] != telemetry["rows"]:
        issues.append("Native summary sample count differs from retained CSV rows.")
    if "[benchmark] failed:" in stdout + stderr:
        issues.append("Native benchmark reported failure.")
    if manifest.get("exit", manifest.get("exit_code")) not in (0,):
        issues.append("A successful process exit is not recorded in the source manifest.")
    if not context.get("hardware"):
        issues.append("Hardware/driver identity is missing.")
    if not context.get("build", {}).get("configuration_hash"):
        issues.append("Build configuration is not pinned.")
    if not context.get("build", {}).get("source_snapshot"):
        issues.append("Binary source snapshot is not pinned.")
    declared = manifest.get("assigned_workers", manifest.get("population"))
    native_seed = native_summary.get("starting.seed")
    if native_seed is None:
        issues.append("Native world seed is not present in the benchmark log.")
    elif context.get("seed") is not None and context["seed"] != native_seed:
        issues.append("Declared world seed disagrees with the native benchmark log.")
    workload = {"id": context.get("workload_id"), "seed": native_seed if native_seed is not None else context.get("seed"),
                "declared_seed": context.get("seed"), "world_type": native_summary.get("starting.world"),
                "voxel_scale": native_summary.get("starting.voxel_scale"),
                "declared_population": declared, "aggregate": manifest.get("aggregate"),
                "arguments": args, "backend": manifest.get("backend", context.get("backend")),
                "trace": bool(manifest.get("trace", context.get("trace", False))),
                "route_timing": bool(manifest.get("route_timing", context.get("route_timing", False))),
                "captures": bool(manifest.get("captures", "--benchmark-capture-frames" in args)),
                "environment": context.get("workload_environment", {})}
    if not workload["id"] or workload["seed"] is None or not workload["backend"]:
        issues.append("Workload ID, world seed or backend is not pinned.")
    if declared != 20:
        issues.append("This is not a declared 20-resident workload.")
    visible = telemetry.get("visible_actors") if telemetry else None
    if not visible:
        issues.append("Visible actor count is not measured; twenty assigned residents does not establish twenty visible working builders.")
    elif visible["min"] < 20:
        issues.append("Fewer than twenty actors were visible in some measured frames.")
    physical = load(raw / "population.json") if (raw / "population.json").exists() else None
    if not physical:
        issues.append("No physical job/cargo/conservation check accompanies this capture; useful builder work is unverified.")
    elif physical.get("requested") != declared or not all(physical.get(k) is True for k in ("materials_conserved", "save_roundtrip")):
        issues.append("Physical-work evidence differs from this population or failed its invariants.")
    elif not physical.get("placements", 0):
        issues.append("The accompanying physical fixture made no paid placements; assigned or moving actors are not proof of productive construction.")
    if telemetry and telemetry["frame_ms"] and telemetry["frame_ms"]["samples"] < 300:
        issues.append("Fewer than 300 measured frames; p99 has fewer than three tail samples.")
    if telemetry and not telemetry["gpu_ms"].get("gpu_pass_ms"):
        issues.append("No valid, uniquely identified GPU samples.")
    issues.append("Pathfinding, job search and voxel remeshing are not independently timed by the basic frame CSV.")
    issues.append("Resident byte counters cover the reported caches, not total process RAM or all GPU allocations; network cost is unmeasured.")
    findings = []
    if telemetry:
        for group, metrics in (("cpu", telemetry["cpu_ms"]), ("gpu", telemetry["gpu_ms"])):
            choices = [(k, value) for k, value in metrics.items() if k in HINTS]
            for key, value in sorted(choices, key=lambda kv: kv[1]["mean"], reverse=True)[:2]:
                source, action = HINTS[key]
                findings.append({"kind": "observed-span", "domain": group, "metric": key,
                                 "mean_ms": value["mean"], "p95_ms": value["p95"],
                                 "source_hint": source, "next_probe": action,
                                 "causality": "Not established by aggregate timing."})
        residual = telemetry.get("frame_interval_minus_selected_cpu_spans_ms")
        if residual and (residual["p95"] > budget_ms / 2 or residual["max"] > budget_ms * 2):
            findings.append({"kind": "unattributed-interval", "p95_ms": residual["p95"], "max_ms": residual["max"],
                             "severity": "stall" if residual["max"] > max(100, budget_ms * 5) else "investigate",
                             "next_probe": "Capture outside-redraw, frame-pacer, acquire and OS scheduling spans. This arithmetic remainder is not a measured CPU subsystem."})
    route = load(raw / "route-timing.json") if (raw / "route-timing.json").exists() else None
    route_frames = read_route(raw / "route-frames.csv") if (raw / "route-frames.csv").exists() else None
    if workload["route_timing"] and not route_frames:
        issues.append("Route timing was declared but its frame CSV is missing.")
    if route and (route.get("recording_closed") is not True or route.get("csv_dropped_frames", 0) or route.get("csv_io_failed")):
        issues.append("Route recording is incomplete or reports dropped rows.")
    if route_frames:
        if not route_frames["measuring_rows"]:
            issues.append("Route CSV contains no attempts marked as measuring; full-process spans cannot be attributed to the measured workload.")
        if route and route_frames["rows"] != route.get("frames"):
            issues.append("Route summary frame count differs from its CSV.")
        if route_frames["failed_render_rows"]:
            issues.append("Route includes failed render attempts absent from the warmed frame distribution.")
        outside = route_frames["groups_ms"]["measuring_attempts"].get("outside_redraw_ms")
        if outside:
            findings.append({"kind": "observed-span", "domain": "event-loop", "metric": "outside_redraw_ms",
                             "p95_ms": outside["p95"], "mean_ms": outside["mean"], "max_ms": outside["max"],
                             "severity": "stall" if outside["max"] > max(100, budget_ms * 5) else "investigate",
                             "source_hint": "crates/app/src/main.rs",
                             "next_probe": "Correlate the event-loop gap with frame-pacer retries, dispatch and OS scheduling. Outside-redraw wall time is not simulation CPU cost."})
    findings.sort(key=lambda finding: (finding.get("severity") == "stall", finding.get("p95_ms", 0)), reverse=True)
    return {"schema": SCHEMA, "status": "reported-unverified", "frame_budget_ms": budget_ms,
            "budget_scope": "Configurable diagnostic target, not an engine acceptance threshold.",
            "quantile_method": "nearest rank; all finite nonnegative frames retained",
            "workload": workload, "binary_sha256": str(manifest.get("sha256", "")).lower(),
            "build": context.get("build", {}), "hardware": context.get("hardware"),
            "native_summary": native_summary,
            "hardware_fingerprint": canonical(context["hardware"]) if context.get("hardware") else None,
            "telemetry": telemetry, "auxiliary": auxiliary(stdout, stderr), "route_summary": route, "route_frames": route_frames,
            "physical_check": physical, "physical_check_scope": "Separate fixture evidence, not per-frame native visibility or work counters.",
            "reliability_issues": issues, "findings": findings,
            "agent_packet": {"observations": findings, "unmeasured_or_unreliable": issues,
                             "required_invariants": ["real paid work and cargo conservation", "collision and navigation semantics", "save and multiplayer compatibility"],
                             "instruction": "Form hypotheses from these observations. Do not infer causality or change benchmarks/thresholds to improve a score."},
            "merge_authorized": False}


def markdown(report):
    if "headless" in report:
        from foundry_perf_headless import markdown as headless_markdown
        return headless_markdown(report)
    lines = ["# Foundry workload performance", "", "Status: reported, independently unverified.", "",
             "Binary: `" + report["binary_sha256"] + "`", "",
             "Workload: `" + str(report["workload"]["id"]) + "`; declared population " + str(report["workload"]["declared_population"]) + ".", ""]
    t = report["telemetry"]
    if t and t["frame_ms"]:
        f = t["frame_ms"]
        lines += ["| Frames | p50 ms | p95 ms | p99 ms | Max ms | Over diagnostic budget |", "| ---: | ---: | ---: | ---: | ---: | ---: |",
                  f"| {f['samples']} | {f['p50']:.3f} | {f['p95']:.3f} | {f['p99']:.3f} | {f['max']:.3f} | {t['over_budget_frames']} |", ""]
        for domain, labels in (("cpu_ms", CPU), ("gpu_ms", GPU)):
            lines += ["## " + domain.replace("_", " "), "", "Nested spans and CPU/GPU timings must not be added together.", "",
                      "| Span | Mean ms | p95 ms | Samples |", "| --- | ---: | ---: | ---: |"]
            lines += [f"| {labels[k]} | {v['mean']:.3f} | {v['p95']:.3f} | {v['samples']} |" for k, v in t[domain].items()]
            lines.append("")
    lines += ["## Next probes", ""] + ["- " + f["next_probe"] for f in report["findings"]]
    lines += ["", "## Evidence limits", ""] + ["- " + issue for issue in report["reliability_issues"]]
    return "\n".join(lines) + "\n"


def ingest(source, output, context, budget_ms=1000 / 60, physical=None):
    source, output = source.resolve(strict=True), output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    raw = output / "raw"
    raw.mkdir()
    total = 0
    for name in FILES:
        p = source / name
        if not p.exists():
            continue
        if not p.resolve().is_relative_to(source):
            raise ValueError("Input artifact redirects outside workload")
        total += p.stat().st_size
        if total > MAX_BYTES:
            raise ValueError("Capture exceeds the 64 MiB snapshot budget")
        before = digest(p)
        shutil.copyfile(p, raw / name)
        if digest(p) != before or digest(raw / name) != before:
            raise ValueError("Capture changed during snapshot; wait for a completed run")
    if physical:
        if physical.stat().st_size > 1024 * 1024:
            raise ValueError("Physical check exceeds 1 MiB")
        shutil.copyfile(physical, raw / "population.json")
    write(raw / "context.json", context)
    report = analyse(raw, context, budget_ms)
    report.update(created_utc=datetime.now(timezone.utc).isoformat(), source_capture=str(source),
                  analyzer_sha256=digest(__file__), metrics_analyzer_sha256=digest(Path(__file__).with_name("foundry_perf_metrics.py")),
                  headless_analyzer_sha256=digest(Path(__file__).with_name("foundry_perf_headless.py")),
                  raw_artifacts={"raw/" + p.name: digest(p) for p in sorted(raw.iterdir())})
    write(output / "performance.json", report)
    (output / "performance.md").write_text(markdown(report), encoding="utf-8")
    return report


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--input", required=True, type=Path)
    p.add_argument("--output", required=True, type=Path)
    p.add_argument("--context", required=True, type=Path)
    p.add_argument("--physical-check", type=Path)
    p.add_argument("--budget-ms", type=float, default=1000 / 60)
    args = p.parse_args()
    if not 0 < args.budget_ms < 10000:
        p.error("budget must be finite and between 0 and 10000 ms")
    print(ingest(args.input, args.output, load(args.context), args.budget_ms, args.physical_check)["status"])
