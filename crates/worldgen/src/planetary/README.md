# Spherical terrain (generator 142 onward)

New Planet worlds and planet recipes use this generator. Older saves retain
their stored generator version; do not silently upgrade them, since their
edited chunks belong to the old terrain.

`topology` constructs a connected sphere with shared cube-edge/corner vertices,
geometric cell areas and neighbour distances. `tectonics` supplies continental
crust, angular plate motion, convergent ranges, rifts and carbonate provinces.
`hydrology` routes rainfall across the entire sphere, erodes by upstream runoff
and rock erodibility, and resolves lake spill levels. `volcanism` selects and
caches vents using those same plates before the final drainage solve.

Generator 183 evolves the crust for 240 Myr in 5 Myr steps. Finite Euler
rotations move the plates and backtrace crust on the shared sphere. Rift events
split selected plates, and convergence, spreading and transform shear change
relief along their moving boundaries. The terrain solve uses the final state.
The loading globe reads height and boundary snapshots from the same solve at
10 Myr intervals.

History length is per world: `history_duration_myr(cfg)` is the one source
(240 Myr; from deep-time generator 287 the `VOXEL_HISTORY_MYR` development
override, then the world's history slider value `PlanetSettings::history_myr`
of 20 to 720 Myr kept by save 73 and the `-Hnnn` recipe segment, then 720 Myr).
Only the full 720 Myr history uses the geology cache and snapshot. The loading screen's
Myr counter reads it. `DRIFT_MYR` is the legacy 240 Myr constant only; the
chronicle provenance still records it for every generator (see
`civilization/chronicle.rs`).

