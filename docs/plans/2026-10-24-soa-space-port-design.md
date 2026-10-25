# Space track: Seed of Andromeda review and port design

Status (2026-10-24): decisions recorded in section 9 and decision 0025;
supporting context, not an input of record. No code has been written against it.

Requested by the human: "Analyze the SoA code and integrate the best
implementation concepts for voxel planets and space travel/high tech high
performance physics simulation. Reimplement it in Rust." Product bar given in
the same conversation: match modded Minecraft (space packs such as Galacticraft
and Advanced Rocketry) in content, playability and polish, with FTL, Rimworld
and Noita as design references.

Source reviewed: `RegrowthStudios/SoACode-Public` (shallow clone, 2026-10-24),
444 files in `SoA/`, plus the `Vorb` engine headers and the `game/` data
(`StarSystems/Trinity`). License: MIT, copyright 2018 Regrowth Studios, for both
code and data. Translation is permitted; any file that translates SoA code keeps
the notice (section 8.4).

## 1. Intent

The game already has a strong surface: cellular fluids, factories, magic,
dungeons. The roadmap already commits to planets and vessels
([0024](../decisions/0024-cube-sphere-planets.md), W08, W09, P10, B09, N12,
claims C-F1, C-F2, C-S5) but schedules them at M6 to M7 with no design yet.
SoA is the one open-source voxel game that shipped a working voxel planet inside
a real star system. Its plumbing answers questions this roadmap has open:
how face coordinates map to a sphere, how terrain LOD scales from orbit to
voxels, how a body hierarchy evaluates orbits, and how an entity moves between a
space frame and a voxel frame.

The point of this document is to take those answers, fix what SoA got wrong,
and fit them to this engine's rules (fixed tick, budgets, positional hashing,
save and generator versioning), so the space track has a concrete design when
its owners start.

### 1.1 What the finished track looks like to a player

1. Stand on a planet whose horizon curves; walk far enough and cross a cube
   edge without a loading pause or a teleport.
2. Build a vessel from blocks, fuel it from the factory, launch, and watch the
   surface turn into a sphere below.
3. Open a system map with real orbits, pick a moon, wait for a transfer window,
   burn, coast on rails, land.
4. On the moon: different gravity, atmosphere (or none), resources and site
   generation; build a base; ship cargo home.
5. Inside the vessel: power, life support, engines, hull and shields are
   separate systems that fail separately; crew seal breaches and repair them
   (FTL, C-F1). Air leaks through a hole in the hull and vents into vacuum
   through the gas CA (Noita, C-N5).
6. Several bases on several bodies keep producing while the player is
   elsewhere, through a bounded off-screen ledger (Rimworld-style colonies).

### 1.2 Goals

- G1. Cube-sphere planets with flat, axis-aligned play inside each face, as
  0024 decides, and true face-edge crossing for player, fluids and bodies.
- G2. One height function per planet, sampled identically by voxel chunks, the
  horizon band and the orbit-view sphere, so LOD transitions never disagree.
- G3. Camera-relative precision from 1 cm (voxel at 0.25 m scale) to tens of AU
  with no visible jitter.
- G4. A deterministic star system: bodies on Keplerian rails evaluated as pure
  functions of the integer tick, so positions never drift and saves store no
  orbital state for natural bodies.
- G5. Vessels built from blocks, with a staged handoff: landed, atmospheric
  rigid body, orbital rails with patched conics, docked.
- G6. FTL-style vessel subsystems that reuse the factory networks, gas CA and
  crew AI instead of a parallel ship simulation.
- G7. Multi-body play with bounded off-screen simulation.
- G8. Space rendering at the art bar: atmosphere from ground and orbit, stars
  coloured by temperature, gas giants, rings, orbit paths and a system map.

### 1.3 Non-goals

- N1. Porting SoA as a whole. Its cellular automata, lighting, mesher, physics,
  UI and OpenGL renderers are below what this engine already has.
- N2. N-body simulation of natural bodies. Rails are deliberate.
- N3. Realistic solar-system scale. Distances and periods are game-scaled per
  system file (section 4.3).
- N4. Multiplayer. Nothing here may block it, but nothing here builds it.
- N5. Interstellar travel between star systems in this track. One system first;
  the data format allows more.

### 1.4 Anti-goals (failure even if it technically works)

- A1. Any jitter, popping seam or visible crack at a LOD or face boundary at
  normal play distances.
- A2. A loading screen, fade to black or teleport when crossing a face edge.
  SoA does exactly this (section 2.3); it is the thing to beat.
- A3. Old worlds changing. Every planet behaviour sits behind a new generator
  version; flat worlds keep generating byte-identically.
- A4. Frame-rate-dependent physics or orbits (SoA divides by a hardcoded 60).
- A5. A second implementation of power, gas, storage or crew logic just for
  vessels (C-MC4 shared-mechanisms audit).
- A6. Orbital mechanics the player cannot read. Burns, windows and fuel cost
  are shown with numbers and units (UI_RULES).

## 2. SoA review

### 2.1 Architecture

