# Mountain erosion review, 2026-11-06

Generator **252** is an opt-in Hills/Ridged candidate. Current defaults remain
Hills 241 and Planet 243. No save format, loaded-world recipe, edit layer,
streaming scheduler, renderer, or material shader changes are included.
Promotion to the default is intentionally outside this patch: other dormant
gates precede 252, and the screenshot's save/seed/version was not available.

## Diagnosis

The supplied screenshot was inspected, as were real native game captures.
Its silhouettes are geometric; the pale horizontal bands and speckling also
contain material/voxel-face shading effects. The screenshot cannot be matched
exactly without its world recipe and camera. The fixed-seed review below is a
controlled substitute, not a reproduction of that save.

The Hills/Ridged solve converts its eroded normalized relief to metres using
`t * (0.3 + 0.7*t)`, which steepens the upper slopes *after* erosion. Its final
talus treatment used four in-place, steepest-neighbour sweeps. The local detail
pass cannot rebuild a broad mountain mass: it mainly adds bounded fine cuts.

There is also relief outside the solver. `vistas::height` adds authored rock
stacks and destinations afterward. For seed 2738958700, the point at metre
`976,-896` is 114.90 m high over a 42.38 m solved surface. Those landmarks are
not evidence that the watershed solver failed to erode a 115 m mountain, and
this change preserves their authored forms. Planet volcanism has a separate
summit-protection mechanism; it was inspected but is not changed here.

The original Hills fine detail also disappeared at a 32 m sampling footprint.
At the sampled points, this produced height jumps as large as 6.75–8.59 m.
That is separate from both summit shape and streaming residency.

## Implementation

`geo::mountain` runs eight coupled steps after conversion to physical metres:

- Four simultaneous eight-neighbour slope-transport passes per step, with a
  critical slope of 0.75. Each paired transfer deposits in the lower cell or
  explicitly counts export at a fixed outlet. This redistributes unsupported
  summits into their supporting slopes without scan-direction bias.
- A priority-flood drainage route weighted by the existing rainfall field.
  Implicit stream-power incision follows receivers in downstream-first order.
  Channel initiation fades in over 4,096–16,384 m² of wet contributing area;
  divides receive slope transport instead of independent noise trenches.
- The effective stream-power coefficient is 0.12 times the existing
  erodibility. A weaker initial candidate filled too much of the observed
  ravine; the accepted native comparison retains that valley recess.

The fluvial model is detachment-limited: removed bedrock leaves this local
model. Only the hillslope transfer component is sediment-conserving. This is
not a full hydraulic/sediment-transport simulation or a new fracture model.
The existing final routing, lakes, climate, and rivers consume the new surface.

The derived `PlanetParams::mountain_erosion` flag changes the cache identity
only when enabled. Every legacy fingerprint remains unchanged. It is enabled
only for Hills/Ridged at gate 252 or later, not for Planet.

For this gate, fine relief fades smoothly from an 8 m to a 32 m footprint;
narrow channel beds retain their existing width-based fade beyond 32 m. The
solved mountain mass is shared by fine columns and far samples. This change is
in `geo::ground`, not in streaming or meshing code.

## Fixed-seed measurements

Each field survey uses 63,001 points on a 16 m grid spanning 4 km, centred at
`0,0`. Peaks are cardinal local maxima above 50 m. Curvature is the absolute
64 m cardinal residual. These metrics include authored landmarks as well as
the solver, so curvature alone is not a measure of realism.

| Metric | Hills seed 2738958700, 241 → 252 | Ridged seed 7, 241 → 252 |
| --- | --- | --- |
| Sample hash | `a580e3d7e6554ded` → `29e35e6ae6cee96b` | `99bef1d8750dec8f` → `2f7e8e59535738aa` |
| Solve checksum | `f1dad00ead297448` → `f7627e4ab2a9741b` | `0ca3490255d3eb78` → `8315b3159529c6e9` |
| Local maxima | 554 → 517 | 388 → 381 |
| Field slopes above 45° | 3.108% → 3.125% | 1.793% → 1.788% |
| Field 64 m residual p95 | 15.322 → 15.480 m | 10.874 → 11.443 m |
| Solved mountain cells above 45° | 17.238% → 16.815% | 4.284% → 3.471% |
| Solved mountain residual p95 | 32.141 → 30.818 m | 22.886 → 22.357 m |
| Maximum sampled 32 m LOD step | 6.753067 → 0.000008 m | 8.592072 → 0.000004 m |

“Solved mountain” cells are above 60 m, excluding the border. The steeper
valley walls can increase the whole-field residual while the mountain crests
become broader; the table intentionally includes both measurements.

