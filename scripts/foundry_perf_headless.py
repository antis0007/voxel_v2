"""Headless receipts; durations are wall spans, never frame or CPU metrics."""
import json
import math
import re
import statistics
from pathlib import Path

DIAGNOSTIC_KINDS = ("slope-build", "watershed-build", "cold-builder", "chunk-build", "history-stages",
                    "river-attempt", "river-attempt-summary", "meander-attempt", "relief-cache-result")
KINDS = {"headless-chunks", "headless-worldgen"}
HASH = re.compile(r"[0-9a-f]{64}\Z")
HEX = re.compile(r"[0-9a-f]{16}\Z")


def records(text, tag):
    rows = []
    for line in text.splitlines():
        if tag in line:
            body = line.split(tag, 1)[1].strip()
            pairs = re.findall(r"(\w+)=([^\s]+)", body)
            row = dict(pairs)
            if len(row) != len(pairs):
                raise ValueError("Duplicate receipt fields: " + tag)
            rows.append(row)
    return rows


def one(text, tag):
    rows = records(text, tag)
    if len(rows) != 1:
        raise ValueError("Expected exactly one " + tag)
    return rows[0]


def number(row, name, integer=False):
    if name not in row:
        raise ValueError("Missing numeric receipt field: " + name)
    value = int(row[name]) if integer else float(row[name])
    if not math.isfinite(value) or value < 0:
        raise ValueError("Invalid nonnegative receipt field: " + name)
    return value



REGIONAL_FIELDS = ('attempts', 'hits', 'misses', 'invalid', 'stored', 'write_skipped',
                   'bytes_read', 'bytes_written', 'bypassed', 'read_errors', 'write_errors')


def regional_cache(text):
    lines = [line.split('[relief-cache-result]', 1)[1].strip()
             for line in text.splitlines() if '[relief-cache-result]' in line]
    if not lines:
        return None
    if len(lines) != 1:
        raise ValueError('Repeated regional cache receipt')
    match = re.fullmatch(r'ReliefCacheStats\s*\{\s*(.*?)\s*\}', lines[0])
    if not match:
        raise ValueError('Malformed regional cache receipt')
    counts = {}
    for item in match[1].split(','):
        pair = re.fullmatch(r'\s*([a-z_]+):\s*([0-9]+)\s*', item)
        if not pair or pair[1] in counts or pair[1] not in REGIONAL_FIELDS:
            raise ValueError('Malformed, duplicate or unknown regional counter')
        counts[pair[1]] = int(pair[2])
    if set(counts) != set(REGIONAL_FIELDS) or any(v > 2**64-1 for v in counts.values()):
        raise ValueError('Missing or out-of-range regional counters')
    c = counts
    if (c['attempts'] != c['hits'] + c['misses']
            or c['misses'] != c['stored'] + c['write_skipped']
            or c['invalid'] + c['read_errors'] > c['misses']
            or c['write_errors'] > c['write_skipped']
            or (c['hits'] > 0 and c['bytes_read'] == 0)
            or (c['bytes_read'] > 0 and c['hits'] + c['invalid'] == 0)
            or bool(c['bytes_written']) != bool(c['stored'])
            or c['bytes_read'] > (c['hits'] + c['invalid']) * 512 * 1024
            or c['bytes_written'] > c['stored'] * 400 * 1024):
        raise ValueError('Inconsistent completed regional counters')
    observation = ('mixed' if c['hits'] and c['misses'] else 'hit-only' if c['hits']
                   else 'miss-only' if c['misses'] else 'bypass-only' if c['bypassed'] else 'inactive')
    return {'kind': 'relief-cache-result', 'counts': counts, 'observation': observation,
            'scope': 'Completed process-cumulative counters; not durations or per-chunk attribution',
            'limitations': 'Bypass-only does not prove configuration was disabled; mixed may include same-run hits.'}


