# Space phase 0: camera-relative chunk rendering, implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Chunk geometry and shadows render without precision loss at any world
coordinate, with no visible change near the origin.

**Architecture:** An integer render origin `O` (eye snapped to a 4096-voxel
grid) replaces the absolute world as the frame for chunk geometry. Arena
vertices become chunk-local at upload; each draw reads its chunk's exact offset
`chunk * 32 - O` from an instance-step vertex buffer selected by
`first_instance`. The camera uniform carries `O` and the f64-exact eye position
relative to `O`; shadow cascades and receivers move to the same frame, with
the cascade texel grid snapped in f64 so it stays world-locked when `O` moves.

**Tech Stack:** Rust, wgpu 27.0.1, WGSL (naga-validated by `crates/app/build.rs`), glam.

**Spec:** [space track design](2026-10-24-soa-space-port-design.md) section 4.2
and phase 0 of section 6; [decision 0025](../decisions/0025-space-track-soa-port.md)
(phase 0 now; per-draw transform carries offset and rotation, rotation is
identity on flat worlds).

## Global Constraints

- Phase 0 exit: capture at x = 1,000,000 m shows no jitter; perf within `just bench-check`.
- Per-draw transform is offset plus rotation from the start; this plan ships
  the offset and reserves the rotation (see Task 2 layout), identity on flat worlds.
- Flat worlds keep generating byte-identically; no save or generator change.
- Host struct and WGSL declaration change in the same patch (`.claude/rules/render.md`).
- No `unwrap`/`expect` outside tests; no new dependency; no `Instant` in simulation.
- New files under 300 lines; new logic goes in `crates/app/src/render/origin.rs`,
  not in `render.rs` (4.8k lines, debt).
- Visual change evidence: before/after capture from the same pose, unique
  label, PNGs inspected (`docs/design/VISUAL_DIRECTION_EXECUTION.md`).
- Every `.wgsl` stays listed in `crates/app/src/shader_sources.rs`.
- Scope: arena chunk meshes and shadows. Scene-stream models (`mesh/models.rs`
  producers) stay absolute via slot 0: they are built as absolute f32 on the
  CPU, so subtracting `O` afterwards cannot recover precision; making them
  origin-aware is a follow-up task, recorded in the design doc phase 0 row
  (Task 5). Viewmodel geometry is already camera-relative.
- Shared tree: work on a branch or worktree; commit only this plan's files
  (pathspec commit), never a sweep of the index.

## Review Focus

1. Camera crossing a 4096-voxel origin cell boundary while moving: no one-frame
   jump of geometry or shadow edges (Task 3 hysteresis test, Task 4
   world-locked texel test).
2. Negative and very large chunk coordinates (x = -900,032, z = 31,250 chunks):
   offsets exact, floor division correct (Task 1 tests).
3. Adapters without `INDIRECT_FIRST_INSTANCE`: draws fall back to direct
   `draw_indexed` with the right offsets, not all at slot 0 (Task 2 test).
4. Horizon, scene stream and viewmodel geometry, which stay absolute or
   camera-relative: unchanged on screen near and far (Task 3 slot-0 rule and
   capture).
5. Animated vertices (grass tufts, flames, embers, water waves) far from the
   origin: displacement stays smooth, not stepped at the f32 grid of the
   absolute position (Task 3 delta accumulation).

---

### Task 1: Render origin math

**Files:**
- Create: `crates/app/src/render/origin.rs` (declare `mod origin;` in `crates/app/src/render.rs` next to the other `render/` modules)
- Test: same file, `#[cfg(test)] mod tests`

