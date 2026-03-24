# Renderer Rewrite Execution Plan

This document is the implementation contract for the next renderer rewrite stage.

No algorithm work should begin until the planning set below is accepted as the target shape. The goal is to stop solving the next problem by adding more transitional logic into `render.rs`, `LightingState`, or terrain/cloud shaders.

## Planning Set

This plan is split into one coordinating document and three checkpoint-specific design documents:

- [Shadow Cascade Plan](./SHADOW_CASCADE_PLAN.md)
- [Atmosphere Runtime Plan](./ATMOSPHERE_RUNTIME_PLAN.md)
- [Weather Foundation Plan](./WEATHER_FOUNDATION_PLAN.md)

These documents are intentionally ordered. The next implementation pass should follow that order unless a hard dependency forces a narrow exception.

## Primary Outcome Order

1. Fix near-field world shadow quality with a stable 4-cascade directional shadow system.
2. Replace placeholder atmosphere runtime products with real runtime-owned generation passes and runtime-owned aerial perspective.
3. Separate weather authoring from runtime simulation and establish the fixed-step persistent weather foundation.

This ordering is deliberate:

- Shadow quality is the most visible current terrain regression and can be solved without re-architecting clouds.
- Atmosphere runtime ownership must mature before more terrain/cloud lighting policy is moved out of shader-local heuristics.
- Weather simulation should be split before adding more cloud behavior so persistent state is not built on top of preset-driven logic.

## Non-Negotiable Invariants

- The sky pass must never own cloud composition again.
- Weather state must remain persistent in world space.
- Floating origin is transport only, not identity.
- Do not move more temporary weather or cloud policy into terrain shaders.
- Do not add more sky policy to `LightingState` unless it is genuinely reusable lighting data.
- Atmosphere ownership must be explicit and runtime-owned.
- Existing debug views must remain functional.
- New debug views should be added at subsystem boundaries, not only at final composite points.
- Current pass ordering should remain:
  1. shadow generation
  2. world/terrain
  3. cloud raymarch
  4. cloud temporal resolve
  5. sky
  6. cloud composite
  7. UI

## Target Ownership Model

### `render.rs`

`render.rs` should remain frame orchestration and resource wiring, not the home for new rendering algorithms.

It may continue to own:

- device/bootstrap
- surface resize handling
- frame entry and submission
- high-level pass ordering

It should stop growing new subsystem-local math and state.

### `render::shadow`

This module should own:

- cascade split policy
- cascade fitting and texel snapping
- shadow texture array resources
- shadow uniforms and bind groups
- shadow draw-list building and per-cascade caster selection
- shadow debug metadata
- shadow pass execution

### `render::atmosphere`

This module should own:

- `AtmosphereParameters`
- atmosphere texture resources
- atmosphere update invalidation rules
- transmittance, multiscatter, sky-view, and aerial perspective generation passes
- atmosphere-derived lighting buffer(s)
- atmosphere debug surfaces
- explicit analytic fallback mode

### `world::weather`

This module should own:

- authoring controls and presets
- runtime weather state
- fixed-step simulation clock
- persistent world-space clipmap levels
- GPU product packing and dirty-rect manifests

Cloud rendering should consume weather products. It must not remain the hidden owner of weather behavior.

## Shared Contracts

### Lighting

`LightingState` should converge toward direct-light and exposure state:

- sun/moon directions
- sun/moon illuminance or intensity
- exposure
- direct-light color where still needed for compatibility
- fog scalar controls only until aerial perspective fully replaces them

It should not remain the long-term source of:

- final sky colors
- ambient sky gradients
- terrain fog target color
- cloud ambient coloration

Those belong to atmosphere runtime products or atmosphere-derived lighting data.

### Atmosphere

The renderer should expose an explicit atmosphere interface:

- CPU-facing parameters: `AtmosphereParameters`
- runtime-owned textures: `AtmosphereProducts`
- small derived-lighting buffer: `AtmosphereLightingProducts`

During transition, an adapter from existing lighting state is acceptable, but it must be clearly marked as compatibility code.

### Weather

The weather system should expose three distinct layers:

1. `CloudControls` or renamed equivalent: authoring/preset input only.
2. `WeatherRuntimeState`: persistent evolving fields and simulation clock.
3. `WeatherGpuProducts`: packed textures and upload manifests consumed by render passes.

The renderer should never have to infer which parts are authoring and which are simulation.

## Target Update Graph

The intended update graph is:

1. World/simulation update
   - fixed-step weather simulation
   - authoring-to-runtime forcing updates
2. Renderer pre-frame update
   - atmosphere invalidation evaluation
   - atmosphere product generation
   - shadow cascade fitting/update
3. Frame rendering
   - shadow cascades
   - world/terrain
   - cloud raymarch
   - cloud temporal resolve
   - sky
   - cloud composite
   - UI

Key rule: atmosphere products must be ready before terrain/cloud lighting, but sky must still not own cloud composition.

## Milestone Gates

### Gate A: Stable Cascaded Shadows

Must be true before closing the checkpoint:

- a dedicated `render::shadow` module exists
- a depth texture array backs four cascades
- near-field sun shadows are visibly sharper than the current single-map path
- cascade transitions are difficult to notice
- debug views exist for cascade index and texel density/utilization

### Gate B: Runtime Atmosphere Products

Must be true before closing the checkpoint:

- sky-view and multiscatter are generated by runtime passes, not CPU-seeded placeholders
- `AtmosphereParameters` exists as an explicit block separate from `LightingState`
- aerial perspective is runtime-owned
- terrain fog target color and at least the first cloud lighting inputs come from atmosphere products
- debug views expose transmittance, sky-view, multiscatter, and aerial perspective

### Gate C: Weather Runtime Foundation

Must be true before closing the checkpoint:

- authoring controls are separated from runtime state
- weather advances through a fixed-step path
- runtime state contains layered wind, humidity, uplift/convection proxy, storm organization, and precipitation tendency
- clipmap upload manifests and dirty-rect uploads still work
- moving the camera does not invent new weather identity

## Anti-Patterns To Reject

- Re-adding hidden cloud composition into sky code because it is convenient for debug or final compositing.
- Solving terrain lighting problems by embedding more cloud policy in `world_shader.wgsl`.
- Expanding `LightingState` into a grab bag for atmosphere colors.
- Replacing stable world-space weather with camera-relative convenience noise.
- Jumping to virtual shadow maps before a real clipmapped/page-cached architecture exists.
- Using temporal history to hide unstable shadow or atmosphere ownership.

## Follow-On Work After A/B/C

These are intentionally deferred until the current plan lands:

- resident cloud storage / cloud brick clipmaps
- unified cloud-shadow products from resident clouds instead of weather optical-depth proxies
- unified volumetrics for fog, precipitation haze, and shafts
- optional short-range sun contact shadows
- directional clipmapped or virtual shadow architectures

## References

- NVIDIA GPU Gems 3, Chapter 10: Parallel-Split Shadow Maps
- Sebastien Hillaire, EGSR 2020: A Scalable and Production Ready Sky and Atmosphere Rendering Technique
- Frostbite: Physically Based Sky, Atmosphere and Cloud Rendering
- Frostbite: Physically Based and Unified Volumetric Rendering
- Unreal Engine documentation: Virtual Shadow Maps
