# ADDENDUM B — Run well on any hardware from the last decade (append as Part VIII-b; supersedes Part VIII where they conflict)

Goal: 60 fps / 60 tps on a 2016 laptop iGPU at a reduced-but-honest preset, 30/30 on a 2014 iGPU with graceful degradation, and full presets on mid-range 2020+ hardware. No feature may exist only on the high end: the simulation is identical everywhere; only resolution, distance, rates, and visual effects scale.

---

## B.1 Hardware classes and acceptance targets

| Class | Example hardware | Backend | Preset | Target | Hard limits |
|---|---|---|---|---|---|
| H0 "Potato" | Intel HD 4000/4400 (2012–14), 4 GB RAM, 2c/4t, no compute on GL | GL (no compute) → **CPU sim backend** | Minimal | 30 fps, 30 tps | 1.0 GB RAM, 512 MB VRAM, view 6 chunks, 64k awake cells |
| H1 "Old laptop" | Intel HD 520/620, Iris 540 (2015–17), 8 GB, 2c/4t | Vulkan/DX12/Metal, compute OK | Low | 60 fps, 60 tps | 1.5 GB RAM, 1 GB VRAM, view 8, 150k awake |
| H2 "Old desktop" | GTX 750 Ti/960, RX 460/560, i5-4xxx (2014–16), 8–16 GB | Vulkan/DX12 | Medium | 60/60 | 2.5 GB RAM, 2 GB VRAM, view 12, 300k awake |
| H3 "Deck/M1" | Steam Deck, Apple M1, Ryzen 5 iGPU 2020+, 8–16 GB | Vulkan / Metal | Medium+ | 60/60 (Deck 40 Hz mode) | 3 GB RAM, 2 GB VRAM, view 12, 400k awake |
| H4 "Mid" | GTX 1660 / RX 5600 / RTX 3050, 16 GB | Vulkan/DX12 | High | 120/60 | 4 GB, 4 GB, view 20, 500k |
| H5 "High" | RTX 3070+/RX 6800+/M2 Pro+ | Vulkan/DX12/Metal | Ultra | 144+/60 | 6 GB, 6 GB, view 32, 1M |

Rules:
- Targets are p95 frame time on the bench scenarios, not averages.
- Each class has a CI/bench machine or a documented emulation profile (`just run --profile h1`) that caps threads, VRAM, and features with wgpu `Limits` and a `Features` mask so H0/H1 are reproduced on any dev box.
- A PR that regresses any class > 8% p95 fails the perf gate.

## B.2 Backend matrix and fallbacks

- wgpu with Vulkan (Win/Linux/Android), DX12 (Win 10+), Metal (macOS/iOS), GL (fallback: Win 7–8, old Linux, GL 3.3+/ES 3.0). WebGPU build for browser demos later (same code, smaller limits).
- **Capability probe at startup** writes `caps.ron`: compute shaders, max storage buffer size, max workgroup size, subgroup ops, timestamp queries, indirect draw, multi-draw-indirect, texture formats (BC/ETC/ASTC), VRAM, adapter name. Presets derive from caps + a 3-second micro-bench, never from the adapter name alone.
- **No compute (GL):** the CPU `CaBackend` is the authoritative sim; meshing on CPU workers; volumes rendered as billboard clouds; indirect draw emulated with per-class batched draws. This path is tested in CI with the GL backend forced.
- **No timestamp queries:** overlay falls back to CPU timing.
- **Storage buffer limits** (some mobile/old drivers: 128 MB): the brick pool is split into N buffers of ≤ 128 MB; brick id = (buffer, index). Design the pool for this from day one.
- **Shader compilation:** WGSL via naga for Vulkan/Metal/DX12; precompiled pipeline cache (`wgpu::PipelineCache`) saved per adapter; warm all pipelines at load with a progress bar, never on first use mid-game.
- **Driver bugs:** maintain `assets/engine/driver_quirks.ron` (adapter/driver regex → disabled feature or workaround). Loaded before pipeline creation.

