# Axiom art direction

Modular, editable voxel construction with the colour craft of a SNES/GBA RPG.
Spend detail on silhouette, construction and readable material behaviour.

- Metre-scale plans own footprints, circulation, foundations and roof masses.
  Detail resolves at 1, 0.5 or 0.25 m without moving doors or blocking aisles.
  Fine geometry is for fittings and anatomy, not noisy surface subdivision.
- Existing world albedo has a 5 cm lattice. Evaluate 32 px/m against that in a
  controlled material scene before changing it globally. Held assets retain
  their documented foreground texel scale until compared at gameplay distance.
- Use connected clusters and limited material ramps. Quiet broad planes,
  oriented wood grain, masonry courses, restrained wear at functional edges.
- Pale warm masonry, dark timber and warm roofs establish settlement hierarchy.
  Grass needs sun/shade and substrate families, not uniform saturation.
  Copper, ceramic and mineral accents identify working parts.
- Distinguish material albedo from illumination. Preserve linear-light shading,
  stable distance filtering and coherent sun/sky contribution. No global pixel
  filter; no bloom used to conceal weak forms.
- Trees need distinct crowns and branch masses. Plants grow in communities
  selected by substrate, moisture and canopy; leave clearings and walking space.
- Buildings need foundations, posts, wall plates, thin roofs, eaves, framed
  openings and purposeful work/storage areas. Keep negative space legible.
- Arcane accents belong at sources, conductors and receivers. Heat, charge,
  motion and damage must represent actual simulation state. Decorative machine
  blocks do not establish a connected working machine.
- LOD must preserve silhouette, palette and placement. Judge near/mid/far and
  motion with fixed seed, camera, weather and executable; profile separately
  from screenshot readback.

Acceptance is native gameplay-scale imagery, including neutral and local light,
not compilation or a Blender beauty render. Preserve previous saves with
generator gates. The 17-asset workshop, eight material families and three real
interactions remain the target integration milestone, not a completed feature.

Existing held-asset rules: `docs/design/PIXEL_WORKSHOP_RESET_20261026.md`.
Current implementation/evidence: `docs/design/AXIOM_VISUAL_PROGRESS_20261026.md`.

## Regional worldbuilding standard

Use Magic: The Gathering's environment art as a benchmark for place, composition
and implied history. Retain AXIOM's editable voxel forms, pixel material clusters
and existing effects. Study the relationships in the reference; create original
geography, cultures and architecture.

The strongest reference is [Murasa's world guide](https://magic.wizards.com/en/news/feature/a-planeswalkers-guide-to-zendikar-murasa-and-sejiri):
cliffs restrict passage, a river opens a route, and people build around those
constraints. [Sam Burley's Forest](https://magic.wizards.com/en/news/magic-story/episode-2-race-murasa-skyclave-2020-09-09)
is a reference for large connected foliage and rock masses, depth and discovery.
An illustration composes one view; our region must survive movement and changes
of scale. A scenic camera alone cannot establish that standard.

| Criterion | Required in AXIOM |
|---|---|
| Regional identity | One dominant geological form, an ecological consequence and a recognizable built response. A palette swap does not establish a new region. |
| Composition | A large enclosing mass, open traversable space, a route and a destination. Use unequal masses and quiet areas between them. Avoid a landmark in every gap. |
| Ecology | Forest cores, glades, wet river margins and exposed divides follow the same physical fields at every distance. |
| History | Foundations, abandoned routes, excavation, retaining walls and altered waterways explain why a ruin or machine occupies its site. |
| Fantasy | An exceptional feature changes local travel, resources or habitation. Its supporting terrain and infrastructure matter as much as its silhouette. |
| Light and scale | Separate foreground, middle distance and skyline through form and light. Preserve terrain silhouettes and player-scale references. |

First regional target: a river cutting a pale escarpment, dark woodland gathered
in sheltered wet ground, open terraces above floods, and a crossing that explains
nearby habitation. A major tower needs a grounded approach and a visible service
footprint. Ruined infrastructure should reveal an older use of the same water
and route. These are composition requirements, not claims that a working economy,
historical simulation or RTS construction already exists.

Review the same seeded region from high altitude, medium altitude, the river,
across the valley, the forest edge and walking height. Hold camera, weather,
time, resolution and executable fixed for comparisons. From above, the drainage,
forest masses, buildable ground and destination must read together; from below,
the route must be physically present. Collect timing separately from image readback.

Gate 138 is a bounded altitude-continuity pass: it retains footprint-filtered
river valleys and riparian fields past the old 32 m cutoff and supplies solved
sediment to the distant material classifier. It does not complete the regional
target above. It is an explicit generator preview; the new-world default remains
126 while intervening generator work is dormant.

2026-11-05 evidence: the worldgen example built and the seed-74 sampler retained
the wet river band across the previous 32-to-33 m LOD discontinuity
(`target/reviews/region-art-20261105-sampler.txt`). The native app rebuild failed
during concurrent renderer changes; no fresh before/after image acceptance is
claimed. `tools/Review-RegionArt.ps1` records the six matched views using a frozen
executable once the app builds.

Gate 141 continues regional woodland composition. On the solved planet face,
the timberline follows lapse-adjusted climate instead of removing all trees
above 122 m. A fixed 96 m terrain survey favors concave shelter and opens convex
divides at every physical voxel scale and render footprint. Open-biome woodland
boosts retain reduced admission on gravel. Existing saves and default generator
126 retain their original rules.

Sampler evidence: `target/reviews/region-art-141-sampler.txt`. At the seed-74
upland survey, 81 of 169 samples exceed 0.15 vegetation admission at an 8 m
footprint, compared with 31 at gate 138. Gate 141's reported densities agree
across 1 m, 0.5 m and 0.25 m voxel grids at matched metre positions. This is
admission data, not actual tree counts or visual acceptance. Filtering still
changes admission at larger render footprints (105 and 110 samples at 64 and
256 m); near/far canopy coverage needs native comparison.

The worldgen example builds. The fresh native app build remains blocked by
concurrent renderer, industry and orbital-habitat compilation errors recorded
in `target/reviews/region-art-build-current.log`. No new image acceptance is
claimed. `tools/Review-RegionArt.ps1` now compares 138/141 at all three physical
scales by default. Use `region_art 74 --woodland-survey` for the additional
`upland` camera, then include `upland` in the script's `Views` parameter.
The 138/141 visual comparison includes intervening bamboo and stand-age gates;
it is a cumulative preview comparison. The final worldgen build passed using
`--target-dir target/region-art-isolated` (log:
`target/reviews/region-art-141-isolated-build.log`). A later native build retry
waited on the shared Cargo lock and was cancelled; it produced no new binary.
