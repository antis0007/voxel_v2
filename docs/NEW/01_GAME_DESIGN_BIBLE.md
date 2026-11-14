# 01 — UNDERKINGDOM: GAME DESIGN BIBLE
**Game working title:** Underkingdom | **Engine:** Axiom | **Version:** 2026-10-26 rev U1 | **Classification:** design canon + testable product intent. Numbers marked TARGET are provisional.

## 1. Product identity and design thesis

A single planet has been built, mined, conquered and broken repeatedly. Its youngest surface is green; below it, each civilization buried the functioning machines, temples, canals, railways, experiments, libraries and errors of the people before. The player is a **delver**: a survivor-builder-explorer who goes below for knowledge and salvage and returns able to do more than before. Technology and magic are surviving dialects of a common discipline: **the Work**. The planet's core is not ordinary rock; the game must preserve the significance of this discovery, not prematurely expose it through UI/tooltips.

**Thesis:** *Matter has rules. Rules can be learned. Learning them is power.* The most valuable reward is a new capability or relationship between existing capabilities. A found rune is a new verb, a machine is a manipulable physical process, a ruin is infrastructure, a creature is a physical/biological system, and a dungeon puzzle is a problem with several systems-based solutions.

### The player promise

- In the first **five minutes**, players can mine, place, walk, look, interact, and make an obvious physical consequence; the world feels tactile and reasonably Minecraft-like.
- In the first **hour**, players have a safeish home, tools, a visible goal, one unusual material reaction, one small ruin and an introductory Primer experiment.
- Within **roughly two hours (TARGET; validated in human playtests)**, an uncoached newcomer can obtain the first useful repeatable automated process without a wiki. Not all players must optimize for it.
- Within the first long expeditions, the player discovers that an old machine, a spell and a geological phenomenon follow compatible rules.
- Across tens to hundreds of hours, the player can create sophisticated factories, wands, creatures, vehicles, programmable devices, captured ruins, remote infrastructure and enormous works; exploration continues to reveal genuinely unfamiliar rules.

### Four product pillars, always coupled

1. **Habitation:** Minecraft construction, shelter, agriculture, crafting, decoration, safe long-term infrastructure, immersive first-person traversal.
2. **Exploration and secrecy:** Noita's layered secrets, Zork's specific places and readable lore, Zelda-like interconnected dungeons, geological history, factions and cosmology.
3. **Invention and automation:** Factorio-quality logistics, multi-step industry, physically meaningful electricity/heat/fluids, modded-Minecraft magical and industrial systems, programmable electronics.
4. **Interaction and danger:** Noita material chains, Mycopunk's functional enemy parts and mobility, Risk of Rain 2's combinatorial effects, FTL's systems-level failures, immersive-sim multi-solution play.

Reject goals that serve none of these pillars or only add cosmetic stat inflation. Not every advanced feature must be compulsory.

## 2. Fundamental player verbs and moment-to-moment feel

**Core verbs:** look/inspect, move/jump/climb/swim, mine/cut/shape, place/rotate/repair, pick up/throw/contain/pour, combine/test/measure, equip/wield/cast, assemble/configure/program, grow/cultivate/tame, connect/route, salvage/repurpose, research/record, travel/map, negotiate/trade, fight/evade/trap, capture/restore. Every verb should have in-world affordances and clear feedback; avoid modal GUI overload.

**Tactile standards:** low interaction latency, clear sound + particles + damage state, stable crosshair placement, satisfying block breaking/placement across legitimate voxel scales, coherent inventory/hotbar access, excellent jumping/grappling motion and readable first-person viewmodels. Cables, pipes, beams and finer pieces should snap intuitively to the chosen world-resolution grid. Fine detail is not a requirement for routine placement.

**Three construction modes:** common block placement (simple); architectural/tool precision (half/quarter/eighth module where supported and physically genuine); advanced blueprint/schematic placement (bulk builds with materials, energy and time). Expose the true physical footprint and collision. A hero model cannot hide a phantom coarse collision block.

**Survival:** hunger and comfort should matter in journeys, not demand perpetual busywork; optional harsher needs. Severe cold, heat, gases, oxygen, pressure and magical exposure are regional hazards with inspectable causes and equipment counters. Build habitats and machines to solve them. A stable home should ordinarily be safe. No arbitrary global difficulty timer for decorating.

