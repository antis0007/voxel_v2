# World, materials and physics overhaul

This records the generator 2 baseline. New worlds now use generator 3; see
[the inland expedition update](EXPEDITION_UPDATE.md) for current features,
controls and the VXWSAVE4 manifest.

New worlds use generator 2. Choose **Continents & biomes** (the new default) or
**Highlands & peaks** to explore it; **Flat building world** remains available.
Existing VXWSAVE1 saves load using the legacy CPU terrain formula. New saves use
VXWSAVE2 and retain their generator version and partial water volumes. A saved
legacy world therefore stays on generator 1 when saved again. The prior GPU/CPU
generation mismatch means this does not promise identical old rendered terrain.

## Implemented

- Deterministic meter-space continents, elevation-dependent climate and ten
  biome categories: ocean, coast, wetlands, meadow, forest, taiga, desert,
  badlands, tundra and alpine peaks. Terrain, soil depth, vegetation and movement
  use physical units across 1 m, 0.5 m and 0.25 m block sizes.
- Overlapping 256 m erosion tiles on a 128 m grid. Each tile runs 48 rainfall,
  downhill sediment transport, erosion/deposition and thermal-creep iterations.
  Blending shared tiles makes generation independent of chunk/worker order.
  Lowland noise channels supplement this regional erosion; they are not a
  complete drainage-basin or river-network simulation.
- Biome surface materials, exposed slopes, conifers, deeper strata and coal/iron
  deposits. New selectable planks, stone bricks, cobblestone and pine leaves.
- One CPU source for rendering, collision, picking and edits, with bounded
  generation workers, cancellation and caches. The old GPU terrain shader is
  retained as inactive legacy code. Generation throughput needs runtime profiling.
- Water uses eight volume units per block, synchronous downward/lateral transfers
  and partial-height mesh faces. Sand and gravel fall, displacing water without
  creating it, and cannot fall into the player's collision box.
- Local physics runs within 24 m, processing up to 256 queued cells per step with
  a nominal 3 ms work cutoff. Terrain materialization can exceed that cutoff;
  it is a scheduling bound, not a measured frame-time guarantee. Remote worlds
  do not continue simulating. There is no fluid pressure or lava solver.
- Swimming, Space to rise, Shift to dive, water drag, reduced mud movement,
  low ice friction, buffered jumps, coyote time and improved diagonal stepping.
- Material-specific world patterns and inventory icons, meter-scaled grain,
  underwater attenuation and an underwater sky treatment. Water remains in the
  opaque terrain pass: this is not transparent/refraction-based water rendering.
- Biome/elevation HUD and physical view-distance controls. Changing block size
  preserves viewing reach subject to the existing 4–32 chunk limit. Smaller
  blocks still increase memory and generation/meshing costs substantially.

The previous disabled GPU fluid implementation is preserved in
`src/fluid/legacy_gpu.rs`; `src/fluid/active.rs` is the active solver.

## Validation and scope

Local command: `cargo build --offline --target-dir target`. The build script
validates the composed WGSL shaders. No tests, benchmark, game launch or visual
acceptance run was performed, following the workspace's no-tests instruction.
Compilation does not establish terrain throughput, biome distribution, save
round-trip behavior, movement feel, or visual quality in the live renderer.

This remains a creative sandbox with a substantially expanded world foundation.
Crafting, survival progression, creatures, combat, audio and full watershed
hydrology are not implemented by this change.
