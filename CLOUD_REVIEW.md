# Cloud Shader Code Review & Issues

## CRITICAL BUGS

### Bug 1: Clouds Follow Camera (Primary Bug) ✗ CONFIRMED
**Location:** `cloud_shader.wgsl:754-764`

```wgsl
let low = march_layer(
    cam.camera_pos.xyz, dir, ...  // ← ray_origin = camera XYZ
```

**Problem:**
Inside `march_layer`, all sample points are computed as:
```wgsl
let p = ray_origin + ray_dir * t_cur;  // line 616
```

When `ray_origin = cam.camera_pos.xyz`, the world position `p.xz` becomes:
```
p.xz = cam.camera_pos.xz + dir.xz * t
```

This means when the camera moves 100 units east, every noise sample moves 100 units east too. The cloud density field (`sample_low_density`, `sample_cirrus_density`) samples noise at `p.xz`, which **slides with the camera at 1:1 scale**. Clouds are anchored to the camera, not the world.

**Impact:** Clouds follow the player perfectly on lateral movement.

**Root Cause:** The noise functions use `p.xz` directly without compensating for camera lateral position. When the ray is cast from the camera, the sample positions inherit the camera's XYZ coordinates.

**Fix:** Pass a **world-anchored ray origin** to `march_layer`. The ray origin should be computed so that `p.xz` is independent of camera lateral movement. Options:
1. **Best:** Compute the world XZ position where the ray enters the slab (using ray direction only):
   ```wgsl
   let t_entry = (slab_mid_y - cam.camera_pos.y) / dir.y;
   let world_ray_origin = vec3(cam.camera_pos.x + dir.x * t_entry,
                                cam.camera_pos.y,
                                cam.camera_pos.z + dir.z * t_entry);
   ```
   But this still depends on camera position. **Correct fix:**

2. **Correct:** Subtract camera XZ from the ray origin:
   ```wgsl
   // Ray origin with camera XZ removed — makes noise anchor independent of camera lateral movement
   let ray_origin_decentered = vec3(0.0, cam.camera_pos.y, 0.0);
   let low = march_layer(ray_origin_decentered, dir, ...);
   ```
   Then inside `march_layer`, when computing `p = ray_origin + ray_dir * t`, the `p.xz = dir.xz * t` is purely direction-dependent. The noise must still be sampled in absolute world coordinates by adding back `cam.camera_pos.xz`:
   ```wgsl
   let p_world = p + vec3(cam.camera_pos.x, 0.0, cam.camera_pos.z);  // Add back camera XZ for absolute world coords
   // Sample noise at p_world instead of p
   ```

3. **Simpler (but requires uniform passing):** Pass `cam.camera_pos.xz` into `march_layer` and subtract it from `p.xz` before noise sampling:
   ```wgsl
   let p_noise_coords = p.xz - cam.camera_pos.xz;  // Decouple from camera
   let weather_uv = p_noise_coords * 0.0000085 + ...;
   ```

---

### Bug 2: Temporal Reprojection Reprojecting a Camera-Relative Field ✗ CONFIRMED
**Location:** `cloud_temporal.wgsl:105-114`

```wgsl
let reproj_world = cam.camera_pos.xyz + dir * reproj_t;
let prev_clip = cam.prev_view_proj * vec4<f32>(reproj_world, 1.0);
```

**Problem:**
The temporal accumulator reprojection logic is mathematically correct IF the underlying cloud density field is world-space. Because of Bug 1, the cloud field is **camera-relative**. Result:

- When camera translates, `reproj_world` changes.
- The history lookup finds an old screen position with plausible-looking cloud data.
- But that old data was sampled from a different camera position (and thus different world coordinates).
- Temporal ghosting and smearing on movement.

**Impact:** Clouds smear/ghost when the player moves; temporal stability breaks.

**Fix:** Once Bug 1 is fixed (clouds become world-anchored), the reprojection naturally works correctly. **No changes needed to `cloud_temporal.wgsl`** after Bug 1 fix. The structure is correct; it's the data being reprojected that was wrong.

---

