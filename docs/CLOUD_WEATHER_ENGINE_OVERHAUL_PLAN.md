# Cloud + Weather Engine Overhaul Plan

## Scope

Bring the current Rust + WGPU cloud/weather stack from transitional quality to stable high-fidelity quality by fixing:

1. visible seams and clipmap discontinuities
2. striped/banded cloud lighting artifacts
3. chunked/weather-wall update behavior
4. frame-time spikes from weather upload churn
5. weak simulation ownership split between authoring and runtime

This plan is staged to reduce risk: correctness first, then simulation quality, then major architecture upgrades.

## Non-Negotiable Guardrails

- No unmeasured performance claims.
- No “quality off” regressions hidden as optimization.
- No shader-side re-simulation duplication across passes.
- No frame-rate-dependent weather evolution.
- No temporal history invalidation spam from routine weather updates.

## Baseline Instrumentation (Required Before Major Refactors)

1. Track per-frame:
   - cloud raymarch GPU ms
   - cloud temporal GPU ms
   - cloud composite GPU ms
   - weather upload bytes
   - weather upload CPU ms
   - cloud history invalidation reason counts
2. Add debug outputs for:
   - macro shadow OD
   - local self-shadow OD
   - combined shadow attenuation
   - weather clipmap blend weights
3. Capture benchmark presets:
   - clear/fair/storm
   - static camera + orbit camera
   - 1080p and higher render scales

## Phase 1 (Now): Correctness + Stability Hotfixes

Implemented in this patch:

- Incremental weather simulation updates instead of periodic full-field refresh.
- Fixed-step weather runtime integration in main loop.
- Cloud history invalidation only on full-level weather refreshes.
- Far clipmap fallback weighting made seam-safe.
- Hybrid cloud shadow path:
  - macro shadow from shared cloud-shadow clipmap
  - local self-shadow from displaced per-tap context re-sampling
  - continuous macro/local blend by footprint and distance
- Removed hard far cheap-shadow branch causing banding/stripes.

Acceptance criteria:

- No obvious edge seam from far clipmap transitions.
- No moving zebra striping in dense cloud stacks.
- Lower frequency of cloud temporal invalidations under normal movement.

## Phase 2: Weather Runtime State Separation

Target:

- Authoring controls are forcing terms only.
- Runtime owns persistent fields:
  - humidity
  - uplift/convection proxy
  - storm organization
  - precipitation tendency
  - layered wind (low/mid/high vectors)

Deliverables:

1. Extend per-texel runtime storage from packed products to explicit simulation fields.
2. Keep product packing as renderer interface (primary/secondary/shadow textures).
3. Maintain clipmap scroll retention and dirty-rect uploads.

Acceptance criteria:

- Authoring change does not instantly replace weather identity.
- Weather remains stable under camera motion and frame-rate changes.

## Phase 3: Tile Scheduler + Budgeted Weather Evolution

Target:

- Deterministic update budget per clipmap level with no full-level refresh in normal operation.

Deliverables:

1. Tile scheduler (e.g., 16x16 texel tiles), priority by:
   - newly exposed scroll regions
   - near-level simulation cadence
   - aging/fairness queue
2. Upload manifest aggregation:
   - merge adjacent dirty rects
   - cap upload bytes/frame with carry-over queue

Acceptance criteria:

- Weather upload spikes remain bounded.
- No visible “wall” stepping during weather evolution.

## Phase 4: Cloud Field Architecture Upgrade

Target:

- Transition from purely analytic layered density to resident cloud data ownership.

Deliverables:

1. `render/clouds` ownership expansion:
   - clipmap rings
   - brick coordinates
   - residency metadata
2. Dual-path migration:
   - analytic path remains as fallback
   - resident path added behind debug/feature gate
3. Shared data source for:
   - visible cloud rendering
   - cloud shadow products

Acceptance criteria:

- Near and far cloud identity no longer diverges.
- Terrain/cloud shadow coherence improves without duplicate logic.

## Phase 5: Temporal + Reconstruction Hardening

Target:

- Temporal accumulation smooths noise without carrying stale cloud structure.

Deliverables:

1. History confidence from:
   - reprojection validity
   - neighborhood variance
   - weather update footprint overlap
2. Optional reactive mask from weather dirty rects.
3. Keep temporal as support system, never primary quality source.

Acceptance criteria:

- Reduced shimmer at silhouettes.
- No temporal ghosting after weather/front movement.

## Phase 6: Scalability and Quality Tiers

Target:

- Stable quality across GPU classes without code-path divergence.

Deliverables:

1. Explicit quality tiers:
   - march step budgets
   - shadow tap budgets
   - cloud render scale
2. Runtime auto-tuning from profiler feedback (bounded and hysteresis-based).

Acceptance criteria:

- Predictable frame pacing.
- No sudden quality popping from uncontrolled runtime adaptation.

## Regression Test Matrix

Run at each phase:

1. Fast camera strafe at low altitude over mixed terrain.
2. High-altitude flight through multi-layer clouds.
3. Storm preset with heavy cloud overlap and low sun angle.
4. Pause/resume and large camera cuts.
5. Weather randomization and preset switching.

Pass conditions:

- No clip seams.
- No moving light/dark striping artifacts.
- No cloud-in-front-of-foreground-terrain failures.
- No frame-time spikes tied to weather upload bursts.
