# Sandbox architecture implementation

This pass implements the formerly missing execution paths. It does not claim production-scale physics or visual acceptance from a successful build.

## Coordinates

Player integration and collision now use a chunk-aligned integer origin plus a small local float position. A double-precision position survives between updates and checkpoints. Picking uses a local DDA. Camera upload retains the double-precision eye residual in its projection, avoiding rounding it away before drawing. Existing teleport assignments are reconciled with the precise position. Integer voxel/save coordinates keep their existing supported bounds; legacy world-space effects and entity APIs still use float views.

## Distant edits

The horizon worker consumes an immutable world snapshot, including saved region records. It subtracts edited chunk volumes from procedural triangles and builds greedy faces from the actual voxel contents, including holes, overhangs and construction. Neighbour records determine face visibility. Region records are loaded a seven-chunk neighbourhood at a time; IO is off the render thread. An unreadable record prevents publishing a procedural replacement. Continuous edits refresh at most once every two seconds, plus worker latency. Exact edit faces use a separate shader marker so the procedural underlap cannot sink structures. Very large edited skylines still require a future independently paged GPU proxy cache; the current combined mesh scales with visible edit surface complexity.

## Heat, combustion and gases

The selectable palette contains Fire, Smoke, Steam and Ash. Heat diffuses through a bounded sparse field. Wood ignites when hot, burns finite fuel, emits smoke, and leaves ash. Adjacent water extinguishes it; enclosed fires without an air neighbour go out. Lava supplies heat. Boiling transfers the water voxel's quantized volume into steam; condensation restores that same amount. Gas rises and equalizes sideways without overwriting solids or another gas type. Transport recipients wait until a later simulation tick. Gas vents only at the world's open upper boundary. Smoke and fire use screen-door coverage; fire is emissive.

The model uses an ambient heat reservoir, equal cell heat capacity and simplified phase thresholds. It is not a pressure/oxygen chemistry solver. The sparse field has 32,768 entries; admission preserves pending work at capacity. Thermal metadata survives source residency filtering and checkpoints.

## Rigid assemblies

Place a Rigid assembly core against connected logs/planks to activate an assembly. Up to 64 assemblies, each up to 128 voxels, retain their block mass and quaternion orientation. Larger connected components remain static. Motion includes gravity, swept substeps, rotational occupancy checks, contact impulses, friction and support torque. Rotation preserves centre-of-mass translation. Collision rejects occupied terrain, water and the player; missing source chunks suspend movement. Atomic prepared edits preserve the assembly and carry its thermal state. Mining or burning a constituent dissolves the assembly without resurrecting removed blocks.

Physics work has a separate 800 microsecond scheduling allowance and bounded accumulated time. Collision geometry is voxel-rasterized, so rotations that collapse multiple cells are rejected. This is a voxel rigid-body solver, not continuous convex contact/manifold physics. Player pushing and fluid buoyancy are not implemented.

## Persistence and verification

Checkpoint V12 adds precision residuals, thermal/gas state and rigid transforms, velocities and pending time. V1-V11 checkpoints remain readable by the new decoder. Save-library discovery and region-reference scanning use the new header offset. Existing region files and material IDs retain their meaning.

The opt-in `--benchmark-sandbox-review` runs the real player, renderer, world simulation, checkpoint worker and production loader against an isolated save directory. The first 68-second release run verified rigid assembly landing, conserved sealed smoke/steam mass, rising gas, exact sandbox checkpoint bytes and precise player position, a saved distant arch, and resumed simulation. It exposed insufficient combustion heat, now corrected. Its original fast-flight precision check was insufficient to exercise sub-float movements; the revised scenario includes a slow movement segment at 900 km.

Local surface detail adds bounded, deterministic grass blades and small stones on exposed supported cells at LOD0/1. Detail stays attached to authoritative voxel support and is regenerated after edits. The horizon compares edited-chunk identities outside its hidden near radius, avoiding full backdrop rebuilds for nearby simulation changes while rechecking on movement. These are cosmetic surface details, not extra collision voxels.

