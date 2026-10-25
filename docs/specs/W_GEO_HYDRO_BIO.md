# W-GEO/HYDRO/BIO — Terrain, hydrology, climate, and biome generation specification

Scope: the generator for a variable-scale voxel planet in a Rust + WGPU engine, designed to be more beautiful and more entertaining than realistic worldgen, while being physically coherent enough that the hydrology, soils, and biomes are *consequences* of the terrain rather than painted on it. Realism is the grammar; drama is the sentence. Every rule below has a "heroic" dial that exaggerates it within physical plausibility, and a "wonder" budget that guarantees set pieces.

Reading order for the agent: §0 principles → §1 scales → §2 pipeline → §3–§9 stages → §10 beauty and entertainment systems → §11 runtime coupling → §12 performance → §13 acceptance.

---

## 0. Principles

1. **Consequence over paint.** Biomes, soils, rivers, coasts, caves are derived from climate × geology × time. No "biome noise" that places a swamp next to a desert because a Voronoi cell said so.
2. **Metric space, not cell space.** All features are defined in metres and seconds; the voxel scale (0.25 m, 0.5 m, 1 m, 2 m, 4 m…) is a sampling decision made at the end. A river is 14 m wide whether the brick is fine or coarse.
3. **Heroic dials.** Each physical process has a parameter set labelled `real` and `heroic` (e.g., relief ×1.6, meander amplitude ×1.4, waterfall height ×2, color saturation ×1.2) with hard caps where exaggeration breaks downstream physics.
4. **Guaranteed wonder.** Every 4 km × 4 km region must contain at least one *set piece* chosen to be rare, specific, and framed (see §10). Variety is scheduled, not hoped for.
5. **Everything is a place.** Named landmarks with roles, visible from a distance, reachable, with a reason to go.
6. **Deterministic, seeded, chunk-local after planning.** Planning stages run on coarse grids once per region; realization is per brick and depends only on planning outputs + seed + coordinates.
7. **Budgeted.** Planning ≤ 3 s per 16 km² on a 4-thread CPU; realization ≤ 4 ms per 32³ chunk on H1; coarse far-field realization ≤ 0.5 ms per coarse chunk.

---

## 1. Scales and grids

| Grid | Cell | Extent | Holds |
|---|---|---|---|
| G0 planet | 8 km | whole world | tectonic plates, continentalness, global circulation, ocean currents |
| G1 region | 1 km | 64 km window | uplift, climate, drainage basins, biome classes, wonder assignment |
| G2 local | 128 m | 16 km window | erosion result, channel network, lakes, coast class, soil classes, vegetation density, structures plan |
| G3 fine | 8 m | 2 km window | channel planform, banks, terraces, bars, karst entrances, tree instances, rocks |
| Bricks | 0.25–4 m | streamed | realization (materials, water columns, planes) |

Data flows downward only. A G3 feature (meander bend) is realized identically at every brick scale by sampling its metric geometry; at coarse scales features below the cell size are represented by *material summaries* (a 4 m coarse cell over a 1 m stream records "water fraction 0.25, bed material gravel, flow vector") so the far field still shows rivers, roads, and treelines correctly.

---

## 2. Pipeline (stages; each is a pure function of seed + upstream grids)

```
P0 tectonics & continents (G0)
P1 uplift, lithology, structure (G1)        — rock types, faults, folds, volcanism
P2 climate (G0→G1)                           — circulation, temperature, precipitation, seasons, wind
P3 erosion & drainage (G1→G2)                — stream power, glaciers, priority-flood, lakes
P4 hydrology network (G2)                    — Q, order, planform class, groundwater table
P5 coasts (G2→G3)                            — cliffs, beaches, deltas, estuaries, reefs, fjords
P6 channel planform realization (G3)         — meanders, braids, oxbows, bars, waterfalls
P7 karst, caves, springs (G2→G3)             — groundwater-driven cave systems, sinkholes
P8 soils & microclimate (G3)                 — soil depth/type, moisture, aspect, frost
P9 biomes & vegetation (G3)                  — community assembly, succession, disturbance
P10 wonders & landmarks (G1→G3)              — set-piece selection and placement
P11 settlements & routes (G2→G3)             — sites, road graph, lots (separate spec)
P12 realization (bricks)                     — materials, water columns, planes, instances
```

---

## 3. P0–P1 Tectonics, lithology, structure

