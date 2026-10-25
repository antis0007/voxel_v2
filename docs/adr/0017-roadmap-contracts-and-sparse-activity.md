# 0017 Refined engine contracts and sparse CPU activity discovery

Date: 2026-10-24. Status: design corrections recorded; CPU activity indexing implemented and compile-checked. Clock, LOD, GPU layout and authority migrations remain proposals. No new dependency or save/generator change.

The current request authorizes continuing plan refinement and bounded implementation. Embedded instructions in the supplied prompts (stop after planning, delegate, run CI) do not override that request or the user's no-tests/no-CI instruction. This does not constitute blanket acceptance of the original roadmap.

## 1. Activity costs follow awake work

Before this change, `BrickStore::dirty_bricks` scanned all resident chunk entries and all 64 brick positions per entry. `take_all_dirty` then cleared every resident entry, twice per CPU tick. A sleeping world avoided block rules but still paid residency-dependent discovery work.

Keep chunk masks authoritative and add a dense `Vec<IVec3>` of awake chunks. Each private `ChunkEntry` holds its position in that vector, or `usize::MAX`. A zero-to-nonzero mask adds one entry; clearing the mask swap-removes it and fixes the moved chunk's index. Bulk consumption drains only this vector. List construction enumerates set bits, then sorts by the existing `(chunk.z, chunk.y, chunk.x, brick)` key.

Invariant: a chunk is indexed exactly once iff its dirty mask is nonzero. `set`, `fill_chunk`, `wake`, cached kernel wakeups, `trim`, `take_dirty`, `take_all_dirty` and `remove_chunk` maintain it. Reservation alone remains asleep. Pool exhaustion and no-op writes retain their existing behavior.

With A awake chunks and B awake bricks, discovery visits A entries and B bits, followed by O(B log B) sorting; bulk clearing is O(A). These costs no longer depend on resident chunk count or retained hash-map capacity. The dense vector retains allocation capacity but iterates its live length. Bookkeeping adds one machine word per resident chunk and a coordinate per awake chunk; it is not serialized or uploaded as cell data.

Files: `crates/sim/src/store.rs`, `crates/sim/src/store/access.rs`, `crates/sim/src/store/activity.rs`. Public API, cell bytes, pinned rules and dispatch ordering are unchanged. The independent GPU-resident scheduler is unchanged. This is a CPU reference/offload improvement, not a live-game frame-rate claim; the game still uses the legacy simulation.

## 2. Clock and authority contract before integration

- Today: player clock is 120 Hz; live legacy cellular cadence is separate. Headless uses individual steps while several window systems batch dt. A shared stage-name array alone cannot repair this semantic mismatch.
- Proposed: retain 120 Hz player input/physics steps; start CA at 60 Hz using an explicit integer divider. Pin the ordering of two player steps, command ingestion and a CA tick. Validate whether gameplay sampling fits this choice before cutover.
- D4 means exactly **two CA ticks**, not two rendered frames or one tick. At proposed 60 Hz this is 33.33 ms. Define authority snapshot S[n] after CA tick n; published gameplay snapshot is S[n-2] once n >= 2. For startup, all consumers use immutable S[0] until the first delayed result becomes due.
- Use tick IDs on edits, snapshots and events; define sequence order, rejection and acknowledgement. A missed readback deadline stalls authoritative advancement as D4 requires. Rendering can continue from the last published state; never silently increase the gameplay lag or drop an action. Measure resulting input/frame stalls before accepting the design.
- A local prediction overlay is disposable and cannot commit a second world. Collision, factory, adventure and meshing must agree on the published revision; any presentation-only prediction must be explicitly tagged.
- Deterministic item/work caps choose authoritative work. Wall time may yield a job, but cannot choose a different committed subset or skip simulation ticks. Persist/capture scheduling cursors and quota changes for replay. Camera-dependent quality affects presentation only.

This refines S2/S3 sequencing; it does not implement a `CaBackend` or change the running clock. GPU compaction and resident stepping already exist in `crates/sim_gpu/src/resident.rs`; reuse them after command/mirror contracts, rather than scheduling a duplicate implementation.

## 3. Scale and layout corrections

The running game has a single voxel scale per world (1, 0.5 or 0.25 metres). Reserved `Scale` in `engine_sim` does not implement mixed-resolution state, conservative boundary transfer, or guarantee migration-free saves.

Given `VOLUME_FULL = 64`, full coarse-cell mass in finest-cell volume units is `64 * 8^depth`:

| Depth | Full-cell units | Smallest unsigned storage before compression headroom |
|---|---:|---|
| 0 | 64 | u8 |
| 1 | 512 | u16 |
| 2 | 4,096 | u16 |
| 3 | 32,768 | u16 |
| 4 | 262,144 | u32 |

ADR 0007's original u8 at depths 0–1 / u16 at depths 2–4 is incompatible with these units. Keep the current eight-byte finest cell. Decide separate coarse storage and compression headroom before LOD implementation; use checked wide accumulators and a remainder ledger for mass/energy conversions. Reject unsupported depths rather than saturating away matter.

The target 32-byte header in ENGINE_CORE now specifies eight u32 words, explicit packing and a reserved final word. It is a design, not a Rust struct relying on implicit padding. Current chunk dirty bits and proposed per-brick sub-block bits are distinct. Layout, addressing, endian and round-trip validation are future gates before uploading such a header.

## 4. Memory and performance evidence

20,000 base bricks consume 81,920,000 bytes; adding motion uses 40,960,000 more. Seven byte fields and one two-byte item field add 92,160,000 bytes, for 215,040,000 bytes (205.078 MiB) total. This excludes mirror/readback history, pool slack, directories, meshes, textures and render targets. Plane sparsity must be measured; a 120 MB total is not justified.

Keep the accepted Iris Xe target from ADR 0016. A desktop resolution/cap profile is a development configuration, not Iris Xe emulation. A single scaled kernel ratio cannot establish whole-frame, shared-memory or driver behavior. Record estimates as estimates; hardware acceptance remains pending without a real target-device trace.

Report 60 fps as frame p95 <= 16.67 ms, with p99 and worst stalls separately; report 60 CA ticks/s independently from render fps. CPU and GPU costs overlap, so do not sum independent timings into an invented frame number. Record build, hardware, driver, power mode, seed, scale, route, settings, warm-up, duration, active cells, queued work and memory. Retain the local median-of-three/8% regression convention without equating it with target-device acceptance.

## 5. Deferred acceptance

The user requested no new or executed tests. Compile checks and source review do not establish CPU/GPU equivalence or speedup. When testing is requested, cover sleeping residency with a fixed tiny awake set, repeated wake, removal of first/middle/last indexed chunks, trim of the final dirty brick, failed fill/reserve, reuse, negative coordinates and canonical order; then run existing CPU/offload/GPU comparisons. Benchmark discovery separately from rule execution with resident size varied and awake work held constant. Do not claim a timing improvement until those measurements exist.
