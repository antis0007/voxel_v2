# Forest rendering repair — 21 October 2026

The revised implementation brief makes Prompt K the immediate prerequisite to
new dungeon content. The original forest save/camera was unavailable; these are
generated hills fixtures, not reproductions of the absent forest screenshot.
Evidence is in `reviews/forest-repair-20261021/`, with frozen executables, hashes,
arguments, logs, captures and CSV samples. This forest pass did not add or run
unit tests or hosted CI; local builds validate composed WGSL as usual.

## Implemented and demonstrated

The starting dirty checkout built, but its horizon mesh reached 201,692,024 bytes
against a 167,772,160-byte publication limit. The worker discarded it and retried
every two seconds. No world frame was rendered: the application remained on the
distant-terrain loading screen. Its reported 2,315 FPS was therefore invalid as a
game benchmark; no valid before image or world-performance baseline exists.

`src/render/horizon_vegetation.rs` now merges coplanar crown faces while keeping
the same occupied cells, winding, normals, tree placements and random stream.
Per-cell random vertex tint is replaced by the existing per-lobe tint and leaf
surface shader. The actual silhouette remains block shaped. Vegetation admission
uses vertex **and index** bytes against the shared mesh limit, reserves 8 MiB for
edits, and rolls back a whole tree rather than retaining a truncated crown.

In the geometry fixture the mesh is 119,283,360 bytes (40.9% smaller), with zero
trees omitted. Packed GPU pages total 113,961,064 bytes. Cold construction took
2,203 ms; a subsequent build took 563 ms. This fixes the reproduced mesh-budget
rejection without increasing the limit or removing terrain coverage.

`src/main.rs` now applies fixed camera time and fixture setup in capture-free
benchmarks as well as capture runs. It excludes horizon loading from benchmark
warmup/samples and aborts with an explicit failure after a bounded readiness wait.
The runner rejects missing summaries, zero world draws and missing captures.

The existing debug workflow gains `projection`, `ao`, `sky-access` and
`mesh-kind`. AO display and production shading share the decoder for resident AO,
procedural backdrop tags and exact edited-proxy AO. This reuses the existing
production decoding; it is not claimed as a newly discovered production fault.

## Observed GPU cost

The first valid geometry capture used an RTX 4090 / Vulkan, optimized
`release-fast`, 1280×720, seed 2738958700, partly cloudy noon, six resident chunks,
camera `44,35,7,-1,0.14`, uncapped presentation. Cloud history/adaptive quality
were disabled by the existing benchmark path. The requested cloud divisor 3
became 2 because of the renderer's 360-row minimum.

| Run | Samples | CPU frame p50 / p95 / p99 ms | GPU world / clouds / selected sum ms |
| --- | ---: | --- | --- |
| `geometry/canopy` | 1,229 | 18.658 / 21.788 / 22.630 | 1.371 / 16.970 / 18.453 |
| `final/perf-1280` (lower camera, canonical source, no captures) | 1,573 | 15.206 / 16.055 / 16.276 | 2.506 / 11.416 / 14.058 |
| `final/perf-2560` (same camera/source/divisor, no captures) | 425 | 63.475 / 88.974 / 104.634 | 19.969 / 26.075 / 48.437 |

The geometry row is a capture run, not a controlled before/after speedup claim. Surface
acquisition averaged 17.849 ms; that wait overlaps GPU execution. It must not be
added to the GPU intervals or described as shader work. The selected-pass sum
is not a whole-frame GPU fence. Shadow cost was zero during these warmed fixed
views; that does not establish that shadows were disabled.

Albedo, AO and cached sky-access images exist in `diagnostic/`. The lower camera
is `44,35,1,-1,-0.12`; its timings are not comparable to the elevated camera.
Neither these views nor source inspection establish the original screenshot's
canopy inconsistency or the cause of its ground bands. No gamma/ambient/bias
adjustment was made to guess at those symptoms.

