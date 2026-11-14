# Generator gate registry

`CURRENT_GENERATOR_VERSION` (`crates/voxel/src/config.rs`) is the save
compatibility contract: `generator_version >= N` must mean one feature set
forever, on every branch. Numbers are shared across branches and worktrees,
so a gate is claimed here before it is used in code.

## Claiming a gate

1. Take the lowest number in **Reserved** that no row uses, or the next after
   the highest row. Add a row with branch, feature, status, commit when known.
2. Grep every branch and the shared tree before committing the claim:

   ```
   git grep -n -E "version >= N[^0-9]|= N;" $(git for-each-ref --format='%(refname:short)' refs/heads) -- crates
   grep -rn -E "version >= N[^0-9]" crates
   ```

3. Dormant work keeps `CURRENT_GENERATOR_VERSION` unchanged and names its gate
   with a `pub const` (`river_ice::RIVER_ICE_GENERATOR`,
   `biome::BIOME_WEIGHTS_GENERATOR`) so renumbering is one edit.
4. The bump to a gate and the baseline re-save (`just bench-save`, golden
   hashes) land in one commit.
5. A collision is resolved by renumbering the dormant branch, never the
   committed or live tree.

Gates 1 to 53 are history (`landscape.rs`, `lib.rs`, `sites.rs`,
`dungeon/v2` at `VERSION = 32`); the rows below start where numbering stopped
being owned by one session.

## Rows

| 320 | Planet wayside finds: player-scale minor finds in 48 m cells on dry, level open ground (stone cairn with a survey cache tucked in its base, abandoned camp with an ash hearth, stone ring and log seats beside a salvage crate, standing-stone ring with an optional offering plinth, hollow fallen log with a cultivator cache in its open end); keeps clear of trails, rivers, caves, trees, settlements, the freight market, ruins, dungeons and surveyor sites (`wayside::WAYSIDE_GENERATOR`, `crates/worldgen/src/wayside.rs`) | local play density | claimed 2026-11-14; dormant, uncommitted; numbered with headroom above the 309 dune gates |

| 309 | Spatially filtered dune availability envelope | dune transition fix2026-11-14 | dormant; preserves306/308/default241 |

| 306 | Authoritative post-erosion sand deposits (dunes::DEPOSIT_GENERATOR) | dune improvement2026-11-14 | dormant; preserves305 and all prior generators |

| 305 | Coherent sand-sea ridge fields (dunes::COHERENT_DUNES_GENERATOR) | dune improvement 2026-11-14 | dormant, preserves303/300/241 |

| 303 | Wind-shaped dune profiles and filtered ripples (dunes::VERSION) | dune improvement 2026-11-14 | dormant, preserves 300 and default 241 |
| 304 | Planet rock relief on generator 300: solved bed resistance remaps hillside height into hard-bed risers, caprock lips and soft benches; dipped, warped bed frame notched by the 300 gullies (`ROCK_RELIEF_GENERATOR`, `planetary/rock_relief.rs`) | planet rock relief 2026-11-13 | claimed 2026-11-13; opt-in, preserves 290/300; no 303 dunes; inherits 301 earthport and 302 loot through their `>=` checks |

Gate 98 is claimed by growth-xmb / arch-meld (2026-10-30): arch legs flare into
the ground, the opening closes onto the floor (`vistas::FLARE_VERSION`), and the
span no longer roofs the ambient sky light beneath it. Gate 97 was already in
`vistas.rs`; all branches grepped for 98 first.

Gate 89 is claimed by growth-xmb / gate-approach (2026-10-30): buried hell
gate approach ruins (processional road, colonnade, toll house, quenched ring),
a taller ruined tower and a pilastered sanctuary (`hell_gate::APPROACH_VERSION`).
Versions 83-88 keep the bare tower and room. All branches grepped for 89 first.

Gate 85 is claimed by growth-xmb / freight-district (2026-10-29): Fernwake
freight hall, powered shutter, inhabited receiving bay and lower service loop.
Branch refs and shared source checked for gate 85 before use. Uncommitted.

Gate 81 is claimed by growth-xmb / maze-seals (2026-10-29): physical seal
anchors, trap courts and paired fold arches in new Manyfold worlds.

Gate 80 is claimed by growth-xmb / maze-districts (2026-10-29): seeded
cylindrical Manyfold maze districts. Uncommitted; affects only newly generated
Manyfold worlds. Versions through 79 retain the original dry tube.

Status: `live` (in HEAD and current for new worlds), `dormant` (committed,
below current), `uncommitted` (in the shared tree or a worktree only),
`planned` (doc only).

