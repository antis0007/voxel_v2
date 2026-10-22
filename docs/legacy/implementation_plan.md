# Volumetric Cloud and Weather Engine Overhaul Plan
(MAY BE OUTDATED, BUT CONTAINS GOOD IDEAS!)
This implementation plan orchestrates the transition to the target architecture across four distinct subsystems and rendering gates. It aligns with the long-term "from-scratch" design goal for real 3D clouds, world-scale weather, and physically based rendering.

## Goal Description
Transform the current transitional renderer into a modular, production-ready system with stable shadow cascades, runtime-owned atmosphere products, decoupled weather simulation, and sparse voxel clouds using empty-space skipping.

## User Review Required
> [!IMPORTANT]
> The implementation is carefully phased into 4 sequential gates (A/B/C/D). Please review these proposed changes and the specific module splits to ensure they align with the master engine plans. Once approved, we will begin execution on Gate A (Shadow Cascades).

## Proposed Changes

### Gate A: Shadow Cascades (Stabilize Near-Field Shadows)
Migrate off the single-directional shadow map heuristic.
#### [NEW] [src/render/shadow.rs](file:///c:/dev/voxel_v2/src/render/shadow.rs) (Shadow system owner)
- Implements 4-cascade depth texture array (`2048x2048x4`).
- Stable sphere-based XY fitting with texel snapping.
- Prepares shadow uniforms with splitting thresholds for seamless [world_shader.wgsl](file:///c:/dev/voxel_v2/src/shaders/world_shader.wgsl) sampling.

### Gate B: Atmosphere Runtime (Hillaire-style Subsystem)
Establish a dedicated atmospheric owner yielding proper LUTs rather than relying on analytical fog properties.
#### [NEW] `src/render/atmosphere_params.rs`
#### [NEW] [src/render/atmosphere.rs](file:///c:/dev/voxel_v2/src/render/atmosphere.rs) (Pass/Texture Owner)
#### [NEW] `src/shaders/transmittance_lut.wgsl` (`256x64 rgba16float`)
#### [NEW] `src/shaders/multiscatter_lut.wgsl` (`32x32 rgba16float`)
#### [NEW] `src/shaders/skyview_lut.wgsl` (`192x108 rgba16float`)
#### [NEW] `src/shaders/aerial_perspective.wgsl` (`32x32x24 rgba16float` volume)
#### [MODIFY] [src/shaders/world_shader.wgsl](file:///c:/dev/voxel_v2/src/shaders/world_shader.wgsl) / [src/shaders/cloud_shader.wgsl](file:///c:/dev/voxel_v2/src/shaders/cloud_shader.wgsl) (Wire to use atmosphere products)

### Gate C: Weather Simulation Foundation
Move from procedurally generated "preset noise" to a persistent world-grid simulation with fixed-step updates.
#### [MODIFY] [src/world/weather.rs](file:///c:/dev/voxel_v2/src/world/weather.rs) (Refactor into simulation owner)
#### [NEW] `src/world/weather_grid.rs` (Tile scheduler, 16x16 chunk updates)
#### [NEW] `src/world/wind_field.rs` (Layered wind)
#### [NEW] `src/world/humidity_field.rs`
#### [NEW] `src/world/uplift_field.rs`
#### [NEW] `src/shaders/weather_step.wgsl` (Optional compute, but CPU acceptable first)

### Gate D: Sparse Voxel Cloud Storage / Bricks
Introduce voxel clipmaps to realize true 3D explorable clouds.
#### [NEW] `src/render/cloud_clipmaps.rs`
#### [NEW] `src/render/cloud_bricks.rs` (Residency tables)
#### [NEW] `src/render/brick_streamer.rs`
#### [NEW] `src/shaders/density_synthesis.wgsl` (Weather macro mass + Genus Profiles + Worley Erosion)
#### [NEW] `src/shaders/sdf_builder.wgsl`
#### [NEW] `src/shaders/light_cache.wgsl`
#### [NEW] `src/render/reprojection.rs` & `history_buffers.rs`
#### [MODIFY] [src/shaders/cloud_shader.wgsl](file:///c:/dev/voxel_v2/src/shaders/cloud_shader.wgsl) -> `cloud_render.wgsl` (Brick traversal & empty-space skipping)

## Verification Plan

### Automated / Debug Views
Add and leverage the following debug overlays:
- **Shadows**: Cascade Index, Texel Utilization.
- **Atmosphere**: Transmittance, Sky-View, Multiscatter, Aerial Perspective slice view.
- **Weather**: Humidity, Uplift, Storm Organization, Layered Wind.
- **Clouds**: Brick bounding boxes, step count heatmaps.

### Manual Verification
Visual and temporal stability must hold up against extreme cases across the frames:
- Sun movement at the horizon matches physical silver-lining and accurate cloud multi-scattering.
- Fast, low-altitude flights avoid ghosting and temporal smearing due to proper motion vectors.
- Moving shadows transition seamlessly between cascading shadow maps.
- World-scale chunks update incrementally (16x16 tiles) to prevent lag spikes.
