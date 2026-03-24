# Atmosphere + Cloud Rewrite Status

This document is the authoritative implementation notebook for the current rewrite of the atmosphere, weather, cloud, and composition stack.

It serves four purposes:

1. Record what has already been changed in the codebase.
2. State the target architecture clearly enough that future implementation is constrained by explicit rules rather than local convenience.
3. Capture the structural reasoning behind current decisions, including tradeoffs and known debt.
4. Define the next checkpoints so work can continue without rediscovery.

## Executive Summary

The current codebase is no longer treated as "a sky shader that happens to draw clouds." The rewrite has started forcing real subsystem seams into the renderer, but it is still mid-transition.

What is now true:

- Clouds are no longer composited inside the sky shader.
- Terrain now renders before the cloud pass, which gives the cloud pass access to real scene depth.
- The cloud pass now clips against scene depth instead of binding the depth texture and ignoring it.
- Weather is no longer invented entirely in the fragment shader. There is now a persistent CPU weather field uploaded as GPU textures.
- Terrain lighting now has an active path that consumes a shared cloud-shadow field derived from the weather field instead of depending exclusively on terrain-side cloud re-simulation.
- Renderer debug modes now reach weather, clouds, and the first atmosphere LUT surfaces end to end: UI -> runtime state -> shader output.
- `render::atmosphere` is no longer only shader assembly glue. It now owns explicit atmosphere textures, a bind group, LUT refresh policy, and a dedicated runtime sky mode.

What is still false:

- This is not yet a LUT-backed atmosphere subsystem.
- This is not yet a sparse cloud brick / clipmap system.
- This is not yet a proper weather simulation with fronts, layered wind, pressure tendency, or persistent storm organization.
- This is not yet a unified cloud shadow cache generated from the same resident 3D cloud representation that the cloud renderer samples.

This means the project is in a structurally improved but transitional state.

## Planning Set

Implementation planning for the next rewrite stage now lives in:

- [Renderer Rewrite Execution Plan](./RENDERER_REWRITE_EXECUTION_PLAN.md)
- [Shadow Cascade Plan](./SHADOW_CASCADE_PLAN.md)
- [Atmosphere Runtime Plan](./ATMOSPHERE_RUNTIME_PLAN.md)
- [Weather Foundation Plan](./WEATHER_FOUNDATION_PLAN.md)

These planning docs define the intended next implementation order:

1. stable cascaded world shadows
2. runtime atmosphere products
3. weather authoring/runtime separation

That ordering should guide the next implementation pass unless a narrow hard dependency requires otherwise.

## Current Implemented Changes

### 1. Persistent Weather Field

Implemented in `src/world/weather.rs`.

Current shape:

- Weather field is a persistent world-space CPU structure.
- Weather now lives in a 3-level world-space clipmap stack:
  - near: 128x128 over 32 km
  - mid: 128x128 over 96 km
  - far: 128x128 over 288 km
- Coverage, cloud type, humidity, convection, storm, and per-layer amounts are generated on a snapped world-aligned region.
- The field updates from player-centered world-space coordinates, not camera UVs.
- Clipmap origins are snapped in world space.
- CPU scroll updates shift retained texels and regenerate only newly exposed texels.
- GPU uploads now consume explicit dirty-rect manifests per clipmap level and update only changed array-texture regions.
- The field also now produces a shared cloud-shadow field.

Current textures produced:

- Weather primary RGBA:
  - R: coverage
  - G: cloud type
  - B: humidity
  - A: convection
- Weather secondary RGBA:
  - R: storm
  - G: low layer amount
  - B: mid layer amount
  - A: high layer amount
- Cloud shadow RGBA:
  - R: low-layer optical depth proxy
  - G: mid-layer optical depth proxy
  - B: high-layer optical depth proxy
  - A: precipitation proxy

Important note:

- This is still a generated field, not a real weather simulation. It is acceptable only as an intermediate step because it introduces persistence, regional coherence, and explicit data products that future simulation can replace.
- Full-level uploads still happen when controls change, the weather seed changes, or the time-throttled refresh path regenerates an entire clipmap level.

### 2. Cloud Composition Split

Implemented in `src/render.rs`, `src/shaders/cloud_composite.wgsl`, and `src/shaders/sky_shader.wgsl`.

