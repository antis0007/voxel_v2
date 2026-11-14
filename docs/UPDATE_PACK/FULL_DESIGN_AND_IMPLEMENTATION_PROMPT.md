# AXIOM — Full reviewed design and implementation prompt

**R1, 29 October 2026.** Reading edition combining the source documents. The package is intended for Codex with the user-selected Astra Light configuration. Read the entry prompt first; load subsystem sections on demand, not this entire book for every edit.

This is a reviewed design and unexecuted game acceptance catalog. It is not engine code or a claim of verified in-game performance. Original section IDs are retained for traceability. The individual files and machine-readable catalog are the authoritative editable sources.


---

<!-- Source: START_HERE.md -->

# Start here — Codex / Astra Light

This package replaces the execution style of the sprawling earlier prompts while retaining their compatible game scope. It is a reviewed design handoff, not a patch to the game. Keep your selected Codex/Astra Light configuration; no undocumented model flags, context sizes, or autonomous-agent capabilities are assumed.

Place the folder somewhere inside the repository, for example `docs/axiom_reviewed/`, or provide Codex its actual accessible path. Do not overwrite existing root instructions. `AGENTS_FRAGMENT.md` is an optional short supplement to merge deliberately, not a replacement for the repository's `AGENTS.md`.

## Paste this into Codex

```text
Work inside the existing Axiom/Meridian Rust + raw-WGPU repository. Use this reviewed package as the product reference. This request authorizes implementation of one bounded, complete improvement, not another rewritten design bible or a whole-engine replacement.

Locate the package containing DESIGN.md, TASKS.md, REVIEW.md and TASK_GRAPH.json. Read the repository's applicable AGENTS.md/override instructions, DESIGN.md D00 and D18, TASKS.md T00, and the current concise tracking/session state. Then read only the source and design sections required by the selected task. Do not load every historical prompt. If a package is unavailable, report its real path problem rather than invent its content.

Preserve these constraints:
- Existing Windows-first Rust/raw WGPU, no Bevy/replacement engine/browser-demo substitute.
- Historical saves/generators, player edits, one authoritative owner per material/item/entity, and active contributors' work.
- Variable physical voxel scale is separate from render LOD, texture density and simulation fidelity.
- Native visual/gameplay evidence is separate from compilation and automated tests.
- Relevant client physics and the existing networking path remain; exact command replication is promoted only where complete-domain replay is proven, with explicit state-sync fallback.
- Early building/storage/traversal must stay convenient; no new compulsory grind, AFK, cruelty, or forced quest sequence.

Begin with T00. Establish the actual checkout, relevant owners, launch/test/capture commands and a repeatable current failure. Previous generator numbers, test totals, source paths and diagnoses are leads, not current facts. Audit only enough to choose and verify the task.

Prioritize confirmed crashes, save/ownership duplication, wrong collision or coverage over cosmetics. If those gates already pass, use reference/FAILURE_REFERENCE.png as a failure reference, not a target to reproduce and default to T03: make one ambiguous ground-surface class physically and visually coherent. T04 vegetation or T05 building/site work is appropriate if verified dependencies or ownership make it the better ready task. Do not simultaneously redesign terrain, trees, shadows, networking and the sky.

Before editing, write a short work card: selected proof, player benefit, actual source paths, ownership/exclusions, one root-cause hypothesis, smallest failing check, acceptance IDs, native reproduction, relevant budget and rollback policy. Pick reversible defaults for minor open details. Ask only when genuinely missing authorization or an irreversible product decision blocks safe work.

Implement the smallest complete change through existing interfaces. Add a focused regression check. Run the repository's applicable validation and fix failures caused by this change. For visual work, launch the actual game where available, capture matched before/after and motion at walking and aerial scales, inspect them, and revise weak results. Writing a Blender script without executing it is not Blender evidence; an offline render is not native acceptance.

For shader work, verify the current Rust/WGSL data layout and pinned toolchain. Do not conceal holes by filling real voids or haze, change shadow combine rules without valid-domain reasoning, disable legitimate shadows, or claim a speedup from instruction counts. For worldgen, use an explicit new-version policy rather than rewriting saved geography. For assets, track all required original sources/exports and use the actual supported importer.

Use one writer per hot path. Never reset/clean/blanket-stage other work, silently alter global permissions, upgrade unrelated dependencies, or loosen tests to obtain a pass. Use isolated disposable fixtures; verify safety before running commands. No commits/pushes or destructive migrations unless the repository/user workflow authorizes them.

Complete and review the selected proof, not merely its first code draft. Run/repeat tests when changes or evidence justify it; do not loop identical full suites without cause. If a required tool/native environment is unavailable, identify the verified limitation, complete safe useful portions, leave exact reproduction steps, and mark the missing evidence BLOCKED or NOT_RUN. Do not fabricate success or expand scope to compensate.

Stop after that proof meets its scoped gates, or when a precise blocker leaves no safe relevant work. Update the existing concise tracker (SESSION_STATE.md only if no equivalent) with exact paths, decisions, command/results, evidence, limitations and the next smallest step. Report IMPLEMENTED, AUTOMATED-TESTED, NATIVE-OBSERVED, MEASURED, and NOT VERIFIED separately. Do not claim that the whole feature or game is complete.
```

## Which file serves which purpose

- `DESIGN.md`: the complete coherent product and subsystem contract; read relevant sections on demand.
- `TASKS.md`: bounded implementation packets with first proofs, exclusions and checks.
- `REVIEW.md`: critical corrections, rationale, sources and remaining unknowns.
- `AGENTS_FRAGMENT.md`: optional small persistent execution policy.
- `SESSION_STATE.md`: an empty handoff template, not a report of current implementation.
- `TASK_GRAPH.json`: dependency graph and acceptance routing.
- `ACCEPTANCE.json`: 122 inherited plus added requirements; initial game statuses are all NOT_RUN.
- `reference/`: provenance and the unchanged inherited-case text; not automatically loaded agent context.
- `tools/validate_spec.py`: checks the package's internal references/DAG/status discipline; it does NOT compile, test, benchmark, or review the actual game.

There is intentionally no requirement to paste the full design into AGENTS.md. Progressive disclosure keeps the active task small without losing the larger design. After completion, reuse the prompt with the next explicit proof recorded in session state; do not re-audit the entire project from scratch unless the relevant baseline changed.

---

<!-- Source: DESIGN.md -->

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

---

<!-- Source: TASKS.md -->

# Execution packets — one active proof at a time

A packet is a small implementation program, not a promise to finish a whole subsystem in one response. Each starts with one specifically bounded proof; later increments are listed separately. Select one proof, fill its source paths/commands from the repository, and finish or honestly block it. Do not expand to its next increment automatically unless the user's requested scope includes that continuation. Dependencies below mean required behavior, not a requirement to rewrite an already working system. No task is pre-certified implemented.

Use `DESIGN.md` only by the section references on the selected packet. Always preserve D00/D05 authority and compatibility. Refer to `ACCEPTANCE.json` for inherited and added requirements. A packet may satisfy only a subset of its mapped requirements; record precisely which. A partial proof never marks the whole packet or its eventual feature complete.

## Work-card template

Before an implementation diff, fill this compact record in the existing tracker or `SESSION_STATE.md`:

```text
Packet / bounded proof:
Player-visible result:
Inspected source paths and ownership:
Existing behavior to preserve:
Actual prerequisites and evidence:
Hypothesis or missing behavior:
Smallest reproduction / failing check:
Proposed change and rejected easy shortcut:
Allowed changes / excluded modules:
Relevant acceptance IDs:
Commands from the repository:
Native reproduction, camera/save/settings:
Performance and memory measurements if affected:
Rollback / migration policy:
Completion / blocked stop condition:
```

Do not require an elaborate system design document for a local bug. If the proof involves a new solver, persistent schema, numerical authority boundary, network protocol, or new renderer data layout, add a short design note covering alternatives, supported range and test oracle before production integration.

## T00 — Reproduce the current game and choose one repair

**Prerequisites:** none. **Read:** D00, D13, D18; root/local agent instructions and the relevant actual source. **Purpose:** establish which defect in the supplied image or current gameplay actually exists and produce a reliable fixture.

Inspect working tree, branch/HEAD, current instructions, active owners, actual launch/build/test scripts, save/generator versions and asset tracking. Do not scan every historical document or map the entire repository when a render fixture is sufficient. The latest provided scene may have changed; reproduce current behavior rather than force obsolete geometry back into the game.

Freeze one save/seed and camera receipt. Capture normal gameplay, clean world without viewmodel, neutral daylight/clay, and the *specific* diagnostics needed for the suspected fault. Record settings, source/dirty state and executable identity. Gather one relevant timing/memory baseline if performance is part of the symptom.

**First proof:** the scene can be reproduced, and one actual cause or constrained hypothesis is identified. If an equivalent harness is missing, implement its smallest useful addition. If a safe bounded defect is found, continue into the appropriate card rather than stop with a giant new plan. Default visual choice after correctness is T03, not a universal shader rewrite.

**Required checks:** reproduced issue vs prior screenshot ambiguity; commands exist; no hidden untracked required asset; user/peer edits preserved. **Do not:** migrate saves, upgrade WGPU, create a new world engine or launch multiple unrelated tracks.

**Stop:** one selected proof implemented/verified, or exact environmental/ownership blocker documented with safe completed work. A reproduction harness is tooling, not the completed visual overhaul.

## T01 — Publish correct terrain coverage and local edits

**Prerequisites:** T00 behavior. **Read:** D05, D14, D17. **First proof:** fix one demonstrated resident/proxy/edit transition that currently renders wrong geometry or collision.

Trace canonical query → mesh inputs → neighbor/seam dependencies → upload → active ownership → retirement. Add a fixture with a surface, cave opening and lower passage sharing XZ space. Delay and reorder one child upload, return explicit empty data, edit during generation, and cross a coarse/fine edge and corner.

Change the smallest actual fault: dependency stamp, owner test, boundary topology, replacement frontier or fast patch. Keep old valid coverage until the replacement and required seam are ready. Distinguish a legitimate hole from missing occupied surface. Preserve critical edit/collision alignment. A view-facing screen-space hack does not repair the source topology.

**Proof:** no new holes or false filled passages in that fixture, stale job cannot resurrect edits, save/reload keeps changes, repeat approach/recede in motion. Test supported voxel sizes actually present. **Measure:** mesh/install/upload cost and overlap memory. **Exclude:** new geology, aesthetic grading and replacement world storage.

**Later increments:** other mesher boundary families, 3D exception coverage, multiple origin frames, wider stress tests. Do not assume one successful edge fixture proves all of them.

## T02 — Isolate shadow and atmospheric errors

**Prerequisites:** T00 behavior; use T01 only where confirmed missing geometry blocks diagnosis. **Read:** D13–D14. **First proof:** classify one suspicious dark/washed region and fix its demonstrated cause.

Preserve existing diagnostics. Add CSM-only, far-visibility-only, combined, validity/domain and disagreement views only where absent. Debug output must have a known color/tonemap path. Freeze sun/camera and test one low-angle and one midday state. Compare with geometry/depth and a controlled thin wall/slope scene.

If the cause is shadow geometry, fix transform, bias, validity or caster representation—not ambient brightness. If it is atmospheric duplication, isolate each layer's transmittance/source contribution and identify which pass already owns it. Changing a combine operator without caster-domain reasoning fails.

**Proof:** known legitimate occluders still shadow; false effect disappears under matched settings; movement/cascade boundary remains stable. **Measure:** relevant passes and scene totals; no unsupported FPS claim. **Exclude:** new terrain generator, full virtual-shadow-map system, arbitrary disabling of real shadows.

**Later increments:** validated cached far visibility, clustered local lights, bounded portal lighting, material-format cleanup. These are separate proofs.

## T03 — Ground surfaces that mean something

**Prerequisites:** T00 behavior. **Read:** D06, D13. **First proof:** replace the current flat green/gray ambiguity for one cause in the existing generator while preserving its broad composition.

Locate where exposed rock, gravel, bare soil, route surfaces and construction clearing are classified. Determine whether gray patches are actual material, slope exposure, clearing masks, or a render fallback. Do not assume from the screenshot. Choose one class—for example exposed bedrock—and make its occupied relief, boundary treatment and material/habitat metadata agree.

Use existing geological/terrain data if valid. If unavailable, introduce a minimal versioned cause field with explicit limits; do not pretend it is tectonics. Preserve drainage and site paths. A bedrock shelf should be a shape, not merely a noisy decal. Nearby habitat should use that substrate classification, but full plant succession is not a prerequisite.