The final pair keeps seed, camera, time, six-chunk visibility, simulation policy
and cloud divisor 2 fixed. Cloud targets change from 640×360 to 1280×720.
Both runs contain zero GPU configuration crossovers and zero late stream work.
The full-resolution run has substantial unexplained world-pass variance: means
over consecutive ~100-frame windows were 29.2, 4.2, 17.6 and 27.1 ms, while cloud
cost stayed near 26 ms. Treat this as an exploratory cost comparison, not a clean
scaling law or proof of a particular material bottleneck. GPU process attribution
was unavailable from the local driver query. The short 425-frame run provides
only a few tail samples; its p99 is descriptive, not a stable tail estimate.

`final/motion-preview/traversal.mp4` is a completed 44-second sequence containing
88 real renderer readbacks. The run uses the explicit preview horizon source,
physics disabled, walking offsets, a sideways pan and vertical movement. Sampled
walking, panning and elevated frames were inspected. It demonstrates rendering
through those camera changes; it does not close the active-physics or ordinary
gameplay gates. Capture stalls (maximum CPU frame 543.6 ms) exclude it from steady
performance comparisons. Its immutable manifest states these options.

## Remaining acceptance gates

- The active-physics motion run (`diagnostic/motion`) failed readiness after
  repeated relevant edit invalidations. It produced no playable motion capture
  or valid world timing. Fluid simulation continued; it was not disabled to
  manufacture a passing result. The logs identify horizon build starvation as
  an integration blocker requiring attention.
- Shared renderer/profiler/horizon work changed during this pass. The final
  source compiled after a mid-edit build failure; preserve the separate owner's
  changes. Frozen build identities distinguish earlier successful fixtures from
  later integration attempts. The final binary's canonical horizon built in
  97,635 ms cold and 15,474 ms on its follow-up; it published 111,411,932 GPU
  bytes with zero omitted trees. This startup cost is separate from frame cost.
  The 720p timing run completed with 1,573 GPU samples and no configuration
  crossovers. Its lower camera differs from the earlier geometry capture.
- The 2560×1440 / 16.67 ms p95 budget in the existing visual-direction document
  failed in this run (88.974 ms). The lower-camera 720p run meets that numerical
  threshold with physics disabled; the elevated 720p capture exceeds it.
  No available headroom for more rendering/content load is established.
- Original forest matching, ordinary gameplay, active-physics motion/edit
  publication, isolated attribution of the variable world cost and full production
  preset acceptance remain open. New dungeon/voxel-density work stays deferred.

## Reproduction

`tools/Review-ForestRepair.ps1` uses generated worlds and does not load a player
save. Choose a new label on every run. Use the frozen executable whose hash is
recorded with the desired evidence; current `target` binaries may belong to a
different shared session.

```powershell
./tools/Review-ForestRepair.ps1 -Executable reviews/forest-repair-20261021/final/forest.exe -Label review-new -Capture
./tools/Review-ForestRepair.ps1 -Executable reviews/forest-repair-20261021/final/forest.exe -Label review-physics-new -Case motion -Physics -Video -Seconds 44
```

Video uses actual renderer readbacks at two samples per second and preserves
missing-frame timestamp gaps. It is a diagnostic sequence, not a claim of smooth
gameplay. Timings must come from separate runs without captures or compilation.

## Follow-up: physics starvation and 1440p performance

Work remains on Prompt K; dungeon expansion is deferred. Renderer chats now
coordinate horizon ownership and native GPU windows. The horizon owner is
integrating retained base meshes, strict GPU coverage, actual changed-cell
visibility and indexed water geometry. Do not overwrite that work.

The review harness now checks the actual render resolution, isolates generated
region files under each evidence directory, and supports genuine cloud/horizon
work removal for attribution. Monitor-sized benchmarks use borderless fullscreen:
the first two `perf-window` runs requested 1440p but actually rendered 1346 rows.
Those runs are exploratory and do not establish 1440p acceptance. Fixed-pose
`camera.png` now waits for the measurement phase and a ready horizon.

