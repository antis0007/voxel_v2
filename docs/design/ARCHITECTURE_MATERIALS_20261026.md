# Architecture material and framing pass, 26 October 2026

## Art rules

Use warm lime plaster for broad infill, dark sawn oak for structural members,
dressed limestone for the plinth, thresholds and sills, and fired clay for roofs.
Built fabric has a quiet, limited ramp: natural rock mottling and bark are not
substitutes for plaster or joinery. Grain follows the member, including end grain.
Surface marks use connected 5 cm clusters; broad walls carry no checker overlay.

A facade owns paired bays and shared sill/head datums. The doorway is centred,
window jambs wrap the reveals, and heads and sills finish the complete opening.
Project trim only by a modest cell at fine scales. Keep the central route clear.
Roof edges and reflected openings must survive voxel rasterization, not just
symmetry in the continuous plan.

## Implemented slice

Generator 61 introduces one rebuilt smith cottage. Its cardinal local frame is
snapped to voxel boundaries, with paired front/rear windows and side windows,
a centred two-metre doorway, complete frames, stone base, clay gable roof and
perimeter workstations. A short approach joins the doorway to the surveyed street.
The previous 58-60 sampler remains available for older saves. Concurrent work
subsequently advanced the current generator to 62; this pass retains its gate 61.

Five appended block IDs provide plaster, dressed stone and vertical/X/Z timber.
Existing block IDs and natural materials retain their meaning. Packed voxel
families 54-58 resolve to internal shader families 64-68 only when a block ID is
present; bare authored-model finish codes retain their existing rendering.

## Evidence

Native build: target/architecture61-r4.exe
SHA256: 0A62CB1DF6863574BA273FA9140B9801EF3FDEDF8A132C09798716A6B7758CB2
Build log: target/architecture61-r4-build.log
All 19 composed WGSL programs validated. No tests or hosted CI were run.

Captures use generator 61, seed 424242, clear daylight, fixed review cameras and
an isolated save directory. Their manifests contain the executable hash and args.
Reviewed native images:

- `target/place-review/architecture61-front-r3/camera.png`: 0.25 m front;
  equal window bays and roof edges, full frames, continuous corner/plate join,
  quiet plaster and matte timber. Purple material-code collision corrected.
- `target/place-review/architecture61-quarter-r2/camera.png`: 0.25 m oblique;
  reveal depth, projecting sills, roof/eaves and the street approach are visible.
  This candidate preceded the small final roof-corner plate correction.
- `target/place-review/architecture61-back-half-r2/camera.png`: 0.5 m rear;
  paired window openings survive the coarser raster. Same pre-corner-fix candidate.

- `target/place-review/architecture61-front-metre-r4/camera.png`: 1 m front;
  raised stone sills now distinguish the paired windows from the centred doorway.
  Framing stays mirrored; the one-metre raster necessarily reads much heavier
  than the fine-scale architecture.

The r4 build log records validation of all 19 composed WGSL programs with
world_fnv1a64=d149382ec2b4b956, the same architecture shader as r2/r3.
The final r4 architecture change raises window sills only at the one-metre scale;
the fine-scale r3 geometry and surfaces are retained. The r3 metre capture exposed
sills merging into terrain and is superseded by r4.

This is one benchmark building, not acceptance of every settlement family.
Openings are actual air reveals; this pass does not add operable doors or glazing.
Camera renders do not establish traversal or performance acceptance.