**Interfaces:**
- Produces:
  - `pub const ORIGIN_GRID_VOXELS: i32 = 4096;`
  - `pub const ABSOLUTE_SLOT: u32 = 0;`
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub struct RenderOrigin { pub voxel: IVec3 }`
  - `impl RenderOrigin { pub fn follow(current: Option<RenderOrigin>, eye_voxels: DVec3) -> RenderOrigin; pub fn chunk_offset(self, chunk: IVec3) -> [f32; 4]; pub fn absolute_offset(self) -> [f32; 4]; pub fn eye_from_origin(self, eye_voxels: DVec3) -> Vec3; }`

Rules: `follow` returns `current` unchanged while every axis of
`eye_voxels - current.voxel` lies in `[-ORIGIN_GRID_VOXELS, 2 * ORIGIN_GRID_VOXELS)`;
otherwise (or for `None`) it snaps each axis to
`floor_div(floor(eye), ORIGIN_GRID_VOXELS) * ORIGIN_GRID_VOXELS` (use
`engine_core::floor_div`). `chunk_offset` is
`(chunk * CHUNK_SIZE - voxel)` as f32 in xyz, `w = 0.0`. `absolute_offset` is
`-voxel` as f32, `w = 0.0`. `eye_from_origin` is `(eye_voxels - voxel.as_dvec3()).as_vec3()`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn origin_snaps_with_floor_division() {
    let o = RenderOrigin::follow(None, DVec3::new(-0.5, 10.0, 900_032.25));
    assert_eq!(o.voxel, IVec3::new(-4096, 0, 897_024));
}

#[test]
fn origin_holds_inside_hysteresis_band() {
    let o = RenderOrigin::follow(None, DVec3::new(100.0, 0.0, 0.0));
    for x in [-4095.0, 0.0, 4096.0, 8191.0] {
        assert_eq!(RenderOrigin::follow(Some(o), DVec3::new(x, 0.0, 0.0)), o);
    }
    assert_ne!(RenderOrigin::follow(Some(o), DVec3::new(8192.0, 0.0, 0.0)), o);
    assert_ne!(RenderOrigin::follow(Some(o), DVec3::new(-4097.0, 0.0, 0.0)), o);
}

#[test]
fn chunk_offsets_are_exact_far_from_spawn() {
    let o = RenderOrigin { voxel: IVec3::new(897_024, 0, -1_003_520) };
    let chunk = IVec3::new(28_126, -2, -31_336);
    let off = o.chunk_offset(chunk);
    assert_eq!(off, [3_008.0, -64.0, 768.0, 0.0]);
    assert_eq!(o.absolute_offset(), [-897_024.0, 0.0, 1_003_520.0, 0.0]);
}

#[test]
fn eye_from_origin_keeps_sub_millimetre_precision() {
    let o = RenderOrigin { voxel: IVec3::new(897_024, 0, 0) };
    let eye = DVec3::new(900_031.999_7, 101.25, 0.5);
    let rel = o.eye_from_origin(eye);
    assert!((rel.x as f64 - 3_007.999_7).abs() < 1e-3);
}
```

- [ ] **Step 2: Run, expect FAIL (module missing)**

Run: `just test render::origin`
Expected: compile error, `RenderOrigin` not found.

- [ ] **Step 3: Implement `origin.rs` per the Interfaces block and rules above**

- [ ] **Step 4: Run, expect PASS**

Run: `just test render::origin`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/render/origin.rs crates/app/src/render.rs
git commit -m "render: integer render origin for camera-relative chunk drawing" -- crates/app/src/render/origin.rs crates/app/src/render.rs
```

### Task 2: Per-draw offset buffer plumbed through every chunk draw (no visual change)

All offsets are zero in this task, so the picture must stay identical. It adds
the buffer, the vertex layout, the WGSL input and the instance-index plumbing.

**Files:**
- Modify: `crates/app/src/render/origin.rs` (add `DrawOffsets`)
- Modify: `crates/app/src/render.rs` (device features ~1678; world pipeline vertex buffers ~2748; main pass draws ~4530-4580; prepare ~4473)
- Modify: `crates/app/src/render/streaming/indirect_draws.rs` (`set_supported`, `prepare`, `draw`)
- Modify: `crates/app/src/render/shadow.rs` (pipeline ~186, draws ~657-680)
- Modify: `crates/app/src/shaders/world_shader.wgsl` (`VsIn` line 17, `vs_main`)
- Modify: `crates/app/src/shaders/shadow_shader.wgsl` (`VsIn` line 17, `vs_main`)
- Modify: `crates/app/src/render/shadow_contract.rs` and any other test that draws with these pipelines (bind a one-entry zero buffer at slot 1)
- Test: `crates/app/src/render/origin.rs` (CPU), new `gpu_draw_offset_first_instance` in `crates/app/src/render/origin_gpu_tests.rs` (declared from `origin.rs` as `#[cfg(test)] mod origin_gpu_tests;` with `#[path]` if needed)