Cloud candidates preserve resolution, march budgets and the medium definition.
Conservative lobe bounds avoid rotations/angular work for cells that cannot
contribute; exact blend endpoints skip unused sheet calculations. A pure-cumulus
density floor rejects fields which remain below the existing zero-density guard.
The separate nested quadrature change is an approximation between the existing
one- and two-tap integration rules. Matching endpoints alone does not prove
equivalence. It requires visual and motion review before acceptance.

All rows below rendered **2560x1440**, cloud divisor **3**, with no readback,
physics off, preview horizon source and camera `44,35,1,-1,-.12`. They are
exploratory sequential candidates in a shared checkout, not release acceptance.

| `perf-window` candidate | CPU p50 / p95 / p99 ms | GPU clouds / world ms |
| --- | --- | --- |
| `tight-1440` | 18.464 / 21.052 / 21.995 | 14.767 / 3.327 |
| `nested-1440` | 15.306 / 18.459 / 23.341 | 11.947 / 3.068 |
| `endpoint-1440` | 15.010 / 18.656 / 19.804 | 11.529 / 3.450 |
| `density-floor-1440` | 14.017 / 17.682 / 18.685 | 10.729 / 2.805 |

Those candidates missed the 16.67 ms p95 budget. Each has zero GPU configuration
crossovers; query scope still excludes resolve/present, egui and upload copies.
Partly cloudy and storm fixed-pose images in `visual-window` retain the cloud
structure at the inspected camera. Sky-region mean absolute differences are
below 0.17/255 per channel, but frame phases and build identities differ; this
does not establish pixel identity or motion stability. Terrain changed between
shared builds, so these images are a cloud comparison, not a geometry comparison.

`physics-window/canonical-1440` published the first canonical cover in 10,270 ms
with active simulation, zero initial stale rejections and 80,776,996 GPU bytes.
The run then completed 857 measured frames. First-cover starvation is cleared
in this candidate, but subsequent edit overlays retried without pages; retained
old cover alone does not close edit-correctness or follow-up publication gates.
The horizon owner's later repair addresses this separate overlay budget issue.

Partial-water render snapshots now share immutable chunk maps, copying only
mutated chunks. Mesh workers visit partial-water maps intersecting the 34-cube
halo rather than scanning all partial cells for every mesh. This changes derived
runtime data only, with no save-format change. In the next canonical physics
run (`physics-window/shared-water-1440`, frozen SHA256 `8E58F084...872E0E`), mean
snapshot CPU time fell from 7.217 to 0.113 ms. Its p95 still failed at 64.669 ms
amid ongoing streaming, geometry and shadow work. Fluid trajectories and shared
source differ, so this is not a controlled whole-frame speedup claim.

Current source also avoids copy-on-write for unchanged partial-water inserts or
absent removals. The angular-harmonic experiment showed no measurable gain and
was reverted. A cross-layer opacity-cutoff experiment suffered a device-loss
startup failure, was stopped by exact executable/PID verification and was also
reverted. It contributes no accepted timing result. Rejected binary copies and
private compiler outputs were removed with SHA256 cleanup receipts; logs remain.

### Controlled static comparison

The `combined-1440` run was rejected: privileged one-second process logging
observed another project renderer alongside it. Its `acceptance.json` explicitly
sets performance acceptance false. The shared timing summarizer now refuses
such rejected runs.

The subsequent reserved comparison used frozen source, zero readbacks, the same
1440p/divisor-3 camera and preview-source fixture, physics off, eight-second
warmup and twenty-second measurement. Each audit observed only its own project
renderer. The horizon owner confirmed no compiler activity during measurement.
GPU clocks remained in P0 at approximately 2895 MHz with 10701 MHz memory.

