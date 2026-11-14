# Planet visual continuity, 8 November 2026

This follows the [civilization integration](PLANET_CIVILIZATION_286.md) and the
[visual audit](../design/VISUAL_AUDIT_20261108.md). The user's town screenshot
correctly exposed bright wedges at the resident/distant terrain boundary.
The same wedges appear in native captures even after every requested resident
mesh has arrived. Chunk counters alone do not certify visual continuity.

## Corrections

The previous fallback terrain pass wrote every triangle at the same artificial
reverse-Z depth, `1e-8`. This created two concrete errors: clouds classified the
pixel as clear sky, and strict `Greater` depth comparison selected the first
fallback triangle rather than the nearest one. Planet shell triangles precede
the refined patch, so distant, fogged geometry could win a boundary pixel.

The renderer now finishes the ordinary scene, copies its depth into a read-only
occupancy mask, and draws fallback terrain with its real depth only in previously
uncovered pixels. This preserves ordinary geometry, nearest-surface ordering,
cloud occlusion and depth precision. It adds one scene-sized Depth32Float texture
(3.52 MiB at 1280x720), a depth copy and a render-pass boundary. Existing terrain
GPU timings include all of that work. Resize recreates the mask and its binding.

Exact terrain proxies and seam curtains also used the prop projection on Planet,
while their parent terrain used the spherical chart projection. All negative-AO
terrain geometry now shares the chart projection and the exact proxies receive
the corresponding normal frame. Existing resident transforms retain priority.

The first material candidate makes PineLeaves shading follow the material key,
with the former colour inference retained only for unidentified legacy proxies.
Broadleaf side/underside reflectance increases by 12%; its sunlit-top and conifer
formulae are unchanged. Grass tuft contrast increases around its existing 0.90
distant fallback. Filtering remains enabled. This retains the distant fallback,
not a mathematical guarantee that the near-field mean is identical. Explicit
pine identity can also change the old partial species blend.

The first rebuilt native town still had the white wedges. Material-colour and
mesh-kind captures showed continuous terrain in the pale basin: the material
itself was green/brown, and the background was rendered terrain. This does not
rule out a foreground join exposing more distant terrain.

The second candidate removes the legacy finite-backdrop sky closure on continuous
Planet surfaces. Previously, all terrain faded completely into sky colour at
the configured 4 km flat-horizon range. Physical atmospheric scattering, cloud
fog and weather remain; flat worlds retain their finite edge closure. The shared
closure value also controls receiver visibility, keeping colour and reflection
response consistent.

The terrain collar no longer rejects a valid land join merely because a
zero-area projected side or a validated uniform vegetation face crosses it.
Those faces are retained unchanged, never counted as replacement land. Water,
edited terrain, non-finite and mixed/unknown geometry remain protected. Native
join counts and visible results are required before calling this successful.

The second candidate (`CFD97538904049AB3E703329686BA049C43E28C268AEC5EBC5778B8FCD786823`)
still showed the white wedges and still joined only 14 of 81 eligible edges.
Unfogged26 removed the white basin, confirming atmospheric whitening, but did
not establish continuous near-surface depth. The next correction separates
resident/edited/scene occupancy from ordinary procedural cover: only the former
is copied into the protection mask on Planet. Ordinary cover and fallback then
compete at their real depth. Fallback is restricted to resident-owned footprints,
so it cannot overwrite an adjusted unowned collar. Flat-world ordering remains.

The accompanying clear-day Planet candidate expands the ground-fog range toward
6 km, smoothly retaining the former settings in dense weather, caves and the
1500–8000 m orbital handoff. Height/falloff, local mist, cloud-layer fog and
planetary scattering are retained. It is an art/readability correction, not a
substitute for the coverage correction.

