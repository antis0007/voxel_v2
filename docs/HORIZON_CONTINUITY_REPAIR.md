# Horizon continuity repair — 21 October 2026

This is an evidence ledger, not a declaration that every visual gate passed.
The horizon implementation owner is the terrain/horizon chat. Material,
vegetation/lighting, water, UI and renderer owners retain their working changes.
No hosted CI or additional target directory is used.

Current status (22 October): **full acceptance has not passed**. The integrated
adaptive/quality-column/dry-collar/local-source/performance candidate builds and
runs. Resident and horizon GPU validation now precedes publication. Ordinary
edit/collision publication, general boundary geometry, active silhouette
handoffs and the frame-time target remain open. Older evidence below describes
its named build, not automatic acceptance of the newest integrated source.

Latest immutable game is `209D76378BC3E7D53CFBD70A9F99FE9AC700023507F1D2EF21A5E3493257E028`.
Its 265 prebuild inputs match archive `6F3DDC739DB200DE5673F905D3A6ABD91D55E0161BF00AB4DF7ED3A49DED296D`.
Both are under the external Codex visualization folder's `horizon-builds/`.
Five CPU geometry/selector contracts and three actual RTX4090 local-source
contracts passed; the latter cover equal-revision COW, record and water changes,
late CPU completion and first quality enrollment. These are controlled resource
fixtures, separate from appearance acceptance.

Two native matched captures completed under
`target/horizon-continuity/quality-collar-v2-receipt-hill-{proxy,reference}/`.
The lower authoritative reference is `2530,-4150,128,2`, from the same eye53.62m
and other settings. The gray left cliff still differs substantially in shape
and shading: the selected nine-column refinement does not establish whole-view
quality. Exact selected/admitted column diagnostics are now in live source and
are not included in209D. Final motion and performance for209D remain pending.

The first integrated8D4A executable correctly refused startup: a reused Cargo
build-script executable validated an older shader composition (`cf03c0436d3d94ce`)
while runtime composed `36e0562c55e540bf`. Raw frozen source matched runtime;
CRLF normalization did not explain it. Forcing recompilation of the frozen
build script and composition module corrected the receipt.209D reports
`build_match=true`. The guard remains enabled and the failed run is preserved.

## Delivered evidence

- [Adopted native overlook](D:/voxel-horizon-review-20261021/source-priority-overlook/camera.png)
- Matched approach lighting: [before](D:/voxel-horizon-review-20261021/proxy-hill-beauty/camera.png), [after](D:/voxel-horizon-review-20261021/final-hill-beauty/camera.png), [full-voxel reference](D:/voxel-horizon-review-20261021/final-hill-voxel128/camera.png). The after/reference E211 build has byte-identical appearance/geometry code to adopted23AFF; only source scheduling changed afterward.
- [Final native motion recording](D:/voxel-horizon-review-20261021/source-priority-pressure-route/traversal.mp4), [coverage receipt](D:/voxel-horizon-review-20261021/source-priority-pressure-route/route-coverage.json), [decoded video/hash receipt](D:/voxel-horizon-review-20261021/source-priority-pressure-route/video-validation.json)
- [Matched three-target appearance measurements](D:/voxel-horizon-review-20261021/final-reference-pixels.json), [approach comparison](D:/voxel-horizon-review-20261021/rock-final-comparison.json), [bounded silhouette measurements](D:/voxel-horizon-review-20261021/rock-geometry-silhouettes.json)
- [Adopted-build timing/resource audit](C:/dev/voxel_v2_backup/reviews/forest-repair-20261021/cloud-ab-post-rock/source-priority-physics-120-readonly-review.md), [named hardware](D:/voxel-horizon-review-20261021/final-hardware.json)
- [Frozen adopted source](D:/voxel-horizon-review-20261021/source-priority-source-23AFF9F6.zip), [per-file hashes](D:/voxel-horizon-review-20261021/source-priority-source-23AFF9F6.json), [narrow source delta](D:/voxel-horizon-review-20261021/source-priority-source-delta.json)
- [Unclosed ordinary edit-publication audit](C:/dev/voxel_v2_backup/docs/EDIT_PUBLICATION_AUDIT.md)

## Reproduction

The original screenshot/save is unavailable. The reproducible equivalent is
Hills seed `2738958700`, generator revision 25, one-metre voxels, feet
`(0,132,-120)`, yaw `0.4`, pitch `-0.2`, vertical FOV 70°, 1280×720,
resident view 4 chunks, partly weather, noon, fixed sky time 12, exposure 1,
wind 0. The CLI camera is `0,-120,100,0.4,-0.2` (clearance, not eye height).
The motion harness logs actual feet and eye positions separately.

Run `tools/Review-HorizonContinuity.ps1` with a rebuilt executable, unique
label, and `-Case still|route|motion|edits|stress|perf|world-switch`. Full route
cases take at least 280s, starting their motion clock after minimum coverage.
`-DirectVideo` records actual native readbacks at 20fps; the receipt distinguishes
real and repeated frames. `-EvidenceRoot` can put captures/test saves on another
drive without moving source, Cargo outputs or player saves.
Each output directory records executable SHA256, arguments, environment,
native logs, process memory, captures and runner acceptance. Captures are
excluded from performance acceptance. Reference runs require both reference
generation and later GPU publication/capture receipts.

Authoritative visible candidates, in voxel/metre X,Z coordinates:

| Target | Position | Source evidence |
| --- | --- | --- |
| Pale ridge | 2602,-3923 | Sandstone, surface 51; sampled line of sight clears terrain |
| Shore | 1653,-1725 | Grass centre, surface 16; surrounding 25 samples include 7 wet and 18 dry |
| Forest | 0,-3448 | Stone centre, surface 25; 6 canonical trees in surrounding 25 placement cells |

The earlier ridge candidate 2387,-2439 was occluded and rejected. Pale
Sandstone is legitimate source material here. Probe receipts are under
`reviews/horizon-continuity/final-source-probe/`; source agreement at points
does not establish mesh silhouette accuracy.

## Confirmed causes and changes

* Exact and preview paths classified terrain differently. Both now use shared
  landscape classification. Exact summaries are used through the 64m ring
  (~6km); farther rings use one fixed 8m cheap evaluator with a distinct policy
  key. Cache answers cannot cross footprint/policy classes. This is point
  sampling plus geometric filtering, not a claimed area-average query.
* Valley plan probing polluted the final canonical column cache with provisional
  heights. Probing now uses the uncached sampler. The observed retained 9m vs
  fresh 6m answer at (-264,-448) is covered by regression and real A/B/A cache
  checks.
* A global half-voxel sink submerged dry coastal knots. It is removed.
  Existing conforming ring splits and shared finest-owner samples remain.