| Accepted static run | Frames | CPU p50 / p95 / p99 ms | GPU clouds / world ms |
| --- | ---: | --- | --- |
| `clean-base-1440` (`E4C98E00...`) | 1,368 | 13.551 / 17.252 / 17.872 | 11.065 / 2.899 |
| `clean-peak-1440` (`194285CB...`) | 1,453 | 13.226 / 16.603 / 17.335 | 10.551 / 2.547 |

The only source change between these frozen builds is an early thermal peak
bound. Monotone blends of the six lobe maxima peak below 1.102; cells unable to
beat the current envelope can be skipped before profile hashing. The candidate
**narrowly meets the static p95 target**, with only 0.067 ms margin. This short
fixture establishes no content headroom or active-physics/motion acceptance.
`total_ms` measures main-thread work through rendering and excludes subsequent
profiling/telemetry and event-loop idle. The old static CSV has no wall-clock
timestamps, so its p95 cannot establish render-attempt or display cadence.
Both runs have zero GPU configuration crossovers and zero late streaming work.
Their independent process/GPU logs, acceptance decisions and
`perf-window/clean-comparison.json` retain the exact scope.

The integrated horizon now uses actual modified bounds, captured omission scopes,
a shared per-column GPU coverage mask and retained resident columns. The
`reviews/horizon-continuity/retained-target-route` recording completed 280 real
one-second captures, but frame 036 still displays destination loading during
travel. Frames 035 and 039 also expose rectangular water shading boundaries.
That run is a 720p, physics-off capture review, not performance acceptance.

The new horizon CPU bucket covers both update calls. Its CSV contains only the
final 1,411 stationary measurement frames (p95 0.0526 ms, maximum 0.1249 ms),
not the preceding route. Travel hitch logs include a 393.87 ms frame dominated
by render CPU time; its horizon bucket is 0.04 ms. These observations neither
reproduce nor attribute the earlier 2,907.8 ms stall. Main-thread procedural
sampling remains a source-level latency concern, documented separately in
`HORIZON_VALIDATION_AUDIT.md`.

Follow-up publication, continuous movement, water-boundary parity and a longer
combined 1440p active-physics run remain open. Available content budget is zero
until those gates pass; the short static pass has only 0.067 ms p95 margin.
New dungeon content remains deferred.

The later `structural-components-albedo` run failed during shader initialization
and produced no image. Its frozen world shader referenced `debug_mode` before
the local declaration; the diagnostic source is corrected, but that executable
is invalid. Shared build-script validation had been reused despite the copied
shader differing. The horizon owner is separating the review package identity
and forcing fresh validation before retrying. A successful compiler exit from
that failed candidate does not establish renderer readiness.

The corrected executable produced `structural-components-v2-*` images, but
the debug paths returned before parent coverage discard. Their owner/depth/normal
views could therefore show excluded geometry and cannot isolate the beauty
view's water strip. The lighting owner corrected diagnostic ordering in source;
new component captures are required. No depth/normal-based cause is established.

## Structural handoff canary

`reviews/horizon-continuity/structural-shore-handoff` completed 90 seconds on the
frozen validated Gen25 review executable, with 90 real PNGs and a traversal video.
Frame 036 was inspected and renders the world. The full-route recorder reports
17,303 attempts, 7,938 initial loading attempts, zero ready-to-loading transitions,
zero failed renders and no CSV I/O errors. Both structural edit revisions
published; three completed CPU results were rejected without starving progress.
Horizon CPU work peaked at 5.6026 ms, at route second 32.769552.

This closes the repeated second-36 loading failure for this short canary. It is
720p, physics off and uses an experimental fourth shadow cascade extended to
8 km; it is not final-preset performance evidence, a full 280-second route or
active-physics acceptance. Water strips and the invalid diagnostic ordering
remain separate gates. New content still has zero available budget pending the
coherent gameplay build and final 1440p measurements.