**Proof:** before/after at walking and aerial views; a material-inspection/debug view confirms identity; player mines/builds correctly at contacts; old worlds remain unchanged under the proper generator policy. Inspect several predetermined seeds rather than selecting a flattering one.

**Measure:** generation/mesh cost, changed volume and patch count. **Exclude:** global biome rewrite, wholesale erosion simulation, added landmark catalog and more random grass scatter.

**Later increments:** litter/soil, sediment deposits, maintained paths, construction disturbance. Complete each identity before combining them into a terrain gallery.

## T04 — One excellent fern and one excellent shrub

**Prerequisites:** T00 behavior; substrate hookup can use T03 when present. **Read:** D07, D13–D14. **First proof:** one fern and one shrub look good and have one persistent interaction in the actual world.

Verify real asset tooling/import format. Create editable source geometry and intentional texture/UV/material assets. Fern: coherent curved fronds, organized leaflets and an anchored base. Shrub: branching near the ground, crown masses and negative space. Review silhouette before fine surface work. Native near/mid/far representations preserve coverage and recognizable morphology.

Add one harvest/cut interaction using existing material/inventory authority. Persist changed organ state; reload cannot restore harvested stock. Implement coherent bounded wind and a transient push response separately from authoritative damage. No full ecosystem rewrite is needed for this proof.

**Proof:** neutral/sunset/cave light; native first-person walk-through; one valid cut/harvest and reload; LOD transition in motion; no per-leaf ECS body. **Measure:** draw/material batches, triangles/overdraw/shadow cost and instance memory against current equivalent coverage. **Exclude:** species-count targets, a forest-wide biology solver, copied reference textures.

**Later increments:** grouped forest stands, tree branching, habitat composition, succession via T08. A documented placeholder is not an accepted master asset.

## T05 — An attractive building that occupies a site

**Prerequisites:** T00 behavior. **Read:** D08, D13. **First proof:** improve the existing shown building and its immediate usable grounds, without replacing the settlement generator.

Preserve its promising warm roof/pale trim/dark wall language. Identify actual entrance and intact/ruin state. Refine eaves/ridge/corner caps at supported local detail. Establish one usable approach, a working/inhabited yard and the relationship to one neighbor. Do not attach decorative stairways to inaccessible doors. Check foundations, door widths and real circulation before props.

Expose new compatible parts in ordinary construction/creative interfaces where supported. Use genuine supported voxel or mesh interaction semantics; finer decorative geometry must not claim physical cuttability it lacks. Preserve old generator layouts and player changes.

**Proof:** front/back/interior/roof/arrival views plus walking route and player reproduction of a useful module. Source materials, collisions and placement agree. **Measure:** generation and render delta. **Exclude:** new city-history engine, distant elevator rebuild, hundreds of modules.

**Later increments:** repair/damage grammar, site utilities, additional construction kit junctions, settlement-scale plans.

## T06 — Remove one daily usability bottleneck

**Prerequisites:** T00 behavior. **Read:** D02–D05. **First proof:** one real inventory/building interaction becomes reliable and easier, selected from the observed game.

Prefer quick-stack/sort with locked slots or the local storage scanner foundation if missing. Map existing item/lot ownership; aggregate recipe demands; respect permissions and capacity; define deterministic ordering. Do not create a second inventory backend. Provide clear UI for inaccessible/full containers. Preserve volume when mixed-scale material is stacked, mined and placed.

**Proof:** actual build/repair/craft flow; concurrent last-item test; duplicate request; container move/unload; save/reload. Relevant second-client smoke test early. **Measure:** interaction latency and index-update cost; do not scan every world container each frame.

**Exclude:** full factory automation, a replacement UI framework, universal remote logistics. **Later increments:** incremental nearby index, search/favorites, blueprint preview, bulk stacks and migration, tool selection polish.

## T07 — A connected geological watershed

**Prerequisites:** T01, T03 behaviors, or equivalent proven existing contracts. **Read:** D05–D06. **First proof:** one cross-region catchment and one actual volumetric opening remain coherent across load order and editing.

Use current macro atlas where available. Identify stable outlets, strata/fault context and bounded regional generation. Choose one formation family such as fracture-controlled karst, not all geomorphic models. Produce its 3D solid/air and material query from the shared source. Keep the surface river and underground connection compatible; avoid later noise changing outlets.

Use two development counterfactuals with the same coarse relief but changed lithology/permeability. Show a meaningful difference in water/material/habitat opportunities. Do not regenerate a saved production world during the comparison.

**Proof:** region boundary continuity, shuffled generation schedules, negative/face-seam coordinates as supported, mine/collision/shadow correctness and save persistence. **Measure:** cold/warm generation and query cost, cached atlas memory. **Exclude:** whole-planet geological simulation, new planet catalog, a new weather engine.

**Later increments:** additional landform families, bounded erosion/deposition, aquifers, spherical seams, seasonal processes. A textbook-looking noise field is not enough.

## T08 — Plant persistence across coarse and fine simulation

**Prerequisites:** T04 and T07 behavior. **Read:** D05, D07. **First proof:** one harvested patch changes predictably under moisture/time and never duplicates biomass on streaming.

Use stable specimen/recruitment IDs, one budget owner and explicit materialization state. Preserve modified specimens while coarse cohorts advance. Model the minimal sources relevant to the chosen plant; environmental inputs are explicit. Do not reseed visible trees when an age cohort changes.

**Proof:** cut, leave, advance time, return, save/reload, join client; stable plants/positions and accounted growth. Change available water in a control fixture and observe appropriate differences. **Measure:** idle/active patch cost and persisted delta size. **Exclude:** whole food web, individual AI for every plant, unbounded mutations.

## T09 — One useful unattended production line

**Prerequisites:** T06 behavior. **Read:** D04, D09, D15. **First proof:** existing source/power → process → storage produces useful goods with a legible bottleneck and correct offscreen continuation.

Choose one already supported process. Avoid using a new fluid solver to deliver the first crusher. Consume real input/work/power; reserve output or preserve a defined partial batch. Handle blocked storage, power cut, source depletion, removed transport and simultaneous collection. Expose requested versus delivered energy and actual rate.

**Proof:** compare watched and unwatched runs for equal simulated time; checkpoint mid-batch; broken input line cannot keep producing; no duplicate lots. **Measure:** idle, steady active and burst-edit cost. **Exclude:** entire industrial catalog or new global currency.

**Later increments:** multi-route refining, thermal utilities, network compilation, larger multiblocks, controller integration. Preserve source values until measured tuning justifies changes.

## T10 — One composable spell changes real matter

**Prerequisites:** T01 and T06 behaviors. **Read:** D10, D15. **First proof:** an existing or minimal wand supports two reusable operators whose combination changes a real interaction, not only visual damage.

Choose a simple supported effect pair such as a collision-triggered heat/impulse operation or a controlled excavation operation plus yield routing. Preserve core wand order/resource semantics. Validate bounded expansion and read sets; no universal new VM needed. Improve the actual first-person effect origin/readability if within ownership.

**Proof:** independent effects and combined effect, resource depletion, retry/replay, material change, save/reload and multiscale yield. Cosmetic particles cannot grant resources. **Measure:** spell evaluation and induced edit costs. **Exclude:** all wand families, a full spell-programming language, advanced folding/portal physics.

**Later increments:** data-driven families, continuous/failure states, organism growth, deeper modifiers, automation via same effects, Earthfold after constrained structures are proven.

## T11 — An enjoyable glider or elastic movement tool

**Prerequisites:** T01 behavior. **Read:** D02, D12, D15. **First proof:** choose one tool, not all mobility, and make ordinary traversal feel good in the actual landscape.

Use a small physical model with clear relative-airflow or stored-energy accounting, collision, readable deployment/charge, landing and recovery. Give effective input assistance without hidden limitless forces. Determine safe prefetch by supported speed and collision-ready latency; do not let a glider outrun ground availability.

**Proof:** launch, maneuver, recover, fail safely, repeat at different terrain/wind conditions, save relevant state and compare render-rate variations. Network prediction smoke test when relevant. **Measure:** collision/prefetch and movement latency. **Exclude:** full aircraft design, CFD, arbitrary no-fly zones.

## T12 — One useful inhabitant and one trustworthy lead

**Prerequisites:** T00 and ordinary T06 ownership/storage behavior; T07 only when the selected resident or lead depends on a newly generated hydrological system. **Read:** D03, D08, D11. **First proof:** a resident performs an existing productive/service role and reports one fact they can actually know.

Use actual supplies/order state and a simple routine. A source-linked observation/report enters the player's notebook only when learned. The player can ignore it or inspect/act through more than one feasible method; no forced quest. Prefer a practical existing mechanism over inventing a named landmark.

**Proof:** service or trade, offscreen/near transition, stock accounting, report provenance, optional help and accessibility; a change in route or supply produces the declared effect. **Measure:** active vs remote behavior. **Exclude:** fully simulated economy, generated biographies, continuous LLM dialogue, NPCs requiring the player to rescue them.

## T13 — Responsive cooperative authority and exact-domain pilot

**Prerequisites:** T01 and T06 behaviors; existing netcode may already meet part of this. **Read:** D05, D16–D17. **First proof:** two players mine/place, contend for a crate, physically interact with one object, and reconnect without duplication or global stalls.

Audit actual actor-slot assignment, edge/held input, transport lanes, receipts and baseline installation. Retain a working locally simulated state-sync fallback. Isolate one closed deterministic pilot; include future-affecting solver state and external inputs. Do not claim exact replication from matching transforms alone.

**Proof:** two machines where available, worker/render-rate variations, lost/reordered/duplicate messages, stale baseline, last-item contention, paused client, resuming checkpoint. If only localhost is available, label cross-machine proof pending. **Measure:** full bytes, state age, reconciliation/replay and correction frequency.

**Later increments:** proven command-only steady operation, domain merge/split, four roles, relay tests, changed-cover combat. No anti-cheat or free-relay promises.

## T14 — Coupled material physics with an honest envelope

**Prerequisites:** T01 behavior; T09 only when the selected fixture needs a new working supply/processing line. **Read:** D05, D15, D17. **First proof:** choose one incremental missing behavior in a tank/breach/crate or support/detachment fixture; do not implement new fluids and new rigid bodies simultaneously.

Declare the supported solver/model, quantities, boundaries, error tolerance and workload. Use a tiny reference/oracle where feasible. Retain a correct existing backend. Demonstrate one interaction transferring force/material/heat once. Keep cosmetic effects nonauthoritative.

**Proof:** same fixture across resolution/activity boundary, interruption/save and authoritative replays where supported. Dense contacts and fragmentation count toward cost. **Measure:** end-to-end coupled cost including collision/update/meshing, not just a compute dispatch.

**Later increments:** pressure/room coupling, structural strength, phase changes, finite joints, moving assemblies. Stop before extending outside the measured model; expose unsupported operation before commitment.

## T15 — A player-designed controller earns its existence

**Prerequisites:** T09 behavior. **Read:** D09. **First proof:** one editable macro-level controller design with actual IO operates an existing pump/furnace/sorter differently from a second design.

Use typed nets and validated macro components, a known-good preset, inspection and saveable versions. Compare useful throughput/latency/IO/energy under fixed resources. Manufacturing initially can use the existing compatible assembly path; do not demand a full fab before proving design value.

**Proof:** invalid wiring diagnostics, deterministic bounded execution, save/manufacture/install identity, actual machine response. **Exclude:** transistor emulation and nanosecond scheduling.

**Later increments:** routing/layers/manufacturing yield/wafer processes, not arbitrary grid-occupancy bonuses.

## T16 — A real static portal before a realm network

**Prerequisites:** T13 and T01 behaviors. **Read:** D05, D12, D14, D16. **First proof:** a local static pair with oblique views, correct parallax/clipping and one actor plus an item crossing under authority.

Declare support for partial straddling, collision, direction and closure. An unready exit remains visibly blocked. Per-view frame/depth/atmosphere/history is explicit. Start recursion at one; rendering limit does not redefine legal topology. Retain simple fluid containment policy rather than fake through-flow.

**Proof:** cross/return, camera near-plane, two opposite travelers, duplicate transfer, close while occupied, save/reconnect, permission denial. **Measure:** extra views/memory/streaming/physics. **Later increments:** orientation/gravity, moving endpoints, remote world, vehicles, selected coupled effects.

## T17 — One editable vehicle and an orbital path

**Prerequisites:** T01 and the shared-ownership core of T13; the selected slice also requires its actual rigid-body, pressure, flight, or thermal interfaces. T14/T11 may supply those interfaces but their unrelated expansions are not mandatory. **Read:** D05, D12, D15, D16. **First proof:** choose the earliest missing supported slice: static assembly to moving seated craft, real cabin leak, atmospheric maneuver, or orbit/return. Do not implement all in one patch.

