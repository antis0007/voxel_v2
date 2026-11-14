# Architecture recovery: Generator 58

Status: structural implementation verified locally; not full visual acceptance.
The benchmark is one smith's cottage, not a replacement for every settlement,
castle, ruin, or the separate workshop yard. No lighting/viewmodel overhaul.

## Audit of the actual generation path

- `valley/settlement.rs` chooses six fixed role-based plots in surveyed village
  axes. `floor_at` snaps the graded hamlet elevation to the selected voxel size.
  The benchmark is role 2, at village coordinates (22,10).
- Generator 56 `settlement/craft.rs::house` overlays walls/roof/windows, but
  returns `None` for much of the interior. `valley.rs::timber` then supplies
  floors, old room furniture and potentially other shell geometry. These are
  independently described volumes, rather than a shared room plan.
- Its roof is a point-sampled height function with a fixed shell thickness.
  It does not guarantee face-connected roof courses on rotated grids. Eaves
  and exposed trims widen to a full metre at coarse scale. `Planks` covers
  both roof and boards. The role-2 opening clears the entire front working bay.
- Generator 57 was already present before this change. Its separate
  `settlement/workshop.rs` owns its interior and adds a porch, chimney, lamps,
  stations and RoofTiles. It remains unchanged by this pass.
- The separate `architecture.rs` already has Building/WingSpec/Palette/Door,
  room partitions, connected doorways and stairs. The valley settlement does
  not use that system. Refactoring all existing buildings into it would change
  far more than the requested benchmark and threaten legacy generation.
- World materials are projected in the shader, not per-building UV charts.
  Log has vertical bark and horizontal end grain; it is not an oriented sawn
  beam. Existing roof tiles have their own material. Limestone is still noisy
  stone, not authored plaster. This pass does not claim those assets are fixed.

## Construction and compatibility

`valley/settlement/hall.rs` defines a fixed HallPlan with footprint, Gable and
Opening records. It owns an 8 x 8 m room, foundation, plank floor, walls,
corner supports, plate/lintel, doorway, side/rear windows with sills, roof and
perimeter fixtures. The centre aisle is reserved. Workbench, Smelter,
SalvageCrate and Torch use existing gameplay block types; no new machine
simulation or connected production line is implied.

The roof has one ridge and two slopes. The sampler includes the vertical
slope interval of a voxel so adjacent courses overlap. At the gable ends,
wall infill conceals that overlap and the exposed verge is one course.
The shell and its intentional air override the legacy sampler as one unit.
Only generator >=58 and role 2 enter it. Versions <=57 retain their branches.
There is no save migration. CURRENT_GENERATOR_VERSION is 58.

Vertical supports use Log; horizontal plates/lintels/sills use Planks to avoid
putting vertical bark/end-grain logic on horizontal logs. This is an explicit
temporary material limitation. Roofing is RoofTiles, infill Limestone,
foundation StoneBricks, threshold Cobblestone. No extra render geometry,
draw pass, material ID, or runtime heap allocation was added for the house.

## Verification and evidence

- Local `cargo test -p engine_worldgen --lib valley::settlement -- --nocapture`:
  4 passed, including the two pre-existing 56/57 tests. New checks cover entry
  headroom, real window voids, overlapping roof columns under four rotations,
  supported scales, bounded footprint, foundation, repeated deterministic
  samples and bounded volume. They are not player traversal or a comprehensive
  historical-world snapshot suite.
- Optimized application build passed. Shader compilation includes the new clay
  diagnostic. Existing unrelated warnings remain.
- `render/debug.rs`, `main.rs` and `world_shader.wgsl` add `--benchmark-debug=clay`:
  geometric normals, fixed diffuse illumination, no albedo texture. This is a
  massing diagnostic, not a physical daylight/shadow validation.
- `valley.rs::place_review_view` adds front/back/left/right/rear-quarter/roof/
  exit cameras. `tools/Review-Architecture.ps1` runs the 12-view gate with
  per-image manifests, hashes, camera receipts and logs.
- Final frozen executable:
  `D:/CodexArtifacts/architecture58-20261026/hall58-r3.exe`
  SHA256 `9DE9CEF660897903834467500043B592F1E717CFD19B41952657E6CE01ECAD92`.
- Disk C: filled during review. Only this chat's superseded r1/r2 executable
  copies were deleted. Their images/logs remain under `target/place-review`.
  Final output is on D:; existing project/user artifacts were not cleaned.
