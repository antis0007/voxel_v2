# Render arena, indirect draws and horizon evidence: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to carry out this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make terrain draw submission cheap and measurable (one shared GPU arena instead of one buffer pair per chunk, then multi-draw-indirect), and replace guesswork about the horizon with measured surface-agreement numbers.

**Architecture:** Chunk meshes move from per-chunk `wgpu::Buffer` pairs into paged vertex and index arenas. Each mesh owns an RAII lease whose drop returns its ranges through a one-frame quarantine. Draw sites bind once per page and draw with `first_index`/`base_vertex`, then optionally with `multi_draw_indexed_indirect`. Horizon agreement is measured by pure `engine_worldgen` tests before any redesign is proposed.

**Tech stack:** Rust 1.93.1, wgpu 27.0.1 (`crates/app/Cargo.toml:19`), PowerShell capture tools, criterion via `just bench` (`scripts/bench.py`).

**Spec:** the 2026-10-22 architecture review in this session, corrected by the code facts below. It sits under `docs/PLAN2.md` and its accepted ADRs 0007–0011. This plan does not change any of their decisions.

## Where this fits

| This plan | PLAN2 item | Status of the PLAN2 item at writing |
|---|---|---|
| Task 1 capture tool fix | none: every "before/after capture" step in PLAN2 needs it | — |
| Task 2 real-scene perf baseline | B5 overlay and B3 "p95 frame time" need a baseline | not started |
| Tasks 3–4 arena | B1 "pooled vertex/index arena replacing per-chunk buffers" (one line in PLAN2; specified here) | not started |
| Task 5 multi-draw indirect | B2(a) "draw_indexed_indirect args … multi-draw where supported" | not started |
| Task 6 horizon evidence | B4 "horizon backdrop stays as is": this measures whether that holds | not started |
| Task 7 predictive streaming | none: added 2026-10-22 from the Noita/architecture brief | not started |
| Appendix A lib-target facts | Phase 0.1 "library target" | in progress by another session |

Tasks 1–2 can run now. Tasks 3–5 are independent of Phase A (bricks), so the user may pull them ahead of PLAN2 order. Ask before doing that. Task 6 is pure worldgen and can run any time.

## Facts this plan relies on (verified 2026-10-22; re-check line numbers, the tree is being edited)