Material-ID19 identified the ground close-up's dense pattern as Gravel (family
33), with adjacent terrace tops in the rock family (3). The content mapping,
not the colour diagnostic alone, establishes that family name. Captured
foreground RGB (111,242,114)
exactly matches the sRGB conversion of the gravel diagnostic colour. The gravel
candidate raises fines from 0.56 to 0.68, reduces crevice darkening from 0.30 to
0.10, narrows pebble values, and halves their painted directional-light factor.
Shapes, coverage, mineral palette, normals, filtering and the distant 0.80
fallback remain. This does not promise an identical near-field average.

No global exposure, continent, drainage, settlement placement or save-format
change is part of this renderer pass. It does not claim a finished app-wide art
pass. The shared candidate also contains the civilization owner's separately
documented economic-autonomy changes.

## Presentation investigation

The first integrated town run measured 3.84 FPS, with 242 ms spent acquiring each
presentation image. Repeating the earlier binary gave 3.90 FPS. Both stalled
before terrain or civilization rendering began. The GPU was observed with its
display inactive and low clocks. After the display became active, the earlier
binary measured 133.97 FPS and 0.028 ms acquisition at normal presentation.
The exact external cause was not established, and no system setting was changed.

The slow run retained 22 nearby missing meshes; the active repeat had zero during
measurement. Publication has per-frame budgets, so the 4 FPS presentation rate
also starved completed-mesh uploads. Increasing streaming budgets to hide that
condition would not repair the cause. These failed runs remain in the evidence.

An uncapped diagnostic of the integrated binary measured 129.34 FPS, but it is
not a normal-presentation comparison. The matched post-fix runs are recorded
below after completion.

## Native evidence

Evidence is under `target/visual-continuity-review/` and
`target/civilization-planet-review/`. Fresh isolated saves and caches are used;
there are no inserted review structures or altered terrain samples.

The initial ground beauty run is stored in the misleadingly named
`ground-albedo-before` folder: its first diagnostic argument was not recognized.
The subsequent `ground-albedo-diagnostic` run used the correct `--benchmark-debug`
argument and actually displays albedo. Both are retained rather than relabelled
as equivalent captures. Its beauty run had 4.6- and 7.9-second frame intervals
outside the short measured redraw span. Those remain unexplained; this pass does
not declare cold loading or all stutters fixed.

No unit tests or hosted CI were added or run. Full save/reload, live resize,
multiplayer and the whole interface require separate native acceptance; build
success and shader validation are not substitutes for those observations.

## Compiled candidate and diagnostic evidence

The completed renderer/material source built as
`target/visual-continuity-review/surface/axiom-286-surface.exe`, SHA256
`6A3F6DCE4CE90F58BD7E108E077E60B04FB379789D4E5D59757F38F5FFC732A9`.
SurfaceDistance37 is a permanent diagnostic of actual rendered eye-to-surface
distance: red <128 m, orange 128–256 m, yellow 256–512 m, green 512–1024 m,
cyan 1–2 km, blue 2–4 km, magenta >4 km. Cloud compositing is disabled in this
diagnostic, not in normal rendering.

In `surface-town-distance/captures/camera.png`, the reported wedge window
x=360..400, y=160..180 contains 861/861 yellow pixels. It contains nearby terrain,
not sky or the cyan far ridge. This is local post-change evidence only: there is
no earlier Distance37 capture, and the 256 m bucket cannot exclude smaller depth
steps. The join count remains 14 joined / 67 curtained / 81 eligible.

The final UI follow-on build is
`target/visual-continuity-review/final/axiom-286-visual-pass.exe`, SHA256
`251973C930AE78939D07A2B9597CB510A818B983580A803B5A71BB99E0AD4480`.
It adds the corrected Items catalogue heading to the same renderer/material
source. All 33 tracked source hashes were stable during its 1m28s release-fast
build. Shader composition validated 28 programs. There were 450 compiler
warnings and no final errors; the preceding private-stagger integration error
was corrected by the civilization owner through its resident presentation API.

The title was reviewed through the production offscreen renderer at three sizes.
The inventory was reviewed over an actual fresh generated town. Their initial
captures use the earlier CFD975 candidate; final UI capture is tracked separately.
Neither establishes every interface screen or interaction.

## Matched town results

