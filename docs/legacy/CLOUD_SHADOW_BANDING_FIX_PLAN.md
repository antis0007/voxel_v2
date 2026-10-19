# Cloud Shadow Banding Fix Plan

This document is the authoritative implementation plan for the specific cloud artifact where dense or layered clouds turn too dark and develop moving light/dark striped patches, especially at farther distances.

Scope:

- Fix the critical shading and shadowing flaw that produces dark banding and moving striped patches.
- Keep the change targeted.
- Do not use this task to do a full cloud look rewrite, full scale retune, or renderer architecture rewrite.

## Problem Summary

Current visible failure:

- Dense clouds darken too aggressively.
- When cloud layers stack or density gets high, the image develops alternating bright/dark striping.
- The striping gets worse with distance.
- The striping moves, which makes the defect much more obvious under temporal accumulation.

This is not just a style issue. It is a stability and correctness problem in the cloud lighting path.

## Current Code Facts

Relevant files:

- `src/shaders/cloud_shader.wgsl`
- `src/shaders/cloud_shared.wgsl`
- `src/shaders/cloud_temporal.wgsl`
- `src/shaders/cloud_composite.wgsl`
- `src/world/weather.rs`
- `src/render.rs`
- `src/render/debug.rs`

Important existing facts from the current code:

1. The main cloud march and lighting live in `march_layer_core()` inside `src/shaders/cloud_shader.wgsl`.
2. The current shadow logic lives around the shadow block inside that function, plus `sun_shadow_density()`.
3. The current shadow taps reuse the current sample's `weather`, `cell_base_y`, and `cell_top_y` even after moving the shadow sample point along the light direction.
4. The far shadow path switches to a very cheap single displaced density lookup based on `dist_t > 0.50`.
5. The shadow step length is derived from the primary raymarch `dt`, so far clouds get very large shadow strides.
6. The runtime already uploads a world-space cloud shadow clipmap texture:
   - generated in `src/world/weather.rs`
   - bound at `binding 7` in the cloud render bind group in `src/render.rs`
7. The cloud shader currently does not consume that existing `cloud_shadow_tex`.

## Root Cause

The artifact is primarily caused by the shadowing model, not by the basic cloud-density generation.

### 1. Shadow taps are evaluated with the wrong context

The current code displaces the sample point `q` along the light ray but still evaluates density using the original sample's weather and local layer bounds.

That means shadow samples do not re-evaluate:

- weather at the displaced world position
- per-layer vertical bounds at the displaced world position
- whether the displaced point is even inside valid cloud volume for that layer

This creates incorrect shadowing and causes dense regions to self-darken too strongly.

### 2. Far shadowing is tied to the main raymarch step size

The far path uses a hard quality switch and a shadow sample offset derived from `dt`.

As distance increases:

- `dt` increases
- shadow taps jump farther
- shadowing becomes structured and under-sampled

That produces regular banding instead of smooth volumetric shading.

### 3. Unstable shadow estimates are fed into strong extinction and ambient suppression

Once the shadow term is wrong, the current lighting stack amplifies the defect through:

- `light_trans`
- `depth_ao`
- `beer_powder`

This is why the result is not just striped, but also too dark.

### 4. Temporal accumulation makes the defect more obvious but is not the primary cause

The temporal pass can preserve or amplify structured instability, but the main issue already exists in the raw cloud pass.

The first implementation goal must be to fix `cloud_current`, not to tune temporal accumulation first.

## Target End State

The fix should produce this behavior:

- Near clouds still have local self-shadowed depth.
- Far clouds use stable low-frequency shadowing instead of striped local shadow aliasing.
- Dense stacked clouds retain form without collapsing into charcoal-dark patchwork.
- Shadow quality transitions are continuous and keyed to sample footprint or resolvability, not an arbitrary hard distance threshold.
- The lighting code is structurally clear:
  - macro shadow sampling
  - local self-shadow sampling
  - shadow combination
  - lighting remap

## Implementation Strategy

Replace the current shadow path with a hybrid model:

1. A stable macro shadow term sampled from the existing world-space `cloud_shadow_tex`.
2. A small local self-shadow term that re-samples weather and AABB state at each displaced light-ray tap.
3. A continuous blend from local-plus-macro near the camera to mostly macro at coarse or far footprints.

This keeps local sculpting where it is resolvable and removes the alias-prone far shadow behavior that is causing the visible striping.

## Detailed Implementation Plan

### Phase 1. Add explicit macro shadow sampling helpers

Files:

- `src/shaders/cloud_shader.wgsl`

Steps:

1. Declare the existing binding in the cloud shader:
   - `@group(3) @binding(7) var cloud_shadow_tex : texture_2d_array<f32>;`
