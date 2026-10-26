# 07 — AXIOM: PLAYER FREEDOM, EXPLORATION, ECOLOGY AND NON-COERCIVE ECONOMY

**Status:** Mandatory game-design and systems addendum, proposed on 2026-10-26. **Evidence:** design, not implemented or playtested. **Companions:** `01_GAME_DESIGN_BIBLE.md`, `02_ENGINE_ARCHITECTURE.md`, `04_IMPLEMENTATION_AND_VERIFICATION.md`, `06_REFERENCE_CONTRACTS_AND_SCENARIOS.md`.

> **Primary player contract:** Automation makes known work repeatable and scalable. Exploration discovers new opportunities, real places and new ways to understand and change the world. Neither is the mandatory prerequisite for enjoying the other. Familiar necessities are obtainable through several fulfilling activities. Technical players are celebrated for invention; explorers are celebrated for observation, courage and judgment. Neither is treated as playing incorrectly.

## 0. Why this addendum exists; provenance and distinction

A user-supplied transcript discusses the Minecraft datapack *Matcha Flavoured*. Its author criticizes the incentive to copy passive AFK mob/iron farms, grinds that eclipse exploratory play, confined animals and iron golem harvesting, costly inventory management, punitive gear loss, hunger busywork, random enchantment, currency arbitrage, and unsafe late-game building. Proposed remedies in the transcript include gravel/sand production from common rock, food intrinsics, prepared meals, healing-food instead of hunger, recoverable life capacity, early portable storage, hybrid tools, repairable equipment, direct enchantment crafting, route-specific special materials, trading, habitat protection, and tangible late-game safety.

**Axiom adaptation, not a claim by the video author:** Preserve Factorio/Mekanism-level automation, genuine science, construction, magical farms and powerful player-created factories. Do not solve the disparity by making technical builds weak, breaking farms artificially, or creating "explorer-only" currencies. Change what is scarce, what can be learned, and what makes a new place useful. Unlike Matcha's mostly anti-grinder direction, Axiom must celebrate both genuine automation *and* nontechnical exploration.

Source references for review: https://modrinth.com/datapack/matcha-flavoured ; https://www.designer-notes.com/page/20/ (Soren Johnson, "Water Finds a Crack"); https://noita.wiki.gg/wiki/Areas ; user transcript dated 2026-10-26. No copyrighted textures, recipes, mechanics implementation or prose from the datapack should be copied wholesale; adapt principles, not assets.

## 1. Twelve invariant player promises (`C-PF*`)

These have priority over productivity maximization whenever the two conflict. Each MUST map to named playable tests and human playtesting, not a plausible design paragraph.

| Claim | Promise | Observable failure condition |
|---|---|---|
| C-PF01 | No tutorial/AFK compulsion for ordinary building, repair, first magic, and first useful machine | A player reasonably concludes that standing AFK or replicating a guide is essential to progress |
| C-PF02 | Multiple viable routes to ordinary necessities, with clear fallback locations and methods | One specific hostile drop, mob farm, villager exploit, or fixed chest is the only practical access to common needs |
| C-PF03 | An expert automated factory has powerful, authentic throughput advantages | All automation is artificially capped or nerfed to match hand-gathering despite paid setup and inputs |
| C-PF04 | Novel exploration yields discoveries, physical assets, opportunities, or meaningful experiences, even after advanced automation | High-tier players systematically receive only inferior commodity loot from unfamiliar sites |
| C-PF05 | Players may acquire, transport, inspect and exploit discoveries without contrived "human-only" restrictions | A useful drone or magic sensor is prohibited from finding/hauling something for no world-consistent reason |
| C-PF06 | Interesting choices about creature treatment exist; nonlethal and humane play is practical, not a challenge-mode handicap | One slaughter/iron-golem torture loop overwhelmingly dominates an essential resource across all progression stages |
| C-PF07 | Peace and safety can be permanently earned locally, including the home and restored surface regions | Routine domestic building suffers endless unpreventable hostile spawns or global timer-based raids |
| C-PF08 | Default survival has low chore friction; food/medicine/gear and inventory support voluntary preparation, not constant babysitting | Ordinary sprinting, cooking, repairs, or carrying blocks impose compulsory dull repetition |
| C-PF09 | Death produces understandable stakes without routinely deleting irreplaceable gear or hundreds of hours of construction | Players avoid exploration for fear of untelegraphed physics death or irreversible tool loss |
| C-PF10 | New and deeper/farther places retain distinct ecological, historical, geological and mechanical identities | "Progress" means recurring reskinned rooms, bigger enemy HP, or copied generated layouts |
| C-PF11 | Knowledge progresses by observation or reproducible experiments; guides remain optional and useful | Essential rule or seed-specific secret has no sufficient in-world evidence and a wiki is the rational solution |
| C-PF12 | Factories remain useful while the player explores, instead of rewarding babysitting or real-life AFK | Standing in a specific spot or keeping the game open without playing is more efficient than exploring with the same running factory |

