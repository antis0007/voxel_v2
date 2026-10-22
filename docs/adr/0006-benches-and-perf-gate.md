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

Dependencies added: `criterion` (dev, `engine_worldgen`; default features off, so no plotters or rayon). The new crate `engine_gpu` depends on `wgpu` and `pollster`, which were already in the tree.
