# Biome overhaul: living biomes

Status (2026-10-25): design, written after the owner asked for "a biome overhaul
that is better than Minecraft or Noita, better than the sum of its parts" and
skipped the question round. Calls marked *(my call)* are the author's, for the
owner to check. Nothing here is implemented yet. Inputs:
[W-GEO/HYDRO/BIO](../../specs/W_GEO_HYDRO_BIO.md) (worldgen input of record),
[master plan](../../MASTER_PLAN.md) rows W12, W13, W17, W18, C12, C13.

## 1. Thesis

In Minecraft a biome is a palette plus a spawn table. It is beautiful at a glance
and inert: nothing in a jungle happens because it is a jungle, and after an hour
every biome has been seen.

In Noita a biome is a set of materials with chemistry and a danger curve. Every
pixel can react, so the world surprises you, but the layout is fixed, nothing
grows back, and deaths often come from causes the player could not read.

Here a biome is a **running system**: climate, ground, water and life producing
a process the player can see, learn, harvest, break and redirect. Its outputs
feed the game's existing loops (factory, wands, survival, the Primer). The
player is not a tourist passing through palettes but an engineer of ecosystems.

The combination neither reference has:

| Property | Minecraft | Noita | This game |
|---|---|---|---|
| Biomes look distinct | yes | yes | yes, readable from 500 m |
| Biomes do something | no | locally (materials) | yes: each runs a process |
| Placement makes sense | partly (temperature bands) | authored | climate x rock x water, learnable |
| Borders matter | no (hard edges) | no | ecotones are the richest places |
| Surface predicts underground | no | layered levels only | the column tells one story |
| World remembers | blocks only | no (run resets) | fire, clearing, drainage persist and recover |
| Danger is readable | mostly | often not | every hazard has a tell first |
| Feeds the economy | resources by biome | gold | processes power machines and spells |

## 2. Current state (2026-10-25)

- One `Biome` enum, 14 labels (`crates/worldgen/src/landscape.rs`), picked per
  column by a hard if-chain in `classify_landscape` from noise climate, height
  and slope. Generator 15 roughens borders with patch noise; there is no ecotone.
- A biome changes the surface table, relief profile and tree forms. It does not
  change creatures (bestiary has no biome field), weather (own temperature proxy),
  fog, sound (none), ores (tectonics only) or sites (except Amberroot).
- Runtime ecology: soil groundcover spreads and dies back (generator 42). No
  succession, snow accumulation or tree growth. Seasons tint foliage only.
- Session 04's generator 53 rescales climate to kilometre regions and adds lakes
  (`lakes::sample`, seven lake kinds) and river fit (`drainage::sample`).
  Generator 55 (session 00) adds underground biomes. This design takes **56**.

## 3. Goals, non-goals, anti-goals

**Goals**

1. Every biome is recognisable at a distance by silhouette, palette, sky and
   sound, and up close by its process.
2. Every biome runs at least one process that changes what the player can do.
3. Placement follows climate, rock and water so players can predict biomes from
   the map, and every seed still meets the variety guarantees of W-GEO §10.6.
4. Borders produce things neither neighbour has alone.
5. The surface biome predicts the ground, caves and materials beneath it.
6. Player actions leave marks that recover on believable timescales.
7. Each biome has a reason to go: a resource, a hazard with a tell, and a trick
   that pays off somewhere else.

**Non-goals (for this overhaul)**

