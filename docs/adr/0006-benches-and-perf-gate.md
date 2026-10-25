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
- `[profile.bench]` uses one codegen unit and no incremental compilation. With 16 units small cross-crate functions (the hash, cell accessors) stopped inlining and the 262k-cell CA tick measured 22 ms instead of 14 ms.
- The benches pin themselves to one thread per physical core of the fastest class (`pin_to_performance_cores`, Windows `GetSystemCpuSetInformation`). On this hybrid CPU the efficiency cores measured 1.6 to 1.8 times slower, so an unpinned bench reported the core the scheduler picked: the same binary read 14, 20 and 32 ms, and the heat bench read 50 ms where it measured 28.8 pinned (before heat was moved to every fourth tick; it now averages 18.2). Pinned, repeated runs agree within about 3%.
- A baseline is a typical pinned value, not the median of a run that overlapped a peer's build and not the single best run. `bench-save` records the median of 3 passes; if the machine was busy, set the value by hand from several runs and say so in the commit. The two CA baselines are set by hand, not by `bench-save`: movement 16.3 ms (pinned runs 15.5 to 17.0) and heat-everywhere 18.2 ms (pinned runs 17.9 to 18.3); the other three are `bench-save` medians. the gate allows +8% (17.6 ms), a little over the 16.7 ms tick budget, so the budget itself is checked by Phase A's exit measurement and not left to the gate.
- Benches under about a microsecond are not gated (an 8% threshold is tens of nanoseconds); assert the property in a test instead.
- The gate asks cargo where criterion writes (`cargo metadata`, or `CRITERION_HOME`). It fails on a baseline that produced no measurement, on a crate with benches but no `baselines.json`, on a crate that produced no result, and when no crate has benches, so a renamed bench or a moved target directory cannot make it pass blind. It prints whether the benches were pinned. `bench-save` prints each old and new value and flags a worse one.
- Not covered by the gate: the `gpu_*` tests need a GPU and run only through `just test-gpu`; and `.claude/verify/policy.json` does not route edits to `benches/baselines.json`, `scripts/bench.py` or per-crate `Cargo.toml` files to `review_policy`, so changing those alone does not demand a policy review.
- These settings hold until a quiet baseline machine exists. Measured before the benches were pinned (terrain baseline then 90.3 ms; pinned it is 79.6 ms): warm runs of `terrain_generate_8_chunks` agreed within ≈1.5% (88.5, 89.2, 89.7 ms), and a 15% injected regression failed the gate. Gate failures are by design reproducible: a real regression fails both passes.
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
- `criterion` (dev; `engine_worldgen`, `engine_sim`, `engine_core` and `engine_space`; default features off, so no plotters or rayon; it still brings `clap`, `regex`, `serde_json`, `tinytemplate`, `ciborium`, `page_size` (and so `winapi`) and `alloca`, which needs a C compiler). Removes the hand-rolled timing of the deleted ratio test.
- `proptest` (dev; `engine_sim`, `engine_voxel`; default features off). It replaces hand-picked cases for the brick, coordinate and chunk round-trip tests. In `Cargo.lock` it brings `rand` 0.10, a second `getrandom` and `r-efi`, `chacha20`, `cpufeatures`, `core_detect`, `rand_xorshift`, `unarray` and `regex-syntax`, for test builds only; no game code uses `rand`, and the simulation keeps positional hashing (ADR 0008).
- `naga` 27.0.3 (dev; `engine_sim_gpu`; `wgsl-in` only). The version the app already builds with, so the lockfile gains nothing; it lets an ordinary `just test` parse and validate the cellular shaders without a GPU.
- `bytemuck` (`engine_sim`; already used by the app). One safe `cast_slice` over the brick pool for the GPU upload.
- Workspace edges, no new crates: `engine_content` -> `engine_core` (packed material rows), `engine_voxel` -> `engine_sim` (chunk conversion), the new `engine_gpu` -> `wgpu` and `pollster`.
