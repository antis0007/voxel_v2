# Walkable construction quarter

This slice adds an opt-in native fixture to the existing Axiom app. It uses the
existing settler job system, physical chest inventory, path search and bounded
voxel effect commits. The surrounding buildings, streets and planted park are
fixture infrastructure. The vacant parcel is built from paid construction jobs.

Run `tools/Run-CityQuarter.ps1` after building the native app. `-Tradition 0..2`
chooses timber framing, courtyard masonry or terraced stone; `-Complexity 1..3`
chooses storeys. These are architectural traditions, not claims to simulate
historical cultures. `-Check` runs the actual gameplay loop without a renderer.
`-Capture` records native images and a binary identity manifest.

The construction controller creates one layer at a time from housing demand.
Three builders each fetch up to eight blocks per trip, walk to reachable work
positions and spend one block per accepted placement. Claims allocate paid cargo
to distinct cells; they do not debit a second inventory.
Buildings have street entrances, window openings, interior stairs, a rear service
stair and distinct roof geometry. Seeded facade and balcony choices vary buildings
without changing their circulation. Occupancy commits only after every planned
cell exists. An obstruction stops the project instead of erasing existing work.

The project record is an optional field in the existing JOB1 farming sidecar.
Old records omit it. No main save version or world-generation gate changes.
The current host/single-player gating and pause model remain in the existing job
service. The new fixture requires an explicit scratch save directory.

This is not yet a city economy. The housing ledger and occupancy count are the
first causal bridge; four residents moving into a completed home remain an
aggregate count. Individual residents do not yet use beds or institutions.
Construction stops outside the current builder's 32 m player interest range.
The overview is a camera preset, not an RTS control system.

Architecture dimensions use a metre lattice. `geometry::plan_for_scale` expands
it into the active one-metre, half-metre or quarter-metre grid without shrinking
rooms or doors. The controller admits at most 256 pending cells and revisits a
layer until it exists. Focused checks compare physical bounds and material volume
across all three scales. The disposable native fixture currently requires the
one-metre preset: fine-scale builder throughput and full play remain unverified.

Interior storeys and their reserved stairwells are the current room boundary.
Future room records should reference the parcel and storey, separate tenancy
from walls, and reserve circulation before household purchases or decorations.
Traits and long-term goals can score candidate furnishings within a real budget;
care, cleaning and repair consume actual work time. None of those household
behaviors is implemented by the fixture's aggregate population count.

## Multiplayer boundary

The existing settler service rejects multiplayer contexts, including the host.
This slice keeps that boundary. It does not add client simulation, bypass host
pause, allocate a second inventory or change integer matter authority. Building
effects use the existing authority, bounds and protected-assembly admission.
Joining a multiplayer session does not grant networked construction support.

Before enabling it, add host-admitted project commands with stable settlement,
parcel, worker and operation identities; revision checks and retry deduplication;
host-owned reservations and inventory transfers; authoritative world edit and
project deltas for clients; reconnect snapshots including cargo and commitments.
Test two clients competing for the last block, replayed commands, host pause,
builder death and disconnection during a haul. The same project must remain
visible in FPS and commander views without either camera becoming a new writer.

## Next functional slices

- Replace the fixed fixture parcels with terrain-aware street frontage, rear
  access and public right-of-way reservations. Preserve occupied parcels and
  player edits while considering infill, extensions and repair.
- Connect a harvested timber stock and masonry workshop to the same chest ledger.
  The visual recipe graph must compile to the same authored item/recipe records
  and expose actual shortages, transport capacity and downstream demand.
- Add households with home and work commitments and journeys through the street
  graph. Market stalls transfer actual stock; school capacity, instruction time
  and available teachers affect learning. Parks provide measured access and
  leisure capacity instead of being decorative tags.
- Cache street-distance fields and affordance indices. Use cheap need scoring,
  scheduled decisions, bounded job claims and shared district demand. Preserve
  identity, cargo and commitments when reducing distant activity to events.
- Introduce bounded technique refinement: retain construction cost, access
  failures, occupied floor area, commute and repair costs per completed design;
  propose a small seeded variation, validate circulation and structure, compare
  those measured scores and retain improvements. Keep diversity and household
  attachment so every district does not converge to one optimal box. Learned
  construction techniques must still consume real tools, materials and labour.
