# UNDERKINGDOM / AXIOM — COMPLETE UNIFIED DESIGN & ENGINE SPECIFICATION

**Compiled master (2026-10-26).** Sections are maintained individually as authoritative source documents. All capabilities and benchmarks are design proposals until verified in the real Rust/WGPU repository.

## Contents

- README_START_HERE.md
- 01_GAME_DESIGN_BIBLE.md
- 02_ENGINE_ARCHITECTURE.md
- 03_ART_CONTENT_AND_WORLDGEN.md
- 04_IMPLEMENTATION_AND_VERIFICATION.md
- 06_REFERENCE_CONTRACTS_AND_SCENARIOS.md
- 05_CODING_AGENT_STARTER_PROMPT.md

---



---

<!-- BEGIN README_START_HERE.md -->

# UNDERKINGDOM / AXIOM — Integrated game-and-engine specification
**Edition:** 2026-10-26 | **Status:** authoritative *design proposal*, not implemented or benchmarked | **Audience:** game director, graphics/physics engineers, procedural world artists, Claude Code and other agents.

> **Thesis:** Matter has rules. Rules can be learned. Learning them is power.

**Working names:** *Underkingdom* is the game's working title taken from the supplied bible. *Axiom* refers to the Rust/WGPU engine and visual-development project. Names are not final. This package combines the supplied **Game Bible and Plan-Generation Protocol**, the previous Axiom design discussions, the Generator-55 overhaul and the Magitech Art Production Pack. It preserves the foundational lore and deliberately *revises* several proposed mandatory acceptance claims.

## Read in order

1. **`01_GAME_DESIGN_BIBLE.md`** — fantasy, player verbs, survival, exploration, civilizations, secrets, magic, devices, technology, bosses, world-generating logic, concrete content and playtest criteria.
2. **`02_ENGINE_ARCHITECTURE.md`** — authoritative state, physical voxel scales, streaming, world LOD, rendering/WGPU, sparse material and structural physics, machine networks, VM, modding, saves, profiling and hardware constraints.
3. **`03_ART_CONTENT_AND_WORLDGEN.md`** — visual style, material/shader grammar, pixel-art textures, Blender production, held wands, Generator-54/55 integration, environments, asset catalogue, and visual acceptance.
4. **`04_IMPLEMENTATION_AND_VERIFICATION.md`** — acceptance claims derived from the supplied C-* claims, phased milestone DAG, high-value spikes, test scenes, budgets, AI-agent plan protocol and prompts.
5. **`06_REFERENCE_CONTRACTS_AND_SCENARIOS.md`** — typed world/schema examples, algorithms, physical conservation contracts, procedural-place and secret-rule RON sketches, concrete multi-scale QA scenarios.
6. **`05_CODING_AGENT_STARTER_PROMPT.md`** — paste as Claude Code session opener **after attaching 01–04 and 06**. It demands actual repo inspection, an incremental vertical slice, no spurious edits and proof.
7. **`UNDERKINGDOM_COMBINED_SPEC.md`** — generated concatenation of all files for offline searching and providing as one large AI context document.

## Precedence and provenance

- **Original user bible:** `Pasted markdown(20261026-113904).md` / uploaded source. *Narrative canon and desired systems* are preserved unless explicitly marked revised below.
- **Earlier Axiom documents:** `Generator_55_Places_Quality_Overhaul.md`, `Axiom_Magitech_Art_Production_Prompt_Pack_2026-10-26.md`, and the recovered `THE LIVING ENGINE — MASTER GAME-SYSTEMS SPECIFICATION v2.0`. Their proposals inform implementation, not evidence that functionality exists in the target repository.
- **Technical web references:** official or author-maintained references cited by URL in section 02 / sources at the end of 04. Their capabilities must be rechecked against the project's locked dependency versions.
- **No local source audit:** The active Generator-54 R7 local Rust worktree, executable and Blender desktop are **not directly accessible in this session**. No code edits, Blender modeling or native benchmark runs are claimed. The separately connected older `3d-falling-sand-prototype` is **not** a verified substitute for the active workspace.
- **Not a guarantee:** "Best voxel game ever" is a directional target, not an evidence-based claim or achievable output of a specification alone. Every superiority claim requires measured and playtested evidence.

## Decisions at a glance

| Decision | Adopt | Explicitly reject |
|---|---|---|
| Game shape | Persistent open-world survival/build/automation with optional risky delves, seeded secrets and discoverable ancient cosmology | Mandatory roguelike resets destroying long-built bases |
| Presentation | Intentional SNES/GBA pixel-cluster materials on readable Minecraft-compatible 3D geometry; cinematic but stable lighting | Photorealism, screen-wide pixelization, highly noisy AI textures, three generic glowing brass wands |
| World authority | Immutable procedural base + transactional sparse exact edits + authoritative dynamic objects + versioned derived caches | A global minimum-size dense voxel array; renderer mesh as source of truth |
| Scale | Adaptive block/storage, render, collision, numerical and UI/workbench scales with explicit conversion rules | Assuming every concept uses one voxel size |
| Performance | Budgeted, causal, sparse active work; compact machine/belt graphs; GPU-driven raster with reliable coarse proxies | Full-fidelity universe simulated at 60 Hz, unlimited update queues, compulsory experimental GPU features |
| Magic + tech | Typed physical couplings; two expressive frontends and one bounded shared control IR/VM | Unbounded energy conversion loops; magic as reskinned electricity or isolated spell list |
| Progression | Capability graph with alternate routes, reusable blueprints, Primer experiments and world secrets | Generic forced linear tech ladder / long craft waits / wiki-required secrets |
| Delivery | Vertical slices, measurements, playtests, migrations, explicit ownership | Giant speculative rewrite or compile-only acceptance |

## Vocabulary

- **World Kernel:** authoritative spatial state/query/transaction interface.
- **Brick:** small fixed-capacity cache-friendly material/occupancy volume; *not* synonymous with a Minecraft block.
- **Region:** scheduling, persistence, streaming or simulation grouping of bricks.
- **Scale level:** power-of-two spatial cell size; distinguish LOD from actual fine edits.
- **Derived artifact:** disposable rendering/collision/nav/lighting cache with dependency revisions.
- **Simulation island:** connected active domain for a particular physical solver, not universal actor ownership.
- **Typed port:** connector carrying declared matter/energy/signals with deterministic debit/credit semantics.
- **The Work:** ancient discipline whose glyph and engineering control expressions compile to a shared intermediate language, with typed world operators.
- **Primer:** early in-world adaptive experiment book, grounded in witnessed simulation events.

## Minimal command to use this pack

Give an agent `05_CODING_AGENT_STARTER_PROMPT.md` plus the other files. Require it to produce an **evidence-based baseline and one tested art/world/gameplay vertical slice** rather than implement the entire spec at once.

## Known source gaps

The uploaded bible names `ENGINE_CORE_PROMPT_v2.md`, `MOD_ADDENDUM.md` and `ADDENDUM_B_HARDWARE.md` as additional architecture inputs. The exact three named files were **not found in the accessible conversation/library search** for this synthesis. This U1 pack therefore incorporates available prior science/worldgen/art specifications and labels unverified H0/H1/H3 hardware assumptions; it does not pretend to have audited the missing three files. When they are supplied, reconcile their feature matrix and any actual hardware limits before coding.


<!-- END README_START_HERE.md -->


---

<!-- BEGIN 01_GAME_DESIGN_BIBLE.md -->

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


<!-- END 01_GAME_DESIGN_BIBLE.md -->


---

<!-- BEGIN 02_ENGINE_ARCHITECTURE.md -->

# 02 — AXIOM: RUST / WGPU ENGINE ARCHITECTURE
**Revision:** U1 — 2026-10-26 | **Scope:** executable design decisions with named alternatives/spikes | **Important:** sizes/budgets are starting hypotheses, not measured values. Existing repository conventions take precedence until reviewed.

## 1. Performance philosophy and ground rules

The engine must represent an enormous 3D destructible planet, elaborate local materials, a dense industrial base and a mechanically complex creature **without processing all stored matter every tick**. Computation should scale with visible surfaces, active material regions, active network boundaries and causal events. The canonical world remains exact about meaningful edits; derived geometry, collision, lighting and remote simulation may approximate detail under declared contracts.

**Five independent resolutions:** (1) persistence/storage, (2) rendered geometric LOD, (3) material numerical update, (4) collision/navigational precision, (5) nested machine/editor scale. Never treat them as one mutable chunk LOD. A 1 m construction block can have a true locally editable 1/8 m architectural eave while a semiconductor design uses a separate micrometer/nanometer-inspired *nested editing domain* with summarized machine behavior in the overworld.

**Hard invariants:** unique authoritative owner for every quantity; correct world-space volume under refinement; deterministic accepted edit ordering where required; no stale mesh/collider publication; no lost saves/edits; no persistent world gaps; bounded frame/job/VRAM cost; visual effects do not create authoritative material; physical simulation cannot silently invent resources during sleep/wake.

**Avoid:** Bevy rewrite, dense global voxels, universal SPICE/Navier–Stokes, mesh-per-cell or rigid-body-per-cell, unconditional GPU readback, shader-on-every-voxel, authoritative geometry derived from distant heightfields, a single giant octree for all systems, simulation of all machines at 60 Hz, and a data-driven system that hides unmeasured O(N²) traversal.

## 2. Architectural layers and ownership

```
Player / NPC / scripts → validated Commands / capability checks
                                  │
                       Authoritative tick + ordered transactions
                                  │
          ┌───────────────────────┴──────────────────────┐
          │                                              │
     World Kernel                                   Object / Entity authority
 (generator + sparse exact edits)             (dynamic voxel bodies, inventory,
          │                                      machines, crops, creatures)
          └───────────────────────┬──────────────────────┘
                                  │ world-change journal + typed events
        ┌─────────────────────────┼─────────────────────────────────┐
        │                         │                                 │
 Physics/material islands    Compiled port networks         Derived spatial caches
 granular/fluid/heat/...      power/belts/fluids/VM         mesh/collision/nav/light
        │                         │                                 │
        └─────────────────────────┼─────────────────────────────────┘
                                  │
                read-only / bounded-snapshot GPU staging
                      wgpu visibility/render/compute
                                  │
                      pixel-art presentation / UI
```

### Proposed Rust boundary modules (not necessarily separate crates)

- `world_coords`: absolute integer coordinates, scale hierarchy, valid transformations and relative camera coordinates.
- `world_kernel`: canonical occupancy/material ownership, queries, edit transactions, persistent overlays.
- `world_store`: segment/chunk file format, atomic checkpointing, version migrations, journals.
- `world_plan`: generator semantic passes, civilizations, POIs, secrets, place graphs; pure deterministic generation.
- `world_jobs`: budgeted scheduling, duplicate suppression, dependencies, revision checks, cancellation.
- `world_materials`: property tables, reaction rule indexing, cellular islands and aggregate numerical fields.
- `world_bodies`: rigid assemblies, connectivity, fracture, joints, collision proxy ownership.
- `world_networks`: typed ports, machine graphs, power, heat, fluids, belts, kinetic, item transactions.
- `world_vm`: common typed IR, runtime sandbox, authoring frontends for glyphs and machine programs.
- `world_ecology`: populations, creature biology, growth, dynamic nav, utility AI, crews.
- `world_render`: WGPU device/feature caps, geometry arenas, culling, LOD residency, lighting and VFX.
- `world_assets`: asset metadata, atlases, Blender conversion, schema compilation, validation.
- `world_diag`: capture receipts, input/event replay, CPU/GPU scopes, performance/failure artifacts.
- `app`: player, UI, input, camera, audio, save/load presentation, mode dispatch.

Consolidate trivial modules and preserve existing code where the actual repository proves they work. Cargo crates are not themselves a quality metric. **Dependency graph must be acyclic at the core library level; runtime feedback among interacting simulations is allowed through typed tick stages**, not circular Rust crate imports.

## 3. Spatial coordinates, voxel scales and planet topology

### Canonical address contract

Use signed 64-bit coordinates for base spatial indexing, with a documented base **quantum** and aligned extents. Proposed first test: 1 m primary building module, with dyadic refinements to 0.5, 0.25, 0.125 m (and optionally smaller only in special volumes). A physical region is `[min,max)` in absolute world units and owns a material occupancy distribution. `RegionKey` includes origin and scale level; mixed-resolution neighbors share exact integer-aligned faces.

**Important:** a 0.125 m cell occupies 1/512 the volume of a 1 m cell. One cubic kilometer at 0.125 m would be 512 billion cells. Do **not** allocate such space globally. 1 km³ at 0.25 m is 64 billion cells (256 GB at 4 B/cell before metadata).

Proposed type sketch:

```rust
struct WorldCellKey { origin_units: I64Vec3, level: u8 }
struct VolumeAabb { min_units: I64Vec3, max_units: I64Vec3 }
struct RegionRevision { region: RegionId, revision: u64 }
struct OwnedMaterialVolume { material: MaterialId, units3: u64 /* or wider */ }
```

Numeric units, fixed-point widths, overflow checks and huge-world extent must be chosen in an ADR. Use u128/wider intermediate multiplication where needed. Perceived camera movement uses chunk-local positions plus origin rebasing; GPU matrices operate on small relative f32 values. Avoid subtracting huge f32 world coordinates directly. Physics uses local islands, rebased anchors and robust contact tolerances; authoritative world positions remain stable.

### World topology research gate

The lore specifies a **single planet with a ring and deep core**, but that does not mandate globally curved voxels on milestone one. Choose by ADR after a feasibility spike:

- **A:** vast planar/local-coordinate voxel continents with planetary framing; easiest for Minecraft-scale build, huge procedural underground, distant horizon.
- **B:** cube-sphere/patch-based planetary voxel topology with local tangent frames and spherical gravity; more complex boundaries, physics and caves.
- **C:** hybrid geodetic world regions joined through explicit geography/orbital transfer; richest long-term route at manageable initial cost.

Do not promise physically seamless digging from surface to opposite hemisphere or literal one continuous mantle-to-orbit render without choosing/benchmarking planet topology. Ring/space are independent simulation domains with explicit transform, atmosphere transition and transfer semantics.

## 4. Storage: sparse exact world + compressed derived summaries

### Canonical layers

1. **Procedural base** `base(seed,generator_version,region)`: immutable/deterministic, generated on demand. Older generator versions remain addressable.
2. **Persistent edit overlay:** canonical placed/removed/sculpted material, fluid/source changes and construction at authored resolution. Sparse, journaled, exact. Edits never disappear merely because a visual LOD cannot represent them.
3. **Persistent dynamic objects:** machine inventories/heat/charge, voxel assemblies, organisms, ownership and dungeon mechanisms with stable IDs.
4. **Derived caches:** occupancy mip summaries, coarse geometry, meshlets/quads, collision, nav, light, volumes. Versioned and replaceable.
5. **Runtime islands:** hot dense bricks/field pages only for locally active numeric simulation; activation is a state conversion, not a second permanent copy of world authority.

**Brick candidates:** 8³ or 16³ dense hot bricks; 32³ or 64³ grouping for streaming/mesh jobs; homogenous regions stored as one material/phase; palette-bitpacking, RLE or dictionary as appropriate for cold data; compressed edit spans and bitmask occupancy. Do not assume a universal sparse octree wins hot kernels. Benchmark flat hash→brick indices, paged trees and hierarchical masks.