The corrected release run (`target/sandbox-review/run3/stdout.log`) passed:
- 600 precise movement updates at 900 km, including 116 sub-float increments and a chunk-boundary crossing.
- Fire ignition/spread, fuel depletion from 80 to 1 at 12 seconds, and complete burnout after load/resumption.
- Exactly four combined steam/water volume units and two smoke units in sealed chambers; smoke rose.
- Three-voxel rigid assembly landing with all occupied cells intact.
- Exact round-trip equality of 23,004 encoded sandbox bytes and double-precision position (zero position delta).
- Saved distant construction and resumed live simulation. The arch opening is visible in the renderer capture from 200 metres, beyond the 128-metre resident view radius.

This uses scripted real in-game input/state and renderer captures, not manual keyboard acceptance or an exhaustive physics stress suite. Gas visualization remains inexpensive stippled voxel-face puffs; feathered coverage removes the rectangular silhouette but is not volumetric smoke.

Captures are in `target/sandbox-review/run3/captures/`. The first ground-level Hills capture confirmed supported grass geometry and clean chunk edges. A refinement varies blade height, lean and dry colour, adds occasional meadow pebbles, and limits detail to 384 clumps per chunk. Pebbles use flattened irregular tops. Geometry is limited to LOD0/1 and scales in physical metres.

The complete sandbox run averaged 2.252 ms/frame (444 FPS), p99 4.883 ms; the first ground-level Hills run averaged 3.471 ms/frame (288 FPS), p99 9.412 ms, at 1280x720 and four view chunks. Shared desktop/compiler activity means these are observed timings, not controlled before/after performance claims. No visible chunks were missing after initial streaming. There were no runtime stderr errors. Distant edits still rebuild the combined horizon when actually visible; independently paged edit meshes remain a scaling limitation.


## Verified build

`run-sandbox-architecture.cmd` launches `target/sandbox-architecture/voxel-sandbox-verified.exe`. Release compilation and build-time WGSL validation passed; 79 existing warnings remain. SHA-256: `608F0749CF43DD3FCC8E4C5610983D1695BB19F54D8C9094CDA90C774C340B41`.

The full sandbox scenario passed on the preceding release; the final rebuild changes cosmetic ground detail and includes concurrent integrated renderer fixes. Its ground-level renderer capture is recorded separately. Ordinary launch does not enable the review scenario and uses the normal save system. Review runs used isolated `VOXEL_SAVE_DIR` paths under `target/sandbox-review/`, preserving the player's saves.

Final ground-level capture: `target/sandbox-review/ground-final/captures/hills-detail/None/Hills-00-Clear-noon-1.png`. Inspected varied tufts and flattened meadow stones sitting on terrain; 206 visible chunks, zero missing, 96.32 MiB GPU mesh residency, zero stderr bytes. The 23-second run exited normally: 1.974 ms average frame, 2.461 ms p99, 13.370 ms maximum. Concurrent integrated renderer changes and machine activity prevent attributing the timing difference solely to ground detail. No further game processes from this review remain running.

## Lighting recovery review

The recovery restores continuous world-space surface grain (removing the square quantization), muted green reflectance, stronger hemisphere/cavity contrast and reduced directionless fill. Coordinated shader changes include bounded cool dusk sky fill and narrower cloud feet, preserving cloud support/seam handling and water fixes.

The integrated first candidate exposed a severe per-fragment local-mist cost: 102.225 ms terrain GPU time in the wide view versus 0.669 ms in the previous build. Local mist is now disabled by default (`MIST_SAMPLES=0`) pending a depth-resolved implementation; existing atmospheric aerial perspective remains. A grey annulus in that early capture coincided with missing streaming chunks, not permanent lost geometry. Settled captures after 12 seconds show complete terrain.

Release `target/lighting-recovery/voxel-lighting-recovery2.exe` compiled with composed WGSL validation. Both `run-lighting-recovery.cmd` and `run-sandbox-architecture.cmd` launch it. The executable was checked for the embedded muted palette, disabled mist, cool dusk fill and narrow cloud-foot shader text. Day/dusk runs exited normally with empty stderr and zero missing visible chunks. Day terrain GPU time was 0.638 ms; dusk 0.224 ms. Overall FPS is not a controlled comparison because another user session and builds shared the machine.