Source hull, modules/cargo/compartments and inventory retain identity. Use one body manifest and model for gravity/air/orbit. A beginner blueprint is convenient but editable. Propulsion/assist has finite actuators/resources; no hidden orbit injection. Log which configuration is a creative fixture vs survival proof.

**Proof:** the selected slice plus relevant interruption/save/co-op case; later full departure/insertion/return. **Measure:** actual coupled physics, visibility and streaming costs. **Later increments:** walkable interiors, docking, station construction, frame changes. Existing example shuttle dimensions are trial art layouts, not certified flight designs.

## T18 — Meridian utility, not another monument

**Prerequisites:** T00 for the visual proof; working custody/storage and endpoints for local cargo; the orbital-return proof from T17 plus applicable T09/T12 supply/schedule behavior for full orbital service. These are separate proof gates. **Read:** D08, D12–D14. **First proof:** one actual consignment travels between two working docks or one current tether renders continuously against land/cloud/sky. Select one of these distinct proofs.

For cargo, preserve custody through coarse and local representations and radio schedule. For rendering, preserve trajectory, actual projected coverage, background composition, precision, atmospheric domain and source ownership. Neither proves the other.

**Proof:** cargo retry/reload/no duplication or native motion/occlusion/LOD/timing. **Exclude:** new kilometer-scale decorative structures. **Later increments:** scheduled orbital exchange, usable terminal, inhabited precinct, skyhook envelope/momentum accounting.

## T19 — A second world changes existing play

**Prerequisites:** T07 regional generation, T17 orbital-return behavior and T18 full orbital service or an equivalently verified return/cargo route. Passing only T18 visual rendering does not satisfy this gate. **Read:** D06–D12. **First proof:** transfer existing equipment/cargo to one destination with a materially different environmental signature and demonstrate a meaningful changed behavior plus return.

Use gravity/air/thermal/geology/ecology differences that interact. No one-exclusive-ore design and no per-planet disabled recipe list masquerading as physics. Region variation and travel safety exist before broad planet multiplication.

**Proof:** planetary mapping/edits, shared material state, compatible atmosphere and orbit, useful arrival activity, solo/co-op and return-home value. **Exclude:** galaxy simulation and ten unfinished planet archetypes.

## T20 — One encounter or dungeon with genuine alternatives

**Prerequisites:** T00 and T01 current collision/edit behavior plus an existing usable combat/movement loop. T10/T11 are conditional only when the chosen encounter requires those new mechanics. **Read:** D10–D11. **First proof:** one compact existing encounter with readable movement/telegraphs and one meaningful environmental alternative.

Improve real AI/action/hit geometry and room topology. Preserve a legitimate bypass; protect only through declared readable properties. Show why the player would revisit or use the place beyond loot. A normal play method and an unconventional method need not share a scripted completion trigger.

**Proof:** solo, cooperative pressure where supported, destruction, retreat/recovery, persistence and low-effects readability. **Exclude:** ten new bosses, required lore terminals or unexplained invulnerability.

## T21 — Realm and Hall content after spatial proof

**Prerequisites:** T16 and T20 behaviors. **Read:** D10–D12. **First proof:** one realm transition demonstrates its actual rule and lawful return; or one complete Hall precinct/warden uses existing machinery. Choose one.

Retain Crimson pocket isolation, Null Choir's revised ordinary-electricity rule, discovered-only routing, and the eventual any-two-of-four Hall progression. Do not register every named realm as a placeholder. Optional personal horror retains bounded knowledge/respite/refuges and uninvolved-player protection.

**Proof:** realm rule actually changes the supported operation; no cargo duplication or transit loophole; alternative exit and reconnect; meaningful encounter behavior. **Later increments:** four wardens/King/extraction/nexus and other realm identities as separately tested content.

## Dependency and selection policy

The machine-readable graph is in `TASK_GRAPH.json`. It deliberately allows T02/T03/T04/T05/T06 to improve current visuals and play after T00 without waiting for a whole geosphere. T13 begins before broad planet work. Performance review is part of every affected card, not a last-stage rewrite.

Correctness blockers override artistic order, but an unrelated failing test owned by another active agent is not permission to rewrite their feature. Record it, establish whether it invalidates this proof, and continue safe scoped work or mark the necessary gate blocked. Aesthetic changes cannot be called accepted without actual native observation. Automated tests should be repeated after relevant changes/failures, not mechanically rerun forever on identical bytes.

## Proof-level dependencies

`TASK_GRAPH.json` separates a packet from the particular proof being accepted. For example, `T18.visual` does not establish cargo transport and cannot unlock `T19.second_world`; `T17.moving_assembly` does not establish an orbital return. A prerequisite can be satisfied by a compatible existing implementation with recorded evidence. Never make an unrelated new glider, hydraulic solver, or full workshop scanner a prerequisite for a local art fix, a basic encounter, or a single-container networking test.

---

<!-- Source: REVIEW.md -->

# Critical review and decisions — R1

**Scope:** Editorial, gameplay, architecture, execution and document-consistency review of the supplied discussion and retrieved design sources. This is not a source-code audit or a completed playtest. The package deliberately retains unresolved hypotheses instead of declaring every problem solved.

## 1. What was actually reviewed

The full 1,494-line `AXIOM_COMPLETE_IMPLEMENTATION_PROMPT.md` consolidated directive was read, including its 122 acceptance clauses. It is the most specific cross-chat baseline retrieved for this task. The entire 355-line V8.4 universe addendum was read. Relevant V8.3 geological/hydrological/ecological/guidance passages, the visible conversation and its uploaded shader text, and the latest supplied screenshot informed the review. Earlier V8–V8.2 ideas were considered through this conversation and their consolidation; this is **not** a claim that every original attachment or every past chat was independently reopened.

The latest screenshot is retained as `reference/FAILURE_REFERENCE.png`. It documents visual symptoms, not the code producing them or the target quality. The actual game checkout, current shaders, executable, performance and native tools were not inspected or run during this design task.

Source-derived requirements and new decisions are distinguished below. `reference/SOURCES.md` identifies technical reference documents checked during the review. The inherited acceptance text is preserved unchanged in `reference/upstream_cases.json` and `ACCEPTANCE.json`; mapping a clause into a later task does not claim it already works.

## 2. The central editorial correction

Earlier drafts accumulated responsibilities faster than they established choices. “A groundbreaking voxel world with all of these influences” is an ambition, not a useful implementation order. A coder asked to be botanist, physicist, animator, writer, renderer author, network architect and critic simultaneously is likely to implement thin approximations of all of them.

The revised identity is narrower than the feature list: **a persistent world that is enjoyable to inhabit, understand and alter**. The product retains industry, spellcraft, ecology, archaeology, celestial travel and unsettling exceptional places. But the execution unit is one useful proof in the existing game. Source fidelity, readable art and basic pleasure in moving/building are not postponed until the simulation is complete.

No new grand location catalog, invented civilization, named cosmic material or new mandatory starting region was added. Old names remain compatible content candidates or existing repository facts to verify. The package is a reviewed production reference, not another fictional universe replacing the previous one.

## 3. Design flaws corrected or explicitly constrained

| Review ID | Weakness in earlier advice or recurring temptation | R1 decision and reason |
|---|---|---|
| R01 | “Nothing can ever be boring or repetitive.” | Reject as an acceptance promise. Familiarity, dependable routines and repeated motifs support mastery and attachment. Test stale decisions and coercive grind instead of maximizing surprise. |
| R02 | Every session must produce three stories; every site three routes. | Use as playtest questions, not quotas. A peaceful building session and a simple useful place are valid. Routes should arise from topology and capabilities. |
| R03 | A spectacular landmark is the remedy for a dull world. | Improve ordinary ground, movement, habitat, construction and social use first. Exceptional sites remain authored exceptions, not uniform scatter. |
| R04 | Every interesting valley conceals a failed ancient machine. | Preserve nature, mundane history, competent present societies, cultural disagreement and genuine unknowns. The Work does not explain away every tradition or spirit. |
| R05 | Serious tone requires permanent decline and misery. | Consequences coexist with comfort, rebuilding, play, thriving communities and understated humor. No requirement that every inhabitant need rescue. |
| R06 | Scientific fidelity requires the player to do technical homework. | Useful presets and instruments reveal cause/effect. Advanced engineering is optional depth; fabrication time and electronic timing are deliberately abstracted. |
| R07 | Exploration must equal a factory's ingots/hour. | Preserve factory throughput. Give exploration new possibilities and reasonable ordinary supplies. No explorer-only currency or arbitrary human-only observation flag. |
| R08 | Machines may be automated, but they stop when the player leaves. | Use validated offscreen production with real inputs, output limits and interruptions. AFK proximity should not outperform doing something enjoyable elsewhere. |
| R09 | Punishment proves survival depth. | Food, exposure and death policies are explicit trials. No default maximum-health death spiral, late-game storage gate, continuous sprint tax or unavoidable loss of long-term tools. |
| R10 | Complete settlement simulation is prerequisite to a believable villager. | Start with an actual service, home, route and supply record. Summarize distant activity. Do not simulate every historical person or require a full market equilibrium. |
| R11 | Humane play should receive virtue points. | Provide practical non-cruel routes and useful living creatures. No universal morality meter or golem slaughter as ordinary metal necessity. |
| R12 | Regional biomass is conserved as a closed scalar. | Plants use explicit environmental inputs and produce outputs under the declared model. Solar energy, water, carbon/nutrients, harvest and respiration cannot be conflated. Promotion itself must not create stock. |
| R13 | More anatomy fields automatically make better creatures. | Author readable roles and bodies; add properties when an interaction consumes them. Avoid random body-part assembly and simulation invisible to the player. |
| R14 | One huge materials matrix with a prescribed reaction percentage ensures depth. | Sparse resolved rules plus a deliberate inert default. Measure useful interactions, not nonzero table entries. |
| R15 | Millions of spell sequences prove Noita-quality depth. | Validate order, resource costs, observable behavior diversity, readable combinations and useful environmental applications. Combinatorial counts are not a fun metric. |
| R16 | One VM should simulate every physical process. | A shared bounded command vocabulary can connect devices and magic. Specialized solvers own fluid, electrical, biological and mechanical state. Separate player interfaces retain personality. |
| R17 | Every challenge must resist mining, flight and powerful spells. | Let mastery solve old obstacles. Particular constraints need readable sources and costs. No universal anti-dig/anti-flight field to protect a scripted route. |
| R18 | More worlds cure repetition. | A useful homeworld and real orbital return precede expansion. Each new destination changes existing decisions and has internal variety, not one mandatory ore. |
| R19 | Complete physical laws require only real sources and no fictional ones. | Explicit magical sources are legitimate. Accounting distinguishes physical supply, numerical losses, game abstraction and intentional magical work. Don't dress an exception as measured science. |
| R20 | Removing external tutorials means hiding every recipe in mysteries. | Common essentials and device diagnostics are accessible in game. Optional rare interactions can reward investigation. Guidance describes known methods, not hidden generator truth. |

These changes are design judgments, not empirical proof of universal player preference. Trial balance must be tested in the actual game and changed when evidence contradicts it.

## 4. Engineering contradictions resolved

