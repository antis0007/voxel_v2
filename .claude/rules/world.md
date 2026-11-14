---
paths:
  - "crates/app/src/mesh*"
  - "crates/app/src/mesh/**"
  - "crates/app/src/render/horizon/**"
  - "crates/app/src/render/streaming/**"
  - "crates/voxel/**"
  - "crates/worldgen/**"
  - "crates/core/**"
---
# World, meshing and LOD contracts

- Voxel cell `IVec3`, chunk `IVec3` (32 cubed, floor division, never `/`), metres `Vec3`. Padded chunk is 34 cubed. Test negative coordinates and large magnitudes whenever a coordinate conversion changes.
- Seams and flicker live at chunk and LOD boundaries and in residency transitions (build, upload, evict). A fix needs a fixed seed and camera route that crosses them, captured before and after; compiling is not evidence.
- Worldgen is a pure function of `WorldSampleConfig`. A behaviour change needs a new generator version gate; never edit a historical gate (see saves rule).
- Planet, Nether and Manyfold previews use that world's own column. A hills preview streams the wrong vertical band, so the spawn floor is missing until the exact range arrives. `dimension_previews_match_the_played_column` pins the height.
- Horizon modules are reached only through `render/horizon.rs`. `VOXEL_HORIZON_CUBES=1` is required for the cube band.
- Bounded passes take an `engine_core::Budget`; no `Instant::now()` in simulation.
- A new gate number is claimed in `docs/worldgen/GENERATOR_GATES.md` before it appears in code; dormant work names its gate with a `pub const` so it can be renumbered.
- Manyfold interior fill is multiplied by resolve exposure. Scale it by `fire_exposure`, or night turns the shell black. Lamps have to be `is_light_source` blocks or the torch list never sees them. A spawn clearance wider than one maze cell hides the first wall.
- Maze walls inside the drum still leave a giant tube if the space above them is air. A ceiling slab has to sit in the walked band, thick enough that one voxel sample cannot miss it.
- A player-edit mark demands the newest revision; a pending GPU upload older than its mark can never complete the edit group, so `edit_groups` must not hold it (it blocked the newer remesh for the whole hold). Check with `VOXEL_LATENCY_DEBUG=1` and the probe: a `[player-mesh-wait]` line with `in_flight=false` and `pending_gpu=true` is this stall.
