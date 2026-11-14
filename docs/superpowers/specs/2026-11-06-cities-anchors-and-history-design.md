# Cities, anchors and history: three views of one world

Status: design, 2026-11-06. Owner direction: first-person adventuring, an overhead
city-building and battle camera, and a planetary strategy map, all controlling **one world**;
real cities from the beginning of the design; geography made legible through history.

Extends, and must agree with:

- [Axiom: persistent civilization simulation](2026-11-06-axiom-direction-persistent-civilization-simulation.md)
  (persistent consequences, conserved economy, needs -> plans -> work, shared policy).
- [Conquest, politics and the playstyle paths](2026-11-06-conquest-politics-and-playstyle-paths-design.md)
  (the strategy surface).
- [Walls, sieges, epochs and RTS inspiration](2026-11-06-walls-sieges-ages-and-rts-inspiration-design.md).
- [Civilization on solved geography](2026-11-05-civilization-on-solved-geography-design.md).

Reuse words: `L` = legacy substrate exists, `P` = design only.

## 0. Corrections to the brief, from the source

- **Generator 136 is taken.** `crates/worldgen/src/geo/corridor.rs` defines `VERSION = 136`
  (floodplains, valley walls, riparian band). The current ceiling is
  `CURATED_SURFACE_GENERATOR = 240` in `crates/voxel/src/config.rs`, and **anchor generators
  already exist**: `ANCHOR_CITY_GENERATOR = 190`, `ANCHOR_RUIN_ROUTE_GENERATOR = 200`,
  `ANCHOR_TERRACE_GENERATOR = 216`, `ANCHOR_EXPLORATION_GENERATOR = 229`. A new city generator
  claims the **next free number above the then-current ceiling** (>= 241 today), checked at plan
  time, not 136.
- **More of the pipeline exists than the brief assumes.** Streets and lots already generate for
  urban ruins (`crates/worldgen/src/ruins/urban.rs`), a market settlement with plots, a square
  and a service circuit exists (`crates/worldgen/src/valley/settlement.rs`), and anchors are
  already placed by generator. Build on these; do not start from zero.

## 1. Three views, one world

The same records are read and written from three cameras:

| View | What it is | What it commands |
|---|---|---|
| First person | the existing player | movement, combat, placement, interaction, possession |
| Overhead city and battle | a pitched, tiltable camera over a settlement or a field | units, divisions, build orders, siege equipment, stances |
| Planetary strategy map | the region and the globe | routes, settlements, ownership, diplomacy, war |

You can **command a siege from above, return to your character to breach the gate, and later
walk through the occupied city as its residents rebuild** - because there is one authoritative
world and the cameras are views, not modes. Command data is the same order data the world brain
emits, so player and AI run one system (per the conquest design).

**F1 still governs**: no camera is forced, and closing a view costs nothing.

## 2. Real cities from the beginning

Building only hamlets now, and assuming cities will emerge by raising a building-count
parameter, lays the wrong foundations. A city needs **districts, transport, public spaces,
infrastructure and institutions**, so those are first-class from the start.

**The generator produces a functioning spatial plan before any geometry:**

| Layer | Determines |
|---|---|
| Geography | water, gradients, flood risk, resources, coast access, passes |
| Settlement purpose | trade crossing, administrative centre, mine, port, fortress, religious site |
| Regional connections | roads, waterways, supply routes, neighbouring settlements |
| Growth history | founding core, expansion, walls, rebuilding, abandoned districts |
| Districts | housing, markets, workshops, civic, industry, military |
| Streets and parcels | access, frontage, intersections, courtyards, service routes, plots |
| Buildings and interiors | style, entrances, stairs, rooms, storage, occupation |
| Inhabitants and property | households, shops, workplaces, loyalties, inventories, ownership |

- **Architecture and street layout vary independently.** One culture builds a planned colonial
  city and a winding ancient capital. A conquered city keeps its streets while gaining new
  roofs, shrines and government buildings.
