# Generator 76 integration

The shared tree had already moved drainage from 71 to 73 and the new-world
default to 75. This integration uses 76, leaving saved 73–75 terrain on the
previous solve. Planet (74), Nether and Superflat retain their own generators.

- Snow discharge follows the receiver graph and carves bounded parabolic
  glacial troughs before final river/lake routing. Landforms use an eight-degree
  colder glacial climate; they do not simulate moving ice.
- Wet permafrost with summer thaw receives static downhill thaw scars. These
  are bounded cuts, not time-evolving landslides. Local erosion detail cannot
  raise peaks in 76.
- Cistern floors use a cached natural-ground/water survey and can now sample
  above the natural surface. Habitat rooms, source actors, lantern interaction
  and rendering share the same elevation. Stair rise reaches its surveyed
  daylight exit, with rooms kept above the world floor.
- Settlement already surveys/grades terrain. Grove already surveys terrain;
  its elevation limit now accommodates the new ranges and rejects lake sites.
- Distant coarse triangles sample solved lake heads without local erosion
  tiles. Narrow rivers remain confined to fine rings. Lakes smaller than the
  far-field mesh spacing can still disappear.
- Thermal and vegetation climate use the drainage climate. Thermal seasons
  use the existing twenty-day sky calendar, with damping underground.
- Join identity mixes the solved terrain checksum into its existing content
  digest. The checksum covers heights, lakes, discharge, temperature, moisture,
  seasonal amplitude, receivers and river nodes. Joining waits for the solve.
- Climate advection precomputes static upwind stencils and rain rates. The
  planet log reports solve milliseconds separately from other world loading.
- Loading progress uses the selected generator's cache.

Validation: no tests added or run. The first offline app check stopped at a
shared karst `B::Snow` compile error (subsequently corrected by another editor).
A coordinator also fixed the new lapse calculation's unqualified sea-level
constant. Final compilation and runtime review belong to the coordinated
combined build; this is source integration, not visual acceptance.

Still open: startup measurements/captures, loading-screen observation,
multi-seed structure walkability and foundation grading, cross-machine checksum
comparison, and other-body terrain constants from `star_systems.ron`. The
spherical generator remains separate; this is a bounded home-face solve.

Source edits held for the shared combined build after integration.