**Death:** Default mode preserves world and bases. Dropped gear, injuries, respawn cost or recovering the prior body are configurable. Optional delve/permadeath modes may end the active delver while leaving world marks/ruins; prohibit save corruption or untelegraphed one-hit physics bugs. Accessibility and difficulty presets define consequences, not silently different underlying material rules.

## 3. Cosmology and world history — canonical spine

### The three strata and their functions

| Civilization | Character | Evidence left in-world | Failure/history | Game consequence |
|---|---|---|---|---|
| **Engineers** (~200–600 m illustrative depth) | Steam/steel, railroads, hydraulics, power grids, mines, civic infrastructure | Recoverable dams, track, substations, foundries, records, populated ruins | Their deepest excavations breached a deeper system; cascading failure | Player can actually restore/reuse their plants, mines, lifts and transport |
| **Choristers** (~600–1500 m) | Acoustic/geometry-based arcana, cathedral-caves, ceremonial resonators, programmable runes | Viable resonant chambers, tuned stones, rune machines, animated sentries | Their schism weaponized resonance; scars remain as ravines | Player learns arcane field measurement, spell composition and alternative control notation |
| **Founders** (>1500 m) | The Work: rule-bearing matter, high-density memory, compilers and Feed networks | Crystalline archives, bizarre physically functioning artifacts, deep storage/assembly systems | Their disappearance and the planet core are ultimate mysteries | Programmable matter, a unified control model, world-scale secrets |

Depths are lore/reference bands, *not invulnerable hard-coded Y-plane gates*. Geography should have faults, uplift, collapsed passages, remote entrances and physical connections. The surface peoples descend from surviving cultures: **Salvagers** (technical reuse), **Cantors** (traditions of sound and magic), **Archivists/concents** (knowledge preservation, source of the Primer), **the Crown** (surface politics and extraction rights), and a deep-worshipping cult. Their economic interests, infrastructure and beliefs need believable material reasons.

**Sky:** daylight, moons, seasons, celestial alignments, meteor activity and a visible artificial orbital ring. Show the ring early; orbital transfer, construction and stable space infrastructure become late capability. It may initially be a different streamed simulation domain with physically defined connection and travel, not a promise of one unbroken maximum-detail 3D voxel universe from mantle to orbit.

**Ages:** environmental catastrophes, historical strata and archive inheritance. Catastrophes should generally be local and recoverable in default persistent play. Opt-in age transitions or new-game-plus can preserve/rebury archives and produce previous-base ruins. Never overwrite a save as a surprise narrative event.

**The Work:** technology and magic need not be *identical* in mechanics or presentation. Both use structured control logic and share typed interfaces. Machine firmware, glyph programs, rituals and certain Founder artifacts can compile to a common safe control IR; physical simulation remains in specialized numerical solvers. The midgame revelation is that the two traditions express a compatible language, not that the universe is just a CPU or that every formula is interchangeable.

## 4. Core play rhythm: home, delve, discover, reproduce

**Preparation at home:** refine tools; grow food/spell reagents; tune a wand; repair a crawler; produce batteries, coolant or mana vessels; inspect recovered maps; assign automation tasks. Comfort and visual ownership encourage long stays.

**Expedition:** choose route, risk and cargo; investigate sites; discover functional machines, creatures, habitats, geography and clues; improvise with magic, physics, constructions and equipment. Retreat with partial salvage is a valid success.

**Experimentation:** test unknown material properties with basic instruments or real-world cues; reconstruct a machine; solve a physical anomaly; learn a spell grammar or biological trait. A discovery must be reproducible enough to be useful or produce a deliberately explainable variant.

**Industrialization:** convert successful experiments into repeatable machines, farming, logistics and programmable controls. Complexity shifts from personal manual labor to system design. A new technology should enable new exploration, not only increase crafting throughput.

**Revisitation:** old locations change meaning as expertise grows. A decorative monument becomes a resonant instrument, a ruined dam becomes a waterworks resource, a boss can become an energy source and a mine becomes a gateway.

## 5. The Primer and codex — experiment-based learning

