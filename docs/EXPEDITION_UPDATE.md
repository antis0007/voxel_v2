# Inland expedition update — generator 3

Historical generator-3 baseline. See [Machines and expedition rewards](FACTORY_UPDATE.md)
for the current crafting controls, 1:1 manual smelting, automated ore doubling,
generator-4 sites and VXWSAVE6 persistence. The route below records the original update.

Create a new **Continents & biomes** or **Highlands & peaks** world. Existing
worlds retain their generator version and saved position. Flat building worlds
stay flat and do not contain generated expedition sites.

## Playable route

1. Spawn in a protected inland clearing. The workshop is roughly 42 metres north
   (straight ahead with the initial camera orientation).
2. Aim at the sealed cache and press **F**. It supplies copper, tin, iron and coal.
3. Press **F** at the ore processor. Each batch consumes one of each available ore
   and 0.25 coal per ore, producing two ingots per ore. Processing is manual.
4. Follow the workshop's central mine stairs into Lanternroot caverns. Jump up
   metre-height steps when returning in the classic block-size world.
5. Harvest luminous mushrooms with **F** or the normal break action. The first
   harvest unlocks a persistent cyan lantern. Further harvests supply lumen fibre.
6. At the workbench, **F** assembles a bronze bore for 2 copper ingots, 1 tin ingot
   and 2 lumen fibre. Aim at stone or ore and press **F** to excavate connected
   material. An excavation consumes 0.125 fibre.
7. Collect the cavern's resonant crystals. Upgrade at the workbench with 4 iron
   ingots, 2 crystal and 2 fibre for a larger excavation radius and cell budget.

**E → Field journal** shows the objective, resources, equipment and recipes.
Normal creative building and its palette remain available. This is an optional
expedition progression loop within that sandbox, not a survival inventory economy.
F interactions have a six-metre reach and only fire on a fresh key press.

## Generation and presentation

- Terrain reserves a broad inland area, with rolling surroundings and a small
  flat clearing. The clearing has stable ground and no generated obstacles or
  cave openings through its support. Generator 3 no longer relies on the old
  ocean-platform fallback for natural-world spawning.
- Seeded domain warping, folded mountain ridges and climate-dependent terracing
  extend the existing regional erosion generator. This does not implement the
  planned complete watershed/river network.
- Regional sites choose a workshop mine, watchtower or waterworks, with varying
  dimensions, orientation, damaged walls and terrain foundations. The initial
  workshop and its descent are deliberately guaranteed.
- Fungal chambers contain mycelium, physically sized mushroom stems/caps,
  emissive luminous mushrooms, limestone, accessible ore and crystal deposits.
  Additional underground chambers are joined by curved galleries where both
  endpoint chambers exist and the terrain has sufficient cover.
- Copper and tin veins join coal and iron. New block patterns distinguish fungi,
  crystals, ore, caches and stations. The discovered lantern casts actual local
  light using the existing point-light renderer.
- Cache rewards and mined resources use 1/64 cubic-metre units; changing voxel
  size does not multiply material yield. Machine/cache bodies remain one metre.
- Supplies, equipment and lantern discovery use the VXWSAVE4 manifest alongside
  the existing region storage. VXWSAVE1–3 remain accepted. Generator versions
  1–2 are retained for legacy worlds; no existing save is automatically migrated.

## Implementation and verification

World features: `src/world/landscape.rs`, `src/world/sites.rs`, `src/world.rs`.
Progression: `src/world/expedition.rs`; persistence: `src/world/storage.rs`.
Integration: input/HUD, material definitions and world material shader.

`cargo build --offline --target-dir target` passed, including composed WGSL
validation. No tests or game launch were performed, in accordance with the
workspace instructions. Runtime movement, frame time, presentation and save/load
round trips still need hands-on verification. The bore has a cell budget and a
6 ms between-cell cutoff; first-time chunk materialization can exceed that cutoff.

This update implements the first exploration/material-processing loop. It does
not implement autonomous factories, pipe networks, multiblock smelteries, enemies
or programmable spells. Inspiration comes from modular equipment in
[Tinkers' Construct](https://modrinth.com/mod/tinkers-construct), ore processing in
[Mekanism](https://wiki.aidancbrady.com/wiki/Getting_Started), and the exploration
and material focus of [Noita](https://noitagame.com/).
