#!/usr/bin/env python3
"""Frame-time benchmark scenes and gate for the windowed game.

    python -I scripts/frame_bench.py run   [scene...]   # one pass, delta against the baseline
    python -I scripts/frame_bench.py check [scene...]   # 3 interleaved passes, exit 1 on a regression
    python -I scripts/frame_bench.py save  [scene...]   # 3 interleaved passes, record the baseline
    options: --exe <path> (default target/release-fast), --passes N

Each scene is a fixed `--benchmark*` command line (seed, generator, size,
weather, duration); the game prints `[benchmark]` summary lines and exits.
Needs a GPU and a desktop session. The machine is shared with other sessions'
builds, so passes are interleaved and each scene keeps its best frame times:
contention only ever slows a pass. Rule (docs/TESTING.md): best p95 or p99 more
than 5 % over the baseline, or median draw calls more than 10 % over, fails.
"""
import csv
import hashlib
import json
import os
import re
import statistics
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASELINES = ROOT / "crates" / "app" / "benches" / "frame_baselines.json"
DEFAULT_EXE = ROOT / "target" / "release-fast" / "minecraft_style_voxel_prototype.exe"

COMMON = ["--benchmark-seed=74", "--benchmark-generator-version=53", "--benchmark-weather=clear",
          "--benchmark-width=1600", "--benchmark-height=900", "--benchmark-warmup=6",
          "--benchmark-duration=15"]
SCENES = {
    # Hills at 1 m, looking across terrain: the everyday view, clouds included.
    "hills-static": ["--benchmark-world=hills", "--benchmark-motion=static"],
    # Same world flown over: streaming, meshing and uploads every frame.
    "hills-orbit": ["--benchmark-world=hills", "--benchmark-motion=orbit"],
    # Clouds are the largest GPU pass; without them terrain and shadows dominate.
    "hills-no-clouds": ["--benchmark-world=hills", "--benchmark-motion=static", "--benchmark-no-clouds"],
    # Flat ground: the floor cost of sky, UI and an almost empty mesh set.
    "superflat-static": ["--benchmark-world=superflat", "--benchmark-motion=static"],
    # Quarter-metre voxels: 64x the cells per metre of the 1 m world.
    "quarter-static": ["--benchmark-world=hills", "--benchmark-scale=quarter", "--benchmark-motion=static"],
    # Globe from 60 km; the long warmup lets the horizon build finish first.
    "orbit-static": ["--benchmark-world=hills", "--benchmark-globe", "--benchmark-motion=static",
                     "--benchmark-terrain-camera=0,0,60000,0,-1.5", "--benchmark-view-chunks=4",
                     "--benchmark-warmup=45"],
    # Straight climb from spawn at 256 m/s toward 40 km: globe tier handover under load.
    "ascent": ["--benchmark-world=hills", "--benchmark-globe", "--benchmark-motion=ascent",
               "--benchmark-duration=120"],
}
GATED_TIMES = ("p95_frame_ms", "p99_frame_ms")
TIME_LIMIT = 0.05
DRAW_LIMIT = 0.10
RUN_TIMEOUT_S = 240
# Peer sessions build this repo and run review captures at any time; a pass during
# either measures the peer. Checked before launch, so the pass's own game is not counted.
BUILD_PROCESSES = ("rustc.exe", "cargo.exe", "rust-lld.exe", "link.exe", "minecraft_style_voxel_prototype.exe")
QUIET_WAIT_S = 900

PAIR_RE = re.compile(r'(\w+)=("[^"]*"|\S+)')


class BenchError(Exception):
    pass


def scene_args():
    return {name: COMMON + args for name, args in SCENES.items()}


def number(text):
    text = text.strip('"')
    try:
        return int(text)
    except ValueError:
        try:
            return float(text)
        except ValueError:
            return text


def parse_report(text):
    """Flatten `[benchmark]` lines: `tag key=v` becomes `tag.key`, untagged keys stay bare."""
    metrics = {}
    for line in text.splitlines():
        if not line.startswith("[benchmark] "):
            continue
        body = line[len("[benchmark] "):]
        first = body.split(" ", 1)[0]
        tag = "" if "=" in first else first + "."
        for key, value in PAIR_RE.findall(body):
            metrics[tag + key] = number(value)
    if "p95_frame_ms" not in metrics:
        raise BenchError("no timing summary (device lost, crash or timeout)")
    metrics["draws"] = int(metrics.get("latest_scene.terrain_draws", 0)) + int(metrics.get("latest_scene.shadow_draws", 0))
    return metrics


def frame_medians(rows):
    """Per-frame medians from frames.csv; shadow cascades redraw on some frames only."""
    rows = list(rows)
    if not rows:
        return {}
    med = lambda key: statistics.median(float(r[key]) for r in rows)
    return {"draws": med("terrain_draws") + med("shadow_draws"),
            "frame.stream_ms": med("stream_ms"), "frame.render_cpu_ms": med("render_cpu_ms"),
            "frame.gpu_pass_ms": med("gpu_pass_ms"), "frame.gpu_shadow_ms": med("gpu_shadow_ms"),
            "frame.terrain_draws": med("terrain_draws"), "frame.shadow_draws": med("shadow_draws"),
            **globe_frames(rows)}


