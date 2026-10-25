# Mod and game feature matrix

Scope: vanilla Minecraft, Create, Mekanism, Thermal Series, Immersive Engineering, IndustrialCraft 2, GregTech, Ender IO, PneumaticCraft, Applied Energistics 2, Refined Storage, Integrated Dynamics, ComputerCraft, OpenComputers, Tinkers' Construct, Silent Gear, Apotheosis; plus Botania, Thaumcraft 4 and 6, Ars Nouveau, Ars Magica 2, Blood Magic, Astral Sorcery, Electroblob's Wizardry, Mana and Artifice, Occultism, Eidolon, Hex Casting, Biomes O' Plenty, Terralith, Twilight Forest, The Aether, Dimensional Doors, Alex's Caves, Deeper and Darker, Farmer's Delight, Pam's HarvestCraft, Mystical Agriculture, Dynamic Trees, Serene Seasons, Storage Drawers, Sophisticated Backpacks, Iron Chests, Waystones, JEI/EMI, JourneyMap, Xaero's, Chisel, Framed Blocks, MineColonies, Millénaire, Noita, Terraria and Dwarf Fortress.

Feature descriptions were checked with web search; `[unverified]` or "unverified" marks a claim not confirmed from a reliable source (details in the notes at the end). Cost and milestone are judgement calls.

Cost key: data-only = new RON rows or a table change, no new Rust logic; small code = a new variant, action, check or hook on an existing path; new system = a subsystem or simulation pass that does not exist yet.

Milestones are defined in `docs/ENGINE_CORE.md` §11: M1 foundation, M2 bodies and mobs, M3 survival core, M4 engineer path, M5 mage path, M6 world and depth, M7 hybrid, education and polish.

Rows per milestone: M1 3, M2 12, M3 17, M4 46, M5 22, M6 38, M7 15 (total 153).