**Do not** implement these as passive numeric achievement counters with meaningless item bonuses. Confirm by behavior, scenario testing and player interviews.

## 2. Three distinct but connected forms of value

### 2.1 Commodity value: producible and fungible

Stone, sand, timber, ores, ingots, glass, food, common reagents, energy, ordinary machine components, common enchantment inputs, cartridges, ammunition and building blocks are **commodities**. Their defining property is that a known, verified production process can reliably produce more when supplied with land, inputs, resources and energy.

- Manual path: gathering, ore veins, cave salvage, controlled explosions, fishing/trading, salvage reclamation and bulk extraction.
- Technical path: crushing, smelting, farming, drilling, fluid processing, alchemy, conveyors, railroads, programmable factories.
- Magical and biological paths: renewable transmutation with explicit inputs/efficiency, grown structural materials, symbiotic colonies, cultivable plants and organisms.
- Material paths follow real mass/unit contracts; e.g. stone -> gravel -> sand is a predictable conversion with losses/byproducts where appropriate, not an infinite no-cost generator.

**Rule:** Common tools, extensive building, recovery from death and first machinery must *not* be balanced assuming a high-output hostile farm. Advanced megastructures may justify advanced factories. The latter is an **ambition ceiling**, not an **accessibility floor**.

### 2.2 Capability value: knowledge, designs and world rules

A new rune operation, material behavior, fabrication cell template, organism trait, geological signature, circuit technique, an efficient chemical catalyst, ancient device programming, world-mechanism explanation or discovered cipher can alter what is possible.

- Knowledge can be recorded, shared, taught and compiled; no arbitrary individual soulbinding.
- Once understood, repeatable effects often *can* be manufactured or automated, sometimes only with demanding infrastructure.
- The first discovery is valuable because it expands the set of available designs, NOT because only a human player can click it.
- Procedural discoveries must describe bounded combinations of authored rule primitives, with generated context/parameters and viable evidence. Do not invent entirely new unsimulated physical laws at runtime.
- Stable laws across worlds support genuine expertise. Per-seed unknowns should typically concern composition, accessible catalysts, spatial resonance or parameters that can be measured—not arbitrary replacement of the entire chemistry table.

### 2.3 Opportunity value: locations and relationships

A unique hydrothermal reservoir, damaged Founder archive, functioning Chorister machine, Engineer dam, fungal superorganism, stranded expedition vessel, meteorite field, celestial observatory or friendly settlement is a **spatial and historical opportunity**.

- Its volume, topology, field conditions, population, local access and infrastructure cannot be copied by printing "Legendary Discovery Token".
- Its value comes from use: exploring, claiming, restoring, protecting, trading with, measuring, learning, modifying or salvaging.
- Some opportunities can be reproduced eventually at large expense; that is an *earned engineering triumph*, not a reason to make discovery obsolete.
- A found site must remain useful after its one-time chest is opened. Favor water rights, location, unique connection, environmental fields, working equipment, lore clues and maintainable ecological assets.

**Consequence:** A player with a hundred thousand ingots may still genuinely desire to visit a strange ruin because it offers new information, geography, social relationships or a physically special environment—not because an arbitrary dungeon currency is needed to craft normal armor.

## 3. Activity balance: avoid false equality and inverted incentives

Do not demand identical iron/hour for a bare pickaxe and a late-game ironworks. That would erase the identity of automation. Instead enforce **equivalent access to ordinary goals within the stage of play**, and **distinct forms of high-end value**.

| Path | Ordinary materials in early/mid game | Unique longer-term value | Risk/skill expression |
|---|---|---|---|
| Caving/prospecting | Rich visible deposits; full ore veins; cave salvage; geological clues | Material signatures, new structures, secret entrances, mineral sources | Navigation, observation, mobility, hazards |
| Combat/clearing dungeons | Gear, consumables, batches of rare but ordinary materials | Functional machinery, artifacts, new techniques, witnesses, factions | Tactical builds, physics manipulation, restraint/capture |
| TNT/controlled excavation | Fast bulk quarrying; meaningful recoverable aggregate and ore | Open new passages, water routes, geothermal access | Placement, blast containment, engineering |
| Factory construction | Increasing commodities per unit attention; precision and reproducibility | Advanced fab capability, supply autonomy, megastructures | Throughput design, constraints, debugging, logistics |
| Magical research | Material conversion and utility at bounded energy cost | New spell verbs, fields, rules and apparatus | Experiments, catalysts, world-state reasoning |
| Agriculture and cuisine | Ingredients, medicines, expedition provisions | Genetics, symbiosis, biological devices, habitation | Cultivation and ecology |
| Humane creature husbandry | Wool, milk, eggs, secretions, spores, shed materials, traded outputs | Partnership, guard/governance roles, traits and culture | Habitat design, trust, life cycles |
| Fishing, trade, missions | Useful construction components, supplies and resource substitutions | Maps, factions, contracts, stories, unusual findings | Navigation, market information, relationships |

