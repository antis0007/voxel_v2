# Lighting, horizon coverage and CPU terrain pass

This change targets four visible problems: wall torches that read as detached from
their wall, a flat pale-blue daytime grade, open gaps between a lagging resident
stream and the distant backdrop, and repeated per-voxel terrain work during CPU
chunk generation.

## Wall torches

`src/mesh.rs` and `src/main.rs` now share the same wall-torch mount:
`(0.50, 0.74, 0.56)` for north, `(0.50, 0.74, 0.44)` for south,
`(0.56, 0.74, 0.50)` for west and `(0.44, 0.74, 0.50)` for east.

The handle now begins about 0.08 m from the supporting face and leans only
0.36 m outward. The previous 0.18 m stand-off and 0.66 m flame reach made the
handle look suspended beside thin voxel trims. The bracket plate remains on the
wall face, and the point light uses the same flame centre as the mesh.

## Daytime grade

`LightingState::from_time_and_day` now keeps midday ambient lower while raising
the direct sun contribution:

- ambient day: `[0.155, 0.195, 0.268]` (was `[0.18, 0.22, 0.30]`)
- sun intensity: `2.1 + 7.9 * sun_elev` (was `2.0 + 7.1`)
- exposure: `0.49 + daylight * 0.425` (was `0.50 + 0.44`)
- underground fill now separates depth and face direction: a cool green-blue
  rock bounce for deep surfaces, a small warm floor bounce on downward faces,
  and reduced sky/cloud in-scatter under low `sky_access` so caves are readable
  without daylight haze flooding them
- zenith/horizon blue and daytime fog are slightly deeper and less washed out
- saturation: `0.77 + daylight * 0.27` (was `0.76 + 0.26`)

The intent is more slope and ridge definition at noon without making shadows
black. Night and sunset paths retain their existing thresholds.

## Horizon coverage handoff

The backdrop previously began at a fixed 78% of the requested view radius. If
the stream had not published resident meshes that far, the discarded inner
backdrop left a gap.

`ChunkCache::published_coverage_meters` computes a conservative radius where the
visible XZ chunk columns have published GPU meshes. Missing columns are measured
by the nearest point on their bounds, reduced by an 8 m safety margin.

The first version took the absolute nearest miss. One chunk that was remeshing,
newly expanded, or temporarily evicted was enough to collapse the entire handoff
to that column. The resident world beyond it then faded to sky and the low-poly
backdrop moved in next to the player until the chunk returned. Sparse misses are
now rejected as holes to fill rather than evidence that the whole ring ended;
several misses clustered at the same distance still reduce coverage immediately.
`backdrop_coverage_meters` quantizes the result down to 64 m steps, holds a drop
for one second, and then eases any genuine regression. A hole repaired by the
next streaming result never moves the horizon at all. Teleports over four chunks
reset the hysteresis instead of holding an obsolete radius.

The radius is passed to:

- `Horizon::update` as the hidden radius, so the horizon hole never exceeds the
  resident meshes that exist;
- `CameraUniform.coverage.x`, so `terrain_backdrop_start()` uses the same value.

A negative coverage means "no override" (menu uniform); zero means no resident
mesh is published. In that case the backdrop is not discarded, so a cold start
cannot expose a hole. The vertex sink and fragment discard still use the same
window, and exact edited fallback geometry remains authoritative.

## Cold horizon build

The first backdrop build was also serial. `horizon::build` now gathers the exact
unique `(block_x, block_z, detail)` requests, reuses samples from the previous
backdrop when the detail matches, evaluates the misses on up to eight scoped
workers, and then assembles the mesh in the original owner order. The rendered
geometry is unchanged.

Measured with the ignored `profile_cold_horizon_build` test (debug profile,
Hills seed 2738958700):

| Path | Cold build | Vertices | Indices |
|---|---:|---:|---:|
| serial (`VOXEL_HORIZON_SERIAL=1`) | 177.28 s | 2,357,543 | 8,872,278 |
| parallel prepass (default) | 32.14 s | 2,357,543 | 8,872,278 |

The 5.5x debug-profile speedup is the relevant comparison between the two code
paths. The previous release receipt measured 17.85 s for the first backdrop;
release timing was not remeasured here. `VOXEL_HORIZON_SERIAL=1` remains as a
diagnostic fallback.

## CPU terrain generation

Three changes reduce repeated work without changing generation formulas:

1. `generation_cache` column capacity rose from 8,192 to 32,768 entries; tree and
   bush caches rose from 1,024 to 4,096. A padded chunk consumes 1,156 columns,
   so the old column cache covered about seven chunks per worker and cleared
   wholesale at the frontier.
2. `sample_tectonics_zoomed` now has a per-worker XZ cache. The tectonic plate
   solve depends only on XZ, but `geological_stratum` previously recomputed it
   for every solid voxel. Results are identical; only the repetition is removed.
3. `ColumnSample` now carries the column-constant cave terms: water clearance,
   ravine strength, ravine centre and ravine half-height. `cave_carves` uses them
   instead of four 2D noise evaluations per underground voxel. The remaining
   three 3D cave fields are unchanged.