Current frame ordering:

1. Shadow map pass
2. Terrain/world pass
3. Cloud raymarch pass
4. Cloud temporal resolve
5. Sky pass on untouched depth only
6. Cloud composite pass
7. UI pass

Why this matters:

- The old shape was effectively "sky owns cloud composition."
- The new shape moves toward "clouds are volumetric media that composite against the scene."

What this does not solve yet:

- Clouds still come from analytic layer functions, not resident 3D cloud storage.
- The cloud pass is still half-resolution with temporal reconstruction.

### 3. Depth-Aware Cloud Marching

Implemented in `src/shaders/cloud_shader.wgsl`.

Current behavior:

- The cloud shader reconstructs a conservative scene-depth distance from the depth buffer.
- March distance is clamped against terrain depth.
- This stops clouds from bleeding through foreground terrain silhouettes in the active path.

Limitations:

- The cloud pass is half-resolution, so the depth test is still conservative rather than exact.
- This is still a screen-space stop condition, not object-aware cloud volume composition against a shared volumetric depth structure.

### 4. Renderer Debug Views

Implemented in:

- `src/render/debug.rs`
- `src/egui_ui.rs`
- `src/main.rs`
- `src/shaders/cloud_shader.wgsl`
- `src/shaders/cloud_composite.wgsl`

Current debug modes:

- Final
- Weather Coverage
- Weather Structure
- Cloud Alpha
- Atmosphere Transmittance
- Atmosphere Sky View
- Atmosphere Multiscatter

Current routing:

- Weather/cloud debug still originates in the cloud path.
- Atmosphere LUT debug is routed through the dedicated cloud composite pass so the cloud composition split stays intact.

This is still not enough long term, but it is now the first real set of renderer debug surfaces that reaches the GPU path.

### 5. Shared Terrain Cloud Shadow Product

Implemented in:

- `src/world/weather.rs`
- `src/render.rs`
- `src/shaders/world_shader.wgsl`

Current active approach:

- CPU weather field generates a low-resolution shadow field.
- GPU uploads that field as a separate texture.
- Terrain shader samples projected layer heights against that field to estimate cloud transmittance.

Why this is important:

- Terrain no longer has to rely only on its own volumetric cloud approximation in the active path.
- This is the correct architectural direction: terrain should consume a shared cloud lighting product, not act as a second cloud system.

Why it is still transitional:

- The shadow field is derived from weather optical-depth proxies, not the same resident 3D cloud representation as the visible cloud pass.
- Terrain now consumes only the shared shadow field path. The remaining transition is data quality, not duplicate terrain-side cloud logic.

### 6. Atmosphere Runtime Owner

Implemented in `src/render/atmosphere.rs`.

What changed:

- Atmosphere-related shader assembly remains outside `render.rs`.
- `render::atmosphere` now owns:
  - transmittance LUT texture
  - sky-view LUT texture
  - multiscatter LUT texture
  - atmosphere bind-group layout / bind group
  - runtime mode and debug state
  - GPU LUT generation passes for sky-view and multiscatter, driven by lighting changes rather than camera changes

Why this matters:

- The future LUT-backed atmosphere work needs a named module owner.
- Keeping atmosphere ownership in `render.rs` would encourage more anonymous bind-group glue and make the eventual LUT transition harder.

What this module is not yet:

- It is not yet a complete LUT atmosphere subsystem.
- The current sky-view and multiscatter textures are generated by explicit runtime compute passes, but they are still approximate rather than fully physical LUT solves.
- Aerial perspective still uses analytic extinction/in-scatter math rather than runtime-owned froxels or slices.

## Architectural Rules

These are non-negotiable unless a later document explicitly replaces them.

### World / Weather

- Weather state must exist in persistent world space.
- Floating origin is allowed only as a transport detail. It must never redefine weather or cloud identity.
- Weather generation/simulation must be separable from cloud rendering.
- Weather products should be explicit textures/buffers, not implicit shader-local hash functions.

### Atmosphere

- Atmosphere must become its own subsystem with explicit ownership of:
  - transmittance LUT
  - multiscattering LUT
  - sky-view LUT
  - aerial perspective
- Clouds and terrain should sample atmosphere products, not fork atmosphere logic.
- CPU lighting state must stop being the long-term owner of sky color policy.