* Desired centre and published geometry are separate. Immutable jobs retain
  source, edit and built-centre provenance. An old-centre result keeps its real
  footprint. Upload and publication both reject incompatible dependencies.
  Whole-mesh publication retains old cover through replacement; guard rings
  cover the allowed centre offset. No per-tile device-idle wait is added.
* The reveal ramp is removed. Cold uncovered destinations use the existing
  loading frame while streaming continues. Geometry is never translated to
  pretend it covers another location.
* Local water volume and cold saved region edits participate in dependency
  checks. Buried local changes do not invalidate unrelated visible work.
  Procedural base meshes are cached independently of edit-overlay retries.
* Whole-chunk edit replacement could exceed the output budget during ordinary
  physics, preventing first publication. Replacement now compares actual cells
  against procedural data, expands changed volumes by a two-voxel face halo,
  assigns non-overlapping 8³-voxel bricks inside chunk owners, subtracts those boxes and inserts
  clipped resident-mesher surfaces, including newly exposed faces.
* Exact fallback AO tags preserve resident AO, roof skylight and packed water
  metadata. All horizon paths now use a shared 10320-byte GPU-ready column mask:
  the 2112-byte dense prefix plus a bounded sparse off-window ownership table.
  CPU draw selection publishes complete vertical columns; partial children do
  not draw over a complete parent. The bit grid uses the actual draw-list
  centre and radial domain. Disabled masks retain the legacy strict-disc path.
  Procedural water clipping interpolates continuous bed depth only; material
  IDs and exact packed water metadata are never interpolated as enums.
* Indexed water shares source knots and edge crossings. Admission counts mesh
  output separately from resident mesher scratch. PNG encoding runs on a
  bounded asynchronous writer instead of blocking the render thread.
* New hot edited chunks carry cumulative derived change bounds rather than
  invalidating a whole 32³ column. Cold decoded records conservatively use full
  bounds; serialization and save version are unchanged.
* Full water cells can prove visual occlusion because the actual world path
  writes opaque colour and depth. A submerged fractional cell is also filled
  by the mesher; a free fractional surface invalidates the proof;
  camera-in-water/lava disables water proofs. This does not change physics.
* Omitted edit bounds travel with the mesh and are checked before publication
  and while an old mesh remains displayed. Exposed invalid omissions enter
  the actual loading view rather than drawing an obsolete wall. This is a
  correctness fallback, not a passed ordinary-travel continuity gate.
* With enabled column ownership, legacy vertex sinking and exact-proxy depth
  bias are disabled. Matching boundary geometry still needs motion acceptance.

The all-far-exact experiment was rejected: its cold build took ~94.9s under
contention. The cheap far evaluator still has approximation errors beyond the
exact-summary domain. No full-resolution generation of the entire far view
is required by the production policy.

## Declared work bounds

One horizon worker, one-result channel, one pending upload, 160MiB mesh output,
64MiB maximum page, normally 8MiB upload/frame (pressure fixture supports
smaller aligned budgets), 64MiB maximum per-edit raw mesher output. Existing
published GPU cover remains while pending replacement is prepared. Resident
streamer limits remain separately reported. Full total cache/peak memory
acceptance still requires the isolated runtime matrix.

## Verification ledger

### Earlier integrated candidate

`target/horizon-continuity/coherent.exe`, SHA256
`9CCF832698F9825A6EDF01DD730F4AE60EB50A74D1DA5CBF67D9F3E82B60774C`.
Normal generation remains revision 26; reviews explicitly request generator 25.
The current local build passes 38 render/horizon contracts (11 opt-in cases are
ignored by the ordinary filter), six production water-mesh checks and the
actual Vulkan edited-LOD regression. Previously run native shadow, water-ownership,
provenance/upload and resident-column contracts have separate receipts.

Changes after the historical runs below:

* Published resident columns survive pending changes to desired vertical sets.
  Both colour and shadow draws use the published set. Pins and trim guards
  retain actual old members instead of pretending desired buffers exist.
* Whole-target validation memoization includes source/config/root, exact edit
  snapshot, eye, radius and coverage words. It is cleared when omissions change.
  Measured average horizon CPU time fell from 3.400ms to 0.457ms in the matched
  active-physics 1440p run; that run still failed the total frame budget.
* Exact tree placements through 4.2km use individual compact crowns, rather than
  thinning to enlarged random representatives. Material, base, top and species
  derive from the canonical placement source. Changed trees remove the complete
  proxy and replace its source bounds. More distant aggregate crowns remain an
  approximation. The retained tree-summary cache separates world and policy.
* Coarsened resident water uses the exact existing unit-water mesher, avoiding
  the observed 1.135m elevated sheets. Parent and resident water share broad
  optical weighting and bed colour; ownership edges taper geometric wave
  displacement, including diagonal neighbours. A production-WGSL compute check
  passed 90 side/corner/negative-coordinate cases at all three voxel scales.
* A fifth shadow cascade preserves the original four near cascades. Actual
  parent/resident ownership is respected in shadow draws. Receiver bias and
  moving canopy shadow appearance still require visual acceptance.
* Cold saved edit sources now restore their saved partial-water map. A live
  chunk supersedes its older saved record even when it has no partial override.
  The regression writes a real region, cold-loads it and checks a newer live edit.
  Cold overrides also enter edit-bound admission, rather than only the mesher.
* Proofs cache their projected footprint under exact floating-point bounds and
  the whole eye voxel. Cache hits allocate no weak pins. Water-volume proof
  dependencies are the encountered free-surface cells, including full cells
  without overrides; block identities at both plane and plane+1 remain checked.
  Drainage, a newly fractional free surface and removed overrides invalidate
  correctly. Submerged volume churn cannot change the opaque geometric proof.
* Water optics and the greedy/partial mesher selector agree with the existing
  full-cell geometry for submerged water. Complete production vertices and
  indices match bit-for-bit for interior levels 1, 7, 8 and no override at all
  three scales; exposed levels 1 and 7 differ. Only this verified equivalence is
  filtered from horizon overlay/dependency work. The vertical chunk-boundary
  case stays conservative because a lower source's top halo lacks its next
  neighbour. Simulation and saved volumes are retained.
* Installing a dirty source no longer re-caches its previous active LOD. Existing
  dirty invalidation also retires inactive siblings. The native regression checks
  ordinary unchanged-source reuse, stale active/inactive rejection, GPU byte
  accounting, revision advancement and published coverage.
* Two cloud endpoint/mass-floor shortcuts were reverted after controlled native
  measurements showed a regression. Other owners' cloud work is preserved.

`continuity-video-smoke` completed native motion recording: 1025 unique frames,
one repeated preceding frame, zero late frames, 1026 decoded frames at 20fps,
encoder exit 0. This checks the recording path and part of the shore approach;
it does not constitute the full route/edit acceptance matrix.

