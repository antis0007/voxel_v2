# Flora pack (2026-11-06)

Fills five plant gaps measured against Better MC 4 (exploration) and All the Mods 10
(progression): water plants, pickable fruit, wild plants that can be grown at home,
biome plants, and plants that do work.

## Blocks

36 `BlockId`s appended after `SulfuricAcid` (save 62). The id space is `u8`; 14 ids
remain, so sprout cells are shared between rows:

| Sprout | Rows |
| --- | --- |
| `WaterShoot` | kelp, seagrass, cattail, rice |
| `CropShoot` | potato, onion, cabbage, sea oats, lavender, orchid, cactus, agave, dripleaf, working flowers, iron fern |
| `FruitBud` | apple, cherry and fig crown fruit; cocoa |
| `TrellisVine` | grapes, tomato |
| `GourdVine` | pumpkin, melon |
| `CaveVine` | glow berries, spore blossom |
| `BerryBush` | sweet berries |
| `Sapling` | apple, cherry and fig trees (the fruit is the seed) |

Shapes and per-part colours live in `engine_content::flora`. Pumpkin and melon are full
opaque cubes. Cactus has contact damage 1/s. Glow berries are a light source and use the
amber light colour and clumping.

## Growth rules (`assets/crops.ron`)

New fields; old rows are unchanged:

- `planting`: `Soil`, `Submerged` (kelp, seagrass: water cell, water above), `Paddy`
  (rice, cattail: water cell, air above), `Floating` (lily pad), `Side` (cocoa on a
  log), `Hanging` (glow berries, spore blossom), `Grafted` (fruit set by a grown tree).
- `regrow`: Use on a ripe plant picks it back to its sprout, as with Minecraft sweet
  berries and Pam's fruit. A wild ripe plant gets a plot when first picked.
- `gourd`: a mature stem sets its fruit block on free ground beside it, then regrows
  (pumpkin, melon).
- `tree.fruit` / `tree.roots`: grown saplings bear `FruitBud` cells with grafted plots
  in the lower outer crown; mangrove propagules grow prop roots.
- `produce`: harvest pays a resource plus one seed (iron fern pays iron dust, after
  Mystical Agriculture's resource crops).
- `food`: the crop item is eaten with Use when no plant spot is targeted.
- `aura`: `Mana` (mana flower, open-sky wand mana within 8 m, after Botania's generating
  flowers) and `Collect` (collector flower moves loose stacks within 6 m into a touching
  chest, after Botania's Hopperhock).

Leaves drop apples at 1% per cubic metre, which plant apple trees.

## Wild flora (generator gate 210)

`engine_worldgen::flora`, live for new Hills and Ridged worlds. Planet worlds get it when
`PLANET_GENERATOR_VERSION` reaches 210.

- Sea and lake beds: kelp forests deeper than 3 m, seagrass meadows 0.5-4 m.
- One-cell shallows: cattail, and rice where warm and humid. Lily pads on calm water.
- Beaches: sea oats; wild cabbage and beetroot (Farmer's Delight places wild cabbage and
  sea beet on beaches).
- Grassland patches by climate (Farmer's Delight wild crops): potato and sweet berries
  when cold, onion, carrot, wheat, lavender and pumpkin when temperate, tomato, grapes,
  orchid and melon when warm.
- Hot dry sand: cactus columns and agave.
- Small trees: mangroves with roots and hanging propagules on warm mud shores; wild
  apple, cherry and fig trees with ripe fruit; cocoa pods on fig trunks.
- Caves: glow berries, cave vines and spore blossoms under rock; dripleaf on floors.

## Meals and trades

FoodKind 18-23: vegetable stew, baked potato, fruit salad, pumpkin pie, rice bowl, cocoa
biscuit (recipes beside bread). Mana flower, collector flower and iron fern are crafted
by hand. The farm villager page sells rice, cocoa, grapes, cherry, fig and mangrove
propagules for fungal biomass.

## Meshing

Submerged plants (`flora::is_submerged`) mesh their cell as water as well as plant
boxes, so kelp and paddies show no air pocket. Chunks on the exact partial-water mesher
do not yet draw water in those cells.

## Pass 2 (2026-11-08)

- Crop harvester (save 70, one more id; 6 remain): set with Use on soil; every second it
  reaps up to 8 ripe plots within 4 m, replants (keeps one seed), picks regrowing plants,
  cuts stalks above the base, breaks gourd fruit, and fills a touching chest or drops
  beside itself. Modelled on Create's mechanical harvester and the IE garden cloche,
  without rotation power.
- Plants whose support is dug out (soil, log, ceiling, water) pop and drop their seed.
  A support swapped for another solid block still only stalls growth.
- Compost recipes turn seagrass, kelp, sea oats, cattails and leaves into fungal biomass,
  which already fertilises plots.
- Picking shows a hint and a burst in the fruit colour.

## Open

- Multiplayer authority does not yet route picking or eating produce.
- Tomatoes do not climb past one cell; cocoa pods do not face their log.
- Full survival progression, multiplayer picking and save/reload acceptance remain unverified.
- No power or rotation cost for the harvester; no seed quality or crossbreeding (Agricraft).

## Integration continuation (2026-11-11)

- Harvester payouts now count only successful stalk/fruit removals. Stale plots
  cannot harvest a column or gourd after their base/stem is removed.
- When the loose-item budget is full, harvesting waits unless the adjacent chest
  can accept the complete output. Crops are preserved instead of silently lost.
- The flora review camera now uses the same cell coordinates as its staged field
  at each supported voxel scale.
- The integration snapshot includes the shared game dependencies. Compile fixes
  retain ordered worker-cell certificates using coordinate arrays, restore the
  resource deserializer's sibling-module visibility, and import the transit queue.
- No new tests were added or run during this continuation.
- The shared-tree release-fast build passed. Native before/after captures under
  `target/creature-review/flora-integrated-20261111-{before,after}/camera.png`
  show crops replanted, gourd fruit removed and the cactus base preserved.
  `flora-integrated-20261111-half-scale/camera.png` verifies the 0.5 m camera.
  These are staged art captures, not full survival or inventory acceptance.
- Include the compile-time wand atlas `assets/source/wand_ecosystem/minimal_v2/atlas.rgba.gz`
  even when omitting untracked reference-art/source folders from a source snapshot.
