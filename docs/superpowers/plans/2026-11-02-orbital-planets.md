# Orbital Planets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Draw the home planet from 1.5 km to high orbit as one lit, weathered, continuous body baked from the same worldgen as the voxels, at a flat frame cost, while chunk streaming shrinks with altitude and pauses in orbit.

**Architecture:** A cube-sphere quadtree of 64x64 height and material tiles (`render/globe/`), baked off the render thread from `engine_worldgen::sample_column` under a `Budget`, cached on disk per seed and generator version, selected by projected pixel error each frame, and drawn by one new shader that folds chart coordinates onto the sphere, shades with the existing atmosphere LUTs, a baked cloud shell and night lights. An altitude tier (`altitude.rs`) drives streaming interest, the cloud path switch and the globe's visibility.

**Tech Stack:** Rust 1.93, wgpu, WGSL (`shader_sources.rs` composition), `engine_core::Budget`, `engine_core::cube_sphere::atlas` (uncommitted peer file; see Global Constraints), `engine_worldgen`, `engine_space`, `scripts/frame_bench.py`.

**Spec:** `docs/superpowers/specs/2026-11-02-orbital-planets-design.md`

## Global Constraints

- Planet: radius 20,371 m, face half-width 16,000 m, chart pitch 65,536 m, tangent projection (`atlas::HALF_M`, `atlas::RADIUS_M`, `atlas::PITCH_M`). The atlas file is a peer's uncommitted work: Task 0 is blocked until it is committed, and this branch never edits it.
- Altitude tiers: Surface below 1,500 m; Transition 1,500 to 30,000 m; Orbit above 30,000 m; hysteresis 200 m on every edge. Loading stops at 6,000 m.
- Frame budget: orbit-static p95 under 8 ms and ascent p99 under 16.6 ms with no frame over 40 ms on the RTX 4090 at 1600x900; orbit-static p95 under 33 ms on the UHD 770 lean tier; globe CPU per frame under 0.5 ms; GPU uploads under 2 MB per frame; bake budget 2 ms per frame on the worker.
- Tile: 64x64 cells, 65x65 samples; height `i16` metres above sea level; material `u8` palette index; water `u1`; quadtree levels 0 to 10 (level 10 cell = 32,000 / (64 x 1024) = 0.49 m, never selected above 1.5 km; level 6 cell = 7.8 m).
- Projected error threshold 1.5 px; split when error over 1.5 px, merge when under 0.75 px.
- Determinism: bake output is a pure function of (seed, generator version, tile id). No `rand`, no `Instant::now()` in bake or selection; the worker thread owns the only wall clock through `Budget::wall`.
- Every bounded pass takes a `Budget`.
- No new crate dependency. No `unwrap`/`expect` outside tests.
- Writing: comments state why; UI text follows `egui_ui/UI_RULES.md`; no new proper nouns.
- Visual changes: before and after captures through `VOXEL_PLANET_FLIGHT_REVIEW=1` at every stage, saved under `target/reviews/<label>/`.
- Shaders: every new `.wgsl` is listed in `crates/app/src/shader_sources.rs`; host uniform structs and WGSL structs change in the same patch with size assertions.
- No file over 300 lines when it can be split; `repo_layout` test must pass.

## Review Focus

1. A tile whose chart rect straddles a face edge (level 0 corners) must fold with no crack: Task 3 tests `TileId::corners` on all six faces at level 0 and 1, and Task 7's seam test renders two adjacent faces and asserts no background pixel on the shared edge.
2. Descending fast from orbit (64 m/s x4 boost = 256 m/s) must not stall: Task 2's hysteresis test crosses 30,000 m and 6,000 m in one tick and asserts loading resumes on the next tick, and Task 12's ascent scene gate covers the real path.
3. Generator version change invalidates the disk cache: Task 5 tests that a cache written for version N is ignored for N+1.
4. The worker thread dying or the save directory being read-only must degrade to in-memory bakes, not a panic: Task 5 tests the read-only path; Task 6 tests a scheduler whose worker is replaced by a closure that returns `None`.
5. Iris-style depth agreement: resident chunks drawn inside the disc must win over the globe under them, and the globe must win over nothing drawn at all: Task 10's coverage test asserts the globe fragment discards inside the resident mask and draws outside it.

---

### Task 0: Branch and dependency gate

**Files:**
- Create: branch `orbital-globe` from `growth-xmb` HEAD after the peer commits `crates/core/src/cube_sphere/atlas.rs`, `crates/app/src/planet.rs`, `crates/app/src/planet/review.rs`, `crates/app/src/render/planet.rs`.

**Interfaces:**
- Produces: `engine_core::cube_sphere::atlas::{HALF_M, RADIUS_M, PITCH_M, split, join, to_body, from_body, frame}` available to every later task.

- [ ] **Step 1: Verify the atlas is committed**

Run: `git log --oneline -1 -- crates/core/src/cube_sphere/atlas.rs`
Expected: one commit hash. If empty, stop: report that the plan is blocked on the peer's commit.

- [ ] **Step 2: Create the branch in a scratch worktree**

```bash
git worktree add -b orbital-globe <scratchpad>/wt growth-xmb
```

- [ ] **Step 3: Build once with a scratch target**

Run: `CARGO_TARGET_DIR=<scratchpad>/target cargo build --profile release-fast -p minecraft_style_voxel_prototype`
Expected: `Finished`. Copy the exe aside as `globe_base.exe`.

- [ ] **Step 4: Record the before captures**

Run: `VOXEL_PLANET_FLIGHT_REVIEW=1 VOXEL_CAPTURE_DIR=<scratchpad>/before globe_base.exe --benchmark-world=planet --benchmark-seed=74 --benchmark-duration=120 --benchmark-warmup=6 --benchmark-width=1600 --benchmark-height=900`
Expected: `01-ground.png` to `06-landed.png` in `<scratchpad>/before`, each `[planet-flight]` line printed. Copy to `target/reviews/orbital-globe-before/`.

---

### Task 1: Altitude tier