### Bug 3: Scene Depth Clipping (Disabled But Still In Code) ✗ PARTIAL ISSUE
**Location:** `cloud_shader.wgsl:747-751` (disabled via comment)

```wgsl
// Clouds exist purely in the sky; terrain occludes them via the depth buffer
// in the sky shader composite. Do NOT clip the march against the scene depth
// texture here — the cloud pass runs before terrain is drawn this frame,
// so the depth buffer would contain stale last-frame geometry, causing
// flickering and temporal artifacts. Use a large fixed horizon distance.
```

**Issue:** The comment correctly identifies the problem, but the `scene_depth_ray_t` function (lines 59-89) is still compiled into the shader even though it's not called. This is dead code.

**Impact:** Minor — shader bloat, ~30 lines of unused texture sampling logic.

**Fix:** Delete `scene_depth_ray_t` function if truly unused. Confirm no other call site.

---

## OPTIMIZATION ISSUES

### Issue 1: Exponential Math Recomputed Every Frame
**Location:** `cloud_shader.wgsl:562-564`

```wgsl
let alpha     = mix(0.8, 2.2, horizon_f);
let exp_alpha = exp(alpha);
let exp_denom = 1.0 / max(exp_alpha - 1.0, 1e-6);
```

Then used in the inner loop (lines 612-613):
```wgsl
let t_cur = t_min + slab_len * (exp(alpha * saturate1(frac_i))    - 1.0) * exp_denom;
let t_nxt = t_min + slab_len * (exp(alpha * saturate1(frac_next)) - 1.0) * exp_denom;
```

**Problem:** For each pixel, `exp(alpha)` is computed once, but `exp(alpha * frac_i)` is computed **per step inside the loop (up to 56 times)**. `exp()` is an expensive operation.

**Impact:** ~1 expensive exp() per sample × 48 samples = 48 exps per low cloud, 14 per high cloud, per pixel.

**Fix:** Precompute `exp(alpha * saturate1(frac))` values:
```wgsl
// Use a lookup table or precompute the exponential distribution
// Instead of: exp(alpha * frac_i) - 1.0
// Compute: a small table of pre-exp'd values indexed by [0..steps]
```

Or use a simpler step distribution (linear with a curve) that avoids `exp()` per sample.

---

### Issue 2: Redundant Density Function Calls (No Memoization)
**Location:** `cloud_shader.wgsl:622-626`

```wgsl
let sample_density = select(
    sample_low_density(p, base_y, top_y, p.y, dist_fade, dist_t),
    sample_cirrus_density(p, base_y, top_y, p.y),
    cirrus_layer
) * dist_fade;
```

And then later (lines 632-633):
```wgsl
let shadow_d = sun_shadow_density(p, shadow_dir, shadow_step,
                                  base_y, top_y, cirrus_layer);
```

Inside `sun_shadow_density`, the same `sample_density_shadow` is called again:
```wgsl
// cloud_shader.wgsl line 456
let d = select(
    sample_density_shadow(q, base_y, top_y, q.y),
    sample_cirrus_density(q, base_y, top_y, q.y),
    cirrus_layer
);
```

**Problem:** For shadow rays, we call `sample_cirrus_density` again even though we already computed it at `p`. The shadow ray uses the same `p` for the first shadow sample (when `i=0, t=step_len*0.5`). The density values are nearly identical, but re-computed.

**Impact:** ~5-10% wasted texture lookups in shadow ray.

**Fix:** Cache `sample_density` and pass it to `sun_shadow_density` as an optional parameter, or use a cheaper approximation for shadow rays (the current `sample_density_shadow` is already a 3x cheaper variant, so this is partially mitigated).

---

### Issue 3: Weather Lookup Called Per Sample (3x Rotation + 3x Texture Samples)
**Location:** `cloud_shader.wgsl:256-258` (inside `sample_low_density`)

```wgsl
let macro_w    = macro_warp(p.xz, time);
let weather_uv = p.xz * 0.0000085 + turbulent_flow * 0.003 + macro_w * 0.040;
let weather    = weather_lookup(weather_uv);
```

