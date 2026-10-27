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
- Horizon modules are reached only through `render/horizon.rs`. `VOXEL_HORIZON_CUBES=1` is required for the cube band.
- Bounded passes take an `engine_core::Budget`; no `Instant::now()` in simulation.
- A new gate number is claimed in `docs/worldgen/GENERATOR_GATES.md` before it appears in code; dormant work names its gate with a `pub const` so it can be renumbered.
