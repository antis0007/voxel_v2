# Game design: evolving proposal

Status: **proposed long-range design with existing systems listed below.** This document does not create assets or establish implementation acceptance. The tech and magic trees are drafts in `docs/game/drafts/` (ADR 0014 row 15). Conflicts with the engine plan are in `docs/adr/0014-spec-v2-reconciliation.md`. Engine dependencies are in `docs/ENGINE_CORE.md`; current sequencing and visual integration are in `docs/plans/2026-10-24-engine-and-world-refinement.md`.

Source: `docs/specs/ENGINE_CORE_PROMPT_v2.md`, sections I, IV, V, VI and VII. The feature matrix is `docs/research/MOD_FEATURE_MATRIX.md`. `docs/MASTER_PLAN.md` reconciles this proposal with the later game bible and mod addendum, including the Work, early Primer, shared VM, civilizations and claim assignments. The North Star document retains its interaction design; the later bible governs conflicts in cosmology and progression. Dated counts below are an earlier inventory, not current acceptance evidence.

---

## 0. What exists today, and where it goes

| Today | Where | Becomes |
|---|---|---|
| 94 spells (26 projectiles, 53 modifiers, 3 effects, 12 other) and an ordered wand deck evaluator, 8 slots | `assets/spells.ron`, `crates/app/src/world/adventure/wands.rs` | Spell cards over the effect primitives (§8); modifiers keep their rows; projectile and effect rows gain a primitive list (M5) |
| Mana, health, hunger and ward as player stats; 3 foods | `crates/app/src/world/adventure.rs` | Needs driven by planes, toggled per profile (§2, M3) |
| 85 materials, 24 with thermal and electric columns, game-tuned | `assets/materials.ron`, `crates/content/src/sim_traits.rs` | Every material gets sim columns; education profile uses measured constants through a name and constant table (M3, M7) |
| 5 reactions | `assets/reactions.ron` | The full reaction table (ENGINE_CORE §5, M1) |
| 36 recipes (hand, bench, crystal tech), JEI-like recipe book | `assets/recipes.ron`, `crates/app/src/egui_ui/recipe_book.rs` | Shaped and shapeless recipes plus machine recipes; the browser stays (M3, M4) |
| 27 machines, integer power, packet item pipes, multiblocks | `assets/machines.ron`, `crates/app/src/world/factory.rs` | The engineer path's first tiers; belts and kinetic networks are new (M4) |
| Tinkers-style tool parts in 6 materials | `crates/app/src/world/adventure/tinkering.rs` | Modular tools with durability and material physics (§4, M3) |
| 9 species, 5 bosses, sites, dungeons, the Undercrown | `assets/bestiary.ron`, `crates/app/src/world/adventure/bosses.rs`, `crates/worldgen/src/` | Mob table (M2); bosses become physics challenges (§5.3, M6) |
| Creative tools (brush, area, destructor), flight | `crates/app/src/creative.rs` | The creative profile (§2) |
| No profiles, text layer, localization, durability, thirst, maps, beds, trading, seasons gameplay, dimensions | | New (milestones below) |

---

## 1. Pillars and the pillar test

Every feature must serve at least one pillar, and a feature that serves none is cut.

1. **Matter is real.** Sand falls, water pressurises, gas ignites, metal conducts, rock collapses without support. No cosmetic blocks.
2. **Systems over scripts.** Machines, spells, creatures, weather and story events are rules over fields and materials.
3. **Scale.** Thousands of machines and hundreds of bodies on a laptop iGPU.
4. **Two syntaxes, one physics.** The technology path and the magic path reach the same capabilities by different means.
5. **Legible.** Everything the player can do is discoverable in game.
6. **Agent-maintainable.** Content is data. Code is small, documented, deterministic and headless-testable.

Audience paths: builder, engineer, mage, explorer, survivor, student, reader.

## 2. Profiles and modes

