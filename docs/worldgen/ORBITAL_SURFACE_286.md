# Orbital surface quality, 2026-11-08

This pass improves the appearance and preparation of the existing Planet 286
surface. It does not replace its continents, bathymetry, erosion history or
playable terrain with an orbital-only height field.

## Implementation

- Orbital tiles retain the real vegetation-admission field, local temperature,
  wetness and a bounded approximation of local sky visibility. Woodland colour
  follows those fields. Modest, distance-filtered reflectance variation uses the
  existing 256 KiB noise texture; it does not create mountains or river paths.
- Body-space normals are computed once from the tile's actual heights and packed
  into two bytes. The vertex shader decodes them instead of sampling four extra
  heights and differentiating the sphere projection on every frame. Parent and
  child normals still blend through the existing geometry transition.
- The surface cache uses ten bytes per sample instead of six. The full fixed GPU
  atlas allocation increases by about 34.1 MiB. There are no extra surface draw
  calls. Tile transfer accounting includes all new fields, and direct uploads
  avoid an intermediate climate-array copy.
- Tile requests prioritise projected visible error, with low-priority prefetch
  for off-screen siblings. This avoids repeatedly refining the centre while the
  rest of the disc remains coarse. Partially baked tiles retain their progress;
  reaching the 32-tile work limit evicts one least-complete tile instead of all.
- Spherical tiles use their sample spacing as the terrain footprint. A 23-64 m
  orbital sample no longer requests metre-scale detail and invisible cave plans.
  The generator's existing filters select the appropriate real erosion stage.
- Broad coarse surveys and local erosion detail have separate bounded caches,
  including their watershed results. Cache hits retain recently used regions.
  Solver steps, sediment transport and terrain equations are unchanged.
- Frozen ocean samples lie at sea level rather than at the bathymetric bed.
  The tile cache format is `VXGT0005`; older files are rejected and regenerated.

