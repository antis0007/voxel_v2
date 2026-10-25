# World, architecture, art and UI coverage

Source IDs: [inventory](../../specs/downloads-2026-10-24/README.md).
`L` = inspected partial foundation; `P` = planned; `D` = deferred.

## Five-biome plan (DL10, sections 1-9)

| Biome | Obligations retained | Master owners / state |
|---|---|---|
| Wisp Fen | Peat banks/deadwood, readable dark water corridors, living lanterns, pumping shrine, diversion/walkway/root-tunnel routes, peat/resin/bog iron/reagents | W01/W04/C06/P07; P for full biome/landmark loop |
| Skyglass Tarn | Dominant asymmetric peak, outlet/scree/meadows/conifers, slope/exposure snow, submerged observatory, multiple crossings and ridge mechanism | W01/W04/R05; P |
| Greenward Basin | Connected watershed, floodplains/buildable meadows, hedgerows/routes/settlement constraints, aqueduct/mill/workshop and independent base options | W01/W04/M01; L: valley/drainage/workshop; full region P |
| Frostbound Gorge | Incised river, supported cliff citadel, bridge/ledge/drainage approaches, readable interior, local explainable protection and excavation consequences | W01/W04/P09; P for complete citadel |
| Amberroot Wildwood | Age/lean/crown/damage tree families, root arch, fungal grove, hollow-tree descent, cultivation/spore/water/light interactions; readable without bloom | W01/C13/P07/R07; L: `worldgen/src/amberroot*`, woodland; full acceptance P |

Shared obligations: region seed/version -> ridges/basins/drainage -> climate ->
biome/habitat weights -> landmark/reveal envelopes -> density/strata/caves ->
structures/vegetation -> derived proxies. Keep bounded candidate selection,
canonical cross-chunk ownership, water outlets and route/visibility constraints.
Cache coarse plans; edits invalidate only dependencies. Preserve non-heightfield
landmarks, forests' canopy coverage, light selection hysteresis, localized fog
and depth-based water. All seven phases and their stopping gates remain in DL10.
The first 200-300 m Amberroot specimen does not certify all five biomes.

## Architecture master (DL02, sections 01-30)

All sixteen R01-R16 reference descriptions are retained in the original. Actual
reference inspection is a separate requirement; filenames do not prove artist
identity, ownership or availability.

| Architectural family | Retained identity | Owner / state |
|---|---|---|
| Pale Reach | Pale canal/cliff walls, inhabited terraces, great round tower with curved flared roof, waterfront support and circulation | W04/W02/R10; L: architecture builders; full family P |
| Crownstone | Terraced districts, small roofs, domes, civic tower/spire hierarchy and connected upper/lower city | W04/W02/N09; P |
| Vermilion March | Red plaster over masonry, exposed substrate, dark roofs, pale homes, gate and continuous wall walk | W04/R13; P |
| Solar Marble | Real apertures/columns/arches, processional axes, colonnades/statues, restrained gold and working water/solar purpose | W04/M01; P |
| Azure Collegium | Cool stepped towers, pale bridges, credible garden terraces, service routes and one working artifact | W04/G05; P |
| Waytowers/border remnants | Tapered isolated skyline landmarks, real entrance, localized support/magic, damaged crossings and cliff manors | W04/R10/P01; P |

The crimson natural arch R04 is an environmental frame, not a seventh building
culture. Historical layers may mix families deliberately, never by random trim.

| Section range | Requirements retained | Master owners / state |
|---|---|---|
| 01-06 | Mission, reference ledger, five-scale shape hierarchy, cultures, human proportions and actual-repo audit | W01/W02/W04; L/P |
| 07 | SitePlan -> BuildingPlan -> ConstructionParts -> ResolvedGeometry; topology before dressing; bounded deterministic compiler/fallback | W04/C02/C15; L: `worldgen/src/architecture*`; complete compiler P |
| 08-10 | Wall joins/openings, arch/pier/column/vault kits, stairs/rails, curved roofs/domes/tapered towers, facade/trim/balcony/ornament grammar | W04/R13; L/P |
| 11-12 | Foundations/cliffs/retaining walls/waterfronts/bridges/aqueducts; occupied room programs, stairs/service/furnishings, honest accessible extent | W04/C10/N09; L/P |
| 13-15 | Town street/plot/service/roofscape hierarchy; city districts/vertical routes/history; castle purpose, multi-route dungeon, combat and excavation | W02/W04/W05/N09/P09; P beyond existing bounded sites |
| 16-17 | Historical damage, meaningful fracture/rubble/weathering, persistent destruction, temples/apertures/obelisk/statue/magitech | W04/W07/C15; P for complete catalogue |
| 18-22 | Material layers/palette, neutral-light review, lighting/sky/water, ecological habitation/props/audio, functional site contracts, biome/reveal placement | R03/R05/R06/R07/R13/C21/W04; L/P |
| 23-25 | Cross-chunk owner, shared edges, non-heightfield proxies, coverage/publication/revisions, edits/save/large coords, measured budgets, authoring/gallery/debug tools | C02/C16/C19/W04/R10; L/P |
| 26-30 | All reference scenes, P0-P13 dependencies, geometry/navigation/save/motion acceptance, failure rules and immediate execution order | W04/R14/T03; P for complete acceptance; no tests run here |

All twelve required specimens remain separate pending deliverables:

