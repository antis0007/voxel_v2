# Generator 52 (shared version)

New worlds use version 52. Every change below is gated at 52; 51 and earlier
keep their branches. No save-format change. Other sessions gate disjoint
files at 52 too; each adds its own section.

## Dungeons

Complex rooms and passages (`dungeon/v2`) read as old and neglected at 0.5 m
and 0.25 m voxels. Measures are metres on voxel centres, as in the generator
48 fine layer.

- Ruin layer (`ruin.rs`). Each room gets a damp value from its job, kind and
  damage (forges, orreries, the core and the kept rooms such as libraries are
  dry). Moss replaces up-facing stone (floors, plinth and cornice tops, pier
  bases, decks, rubble) in patches thickest at the walls; it never adds a
  voxel, and is the one part that also shows at 1 m. Vines hang down wall
  faces from the cornice or vault, from ribs and beams in very damp rooms,
  and down passage walls in damp complexes; never below 2.2 m at a wall or
  2.8 m in the open, never in rooms with a gallery deck or balcony in the
  open. Scree, fallen ashlar blocks (sometimes two stacked askew) and ferns
  by lit piers lie in a 1.5 m band along the wall foot, most in collapsed
  and breached rooms, none in flooded, kept, puzzle, cleft or orrery rooms.
  Overgrown rooms' metre-square root columns become tapered, swaying roots
  with a leaf fringe, inside the column the coarse root fills.
- Carving (`carve.rs`). Wall piers get a two-course base and a capital
  (echinus and abacus) under the cornice; at 0.25 m their faces are fluted.
  Flat beam ceilings become coffered: each field between the beams and the
  cross lines is recessed half a metre (stepped at 0.25 m) round a trim
  rosette.
- Fallen vaults (`collapse.rs`). A collapsed room's vault is open over its
  rubble cone: a ragged cavity up into the shell, hung with roots, its rock
  roof set with amber caps, which are light sources, so a warm pool falls on
  the spill.

1 m rooms differ from 51 only by moss. Door clearances run before every new
layer.

Tests (`dungeon/v2/tests_ruin.rs`): door-to-door reach in damp and damaged
rooms at 1, 0.5 and 0.25 m; the layer shows at 0.25 m and adds no vines at
1 m; generator 51 rooms carry none of it; 0.25 m sampling is deterministic.
The existing reach, gallery, light-support and pin tests run at 52 unchanged.
`gen_golden` digests for versions 2-51 are identical before and after.
Review: `VOXEL_REVIEW_DUNGEON=room:<name>` now also takes crypt, ossuary,
shrine, cistern, garden, barracks, store, cells, overgrown and collapsed; an
ignored `view_room` test draws a quick ray-cast picture of one planned room.
