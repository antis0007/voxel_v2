# Living world integration design

Date: 2026-11-14. Status: owner direction agreed in conversation; this document
awaits owner approval. No source changed. Proper nouns are placeholders until
approved (`crates/app/src/egui_ui/UI_RULES.md`).

Builds on, and does not replace:
- `docs/superpowers/specs/2026-11-12-mana-magitech-design.md` (essences table,
  mana field, one carrier, conversion table, purifier). Referred to below as
  **MM**.
- `docs/GAME_DESIGN.md` §1 pillars, §6.1 capability graph, §6.3 disciplines,
  §8 magic, §9.1 one process shape, §10.1 populations, §11.2 one-valley slice.
- `docs/00_MASTER_DESIGN_AND_INTEGRATION_V2.md` conservation and integration rules.

## 1. Intent

The owner asked for four things at once: the anvil, smeltery and other station
or machine pages reworked; Forestry-like trees and tree farms; mob farms that
never rely on darkness; and magic and magitech linked seamlessly through all of
it. The governing requirement came after: **every system built or in progress
is part of one living, simulated world. Nothing is left out or abandoned as a
mechanic, and nothing is a disconnected imitation of modpack compatibility.**

Then: consequences must be real, but mitigable by technology that is realistic
(carbon capture and reuse, emissions monitoring, environmental effects and
mitigation). Mitigation must not slow the game or make it harder to play. It
should be more realistic, more interesting, and educational for a layperson.

Why this shape: point-to-point links between ~40 systems need ~800 authored
connections and always leave some missing. Instead every system reads and writes
a **small set of shared world quantities**. A system is linked when it changes a
quantity that some other system reacts to, and that is checkable by a tool.

## 2. Goals, non-goals, anti-goals

Goals
- One set of shared world quantities (§4) that every gameplay system touches,
  with a machine-checked link table (§11) proving no orphan mechanic.
- Hedrons coupled to the physics their element names (§5).
- Land health: one regional consequence field fed by industry, magic draw,
  mining, felling and ecology (§6).
- World carbon account with realistic monitoring, capture, storage and reuse
  (§7), cheap at runtime and adjustable per world.
- One process page template for every machine, station and ritual; anvil and
  smeltery rebuilt on it first (§8).
- Tree species with heritable traits and an automated tree farm (§9).
- Mob farms as engineered habitats under population rules; never darkness (§10).

Non-goals
- Sea-level rise, CO2 transport on a grid, or any per-voxel atmosphere field.
- Re-placing hedrons. Their positions come from the seed; `share` in
  `assets/mana_sources.ron` moves every hedron if changed. Leave placement alone.
- A second logistics network for mana (MM §5 decides one carrier).
- Bees as a full genetic sub-game in this pass (pollination is by flowers/wind, §9).

Anti-goals (the work fails even if it compiles)
- A link that exists only in a tooltip or a recipe row with no world effect.
- Consequences that compound into a death spiral or punish a casual player.
- Any new per-frame cost proportional to world size, or a field that never sleeps.
- Mitigation that is a single "clean air" button with no physical model.
- A mob farm that works because it is dark.

## 3. Constraints

- Save: `SAVE_VERSION` is 74 today (`crates/app/src/world/storage.rs:112`).
  Re-read it before implementing. Bundle every new persisted field from this
  document into **one** bump (§12). Old saves load with neutral values.
- Factory resources are a fixed array of 25 (`crates/content/src/factory_resources.rs`),
  and every per-resource save section has a version-inferred width. New
  resources (wood, CO2, biochar, carbonate, mana fluids) wait on
  `docs/design/RESOURCE_REGISTRY_PLAN_20261113.md` (step 1 done uncommitted,
  steps 2-6 open; its version numbers are stale). After that one migration a
  new resource is data-only. Do not add resources by the old one-bump-each
  route. It also unblocks ledger rows A09-07/A09-08. Prefer modes/fields where
  a new machine Kind is not essential.
- Player text: `egui_ui/text_rules.rs` bans "essence" (except the item name
  "wild essence") and "you/your". Internal ids may say essence; labels may not.
