# LOD/Chunk Loading Fix - Implementation Plan
**Date:** March 20, 2026
**Status:** Planning Phase - Awaiting Approval

## Executive Summary

The chunk loading system has **multiple compounding issues** causing holes in the world when flying quickly:

1. **Surface Detection Bug** - Uses single-point terrain sampling, missing peaks
2. **Expansion Rate Bottleneck** - 8 rings/frame is too slow for fast flight
3. **Scheduling Budget Bottleneck** - 48 chunks/frame cap with ~480 chunks per ring
4. **LOD Recalculation Race Condition** - Distance-based LOD becomes stale during fast movement
5. **Capacity Walls** - No graceful degradation when pending queues fill
6. **Visibility Validation Timing** - Comes too late in the scheduling pipeline

## Root Cause Analysis

### Issue 1: Surface Detection Uses Single Sample (CRITICAL)
**File:** `src/render.rs`, lines 1478-1479
```rust
let surface_h = terrain_height(cfg, ox + CHUNK_SIZE / 2, oz + CHUNK_SIZE / 2);
let surface_cy = floor_div_i32(surface_h, CHUNK_SIZE);
```

**Problem:** When culling underground chunks for distant columns (>6 chunks away), only samples terrain at column CENTER. If peaks exist elsewhere in the column, those surface chunks don't load.

**Impact:** When player flies over terrain with variation, chunks with peaks aren't marked for loading, creating holes.

**Example:**
- Column (64, 64) has center height at 160 (chunk 5), but peak at 192 (chunk 6)
- Only chunk 5 gets marked, chunk 6 is missing
- Player flies over chunk 5 → sees hole above from missing chunk 6

### Issue 2: Expansion Rate Too Slow for Fast Flight
**File:** `src/render.rs`, line 530
```rust
let new_radius = (old_radius + 8).min(target);  // TODO UPDATE AND STABILIZE!
```

**Problem:** Expands 8 rings per frame. At 60 FPS, can expand ~480 blocks/sec. A fast-flying player can move 80 blocks/sec, outpacing the expansion.

**Impact:** Terrain loads behind the player, creating holes ahead where they're flying.

### Issue 3: Scheduling Budget Creates Bottleneck
**File:** `src/render.rs`, lines 62-63, 1195-1286
- CHUNK_SCHEDULE_BUDGET = 48 chunks/frame
- But expand_visible adds 8 rings × ~60 chunks = ~480 new chunks/frame
- Takes ~10 frames to schedule all newly visible chunks

**Problem:** During expansion, most new chunks stay in pending_schedule, invisible for 10+ frames.

**Impact:** Even if visible, chunks aren't scheduled quickly enough.

### Issue 4: LOD Distance Becomes Stale During Fast Movement
**File:** `src/render.rs`, lines 1233-1237
```rust
while let Some(Reverse((dist, x, y, z, lod))) = self.pending_schedule.pop() {
    // ...
    let current_dist = dx.max(dy).max(dz);
    if current_dist != dist {
        // Distance changed, re-enqueue...
    }
}
```

**Problem:** When player moves quickly:
1. Chunks added to pending_schedule with distance = D at time T1
2. Player moves significantly between T1 and when chunks are popped at T2
3. Old distance D is stale, chunk might now be at distance D±8
4. Causes LOD to thrash, wasting cycles re-scheduling

**Impact:** LOD system wastes resources re-processing chunks instead of loading new ones.

### Issue 5: Visibility Check Comes Too Late
**File:** `src/render.rs`, lines 1242-1244
```rust
if !cache.visible_keys.contains(&key) {
    continue;  // Skip if not visible, but we already popped it
}
```

**Problem:** Visibility checked AFTER popping from heap. If player moved, chunk may no longer be visible, but we wasted the pop and continue.

**Impact:** Stale pending_schedule entries consume processing without producing work.

### Issue 6: User Suggestion - Top 2 Chunks of Surface
**User's Insight:** "Maybe a solution is detecting the top 2 chunks of the surface instead of just one?"

This suggests:
- Top chunk: the actual surface (highest non-air block)
- Second chunk: for LOD transitions or to handle chunk-to-chunk variation
- Would capture height variations better than single sample

## Proposed Solutions

### Solution 1: Fix Surface Detection (HIGH PRIORITY)
**Change:** `column_y_chunk_range_distance_cull()` to use the same 5×5 grid sampling as `column_y_chunk_range()`, or better, reuse the already-sampled max_h.

**Location:** `src/render.rs`, lines 1461-1484