Two ECS worlds. `SpaceSystem` holds celestial bodies as components:
`NamePosition` (f64 km), `Orbit`, `AxisRotation` (f64 quaternions),
`SphericalGravity`, `SphericalTerrain` (orbit-view patches), `FarTerrain`
(flat patches on one face), `SphericalVoxel` (six `ChunkGrid`s, one per face),
`Atmosphere`, `Clouds`, `Star`, `GasGiant`, `PlanetRings`, `SpaceLight`.
`GameSystem` holds the player and entities: `SpacePosition` (f64 km,
parent-relative, with parent gravity and parent terrain ids), `VoxelPosition`
(face plus f64 grid position), `Physics`, `AabbCollidable`, `Head`, `Frustum`,
`ChunkSphere` (load radius), `Parkour`/`FreeMove` input. Update order in
`SpaceSystemUpdater::update`: axis rotation, far terrain, spherical terrain,
gas giants, voxels, orbits.

### 2.2 Coordinates (`VoxelCoordinateSpaces.h`, `VoxelSpaceConversions.cpp`)

- Six faces `TOP, LEFT, RIGHT, FRONT, BACK, BOTTOM`. A face position is
  `(face, x, y, z)` with `y` the height above the face plane in voxels and x/z
  in `[-radius, radius]` voxels.
- Face to sphere: per-face axis permutation `VOXEL_TO_WORLD[6]`, sign table
  `FACE_TO_WORLD_MULTS[6]` and `FACE_Y_MULTS[6]`; build the cube point
  `(x*mx, radius*my, z*mz)` permuted, then `normalize` (plain gnomonic map)
  and scale by `radius + y`.
- Sphere to face: ray-box hit on the radius cube with a 32-voxel epsilon per
  face, in f32 (`computeGridPosition`). Imprecise near edges and corners.
- Local frame: `calculateVoxelToSpaceQuat` builds tangent, normal and
  bitangent from two nearby projected points. Works, but is a finite difference
  where an analytic Jacobian is available.

### 2.3 Face-edge crossing (the weak point)

- `VoxelSpaceUtils::offsetChunkGridPosition` has its neighbour and rotation
  tables commented out (`TODO(Ben): New transition logic`).
- The player crossing (`PhysicsComponentUpdater::transitionPosX` and siblings)
  is a hand-written switch per face: it pushes the player 32 voxels inside the
  new face (`VOXEL_PUSH`) and rotates orientation only for TOP/BOTTOM cases.
- Terrain then deletes the whole `SphericalVoxel` component and rebuilds it on
  the new face behind a timed fade (`SphericalTerrainComponentUpdater::
  updateVoxelComponentLogic`, "RE-INIT Voxels"). Fluids and bodies never cross.
- Lesson: derive neighbour and rotation tables from face basis vectors, test
  them exhaustively, and keep chunks of adjacent faces resident at the same time.

### 2.4 Terrain LOD (`TerrainPatch*.cpp`, `FarTerrain*.cpp`)

- Per face, a grid of top-level patches (`ST_PATCH_ROW`), each a quadtree.
  Patch mesh 33 x 33 vertices plus 4 skirts, heightmap and normal map padded.
- Split when camera distance `< width * DIST_MIN`; merge when
  `> width * DIST_MAX` (1.0 vs 1.1: hysteresis). Keep the parent mesh until all
  four children are renderable, so no holes appear during refinement.
- Distance to the patch AABB, not its centre; horizon culling by angle
  (`isOverHorizon`). Quality setting moves `DIST_MIN` and max LOD.
- Meshes built on a thread pool (`TerrainPatchMeshTask`), uploaded on the GL
  thread.
- Two modes. Spherical patches in orbit view; within `radius * 1.02` the game
  spawns flat `FarTerrain` patches on the current face plus voxel chunks and
  crossfades alpha between them (`TERRAIN_ALPHA_STEP`, fade constants). Exit at
  `radius * 1.0205`: a hysteresis band between modes.
- No geomorphing: skirts hide cracks, LOD changes pop.

### 2.5 Generation (`SphericalHeightmapGenerator.cpp`, `PlanetGenData.h`, `Noise.h`)

- One function `generateHeightData(normal * radius, normal)` serves patches,
  far terrain and voxel columns. This is the single best idea in SoA.
- Height, temperature and humidity are data-driven noise trees
  (`TerrainFuncProperties`: stage, op add/mul/sub, octaves, persistence,
  frequency, low/high clamps, children) loaded from `terrain_gen.yml`.
- Temperature and humidity take latitude and height falloff
  (`tempLatitudeFalloff`, `tempHeightFalloff`, `hum*`).
- Biomes come from a 2D temperature x humidity lookup with bilinear influence
  weights (`getBaseBiomes`), then child biomes by noise.
- Flora and trees roll per column with `std::mt19937` seeded from a hash of the
  position. The code comments "ITS SLOW". Positional hashing replaces it here.
- Staged chunk generation `GEN_TERRAIN -> GEN_FLORA -> GEN_SCRIPT -> GEN_DONE`
  through `ChunkQuery` objects on a lock-free queue, ref-counted
  `ChunkHandle`s, and a per-column `ChunkGridData` that caches the heightmap for
  every chunk in that column.

### 2.6 Space system (`OrbitComponentUpdater.cpp`, `SpaceSystemLoader.cpp`)

- Elements per body: period `t`, eccentricity `e`, inclination `i`, longitude
  of ascending node `n`, longitude of periapsis `p`, start mean anomaly `a`.
  Semi-major axis derived from period and masses by Kepler's third law.
- Barycenters: a body of type `Barycenter` with two `comps`; masses summed,
  each component's semi-major axis scaled by the other's mass ratio, B at
  periapsis + 180 degrees. Nested barycenters recurse (Trinity is a triple star).
- Position is a pure function of time: mean anomaly, Newton solve for
  eccentric anomaly, true anomaly, radius, then rotate by node, periapsis and
  inclination; add parent position.
