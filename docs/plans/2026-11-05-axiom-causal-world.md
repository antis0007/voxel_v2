# Axiom: from solved geography to persistent consequences

2026-11-05 · Source audit of the working tree and execution plan for the
regional world track. This does not record a build, playtest, performance pass,
or multiplayer acceptance. See the [civilization design](../superpowers/specs/2026-11-05-civilization-on-solved-geography-design.md)
for the longer-term cities and anchors.

Axiom has meaningful generation-time simulation and several persistent runtime
loops. The regional milestone connects them in an inhabited province where a
player can travel, build, trade, recruit, explore and take part in a conflict.
Use authored ancient sites and initial conditions alongside simulated recent
history. The current source baseline is below.

| System | What the source supports | Boundary of the claim |
| --- | --- | --- |
| Geology and drainage | Planar uplift and erosion; Planet 142+ builds a spherical receiver network, uses shared plate data for relief and vents, and routes runoff after shaping terrain. [Planar solve](../../crates/worldgen/src/geo/planet.rs) · [Spherical solve](../../crates/worldgen/src/planetary/mod.rs) | These establish baseline geography. Neither ordinary voxel edits nor local volcano effects establish a new authoritative regional receiver graph or a long-running sediment model. |
| Ores and extraction | Geology-conditioned deposit placement and in-world mining/extraction. [Ore geology](../../crates/worldgen/src/ore_geology.rs) · [Extraction](../../crates/app/src/world/factory/extraction.rs) | Removing a vein's blocks is already local depletion. Regional reserves, labor, haul costs and scarcity feeding supply are separate work. |
| Water and materials | Generated river stock is installed once; saved edits win. Runtime water has bounded voxel behavior and shared transfer paths, including soil water exchange. Ordinary play and isolated cellular regions have distinct authority paths. [River stock](../../crates/app/src/world/river_water.rs) · [Soil](../../crates/app/src/simulation/environment/soil.rs) · [Fixed step](../../crates/app/src/simulation/fixed.rs) | Local transfers do not establish a catchment-wide balance after edits. No blanket conservation or CPU/GPU equivalence claim follows. |
| Climate and weather | Generated climate informs runtime ambient temperature with depth, season, day and cloud effects. The storm path includes local snow cover changes and lightning effects. [Ambient climate](../../crates/app/src/simulation/climate.rs) · [Storms](../../crates/app/src/simulation/storms.rs) | Local weather effects do not establish a persistent feedback loop from land use, moisture or flooding to regional weather. |
| Ecology | Bounded local wildlife identities, behavior and husbandry; persisted crop plots and farming clock. Crop growth samples local soil, water and light. Soil patches can colonize and change cover as biomass changes. [Wildlife](../../crates/app/src/world/adventure/wildlife.rs) · [Farming](../../crates/app/src/world/farming.rs) · [Soil succession](../../crates/app/src/simulation/environment/soil.rs) | No regional food web, population migration or succession model was established by these paths. Plot output is not automatically a settlement food supply. |
| Settlement and history | Placed or authored sites, local road and parcel geometry, named residents and daily trade uses. Fernwake has harvest-backed stock and a shipment that reads the editable bridge. [Settlement parcels](../../crates/worldgen/src/households/settlement.rs) · [Trade rules](../../crates/app/src/world/travel/trades.rs) · [Freight](../../crates/app/src/world/farming/freight.rs) | The freight loop covers one district crossing. A province-wide economy, competing states and a recent-history pass remain open. |
| Industry and magic | Factory power, recipes and extraction run in ordinary gameplay. Existing world effects and water transfer offer scoped mutation paths. [Factory power](../../crates/app/src/world/factory/power.rs) · [World effects](../../crates/app/src/world/effects.rs) · [Water transfers](../../crates/app/src/world/effects/water.rs) | Existing costs, stocks and edits matter; inspect each operation's contract. A shared operation for this slice does not require a universal energy or spell rewrite. |
| Space | Body-relative frames, gravity and foreign-body paths are in source; ordinary gameplay also steps orbital travel/cargo paths. [Body influence](../../crates/app/src/planet/influence.rs) · [Fixed step](../../crates/app/src/simulation/fixed.rs) | These do not prove shared interplanetary stock, colonization history or cross-body causal persistence. Planet count is a scale choice. |
| Multiplayer | Gameplay has an authority path and explicit replication; water already has a replication module. [Authority](../../crates/app/src/multiplayer/authority.rs) · [Water replication](../../crates/app/src/multiplayer/replication/water.rs) | A generated terrain checksum, a replicated edit, and a replay of a declared input stream establish different things. Do not require global peer lockstep or claim deterministic whole-world replay. |

