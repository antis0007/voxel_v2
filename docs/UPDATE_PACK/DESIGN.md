# AXIOM — Reviewed game and engine design

**Revision:** R1, 29 October 2026. **Target executor:** Codex with the user's Astra Light configuration. **Status:** reviewed design, not implemented or playtested game functionality. **Platform:** existing Windows-first Rust + raw WGPU project. Preserve the repository's actual names, APIs, saves, assets, and established content.

This is the product and technical reference. Execute one task from `TASKS.md`, not this whole document as a single patch. `START_HERE.md` is the execution entry point. `REVIEW.md` records changes to earlier advice, sources, and unresolved risks. `ACCEPTANCE.json` retains the 122 reviewed upstream requirements and adds focused tests; every game-test status initially remains `NOT_RUN`.

## D00. Authority, scope, and definition of success

The game is a persistent, editable magitech world. Players build comfortable homes, investigate unfamiliar terrain and materials, make useful machines and spells, form relationships, explore underground and eventually beyond the planet, and return to places changed by their actions. Exploration and construction are not interludes between combat upgrades. Automation is not a prerequisite for enjoying the world.

**Preserved decisions:** native Rust/raw WGPU, no Bevy or replacement engine; variable voxel resolution and persistent edits; physically consequential magic and technology; beautiful deliberate pixel-material art; single-player completeness and cooperative play; seed-stable initial geography/history; authoritative outcomes with relevant client physics; exact command replication preferred only for domains where it is actually proven; early storage and graceful movement; planetary/portal/spacecraft ambitions; historical worlds must not be silently regenerated.

**This revision changes execution and weak proposals, not those intentions.** It replaces infinite feature escalation with bounded tasks; mandatory full physical simulation with explicit useful models; arbitrary diversity quotas with editorial review; and definitive screenshot diagnoses with reproducible evidence. Numbers marked **trial** are starting experiments, not established balance or hardware capacity. Algorithms marked **candidate** need a comparison against the current implementation. Deferred functionality is not deleted from the vision.

Authority order: latest explicit user decision; verified existing compatibility/ownership contracts; this review's explicit corrections; the current consolidated source directive; earlier compatible briefs; illustrative examples. If an existing file contains a real conflict, record the competing statements and choose the smallest safe reversible step. Do not quietly erase a user requirement. Technical version statements and external performance claims from older conversations are not authoritative.

A passing contribution produces a specific player benefit with evidence appropriate to the risk. A save fix may need no new artwork. An art change needs native visual inspection. A gameplay system needs interaction and persistence. No model, prompt, automated metric, or source-code count certifies that the game is fun or the fastest engine ever made.

## D01. Creative direction and anti-generic rules

### Identity

A serious but hospitable science-fantasy world, layered with nature, useful work, historical damage, and recovery. It can be playful, frightening, melancholy, beautiful, and occasionally absurd without turning every place into a joke or every mystery into a malfunctioning computer. The Work names an inherited discipline of manipulating matter and relationships; it is not a demand that all lore become one universal VM.

Engineers, Choristers, Founders, and present societies are overlapping traditions, not three exclusive species, three neat depth bands, or three mandatory technology tiers. Some communities thrive. Some buildings are new. Some mountains are simply mountains. Very old surviving machinery needs a plausible preservation/maintenance premise, but not every rock needs a lore explanation.

Reference roles remain distinct. Minecraft contributes tactile construction and useful modular content; Noita contributes compositional magic and material discovery; Factorio and technical modpacks contribute logistics and understandable optimization; Terraria contributes rewarding depth and capability-changing equipment; RimWorld contributes inhabited spaces and personal continuity; Seed of Andromeda contributes planetary continuity; Zelda contributes spatial readability and traversal. SNES/GBA and Hyper Light Drifter inform shape and color, not copied assets. Threads informs consequences and resilience, not compulsory suffering. Ukrainian/Slavic influences require situated traditions and cultural review, not an interchangeable monster roster. Space-opera influences expand the horizon without replacing the homeworld with a menu of loot themes.

### Editorial vetoes

Reject new content whose description is mostly adjectives, whose only novelty is color, or whose interaction is another power switch opening another loot room. Reject automatic ancient laboratories under every village, arbitrary keys in threes, generic cosmic names as substitutes for design, obligatory NPC rescue, one-resource planets, and a random mix of pipes/crystals/gears that communicates no function.

Do **not** reject all repetition. Familiar tools, understandable roof kits, recognizable animals, dependable workshops, and routine production create mastery and belonging. Remove repetitive decisions that no longer require thought; retain satisfying actions and coherent motifs. Quiet scenery does not owe the player a reward.

Authored material is necessary. Excellent plants, enemy roles, architectural joints, mechanisms, dialogues, music, and exceptional dungeons come from deliberate production. Procedural systems arrange and vary them under constraints. Pure simulation does not guarantee composition; pure composition does not excuse false physics.

### Admission test for a substantial feature

State its player-facing purpose, visible evidence, usable actions, persistent consequences, and worst relevant cost. Identify one existing system it strengthens. When a feature is primarily aesthetic, say so; it does not need a fictitious simulation justification. If removing a supposedly consequential simulation leaves every player decision unchanged, simplify it or postpone it.

## D02. Ordinary play is the first deliverable

### The first-session contract

A new player can move comfortably, inspect, gather, place, mine, store, craft, prepare food, light a shelter, and use a useful basic tool or spell without completing research, fighting a boss, or assembling an expert factory. They can reasonably discover more than one next intention. This is not a timed checklist or a guaranteed sequence of tutorial events.

Keep existing compatible controls. Improve selection accuracy, reachable face highlighting, rotation/snapping, placement previews, break progress, interaction priority, and feedback. A failed action identifies its relevant reason: occupied space, missing material, insufficient tool capability, pressure hazard, permission, or pending destination—not an unexplained denial. Development queue jargon stays out of ordinary UI.

Physical dimensions and cursor semantics remain stable across voxel resolutions. A large excavation brush and a precision chisel are different tool modes, not different inventory economies. Display affected volume and recoverable yield when useful. Fine editing must not require manual resolution bookkeeping for every wall.

Early construction includes doors, stairs, roofs/eaves, windows, lamps, storage, basic furnishings, farming surfaces, simple mechanical interfaces, and repair. Generated architecture uses the same accessible construction vocabulary wherever feasible. A beautiful generated window cannot be an untouchable visual lie when players are promised editable buildings.

### Survival without a chore treadmill

**Trial default:** no hunger-related sprint cutoff; food heals and offers a small number of understandable preparation benefits. Limit simultaneous meal effects through transparent categories rather than encouraging consumption of fifty buffs. Plants, cooked dishes, medicines, and familiar meat/food choices have useful alternatives. Ingredient behavior is learnable. Common recipes and essentials are visible; rare interactions may remain discoveries.

Temperature, air, gas, pressure, and exposure matter where their conditions create choices. Ordinary temperate home life does not require constant meter maintenance. Ventilation and extreme environments can matter without simulating every gas voxel in every room.

**Trial default recovery:** retain equipped tools and permanent knowledge; recover loose expedition cargo from a persistent, non-despawning recovery container or an explicit recovery service. A prepared return option should be easier than suicide. Severe modes can change this. Do not permanently reduce maximum health after repeated failures as the default: that can make the struggling player progressively less able to recover. Do not add a repair fee merely to make death inconvenient. Test stakes, recovery time, and willingness to experiment with real players.