**Brick header (illustrative):** region+scale key; compressed-form enum; palette reference; dirty masks; wake flags; per-domain revision and dependency bounds; active surface/contact mask; packed physical property table references; optional field slab handle. Avoid per-cell heap allocation. Hot material state may use SoA arrays, bitplanes and compact material IDs. Separate persistent state, immutable material definitions and transient solver scratch.

### Transaction rules

A `WorldCommand` executes through validation, authorization/resource reservation, deterministic conflict ordering, apply-on-authoritative-state, stable object allocation and change record emission. Adjacent edits are coalesced for **derived rebuilds**, not silently combined when it would alter game-visible timing.

```
validate(Command)
  → reserve resources and ownership
  → apply to canonical world/objects (atomic gameplay step)
  → append journal + version changes
  → invalidate dependent spatial summaries
  → enqueue bounded derived rebuilds
  → publish only if source dependency revisions are current
```

Every artifact includes **neighbor/dependency revisions** as needed. A chunk mesh built from neighborhood voxels cannot be considered valid using only its own chunk's revision. A powered gate must stop when its line is cut even if its visual mesh job is pending. Failed transaction restores reservations. All transfers of fluid/item/energy have one source and one destination with atomic or explicitly delayed commit.

### Save format

Use versioned manifests plus append-only write-ahead journal/delta files and atomic snapshots (write temp + sync/rename at safe intervals). Stable identifiers separate world objects, voxel region, entities, machines, graph/network, design revision and GPU handle; GPU handles never persist. Store seed, generator version, content pack version/hash, mod registry mapping, RNG streams, relevant transactions, background simulation events and binary data integrity CRC/hash.

Persistence test matrix: incomplete save/crash recovery; refines→unload→reload→coarsen; generator 53/54/55 regression where applicable; two agents edit adjacent brick seams; fast teleport while saving; mobile assemblies crossing region boundaries; old saves never overwritten without backup/migration prompt. Migrations preserve player ownership and resources and must be idempotent.

## 5. Hierarchical rendering and LOD correctness

### Baseline: GPU-driven raster, hybrid geometry only where valuable

- Cubic structural terrain: packed face/quad representations; binary greedy meshing where shape/material constraints allow; compact vertex pulling or equivalent; atlas/material id, normals and lighting data in small GPU-friendly records.
- Natural/organic smooth surfaces **only when art/geometry demands them**: Marching Cubes + Transvoxel for compatible 2:1 boundary transitions, or Dual Contouring with proven transition handling. Do not force SDF on cubic architecture.
- Distant proxies: same canonical generation/edit data, conservative mesh silhouette and occupancy summaries; no separate unrelated heightfield pretending to be identical to near cave/overhang/arch terrain.
- High-detail special objects: curated low-poly models, sparse voxel bricks or optional direct volume traversal by measured choice. Most 3D assets can be lightweight meshes with honest collision rather than hundreds of destructible fine voxels.

### LOD-residency contract (critical for reported horizon problems)

1. A visible spatial region must have a resident **valid** parent proxy or appropriate leaf mesh (except intentional visibility clipped by camera/frustum/occlusion).
2. Request new child LOD; build mesh from versioned world state with neighbors; upload; verify GPU residency; transfer visible coverage; retire parent only after replacement is usable.
3. If a chunk becomes unloaded, the parent/fallback must cover it before child retirement. Never present an unmodified base proxy on top of player edits that change the visible shape.
4. Use neighbor level restrictions and transition/stitch geometry. Morph geometry/normals or use controlled fades without z-fighting, cracks, light leaks and misleading collision silhouettes.
5. Persistent macro-structures (arches, caves, bridges, towers, trees) must remain recognizable as parent features. LOD does not erase significant topology because a heightmap lacks it.
6. Track temporal histories; LOD swap, large terrain edits, exposure changes and rebasing invalidate/reactivate relevant TAA/upscaler/Hi-Z history conservatively.

**Anti-pattern:** radial discard + sinking the horizon mesh through resident chunks. It hides cracks rather than agreeing on geometry. The known legacy tree LOD disconnected canopies must be diagnosed at generation, proxy/culling/parent or alpha stage, never masked with fog.

### Draw pipeline

```
CPU: canonical view → streaming candidates → job budget → shared GPU arenas
GPU: instance/region bounds → frustum → previous/conservative Hi-Z visibility
   → visibility compaction → indirect draw stream → depth/material/shadow
   → lighting/volumes/transparent effects → temporal resolve → UI
```

Use conservative bounds with camera acceleration, edits or stale occlusion history. Indirect multi-draw/count and GPU timestamp features are optional via `wgpu::Features`; feature probe and fallback. Mesh shading/ray queries in `wgpu` 30 are experimental and cannot be core requirements. The renderer may run on native D3D12/Vulkan; OpenGL/no-compute fallback need not reproduce high-tier effects if it provides safe reduced fidelity. No need to rebuild all geometry into GPU meshlets before first proven indirect-draw baseline.

### Memory/resource model

Persistent pooled GPU geometry and instance arenas, versioned allocation handles, deduplicated material/texture atlases, upload staging ring buffers, deferred frees after fences/completion, per-frame upload budget, high/low watermarks, compact residency telemetry. When requested LOD exceeds memory, preserve valid coarser coverage and prioritize view-critical surfaces over distant ornaments. No per-chunk buffer creation/destruction every edit.

**Asset material scheme:** intentionally authored pixel albedo; material masks (wetness/roughness/metal/heat/rune) and stable textures; atlas padding and mips; world-aligned texel grammar; no photorealistic noise. Near pixel materials and distant filtered mip levels balance pixel identity with anti-shimmer. Decals and variations cannot jitter in world coordinates.

### Lighting & visual style

Maintain a high-quality conventional raster foundation: stable directional/point shadows, selected contact AO, sky/atmosphere, physically coherent fog, screen-space or probe-based indirect light with bounded updates, stylized energy emission. Distinct cave light compositions; avoid uniformly yellow torch-washed underground stone. Emissive effects affect surrounding light as budgets allow, but gameplay material temperatures do not depend on shader color. Ray-based GI and mesh shaders remain optional research paths.

Material classes: stone/strata, wood, metal/corrosion, ceramic/glass, fluid/wet, crystalline/aether, fungus/biology, arcane/kinetic field, soil/organic, cloth, atmospheric/volumetric. State may alter appearance: temperature, moisture, corrosion, charge, failure, mana and field resonance. Support close/medium/far effects levels. The default block shader is inexpensive.

**Shields:** use original localized Fresnel/event ripples and anchored material pattern, bounded hit records and per-emitter charge; kinetic facets, electromagnetic conductive branches, arcane glyph lattice, biological membrane, crystal fractures. Visible topology follows emitter anchors and damage, not an always-on neon hex sphere. No verified publicly available proprietary Mycopunk shield shader source should be claimed.

## 6. Active sparse material simulation and adaptive time

### Material registry

Material definitions describe density, mechanical strength, hardness/fracture, flammability/ignition, thermal capacity/conductivity/phase change, permeability/viscosity where relevant, electric conductivity, chemical family/reagent properties, optical/emissive and *explicit fictional* arcane couplings. Persistent cells refer to shared material IDs; dynamic fields store only active state (heat/temperature derived from enthalpy, damage, moisture, composition, charge, pressure when supported).

**Reaction matrix rule:** All material combinations have a defined resolver result (often inert/no effect). Reactivity rules are indexed by tags/properties/conditions and compiled into fast lookup dispatch. Do NOT enforce >=30% of all N² material pairs as nontrivial reactions; it would produce expensive, incoherent chemistry and block content growth. Instead test *curated* interaction breadth: each signature material must have at least several meaningful relations and no unexplained gaps in intended chains. Reaction fronts/contact surfaces are a better work metric than all interiors.

### Solver families (bounded and honest)

| Domain | Candidate solver / representation | Hot activity | Cold/sleep contract |
|---|---|---|---|
| Static solid | Sparse immutable occupancy/material | Event-driven edits | Zero routine per-cell updates |
| Sand, snow, rubble | 3D cellular local movement with density/friction/piles; ballistic ejection for fast sprays | Active surface/bricks | Stable pile sleeps |
| Water/lava locally | Conservative finite-volume/cellular pressure/flux or an experimentally chosen scheme | Active interfaces, moving front | Coarse basin/tank or mass/level state |
| Large reservoirs | Compressed connected cavity/volume, level, pressure head and links | Changing connectivity/valves | Aggregated conserved volume |
| Gas/smoke | Sparse transport and pressure zones; visual particles separated | Moving fronts and occupied regions | Coarse compartment state |
| Heat/fire | Enthalpy/material interfaces; front propagation and thermal events | Gradient/interface regions | Analytic cooling/coarse time step where valid |
| Chemistry | Contact-driven reaction registry with reactant/product debit and heat | Active contact sets | Dormant concentrations retained |
| Structural | Incremental connectivity + selective load/fracture checks | Local supports and detach | Stable loads/structure graph |
| Organic growth | Resource/temperature/light-limited growth events | Growth fronts and nearby organisms | Scheduled coarse biological change |
| Radiation/field | Bounded spatial source/attenuation, solver chosen per class | Field emitters, modified topology | Cached regional field/mode |
| Arcane resonance | Cheap source/mode graph, sampled intensity and field properties | Changed resonators, sustained spell fields | Cached modes and periodic refresh |

Avoid a global PDE solver for the entire world. In world interfaces where precise pressure/fluid momentum matters, activate a local domain with numerical tolerances and bounded complexity; elsewhere approximate in physically intelligible terms.

### Conservation at mixed resolution

A cell's volume is `(world_size(level))³`. Restrict fine-to-coarse by conserved extensive quantities (mass, enthalpy, momentum, species inventory) and recompute intensive quantities (temperature, pressure) from totals and volume/material parameters. Do **not** simply average temperature when different heat capacities/densities matter. Across coarse-fine face boundaries compute one authoritative integrated flux and debit/credit both sides exactly (integer/fixed or tolerance-accounted numerics).

**Prolongation:** refining a coarse mixed cell must reconstruct plausible state without inventing matter/energy. Preserve exact micro-edits in overlay and keep subcell occupancy detail where needed for topology. If hidden microscopic configuration is ambiguous, use deterministic reconstruction plus conserved reservoirs; only infer what model permits. A physics LOD transition cannot permanently erase a player-created 0.125 m tunnel.

### Time model

- Authoritative fixed simulation tick proposed 60 Hz for gameplay actions; test 30/60 alternatives on hardware. Rendering interpolates; some local collision/projectile/force updates substep where stability requires.
- Material islands and compiled networks use event-driven ticks, variable substeps or coarser batch advancement according to declared validity. Separate **simulation time** from wall-clock/UI time. Offscreen factories advance using preserved inputs, capacity and bounded scheduled events, not speculative free production.
- GPU compute suitable for large coherent active regions, not small random networks or gameplay logic merely because GPU exists. Keep compact event exchange; avoid full state readback every frame. Deterministic CPU reference for rules requiring replay/multiplayer; visually rich GPU particles need not be bit-identical across devices.
- Physics island activation priority depends on potential consequences, velocity, hazard, player proximity, visibility, topology changes and available work budget. No indefinite starvation of saves or gameplay-critical pending events.

### Parallel correctness

Noita's 2D four-phase checkerboard update is a useful reference but not a universal 3D solution. Define conflict-free writes with region ownership, bounded neighborhood/halo, multi-phase spatial coloring or buffered cross-region transfers and barrier synchronization. For 3D CA, update phase count follows write radius and dimensions—prove it, don't assume four. Maintain seed-independent/discrete ordering where required. Use property tests, ThreadSanitizer where practical and randomized boundary stresses.

### CPU/GPU numerical authority

Use exact integer identities and ownership, integer material counts/volumes, quantized energy and charge where gameplay budgets demand conservation. Not all physics state must ban floats: local pressure and rigid-body simulation can use f32/f64 with bounded accuracy, deterministic step ordering where feasible, and quantized authoritative commit. Cross-platform bit-exact GPU floating-point is not a realistic universal promise. Explicitly describe numerical error and conservation policies in each solver's SDD.

## 7. Structural physics, moving voxel assemblies and collisions

**Static world** retains simple collision proxies derived from authoritative occupancy; Rapier/Parry's voxel collider can be evaluated as a starting point, including local `set_voxel` updates and internal-face awareness. Version-check collision region changes, and do not block the render frame rebuilding million-triangle colliders. Shape choice is heterogeneous: voxel collision for cubic static surfaces, triangle/compound simplified shapes for irregular/moving geometry and exact local volume queries for editing.

**Structural stages:** local edit → anchored connectivity/support dirty region → constrained support/update → detach identified component (if relevant) → create moving object-local voxel volume with independent transform → broadphase simple body → narrowphase relevant contact region → optional secondary fracture into limited children → settle/fuse/recover/reclaim static ownership. Prevent material double-ownership when turning terrain into a moving structure.

**Incremental topology:** union-find is useful for newly added structural connections but does not alone support deletions. Use region-local flood/split checks, cached component boundary connections, and hierarchical deferred global reconnection where necessary. Limit huge collapse fanout via cohesive chunks/aggregated debris; independent rigid bodies only for components with visible/gameplay consequence. Conservation and energy dissipations are modeled within chosen physics resolution.

**Movable craft:** ships, trains, rovers, airships and golems can contain editable object-local material, explicit utility networks and compartment summaries. Damage localized to physical segments affects networks, sensors, motion or hull pressure. Crew orders act on actual reachable modules; pathfinding uses object-local nav and connectors to world, rather than voxel-by-voxel AI searches across every ship compartment each frame.

**Force/collision consistency:** physical dimensions and mass depend on real volume, density and additional components, not voxel count. Change of grid scale doesn't modify object momentum or resistance. Broadphase overlaps with island bounds; narrowphase only visible/active surfaces; reliable raycast from camera and hands. Physics authority can be separately ticked from visual smoothing; avoid dynamic polygon rebuild at every minor texture variation.

## 8. Machine networks: compiled sparse processes instead of per-voxel updates

**Rule:** world voxels author machine *geometry and connections*. Actual machine functions compile into compact ports, graphs, segments, tanks, thermal zones and process state that do not require updating every shell block.

| Network | Core abstraction | Behavior under edit |
|---|---|---|
| Item belts | Packed transport-line runs with ordered gaps/items and shared progression (Factorio inspiration) | Split/merge affected run; preserve items and order |
| Inventories | Unique inventory/lot owners, capacity, transactional transfers | Broken container drops or retains exact items according to explicit policy |
| Fluid pipes | Control volume nodes, hydraulic resistance and pumps; typed species/pressure/temperature | Cut port disconnects; inventory fluid remains in real surviving segments |
| Mechanical | Shafts, drive/bearing edges, torque/speed and rated losses | Shaft failure severs branch and changes machines/load |
| DC electrical | Typed positive/return paths, resistance and source/current limits, small bounded nodal/MNA solve | Wire removal invalidates branch immediately |
| AC electrical | Averaged RMS/frequency, phase/impedance, rated transformers and conversion losses | Recompute affected island, not world-wide waves |
| Heat | Energy/enthalpy nodes with thermal links and emitters | Wall area, contact or insulation change alters correct conductance |
| Mana/aether | Field sources, capacitors, channels, resonator graphs, explicitly costed machine ports | Damaged focus reroutes/stops local magical flow |
| Logic/signals | Evented typed IO + VM/controller program | Program runs bounded instructions, cannot directly mint energy |
| Chemical batches | Ledger of mass/species/temperature/catalysts/process stage | Real changes to heat/input affect yield and throughput |
| Biology | Growth compartments, nutrient/oxygen demand and scheduled traits | Conditions affect growth and harvestable product |

