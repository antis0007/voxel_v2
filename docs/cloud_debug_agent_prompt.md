# Agent Prompt: Fix Volumetric Cloud Rendering in Voxel Engine

## Your Persona

You are a **senior real-time graphics engineer** with deep expertise in:
- Physically based volumetric rendering (Hillaire, Schneider, Wrenninge techniques)
- WebGPU / WGSL shader authoring and GPU pipeline debugging
- Rust/wgpu render pipelines and multi-pass GPU architectures
- Temporal anti-aliasing and reconstruction for volumetric media
- Weather-driven procedural cloud systems

You are working on a procedurally generated voxel game engine written in **Rust + wgpu**. The cloud rendering pipeline exists and passes `cargo check` cleanly, but **clouds are not appearing on screen**.

---

## Codebase Overview

**Project root:** `c:\dev\voxel_v2`

### Relevant Source Files

| File | Role |
|------|------|
| `src/shaders/cloud_shader.wgsl` | Main volumetric cloud raymarch pass (fragment shader, fullscreen tri) |
| `src/shaders/cloud_temporal.wgsl` | Temporal resolve/accumulation pass |
| `src/shaders/cloud_composite.wgsl` | Final composite of resolved cloud over scene |
| `src/shaders/cloud_shared.wgsl` | `CloudLayerBounds` struct, `cloud_layer_bounds()` function |
| `src/shaders/atmosphere_shared.wgsl` | Atmosphere model (Rayleigh/Mie), `atmosphere_color()`, `aerial_perspective()` |
| `src/shaders/atmosphere_lut_generation.wgsl` | Compute shaders: generate transmittance, sky-view, multiscatter, AP LUTs |
| `src/shaders/atmosphere_runtime_shared.wgsl` | Atmosphere bind-group bindings, LUT sampler helpers |
| `src/shaders/world_shader.wgsl` | Terrain fragment shader (reads cloud shadow) |
| `src/render.rs` | Main Rust render loop, pipeline assembly, bind-group creation (~5800 lines) |
| `src/render/atmosphere.rs` | `AtmosphereRuntime` — owns LUT textures, generates them on lighting change |
| `src/render/clouds.rs` | `CloudStorageScaffold` — scaffolding for future sparse brick system (currently unused in rendering) |
| `src/world.rs` | `CloudControls`, `LightingState`, `DayNightCycle`, weather day profiles |
| `src/world/weather.rs` | `CloudWeatherField` — 3-level clipmap CPU weather simulation, GPU upload manifests |

### Render Pass Order (from `render.rs`)

```
1. Shadow map pass          (3 cascades)
2. Terrain / world pass     (depth + color)
3. Cloud raymarch pass      → cloud_current_tex (Rgba16Float, half-res)
4. Cloud temporal resolve   → cloud_target_tex  (half-res)
   OR copy current→target if CLOUD_TEMPORAL_ENABLED=false or history invalid
5. Sky pass                 (depth=untouched pixels only)
6. Cloud composite pass     → swapchain surface
7. UI pass
```

### Key GPU Uniforms (WGSL side)

**Group 0:** `CameraUniform` (`cam`)
- `cam.camera_pos.xyz` — world-space eye position
- `cam.world_params` — `[world_scale, 1/world_scale, camera_motion, visible_world_m]`
- `cam.camera_forward.w` — `frame_index as f32`

**Group 1:** `LightingUniform` (`light`)
- `light.weather_globals[0]` — `[sky_time, wind_speed, wind_cos, wind_sin]`
- `light.weather_globals[1]` — `[base_height (0..1), layer_thickness (0..1), wind_shear, erosion]`
- `light.weather_globals[2]` — `[noise_scale, detail_strength, silver_lining, layer_separation]`
- `light.weather_globals[3]` — `[cirrus (0..1), mid_layer_height (0..1), cumulus, stratus]`
- `light.cloud_authoring[0]` — `[density, altocumulus, altostratus, storm_thickness]`
- `light.cloud_authoring[1]` — `[anvil_spread, coverage, cumulonimbus, advection]`
- `light.weather_clip_regions[0..2]` — `[origin_x, origin_z, span_m, 1/span_m]` per clipmap level
- `light.weather_clip_summaries[0..2]` — `[resolution, avg_coverage, avg_storm, avg_precipitation]`

