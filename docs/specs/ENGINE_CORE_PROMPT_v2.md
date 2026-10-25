# MISSION SPECIFICATION v2 — Definitive voxel engine, physics, gameplay, magic, and technology

You are the principal engineer for a Rust + WGPU variable-scale voxel game. This document is the complete specification for the next 12–18 months of engine and gameplay work. Your first task is to turn it into `docs/ENGINE_CORE.md` (engine) and `docs/GAME_DESIGN.md` (gameplay), each with exact file lists, data layouts, algorithms, acceptance tests, bench targets, and a session schedule, then stop for approval. Nothing is implemented before approval.

Read first: `CLAUDE.md`, `docs/ARCHITECTURE.md`, `docs/PLAN2.md`, `docs/PERF.md`, `docs/WORLDGEN.md`, all ADRs. Standing decisions D1–D7 remain (32³ chunks, 8³ GPU bricks, integer-only deterministic state, `CaBackend` with 2-tick authoritative lag, no ECS crate, save versioning). Conflicts between this spec and code reality are resolved by an ADR, never silently.

Working rules for every session: plan mode first; one section-chunk per session; `/compact` with a focus note between; subagents for parallel kernels, research, and content; every change ships with a scenario, a bench number on the low-end profile, and a visual capture if it changes what the player sees; `just verify` green at every commit; LOC delta reported.

---

# PART I — PRODUCT VISION AND PILLARS

**One sentence.** A Minecraft-shaped world where every block is simulated matter (Noita in 3D), where technology and magic are two syntaxes over the same physics, where factories run at Factorio scale, and where the world can also be explored and manipulated through text like Zork.

**Pillars** (every feature must serve at least one; features that serve none are cut):
1. **Matter is real.** Sand falls, water pressurizes, gas ignites, metal conducts, rock collapses without support. No cosmetic blocks.
2. **Systems over scripts.** Machines, spells, creatures, weather, and story events are rules over fields and materials. Emergence is the content.
3. **Scale.** Thousands of machines, hundreds of bodies and creatures, half a million moving cells, on a laptop iGPU.
4. **Two syntaxes, one physics.** Tech path and magic path reach the same capabilities by different means and aesthetics; mixing them is the endgame.
5. **Legible.** Everything the player can do is discoverable in-game (recipe browser, material inspector, spell lab, text verbs). Education profile exposes real units and names.
6. **Agent-maintainable.** Content is data; code is small, documented, deterministic, and testable headless.

**Audience paths:** builder (Minecraft creative), engineer (Factorio/Create/Mekanism), mage (Noita/Thaumcraft/Botania), explorer (Terraria caves, Twilight Forest), survivor (vanilla survival), student (education profile), reader (text layer).

---

# PART II — REQUIRED RESEARCH (do before planning; cite in plan)

Write `docs/research/<topic>.md` (≤ 8 lines each: what we adopt, what we reject, why). Subagents in parallel.

**Engine**
- Petri Purho, "Exploring the Tech and Design of Noita" (GDC 2019): dirty rects, 4-pass checkerboard, pixels→Box2D bodies, body re-pixelization, chunk multithreading.
- Brickmap (van Schaik 2016), ESVO (Laine & Karras 2010), "GigaVoxels": GPU sparse storage, why octrees are bad for mutable data.
- Toffoli & Margolus, *Cellular Automata Machines*: block (Margolus) CA, conservation, reversibility.
- Jon Gallant "Fluid Flow for the Rest of Us"; Terraria/Minecraft liquid levels; cellular compression models.
- Teardown (Gustafsson blog + GDC 2022/23): destruction, body sleep, debris budgets, voxel ray marching.
- Mikola Lysenko greedy meshing; binary greedy meshing (Tantan/Ethan Gore); visibility buffers; GPU-driven rendering (Haar & Aaltonen, "GPU-Driven Rendering Pipelines", SIGGRAPH 2015).
- HPA* (Botea 2004); Recast navmesh; Dwarf Fortress connectivity map; Minecraft goal-selector AI.
- Factorio FFF #176 (belt optimization), #204/#317 (fluids), #250 (update order), #367 (fluid rewrite), circuit network FFFs; Factorio deterministic multiplayer.
- "1500 Archers on a 28.8" (AoE lockstep).
- Minecraft 1.18 worldgen (noise routers: continentalness, erosion, peaks/valleys, weirdness), Terralith, Tectonic for terrain shapes.
- Dave Mark, *Behavioral Mathematics for Game AI* (utility AI); The Sims needs; Rain World creature talk.

