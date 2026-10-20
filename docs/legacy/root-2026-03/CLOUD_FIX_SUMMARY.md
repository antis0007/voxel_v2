# Cloud Shader Fix: Flickering & Layer Fighting — IMPLEMENTED

## Problem

**Only one cloud layer renders at a time, and they flicker randomly.**

This was caused by a **binary 0.005 threshold** in each layer's per-cell AABB function:

```wgsl
// OLD (broken):
if w.low_amount <= 0.005 { return degenerate_aabb; }
```

### Why it flickered:

1. **Weather changes per frame**: Wind advection shifts the weather sample point by ~4m/frame at moderate speeds.
2. **Hash field is continuous**: `w.low_amount` is computed from smoothstep(hash), which varies smoothly across the world.
3. **Binary threshold**: Crossing `0.005` caused the entire 24km cell to snap from "has clouds" to "no clouds" in a single frame.
4. **Per-layer independence**: Each layer uses different wind multipliers (12×, 13×, 15×), so they independently cross the threshold at different times → each layer flickers independently → appearance of layer fighting.

## Solution

Replaced the hard `0.005` threshold with a **smooth fade** over a 4% range:

```wgsl
// NEW (fixed):
let low_fade = smoothstep(0.0, 0.04, w.low_amount);
if low_fade <= 0.0 { return degenerate_aabb; }
// ... compute aabb normally, then scale thickness:
aabb.y_max = aabb.y_min + cell_thick * low_fade;
```

### Why this fixes it:

- **Smooth boundary**: Cells now fade in/out over `amount ∈ [0.0, 0.04]` (4% range) instead of snapping at one point.
- **Spatial gradient**: Given the hash field's spatial variation, this 4% range spans ~1–2km, making presence boundaries smooth and continuous.
- **Wind invariance**: Wind advection at realistic frame rates (~4m/frame) produces tiny changes in `low_fade` — no more visible frame-to-frame flickering.
- **Layer agreement**: Even with different wind offsets, the smooth fade is much less sensitive to threshold crossing, so layers mostly agree on whether clouds are present.

## Changes Made

**File**: `src/shaders/cloud_shader.wgsl`

### `cell_low_aabb` (lines 170–191)
- Added: `let low_fade = smoothstep(0.0, 0.04, w.low_amount);`
- Changed: `if low_fade <= 0.0` (was `if w.low_amount <= 0.005`)
- Changed: `aabb.y_max = aabb.y_min + cell_thick * low_fade;` (was without `* low_fade`)

### `cell_mid_aabb` (lines 193–209)
- Added: `let mid_fade = smoothstep(0.0, 0.04, w.mid_amount);`
- Changed: `if mid_fade <= 0.0` (was `if w.mid_amount <= 0.005`)
- Changed: `aabb.y_max = aabb.y_min + base_thick * mix(0.3, 1.0, w.mid_amount) * mid_fade;` (was without `* mid_fade`)

### `cell_high_aabb` (lines 211–226)
- Added: `let high_fade = smoothstep(0.0, 0.04, w.high_amount);`
- Changed: `if high_fade <= 0.0` (was `if w.high_amount <= 0.005`)
- Changed: `aabb.y_max = aabb.y_min + base_thick * mix(0.3, 1.0, w.high_amount) * high_fade;` (was without `* high_fade`)

## Testing

After deploying, verify:

1. ✅ **No flickering**: Observe clouds at rest or in slow motion — no frame-to-frame on/off flashing
2. ✅ **All layers visible**: Enable all three cloud layers (low, mid, high) and confirm they all render simultaneously
3. ✅ **Layer separation**: Low layer should be below mid, which is below high — no overlap within a layer
4. ✅ **Wind behavior**: Increase wind speed and observe clouds drift smoothly, not flicker

## Technical Details

- **`smoothstep(0.0, 0.04, x)`**: Returns 0 for x ≤ 0, 1 for x ≥ 0.04, and smooth Hermite interpolation between
- **Fade applied to thickness, not base Y**: This keeps cloud base consistent while thinning toward zero, matching natural cloud fading
- **No density sampler changes needed**: Density functions receive the original `cell_thick`, but the faded AABB bounds clamp the march region, naturally reducing sample count near the fade zone

## Related Notes

- Boundary tearing (sharp seams at cell edges) is a separate issue analyzed in `CLOUD_ANALYSIS.md` — not fixed by this change but documented for future work
- Current implementation still samples weather at the cell center (not at actual ray position), which can cause discontinuities at 24km boundaries — a medium-priority improvement