**Group 2:** `AtmosphereRuntime` (transmittance LUT, sky-view LUT, multiscatter LUT, aerial perspective 3D)

**Group 3 (cloud render BG):**
- binding 0: `weather_primary_tex` (texture_2d_array — 3 clipmap levels)
- binding 1: `weather_secondary_tex` (texture_2d_array — 3 clipmap levels)
- binding 2: `cloud_field_sampler`
- binding 3: `shape_noise_tex` (texture_3d)
- binding 4: `detail_noise_tex` (texture_3d)
- binding 5: `blue_noise_tex` (texture_2d)
- binding 6: `scene_depth_tex` (texture_depth_2d)
- binding 7: `cloud_shadow_tex` (texture_2d_array)

---

## Diagnosed Root Causes of "No Clouds"

This is a multi-layered failure. Investigate **all** of the following:

### 1. Scale Mismatch: Cloud Layer Heights vs. Camera Position

**Critical.** `cloud_layer_bounds()` (in `cloud_shared.wgsl`) returns heights in **meters** derived from `CloudControls`:

```wgsl
let low_base_center = mix(350.0, 2200.0, base_ctl);  // 350–2200 m
b.low_base = low_base_center * 0.82;                  // ~287–1804 m
b.low_top  = low_base_center + low_thickness + storm_extension; // 1200–10000+ m
```

The camera Y coordinate in this engine uses **blocks**, not meters. The `world_scale` (= meters per block, set in `cam.world_params.x`) is the conversion factor.

**Problem:** The shader never converts layer heights from meters to block-space. If `world_scale = 0.5` (current likely default), a cloud at Y=350m should be at Y=700 blocks. But `compute_layer_ray_origin` compares `cam_pos.y` (in blocks) against `base_y / top_y` (in meters), so the camera is always below the clouds when it shouldn't be, and no ray intersects the slab.

**Fix:** Either:
- (A) Multiply all layer bounds by `cam.world_params.y` (= `1/world_scale`) to convert to block-space before use, OR
- (B) Divide `cam.camera_pos.y` by `world_scale` to convert camera Y to meters for the intersection tests

Option (B) is safer since it keeps all height math in world-meters:

```wgsl
let cam_pos_m = vec3<f32>(cam.camera_pos.x, cam.camera_pos.y * cam.world_params.y, cam.camera_pos.z);
// Use cam_pos_m in slab intersection, not cam.camera_pos.xyz
```

**Verify:** Check what `cam.world_params.x` (world_scale) is actually set to at runtime. See `render.rs` near `world_scale` assignment.

---

### 2. `layer_activity()` Early-Out Killing All Layers

In `cloud_shader.wgsl`, `march_layer_core()` calls `layer_activity()` and returns empty if too low:

```wgsl
fn layer_activity(cirrus_layer: bool, mid_layer: bool) -> f32 {
    let density = authored_cloud_density();   // light.cloud_authoring[0].x
    let cover = max(authored_coverage(), near_field_cloud_cover()); // light.cloud_authoring[1].y
    return clamp(density * cover, 0.0, 1.0);
}
// ...
if !cirrus_layer && !mid_layer && activity <= 0.0 { return empty; }  // ← note: threshold is 0.0 for low
if cirrus_layer && activity <= 0.010 { return empty; }
if mid_layer   && activity <= 0.012 { return empty; }
```

**Problem:** `near_field_cloud_cover()` reads `light.weather_clip_summaries[0].y` which is `avg_coverage` from the weather clipmap. If the weather texture hasn't been uploaded yet (first few frames), that value is 0.0. Combined with any authored coverage value, `max(authored, 0.0) * density` could still pass. But if `authored_cloud_density()` or `authored_coverage()` are also 0 (e.g., from `CloudControls::default()` at pipeline init time vs. the actual running controls), the product is 0 and all layers abort.

**Verify:** Add a debug view that outputs `vec4(density, cover, activity, 0.0)` to diagnose.

