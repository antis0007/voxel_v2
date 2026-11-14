# Workload performance evidence for Forge and agents

The workload analyzer consumes existing native telemetry. It does not modify the
engine, synthesize a crowd or treat a timing correlation as a proven cause.
The first workload is the population task's 20-resident paid-construction fixture:
physical bodies and jobs, one stockpile and 256 finite plank orders. Native seeds
come from the benchmark log; the separate CPU-only physical fixture uses 424242.
It is a bounded local construction test, not a complete city economy.

## Ingest a completed capture

```powershell
python -I scripts/foundry_perf.py --input C:/runs/builders20 --context C:/runs/context.json --physical-check C:/runs/physical/population.json --output C:/evidence/builder20-performance
```

The output directory must not exist. Up to 64 MiB of native logs, frame CSVs,
manifest and route timing are copied into `raw/` and hashed. Source files are
checked for changes during copying. An optional physical check is separately
bounded to 1 MiB. Images and executables are not duplicated. `performance.json`
contains machine-readable findings; `performance.md` is the human summary.

Context JSON supplies a workload ID, seed, hardware/driver record, backend, build
configuration identity and declared execution/cache environment. Unknown values
must remain unknown. A binary hash and build-command receipt are useful evidence
but do not independently attest its source snapshot.

```json
{
  "workload_id": "city20-paid-stockpile-v1",
  "seed": 424242,
  "backend": "vulkan",
  "hardware": {"cpu": "record actual CPU", "gpu": "record actual GPU and driver"},
  "build": {"configuration_hash": "build-command hash", "source_snapshot": null},
  "workload_environment": {"execution_domain": "record actual domain", "cache_policy": "record actual warming"}
}
```

## What is measured

- Frame interval mean, nearest-rank p50/p95/p99/max, diagnostic budget exceedances
  and five slowest raw frame rows. Slow frames are retained.
- CPU gameplay, streaming, render-call and horizon spans. Acquire, submit and
  present are labeled nested spans. Their sum is not total CPU work.
- GPU total and child stages. Delayed GPU samples are deduplicated by frame/config
  ID; missing, unsupported, mismatched and future samples are explicitly counted.
  GPU time is not added to CPU wall time or assumed to belong to the current frame.
- Available queue, upload, terrain geometry and cache-residency ranges. These
  counters are not total process RAM, all VRAM allocations or network traffic.
- Startup markers and opt-in trace intervals, labeled as full-process evidence
  including loading/warmup. Thresholded gameplay-stall logs are not complete profiles.
- Route timing separates all attempts from attempts marked as measuring. Event-loop
  gaps, frame-pacer retries, dispatch and resident upload spans remain distinct.
  Incomplete recordings, dropped rows and summary/CSV count differences are flagged.

The arithmetic frame interval minus selected CPU spans is labeled **unattributed**.
It may contain uninstrumented work, event-loop delay, OS scheduling or other waits.
It is never assigned to pathfinding, rendering or a particular cause. Rare large
gaps are raised even if they fall beyond p99.

Findings include observed span magnitudes, candidate source owners and specific
next probes. A high render-call span suggests splitting model assembly, uploads
and driver waits; it does not establish that resident geometry is responsible.

## Workload honesty

Declared residents, active physical actors, assigned workers, busy workers,
carriers, visible actors and productive builders are different counts. Missing
per-frame visible/working counters are explicit gaps. A separate physical fixture
can establish movement, paid placements, material conservation and save round-trip;
it cannot establish how many actors were visible or productive in a native frame.
A short fixture with zero placements is not accepted as productive construction.
The native log's seed takes precedence over declared context. A contradictory
declaration is retained and blocks comparison; a CPU fixture's seed must not be
substituted for the native world's seed.

Basic frame telemetry does not independently split path search, job search,
fixed-step counts or voxel remeshing. These remain unmeasured until instrumented
or supported by appropriately scoped traces. No missing probe is reported as zero.

## Compare only matching work

```powershell
python -I scripts/foundry_perf_compare.py --baseline C:/evidence/before/performance.json --candidate C:/evidence/after/performance.json --output C:/evidence/comparison.json
```

Repeat baseline/candidate options for independent passes. The verifier checks raw
hashes, recomputes the report and requires the same analyzer version, workload,
arguments, warmup, duration, preview phase, tracing/capture/route flags, backend,
hardware, active adapter, native resolution, build configuration and diagnostic
budget. Different source variants are allowed; each variant needs one exact binary
identity. Duplicate captures cannot count as independent repetitions.

An incompatible trace or backend comparison is rejected instead of presented as
an optimization. One pass yields descriptive deltas only. At least three independent
passes per variant are required for a pass-level bootstrap interval; frames are
not independent experimental repetitions. All outputs retain `merge_authorized=false`.
Visual/work equivalence, build trust and independent acceptance are still separate.
Exit codes are `2` for incomparable data, `1` for a repeated compatible timing
regression whose pass-level interval exceeds tolerance, and `0` for descriptive
results without that finding. A regression observation still carries no causal or
integration authority.

## Forge and agent context

The Foundry panel discovers immediate evidence-root children with `performance.json`.
It shows frame tails, observed spans, missing workload proof and next probes. Raw
artifacts are accessible through the existing authenticated, hash-checking API.

```powershell
python -I scripts/install_foundry_perf_forge.py --forge-root C:/engine --backup C:/evidence/perf-hook-backup --expected-addons C:/evidence/prior-foundry-install/manifest.json --apply
```

The installer checks the prior addon hashes, backs up exact existing source and
adds narrow hooks to context collection and its three agent prompt serializers.
Unknown integration anchors or concurrent addon changes stop installation.
No agent tool, edit permission or merge authority is added.
For later addon-only updates, pass `--refresh-addons` and the last installation's
manifest. This mode never rewrites the host's agent context or runner files.

For performance-related tasks, up to three relevant reports fit within a 9,000-byte
context budget. Explicit actor counts scope the selection. Severe frame tails are
prioritized so a fast average cannot hide a stall. Packets include binary/build,
hardware/configuration, measured spans, physical-fixture scope, uncertainty and raw
report identity. Report and artifact hashes are checked again before a gathered
context is used. Unsupported or stale evidence becomes a visible context error.

These packets are reported data and hypotheses, not instructions or verified causes.
The agent must preserve paid work, cargo, collision, saves and multiplayer behavior
and verify an actual change on a matching workload.
