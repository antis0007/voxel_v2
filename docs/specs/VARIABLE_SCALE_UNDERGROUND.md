# VARIABLE SCALE + UNDERGROUND — performance architecture and dungeon generation

This closes two gaps: (1) how Minecraft-style 1 m block gameplay and Noita-style sub-block matter coexist in one world without the performance collapse of simulating everything at fine scale; (2) how the underground becomes the main event — strata, caves, and Noita-quality dungeons in 3D, assembled from authored modules and reacting materials.

---

## PART 1 — Variable scale that is actually performant

### 1.1 The three resolutions and what lives at each

| Level | Cell | Role | Where it exists |
|---|---|---|---|
| **Block** | 1 m | The build grid. Placement, mining, machines, structures, inventories, nav, most rendering. The Minecraft layer. | Everywhere resident |
| **Fine** | 0.25 m (4³ = 64 per block) | Matter detail: sand grains, water, fire, gas, acid, blood, debris, cracks, partial blocks. The Noita layer. | Only inside **disturbed** blocks within radius `R_fine` (default 24 m) of a player or an active hazard |
| **Coarse** | 2/4/8/16 m | Far field: terrain, water columns, treelines, smoke plumes, biome exports. | Beyond `R_block` (default 96–256 m by profile) |

Key rule: **a block is a uniform group of 64 fine cells until something makes it non-uniform.** A stone block is one 8-byte record. It becomes 64 fine records only when acid eats a corner, a pick chips it, water flows into it, or it is placed as a slab/stair (shape = fine-cell mask). When the fine cells return to uniform (water drains, debris settles into a full block, player repairs), it collapses back to one record. Expected fine ratio in play: 2–8% of resident blocks; steady-state < 1%.

### 1.2 Storage that makes this cheap
- Brick = 8³ **blocks** (not fine cells). Header carries `fine_mask: u512` (one bit per block: has fine sub-brick) and `dirty: u64` as before.
- Fine sub-bricks live in a separate pool (4³ fine cells × 8 B = 512 B each), indexed from the block's record (`fine_slot: u32` replaces the velocity/aux fields of a uniform record).
- CA kernels run at **block scale** for uniform blocks (powder/liquid/gas rules on 1 m cells — this is the Terraria/Minecraft-liquid tier and covers 90% of visible motion cheaply) and at **fine scale** inside fine sub-bricks (Noita tier). The boundary between a fine sub-brick and a uniform neighbour is handled by treating the neighbour as 64 identical fine cells during the fine pass (no allocation), and writing back only if any of its cells changed (which triggers promotion).
- Promotion/demotion is a budgeted job (≤ 2000 blocks/tick) with hysteresis (stay fine for 60 ticks after last change). Conservation: a uniform block of water volume 15 → 64 fine cells of 15; demotion averages and rounds by the ADR'd rule (remainder carried to a neighbour, never destroyed).
- Memory on H1: 20k bricks × 8³ blocks × 8 B = 80 MB blocks + fine pool cap 64 MB (131k fine sub-bricks ≈ 2k fully-fine blocks of 64 — enough for several large Noita-style events at once).

