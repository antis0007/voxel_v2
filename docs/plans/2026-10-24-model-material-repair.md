# Model material repair — 2026-10-24

Scope confirmed by the user: materials/rendering here; the other active session owns the landscape/architecture overhaul. No tree or generator-version edits were applied by this task.

## Confirmed fault and changes

- `mesh/models.rs` selected family 0 before creatures and after spell pieces. `world/habitats.rs` also reset the shared builder to 0. Family 0 selects the grass/soil/climate shader, so enemy bodies and the swinging mining tool inherited terrain coloration.
- Family 39 now preserves authored vertex colors for generic moving models. Entity material selection is scoped; habitats restore their incoming material. The hand uses skin (54), and mining/maul heads use the existing metal material (59) with local UVs. Emissive accents remain separate.
- The existing camera-relative mesh flag now reaches the fragment shader. Depth placement, pattern coordinates, neutral first-person fill and sky-reflection exclusion apply to all held parts, including the metal head.
- Rigid model materials skip world-anchored terrain dither, relief, cave grime and roughness noise. Their colors no longer depend on biome coordinates.
- The earlier masonry/wood pass remains: physically narrow joints, larger horizontal paving, neutral masonry variation, staggered boards and lengthwise grain.

## Evidence

`target/model-material-review/` contains immutable source snapshots and per-run executable hashes/arguments/environment in `manifest.json`.

- `before-daylight/bench-000.png`: original model routing and original masonry shader, with the same diagnostic pose and arena as the fixed build.
- `after-daylight/bench-000.png`: gray metal head, brown shaft, skin-colored hand and distinct creature materials; quieter stone paving.
- `after-albedo/bench-000.png`: material-only view confirms this is a material-routing repair, not a lighting/color-grade workaround.
- `target/release-fast/material_fixed.exe`: successfully launched frozen candidate; source in `fixed-source/`.
- `target/model-material-tests.log`: full production model batch regression passed for five creature families, mining tool/maul, and 1 m/0.25 m voxels (1 test, 452 filtered out).

The diagnostic scene forces a held-tool pose. It is actual engine output, not a gameplay walkthrough or the original screenshot location. Original seed/location and the 13 reference images were unavailable. The user confirmed they do not know the original seed/location.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/Review-ModelMaterials.ps1 -Executable target/release-fast/material_fixed.exe -Label unique-label
# Add -DebugView albedo for the material-only view.
cargo test -p minecraft_style_voxel_prototype --profile release-fast --locked --offline --target-dir target model_materials_keep_terrain -- --nocapture
```

Short 1280×720 diagnostic runs on RTX 4090/Vulkan recorded GPU pass means of 0.585 ms before and 0.552 ms after; frame p95 was 2.151/2.687 ms. These six-second capture runs do not establish a performance improvement or representative gameplay performance.

## Integration status

All material changes are in the shared source. The frozen candidate and shader validation passed. The normal executable rebuild was blocked by simultaneous worldgen work (duplicate `surface_reserved` and missing `refinement::silhouette`); see `integrated-build-retry.log`. Do not replace or revert the other session's changes.

Building identical package names from separate frozen workspaces into the shared target reused a stale shader build-script executable. The runtime integrity guard correctly refused the mismatched build. Rebuilding the two generated build-script executables fixed validation; no source guard was weakened. A normal build after architecture work settles is still needed, followed by startup verification. Older failed `after/` evidence contains no successful render and must not be used as acceptance.

Remaining work belongs to the broader overhaul: architecture and landscape specimens, ordinary gameplay walkthroughs, dark-interior light composition, and full reference/quality-tier/streaming acceptance. This patch does not claim those packages complete.
