# Weather Foundation Plan

## Objective

Separate weather authoring controls from runtime simulation state, add a fixed-step runtime update path, and establish persistent evolving weather fields without breaking the current clipmap upload and dirty-rect architecture.

This checkpoint is about ownership and evolution rules first. It is not the final realistic weather simulation.

## Current Problems

The current weather path already has persistent world-space clipmaps, but the runtime model is still too tightly fused to authoring controls:

1. `CloudControls` acts as both authoring intent and runtime state driver.
2. Weather evolution is effectively regenerated procedural logic, not a simulation step.
3. Wind, humidity, convection, storm organization, and precipitation tendency do not exist as explicit persistent runtime fields.
4. Authoring changes can still behave like hidden full weather replacement instead of forcing terms.

## Required Outcomes

- `CloudControls` becomes authoring input only.
- Weather advances through a fixed-step runtime path.
- Runtime state contains:
  - layered wind
  - humidity
  - convection/uplift proxy
  - storm organization
  - precipitation tendency
- The weather field remains persistent and world-space stable while moving.
- The existing clipmap upload manifest and dirty-rect architecture remains intact.

## Ownership Split

The weather module should expose three clear layers.

### 1. Authoring Layer

Keep `CloudControls` or rename it to something equivalent, but make its role explicit:

- preset input
- biome/climate bias
- time-of-day or scenario forcing
- artist tuning

It must not be treated as the persistent weather state itself.

### 2. Runtime Simulation Layer

Add a runtime owner, for example:

```rust
pub struct WeatherRuntimeState { ... }
```

This owns:

- simulation clock
- fixed-step accumulator
- persistent clipmap level state
- evolving scalar/vector fields

### 3. GPU Product Layer

Keep the explicit weather GPU products:

- primary RGBA
- secondary RGBA
- cloud-shadow RGBA
- upload manifest / dirty rects

The renderer consumes this layer only. It should not need to know how the simulation evolves internally.

## Suggested Data Model

Keep the current clipmap stack and extend each level with persistent runtime fields.

Recommended per-texel runtime fields:

- `humidity`
- `uplift`
- `storm_organization`
- `precipitation_tendency`
- `wind_low : vec2`
- `wind_mid : vec2`
- `wind_high : vec2`

Optional helper/source fields that are acceptable in this checkpoint:

- low-frequency convergence or frontal forcing seed
- temperature or instability proxy
- dryness or subsidence proxy

Those helper fields are allowed only if they support the explicit runtime fields above. They should not become another hidden preset generator.

## Evolution Rules

The first runtime model should be simple, deterministic, and persistent.

Recommended direction:

### Humidity

Humidity should:

- advect with a chosen layer wind, usually mid-level
- replenish slowly from authoring climate moisture supply
- decrease under persistent precipitation
- increase under convergence or uplift support

### Uplift / Convection Proxy

Uplift should:

- advect slowly
- increase from instability and convergence forcing
- decay when forcing is absent
- respond to authoring convective tendency without being replaced by it

### Storm Organization

Storm organization should:

- grow where humidity and uplift coexist
- decay gradually when support weakens
- persist long enough to create coherent storm lifetime
- optionally benefit from moderate shear rather than directly mirroring `CloudControls`

### Precipitation Tendency

Precipitation tendency should:

- increase from organized moist uplift
- decay more slowly than raw uplift
- feed back negatively into humidity

### Layered Wind

Wind should:

- exist as runtime vector fields
- vary slowly in world space
- carry level-dependent shear
- evolve from authoring baseline plus persistent low-frequency forcing

Do not keep wind as only one angle/speed pair pushed directly into every weather computation.

## Fixed-Step Update Model

Introduce an explicit fixed-step simulation clock.

Recommended initial policy:

- `weather_dt = 0.5 s`
- clamp catch-up to a small number of steps per frame
- accumulate partial frame time rather than using variable-step updates

Why:

- weather is low frequency
- fixed-step ownership is more important than very high temporal resolution
- it avoids frame-rate-dependent weather evolution