**Economic test:** Compare players pursuing *specific, practical goals* (starter home, useful magic, 1st workshop, 2-story house, first ruined dam) under equivalent tools and hours; do NOT just compare a raw material/h number. Set tuning bands only from measured cohort performance.

### 3.1 Bulk exploration yields without loot-pinata spam

Caves expose coherent ore bodies, not isolated 1-ingot drops. Mine workings contain valuable recoverable machine assemblies, stockpiles and metallurgical waste suitable for recycling. Dungeons and settlements may contain crates of common materials, tools, consumables, functional devices, and blueprints. Ruined industrial facilities provide meaningful *industrial bulk salvage*, not just rare artifacts.

Implement a proper **field-extraction interface**: early portable containers and salvage kits, route marking, recoverable chests, later portable carts/pack animals, winches, vehicles, mass-based extraction via machines. Never force 90% of an adventure to be walking home because the player found 100 stone blocks. Weight/volume limits are only worthwhile when they produce interesting logistic choices; ordinary building stacks must not be unnecessarily restricted.

### 3.2 Resource substitutions and recovery

- Crushed rock -> gravel -> sand; erosion/sediment collection; reuse crushed brick, industrial slag and sawdust for appropriate applications; physically plausible byproducts.
- Repairs are indefinitely possible in principle with appropriate replacements/material inputs. No arbitrary repair-count hard stop or escalating XP tax.
- No single loot-only weapon enchantment should be required for comfortable mining or building. Let players research, craft, salvage, trade, or find essential tool functions via different routes.
- Monster/animal parts that are ordinary industrial reagents need humane, chemical, biological, trading, or recycling alternatives where physically meaningful.
- Rare magical components can remain **specialized**—an unusual boss organ should not be replaceable with common dirt—but also shouldn't be required for all mundane accessibility features.

### 3.3 Trade and price exploits

Traders and factions provide genuine conversion routes between playstyles, including sales of exploration finds, maps, salvage, fishing catches and negotiated contracts. Prices derive from regional supply, useful work and bounded demand. Avoid infinite guaranteed buyback of a fully automated cheap commodity for unlimited scarce currency. Equally, avoid a universal "adventure currency" that makes a player chase generic chests for every normal recipe.

Trade should be particularly good at letting explorers obtain ordinary building materials, and letting factories acquire maps, specialist services and local access—not simply forcing the player into tedious inventory arbitrage.

## 4. Exploration system: permanent forward frontiers, not endless reskinned rooms

### 4.1 A frontier is a change in **systems**, not only coordinates

New depths, distant regions, high elevations, water systems and optional domains must offer changing combinations of **terrain topology, hazards, ecology, architecture, physical fields, history, travel mode, available materials and hidden principles**. Increasing enemy HP is not a new frontier.

- Surface: generous home/settlement space, biomes and ordinary danger; recoverable local security.
- Engineer strata: industrial archaeology, pipe/rail/dam networks, flood control, metallurgy, historical logistics, working machines.
- Chorister strata: acoustics/resonance, ritual geometry, vertical cathedral caverns, shifting geology, rare natural magic.
- Founder strata: rule-bearing materials, geometric anomalies, computing/memory structures, nested domains, ancient machinery.
- High and distant: mountain altitudes, skyborne formations, anomalous weather, ocean trenches, orbital clues; later orbital engineering.
- **Nonvertical variation:** depth matters, but geographic distance matters too. Faults, displacement, ruins on the surface and alternate entrances keep the world nonlinear. No mandatory bedrock-to-railway-to-choir-to-founder shaft.

### 4.2 Semantic site generation and novelty

A new site must differ in *affordances*: what the player can inspect, modify, repair, harvest, manipulate, negotiate with, recover or permanently transform. Use a bounded library of excellent authored structure grammars and physical mechanisms, with robust composition and seed-specific parameters. Do not claim infinite hand-authored-quality secrets from noise functions or procedural names.

Proposed `SitePlan` contains:
- reason for existence (history/geology/ecology/economy), actual world location and interconnected spatial topology;
- physical entry routes, major voids, navigation, vertical landmarks and distant silhouettes;
- one or more useful systems (fluids, energy, logistics, chemical, ecological, arcane, mechanical);
- testable clues and physically functioning interactables;
- recoverable ordinary materials plus one or more meaningful opportunities;
- legacy state, repair/ownership possibilities and remote logical simulation contract;
- authored quality grade and successful validation checks.

Use **regional histories** for continuity; a dam, damaged rails and drowned Engineer district should logically connect. A ruin without meaningful function must be exceptional, not the default.

### 4.3 Discovery rhythm without checklist clutter

Novelty needs **rhythm**, not a secret every 10 meters. Maintain near-, mid- and long-horizon mysteries in the same regions. A short cave trip often reveals a new reaction, local mystery, unusual geology or navigational connection; a long expedition sometimes provides major breakthroughs. Do not force every expedition to deliver a guaranteed legendary item.