def globe_frames(rows):
    """Globe cost and the measured second after which no tile is wanted; slowest frame's columns."""
    if "globe_ms" not in rows[0]:
        return {}
    out = {"frame.globe_ms": statistics.median(float(r["globe_ms"]) for r in rows),
           "frame.globe_ms_max": max(float(r["globe_ms"]) for r in rows)}
    slow = max(rows, key=lambda r: float(r["total_ms"]))
    for key in ("total_ms", "gpu_pass_ms", "globe_ms", "globe_upload_bytes", "stream_ms", "acquire_ms"):
        out[f"slowest.{key}"] = float(slow[key])
    if "globe_wanted" in rows[0]:
        elapsed, settled = 0.0, 0.0
        for r in rows:
            elapsed += float(r["total_ms"]) / 1000
            if float(r["globe_wanted"]) > 0:
                settled = elapsed
        out["frame.globe_wanted_settled_s"] = settled
        out["frame.globe_wanted_last"] = float(rows[-1]["globe_wanted"])
    return out


def build_activity():
    try:
        out = subprocess.run(["tasklist", "/fo", "csv", "/nh"], capture_output=True, text=True).stdout.lower()
    except OSError:
        return 0
    return sum(out.count(f'"{name}"') for name in BUILD_PROCESSES)


def wait_for_quiet():
    """Block until no build runs; True when one was still running at the deadline."""
    deadline = time.monotonic() + QUIET_WAIT_S
    while build_activity():
        if time.monotonic() > deadline:
            return True
        time.sleep(15)
    return False


@dataclass
class Verdict:
    current: dict
    failures: list = field(default_factory=list)
    unverified: list = field(default_factory=list)


def best_pass(runs):
    """The pass with the lowest p95: background load only ever slows a pass."""
    return min(runs, key=lambda r: r["p95_frame_ms"])


def compare(runs, baseline):
    adapters = {r["adapter"] for r in runs}
    if adapters != {baseline["adapter"]}:
        raise BenchError(f"baseline adapter {baseline['adapter']!r}, runs on {sorted(adapters)}")
    best = best_pass(runs)
    current = {k: best[k] for k in GATED_TIMES}
    current["draws"] = statistics.median(r["draws"] for r in runs)
    failures, timing = [], []
    for key in GATED_TIMES:
        if current[key] > baseline[key] * (1 + TIME_LIMIT):
            timing.append(f"{key} {current[key]:.2f} vs {baseline[key]:.2f} (+{100 * (current[key] / baseline[key] - 1):.1f} %)")
    if all(r.get("contended") for r in runs):
        # Every pass overlapped a peer's build or game: slow frames prove nothing either way.
        verdict = Verdict(current, failures)
        verdict.unverified = timing
    else:
        failures += timing
        verdict = Verdict(current, failures)
    if baseline["draws"] and current["draws"] > baseline["draws"] * (1 + DRAW_LIMIT):
        verdict.failures.append(f"draws {current['draws']:.0f} vs {baseline['draws']}")
    return verdict


def run_scene(exe, name):
    contended = wait_for_quiet()
    with tempfile.TemporaryDirectory(prefix="frame-bench-") as scratch:
        env = dict(os.environ, VOXEL_SAVE_DIR=scratch, VOXEL_CAPTURE_DIR=scratch)
        try:
            done = subprocess.run([str(exe), *scene_args()[name]], cwd=ROOT, env=env,
                                  capture_output=True, text=True, errors="replace", timeout=RUN_TIMEOUT_S)
        except subprocess.TimeoutExpired:
            raise BenchError(f"{name}: no exit within {RUN_TIMEOUT_S} s")
        except OSError as error:
            raise BenchError(f"{name}: cannot start {exe}: {error}")
        metrics = parse_report(done.stdout)
        frames = Path(scratch) / "frames.csv"
        if frames.exists():
            with frames.open(newline="", encoding="utf-8") as f:
                metrics.update(frame_medians(csv.DictReader(f)))
    metrics["contended"] = contended or build_activity() > 0
    return metrics


def passes(exe, names, count):
    """Round-robin passes; a failed run is reported and retried once."""
    results = {name: [] for name in names}
    for index in range(count):
        for name in names:
            for attempt in (1, 2):
                try:
                    m = run_scene(exe, name)
                    results[name].append(m)
                    print(f"  pass {index + 1} {name}: p95 {m['p95_frame_ms']:.2f} ms, p99 {m['p99_frame_ms']:.2f} ms, "
                          f"max {m.get('max_frame_ms', 0):.1f} ms, {m['avg_fps']:.1f} fps, draws {m['draws']:.0f}"
                          + (f", globe {m['frame.globe_ms']:.2f} ms, wanted settled {m.get('frame.globe_wanted_settled_s', -1):.1f} s"
                             f", slowest gpu {m['slowest.gpu_pass_ms']:.1f} globe {m['slowest.globe_ms']:.1f}"
                             f" stream {m['slowest.stream_ms']:.1f} acquire {m['slowest.acquire_ms']:.1f} ms"
                             if "frame.globe_ms" in m else "") + (" (builds running)" if m["contended"] else ""))
                    break
                except BenchError as error:
                    print(f"  pass {index + 1} {name} attempt {attempt}: {error}")
    return results