- **Plates:** 8–20 Voronoi plates on G0 with velocity vectors; boundaries classify as convergent (mountain belts, volcanic arcs, trenches), divergent (rifts, mid-ocean ridges), transform (strike-slip valleys, offset rivers). Uplift map = f(boundary type, distance, age). Hotspot tracks produce island chains with age gradients (young volcanic cones → eroded atolls).
- **Lithology (G1):** layered strata model: per column a stack of (rock type, thickness, dip, age). Types with erodibility K and permeability: granite, basalt, sandstone, limestone, shale, marble, quartzite, volcanic tuff, glacial till, loess, alluvium, plus fantasy rock-tier materials from the strata lore (Founder crystal, Chorister resonance stone) placed by depth band. Folds (anticlines/synclines) and faults (normal, reverse, strike-slip) deform the stack → hogbacks, cuestas, fault scarps, rift valleys, offset streams.
- **Volcanism:** stratovolcanoes (steep, layered), shields (broad), calderas with lakes, lava fields (young = bare basalt, old = fertile), cinder cones, geothermal fields (hot springs, geysers, fumaroles, sinter terraces), lava tubes (cave stage), obsidian flows. Active volcanoes are runtime hazards (magma body + eruption rule).
- **Impact craters:** rare, with central peaks, rim lakes, ejecta rays, and exotic materials (lore hook).

---

## 4. P2 Climate (the engine of everything else)

- **Insolation** by latitude and axial tilt; **seasons** from tilt; day length by latitude.
- **Circulation:** three-cell model (Hadley/Ferrel/polar) → prevailing winds by latitude band; ITCZ migration gives monsoons; ocean currents from a simple gyre model with warm/cold coasts (west-coast deserts next to cold currents, east-coast rainforests).
- **Temperature:** base by latitude → lapse rate with elevation (6.5 K/km, `heroic` 8) → maritime moderation by distance to ocean → cold current/warm current offsets → urban/heat sources at runtime.
- **Precipitation:** moisture advected along prevailing wind from oceans; **orographic lift** drops rain on windward slopes, **rain shadow** leeward; convergence zones; monsoon seasonality; snow when temp < 0 with snowpack accumulation → spring melt pulse (feeds hydrology Q seasonality).
- **Wind:** vector field at G1 (prevailing + terrain channeling via valleys/passes) used by dunes, tree lean, fire spread, sailing, airships, pollution.
- **Climate zones (Köppen-like) emerge**; they are not placed. Exotic climates from lore: mana-rich highlands with "mana weather" (aurora storms), geothermal oases in polar zones, a permanent cyclone pinned over a wonder.
- **Microclimate (P8):** aspect (north/south slopes differ 2–4 K), cold-air pooling in valleys (frost hollows, fog), lake effect, canopy shade, cave breath (constant temperature air at entrances → fog, ice caves).

---

## 5. P3 Erosion and drainage

- **Fluvial:** stream-power incision `dh/dt = U − K(lithology)·A^m·S^n` (Braun & Willett implicit, m=0.5, n=1), 100–300 iterations at G1, then refined at G2. Produces drainage divides, V-valleys, knickpoints at hard layers (waterfalls), water gaps through ridges where antecedent rivers kept pace with uplift (dramatic gorges), wind gaps, stream capture events (barbed tributaries, elbow of capture — a classic landmark).
- **Hillslope:** linear diffusion + threshold landsliding (slope > angle of repose for the material → landslide scars, debris fans, landslide-dammed lakes).
- **Glacial:** where mean annual temp < threshold at elevation: ice accumulation model → U-valleys, cirques, arêtes, horns, hanging valleys (waterfalls into main valley), moraines (lateral/terminal; terminal moraines dam lakes), drumlins, eskers, outwash plains (braided rivers), kettle lakes, fjords where glacial troughs meet the sea; current glaciers as ice material with crevasses and meltwater caves; glacial lakes with ice-dam outburst floods as runtime events.
- **Aeolian:** dune fields where sand supply + wind + aridity: barchans, transverse, longitudinal, star dunes by wind regime; yardangs; loess deposition downwind of outwash.
- **Periglacial:** permafrost, patterned ground, pingos, thermokarst lakes.
- **Depressions:** priority-flood with lake retention (volume ≥ `lake_min` and inflow > evaporation → lake; else fill). Endorheic basins → salt lakes, playas, salt flats with polygonal cracks, evaporite minerals (resource).
- **Karst (P7):** on limestone/marble with sufficient rainfall: dissolution lowers surface, produces sinkholes, dry valleys, disappearing streams, resurgences, towers (fenglin/fengcong) in tropical karst, cenotes near coasts, collapse dolines; cave networks follow the water table and bedding/joint sets.