Proposed research questions for playtesting:
1. Does the first 10–20 minutes of a purposeful new expedition contain an interesting observation, fork, risk or useful resource even for a well-equipped veteran?
2. Do 30–60 minutes of deep exploration produce memorable events and at least a plausible *lead* to something new (not necessarily a completed secret)?
3. Across six different generated seeds, can experienced players name different actual places, not merely different biome colors?
4. How often does the player say "I've seen this exact room/puzzle/loot roll ten times"?

Do not promise numeric encounter density until capture-driven testing validates it. Rare solitude and atmospheric quiet have value.

### 4.4 Knowledge and hints: the unknown must be discoverable

For a procedural secret, maintain an internal **evidence graph**:
- physical phenomenon and rule predicates (the actual system truth),
- two or more plausible discoverable clue sources for major secrets,
- observation/test equipment that can confirm or disconfirm a hypothesis,
- alternate solution channels when relevant,
- seed-derived clue placement with reachability and save persistence,
- an in-world record through the Primer/codex (unknown entries remain hidden).

Evidence should be *about physical reality*: a geological seam, damaged pump, inscribed archive, temperature anomaly, field spectral line, creature behavior or instrument reading. Avoid arbitrary riddles with no consistent rule.

Once a player demonstrates a rule, their Primer can suggest experiments using related properties and record a **verified observation**; do not automatically reveal all other unknown rules. The external wiki becomes optional context, not an essential dependency.

### 4.5 What machines may explore

Allow mining drills, autonomous survey robots, remote cameras, sonar, spectrometers, mapping spells, explorers in vehicles, recovered maps, and advanced machine intelligence where its cost and sensing limits are defined. **No `if is_player { award_discovery }` loopholes.**

The discovery advantage of *participating* is contextual: deciding which unexplored anomaly to investigate, designing experiments, interpreting contradictory findings, changing a plan, interacting with a culture, modifying an unknown machine, choosing whether to preserve/dismantle a ruin. Players can automate pieces of that process, including later remote characterization, but discovering another physical opportunity should continue to create new meaningful decisions.

Don't hide progress behind a fake achievement token that can be claimed only by a physically present player. If an automated apparatus successfully demonstrates a valid physical rule, the scientific observation should be credited to the relevant world archive. The player still chooses what to build with it.

### 4.6 Revisit value and world state

A cleared dungeon is not "empty content". It may be usable as a waterworks, portal, greenhouse, power station, sacred site, archive, habitat or transport shortcut. The player can restore or alter its systems and connect them to their factory. NPCs respond to restored places; ecological populations change; old clues acquire new meanings with advanced knowledge.

**Avoid instant resetting of a site to renew chests.** Repeatable encounters can persist through real habitats, mechanisms, trials or controlled rituals, while treasure and historical discoveries have coherent one-time or replenishment conditions.

## 5. Creature welfare and choice: empathy is a viable playstyle

A fictional creature can be emotionally meaningful. Treat that attachment as a design opportunity. Avoid assuming "non-player entity == expendable input item." Iron golems are partly inspired by golem traditions in Jewish folklore; Axiom should develop its own respectful fictional construct cultures and agency, rather than borrowing religious imagery only to turn protectors into ingot dispensers.

### 5.1 Creature design contracts

- Define whether an entity is an inert constructed machine, mindless growth, reactive organism, social animal, self-aware construct, or person; don't assign identical welfare semantics everywhere.
- Friendly and defensive entities have consistent sensory, attachment, trust and territorial behavior; injuries and care are communicated, not exploited via opaque meters.
- Nonlethal harvests where plausible: wool, milk, eggs, shed chitin, sloughed scales, fungal fruiting bodies, molts, waste, renewable secretions, crafted substitutes and cultural trade.
- Animals need believable housing, grazing/nutrition, rest and reproduction for sustainable husbandry; dense confinement can affect health/productivity rather than trigger a heavy-handed binary punishment.
- Construct guardians may be repaired, befriended, employed, safely retired or salvaged *after destruction*; their primary optimal use is not compulsory murder for simple metal.
- Hostile combat and hunting remain legitimate play. No universal morality score or scolding dialogue. Offering humane alternatives is not equivalent to erasing danger or predation.

### 5.2 Construct protectors

A constructed guardian might protect villagers because of purpose, memory, instruction or bond. Its components are useful, but a *functioning* guardian is more valuable through sustained protection, maintenance and trust than through one-time dismantling, for peaceful players. Broken inert shells can be salvaged for useful materials. Player-built protectors can be maintained/recycled through explicitly characterized methods.

Require that basic ingots, reinforced alloys and early metal equipment remain accessible through metallurgy, geological extraction and machinery. No crucial crafting prerequisite should depend on an efficient guardian-killing contraption.

### 5.3 Farming should be powerful without being the only moral or economic route

Industrial farms, enemy containment, boss capture and magic creature production are allowed. They must obey the rules of ecology, resources and machine capacity. Players who build ingenious facilities should receive tangible returns. However, extractive loops should not uniquely supply all desirable loot categories, and nonviolent equivalents should exist for ordinary economic needs. Ethical player choices primarily lead to **different practical outputs and relationships**, not invisible virtue points.

