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
| Education | Off | Off | Real units and names | Lab world with time control and environment dials, instruments and probes, system editor, lessons, fidelity labels (§15); magic off or labelled fictional |
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
| Student | States of matter, density and buoyancy, pressure, heat transfer, combustion and oxygen, electricity, circuits, simple machines, ecology, geology, weather → gravity, orbits, seasons, eclipses, transfers, tides and scale (§15.7) | Predict-then-measure experiments checked by scenario asserts; what-if star systems visited from the ground |
| Reader | Text-layer playthrough with full verbs and narrative modules | LLM-driven NPC dialogue hook through the action API |

### 6.1 Capability graph (proposed 2026-11-12)

The tech and magic trees (§7, §8) become two views of one **capability graph**. A node states what the player can now do (pump water uphill, see mana fields, cross a sea), not which recipe appears.

A node is a RON row with: id, domains (technology, magic, ecology), prerequisites, discovery methods, world conditions, evidence required, and unlocks. Rules:

- **Several routes per node.** Discovery methods include an experiment, a material sample, observing or dissecting a creature, a recovered text, a faction teaching it, reverse-engineering an artifact, or reaching a place. Gate nodes (the ones that open a band or a region) list at least two routes, and at least one route never requires killing a particular creature or destroying a unique site.
- **Evidence for gates, crafting for the rest.** Gate nodes unlock on a validated sim event, the same kind the Primer uses (§15.6). Ordinary nodes unlock by making or using the thing once. Demanding a demonstration for every recipe turns discovery into a checklist; a few dozen evidence gates across the whole game is the intended order of magnitude.
- **Use is not manufacture.** Found or salvaged equipment works at once. Making more of it needs the node. This keeps exploration rewards immediate without skipping the industry they point to (master design, "capability graph").
- **Knowledge belongs to the player and can be taught.** Unlocks are stored per player per world. Another player, or a faction that knows the node, can teach it for a cost, so a group does not repeat every demonstration and a specialist is worth having. Nothing is account-wide: a new world starts unknown, but the player's own understanding makes the second time fast, which is the point of an education game.
- **No stranded seeds.** A node that needs something located (an ore, a species, a ruin) also has an unlocated route that is slower or costlier: trade, synthesis, cultivation. Worlds are too varied to prove every seed by inspection, so the fallback carries the guarantee and the validator only checks that the fallback exists.
- **Connected, with exceptions.** A gate node feeds at least two other systems (a hunt feeds research, research feeds automation). Comfort, decoration and convenience nodes may be leaves; they are rewards, not load-bearing.
- **The view shows the next step, not the whole tree.** The research screen shows known nodes in full and adjacent nodes as a silhouette with a hint about where such knowledge is found. Further nodes are hidden. A complete visible tree turns a world into a to-do list and removes the surprise the bible asks for.
- **One table, many readers.** Research screen, recipe availability, loot relevance, NPC knowledge and quest hooks all read this graph. No second unlock list.
- The validator keeps the §7 checks (every node reachable, no energy-free cycle) and adds the discipline check in 6.3.

Today: recipes, progression, spells and discovery are separate tables and modules (`assets/recipes.ron`, `assets/progression.ron`, `adventure/discovery.rs`). The graph is a layer over their ids, introduced without renumbering saved ids.

### 6.2 Capability bands

Bands describe reach, not a mandatory order. A player may be far along one column and at the start of another.

| Band | Technology | Magic | Reach |
|---|---|---|---|
| Survival | Hand tools, fire, shelter | Sensing, minor runes | Home biome |
| Workshop | Alloys, components, simple machines | Wands, circles, basic elements | Caves and ruins |
| Mechanized | Power networks, pumps, processing | Enchanting, mana transfer | Dangerous regions |
| Industrial | Automated logistics, factories, vehicles | Ritual machinery, automated casting | Deep geology, oceans |
| Combined | Portals, constructs, machine-cast rituals | Large-area manipulation | Other realms |
| Planetary | Reactors, mass transport, orbital industry | Ley networks, weather and terrain magic | Moons and planets |

A band beyond Planetary is not designed here; it depends on decisions about realms and lore that are the owner's.

### 6.3 Discipline: science only, magic only, both

A per-world rule chosen at creation, stored with the world's rules (no save or generator change in its first form):

- **Science only** removes the practice of magic: no casting, no mana use, and magic items are inert curiosities. This is also the Education profile's default (§2).
- **Magic only** removes powered industry: no engines, electricity or chips. Hand tools, building, farming, simple mechanisms (lever, pulley, water wheel driving a millstone) and containers stay.
- **Both** is today's game and the default for existing worlds.

