# Engine performance and physics candidates (2026-11-06)

Source: six web research passes (storage, meshing and rendering, rigid bodies,
fluids and granular, worldgen and loading, simulation threading), mapped to the
Axiom brief's measurable goals: more voxels, faster entry, faster frames, better
physics. External numbers are the source projects' own unless marked.

## Baselines read from the code on 2026-11-06

| Item | Value | Where |
|---|---|---|
| Terrain vertex | 13 f32 = 52 B, 208 B per quad | `mesh.rs` `Vertex3` |
| `surface` attribute | f32x4 carrying wall-clock stamps and packed codes | `mesh/water_motion.rs` `stamp` |
| Authoritative chunk | dense u8 block + u8 light = 64 KiB | `voxel/src/chunk.rs` |
| Streaming source | 34^3 u8 blocks + three 34^2 column arrays, about 53 KB | `mesh.rs` `GeneratedChunk` |
| CPU source budget | 256 MiB, about 4,900 sources | `render/streaming/residency.rs` |
| `engine_sim` tick, 262k cells awake | 16.3 ms (62 ns per cell) | `crates/sim/benches/baselines.json` |
| Live water | serial CA, 0.6 ms per step idle | 2026-10-27 audit |
| Rigid | 64 bodies x 128 cells, 8 PGS iterations, no warm start or islands | `simulation/rigid/README.md` |

Already in the tree, so not candidates: multi-draw indirect for chunks
(`streaming/indirect_draws.rs`), cave culling (`streaming/occlusion.rs`), a
pipeline cache (`render/pipeline_cache.rs`), staged far shadow cascades
(3147f06f).

## Candidates and critique

Scores: gain (measured or derived), cost, risk to determinism or visuals, and
collision with files other sessions are editing on 2026-11-06.

### 1. Packed quads (8 B) with vertex pulling
- Gain: 208 B to 8 B per quad, about 26x less vertex data; every shadow
  cascade reads the same buffers. Largest lever for the UHD 770.
- Critique: the mesher emits `Vertex3` from 61 files, and `surface` encodes
  per-vertex water motion and material codes as f32. A true quad format means
  a mesher rewrite plus a shader contract change. Packing at upload alone
  (u16 position, f16 colour, snorm normal) reaches only about 36 B (1.4x)
  because `surface` must stay f32.
- Collision: mesh, streaming upload, shadow and shader files all changed in the
  last 6 hours.
- Verdict: winner, but as its own owned project with captures. Not this pass.

### 2. Uniform collapse and palettes for chunk data
- Gain: 0 B for one-value chunks, 8 to 16 KiB for mixed ones; view 28 could fit
  the 256 MiB budget and the source-cache stall goes away.
- Critique: `blocks` is a public dense field read at 89 index sites. The
  streaming source keeps a padded `Vec<BlockId>` that the mesher indexes
  directly. Collapse fits the cache (`ResidentChunk`) best, but that file is
  under active edit. A palette with more than about 256 ids is worse than dense
  (voxel-forge), so a dense fallback is required.
- Verdict: winner after measuring the one-value fraction at view 28.

### 3. Streaming scheduler saturation
- Gain: C2ME runs the same generator 10 to 30x faster than vanilla by removing
  lock and dependency serialisation. Our symptom (300 pending, 0 to 16
  generating) matches.
- Critique: cause unmeasured; could be a dispatch cap, a cache lock or a
  dependency gate. Streaming files are under active edit.
- Verdict: measure first (per-second pending, in flight, idle workers).

### 4. Live water quality: brick sleep, integer dead-band, water bodies
- Gain: settled lakes cost 0 cells per tick; no surface jitter; no per-cell
  outlet search.
- Critique: `cellular.rs` and `cellular/drainage.rs` changed in the last 6 hours
  and already carry a lip cap and inertia dead-band. Bodies need invalidation
  on every edit path.
- Verdict: winner, coordinate with the owner of `drainage.rs`.

### 5. `engine_sim` kernel throughput
- Gain: a plain 3D sand CA measures 2.6 ns per cell-step on one thread; ours is
  62 ns. Lockstep multiplayer (every peer runs `engine_sim`) and the headless
  CPU region pay this directly.
- Critique: the kernel must stay byte-identical to the WGSL port. Blocks of one
  phase are disjoint, so batching and parallel brick work cannot change bytes.
  In the odd phase 98 of 125 blocks per brick straddle a face and resolve each
  cell's slot three times through a linear cache scan.