## 6. Food, survival, health, death and inventory: remove routine irritation

### 6.1 Food and health

**Default recommendation:** no punitive sprint-linked hunger meter. Eating is purposeful: healing, short-term recovery and preparation for a known environment. Compare two prototypes before selecting whether there is small passive regeneration, food-only healing, or medical regeneration; food-only healing may create tedious consumption under frequent incidental damage.

Make 8–12 early ingredient affinities with consistent **intrinsic** signals visible in simple preparation; preservation emphasizes portability/duration; composed meals combine selected effects with understandable constraints. Examples: emberroot (cold resistance/thermal comfort), cavecarrot (night exploration/visual clarity), rainberry (hydration/chemical wash), sporeleaf (gas tolerance with limitations), moongrain (magical resonance stability). These are fictional examples; real cooking and chemical effects must be declared honestly.

Avoid a permanent optimal food. Let cooking deliver stronger or more convenient expedition effects without making elaborate meals mandatory for basic mining. Food stacks and can be prepared in batches; no extreme freshness micromanagement by default.

### 6.2 Death

Default: preserve carried unique gear, player-built structures, and the world. Death has a bounded recovery consequence that matters (e.g. temporary recoverable injury, limited crystal-heart reserve, or recoverable lost supplies). **Prototype** a "heart capacity / max-health recover" variant inspired by transcript, but don't blindly adopt a system where unlucky newcomers permanently lose survivability and enter a death spiral.

Allow explicit selectable world modes: comfortable, standard, harsh/salvage-drop, optional Noita-like permadeath delve. Physics bugs must never be used as acceptable death challenge. Guarantee rescue/recovery methods, clear death causes, and restoration paths with multiple playstyles.

### 6.3 Building QoL from minute one

- Early portable containers, categorized building supply access and bulk placement; do not gate basic organization behind deep world exploration.
- Hybrid tools with minor specialization tradeoffs rather than five indispensable tools occupying the hotbar.
- Infinite repair cycles given valid materials; durability represents wear, not inevitable irreversible item obsolescence.
- Building pieces and small roofs/trim available early; multi-voxel precision construction not gated until orbital tier.
- Auto-sort, area selection, repeat-last recipe, contextual snapping, craft-from-nearby-storage where safe, blueprints, rotate/undo/replace and simple pickup/placement tools.
- Loot auto-collection and terrain-material recovery favor exploration and creative TNT use. Provide a way to protect chosen natural areas from destructive extraction through alternatives, not by weakening mining.

Quality-of-life is not an endgame reward. Sophisticated automation and cosmetic customization are.

## 7. Progression of safety and agency rather than global enemies forever

Axiom's surface can be beautiful and peaceful because the player earned peace in a real region. Restoring a settlement, repairing a beacon, removing a nearby nest, establishing ecological boundaries, negotiating safe passage or defeating a world-linked threat may change local spawning and relationships persistently.

- Secured builds have reliable perimeter behavior; ordinary hostiles do not teleport through walls into certified safe rooms.
- A home valley can become truly safe for long-duration building and social play; threats remain outside, underground, in exceptional sites or when the player chooses risky projects.
- Night need not remain mandatory harassment after major achievements. Light, weather, creatures and ambient activities remain interesting at night without forced combat.
- Environmental accidents at the player's own dangerous refinery may still occur; player-controllable industrial risks are a different challenge than mandatory random attacks.
- Build defense, ecological negotiation and exploration solutions, not only enemy extermination. A player can choose lower-conflict world settings without disabling discovery, creative physics or commodity access.
- Regional safety is reflected in worldstate and saved coherently across chunk load, respawn, generator versions and co-op.

The world becomes easier *where expertise has genuinely transformed it*, while deeper and farther frontiers stay challenging. No global difficulty timer that pressures builders. No rubber-band attacks on conquered surface villages just to keep statistics high.

## 8. AFK economics and simulation authority

### 8.1 Never require real-life AFK to make progress

Factories can continue advancing while the player is physically elsewhere in a loaded game, through efficient graph/network simulation. No production boost from standing nearby. Distant physical simulation uses appropriate aggregate models, but actual material, energy, inventory, failure and time accounting remain authoritative.

Where single-player game is paused or closed, do not silently claim that five real-world hours of progress occurred without consuming inputs or simulating dependencies. Optional catch-up/fast-forward when sleeping can use bounded event integration and input ledgers, with explicit incident handling. The player should not be required to leave hardware running overnight to receive essential items.

A machine that earns production over 20 minutes should earn it when the player explores for those 20 minutes. It need not continue when real time doesn't progress.

### 8.2 Deterministic transactional accounting

Production state: typed input reservations, machine process/efficiency, network power/heat/fluid, bounded output capacity, stable item/cargo ownership, transaction commit at authoritative time. Sleeping states integrate conservatively where supported. Avoid duplicate materials through unload/load/lag/time warp. Factories may earn extraordinary throughput as a reward for expert design: their advantage is convenience and scale, not exclusive ability to build a normal house.