- `ChunkGpuMesh { boundary_profile, pub vertex_buffer, pub index_buffer, pub index_count: u32, pub lod: u8, lod_invariant, bytes }`: `render/streaming.rs:72-80`. Resident map: `ChunkCache.gpu_meshes: HashMap<IVec3, ChunkGpuMesh>` (`:112`). LOD cache: `streaming/lod_cache.rs:11` (64 MiB / 512 entries).
- Upload: `gpu_upload::start` (`streaming/gpu_upload.rs:32-136`) does one `create_buffer` per buffer (`:60`) inside three error scopes, then an async `Check`. Installs go through `install_mesh_with_casters` (`streaming.rs:850`). Stale-epoch and stale-revision rejection lives in `pending_gpu.rs:95-203`. Old buffers are dropped (no `destroy`, no completion fence) at `streaming/residency.rs:234,277`, `streaming/quality_proxies.rs:215`, `streaming.rs:641,680,809`, plus LOD-cache eviction.
- Empty meshes share `empty_vertex`/`empty_index` (`streaming.rs:1148-1159`). Placeholder proxies create 16-byte buffers (`streaming/edit_proxies.rs:49-50`). Tests build `ChunkGpuMesh` with real buffers (`streaming/pin_tests.rs:9,44,92,150`).
- Draw sites: main pass `render.rs:4354-4359` (`set_vertex_buffer`, `set_index_buffer(Uint32)`, `draw_indexed(0..n, 0, 0..1)` per chunk); shadows `render/shadow.rs:641-643`. `draw_list` is front-to-back (`streaming/visibility.rs:447-449`). Horizon pages (`render.rs:4368-4380`, `shadow.rs:649-651`) are a separate system and out of scope.
- Vertex `Vertex3` = 56 bytes (`mesh.rs:13-19`), absolute voxel-unit positions; indices `u32`. Water is in the same opaque mesh. There is no transparent sort.
- Device: `Limits::default()` (`render.rs:1560`, so `max_buffer_size` = 256 MiB), features `TIMESTAMP_QUERY | FLOAT32_FILTERABLE` only. In wgpu 27, `multi_draw_indexed_indirect` needs `DownlevelFlags::INDIRECT_EXECUTION` and no feature flag. `first_instance` stays 0, so `INDIRECT_FIRST_INSTANCE` is not needed.
- Counters: `RenderFrameStats.terrain_draw_calls`, `shadow_draw_calls`, `terrain_encode_ms`, `gpu_terrain_ms` (`render.rs:153-195`). `frames.csv` columns are written at `main.rs:1225`.
- Defaults: view 28 chunks, LOD start 10, 4 levels (`render.rs:63-65`). The meshed ring is 896 m at 1 m voxels; LOD0 is 320 m.
- Horizon: `distant_surface(cfg, x, z) -> (f32, BlockId)` = `sample_column(..).height + 1` (`crates/worldgen/src/lib.rs:447`). The fine grid samples it every 8 m inside ±1536 m (`render/horizon/fine.rs`). Rings beyond about 6 km use `landscape::preview_surface_lod`. Ownership is a per-column bitmap discard (`world_shader.wgsl:2546`), not only radial. No test compares horizon height with chunk surfaces.
- Capture tool bug: `tools/Review-VisualDirection.ps1` runs `Get-ChildItem -LiteralPath (Join-Path $repo 'src')`, but `src/` moved to `crates/` in the workspace split. With `$ErrorActionPreference = 'Stop'` every capture throws. Its `--benchmark-view-chunks=6` is too small for draw-cost measurement.

## Global constraints

- `CLAUDE.md` rules: no `unwrap`/`expect` outside tests and `main.rs` startup; no glob re-exports; no new file over 300 lines when it can be split; no scratch files in the repo root; no new dependency without an ADR line. Do not touch `egui_ui/wand_xmb.rs` or `adventure/growth.rs`.
- `.claude/rules/render.md`: a host struct and its WGSL declaration are one contract. Visual evidence comes from `tools/Review-VisualDirection.ps1` with a unique `-Label` (the tool refuses to overwrite). Speed gained by missing geometry is not an optimisation.
- No save, generator-version or scenario-hash change anywhere in this plan. If a scenario snapshot changes, stop: that is a regression.
- Gate: `just verify`, then `just task-verify` (`scripts/workflow.py`). Record manual capture checks with `python -I scripts/workflow.py attest`, and only after you have looked at the PNGs.
- Single writer: before each task, `git status --short` must show only this task's files. Another session is active on this tree. If its files are dirty, stop and ask; do not commit its files.
- Timing evidence: interleave A/B runs (A, B, A, B) and take the best per side. The capture tool refuses to run while `cargo`/`rustc` run. Disk is about 95% full: build release-fast once and do not create extra target dirs unless the exe is locked.
- ADR numbers: take the next free number at the time you write it (0006–0011 are taken or claimed).

## Review focus

1. **Freed arena range rewritten while an already-encoded draw still reads it.** `queue.write_buffer` lands at the start of the next submit, before that frame's draws, so the frame shows wrong geometry (flicker). Covered by `freed_range_not_reused_same_frame` in Task 4.
2. **Device loss or an epoch reset** (`streaming.rs:626,657`) leaves leases pointing into dead pages. Covered by the reset test in Task 4.
3. **Page churn breaks front-to-back order** if draws are regrouped by page, which increases overdraw. Task 4 keeps `draw_list` order and rebinds only when the page changes.
4. **Negative chunk coordinates in the horizon sampling grid** (floor division). Task 6 samples x, z < 0.
5. **Bench and capture numbers polluted by a concurrent build or a stale exe.** Task 1 restores the stale-exe check; Task 2 runs A/B interleaved.

---

### Task 1: Repair the capture tool and make view distance a parameter