def diagnostics(text):
    result = []
    count_names = ('nodes', 'steps', 'passes', 'epochs', 'previews', 'parcels', 'input_total_nodes',
                   'fixed_nodes', 'sample_stride', 'sample_phase', 'sampled_nodes', 'sampled_fixed',
                   'rank', 'ranked', 'considered_max', 'attempted', 'overlap_skipped', 'inlets')
    for line in text.splitlines():
        for tag in DIAGNOSTIC_KINDS:
            if tag == 'relief-cache-result':
                continue
            marker = '[' + tag + ']'
            if marker not in line:
                continue
            row = one(line, marker)
            durations = {k: number(row, k) * (1000 if k.endswith('_s') else 1)
                         for k in row if k.endswith(('_ms', '_s')) and not k.startswith('sampled_')}
            samples = {k: number(row, k) for k in row if k.startswith('sampled_') and k.endswith('_ms')}
            counts = {k: number(row, k, True) for k in row if k in count_names}
            derived = {k: number(row, k, True) for k in ('terrain_calls', 'rock_calls', 'cell_steps') if k in row}
            if samples:
                required = ('input_total_nodes', 'fixed_nodes', 'sample_stride', 'sample_phase', 'sampled_nodes', 'sampled_fixed')
                if any(k not in counts for k in required) or not 0 < counts['sample_stride'] or not 0 <= counts['sample_phase'] < counts['sample_stride']:
                    raise ValueError('Missing or invalid sampling counts')
                if counts['sampled_nodes'] > counts['input_total_nodes'] or counts['fixed_nodes'] > counts['input_total_nodes'] or counts['sampled_fixed'] > min(counts['sampled_nodes'], counts['fixed_nodes']):
                    raise ValueError('Inconsistent sampling counts')
            if tag == 'river-attempt' and (row.get('outcome') not in ('initial-reject', 'late-reject', 'accepted') or 'init_ms' not in durations or (row['outcome'] != 'initial-reject' and 'hydro_ms' not in durations)):
                raise ValueError('Incomplete river attempt')
            if tag == 'river-attempt-summary':
                counts['accepted'] = number(row, 'accepted', True)
                if 'elapsed_ms' not in durations or any(k not in counts for k in ('ranked', 'attempted', 'overlap_skipped', 'considered_max')) or counts['accepted'] > counts['attempted']:
                    raise ValueError('Incomplete river summary')
            if tag == 'meander-attempt' and (row.get('accepted') not in ('true', 'false') or 'rank' not in counts or 'elapsed_ms' not in durations):
                raise ValueError('Incomplete meander attempt')
            result.append({'kind': tag, 'durations_ms': durations, 'counts': counts, 'raw': line,
                'sampled_durations_ms': samples, 'sample_scope': 'Only sampled nodes; no extrapolation or whole-stage attribution.',
                'derived_counts': derived, 'derived_count_scope': 'Formula-derived from grid size/loop bounds, not observed counters.',
                'scope': ('Attempts may overlap; never sum into network wall. Hydro includes setup and excludes final tile packing.'
                          if tag.startswith(('river-', 'meander-')) else 'Inclusive builder or nested child; no cross-builder sum'),
                'parent_identity': 'not recorded; temporal adjacency does not establish parentage'})
    return result


def analyse(raw, context, budget_ms):
    try:
        return _analyse(raw, context, budget_ms)
    except (KeyError, TypeError, OverflowError) as exc:
        raise ValueError("Missing or malformed headless receipt") from exc


