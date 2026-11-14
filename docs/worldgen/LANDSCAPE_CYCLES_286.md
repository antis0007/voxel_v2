# Landscape cycles 286

The subsequent [orbital surface pass](ORBITAL_SURFACE_286.md) records the current
compiled candidate, native rendering comparisons and erosion-cache optimization.

Generator 286 is the new Planet default and extends the directional watershed work in 284. Existing generator
numbers keep their terrain rules. The loading-globe presentation changes apply
to all worlds; terrain, climate and route changes are gated to 286.

## Implementation and sources

- **Globe continuity.** Relief-normal gain is 2.25 rather than 9. The history and
  finished survey share geographic orientation and a two-second dissolve.
  Camera tracking is angular-speed limited. Climate-dependent canopy colour uses
  the game's leaf and pine materials. Altitude alone no longer strips warm wet
  plateaus down to rock. Survey work is shared across at most three workers;
  all six faces still publish together at the original 192 samples per edge.
- **Climate and sea level.** A 24-band equal-area energy-balance calculation
  solves absorbed solar energy, linear outgoing radiation, meridional diffusion
  and temperature-dependent ice albedo. Its equations and decomposition were
  checked against [CLIMLAB's implementation](https://github.com/climlab/climlab/blob/main/climlab/model/ebm.py).
  This is an independent Rust implementation with reduced radiative boundary
  forcing, not CLIMLAB embedded in the game or a resolved carbon/ocean model.
  Historical sea level follows the persistent, transported excess land-ice
  inventory divided by ocean area, with finite accumulation and thaw time.
  Floating sea ice does not remove ocean volume. The reference inventory anchors
  the present epoch to the same sea datum used by gameplay. Elevation cools the
  historical surface using the gameplay lapse rate.
- **Erosion throughout history.** Ice persists between the 250,000-year surface
  steps. Each step applies river erosion, slope weathering, ice transport,
  abrasion and till deposition before the next stage inherits the bed. Changing
  water inventory updates the marine datum without adding rock. Glacial debris
  goes through the same dated sediment columns and conservation budget. The
  separate end-of-history glacial-carving pass is disabled for 286; its
  former-glacier field comes from the evolved history. `[erosion-epoch]` logs
  report actual removed rock and glacial erosion at every plate interval.
- **Woodland structure.** Occupancy, clearings and regeneration use continuous
  body-space fields, so chunk boundaries and cube-chart boundaries do not reset
  them. Temperature, moisture, slope and river proximity affect occupancy.
  The same disturbance field controls tree cohorts and forest-floor cover.
  Rock, water, scree and site exclusions remain authoritative.
- **Debris flows.** Saturation lowers slope stability, weak exposed strata fail,
  mobile material follows drainage, and declining mobility deposits it. Deposits
  participate in the next routing solve. This is a bounded CPU reduction of the
  process described in [Jain et al. 2024](https://www-sop.inria.fr/reves/Basilic/2024/JBC24/),
  not a port or reproduction of the paper's full GPU method.
- **Travel.** Surveyed settlements use bounded terrain-cost search, with a
  finer ground check on the selected edges. Failed edges can be avoided on a
  subsequent search. Accepted connections become narrow gravel paths and keep
  vegetation clear. Paths do not flatten cliffs, cross deep water or create
  bridges. Settlement history uses the actual route's existence for connectivity.
  This addresses readable travel inspired by Tectonic, Veloren and the useful
  geography of TerraFirmaCraft; it imports no content from those games.
- **Loading and visibility.** Workers share completed settlement plans instead
  of repeating the route search on each thread. Planet fog uses the weather's
  physical range rather than the near-chunk radius. Clear-weather mountains
  therefore remain visible beyond the voxel residency boundary. The review
  starts geology asynchronously, through the same loading display as ordinary
  world creation, and its walk starts after loading and warmup.
  A native run traced a 14.8-second startup pause to regional settlement discovery.
  Cold discovery now queues the same deterministic plans on one background
  worker with one pending request. Resident admission waits for those plans;
  existing settlements and their historical simulation remain enabled.

## Reference implementation decisions

[Geotransport](https://github.com/erosiv/geotransport) is the MIT-licensed
reference for the 2026 stochastic transport paper. Its CUDA path kernel was
inspected: streamline integration, attenuation and normalized flux estimates
are distinct from the erosion update. It is not a complete erosion application.
[Soillib](https://github.com/erosiv/soillib) contains the broader evolving CUDA
implementation under LGPL-3.0. Neither has been copied or linked into this build.
286 retains 284's existing deterministic directional approximation.

[Arenite](https://jango6324.github.io/arenite/) is the stronger reference for
volumetric sandstone arches and alcoves, but the public project page offers a
paper rather than a reusable full solver. No claim is made that this build
implements its stress/fabric-interlocking solver. Likewise, FastFlow is not
integrated: a GPU routing port needs measured benefit and deterministic boundary
handling before replacing the current CPU graph.

## Validation

Native build, images, route results and timings are recorded with the candidate
executable under `target/landscape-cycles-review`. A successful build alone does
not establish terrain quality, route playability or frame pacing.

Local generation diagnostics, 8 November 2026:

| Fresh seed | Surface steps | Invalid values | Largest sediment residual | Continental volume residual | Sea range relative to present |
| --- | ---: | ---: | ---: | ---: | ---: |
| 74 | 960 | 0 | 0.007599 m³ | 0.000000003 km³ | −0.7123 to 0 m |
| 731 | 960 | 0 | 0.018814 m³ | 0.000000005 km³ | −1.5736 to 0 m |

Every one of the 48 plate intervals reports positive bedrock removal and glacial
abrasion. Both histories contain 49 saved states across 240 million years.
The surface solver uses reduced, scaled transport rates; these figures establish
terrain evolution and numerical conservation, not validation against Earth data.

The seed-74 bathymetry comparison kept land coverage at 35.2234679% before and
after ocean-floor shaping: zero drowned or emerged nodes, zero coastline shift
across 22,570 shared crossings, and zero land flips in 37,633 resolved samples.
The offshore bed changed in 1,910 of those samples.

The 256 m valley window at chart (323180, 2210), sampled every 2 m, had no false
rock classifications by the existing slope diagnostic. Physical terrain hashes
matched at 1, 0.5 and 0.25 m voxel scale. The sampled change across the 32 m
footprint threshold was at most 0.000136 m. This is an input-continuity check;
it does not establish that the renderer has no missing chunks or visible seams.

A wider relief diagnostic found a real 1.409214 m height discontinuity at the
32 m detail footprint near chart (324884, 1760). Filtering out the narrow river
bed also discarded its water head, enabling dry-slope erosion in the coarser
sample. Generator 286 now retains the physical channel head independently of
the filtered bed geometry. Repeating that diagnostic reduced the maximum jump
to 0.000083924 m; all three voxel-scale hashes still agree. This corrects the
terrain input, while native captures remain necessary to assess mesh coverage.

The same native canyon walk reached all ten waypoints in 24.48 simulation
seconds without flight. A second route reached all 34 waypoints in 88.44 seconds,
climbing from roughly 29 m to 130 m through existing terrain. Neither route
edits the terrain. The final stream snapshots had zero missing visible chunks.
These are route-specific observations, not a proof that every chunk boundary is
seamless. During movement, the one-second stream samples reached 103 missing
visible detail meshes on the valley route and 105 on the pass route. The
retained coarse terrain can cover these, but zero missing meshes at the endpoint
must not be presented as uninterrupted detail residency throughout the walk.
Both runs used 1280 by 720, a 12-chunk view and RTX 4090/Vulkan; cellular
physics was disabled by the existing benchmark configuration, while player
collision and ordinary movement remained active.

The latest 35-second valley window measured 6.85 ms p99 versus 7.82 ms in the
earlier 55-second window, including a stationary tail in both. Worst frames were
339.69 ms and 283.37 ms respectively: typical pacing improved, but the maximum
did not. The pass run measured 10.66 ms p99 and a 302.98 ms maximum. No claim of
stutter-free play or release acceptance is made.

The orbit pass reached 24 km altitude and crossed a cube face. Its descent ended
on an ocean bed, not in the canyon. Images revealed coarse globe banding and
regular ground-light blooms over open water. The latter was traced to city glow
being added after water shading without the water mask; it now uses the same
wet mask as the terrain surface. Native verification of that last shader change
is recorded separately from the preceding orbital captures.

## Compiled release candidate

The full `release` profile (thin LTO, one codegen unit) built successfully in
12m 09s. Frozen executable: `target/landscape-cycles-review/final/axiom-286-release.exe`.
SHA-256: `3FAA5300363388E66E1C08C4A07E917833B9CAF876A52E22351BF0E70F6C3E91`.
`Play-Planet.cmd` beside it starts the game from the checkout with separate
candidate saves. This is a local compiled candidate, not a portable package or
a claim of release acceptance.

Nine factory/editor/multiplayer files changed concurrently during compilation;
planetary files did not. `source-release.json` and `source-release-drift.json`
record that boundary. The frozen binary is the reference for the following run,
not a claim that every current working-tree file is represented by it.

The fresh release valley run enabled cellular physics in addition to player
collision. Actual simulation steps and environment updates appear in its log.
All ten waypoints completed in 24.48 simulation seconds. Across the 55-second
measurement window, including the stationary tail, average rate was 149.34 FPS,
p95 12.673 ms, p99 15.147 ms, and maximum 196.530 ms. Generation took 20.2921 s.
Its terrain checksum, `53e34beb3a962eac`, exactly matched the previous candidate.
This workload is different from the earlier physics-disabled runs and is not
used as a direct before/after performance comparison.

The earlier 260-second orbit run had a 24.869-second worst frame after descent;
that severe stall remains unresolved. The final orbital shader check uses a
shorter 110-second flight window and cannot establish that the long-run stall is
gone. Coarse distant slopes, orbital banding and transient detail residency also
remain open quality/performance issues.

The release orbital captures `release-orbit/captures/03-space.png` and
`04-face-crossing.png` were inspected against the preceding captures at the same
review stages. The repeated light blooms over open water are gone; land lights
remain. The broad bands on the land/globe remain visible and are not counted as
fixed. This is a night-side inspection; it does not establish daylight quality.
The completed 110-second release orbital window measured 6.787 ms p99 and a
492.157 ms maximum. It reached the same 24 km view, face crossing and ocean-bed
descent. The shorter run did not reproduce the earlier 24.869-second stall;
that is not sufficient evidence to mark the long-run issue resolved.

## Follow-up quality and frame pacing, 2026-11-08

The broad green bands in the night-side face-crossing capture were an aurora
projected onto the terrain with 96 hard longitude columns. The disc and sky now
integrate one raised emission shell, clipped at the visible surface, with the
same body-space north pole and animation clock. The old hard-column effect is
removed; the aurora itself remains. The inspected `quality-corridor-orbit` capture
shows the hard strip edges gone. This does not establish daylight or all-weather
visual quality.

New timing splits identified a roughly 386 ms foreground ruin-modifier lookup.
Moving that lookup alone exposed duplicate cold terrain work in location labels,
regional air and underground colour: the intermediate run still reached 516 ms.
The final implementation prefetches ruin influence volumes on a bounded worker
and checks their original 36/68 m boundaries without terrain generation in the
frame loop. Presentation now shares the existing environment sampler with audio:
ground height, biome/location labels, regional air and cosmos fields are read in
the background, then reused for sky and underground grading. Samples retain the
existing 0.5 s request cadence, 16 m reach and 3 s freshness limit. A world change
or teleport discards stale fields; an unavailable sample temporarily uses neutral
presentation. Terrain, settlements, encounter rules and the erosion solve are
not replaced by presentation proxies.

A further cold path existed in orbital scene construction: ground-terminal
details sampled their floor before the existing distance rejection, and skyhook
cargo sampled a ground port before checking whether any shipment existed. The
terminal now performs the conservative horizontal distance check first. Cargo
accepts a lazy port lookup, used only by an actual ground-transfer leg. These
changes preserve detail ranges, distant silhouettes and the saved shipment path.

Further scene timing located the remaining face-crossing spike inside the orbital
model builder. Its newly visible corridors synchronously surveyed nine clearance
columns plus footing/root columns. A renderer-owned worker now prefetches those
exact heights, including during the approach above the existing drawing cutoff.
It retains 64 completed sites, at most eight pending replies and four queued jobs;
world/configuration changes cancel obsolete work. Pending corridors appear once
their actual survey completes; no guessed-height model is substituted. The
normal terrain cover continues drawing throughout. Runtime completion messages
make the eventual return of these structures inspectable in the review log.

Validation uses locally compiled `release-fast` candidates, fresh isolated saves,
seed 74, generator 286 and the same 110-second flight route. The diagnostic run
that overlapped compilation is excluded from performance comparisons. No unit
tests or hosted CI were run, per the workspace instruction.

The final build completed in 3m17s, including shader composition validation.
`target/landscape-cycles-review/final/axiom-286-smooth.exe` has SHA256
`1AEEA577CD6CC842983BF86BE99749E1DC3A8FF2D997EA5CC91A9075C75496CF`.
The 20-file source receipt still matched after compilation. This is an optimized
`release-fast` candidate; the previous full-release executable is preserved.
`Play-Planet-Quality.cmd` launches this candidate with an isolated player save.

Matched flights on the RTX 4090, at 1280x720 and view distance 12:

| Measurement | Before | Updated |
| --- | ---: | ---: |
| Average FPS | 160.93 | 164.28 |
| 99th-percentile frame | 6.801 ms | 6.838 ms |
| Worst measured frame | 511.957 ms | 187.795 ms |
| Global generation solve | 21.514 s | 21.671 s |
| First terrain cover publication, from its request | 23.599 s | 23.411 s |
| First refined cover build, from its request | 27.044 s | 26.888 s |

These runs support a reduction in the largest flight hitches, not faster world
generation or improved steady frame pacing. Both had terrain checksum
`53e34beb3a962eac`. The final face-crossing model build peaked at 17.60 ms;
the diagnostic intermediate implementation had spent 727-807 ms there. Worker
completion messages confirm the surveyed orbital structures returned, including
a cold survey taking 1011 ms away from the frame loop. Weather and gameplay
hitches remain. The route finishes over an ocean bed, not the starting valley.
The earlier 24.869-second stall on the longer flight remains unproven resolved.

The separate `quality-corridor-physics-walk` used the same final executable,
fresh save and ten-leg valley route, with physics and orbital scenery enabled.
All ten legs arrived through normal grounded movement, with `flying=false`, in
24.48 seconds of route simulation. The 55-second measurement averaged 157.15 FPS,
with a 14.635 ms 99th-percentile frame and a 413.796 ms worst frame. That largest
gap included only 12.87 ms of recorded redraw work; its full cause is unresolved.
A 462.91 ms gameplay hitch occurred before measurement, and entering the fresh
world still produced a 17.70-second startup frame. These are remaining problems,
not excluded evidence of smooth loading. There is no matched pre-change physics
walk with orbital scenery enabled, so no ground-walk speedup is claimed.

At the walk endpoint, all 1,563 visible chunk entries had meshes. During movement,
453 of 8,645 measured frames still reported missing detail meshes, with peaks of
69 visible and 20 near entries. These counters do not describe the terrain cover
underneath, but they prevent treating this as a completed streaming/seam fix.
Both orbit runs also had 38 measured frames with missing detail meshes, peaking
at 43; that part of streaming did not improve. Inspected `walk-07.png` and
`walk-10.png` show continuous ground through this part of the valley, but do not
prove every chunk boundary is correct. Tree repetition, bare
ground patches and coarse distant silhouettes still need visual work. The
physics setting and collision traversal were verified; the once-per-second
fluid counters did not establish fluid-update progress in this run.

Receipts, frame CSV files, logs and unedited images are under
`target/landscape-cycles-review/final/quality-before-orbit`,
`quality-corridor-orbit` and `quality-corridor-physics-walk`.
The combined measurements are in `final/native-results.json`.