**Files:** Modify `tools/Review-VisualDirection.ps1`.

- [ ] **Step 1: Reproduce.** Run `cargo build --profile release-fast`, then `powershell -File tools/Review-VisualDirection.ps1 -Label t1-repro -Seconds 10`. Expected: it throws on the missing `src` path. If it does not throw, someone fixed it; skip to Step 3.
- [ ] **Step 2: Fix the freshness check.** Use `Get-ChildItem -LiteralPath (Join-Path $repo 'crates') -Recurse -Include *.rs,*.wgsl`.
- [ ] **Step 3: Add `[int]$ViewChunks = 6`.** Pass it as `"--benchmark-view-chunks=$ViewChunks"`. The default stays 6, so existing evidence remains comparable.
- [ ] **Step 4: After a successful run, write `summary.txt` into the evidence dir** containing the `[benchmark]` lines the script already selects. `tools/Summarize-RendererReview.ps1` requires a `summary.txt` with `terrain_draws=`.
- [ ] **Step 5: Verify.** Run `-Label t1-check -Seconds 10`, then `-Label t1-check-28 -ViewChunks 28 -Seconds 10`. Expected: both print `Evidence: …` and `[benchmark] avg_fps=…`. `tools/Summarize-RendererReview.ps1 -Paths target/visual-direction/t1-check-28/daylight-2560-1440` prints frame p50/p95/p99.
- [ ] **Step 6: Commit** `fix(tools): capture tool reads crates/ after the workspace split`.

### Task 2: Real-scene render baseline (no code change)

**Files:** Append a "Baseline" table to this plan file.

- [ ] **Step 1: Run the baseline, interleaved.** Run `-Case daylight -ViewChunks 28 -Seconds 40` twice (labels `base-a1`, `base-a2`) and `-Case motion -ViewChunks 28 -Seconds 40` twice (labels `base-m1`, `base-m2`), in the order a1, m1, a2, m2.
- [ ] **Step 2: Summarise each run** with `tools/Summarize-RendererReview.ps1`. From each `summary.txt`, record frame p50/p95/p99, `gpu_terrain_mean_ms`, and `terrain_draws` and `shadow_draws` from the `[benchmark] latest_scene` line. Also record peak `gpu_resident_bytes` from `frames.csv`.
- [ ] **Step 3: Write the table** (best of the two runs per case), plus the adapter name from `stdout.log`, then commit `docs(plan): render baseline at view 28`.

These numbers are the acceptance baseline for Tasks 4 and 5.

**Baseline (2026-10-22, run order a1, m1, a2, m2).** RTX 4090, Vulkan, 2560×1440, view 28, LOD start 10, 4 levels. Evidence is under `target/visual-direction/base-*`.

| Run | Frame p50 ms | p95 | p99 | GPU terrain ms | render CPU p50 ms | terrain draws | shadow draws | peak GPU MiB |
|---|---|---|---|---|---|---|---|---|
| base-a1 daylight | 26.08 | 36.52 | 43.28 | 2.80 | 18.48 | 1267 | 9352 | 1892 |
| base-a2 daylight | 28.16 | 104.74 | 186.35 | 5.87 | 21.53 | 1133 | 4142 | 1765 |
| base-m1 motion | 29.62 | 115.08 | 184.05 | 15.16 | 23.30 | 809 | 4680 | 1140 |
| base-m2 motion | 28.99 | 36.76 | 53.04 | 12.33 | 22.52 | 1010 | 2655 | 1222 |

How to read it:
- **Render CPU dominates.** Render CPU p50 is 18–23 ms of a 26–30 ms frame, while terrain GPU time is 3–15 ms. Thousands of shadow draws plus about 1,000 terrain draws per frame make per-draw encode the bottleneck, which is what Tasks 4–5 target.
- **The runs are not clean.** Other sessions were building and editing the tree the whole time. The capture tool blocked up to 12 attempts per run, and the four exes come from four different tree states (`working_tree_sha256` in each `manifest.json`). So p95/p99 are noisy (37 ms against 105–115 ms within the same case), and the draw counts are `latest_scene`, the last frame only.
- **Use as reference, not as the gate.** Task 4 Step 5 re-runs interleaved baselines from the same tree as the arena build, and those are its gate.

