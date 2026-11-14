# Civilization on solved geography

Status: design, 2026-11-05. Owner direction from the 2026-11-04/05 sessions. Extends
`docs/NEW/AXIOM_Planetary_Simulation_And_Coop_Architecture.txt` chapter 4 (city
pipeline, bounded history, transport before streets, ruins from history, one city
across simulation levels) and `docs/NEW/AXIOM_Living_World_Expansion_Prompt.txt`
Appendix C. Those chapters are design references under
[decision 0039](../../decisions/0039-axiom-expansion-briefs.md). This document
adds what they leave open: how the existing geology decides where and why places
exist, the seven anchor places the owner chose, how three views share one world, the
play loops that make it fun, and the first vertical slice.

## 1. Intent

Axiom is an explorable civilization game with editable terrain, dangerous magic,
and places worth discovering. Generate an inhabited region with substantial cities,
competing states, migration, trade and armies. Valleys, crossings, fertile land
and inherited infrastructure shape where people settle and what they contest.
Investment, skills, ownership and transport can keep a city important after its
founding industry disappears. Leave room for sparsely settled frontiers and places
that no state can reliably hold.

Composition follows the Hyper Light Drifter and Breaker direction: quiet fields
beneath impossible machinery, restrained colour punctuated by magical light, and
ancient systems that still perform duties nobody remembers (counterweights that
still move, flood gates that still hold, surfaces that still release stored light).
Every landmark has a recognizable silhouette and a purpose beyond scenery.
"Realistic" means believable relationships and usable spaces; distances may be
compressed and landmarks may be larger than life.

The player can spend the whole game traveling, building and experimenting.
A holding can also grow into a settlement that neighboring powers deal with.
Players who choose to govern can direct workers, command forces and contest
territory. A siege commanded from above, a gate breached on foot and a district
rebuilt after occupation all happen in the same place.

