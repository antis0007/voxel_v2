# Atmosphere + Cloud Rewrite Status

## Palette revision after user review

The user rejected the V9 color response as too boring and consistent compared with the supplied photographs. V9's palette is not accepted visual quality. The new candidate keeps the physical extinction/transport tables but applies photograph-guided radiance art direction: deep daytime blue, cyan-to-indigo upper twilight, a localized golden solar horizon and rose antisolar belt, altitude-dependent peach/rose cloud sunlight, and cooler diffuse cloud fill. Direct sunset light no longer inherits an inappropriate top-down height-darkening mask. Terrain and cloud aerial perspective use the same radiance response. This is explicitly an artistic illumination response, not a claim that the modified radiance is a purely physical simulation.

`--benchmark-review-time=<0..1>` can override the review clock without changing production time/weather controls. The palette review script records the actual time argument in each case's metadata, since the existing capture filenames retain the broader noon/sunset phase names.

### Palette live review and second refinement

Launched the actual renderer for six cases in `target/photo-review/palette-v1`: daytime, golden hour (0.7425), sunset (0.7475), rose afterglow (0.7525), storm noon and storm afterglow. The 24 captures showed improved separation but overly violet upper air and pale cloud sunlight. The second refinement uses quieter slate-blue upper twilight, richer gold/coral incident sunlight and a 1.65x twilight gain only on directly exposed faces and thin rims. It does not increase every warm scattering order, which would wash the cool interiors out again.

The final palette source was built with `cargo build --offline --target-dir target`, including the existing composed WGSL validation, then reviewed in five more cases (20 captures): golden hour, sunset, overcast golden hour, storm sunset and storm afterglow. Representative captures at 1600x900 are in `target/photo-review/palette-v2`, with per-case arguments, capture timestamps, logs and executable hashes. Frozen reviewed development executable: `target/photo-review/sky-review-palette-v2.exe`, SHA256 `9D2BC9A7EDD9926BBC8C1CA2A5C8D76B7BD79353B50C06ACAA140DE6D030B0B3`. This checkpoint also includes the coordinated rendering owner's corrected camera-relative cloud fade range and cubic reconstruction.

Visual result: brighter peach/gold lit edges against cooler interiors, a localized amber solar horizon, rose-colored afterglow and darker slate storm ceilings. This is an inspected improvement over the rejected V9 palette, not user acceptance or photographic/AAA parity. Fully overcast golden skies still have a uniform blanket; fine distant cloud bands, repeated morphology and incomplete terrain horizons remain. GPU timing is not an isolated comparison because another game instance was already running. Review cameras left the mouse released and did not autosave. No test suite or hosted CI was run. The pre-existing release session was preserved; it does not acquire newly compiled shaders without restart.

### Final conservative overcast refinement

The overcast density field saturated across most of the sheet. A trial changing sheet geometry (palette-v3) revealed angular cutouts in the noon capture and was rejected. The final palette-v4 restores the original sheet geometry and condensation floor, varying only stratiform water content from the existing bank and shape fields. Convective geometry is unchanged. Both noon and golden-hour overcast captures were inspected at full size before freezing this checkpoint: the blanket remains largely closed, with modest cool pockets and warmer translucent folds. This does not eliminate every distant seam or band.

Final focused-review executable: `target/photo-review/sky-review-palette-v4.exe`, SHA256 `CDCEA6DF488AE96FD78551E0528F57B3F2C6A8EB07AFCBA1CE46B9F789970590`. Eight captures and per-case metadata are under `target/photo-review/palette-v4`. This is an optimized development build; the coordinated rendering chat owns the separate final release/all-preset review. Palette-v2 lighting is unchanged. Rejected palette-v3 imagery is diagnostic evidence, not the delivered morphology.