A secure home can remain secure. Fire and industrial risk are real, but warnings, breakers, isolation, containment, reasonable defaults, and accessible repair prevent ordinary play from becoming constant disaster recovery. Cosmetic and safety settings must be clearly separate from canonical multiplayer difficulty.

## D03. Motivation, knowledge, and fair progression

### Player intentions, not compulsory classes

Builder, explorer, engineer, mage, naturalist, and pilot are overlapping play preferences. Solo players can use presets, companions, commissioned services, salvage, and accessible tools; no core expedition requires several human roles. Cooperation should create choices, not mandatory busywork for the least interested teammate.

Progression is a capability graph: recover or learn a useful method, apply it, make it reliable, and scale it when desired. Salvage can leapfrog fabrication for use without granting immediate mass production. Knowledge can improve targeting, access, efficiency, or interpretation; it is not always a numerical damage bonus.

The inherited pacing intention places a first self-built crewed atmospheric departure around the latter middle of the intended capability journey, approximately 60%, with an orbital return soon after. This is a playtest hypothesis, not a literal percentage meter, quest lock, required playtime, or instruction to withhold early gliders. Preserve it until evidence or the user changes it.

### Discovery value and automation

Factories excel at repeatable quantity, reliability, precision, and convenience. Exploration excels at unfamiliar situations, evidence, relationships, new processes, routes, specimens, places to inhabit, and surprising uses for familiar tools. Explorers also receive useful ordinary resources; do not replace all tangible rewards with lore.

Do not enforce equal iron-per-hour among every playstyle. Instead compare whether a builder, explorer, or keeper can afford representative personal projects without forced farms. Factories legitimately outperform hand labor at industrial volume. Test the travel, sorting, recovery, and setup time of alternatives, not only raw drops. Avoid a universally convertible farm currency buying every discovery and permanently dominating every reward.

Drones and instruments may discover real information. There is no arbitrary human-only discovery flag. They do not automatically interpret everything or provide every tool, social relationship, physical access route, or unknown technique. Powerful mobility and automation should solve old inconveniences rather than trigger invisible restrictions that preserve the original obstacle forever.

### Guidance contract

Use one knowledge store beneath the Primer, map, notebook, radio, and optional project interface. Records distinguish **Observed, Reported, Inferred, Tested**, with subject, source, time/revision, uncertainty, and explicit sharing scope. An observation can be incomplete; a report can be biased. System facts and character interpretations remain distinct.

Guidance reads only authorized known facts. It may suggest a next experiment or required capability, not expose a hidden cave because the generator has its coordinates. Give useful optional methods, proven starter blueprints, and honest instrument limits. Do not lock normal functionality behind reading the Primer or following its sequence.

Support unassisted, subtle, and structured help without mechanical penalties. Let a player pin their own project, hide suggestions, rename places, share a map, or simply ignore the system. Information needs accessible non-color/non-audio alternatives. Never infer that quiet building means boredom and spawn a raid to fix it.

## D04. Storage, construction, and the resource economy

One item/lot and transaction model serves player inventory, containers, machines, crafting, vehicles, trading, and pickup. Stack compatibility uses intentional material/quality/state classes, not a unique float signature for every pebble. Stable item or lot identity is required where ownership or history matters; bulk identical material can use aggregate quantities.

Provide quick stack, sort, search, locked/favorite slots, clear capacity feedback, repair, reasonable bulk stacks, and early portable storage. The prior 256-unit bulk stack and approximately 16 m local storage scanner are **trial data values**, not engine constants. Index only eligible accessible containers; the scanner is not infinite storage, unlimited wireless freight, or permission to take another player's cargo. Moving a container or closing access updates eligibility once.

Variable voxel sizes require conserved volume/mass accounting. Removing eight half-edge cubes must recover the same declared material as one full-edge cube, subject to identical process loss. Cutting material into finer pieces cannot increase recipe output, inventory capacity, fuel value, or experience. Use checked integer/fixed-quantum amounts where suitable; retain remainders rather than rounding every action independently upward. Blueprint substitutions and mixed shapes use actual occupied material, not bounding-box volume.

Operations validate permissions, source generation/revision, aggregate input demand, output capacity, and conflicts. Reserve or stage transfers according to process semantics; record operation identity so retries do not repeat consumption. Prediction may show an item provisionally but cannot spend it before authority. Moving a chest onto a vehicle or through a portal changes its frame/custody without copying contents.

Common stone, sand, metals, fiber, and fuel have several practical routes. Processing can recover more useful material from ore and tailings; it cannot duplicate an element without an explicit input/transformation. Humane animal/plant/cultured/salvage routes should be effective, not token ethical options at a crippling productivity penalty. Harvest and hunting may exist without making protectors or villagers the best source of ordinary ingots.

## D05. World authority and coordinate contract

### One authority, several representations

Keep three categories explicit:

1. **Immutable generated baseline:** seed/version/content-bound geology, regional history, initial terrain/species/site facts.
2. **Authoritative changes:** player edits, current material and entity state, inventories, living populations, machine batches, social facts, realm progression.
3. **Derived products:** meshes, LODs, light/shadow summaries, cooked collision acceleration, maps, navigation caches, UI and visual animation.

A derived product is not allowed to invent authoritative material. Collision acceleration is derived, but interactions use a current exact-enough physical representation consistent with authority. Render simplification does not change loot, pressure, attack cover, or building support.

Queries distinguish ready occupied, ready empty, approximate, pending, and stale. Unknown is not automatically air or a solid wall. A task-specific conservative bound may reject or nominate a candidate; it cannot certify a ready playable surface. Hold a portal inactive or defer a new remote action visibly rather than let an actor fall through absent data.

### Space and scale

Retain the actual viable topology; do not introduce a new universal octree by decree. Use exact hierarchical/dyadic local cell addresses, stable region/body/frame IDs, high-precision inter-frame transforms, and small local floats for rendering and supported physics. Subtract origins before f32 conversion. Render origin changes do not silently move authoritative physics.

Separate physical cell size, authored texel density, geometric LOD, collision approximation, and simulation resolution. Uniform regions compress; active bricks use contiguous hot data. Benchmark 8³/16³ or existing brick shapes only when a measured bottleneck justifies it. No entity, allocation, lock, or rigid body per ordinary voxel.

Body/planet/ship/portal frames form a versioned transform hierarchy without cycles. Position, orientation, point velocity, angular velocity, local gravity, and attachments follow explicit conversions. Spherical mapping needs real adjacency and metric tests at face seams/poles. Preserve the existing working local world while validating this; planetary ambitions do not authorize a speculative coordinate rewrite of every subsystem.

### Edits and publication

An edit has a bounded footprint/read set, material accounting, actor/operation identity, and source generations. Update canonical geometry and critical collision/interaction together at the declared simulation step. A simple immediate patch can reveal new faces while an optimized mesh cooks; an erase mask alone cannot represent the newly visible interior.

Each cache records only dependencies that can affect it. Mesh inputs include relevant neighboring boundaries, not unrelated inventory revisions. Reject stale completions; do not roll back a whole chunk over other players' changes. Unknown support is not evidence a structure detached. Work queues coalesce reconstructible stale products, never paid effects or accepted gameplay history.