**Port schema** (RON or equivalent data-driven format) must have stable `(ObjectId,PortId)` identity, domain, direction, capacity/rating, connect rules, optional channel/phase and pressure reference. Explicit converters couple domains using debit→credit transactions. Not every energy pair needs a converter. A closed loop of *passive/ordinary* converters must have accounted inefficiency and cannot generate unbounded energy; fictional magical source behavior is separately defined, metered and constrained.

**Sample electrical physics:** 48 V source, load 4.8 Ω, round-trip conductor 0.2 Ω → 9.6 A, 46.08 V across load, 18.432 W conductor loss. Show meter diagnostics for disconnected supply, open return, overload, brownout, thermal fault. Resistive conductor should heat with `I²R` within declared model. Ordinary convenience paired-conductor blocks reduce placement tedium without inventing an invisible circuit return. Passive transformer cannot act on DC; use rated DC-DC converter separately.

**Conveyor complexity:** join many ordinary belts into long transport lines so stable packed runs update primarily at their boundaries; transfer at inserters/splitters/edits. Rendering close-by items may be instanced while authority remains compressed. Splitting/removing a belt midway retains exact cargo ownership and reserves placements. Never allocate one rigid body for each throughput item.

**Multiblock example:** a variable-size furnace is a validated connected chamber with refractory lining, burner/heating surface, volume, geometry-dependent loss, actual inputs/outputs, exhaust and a small thermal state model. Small and large designs should differ for physically comprehensible reasons. Cache structure validation until touched, mark it invalid safely during reconstruction, and preserve input batches/charge when compatible portions survive.

**Digital design:** functional module editor → validated typed nets/macros → characterized performance/power/heat/failure → saved mask/version → fabricable wafer lot. Use nested 2.5D fabrication grids. The actual processor's computational capacity can determine bounded VM throughput, memory or I/O, but not depend on emulating every transistor toggle in ordinary play. Timing during user interaction follows gameplay-compressed but real causal process stages.

**Matter compiler:** feedstock reservoir, real mass/species, authoring blueprint, bounded assembler rate, power costs and device quality; anti-cheat and mod consistency. Complex items may consume more specialized processing/correction operations. **Do not use raw compression entropy as a mandatory 'bits-per-block cost'**: random noise/uncompressible images would become arbitrary economic exploits and penalize decorative builds.

## 9. Shared VM, typed world operations, magic and player tool code

### IR / sandbox

Two UI notations—engineer ladder/text/blocks and Chorister glyph/resonant geometry—compile to a shared typed small-step intermediate representation where semantically valid. Founders' artifacts use extensions gated by content capabilities. The bytecode runtime does arithmetic, fixed vectors, branching, time/event wait, sensor reads and typed `request_action` calls. It does not directly reach OS APIs, render buffers, raw voxel pointers or inventory memory.

**VM authority contract:** a program runs with object identity, permissions, precharged resource budgets, instruction fuel, memory bound, per-tick event cap, allowed port references and deterministic event ordering; scheduling respects global server/per-world budget. Opcodes may request `HeatVolume`, `MoveMaterial`, `LaunchProjectile`, `SetPort`, `EmitField`, `ReportSensor` etc. The domain solver validates the operation and debits energy/material as appropriate. An aether program cannot create a physical projectile without authorizing the requisite mass/energy/fictional source.

**Noita-style wand engine:** fixed wand capacities/statistics; ordered/shuffle deck rules; triggers/timers; calls and wrapping; multi-action frames; intrinsic effects; material modifiers; explicit execution stack/state. Test nested triggers and chain reactions. Power is limited by fuel, payload cost and event budgets, not arbitrarily denying ingenious effects. Make spell graphs and tooltips inspectable: cast order, expected costs, cooldown, trigger interactions, physical world target.

**Programmable robot/crew scripts:** same IR where feasible but different permission set and domain APIs. Avoid building a second unrelated interpreted language for every NPC and machine. Utility AI selecting goals remains a separate optimized runtime.

**Mod interfaces:** RON/data schemas for material/machine/creature/spell defs, codegen/scaffolds, validators for IDs, ports, physical dimensions, rendering palettes and compatibility. Hard-coded efficient kernels can be shared by data-driven definitions; do not prohibit small `match` statements on closed hot-loop enums merely in pursuit of 'zero matches'. Sandboxed WASM optional for complex events, with capability and fuel restrictions. No WASM call per material cell. Mods must be versioned, migrated and attributed in performance diagnostics.

## 10. Dynamic world generation and physical secret placement

**Pure generation stages:** geological tectonic/climate model → hydrology/aquifers/fault/ore/arcane field → ecology and regions → historical civilization footprint and infrastructure graph → named landmarks and site connectivity → architecture `PlacePlan` → material/voxel stamps → deterministic physical validation → secret/hint distribution.

**Generator versioning:** keep generators 53 and 54 behavior for old saved worlds; add generator 55 only when canonical place generation changes. Generated output must be independent of chunk visit order and rendering detail requested. Use named deterministic RNG streams keyed by seed/feature coordinates; never use global RNG advancement dependent on thread scheduling.

**Functional POIs:** dams hold and channel water, mines have supports/rails, towns connect to resources, Chorister spaces contain acoustic/rune structure, Founder archives expose actual decipherable records. Distinct visual civilizations share world-system physics. Inter-place quests/secrets arise from explicit graph/hint placement validation, not probabilistic object scatter.

**PlacePlan:** site; landscape approach; main/secondary masses; room/circulation graph; protected 3D sightline/traversal volumes; support geometry; functional machine/puzzle/resource anchors; light and climate plan; dressing sockets; secret clues; LOD-critical silhouette records. Stamp from macro to micro and validate occupancy collision and all required routes with player/creature movement constraints.

**Secret rule generation:** each world-scoped mystery has seeded predicates, several hints and evidence locations, optional experiments, physical outputs, and a proven reachable route. Not all secret mechanisms are per-seed random chemistry; broad material laws remain understandable. Where seed varies transmutation recipes, build a solver test to establish that clues specify a bounded hypothesis space. Non-scripted emergent discoveries do not need pre-authored hint sets.

**Nested worlds:** pocket spaces/domains and electronic virtual worlds may share region and save interfaces, but different boundary physics, resource transfer, time scale and avatar presence demand explicit rules. A 'virtual space in an in-world computer' can use the same content/editor technology without magically granting arbitrary physical items to the parent world.

## 11. Dynamic AI, navigation, ecology and procedural raids

AI navigation uses hierarchical cell/region portals + local traversability; different nav modes for ground, flying, swimming, burrowing, wall climbing and vehicle interiors. Preserve portals through generated caves/arches and player excavations. Spatial edits invalidate only relevant summaries; no global navmesh rebuild per mined block. Distant populations use scheduled ecological aggregate state, not thousands of individual walking NPCs.

Senses derive from real sightlines, audible events/local acoustic portal summaries, heat/smell where justified, vibration/arcane sensors and faction knowledge. Utility AI agents have tasks and needs; use cheap evaluation scheduling based on actual decision points. Crews can diagnose subsystem damage and execute repair/evacuate orders if physical path and materials exist. Combat behaviors telegraph physics and functional weak points.

Enemy spawning must have visible/ecologically relevant origin (nest, migration route, portal, habitat), with localized disturbance budgets. Player can intercept, farm, reduce, cultivate or reroute threats. Persistent secure bases should not have teleporting random enemies. Difficulty comes from exciting regions and operations, not an ever-ticking global spawn meter.

## 12. CPU/GPU budget scheduler, backpressure and precision

**Work category** | **Priority and constraint**
:---|:---
Critical | input, authoritative local edits, player contact, attack outcome, critical nearby hazards and core network severance—must finish coherently or enter a safe constrained state
Interactive | visible nearby meshing, fluid/reactive fronts, nearby NPC decisions, high-value lights and audio—budgeted and responsive
Background | remote ecology/factory integration, generation, save snapshots, distant nav, streaming compression—service without starvation
Opportunistic | far aesthetic refinement, extra indirect-light bounce, visual particle density, advanced cache compaction—drop/reduce freely

A job has stable identity, input dependency revisions, cost estimate, deadline/importance class, memory and upload budget, cancelation state and permitted result destination. Coalesce obsolete geometry jobs. Use an owner-execution model for conflicting simulation writes, work-stealing for independent tasks, explicit staging buffers and bounded queues. A slowdown reduces cosmetic fidelity and far LOD quality before gameplay truth; conservative parent representation preserves terrain coverage.

**Dynamic adjustment:** a governor monitors CPU+GPU frame time, 95th/99th percentile, upload delay, VRAM occupancy, simulation backlog and persistence lag. It adjusts separate budgets subject to hard minimum safety requirements, hysteresis and no quality oscillation every frame. An overheating machine does not silently stop being authoritative under heavy GPU load.

**Precision and parallelism:** no cross-branch use of unstable floats as save identity; fixed-point/integer extensive quantities where appropriate; gravity/contact local physical units; log numerical residuals on iterative solvers. Carry interface ownership and material conservation through sleep/coarse/fine/wake. Separate render motion vectors from true world transforms for temporal stability.

## 13. Performance target matrix and benchmarking (proposed, unverified)

**Three target classes (definitions require actual hardware discovery):**

- **H0:** low-spec integrated GPU/CPU configuration; reduced draw distance/volumetrics and limited active material bricks; target playable 30 FPS at 720p–1080p under defined reduced settings.
- **H1:** mainstream current desktop discrete GPU; target 60 FPS at 1080p in typical scenes and stable factory demos with defined active regions.
- **H3:** development-class high-end RTX 4090 (24 GB) or equivalent; **stretch** 120 FPS 1440p ordinary exploration, and **stretch** 60 FPS in a *defined* heavy physical scene. These are aspiration targets, not measurements.

No performance number is valid without hardware, resolution, graphics settings, camera path, scene seed, world state, active voxel count, material interaction set, machine counts and duration. Separate initial load, streaming travel, stationary factory, cave combat, structural collapse, flood and soak. Add warmup, repeated runs and cold-cache variants.

**Instrumentation:** CPU tracing (`tracing`/Tracy etc.), WGPU timestamp queries/profiler where supported, allocator metrics, GPU buffer arena/fragmentation, quads/triangles/instances, vertex pulling occupancy, world-gen/mesh build time, uploads/readback, active brick counts, island solver cost, save bytes and queue wait. Store capture receipts with executable hash + shader/asset fingerprint. A 'faster' frame that renders a hole or drops fluid is a FAILED run.

**Acceptance workloads:**

1. Horizon fly: 24 km-style vista if actual representation supports it; fast flight over mountains/arches/edited buildings; no near/far cracks, proxy holes or stale edited terrain.
2. Mixed LOD edits: dig a 0.125 m passage at a 1 m/0.5 m seam, move away, unload/reload and return; correct rendered/collision geometry and conserved removed volume.
3. Flood/chemical: dam breach through caves into a machine room; correct mass ledger, bounded simulation, no missing chunks or explosion of rigid bodies.
4. Destruction: damage tower supports, detach coherent body, fall into liquid/electrical site, save/reload, no duplicated mass and reasonable p99.
5. Factory: large packed belt runs, real loads, multi-source power, fluid tanks, repeated split/merge; transfers conserve items and charged energy.
6. Ancient dungeon: mine beams, pit shaft, library nave, hidden doors and multiple routes remain traversable with actual player capsule and lighting.
7. Overnight soak: large edit/save/explore/return history, VRAM pool stable, no unbounded queue growth, no dirty-save loss.
8. Mods: add a new data-only material and machine/spell recipe, validate coverage and measured behavior without unrelated source changes.

## 14. Failure taxonomy and remediation requirements

| Failure | Root-cause diagnosis | Prohibited workaround |
|---|---|---|
| Horizon gaps | Residency ownership, proxy topology, neighbor transitions, revision errors | Depth-sink, radial discard, thicker fog |
| Disconnected tree crowns | Parent LOD generation or mesh/culling | Add random leaf particles instead of fix |
| Coarse roofs at 1 m | Missing true fine geometry/physical proxy | Photogenic mesh with incompatible collision |
| Yellow dungeon lighting | Light color/intensity, albedo, exposure, tone map, indirect | Tint stone blue solely to cancel shader bug |
| CA race / duplication | Cross-chunk ownership, scheduler, boundary transfers | Silently delete overflow material |
| Factory dupes | Inventory/network authority conflict, transaction commit | Reconcile inventories from display state |
| GPU stalls | Too many buffers, uploads, readback, allocation churn | Drop near chunks or stall gameplay inputs |
| Wands look generic | Silhouette grammar, tiny-size material/animation, art production | More bolts, glow, bevels or heavy head |
| Boring generated dungeon | Protected voids, role grammar, functional anchors, light, traversal | More random containers/enemies |

## 15. Research-to-implementation decisions to measure

**Before changing major engine architecture, run isolated spikes:** dense 8³ vs 16³ active bricks CPU/GPU; volume/surface reaction kernels; greedy quad vs direct volume rendering; CPU SIMD active-CA throughput; conservative mixed-grid transport; structure connectivity under removals; real collision API vs custom static proxies; one plausible resonance graph field; content-driven procedural secret solver; novel artifact rendering 2D/3D hybrid; long belt compression; job scheduler hysteresis. Each spike has fixed scenes, results, a keep/reject decision, source ownership and an ADR.

**References (verified conceptual anchors as of 2026-10-26):**
- Noita GDC physics+design: https://www.gdcvault.com/play/1025695/Exploring-the-Tech-and-Design/
- Transvoxel 2:1 transitions: https://transvoxel.org/
- wgpu 30 docs and experimental capability notes: https://docs.rs/wgpu/30.0.1/wgpu/
- WGPU GPU timestamps example: https://wgpu.rs/doc/wgpu_examples/timestamp_queries/index.html
- Rapier voxel collider: https://docs.rs/rapier3d/latest/rapier3d/geometry/struct.Voxels.html
- Factorio transport-line optimization: https://factorio.com/blog/post/fff-176
- Official Aseprite asset automation: https://aseprite.com/docs/cli/

**Research freshness warning:** Some search engines surface publications with future dates (e.g. a December 2026 dynamic NanoVDB paper). Since this spec is dated **October 26, 2026**, such documents are *not evidence of currently available technology* and must not be represented as implemented/published as of this date. Sparse active-grid processing is still a worthwhile experimental design idea; evaluate independently.


<!-- END 02_ENGINE_ARCHITECTURE.md -->


---

<!-- BEGIN 03_ART_CONTENT_AND_WORLDGEN.md -->

# 03 — ART DIRECTION, CONTENT PRODUCTION AND PHYSICAL WORLD GENERATION
**Rev U1 | October 26, 2026.** Visual quality is a production contract. Asset/model availability and claims of actual Blender execution require receipts.

## 1. The canonical visual statement