The Archivists' **Primer** is an optional, early obtainable in-world book that records observations and teaches by proposing safe experiments in the world around the player. It must read actual *validated game events*, not merely book page visits. For example: "demonstrate siphoning from a higher container to a lower one" passes when a real tracked hydraulic transfer occurs, not when the player places a decorative pipe. It knows if a switch interrupted current, if a rune created a particular field, or if a material reaction changed a measured quantity.

- Layers: hints → observation → hypothesis → apparatus → experiment → measured result → alternative approaches → reusable schematic.
- Guiding questions adapt to world surroundings, capabilities and progress. Avoid external LLM dependency; hand-authored semantic templates and rule-backed query/conditions suffice. LLM-driven prose, if ever used, cannot invent physical outcomes.
- The journal auto-records witnessed samples, sketches, material observations, found language fragments, historical maps, encrypted scraps, experimental designs, and failed hypotheses. Players may write and organize notes.
- Explain **why**, not merely **what**. Show provenance of quantities where the game models them. Support accessibility, controller, localization, and concise tooltips.
- Rare mysteries should remain mysterious. The codex records encountered evidence, not a spoiler checklist of all unknown legendary secrets.

**Onboarding targets:** within minutes learn material/placement; first meaningful simple machine through Primer under a couple hours; first real spell composition before extended hard combat; optional advanced circuits when ready. Validate with humans of varying background, not just an AI bot following designer breadcrumbs.

## 6. Mining, material economy, research and alternative technology paths

Core resource categories: wood, soil, stone, metals, salts, water, fuels, gases, reagents, crystals, organics, aether-bearing matter, machinery salvage, information (maps, designs, firmware, memory). The world should have *consistent* resource location logic: geological history, water transport, thermal alteration, ruins and ecological needs. Mining precision changes actual volume and yields; no material duplication across voxel-resolution changes.

**Capability graph, not strict tier ladder:** mechanics/metalworking; coal/charcoal/coke/steam; glass/ceramic and chemistry; motors/electrical; fluids/thermal and industrial logistics; arcane research/cultivation; biology; programming/control; semiconductors; high-grade chemistry/metrology; exotic materials; large vehicles/orbital. Salvaged advanced devices can be *used* without immediately granting their mass-production recipe.

**First useful automation** should be possible with one of several routes: waterwheel→crusher/conveyor, fungal sorting colony, rune-powered repetitive manipulator, mechanical clockwork feeder. Some are cheaper and simpler; others trade energy, reliability, speed and environment.

A recipe should reflect a comprehensible physical conversion where the model supports it. Offer several sensible processes for major resources (e.g., thermal/chemical/biological/magical copper extraction), and let the player optimize throughput, yield, energy, material purity and waste. Abstract real industrial wait times for fun; retain process order, controls, useful metrology and failure modes. Avoid a giant nuisance catalog of near-identical dust intermediates.

## 7. Industrial automation as spatial system design

**Required domains:** item lots/belts/inserters/splitters, inventories, storage and autocrafting; fluid pipes/valves/tanks/pumps; mechanical shafts, torque and gears; thermal chambers, boilers/heat exchangers; real power delivery (DC first, AC later, correct converters); chemical batches; signals, relays and microcontrollers; aether transport/concentration; biological growth routes; cargo transport/trains/vehicles/remote outposts.

**Game feel:** machines look and move as if functioning. A player can walk into a factory and identify what it does. Cut a conductor → affected machines stop. Divert coolant → temperature changes. Damage a rail switch → cargo reroutes or stalls. Build a better machine → measured throughput or efficiency changes because of meaningful design.

**Infrastructure salvage:** a dungeon's machines are ordinary compatible infrastructure when repaired. Why recover an ancient regulator? It can become a functioning part of the player's factory. Avoid distinct "dungeon-only" fake machinery. The best discovery is a new industrial capability with exploration implications.

**Variants and player multiblocks:** real chamber volume, wall/insulation area, exposed heating surfaces, pressure limitations, ports and topology affect operating behavior. A larger furnace is not automatically faster. Validated blueprints, configurable ports, inspectable faults, repair and salvage are essential. The player should be able to build a machine from components rather than choose only a single animated block.

