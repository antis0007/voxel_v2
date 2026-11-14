# Mountain ravines, material bands and history evidence

2026-11-06 follow-up to [the first mountain erosion review](MOUNTAIN_EROSION_REVIEW.md).
Opt-in Hills/Ridged generator **253**, `geo::RAVINE_GENERATOR`.
The implementation was developed against a frozen copy of the shared dirty tree
in an isolated worktree and applied as narrow hunks. No streaming or mesh file
was changed. Existing saves and world defaults were not migrated.

## Implemented causes and changes

- The horizon shader added seven-metre treads/risers and formation bands after
  the shared terrain material. It also bent the lighting normal toward those
  invented surfaces. Removing that block removes the conspicuous horizontal
  lighting bands; the actual relief normal and shared geological material remain.
  This is a visual correction for all generators, with no changed saved geometry.
- Three stacks at each `vistas` kind-1 landmark were added *after* erosion.
  Generator 253 removes that bank of towers, preserves the lookout footing and
  approach, and leaves natural height, detail and geology outside the footing.
  Its footing blends into the real slope instead of levelling the entire site.
  Other composed arches, cliffs and lookout platforms still exist.
- On the 12 m Hills/Ridged solve grid, existing fault-plane and rift damage
  change rain-fed stream-power erodibility. They do not directly subtract a
  decorative trench. The existing priority-flood drainage determines channels.
  A resistant wall can retain a steeper slope only after actual channel incision
  below it; unincised summits retain the 0.75 talus threshold. Eight effective
  cycles, each with four conservative slope transfers, remain bounded.
- Transport caches its movable neighbour pairs once in the original accumulation
  order. Compact routing omits fixed interior ocean cells from the priority heap.
  Generator 252's terrain, drainage and climate checksum remains bit-identical.

## History interface

`geo::history::sample(&cfg, x_m, z_m)` returns the recorded starting height,
height after the erosion stage, final solved height, net hillslope transfer,
fluvial incision, fracture damage, final wet contributing area and lake surface.
`geo::history::budget(&cfg)` exposes total hillslope boundary export and fluvial
removal. The sample coordinates and heights are physical metres, independent of
voxel scale. `mountain_review` exports these fields as CSV and prints bookmarks
for strongly incised valleys.

This is evidence a settlement or ruin placement pass can consume. Positive
hillslope transfer means net deposited material at this stage. Fluvial removal
is detachment-limited; this pass does **not** simulate its downstream deposition.
Area is rain-weighted contributing area, not cubic metres of flow per second.
Final height can differ after subsequent glacial and lake processing.

The four recorded f32 fields cost 4 MiB per 512-square solved face. They are
generated only for the new recipe, included in its checksum, and kept in the
existing derived planet cache. No new save payload, on-demand chunk simulation,
settlement placement or invented chronology is introduced. Unsupported versions,
Planet/other realms, non-finite coordinates and points outside the solved face
return `None`.

## What the long histories actually mean

The civilization design calls for consequences backed by geography and a ledger,
and explicitly rejects pretending that a projected Planet face is a separate
historical watershed. The new Hills history follows that boundary. Existing
Planet work retains its original spherical graph.

| Existing source at the review snapshot | Actual model | Limitation |
| --- | --- | --- |
| `planetary/drift.rs` | 240 Myr of plate advection, 5 Myr steps, snapshots every 10 Myr | Kinematic plate motion and material-age rules, not a mantle-flow simulation |
| `planetary/review/catchment/structural/evolution.rs` | 36 coarse steps with 30,000-year tectonic increments where the wind/weathering stage is enabled | 1.08 Myr of model uplift; compressed, approximate erosion response |
| Same regional catchment solver | 36×180 + 48×73.5 + 48×32 = 11,544 model years | Local multiresolution bedrock/sediment transport, not a global historical river chronicle |
| `planetary/epochs.rs` | Six cooling/thaw stages with three passes each | Representative climate stages without calibrated elapsed years |
| `civilization.rs`, existing gate 242 | 24×20 = 480 years; food, hazard events, aid, displacement, growth and abandonment across two candidate settlements | Small bounded regional model; hazard events are deterministic samples weighted by current geography |
| New Hills/Ridged gate 253 | Eight effective erosion cycles with measured transport and incision | No claimed million-year or thousand-year duration |

The earlier civilization design's statement that no bounded settlement history
was found is stale relative to this source snapshot. A separate history task owns
gate 254; this change does not edit or claim completion of that work.
No connection from this new API to settlement placement is claimed here.

## Verification

Seven focused tests passed in the isolated worktree (26.45 s) and again after
integration in the shared checkout (25.76 s). They cover conservative transfer,
fixed outlets, wet versus dry incision, connected drainage, compact-route exact
equivalence across flat/pitted/coastal/all-fixed cases, history accounting,
legacy checksums, scale independence and the 32 m footprint transition.

- Hills seed 2738958700, generator 241: `f1dad00ead297448` unchanged.
- Same world, generator 252: `f7627e4ab2a9741b` unchanged after optimization.
- Planet seed 74, generator 243: solve checksum `b52ab1a8f7312264` and sampled
  hash `67223bf1c3aa753a` unchanged in a separate fresh-process survey.