**Files:**
- Create: `crates/app/src/altitude.rs`
- Modify: `crates/app/src/main.rs` (add `mod altitude;`)
- Test: `crates/app/src/altitude.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Produces:
  - `pub enum Tier { Surface, Transition, Orbit }`
  - `pub struct TierState { tier: Tier }` with `pub fn new() -> TierState`, `pub fn update(&mut self, altitude_m: f64) -> Tier`, `pub fn tier(&self) -> Tier`
  - `pub const TRANSITION_M: f64 = 1_500.0; pub const ORBIT_M: f64 = 30_000.0; pub const LOAD_STOP_M: f64 = 6_000.0; pub const HYSTERESIS_M: f64 = 200.0;`
  - `pub fn resident_radius_chunks(altitude_m: f64, view_distance_chunks: i32) -> i32` (full view distance below `TRANSITION_M`, linear to 0 at `LOAD_STOP_M`, 0 above)
  - `pub fn altitude_m(eye_m: glam::DVec3, planet: bool) -> f64` (planet: `atlas::split(eye_m).1.y`; flat: `eye_m.y`)

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn tier_edges_carry_200_m_of_hysteresis() {
    let mut s = TierState::new();
    assert_eq!(s.update(1_400.0), Tier::Surface);
    assert_eq!(s.update(1_600.0), Tier::Surface);      // needs 1_500 + 200
    assert_eq!(s.update(1_701.0), Tier::Transition);
    assert_eq!(s.update(1_400.0), Tier::Transition);   // needs 1_500 - 200
    assert_eq!(s.update(1_299.0), Tier::Surface);
    assert_eq!(s.update(30_201.0), Tier::Orbit);
    assert_eq!(s.update(29_801.0), Tier::Orbit);
    assert_eq!(s.update(29_799.0), Tier::Transition);
}

#[test]
fn a_256_m_descent_step_resumes_loading_next_tick() {
    let mut s = TierState::new();
    s.update(31_000.0);
    assert_eq!(s.update(5_900.0), Tier::Transition);
    assert!(resident_radius_chunks(5_900.0, 28) >= 1);
}

#[test]
fn resident_radius_shrinks_linearly_to_zero_at_six_km() {
    assert_eq!(resident_radius_chunks(0.0, 28), 28);
    assert_eq!(resident_radius_chunks(1_500.0, 28), 28);
    assert_eq!(resident_radius_chunks(3_750.0, 28), 14);
    assert_eq!(resident_radius_chunks(6_000.0, 28), 0);
    assert_eq!(resident_radius_chunks(50_000.0, 28), 0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p minecraft_style_voxel_prototype altitude::`
Expected: FAIL, `altitude` module not found.

- [ ] **Step 3: Implement `altitude.rs`**

Rounding in `resident_radius_chunks`: `((1.0 - (altitude - TRANSITION_M) / (LOAD_STOP_M - TRANSITION_M)) * view_distance as f64).round()` clamped to `0..=view_distance`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p minecraft_style_voxel_prototype altitude::`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/altitude.rs crates/app/src/main.rs
git commit -m "app: altitude tier with hysteresis and resident radius"
```

---

### Task 2: Streaming follows the altitude tier

**Files:**
- Modify: `crates/app/src/render/streaming/visibility.rs` (visible-set radius takes a chunk radius argument)
- Modify: `crates/app/src/main.rs` (the two streaming spans: pass `altitude::resident_radius_chunks` and skip `enqueue_missing`/`expand_visible` in `Tier::Orbit`)
- Modify: `crates/app/src/render/streaming/residency.rs` (`pub fn freeze(&mut self, frozen: bool)`: no evictions while frozen)
- Test: `crates/app/src/render/streaming/altitude_tests.rs` (new, listed with `#[cfg(test)] mod altitude_tests;` in `streaming.rs`)

**Interfaces:**
- Consumes: Task 1 `resident_radius_chunks`, `Tier`.
- Produces: `ChunkCache::update_center_chunk_with_snapshot_radius(&mut self, center: IVec3, snapshot: &WorldSnapshot, radius_chunks: i32) -> bool`; `ChunkCache::freeze(&mut self, frozen: bool)`; `ChunkCache::frozen(&self) -> bool`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn visible_set_is_empty_at_radius_zero_and_full_at_view_distance() {
    let mut cache = ChunkCache::new(IVec3::ZERO);
    let snapshot = WorldSnapshot::test_flat(74);
    cache.update_center_chunk_with_snapshot_radius(IVec3::ZERO, &snapshot, 0);
    assert!(cache.visible_keys.is_empty());
    cache.update_center_chunk_with_snapshot_radius(IVec3::ZERO, &snapshot, 4);
    assert!(cache.visible_keys.iter().all(|k| (k.coord - IVec3::ZERO).abs().max_element() <= 4));
    assert!(!cache.visible_keys.is_empty());
}

#[test]
fn frozen_cache_keeps_published_meshes_under_pressure() {
    let mut cache = ChunkCache::new(IVec3::ZERO);
    cache.freeze(true);
    // fill past the gpu budget with placeholder meshes, then trim
    cache.residency_limits.1 = 1;
    cache.install_test_mesh(IVec3::new(9, 0, 9), 4096);
    cache.trim_lod_budget();
    assert!(cache.gpu_meshes.contains_key(&IVec3::new(9, 0, 9)));
    cache.freeze(false);
    cache.trim_lod_budget();
    assert!(!cache.gpu_meshes.contains_key(&IVec3::new(9, 0, 9)));
}
```

`WorldSnapshot::test_flat` and `install_test_mesh` are `#[cfg(test)]` helpers added in this task if absent.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p minecraft_style_voxel_prototype altitude_tests::`
Expected: FAIL, method not found.

- [ ] **Step 3: Implement the radius argument and freeze**