### Persistence

Use existing save formats with tested migrations and backups. Separate rollback history, network recovery checkpoints, durable snapshots, and disposable caches. Save a coherent simulation frontier and bounded journal suffix. Deduplication records survive reconnect/compaction long enough to prevent old accepted actions becoming new purchases. A transport ACK, Applied receipt, and Durable save acknowledgment are different states.

Keep old generator versions available where required by existing saves. New generated geography applies through an explicit world/version policy, never silent replacement of a visited valley. A render upgrade does not mandate a world-generation upgrade. A corrupted mesh can rebuild; a corrupted canonical save requires recovery or an honest error, not fabricated stock.

## D06. Geosphere: topology before noise

### Generation sequence

Use a bounded strategic atlas with stable boundary conditions:

`body constraints → structural provinces/rock bodies → regional relief and catchments → constrained erosion/deposition + subsurface hydrology → sparse 3D formations → soils/microclimate → ecological communities → settlement/route history → site construction/reuse → local editable materialization`.

This is not an unbounded recursive simulation. Erosion/drainage feedback uses a chosen finite preprocessing algorithm or convergence budget. Macro facts are cached/versioned. Local refinement cannot erase a river, change an outlet, move a ruin, or break a road. Derive independent seeded streams from stable keys and purpose tags; adding cosmetic flowers must not move a deposit.

A heightfield is acceptable for broad relief, catchment calculations, or an appropriate distant envelope. It is not the complete solid/air model. Store layered/deformed rock bodies, faults, intrusive/extrusive features, fracture summaries, and sparse cave/arch/overhang descriptors. Compose persistent edits over those sources; arbitrary vertical solid-air intervals must be possible.

### Geology that affects decisions

Begin with a few visibly and mechanically different materials: sandstone, limestone, shale, basalt, granite, and selected metamorphic products. Distinguish mineral composition from rock fabric and moisture/alteration. Use properties only where a supported process consumes them: fracture direction, strength class, erodibility, hydraulic conductivity, thermal response, processing output.

Basalt does not always mean a field of identical hexagonal columns; columnar jointing is one cooling/fabric outcome. Rock bodies can include irregular joints, talus, weathered surfaces, and younger soil. Limestone does not guarantee a cavern in every chunk; caves depend on soluble rock, recharge, fractures, and discharge. Arches need plausible openings and remaining support, not random torus subtraction. Granite, layered sedimentary regions, older rounded terrain, folded ranges, volcanic structures, glacial valleys, and exceptional magical forms need distinct silhouette grammars.

Start with macro landform mass, then connected ridges and valleys, then cliffs/outcrops/gullies, then surface detail. Avoid adding octave after octave of noise to the same peak operator. Rare needles are permitted; a horizon composed entirely of needles fails the ordinary landscape brief.

### Hydrology and erosion

Resolve catchments/outlets above chunk scale. Use a regional flow graph, reservoirs and storage, bounded groundwater control volumes/conductance, and local flow where interaction matters. Springs arise from head, connectivity, and exposure, not just a permeability-colored mask. Floodplains, endorheic basins, waterfalls, and intermittent streams have declared conditions; not all rivers must reach the ocean.

Historical erosion shapes the base world. Runtime rain, diversion, and quarrying can transport supported local sediment and alter a regional boundary update, but do not rerun ancient tectonics. Fine simulation exchanges equal signed flux with coarse reservoirs; old and new boundary models switch at a coherent authority tick. A dam affects upstream storage and downstream discharge; an unmodeled neighbor is neither an infinite drain nor a magic wall.

Compress travel distances and select landform scales intentionally, not every physical meter indiscriminately. Keep local gravity, speed, flow, and construction units coherent. A small world may use declared fictional planetary parameters; test the same parameters in flight and orbit rather than switching incompatible physics at the atmosphere boundary.

### Ground classification required by the latest screenshot

Flat gray patches cannot stand for five different surfaces. Classify exposed bedrock, gravel/sediment, bare soil/litter, maintained route, and construction disturbance explicitly. Generate different edge shapes, thickness/relief, habitat response, and materials. A road connects use; rock emerges from a geobody; debris accumulates downhill or through history. Improve the physical relationships before sprinkling more grass over the masks.

**Proof:** same regional relief with altered lithology/permeability yields measurably different valid caves, springs, soil/habitat outcomes, and excavation opportunities. Counterfactual runs are development fixtures, not post-load rerolls of player worlds. Generation order/worker count tests must preserve output. Uncurated seed review checks geography, access, massing, and meaningful differences.

## D07. Plants, wildlife, and ecological accounting

### Plant architecture

Use authored species masters plus constrained variation. A fern has a rhizome/base, arching rachis, and meaningful pinnae grouping; a shrub branches near ground; a mature tree has trunk taper, major limbs, crown lobes and negative space. A species is not a random collection of cuboids or a recolored copy of another. Cubic stylization is welcome when the overall form is intentional.

Near plants use organ/branch representation and appropriate geometry. Middle distance groups fronds/crowns. Far plants preserve whole-plant silhouette, coverage, and average shading. Dropping tiny leaf triangles without a coverage-aware replacement must not make a dense stand transparently disappear. Alpha cards, opaque geometry, and impostors are candidates to compare; neither cards nor microgeometry are universally forbidden or universally optimal.

Persistent interaction needs a clear resolution. Critical trunks, roots, branches, and harvestable organs have authoritative state. Decorative leaflet bending may be visual. If fine cutting is promised, materialize a supported local occupancy/organ patch and debit its biomass, then persist the wound. Do not pretend to simulate every visible leaf as a full world voxel. Show the player's effective cut/harvest target honestly.

### Ecology and stable individuals

Regional patches track a minimal useful set: species/cohorts, seed/recruitment budgets, biomass/fuel, water/substrate availability, disturbance and next scheduled threshold. Named, modified, moved, interacted, or otherwise important specimens retain stable identity. Do not reseed every visible tree from a changing cohort index; growth must not shuffle a familiar forest.

Promoting a plant/animal into detailed simulation transfers its account out of the coarse representation, or marks one reconciled owner; it never duplicates both. Demotion records final damage, consumed resources, offspring, and position/range appropriately. Stable recruitment IDs are allocated once per recruitment event. Leave persistent tombstones or compressed disturbance records for cut specimens until regrowth rules justify replacement.

Biomass growth is an open-system process: declared water, nutrients/carbon sources, energy, and game-scale growth rates. Do not insist on closed mass conservation while silently creating wood; do not require an atomistic carbon simulation either. Log modeled environmental inputs and explicit abstractions. Growth/season times are tunable world rules, never frame-count dependent.

### Ecological gameplay

First prove cutting/regrowth, shade changes, and moisture response with a small excellent plant set. Then add relevant pollination, dispersal, grazing, predation, symbiosis, cultivation, and mutations. Avoid obligate worldwide population collapse because one player harvested an early resource. Recolonization and refuge populations need modeled sources, not infinite spawns beside the camera.

Humane husbandry has effective outputs: milk/eggs/fiber/molts/secretions/seed/fungal products where appropriate. Space, food, shelter, and stress can matter without making every animal a constant maintenance task. Size augmentation changes fit, anatomy, mass/source, collider, reach, locomotion, and needs—not only model scale.