### Task 3: `RangeAllocator` and `ArenaLease` (pure CPU, fully unit-tested)

**Files:** Create `crates/app/src/render/streaming/arena.rs` (≤ 300 lines; put tests in `arena_tests.rs` through `#[path]`, as `pending_gpu.rs:207` does). Modify `render/streaming.rs` to add `mod arena;`.

**Produces:**
- `pub(super) struct RangeAllocator`, with:
  - `new(capacity: u64, align: u64) -> Self`
  - `alloc(&mut self, size: u64) -> Option<u64>`: first fit over a `BTreeMap<u64, u64>` free list (offset → len); rounds `size` up to `align`.
  - `free(&mut self, offset: u64, size: u64)`: coalesces with both neighbours.
  - `used(&self) -> u64`, `largest_free(&self) -> u64`, `capacity(&self) -> u64`.
- `pub(super) struct ArenaRange { pub page: u32, pub vertex_offset: u64, pub vertex_bytes: u64, pub index_offset: u64, pub index_bytes: u64 }`, with derived `first_index(&self) -> u32` (`index_offset / 4`) and `base_vertex(&self) -> i32` (`vertex_offset / 56`, asserting divisibility).
- `pub(super) struct ArenaLease { range: ArenaRange, returns: std::sync::mpsc::Sender<ArenaRange> }`. Its `Drop` sends `range`; a send error (arena gone) is ignored. Accessor: `range(&self) -> &ArenaRange`.
**Reuse safety** comes from when the receiver is drained, not from a separate type. Task 4's `MeshArena::begin_frame` is the only place that drains it, and it runs after the previous frame's `queue.submit` and before any upload of the new frame. A range dropped at any point in frame N is therefore never rewritten while an unsubmitted frame-N command still reads it.

**Alignment:** vertex offsets must be multiples of 56 (one `Vertex3`, so `base_vertex` is exact) and of 4 (the `COPY_BUFFER_ALIGNMENT` that `write_buffer` needs); align 56 satisfies both. Index offsets use align 4.

- [ ] **Step 1: Write the failing tests** in `arena_tests.rs`:
  - `alloc_rounds_to_alignment`: `RangeAllocator::new(1024, 56)`; `alloc(1)` gives `Some(0)`, then `alloc(1)` gives `Some(56)`.
  - `free_coalesces_both_sides`: allocate a, b, c of 56 each; free a, then c, then b; `largest_free() == capacity()`.
  - `exhaustion_returns_none`: `new(112, 56)`; two `alloc(56)` succeed and the third returns `None`.
  - `fragmentation_first_fit_reuses_hole`: allocate 4 × 56, free the 2nd, `alloc(56)` returns the 2nd's offset.
  - `lease_drop_returns_range_once`: drop an `ArenaLease`; the receiver yields exactly that range and then `try_recv` errors.
  - `base_vertex_and_first_index`: `vertex_offset = 56 * 3` gives `base_vertex() == 3`; `index_offset = 4 * 5` gives `first_index() == 5`.
- [ ] **Step 2:** `just test arena`. Expected: compile failure, because the types don't exist yet.
- [ ] **Step 3: Implement** to the signatures above.
- [ ] **Step 4:** `just test arena`. Expected: 6 passed.
- [ ] **Step 5:** `just verify`, then commit `feat(render): range allocator and leases for a chunk mesh arena`.

### Task 4: Move chunk meshes into the arena

**Files:**
- Modify `render/streaming.rs`: `ChunkGpuMesh`, the install path, the epoch reset at `:626` and `:657`, and stats.
- Modify `render/streaming/gpu_upload.rs`, `render/streaming/arena.rs` (add `MeshArena`), `render.rs:4354-4359`, `render/shadow.rs:641-643`, `render/streaming/edit_proxies.rs:49-50` and `render/streaming/pin_tests.rs`.

**Consumes:** Task 3 types.

