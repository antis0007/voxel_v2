# Torch lighting and distance detail

- Torches use a wooden shaft, two metal collars, wrapped grips, wall plates and brackets, an emissive ember head, and an animated split flame. Ember/flame emission is explicit rather than inferred from block coordinates, including wall mounts.
- LOD preserves the six-sided split flame silhouette at every level and omits small fittings at levels 2–3. The full-detail allocation bound remains 320 vertices / 480 indices per torch.
- Up to 64 indexed torch sources illuminate terrain. Selection covers 64–192 metres according to terrain range; smooth fades at the range and light-budget boundaries prevent hard source swaps. The physical influence radius remains 13.5 metres.
- Light attenuation uses a softened inverse-square response with a smooth finite cutoff. Reduced back-facing fill improves directional contrast; these lights still do not cast occlusion shadows.
- Moonlight intensity is doubled. Clear-night fog starts at 75 metres and ends at 320 metres, with the existing weather attenuation; daytime endpoints are preserved. Night ground bounce is cool and dim instead of brown.
- Streaming now builds actual LOD variants instead of relabelling identical meshes. Terrain simplification only removes collinear interior edge vertices within level-dependent normal/AO tolerances; chunk edges and geometric contours remain fixed. A one-chunk hysteresis band reduces boundary thrashing. Existing GPU meshes remain visible while replacements are prepared.

Build validation is separate from visual acceptance. Dense torch-field GPU cost and travel across LOD boundaries still require dedicated review. No automated tests or hosted CI were added or run.

Torch dimensions and flame displacement are metre-based at every voxel scale. Floor anchors follow the support face; wall anchors follow the wall face. The same transform places the light emitter. Chunk culling includes the overhang. Above 1024 torches per chunk, the existing compact geometry path bounds allocations.

Inventory: category filters, stable item IDs, larger labeled cards with focus/hover feedback, item preview, expanded supply groups and empty-search feedback. Existing hotbar assignment and drag controls are retained.

## Rendered review

The opt-in `--benchmark-torch-review` fixture renders standing and wall torches from a repeatable physical camera, then captures day, night, Blocks and Pack phases. Set `VOXEL_CAPTURE_DIR` and `VOXEL_SAVE_DIR` to scratch directories; use `--benchmark-scale=one`, `half`, or `quarter`.

Twelve actual renderer captures in `target/torch-review/v2` cover 1 m, 0.5 m and 0.25 m voxels. The standing torch occupies the same approximately 134-pixel height in each 1280×900 view. Wall placement follows the support face while model proportions stay fixed. This does not establish distance-transition performance.

Inspection found a collapsed inventory body at 200% UI scaling. Compact headers, cards and footer now leave the content scroll area visible; corrected Blocks and Pack captures are in `target/torch-review/final`. Flame colours now convert from authored sRGB before emission, avoiding pale pink daytime flames. Local fire compensates the environment exposure and night grade so nighttime does not dim the source and its pool along with moonlit terrain.

Final release `target/release/torch-visual-reviewed.exe` (SHA256 `3A14AB365CEF5732B9156D2B9D2F5A459B1F36D114727443450F67B5DCE3E480`) exited successfully with empty stderr. Its four quarter-scale captures in `target/torch-review/reviewed` were inspected: night illumination visibly reaches the wall and floor while the surroundings remain dark; Blocks and Pack retain readable scrollable content. These are rendered visual checks, not manual drag/drop or dense-field performance acceptance.

## Movement and drag regression review

Set `VOXEL_TORCH_REVIEW_MODE=motion` with `--benchmark-torch-review --benchmark-scale=quarter --benchmark-view-chunks=8 --benchmark-duration=85`. The camera retreats 56 metres and returns; three-second captures include source-strength and resident-LOD telemetry. The held torch is disabled to isolate placed lights. LOD 0, 1 and 2 were exercised in both directions; the two indexed sources remained present, with smooth strength reduction near the 64-metre range boundary.

This revealed blackening at approximately 32 metres: exact edited backdrop faces overlapped resident faces while all backdrop geometry skipped torch illumination. Exact edited backdrop now receives local lights and sits 3 mm inside the resident surface to avoid coplanar competition. Procedural distant terrain retains the cheaper lighting path.

`VOXEL_TORCH_REVIEW_MODE=drag` replays actual egui pointer events against rectangles recorded by the rendered inventory. `target/torch-review/drag-after/run.log` records PASS for palette-to-hotbar assignment, hotbar swap, release outside a slot, and focus-loss cancellation. Captures verify the slot contents and item-shaped drag preview. Native desktop attempts were interrupted by disappearing/minimized windows; these results are renderer integration checks, not a completed manual mouse session. Drops now respond only to primary-button release, Escape/focus loss cancels the pending drag, and hotbar drags show the actual item icon.

The corrected distance-lighting candidate is `target/release/torch-lighting-final.exe`, SHA256 `5B9DF445441AADF678151CED62DE6776C71CA4DBB6F7C6B8DEAD4248C43F66FF`. Compare `motion-after/QuarterMeter-007.png` (black wall at roughly 35 m) with `lighting-final/QuarterMeter-007.png` (lit wall at the same camera position), under `target/torch-review`. The candidate contains the backdrop-lighting and depth-separation fixes. The subsequent general GGX cosine clamp is present in source but was added after this candidate's shader was embedded; it is not part of that binary's visual evidence.

Final 85-second movement run exited 0 with empty stderr. Outward and return captures were inspected, including the former blackening boundary, far LOD and near return. Both lights remain indexed; source strength returns to 2.1 before the near approach, and resident LOD returns to 0 without a missing torch mesh after initial loading. This fixture does not cover a saturated 64-light budget or disk-evicted saved-world reloads.