`update_center_chunk_with_snapshot` keeps its signature and calls the `_radius` form with `render_distance_settings().view_distance_chunks`. In `main.rs` both spans compute `let tier = altitude_state.update(altitude::altitude_m(precise_eye_m, world.world_type == WorldType::Planet));` once per frame, call the radius form, and when `tier == Tier::Orbit` call `chunk_cache.freeze(true)` and skip `expand_visible`, `enqueue_missing` and `set_quality_columns`; otherwise `freeze(false)`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p minecraft_style_voxel_prototype streaming::`
Expected: all pass, including the existing streaming tests.

- [ ] **Step 5: Verify no loading in orbit by log**

Run the before exe route from Task 0 with the new exe and `grep "stream-profile" out.txt | tail -3` after the `03-space` line.
Expected: `generating=0 meshing=0 pending=0` on every line after `03-space` until `05-descent`.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/render/streaming.rs crates/app/src/render/streaming/visibility.rs crates/app/src/render/streaming/residency.rs crates/app/src/render/streaming/altitude_tests.rs crates/app/src/main.rs
git commit -m "streaming: resident disc shrinks with altitude; orbit freezes the cache"
```

---

### Task 3: Globe tile addressing

**Files:**
- Create: `crates/app/src/render/globe.rs` (module root: `pub mod tile; pub mod bake; pub mod cache; pub mod scheduler; pub mod gpu; pub mod shell;`)
- Create: `crates/app/src/render/globe/tile.rs`
- Modify: `crates/app/src/render.rs` (add `pub mod globe;`)
- Test: inline in `tile.rs`

**Interfaces:**
- Produces:
  - `pub const TILE_CELLS: u32 = 64; pub const TILE_SAMPLES: u32 = 65; pub const MAX_LEVEL: u8 = 10;`
  - `#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)] pub struct TileId { pub face: u8, pub level: u8, pub x: u32, pub y: u32 }`
  - `impl TileId { pub fn root(face: u8) -> TileId; pub fn children(self) -> [TileId; 4]; pub fn parent(self) -> Option<TileId>; pub fn cell_m(self) -> f64 /* 32_000 / (64 << level) */; pub fn chart_min(self) -> glam::DVec2 /* face-local metres, -16_000.. */; pub fn chart_max(self) -> glam::DVec2; pub fn sample_chart(self, i: u32, j: u32) -> glam::DVec2; pub fn centre_body(self) -> glam::DVec3; pub fn corners_body(self) -> [glam::DVec3; 4]; pub fn bounding_radius_m(self) -> f64; pub fn key(self) -> u64 }`
  - `pub fn projected_error_px(id: TileId, eye_body: glam::DVec3, focal_px: f64) -> f64` = `focal_px * id.cell_m() / distance(eye_body, nearest point of the tile's bounding sphere)`, where the sphere centre is `centre_body()` and radius `bounding_radius_m()`; returns `0.0` when the tile is behind the planet limb (centre direction dot eye direction below the horizon cosine `RADIUS_M / |eye_body|`).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn levels_halve_the_cell_and_quarter_the_rect() {
    let root = TileId::root(2);
    assert_eq!(root.cell_m(), 500.0);
    let kids = root.children();
    assert!(kids.iter().all(|k| k.cell_m() == 250.0 && k.parent() == Some(root)));
    assert_eq!(TileId { face: 2, level: 10, x: 1023, y: 1023 }.cell_m(), 32_000.0 / 65_536.0);
}

#[test]
fn level_0_corners_land_on_the_sphere_and_meet_across_faces() {
    for face in 0..6u8 {
        for p in TileId::root(face).corners_body() {
            assert!((p.length() - atlas::RADIUS_M).abs() < 1e-6, "face {face}");
        }
    }
    // the +X and +Z faces share an edge: their shared corner set must coincide
    let a = TileId::root(0).corners_body();
    let b = TileId::root(2).corners_body();
    let shared = a.iter().filter(|p| b.iter().any(|q| p.distance(**q) < 1e-6)).count();
    assert_eq!(shared, 2);
}

#[test]
fn samples_cover_the_rect_inclusive_and_match_the_chart() {
    let id = TileId { face: 3, level: 4, x: 5, y: 9 };
    assert_eq!(id.sample_chart(0, 0), id.chart_min());
    assert_eq!(id.sample_chart(64, 64), id.chart_max());
    assert!((id.chart_max().x - id.chart_min().x - 64.0 * id.cell_m()).abs() < 1e-9);
}

#[test]
fn projected_error_is_zero_behind_the_limb_and_scales_with_distance() {
    let eye = DVec3::new(0.0, 0.0, atlas::RADIUS_M + 100_000.0);
    let front = TileId::root(/* face whose centre is +Z */ 2);
    let back = TileId::root(/* face whose centre is -Z */ 3);
    assert_eq!(projected_error_px(back, eye, 900.0), 0.0);
    let near = projected_error_px(front, eye, 900.0);
    let far = projected_error_px(front, eye * 2.0, 900.0);
    assert!(near > far && far > 0.0);
}
```

Face index to axis mapping comes from `engine_core::cube_sphere::Face::ALL`; the test picks the faces by checking `centre_body().z.signum()` rather than hard-coding 2 and 3.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p minecraft_style_voxel_prototype globe::tile::`
Expected: FAIL, module not found.

- [ ] **Step 3: Implement `tile.rs`**

`centre_body` and `corners_body` go through `atlas::to_body(atlas::join(face, DVec3::new(s, 0.0, t)))`. `bounding_radius_m` is the max corner distance from the centre plus `cell_m() * 2.0` (height slack: terrain is under 2 km tall, so add `2_000.0` at levels 0 to 3 and `cell_m() * 2.0` above).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p minecraft_style_voxel_prototype globe::tile::`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/render.rs crates/app/src/render/globe.rs crates/app/src/render/globe/tile.rs
git commit -m "globe: cube-sphere tile addressing and projected error"
```

---

### Task 4: Tile bake from worldgen

**Files:**
- Create: `crates/app/src/render/globe/bake.rs`
- Test: inline