def _analyse(raw, context, budget_ms):
    from foundry import canonical
    manifest = json.loads((raw / "manifest.json").read_text(encoding="utf-8-sig"))
    text = "\n".join((raw / p).read_text(encoding="utf-8-sig", errors="replace")
                     for p in ("stdout.log", "stderr.log") if (raw / p).exists())
    kind = context["workload_kind"]
    if kind not in KINDS:
        raise ValueError("Unsupported headless workload")
    if manifest.get("exit", manifest.get("exit_code")) != 0:
        raise ValueError("Completed successful process manifest required")
    if "test result: FAILED" in text or "panicked at" in text:
        raise ValueError("Failed headless workload")
    if len(re.findall(r"test result: ok\. [1-9]\d* passed; 0 failed;", text)) != 1:
        raise ValueError("Completed Rust test harness required")
    build = context.get("build", {})
    for name, value in (("executable", manifest.get("sha256", "")),
                        ("configuration", build.get("configuration_hash", "")),
                        ("source", build.get("source_snapshot", ""))):
        if not HASH.fullmatch(value):
            raise ValueError("Pinned SHA256 required for " + name)
    config = context.get("world_config")
    if not isinstance(config, dict) or manifest.get("world_config") != config:
        raise ValueError("Matching manifest/context world_config required")
    if not HASH.fullmatch(config.get("full_config_sha256", "")):
        raise ValueError("Complete serialized world configuration hash required")
    if not context.get("hardware") or not context.get("workload_id"):
        raise ValueError("Hardware and workload identity required")
    spherical = one(text, "[worldgen] spherical")
    if number(spherical, "seed", True) != config.get("seed") or not HEX.fullmatch(spherical.get("checksum", "")):
        raise ValueError("Spherical seed/checksum mismatch")
    solve = number(spherical, "solve_ms")
    if solve <= 0 or config.get("world_type") != "Planet" or not isinstance(config.get("generator"), int):
        raise ValueError("Positive solve and explicit planet configuration required")
    cache = records(text, "[geology-cache] hit")
    mode = "hit" if cache else "miss-or-disabled"
    if len(cache) > 1 or mode != context.get("geology_cache_state"):
        raise ValueError("Cache state mismatch or repeated hit")
    if cache and (number(cache[0], "seed", True) != config.get("seed") or
                  number(cache[0], "generator", True) != config.get("generator")):
        raise ValueError("Cache identity mismatch")
    spans = [{"name": "spherical.solve", "duration_ms": solve, "parent": "world.setup",
              "source": "[worldgen] spherical solve_ms", "scope": "inclusive wall"}]
    for key in ("drift_ms", "flank_ms", "riverbed_ms", "erosion_ms", "routing_ms"):
        spans.append({"name": "spherical." + key[:-3], "duration_ms": number(spherical, key),
                      "parent": "spherical.solve", "source": "[worldgen] spherical " + key,
                      "scope": "nested wall; not additive to parent"})
    metrics = {"solve_ms": solve}
    identity = {"kind": kind, "id": context["workload_id"], "config": config,
                "cache_state": mode, "environment": context.get("workload_environment", {})}
    output = {"world_checksum": spherical["checksum"]}
    if kind == "headless-chunks":
        result = one(text, "[planet-chunk-result]")
        setup = one(text, "[planet-chunk-setup]")
        for field, key in (("version", "generator"), ("seed", "seed")):
            if number(result, field, True) != config.get(key):
                raise ValueError("Chunk config mismatch: " + field)
        if result.get("scale") != config.get("voxel_scale") or setup.get("scale") != result["scale"]:
            raise ValueError("Voxel scale mismatch")
        count = number(result, "count", True)
        blocks = number(result, "blocks", True)
        solid = number(result, "solid_count", True)
        if not 1 <= count <= 1024 or blocks != count * 34 ** 3 or solid > blocks:
            raise ValueError("Chunk count/block coverage invalid")
        rows = re.findall(r"\[planet-chunk\] index=(\d+) coord=IVec3\((-?\d+), (-?\d+), (-?\d+)\) generate_ms=(\S+)", text)
        if len(rows) != count or text.count("[planet-chunk]") != count:
            raise ValueError("Missing or malformed per-chunk receipt")
        coords, times = [], []
        for expected, (index, x, y, z, duration) in enumerate(rows):
            if int(index) != expected:
                raise ValueError("Chunk ordering mismatch")
            if any(not -(2 ** 31) <= int(v) < 2 ** 31 for v in (x, y, z)):
                raise ValueError("Coordinate outside i32")
            coords.append([int(x), int(y), int(z)])
            times.append(number({"ms": duration}, "ms"))
        if coords != context.get("chunk_coordinates"):
            raise ValueError("Declared chunk coordinates mismatch")
        generated = number(result, "generate_ms")
        wall = number(result, "wall_ms")
        setup_ms = number(setup, "world_ms")
        if min(generated, wall, setup_ms, *times) <= 0:
            raise ValueError("Positive completed durations required")
        if abs(sum(times) - generated) > .002 * count or generated > wall + .002 or solve > setup_ms + .002:
            raise ValueError("Inconsistent duration receipts")
        if not HEX.fullmatch(result.get("checksum", "")):
            raise ValueError("Missing chunk checksum")
        identity["chunk_coordinates"] = coords
        output.update(checksum=result["checksum"], blocks=blocks, solid_count=solid)
        metrics.update(generate_ms=generated, chunk_wall_ms=wall, setup_ms=setup_ms)
        spans += [{"name": "world.setup", "duration_ms": setup_ms, "parent": None,
                   "source": "[planet-chunk-setup] world_ms", "scope": "inclusive wall; outside chunk timer"},
                  {"name": "chunks.wall", "duration_ms": wall, "parent": None,
                   "source": "[planet-chunk-result] wall_ms", "scope": "includes hashing and diagnostics"},
                  {"name": "chunks.generate", "duration_ms": generated, "parent": "chunks.wall",
                   "source": "[planet-chunk-result] generate_ms", "scope": "sum of serial generate_padded calls"}]
    else:
        spans[0]["parent"] = None
    if output != context.get("expected_output"):
        raise ValueError("Expected output checksum/count mismatch")
    # Preserve coarse diagnostics without summing nested or concurrent builds.
    coarse_diagnostics = diagnostics(text)
    regional = regional_cache(text)
    if regional is not None:
        coarse_diagnostics.append(regional)
    identity["regional_cache"] = {"observation": regional["observation"] if regional else "unobserved",
                                  "has_bypasses": bool(regional["counts"]["bypassed"]) if regional else None}
    instrumentation = context.get("instrumentation")
    if not isinstance(instrumentation, dict):
        raise ValueError("Explicit instrumentation profile required")
    profile = instrumentation.get("profile")
    declared = instrumentation.get("diagnostic_kinds")
    known = set(DIAGNOSTIC_KINDS)
    if (not isinstance(profile, str) or not profile.strip() or not isinstance(declared, list)
            or any(not isinstance(k, str) or k not in known for k in declared)
            or len(set(declared)) != len(declared)):
        raise ValueError("Nonempty profile and unique known diagnostic kinds required")
    observed = sorted({d["kind"] for d in coarse_diagnostics})
    if not set(observed).issubset(declared):
        raise ValueError("Observed diagnostics exceed declared instrumentation")
    identity["instrumentation"] = {"profile": profile, "declared_kinds": sorted(declared),
                                    "observed_kinds": observed,
                                    "sample_strides": sorted({d["counts"]["sample_stride"] for d in coarse_diagnostics if "sample_stride" in d["counts"]})}
    issues = ["Headless wall spans do not measure FPS, CPU utilization, GPU work, visible quality or multiplayer.",
              "World configuration hash is supplied by the runner; log fields cross-check only seed, generator when present, and scale.",
              "Checksums establish recorded output equality, not independent correctness or save compatibility."]
    if kind == "headless-worldgen" and "[planet-chunk-result]" in text:
        raise ValueError("Chunk workload must be classified as headless-chunks")
    if kind == "headless-worldgen" and not cache:
        issues.append("Generator version is manifest-pinned but not printed in the spherical receipt.")
    findings = [{"kind": "observed-span", "metric": k, "duration_ms": v,
                 "next_probe": "Compare independent runs with identical configuration, cache state and output; inspect nested spans without adding parents."}
                for k, v in metrics.items()]
    return {"schema": "axiom.foundry.performance.v1", "status": "reported-unverified",
            "frame_budget_ms": budget_ms, "workload": identity, "binary_sha256": manifest["sha256"],
            "build": build, "hardware": context["hardware"], "hardware_fingerprint": canonical(context["hardware"]),
            "telemetry": None, "native_summary": {}, "headless": {"metrics_ms": metrics, "spans": spans,
            "output": output, "diagnostics": coarse_diagnostics, "aggregation": "Nested diagnostics are not summed."},
            "reliability_issues": issues, "findings": findings, "merge_authorized": False,
            "agent_packet": {"observations": findings, "unmeasured_or_unreliable": issues,
                             "required_invariants": ["exact generated output", "save compatibility", "bounded memory"],
                             "instruction": "Use measured wall spans; never infer FPS or sum nested spans."}}