---

## 6. P4 Hydrology network and groundwater

- **Flow direction:** D∞ for accumulation; D8 for channel path. **Accumulation** uses precipitation − infiltration (by soil/lithology permeability) − evapotranspiration (by temp/vegetation).
- **Channels** where accumulation ≥ threshold(slope); **Strahler order**; **discharge Q** per segment with seasonal curve (snowmelt, monsoon, dry season). Width ∝ Q^0.5, depth ∝ Q^0.4, velocity from Manning (bed roughness by material).
- **Groundwater:** a water-table surface per G2 cell from recharge, permeability, and distance to channels/lakes. Drives springs (seep lines, contact springs over impermeable layers, karst resurgences, artesian springs where confined aquifers meet faults), wetlands (table ≥ surface − 1 m), baseflow (keeps rivers running in dry season), oases in deserts (fault-fed), and well behaviour for gameplay.
- **Lakes:** level = spill elevation; one outlet unless endorheic; thermal stratification seasons (ice cover in winter at cold latitudes); lake types: glacial, oxbow, caldera, landslide-dam, kettle, karst, reservoir, salt, mana-stilled (lore).
- **Hot springs & geysers** from geothermal fields intersecting the table: sinter terraces, colored microbial mats (temperature → color bands), geyser cycle timer.

---

## 7. P5 Coasts

Classify each coastal G2 cell by exposure (fetch × wind), tidal range (from ocean basin size), sediment supply (nearby river Qs), and lithology:
- **Rocky high-energy:** cliffs with wave-cut platforms, sea caves, arches, stacks, blowholes; cliff retreat scars; talus.
- **Sandy:** beaches with berm and dunes behind; spits and tombolos where longshore drift meets headlands/islands; barrier islands and lagoons on gentle shelves; tidal flats and salt marsh in sheltered low-energy coasts; mangroves in warm sheltered coasts.
- **Deltas:** Galloway classification by river vs wave vs tide dominance: bird-foot (river), cuspate (wave), funnel estuary with tidal channels and sandbanks (tide). Growth model with avulsion → 3–12 distributaries, abandoned channels as bayous, levees, crevasse splays, lagoons, chenier ridges.
- **Fjords, rias (drowned river valleys), skerries**, **atolls** (volcanic subsidence + reef growth), **coral reefs** (warm, clear, shallow) with lagoons, reef passes, and drop-offs; **kelp forests** in cold rich water.
- **Tides** at runtime: a global sea-level sine with range per coast class; intertidal zones flood/drain; tidal bores on funnel estuaries (a wonder candidate); rip currents at beach breaks (velocity field).
- **Sea stacks, arches, and caves are real geometry** the CA respects; storms (weather) erode at runtime by a slow reaction rule.

---

## 8. P6 Channel planform realization (G3)

Per segment, from slope S, sediment load, bank cohesion (soil/vegetation), and Q:
- **Step-pool / cascade** (S > 0.03): boulders, plunge pools spaced 3–4 widths, rapids classes (I–V) stored for gameplay (rafting), log jams in forested reaches.
- **Pool–riffle** (S 0.005–0.03): alternating deep pools on bends and shallow riffles, spacing 5–7 widths; gravel bars.
- **Meandering** (S 0.0005–0.005, cohesive banks): bank-migration model (Howard–Knutson): curvature → outer-bank erosion / point-bar deposition; wavelength 10–14 widths, sinuosity grows to 1.5–2.5; **neck cutoff** when neck < 1 width → **oxbow lake**, later marsh, later meander scar visible as a vegetation/soil ring; **chute cutoffs** in floods; scroll bars on point bars; natural levees; backswamps; crevasse splays; yazoo tributaries that run parallel to the main river for kilometres before joining (a landmark).
- **Braided** (moderate S, high bedload, weak banks — outwash, volcanic, arid): Murray–Paola cellular braiding → 2–6 threads, mid-channel bars, frequent splits; wandering gravel bars shift at runtime after floods.
- **Anastomosing** (very low S, cohesive, vegetated): stable multi-channel islands, wetlands, bayous.
- **Confluence geometry:** junction angle from tributary ratio; scour pool at junction; mixing plume (color/clarity material gradient when tributaries differ, e.g., clear meeting silty — the "two-color river" wonder).
- **Waterfalls:** at knickpoints and hanging valleys: plunge pool depth ∝ height; spray zone moss/ferns; cave behind the falls when a soft layer underlies a hard cap; multi-tier falls on layered rock; `heroic` dial allows 120 m single drops.
- **Whirlpools / eddies:** marked at sharp confluences, behind large boulders/piers, in constricted gorges at high Q, and in oxbow necks during floods; runtime vortex strength ∝ Q × curvature; a permanent maelstrom wonder at a tidal strait.
- **Islands in rivers** (mid-channel, vegetated on older bars), **fords** (riffles on gravel) and **natural bridges** (collapsed cave roofs over streams, resistant caps) are flagged for the route planner.