A shared **craft** domain (tools, fire, shelter, building, farming, cooking, simple mechanisms) is always on. Without it a single-discipline world has no early game.

The world itself is the same in all three. Magical places and old machines still exist; the player in a single-discipline world can study, trade or salvage them for materials but cannot practise that side. This keeps one seed comparable across modes, which is useful for teaching and cheap to build.

Honest scope: the two disciplines are not equal in content today, and a mode that silently dead-ends is worse than no mode. So each discipline declares the furthest band (6.2) it currently reaches, the world-creation screen states it, and the validator checks that declared reach rather than demanding full parity. Closing the gap is content work tracked per band. Hybrid-only capabilities (§6 table) exist only in Both.

Whether a discipline also changes what the generator places (ruins, crystals, machines) is a later, gated step: worldgen is a pure function of its config, so that needs a persisted field and a claimed generator gate.

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

### 9.1 One process shape, and loot as capability (proposed 2026-11-12)

- Every machine, ritual and reaction is the same row shape: inputs + conditions + energy + time → outputs + by-products + heat + risk. A crusher and a mana infuser differ in data, not in executor. Today `assets/processes.ron` and `assets/reactions.ron` hold parts of this; they converge rather than gain a third table.
- Machines stay legible: a player can tell working, starved and jammed apart without opening a panel, and the motion shown matches real production. Distant factories resolve as aggregates; nearby ones show items moving.
- Loot is mostly capability, not percentage upgrades: materials, machine parts, rune fragments, wand cores, creature organs, blueprints, research notes, coordinates, credentials, broken artifacts to rebuild. Equipment stats derive from parts and materials (§4), never from a rarity colour.
- Capability loot is finite, so it needs a second life: a duplicate note or blueprint can be taught, traded or given to a faction for standing; duplicate parts are consumed by building and repair. Sites never need to respawn loot to stay worth visiting.
- Loot location follows the site's function: the foundry's parts are at the foundry floor, its records in its office.

## 10. Farming, food and ecology

Crops need light, moisture and fertility, and follow seasons. Animals breed under needs. Wild mobs form a food chain. Fishing uses liquid volume and spawns mobs. Pollination, composting (a decay reaction) and fertiliser are included.

### 10.1 Populations and hunting (proposed 2026-11-12)

- **Populations, not spawns.** A species row declares habitat and climate range, diet, territory, social form, activity cycle, breeding limit and response to settlement. Far away a population is a cohort count per region advanced on fixed steps from seeded hashes; near the player it expands into individuals and folds back when left. The count is the truth in both states.
- **Consequences that bend, not break.** Overhunting lowers the count; a missing predator lets prey grow; a starved territory is taken by a neighbour. But a region's population recovers over seasons from modelled sources (neighbouring regions and refuge populations with their own counts, never spawns beside the camera), and a species is only ever lost locally (update pack, ecology). A player must not be able to remove something the capability graph needs, and a careless first week must not cost the whole world. Rare variants stay rare by low carrying capacity, not by a finite stock.
- **Investigate once, then recognise.** The first hunt of a species is an investigation: signs (tracks, remains, damage) → identify species and territory → learn its anatomy and habits → prepare → find or bait → fight → butcher and carry. What is learned is kept: later hunts read the same signs at a glance and skip to preparation. Repeating a full investigation for every kill would make the system a chore by the third animal.
- **Method changes the yield, without one right answer.** Every kill gives the base yield. The method adds one thing and costs another: fire spoils the hide and renders fat; blast ruins organs and scatters bone; poison taints meat and keeps the hide whole; a precise kill keeps organs; capture keeps the creature. No method is best for every product, so a fire mage is not simply worse at hunting than an archer.
- **Killing is never the only good source.** Husbandry, cultivation, shed or gathered parts and salvage are effective routes to the same products, not token options at a crippling penalty (update pack, resources). A player who keeps a nest or herd under the same population rules is playing as intended.
- **Pressure follows the player's actions, by region.** Threat grows where the player disturbs a territory, not on a timer, and nothing appears inside an enclosed, secured base (bible, ecology).
- **Not everything attacks.** Fear, bait, stealth and keeping out of a territory are valid. Hostility is a species column, not a default.
- Today: habitats, wildlife, fieldcraft and a spawn table exist (`adventure/habitat.rs`, `wildlife.rs`, `fieldcraft.rs`, `assets/spawns.ron`). Persistent cohort counts and method yields are not built. Cohorts need a stated budget: a coarse region grid and a few numbers per species, updated on a slow schedule, not per-animal simulation.
- Education tie-in: lesson 8 (predator and prey over a season) reads these same counts through a probe (§15.2).

