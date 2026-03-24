# Engine Refactor Plan

This document describes the target architecture for evolving the current voxel prototype into a cleaner,
higher-quality engine without preserving the current monolithic renderer shape.

## Current structural problems

- `render.rs` owns GPU device bootstrap, shader composition, atmosphere, cloud generation, terrain generation,
  chunk streaming, shadow rendering, frame orchestration, dynamic vertex buffers, and utility math.
- World lighting, cloud controls, and weather generation are tightly coupled to rendering uniform packing.
- Atmospheric rendering is still based on hand-authored shader logic instead of a dedicated atmosphere subsystem.
- Weather field generation is static and was previously dominated by sinusoidal macro structure, producing limited variety.
- There is no explicit renderer frame graph or pass ownership model.

## Target module layout

```text
src/
  app/
    mod.rs
    app.rs
    input.rs
    state.rs
  gameplay/
    mod.rs
    player.rs
    block_interaction.rs
  world/
    mod.rs
    config.rs
    generation.rs
    snapshot.rs
    lighting.rs
    weather.rs
    collision.rs
  renderer/
    mod.rs
    gpu.rs
    frame.rs
    camera.rs
    shadow.rs
    terrain.rs
    chunk_stream.rs
    atmosphere.rs
    clouds.rs
    weather_fx.rs
    debug.rs
    shaders/
  simulation/
    mod.rs
    fluid.rs
```

## Recommended migration order

1. Split the current `world.rs` into `world/config.rs`, `world/weather.rs`, `world/lighting.rs`,
   `world/generation.rs`, and `world/collision.rs`.
2. Split `render.rs` into pass ownership modules before changing rendering algorithms.
3. Replace the hand-authored sky model with a LUT atmosphere subsystem.
4. Replace flat cloud slab projection with shell-aware cloud-space mapping plus temporal history.
5. Introduce a unified volumetric media pass for fog, shafts, precipitation mist, and cold haze.
6. Move precipitation, leaves, dust, and drifting cold fog to a shared wind-driven weather FX system.
7. Add cloud shadow projection and physically motivated wetness / snow surface response.
8. Introduce explicit debug views for atmosphere LUTs, cloud coverage/regime maps, wind slices, and froxel media.

## Rendering principles

- Every major subsystem must expose debug buffers.
- No shader source should remain embedded inside Rust string literals.
- Pass resources should have clear ownership and lifecycle.
- Weather/randomization must change macro morphology first, not just low-level noise weights.
- High-frequency cloud erosion must be distance-aware and horizon-suppressed.
- Time-of-day coloration must come from atmospheric transmittance, not only from day/night scalars.

## Phase 1 changes in this patch set

- Shader sources have been externalized into `src/shaders/*.wgsl`.
- CPU weather texture generation now uses layered value noise instead of sine/cosine-dominated macro fields.
- Lighting now passes a normalized weather seed into the cloud shader so the weather lookup can rotate and offset
  macro cloud regimes between randomized days.
- The patch is intentionally conservative: it improves maintainability and cloud variety without attempting a risky
  full renderer rewrite in one step.

## Next implementation milestone

The next substantial change should be a new `renderer::atmosphere` module containing:
- transmittance LUT generation
- multiple-scattering LUT generation
- sky-view LUT generation
- aerial perspective froxel integration

Only after that should the cloud system be upgraded to use atmosphere-derived sun transmittance and cloud lighting.