**"Chrono Trigger / A Link to the Past / polished GBA-era pixel art as a coherent Minecraft-constructible 3D world, with more responsive materials, high-quality lighting and Noita-caliber physical effects."** An imaginary premium 1995–2003 pixel RPG whose artists had access to modern WGPU illumination, dynamic destructible materials, huge caves and physically functioning factories. Keep human-scale readability and grid affordances. Never substitute generic photorealism, generic low-poly, PS1 jitter or a screen-wide pixelation postprocess.

### Visual principles

1. **Silhouette before material:** locations read as terrain masses → buildings → supports → human use → pixel highlights. Entities and wands must have distinct negative-space and shape identities when rendered solid black.
2. **Pixel clustering rather than noise:** textures use coherent shade clusters, selective texture features and restrained dithering where justified. A photo downsampled with nearest-neighbor is not premium pixel art.
3. **Material palette grammar:** intentional hue-shifted shadow/mid/highlight ramps, roughly 3–6 deliberate values per base material before dynamic lighting. Avoid random 16-bit noise and uniform brown/yellow wash. Colors may be extended for emission and weathering.
4. **Physically aligned details:** beams meet columns, roof ridges connect, chimneys have purpose, machine shafts connect to drives, pipes and electrical conductors have typed ports and coherent routes.
5. **Every scale gets attention:** first-person object, medium-distance building, distant silhouette and giant world composition are different views of one design. No hero-detail that vanishes into disconnected far silhouettes.
6. **State-driven effects:** shaders express wetness, heat, charge, pressure, rust, fungal growth, field strain, leakage and damage from actual simulation inputs. Small common objects can be visually quiet.
7. **Restrained contrast and bloom:** warmth in lantern pools, cool cave bounce, distinct industrial heat, magical emission reserved for interesting conditions. The player should see underground geometry without fullbright fog.
8. **Art supports play:** readable collision/head clearance; exact object grip and attack emitter; pickups visible; machine gauges legible; hidden doors discoverable from clues, not invisible pixels.

### Reference provenance

Named Downloads paintings from previous brief: `sylvain-sarrailh-danghostisland.jpg`, `tu-bui-dmz-overall.jpg`, `jung-park-boomtown.jpg`, `Витаутас Лайсонас.jpg`. **Only the last was inspected during previous reachable review**, showing towering pale walls, a tall deep vertical canyon, layered tiny warm roofs and a narrow watercourse; use *monumental mass vs delicate human-scale density* as composition principle. Future agent must open all originals locally before naming specific visual features. No art theft or AI texture lifted from reference illustrations. Anime/SNES references are influences, not sprite copying.

### Pixel scale research

Start controlled studies of ~16, 32 and 48 px/m for world materials, 48–64 px/m for hero equipment, and authored inventory sprite icons at game UI pixel scales. These are *art experiments*, not hardcoded requirements. Lock apparent size by world-space UVs/atlas policy and test under 1080p/1440p gameplay FOV, sloped roofs, distance LOD and mips. Texture filtering should preserve clusters nearby while preventing far shimmering: mip-aware nearest-in-level or appropriately configured blended mips with padded atlases, evaluated visually. Texture density may vary deliberately by material/hero intent, but never look like unrelated artpacks.

## 2. Materials and shaders: one coherent expression system

Build shared WGSL lighting/BRDF and a small number of material-family responses instead of one bespoke shader pipeline per block ID.

| Class | Authored visual clues | Dynamic state | Unique effect bound |
|---|---|---|---|
| Stone/limestone/basalt | Broad strata, designed chips, mortar seams, hue-ramp shadows | Wet, cracked, heated, soot, moss | Thermal fissures, trickle seams, crystalline growth, localized AO |
| Timber/leather/cloth | Directional grain, end-grain, woven clusters, believable stress | Moisture, scorch, char, fracture | Bark/shutter flex, subtle heat scorch; no constant noise |
| Copper/brass/steel | Large deliberate metallic facets, rivets only when mechanically necessary, tarnish | Temperature, voltage, corrosion, oxidation | Broad anisotropic glints, heating hue, conductor pulse near faults |
| Ceramic/glass | Restrained glazed bands, chipped edges, local translucency | Pressure, cracks, heat, contaminants | Gauge fluid meniscus, refraction at meaningful surfaces |
| Water/oil/acid/lava | Volume/depth color, patterned contact foam, directional movement | Level, flow, temperature, sediment | Surface flow, steam, bubbles, precipitation, wet material response |
| Crystals/aether | Internal structured facets, sparse luminous veins, embedded geometry | Flux, resonance, fracture, overcharge | Field distortions and spectral splitting on hero assets |
| Fungus/biology | Grown asymmetric segmentation, gills, soft spore sacs, varied scale | Growth, age, health, spores | Bioluminescent breathing, directed mycelial pulses, decay |
| Rune fields/shields | Geometry-anchored lines, stylized angular glyphs, inactive transparency | Charge, impact, frequency, damage | Local ripples, fracture, interference; no generic always-on hex bubble |
| Atmospheric | Layered fog, dampness, spectral ambient light | Weather, temperature, pressure, toxins | Physically informative fog and wind, restrained volumetric scattering |

**Material parameters** are compact, derived from authoritative world fields where available. Texture IDs, roughness masks, palette lookup, emission mask and dynamic state use shared arrays/bindings; variant count should not multiply GPU pipelines gratuitously. Fix lighting/color-space correctness before creating color-correcting material hacks (particularly the reported underground yellow problem). Ordinary blocks should be cheap to draw; only visible hero effects use higher cost passes.

**Shields inspired by, not copied from, Mycopunk:** public developer notes do not verify accessible proprietary energy-shield source. Implement original material-specific logic using near-invisible rest field, geometry-anchored patterns, fresnel at grazing angles, impact-ring propagation, emitter-directed pulses, localized damage and bounded event buffers. Five aesthetic families: kinetic polygon stress, electromagnetic trace lattice, arcane engraved knot/rune, fungal membrane, faceted crystalline interruption. `shield_state` carries charge, hit positions/ages/energies, topology and failure. All effects tested against world stone, water, glass, player camera and a rapidly changing light rig; surface culling/occlusion and `fwidth` line anti-aliasing are essential.

**Noita-like environmental feedback:** sand granular tumbling, burning wood, acid erosion, condensing steam, droplets, sparks and creature damage have distinctive pixel-cluster particle motifs; nearby particle density adapts without altering physical authority. Effects must make reactions legible, not conceal them. Hit FX should have clear onset/evolution/outcome. Avoid shader "magic" that implies nonexistent conductivity or water pressure.

## 3. Wand art is an ecosystem, not three heavy props

**Reject prior A/B/C prototypes as wands:** Spark-Lantern is a placeable aether lantern; Annular Induction is a ritual machine; Offset Coil is a magical industrial discharge tool. Their nearly identical dark teal/bronze/cyan lantern-like look is not Noita/parity art direction. Reuse shapes in the world only if function fits.

Noita documents a finite pool of **900 random wand graphics and 20 special wands**, with physical hints to stats. Axiom should establish 12 *hand-designed* visual families and ultimately a high-quality constrained generator for many valid combinations, without claiming equivalence from raw permutations.

### 12 authored origins and shapes

| Family | Visual silhouette | Casting/prop identity | Construction language |
|---|---|---|---|
| Witchwood switch | Crooked few twigs, unbalanced thin outline | Rapid crude casting / early wand | Wood, fiber, seed, soot |
| Bifurcated focus | Long forks/open negative space | Multicast, split streams | Hand-carved stave and narrow tips |
| Bone needle | Long needle or barb | Precise damage/curses | Bone, dark ferrule, tiny rune |
| Runic key | Irregular key-tooth/bit rather than rod | Wards, locks, structural manipulations | Flat etched metal or stone |
| Rootbound scepter | Root weave and living asymmetry | Growth, creatures, healing | Organic/chitin/symbionts |
| Cinder rod | Fine dark char with ember cracks | Heat and burning | Blackened grain, glassy fissures |
| Alchemical quill | Long taper with asymmetric nib | Drawing sigils and transforming matter | Thin ceramic and ink channels |
| Forked conductor | Precisely manufactured narrow double prong | Electrical/kinetic | Modest ceramic, copper wire, metal tip |
| Shard cluster | Angular uneven thin splinters | Chaotic elemental behavior | Multiple minerals, restrained bindings |
| Grave reliquary | Hanging charms/voids on slender base | Curse/summon | Iron, bone, translucent ancient wax |
| Astral filament | Sparse outline with suspended tiny focus | Spatial/gravity | Dark frame, seemingly weightless geometry |
| Ancient prism | Strong monolithic tiny shape | Ancient/secret intrinsic property | Founder blackglass/rule-bearing matter |

**Visual lineage constraints:** vernacular witchcraft; scholarly thaumaturgy; industrial magitech; biological fungal; ancient/anomalous. Each has different silhouette and craft grammar. Avoid simply changing material IDs. Smaller normal wands can use 2D sprite/2.5D or intentionally minimal 3D meshes, while hero objects use actual Blender models. Inventory pixel icons are authored/retouched, not automatically accepted from beauty renders.

**Mechanics affect appearance in subtle ways:** deck capacity may affect length; focus/fork may hint multicast; conductive prongs hint electric interactions; overcharged base emits temporary glints; inactive objects should usually be quiet. Spell-installed particle cues and activation signatures distinguish otherwise similar items. Never make every wand a big cyan lantern.

**First-person standard:** model occupies restrained screen area at actual FOV; grip placed on visible hand; named tip transform aligns visible effects with correct camera-targeting projectile semantics. Animation states: idle/walk/sprint, anticipation, release, recovery, sustained channel, switching, instability/damage. Do not add arbitrary camera recoil that ruins precision or accessibility. Compare silhouette at inventory ~48 px display, ground pickup, close inspection and first person. 

**Acceptance of first 6:** at least six visually **unmistakably** distinct wands built/exported/loaded in native WGPU; grayscale silhouettes; 3/4, first-person, neutral/day/torch scene; matching icon and in-world pickup; three distinct spell effects; real emission and damage state; no Blender-only claims. Then scale to 12 authored designs and constrained proc catalog.

## 4. Blender + image + pixel tool workflow (verifiable)

**Source art contract** for each approved interactive asset:

```
<asset_id>/
  source/<asset_id>.blend                # truly editable Blender model
  source/build_<asset_id>.py             # reproducible bpy if script-built
  source/pixel/<asset_id>.aseprite       # or editable Krita/LibreSprite layered source
  textures/albedo.png                   # controlled texel grid
  textures/material_masks.png           # rough/metal/wet etc as needed
  textures/emission.png                 # only if emitted
  export/<asset_id>.<actual_game_format> # RON mesh, glTF via verified converter, etc.
  manifest.ron                          # verified schema: dimensions/ports/states/LODs
  evidence/{neutral,firstperson,distance,active}.png
  report.md                              # tools actually run + native scene hash + defects
```

1. Read reference locally, design card + use cases, actual world dimensions, interaction ports, historical lineage, palette and collision.
2. Draw silhouette/contact sheet (3+ distinct concepts for hero assets); reject weak/overly symmetrical shapes before details.
3. Model in Blender 5 using named components, appropriate transforms and pivots; geometry has purpose. Script repeated parametric families via `bpy`; use visual UI adjustment for artistic quality.
4. Unwrap, align texels, generate controlled limited ramps; paint in Blender/Krita/Aseprite/LibreSprite. Aseprite CLI can batch palette-convert and export spritesheet + JSON; asset-import build must preserve mask channels and mip padding.
5. Bake/capture AO/normal where helpful, then **retouch** into suitable clustered hand-directed pixel art. Do not blindly copy Blender smooth highlights into albedo as high-res shading.
6. Author state→effect mapping. Export with actual importer or validated build-time converter, preserve physical origin/ports/LOD/material IDs.
7. Capture fixed-camera neutral Blender render and **native WGPU** screenshots/video at several distances and illuminations. Rework until shape, lighting and world interaction match.
8. Test collisions, destruction/salvage, edits, save/load, LOD and performance. Only after actual proof can agent say "integrated".

**Computer Use note:** This specification cannot itself operate the user's Windows Blender 5 session. In a computer-use-capable environment, use Blender UI and `bpy` and produce receipts. No environment/tool access → generate reproducible scripts and clearly mark modeling/game screenshots NOT RUN. Keep previous RON mesh/vertex/material pipeline working until a real importer migration is verified.

## 5. Placeable block and entity taxonomy (content as functional affordance)

Use a **family grammar** rather than thousands of disconnected one-off meshes. Examples of primary visual/functional families:

### Architecture (minimum starting kit)

Stone/ashlar, cobble, brick, timber frame, beams/struts, stair/cap/half/quarter, arches, vaults, buttresses, steep gables, dormer, hip/steeple/cap, ridge/eave/valley, shutter/window/mullion, balcony/rail, bridge/gate/portcullis, service grates, pipelines and gutters. Aesthetic and collision fidelity at locally refined scale is necessary especially for roofs. Each piece must work at supported placement rotations and snapped resolutions.

### Settlements and interiors

Bed, cabinet, chest, shelves and books, crate/sack/cask, butcher/kitchen, basin/well, market stall, inn furniture, archive index, cart, street lamp, workbench, hand pump, waterwheel, forge, millstone, chimney, noticeboard, measurement board, movable hoist and lift. Town planning should create parcels and streets, not scatter identical house shells.

### Industry and science

Ore crusher, mill, sieve, sorter, washing/beneficiation machine, charcoal/coke oven, foundry, alloy furnace, heat exchanger, boiler, pump, turbine, generator, motor, battery/capacitor, converter, transformer, rectifier, cable/pipe/shaft networks, tank/valve/filter/meter, inserter/conveyor/splitter, vacuum chamber, distiller, electrolyzer, crystal growth, wafer prep/lithography, microcontroller board and packaging. At least one real input/output and operational state per functional family.

### Magical industry

Mana condenser, resonance resonator, charged crystal, runic projector, glyph writer, wand workbench, shield node, spell logic port, ritual plates, material transmuter, levitation anchor, containment cell, star telescope, observatory, gate lock, spell turret, growth device and arcane reactor. Consistent historical lineages and physical interfaces.

### Ecology and cultivation

Trees with stable canopy LOD, mushrooms with growth stage, fungal root/drip conduits, spore organ, reed/pond life, lichen/moss drapes, vines, magical flowers, cocoon/nest, living sensors, guardian fruiting bodies, biological reactor trough, luminous fungi. Plants are renewable physical resources and habitat interactions, not decorative decals alone.

### Interactive secrets, dungeon devices and creatures

Pressure plates, tuneable resonant chamber, repairable generator, mechanical locks, hidden actuator, movable statue, counterweight, pressure valve, rotatable mirror, siphon gate, trap launcher, rune reader, coded archive, mining hoist, defensive golem, fungal scout, ore-borer, mechanical spider, crystal parasite, dragonfly-like flyer, sentry, repair drone. Every family gets use, failure and repair/scavenging behavior where relevant.

**Priority:** build 20–40 **working visual families** that establish a coherent world, then extend to 150+ useful placements with variants. Do not call 200 differently painted cubes meaningful content parity. All item/part variants have semantic assets/collision and LOD plan.

## 6. Generator 54 → 55: physical and artistic repairs

