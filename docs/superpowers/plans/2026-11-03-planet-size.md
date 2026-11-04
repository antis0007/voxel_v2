# Planet Size Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** New planets are 33% larger (`HALF_M` 32 000 m, radius 40 744 m) while every existing planet keeps 30 558 m, with no change to face addressing.

**Architecture:** The three atlas constants become a `Geometry` value chosen by generator version: `atlas::geometry(version)` returns the old numbers below the size gate and the new ones from it. `WorldSampleConfig::geometry()` is the one accessor for worldgen and the app; `atlas::to_body`/`from_body` take a `Geometry`. The globe shader reads `half_m` from the uniform instead of a constant. The orbit tier threshold becomes the active radius.

**Tech Stack:** Rust; `engine_core`, `engine_worldgen`, app; WGSL globe shader; `just test`, `just test-gpu globe`, captures.

**Spec:** `docs/superpowers/specs/2026-11-03-ambience-fauna-planet-clouds-design.md`, Part C "Planet size" and "Owner decisions".

## Global Constraints

- `PITCH_M` stays 65 536; `HALF_M` new value 32 000 (< 32 768 so `split`/`join` are unchanged).
- Never edit a historical gate: versions below the size gate return exactly `{ half_m: 24_000.0, radius_m: 30_558.0 }`.
- The gate number is the next after the tree's `CURRENT_GENERATOR_VERSION` at implementation time (126 → 127); bump `CURRENT_GENERATOR_VERSION` and `PLANET_GENERATOR_VERSION` together with it. Read `crates/voxel/src/config.rs` first; do not trust this number if the tree moved.
- Saves: no format bytes change; the generator version already in the save selects the geometry.
- Tile cache key already carries `h{half}-r{radius}` (`globe/cache.rs:22`); keep it so old caches stay valid for old worlds.
- `GLOBE_HALF_M` leaves the shader as a constant; the `band.rs:77` text test is replaced by a uniform-value test.
- Captures: globe pose from orbit and a horizon pose at 1.5 km, before and after, same seed, new world only.

## Review Focus

- Loading an old planet save (gen ≤ 126) after the change: terrain, elevator sites, orbital rails and the globe must match pixel-for-pixel; pinned in Task 1 by the fixture round trip at both versions and in Task 5 by the old-world capture.
- A tile cache from an old world opened under the new geometry: key mismatch must rebake, not misplace tiles; pinned in Task 4 by the cache key test.
- Orbit tier hysteresis near the new radius: `TierState` thresholds move with radius; pinned in Task 3.
- Spawn face anchoring (memory: geo solve features need re-anchoring when relief scales): `planet.rs` uses `local / HALF_M` normalised coordinates, so the same seed moves features outward by 4/3; the plan accepts this for new worlds only; pinned by Task 2's determinism test at the old version.
- Chart cell size (`tile.rs:41`, `2 HALF_M / cells`) grows 4/3: LOD level selection and the 192 MiB fine-ring cap must still hold; pinned in Task 4.

---

### Task 1: Versioned geometry in `engine_core`

**Files:**
- Modify: `crates/core/src/cube_sphere/atlas.rs`
- Modify: `crates/core/src/cube_sphere/atlas_tests.rs`
- Modify: `crates/voxel/src/config.rs` (`WorldSampleConfig::geometry`, `SIZE_GENERATOR_VERSION`)

**Interfaces:**
- Produces:

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry { pub half_m: f64, pub radius_m: f64, pub pitch_m: f64 }
pub const LEGACY: Geometry = Geometry { half_m: 24_000.0, radius_m: 30_558.0, pitch_m: 65_536.0 };
pub const LARGE: Geometry = Geometry { half_m: 32_000.0, radius_m: 40_744.0, pitch_m: 65_536.0 };
pub fn geometry(generator_version: u32) -> Geometry; // LARGE from SIZE_GENERATOR_VERSION
pub fn split(p: DVec3) -> (Face, DVec3); // unchanged, pitch only
pub fn join(face: Face, p: DVec3) -> DVec3; // unchanged
pub fn to_body(g: Geometry, p: DVec3) -> DVec3;
pub fn from_body(g: Geometry, p: DVec3) -> DVec3;
```

- `engine_voxel::SIZE_GENERATOR_VERSION: u32` lives next to `PLANET_GENERATOR_VERSION`; `engine_core` cannot see it, so `atlas::geometry` takes the threshold as `atlas::geometry_at(version, gate)` and `WorldSampleConfig::geometry(self)` calls it with the voxel crate's constant. The bare `HALF_M`/`RADIUS_M` constants are removed; the compiler lists every caller.

- [ ] **Step 1: Write the failing tests** in `atlas_tests.rs`:

```rust
#[test]
fn geometry_keeps_the_legacy_sphere_below_the_gate() {
    assert_eq!(geometry_at(126, 127), LEGACY);
    assert_eq!(geometry_at(127, 127), LARGE);
    assert!((LARGE.radius_m - LARGE.half_m * 4.0 / std::f64::consts::PI).abs() < 1.0);
    assert_eq!(LARGE.pitch_m, LEGACY.pitch_m);
}

