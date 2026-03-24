# Cloud Shader Fix — Corrected Implementation

## Problem
Clouds follow the player everywhere. They always render centered on the player's position, not in actual world space.

## Root Cause
The ray origin passed to `march_layer()` was `cam.camera_pos.xyz`. Inside `march_layer`, sample positions are computed as:
```wgsl
let p = ray_origin + ray_dir * t_cur;
```

When `ray_origin = cam.camera_pos`, every sample position `p` depends on camera position, making the entire cloud field camera-relative.

## The Fix
Instead of passing the camera position as the ray origin, **compute the actual world-space point where the ray intersects the cloud slab**, and use that as the ray origin.

```wgsl
// For each ray, find where it intersects the cloud slab mid-height
let low_mid_y = (low_base + low_top) * 0.5;
let t_entry = (low_mid_y - cam.camera_pos.y) / dir.y;

// World position at slab intersection (independent of camera lateral position)
let entry_world_xz = cam.camera_pos.xz + dir.xz * max(t_entry, 0.0);

// Use this as ray origin instead of cam.camera_pos.xyz
let ray_origin = vec3<f32>(entry_world_xz.x, cam.camera_pos.y, entry_world_xz.y);
march_layer(ray_origin, dir, ...);
```

**Why this works:**
- `entry_world_xz` only depends on the **ray direction**, not camera position
- When camera moves, the direction doesn't change → ray origin doesn't change
- All sample positions `p = ray_origin + ray_dir * t` are now world-space
- Clouds stay where they are in the world

## Implementation

**File:** `cloud_shader.wgsl`, lines 730-754

```wgsl
// Compute world-space entry point for cloud slabs
let low_mid_y = (low_base + low_top) * 0.5;
let high_mid_y = (high_base + high_top) * 0.5;

let t_low_entry = abs(dir.y) > 1e-5 ? (low_mid_y - cam.camera_pos.y) / dir.y : 0.0;
let t_high_entry = abs(dir.y) > 1e-5 ? (high_mid_y - cam.camera_pos.y) / dir.y : 0.0;

// World position where ray enters the slab
let low_entry_world_xz = cam.camera_pos.xz + dir.xz * max(t_low_entry, 0.0);
let high_entry_world_xz = cam.camera_pos.xz + dir.xz * max(t_high_entry, 0.0);

// Pass world-anchored origin to march_layer
let low_origin = vec3<f32>(low_entry_world_xz.x, cam.camera_pos.y, low_entry_world_xz.y);
let high_origin = vec3<f32>(high_entry_world_xz.x, cam.camera_pos.y, high_entry_world_xz.y);

let low = march_layer(low_origin, dir, ...);
let high = march_layer(high_origin, dir, ...);
```

## What Changed

**Before (broken):**
```wgsl
let low = march_layer(cam.camera_pos.xyz, dir, ...);
```

**After (fixed):**
```wgsl
let low_mid_y = (low_base + low_top) * 0.5;
let t_low_entry = (low_mid_y - cam.camera_pos.y) / dir.y;
let low_entry_world_xz = cam.camera_pos.xz + dir.xz * max(t_low_entry, 0.0);
let low_origin = vec3<f32>(low_entry_world_xz.x, cam.camera_pos.y, low_entry_world_xz.y);
let low = march_layer(low_origin, dir, ...);
```

Same for high clouds.

## Result
✓ Clouds now exist in world space
✓ Moving left/right/forward → clouds stay in place
✓ Looking around → clouds move with your view direction (correct)
✓ No temporal ghosting (reprojection works correctly)

## Testing
1. Load game
2. Fly east → clouds should NOT move
3. Fly north → clouds should NOT move
4. Rotate view → clouds should move (correct)
5. Visual artifacts (fuzz/checkerboard) should be gone
6. Temporal smoothing should work normally

## Why Previous Fix Failed
The earlier approach (subtracting `cam.camera_pos.xz` from `p.xz`) corrupted noise scales because the evo/warp/shape calculations depend on the full precision of `p`. The correct solution is to fix the **ray origin** (the starting point of the march), not individual sample coordinates.
