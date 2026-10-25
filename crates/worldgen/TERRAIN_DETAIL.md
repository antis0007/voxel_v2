# Terrain detail tuning

`src/terrain_detail.rs` owns generator-38 residual relief. It runs after the
8 m erosion grid and spawn-basin shaping, before valley and landmark carving.
Versions below 38 retain their previous height recipe; newer generators inherit
this layer. Superflat bypasses it. Existing worlds keep their stored version.

Tune `profile(Biome)` for biome character. Its four amplitudes are metres at
32, 9, 2.8 and 0.9 m noise-cell scales. Meadows stay gentle; forests and rainforest
gain hummocks; marsh/fen relief stays shallow; desert fields stretch along a
seeded wind axis; badlands/alpine rock has stronger ribs and hollows. Cold ground
has restrained fine relief. Sediment damps the two smallest bands. These are
procedural approximations, not an erosion simulation at sub-metre resolution.

`climate_profile` blends profiles using temperature, moisture, height and slope.
Do not switch heights on discrete biome labels. Blend fixed fields rather than
scaling world coordinates by a changing climate weight. Shoreline and spawn
masks limit displacement where water continuity and walkability matter.

Generation filters each band between 0.20 and 0.50 cells per sample footprint;
unresolved bands skip their noise evaluations. The editable footprint is voxel
size. Preview meshes and distant normal maps use their sampling spacing. The
canonical horizon keeps its fixed 8 m macro policy but filters this residual
layer at 64 m beyond the exact rings, preserving shared-vertex/cache ownership.
This does not make the cheap macro evaluator identical to eroded resident terrain.

`../app/src/shaders/terrain_surface.wgsl` owns visual relief and climate tint
inputs for both near terrain and the horizon. It adds 48, 12, 3 and 0.85 m shading
bands, with separate turf/rock/sand/snow strengths. Pixel footprint removes
unresolved normal contrast and transfers an approximate variance to roughness.
These are shading normals, not collision or silhouette displacement. Existing
material textures supply finer grains, clods and pebbles. Shader climate includes
the same regional and ecotone stages as generation, but has no erosion-sediment
moisture contribution.

Tune generation amplitudes and visual normal strengths independently. Raising
noise amplitudes globally makes wetlands, dunes and rock look alike. Native
visual review and performance measurement are still needed before judging the
final balance; compilation alone does not establish either.