## 11. Combat

Weapons by material and tool system. Ranged weapons (bows, crossbows, tech guns, wands) use one projectile table. Shields and armour. Status effects. Environmental combat is first-class: flood, burn, collapse, electrify, gas.

### 11.1 Encounters and raids (proposed 2026-11-12)

- **Enemies have a job.** Predator, territorial giant, nest swarm, blighted creature, construct guard, raider band with supply needs, hostile faction, apex creature that reshapes a region. The job sets where it is found and when it retreats.
- **Encounter qualities:** readable attacks, terrain and cover that break, weak points, morale and retreat, reinforcement with a cause the player can cut.
- **A raid target is a working place.** It has patrols and shifts on sim time, an alarm that travels by a real path, power or mana that can be cut, guarded rooms, and loot placed by function (9.1). Routes in: sneak, siege, sabotage, tunnel, flood, fly, bribe. Outcomes: strip it, wreck it, or take it intact as a base.
- **Digging must not be the answer to everything.** In a voxel world the cheapest route is always a tunnel to the vault. Sites answer with what a real place would: hard or reinforced materials around what matters, noise that raises the alarm, flooding or collapse risk, and valuables that are heavy, fixed or in use. Tunnelling stays valid; it stops being free.
- **Raids are remembered.** Attacking a living settlement changes faction standing, trade and later events through the civilisation simulation (`crates/civ`). The player is told the likely standing cost before committing, and standing can be repaired; an accidental stray shot does not end a faction relationship.
- **Other players' bases:** raiding is a server rule, off by default, and never applies to an owner who is offline.
- **Solo is complete; co-op adds options.** Every encounter and raid has full counterplay for one player. More players add attack openings, simultaneous jobs and recovery options before any health increase (update pack, co-op).
- **A site earns its place.** No laboratory under every village and no switch that only opens a loot room (update pack, rejections). Each site states what it was for, and its layout, defences and contents follow from that.
- **Scope.** Build one raider outpost to this standard before generalising. A battle and army editor (place armies, budgets, save as scenario) is attractive but is a second product; it is parked until the lab's run, reset and compare controls (§15.3) exist to host it.

### 11.2 Second integrated slice: one valley (proposed 2026-11-12)

After the Flooded Observatory (§3), the next slice is one dense valley rather than more tiers.

Core (the slice is not done without these): one ore chain to iron, one mana crystal source and a first wand, one item network, a blighted cave with its own creatures, three ordinary creature families and one dangerous huntable creature, a raider outpost, an abandoned magic laboratory, a small settlement.

Stretch (add only after the core plays well): a second ore chain to steel and basic electricity, a fluid line, an apex creature, a dungeon, and a portal that needs both machine and magic work.

Acceptance chain for the core, crossing each system once: missing livestock → track a blighted predator → keep its sense organ whole → gain a new way of seeing → find a buried conduit → enter the laboratory → learn to filter mana → automate crystal production. The portal extends the chain in the stretch set.

The chain is authored for Both. For single-discipline worlds (6.3), only its two gates (the new way of seeing, and filtering) need an alternative route in this slice; tripling the whole chain before the first version has been played would be wasted work. Target length is a few hours for a first-time player, to be checked by playtest, not asserted.

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

## 15. Education and universe sandbox (for review)

Proposed 2026-11-12. Nothing in 15.1–15.7 is built unless a "today" note says so. The aim is one rule applied everywhere: **the player changes a cause, predicts the effect, and measures what the simulation actually did.** Reading a page never completes anything (master plan C-S1).

### 15.1 Five design rules

1. **Measure, do not be told.** Every simulated quantity the player can affect is readable through an in-world instrument, in a stated unit. No lesson depends on a value the player cannot measure.
2. **Predict before running, where it is a lesson.** A graded lesson asks for a prediction first (a number, a ranking, or a yes/no), then shows prediction against measurement. A wrong prediction is recorded, not punished; the journal keeps failed hypotheses (bible §5). In survival the Primer offers the prediction as an optional prompt and never blocks play on it.
3. **One cause at a time.** The lab and the system editor let the player change one parameter and hold the rest, and show which parameters differ from the reference.
4. **Say how true it is.** Each quantity and law carries a fidelity label (15.5). The game never presents a tuned or fictional value as a measured real one.
5. **Same simulation as the game.** Lessons run on the shipping sim with fixed steps and seeded hashes. There is no separate "teaching physics". A lesson is a seed, an edit list and asserts, so it replays identically on any machine and can be shared.