**Semiconductor route:** wafer fabrication follows real categories (purification, crystal growth, deposition, lithography, etching, doping, metallization, inspection, assembly). Provide a modular 2.5D chip layout/editor with functional blocks, typed signal/power nets, routing and characterized workloads, not literal nanometer-sized overworld voxels. Smaller processes improve digital density but do not automatically improve analog/I/O/heat proportionally. Store immutable mask versions and manufacturable lots; a designed chip changes controller throughput, memory, I/O, power, heat, reliability and machine capability. Allow automated starter layouts; offer expert layer editing as optional deep play.

**Endgame Feed / programmable matter:** later-stage compilers transform real stored feedstock and power into designed geometry/items with throughput/precision/energy/complexity constraints. Complexity is based on robust manufacturing work/feature count/state requirements, not naively on compressibility of player-supplied noisy files (an exploitable metric). Feed theft/distribution, secret recipes and illicit compilers create systemic stories. Maintain explicit conservation and permissions.

## 8. Magic as both expressive improvisation and industrial discipline

**Magic source:** Choristers discovered field resonances and stabilized them in crystals, chambers, living organisms and inscriptions. An *aether/resonance field* is a **fictional but modeled** energy/information-carrying substrate; its simulation can use low-order modes or graph approximations rather than real wave PDEs across all loaded stone. Players learn instruments, thresholds, notes, geometries and interactions. Some phenomena can remain mysterious in origin but observable in behavior.

**Five equipment tiers/classes:** personal wands; specialized staffs/scepters; surveying/focusing/warding instruments; glyph journals and programmable spell circuits; placeable ritual/manufacturing devices. Any wand may be mechanically exceptional even if aesthetically plain.

**Wand execution model:** capacity, spell deck and shuffle, cast delay, recharge, stored energy, reload/mana recharge, spread, spells per cast, intrinsic effects and material/tip modifiers. Distinguish visual appearance from promises of exact capability: silhouette can hint, inspection reveals. Ordered deck traversal, multicast, wrapping, triggers, timers, proximity effects, conditional casting and nested logic need explicit tested semantics.

**Spell primitives** are composable typed operations: delivery (projectile, beam, contact, cloud, organism, field); motion (homing, bounce, recoil, gravity, orbit); life cycle (impact/timer/death/contact triggers); matter (dig, deposit, melt, freeze, erode, change); energy (heat, light, electrical, kinetic); spatial (teleport, levitate, field); biology (grow, shrink, split, summon, mutate); sensing/logic (measure, condition, target). A damage-only spell is a small part of the library.

**Execution safety:** bounded instruction/fuel budget, nested depth, spawned-entity budget, effect-provenance IDs, global event cap and aggregation of physically equivalent micro-events; author a fail policy that doesn't silently eat the player's resources. Keep powerful chain reactions where costs are explicit and the engine can summarize bulk effects. Test recursion, deck wraps, time-based triggers and damage events for duplication.

**Crafting:** local physical rods/foci/glyph interfaces/materials; found relics; biological wand growth; ancient shapes; interchangeable components where appropriate. Some magic is best performed through placeable machines and rituals. Wands can become machine programs through common typed world operators, but controllers cannot bypass wand energy/material costs.

**Hidden alchemy:** seeded recipe/condition variations with hints in texts, cultures, environmental residues and lab instruments. World seed influences conditions and catalysts but does not turn every chemistry rule into random trivia. A recipe is discoverable through experimentation and reproducible. Support cross-world knowledge transfer.

## 9. The Work: controller VM as a midgame revelation

The Work is a single **bounded control intermediate representation and virtual machine**, not a single physics engine. Surface engineers use logic blocks, relays, text programs; Choristers use resonant rune glyphs, chant-pattern diagrams and geometric sequences. They compile to the same typed safe operators where semantically compatible. Founder discoveries reveal this commonality.

**Control domains:** read sensors, arithmetic and vectors, compare/branch, event subscriptions, timed actions, typed port control, spell effect invocation with energy authorization, creature behaviors, turret logic, archive decode, measured feedback. Programs cannot directly mutate canonical voxels or inventories: they submit validated transactions via capability-gated APIs and incur operational costs.

**Programmable matter** is a privileged late Work operation with explicit matter/energy budget. **Language-as-power** influences robotic/creature utility weights through modeled behavioral interfaces; no magical unlimited mind control. **Cryptography** uses toy-but-honest, documented ciphers as puzzles; important game communication uses appropriate secure libraries, not toy ciphers mistaken for security.