Inspected captures: `target/lighting-recovery/final-day/captures/hills-detail/None/Hills-00-PartlyCloudy-noon-3.png` and corresponding `final-dusk` path. Daytime terrain has less lime tint and better face separation. Dusk remains deliberately dark with discernible foreground forms. This is a visual recovery increment, not an exact restoration of an unidentified earlier build or photorealistic terrain. Ongoing inventory, mesh-lighting and pebble refinements from other chats are independent work.

## Woodland recovery integration

The coordinated release `target/reference-landscape-review/voxel-reference-landscape.exe` builds successfully (SHA-256 `9625767304BD3822260E2B362915CB6DF2D843806645F25AB0DEF210F6232126`). `run-woodland-recovery.cmd`, `run-lighting-recovery.cmd`, and `run-sandbox-architecture.cmd` now launch this frozen candidate.

Intermediate tree proxies now use authoritative cached tree anchors, species and dimensions out to 720 metres. The fixed 512-metre shader exclusion was replaced with a handoff tied to the requested view distance. Generator version 11 permits suitable dirt, mud and gravel woodland soil while preserving slope/water exclusions and old save generation versions. Crown contours and larger far clusters improve canopy silhouettes. The former sampled local mist was replaced by analytic height integration without shadow-map loops.

Release compilation passed. New runtime visual/performance acceptance is being coordinated by the landscape chat; it must not be inferred from the build or prior lighting-recovery timings. Existing saves receive proxy coverage changes without being forcibly regenerated; expanded spawn eligibility requires a version-11 world.

Runtime follow-up: another coordinated Superflat capture exceeded the device's 256 MiB horizon vertex-buffer limit with the dense forest integration. The three launchers were temporarily returned to the previously verified lighting-recovery2 executable. A live horizon upload size guard now preserves the previous backdrop instead of triggering this validation crash; vegetation budgeting and rebuilt runtime acceptance remain with the landscape owner. The candidate above is not accepted for general launch.

## Vegetation budget fix: verified replacement

Supersedes the temporary rollback above. `run-woodland-recovery.cmd`, `run-lighting-recovery.cmd`, and `run-sandbox-architecture.cmd` now launch `target/reference-landscape-review/voxel-reference-landscape-v3.exe`, SHA-256 `EF3ECB10C2E9BBBDE872B39FC8DC8279D94937720160149FCE39F5DC1266E513` (rechecked at launcher installation).

The root cause was edited-terrain clipping expanding every indexed horizon triangle into unique vertices even outside edit volumes. Unaffected triangles now retain shared vertices; only intersecting polygons are clipped. Vegetation has a hard 1,600,000-vertex budget (89.6 MB), preserves nearby authoritative anchors first, then uses deterministic hash ordering for far groves to avoid scan-order gaps. Crowns beyond 240 m use cheaper geometry. GPU allocations also check the device buffer limit before upload.

Release compilation passed. Three isolated-save 28-second actual-game runs exited 0 with empty stderr. Inspected the emitted PNGs, including camera translation/rotation in both woodland runs:

| Scenario | Horizon vertex bytes | Horizon triangles | Settled visible chunks missing |
| --- | ---: | ---: | ---: |
| Edited Superflat reference courtyard (crash reproducer) | 59,171,728 | 1,484,706 | 0 |
| Quarter-metre Hills, moving view | 69,002,192 | 1,744,713 | 0 |
| One-metre Hills, moving view | 68,194,448 | 1,724,328 | 0 |

The courtyard previously failed at approximately 344 MB against a 268,435,456-byte device limit. The replacement renders the edited scene normally. Woodland captures show continuous middle-distance tree coverage. No upload-limit rejection occurred. Quarter-scale streaming settled by 24 seconds; initial streaming is not instantaneous.

Evidence is under `target/woodland-budget-review/{edited-reference,woodland-quarter,woodland-meter}` with stdout/stderr and captures. Final woodland images are `captures/hills-detail/None/Hills-01-PartlyCloudy-noon-3.png`; courtyard is `captures/OneMeter-noon.png`. One-metre observed average was 114 FPS, p99 18.494 ms, maximum 367.056 ms; concurrent work and a briefly overlapping cloud review prevent a controlled performance comparison. These runs verify the budget/crash and tree-coverage repair, not elimination of all streaming hitches or final art quality. Player saves were not touched. Later art refinements are independent of this verified executable.
