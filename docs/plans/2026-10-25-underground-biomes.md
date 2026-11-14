# Underground biomes (generator 55)

Owner request (2026-10-25): the underground must beat the vanilla cave update,
taking heavy, specific inspiration from Noita's biomes and modded Minecraft
(Alex's Caves, YUNG's Better Caves, Terralith's underground, geodes, flooded
caves), with 3D volumetric biomes. Mushrooms in particular still look bad.

## What exists (2026-10-25 survey)

- Natural caves are one function, `lib.rs::cave_carves`: four threshold noises
  (spaghetti, cheese, pillar, ravine) that only ever produce Air, plus a wet
  floor below the water table. No decoration, no pools, no lava, no biomes.
- `sites::cavern` adds rare 72 m chambers (four variants, `fungus()` mushrooms
  that are a straight stem under a lens of caps), and ore geology adds seams,
  stocks and sealed geodes.
- `zones.rs` already names eight underground archetypes in 52 m layers (Ember
  Deep, Drowned Reach, Prism Vault, Sporefall, Hushed Ice, Hollow Dark, Static
  Hall, Gilded Hush) with entry lines, colour grading and spell effects, but
  the rock is the same plain strata whichever archetype you are in.
- The world floor is -64 m (bedrock 2-6 m), so under plains there are about
  60-80 m of rock below the shallow cave band.
- Only torches and AmberCap are light sources.

## Intent

Make the archetypes real places. Going down should feel like Noita: each
region has its own rock, its own cave shape, its own liquids and its own set
pieces, and you know where you are from one screenshot. Horizontally the
underground varies too (Alex's Caves style volumes), so a world has every
archetype somewhere, not one per layer.

## Goals

1. A 3D biome field below 14 m: warped jittered cells about 80 m across and
   36 m tall, each one of the eight archetypes or plain rock, weighted by depth
   (Ember, Void and Gilded deeper; Rot, Flooded, Frost, Crystal, Storm higher).
   Boundaries blend over a few metres of mixed rock, never a straight seam.
2. Per-biome cave shape, not just per-biome paint:
   - Sporefall (Noita fungal caverns, Terralith fungal caves): wide domed
     caverns with stout rock pillars, mycelium floors, mud wallows.
   - Drowned Reach (YUNG flooded caves, Noita underground lake): low wide
     caverns filled to a level with water, clay and gravel beds.
   - Prism Vault (geodes, Noita crystal caves): round halls with crystal
     linings and spikes from every surface.
   - Hushed Ice (Noita snowy depths, Terralith ice caves): tall fissures, ice
     walls, icicles, frozen pools.
   - Ember Deep (Noita lava lake, Terralith thermal caves): lava tubes and a
     lava lake at the floor of each big cavern, basalt columns, ash floors.
   - Hollow Dark (deep dark, Alex's Forlorn Hollows): huge dark voids and
     vertical chasms of void stone, almost no light.
   - Static Hall (Alex's Magnetic Caves): tall spires and arches, rich iron
     and copper veins, star glass threads along the ceilings.
   - Gilded Hush (Noita temple of the art): smoothed halls with carved stone
     and glazed tile ruins set into the walls.
3. Set pieces anchored to real surfaces: stalactites and stalagmites
   (dripstone), icicles, crystal clusters, basalt column fields, and giant
   mushrooms with real shapes (bent stems, umbonate or flat caps, wavy rims,
   spotted tops, glowing gills, clusters of small ones round a large one).
4. Liquids contained: each flooded or ember cell has a level; liquid fills
   only carved cells below it, and the cell's rim stays solid below the level
   so a lake never stands as a wall against open cave.
5. Mushrooms everywhere get the new shape from 55: deep fungal caverns, the
   `sites::fungus` chambers, and the Undercrown groves.

## Non-goals (later phases)

- Deepening the world below -64 m (streaming, bedrock, physics and save
  checks; its own design).
- New blocks (hanging roots, snow, sulfur, dripstone, obsidian) and emissive
  GlowCap; both touch the content table and the renderer's light list.
- Zone grading that follows the 3D biome (an app change in `main.rs`).
- New liquids (acid, toxic sludge, oil).

## Anti-goals

- No straight seams, flat-topped noise caverns, or features floating in air
  or sunk in rock (the mushroom bug, again).
- No change to any generator before 55 (pin and golden tests).
- No surface breach: everything new is 14 m or more below the surface.
- No gold or storage blocks as free loot in walls.
- Chunk generation must not get much slower (bench before and after).

## Shape of the code

`crates/worldgen/src/deep/`, each file under 300 lines:

- `mod.rs`: `VERSION = 55`; `block(cfg, p, column) -> Option<BlockId>` used
  by `sample_block_with` below the shallow band; the shared carve test.
- `biome.rs`: the 3D cell field and per-biome parameters (palette, carve
  shape, liquid, feature set).
- `carve.rs`: density per biome built from the existing noise families.
- `dress.rs`: rock palette, floor/wall/ceiling skins, liquids.
- `features.rs`: anchored set pieces (anchor grid, cached surface search).
- `fungi.rs`: the mushroom shape shared with sites and the Undercrown.

Tests: determinism, old versions unchanged (`gen_golden` digests for 54 and
below), every biome appears within 600 m of spawn on several seeds, no
feature voxel without support, liquids contained, nothing new above 14 m
depth. Captures: one pose per biome at 0.5 m, 54 vs 55.