The coordinated final release review completed all 40 captures (five presets, noon/sunset, two views and adjacent frames) at 1600x900 in `target/cloud-quality-review/quality-v4`. Release executable SHA256: `64EA3742EDA20E196D0ADF4A0A9155D8FE4C5CF5D9B0AC43FBD67EB6A2D28E95`. The final contact sheets and WinterSky follow-up were also inspected here. Shader snapshot hashes match the final palette source. Fine horizon banding and regular cloud distribution are still visible; these are not claimed fixed.

The Day/Night settings now expose Golden hour (0.7425), Sunset (0.7475) and Afterglow (0.7525); the previous Dusk button jumped to 0.79 and missed these lighting conditions. This UI-only change compiled successfully after the release visual review. `target/photo-review/sky-review-palette-final.exe` includes it; `palette-final.json` records its hash and confirms the reviewed shader hashes are unchanged. The new buttons have compile/source verification, not an interactive click-through claim.

## Organized weather and atmospheric transport — 2026-10-18

The next implementation builds on the photograph-guided V5 below:

- `cloud_morphology.wgsl` groups local updrafts into continuous kilometer-scale banks with clear corridors. A nonperiodic, smooth hashed field controls coverage, storm floor relief, and wind-sheared anvils. Fine cloud erosion remains independently filtered.
- `weather_volume.wgsl` integrates precipitation below storm banks: 40 bounded, quadratically spaced ray segments; wind-deflected source coordinates; elongated, mip-filtered filaments; humidity-dependent evaporation; Beer extinction and sky/solar scattering. Scene depth clips the medium against terrain. Dry presets skip the pass. This is distant rain volume, not nearby droplet particles or surface wetness.
- Clouds and precipitation compose as four radiance/transmittance volumes ordered by opacity-weighted distance. This remains an approximation when volumes overlap in depth; it is not per-sample interleaved transport.
- Atmospheric multiple scattering now uses 32 equal-solid-angle directions and 20 integration segments per direction. It accumulates incident sunlight, ozone absorption, ground bounce, and the response to an isotropic field; higher orders use `L / (1 - F)`. This is a finite-quadrature isotropic transport approximation, not a full spectral or fluid simulation.
- The transport table spans solar elevation and altitude, with nonlinear coordinates concentrating resolution near the horizon and ground. It is computed once after solar transmittance. Sky-view and aerial-perspective integrators sample it locally; the old final-color addition indexed by view/sun angle was removed. Separate read/write layouts avoid a storage/sampled-texture alias.
- Cloud diffuse illumination uses a shorter hemispheric occlusion path than direct sunlight, retaining cool fill in sunset shadows without disabling self-shadowing.

The visual target remains the supplied photographs and the quality of Horizon/RDR2. This implementation does not claim parity with their production weather systems. Missing pieces include fluid-authored weather volumes, interleaved cloud/rain/fog transport, robust temporal reconstruction, nearby precipitation particles, surface wetness, and terrain extending to the atmospheric horizon.

### Live review evidence for this increment

Built locally with `cargo build --offline --target-dir target`, including the existing Naga shader compilation. V6/V7 live comparisons guided the bank/rain/transport work. V8 then captured all five presets at noon and sunset over Hills, 16 km from the origin, at 1600x900: 40 fresh frames in `target/photo-review/v8-full`. Representative captures from every preset/time were inspected. Rain curtains now occupy the gap beneath the storm deck; daytime updrafts retain bright edges and cooler undersides. The new transport produces a stronger golden sunset horizon. Overcast remains comparatively smooth, and sunset cloud shadows still need more photographic color nuance. Fine horizontal sampling bands remain visible near some distant cloud horizons.

The diffuse-light revision made storm cores too bright. V9 restores a longer ambient occlusion path as storm intensity increases; eight additional Stormy noon/sunset captures were inspected in `target/photo-review/v9-hills`. Final copied executable: `target/photo-review/sky-review-v9.exe`, SHA256 `41BD3D76BCD285E866FDAF416D41404468B63D0CEFCCFAD7C244FCEC9845A218`. `verification.json` records frame dimensions and source hashes. V8 all-preset average: 112.77 FPS, cloud pass 7.053 ms. V9 storm samples: 105.29/104.92 FPS, cloud pass 7.694/7.826 ms on RTX 4090/Vulkan. These are development-build local samples, not performance guarantees for other machines. The final V9 refinement has storm-specific follow-up captures, not a second all-preset run.