The later identity candidate (`40E40B1E...19925AD0`) preserves all four near
shadow cascades and adds a fifth distant cascade. Coarse terrain now appends the
fine water-only surface instead of rounding water up to coarse cell boundaries.
The traced old mismatch was 14.0 m versus the 12.865 m normal water surface.
`identity-water-beauty/camera.png` was inspected: the bright rectangular strips
are gone at that low camera. This is a fixed-view visual result; sustained
active-physics performance and full-route review remain pending.

## Active-physics identity run: failed readiness

`physics-window/identity-1440` used that frozen Gen25 identity executable,
canonical terrain, 2560x1440, six view chunks, partly cloudy noon, divisor 3,
physics enabled, twelve-second warmup, ninety-second requested measurement,
full-frame diagnostics and GPU telemetry. Capture readback was disabled and
normal presentation preferences were requested.

It ran from 17:35:14 to 17:38:43 UTC but never completed a stable measurement.
The initial cover and several later covers published, then readiness repeatedly
failed: 37 returns to loading, 24,474 loading attempts and only 2,411 ready
attempts out of 26,885. The longest ready interval was 12.43 seconds. At the end,
all 443 visible GPU meshes were present and all mesh queues were empty, while
simulation was paused behind loading with 379,528 updates pending.

The exact owned process was closed gracefully; its exit code was zero, but the
runner correctly rejected the missing completed benchmark. `acceptance.json`
explicitly rejects performance evidence. There is no valid ninety-second FPS
or GPU-stage result. The 135-second process audit covered the initial failure,
but not the extended run's final interval, so whole-run exclusivity is unproved.

Across the full diagnostic recording, horizon CPU work peaked at 11.9456 ms and
gameplay work at 6.1507 ms. No mesh-budget-exceeded message occurred. The failure
is follow-up occlusion/coverage readiness, distinct from first-cover publication
and the separate resident stale-result risk in `EDIT_PUBLICATION_AUDIT.md`.
Peak process working/private memory was 1,134,747,648 / 2,407,211,008 bytes;
last GPU resident geometry was 489.61 MiB. Available new-content budget remains
zero until active-physics readiness and sustained performance pass.

## Published-column repair and clean 1440p physics result

Desired vertical chunk membership previously revoked coverage before its new
meshes were ready. The renderer now retains the published membership until a
complete replacement is available; drawing, coverage, pinning and eviction guards
use that published ownership. `physics-published-columns` passed a forty-second
physics canary without returning to loading.

`physics-window/current-1440-clean` then completed the requested ninety-second
measurement: 6,016 frames, zero ready-to-loading transitions, zero failed renders
and zero GPU configuration crossovers. It used the actual root executable
`FC2F2B48...71ACF9`, explicit generator 25, canonical horizon, physics on,
2560x1440, High/divisor 3, six view chunks, LOD start 5, all five shadow cascades,
uncapped presentation and no capture readback. Tree provenance was enabled.

Main-thread work p50/p95/p99 was **14.602 / 17.651 / 19.974 ms**, maximum
42.781 ms. Adjacent measured render attempts had p95 **17.862 ms**, p99
20.158 ms and maximum 43.706 ms; this includes recorder/event-loop overhead and
does not measure display cadence. The 16.67 ms target **failed**. Selected GPU
passes averaged 13.965 ms, including clouds 10.678, terrain 2.180 and shadows
0.862 ms; these are asynchronous stage timings, not a complete GPU frame.
Horizon work peaked at 29.9598 ms across the full diagnostic route, so the repair
does not establish a bounded main-thread horizon latency.

The process audit covers the measured interval and observed no other project
renderer; its first sample missed approximately three seconds of initial
startup. Peak working/private memory was 1,113,370,624 / 2,516,086,784 bytes.
The process exited normally. Generated worlds were recoverably archived with
verified per-entry hashes; logs and timing evidence remain in the case folder.

Earlier helpers set absent environment flags to empty strings. Native
presence-only checks consequently enabled tracing and reference-only tree
removal in earlier cases. The helper now actually removes absent flags. This
clean case has no horizon trace and preserves tree provenance; older cases are
not equivalent feature-state performance comparisons.