**Produces:**
- `pub(super) struct MeshArena` (in `arena.rs`, with `ArenaPage { vertices: wgpu::Buffer, indices: wgpu::Buffer, vertex_alloc: RangeAllocator, index_alloc: RangeAllocator }`):
  - `new(byte_budget: u64) -> Self` (pages created lazily)
  - `alloc(&mut self, device, vertex_bytes, index_bytes) -> Option<ArenaLease>`: tries existing pages, then a new page if `pages × PAGE_BYTES ≤ byte_budget`, else `None`.
  - `write(&self, queue, lease, mesh: &ChunkMesh)`: two `queue.write_buffer` calls.
  - `begin_frame(&mut self)`: drains the mpsc receiver and frees each range into its page's allocators.
  - `page(&self, i: u32) -> (&wgpu::Buffer, &wgpu::Buffer)`
  - `reset(&mut self)`: drops all pages and drains the receiver.
- `PAGE_VERTEX_BYTES = 128 MiB`, `PAGE_INDEX_BYTES = 16 MiB`. Indices run about 24 B per quad against 224 B of vertices, and both sizes fit `Limits::default().max_buffer_size`.
- `ChunkGpuMesh`: replace `vertex_buffer`/`index_buffer` with `pub alloc: Option<ArenaLease>`. `None` means an empty mesh or a test or placeholder mesh, so drop the 16-byte placeholder buffers and the shared `empty_*` buffers.
- Add `#[cfg(test)] fn ChunkGpuMesh::test_mesh(index_count: u32, lod: u8, bytes: usize) -> Self` for `pin_tests.rs`.
- Stats: add `arena_capacity_bytes` and `arena_largest_free_bytes` to `ChunkStreamerStats` next to `gpu_resident_bytes`.

**Rules the implementer must keep:**
- Keep `PendingGpuLedger` and its epoch, revision and budget rejection unchanged. `gpu_upload::start` allocates a lease, writes, and returns `Box::pin(std::future::ready(None))` as its `Check`. Rejection drops the pending entry, and the lease drop frees the range.
- Keep the old mesh installed until the replacement is installed. Do not change the move-into-LOD-cache behaviour at `streaming.rs:873-875`.
- `begin_frame` is called exactly once per frame, after the previous frame's `queue.submit` and before this frame's first upload. Nothing else drains the receiver (Review focus 1). A range freed in frame N is reusable from frame N + 1.
- Draw sites keep `draw_list` order. They call `set_vertex_buffer`/`set_index_buffer` only when `alloc.page` differs from the previous draw's page, then `draw_indexed(first_index..first_index + index_count, base_vertex, 0..1)`. Skip `alloc: None` draws. The shadow vertex layout (`shadow.rs:183`, stride 56) is unchanged.
- Arena alloc failure is a budget reject on the existing path, never a panic.

Steps:

- [ ] **Step 1: Write the failing tests** (in `arena_tests.rs`; these use a wgpu device the way `pin_tests.rs` does):
  - `arena_reset_invalidates_pages` (Review focus 2): allocate a lease, `reset()`, drop the lease (must not panic), then `alloc` succeeds on a fresh page.
  - `arena_respects_budget`: with `new(PAGE_VERTEX_BYTES + PAGE_INDEX_BYTES)`, the second page's alloc returns `None`.
  - `freed_range_not_reused_same_frame`: alloc A, drop A, `alloc` in the same frame without `begin_frame` returns a different offset; after `begin_frame()` A's offset is reused.