---

### 3. `scene_depth_distance()` Returns 0.0 for All Pixels When Terrain Hasn't Drawn

The cloud shader caps `max_dist = min(120000.0, max(scene_dist - scale * 2.0, 0.0))`.

If `scene_depth_distance()` returns a very small value (terrain rendered but cloud pass reads stale depth, wrong depth format, or wrong NDC flip), `max_dist` collapses to 0 and the march never executes.

**Verify:** 
- Is `scene_depth_tex` written by the terrain pass before the cloud pass executes? (Check render pass order in `render.rs` around line 5540)
- Is the depth texture bound with `texture_depth_2d` in WGSL and as a depth texture view (not a regular float view) in the bind group?
- Does the depth texture use **reversed-Z** (depth 0.0 = far, 1.0 = near)? The `>= 0.99999` early-out in `scene_depth_distance` assumes **standard Z** (1.0 = far). If reversed-Z is used, this needs to flip to `<= 0.00001`.

---

### 4. `compute_layer_ray_origin()` Returns `valid = false` Due to Scale Mismatch

Even if item #1 is fixed, `compute_layer_ray_origin` will return `valid = false` if the ray is pointing downward (camera above clouds) or if the Y-comparison fails. With the current default `base_height = 0.28`, `low_base ≈ 287 m`. If `world_scale` causes `cam.camera_pos.y` to be e.g. `64.0` (blocks) while `base_y = 287.0` (meters), the function correctly sees the camera is below the slab but then requires `t_enter >= 0` — which it would be since the ray goes up. So this should actually work *if the ray is going up*, but it means the player needs to be looking at the sky. Looking horizontally or down will produce `valid = false`.

**Verify:** At startup, is the camera looking upward? The default camera direction matters.

---

### 5. Weather Texture Not Uploaded / Empty on First Frame

The weather system uploads via dirty-rect manifests. On the first frame, `take_gpu_upload_manifest()` should return a full-level upload. However, if the upload loop in `render.rs` doesn't execute before the cloud pass (e.g., it's gated on a condition), the weather texture contains all zeros, causing zero coverage and thus zero density everywhere.

**Verify:** In `render.rs`, find where `CloudWeatherUploadManifest` is processed and confirm it runs before the first cloud pass frame.

---

### 6. `cloud_history_valid = false` → Temporal Pass Skips, Copy Happens Instead

When `CLOUD_TEMPORAL_ENABLED = true` and `cloud_history_valid = false`, the code does a texture copy from `cloud_current_tex` → `cloud_target_tex` instead of running the temporal pass. This is correct behavior, but it means the first 1–2 frames won't have history. This is **not causing invisible clouds** but means the first frame's clouds are unfiltered.

---

### 7. Cloud Composite Pass May Be Discarding Output

In `cloud_composite.wgsl`, verify it is not discarding due to:
- Alpha = 0: if `combined_alpha < threshold`, it returns early
- Wrong blend mode: if the composite pass blend state is misconfigured, it won't write to the swapchain

**Verify:** In `render.rs`, check the `ColorTargetState` blend for the cloud composite pipeline.

---

## Recommended Debugging Sequence

### Step 1: Add a Trivial Output Test

Temporarily replace `fs_main` in `cloud_shader.wgsl` with:
```wgsl
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 0.5); // Solid red at 50% alpha
}
```
If you see a red tint, the cloud pass is executing and the composite is working. If not, the problem is in the pipeline setup or the composite pass.

### Step 2: Check Scale

In `fs_main`, output `cam.world_params` to verify:
```wgsl
return vec4<f32>(cam.world_params.x, cam.world_params.y, cam.world_params.z, 1.0);
```
`world_params.x` = world_scale (meters/block). A value of `0.25`–`2.0` is expected.

Then output layer bounds:
```wgsl
let b = cloud_layer_bounds();
return vec4<f32>(b.low_base / 1000.0, b.low_top / 1000.0, cam.camera_pos.y / 1000.0, 1.0);
```
If `low_base/top` are in the 0.3–2.0 range (normalized to km) but `cam.camera_pos.y` is tiny (like 0.05), the scale mismatch is confirmed.