The preceding D1319D15 candidate completed `continuity-full-route` on D: with
55,335 actual render attempts, zero failed draws and zero ordinary-travel
ready-to-loading transitions. Its two loading intervals are the explicit ~90km
teleport and ~94km return (6.63s and 6.59s). All motion phases and the 766-cell
edit/94-tree-voxel removal/restoration fixture completed. The real 20fps native
recording has 5706 unique frames, 16 held frames, zero late frames and 5722
decoded frames (286.1s). This is a coverage receipt, not a watertightness or
collision/geometry atomicity certificate. Peak sampled process working/private
memory was 1,165,250,560 / 2,220,154,880 bytes, including capture overhead.

The real in-app world-switch fixture completed while CPU build was pending and
again while upload was pending. Only final seed 2738958702 published. The logs,
source ZIP/hash receipts and videos are in `D:/voxel-horizon-review-20261021/`.
Completed historical task evidence moved there is SHA-verified; original C:
paths remain usable through junctions. No source or player save was removed.

The D1319D15 isolated active-physics High1440 run still **failed**: p95 18.233ms,
p99 21.876ms, work max 34.992ms on RTX4090/i7-13700K/64GiB. Cadence p95 was
18.447ms and max 235.849ms; the latter is largely outside the work timers and
is not attributed to horizon work. Average GPU selected passes were 13.930ms
(clouds 10.621ms). A long interval without follow-up publication under continuous
physics remained despite zero loading transitions. The coherent candidate above
specifically addresses that proof/dependency workload; its native acceptance
must be recorded separately rather than inheriting the older route result.

Matched ridge-approach component captures showed why static views alone are
insufficient: the exact moving camera became fully resident after waiting, so
its owner/normal/material selection differed from the moving frame. The smooth
bright hill versus resolved voxel terraces remains a visual transition concern;
these component views do not prove a global exposure/AO/cascade error. No blind
colour or shadow-bias compensation was applied.

### Historical evidence and failures

Hardware identified by native Vulkan: RTX 4090, driver 610.88; CPU i7-13700K,
16 cores/24 logical processors. Review profile is release-fast, opt-level 3,
no LTO, 16 codegen units. Actual effective settings are in startup logs.

* Focused continuity suite: 8 passed, 6 deliberately ignored expensive/native
  checks in the earlier build. The integrated mask run passed 24 render checks;
  the disjoint-brick run passed 25. Six expensive/native checks remain opt-in.
  Capture writer pixel/flush test passed separately.
* Full retained/fresh A→B→A source-cache check passed (74.88s).
* Native Vulkan provenance/upload check passed: stale edit/world results
  rejected before and during upload; centre-A coverage retained while B was
  desired; four-byte upload pressure exercised ordered queue writes.
* Native before and after stationary captures exist. Earlier failed reference
  runs remain invalid. The current `mask-matched-*` runs successfully published
  and captured all three full-voxel patches at the same camera. Requested 64m
  bounds align to chunks: ridge/shore 96×96m, forest 64×96m. The pixel report
  under `lighting-audit/matched-review.md` is a bounded ROI comparison, not a
  whole-feature silhouette or motion measurement. Newly added orbital tethers
  appear in current root source and are unrelated to the terrain patch.
* Physics plus edit fixture exposed repeated failure to publish first cover.
  Both B497 and repaired-v2 runs failed this gate; the localized-volume repair
  fixed initial starvation in `mask-root-physics`: first build 6.45s,
  80,916,468 uploaded bytes over ten frames, zero stale results at that handoff.
  The run completed with live physics, edits and native PNGs. Later overlays
  still reached the 160MiB limit, so edit propagation is not accepted.
* `brick-physics` completed with valid displayed cover and exact brick patches,
  but later output still reached 160MiB and a frame stalled for 2907.792ms.
  Average 6.893ms, p95 9.274ms, p99 14.444ms are observed capture-run values;
  that maximum stall is a failure, regardless of averages. Root mask physics
  sampled 1,189,888,000 working bytes and 2,659,299,328 private bytes.
* Contended capture frame timings are not accepted performance measurements.
  Selected GPU pass sums omit resolve/present and are not full GPU-frame time.

## Latest continuity and publication evidence

The 310-second `support-delayed-upload-route` used a four-second worker delay
and 256KiB/frame horizon uploads. The earlier `coherent-delayed-upload-route`
failed with three ordinary ready-to-loading transitions at the old 2048m centre
guard. The replacement guard is 4096m and its source clipping margin expands
with it; terrain stays at its actual coordinates. The support run completed
82,152 render attempts, no failed draws, and **zero ordinary-travel loading
transitions**. The two deliberate remote teleports loaded for 11.13s and 15.03s
under this pressure. Its native recording contains 6393 real frames and 16
held frames, 6409 decoded frames at 20fps (320.45s), no late frames. SHA256:
`97AB2ED27A4B6A1B3C3D816A87EE73E860F99B1431E0CDD291AF4F346B34E4F9`.
Sampled process working/private peaks: 1,085,853,696 / 1,919,463,424 bytes.
This pressure route predates the supplemental edit-proxy integration below.

The `support-unfogged`, `support-ridge64`, `support-shore64` and
`support-forest64` runs published actual full-voxel reference patches at the
same camera with fog disabled. `support-reference-pixels.json` checks identical
settings/build and reports bounded ROI RGB errors. Those errors include
background and do not establish the 1–2px silhouette target.

Continuous active physics exposed genuine free-water changes immediately
outside resident ownership, repeatedly invalidating a costly parent build.
These are geometry dependencies, not safe changes to ignore. A bounded
supplemental source-only lane now prepares exact edited columns, including
saved cold edits and full vertical members, without enrolling generated sources
in simulation. Only complete GPU bundles gain ownership. The 512-column / 1GiB
budget is shared with retained owners; at most eight incomplete columns are
scheduled. Future parent selection remains independent of actual proxy owners.
Retirement requires both a compatible parent revision and proof that its actual
footprint baked the column rather than omitted it.

Cold saved source interest loads through existing region IO without seeding
physics. Its real-save regression passed. GPU tests passed dense/sparse colour
and shadow ownership, 512/513 capacity pressure, old-cover retention, origin
shifts, and rejection of edited inactive LODs.

The first sparse-mask executable overflowed the Windows main stack before GPU
startup. Retained CPU provenance now shares immutable masks through `Arc`;
the flat GPU ABI remains unchanged. The replacement `proxy.exe` booted and
captured the overlook. Its 120-second active-physics High1440 run completed
8046 measured frames: mean 14.710ms, p95 16.531ms, p99 18.747ms, max 70.116ms;
average selected GPU passes 13.972ms, clouds 10.605ms. Peak sampled process
working/private: 1,218,048,000 / 2,657,599,488 bytes. This was a liveness
diagnostic with bounded rejection tracing, not a final clean timing certificate.
It retained coverage but **failed follow-up publication progress**: only 4/118
proxy columns admitted and oldest pending age reached 118.5s. Continuous normal
dirty repairs outranked incomplete bundles. The subsequent queue fix prioritizes
missing minimum normal cover, then all members of the oldest incomplete proxy
column (with an XZ tie-break), then ordinary repairs/refinement. Both CPU-ready
and GPU-ready paths use that priority; validation and caps are unchanged.