**Interfaces:**
- Consumes: Task 3 `TileId`, `TILE_SAMPLES`; `engine_worldgen::sample_column(&WorldSampleConfig, i32, i32) -> ColumnSample` (fields `height: i32`, `surface: BlockId`); `engine_worldgen::landscape::visual_ecology(cfg, x, z, y) -> (heat, wet, _)`; `engine_core::Budget`.
- Produces:
  - `pub struct TileData { pub id: TileId, pub height_m: Vec<i16> /* 65*65, metres above sea level, sea floor kept */, pub material: Vec<u8> /* 65*65 palette index */, pub water: Vec<u8> /* 65*65 bitmask bytes, 1 = water surface */, pub climate: Vec<[u8; 2]> /* heat, wet 0..255 */ }`
  - `pub struct BakeCursor { pub next_sample: u32 }` (resume point)
  - `pub fn bake_tile(cfg: &WorldSampleConfig, id: TileId, data: &mut TileData, cursor: &mut BakeCursor, budget: Budget) -> bool` (returns `true` when complete; samples in row-major order from `cursor.next_sample`; stops when `!budget.has_time()` after every 64 samples)
  - `pub fn material_index(surface: BlockId) -> u8` (palette: 0 grass, 1 dirt, 2 stone, 3 sand, 4 snow, 5 gravel, 6 mud or clay, 7 sandstone, 8 leaves, 9 log, 10 ice, 255 other)
  - `pub fn sample_coord(cfg: &WorldSampleConfig, id: TileId, i: u32, j: u32) -> (i32, i32)` (chart metres through `atlas::join`, divided by `cfg.voxel_meters()`, floored)

- [ ] **Step 1: Write the failing tests**

```rust
fn cfg() -> WorldSampleConfig { WorldSampleConfig { generator_version: 53, seed: 74, world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter } }

#[test]
fn baked_heights_and_materials_equal_the_voxel_columns() {
    let id = TileId { face: 1, level: 6, x: 33, y: 20 };
    let mut data = TileData::empty(id);
    let mut cursor = BakeCursor::default();
    assert!(bake_tile(&cfg(), id, &mut data, &mut cursor, Budget::UNBOUNDED));
    for (i, j) in [(0, 0), (64, 64), (17, 41), (64, 0)] {
        let (x, z) = sample_coord(&cfg(), id, i, j);
        let column = engine_worldgen::sample_column(&cfg(), x, z);
        let k = (j * TILE_SAMPLES + i) as usize;
        assert_eq!(data.height_m[k] as i32, cfg().world_y(column.height).round() as i32);
        assert_eq!(data.material[k], material_index(column.surface));
        assert_eq!(data.water[k], u8::from(column.height < cfg().water_level()));
    }
}

#[test]
fn a_bake_interrupted_by_its_budget_resumes_to_the_same_result() {
    let id = TileId { face: 4, level: 5, x: 3, y: 30 };
    let mut whole = TileData::empty(id);
    bake_tile(&cfg(), id, &mut whole, &mut BakeCursor::default(), Budget::UNBOUNDED);
    let mut pieces = TileData::empty(id);
    let mut cursor = BakeCursor::default();
    let mut rounds = 0;
    while !bake_tile(&cfg(), id, &mut pieces, &mut cursor, Budget::wall(Duration::from_micros(200))) { rounds += 1; assert!(rounds < 10_000); }
    assert!(rounds > 0);
    assert_eq!(whole.height_m, pieces.height_m);
    assert_eq!(whole.material, pieces.material);
}

#[test]
fn negative_and_far_face_coordinates_bake_without_overflow() {
    for id in [TileId { face: 5, level: 10, x: 1023, y: 0 }, TileId { face: 0, level: 10, x: 0, y: 1023 }] {
        let mut data = TileData::empty(id);
        assert!(bake_tile(&cfg(), id, &mut data, &mut BakeCursor::default(), Budget::UNBOUNDED));
        assert!(data.height_m.iter().all(|h| (-2_000..4_000).contains(&(*h as i32))));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p minecraft_style_voxel_prototype globe::bake::`
Expected: FAIL, module not found.

- [ ] **Step 3: Implement `bake.rs`**

Heights clamp to `i16`; climate bytes come from `visual_ecology` at the column height. `TileData::empty` allocates all four vectors at `65 * 65`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p minecraft_style_voxel_prototype globe::bake::`
Expected: 3 passed.

- [ ] **Step 5: Measure one bake**

Run: `cargo test --release -p minecraft_style_voxel_prototype globe::bake::baked_heights -- --nocapture` with a temporary `eprintln!` of elapsed time around `bake_tile`.
Expected: under 60 ms for 4,225 samples (about 14 µs per column). Record the number in the commit message; remove the print.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/render/globe/bake.rs
git commit -m "globe: bake 65x65 height, material, water and climate tiles under a budget"
```

---

### Task 5: Disk cache

**Files:**
- Create: `crates/app/src/render/globe/cache.rs`
- Test: inline (uses `tempfile`-free temp dirs via `std::env::temp_dir().join(unique)`)

**Interfaces:**
- Consumes: Task 4 `TileData`, Task 3 `TileId::key`.
- Produces:
  - `pub struct TileCache { root: PathBuf, seed: u32, generator_version: u32, writable: bool }`
  - `pub fn open(save_dir: &Path, seed: u32, generator_version: u32) -> TileCache` (directory `<save_dir>/globe/<seed>-g<version>/`; `writable = create_dir_all` succeeded)
  - `pub fn load(&self, id: TileId) -> Option<TileData>` (file `<key hex>.gtl`: magic `VXGT0001`, then `height_m` as little-endian `i16`, `material`, `water`, `climate`; any length or magic mismatch returns `None`)
  - `pub fn store(&self, data: &TileData) -> bool` (write to `.tmp` then rename; `false` when not writable)
  - `pub const MAGIC: &[u8; 8] = b"VXGT0001";`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn round_trip_restores_every_field() { /* store then load, assert_eq on all four vectors and id */ }

#[test]
fn a_generator_bump_misses_the_old_cache() {
    let dir = temp();
    let a = TileCache::open(&dir, 74, 53);
    a.store(&sample_data());
    let b = TileCache::open(&dir, 74, 54);
    assert!(b.load(sample_data().id).is_none());
}