2. Add helper functions that mirror the weather clipmap sampling path:
   - clip-region lookup
   - clip UV generation
   - per-level weights
   - manual bilinear sampling for the non-filterable array texture
3. Add a helper like `sample_macro_shadow_od(world_xz, layer_id) -> f32`.
4. Read layer-specific optical-depth channels from the uploaded shadow field:
   - low layer: `R`
   - mid layer: `G`
   - high layer: `B`
5. Keep layer-specific scale coefficients. Do not assume one transmittance mapping is correct for all cloud genera.

Important note:

- `src/render.rs` already exposes binding 7 in the cloud render bind group layout and bind group creation path, so this should not require new renderer resource plumbing unless the implementation discovers a real mismatch.

### Phase 2. Introduce proper per-tap shadow context sampling

Files:

- `src/shaders/cloud_shader.wgsl`

Steps:

1. Add a helper that evaluates weather and layer bounds at an arbitrary shadow sample position.
2. That helper must:
   - apply the correct layer-specific wind advection multiplier
   - call `weather_at_world()` at the displaced world position
   - compute `cell_aabb_for_layer()` for that displaced sample
3. Add a density wrapper for shadow taps that uses the displaced context instead of the original sample's context.
4. Early-out the shadow tap when the displaced sample is outside the displaced layer AABB.

Non-negotiable rule:

- Do not reuse the current sample's `weather`, `cell_base_y`, or `cell_top_y` for displaced shadow taps.

That is the main correctness flaw that needs to die.

### Phase 3. Replace the current shadow algorithm with a hybrid macro + local model

Files:

- `src/shaders/cloud_shader.wgsl`

Steps:

1. Remove the hard `use_cheap = dist_t > 0.50` split.
2. Replace `sun_shadow_density()` and the current cheap branch with:
   - a short local self-shadow function
   - a macro shadow function
   - a blend function between them
3. The local self-shadow function should:
   - use a small fixed tap count, such as 2 to 4 taps
   - march in world meters, not in multiples of the primary-view `dt`
   - scale tap spacing from `sample_footprint_m` and layer thickness
   - clamp spacing to sane layer-specific ranges
4. Each local tap should:
   - compute its own world position
   - re-sample weather and AABB there
   - skip if outside valid cloud bounds
   - sample density with shadow-friendly LOD
5. The macro shadow term should be:
   - low frequency
   - stable
   - always available
   - dominant for coarse or far footprints
6. Use a continuous blend keyed to footprint or resolvability, for example:
   - near/fine footprints: local self-shadow + moderate macro base
   - far/coarse footprints: mostly macro, little or no local self-shadow

Design intent:

- Macro shadow answers whether the region is broadly under heavy cloud mass.
- Local shadow answers how nearby cloud structure sculpts the light.
- The far path should not depend on a single displaced density lookup.

### Phase 4. Retune the dense-volume lighting response after the new shadow model is in place

Files:

- `src/shaders/cloud_shader.wgsl`

Steps:

1. After replacing the shadow path, retune only the coefficients that currently over-amplify bad shadows.
2. The likely tuning points are:
   - `ext_light`
   - `depth_ao` attenuation strength
   - `beer_powder` influence
   - the mapping from macro/local shadow optical depth to transmittance
3. Keep this retune narrow and justified.
4. Prefer a softer remap from shadow optical depth to light attenuation over globally brightening the clouds.

Goal:

- Remove the pathological over-darkening in dense stacks without washing out close-range form.

### Phase 5. Make the shadow path stable under temporal accumulation

Files:

- `src/shaders/cloud_shader.wgsl`
- `src/shaders/cloud_temporal.wgsl` only if needed after the raw pass is fixed

Steps:

1. Keep the main density jitter separate from the shadow stability problem.
2. Do not rotate shadow taps every frame with the existing per-frame blue-noise phase.
3. If shadow dithering is still needed:
   - use deterministic per-pixel or per-cell variation
   - do not use frame-varying motion that causes visible lighting crawl
4. Inspect the raw cloud render before making temporal-pass changes.
5. Only change `cloud_temporal.wgsl` if the raw cloud pass is stable and the temporal pass still reintroduces visible artifacts.

### Phase 6. Add focused debug instrumentation

Files:

- `src/shaders/cloud_shader.wgsl`
- `src/render/debug.rs`
- `src/shaders/atmosphere_runtime_shared.wgsl`
- `src/shaders/cloud_composite.wgsl` if needed

Recommended debug outputs:

- macro shadow optical depth
- local self-shadow optical depth
- combined shadow attenuation
- optional far/near shadow blend weight

Why this matters:

- Without separate macro/local debug outputs, it will be too easy to fix the striping by accidentally flattening all lighting.