- Separate street layout, building grammar and economy graphs. A graph connection
  must represent a typed spatial constraint, job prerequisite or conserved flow;
  layout coordinates are editor metadata. Show simulation evidence beside edits.

Trade between two settlements, warfare, functional utilities, regional
demography, RTS firegroups and adaptive cultural learning are later milestones.

## Background civilization and consequences

The quarter is a local construction proof, not the background civilization
driver. Distant progress should commit resource-backed project and occupation
events against a settlement record. The pure architectural plan can then
materialize a resulting building at the active scale without replaying decades
of historical block placement. Store the adopted technique revision, physical
damage and player edits separately; loading a view must not reroll a plan or
create another copy of its inventory.

Keep people, households, enterprises and authorities as separate owners.
Domestic and care work consume time; goals can outlast a current job. An illegal
extension is a project with an owner and physical access, while an enforcement
order is another institution's decision with evidence and authority. Demolition
must account for occupants, displaced households and salvage through the same
project and resource lifecycle. These event records can support local stories
without making every distant citizen run a full embodied update.

Early defense should protect an economically functioning settlement before
large RTS command is introduced. Raid pressure can rise with visible wealth,
exposed routes and the history of encounters, but raiders still need an origin,
travel, supplies and objectives. Existing host pause and authority rules apply
to city and raid clocks alike. No new raid system is implemented here.

## Orientation

The fixture currently uses aligned frontages with a 180-degree reversal. It has
no diagonal street planner or diagonal building rasterizer. Future site scoring
should add a strong but finite alignment cost, for example
`alignment_weight * sin(2 * angle)^2`, alongside terrain and access costs. That
prefers cardinal alignment without forbidding a diagonal bridgehead or infill
plot. Rasterize rotated solid volumes by inverse sampling at the active voxel
scale, then validate door width, stair headroom, street frontage and connected
interiors. A preview alone cannot establish those physical guarantees.

## Three-builder follow-up

The quarter now assigns the existing resident identities 0, 1 and 2 to a bounded
construction crew. Each keeps a separate saved position, cargo and material flag,
plus runtime route and action state. Shared orders, stockpile and priorities stay
in the existing job state. This is three embodied workers, not 24 embodied
residents, a general population scheduler or a functioning household economy.

Paid fetches reserve specific unclaimed cells for the carrier. Claims share the
checksummed JOB1 record with cargo and worker identity. Repeated fetches cannot
replace existing cargo; placement requires the carrier's claim and a successful
conditional Air-to-material edit before one block is consumed. Completed and
canceled orders release claims while unused cargo remains physical and returns
through the existing storage action. A dead worker releases claims and drops the
actual load. An unavailable or interrupted worker retains its load and claims.
There is no timer that silently refunds or recreates materials.

Legacy cargo without claims can reserve remaining unclaimed orders after load.
Saved claims are rejected if workers are duplicated, a cell has multiple owners,
the order is missing, or claims exceed the carrier's matching cargo. This does
not make the independently written farming, inventory and voxel files an atomic
crash-safe world snapshot. Multiplayer construction remains disabled.

The next economic slice should connect a real producer's output to this same
physical stockpile and its construction demand. Shops and stalls need conserved
goods and payment, owner identities and reachable delivery work before their
presence represents commerce. Schools, municipal zoning, autonomous demolition,
offscreen growth and personality-driven life goals are still future systems.

## Timber barter and paid market cover

`-Market` adds a fourth persistent resident, a finite eight-cell fallen-timber
input and a staffed merchant chest. The authored offer in `assets/city_market.ron`
exchanges one existing iron inventory unit for one log. These are barter goods,
not a newly minted currency. Builders carry payment to the merchant when the
construction stockpile lacks timber. Empty stock, an absent merchant or full
payment storage leaves the payment in the carrier's possession.

The supplier has a saved delivery chest and work priorities. Each buyer's saved
trip retains its destination, offer and physical payment. The existing sidecar
checks also cover these commitments. This is not a crash-atomic journal spanning
farming, inventory and voxel files, and it does not implement simulation tiers.