- **"Realistic" means believable relationships and usable spaces**, with compressed distances
  and unusually striking landmarks allowed. A literal metropolis is not required to *feel* like
  a major city.
- **A deliberate hierarchy** (isolated holding, village, market town, regional city, rare
  capital). Large ruins and megastructures are a **separate distribution** and never replace
  inhabited places.
- **Ordinary life is required.** A memorable skyline is not convincing if every building is a
  dungeon and nobody has a kitchen, workshop, courtyard or a place to sleep.

## 3. History must produce evidence and gameplay

A historical event writes consequences other systems consume, not a paragraph of prose:

> A copper town depended on a bridge through the mountain pass. A war destroyed the crossing.
> Trade shifted to a downstream settlement. The copper town contracted, leaving its outer
> district abandoned. Smugglers now use the old road; the downstream city opposes rebuilding
> the bridge.

That yields two settlements with different fortunes, a broken bridge, an abandoned district, a
surviving trade route, relevant opinions and faction interests, occupation-linked loot, and a
**player project that changes the regional economy**. Far more useful than a text that a war
occurred.

**Keep room for authored mysteries.** The simulation explains who occupied a ruin recently
without explaining the ultimate origin of the thing underneath it.

## 4. The seven places to build the world around

Each landmark gets a recognizable silhouette and a purpose beyond scenery. Composition follows
the Hyper Light Drifter / Breaker direction: quiet fields under impossible machinery, restrained
colour punctuated by concentrated magical light, ancient systems still doing duties nobody
remembers.

| # | Place (silhouette) | Cause and geometry | Play |
|---|---|---|---|
| 1 | City inside the broken sky elevator (a luminous broken spine above a crowded terraced city) | A severed orbital tether; the lower anchor supports a city: wealthy upper platforms, freight-gallery workshops, oldest neighbourhoods at the foundations. A river passes the original coolant works, which control water and much of the power. | Climb service routes, descend counterweight shafts, recover sealed freight; fight for lifts, bridges and machinery (aerial units bypass some defences); restore transit and water while deciding whether restarting the anchor is worth the risk. |
| 2 | Drowned capital whose flood defences still work | A basin behind immense hydraulic gates; a historic flood submerged the lower districts and a town grew on the rooftops and slopes. Some pumping stations still run. The underwater city keeps a readable street plan. | Restoring a station exposes districts and yields land and salvage but moves water elsewhere; draining may flood downstream farmland unless channels are cut; armies approach through exposed avenues or maintenance tunnels; factions dispute restore, scavenge or seal. |
| 3 | Pass city across the easiest continental route | Bridge and tollhouse became caravan yards, inns, warehouses and walls; a steep city wrapped the crossing. Successive walls survive in the fabric, old gatehouses are shops, a newer fortress watches an older one. | Wealth is traffic, so destroying the bridge wins a battle and ruins the prize. The same place plays differently for a small party and a large army: trade road, hillside stairs, river route, smuggling passages. |
| 4 | The glass orchard | A magitech manufacturing city among pale trees and translucent slag; broken towers became terraces. Its catastrophe changed how infrastructure stores energy, so certain surfaces gather daylight and release it after sunset, lighting fragments of the old street grid. | A living settlement on the safe edge cultivates useful plants and salvages under strict rules. Districts differ in hazard and salvage; restoring a grid segment powers your settlement while waking defences elsewhere; battles change after sunset. Visual identity: quiet green, chalk-white ruins, dark interiors, occasional cyan or violet. |
| 5 | The walking fortress that became a town | A colossal military walker stopped centuries ago: legs became gatehouses, the underside shelters a market, the hull holds inhabited decks and sealed compartments. The town grew along its supply roads and borrows its materials. | Inhabitants disagree about its future (engineers want the power systems; merchants fear for the market; a neighbour claims it as ancestral property). The first restoration goal is limited and useful (lighting, pumps, a freight lift, one defensive weapon); making it mobile stays a late capability. |
| 6 | Three cities underneath one another | Surface streets, an older city on a different river course, and deeper monumental geometry the later inhabitants never understood, intersecting imperfectly: a cellar opens into an avenue, a well reaches a sealed transit station, a fortress foundation rests on a machine hall, a new sewer reconnects a dangerous older network. | Each layer gets its own methods, materials, infrastructure and failure history, recognizable without a tooltip. Excavation changes the surface settlement, enabling trade, invasion, subsidence, salvage or disease, so underground exploration stays relevant to city management. |
| 7 | The city preparing for a routine disaster | Rich volcanic ground or a geothermal field, with institutions built around periodic danger: evacuation avenues, ash shelters, sacrificial outer farms, movable market stalls, maintained refuge tunnels. Citizens are practical, not oblivious. | A past eruption explains an abandoned district; a future one tests the infrastructure you inherit. This connects the existing Planet volcanism to civilization without making every volcano a scripted apocalypse: a plausible risk model with warning and mitigation. |