Previous reported state (NOT independently revalidated here): generator-54 R7 improved six families (castle, town, tower, mine, pit, dungeon library) with R5/R7 screenshots; the later R7 changed mine beams and pit dimensions; build/shader validation passed but traversal/save/perf did not. Remaining problems: coarse 1-meter stepped roofs, scattered town parcels, broken far tree canopy LOD, narrow ring pit, mine fungal clutter and intersecting gallery, uniformly yellow underground stone. Old generators 53 and 54 and saved worlds must be preserved.

### Required semantic `PlacePlan` recipe

```
site context and approach
→ terrain/landscape and primary composition
→ protected major spaces + room/circulation graph
→ load-bearing anchors/foundations
→ walls, roof planes, stair/lift/ventilation/utility geometry
→ physically meaningful materials and source lighting
→ production/quest/secret machine sockets
→ dressing within clearance exclusions
→ navigation/visibility/structural/LOD validator
→ deterministic multiscale voxel stamps + generation metadata
```

**Six families:**

- Castle: defensible gate and approach, courtyard/keep, differentiated rooflines, wall walks and usable stairs, built into terrain.
- Town: main/secondary streets, shared walls, varied heights/roof orientation, distinct workshops and civic services, plaza, credible density and traversal.
- Tower: a memorable silhouette, inhabitable vertical progression, functioning interior mechanisms, unusual architectural lineage.
- Mine: separate large working galleries, supports anchored to real stone, physically plausible hoist/ore routing, clear working headroom; coherent fungi placement, no low beam crossing.
- Pit: wide, dramatic open shaft/chamber and explicit traversal terraces, not just a narrow ring corridor; legible depth and wall lights outside clearances.
- Library: real nave/vault, playable book aisles, galleries/reading pools and important landmark, cooler stone separation with localized warm lighting.

**Version tests:** frozen original R7 capture hash reported in previous doc (verify local file), same seeds/cameras where available; generator 53/54 golden outputs unaffected; new 55 examples under castle/town/tower/mine/pit seed `2738958700`, library seed `424242`, plus at least three other seeds per family and supported voxel levels. Screenshots at neutral/dawn/night and near/medium/far; traverse with *actual player capsule*; save/reload edits; performance budgets. Do not simulate unsupported finest-grid worldgen by adding undestroyable mesh ornaments.

### Performance-sensitive art acceptance

Full-fidelity assets must use LOD, shared material atlases, GPU batching and stable lookup data. Physically animated machine parts can have low-cost skeleton/transform animation; large amounts of live industry need aggregate item rendering. Variation should come from restrained pattern/mask/geometry choice, not costly separate shader pipelines or unique megabyte-sized textures.

## 7. Native scene acceptance: one beautiful playable workshop

Build a **17-asset integrated riverside workshop**: limestone+arched door, dark timber, steep roof, shutter, stair; workbench/shelf/lantern; waterwheel/pump/ore crusher/generator; aether condenser/rune switch/shield node; a proper tree and fungal cluster. At least three real chained interactions: flowing water → wheel/generator (appropriate approximation) → condenser → shield or machine; plus two repair/failure states and one clue to a deeper ruin. If a mechanical system isn't yet available, mark it **visual-only** and do not claim it works.

Pass only when: Blender editable source + game importer; real WGPU captures; 4 lighting contexts; consistent pixel scale; recognizability from 15–25 m and silhouette from 75 m; corrected roof and tree LOD; no yellow underground wash; actual ports/physical interactions; modest GPU cost; saved evidence hashes and reproduction steps. An image generator or Blender-only beauty scene is **not enough**.

## 8. AI/model prompts: build the process, not just a picture

**Master model creation prompt** (prepend to per-asset tasks):

> You are the art director, Blender 5 technical modeler, SNES/GBA-inspired pixel artist and Rust/WGPU integrator for Underkingdom. Read the source art bible, actual renderer materials, supported asset import format and reference images you can truly open. Compose a distinct role/silhouette first. The artwork must be coherent with Minecraft construction grids, readable natural/industrial/arcane lineage and real physical world interaction. Use actual Blender geometry with named pivots and collision, UVs on an intentional texel lattice, hand-directed clustered materials and useful active/failure states. Never fall back on a teal/copper lantern with random bolts, oversized magical heads, sprayed texture noise, excessive bloom or a generic bevelled cube. Produce real editable source + script + atlas + importer manifest + actual native 3D gameplay captures (or mark game integration not run). Compare normal FOV, inventory, world dropped, neutral light and underground light. Iterate when appearance or function fails; do not declare acceptance from a beautiful concept sheet.

**Hero asset prompt (one natural wand / one industrial focus / one ancient artifact):**

> First create a sheet of 18 solid-black shapes spanning at least six material/shape origins, showing true variety. Choose three that remain readable at 48 px and in first-person held pose, with clearly different dominant length-to-width ratios and negative space. Model actual artifacts in Blender: inexpensive irregular living wood, precise slender conductor and rare asymmetrical ancient object. Put meaningful material and casting channels at the tip, not a big glowing battery case. Author inventory icons separately; animate anticipation, release, recovery, and spell-dependent response. Export via existing pipeline and prove in WGPU. Any two assets that look identical when rendered black at inventory scale must be reworked.

**Factory family prompt (early and late industrial):**

> Design 3 related visually legible machines: water-powered crusher, fired-clay refractory furnace, early electrical generator. Their drive, ports, output, stateful gauges, material wear, simple repair behavior and input/output need to be represented physically in game. Match shared machine-body kit without identical box silhouettes. Model in Blender, UV to common atlas, rig useful moving parts, export and prove one functional loop in the existing game. Show the relationship between mechanical input, heat/power and production rate; no progress bar that advances without supplied resources.

**World-site prompt (mine + library + civil factory):**

> Generate a staged, physically navigable PlacePlan. Compose landmark silhouette, protected 3D voids and human circulation before walls and props; root supports in actual terrain; match voxel scales; use temperature-separated lighting; include functional machine anchor and two route alternatives with in-world clues. Fix observed R7 mine low beams/pit ring corridors/yellow wash. Preserve older generator outputs. Capture near/far and player-capsule traversal; reject generic empty or cluttered rooms.

**Shader/material prompt (WGSL):**

> Build one compact shared lighting pipeline with deterministic procedural material masks and state-driven stone, wood, copper/iron, ceramic, water, crystal, fungus and arcane effects. Reference stylized Noita/Chrono Trigger shape grammar rather than copying any game shader. Implement original shield families with emitter-bound ripples and charge/damage; measure GPU cost. Provide neutral benchmark assets, near/far textures, day/night/cave cameras and shader-validation + p95/p99 metrics. Avoid postprocess pixelization, texture shimmer and physically misleading visual states.

## 9. Source tools and trusted implementation references

- Blender 5 Python API: https://docs.blender.org/api/5.0/ and model/UV/texture baking tools corresponding to installed Blender version.
- Aseprite CLI for palette and atlas generation: https://aseprite.com/docs/cli/
- Noita wand visual and stat references: https://noita.wiki.gg/wiki/Wand and https://noita.wiki.gg/wiki/Wand_Appearance
- Mycopunk developer material/shader announcements should be read as technical inspiration only; no proprietary energy shield source is verified available.
- Transvoxel for smooth 2:1 transition where chosen: https://transvoxel.org/
- GDC Noita technical/design talk: https://www.gdcvault.com/play/1025695/Exploring-the-Tech-and-Design/


<!-- END 03_ART_CONTENT_AND_WORLDGEN.md -->


---

<!-- BEGIN 04_IMPLEMENTATION_AND_VERIFICATION.md -->

# 04 — EXECUTION PLAN, CLAIMS, ACCEPTANCE AND AI-AGENT PROTOCOL
**Revision:** U1 — 2026-10-26 | **Status:** planned and verified as document structure; no active repository build has been run here.

## 1. Operating principle: an architecture is a hypothesis until measured

The source document's Part B is excellent in spirit: a rich bible cannot replace experiments, narrow system designs and evidence. Preserve its plan-generation protocol but prohibit analysis paralysis: **a minimal first playable vertical slice is the highest-value integration test.** Use architecture documents to constrain code that exists, not to schedule a year of paperwork before a single player can mine a block, cast a spell or make a machine work.

All claims are traced to a named scenario, owner, target milestone and evidence class: `CODE / TEST / SCREEN / PLAYED / MEASURED / UNSUPPORTED`. A written assertion is not evidence. Every completion report states the exact branch/commit, command lines, build hash, seed, hardware, resolution, content version and change ownership. Migrations/backups and concurrent worktree constraints are first-class. Do not fabricate test outcomes or cite inaccessible Windows files as inspected.

## 2. Product goals and metrics

### P0 correctness gates (must never regress)

- Old saved worlds and generator versions load correctly; no edit, item, power, recipe, boss reward or machine inventory silently disappears or doubles.
- Near/far geometry covers all visible world surfaces in valid residency states; no chunk holes or unacceptable seams/floating roofs or disconnected tree crowns during streaming.
- The player's physical edit has a prompt and correct visible response, collision and persistent material amount across scale levels.
- Actual factory utility and spell resource costs are accounted authoritatively; no invented free Joules, watts, goods or mana.
- All scene claims have reproducible evidence and known limitations; no agent calls a shader compilation a successful visual overhaul.
- Stable code ownership prevents concurrent edits from silently overwriting unrelated work.

### Proposed hardware matrix (must be measured)

| Class | Prototype description | Normal-play target | High-load target | Explicit caveat |
|---|---|---|---|---|
| H0 | Low-spec integrated GPU | 30 FPS at reduced 720p–1080p | Defined reduced simulation budget; prefer safe responsiveness to full effect fidelity | Real hardware profile needed |
| H1 | Mid-tier discrete GPU | 60 FPS 1080p | p99 controlled at reduced fidelity | GPU/CPU specs TBD |
| H3 | High-end 4090-class 24 GB | 120 FPS at 1440p stretch | 60 FPS stretch in bounded large incident | Not a promise; settings/scenes must be frozen |

Render FPS and physics tick are separate. Primary authority may use 60 Hz while other islands/networks run appropriate rates. Hard-code **budgets** only after profiling. Scaled quality must not alter authoritatively credited inventories, material reactions or boss mechanics.

### Gameplay satisfaction and art gates

- First meaningful survival action within 5 min; basic automation discovery within around 2 h for uncoached tests.
- At least two genuinely different solutions to major authored physical gates; at least one player-proposed third route discovered during playtests.
- At least six excellent, distinct actual in-game wands before procedural catalog enlargement; spell-synergy builds discovered by players without author hints.
- Integrated scenic world: six place families tested for navigability, sightline, structured silhouettes, palette and player use; neutral/dungeon light both acceptable.
- A single full workshop→delve→salvage→factory upgrade loop tested end-to-end rather than only counted as implemented feature flags.
- The world exposes mysteries without requiring external documentation; humans can solve at least one seeded secret chain under an allotted, reasonable test session with clues.

## 3. Corrected claims matrix from the supplied C-* design bible

The original author IDs are preserved for traceability. Each change below is an explicit refinement, not an accidental replacement.

| Claim | Design target and revision | Verification scenario | Gate |
|---|---|---|---|
| **C-MC1** | Energy converters connect compatible domains and debit/credit known inputs, losses and declared magical sources. **Revision:** not every energy pair convertible; no free-loop assumptions. | `energy_converter_loop`, `missing_source`, `typed_port_rejection`, input-output ledger | M5 |
| **C-MC2** | Machine classes respond to hazards physically relevant to their materials and internals; visible damage/repair. **Revision:** not every acid/fire/radiation hazard necessarily damages every material equally. | `hazard_machine_matrix`, `repair_after_flood`, `heat_cable_fault` | M5–M7 |
| **C-MC3** | Adding new material data produces valid derived tools/caps/armor behavior where the applicable component rules exist. | `data_only_orichalcum`, `derived_mass_strength`, mod sandbox validator | M7 |
| **C-MC4** | One authoritative ledger/model per physical **concept**, multiple implementations/proxies permitted where mathematically appropriate. **Revision:** no blanket ban on alternate solvers or helper implementations. | `unique_ownership_lint`, `port_model_matrix`, code architecture review | M1–M7 |
| **C-MC5** | Uncoached newcomer reaches first genuinely useful automation with in-game Primer hints in around two hours. | `human_first_automation`, controlled tutorial/cohort notes; bot supplementary | M4 |
| **C-N1** | >=3 seeded hidden recipe families with explicit in-world clue chains and bounded hypothesis space. | `seeded_recipe_discovery`, 20-seed validity + human hint study | M8 |
| **C-N2** | Every material contact has a resolver result (possibly inert). **Revision:** reject forced 30% nontrivial pairwise chemistry; curate reactions by properties and game value. | `reaction_resolver_totality`, `signature_chain_coverage`, `combustion_acid_water` | M3, M8 |
| **C-N3** | >=6 optional world-scale secrets with place/time/condition, seeded feasible clue paths and lasting consequences. | `secret_graph_validate_50_seeds`, human cold-run solve, persistence | M9–M11 |
| **C-N4** | Strongly compositional spell grammar and high measured useful diversity. **Revision:** raw million combinations not acceptable proof of distinct player-facing behavior. | `spell_semantic_fuzz`, `trigger_order`, `recursion_budget`, expert playtest | M4–M9 |
| **C-N5** | Deep 3D reactive materials and destruction at stable frame/tick targets. **Revision:** compare *causal behavior*, not pixel-for-pixel with 2D Noita. | `dam_break`, `acid_floor`, `oil_fire`, `collapse_into_water`, capture/perf | M3–M7 |
| **C-F1** | Physical editable vessels with independent subsystems and task-based NPC/automation repair. | `crawler_hull_breach`, `battery_reroute`, `crew_repair_access` | M8–M10 |
| **C-F2** | Expeditions with travel/risk choices, seeded physical sites and persistent outcomes; explicit travel semantics. | `expedition_route`, `retreat_partial_salvage`, `persistent_event` | M9 |
| **C-F3** | Optional high-risk delve/permadeath preserving history in world/codex. | `delve_body_legacy`, `archive_survives`, `base_unaffected` | M8–M10 |
| **C-Z1** | >=3 coherent layered civilizations with language, material culture, technology, purpose and downfall in generation. | `culture_10_seeds`, manual art/story review, machine/city consistency | M6–M9 |
| **C-Z2** | Distinct named landmarks with roles and curated perceived density. **Revision:** 1/500 m underground universally would create clutter and harm wilderness scale. | `named_place_density_histogram`, exploration recognition playtest | M6 |
| **C-Z3** | >=2 independent physically valid solutions for every authored hard gate; stretch >=3 for key sites. **Revision:** do not require all random obstacles solved exactly three ways. | `dam_3_routes`, `resonance_gate_2_routes`, `builder_bypass` | M6–M9 |
| **C-Z4** | Optional room-description layer grounded in actual surfaces, history and sensory summaries; authored-quality templates. | `description_grounding`, `false_object_absence`, readability review | M8 |
| **C-S1** | Primer grounded in actual world demonstration, not reading a book page or guessed progress. | `primer_siphon`, `primer_circuit`, `observed_mastery`, onboarding cohort | M4 |
| **C-S2** | Feed/matter compilation costs matter, energy, manufacturing operations and complexity; editable blueprints. **Revision:** raw compression-bits-per-block not universal cost metric. | `compiler_matter_ledger`, `blueprint_complexity`, `noise_exploit` | M11 |
| **C-S3** | Glyph and machine controls compile into common bounded typed VM IR; behavior manipulation only via modeled utility API. | `cross_compile_same_output`, `vm_fuel`, `auth_no_raw_world_write` | M4–M8 |
| **C-S4** | Archives, deep time, faction knowledge and optional age/NG+ transition. **Revision:** avoid unwarned destructive permanent loss. | `archive_saved`, `optional_age_transition`, `reburied_ruins` | M9–M11 |
| **C-S5** | Orbital engineering with comprehensible transfer/stress/thermal models. **Revision:** don't mandate integer patched conics everywhere; use validated stable numeric formulation. | `orbit_transfer`, `tether_load`, `habitat_thermal`, solver-error budget | M11–M12 |
| **C-S6** | Toy-honest cryptography puzzles alongside real library-backed secure network features. | `cipher_primer`, `reproducible_encrypted_clue`, `secure_channel_boundary` | M8 |
| **C-S7** | Pocket/dimensional/virtual worlds have explicit time/physics/transfer boundaries and persistence. **Revision:** 2 levels nested is stretch after safe first level, not baseline. | `domain_enter_exit`, `inventory_transfer`, `nested_clock` | M10–M12 |

