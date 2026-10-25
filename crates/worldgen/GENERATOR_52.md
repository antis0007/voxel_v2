# Generator 52: Undercrown dressing fitted to the room (shared version)

New worlds use version 52. Everything below is gated at 52 in
`undercrown/v2/settle.rs`; 51 and earlier keep their grid dressing (a test
checks the gate). No save-format change. Session voxel-v2-backup-a1 also
gates dungeon work at 52 in `dungeon/v2/` (disjoint files).

Before 52 the Undercrown's mushrooms, orchard lanterns and Choir ice pipes
were drawn on a fixed grid over everything else in their box. In the chapel
the caps cut through the columns, the altar and the vault; grid cells past
the box edge left caps with no stem; orchard stems stood in the cave wall
and lantern chains ran through the caps; ice pipes stood half inside the
cave walls. Measured at 0.5 m over seeds 424242, 7 and 99: 2-19 voxels of
mushroom inside columns, altar, pews or lanterns per seed, 13-72 voxels of
stem or cap inside rock, 120-243 voxels of ice pipe in walls.

From 52, `settle` replaces each grid with pieces fitted to the finished
layout. Every candidate is probed at a quarter metre in both mechanism
extremes and must stand in air, 0.4 m clear of every other feature, off
the props and out of the passage mouths:

- Mushrooms (`grove.rs`): a grid spread evenly over the box; each tries
  its cell's middle and rings of spots out to 3 m, four sizes and two
  heights. The new shape has a flared foot, a domed cap with a drooping
  rim, glowing gills, two metres of headroom under the rim, and at 0.25 m
  a skirt on the stem (at 0.5 m it reads as a crossbar, so it is left out).
  Caps may overhang pews but never the floor or the walls.
- Shelf fungi on the faces of the columns in a mushroom room, one or two
  above head height, pale with glowing gills where two voxels thick, about
  a third luminous; 0.5 and 0.25 m only. The chapel, whose floor
  is pews, altar and columns, now has these instead of caps through the
  columns.
- Lanterns keep only those whose body hangs in air and whose chain reaches
  rock without touching anything; the chain ends half a metre in the roof.
- Ice pipes: standing ones need ground under them, hanging ones hang their
  old length below the roof straight above them.

Fitted pieces give way to the room wherever they still meet it (a probe
step can pass over a one-voxel torch), so they never overwrite a light.

Tests: `tests_clip::dressing_stands_clear_of_rock_and_every_other_feature`
(four seeds, 1, 0.5 and 0.25 m, both mechanism states),
`generator_51_keeps_its_grid_dressing`, and the existing Undercrown reach
and light tests at 52. Fitting adds under 0.1 s to a layout build.

Captures, seed 2738958700, generator 51 vs 52 from the same pose
(`VOXEL_REVIEW_DUNGEON=undercrown:<room>`): chapel at 0.5 m (51: the camera
stands inside a mushroom stem; 52: pews, chandelier, shelf fungi), orchard
at 1 m and 0.5 m (51: flat cyan slabs cut by the roof, chains through the
caps; 52: domed caps, lanterns clear), orchard at 0.25 m for 52.

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