Two further cloud skips are source-ready and independently algebra-reviewed:
pure-cumulus density is zero at exact shaped-height endpoints, and an envelope
below the broad-mass smooth-maximum dominance floor cannot affect density.
They retain noise, filters, ray-march samples, resolution and weather features.
The source hash is `6B664A5A...BA249`; compilation, visual review and performance
measurement of this candidate remain pending the coordinated build. No speedup
or available dungeon budget is claimed.

## Coherent candidate: readiness passes, performance still fails

`physics-window/faithful-1440-clean` completed another ninety-second measurement
on `faithful.exe` (`A084077E...F995A1`), with the same explicit Gen25/1440p/High
settings and active physics. This coherent build includes both exact cloud skips,
water ownership weighting, canonical tree placements through 4.2 km and the
forest source cache. All 133 process-audit samples bracket the native process;
no competing project renderer was observed.

There were zero returns to loading, failed renders, GPU configuration crossovers,
omitted trees or mesh-budget failures. Cold vegetation used 91,340,180 bytes;
source-cache counters were 1,624 hits / 403,890 misses / 403,890 entries. Peak
process working/private memory was 1,221,275,648 / 2,611,589,120 bytes.

The target nevertheless failed: 5,901 measured frames had work p50/p95/p99 of
**14.900 / 18.989 / 24.639 ms**, maximum 39.340 ms. Adjacent measured attempts
had p95 **19.196 ms**, p99 24.891 ms and maximum 39.653 ms. Average horizon CPU
work rose from 0.570 to **3.400 ms**; full-route horizon maximum was 29.509 ms.
Selected GPU passes averaged 14.257 ms, including clouds 10.909, terrain 2.233
and shadows 0.863 ms. The combined build demonstrates no cloud speedup and does
not isolate either cloud skip's effect from its other geometry changes.

Normal-presentation validation was deferred because the uncapped target failed.
Evidence is valid for this scope, with `budget_passed=false`. The native and
compiler slots were released to the horizon owner for the measured CPU-cost
regression; no new content budget is available. The completed generated world
was archived with verified entry hashes, retaining all timing/process evidence.

The next targeted repair caches the **complete** `BuildTarget::matches` result,
including its structural voxel scan. Previously only the later visibility check
was memoized, so the same immutable desired target could trigger that scan every
idle frame within the 500 ms target-refresh interval. The new third memo slot
retains the same snapshot/configuration, eye, radius and coverage dependencies;
center mismatch is checked first, and the existing omission-install reset clears
all slots. Static peer review accepted these keys and the borrow lifetime.

This exact candidate (`horizon.rs` SHA `8DD41626...7492EA`) compiled locally in
21.02 seconds. No formal tests were run by this chat, as requested. The coherent
build owner retains final integration and the next native slot. Performance and
additional retained-snapshot memory are unmeasured; first-scan latency remains
unbounded. The root source is frozen to that owner after this bounded change.

The no-readback helper also clears the new opt-in video path and FPS variables.
Twelve obsolete task-generated review executables were recoverably ZIP-archived
with verified content hashes to recover 131 MiB. The older baseline/combined
copies were subsequently archived the same way for diagnostic headroom; peak
remains unpacked. Source, user saves, logs and captures were preserved.

## Whole-match memo measured: CPU repair passes, total target fails

`physics-window/memo-1440-clean` completed ninety seconds on coherent `memo.exe`
(`7D20C71E...88DFFB`), retaining Gen25, High 2560x1440, physics, canonical horizon,
all five cascades and no readback. Its 171 process samples bracket the complete
native process: no other project renderer or Cargo/rustc/link was observed, and
the resident rust-analyzer's CPU counter did not advance. Earlier audits did not
record global compiler activity, so they establish renderer isolation only.