Authored anchors coexist with simulated places: their backstory need not be simulated history,
but their **current function and consequences** must be.

## 5. Revised implementation order

A complete history simulator must not be a prerequisite for proving city generation or
conquest. History needs settlement and economic rules to simulate, and settlement generation
needs historical outputs, so **their shared data model is developed together**.

Generator number: **not 136** (taken). Claim the next free number above the current ceiling
(`CURATED_SURFACE_GENERATOR = 240` today), verified at plan time.

1. **Geographic suitability and route graph** from the existing rivers, passes, slopes,
   coastlines and resources.
2. **A regional network**: villages, towns and at least one genuine city footprint; Planet
   worlds supported explicitly.
3. **One city's districts, streets, parcels and usable buildings.**
4. **A bounded historical pass** that changes settlement importance and records a few
   consequential events.
5. **Materialize those consequences**: a ruined district, a diverted road, a rebuilt gate, an
   abandoned mine.
6. **Prove one living district**: residents, guards, shops, stock ownership and a basic
   production chain.
7. **Tactical command and one settlement takeover**, then verify that ownership, damage and
   daily life persist.

The first demonstration is **one inhabited city, one historically related ruin and a route
connecting them** - stronger evidence than hundreds of disconnected hamlets. The 1.6 s erosion
solve is an architectural opportunity, **not a history-generation estimate**: keep history
bounded, cache its compact outputs, and generate detailed buildings and interiors only as
regions load. Planning a large city must not require loading all its voxels.

## 6. What to build on (audited 2026-11-06)

| Need | Existing substrate |
|---|---|
| Street grids and building lots at city scale | `crates/worldgen/src/ruins/urban.rs` (`AVENUE`, `CITY_HALF`, `CITY_X`, `CITY_Z`, `CROSS_AVENUE`, `CITY_PIERS`, ramps) |
| A market settlement with plots, a square, a service circuit | `crates/worldgen/src/valley/settlement.rs` (+ `craft`, `workshop`, `hall`, `facade`, `residence`) |
| Anchors already placed by generator | `ANCHOR_CITY_GENERATOR = 190`, `ANCHOR_RUIN_ROUTE_GENERATOR = 200`, `ANCHOR_TERRACE_GENERATOR = 216`, `ANCHOR_EXPLORATION_GENERATOR = 229` in `crates/voxel/src/config.rs` |
| Districts (interior maze sense), buried plans | `crates/worldgen/src/manyfold/districts.rs`, `crates/worldgen/src/manyfold/weave.rs`, `crates/worldgen/src/undercrown/plan.rs` |
| Ruins and their sampling | `crates/worldgen/src/ruins.rs`, `crates/worldgen/src/ruins/sample.rs` |
| Households, parcels and residents | `crates/worldgen/src/households.rs`, `crates/worldgen/src/households/settlement.rs`, `crates/app/src/world/travel/residents.rs` |
| Volcanism and hazards | `crates/app/src/world/volcanism.rs`, `crates/worldgen/src/volcano.rs` |
| Three buried strata (Engineers, Choristers, Founders) | `docs/NEW/UNDERKINGDOM_COMBINED_SPEC.md` section 3 |
| Route graph, drainage, passes | `crates/worldgen/src/drainage.rs`, `crates/worldgen/src/valley.rs`, `crates/worldgen/src/geo/corridor.rs` |