### Step 3: Test `compute_layer_ray_origin`

```wgsl
let bounds = cloud_layer_bounds();
let cam_pos_m = vec3<f32>(cam.camera_pos.x, cam.camera_pos.y * cam.world_params.y, cam.camera_pos.z);
let low_ro = compute_layer_ray_origin(cam_pos_m, dir, bounds.low_base, bounds.low_top);
return vec4<f32>(select(0.0, 1.0, low_ro.valid), low_ro.origin.y / 2000.0, 0.0, 1.0);
```
Green channel should show the slab entry height.

### Step 4: Check Depth

```wgsl
let depth_dist = scene_depth_distance(in.ndc, dir);
return vec4<f32>(depth_dist / 10000.0, 0.0, 0.0, 1.0);
```
Should show non-zero values where terrain exists, and very large (>1.0, clamped) for sky pixels.

### Step 5: Check Weather Texture

In the cloud shader, sample the weather primary tex directly:
```wgsl
let weather = weather_at_world(vec2<f32>(0.0, 0.0));
return vec4<f32>(weather.coverage, weather.humidity, weather.cloud_type, 1.0);
```
Should show non-zero coverage values if the weather upload worked.

---

## Implementation Fix Plan (After Diagnosis)

### Fix A: Height Unit Conversion (Most Likely Root Cause)

In `cloud_shader.wgsl` `fs_main`, convert camera position to meters before all slab operations:

```wgsl
// Convert camera position to world-meters for cloud height comparisons.
// All layer bounds are in meters; camera_pos is in blocks.
let world_scale = cam.world_params.x;
let cam_pos_m = vec3<f32>(
    cam.camera_pos.x * world_scale,
    cam.camera_pos.y * world_scale,
    cam.camera_pos.z * world_scale,
);

// Use cam_pos_m everywhere compute_layer_ray_origin is called.
let low_ro  = compute_layer_ray_origin(cam_pos_m, dir, bounds.low_base, bounds.low_top);
let mid_ro  = compute_layer_ray_origin(cam_pos_m, dir, bounds.mid_base, bounds.mid_top);
let high_ro = compute_layer_ray_origin(cam_pos_m, dir, bounds.high_base, bounds.high_top);
```

Also fix `march_layer_core` similarly — when sampling density, the `ray_origin` passed in is meters, but the noise domain may expect block-space. Check all world-position noise arguments.

Also fix `camera_inside_layer` call in the interior cloud ambient block:
```wgsl
let inside_low  = camera_inside_layer(cam_pos_m.y, bounds.low_base, bounds.low_top);
```

### Fix B: Depth Extraction (If Reversed-Z)

If the engine uses reversed-Z depth:
```wgsl
// OLD:
if depth >= 0.99999 { return 1e9; }
// NEW for reversed-Z:
if depth <= 0.00001 { return 1e9; }
```

### Fix C: Ensure Weather Upload Happens Before First Cloud Pass

In `render.rs`, verify the weather upload code path runs unconditionally on the first frame, not only when `manifest.is_some()` and the manifest is empty due to missed first-frame trigger.

---

## Architectural Context (Do Not Regress)

These constraints are non-negotiable per `docs/ATMOS_CLOUD_REWRITE_STATUS.md`:

1. **Sky pass must not own cloud composition** — clouds composite via `cloud_composite.wgsl`
2. **Weather textures are the source of truth** — no fallback to shader-local hash functions for coverage
3. **Temporal history is reconstruction support** — do not disable temporal to "fix" the bug
4. **`render::atmosphere` owns all LUT textures** — do not re-add LUT generation to `render.rs`
5. **Cloud layer heights must be continuous world-space values** — no camera-relative slab coordinates

---

## Final Deliverable

After diagnosing and fixing the issue, confirm:
1. `cargo check` passes with no errors
2. Clouds are visible in the game window
3. The `CloudAlpha` debug view shows non-zero alpha where clouds should be
4. The root cause is documented with a one-sentence explanation of why it was failing
5. No existing fog, sky, terrain, or shadow behavior regressed