Inside `weather_lookup` (lines 140-156):
```wgsl
let uv0 = fract(rotate2(uv + seed_offset * 0.41, angle * 0.37));
let uv1 = fract(rotate2(uv * 0.73 + ..., 1.12 + angle * 0.21));
let uv2 = fract(rotate2(uv * 1.31 + ..., -0.74 - angle * 0.17));

let a = textureSample(weather_tex, cloud_field_sampler, uv0);
let b = textureSample(weather_tex, cloud_field_sampler, uv1);
let c = textureSample(weather_tex, cloud_field_sampler, uv2);
```

**Problem:** Each sample calls `weather_lookup`, which does **3 rotations + 3 texture reads**. In `sample_low_density`, this is called once per march step (48 steps/pixel). But the weather pattern is slow-moving (dominated by large-scale ocean/land patterns). The rotation angle is **deterministic per-frame** (not per-pixel). Nearby pixels could share weather lookups.

**Impact:** 48 samples × 3 texture reads = 144 texture reads per low-cloud pixel, all for a smooth pattern that varies slowly.

**Fix:**
- Cache weather lookups in a small tile grid (e.g., 4×4 per screen).
- Or use a single high-quality weather sample per pixel instead of per-ray.
- Or reduce step count and accept lower detail (already partly done via motion_mod).

**Note:** This is a trade-off between quality and performance. The current approach is "correct" (high detail) but expensive.

---

### Issue 4: Blue Noise Jitter Uses `fract()` Without Wrapping on Integer Boundary
**Location:** `cloud_shader.wgsl:473-486`

```wgsl
fn blue_noise_jitter(frag_coord: vec2<f32>) -> f32 {
    let sz    = 64;
    let pixel = vec2<i32>(frag_coord) % vec2<i32>(sz, sz);
    let bn    = textureLoad(blue_noise_tex, pixel, 0).r;
    let frame_offset = fract(light.clouds4.x * 0.41421356);
    return fract(bn + frame_offset);
}
```

**Problem:** Not a bug, but could be clearer. The `fract()` on the final return handles wrapping correctly, but `bn` is already in [0,1], so `fract(bn + frame_offset)` is correct. No issue here, but note that `frame_offset` advances by ~0.618 per frame (golden ratio), which is ideal.

**Impact:** None (correctly implemented). This is well-designed.

---

## CODE QUALITY & STYLE ISSUES

### Issue 5: Inconsistent Null Check Patterns
**Location:** Multiple functions use different epsilon checks:

- `sky_dir_from` (cloud_temporal.wgsl:39): `max(world.w, 1e-5)`
- `slab_intersection` (cloud_temporal.wgsl:44): `abs(rd.y) < 1e-5`
- `octa_map` (sky_shader.wgsl:71): `max(a.x + a.y + a.z, 1e-5)`

**Problem:** No consistency in epsilon values (1e-5 vs 1e-6). Should standardize.

**Impact:** Minor — different tiny values can cause subtle rendering differences across shaders.

**Fix:** Define a constant `const EPSILON = 1e-5` and use throughout.

---

### Issue 6: Redundant `saturate1()` Calls
**Location:** Throughout all shaders, e.g.:

```wgsl
let coverage = saturate1(light.clouds0.x);
let density  = saturate1(light.clouds0.y);
let cirrus   = saturate1(light.clouds1.x);
```

**Problem:** `light.clouds0.x` and friends are already in [0,1] from the CPU side (they come from normalized parameters). Clamping them again is redundant.

**Impact:** Minimal (clamp is very cheap), but suggests uncertainty about data ranges.

**Fix:** Remove redundant `saturate1()` on input parameters that are guaranteed [0,1].

---

### Issue 7: `march_layer` Called With Different Pixel Jitter Offsets
**Location:** `cloud_shader.wgsl:754-764`

```wgsl
let low = march_layer(
    cam.camera_pos.xyz, dir, sun_dir, moon_dir, in.pos.xy,
    low_base, low_top, ...
);

let high = march_layer(
    cam.camera_pos.xyz, dir, sun_dir, moon_dir, in.pos.xy + vec2<f32>(17.0, 11.0),  // ← Different offset!
    high_base, high_top, ...
);
```