**Interfaces:**
- Consumes: `RenderOrigin`, `ABSOLUTE_SLOT` (Task 1).
- Produces:
  - `#[repr(C)] #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)] pub struct DrawTransform { pub offset: [f32; 4], pub rotation: [f32; 4] }`
    (`rotation` is a unit quaternion `xyzw`, always `[0, 0, 0, 1]` in phase 0; reserved for model R's bent chunks per 0025).
  - `pub struct DrawOffsets` with `pub fn begin_frame(&mut self, absolute: DrawTransform)` (slot 0 = `absolute`),
    `pub fn slot(&mut self, chunk: IVec3, transform: impl FnOnce() -> DrawTransform) -> u32`
    (dedupes by chunk within a frame, so the main pass and every shadow cascade share one slot per chunk),
    `pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue)`, `pub fn buffer(&self) -> Option<&wgpu::Buffer>`.
  - Vertex buffer slot 1 layout, shared by world and shadow pipelines:
    `array_stride: 32, step_mode: Instance, attributes: [5 => Float32x4 (offset), 6 => Float32x4 (rotation)]`.
  - WGSL: `VsIn` gains `@location(5) draw_offset: vec4<f32>, @location(6) draw_rotation: vec4<f32>` in both shaders.
  - `indirect_draws::set_supported(adapter, device_features)`: multidraw is active only when `INDIRECT_EXECUTION` and `Features::INDIRECT_FIRST_INSTANCE` are both present; `IndirectDraws::prepare` takes a `slot_of: impl FnMut(IVec3) -> u32` and writes it to `first_instance`.

Rules: request `Features::INDIRECT_FIRST_INSTANCE` when the adapter has it
(add to `requested_features`). Direct draws pass `slot..slot + 1` as the
instance range. Horizon and scene-stream draws keep `0..1` (slot 0). Upload
`DrawOffsets` after every draw list (main and shadow) is prepared and before
the first pass that binds it; never resize it after a pass has bound it.
In WGSL, phase 0 applies only the offset: `model_pos = in.pos + in.draw_offset.xyz`
in both shaders; `draw_rotation` is read nowhere yet (keep it in `VsIn` so the
layout contract is fixed).

- [ ] **Step 1: Write the failing CPU test**

```rust
#[test]
fn draw_offsets_dedupe_chunks_and_reserve_slot_zero() {
    let mut d = DrawOffsets::default();
    d.begin_frame(DrawTransform::IDENTITY);
    let a = d.slot(IVec3::new(1, 0, 0), || DrawTransform::IDENTITY);
    let b = d.slot(IVec3::new(-1, 0, 0), || DrawTransform::IDENTITY);
    assert_eq!((a, b), (1, 2));
    assert_eq!(d.slot(IVec3::new(1, 0, 0), || panic!("recomputed")), 1);
    d.begin_frame(DrawTransform::IDENTITY);
    assert_eq!(d.slot(IVec3::new(-1, 0, 0), || DrawTransform::IDENTITY), 1);
}
```

(`DrawTransform::IDENTITY` = offset zero, rotation `[0, 0, 0, 1]`.)

- [ ] **Step 2: Run, expect FAIL**

Run: `just test draw_offsets_`
Expected: compile error, `DrawOffsets` not found.

- [ ] **Step 3: Write the GPU test `gpu_draw_offset_first_instance`**

`#[ignore]` like `shadow_contract.rs`, using `engine_gpu::test_adapter()` and
`crate::shader_sources::SHADOW_SHADER` with `ShadowRenderer`'s pipeline: one
quad mesh at the origin, offset table `[zero, (+8,0,0), (-8,0,0)]`, two draws
with `first_instance` 1 and 2, run twice: always as direct draws
(instances `1..2`, `2..3`), and additionally through `multi_draw_indexed_indirect`
when the adapter has `INDIRECT_FIRST_INSTANCE`; 32 x 8 depth
target, orthographic view over x in [-16, 16]. Assert depth is written at the
texel columns covering x = +8 and x = -8 and not at x = 0.

- [ ] **Step 4: Implement `DrawOffsets`, layouts, WGSL inputs and draw plumbing per Interfaces and Rules**

- [ ] **Step 5: Run CPU and GPU tests, expect PASS**

Run: `just test draw_offsets_` then `just test-gpu draw_offset_first_instance` and `just test-gpu far_shadow`
Expected: all pass (the existing shadow contract still passes with the zero buffer bound).

- [ ] **Step 6: Capture A/B, expect identical images**

Run the capture tool at one fixed daytime pose with labels
`phase0-t2-before` / `phase0-t2-after`, and `--benchmark-sandbox-review`
(shot `30-floating-origin`) before and after. Expected: no visible difference.

- [ ] **Step 7: `just verify`, then commit the files listed above with a pathspec commit**

### Task 3: Main pass in the origin frame

Chunk vertices become chunk-local at upload, the camera uniform carries the
origin, and the world shader computes the clip position from small exact
numbers. The shadow pass keeps today's absolute frame in this task by drawing
with an absolute offset table (Task 4 moves it).

**Files:**
- Modify: `crates/app/src/render/origin.rs` (absolute twin table, see Rules)
- Modify: `crates/app/src/render/streaming/gpu_upload.rs` (`start`: write localized vertices)
- Modify: `crates/app/src/render.rs` (`CameraUniform` ~802 and its initializer ~2014; `upload_camera` ~3844)
- Modify: `crates/app/src/shaders/camera_lighting_shared.wgsl` (`Camera` struct, the only WGSL copy)
- Modify: `crates/app/src/shaders/terrain_detail_shared.wgsl` (add `tuft_wind_offset`)
- Modify: `crates/app/src/shaders/world_shader.wgsl` (`vs_main` 297-470)
- Test: `crates/app/src/render/streaming/gpu_upload.rs` (CPU), `crates/app/src/render/origin_gpu_tests.rs` (GPU)

**Interfaces:**
- Consumes: `RenderOrigin::{follow, chunk_offset, absolute_offset, eye_from_origin}`, `DrawOffsets`, `DrawTransform` (Tasks 1-2).
- Produces:
  - `pub(super) fn localize_into(dst: &mut [u8], vertices: &[Vertex3], chunk: IVec3)` in `gpu_upload.rs`: writes `vertices` as bytes with `pos -= chunk * CHUNK_SIZE` (as f32), all other fields unchanged; `dst.len() == size_of_val(vertices)`.
  - `CameraUniform` gains, appended after `resident_coverage`: `render_origin: [i32; 4]` (xyz = `O`, w = 0) and `eye_from_origin: [f32; 4]` (xyz metres = `eye_from_origin(eye_voxels) * world_scale`, w = 0). WGSL `Camera` gains `render_origin: vec4<i32>, eye_from_origin: vec4<f32>` in the same order.
  - `fn tuft_wind_offset(p: vec3<f32>, height_m: f32, scale: f32, time: f32, wind: vec2<f32>) -> vec3<f32>` with `tuft_wind_position(p, ...) = p + tuft_wind_offset(p, ...)`.
  - `DrawOffsets::slot_with_twin(&mut self, chunk: IVec3, primary: impl FnOnce() -> DrawTransform, twin: impl FnOnce() -> DrawTransform) -> u32` and `DrawOffsets::twin_buffer(&self) -> Option<&wgpu::Buffer>`: a second table in the same slot order (slot 0 of the twin = zero offset). Task 4 deletes both.
  - Renderer field `render_origin: Option<RenderOrigin>`, updated in `upload_camera` with `RenderOrigin::follow(self.render_origin, precise_eye / world_scale as f64)`.

Rules:
- Main-pass chunk slots carry `chunk_offset(coord)`; slot 0 carries `absolute_offset()`.
- The shadow pass in this task binds a second buffer filled in the same slot
  order with today's absolute frame: chunk slots `(coord * CHUNK_SIZE)` as f32,
  slot 0 zero. Same slot numbers, different buffer.
- World shader vertex stage, in voxel units unless named `_m`:
  - `local = in.pos + in.draw_offset.xyz` (origin-relative position; exact for chunks and for slot-0 absolute vertices, whose subtraction of `O` is exact)
  - `absolute = vec3<f32>(cam.render_origin.xyz) + local`: replaces every current use of `in.pos` as an absolute position (tuft phase, flame phase, `voxel_pos` output, water wave lattice), so near-origin values are bit-identical to today.
  - Displacements accumulate as deltas instead of mutating positions: `disp` (voxel units: tuft offset, flame sway, ember motion) and `disp_m` (metres: water waves, breakers, backdrop sink).
  - `world_pos = (absolute + disp) * scale + disp_m` keeps every absolute-space computation (ownership weights, coverage, fog distance) as today.
  - `vertex_relative = (local + disp) * scale + disp_m - cam.eye_from_origin.xyz` for non-viewmodel vertices; viewmodel (`CAMERA_RELATIVE_FLAG`) vertices keep `world_pos` as today.
  - Embers today set `model_pos = in.pos + ...`; express that as `disp = (drift + rise) / scale + rest * (grow - 1.0)` relative to `absolute`.
- `camera_pos` in the uniform stays absolute (clouds, sky, weather read it).

- [ ] **Step 1: Write the failing CPU test**

```rust
#[test]
fn localize_into_subtracts_chunk_origin_exactly() {
    let v = |p: [f32; 3]| Vertex3 { pos: p, color: [0.1; 3], normal: [0.0, 1.0, 0.0], surface: [1.0, 2.0, 3.0, 4.0], ao_strength: 0.5 };
    let src = [v([-96.0, 0.5, 1_000_032.0]), v([-64.0, 33.0, 1_000_000.25])];
    let mut dst = vec![0u8; std::mem::size_of_val(&src)];
    localize_into(&mut dst, &src, IVec3::new(-3, 0, 31_250));
    let out: &[Vertex3] = bytemuck::cast_slice(&dst);
    assert_eq!(out[0].pos, [0.0, 0.5, 32.0]);
    assert_eq!(out[1].pos, [32.0, 33.0, 0.25]);
    assert_eq!(out[1].surface, src[1].surface);
}
```

- [ ] **Step 2: Run, expect FAIL**

Run: `just test localize_into_`
Expected: compile error, `localize_into` not found.

- [ ] **Step 3: Write the GPU test `gpu_far_chunk_matches_near_chunk`**

Use the world shader through the smallest pipeline setup the existing
contracts use (follow `directional_facet_contract.rs`). Render one 32 x 32 quad
twice into a 64 x 64 depth target: once as chunk (0, 0, 0) with eye (16, 10, 16)
and `O = (0,0,0)`; once as chunk (28_126, 0, 0) with eye (900_048, 10, 16) and
`O = RenderOrigin::follow(None, eye)`. Assert both depth images are equal within 1e-6.
Add a third render with today's absolute path (offset zero, absolute vertices,
eye at 900_048) only as a diagnostic print of the max difference, not an
assertion.