| Review ID | Dangerous shortcut | Corrected contract |
|---|---|---|
| R21 | “Every ray at the horizon must hit terrain.” | Rays through real caves, arches and sky must remain open. Coverage means an appropriate representation wherever authoritative geometry should be visible, not filling the background. |
| R22 | Inflate or raise fallback terrain to conceal gaps. | Preserve valid published geometry; repair ownership, topology, seams and revisions. An uncertainty bound is not permission to create material or close a tunnel. |
| R23 | A black shadow diagnostic proves missing terrain. | Separate geometry/depth, CSM, horizon occlusion and final radiance. Zero direct-sun visibility can be legitimate; a single still cannot diagnose its cause. |
| R24 | Replace `min(CSM, horizon)` with `max`, averaging or confidence weights. | Define caster/receiver coverage and uncertainty first. `min` can be valid for combined occlusion; `max` can remove real shadows. No unexplained “confidence-aware” blend fixes incompatible geometry. |
| R25 | Transvoxel is a universal stitching solution. | Its sampled-isosurface 2:1 case is specific. Cubic face ownership, heightfields and other surfaces require compatible boundary construction. Benchmark only applicable methods. |
| R26 | A 2D resident column bit owns all 3D space. | Heightfield domains may use it; overlapping bridge/cave/floating/editable volumes need correct interval/volume ownership. |
| R27 | Fade thin orbital cables to sky color while retaining opaque depth. | Physical subpixel coverage must compose against the actual scene/background; widened support cannot erase land, clouds or objects. Pipeline choice is a measured integration task. |
| R28 | Latest shader complexity equals latest visual quality. | Fix shapes, classification, source data, sampling and lighting domains first. The old shader's material-family collisions are investigation leads, not facts about the current checkout. |
| R29 | Every fern leaflet needs fine canonical voxels or a rigid body. | Stable organ/gameplay state with reusable render geometry, bounded physical proxies and explicit conversion when interaction requires it. |
| R30 | All CPU/GPU state must be bit-identical and float-free. | Prove deterministic generation and selected complete domains under a declared compatibility profile. Keep the supported floating numerical model and explicit state-sync fallback elsewhere. |
| R31 | Deterministic physics means all players wait for the slowest. | Host authority advances accepted ticks. A slow client exhausts bounded prediction then repairs independently. Replicas must receive all required inputs and full relevant state. |
| R32 | Shared seed is secure hidden-world synchronization. | Seed reconstruction saves bandwidth but permits datamining. Player-facing discovery and private mutable data protection are different contracts. |
| R33 | Positions/velocities are a complete physics checkpoint. | Include future-affecting contacts, caches, RNG, controllers, topology, solver parameters, attachments and boundary inputs as required by the backend/domain. Prove continuation. |
| R34 | A large action can reserve all future causal consequences exactly. | Reserve a conservative supported immediate footprint and staged work model. Validate subsequent expansions before activation; don't silently lose paid effects. Truly unlimited aftermath cannot be guaranteed. |
| R35 | Fixed finite queues guarantee stable responsiveness. | Also bound task size, installation/retirement, service fairness and oldest state age. Show recovery under ongoing activity and after bursts. |
| R36 | Rendering fewer objects frees VRAM. | Track actual backing, pools, staging, overlap and retirement. Account for native budgets when available without inventing a portable free-memory API. |
| R37 | A heat pump producing more heat than electrical input violates conservation. | Account for heat extracted from its environment plus electrical work. The closed conversion-loop test must include every source and sink. |
| R38 | Connectivity proves structural strength. | Connectivity and a declared load/strength model are separate. Unknown unloaded support is neither detached nor a certified anchor. |
| R39 | A flow transport algorithm is a complete fluid/pressure model. | Specify behavior and coupling; do not claim pressure/buoyancy/blast fidelity from an unrelated transport kernel or smoke effect. |
| R40 | Fluid boundaries can freeze when offscreen without consequence. | Keep matched flux/control volumes or a validated reduced model. Unloaded neighbors are not infinite drains or automatic walls. |
| R41 | Saving a long journal makes loading safe. | Indexed coherent snapshots plus bounded retained suffix; test compaction, crash boundaries and late network snapshot installation. |
| R42 | A portal's visible destination is ready for crossing. | Collision, ownership, permission, state frontier and required assets must be ready. Pending destinations remain visibly blocked, without pausing unrelated players. |
| R43 | A portal conserves energy merely by rotating velocity. | Moving/separated endpoints can change inertial energy. Account for explicit portal work and point-frame velocity. |
| R44 | A shuttle with wings and a high altitude has reached orbit. | Use one gravity/air/propulsion model, distinguish ballistic altitude from orbit, require an actual useful return. Blueprint dimensions are not proven flight configurations. |

### Two corrections applied to the task graph during this review

An encounter does not need a newly implemented hang glider before its attacks can become enjoyable. A moving vehicle does not need the entire advanced liquid solver before a seated rigid-assembly proof. Those overbroad dependencies were removed. The selected proof still needs real collision, ownership and the specific physical interfaces it uses.

Meridian's thin-tether **visual** proof and its **cargo-service** proof now have separate identifiers. A successful render cannot unlock a “second world with real transport” milestone. The same distinction separates a moving assembly from an orbital-return proof. This prevents partial milestones from silently satisfying full-system gates.

## 5. Codex execution flaws removed

| Review ID | Earlier prompt habit | Replacement |
|---|---|---|
| R45 | Reload all old bibles before every edit. | Short durable instructions, concise session state, a section router and one current work card. Sources remain available without occupying every session. |
| R46 | Give the agent a dozen expert roles and demand the best game ever. | Name the selected player benefit, actual paths, allowed boundaries, evidence and stop condition. No adjective is accepted as an implementation mechanism. |
| R47 | “Never stop until everything is complete.” | Finish/revise the selected proof; stop at its acceptance boundary or an honest blocker. An endless task is not responsible autonomy. |
| R48 | Spend every turn generating a new master plan. | The master is stable; necessary changes are short explicit amendments. No renamed starting valley or repeated architecture rewrite. |
| R49 | Independent critic means the author's own praise. | Record actual adversarial questions and executed checks. A separate reviewer is optional when available; no invented independent review. Human visual/gameplay judgement remains necessary. |
| R50 | Repeated full tests always increase confidence. | Run relevant regression checks and broader suites when warranted. Repeat after meaningful changes or contradictory evidence, not automatically on identical bytes. |
| R51 | Strict lines-of-code/public-API/page quotas equal quality. | Review cohesion, interfaces, code ownership, measured cost and tests. A number can focus scope but cannot certify design. |
| R52 | Documentation can assume Windows/Blender/game access. | Verify tools and actual commands. Preserve executable source and precise blocked status when native evidence is unavailable. |
| R53 | One failing unrelated suite authorizes cleanup of another agent's code. | Classify ownership and whether the failure invalidates the selected proof. Do not overwrite shared work to manufacture a green report. |
| R54 | Choosing a model name implies known limits and special flags. | Respect the user's selected Codex/Astra Light configuration; no invented context size, switches or performance guarantees. |

The package follows official Astra/Codex guidance on progressive disclosure and concise persistent instructions, but the content is a project-specific proposal. It has not been A/B tested against other prompts on the user's actual selected Light configuration.

## 6. Scope and canon preserved rather than silently rewritten

The newest retrieved directive preserves actual local physics on relevant clients and favors command replication for proven complete domains. Older snapshot-only wording is therefore not used to erase that objective. State synchronization remains an explicit safety/compatibility mode, not a claim that exact replication is already achieved.

The approximately 60% self-built atmosphere-exit pacing goal remains an **authoring hypothesis**. In an open capability graph it cannot be a meaningful universal runtime research percentage. Players may buy passage or repair technology earlier where designed. Early gliding is retained.

Crimson planet-associated pocket isolation, the exceptional Hall routing, sustained-magic-only Null Choir behavior, multiple realm families, meaningful ship pressure/flight/docking, working Meridian cargo, and the eventual any-two-of-four Hall challenge are preserved. Their names do not authorize placeholder registries, mandatory early completion, or a rewrite of existing world coordinates.

Unified accessible storage, same-owner inventories/materials, practical presets, and user-designed electronics remain. The latest simplified AC/RMS abstraction with phase ignored takes precedence over the older draft proposing a richer phase/reactive model. A more advanced electrical solver would be a deliberate later decision, not implied by calling a block a transformer.

The declared scientific abstractions do not negate magitech. A fictional energy source or supernatural relationship can be legitimate when its operating rules, costs and consequences are clear. Cultural meaning should not be reduced to an instrument reading merely because some effects are measurable.

## 7. What still cannot be settled by proofreading

**Visual quality:** The forest, ground, roofs, mountains and held-item framing need actual assets and captures. A language model cannot confer “professional quality” on an unrendered mesh. Review silhouettes and materials at walking scale as well as aerial vistas.

**Gameplay balance:** Trial food, recovery, movement, storage range/stack size, resource availability and manufacturing cadence need playtests. They are deliberately not immutable code constants.

**Performance:** Minimum hardware, native working set, supported destruction/contact complexity, region streaming latency and exact-domain cost are unknown until measured. No promise of 120 FPS or superiority over Teardown follows from this document.

**Algorithm choice:** Current storage, meshing, shaders, rigid backend, transport and importer may already solve portions well. The prompt cannot prescribe a replacement before inspecting the repository.

**Determinism:** Cross-machine command replay, checkpoint restoration, boundary merges and hidden inputs must be proven per compatibility profile. Backend marketing or unit tests on an isolated kernel are insufficient.

**Economy and ecology:** Even a conserved simulation can be tedious, unstable or unfair. Remote operation must not become mandatory maintenance; abundance must not erase curiosity; self-sustaining communities must not require the player to babysit them.

**Production scope:** The long-term design is large. Completion of one strong home-to-expedition loop is evidence to expand, not proof that all named planets/realms/encounters are affordable. Stop or cut a feature when its pilot fails its stated player benefit.

## 8. Verification performed by this package

`tools/validate_spec.py` checks document sections, task/proof references, dependency cycles, retention of all 122 source clauses, acceptance status discipline, and local links. `tools/test_validate_spec.py` tests the checker against intentionally invalid small examples. File hashes and ZIP integrity protect the delivered artifact bytes.

These are **document/package checks**. They do not compile Rust, validate the game's WGSL, run Blender, simulate physics, launch the game, measure FPS, prove visual quality, or establish player enjoyment. All game acceptance cases remain `NOT_RUN` in this handoff. A source clause retained verbatim is traceable, not automatically endorsed as already achieved or mathematically sufficient.

---

# Full acceptance catalog — all game checks NOT_RUN

The 122 inherited clauses are retained verbatim; 38 review additions close identified gaps. Routes indicate relevant work packets, not that one small proof finishes all mapped requirements. No case is a completed game test.

## AUD-01 — Reproduce the current native baseline

The actual repository, branch, dirty files, executable, seed/save, dependencies, and source ownership are recorded; prior reported status is not substituted for inspection.

Origin: `inherited_verbatim`. Sections: D00, D13, D18. Tasks: T00. Status: **NOT_RUN**.

## AUD-02 — Clean checkout and asset dependencies

All required owned source and original asset inputs are tracked/reproducible; unrelated work is untouched and no hidden local file is required.

Origin: `inherited_verbatim`. Sections: D00, D13, D18. Tasks: T00. Status: **NOT_RUN**.

## AUD-03 — Existing save migration and rollback safety

Backups and migrations preserve canonical entities, construction, quantities, and versions; an incompatible migration fails safely without silent regeneration.

Origin: `inherited_verbatim`. Sections: D00, D13, D18. Tasks: T00. Status: **NOT_RUN**.

## AUD-04 — Authored asset to native interaction

An original authored item retains editable source, scale, pivots, materials, collider/ports and import provenance, and is inspected while used in the native game.

Origin: `inherited_verbatim`. Sections: D00, D13, D18. Tasks: T00. Status: **NOT_RUN**.

## AUD-05 — Completion report truthfulness

Implemented, unit-tested, native-observed, playtested and performance-validated are reported separately; failed/unavailable checks and placeholders remain visible.

Origin: `inherited_verbatim`. Sections: D00, D13, D18. Tasks: T00. Status: **NOT_RUN**.

## WLD-01 — Purpose-isolated deterministic generation

Same manifest produces the same canonical results across visit orders and worker schedules; adding cosmetic decoration does not move rivers, settlements or deposits.

Origin: `inherited_verbatim`. Sections: D05, D06, D16. Tasks: T01, T07, T13. Status: **NOT_RUN**.

## WLD-02 — Planetary seam and pole traversal

Terrain, gravity, ownership, roads, drainage and saved edits remain continuous at all supported region-face seams and exceptional topology locations.

Origin: `inherited_verbatim`. Sections: D05, D06, D16. Tasks: T01, T07, T13. Status: **NOT_RUN**.

## WLD-03 — Shared map, terrain and orbit facts

All representations use the same body parameters and canonical geography; changing camera scale does not create another mutable world.

Origin: `inherited_verbatim`. Sections: D05, D06, D16. Tasks: T01, T07, T13. Status: **NOT_RUN**.

## WLD-04 — Coherent geology and connected water

The default fixture contains explained lithology, catchments, outlets, strata, springs and actual volumetric openings; refinement preserves macro boundaries.

Origin: `inherited_verbatim`. Sections: D05, D06, D16. Tasks: T01, T07, T13. Status: **NOT_RUN**.

## WLD-05 — High-precision frame round trip

At large system coordinates, frame transforms preserve declared player-scale tolerance, rotation and point velocity without early f32 truncation.

Origin: `inherited_verbatim`. Sections: D05, D06, D16. Tasks: T01, T07, T13. Status: **NOT_RUN**.

## WLD-06 — Operation identity through retries and compaction

Duplicate requests, reconnect, journal compaction and expired receipt detail never repeat a successful cost, reward, transfer or damage event.