---

## 9. P8–P9 Soils, microclimate, biomes, vegetation

**Soils (P8):** depth from slope × erosion × time; type from lithology + climate + drainage: alluvium (floodplains, deltas), colluvium (foot slopes), loess, peat (wetlands), podzol (cold conifer), laterite (hot wet), chernozem (grassland), saline (playas), volcanic andosol (fertile), bare rock on steep/young. Soil feeds the fertility and moisture planes at runtime; farming is real.

**Biomes (P9) emerge from the assembly of:** mean/seasonal temperature, precipitation and its seasonality, soil moisture/drainage, soil fertility, elevation band, aspect, wind exposure, salinity, fire regime (dry season × lightning × fuel), flood regime, disturbance (landslide, volcanism), and lore fields (mana, resonance, Founder light). Communities are *lists of species with tolerance curves* (temperature, moisture, light, soil, salinity, mana) assembled per G3 cell by competition (shade tolerance, growth rate) → **succession state** (pioneer → mature → old-growth) that advances at runtime and resets on disturbance. Result: ecotones are gradients, not borders; edge effects exist (forest edge shrubs); gallery forests follow rivers through grassland; treelines are elevation × aspect × wind; fire-adapted savannas have scattered fire-resistant trees; floodplains show vegetation rings around oxbows.

**Biome families (emergent targets the validator checks exist somewhere in a planet):**
tropical rainforest with emergent giants and buttress roots; cloud forest (moss, epiphytes, perpetual fog at the lifting condensation level); monsoon forest; savanna with termite mounds and baobab-like trees; desert families (sand sea, rock desert/hamada, salt flat, cactus scrub, oasis); Mediterranean scrub (maquis, cork oak, fire-prone); temperate deciduous (seasonal color turn); temperate rainforest (nurse logs, ferns); boreal taiga (muskeg, lichen floors); tundra (patterned ground, dwarf willow); alpine (meadows, scree, krummholz); steppe/prairie (tall and short grass, wind); wetlands (marsh, swamp forest, peat bog with pools, fen, mangrove, salt marsh); riparian gallery forest; karst forest on towers; volcanic (young lava with pioneer ferns, old lava forests, geothermal mats); coastal (dune grass, maritime pine, kelp, reef); glacial foreland; cave biomes (twilight zone ferns, dark zone fungi); plus **lore biomes** that are physical: crystal forest (resonance stone outcrops growing crystal "trees" by mana precipitation), bioluminescent swamp (fungal mats where mana meets peat), petrified forest (silicified trees in volcanic ash beds), floating islands (rule-bearing Founder material with negative mass; they drift on wind; rivers fall off their edges), mana-weather highlands (aurora storms, glass-fulgurite fields), the Chorister ravines (shattered strata with resonance cliffs that hum in wind), the Deep's fungal cathedrals.

**Vegetation realization (P9→G3):** tree instances with species, age, height, lean (wind), crown shape; deadfall and snags; understorey; groundcover by light; flowers by season; fruit by species × season; fungi by moisture and dead wood. Trees are grown by the engine's growth rule at runtime (Dynamic-Trees style), so forests age, burn, regrow, and the player sees succession in long games.

**Fauna placement hooks:** community lists carry spawn tables (herds on grassland, waterfowl on wetlands, salmon runs up pool–riffle rivers in season, bats in caves) feeding the mob system.

---

## 10. Beauty and entertainment systems (the part that beats realism)