- [ ] **Step 4: Implement upload, uniform, origin follow, shader and dual-table rules**

`gpu_upload::start` writes `localize_into` straight into the
`write_buffer_with` staging view, so no intermediate `Vec` is allocated.

- [ ] **Step 5: Run tests, expect PASS**

Run: `just test localize_into_`, `just test render::origin`, `just test-gpu far_chunk_matches`, `just test-gpu far_shadow`
Expected: all pass.

- [ ] **Step 6: Captures**

Same fixed pose as Task 2 with labels `phase0-t3-before`/`-after`: identical.
`--benchmark-sandbox-review` shot `30-floating-origin` before/after: the after
image shows no cracks or wobble at block edges; the scenario log line
`[sandbox-review] floating_origin=... sub_ulp_steps=...` still reports success.

- [ ] **Step 7: `just verify` (bench-check within gate), then pathspec commit**

### Task 4: Shadows in the origin frame

**Files:**
- Modify: `crates/app/src/render/shadow.rs` (cascade build ~380-420; uniform `ShadowGenUniform` ~50; drop the absolute twin table use)
- Modify: `crates/app/src/render/origin.rs` (remove the absolute twin table)
- Modify: `crates/app/src/shaders/shadow_shader.wgsl` (`ShadowGenUniform`, `vs_main`)
- Modify: `crates/app/src/shaders/world_shader.wgsl` (receiver lookups at ~803, ~828, ~2088, ~2736)
- Modify: `crates/app/src/shaders/local_mist_shared.wgsl` (cascade sampling, if it projects a world position)
- Test: `crates/app/src/render/shadow.rs` tests (CPU), `crates/app/src/render/shadow_contract.rs` (GPU)