The gap: a **city-scale plan** (purpose, districts, streets, parcels, usable buildings,
inhabitants) exists only piecewise, and **history writes no consequences**. Streets exist for
ruins, plots for one market settlement; nothing composes a living city with districts and
institutions, and nothing turns a past event into a broken bridge and an abandoned district.

## 7. Risks and open questions

- **Scope**: cities from the beginning is right, but the first proof is *one* city, one ruin and
  a route. Resist generating a continent of cities before one is convincing.
- **Cost**: planning must not require loading voxels; history must be bounded and cached
  (section 5). The 1.6 s erosion figure is not a history budget.
- **Determinism and saves**: a new generator gate must not alter historical generation, and new
  persisted records need versioning. Read `crates/app/src/world/storage.rs` and
  `crates/voxel/src/config.rs` before choosing.
- **Naming**: the seven anchors, and any new faction or place, need owner approval before they
  are named.
- **Open**: which anchor is the first to implement (the pass city reads as the cheapest, the
  drowned capital as the strongest single demonstration); how much authored backstory is stored
  versus simulated; whether the overhead camera is one camera with two ranges or two cameras.

## 8. Direction to the coding agent

> Design for cities and civilizations now; implement one coherent region first. First-person
> exploration remains central, with seamless tactical command and planetary management.
> Geography determines opportunities, history changes them, settlements make them physical, and
> the player can alter their future. Large inhabited cities, layered urban ruins and rare
> monumental infrastructure are core world content - not deferred decorations.

## 9. Urban life: churn, routine and the home (extends section 2)

Cities are lifelike and unique: procedural but never templated, organized in a **chaotic** way
and governed by **efficiency**. Buildings are torn down and others are put up. People optimise
their routine but settle into a home they come to like and customise it, and the player can read
a resident's name, style, decorations and motivations.

**Churn (rebuild).**

- Every building carries age, upkeep and efficiency. Low efficiency is the signal to replace it.
- A replacement happens **on the same lot**: the building is torn down and a new one raised, so
  the street plan persists while the fabric changes. Successive walls, shopfronts in old
  gatehouses and rebuilt terraces fall out of this one rule.
- **Efficiency governs; chaos remains.** The decision is efficiency, but the outcome is not a
  clean grid: growth history, held lots and seeded variation leave irregular infill, gaps and
  buildings that outlive their era.
- **Occupied homes resist.** A settled household is attachment, not a number: it moves only when
  the gain clearly beats the cost of leaving, so teardown spares occupied homes unless they are
  genuinely failing.

**Routine.**

- A routine (work, market, home) with a commute and a satisfaction. Households press to improve
  it, but optimise **locally**: they do not re-plan the city.

**The home and the person.**

- A household settles into a home and attaches to it; the longer it stays, the less a marginal
  gain moves it.
- It **customises** the home with decorations in its own style, and the player can see the
  decorations, the style, the household's name and its motivation (security, wealth, family,
  faith, craft, status) - and read why it acts as it does.

| Concept | Model | Rule |
|---|---|---|
| A building's condition | churn `Hob` (age, upkeep, efficiency) | ages, loses efficiency, is the teardown signal |
| A plot of ground | churn `Lot` (district, street, hob, held) | persists across rebuilds |
| The settlement | churn `City` (`grow`, `redevelop`, `efficiency`) | accretes and redevelops, seeded |
| A resident group | household `Household` (motivation, style, home, decorations, attachment, routine) | settles, customises, considers moving |
| What the player reads | household `Profile` | name seed, motivation, style, decorations, commute |

Naming stays a content decision: a household keeps a `name_seed`, and a content table maps seeds
to names. No proper noun is invented here.