**Gameplay/mods** (play or read wiki + design posts; produce feature lists)
- Vanilla Minecraft: survival loop, redstone, enchanting, villages/trading, Nether/End, brewing, advancements, structures.
- Create (kinetic machines, contraptions), Mekanism, Thermal Series, Immersive Engineering, IndustrialCraft 2, GregTech (processing chains, voltage tiers), Ender IO, PneumaticCraft, Applied Energistics 2 / Refined Storage (digital storage), Integrated Dynamics, ComputerCraft/OpenComputers.
- Botania, Thaumcraft 4/6, Ars Nouveau/Ars Magica, Blood Magic, Astral Sorcery, Electroblob's Wizardry, Mana and Artifice, Occultism, Eidolon, Hexcasting.
- Tinkers' Construct (tool building), Silent Gear, Apotheosis.
- Biomes O' Plenty, Terralith, Twilight Forest, The Aether, Dimensional Doors, Cave Dweller/Alex's Caves, Deeper and Darker.
- Farmer's Delight, Pam's HarvestCraft, Mystical Agriculture, Dynamic Trees, Serene Seasons.
- Storage Drawers, Sophisticated Backpacks/Storage, Iron Chests, Waystones, JEI/EMI, JourneyMap, Xaero's, Chisel, Carpenter's Blocks, Framed Blocks, MineColonies, Millénaire.
- Noita (wands, perks, alchemy, secrets), Terraria (progression gates, bosses, biomes), Dwarf Fortress (legends, z-levels), Zork/Infocom (parser design, room descriptions).

Deliverable of this section: `docs/research/MOD_FEATURE_MATRIX.md` — a table of ~150 features across these sources with columns: feature, source, how it maps to our physics/data model, cost estimate (data-only / small code / new system), target milestone.

---

# PART III — ENGINE ARCHITECTURE

## III.1 Invariants (encoded as types, lints, or tests — not advice)

1. **World state** = GPU-resident 8³ bricks + CPU entity tables. CPU brick mirror only for: authoritative headless backend, player's prediction region (radius R bricks), streamed-out regions.
2. **Integer-only simulation state.** Positions `i32` cell + `u8` sub-cell (1/256). Velocities `i16` (1/256 cell/tick). Temperature `u16` (K×4). Randomness `hash64(seed, tick, x, y, z, phase)`; hash version pinned with a golden test. Lint: no `f32`/`f64` in `sim::state` and `ca::*`.
3. **Tick order** is one `const` array, tested: `ingest_actions → predict_local → ca_phase[0..8] → thermal → support → bodies_integrate → bodies_contacts → mobs_sense → mobs_think → mobs_move → machines_bucketed → networks → rules_eval → fuse_settled_bodies → weather_tick → dirty_compact → mesh_dispatch → audio_events → render`.
4. **Budgets.** Every system: `Budget { max_items, max_micros }` from `assets/engine/budgets.ron`, per profile; reports used/allotted. Over budget → degrade by Chebyshev distance from player in bricks; never drop player actions.
5. **Sleep default.** Brick awake iff `dirty != 0`. Steady state in a static world: 0 CA dispatches, 0 remesh, 0 body updates, mobs at idle rate. Awake counts are on the overlay and in every bench.
6. **One mutation path.** `Action` enum (versioned, `#[non_exhaustive]`) is the sole writer to world state. Player, mobs, machines, spells, worldgen stamps, text verbs, scenarios all emit `Action`s. Per-tick action log → replay → desync detection.
7. **Data over code.** Materials, reactions, items, machines, recipes, tech, mobs, spells, rituals, structures, biomes, loot, dialogue, budgets: RON. Lint: no `match` on material/item ids outside `content/`.
8. **CPU reference ≡ GPU kernel**, bit-exact, `just test-gpu` compares 200 random bricks per kernel.
9. **Files ≤ 300 lines, functions ≤ 60, nesting ≤ 3.** Public surface per crate ≤ 40 items (test counts them).

## III.2 Core data layout

### Cell (8 bytes, two u32 SoA planes per brick)
```
Plane A: material:12 | state:2 (solid/powder/liquid/gas) | volume:4 (0–15) | flags:6 (awake, burning, supported, charged, wet, reserved) | temp_lo:8
Plane B: vel_x:i8 | vel_y:i8 | vel_z:i8 | aux:u8  (fuel / oxygen / item progress / signal / growth, by material class)
Optional field planes (u8 each, allocated per brick on demand): temp_hi, pressure, mana, oxygen, scent, light_sky, light_block, item_id(2 planes), moisture
```
Brick = 512 cells → 4 KB + 512 B per field plane. Uniform bricks (all one material, no fields) are a 4-byte constant in the grid, allocated on first edit. Expected residency on low-end: 20k allocated bricks ≈ 80–120 MB.

### Brick header (32 bytes, GPU array)
```
dirty: u64 (one bit per 2³ Margolus block) | flags: u32 (field bitset:10, uniform, has_bodies, nav_dirty, mesh_dirty, light_dirty, network_dirty, weather_exposed) | support_min: u8 | awake_ticks: u16 | lod: u8 | mesh_slot: u32 | field_base: u32 | light_base: u32
```

### Top level
- Chunk grid: hashmap chunk(32³)→64 brick ids; resident ring radius `view_distance` with hysteresis; chunk paging to disk LZ4.
- Brick pool: single GPU buffer, free-list, background defrag ≤ 1 ms/tick.
- `awake_list`: GPU compaction (prefix sum) each tick → indirect dispatch for every kernel. No CPU readback.
- Variable scale: `lod` reserved; far terrain stored as coarse bricks (each cell = 2³/4³/8³ world cells) with the same format; CA runs at coarse resolution with scaled rates; promotion/demotion on player approach with conservation (ADR for volume rounding rule).