Origin: `inherited_verbatim`. Sections: D05, D06, D16. Tasks: T01, T07, T13. Status: **NOT_RUN**.

## WLD-07 — Crash injection at transaction boundaries

Interrupt before/after activation, persistence and receipt; restore one coherent allowed state and correct Applied-versus-Durable semantics.

Origin: `inherited_verbatim`. Sections: D05, D06, D16. Tasks: T01, T07, T13. Status: **NOT_RUN**.

## WLD-08 — Stale derived publication

Old mesh, collision, navigation, light or proxy results cannot overwrite newer relevant source projections; unrelated inventory changes do not force full rebuilds.

Origin: `inherited_verbatim`. Sections: D05, D06, D16. Tasks: T01, T07, T13. Status: **NOT_RUN**.

## WLD-09 — Mutable overlays and changed generators

A world upgrade preserves installed geography, sites, identities, recorded knowledge and player edits; caches can be rebuilt without rewriting canon.

Origin: `inherited_verbatim`. Sections: D05, D06, D16. Tasks: T01, T07, T13. Status: **NOT_RUN**.

## WLD-10 — Indexed resume after long play

Saved entry reads a current indexed state plus bounded suffix, not full history or global erosion; genuine collision-ready first interaction is measured.

Origin: `inherited_verbatim`. Sections: D05, D06, D16. Tasks: T01, T07, T13. Status: **NOT_RUN**.

## REN-01 — Default-preset freeze reproduction

Capture the actual failing timeline and isolate CPU work, GPU work, memory pressure, waits, pipeline compilation or driver causes before claiming a fix.

Origin: `inherited_verbatim`. Sections: D13, D14, D17. Tasks: T01, T02, T03, T04, T05, T18. Status: **NOT_RUN**.

## REN-02 — Whole-job peak admission

Saturate producers and stall GPU completion; descriptors, ready outputs, staging and retirements plateau within declared credits without deadlock.

Origin: `inherited_verbatim`. Sections: D13, D14, D17. Tasks: T01, T02, T03, T04, T05, T18. Status: **NOT_RUN**.

## REN-03 — Cancellation and stale completion

Cancel before/after encoding, supersede a region repeatedly, and reuse handles; no premature reclaim, leaked staging, dropped completion or double release.

Origin: `inherited_verbatim`. Sections: D13, D14, D17. Tasks: T01, T02, T03, T04, T05, T18. Status: **NOT_RUN**.

## REN-04 — Minimize, resize and device epoch

Rapid resize/minimize and simulated device recovery keep authority state intact; transitions are bounded, zero-size surfaces handled, and cleanup makes progress.

Origin: `inherited_verbatim`. Sections: D13, D14, D17. Tasks: T01, T02, T03, T04, T05, T18. Status: **NOT_RUN**.

## REN-05 — Native budget pressure and pool backing

Lower available budget during replacement; optional admission stops, real backing or reuse is demonstrated, and essential correct geometry remains available.

Origin: `inherited_verbatim`. Sections: D13, D14, D17. Tasks: T01, T02, T03, T04, T05, T18. Status: **NOT_RUN**.

## REN-06 — Coarse-fine seams and edited holes

Traverse edge/corner transitions with caves, fine railings, bridges and destroyed landmarks; no sky leaks, filled tunnels, overlap flicker or restored distant tower.

Origin: `inherited_verbatim`. Sections: D13, D14, D17. Tasks: T01, T02, T03, T04, T05, T18. Status: **NOT_RUN**.

## REN-07 — LOD publication frontier

Delay arbitrary child uploads and empty results; the published frontier keeps complete valid coverage, bounded transitions and no unbounded neighbor-refinement cascade.

Origin: `inherited_verbatim`. Sections: D13, D14, D17. Tasks: T01, T02, T03, T04, T05, T18. Status: **NOT_RUN**.

## REN-08 — Occlusion after cover removal

Remove a wall, move occluders and turn rapidly at non-power-of-two resolutions; compare with unculled output and detect no false hidden actor or stale cover.

Origin: `inherited_verbatim`. Sections: D13, D14, D17. Tasks: T01, T02, T03, T04, T05, T18. Status: **NOT_RUN**.

## REN-09 — Shadow and depth correctness

Near/far casters, moving fragments, foliage and interiors use correct depth/bias/material behavior without broad black patches or missing geometry.

Origin: `inherited_verbatim`. Sections: D13, D14, D17. Tasks: T01, T02, T03, T04, T05, T18. Status: **NOT_RUN**.

## REN-10 — Art identity at actual camera scales

Matched native day/night/motion captures show coherent silhouettes, pixel clusters, distinct materials, grouped plants, quiet spaces and accessible original architecture.

Origin: `inherited_verbatim`. Sections: D13, D14, D17. Tasks: T01, T02, T03, T04, T05, T18. Status: **NOT_RUN**.

## REN-11 — Fast travel and turnaround

Move at supported speed toward an uncached site and reverse; collision/readiness deadlines, upload age and memory remain bounded without missing ground.

Origin: `inherited_verbatim`. Sections: D13, D14, D17. Tasks: T01, T02, T03, T04, T05, T18. Status: **NOT_RUN**.

## REN-12 — Persistent surface complexity

Compare solid, hollow, checkerboard, perforated and material-fragmented regions at equal voxel count; measure real output/collision/memory costs and preserve supported geometry.

Origin: `inherited_verbatim`. Sections: D13, D14, D17. Tasks: T01, T02, T03, T04, T05, T18. Status: **NOT_RUN**.

## NET-01 — Cross-machine command replay

On the declared compatibility profile, accepted commands reproduce complete canonical state across machines, worker counts and render rates without routine corrective state traffic.

Origin: `inherited_verbatim`. Sections: D05, D16, D17. Tasks: T13. Status: **NOT_RUN**.

## NET-02 — Complete checkpoint continuation

Restore terrain, solver/cache, controller, RNG, material and attachment state; continued hashes match an uninterrupted run, not merely object poses.

Origin: `inherited_verbatim`. Sections: D05, D16, D17. Tasks: T13. Status: **NOT_RUN**.

## NET-03 — A deliberately stalled client

Host and other peers continue; the stalled peer exhausts bounded prediction then recovers or receives an explicit readiness result, without global waiting.

Origin: `inherited_verbatim`. Sections: D05, D16, D17. Tasks: T13. Status: **NOT_RUN**.

## NET-04 — Missing commands and sealed ticks

Reorder/drop accepted frames and no-event ranges; confirmed time never passes a gap, axes follow bounded hold policy, and one-shot actions never repeat.

Origin: `inherited_verbatim`. Sections: D05, D16, D17. Tasks: T13. Status: **NOT_RUN**.

## NET-05 — Immediate break and dependent movement

A predicted hole, selection ray, local motion and crate contacts use the same branch; authority rejection restores coherent dependencies without overwriting others edits.

Origin: `inherited_verbatim`. Sections: D05, D16, D17. Tasks: T13. Status: **NOT_RUN**.

## NET-06 — Target generation contention

Two players mine/replace one cell; only one gets original loot and a late request cannot destroy a newly placed unrelated machine at the same coordinate.

Origin: `inherited_verbatim`. Sections: D05, D16, D17. Tasks: T13. Status: **NOT_RUN**.

## NET-07 — Receipt before canonical patch

Receipt/data arrival orders never flash a wall back, spend provisional loot, duplicate sound, or discard unresolved dependent actions.

Origin: `inherited_verbatim`. Sections: D05, D16, D17. Tasks: T13. Status: **NOT_RUN**.

## NET-08 — Baseline capture versus compaction

Race snapshot publication and suffix pinning with edits/compaction; every committed event remains represented exactly once.

Origin: `inherited_verbatim`. Sections: D05, D16, D17. Tasks: T13. Status: **NOT_RUN**.

## NET-09 — Late baseline versus newer live state

Old epochs and same-epoch stale candidates cannot regress the installed frontier or overwrite newer geometry.

Origin: `inherited_verbatim`. Sections: D05, D16, D17. Tasks: T13. Status: **NOT_RUN**.

## NET-10 — Bulk transfer saturation

First join, repair and content transfer under limited upload do not starve controls/receipts; queued bytes, age and processing cost are bounded.

Origin: `inherited_verbatim`. Sections: D05, D16, D17. Tasks: T13. Status: **NOT_RUN**.

## NET-11 — Domain merge, split and mode transition

Before physical coupling, install a coherent merged baseline; splitting and fallback preserve contacts, momentum, frames, caches and ownership under tested rules.

Origin: `inherited_verbatim`. Sections: D05, D16, D17. Tasks: T13. Status: **NOT_RUN**.

## NET-12 — Hidden information boundary

Portal interest and domain membership reveal only authorized facts; exact-mode convenience does not expose secret inventories or undiscovered private structures.

Origin: `inherited_verbatim`. Sections: D05, D16, D17. Tasks: T13. Status: **NOT_RUN**.

## NET-13 — Changed-cover combat policy

Test accepted/rejected break-then-shot, replacement blocks, current new cover, moving doors, exhausted history and duplicate fire; authority selects one damage/cost outcome.

Origin: `inherited_verbatim`. Sections: D05, D16, D17. Tasks: T13. Status: **NOT_RUN**.

## NET-14 — Impairment and full traffic accounting

Configured RTT, asymmetry, loss, burst loss, reorder, duplication and relay migration converge within declared limits; report complete traffic, state age, corrections and replay costs.

Origin: `inherited_verbatim`. Sections: D05, D16, D17. Tasks: T13. Status: **NOT_RUN**.

## PHY-01 — One material owner through fracture

Cut a structure and collect fragments; original terrain, debris, salvage and inventory never own the same quantity twice.

Origin: `inherited_verbatim`. Sections: D05, D15, D17. Tasks: T14. Status: **NOT_RUN**.

## PHY-02 — Load versus connectivity

Connected but overloaded construction can fail under the defined model; unknown unloaded support is not automatically detached or a certified anchor.

Origin: `inherited_verbatim`. Sections: D05, D15, D17. Tasks: T14. Status: **NOT_RUN**.

## PHY-03 — Support deletion with cyclic and cross-brick bonds

Repeated cuts and moving attachments yield correct bounded component work, mass/inertia, point velocities, cargo and collider publication.

Origin: `inherited_verbatim`. Sections: D05, D15, D17. Tasks: T14. Status: **NOT_RUN**.

## PHY-04 — Heated tank, powered hinge and floating crate

Breach the tank while inlet and obstacle move; real flow, pressure, heat and two-way impulses operate once within declared numerical tolerances.

Origin: `inherited_verbatim`. Sections: D05, D15, D17. Tasks: T14. Status: **NOT_RUN**.

## PHY-05 — Fluid boundary activation/deactivation

Across active/inactive or adjacent patches, preserve mass/composition/heat/motion as supported; no invisible drain/wall, duplicated flux or camera-dependent disappearance.

Origin: `inherited_verbatim`. Sections: D05, D15, D17. Tasks: T14. Status: **NOT_RUN**.

## PHY-06 — Freeze, cable support and melt

Freeze flowing water around a cable, load/cut it and melt it; latent heat, fluid/solid ownership, tension and collision remain coherent.

Origin: `inherited_verbatim`. Sections: D05, D15, D17. Tasks: T14. Status: **NOT_RUN**.

## PHY-07 — Grains on belt, settling and reload

Transport, settle, save, reopen and excavate material without a body per grain, duplicated quantity or reset provenance.

Origin: `inherited_verbatim`. Sections: D05, D15, D17. Tasks: T14. Status: **NOT_RUN**.

## PHY-08 — Fast projectile and rotating thin cover

Swept contacts use the correct revised geometry for both moving participants; render approximation does not grant tunneling or phantom obstruction.

Origin: `inherited_verbatim`. Sections: D05, D15, D17. Tasks: T14. Status: **NOT_RUN**.

## PHY-09 — Dense contact and wake amplification

Converge existing bodies, alter gravity near sleepers and drive joint loops; common physics stays correct with measured contacts/constraints and no private accuracy reduction.

Origin: `inherited_verbatim`. Sections: D05, D15, D17. Tasks: T14. Status: **NOT_RUN**.

## PHY-10 — Already admitted impact under pressure

After projectile activation, saturate derived queues; reserved effects and costs occur once at the defined stage/tick rather than disappearing or being indefinitely delayed.

Origin: `inherited_verbatim`. Sections: D05, D15, D17. Tasks: T14. Status: **NOT_RUN**.

## KIT-01 — Tool-family functional loop

Actual mine/build/hoe/harvest/bucket/shear/fish/ignite/map/repair affordances use correct tags, feedback and authoritative results, not inventory icons only.