- Review suite uses seed 424242, 0.25 m, fixed cameras, time 0.5 (night 0.0),
  clear weather, 1920x1080, four view chunks, two measured seconds. Horizon
  drawing is disabled to isolate the building. Distant terrain is therefore
  intentionally absent; these images are not landscape/streaming acceptance.
  Material-ID views classify rendering materials, not structural roles or
  wireframe. The review script explicitly stops its owned process only after
  the resident image and final benchmark report exist, to avoid waiting for
  unused horizon-worker shutdown. This is recorded in `capture-stop.txt`.

## Measured limits

The final local sampling test counted 852 / 5,036 / 33,333 occupied cells at
1 / 0.5 / 0.25 m in a bounded 12 x 12 x 16 m sample volume, including the deep
foundation. Four sweeps took 0.064 / 0.234 / 1.716 ms in that test build.
These are synthetic sampler timings, not whole-world generation benchmarks
or allocator profiles; no speedup is claimed.

The first native r1 capture reported 2,641,506 terrain triangles, 259 terrain
draws and 10.649 ms average GPU passes. These include the surrounding scene
and horizon and are not final-candidate performance or a regression comparison.
Final per-view benchmark/stream logs carry draw, triangle, GPU memory, missing
chunk and timing counters. Readback, background work and short runs prevent
using them as an isolated performance acceptance gate.

## Visual gate and remaining scope

R1 was rejected after native inspection: its tall projecting roof remained
too heavy. R2 reduced pitch/eaves; the quarter-metre view clearly enclosed a
room, exposed actual rear windows through the doorway and eliminated the black
entry void. Its clay roof view showed two continuous slopes. The metre-scale
version remained visibly chunky. R3 conceals overlap behind the gable and adds
window sills; final multi-angle findings follow below.

Still required before full acceptance: quiet authored plaster, sawn oriented
timber, better coarse-scale edge treatment, functional ventilation, a more
distinct occupational facade, terrain-aware worn approach edges, actual player
navigation/station interaction, broader seed/slope/legacy regression checks,
and isolated generation/frame-time/memory comparisons. The existing garden and
yard were preserved; no new botanical/mana apparatus is claimed. Trees, sky
lines and held assets remain separate defects/workstreams. Additional roof
archetypes, cottage/ruin reference families and decorative systems are deferred
until this benchmark is accepted.

## Final native observations

The final 0.25 m front/rear/right, rear three-quarter and elevated images show
one enclosed volume with two continuous stepped slopes and no disconnected
cantilever masses. The work-area image shows a supported torch, workbench and
storage blocks. The interior-to-exit image shows doorway depth, the threshold,
floor and unobstructed approach. The night interior remains readable under
local illumination. Clay confirms massing independently of albedo; material-ID
confirms a separate roof family. Neither proves correct authored timber/roof UVs.

The left elevation is partially obstructed by the preserved yard canopy and is
not accepted as a clean elevation. One front three-quarter launch exceeded the
180-second limit before its camera receipt; its failure log is retained. This
is a capture/runtime limitation, not a passing image. A wider camera retry is
recorded separately. The metre-scale roof/material gate remains stricter than
the quarter-metre result; no cross-scale artistic acceptance is claimed.

The most important next visual changes are quiet plaster/sawn timber, finer
coarse-scale roof edges, and a workshop facade shaped by its occupation. Then
address worn terrain-aware approaches and the canopy's poor relationship to
the main cottage. No additional roof archetype was added after this review.

Files changed by this pass: `hall.rs`, its version-gated call in
`valley/settlement.rs`, `voxel/config.rs`, camera additions in `valley.rs`, the
clay enum/CLI/shader branches in `render/debug.rs`, `main.rs` and
`world_shader.wgsl`, `Review-Places.ps1`, `Review-Architecture.ps1`, this report
and `GENERATOR_58.md`. These files coexist with substantial pre-existing dirty
work; their full Git diff is not attributable to this pass.

## Modular authoring contract added after the review

`architecture/plan.rs` now supplies explicit room rectangles with semantic roles,
interior edge portals, typed exterior entrances, physical bay dimensions and a
role-based palette. Its compiler feeds the existing Wing sampler. Validation
rejects gaps/overlap, out-of-bounds or duplicate openings, disconnected rooms,
unmatched mirrored rooms/roles/portals/entrances, invalid scales and excessive
allocation budgets. Planned wings suppress random per-cell furniture so an
unrelated furnishing roll cannot obstruct the authored circulation. Historical
Wing constructors retain their previous behaviour.

The included sanctuary grammar creates 6–14 rooms: a central processional
passage, paired library/shrine chambers, longitudinal loops and a terminal hall.
It is an opt-in authoring example, not a newly populated world landmark. It does
not yet place semantic furnishings or authored windows, and its existing Wing
roof renderer is not the new cottage roof. No new city/swamp/graveyard population
or artistic acceptance is claimed. Source tests cover scales 1, 0.5, 0.25 and
0.125 m. Native grid phase, player navigation, stairs, multi-floor constraints,
terrain adaptation and visual acceptance remain separate gates.