### 15.2 Instruments and the inspector

- Lab tools: thermometer, barometer, scale, spectrometer (§2), plus stopwatch, ruler/rangefinder, flow meter, voltmeter/ammeter, light meter and a sextant-style angle tool for the sky. Each reads an existing plane or body state; an instrument for a plane that does not exist yet waits for that plane.
- A reading shows value and unit. Resolution and the fidelity label sit one step away in the "why" view, so the world is not covered in badges. "Why" opens the relation that produced it with the current numbers substituted (for example, hydrostatic pressure from depth and density), and names the constants' source row.
- Probes: an instrument can be left in place to log a reading per fixed step. Logs plot against tick time and export as CSV. Logging is budgeted (a bounded number of probes and samples per world).
- Today: the look-at readout (`crates/app/src/egui_ui/look_at.rs`) and the material inspector are the starting surface. Extend them; do not add a parallel readout.

### 15.3 The lab (ground-scale sandbox)

The Education profile opens into a lab world built from the deterministic reference scenarios (`docs/06_DETERMINISTIC_REFERENCE_SCENARIOS_V2.md`) instead of open terrain.

- Time control: pause, single fixed step, slow and fast as integer multiples of the fixed step. Wall-clock time never enters the sim.
- Environment dials per lab world: gravity, ambient temperature, air pressure, oxygen. Dials are world constants set at creation or between runs, not mid-step, and are shown as differences from the home reference.
- Run, reset, compare: reset restores the scenario's initial state; two runs with one changed parameter can be plotted on one graph.
- Share: a lab setup is its scenario id, dial values and edit list. Text that fits in a clipboard.

### 15.4 The universe sandbox (system-scale)

Built on `engine_space` (rails, `transfer`, `design`, `influence`, `validate`) and the system map. Today the crate computes sphere of influence, Hill radius, Roche limit, circular and escape speed, Hohmann transfers, synodic period, lead angle and launch windows; the system map and flight HUD display part of this. The sandbox exposes the rest and makes it editable.

- **System editor.** Open a copy of a star system from `assets/star_systems.ron` and change a body's mass, radius, period, eccentricity or parent. Derived values update live: semi-major axis from the period, surface gravity, density, escape speed, Hill radius, Roche limit, sphere of influence, apparent size from a chosen body, day and year length. `validate` and `design::audit` findings appear as plain statements ("this moon orbits inside the Roche limit and would break up") instead of blocking the edit.
- **What-if copies only.** Edited systems are sandbox worlds. The survival system file and saved worlds are never rewritten by the editor, which keeps generator gates and save compatibility intact.
- **Time warp on rails.** Bodies are on analytic rails, so the map can scrub or warp system time without stepping the voxel sim. Warp on a landed world remains an integer multiple of the fixed step.
- **Observe from the ground.** An edited system can be visited from the surface of a body that already has terrain: the player sees the changed day length, moon size, seasons and sky. This link between the two scales is the main thing a 2D orbit toy cannot do. Limits, stated to the player: sky, light, day length and gravity follow the edit; the terrain does not regenerate, so a body whose radius or mass was changed keeps its original ground and is labelled as such. Eclipses: today the planet's shadow falls on orbital structures (`world/orbital/megastructures.rs`); no code was found that darkens the ground when a moon covers the sun, so a ground solar eclipse is new render work.
- **Bounded edits.** Fields have ranges that keep the world playable and safe to look at (no day shorter than a set minimum, to avoid strobing light). A value outside the range can be explored on the map but not visited.
- **Transfer planner as a lesson surface.** The planner shows the burn budget, the transfer time and the next window, with the relations behind each number available through the same "why" as instruments.
- **Out of scope for now:** N-body integration, collisions between bodies, stellar evolution, and anything that changes loaded terrain as a consequence of an orbit edit. Rails stay two-body patched conics; the fidelity label says so.

### 15.5 Fidelity labels and the scale card

Every instrument reading, lesson and editor field carries one label:

| Label | Meaning | Example |
|---|---|---|
| Real | Law and constant match the cited real value | G; water's freezing point in the education constant table |
| Model | Real law, reduced model; the simplification is stated | Two-body rails; cellular water; yield as `exp(-D*A)` |
| Game value | Real law, but the number is compressed or tuned for play | Home radius about 20 km, 480 s solar day, tuned thermal columns in `assets/materials.ron` |
| Fictional | Not real physics, but rule-governed | Mana, aspects, resonance |

Four labels, not more: a student has to remember them. Whether a game value is a clean scaling or a tuning is said in its "why" text, not by a fifth badge.