| Scene | Reference | Required emphasis |
|---|---|---|
| S01 Gate of the Pale Reach | R01/R02/R14 | Roofed tower, 8-12 inhabited street buildings, courtyard/interior/gate/service route |
| S02 Cliff Manor | R14 | Terrain interlock, support, interior loop, broken crossing/service ledge |
| S03 Crownstone City | R05/R06/R13 | Connected districts; separately validated skyline variants |
| S04 Vermilion Fortress | R10 | Red masses/substrate wear, working gate/wall circulation |
| S05 Drowned Colonnade | R07/R09 | Water court, open column bays, close ruin, traversable routes |
| S06 Solar Aperture | R03 | Actual circular void, dais, meaningful conduit/mechanism |
| S07 Marble Threshold | R11 | Sculptural curved ribs/apertures in turntable and approach |
| S08 Long Waytower | R12 | Strong taper, empty setting, entrance, stable far silhouette |
| S09 Undercliff Harbor | R08 | Overhang clearance, inhabited waterfront/land and water access |
| S10 Azure Terraces | R15 | Connected upper/lower routes, gardens, artifact mechanism |
| S11 Sanctuary of the Colossus | R16 | Readable statue, processional/side routes, water court/public function |
| S12 Crimson Arch Setting | R04 | Natural geological frame and a fitting playable site |

## Painterly overhaul (DL03, sections 01-26)

Keep silhouette/value/space before surface detail. Scope includes the original
shoreline and dungeon, tree anatomy/age/crowns, stands/edges/clearings, understory,
bank/rock/soil cross-sections, palette, sun/sky/reflected light, cloud structure,
mist/depth, water/reflections/contact, shadows/motion, architecture/ruins,
seasons, procedural composition, wand/effects and shared edit/LOD ownership.
Owners W01/W04/R03-R08/R10/R13. L: woodland and shoreline/lighting/material
changes; [the painterly checkpoint](../../design/PAINTERLY_20261024_CHECKPOINT.md)
explicitly records rejected candidates and unfinished reference acceptance.

All thirteen specimens remain: V01 original shoreline; V02 original dungeon;
V03 old tree/flower woodland; V04 meditative wooded water; V05 rocky wet woodland;
V06 pastoral valley; V07 pale courtyard/ruined arcade; V08 columned interior;
V09 autumn bridge canal; V10 winter wetland; V11 formal reflective garden; V12 ornamental
flowering garden; V13 monument/storm light. P0-P9 remain distinct gates:
baseline; material/hall calibration; tree/stand laboratory; shoreline; water/sky;
architecture/dungeon; landscape expansion; seasons/gardens; edit/performance
consolidation; final reference review. A palette change cannot close the list.

## UI and multiscale world (DL22, sections 01-22)

| Sections | Scope retained | Master owners / state |
|---|---|---|
| 01-04 | Audit real screens, carved-stone/ink/ember language, readable body/pixel accents, DPI/layout/safe areas, icons and interaction states | P17/P19/R14; L: presentation/UI rules; complete review P |
| 05-07 | Compact HUD/status/inspection; inventory/container drag/transfer/search/equipment/tooltips; safe crafting/ingredients/discovery/production chains | P03/P05/P17/P18; L: inventory, recipe browser, staged crafting; every gesture P |
| 08-10 | Wand ordering/cost/relationships; building preview/material/yield/undo and scale controls; machine bottlenecks/process/circuit/overlay | G02/P08/M01/M07/P17; L: existing panels; complete functionality P |
| 11-12 | Menus/settings/maps/help/accessibility/rebinding; focus/back/input routing, stable identity/persistence and gameplay-owned actions | P19/P17/R15/C15/C16; L/P |
| 13-16 | Preserve all five biomes, architecture/painterly scope; region -> landform -> habitat -> landmark -> geometry -> detail, cross-region hydrology | W01/W04/C02/R10; L/P |
| 17-18 | Physical scale vs design scale vs render LOD; volume/mass/yield conservation, negative coordinates, subdivision/coarsening, coverage/interaction/revisions/save | C02/C15/C16/R10; L: 1/0.5/0.25 m presets; mixed-resolution proof P |
| 19-22 | Complete UI/world action routes, CPU/GPU/interaction latency and allocation budgets, P0-P8 packages and real-engine review | C19/P17/W01; P for full scope |

Preserve existing terrain, saves and controls. UI improvements may apply across
saves; revised generation applies to new worlds. Fixed per-world physical voxel
presets do not establish an editable mixed-resolution world. The environmental
and crafting slice in ADR0019 is partial evidence, not completion of DL22.

## Alien world visuals (DL21, phases 1-5)

Retain data-driven Hillaire atmosphere LUTs/earthlike and alien presets/sky
irradiance; vapor/condensate grid, weather map, injection/precipitation hooks,
Nubis-style cloud shape/light/shadow/history; connected macro terrain and cached
erosion/flow/sediment; ecology-derived material/flora templates and canopy LOD;
debug slices, matching captures and the requested world-visual data-flow doc.
Owners C07/C12/W01/R04/R06/R10/R13. L: atmosphere config/cloud shaders,
drainage/landforms/ecology. Full coupled weather/erosion/flora specification P.

Do not treat proposed grid sizes, 0.3/1.5 ms costs, compute-only effects or
"no silhouette pop" as measured facts. Authoritative weather needs stable
world ownership; a camera-centered visual grid cannot mint gameplay water.
All LODs sharing a seed is insufficient seam proof. GPU erosion needs a
determinism/halo/cache contract. Replace old fog only when every affected pass
and the no-compute fallback are covered, retaining working paths meanwhile.
