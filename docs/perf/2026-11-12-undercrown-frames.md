# Undercrown frame cost, 2026-11-12

## Setup

- Build: release-fast, built from a D: snapshot of the shared tree.
- Hardware: 4090, default benchmark view distance.
- Voxel scale: 1 m.
- Camera: `VOXEL_REVIEW_DUNGEON=undercrown:<room>`.
- Flags: `--benchmark-motion=cloud-review --benchmark-warmup=17 --benchmark-duration=10`.
- One pass per room per generator, not interleaved. Treat the timings as indicative only.

| room | gen | median ms | p95 ms | GPU pass ms | shadow ms | terrain draws | terrain tris |
|---|---|---|---|---|---|---|---|
| First landing | 290 | 30.6 | 41.8 | 30.2 | 6.0 | 757 | 5.28 M |
| Counterweight hall | 290 | 23.4 | 29.7 | 20.7 | 6.0 | 756 | 5.35 M |
| Crown Chasm | 290 | 30.0 | 44.3 | 28.6 | 6.8 | 725 | 4.96 M |
| Bridge village | 290 | 25.8 | 35.3 | 24.9 | 6.4 | 724 | 5.24 M |
| Hall of patient bells | 290 | 22.9 | 36.1 | 19.5 | 5.8 | 715 | 4.87 M |
| Clifftop observatory | 290 | 24.2 | 32.5 | 23.9 | 5.4 | 752 | 5.20 M |
| First landing | 292 | 18.6 | 23.7 | 18.5 | 4.7 | 757 | 5.28 M |
| Counterweight hall | 292 | 16.5 | 21.6 | 15.3 | 4.8 | 756 | 5.35 M |
| Crown Chasm | 292 | 12.7 | 17.2 | 11.9 | 5.1 | 725 | 4.96 M |
| Bridge village | 292 | 24.0 | 32.4 | 23.7 | 6.2 | 733 | 5.12 M |
| Hall of patient bells | 292 | 25.4 | 31.1 | 24.6 | 6.0 | 715 | 4.87 M |
| Clifftop observatory | 292 | 31.2 | 39.6 | 30.4 | 6.1 | 752 | 5.20 M |

## Findings

- **The frame is GPU-bound in terrain.** Inside every room the GPU draws about 5 M terrain triangles in about 730 draws, which matches surface frames at the same view distance. The hills, terrain and caves around the Undercrown are drawn even though the room hides them. Cave culling does not remove them at this distance.
- **Shadows cost 5–7 ms per frame**, the second largest item.
- **Generator 292 prop models add nothing measurable.** Triangle and draw counts are within 0.1 %. The swings in the timing columns are run-to-run noise: for example, the same Crown Chasm pose took 30.0 ms and 12.7 ms with identical triangle counts. A reliable A/B needs interleaved passes.
- **Torch occlusion upload time is not in frames.csv**, so this capture could not test the half-scale restamp suspect.

## Why cave culling removes nothing

`[benchmark] latest_scene` now prints `cave_culled`. It was 0 in First landing and Crown Chasm. A throwaway diagnostic (D: snapshot only) in First landing showed:

- The LOD 0 grid was 17 × 12 × 20 = 4080 chunks. The walk reached 4074 of them, so it leaks out of the Undercrown almost at once.
- Of 661 frustum-visible chunks, 449 (68 %) were LOD > 0. `Occlusion::hides` never culls those, so even a sealed walk could remove at most a third of the draws.

## Next lever (not implemented)

1. Find the leak. Likely causes: the well and entrance shafts, cave noise connecting the rooms to the hill surface, and 32 m chunks whose single flood component joins room air to cave air. Log the walk's path from the camera chunk to the first surface chunk.
2. Give LOD > 0 meshes a conservative connectivity, for example the OR of their LOD 0 children's links, or compute it from the coarse voxels. Then let `hides` cull them.
3. Re-measure triangles and GPU time in the same rooms, with interleaved passes.