- Collision: `kernel.rs`, `anchors.rs`, `store.rs` untouched by others.
- Verdict: winner, implement now.

### 6. Simulation thread with render interpolation, agreed time dilation
- Gain: main-thread sim under 0.5 ms; no catch-up spiral.
- Critique: touches `main.rs` and `simulation/fixed.rs`, both under edit;
  interpolation needs prior/current transforms for player, bodies and water
  motion.
- Verdict: winner, needs a design note and owner slot.

### 7. Rigid: Soft Step, warm starting by voxel face, persistent islands
- Gain: Avian measured 4 to 5x over XPBD with stable stacks; Box2D islands cut
  sleep bookkeeping from 0.69 to 0.01 ms at 10k bodies.
- Critique: `rigid.rs`, `rigid/contact.rs` and `rigid/fracture.rs` changed in
  the last 6 hours. Needs the determinism harness first so the change is
  provable.
- Verdict: winner, after the harness; coordinate with the rigid owner.

### 8. Cached scrolled shadow cascades and sun quantisation
- Gain: Insomniac reports about 70% less static geometry redrawn; removes
  refresh hitches.
- Critique: overlaps 3147f06f (staged far cascades). Quantised sun steps can
  show as visible jumps unless cross-faded.
- Verdict: second tier; extend the existing staging.

### 9. Persisted macrogeography and horizon tiles
- Gain: removes the 21 to 41 s horizon wait on reopen.
- Critique: the cache key must cover seed, generator version and planet
  parameters; worldgen is under heavy edit.
- Verdict: winner for load time; separate pass.

### 10. Cheaper density sampling and SIMD noise
- Gain: 2 to 7x on density-bound columns (FastNoise2, Hytale guide).
- Critique: any change to sampled values is a generator gate.
- Verdict: only as a new gate with A/B captures.

### Rejected
- Real-time MPM sand: no shipped game, 10 s per frame at 256^3 on GPU.
- GPU rigid bodies: deterministic only on one GPU model; breaks lockstep.
- Mesh-shader LOD (Nvidium): wgpu has no mesh shaders.
- SVDAG for authoritative data: no per-tick writes; far field only.
- Full structural stress: judged impractical by Teardown; bounded connectivity
  instead.

## This pass: candidate 5 landed

Changes in `crates/sim/src/kernel.rs`:
- Blocks inside one brick are grouped per brick: one slot lookup per brick
  instead of one per block. Straddling blocks keep their order, which is the
  only order that can decide a pool-full stall.
- A straddling block splits each axis once (`block_spots`) and resolves its
  eight slots once for the read, the write and the wake, reusing the cache
  entry while consecutive cells share a chunk.
- A block of eight `EMPTY` cells skips the rule when empty space has no
  reaction; a test pins that the rule leaves such a block alone on every phase.
- Brick groups can run on worker threads (`set_kernel_workers`,
  `VOXEL_SIM_WORKERS`); the default is one thread.

Byte identity: the 95 `engine_sim` tests pass, including the pinned canonical
scenario, dam-break replay and the lockstep proof; a new test runs a 262k-cell
busy region with one and four workers and compares hash and stats every tick;
all 9 `engine_sim_gpu` GPU parity tests pass on HEAD plus this change. On the
shared tree 5 GPU tests fail with a wgpu validation error from uncommitted
`sim_gpu` resident work; HEAD plus this change alone passes them.

Measured on 2026-11-06 with peer builds running (11 rustc processes), best of
interleaved runs:

| Scene | Before | After (1 thread) | Gain |
|---|---|---|---|
| bench `ca_step_262k_cells_all_awake` | 15.71 ms | 12.75 ms | 1.23x |
| bench `..._with_heat` | 18.09 ms | 14.38 ms | 1.26x |
| `dam_break_200k_tick_time` p50 | 3.41 ms | 2.75 ms | 1.24x |
| `dam_break_200k_tick_time` p95 | 5.33 ms | 4.24 ms | 1.26x |
| `dam_break_200k_tick_time` p99 | 6.27 ms | 4.83 ms | 1.30x |

Four workers reached 10.8 ms (1.45x) on the bench at best, but a phase waits
for its slowest worker and under peer load single runs took 30 to 280 ms. Keep
one thread until a quiet-machine measurement or a persistent pool exists.

Profile of the remaining tick (busy region, 16.3 ms): the rule itself is about
55%, anchor listing 5%, the rest is per-block bookkeeping. The next kernel step
is the rule's per-cell table lookups, not more threads.