### 8.3 Anti-degeneracy without artificial restrictions

Test for copy-paste dominant strategies. Fix root incentive loops (scarcity, poor alternative rewards, infinite buyback demand, inconsistent drops, free energy) rather than introducing arbitrary mob-farm caps, achievement-only gating, or player-specific restrictions. Aggressive automation is a successful use case, not a design failure in itself.

## 9. Underkingdom lore integration: every discipline reveals a different depth

- **Engineer strata:** industrial salvage and maps, railway power/repair, hydraulic workings, rare machine architectures, historical decisions, functioning foundries, trade routes. Not merely piles of ingots.
- **Chorister strata:** resonance fields, sound puzzles and measurement, musical machinery, rituals, derived wand logic, unknown materials and social/moral remnants. Not generic mana gems.
- **Founder strata:** new Work-language operations, archival records, rule-bearing materials, unusual spatial physics, nested domains, revelation of what the planet's core does. Not simply strongest tier of sword.
- **Surface peoples:** villages can become real homes; refugees, guardians, archives, traders, infrastructure and political relationships persist. Neither faction questing nor protector welfare should be reducible to a farmable currency.

Different specializations should find unique access methods and value in the same place. A manufacturing genius repairs an ancient regulator; a mage tunes it; a naturalist identifies a symbiotic fungal circuit; an explorer follows its lost pipe network. The same underlying mechanism supports all routes.

### 9.1 Sample exploratory rewards by era

| Stage | Commodity reward | Discovery/opportunity reward | What it enables back home |
|---|---|---|---|
| Early surface | Useful crop seeds, logs, salvage crates, metal deposits | Abandoned watermill, secret wall vent, unusual organism trait | First useful automation, food medicine, underground clue |
| Early deep cave | Ore-rich seam, naturally concentrated reagents | Hydrothermal anomaly and unknown catalytic mineral | New ore refinement process, thermal tool, alternate fuel |
| Engineer ruin | Machine parts, iron stockpiles, specialist tools | Functional pump network and historical map | Regional irrigation, hydraulic factory connection |
| Chorister cathedral | Rare ceramics, stones, glyph materials | Resonance pattern, alternative spell grammar | New wand operations and industrial acoustic machinery |
| Sky/ocean anomaly | Special gas/mineral resources | New transport corridor, strange atmospheric signature | Vehicles, remote stations, new field observation |
| Founder archive | Exotic industrial salvage | Work-language verb and partial core-location topology | New design space; large advanced equipment project |
| Late-game frontier | Valuable mass cargo and specialized ores | Unique research site with unstudied material rule, ecological intelligence or astronomy | New science/magic manufacturing possibilities, new objectives |

**Never** promise unlimited qualitatively novel rules from random generation. A bounded authored system can still generate vast combinations of new practical situations; reserve genuinely foundational discoveries for curated high-quality content.

## 10. Proposed engine data contracts (not claims of existing code)

```rust
// Schemas illustrative. Map to existing world IDs, revisions and
// persistent ownership; do not add a second inventory/economy system.
struct SiteOpportunity {
    site_id: SiteId,
    region_id: RegionId,
    state_rev: WorldRevision,
    functions: SmallVec<[SiteFunction; 4]>,
    clue_refs: Vec<EvidenceId>,
    restored_functions: FunctionFlags,
    persistence: SitePersistence,
}

struct ObservationReceipt {
    observation_id: ObservationId,
    world_revision: WorldRevision,
    rule_or_pattern: KnowledgeSignature,
    evidence_source: EvidenceSourceId,
    verified_operations: Vec<OperationProof>,
    archive_owner: ArchiveId,
}

struct EconomyRoute {
    desired_category: ResourceCategory,
    method: SourceMethod,
    stage: ProgressionBand,
    requirements: Vec<CapabilityRequirement>,
    output_range: QuantityRange,
    service_kind: SourceKind, // manual, salvage, barter, machine, biology, magic
}

struct HabitatSummary {
    habitat_id: HabitatId,
    species: SpeciesId,
    population: PopulationCount,
    food_supply: Quantity,
    space_quality: FixedFraction,
    disturbance: FixedFraction,
    social_state: SocialRelation,
    next_wake_tick: SimTick,
}
```

Do not instantiate `SiteOpportunity`, `ObservationReceipt` or `EconomyRoute` once per voxel. These are sparse semantic records associated with meaningful places or discoveries. Use an existing save/version mechanism, ID authority, and event type registry. Player-facing logs may consume receipts, but shouldn't become authoritative world science.

**Integration requirements:** `PlacePlan` attaches real resources/opportunities, not loot-only labels. The Primer observes verified operations. The VM can enact world interactions only through typed authorized actions. Save/replay records discoveries, restoration, secure regions, resource ledgers, and NPC/habitat state. Nearby render/lighting/nav reflect real changes. Remote simulation does not need detailed voxel AI per sleeping creature.

## 11. Verification suite (new named acceptance scenarios)

