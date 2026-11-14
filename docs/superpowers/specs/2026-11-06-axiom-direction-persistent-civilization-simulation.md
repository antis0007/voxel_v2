# Axiom: persistent civilization simulation — direction

Status: **direction / governing**, 2026-11-06. Owner-authored. This is the frame the
civilization work reports to. It is a design and a set of **proposed profiling targets**, not
measurements of Axiom.

## 0. What this supersedes, and what it keeps

Superseded in the earlier discussion (discarded as unsupported):

- that tiny spiking networks will spontaneously develop civilization;
- that everything belongs on the GPU;
- that hundreds of thousands of fully interactive learning agents are an established 60 FPS
  workload.

Kept, unchanged:

- the **Rust + raw wgpu** architecture, the variable-resolution voxel world, the emphasis on
  bounded workloads and weak-hardware support. Nothing here justifies a C++ or direct-Vulkan
  rewrite.

Replaced goal:

> **Maximize meaningful, persistent consequences per millisecond, not agent updates per
> second.**

## 1. The objective

A world where settlements need resources, people have individual experiences, infrastructure
changes opportunity, societies adapt, and their decisions leave **physical evidence** the
player can read and intervene on anywhere along the chain. Depth comes from geography and
people explaining one another, not from a larger crowd.

## 2. Start from the actual budget

At 60 FPS the whole frame is **16.67 ms**. Civilization simulation shares it with terrain,
shadows, visibility, voxel changes, particles, physics, streaming and the interface. The new
civilization and behaviour work gets roughly **1-2 ms per frame**, subject to profiling: an
allocation decision, not a prediction that it already fits.

Performance is bounded by the roofline, not by a FLOP count:

```
t_kernel >= max( arithmetic work / achieved throughput, memory traffic / achieved bandwidth )
```

Synchronization, dispatch overhead, irregular access and dependencies add more.

**An illustrative policy** (96 inputs, 128 input units, a 128-unit GRU, 32 outputs) is
`96*128 + 3*(128*128 + 128*128) + 128*32 = 114,688` weights, about **229,000 FLOPs per
decision**. That draws to roughly **224 KiB in FP16**.

| Agents at 10 decisions/s | Neural arithmetic | Arithmetic-only ms/frame at 5 TFLOP/s |
|---:|---:|---:|
| 4,096 | 9.4 GFLOP/s | 0.031 |
| 16,384 | 37.6 GFLOP/s | 0.125 |
| 65,536 | 150.3 GFLOP/s | 0.501 |

Arithmetic-only lower bounds, with decisions spread across frames. The conclusion is not that
65,000 agents are easy. It is that **a shared policy may be cheaper than obtaining its
observations and executing its decisions**: pathfinding, sensing through editable terrain,
congestion, social lookups, construction and resource contention can dominate the cost.

**Unique brains destroy the budget.** 100,000 independent policies is about **22.94 GB** of
weights, near the whole VRAM before rendering or world data. Reading every weight once per
decision is about **229 GB/s at 10 decisions/s** and **1.38 TB/s at 60**, the latter past the
card's theoretical bandwidth. Sixteen shared policies occupy about **3.5 MiB**.

> **First design decision: shared computational machinery, persistent individual state.**

---

## 3. Separate population, cognition, embodiment and rendering

An agent is not one indivisible object that is either fully simulated or absent. Separate the
responsibilities:

| Representation | Persists or runs | Use |
|---|---|---|
| Individual record | identity, age, household, possessions, relationships, skills, key memories | everyone whose individuality matters |
| Scheduled activity | travelling, working, resting, learning, consuming, awaiting an event | most distant individuals |
| Active decision-maker | perception, recurrent state, immediate choices, navigation | relevant settlements, encounters, disruptions |
| Embodied actor | collision, locomotion, reach, combat, animation | nearby physical interactions |
| Rendered representation | mesh, animation detail, impostor, marker, or nothing | whatever the viewer needs |

A person does not lose identity because their body is unloaded, and their household does not
stop eating. An offscreen carpenter does not need sixty collision solves per second to remain
a carpenter.

**Offscreen is a compact, interruptible activity**, not a frozen NPC:

```
Actor: individual 4821
Activity: transport, cargo 18 timber, route edges 91 -> 104 -> 107
Departure: world time T, next event: arrival at crossing 104
Reservations: destination delivery contract
```