- Orbit paths are precomputed ellipse meshes (2880 vertices).
- Tidal lock decided by a heuristic formula at load.
- Axial tilt (`aTilt`), north longitude (`lNorth`) and rotation period drive an
  `AxisRotation` quaternion; the voxel world rotates with it.
- Defects found: mean motion uses `M_2_PI`, which `<math.h>` defines as
  `2/pi`, not `2*pi` (orbits run about pi^2 too slow); Kepler solve is a fixed 3
  Newton steps from `E = M`, which converges badly for high `e`; velocity
  formula ignores the node rotation; parents and children update in hash-map
  order, so a moon can read its planet's previous-frame position.

### 2.7 Space to surface handoff (`PhysicsComponentUpdater.cpp`)

- An entity always has a `SpacePosition`; it gains a `VoxelPosition` when its
  planet's voxel component exists. Entry converts space position to face
  coordinates through the inverse axis rotation, then sets voxel orientation
  from the inverse local frame.
- In voxel mode, the space position is recomputed each tick from the face
  position (the face position is authoritative near the ground).
- Gravity: inverse-square from the parent mass, but scaled by a hard `0.1`
  hack on the surface, and both modes divide by a constant `FPS = 60` with no
  timestep.

### 2.8 Storage and rendering details

- `SmartVoxelContainer<T>`: each chunk array is either a flat array or an
  interval tree (run-length), switching to the tree after 60 quiet frames,
  with a cap on compressions per frame.
- `PagedChunkAllocator`: chunks allocated in pages and recycled.
- Atmosphere: O'Neil scattering (GPU Gems 2) with `kr`, `km`, `esun`, `g`,
  scale depth, wavelength^-4. Stars: glow sprite sized by distance, colour
  from a temperature colour map, occlusion query for visibility, lens flare.
  Gas giants: 1D colour map plus oblateness. Rings: textured annuli with
  orientation. System map (`SystemARRenderer`, `MainMenuSystemViewer`): orbit
  paths, hover fade with a Hermite curve, selector sprites, name labels,
  click to focus.

### 2.9 Verdict by area

| Area | SoA quality | Verdict |
|---|---|---|
| One height function for all LODs (2.5) | Good | Adopt |
| Cube-face coordinates and projection (2.2) | Correct, gnomonic, f32 inverse | Adopt idea, rewrite with equal-area map and f64/integer inverse |
| Face-edge crossing (2.3) | Unfinished | Reject; design anew from 0024 |
| Patch quadtree with hysteresis and parent-until-children (2.4) | Good | Adopt, add geomorphing |
| Two render modes with crossfade and hysteresis band (2.4) | Good idea, fade crude | Adopt, blend by height over a band |
| Data-driven noise trees, latitude climate, biome lookup (2.5) | Good | Adopt shape; feed W-GEO instead of replacing it |
| Staged generation, column cache, ref-counted handles (2.5) | Good | Adopt into C03/C16 designs |
| Kepler rails, barycenters, parent-relative positions (2.6) | Good idea, buggy math | Adopt with fixes |
| Space/voxel dual position with handoff hysteresis (2.7) | Good idea | Adopt |
| Entity physics (2.7) | Frame-dependent, hacks | Reject |
| RLE interval-tree chunks with quiet compression (2.8) | Reasonable | Candidate for C03 measurement only |
| CA, light engine, chunk mesher | Below current engine | Reject |
| Atmosphere, star, gas giant, ring, system map rendering (2.8) | Dated GL, sound ideas | Adopt ideas in WGSL; atmosphere extends the existing sky |
| Procedural trees (`FloraGenerator`, 1342 lines) | Detailed | Defer; compare with current trees later |

## 3. Constraints from this engine

- Generator 51 and save 34 at this writing (`crates/voxel/src/config.rs`,
  `crates/app/src/world/storage.rs`). Reread both before any migration.
  Planets arrive with a new generator version; never edit a historical gate.
- Fixed tick (`simulation::STEP_SECONDS`), `Budget` on every bounded pass, no
  `Instant` in simulation, positional hashing only.
- Coordinates today: voxel `IVec3`, chunk 32^3 with floor division, `Player::pos`
  in voxel units with `precise_pos: DVec3` authoritative. Chunk vertices are
  absolute f32 and the shader subtracts the camera, so precision already
  degrades with distance (`render.rs` `upload_camera`, `world_shader.wgsl`).
- Existing sky layer: f64 circular orbits for megastructures and skyhooks
  (`crates/app/src/world/orbital/`), sun and moon on a fixed tilted circle
  (`world.rs`), planet radius constants that disagree between Rust
  (`6_371_000/4`) and WGSL (`6371000.0`). Sky positions are pure functions of
  seed and clock, but the orbital gameplay state is saved (`world.orbital` in
  `storage.rs`, including the v24 alignment booking).
- Master plan owners: C02 (face id, cross-edge rotation), C03 (brick storage),
  C07 (gas, oxygen), W01 (worldgen stages, equal-area mapping choice), W08
  (per-world constants, planet size), W09 (space layer, S9 spike first),
  W10 (ring), R10 (far LOD, curvature), B01 to B09 (bodies and controllers),
  P10 (vessels), N12 (crew), P13 (expeditions), M23 (pocket worlds).
- Writing, file-size, dependency and visual-capture rules in `CLAUDE.md`.

## 4. Design

### 4.1 Crates and ownership