## Next regional milestone: a working province

### Regional continuity increment, 2026-11-06

The Planet freight site links Westbank Market to Eastbank Farms. Generator 243 adds a west-bank dwelling and a separate service crossing. The freight record owns the holding, cargo, route, progress and driver wounds. A nearby driver materializes as a talkable, damageable actor with the same identity. Damage persists, and wounds can send the convoy back along its route or strand the cargo if the driver dies. Saved route cells can be checked without admitting those chunks to the active world. A cart can backtrack on safe ground and switch crossings at the fork. The map shows both banks and the available crossings. This is one freight route with two economic endpoints. Regional settlement ledgers and faction encounters still need a shared employment and shipment lifecycle. See the [Dev Panel review procedure](../../tools/axiom-dev/RESIDENT_WORK.md#westbank-convoy-review).

The [living-world](../specs/AXIOM_Living_World_Expansion_Prompt.txt) and [planetary co-op](../specs/AXIOM_Planetary_Simulation_And_Coop_Architecture.txt) briefs call for detailed local actors, scheduled regional work and compact distant records. The runtime now seeds a bounded neighborhood of settlement ledgers when the player discovers an unregistered site. Up to 128 known settlements can advance on the running solo world clock without their chunks or residents loaded. Food, materials, work orders and population remain in the settlement sidecar.

Shortages can send an eligible resident into a saved journey. A journey chooses a nearby known settlement over a generated passable link, takes days based on distance, and admits the original identity only if the destination still has food and a finished vacant home. Otherwise it waits or looks for a route again. The save reader checks that each resident has one owner across households and journeys. Distant workers can fund a project and bank labor. On approach, the existing edit path projects credited labor in batches of up to 32 cells, defers occupied cells and stops on conflicting terrain edits. A workshop or home becomes usable after projection completes. The world advances on the game clock while the session runs.

This covers known regional sites, simple stock balances, one project per site, and abstract resident migration. The current route is a generated link with an arrival day. Journey bodies, waypoints that reflect player bridge edits, cargo and interception, multi-leg routing, distant actors in co-op, institutions and faction campaigns still need implementation. The ledger currently seeds neighborhoods as the player explores, and its cap bounds the work. Establish actual population throughput and frame cost in a repeatable native scene before raising it.

Build the next proof around two settlements and one contested crossing. Put the same worker, load and job under one owner through departure, offscreen progress, visible arrival, interruption, save/reload and resumed work. Share route searches by destination and invalidate only edges touched by construction, damage or changed access. Schedule jobs and trips as due events with budgets per region. Materialize the few actors and facilities players can affect. Keep a compact record of commitments, household needs and physical inventories so that distant production still responds to shortages and disruptions. Once that loop is measured, connect trade, migration, municipal projects, campaigns and orbital links through the same event and ownership model.

On Hills, generate a substantial city, a defensible settlement held by a
rival state, supporting villages and production sites, and terrain routes
with a viable detour. Include inherited infrastructure and a dangerous
underground site that affect settlement or travel. The city has a walkable
hinterland, ordinary occupied streets, mixed districts, and visible traces of
recent change. Reuse existing peoples and authored setting.

Use the following component checks during development. Integrate them in one
playable province before widening distribution or moving to planetary scale.

Fernwake's Hills nursery supports four Glowcap beds on exact-water worlds.
The original bed is in place. Additional fungal-soil beds can be built one
metre west, north and northwest at the same level. All draw from the same
finite intake. Where the orchard spillway exists, they can also draw from
water that reaches the bottom of its ditch.
On exact-water worlds, a completed growth stage consumes 32 L per square
metre of bed area from the intake. The same committed water transfer used
by pumps records the consumption in the environment's transpiration ledger.
Older worlds retain one bed and the water-presence check. Clear and refill the intake with
ordinary water tools to resume growth. The orchard keeps one seed in the bed
at each harvest and sells the surplus. Stock persists through midnight.
Unloaded beds cannot replay missed growth intervals using newly observed
water. A wider catchment network and native save/reopen evidence are still open.

The Fernwake freight clerk also buys and sells Glowcaps from finite stock.
The orchard holds back one Glowcap for local trade and dispatches surplus
across either clear bridge lane. A cut or blocked crossing holds the cargo
until a lane opens. Unloaded edited cells leave the crossing unobserved.
Hand deliveries can stock the clerk while the shipment waits. The farming
sidecar records the orchard, cargo, trip progress, market and last bridge
report together. The bridge cart and receiving counter crates reflect those
stocks. The orchard keeper and clerk visit their work areas during dispatch
and arrival. The clerk distinguishes the last report from a current observation.
Orchard and freight staff give optional directions to the existing nursery,
freight office and service ledger. The personal journal saves these leads
for later map pins. This is the bridge and trade component of the province,
with native acceptance pending.

The receiving pantry uses the existing trailcake recipe, consuming Glowcaps
from freight stock and biomass from its stores. A usable receiving bench is
required for cooking. Up to five local residents draw one meal each per day.
The clerk protects their next day's reserve and sells surplus. The pantry can
buy biomass and prepared meals with a finite purse. Player donations can keep
it supplied when the purse is empty. Ingredients, meals, funds and meals
served persist with the freight state. This models one district's food supply.

| Order | Deliverable | Evidence needed to advance |
| --- | --- | --- |
| 1. Bind a reach | Pick a fixed Hills fixture only after checking the existing water writer and a reachable inlet, field and outlet. Give the reach, intake, plots and workshop stable IDs and map their cells to the existing baseline. Start with an admitted obstruction in the loaded area; do not change the generator for the fixture. | Record baseline intake delivery, plot conditions and the outlet path. A dry or disconnected fixture must not pass on its appearance. |
| 2. Follow the water | At a fixed logical interval, derive intake delivery from committed water state. Reuse the existing water and plot paths; use a bounded reach summary only where necessary. A read of flow creates no water. Any actual abstraction must debit its owner and report its destination. | Compare control, blocked intake and restored supply on the same starting state. Show where the water goes or remains stored, and which plots lose and regain growth. |
| 3. Connect one livelihood | A defined harvest deposits real items into one workshop's owned stock by an explicit handoff. Production or a trade offer draws from that stock. An empty stock limits output; a daily offer reset never creates inventory. Player delivery can bridge a shortfall. | Without supplies the workshop eventually stops, with a reason the player can inspect. Repair or delivery restores the offer through the same rules. Crop loss or trade scarcity may be delayed by growth and reserves. |
| 4. Preserve consequences | Save committed edits, stock, logical cursor and any pending operation needed to resume. On the first slice, defer observations that need unavailable edited chunks; surface the last known state as such. Add coarse continuation only with an explicit exchange contract. | Leave and return, then save and reopen both blocked and recovered states. No regenerated water, duplicate harvest, or double payout. A failed or deferred write cannot debit or credit stock. Compare bounded work with the idle/control case. |
| 5. Connect the province | Use the city, rival stronghold and supporting settlements to prove trade and the broken-bridge history fixture. Carry recent changes to homes, work, migration, stocks and authority into play. | Walk the changed district and routes. Compare intact and severed crossings, inspect the detour and shortages, then repair or replace the crossing through normal play. |
| 6. Make conflict playable | Raise and move a supplied force along those routes. Let the player travel, recruit, trade, build an outpost and participate on foot and through overhead orders. Give both powers reachable objectives and terms for withdrawal or peace. | Compare army and civilian responses before and after player intervention. Carry casualties, equipment loss, fortification damage and recovery through both views and a save reload. |

### Factions, settlement building and war

The [Realm orders review](../../tools/axiom-dev/RESIDENT_WORK.md#realm-orders-and-campaigns) covers the first native implementation. The travel sidecar saves faction relations and expeditions for the active world. A player can establish a charter at an abandoned site by transferring a real recruited household and stock. A supplied expedition ties up one household while it marches, fights, sieges and returns. The siege resolves reserve depletion and ownership transfer. Explicit fortifications, siege weapons, larger local formations, arbitrary founding sites and co-op authority are separate milestones.

Run this on one active planet or dimension. Keep inactive worlds as saved state. A local actor, a regional traveler and a distant summary refer to the same identities, stocks and commitments. The reduced layer owns daily decisions and the detailed layer resolves the physical work players can observe and interrupt. Keep decisions explainable through a small set of reasons such as housing, work, supplies, access, security and obligation.

The first faction increment gives each registered settlement a saved charter from twelve regional factions. Charters choose housing-first or workshop-first construction using the existing stock, labor and edit path. Migrants prefer a reachable holding of their charter when it has room. Residents report the local charter, building priority and settlement size in conversation. Dev Panel's **Inspect factions** shows holdings and households in the active world. This increment establishes ownership and civilian policy. The first campaign below still needs mobilization, supply, combat and recovery under the same records.

Use the [conquest and politics design](../superpowers/specs/2026-11-06-conquest-politics-and-playstyle-paths-design.md) and the [fortification and siege design](../superpowers/specs/2026-11-06-walls-sieges-ages-and-rts-inspiration-design.md) to build one persistent province. Westbank Market and Eastbank Farms are economic endpoints today. The next slice gives each a faction, a reason to contest the crossing, and the means to build, defend or lose a holding through the same simulation.

| Work | Playable result |
| --- | --- |
| Claim and found | Stake an outpost with a real route, supplies and an authorized builder. The settlement record owns its claim and jobs. Housing, water, food, work and trade determine whether people move in and whether the outpost can sustain a village or town. Let AI factions use the same founding and building orders. |
| Build and maintain | Residents take jobs from the settlement and spend materials from its stock. Add storehouses, workshops, housing, walls, gates and towers in usable stages. Broken access or missing supplies stall the affected job. Nearby residents build through normal voxel edits. Distant work advances through bounded orders that reconcile with committed edits on approach. |
| Raise a force | Factions mobilize named residents, equipment and food from real holdings. The same people leave civilian work, travel with a supply record, and appear in the local world on approach. An army without supply must change its objective, withdraw or negotiate. |
| Fight over a place | Give each side a concrete objective at the crossing. Scouts report access and defenses. A force can secure a gate, cut deliveries, take a detour or withdraw. The fight uses the same roads, walls, gates and supplies the player can change on foot. |
| Resolve and recover | Occupation or a negotiated settlement changes who can command the holding. Preserve the residents, losses, damaged buildings, depleted inventories and outstanding repairs. First-person inspection and the strategic map read the same ownership and campaign records. |

The first campaign should start with two powers already trading and one player-foundable outpost. Follow a resident from construction or civilian work into mobilization, interrupt a supply run, and leave the area during the conflict. On return and after a reload, check the same people, cargo, ownership, damaged structures and repair orders. End the campaign through withdrawal, terms or occupation and watch both settlements keep operating. Measure this loop before expanding the number of armies or settlements.

### Province acceptance

| Check | Playthrough evidence |
| --- | --- |
| Settlement | Walk from the hinterland into a working city. Follow goods from production to market. Growth or migration changes occupied homes, workplaces, storage and services. Find physical traces of an earlier district or route. |
| Conflict | A state raises, supplies, moves, loses, replenishes and withdraws a force. Recruitment and recovery debit people, goods and time. Withdrawal or a settlement ends the campaign while civilian life continues. |
| Intervention | On the same starting state, cut a crossing, damage a production site or alter a fortification. Observe a different shipment or military decision. Show reserves, rerouting or repair at work, and use an alternate route or rebuild through ordinary tools. |
| Persistence | Leave and return, save and reload, and switch between first person and overhead command. Check identities, stock, ownership, troop losses, terrain damage and player construction in each view. Verify host, join and rejoin separately for co-op. |
| Exploration | Reach and investigate a dangerous underground site without taking a governance role. Show how its presence or infrastructure affects a surface route or settlement and what the player can discover or change locally. |

## Rules at the boundaries

- **One owner per quantity.** The generated terrain/receiver graph is a
  baseline. Committed terrain and water state own local changes; farming owns
  plot progress; the workshop owns its stock. A summary is derived or owns a
  distinct declared stock. Do not run fine and coarse writers for the same
  stock or substitute generated cells for unloaded edited ones.
- **Explicit inputs and outcomes.** For water, specify units, boundary inflows
  and outflows, storage, and any allowed sources/sinks. For goods, account for
  harvest, handoff, consumption and output. Record operation identity and the
  changes that downstream consumers need; do not add a universal event bus
  before proving a use. Rendering, map symbols and dialogue read those
  outcomes.
- **Bounded progress.** Work in stable logical order with fixed work limits
  and a resumable cursor. Wall time can schedule processing; it cannot decide
  what is consumed, lost or created. Declare which subset is reproducible and
  which reads may be deferred or stale. Measure admission, backlog and save
  costs before setting a performance target.
- **Compatibility.** Keep stored generator versions and existing terrain.
  Version any new persisted record and handle missing state explicitly. A new
  generator gate applies only if baseline generation changes. For Planet
  expansion, consume the spherical receiver graph and check seam crossings;
  the planar fixture is not proof of Planet behavior.

Dynamic sediment transport, regional climate feedback and coarse ecosystem
succession can wait for a specific playable use. Planetary expansion and
interplanetary infrastructure extend the province's settlement and transport
rules.

Plant placement can advance alongside this slice. Bamboo keeps its wild
groves on dense grass and its planted stalk growth with a moisture bonus.
Sugarcane adds a distinct niche at Hills generator 149: seeded patches on
warm, gentle banks just above solved freshwater channels or shallow inland
lake margins. The player can
harvest cane and plant it on grass, dirt, sand, or mud. Cultivated cane
needs light and water within two metres horizontally and one metre down;
growth pauses when it dries out. Older terrain keeps its stored generator.
Cultivation uses ordinary crop persistence. For the next plant, choose a
habitat and a player use before adding a new placement rule.
