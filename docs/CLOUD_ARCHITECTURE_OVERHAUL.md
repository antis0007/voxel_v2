# Cloud Rendering Architecture Overhaul Plan

**Author:** Senior Realtime Graphics Engineer review
**Status:** Active — full architectural rewrite required

---

## Executive Summary

The current cloud renderer has fundamental architectural flaws that cannot be patched incrementally. The DDA cell-grid system, per-cell random noise transforms, and weather clipmap level seams combine to produce clouds that are visually broken at an engine-quality level. Competitors (Guerrilla Nubis, Decima, Frostbite) ship none of these artifacts because they use a fundamentally different architecture. This document specifies the correct approach.

---

## What Is Actually Wrong (Root Cause Analysis)

### Problem 1: DDA Cell Traversal — The Wrong Primitive

**Current:** The march loop uses Amanatides-Woo 2D DDA on a 400m XZ weather cell grid. Each cell gets an AABB from its weather data. The ray marches only within each cell's AABB.

**Why it is wrong:**
- DDA is correct for voxel terrain where each cell has a hard surface. Clouds are not cells.
- The per-cell AABB creates hard Y boundaries that appear as horizontal seams across the sky.
- When adjacent cells have different weather-driven AABB heights, the boundary between them is a visible artifact — exactly the "horizontal stripes" being reported.
- Per-step weather resampling was added as a workaround, making the DDA traversal itself redundant (if weather is per-step anyway, DDA gives no information benefit).
- Nubis, Frostbite, and all other production cloud renderers do NOT use DDA for clouds. They use a direct slab-bounded march with coverage-based empty-space skipping.

**Fix:** Remove the DDA loop entirely. Replace with a simple slab-bounded march with two step sizes: large (empty-space skip when coverage < threshold) and small (inside cloud region).

### Problem 2: Per-Cell Random Noise Transforms — The Primary Seam Source

**Current:** `sample_shape_field` bilinearly blends 4 cells (CLOUD_SHAPE_CELL_M = 600m after recent fix). Each cell applies a random rotation, scale, and offset to the 3D texture UV, producing a unique-looking noise domain per cell.

**Why it is wrong:**
- Although the bilinear blend is mathematically C1 continuous, adjacent cells map to completely different regions and orientations of the shape texture. The GRADIENT of density changes discontinuously at blend boundaries because both sides of the blend are uncorrelated random samples.
- This creates visible bands at shape cell boundaries — at 600m intervals across the entire sky. With a 16km view distance, this gives ~26 band intervals along any horizontal ray.
- Nubis uses a GLOBAL 3D noise domain. No per-cell transforms. No seams. The 3D texture tiles at ~80km (world radius ~6km, so never visible).
- The same applies to detail cells at 300m. These create 2× as many bands.

**Fix:** Replace `sample_shape_field` and `sample_detail_field` with global 3D sampling functions that use world coordinates directly. No cell grid, no random transforms, no seams.

### Problem 3: Weather Clipmap Level Seams

**Current:** Three clipmap levels (1024m/4096m/16384m) blend with `smoothstep(0.04, 0.38, edge_distance)`. Level transitions are at ~400m and ~1600m radius.

**Why it is wrong:**
- The weather field is evaluated by three independent CPU-side noise passes, one per level. Adjacent levels can produce different coverage values at the same world position because different texel spacings produce different bilinear samples of the same underlying FBM.
- This causes visible ring artifacts where clouds appear/disappear at clipmap level boundaries.
- The outermost level (16384m, 128m/texel) severely undersamples the `breakup` noise (period ~244m), causing `low_amount` to diverge from inner levels.

**Fix:**
- Use a single procedural coverage function evaluated in-shader (eliminates level seams entirely)
- OR use only level 2 (16384m) as the sole weather source and increase resolution
- The weather CPU authoring system should write to a single texture with smooth FBM that matches across scales

### Problem 4: Cirrus Clouds Have No Physical Appearance

**Current:** Cirrus uses the cell-based 3D noise system (`sample_shape_field` with CLOUD_SHAPE_CELL_M). The result looks like splotchy cumulus fragments at high altitude, not real cirrus.

**Why it is wrong:**
- Real cirrus are ice crystal streaks formed in jet streams. They are:
  - Extremely elongated (aspect ratio 5:1 to 50:1 along wind direction)
  - Very thin (20-100m)
  - Fibrous and wispy, not billowy
  - White to pale gray (low optical depth)
- The current system applies the same billowy FBM used for cumulus, producing square patches at 600m intervals.

**Fix:** Dedicated cirrus density function that:
- Applies strong anisotropy: compress the noise domain by 4-8x perpendicular to the wind direction
- Uses very thin vertical profile (top_soft ~0.05 instead of 0.20)
- Uses finer-scale fiber noise for the wispy texture
- Uses the cirrus phase function (already exists) but needs the shape to match

### Problem 5: Extinction Calibration (Fixed in Recent Pass)