| Piece | Home | Why there |
|---|---|---|
| Face id, face coordinates, projection, edge tables | `engine_core::cube_sphere` | C02 owns coordinates; `sim` depends only on `core` and needs edge rotation for cross-face CA neighbours |
| Star system data, rails, frames, rotation, gravity, SOI, conics | new crate `engine_space` (depends on `core`) | Pure functions of data and tick; testable headless; no app state |
| Planet height, climate, biome sampling on the sphere | `engine_worldgen::planet` | Worldgen stays pure functions of `WorldSampleConfig` |
| Planet parameters (radius, gravity, atmosphere) | `engine_voxel` config, owned by W08 | Per-world constants |
| Handoff, vessel state machine, off-screen ledger | `app/src/world/space/` | Needs world, bodies, factory |
| Sphere patches, horizon curvature, atmosphere, stars, rings, map | `app/src/render/space/` and `render/horizon/` | Renderer |
| System data | `assets/star_systems.ron` | No content tables in Rust |

No new external dependency is needed; `glam` f64 types already exist in the
workspace.

### 4.2 Precision model (prerequisite for everything else)

Three frames, each with one authority:

1. **Face frame** (surface play). Integer chunk `IVec3` on a face, f32 offset
   inside the chunk. Authoritative for anything on or near a planet.
2. **Body frame** (orbit). f64 metres relative to the body's centre in the
   body's rotating frame (for landing) or inertial frame (for orbit).
3. **System frame** (map and rails). f64 metres relative to the system root,
   derived, never stored for natural bodies.

Rendering is camera-relative in every frame: each chunk mesh carries its own
origin as an integer chunk coordinate; the CPU computes
`(chunk_origin_f64 - camera_f64)` per draw and passes it as an f32 offset in a
per-draw uniform or instance buffer. Vertex positions stay small f32 offsets
inside the chunk. Space bodies follow the same rule with their f64 positions.
This replaces absolute f32 chunk vertices and is the first piece to build
(phase 0), because it also fixes the existing jitter far from spawn on flat
worlds.

Depth: reversed-Z with an infinite far plane for the space pass; planets and
stars at system scale draw in a separate far pass with their own near/far,
composited before the world pass (SoA's split of space and voxel render stages,
done with one depth convention).

### 4.3 Cube-sphere coordinates

Data (in `engine_core::cube_sphere`):

```rust
#[repr(u8)]
pub enum Face { PosX, NegX, PosY, NegY, PosZ, NegZ }

/// Position on a planet: face, integer cell, height above datum.
pub struct FaceCell { pub face: Face, pub cell: IVec3 } // cell.y is height

pub struct FaceBasis { pub normal: IVec3, pub u: IVec3, pub v: IVec3 }
```

- Each face has an integer basis (normal, u, v), right-handed, with "down"
  equal to `-normal` (0024). Neighbour and rotation tables are *derived* from
  the bases at compile time (`const fn`), never hand-written: crossing the
  `+u` edge of face F lands on the face whose normal is F's `u`, with a
  rotation that maps F's (u, v) to the neighbour's. A test checks all 24
  edge directions: crossing and crossing back is identity; every corner is
  shared by exactly 3 faces; the 4 rotations around a face edge compose to
  identity.
- Face half-size `H` in cells comes from W08 (default face edge 32 km, so at
  1 m voxels `H = 16_000`). Cells satisfy `-H <= x,z < H`.
- Projection from `(face, s, t)` with `s, t` in `[-1, 1]` to a unit direction:
  equal-area or near-equal-area mapping chosen in W01's SDD (0024). Candidates,
  to be measured for area ratio and cost: the "spherified cube" analytic map,
  the tangent-adjusted map used by Google S2, and the COBE quadrilateralized
  cube. The plain gnomonic map (SoA) has an area ratio near 1.6 at corners.
  The inverse runs in f64 with an exact face pick by largest absolute
  component, replacing SoA's f32 ray-box hit with an epsilon.
- Local frame for rendering and the space handoff: the analytic Jacobian of
  the projection (tangent u, tangent v, normal), not a finite difference.

Edge geometry (open decision 9.1; owned by C02, built in phase 2). Two
coherent models exist; the design must pick one, because collision, meshing,
CA neighbours and the render transform all differ:

- **Model R, radial columns (recommended).** Topology is the unfolded cube:
  each face is a 2D grid of columns, `y` is height along the column, and
  column `(A, x = H-1)` neighbours column `(B, x' = -H)` at the same `y`
  through the derived rotation. Geometry is bent: every vertex is placed at
  `project(face, s, t) * (R + y)`, so columns point at the planet centre and
  faces meet with no crease and no wedge. Simulation stays axis-aligned in
  grid space with down `= -y` everywhere, which is radial gravity. Near
  chunks are drawn with a per-draw offset plus rotation (the Jacobian frame at
  the chunk centre) plus a curvature term in the vertex shader; at a 20 km
  radius one 32-cell chunk bends by under 0.1 degrees, so blocks look square.
  Cost: grid angles skew near the 8 corners (cell corner angles approach 120
  and 60 degrees with the gnomonic map, less with equal-area maps). Corners
  have valence 3: the 2 x 2 x 2 Margolus block at a corner vertex has a
  missing cell, treated as solid bedrock. Worldgen places all 8 corners
  under ocean or polar ice so the skew is rarely walked on.
