# Regional erosion review

## Erosion below bedrock sills in preview 265

The valley pass follows each receiver and carries a short-lived record of
upstream bedrock resistance. Where runoff crosses from harder rock into
weaker rock at a drop, it concentrates incision downstream of the resistant
lip. Contributions from small tributaries fade at confluences. The cut
still requires opposing valley sides and enough downstream fall to drain.
Smaller connected channels can erode headward before the bank-widening
pass becomes active. Broad openings are reserved for larger, slower reaches.
The existing sediment budget accounts for retained material and export.
[Field measurements of substrate-controlled bed roughness](https://research.fs.usda.gov/treesearch/35929)

Run `catchment_review <output> 74 --parent 160 --rock-package --jointed
--sill-erosion` for a full bake. To replay the final pass from a frozen
`--explore` export, run `catchment_review replay-valley 265 4 199.663 74
<canyon-bedrock.f32> <canyon-sediment.f32> <height.f32>` for the seed 74 review
window. Version 257 in that command replays the previous valley pass.

The full seed 74 bake matched the replay at cell 873586 on the 4 m grid.

| Measurement | Preview 257 | Preview 265 | Black Dragon Wash |
| --- | ---: | ---: | ---: |
| Small tributary median width (m) | 91.22 | 82.65 | 39.46 |
| Mid-size tributary median width (m) | 99.02 | 98.21 | 40.89 |
| Main valley median width (m) | 104.77 | 103.08 | 105.63 |
| Main stem length (m) | 1,166 | 1,178 | 2,685 |
| Profile slope breaks at 48 m sampling | 3 | 4 | 15 |

The rise in coarse profile breaks came with fewer sharp grade changes at
12 m spacing, from twelve to ten at a threshold of twelve percentage points.
The regional pass retained 5,931 m³ of detached material and the full
sediment budget closed to 0.122 m³. In an archived seed 731 replay from
preview 251 terrain, the previous fixed outlet fell outside the audit's
enclosure or area criteria, so that window has no matched-outlet result.

The normal seed 74 Planet sampler reproduced the exact waypoint traces from
preview 255. It found solid footing and two dry clearance cells at all 867
samples. This was a 1 m voxel clearance scan.

## Connected valley floors in preview 257

The new pass traces the existing channel network with contributing area in
square metres. Larger channels carry a floor grade into their connected lower
slopes. Fast reaches stay confined and slower reaches can widen into existing
shoulders. Opposing terrain rises must support the cut so the routing solver
cannot etch a line across flat ground. Exposed weak beds retreat farther than resistant beds, with the cut
bounded by the drop to the downstream receiver. The pass reroutes the finished
surface before surveying trails.

Bank and bed detachment supplies sediment that moves down the same network.
Gentler margins can retain part of that load as sand or angular gravel mounds.
The remainder enters the exported sediment budget. The local deposit map
supplies the actual surface material in the Planet sampler.
[Field measurements of rock-controlled bed steps](https://doi.org/10.1029/2008JF001188) ·
[Sediment cover and incision in Utah](https://doi.org/10.1029/2007JF000862)

Reproduce the offline comparison with `catchment_review <output> 74
--parent 160 --rock-package --jointed --valley-form`, and compare with
`--explore` in place of `--valley-form`. Use `whole 4 1` on the resulting
bedrock and soil pair, and compare against the same Black Dragon Wash raster.

At the fixed generated outlet cell 873586, the basin area changed from
1.000 to 1.019 km² and the footprints overlapped by 98.09% of their union.
The main valley widened from 74.25 to 104.77 m at 5 m
above the bed. Black Dragon Wash measured 105.63 m. The two tributary bands
widened from 85.34 and 92.07 m to 91.22 and 99.02 m, farther from the reference
than before. The main stem length changed from 1,161 to 1,166 m and retained
three measured profile breaks, versus 2,685 m and fifteen in the reference.

This bake detached 234,112 m³ and retained 10,739 m³ on valley margins across
the full 4 km window. A stronger
fracture pass disrupted the fixed outlet and produced no trail pair in the
offline comparison, so the preview uses the wider-bank candidate.

## Moving sediment and weathering in preview 256

The coarse-load transport pass wears exposed bedrock using gravel arriving
from upstream, limited by local transport capacity. Abrasion only activates
in concentrated flow. Stored soil and excess gravel shield the bed.
The released coarse and fine fractions enter the same downstream volume budget.
The earlier detachment pass still produces material on bare slopes and at
channel heads. This is a reduced version of sediment-impact feedback.
[Davy et al. (2026)](https://esurf.copernicus.org/articles/14/635/2026/)

Rock breaks down into stored regolith during recurring weathering seasons.
Rates vary with recharge, mapped rock, joint density, and existing cover.
Recharge can also dissolve exposed carbonate along open joints. Its volume
leaves the local domain as solute. It is reported separately from loose soil
and included in the combined volume balance.
The three refinement levels share 1.08 million model years of weathering.
Storm erosion uses the existing compressed response steps. The weathering
rates are regional model coefficients.
[Draix seasonal sediment model](https://doi.org/10.1029/2024JF008183)

Compare with `catchment_review <new-output-directory> 731 --parent 160
--rock-package --jointed --sediment-history`. Omit `--sediment-history` for the
251 regional baseline. Both use the same parent terrain and section geometry.

The held-out references are 4 km USGS 3DEP windows sampled at 4 m in
EPSG:26912. Buckhorn Draw covers 519700–523700 E and 4325400–4329400 N.
Mill Creek near Spring Canyon covers 631300–635300 E and 4265000–4269000 N.
Both generated previews use each seed's fixed 251 cross-section.

| Seed | Section width 20 m above floor, 251 → 256 | Gentle upland 24 m median, 251 → 256 |
| --- | --- | --- |
| 74 | 331 → 281 m | 0.58 → 0.86 m |
| 731 | 488 → 509 m | 0.25 → 0.33 m |
| 2026 | 275 → 290 m | 0.26 → 0.38 m |
| Buckhorn Draw | 122 m | 0.18 m |
| Mill Creek | 255 m | 0.45 m |

The roughness change approaches Mill Creek on 731 and 2026 and moves farther
from both references on 74. The wide 731 trough and regular gullies remain
visible. In the seed 731 domain, the rock-volume balance closes to about
0.025 m³ after about 2.23 billion m³ of cumulative rock conversion and erosion.
[USGS 3DEP elevation service](https://elevation.nationalmap.gov/arcgis/rest/services/3DEPElevation/ImageServer)

In one matched local seed 731 bake the 251 and 256 profiles took 47.9 and
61.4 CPU seconds, respectively. The added passes run during the regional
bake. Chunk generation samples its existing cached surface.

Two independent seed 731 regional runs produced identical bedrock, sediment,
and runoff rasters. The same binary reproduced the archived 251 rasters.
The 256 standalone survey and the native game also agreed on the Planet
checksum. In the native game, all three legs of the existing 36 m seed 731
route completed at 1 m voxel scale with flight disabled.
Native loopback discovery and join used seed 731, generator 256, and
1 m voxels. Host and guest both became ready and independently reported
Planet checksum `94e96314bb3e00a6`.

Reference implementations for deeper channel work include Landlab's
gravel-bedrock component and its 2026 extension for multiple sediment sizes,
goSPL's July 2026 groundwater and regolith release, and the pyfastflow GPU
implementation of GraphFlood for estimating reach-scale water depth and flow.
[Landlab extension](https://meetingorganizer.copernicus.org/EGU26/EGU26-19265.html) ·
[goSPL release](https://github.com/Geodels/gospl/releases) ·
[pyfastflow](https://bgailleton.github.io/research-softwares/pyfastflow/)

## Whole catchments and exploration routes in preview 255

The audit compares complete upstream basins on 4 m rasters in 4 km windows.
It rejects outlets with contributing cells in the outer two rows or columns.
The reference is Black Dragon Wash, Utah, with bounds 548000–552000 E and
4308000–4312000 N in EPSG:26912. Both generated runs use seed 74 and the same
preview 160 parent and jointed rock package. The terrain pass changes routing,
so the table uses the nearest enclosed basin to 1 km² in each run. The original
outlet fails the enclosure or area requirement after the change. The two
generated basin footprints share 99.17% of their combined area.
[USGS 3DEP bare-earth elevation service](https://elevation.nationalmap.gov/arcgis/rest/services/3DEPElevation/ImageServer)

| Measurement | Control | Preview | Black Dragon Wash |
| --- | ---: | ---: | ---: |
| Basin area (km²) | 1.006 | 1.000 | 1.005 |
| Channel heads per km², at 0.03 km² contributing area | 11.93 | 10.00 | 6.97 |
| Median ridge spacing (m) | 120 | 120 | 140 |
| Median tributary width, 0.03–0.10 km² (m) | 99.56 | 85.34 | 39.46 |
| Median tributary width, 0.10–0.40 km² (m) | 103.27 | 92.07 | 40.89 |
| Median valley width, above 0.40 km² (m) | 114.38 | 74.25 | 105.63 |
| Main stem length above the channel threshold (m) | 1,185 | 1,161 | 2,685 |
| Main stem slope breaks | 1 | 3 | 15 |

Widths use the contour 5 m above the channel bed. Narrower tributaries and
fewer heads moved toward the reference. Ridge spacing was unchanged, the
trunk narrowed too much and its profile still has far fewer slope breaks.
The basin comparison is one seed and one site.

The fine-grid pass uses the existing runoff network to cut around channel
heads where terrain already rises on opposite sides. This suppresses artificial
incision along directions assigned by the drainage solver on flat ground.
The incision carries a diminishing adjustment up the connected hillslopes.
It relaxes adjacent banks in one bounded pass and routes the resulting bed
again. Detached soil and rock count as export in the regional sediment budget.
The pass supplies no additional deposit or water stage.

The trail survey checks short segments for dry footing and grade. It pairs
two paths to the same destination and requires at least one third of the
alternate waypoints outside the first path's 24 m search buffer. The trails
follow existing ground, avoid potential ruin and encounter footprints and paint a gravel
tread. The tread clears low foliage and reserves space when selecting settlement
and freight pads. The two routes can form a loop, with the first path's start
available as the preview spawn.

A native daylight replay on seed 74 walked route one out and route two
back using fixed-step movement and collision. All 45 outbound and 52 return
legs reached their targets within 1.3 m with flight off. The replay froze
the local solar clock at noon and used one-metre voxels. Its manifest and
captures are under `target/reviews/valley-walk-74/local-noon-255`.

The normal Planet sampler for seed 74 produced a 360 m and a 419 m route
between the same endpoints. At 1 m voxel scale, 402 and 465 centerline
samples found solid footing, two dry clearance cells and a maximum change
in standing height of one voxel. The extra incision, rerouting and trail
survey took 280 ms in a 51.6 s regional bake. This was a voxel scan.

For the offline pair, run `catchment_review <output> 74 --parent 160
--rock-package --jointed`, then repeat in another directory with `--explore`.
Run `catchment_review whole 4 1 <bedrock.f32> <sediment.f32>` for a generated
raster or omit the sediment argument for the USGS surface. The production
Planet sampler uses version 255 and its own parent configuration. Run
`catchment_review live-trails 74` to check emitted blocks there.
The Planet benchmark accepts `--benchmark-world=planet --benchmark-seed=74
--benchmark-generator-version=255` on a build containing this preview.

## Jointed bedrock in preview 251

Seeded joint swarms vary the response to runoff across exposed bedrock. They
are evaluated once per mesh cell and affect detachment and sediment supply.
Their spatial field supplies no elevation displacement. The initial surface
uses less of the inherited coarse valley and a narrower transition to downstream
base level. Secondary fault damage fades toward the ends of each segment.
Resistant valley walls can remain steeper and channels carry more sediment
before depositing it.
These are regional model coefficients. [Field measurements of lithology, runoff
and drainage density](https://doi.org/10.1029/2024JF007684)

Use `catchment_review <new-output-directory> <seed> --parent 160 --rock-package
--jointed` for the paired offline comparison. Use `--weathering` in place of
`--jointed` to compare against preview 248's regional solver with the same parent.

Four 4 km windows were compared at 4 m spacing with USGS 3DEP elevation
from Black Dragon Wash, Little Wild Horse Canyon, and Capitol Reef. At the
fixed 248 section locations, the width 20 m above the floor changed from
288 m to 239 m for seed 74 and 619 m to 322 m for seed 731. Black Dragon Wash
was 256 m. A separately selected section on 731 was still 488 m wide.
The strongest sections for seeds 7 and 2026 were 199 m and 275 m wide.

The review also reports detrended 24 m roughness in the highest 35% of
elevations on grades of 18% or less. Median roughness for seeds 74, 731, 7 and
2026 was 0.58, 0.25, 0.08 and 0.26 m, against 1.44 m for Black Dragon Wash.
Seed 74 improved from 0.45 m and seed 731 declined from 0.31 m. The width
improvements are local and the smooth uplands remain a preview limitation.

In the native 1 m Planet review, seeds 74 and 731 each completed a three-leg
36 m canyon route using normal movement and collision. Both daylight captures
still show long, regular slopes on the far sides of the valleys.

## Uplift, storms, and wind in preview 248

Preview 248 keeps the earlier mapped rock units, plate faults, and direct mesh
transfer. The coarsest rock stage adds 36 intervals of 30,000 years of slow
uplift. Its rate varies with plate shortening and distance from the local
fault. Wind and river erosion use compressed response steps within that
history. The elapsed uplift time and response coefficients are model inputs
that have yet to be calibrated against a site's geological record.

Six recurring rainfall levels have a mean multiplier of one. Storms can exceed
local infiltration capacity. The lower cutting threshold for weak rock lets
small drainage channels respond to those flows. The finer grids inherit the
changed surface.
There is no new height noise pass at refinement. Wind moves mobile soil along
the prevailing direction on the same irregular mesh. Higher ground upwind
reduces exposure. The model settles more sediment on sheltered or wet ground,
and abrasion requires both transported sediment and exposed rock. The wind
report tracks bedrock wear, cumulative soil entrainment and export. Bedrock
wear and export also enter the main sediment budget.
[Bedrock abrasion and wind-water interaction](https://doi.org/10.1002/2017JF004462) ·
[Directional geomorphological transport](https://doi.org/10.1145/3811336)

Run a matched review with `catchment_review <new-output-directory> 74 --parent
160 --rock-package --weathering`. Omit `--weathering` for the previous
regional process with the same seed and inherited terrain.

In a 4 km review against Black Dragon Wash (USGS 3DEP), seed 74's section
relief moved from 160 m to 215 m, against 223 m measured. Width at 20 m above
the floor moved from 371 m to 288 m, against 256 m measured. Seed 731 reached
238 m relief but was 619 m wide at the same depth. Both generated windows
still have smooth uplands and regularly spaced gullies. Median elevation
residual over 24 m after removing local tilt was 0.43 m and 0.50 m, against
1.26 m in the reference.

## Playable canyon and revised mesh transfer

Planet preview 246 uses the mapped-rock solver in a regional canyon. The same
tilted contacts control resistance, visible sandstone and limestone, and the
rock sampled below ground. The less distinct siltstone and mudstone units use
the existing stone block. The preview searches the solved surface for dry,
gently sloped footing near the canyon, with the usual Planet spawn as a fallback.

The geological stages now transfer bedrock and soil directly between irregular
triangular meshes. Each stage recovers a bounded residual of the same initial
terrain field. Slope failure uses elapsed model time, and coarse sediment retains
an incoming direction when its downhill path splits. The direction weighting
is a reduced adaptation of the momentum idea in the 2026 paper. The solver
keeps its existing graph and conserves sediment by normalizing the shares.
[Progressive terrain erosion](https://doi.org/10.1145/3658200) ·
[Stochastic geomorphological transport](https://doi.org/10.1145/3811336)

In the offline parent-160 review for seed 74, the 4 m and 2 m variants gave
1.642 and 1.644 km² for the selected basin and 279.60 and 279.62 m of relief.
This is agreement at the measured basin scale. At 20 m above the selected
valley floor the 4 m AXIOM profile was 371 m wide, versus 256 m at Black Dragon
Wash. The evolved tile still has too little relief on its broad uplands and
locally repetitive gully spacing. Seed 731 also ran at 4 m and produced a
different landscape, with 152.01 m of relief in its selected basin. Use the
same cross-section locations when comparing resolutions. The seed set is too
small to establish general terrain realism. [USGS 3DEP elevation service](https://elevation.nationalmap.gov/arcgis/rest/services/3DEPElevation/ImageServer)

## Canyon cut through a rock package

```powershell
cargo build --locked --offline --profile release-fast -p engine_worldgen --example catchment_review
$review = 'target/release-fast/examples/catchment_review.exe'
& $review target/rock-canyon-4m 74 --parent 160 --rock-package
& $review target/rock-canyon-2m 74 --parent 160 --rock-package --finer
```

This mode starts from the planet's relief and six synthetic rock units
with tilted contacts. Permeability, erosion resistance and slope failure
are separate rock properties. The geological stage evolves for 11,544
model years on an irregular triangular mesh beyond the review window.
It gradually lowers the downstream outlet, partitions effective annual
rainfall into recharge and surface runoff, and reroutes flow as terrain
changes. Detachment depends on runoff and slope. Sediment travels along
the same flow links, then deposits or leaves through an open boundary.
Weak banks can fail and deliver loose material to lower cells. The
recharge term leaves the surface budget. The prototype treats that term
as an effective loss from surface runoff.

`sections.csv` records the solved surface, the bedrock surface, and the
five contacts at 4 m intervals along three cross-sections. The first
comparison used a Capitol Reef holdout at 479000–483000 E and
4233000–4237000 N. The final holdout uses Black Dragon Wash at
548000–552000 E and 4308000–4312000 N. Both use 4 m USGS 3DEP exports in
EPSG:26912.

In the earlier prescribed-wall solve, the three exported sections had 176 m, 206 m,
and 226 m of rim-to-floor relief. A separate comparison picks an 880 m
transect by the same bilateral relief rule in all three windows.

| Window | Bilateral relief (m) | Terrain at least 45° | Largest connected steep face span (m) |
|---|---:|---:|---:|
| AXIOM | 228 | 11.38% | 2,576 |
| Little Wild Horse Canyon | 163 | 6.36% | 1,888 |
| Black Dragon Wash, held out | 206 | 5.30% | 2,175 |

Steep faces were measured on a common 8 m display grid, joining adjacent
cells with a slope of at least 1 and similar slope directions. The
comparison reveals overly uniform rims and continuous cliffs in the
prescribed-wall prototype, even where overall canyon depth was comparable.

### Mobile gravel bed

```powershell
& $review braid target/braid-review 74
```

This separate 512 m by 1.8 km reach receives 72 m³/s of water and an
assumed bedload supply. Downstream cells share discharge and transport
sediment. Deposits redirect later flow, and the mobile bed has a finite
thickness above a resistant substrate. The output contains the initial
bed, evolved bed and discharge per cell as row-major float32 rasters.
The discharge field describes routed flow and is used for the map
overlay. The model is a reduced downstream routing experiment based on
[Murray and Paola](https://www.nature.com/articles/371054a0).

## Paired erosion review

Run the offline prototype from the repository root.

```powershell
cargo build --locked --offline --profile release-fast -p engine_worldgen --example catchment_review
$review = 'target/release-fast/examples/catchment_review.exe'
& $review target/catchment-4m 74 --parent 160
& $review target/catchment-2m 74 --finer --parent 160
```

The example picks two separate inland receiver reaches on the same spherical
planet. Each 4 km square has open edges and a fixed downstream control near
the planet reach. The canyon uses tilted, alternating rock resistance, thin
soil and 34 m of outlet lowering. The valley uses thicker soil, higher
hillslope transport and 8 m of outlet lowering. Both use the same routing,
bed incision, bank weathering, transport and deposition. The starting
landforms follow continuous fall lines between the planetary ridges and
valleys. The tributary cuts use widths in metres, and the canyon's steep
walls expose resistant bands. Crests rise where the surface falls away on
both sides over 40 m, and existing local summits get compact caps over
their shoulders. This inherited relief enters before the sediment budget
starts. The regional solve
splits water and sediment among downslope neighbours and concentrates
stronger flows into channels. Excess sediment settles over a 70 m length.
Deposited material affects the next step's surface and drainage. The outlet
mask and comparison measurements use a single receiver per cell so the
generated and observed drainage areas have the same definition.

The solve refines from 32 m to 4 m. The finer run halves the time step and
splits the last stage's duration between 4 m and 2 m. Both schedules represent
444 model years. Parameters are experimental. The water budget covers
steady effective rainfall inside the window and runoff exported at the
perimeter. Sediment accounting includes the change introduced by grid
refinement.

The overview shows starting relief, evolved relief, and soil and drainage,
with the canyon above the valley. The starting surface includes regional
relief perturbations and the outlet drawdown. Blue identifies cells over the
runoff display threshold. Grey in the right panel marks terrain that drains
elsewhere.

The cache files are row-major little-endian float32 rasters. Add bedrock
elevation and sediment thickness to obtain surface elevation. Runoff is in
cubic metres per model year. The outlet raster uses 1 inside the selected
drainage area. The report gives the chart origin and unit axes. Local cell
centres are at `(column + 0.5, row + 0.5) * spacing_m`. The same in-memory
surface can be sampled in chart metres for 1 m and 0.25 m columns. This
example runs separately from the Planet generator and game renderer.

## Matched elevation comparison

```powershell
& $review measure 4 target/catchment-4m/canyon-bedrock.f32 target/catchment-4m/canyon-sediment.f32 target/catchment-4m/canyon-outlet.f32
& $review measure 4 target/reference-canyon.f32
```

Provide a 1,000 by 1,000 elevation-only grid for a 4 m reference, or 2,000 by
2,000 at 2 m. Elevations must be finite and in metres. With an elevation-only
grid the comparison selects the dominant perimeter outlet and adjacent
outlets within 160 m. Catchments can be truncated by the 4 km window.

Widths are medians from cross sections that reach 5 m above the bed on both
sides within 240 m. The analysis bins them by geometric contributing area
and counts channel heads at 0.03 km². Ridge spacing uses cross-window
transects and a 6 m local rise threshold. Profile breaks are slope changes
of at least 0.08 and a factor of two on 48 m main stem segments. Prominence
uses saddles within the window on an 8 m grid, with the dominant summit
censored by the window and a 10 m minimum for the remaining summits. These
definitions make the comparison reproducible and sensitive to grid alignment.

The reference tiles were exported from the [USGS 3DEP elevation
service](https://elevation.nationalmap.gov/arcgis/rest/services/3DEPElevation/ImageServer)
as 1,000 by 1,000 float32 GeoTIFFs with bilinear resampling and a 4 m output
pixel. The Utah canyon window covers 515000–519000 E and 4269000–4273000 N
in EPSG:26912. The Tennessee valley window covers 242000–246000 E and
3942000–3946000 N in EPSG:26917. To use them with `measure`, export band
one as little-endian float32 in its original row order without a file header.

### Oblique geometry and sections

Render the elevation grids with the same oblique camera and scale.

```powershell
& tools/Review-CanyonGeometry.ps1 -Candidate target/catchment-4m -Output target/catchment-4m/geometry.png
```

The default reference is the Little Wild Horse Canyon raster. For a second
site, pass its 4 m raster and label through `-Reference` and
`-ReferenceLabel`. The tool picks one deep, two-sided 1.2 km section per
tile. Widths at 20 m and 60 m above the local floor describe those sections.
The console reports how the candidate section changed from its initial
surface, plus local elevation residuals at 24 m and 96 m offsets across
each window. Use `-CandidateSpacing 2 -CandidateSectionFrom4m <4m directory>`
to compare resolutions at the same candidate section.

## First prototype, seed 74, generator 160

| Measurement | Canyon, 4 m | Canyon, 2 m | Valley, 4 m | Valley, 2 m |
|---|---:|---:|---:|---:|
| Outlet area (km²) | 2.826 | 2.826 | 2.801 | 2.810 |
| Relief (m) | 316.08 | 316.10 | 150.06 | 150.03 |
| Exposed rock | 44.68% | 45.21% | 1.08% | 1.05% |
| Median width above 0.40 km² drainage area (m) | 107.24 | 195.91 | 367.47 | 380.05 |
| Absolute sediment residual (m³) | 0.041 | 0.074 | 0.237 | 0.110 |

In the last 500 m of the selected canyon catchment, about 52% of the interior
surface gained more than 0.3 m at both resolutions. The main canyon width
changed by 83%, failing the resolution comparison. The corresponding USGS
widths were 32.47 m and 124.97 m. Both observed catchments yielded ridge
spacing and more than ten peaks above the prominence threshold. The generated
catchments yielded at most one such peak. These measurements describe two
terrain windows per source, with different outlet areas, and are a prototype
comparison with held-out elevation data.

## Fall-line revision, seed 74, parent generator 160

The revised solve uses the same two windows as the first prototype. At 4 m,
the median width on small canyon reaches fell from 211 m to 101 m. The
median on large canyon reaches varied from 132 m at 4 m to 119 m at 2 m,
versus 107 m and 196 m in the first prototype. The revised canyon yielded
ridge spacing of 304 m and 316 m, compared with 144 m in the Utah window.
The generated valley still has smoother and less intricate slopes than the
Tennessee window. Absolute sediment residuals stayed below 0.12 m³ across
the four revised solves.

## Crest and summit revision, seed 74, parent generator 160

The canyon catchment had nine summits with at least 10 m of prominence at
4 m spacing and ten at 2 m, up from one at either spacing. At the
highlighted canyon crest, the surface rose 37.48 m at 4 m and 37.64 m in
the finer solve sampled at the same location. The corresponding valley
rise was 12.67 m and 12.75 m. The orthographic comparison shows identical
1.8 km crops at true vertical scale.

## Steep gorge review

This prototype borrows progressive refinement from Schott et al.,
[Terrain Amplification using Multi-Scale Erosion](https://doi.org/10.1145/3658200)
(SIGGRAPH 2024). The authors' [released preset](https://github.com/H-Schott/MultiScaleErosion/blob/main/src/code/src/main.cpp#L90-L139)
carries the whole evolved surface forward and runs separate stream-power,
thermal stabilization, and deposition passes. As the grid gets finer, that
preset reduces the incision and deposition iteration counts and increases
thermal iterations. AXIOM samples its canyon template again at each
resolution, adds the interpolated bedrock change, and repeats coupled
incision and deposition with a hillslope pass after each step.

The reference uses slope-weighted flow with an exponent of 1.3, bounded
stream power, and a spatially varying talus angle. AXIOM uses a flow
exponent that grows from 2 to 4 with runoff, shields incision with existing
sediment, and uses a constant critical slope for soil diffusion. Its
priority flood routes through depression spill levels. The paper separately
describes elevation retargeting and multi-scale breaching as optional
post-processing outside the released preset sequence.

`--extreme` keeps the same selected windows and adds a layered gorge profile:
62 m of outlet lowering, stronger slope-aligned spurs and a deeper tributary
profile with a narrow inner cut inside broader shoulders. The cuts are initial
landforms guided by routed drainage, not sediment removed by the subsequent
solver. The soil-covered valley remains a control.

```powershell
& $review target/gorge-4m 74 --parent 160 --extreme
& $review target/gorge-4m-half 74 --parent 160 --extreme --half-step
& $review target/gorge-2m 74 --parent 160 --extreme --finer
```

Compare the two 4 m runs for time-step sensitivity. Compare 4 m with
`--half-step` against 2 m with `--finer` for the spatial comparison: both
have the same time steps, but the latter spends the final 12 model years on
the 2 m grid. Keep this profile offline until the cross sections and drainage
remain stable across those runs.

Seed 74, parent generator 160, same canyon window as the fall-line revision:

| Measurement | Earlier 4 m | Gorge 4 m | Gorge 4 m, half step | Gorge 2 m |
|---|---:|---:|---:|---:|
| Relief (m) | 323.23 | 388.15 | 387.82 | 388.63 |
| Large-reach median width (m) | 132.17 | 72.86 | 71.24 | 75.41 |
| Ridge spacing (m) | 304 | 240 | 240 | 224 |
| Selected basin area (km²) | 2.531 | 2.669 | 2.668 | 2.665 |
| Peaks over 10 m prominence | 1 | 12 | 12 | 14 |

The large-reach width changed 5.9% between the runs with matched time
steps. The measured main stem length still changed from 1,985 m to
2,441 m. The Utah reference's ridge spacing is 144 m. This is an offline
result for one canyon window; the gorge does not replace live Planet terrain.

The next gorge revision varies bed thickness without reversing layer order.
The same bed coordinates control ledge relief and rock resistance. Two
intersecting joint sets weaken the visual ledges and lower the contributing
area needed to cut a small ravine. Joint influence tapers out on large
reaches. The layered canyon profiles use the varied beds; the soil-covered
valley does not. These review profiles do not change live Planet terrain.

The extreme profile also uses the fine rill pass. It follows steep fall lines
at 22 m and 49 m wavelengths, with a fade at channel floors and window edges.
On seed 74, parent 160, ridge spacing fell from 240 m to 168 m at 4 m grid
spacing and 160 m at 2 m. Relief stayed at 387–388 m. The large-reach
median width was 84.50 m at 4 m and 79.00 m at 2 m. The selected drainage
area changed from 2.781 to 2.657 km²; the refinement still changes some
drainage divides even though the main-stem length is within 3%.