The market cover is no longer installed as scenery in new market fixtures. Its
four posts use eight log cells; the roof and exterior work steps use 29 plank
cells. These join the same paid, ordered project as the house. Exterior steps provide reachable work positions; the two-metre
clearance is checked by grounded player movement. Old lots default to no new
cover project, so loading them does not replace their existing geometry.

The default verified plan has 524 house cells plus 37 market-cover cells. The
focused native check traces all 561 placements, eight recovered and sold logs,
the buyer's empty iron treasury and the merchant's eight received iron units.
The no-market fixture retains its original 524-cell bill. Geometry-only checks
cover the added structure at all three voxel scales; the native fixture remains
restricted to one-metre voxels.

`-PreviewTrade -PreviewTicks 1000000` advances actual gameplay until the first
sale or the supplied tick limit. It does not synthesize completed construction
or represent unloaded-city progression. `-Benchmark` provides a bounded run
without periodic PNG captures; `-Capture` remains available for visual review.
This fixture still uses authored roles, shared public project funding and a
fixed offer, rather than autonomous enterprise formation or general economics.
# Seeded neighborhood layouts

The optional `-Layout Ordered|Mixed|Organic -LayoutSeed <u32>` quarter mode uses
`assets/city_layouts.ron` to control frontage and setback variation, diagonal
cost, turn cost and elevation cost. Architecture tradition remains independently
selectable. These are authored planning predispositions, not simulated cultural
learning.

The planner selects six non-overlapping, level parcels within a bounded area and
routes three-metre streets between their frontages and the public spine. A small
authored terrain rise is surveyed and materialized from the same height function.
Existing buildings, the construction chest and terrain exclusions constrain
routing. Eight-direction A* retains heading in its search state, uses deterministic
tie breaks, and stops at 20,000 expansions per connection. Diagonal cost is finite
in every profile. Parcel search stops after 64 candidates per slot. Failure is
reported; the planner does not clear obstacles to force a fit.

Five houses and public streets are supplied by this disposable scene. The sixth
parcel's actual position enters the existing paid construction queue. Its bill
includes a street-facing entrance landing. The three workers retain the same
cargo, claims and conditional world-edit path. Layout mode currently runs
separately from the market fixture.

The JOB1 quarter state stores the chosen parcels, centerlines, seed and planner
version. Old states default to no layout and old lots to no entrance landing.
Loading does not regenerate streets or replace voxel edits. The existing world
generator versions are unchanged. The scale conversion also keeps reflected fine
cells within their original coarse-cell footprint.

`-Check -LayoutOnly` runs the layout matrix and grounded native street walks.
The full `-Check -Layout <profile>` additionally runs paid construction and its
interrupted-state checks. Geometry checks cover 1 m, 0.5 m and 0.25 m cells;
the playable quarter remains restricted to one-metre Superflat. This is a local
fixture, not arbitrary-terrain autonomous expansion, road construction by NPCs,
co-op settlement growth or distant simulation. Buildings remain cardinally
oriented while streets may be diagonal.

Observed trade-route learning is a subsequent movement slice. It needs bounded,
host-owned records of completed journeys, elapsed travel time, failures and real
threat observations. Route availability must follow the edited physical world,
including excavated shortcuts. Congestion and diminishing returns must prevent
unlimited reinforcement of a single path. Traffic alone must never erase threats:
safer travel needs actual patrol or infrastructure effects. This layout planner
does not yet collect those observations, choose trade routes, or attract businesses.

## Verified layout increment

The optimized native build passed the Ordered seed 7 and Organic seed 424242
construction runs. Each supplied 530 physical material units and placed 530 cells,
including six entrance-landing cells, with zero remaining stock. Both used three
builders, preserved interrupted cargo and claims through the JOB1 sidecar, and
passed grounded entry into the completed home. The legacy scene still placed
524 cells. Its market variant placed 561 cells and transferred eight recovered
logs for eight existing iron inventory units.

The native layout matrix covered three profiles and three seeds: 54 grounded
street-to-frontage walks and 45 entries into existing houses. Geometry and service
radius checks covered all three complexity levels; reflected geometry retained
its metre-space footprint at 1 m, 0.5 m and 0.25 m scales. The prior market build's
actual interrupted JOB1 file also loaded successfully.