**Constraints:** fixed fuel per tick/event, memory and state caps, explicit yield/sleep, no unrestricted recursion or arbitrary OS/network access, deterministic ordering for authoritative events. Provide useful debugging and trace interfaces both as player equipment and developer tooling.

## 10. Creatures, AI, ecology and biology

**Enemy anatomy:** physically meaningful limbs, sensors, drives, armor, organs and weak points. Removing legs changes mobility; damaging a heat sac can release stored heat; severing a sensory array affects detection. Avoid constructing every monster from random parts without readable silhouette/AI/attack identity.

**Ecology:** population production grounded in nests, growth conditions, migration corridors, light/moisture/food and disturbance. Behavior is causal and observable; avoid arbitrary enemies spawning in an enclosed, proven secure base. Threat escalation is **regional and player-action-linked**, not a timer punishing builders. Sophisticated players may cultivate a nest or farm a boss under explicitly defined ecology rules.

**Biological industry:** growth media, soil, fungi, living conduits, bioractors, spore pumps, creature genetics/traits, grafting and controlled mutation. A mushroom-growth spell should grow an organism, not merely tint dirt. A size modifier changes meaningful geometry, collision/attack range, nutritional and energetic costs; simulation uses authored constraints, not an impossible organic physics model.

**AI:** sensory lines and volumetric nav summaries; ground, flyer, tunneler, wall-climber and swimming locomotion; utility AI with investigate/flee/repair/harvest/attack/guard roles; NPC crews for vehicles; offline ecology aggregated into local populations. Destruction invalidates only relevant navigation/connectivity. A boss's move set must respond to damage and material/electrical states, not only a health threshold.

**Bosses** act as recoverable physical systems: geothermal foundry beast, fungal network intelligence, ancient orbital/astral regulator, giant clockwork golem. Defeat, disable, capture, reroute, exploit or farm are possible if resources/knowledge allow. Several independently interactive components, legible telegraphs, dynamic arena and useful salvage; no gigantic boring static target.

## 11. Movement, combat, FTL-style vessels, expeditions

**Movement:** responsive, momentum-aware climbing, grappling, sliding, swimming, levitation/teleportation where earned, conveyors/elevators, rails and vehicles. Avoid clunky FPS viewmodels or screen-filling held implements. Controller and accessibility variants remain necessary. Fast traversal should increase world interest, not be countered entirely by arbitrary walls.

**FTL-like vessels:** moving 3D editable voxel assemblies (crawler, rover, rail engine, submersible, airship and eventually orbital craft). Typed subsystems include energy, hull integrity, mobility, cooling, life support where relevant, sensor/control, cargo, weapon/shield and crew. A hull breach changes actual fluid/air/collision. A damaged drive changes movement. Crew use utility AI or player commands to repair, evacuate and reroute resources.

**Systems emergencies** should be occasional and meaningful: a cave crawler loses battery supply and must choose lights vs drive, a reactor overheats due to cooling failure, a bio-hull sheds organisms. The player should have clear gauges, tactical pause in solo modes when appropriate, and automation/priorities. No routine busywork maintenance for every journey.

**Delve mode:** optional high-stakes expedition run into persistent geography or special physically defined domains, with recoverable bodies, ruins, notes and archives. **Expedition mode:** travel between seeded sites with scarce supplies, risk/reward choices and genuine changes to the underlying world; 'jumps' are travel abstractions for appropriate vessels rather than free unbounded teleportation. Both use shared material/world rules.

## 12. Procedural places and immersive-sim puzzles

Generated sites are deliberately inhabited and spatially coherent: Zork-like named landmarks with *function*. A mine has hauling routes, proper ventilation and real supports. A dam holds water. A library has accessible aisles, indexing instruments, and readable navigation. Cathedrals have sound geometry/rune machines. Towns have parcel density, streets and believable services.

**PlacePlan** data: site/landscape, major masses/silhouettes, protected void volumes, room and path graphs, foundations/support anchors, typed service connections, semantic history, secrets/hints, light plan and detail sockets. Generate large masses first, preserve traversal, then structural logic, then small detail. Validate full 3D capsule access and useful sightlines; do not generate dead unlit empty rooms.