### 10.1 Wonder scheduler (P10)
Each G1 region draws one *major* and two *minor* wonders from a weighted catalogue conditioned on what the physics already produced there (a wonder must be physically motivated: a maelstrom needs a tidal strait; a crystal forest needs resonance stone). Catalogue (minimum 60; examples):
- Hydrological: two-color confluence; 120 m tiered waterfall with cave behind; a river that vanishes into karst and resurges 2 km away; a yazoo running beside its river; a braided outwash plain 3 km wide; a bird-foot delta with 12 distributaries; a tidal bore; a permanent maelstrom; a lake-within-an-island-within-a-lake; a hanging valley of seven falls; a travertine terrace cascade; a geyser field with a 60 m geyser on a 10-minute cycle; a glacial outburst flood scar; a natural bridge over a gorge; a water gap canyon 400 m deep.
- Geological: a caldera lake with an island cone; a basalt column coast; a slot canyon; a hoodoo badland; a fault scarp 200 m sheer; a salt-flat mirror; a cenote field; a karst tower forest; a crater with a central peak lake; an obsidian flow; a glacier with ice caves.
- Biological: a 90 m emergent tree; a bioluminescent bog; a redwood-class cathedral grove; a mangrove labyrinth; a cloud forest on a tepui; a petrified forest; a fire-scarred savanna with a single giant.
- Lore: floating island with a waterfall into the void; a resonance cliff that sings; a Founder-light meadow; a drowned Engineer dam still holding a lake.
Wonders are placed, then *framed* (10.3), *named* (10.4), and *given a role* (10.5).

### 10.2 Drama dials (per region, seeded, within caps)
`relief`, `valley_depth`, `waterfall_height`, `meander_amplitude`, `cliff_sheerness`, `color_saturation`, `fog_density`, `canopy_height`, `contrast_between_adjacent_biomes`. Dials are drawn from a distribution that guarantees variety across regions (no two adjacent regions share the same dial vector) and are capped where physics would break (slopes stay under material repose unless rock; rivers keep positive slope).

### 10.3 Composition and vistas
- **Vista points:** G2 cells with high visibility (viewshed over 2 km) get framed: foreground anchor (tree, rock), mid-ground subject (river bend, waterfall), background (ridge line), with a clear sightline carved through vegetation. Roads and trails are routed through vista points.
- **Approach sequencing:** the route planner (P11) ensures a wonder is revealed by a reveal moment (crest of a pass, exit of a gorge, forest edge) rather than seen from everywhere.
- **Silhouettes:** ridge lines are smoothed or sharpened per dial so the skyline reads at distance; lone trees on ridges; towers on karst.
- **Color design:** each biome has a palette (ground, canopy, water, rock, sky tint, fog) tuned for harmony and for *contrast with its neighbours*; seasonal palettes (autumn turn, spring bloom, dry-season gold, snow); water color from depth, turbidity, algae, and mineral content (glacial milk, tannin black water, tropical turquoise, hot-spring bands); rock color from lithology with weathering.
- **Light and atmosphere:** valley fog at dawn (cold-air pooling), god rays in canopy, aurora at high latitude/mana, dust haze downwind of deserts, sea spray at cliffs, mist at falls, heat shimmer on lava fields; each is a runtime volume effect driven by the planes, not a decal.
- **Sound** rides on the same data: river class → sound, canopy → wind, cliff → echo, cave → drip and reverb.

### 10.4 Naming and lore
A phonotactic name generator per civilization/region produces names for rivers, lakes, peaks, passes, falls, forests, deltas, and bays, with descriptive compounds ("Twocolor Water", "the Singing Scarp"). Names are stored on the hydrology graph and landmark list; the map, the text layer, and NPC rumour use them. Wonders get a one-line generated legend tied to the civilization lore (the Engineers built the dam; the Choristers cut the choir-canyon).

### 10.5 Roles and reasons to go
Every wonder and most landmarks carry a *role*: resource (geothermal power, evaporite minerals, clear-water fishery, mana well), mechanism (dam that can be drained, karst sink that drains a lake, bore that can be ridden), gate (gorge pass, fjord head, cave resurgence), lore (archive, ruin, mural), event (geyser schedule, seasonal flood, salmon run, aurora season). Roles feed quests, trade, settlement sites, and the Primer's experiments.

### 10.6 Variety guarantees (validator)
Per 16 km²: ≥ 4 biome families, ≥ 2 river planform classes, ≥ 1 lake type, ≥ 1 coast class if coastal, ≥ 1 cave entrance, ≥ 1 vista, 1 major + 2 minor wonders, ≥ 3 named landmarks with roles. Adjacent regions differ in dominant biome family. The validator fails generation of a seed that misses these; regeneration perturbs dials, not physics.

### 10.7 Playability constraints
Traversability: every settlement reachable on foot from the spawn without swimming > 60 m or climbing > 70°; at least one road or trail per region; fords or bridges within 2 km on any order-3+ river; resource accessibility: each tier-1 resource within 500 m of spawn, tier-2 within 2 km; danger gradient with distance and depth.