Moving the paid parcel exposed two navigation defects. A millimetre tolerance
keeps work clearance stable at translated coordinates; 432 perturbed samples
cover positive and negative coordinates at the three voxel scales. Stair
transitions now check body clearance at both ends, avoiding an impassable roof
lip even when the destination itself is standable. A focused grounded descent
check and both complete construction runs exercise this correction. Paid parcel
selection also rejects any plan cell outside the stockpile's 24 m service radius.

## Population and economy boundary

This fixture has three individual builders, or four assigned residents with the
market supplier. The controller currently caps helpers at three plus the primary
worker. It has not been measured at 16, 24, hundreds or thousands of active NPCs.
The displayed resident count of 20, then 24, is aggregate data; completing a home
does not instantiate four additional people. Housing demand is authored at scene
startup. Five houses and public roads are supplied immediately; only one paid
home project progresses. Workers require an active actor within 32 m of the
player. Preserved cargo and claims outside that radius are not offscreen work.
The tested ordered and organic houses took 383,220 and 388,350 fixed steps,
respectively, or about 53 to 54 minutes of simulated time at 120 Hz. The bounded
preview can advance these same jobs for inspection; it is not a live speed
control or proof of rapid city-scale development.

The market supplier retrieves eight authored fallen log cells. This demonstrates
physical salvage, hauling, stockpiling, purchase and construction, but not a
renewable logging industry. Crop growth, crafting recipes and an authored pantry
exist elsewhere in the engine; this quarter does not connect them into resident
farming, cooking, eating or furniture production. There are no autonomous
enterprises, resident life goals, general redevelopment or learned trade routes
in this increment. Co-op settlement work and arbitrary-scale native construction
remain disabled here. These bounds must accompany any population or performance
claim about the scene.

The next production milestone should reuse the shared item/job path for one real
harvest-to-household loop: a resident harvests, hauls to storage, a staffed kitchen
consumes recipe inputs, and a household removes and eats the resulting food.
Inventory, reservations and interruption behavior need tests before adding more
residents. The separate Downloads/Java audit should be reconciled before
duplicating any existing economy implementation.

Concurrent production tasks were subsequently added to the active checkout's
`jobs.rs`. They were preserved, with both tested movement functions unchanged.
The executable and results above come from the isolated layout snapshot and do
not validate those newer production tasks.

The final rendered construction benchmark used Ordered seed 7, a 150,000-step
gameplay preview, 1280 x 720, four view chunks, 30 seconds of warm-up and a
20-second measurement with periodic screenshots disabled. Three builders remained
within their activity radius. It averaged 40.78 FPS, with 73.861 ms p99 and
99.142 ms maximum frame time. Recorded gameplay CPU time averaged 0.320 ms;
GPU terrain passes averaged 11.242 ms. These are observations on the shared
development machine, not a population capacity claim. Pipeline creation in the
rendered runs took roughly 60 to 106 seconds before warm-up.

The separate close capture shows all three workers changing positions and the
active layer's queue decreasing from 51 to 45 as six wall cells appear. Native
overview captures compare ordered and organic seed 7 layouts at the same camera.
The earlier six-view comparison also includes seed 424242 and the three
architectural traditions. Those views document the actual supplied scene and
paid project; they do not imply that the existing neighborhood was built by NPCs.

## Bare-site production and construction

`tools/Run-CityQuarter.ps1 -StartSite` starts a separate local one-metre scene.
It reuses the resident production implementation reconciled by the Downloads/Java
integration task. The standalone Rust prototype in that folder is reference
material; its existence does not make its systems part of the native game.

The scene starts with three assigned residents and zero buildings. Bootstrap
content consists of a flat serviced plot, water, a workbench, a stockpile,
twelve seeds and one hundred finite fallen-log cells. The twelve crop beds use
existing crop growth. Existing production jobs sow, harvest, cook, eat, cut
planks and craft chests. Construction consumes those actual goods.
Ambient spawning is disabled in this controlled founding scene. Construction
still refuses to replace occupied cells; clearing animals from construction
sites in the wider world is not implemented here.