**Puzzle principle:** Every hard progression obstacle permits at least two independent coherent categories from technology, magic, physical construction, knowledge/social access and combat. Three solutions are an excellent authored high-stakes target but not a guaranteed automated property of all general procedural gates. A boss can be bypassed by clever digging where the materials/constraints allow, but some places explicitly resist certain tools with clear fiction and detectable rules.

**Examples:** flooded archive (pump / freeze / dig alternate drain); acoustic gate (tune resonance / wire speaker driver / damage mechanical lock with heavy cost); guarded furnace (combat / disable heat network / exploit environmental gas); levitating bridge (arcane stabilization / construct engineered substitute / reroute inherited machinery).

**Environmental narration:** architecture, machinery, strata, corrosion, resource scarcity, faction names, trade records, murals and physical failure evidence. Generated descriptions must be grounded in actual tags, not invent props or quests. Text-room layer adds Zork-quality short writing with controlled authored templates and sound/light/heat tags, accessible as an optional inspection mode.

## 13. Secret system: fair mysteries at five scales

1. **Minor:** hidden storage, unusual material behavior, inscriptions, enemy trait, laboratory observation; minutes, not twenty hours.
2. **Local:** physical puzzle, hidden workshop, optional side room, uncommon resonance; site-scale exploration.
3. **Regional:** buried transport, linked dams, underground fungal communication, civilization-specific riddle; several sites.
4. **Grand:** world-distributed scientific/arcane rules, astronomical conditions, lost compiler language, coordinate-free clues.
5. **Legendary:** six or more optional cosmological puzzles changing meaningful world state—new region, sky mechanics, deep access, historical revelation or technology—with spoiler-safe journals.

**Original core secret examples to preserve:** seeded transmutation under moon+temperature+ley conditions; sustained resonant chord opens a Chorister barrier; draining a genuine dam exposes a Founder antechamber; a plant grown in Founder light yields a rule-bearing product; eclipse timing aids ring access; the planetary core mystery requires both dialects and evidence from all cultures.

**Fairness:** each major authored secret has (a) at least two clue channels and one experiment affordance, (b) generated evidence placed in the world with validity checked, (c) no required unrecoverable one-shot item, (d) no fixed global coordinate that spoils all seeds, (e) repeatable or explicitly one-time consequence, (f) spoiler-safe UI, and (g) human playtest solvability. Some authored sites can be canonical landmarks; procedural placement should vary.

**Secret design anti-patterns:** moon-only riddle with no observation cues; hidden 1-pixel switch in random terrain; 'exactly seven items' ritual without lore; twenty-hour dead ends due to seed generation; a secret only documented on a wiki; pure guessing disguised as science. Maintain genuine obscurity through layers of evidence, not absence of evidence.

## 14. Player progression beats: 5 min → 200+ h (TARGET, not prescribed)

| Approx. session | Experience and capability | Reveal/engineering question | Quality gate |
|---|---|---|---|
| 0–5 min | Land, mine/place, basic shelter/building, discover one physical material behavior | Is the world really simulated? | Controls are responsive and clear |
| 5–60 min | Starter base, simple thermal/chemical effect, Primer, little ruin or cavern | Why does this material behave differently? | Tutorial readable without external guide |
| 1–3 h | Waterwheel/small circuit/crusher or an alternative first automation; first nontrivial spell | Can I reproduce this reaction? | First automation accessible and useful |
| 3–8 h | First Engineer site, dam or rail stub, cave ecosystems, stronger expeditions | What were these machines doing? | 2–3 different route solutions |
| 8–20 h | Bases expand; furnace, belts, power, spell composition, biological cultivation; Chorister echoes | How can I combine these systems? | Several satisfying machine/spell builds |
| 20–40 h | Regional facilities and cities, rune logic, vehicle component, large dungeon, first shared-VM clue | Are glyphs and circuits related? | Meaningful cross-discipline invention |
| 40–100 h | Advanced industries, factories, programmable controllers/wafer work; grand secret chains and remote networks | Can I engineer the Work? | Factory throughput and mysteries both compelling |
| 100+ h | Deep Founder works, astronomical engineering, planetary/ring mysteries, programmable matter, megastructures | What is the world made for? | Spectacular but coherent emergent endgame |