| Profile | Needs | Hostiles | Building | Notes |
|---|---|---|---|---|
| Survival (default) | On | On | Normal | Death drops items |
| Creative | Off | Off | Brush, fill, copy and paste as bodies, symmetry, schematics | Infinite items, flight |
| Education | Off | Off | Real units and names | Lab tools (thermometer, barometer, scale, spectrometer), experiment scenarios, lessons; magic off or labelled fictional |
| Fantasy | On | On | Normal | Magic on, default naming |
| Adventure | On | On | Normal | Pre-made worlds with narrative modules and locked blocks |
| Hardcore, Peaceful, Spectator | Variants of the above | | | |

Survival needs (health, hunger, thirst, temperature, oxygen, darkness exposure) are each driven by a plane and each can be switched off per profile.

## 3. The core loop

Explore, gather, craft or cast, build and automate, discover deeper layers, biomes and secrets, unlock tiers, repeat at a larger scale.

First integrated target: the **Flooded Observatory** from the existing North Star design. Arrive through a readable wooded shoreline, enter a usable ruin, inspect a blocked/flooded route, and reach the same objective by a physical intervention, a machine, or a spell. Observe the consequence, obtain a useful reward, then save, leave and return. Start with supported mechanics; finite pressure, oxygen fire and structural collapse remain future gates until their real substrate exists. This is a proposed acceptance slice, not a claim that the scenario is built.

Keep three decisions coupled: what the player learns, what physical state changes, and what geometry/light/sound communicates that change. Both disciplines expose common effects with their own costs: magic supplies portable intervention, machinery sustained throughput. Reuse the current wand, factory, enemy and reward paths while migrating their writers incrementally.

## 4. Player

- **Body:** capsule with inventory: nine hotbar slots, 27 backpack slots, armour, accessories, backpack upgrades.
- **Tools:** modular, Tinkers-style. Head, binding and handle materials set speed, durability, damage and traits. Materials have physics: a copper tool conducts, a wooden one burns.
- **Armour:** metal conducts heat and electricity; wet cloth freezes.
- **Status effects:** burning, wet, frozen, poisoned, irradiated, charged, mana-sick. Each comes from a plane or a reaction.
- **Movement:** walk, sprint, crouch, swim, climb ladders and vines, glide with a wing item (lift from the body model), mounts, vehicles.
- **Interaction:** break time depends on tool against material hardness. Placement is physical: an unsupported block falls. Small bodies can be carried, large ones pushed.

## 5. World progression

### 5.1 Depth tiers

Topsoil, caves, deep caves (aquifers, lava), crystal and fungal caves, the Deep (dark, eldritch, high-tier ores, ancient machines), the Mantle (lava ocean, heat-resistant tech and magic needed). Gates are material hardness, temperature, gas and light.

### 5.2 Dimensions

Each is a separate world with a portal body, a worldgen configuration and physics constants (gravity, ambient temperature, oxygen, mana density).

- **Nether analogue:** pressure and fire world, lava physics throughout.
- **Sky analogue:** floating islands, low gravity, wind.
- **Void analogue:** low light, magic-dense, bosses.
- **Pocket dimensions:** small worlds for bases (Dimensional Doors style). Optional.

### 5.3 Structures and bosses

Villages, temples, mineshafts, dungeons, strongholds, ancient factories (tech lore), wizard towers (magic lore), ruined labs (education lore), boss lairs. Loot tables are RON.

Bosses are physics challenges: an iron golem built of bodies, a liquid hydra, a storm elemental of gas and charge. Each needs the environment manipulated to defeat.

The architectural and painterly references refine this target: coherent rooflines, real arches/openings, supported waterfronts, multi-route interiors and distinct cultural palettes. Build one complete site before expanding to cities. Extend `crates/worldgen/src/architecture.rs`, `sites.rs` and the existing dungeon plans; share their resolved state with collision, navigation, lighting and far proxies. Low settings may simplify ornament, but must retain routes, openings, enemy cues and landmark silhouettes. Reference-image fidelity is unreviewed: the supplied text describes images that were not attached to this request.

