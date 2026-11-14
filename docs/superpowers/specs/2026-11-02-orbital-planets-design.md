# Orbital planets: design

Owner request, 2026-11-02: near-photoreal planets from the orbital view,
frame times that stay flat, no ground chunks loaded out of view, no hitch
while in space, and a first idea of building in space. Reference points the
owner named: high-tech modded Minecraft (the Galacticraft, Ad Astra, GregTech,
Create: Aeronautics and Advanced Rocketry line of play).

Status: design for review. Nothing here changes a generator gate or a save
until a slice is approved and captured before and after. The planet cover in
the shared tree (`render/planet.rs`, `planet.rs`, `cube_sphere/atlas.rs`) is a
peer session's uncommitted work; this design builds beside it and replaces
the cover once the globe renderer draws the same surface.

## 1. What exists

| Thing | Where | Facts the design uses |
|---|---|---|
| Cube-sphere atlas | `engine_core::cube_sphere::atlas` (uncommitted) | radius 20,371 m, face half-width 16,000 m, chart pitch 65,536 m between faces, tangent projection, `split`/`join`/`to_body`/`from_body`/`frame` |
| Planet cover | `render/planet.rs` (uncommitted) | one mesh, 6 faces x 128 x 128 cells = 250 m per cell, flat shade, per-cell colour, folded on the GPU by the world shader when `cam.planet[0].w > 0.5` |
| Surface rendering | resident chunks to about 900 m at 28 chunks; horizon rings (8 m fine grid to 3 km, heightfield to 24 km) skipped on `WorldType::Planet` | the far field a flat world gets is missing on the planet |
| Star system | `assets/star_systems.ron`, `engine_space` | star 1.2e6 m, home planet 20,371 m, moon 1,000 m, gas giant 6e5 m; integer-tick rails, `System::evaluate`, `orientation`, `radius` |
| Atmosphere | `render/atmosphere.rs`, `atmosphere_lut_generation.wgsl`, `sky_probe.wgsl` | transmittance and multiscatter LUTs built at startup; aerial perspective LUT for the near field; sky probe evaluated once per frame |
| Clouds | `cloud_shader.wgsl`, weather field | volumetric march in three slabs, 4.1 ms at 800x450 on the 4090; depth-bounded; unusable on the iGPU |
| Orbital infrastructure | `world/orbital/*`, `mesh/models/orbital/*` | skyhooks with Courier, Freight and Conservatory carriages, anchorages, receiver, radio dish, meteors, ground monuments, pressure hull rules (`pressure.rs`), GPU-posed rigid assets |
| Flight review | `planet/review.rs` (uncommitted) | `VOXEL_PLANET_FLIGHT_REVIEW=1` flies ground, 1 km, 24 km, face crossing, descent, landing and captures each stage |
| Streaming | `render/streaming*` | interest = view distance in chunks around the player, independent of altitude; source cache 256 MiB; horizon pins |
| Perf gate | `scripts/frame_bench.py`, `crates/app/benches/frame_baselines.json` | p95 or p99 over baseline by 5 % fails; scenes are fixed command lines |

## 2. Intent

From a few kilometres up to high orbit the planet reads as one continuous,
lit, weathered body: terrain relief with shadows, coloured by what the
surface is made of, oceans with sun glint, a cloud shell that casts shadow,
atmosphere that thickens toward the limb and glows at the terminator, night
side lit by the player's own works. It is the same world the player walks
on: every ridge, lake, river, road and anchorage is where the voxels put it.
It costs under a third of the frame on the 4090 and runs on the iGPU tier,
and no ground chunk is generated, meshed or kept resident because the
player is in space.

## 3. Player experience (ideation)

Three bands of play, by altitude above the surface.

**Surface (0 to 1.5 km).** Unchanged. The globe is invisible behind the
horizon rings.

**Ascent and descent (1.5 to 30 km).** The ground stops being voxels and
becomes map. Resident chunks shrink to a disc under the player and stop
loading at all above 6 km; the globe renderer carries the view. Clouds
switch from the volumetric march to the cloud shell when the camera rises
above the top slab. Sound thins out. The HUD shows altitude and the ground
track. This is the skyhook carriage ride that already exists, and the view
out of its window is the reason to build the ride.