A destroyed bridge, a closed border, a flood or a player encounter invalidates it and forces a
new decision. This represents ongoing behaviour at the level where consequences occur.

**Population thresholds do not control reality.** Reject "only social settlements get voxels".
A lone prospector can open a valuable mine, a caravan can raise a temporary bridge, a dungeon
can hold working machinery, a player can build a remote power station alone. Detailed
simulation is activated by **physical interaction and model inadequacy**, not by resident
count.

**Render detail stays independent.** Looking at a town must not make its citizens more
productive; looking away must not change its food consumption. Materializing geometry,
raising animation quality and showing statistics are presentation decisions; changing the
behavioral approximation is a separate, validated decision. Test by running one scenario at
different simulation-detail settings: if famine, migration or conflict change materially when
the camera moves, the approximation is influencing the result.

**Initial profiling envelope** (ranges to benchmark, not promised capacity):

| Workload | Range |
|---|---:|
| Persistent individual records | 100,000 - 1,000,000 |
| Individuals actively receiving decisions | 1,000 - 16,000 |
| Nearby embodied, interactive NPCs | 64 - 512 ordinary; about 2,000 in a crowd stress test |

One million 256-byte summaries is 256 MB, before detailed memories, inventories, relationships
and learning state, which belong in separate cold storage rather than resident GPU memory.

## 4. Build the civilization layer on the geography, not beside it

One underlying set of geographical facts serves terrain generation, ecology, navigation,
settlement planning and history. Three complementary forms:

- **Regional environmental records** (water, seasonal temperature, growing conditions,
  vegetation stocks, geological resources, accessibility, accumulated disturbance). Not voxel
  chunks; resolution follows environmental variation, not the mesh. A 128-byte regional record
  is 128 MB for a million regions, a strategic dataset before caches and indices. Keep a few
  meaningful patches inside a region so a half-floodplain, half-cliff region is not uniformly
  mediocre.
- **Geographical and infrastructure graphs** (rivers, crossings, passes, roads, tunnels,
  ports, portals). These make distance real: two settlements 10 km apart are not automatically
  more connected than two 50 km apart; the route, its capacity, season, danger and energy cost
  decide. For Axiom this graph also carries powered portals, skyhooks, underground passages and
  Manyfold routes.
- **Local physical representations** (voxels, collision, rooms, machines, resource objects).

The loop is two-way:

```
geography -> opportunities and travel constraints -> settlement decisions and infrastructure
          -> physical construction -> changed environmental and economic conditions -> ...
```

A town that cuts its woodland changes its regional timber stock and future supply, not only
decorative trees. World generation should establish **connected causes** (provinces, drainage
basins, passes, floodplains, coastlines, resource formations) that settlement generation
consumes; do not place a mining village and then invent a deposit to justify it.

**Keep the map projection out of the laws.** A rectangular map is an interface. On a sphere,
east-west distance scales with `cos(latitude)`; at 60 degrees the same longitude interval is
half the distance it is at the equator. Use the existing topology; regions carry explicit areas
and connections explicit lengths. Choose planet size at world creation, not from current
population, so deaths cannot change geographical density.

## 5. Make the economy physical enough to matter, but not atomistic

Begin with a small number of tightly coupled systems, not dozens of weakly connected
biological variables.

**Food, water, fuel, materials and labour.** Every productive activity consumes real inputs and
time. A farm depends on land, water, planting material, labour, tools and conditions. A mine on
accessible deposits, extraction capacity, transport and maintenance. A workshop cannot produce
because its region has a positive industrial score. A useful bound:

```
output = min( input-limited, labour-limited, tool/machine capacity,
              energy-limited, storage/transport capacity )
```

A proposed game model, not a claim of economic completeness. It gives the player real levers: a
bridge unlocks a mine without changing its ore; better tools cut labour; a portal reorganizes a
supply network.

**Agriculture supports adaptation without simulating every plant.** Crop lineages carry a small
trait vector (yield potential, maturation time, water demand, temperature tolerance, storage
durability) with **trade-offs**: higher yield may need more water or mature later; a hardy
crop yields less. No action monotonically raises every desirable property. Agents must
encounter observable differences, keep seed stocks, and choose what to replant, or agricultural
improvement is a background bonus instead of learned practice. Aggregate growth over fields;
individual plant geometry only where interaction demands it.