#[test]
fn body_round_trip_holds_for_both_geometries() {
    for g in [LEGACY, LARGE] {
        for face in Face::ALL {
            let p = join(face, DVec3::new(0.37 * g.half_m, 120.0, -0.81 * g.half_m));
            let back = from_body(g, to_body(g, p));
            assert!((back - p).length() < 1e-6, "{face:?} {g:?}");
        }
    }
}
```

- [ ] **Step 2: Run** `just test geometry_keeps`. Expected: compile failure.
- [ ] **Step 3: Implement** `Geometry`, `geometry_at`, the new `to_body`/`from_body`, the config accessor and `SIZE_GENERATOR_VERSION`. Do not bump `CURRENT_GENERATOR_VERSION` yet.
- [ ] **Step 4: Run** `just test atlas`. Expected: pass in `engine_core`; the workspace does not compile yet (callers), which Tasks 2 to 4 fix.
- [ ] **Step 5: Commit** after Task 4 (the workspace must compile at each commit).

### Task 2: Worldgen callers

**Files:**
- Modify: `crates/worldgen/src/planet.rs`, `basin.rs`, `volcano.rs`, `geo/mod.rs` and their tests (`basin_tests.rs`, `deep_tests.rs`)

**Interfaces:**
- Every `atlas::HALF_M`/`RADIUS_M` read becomes `cfg.geometry().half_m` etc.; functions without a `cfg` gain a `g: Geometry` parameter from their caller.

- [ ] **Step 1: Write the failing test** in `crates/worldgen/src/planet_tests.rs` (or the nearest planet test module): sample the planet surface height at 64 fixed points on each face with `generator_version = 126` and assert equality with a table captured from the current tree (record the table in the test as the pin), then assert that `generator_version = 127` differs at the same world coordinates for at least one point.
- [ ] **Step 2: Run** it before the edits. Expected: compile failure (no `geometry()`), then after the stub, the 126 table must pass unchanged.
- [ ] **Step 3: Implement** the caller changes.
- [ ] **Step 4: Run** `just test planet` and `just test basin`. Expected: pass.

### Task 3: App callers and altitude tiers

**Files:**
- Modify: `crates/app/src/altitude.rs`, `crates/app/src/planet.rs`, `crates/app/src/player.rs`, `crates/app/src/render/planet_frame.rs`, `crates/app/src/render/planet.rs`, `crates/app/src/render/horizon.rs`, `horizon/fine.rs`, `crates/app/src/world/{volcanism.rs,block_data/tnt.rs,scanner/*.rs,mining/charge.rs,storage.rs}`, `crates/app/src/simulation/storms.rs`, `crates/app/src/egui_ui/world_loader/{sphere.rs,survey.rs}`, `crates/app/src/render.rs:4313`

**Interfaces:**
- `altitude::ORBIT_M` becomes `pub fn orbit_m(g: Geometry) -> f64 { g.radius_m }`; `TierState::update(&mut self, altitude_m: f64, g: Geometry)`.
- `cam.planet[0].x` is `g.radius_m`; `planet.rs:58` writes `[radius, half, pitch, 1]` from `g`.

- [ ] **Step 1: Write the failing test** in `altitude.rs` tests: with `LARGE`, `update(39_000.0)` stays `Transition` and `update(41_001.0)` is `Orbit`; with `LEGACY` the existing table holds.
- [ ] **Step 2: Run** `just test tier`. Expected: compile failure.
- [ ] **Step 3: Implement**; every former constant read goes through the world's config (`world.sample_config().geometry()`), threaded as a parameter where no world is in reach.
- [ ] **Step 4: Run** `just check` and `just test altitude`. Expected: pass.

### Task 4: Globe renderer

**Files:**
- Modify: `crates/app/src/render/globe/chart.rs` (consts → `Geometry` field on the chart config), `tile.rs`, `scheduler.rs`, `shell.rs`, `shell_weather.rs`, `band.rs`, `cache.rs`, `globe_driver.rs`, `globe_driver_shell.rs`, `gpu.rs` (`GlobeUniform.half_m`)
- Modify: `crates/app/src/shaders/globe_shader.wgsl:56,113-114` (`GLOBE_HALF_M` → `globe.half_m`)
- Tests: `band.rs`, `shell_tests.rs`, `cache.rs`, `gpu_band_tests.rs`

**Interfaces:**
- `GlobeUniform` gains `half_m: f32` (use one of the pads added by the cloud-handoff plan if it landed first, else add 16 bytes and update the size const).
- `band.rs:77` text test becomes: the composed shader contains `globe.half_m` and no `GLOBE_HALF_M` literal.

- [ ] **Step 1: Write the failing tests**: cache key differs between `LEGACY` and `LARGE` for the same seed; `band.rs` no-literal test; `tile.rs` cell size test at `LARGE` for level 0 equals `2 * 32_000 / TILE_CELLS`.
- [ ] **Step 2: Run** `just test globe`. Expected: FAIL.
- [ ] **Step 3: Implement**.
- [ ] **Step 4: Run** `just test globe`, `just test-gpu globe`, `just check`. Expected: pass.
- [ ] **Step 5: Commit** Tasks 1 to 4 as one commit (the workspace compiles only with all four), message `feat(planet): versioned atlas geometry; new planets 33% larger`. Include `SIZE_GENERATOR_VERSION` dormant (equal to `CURRENT + 1`).

### Task 5: Wake the gate and capture

- [ ] **Step 1: Bump** `CURRENT_GENERATOR_VERSION` and `PLANET_GENERATOR_VERSION` to the size gate in `crates/voxel/src/config.rs`; run the fixture tests `just test generator` and `just test save`. Expected: old fixtures pass unchanged.
- [ ] **Step 2: Captures** before (exe from the commit before Task 5) and after: orbit globe pose and the 1.5 km horizon pose on a new world; old-world pose must be pixel-identical. Paths under `target/review/planet-size/`.
- [ ] **Step 3: Run** `python -I scripts/workflow.py verify`; attest `capture_ab` and `review_save`.
- [ ] **Step 4: Commit** message `feat(worldgen): generator 127, larger planet sphere`.
