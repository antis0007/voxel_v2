# World visual overhaul: native evidence

This records the implementation following the rejected palette-only revisions.
It is not acceptance of the complete P0-P9 brief.

## Implemented in this pass

- Replaced shader-painted flower dots with rooted, double-sided stems and five
  petal cups. Flowers occupy selected meadow patches rather than every turf tile.
- Rebuilt grass as five curved, tapered blades per clump, with separate root,
  shoulder and tip vertices, height-based wind response and root-to-tip shading.
- Ground cover uses continuous nine-metre patch fields, open gaps and a bounded
  768-item budget per chunk. Placement remains derived from supporting voxels,
  with stable world hashes and the existing normal remesh path.
- Removed the old flower-pixel shader branch. Kept the common world texture grid.
- Enlarged low cloud masses and reduced internal erosion; grouped the middle
  layer into broader banks. Fair-weather middle coverage is subordinate to low
  clouds, and high ice sheets require a matching weather bank rather than
  accumulating faint streaks across the whole sky.

Concurrent workspace changes include generator-47 crowns, architectural roofs,
tuft vertex colour correction, stone geometry and torch lighting. They are
preserved in these builds; screenshot differences cannot all be attributed to
this pass. Current generator default advanced to 48 during the work. Captures
below explicitly force 47 for continuity.

## Evidence

Frozen binaries: `target/world-overhaul-20261024/after-r2.exe`, `after-r3.exe`,
and `after-r4.exe`. Each capture manifest records the executable SHA-256.
R4 local release-fast build succeeded; build.rs validated all 19 composed WGSL
programs. Existing unrelated compiler warnings remain. No new tests or CI added.

- `target/reviews/forest-repair-20261021/overhaul-r2-meadow/camera.png`:
  first meshed flowering meadow; straight blades were visibly too rigid.
- `target/reviews/forest-repair-20261021/overhaul-r3-meadow/camera.png`:
  inspected curved blades and smaller flower cups. At 1280x720, capture-enabled
  p95 was 15.741 ms, GPU terrain 2.799 ms, total GPU passes 9.359 ms.
- `target/dungeon-review/overhaul-r2-hall/camera.png`: inspected supported ribs,
  pilasters, pale central route and local torch pools. Hall still needs broader
  architectural composition work; this is not final reference acceptance.
- `target/reviews/forest-repair-20261021/overhaul-r4-shore/camera.png`:
  inspected final cloud revision and near/far water. Capture-enabled p95
  9.433 ms at 1280x720. Broad flat gravel apron and distant landform composition
  remain visibly below the reference direction.
- `target/reviews/forest-repair-20261021/overhaul-r4-edits/traversal.mp4`:
  68-second edit run completed. Inspected capped frame 004, restored frame 020,
  and final restored camera after the away/return stages. Plants disappear under
  caps and return after normal remeshing; final support includes the same flower
  clump. This checks derived presentation, not disk reload or physical movement.
  The run contains a 1658.512 ms maximum frame during streaming/capture and does
  not pass smooth-traversal acceptance.

These are renderer captures, not general performance acceptance or proof of
physical traversal, save migration, all weather conditions, or all 13 specimens.
The full castle/town/tower/pit/mine layout scope is not completed by these
renderer and meadow changes. Existing saves were not opened by the review runs.

## Generator 49 continuation

Implemented a narrower dry beach band, grass-backed coastal shoulders and a
shallower castle gatehouse with usable forecourt space, planted side bays and
limestone seating. All generation changes are gated at 49. Older version branches
remain in place; no existing saves were opened or migrated.

The first coastal revisions revealed an ordering defect: the spawn basin and
later landforms erased relief applied in the early shore pass. Coastal shoulders
now operate after natural landforms, before authored river/village and Amberroot
grading. The fixed coast view now has a raised planted bank rather than a broad
flat gravel apron. The waterline remains in place.

Matched coast comparison uses `after-v49-r3.exe` for both versions, camera
`192,-192,1.7,0,-0.12`, seed 2738958700, 1280x720:
- Before: `target/reviews/forest-repair-20261021/coast-v48-matched/camera.png`.
- After: `target/reviews/forest-repair-20261021/coast-v49-r3/camera.png`.
Both images were inspected. This is one coastal specimen, not approval of all
coast types, world scales, or distant terrain transitions.

Final build: `target/world-overhaul-20261024/after-v49-r4.exe`, SHA-256
`9A365EB1440E582DAF5C67D4D913692821FF330F7CBE750F631FCD7160140E8D`.
Local release-fast build passed with composed shader validation; no tests or CI
were added or run in this continuation. Build output was isolated after the
shared artifact directory became occupied by another session.

Castle location: (1729.1007, 20, 1912.3262), quarter-turn 2. A first capture hit a
GPU device-loss error before rendering. Subsequent captures succeeded. Initial
planting was mostly clipped by the deep gatehouse and threshold clearances;
the final layout shortens the gatehouse from two bays to one, retaining its
exterior gate threshold and rebuilding the room plan from the changed dimensions.