### Entity tables (CPU, `sim::tables`)
Generational `Vec` columns; tables: `bodies`, `mobs`, `machines`, `projectiles`, `items_free`, `players`, `vehicles`, `npcs`. Counting-sort by brick id per tick for locality (budgeted). Chunk-local spatial hash for broadphase.

## III.3 Cellular physics (Phase A; target 500k awake cells @ 60 tps low-end)

**Kernel.** Margolus 2×2×2, 8 phases/tick, one workgroup (64 threads) per awake brick per phase, each thread owns one block fully in registers; halo via 10³ shared tile. Rule order per block (const, tested): `reactions → phase_change → transport_cells → powder → liquid → gas → velocity_integrate → growth → sleep_check`. Phase order permuted per tick by hash to kill checkerboard artifacts. Dirty bits propagate to touched neighbours including across brick faces.

**Powder.** Per-cell velocity, gravity per material, sub-cell accumulator in `aux`, swap with lower-density cells, blocked → ⅔ velocity to lateral spread biased by existing velocity, angle of repose per material, shatter on impact above `shatter_speed` (reaction row). Fast cells (|vel|>256) get extra sub-step dispatches from a compacted fast list, budgeted. Materials: sand, gravel, snow, ash, dust, salt, sugar, grain, coal dust (explosive in air), gunpowder.

**Liquids.** Volume 0–15; hydrostatic pressure plane computed in phase 0 per column; face flow by pressure gradient, `flow_rate` cap; lateral spread ≤ 3 cells/tick when level; interior cells sleep; compression 16–19 under ceilings; viscosity, surface tension threshold, evaporation/freezing/boiling by temperature. Materials: water, salt water, oil, lava, acid, blood, honey, slime, mercury, liquid mana, molten metals (one per metal), alcohol, milk, tar, polluted water.

**Gases.** Density 1–15, buoyancy from density and temperature, diffusion, dissipation threshold, `gas_divisor` tick rate except near player. Oxygen with sky refill; CO₂ sinks; methane/hydrogen explode (pressure pulse reaction); steam condenses; smoke; toxic gas; spore clouds; mana vapour.

**Thermal.** Explicit diffusion kernel over bricks with temp plane; conductivity per material; sources: lava, fire, machines, sun exposure (weather), mobs; sinks: water, ice, night sky. Phase changes from `materials.ron` (melt/freeze/boil/condense/sublimate with latent heat as energy_delta).

**Reactions.** `assets/reactions.ron` compiled to GPU hash table keyed by (a,b) and (a,self) with rows: conditions bitmask (needs_oxygen, needs_water, needs_light, needs_mana, needs_pressure), temp min/max, products, energy_delta, probability u16, emit_event. Covers combustion, oxidation/rust, corrosion, dissolution, crystallization, fermentation, decay, photosynthesis (growth), electrolysis (with charge), transmutation (with mana).

**Fire.** Fuel in `aux`, ignition by temperature, consumes oxygen, produces heat+smoke+ash/char per material, spreads via heat and contact, wind bias from gas velocity, extinguished by water/sand/low oxygen. Emissive rendering from temperature.

**Support/collapse.** `support` u8 relaxation from ground (bedrock=255), per-material decay and `max_span`, recomputed per dirty brick (CPU local / GPU far). Below `collapse_threshold`: small clusters → powder; ≥ `body_min_cells` → rigid body. Cave-ins are thus physical: removing pillars matters.

**Growth/biology.** `growth` in `aux` for plants: needs light plane, moisture plane, temperature band; stages are materials; L-system/template stamps for trees (Dynamic-Trees-style incremental growth). Crops, mushrooms, moss, coral, vines, kelp.

**Light.** Sky and block light planes propagated by a BFS kernel on `light_dirty` bricks (flood from sources, budgeted); coloured emissives via render only. Light feeds growth and mob spawning and the text layer ("it is pitch black").

**Weather.** Per-chunk exposure; rain adds water cells to exposed surfaces and raises moisture; snow accumulates powder; wind as a global gas velocity bias per region; lightning = heat+charge impulse at random exposed conductors; seasons modulate temperature and growth (Serene Seasons).

**Acceptance.** Scenarios: `avalanche_100k`, `dam_break_200k`, `u_tube`, `fountain`, `gas_room_seal`, `methane_explosion`, `lava_water_obsidian`, `oil_fire_forest`, `ceiling_collapse`, `rain_flood_valley`, `crop_growth_season`, `rust_in_rain`. Gate: ≥ 60 tps with 500k awake cells on low-end; CPU≡GPU; conservation; captures.

## III.4 Rigid bodies (Phase B; 256 awake @ 60 tps)