**All C-* claims remain in the formal master plan** as preserved or explicitly modified. The engine must not ship shallow proxies to force early PASS on late claims.

## 4. Phase dependency graph: vertical slices first

```
M0 Repo archaeology + frozen benchmark + ownership map
│
├─ M1 Canonical world transaction + save/LOD revision contracts
│   ├─ M2 Stable render/streaming/horizon/voxel precision + pixel art baseline
│   │   └─ M3 Sparse CA/material interactions + structural/thermal test sandbox
│   └─ M4 Gameplay foundation: survival, first spells, Primer, one small powered line
│         ├─ M5 Factory graphs: power + belts + fluid + two actual process machines
│         └─ M6 Generator55 place plans: settlement, mine, library, first dungeon + physical alternatives
│                 └─ M7 Integrated First Valley workshop/ruin, six wands, basic creature anatomy
│                       ├─ M8 VM/runes/AI/ecology/vessel proof + richer magitech
│                       ├─ M9 Deep civs, multi-site secrets, persistent expedition
│                       └─ M10 Advanced manufacturing, controller/IC design, big plants
│                              └─ M11 Founder Work, Feed, ages/domains, ring prototype
│                                    └─ M12 Orbital/legendary secrets/megabase quality and polish
```

This graph has parallel opportunities: art R&D and procedural semantic layouts can progress during M1–M5 in independent worktrees with stable interfaces; entertainment quality gets continuous attention, not delayed to M12. Claim targets belong to several systems, while *test implementation* has one primary owner.

### M0 — Repo archaeology, runnable baselines, safety (first working session)

- Inspect `CLAUDE.md/AGENTS.md`, Cargo workspace and locked versions, branch/dirty files, generator 54 R7 reports, actual render and world store, current saves, active concurrent ownership.
- Generate source-based map of storage/rendering/physics/manipulation/UI and current tested vs merely reported features. Verify path and file existence before claiming code knowledge.
- Freeze reproducible native scenes and hashes: first-person tool, horizon, roof/town/library, functioning factory, starter ecology. Establish CPU/GPU profiler and minimal capture receipts.
- **Exit:** build/test/capture baseline commands plus failures documented, visual evidence (or explicitly not run), no code rewrite.

### M1 — World kernel contracts + preservation

- Implement / formalize region identity, world units and scale levels, exact persistent edits, transaction ownership, artifact dependency revisions and safe old-generator fallback.
- Add `same_mass_across_scale`, `stale_collider_discard`, `save_upgrade_legacy`, `neighbor_revision` tests.
- **Exit:** a tiny edit seam / save/load demonstration, proven before cosmetics.

### M2 — Render/streaming correctness + first art benchmark

- Fix reported horizon seams/holes, fallback proxy continuity, near/far trees, chunky roof primitive and memory/upload backpressure; verify actual renderer scene, not shader compilation only.
- Make neutral-light stone/wood/metal/crystal reference and first genuinely pixel-authored workshop items.
- **Exit:** no visible holes across a fixed traversal route; stable captures at 1 m and refined geometry; p99/VRAM measured; old saves unaffected.

### M3 — Sparse material island proof

- One 3D sand/granular area, water basin, fire/heat and small acid reaction, only active bricks simulated; conservative coarse-fine contact and bulk ejection. Compare CPU vs GPU only where appropriate.
- **Exit:** dam breach/material conservation, reproducible 60 Hz/variable time benchmark under fixed H0/H1/H3 scenarios, no cross-region race/material dupes.

### M4 — One enjoyable survival-to-automation-to-magic loop

- First home, resource gathering, simple machine with real input/output and visual state, basic crafting, several spell compositions, Primer demonstration and small functional archaeological clue.
- **Exit:** uncoached tester moves from start to automated useful output and solves a physical secret without external docs. Runtime screenshots and UI review.

### M5 — Factory logistics and utility foundations

- DC/source/return, safe converter ledger, basic motors/heat, compact belt runs, inventories and tank/pump, one validated real multiblock. Don't immediately implement full AC/HV or all machines.
- **Exit:** ore→crusher→furnace→storage at measurable rates, respond correctly to a wire cut, a blocked belt and insufficient coolant, save/reload and benchmark.

### M6 — Art-directed procedural site framework

- Adopt semantic PlacePlan of Generator55, six place families, protected navigable voids and meaningful lighting; procedural culture tags; one serious multi-solution dungeon; preserve generator 53/54 exact output.
- **Exit:** headroom, route graph and interaction checks plus comparison images under native renderer for frozen seeds and different supported resolutions. One castle/town/mine/library should feel intentionally designed.

### M7 — The First Valley (first game-complete milestone)

- Beautiful small region, compelling caves/ruin, 17-asset workshop scene, functioning factory ↔ one magical machine interaction, 6 distinct wands, 3 movement/combat enemy archetypes with functional parts, short story/dungeon encounter and optional alternative route.
- **Exit:** a 90–180 min playtest with no out-of-band debugging required; beautiful native evidence; survival/world edits persist; frame times acceptable on defined tier.

### M8 — Unified Work programming and systemic encounters

- Explicit typed VM IR, ladder/glyph frontends, world effect capability checks, machine controller, first biologically grown creature, basic vessel/crawler/hull emergency, regional threat and useful enemy salvage.
- **Exit:** same sensor-control-spell behavior expressed via both machine and glyph frontends under equivalent conditions; deterministic safe energy debit; meaningful encounter alternatives.

### M9 — The deeper world, cultures and secret chains

- Engineers/Choristers/Founders coherent sites, maps, seed-dependent clue graph, archive factions, legend and physical secret, expedition and persistent ruin restoration. Avoid lore dumping and checklist puzzles.
- **Exit:** uncoached users can pursue several different mysteries; seeded graph passes validity checks and has authored-quality art/locations.

### M10 — Deep factory and electronics

- AC power, major logistics, working chip editor + real characterized controller, chemical fab stages, robot/crew loops, capturable industrial ruins. Nested editor is separate from overworld nanovoxels.
- **Exit:** two player-made chip variants behave differently in a factory, genuine throughput/energy budget, reusable design and persisted lot revision.

### M11 — Founder Work / matter compiler / world ages / ring research

- Founder archives, Feed proof, optional Age change with backup, first dimensional domain, orbital transfer spike and physically constrained tether. No automatic game-finish screen after first ring.
- **Exit:** verified separate orbital domain physics interface, compiler ledger and human story quality testing. Rule-bearing material semantics documented and safe.

### M12 — Content depth and long-term polish

- Extended civilizations/biomes, exceptional 3D dungeons, dozens of advanced spells and material interactions, boss capture/farming, original VFX, sound, deep optional endings, improved low-end GPU path and modding. Long-duration soak tests plus repeated blind human sessions.
- **Exit:** performance + correctness + significant player retention/playtest evidence; honest comparisons against chosen reference games in equivalent scenarios.

## 5. Required technical spikes (measured *before* binding ADRs)

| Spike | Competing candidates | Fixture/output | Decision |
|---|---|---|---|
| S01 Hot brick sim | 8³ vs 16³, CPU SIMD vs WGPU compute, halo vs global | `sand_flood`, `heat_front`, active cells/s, bytes, sync | Brick/kernel choice |
| S02 Fluid model | Local conservative cellular, pressure-column, small finite-volume | U-tube and dam-break equality/mass/feel | Fluid base model |
| S03 Geometry | Bitwise greedy raster, compact vertex-pull, smooth voxel mesh, direct raymarch for select | Cave/castle/edited overhang at 1440p; edit latency/VRAM/p99 | Renderer path by class |
| S04 LOD topology | Parent-cover vs alternate transition strategy; Transvoxel if smooth | Edit small hole at far/near boundary, camera flight, animated morph | Safe residency/stitch rules |
| S05 Structural splits | Hierarchical region graph vs local flood after deletion | Cut bridge/tower then fall/settle, memory/CPU | Detach algorithm |
| S06 Machine network | Dense nodes vs compiled cables/belts, isolated islands | Long linear network and branched system; idle ticks | Graph representation |
| S07 VM | Small typed stack vs register interpreter; compiled glyph/text frontend | branch/sensor/shape operations/s; worst recursion | IR and fuel |
| S08 Resonance | Cached modal graph vs PDE-lite vs purely authored field | Chorister chamber chord, editable geometry, cost & observability | Grounded magic field |
| S09 Primer | Rule-template + event certificates vs scripted achievements | First 20 min, experimental tasks, hint behavior | Instruction/UX |
| S10 Artifact renders | Flat sprite/2.5D/mesh; varying pixel density | 12 silhouettes in inventory/FOV, near/far shader light | Asset representation |
| S11 Cities | PlacePlan+protected void vs direct stamps | Generator 54 comparisons, 10–30 seeds, traversability | Worldgen structure |
| S12 Planet/ring | Planar/patched sphere/domain transitions, orbital double vs fixed | Huge coordinate stability, transfer conditions, resource cost | Long-term topology |
| S13 Secret generation | Hand-linked graphs vs procedural logic constraint solver | 20–50 seeds, reachability, human clue path | Mystery generator |
| S14 Matter compiler | Primitive-op complexity vs compressor size alone | Simple/noisy/complex comparable builds, economic exploits | Cost metric |

**Spike discipline:** one focused session where practical; include exact compiled code, hardware and results, no selection by trendiness. A failed idea remains a documented rejected alternative, not a hidden half-implemented dependency. Long-term ADR only after data.

## 6. SDD structure: per-system executable design

Each material subsystem or deep machine domain must have a **2–6 page concise SDD** (more only when warranted) with:

1. Purpose, player-visible need and C-* traceability.
2. Scope (in/out), dependency/owner map and stable interface.
3. Data layout, field sizes, alignment, CPU/GPU/storage residency, invariants, lifetime and migration.
4. Operations and pseudocode, time/memory/bandwidth complexity and numerical/determinism contracts.
5. Authoritative commands, typed input/output ports, events and permitted VM/UI queries.
6. Tick scheduling, island/sleep/wake rules, adaptive fidelity limits and safe failure states.
7. Budget targets on hardware tiers, low-end feature fallback and monitoring counters.
8. Data/RON schemas with simple working example, source content rules and validation.
9. Unit, property, conservation, replay, fuzz, visual and actual-playtest cases.
10. Named performance fixture with before/after required.
11. Compatibility with existing saves, generator versions, and explicit deleted/replaced code.
12. Risks, spike results and at least two meaningful alternatives rejected.
13. Small ordered implementation sessions and checkable exit conditions.
14. Evidence ledger: reported/code inspected/build/test/native visible/played/profiled.

**Independent critic** reviews ownership, exactness, perf, data-driven complexity, migration, visual integration and target player behavior. Reviewer should not approve vague "it feels alive" without mechanism: *active nest state emits an alarm when vibration threshold is met and nearby defenders respond* is a mechanism. Three failed review rounds → short human escalation with disagreement, not endless rewrite loops. Do **not** treat all 12 rubric dimensions as literally equal priority if a simple SDD legitimately has no GPU data; justified N/A is acceptable.

## 7. CI, property tests and visual reviews

### Automation commands (names proposed; adapt to repo)

```
just check           # fmt, clippy, cargo tests, WGSL validation
just verify          # focused integration, saved fixture round-trips
just bench           # deterministic CPU/GPU scene benchmark with receipts
just scenes          # headless scripted play scenes and invariant assertions
just captures        # native game capture receipts at frozen cameras
just claims          # C-* scorecard with PASS/FAIL/NOT_RUN + evidence paths
just assets-validate # RON/material/texture/LOD/port validators
just docs-check      # SDD claim ID, ADR and reference-source completeness
```

Never change tolerance or remove a failing test simply to obtain a green score. Build pipeline must validate shader compilation and render results separately. Every new generator version is tested against old-version golden generation. Scene timeouts and physics heat/friction tolerance differences must be explained.

### Scenario specification format (proposal; adapt to real harness)

```
id: "m1_mixed_lod_edit_roundtrip"
seed: 2738958700
world_generator_version: 55
voxel_scales_m: [1.0, 0.5, 0.125]
steps:
  - spawn_player_at_named_testsite: "refined_wall"
  - excavate_volume_m3: { volume: 0.125, region: "fine_seam" }
  - stream_away_and_return: { distance_m: 2048 }
  - save_and_reload: true
assert:
  - saved_exact_world_delta: true
  - occupied_volume_change_matches_action: true
  - collision_ray_matches_visual_opening: true
  - no_horizon_gap_on_camera_path: true
metrics: [cpu_p95_ms, gpu_p95_ms, frame_p99_ms, gpu_resident_mb,
          jobs_queued_max, compressed_region_bytes]
```

**This is a schema idea, not current runnable test YAML.** Machine-generated evidence includes build SHA, asset SHA, frozen camera pose, GUI/capture state, device info, command and logs. Recorder replays apply authoritative inputs/events with versioned RNG streams. CPU/GPU comparison is required only when solving the *same declared model*; higher-fidelity reference numerics can serve tolerant truth tests otherwise.

### Gameplay acceptance methods

- **Think-aloud uncoached**: early automations, first magic and interpreting hazards.
- **Expert sandbox:** discover wand synergies, abuse physics, invent boss traps, reverse-engineer machines.
- **Dungeon route audit:** at least two solutions physically traversed per key barrier; no false 'three ways' claimed from static design text.
- **Blind visual:** compare in-game screenshots and camera path with style reference; record real objective flaws: silhouette, palette, texel scale, tree LOD, clutter, light.
- **Mod compatibility:** add new material/recipe via data, version content pack, validate and profile without rebuilding unrelated systems.
- **Soak:** hours of ordinary edits/exploration/saving; test RAM/VRAM leak and world/collider correctness.

## 8. Concurrency and repo ownership safety

**Required order before code:** read working tree status, branch and untracked files; review local instruction docs, build toolchain, Cargo.lock, test histories and asset pipeline. Record file ownership per worktree (terrain/worldgen/shader/horizon/item/UI). Use isolated branches/worktrees for research, meshing, worldgen and art. One designated integrator owns shared files and staging. Do not `git reset --hard`, `git clean -fd`, auto-stash or overwrite concurrent edits. Don't auto-push production changes, merge into main, alter saves or change user-facing branches without permission/context.