At all 289 scale-check positions (including negative chunk/tile boundaries),
the physical-height hashes are identical at 1 m, 0.5 m and 0.25 m voxels:
`3cfd6395bd3b01cc` for the final Hills candidate and `587f2a72caf29a3e` for
Ridged. The largest sampled spatial change across ±0.001 m is 0.003708 m and
0.004292 m respectively.

Historical Planet 243, seed 74, centred at `-21000,-365`, retains solve checksum
`b52ab1a8f7312264` and sample hash `67223bf1c3aa753a`, matching the unmodified
build. Historical Hills 241 also retains its original full sample hash.

## Native evidence and limits

Evidence is outside the disposable worktree at
`C:/Users/antis0007/Documents/Codex/2026-11-06/task-10/evidence/`.

- Baseline: `before-mountain-241/bench-001.png`.
- Final candidate: `final-mountain-252/bench-001.png`.
- Quarter-metre review: `final-mountain-quarter-252/`.
- Earlier candidate and lower, partly tree-obscured views are retained in
  the other folders, but are not the accepted comparison.

The matched camera argument is `464,-720,-710,0,-0.27`: X/Z in metres, the
engine's absolute-height sentinel for a 210 m player origin, yaw, pitch.
Both use Hills seed 2738958700, 1280×720, Vulkan/RTX 4090, view-chunks 12,
LOD start 5/levels 2, clear weather, surface clock 0.5 and sky time 12.
Every run uses a new scratch save directory. Each directory contains the
executable SHA-256, complete arguments, raw frame timings and engine logs.

The final native comparison shows a broader crest with the principal ravine
retained. It is a modest terrain improvement, not evidence that all pictured
mountains or all geology problems are fixed. Layer bands, voxel stair faces,
and authored stacks remain. No new cracks or 3D fracture system are claimed.

The new bake measured 0.28–1.26 seconds across these shared-machine runs;
full solve and sampling times varied substantially under concurrent native
reviews/builds. No erosion executes per frame. These observations are not a
clean performance regression benchmark. A final sequential fresh-process pair
measured 9.346 s legacy solve versus 8.666 s candidate solve, with 0.930 s
inside the added pass; the reversal in total time demonstrates the noise.
Both final hashes reproduced exactly. The early quarter-metre run had only
three measured frames, 2,083 ms p95 CPU frame time versus 2.99 ms GPU passes;
its logs are useful to the separate streaming investigation, not an FPS claim.
The final quarter-metre capture, with no simultaneous reviews/builds from this
task, completed 3,042 measured frames at 8.206 ms p95, with 3.159 ms GPU passes.
The final one-metre capture measured 9.155 ms p95 and 6.190 ms GPU passes.
Its baseline measured 18.873 ms p95 and 8.169 ms GPU passes. Scene triangle
counts changed with the terrain; these are observed captures, not an isolated
rendering speedup claim. Both final native scales were visually inspected.

## Verification and reproduction

The final native release-fast build passed. Four focused integration tests
passed in 9.52 seconds: mass conservation and scan symmetry, fixed outlet
accounting, rain-fed downhill drainage, legacy checksum, scale equality,
negative-coordinate continuity and the 32 m LOD boundary.

```powershell
cargo test --locked --offline --profile release-fast -p engine_worldgen --test mountain_transport
cargo run --locked --offline --profile release-fast -p engine_worldgen --example mountain_review -- hills 252 2738958700 0 0 output.csv
./scripts/review-mountain-terrain.ps1 -Executable <release-fast-exe> -Generator 241 -OutputDirectory <new-baseline-directory>
./scripts/review-mountain-terrain.ps1 -Executable <same-exe> -Generator 252 -OutputDirectory <new-candidate-directory>
```

The broad worldgen unit target is blocked by four inherited compile errors
in `planet.rs`, `plants.rs` and `trees/structure/tests.rs`. The existing v12
save/inventory migration test could not run because the app test target has
28 inherited compile errors, including outdated egui APIs and test helper
signatures. Logs are `mountain-tests.log` and `save-compatibility-test.log`
in the task evidence root. These failures were not repaired outside scope.

No real player saves were loaded or rewritten. Save compatibility is supported
by unchanged generator defaults, unchanged persistence code and bit-identical
legacy generation; the blocked migration fixture is not claimed as passing.

The implicit stream-power approach follows
[Braun and Willett (2013)](https://doi.org/10.1016/j.geomorph.2012.10.008).
This improves the existing solver; it is not a claim of parity with the latest
terrain authoring tools or a physically calibrated geological history.

## Shared-tree integration

The scoped patch was applied to the current shared source. All four focused
integration tests passed there in 20.62 seconds after a 26.92-second build.
The concurrent flat-river-current function, water-review controls and Planet
fault/weathering review gates were preserved. No streaming or rendering files
were replaced. The full native build/captures above use the isolated snapshot;
the shared-source verification covers the worldgen integration target.