**Interfaces:**
- Consumes: `RenderOrigin`, `DrawOffsets` chunk slots with `chunk_offset` (Tasks 1-3).
- Produces:
  - Cascade `light_view_proj` maps origin-relative metres (`(absolute_voxel - O) * scale`) to clip space.
  - `ShadowGenUniform` gains `render_origin: [i32; 4]` (WGSL `vec4<i32>`), appended at the end in both declarations.
  - `fn world_locked_texel_offset(view_rel: DMat4, origin_m: DVec3, texel_size: f64) -> DVec2` in `shadow.rs`: light-space position of absolute world zero (`view_rel.transform_point3(-origin_m)`), taken modulo `texel_size` and rounded to the nearest texel, as today's `offset_x`/`offset_y` but in f64.

Rules:
- Cascade centre and light eye are origin-relative (`center_m - O * scale`), built in f64 and cast after subtraction.
- The ortho window uses `world_locked_texel_offset`, so the texel grid of every cascade is fixed to the world when `O` changes.
- Shadow vertex stage: `clip = light_view_proj * vec4((in.pos + in.draw_offset.xyz + tuft_offset) * world_scale, 1)`. Shadow ownership (`world_xz`) uses the absolute reconstruction `(vec3<f32>(render_origin.xyz) + local) * world_scale`, as today's precision.
- Horizon parent draws in the shadow pass use slot 0 (`absolute_offset`).
- Receivers: every cascade projection in the world shader (and local mist) uses `cam.eye_from_origin.xyz + relative_pos` instead of an absolute world position.
- When `O` changes, mark every cascade pending and urgent, as an existing cascade refresh does.