**Shelter changes actual conditions.** Do not reward "roof detected" or "large room" directly.
A shelter influences temperature exposure, precipitation, wind, security, storage and access,
through an approximate room-level thermal model (`C dT/dt = sum(U A (T_other - T)) + Q_heat -
Q_vent`), calibrated as game parameters. Spaciousness contributes to comfort with diminishing
returns and competing cost, so an empty hangar is not the ideal home. The 0.5 m proposal stays a
**planning resolution**; clearance uses real geometry, body dimensions and movement margins,
not "four air voxels means walkable".

**Player machines join the same economy.** A player-built farm, furnace, storage network or
mana installation feeds the same inventories and production rules as NPC settlements, and its
distant representation conserves inputs and outputs. Unloading a machine must not grant free
production, remove maintenance, or duplicate inventory. The player's factory and the
civilization are one simulation.

## 6. Construction emerges from needs, capabilities and local constraints

Do not ask every NPC to rediscover structural engineering through random voxel placement in
ordinary play. Separate three questions:

1. **What is needed?** A household lacks protected sleeping space; a workshop lacks storage; a
   crossing has become unreliable.
2. **What does this person or community know how to do?** Available techniques, materials,
   plans, tools and demonstrated practices.
3. **What can be built here?** Terrain, ownership, access, structural support, water, resources
   and existing buildings constrain the result.

The output is a **construction project**, not an instantly appearing building: it reserves
materials, creates work tasks, modifies the real world, and stays vulnerable to interruption.

A plan is a **constrained arrangement of functions** (entrance, circulation, sleeping, storage,
work, heating, drainage, support); its realization depends on the site and techniques. The same
need yields a courtyard house, a hillside dwelling, a timber hall, or rooms inside an older
ruin, and historical alterations stay visible. This also grounds ruins: **a ruin is the
surviving material state of a place that had inhabitants, dependencies, modifications and a
reason to decline**, not a regenerated clean template.

**Recalculate voxel costs before fixing settlement dimensions.** A 2 km x 2 km x 256 m site at
0.5 m is 8.192 billion voxels, about **16.384 GB at two bytes per voxel** before lighting,
navigation, meshes or simulation. A 256 m x 256 m x 32 m site is 16.8 million voxels, 32 MiB.
So distant settlements persist as **site plans, structure records, inventories, terrain
references and edits**, with voxel pages materialized and cached through Axiom's world system.
"Only settlements have voxels" is not enough: **settlements themselves need sparse, layered
representation.**

## 7. A hybrid agent architecture, not a tiny universal brain

Preferred Axiom agent: **persistent identity + needs + explicit memory + task execution + a
compact learned policy**, with different responsibilities.

- **Persistent state establishes individuality**: a durable identity, household, possessions,
  relevant relationships, skills, physiological condition, remembered experiences.
- **Memories are structured facts with provenance and uncertainty**, not unexplained floats:
  `Food was available at the western storehouse; observed three days ago; confidence reduced
  after reports of a fire.` The agent never gets omniscient access to the true resource map;
  perception, communication and exploration decide what it believes.
- **Explicit execution handles commitments**: walking, carrying, completing a construction
  task, operating a machine need robust execution. Do not ask the policy to rediscover an
  inventory transfer every frame. Use **action contracts**: prerequisites, reserved resources,
  progress, interruption conditions, outcomes.
- **Learned policies handle uncertain choices**: whether to continue working, seek shelter,
  trade, explore, cooperate, retreat, or take a risky opportunity.

The illustrative 115,000-weight network is a **benchmark candidate, not a claim of sufficient
intelligence**. Compare it against a smaller feed-forward policy and a **non-neural utility
baseline**; a larger brain must earn its cost through measurable behaviour.

**Learning method**: parameter-shared recurrent PPO is a defensible baseline for cooperative
skill learning (MAPPO evidence supports testing it; it does not establish open-ended
civilization). Do not default to spiking networks or forbid backpropagation; the comparison is
**useful learning per unit of compute and experience**. Keep three processes separate:
**development training** (reusable skills in small accelerated scenarios), **runtime
adaptation** (individual memories, preferences, value estimates, small personal adapters), and
**evolution** (inherited traits across births).