GPU wind is coherent across a stand: prevailing flow, gusts, branch stiffness, local contact impulses, and reduced distant modes. Visual animation can differ across GPUs; persistent uprooting, cuts, fire, harvest, or damage are authoritative. No per-leaf physics actor. Performance tests measure transparency, shadows and streaming, not just instance count.

## D08. Inhabitants, history, and settlements

Generate present-day settlement opportunity from water, food/trade, resources, viable access, productive activity, safety and institutions. Bound history to meaningful events with causes/outcomes. Current people and inventories are mutable; ancient baseline history does not rerun because a player repairs a bridge.

A living settlement needs a useful service, homes, circulation, supplies and a plausible connection to other places. An NPC's job is not an independent resource generator. Orders, shipments, workers and stores share authoritative lots. A distant caravan and its nearby wagon refine the same consignment; entering/leaving the region cannot deliver it twice.

Do not require full microscopic economics before one baker can work. A bounded aggregate supply/consumption model with real inputs and reasonable buffers is valid. Shortages create legible graduated consequences and substitutions, not automatic extinction. People can have stable schedules, priorities, a few relationships, authored dialogue and informed knowledge without an always-running language model.

Civilization diversity comes from different practical decisions and histories as well as aesthetics. Species, culture, profession, faction, morality and technical sophistication are separate axes. Friendly mythic inhabitants need distinct body/access constraints, not stereotyped roles. Preserve verified existing species and give ordinary people competence.

Cities are planned from terrain and transport to districts, parcels, circulation, buildings, utilities, and reuse—not a grid filled with random facades. Build loops and service access. Reject unusable sliver parcels. A door promises access, a meaningful lock, or visible blockage. Large decorative backdrops must not masquerade as fully accessible interiors.

Settlement growth respects claims and existing builds. It may ask to extend onto player land; it cannot overwrite a house overnight. Player contributions alter real capacity/access/comfort, not only reputation text. Calm communities and new construction belong beside old ruins.

## D09. Technology, industry, and electronics

### Throughput with understandable causes

A machine is a process plus geometry, typed ports, conditions, resources, capacity and feedback. Useful presets work first. Deeper construction changes volume, heating area, insulation, flow limits, input purity or control capability with understandable tradeoffs. Avoid decorating a fixed recipe box with fake exposed mechanisms.

Use compact compiled graphs/runs for stable belts, pipes, electrical networks and machine structures. Recompile touched topology. Do not update one simulated physics object per conveyor item or per cable block. Nevertheless, cutting a pipe, removing support, blocking an output or extracting an item changes the real network.

Every production stage has input/output/byproduct ownership, work/energy, interrupted state, and output-capacity policy. Compare dormant/coarse advancement with the detailed model at thresholds and interruptions. A factory advances for the same simulated time whether the player watches it or explores elsewhere. A closed single-player game does not award offline production unless the world mode explicitly supports an accounted offline policy.

### Power and scientific abstractions

Keep joules distinct from watts; reserve capacity distinct from output rate. Source/return, finite resistance, current/voltage limits and heat matter under the supported electrical model. Ordinary paired-conductor connectors make circuits convenient without an invisible unlimited return path. Use existing suitable numerical solvers; no blanket ban on floating point. Exact replicated domains need specified numerical behavior and full replay proof.

Retain the latest agreed AC simplification: RMS/real-power behavior with phase ignored initially. Do not quietly revive the older demand for full reactive/phase dynamics. Transformers, rectifiers, inverters and DC/DC converters retain distinct roles. A passive DC transformer is not a valid default model.

Conversions have explicit inputs/losses or fictional sources. Not every energy form must convert into every other. Heat pumps and biology are open systems; their useful-output/input ratio cannot be evaluated as if they were closed energy creation. Track the correct source and sink before declaring a perpetual-motion bug.

Known trial machine capacities/footprints from earlier work are compatibility leads, not permission to overwrite the repository. Reject recipes whose only purpose is another manual intermediate with no new process choice. Tailings can be useful or auto-managed; they should not become a compulsory trash-sorting job.

### Custom processors

Preserve the player-designed multilayer chip ambition. Start with a working controller assembled from validated functional macros, typed nets, routing, memory, timers and IO. Compile it to a bounded executable model; characterize real workloads, power/heat, routing/congestion, capacity and manufacturing constraints. A denser grid is not automatically a better processor.

Starter firmware and automatic routing support ordinary users. Expert layout can change throughput, responsiveness, robustness, sensor count or power demand. Save design revisions, manufacture lots, inspect/bin, and install the resulting device into an actual machine. Reload must not reroll yield. Historical discrete electronics and mature nodes retain uses; early useful automation and avionics cannot require an advanced wafer fab.

Deposition, lithography, etch, doping, packaging and clean handling are game abstractions with ordered requirements and visible consequences, not real hazardous lab instructions or literal industrial waiting times. Expand fabrication only after a designed controller changes real gameplay.

## D10. Magic, combat, and meaningful encounters

### Shared consequences, different experiences

A small shared operation vocabulary handles material removal/deposition, heat, force, flow, organism changes, sensory queries and authorized transit. Tools, machines, creatures and spells request these through their own semantics. This does not require a universal solver, a new scripting engine for every subsystem, or a single programming UI for every player.

Basic magic must work immediately. Ordered/shuffle decks, cast delay/recharge, capacity, draw count, resource use, spread, multicast, modifiers, triggers and timers support expert construction. Preview execution and costs; include a safe testing facility using the real world systems. A permanent intrinsic is real behavior, not a decorative glow.

Validate finite effect graphs, recurrence and resource constraints. Reserve a supported operation's immediate authoritative consequences before activation. Do not pretend to precompute every future causal chain. Subsequent induced events are deterministically admitted/staged within the public model; mandatory already-accepted consequences must not vanish. Expensive spells can visibly charge or propagate rather than silently drop damage when the GPU is busy. Numerical limit changes are server rules, not per-client quality sliders.

A mining effect routes its yield once: debris, inventory, processing, or explicit conversion. Summoning creates actual supported entities. Shields differ by impulse, conductivity, containment, or biological repair, not just color. An Earthfold-style spell can eventually manipulate bounded donor panels with real voids and constraints; this is a dependency-heavy advanced task, not the first spell required to prove the game.

### Combat quality

Before a large catalog, prove responsive movement, aim, hit feedback, enemy preparation/recovery, coherent collision, and environmental interactions. Enemy roles must change decisions: pursuer, ranged threat, flier, support, burrower or habitat defender. Functional parts can alter behavior, but arbitrary dismemberment of every voxel is not a prerequisite for every creature.

Bosses need readable attacks, positioning, changing opportunities and supported unconventional solutions. A larger model and health bar are not enough. Avoid input-reading, unexplained immunity, and scripted removal of player construction. Damage to an important organ can affect a shared resource/attack. Capturing, diverting, farming or disabling some bosses is legitimate; not all creatures are renewable bosses.

Co-op scales attack opportunities, simultaneous jobs and recovery options before simply inflating health. Solo has complete counterplay. Persistent wounds, loot, resets and encounter state survive disconnect/reload without duplication. Secure homes and uninvolved players are protected from personal horror mechanics.

## D11. Exploration, dungeons, tone, and exceptional content