Body = own brick-aligned grid ≤ 64³ (same planes) + state (pos i64×3, orient i16 quat, lin/ang vel, mass, inertia fixed-point, material summary, sleeping, owner). Creation: support loss, explosion carve, player/magic lift, machine detach (`Action::Detach{region}`), vehicles (boats, carts, airships are bodies with controllers). GPU flood fill ≤ 64³ window → copy → clear → dirty. Integration semi-implicit fixed-point; fluid drag/buoyancy from 8 probes; collision via GPU surface-cell sampling vs bricks and other bodies (AABB broadphase), contacts compacted and read back ≤ 4 KB/tick; sequential impulses 4 iters; friction by material pair. World interaction: displaces fluids/powder, damages below `toughness`, crushes mobs. Fracture: impact energy > `fracture_energy` → split on weakest axis-aligned plane; debris < `debris_min_cells` → cells. Sleep/re-fuse: snap to nearest of 24 cube rotations if angular error < 10° else sleeping body; write back; free. Caps by distance.

Also: **joints** (hinge, slider, fixed) for Create-style contraptions — doors, drawbridges, windmills, minecart-on-rail as constrained body; **ropes/chains** as point chains with distance constraints (budgeted, visual only unless load-bearing flag).

Acceptance: `boulder_rain_200`, `collapsing_tower`, `log_raft`, `glass_shatter`, `piston_launch`, `windmill_hinge`, `cart_on_rail`, `airship_lift`.

## III.5 Mobs (Phase C; 500 awake @ 60 tps)