- **Model F, folded cube (0024 as literally written).** Each face is drawn
  flat in its own plane; the seam is a real 90 degree convex crease. Above
  ground there is a wedge-shaped gap above each edge and an overlap below it,
  so columns cannot carry straight across; collision and fluids need a
  special edge band. Gravity and camera up blend over `EDGE_BLEND` cells. The
  near-chunk render transform is offset only. Cheaper in the renderer, harder
  everywhere else.

Common to both:

- Chunk neighbour lookup goes through one function
  `neighbour(face, chunk, dir) -> (face, chunk, Rotation)`. Inside a face it is
  plain arithmetic; across an edge it applies the derived rotation. Meshing,
  CA, light and bodies all call it, so nothing special-cases edges.
- CA cells never change face, only neighbour addressing does, so the integer
  CA stays conservative.
- Chunks on both sides of an edge stay resident together (no SoA-style reload).

Phase 0's per-draw transform is built as offset plus rotation from the start,
so either model fits without reworking it (rotation is identity on flat worlds).

### 4.4 Planet terrain from one function

- `engine_worldgen::planet::sample(config, dir: DVec3) -> ColumnSample` gives
  height, temperature, humidity, biome weights and flora id for a unit
  direction. Voxel generation evaluates it at each column's projected centre;
  the horizon band and orbit patches evaluate the same function at coarser
  spacing. All W-GEO stages (plates, climate, drainage) read latitude and
  longitude from the direction (0024 says climate reads lat/long from the
  projection).
- The SoA noise-tree shape (stage, op, octaves, frequency, persistence,
  low/high) becomes a RON schema for per-body terrain recipes in
  `assets/planets/*.ron`, but W-GEO's stage chain stays the authority for the
  home planet. Moons and simple bodies may use the recipe alone.
- Determinism: f64 noise is not guaranteed bit-identical across CPUs and
  compilers. The column sample feeds integer heights through a fixed rounding
  step, and generator golden tests pin the results per generator version, as
  today. Fixed-point noise is left to W01's decision on 0023's open question.
- Flora rolls use `engine_core::hash` on (seed, face, cell), never a seeded RNG.
- Staged generation per chunk: terrain, then features needing neighbours
  (trees, sites), then structures, each stage a `Budget`-bounded pass with a
  per-column cache of the sampled heights shared by all chunks in that column.
  This matches SoA's `ChunkGenLevel` plus `ChunkGridData` and goes into the C03
  and W01 SDDs as a requirement, not a separate system.

### 4.5 LOD from orbit to voxels

Three representations, one height source:

| Distance from surface | Representation | Owner |
|---|---|---|
| 0 to about 1.5 km | Voxel chunks on the face(s) in view | existing mesher |
| 1.5 km to horizon | Horizon band, bent onto the sphere by the projection | R10 (`render/horizon.rs`) |
| Orbit (above about 2% of radius) | Sphere patch quadtree per face | new `render/space/planet_patches` |

- Horizon band: the existing ring levels keep their layout but place vertices
  by the planet projection instead of the flat plane, sampling `planet::sample`.
  On flat worlds nothing changes.
- Sphere patches: SoA's quadtree (33 x 33 vertices, skirts, split at
  `d < w * k`, merge at `d > w * k * 1.1`, parent kept until children ready,
  AABB distance, horizon cull), plus geomorphing: each vertex stores its
  parent-level height and blends by distance in the shader, so splits do not
  pop. Patch builds are `Budget`-bounded jobs; uploads go through the existing
  upload path.
- Mode blend: SoA's alpha fade on a timer becomes a blend by camera altitude
  over a band (proposal: from 1.0% to 2.0% of radius above datum), with
  hysteresis on residency (load voxels below 1.5%, unload above 2.05%). The
  horizon band and sphere patches overlap inside the band, drawn with a
  depth-tested dither blend so no frame shows neither.

### 4.6 Star system and rails (`engine_space`)

Data (`assets/star_systems.ron`, schema sketch):

```ron
(
  systems: [(
    id: "home",
    time_scale: 1.0,
    bodies: [
      (id: "sun", kind: Star(temperature_k: 5600), mass_kg: 1.2e29, radius_m: 2.0e7,
       parent: None, rotation: (period_s: 2.0e5, tilt_deg: 3.0, north_deg: 0.0)),
      (id: "home_planet", kind: Planet(world: Home), mass_kg: 4.0e22, radius_m: 20_371.0,
       parent: Some("sun"), orbit: (period_s: 3.6e5, e: 0.016, i_deg: 0.0,
       node_deg: 0.0, peri_deg: 102.0, mean_anomaly_deg: 0.0),
       rotation: (period_s: 1440.0, tilt_deg: 23.4, north_deg: 0.0)),
    ],
  )],
)
```

Game scale: the home planet radius equals the face geometry
(`radius = 2 * H * cell / (projection's face-to-sphere scale)`), the day
matches today's `DayNightCycle` length, and the year is a chosen number of days.
Masses are then set so surface gravity matches W08's gravity constant. Realism
is a per-system knob, not a goal.

Rails:

- Body position at tick `n`: `t = n * STEP_SECONDS * time_scale` as f64 from
  the integer tick (no accumulation, so no drift). Mean anomaly
  `M = M0 + 2*pi*t/T` reduced modulo `2*pi` using the integer tick modulo the
  period in ticks first, so precision does not decay over long saves.
- Kepler solve: starting guess `E0 = M + e*sin(M)` (or `pi` for `e > 0.8`),
  Newton-Raphson to `|dE| < 1e-12` with an iteration cap of 16 and a bisection
  fallback; tests cover `e` in `[0, 0.97]` against a reference table. Fixes
  SoA's fixed 3 steps.
