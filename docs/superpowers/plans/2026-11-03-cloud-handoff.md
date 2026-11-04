# Cloud Handoff Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rising from the ground to orbit, the volumetric cloud march fades out and the globe's cloud shell fades in over a 2 km band, with no frame where both draw a full deck.

**Architecture:** One scalar `handoff` in 0..1 = `smoothstep(top_slab - 1000, top_slab + 1000, altitude)`. The march computes it in-shader from `cam.camera_pos.y` and the existing layer-bounds function, and scales sample density by `1 - handoff`. The host computes the same value for the globe and passes it in `GlobeUniform`; the shell scales its alpha by it. The march keeps running (and its history valid) up to the band top instead of stopping at `top_slab + 500`. The shell's coverage ramp is refit so the shell shows the same deck fraction the march does.

**Tech Stack:** Rust, wgpu, WGSL composed by `crates/app/src/shader_sources.rs`; GPU tests via `just test-gpu`; captures per `.claude/rules/render.md`.

**Spec:** `docs/superpowers/specs/2026-11-03-ambience-fauna-planet-clouds-design.md`, Part C "Cloud handoff".

## Global Constraints

- Band: 2 km centred on `cloud_top_slab_m` (host) which equals `high_top + 7000` in `cloud_shared.wgsl`; both sides must agree to the metre.
- `GlobeUniform` is a shader contract: its size const `[(); 128]` and the WGSL struct change together.
- No `Instant` in render logic; no new dependency.
- Before/after captures from the same four poses (ground under the deck, top_slab - 800, top_slab + 800, 15 km) under `target/review/cloud-handoff/`; the review world and poses come from the horizon capture runner.
- Shell altitude stays at `SHELL_HEIGHT_M` 1.6 km (writer's call in the spec).
- Comments state why; no banned words.

## Review Focus

- Descending through the band: cloud history must not blend stale frames from above the band; pinned in Task 2 by `march_weight` endpoints and the history rule.
- Storm weather raising `top_slab` while the viewer sits in the band: handoff must track the moving slab, not a cached value; pinned in Task 1 (host) and Task 2 (shader uses live bounds).
- Globe not drawn (Surface tier, globe disabled): shell weight irrelevant, march at full strength; pinned in Task 1 by `march_handoff` when no globe.
- Coverage control at 0 and 1: shell alpha must be 0 at zero coverage and near full at full coverage; pinned in Task 3's fit table ends.
- Nether (`planet[0].w < 0`): no handoff, march unchanged; pinned in Task 2 by the early return in the shader helper mirroring `cloud_planet_drop`.

---

### Task 1: Host handoff weight

**Files:**
- Modify: `crates/app/src/render/globe/shell.rs:18,83-86`
- Modify: `crates/app/src/render/globe_driver.rs:154-157,223`
- Modify: `crates/app/src/render/globe/gpu.rs:22-36` (`GlobeUniform`)
- Modify: `crates/app/src/shaders/globe_shader.wgsl:8-14` (uniform struct)
- Modify: `crates/app/src/render.rs:5087-5092`
- Test: `crates/app/src/render/globe/shell_tests.rs`

**Interfaces:**
- Produces in `shell.rs`: `pub const HANDOFF_HALF_M: f64 = 1_000.0;` and `pub fn march_weight(altitude_m: f64, top_slab_m: f64) -> f32` returning `1 - smoothstep(top_slab - 1000, top_slab + 1000, altitude)`; `volumetric_clouds_enabled` becomes `march_weight(..) > 0.0`.
- `GlobeDriver` stores `handoff: f32` (= `1 - march_weight`) and exposes `pub fn handoff(&self) -> f32`; `march_allowed()` stays `!running || march` where `march = march_weight > 0`.
- `GlobeUniform` gains `pub handoff: f32` after `flags` with three `f32` pads; size const becomes 144; WGSL struct adds `handoff: f32, _pad: vec3<f32>` in the same position.
- `render.rs`: history is invalidated only when `march_allowed()` is false (unchanged line), which now happens above the band top instead of at `top_slab + 500`.

- [ ] **Step 1: Write the failing tests** in `shell_tests.rs`:

```rust
#[test]
fn march_weight_spans_a_two_km_band_around_the_slab() {
    let slab = 7_700.0;
    assert_eq!(march_weight(0.0, slab), 1.0);
    assert_eq!(march_weight(slab - 1_000.0, slab), 1.0);
    assert!((march_weight(slab, slab) - 0.5).abs() < 1e-6);
    assert_eq!(march_weight(slab + 1_000.0, slab), 0.0);
    assert!(volumetric_clouds_enabled(slab + 999.0, slab) && !volumetric_clouds_enabled(slab + 1_000.0, slab));
}
```

- [ ] **Step 2: Run** `just test march_weight_spans`. Expected: compile failure, no `march_weight`.
- [ ] **Step 3: Implement** `march_weight`, the driver field, the uniform field (host and WGSL), and set `uniform.handoff = driver.handoff()` where the globe uniform is filled (`gpu.rs` `prepare` or the driver's upload; find the one site that writes `flags`).
- [ ] **Step 4: Run** `just test globe` and `just test shader_sources`. Expected: pass; the size const assertion compiles at 144.
- [ ] **Step 5: Commit** the five files, message `feat(globe): handoff weight across a 2 km band at the cloud top slab`.

### Task 2: March fades out through the band

**Files:**
- Modify: `crates/app/src/shaders/cloud_shared.wgsl` (new `cloud_handoff(altitude_m: f32) -> f32`)
- Modify: `crates/app/src/shaders/cloud_shader.wgsl:1382-1401` (density scale)
- Test: a `shader_sources` composition test already validates WGSL; add a GPU test in `crates/app/src/render/gpu_cloud_tests.rs` if one exists for the march, else a naga validation assertion in `build.rs`'s existing list is enough.

**Interfaces:**
- `cloud_handoff(altitude_m)` = `smoothstep(top - 1000, top + 1000, altitude_m)` with `top = cloud_layer_bounds(..).high_top + CLOUD_HIGH_ALTITUDE_OFFSET_M`; returns 0 when `cam.planet[0].w < -0.5` (Nether) so the realm march is untouched.
- `cloud_shader.wgsl`: `sample_density *= 1.0 - cloud_handoff(cam.camera_pos.y)` at line 1401 and the same factor on `dist_fade` passed to the shadow trace at 1399.

- [ ] **Step 1: Write the failing check**: in `crates/app/src/shader_sources.rs` tests (or the nearest WGSL text test), assert the composed cloud shader contains `cloud_handoff(` exactly twice in `cloud_shader.wgsl` and once as a definition in `cloud_shared.wgsl`.
- [ ] **Step 2: Run** `just test shader_sources`. Expected: FAIL, zero occurrences.
- [ ] **Step 3: Implement** the helper and the two uses. The host top-slab formula in `shell.rs:75-81` must stay byte-equal to `high_top + 7000` (same `mix` inputs); add a comment at both sites naming the other.
- [ ] **Step 4: Run** `just test shader_sources` and `just test-gpu cloud`. Expected: pass.
- [ ] **Step 5: Commit** message `feat(clouds): march density fades across the handoff band`.

### Task 3: Shell fades in and matches coverage

**Files:**
- Modify: `crates/app/src/shaders/globe_shell.wgsl:22-24,178-196`
- Modify: `crates/app/src/render/globe/shell.rs` (new `pub fn cover_fit() -> (f32, f32)` constants and a coverage mean helper for the test)
- Test: `crates/app/src/render/globe/shell_tests.rs`

**Interfaces:**
- `globe_cover(c)` becomes `smoothstep(COVER_LO, COVER_HI, c) * globe.handoff`; the two constants are emitted from `shell::COVER_FIT` into the shader at compose time (the same mechanism `GLOBE_HALF_M` uses in `band.rs:77`, or a text test that pins the literals).
- Fit: for 16 `CloudControls.coverage` values 0.0..1.0, the host renders the shell coverage field (`refresh_shell` CPU path) and computes mean `smoothstep(lo, hi, c)`; the march's mean deck opacity for the same controls is approximated by the low-layer fade `smoothstep(0.01, 0.12, low_amount) * coverage` from `cloud_shader.wgsl:280`. Builder fits `lo, hi` by a 2-D grid search minimising the max abs error over the table, records the table and the chosen constants in the test.

- [ ] **Step 1: Write the failing test**:

```rust
#[test]
fn shell_cover_tracks_the_march_deck_fraction() {
    let (lo, hi) = COVER_FIT;
    for i in 0..16 {
        let coverage = i as f32 / 15.0;
        let shell = shell_cover_mean(coverage, lo, hi);
        let march = march_deck_fraction(coverage);
        assert!((shell - march).abs() < 0.05, "coverage {coverage}: shell {shell} march {march}");
    }
    assert_eq!(shell_cover_mean(0.0, lo, hi), 0.0);
}
```

- [ ] **Step 2: Run** `just test shell_cover_tracks`. Expected: compile failure.
- [ ] **Step 3: Implement** the helpers and the fit; write the constants into the shader and multiply by `globe.handoff`.
- [ ] **Step 4: Run** `just test globe` and `just test-gpu globe`. Expected: pass.
- [ ] **Step 5: Commit** message `feat(globe): shell alpha follows the handoff and the march's deck fraction`.

### Task 4: Captures and gate

- [ ] **Step 1: Before captures** from the commit before Task 1 (build that exe to a scratch target dir per the memory recipe), four poses, into `target/review/cloud-handoff/before/`.
- [ ] **Step 2: After captures** same poses, `target/review/cloud-handoff/after/`. Expected: at top_slab ± 800 m the deck is one layer, not two; at 15 km only the shell; at ground unchanged (pixel diff under 1% on the ground pose).
- [ ] **Step 3: Run** `python -I scripts/workflow.py verify`; attest `capture_ab` with the two directories.
- [ ] **Step 4: Report** the commit list and capture paths.
