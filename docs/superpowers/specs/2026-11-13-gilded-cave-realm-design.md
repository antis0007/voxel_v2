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

## As built (2026-11-14)

- `crates/worldgen/src/gilded.rs`: landing at the origin (return gate in the floor), a 45° stair to
  the 12 m-deep gallery (6 m domed bays, columns, lapis/gilded/carved wall bands, scale-tile vaults),
  eight side vaults at z 25/37/49/61 (treasury, mirage-floored pit trap with a cache below, coin
  store, collapse), a grand arch into the coin cavern (60 x 56 m, dunes climbing to the walls, a
  causeway trench with torch kerbs, rock pillars, half-buried caches, gold-block stacks, jeweled
  crusts, sunstone clusters in the roof), a stepped passage down to the grotto (pool, shore,
  stepping stones, pillar with a 3-turn spiral stair, marble and jewel summit, pedestal cache, a
  second return gate, sunstone boss overhead, foxfire at the waterline).
- Stairs step 0.25 m at fine scales and one voxel at 1 m; the plan is identical at both.
- Lighting: `portals::realm::gilded_lighting` (warm low ambient and haze). The Hush darkness
  hazard is now Lightless-only.
- Caches in the realm pay through `containers::dungeon_treasure`.
- Gate travel is shared with the Lightless (`lightless_gate.rs`): one chart file per realm.
- Review cameras: `VOXEL_REVIEW_DUNGEON=gilded:<arrival|gallery|vault|wall|hoard|dunes|grotto|stair|summit>`.

## Needs owner sign-off

Player-visible names: "Gilded Cave", "Gilded Gate", and the 13 block names in the table above.

## Verification

- `art_refiner` sheets at `--dist 1` and `--dist 6` for every new block.
- `cargo check` for app and worldgen.
- In-game captures of each set piece (new area: no before frame exists; evidence is the frames).

## Water and light pass (2026-11-14)

Owner request: glowing blue water with caustics on the surface, under clear water and on the walls;
the blue stair grotto and the red treasure areas matched to the reference stills.

- Grotto layout: fluted walls (radius ribbed by angle, noise stretched vertically), stalactites on a
  2.6 m grid clear of the shaft, a boulder-stack pillar (2.1 m courses) on a ragged islet, one and a
  half sweeping stair turns with headroom cut through the capital, round stepping stones, a 4 m pool
  over a boulder bed. Stair and summit torches removed: the pool and the shaft light the scene.
- Treasure hall: pointed coin heaps on a 12 m grid; two reflecting pools flank the causeway (coins and
  jewels on the bottom, carved coping).
- `engine_worldgen::gilded::{glow_pools, light_shaft}` export the geometry; `portals::realm::gilded_rig`
  packs it with colours into `LightingState::realm` (words 0..10).
- `shaders/realm_light.wgsl`: glow-pool emission (bed through clear water lit by a caustic web,
  luminous column, surface filaments), caustics thrown on walls, ceilings and submerged faces, a
  sky-free fill (underground the sky ambient is zero), red cavern and blue grotto air, the volumetric
  shaft and its spot, and the submerged view. Caustics snap to the shared texel lattice.
- GrottoStone palette moved from teal to cobalt and indigo.
- Captures: `target/reviews/gilded-water-20261114/v1..v4`; views `lake`, `pool`, `underwater`,
  `shaft`, `hoardpool` added to `gilded:<view>`.

## Per-seed plan (2026-11-14)

Owner request: no Gilded Cave should feel like the same copied location. The procession stays fixed
(landing, gallery, coin cavern, grotto); `gilded::Plan`, made from the world seed with a full
avalanche mix (`hash3` on a raw seed gave near-identical caves for seeds 11, 22, 33), picks:

- Grotto: radius 22-29 m, height 36-45 m, two or three outline lobes (faded at the passage mouth).
- Pillar: off-centre axis; boulder stack or rounded-square monolith with strata ledges.
- Stair: hugging spiral (1.25-1.75 turns) or a flying sweep (0.75-0.95 turns at 7-8.5 m) over open
  water, hung with calcite drips, rising from a landing rock and ending in a bridge to the summit.
- Up to three rock islets with stalagmite spires, clear of the stair and the stone path; the path
  bends per seed.
- Coin cavern: heap grid 10-14 m, density and height; twin channels or one round basin crossed on a
  dyke; a free-standing portal (stepped jambs, lapis band, marble reveal, gilded frieze, cornice) at a
  per-seed place and size on the causeway.

The light rig and review poses read the plan: `glow_pools(seed)`, `light_shaft(seed)`,
`review_view(seed, name)`; `realm_lighting` takes the seed. Review receipts print `[gilded-plan]`.
New view `door`. Evidence: `target/reviews/gilded-water-20261114/v5..v9` (contact sheets v5, v6, v7).

## Editable layout (2026-11-14)

Owner request: every room and structure customisable from the dev tools. `gilded::Layout` (77
fields, schema `LAYOUT_FIELDS`, declared once by the `layout!` macro) replaces the room constants:
gallery, coin cavern, passage, grotto and light rig colours (packed sRGB). The room chain (gallery
end, cavern centre, passage, grotto) is derived from the lengths. `default_layout(seed)` is the
per-seed default; `set_layout_override(seed, ..)` holds a dev edit or a saved `gilded_layout.ron`
sidecar (loaded with the world). New morphology: banded sandstone ledges in the cavern walls and
pillars, slumped heap skirts with spill ripples, beaded stalactites and stalagmites, rimstone
terraces along the grotto shore; wall caustics streak upward from the water.
Editor: Playtest > Realm layout editor (`tools/axiom-dev/layout-editor.mjs`), engine op `layout`.
End-to-end check: `target/reviews/gilded-water-20261114/editor-e2e`.
