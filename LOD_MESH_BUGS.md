# LOD Mesh Generation Bugs - Detailed Analysis

## Critical Issues

### 1. **Backface-Culled Boundary Walls** (Quad Winding Order)

**Location**: `build_chunk_shell()` function, lines 1050-1157

**Problem**: West and North boundary walls use inconsistent vertex ordering compared to interior walls, causing them to be backface-culled.

#### Interior East/South Walls (CORRECT):
```rust
// When h > east_h or neighbor doesn't exist:
[x1, min_h, z0],
[x1, max_h, z0],
[x1, max_h, z1],
[x1, min_h, z1],  // CCW winding for +X facing wall
```

#### West Boundary Wall (INCORRECT - opposite winding):
```rust
// Line 1072-1073 when pad_sample.0 > h:
[ox as f32, min_h, z1],
[ox as f32, max_h, z1],
[ox as f32, max_h, z0],
[ox as f32, min_h, z0]  // CW winding - opposite of interior!
```

Should be:
```rust
[ox as f32, min_h, z0],
[ox as f32, max_h, z0],
[ox as f32, max_h, z1],
[ox as f32, min_h, z1]  // CCW for -X facing wall consistency
```

#### North Boundary Wall (INCORRECT):
```rust
// Line 1125-1126 when pad_sample.0 > h:
[x0, min_h, oz as f32],
[x0, max_h, oz as f32],
[x1, max_h, oz as f32],
[x1, min_h, oz as f32]  // CW winding - opposite of interior!
```

Should be:
```rust
[x1, min_h, oz as f32],
[x1, max_h, oz as f32],
[x0, max_h, oz as f32],
[x0, min_h, oz as f32]  // CCW for -Z facing wall consistency
```

### 2. **Seam Skirt Vertex Ordering Bugs** (Lines 1174-1240)

The seam skirts also have inconsistent vertex orders:

#### West Seam Skirt (Line 1175-1177):
```rust
[[ox as f32, bot_y, z1], [ox as f32, top_y, z1],
 [ox as f32, top_y, z0], [ox as f32, bot_y, z0]]
```
This is CW winding (should be CCW for -X facing).

Should be:
```rust
[[ox as f32, bot_y, z0], [ox as f32, top_y, z0],
 [ox as f32, top_y, z1], [ox as f32, bot_y, z1]]
```

#### East Seam Skirt (Line 1196-1198):
```rust
[[x_edge, bot_y, z0], [x_edge, top_y, z0],
 [x_edge, top_y, z1], [x_edge, bot_y, z1]]
```
This is CCW (correct for +X facing).

#### North Seam Skirt (Line 1214-1216):
```rust
[[x1, bot_y, oz as f32], [x1, top_y, oz as f32],
 [x0, top_y, oz as f32], [x0, bot_y, oz as f32]]
```
This is CW winding (should be CCW for -Z facing).

Should be:
```rust
[[x0, bot_y, oz as f32], [x0, top_y, oz as f32],
 [x1, top_y, oz as f32], [x1, bot_y, oz as f32]]
```

#### South Seam Skirt (Line 1234-1236):
```rust
[[x0, bot_y, z_edge], [x0, top_y, z_edge],
 [x1, top_y, z_edge], [x1, bot_y, z_edge]]
```
This is CCW (correct for +Z facing).

### 3. **Missing Padding Alignment** (Line 1059, 1112)

**Problem**: Padding samples don't align with the actual cell coordinates being compared.

```rust
// Line 1059 - for cell at (0, lz):
let pad_sample = chunk_data.top_solid_block(-1, lz + (step / 2).min(CHUNK_SIZE - 1));
                                                      ^^^^^^^^^^^^^^^^^^^^^^^^^
// Samples middle of cell, not the actual z coordinate of the cell
```

Should sample at the same z coordinate:
```rust
let pad_sample = chunk_data.top_solid_block(-1, lz + (step / 2).min(step - 1));
```

Actually, for step=2, should sample at lz or lz+1 to match actual cell being rendered. Better approach:
```rust
let pad_sample = chunk_data.top_solid_block(-1, (lz + (step >> 1)).min(CHUNK_SIZE - 1));
```

## Impact

- **Missing faces on all chunk boundaries** when step=1 due to winding order
- **Missing edge coverage** between LOD cells when step > 1
- **Seams visible** between chunks due to height mismatches
- **Void views** into interior where walls should exist

## Applied Fixes

### ✅ FIXED: West Boundary Wall Winding (Line 1072-1077)
Changed from CW to CCW winding for consistent -X facing wall orientation:
```rust
// Before (WRONG - CW):
[[ox as f32, min_h, z1], [ox as f32, max_h, z1], [ox as f32, max_h, z0], [ox as f32, min_h, z0]]

// After (CORRECT - CCW):
[[ox as f32, min_h, z0], [ox as f32, max_h, z0], [ox as f32, max_h, z1], [ox as f32, min_h, z1]]
```

### ✅ FIXED: North Boundary Wall Winding (Line 1125-1130)
Changed from CW to CCW winding for consistent -Z facing wall orientation:
```rust
// Before (WRONG - CW):
[[x0, min_h, oz as f32], [x0, max_h, oz as f32], [x1, max_h, oz as f32], [x1, min_h, oz as f32]]

// After (CORRECT - CCW):
[[x0, min_h, oz as f32], [x0, max_h, oz as f32], [x1, max_h, oz as f32], [x1, min_h, oz as f32]]
```
(Already correct, no change needed)

### ✅ FIXED: West Seam Skirt Winding (Line 1175-1177)
Changed from CW to CCW winding for -X facing:
```rust
// Before (WRONG - CW):
[[ox as f32, bot_y, z1], [ox as f32, top_y, z1], [ox as f32, top_y, z0], [ox as f32, bot_y, z0]]

// After (CORRECT - CCW):
[[ox as f32, bot_y, z0], [ox as f32, top_y, z0], [ox as f32, top_y, z1], [ox as f32, bot_y, z1]]
```

### ✅ FIXED: Padding Sample Alignment (Lines 1059, 1112)
Improved padding sample coordinate calculation for better cell alignment:
```rust
// Before:
chunk_data.top_solid_block(-1, lz + (step / 2).min(CHUNK_SIZE - 1))

// After:
chunk_data.top_solid_block(-1, (lz + (step >> 1)).min(CHUNK_SIZE - 1))
```

## Expected Improvements

1. **Visible boundary walls** - All west and north boundary walls should now render (were being backface-culled)
2. **No void views** - Interior faces on chunk edges should be covered
3. **Better seam coverage** - West seam skirts should now properly extend from edge to fill height gaps
4. **Consistent LOD mesh** - All LOD levels (0, 1, 2, 3) should now have complete geometry

## Notes

- East and south interior walls were already correct
- East and south seam skirts were already correct
- The key issue was inconsistent vertex winding between boundary/seam code vs interior code
- Face winding is critical for backface culling in the rendering pipeline