Reference mix (owner's list, translated into requirements, not copied content):

| Reference | What it contributes |
|---|---|
| Dwarf Fortress | Settlements and historical identities that persist across generations and styles of play |
| Songs of Syx | Productive cities at urban scale, with delegated work and administration |
| RimWorld | Memorable people, lasting relationships and injuries, and room to shape pacing |
| Total War | Regional diplomacy, terrain, supply, occupation and the cost of concentrating armies |
| Noita | Materials, spells and engineering that change situations wherever the player uses them |
| TABS | Ordered battles can erupt into physical and magical chaos |
| Titanfall | Immense infrastructure creates vertical routes and play spaces |
| Hyper Light Drifter, Breaker | Quiet landscapes holding damaged, half-understood power; restrained colour, concentrated light |

## 2. Goals, non-goals, anti-goals

Goals
- Places in the simulated region have causes the player can read from the land.
  Curated places can coexist; their authored backstory is not simulated history.
- Real cities from the start of the design: districts, transport, public space,
  infrastructure, institutions. Not a hamlet generator with a bigger count.
- A deliberate hierarchy: isolated holdings, villages, market towns, regional cities,
  rare capitals. Megastructures and large ruins are a separate distribution and never
  replace inhabited places.
- One authoritative record per settlement, district, building, resident and caravan,
  shared by first-person play, the overhead camera and the planetary map.
- History writes consequences other systems consume (routes, ownership, damage,
  population, opinions, loot), not paragraphs.
- Hills and Planet worlds both supported eventually; prove the loop on Hills first.

Non-goals (for this design)
- A full biography of every inhabitant. Cohorts plus a few named figures.
- Explaining the ultimate origin of the deepest strata. History covers who used a
  ruin recently; the thing underneath may stay a mystery.
- Mobile megastructures and multiplayer strategy balance. Later work. Working
  elevator stations are the bounded orbital extension in section 16.

Anti-goals (a failure even if it runs)
- Cities that are dungeons with facades: every door is usable, visibly blocked or
  explainably locked; ordinary homes, kitchens and workshops exist.
- History as flavour text with no physical evidence.
- A world where geology and civilization disagree (a port with no harbour, a pass
  city off the pass, a volcanic arc away from its mountain belt).
- Planning a city by generating its voxels. Plans are compact; voxels come on demand.
- Pop-in of landmarks. A megastructure's silhouette is visible from the horizon band
  as soon as the world loads.
- Brittle boom-bust cascades that delete a civilization over one missing input.

## 3. What exists (source audit, 2026-11-05)

See the [world simulation audit and execution plan](../../plans/2026-11-05-axiom-causal-world.md)
for source paths and limits across the world systems. The key distinction is
baseline generation, persistent local changes, and regional cause and effect.

- Hills uses a planar uplift, erosion and drainage solve. Planet generator
  142 and later builds a connected sphere; terrain and vents use the same
  plates, and drainage is solved across face edges. The older two-model
  contradiction is a legacy Planet issue. A regional view of the sphere is a
  compatibility projection, not a separate drainage graph.
- Existing village lanes, homesteads, Fernwake and local roads provide usable
  places and fixtures. Fernwake trade can already respond to an edited channel
  or workbench; its daily trade uses are not stock produced and shipped by a
  regional economy.
- No bounded history pass or ongoing regional settlement economy was found
  on the inspected paths. Fixed placement or authored events can seed a
  fixture; they do not prove that an economy produces settlement growth,
  rerouting, abandonment or ruins.

## 4. Geology made solid

### 4.1 Read the existing geography

Use the current planar solve on Hills and the spherical solve on supported
Planet generations. On Planet, expose the existing shared plate and receiver
data for planning when needed; do not derive an independent plate or drainage
model from the projected view. Use the stored generator version for older
worlds. Hills volcanism and legacy migration are separate projects.

### 4.2 Geologic ledger (deep time, computed once per seed)

A compact, derived record for later historical scenarios. Key it by world
recipe, generator and model revision. Reuse existing fields before adding
snapshots or a new cache. The first slice needs only present-day geography.

- Boundary class per region: convergent arc, collision belt, rift, stable craton,
  hotspot. From plate convergence and boundary distance.
- Paleo drainage: an earlier solver snapshot is only a candidate input. A
  different intermediate river course does not establish a dated river shift;
  the history model must supply a cause and ensure each course is valid on its
  own terrain. The planar erosion callback is not a Planet history API.
- Lake history: spill levels and outlets from `Flow::spill`. A lake whose outlet
  passes a steep gorge or a lava-prone flank is a candidate for a dammed flood.
- Hazard recurrence by cause: active vent kind and size (caldera rare and large,
  cinder frequent and small), quake likelihood by boundary distance, flood by
  discharge and floodplain width, landslide by slope on weak beds.
- Resource formation: which ore bodies exist within reach of each candidate site,
  reusing `ore_geology::deposit`, `placer`, `deep` rather than a second ore model.

### 4.3 Suitability fields

Sampled at the scale of the regional plan using the active generator:

| Field | Source |
|---|---|
| Buildable land | `Ground.slope`, permafrost, lake and channel masks |
| Fresh water | channel size from `rivers::nearest`, lake level |
| Flood risk | upstream area, floodplain width (gate 136), spill depth |
| Food hinterland | `Ground.moisture`, `maat_c`, `sediment`, season |
| Defensibility | local relief, river meander necks, spurs, single approaches |
| Pass value | route-graph betweenness across an uplift belt (4.4) |
| Harbour | coast with shelter and shelf depth (gate 135 on Planet) |
| Ore access | deposits within haul distance and their grade |
| Volcanic soil, geothermal | distance and kind of vents, active or not |
| Hazard | 4.2 recurrence fields |

### 4.4 Route graph

A cost-distance graph on the solve grid: slope, wet ground, river crossings (cost by
channel size, with bridge sites at narrow, firm banks), passes, coast. Betweenness
over pairs of good sites marks where traffic would naturally flow. High-betweenness
saddles become pass-city candidates; high-betweenness river crossings become bridge
towns. Regional roads and waterways are drawn along these paths before any street
exists.

## 5. The seven anchor places

The owner chose these as the world's backbone. Descriptions below are design
references; their in-game names are not approved yet. Each gets an exploration verb,
a battle verb, a governance verb and a geological precondition the generator checks.

| Place | Geological precondition | Authored premise and later occupation | Explore / battle / govern | Frequency |
|---|---|---|---|---|
| City in the broken sky elevator | Lower anchor on a stable craton (low quake field) with a river through its footprint; the severed tether lies across an uplift belt | Orbital collapse, then a surface settlement grows around the surviving terminal | Climb service routes and counterweight shafts / fight for lifts and bridges, aerial units bypass walls / restore transit and water, decide whether to restart the anchor | One per world or planet |
| Drowned capital behind working flood gates | Rift basin lake whose outlet is damable (gorge, lava flank, landslide slope) | Capital founded on the basin floor; outlet blocked by a recorded event; survivors build on rooftops and slopes | Restore pumping stations to expose readable streets / approach by drained avenues or maintenance tunnels / choose where the water goes, downstream farms flood unless channels are cut | Rare; one per large basin |
| Pass city | Top betweenness saddle on a collision belt with a river crossing | Bridge and tollhouse, then caravan yards and walls; successive walls survive inside the fabric | Several approaches: trade road, hillside stairs, river route, smuggling passages / destroying the bridge wins a battle and ruins the prize / toll, garrison, trade | One per major belt |
| Glass orchard | Former industrial city near crystal deposits and water power; contamination follows its buried conduit network | Magitech manufacturing city; a catastrophe changed how its infrastructure stores light | Districts with distinct hazards and salvage / battles change at sunset as old barriers wake / restoring a grid segment powers the settlement and wakes defences elsewhere | Rare |
| Walking fortress turned town | Wide valley floor on a historical war route | A walker halted during a recorded war; the town grew along supply roads | Legs as gatehouses, market underneath, sealed decks / defend or take a fortress that is also a weapon / limited restoration first: lights, pumps, a freight lift, one gun | Rare |
| Three cities underneath one another | Long-settled site whose river moved (paleo drainage differs from present) | Each civilization built to its own river and geometry; the deepest layer follows a geometry later builders never understood | Cellars break into older avenues; wells reach sealed stations / underground routes enable raids and invasion / excavation changes the surface city: trade, subsidence, salvage, disease | One per world near a major river |
| City built around routine disaster | Fertile volcanic soil or geothermal field near an active vent | Institutions shaped by periodic eruptions: evacuation avenues, ash shelters, sacrificial outer farms | Refuge tunnels and abandoned districts / war blocks evacuation / maintain defences or neglect them | One or two per volcanic arc |

Identity of each place (owner's direction):

| Place | Silhouette from afar | Duty still running | Dispute | Palette and layout notes |
|---|---|---|---|---|
| Sky elevator city | A luminous broken spine above a crowded, terraced city | Counterweights in the foundations still move | Restart the anchor or leave it dead | Wealthy on stable upper platforms, workshops in freight galleries, oldest quarters around the foundations; the river runs through the coolant works, and whoever holds them holds water and power |
| Drowned capital | Immense hydraulic gates across a basin, rooftops above water | Some pumping stations still run | Restore, scavenge, or keep sealed | Submerged plazas, tram tunnels, workshops and walls on a readable street plan, never a random scatter |
| Pass city | A steep city wrapped around one crossing, nested walls | Tolls and the bridge itself | Who holds the crossing | Old gatehouses turned into shops; a newer fortress watches an older one that fell in a previous war |
| Glass orchard | Broken towers terraced with growth among pale trees and translucent slag | Surfaces store daylight and release it after sunset along fragments of the old streets | Salvage rules at the safe edge | Quiet green growth, chalk-white ruins, dark interiors, occasional concentrated cyan or violet light; most surfaces do not glow |
| Walking fortress town | A colossal walker standing in a valley, town spread along its supply roads | Sealed military compartments, one restorable weapon | Engineers want power restored, merchants fear losing the market, a neighbouring faction claims it as military property | Local buildings borrow its proportions and salvaged materials |
| Three stacked cities | An ordinary surface city; the clue is underground | The deepest network still routes something | Excavate or seal | Each layer has its own construction methods, materials, infrastructure and failure history, recognisable without a tooltip |
| Routine-disaster city | A city on dark fertile slopes below an active cone | Evacuation avenues, ash shelters, refuge tunnels kept in order | Spend on defences or on growth | Sacrificial outer farms, movable market stalls; eruptions give warning (quakes, gas, ash) and mitigation works, so no eruption is a scripted apocalypse |

Where the three stacked cities meet, layouts intersect imperfectly and each
intersection is generated on purpose: a cellar breaks into an ancient avenue, a
municipal well reaches a sealed transit station, a fortress foundation rests on a
buried machine hall, a modern sewer reconnects a dangerous older network.
Excavation feeds the surface settlement: new routes enable trade, invasion,
subsidence, salvage or disease.

The three stacked civilizations use the strata already in the design bible
(Engineers, Choristers, Founders). Surface peoples are those already in the bible
(Salvagers, Cantors, Archivists, the Crown). No new factions without owner approval.

Each anchor has a distinct silhouette readable at horizon distance, and its far
silhouette is part of the horizon band from load (no pop-in).

## 6. Settlement hierarchy and distribution

Central-place spacing on the route graph, scaled to face size. Starting budgets (my
call; tune with playtests):

| Face | Capitals | Regional cities | Market towns | Villages | Holdings | Anchor places |
|---|---|---|---|---|---|---|
| Hills `SETTLED_HOME` 6.1 km | 0 | 1 | 2-3 | 8-15 | many | 1-2 |
| Hills `EXPLORATION_HOME` 10.2 km | 0-1 | 1-2 | 4-6 | 15-30 | many | 2-3 |
| Planet face 48 km | 1 | 4-8 | 15-30 | 60-150 | many | most of the seven |

Rules
- Every settlement has a site record: founding reason, water source, hinterland,
  economic function, connections, usable land, hazards, relationships (brief 4.2).
- Purposes are compromises, not one score: hill fort trades freight access for
  defence; river market trades flood risk for transport and soil.
- Spawn is within walking reach of one village, a route and a visible landmark; the
  starter workshop becomes a holding on that route, which answers "why is there a
  house".

## 7. Ancient setting and recent history

Author ancient technologies, construction traditions, catastrophes, strange
machinery and memorable layouts. Place them where their physical premises fit.
A ruin can have authored encounters and puzzles, then acquire a recent history
of salvage, occupation, repairs or failed excavations. Physical evidence and
observable mechanisms are reliable. Residents can disagree about who built
them and why, and the deepest places can remain unexplained.

Seed a settlement network from the terrain, routes, older works and existing
communities. Run a bounded recent history of growth, trade, migration, conquest,
changes of government, abandonment and rebuilding. Use the same rules after
play begins for the changes that affect the player. Record changes to districts,
roads, stocks, households and authority where they occur. Stop at a playable
present with open tensions and enough time for ordinary life.

Use households, workplaces and neighborhoods for routine decisions at city
scale. Companions, recruits, rivals and recurring contacts retain skills,
injuries, relationships, loyalties and obligations. Lasting attention follows
the player's dealings with people. Cultural foundations shape land ownership,
succession, service, recruitment, treatment of outsiders, building traditions,
and access to magic and infrastructure. States can share a cultural origin
and develop different institutions. Cities can have mixed affiliations.

Acceptance fixture (owner's example): a copper town depends on a bridge in a pass; a
war destroys it; trade shifts downstream; the copper town contracts and its outer
district is abandoned; smugglers use the old road; the downstream city opposes
rebuilding. Result in the world: two settlements with different fortunes, a broken
bridge, an abandoned district, a surviving smuggling route, NPC opinions, loot tied
to former occupations, and a player project (rebuild the bridge) that changes the
regional economy.

## 8. Cities

Follow brief chapter 4 for the pipeline:

- Plan the city's hinterland with it. Farms, villages, quarries, workshops and
  freight routes supply the city and give an approaching player a readable
  progression from fields and roadside works to warehouses and dense streets.
- Lay out entrances, main streets, waterfronts, elevation changes, walls, public
  spaces and institutions. Fit mixed districts and connections inside that
  structure, then lay out parcels, yards, alleys, stairs, bridges and interiors.
- Give expanding, planned, cramped and terrain-bound cities different forms.
  Use ordinary homes, kitchens and workshops alongside exceptional structures.
- Voxels come from a pure per-chunk lookup into the plan (the `authored_block` pattern
  behind gate 127). Plan queries must not re-enter site, homestead, dungeon or
  undercrown caches.
- Growth adds homes, workplaces, storage and services. Later roads can cut
  through older blocks, workshops can absorb their neighbors, and damaged
  districts can remain partly vacant. Earlier walls and old cores can survive
  inside the expanded city.

## 9. Three views, one world

| View | Exists | Needs |
|---|---|---|
| First person | yes | entering any building, district or battle from the other views |
| Overhead city and battle camera | no | a camera mode over the same resident chunks; selection, orders, construction placement; units are the same entities first person sees |
| Planetary strategy map | partly (map, loading globe) | settlement, route, territory and army layers drawn from the compact records |

Runtime state (ownership, damage, population, stock, armies) needs versioned
persistence over a generated baseline, with explicit compatibility behavior.
Start by deferring observations of unavailable edited terrain. Far settlement
updates use persisted route and settlement state with one owner per stock,
caravan or resident. Entering a local view brings the same people, position,
equipment and damage into detail. Raising the camera respects the player's
knowledge and command authority.

Each army has a home, roster, equipment, supplies and destination. States raise
and replenish forces from available people, stock and time. They pursue reachable
objectives such as crossings, productive districts or defensible territory.
Occupation, access and the ability to enforce authority determine control.
Retreat, exhaustion, threatened supply, concessions and negotiated settlements
can end wars. Reserves, alternate routes and repairs give a city ways to absorb
local setbacks.

## 10. What makes it fun

Loops the player feels:

1. Read the land. A silhouette on the horizon, a road that leads toward it, evidence
   before contact (smoke, a broken bridge, a waterline on walls), then a choice of
   approach.
2. Inherit and repair. Partial repair gives partial benefit at once: a fixed bridge
   reroutes caravans, a reopened cistern supplies a district, a restored lift opens a
   precinct.
3. Take over. Claim a holding, attract settlers through hospitality, work or rescue,
   grow it, then contest a town. Ownership, damage and daily life persist after.
4. Command and descend. Order a siege from above, drop into first person for the
   breach, walk the streets afterwards.
5. Change the situation with ordinary tools. Reinforce a position, burn supplies,
   dig a connected passage, disable machinery, flood an approach or build a
   bypass. Regional decisions consume the resulting route, stock or structure
   changes. Deep hazards can keep even prosperous states from occupying a site.

A pacing director may surface a rumor, a plausible opportunity or an existing
threat. Traders carry real goods, raids draw from an available force, and news
describes a conflict that exists. Leave quiet intervals for building, trading,
travel and preparation.

Give a new player a recognizable home, a short trip with a meaningful route
choice, and a longer question raised by physical evidence. The workshop compass
introduces the home. Further project tracking is voluntary. Residents offer
directions to things they know, and the personal journal keeps their leads
and any landmark the player chooses to pin. A safe route, a useful shortcut and
a route with salvage can serve different purposes. Ordinary supplies and a
workable first home should be attainable through local trade, salvage or production.

Give settlements recognizable routines with visible outcomes. Show goods
moving on an ordinary day so their absence means something later. Let players
join a shipment, repair a crossing, investigate a lower route or spend the day
building. A neglected opportunity can change with the world. Show the local
effect and leave a way to re-enter the situation after time away.

Fun tests (checked by playtest, not only by code):
- Within 10 minutes of spawn a landmark silhouette, a road and an inhabited place are
  visible.
- Major ruins offer something observable, discoverable or operable without
  explaining their original purpose.
- Every city has at least two meaningfully different approaches.
- The first city is reachable on foot in under 15 minutes from spawn on Hills.
- Danger reads from history: bandits on abandoned roads, guards in held towns,
  aberrations in deep strata. Filler spawns thin out inside held territory.
- Players who follow the opening compass, untrack it, or spend the session
  building can each name a next step and find their way back. On return, they
  can identify one change and explain why it matters.

## 11. Performance

- Prepare history, suitability and routes outside chunk requests. Key any
  derived cache by recipe and model revision as well as seed and generator.
- Index plans by chunk so generation visits only intersecting records. Measure
  prepare latency, memory, chunk generation and runtime backlog; historical
  timings are not a budget or acceptance result for this work.
- Detailed buildings and interiors are generated when their region becomes resident.
  Planning a large city never loads its voxels.
- City and anchor silhouettes come from plan records into the horizon band, so they
  are present at load and do not pop in.

## 12. First vertical slice

Build one functioning Hills province with a substantial city, a rival state
with a defensible settlement, supporting villages and production sites, a primary
and alternate route, older infrastructure and a dangerous underground area.
Generate a recent change that is visible on foot and continue using the same
records once play starts. A player can travel, recruit, trade, build an outpost,
interfere with a route and take part in a conflict. Overhead orders operate on
the people and places encountered on foot.

Use the existing nursery and the bridge fixture to prove production, stock,
transport and repair during development. The province is the first integrated
playable milestone. See the [province execution plan and acceptance checks](../../plans/2026-11-05-axiom-causal-world.md#next-regional-milestone-a-working-province).
Expand to planetary strategy, seam-crossing routes, wider settlement distribution
and interplanetary logistics from the same settlement and transport rules.

## 13. Constraints

- Worldgen stays a pure function of `WorldSampleConfig`. Every behaviour change goes
  behind a new generator gate claimed in `docs/worldgen/GENERATOR_GATES.md` first.
  Gates move quickly (136 was taken on 2026-11-05); claim at plan time and grep all
  branches. Never edit historical gates.
- Version new persistent state and define compatibility; change the main
  save version only if the selected storage path requires it.
- Bound simulation by logical work counts. Wall-clock budget may yield to the
  next service pass; it must not select different committed outcomes.
- Randomness is positional hashing from the world seed; no `rand`.
- Content rows (building families, event types, district kinds) live in
  `assets/*.ron`, not Rust tables.
- No new dependency without an ADR line.
- No visual change without before and after captures.
- No new proper nouns or factions without owner approval.

## 14. Left to the builder

- Data layout of the records and the cache format.
- Street layout technique (directional fields, contour following, grids per district).
- Exact epoch and event counts within the budget.
- The overhead camera's controls and rendering approach.
- Choice of Hills province, its catchment and the later Planet seam fixture.

## 15. Calls made without owner input

- Read the active Planet generator's existing plate model (4.1).
- Settlement budgets per face size (section 6).
- The copper-town example is a component fixture for recent history and routes.
- Spawn sits within walking reach of a village and a route; the starter workshop
  becomes a holding on that route.
- Spawn stays fixed; the regional network bends around it.
- Existing places can seed the first fixture without changing their layout.
  Later generation can model how new sites are founded and abandoned.
  Preserve named residents and historical gates; require a new gate for any
  replacement of baseline sites.
- The copper-town fixture may set its preconditions (where the war happens); every
  consequence after that (rerouted trade, contraction, smugglers, opinions) must come
  from the general event rules.
- Target reproducible history within a declared generator and model version,
  with a checksum as a divergence check. Persist live outcomes under the
  host's authority when co-op is added. A baseline checksum does not prove
  live peer agreement, recovery or replay.
- The three stacked cities compress into the existing column (floor at -64 m); the
  bible's 200 to 1500 m strata depths are not literal.

## 16. Working elevators and inhabited stations

Owner extension: several working personnel elevators, stations with loot, enemies,
friends, neutral residents and traders, and authored human models replacing the
villager placeholders. The broken elevator city remains a distinct damaged site;
it does not imply every other elevator failed. No new faction or place name is
assigned here.

The implementation design and source audit are in
[Station voxel interiors, pressure and access](2026-11-05-station-voxel-interiors-design.md).
It covers real metre clearances, station-local voxel volumes above the terrain
column, lift connections, pressure compartments, breaching, airlocks, environmental
capabilities, persistent residents and trade stock. These are proposed additions,
not a claim that editable station interiors or pressure simulation already run.

Stations share the settlement identities and runtime deltas in sections 7–9.
Their districts, ownership, damage, population and stock remain the same records
when approached, boarded, unloaded or viewed on the map. Physical location is a
station frame rather than a surface parcel. Prove one inhabited, repairable station
and a second independent lift destination before expanding station distribution.