**Do not require orbit in 40 h**, and do not force players to enjoy electronics, magical farming, and boss battles in a rigid sequence. Playtest different specializations. Surviving the world remains fun long after unlocking complex systems.

## 15. Quantitative content strategy: density before bloat

At early content milestone, deliver **one integrated valley**: a real settlement/castle/mine/library family selection; river/groundwater; large caves; one functional small foundry/factory; initial flora/fauna; an excellent procedural site with three approaches; six highly distinct wands; 20–40 truly interactive material/equipment definitions; initial secret clue chain; coherent sound and lighting. Don't label the catalog 'parity' until player-tested breadth exists.

Follow by **a curated family expansion**: architecture kits, 30+ meaningful processing machines/components, 20+ biological/ecological items, dozens of spells, 10+ distinct creature roles, multiple civilizations and environments, 12 authored wand silhouette families plus many valid procedural appearances. These are scope hypotheses, not signed-off schedule commitments. Each family must have at least one material interaction or useful task beyond decorative placement where described as functional.

A strong content unit includes: readable silhouette, meaningful purpose, physical/typed interfaces, operational/failure states, fitting loot/material role, at least one secret/interesting relationship, and first-person/natural camera QA. A hundred visually random props are worth less than ten deeply interactive objects.

## 16. Games and references: adopt / reject / improve

- **Minecraft modpacks (Mekanism, BuildCraft, IndustrialCraft, Railcraft, Forestry, Tinkers, Thermal Expansion, Immersive Engineering, Botania, Aether, enchanting mods):** adopt modularity, immense capability breadth, useful factories; reject incompatible independent power currencies and five duplicate ore doublers; improve physical couplings and unified progression.
- **Noita:** adopt wild spell composition, reactive materials, terrifying discoveries and mastery; reject opaque wiki-only secret chains, fixed-world repetition and run loss as the only mode; improve discoverability, constructive uses, 3D physicality and long-term bases.
- **Factorio:** adopt throughput planning, deterministic logistics, bottleneck clarity, robust blueprints, circuit controls; reject mandatory monotone recipe inflation; improve physical habitats, magic and 3D processes.
- **FTL:** adopt functional subsystem failures, crew decisions and expeditions; reject scripted vessel-only upgrade menus; improve editable actual vessels and persistent consequences.
- **Mycopunk / Risk of Rain 2:** adopt beautiful responsive mobility, modular hardware, readable anatomy, build-chain synergies and escalating threats; reject purely numeric loot bloat and global pressure that punishes peaceful base work; improve physicochemical world consequences.
- **Zork, Zelda/Dark Souls, D&D tombs, Dishonored/Prey, Ravnica:** adopt authored-quality spaces, puzzles, traps, spatial alternatives, multiple independent solutions; reject arbitrary invulnerable locks and linear failures; improve procedural physical consistency.
- **Stephenson-inspired themes:** real enough engineering, great ideas, the Primer, archives/deep time, orbital tether, enciphered knowledge, matter compiler; translate concepts into *playable systems* rather than encyclopedic lore.

## 17. Game design test charter

- **Play clarity:** at least one significant action visible in the first five minutes, no wiki to learn the first automation, meaningful hazard telegraphs.
- **Experimental freedom:** at least two viable alternative approaches per major gate; surprise systems chains must be physically explainable. Count useful discoveries **found by testers**, not syntactic build variants.
- **Permanent consequence:** return to a site to find meaningful edits, repaired power, drained water, altered habitat and salvage in consistent state after save/load.
- **Automation:** first factory line genuinely produces/carries items, responds to power and input constraints, can be expanded without micromanagement.
- **Wand quality:** substantially different silhouettes + pixel-art sprites + held meshes, compelling base cast feedback, modular casting with safety budget; tests include inventory, first-person, item-in-world, and casting.
- **World artistry:** no 1 m chunky roofs on hero structures, no broken distant tree canopies, no uniformly yellow dungeon materials, no dungeon dead-ends from random decoration.
- **Secrets:** at least one clue-chain discoverable independently by uncoached human testers on random seeds; mystery persists without unfair withholding.
- **Performance:** stable measured frame times and bounded simulation/backlog under real gameplay and stress scenarios; never accept FPS gained by omitting terrain or corrupting physics.