- Generator 253 history and shape agree at 1 m, 0.5 m and 0.25 m voxel scales.
- The native build passed validation of all 27 composed WGSL programs. The new
  runtime reports `world_fnv1a64=00c78b918eeeda96 build_match=true`; the baseline
  reports `fa7a67aadc619ab3`.

Full repository unit tests and the app migration suite were already blocked by
unrelated test compilation errors in the first review. The focused integration
test includes the production solver source and exercises the public worldgen
library. It is not evidence that those unrelated suites are fixed.

## Reproduction and evidence

`scripts/review-mountain-terrain.ps1` defaults to generator 253. It creates fresh
scratch saves and records the executable hash, full arguments and environment.
Native comparisons use 1280×720, clear weather, fixed sky/review time, 12 view
chunks, 20 seconds warmup and 20 seconds measurement. These static cameras do
not validate a flying residency transition; that remains the streaming task.

Evidence is retained outside the disposable worktree under
`C:/Users/antis0007/Documents/Codex/2026-11-06/task-10/evidence/`:

| Hills seed | Matched camera x,z,absolute-height encoding,yaw,pitch | Capture directories |
| --- | --- | --- |
| 2738958700 | `464,-720,-710,0,-0.27` | `followup-before-2738958700`, `followup-after-2738958700` |
| 7 | `-272,-676,-640,0,-0.15` | `followup-before-7`, `followup-after-7` |
| 74 | `-304,-20,-620.55,0,-0.15` | `followup-before-74`, `followup-after-74` |

The benchmark's negative height encoding means absolute y = -argument - 500 m.
Each comparison uses quarter-metre voxels. The primary seed also has a new
one-metre capture in `followup-after-2738958700-1m`.

All nine capture directories were inspected. The primary view loses its broad
horizontal lighting bands. Seed 7 loses the three imposed pillars while keeping
the lookout. Seed 74's remaining composed cliff still exists, with smoother
lighting. At one metre, ordinary voxel quantization remains obvious. The same
view-chunk count spans a different physical distance at each voxel scale, so the
one-metre and quarter-metre images are not a matched residency comparison.

An additional matched one-metre drainage view uses seed 74 and camera
`-880,-420,-660.47,0,-0.30`: `followup-gorge-before-74` and
`followup-gorge-after-74`. It retains a connected valley leading toward the lake,
with a localized cut and steeper adjoining wall. At (-880,-640), sampled ground
changes from 63.222584 m to 50.469036 m, a 12.753548 m lowering. The overall
render difference is modest; this is not a newly generated spectacular canyon.

At sampled incised valleys the recorded cut is 21.14 m at (832,-496) for seed
2738958700, 10.07 m at (-512,528) for seed 7, and 30.81 m at (-880,-640) for seed
74. These are cuts from the starting surface during the *entire* new erosion
stage, not additional depth relative to generator 252. Per-seed CSVs include
the actual before/after stage fields; the rendered height also includes later
sampling and authored features.

The primary sampled field has fewer steep samples after the change, while seed
74 has slightly more. Steepness alone is not a realism score: resistant gorge
walls may be steeper even as unsupported peaks retreat. The model still uses
approximate fracture resistance and a heightfield. It does not create volumetric
joint openings, simulate bank collapse at voxel resolution, or date canyon
formation. Pale broad slopes, some authored relief and hard shadow boundaries
remain visible. The user's original screenshot seed/camera were not available.

## Bake cost and integration receipt

Three interleaved fresh-process rounds used Hills seed 2738958700, in the order
old 252, optimized 252, then new 253. These ran after our native capture processes
and builds ended. Values are milliseconds; machine load from other tasks was not
controlled. Logs are `evidence/timing-{old252,new252,new253}-{1,2,3}.txt`.

| Recipe | Erosion stage, median (range) | Including fault-field setup, median | Full solve, median |
| --- | ---: | ---: | ---: |
| Original 252 | 532.9 (474.0–551.9) | 532.9 | 5678.1 |
| Optimized 252, identical output | 361.9 (359.2–362.9) | 361.9 | 5393.1 |
| New 253, recorded fractured ravines | 419.0 (403.0–441.1) | 529.8 | 5616.9 |

The unchanged 252 erosion pass is 32.1% faster by these medians. New fracture
setup and history consume the savings: the complete new stage costs about the
same as the old one. The full world solve improves only slightly; the older
climate/erosion stages still dominate. No runtime FPS improvement is claimed.
Native p95 frame times varied from 6.4 to 45.3 ms under shared machine load;
complete receipts and image hashes are in `followup-evidence-summary.json`.

The narrow patch applied cleanly to `C:/dev/voxel_v2_backup`, and reverse-check
verified its presence after shared-checkout tests. Source remains uncommitted;
the opt-in review executable is `task-10/native-followup-after.exe`, not a
replacement for any distributed or default game executable. The reproducible
increment is `task-10/mountain-ravines.patch`, with complete files and hashes in
`followup-deliverable/` and `followup-deliverable-manifest.json`.

`scripts/session_report.py HEAD` completed. Its +141756 net code lines and +833
test functions describe the inherited shared snapshot, not this change. The
terrain increment itself changes 12 implementation/test/script files: 317 added,
77 removed code lines, plus this review. The disposable worktree is removed
after preserving this package, captures, frozen executables and logs.