- [ ] **Step 2:** `just test arena`. Expected: the new tests fail.
- [ ] **Step 3: Implement** `MeshArena`, the `ChunkGpuMesh` change and the upload change. Then fix every compile error from the removed fields; the fact list above names all known sites.
- [ ] **Step 4:** `just test` (all), then `just test headless`. Expected: green, with no snapshot diffs (`git status crates/app/src/snapshots` is clean).
- [ ] **Step 5: Captures.** Build release-fast. Run `-Case daylight` and `-Case motion` at `-ViewChunks 28`, labels `arena-a1`/`arena-m1`, interleaved with fresh baseline runs `base-a3`/`base-m3`. Inspect the PNGs side by side. Expected: no holes, no flicker, no missing chunks in the `motion` sweep.
- [ ] **Step 6: Perf check against the Task 2 table.** Frame p95 must not regress by more than 5%, and `terrain_draws` must be equal within ±2% (the same visible set). Record the numbers under "Task 4 results" in this file. Then `python -I scripts/workflow.py attest` the capture inspection.
- [ ] **Step 7:** `just verify`, `just task-verify`, then commit `feat(render): chunk meshes live in a paged GPU arena`.

### Task 5: Multi-draw indirect per page (keep only if measured faster)

**Files:** Modify `render.rs` (main-pass draw loop and the device capability check), `render/shadow.rs:641-643`, and `main.rs` (flag parsing near `:439`). Create `render/streaming/indirect_draws.rs`.

**Produces:**
- `pub(super) struct IndirectDraws`, with:
  - `new(device, capacity: u32) -> Self`. Its buffer has usage `INDIRECT | COPY_DST` and is sized `capacity * 20` bytes. Initial capacity 16384, doubling on demand.
  - `build<'a>(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, draws: impl Iterator<Item = (&'a ArenaRange, u32)>) -> Vec<(u32, u64, u32)>`. Each draw is `(range, index_count)`; each returned run is `(page, byte offset, count)`. It writes `wgpu::util::DrawIndexedIndirectArgs { index_count, instance_count: 1, first_index, base_vertex, first_instance: 0 }` in `draw_list` order and returns runs of consecutive same-page draws.
- Enabled when `adapter.get_downlevel_capabilities().flags.contains(wgpu::DownlevelFlags::INDIRECT_EXECUTION)` and `--benchmark-no-multidraw` is absent. Otherwise use the Task 4 loop. Main and shadow passes each get their own `IndirectDraws`, because shadow cascades have different lists.

Steps:

- [ ] **Step 1: Write the failing unit test** `indirect_runs_split_on_page_change`: `split_runs(&[0,0,1,0]) == vec![(0,0,2), (1,2,1), (0,3,1)]` as `(page, first_draw, count)`, and `split_runs(&[])` is empty. The caller's byte offset is `first_draw * 20`. `split_runs(pages: &[u32]) -> Vec<(u32, u32, u32)>` is a pure fn, so the test needs no device.
- [ ] **Step 2:** `just test indirect`. Expected: FAIL. Then implement and re-run. Expected: PASS.
- [ ] **Step 3: A/B.** Run release-fast at `-ViewChunks 28`, with and without the flag. The capture tool has no flag passthrough, so add `[string[]]$ExtraArgs = @()` appended to `$argsList`. Interleave four runs per side (daylight case).
- [ ] **Step 4: Decision rule, fixed before measuring.** Keep multi-draw if the best `render_cpu_ms` p50 (from `frames.csv`) improves by ≥ 10% and frame p95 does not regress. Otherwise revert the draw-loop change and keep `IndirectDraws` only if PLAN2 B2 will consume it (ask). Record the numbers here.
- [ ] **Step 5:** Captures as in Task 4 Step 5 (labels `mdi-*`), `just verify`, `just task-verify`, then commit `feat(render): multi-draw indirect for chunk meshes` (or `docs(plan): multi-draw measured, not kept` with the numbers).

### Task 6: Measure horizon/chunk surface agreement, then write an ADR (proposed)

**Files:**
- Create `crates/worldgen/src/horizon_agreement.rs`, `#[cfg(test)]`, declared from `lib.rs` next to `gen_probe`.
- Create an ADR with the next free number, titled `NNNN-horizon-representation.md`, `Status: proposed`.

