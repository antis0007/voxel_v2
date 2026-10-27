# Frame-time audit at 08e11ae3

2026-10-27. Brief Milestone 1 deliverable 4, scoped by decision 0036. Numbers come
from `crates/app/benches/frame_baselines.json`: release-fast exe built from a clean
worktree of `08e11ae3`, RTX 4090 (Vulkan), 1600x900, seed 74, generator 53, clear
weather, 6 s warmup, 15 s timed, best of 3 interleaved passes per scene
(`just frame-bench-save`). Timings are per-frame medians from `frames.csv` or the
game's own averages.

## Measurement quality

Peer sessions built and ran the game throughout: 6 of 15 passes overlapped a build
even after waiting up to 15 minutes. Frame rate for the same scene varied up to 3x
between passes (Hills orbit 12.2–29.6 fps). Absolute times below are upper bounds
for a quiet machine. Draw counts are steadier: per-frame medians repeated within 3 %
on the static Hills and quarter scenes, 11 % on orbit (motion) and 21 % on superflat,
whose low pass also ran at 21 fps and likely had fewer chunks resident.
Re-record baselines when the machine is quiet; until then the time half of
`just frame-bench-check` reports UNVERIFIED rather than PASS or FAIL.

## Scenes

| Scene | p50 / p95 / p99 ms | fps | GPU passes ms | shadow | terrain | clouds | CPU render submit | CPU stream | terrain draws | shadow draws |
|---|---|---|---|---|---|---|---|---|---|---|
| hills-static | 26.0 / 28.8 / 31.3 | 38 | 23.4 | 13.5 | 5.6 | 4.2 | 16.4 | 7.5 | 1,146 | 11,713 |
| hills-orbit | 33.8 / 38.2 / 41.3 | 30 | 30.1 | 10.1 | 15.3 | 4.5 | 26.7 | 5.7 | 722 | 7,154 |
| hills-no-clouds | 20.7 / 25.3 / 29.7 | 48 | 20.1 | 14.4 | 5.6 | 0 | 11.5 | 7.3 | 1,188 | 11,954 |
| superflat-static | 18.6 / 22.0 / 23.8 | 53 | 14.2 | 5.2 | 3.1 | 5.8 | 13.1 | 3.8 | 445 | 3,941 |
| quarter-static | 15.9 / 20.2 / 21.8 | 62 | 12.8 | 3.5 | 4.6 | 4.5 | 5.9 | 7.9 | 923 | 9,267 |

No scene meets the brief's 16.6 ms frame (p95 ≤ 14 ms). Superflat draws 4,386 per
frame against the brief's 800 for a flat surface.

## Answers to the brief's audit questions

1. **Meshing on the main thread?** Terrain meshing is on `chunk-mesh-N` workers.
   Main-thread streaming still costs 3.8–7.9 ms per frame in static scenes where
   nothing new loads (`cpu_avg_ms.stream`), and models (`mesh::models::build`) are
   rebuilt each frame (0.67 ms measured 2026-10-22).
2. **Passes and pipelines.** Not counted in this audit.
3. **Overlapping geometry at scale seams.** Scale is one preset per world (ADR 0007);
   LOD seams were not measured here.
4. **Simulation scope.** Not exercised by these scenes (no fluids); the cellular
   path uses dirty bricks (ADR 0007).
5. **One culling path?** No: terrain, shadows and horizon cull separately; shadow
   draws are 8–10x terrain draws in every scene.
6. **Texture binding.** Terrain uses vertex colour and procedural surfaces; no atlas.
7. **Largest allocation per frame.** Not measured (no counting allocator).
8. **Three largest p95 contributors** (below).

## Three largest costs

| # | Cost | Evidence | Owner | Target |
|---|---|---|---|---|
| 1 | Shadow cascades: 13.5–14.4 ms GPU and 11.7–12.0 k draws per frame on Hills | `gpu_avg_ms.shadow`, `frame.shadow_draws`; `render/shadow.rs` 4 near + 1 far cascades at 2048² | renderer (`render/shadow.rs`; indirect multi-draw work was in progress 2026-10-22) | shadow draws ≤ 2x terrain draws, shadow GPU ≤ 5 ms on hills-static |
| 2 | CPU render submission: 11.5–26.7 ms per frame on the main thread | `cpu_avg_ms.render_call`; tracks draw count | renderer (`render.rs` `Gpu::step_inner`) | ≤ 3 ms (brief budget) after #1 cuts draws |
| 3 | Main-thread streaming with nothing to load: 3.8–7.9 ms per frame | `cpu_avg_ms.stream` in static scenes | streaming (`render/streaming*`) | ≤ 1 ms when no chunk changes |

Clouds (4.2–5.8 ms GPU) are fourth; they were first on 2026-10-24 before the
single-evaluation march landed.

## Not measured

Factory-dense, underground with active fluids and lights, orbital approach and
combat casting have no benchmark scene yet. Existing review fixtures
(`--benchmark-water-review`, `--benchmark-torch-review`, `--benchmark-sandbox-review`,
the receiver review) are candidates; each must exit on its own and print the
`[benchmark]` summary before it can join `scripts/frame_bench.py`.