Origin: `inherited_verbatim`. Sections: D02, D04. Tasks: T06. Status: **NOT_RUN**.

## KIT-02 — Mixed-resolution construction

Place, rotate, remove and repair supported fine shapes on large cells; collision/selection/quantity and seam behavior agree.

Origin: `inherited_verbatim`. Sections: D02, D04. Tasks: T06. Status: **NOT_RUN**.

## KIT-03 — Blueprint, mirror and creative distinctions

Ghosts show real fit/resources; survival consumes once, creative sources are labeled, undo respects ownership, and construction does not steal neighbouring structures.

Origin: `inherited_verbatim`. Sections: D02, D04. Tasks: T06. Status: **NOT_RUN**.

## KIT-04 — Stack, sort and consolidate

Repeated sorting/stacking preserves lot compatibility, quality, identity/quantity and locked/favourite slots across saves and concurrent edits.

Origin: `inherited_verbatim`. Sections: D02, D04. Tasks: T06. Status: **NOT_RUN**.

## KIT-05 — Storage scanner permissions and movement

Incremental index follows container moves, locks, range/connections and destruction without full-world scans or access to private containers.

Origin: `inherited_verbatim`. Sections: D02, D04. Tasks: T06. Status: **NOT_RUN**.

## KIT-06 — Concurrent last-item crafting/withdrawal

Exactly one valid transaction acquires the final input; duplicate ingredients/overflow/partial capacity are handled without partial unrecorded consumption.

Origin: `inherited_verbatim`. Sections: D02, D04. Tasks: T06. Status: **NOT_RUN**.

## KIT-07 — Comfort without late-game gates

A fresh survival fixture provides useful storage, shelter, food/light and simple work without compulsory boss kills, slaughter, AFK or advanced-chip fabrication.

Origin: `inherited_verbatim`. Sections: D02, D04. Tasks: T06. Status: **NOT_RUN**.

## IND-01 — Material recovery and process lots

Alternate ore routes change recovery/purity/byproducts while conserving represented feedstock and inputs; reload does not reroll yield.

Origin: `inherited_verbatim`. Sections: D04, D09, D15. Tasks: T09, T15. Status: **NOT_RUN**.

## IND-02 — Circuit and storage limits

Closed DC network, resistance/heating, source/provider/port limits and capacitor behavior agree with the declared model; large capacity does not grant unlimited power.

Origin: `inherited_verbatim`. Sections: D04, D09, D15. Tasks: T09, T15. Status: **NOT_RUN**.

## IND-03 — Conversion and thermal loops

Converters, electrolysis/fuel-cell or equivalent loop, pumps, heat exchange and cooling cannot create free energy through missing losses or erased heat.

Origin: `inherited_verbatim`. Sections: D04, D09, D15. Tasks: T09, T15. Status: **NOT_RUN**.

## IND-04 — Interrupted or blocked production

Stop power, fill output, change a valve and save mid-stage; quantities and partial state remain coherent with useful player-facing diagnoses.

Origin: `inherited_verbatim`. Sections: D04, D09, D15. Tasks: T09, T15. Status: **NOT_RUN**.

## IND-05 — Deterministic wake scheduling

Two networks wake one consumer, reused entities and canceled events; no duplicate activation/transfer and no unbounded stale scheduler storage.

Origin: `inherited_verbatim`. Sections: D04, D09, D15. Tasks: T09, T15. Status: **NOT_RUN**.

## IND-06 — Dormant factory oracle comparison

Coarse/batched advancement matches the defined tick model through thresholds and interruptions; broken logistics and blocked outputs do not produce offline goods.

Origin: `inherited_verbatim`. Sections: D04, D09, D15. Tasks: T09, T15. Status: **NOT_RUN**.

## IND-07 — Functional multilayer IC

Two distinct layouts show different real IO/memory/control/throughput behavior with test-bench diagnostics; design save, manufacture and installation preserve function.

Origin: `inherited_verbatim`. Sections: D04, D09, D15. Tasks: T09, T15. Status: **NOT_RUN**.

## IND-08 — Complete useful product chain

Prospect or salvage, process, fabricate a controller, power/automate a pump and improve a place; statistics expose actual throughput, losses, quality and bottlenecks.

Origin: `inherited_verbatim`. Sections: D04, D09, D15. Tasks: T09, T15. Status: **NOT_RUN**.

## MAG-01 — Wand compiler limits and order

Deck order/shuffle, triggers, cost, delay and modifier composition behave as declared; recursive expansion is bounded and errors are understandable.

Origin: `inherited_verbatim`. Sections: D10, D11. Tasks: T10, T20, T21. Status: **NOT_RUN**.

## MAG-02 — Earthfold physical closure

Donor material becomes constrained panels with actual voids/collision, force limits and occupant handling; payload activation follows real supported closure state.

Origin: `inherited_verbatim`. Sections: D10, D11. Tasks: T10, T20, T21. Status: **NOT_RUN**.

## MAG-03 — Mining spell exclusive yield destination

Hammerhand/additional heads and output modifiers perform paid work; debris/store/transmute/XP paths cannot duplicate the same resource.

Origin: `inherited_verbatim`. Sections: D10, D11. Tasks: T10, T20, T21. Status: **NOT_RUN**.

## MAG-04 — Cultivated creature and size change

An actual organism gains supported behavior, anatomy/collider/mass or explicit magic-source changes and persistent identity, not only a scaled visual.

Origin: `inherited_verbatim`. Sections: D10, D11. Tasks: T10, T20, T21. Status: **NOT_RUN**.

## MAG-05 — Dark spell anchor and alternatives

A restricted technique uses discoverable dependencies and effective counters; consent and legitimate alternatives matter, not just a purple damage type.

Origin: `inherited_verbatim`. Sections: D10, D11. Tasks: T10, T20, T21. Status: **NOT_RUN**.

## MAG-06 — Unreturned bounded knowledge

Only allowed observations/anchors influence behavior; reload and networking preserve limits, respite and ownership without omniscient inventory counters.

Origin: `inherited_verbatim`. Sections: D10, D11. Tasks: T10, T20, T21. Status: **NOT_RUN**.

## MAG-07 — Unreturned resolution and safe home

Supported investigative/resolution path works; wards and uninvolved co-op players remain protected, with no fake chat or offscreen storage deletion.

Origin: `inherited_verbatim`. Sections: D10, D11. Tasks: T10, T20, T21. Status: **NOT_RUN**.

## MAG-08 — First-person equipment quality

Original held wand/tool has useful grip, scale, animation, cast origin and coherent world/icon appearance; native use is reviewed rather than inferred from Blender output.

Origin: `inherited_verbatim`. Sections: D10, D11. Tasks: T10, T20, T21. Status: **NOT_RUN**.

## LIFE-01 — Coarse-to-detailed resident and cargo

Repeated activation of adjacent districts refines one household/journey/consignment, never duplicates stock or overlaps abstract and physical work.

Origin: `inherited_verbatim`. Sections: D07, D08. Tasks: T08, T12, T18. Status: **NOT_RUN**.

## LIFE-02 — Friendly daily life and habitat

Different supported body plans can reach home/work/services; organisms follow habitat and ordinary routines without meal-by-meal micromanagement.

Origin: `inherited_verbatim`. Sections: D07, D08. Tasks: T08, T12, T18. Status: **NOT_RUN**.

## LIFE-03 — Wizard compound alternate access

Public work, restricted rooms, service routes and actual containment exist; research/trade/service/infiltration have distinct consequences and alarms do not instantly fail the site.

Origin: `inherited_verbatim`. Sections: D07, D08. Tasks: T08, T12, T18. Status: **NOT_RUN**.

## LIFE-04 — City growth and player ownership

New construction respects plots, resources, access and player edits; no overnight overwrite or visit-order reroll.

Origin: `inherited_verbatim`. Sections: D07, D08. Tasks: T08, T12, T18. Status: **NOT_RUN**.

## LIFE-05 — Radio and scheduled delivery

Broadcast/map information has a real source/window/uncertainty and authorized destination; missed windows have supported alternatives rather than permanent ordinary lockout.

Origin: `inherited_verbatim`. Sections: D07, D08. Tasks: T08, T12, T18. Status: **NOT_RUN**.

## LIFE-06 — Consignment custody through transport

Warehouse, carrier, coarse travel, pod, dock and recipient transfer one recorded cargo identity; disconnect/reload cannot award it twice.

Origin: `inherited_verbatim`. Sections: D07, D08. Tasks: T08, T12, T18. Status: **NOT_RUN**.

## LIFE-07 — Peaceful settlement viability

Hospitality, trade, restoration, gardens and specialist work create real benefits; prosperity does not mandate endless escalating raids or city-government chores.

Origin: `inherited_verbatim`. Sections: D07, D08. Tasks: T08, T12, T18. Status: **NOT_RUN**.

## EXP-01 — Complete first journey and return

Stream/embankment/gallery loop offers visible invitation, several approaches, evidence, optional action/understanding, quiet discovery and a meaningful return.

Origin: `inherited_verbatim`. Sections: D03, D06, D11, D12. Tasks: T07, T11, T12, T20. Status: **NOT_RUN**.

## EXP-02 — Player-created alternate route

Digging, building, gliding or another supported bypass discovers a coherent site and valid clues rather than broken scripting or blanket immunity.

Origin: `inherited_verbatim`. Sections: D03, D06, D11, D12. Tasks: T07, T11, T12, T20. Status: **NOT_RUN**.

## EXP-03 — Physical puzzle consistency

At least one pressure/gravity/false-wall/fold interaction uses real state and multiple solutions with legible consequences, not hidden matching-key flags.

Origin: `inherited_verbatim`. Sections: D03, D06, D11, D12. Tasks: T07, T11, T12, T20. Status: **NOT_RUN**.

## EXP-04 — Knowledge provenance and reconstruction

Observed/reported/inferred/tested records retain source/time; clues and old maps do not become omniscient exact markers or reroll on visit order.

Origin: `inherited_verbatim`. Sections: D03, D06, D11, D12. Tasks: T07, T11, T12, T20. Status: **NOT_RUN**.

## EXP-05 — Quiet exploration and accessible guidance

A view/person/specimen is worthwhile without obligatory loot/combat; optional guidance supports players who cannot rely on subtle sound or color.

Origin: `inherited_verbatim`. Sections: D03, D06, D11, D12. Tasks: T07, T11, T12, T20. Status: **NOT_RUN**.

## EXP-06 — Boss and enemy readability

Distinct roles, telegraphs, recovery, paths and actual counterplay work alone and in co-op; damage persists and no instant input-reading defeats clever methods.

Origin: `inherited_verbatim`. Sections: D03, D06, D11, D12. Tasks: T07, T11, T12, T20. Status: **NOT_RUN**.

## EXP-07 — Farming and restoration persistence

Supported ecosystem/ritual resets have real costs/population/provenance; repaired routes and player changes remain useful without duplicate experience/material.

Origin: `inherited_verbatim`. Sections: D03, D06, D11, D12. Tasks: T07, T11, T12, T20. Status: **NOT_RUN**.

## POR-01 — Multi-family realm policy

Universe family, realm, body and frame remain separate; direct requests cannot forge discovery/private permissions or restricted cross-family travel.

Origin: `inherited_verbatim`. Sections: D05, D12, D14, D16. Tasks: T16, T21. Status: **NOT_RUN**.

## POR-02 — Crimson pocket isolation

Local travel and valid paired return work; ordinary flight or ordinary forged portals cannot connect different Crimson planet pockets.

Origin: `inherited_verbatim`. Sections: D05, D12, D14, D16. Tasks: T16, T21. Status: **NOT_RUN**.

## POR-03 — Null Choir revised rule

An ordinary electrical machine continues functioning while a sustained magical binding attenuates; material fire and unsupported magical structures follow distinct rules.

Origin: `inherited_verbatim`. Sections: D05, D12, D14, D16. Tasks: T16, T21. Status: **NOT_RUN**.

## POR-04 — Static arbitrary portal orientation

Position, direction, orientation and handedness map correctly for nonparallel apertures and different gravity frames.

Origin: `inherited_verbatim`. Sections: D05, D12, D14, D16. Tasks: T16, T21. Status: **NOT_RUN**.

## POR-05 — Moving endpoints and point velocity

Rotating/translating source and destination preserve intended portal-relative motion, angular state and explicit energy accounting without giant accidental orbital impulses.

Origin: `inherited_verbatim`. Sections: D05, D12, D14, D16. Tasks: T16, T21. Status: **NOT_RUN**.

## POR-06 — Native visible doorway and mid-crossing view

Parallax, near-plane behavior, occlusion and clipped actors work from oblique angles and halfway through, with no black frame, fake screenshot or source fog contamination.