**Measures**, for Hills and Ridged, seed 424242, at `OneMeter` and `QuarterMeter`, over columns on a 2 m grid in the annulus 320 m ≤ r ≤ 1536 m around (0, 0) (metres to voxels via `cfg.voxel_meters()`; at QuarterMeter, 1536 m is 6144 voxels), with half the samples at x < 0 or z < 0:
- **(a) Source divergence.** `distant_surface(cfg, x, z).0` against the top non-Air cell + 1 found by scanning `sample_block(cfg, IVec3::new(x, y, z))` from `h + 48` down to `h - 48` in voxels. Report the share of columns with |Δ| = 0, p95 |Δ| and max |Δ| in voxels, plus the share where Δ is explained by trees or structures (the top block is Log/Leaves or above `h + 1`).
- **(b) Interpolation error** at knot steps 8, 16, 32 and 64 m (converted to voxels with `cfg.voxel_meters()`). Interpolate `distant_surface` at knots over the same two triangles the horizon lattice uses: read the triangulation in `render/horizon/fine.rs` and `render/horizon.rs` and copy its diagonal choice, and state which diagonal in the test doc comment. Report p50/p95/max |exact − interpolated| in voxels and metres.

Steps:

- [ ] **Step 1: Write an ignored print test** `horizon_agreement_report` that prints the table, then run `cargo test -p engine_worldgen horizon_agreement_report -- --ignored --nocapture --test-threads=1`.
- [ ] **Step 2: Add a non-ignored regression test** `horizon_agreement_stays_within_measured_bounds` that asserts each p95 at most the measured value × 1.1 (+0.5 voxel), for the 8 m step and source divergence only. Put the measured values in the test as constants, with the date. Use a small sample so it runs in under 2 s: 1 seed, OneMeter, 4096 columns.
- [ ] **Step 3: Write the ADR** with the table and a recommendation under this rule, fixed now:
  - If (b) p95 at 8 m ≤ 1 voxel and (a) is explained mostly (≥ 90%) by trees and structures, the recommendation is "keep the heightfield horizon; fix specific seams with captures".
  - Otherwise, open a design for coarse-mesh horizon tiles derived from `generate_padded`, as a separate plan.

  The decision is the user's; the ADR stays `proposed`.
- [ ] **Step 4:** `just verify`, then commit `test(worldgen): measure horizon surface agreement` plus the ADR.

A third measurement, the LOD3 `coarsened_mesh` top surface against the horizon fine grid at the 896 m handoff, needs the app library target (Appendix A). Add it after PLAN2 Phase 0.1 lands, as a test in `crates/app` using `world::generate_padded` (`crates/app/src/world.rs:1181`) and `mesh::coarsened_mesh(chunk, 3)` (`mesh.rs:714`).

### Task 7: Velocity-aware streaming priority (keep only if measured better)

**Today:** `pending_priority(center, key, urgent)` (`render/streaming/visibility.rs:180`) orders pending chunks by radial distance from the camera chunk only. `rebuild_pending_schedule` (`render/streaming.rs:1220`) re-sorts only when the centre chunk changes (`update_center_chunk_with_snapshot`, `:690`). So fast flight loads behind the player as eagerly as ahead of it, and a teleport arrives with an empty queue.

**Files:**
- Modify `render/streaming/visibility.rs` (`pending_priority` gains a `lead: IVec3` argument).
- Modify `render/streaming.rs`: store `lead` beside `pending_schedule_center` and pass it at both call sites.
- Modify `main.rs` to compute `lead` from the camera velocity.

**Rule:**
- `lead` = camera velocity × `LEAD_SECONDS` (0.75 s), converted to chunks and clamped to ±4 chunks per axis.
- Priority distance is measured from `center + lead`; `dy`, LOD and the tie-breakers are unchanged.
- `urgent` keeps its absolute precedence. The current centre chunk and its 26 neighbours always rank above any lead target, so collision near the player never waits behind look-ahead.
- Rebuild the schedule when `center + lead` changes, not only when `center` changes.
- A teleport (a jump of more than 4 chunks in one frame) sets `lead` to zero for that rebuild.

Steps:
- [ ] **Step 1: Failing unit tests** in `visibility.rs`:
  - `lead_zero_matches_radial_order`: with `lead = 0`, order is identical to today's for a fixed key set (negative coordinates included).
  - `lead_prefers_chunks_ahead`: two keys at equal distance from `center`, one ahead and one behind; the ahead key ranks first.
  - `neighbours_outrank_lead`: a key at `center + (1,0,0)` outranks a key at `center + lead` when lead points the other way.