| Feature | Source | Maps to (cells, planes, reactions, RON content, Action enum, machine table, network, or new system) | Cost | Milestone |
|---|---|---|---|---|
| Survival loop: health, hunger, food values `[unverified]` | Vanilla Minecraft | Player state plus day cycle (existing); food as RON content | small code | M3 survival core |
| Redstone signal: 15-block decay, repeater delay 1-4 ticks, one-way, lockable | Vanilla Minecraft | network (signal graph over cells, stepped on the fixed tick) | new system | M4 engineer path |
| Redstone pulse parts: observer pulse, piston push 12 / pull | Vanilla Minecraft | Action enum (pulse trigger) plus network edges; piston as machine table entry | small code | M4 engineer path |
| Enchanting table: levels up to 30 from 15 bookshelves, lapis cost | Vanilla Minecraft | RON content (enchantment rows); level formula in code | small code | M5 mage path |
| Villager job blocks, professions, five-tier leveling, restock twice a day | Vanilla Minecraft | machine table (job block) plus RON trade tables; villager AI as new system | new system | M2 bodies and mobs |
| Nether fortress blaze rods (50% drop, Looting) and Eye of Ender chain (12 eyes to End portal) | Vanilla Minecraft | RON content (bestiary drop table, recipes) plus fortress worldgen | data-only | M6 world and depth |
| Ender pearls from Endermen, Looting bonus | Vanilla Minecraft | RON content (bestiary drop table) | data-only | M2 bodies and mobs |
| End dimension and Ender Dragon fight: crystal respawn, 500 XP on repeat kills | Vanilla Minecraft | Action enum (boss phases) plus existing adventure bosses | small code | M6 world and depth |
| Nether dimension: portal travel and ambient hazards `[unverified]` | Vanilla Minecraft | new dimension layer; hazards as status effects | new system | M6 world and depth |
| Brewing: nether wart base, modifiers, 20 s per brew, blaze fuel | Vanilla Minecraft | reactions (ingredient pair table) plus machine table (brewing stand) | data-only | M5 mage path |
| Advancements: tabs, criteria triggers, rewards (XP, recipes, loot) | Vanilla Minecraft | new system (event triggers feeding a progress store) | new system | M7 hybrid, education, polish |
| Ancient City in Deep Dark: warden, sculk, swift sneak loot | Vanilla Minecraft | worldgen template plus bestiary entry | new system | M6 world and depth |
| Woodland mansion: evokers, vindicators, spider spawner rooms | Vanilla Minecraft | worldgen template plus bestiary entries | new system | M6 world and depth |
| Stronghold: portal frames, Java cap of 128 per world | Vanilla Minecraft | worldgen structure placement, seeded by world seed | small code | M6 world and depth |
| Villages: structures with job-site blocks and houses `[unverified]` | Vanilla Minecraft | worldgen template plus machine table (job blocks) | new system | M6 world and depth |
| Mob spawn rules: opaque floor, biome lists, 24-block minimum distance (light threshold disputed, 7 vs 0) | Vanilla Minecraft | bestiary RON spawn conditions checked against cells (light, biome) | small code | M2 bodies and mobs |
| Mechanical rotational stress network: shafts, cogs, gearboxes `[unverified]` | Create | network (new rotational graph beside power) | new system | M4 engineer path |
| Mechanical processing: press, mixer, deployer recipes `[unverified]` | Create | machine table plus RON recipes | data-only | M4 engineer path |
| Belts moving items between processors `[unverified]` | Create | network (item edges between inventories) | small code | M4 engineer path |
| Contraptions: assembled moving structures `[unverified]` | Create | bodies (moving voxel body inside the cell grid) | new system | M2 bodies and mobs |
| Trains on rails with stations and schedules (confirmed via Create addons) | Create (Steam 'n' Rails, Railways Navigator, Realism) | network (rail graph) plus Action enum (schedule instructions) | new system | M6 world and depth |
| Signals, route navigator, conductor whistle that summons a train (addon) | Create addons (Steam 'n' Rails, Railways Navigator) | network (signal and route pathing) | small code | M6 world and depth |
| Energy cubes and tiered energy cables | Mekanism | network (power) plus RON tier rows | small code | M4 engineer path |
| Chemical tanks: gas, infusion, pigment, slurry as a fourth fluid class | Mekanism | cells (fluid or gas layer); new chemical type | new system | M4 engineer path |
| Fission reactor: fuel, heat, meltdown risk | Mekanism | reactions plus thermal plus machine table (multiblock) | new system | M4 engineer path |
| Crusher, enrichment chamber, combiner chain | Mekanism | machine table plus RON recipes | data-only | M4 engineer path |
| Machine side reconfiguration and redstone control modes | Thermal Series | machine table (per-face I/O) | small code | M4 engineer path |
| Forge Energy / RF lineage (Redstone Flux origin); Immersive Flux converts 1:1 | Thermal Series, Immersive Engineering | network (power unit and conversion) | small code | M4 engineer path |
| Thermal machine recipe set (pulverizer and smelter style) `[unverified names]` | Thermal Series | RON machine recipes | data-only | M4 engineer path |
| Cross-mod recipe compatibility (Create, Refined Storage, Tinkers' Construct recipes) | Thermal Integration | RON content (recipe aliases per source) | data-only | M7 hybrid, education, polish |
| Multiblock formation by right-clicking the master block with a hammer | Immersive Engineering | machine table (multiblock pattern) plus Action (form) | new system | M4 engineer path |
| Coke oven and blast furnace (steel gate for mid game) | Immersive Engineering | reactions plus multiblock pattern | data-only | M4 engineer path |
| Excavator: 3x7x8 bucketwheel digger that removes terrain | Immersive Engineering | cells (bulk removal sweep) plus machine table | new system | M6 world and depth |
| AC-style wire network with Immersive Flux | Immersive Engineering | network (power, AC-like rules) | new system | M4 engineer path |
| Holographic multiblock build guides (Engineered Schematics addon) | Immersive Engineering addon | UI overlay on machine table | small code | M7 hybrid, education, polish |
| Reactor component grid with chambers, heat and meltdown | IndustrialCraft 2 | cells (grid of components) plus reactions | new system | M4 engineer path |
| Radiation exposure, protected by hazmat gear | IndustrialCraft 2 | Player status effect plus RON armour rows | small code | M6 world and depth |
| RTG from plutonium recovered via thermal centrifuge | IndustrialCraft 2 | RON recipes plus machine table | data-only | M4 engineer path |
| Material generation: one material yields dust, ingot and other forms `[unverified]` | GregTech (GTCEu) | RON content (materials.ron) plus BlockId generation | data-only | M1 foundation |
| Ore processing chain: crush, wash, centrifuge, electrolyze | GregTech (GTCEu) | reactions plus machine table | data-only | M4 engineer path |
| Power tiers from steam to plasma turbines | GregTech (GTCEu) | network (power tiers) | small code | M4 engineer path |
| Ores spread across many base stones (sand, gravel, stone types) | GregTech (GTCEu) | worldgen ore table in RON | data-only | M6 world and depth |
| Shared-space conduits: several conduit types in one block | Ender IO | network (layered edges per cell) | new system | M4 engineer path |
| Per-face conduit modes (insert or extract), filters and colour channels via wrench | Ender IO | network edge config plus Action (wrench) | small code | M4 engineer path |
| Dimensional transceiver: long-distance and cross-dimension transport | Ender IO | network (remote edges) | new system | M6 world and depth |
| Soul vial: captures mob essence | Ender IO | bestiary RON plus Action (capture) | small code | M2 bodies and mobs |
| Soul binder: item plus essence plus XP plus RF recipes | Ender IO | machine table plus RON recipes | data-only | M4 engineer path |
| Pressure from air quantity over tank volume | PneumaticCraft | cells (gas pressure field) | new system | M4 engineer path |
| Compressors (fuel, liquid, lightning, FE) with heat dissipation | PneumaticCraft | machine table plus thermal | small code | M4 engineer path |
| Programmable drones with widgets and charging station | PneumaticCraft | bodies plus Action enum (drone program) | new system | M2 bodies and mobs |
| ME network: central item storage reachable from any interface | Applied Energistics 2 | network (item inventory graph) | new system | M4 engineer path |
| Pattern terminal encodes crafting patterns | Applied Energistics 2 | network plus RON recipes | small code | M4 engineer path |
| Molecular assembler performs autocrafting | Applied Energistics 2 | machine table | small code | M4 engineer path |
| Wireless crafting terminal, range-limited | Applied Energistics 2 | network plus range check | small code | M4 engineer path |
| Storage disks 1k/4k/16k/64k in a drive of 8 | Refined Storage | RON content (disk tiers) plus network inventory | data-only | M4 engineer path |
| Single controller per grid, no channels | Refined Storage | network topology rule | small code | M4 engineer path |
| Crafter with recursive autocrafting (Step Crafter addon) | Refined Storage (addon) | network planner | small code | M4 engineer path |
| Readers pull world values into network variables | Integrated Dynamics | Action enum (sensor) plus network reads | small code | M4 engineer path |
| Logic programmer computes on variables, held in a variable store | Integrated Dynamics | new system (expression evaluator over network variables) | new system | M4 engineer path |
| Writers act on the world (set blocks, move items) | Integrated Dynamics | Action enum | small code | M4 engineer path |
| Lua computers that drive the world (CC: Tweaked) | ComputerCraft | new system (sandboxed scripting host) | new system | M7 hybrid, education, polish |
| Turtles: mobile machines whose upgrades change behaviour (diamond axe makes a felling turtle) | ComputerCraft | bodies plus Action enum plus RON upgrades; 16-stack inventory | new system | M2 bodies and mobs |
| Peripherals: monitors, speakers, modems, wired and wireless messaging | ComputerCraft | network (message channels) | small code | M7 hybrid, education, polish |
| Hardware-assembled computers: cards, cases, tiers | OpenComputers | machine table plus RON component table | small code | M4 engineer path |
| Computer state persists across chunk unloads | OpenComputers | new system (chunk-persisted script state) | new system | M7 hybrid, education, polish |
| Robots with tools and a large inventory, stronger than drones | OpenComputers | bodies plus Action enum | new system | M2 bodies and mobs |
| Modular tools assembled from parts (head and handle) | Tinkers' Construct | RON content (parts) plus small assembly code | small code | M3 survival core |
| Data-driven material stats per metal | Tinkers' Construct | RON content (materials.ron) | data-only | M1 foundation |
| Tool modifiers (durability, mining speed, auto-repair, fortune) and modifier worktable | Tinkers' Construct | Action-style effect table plus machine table | small code | M3 survival core |
| Smeltery, melter, alloyer and foundry for molten metals | Tinkers' Construct | machine table plus reactions plus fluid cells | new system | M4 engineer path |
| Casting molten metal into parts | Tinkers' Construct | cells (fluid) plus reactions | small code | M4 engineer path |
| Blueprint and template crafting; main part, rod and cord | Silent Gear | RON recipes | data-only | M3 survival core |
| Traits: synergy, base-value thresholds, potion-cancelling armour | Silent Gear | Action hooks plus RON trait table | small code | M3 survival core |
| Affixes by rarity on any gear, rolled from loot tiers; gem sockets | Apotheosis | RON content (affix table) plus loot tables | data-only | M3 survival core |
| World Tiers scaling loot and mob rarity | Apotheosis | data-only tier table | data-only | M6 world and depth |
| Salvage to rarity materials and Reforging Table | Apotheosis | machine table plus RON recipes | data-only | M3 survival core |
| Affix bosses and Invader bosses `[mechanics unverified]` | Apotheosis | bestiary RON plus Action enum | small code | M6 world and depth |
| Balance configurator: one TOML file scales all values | Apotheosis (companion tool) | data-only config | data-only | M7 hybrid, education, polish |
| Mana-generating flower converts an input item into mana for a network | Botania | Machine table (recipe rows in RON) feeding a network | data-only | M4 |
| Spreaders and sparks throw mana along aimed routes | Botania | Network with directional transfer (item/power-style routing) | small code | M4 |
| Six primal aspects plus compound aspects tagged on items and blocks | Thaumcraft 4 | RON content (aspect field on block and item rows) | data-only | M5 |
| Scanning items with a thaumometer grants research knowledge | Thaumcraft 4 | New system: per-player knowledge state (save field, needs save version bump) | small code | M5 |
| Vis pool per chunk that spreads slowly to neighbour chunks | Thaumcraft 6 | Chunk-level scalar field diffused in a budgeted pass (same pattern as cellular) | small code | M6 |
| Research tabs unlocked by topic (Eldritch tab hidden until a topic is read) | Thaumcraft 6 | RON content: research graph with unlock predicates | data-only | M5 |
| Spell as glyph chain: form (cast method), effect, augments that modify the glyph before them | Ars Nouveau | Action enum composition in spells.ron | small code | M5 |
| Source as fuel: Source Jars, relays and sourcelinks feed machines | Ars Nouveau | Network (a parallel source network beside item and power) | new system | M5 |
| Spell built from parts, shapes, components and modifiers | Ars Magica 2 (Mithion) | Action enum composition plus RON content | small code | M5 |
| Shape Groups set spell cost and power per caster | Ars Magica 2 (1.2+) | RON content: caster and shape-group table | data-only | M5 |
| Self-sacrifice converts health into Life Essence that fills an altar | Blood Magic | New system: health as a resource, Action enum entry | small code | M5 |
| Soul Network: personal essence store bound to a blood orb | Blood Magic | Network (account-style store keyed by player) | small code | M5 |
| Constellation perk tree spent from experience for stat boosts | Astral Sorcery | New system: perk tree and progression state | new system | M5 |
| Ritual pedestals around a central altar for large effects | Astral Sorcery | Machine table with a multi-block pedestal pattern | small code | M5 |
| 180+ spells tagged with elements (fire, ice, earth and so on) | Electroblob's Wizardry | RON content: element tags in spells.ron (count from listings, not the official wiki) | data-only | M5 |
| Wand tier upgrades unlock higher-tier spells | Electroblob's Wizardry | RON content: wand tier gate in the wand deck evaluation | data-only | M5 |
| Arcane forging lets the player choose enchantments | Mana and Artifice | New system: enchant selection UI and data | new system | M5 |
| Manaweaving wand feeds mana into ritual machines | Mana and Artifice | Machine table (ritual recipe) with a mana input | small code | M5 |
| Rituals that change time, weather or long-distance travel | Mana and Artifice | New system: world-state mutation driven by a ritual | new system | M6 |
| Summoned spirits act as job workers | Occultism | Bodies and mobs system with job AI | new system | M2 |
| Mechanical chamber automates sacrifice-free rituals through a network | Create: Occult Engineering | Machine table plus network integration | small code | M4 |
| Brazier rituals powered by soul shards that craft items, summon mobs or ward areas | Eidolon | Machine table: ritual rows with a soul-shard input | data-only | M5 |
| Reputation with the dark lord unlocks knowledge and abilities (Theurgy) | Eidolon | New system: faction reputation state (exact mechanics unverified) | new system | M6 |
| Stack-based pattern programming, with patterns drawn by a staff | Hex Casting | New system: stack VM over the Action enum (base pattern list unverified, hexdoc not reached) | new system | M5 |
| Typed iota values (numbers, vectors, entities, lists) | Hex Casting | New system: typed value enum | small code | M5 |
| Patterns read and write iotas to blocks such as the Akashic Record and Slate | IoticBlocks (Hex Casting addon) | Block-state storage read and write actions | small code | M6 |
| 50+ new biomes with new plants, trees and blocks | Biomes O' Plenty | RON content plus worldgen biome table (count varies by source) | data-only | M6 |
| Nether and End biome additions (Crystalline Chasm, Ashen Inferno and so on) | Biomes O' Plenty | Worldgen biome table for each dimension | data-only | M6 |
| Datapack-style biome overhaul: 95+ biomes, updates most vanilla biomes | Terralith | RON content: biome table, layered over vanilla | data-only | M6 |
| Generator changes apply only to newly generated chunks | Terralith (datapack note) | CURRENT_GENERATOR_VERSION gate; historical gates stay frozen | small code | M6 |
| Boss-gated dimension where each boss unlocks the next area | Twilight Forest | Progression gate in regions (Action gate) | new system | M6 |
| Valkyrie Queen gated by collected victory medals (single source, unverified) | The Aether | RON content: boss arena with a token-count gate | data-only | M2 |
| Door types lead to pocket dimensions (personal, treasure, random rift) | Dimensional Doors | New system: instanced pocket regions | new system | M6 |
| Themed cave biomes, each with its own creatures and bosses | Alex's Caves | Underground worldgen biomes plus boss mobs | new system | M6 |
| Enormous bosses (Luxtructosaurus, Tremorzilla) that hunt the player | Alex's Caves | Body AI with pursuit behaviour | new system | M2 |
| Otherside opened by a Heart of the Deep dropped from the Warden | Deeper and Darker | Action portal plus boss drop plus dimension worldgen | small code | M6 |
| Darkness effect that only a Warden Helmet removes | Deeper and Darker | Status effect with an equipment check | small code | M3 |
| Cooking stations (pot, skillet, cutting board) with multi-step recipes | Farmer's Delight | Machine table with a multi-station recipe chain | small code | M4 |
| Meals as consumables with stacked effects and bowl return | Farmer's Delight | RON content: food item effects | data-only | M3 |
| Foods built from vanilla ingredients (Food Core, about 180) | Pam's HarvestCraft 2 | RON content: food rows | data-only | M3 |
| Crops and fruit trees grown from food items dropped by bushes | Pam's HarvestCraft 2 | RON content: crop and tree rows on the cell grid | data-only | M3 |
| Essence tier upgrade by surrounding a seed with the next-tier essence and an infusion crystal | Mystical Agriculture | Machine table: tier-upgrade recipe | data-only | M4 |
| Mob seeds that need chunks of the mob, obtained with a dagger | Mystical Agriculture | RON content: mob drop to seed recipe | data-only | M3 |
| Trees built from rooty soil, branch and leaf blocks; saplings grow over time | Dynamic Trees | Cellular growth on the voxel grid, budgeted tick | new system | M6 |
| Tree growth slows or stops with climate or season | Dynamic Trees (with Serene Seasons) | Season state feeding the growth tick | small code | M6 |
| Seasonal crop fertility per season, with a greenhouse override | Serene Seasons | RON content: season flags plus a greenhouse block flag | data-only | M3 |
| Seasons with a fixed length and a season clock item | Serene Seasons | Day-cycle step (existing order) plus season state | small code | M6 |
| Controller network: drawers in range form one combined inventory | Storage Drawers | Network (item network with a controller hub) | small code | M4 |
| Mass key actions applied to every connected drawer | Storage Drawers | Action enum entry: bulk network action | small code | M4 |
| Tiered backpacks with upgrade slots | Sophisticated Backpacks | RON content: tier and slot rows, plus inventory | small code | M3 |
| Filter upgrade that gates automation insert and extract by mod, tag or NBT | Sophisticated Backpacks | Network filter predicate | small code | M4 |
| Chest tiers upgraded in place with contents kept | Iron Chests | RON content: tier table plus an in-place upgrade action | data-only | M3 |
| Chest Grabber moves a chest with its contents, at a slowness cost | True Iron Chests | Action enum entry plus a status effect | small code | M3 |
| Waypoints and warp stones, with durability and cooldown rules | Waystones | New system: waypoint registry and teleport action | small code | M4 |
| Global waypoints shared across all players | Waystones | World-level registry (multiplayer) | new system | M7 |
| Recipe lookup showing craft and usage recipes | JEI | Recipe index over the RON recipe tables, plus a UI | small code | M7 |
| Craftable-now filter and recipe tree showing base ingredients | EMI | Recipe graph over the RON recipes | small code | M7 |
| Explored-area world map that records visited chunks | JourneyMap | Explored-chunk set per dimension, plus UI | small code | M7 |
| Colour-coded waypoints kept per dimension | JourneyMap | Save data keyed by dimension | small code | M7 |
| Chisel cycles a block through up to 24 variants | Chisel | RON content: variant groups plus a chisel action | small code | M3 |
| Connected textures with 2x2, 3x3 and 4x4 multiblock models | Chisel 3 | Meshing: neighbour-aware face selection (needs before/after captures) | small code | M7 |
| Blocks take the texture of the covering material in new shapes | Carpenter's Blocks | Cell material plus a sub-cell shape (new geometry) | new system | M6 |
| Frame shape camouflaged by any solid block, with split two-material faces | Framed Blocks | Cell holding a shape and two material references | small code | M6 |
| Camouflaged frame inherits the light emission of its camo block | Framed Blocks | Lighting data taken from the material | small code | M6 |
| NPC citizens with 30+ jobs living in buildings | MineColonies | Bodies and NPC system with a job table | new system | M6 |
| Raids by hostile barbarians against guard towers and barracks | MineColonies | New system: raid events | new system | M6 |
| Wand spells cast sequentially or shuffled on recharge | Noita | Wand deck evaluation (already in the adventure code) with a shuffle flag | small code | M5 |
| Stackable and non-stackable perk table | Noita | RON content: perk table with a stack flag | data-only | M5 |
| Liquids mix and transform materials in cells (potion-internal reactions conflict across sources, unverified) | Noita | Cellular reaction table in the physics sim | small code | M1 |
| Pre-Hardmode boss order gates progression | Terraria | RON content: boss gate ordering | data-only | M2 |
| Hardmode switch converts the world, with corruption and hallow spreading as diagonal stripes | Terraria | Chunk-wide cell-type conversion that spreads | new system | M6 |
| Legends mode explores the generated world history after generation | Dwarf Fortress | New system: history log as savable data | new system | M6 |
| Z-levels: sea level at z=0, cavern layers below | Dwarf Fortress | Voxel vertical chunk layers (existing Y axis) | small code | M6 |
| Verb-noun parser with synonyms and spelling leeway | Zork / Infocom | New system: text command parser | new system | M7 |
| Each verb has a function; transitive and intransitive verbs | Zork | Action enum with an arity table | small code | M7 |

## Verification notes

- All 17 named sources returned search results, so each one exists. None was checked against its official docs in depth.
- Not confirmed directly, so treat as unverified: base Create mechanics (search returned only addons); Mekanism's official feature list (search returned guides); Thermal machine names; the Immersive Engineering official wiki URL; the Refined Storage official docs; the full advancements list; the Minecraft mob light threshold (sources disagree between 7 and 0); the Apotheosis boss mechanics.
- ComputerCraft 2.0 version details were not verified. The original mod's last listed version is 1.80pr1.
- Where a row cites an addon (Create trains, Refined Storage Step Crafter, Immersive Engineering schematics), the base mod is not confirmed for that feature.
- Sources I could not fully confirm, or where the search returned only third-party or mirror pages:
  - Ars Nouveau base mod: only addon pages and third-party guides were found; the official wiki (arsnouveau.wiki) was not reached, so Source and wand details are unverified.
  - Hex Casting base pattern list: the hexdoc site was not reached; only addons and mod listings were found.
  - Noita potion reactions: the two wiki pages disagree on whether reactions happen inside potions.
  - Terraria Hardmode boss order and Queen Slime: sources disagree; supercraft.host pages contain claims that could not be verified.
  - The Aether medal count: one G-Portal wiki source; treat as unverified.
  - Eidolon Theurgy reputation mechanics: only a summary was found.
  - Ars Magica (original 1.x title): not separately confirmed; rows use Ars Magica 2 and Ars Magica: Legacy.
  - Electroblob's Wizardry spell count (180+) and Pam's HarvestCraft food and crop counts come from mod-listing pages and vary by source.
  - Dimensional Doors door mechanics come from a Modrinth page; the Hexxit wiki covers an older, different version.
  - JEI mechanics came from mod-listing and guide pages; the official JEI project page was not checked.