## B.3 Scaling ladder (every knob, who owns it, what it costs)

All knobs live in `assets/engine/profiles/<preset>.ron` and are hot-reloadable. The auto-tuner (B.5) moves them in this order when over budget:

1. Render resolution scale (0.5–1.0) + FSR1/bilinear upscale. Cost owner: render.
2. Volume (gas/liquid/fire) raymarch resolution (¼ → ⅛ → billboards).
3. Shadow cascades (4 → 2 → 1 → none) and shadow map size.
4. View distance in chunks (6–32) with far-LOD coarse bricks beyond `fine_distance`.
5. Mesh LOD: fine bricks within R, 2× coarse beyond, 4× beyond that (same data, coarse CA).
6. Awake-cell budget per tick (64k–1M); beyond budget, farthest awake bricks are deferred (round-robin, not dropped), so distant physics runs slower, never wrong.
7. Gas tick divisor (1–4), thermal divisor (1–4), light update budget.
8. Body cap (32–256), mob cap (64–500), mob LOD radii.
9. Particle count, decal count, post effects (SSR, bloom, AO: on/half/off).
10. Audio voice count and occlusion rays.
11. Texture resolution / material procedural detail (procedural materials scale free; atlas tier if used).
12. Last resort: tick rate 60 → 30 with 2× substep velocities (sim stays deterministic per tick count, scenarios are tick-indexed).

Invariants: the sim never changes *rules* by preset, only *budgets*; a scenario's world hash at tick N is identical on every preset when run headless at fixed budgets (the "preset does not change outcomes" test).

## B.4 Memory plan (hard budgets, measured)

- Brick pool: 8 KB/brick max (A+B planes + up to 8 field planes); H0 cap 48k bricks (≤ 384 MB), H5 cap 600k. Uniform bricks free. Eviction: LRU by distance, written to disk page cache (LZ4) before free.
- CPU mirror: prediction region (radius 3 bricks ≈ 343 bricks) + authoritative headless; never a full copy on GPU-backed profiles.
- Mesh pool: 8-byte quads, 16 MB (H0) – 256 MB (H5), per-brick slots sized by quad count class (64/256/1024/4096) to avoid fragmentation.
- Entity tables: SoA, pre-sized per cap, no per-tick allocation (allocation counter test = 0 in steady state).
- Textures: procedural materials by default (no atlas memory); optional 1k atlas with BC7/ASTC chosen by caps.
- Disk: chunk pages LZ4, 32 KB average; save size test per bench world; streaming reads on a dedicated IO thread with a 64 MB page cache.
- RAM budget test: `just mem --profile hX` asserts RSS and VRAM under the class limit after 10 minutes of the `tour` scenario.

## B.5 Auto-tuning and user control

- First launch: probe + 3 s micro-bench (brick CA, mesh, draw, fill-rate) → preset. Show a one-line summary ("Running Medium; press F7 to change") and never nag again.
- Runtime governor: moving p95 frame/tick time; if over budget for 2 s, step one knob down the ladder; if under by 30% for 30 s, step up. Hysteresis and a per-knob cooldown; changes logged to the overlay. User can pin any knob.
- Settings UI exposes presets + the ladder as individual sliders with a live cost estimate (ms) from the micro-bench coefficients.
- Deterministic benchmarks are run with the governor off.

## B.6 CPU plan (old 2c/4t CPUs are the real constraint on H0–H1)