4. Cavern sampling rejects voxels above the natural column height before the
   gallery/dungeon/valley solve. Caverns and dungeon bands belong below the
   surface, so this removes per-voxel cave work from the sky portion of surface
   chunks.

Single-threaded generation of 100 padded chunks (Hills, v16) measured **2.693 s**
in the existing cost test, with the v15 control at **2.589 s** (1.04x). These
numbers include the test's warmups and five alternating runs; they are not a
frame-time or cold-start measurement.

## Pixel-art resolve

`postprocess.wgsl` is no longer a generic FXAA/random-noise resolve. It now
implements one coherent pixel-art treatment for terrain, water, sky, clouds and
aerial haze:

- **Native resolve:** the scene is sampled and resolved at one texel per output
  pixel. There is no art-pixel block scale.
- **16-bit float resolve:** the resolve target stays `Rgba16Float` through the
  glow and grade, so smooth sky, fog and cloud gradients are not quantized
  before the final write.
- **No palette quantizer by default:** the authored material and lighting ramps
  reach the display without a 32/64-level palette. High-contrast detail drives
  the image instead of a simulated limited-palette look.
- **Native blue-noise dither:** the present pass performs the only 8-bit write
  and uses the existing blue-noise texture per **native screen pixel**. This
  breaks sky and horizon banding without a visible ordered pattern.
- **Stacked Noita-style glow:** bright scene light is extracted at native
  resolution in two halo layers. A tight four-neighbour ring keeps torches and
  emissive voxels focused; a four-tap wide ring lets spells and lanterns bleed
  soft, saturated colour into nearby dark pixels. The default threshold is 0.72
  and strength 0.30. The centre contribution is support-gated, so a lone bright
  pixel in blackness does not become a floating cross.
- **Dark despeckle:** near-black blocks blend toward their four-neighbour
  average to suppress single-texel popping while moving in caves.
- **Native FXAA:** optional native-resolution FXAA remains available when the
  user enables it.
- **Split tone and vibrance:** a restrained cool-shadow / warm-highlight lift
  follows the neutral tonemap, and selective vibrance lifts less-saturated sky
  and cloud chroma without pushing already vivid materials into neon.
- **Vignette:** 6% maximum at the frame edges.
- Debug views bypass bloom, grading, dithering and vignette, and remain
  native-resolution diagnostic images.

The correspondences live in `src/render/postprocess.rs` and
`src/shaders/postprocess.wgsl`. The resolve still writes the existing HDR scene;
no extra scene render or full-size target was added.

## Terrain fade and holes

`terrain_streaming_fade` previously faded resident terrain to bare sky whenever
the backdrop had not revealed, starting at roughly half the requested radius.
That produced a moving ring of holes while streaming. It now:

1. returns zero until the horizon mesh is published,
2. fades only inside the backdrop overlap band,
3. uses the same published coverage radius as the backdrop handoff.

Resident chunks are still discarded beyond `backdrop_start + seam`, but the
backdrop now covers that region instead of a sky-coloured resident ring.

## Void fall guard

The player must never be lost to an unloaded bottom chunk. Two independent
safeguards now exist:

1. `World::solid` returns true at and below `min_world_y`, giving collision an
   invisible bedrock plane that does not depend on chunk residency.
2. `rescue_from_void` runs after the fixed-step player update. It records the
   last grounded position while the player is safely above the floor, then
   returns the player to that position (or `suggested_spawn` if no grounded
   position exists) if feet fall below `min_world_y + 2`.

The rescue clears velocity and prints “You fell out of the world. Returned to
solid ground.” It is a recovery contract, not a claim that every surface hole is
fixed. A player who falls through a terrain hole will still be returned safely
instead of falling forever.

## Verification and limits

- `cargo check --offline --target-dir target` passed, including build-time WGSL
  validation of the new resolve, two-scale glow and shared camera struct.
- `cargo test --offline --target-dir target`: **60 passed, 0 failed, 5 ignored**.
  Three coverage tests cover isolated near misses, a complete ring, and a
  contiguous near sector; a fourth verifies the authored compass waypoint.
- The release executable could not be replaced because
  `minecraft_style_voxel_prototype.exe` was running (PID 30912 at the final
  check). The process was deliberately not terminated. Close it and rebuild
  before judging the visual defaults.
- No runtime visual capture or frame-time profile was performed in this pass.
  The new horizon hysteresis and glow still need an in-game capture at the
  reported seed/weather. Pixel scale 3/2, 32-level palette, 17% edge darkening,
  0.34 two-scale glow, 0.70 threshold, one-code dither and 10% vignette are
  initial art-direction values, not final acceptance.
- The resolve still rasterizes the scene at native resolution; pixelation changes
  sampling and colour treatment, not raster cost. A lower-resolution scene
  target would be a separate performance/quality change.
- Coverage is a 2D column test. It does not yet prove that every LOD mesh is
  uploaded for the exact draw list or that far shadow casters exist. Those
  remain separate renderer work.
- The 64 m coverage quantum is a lower bound; resident terrain may overlap the
  backdrop by up to 64 m. That overlap is hidden by depth and is preferable to
  an open gap. A sparse transient hole may briefly expose sky instead of moving
  the global handoff; that local gap is the intended trade for not replacing the
  whole resident world with a low-poly ring.
