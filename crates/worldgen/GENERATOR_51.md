# Generator 51: fine-scale rooms and facades (shared version)

New worlds use version 51. Everything below is gated at 51; 50 and earlier keep
their branches. No save-format change. Session voxel-v2-backup-0a also gates a
sites.rs siting change at 51 (disjoint files; see its own note if present).

Detail that resolves with the voxel size. All measures are metres on voxel
centres, so 1 m worlds are unchanged (a test rasterises refined and unrefined
site buildings at 1 m and requires them identical) and 0.5 / 0.25 m worlds gain:

- Undercrown halls (`undercrown/v2/fine.rs`): projecting plinth and cornice
  mouldings, a chain-hung chandelier with four candles in halls 9 m+ across
  (kept off props), jointed flagstone floors in every room and passage, a kerb
  along passage floors. Wet halls keep their plinth band clear.
- Site buildings (`architecture/fine.rs`, switched on by `Building::refined`
  in `site_building_version`): window frames (sill, jambs, lintel) with a
  glazed pane set back in the wall and a mullion cross, door architraves, a
  string course at each upper floor line, a taller capped plinth, and at
  0.25 m panels recessed one voxel between pilasters at the cell lines.
  Homesteads and dungeon gatehouses are not refined.

Also in this pass (dungeon generator 48, unreleased, corrected in place): the
chandelier plate was widened so a candle snapped to a 0.5 m voxel centre always
stands on it; a new test checks every fine-scale dungeon light is supported.

Tests: `architecture::fine` (1 m unchanged, 0.25 m gains detail, refinement
never fills a doorway at 0.5 / 0.25 m), the Undercrown suite at 51 (reach at
1 m and 0.5 m, every light hung on solid at 1 m and 0.5 m), dungeon
`fine_scale_lights_are_all_supported`. Same-pose captures, generator 50 vs 51
at 0.5 and 0.25 m (castle facade at seed 2738958700, two Undercrown halls).
Player traversal at 0.25 m is covered room by room, not across whole sites.

## Boss campus siting (`sites.rs`, session voxel-v2-backup-0a)

Generator 39 surveys the whole boss campus (ruin plus court, about 68 x 95 m for
the Foundry) and rejects it on more than 7 m of relief. From 38 on no Foundry
ruin (kind 3) was sited within ten regions of spawn on seed 424242, so its boss
complex could not be reached. From 51 a boss campus (kinds 1, 3, 5) that misses
at its rolled facing tries the other three, and the Foundry gets the starter's
10 m allowance. Over ten regions of spawn, Foundry sites go from 0-1 to 3-9 on
seeds 424242, 7, 99 and 2738958700; Toll and Archive sites roughly double.

Test: `every_boss_family_is_sited_from_generator_51`. `gen_golden` digests for
versions 36-50 are identical with and without the gate.