def markdown(report):
    h = report["headless"]
    lines = ["# Foundry headless workload", "", "Status: reported, independently unverified.", "",
             "Workload: " + report["workload"]["kind"], "Binary: " + report["binary_sha256"], "",
             "| Span | Milliseconds | Parent |", "| --- | ---: | --- |"]
    lines += [f"| {s['name']} | {s['duration_ms']:.3f} | {s['parent'] or 'none'} |" for s in h["spans"]]
    lines += ["", "Nested spans must not be added to parents.", "", "## Evidence limits"]
    lines += ["- " + x for x in report["reliability_issues"]]
    return "\n".join(lines) + "\n"


def compare(baseline, candidate, tolerance):
    from foundry import canonical
    from foundry_compare import interval
    all_runs = baseline + candidate
    reasons = []
    heads = [r.get("headless") if isinstance(r.get("headless"), dict) else {} for r in all_runs]
    metrics = [h.get("metrics_ms") for h in heads]
    valid_metrics = all(isinstance(m, dict) and m and all(
        isinstance(k, str) and isinstance(v, (int, float)) and not isinstance(v, bool)
        and math.isfinite(v) and v > 0 for k, v in m.items()) for m in metrics)
    if not valid_metrics or len({tuple(sorted(m)) for m in metrics if isinstance(m, dict)}) != 1:
        reasons.append("Missing, invalid or different metric keys/values")
    for key in ("workload", "hardware_fingerprint"):
        if len({canonical(r.get(key)) for r in all_runs}) != 1:
            reasons.append("Different " + key)
    if not baseline or not candidate or any("headless" not in r for r in all_runs):
        reasons.append("Both variants require headless receipts")
    if len({canonical(h.get("output")) for h in heads}) != 1:
        reasons.append("Different output checksums/counts")
    if len({r.get("build", {}).get("configuration_hash") for r in all_runs}) != 1:
        reasons.append("Different build configurations")
    if len({r.get("source_capture") for r in all_runs}) != len(all_runs):
        reasons.append("Duplicate source captures")
    hashes = [canonical({k: v for k, v in r.get("raw_artifacts", {}).items() if k.endswith(".log")}) for r in all_runs]
    if len(set(hashes)) != len(hashes):
        reasons.append("Duplicate raw log evidence")
    for group in (baseline, candidate):
        if len({r.get("binary_sha256") for r in group}) != 1:
            reasons.append("Each variant requires one executable identity")
    result = {"schema": "axiom.foundry.performance-comparison.v1", "merge_authorized": False,
              "status": "incomparable" if reasons else "diagnostic-only", "reasons": reasons,
              "metrics": {}, "baseline_runs": len(baseline), "candidate_runs": len(candidate)}
    if reasons:
        return result
    for key in baseline[0]["headless"]["metrics_ms"]:
        a = [r["headless"]["metrics_ms"][key] for r in baseline]
        b = [r["headless"]["metrics_ms"][key] for r in candidate]
        if statistics.mean(a) <= 0:
            continue
        result["metrics"][key] = {"baseline_passes_ms": a, "candidate_passes_ms": b,
            "mean_relative_change": statistics.mean(b) / statistics.mean(a) - 1,
            "pass_bootstrap_95_interval": interval(a, b) if min(len(a), len(b)) >= 3 else None}
    result["reasons"] = ["Diagnostic wall timings only; no merge or correctness authorization."]
    return result