#[test]
fn a_truncated_file_is_a_miss_not_a_panic() { /* write 20 bytes to the expected path; load() is None */ }

#[test]
fn a_read_only_root_degrades_to_misses() {
    let cache = TileCache::open(Path::new("Z:/nonexistent/globe"), 74, 53);
    assert!(!cache.writable);
    assert!(!cache.store(&sample_data()));
    assert!(cache.load(sample_data().id).is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p minecraft_style_voxel_prototype globe::cache::`
Expected: FAIL, module not found.

- [ ] **Step 3: Implement `cache.rs`**

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p minecraft_style_voxel_prototype globe::cache::`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/render/globe/cache.rs
git commit -m "globe: per-seed tile cache with atomic writes"
```

---

### Task 6: Scheduler (selection, bake queue, worker)

**Files:**
- Create: `crates/app/src/render/globe/scheduler.rs` (selection and queue; pure, no GPU)
- Create: `crates/app/src/render/globe/worker.rs` (thread, channel, `Budget::wall`)
- Test: inline in `scheduler.rs`

**Interfaces:**
- Consumes: Task 3 `TileId`, `projected_error_px`; Task 4 `bake_tile`, `TileData`, `BakeCursor`; Task 5 `TileCache`.
- Produces:
  - `pub struct Selection { pub draw: Vec<TileId>, pub wanted: Vec<TileId> /* missing children, nearest first */ }`
  - `pub fn select(ready: &dyn Fn(TileId) -> bool, eye_body: DVec3, focal_px: f64, max_draw: usize /* 512 */) -> Selection`: breadth-first from the six roots; a tile splits when `projected_error_px > 1.5` and all four children are ready; a tile with error over 1.5 whose children are not all ready is drawn itself and its missing children are `wanted`; tiles with error `0.0` (behind the limb) are dropped; a tile is never split below `MAX_LEVEL`; a split that is already drawn at the child level merges only when the parent's error is under `0.75` (hysteresis uses the previous frame's `draw` passed back in as `ready`'s complement; the test fixes this by calling `select` twice).
  - `pub struct Worker` with `pub fn spawn(cfg: WorldSampleConfig, cache: TileCache, budget_per_turn: Duration /* 2 ms */) -> Worker`, `pub fn request(&self, ids: &[TileId])` (dedup, nearest first), `pub fn drain(&self, max_bytes: usize /* 2 MB */) -> Vec<TileData>`, `pub fn alive(&self) -> bool`.
  - Worker loop: pop nearest id; `cache.load` else bake with `Budget::wall(budget_per_turn)`, resuming the same tile next turn; `cache.store`; send. A panic in the thread sets `alive() == false` and `drain` returns what it has.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn roots_draw_until_children_are_ready_then_split() {
    let eye = DVec3::new(0.0, 0.0, atlas::RADIUS_M + 2_000.0);
    let none = |_: TileId| false;
    let s = select(&none, eye, 900.0, 512);
    assert!(s.draw.iter().all(|t| t.level == 0));
    assert!(!s.wanted.is_empty() && s.wanted.iter().all(|t| t.level == 1));
    let all = |_: TileId| true;
    let s = select(&all, eye, 900.0, 512);
    assert!(s.draw.iter().any(|t| t.level >= 6), "2 km up wants 7.8 m cells under the eye");
    assert!(s.draw.iter().all(|t| t.level <= 10));
    assert!(s.draw.len() <= 512);
}

#[test]
fn tiles_behind_the_limb_are_never_drawn_or_wanted() {
    let eye = DVec3::new(0.0, 0.0, atlas::RADIUS_M + 400_000.0);
    let all = |_: TileId| true;
    let s = select(&all, eye, 900.0, 512);
    assert!(s.draw.iter().all(|t| t.centre_body().dot(eye) > 0.0));
}

#[test]
fn wanted_tiles_are_nearest_first() { /* distances of s.wanted to eye are non-decreasing */ }

#[test]
fn a_dead_worker_reports_not_alive_and_drains_cleanly() {
    let w = Worker::spawn_with(|_, _| panic!("bake failed"), ...);
    w.request(&[TileId::root(0)]);
    std::thread::sleep(Duration::from_millis(50));
    assert!(!w.alive());
    assert!(w.drain(2 << 20).is_empty());
}
```

`Worker::spawn_with` takes the bake closure so the test can inject a panic; `spawn` passes `bake_tile`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p minecraft_style_voxel_prototype globe::scheduler::`
Expected: FAIL.

- [ ] **Step 3: Implement `scheduler.rs` and `worker.rs`**

Channels: `std::sync::mpsc` both ways; the request side replaces the queue wholesale each frame (newest selection wins). `drain` stops before the tile that would exceed `max_bytes` (a tile is `65*65*(2+1+1+2)` = 25,350 bytes, so 2 MB is about 80 tiles).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p minecraft_style_voxel_prototype globe::scheduler::`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/render/globe/scheduler.rs crates/app/src/render/globe/worker.rs
git commit -m "globe: error-driven tile selection and a budgeted bake worker"
```

---

### Task 7: GPU tiles and the globe shader

**Files:**
- Create: `crates/app/src/render/globe/gpu.rs` (tile textures, index buffer, uniform, pipeline, draw)
- Create: `crates/app/src/shaders/globe_shader.wgsl`
- Modify: `crates/app/src/shader_sources.rs` (list `globe_shader.wgsl`, composed with `atmosphere_runtime_shared.wgsl`)
- Modify: `crates/app/src/render.rs` (own `globe::gpu::Globe`, draw it in the terrain pass before the horizon/cover block when `altitude::Tier != Surface` and `world_type == Planet`)
- Test: `crates/app/src/render/globe/gpu_tests.rs` (`gpu_*` tests, run by `just test-gpu`)

**Interfaces:**
- Consumes: Task 6 `Selection`, Task 4 `TileData`; existing `planet::camera` uniform (`cam.planet`), atmosphere group bindings (`sky_probe`, transmittance LUT, multiscatter LUT), `REVERSE_Z` projection, `self.draw_offsets`.
- Produces:
  - `#[repr(C)] pub struct GlobeUniform { pub eye_body: [f32; 4] /* f32 of eye_body - tile_origin handled per tile */, pub sun_dir_body: [f32; 4], pub radius_m: f32, pub sea_level_m: f32, pub focal_px: f32, pub flags: u32 /* bit 0 lean tier, bit 1 cloud shadow on, bit 2 night lights on */, pub limb: [f32; 4] /* glow colour rgb, strength */ }` with `const _: [(); 64] = [(); size_of::<GlobeUniform>()];` and the WGSL `struct GlobeUniform` of the same 64 bytes in the same patch.
  - Per tile: one `Rg16Sint` height texture 65x65 (height, material in the second channel), one `Rg8Unorm` climate texture, a shared 64x64 grid index buffer (24,576 indices, strips by rows), per-tile push data through a storage buffer `TileInstance { origin_body: [f32; 4] /* tile centre minus eye, f32 after f64 subtraction */, chart_min: [f32; 2], cell_m: f32, face: u32 }`.
  - `pub struct Globe { .. }` with `pub fn new(device, layouts) -> Globe`, `pub fn upload(&mut self, queue, tiles: Vec<TileData>)`, `pub fn evict_unused(&mut self, keep: &[TileId], max_resident: usize /* 2048 */)`, `pub fn prepare(&mut self, queue, selection: &Selection, eye_body: DVec3, uniform: GlobeUniform)`, `pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>)`, `pub fn resident(&self) -> usize`.
  - Vertex stage: grid vertex (i, j) reads height from the tile texture, builds chart point `chart_min + (i, j) * cell_m`, folds through the tangent projection (port `atlas::to_body` to WGSL as `fn globe_fold(face: u32, s: f32, t: f32, h: f32) -> vec3<f32>` on a camera-relative origin), outputs body-space normal from the height texture's 4 neighbours (cell_m spacing) rotated into the tangent frame.
  - Fragment stage: base colour from `material` through a 16-entry palette constant array (sRGB-decoded linear values taken from `assets/materials.ron` average colours: the implementer dumps them with the ignored `dump_materials_ron` test and pastes 11 rows); climate tints grass and leaves toward yellow when `wet < 0.3` and toward blue-green when `wet > 0.7`; water: Fresnel (Schlick, F0 0.02) sun glint `pow(max(dot(reflect(-view, n), sun), 0), 400) * 4.0` and deep colour `(0.02, 0.07, 0.14)`; sun shading `max(dot(n, sun), 0)` plus sky ambient from `sky_probe` zenith term; atmosphere: transmittance LUT along the view ray from the eye to the surface point (two LUT taps, eye and surface altitude, standard Hillaire difference) and in-scatter from the multiscatter LUT scaled by path length, clamped at the limb; night side: `smoothstep(-0.05, 0.05, dot(n, sun))` blends to the lights term (Task 9) and a 0.02 blue-grey floor; lean tier (flag bit 0): skip the in-scatter LUT taps, use the probe's horizon colour times a distance fade instead.
  - Depth: reverse-Z, same projection as terrain; the globe writes depth so resident chunks and orbital assets sort against it.

- [ ] **Step 1: Write the failing GPU tests**

```rust
#[test]
fn gpu_globe_uniform_matches_wgsl_size() {
    // Compile the composed shader with naga through the shared test adapter and read back the
    // `GlobeUniform` struct size from the module's type arena.
    assert_eq!(wgsl_struct_size("GlobeUniform"), 64);
}

#[test]
fn gpu_two_adjacent_faces_leave_no_crack_on_their_shared_edge() {
    // Render roots of two edge-adjacent faces at level 0 with flat height 0 into a 256x256
    // target from an eye 60_000 m out on the bisector; count background-coloured pixels inside
    // the disc: expected 0.
}

#[test]
fn gpu_globe_matches_cover_heights_at_a_probe_point() {
    // Readback the depth at the screen centre for the globe and for a point computed on the CPU
    // with atlas::to_body of the baked height; within 2 m at 50 km.
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `just test-gpu gpu_globe`
Expected: FAIL, module not found.

- [ ] **Step 3: Implement `gpu.rs` and `globe_shader.wgsl`; list the shader; wire the draw**

The draw happens once per frame inside the existing terrain render pass, after `set_pipeline(world_pipeline)` draws and before the horizon block, under `if let Some(globe) = self.globe.as_ref().filter(|_| self.globe_visible)`.

- [ ] **Step 4: Run the tests and the shader validation**

Run: `cargo check -p minecraft_style_voxel_prototype` (build.rs validates the composed WGSL) then `just test-gpu gpu_globe`
Expected: `Validated 21 composed WGSL programs`; 3 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/render/globe/gpu.rs crates/app/src/render/globe/gpu_tests.rs crates/app/src/shaders/globe_shader.wgsl crates/app/src/shader_sources.rs crates/app/src/render.rs
git commit -m "globe: GPU tiles, fold shader, materials, ocean and atmosphere"
```

---

### Task 8: Frame driver (selection, upload, eviction, budget)

**Files:**
- Modify: `crates/app/src/render.rs` (`fn update_globe(&mut self, world, eye_m: DVec3, tier: Tier, focal_px: f32)` called from the same place `horizon.update` is called for flat worlds)
- Modify: `crates/app/src/main.rs` (pass `tier` into the renderer; add `[benchmark] globe resident= drawn= wanted= bake_ms= upload_kb=` to the summary and `globe_ms,globe_upload_bytes` columns to `frames.csv`)
- Test: inline in `scheduler.rs` for the eviction policy; the per-frame CPU bound is a frame-bench gate (Task 12)

**Interfaces:**
- Consumes: Tasks 6 and 7.
- Produces: `Gpu::globe_stats(&self) -> GlobeStats { resident, drawn, wanted, upload_bytes, cpu_ms }`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn eviction_keeps_the_drawn_set_and_their_parents_first() {
    // 2_100 resident ids, keep = 300 drawn + their parents; evict_unused(keep, 2048) drops only
    // ids outside keep, farthest from the eye first, down to 2048.
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p minecraft_style_voxel_prototype globe::scheduler::eviction`
Expected: FAIL.

- [ ] **Step 3: Implement `update_globe`**

Order per frame: `worker.drain(2 MB)` then `globe.upload`; `select` with `ready = resident set`; `worker.request(&selection.wanted[..64])`; `globe.evict_unused(&draw ∪ parents, 2048)`; `globe.prepare`. Skip everything when `tier == Surface` (and drop the worker after 30 s on the surface so it does not hold the cache open). Time the whole function with `Instant::now()` on the render thread only, into `GlobeStats::cpu_ms`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p minecraft_style_voxel_prototype globe::`
Expected: all pass.

- [ ] **Step 5: Measure the driver in the flight route**

Run the Task 0 route with the new exe; `python -I - <<EOF` over `frames.csv`: median and p99 of `globe_ms` after `03-space`.
Expected: median under 0.3 ms, p99 under 0.5 ms, `globe_upload_bytes` never over 2,097,152.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/render.rs crates/app/src/main.rs crates/app/src/render/globe/scheduler.rs
git commit -m "globe: per-frame driver with bake drain, selection, eviction and stats"
```

---

### Task 9: Cloud shell, cloud shadow and night lights

**Files:**
- Create: `crates/app/src/render/globe/shell.rs` (cloud coverage bake 512x512 per face from the weather field, uploaded as one `R8Unorm` 2D array texture of 6 layers; night-light splat list)
- Modify: `crates/app/src/shaders/globe_shader.wgsl` (sample the shell for shadow; second entry point `fs_shell` for the translucent shell sphere at `RADIUS_M + 1_600.0`)
- Modify: `crates/app/src/render.rs` (draw the shell after the opaque globe, blend over, depth test on, no depth write; switch the volumetric cloud march off when the camera altitude is above the top slab plus 500 m)
- Test: inline in `shell.rs`

**Interfaces:**
- Consumes: the weather field sampler used by `cloud_shader.wgsl` (`weather_coverage(x, z, time)` on the CPU side in `render/weather*.rs`), the orbital site list (`world::orbital::visual_contacts` sites) and `world.factory.nodes` positions for lights.
- Produces: `pub fn bake_shell(cfg, weather: &CloudWeatherField, time_s: f32, face: u8) -> Vec<u8>` (512x512, coverage 0..255; the shell drifts with the same wind vector as the march so the cloud shadow over resident chunks agrees in direction within 5 m per minute); `pub fn night_lights(world: &World) -> Vec<Light { body: [f32; 3], radius_m: f32, intensity: f32 }>` (one light per anchorage at 3.0 intensity, one per 64 factory nodes clustered by chunk at 0.6; capped at 256, strongest first).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn shell_coverage_matches_the_weather_field_at_face_centres() { /* bake face 0 at t=0; sample the CPU weather field at the chart centre; assert within 8/255 */ }

#[test]
fn night_lights_cap_at_256_strongest_first() { /* build a World with 300 anchorage-class sites; assert len 256 and non-increasing intensity */ }

#[test]
fn cloud_march_is_off_above_the_top_slab() { /* pure function `volumetric_clouds_enabled(altitude_m, top_slab_m) -> bool`: 5_000 above 4_000+500 is false; 4_300 is true */ }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p minecraft_style_voxel_prototype globe::shell::`
Expected: FAIL.

- [ ] **Step 3: Implement `shell.rs` and the shader additions**

Shell fragment: coverage `c = texture(shell)`, alpha `smoothstep(0.45, 0.75, c)`, colour sunlit `(1.0, 0.98, 0.95) * max(dot(n, sun), 0.0) + probe zenith * 0.4`, night `0.02`; cloud shadow on the ground is `1.0 - 0.6 * alpha` sampled at the ground point projected along the sun direction to the shell radius. Rebake the shell on the worker every 30 s of game time per face that is in front of the limb (a bake is 262,144 noise taps, under 10 ms).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p minecraft_style_voxel_prototype globe::shell::` and `cargo check`
Expected: 3 passed; WGSL validated.

- [ ] **Step 5: Capture before and after at `03-space` and `04-face-crossing`**

Compare with Task 0's captures; inspect that clouds cast shadow and the night side shows lights.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/render/globe/shell.rs crates/app/src/shaders/globe_shader.wgsl crates/app/src/render.rs
git commit -m "globe: cloud shell with ground shadow, night lights, march handoff"
```

---

### Task 10: Transition band: globe under resident chunks and horizon

**Files:**
- Modify: `crates/app/src/shaders/globe_shader.wgsl` (discard inside the resident coverage mask; fade band)
- Modify: `crates/app/src/render.rs` (bind `resident_coverage` to the globe; on `Planet` run the horizon rings in Transition when below 6 km and clip them to the face; the cover from `render/planet.rs` is not drawn once `globe.resident() > 0`)
- Modify: `crates/app/src/render/horizon.rs` (`pub fn set_chart_clip(&mut self, clip: Option<(f64, f64, f64, f64)>)`: sampling outside the rect returns non-finite heights, which `fine.rs` already skips)
- Test: `gpu_tests.rs` addition; `horizon` unit test for the clip

**Interfaces:**
- Consumes: `ResidentCoverage` (`render/coverage.rs`), `shadow_resident_owned` WGSL helper already used by the parent pages.
- Produces: `horizon::set_chart_clip`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn gpu_globe_discards_inside_the_resident_mask_and_draws_outside() { /* mask owns column (0,0): readback at the column centre is the clear colour; 3 columns away it is globe colour */ }

#[test]
fn horizon_samples_outside_the_chart_clip_are_non_finite() {
    let mut h = Horizon::test_new();
    h.set_chart_clip(Some((-16_000.0, -16_000.0, 16_000.0, 16_000.0)));
    assert!(!h.sample_height_m(16_001.0, 0.0).is_finite());
    assert!(h.sample_height_m(15_999.0, 0.0).is_finite());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `just test-gpu gpu_globe_discards` and `cargo test -p minecraft_style_voxel_prototype horizon::`
Expected: FAIL.

- [ ] **Step 3: Implement**

In the Planet branch of the horizon setup in `render.rs`, replace the early `return` with: keep the cover logic; when `tier == Transition && altitude < LOAD_STOP_M` call `self.horizon.set_chart_clip(Some(face rect in chart metres))` and `self.horizon.update(..)`; otherwise `self.horizon.clear_mesh()`. The globe's fade band: inside the disc the globe discards; from the disc edge to edge + 256 m the globe's alpha ramps 0 to 1 (blend on for that pass; the globe draws after resident chunks).

- [ ] **Step 4: Run tests to verify they pass**

Run: both commands above.
Expected: pass.

- [ ] **Step 5: Capture `02-ascent` and `05-descent` before and after**

Inspect the band at the resident edge for cracks and double-drawn ground.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/shaders/globe_shader.wgsl crates/app/src/render.rs crates/app/src/render/horizon.rs crates/app/src/render/globe/gpu_tests.rs
git commit -m "globe: resident mask discard, fade band, horizon rings on the planet face"
```

---

### Task 11: Lean tier

**Files:**
- Modify: `crates/app/src/render/globe/gpu.rs` (set flag bit 0 from `render::lean_shading()`; `max_draw` 256 on lean; shell texture 256x256 on lean)
- Test: inline (flag plumbing)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn lean_tier_halves_draw_count_and_shell_size() { assert_eq!(lean_limits(true), (256, 256)); assert_eq!(lean_limits(false), (512, 512)); }
```

- [ ] **Step 2: Run, fail, implement, pass**

Run: `cargo test -p minecraft_style_voxel_prototype globe::gpu::lean`

- [ ] **Step 3: Measure on the iGPU**

Run: `VOXEL_GPU_POWER=low <exe> --benchmark-world=planet --benchmark-seed=74 --benchmark-width=1280 --benchmark-height=720 --benchmark-motion=static --benchmark-warmup=10 --benchmark-duration=15 --benchmark-terrain-camera=0,60000,0,0,-1.5`
Expected: `p95_frame_ms` under 33 and `gpu_avg_ms passes` under 28. Record both numbers in the commit message.

- [ ] **Step 4: Commit**

```bash
git add crates/app/src/render/globe/gpu.rs
git commit -m "globe: lean tier limits; UHD 770 orbit p95 <n> ms"
```

---

### Task 12: Benchmark scenes and gate

**Files:**
- Modify: `scripts/frame_bench.py` (`SCENES["orbit-static"]`, `SCENES["ascent"]`)
- Modify: `crates/app/src/main.rs` (`--benchmark-motion=ascent`: fixed route from 0 to 40 km at 256 m/s then hold, 120 s)
- Modify: `crates/app/benches/frame_baselines.json` (after `just frame-bench-save orbit-static ascent`)
- Modify: `docs/TESTING.md` (two lines: the scenes and their limits)

**Interfaces:**
- Produces: scenes `orbit-static` = `--benchmark-world=planet --benchmark-motion=static --benchmark-terrain-camera=0,60000,0,0,-1.5` and `ascent` = `--benchmark-world=planet --benchmark-motion=ascent --benchmark-duration=120`.

- [ ] **Step 1: Add the scenes and the ascent motion**

- [ ] **Step 2: Run both scenes three times interleaved**

Run: `python -I scripts/frame_bench.py save orbit-static ascent --exe <exe>`
Expected: orbit-static p95 under 8 ms; ascent p99 under 16.6 ms and `max_frame_ms` under 40; `[benchmark] globe` line shows `wanted` reaching 0 within 20 s of `03-space`. If a number misses, this task is not done: profile with `frames.csv` columns `globe_ms`, `globe_upload_bytes`, `gpu_pass_ms` and fix in the owning task before saving.

- [ ] **Step 3: Commit**

```bash
git add scripts/frame_bench.py crates/app/src/main.rs crates/app/benches/frame_baselines.json docs/TESTING.md
git commit -m "bench: orbit-static and ascent scenes with globe gates"
```

---

### Task 13: Remove the cover draw, document, verify

**Files:**
- Modify: `crates/app/src/render.rs` (the cover mesh is no longer built or drawn when the globe exists; keep `render/planet.rs` until the peer agrees to delete it)
- Create: `docs/adr/0039-orbital-globe.md` (decision: tile quadtree baked from worldgen, disk cache format `VXGT0001`, tier thresholds, budgets; no new dependency)
- Modify: `docs/ARCHITECTURE.md` (one paragraph under rendering: globe, tiers)
- Modify: `docs/PLAN.md` (status row)

- [ ] **Step 1: Run the full gate**

Run: `just verify`
Expected: check, clippy, tests, `frame-bench-check` and LOC all pass; `repo_layout` passes.

- [ ] **Step 2: Final captures**

Run the Task 0 route; copy to `target/reviews/orbital-globe-after/`; write the before/after pairs into the ADR's evidence section.

- [ ] **Step 3: Commit**

```bash
git add crates/app/src/render.rs docs/adr/0039-orbital-globe.md docs/ARCHITECTURE.md docs/PLAN.md
git commit -m "globe: replace the planet cover; ADR 0039"
```

---

## Follow-on plan (not in this plan): building in space

Written as the spec's section 3 ideation. When approved it becomes its own
plan with these slices, in order, each with a save-version bump decided
once at the start:

1. Orbital yard region: a finite voxel region parented to an anchorage
   (`world::regions`), rotating with the planet; placement requires adjacency;
   `pressure.rs` sealed volumes and an airlock block.
2. Carriage freight: Freight class moves stacks between a ground station
   inventory and the yard inventory on the existing travel time; automation
   networks see both ends as containers.
3. Orbit resources: solar panel by real sun exposure from `engine_space`,
   radiator needing a free face toward space, oxygen tank and pump, consumption
   inside hulls; chip modules for door logic and alarms.
4. Survey: the radio dish scans the moon and gas giant; survey data unlocks
   recipes.
5. Mass driver and the moon as a second surface.