`assets/city_start_site.ron` defines three sequential paid projects: a work
shelter, a shared home and a storehouse, including connecting boardwalks.
The later projects require actual stored meals and recorded consumption before
their first layer opens. This is an authored expansion policy, not autonomous
site selection, household demand or a general city economy. Layer admission is
bounded and blocks on conflicting voxel contents and protected ownership.
Furniture follows roofs; permanent two-lane steps provide construction access.
Each building also has a paid perimeter apron. It lets a builder leave an
unfinished window ledge without a two-metre drop. The three projects now cost
118, 136 and 108 cells, including three crafted chests. Source logs were moved
away from this frontage during fresh-scene bootstrap.

The project cells, finite timber provenance, phase and current layer persist in
the existing JOB1 sidecar. Old quarter states default to no start-site state.
Loading does not rebuild geometry or replenish resources. Existing worker cargo,
claims and recipe counters retain their normal persistence paths.
The source ledger admits at most 64 mining orders, matching the existing JOB1
validation bound. A persisted cursor considers each source once, so later player
replacements do not become new timber work. Older valid site saves without this
cursor retain their already-admitted queue. The earlier fixture incorrectly
queued all 100 logs and could not reload a save made before its queue fell to 64.

The native start-site acceptance path checks material conservation, all physical
project cells, a deliberately blocked layer, JOB1 reload during carried
construction, an immediate-start reload, and grounded routes into the buildings. Validation results are
recorded with the executable and captures; this description alone is not a
passed test. The scene remains local and single player. It does not provide
renewable forestry, offscreen municipal growth, new households, markets,
personality-driven decisions, multi-scale live construction or crowd-scale
performance. This scene measures three assigned workers. Concurrent population
work changes the general crew capacity and is assessed separately.

Worker routes now consider flat diagonals with 10/14 cardinal/diagonal costs.
Both adjacent corners and a conservative swept body must remain clear; height
changes use the existing cardinal step checks. The existing collision mover
remains authoritative. The production task's world-speed control is exposed in
the site panel and uses the existing fixed-step cap. It changes the whole local
simulation, including player movement, and does not guarantee the selected
speed when frame time reaches that cap. Use 1x for normal first-person walking.

The shared job controller gained concurrent staggered scheduling, paving costs,
open-ground route smoothing and construction travel multipliers during this work.
Integration preserves those changes and adds the conservative diagonal
clearance guard. Work endpoints also use shaped-block collision support heights:
a chest is 0.875 voxels high, so treating its top as a full cube could select a
crafting position whose workbench ray never cleared the chest. The regression
checks the actual support height and a reachable endpoint with arrival tolerance.
The earlier isolated native snapshot's 10/14 route costs and 2.2 m/s
movement figures must not be presented as measurements of that newer controller.

The reconciled native start-site acceptance passed 362 paid placements in
109,380 fixed steps (15.19 simulated minutes). Three residents made 18 meals and
ate 12. Initial, obstructed, carried-construction and finished JOB1 round trips
passed with conserved wood, planks, chests and meals. The grounded route through
all three interiors covered 55.81 m. Construction travel peaked at 4.40006 m/s;
69,570 actor ticks included actual diagonal displacement. The diagonal guard
also passed blocked-corner, headroom and climb checks at 1, 0.5 and 0.25 m.
This validates the guard at those scales, not multi-scale live settlement work.

The same executable passed the 561-cell market construction regression, including
eight salvaged logs exchanged for eight physical iron units, and nine native
layout cases covering 54 frontages and 45 existing interiors. Native start-site
captures show work in progress and the three completed buildings. Those buildings
are paid projects; the serviced plot, sources, recipes and policy are authored.

The active-work rendered benchmark used Vulkan, a persisted pipeline cache,
1280 x 720, four view chunks, 12,000 preview steps, 30 seconds warm-up and a
20-second measurement without screenshots. It averaged 164.56 FPS, with 6.264 ms
p99 and 47.347 ms maximum frame time. Gameplay CPU averaged 0.087 ms/frame.
Warm renderer startup was 3.51 seconds; a cold Vulkan capture took 11.84 seconds.
These are three-resident measurements on the shared development machine. They
are not a matched backend comparison or evidence of city-scale capacity.