Deeper/farther travel changes terrain, environmental conditions, societies, relationships, materials and available tactics. It need not produce a new resource or major secret every few minutes. Preserve quiet, beautiful, familiar regions and unhurried home life.

Dungeons are authored-quality spatial grammars placed in compatible world history. Establish useful rooms, route graphs, vertical relationships, sightlines, shelter, service passages and mechanisms before decoration. A ruin's former use and present use are different layers. Avoid every room becoming a puzzle or every building becoming an indestructible encounter box.

Important obstacles should support more than one meaningfully different feasible approach where appropriate, not a rote rule requiring exactly three solutions to every door. Digging, building, flight, flooding, stealth, bargaining and magic can bypass designed routes. Local wards/materials/pressure/ownership can make some approaches costly; their sources and scope are observable. Bypassing a puzzle can be a reward for mastery. Later content creates new questions rather than disabling old tools everywhere.

Secrets require evidence and recoverability. Common game rules need clear feedback, not wiki-dependent obscurity. A deep secret can use several independent clues, rare conditions, history and experiments without forcing fixed item-pedestal combinations. Destruction may change evidence, but one lost note should not permanently destroy every route to understanding. Neither all lore nor every supernatural phenomenon must resolve to machinery.

Treat old named examples—Fernwake, Orchard, Windward Basin, Hanging Parish and similar—as retained fixtures/content candidates, not requirements that every seed contain them or orders to rename existing geography. Keep exceptional authored set pieces scarce enough to remain exceptional. Evaluate whether varied regions change choices after removing their names and color labels.

The inherited personal-shadow/Unreturned and restricted-magic concepts remain later optional authored content: limited knowledge, observable anchors, real respite, several resolutions, reliable refuge, and consent for cooperative involvement. No fake chat impersonation, omniscient inventory counters, or offline home destruction.

## D12. Traversal, realms, planets, and orbital continuity

### Movement

Early movement includes comfortable walking, stepping/climbing where supported, building/ropes, and a modest hang glider plus an elastic movement option. Preserve momentum with learnable limits. A sling supplies stored work; boots dissipate or explicitly replenish energy; a glider uses relative airflow, lift/drag and gravity. Assistive control is permitted, but it operates the declared model rather than creating hidden unlimited thrust.

The landscape should supply launches, ridgelines, passes, wind shelter, landing areas, water routes and vertical caves. Do not stamp stunt ramps into every region. Gliding can bypass a path; it should also reveal geography. Later powered flight and transport make old journeys convenient. No universal no-fly bubbles to preserve walking challenges.

### Realm canon retained

Material space is distinct from the dark crimson cosmos of Crimson Mirror. Crimson counterparts are isolated planet-associated pockets; ordinary flight and ordinary forged gates do not connect different pockets. The Hall of the Crimson King is the exceptional charted nexus. This is a declared topology rule, not a reachable sky planet with an invisible wall discovered after hours of travel.

Null Choir suppresses sustained magical bindings, not ordinary electricity, chemistry or mechanics. Ordinary fire remains fire after its magical ignition. Underwritten City, Green Continuance, Tideglass, High Reaches, Last Hearth and Manyfold remain distinct later realm concepts; do not register empty versions as finished content. Each needs a different meaningful rule/history/interaction, not another color grade. The common authoritative clock remains common; process-rate variation is not a different network tick.

The Hall's retained eventual progression allows any two of four wardens to enable the King's encounter; remaining systems affect the fight. Resolve this through actual encounter/world state, not four decorative counters. It remains revisitable with disclosed reset conditions, safe return options and replay-safe rewards. This is a later content target, not a prerequisite for today's rendering repair.

### Portals

First prove a static nearby pair with true parallax, clipping, collision and actor/item transfer. Then arbitrary orientation and gravity, moving endpoints, remote bodies, realm policy, passengers and supported assemblies. Rendering recursion, view resolution and streaming have explicit budgets. Each destination view has its own frame, atmosphere, lighting and history; source-world fog must not contaminate the remote room.

Destination readiness includes current collision and authorized state, not just a visible texture. Pending portals are visibly inactive and block crossing. Continuous busy worlds cannot require an eternally unchanged baseline; use a coherent activation frontier. Atomic transfer closes over supported inventory/attachments once. Partial straddling, ropes, large assemblies, closing apertures and opposite-direction crossings have explicit policies. Start by delaying closure until clear and rejecting unsupported crossings before commitment.

Relative velocity mapping includes translating and rotating endpoint point velocities. Moving portals may exchange energy; account for fictional work instead of assuming inertial kinetic energy is always conserved. Fluid/light/electrical continuity is a separate feature. An initial clearly explained containment membrane is acceptable; a rendered waterfall is not proof of actual transferred water.

### Spacecraft and infrastructure

Use real editable assemblies with mass/inertia, cargo, power, propulsion, control surfaces where relevant, pressure compartments, heat and repair. The initial self-built atmospheric shuttle has functional wings; vacuum-only craft do not require decorative ones. Launch cannot replace a block-built hull with an unrelated prefab or carry its attached hangar. Starter blueprints and packaged services make the first design approachable.

Distinguish airtight, pressurizable, supplied and habitable. A hatch does not refill air when closed. Pressure solves actual free-space connectivity at supported shape/voxel boundaries; unknown topology is not a certified seal. Buoyancy, control forces and heat have one owner each. Seated flight precedes walkable multiplayer interiors; passengers cannot receive deck velocity twice.

Planet radius, gravity, rotation, atmosphere and orbital motion use one declared model. Height alone is not orbit. Launch/insertion/rendezvous/entry/landing need usable guidance and recoverable failure. Accelerated time must obey the shared-world policy; one player cannot fast-forward everyone else's factory privately.

Surface elevator, orbital-ring support, skyhook and mass driver are different systems. Meridian architecture needs a recognizable anchorage, terminal, cargo movement and regional footprint; a kilometer-scale tether is analytical/swept geometry, not millions of stacked active cells. Tether support and orbit must fit the planet or use explicit fictional stabilization. Cargo follows one consignment across warehouse, ship, skyhook, pod and recipient. Radio reports real known schedules; missed ordinary appointments recur or have alternatives.

Build one rich homeworld and a useful orbital round trip, then one genuinely distinct second destination before multiplying planets. Changing gravity/atmosphere/thermal/geological conditions should change existing equipment behavior, not only recipe availability. Home remains valuable. No claim of infinite unprecedented content.

## D13. Visual reconstruction of the supplied scene

The current image is a failure reference, not a target to copy. Its broad defects are ambiguous flat gray ground patches, tree-post/cuboid-crown repetition, coarse roof steps, weak building-to-site connections, a shallow near landscape abruptly meeting spiky pale mountains, a thin tower with weak regional context, and an oversized held construction block. Lighting and atmosphere cannot independently solve those relationships. A screenshot alone cannot establish the generator version, debug mode, performance, or cause of a dark pixel.

### Form and site order

Work at landscape mass → terrain relationships → vegetation stands → site circulation → building masses/voids → usable architectural details → materials → lighting/effects. This is an evaluation order, not a demand to rebuild all upstream systems before improving an asset.