- Generator: no change to worldgen output in this design. Any later placement
  (wild tree species, hives) needs a claimed dormant gate; grep all branches
  for free gates first.
- Pillar switches (`assets/systems.ron`: automation, magic, forging, space) and
  disciplines (GAME_DESIGN §6.3) must leave no dead end. Every link row declares
  its fallback when its pillar is off (§11).
- Simulation uses fixed steps and positional seeded hashes. `world/ambience.rs`
  uses `Instant` and a frame-thread probe: it is a *reader* for presentation and
  recovery, never the authority for any field below.
- Content in `assets/*.ron`, validated once at startup; no content tables in
  Rust; no unwrap/expect outside tests/startup; no new dependency without an ADR.
- Shared tree: `smeltery.rs`, `habitats/*`, `farming/*`, `tool_station*` have
  uncommitted peer work. Coordinate; commit own hunks via the clean-worktree
  recipe.
- Visual changes need A/B captures and owner sign-off.

## 4. Shared world quantities

Each quantity has one owner module. Everything else reads it or submits deltas
through the owner's API. Rates are on fixed steps.

| Quantity | Owner (existing unless marked) | Resolution / step | Persisted |
|---|---|---|---|
| Matter (blocks, items, fluids) | world, inventory, factory | voxel / tick | yes |
| Heat | `simulation/thermal.rs`, `climate.rs` | sparse lattice | blocks yes, cells no |
| Power | `world/factory/power.rs` | network / tick | yes |
| Water | `simulation/cellular.rs`, `world/river_water.rs` | cell / tick | yes |
| Local air (dust, soot, acid, fuel gas, steam, O2) | `world/industry.rs` | ≤8192 cells | yes |
| Mana by essence | **new** field per MM §2 (16 m cells, u8, sparse, event-driven) | dirty cells | yes (pools, node charge) |
| Land health | **new** `world/land.rs` (§6), same 16 m grid as the field | slow step | yes, sparse |
| World carbon | **new** `world/carbon.rs` (§7) | one record / game hour | yes |
| Populations | habitats now; region cohorts later (GAME_DESIGN §10.1) | slow step | yes |
| Knowledge | capability graph (GAME_DESIGN §6.1) | event | yes |
| Standing | `crates/civ` | event | yes |

Rule: a new mechanic must read at least one quantity it does not own and write
at least one that something else reads. The link check (§11) enforces it.

## 5. Hedrons coupled to physics

Today `hedrons::supply` (`world/adventure/hedrons.rs:231`) returns a constant
`mana_per_second` per captured hedron. The five elements in
`assets/mana_sources.ron` each match a simulated quantity:

| Element | Fed by (sampled near the hedron) | Spent mana does |
|---|---|---|
| Fire | heat above climate ambient, lava/magma cells, burning blocks | heat impulse; smeltery/forge fuel; cooking |
| Frost | ambient below 0 °C, ice/snow cells, altitude | cooling; freeze water; chill machines; preserve food |
| Shock | active storm (`simulation/storms.rs`), conductor blocks, grid load within reach | power (existing hedron tap mode 6) |
| Toxic | fungal/biomass blocks, rot, blight pressure | growth/decay; fertiliser; biomatter |
| Wet | moving water volume (river/cellular), springs | water spawn/move; irrigation; brewing base |

Design
- `condition(hedron) -> f32` in 0.25..2.0, computed on a slow step (my call:
  every 600 fixed steps, and on edits in its reach) from a bounded sample of
  cells (my call: ≤256 cells per hedron, deterministic positions from the
  hedron anchor hash). Cached in a sidecar keyed by anchor; recomputed, not saved.
- `supply` multiplies by `condition`. The element table gains per-element
  coupling rows (which quantity, weight, clamp), in data (`essences.ron` when
  MM slice 1 lands; `mana_sources.ron` until then).
- **Draw versus regrowth** (MM §4): drawing faster than the hedron regrows
  raises local land pressure (§6). Sustainable taps are free; over-tapping has
  a readable cost.