**Shared weights must not create telepathic culture.** Globally updating from one village's
experience silently transmits knowledge to every village. For ordinary play, **freeze the
common skill foundation during a session** while personal learning and explicit transmission
continue. For experiments, shared-learning boundaries are part of the design: an isolated
society must not gain a practice because an unrelated society supplied a sample. Culture
travels through observation, teaching, apprenticeship, copied plans, records, migration and
trade.

**Risk-taking has reasons.** Agents can act harmfully through incomplete information, mistaken
memory, conflicting commitments, impatience, social pressure, or a bad danger estimate, so the
player can understand the consequence and researchers can analyse it.

**Two configurations.** *Playable Axiom*: agents begin with basic embodied competence and some
inherited or taught practices. *Experimental worlds*: selected capabilities start untrained,
with controlled rewards, curricula and measurement. They share the world and action systems but
not the initialization assumptions.

## 8. Use the GPU selectively; keep authoritative changes transactional

Moving an irregular, low-volume job to the GPU can add synchronization without improving frame
time. Divide approximately, then follow measurements:

| Suitable GPU batches | Suitable CPU / event-driven work |
|---|---|
| Large batches of policy inference | Sparse institutional and historical events |
| Dense environmental-field updates | Construction-project scheduling |
| Local perception sampling | Save/load and lineage archives |
| Crowd steering and spatial filtering | Irregular route-graph maintenance |
| Population and field reductions | Authoritative inventory and ownership transactions |
| Map visualization | Network coordination and validation |

Use Axiom's existing wgpu backend and WGSL. Optional half precision and subgroup operations must
be checked against the adapter and the pinned wgpu version; experimental cooperative-matrix
support has significant format and backend restrictions and is not a portable fast path.

**Do not attempt one giant simulation kernel.** Use explicit phases:

```
snapshot -> observations -> policies/activities -> proposed actions
         -> resolve conflicts and reservations -> commit accepted changes -> publish events
```

Two agents cannot both consume the last meal because they read the same old inventory;
construction cannot create resources through simultaneous placement; a regional aggregate and a
local actor cannot both count the same harvest. **The commit stage enforces these invariants.**

**Preserve Axiom's networking principles.** Clients predict local movement and effects, but
shared economic, social and construction outcomes need an authoritative owner. WGSL permits
variation in rounding and expression evaluation, so cross-device bitwise reproducibility is not
guaranteed: replicate commands where determinism is proven, otherwise replicate **compact
accepted outcomes** and reconcile. A population's whole neural state never becomes a per-frame
network payload.

## 9. History, culture and infrastructure are the source of depth

The valuable outcome is not a larger crowd; it is a world whose geography and people explain one
another. A **hypothetical systemic sequence**, not an authored quest:

> A settlement mines copper uphill from a river crossing, and its food arrives from a lowland
> district. A storm destroys the crossing; food prices rise, some miners leave, timber redirects
> from new housing into repairs. The player opens a powered portal, which changes which
> settlements are reachable, who controls the connection, whether the old road stays maintained,
> and where workshops become profitable. Later a mana blight destabilizes the portal supply and
> the neglected road matters again, but its bridge is still damaged.

Nothing there needs every resident to run a network at 60 Hz. It needs persistent dependencies,
competing decisions and physical consequences.

Axiom-specific systems worth connecting:

- **Magic alters constraints** (heating, preservation, transport, healing, extraction), changing
  resource requirements and vulnerabilities, not a generic prosperity score.
- **Knowledge is unevenly distributed**: a settlement can run a machine it can no longer
  manufacture; another knows a technique but lacks the material.
- **Maintenance matters**: roads, wards, mines, irrigation, buildings and machines deteriorate
  under bounded models, so abandonment has a physical trajectory.
- **Ruins retain useful and dangerous dependencies**: a buried pumping station still draining a
  district; a sealed workshop holding the only surviving tooling for a local industry.

**Genetics is not the fastest route to change.** With a twenty-minute game day, a 365-day year
is about **122 real hours** and a twenty-year generation is about **101 real days of continuous
play**. So visible adaptation in ordinary play must come from learning, migration, institutions
and technology; use accelerated historical runs for multigenerational development, recognizing
that their coarse model is not identical to replaying every embodied action faster.

## 10. Keep the project small; test the model before expanding

Add responsibilities inside Axiom's existing workspace; the important boundary is **ownership
of state**, not the number of folders.