- Threading: main thread = input/ingest/predict/render submit; worker pool (N-1 threads, min 1) = meshing jobs, nav summaries, support relaxation, IO, audio mixing; sim orchestration on main. No thread is allowed to block on the GPU except the dedicated readback thread (contacts/events, 1-tick lag).
- SIMD: CPU reference kernels use `std::simd`/`wide` with scalar fallback; AVX2/NEON dispatch at runtime. Target: CPU CA ≥ 2M cell-updates/s per core on 2015 hardware (H0's authoritative path).
- Allocation: zero steady-state allocations in sim and render; arena per tick for transient data; bump allocator for jobs.
- Cache: counting-sort entities by brick; brick headers in a flat array; hot structs ≤ 64 B.
- Build: `opt-level=3`, `lto=fat`, `codegen-units=1`, `panic=abort` for release; `target-cpu=x86-64-v2` baseline (2009+ CPUs), with a v3 (AVX2) build variant selected by launcher; ARM64 builds for Apple/Deck-class ARM.
- Startup: ≤ 5 s to menu on H1 (measured); world load ≤ 10 s; pipeline warm with progress; lazy content load by profile.

## B.7 GPU plan details that matter on weak GPUs

- Workgroup size 64 (iGPUs choke on 256); one brick per workgroup; shared memory ≤ 16 KB (10³ halo of 8-byte cells = 8 KB).
- Avoid: 64-bit atomics (use split u32), subgroup ops (optional fast path only), geometry shaders, tessellation, bindless (not on old drivers). Use: indirect draw, storage buffers, compute, timestamp queries where present.
- Bandwidth is the limiter on iGPUs: 8-byte cells, 8-byte quads, R8 volume textures, packed headers; readbacks ≤ 64 KB/tick total.
- Fill-rate: depth pre-pass, front-to-back material-class draws, volumes at ¼ res, no overdraw-heavy transparency; particles capped per preset.
- Thermal throttling (laptops): governor reacts; frame pacing uses a fixed-step sim with interpolated render to keep input feel at 30 fps.

## B.8 Feature improvements that also reduce cost

- **Coarse far-field simulation**: beyond `fine_distance`, bricks store 2³/4³ cells per entry and run the same CA with scaled rates; conservation on promotion/demotion by an ADR'd rounding rule. Makes mountains and oceans cheap.
- **Event-driven far world**: outside the awake budget, time-skip bricks on reload by integrating elapsed ticks with closed-form rules (crops grow, water settles, fires burn out, ores don't change) — "catch-up" is a scenario test.
- **Deterministic interpolation**: render at any fps from a 60 tps sim with per-cell velocity interpolation for powder/liquid motion; mobs/bodies interpolated by state; input latency unaffected.
- **Replay-based bug reports**: F8 writes the last 30 s of actions + world hash + caps; agents reproduce on any machine.
- **Tour scenario**: a scripted 10-minute flythrough + build + fight + factory used for every class bench and memory test.

## B.9 Testing and CI for hardware breadth

- CI matrix: Linux (Vulkan via lavapipe for correctness, GL via llvmpipe for the no-compute path), Windows (DX12 + Vulkan on a cheap dGPU runner), macOS (Metal on M1 runner). Headless CPU sim on all.
- Emulated profiles H0–H5 run the bench suite nightly on one mid machine; real-hardware benches weekly on an H1 laptop and a Deck (document owners).
- Golden images per class preset (differences by preset are expected and versioned separately).
- Fuzz the capability probe with synthetic `caps.ron` files to prove every combination picks a valid preset and all pipelines compile.
- Perf gate = 3-run median p95 per class; 8% threshold; results in `docs/PERF.md` with the hardware table.

## B.10 Session plan addendum (interleave with milestones)

- **S-H1 (before M1 ends):** caps probe, profiles H0–H5, emulation flags, GL no-compute CPU path proven on `dam_break` at 30 tps, split brick pool, pipeline cache, startup budget.
- **S-H2 (M1):** ladder knobs wired, governor, overlay cost readouts, memory tests, `tour` scenario.
- **S-H3 (M2):** CPU SIMD kernels, zero-alloc audit, thread pool, interpolation.
- **S-H4 (M3):** coarse far-field CA, catch-up rules, far-LOD meshing.
- **S-H5 (M4+):** per-class golden images, driver quirks table, real-hardware weekly benches, Deck/M1 passes, replay bug reports.

Each hardware session ends with the class table filled with measured numbers; a class without a number is "unsupported" in docs until it has one.
