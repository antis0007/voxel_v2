# Planet tributary drainage and DEM comparison

Generator **271** is an opt-in increment based on planet-wide headwater recipe
266. It fixes slope composition and adds sediment-driven bed abrasion. It does
**not** achieve USGS landform quality. Broad plateaus, insufficient connected
regional relief and native distant contour artifacts remain visible.

Source was integrated into the shared checkout without changing defaults,
persistence formats, stored edits, the imported DEM control, or the separate
regional scarp and tectonic previews. The gate was initially reserved as 270;
another concurrent change used 270 for `TECTONIC_TRAVERSE_GENERATOR`. This work
was moved to 271 before integration. Do not use provisional 270 artifacts to
identify the current shared generator 270.

## Implemented behavior

- Compose the parent height gradient with the signed local correction. The old
  scalar hypotenuse could only increase slope, even when deposited material
  flattened the parent slope. Convert the composed derivative through the cube
  chart metric. Include tile blending derivatives and the positive-height
  headroom derivative. Analytic projection tangents avoid four extra sphere
  projections per affected sample.
- Carry the regularized parent height through the existing cached 32/16/8 m
  solve. Only 271 stores this extra field. Partition the hot tile cache by recipe
  as well as seed, settings, parent checksum and tile address.
- Use incoming transported sediment as abrasion tools. The effect vanishes
  without incoming load, transport capacity or downhill grade, and when supply
  reaches capacity. Soil cover suppresses abrasion. The removed rock enters the
  same routed sediment load and existing deposition pass. Existing bed-depth
  and per-step relief bounds remain. No decorative trench or extra noise was
  added. This is a bounded effective-cycle approximation of the tools/cover
  mechanism, not a calibrated saltation simulation. The mechanism is motivated
  by [Sklar and Dietrich (2004)](https://doi.org/10.1029/2003WR002496).
- Retain bedding as rock erodibility and repose limits without imposing periodic
  contour displacement on the parent surface. This had almost no height effect
  in the two measured highland windows; it is not the explanation for all bands.
- Add a fixed-window sampler exporting height, slope and water fields, and a
  native review launcher accepting recipe, seed, scale, camera and clay/albedo
  diagnostics with isolated saves and a manifest.

The local solve still has 23 effective refinement passes (8 + 7 + 8), 448 m
cores, 128 m padding, 48 m blends and a 192-tile cache. Its duration is not a
number of years. This increment adds no civilization chronology. Existing
long-timescale approximations and the Hills/Ridged provenance API are documented
in [Mountain ravines and history evidence](MOUNTAIN_RAVINES_REVIEW.md).

## Actual screenshot and renderer observations

Both supplied images were inspected. The plateau image contains broad geometric
bands, a high coastal face, uniform gray lighting/material response and long sky
lines. The later image is a close view of repeated grape/flower billboards on a
terraced hillside. It is not a DEM reference image; vegetation/animation work is
owned separately.

The live process inspected was `target/upland-relief-review/axiom-reference.exe
--real-terrain`; its latest logged seed was 1805968922. That flag selects imported
reference recipe 268. It does not turn the procedural planet into a USGS-quality
generator. The current reference source has a 4 km DEM patch and 384 m blend
apron; a large mismatch to the surrounding procedural elevation can make an
artificial perimeter slope. Outside that patch the ordinary procedural path
still applies. Exact screenshot camera, seed and frozen executable source could
not be recovered confidently. Two plausible HUD-coordinate readings produced
either ocean floor or an interior DEM slope, so neither is claimed as a match.

Native clay captures retain dense contour-like steps at the near/distant mesh
transition. This proves the remaining visual issue is not solely surface color;
its precise renderer/LOD contribution is still unresolved. The long sky lines
also occur in the controlled native views. Orbital skyhook code intentionally
renders long tethers, but exact attribution of each screenshot line is unproven.
No renderer, streaming, vegetation, skyhook or user-session changes were made.

## Reproducible elevation references

The comparison uses the existing USGS 3DEP service samples from Black Dragon
Wash and Capitol Reef, Utah. Both are 1000 by 1000 float GeoTIFF grids at a 4 m
service sampling interval in EPSG:26912. This is resampled service spacing, not
a claim of native lidar accuracy. Four cells are averaged for the 8 m analysis.
No vertical exaggeration or normalization is applied.

| Site | Upper-left UTM easting/northing | Frozen GeoTIFF SHA-256 |
| --- | --- | --- |
| Black Dragon Wash | 548000 / 4312000 | `e5907f093afea2975f68cbc1691cfe7f0070472f2b8dcf8de8cc1dd21a19711b` |
| Capitol Reef | 479000 / 4237000 | `b8baf21f4f1f3eba74ced1711cd74d8944b88695bb0608e46642344bd02db69a` |

Source: [USGS 3DEP elevation service](https://elevation.nationalmap.gov/arcgis/rest/services/3DEPElevation/ImageServer).
USGS states its 3DEP products are available without use restrictions on the
[3DEP program page](https://www.usgs.gov/3d-elevation-program/).
The frozen files, GeoTIFF metadata, exact sampled fields and analysis scripts are
preserved with the review evidence. Two arid sites are a limited reference set;
they do not establish quality across other climates or all planetary terrain.

## Measured results

Each procedural survey uses a fixed 4 km chart window, 500 by 500 samples at an
8 m footprint. `planet_relief_review` contains the exact frames for seeds 74 and
731. Seed 1805968922 provides a coastal/ocean control. Slopes in the table use
the local cube chart metric to compare the reported slope with the sampled
geometry. The finite-difference comparison itself remains resolution dependent.

| Seed | Slope absolute error, 266 to 271 | Error p95, 266 to 271 | Mean absolute height change | Height-change p01 / p99 |
| --- | --- | --- | --- | --- |
| 74 | 0.11411 to 0.03240 (71.6% lower) | 0.38156 to 0.12510 | 0.138 m | -0.921 / +1.344 m |
| 731 | 0.11443 to 0.03195 (72.1% lower) | 0.38446 to 0.11852 | 0.218 m | -2.087 / +2.196 m |
| 1805968922 | 0.01759 to 0.01640 | 0.05556 to 0.05327 | 0 m | 0 / 0 m |

Height changes above 10 cm cover 23.7% and 26.5% of the two highland windows.
Extrema are -10.50/+14.21 m and -15.82/+10.36 m. These are net differences after
drainage redistribution and deposition, not direct per-pass incision depths.
The slope fix is approximate: the fraction with sampled grade below 0.3 but
reported slope above 0.65 rose from 0.006% to 0.091% for seed 74 and 0.013% to
0.069% for seed 731, despite the much lower overall and p95 error. Unresolved
subcell geometry and spatial exposure derivatives remain limitations.

| Field | Upper-terrain 24 m residual p90 | Metric slope p90 | Relief p05-p95 |
| --- | --- | --- | --- |
| 74 / 271 | 3.65 m | 0.424 | 264.2 m |
| 731 / 271 | 3.50 m | 0.418 | 254.5 m |
| USGS Black Dragon | 7.67 m | 0.716 | 292.5 m |
| USGS Capitol Reef | 8.04 m | 1.260 | 341.7 m |

Additional diagnostics cover slope and graph mean-curvature percentiles at
8, 16, 32 and 64 m; detrended, Hann-windowed radial power from 16 m to 8 km;
and priority-flood D8 drainage at 1, 10 and 100 hectare area thresholds. D8 uses
uniform diagnostic runoff, not the engine's hydrology or a history output.
Curvature, D8 and frequency measurements use nominal chart metres; their cube
projection distortion is not corrected. These are diagnostic comparisons, not
a strict geodesic or process-calibrated benchmark.

At 8 m, p90 absolute graph curvature is 0.01376/0.01298 per metre in 271 versus
0.01960/0.02551 in the DEM samples. The 1 ha drainage densities are 7.23/7.10
km per square kilometre versus 8.06/6.43. Drainage density alone therefore does
not establish matching landform character. The generated fields remain short
of medium-scale relief and connected large scarps. The new distributions remain
close to 266; **no improvement in overall USGS similarity is claimed**.

## Validation and cost

- Shared integration: 13 focused tests pass (7 mountain transport, 6 gradient,
  abrasion, cache, legacy and boundary tests). The shared runs took 48.25 s and
  38.05 s under concurrent work. Historical 241/252 checksums and frozen 266
  sample bits pass, including 266/271/266 cache interleaving.
- Physical heights match at 1 m, 0.5 m and 0.25 m voxel scales. Checks cross exact
  448 m tile boundaries and four-tile corners, and 16/32/36/96 m footprint joins.
  The surveyed 32 m footprint step is at most 0.000275 m; the largest sampled
  height difference across 0.125 m is 0.118 m.
- Nine exported fields (height/slope/water over three seeds) are byte-identical
  between the provisional local 270 and final 271. Only recipe identity changed.
- Native builds of the isolated snapshot succeed; the evolving shared app was
  not rebuilt in full after integration. Its production worldgen library passes
  the shared tests above. Baseline SHA-256 starts `7f8d7d58`; the 271 executable
  is `94cabb70c0c5cf81cb5bfa63d7175ffdc22c499be1c99cd2991e2d93ac17dfab`.
  The final build took 7m01s while several other builds were running.
- Initial matched sampling of 250,000 queries took 17.20/18.62 s (seed 74,
  266/candidate), 18.55/16.98 s (731) and 7.82/7.86 s (coastal control), including
  world creation. After subtracting the logged world solve, the highland query
  portions were 5.66/5.62 s and 5.06/4.94 s. Later equivalent 271 runs under
  contention took 22.75/46.83/41.50 s in total. These are receipts, not a stable
  speed comparison. No erosion-bake or FPS speedup is claimed for this increment.
- Native reviews use 1280x720 Vulkan, fixed weather/date/orbit/cameras, isolated
  saves/cache, 24 s warmup and 12 s measurement. Readback, concurrent compilers
  and the live game make their frame times unsuitable for a performance claim.

All four final 271 views and their baseline pairs were inspected at native
resolution. Fixed seeds are 74 and 731 at 1 m, and 1805968922 at 0.5 m. A close
clay pair for seed 74 uses 0.25 m voxels. Local contour shapes change in that
close pair, but conspicuous step bands and the broad, smooth distant terrain
remain. The 1805968922 ocean/coast control is visually unchanged. There is no
verified overall visual-quality improvement or performance acceptance.

The near-grid and distant-grid rendering should be investigated independently
of the measured heightfield. Both quarter-metre runs still had nine near chunks
missing at the end of measurement; frame-residency receipts accompany the
images. No native LOD continuity fix is claimed here.

Final pairs under `evidence/plateau-native/` (compare `bench-001.png`):

| Before | After | Physical camera bookmark x / z / y, yaw / pitch |
| --- | --- | --- |
| `74-266-color` | `74-271-color` | 344059.90 / 19852.79 / 830, 0.685844 / -0.48 |
| `731-266-color` | `731-271-color` | 219524.64 / 14842.46 / 830, -0.724 / -0.48 |
| `1805968922-266-half` | `1805968922-271-half` | 345526 / 16546 / 564, 0 / -0.2 |
| `74-266-quarter-close` | `74-271-quarter-close` | 345341.854 / 17773.834 / 357.625, 0 / -0.59 |

## Reproduction and integration

```text
cargo test --locked --offline -p engine_worldgen --profile release-fast --test relief_gradients --test mountain_transport
cargo run --locked --offline -p engine_worldgen --profile release-fast --example planet_relief_review -- 271 74 <output-prefix> 500 8
powershell -File scripts/review-planet-relief.ps1 -Executable <frozen-exe> -OutputDirectory <new-directory> -Generator 271 -Seed 74
```

The sampler also accepts `origin-x origin-z across-x across-z` after spacing.
Do not read its output as geological age or simulated river discharge.

Evidence root:
`C:/Users/antis0007/Documents/Codex/2026-11-06/task-10/`.
`plateau-integration.patch` contains the narrow merge preserving concurrent
tectonic and parallel-bake changes. `plateau-drainage.patch` targets the original
isolated snapshot. `plateau-deliverable/` retains the source increment.
`evidence/plateau-271-survey/` contains numeric fields, hashes, JSON metrics and
standardized numerical hillshades. `evidence/plateau-native/` contains untouched
native PNGs, launch manifests, frame CSVs and logs. `evidence/usgs/` retains the
two source GeoTIFFs. `analyze-plateau.py` and `geomorphology-metrics.py` reproduce
the comparisons with NumPy, Pillow and Matplotlib as review-only tools.

The current default generator and save format are unchanged. No player's save
was opened for these tests. Broad inherited application/save test failures from
the earlier increment are not represented as passing here. Gates 269, 270 and
271 are separate previews; this increment does not combine their landscapes.