Average horizon CPU work fell from **3.400 to 0.457 ms**. The run had zero returns
to loading, failed renders or GPU configuration crossovers. Work p50/p95/p99 was
**15.011 / 18.012 / 20.068 ms**, maximum 45.370 ms over 5,852 measured frames.
Adjacent measured attempts had p95 **18.239 ms**, p99 20.376 ms, maximum
45.552 ms. The 16.67 ms target still fails; no normal-presentation run followed.

GPU selected passes averaged 14.393 ms, including clouds **11.046 ms**, terrain
2.245 ms and shadows 0.852 ms. Full-route horizon CPU maximum remains 29.776 ms,
so first-check latency is still not bounded. Peak working/private memory was
1,135,906,816 / 2,514,767,872 bytes. The process exited normally and its generated
world was archived with verified hashes. The next requested step is an isolated
cloud comparison; available content budget remains zero.

## Controlled cloud comparison: earlier skips regress GPU cost

Four thirty-second diagnostics used the same coherent Gen25 scene, High native
2560x1440, canonical horizon, static camera, twelve-second warmup, physics off
and no capture readback. All rendered exactly 367 meshes / 224 draws /
1,693,685 terrain triangles. Each complete process interval was bracketed by the
renderer/compiler audit, with zero competing project renderers or compiler
observations and zero rust-analyzer CPU advance. There were no loading returns,
failed renders or GPU configuration crossovers. All measured one-Hz GPU samples
were P0, with graphics clocks 2895–2910 MHz; the original-code control stayed
at 2895 MHz. These are diagnostics, not active-physics or display acceptance.

| Cloud variant | Cloud mean ms | Cloud p95 ms | Selected GPU mean ms | Work p95 ms |
| --- | ---: | ---: | ---: | ---: |
| Current exact endpoint + mass-floor skips | 11.361 | 12.665 | 14.412 | 17.539 |
| Uniform cirrus attenuation moved to flat vertex output | 11.177 | 12.429 | 14.248 | 17.318 |
| Exact compact-support bounds + central-cell-first traversal | 11.054 | 12.592 | 14.125 | 17.519 |
| Original code before endpoint + mass-floor skips | **10.747** | **12.314** | **13.873** | **17.314** |

Table quantiles use nearest rank; benchmark summary interpolation differs by a
few microseconds. Selected GPU passes exclude some frame work. GPU samples are
deduplicated by origin frame and restricted to the measured render interval,
excluding the four delayed warmup samples in each case.

The prior exact-skip changes therefore regress this controlled fixture despite
preserving the density algebra. Restoring the original morphology (`508D7300...`
in `cloud-candidates/before-exact-skips.wgsl`) is the recommended first rollback.
The vertex hoist is a small isolated gain; the thermal candidate does not improve
the p95 target meaningfully. No further speculative candidates were started.

The staged package has a fresh unique name, full current source inventory and
fresh validation of all sixteen composed WGSL programs. Variant source audits
preserve all geometry and other shaders. Its inactive package cache was cleaned
only after a checked dry-run and saved hash inventory; staged executables remain
recoverable in verified archives. The live shaders have **not** been changed by
these diagnostics. Results are in `cloud-ab/comparison.json`; root integration
and compiler/GPU ownership were explicitly released to the coherent build owner.
The 16.67 ms target remains failed and new-content budget remains zero.

## Coherent rollback and cold-water repair: latest acceptance remains failed

`physics-window/continuity-1440-clean` completed ninety seconds on `continuity.exe`
(`D1319D15...185EBC`), with the whole-match memo, restored original morphology
and cold saved-water repair. Settings remained Gen25, High native 2560x1440,
canonical horizon, physics on, twelve-second warmup, uncapped, no readback.
There were zero returns to loading, failed renders, GPU configuration crossovers,
omitted-tree reports or mesh-budget failures.