The **scale card** is one screen that lists every game value that has a real counterpart beside its real counterpart and the ratio, and states which laws still hold exactly at game scale (Kepler's third law does; the density needed for Earth gravity on a 20 km world does not exist). Constants and labels live in the X05 name and constant table, not in Rust. The home radius is currently stated differently in different places (20,371 m in `assets/star_systems.ron`, a 30.6 km sphere in the cloud shader, a sky-scaled Earth radius in `world/orbital/megastructures.rs`, 6,360 km in the atmosphere config). These must be reconciled, or each declared as a deliberate separate scale, before the scale card ships; the "about 20 km" example above is the star-system value, not a settled one.

### 15.6 Lesson format and assessment

A lesson is a RON row: scenario id, the question, the prediction type, the instruments allowed, the asserts on validated sim events, the tolerance, the constants cited, and the fidelity label. Layers follow the bible: hint, observation, hypothesis, apparatus, experiment, measured result, alternatives, reusable schematic.

- Completion requires the asserted event in the sim (a tracked transfer, a measured value inside tolerance), reached by any apparatus. Lessons accept alternative methods.
- Open-ended tasks ("put a moon in a stable orbit with a 2-day month") are checked by asserts on the outcome, not on steps.
- A lesson report lists prediction, measurement, error and the run's seed and edit list. It exports as text for a teacher; nothing is uploaded.
- The Primer in survival draws from the same lesson rows using the player's surroundings; the Education profile presents them as a course. One table, two presentations.

### 15.7 Lesson list

Ground lessons:

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

System lessons (need 15.4):

11. Gravity and mass: change a planet's mass or radius, predict surface gravity, then drop a body and time it.
12. Orbits and Kepler's third law: change a moon's period, predict its distance, measure it on the map.
13. Day, year and seasons: change spin and axial tilt, measure day length and noon sun height through a year.
14. Phases and eclipses: predict the moon's phase from the orbit, then watch from the ground. The eclipse half waits on ground eclipse rendering (15.4).
15. Escape and transfer: predict escape speed and a Hohmann burn budget, then fly it.
16. Tides, Hill sphere and Roche limit: move a moon inward until the audit reports break-up; explain the two limits.
17. Scale: use the scale card to convert a game measurement to its real counterpart.

Lessons 5, 7 and 9 depend on substrate that is still a future gate (oxygen fire, lever bodies, erosion); they stay listed but ship only when the sim supports their asserts. Lessons 11–13, 15 and 17 rest on maths `engine_space` already has and are the cheapest first set.

Accuracy: each lesson cites the constant it uses and carries a fidelity label. Lesson text is reviewed before release (PLAN2 and spec XII).

## 16. Feature adoption

The full matrix is `docs/research/MOD_FEATURE_MATRIX.md`. Each feature is classified as data-only, small code, or new system, and assigned to one of the spec milestones M1–M7 (ENGINE_CORE §11).

The spec's minimum list (VII) is the candidate content backlog. A "data-only" cost is conditional on its substrate: reaction rows can express transformations after the required fields/conditions exist, but tree growth needs geometry updates, sticky materials need contact rules, and brewing/cooking need containers and interactions. Before moving a matrix row into a milestone, name its existing loader, executor, player feedback and persistence path. Missing any of those makes it code plus data, not a free content addition.

## 17. Open decisions for approval

1. Accept the profiles and the path milestones in section 6.
2. Accept the draft tech and magic trees (`docs/game/drafts/`) as direction only, with the real files written in M3 (tech) and M5 (magic).
3. Confirm the text-layer verb list (section 14) as the first scope.
4. Confirm the education lessons (section 15.7) as first-release scope, and which set ships first: ground (1–4, 6) or system (11–13, 15, 17).
5. Accept the five education rules (15.1) and the fidelity labels (15.5) as binding on all instrument and lesson text.
6. Accept the system editor as what-if copies only (15.4), with N-body and body collisions out of scope.
7. Decide whether instruments exist as items in survival (found and crafted) or only in the Education profile.
8. Confirm the milestone each path ships with, against ENGINE_CORE §11.
9. Accept the capability graph (6.1) as the single unlock table over the existing recipe, progression, spell and discovery ids.
10. Accept the discipline rule (6.3) as a world rule first; decide later whether it also changes generation.
11. Accept persistent, recovering population counts and method yields (10.1) as the hunting model.
12. Accept the valley (11.2) as the slice after the Flooded Observatory, with its core and stretch split.
13. Accept that a discipline declares its current reach (6.3) instead of waiting for full parity.
14. Accept per-player, teachable knowledge (6.1).