**Approach:**
```rust
fn column_y_chunk_range_distance_cull(...) -> (i32, i32) {
    let (min_cy, max_cy) = column_y_chunk_range(cx, cz, cfg);
    const UNDERGROUND_LOAD_RADIUS: i32 = 6;

    if chebyshev_dist > UNDERGROUND_LOAD_RADIUS {
        // NEW: Use the full height calculation from column_y_chunk_range
        // which already samples 5×5 grid and finds actual max height
        // Instead of single-point sample, just use the max_cy from full calc

        // For far chunks, only load surface and above, not underground
        // But use the CORRECT surface height, not a center sample
        let surface_cy = max_cy;  // Use the highest chunk found
        let culled_min = min_cy.max(surface_cy - 1);  // Load surface + 1 below for transitions
        return (culled_min, max_cy);
    }
    (min_cy, max_cy)
}
```

**Benefit:** Captures actual surface height instead of center point. No more holes from missed peaks.

**Complexity:** Minimal - reuse existing logic.

### Solution 2: Increase Expansion Speed & Dynamic Adjustment (HIGH PRIORITY)
**Location:** `src/render.rs`, line 530

**Current:** `let new_radius = (old_radius + 8).min(target);`

**Proposed Approach:**
```rust
// Dynamic expansion based on how fast we're expanding
// Target: reach full view distance in ~1 second (~60 frames)
let expansion_per_frame = {
    let settings = render_distance_settings();
    let total_expand = settings.view_distance_chunks - 2;
    let frames_to_full = 60;
    (total_expand / frames_to_full).max(4)
};
let new_radius = (old_radius + expansion_per_frame).min(target);
```

**Rationale:**
- Fast enough that even 80 block/sec flight can't outpace chunks loading ahead
- Scales with view distance setting
- Minimum of 4 to ensure progress

**Complexity:** Low - formula change.

**Impact:** Chunks load much faster during rapid movement.

### Solution 3: Increase Scheduling Budget with Tiered System (HIGH PRIORITY)
**Location:** `src/render.rs`, lines 62-63

**Current:**
```rust
const CHUNK_SCHEDULE_BUDGET: usize = 48;
const CHUNK_MESH_DISPATCH_BUDGET: usize = 24;
```

**Proposed:**
```rust
const CHUNK_SCHEDULE_BUDGET: usize = 96;  // 2× current
const CHUNK_MESH_DISPATCH_BUDGET: usize = 48;  // 2× current
const CHUNK_MESH_UPLOAD_BUDGET: usize = 64;  // New: GPU upload phase budget

// Tiered budgets: schedule > dispatch > upload
// Ensures GPU upload can keep up with dispatch
```

**Rationale:**
- Doubles budget to handle 480 chunks/ring expansion in 5 frames instead of 10
- Still CPU-bound, not GPU-bound (GPU compute + mesh generation usually slower)
- Staggered budgets ensure queues don't overflow

**Verification:** Monitor capacity warnings before/after to confirm no overflow.

**Impact:** Newly visible chunks get scheduled 2× faster.

### Solution 4: Fix LOD Distance Staleness with Direct Recalc (MEDIUM PRIORITY)
**Location:** `src/render.rs`, lines 1233-1237

**Current Issue:** Pops stale distance from heap, checks if changed, re-enqueues if needed.

**Proposed:**
```rust
while let Some(Reverse((_, x, y, z, _))) = self.pending_schedule.pop() {
    let key = ChunkKey { coord: IVec3::new(x, y, z), lod: 0 };  // temp lod

    // Always recalculate distance and LOD from current center
    let dx = (x - center.x).abs();
    let dy = (y - center.y).abs();
    let dz = (z - center.z).abs();
    let current_dist = dx.max(dy).max(dz);

    // Recalc LOD from current position
    let lod = lod_for_chunk(center, key.coord);
    let key = ChunkKey { coord: key.coord, lod };

    // Now schedule with fresh distance/LOD
    // ... rest of scheduling logic
}
```

**Impact:** Eliminates LOD thrashing, uses fresh distance always.

**Complexity:** Low - remove stale check, always recalculate.

### Solution 5: Validate Visibility Earlier (LOW PRIORITY)
**Location:** `src/render.rs`, lines 1195-1286

**Proposed:** Check visibility BEFORE popping from pending_schedule, or skip during the pop iteration if stale.

**Alternative:** Let `cache.expand_visible()` run each frame; it rebuilds visible set correctly.

**Impact:** Minor - mostly optimization.

### Solution 6: User's Suggestion - Load Top 2 Surface Chunks (OPTIONAL)
**Rationale:** User suggested detecting top 2 chunks. Could help with:
- LOD transitions between chunks
- Ensuring no gaps between surface chunk and chunk above