The 6,015 measured frames had work p50/p95/p99 **14.597 / 18.233 / 21.876 ms**,
maximum 34.992 ms. Selected GPU passes averaged 13.930 ms, including clouds
**10.621 ms**, terrain 2.203 and shadows 0.862 ms. Average horizon CPU work rose
to **2.724 ms**, with measured median 3.968 and p95 6.097 ms; full-route maximum
was 30.512 ms. Peak working/private memory was 1,232,130,048 / 2,610,020,352 bytes.
The process exited normally; its generated world was recoverably archived.

The 117 process samples bracket the full process and observed no competing
project renderer or compiler. Rust-analyzer used 0.359375 CPU seconds during
startup, last changing at 19:44:25.321 UTC. Even the earliest possible measurement
start was 19:44:32.636 UTC, and its counter stayed flat throughout measurement.
This is measured-interval isolation, not whole-process analyzer inactivity.

Adjacent measured attempts had p95 **18.447 ms**, p99 22.215 ms and maximum
**235.849 ms**. That largest gap spans route seconds 88.004–88.240, with adjacent
work timers only 15.059 / 15.466 ms. Most of this gap is outside the work timers;
its cause is unassigned, and it must not be reported as a measured horizon stall.
Normal-presentation validation was skipped because the target failed.

### Retained coverage does not close follow-up build starvation

The previous memo run published six times, last at route seconds 17–18, followed
by fourteen completed-CPU stale rejections and no further publication. Continuity
published eight times, last at **32–33 seconds** (request generation 47), with
twelve completed-CPU stale rejections overall and no new publication during the
final **approximately 76 seconds**. Workers finish, but useful follow-up publication
still starves under continuous physics. Zero loading transitions proves retained
coverage stability, not full starvation repair.

The new CPU cost begins after that last continuity publication. Before thirty
seconds, horizon median is about 0.06 ms; after forty seconds it is about 4 ms.
Within conservative interior intervals when a worker is still building, median
horizon work is 3.982 ms versus 0.071 ms in the memo run. The worker-busy path
returns before whole-target `matches`, so its memo cannot explain this cost by
itself. Published-scope checks, snapshot work and coverage generation still run
there. Phase/rejection-reason instrumentation is required before attribution or
changing validity rules. Event times are bracketed by one-Hz stream logs, and
simulation evolves nondeterministically; this is correlation, not proof that
the cold-water repair caused the regression.

The comparison and exact adjacent-frame pair are retained in
`physics-window/horizon-churn-comparison.json`, `horizon-publication-correlation.json`
and each case's `horizon-churn-review.json`. Active-physics starvation and 1440p
performance remain open. New-content budget remains zero; dungeon work stays
deferred.

### Coherent 1440p follow-up: CPU repair, performance still open

`physics-window/coherent-1440-clean` used executable SHA-256
`9CCF832698F9825A6EDF01DD730F4AE60EB50A74D1DA5CBF67D9F3E82B60774C`.
The 5,882 measured frames had work p50/p95/p99 **14.678 / 18.155 / 18.837 ms**.
Average horizon CPU work fell to **0.269 ms**; selected GPU passes averaged
14.213 ms, including clouds 10.782 ms. Zero loading returns were observed, but
the final publication was at route seconds 23–24, followed by thirteen completed
CPU stale rejections. Useful follow-up publication still starves.

This run is diagnostic only: Rust-analyzer accumulated 0.453125 CPU seconds
during the measured interval. It also missed the 16.67 ms p95 target. Adjacent
render attempts had p95 18.364 ms and maximum 426.556 ms; the largest gap is mostly
outside the recorded CPU work timers, and its cause is unassigned. No display
cadence acceptance is implied. The generated-world archive was hash-verified
and moved to `D:\voxel-horizon-review-20261021\forest-performance-archives`;
the case's archive receipt records the recoverable location.

The later rejection diagnostic identifies real changing free-water volumes
outside resident ownership, rather than identity churn alone. The next bounded
implementation is supplemental source-only resident coverage, subject to the
contracts in `HORIZON_DYNAMIC_PROXY_AUDIT.md`. New-content budget remains zero.