Matched native `proxy-water-shadow-on/off` at eye
`(1201.1549,33.5142,-1286.2756)`, yaw 0.800130, pitch -0.2 reproduced dark
foreground quadrilaterals only with shadows enabled. The shadow shader casts
flat water while the colour shader displaces it. The repaired vertex path omits
homogeneous water triangles from opaque CSM casting, including encoded material
keys; water's land/tree-shadow receiver path is retained. The production Vulkan
depth fixture confirms resident/far/encoded water leaves depth clear while solid
and foliage still cast, alongside the existing dense/sparse owner cases.
The rebuilt shadow-on beauty capture reproduces the original camera and removes
the foreground self-shadow quadrilaterals with shadows still enabled.

`priority-water.exe` SHA256
`3DD4B8EF8BDD442853F0DFAA0A0B14041F3C3E967CB4BF8795A1515ED3C589AD`
passed 45 renderer CPU checks and the native depth fixture. Its frozen source,
Cargo lock/config and small runtime assets are archived with per-file hashes in
`D:/voxel-horizon-review-20261021/priority-water-source-3DD4B8EF.zip` and JSON.
No build input was newer than the copied executable when archived.

The integrated 120-second active-physics run restored useful late publication:
19 parent publications through the end (rather than stopping early), four stale
CPU rejections, 45/117 proxy columns admitted, 670 source-only completions and
1132 proxy mesh publications. Its retained/proxy pool reached 720,075,132 bytes
of the 1,073,741,824-byte limit; no capacity deferrals. Ordinary post-ready
loading and failed draws remained zero. Oldest pending age was still 113s:
upstream backpressure can fill the 64-mesh ready queue before source admission,
so this is a remaining latency issue, not a claim that every request completed.
Work mean/p50/p95/p99/max: 14.912 / 14.735 / **17.148** / 18.769 / 39.924ms
over 7939 measured frames. Selected GPU passes averaged 14.179ms, including
clouds 10.536ms, terrain 2.422ms and shadows 0.975ms. Sampled working/private
peaks were 1,230,876,672 / 2,924,617,728 bytes. This does not meet either a
16.67ms p95 target or the broader 144fps budget. Process-audit scope is recorded
separately: all 122 samples show constant analyzer/helper CPU counters and no
competing compiler/project renderer. This is an isolated measured failure,
not a result dismissed because an idle analyzer was present.

The final `priority-water-pressure-route` repeats the complete 310-second route
with four-second worker delay and 256KiB/frame horizon uploads. **74,149 actual
render attempts, zero failed draws, zero ordinary-travel loading transitions**.
The two explicit 90/94km teleports loaded for 11.170s and 11.008s. All route,
766-cell edit, tree-removal and restoration stages completed; final parent
revision 3 returned at the original centre. Video: 6393 real + 18 held frames,
zero late frames, 6411 decoded frames, 20fps, 320.55s. SHA256:
`84AF167D194725A3339FE4DDAB340B8578C2398B654DCC60FFDA0E55D83F6997`.
Sampled process working/private peaks: 1,133,109,248 / 2,043,138,048 bytes.
The final `priority-water-world-switch` also completed both actual pending-build
and pending-upload switches. Only final seed 2738958702 published.

The gameplay launcher now selects the immutable tested review executable
instead of the old 18 October review executable. The local mist preference is
preserved. Benchmark generator 25 is explicit; normal game defaults remain 26.

### Approach rock response diagnosis

The real pressure frame at wall-clock 160s has eye
`(2581.863,53.62,-3893.5686)`, yaw 0.600041, pitch -0.2. Video frame indices use
wall time; the route clock starts after minimum coverage, so equal frame numbers
from different cold starts are not automatically matched route positions.

The five `proxy-hill-*` component captures use this exact camera. Albedo and
ambient are already dark at the disputed outcrops; the white lobes are isolated
to directional light. The owner diagnostic keeps those hills blue (procedural
parent), unlike the resident foreground. A bounded authoritative source-ray
diagnostic identifies Basalt ID19 at (2599,-4281), (2560,-4141), (2548,-4140),
with roughness 0.46 and historical specular gain 0.22. The edge sample
(2623,-4280) is Grass. This source ray locates material; it is not exact proxy
raster identity or a whole-feature material-distribution measurement.

`proxy-hill-beauty` and `proxy-hill-voxel128` compare an additional bounded
full-voxel patch at the same camera. The source Basalt is dark and stepped;
its smooth proxy has strong white lobes. Historical source establishes that
the mesh specular values predate the physical BRDF and were artistic gains.
The newer conversion instead interprets gains as F0, saturating Stone 0.18
and Basalt 0.22 to reflectance 1 / F0 0.16. The material/lighting owners agreed
on an unset rock-family default of reflectance 0.5 / F0 0.04, shared by near
and far vertices. Explicit family and block overrides retain precedence.
Albedo, authored roughness and wet-film formulas stay intact; inherited wet
rock radiance will intentionally change with the corrected base reflectance.
This follow-up shader change requires its own GPU and matched-image receipts.

The correction is built as `rock.exe`, SHA256
`F066057FE0777D268FEE1B6BDD7654EBA25F5867DEB4DC52F4B781ED17E223CD`.
The 40-case Vulkan fixture executes the actual VS resolution/dry-F0 source
block with real material keys and palettes: resident/parent/exact fallback,
default/family/individual/explicit-zero precedence, and unchanged non-rock
families all passed. Naga and the optimized build passed. Its source/archive
receipt is `rock-source-F066057F.*` on D:.

Matched `rock-hill-beauty`, `rock-hill-voxel128`, `rock-hill-direct-light` and
`rock-overlook` completed in the actual game. Source hash comparison finds only
the VS fallback plus test-only diagnostics changed from the route build.
`rock-finish-comparison.json` verifies camera/settings and measures a mixed
175×65px ROI: proxy/full-voxel mean absolute RGB-channel error fell from
24.810 to 16.089; direct-display pixels above luma 200 fell from 808 to 422.
Those are image-space appearance observations, not physical radiance or
silhouette error. Silver lobes remain visible compared with the matte voxel
reference. The calibrated default is verified; **full rock handoff appearance
still fails**. Further arbitrary reflectance/roughness suppression is not a
substitute for normal/geometry correspondence. Performance on this exact
follow-up build is tracked separately from the earlier 17.148ms run.
The launcher advances to this compiled and image-verified `rock.exe`; the
previous `priority-water.exe` remains retained with its full pressure recording.