`MobTable`: pos, vel, capsule, kind, health, needs[8], goal, path handle, brick id, lod tier, inventory handle, faction, status flags (burning, wet, poisoned, charged). Movement: capsule sweep vs CPU mirror (local) or 1-tick-stale snapshot (far); modes walk/climb/swim/fly/burrow (burrowers emit `Action::Dig`). Nav: per-brick `NavSummary` (walkable 512-bit, 6×portal bytes), coarse graph, HPA*, cached paths, jittered replans. Senses from planes: scent (gas plane emitted by mobs/food/blood), hearing (event ring), sight (GPU rays ≤ 4/mob/tick), vibration (support/body events). Behaviour: utility AI in RON (needs, considerations, actions emitting `Action`s); factions and reputation; herds/packs via shared blackboard; day/night schedules; taming/breeding/husbandry (Minecraft animals, Farmer's Delight); villagers with jobs, trades, homes, workstations (Minecolonies-lite); bosses as scripted utility sets + phases. Physics coupling via planes only. LOD tiers 0/1/2.

Acceptance: `mob_swarm_300`, `flood_evacuation`, `predator_prey_100`, `village_day_cycle`, `boss_arena`.

## III.6 Machinery, logistics, power (Phase D; 2k machines, 10k belts @ 60 tps)

- **Transport as cells:** belts (`Belt{dir, speed}`), item plane u16 + progress; splitters/mergers/filters as 2-cell block rules; inserters as 1-cell entities with reach; pipes = confined fluid cells with pumps adding pressure; pneumatic tubes (PneumaticCraft) = pressurized gas moving item cells; conveyors on slopes/vertical; minecart rails = body track.
- **Machines:** `MachineTable` with 8-slot flat inventories, energy/heat/mana buffers, recipe, progress, tick divisor buckets; multiblocks via RON shape matrix (Immersive Engineering style: blast furnace, coke oven, excavator, distillation tower, nuclear reactor); controller cell is the entity. Kinetic machines (Create): rotation networks with speed/stress solved as a network type; mechanical presses, mixers, saws, drills, fans, deployers; contraptions = bodies with joints.
- **Processing chains:** tiers (stone → bronze → iron → steel → aluminium/titanium → advanced alloys), ore processing yield multipliers (crush → wash → smelt, Mekanism-style 2×/3×/4× with increasing inputs), chemical processing (electrolysis, acid leaching) implemented as reactions with machine-supplied conditions.
- **Networks:** electric (graph per edit, supply/demand priority solve, voltage tiers GregTech-style with machine explosions on overvoltage), kinetic (speed/stress), heat/steam/mana (fields), signal (fixed evaluation order by cell id; gates, comparators, counters, timers, displays; programmable controller later via a tiny sandboxed bytecode VM like ComputerCraft, data-only programs).
- **Digital storage:** AE2/RS-style storage network as an entity graph with item index, crafting planner solving recipe graph, autocrafting jobs scheduled through machines — implemented in CPU tables, budgeted.
- **Generators:** burner, steam (boiler+turbine as reactions+kinetic), solar (light plane), wind (gas velocity), hydro (fluid flow through wheel), geothermal (temperature plane), nuclear (fission as reaction chain with neutron field, meltdown physics), mana engines (magic path).
- **Content:** items, machines, recipes, tech tree in RON; validator proves reachability and no energy-free cycles.

Acceptance: `megafactory`, `logic_counter`, `fluid_refinery`, `kinetic_sawmill`, `nuclear_meltdown`, `autocraft_100`.

## III.7 Magic (Phase E; 200 spells @ 60 tps) — detailed in Part V

Mana plane; spells as rule content with a fixed effect primitive set; projectiles table; wands as card sequences; rituals as multiblock + condition rules; enchantments as item modifiers evaluated by the same rule engine.

## III.8 Rendering

GPU binary greedy meshing per brick into pooled 8-byte quads; indirect draws per material class; GPU frustum + Hi-Z occlusion; cascaded shadows with per-cascade GPU culling; bodies instanced; volumes (liquid, gas, fire, mana) via ¼-res raymarch over a 3D texture from planes with emissive from temperature; water surface with screen-space reflections and refraction; sky with day/night, weather, volumetric clouds (coarse gas cells); coloured block light by material; particles as GPU instanced quads driven by events; per-material shading params (roughness, emissive, translucency) in RON; variable-scale LOD seams hidden by skirts; texture atlas or procedural materials (decide by bench and art direction). Dirty→mesh ≤ 1 frame local, ≤ 2 far. Target: 60 fps low-end at 12 chunks view distance; 144 fps desktop at 24.

## III.9 Audio

Event-driven (`audio_events` stage): material pairs → impact/scrape/break sounds; fluids → flow by volume delta; fire → crackle by burning count; ambient per biome/depth/light; occlusion by raycast against bricks (budgeted); reverb by cave size estimate (nav summary); dynamic music by threat/exploration state. Data in RON; `kira` or `rodio`, decided by ADR.

## III.10 Cross-cutting

- Action log, replay, desync tools; scenario scripts are action logs with asserts.
- Save v30: paged chunks, LZ4, entity tables, field pools, action-log tail; incremental autosave; `just save-roundtrip`.
- Observability overlay + `just session-report`.
- Profiles: `lowend`, `desktop`, `steamdeck`.
- Multiplayer-ready: lockstep deterministic sim with action exchange is the design; implement only replay now, network transport later (ADR).
- Modding: all content loadable from `mods/<name>/` overlays with the same validators; later a bytecode scripting layer (same VM as the in-game controller) for custom rules.
- Lints in `just verify`: no float in sim, no id matches outside content, no unwrap, file size, tick order, hash version, CPU≡GPU, conservation, replay, perf gate (3-run median, 8%).

---

# PART IV — GAMEPLAY DESIGN

## IV.1 Core loop
Explore → gather → craft/cast → build/automate → discover deeper layers/biomes/secrets → unlock tiers → repeat at larger scale. Survival needs (health, hunger, thirst, temperature, oxygen, sanity-lite via darkness/eldritch exposure) all driven by planes, each toggleable per profile.

## IV.2 Modes/profiles
- **Survival** (default): needs on, hostile mobs, death drops.
- **Creative**: infinite items, flight, no needs, instant break; building tools (brush, fill, copy/paste regions as bodies, symmetry, schematic placement).
- **Education**: real materials/units/names, lab tools (thermometer, barometer, scale, spectrometer), experiment scenarios, no magic (or magic labeled as fictional sandbox), guided lessons as text-layer scripts.
- **Fantasy**: default naming and magic on.
- **Adventure**: pre-made worlds with text-layer narratives, locked blocks.
- **Hardcore**, **Peaceful**, **Spectator**.

## IV.3 Player
Capsule mob with inventory (hotbar 9 + 27 + armour + accessories + backpack upgrades), tools with durability and material tiers (Tinkers-style modular: head/binding/handle materials affect speed, durability, damage, special traits like lava-proof), armour with material physics (metal conducts heat/electricity; wet cloth freezes), status effects from planes (burning, wet, frozen, poisoned, irradiated, charged, mana-sick). Movement: walk/sprint/crouch/swim/climb ladders+vines/glide (elytra-like wing item with real lift from body model)/mounts/vehicles. Block interaction: break progress by tool-vs-material hardness; place with physics (unsupported placement falls); carry small bodies; push large ones.

## IV.4 World structure and progression gates
- Surface biomes (worldgen mission) + depth tiers: Topsoil → Caves → Deep caves (aquifers, lava) → Crystal/fungal → The Deep (dark, eldritch, high-tier ores, ancient machines) → Mantle (lava ocean, heat-resistant tech/magic required). Depth gates by material hardness, temperature, gas, and light.
- Dimensions (as separate worlds with portal bodies): a Nether analogue (fire/pressure world, lava physics everywhere), a Sky/Aether analogue (floating islands, low gravity constant per world, wind), a Void/End analogue (low light, magic-dense, bosses), optional pocket dimensions (Dimensional Doors) as small worlds for bases. Each dimension = worldgen config + physics constants (gravity, ambient temp, oxygen, mana density).
- Structures: villages, temples, mineshafts, dungeons, strongholds, ancient factories (tech lore), wizard towers (magic lore), ruined labs (education lore), bosses' lairs. Loot tables in RON.
- Advancements/quests: RON graph; both as goals and as tutorialization; text-layer narrated.
- Bosses: physics-based (a golem of iron bodies, a hydra of liquid, a storm elemental of gas/charge) requiring environment manipulation to defeat.

## IV.5 Gameplay paths (each must reach "endgame capability" ≈ automated resource, flight, long-range transport, world shaping)

**A. Engineer path (Factorio/Create/Mekanism):** stone tools → burner machines → kinetic (water wheel, windmill, hand crank) → steam → electric (coal, then solar/wind/hydro) → chemical processing → nuclear → digital storage/autocraft → drones/logistics bots → rockets/airships. Capability milestones: auto-mining (drills, excavators), auto-farming, auto-smelting, item transport, power grid, teleport pads (end), terraforming machines.

**B. Mage path (Noita/Thaumcraft/Botania/Blood Magic):** mana gathering (leyline wells, mana flowers consuming light/growth, blood/soul sacrifice for dark branch, crystal resonance) → wands and spell cards → runes/glyph circuits (inscribed cells carrying mana signal; this is "magic redstone") → rituals (multiblock + condition) → golems/familiars (mobs with RON brains bound to player) → transmutation/alchemy (reaction rows gated by mana) → enchanting → astral/celestial (light plane and sky events) → portals and teleport. Capability milestones mirror A: golem mining, growth magic farming, transmutation smelting, mana pipes, rune networks, blink/portals, terraforming spells.

**C. Hybrid (endgame):** techno-magic: mana-electric converters, enchanted machines, spell-casting turrets, machine-cast rituals, automation of magic (Botania/Mekanism crossover). Reward: capabilities neither path reaches alone (stable portals, weather control, matter synthesis).

**D. Explorer/survivor:** maps, waystones, mounts, dungeons, bosses, trading, base building, cooking (Farmer's Delight depth), seasons.

**E. Student:** lesson scenarios (states of matter, density/buoyancy, pressure, heat transfer, combustion & oxygen, electricity basics, circuits, simple machines, ecology & food webs, geology & erosion, weather); lab tools; sandbox with real constants; graded experiments via scenario asserts; text layer explains.

**F. Reader:** text-layer playthrough with full verbs; narrative modules; LLM-driven NPC dialog hook (action API exposed).

## IV.6 Crafting and items
- Hand crafting (shaped/shapeless RON), workbench tiers, smelting (furnace as machine; heat plane makes campfires and lava work), cooking (temperature + time + vessels), alchemy/brewing (reactions in cauldrons), enchanting (rule modifiers), tool forging (Tinkers), smithing upgrades, repair.
- Item properties come from material: a bronze sword conducts, a wooden shield burns, a gold pickaxe is soft. Items are small bodies when dropped (physics-aware, float/sink by density).
- Recipe browser (JEI) in-game, searchable, shows uses/sources; material inspector shows planes.

## IV.7 Building
- Block variants via chisel/carpenter system: shapes (slab, stair, pillar, arch, beam) are data; framed blocks take any material texture. Decorative blocks are still matter (burn, conduct, collapse).
- Structural realism toggle (support model strength per profile).
- Schematics (copy region → body → paste), blueprints with material lists, auto-build by drones/golems.
- Lighting by colour, furniture with function (beds set spawn & skip night; chests; drawers; signs with text layer).

## IV.8 Farming, food, ecology
Crops with growth needs, soil moisture/fertility planes, seasons, animals breeding with needs, food chain for wild mobs, fishing (liquid volume + mob spawn), bees/pollination (Forestry-lite), composting (decay reaction), fertilizer.

## IV.9 Combat
Weapons by material/tool system; ranged (bows, crossbows, guns as tech, wands as magic) via projectile table; shields; armour; status effects; environmental combat is first-class (flood, burn, collapse, electrify, gas).

## IV.10 Transport
Rails + carts (bodies on tracks), boats (buoyant bodies), airships (gas-lift bodies), gliders, mounts, elevators (kinetic), teleport pads/waystones (late), portals.

## IV.11 UI/UX
Inventory, hotbar, recipe browser, material inspector, spell lab (wand editor), machine GUIs generated from RON schemas (slots, bars, buttons), map (chunk summary → PNG, JourneyMap-like with POIs), advancement tree, text console (toggle; shows world description, accepts verbs), accessibility (remap, colourblind palettes, text scale, reduced motion), controller support.

## IV.12 Text layer (Zork meets Noita)
World-query API over `sim`: nouns (materials/entities near), verbs = `Action`s; descriptions generated from planes (light, temperature, gases, sounds, moisture) and material tables; rooms = named regions from worldgen POIs; narrative modules as rule content; parser with synonyms and pronouns; LLM hook for NPC dialogue through the action API; the agent playtest harness.

---

# PART V — MAGIC SYSTEM (detailed)

**Substrate.** Mana plane (u8) with regeneration from sources (leylines, moon phase, sacred groves), decay, diffusion through `mana_conductive` materials (crystal, silver, living wood), storage in crystals (volume-like), and consumption by effects. Mana has "colour"/aspect encoded as a second plane only where needed (`aspect` u8: 8 aspects — fire, water, earth, air, light, dark, life, order) enabling Thaumcraft-style aspect gathering from materials.

**Casting.** Wands (Noita): item with slots, mana capacity, recharge, cast delay, spell cards in sequence; modifiers (speed, bounce, pierce, split, homing, delay, trigger, timer, area, chain, mana-cost, lifetime) and effects (primitive set: `HeatImpulse, ForceImpulse, Transmute(table), SpawnMaterial, SpawnBody, SpawnMob, SetField, Teleport, Shield, EmitEvent, Text`). Cards are RON; random wands generated from a grammar; wand editing at wand tables.

**Runes/glyphs.** Inscribed cells carry mana signal; rune blocks are gates/transformers (magic redstone): emit, amplify, delay, condition on aspect, convert to kinetic/heat/light. Rune circuits are solved in the network stage with the same deterministic ordering.

**Rituals.** Multiblock template + condition (time, moon, sacrifice item/material, mana ≥ X, aspects present) → effect sequence over ticks; failure modes (backfire reaction rows) make rituals risky.

**Alchemy/transmutation.** Reaction rows gated by `needs_mana` and aspect; cauldrons and alembics as machines; Noita-style secret recipes rolled per world seed from a grammar (discoverable via text/lab).

**Enchanting.** Item modifiers = rule attachments (on hit → effect, on break → effect, passive field). Costs aspects and mana.

**Constructs.** Golems/familiars: mobs with RON brains bound to a player; built from material bodies (clay, iron, crystal) whose properties matter.

**Schools (aesthetic groupings of the same primitives):** Pyromancy, Hydromancy, Geomancy, Aeromancy, Illumination, Umbral, Vitalism, Order/Ward. Dark branches (blood/soul) trade sanity/health for power; celestial branch uses light plane and sky events.

**Progression.** Discovery by experimentation (reaction and spell discovery logged in a codex), research via aspect scanning of materials (Thaumcraft), unlock nodes in RON tech graph, risk/corruption mechanics (mana sickness, flux pollution as a gas).

---

# PART VI — TECHNOLOGY SYSTEM (detailed)

**Tiers.** Primitive (stone, fire, hand tools) → Bronze/kinetic (water wheels, windmills, hand cranks, gears, belts, presses) → Iron/steam (boilers, engines, rails, coke, blast furnace) → Electric I (dynamos, coal power, wires, basic machines, batteries) → Chemical (electrolysis, acids, polymers, fertilizers, explosives) → Electric II (solar, wind farms, hydro, high-voltage, automation logic, digital storage) → Nuclear (reactors, waste, radiation gas/field) → Advanced (lasers, teleport pads, matter synthesis, airships, rockets to sky dimension).

**Mechanics mapped to physics.** Heat engines need real temperature deltas; steam needs water + heat + pressure; electricity needs conductors (metals), insulators, and grounding (lightning); chemistry is reaction rows; explosives are pressure pulses; radiation is a field that damages and mutates (reaction rows).

**Logistics.** Belts, pipes, tubes, inserters, trains with signals (rail blocks as network), drones (flying mobs with RON brains and inventories), digital storage with autocraft.

**Automation logic.** Signal cells, gates, comparators, displays, controller VM (bytecode, sandboxed, data programs; agents can write programs in RON).

**Safety/failure.** Overpressure explodes pipes; overvoltage burns machines; overheating melts; meltdown is a reaction cascade; fire spreads through wooden factories — danger is a design feature.

---

# PART VII — MINECRAFT + MOD FEATURE ADOPTION

Produce `MOD_FEATURE_MATRIX.md` in research. Minimum feature list to adopt (grouped by how they map):

**Data-only (reaction/material/recipe rows):** smelting, brewing/alchemy, cooking, composting, rust, obsidian from lava+water, concrete, dyes, glass from sand, charcoal, tanning, fermentation, crystal growth, mushroom/moss spread, coral, snow/ice physics, cactus damage, honey/slime stickiness, soul sand slowing (viscosity), magma heat, dripstone, sculk-like spread (dark-branch material), nether wart, bone meal (growth impulse).

**Small code + data:** beds/spawn, maps, waystones, signs/text, shields, bows/crossbows, elytra-like glider, boats, minecarts/rails, fishing, leads, saddles, banners, armour trims, trading, villager jobs, enchanting table, anvil, grindstone, loom, cartography, smithing, beacons (light-plane effect pillars), lightning rods (charge sink), tripwire/observer/pressure plates as signal cells, hoppers/droppers/dispensers as inserter-class machines, pistons (body detach/push), slime/honey blocks (sticky body groups), TNT (pressure pulse), redstone equivalents (signal cells), daylight sensors (light plane), note blocks/jukebox (audio events).

**New systems (planned above):** kinetic networks (Create), electric tiers (IC2/GregTech/Mekanism), chemical processing, digital storage (AE2/RS), computers (CC/OC), drones, trains with signals, colonies (Minecolonies-lite), tool building (Tinkers), dynamic trees, seasons, dimensions, bosses, aspects/runes/rituals (Thaumcraft/Botania/Blood Magic), wands (Noita/Ars), chisel/framed blocks, storage drawers/backpacks, recipe browser, minimap/world map.

**Explicitly better than Minecraft:** liquids with pressure and finite volume; fire that needs oxygen and spreads realistically; structural support; temperature everywhere; gases; rigid bodies and fracture; mobs that smell and hear; weather that wets and freezes; machines that overheat; magic that is physics; text layer; education profile; variable-scale distances (mountains that are mountains).

---

# PART VIII — OPTIMIZATION DESIGN (how we stay at 60 on an iGPU)

1. Cost ∝ change: awake lists everywhere, sleep after N ticks, uniform bricks free, dirty propagation exact.
2. GPU-resident state; CPU reads only compacted small results (contacts, events) once per tick, double-buffered with 1-tick lag.
3. Integer SoA planes; 8 bytes/cell; optional planes only where materials need them; LZ4 paging.
4. Margolus removes atomics and races; rules in registers; halo via shared memory; indirect dispatch from GPU-built lists.
5. Budgets with distance priority for every CPU system; counting-sort locality; bucketed machine updates by divisor.
6. Networks rebuilt incrementally and solved once per tick in deterministic order.
7. Rendering: GPU meshing into pooled buffers; indirect draws per material class; Hi-Z culling; ¼-res volumes; no per-chunk CPU work in steady state.
8. Variable scale: far bricks coarse, CA at scaled rates; promotion with conservation.
9. Profiles tune caps/budgets/view distance; low-end is the bench default; perf gate on p95.
10. Measure: GPU timestamps per pass, CPU spans per system, awake counts, bytes uploaded; `just session-report` every session.

---

# PART IX — CONTENT AND TOOLING

- Scaffolds: `just new-material|machine|recipe|spell|mob|structure|scenario|lesson` generate RON + test + docs stub.
- Validators: schema, cross-refs, reachability, energy-cycle, no-orphan textures/sounds, name uniqueness per profile (science vs fantasy names in a name table).
- Editors in-game (creative): material inspector, spell lab, machine GUI schema preview, structure stamp tool, scenario recorder (records actions to a scenario file with asserts).
- Art direction: decide procedural-material vs atlas by bench and an ADR; palette per biome; consistent icon generation from in-world render (`just icons`).
- Localization: all strings via a key table; text layer uses templates.

---

# PART X — TESTING STRATEGY

- Unit: brick layout, fixed point, hash, coords (proptest).
- Kernel equivalence: CPU≡GPU 200 random bricks per kernel.
- Scenario suite: every system's scenarios run headless in CI on the CPU backend; GPU scenarios nightly.
- Conservation: mass per material ± reaction deltas; energy per network; items per factory.
- Replay/desync on all scenarios.
- Golden world hashes per seed; golden images per render scenario (diff on failure only).
- Perf gate: p95 tps/fps per bench vs committed baseline, 3-run median, 8%.
- Mutation testing on `core`, `voxel`, `ca` with reported score.
- Playtest script: text-layer bot walks N metres, digs, fights, builds; asserts "≥1 notable event per 30 m" and no stuck/crash.

---

# PART XI — MILESTONES

- **M1 Foundation (sessions 1–6):** research, plan, brick layout, awake lists, CA powder/liquid/gas/thermal/reactions/fire/support, GPU mesh/indirect/volumes. Demo: avalanche, dam break, forest fire at 60 tps low-end.
- **M2 Bodies & mobs (7–12):** rigid bodies, joints, fracture, mobs, nav, utility AI, animals/villagers basics. Demo: collapsing cave with fleeing creatures.
- **M3 Survival core (13–18):** player needs, tools (Tinkers), crafting, smelting/cooking, farming, building variants, maps, beds, trading, advancements, weather/seasons, text layer v1.
- **M4 Engineer path (19–26):** kinetic, steam, electric I/II, belts/pipes/inserters, multiblocks, networks, logic, digital storage, trains, drones.
- **M5 Mage path (27–34):** mana, wands, runes, rituals, alchemy, enchanting, constructs, aspects, codex.
- **M6 World & depth (35–40):** depth tiers, dimensions, structures, bosses, dungeons, POIs, worldgen polish.
- **M7 Hybrid, education, polish (41–48):** techno-magic, education lessons and lab tools, audio, accessibility, modding overlays, performance passes, Steam Deck profile.

Each milestone ends with a playable build, a captured demo, bench table, LOC and test counts.

---

# PART XII — RISK REGISTER (address in plan with mitigations)

GPU memory on iGPU (pool sizing/eviction); readback latency (double-buffer); Margolus artifacts (phase permutation); cross-vendor determinism (integer ops only, test matrix); save migrations; content explosion vs validators; feature creep (pillar test + matrix cost column); art pipeline cost (procedural materials); multiplayer deferred (lockstep-ready design); education accuracy (cite constants, review lessons); magic balance (budgets on mana, risk mechanics); scope (milestones are cut lines, not promises).

---

# PART XIII — DELIVERABLES OF SESSION 1

1. `docs/research/*.md` (engine topics and mod feature matrix).
2. `docs/ENGINE_CORE.md`: per-section file list, structs with byte layouts, kernel pseudocode, acceptance tests and bench targets, risks, session schedule for M1–M2.
3. `docs/GAME_DESIGN.md`: profiles, paths with milestone capability tables, tech and magic trees as RON drafts (`assets/tech.ron`, `assets/magic/tree.ron`), feature matrix mapping, UI list, text-layer verb list, education lesson list.
4. ADRs for any deviation from this spec.
5. STOP for approval.