Keep the following layers distinct when expanding the generator:

- Site: terrain envelope, roads, waterline, public approaches and district graph.
- Building: oriented modules, load-bearing envelope, rooms and connected floors.
- Programme: room purpose, dependencies, optional loops, gated objectives and exits.
- Surface: structural material roles, physical texture density, local orientation,
  edge/trim profiles and limited wear masks. Seed variation changes coherent
  patches and module choices; it must not turn every voxel into random colour noise.
- Dressing: purpose-specific stations/storage/lighting outside reserved routes,
  doors, stairs and interaction volumes. Damage is applied after the intact plan;
  required routes and objective dependencies must be checked again afterward.

Family-specific composition rules, before adding more placements:

| Family | Layout and focal order | Material / damage rule |
| --- | --- | --- |
| City | Street → courtyard → public hall → private/service rooms; linked districts | Repeat facade bays and floor datums; vary whole buildings, not individual blocks |
| Ruined city | Preserve readable former streets and building footprints; deliberate alternate routes | Structural collapse around breaches; surviving piers and restrained rubble |
| Dungeon | Entry → reveal → branching programme → objective; loops and explicit locks | Materials distinguish occupation and depth; damage cannot silently erase progression |
| Sanctuary | Axial approach, paired chambers, terminal sacred space; quiet secondary route | Matched trim and columns; highest contrast at the focal space |
| Haunted swamp | Raised paths, dry refuges, clustered rooms around water courts | Waterline staining, directional rot and moss; safe routes remain readable |
| Graveyard | Gate, main axis, plots, side chapels, crypt network and landmark | Aligned monuments, family groupings and selective subsidence; avoid random tomb scatter |

These are authoring requirements for future families, not six shipped generators.
Symmetry belongs to important axes, bays and focal rooms; terrain and peripheral
wings can vary without destroying those relationships. The current compiler is
single-floor and rectangular, deliberately narrower than the eventual site graph.
Its supported voxel range is positive resolutions up to 1 m; coarser input is
rejected instead of silently collapsing circulation. Continuous geometry is
resolution-independent; rasterized beauty and clearance still require captures.

The transferable principle from Minecraft's documented jigsaw system is the
separation of reusable templates, compatible connectors, pool selection and
placement. This implementation uses metre-based geometry rather than importing
block-resolution templates. Reference:
https://learn.microsoft.com/en-us/minecraft/creator/documents/structures/introductiontojigsawstructures?view=minecraft-bedrock-stable

Additional evidence limitation: the matched Generator 56 front capture succeeded;
the subsequent Generator 58 one-metre run ended without a camera receipt. The
planned half-metre run was not reached. Do not treat those runs as passing visual
comparisons. The wider final quarter-metre front-quarter retry succeeded.

Planner verification: `cargo test -p engine_worldgen --lib architecture:: --
--nocapture` passed 7 tests; the existing blueprint-dump test remained ignored.
This includes actual open portal samples and adjacent solid wall samples for
all sanctuary sizes at 1/0.5/0.25/0.125 m, plus the existing facade, connectivity,
roof and deterministic sampling checks. The compiler produced a non-fatal
incremental-cache hard-link warning and copied files instead. The frozen native
R3 executable predates this opt-in planner addition; its captures prove the
cottage changes, not the sanctuary planner's visual quality.

Late capture retry: `target/place-review/hall58-final-half/camera.png` succeeded
and was inspected. The half-metre facade has a continuous roof and visible
through-door interior, but the coarse raster phase produces unequal eave steps;
large plaster speckles and bark-like frame material remain conspicuous. This
confirms why topology tests alone cannot accept symmetry or material quality.

The final one-metre retry also succeeded:
`target/place-review/hall58-final-metre/camera.png`. Its coherent closed gable
and through-door interior improve on Generator 56, but oversized roof courses,
heavy lintel and bark/plank mismatch fail the intended artistic bar. The matched
before image is `D:/CodexArtifacts/architecture58-20261026/hall58-matched-before/camera.png`.
Both use the frozen R3 executable; generation changes the village placement,
so these are relative-view comparisons, not pixel-aligned scene/performance tests.

Final 0.25 m front run: 179.32 average FPS, 5.577 ms average frame, 6.042 ms p95,
6.236 ms p99; 192,549 terrain triangles and 77 terrain draws. Short fixed-scene
samples with horizon disabled do not establish an overall performance regression
budget. No full generator allocation/time baseline was measured.
