# Cubic environment lighting review

Cubic geometry remains the chosen style. This pass improves environmental
lighting and the visibility of existing climate regions; it does not claim
photorealism or add a physically complete light transport solver.

## Changes

- Cached per-column overhead coverage attenuates daylight and atmospheric fill
  inside caves. Torch lighting remains independent. Mushroom/crystal emission
  is reduced. Edited roofs within a source chunk update its coverage metadata.
- Coverage is encoded into regular mesh vertices without increasing the GPU
  vertex stride. CPU source and worker reservations account for its allocation.
- Meadow, cold woodland, tundra and dry-grass colors use the existing seeded
  climate field continuously across nearby and distant surfaces.
- Cloud-shadow clipmap blending matches the visible cloud transition width.
  The outer map fades at its finite boundary instead of extending a clamped
  edge texel into a distant shadow band.
- `--benchmark-water-review` installs an unsaved pool and passes timed jump and
  double-tap input through normal player physics. It logs surface escape and
  flight state and captures both outcomes. This is a runtime control-path
  review, not a camera teleport proof or a manual keyboard interaction.
- Underground `--benchmark-terrain-sweep` uses a small translation and full
  rotation so the review stays within a chamber.

## Candidate and limitations

Release candidate: `target/release/natural-light-cubic-final2.exe`.
Frozen source: `target/natural-light-review/source`; SHA-256 inputs are listed in
`source-manifest.json` beside it. Build and composed shader validation passed.
Reviews ran after the other chats released their profiling/capture window.

## Visual review

`cave-final` contains the moving chamber review: a small translation and full
rotation, with dark distant walls, warm nearby torch illumination and reduced
cap emission. The first lower route intersected a mushroom cap and was replaced
with this higher route. `horizon-final` records a 100-second partly-cloudy sweep,
approximately 960 m of translation and more than one rotation. Cardinal views
and the recenter area were inspected. No obvious open gaps were seen in these
views; this is not proof of seamless rendering at every seed, scale or speed.

Both visual runs used a 2048 MiB GPU terrain cache. At the default 512 MiB,
the cave view left 1406 requested chunks deferred indefinitely; at 2048 MiB all
3093 were resident by 16 seconds. Use the larger terrain budget for this draw
distance on the reviewed RTX 4090. Budget/radius adaptation remains separate
engine work. Do not interpret the captures as default-budget acceptance.

The outdoor review exposed excessive canopy darkness: leaves were being treated
as opaque roofs. Final2 excludes leaves, pine needles and bushes from the
overhead estimate; ordinary shadow maps still shade vegetation. The final
same-scene check is stored in `foliage-final`.

## Water control evidence

The first actual player-physics run exposed insufficient surface clearance:
the one-metre jump peaked at 100.131 m against a 100 m water surface. The swim
kick is now 8.5 m/s, held ascent at least 8 m/s, and vertical cap 9 m/s.
Both final runs report jump PASS at 100.516 m, without flight. Double-tap flight
also passes at both scales, leaving swimming and reaching 143.994 m. Logs and
captures: `target/natural-light-review/water-final-1m` and `water-final-quarter`
(matching `.log` files sit beside the directories). Bulk world simulation is
disabled for the fixture; normal fixed-step player movement/collision remains
active. This establishes surface escape, not climbing every bank or keyboard
double-tap timing on a physical device.

Overhead coverage is a bounded approximation of skylight, not flood-filled
global illumination. Edits to an overhead chunk do not yet propagate a full
sky-light solution into every chunk below. Shadow detail beyond the outer
weather clipmap fades away; it does not invent new shadow data.