Origin: `inherited_verbatim`. Sections: D05, D12, D14, D16. Tasks: T16, T21. Status: **NOT_RUN**.

## POR-07 — Destination not ready or unauthorized

Pending aperture is visibly blocked; preview sends no private data and traversal waits only for that valid transfer rather than pausing every player.

Origin: `inherited_verbatim`. Sections: D05, D12, D14, D16. Tasks: T16, T21. Status: **NOT_RUN**.

## POR-08 — Opposite crossings, disconnect and replay

Simultaneous travelers and retries at the exact transfer tick produce one entity/cargo owner each and a consistent save/reload result.

Origin: `inherited_verbatim`. Sections: D05, D12, D14, D16. Tasks: T16, T21. Status: **NOT_RUN**.

## POR-09 — Nested portal and coupled-effect limits

Recursion/render-target/interest budgets are bounded; closed-while-occupied and unsupported ropes/fluid/networks follow explicit policies rather than deletion or hidden fake continuity.

Origin: `inherited_verbatim`. Sections: D05, D12, D14, D16. Tasks: T16, T21. Status: **NOT_RUN**.

## POR-10 — Remote edits and players through a portal

Two clients see actual destination movement and current terrain; source/destination view histories and authoritative interest remain coherent during editing and recovery.

Origin: `inherited_verbatim`. Sections: D05, D12, D14, D16. Tasks: T16, T21. Status: **NOT_RUN**.

## SHP-01 — Creative category and editable starter

Spacecraft & Space Building contains functional parts/blueprints; Kestrel can be edited and built from the same survival-compatible kit.

Origin: `inherited_verbatim`. Sections: D05, D12, D15, D16. Tasks: T17. Status: **NOT_RUN**.

## SHP-02 — Assembly boundary and stable identity

Launching/staging/landing never swaps to an unrelated prefab, carries the hangar, duplicates hulls or loses module/cargo ownership.

Origin: `inherited_verbatim`. Sections: D05, D12, D15, D16. Tasks: T17. Status: **NOT_RUN**.

## SHP-03 — Airtightness actual geometry

Thin walls, corners, windows, partial shapes and fine/coarse joints produce correct seal/leak diagnostics; a missing canopy corner cannot be concealed by a seat bubble.

Origin: `inherited_verbatim`. Sections: D05, D12, D15, D16. Tasks: T17. Status: **NOT_RUN**.

## SHP-04 — Compartment split, merge and equalization

Hatches, breaches, docking and moving volumes conserve gas/enthalpy under the model; unknown topology never certifies safety and closing a door does not refill a room.

Origin: `inherited_verbatim`. Sections: D05, D12, D15, D16. Tasks: T17. Status: **NOT_RUN**.

## SHP-05 — Life support and thermal state

Separate pressure/composition, scrubber, power, heat and supplies persist during travel/unload; breach is repairable and a suit enables declared contingency behavior.

Origin: `inherited_verbatim`. Sections: D05, D12, D15, D16. Tasks: T17. Status: **NOT_RUN**.

## SHP-06 — Aero, actuator and load diagnostics

Wings require usable airflow/exposure, actuators and finite power; mass/fuel/cargo changes affect steering; vacuum-only craft need no decorative wing blocks.

Origin: `inherited_verbatim`. Sections: D05, D12, D15, D16. Tasks: T17. Status: **NOT_RUN**.

## SHP-07 — Real propulsion and guidance

Installed engines consume finite actual supplies and guidance uses available control authority; insufficient capability is diagnosed rather than corrected by free forces.

Origin: `inherited_verbatim`. Sections: D05, D12, D15, D16. Tasks: T17. Status: **NOT_RUN**.

## SHP-08 — Altitude versus orbit and round trip

A returning arc and stable orbit are distinguished; launch, insertion, rendezvous, deorbit, entry and landing use one consistent configured model without unlabeled cheats.

Origin: `inherited_verbatim`. Sections: D05, D12, D15, D16. Tasks: T17. Status: **NOT_RUN**.

## SHP-09 — Walking passengers on moving ship

Clients can walk/drop objects during translation, rotation, acceleration and correction with coherent collision time and no double deck velocity or lost passengers.

Origin: `inherited_verbatim`. Sections: D05, D12, D15, D16. Tasks: T17. Status: **NOT_RUN**.

## SHP-10 — Dock, service, disconnect and depart

Compatible capture/permissions/seals/equalization precede opening; both vessel identities and all inventory survive repeat docking and restart.

Origin: `inherited_verbatim`. Sections: D05, D12, D15, D16. Tasks: T17. Status: **NOT_RUN**.

## SHP-11 — Station construction and skyhook fit

A built station needs supported orbital state; tether/elevator/transfer geometry has explicit eligibility, load/clearance and schedule rather than a generic hovering prop.

Origin: `inherited_verbatim`. Sections: D05, D12, D15, D16. Tasks: T17. Status: **NOT_RUN**.

## SHP-12 — First useful orbital return and vehicle portal

A real platform mission returns useful cargo to the same workshop; a supported fitting ship can transfer through a legal portal with crew/compartments intact and no Crimson loophole.

Origin: `inherited_verbatim`. Sections: D05, D12, D15, D16. Tasks: T17. Status: **NOT_RUN**.

## HALL-01 — Original hanging machine-city composition

Approach/interior/reverse/distant native views establish scale, voids, original structures and useful space; no generic castle or copied finale geometry.

Origin: `inherited_verbatim`. Sections: D10, D11, D12. Tasks: T21. Status: **NOT_RUN**.

## HALL-02 — Independent route families

Foundry, vertical machinery/choir and underworks offer distinct routes and systemic choices; damaged paths and player construction remain coherent.

Origin: `inherited_verbatim`. Sections: D10, D11, D12. Tasks: T21. Status: **NOT_RUN**.

## HALL-03 — Warden functional counterplay

Each completed warden has readable combat and at least one real environmental/engineering intervention tied to functioning machinery, not just a switch counter.

Origin: `inherited_verbatim`. Sections: D10, D11, D12. Tasks: T21. Status: **NOT_RUN**.

## HALL-04 — Any two of four progression

All valid orders and co-op joins persist correctly; remaining wardens affect the King through declared systems and no client can forge the shared state.

Origin: `inherited_verbatim`. Sections: D10, D11, D12. Tasks: T21. Status: **NOT_RUN**.

## HALL-05 — King combat and realignment

Fair telegraphs, actual hit volumes, dynamic routes, player-built interventions and persistence work without arbitrary immunity or deleting the players tools.

Origin: `inherited_verbatim`. Sections: D10, D11, D12. Tasks: T21. Status: **NOT_RUN**.

## HALL-06 — Extraction and reconnect

Multiple supported exit/rescue paths preserve participants through disconnect and failure; pre-victory return is available and ordinary possessions are not wiped.

Origin: `inherited_verbatim`. Sections: D10, D11, D12. Tasks: T21. Status: **NOT_RUN**.

## HALL-07 — Charted nexus and replay-safe farming

Successful extraction unlocks only authorized discovered edges; return visits/reset costs/rewards preserve identity, avoid duplicate loot and do not erase the whole location.

Origin: `inherited_verbatim`. Sections: D10, D11, D12. Tasks: T21. Status: **NOT_RUN**.

## MIX-01 — Four simultaneous player intentions

Explorer, factory operator, fighter/builder and joining player share the actual inhabited world under impairment and forced relay with bounded state age and queues.

Origin: `inherited_verbatim`. Sections: D00, D05, D13, D16, D17, D18. Tasks: T13, T17, T19, T21. Status: **NOT_RUN**.

## MIX-02 — Long-run memory and recovery

Travel, edits, plants, scripts, saves, frames, portals and vehicles stop growing retained state after expected steady demand; post-burst debt drains under continued normal use.

Origin: `inherited_verbatim`. Sections: D00, D05, D13, D16, D17, D18. Tasks: T13, T17, T19, T21. Status: **NOT_RUN**.

## MIX-03 — Reduced visual quality and accessibility

Lower effects/resolution/detail preserve collision, hazards, clues, cover, material distinction and operable UI; color/sound-only dependencies have alternatives.

Origin: `inherited_verbatim`. Sections: D00, D05, D13, D16, D17, D18. Tasks: T13, T17, T19, T21. Status: **NOT_RUN**.

## MIX-04 — Current world at every representation

Globe, far skyline, near terrain, interior, vehicle frame, portal view and reload agree on consequential edits and identities through extended play.

Origin: `inherited_verbatim`. Sections: D00, D05, D13, D16, D17, D18. Tasks: T13, T17, T19, T21. Status: **NOT_RUN**.

## MIX-05 — Honest finished-slice delivery

Provide original code/assets, executable native procedures, observed failures, measured comparisons and remaining scope; no synthetic screenshot or unrun test presented as proof.

Origin: `inherited_verbatim`. Sections: D00, D05, D13, D16, D17, D18. Tasks: T13, T17, T19, T21. Status: **NOT_RUN**.

## REV-01 — Scope before patch

One selected bounded proof names actual source owners, allowed changes, native steps and stop condition; no unrelated subsystem rewrite.

Origin: `review_addition`. Sections: D00, D18. Tasks: T00. Status: **NOT_RUN**.

## REV-02 — Correctness before cosmetic diagnosis

A suspicious dark region is separated into geometry/depth, CSM, distant visibility and final radiance before any asserted root cause.

Origin: `review_addition`. Sections: D14. Tasks: T01, T02. Status: **NOT_RUN**.

## REV-03 — No invented terrain fallback

Known cave/arch and edited openings remain open through delayed child uploads; unknown state does not become fabricated solid terrain.

Origin: `review_addition`. Sections: D05, D14. Tasks: T01. Status: **NOT_RUN**.

## REV-04 — Shadow domain overlap

Valid distant and near blockers remain effective; no max/average/min change passes solely because it makes an artifact lighter.

Origin: `review_addition`. Sections: D14. Tasks: T02. Status: **NOT_RUN**.

## REV-05 — Shader semantic layout

Material kind/flags, host layouts and shared declarations are checked in current source; any formerly overloaded family ID has explicit supported meaning.

Origin: `review_addition`. Sections: D14. Tasks: T02, T03. Status: **NOT_RUN**.

## REV-06 — Ground identity without labels

Bedrock, sediment, bare soil and maintained ground differ through appropriate shape/edges/use; native unlabeled scenes are understandable beyond green-gray color masks.

Origin: `review_addition`. Sections: D06, D13. Tasks: T03. Status: **NOT_RUN**.

## REV-07 — Terrain mass hierarchy

Ordinary uncurated clay views show connected masses/ridges/valleys and quiet sections; new peak noise alone is insufficient.

Origin: `review_addition`. Sections: D06, D13. Tasks: T07. Status: **NOT_RUN**.

## REV-08 — Geological counterfactual

Alter lithology or hydraulic connectivity in a disposable fixture; valid cave/spring/habitat/excavation consequences differ without rerolling runtime worlds.

Origin: `review_addition`. Sections: D06. Tasks: T07. Status: **NOT_RUN**.

## REV-09 — Regional schedule independence

Generation order, worker count and cosmetic content streams preserve immutable geological/river/site identities under the same manifest.

Origin: `review_addition`. Sections: D05, D06. Tasks: T07, T13. Status: **NOT_RUN**.

## REV-10 — Fine-edit amount invariance

Equivalent physical removed/placed volumes across supported cell sizes preserve material accounting and rounding remainder under repeated edits.

Origin: `review_addition`. Sections: D04, D05, D15. Tasks: T01, T06, T14. Status: **NOT_RUN**.

## REV-11 — Botanical anatomy and origin

One fern/shrub has coherent attachment, recognizable form and actual production source; a procedural blob or unexecuted script does not pass native art review.

Origin: `review_addition`. Sections: D07, D13. Tasks: T04. Status: **NOT_RUN**.

## REV-12 — Specimen identity under recruitment

Adding seedlings or changing cohort membership does not move/reidentify existing harvested or protected plants; reload preserves modifications.

Origin: `review_addition`. Sections: D05, D07. Tasks: T08. Status: **NOT_RUN**.

## REV-13 — Ecology open-system balance

Growth ledger declares environmental material/energy inputs and outputs; promotion/demotion alone changes no harvestable biomass or stock.

Origin: `review_addition`. Sections: D07, D15. Tasks: T08. Status: **NOT_RUN**.

## REV-14 — Foliage coverage and motion

Near-to-far plant coverage preserves recognizable crowns without detached trunks, uncontrolled alpha overdraw or significant camera-dependent flicker.

Origin: `review_addition`. Sections: D07, D14. Tasks: T04. Status: **NOT_RUN**.