```
world         regional records, geographical connections, site records and materialization
society       people and households, inventories and production, activities, construction, knowledge
behavior      observations and memory, policy interface, task execution, learning experiments
simulation    scheduling, action validation and commits, conservation checks, history and events
tools         world overlays, individual/site inspection, causal event inspection, performance views
```

A renderer reads simulation state; it does not advance society. A policy proposes actions; it
does not modify inventories. A settlement planner creates projects; it does not bypass the voxel
edit system.

**The first experiment is a valley, not a planet.** Several settlements, one water system, a
small transport network, food, timber, one mineral, shelter, storage, construction, migration and
trade; disrupt with a cold season, a failed crossing, resource depletion and a player-built
transport shortcut.

**The first success criterion is not that the population grows.** It is that you can answer:
*why did this household leave, what options did it perceive, what resources constrained it, and
what changed elsewhere because of that decision?* Then check that the answers stay coherent when
the area is unloaded, revisited, or represented at a different simulation resolution.

**Measure behaviour and uncertainty, not screenshots.** Food security, mortality causes, travel
costs, resource depletion, settlement persistence, inequality of access, infrastructure
utilisation, recovery after disruption. Run multiple independent worlds: thousands of interacting
agents in one world are not thousands of independent experiments. For learning, report
distributions and uncertainty, not the best run. For engineering, record GPU and CPU timings,
tail frame times, memory peaks, queue growth, edit latency and transition costs. A model that
behaves well only while the camera is elsewhere, or before the first large project, is not ready
to scale.

## 11. Recommended Axiom design

| Decision | Recommendation |
|---|---|
| Engine | Keep Rust + raw wgpu |
| Main objective | Persistent causal depth, not maximum simultaneous neural agents |
| Population | Persistent individuals with event-driven distant activities |
| Active AI | Compact shared policies with individual memory and bounded adaptation |
| Construction | Needs -> known techniques -> constrained plans -> physical work |
| World detail | Regional records, infrastructure graphs, sparse local geometry |
| Economy | Conserved resources, explicit production limits, real transport |
| Learning | Train reusable skills separately; support controlled runtime adaptation |
| Culture | Explicit transmission, not global weight-update telepathy |
| History | Accelerated coarse simulation with persistent material consequences |
| Multiplayer | Authoritative shared outcomes; prediction where appropriate |
| Validation | Profiling, conservation tests, resolution comparisons, independent runs |

> The ambitious version is a world that **remembers what happened and continues to respond
> coherently** - not a world that spends every available FLOP making every citizen think
> constantly. Prove that architecture with **one living valley**: settlements that consume,
> build, trade, learn, fail and recover, and a player whose interventions stay consequential
> after leaving. Then expanding the world becomes a scaling problem instead of an attempt to
> rescue an unproven simulation model.

## 12. Relation to the other 2026-11-06 designs

| Design | Effect of this direction |
|---|---|
| [World AI, companions and conquest](2026-11-06-world-ai-companions-and-conquest-design.md) | **Superseded in part.** The "hyperintelligent world AI" as a big neural brain is replaced by a compact **shared policy + persistent individual memory + explicit events**. The three tiers and one-owner-per-quantity rule still hold. |
| [Conquest, politics and the playstyle paths](2026-11-06-conquest-politics-and-playstyle-paths-design.md) | Kept; the strategy surface reports to this direction. |
| [Walls, sieges, epochs and RTS inspiration](2026-11-06-walls-sieges-ages-and-rts-inspiration-design.md) | Kept. |
| [Fantasy battle sandbox](2026-11-06-fantasy-battle-sandbox-design.md) | Kept as a **game mode**; survival stays clean, summoning is gated. |
| [Villager civilization slice: plan](../../plans/2026-11-06-villager-civilization-slice.md) | Kept as the settlement-record path; the demography, economy and construction steps gain the conservation and detail rules above. |

## 13. References (as supplied)

- Ada GPU architecture paper (RTX 4090 FP32, bandwidth, VRAM).
- Roofline / performance-optimization guidance (arithmetic vs memory bound).
- Priority-Flood watershed construction (drainage relationships).
- MAPPO (parameter-shared cooperative learning) - evidence to test, not proof.
- Emergent-tool-use environment studies - research mode, not the default.
- Deep-RL evaluation reliability (distributions over runs).
- wgpu feature documentation (optional and experimental features).
- WGSL specification (rounding and expression variation across devices).