## 6. Paths and capability milestones

Each path must reach endgame capability: automated resources, flight, long-range transport and world shaping.

| Path | Milestone order | Capability at each step |
|---|---|---|
| Engineer | Stone tools → burner machines → kinetic (water wheel, windmill, hand crank) → steam → electric (coal, then solar, wind, hydro) → chemical → nuclear → digital storage and autocraft → drones and logistics → rockets and airships | Auto-mining; auto-farming; auto-smelting; item transport; power grid; teleport pads (late); terraforming machines |
| Mage | Mana gathering (leyline wells, mana flowers, blood or soul sacrifice for the dark branch, crystal resonance) → wands and spell cards → runes and glyph circuits → rituals → golems and familiars → transmutation → enchanting → celestial branch → portals | Golem mining; growth-magic farming; transmutation smelting; mana pipes; rune networks; blink and portals; terraforming spells |
| Hybrid (endgame) | Mana-electric converters, enchanted machines, spell-casting turrets, machine-cast rituals | Capabilities neither path reaches alone: stable portals, weather control, matter synthesis |
| Explorer and survivor | Maps, waystones, mounts, dungeons, bosses, trading, base building, cooking depth, seasons | Safe travel; base building; seasonal food |
| Student | States of matter, density and buoyancy, pressure, heat transfer, combustion and oxygen, electricity, circuits, simple machines, ecology, geology, weather | Graded experiments via scenario asserts |
| Reader | Text-layer playthrough with full verbs and narrative modules | LLM-driven NPC dialogue hook through the action API |

## 7. Tech tree (draft)

Tiers: Primitive → Bronze and kinetic → Iron and steam → Electric I → Chemical → Electric II → Nuclear → Advanced.

Draft: `docs/game/drafts/tech.ron`, for review only. Each node has an id, a tier, the nodes it requires, what it unlocks, and the capability it grants (auto-mining, item transport, power grid, flight, world shaping). The tree moves to `assets/` in M3 with its loader, validator and round-trip test. The validator must prove that every node is reachable and that no energy-free cycle exists (spec IX).

## 8. Magic (draft)

Substrate: a mana plane (u8) with regeneration from sources, decay, diffusion through conductive materials, and storage in crystals. An aspect plane (8 aspects: fire, water, earth, air, light, dark, life, order) appears only where needed.

Casting: wands are items with slots, mana capacity, recharge and cast delay. Spell cards are modifiers (speed, bounce, pierce, split, homing, delay, trigger, timer, area, chain, cost, lifetime) followed by effects drawn from a fixed primitive set: `HeatImpulse`, `ForceImpulse`, `Transmute`, `SpawnMaterial`, `SpawnBody`, `SpawnMob`, `SetField`, `Teleport`, `Shield`, `EmitEvent`, `Text`.

Draft: `docs/game/drafts/magic_tree.ron`, for review only. It holds the eight schools with their aspect and primitives, the research nodes (mana gathering → wands → runes → rituals → constructs → transmutation → enchanting → celestial → portals) with the same node shape as the tech tree, and two example rituals with backfire rows. It moves to `assets/` in M5 with its loader and round-trip test.

Risk and corruption: mana sickness, and flux pollution as a gas. Backfire reactions make rituals risky.

Discovery: experiments log reactions and spells in a codex. Aspect scanning of materials unlocks research nodes. Secret recipes are rolled per world seed from a grammar.

## 9. Crafting, items and building

- **Crafting:** shaped and shapeless RON recipes; workbench tiers; furnaces as machines (heat plane makes campfires and lava work); cooking by temperature, time and vessel; brewing as reactions in cauldrons; enchanting as rule modifiers; tool forging; repair.
- **Item physics:** properties come from material. Dropped items are small bodies that float or sink by density.
- **Recipe browser:** searchable, shows uses and sources. Material inspector shows the planes at a point.
- **Building:** shapes (slab, stair, pillar, arch, beam) are data. Framed blocks take any material. Decorative blocks are still matter: they burn, conduct and collapse.
- **Structural realism:** support strength is a profile setting.
- **Schematics and blueprints:** copy a region into a body, paste it, list materials, and let drones or golems build.