## REV-15 — Construction-kit parity

Player-accessible roof/door/frame interfaces reproduce the promised visual language; site access and structure work from reverse/ground-level approaches.

Origin: `review_addition`. Sections: D04, D08, D13. Tasks: T05. Status: **NOT_RUN**.

## REV-16 — Building condition is legible

Intact/ruined/repaired state has structural and usage evidence, not arbitrary missing walls beside pristine surfaces.

Origin: `review_addition`. Sections: D08, D13. Tasks: T05. Status: **NOT_RUN**.

## REV-17 — Early convenience playtest

Fresh builder/explorer can obtain useful storage and tools without boss completion, forced factory, hostile grinder or external tutorial; observed friction is recorded.

Origin: `review_addition`. Sections: D02, D03, D04. Tasks: T06. Status: **NOT_RUN**.

## REV-18 — Death recovery trial

Repeated failures do not create default permanent fragility/gear-loss spiral; recovery has understandable stakes and voluntary experimentation remains viable.

Origin: `review_addition`. Sections: D02. Tasks: T06. Status: **NOT_RUN**.

## REV-19 — Offscreen autonomy parity

Same functioning factory receives the same elapsed simulation and real inputs while owner explores versus stands nearby; comparable outcomes follow the declared reduced model.

Origin: `review_addition`. Sections: D03, D09. Tasks: T09. Status: **NOT_RUN**.

## REV-20 — Heat-pump source accounting

Thermal output, environmental extraction and electrical work reconcile; a useful coefficient of performance is not confused with closed-loop free energy.

Origin: `review_addition`. Sections: D09, D15. Tasks: T09, T14. Status: **NOT_RUN**.

## REV-21 — Discovery without forced human trigger

Sensors/drones can perform supported observations; UI only reveals legitimately obtained facts and distinguishes generated knowledge from inaccessible private state.

Origin: `review_addition`. Sections: D03, D16. Tasks: T12, T13. Status: **NOT_RUN**.

## REV-22 — Guidance is optional and factual

Help can be disabled or requested; it explains known next actions without inventing an event, source, buried reward or mandatory objective.

Origin: `review_addition`. Sections: D03, D11. Tasks: T12. Status: **NOT_RUN**.

## REV-23 — Familiarity versus stale repetition

Playtest ordinary building/production/travel and record unwanted repetition without treating all recognizable motifs or quiet sessions as failure.

Origin: `review_addition`. Sections: D01, D03, D18. Tasks: T05, T09, T12, T20. Status: **NOT_RUN**.

## REV-24 — Accessible cue redundancy

A required clue/operational signal is not solely color, tiny pixels, sound or flashing; alternate supported cues preserve the same knowledge.

Origin: `review_addition`. Sections: D03, D13. Tasks: T05, T12, T20. Status: **NOT_RUN**.

## REV-25 — Seed secrecy boundary

Security review explicitly distinguishes reconstructible generated geography from protected mutable inventories/claims; shared seed is not asserted to prevent datamining.

Origin: `review_addition`. Sections: D16. Tasks: T13. Status: **NOT_RUN**.

## REV-26 — Partial proof does not finish a feature

Tether visual acceptance never marks cargo/orbit ready; moving assembly never marks orbital return ready; only the selected proof and its evidenced cases change status.

Origin: `review_addition`. Sections: D18. Tasks: T00, T17, T18, T19. Status: **NOT_RUN**.

## REV-27 — Task prerequisite independence

A local encounter or art correction is not blocked by unrelated new gliding, fab, civilization or fluid systems; required behaviors can come from proven existing code.

Origin: `review_addition`. Sections: D18. Tasks: T00, T20. Status: **NOT_RUN**.

## REV-28 — Subpixel tether compositing

A narrow/widened tether displays against land, cloud and sky without opaque background erasure; motion and crossings preserve projected coverage and depth policy.

Origin: `review_addition`. Sections: D14. Tasks: T18. Status: **NOT_RUN**.

## REV-29 — Matched visual evidence

Before/after record same save, seed, camera, exposure/settings and build identity plus motion; any changed conditions are disclosed.

Origin: `review_addition`. Sections: D13, D17. Tasks: T00, T02, T03, T04, T05. Status: **NOT_RUN**.

## REV-30 — First-person construction framing

Held material is identifiable without unnecessarily obscuring placement target/scene; viewport/FOV/movement and other-player representation are checked.

Origin: `review_addition`. Sections: D02, D13. Tasks: T05, T06, T10. Status: **NOT_RUN**.

## REV-31 — Production provenance and tool reality

Authored sources/imports/licences are recorded; executed Blender or asset tools have real outputs; unsupported formats/tools are marked blocked, not claimed.

Origin: `review_addition`. Sections: D13, D18. Tasks: T04, T05. Status: **NOT_RUN**.

## REV-32 — Supported overload retains accepted effects

When work queues saturate, already admitted damage/transfers occur under declared staged semantics; optional presentation is shed without deleting reality.

Origin: `review_addition`. Sections: D05, D15, D17. Tasks: T01, T10, T14. Status: **NOT_RUN**.

## REV-33 — Capability diversity without compulsory curriculum

An advanced system provides useful presets and alternative acquisition/use paths; ordinary house/expedition play does not require every industrial discipline.

Origin: `review_addition`. Sections: D03, D09, D12. Tasks: T09, T15, T17. Status: **NOT_RUN**.

## REV-34 — Ordinary place without forced spectacle

Remove lore labels, special-event scripts and quest markers in a test copy; ordinary movement/building/observation remains worthwhile to sampled players.

Origin: `review_addition`. Sections: D01, D03, D06, D18. Tasks: T03, T07, T12. Status: **NOT_RUN**.

## REV-35 — Two-client proof before planetary multiplication

Early shared crate/edit/local physics/reconnect evidence exists; low routine traffic is not claimed from a seed or localhost connection alone.

Origin: `review_addition`. Sections: D16. Tasks: T13, T19. Status: **NOT_RUN**.

## REV-36 — Second-world mechanical difference

Transport existing equipment to a different environment; at least one meaningful changed decision and valid return are observed, not just a palette or crafting lock.

Origin: `review_addition`. Sections: D12. Tasks: T19. Status: **NOT_RUN**.

## REV-37 — Cultural/editorial integrity

Culture, body plan, job and morality remain separate; source-specific folklore is reviewed with appropriate expertise before final direct portrayal, and origin need not be explained away as machinery.

Origin: `review_addition`. Sections: D01, D08, D11. Tasks: T12, T21. Status: **NOT_RUN**.

## REV-38 — No environmental global freeze

A deliberately stalled client, renderer minimization or device recovery does not erase listen-host state or freeze other supported players; retained work and retirement stay bounded.

Origin: `review_addition`. Sections: D16, D17. Tasks: T13. Status: **NOT_RUN**.


---

# Reference notes

# Source and decision map

Reviewed 29 October 2026. This is provenance, not an extra file the agent must load before every task.

## Project sources

| Source | Read coverage in this review | Use |
|---|---|---|
| `AXIOM_COMPLETE_IMPLEMENTATION_PROMPT.md`, Library file `file_000000008c2081fd8f1289962ea53a28` | Entire 1,494-line document, including all 122 acceptance clauses | Main cross-chat product/technical baseline; local physics, exact proven domains with fallback, storage, realms, spacecraft, Hall, safe implementation |
| `AXIOM_V8_4_UNIVERSE_ARCHITECTURE.md`, conversation file `file_00000000ce9881fd89046f81d7763cf5` | Entire 355-line document | Inhabited worlds, homeworld-first, different engineering/ecological opportunities and limits of procedural novelty |
| `V8_3_COMBINED_MASTER.md`, conversation file `file_00000000ea5481fdbdbda73375b594ea` | Relevant geology, water, ecology and guidance passages, plus visible conversation | Geosphere causality, stable regional generation, coarse/fine ecological accounting, meaningful player evidence |
| Current conversation and supplied shader text | Available conversation context; shader statements are historical investigation leads | Art failures, hypotheses to reproduce, progressive correction of earlier overconfident diagnostics |
| `1000021717.png`, conversation file `file_00000000ddf081fdbaf0c6a6647e4aac` | Supplied image visually reviewed | Failure reference for ground classification, tree morphology, site/roof/landform hierarchy and held-item framing |

The latest directive's source identity and SHA256 are recorded in `UPSTREAM_SOURCE_IDENTITY.json`. Its 122 acceptance clauses are retained literally in `upstream_cases.json` and in the inherited part of `../ACCEPTANCE.json`; no assertion is made that their game tests ran. Earlier files mentioned by the consolidated source are not thereby independently re-inspected here. No private repository inspection or broader account-wide completeness is claimed.

## How sources become this design

| Reviewed design sections | Main source-derived requirements | Explicit new refinement |
|---|---|---|
| D00–D04 | Existing game/engine, early comfort, sandbox economy, storage and capability progression | Clear trial rules; retain useful repetition; scope model, human-only novelty and performance claims corrected |
| D05–D08 | Canonical world, deterministic generation, causal geology, plants, working history/settlements | Current snapshot ground semantics; open ecosystem inputs; stable recruitment identities; no global simulation prerequisite |
| D09–D12 | Industry, actual controller usefulness, compositional magic, traversal, realm/space/Hall canon | Presets and closed accounting without forcing scientific chores; staged optional exceptional content |
| D13–D15 | Pixel-material art, native assets, coverage, shadow diagnosis, material coupling | Explicit screenshot diagnosis limits; subpixel cable/background contract; no invented solid fallback; prove compatible shadow domains |
| D16–D18 | Relevant client physics, proven exact domains/fallback, bounded resource lifetime, evidence | Seed not secrecy; proof-level dependencies; short task-focused execution and plural technical/art/playtest acceptance |

Retained source wording is a design requirement, not evidence. New trial balance and candidate algorithms are labeled in DESIGN.md. REVIEW.md records editorial and technical corrections instead of silently presenting them as source facts.

## Primary technical references checked

### Codex and Astra instructions

OpenAI, “Rethinking skills and prompts for GPT-6 Astra”:
https://developers.openai.com/blog/rethinking-skills-and-prompts-for-gpt-6-astra

Adopted principle: concise routing, relevant context loaded when needed, and removal of contradictory/overspecified legacy workflow. This is general documented Astra guidance, not an independently measured result for the user's exact Astra Light configuration. No undocumented flags, context limits or model benchmarks are specified.

OpenAI, “Custom instructions with AGENTS.md”:
https://developers.openai.com/codex/guides/agents-md

Adopted principle: respect actual repository instruction discovery and local scopes; merge a small optional fragment rather than overwriting existing root policy. The executing agent must inspect the currently installed environment.

OpenAI, Codex execution plans:
https://developers.openai.com/cookbook/articles/codex_exec_plans

Adopted principle: keep concise durable state for larger work with explicit verification. Do not require an elaborate plan for every small change or load every historical design on every iteration.

### Rendering and deterministic computation

WGPU official generated API documentation, Features:
https://docs.rs/wgpu/latest/wgpu/struct.Features.html

Capability queries govern optional GPU features. Consult the **pinned repository version** before implementation. Documentation of an optional feature is not proof the user's chosen adapter/backend supports it or that it improves a given workload.

Eric Lengyel, Transvoxel:
https://transvoxel.org/

This method connects compatible sampled voxel isosurfaces at a 2:1 resolution boundary. It is not a universal replacement for cubic face ownership, heightfield seams or edit publication. Reuse only when the representation fits.

NVIDIA GPU Gems 2, GPU-based geometry clipmaps:
https://developer.nvidia.com/gpugems/gpugems2/part-i-geometric-complexity/chapter-2-terrain-rendering-using-gpu-based-geometry

Nested heightfield representations and transition techniques are relevant to suitable terrain envelopes. They do not by themselves represent multiple solid/air intervals, arches or edited tunnels.

W3C, WGSL specification:
https://www.w3.org/TR/WGSL/

Use actual language/layout and derivative rules with the pinned shader validator and host ABI. A text-level shader review is not compilation or native GPU evidence.

Factorio developers, Friday Facts #340, “Deep desyncs”:
https://www.factorio.com/blog/post/fff-340

Reference for the practical burden of simulation consistency. Axiom's exact replication requires whole-domain tests and restoration proof; it is not established simply by adopting one deterministic component.

## Source boundaries

No release dates, “latest engine” claims, historical model performance, proprietary Mycopunk shader source availability, or purported Blender access from old answers are inherited as current facts. No third-party artwork or game assets are licensed for production merely because they appear in an inspiration list. No cultural tradition is reduced to a technical fact solely for game convenience.