If new debug modes are added:

1. Keep Rust enum values and shader constants synchronized.
2. Make sure the debug output can be viewed before and after temporal resolve if possible.

### Phase 7. Validate the result before calling it done

Validation checklist:

1. Inspect the raw cloud pass, not just the final composited result.
2. Dense low clouds under layered coverage should no longer show moving striped patches.
3. Far clouds should keep broad shading gradients instead of zebra-like banding.
4. Near clouds should still show local shadowed depth and not become flat white blobs.
5. Mid and high layers should remain stable when overlapping the low layer.
6. Performance should stay within the current half-resolution cloud budget class.
7. `CloudStepCount` should not regress badly just because the artifact was brute-forced with more primary march steps.

## Suggested Concrete Code Changes

### Required

1. `src/shaders/cloud_shader.wgsl`
   - declare `cloud_shadow_tex`
   - add shadow clipmap sampling helpers
   - add displaced shadow-context resampling helpers
   - replace the current shadow path with the hybrid macro/local model
   - retune the dense-light attenuation response

### Recommended

2. `src/render/debug.rs`
   - add new debug modes if separate macro/local shadow inspection is exposed
3. `src/shaders/atmosphere_runtime_shared.wgsl`
   - keep debug constants aligned with `RenderDebugView`
4. `src/shaders/cloud_composite.wgsl`
   - optionally expose the new shadow debug outputs in the final composite path

### Probably Not Required

5. `src/render.rs`
   - likely no change, because binding 7 and the cloud shadow texture already exist
6. `src/world/weather.rs`
   - likely no structural change unless the existing shadow channels need new normalization or interpretation

## Guardrails

Do not do any of the following while implementing this fix:

- Do not do a broad cloud appearance rewrite in the same task.
- Do not start by changing the temporal pass.
- Do not brute-force the problem by blindly increasing the main raymarch step budget.
- Do not keep the old hard `dist_t > 0.50` shadow-quality switch.
- Do not keep reusing the current sample's weather for displaced light-ray taps.
- Do not flatten all cloud lighting just to hide the banding.

## Acceptance Criteria

The task is done only when all of the following are true:

1. The raw cloud pass is materially free of the previous dark striped banding in dense and layered cases.
2. Dense cloud stacks are less dark in a controlled way, not just globally brighter.
3. Far clouds use broad, stable shading rather than structured alternating bands.
4. Near clouds keep readable local volumetric depth.
5. Performance remains reasonable for the current half-resolution cloud pipeline.
6. The new code structure is readable and clearly separated by responsibility.

## Fresh Agent Prompt

Use this prompt on a fresh agent:

```text
Read `docs/CLOUD_SHADOW_BANDING_FIX_PLAN.md` first and treat it as the authoritative implementation plan for this task.

Then inspect the live code paths it references, especially:
- `src/shaders/cloud_shader.wgsl`
- `src/shaders/cloud_temporal.wgsl`
- `src/shaders/cloud_composite.wgsl`
- `src/world/weather.rs`
- `src/render.rs`
- `src/render/debug.rs`

Task:
Implement the plan end to end with professional realtime graphics-engineering quality. The goal is to eliminate the current cloud artifact where dense or layered clouds become too dark and develop moving striped light/dark patches, especially at far distances.

Hard requirements:
- Fix the shadowing and lighting flaw, not just the symptom.
- Do not do a broad cloud look rewrite.
- Do not start by changing temporal accumulation.
- Do not brute-force the issue by blindly increasing main raymarch steps.
- Do not reuse the current sample's weather or AABB for displaced shadow taps.
- Prefer reusing the existing `cloud_shadow_tex` at cloud render binding 7 instead of inventing new renderer resources unless a real code-level constraint forces a change.
- Keep the implementation structurally clean: separate helpers for macro shadow, local self-shadow, shadow blending, and lighting remap.
- Add focused debug instrumentation if needed to validate macro shadow, local shadow, and combined attenuation.
- Validate the raw cloud pass before deciding whether any temporal-pass change is still necessary.

Quality bar:
- Work like a senior realtime graphics / game engine engineer.
- Preserve project structure and keep the code maintainable.
- Make the minimal renderer-side changes needed.
- Keep performance in the same budget class as the current half-resolution cloud path.
- If you discover a concrete mismatch between the plan and the live code, adapt pragmatically and explain the deviation.

Execution guidance:
1. Read the plan.
2. Inspect the referenced code.
3. Implement the hybrid macro + local shadow model cleanly.
4. Retune the dense-volume lighting response only after the shadow model is corrected.
5. Run the strongest available validation you can in this environment.
6. Summarize changed files, the exact root cause you fixed, validation performed, and any remaining risks.

Do not revert unrelated user changes.
```