- [ ] **Step 2:** `just test lead_`. Expected: FAIL, then implement, then PASS.
- [ ] **Step 3: Measure.** Use a high-speed flight route across 2 km, `-Case motion -ViewChunks 28`, with `--benchmark-no-stream-lead` for the A side (add the flag, using the Task 5 `ExtraArgs` passthrough). Interleave 4 runs per side. Record frame p95/p99 and the count of frames where a visible chunk had no resident mesh or proxy (`RenderFrameStats`; add a `visible_missing` counter if none exists).
- [ ] **Step 4: Decision rule, fixed before measuring.** Keep it if `visible_missing` drops by ≥ 25% and p99 does not regress by more than 5%. Otherwise revert and record the numbers here.
- [ ] **Step 5:** Captures (labels `lead-*`), `just verify`, `just task-verify`, then commit `feat(render): stream ahead of camera velocity`.

## Not planned, with reasons

- **Uniform-chunk storage shortcut.** Only edited chunks become `ChunkData` (`World::get` falls back to `sample_cache`/`sample_block`, `world.rs:1495-1510`). The edit budget is 64 MiB (≈ 1,008 chunks, `world/regions.rs:17,902`), regions are already RLE (`regions.rs:247-363`), and render already skips uniform padded volumes (`render/streaming.rs:1713-1719`). `Arc::ptr_eq` change detection (`render/horizon/edits.rs:665,762,1097`) also forbids a shared uniform singleton.
- **64-bit world coordinates.** i32 voxels at 0.25 m reach ±537,000 km.
- **Transvoxel / dual contouring.** Terrain is cubic.
- **Structural collapse of unsupported builds.** `simulation/rigid.rs:24-80` captures only `RigidCore`-connected bodies, with no support check, and ADR 0011 keeps rigid bodies on the CPU. Adding collapse changes gameplay: it is a design question for the user (anchors, which materials, warning cues, dungeon safety), not an engineering task.

## Appendix A: facts for PLAN2 Phase 0.1 (app library target)

- `main.rs:1-45` declares 26 modules plus three aliases that other modules use through `crate::`: `use engine_audio as audio;` (`:16`), `use engine_core::fast_hash;` (`:19`), `use engine_content as material;` (`:22`). `lib.rs` must declare the same aliases.
- `main.rs` contains no `crate::` paths. In the bin, replace each `mod x;` with `use minecraft_style_voxel_prototype::x;`: imported names resolve under `crate::x`, so the code inside `main.rs` compiles unchanged.
- There are 126 `pub(crate)` items in 35 files. Only those the bin uses need `pub`; the compiler lists them.
- Insta snapshot names derive from the module path (`minecraft_style_voxel_prototype__headless__tests__*.snap`). The lib's default name equals the package's, so names are unchanged. Check that no new `.snap` appears.
- `#[path]` uses (`render/capture.rs:8`, `render/coverage.rs:479`, `render/shadow.rs:7`, `render/streaming/pending_gpu.rs:207`, `simulation/cellular.rs:10`, `build.rs:2`) are file-relative and unaffected.
- Headless closure (no wgpu/egui): `world`, `mesh`, `simulation`, `fluid`, `player` (uses `winit::keyboard::KeyCode`, `player.rs:2,57`), `tool_effects`, `wand_style`.
- Mesh bench entry points once public: `MeshPlan::for_chunk(&GeneratedChunk, &AtomicBool) -> Option<MeshPlan>` and `MeshPlan::build_lod(&self, &GeneratedChunk, lod: u8) -> ChunkMesh` (`mesh/memory.rs:72,88`), and `coarsened_mesh(&GeneratedChunk, lod: u8) -> ChunkMesh` (`mesh.rs:714`). There is an ignored timing test to convert: `mesh.rs:1806-1834` `lod_speed::coarsened_mesh_cost`.