For the shown building, preserve the promising warm roof/pale trim/dark wall palette. Clarify a principal entrance, usable route, yard, service space and relationship to the rectangular neighbor. Decide intact/ruined/under-construction from actual source state. Ruin requires coherent damage/debris/water/repair history, not random missing blocks. Refine eaves and roof ridges locally; do not distribute tiny bevels across every surface.

Tree production begins with an excellent trunk-to-branch-to-crown master, not a species count target. Add grouped stands, age variation, clearings, understory and litter that depend on actual habitat/use. Forest-floor plants should change aggregate appearance from an aerial view without forcing every frond to remain visible at kilometers.

Mountains need connected masses, secondary forms and quiet intervals before exceptional spires. Geology/material transitions must remain recognizable between near voxels and far representations. Artistic fantasy remains allowed; gray clay review should reveal composition, not enforce generic realism.

### Art style

Use physically convincing geological form and spatial light with deliberate stylized pixel-material art. Do not switch to photographic texture scans merely because the user asked for photorealistic valleys. Preserve crisp clusters, restrained ramps, meaningful texture direction and original shapes. Lighting may remain continuous; albedo does not need random Bayer grain to count as pixel art.

Use a coherent world texel-density family and justified object-scale exceptions. The historical 20/32 texels-per-meter and 4.5–5.5 mm viewmodel pixel proposals are experiments, not universal laws. Compare at actual FOV, resolution, object size and movement. UV atlases must preserve padding/mips; nearest magnification with appropriate minification is a candidate, not a command to alias every distant pixel.

Define top/side/bottom and material contacts for grass, soil, litter, gravel, rock, masonry, timber, roof, metal, ceramic, crystal and plant matter. Authored base reflectance, roughness, normal detail, emission and weather masks have clear ownership. Climate modifies appropriate materials without turning painted wood into grass or moving biome boundaries with texture scale.

### Asset workflow

Use real available Blender/software, not a statement that it was used. First inspect the current importer and asset schema. A GLB is not a deliverable when the game reads only a custom RON mesh unless a tested converter exists. Retain original source/model or construction script, UV/texture sources, material references, scale, sockets/pivots, collision/interaction representation, LODs and native captures. A Blender render is offline evidence only.

Prefer editable deterministic assembly scripts for repeated mechanical pieces and controlled plant geometry, then artistic revision. Do not interpret a script-generated cuboid as a finished model. No requirement for hundreds of variants before a few strong masters exist. Reusable kits need corners, ends, joins and construction affordances, not merely straight modules.

User reference art is reference, not automatically licensed production material. Maintain a license/source record for shipped code, textures, fonts and audio. Do not extract Mycopunk proprietary shaders or claim access to them; make original effects from licensed/public techniques. Shields and scientific/magical effects should reveal state, sources and interactions, not fill the screen with a universal glow cube.

### Native acceptance

Inspect a fixed aerial scene and walking-height close/mid/far routes, reverse approaches, interiors and contact details under neutral daylight, sunset, overcast and night. Use a small fixed seed set initially and a larger uncurated set after the mechanism stabilizes. Exact seed counts are QA sampling choices, not proof every possible world is beautiful.

Keep a neutral studio-clay mode with fixed exposure/background and known lighting separate from live shadow-visibility mode. Debug outputs should bypass unwanted tone mapping/fog where their numeric meaning requires it. Label the active mode in the capture receipt rather than infer it from a screenshot.

For architectural captures hide HUD/viewmodel, then verify normal gameplay with them restored. The held block remains a legitimate block; improve its pose/scale/screen coverage rather than add ornament to a cube. Wands need distinctive silhouettes, proper grip and casting origin, and purposeful idle/move/cast/sustain/failure motion. Inventory, held, dropped and other-player views depict the same object, not necessarily the same mesh.

UI is readable, compact and accessible. Avoid ornamental gold dashboards, tiny type and empty feature buttons. Keep important interaction visible at a range of screen sizes. Audio, quiet intervals, material impacts and truthful machine/creature cues are part of atmosphere. Do not use continuous music or mystery sounds to make empty terrain seem meaningful.

## D14. Rendering, LOD, shadows, and atmosphere

### Audit before algorithm replacement

Inspect the pinned WGPU/Naga versions, actual Rust layouts, shader preprocessing, feature checks, render graph, depth convention, color formats, material registry, mesh paths and validation tools. Historical source paths are search leads. Do not apply old WGSL files wholesale to a changed ABI.

The supplied older megashader packed unrelated semantics into floats and family IDs. In that historical source, family 46 appears both in a fired-clay surface branch and a later animated emissive branch. Check whether this conflict remains in current code; it is not a certified current bug. Build material/tag round-trip fixtures. Gradually replace conflicting metadata with explicit per-draw/material flags only through a tested host/shader change; no unreviewed ABI replacement.

A shader branch uses physical material, geometry kind, render ownership, emitter state and camera-relative status for different purposes. Do not use one numeric range for all of them. Validate indexing, flat interpolation of IDs, uniform layouts, texture/color-space interpretation and derivative legality with the repository compiler. Never silence uniformity diagnostics to force an unvalidated shader through.

### Geometry coverage

A render region has one active representation frontier, potentially composed of compatible children and explicit empty results. Request, build, upload and validate replacement/transition resources before retiring the old valid coverage. Bound replacement overlap memory. Coarse/fine edges and corners need shared ownership and a proven stitching rule.

Heightfield clipmap morphing works only for that heightfield domain. Transvoxel is a candidate for compatible sampled-isosurface resolution transitions, not a drop-in fix for arbitrary cubic meshes, tree impostors or mixed shape blocks. Use the appropriate face-coverage/stitch algorithm for discrete voxels. Do not adopt two terrain surface models casually; retain cubic construction and prove every natural-surface alternative fits edits/collision/LOD before expansion.

A two-dimensional XZ mask cannot certify ownership of every surface in a column containing a bridge, cave and lower tunnel. Use vertical intervals or appropriate sparse 3D exception ownership where required. Unknown terrain is not a license to draw higher/thicker mountains. Small far features may fall below a declared visual error threshold, but approach must refine coherently and gameplay queries remain canonical.

Depth bias can order nearly identical coplanar surfaces. It cannot create missing geometry or stitch incompatible silhouettes. Remove crack-hiding sink/discard policies only after their replacements provide valid coverage. Skirts, if used for a strictly appropriate opaque boundary, must not seal actual tunnels, introduce fake collision, or cast false walls of shadow. Keep unrelated existing functioning fallback paths until replacement evidence exists.

### Culling and thin features

Use bounding volumes, frustum culling, candidate batching and optional Hi-Z where it wins. Previous-frame occlusion is invalid after relevant edits, teleports and moving cover. Use conservative visibility when uncertain. Match min/max depth reduction and comparisons to reverse-Z and handle non-power-of-two dimensions and near-plane intersections. Compare against culling disabled.

Thin tethers/cables require continuous projected geometry and coverage-aware antialiasing. Subpixel raster support may be wider than physical geometry, but its contribution must reflect physical coverage. Do not blend it against sky as an opaque surface when terrain or another object is actually behind it. Transparent/coverage composition and depth ordering must be tested against both land and sky. A bright dotted line in a still does not certify the temporal cause; test motion, AA and LOD.

### Shadows

Freeze sun/camera/weather and isolate geometric occupancy, CSM visibility, far visibility, their domains, combination and stale validity. A zero direct-sun visibility pixel is not missing terrain and should not alone be judged against final ambient lighting. A terrain hole is demonstrated through occupancy/depth, not by the color black.