### Clouds

- Near and far clouds must come from one cloud field, not separate fake systems.
- A cloud debug mode is not a substitute for explicit data ownership.
- Temporal history is support code, not the source of quality.
- Shared shadow products should be derived from the same cloud data that visible rendering uses.

### Renderer

- The sky pass must never own cloud composition again.
- Render pass order should reflect dependency order, not historical convenience.
- Depth, cloud media, and sky should each have explicit pass ownership.

## Known Debt

These are real problems, not optional cleanup.

### 1. Runtime Sky Mode Exists, But LUT Generation Is Still Placeholder

The renderer now has an explicit runtime sky mode that samples owned sky-view and multiscatter textures, but those textures are still seeded from CPU approximations rather than generated by real LUT passes.

Impact:

- Lighting-dependent sky colour can route through runtime-owned resources, but the underlying data is still approximate.
- Aerial perspective and several lighting terms are still tied to shader-local analytic math rather than fully precomputed atmosphere products.

### 2. Cloud Storage Is Still Analytic Layer Logic

The visible cloud renderer still evaluates density procedurally from world position plus weather textures and noise textures. There is still no sparse 3D resident cloud field.

Impact:

- No stable cloud identity beyond weather-field persistence.
- No clipmap residency.
- No true world-space brick hierarchy.
- No empty-space acceleration structure beyond current analytic traversal tricks.

### 3. Shared Terrain Shadow Path Still Uses Proxy Data

The dead terrain-side cloud approximation path has been removed. Terrain now depends only on the shared cloud-shadow texture product.

Impact:

- Better architectural clarity
- Lower risk of terrain/cloud lighting drift
- The remaining limitation is that the shared shadow product is still derived from weather optical-depth proxies rather than resident 3D cloud storage

### 4. Weather Field Is Still Procedural Generation, Not Simulation

The weather system is persistent, but it is not a weather simulation.

Missing:

- front tracking
- pressure tendency
- layered wind advection
- vertical velocity / uplift field
- precipitation evolution
- storm lifetime / organization

### 5. Atmosphere Resource Ownership Exists, But Full LUT Shading Does Not

What now exists:

- transmittance
- sky-view
- multiscatter

What is still missing:

- aerial perspective slices / froxels
- migration of more cloud lighting and fog policy onto runtime-owned atmosphere products

## Next Major Checkpoints

These are ordered. Do not skip ahead casually.

### Checkpoint A: Atmosphere Runtime Subsystem

Goal:

- Replace analytic atmosphere-only ownership with explicit atmosphere runtime resources.

Deliverables:

- `renderer::atmosphere` owns texture resources and update policy.
- Add explicit textures for:
  - transmittance LUT
  - multiscatter LUT
  - sky-view LUT
- Replace direct analytic-only atmosphere calls where practical with LUT sampling hooks.
- Add atmosphere debug modes.

Implementation notes:

- It is acceptable to keep analytic fallback code while LUT generation is coming online.
- Do not hide the fallback. Name it explicitly.
- The first acceptable transition state is:
  - analytic sky fallback still exists
  - runtime owns atmosphere textures
  - shaders are wired to consume them behind an explicit mode gate

### Checkpoint B: Terrain / Cloud Lighting Cleanup

Goal:

- Remove legacy terrain-side cloud approximation from the active path.

Deliverables:

- Delete dead functions from `world_shader.wgsl`.
- Remove unused terrain-side cloud noise dependencies if no longer needed.
- Keep only shared weather + shared cloud shadow field consumption on the terrain side.

### Checkpoint C: Sparse Cloud Data Scaffolding

Goal:

- Create the ownership model for future resident cloud storage, even before final rendering is switched over.

Minimum deliverables:

- `clouds/` module
- clipmap/ring definitions
- brick coordinate types
- residency/page table structures
- placeholder upload/update API

Important:

- This checkpoint is about architecture and data ownership first.
- Do not fake completion by just creating empty files with no integration plan.

### Checkpoint D: Weather Simulation Upgrade

Goal:

- Replace procedural weather generation with an explicit simulation step.

Deliverables:

- persistent regional fields
- multi-altitude wind
- humidity field
- convection/uplift proxy
- storm organization state
- explicit update step decoupled from rendering