- **Celestial tide** (my call, cheap): a ±15% multiplier from time of day and
  the moon's position. No `moon_phase` function exists today; derive it from the
  celestial/orbital clock the sky already uses (builder confirms the source),
  never wall-clock. The observatory predicts it.
- Conservation: Fire hedron → heat → Fire hedron must lose. The MM §5 startup
  cycle check covers mana↔power; extend it to mana↔heat and mana↔water.

Gameplay: siting matters. Dam a river and the downstream Wet hedron weakens;
run a forge beside a Fire hedron and it strengthens, but the forge's exhaust
raises pressure. Players read this on the hedron's inspector card.

## 6. Land health (regional consequence)

One scalar **pressure** per 16 m cell (sparse; absent = baseline), plus the
existing static blight zones in `adventure/affinity.rs` (160 m lattice) as the
seeded baseline. MM §2 already says affinity becomes "Void field minus
purification": land health is that subtraction made explicit and fed by more
sources.

Raises pressure (each a data row with a rate)
- Industrial air deposition from `world/industry.rs` (soot, acid) where it lands.
- Over-tapping a hedron or collector.
- Strip mining and quarry volume removed per cell.
- Clear-felling: trees removed without replanting within N steps.
- Spilled smeltery slag; dungeon machinery left running (ruins).

Lowers pressure
- Living trees by species (§9), mature mana flowers, clean moving water.
- Healthy habitat populations; dust filters and scrubbers reduce deposition at
  source; purifier machine (MM §5); rituals at mana cost.
- Natural recovery toward baseline over seasons (GAME_DESIGN §10.1: bend, not break).

Effects (readers)
- Blight spread and blighted creature variants (spawn rows gain a pressure range, §10).
- Crop and tree growth multiplier (0.5..1.2).
- Hedron purity: high pressure shifts a hedron's output toward Toxic.
- `ambience.rs` safe density reads `field - pressure`; `affinity.rs` reads it too.
- Civ: settlements near high pressure lose standing/trade willingness.

Cost: updated on a slow step only for dirty cells (deltas submitted by owners),
sleeping cells untouched. Cap the sparse map (my call: 65,536 cells, oldest
baseline-equal cells dropped).

## 7. World carbon account

Short-lived local pollutants stay in `world/industry.rs`. Long-lived gases are
well mixed in about a year, so one record per world is both realistic and free.

Model (fidelity label: "simplified, one-box atmosphere, real forcing law")
- State: atmospheric CO2 (ppm-equivalent), methane (ppb-equivalent), heat lag
  (K), sink stores (biosphere, soil, ocean surface), cumulative captured/stored.
- Step: once per in-game hour.
  - Forcing F = 5.35·ln(C/C0) + 0.036·(√M − √M0) W/m² (CO2 + simplified CH4).
  - Equilibrium ΔT = λ·F with λ ≈ 0.8 K per W/m²; actual ΔT relaxes toward it
    with a lag (game-scaled).
  - Methane decays to CO2 with a game-scaled lifetime (real ~12 years).
  - Sinks take a fraction of the excess each step (ocean, biosphere); trees
    and soil add explicit stores (§9, biochar).
- **Game scale:** atmosphere mass is scaled so a mid-game factory moves CO2
  measurably in a few play sessions, and a fully mitigated base can reverse it.
  The scale constant lives in data and is shown on the carbon page. Tuning
  target, not asserted: Gentle never exceeds +2 K equivalent with heavy play.
- World rule **Environment: Off / Gentle (default) / Realistic**, stored with
  world rules. Off keeps monitoring (education) but applies no effects.

Sources (data columns, not code)
- `assets/industry.ron` `emissions` and `fuels` rows gain a `co2` (and `ch4`)
  column: coal, coke, creosote, gas generator, blast furnace, coke oven.
- Fires and burning blocks; rotting biomass (CH4); volcanoes as natural baseline.

Effects (all at runtime, never in worldgen output)
- One offset added in `simulation/climate.rs` `ambient_c` (scaled by the
  world rule). Ice lines, sea ice, snow, crop ranges and freezing follow from
  the existing thermal model.
- Storm frequency scales with ΔT in `storms.rs` (more Shock, more lightning).
- Hedron balance: Frost condition falls and Fire rises with ΔT (§5).
- Civ standing reacts to the world record and to local air.