| Gate | Feature | Branch / owner | Status (2026-10-26) |
|---|---|---|---|
| 53 | Lakes, regional climate, smaller seas, river fit, climbable galleries | growth-xmb HEAD 8c258df8 | live |
| 54 | River ice (`river_ice::RIVER_ICE_GENERATOR`) | 95155596 | dormant |
| 54 | Castle composition, dungeon room sizes, settlement roles, workings, valley | shared tree, peer session | uncommitted |
| 55 | Forge room, workings floor, settlement well | shared tree, peer session | uncommitted |
| 56 | Settlement craft houses and paving | shared tree, peer session | uncommitted |
| 57 | Roof tiles, conifer classic form, terrain variety | shared tree, peer session | uncommitted |
| 58 | Settlement role 2 interior | shared tree, peer session | uncommitted |
| 59 | Boss access ossuary | shared tree, peer session | uncommitted |
| 60 | Undercrown observatory | shared tree, peer session | uncommitted |
| 61 | Valley profile, settlement role 2 | shared tree, peer session | uncommitted |
| 62 | Route bridges, drainage crest wetness, surface min reduce | shared tree, peer session | uncommitted |
| 63 | Undercrown observatory revision | shared tree, peer session | uncommitted |
| 64 | unused, reserved by the shared tree sequence | shared tree, peer session | uncommitted |
| 65 | Elevator monument base (`docs/design/ELEVATOR_MONUMENT_20261026.md`); `CURRENT_GENERATOR_VERSION = 65` in the shared tree | shared tree, peer session | uncommitted |
| 66 | Biome fields and family weights (`biome::BIOME_WEIGHTS_GENERATOR`), was 56 | biome-p1 | dormant, renumbered 2026-10-26 |
| 67 | Abandoned Anchor districts, industrial materials, dock machines (`ruins::VERSION`), was 55 | anchor-ruins d013536b | dormant, renumber pending |
| 68 | Underground 3D biome cells (`docs/plans/2026-10-25-underground-biomes.md`), was 55 | none | planned |
| 69 | Surface site kind picked by the region's biome (`sites::kind::BIOME_SITES_GENERATOR`) | sites-enum | dormant |
| 70 | Teaching site stair base only where the ground reaches it (`habitats_region::STAIR_GROUND_GENERATOR`) | habitat-stair-ground | dormant |
| 71 | unused: geo moved to 73 so that 72 could go live below it | none | free after 72 commits |
| 72 | Connected roofs: stair treads, store-roof rows, wing shells on the wall plate, kept wall plates, well corner posts (`toll_crossing::CONNECTED_STAIRS_GENERATOR`, `architecture::CONNECTED_ROOF_VERSION`); homestead variety (stone, roof, windows, mill tower, walled longhouse); dungeon engine works job and machined labyrinth with caches and a relic (`job::WORKSHOP_VERSION`, `maze::TECH_VERSION`); `CURRENT_GENERATOR_VERSION = 72` | growth-xmb shared tree | uncommitted |
| 73 | Planet terrain from a drainage solve: plate uplift, stream-power erosion, climate, rivers, permafrost (`geo::GEO_GENERATOR`, ADR 0038), was 71 | growth-xmb shared tree | dormant, renumbered 2026-10-28 |
| 80 | Grouped tree stands (`trees::stands::STANDS_GENERATOR`) | vista-perf | dormant (row added 2026-10-29 from a branch grep) |
| 80 | Manyfold districts (`manyfold::districts::VERSION`); `CURRENT_GENERATOR_VERSION = 80` | growth-xmb shared tree, peer session | live, uncommitted; collides with vista-perf stands |
| 121 | Planet worlds: latitude plus stretched two-octave climate, altitude-cooled cover bands (`planet::CLIMATE_VERSION`) | growth-xmb shared tree | live, uncommitted |
| 122 | Planet worlds: the spawn cube face runs the geo drainage solve and the hills column pipeline; placer gold and paydirt (`basin::VERSION`) | growth-xmb shared tree | live from 123, uncommitted |
| 123 | Planet worlds: body-metre fine relief and ridged ranges, hills strata and ore bodies below the soil (`planet::DETAIL_VERSION`) | growth-xmb shared tree | live, uncommitted |
| 125 | Planet worlds: dungeons, caverns and cisterns from `sites::cavern`; caves keep hills aquifer pockets instead of flooding below sea level (`planet::UNDERGROUND_VERSION`) | growth-xmb shared tree | live, uncommitted |
| 126 | Planet worlds: cave zones (magma pools, crystal, frozen, glowing fungal), gravel cave-in lenses, depth-tiered ore lodes with surface tells (`deep::VERSION`) | growth-xmb shared tree | live, uncommitted |
| 127 | Tree-only release: resolution-aware leaf relief from original gate 130 and branch-attached broadleaf crowns from original gate 137; near geometry, crown bounds and far proxy lobes share the release gate (`TREE_DETAIL_GENERATOR`). Existing 126 and custom-planet 136 worlds retain their original samplers. | growth-xmb tree-detail repair | live default, uncommitted |
| 128 | Planet worlds: hills authored sites (settlements, households, fernwake, amberroot, undercrown, vistas) through `authored_block` (`planet::SITES_VERSION`), plus Surveyor ziggurats of fused black stone with a relic chamber, density from the seed's planet profile (`surveyor::VERSION`, `planet_profile`). The dormant site threshold moved from 127 to 128 to keep the tree release isolated; existing supported 126 and 136 generation is unchanged. | growth-xmb shared tree | dormant for ordinary new worlds, uncommitted |
| 129 | Planet face: glacial-maximum cooling, cirques and horns, lateral and terminal moraines, outwash fans, kettle lakes, former ice cover, erratics, ice-cored pingos (`geo::LANDFORMS_VERSION`) | growth-xmb shared tree | dormant, uncommitted |
| 130 | Every tree crown shares one leaf surface: continuous broad leaf noise replaces hashed metre cubes; 1.1 m and 0.7 m leaf relief (up to 0.6 m, 0.36 m for needles, 0.21 m for fronds and willow curtains) only where the grid resolves it. Covers giant, woodland, open and high crowns, umbrella, weeping, whorled and padded conifers, cycad fronds and redwood tiers; mushroom caps unchanged. Generator 52 woodland lobes also split into a core clump and 2-4 satellites; giants fit 0.6 m inside their old reach (`trees::foliage::VERSION`) | growth-xmb shared tree | original authoring gate retained; also live in tree-only release 127 |
| 131 | Planet shell climate in degrees C from the spin axis (`engine_core::planet_frame`), circulation-belt rainfall, separate snowpack, ice sheet, polar desert and sea ice (`planet_climate::VERSION`) | growth-xmb shared tree | dormant, uncommitted |
| 132 | Classic crowns: broadleaf woodland back to the generator 47 crown masses, generator 52 skeleton crowns and landmark giants retired (`trees::classic::VERSION`, preview `VOXEL_REVIEW_CLASSIC_TREES=1` under `--benchmark-*`). Overlaps 130 on giant and woodland crowns: whichever goes live second must decide which wins | orbital-weather-integ branch `integration-20261103` | dormant |
| 133 | Meadow ground: dry drainage swales keep their turf instead of bare mud (they drew straight tan ribbons across meadows); trail gravel beds fray into a packed-earth band with a wandering edge before the turf (`surface::MEADOW_GROUND_VERSION`) | growth-xmb shared tree, grassland session 35f1f61f | dormant, uncommitted |
| 134 | Vista landmarks (hills): about half as many sites, per-peak butte heights, flank gullies, geo gully detail kept on flanks (platform, span and road stay graded), uneven beds of hashed hardness whose soft layers recede up to 1.6 m from steep faces only where the voxel grid resolves the recess, bare rock and scree cover by slope (`vistas::FORM_VERSION`) | growth-xmb shared tree | dormant, uncommitted |
| 135 | Planet shell sea floor: continental shelf, shelf break, steep slope and abyssal plain by distance past the coast; drowned valleys on the shelf, canyons on the slope (`planet_ocean::VERSION`) | growth-xmb shared tree | dormant, uncommitted |
| 136 | Planet customization dials (`engine_voxel::CUSTOM_PLANET_VERSION`) and planet face river corridors and woods: floodplain and valley walls sized by discharge, riparian rainforest, regional forest masses with clearings and understory bushes (`geo::corridor::VERSION`) | growth-xmb shared tree | uncommitted; already used by the custom-planet creation and save loader, preserve both meanings |
| 137 | Broadleaf woodland crowns as branch-attached foliage: tapered trunk and primary limbs around one open sector, secondaries and twigs by a pipe rule, oriented leaf masses with a protruding and a hanging lobe at every twig tip, undersides sagging by per-mass density; far proxies take one mass per branch group (`trees::structure::VERSION`). First tree-detail customized-planet release; minimum recipe version remains 136. Save and join acceptance retain 137 when later planet work advances the defaults. | growth-xmb shared tree | live in planets 137+ and tree-only release 127, uncommitted |
| 138 | Planet face altitude continuity: retain solved valley, river and riparian fields beyond the 32 m detail cutoff with footprint filtering; distant biome classification uses solved sediment (`geo::corridor::LOD_VERSION`) | growth-xmb / regional-art | dormant, uncommitted; 137 already used by woodland structure in shared source |
| 139 | Scale-aware wild bamboo groves on dense warm grass (`plants::VERSION`) | interrupted growth session / recovery | dormant, uncommitted |
| 140 | Stand growth history: correlated mature cohorts, shaded recruits and younger gap regeneration; shared age for broadleaf profile and branch skeleton (`trees::stands::VERSION`) | codex water/forest slice | dormant, uncommitted; no live save version bump |
| 141 | Planet woodland follows climate timberline and fixed metre-space shelter; warm uplands retain forests, exposed divides retain openings (`landscape::woodland::VERSION`) | regional-art continuation | dormant, uncommitted; branch refs and shared source checked; default remains 126 |
| 81 | Forest/meadow slope outcrops from the stratigraphic bed replace noise gravel patches; outcrops rise 0.5 m (`ground_exposure::OUTCROP_GENERATOR`) | growth-xmb shared tree | dormant |

## Reserved

84 is claimed by `manyfold::architecture::VERSION`: annular arcade, six roof
families, grouped planted ruins and recessed end-wall masonry. New-world
default is 84; Manyfold versions through 83 retain their previous sampler.
83 is used by `hell_gate::VERSION` in the shared tree.

82 is claimed by `fernwake::waterworks::VERSION` on growth-xmb: finite reservoir,
receiving basin, rebuildable timber stop and upper traversal loop. New-world
default included this change at 82; versions through 81 keep their previous
Fernwake court. Later generator defaults retain the waterworks gate.

74 is the spherical planet generator; 75 adds karst and shared-tree features.
76 integrates drainage terrain: glacial troughs, thaw slumps, anchored teaching
sites, seasonal thermal climate and the new-world default (GENERATOR_76.md).
77 to 80 are live in the shared tree (`CURRENT_GENERATOR_VERSION = 80`); 81
is claimed above; 82 through 85 are claimed above. See the newer claims below. Rows 54 to 65 belong to the peer session that holds the
shared tree's uncommitted worldgen edits; when that work commits, replace
"uncommitted" with the commit hash. If it lands under different numbers, the
rows above move with it and 66 to 68 stay as claimed.

87 is claimed by `manyfold::undercroft::VERSION`: buried service circuit,
listening chamber and abandoned waiting room. 86 was already the shared-tree
default when this gate was claimed; neither historical version is rewritten.
The shared default has since advanced to 88; check live source before claiming
the next gate.

100 was claimed for the dormant corkscrew strand. That gate moved to 102
so 101 can ship on the straight tube. Versions through 100 keep the 3.6 m
district-coloured partitions.

101 is `manyfold::TALL_WHITE_WALLS`: those same partitions are white plaster,
10 m tall, in both grid directions, around the whole shell. The hollow stays
open. Straight-tube gravity is unchanged. Versions through 102 keep that tube.

102 is `manyfold::strand::VERSION` (dormant): the corkscrewing tube replaces
the straight Stage-A tube. A helix centerline swells and narrows, with one
tangential branch and an outward-from-the-strand field that is zero on the
axis and g_wall at the floor. Lives behind the gate until the app-side gravity
wiring (`manyfold::acceleration` needs a cfg-bearing, version-gated signature)
and native traces land with the bump.

103 is `manyfold::INFINITE_MAZE`. White plaster walls, 10 m tall, on a floor that
does not end in x or z. Ordinary gravity. Versions through 102 keep the finite tube.

104 is `manyfold::shell::VERSION`. The shell continues along its axis and the
radius moves between 16 m and 32 m. White plaster walls are 10 m tall. Gravity
is outward at the local radius, zero on the axis. Versions through 103 keep
the shape they already generated.

105 is `manyfold::dream::VERSION`. The same shell
carries seven districts: limestone folds, bronze bell frames, a cistern, a
stair terrace, a repeating borough, a garden wing, and a low planted court.
A dark floor is solid, a stone floor hides a short room, and one wall can be
walked through. Versions through 104 keep the white shell.

106 is `hall::VERSION`: the Hall of the Crimson King on hills worlds.
Manyfold worlds at 106 keep the dream shell.

107 is `manyfold::shell::VAST`. The axis sits at y = 224 m. Radius moves
between 130 m and 270 m. Bays are 96 m along the axis and about 105 m
around at the mean radius. Walls are 64 m tall, 32 m in the guest wing and
24 m in the garden. Versions through 106 keep the 16–32 m shell.

108 is `manyfold::expedition::VERSION`. The shell gains a balcony stair,
a high ledge, a floor gallery, a walled street with a counter, a court pool
and bench, and a room past the existing gate plug. Those rooms scale with
the bay, so 109 keeps them. Versions through 107 keep the dream sampler.

109 is `manyfold::shell::GRAND`. Same axis.
Radius moves between about 75 m and 275 m, with a second swell so the
waist and the crest are not one sine wave. The shell spirals. Bays are
320 m long and about 180 m around. Dividers are colonnades. Two avenues
run the length of the drum. Every fourth bay is a piered vault. Odd
sectors carry a gallery 36 m above the floor, with a ramp and an opening
through the deck. The waist is narrow enough that the far floor is in
view. Versions through 108 keep the shape they already generated.