Use the existing camera reverse-Z and shadow-map convention consistently. Inspect projection matrices, receiver/caster bias, cascade padding/blending, geometry normals, sampler comparisons and origin frames. Receiver derivatives are valid only under their stated projection assumptions.

Do not mechanically replace `min(CSM, far)` with `max`, product, or a confidence lerp. Visibility models with distinct occluder sets may legitimately disagree; correlated PCF penumbra estimates are not independent probabilities. First identify covered caster domains and errors, then define the combination. Never brighten genuine shadows because a diagnostic is black.

Filtered mean terrain heights do not establish exact ray blockers. Where heightfield shadows are appropriate, define height/error bounds and receiver offset, detect uncertainty, and refine or use an explicit fallback. Arbitrary step skipping or rim probes can miss thin blockers; optimized early exits need conservative evidence or a documented quality approximation with an oracle comparison. Cache distant visibility only after correctness and update/invalidation costs are understood.

### Lighting and atmosphere

Maintain one linear-HDR accounting path and a clearly owned exposure/tonemap resolve. Distinguish albedo, illumination, opacity and emission. Test sRGB decode exactly once. Painted cyan is not light. Fire can be stylized, but exposure compensation cannot silently make it radiometrically incomparable across every pass.

Atmospheric segments use a compatible convention: `out = incoming * T + S`, with `S` already in-scattered radiance for that segment. For ordered near/far segments, compose their transmittance and source terms consistently. Arbitrarily stacking air, homogeneous cloud fog, a second veil, local mist and streaming fades can over-wash the scene. Confirm which pass owns each effect and avoid double cloud extinction. Artistic grading is allowed when identified, not misrepresented as physical transport.

Improve local lights only after separating surface/reflection/fog costs. Up to dozens of per-fragment lights may justify tiled/clustered lists, but the existing scene and adapter decide. Light-list overflow needs a deterministic importance/fallback policy and statistics, not flickering silent omission. Portal views require their own relevant list/visibility context. Bake or cache stable material patterns where appropriate rather than run every noise field for every pixel.

Use an established raster path as baseline. Indirect-count drawing, timestamp features and other advanced paths are adapter/version gated; no engine-wide dependency on experimental ray tracing/mesh shaders. A correctly batched fallback remains available. Native timing—not API fashion—decides promotion.

## D15. Physical simulation and bounded complexity

### Appropriate representations

Static terrain is compressed/sleeping material. Structural components and vehicles use object-local volumes and simplified physical aggregates. Grains use local cells/parcels and conserved deposits. Pipes/tanks use control volumes/networks. Free liquids use one selected bounded solver for the interactions promised. Gases/heat use compartments or sparse fields. Cosmetics never also count as collectable material or damage.

A universal solver is not required. A coherent ownership/exchange contract is. Before selecting a method, define the actual demonstrated behavior, valid range, conserved quantities, numerical tolerance, timestep/substeps and cost. Tolerances depend on scale and test duration; do not invent one epsilon for all physics.

### Structures

Connectivity and strength are separate. Use local components, bonds/supports and an intentionally bounded strength/load model. Merge-only union-find does not handle arbitrary deletions by itself. Detachment also requires material transfer, mass/inertia, collider creation, contained modules/inventory and possible pressure changes. Reserve those outputs; enumerate incrementally when necessary. Never detach from unknown unloaded support.

Split pieces inherit parent point velocities and angular motion consistently. Moving center of mass must not add an impulse. Numerous cosmetic fragments may replace unimportant dust, but consequential cover, machinery and recoverable mass retain supported authoritative representations. A physics quality setting cannot privately remove a projectile blocker on one client.

### Liquids and gases

Begin from real gameplay fixtures: a fill/drain, communicating vessels, breached tank, moving obstacle/buoyancy, and a fine/coarse boundary. A falling-cell water model can provide early flows but is not a promise of pressure, incompressibility, or arbitrary submerged-body dynamics. PIC/FLIP/APIC or alternative pressure-based solvers are candidates only when the coupling and target hardware justify them. Do not implement all candidates in production.

At interfaces, compute a transfer once and apply equal debit/credit. If pressure/forces couple two regions strongly, merge the solve or use a validated boundary condition; camera distance does not decouple physics. Coarse/fine flux corrections and simultaneous state publication are required where different resolutions interact. Moving boundary poses share the same substep as contacts. Avoid double buoyancy if resolved pressure already supplies it.

Combustion uses represented fuel/oxidizer, products and heat. Open-air boundary sources are declared. Phase changes conserve modeled material and enthalpy/latent heat. Pressure in a room is not merely a smoke visual, and a low-speed fluid model is not a shock solver. Explosion propagation has its own supported approximation and explicit limits.

### Activity and overload

Simulate by relevance to physical consequences, not only the camera. A flood moving toward a remote settlement, a projectile about to enter a region, or a factory at a thermal threshold can require work offscreen. Stable regions sleep only with a valid continuation model and wake dependencies.

Not every possible world fits finite hardware. Declare a supported workload envelope and fair admission for new expensive actions. Preserve committed consequences; optional graphics degrade first. For authoritative overload, use an explicit common policy such as bounded authored propagation/substeps or clear simulation slowdown/readiness limits—not divergent per-client accuracy or silently lost events. Exact thresholds need measurement. A busy existing contact pile is a workload even when no new bodies spawn.

## D16. Multiplayer and single-player share the same game

Use the existing sound transport with a listen host and optional user/community headless host. No mandatory commercial simulation fleet, consensus protocol or transport rewrite. Relay fallback, where available, has real capacity/cost/failure behavior. Single-player uses the same authority rules without requiring a network service. Host departure initially means a documented save/end/reconnect flow; seamless host migration is a separate later feature.

### Three compatible modes

**Generated baseline:** compatible clients reconstruct immutable geography and initial history from versioned manifests and independently verified digests. A seed alone does not prove cross-platform equality. Authoritative overrides are applied before interaction.

**Exact command-replicated domain:** preferred for a complete, proven compatible simulation island. Host seals accepted commands and external inputs; subscribed clients run relevant local physics. Complete state includes any future-affecting solver cache, terrain/contact state, RNG, controllers and boundary inputs. Start with a simple closed domain. Do not disable periodic reference corrections until cross-machine replay and checkpoint continuation pass.

**Locally simulated state-sync fallback:** relevant client physics plus prediction/reconciliation when exact compatibility, hidden inputs or open coupling prevent command-only reconstruction. Far state is summarized. Do not quietly remove local physics and label transform interpolation equivalent. Do not force all peers to wait for the slowest machine.

### Boundary and prediction contract

Domains are not arbitrary interest spheres. Coupled contacts, ropes, moving cargo and pressure must be included or represented by declared authoritative boundary inputs. Merge/split preparation has a baseline, activation tick, epochs, bounded workload and hysteresis. If proof fails, retain the explicit fallback rather than attempt exact lockstep for all planetary physics.

Inputs have sequence/actor/session identity. Continuous controls may hold briefly under a defined rule; one-shot actions never repeat due to missing packets. Clients distinguish predicted, confirmed and durable progress. Bound rollback/replay by work and memory. Missing accepted-command ranges cannot be skipped just because a later watermark arrived.

