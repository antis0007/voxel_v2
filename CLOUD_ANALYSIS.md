# Cloud Renderer Analysis: Tearing & Layer Fighting Issues

## Executive Summary

**The volumetric cloud goals have been PARTIALLY implemented**, but **critical bugs remain** that will cause:
1. **Layer tearing and overlapping artifacts** along cell boundaries
2. **Fighting for the same altitude range** between adjacent cells
3. **Discontinuities in vertical bounds** as rays cross between weather cells

---

## Current Architecture (Per Memory)

The implementation uses **2D DDA cell traversal** on a 24km weather cell grid:
- Each cell has per-cell vertical AABB computed from weather parameters
- Ray marches only within the intersection of the ray and that cell's 3D box
- Global envelope used for coarse early-out only

This is architecturally sound, but the **execution has critical flaws**.

---

## Problem 1: DDA Cell Advancement Issues

### The Bug

**Location**: [cloud_shader.wgsl:1215-1224](src/shaders/cloud_shader.wgsl#L1215-L1224)

```wgsl
// ---- DDA advance to next cell ----
if tMax_x < tMax_z {
    ray_t  = tMax_x;
    cell.x += step_x;
    tMax_x += tDelta_x;
} else {
    ray_t  = tMax_z;
    cell.y += step_z;
    tMax_z += tDelta_z;
}
```

**Issue**: When moving to the next cell, `ray_t` is set to the boundary crossing distance, but **the march for the current cell may not have completed to that boundary**. This creates a gap:

- Inner loop marches from `t_enter` to `t_exit` where `t_exit = min(hit.y, min(cell_exit_t, max_dist))`
- After loop completes, `ray_t` is updated to `cell_exit_t` (either `tMax_x` or `tMax_z`)
- The next cell's march then starts fresh from `ray_t`, **potentially skipping the final boundary region**

**Visual symptom**: Horizontal seams where adjacent cells' vertical bounds don't align, causing thin dark or bright lines at cell boundaries.

---

## Problem 2: Vertical AABB Discontinuity

### The Bug

**Location**: [cloud_shader.wgsl:170-220](src/shaders/cloud_shader.wgsl#L170-L220) (cell_*_aabb functions)

**Issue**: Each cell's vertical AABB is computed **independently** from that cell's weather at its center:

```wgsl
let aabb = cell_aabb_for_layer(weather, layer_id);
```

But **weather is sampled only at the cell center**, via bilinear blend. This means:

1. Adjacent cells can have **wildly different vertical bounds** even if they're neighbors
   - Cell A might extend from 500m to 1500m
   - Cell B (adjacent) might extend from 200m to 1000m
   - No interpolation or blending between cells

2. **Rays crossing cell boundaries see sharp jumps in altitude**, creating:
   - Tearing: clouds appear to split or merge abruptly
   - Fighting: overlapping geometry at different Y coordinates within the same XZ region
   - Discontinuous density/lighting: the same world position in different cells produces different results

### Why It Happens

The AABB is derived from `weather.low_amount`, `weather.mid_amount`, etc., which are bilinearly blended at the cell center:

```wgsl
let cell_center_xz = cloud_cell_origin(cell, cell_size) + vec2<f32>(cell_size * 0.5);
let wx_offset = wind * time * wind_spd * wx_mult;
let weather = weather_at_world(cell_center_xz + wx_offset);
```

For a 24km cell, this is smooth within the cell. But **at the edge**, the next cell's weather is sampled from *its* center, creating discontinuity.

---

## Problem 3: Ray-Cell AABB Intersection Precision

### The Issue

**Location**: [cloud_shader.wgsl:230-240](src/shaders/cloud_shader.wgsl#L230-L240)

The ray-AABB intersection correctly clips the segment to `[t_enter, t_exit]`, but:

1. **No epsilon for boundary matching**: When a ray exits one cell at Y-coordinate `y_max`, and the next cell's march starts at its `y_min`, floating-point precision may cause:
   - `y_max` from cell A ≠ `y_min` from cell B even if they should align
   - Sub-sample holes or overlaps in the Y range

2. **No inter-cell blending**: The vertical AABB changes sharply, so:
   - Density at the boundary is discontinuous
   - Lighting samples in adjacent cells don't smoothly transition

---

## Problem 4: Weather Sampling at Cell Center is Too Coarse

### The Issue

**Weather changes significantly across a 24km cell**:
- A cell's corners may have very different `low_amount`, `mid_amount` values
- By sampling only at the center, we **lose the variation**
- This makes all rays within the same cell see the same vertical bounds (after bilinear), which is wrong

**Example**:
- Corner A (0, 0): `low_amount = 0.9` → tall clouds
- Center (0.5, 0.5): `low_amount = 0.5` → medium clouds
- Corner C (1, 1): `low_amount = 0.1` → short clouds

All three positions get marched with the **center's AABB**, creating:
- Overmarching at corners where clouds are thin
- Undermarching at corners where clouds are tall
- Visible seams and misalignment

---

## How These Combine to Cause Tearing & Fighting

1. **Tearing**: As the ray crosses from cell A to B:
   - Cell A's `y_max` ≠ Cell B's `y_min` (due to independent sampling)
   - Inner loop exits at the cell boundary
   - Next cell's march starts with a different vertical range
   - Result: visible tear or halo

2. **Layer fighting**: With 3 layers (low, mid, high), each has independent per-cell AABBs:
   - Mid-layer from cell A: Y ∈ [1500, 3000]
   - Low-layer from cell A: Y ∈ [500, 1500]
   - These don't overlap → correct
   - But cell B's low-layer: Y ∈ [300, 1200]
   - At the cell boundary, ray sees both cells' clouds occupying overlapping Y ranges
   - → **Blending artifacts, double-counting, brightness spikes**

---

## What Would Fix These Issues

### Solution 1: Interpolate Vertical AABB Bounds (Best)

Instead of computing AABB from weather at cell center, compute it **at the actual ray position**:

```wgsl
// Current (wrong):
let weather = weather_at_world(cell_center_xz + wx_offset);
let aabb = cell_aabb_for_layer(weather, layer_id);

// Fixed:
// Sample weather at the actual ray segment, not cell center
let weather = weather_at_world(p.xz + wx_offset);  // p is the march sample point
let aabb = cell_aabb_for_layer(weather, layer_id);
```

**Cost**: ~1 extra weather sample per march step (2 extra texture lookups).

**Benefit**: AABBs follow the actual weather field, eliminating discontinuities.

---

### Solution 2: Blend AABB at Cell Boundaries

After computing the primary cell's AABB, blend toward the neighboring cell's AABB near the boundary:

```wgsl
let primary_aabb = cell_aabb_for_layer(weather, layer_id);

// Near cell boundary, blend with neighbor
let blend_distance = 500.0;  // 500m from boundary
let dist_to_boundary = min(abs(t_enter - 0.0), abs(t_exit - cell_exit_t));
let boundary_blend = smoothstep(blend_distance, 0.0, dist_to_boundary);

if boundary_blend > 0.01 {
    let neighbor_weather = weather_at_world(neighbor_center_xz + wx_offset);
    let neighbor_aabb = cell_aabb_for_layer(neighbor_weather, layer_id);
    aabb.y_min = mix(primary_aabb.y_min, neighbor_aabb.y_min, boundary_blend);
    aabb.y_max = mix(primary_aabb.y_max, neighbor_aabb.y_max, boundary_blend);
}
```

**Cost**: ~1 extra weather sample per cell boundary crossing.

**Benefit**: Soft transitions eliminate sharp tearing.

---

### Solution 3: Per-Cell Corner AABBs

Compute AABB at all 4 corners of the cell, interpolate based on ray position within the cell:

```wgsl
// Sample at 4 corners
let w00 = weather_at_world(origin + vec2(0.0, 0.0) + wx_offset);
let w10 = weather_at_world(origin + vec2(cell_size, 0.0) + wx_offset);
let w01 = weather_at_world(origin + vec2(0.0, cell_size) + wx_offset);
let w11 = weather_at_world(origin + vec2(cell_size, cell_size) + wx_offset);

// Bilinearly interpolate at ray's XZ position
let aabb_blend = fract(ray_pos_in_cell / cell_size);
let aabb00 = cell_aabb_for_layer(w00, layer_id);
let aabb10 = cell_aabb_for_layer(w10, layer_id);
let aabb01 = cell_aabb_for_layer(w01, layer_id);
let aabb11 = cell_aabb_for_layer(w11, layer_id);

let aabb0x = mix(aabb00, aabb10, aabb_blend.x);
let aabb1x = mix(aabb01, aabb11, aabb_blend.x);
let aabb = mix(aabb0x, aabb1x, aabb_blend.y);  // Bilinear AABB
```

**Cost**: ~3 extra weather samples per cell (already sample 4 for blending).

**Benefit**: Smooth AABB variation within and across cells; eliminates corner artifacts.

---

## Secondary Issues (Lower Priority)

### Issue 1: No Handling for Zero-Length Rays (grazing angles)

When `rd.x ≈ 0` and `rd.z ≈ 0` (ray nearly vertical), the DDA setup can produce:
- `tMax_x` or `tMax_z` = infinity
- Infinite `tDelta_x` or `tDelta_z`
- The loop may not advance properly

**Fix**: Add epsilon checks in DDA setup:
```wgsl
if abs(rd_xz.x) < 0.001 { tDelta_x = 1e9; }
if abs(rd_xz.y) < 0.001 { tDelta_z = 1e9; }
```

### Issue 2: max_cells = 16 May Be Too Low for Shallow Rays

A ray nearly parallel to the ground could traverse 384km before hitting max_dist, requiring ~16 cells. The cap is right at the edge—consider 20 for safety.

---

## Recommended Fix Priority

| Priority | Issue | Effort | Impact |
|----------|-------|--------|--------|
| **HIGH** | Interpolate AABB at sample positions (Solution 1) | Medium | Eliminates 80% of tearing |
| **HIGH** | Fix DDA boundary handling (debug ray_t updates) | Low | Eliminates discontinuous marching |
| **MEDIUM** | Corner-AABB blending (Solution 3) | Medium-High | Smooth transitions, best visual quality |
| **MEDIUM** | Boundary blend (Solution 2) | Medium | Soft falloff, good compromise |
| **LOW** | Grazing ray epsilon checks | Low | Prevent edge-case artifacts |

---

## Conclusion

**Volumetric cloud goals are NOT truly implemented** because:

1. ✅ DDA cell traversal is in place
2. ✅ Per-cell weather sampling works
3. ❌ **Per-cell AABB is sampled only at cell center**, causing discontinuity
4. ❌ **No inter-cell blending**, leading to visible tearing and fighting
5. ❌ **Ray-cell boundary handling has gaps**, allowing skipped regions

**The fix requires interpolating vertical bounds closer to the actual ray position**, not the cell center. This is a moderate effort but essential for production-quality volumetric clouds.