The model draws on [procedural tectonic planets](https://doi.org/10.1111/cgf.13614),
[plate reorganization and fragmentation](https://www.nature.com/articles/s41561-025-01772-7)
and the boundary profiles in [plate-local terrain generation](https://arxiv.org/abs/2610.00301).

Preview 276 thins advected crust
according to accumulated extension and selects volcanic fronts using the
buoyancy of the crust on each side of a converging margin. It follows the
stretching mechanism in [McKenzie](https://doi.org/10.1016/0012-821X(78)90071-7).
After the plate history, climate epochs and sediment transport reshape the
224-cell-per-face sphere. A fetch-weighted wave pass transfers coastal rock
to the adjacent shelf before the final watershed solve. Near the player, the
existing 32, 16 and 8 m erosion stages feed a 4 m stage. Its rock and sediment
state inherits the coarser solution and reroutes runoff as the bed changes.
The landscape transport model draws on [Davy and Lague](https://doi.org/10.1029/2008JF001146).

Preview 278 introduced coupled crust and sediment history. Each 5 Myr
step transfers continental material and sediment by spherical cell volume.
The state carries continental thickness, dominant original plate ancestry,
carbonate composition, oceanic age, accumulated extension and sediment cover.
Strain triggers bounded plate splitting. Gaps between moving parcels acquire
young oceanic substrate. Surface runoff, erosion, deposition and directional
coastal sediment transport operate during the same history. Ten-Myr snapshots
include the resulting surface, runoff, deposits and material accounting.

The final surface uses the irregular 224-cell spherical mesh. Refinement
inherits the evolved sediment cover and admits external runoff where parent
receiver edges enter its boundary. The 32, 16, 8 and 4 m stages carry the
bedrock changes and sediment forward, with shared chart edges and filtered
gradients. The geological loading globe shows deposits and drainage from its
history frames. The crust kinematics, wave climate and erosion rates are
reduced, game-scale models with uncalibrated response coefficients.

New Planet worlds use generator 283. Older saved generator versions retain
their terrain rules. The development panel's project index exposes the
history source and constants under `drift/history.rs` and `drift/coupled.rs`.

In 280, climate changes continuously across epochs. Moving continents and
mountain barriers change ocean moisture supply and rain shadows. Eight
30 Myr deposit bins and two recent beds carry volume-weighted formation dates, moisture, organic
content, carbonate and lava through the same conservative plate remap.
Recent lava and soil remain separate, preserving the latest buried ground.
Erosion removes the youngest surviving beds first. Burial and lithification
determine exposed soil, clay, sandstone, limestone, coal and basalt; no
historical bed is created without a deposition or eruption. Mineable blocks
and the fine erosion stages read the same columns. Height fitting compresses
their contacts with the surrounding terrain, and modern volcanic cover
buries the older record rather than replacing the whole rock column.

Fault loading and magma recharge accumulate between events. Converging,
subducting margins can release larger thrust displacements; spreading and
subduction supply eruptions. Released stress also changes plate motion
gradually. Uplift and fresh lava reset weathering age. Resistant young rock
retains steeper relief, while weathering, slope transport and rivers wear
older relief into hills and sediment-filled valleys. In 280/281 these are
aggregates over 5 Myr steps. Deposits within a 30 Myr bin are mixed; the model is a bounded
geological approximation, not a calibrated Earth reconstruction.

The loading globe interpolates heights and colours between real published
epochs on the GPU. Its age follows the same interpolation, waits for new
history when necessary, and finishes before handing off to the final map.

From 282, `drift/episodes.rs` separates plate transport from surface evolution.
The crust remap remains 5 Myr; erosion, cover, slope failure and sediment
exchange advance in twenty 0.25 Myr intervals. Fault and magma reservoirs
solve recharge, leakage, threshold crossing and reset analytically within
each interval. Repeated equal cycles are batched with their exact residual,
released volume and last occurrence date. Earlier lava uses its mean formation
date; the latest cap retains its individual date. Rates are game-scale model
coefficients, not calibrated earthquake forecasts or event catalogues.

The drainage DAG is reused with updated spill levels; changed connections
can trigger at most three additional priority floods per plate step. A final
route belongs to the published bed. Sediment retains its incoming direction,
and sharp turns shorten transport distance. Incoming material abrades exposed
bed; deposited cover shields it. Over-capacity supply stays in the mass balance.
The 32/16/8/4 m refinement stages also use cover-based abrasion in 282 onward.
Hillslope transfers retain the eroded material's carbonate fraction.
Forty-nine globe frames and at most four dated event markers per interval bound
preview storage. The playback clock caps catch-up speed instead of racing
through a completed solve. Marker visibility is exaggerated for legibility;
the event dates are simulated, not generated by the loading animation.

Motion in 282 onward uses a 2.5 cm/year Earth-reference angular rate. Seeded
initial plate factors give 0.75–3.25 cm/year at their Euler equators;
local speed falls toward each Euler pole. Later motion is capped at 5 cm/year
equivalent, with bounded acceleration and smaller velocity changes on splitting.
The compact planet preserves these angular rates, scaling linear distances
with its radius. Hotspot tracks now integrate the recorded Euler stages and
parent plate history; the older artificial 1.65+ km/Myr track-spacing speed
is retained only for old generators. Longer, seeded plume histories provide
island chains at the real carrier speed. Cooling subsidence follows a square-root
age response, rather than forcing every island underwater at a fixed age.

Generator 288 keeps the 287 history and shapes the shoreline at sampling
time (`coast_roughness.rs`). Interpolating the 224-cell solve leaves coasts
along cell rows and plate fronts. Inside a band around the solved shoreline the
surface is resampled at a vector-fBm-warped point (4 km down to 12 m, octave
ratio 0.616, i.e. fractal dimension about 1.3 after Mandelbrot 1967 and
Sapoval et al. 2004). Collision, rift and former ice raise the roughness;
thick sediment cover lowers it, leaving smoother strand coasts. Octaves finer
than the footprint are filtered, so globe, horizon and voxels agree. Lakes,
active calderas and river channels (`calm`) keep their solved surface.

Generator 289 adds process shorelines. `shoreline.rs` runs a one-line
coastline model (Ashton, Murray and Arnoult 2001; Ashton and Murray 2006) on
the solve mesh after coastal transport and before routing: CERC alongshore
flux `cos^1.2(phi) sin(phi)` from a spread of zonal-wind waves, shadowing by
land toward each wave source, and volume-conserving exchange of mobile cover
over the shoreface (closure depth to berm). High-angle waves grow capes and
spits; rock without cover keeps its planform. The 288 warp is also stretched
along the rock grain (strike across the fold/fault/rift intensity gradient),
giving Dalmatian and ria coasts where structure meets the sea.

`reefs.rs` ages those connected hotspot platforms before the final drainage
solve. Only affected cells and a two-ring sediment halo participate. Growth
uses shallow substrate, light, warmth, incoming resources, adjacent living
reef and burial by sediment. Land blocks resource exchange; reef consumes it.
Framework breakdown and volcanic weathering create finite mobile sand volumes,
which rivers and shore currents redistribute. Atoll-like rims, lagoon gaps and
sand shoals must emerge from that substrate and exchange, without circular
reef masks. The local surrogate uses at most 256 age-weighted intervals (at
most 0.25 Myr for the generated plume ages), each with four resource exchanges.
Its flow is an advection/exchange approximation, not Navier–Stokes. It does not
reconstruct coral species, ecological years, or each historical eruption.

Research applied in this refinement (reviewed 2026-11-07):

- [Schott et al., 2024, Terrain Amplification using Multi-Scale Erosion](https://h-schott.github.io/p/mserosion/): keep coarse drainage and material inheritance connected during fine erosion.
- [McDonald and Cordonnier, SIGGRAPH 2026, Stochastic Geomorphological Transport](https://erosiv.studio/publications/stochastic-geomorphological-transport): separate transport and landscape clocks and retain transport direction. This code is a deterministic reduced adaptation, not their stochastic momentum solver.
- [Davy et al., 2026, Lift or impact](https://esurf.copernicus.org/articles/14/635/2026/): couple sediment tools, evolving bed cover, and deposition without clipping excess supply.
- [Llabrés et al., March 2026 preprint, Hydrodynamics shapes annularity in coral reefs](https://arxiv.org/abs/2603.16933): substrate/resource/growth feedback motivates uneven reef rims; this implementation does not reproduce their LBM solver.
- [USGS, Understanding plate motions](https://pubs.usgs.gov/gip/dynamic/understanding.html): centimetre-per-year plate rates provide the reference scale, independent of preview playback speed.
- [Davis, October 2026 preprint, Plate-Local River Generation](https://arxiv.org/abs/2610.00301): considered for bounded caching; independent plate river graphs are not adopted here because moving boundaries and inherited cross-plate watersheds must remain connected.

Cloud geometry uses the compact world's scale separately from the optical
scattering radius. A shared monotone height transform keeps all volume bounds,
shadows and temporal reconstruction consistent. Low cloud morphology uses
0.58 of the former horizontal scale; the orbital shell is 980 m high with
280 m maximum near-orbit relief and a nonzero distant relief floor.

Generator 283 varies historical sea level by up to 6 m below the modern
datum using the climate cooling cycle as a proxy for water stored on land.
Drainage, wave abrasion and weathering use the same datum for each resolved
interval. The loading globe shows the simulated range of local annual
temperatures and sea level relative to the present. Present-day gameplay
uses the modern datum.

The shelf profile runs after land formation, drainage and coastal sediment
transport. It tapers in below 8 m of water and leaves the sea-level contour
to the terrain solve. The `planet_relief_review --bathymetry <version> <seed>`
audit compares the land mask and coastal crossings on the spherical mesh
with the shelf profile enabled and disabled.
Biome colours use each epoch's simulated moisture and temperature; rupture
and eruption colours use actual event fields. The camera eases toward the
spawn view without replaying or delaying the world-loading job.

Columns, filtered orbital cover, river sources, climate and caves read body-space
fields. Local relief uses physical wavelengths and fades at waterways. A regional
view exists for older geographic UI consumers; its checksum covers the whole
spherical solve. It is not a separate drainage network. New drainage consumers
must use the spherical receiver graph rather than that regional view.

The cache key includes the seed and all planet recipe dials, but not voxel size:
0.25, 0.5 and 1 metre worlds sample the same physical terrain. Cached worlds are
bounded and callers retain their own references while another recipe builds.

Run `tools/Play-Planet.ps1 -Rebuild` to build and open the current game. Create a
new Planet world for generator 142 or later. Existing worlds require a separate explicit
migration design; replacing their generation beneath edited chunks is unsafe.

Generator 151 added shelf breaks,
trench depressions, broken island arcs, headlands and rift channels before the
drainage solve. River mouths widen at the coast and shore materials follow
slope, rock and sediment. Strait and isthmus connectivity can vary by seed;
the generator does not guarantee either at a particular location. Globe tiles
carry inland water heads from the same sampler, using spare bits in their
existing atlas format. Derived tiles rebake automatically after the format change.

Generator 152 added loose-rock transport during the existing watershed solve.
Loose rock moves from steep slopes to adjacent lower ground. Transfers conserve
volume across unequal mesh cells, and resistant provinces retain steeper slopes.
The process leaves the shoreline alone and adds no work to voxel or GPU sampling.

Generator 154 added folded beds that shape inland
slopes into steep faces and gentler benches. The same folded datum picks the
sandstone or limestone exposed on a face and the rock below it. The effect
fades on plains, in volcanic terrain, and at coarse viewing distances. It
leaves the watershed solve and channel beds in place. Versions through 153
retain their previous surfaces.

Generator 157 weathers those escarpments. Warped fractures open into wider
gullies, with gravel fans where the same cuts meet the gentler foot of the bed.
Small fracture detail fades before the larger gullies and fans. New Planet
worlds and recipes use this generator.

No claim of visual acceptance or comparison with another game's generation
follows from compilation. Review face edges/corners, river confluences, shores,
ground-to-orbit transitions and all three voxel sizes in the native renderer.

Generator 160 evolves selected river mouths before chunks stream. The solver
uses 16 m hydraulic cells, a fixed step budget with a CFL-controlled time step,
hydrostatic reconstruction, and shared water, momentum and sediment fluxes.
Sand and mud settle at different rates. A fetch-weighted alongshore sand
transfer approximates wave reworking. Bed elevation feeds back into the
next flow step. Sediment supply represents an accelerated depositional
episode and is capped by the solved catchment's export.

An outlet-first, steady backwater calculation passes the downstream water
level into each connected reach. It estimates the water level needed to
carry flow through the section against Manning friction. High main stems
can back up flatter tributaries or spill onto their existing floodplains.

The six strongest non-overlapping eligible mouths can each bake a coastal
tile. The working state is discarded. The bake stores height, water depth
and sediment in body coordinates for the 0.25, 0.5 and 1 m voxel samplers
and distant terrain. Both peers derive it from the same recipe. The planet
checksum includes the tile geometry and fields.

Run `cargo run -p engine_worldgen --example morphodynamics_review -- 74` for
the coastal bake budget, repeatable terrain hashes and voxel sampling times.
A new dedicated Planet session accepts `--multiplayer-planet` and
`--multiplayer-voxel-mm=250` (also 500 or 1000).

Preview generator 206 couples incision and deposition during the 18 spherical
erosion passes. Incoming load settles over a discharge- and slope-dependent
transport length. Deposition raises the bed before the next priority flood.
The coastal bake reads the material that crossed the land-to-ocean edge at its
mouth. Placers still read local incision volumes. The transport budget separates
bed deposits, coastal export and any unsettled stock in a closed basin.

The graph approximation follows the transport-length idea from
[Davy and Lague](https://agupubs.onlinelibrary.wiley.com/doi/full/10.1029/2008JF001146).
It uses one receiver and one mobile load per vertex, with an explicit bound on
bed rise per pass. [RiverBedDynamics](https://gmd.copernicus.org/articles/18/3427/2025/)
and [basin-scale migration modeling](https://hess.copernicus.org/articles/30/1563/2026/)
illustrate finer process models for comparison. The preview retains the separate
lake-depth setting and local coastal hydraulic bake. Compare 205 and 206 with
`morphodynamics_review 74 205` and `morphodynamics_review 74 206`.

In the matched Planet survey, seed 74 retained its selected coastal tile.
The two-dimensional coastal bake formed 20,224 m² of new land in 206,
compared with 24,320 m² in 205. The coupled run deposited 13.47 billion m³
across the spherical mesh before the coastal bake, with a sediment-budget
residual below 0.001 m³. Repeat runs matched the terrain and voxel hashes
at 1, 0.5 and 0.25 m. Coastal coverage changes with the drainage solve:
seed 4 went from one eligible bake to zero and seed 731 from two to one.
Matched native overhead views of seed 74 show the changed bar geometry.
Both views still show a sharp water-surface color boundary after the visible
terrain finished streaming. The survey does not establish visual acceptance,
calibration against external terrain references or live multiplayer behavior.

Preview 221 builds a 2 m depositional fan inside the existing hydraulic tile.
One trunk splits into progressively narrower distributaries. Small branches
and cross-creeks separate the bars inside the fan, while the outer platform
grades into the shallow shelf. The fan borrows a bounded share of the retained
sediment and stores a small raster shared by terrain and water sampling.
Compare 206 and 221 with `morphodynamics_review 74 221`.

[Wax Lake imagery](https://science.nasa.gov/earth/earth-observatory/world-of-change/wax-lake/)
and [mouth-bar bifurcation observations](https://agupubs.onlinelibrary.wiley.com/doi/10.1029/2006JF000574)
set the planform target. The existing [catchment prototype](../../../../docs/worldgen/CATCHMENT_REVIEW.md)
and [Schott et al.](https://aparis69.github.io/public_html/projects/schott2024_Erosion.html)
provide the coarse-to-fine precedent. [DeltaRCM](https://esurf.copernicus.org/articles/3/67/2015/)
and its [water](https://github.com/DeltaRCM/pyDeltaRCM/blob/develop/pyDeltaRCM/water_tools.py)
and [sediment](https://github.com/DeltaRCM/pyDeltaRCM/blob/develop/pyDeltaRCM/sed_tools.py)
implementations let channels emerge from parcel routing and evolving beds.
In 221 the finer planform is constrained by a seeded channel graph, with the
coarse hydraulic tile supplying the shelf and sediment budget.

The metre-scale terrain survey found connected river-to-sea water paths on
seeds 74, 731 and 16. Counting detached patches of 12–1,200 m² at a 2 m sample
step gave 2, 3 and 3 small islands respectively. Seed 74 repeated its hashes
across the three physical voxel scales. In-game review is still open.

Generator 161 adds oceanic hotspot tracks. The same plate motion that drives
tectonic boundaries carries older shields away from the active plume. The
compressed track spans about eight million years, from submarine growth
through overlapping rift-fed shields to deeply weathered islands and drowned
seamounts. One-sided flank failures leave headwalls and debris aprons.
Shields enter the climate and watershed solves before incision and sediment
delivery, with more time for runoff to carve the older surfaces. The active
summit feeds the existing eruption system.

Generator 164 gives active shields stepped collapse walls and a lava lake
offset within the summit. Lava fields follow downhill routes from rift
outlets. Their lobes build relief and expose fresh basalt and ash over
the weathered shield.

Generator 181 builds composite volcanoes on convergent arcs. Steep crowns
rise from older shoulders, with ravines between buttresses and occasional
one-sided collapse scars feeding debris fans. The edifice enters the
watershed solve before erosion. Ground, horizon and globe samples use the
same relief and exposed rock.

Opt-in preview 293 (`GROUND_BREAK_GENERATOR`, `ground_breaks.rs`) keeps the
290 terrain and carves ravines, fissure swarms and sinkholes below the solved
surface through `cave_carves`. Ravines favour and follow spreading plate
boundaries, fissures open only there, sinkholes favour carbonate ground. The
far horizon still draws the solved surface, so these are kept narrow; a wide
canyon or a rift valley has to be part of the height solve instead. Review
with `VOXEL_HISTORY_MYR=20 cargo test -p engine_worldgen --lib -- planetary::ground_breaks --ignored --nocapture`.

Preview 297 (`rift_valley.rs`) faults a graben into continental divergent
margins and turns on the 293 ground breaks and the 295 living surface. The
valley has a master border fault on one side and a flexed ramp on the other;
the master side flips along strike and the floor shallows into a saddle where
it flips. The smooth part is added to the mesh heights before climate, erosion
and drainage (not inside `evolved_at`, which the drift history calls every
step). Each column then adds the full section minus the interpolated mesh
part, which restores the scarps the 375 m mesh cannot hold and is zero at the
nodes. Floors stop 14 m under the sea. Review with
`VOXEL_HISTORY_MYR=20 cargo test -p engine_worldgen --lib -- planetary::rift_valley --ignored --nocapture`.

Preview 298 (`dikes.rs`) intrudes basalt dikes on the 297 terrain. A dike
opens perpendicular to the least compressive stress, so swarms strike along
rifting boundaries and radiate from volcanic vents. Each 192 m cell holds at
most one segment; the block sampler reads it before soil and strata because
the dike is the younger rock. Segments are vertical, the contact is not
baked, and the surface is not raised over them.

Preview 300 (`carve.rs`) keeps the 298 terrain and the 296 lowland rivers and
adds the relief finer than the 8 m erosion tiles. Those tiles already return
the gradient of the surface they eroded; `carve` cuts gullies that follow it.
A 27.5 m body-space lattice holds, per corner, that gradient and its
divergence (two extra samples 12 m away), computed once and shared by every
column and thread around it. The coarsest octave (22 m) is phasor noise after
the Phacelle erosion filter (Johansen 2026, blog.runevision.com): each corner
draws straight stripes along its own gradient, with its own spacing, and the
sum forks where neighbours disagree. Each finer octave (down to 1.4 m) runs
down the walls of the one above, because every octave adds its side slope to
the gradient the next follows; that lean is capped at 0.55 of the hill's own
slope so rills never turn along the contours. There is no simulation and no
chunk or cube-face seam.

Strength follows process. Gullies start where topography gathers runoff
(positive divergence, the planform reading of the area-slope threshold for
channel initiation), so hollows are scored and spurs stay whole. Drainage
density is lowest on moderately watered, soil-mantled ground and high both on
bare dry slopes and under daily rain. Walls are no steeper than the hill can
feed; hard rock on steep faces holds sharper chutes. Ribs stay near the uncut
surface and creases cut below it (rounded ribs, V creases). Creases on steep
ground collect scree and hold a little more moisture; the cut raises the
local slope, so the existing cover rules expose rock on ribs. The stage fades
to nothing beside channels, lakes, the shore, deltas and the summit clamp,
and octaves finer than the sample footprint are filtered without moving the
coarser ones.

The same stripe function draws dunes across the prevailing zonal wind
(asymmetric crests, flat interdune floors, forked crestlines): a low belt on
dry backshore sand in place of the 295 ridge noise, and sand seas with 130 m
draa in patches of hot dry lowland. Flats keep only a fifth of the 295 knoll
and hollow relief, and shore stacks stand in groups. Rainforest floor cover
and its relief read one warped two-octave patch field (mud in flat wet
hollows, moss on drained rises) instead of single-wavelength bands, and the
stand floor's edge is frayed. Sub-half-metre samples get terracettes on a few
turf slopes; at full strength on every slope they ruled contour lines over
whole ranges, which the lab's `--layer height` view exposed.

Review without the game:
`node tools/axiom-dev/planet-lab-cli.mjs review 290 300 --at 5000,4500`.

Opt-in generator 304 (`rock_relief.rs`) keeps the 300 surface and lets the
rock under it shape the hillside. Each 48 m lattice corner reads its history
column into a resistance profile, one entry per metre from 80 m below to 16 m
above the solved top (above it the top bed is held). Below the beds, the crystalline basement is foliated. Its bands dip gently
in old cratons and steeply in collision belts, and their strike swings across a
1.5 km lattice. The carved height goes through a run function, the integral of
`1/R^n` over each 20 m band of height. Both edges of the band stay fixed, so the
result is continuous and bounded. Ground climbs fast through hard bands, which
form risers and caprock lips, and slowly through soft ones, which form benches.
The exponent falls on moist ground, so wet hills stay soil-mantled and dry ones
show sharp ledges. The 300 gullies lower the bed frame in their creases, so
chutes cut the risers. The stage fades out above a 30 m footprint. Check it with
the lab's `riser_fraction` and `break_of_slope_per_ha`. Like every gate above
302, a 304 world also gets the 301 earthport and 302 Undercrown loot through
their `>=` checks, so it depends on those gates staying frozen.
