# ADR 0038: Planet terrain from a drainage solve

2026-10-28. Accepted; dormant behind generator 71 until captures pass review.

Generators up to 65 build height from stacked noise (thresholded mountain
blobs, three unaligned ridged networks, height-gated amplifiers), then carve
rivers into it. Peaks came out 1:1 cones, valleys were closed noise contours,
and a measured catchment never exceeded 6 km² (spec gap map, 2026-10-24).
Climate was warped noise with no wind, sea distance or rain shadow.

Generator 71 replaces the height and climate inputs with `engine_worldgen::geo`:

1. Plates on a warped Voronoi set uplift: convergent belts that vary along
   strike, rift basins where land pulls apart, fractal coasts.
2. Stream-power erosion (Braun and Willett, m = 0.5, n = 1) over a
   priority-flood stack evolves the whole face. The solve has 256² cells of 128 m
   and runs 120 steps in two phases. The second phase weights runoff by the
   first climate pass, so dry lee slopes erode less.
3. Climate on the same grid:
   - Latitude comes from the planet's rotation axis.
   - Winds follow the three circulation cells.
   - Moisture is advected off the ocean and rains out where the wind climbs.
   - The lapse rate is 22 C/km because game ranges are compressed.
   - Seasonal swing grows with latitude, tilt and distance inland.
4. Per-column sampling bilinearly interpolates the face grid and adds detail in
   two layers:
   - 1 km tiles of 16 m cells run their own short erosion. Only the high-passed,
     clamped difference is kept, so detail cuts gullies and never lifts a new peak.
   - River channels follow the solve's receiver network as smooth curves through
     jittered nodes. Width and depth grow with the square root of discharge.
   - Flat permafrost gets ice-wedge polygons, with wedge ice beneath the troughs.
5. Far-field samples coarser than 32 m use the face grid alone, so the horizon and
   the ground agree to within the bounded detail.

The world is one bounded cube face, 32 km across and centred on the origin.
Its edges are ocean until face-edge crossing (decision 0024) lands. The rotation
axis is tilted so the face runs from the equator (+z edge) to the pole (−z
edge), with spawn at 45°N. Planet constants live in `geo::PlanetParams`; other
bodies take theirs from `assets/star_systems.ron` when planets become reachable.
The solve then extends to all six faces, with a coarse whole-planet pass that
fixes the boundaries of finer regional passes.

Determinism: the solve uses only IEEE-exact f64 operations (add, subtract,
multiply, divide, sqrt) in a fixed order, with no libm calls and no parallel
reductions. Every peer derives the same planet from the seed.
`geo::checksum(seed)` is the value peers compare when joining. This meets the
spec's fixed-point requirement without fixed-point code.

Cost: the solve takes about 1.6 s on one core in the dev profile and runs once
per process per seed. `geo::prepare` exposes real step counts and a preview for
the loading screen. A detail tile takes about 30 ms and is cached (256 tiles).

Not covered yet: glacial valleys, karst, deltas and braided rivers, soils as
their own layer, and thaw slumps.
