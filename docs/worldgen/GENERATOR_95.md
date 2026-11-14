# Generator 95: habitats instead of competing noise masks

Generator 94 introduced seven names but used one regional lottery to decide
between them. A warm slope could become a plateau; a wet patch could become a
marsh without occupying low ground. Tree silhouettes and ground palettes changed
at the label boundary, and distant mushrooms became green broadleaf crowns.

Generator 95 adds no biomes. It replaces that placement policy for integrated
Hills/Ridged worlds and keeps the generator-94 implementation for historical
saves.

## Habitat relationships

The selector surveys the solved geological surface and the existing sculpted
destination height field at fixed distances: a 24 m
gradient, a 96 m surrounding-height ring, precipitation and runoff accumulation.
These inputs do not depend on chunk resolution or the horizon triangle size.
They are terrain-derived habitat heuristics, not a water-table simulation.

- Lost plateau requires a gentle raised shelf above more varied surrounding
  relief. The selector identifies existing landforms; it does not manufacture
  a mesa or alter the river solve to make its name true.
- Cycad marsh needs warm, wet, gentle valley ground or runoff accumulation.
  Rainfall alone is insufficient. It does not invent standing water.
- Cloud forest favours cool humid upland shoulders with surrounding relief.
- Redwood groves need moist but draining ground. A separate stand-history
  field governs mature forest occupancy, rather than competing with marshes
  through the same lottery value.
- Fungi occupy smaller sheltered decay pockets inside existing forest or
  rainforest. Their canopies mix with ordinary woodland instead of replacing
  every tree with a mushroom.
- Last shore remains a cool coastal variant.
- Overgrown gardens no longer generate in new integrated worlds. A garden
  without authored garden remnants was an unsupported label; generator 94
  remains readable without silently changing old saves.

## Boundaries and populations

Continuous habitat affinity is retained alongside the display label. A blended
edge mixes ground patches, tree admission and vegetation density. The label's
midpoint does not independently swap the full surface or tree population.
The starting landscape fades into this policy between 130 and 210 m.

Redwood groves include young trees as well as mature canopy trees. Fungal
pockets retain a majority ordinary-tree lottery before the habitat blend.
Native review prompted a second redwood pass: unequal rotating branch fans,
tree-specific tier spacing, flared roots and a woody leader ending inside the
upper foliage replace the regular stacked rings and exposed brown tips.
New soil patches still yield to existing rivers, lakes and authored districts.
No biome transition changes terrain height.

The distant-tree source now carries canopy kind and foliage material. Cycads
retain low crowns; mushrooms retain caps and fungal stems rather than becoming
green trees. These remain inexpensive proxies, not exact copies of each voxel.

## Measured offline coverage

`habitat_survey` sampled a 5.12 km square at 64 m intervals: 6,400 positions per
seed. Distances below are straight-line distances to sampled labels, not walk
times, complete region boundaries, route guarantees, or native visual evidence.

| Seed | Redwood | Cycad | Cloud forest | Fungal pocket | Shelf | Last shore |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 424242 | 256 m | 500 m | 500 m | 528 m | 689 m | 716 m |
| 1337 | 272 m | 590 m | 607 m | 607 m | 516 m | 1701 m |
| 8675309 | 181 m | 543 m | 643 m | 724 m | 326 m | 842 m |

In the matched seed-424242 survey, fungal labels shrank from 2.42% to 0.17%
of the sampled area, shelf labels from 1.69% to 0.11%, and rainforest recovered
from 2.97% to 6.17%. There were 179 samples within blended edges versus zero
continuous transitions in the generator-94 policy. Rare regions can still be
smaller than this sampling grid; this is not a minimum-area guarantee.
The version comparison includes the preserved sculpted-destination pass, so
the total area changes are not an isolated measurement of ecology alone.

An optional `--trees` pass searches actual admitted tree anchors near habitat
cores. Seed 424242 yielded a 29 m redwood at (18, 41, 275), a 6 m fungal stem
at (1284, 18, -688), an 8 m cloud-forest broadleaf, and 3–4 m cycads in marsh
and shelf habitats. The nearest fungal label did not contain an admitted
large mushroom within 120 m; biome labels are not a canopy-location guarantee.

Run the diagnostic without tests:

```powershell
cargo run -p engine_worldgen --example habitat_survey --offline -- 424242 95 habitat-95-424242.svg
```

The SVG records the map and legend. The diagnostic does not write saves or
change game settings. No tests or hosted CI were added or run. The app compile
check and isolated development build passed. Native traversal, canopy transitions
and frame performance require separate review from static camera captures.

## Native review artifacts

All captures use the built development executable, seed 424242, one-metre
voxels and fresh isolated review worlds. Receipts include executable hashes.

- `target/habitat-review-95/native-redwood-anchor/camera.png`: actual 29 m
  redwood stand before silhouette refinement.
- `target/habitat-review-95/native-redwood-final/camera.png`: matched camera
  after the branch-fan, root and leader refinements.
- `target/habitat-review-95/native-fungal-anchor/camera.png`: mixed woodland
  with mature amber and umber caps and small ground mushrooms.

The capture helper stops only its owned process after a frame and final render
report exist. Acquisition timeouts also appear in the renderer logs; these
artifacts establish static appearance, not smooth traversal or device stability.
The review executable is
`target/worldgen92-check/debug/minecraft_style_voxel_prototype.exe`.