**Work composition:** research agents gather verified references; physics author implements CPU reference while GPU specialist prototypes optimized kernel against identical fixtures; procedural art/Blender specialist provides actual source assets and importer manifest; independent critic verifies runtime/memory/visual claim. These agents coordinate through interfaces, not simultaneously editing the same WGSL files.

**Status vocabulary:** `PROPOSED`, `INSPECTED`, `IMPLEMENTED`, `BUILT`, `UNIT_TESTED`, `INTEGRATION_TESTED`, `NATIVE_SEEN`, `PLAYTESTED`, `PROFILED`. A single item may have different statuses in different dimensions. Partial completion must be reported explicitly. No background promises or invented benchmark durations.

**Per-session output**: `BASELINE`, `SCOPE`, `DECISIONS`, `CHANGED`, `VALIDATION`, `PERFORMANCE`, `RISKS`, `NEXT`. Evidence table with exact paths and test output. End with smallest high-value next slice; do not plan arbitrary 30-step futures instead of finishing a feature.

## 9. Risk register and mitigation

| Risk | Consequence | Containment |
|---|---|---|
| Universal abstraction overdose | Months of code without playable result | Thin vertical slices; measure before layering |
| Global fine voxels | VRAM/RAM exhaustion, stalls | Sparse uniform regions + exact edit overlays + island refinement |
| Magic → energy perpetual motion | Broken economy/physics | Typed conversion graph, explicit sources, entropy/dissipation ledger |
| GPU simulation authority | Cross-driver divergences and readback stalls | CPU reference, event-owned authoritative state, tolerant model boundaries |
| Procedural interiors empty or blocked | Bad dungeon gameplay | Protected navigation volumes, role grammar, test traversals and artistic camera reviews |
| Wands generic/bulky | Noita parity failure despite lots of code | Hand-authored silhouette families, pixel icon inspection, actual Blender/FOV testing |
| Too many materials/recipes | Busywork and opaque combinations | Unique physical traits, curated high-value interactions, sensible alternatives |
| Overambitious 40 h orbit | Rushed narrative and incomplete midgame | Ring glimpsed early; usable space late; playable deep-earth first |
| Puzzle clue dead ends | Wiki-only secrets / unwinnable seed | Constraint solver, 2+ hint paths, recoverable items, human blind tests |
| AI agent conflicts | Lost code or broken shader branches | Worktree ownership, integrator, no auto resets, frozen captures |
| Performance 'optimizations' hide graphics | Lower time but worse world | Paired pixel/geometry and invariant gates |
| NPC ecological infinite spawns | Player frustration and simulation overload | Habitat-based population budgets; local threat logic |
| Dynamic rigid body explosion | Physically plausible but unplayable stalls | Cohesive body segmentation, aggregated debris, body count budget |
| Save corruption on generation updates | Permanent user data loss | Generator versioning, atomic journals, backed migrations |

## 10. Research source index and evidence date

Technical references verified before or on 2026-10-26:

- Noita engine/designer GDC 2019: https://www.gdcvault.com/play/1025695/Exploring-the-Tech-and-Design/
- Noita wand appearances and statistics: https://noita.wiki.gg/wiki/Wand / https://noita.wiki.gg/wiki/Wand_Appearance
- Factorio transport lines, sim optimization: https://factorio.com/blog/post/fff-176
- Factorio 2.1 quality/automation release notes (2026-10-23): https://www.factorio.com/blog/post/fff-449
- Transvoxel by original algorithm author: https://transvoxel.org/
- WGPU API 30.0.1 and experimental ray/mesh restrictions: https://docs.rs/wgpu/30.0.1/wgpu/
- WGPU timestamp queries example: https://wgpu.rs/doc/wgpu_examples/timestamp_queries/index.html
- Rapier/Parry voxels and localized changes: https://docs.rs/rapier3d/latest/rapier3d/geometry/struct.Voxels.html
- Blender 5 Python API, version-specific access: https://docs.blender.org/api/5.0/
- Aseprite batch export: https://aseprite.com/docs/cli/

**Freshness precaution:** A search result for a paper titled *Dynamic GPU Sparse Grids and Voxel-sequential Processing in NanoVDB* is marked **December 2026**, after this document's October 26 date. It is not treated as published current evidence or an implemented engine dependency. The idea of flat active-voxel iteration is included as a *research direction* to validate using currently available code and hardware.

**Source-specific references in this work:**

- User uploaded `Pasted markdown(20261026-113904).md` (Original Game Bible and Plan-Generation Protocol), retained the C-* codes and core narrative/plan protocol, with explicit normative corrections.
- Prior `00_MASTER_DESIGN_AND_INTEGRATION_V2.md` / *THE LIVING ENGINE* from user's file library: engineering/circuit/nested chip inspiration, no invented verification of current live code.
- Prior `Generator_55_Places_Quality_Overhaul.md`: six place families, observed R7 defects, seed and version targets; code evidence is a reported status, not a runtime audit.
- Prior `Axiom_Magitech_Art_Production_Prompt_Pack_2026-10-26.md`: visual grammar, Blender workflow, material shaders, asset families, true first-person evidence requirements.

## 11. What to build right now — first sprint definition

**Sprint 00 (bounded)** — perform full repository reality audit and produce one replayable **First Valley baseline** with frozen camera positions and CPU/GPU captures: first-person current wand, active old machine, castle roofs, town streets, mine/pit/library, horizon with tree canopies, starter cave. Avoid refactoring or adding features during this audit. Record actionable discrepancies between reported generator 54 R7 state and what the executable actually does.

**Sprint 01 (bounded)** — repair the most damaging observed integration invariant (world LOD/proxy hole / stale collision / physical roof geometry as evidence dictates) and improve one actual 3D asset to art acceptance, with pixel source and in-game view. Don't start 900 wand variants or a full space subsystem.

**Sprint 02 (bounded)** — implement *one* complete gameplay loop linking a mined resource, a machine with real inputs, a Noita-like spell affecting the world and a physically findable ruin clue. Demonstrate fixed seeds, actual camera footage, save/reload and CPU/GPU metrics. Only then expand catalog.

The engineering and narrative specifications should be **a forcing function for excellent playable slices**, not a pretext for leaving everything at the proposal stage.


<!-- END 04_IMPLEMENTATION_AND_VERIFICATION.md -->


---

<!-- BEGIN 06_REFERENCE_CONTRACTS_AND_SCENARIOS.md -->

# 06 — REFERENCE DATA CONTRACTS, ALGORITHMS AND ACCEPTANCE SCENARIOS
**Status:** illustrative design-level code/schema examples, **NOT** current repository types or executable fixtures. Intended to guide system-level SDDs and integration tests after repo reconnaissance. All dimensions, bounds and formats require ABI/serialization review.

## 1. Stable IDs, coordinate transforms and lifetime ownership

```rust
// Sketch only: real names, integer quantum, traits and serialization are repo-defined.
struct AbsolutePos { base_units: [i64; 3] }
struct CellExtent { origin: AbsolutePos, log2_size_units: u8 }
struct WorldRegionId(u64); // identity independent of streaming array index
struct ObjectId(u128);      // persistent identity, generated deterministically or saved
struct MaterialId(u32);    // registry-versioned, migrated by stable string ID
struct WorldRevision(u64);
struct LocalPortId(u16);
struct Tick(u64);

struct WorldChange {
    revision: WorldRevision,
    bounds: (AbsolutePos, AbsolutePos),
    kind_bits: u64,
    touched_regions: Vec<WorldRegionId>, // compacted journal; not hot path heap / voxel
}

struct ArtifactRevision {
    own: (WorldRegionId, WorldRevision),
    adjacent: Vec<(WorldRegionId, WorldRevision)>,
    registry_generation: u64,
}
```

**Design rules:** 64-bit or 128-bit IDs are stable across reload, versioned registries remap material IDs, derived IDs are never player save IDs. A world edit modifies canonical data *once*. GPU allocations and materials are generated from stable source IDs; they do not own matter. Ghost/resident/LOD representations have explicit owner to prevent duplicate visible surfaces and double CPU simulation.

**Voxel-size invariants:** Each cell occupancy is measured in exact world units; face contact uses rational dyadic alignment. Restriction sums extensive quantities, while intensive parameters are computed using material composition. Mixed-resolution collision uses physical world coordinates with no hidden rounding that changes passage clearance. Erosion removes matter by actual volume and may generate appropriately quantified item or particulate output.

### Mesh publication pseudocode

```text
request_mesh(region, resolution):
  deps = kernel.snapshot_source_revisions(region, neighbors, palette)
  geometry = mesh_from_snapshot(deps.snapshot)
  if !deps.still_current(): discard(geometry); optionally re-request
  else upload_behind_existing_parent_proxy(geometry)
  if upload_finished && deps.still_current():
      publish_new_geometry_coverage(region, resolution)
      retire_old_geometry_after_gpu_completion()
  else retain_valid_old_parent()
```

**Edge cases:** parent may not represent microscopic hole visually, but parent is not permitted to *physically cover* a near active traversable edit. Locally refine the needed region or attach an exact render/collision edit overlay. Cross-chunk quad decisions require dependency on neighbor revisions. After ref change, invalidate temporal history and occlusion conservative coverage; no stall can turn into an empty screen region.

## 2. Proposed material definition and runtime state

```ron
// EXAMPLE conceptual schema (not a claim of current parsable RON):
(
  id: "underkingdom:copper",
  family: Metal,
  reference_density_kg_m3: 8960.0,
  heat_capacity_j_kg_k: 385.0,
  thermal_conductivity_w_mk: 401.0,
  electric_resistivity_ohm_m: 0.0000000168,
  softening_k: 900.0,
  melting_k: 1357.77,
  mechanical_class: DuctileMetal,
  tags: [Conductive, Smeltable, Oxidizes],
  base_texture: "materials/copper_clustered",
  palette: "metals/warm_copper_v1",
  response_class: Metal,
  dynamic_overlays: [Wet, Hot, Corroded, Cracked],
  reactions: ["underkingdom:copper_oxidation", "underkingdom:acid_copper"],
)
```

**Note on accuracy:** Values are illustrative physical baselines, not a promise that a game should simulate exact chemistry or all temperature ranges. Definition must explicitly state which are absolute-world quantities and which are stylized coefficients. The runtime material cell references `MaterialId` and stores only relevant local state; high-precision values belong in material tables, not every voxel.

**Reaction rule requirements:** stable id, reactant selectors by tag/material, priority/conflict ordering, temperature/pressure/contact prerequisites, bounded rate expression, stoichiometry/material ledger, energy ledger, products, persistent byproducts, human-readable explanatory diagnosis, VFX class and tests. The fallback `NoReaction` is a valid fully specified resolution. Source claims of '30% every pair reacts' are not a shipping requirement.

### One conservative reaction step

```text
for each relevant active contact interface (source_region, target_region):
  choose allowed reactions by material tags & conditions
  compute candidate flux from contact area, dt, available reactants, catalyst
  clamp to source and destination material/energy capacities
  reserve reactants and energy on both sides
  commit stoichiometric removal/product addition together
  add explicit heat/pressure/event changes
  schedule affected regions, no full-world scan
```

**Tests:** symmetric contacts don't double-react; two reactions competing for the same reagent don't consume it twice; coarse↔fine face computes same integrated mass flux and respects edit overlays; reloading with a half-processed batch cannot produce it twice.

## 3. Machine ports, graph compilation and transaction ledger

```rust
#[derive(Clone, Copy)]
enum PortDomain { Cargo, Fluid, Gas, DcElectric, AcElectric, Shaft, Heat, Signal, Aether, Bio }

struct PortKey { owner: ObjectId, local: LocalPortId }
struct TypedPort {
    key: PortKey,
    domain: PortDomain,
    frame_local_m: [f32; 3], // visual / connector transform, not save authority
    facing: Direction,
    capacity: CapacityInDomainUnits,
    mode: PortMode,
}

struct ConversionRecipe {
    input_ports: Vec<PortKey>,
    output_ports: Vec<PortKey>,
    transfer_rule: StableRecipeId,
    efficiency_or_source_model: EnergyModelId,
}
```

A machine may advertise DC energy and heat input, fluid intake, exhaust, cargo in/out, logic channel and mana port independently. The compiler finds stable connected typed ports from voxels/anchors, collapses long wires, belts and tubes into segments, then caches graphs until valid connections change. Graph indices are scratch; stable port keys persist.

**Ledger rule:** `source.available >= requested` and destination capacity sufficient, otherwise reserve partial work or reject. Consumption, production, heat and byproduct all commit in one stage. If a brownout occurs, machine workload advances only by actual received Joules, not requested watts. Network cache cannot temporarily deliver more than source stored energy during rebuild.

### Example multiblock machine contract

```ron
(
  id: "underkingdom:basic_foundry",
  appearance_family: "engineer:clay_forge",
  world_units: Meter,
  validation: (
    min_chamber_volume_m3: 1.0,
    needs_continuous_refractory_shell: true,
    needs_heat_input: true,
    needs_exhaust_path: true,
  ),
  ports: [
    (name: "ore_in", domain: Cargo, side: North),
    (name: "fuel_in", domain: Cargo, side: West),
    (name: "exhaust", domain: Gas, side: Up),
    (name: "heat", domain: Heat, side: South),
    (name: "ingot_out", domain: Cargo, side: East),
  ],
  simulation: "underkingdom:foundry_thermal_v1",
  visual_states: [Idle, Heating, Processing, Starved, Damaged],
  blueprint_source: "industrial/early_foundry_1",
)
```

Save design authority, material inventory and internal thermal energy, not simply recipe progress based on instantaneous UI. A removable shell block invalidates chamber geometry, not persistent input items. Machine repair should be user-visible and explain what changed.

## 4. Shared Work virtual machine / glyph and controller equivalence

### Typed IR examples

```text
LOAD_SENSOR   port=temp_sensor       -> r0 : Temperature
CONST         700K                   -> r1 : Temperature
COMPARE_GE    r0, r1                  -> flag
JUMP_IF_FALSE flag, label:safe
REQUEST_PORT  output=coolant_valve, value=Open, auth=owner
WAIT_TICKS    6
RETURN
```

Engineer ladder/text and Chorister rune diagrams both compile into the same typed IR. A compiler is allowed to reject unsafe/meaningless operations rather than silently reinterpret them. A spell requires the correct energy/material authorization, target queries and effect constraints. A controller's program cannot mutate arbitrary voxels by pointer.

**Sandbox fuel:** bounded instructions per active machine/trigger; bounded persistent registers, event lists, output effect bytes/entities and recursion. A pathological spell chain must fail in a coherent, visible way and cannot corrupt the world/consume infinite CPU; sometimes sim aggregation preserves the physical outcome of a huge number of identical events.

**Cross-dialect acceptance scenario:** a rune-powered controller and an electrical logic controller both read a shared sensor, actuate a physical cooling valve through an authorized port and produce **equivalent** resulting valve/machine state under matching inputs. The devices may have different energy costs and casting timings; semantics equal where the typed operator matches. Verify save/replay determinism for the authoritative action trace.

## 5. Declarative procedural `PlacePlan` and world secrets