A controlled post-rock cloud experiment was rejected. Both staged builds used
the same frozen source/assets and original quality settings; only two cloud
shader candidates differed. Static 1440p control/candidate work p95 was
16.929/17.009ms; cloud GPU means 10.636/10.612ms. Geometry and configuration
counts matched. A 0.024ms mean change with worse p95 is not a convincing gain.
Live cloud sources stay at their original hashes. These static no-physics
screens are not active-physics performance acceptance.

## Final directional-response integration

The additional approach diagnosis separates two causes: historical artistic
specular gains were interpreted as physical F0, and smooth heightfield normals
responded differently from the source voxel stairs. The calibrated rock default
fixes the first. The second now uses the projected visible areas of signed X,
positive Y and signed Z source facets for directional GGX and Lambert/Oren
response. Back-facing light contributes zero; visible-area weights stay in the
denominator. The helpers consume the unperturbed macro normal, not texture noise.
The gate is the procedural-parent AO tag plus existing continuous rock-family
coverage. Resident/exact-edit meshes, foliage, water, explicit material overrides,
ambient/environment and local lights retain their existing paths. This is a
filtered appearance approximation, not a replacement mesher or exact face histogram.

The lighting verification executable is `target/horizon-continuity/facet-unrolled.exe`, SHA256
`E21109A7463CA1BD4D4E0AC5E580F20DC293D7A66569B4A78D2B251DACF18CEA`.
Its shader raw SHA256 is
`B4F40FAC54403691DEE4A424D5B66A4119576B85EA42235E93189971B1E2B6F9`.
The production loops were made explicit in X/Y/Z order. Actual Vulkan execution
compared old and new helpers over 4,386 cases with **zero bit differences**.
The 290 specular and 580 diffuse production-helper checks also pass. Naga validates
all composed production programs during the successful optimized local build.
The extra unrolling showed no useful measured speedup; none is claimed.

The matched 175x65 mixed approach ROI's proxy/full-voxel mean RGB-channel error
was 24.810 before the finish repair, 16.089 after the calibrated default, 9.253
with facet specular, and **4.916 with combined directional response**. These are
real native captures at fixed camera/settings, not a substitute render. The
final unroll is GPU-equivalent to the combined candidate. The remaining lower
grey outcrop discrepancy is visible in direct light and is not assigned to
environment lighting without further evidence.

The final E211 native `final-hill-*` captures reproduce the combined model's
4.9157509 error and71.6763 direct-display mean exactly in this ROI. Its separate
fog-disabled `final-unfogged/ridge64/shore64/forest64` captures use the same
executable, camera, arguments and environment except the requested reference
patch. All four completed generation, publication and capture validation.
Ridge/shore/forest mixed870-pixel ROI mean RGB-channel errors were
1.24215/4.44215/1.02759. These are bounded appearance observations, not a
substitute for silhouette or whole-route acceptance.

Independent native water-depth diagnostics on the geometry-equivalent rock
build compare visible land boundaries with bounded full-voxel references. In
48x40 target ROIs, maximum bidirectional boundary distances were ridge **1.414px**,
forest **2px**, shore **4px** (p95 0, 0, 2px). This does not measure hidden
contours, entire feature silhouettes or all moving handoffs. The shore maximum
does not meet the initial two-pixel target.

Static 1440p final screen: mean 14.115ms, p95 17.729ms, terrain GPU mean 2.943ms,
cloud mean 10.740ms. Geometry/settings match the prior static screens; there
is no video readback. The first A/B measured terrain costs were 2.825/2.949ms;
unchanged cloud costs also varied, so the complete frame difference cannot be
attributed to this lighting code. Reverse B had 2.516 CPU seconds of analyzer
activity during its measured window and is excluded from isolated p95 attribution.
The post-rock coherent active-physics control independently measured 17.661ms
p95. All of these exceed 16.67ms; the broader 144fps target is also unachieved.

## E211 lighting-reference native receipts

`D:/voxel-horizon-review-20261021/final-source-E21109A7.{zip,json}` preserves
234 source/configuration/asset files, including shared untracked work. No input
is newer than the executable. Archive SHA256:
`819699FE56D31E2F87CB25C3F1BC6423206C185EF58287114EBCF3C147CBAC12`.
HEAD is `6e162de491b5c52fed114abd9ad5ff9498b7f305` with preserved working changes.
The per-file delta from F066 confirms unchanged geometry, generation, streaming
and assets; production changes are in the world shader, with test module additions.

The E211 executable's 120s active-physics, no-readback High1440 run
(`final-physics-120`) completed on RTX4090 Vulkan driver610.88, i7-13700K
(16 cores/24 threads), 64GB RAM. Fixed camera `44,35,1,-1,-0.12`, view6,
cloud divisor3, noon/partly/exposure1, 12s warmup, uncapped. Work mean/p50/p95/p99
**15.153/14.822/18.020/19.594ms**, max33.793ms, 7,810 measured frames.
Selected GPU mean14.421ms: terrain2.669, cloud10.531, shadow0.968ms; selected
pass sum is not a complete GPU-frame measurement. Process peaks sampled every
500ms: working1,259,864,064B/private2,930,257,920B. No peak-VRAM certificate is
claimed. Adjacent render-attempt p95/max18.259/34.024ms is not display cadence.

The audit observed **3.421875 analyzer CPU seconds during measurement**; this
run is not analyzer-idle and its difference is not attributed solely to lighting.
Earlier analyzer-idle post-rock control independently failed at17.661ms p95.
Nineteen parent publications continued through stream seconds133–134, four
obsolete CPU results were rejected, and there were no failed draws or returns to
loading after initial coverage. Proxy state ended at41/119 admitted columns,
244 members, oldest pending118.93s, 610 source-only generations, 1,130 mesh
publications, 696,087,784B below its1GiB cap and no cap deferrals. This demonstrates
progress and retained coverage, **not bounded proxy completion latency**.

`final-pressure-route` completed the full310s motion/edit fixture with4s worker
delay and256KiB/frame horizon uploads: **70,786 actual render attempts, zero
failed draws and zero ordinary-travel loading transitions**. The explicit90km
and94km teleports loaded for10.774748s and10.869153s. The766-cell excavation,
shoreline and94-tree-voxel removal/restoration stages completed; final parent
revision3 published at the original centre. This fixture is not proof of
ordinary tool/collision atomicity or an observed walk through an excavated cave.
Video:6,383 real+28 held frames, zero late,6,411 decoded at20fps (320.55s),
native encoder finalized/exit0. SHA256:
`0031AEAA212571D1B94C705C0983647BD1B54C7C81A174E5EEE1A428A0750A53`.
Sampled process working/private peaks1,172,160,512/2,008,645,632B. Native
keyframes from overlook, shore approach/retreat, ridge approach/retreat and
elevated/forest portions were inspected; this does not certify every frame.