The cached-normal approach follows the separation of terrain geometry and
shading described in [GPU Gems 2, terrain clipmaps](https://developer.nvidia.com/gpugems/gpugems2/part-i-geometric-complexity/chapter-2-terrain-rendering-using-gpu-based-geometry).
The implementation retains this engine's cube-sphere quadtree, not the paper's
grid layout. Texture transfer and colour handling were checked against the
[wgpu guidance](https://github.com/gfx-rs/wgpu/wiki/Do's-and-Dont's).

## Native options compared

`tools/Review-OrbitalSurface.ps1` fixes seed 74, Planet 286, the valley's position,
26 km camera clearance, daylight, clear weather, 1600x900 and view distance 12.
Every run uses an isolated fresh save/cache, normal presentation, a 25-second
warmup and 35-second measurement. No compilation overlaps these measurements.
PNG captures are unedited. Sparse screenshot readbacks are enabled in this
visual comparison; these are not capture-free GPU laboratory measurements.

| Option | Average FPS | Average GPU passes | Surface pass | Drawn globe tiles at end |
| --- | ---: | ---: | ---: | ---: |
| Previous compiled build | 163.03 | 2.099 ms | 1.632 ms | 37 |
| Twice the requested sampling resolution | 164.46 | 2.237 ms | 1.708 ms | 67 |
| Ecological shading, normals calculated while drawing | 162.71 | 2.239 ms | 1.695 ms | 37 |
| Ecological shading, cached normals | 164.17 | 2.095 ms | 1.614 ms | 37 |

All three options used the same comparison executable, SHA256
`BDE9220942F6DB1653979FF839FFEBA7BFDE0B5ABE2B730251EB5DEAE15EFFDE`.
The previous executable was `axiom-286-smooth.exe`, SHA256
`1AEEA577CD6CC842983BF86BE99749E1DC3A8FF2D997EA5CC91A9075C75496CF`.
Cached shading was selected. Merely asking for more samples did not correct
uneven refinement across the visible disc.

The next iteration added error-based priority and correct filtering. It drew
215 globe tiles at the end and made the surrounding relief visibly clearer, at
2.361 ms total GPU time. However, the local cover build regressed from 29.084 s
to 107.296 s while competing with the wider survey. The cold runs consequently
ended at different wall times; tile counts are not a same-time loading-speed
comparison. This exposed the shared FIFO erosion caches and prompted the cache
separation above. The intermediate build is retained as
`axiom-orbital-refined.exe`, SHA256
`4A6856C3EE17A8CF5C39D36694C5D50606E87FF1BB135BDE3D85ADA935C13A2A`.

## Final cold orbital comparison

After separating the erosion caches, the same daylight run produced:

| Measurement | Previous build | Final candidate |
| --- | ---: | ---: |
| First terrain cover publication, from its request | 22.650 s | 21.816 s |
| Valley cover publication after the review camera moves there | 4.013 s | 2.300 s |
| Detailed valley cover build | 29.084 s | 19.226 s |
| Global generation solve | 20.954 s | 21.463 s |
| Average FPS | 163.03 | 164.00 |
| 99th-percentile frame | 8.768 ms | 8.420 ms |
| Total measured GPU passes | 2.099 ms | 2.336 ms |
| Surface GPU pass | 1.632 ms | 1.917 ms |
| Drawn globe tiles at run end | 37 | 84 |
| Process wall time | 137.990 s | 136.505 s |

The detailed cover job improved by about 34%; overall world generation did not
get faster. The additional visible detail costs about 0.24 ms of GPU time on this
RTX 4090 at 1600x900. Average FPS stayed near the presentation limit. These are
single native runs, not a general claim of faster frames on every machine.
Both had one measured frame above 100 ms; the final maximum was 251.188 ms.
The endpoint still wanted additional globe tiles, so this is improved progressive
detail, not an instantly complete high-resolution globe.

The global terrain checksum remains `53e34beb3a962eac`. The final executable is
`axiom-286-orbital-final.exe`, SHA256
`19BB47D1BA65BF5967087477116B122519E4EEF84F82FE30573F86AAABDE58C0`.
The final build completed in 1m29s and the 14-file source receipt matched after
compilation. `target/landscape-cycles-review/final/Play-Planet-Orbital.cmd` starts
this executable with an isolated player save and no review settings.

## Lower orbit and flight

The final candidate also completed the 8 km view of the same valley and a fresh
110-second orbit, face-crossing and descent route. The latter finishes over an
ocean bed; it is not a repeat of the earlier physics-enabled valley walk.

| Measurement | 8 km fixed view | Orbit and descent |
| --- | ---: | ---: |
| Resolution | 1600x900 | 1280x720 |
| Average FPS | 161.06 | 163.67 |
| 99th-percentile frame | 6.299 ms | 6.811 ms |
| Maximum measured frame | 478.763 ms | 382.712 ms |
| Frames above 100 ms | 2 / 5639 | 3 / 18005 |
| Total measured GPU passes | 2.127 ms | 1.778 ms |
| Detailed valley cover build | 19.113 s | Different moving-camera workload |

Both runs retain screenshot readbacks. The long frames in the fixed views fall
outside the measured gameplay, streaming and render-CPU spans; their cause is
not established by these timings. The flight additionally traces a real 195.68 ms
reliquary site query. It reports missing nearby meshes in 39 measured frames,
peaking at 43 and returning to zero at the endpoint. Orbital views report no
missing local meshes, but that counter does not certify globe-tile boundaries.
The previous long-flight stall is not declared fixed by this shorter route.

Unedited captures are at `target/orbital-surface-review/final-day/captures/camera.png`
and `target/orbital-surface-review/final-close/captures/camera.png`. The flight
captures are in `target/landscape-cycles-review/final/orbital-final-flight/captures/`.
Visible relief and woodland continuity have improved; large biome-colour patches
and the difference between the regional erosion basin and surrounding terrain
remain conspicuous. No claim that all seams are gone follows from these images.

## Capture-disabled follow-up

Two additional fresh runs used the same fixed 26 km camera and 35-second
measurement with `-NoCaptures`. This also leaves the ordinary HUD enabled, so
compare these two runs with each other, not directly with the visual-run FPS.

| Measurement | Previous build | Final candidate |
| --- | ---: | ---: |
| Average FPS | 151.01 | 165.09 |
| 99th-percentile frame | 8.503 ms | 8.455 ms |
| Maximum measured frame | 739.606 ms | 22.492 ms |
| Frames above 100 ms | 9 | 0 |
| Total measured GPU passes | 2.186 ms | 2.263 ms |
| Detailed valley cover build | 27.825 s | 19.255 s |
| Drawn globe tiles at run end | 28 | 84 |

The final stationary view therefore ran without a large hitch in this sample,
at the normal presentation limit. This does not establish hitch-free travel.
The older build's cold global solve took 69.063 s in this follow-up, compared
with 20.954 s in its earlier run; the final solve took 20.984 s. That substantial
run-to-run variation prevents attributing the entire FPS or loading difference
to this patch. The detailed cover improvement repeated, while global solve
time is not claimed as an improvement. Long frames also occurred without
capture in the older build, so screenshot overhead alone is not an adequate
explanation for all observed stalls.

## Evidence and limits

Build logs, source hashes, receipts, frame CSV files and PNGs are under
`target/orbital-surface-review/`. `summarize.py` reads the native logs into
`results.json`; it does not run the game or tests. The flight harness and its
captures are under `target/landscape-cycles-review/final/`.

This is a local optimized `release-fast` candidate, not a new shipping-release
certification. No unit tests or hosted CI were run. The regional erosion feature
and the rest of the globe still have visibly different landscape character;
this pass does not claim uniform geological detail everywhere. Loading stalls,
ground streaming and every chunk boundary are not declared fixed by orbital
screenshots.