```ron
(
  site_id: "engineer:abandoned_waterworks",
  generator_version: 55,
  biome_filter: [MountainValley, Karst],
  architectural_tradition: Engineer,
  site_mass_plan: "waterworks_gatehouse_and_tank",
  protected_spaces: ["main_approach", "primary_pump_hall", "spiral_stair"],
  route_nodes: ["surface_gate", "sluice", "service_tunnel", "main_chamber"],
  physical_anchors: ["reservoir", "pump_mount", "generator_mount", "return_pipe"],
  light_plan: "cool_stone_with_local_warm_utility",
  functional_mechanisms: ["gate_valve", "pump_control", "backup_generator"],
  secret_hooks: ["dam_water_antechamber_hint", "archive_cipher_fragment"],
  min_distinct_routes: 2,
  artifact_palette: "engineers_weathered_steel_limestone",
)
```

`PlacePlan` stages produce real usable rooms and structure. Verify route graph traversability using actual player collision capsule and clearance, not only a 2D open/closed map. Reserve major voids before furnishings/foliage. Attach lore according to economy/use/downfall: why waterworks exists, which civilization maintained it, why it failed. Generate multiple approaches and one salvageable subsystem the player can reuse.

### Secret-rule example with fairness guarantees

```ron
(
  id: "chorister:gate_of_sustained_chord",
  rule_family: ResonantGate,
  generation: (requires: [ChoristerRuin, ResonanceSource, SideAccess]),
  conditions: [
    (kind: SignalPattern, source: "local_resonators", expected: "chord_template_seeded"),
    (kind: DurationTicks, min: 180),
  ],
  clues: [
    (place_in: "ritual_archive", clue_type: Inscription),
    (place_in: "adjacent_instrument", clue_type: MeasurableResonance),
  ],
  physical_effect: "retract_ancient_gate",
  alternatives: ["power_repair_and_bypass", "hard_rock_excavation_with_cost"],
  permanence: PersistentIfMechanismRemainsActive,
)
```

**Fairness test:** seed/site validator ensures at least two different clue types exist in accessible places; required instrument/item is recoverable; if the main gate is permanently disabled, construction/knowledge alternatives remain physically possible. Human blind testers can infer the mechanic from evidence. Do not invent world state by freeform generated lore text.

## 6. Scenario harness and nested test coverage

For each feature, define expected **code-level, world-level and player-level** outcomes:

| Fixture | Action | Numerical/invariant oracle | Player-visible proof |
|---|---|---|---|
| `coarse_fine_acid_wall` | Pour acid across 1 m/0.125 m seam | Mass/enthalpy within declared bounds, no duplicated interface flux | Seam erodes consistently; nearby cave still collidable |
| `water_and_wire` | Flood a low-voltage circuit cabinet | Typed conductor/water interaction; breaker policy and charge unchanged except real debit | Fault, sound, meter diagnosis, responsive machine stop |
| `torch_library_nave` | Enter after sunset | Same physically tagged materials and source lights; no unjustified color-space gamma double conversion | Cool stone readable with warm light islands |
| `wand_trigger_multicast` | Shoot trigger with stored chained effect | Deck order/cost/event provenance and bounded fuel | Visually distinct second cast and real impact |
| `cast_vs_machine_vm` | Equivalent magic and electrical control programs | Same typed state transition, energy-accounted | Both devices operate real gate when supplied |
| `ruin_three_routes` | Pump/freeze/cut and traverse | Actual reachable paths and correctly consumed resources | All 3 approaches playable, not just quest completions |
| `rover_hull_pressure` | Breach moving vehicle under water | Reservoir/air coupling and collision stability | Flood, crew task, repair and altered mobility |
| `tree_lod_during_flight` | Rapid fly across seam near old generator site | No missing parent coverage, stable dependency revisions | No disconnected canopy or silhouette popping |
| `save_on_belt_split` | Remove busy belt and save mid-transfer | Exactly-once lot ownership before/after reload | Goods remain on correct pieces/in inventories |
| `foundation_collapse` | Remove support then drop attached volume | Detached object mass and momentum based on world scale | Coherent falling structure, not 100k independent bodies |

**Failure reports** should include `seed, generator_version, world_edits, tick, relevant revisions, asset_sha, app_exe_sha, gpu_backend, hardware profile, camera, debug-overlay settings, numeric residuals, exact commands`.

### Bench thresholds: set from observed baselines, not invented figures

- Compare H0/H1/H3 with a fixed full scene and fixed camera route. Store p50, p95, p99 frame, CPU/GPU timings, active islands/voxels, pending jobs, resident GPU arenas and texture atlas bytes.
- Profile cold and warm caches; capture first frames after teleportation, high-velocity flight, large collapse, sudden high-heat field, night/day transition and prolonged factory activity.
- **Visual truth is required:** missing world or unused spell effect does not count as a successful performance optimization.
- Keep user-facing title screen, hotbar, inventory and held item responsive under heavy world jobs.

## 7. SDD-to-code handoff acceptance checklist

- [ ] No authored code claims a non-existent repo file/API.
- [ ] One named gameplay goal and one reproducible canonical world scenario.
- [ ] Data ownership and persistent ID lifecycle defined.
- [ ] Numeric unit/rounding/model accuracy and error policy defined.
- [ ] Cross-grid and cross-chunk effects/tests covered where relevant.
- [ ] One minimal real player affordance, including accessibility where applicable.
- [ ] Actual Blender/source art to game importer path proven if visual asset.
- [ ] Old save/generator and concurrent worktree safety addressed.
- [ ] Native game screenshots separate from shader/build/tests.
- [ ] Perf delta measured if runtime available; otherwise NOT MEASURED.
- [ ] Scope excludes unrelated work and no giant speculative refactor.

**Implementation note:** The RON samples are illustrative user-facing authoring data. Production SDDs must define final schema types and validators, including safe untrusted-mod resource caps, stable naming, defaults and explicit migrations. H0 OpenGL/no-compute path needs actual capability checks rather than untested code guesses.


<!-- END 06_REFERENCE_CONTRACTS_AND_SCENARIOS.md -->


---

<!-- BEGIN 05_CODING_AGENT_STARTER_PROMPT.md -->

# 05 — CLAUDE CODE / AUTONOMOUS CODING AGENT MASTER STARTER
**Use:** Paste the block below at the start of a **real source-code session** after giving the agent the documents `README_START_HERE.md`, `01_GAME_DESIGN_BIBLE.md`, `02_ENGINE_ARCHITECTURE.md`, `03_ART_CONTENT_AND_WORLDGEN.md`, `04_IMPLEMENTATION_AND_VERIFICATION.md`. **Not permission to rewrite the entire repository in one session.**

---

## PRIMARY AGENT PROMPT (copy/paste)

You are the lead senior Rust/WGPU graphics-and-simulation engineer, technical gameplay director and adversarial implementation integrator for an existing, actively edited multiresolution 3D voxel game, provisionally titled **Underkingdom** with engine **Axiom**.

**The game thesis is:** "Matter has rules. Rules can be learned. Learning them is power." It is a cohesive, extraordinary, beautiful SNES/GBA-inspired 3D successor to heavily modded Minecraft and Noita, with Factorio-grade automation, physically integrated magitech, functionally destructible structures, FTL-style vessels, named Zork-like places, secrets, civilizations and an extensible shared control/rune VM. This is a *long-term product direction*, not an order to implement it all at once.

Read all four U1 specifications and the README. The uploaded **Game Bible and Plan-Generation Protocol** is the source for the Underkingdom lore, C-* claims and intended execution discipline. The U1 claims matrix explicitly revises impractical requirements and takes precedence where conflicts are explained. The style pack and Generator-55 review specify actual art and worldgen failure cases.

### First required action: inspect reality, not memory

1. Load `CLAUDE.md`, `AGENTS.md`, workspace `Cargo.toml` / `Cargo.lock`, `git status --short`, branch name and ongoing worktrees. Check whether this is the actual generator-54 R7 workspace, **not** an older `3d-falling-sand-prototype` repository.
2. Map real authoritative world storage and voxel scales, simulation logic, rendering/meshing and horizon LOD, generator versions/save schema, shader/material code, current held-wand/viewmodel representation, machines/inventories, power and fluids, worldgen and UI.
3. Inspect real Generator-54 reported art captures from `target/place-review` and versioned source notes if available. Check actual build hash and image receipts. Open the four named reference paintings in Windows Downloads only if the environment actually permits it. Never claim to have seen inaccessible images.
4. Determine which Rust/WGPU modules and files are owned by other active agents, especially terrain/water/horizon/shaders/UI. **Do not overwrite unrelated dirty changes, reset, clean, auto-stash, push, commit or merge unrelated work.** Work in an isolated scope or escalate precise ownership collision.
5. Run the smallest baseline build/tests and launch the **native game** when possible. Record actual output. Do not equate build, test, native render, played result and measured performance.

### Your current deliverable: the smallest high-value integrated milestone

Create an evidence-linked reality report and implement **one small but complete** improvement according to actual blockers. Prefer, in order:

- Eliminate a demonstrated world LOD/proxy/mesh/collider mismatch causing visible terrain or structural errors (especially chunk holes, disconnected tree canopies and 1 m jagged roofs).
- Produce **one excellent Blender-authored and pixel-textured item or machine**, with actual model, UV/source texture, importer/exporter and first-person/in-world WGPU proof. Reject lantern-like, bulky, generically cyan glowing wand designs; prioritize strong original silhouette and coherent art.
- Add the minimal real gameplay interface linking one mined material, one working processing machine and one Noita-like physical spell, preserving exact inventory/matter/energy ownership.
- If source limitations block these, establish a reproducible failing test/scene and implement the smallest enabling API; do not fabricate a completion screenshot or pretend an entire system exists.

### Implementation standards

- Rust: lean modules, typed stable IDs, ownership clarity, hot loops with cache-friendly layouts, very few unnecessary allocations/indirections. Use existing conventions, not a global ECS or second renderer. New dependencies require concrete cost/benefit justification.
- World: canonical sparse state and versioned exact edits, scale-aware world volume, revision-aware derived caches, deterministic generation by version. Old worlds and player edits must be safe.
- GPU: use existing WGPU render pipeline, profile with actual timestamps where available; do not rely unconditionally on experimental ray tracing or mesh shaders. No expensive universal material shader.
- Physics/machines: authoritative typed resources, no item/energy duplication or silent teleporting. Material interactions tested across active region boundaries; complex phenomena use honest coarse approximations if necessary.
- Gameplay: physical actions must be visible, usable and understandable. No machine that operates only via an empty animation; no wand that only looks nice in a Blender render; no dungeon corridor that blocks the actual player capsule.
- Visual: SNES/GBA-like pixel clustering, Chrono Trigger/ALTTP shape/palette discipline, Minecraft-compatible block grammar, distinctive material shaders and physically meaningful original magical effects. No arbitrary grunge, global pixel filter or big teal/brass energy battery wand.
- Accessibility: distinguish normal simple interaction from expert fine-grid/circuit/wand editors. No forced tutorial and no wiki-only first automation.
- Modding: prefer real data-driven material/item and machine schema, with validators and consistent IDs. Do not implement hundreds of decorative varieties before one functioning archetype is approved.
- Parallel development: declare write ownership and communicate narrow interfaces. Do not alter concurrent hot shaders or generator code without consent/hand-off.

### Scope and completion protocol

1. **BASELINE:** file+function location evidence, run commands, seed/camera, current screenshot; what was reported vs directly inspected.
2. **ONE-SLICE PLAN:** precise player-visible problem, bounded files, invariants, perf/correctness/visual acceptance, at least one rejected alternative.
3. **IMPLEMENT:** minimal complete path; remove redundant old paths where safe and documented; don't grow a new framework from the slice.
4. **TEST:** unit/property/replay/CI where available plus real save/load, mixed resolution, collision/interaction and shader validation.
5. **LOOK:** native gameplay screenshots under neutral and real game lighting; inspect near/mid/far and first-person. Blender source and image-only proof are distinct statuses.
6. **MEASURE:** CPU and GPU times, p95/p99, active cells/bricks, GPU residency/uploads and performance delta in a fixed scene. If measurement unavailable, mark **NOT MEASURED**, never supply a guess.
7. **CRITIC:** an independent reviewer or adversarial self-review tests for bugs, duplicate ownership, visual lies, stale jobs, coherence and maintainability.
8. **REPORT:** `BASELINE / OWNERSHIP / CHANGES / VALIDATION (PASS/FAIL/NOT RUN) / VISUAL / PERFORMANCE / RISKS / NEXT` with exact file paths and reproducible evidence; no unrelated promises.

### Anti-bloat and anti-fake-finish rules

- A zero-error compile is not a successful visual or gameplay milestone.
- No global fine-cell allocation as solution to roof details.
- No sinking a distant heightfield to cover mismatched near terrain.
- No heavy runtime that evaluates every known material pair everywhere.
- No global plant/sand physics while regions are inert.
- No magic circuits producing net unlimited passive energy unless defined as a source with explicit costs.
- No documentation boasting “900+ wands” when there are 900 procedural recolors of three ugly silhouettes.
- No mandatory full CFD/atomistic/semiconductor emulation to make gameplay interesting.
- No irreversible save/civilization wipe due to unannounced story event in normal persistent play.
- Do not claim success from any reference images you could not actually inspect.

### Expected immediate response

Provide the reality map, exact planned slice (one!), risks and baseline numbers/images where actually obtained; then implement and verify it in the current working environment. If interrupted, leave an accurate handoff document with the smallest next executable task. **Prioritize playable quality and preserved data over speculative completeness.**

---

## OPTIONAL FOLLOW-UP PROMPTS AFTER THE FIRST VERIFIED SLICE

### A. Multiscale rendering and worldgen proof

> Using accepted U1 contracts, implement one real fine-resolution roof + tree + cave transition scene: generator-54 frozen baseline / new generator 55 only if changed, valid parent LOD coverage, physical collision matching roof, no yellow torch wash from shader correction hacks, and repeatable player traversal. Capture actual 1080p and 1440p near/mid/far with source hash and CPU/GPU costs. Only touch files you own.

### B. Magic composition and visual proof

> Build six exceptionally distinct original wands (not heavy lantern heads) and 12 compositional spell primitives with ordered/shuffle/trigger semantics, a functioning real physical effect each, expressive casting VFX, and player-inspectable execution. The first deliverable is an in-game playable testing gallery, not a rendered parchment sheet or a million fake combinations. Export actual Blender or 2.5D sources and show held/inventory/dropped proof and budgets.

### C. Factory + magical interface proof

> Create one functional workshop from raw mined material to stored processed output using existing machines, a compiled belt run and stable power. Integrate one magic effect through the shared typed port/event API with actual debit and physical consequence. Demonstrate no duplication on wire cut, loaded belt split, save/unload and concurrent edits. Provide a performance comparison vs baseline.

### D. Physical dungeon / secret chain proof

> Generate one unique two- to three-level engineer/chorister ruin with an original readable room silhouette, water/power network and physically valid locked gate. Prove pump/freeze/build or alternate solutions in native game; distribute at least two physically observable clues on deterministic seeds; preserve repairs and changed water volume after reload. Capture actual interior camera views, check light palette and traversal, and reject generic empty ring rooms.

## FINAL REMINDER

The best-looking screenshot is not the best game. The best game makes its visuals, material physics, modpack engineering, exploration, secrets and first-person feel reinforce each other while staying fast and robust. **Ship a measured, playable, coherent improvement every session.**


<!-- END 05_CODING_AGENT_STARTER_PROMPT.md -->