Suggested API direction:

```rust
fn advance(
    &mut self,
    dt_real_s: f32,
    center_world_xz_m: Vec2,
    authoring: &CloudControls,
    weather_seed: u32,
) -> Option<CloudWeatherUploadManifest>
```

The implementation may internally perform zero or more fixed steps and return a manifest only when tiles or levels changed.

## Clipmap and Dirty-Rect Preservation

The current clipmap upload architecture is worth keeping. The new design should preserve:

- world-space snapped clipmap origins
- scroll-based partial retention
- dirty-rect manifests
- per-level partial uploads

To make fixed-step simulation compatible with this, move from "entire level changes every time" toward tiled simulation updates.

### Recommended Tile Scheduler

Split each level into fixed simulation tiles, for example `16 x 16` texel tiles.

At each fixed step:

- near level updates more tiles
- mid level updates fewer tiles
- far level updates the fewest

Scroll-exposed tiles are always updated immediately.

This keeps:

- simulation incremental
- uploads partial
- far levels affordable

It also preserves the current dirty-rect architecture rather than fighting it.

## Product Packing Strategy

Keep the current GPU texture products for this checkpoint.

Map runtime state into them explicitly:

### Primary RGBA

- R: coverage
- G: cloud type
- B: humidity
- A: uplift / convection proxy

### Secondary RGBA

- R: storm organization
- G: low layer amount
- B: mid layer amount
- A: high layer amount

### Shadow RGBA

- R: low-layer optical depth proxy
- G: mid-layer optical depth proxy
- B: high-layer optical depth proxy
- A: precipitation tendency

The important decision is that these become products derived from runtime state, not the state itself.

## Authoring-to-Runtime Relationship

Authoring should provide forcing, not direct identity replacement.

Recommended rules:

- coverage, density, and cloud-type style controls bias the runtime target tendencies
- wind controls bias the baseline layered wind profile
- explicit reseed or world reset can still rebuild the field
- ordinary authoring changes should not instantly replace persistent weather identity

This prevents the simulation from regressing into "animated preset noise."

## Runtime Stability Rules

The following rules must hold:

- moving the camera only moves the clipmap window
- weather identity is anchored in world space
- simulation time is independent from render frame rate
- sky time and weather time may influence each other, but are not the same clock

## Debug Plan

Preserve existing weather debug modes and plan to add deeper inspection once the new fields exist.

Recommended new debug priorities after the split lands:

- humidity
- uplift
- storm organization
- precipitation tendency
- layered wind direction/magnitude

These should remain renderer-wide debug surfaces, not temporary CPU printouts.

## Implementation Stages

### Stage 1: Structural Split

- separate authoring controls from runtime state types
- keep old visual behavior as much as possible

### Stage 2: Fixed-Step Clock

- add accumulator and fixed-step update loop
- remove frame-rate-dependent evolution

### Stage 3: Persistent Runtime Fields

- add humidity, uplift, organization, precipitation, layered wind fields
- derive existing packed products from them

### Stage 4: Tiled Simulation and Uploads

- add tile scheduler
- mark dirty rects from tile updates and clipmap scroll
- preserve partial uploads

### Stage 5: Behavior Tuning

- tune decay/growth constants
- validate persistence and spatial coherence

## Validation Checklist

Visual validation must confirm:

- weather remains spatially stable while moving through the world
- storm regions feel persistent rather than re-invented around the camera
- precipitation tendency varies coherently over time instead of flickering
- cloud-shadow behavior remains directionally coherent with the sun

Structural validation must confirm:

- authoring and runtime state are different types with different responsibilities
- fixed-step simulation exists
- clipmap dirty-rect uploads still function
- renderer code still consumes explicit products rather than simulation internals

## Deferred Work

This checkpoint does not attempt:

- final realistic pressure/front simulation
- terrain-coupled meteorology
- resident cloud storage
- final precipitation rendering effects

It is the ownership and simulation-foundation checkpoint.