### 1.3 Gameplay semantics across scales
- **Mining**: a pick removes a block in N hits (block scale) but each hit chips fine cells from the face (visible damage, debris cells). Explosions and acid work at fine scale and leave jagged fine-cell geometry; placing a block on a jagged surface fills to the block grid (building stays clean).
- **Partial blocks** (slabs, stairs, pillars, arches, carved stone, eroded edges) are fine-cell masks with a template id; rendering uses the template mesh, not 64 quads, unless the mask deviates from the template (then greedy-mesh the fine cells).
- **Liquids**: at block scale a water block holds volume 0–15 in 1 m; at fine scale the 64 cells carry it. Pressure columns work at both scales (pressure is per block; fine cells share the block's pressure).
- **Fire/gas**: block scale for spread between blocks; fine scale for flames licking surfaces, smoke curling, gas pockets in cracks.
- **Mobs/bodies**: collide against the block grid plus fine masks within `R_fine` (capsule sweep samples fine cells only inside fine sub-bricks).
- **Far field**: coarse cells carry material fractions and flow vectors; rivers, lava flows, smoke, and treelines render from them.

### 1.4 Per-tick budgets that make 60 tps on H1 credible
| Stage | Budget (H1) | Mechanism |
|---|---|---|
| Block-scale CA | ≤ 4.0 ms | awake bricks only; typical 300–1500 awake |
| Fine-scale CA | ≤ 3.0 ms | ≤ 2000 fine sub-bricks awake |
| Promotion/demotion | ≤ 0.5 ms | 2000 blocks |
| Thermal/light/support | ≤ 1.5 ms | dirty bricks, divisors 2/4/4 |
| Bodies + mobs + machines | ≤ 3.0 ms | caps 128/300/1500 on H1 |
| Mesh dirty bricks | ≤ 2.0 ms | GPU; fine masks via templates |
| Readback/compact/misc | ≤ 1.0 ms | |
Sum ≤ 15 ms, leaving headroom. Governor degrades `R_fine` first (24 → 16 → 12 m), then block CA radius, never the player's own block edits.

### 1.5 Modded-feature consequences
Machines, belts, pipes, cables, and multiblocks live at block scale (they are blocks with records; conduits are block cells with transport flags). Their *effects* reach fine scale through planes (heat, charge, gas). A reactor meltdown promotes its surroundings to fine (corium eats floors at 0.25 m granularity). Nothing in the machine layer needs to know fine cells exist. This is the line that keeps 2k machines cheap.

---

## PART 2 — The underground as the main event

### 2.1 Strata (from lithology stage) → cave families → biome processes
Depth bands with their own rock, caves, hazards, resources, and dungeon families. Surface biome predicts the first band (karst upland → sinkholes and flooded tunnels; volcanic field → lava tubes). Below ~200 m the civilizations' strata take over (Engineers → Choristers → Founders), each with its ruins as dungeon families.

| Band | Depth | Rock / caves | Hazards (all physical) | Resources | Dungeon family |
|---|---|---|---|---|---|
| Topsoil/karst | 0–60 | soil, limestone; sinkholes, cenotes, dry caves | flooding, collapse of thin roofs | clay, limestone, copper | animal dens, bandit camps |
| Fractured | 60–250 | sandstone/shale; spaghetti + ravines, aquifers | pressurized water behind walls, gas pockets (CO₂ sinks into pits) | iron, coal, salt | Engineer mineshafts, rail stubs |
| Engineer | 250–600 | granite; cheese caverns, flooded halls | rust-gas, unstable brick vaults, live machinery | steel salvage, components | Engineer city, foundry, dam works |
| Deep fractured | 600–1000 | basalt/marble; lava tubes, crystal geodes, fungal hollows | methane, lava, acid pools, spore clouds | gold, crystal, oil | fungal lairs, geode shrines |
| Chorister | 1000–1500 | resonance stone; cathedral caves, shattered ravines | resonance (vibration plane) shaking loose rock, golems | mana crystal, silver | cathedrals, choir-canyons, golem forges |
| The Deep | 1500–2500 | dark stone; vast voids, bioluminescent chasms | darkness (light plane 0), cold, eldritch spore, pressure | exotic ores, Founder shards | archives, sealed vaults, matter compiler halls |
| Mantle | > 2500 | hot rock, lava ocean | heat, pressure, gas | Founder materials | the core approach |

### 2.2 Cave network generation (G2→G3→bricks)
- **Network skeleton** per band: 3D graph grown from surface entrances and aquifer paths (karst follows water table and joint sets; lava tubes follow old flow paths; tectonic caves follow faults). Edges carry width profile, floor material, water presence, gas presence, and a "flow" direction (downhill for water, up for gas) that the hazard placement uses.
- **Chamber types** as SDF templates with parameters: tube, cheese cavern, ravine, dome, pit (vertical shaft — Terraria "hellevator" moments), geode (hollow crystal sphere), cathedral (ribbed vault), void (irregular 100 m+). Secondary detail: speleothems (stalactites, flowstone, columns), breakdown piles, fissures, chimneys to the surface (light shafts — a landmark), natural bridges.
- **Flooded and gassed sections** from the water table and gas sources; sumps (U-bends full of water) as gates; CO₂ pits in low spots; methane pockets in high spots (both physically consistent).
- **Verticality**: ensure every 200 m depth band has ≥ 2 vertical connections per km² (shaft, cascade, breakdown slope) so descent is navigable without digging; ≥ 1 is a "secret" connection (behind a waterfall, under a lake, through a resonance door).

### 2.3 Noita-quality dungeon assembly in 3D
Noita's trick: biome layouts are tiles of hand-authored pixel art with material semantics, stitched procedurally, then everything is simulated. We do the same at block scale with fine-scale materials inside.

- **Modules**: authored in-game in creative mode and saved as `assets/dungeons/<family>/<name>.ron` (schematic of blocks + fine masks, plus *semantic layers*: material zones, spawn zones, loot zones, hazard zones, connector ports, secret hooks, lore tags). Module size classes 8³, 16³, 32³ (blocks); ≥ 40 modules per family at launch (scaffold `just new-module` and a copy-from-world tool make authoring fast).
- **Assembly**: 3D Wang-tile / socket matching on connector ports (door, hall, shaft, pipe, canal, rail, void) with family-specific grammars (an Engineer city grammar: avenue → blocks of foundry/housing/powerhouse; a cathedral grammar: nave → transepts → crypt). Place along the cave network at the band's sites; connect to natural caves via "breach" modules (collapsed wall into a cavern). Rotations/mirrors allowed where the family permits.
- **Material semantics make it Noita**: modules declare *material zones*, not fixed blocks — "weak brick", "oil-soaked timber", "acid-stained pipe", "pressurized tank", "gas-filled vault", "load-bearing pillar" — so the sim does the rest: a stray spark burns the timber hall, shooting the tank floods the corridor with acid, breaking a pillar drops the floor above, a gas vault explodes when a lamp mob walks in. Every room is a reaction waiting to happen; the validator requires ≥ 3 reactive zones per module.
- **Living dungeons**: Engineer modules can be *powered* (restore a dynamo → lights, elevators, doors, conveyors, and turrets come on; also wakes defenders); Chorister modules respond to resonance (chords open doors, wrong notes shake rock loose); Founder modules run on mana/Founder light and reconfigure (matter compilers rebuild walls).
- **Lairs and ecology**: fauna families per band with nests placed by the mob system (bats in chimneys, fungal crawlers in hollows, golems in forges, deep things in voids); predators and prey; cave rivers with blind fish; the scent plane carries hints.
- **Waystations (the Holy Mountain analogue)**: between bands, an Archivist waystation: safe room, archive (deposit knowledge/items — C-S4), trader, a Primer chapter, a bed. One per band per km², found at the band boundary; leaving it seals it until the next is found (optional, delve mode).
- **Secrets** in modules: secret hooks are satisfied by the secrets generator (a mural that references a real condition elsewhere; a door that needs the chord; a chest under the sump). Hint density: ≥ 1 hint object per 3 modules.
- **Loot**: tables per family and depth; items and materials, not just gold: Engineer salvage, Chorister wands/cards, Founder shards; rare rule-bearing materials (new reaction rows — "a new verb").
- **Difficulty**: by depth, by band hazard density, by lighting; the delve run mode layers permadeath on top.

### 2.4 Lighting and atmosphere underground
Light plane 0 in the Deep; player lights are real sources with fuel; luminous materials (crystal, fungi, lava, Founder light) drive both the render and growth/spawn rules; fog from cold air pooling; dust after collapses; echoes from nav-summary cave size; drips/wind from hydrology.

---

## PART 3 — Performance of the underground specifically
- Caves are mostly uniform air and uniform rock → bricks stay uniform; only surfaces with water/gas/lava are awake. Typical awake count in a big cavern: a few hundred bricks (water surfaces, falls, gas pockets).
- Dungeons are block-scale schematics; fine scale only where fights/accidents happen.
- Far caves are not simulated; their state is the catch-up record (gas level, water level, machinery powered?) and is re-realized on approach.
- Mobs underground use nav summaries; vertical connections precomputed into the coarse nav graph at generation.
- Streaming: cave chunks compress well (LZ4 of uniform runs); page cache favours the vertical column under the player.

---

## PART 4 — Acceptance
- `scale_promote_demote`: a 10 m acid spill promotes, eats, settles, demotes; mass conserved; fine ratio returns < 1% within 2 minutes.
- `mine_face_chip`: pick hits chip fine cells; block removal on Nth hit; placing on jagged surface fills to grid.
- `fine_budget_h1`: three simultaneous fine events (fire, flood, explosion) within `R_fine` at ≤ 3 ms fine CA on H1 emulation.
- `cave_vertical_connectivity`: every band reachable without digging on 100 seeds.
- `dungeon_reactive_rooms`: in 50 assembled Engineer modules, a single spark or breach causes a chain reaction in ≥ 80%.
- `dungeon_power_restore`: restoring a dynamo powers lights/doors/turrets in the module graph.
- `waystation_per_band`: found within 300 m of each band boundary on 100 seeds.
- `secret_hint_density`: ≥ 1 hint per 3 modules; each secret has a satisfiable chain.
- `underground_awake_count`: big cavern scenario keeps awake bricks < 800 at rest.
- Capture board per band: lighting, atmosphere, and one reaction each; human review.