---

## 11. Runtime coupling (what the generator hands to the simulation)

- **Hydrology steady-state network:** per segment Q (seasonal), width, depth, velocity vector, bed material, source/sink faces, eddy markers. Runtime injects at sources, absorbs at sinks, keeps river cells asleep at target state, wakes disturbed reaches; dams, diversions, lake drains, floods, cutoffs, avulsions, delta growth all work via the CA + occasional budgeted re-planning of a local G3 segment.
- **Groundwater table:** per-chunk level feeding springs, wells, cave water, wetland moisture; drops when overdrawn.
- **Sea level and tides:** global clock; intertidal planes.
- **Climate fields:** temperature/precip/wind per chunk per season/day → weather stage; snowpack; fire regime.
- **Soil planes:** moisture, fertility, depth, salinity.
- **Vegetation state:** instances + succession stage + fuel load (fire) + fruit timers.
- **Geological hazards:** volcano state, landslide-prone slopes, karst collapse risk, glacier front.
- **Landmark registry:** names, roles, legends, vista frames → map, text layer, quests.
- **Far field:** coarse bricks with material summaries; rivers/coasts/treelines rendered from the network and instance summaries.

Player actions change the world: a dam creates a reservoir that drowns a forest (succession resets), drops the delta's sediment (shoreline retreats over seasons), and silts up (fertility). Draining a karst lake opens a cave. Clear-cutting a slope triggers landslides in the next storm. Pollution follows the river and tides. These are emergent from the systems above and are acceptance scenarios.

---

## 12. Performance and multi-scale realization

- Planning stages are coarse-grid CPU jobs (SIMD, rayon), cached per region on disk; G1/G2 ≤ 3 s per 16 km², G3 ≤ 150 ms per 2 km window, amortized while streaming.
- Realization per 32³ chunk: sample G3 features into the brick via analytic SDFs (channel cross-sections, terraces, dune profiles, cliff profiles) + noise detail bands; ≤ 4 ms on H1; GPU realization kernel for far coarse chunks.
- Feature-aware LOD: at coarse scales, features smaller than a cell contribute material fractions and flow vectors; at promotion to fine, the SDFs regenerate detail deterministically; conservation on water columns by the ADR'd rounding rule.
- Memory: G2 ≈ 64 B/cell → 1 MB per 16 km²; G3 ≈ 32 B/cell → 2 MB per 2 km window; instances streamed.
- Determinism: all stages use the positional hash; no floats cross stage boundaries in state (fixed-point heights in cm, Q in L/s).

---

## 13. Acceptance scenarios and reviews

- `hydro_basin`: all channels reach sea/lake; no channel climbs; Strahler orders plausible; Q increases downstream; one outlet per lake.
- `meander_cutoff`, `braided_outwash`, `delta_birdfoot`, `delta_cuspate`, `estuary_tidal`, `karst_vanish_resurge`, `hanging_valley_falls`, `glacial_fjord`, `dune_field_wind`, `salt_flat_endorheic`, `two_color_confluence`, `whirlpool_strait`, `tidal_bore`, `geyser_cycle`, `oasis_fault_spring`.
- `dam_reservoir_delta`: build a dam → reservoir fills to spill, downstream Q drops, delta retreats over 3 seasons, silt accumulates.
- `wildfire_succession`: dry season fire → pioneer regrowth → canopy return over N seasons.
- `variety_16km`: §10.6 guarantees for 100 seeds; `playability_spawn`: §10.7 for 100 seeds.
- `vista_review`: 20 generated vistas rendered and rated by human review each milestone against a reference board (target ≥ 4/5 median).
- `names_review`: 50 landmark names reviewed for pronounceability and distinctiveness.
- Perf: planning and realization budgets on H1; far-field coarse correctness (river continuity across LOD seams) golden images.

References for the agent (read before SDD): Génevaux et al. 2013 (hydrology-based terrain); Braun & Willett 2013; Barnes 2014; Tarboton 1997; Howard & Knutson 1984; Murray & Paola 1994; Galloway 1975; Leopold & Maddock 1953; Cordonnier et al. 2016–2018 (large-scale terrain with tectonics/erosion); Guérin et al. 2022 (coastal terrains); Cortial et al. 2019 (procedural tectonic planets); Peytavie et al. (arches/karst); Dreamworks "Ecosystem" and "Procedural generation of biomes" literature; Whittaker biome diagram; Köppen–Geiger; Minecraft 1.18 noise routers and Terralith for what players already find beautiful.
