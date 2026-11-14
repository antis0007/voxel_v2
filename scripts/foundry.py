#!/usr/bin/env python3
"""Axiom Foundry's evidence-preserving native baseline runner."""
import argparse
import hashlib
import json
import os
import platform
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import frame_bench

ROOT = Path(__file__).resolve().parent.parent
SCHEMA = "axiom.foundry.receipt.v1"
# Static camera avoids the existing wall-clock-driven orbit's variable frame samples.
PROTOCOL = {
    "id": "horizon-static-v1", "scene": "hills-static", "seed": 74,
    "arguments": frame_bench.scene_args()["hills-static"] + [
        "--benchmark-terrain-camera=0,0,90,0,-0.12", "--benchmark-view-chunks=8",
        "--benchmark-review-time=0.35", "--benchmark-review-day=1",
        "--benchmark-sky-time=0", "--benchmark-capture-frames",
        "--benchmark-capture-interval=5"],
    "limitations": ["Static camera only; animation and streaming remain wall-clock scheduled.",
                     "PNG sequence is evidence, not a deterministic temporal replay.",
                     "GPU timing is engine telemetry, not a vendor profiler capture."]}


def digest(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def canonical(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()


def command(args, timeout=30):
    try:
        p = subprocess.run(args, cwd=ROOT, capture_output=True, text=True,
                           errors="replace", timeout=timeout)
        return {"command": args, "exit_code": p.returncode,
                "stdout": p.stdout, "stderr": p.stderr}
    except (OSError, subprocess.TimeoutExpired) as error:
        return {"command": args, "exit_code": None, "error": str(error)}


def source():
    rev = command(["git", "rev-parse", "HEAD"])
    status = command(["git", "status", "--porcelain", "--untracked-files=all"])
    diff = command(["git", "diff", "HEAD", "--binary"])
    return {"commit": rev.get("stdout", "").strip(), "status": status,
            "tracked_diff": diff, "clean": status.get("exit_code") == 0
            and not status.get("stdout"), "rollback_token": rev.get("stdout", "").strip()}


def hardware():
    gpu = command(["powershell.exe", "-NoProfile", "-NonInteractive", "-Command",
                   "Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion,"
                   "PNPDeviceID | ConvertTo-Json -Compress"])
    cpu = command(["powershell.exe", "-NoProfile", "-NonInteractive", "-Command",
                   "Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,"
                   "NumberOfLogicalProcessors | ConvertTo-Json -Compress"])
    gpu_ok = gpu.get("exit_code") == 0 and bool(gpu.get("stdout", "").strip())
    cpu_ok = cpu.get("exit_code") == 0 and bool(cpu.get("stdout", "").strip())
    fallback = {}
    if not gpu_ok:
        fallback["gpu"] = command(["nvidia-smi", "--query-gpu=name,driver_version,pci.device_id,pci.bus_id,uuid",
                                   "--format=csv,noheader"])
        lines = fallback["gpu"].get("stdout", "").strip().splitlines()
        # Without native adapter PCI identity, several installed GPUs are ambiguous.
        gpu_ok = (fallback["gpu"].get("exit_code") == 0 and len(lines) == 1
                  and len(lines[0].split(",")) == 5 and "N/A" not in lines[0])
    if not cpu_ok:
        fallback["cpu"] = command(["powershell.exe", "-NoProfile", "-NonInteractive", "-Command",
                    "Get-ItemPropertyValue -LiteralPath 'HKLM:\\HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0' "
                    "-Name ProcessorNameString"])
        cpu_ok = fallback["cpu"].get("exit_code") == 0 and bool(fallback["cpu"].get("stdout", "").strip())
    value = {"platform": platform.platform(), "machine": platform.machine(),
             "gpu": gpu, "cpu": cpu, "fallback": fallback}
    return {"details": value, "fingerprint": canonical(value),
            "complete": gpu_ok and cpu_ok}


def write(path, value):
    Path(path).write_text(json.dumps(value, indent=2, allow_nan=False) + "\n", encoding="utf-8")


def collect(exe, output, passes):
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    exe = exe.resolve(strict=True)
    # Candidate process cannot load settings, save paths or experimental flags from a peer session.
    env = {k: v for k, v in os.environ.items()
           if not k.upper().startswith(("VOXEL_", "WGPU_", "AXIOM_"))}
    env["WGPU_BACKEND"] = "vulkan"
    identity = source()
    receipt = {"schema": SCHEMA, "created_utc": datetime.now(timezone.utc).isoformat(),
               "protocol": PROTOCOL, "protocol_sha256": canonical(PROTOCOL),
               "source": identity, "hardware": hardware(),
               "build": {"executable": str(exe), "sha256": digest(exe),
                         "provenance": "external-unverified",
                         "configuration": None,
                         "reason": "A binary hash does not establish its source or build flags."},
               "runner_sha256": digest(__file__), "backend": "vulkan", "runs": [],
               "merge_authorized": False,
               "unverified": ["clean-checkout build reproduction", "visual correctness",
                              "physics invariants and promotion/persistence", "networking",
                              "memory bounds", "weak-GPU profile", "hidden scenes"]}
    receipt_path = output / "receipt.json"
    write(receipt_path, receipt)
    for index in range(passes):
        directory = output / f"pass-{index:03d}"
        directory.mkdir()
        saves = directory / "saves"
        saves.mkdir()
        pass_env = dict(env, VOXEL_SAVE_DIR=str(saves), VOXEL_CAPTURE_DIR=str(directory))
        args = [str(exe), *PROTOCOL["arguments"]]
        run = {"index": index, "command": args, "status": "failed",
               "contention_before": frame_bench.build_activity(), "metrics": {}}
        with (directory / "stdout.log").open("w", encoding="utf-8") as stdout, \
                (directory / "stderr.log").open("w", encoding="utf-8") as stderr:
            try:
                p = subprocess.run(args, cwd=ROOT, env=pass_env, stdout=stdout,
                                   stderr=stderr, timeout=frame_bench.RUN_TIMEOUT_S)
                run["exit_code"] = p.returncode
            except (OSError, subprocess.TimeoutExpired) as error:
                run["error"] = str(error)
        run["contention_after"] = frame_bench.build_activity()
        try:
            run["metrics"] = frame_bench.parse_report((directory / "stdout.log").read_text(encoding="utf-8"))
            if run.get("exit_code") == 0 and (directory / "frames.csv").is_file() and list(directory.glob("*.png")):
                run["status"] = "captured"
        except frame_bench.BenchError as error:
            run["error"] = str(error)
        receipt["runs"].append(run)
        write(receipt_path, receipt)
        print(f"pass {index}: {run['status']}", flush=True)
    receipt["source_unchanged"] = source() == identity
    receipt["binary_unchanged"] = digest(exe) == receipt["build"]["sha256"]
    gpu_fallback = receipt["hardware"]["details"].get("fallback", {}).get("gpu")
    if gpu_fallback:
        gpu_name = gpu_fallback.get("stdout", "").split(",")[0].strip().casefold()
        matched = bool(gpu_name) and all(gpu_name in r["metrics"].get("adapter", "").casefold()
                                        for r in receipt["runs"])
        receipt["hardware"]["active_adapter_matched"] = matched
        receipt["hardware"]["complete"] = receipt["hardware"]["complete"] and matched
    receipt["artifacts"] = {str(p.relative_to(output)).replace("\\", "/"): digest(p)
                            for p in sorted(output.rglob("*")) if p.is_file() and p != receipt_path}
    receipt["conclusion"] = "Diagnostic baseline only. No improvement or merge decision is supported."
    write(receipt_path, receipt)
    return 0 if all(r["status"] == "captured" for r in receipt["runs"]) else 2


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    sub.add_parser("protocol")
    run = sub.add_parser("run")
    run.add_argument("--exe", required=True, type=Path)
    run.add_argument("--output", required=True, type=Path)
    run.add_argument("--passes", type=int, choices=range(1, 21), default=3)
    args = parser.parse_args()
    if args.action == "protocol":
        print(json.dumps(PROTOCOL, indent=2))
        return 0
    return collect(args.exe, args.output, args.passes)


if __name__ == "__main__":
    sys.exit(main())