## Adopted source-admission completion repair

Runtime investigation found that dispatch and upload favored whole oldest
incomplete proxy columns, but **pending source admission still ordered all
urgent repairs by distance**. Nearby continuous repairs could therefore prevent
an old column's remaining source members from ever reaching those ready queues.
Source admission now uses the same missing-ordinary-cover / oldest-incomplete-
column lanes, with its previous radial ordering retained as the fallback.
Equal birth times use X/Z before Y or distance. Missing Y over a published older
column is repair, not falsely classified as missing minimum cover.

The heap is re-keyed in its reused vector allocation only when the original
capacity checks permit admission; those checks are repeated for every request.
CPU-ready128, GPU-ready64, mesh jobs64 and generating64 thresholds, scan/schedule
budgets, source reservations, edit readiness, deduplication and provenance remain.
The CPU-ready number is an admission threshold, not a newly proved hard bound
on every result waiting elsewhere. Full downstream queues still apply backpressure.
Two targeted priority regressions and the optimized/Naga build pass.

Adopted executable: `target/horizon-continuity/source-priority.exe`, SHA256
`23AFF9F6D4D7B45C00A0E457B861C48F30E0B385EFF89FD457925DA8E03C9A66`.
The gameplay launcher selects this immutable, native-tested build. Frozen234-file
archive `source-priority-source-23AFF9F6.zip` SHA256:
`AD5A2198A84AEA13F613FA91FF7F120E49B83E259422A1F090FE78C2A9CF2904`.
Its per-file delta from E211 contains only `streaming.rs` and the priority-test
addition in `edit_proxies.rs`. Shader, generation, geometry and assets are byte
identical; E211's matched reference renders retain their stated appearance scope.

The repeated native active-physics120 run ended with **116/116 columns admitted,
555 members and oldest pending0.00s**, versus E211's41/119 and118.93s. There
were896 source-only generations,1,326 mesh publications,21 parent publications
through stream seconds134–135, one obsolete CPU rejection and zero failed draws
or post-startup loading transitions. Proxy accounting962,523,216B stayed below
1GiB with no cap deferrals; sampled accounting peaked at967,961,588B. Recorded
admission ages p50/p95/max were0.24/8.09/22.61s, and maximum sampled oldest
pending age was22.45s. This is a strong improvement in this measured scene;
it does not prove a universal completion-latency bound under saturated queues.

On the same named hardware/settings, work mean/p50/p95/p99 was
**15.109/14.937/17.828/19.829ms**, max30.822ms,7,834 measured frames.
Selected GPU mean14.386ms: terrain2.485, cloud10.549, shadow1.104ms.
Endpoint977 meshes/406 draws/3,840,590 triangles is more geometry than E211,
so this is not an identical-trajectory cost isolation or a claimed frame-speed gain.
Sampled process working/private peaks1,277,575,168/3,317,424,128B. Adjacent
attempt p95/max18.087/31.024ms is not display cadence. Sampled analyzer/macro
counters were flat, with no other tracked renderer/compiler. The audit starts
0.777s after launch and ends0.952s before process exit; full lifecycle/window
bracketing and GPU-clock exclusivity are not claimed. **The16.67ms gate fails.**

The adopted23AFF `source-priority-pressure-route` repeats the full310s fixture
with4s worker delay and256KiB/frame uploads. **71,830 actual render attempts,
zero failed renders, zero ordinary-travel loading transitions.** Explicit90km
and94km teleports loaded for10.863846/10.839841s. All excavation/shoreline/tree
removal/restoration stages completed; parent revision3 published back at(0,0).
The native video finalized with6,394 real+18 held frames, zero late,6,412 decoded
at20fps/320.6s. SHA256:
`2BF5F64836ACBFBCA406E3D7EC29B59CE872C851FBA921EAC9A262C897840E54`.
Sampled working/private peaks1,131,712,512/2,018,332,672B. Actual ridge,
shoreline, elevated, forest and return keyframes were inspected. These receipts
detect loading and failed draws, not every possible geometric crack or visual
edit/collision mismatch.

`source-priority-world-switch` completed both actual pending-build and
pending-upload switches; only final seed2738958702 published. The final
`source-priority-overlook` uses the bookmarked matched daylight/exposure and
completed capture validation and visual inspection. All three processes exited0.

Root's native/compiler sequence is closed. The existing renderer performance
owner now coordinates one separately staged atmosphere-lookup hoist experiment,
with stage-only GPU equivalence validation before A/B timing. It has no live
adoption or measured gain in this ledger;23AFF remains the delivered build.

Follow-up stage-only component validation passed90 scenarios/46,080 RGBA float
components bit-identically (zero bit differences). It executes the actual
production vertex/flat-varying path against original fragment evaluation with
synthetic HDR LUTs and unchanged height blending. Nontrivial raster values and
both hardware/manual-filter full modules were checked;16 composed WGSL programs
validate. This is component equivalence, not a whole-cloud image comparison.
The receipt is preserved at `reviews/horizon-continuity/four-hemi-component.json`.

The initial coherent static30s screen used367 GPU meshes/224 draws/1,697,191
triangles in both runs. Control/candidate cloud means10.798/10.349ms; engine
work p9518.000/16.646ms. Candidate raw-CSV nearest-rank p95 is16.6624ms,
barely below16.67. Sampled GPU clocks match and background analyzer counters
are flat. This promising static result is **not active-physics acceptance**;
the same experiment's longer reversed-order physics runs and separate native
image pair are being validated. No root shader adoption follows from this screen.

## Subsequent integrated build: 097B

The four-hemisphere cloud lookup hoist is now integrated. It moves only four
frame-uniform atmosphere samples to the fullscreen vertex path. The actual
production-path component fixture passed 90 scenarios / 46,080 RGBA components
bit-identically. The full native image pair differed by one 1-LSB pixel in its
top 600 rows; larger differences were confined to animated foreground effects.
The reversed active-physics control/candidate p95 was 18.203/17.760ms, so this
does not pass the 16.67ms gate. Source shader SHA256 is
`D9317142035E7FD06BF3E47BB81FDE06FDC0342310CDDDDF20A5EAF852C517CA`.

Build `097B03682BE73F76B0A7FD71DDAC661BA47BF0BD9D7D5BD5929FDEB58A570B04`
also measures resident uploads separately and avoids rebuilding the complete
published-column selection on geometry-only replacements. Draw revision still
advances; presence/LOD changes still rebuild selection. The invariant mesh LOD
activation path explicitly republishes ownership. Five actual GPU contracts
passed, including retained pending columns and invariant LOD2→0 admission.