- Tectonic plates and erosion simulation (W11/W13 own them; this design reads
  their outputs when they exist and today's fields until then).
- A full food web. Creatures get biome habitat and one ecological role each.
- Seasonal planet orbit and latitude climate (decision 0024 cube-sphere work).

**Anti-goals: failure even if it works**

- Biomes as noise patches: a swamp beside a desert because a cell said so.
- Palette swaps: twenty biomes that differ only in colour.
- Simulation everywhere: processes running in unseen chunks and costing frames.
- Unreadable death: a hazard that kills before the player could have noticed it.
- Lore inflation: new proper nouns or factions without the owner's approval.
- Banding: posterized shading or hard colour rings at biome edges.

## 4. Design pillars

### 4.1 Consequence, not paint

A biome is the output of a pipeline, never an input. Per column the generator
computes continuous fields: temperature, rainfall, seasonality, water-table
depth, substrate (rock and soil), exposure (wind, slope, aspect) and disturbance
(fire regime, flooding). Biome **weights** come from distance to family
prototypes in that space (a Whittaker diagram extended with substrate and water).
The `Biome` label stays as the argmax for the HUD, survey and old code.

Species (trees, shrubs, groundcover, creatures) have tolerance curves over the
same fields and are drawn by weighted lottery. Ecotones are therefore gradients
by construction: a forest thins into heath because oaks run out of rain, not
because a border was drawn.

### 4.2 Every biome runs a process

Each family owns one signature process built from systems that already exist
(cellular water, sand, gas, fire, heat, reactions, soil). The process is visible,
explainable in one sentence, and has a lever the player can pull. Processes run
at full fidelity only near the player; elsewhere they are summarised (4.6).

### 4.3 The border is the best place

Where two families meet, their processes interact. These interactions are rules,
not hand placement, so they appear wherever the pair meets: steam vents where
lava meets wet ground, gallery forest where a river crosses desert, salt marsh
where coast meets fen. A player who learns the rules learns where to look.

### 4.4 The column tells one story

Noita's strength is vertical: each layer has its own materials. Here every
surface family implies a subsurface stack (soil, regolith, rock, cave archetype)
so what you see above predicts what you find below. Meltwater from alpine snow
feeds the caves under it; volcanic fields sit above the hot deep; peat bogs sit
on drowned galleries. The existing underground zones (Ember Deep, Drowned Reach,
Hushed Ice, Sporefall, Prism Vault, Gilded Hush, Hollow Dark, Static Hall) become
the deep end of these stacks, coordinated with session 00's generator 55.

### 4.5 Tell before harm

Every hazard announces itself at least one beat early, through a sound, a
particle, a colour or a creature behaving oddly. Methane hisses and bubbles before
it ignites; snowpack creaks before it slides; heavy gas pools stain the grass
yellow and kill moss before they suffocate. This keeps Noita's surprise without
Noita's unfairness.

### 4.6 The world remembers, cheaply

Each 32 m ecology cell stores a few bytes: succession stage, fuel load, grazing
pressure, water-table offset, snowpack depth, last-visited tick. When a chunk
loads, its cells **catch up** deterministically for the time since the last visit
(closed-form or a bounded number of coarse steps), then voxels are regenerated
or edited to match. Burned forest is fireweed meadow a season later and birch
scrub a few seasons after that, without simulating it while the player is away.
This is the technique that makes a living world affordable on the Iris Xe target.

### 4.7 Reasons to go

Every family card (section 6) lists a resource, a hazard and a trick. The trick
is a physical interaction that is possible only here and pays off elsewhere, so
biomes become parts of the player's machine rather than a checklist.

## 5. How a biome is decided

```
fields(x, z)          temperature, rainfall, seasonality      (generator 53 climate, lapse rate)
                      water table, wetness, lake/river edge   (lakes::sample, drainage::sample)
                      substrate: rock family, soil depth       (tectonic surfaces, slope, strata)
                      exposure: wind, slope, aspect, altitude
                      disturbance: fire regime, flood regime   (derived from the above)
weights = softmax(-distance(fields, prototype_f) / width_f)    one weight per family
label   = argmax(weights)                                       HUD, survey, legacy callers
surface, flora, fauna = tolerance-curve lottery using fields and weights
```

- Prototypes and widths are data (`assets/biomes.ron`), not Rust tables.
- Keep the top two weights per column for blending surfaces, vegetation and
  sound across an ecotone.
- Special families (volcanic, karst) need their substrate; they cannot appear on
  climate alone. Amberroot remains a site placed on suitable families.
- Spawn rules (inland spawn, spawn disk) stay with their owners.
- Variety guarantee: a test generates 16 km² for many seeds and checks at least
  four families, two water kinds and one ecotone pair per region (W-GEO §10.6
  adapted). Failing seeds are a test failure, not a regeneration loop, until W19.

## 6. Biome families

Fourteen existing labels map to these families; two families are new
(volcanic field, karst upland) because the tectonic surfaces already exist.
Names are descriptive on purpose: new proper nouns need owner approval.

Each card: **Process** (always happening), **Resource**, **Hazard and tell**,
**Trick** (pays off elsewhere), **Reads as** (distance identity), **Below**.

**Temperate forest** (Forest)
- Process: leaf litter builds humus; deadfall rots into fungus logs; after fire,
  succession runs fireweed meadow to birch scrub to closed forest.
- Resource: timber, fungi on logs, humus for farms.
- Hazard and tell: crown fire in a dry spell; tell is crackling and drifting ash.
- Trick: charcoal from controlled burns in clearings fuels early smelting.
- Reads as: dark rounded canopy, shafts of light, birdsong by day.
- Below: root-threaded loam over limestone or granite; Sporefall caves.

**Temperate meadow** (Meadow)
- Process: grazing keeps grass short and flowers dense; without grazers, scrub
  creeps in over seasons.
- Resource: deep fertile soil, flowers and pollinators that raise nearby crop yield.
- Hazard and tell: few; open ground in storms (lightning on the tallest point).
- Trick: managing grazing turns any meadow into the best farmland.
- Reads as: open rolling green, flower colour bands, wind waves in grass.
- Below: deep soil, gravel lenses, shallow caves rare.

**Boreal forest** (Taiga)
- Process: permafrost under the moss; heat from a building or furnace thaws it and
  the ground slumps.
- Resource: resin and pitch (sealant for pipes, boats and tanks).
- Hazard and tell: subsidence under heated builds; tell is tilting posts and
  meltwater seeping from the floor.
- Trick: frozen ground is free cold storage until you heat it.
- Reads as: spiky dark conifers, pale lichen floor, deep quiet.
- Below: frozen soil over rock; Hushed Ice galleries.

**Tundra** (Tundra)
- Process: wind moves snow into drifts that change shape with the weather.
- Resource: ice blocks for building and cooling machines.
- Hazard and tell: whiteout; tell is wind pitch rising and far terrain fading.
- Trick: ice houses keep food and cool engines anywhere you carry the ice.
- Reads as: flat white and grey-green, patterned ground, long sightlines.
- Below: permafrost; Hushed Ice.

**Alpine peaks** (Alpine)
- Process: snowpack loads on lee slopes and releases as avalanches; meltwater
  pulses each afternoon.
- Resource: exposed ore and crystal on scree; reliable meltwater.
- Hazard and tell: avalanche; tell is creaking and small sloughs first.
- Trick: a triggered avalanche dams a valley or clears a pass; daily meltwater
  makes waterwheels run harder in the afternoon.
- Reads as: jagged silhouette, snowline, thin blue air.
- Below: fractured rock, meltwater shafts into Hushed Ice or Prism Vault.

**Desert** (Desert)
- Process: dunes migrate downwind; rare storms send flash floods down dry washes.
- Resource: sand for glass, salt pans, ruins uncovered as dunes move.
- Hazard and tell: thirst, flash flood (tell: distant thunder, wash darkening).
- Trick: mirrors focusing sunlight make a solar furnace that needs no fuel.
- Reads as: smooth dune crests, heat shimmer, gold sky at low sun.
- Below: sandstone, dry caves; Gilded Hush.

**Badlands** (Badlands)
- Process: erosion strips layers and topples hoodoos after rain.
- Resource: clay, ochre pigment, strata exposed for prospecting.
- Hazard and tell: collapsing overhangs; tell is pebbles falling.
- Trick: the colour bands in a cliff predict ore depth anywhere on that rock.
- Reads as: striped red and cream cliffs, hoodoo spires.
- Below: layered sediment; Prism Vault crystal pockets.

**Savanna** (Savanna)
- Process: dry-season grass fires renew the grass; herds follow the rains.
- Resource: fibre, herds, fire-hardened wood.
- Hazard and tell: fire fronts; tell is smoke columns and fleeing animals.
- Trick: backburning a firebreak protects a base and leaves rich ground behind.
- Reads as: gold grass, flat-topped trees, termite mounds.
- Below: red soil over granite; dry caves.

**Rainforest** (Rainforest)
- Process: constant growth; vines reclaim untended builds; humidity rots wood
  and rusts iron faster.
- Resource: rare plants (alchemy), latex, fruit.
- Hazard and tell: spore clouds and decay; tell is spore haze and fungus on walls.
- Trick: guiding growth along a frame grows a living bridge that repairs itself.
- Reads as: layered canopy, mist, emergent giants, constant insect sound.
- Below: thin soil over karst; Sporefall and Drowned Reach.

**Wetlands** (Marsh) and **Lowland fen** (Fen)
- Process: peat accumulates; methane seeps up through the water.
- Resource: peat fuel, reeds, methane captured in vessels.
- Hazard and tell: gas ignition and sinking mud; tell is bubbles and a hiss,
  and the drifting pale lights that are burning gas.
- Trick: draining a fen lowers the water table and exposes farmland and
  preserved ruins; lighting the gas on purpose clears a flooded passage.
- Reads as: flat reflective water, reeds, low mist, frogs at night.
- Below: peat over clay; Drowned Reach galleries.

**Highland heath** (Heathland)
- Process: wind keeps shrubs low; heather burns in patches and recovers.
- Resource: dyes, peat at the boggy edges, wool from hardy herds.
- Hazard and tell: exposure and wind chill; tell is gusts flattening the heather.
- Trick: windmills here run near full output most of the day.
- Reads as: purple-brown rolling moor, stone outcrops, open sky.
- Below: thin soil on granite; Hollow Dark.

**Coast** (Coast) and **Ocean** (Ocean)
- Process: tides expose flats twice a day; storms move sand and cut cliffs.
- Resource: salt from evaporation pans, shellfish and kelp in tide pools.
- Hazard and tell: being cut off by the tide; tell is water reaching the
  marker rocks.
- Trick: a tidal mill gives free power on a timetable.
- Reads as: surf lines, gulls, bright sand or dark cliff.
- Below: wave-cut caves, drowned galleries.

**Volcanic field** (new family; needs tectonic basalt)
- Process: geothermal heat; vents steam; heavy gas pools in hollows.
- Resource: sulphur, obsidian, steady geothermal heat.
- Hazard and tell: suffocating gas in still hollows; tell is yellowed dead grass
  and silent birds before the air turns.
- Trick: geothermal heat runs boilers and forges with no fuel; hot springs heal.
- Reads as: black ground, steam plumes, sparse ferns, orange night glow.
- Below: basalt, lava tubes; Ember Deep.

**Karst upland** (new family; needs limestone)
- Process: streams vanish into sinkholes and reappear as springs; cave mouths
  breathe cold air that makes fog on warm days.
- Resource: lime (mortar, cement), clear spring water, cave access.
- Hazard and tell: sinkhole collapse; tell is cracked ground and a hollow sound
  under footsteps.
- Trick: follow a vanishing stream underground to a hidden network.
- Reads as: white rock pavements, fog plumes from holes, sharp towers in the wet.
- Below: the deepest cave systems; Drowned Reach and Echo-style limestone naves.

**Amberroot** remains a site, placed only on families its rules allow.

## 7. Ecotone interactions

Rules keyed on the top two weights at a column and on materials meeting in the
simulation. Examples, all generated wherever the pair occurs:

| Pair | Produces | Why it matters |
|---|---|---|
| Forest and meadow | edge thicket: densest berries, birds, game | best early foraging |
| Desert and river or lake | gallery forest and oasis soil | farming in the desert |
| Volcanic and wetland | steam fields, sulphur springs, obsidian | best power site |
| Tundra or alpine and forest | krummholz treeline, wind shelter | first warm camp |
| Savanna and rainforest | fire stops at the wet edge | teaches firebreaks |
| Coast and fen | salt marsh: salt and peat together | preserves food, fuel |
| Alpine and desert | alluvial fan below the range | fertile, gravel, water |
| Karst and river | sink and resurgence pair | hidden routes |

Material reactions extend `assets/reactions.ron` (lava + water = obsidian and
steam, methane + fire = flash, acid + limestone = gas). Feature rules live in
`assets/biomes.ron`.

## 8. Senses and identity

- **Silhouette:** each family has one landmark form readable at 500 m (lone
  umbrella tree, hoodoo, snag, termite mound, steam plume, karst tower).
- **Palette:** per-family ground, canopy, water, rock and fog colours tuned for
  harmony and for contrast with likely neighbours. Blends are continuous across
  ecotones (no posterized rings).
- **Sky and fog:** weather and fog read the real climate fields instead of the
  separate proxy in `weather.rs`, so rain falls where rainforest grows.
- **Sound:** a per-family ambience bed from the existing synth (wind in grass,
  insects, frogs, creaking ice), crossfaded by biome weight.
- **Water colour:** from depth, turbidity and source (glacial milk, tannin-dark
  fen water, clear karst springs), shared with session a1's water shading.

## 9. Creatures as ecology

- `bestiary.ron` gains a habitat (preferred field ranges) and a role: grazer,
  browser, predator, pollinator, scavenger or burrower.
- Spawns come from biome weights near the player, plus the existing site
  encounters.
- Roles touch the ecology cells: grazers lower grass biomass, browsers slow
  scrub, pollinators raise crop yield nearby, burrowers loosen soil. Removing a
  role shifts the cell's succession, so hunting out grazers lets scrub take a
  meadow back.

## 10. Seasons and weather

- Seasons change what is possible, not only colour: lakes freeze into paths
  (session 36's river ice, generator 54), dry seasons open fords and raise fire
  risk, wet seasons flood lowlands and fill dry washes, snow builds on alpine lee
  slopes until it slides.
- Weather draws from the climate fields per region.

## 11. Knowledge loop

Each family has three Primer experiments: observe the process, use it, cause it.
Codex entries follow the tooltip contract (≤ 50 words, numbers from data). The
"tell before harm" signals are the vocabulary the Primer teaches.

## 12. Technical design

### 12.1 Data

- `assets/biomes.ron` *(new)*: family prototypes (field centre, widths), surface
  tables, flora and fauna lists with tolerance curves, ecotone rules, palette,
  ambience. A round-trip test like the other RON tables.
- Worldgen API: `biome::fields(cfg, x_m, z_m) -> Fields` and
  `biome::weights(cfg, &Fields) -> Weights { top: [(Family, f32); 2], label }`,
  pure functions, generator-gated at 56.
- Ecology cells (phase 4): 32 m cells, about 8 bytes each, owned by the planes
  manager (C14) with a save-version bump chosen at migration time.

### 12.2 Generation

- Generator 56 replaces the biome if-chain with weights for new worlds. Older
  generators keep the if-chain untouched.
- Resident and horizon columns share one policy (`column_from_landscape` today),
  so far terrain gets the same blends.
- The shader's `terrain_surface_climate` mirror must gain matching gates
  (session 04 is adding the 53 stage).

### 12.3 Runtime processes

- Near the player only, under existing budgets: methane seeps (gas), steam vents
  and heavy-gas pools (gas and heat), dune drift (sand, very slow), tide (sea
  level clock), snow loading and release (powder), vine growth on untended
  blocks (growth rule).
- Far from the player: ecology cells catch up at chunk load.

### 12.4 Performance targets

- Generation stays within the terrain bench gate (+8 % of
  `terrain_generate_8_chunks`) for phase 1; weights cost less than the if-chain
  they replace plus the lottery they feed.
- Runtime processes share the existing CA and environment budgets; no new
  unbounded pass. Catch-up is bounded per chunk load.

### 12.5 Ownership and coordination

- This overhaul: new `crates/worldgen/src/biome/` module, `assets/biomes.ron`,
  generator 56, the biome call site in `classify_landscape`.
- Not touched without asking: session 04's climate, carve and shore blocks
  (landscape.rs), lakes, drainage; session a1's trees and leaves shader (trees
  get species requests through a tree API); session 00's underground biomes
  (column coupling agreed with them).

### 12.6 Biome process contract (binding from phase 2; phase 1 must leave room in the schema)

Added 2026-10-25 from owner review. It fixes the data shape that makes the
decisions above enforceable; the section numbers inside it (§1 to §6) are its own.

### 1. Biomes are assembled, not placed
A cell's biome is the best-scoring entry in `biomes.ron` against the local fields:
`temp_mean, temp_range, precip, precip_season, drainage, soil_depth, lithology, slope, aspect, salinity, mana`.
Each biome declares tolerance curves (trapezoids) per field; score = product. Top two
scores give the blend weight; `closest-two by distance` is only the tie-break. Border
effects are computed from exported properties (§3), never from biome-pair tables.

### 2. Every biome exports a process with a deniable input and a routable output
```ron
Biome(
  id: "fen", display: "Mistfen", family: Wetland,
  tolerances: { temp_mean: (2, 6, 16, 22), precip: (700, 900, 2000, 3000), drainage: (0, 0, 0.2, 0.4), ... },
  exports: { heat: 0.0, water: 0.9, mineral: 0.1, fuel: 0.6, mana: 0.2 },   // for borders, §3
  substrate: Peat, rock: Mudstone, cave: FloodedTunnels,
  process: Process(
    input:  Field(water_table, min: 0.8),          // deny: drain it
    reaction: "peat_decay",                         // row in reactions.ron, runs on growth tick
    output: Gas("methane", rate: 0.02),             // route: cap vents into pipes
    vent: Surface(spacing_m: 12, jitter: 0.5),
  ),
  hazard: Hazard(
    trigger: Concentration("methane", 0.3),
    stages: [
      (cue: Audio("hiss"), visual: Shimmer, at: 0.3),
      (cue: Audio("whump"), damage: 2, at: 0.5),
      (lethal: true, effect: Explosion(strength: 3), at: 0.7, requires_ignition: true),
    ],
  ),
  trick: Trick(desc: "methane feeds burners and lights", unlock: Codex("fen_lights")),
  reclaim: Reclaim(rate_days: 30, suppressed_by: [Light(>=8), Heat(>=300)]),
  succession: ["burned", "meadow", "scrub", "fen"],  // states; durations in seasons
  palette: "fen", sound: "fen", sky: "mist",
)
```
Validator rules: `process.input` must be a field the player can change; `process.output`
must be a material or energy that at least one machine or recipe consumes; `hazard.stages`
must have ≥2 non-lethal stages before any `lethal: true`; `trick` must reference a codex
entry; `succession` states must be biomes or declared transient states.

### 3. Borders are computed
For a cell with blend (A: w, B: 1−w), border effect = f(|exports_A − exports_B|) over the
blend band (w ∈ 0.3..0.7):
- heat × water → steam vents, mineral terraces (travertine/sinter), obsidian where heat ≥ 0.8
- water × mineral → fertile alluvium (oasis farmland), salt crust if drainage < 0.2
- fuel × heat → fire-scar ecotone, charcoal seams
- mana × any → resonance anomalies (crystal growth, lights)
- cold × water → frost hollow, ice lens, thermokarst
The table of effect rules is small (≤ 12 rows) and keyed by property deltas, so adding a
biome adds borders for free. Phase 1 reserves `exports` in the schema even if unused.

### 4. Catch-up ("the world remembers")
Per 32 m cell: 8 bytes — `state: u8 (succession index), age: u16 (seasons), stock: u8 (process accumulator), disturbance: u8 (last event), reclaim: u8, pad`.
On load after Δt ticks: advance `age` by Δt/season; apply succession transitions in closed
form; integrate the process output into `stock` capped by storage (a vent cannot bank
infinite methane); apply reclaim unless suppressed at save time; then realize materials.
Closed-form only — no simulation while away. `catch_up_replay` scenario asserts that
leaving for N seasons equals running N seasons live within tolerance.

### 5. Sanity constraints
- Processes never produce net energy from nothing: every output has an input field that
  depletes or a solar/geothermal source declared (`source: Geothermal(heat_plane)`).
- Hazards obey physics: methane is the gas sim's methane; avalanche is a powder mass with
  support loss; permafrost slump is a thermal → material-state change. No scripted
  "hazard events" that bypass planes.
- Far-field cells (coarse bricks) carry `exports` and `state` so the horizon shows the
  right palette and smoke/steam without realization.

### 6. Phases 2–6, restated as deliverables against this schema
2. Processes for 6 launch biomes + validator + `biome_report` (process presence per region).
3. Derived borders + 12-row effect table + capture board.
4. Hazard stages wired to planes + warning audio cue per hazard + lethal flags per profile.
5. Catch-up + reclaim + succession with the `catch_up_replay` scenario.
6. Identity: palettes, sky/fog, sound beds, landmark naming hook from the wonder scheduler.

Reviewer's entry point: `biomes.ron`, the validator, `biome_report` output for three
seeds, and the capture board. If those four read as the design, the diff is secondary.
The phase table in §13 keeps its original order; where the two disagree, this contract
wins from phase 2 onward.

## 13. Phases

| Phase | Delivers | Save change | Depends on |
|---|---|---|---|
| 1 Placement | fields, weights, ecotone blending of surfaces and groundcover, volcanic and karst families, signature landmark forms, variety test, HUD label from argmax | none | generator 53 climate |
| 2 Identity | per-family sky, fog, palette, ambience sound; weather from climate | none | phase 1 |
| 3 Processes | methane seeps, steam vents, heavy-gas hollows, tides, dune drift, with tells | none (runtime only) | phase 1 |
| 4 Memory | ecology cells, catch-up, succession after fire and clearing, grazing | yes | C14 planes |
| 5 Creatures and column | bestiary habitat and roles, biome spawns, surface-to-cave coupling | maybe | phases 1 and 4, generator 55 |
| 6 Wonders | per-region set pieces from W-GEO §10 | no | W13, W19 |

Phase 1 is the first slice: it changes what every new world looks like and sets
up the data every later phase uses.

## 14. Acceptance

- Phase 1: variety test passes for 50 seeds; before/after captures from fixed
  poses on the review seed show readable families and smooth ecotones; terrain
  bench within the gate; old generator outputs unchanged (existing version tests).
- Every hazard has a documented tell and a capture showing it.
- Owner review of captures for "better than Minecraft or Noita" at the end of
  phases 1, 2 and 3.

## 15. Open questions for the owner

1. Approve the two new family names (volcanic field, karst upland) or supply names.
2. May hazards kill (Noita-style) or only harm and push back?
3. Should vines reclaiming untended builds apply to player bases, or only to
   ruins?
4. Phase order: identity (sound, sky) before processes, or the reverse?
