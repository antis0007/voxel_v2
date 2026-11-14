# Gilded Cave realm: treasure caverns, palace wings, pillar chamber (design)

Status: in progress 2026-11-13. Owner request: "Aladdin Cave of Wonders inspired areas, with dungeons,
loot and a final room like the area in the movie. Similar colour scheme, many new building blocks
inspired by it and by the zigzag palace areas in The Thief and the Cobbler. Really good textures,
refined with the dev tools until better than a Minecraft mod." The owner delegated the design calls
("refine it yourself against the docs and spec").

## Intent

A place a player enters on purpose, crosses as a sequence of set-piece rooms, and leaves richer.
The rooms follow a fixed order of light and colour: warm gold for wealth, then cold teal for the
secret at the end. The building blocks are good enough that players carry them home to build
palaces.

## Anti-goals

- No Disney or Williams IP. No tiger-head entrance, no lamp item, no genie, no carpet, no names
  from either film. Only the palette logic, the scale and the motifs (coin dunes, a pillar stair
  under a shaft of light, op-art chevrons, Persian tile geometry) carry over. All of these motifs
  exist in real architecture or are generic.
- No posterised flat fills (memory: no posterization). Smooth lit shading on top of pixel-lattice
  patterns.
- No content tables in Rust beyond what the existing realm pattern already does (layout is code,
  like `lightless.rs`). Materials, recipes and descriptions live in `assets/*.ron`.

## Approaches considered

1. **Pocket realm entered through a placed gate** (chosen). It mirrors the Lightless (save 74):
   `RealmKind::Gilded`, save code 8. No historical world is this realm, so its first layout needs
   no generator gate. That avoids the crowded gate numbers (up to 304, with collisions across
   branches). The layout is fully authored and the set pieces are guaranteed.
2. Rare structure under desert dunes. Rejected for now: it needs a new generator gate, fights
   biome/terrain files that peers own, and only new worlds get it. It could come later as a found
   gate on the surface that leads into this realm.
3. Theme on dungeon v2 complexes. Rejected: it inherits the shoebox room tree and loses the big
   rooms.

## Blocks (save 75, appended after `LightlessShaft`)

Names follow the UI_RULES `<Material> <Thing>` grammar. They are descriptive, not proper nouns,
but the realm name "Gilded Cave" and the gate name still need owner sign-off.

| Id | Name | Look | Source motif |
|---|---|---|---|
| GildedGate | Gilded Gate | gold frame, lapis and turquoise inlay, glow | entry block, light source |
| OchreSandstone | Ochre Sandstone | domain-warped iron-oxide bands, gold to rose to violet | slot-canyon sandstone |
| GrottoStone | Grotto Stone | teal-blue rock, calcite veins, wet sheen | lamp-chamber cavern |
| GoldCoins | Gold Coins | stacked and scattered coin discs, metallic, rare gems | treasure dunes |
| JeweledGold | Jeweled Gold | cabochon gems in gold settings, metallic | jewel-encrusted hoard |
| ZigzagTile | Zigzag Tile | black and ivory chevrons, gold pin line | op-art palace |
| LozengeTile | Lozenge Tile | concentric diamonds, ink, paper and lapis | op-art palace |
| StarTile | Star Tile | 8-fold zellige star, cobalt, turquoise, white, ochre | Moroccan zellige |
| LapisBrick | Lapis Brick | ultramarine glazed brick, gilt joints | Isfahan glazed brick |
| GildedRelief | Gilded Relief | gold arabesque vine on a lapis field, worn gilt | gilded arabesque |
| InlaidMarble | Inlaid Marble | white marble, carnelian, lapis and malachite flower | pietra dura |
| CarvedSandstone | Carved Sandstone | pink ashlar with a carved band | palace masonry |
| ScaleTile | Scale Tile | turquoise glazed fish-scale tiles | dome tiling |
| Sunstone | Sunstone | warm white crystal, light source | the shaft of light |

Shader: one new file `gilded_surfaces.wgsl`. It branches on block id (the industrial-block
pattern, family 12), because the 64 family slots are nearly full. Per-texel metallic and relief
tilt come from two private vars that `fs_main` and the art_refiner harness both apply.

## Layout (world metres, scale-independent, +z is the procession)

1. **Arrival stair**: the return gate sits at the origin on a landing. A 7 m wide sand stair
   descends 14 m between ochre walls, lit by torches.
2. **Palace wings** (the dungeon): a long colonnaded hall with zigzag floors, lapis walls,
   gilded relief bands and scale-tile domes. Side vaults carry chests (dungeon treasure loot) and
   a few pit traps over sand, so the floor is not safe everywhere.
3. **Treasure hall**: a large ochre cavern with coin dunes (a noise heightfield in Gold Coins),
   jeweled-gold outcrops, gold blocks, half-buried chests and torch-lit pillars.
4. **Pillar chamber** (final room): a tall teal grotto with a still pool and stepping stones. A
   rock pillar rises to a summit with a long straight stair up its face. A sunstone in the roof
   lights the summit, which holds the richest chest. Foxfire patches give cold teal fill light.

## Loot

Chests in the realm pay through the existing `dungeon_treasure` roll (potions, materials, wands).
The summit chest pays the survey-tier roll.

## Lighting

A realm-specific light state: no sun, a warm low ambient so gold reads in shadow, and warm fog.
Block light (torches, the gate, sunstone, foxfire) does the rest.

## Left to the builder

Exact room dimensions, trap density, palette hex tuning (the inferred hexes in the reference
brief are a starting point), and capture poses.

## Verification

- `art_refiner` sheets at `--dist 1` and `--dist 6` for every new block.
- `cargo check` for app and worldgen.
- In-game captures of each set piece (new area: no before frame exists; evidence is the frames).