def load_baselines():
    return json.loads(BASELINES.read_text(encoding="utf-8")) if BASELINES.exists() else {}


def provenance(exe):
    rev = subprocess.run(["git", "-C", str(ROOT), "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip()
    dirty = bool(subprocess.run(["git", "-C", str(ROOT), "status", "--porcelain", "--untracked-files=no"],
                                capture_output=True, text=True).stdout.strip())
    return {"rev": rev, "dirty_tree": dirty, "exe_sha256": hashlib.sha256(exe.read_bytes()).hexdigest()[:16],
            "date": time.strftime("%Y-%m-%d")}


def summarize(runs, meta):
    """Every timing from the best pass, so the record is one consistent frame distribution."""
    best = best_pass(runs)
    keep = ("p50_frame_ms", "p95_frame_ms", "p99_frame_ms", "avg_fps", "gpu_avg_ms.passes",
            "gpu_avg_ms.terrain", "gpu_avg_ms.shadow", "gpu_avg_ms.raymarch", "cpu_avg_ms.gameplay",
            "cpu_avg_ms.stream", "cpu_avg_ms.render_call", "latest_scene.terrain_tris",
            "frame.stream_ms", "frame.render_cpu_ms", "frame.gpu_pass_ms", "frame.gpu_shadow_ms",
            "frame.terrain_draws", "frame.shadow_draws", "max_frame_ms", "frame.globe_ms",
            "frame.globe_ms_max", "frame.globe_wanted_settled_s", "frame.globe_wanted_last", "globe.wanted")
    return {"adapter": best["adapter"], "res": best.get("res"), **{k: best[k] for k in keep if k in best},
            "draws": statistics.median(r["draws"] for r in runs), "passes": len(runs),
            "contended_passes": sum(bool(r.get("contended")) for r in runs), **meta}


def main(argv):
    args = argv[1:]
    exe, count = DEFAULT_EXE, None
    if "--exe" in args:
        i = args.index("--exe")
        exe = Path(args[i + 1])
        del args[i:i + 2]
    if "--passes" in args:
        i = args.index("--passes")
        count = int(args[i + 1])
        del args[i:i + 2]
    action = args[0] if args else "run"
    names = args[1:] or list(SCENES)
    unknown = [n for n in names if n not in SCENES]
    if action not in ("run", "check", "save") or unknown:
        print(__doc__ + f"\nscenes: {', '.join(SCENES)}")
        return 2
    if not exe.exists():
        print(f"missing {exe}; build it with `cargo build --profile release-fast -p minecraft_style_voxel_prototype`")
        return 2
    count = count or (1 if action == "run" else 3)
    results = passes(exe, names, count)
    baselines = load_baselines()
    status = 0
    if action == "save":
        meta = provenance(exe)
        for name, runs in results.items():
            if runs:
                baselines[name] = summarize(runs, meta)
            else:
                print(f"{name}: no successful pass, baseline unchanged")
                status = 1
        BASELINES.parent.mkdir(parents=True, exist_ok=True)
        BASELINES.write_text(json.dumps(baselines, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(f"wrote {BASELINES.relative_to(ROOT)}")
        return status
    for name, runs in results.items():
        if not runs:
            print(f"FAIL {name}: no successful pass")
            status = 1
            continue
        if name not in baselines:
            print(f"FAIL {name}: no baseline (record one with `just frame-bench-save {name}`)")
            status = 1
            continue
        try:
            verdict = compare(runs, baselines[name])
        except BenchError as error:
            print(f"FAIL {name}: {error}")
            status = 1
            continue
        base = baselines[name]
        line = (f"{name}: p95 {verdict.current['p95_frame_ms']:.2f} ms (base {base['p95_frame_ms']:.2f}), "
                f"p99 {verdict.current['p99_frame_ms']:.2f} (base {base['p99_frame_ms']:.2f}), "
                f"draws {verdict.current['draws']:.0f} (base {base['draws']:.0f})")
        if verdict.failures and action == "check":
            print("FAIL " + line + "; " + "; ".join(verdict.failures))
            status = 1
        elif verdict.unverified and action == "check":
            print("UNVERIFIED (every pass contended) " + line + "; " + "; ".join(verdict.unverified))
            status = max(status, 3)
        else:
            print(("ok   " if not (verdict.failures or verdict.unverified) else "slow ") + line)
    return status


if __name__ == "__main__":
    sys.exit(main(sys.argv))