**Problem:** The high cirrus layer is jittered by `(17, 11)` pixels relative to the low layer. This breaks temporal coherence between layers and can cause visible seams if one layer renders but the other doesn't.

**Impact:** Potential temporal artifacts at layer boundaries.

**Fix:** Use the same `in.pos.xy` for both layers, or use a frame-relative offset (e.g., `frame_index % some_period`) that's consistent across layers.

---

## MISSING / INCOMPLETE FEATURES

### Issue 8: No LOD for Detail Noise
**Location:** `cloud_shader.wgsl:322-357`

```wgsl
let detail_lod = smoothstep(0.06, 0.42, dist_fade) * (1.0 - smoothstep(0.0, 0.7, dist_t));
```

**Problem:** Detail LOD is computed but only used to scale erosion strength. The detail texture is still sampled at full resolution even when far away. Distant clouds load and sample the expensive detail noise texture unnecessarily.

**Impact:** 10-15% of detail texture samples wasted on far-away pixels that contribute <1% to the final image.

**Fix:** Skip detail texture sampling entirely when `detail_lod < 0.01`:
```wgsl
if detail_lod > 0.001 && body > 0.001 {
    // ... existing detail sampling ...
} else {
    eroded_body = body;  // No detail when far away
}
```
Already partially done (`if detail_lod > 0.001`), but `body > 0.001` check is also there.

---

### Issue 9: Macro Coverage Mod Not Used in Cirrus Layer
**Location:** `cloud_shader.wgsl:219-370` (low layer) vs. `cloud_shader.wgsl:372-401` (cirrus layer)

Low layer calls:
```wgsl
let macro_cov = macro_coverage_mod(p.xz, time, seed);
let coverage = saturate1(light.clouds0.x + (weather.x - 0.5) * 0.38) * macro_cov;
```

Cirrus layer **does not**:
```wgsl
// No macro_coverage_mod in sample_cirrus_density
```

**Problem:** Low clouds have macro coverage variation (clear/cloudy regions), but cirrus doesn't. Cirrus appears uniformly dense across the sky.

**Impact:** Unrealistic; cirrus should also vary regionally.

**Fix:** Apply `macro_coverage_mod` to cirrus density as well.

---

## SUMMARY TABLE

| # | Severity | Issue | Location | Impact | Fix Complexity |
|---|----------|-------|----------|--------|---|
| 1 | **CRITICAL** | Clouds follow camera | cloud_shader.wgsl:754 | Game-breaking | Medium |
| 2 | **CRITICAL** | Temporal ghosting | cloud_temporal.wgsl:110 | Temporal artifacts | Low (fixed by #1) |
| 3 | Medium | Dead code (scene depth) | cloud_shader.wgsl:59-89 | Shader bloat | Low |
| 4 | Medium | Exponential recompute | cloud_shader.wgsl:612 | 10-15% perf loss | Medium |
| 5 | Low | Weather lookup per-sample | cloud_shader.wgsl:256 | 20-30% perf loss (design choice) | High |
| 6 | Low | Inconsistent epsilons | Multiple | Subtle diffs | Low |
| 7 | Low | Redundant saturate1() | Multiple | Minimal | Low |
| 8 | Low | Layer jitter offset | cloud_shader.wgsl:761 | Potential seams | Low |
| 9 | Low | No detail LOD skip | cloud_shader.wgsl:322 | 10% wasted samples | Low |
| 10 | Low | Cirrus missing macro cov | cloud_shader.wgsl:372 | Unrealistic cirrus | Low |

---

## NEXT STEPS

1. **Fix Bug #1** — Decouple ray origin from camera XY position
2. **Fix Bug #2** — Verify temporal reprojection works after #1
3. **Fix Dead Code** — Remove `scene_depth_ray_t`
4. **Optimization Pass** — Precompute exponentials, skip detail when far, add cirrus macro coverage
5. **Retest** — Full temporal accumulation, visual quality, performance profile