## 10. Farming, food and ecology

Crops need light, moisture and fertility, and follow seasons. Animals breed under needs. Wild mobs form a food chain. Fishing uses liquid volume and spawns mobs. Pollination, composting (a decay reaction) and fertiliser are included.

## 11. Combat

Weapons by material and tool system. Ranged weapons (bows, crossbows, tech guns, wands) use one projectile table. Shields and armour. Status effects. Environmental combat is first-class: flood, burn, collapse, electrify, gas.

## 12. Transport

Rails and carts (bodies on track), buoyant boats, gas-lift airships, gliders, mounts, kinetic elevators, waystones and teleport pads (late), portals.

## 13. User interface

Inventory, hotbar, recipe browser, material inspector, spell lab (wand editor), machine GUIs generated from RON schemas, a world map with points of interest, an advancement tree, a text console, and accessibility (remapping, colour-blind palettes, text scale, reduced motion, controller support).

## 14. Text layer (verbs and description)

The text layer exposes a world-query API over `sim`. Nouns are materials and entities near the player. Verbs are `Action`s. Descriptions are generated from planes (light, temperature, gases, sound, moisture) and material tables. Rooms are named regions from worldgen points of interest. Narrative modules are rule content. The parser supports synonyms and pronouns, and an LLM hook for NPC dialogue goes through the action API.

Initial verb list (for review):

| Verb | Action | Example |
|---|---|---|
| look | describe region | "look" |
| examine | material and plane readout | "examine water" |
| take, drop | item transfer | "take torch" |
| dig, place | `Action::Dig`, `Action::Place` | "dig north" |
| light | heat impulse | "light the log" |
| pour | liquid transfer | "pour water on sand" |
| cast | spell cast | "cast fire at the door" |
| build | place a schematic | "build bridge" |
| wait | advance time | "wait 10" |
| measure | lab readout | "measure temperature" |
| inventory | list items | "inventory" |

## 15. Education lessons (for review)

Each lesson is a scenario with asserts and a text-layer explanation:

1. States of matter: water freezes, boils and condenses at fixed temperatures.
2. Density and buoyancy: which materials float on water.
3. Pressure: a column of water pushes harder at depth.
4. Heat transfer: conduction through metal against stone.
5. Combustion and oxygen: fire in a sealed room goes out.
6. Electricity: conductors, insulators, and a closed circuit.
7. Simple machines: a lever and a fulcrum over a body.
8. Ecology and food webs: a predator and prey population over a season.
9. Geology and erosion: water carving stone over time.
10. Weather: rain and exposure on a valley floor.

Accuracy: each lesson cites the constant it uses. Lesson text is reviewed before release (PLAN2 and spec XII).

## 16. Feature adoption

The full matrix is `docs/research/MOD_FEATURE_MATRIX.md`. Each feature is classified as data-only, small code, or new system, and assigned to one of the spec milestones M1–M7 (ENGINE_CORE §11).

The spec's minimum list (VII) is the candidate content backlog. A "data-only" cost is conditional on its substrate: reaction rows can express transformations after the required fields/conditions exist, but tree growth needs geometry updates, sticky materials need contact rules, and brewing/cooking need containers and interactions. Before moving a matrix row into a milestone, name its existing loader, executor, player feedback and persistence path. Missing any of those makes it code plus data, not a free content addition.

## 17. Open decisions for approval

1. Accept the profiles and the path milestones in section 6.
2. Accept the draft tech and magic trees (`docs/game/drafts/`) as direction only, with the real files written in M3 (tech) and M5 (magic).
3. Confirm the text-layer verb list (section 14) as the first scope.
4. Confirm the education lessons (section 15) as first-release scope.
5. Confirm the milestone each path ships with, against ENGINE_CORE §11.