## Detailed Future Plan

## Phase 1: Complete Atmosphere Ownership

Tasks:

1. Create atmosphere runtime resource structs.
2. Allocate transmittance, multiscatter, and sky-view textures.
3. Add update invalidation rules:
   - camera change should not invalidate LUTs
   - sun/moon direction change may invalidate sky-view
   - atmosphere parameter changes invalidate dependent LUTs
4. Introduce explicit atmosphere parameter block rather than inheriting everything from `LightingState`.
5. Add debug output routes for LUT inspection.

Risks:

- WGPU texture formats and sample paths need to be chosen so both compute and fragment sampling are straightforward.
- Avoid premature optimization. Correct ownership matters more than the perfect first LUT resolution.

## Phase 2: Separate Weather from Authoring

Tasks:

1. Split "artist controls" from "simulated state."
2. Keep `CloudControls` only as authoring / biome / preset input.
3. Add runtime weather state that can evolve independent of current time-of-day presets.
4. Stop using day-based preset selection as the main weather driver.

Risks:

- If preset logic remains fused with simulation state, every future weather system will regress toward "animated preset noise."

## Phase 3: Build Cloud Storage

Tasks:

1. Define clipmap rings in world space.
2. Define brick dimensions and brick world extents.
3. Add residency selection around the camera.
4. Add occupancy / min-max / metadata storage per brick.
5. Add render sampling hooks that can coexist with the analytic fallback during transition.

Important design note:

- Do not attempt to jump directly from current analytic layers to final production sparse clouds in one patch.
- The correct migration path is dual-path:
  - analytic path remains functional
  - sparse path is introduced behind explicit ownership and debug tools
  - visible rendering then migrates incrementally

## Phase 4: Replace Shared Shadow Proxies with Shared Cloud Products

Tasks:

1. Generate cloud shadow data from actual resident cloud storage.
2. Stop deriving terrain cloud shadow purely from weather optical-depth proxies.
3. Optionally add multiple shadow products:
  - low-frequency cloud shadow map for terrain
  - volumetric light cache for cloud self-lighting

## What To Avoid

- Do not add more logic to `LightingState` unless it is truly lighting.
- Do not move more weather policy into WGSL convenience code.
- Do not add more "temporary" cloud logic inside terrain shaders.
- Do not let the sky shader reacquire ownership of cloud behavior.
- Do not rely on temporal accumulation to compensate for structural mistakes.
- Do not claim "world-space clouds" until persistent cloud storage actually exists.

## Debugging Priorities

The next debug surfaces worth implementing are:

1. Atmosphere debug:
   - transmittance LUT
   - sky-view LUT
   - multiscatter LUT
2. Weather debug:
   - humidity
   - convection
   - storm field
   - precipitation field
3. Cloud debug:
   - layer occupancy
   - shadow field
   - step count
   - depth clip distance
   - temporal confidence

## Validation Checklist

Use this checklist after each major checkpoint.

### Visual

- Terrain silhouettes never show clouds in front when depth should occlude them.
- Sky still renders only where scene depth is untouched.
- Weather debug modes are spatially stable while moving.
- Terrain cloud shadow direction remains coherent with sun direction.

### Structural

- New subsystem code lives outside `render.rs` where appropriate.
- Bind-group layout changes are mirrored exactly in WGSL.
- Shared products are consumed by multiple passes rather than recomputed locally.

### Future Hand-off Readiness

A fresh task/handoff is appropriate after the next checkpoint when:

- the atmosphere runtime module owns actual textures or passes, or
- cloud storage scaffolding exists with real types and ownership, or
- terrain/cloud lighting cleanup has removed the remaining dead legacy path.

At that point, a fresh task should include:

- the current contents of this file
- the current contents of `docs/ENGINE_REFACTOR_PLAN.md`
- a short statement of the exact next checkpoint
- a list of active runtime paths that must not regress

## Immediate Next Action

The next implementation target after this checkpoint should be:

1. move more of sky colour / aerial perspective sampling onto runtime atmosphere products
2. add runtime-owned aerial perspective slices or froxel integration
3. then proceed toward sparse cloud storage once the atmosphere path is visually validated

That remains the correct order before sparse cloud storage, because cloud lighting and aerial perspective still need a more complete atmosphere owner first.
