# 0006 Criterion benches, a median-of-3 perf gate, GPU tests behind one helper

Date: 2026-10-22. Status: accepted.

**Benches.** Any crate with a `benches/` directory has criterion benches (`harness = false`). Each crate commits its baselines next to them in `benches/baselines.json` as {bench id: median ns}.
- `just bench` runs once and prints the median against the baseline.
- `just bench-save` records the median of 3 runs as the new baseline.
- `scripts/bench.py` reads criterion's own `estimates.json`, so no custom harness exists.
- `[profile.bench]` uses release optimisation without thin LTO, so the gate does not pay for a full LTO link.

**Gate.** `just bench-check` runs in `just verify`:
- It takes 3 whole passes over every bench, interleaved rather than batched, and uses the median per bench.
- It fails above +8% or on a bench with no baseline.
- A failing pass is measured once more and only a second failure fails the gate: `brick_set_32k_cells` read +16% to +118% while peer sessions compiled and +0.9% on a quiet machine. A real regression fails both passes. Tradeoff: with a few percent of noise a true +9% to +12% regression can pass on the second pass, so the effective threshold sits somewhat above +8%.
- The gate asks cargo where criterion writes (`cargo metadata`, or `CRITERION_HOME`), and a baseline that produced no measurement fails it, so a renamed bench or a moved target directory cannot make it pass blind.
- These settings hold until a quiet baseline machine exists. On this workstation, warm runs of `terrain_generate_8_chunks` agreed within ≈1.5% (88.5, 89.2, 89.7 ms), and a 15% injected regression failed the gate.
- The ignored wall-clock ratio test it replaces read anywhere from 1.07× to 1.98× on the same binary.

**GPU tests.**
- Tests that need a GPU are named `gpu_*` and `#[ignore]`d, so `just test` never needs one. `just test-gpu` runs them one at a time.
- They get their adapter from `engine_gpu::test_adapter()`: `WGPU_BACKEND` selects the backend (default Vulkan, which the contracts were written for), and `VOXEL_TEST_ADAPTER_NAME` requires a specific adapter.
- With no adapter it panics, so the run fails rather than skips.
- A layout test allows only that helper and the windowed renderer to request an adapter.

**Session report.** `just session-report` prints the following against the merge base with `main` or a given rev:
- LOC delta;
- `#[test]` count delta;
- scenario count;
- wall time;
- the last bench results against their baselines.

Dependencies added (CLAUDE.md: each needs a line here):
- `criterion` (dev; `engine_worldgen` and `engine_sim`; default features off, so no plotters or rayon). Removes the hand-rolled timing of the deleted ratio test.
- `proptest` (dev; `engine_sim`, `engine_voxel`; default features off). It replaces hand-picked cases for the brick, coordinate and chunk round-trip tests. It pulls `rand` and its helpers into `Cargo.lock` for test builds only; no game code uses `rand`, and the simulation keeps positional hashing (ADR 0008).
- `bytemuck` (`engine_sim`; already used by the app). One safe `cast_slice` over the brick pool for the GPU upload.
- Workspace edges, no new crates: `engine_content` -> `engine_core` (packed material rows), `engine_voxel` -> `engine_sim` (chunk conversion), the new `engine_gpu` -> `wgpu` and `pollster`.