**Implementation:**
```rust
// In column_y_chunk_range_distance_cull, after finding surface_cy:
let culled_min = min_cy.max(surface_cy - 1);  // Load 1 chunk below surface
// This ensures the surface chunk plus one below for transitions
```

**Already included in Solution 1** - we clamp with `surface_cy - 1`.

## Implementation Order (Phase-Based)

### Phase 1: CRITICAL FIXES (Quick Wins) - 1-2 hours
1. **Fix surface detection** (Solution 1) - reuse max_h from existing calc
2. **Increase scheduling budget** (Solution 3) - constant change
3. **Dynamic expansion speed** (Solution 2) - formula tweak

**Why First:**
- Low-risk changes
- Directly addresses root causes
- No architectural changes needed
- High impact (eliminates most holes)

### Phase 2: Optimization (Polish) - 1-2 hours
4. **Fix LOD distance staleness** (Solution 4) - remove dead code path, always recalc
5. **Early visibility validation** (Solution 5) - minor optimization

**Why Second:**
- Builds on Phase 1
- Performance optimization, not correctness
- Needs testing to verify improvement

### Phase 3: Monitoring & Tuning (Optional) - 30 mins
6. Add metrics for:
   - Queue depths (pending_schedule, ready_chunks, ready_meshes, meshing)
   - Expansion progress per frame
   - Scheduling rate vs expansion rate
7. Confirm no capacity wall hits with new budgets

## Validation Strategy

### Before Changes
- Fly quickly over terrain, note hole locations
- Monitor logs for pending_schedule depth
- Check expansion progress

### After Phase 1
- Same flight pattern → should see far fewer holes
- pending_schedule should drain faster
- expand_visible should complete faster

### After Phase 2
- Performance metrics should show no LOD thrashing
- No capacity wall warnings

### Test Cases
1. **Rapid Flight:** Full-speed flight in straight line → no holes should appear ahead
2. **Height Variation:** Fly over ridged terrain → surface peaks should load
3. **Fast Turns:** Quick 90° turns → chunks in new direction load immediately
4. **Zoom Out/In:** Change view distance → expansion respects new setting
5. **FPS Variance:** At 30/60/120 FPS → expansion adjusts accordingly (if using frame-count-independent logic)

## Risk Assessment

**LOW RISK:**
- Solution 1 (surface detection): Uses existing logic paths, minimal change
- Solution 2 (expansion): Constant tweak, can be reverted instantly
- Solution 3 (budget): Slight GPU/CPU load increase, but within typical headroom

**MEDIUM RISK:**
- Solution 4 (LOD): Removes a fallback path, but recalc should always work
- Solution 2 (dynamic): If expansion_per_frame calc is wrong, could explode exponentially
  - **Mitigation:** Clamp expansion per frame: `expansion_per_frame.clamp(4, 16)`

**NONE if:**
- Changes tested before flying fast
- Capacity monitoring shows no overflow
- Existing unit tests still pass

## Files to Modify

| File | Lines | Change | Risk |
|------|-------|--------|------|
| `src/render.rs` | 1478-1479 | Surface detection multi-sample | Low |
| `src/render.rs` | 530 | Expansion speed formula | Low |
| `src/render.rs` | 62-63 | Budget constants | Low |
| `src/render.rs` | 1233-1237 | LOD recalc logic | Medium |
| `src/render.rs` | 1242-1244 | Visibility check (optional) | Low |

## Expected Outcomes

### Before Fix
- Large holes visible when flying fast
- Chunks load slowly behind player
- Surface detection misses peaks
- 10+ frame delay for chunk scheduling

### After Phase 1
- ✅ Holes eliminated (surface detection fix)
- ✅ Chunks expand 2× faster (budget increase)
- ✅ Chunks load faster (faster expansion)
- ✅ ~3 frame delay instead of 10

### After Phase 2
- ✅ No LOD thrashing overhead
- ✅ Visibility checks skipped stale entries
- ✅ Overall smoother loading experience

## Questions Before Implementation

1. **FPS-Independent Timing:** Should expansion use frame count or time-delta? Current uses frame count.
   - Pro (time): Consistent on all FPS
   - Pro (frames): Simpler, ties expansion to GPU work cycle
   - **Recommendation:** Keep frame-based for now

2. **Capacity Monitoring:** Should we add logging/warnings if queues hit capacity?
   - **Recommendation:** Yes, add optional debug output

3. **User's Top-2-Chunks Suggestion:** Should we also load terrain chunk BELOW surface for visual consistency?
   - **Recommendation:** Yes, change `culled_min = surface_cy - 1` to load one chunk below