110 is `manyfold::maze::VERSION`. The bay after
the court gains a block maze and a cleared throat from the court to its
mouth. Two mouths mark the portal: one in the court bay, one inside the
maze. Versions through 109 keep the grand drum.

111 is `SEEDED_REALMS`. Hall hazard bands, choir ring and pillars shift with
the seed. Nether ground gains ribs, lava canals and camp mounds, sparing
the spawn disk and buried gates. Manyfold bays gain a crystal post and a
guardian. Versions through 110 keep the shapes they already generated.
The host's seed and generator version are what every client samples, and
the encounter id is that same grid cell.

112 is `manyfold::maze::FIELD`. The shell interior is a seeded plaster maze
on a limestone floor: walls about 11 m tall, corridors about 8 m, wrapping
the drum. Spawn and the two portal mouths stay open. The colonnade, gallery
and court dressing stay on versions through 111.

113 is `manyfold::maze::CLOSE`. The shell is a tighter stone maze: broken
wall heights, cobble at the foot, grass on a third of the floor, vines and
leaf clumps, and an amber pier on every other cell. Creatures stand in open
cells near the player. Spawn and the portal mouths stay open. Versions
through 112 keep the plaster field.

114 is `manyfold::maze::ROOF`. A stone ceiling
closes the drum 3.4 m above the floor, with amber set into that ceiling.
Spawn is an 8 m room. Every sixth cell is a grass chamber. Versions through
113 keep the open maze.

115 is `manyfold::maze::STOREY`. The same chambers
open to 7.2 m, under a 2.8 m stone cap, with amber in that cap. Corridors stay at 3.4 m.
Versions through 114 keep the single ceiling.

116 is `manyfold::maze::ROUTE`. Three-cell avenues
run along the axis every 18 cells, and three-cell rings cross them every 24
cells. Those roads are grass under the 7.2 m cap. A 2-by-2 yard sits off each
crossing. The hash maze fills only what the roads leave. Versions through 115
keep the unbroken hash.

117 is `manyfold::maze::HALLS`. The same roads
remain. Between them, walls 2 m thick stand every fifth cell, with a door in the
middle of each wall. Versions through 116 keep the hash in those blocks.

118 is `manyfold::maze::ARRIVAL`. Spawn is a grass
court 16 m across under the 7.2 m cap. Each of the nine cells at a road crossing carries an amber
post, about 2.5 m wide, from 1 m to 6.5 m inward. Versions through 117 keep the 8 m spawn
disk and unlit crossings.

119 is `manyfold::maze::ROOM` and the new-world default. Low corridors open to
5.6 m. Vines are a single voxel on the cell edge, not a slab through the room.
Versions through 118 keep the 3.4 m cap and the vine volume.



124 is `volcano::VERSION`, dormant. Planet plates come from 14 warped Voronoi
plates, each turning on its own Euler pole. Vents sit on a 2.6 km body lattice
and are weighted by convergence (stratovolcanoes, calderas) and spreading or
hotspots (shields), with cinder cones in both. Active craters hold a 3 m lava
lake and dormant craters and calderas hold water. Ground near active vents
turns to basalt, ash and gravel. Shell faces also get rift troughs and
collision welts; the solved spawn face (122) gets cones only, and no vent lies
within 4.5 km of spawn. Warm wet woodland patches turn to grove moss. Runtime
eruptions and quakes (`world/volcanism.rs`) run only on planets at 124 or later.
Versions through 123 keep their columns.

## Collisions found 2026-10-26

- 54: river ice (committed) and the shared tree's castle and settlement work.
- 55: anchor-ruins, the underground plan and the shared tree's forge room.
- 56: biome-p1 and the shared tree's craft houses.

- 136 (found 2026-11-05): customization and river corridors share this gate.
  Custom planet creation already writes 136, and the save loader accepts it.
  Preserve that combined feature set; renumbering customization would invalidate
  existing recipes. The row now records both uses.

biome-p1 moved to 66. anchor-ruins moves to 67 when it next rebases. The
underground plan takes 68 when it is built.

## Recovery integration (2026-11-05)

| Gate | Feature | Source | Status |
| --- | --- | --- | --- |
| 142 | Connected spherical terrain and climate | shared planet work | live planet gate |
| 143 | Hell thresholds and open-sky terrain | shared portal work | live planet gate |
| 144 | Weighted biome families, biome-aware ruins, abandoned districts and regional roads | df6228c5 / d69af9a3 / 44cbbf20 / fd920f9c recovery | integrated; new-world default; historical saves keep their stored version |

The recovered biome and district modules use 144, not their collided historical 55, 66, 67 or 69 gates. Existing versions through 143 retain generation behavior. The new default also enables previously authored gates 128 through 143. No test baselines were regenerated and no tests were added or run.

| Gate | Feature | Scope |
| --- | --- | --- |
| 149 | Wild sugarcane on warm, gentle freshwater margins | New Hills worlds only; cultivation uses existing plot saves |
| 154 | Sedimentary escarpments and exposed bedrock | growth-xmb / Planet geology | New Planet worlds and recipes |
| 157 | Fractured escarpments, gullies and talus aprons | growth-xmb / Planet geology | New Planet worlds and recipes |
| 162 | Layered rainforest trees, hanging vines, shrub thickets and humid slope cover | growth-xmb / Rainforest | New Hills and Planet worlds |
| 165 | Taller rainforest canopy and buttressed emergents with broader crowns | growth-xmb / Rainforest | New Hills and Planet worlds |
| 166 | Rainforest stand openings, shade fern colonies and leaf litter | growth-xmb / Rainforest | New Hills and Planet worlds |
| 167 | Amazonian rainforest canopy with ceiba crowns, towering angelim and buttress fins | growth-xmb / Rainforest | New Hills and Planet worlds |
| 168 | Fallen rainforest trunks and mossy nurse logs | growth-xmb / Rainforest | New Hills and Planet worlds |
| 169 | Rainforest palms, low figs, climbable lianas and forest-floor swales | growth-xmb / Rainforest | New Hills and Planet worlds |
| 177 | Climate-limited bamboo groves and habitat-weighted ground flora | Surface ecology | New Hills, Ridged and Planet worlds |
| 179 | Surveyed Anchor streets, industrial ruins, cargo yards and rebuilt transit spans | Ruin generation | New Hills and Planet worlds |
| 180 | Old-growth oaks, tall taiga spruce, redwood groves and larger relict crowns | growth-xmb / Forest ecology | New Hills and Planet worlds |
| 186 | Graded trail surveys around slopes (rounded switchbacks) and relative-neighbourhood road links | growth-xmb / Surface routes (dormant, `TRAIL_SURVEY_VERSION`) | New Hills, Ridged and Planet worlds |
| 182 | Distinct branching and foliage for old oaks, redwoods, spruce, cloud trees and plateau umbrellas | Forest ecology | New Hills and Planet worlds |
| 190 | Expanded Anchor cities with surveyed street blocks, room plans and building-specific salvage | Ruin generation | New Hills and Planet worlds |
| 200 | Anchor service cuts, carriage wrecks and collapsed city wings | Ruin generation | New Hills and Planet worlds |
| 185 | Aspen stands, cypress along wet ground and larger savanna acacias | Forest ecology | New Hills and Planet worlds |
| 194 | Broader mature crowns, spreading shrub colonies and larger understory plants | Forest ecology | New Hills and Planet worlds |
| 195 | Surveyed estuary outlooks and graded walking approaches | Landscape art pass | Explicit Planet preview at 195 only |
| 210 | Wild flora: kelp, seagrass, lily pads, cattails, sea oats, mangroves, berry bushes, wild crops, desert and cave plants (`engine_voxel::WILD_FLORA_GENERATOR`) | Flora pack 2026-11-06 | New Hills and Ridged worlds; Planet once its generator reaches 210 |
| 216 | Anchor ward grades, bent side streets, varied floor plans, freight halls, transit shelters and industrial ruin frames (`engine_voxel::ANCHOR_TERRACE_GENERATOR`) | Ruin generation | New Hills and Planet worlds |
| 206 | Coupled fluvial incision, sediment transport and deposition on the spherical drainage mesh | Planet hydrology | Explicit Planet preview at 206 only |
| 207 | Branch-attached foliage fans, upright sprays and hanging leaf curtains | Forest ecology | New Hills and Planet worlds |
| 221 | Connected distributary fans and bar islands on shallow river deltas | Planet hydrology | Explicit Planet preview at 221 only |
| 224 | Mobile-bed hydraulic solve across the lower river and coastal shelf | Planet hydrology | Explicit Planet preview at 224 only |
| 229 | Grounded Anchor vegetation, a marked relay route with two stairheads, and a through-route in the port wreck | Ruin exploration | New Hills and Ridged worlds, and Planet worlds at 229 or later |
| 230 | Mobile-bed river mouths with tidal deposition, intertidal mud and optical plumes from suspended fines | Planet hydrology | New Planet worlds |
| 232 | Conveyance-weighted coastal inflow and local fine sediment settling throughout the tidal domain | Planet hydrology | Explicit Planet preview at 232 only |
| 234 | The first flood records high-water marks and wet duration. Before the second flood, drainage over the changed bed steers the inlet and isolated pools settle fines | Planet hydrology | Explicit Planet preview at 234 only |
| 237 | Lower-valley flood handoff and event-scale sand transport on the evolving delta bed | Planet hydrology | Explicit Planet preview at 237 only |
| 245 | Cold and warm terrain stages, flood scour on exposed banks, and beach sand transport around the tidal waterline | Planet geology and hydrology | Explicit Planet preview at 245 only |
| 246 | Playable regional canyon where mapped strata control erosion, exposed rock, and the rock players mine | Planet geology | Explicit Planet preview at 246 only |
| 247 | Segmented fault belts and asymmetric deformation tied to moving plate margins; damaged rock feeds differential erosion | Planet tectonics | Explicit Planet preview at 247 only |
| 248 | Regional uplift history, episodic river incision and sediment-limited wind abrasion | Planet geology | Explicit Planet preview at 248 only |
| 251 | Jointed bedrock controls hillslope wash and incision, with a narrower downstream boundary | Planet geology | Explicit Planet preview at 251 only |
| 253 | Mixed woodland stands with dry-ridge pines, aspen groves, broad oaks, wetland cypress and willows, and more pronounced rainforest palm and fig niches | Forest ecology | Explicit Hills, Ridged and Planet preview at 253 only |
| 250 | Anchor district lines: the viaduct turns toward the braid exchange or a downstream district, ends in a buffer stop and an abutment, and breaks only over a river or ground below its footings; civilization holds favour the line out of town (`ruins::line::VERSION`) | Ruin cohesion | Dormant |