- Position and velocity from the standard perifocal-to-inertial rotation
  (node, inclination, argument of periapsis), both computed from the same
  rotation matrix. Fixes SoA's velocity formula.
- Evaluation order: bodies stored in topological order (parents before
  children) at load; one pass computes the system frame. Fixes SoA's
  hash-map order lag.
- Barycenters: as SoA (sum masses, split semi-major axes by mass ratio, second
  component at periapsis + 180 degrees, recursive), validated at load.
- Axis rotation: tilt and north give a fixed axis; the angle is
  `2*pi * (n mod period_ticks) / period_ticks`. Tidal lock is a data flag
  (`period_s: Locked`), not a heuristic.
- On planet worlds, sun and moon directions come from the rails (body
  direction relative to the observer's face cell) instead of the fixed circle
  in `world.rs`; flat worlds keep the circle (8a.1). Seasons and day length
  then fall out of tilt and orbit. Planet worlds read radius from one source,
  which retires the Rust/WGSL radius mismatch there.

Determinism scope: rails are pure functions of data and tick, recomputed on
load, never saved. Cross-machine bit equality of f64 trig is not claimed; the
claim C-S5 asks for integer patched conics, which is S9's question (4.8).

### 4.7 Space to surface handoff

Every vessel and player has a `SpaceState`:

```rust
pub enum SpaceState {
    Surface { body: BodyId, cell: FaceCell, offset: Vec3 },   // face frame
    Flight  { body: BodyId, pos: DVec3, vel: DVec3 },          // body inertial frame, integrated
    Rails   { body: BodyId, conic: Conic, epoch_tick: u64 },   // coasting, analytic
    Docked  { host: VesselId, slot: u16 },
}
```

- Surface to Flight when altitude exceeds the voxel residency band (4.5) or
  the vessel leaves the simulated chunk set; Flight to Surface when it drops
  below the lower threshold. Thresholds differ (hysteresis), as SoA's 1.02 /
  1.0205.
- Flight to Rails when thrust is zero, the vessel is outside the atmosphere,
  and no contact is pending: the state vector becomes a conic. Rails to Flight
  on thrust, atmosphere entry or SOI change (patched conics: SOI radius
  `a * (m/M)^(2/5)`).
- Conversion between face frame and body frame uses the projection, its
  Jacobian frame and the body's rotation at that tick; a round-trip test bounds
  the error (target: under 1 mm at the surface, under 1 m at 100 km altitude).
- Gravity: in the face frame, magnitude from W08 and direction `-normal`,
  blended across edges (4.3); in Flight, inverse-square from the body, with
  `GM` from data. No frame-rate constants: every integrator step uses
  `STEP_SECONDS`.
- Time warp: allowed only when every active vessel is on Rails or Surface and
  no surface simulation is awake near the player; rails need no stepping, so
  warp is free.

### 4.8 Vessels

- A vessel is a rigid body (B01 to B05) built by flood fill from a control
  block, like today's `rigid.rs` bodies, with a block count cap per vessel class
  (C18 profiles). Inside, it keeps its own small voxel grid so the gas CA,
  thermal, power and fluids run inside it in its local frame.
- Controllers are B09's shared contract: thrust blocks produce force from fuel
  flow; mass and centre of mass update from blocks plus tank contents.
- Patched-conic propagation and transfer windows come from S9 (integer
  elements); this document fixes the interface (`Conic`, `epoch_tick`,
  `SOI` switch events) and leaves the number format to S9's ADR.
- Docking joins two vessels through B08 joints; stations are vessels with no
  thrust.
- Landing writes the vessel back into the face grid only when it comes to rest
  (B07 fuse), so landed ships become part of the base.

### 4.9 FTL-style vessel systems (C-F1) on shared mechanisms

| Ship system | Existing mechanism it reuses |
|---|---|
| Power | factory power network |
| Life support (O2, CO2, pressure) | C07 gas CA with mixtures inside the vessel grid |
| Hull and breaches | voxel damage plus gas venting to vacuum (vacuum = a boundary that deletes gas) |
| Engines, fuel | B09 controller plus fluid tanks on item/fluid networks |
| Shields, sensors, weapons | M22 fields, existing spells/machines where they fit |
| Crew | N12 utility AI with jobs seal, repair, evacuate, man station |

FTL's run structure (C-F2) maps onto the system map: each body and orbit has
generated sites and events (P13), and a "route" is a sequence of transfers with
known fuel and time costs and unknown site content until scanned.

### 4.10 Multi-body colonies

- Each body with a base is a world in W08's sense with its own region files
  (M23 nested parents). Only the body the player is on runs the full fixed
  tick.
- Off-screen bodies run a bounded ledger: machines, fields and storage report
  per-tick rates when the player leaves; on return the elapsed ticks apply
  those rates with the same caps (a deterministic catch-up, not a replay).
  This is the Rimworld-style "colonies keep working" layer; its exact rules
  belong to M23's mini-SDD.
- Logistics between bodies are vessel trips, automatable later through the
  shared controller and routes.

### 4.11 Space rendering (art bar)

- Atmosphere: extend the existing sky model so it works from orbit (view ray
  from outside the shell); SoA's O'Neil parameters are a reference, not a
  target. One radius source (4.6).
- Stars: blackbody colour from temperature (table in RON), glow sized by
  angular diameter and exposure, occlusion via the depth buffer rather than GL
  queries.
- Gas giants: banded colour lookup plus oblateness; rings: textured annuli
  with planet shadow.
- System map: orbit paths drawn from the analytic conic (no 2880-vertex
  meshes needed: a screen-space adaptive polyline), hover with a smooth
  ease, labels with numbers and units per UI_RULES, click to focus, transfer
  planner overlay later.
- Every visual change follows the A/B capture rule.

## 5. Port catalogue

Each row: SoA source, what is kept, what changes, owner and phase (section 6).

| # | SoA source | Keep | Change | Owner | Phase |
|---|---|---|---|---|---|
| 1 | `VoxelCoordinateSpaces.h` | face + position types | integer basis, derived tables | C02 | 1 |
| 2 | `VoxelSpaceConversions.cpp` | face-to-sphere idea | equal-area map, f64 exact inverse | C02/W01 | 1 |
| 3 | `VoxelSpaceUtils::calculateVoxelToSpaceQuat` | local frame | analytic Jacobian | C02 | 1 |
| 4 | `PhysicsComponentUpdater::transition*` | nothing | derived edge crossing, gravity blend | C02/B03 | 2 |
| 5 | `SphericalHeightmapGenerator` | one sampler for all LODs | W-GEO stages, hashing, integer rounding | W01 | 3 |
| 6 | `Noise.h` / `terrain_gen.yml` | noise-tree schema | RON, validator | W01 | 3 |
| 7 | `PlanetGenData` climate falloffs | latitude/height climate | W-GEO P2 owns it | W12 | 3 |
| 8 | biome lookup with influence weights | blend weights | W-GEO biomes | W12 | 3 |
| 9 | `ChunkGenLevel`, `ChunkGridData` | staged gen, column cache | `Budget` per stage | C03/W01 | 3 |
| 10 | `TerrainPatch` quadtree | split/merge rules, parent-until-ready | geomorphing, job budget | R10 | 4 |
| 11 | `FarTerrain` + crossfade | mode band with hysteresis | altitude blend, horizon bent onto sphere | R10 | 4 |
| 12 | `OrbitComponent` elements | element set, barycenters | solver, velocity, order, modulo time | W09 | 5 |
| 13 | `AxisRotationComponent` | tilt/north/period | lock as data | W09 | 5 |
| 14 | `SystemProperties.yml` | hierarchy format | RON, game scale | W09 | 5 |
| 15 | `SpacePosition`/`VoxelPosition` handoff | dual frame, hysteresis | `SpaceState` enum, exact conversions | W09/P10 | 6 |
| 16 | `SphericalGravity` | inverse-square | `GM`, fixed step, edge blend | W08/B03 | 6 |
| 17 | `SystemARRenderer`, `MainMenuSystemViewer` | map UX | wgpu/egui, UI_RULES | R/P | 5 |
| 18 | `StarComponentRenderer` | temperature colour, glow | depth occlusion | R | 7 |
| 19 | `AtmosphereComponent` | from-space scattering | extend current sky | R | 7 |
| 20 | `GasGiantComponent`, `PlanetRings` | colour map, oblateness, rings | WGSL | R | 7 |
| 21 | `SmartVoxelContainer` | quiet-frame RLE | only if C03 numbers favour it | C03 | spike |
| 22 | `FloraGenerator` | branch/leaf parameters | compare with current trees | W | later |

Rejected: `CAEngine`, `VoxelLightEngine`, `ChunkMesher`, `Collision`,
`FreeMove`/`Parkour` physics, `PagedChunkAllocator` (C03 owns pooling), GL
render stages, keg/YAML loaders.

## 6. Phases

Each phase is shippable on its own, keeps flat worlds unchanged, and ends with
named tests or captures. Sizes are rough session counts.

| Phase | Content | Exit evidence | Depends on | Size |
|---|---|---|---|---|
| 0 | Camera-relative chunk and model rendering (4.2) | capture at x = 1,000,000 m: no jitter; perf within `bench-check` | none | 2 |
| 1 | `engine_core::cube_sphere`: faces, bases, derived tables, projection candidates, inverse, Jacobian | exhaustive edge tests; area-ratio table for each candidate; round-trip error bounds | C02 SDD face section | 2 |
| 2 | Planet world type behind a new generator version: bounded faces, edge crossing for player, chunks, CA, bodies | walk a full great circle in a headless scenario; water poured on an edge conserves mass; old generator golden tests unchanged | 1, W08 constants | 4 |
| 3 | `planet::sample` on directions; staged gen with column cache | golden heights per generator version; same height from chunk, horizon and patch samplers | 2, W01 | 3 |
| 4 | Horizon bent onto sphere; sphere patch quadtree; altitude mode blend | fly-up capture from 2 m to 3 radii: no crack, no blank frame; patch build ms in budget | 3, R10 | 4 |
| 5 | `engine_space` rails, axis rotation, RON system, sky directions from rails, system map | rails tests (solver, barycenter, order, long-time precision); map screenshot; flat-world sky unchanged (golden directions); fixed-point sim sun matches f64 rails within one quantum | 1 | 3 |
| 6 | `SpaceState` handoff and gravity; free-flight camera to orbit and back | headless: launch, coast, re-enter, land at the same cell within tolerance | 4, 5, B03 | 3 |
| 7 | Atmosphere from orbit, stars, gas giants, rings | A/B captures | 4, 5 | 3 |
| 8 | S9 spike: integer patched conics, transfer windows | spike protocol CSV + ADR | 5 | 2 |
| 9 | Vessels: build, controllers, thrust, staging, docking, landing fuse | `vessel_hull_breach` precursor scenario; launch-to-orbit scenario | B01 to B09, 6, 8 | 6+ |
| 10 | Vessel systems and crew (C-F1), routes and sites (C-F2) | `vessel_hull_breach`, `seeded_expedition_sites` | 9, C07, N12, P13 | 6+ |
| 11 | Multi-body bases and off-screen ledger | leave and return after N days: outputs match the ledger rule | 9, M23 | 4 |

Phases 0, 1 and 5 have no dependency on the M1 slice and touch disjoint files;
they can run in parallel. Phases 2 to 4 depend on C02/W01/R10 SDDs.

## 7. Risks

- R1. Edge crossing in the CA. Cross-edge neighbours change the Margolus block
  shape at edges. Mitigation: phase 2 starts with a CA test on a 2-face strip
  before any rendering.
- R2. Precision at the handoff. Converting face frame to body frame at
  16 km half-size in f64 is well inside precision; the risk is f32 leaking into
  the path. Mitigation: round-trip tests with explicit bounds (4.7).
- R3. Generator cost. Sampling on directions costs a projection per column.
  Mitigation: column cache (4.4); bench under `just bench`.
- R4. Scope. Phases 9 to 11 are each larger than the whole of phases 0 to 5.
  Mitigation: the phase table is ordered so every stop point is a playable
  improvement.
- R5. Master-plan conflict (section 9).

## 8. Left to the builder

1. Exact projection, picked by measurement in phase 1 (criteria: max area
   ratio, max angular distortion, cost per sample, exact inverse).
2. `EDGE_BLEND` width and the gravity/camera blend curve, chosen by play test.
3. Mode-blend altitudes (4.5) after phase 4 captures.
4. MIT notice: if any file translates SoA code (as opposed to reimplementing
   from the math), add `docs/licenses/SoA-MIT.txt` with the copyright line
   and reference it in that file's module comment. Reimplementing from
   formulas and this design needs no notice; record which in the session
   report.
5. Patch mesh layout, job granularity and upload batching.
6. RON field names, as long as validators and round-trip tests exist.

## 8a. Calls made by the author (check these)

1. **Existing orbital content.** Flat worlds keep today's sky, megastructures,
   skyhooks, passes, radio and receiver bookings exactly as saved (the v24
   alignment booking and the `world.orbital` save section stay untouched; no
   migration). Planet worlds, which are new saves on a new generator version,
   drive sun, moon and pass timings from `engine_space` rails. The orbital
   gameplay is ported onto rails for planet worlds in phase 5.
2. **Planet size is in metres** (W08), so the planet is the same size at every
   voxel scale; at 0.25 m a 32 km face has `H = 64_000` cells, which fits `i32`
   with room. LOD band distances in 4.5 are metres. Generation and region cost
   scale with resident chunks, not face area. Planets are supported at all three
   presets; phase 2 tests run at 1 m and 0.25 m.
3. **Planets are an opt-in world type first.** `WorldType::Planet` behind the
   new generator version, with terrain and W-GEO stages only. Sites, dungeons,
   valleys and Undercrown become face-aware in a later phase; until then they
   do not generate on planets. Planets become the default world type only after
   that phase and a human review.
4. **Sim-facing rails are fixed-point.** Anything the simulation reads (sun
   elevation for thermal, solar power, growth, day logic) comes from an integer
   evaluation: phase angle from `tick mod period_ticks`, trig from a fixed
   lookup table, results quantised to integer units. f64 rails feed rendering
   and the map only. This keeps C15 replay intact without waiting for S9.
5. **Time warp before phase 11** is allowed only when the player is in Flight
   or Rails and no surface region is resident; unloaded regions stay frozen, as
   unloaded chunks are today. Warp on a surface (sleep) waits for the phase 11
   ledger. Phase 6 does not depend on M23.
6. **Planning depth.** Detailed implementation plans for phases 0, 1 and 5;
   phases 2 to 4 get plans once this track drafts the face and projection
   sections of the C02, W01 and R10 SDDs (the track writes those drafts; their
   owners' critics review them); phases 6 to 11 are fixed here as interface
   contracts only (`SpaceState`, `Conic`, SOI events, ledger hooks).

## 9. Decisions (human, 2026-10-24; recorded in [0025](../decisions/0025-space-track-soa-port.md))

1. **Edge geometry: model R, radial columns** (section 4.3). Faces are flat
   in the simulation grid, not on screen; 0025 supersedes that clause of 0024.
   Model F is rejected: its edge wedge taxes collision, fluids, meshing, light
   and pathing permanently.
2. **Schedule: phase 0 starts now; phases 1 and 5 are held** until phase 0
   lands, then decided again. The rest of the track keeps its master-plan
   milestones. This document is supporting context, not an input of record.
3. **Home planet scale: 32 km face edge, provisional.** The system-scale spike
   may revise it.
4. **System scale: decided by a small playable spike** (one vessel, two bodies,
   a clock) before numbers are committed. KSP-like ratios (orbits of minutes,
   transfers of hours, which require warp) are the reference; shorter
   transfers are the fallback if warp outside orbit is unwanted.
5. **Proper nouns: open, owned by the human.** Agents keep ids and do not
   invent placeholder names.

Commit hold: this document and 0025 are committed only after the
`CLAUDE.md` generator-version drift (47 written, 51 in source) has an owner.