All 48 final-review images are 1600x900. Review cameras left the mouse released and skipped autosave. No renderer errors were logged; existing Vulkan validation-layer and egui framebuffer warnings remain. No test suite or hosted CI was run. The other rendering chat's separately frozen V5 release review remains intact under `target/graphics-review`.

## Photograph-guided rendering update — 2026-10-18

The current shaders now use the seven supplied sky photographs as visual targets:

- June daytime cumulus: fuller updraft cores, nested cellular silhouettes, flatter bases, and fine edge erosion.
- August evening layers: shape erosion before density scaling preserves small mid-level patches; illumination retains the sky's cool color and samples sunlight at both layer heights.
- July dawn and October storm skies: integrated light-path extinction, two approximate higher scattering orders, spherical atmosphere transmittance, and ozone absorption replace flat density shadows and the clamped solar air-mass approximation.
- Each cloud layer applies aerial perspective at its opacity-weighted visible distance. The atmosphere volume reaches at least 16 km, independently of terrain visibility, and starts at zero optical depth.
- Removed the extra white camera-inside-cloud overlay and duplicate direct-light extinction. Fixed the maximum storm-type interpolation endpoint and cirrus fibers stretching across rather than along the wind.

Existing weather controls and presets drive these changes; no generated backdrop or photographic texture is used. Sun transmittance now uses nonlinear horizon/altitude coordinates covering 60 km, shared by generation and all runtime consumers.

### Live photograph comparison and refinement

Launched the actual Vulkan renderer and compared daytime, sunset and storm captures against the supplied photographs. The initial captures were blurry, nearly flat cloud patches; the first shape revision was too regularly rounded. The final `cloud_morphology.wgsl` combines unequal overlapping turrets, rotated footprints, deeper updrafts, fine erosion and broad condensation-floor relief. Compact support keeps the neighborhood continuous. Pixel footprint filtering is separate from ray-step length so shallow viewing angles retain detail.

Sunset illumination now uses a nearly white incident solar source, allowing atmospheric extinction to provide its color once. Diffuse illumination retains the cool upper sky, and interior ambient occlusion has a bounded path instead of scaling indefinitely with storm depth. Fixed-count shadow integration and removal of the depth-8 early exit avoid discontinuities in higher scattering orders. The distant macro fallback skips local shadow work only after its contribution reaches zero.

A visible storm underside plane persisted with self-shadowing disabled and with raw low-layer output. A constant-footprint diagnostic removed it. Moving condensation-floor relief to broad noise substantially improved that sampled region without disabling distance filtering. This is bounded visual evidence, not a guarantee against every artifact.

Validation: `cargo build --offline --target-dir target` succeeded, including the existing composed-WGSL parser/validator. Final V5 review captured 16 frames over Hills at 16 km, 1280x720, with the mouse released: daytime PartlyCloudy, sunset PartlyCloudy, sunset Stormy, and daytime Stormy. Images and logs are in `target/photo-review/v5-hills`; executable is `target/photo-review/sky-review-v5.exe` (SHA256 `F80230A80393BA6CD723BE3BEFF388791879C983D7EDE606C9B20D468C4D65EA`). Cloud GPU pass averages were 5.109, 5.687, 4.994 and 5.025 ms respectively on RTX 4090/Vulkan. These are short development-build samples with concurrent desktop activity, not general hardware performance guarantees. No test suite or hosted CI was run.

Photographic parity remains a target. The reference photographs have larger organized weather structures, precipitation shafts and a wider range of twilight illumination than this implementation reproduces. Layer ordering still approximates intersecting volumes using one representative depth per layer; temporal reconstruction remains disabled in the reviewed build. The coordinated rendering chat owns the separate final release/all-preset Hills review.

The older implementation notes below describe earlier checkpoints and may be superseded by this update.

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
