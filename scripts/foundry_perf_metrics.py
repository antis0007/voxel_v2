"""Read native telemetry without treating nested CPU/GPU spans as additive."""
import csv
import math
import re
import statistics
from collections import defaultdict

CPU = {"gameplay_ms": "Gameplay", "stream_ms": "Streaming", "render_cpu_ms": "CPU render call",
       "horizon_ms": "Horizon", "acquire_ms": "Acquire (nested)",
       "submit_ms": "Submit (nested)", "present_ms": "Present (nested)"}
GPU = {"gpu_pass_ms": "Measured GPU pass subtotal", "terrain_ms": "Terrain (nested)",
       "gpu_shadow_ms": "Shadows (nested)", "cloud_ms": "Clouds (nested)",
       "cloud_composite_ms": "Cloud composite (nested)",
       "gpu_sky_probe_ms": "Sky probe compute (outside legacy subtotal)"}
COUNTERS = ("worker_pending", "chunk_upload_bytes", "cpu_resident_bytes", "gpu_resident_bytes",
            "terrain_draws", "terrain_triangles", "visible_missing", "near_missing")


GPU_TIMING_SCOPE = {
    "kind": "partial_pass_sum",
    "complete_frame_cost": False,
    "instrumented_passes_in_reviewed_source": ["shadow cascades", "terrain", "indirect", "cloud raymarch",
                        "cloud temporal", "sky", "cloud composite", "outline", "postprocess", "UI"],
    "known_unmeasured_work": ["atmosphere transport/sky-view/aerial LUT compute",
                             "sky-volume bake", "sky probe", "portal auxiliary passes"],
    "basis": "Source audit 2026-11-10; not verified against the captured binary or proof that every pass ran.",
    "inventory_verified_against_capture": False,
    "owners": ["crates/app/src/render.rs", "crates/app/src/render/atmosphere.rs",
               "crates/app/src/render/sky_volume_gpu.rs", "crates/app/src/render/sky_probe.rs",
               "crates/app/src/render/portals"],
    "interpretation": "Sample coverage is not workload coverage. Missing work has unknown cost, not zero. "
                      "The subtotal excludes queue/presentation waits and is not whole-frame GPU time.",
}


def timing_scope(columns):
    scope = dict(GPU_TIMING_SCOPE)
    if "gpu_sky_probe_ms" in columns:
        scope["known_unmeasured_work"] = [p for p in scope["known_unmeasured_work"] if p != "sky probe"]
        scope["additional_instrumented_passes"] = ["sky probe compute"]
        scope["legacy_subtotal_excludes"] = ["gpu_sky_probe_ms"]
        scope["interpretation"] += " Sky-probe compute is reported separately; its buffer copy remains unmeasured."
    return scope


def shader_validation(stdout, stderr):
    """Read reported world-program identities, never infer identity from checkout files."""
    records = []
    rejected = 0
    for stream, text in (("stdout", stdout), ("stderr", stderr)):
        for line_number, line in enumerate(text.splitlines(), 1):
            if not line.startswith("[shader-validation]"):
                continue
            match = re.fullmatch(r"\[shader-validation\] world_fnv1a64=([0-9a-f]{16}) "
                                 r"build_match=(true|false) hardware_weather_filtering=(true|false)", line.strip())
            if not match:
                rejected += 1
                continue
            records.append({"program": "world", "fnv1a64": match[1],
                            "build_match": match[2] == "true",
                            "hardware_weather_filtering": match[3] == "true",
                            "stream": stream, "line": line_number})
    return {"status": "reported" if records else "unavailable", "records": records[:16],
            "omitted_records": max(0, len(records) - 16), "rejected_records": rejected,
            "scope": "Process log receipts for the world shader only; not all shaders, visual quality, "
                     "or proof of correspondence to a particular CSV frame. FNV is diagnostic, not cryptographic."}


def stats(values):
    values = sorted(values)
    if not values:
        return None
    q = lambda p: values[max(0, math.ceil(p * len(values)) - 1)]
    return {"samples": len(values), "mean": statistics.mean(values), "p50": q(.50),
            "p95": q(.95), "p99": q(.99), "max": values[-1]}


def numeric(row, key):
    try:
        value = float(row[key])
        return value if math.isfinite(value) and value >= 0 else None
    except (ValueError, KeyError, TypeError):
        return None