**Orbit (30 km and up).** Chunk streaming is paused and the resident cache
kept for the return. The globe is the whole scene: terrain, ocean, cloud
shell, limb, night lights, the moon and gas giant on their rails, the star
with the existing disc. The player moves in flight mode with the existing
flight speed curve (64 m/s, boost 4x), and the orbital frame rotates with
the planet so the anchorage under them stays under them.

**Building in space** (follow-on plan, not this one):

- The first yard is the crown of a skyhook the player already reached by
  carriage: a finite voxel region (the `regions` system) parented to the
  anchorage, so it turns with the planet and keeps its position above the
  ground factory. The Conservatory class carriage already carries a
  pressurised hull; the yard starts as its dock.
- Placement rule in a yard: a block must touch the structure (no free
  floating), and a placed block outside a sealed hull is "exposed". The
  pressure rules in `pressure.rs` decide sealed volumes; an airlock is two
  doors with a sealed cell between.
- Resources that exist on the ground carry up by carriage: Freight moves
  stacks between the ground station inventory and the yard inventory on the
  existing travel time, so the factory automation reaches orbit through one
  slow, visible pipe. Nothing teleports.
- New in orbit: solar panels that produce by real sun exposure (orbit night
  is real), radiators that must see space, oxygen as a stored resource that
  machines and the player consume inside hulls, and a telescope (the radio
  dish asset) that scans the moon and gas giant for survey data. Chips from
  `engine_chip` drive attitude lights, door logic and alarms, which is the
  GregTech-style payoff: the station is a machine the player programs.
- Later tiers, in order: a mass driver that throws cargo pods to the yard
  (faster than the carriage, needs power), a dock for a player-built hull
  that counts mass and thrust from its blocks, and the moon as a second
  surface with its own smaller globe.
- Register: no new proper nouns. Carriage classes, anchorage, yard, hull,
  airlock, dock are common nouns and stay so.

## 4. Goals

1. Globe renderer: cube-sphere quadtree of height and material tiles, baked
   from `engine_worldgen` on a worker under a `Budget`, cached on disk per
   seed and generator version, refined by projected error, drawn with
   normals, material palette, ocean, cloud shell, cloud shadow, atmosphere
   and night lights.
2. Altitude policy for streaming: the resident disc shrinks with altitude,
   loading stops at 6 km, streaming pauses above 30 km, the cache is kept
   for descent.
3. Frame budget: orbit-static p95 under 8 ms on the 4090 at 1600x900 and
   under 33 ms on the UHD 770 lean tier; ascent scene p99 under 16.6 ms on
   the 4090 with no frame over 40 ms; globe CPU under 0.5 ms per frame;
   uploads under 2 MB per frame.
4. One source of truth: the globe's height and material at any point equal
   the voxel column there (test against `sample_column`).

## 5. Non-goals and anti-goals

- No ground chunk generation, meshing or residency driven by the globe.
- No posterization, no banding; no new post effects beyond what the globe
  needs (one sun glint term, one limb glow term).
- No second atmosphere model: the globe uses the existing LUTs.
- No building, inventory or save-format change in this plan.
- Anti-goal: a flattering orbit screenshot with a hitch on the way up.
  Ascent is the acceptance scene.
- Anti-goal: a different planet from the one on the ground. Any mismatch
  with `sample_column` beyond one texel is a bug.

## 6. Constraints

- Shared tree: the planet files are a live peer's; this work lands on its
  own branch and reads the atlas constants through `engine_core`, which
  must be committed first (the peer's call).
- Coordinates: globe tiles are in chart coordinates (face index, s, t) and
  fold to the sphere on the GPU; vertices are camera-relative in f32 after
  an f64 subtraction on the CPU (`origin.rs` pattern).
- Determinism: bake order and output depend only on seed, generator
  version and tile id. Positional hashing only.
- `Budget` in every bake pass; no `Instant::now()` outside the render
  thread's scheduling.
- Visual changes carry before and after captures through the flight review
  harness at every stage.
- The globe must run on the lean tier: no volumetric march, one texture
  fetch per material, no screen-space passes.

## 7. Decided by the owner

- Photoreal-leaning look from orbit, performance and no ground loading are
  the hard requirements; building in space is ideation for a later plan.

## 8. Left to the builder

- Exact tile texture formats within the limits in the plan.
- Whether the cloud shell map is baked per face or sampled from the weather
  noise at draw time (the plan says baked; the builder may measure).
