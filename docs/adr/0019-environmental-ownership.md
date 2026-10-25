# 0019: Environmental ownership and bounded soil aggregation

Status: implemented environmental and everyday UI increment; full plan remains in progress.

Generator 42 enables the environmental model. Generator 1-41 terrain and
behavior retain their historical paths. Save 31 adds a versioned environmental
footer. No dependency or hosted CI was added.

## Ownership and reuse

- Core defines physical units and material laws; water is microlitres and
  mineral/biological stocks are milligrams. Content validates substrates,
  species niches, compost compositions and explicit development/wear rates.
- Simulation calculates conservative exchanges without block names. World
  authority handles admission, activity, geometry, inventory and persistence.
  UI receives observations and requests actions.
- Canonical voxel occupancy/collision retains each world's resolution.
  Homogeneous buried soil may share one cubic metre reservoir. Surface,
  heterogeneous and edited soil retains fine owners. Adjacent edits split
  aggregates conservatively before mutation. Stacked surfaces have distinct
  three-dimensional addresses.
- Free water has ONE quantity: coarse eighths times physical cell volume,
  minus a sparse fractional deficit. This encoding preserves exact microlitres
  while retaining existing storage/protocol interfaces. Deficits persist and
  share immutable chunk maps with render snapshots. They are not a second
  inventory. Infiltration responds on its first one-second step at every scale.
- Pumps, paired water transfers, excavation and irrigation use that quantity.
  Mesh heights, player immersion and splash queries receive fractional amounts.
  Both owners are admitted before a paired commit. Failed admission retains
  the source and pending work. Grain volume and retained pore water reduce
  available fluid capacity.
  Fraction capacity is reserved only when the result needs a fractional
  record; a full fractional table cannot block unrelated integral water.
- Sediment has suspended and resting stocks. Only suspended material advects;
  quiet supported water settles grains, and admitted hydraulic work can lift
  them. Dry deposits become geometry once sufficient mass is available.
  Maintenance recovers a real sand item, respecting backpack capacity.
- Construction plans displacement through actual neighboring openings before
  changing geometry. Sealed wet cells reject placement. Editor-created or
  removed free water is recorded as an explicit external exchange.
- Plants use finite soil nutrients, biomass and litter. Connected porous roots
  reach one physical metre and stop at barriers. A bounded, resumable sky query
  derives foliage/roof shading; edits invalidate the affected column only.
  Growth credit prevents small cohorts rounding every step to zero.
- Gravity drainage only removes water above the substrate's retained fraction;
  roots and evaporation can still consume retained water. This avoids treating
  all damp soil as a continuously draining tank. The field-capacity distinction
  follows the [NRCS National Agronomy Manual](https://www.nrcs.usda.gov/sites/default/files/2022-10/National-Agronomy-Manual.pdf).
  RON retention values are authored model parameters, not measured calibrations
  or a full soil-water-potential solver. No water is deleted at the threshold.
- Initial reserves are imported once. Newly constructed substrate has no new
  nutrient reserve. Applying held foliage with F consumes an inventory item
  and adds compost, which must decompose before nutrient uptake. Compost and
  sediment maintenance use normal inventory admission.
- Geological wear and biological development multipliers are explicit and
  saved per world. Environmental state survives geometry removal and unloading.
- Generation consumes shared downhill drainage connections, ocean outlets and
  closed basins in metre coordinates. Carving, water placement and bank
  substrates use the same versioned plans. No encounter script creates rivers.

## Work and memory bounds

Caps: 65,536 soil owners, 65,536 scheduled owners, 32,768 sediment owners,
65,536 hydraulic-work records, 32,768 fractional-fluid records and 4,096
light queries and 256 derived sky-chunk summaries. Conservative state allocation reservations fit below 64 MiB;
meshing and snapshots still have their own allocations and must be profiled.

No stable voxel scan discovers environmental work. Edits, accepted transfers
at substrate/sediment contacts, and a once-per-second player contact probe supply
eligibility. Bulk water over inert known geometry allocates no soil events.
External changes enter a future target tick; they cannot indefinitely extend
an overdue step. Read-only geometry dependencies consume no event slots.
Actual exchanges admit their receiving events before changing either stock.
Soil schedules one-second steps at fluid contacts, up to four seconds for
biological stocks, and up to eight seconds for slow subsurface storage. The
material flux/storage bound can shorten those intervals. Rates integrate the
actual eligible elapsed seconds; waking dormant soil resets the interval start
without evolving absent time. Soil-to-soil wakes retain this cadence, while
player edits and new free-water contact wake promptly. Sediment keeps its own
one-second settling/work service. These are process limits, not render LOD.
The service executes at most 512 ordered events inside a 0.5 ms child budget.
Light rays build at most one exact chunk summary per service and cross up to
64 cached chunks. Bit planes preserve every roof, opening and foliage cell;
column edits invalidate affected queries and the changed chunk summary.
Each exchange is atomic, and pending work persists. F3 reports environmental
lag separately. These limits are admission bounds, not hardware acceptance.

## Remaining implementation and acceptance

The production water/factory/thermal schedulers still depend on wall time and
near/fair service order. Ordered environmental events do not establish a
complete coupled tick, deterministic replay, or camera-independent outcomes.
The hydraulic solver still chooses transfers using its coarse compatibility
view; exact quantity accounting is not a new pressure/flow solver.

The drainage graph is an initial bounded model, without calibrated catchment
accumulation or climate-driven recharge. Settling is supported-bed settling;
it is not a complete deep-water suspended-load solver. Long sky queries and
growing disturbances need integrated throughput/streaming acceptance.

Groundcover cohorts are implemented. Individual tree development, shared
runtime tree construction/damage, broader communities and disturbance history,
new thermal energy/latent-heat rules, environmental wetness/sediment visuals,
and far river-water proxies remain unfinished. Fractional water cannot become
a full ice inventory item; partial ice needs an explicit geometry/inventory
contract. Legacy thermal thresholds are not the planned scientific model.

The UI increment supplies readable body typography, pixel accents, atomic
recipe staging, shared crafting admission, armor comparisons, actionable ground
inspection and consistent UI-scale bounds. Existing wand evaluation and machine
units remain available. Full key rebinding, building undo, remaining interface
redesign and accessibility/visual acceptance are still open.

Reference artwork and specimen identifiers belong in design/evidence only.
Production identifiers describe domain responsibilities and units.