Each scenario has a gameplay record, seed/config, setup, user or bot actions, observable events, before/after saves, optional performance data and explicit PASS/FAIL/NOT-RUN. Automated validation complements blind human playtests.

| Scenario | Setup and test | Evidence |
|---|---|---|
| `player_freedom_home_without_farm` | Two-hour newcomer attempts a useful home, repair and first automation, no hostile farm or external wiki | Playtest video/log and task outcomes |
| `explorer_building_materials` | Explorer returns from cave/ruin and constructs substantial ordinary architecture using found/salvaged/traded material | Resource accounting and playtest friction |
| `technical_factory_honored` | Expert builds ore line and scales production well beyond manual collection, without arbitrary output cap | Throughput, power, failures, process bottlenecks |
| `tnt_material_recovery` | Controlled blast vs pickaxe; compare recovered mass by declared rules and world edits | Mass conservation and visual/collider integrity |
| `science_discovery_via_drone` | Remote sensing discovers real material anomaly; player experiments or robot validates using real apparatus | No actor-identity-gated discovery credit |
| `legendary_site_not_loot_only` | Claim an ancient dam/forge after initial combat and salvage; restore useful function | Physical network effects and saved state |
| `new_deep_frontier` | Expert late-game player travels into a generated unfamiliar depth/region | Variation and usefulness analysis across seeds; blind human review |
| `coherent_rare_hint_chain` | Procedural rule unknown; discoverable multi-source evidence leads to an in-world experiment | Solver/reachability check; no external wiki |
| `humane_material_alternatives` | Nonlethal livestock/construct/chemical route supplies ordinary resource needed for progression | Comparable progression access, distinct costs |
| `golem_guard_vs_scrap` | Guardian protects settlement, can be repaired/salvaged in destroyed state; ingots have regular metallurgy path | AI, economy, repair and persistent trust behavior |
| `regional_peace_victory` | Secure village, restore defenses, travel away/return after nights | Persistent peace; threats still beyond safe area |
| `afk_vs_adventure_factory` | Identical factory runs equal simulation time with player at factory and far away; compare inputs/outputs/events | Equal production ledger within declared approximation tolerance |
| `normal_death_recovery` | Unexpected cave death in standard mode; retain unique gear and recover capability without grind/death spiral | Save, UI explanation, replay and player feedback |
| `starter_cooking_intrinsics` | Discover ingredient intrinsic through simple cooking, prepare preserved food and meaningful meal | Player infers effect from hints; situational use |
| `inventory_first_hour` | Building/exploration session with early containers, hybrid tools and salvage | Inventory-management burden observed, not just inventory slot count |
| `trading_no_infinite_arbitrage` | Unlimited automated commodity supply attempts to print scarcity currency through NPC | Supply/demand and transfer ledger finite, no duplication |
| `cooperative_discovery_credit` | Two players and a robot survey and restore site; world archive contains coherent discovery; no unique essential reward lost | Multiplayer ownership and save/rejoin |

### Quantitative targets are **hypotheses** to tune, not claimed results

- First genuinely useful automation accessible in about two hours *without guide*, retained from C-MC5.
- First meaningful environmental observation within early exploration; structured player cohort after first 30–60 minutes.
- Compare early and midgame capability access for explorer/builder, industrial player, wizard, naturalist and combat-oriented delver. Identify **mandatory dominating routes**, not a demand for equal mass production.
- Successful late-game exploration after strong factories: players voluntarily leave their bases because the world still contains physically novel opportunities and high-quality places.
- No invented benchmark values. Report toolchain/platform, player cohort size, median and dispersion of outcomes, exact failures and balance changes.

## 12. Implementation order and affected prior documents

**M0–M2:** Add benchmark and instrumentation for the resource loop, machine passive authority, early inventory UX, save identities and world-feature discovery state; don't rewrite core sim prematurely.

**M4 vertical slice:** Starter valley provides early portable storage, a noncompulsory cooking experiment, three viable ordinary-material acquisition routes, a reliable first machine, a cave with concentrated salvage, a one-hour exploratory mystery and a safe-home mechanism. Verify `player_freedom_home_without_farm` and `afk_vs_adventure_factory` immediately.

**M5 industry:** Production runs correctly while roaming; machine network supports meaningful exceptional scale and correct input/energy accounting; no currency loop makes infinite passive money mandatory.

**M6–M7 sites:** SitePlan contains site usefulness beyond loot, clue graphs, restoreable physical function and a physically coherent reason for its existence. Validate a named Engineer dam or foundry and connected cave/region at several seeds. Implement regional security and ecology that avoids hostile spawning inside established safe bases.

**M8–M9 advanced:** Distinct meaningful creature welfare interactions, learned magic/tech operations, drone discoveries, Chorister/Founder frontier variation, alternative routes through place mechanics; preserved program/exploration balance.

**M10–M12 late game:** Richer factories and megastructures remain worthwhile, but distant and deep expedition content must feature **new physical challenges and knowledge**, not just stronger versions of known items. The orbital layer, secret domains and civilizations may use curated content to maintain quality. No assumption of infinite innovation by noise.

