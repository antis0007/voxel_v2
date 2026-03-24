# Shadow Cascade Plan

## Objective

Replace the current single directional shadow map with a stable 4-cascade system that materially improves near-camera terrain and voxel shadow quality while keeping the renderer architecture clean enough to evolve later.

This checkpoint is about stable cascaded shadows, not about virtual shadow maps, atlas packing, or full shadow-system finality.

## Current Problems

The current path in `src/render.rs` has four structural issues:

1. One shadow transform must cover too much world space, so near-camera texel density is poor.
2. The single-map fit shifts noticeably under camera motion, so shadows shimmer during strafing.
3. One bias pair is shared for all distances, so near and far quality fight each other.
4. The entire shadow system lives inside `render.rs`, which is already too large.

## Required Outcomes

- Four cascades backed by a depth texture array.
- Stable split selection using a practical split scheme.
- Stable XY fitting using texel snapping.
- Separate per-cascade bias and filter configuration.
- Per-cascade caster selection.
- Debug view for cascade index.
- Debug view for shadow texel density/utilization.
- No regression of current pass ordering.

## Non-Goals

- No virtual shadow maps.
- No shadow atlas.
- No point-light or spot-light shadow rewrite.
- No cloud-shadow unification with visible clouds in this checkpoint.
- No mandatory contact-shadow implementation in the base milestone.

## Target Module Shape

Create `src/render/shadow.rs` and move shadow-specific code behind a dedicated owner:

```text
src/render/
  shadow.rs
```

The module should expose a single high-level owner, for example:

```rust
pub struct ShadowRenderer { ... }
```

Suggested owned sub-structures:

- `ShadowConfig`
- `CascadeConfig`
- `CascadeFrameState`
- `ShadowUniform`
- `CascadeDrawList`
- `ShadowDebugInfo`

`render.rs` should only:

- create the module
- feed it camera/light/mesh state
- let it render before the world pass
- bind its resources for terrain shading

## Resource Model

### Depth Resources

Use one `Depth32Float` texture array with four layers:

- resolution: `2048 x 2048 x 4` as the first target
- one array view for sampling in the world pass
- one per-layer `D2` view for rendering each cascade

Rationale:

- texture arrays are simpler than atlases for filtering, edge handling, and future debug
- they avoid manual atlas border problems
- they fit the requested architecture directly

### Sampler

Keep a comparison sampler for shadow tests.

The border/edge policy should be explicit. Sampling outside a cascade should return fully lit, not undefined edge garbage.

### Uniform Layout

Replace the single `ShadowUniform` with a cascade-aware layout.

Recommended shape:

```text
ShadowUniform
  cascade_count
  shadow_map_size
  global_blend_width
  world_scale
  cascades[4]

CascadeData
  light_view_proj
  split_depths          // near, far, blend_start, blend_end
  texel_world_size
  bias                  // constant, slope, normal offset if used
  filter                // kernel radius, softness
  debug_metrics         // utilization or padding metadata
```

The exact packing is flexible, but it must provide enough data for:

- cascade selection
- cascade blending
- per-cascade PCF
- per-cascade bias
- debug output

## Split Strategy

Use the practical split scheme from PSSM:

`C_i = lambda * n * (f / n)^(i / m) + (1 - lambda) * (n + (f - n) * i / m)`

Recommended initial tuning:

- cascade count: `4`
- `lambda`: `0.65`
- near distance: camera near plane, clamped to a sane minimum
- shadow distance: capped independently from the full camera far plane

Recommended initial shadow distance policy:

- `shadow_distance = min(camera_far_plane, max(192 m, min(visible_world * 0.85, 512 m))) * world_scale`

Why this policy:

- near cascades stay dense
- far cascades remain useful without trying to cover the full terrain render distance
- stability and near quality are prioritized over extreme distance coverage

The expected default split ends should land roughly in the range:

- cascade 0: ~40 to 50 m
- cascade 1: ~90 to 100 m
- cascade 2: ~170 to 190 m
- cascade 3: ~400 to 500 m

The exact result depends on camera near/far and world scale, but the first cascade must stay tightly focused on player-near space.

## Stable Cascade Fitting

Use a stable fit, not an aggressively tight per-frame crop.

Recommended fitting algorithm per cascade:

1. Build the camera frustum slice corners for that cascade.
2. Compute a bounding sphere for the slice in world space.
3. Build a light view matrix from the sun direction and slice center.
4. Transform the sphere center into light space.
5. Use the sphere radius for stable XY half extents.
6. Snap the XY center to shadow texel increments.
7. Fit Z conservatively using slice corners plus caster padding.

Why a sphere-based XY fit:

- it is more stable under camera rotation than a raw AABB fit
- it reduces shimmer from small frustum orientation changes
- it still allows texel snapping and conservative coverage

### Texel Snapping

Texel snapping is mandatory for stability.

For each cascade:

- compute `texel_world_size = (2 * radius) / shadow_map_size`
- snap the light-space XY center to that increment

This keeps the projected shadow grid from sliding continuously over the world during small camera motion.

### Z Fit

Z fit should be conservative and independent from XY stability.

Initial plan:

- use the cascade slice corners transformed to light space
- expand along light-space Z by configurable caster and receiver margins
- bias the near plane toward slightly more headroom instead of tight clipping