All rows use fresh isolated save/cache directories, normal presentation, seed74,
Planet286, the same camera/time/weather, 1280x720, view12, physics, an 8-second
warmup and 65-second measured interval. These are individual shared-host runs,
not a repeatability study. Uncapped and inactive-display diagnostics are excluded
from this comparison and retained separately.

| Candidate | Average FPS | p99 frame ms | Worst frame ms | GPU ms | Gameplay ms | Process wall s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Earlier active baseline, 19BB47 | 133.97 | 15.664 | 121.189 | 4.540 | 1.788 | 162.578 |
| First visual, 88C8BB | 101.77 | 21.239 | 416.608 | 4.506 | 2.591 | 201.453 |
| Refined, CFD975 | 81.55 | 17.589 | 1498.845 | 4.323 | 7.230 | 218.750 |
| Final visual, 251973 | 139.94 | 15.330 | 117.952 | 4.368 | 1.669 | 181.703 |

All 9,099 measured final frames reported zero near/visible missing meshes; one
exceeded100ms. The final beauty capture removes the reported bright hillside
wedges and retains a more readable basin. The geometry diagnostic above supports
current local coverage; neither proves universal seam closure.

The civilization owner replaced a resource-survey path that could synchronously
generate nonresident terrain. The final CPU recovery is consistent with that
correction, but other changes and host activity prevent isolated attribution.
The final GPU work is slightly lower than the earlier baseline, not a large
performance breakthrough.

Cold loading is not established as faster. Final renderer startup was9.40s
versus1.13s in the earlier active baseline. Full target visibility first appeared
at engine clock53s and completed at61s, versus47.01s/55.01s earlier: eight seconds
after the final target was requested in both runs. These engine clocks are not
process-wall time or a measurement of when the first useful terrain was shown.
The wall column includes startup, cold generation/relocation, warmup, measurement
and shutdown. The full final visit was19.125s longer than the older baseline.

Raw receipts, images, frame CSVs and summaries are preserved under
`target/civilization-planet-review/` and
`target/visual-continuity-review/native-results.json`.

## Ground movement and interface

`visual-final-walk` uses the real valley route and ordinary fixed-step movement
with physics. All ten legs arrived successfully, with `flying=false`. The
55-second measured interval averaged 159.74 FPS, p99 14.324 ms, worst 30.309 ms,
and GPU 3.766 ms. These measurements are not comparable to the stationary town's
different visible scene. Reviewed captures include `bench-000`, `walk-05` and
`walk-10`; land remains continuous in those views. Gravel is quieter but still
too patterned beside broad rock planes, and the canopy still loses useful
separation in shade. These are remaining art defects, not accepted final polish.

`visual-final-inventory/captures/bench-000.png` verifies the final binary's Items
heading in the actual inventory over a fresh generated world. Portal guidance
remains on Recipes. The review preserves the existing navigation and theme.

The material diagnostic's support mask has a known interpretation limitation:
it marks valid generic family 39 magenta. The lime crop cubes are likely wild
Melons, which use that family and generic grain; their exact BlockId was not
sampled. Magenta in that diagnostic is not proof of an unsupported new material.

## Orbital review

`visual-final-orbit` completed the scripted native ascent, space view, cube-face
crossing and descent. The 110-second measured sequence averaged 133.78 FPS,
p99 14.808 ms and worst 97.403 ms. Its changing scenes and scripted relocations
make it unsuitable for comparison with the stationary town or walking average.
There was no runtime failure in this visit.

Reviewed images: `02-ascent.png`, `03-space.png`, `04-face-crossing.png` and
`06-landed.png`. This route viewed the night side. The globe remains visible
through the face crossing, but the bright atmospheric rim and coarse cloud
pattern overpower its dark surface. It is not daytime visual acceptance, nor a
new review of the generator's cold/warm epochs. The final file's name is also
misleading: the logged altitude is -56.3 m and grounded=false; the capture is
underwater, not evidence of a walkable landing. Its seabed is readable but dark,
and no broad underwater-quality or audio acceptance follows from that image.