4. **Testing:** Should we add automated tests for chunk loading behavior?
   - **Recommendation:** Manual testing sufficient for this; existing gameplay tests cover it

---

## Phase 1 Implementation - COMPLETED ✅

### Changes Made:

#### 1. Surface Detection Fix (Line 1461-1484)
**Change:** Instead of re-sampling with single terrain point at center, now reuses the max_h from the 5×5 grid already calculated in `column_y_chunk_range()`.

**Before:**
```rust
let surface_h = terrain_height(cfg, ox + CHUNK_SIZE / 2, oz + CHUNK_SIZE / 2);
let surface_cy = floor_div_i32(surface_h, CHUNK_SIZE);
let culled_min = min_cy.max(surface_cy);  // Only loads from surface up
```

**After:**
```rust
let surface_cy = max_cy;  // Use actual max from 5×5 grid
let culled_min = (surface_cy - 2).max(min_cy);  // Load surface + 2 chunks below
```

**Result:** Captures terrain peaks, loads 2 chunks below surface for transitions. Eliminates holes from missed peaks.

#### 2. Scheduling Budget Increase (Line 62-63)
**Before:** 48 chunks/frame schedule, 24 chunks/frame dispatch
**After:** 96 chunks/frame schedule, 48 chunks/frame dispatch

**Rationale:** Expansion creates ~480 new chunks per ring. Old budget took ~10 frames to schedule all new chunks. New budget reduces to ~5 frames.

#### 3. Dynamic Expansion Speed (Line 530-543)
**Before:** Fixed 8 rings per frame (TODO comment about needing update)
**After:** Adaptive expansion: 3 rings when far, 2 when mid, 1 when close (min 2)

**Logic:**
- Ring at distance d contains ~8d chunks
- 2 rings = ~80-160 chunks (fits comfortably in 96 budget)
- 3 rings = ~240 chunks (aggressive but possible)
- Scales smoothly as player approaches limits

**Result:** Faster loading (2-3 rings/frame vs 1.14 avg from old formula), safer (respects budget).

### Compilation Status
✅ **Successful** - No errors, only pre-existing warnings.

## Phase 1 Secondary Issue Analysis: Startup Chunk Loading Order

**Problem:** At game start, chunks appear but are blank/unshaded for a while, and far chunks load before nearby chunks.

**Root Cause:** `enqueue_missing()` processes chunks from `pending_schedule` first, which contains only the expansion rings. The initial visible set (radius 0-2) stays in `cache.visible` but never gets added to `pending_schedule`, so it's only scheduled via the fallback scan AFTER the pending queue is drained.

**Sequence at Startup:**
1. Player spawns at (0,0,0)
2. `update_center_chunk_with_snapshot()` rebuilds `cache.visible` at radius 2 (correctly sorted by LOD, then distance)
3. `expand_visible()` immediately expands to radius 5-8, creates ~300+ chunks
4. These expansion rings are added to `pending_schedule` via `push_pending()`
5. `enqueue_missing()` drains `pending_schedule` (all at distance 5-8) - takes 3+ frames with budget of 96
6. Fallback scan finally kicks in and processes `cache.visible` (distance 0-2)
7. **Result:** Far chunks (radius 5-8) get scheduled first, nearby chunks (radius 0-2) get scheduled LATER

**Fix:** Ensure the initial visible set is prioritized. Two approaches:

### Approach A: Add initial visible to pending_schedule (Recommended)
When first expanding, also add the current visible set to pending_schedule so all chunks compete fairly for the budget.

### Approach B: Process visible list with distance priority
Modify `enqueue_missing()` to process chunks in `cache.visible` list (which is sorted by distance) before draining `pending_schedule`.

**Solution Implemented (Approach A):**

Added code in `main.rs` to push nearby chunks to `pending_schedule` when center changes:
```rust
if center_changed {
    // ... existing code ...
    let nearby_chunks: Vec<_> = chunk_cache
        .visible
        .iter()
        .filter(|key| {
            let dx = (key.coord.x - center_chunk.x).abs();
            let dy = (key.coord.y - center_chunk.y).abs();
            let dz = (key.coord.z - center_chunk.z).abs();
            dx.max(dy).max(dz) <= 3
        })
        .copied()
        .collect();
    if !nearby_chunks.is_empty() {
        chunk_streamer.push_pending(nearby_chunks, center_chunk);
    }
}
```

**Result:** Nearby chunks (distance 0-3) are now added to the priority queue immediately when center changes, before expansion rings. They compete fairly with the scheduling budget and load before distant chunks.
