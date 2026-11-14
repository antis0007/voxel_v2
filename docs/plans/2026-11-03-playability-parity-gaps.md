# Playability gaps against modded Minecraft, 3 November 2026

Source: read of `2026-10-25-tech-mod-parity.md`, `RESIDENT_GAMEPLAY_AUDIT_20261029.md`,
`MASTER_PLAN.md` and the data files below. Counts are from the working tree at
branch `growth-xmb`. No code changed in this pass.

## Why nothing was implemented

Every addition that widens content needs a save bump. A new `BlockId` appends
materials (save 43 did this), a new `FoodKind` or resource changes item
encoding, a new machine face field bumped save 47. The working tree already
carries peer edits to `world/storage.rs`, `world/storage/*`, `inventory.rs`,
`farming.rs`, `recipes.ron`, `materials.ron` and the factory module. Picking
48 here would collide with those edits. Quarry block yield (`factory/blocks.rs`,
untracked) is already in progress by a peer.

## Gaps measured in data

| Area | Now | Modded-MC baseline | Needs bump |
|---|---|---|---|
| Crops | 3 rows in `crops.ron`: red cap, glow cap, sapling | grain, root and fruit crops, seeds, bonemeal-style growth | Yes: new crop blocks and seed items |
| Food | 10 `FoodKind`: 3 mushroom, 2 mutton, 1 fish, 4 potions | bread, stews, fruit, cake, golden foods | Yes: `FoodKind` is a u8 item id |
| Brewing | 4 rows, all from resource 18 plus one modifier | splash, long, strong, negative potions | Yes for new potion kinds; no for more modifiers on existing kinds |
| Stonecutter | slab, stairs and fence for plank, brick and cobble only | variants for every stone | Yes: shaped blocks are materials |
| Fishing | 5 rows | junk and treasure tiers | No: rows over existing items |
| Panning | 4 rows | n/a | No |
| Armor | 4 tiers with reduction wired to damage | enchant and trim layers | Partly |
| Tools | `engine_tools` has parts, traits, modifiers, repair; no persistent assembly in items | full Tinkers loop | Yes: `ToolAssembly` item |

## Open items already named by the parity plan

1. Machine face modes: landed as save 47 face-role word.
2. Pipe sorting per conduit face: reuses `filter` and side bits, no bump.
3. Quarry yield beyond ores: peer slice in progress.
4. Smeltery multiblock, molds, part casting, tool station integration: needs the
   Tinkers item bump.
5. Machine augments, ore processing depth, steam, digital miner, cardboard box:
   Phase 3 rows, each data plus one bounded change.
6. Multiplayer resident trade and companion ownership: listed in the resident
   audit, needs ordered host requests.

## Proposed order once a save number is free

1. One bundled bump carrying: three grain and vegetable crops with seeds, six
   foods (bread, stew, two fruit items, cooked fish, cake), a brewing tier
   (long and strong variants as data rows), and stone shaped variants.
2. Pipe sorting per face (no bump) before or independent of the bundle.
3. Fishing junk and treasure rows (no bump), safe now.

Claim the save number in the same commit as the first new `BlockId`, and read
`SAVE_VERSION` in `world/storage.rs` against all branches first.

## Landed 2026-11-03: save 48 crop and food pack (uncommitted)

Owner accepted a save bump. Save 48 appends wheat, carrot, beetroot and a shared
crop shoot block (after peers' bamboo), and four foods: bread, root stew, cooked
fish, harvest cake. Record layout is unchanged; a v47 save loads as before.

- Crops: three rows in `crops.ron`, all sprouting as the shared shoot, thin
  cutout boxes in `content/thin.rs`.
- Foods: rows in `foods.ron`, hand recipes in `recipes.ron`, sprites 60 to 63 in
  `items16.txt` (late foods map there because 20 to 35 are armour).
- Seeds: villager page 0 trades biomass for wheat, carrot and beetroot. No wild
  source exists yet because that needs a generator gate.
- Tests: ripening and seed payout per crop, soil refusal, recipe wiring, v48
  round trip with a v47 fixture. All pass.

Failing before and after this change, owned by peers: `materials_ron_round_trips`,
`recipes_ron_round_trips` (compact rows), `bamboo_grows_one_cell_at_a_time`.
HEAD is at save 38, so these hunks cannot be committed alone; they depend on the
uncommitted bamboo, smeltery and plaster blocks.