## Planet river and coast history

Preview 247 extends the same 240 million year kinematic plate reconstruction
as the earlier Planet recipes. The drift solve varies mixed-crust deformation
with the age of the oceanic side. At the present margin, coherent patches
along strike modulate asymmetric forearc uplift, trench subsidence, and the
steeper face of tilted continental blocks. The same displacement offsets
exposed and buried bedding outside the mapped canyon. Fault damage raises
erodibility before rivers route and favors loose debris on moderate slopes.

This is a reduced terrain deformation model. It uses Euler rotations, an
advected crust field and bounded deformation profiles. It does not resolve
mantle rheology, rupture stresses, shaking or specific historical earthquake
magnitudes. The 2026 slab experiments motivate varying forearc width with
geometry, and rupture simulations motivate coherent variation along the fault.
The Illapel terraces study found that long-term uplift there did not track a
single seismic cycle, so the preview does not interpret uplifted terrain as
evidence of one particular rupture.
[Slab geometry and forearc deformation](https://doi.org/10.1029/2026JB033850) ·
[Megathrust fault heterogeneity](https://doi.org/10.1038/s41467-026-71722-3) ·
[Illapel marine terraces](https://doi.org/10.1029/2025GC012781)

The offline 247 surveys found strongly damaged margin nodes on both seeds.
Seed 74 had 2,798 nodes above a damage weight of 0.35 and a high-uplift review
point at atlas `334805, 13875`. Seed 731 had 2,269 such nodes and a review
point at `19875, -8250`. The sampled deformation profile is bounded and spans
about −15 to +38 m before erosion at AXIOM's scale. Seed 74 repeated its
planet checksum and coastal sample hashes at 1, 0.5 and 0.25 m. These checks
exercise the generator, with native fault appearance and a fresh multiplayer
session still to be reviewed.

The version on a Planet recipe selects its terrain rules. The coastal previews
below use exact version matches for their experimental changes. The regular
Planet recipes at 230 and later use the tidal baseline unless an exact
preview selects a different coastal solve.

| Version | Change in generated terrain | Evidence and limits |
| --- | --- | --- |
| 142 | A spherical drainage mesh gave each river one receiver across cube-face boundaries. | Rivers shared watershed geometry across faces. The receiver graph alone could not split a mouth into several outlets. |
| 151, 163 | Coastal relief progressed from sampled shoreline profiles to a locally baked river-to-shelf surface. | Local relief was driven by the mouth and its shelf. Disconnected low spots did not establish a flowing outlet. |
| 175, 178, 195 | Estuary sections, landward approaches, then the opt-in estuary outlook and walking grade. | These versions shaped an estuary and its access using surveyed geometry. |
| 206 | Coupled incision and sediment transport on the drainage mesh, with deposition changing the bed for the next routing pass. | Captured watershed-scale sediment supply. Local distributaries still required a finer receiving shelf. |
| 221 | Shallow-water distributary fans and islands derived from surveyed outlet geometry and sediment supply. | An explicit geometry preview. Island placement was outside the hydraulic solver. |
| 224 | A mobile-bed solver moved water and two grain sizes across the river mouth. It admitted coastal tiles with a wet route to the sea. | The route check established one outlet. It did not measure separate river-fed branches. |
| 230 | Two tidal flood pulses, bank relaxation, wave drift and a tracer for river fines. Wet duration and fine accretion distinguished intertidal mud, and suspended fines drove the plume. | Seed 16 produced detached high ground in the earlier terrain survey. That survey did not verify flow on both sides. |
| 232 | The inundated inlet apportioned the same total water and sediment by conveyance. Slack, shallow water retained more fines, and mobile sand followed the bedload current. | The inlet sensed its local cross-section. It could still feed a route that had shoaled farther downstream. |
| 234 | One downstream resistance pass used the first flood's crest, wet duration and changed bed. It reweighted the second inlet and settled fines in pools stranded at ebb. Inland floodplain meanders also use gate 234 or later. | A two-outlet replay shifted from 50/50 to 18/82 when a bar closed one route. The coastal handoff alone took a median 0.62 ms in nine generated bakes. Persistent coastal splits were still unverified. |
| 245 | Six climate stages reuse the existing drainage mesh and 18 river erosion passes. Ice quarries and transports rock, till covers the bed, and wet thaw slopes fail downhill. Flood scour cuts low, exposed banks based on local channel direction and bend curvature. Mouths follow the surveyed valley floor and widen in proportion to grade. Sand moves around the current tidal waterline with swash tied to offshore exposure. | A representative history with compressed events. Its parameters need field calibration before interpreting the stages as a timeline in years. |

Preview 245 kept the sampled coastal outlets connected on seeds 16, 74, and
731. The coastal water and sediment residuals rounded to zero at six decimals.
Two independent runs of seed 74 matched the world and sampled voxel hashes at
1, 0.5, and 0.25 m. The full development bake took 22–27 seconds across
these runs, including the existing regional erosion bake. Surveyed mouth
tiles took 1.34–2.29 seconds. These are generation timings on a shared build
machine. The wave work runs only on baked mouth tiles and moves existing sand
with paired transfers. The glacial coupling follows the observed importance of
ice-derived sediment in subsequent fluvial incision and the interaction of
postglacial bank erosion with slope failure.
[Glacial–fluvial coupling](https://doi.org/10.1029/2024JF007721) ·
[River and landslide feedback](https://doi.org/10.1029/2024JF008249)

The native 1 m seed-74 review used the same checksum as the terrain survey.
The fixed 1280×720 view measured 5.27 ms at the 95th percentile on an RTX 4090.
The approach is still fairly straight in the comparison and some hard joins
remain visible on the rendered water surface.

Preview 237 advances sand between the two hydraulic floods. The lower valley
receives the catchment's discharge close enough to the mouth for the
compressed flood to deliver sediment. The first flood records peak
sediment-bearing transport on the existing eight-step sampling cadence.
Submerged rises can retain sand when the measured flow has faster lanes on
both sides. Six sediment-only sweeps route the remaining reserve and sand
scoured from the loose bed. The reserve uses up to 2% of the local catchment
sediment allowance. Each sweep follows the measured direction and uses the
changed bed to determine which sills pass sand. The second hydraulic flood
runs over the resulting surface at the existing step count. The 4 m and 8 m
bakes are cached and sampled in physical metres, independently of voxel size.

### Preview 237 evidence

An opt-in audit follows the river sediment tracer from the inlet to deeper
receiving water at 66%, 76%, and 86% of the bake. A split needs a dry interior
islet at final drawdown and paired submerged lanes across at least two
cross-sections. Each counted lane stays connected at all three snapshots and
has measurable seaward flow at two or more of them. This permits a brief
tidal reversal. Each new islet below consists of cells that started below
sea level.

| Seed | Preview 234 splits | Preview 237 splits | New split islet centres in atlas metres |
| --- | ---: | ---: | --- |
| 74 | 0 | 2 | `141554.4, -14651.5` and `141638.2, -14612.2`, both 48 m² |
| 2025 | 0 | 1 | `77913.7, 23673.9`, 48 m² |

Seeds 16, 731, 2026, 733 and 9000 did not meet the persistent split criterion
in this comparison. The audit measures the second flood and the baked
drawdown. The game retains that surface in the generated world.

The new sand pass took about 0.5–0.7 ms on the two successful 4 m tiles.
Six paired CPU measurements per seed on identical input terrain measured the
complete coastal bake, including flood recording and changes in the wetted
area. Seed 74 used 1.9% more CPU time and seed 2025 used 3.9% less. All three
voxel scales produced the same physical sample hash for each seed.

The native daylight approach used seed 74 at 1 m voxel scale, clear weather,
and a camera at atlas `141560, -14535`, 95 m above the world datum. The fixed
surface clock was day 5.63542. The frame shows the small offshore bar and
the mouth. Hard joins between some of the rendered shoreline surfaces are
still visible.

To reproduce the audit locally, set `VOXEL_DELTA_AUDIT=1` and run
`cargo run --offline --profile release-fast -p engine_worldgen --example
morphodynamics_review -- 74 237`. The example accepts the seed and recipe
version as its final two arguments. The optional audit leaves the generated
surface unchanged.


### Mountain erosion review gate

| Gate | Feature | Owner | Status |
| --- | --- | --- | --- |
| 252 | Hills/Ridged erosion in physical metres after relief mapping: coupled drainage incision and conservative slope transport (`geo::MOUNTAIN_EROSION_GENERATOR`) | terrain-task10 | opt-in review, uncommitted 2026-11-06 |

Gate 252 review evidence: [Mountain erosion review](MOUNTAIN_EROSION_REVIEW.md). Fine relief also fades continuously at the 32 m footprint boundary; current defaults and persistence formats are unchanged.

| 253 | Hills/Ridged fracture-guided ravines and natural landmark relief; preserves generator 252 (`geo::RAVINE_GENERATOR`) | terrain-task10 follow-up | opt-in review, uncommitted 2026-11-06 |

| 254 | Bounded thousand-year settlement chronicle, physical geography provenance and causal archive records (civilization::CHRONICLE_GENERATOR) | history-task13 | opt-in review; integrated source, native library tests passed; see CAUSAL_HISTORY_REVIEW.md |

| 255 | Connected headwater incision and alternate walking routes (`planetary::CATCHMENT_EXPLORATION_GENERATOR`) | Planet catchments | Opt-in Planet preview. [Comparison and reproduction](CATCHMENT_REVIEW.md#whole-catchments-and-exploration-routes-in-preview-255) |
| 256 | Moving bedload abrades exposed bedrock and seasonal weathering replenishes upland regolith (`planetary::SEDIMENT_WEATHERING_GENERATOR`) | Planet erosion | Explicit Planet preview at 256 only |
| 257 | Connected valley floors, resistant bed steps and retained sand and gravel (`planetary::VALLEY_FORM_GENERATOR`) | Planet catchments | [Opt-in Planet preview](CATCHMENT_REVIEW.md#connected-valley-floors-in-preview-257) |
| 265 | Runoff concentrates incision below resistant bedrock sills and widens larger reaches selectively (`planetary::SILL_EROSION_GENERATOR`) | Planet catchments | [Opt-in Planet preview](CATCHMENT_REVIEW.md#erosion-below-bedrock-sills-in-preview-265) |
| 271 | Sediment abrasion and composed terrain gradients (`planetary::DRAINAGE_RELIEF_GENERATOR`) | plateau-task10 | integrated opt-in; 13 shared tests pass; [review](PLANET_DRAINAGE_GRADIENT_REVIEW.md); renumbered after concurrent 270 claim |



| 285 | Opt-in 1 Myr plate history, bounded replay and timestep-consistent island weathering (planetary::FINE_HISTORY_GENERATOR) | dot geology refinement | claimed 2026-11-08; default remains 283 pending validation |
| 287 | 720 Myr continental assembly/breakup, clustered volcanic provinces, carbon feedback and extended stratigraphy (`DEEP_TIME_GENERATOR`) | deep geological history | claimed 2026-11-08; implementation candidate |
| 288 | Generator 287 history plus a fractal, rock-dependent shoreline: coastal-band domain warp of the solved surface (`FRACTAL_COAST_GENERATOR`, `planetary/coast_roughness.rs`) | planet coastlines | claimed 2026-11-12; new Planet default; 287 history and geology cache path shared |
| 291 | Undercrown wilds: the Rift ravine, the Hollow Sea lake cavern, the sunken grove, and bedded rock in place of masonry on cavern walls (`undercrown::v2::wilds::VERSION`) | Undercrown overhaul | claimed 2026-11-12; dormant, uncommitted; walk tests pass with `UC_GEN=291` |
| 289 | Generator 288 plus process shorelines: Ashton-Murray one-line longshore transport with wave shadowing on sandy coasts, and rock-grain-aligned coast warp on structured coasts (`PROCESS_COAST_GENERATOR`, `planetary/shoreline.rs`) | planet coastlines | claimed 2026-11-12; becomes Planet default once reviewed |
| 290 | Generator 288 coasts plus clean riverbeds and shores: river gravel only on wet channel or bars, grey pond cover only under 0.5 m+ water, beach rock only on true cliffs, sand on gentle shores, dry placer bands keep turf, upland rock above slope 0.75 (`CLEAN_RIVERBED_GENERATOR`) | planet surface cover | claimed 2026-11-12; new Planet default |
| 292 | Undercrown props: a distinct voxel model per prop kind in place of the shared pedestal, with the focus box taken from the model (`undercrown::v2::props::VERSION`) | Undercrown props | claimed 2026-11-12; dormant |
| 293 | Planet ground breaks: slot ravines with ramped ends and benches, fissure swarms parallel to spreading plate boundaries, and sinkholes on carbonate ground, all 3D carves through `planetary::cave_carves`; keeps generator 290 terrain (`GROUND_BREAK_GENERATOR`, `planetary/ground_breaks.rs`) | terrain events | claimed 2026-11-12; opt-in Planet preview, uncommitted |
| 294 | Dungeon quest layer phase 1: mission graph over the dungeon v2 room tree; small-key locked doors on the critical path, keys in side-branch caches, big key in the big chest, boss door on the Core; lectern hints (`dungeon::v2::quest::VERSION`, spec docs/superpowers/specs/2026-11-12-dungeon-quest-design.md) | dungeon quest | claimed 2026-11-12; dormant; save 72 adds LockedDoor and BossDoor |
| 295 | Living planet surface on generator 290 terrain: province fine relief (rolling ground, knolls, dells), rocky shore skerries and coastal dunes, open woodland between forest stands, climate ground plants, and metre-sized multi-voxel plant stamps at sub-metre voxels (`LIVING_SURFACE_GENERATOR`, `planetary/fine_relief.rs`, `flora/stamps.rs`) | planet surface and plants | claimed 2026-11-12; opt-in Planet preview, uncommitted |
| 296 | Lowland rivers on generator 290 terrain: coastal warp and shore shift lower river corridors with the surrounding plain instead of leaving them perched; channel stage and floodplain ceiling follow that drop; low natural levees (crest <= 1.4 m) fall to backswamps that hold marsh water near the water table (`LOWLAND_RIVER_GENERATOR`) | planet rivers | claimed 2026-11-12; opt-in Planet preview, uncommitted |
| 297 | Planet rift valleys on generator 290 terrain: continental divergent margins get a faulted graben in the height solve (flank uplift, a master border fault whose side flips along strike, tilted floor blocks, transfer-zone saddles), with the sub-grid scarp restored per column; includes the 293 ground breaks and the 295 living surface (`RIFT_VALLEY_GENERATOR`, `planetary/rift_valley.rs`) | planet tectonics | claimed 2026-11-12; opt-in preview, Planet default stays 290 |
| 298 | Planet dike swarms on generator 297 terrain: near-vertical basalt sheets that cut the strata, striking along rifting plate boundaries and radiating from volcanic vents; one en echelon segment per 192 m cell, 40% cropping out (`DIKE_SWARM_GENERATOR`, `planetary/dikes.rs`) | planet igneous structure | claimed 2026-11-12; opt-in preview, uncommitted, not yet captured; Planet default stays 290 |
| 299 | Anchor city outskirts: past the kerb the city streets carry on as lanes that fray into the grass, with timber poles on one verge; lots thin out exponentially with distance from the deck (houses near, then sheds, tanks, foundations, rubble), levelled on natural ground and dropped where it is steep or wet; a share of houses and sheds hold a household or workshop cache (`ANCHOR_OUTSKIRTS_GENERATOR`, `ruins/outskirts.rs`) | Ruin generation | claimed 2026-11-12; opt-in preview, uncommitted; map A/B via `examples/outskirts_preview.rs`, not yet captured in game; defaults unchanged |
| 300 | Planet carved surface on generator 298 terrain plus the 296 lowland rivers: a slope-aligned erosion filter below the 8 m erosion tiles (branching gullies and rills that follow the solved drainage gradient, deeper in weak cover, chutes in hard rock), bed-controlled benches on steep rock, terracettes on grassed slopes, and ridge/crease cover (bare rock on sharp ribs, debris in creases) (`CARVED_SURFACE_GENERATOR`, `planetary/carve.rs`) | planet fine detail | claimed 2026-11-12; opt-in Planet preview, uncommitted |
| 301 | Earthport on the generator 65 anchor monument: three 210 m flying fins and a crown cage above ground; below it a domed anchor vault with the tether root, strain frame, six tendons and anchor blocks, the six buttress caissons as hall piers, a mezzanine, a one-turn helical freight ramp from a terrace portal, a substation hall with a stair from the control wing, a sump and two vent stacks (`meridian_terminal::earthport::VERSION`, `crates/worldgen/GENERATOR_301.md`) | elevator earthport | claimed 2026-11-12; dormant, uncommitted; `earthport_section` example renders sections |
| 302 | Inhabited Undercrown, first slice: banded strongboxes and reading stands in the Drowned Borough (prop index 2), each a one-shot claim from `assets/undercrown_rewards.ron` that can add a found spell or mint a seeded found wand (`undercrown::v2::life::VERSION`, `undercrown/v2/life.rs`, spec `docs/superpowers/specs/2026-11-13-undercrown-inhabited-design.md`) | Undercrown | claimed 2026-11-13; dormant, uncommitted |

## Gate constants in source

<!-- gates-cli index: begin (generated, do not edit) -->
| Gate | Constant | File | Source note |
| --- | --- | --- | --- |
| 18 | `VERSION` | `crates/worldgen/src/surface.rs` | Surface worldbuilding rides the current v18 baseline. It is sampled only in newly generated or unedited chunks; resident edits always win, as they do for every other generator layer. |
| 19 | `FIRST_VERSION` | `crates/worldgen/src/undercrown.rs` | First generator version allowed to contribute Undercrown geometry. |
| 25 | `FIRST_VERSION` | `crates/worldgen/src/prism_sanctum.rs` |  |
| 28 | `FIRST_VERSION` | `crates/worldgen/src/dungeon/layout.rs` |  |
| 31 | `VERSION` | `crates/worldgen/src/bedrock.rs` | First generator with a bedrock floor. Older versions keep their flat basalt. |
| 32 | `DUNGEON_V2_GENERATOR` | `crates/worldgen/src/gates.rs` | Dungeon v2 and Undercrown v2 layouts, boss access and site modifiers. |
| 35 | `VERSION` | `crates/worldgen/src/amberroot.rs` |  |
| 36 | `VERSION` | `crates/worldgen/src/undercrown/v2/glow.rs` | First generator with cavern lamps and crystal seams. |
| 42 | `ENVIRONMENT_GENERATOR` | `crates/core/src/environment.rs` |  |
| 43 | `PALETTE_VERSION` | `crates/worldgen/src/amberroot.rs` | Teal canopy layers and grove moss. |
| 48 | `VERSION` | `crates/worldgen/src/dungeon/v2/fine.rs` | First generator with scale-resolved detail. |
| 50 | `MEANDER_GENERATOR` | `crates/worldgen/src/drainage.rs` | Generator 50: jittered nodes, eight-way steepest descent and meandering, width-varying channels. Earlier environment worlds keep the axis lattice. |
| 51 | `FINE_ARCHITECTURE_GENERATOR` | `crates/worldgen/src/gates.rs` | Fine architecture detail, above ground and in the Undercrown. |
| 52 | `GIANTS_AND_RUINS_GENERATOR` | `crates/worldgen/src/gates.rs` | Giant trees, dungeon ruins and Undercrown settling. |
| 53 | `LAKES_AND_CLIMATE_GENERATOR` | `crates/worldgen/src/gates.rs` | Lakes, regional climate scale, smaller seas and river fit. |
| 54 | `RIVER_ICE_GENERATOR` | `crates/worldgen/src/river_ice.rs` | Generator 54: snowy reaches of rivers and lakes carry a top layer of ice. |
| 57 | `MESA_SOFT_GENERATOR` | `crates/worldgen/src/terrain_variety.rs` | Generator 57: plateau benches get wandering edges and a gentler blend, so the mesas stop reading as contour stairs. |
| 60 | `VERSION` | `crates/worldgen/src/undercrown/v2/observatory.rs` |  |
| 63 | `ENCOUNTER_VERSION` | `crates/worldgen/src/undercrown/v2/observatory.rs` |  |
| 64 | `VERSION` | `crates/worldgen/src/meridian_terminal.rs` |  |
| 65 | `VERSION` | `crates/worldgen/src/meridian_terminal/monument.rs` |  |
| 72 | `CONNECTED_ROOFS_GENERATOR` | `crates/worldgen/src/gates.rs` | Connected roofs and stairs, dungeon workshops and tech mazes. |
| 73 | `GEO_GENERATOR` | `crates/worldgen/src/geo/mod.rs` |  |
| 75 | `VERSION` | `crates/worldgen/src/karst.rs` |  |
| 76 | `INTEGRATED_GENERATOR` | `crates/worldgen/src/geo/mod.rs` |  |
| 77 | `GENERATOR` | `crates/worldgen/src/valley/settlement/residence.rs` |  |
| 78 | `VERSION` | `crates/worldgen/src/fernwake.rs` |  |
| 79 | `VERSION` | `crates/worldgen/src/households.rs` |  |
| 80 | `VERSION` | `crates/worldgen/src/manyfold/districts.rs` |  |
| 81 | `OUTCROPS_AND_WEAVE_GENERATOR` | `crates/worldgen/src/gates.rs` | Shoulder outcrops and the Manyfold weave. |
| 82 | `VERSION` | `crates/worldgen/src/fernwake/waterworks.rs` |  |
| 83 | `VERSION` | `crates/worldgen/src/hell_gate.rs` |  |
| 84 | `VERSION` | `crates/worldgen/src/manyfold/architecture.rs` |  |
| 85 | `VERSION` | `crates/worldgen/src/fernwake/freight.rs` |  |
| 87 | `VERSION` | `crates/worldgen/src/manyfold/undercroft.rs` |  |
| 88 | `VERSION` | `crates/worldgen/src/dungeon/v2/sprawl.rs` |  |
| 89 | `VERSION` | `crates/worldgen/src/hell_gate/approach.rs` |  |
| 95 | `VERSION` | `crates/worldgen/src/vistas.rs` |  |
| 98 | `FLARE_VERSION` | `crates/worldgen/src/vistas.rs` | Arch legs flare into the ground and the opening closes onto it. |
| 102 | `VERSION` | `crates/worldgen/src/manyfold/strand.rs` |  |
| 104 | `VERSION` | `crates/worldgen/src/manyfold/shell.rs` |  |
| 105 | `VERSION` | `crates/worldgen/src/manyfold/dream.rs` |  |
| 106 | `VERSION` | `crates/worldgen/src/hall/mod.rs` |  |
| 108 | `VERSION` | `crates/worldgen/src/manyfold/expedition.rs` |  |
| 110 | `VERSION` | `crates/worldgen/src/manyfold/maze.rs` |  |
| 120 | `BIOME_VERSION` | `crates/worldgen/src/planet.rs` | Climate, biome cover, caves and woodland. Planet saves below this keep the shell. |
| 121 | `CLIMATE_VERSION` | `crates/worldgen/src/planet.rs` | Latitude climate and banded cover. Older planets keep the one-octave field. |
| 122 | `VERSION` | `crates/worldgen/src/basin.rs` |  |
| 123 | `DETAIL_VERSION` | `crates/worldgen/src/planet.rs` | Fine relief, ridged ranges, strata and ore bodies. |
| 124 | `VERSION` | `crates/worldgen/src/volcano.rs` | Volcanoes, plate boundaries, fiery wastes and enchanted woods. Dormant until the live version reaches it. |
| 125 | `UNDERGROUND_VERSION` | `crates/worldgen/src/planet.rs` | Dungeons, caverns and dry caves with aquifer pockets. Dormant until the live version reaches it. |
| 126 | `VERSION` | `crates/worldgen/src/deep.rs` |  |
| 127 | `TREE_DETAIL_GENERATOR` | `crates/voxel/src/config.rs` | Tree-only release of the leaf relief and branch-attached crown work. Keep their original gates for previously authored higher-version worlds. |
| 128 | `SURVEYOR_SITES_GENERATOR` | `crates/worldgen/src/gates.rs` | Surveyor ziggurats and planet site profiles. |
| 129 | `LANDFORMS_VERSION` | `crates/worldgen/src/geo/mod.rs` | Gate 129: glacial landforms and erratics on the planet face. Hills faces are too low to hold glaciers, so they keep their solve. |
| 130 | `VERSION` | `crates/worldgen/src/trees/foliage.rs` | Original authoring gate; also released in tree-only generator 127. |
| 131 | `VERSION` | `crates/worldgen/src/planet_climate.rs` | Axis climate and the cover split. Older planets keep the noise-weighted field. |
| 133 | `MEADOW_GROUND_VERSION` | `crates/worldgen/src/surface.rs` | Trail beds fray into packed earth and dry drainage swales keep their turf. |
| 134 | `FORM_VERSION` | `crates/worldgen/src/vistas.rs` | Rarer sites, per-peak heights, flank gullies, uneven beds with recessed soft layers. |
| 135 | `VERSION` | `crates/worldgen/src/planet_ocean.rs` | Shaped sea floor. Older planets keep the clamped continental ramp. |
| 136 | `VERSION` | `crates/worldgen/src/geo/corridor.rs` |  |
| 137 | `VERSION` | `crates/worldgen/src/trees/structure.rs` | Original authoring gate; also released in tree-only generator 127. |
| 138 | `LOD_VERSION` | `crates/worldgen/src/geo/corridor.rs` | Preserve regional drainage in distant geometry; older saves keep their LOD. |
| 139 | `VERSION` | `crates/worldgen/src/plants.rs` | Gate 139: bamboo groves. Dormant until `CURRENT_GENERATOR_VERSION` reaches it. |
| 140 | `VERSION` | `crates/worldgen/src/trees/stands.rs` |  |
| 141 | `VERSION` | `crates/worldgen/src/landscape/woodland.rs` |  |
| 142 | `SEAMLESS_PLANET_GENERATOR` | `crates/voxel/src/config.rs` | Connected spherical tectonics, climate, drainage and local terrain. |
| 143 | `HELL_PORTAL_GENERATOR` | `crates/voxel/src/config.rs` | Player-built thresholds and the red, open-sky Hell terrain. Older realms retain their climate, material distribution and saved portal addresses. |
| 144 | `RECOVERED_SYSTEMS_GENERATOR` | `crates/voxel/src/config.rs` | Recovered biome families, regional roads and abandoned districts. |
| 145 | `WATERSHED_LOD_GENERATOR` | `crates/voxel/src/config.rs` | River valleys retain their physical relief in distant Planet sampling. |
| 146 | `HELL_ECOLOGY_GENERATOR` | `crates/voxel/src/config.rs` | Shelves, fungal colonies and volcanic exposure for newly linked Hell realms. |
| 147 | `HELL_RUINS_GENERATOR` | `crates/voxel/src/config.rs` | Fungal Hell with explorable camp ruins. |
| 148 | `HELL_SURVEYED_RUINS_GENERATOR` | `crates/voxel/src/config.rs` | Ruins surveyed for dry ground and an open approach. |
| 149 | `HILLS_CANE_GENERATOR` | `crates/voxel/src/config.rs` |  |
| 150 | `HELL_MIRROR_GENERATOR` | `crates/voxel/src/config.rs` | Hell terrain driven by its parent planet recipe. |
| 151 | `COASTAL_PLANET_GENERATOR` | `crates/voxel/src/config.rs` | Plate-driven coasts and bathymetry for new Planet worlds. |
| 152 | `TALUS_PLANET_GENERATOR` | `crates/voxel/src/config.rs` | Conserved hillslope transport alongside runoff incision. |
| 153 | `FERNWAKE_SPILLWAY_GENERATOR` | `crates/voxel/src/config.rs` | Adds the editable spillway beside Fernwake's orchard. |
| 154 | `BEDDING_PLANET_GENERATOR` | `crates/voxel/src/config.rs` | Sedimentary beds shape inland slopes and their exposed rock. |
| 155 | `ALLUVIAL_PLANET_GENERATOR` | `crates/voxel/src/config.rs` | Discharge-scaled rivers and depositional deltas on the spherical watershed. |
| 156 | `CONTINUOUS_RELIEF_PLANET_GENERATOR` | `crates/voxel/src/config.rs` | Reconstructs solved land heights with shared vertex slopes. |
| 157 | `SCREE_PLANET_GENERATOR` | `crates/voxel/src/config.rs` | Erodes joints into gullies and forms talus at the foot of sedimentary cliffs. |
| 158 | `PERMAFROST_PLANET_GENERATOR` | `crates/voxel/src/config.rs` | Seasonal soil over shallow ground ice in cold, wet sediment. |
| 159 | `HELL_BIOME_GENERATOR` | `crates/voxel/src/config.rs` | Regional Hell terrain, ecology and hazards. |
| 160 | `MORPHODYNAMIC_PLANET_GENERATOR` | `crates/voxel/src/config.rs` | Evolving river mouths with conservative shallow-water and sediment transport. |
| 161 | `HOTSPOT_PLANET_GENERATOR` | `crates/voxel/src/config.rs` | Age-progressive oceanic shields and erosion of volcanic islands. |
| 162 | `RAINFOREST_GENERATOR` | `crates/voxel/src/config.rs` | Layered rainforest canopies and clustered undergrowth in new worlds. |
| 163 | `COASTAL_RELIEF_PLANET_GENERATOR` | `crates/voxel/src/config.rs` | Compose coastal erosion and deposits with the native shoreline relief. |
| 164 | `VOLCANIC_LANDFORM_PLANET_GENERATOR` | `crates/voxel/src/config.rs` | Collapsed summits and lava fields on active oceanic shields. |
| 165 | `TALL_RAINFOREST_GENERATOR` | `crates/voxel/src/config.rs` | Taller rainforest canopy and broad-crowned emergent trees. |
| 166 | `RAINFOREST_UNDERSTORY_GENERATOR` | `crates/voxel/src/config.rs` | Rainforest stand openings, fern colonies and leaf litter. |
| 167 | `AMAZONIAN_CANOPY_GENERATOR` | `crates/voxel/src/config.rs` | Amazonian rainforest canopy with distinct tall and wide emergent trees. |
| 168 | `RAINFOREST_DEADWOOD_GENERATOR` | `crates/voxel/src/config.rs` | Fallen rainforest trunks in canopy gaps and on stable forest floors. |
| 169 | `RAINFOREST_FLOOR_GENERATOR` | `crates/voxel/src/config.rs` | Palms and figs, reachable canopy vines and shallow rainforest swales. |
| 175 | `ESTUARY_PLANET_GENERATOR` | `crates/voxel/src/config.rs` | Tidal channels, point bars and floodplain benches on sediment-fed shelves. |
| 177 | `SURFACE_FLORA_GENERATOR` | `crates/voxel/src/config.rs` | Localized bamboo groves and climate-weighted ground flora. |
| 178 | `COASTAL_APPROACH_PLANET_GENERATOR` | `crates/voxel/src/config.rs` | Broader, curved approaches where the solved river meets the sea. |
| 179 | `REBUILT_RUINS_GENERATOR` | `crates/voxel/src/config.rs` | Connected Anchor districts and rebuilt ruin architecture. |
| 180 | `TALL_WOODLAND_GENERATOR` | `crates/voxel/src/config.rs` | Larger biome-specific forest canopies and old-growth trees. |
| 181 | `STRATOVOLCANO_PLANET_GENERATOR` | `crates/voxel/src/config.rs` | Eroded composite cones, collapse scars and volcanic aprons on Planet worlds. |
| 182 | `TREE_MORPHOLOGY_GENERATOR` | `crates/voxel/src/config.rs` | Species-specific branching and crowns for large woodland trees. |
| 183 | `CONTINENTAL_DRIFT_GENERATOR` | `crates/voxel/src/config.rs` | Time-evolved spherical crust for newly generated planets. |
| 185 | `WOODLAND_VARIETY_GENERATOR` | `crates/voxel/src/config.rs` | New aspen, cypress and acacia forms in their native habitats. |
| 186 | `TRAIL_SURVEY_VERSION` | `crates/worldgen/src/surface.rs` | Blocked links survey a graded path around the slope, and the road web keeps only links no third node sits between. |
| 190 | `ANCHOR_CITY_GENERATOR` | `crates/voxel/src/config.rs` | Larger Anchor cities with zoned blocks and building-specific salvage. |
| 191 | `CONTINUOUS_COAST_PLANET_GENERATOR` | `crates/voxel/src/config.rs` | Continuous bed elevations across the open-ocean shoreline. |
| 192 | `VOLCANIC_FLANK_EROSION_GENERATOR` | `crates/voxel/src/config.rs` | Local runoff incision and talus on volcanic edifices. |
| 194 | `CANOPY_UNDERSTORY_GENERATOR` | `crates/voxel/src/config.rs` | Broad woodland crowns, shrub colonies and larger ground plants. |
| 195 | `ESTUARY_OUTLOOK_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Opt-in landscape study, independent of the rolling Planet default. |
| 196 | `PLACER_PLANET_GENERATOR` | `crates/voxel/src/config.rs` | Finite placer deposits in spherical river beds and low gravel benches. |
| 200 | `ANCHOR_RUIN_ROUTE_GENERATOR` | `crates/voxel/src/config.rs` | Anchor landmarks, street-level exploration routes and damaged skylines. |
| 206 | `COUPLED_SEDIMENT_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Explicit preview of erosion with sediment feedback into the next solve. |
| 207 | `DISTINCT_FOLIAGE_GENERATOR` | `crates/voxel/src/config.rs` | Branch-attached foliage shaped by the tree's growth habit. |
| 210 | `WILD_FLORA_GENERATOR` | `crates/voxel/src/config.rs` | Wild flora pack: water, shore, wild crop, desert, small tree and cave plants. |
| 216 | `ANCHOR_TERRACE_GENERATOR` | `crates/voxel/src/config.rs` | Terrain-following Anchor wards and street-connected transit landmarks. |
| 217 | `RIVER_MORPHOLOGY_GENERATOR` | `crates/voxel/src/config.rs` | Valley-guided rivers with pools, point bars and unequal banks. |
| 218 | `CALDERA_FAULTS_GENERATOR` | `crates/voxel/src/config.rs` | Tilted collapse blocks and nested scarps on active hotspot shields. |
| 221 | `DELTA_ISLANDS_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` |  |
| 224 | `MOBILE_DELTA_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Refined mobile-bed solve spanning the last reach and the receiving shelf. |
| 229 | `ANCHOR_EXPLORATION_GENERATOR` | `crates/voxel/src/config.rs` | Connected ruin routes, grounded vegetation and purposeful salvage. |
| 230 | `COASTAL_DELTA_GENERATOR` | `crates/voxel/src/config.rs` | Tidal river mouths and suspended sediment from the coastal hydraulic solve. |
| 231 | `MOBILE_RIVERBED_GENERATOR` | `crates/voxel/src/config.rs` | Inland gravel transport and bank retreat in broad alluvial valleys. |
| 232 | `EVOLVING_DELTA_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Opt-in coastal solve with conveyance-weighted inflow and settling over the full bed. |
| 233 | `ANCHOR_ROAD_GENERATOR` | `crates/voxel/src/config.rs` | Surveyed regional roads between Anchor district gateways. |
| 234 | `FLOODPLAIN_MEANDER_GENERATOR` | `crates/voxel/src/config.rs` | Wider lowland floodplains, with migration and neck capture over a whole bend. |
| 235 | `ORE_MINERAL_GENERATOR` | `crates/voxel/src/config.rs` | Primary and weathered ore minerals share the established refining chains. |
| 236 | `PLANET_FREIGHT_GENERATOR` | `crates/voxel/src/config.rs` |  |
| 237 | `FLOODPLAIN_SCROLL_GENERATOR` | `crates/voxel/src/config.rs` | Floodplain relief from successive positions of the migrating bank. |
| 238 | `BEDROCK_RIVER_GENERATOR` | `crates/voxel/src/config.rs` | Flood abrasion of exposed rock and buried coal-bearing sedimentary beds. |
| 239 | `PLANET_FREIGHT_SITE_GENERATOR` | `crates/voxel/src/config.rs` | Terrain grading and building details around planetary freight sites. |
| 240 | `CURATED_SURFACE_GENERATOR` | `crates/voxel/src/config.rs` | Regional landmarks follow their habitats and farmhouses cluster in villages. |
| 241 | `WORKSHOP_SUPPLY_GENERATOR` | `crates/voxel/src/config.rs` | The starter workshop has a flooded supply gallery off its mine decline. |
| 242 | `CIVILIZATION_GENERATOR` | `crates/voxel/src/config.rs` | Regional settlement history follows solved planetary terrain. |
| 243 | `PLANET_FREIGHT_DETOUR_GENERATOR` | `crates/voxel/src/config.rs` | The planetary freight site includes an independent service crossing. |
| 244 | `REGIONAL_EROSION_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` |  |
| 245 | `LANDSCAPE_HISTORY_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Opt-in geological history with variable runoff, glacial sediment and tidal beaches. |
| 246 | `MAPPED_CANYON_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Playable canyon with shared erosional and mineable stratigraphy. |
| 247 | `FAULT_HISTORY_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Asymmetric subduction and tilted, segmented fault blocks feed erosion. |
| 248 | `WEATHERING_HISTORY_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Long regional uplift with episodic runoff and sediment-limited wind abrasion. |
| 249 | `UPLAND_RUNOFF_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Opt-in rainfall erosion on weak exposed upland beds as well as channels. |
| 250 | `ANCHOR_LINE_GENERATOR` | `crates/voxel/src/config.rs` | Dormant: district viaducts aim along a line toward the braid exchange, break only at a found cause. |
| 251 | `JOINTED_CANYON_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Regional canyon incision in heterogeneous, jointed bedrock. |
| 252 | `MOUNTAIN_EROSION_GENERATOR` | `crates/worldgen/src/geo/mod.rs` | Opt-in Hills/Ridged review; existing saves retain their original landscape. |
| 253 | `RAVINE_GENERATOR` | `crates/worldgen/src/geo/mod.rs` | Fractured channel walls and natural relief at former stack landmarks. |
| 254 | `CHRONICLE_GENERATOR` | `crates/worldgen/src/civilization.rs` |  |
| 255 | `CATCHMENT_EXPLORATION_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Connected headwaters and surveyed exploration loops on the regional bed. |
| 256 | `SEDIMENT_WEATHERING_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Moving bedload abrades rock and weathering replenishes upland sediment. |
| 257 | `VALLEY_FORM_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Wider valley reaches, resistant steps and bank-derived sediment storage. |
| 264 | `FOLDED_SCARP_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Incision across asymmetric folds with retreating, sediment-producing scarps. |
| 265 | `SILL_EROSION_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Bedrock sills concentrate incision into short pools downstream. |
| 266 | `HEADWATER_RELIEF_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Planet-wide headwater incision with local lithology and talus transport. |
| 268 | `VERSION` | `crates/worldgen/src/planetary/reference_relief.rs` |  |
| 269 | `BEDROCK_SCARP_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Layered escarpment retreat and channel incision across a tilted rock package. |
| 270 | `TECTONIC_TRAVERSE_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Continuous mountain belts, traversable passes, and bent hotspot chains. |
| 271 | `DRAINAGE_RELIEF_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Eroded beds and composed gradients across the planet's tributaries. |
| 272 | `WATERSHED_EROSION_GENERATOR` | `crates/voxel/src/config.rs` | Tectonic terrain with refined erosion and routed groundwater. |
| 273 | `VOLCANIC_SECTOR_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Volcanoes dissected from sector failure through routed sediment transport. |
| 274 | `CONNECTED_WATERSHEDS_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Evolve bedrock, channel deposits and floodplains on the shared sphere. |
| 275 | `REFINED_DRAINAGE_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Finer spherical drainage and sediment-driven lateral channel evolution. |
| 276 | `RIFTED_MARGIN_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Rifted continental margins and fragments carried by plate motion. |
| 277 | `CONNECTED_RELIEF_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Connect the planet's runoff and high-resolution erosional relief. |
| 278 | `COUPLED_RIFT_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Advected crust ancestry with erosion and coastal transport during plate motion. |
| 279 | `LAYERED_CANYON_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Faulted strata exposed by connected canyon erosion. |
| 280 | `GEOLOGICAL_HISTORY_GENERATOR` | `crates/voxel/src/config.rs` | Climate strata, eruptions and fault history coupled to plate motion. |
| 281 | `EVOLVED_WATERSHEDS_GENERATOR` | `crates/voxel/src/config.rs` |  |
| 282 | `RESOLVED_HISTORY_GENERATOR` | `crates/voxel/src/config.rs` | Separate plate, event and sediment clocks; subsiding volcanic reef platforms. |
| 283 | `CHANGING_SEAS_GENERATOR` | `crates/voxel/src/config.rs` | Historical sea-level cycles and offshore-only bathymetry. |
| 284 | `BRANCHING_WATERSHEDS_GENERATOR` | `crates/voxel/src/config.rs` | Momentum-routed tributaries and continuous main-stem reaches. |
| 285 | `FINE_HISTORY_GENERATOR` | `crates/worldgen/src/planetary/mod.rs` | Opt-in finer plate integration with bounded physical-history playback. |
| 286 | `LANDSCAPE_CYCLES_GENERATOR` | `crates/voxel/src/config.rs` | Climate-driven ice history, terrain-linked woodland and connected travel routes. |
| 287 | `DEEP_TIME_GENERATOR` | `crates/voxel/src/config.rs` | Extended continental and climate evolution for newly created planets. |
| 288 | `FRACTAL_COAST_GENERATOR` | `crates/voxel/src/config.rs` | Deep-time history with a fractal, rock-dependent shoreline planform. |
| 289 | `PROCESS_COAST_GENERATOR` | `crates/voxel/src/config.rs` | Generator 288 plus wave-driven longshore shorelines and rock-grain coasts. |
| 290 | `CLEAN_RIVERBED_GENERATOR` | `crates/voxel/src/config.rs` | Generator 288 coasts plus clean riverbeds, ponds and sandier shores. |
| 291 | `VERSION` | `crates/worldgen/src/undercrown/v2/wilds.rs` | First generator with the Rift, the Hollow Sea and the sunken grove. |
| 292 | `VERSION` | `crates/worldgen/src/undercrown/v2/props.rs` | First generator with per-kind prop models. Dormant until bumped. |
| 293 | `GROUND_BREAK_GENERATOR` | `crates/voxel/src/config.rs` | Generator 290 terrain plus ravines, fissure swarms and sinkholes (opt-in). |
| 294 | `VERSION` | `crates/worldgen/src/dungeon/v2/quest.rs` |  |
| 295 | `LIVING_SURFACE_GENERATOR` | `crates/voxel/src/config.rs` | Generator 290 terrain plus fine relief, shore forms, climate plants and plant stamps (opt-in). |
| 296 | `LOWLAND_RIVER_GENERATOR` | `crates/voxel/src/config.rs` | Generator 290 terrain plus lowland rivers that sit in their plain: levees, backswamps, marsh (opt-in). |
| 297 | `RIFT_VALLEY_GENERATOR` | `crates/voxel/src/config.rs` | Generator 290 terrain plus faulted rift valleys, the 293 ground breaks and the 295 living surface (opt-in). |
| 298 | `DIKE_SWARM_GENERATOR` | `crates/voxel/src/config.rs` | Generator 297 terrain plus basalt dikes: rift-parallel and volcano-radial swarms that cut the strata (opt-in). |
| 299 | `ANCHOR_OUTSKIRTS_GENERATOR` | `crates/voxel/src/config.rs` | Anchor city outskirts: lanes that fray out past the kerb and lots thinning with distance (opt-in). |
| 300 | `CARVED_SURFACE_GENERATOR` | `crates/voxel/src/config.rs` | Generator 298 terrain and 296 lowland rivers plus slope-aligned gullies, rock benches and terracettes (opt-in). |
| 301 | `VERSION` | `crates/worldgen/src/meridian_terminal/earthport.rs` |  |
| 302 | `VERSION` | `crates/worldgen/src/undercrown/v2/life.rs` | First generator with Undercrown loot and spell finds. Dormant until bumped. |
| 303 | `VERSION` | `crates/worldgen/src/planetary/dunes.rs` |  |
| 304 | `ROCK_RELIEF_GENERATOR` | `crates/voxel/src/config.rs` | Generator 300 terrain plus rock relief: solved bed resistance raises risers and caprock benches (opt-in). |
| 305 | `COHERENT_DUNES_GENERATOR` | `crates/worldgen/src/planetary/dunes.rs` | A second opt-in candidate; generator303 remains reproducible. |
| 306 | `DEPOSIT_GENERATOR` | `crates/worldgen/src/planetary/dunes.rs` | Deposit after erosion and retain its physical thickness. |
| 307 | `ERRATIC_RELIEF_GENERATOR` | `crates/worldgen/src/planetary/erratics.rs` |  |
| 308 | `ERRATIC_DEPOSIT_GENERATOR` | `crates/worldgen/src/planetary/erratics.rs` | Solid rock deposited after erosion; historical307 relief stays unchanged. |
<!-- gates-cli index: end -->