Prediction of mining/placement must align visible patch, selection ray and relevant local collision. A receipt cannot remove the speculative patch until canonical data is installed. Rejection removes/replays the dependent branch without overwriting unrelated edits. Provisional loot is not spendable. Complex collapse prediction starts only where the whole dependency set is supported.

### Delivery and security

A shared procedural seed is not a secrecy mechanism: a modified client can inspect generated facts it can reconstruct. Knowledge gating is a player-facing discovery system, not proof against datamining. Keep private player inventories and unrevealed mutable information server-side; choose explicit server-only generation/streaming only where stronger secrecy is actually required.


Separate fresh control, reliable outcomes/ownership, and bounded bulk baselines with fair service. A delta refers to an installed retained baseline, not merely one sent. Join/recovery captures a coherent snapshot plus suffix retention before compaction races it. Restore a manageable region/domain, not the entire planet. Abandon stale transfer epochs safely. No old snapshot may move state backward.

Interest management protects private containers, unrevealed player bases and permissions. Exact simulation cannot derive inputs intentionally hidden from it; use appropriate authorized boundaries or fallback. Host remains privileged; checksums are not anti-cheat. Cooperative defaults and competitive lag-compensation policy are distinct. Changed-cover PvP needs explicit accepted edit order and a bounded adjudication policy, not naive rewind of inventories or time travel through newly placed walls.

Test two physical machines before efficiency claims: shared crate, competing last-item withdrawal, edit/collision, reconnect, missed input, and checkpoint continuation. Expand to four roles with exploration, factory operation, combat/building and joining concurrently under latency/loss/jitter/reordering. Count baselines, repairs, headers and retransmissions, not just happy-path command payload. State age and correction cost matter as much as ping.

## D17. Performance architecture and measurement

### Budgets follow work, not marketing targets

Keep render/input/audio/network responsive while bounded authority and derived work advance. Use data-oriented hot arrays, sparse indices, stable generational handles, pooled scratch, owned mutation and immutable snapshots where useful. Do not introduce dozens of crates, a service locator or a generic event bus to imitate an architecture diagram. Adopt the existing engine's patterns when sound.

Each work queue has byte, count, estimated-work and age bounds. Bound result installation and disposal as well as worker computation. Coalescing may remove stale meshes, not paid effects. Give required aging jobs progress; prevent continual cancellation from starving the only current collider. Avoid nested thread pools all taking every CPU core.

Resource lifetimes cover reserved → producing → ready → submitted → resident → retiring → reclaimed. Cancellation does not free buffers still referenced by a worker/GPU. Account for old/new overlap, staging, texture mips, render targets, portal views, shadow caches, snapshots and allocator backing. Completion notifications must not block or silently drop. Resize/minimize/device loss/recovery are tested separately from gameplay authority. A WGPU buffer-size limit is not free-VRAM telemetry.

Use available native memory budget telemetry when practical with an explicit conservative fallback, not invented capability. Avoid allocating to the nominal full 24 GB of the development GPU. Lower optional detail/targets/caches before breaking physical correctness. Memory goals are measured working-set envelopes with headroom, not fixed global percentages promoted to laws.

### Benchmark contract

Record exact source/dirty state, build profile, world/save/seed, graphics settings, exposure, CPU/GPU/RAM/backend/driver, resolution, cache state, scene activity, player count and network impairment. Compare repeatable paths and fixed-state scenes. Report p50/p95/p99 frame and tick times, stalls, state age, upload/queue age, resident/staging/retiring bytes, useful versus wasted rebuilds, and total network traffic. Do not add per-pass percentiles to invent a frame percentile; measure actual critical paths.

The prior 120 FPS normal exploration and 60 FPS bounded stress at 1440p on an RTX 4090 are **trial goals**, not present results or promises for unknown CPUs. Define the stress workload explicitly. Compare changes against both correctness and absolute usefulness, not arbitrary percentage speedup demands. No claim of beating Teardown without comparable measured scenes and disclosure of differences.

Optimization begins in every task, but not with speculative replacements. Profile before/after. Keep a correct fallback. Test dense perforated geometry, many material boundaries, transparency, active fluids, contact-heavy piles and edits during streaming—not only a mostly empty flyover. One optimized kernel does not prove an optimized game.

## D18. Delivery gates and editorial acceptance

### Scope tiers

**A — Current playable foundation:** reproducible build/evidence; safe world edits/save; grounded terrain/material transitions; a strong plant/roof kit; comfortable tools/storage; correct lighting/LOD; one useful machine and a few world-interacting spells; one exploration/return loop; early two-client ownership/physics smoke tests.

**B — Systemic region:** meaningful 3D geology/hydrology, persistent plant interactions, a functional settlement/transport leg, contextual guidance, rich encounters, early movement, reliable background factories and proven exact replicated pilot domains.

**C — Expanding world:** wider regional history/ecology, custom controllers/fabrication depth, moving assemblies, static then advanced portals, atmospheric/orbital round trip, real Meridian cargo, additional destinations.

**D — Exceptional content and scale:** fuller realms/Hall/Unreturned, large moving bases, deeper research/modding and broader civilizations/planets. Preserve interfaces and canon early; do not fill the registry with placeholders and count them as content.

These are dependency bands, not calendar estimates or mandates to implement every feature before playtesting. A mature existing feature can pass its evidence gate and remain; no rewrite because the document lists it later.

### Acceptance is plural

**Technical review:** invariants, transitions, races, failure handling, supported numerical model and reproducibility.

**Native visual review:** actual matched captures and motion, including low-quality settings; distinguish user-supplied failure reference, offline asset render and native engine evidence.

**Game-design review:** observed choices and friction with real players; record confusion, unexpected strategies, desire to return, enjoyment of building/movement, and which activities feel compulsory. Small tests reveal problems but do not statistically certify universal fun.

**Editorial review:** region/creature/building identity without names, different kinds of actions and stories, sensible quiet intervals, human competence and practical use. Fewer stronger mechanisms and assets can outperform a long catalog.

No requirement that every session produce three stories, every landmark offer three routes, or every material react with 30% of the catalog. Use those as exploratory questions where helpful, not generation quotas. No mandatory rescue, boredom metric that manufactures disasters, or inventory friction presented as survival depth.

### First action under this prompt

Start with `T00` in `TASKS.md`. Audit enough to reproduce the actual current failure and discover ownership/commands. If a crash, duplication, save corruption, or wrong collision is confirmed, address it first through the appropriate bounded card. Otherwise default to the latest screenshot's **ground/site/vegetation relationship**—choose `T03` for terrain surface identity, or the earliest blocking visual card actually established by evidence. Do not launch all tracks, regenerate the planet, or rewrite the megashader.

Complete the selected card including tests, native observation when available, and revision within its acceptance criteria. If the environment prevents a required native test, mark that evidence blocked, keep safe useful work, and leave exact reproduction steps. Do not mark a feature accepted from self-praise or compilation alone.

## Source and execution map

Source-derived constraints, new trial decisions and reviewed corrections are distinguished in `REVIEW.md` and `reference/SOURCES.md`. The full inherited acceptance text is preserved in `ACCEPTANCE.json`; source retention is not a passing test. Use `START_HERE.md` to execute one selected proof from `TASKS.md`.