Later improvements can replace this with better caster/receiver analysis, but the first milestone should already move Z fitting out of the single-map heuristic currently embedded in `render.rs`.

## Caster Selection

Keep the current chunked draw path, but make selection per cascade explicit.

Recommended first-pass culling model:

1. Derive a conservative world-space or light-space bound for each cascade.
2. Test chunk bounds against that bound.
3. Build one draw list per cascade.

Suggested implementation structure:

- `fn build_cascade_draw_lists(chunk_cache: &ChunkCache, cascades: &[CascadeFrameState; 4]) -> [Vec<IVec3>; 4]`

Do not keep a single shared shadow draw list once the cascade system exists.

Future work can add:

- receiver-based caster extrusion
- per-cascade LOD-aware shadow caster selection
- terrain-only or object-only special cases

Those are not required for the first checkpoint.

## Render Path

The shadow pass remains before the world pass, but it becomes four sub-passes over one texture array.

Recommended execution:

1. Update cascade states if camera/light/world changed enough.
2. If dirty, render four depth passes, one per array layer.
3. Sample the depth array in the world pass.

The renderer may still cache shadow updates based on:

- cascade state changes
- mesh revision changes

The update cache should live in `ShadowRenderer`, not in `render.rs`.

## World Shader Contract

Change the world shader shadow bindings to:

- shadow uniform buffer with all cascades
- `texture_depth_2d_array`
- comparison sampler

### Cascade Selection

Cascade selection should be based on linear eye depth, not Euclidean world distance.

To support this cleanly, extend `CameraUniform` with one of:

- a view matrix
- camera forward basis data
- explicit linear depth reconstruction helpers

Do not choose cascades by raw `distance(camera_pos, world_pos)` because wide FOV and off-axis geometry will cause wrong splits near screen edges.

### Cascade Blending

Blend between adjacent cascades over an overlap band. Recommended first pass:

- overlap width: `10%` of each cascade depth range, or a minimum world-space width
- only blend between the chosen cascade and the next farther one

The transition should be hard to notice in motion.

### Per-Cascade Bias and Filter

Use per-cascade tuning instead of one global pair.

Suggested initial direction:

- cascade 0: smallest constant/slope bias, tightest PCF radius
- cascade 1: slightly larger bias and filter
- cascade 2: moderate bias/filter
- cascade 3: largest bias/filter

Example seed values to start tuning from:

- cascade 0: constant `0.00004`, slope `0.00030`, filter `1.0 texel`
- cascade 1: constant `0.00006`, slope `0.00045`, filter `1.25 texel`
- cascade 2: constant `0.00010`, slope `0.00080`, filter `1.75 texel`
- cascade 3: constant `0.00016`, slope `0.00120`, filter `2.25 texel`

These are starting points, not final truth. The important decision is structural: the system must support tuning each cascade independently.

## Debug Views

Add at least two new debug surfaces:

### Shadow Cascade Index

Per-pixel false color showing:

- active cascade
- optional blend overlay near transitions

This is required for validating split placement and blend regions.

### Shadow Texel Density / Utilization

Per-pixel false color showing a combination of:

- world meters per shadow texel for the sampled cascade
- optional utilization/edge metric for how efficiently the cascade projection is used

This is required for validating that cascade 0 is actually denser than the old single-map path.

These debug views should be routable through the existing renderer debug selection path. Do not hide them behind temporary local constants.

## Optional Stretch Goal: Contact Shadows

This is optional and should not block the base shadow cascade milestone.

If time remains after cascades are stable:

- add a very short-range screen-space contact shadow pass using the resolved depth buffer and sun direction
- apply it only to the direct sun term
- keep it behind a toggle

Do not attempt a large unified contact-shadow system inside this checkpoint.

## Implementation Stages

### Stage 1: Module Extraction

- move current single-shadow code into `render::shadow`
- keep behavior equivalent

### Stage 2: Resource Upgrade

- switch to depth texture array
- define cascade-aware uniforms
- keep rendering one layer if needed while wiring

### Stage 3: Cascade Fitting and Sampling

- implement split computation
- implement stable fitting and texel snapping
- render all four layers
- sample the correct cascade in `world_shader.wgsl`

### Stage 4: Quality and Debug

- tune blend zones
- tune per-cascade bias/filter
- add debug views
- validate motion stability

### Stage 5: Optional Contact Shadowing

- only if the previous stages already satisfy the visual requirements

## Validation Checklist

Visual validation must confirm:

- near-camera voxel edges are materially sharper than the old path
- ground contact shadows do not swim while strafing
- cascade transitions are difficult to notice during forward movement and rotation
- far cascades remain acceptable even if softer than cascade 0

Structural validation must confirm:

- shadow code no longer grows in `render.rs`
- all bind group changes are mirrored in WGSL
- per-cascade state is explicit, not hidden in ad hoc arrays

## Deferred Work

The following items are intentionally deferred:

- virtual shadow maps
- clipmapped directional shadow caches
- cloud shadow unification with resident clouds
- object-specific shadow LOD policies
- temporal shadow denoising

The current checkpoint should solve the actual near-field problem first.

## References

- NVIDIA GPU Gems 3, Chapter 10: practical split scheme for cascaded shadows
- Unreal Engine documentation: directional virtual shadow map clipmaps as a future architecture reference, not the current target