Prior to the recent fix: `extinction = 0.65` with `dt = 2-5m` made clouds fully opaque in 10-20m. `ext_light = 12` turned tiny shadow variations into black stripes. These have been corrected.

### Problem 6: Step Size Aliasing (Fixed in Recent Pass)

Prior to the recent fix: `dt = 2-5m` caused consecutive samples to hit nearly identical noise texel positions, creating interference patterns as the cloud render target is at half resolution with temporally-rotated jitter. Corrected to 12-28m range.

---

## Target Architecture

The target is the approach used in Nubis (Guerrilla) and described in Hillaire 2020 (Frostbite).

### Noise Domain
```
// Single global 3D noise — no cells, no transforms, no seams
fn sample_cloud_shape(world_pos, wind_advect, lod) -> vec4 {
    let p = world_pos + vec3(wind_advect, 0);
    let uvw_a = fract(p * BASE_FREQ);                          // ~2500m period
    let uvw_b = fract(p * BASE_FREQ * 0.618 + OFFSET_B);      // anti-aliasing
    return texSample(shape_noise, uvw_a) * 0.62 +
           texSample(shape_noise, uvw_b) * 0.38;
}
```

### March Loop
```
// Coverage-based empty-space skipping — no DDA
slab_enter ─► while T < slab_exit:
  weather = sample_2d_coverage(p.xz)
  if coverage < 0.05:  T += 80m; continue          // large skip
  density = sample_3d_density(p, weather)
  if density < 0.001:  T += 30m; continue          // medium skip
  integrate_lighting(density)                       // small step
  T += 12m
```

### Cirrus
```
// Anisotropic elongated noise, no volumetric DDA
fn cirrus_density(p, base_y, top_y, weather):
  wind_dir = get_wind()
  side_dir = perp(wind_dir)
  adv = p.xz + wind_dir * time * speed
  // Compress perpendicular to wind → elongated streaks
  stretched = dot(adv, wind_dir) * wind + dot(adv, side_dir) / 5.0 * side_dir
  shape = sample_2d_at(stretched, LOW_FREQ)
  fiber = sample_2d_at(stretched, HIGH_FREQ)
  height = thin_slab_profile(p.y, base_y, top_y)
  return (shape * 0.5 + fiber * 0.5) * height * weather.high_amount
```

---

## Implementation Gates (Ordered by Impact)

### Gate 0 — Immediate (this session)
- [x] Fix extinction: 0.65 → 0.022 (prevents pitch-black clouds)
- [x] Fix ext_light: 12 → 2.2 (eliminates banding stripes)
- [x] Fix step size: 2-5m → 12-22m (reduces noise aliasing)
- [x] Fix weather snap: 32m → 8m (smoother clipmap tracking)
- [ ] **Replace DDA with simple adaptive march** ← DO NEXT
- [ ] **Replace cell-based noise with global 3D sampling** ← DO NEXT
- [ ] **Rewrite cirrus with anisotropic elongation** ← DO NEXT

### Gate 1 — Near Term
- [ ] Single-level weather: remove level 0/1 boundaries, use level 2 only (or match values)
- [ ] Validate cloud brightness against reference photographs
- [ ] Verify no temporal ghosting after DDA removal

### Gate 2 — Medium Term
- [ ] In-shader procedural weather macro field (eliminates clipmap seams entirely)
- [ ] Proper light cache / precomputed per-column OD for shadowing
- [ ] True per-pixel motion vectors for temporal reprojection

### Gate 3 — Long Term (Nubis parity)
- [ ] SDF-accelerated empty-space skipping
- [ ] Low-res 3D light cache updated each frame
- [ ] Temporal upsampling at 1/4 resolution with 4-frame history

---

## Files to Change for Gate 0 Completion

| File | Change |
|------|--------|
| `src/shaders/cloud_shader.wgsl` | Remove DDA, remove cell noise, global 3D noise, rewrite cirrus |
| `src/shaders/cloud_shared.wgsl` | Remove CLOUD_WEATHER_CELL_M reference from comments |
| `src/world/weather.rs` | snap_m = 1x texel (done) |

## What NOT to Change
- `cloud_temporal.wgsl` — temporal reprojection is correct, no change needed
- `atmosphere_shared.wgsl` — atmosphere is correct
- `atmosphere_lut_generation.wgsl` — LUT generation is correct
- `render.rs` weather uniform upload — weather clipmap GPU meta is correct
- Weather simulation CPU code — authoring pipeline is separate from rendering bugs

---

## Test Checklist

Before considering Gate 0 complete:
- [ ] No visible horizontal stripes on any cloud type
- [ ] No visible grid/square patterns at any view angle
- [ ] No cloud base/top flickering when camera is still
- [ ] Clouds appear continuously (no sudden edges aligned with any axis)
- [ ] Cirrus looks like white streaks, not splotchy patches
- [ ] Cloud self-shadowing visible but not pitch-black
- [ ] Smooth cloud motion (no 32m jumps)
- [ ] No seam ring at ~400m or ~1600m camera radius