Monitoring
- Gas analyser block (NDIR CO2 sensor): reads world CO2 and local air species;
  readable by chip modules and the storage scanner.
- Weather station: ΔT, storm odds, season.
- Carbon page: rising-curve chart, source breakdown by machine kind, sinks,
  stored totals, the fidelity label and one-paragraph explainers (UI_RULES text).

Mitigation (each one is a process row in the one process shape, §8)
| Route | Real basis | Cost / by-product |
|---|---|---|
| Amine scrubber on a chimney | post-combustion capture | 20–30% of the plant's energy; captured CO2 |
| Compressor + injection well | geological storage in porous rock | power; needs a prospected porous layer |
| Greenhouse enrichment | CO2 raises C3 crop growth | CO2 consumed; crop multiplier ≤1.3 |
| Mineralisation | crushed basalt/olivine + CO2 → carbonate | carbonate building block; crusher work |
| Synthetic fuel | CO2 + electrolysis H2 → methanol | lots of power; carbon-neutral only on clean power |
| Algae / fungal bioreactor | photosynthetic capture | biomass; existing bioreactor gains a CO2 input |
| Biochar | pyrolysis (coke-oven variant) of biomass | stable soil carbon + syngas; soil fertility up |
| Flaring / methane capture | CH4 → CO2 (≈28× less warming) or fuel | gas generator fuel |
| Forests, wetlands, rock dust | biological and weathering sinks | land use; slow |
| Toxic/Wet ritual weathering | magic route, same conservation | mana cost |

Education: every row shows its real-world analogue and a fidelity label on the
inspector card (GAME_DESIGN §15.5). Suggested lesson hook: "the carbon cycle in
one base", reading the same account through a probe.

## 8. One process page

GAME_DESIGN §9.1: every machine, ritual and reaction is inputs + conditions +
energy + time → outputs + by-products + heat + risk. Build one egui page shape
from it; every station and machine page becomes a configuration of it.