- [ ] **Step 1: Write the failing CPU test**

```rust
#[test]
fn cascade_texel_grid_is_world_locked_across_origin_moves() {
    // Same absolute camera and sun; origins one grid cell apart.
    let scale = 1.0;
    let eye = DVec3::new(900_040.0, 120.0, -77.0);
    let a = RenderOrigin { voxel: IVec3::new(897_024, 0, -4096) };
    let b = RenderOrigin { voxel: IVec3::new(901_120, 0, -4096) };
    let world_point = DVec3::new(900_050.5, 100.0, -70.25);
    let ta = cascade_texel_of(a, eye, world_point, scale); // test helper: builds cascade 0, projects the point, returns texel coords (f64)
    let tb = cascade_texel_of(b, eye, world_point, scale);
    assert!((ta - tb).abs().max_element() < 1e-3);
}
```

`cascade_texel_of` is a test-only helper in `shadow.rs` tests that calls the
same cascade-building function the renderer uses.

- [ ] **Step 2: Run, expect FAIL**

Run: `just test cascade_texel_grid_`
Expected: FAIL (texel phase depends on the origin) or compile error for the helper.

- [ ] **Step 3: Implement per Interfaces and Rules; update `shadow_contract.rs` to bind the offset buffer and the new uniform field**

- [ ] **Step 4: Run tests, expect PASS**

Run: `just test cascade_texel_grid_`, `just test shadow`, `just test-gpu far_shadow`, `just test-gpu far_chunk_matches`
Expected: all pass.

- [ ] **Step 5: Captures**

Fixed daytime pose with long shadows, labels `phase0-t4-before`/`-after`:
identical near spawn. `--benchmark-sandbox-review` shot `30-floating-origin`:
shadow edges are sharp and do not shimmer between consecutive frames of the
scenario's sub-ulp steps.

- [ ] **Step 6: `just verify`, then pathspec commit**

### Task 5: Phase 0 evidence and status

**Files:**
- Modify: `docs/plans/2026-10-24-soa-space-port-design.md` (phase 0 row: add "done" with commit ids and capture labels)
- Modify: `docs/ARCHITECTURE.md` only if it describes chunk vertex coordinates (grep "absolute" / "Vertex3" first; skip if nothing matches)

- [ ] **Step 1: Run `just bench-check` and record the result line in the design doc phase 0 row**
- [ ] **Step 2: Record capture labels from Tasks 2-4 and the sandbox-review log line**
- [ ] **Step 2b: Add the follow-up row "phase 0b: origin-aware scene-stream models (`mesh/models.rs` producers)" to the design doc phase table**
- [ ] **Step 3: `just session-report`, then pathspec commit of the doc changes**

Commit hold from the design doc applies to the design doc itself: commit it
only after the `CLAUDE.md` generator-version drift has an owner; if it still
has none, leave the doc edit uncommitted and say so in the report.