### Changes to original C-* claims

- **C-MC5:** first automation must be achievable without AFK farms or external tutorials; assess the full starter-resource economy.
- **C-N3:** secrets are discoverable through evidence and changes to physical world; avoid arbitrary nonrepeatable loot gates.
- **C-N4:** strong builds measured by useful emergent behavior, not number of syntactically valid combinations.
- **C-Z2:** named site density measured through meaningful experience; avoid evenly spaced placeholder ruins.
- **C-S1:** Primer recognizes discoveries made through actual experiments even by machines and records the relevant evidence.
- **C-S4:** protect emotional investment, homes and archives against arbitrary irreversible loss outside explicit hard-mode choices.
- **C-F3:** optional permadeath delve does not make default world unsafe or invalidate noncombat play.

### Ownership and concurrency

Make this a **game-design/economy/worldgen acceptance addendum first**. Coding agents must audit the current real repository for player, inventory, factory tick authority, cave/resource generation, ecology and save schema before editing. One owner per hot subsystem. Build one complete playable slice; do not immediately add 100 food items, currencies, soulbound loot, species-specific AI or a second economy implementation. All claims must be backed by actual runtime or explicitly marked unverified.

## 13. Adversarial design review — failures to catch before shipping

1. "Explorer-only essence" becomes new mandatory grind → remove and reward actual discoveries/opportunities instead.
2. Factory is nerfed after 30 minutes of operation → reject arbitrary caps; respect physical constraints and paid design effort.
3. Explorer finds 400 ingots, inventory only carries 64 → early logistics and portable capacity must match rewarding finds.
4. Dungeon has 15 treasure chests but no interactive machinery/places → add function, layout, causality and secrets; less generic loot.
5. Player can print thousands of ingots from protector deaths at zero cost → let metallurgy outperform basic protector slaughter; protectors have sustained functional value.
6. Nonlethal farming produces only cosmetics while lethal farm gates machines → add real industrial substitutions and sustainable outputs.
7. A beginner loses favorite wand in lava and rage quits → default gear/repair/death policy should protect long-term attachment while retaining bounded stakes.
8. Hardcore deaths and irreversible world catastrophes are enabled silently → require explicit mode/opt-in and clear telegraphing.
9. Building player gets attacked every few minutes after regional victory → security must persist and be trustworthy.
10. A robot scientifically tests an unknown crystal, but only human click counts → forbid actor-identity-gated knowledge.
11. Player camps at the factory and waits instead of exploring because remote chunks freeze production → correct remote authoritative integration.
12. Farthest regions differ only by HP and palette → generation lacks distinct mechanics and place relationships.
13. An online tutorial trivializes an exact fixed secret chain → retain stable governing principles with seeded observable parameters, multiple methods and nontrivial sites; sharing knowledge remains legitimate.
14. To make exploration profitable, automated devices are forbidden from scouting → reject arbitrary restrictions; player creativity remains valid.
15. Food has thirty buffs, but players use the same optimal meal → introduce context-specific benefits and access, not raw numeric power creep.
16. All valuable knowledge eventually becomes manufacturable, so there's no point exploring → the planet remains vast, varied, historically specific, and new places present new combinations and societal/ecological consequences.
17. Endless worldgen claims endlessly original content but repeats five room templates → curated grammars, authored exceptional sites, novelty tests and honest content limits.
18. Rare special drop is required for basic storage or tool repairs → fundamental QoL must not be endgame gated.
19. Farming is treated as unethical by the game globally and the player is punished for technical creativity → contextual behavior and alternative rewards instead of a universal moral meter.
20. An optional ten-hour boss run is required to obtain the only building block suitable for ordinary roofs → replace gate with creative alternatives.

## 14. Coding-agent kickoff brief

> Integrate `07_PLAYER_FREEDOM_EXPLORATION_ECONOMY.md` into Axiom/Underkingdom's existing docs and acceptance gates. Audit current Rust/WGPU repository and baseline without assuming systems are complete. First implement and measure the smallest **two-path resource + one functioning site** demo: an explorer with a portable storage kit can obtain ordinary building and machine materials by caving/salvage/trade, and a technical player can obtain the same broad material categories using a working automated crusher/smelter; both can complete a small home plus first functional workstation without external guide or hostile farm. Machines must maintain authoritative output while the player is away; a nearby Engineer ruin has a recoverable physical mechanism and a grounded clue to a deeper secret. Implement tests for inventory ownership, recipe mass, remote machine integration, save/load, and site repair. Protect existing generator versions, previous save data, concurrent edits and rendering. Record user actions and native captures. Do not claim success without in-game testing. Then prototype food/health/death and humane-creature alternatives separately with human playtesting before freezing mechanics. Include the new `C-PF01..12` rows and named scenarios in the existing claims harness. Reject fake explorer-only currencies, arbitrary factory caps, and repeated empty dungeons. The deliverable is a demonstrably fun alternative to compulsory tutorials/AFK farming, not a longer design document.