```
┌ Smeltery ─────────────────────────────── [status pill: Working | Starved: fuel | Jammed: output full] ┐
│ INPUTS            CONDITIONS               ENERGY / TIME          OUTPUTS          BY-PRODUCTS        │
│ [ore][ore][  ]    Shell 5×5×4 ✓            Heat 1180/1085 °C ▮▮▮▮  [molten list]    Slag 12 · CO2 3/h  │
│                   Fire hedron 14 m ×1.4     Fuel: coke · lava · Fire mana          Soot → filter ✓   │
│ ──────────────── local field: Fire 82 · pressure 0.31 (low) · air: soot 4 mg/m³ ─────────────────── │
│ [ pack grid ]                                                                       [Done]          │
└──────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

- Status pill names the single blocking reason (starved/jammed/unpowered/
  wrong conditions), so the player can tell working, starved and jammed apart.
- Conditions list names the missing shell cell (fixes parity note B-GT-05).
- The local field strip is shared by every page and reads §4 quantities.
- Existing panels in `egui_ui/factory.rs` and `egui_ui/station_page.rs` migrate
  one at a time; the template lives in a new `egui_ui/process_page.rs`.

Anvil (first)
- Keep repair/combine/salvage (`world/stations.rs`) and XP cost.
- Add: repair forged tool parts by part (reads `tool_workshop.rs` parts/materials);
  rune inscription with a mana cost from the local field or carried vessels
  (reads `assets/runes.ron` grammar; the enchanting table keeps bindings);
  anvil material sets the max tier it can work.
- Show durability bars before/after, level cost, and why a pair is invalid.

Smeltery (second)
- Controller page: molten contents as stacked layers with units, temperature
  versus each metal's melting point, alloy preview from `smeltery::alloy`,
  fuel source (fuel item, adjacent lava, Fire mana), drain/faucet targets,
  shell dimensions and missing cell.
- Physical pouring and casting in `world/smeltery.rs` stay authoritative; the
  page only reads and issues the same actions.
- Emissions: smeltery fuel rows feed §7 and local air.

Other pages migrate after these two: furnace/alloy kiln, generator, battery,
induction, assembler, coke oven, blast furnace, brewing stand, stonecutter,
grindstone, enchanting table, ritual circles, tree farm, habitats.

## 9. Trees and the tree farm

Species table `assets/trees.ron` (new). A row: id, display name (placeholder),
shape (extends `TreeShape` in `farming/table.rs`: trunk, crown, branching
form), growth steps, soil, light, climate range (reads `climate.rs`), wood
yield per m³, fruit, sap/resin product, **element affinity** (which essence it
soaks or emits near hedrons), **carbon per m³** (feeds §7 biosphere store),
**pressure relief** (feeds §6).

Traits are heritable: each sapling item carries two alleles per trait
(growth, yield, height, fruit, affinity, hardiness). Pollination by mature
flowers or wind within a radius, rolled by positional hash on fixed steps.
Crossing two species can yield a defined hybrid row (data, not random).
Sapling items need item data (alleles) → part of the §12 save bump.

Tree farm (multiblock, MM §6 footprint)
- Plants held saplings on marked soil, waits on the growth table, fells
  mature trees using the existing tree physics, collects logs/leaves/saplings
  into adjacent storage or the item network, replants.
- Runs on power (Machines pillar) or Toxic/Wet mana (Magic pillar) or by hand
  (craft domain only): one row, three energy routes.
- Better than Minecraft: whole-tree felling with real physics, species chosen
  for the job (fuel wood, resin, fruit, carbon sink, essence soak), breeding,
  and the farm's carbon and pressure effects shown live on its page.
- City woodlot jobs (`farming/jobs/production/woodlot.rs`) use the same species
  table and growth path.

## 10. Mob farms: engineered habitats

Existing `world/habitats.rs` already says "a source owns one resident and its
independent product cycle; combat is never approximated as production", with
feed and mana costs. MM §6 adds a captured-creature vessel. Combine them:

- A species row in `assets/bestiary.ron`/`spawns.ron` gains habitat
  conditions in field terms: element range, pressure range, temperature,
  moisture, block palette, food. **No darkness or light condition for farms.**
  Wild night spawning (`adventure/night.rs`) may keep its light rule; farms
  never read it.
- A farm is a built enclosure that meets a species' conditions plus a vessel
  or a caught founding pair. Production is husbandry: eggs, wool, feathers,
  venom, silk, shed scales, milk, breeding stock, at table rates per input.
  Killing is optional and follows GAME_DESIGN §10.1 method-yield rules.
- Blighted variants breed only where pressure is high: a player can farm them
  deliberately, at the cost of land health around the farm.
- The three existing families (Nursery, Spider, Matron) move from the Rust
  `Family` enum to data rows in this format (keep their saved ids).
- Automation never beats manual yield per input (MM §6 rule).

## 11. Link table and check (dev tool)

`assets/links.ron` (new, read only by tools and a startup validator). One row
per system:

```
(id: "smeltery", pillar: "automation",
 reads: ["heat", "matter", "mana:Fire"], writes: ["matter", "air:soot", "carbon", "land:slag"],
 off_fallback: { "magic": "fuel and lava only" },
 evidence: ["scenario:smeltery-alloy", "symbol:smeltery::tick"]),