def read_frames(path, budget_ms):
    with path.open(encoding="utf-8-sig", newline="") as stream:
        reader = csv.DictReader(stream)
        columns = reader.fieldnames or []
        rows = list(reader)
    cpu, gpu, counters = defaultdict(list), defaultdict(list), defaultdict(list)
    totals, residual, invalid, seen_gpu, lags = [], [], [], set(), []
    slowest = []
    gpu_rejected = defaultdict(int)
    visible_key = next((k for k in ("visible_residents", "visible_agents", "actors_visible") if k in columns), None)
    visible = []
    for index, row in enumerate(rows):
        total = numeric(row, "total_ms")
        if total is None:
            invalid.append(index)
        else:
            totals.append(total)
            slowest.append({"csv_line": index + 2, "frame": row.get("frame"), "total_ms": total,
                            "cpu_ms": {k: numeric(row, k) for k in CPU},
                            "gpu_sample": {k: row.get(k) for k in ("gpu_frame_id", "render_frame_id", "gpu_config_id", "render_config_id")},
                            "counters": {k: numeric(row, k) for k in COUNTERS}})
            slowest.sort(key=lambda frame: frame["total_ms"], reverse=True)
            del slowest[5:]
        for key in CPU:
            value = numeric(row, key)
            if value is not None:
                cpu[key].append(value)
        for key in COUNTERS:
            value = numeric(row, key)
            if value is not None:
                counters[key].append(value)
        if visible_key and numeric(row, visible_key) is not None:
            visible.append(numeric(row, visible_key))
        disjoint = [numeric(row, k) for k in ("gameplay_ms", "stream_ms", "render_cpu_ms", "horizon_ms")]
        if total is not None and all(v is not None for v in disjoint):
            residual.append(max(0, total - sum(disjoint)))
        if str(row.get("gpu_supported", "")).lower() != "true":
            gpu_rejected["unsupported_or_missing"] += 1
            continue
        if numeric(row, "gpu_sample_count") is None or numeric(row, "gpu_sample_count") <= 0:
            gpu_rejected["missing_sample"] += 1
            continue
        if not row.get("gpu_config_id") or row.get("gpu_config_id") != row.get("render_config_id"):
            gpu_rejected["configuration_mismatch"] += 1
            continue
        try:
            key = (int(row["gpu_frame_id"]), row["gpu_config_id"])
            lag = int(row["render_frame_id"]) - key[0]
        except (ValueError, KeyError):
            gpu_rejected["identity_missing"] += 1
            continue
        if lag < 0:
            gpu_rejected["future_sample"] += 1
            continue
        if key in seen_gpu:
            gpu_rejected["duplicate_sample"] += 1
            continue
        seen_gpu.add(key)
        lags.append(lag)
        for field in GPU:
            value = numeric(row, field)
            if value is not None:
                gpu[field].append(value)
    return {"rows": len(rows), "invalid_frame_rows": invalid, "columns": columns,
            "frame_ms": stats(totals), "over_budget_frames": sum(v > budget_ms for v in totals),
            "frame_interval_minus_selected_cpu_spans_ms": stats(residual),
            "slowest_frames": slowest, "cpu_ms": {k: stats(v) for k, v in cpu.items()},
            "gpu_ms": {k: stats(v) for k, v in gpu.items()},
            "gpu_coverage": {"timing_scope": timing_scope(columns), "unique_samples": len(seen_gpu), "excluded_rows": dict(gpu_rejected),
                             "lag_in_render_frames": stats(lags)},
            "counters": {k: {"min": min(v), "max": max(v)} for k, v in counters.items()},
            "visible_actors": {"column": visible_key, "min": min(visible), "max": max(visible)} if visible else None}


def auxiliary(stdout, stderr):
    startup = []
    for line in stdout.splitlines():
        if line.startswith("[startup-timing]"):
            match = re.search(r"ms=([\d.]+)", line)
            if match:
                startup.append({"label": line.split(" ms=")[0], "ms": float(match[1])})
    marks, intervals, stalls = [], defaultdict(list), defaultdict(list)
    for line in stderr.splitlines():
        match = re.match(r"\[frame-stage\] t=([\d.]+) ([\w-]+)$", line)
        if match:
            marks.append((float(match[1]), match[2]))
        match = re.search(r"\[gameplay-stall\] stage=([\w-]+) ms=([\d.]+)", line)
        if match:
            stalls[match[1]].append(float(match[2]))
    for (a, label), (b, next_label) in zip(marks, marks[1:]):
        if b >= a:
            intervals[label + " -> " + next_label].append((b - a) * 1000)
    return {"shader_validation": shader_validation(stdout, stderr), "startup_spans": startup,
            "full_process_trace_intervals_ms": sorted(
                ({"interval": k, **stats(v)} for k, v in intervals.items()),
                key=lambda v: v["max"], reverse=True)[:12],
            "thresholded_gameplay_stalls_ms": {k: stats(v) for k, v in stalls.items()},
            "trace_scope": "Whole process, including startup/warmup; not aligned to measured CSV frames.",
            "stall_scope": "Only logged threshold exceedances; absent events do not mean zero cost."}


def read_route(path):
    with path.open(encoding="utf-8-sig", newline="") as stream:
        rows = list(csv.DictReader(stream))
    measured = [row for row in rows if row.get("measuring_at_render") == "true"]
    keys = ("total_ms", "redraw_ms", "outside_redraw_ms", "gate_wait_ms", "gate_retry_redraw_ms",
            "resident_upload_wall_ms", "redraw_dispatch_ms", "gpu_retry_late_ms", "ui_ms", "weather_ms")
    groups = {}
    for label, group in (("all_attempts", rows), ("measuring_attempts", measured)):
        groups[label] = {key: stats([v for row in group if (v := numeric(row, key)) is not None]) for key in keys}
    return {"rows": len(rows), "measuring_rows": len(measured), "groups_ms": groups,
            "failed_render_rows": sum(row.get("render_ok") == "false" for row in rows),
            "scope": "Route timer spans, with all attempts and measuring attempts separate; spans may be nested."}