Its native 120s active-physics run at the named 1440p settings measured
**15.051ms mean / 17.786ms p95 / 18.933ms p99**, max37.787ms. Resident upload
mean/p95 fell from 1.9700/3.0604 to 1.1176/1.7537ms; total streaming fell from
2.5500/4.6116 to 1.6547/3.2930ms. This is a CPU phase improvement, not a whole
frame speedup: GPU mean14.324ms, cloud10.272ms, and acquire wait increased.
The historical 455.5155ms streaming hitch was not reproduced; its underlying
operation remains unassigned.

All116/116 proxy columns were admitted,555 members,oldest pending0s;
968,046,040 accounted bytes stayed below1GiB with no cap deferrals. There
were24 parent publications,zero stale results,zero failed draws and no ordinary
loading reversals. Peak sampled working/private memory was
1,199,280,128/3,211,300,864 bytes. These admission counts prove complete GPU
column presence, not equality with the latest gameplay/collision revision.

Frozen sources: [097B archive](D:/voxel-horizon-review-20261021/publication-cache-source-097B0368.zip),
[per-file receipt](D:/voxel-horizon-review-20261021/publication-cache-source-097B0368.json).
The [CPU timing audit](C:/dev/voxel_v2_backup/reviews/forest-repair-20261021/physics-window/publication-cache-resident-upload-review.md)
records the comparison and its process-isolation limits. The launcher remains
on23AFF until a full native continuity rerun validates the integrated build.

The empty-water-caster revision experiment was rejected: its native eligibility
preflight found **zero eligible empty→empty replacements**. It is not adopted.
A broader mixed-caster proof remains design-only. The solar-transmittance
lookup packet is isolated component-validation work, not live implementation.

Two bounded shoreline interior-sampling candidates were also rejected. Both
compiled and passed their shared-edge/water-area contracts, but matched actual
native voxel references retained a worst boundary distance of4px. The second
used16,384 extra canonical samples and refined10,201 cells, yet only reduced
land-mask disagreements from10 to9 pixels in the fixed48×40 ROI. The original
working horizon source was restored under exact SHA256 guards. See the
[decision receipt](C:/dev/voxel_v2_backup/reviews/horizon-continuity/shore-refinement/decision.json)
and [native measurements](D:/voxel-horizon-review-20261021/shore-priority-geometry-silhouettes.json).
The base native process exited0; its subsequent PNG validator failed under
PowerShell5, while the reference and comparison completed. The validator now
uses a compatible PNG signature conversion; no visual pass is inferred from
that harness repair.

Follow-up on the unchanged097B executable used a larger128m full-voxel patch
(actual aligned bounds1568,-1792..1728,-1632), with a native mesh-kind capture
to identify reference ownership. The same48×40 shoreline ROI measures **1px
maximum/p95 boundary distance**, five mask disagreements,64 boundaries in each
image. This demonstrates sensitivity to the bounded reference footprint; it is
not a production geometry improvement or proof of every coastline. The prior
64m4px result remains recorded rather than silently replaced. Receipt:
[larger-reference comparison](D:/voxel-horizon-review-20261021/publication-cache-geometry-silhouettes-span128.json).
Moving handoff quality remains a separate visual check.

The first current-driver097B pressure recording is invalid: its native recorder
aborted after a real49.529631s adjacent-attempt gap before initial publication
(frame9857,wall57.334107s). Prior/next frame work was0.5313/0.8730ms; the
gap is outside recorded engine-work timers and its cause is unknown. The
30-second repeat limit was retained. Its incomplete MP4 must not be delivered
as motion evidence. The native route itself continues separately; a fresh
recording is required. Current hardware receipt reports NVIDIA617.42 rather
than the earlier610.88, so cross-run timing is not a controlled driver comparison.
Its completed coverage receipt contains66,805 actual render attempts,zero failed
renders and zero ordinary-travel loading transitions. Explicit teleports loaded
for11.116821/10.899655s. This remains coverage-only evidence, not a valid video.

The unchanged097B retry completed the full310s pressure fixture with4s worker
delay and256KiB/frame uploads: **72,107 actual render attempts,zero failed
renders,zero ordinary-travel loading transitions**. Explicit90/94km teleports
loaded for11.315884/10.997318s. Excavation,shoreline/tree removal and restoration
completed. Native video finalized and decoded **6,287 real+127 held=6,414 frames**
at20fps/320.7s,zero late,encoder exit0. SHA256:
`4890530483B0A122B58725457D86565B9D2CD9A6FD919394ACFF6BF087B9E2CA`.
Sampled working/private peaks1,170,104,320/2,141,270,016 bytes. Actual shoreline,
ridge,return and elevated keyframes were inspected; smooth proxy versus voxel
detail changes remain visible. This is not ordinary edit/collision atomicity or
a universal crack/silhouette certificate.

- [Integrated native motion](D:/voxel-horizon-review-20261021/publication-cache-pressure-retry/traversal.mp4)
- [Actual coverage receipt](D:/voxel-horizon-review-20261021/publication-cache-pressure-retry/route-coverage.json)
- [Decoded video receipt](D:/voxel-horizon-review-20261021/publication-cache-pressure-retry/video-validation.json)

The097B pending-build/pending-upload world-switch sequence completed with14
captures and exit0; only final seed2738958702 published. The matched daylight
overlook completed five captures and visual review. Both use the frozen097B
executable rather than the rejected shoreline candidates.

The fresh current-driver120s active-physics result **fails**: mean20.447,
p9525.694,p9953.203,max407.921ms at2560×1440/view6/LODstart5/fourlevels/
clouddiv3/fiveCSM. Actual adapter is RTX4090/Vulkan. Selected GPU mean17.896ms
(cloud10.609,terrain5.157,shadow1.723). Native summary5734 samples and the
measured-ready CSV selection5733 rows differ by one boundary sample; they are
reported separately. Process working/private peaks1,275,518,976/3,282,157,568B,
exit0. This is a current failing result, not a controlled cross-driver comparison.

The25-column diagnostic validates nested upload timing in every row. In its
5733 measured-ready rows, stream mean/p95/max2.6814/4.4130/195.4133ms,with
ten>50ms; resident-upload mean/p95/max1.9190/2.8752/160.9863ms,with seven>50ms.
The operation inside that phase remains unassigned. A bounded, opt-in finer
allocation/staging/error-scope diagnostic is now source-only and uncompiled.
[Current run](D:/voxel-horizon-review-20261021/publication-cache-driver617-physics-120/manifest.json),
[phase receipt](D:/voxel-horizon-review-20261021/publication-cache-driver617-physics-120/resident-upload-review.json).

A separate source audit confirmed resident buffers enter active/inactive draw
caches before their asynchronous GPU error-scope result is ready. Later errors
are unkeyed and cannot identify or retry that publication. Existing interest,
dirty-halo,desired-LOD and budget checks occur before upload; there is no second
publication check for that future. A keyed pending-upload repair is being
prepared, not yet adopted or verified. The successful native cases did not
exercise allocation failure and do not close this gap. See
[GPU publication audit](C:/dev/voxel_v2_backup/reviews/horizon-continuity/lighting-audit/GPU_PUBLICATION_AUDIT.md).