```

`tools/axiom-dev/links-cli.mjs` (new; same structure as the existing
`*-cli.mjs` tools):
- `check`: fail when a system has no foreign read or no read-by-others write;
  when a quantity has a writer but no reader; when a pillar-off or discipline
  combination leaves a capability with no route; when a referenced symbol is
  missing from the source (grep) or a scenario id is missing.
- `graph`: print or emit the system/quantity graph (also shown on the dev panel).
- `orphans`: list mechanics in code that no row names (heuristic: modules under
  `world/` with tick entry points not referenced).
Runtime proof per link: a headless scenario through `factory-lab-cli.mjs` or
`engine-cli.mjs`, recorded in the parity ledger where a row exists.

Initial rows must cover every system in the inventory (§13).

## 12. Persistence

Prerequisite: the resource registry migration (own plan, own bump) lands
first. Then one save bump bundles: mana field pools and node
charge (MM); land pressure cells; world carbon record and Environment rule;
sapling alleles; habitat rows by name. Old saves: neutral pressure, carbon at
baseline, saplings with the default species' alleles, habitats mapped by id.
Test an old fixture, not only a new round trip.

## 13. System inventory and their link (initial `links.ron` content)

| System | Reads | Writes |
|---|---|---|
| Factory machines, power, conveyors, chips | matter, power, mana:Shock, heat | matter, air, carbon, heat |
| Industry parts (exhaust, chimney, filter, scrubber) | air | air (less), carbon (captured) |
| Smeltery, anvil, tool forging | heat, matter, mana:Fire | matter, air, carbon, land |
| Farming, crops, flowers, pantry, freight | water, land, heat, carbon (CO2 enrichment) | matter, mana (flowers), land (relief) |
| Trees, woodlot, tree farm | soil, light, climate, mana | matter, carbon (store), land (relief) |
| Habitats, bestiary, mobs, bosses | land, mana, matter (feed), heat | matter (products), populations, mana:Shock (Thunderhen) |
| Water, rivers, mill, pumps | weather, terrain | water, power (wheel), mana:Wet (via hedron) |
| Thermal, climate, weather, storms, volcanoes | carbon (ΔT), sky | heat, water (ice), mana:Fire/Frost/Shock (via hedrons) |
| Hedrons, collectors, circles, wands, runes, enchanting | mana, heat, water, power | mana draw, land (over-tap), matter |
| Brewing, cooking, alchemy (planned) | matter, heat, mana | matter, air (fumes) |
| Ruins, dungeons, Undercrown, gates, portals | power, mana | land (machinery), matter (salvage), knowledge |
| Civ, cities, trade, raids | land, air, carbon, standing | standing, matter (trade) |
| Space, moons, observatory | sky clock | mana (tide), knowledge |
| Scanner, storage, logistics, blueprints | matter, air, mana (sensors) | matter |
| Progression, discovery, folklore, records, goals | evidence events | knowledge |
| Multiplayer, edit history | all (shared deterministic state) | none new |

The builder completes this into exact `links.ron` rows from source; any system
found in code and missing here is added, never dropped.

## 14. Performance contract

- Carbon: O(1) per game hour.
- Field and land: dirty-cell updates on slow steps, within the existing
  simulation `Budget`; sleeping cells cost nothing; sparse caps stated above.
- Hedron condition: ≤256 cell samples per hedron per 600 steps.
- Pages: read-only views over existing state; no per-frame scans of the world.
- Each slice proves cost with frame-bench before/after on a fixed pose
  (interleaved A/B, P-core pinned).

## 15. Slice order (each slice playable and verified before the next)

1. `links.ron` + `links-cli check/graph` over today's systems (no game change);
   it lists the current orphans, which become the work list.
2. Process page template; anvil on it; smeltery controller page on it.
3. Carbon account + `co2`/`ch4` columns + gas analyser + carbon page,
   Environment rule (Off/Gentle/Realistic). Effects behind the rule.
4. Hedron condition coupling + celestial tide (A/B: supply equals today's at
   condition 1.0).
5. Land pressure grid; affinity/ambience read it; deposition, over-tap,
   mining and felling deltas.
6. Resource registry migration (its own plan), then the §12 bump; captured CO2 and mitigation rows
   (scrubber, mineralisation, biochar, greenhouse, flaring).
7. Tree species + alleles + pollination + tree farm.
8. Habitat rows to data + field conditions + husbandry products.
9. Migrate remaining pages to the template.
10. One-valley slice (GAME_DESIGN §11.2) as the integration playtest.

## 16. Left to the builder

Exact cell sizes inside the stated caps, step intervals, sampling patterns,
module splits, egui layout details within UI_RULES, test fixtures, scenario
names, and the numeric tuning of rates (within the stated targets).

## 17. Owner decisions still open

- Placeholder names for new blocks/machines/species (UI_RULES approval).
- Essence count and names (MM open decision) and whether Void is a sixth
  element alongside the five hedron elements.
- Environment rule default (this document assumes Gentle).