The keyed pending-GPU protocol has since been applied to live source. It retains
CPU retry leases and old draw cover until scoped GPU validation succeeds, then
rechecks streamer/cache epochs, upload token, local dirty halo, interest, LOD and
current aggregate GPU/supplemental budgets before active or inactive publication.
It polls nonblocking without requiring capture/profiling and uses normal queue
ordering. Root's **11 actual RTX4090/Vulkan617.42 contracts passed**: six pending
protocol cases (including actual7168>4096 buffer validation failure and normal
nonblocking success), four retained-column/origin/dirty-LOD cases and one invariant
LOD publication case. This is source/device-protocol acceptance, not motion or
frame-time acceptance.

The complete protocol-only game built and all composed WGSL validated. Frozen
EXE SHA256 `013FD81F33F343CD8687BE0CF6513737C3BCE264EA6EC9AB0742E3F137CBB71D`;
[237-input source](D:/voxel-horizon-review-20261021/pending-gpu-source-013FD81F.zip)
SHA256 `40AB54872C40729A69BCEAC04ABD6FD1EF989CCA52DB9D6445209477B5B2BFB1`.
All archived inputs match the pre-build manifest. Runtime lighting/continuity
verification of this executable is still pending. Later live caster reuse and
habitat/dungeon work are outside that immutable snapshot and its test evidence.

The separately requested exact caster-reuse change is integrated in live source
under its owner's **static-only, no tests/game** scope. It preserves draw revision
and compares exact indexed shader-consumed inputs before reusing shadow identity.
Its Cargo check was blocked by a concurrently half-wired unrelated habitats
module; no full compile or runtime pass is claimed. The frozen protocol executable
does not contain this caster change. See
[applied static packet](C:/dev/voxel_v2_backup/reviews/renderer-profiling-20261021/water-shadow-integration/applied.json).

## Remaining acceptance work

Publication progress, pressured motion and both world-switch stages have real
receipts on the `3DD4B8EF` and `E21109A7` builds. The adopted `23AFF9F6` build
has its own native timing and source-admission evidence. Full visual acceptance
is still incomplete:
the smooth proxy versus resolved voxel transition remains visible in sampled
approach frames. The 1–2px silhouette goal is not established. Ordinary gameplay
edits still commit collision/authority before asynchronous near geometry;
`EDIT_PUBLICATION_AUDIT.md` records that known failure rather than claiming
atomicity from the route fixture. Source-admission starvation observed in E211
is repaired in the measured scene; a universal latency bound under downstream
or memory saturation is not established.

Enabled column ownership disables vertex underlap and proxy depth bias, and
partial resident columns are held out of terrain draws. This removes arbitrary
parent/child overlap, but is not a completed matching boundary collar solution.
Beyond4.2km, aggregate-crown edit/skyline guarantees remain unverified. Natural
unmodified caves beyond resident cover remain heightfield approximations.
Those are material limitations, not proven user pass conditions.
# Latest acceptance checkpoint: adaptive geometry staging

Newer update: scoped horizon allocation/write checks and profiler-independent
device loss are now integrated live with guarded seven-file before/after receipt
at `reviews/horizon-continuity/horizon-gpu-validation/live-applied.json`.
One actual RTX4090/Vulkan scope/loss contract passed (including intentional
test-owned device destruction without timestamp profiling); seven resident
publication contracts passed, including controlled signal change during Ready
polling. This is queue-ordered validation, not a GPU-idle fence or driver reset.
The game/Naga build passed. Native immutable2A4CB world-switch passed both build
and upload switches, only final seed2738958702 published, exit0, no render errors.
Receipt `D:/voxel-horizon-review-20261021/horizon-gpu-health-world-switch`.
The original013 loss is still unassigned; this does not prove its cause fixed.

Adaptive516747 native128m reference comparisons measured small-ROI maximum
contour distances: ridge1.414px, shoreline1px, forest2px. These are fixed crops,
not whole silhouettes or a moving handoff guarantee. The corrected lower-hill
comparison still shows a smooth triangular proxy versus stepped voxel geometry.
That candidate remains unapplied rather than being declared a topology repair.

Source-only successor packets use existing exact-column meshing for bounded
visible steep features and installed-mesh dry boundary profiles. The proposed
collar draw bundle carries Common, Collared and original affected Uncollared
indices together; a changed profile/owner immediately selects the original
parent alternative, with the same shadow selection. Neither packet is compiled,
adopted or visually accepted yet. Current preparation is under
`reviews/horizon-continuity/collar-integration` and `quality-streaming`.

Supersedes older status statements below. Live static caster integration now
passes Cargo check; its runtime remains deliberately untested under its owner's
instruction. Habitats generator27 and dungeon generator28 source work is
preserved, outside the frozen horizon runtime comparison stages.

The pending-GPU protocol passed eleven RTX4090/Vulkan contracts and the game
build. However, native013 world-switch failed after surface acquisition began
failing; the device-lost callback was not yet recording its reason. Its failed
run is retained. Diagnostic379 records those errors and passed one repeat;
this is nonreproduction, not an assigned cause or repair of the original failure.

Lighting owner subsequently captured five native A53 cases, including a real
enclosed generated cave and corrected gray-hillside reference. Camera CLI uses
player feet, not eye: the hill eye is53.62m. The earlier upper reference patch
did not include the lower gray hillside. Correct replacement is
`2530,-4150,128,2`; it confirms real pale Stone mixed with Basalt. Remaining
shape and near/far appearance differences must not be interpreted as uniformly
wrong material color. Full receipts are in
`reviews/horizon-continuity/lighting-audit/terrain-texture-cave/native-review.md`.

Root's adaptive refinement is a staged candidate only: original ring edges and
transition cells retained, bounded canonical midpoint scouting, actual selected
samples from the correct ring policy, no overlapping parent/child opaque draw.
Cache identity now includes canonical height/focal classes. Unrepresentable f32
midpoints retain the original coarse cell. Two edge/water contracts and one
midpoint contract passed. The full A/B/A fresh/retained source-sample equality
exercise passed in37.73s,222705 samples per centre. No native quality or cost
claim follows from these checks. Candidate hashes and238 frozen build inputs
are under `reviews/horizon-continuity/adaptive-refinement/guard.json` and
`target/horizon-continuity/adaptive-prebuild.json`.

Overall acceptance is OPEN. The latest current-driver1440p physics p95 still
exceeds16.67ms. Ordinary immediate collision/visual edit atomicity, natural
far volumetric topology, resident/proxy collar quality, and aggregate distant
forest guarantees remain unresolved.

