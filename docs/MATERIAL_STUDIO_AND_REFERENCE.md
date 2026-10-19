# Moss & Ember reference studio

Create the scene from **New world → Create Moss & Ember studio**, after choosing 1 m, 0.5 m, or 0.25 m voxels. The courtyard is an ordinary editable, saveable world. Physics starts paused to preserve the authored pond. Building and walking remain available; the initial view uses flight.

Open **Inventory → Materials**. The compact floating editor leaves the world visible. Change a surface family's RGB multiplier, roughness and reflectance, switch lighting with Noon / Sunset / Night / Rain, or try Moss & Ember and Moonstone palettes. Undo/redo and per-family/all resets are available. Changes are saved automatically in the current world’s region folder after each completed gesture. Switching worlds resets the editor history and loads that world’s palette. Save palette / Load palette provide a separate per-world snapshot for experimentation.

Surface families group related blocks (for example stone/basalt/limestone, or leaves/bushes/pine). Enable Individual material override to edit one block independently. Individual RGB multiplies its family tint; enabled finish overrides replace family roughness/reflectance. Texture scale, detail and rain response multiply the family settings; reset returns to inheritance. These are renderer overrides, not changes to collision, recipes, block identity, or the simulation. The GPU receives them every frame; existing meshes and compatible distant surfaces do not need rebuilding. Water reflections still follow water's dedicated lighting model.

Each world stores `regions/<world identity>/materials.vmat`; manual snapshots use `palette-preset.vmat` beside it. Renaming a world retains its identity and palette. The version-2 format has 256 records: 64 surface families followed by stable BlockId overrides, with RGB, roughness, reflectance, texture scale, detail and wetness response. Loading is all-or-nothing with finite/range/completeness checks. Version-1 64-family files remain readable. Replacements retain a `.backup` file. Palettes are world sidecars, so checkpoint rollback does not roll back art settings; copy the complete world folder when transferring a world.

Rain wetness accumulates over several seconds and dries more slowly. Cached overhead visibility suppresses wet films below roofs on resident geometry; upward-facing exposed surfaces darken and gain a broad glossy finish. Water/fire use their dedicated shading instead. This is a surface-film response, not a puddle/fluid simulation. Nonresident fallback geometry has no detailed roof visibility. Wetness history is transient and starts dry when opening a world.

## Authored scene

The physical layout is shared across scales: a winding stream through a wooded valley, shallow pond, timber crossings, warm stone cottage with an accessible interior, rock sanctuary, ruined watchtower and asymmetric groves. The courtyard blends into the valley floor, with limestone shoulders and pine silhouettes framing the open stream corridor. All construction dimensions use metres. Each torch is placed once and uses the existing fixed-metre model, rather than filling a metre cube with multiple torches. Coarser voxels intentionally simplify the contour of crowns, banks, and rocks.

## Repeatable review

`--benchmark-reference-review --benchmark-scale=one|half|quarter` creates an unsaved scene and captures noon, sunset, night, and storm/rain after each lighting condition settles. `VOXEL_CAPTURE_DIR` chooses the output directory. `VOXEL_REVIEW_EDITOR=1` includes the material panel. These captures are renderer evidence, not a claim of native mouse interaction.

The opt-in editor review clicks Moonstone, Save, Reset all, Load, Undo and Redo through egui pointer events and records the resulting masonry values. Use an isolated `VOXEL_SAVE_DIR` for this replay. `VOXEL_REFERENCE_PHASE=0|1|2|3` pins a single condition for a shorter diagnostic run. A 16-second run captures the pinned condition after 13 seconds.

## Cloud layering correction

The original storm view showed repeated horizontal shelves. The cloud march now samples the full integration intervals with two stratified density samples on long segments, instead of shifting a fixed quadrature pair together. Low/mid/high layer budgets are 192/72/32 steps. The rain march also uses a frame-stable, per-pixel phase. Dense storm skies receive a small positive-weight reconstruction filter that rejects terrain neighbours and preserves opacity edges; clear-sky reconstruction remains unchanged. This removes the coherent shelf pattern without animated dithering or reliance on temporal history.

The editor uses compact local typography and collapsible RGB controls. Material controls scroll independently; presets, history, save and load remain in a fixed footer, including when finish controls expand.

The review also corrected the courtyard's smaller-scale rectangular ground step, a floating outer tree trunk, the tree that blocked the sanctuary, and material seam filtering that used voxel units instead of metres. Material table lookups run in the vertex shader and reach the fragment shader as resolved colors and finish values. Coarse terrain carries its material family to the vertex shader, allowing smooth palette blending across distant triangles without interpolating IDs.

## Next implementation increments

Implemented: world-bound palettes, independent block tint/finish/detail/scale/rain response, gradual wet films, and the reference valley with stream, crossings, woodland and ruins.

1. Named preset browser and import/export picker; bundle world sidecars for transfer. Include art settings in checkpoint history if rollback should restore them.
2. Texture/normal asset slots and a preview swatch, with metre-based UVs and filtering at distance.
3. Real rain accumulation, puddle thresholds, contact ripples and gradual drainage; retain dry shelter areas through distant geometry transitions.
4. Ground-level walking review routes through the cottage, stream crossing and ruin; inspect light continuity and tree silhouettes while crossing LOD boundaries.
5. Larger terrain-integrated biomes, stream source/outlet simulation and ecological vegetation distribution. The current valley is a bounded, editable art reference on the building-plains world.

## Captured evidence

The 12-view environment review is in `target/reference-review/reviewed/review-matrix.png`; every full-size view was inspected individually. Its frozen executable is `target/release/reference-environment-reviewed.exe` (SHA256 `A2715DFC17C90F0753170FC9A054893688DF79283325E8E174C22D06C7649579`). The cloud shelf comparison is `target/reference-review/cloud-layering-comparison.png`.

Keep environment and editor receipts separate: concurrent development may change lighting or UI after a capture. Frozen source snapshots must not share an indistinguishable Cargo artifact fingerprint with the live checkout. When validating an edited source file, inspect the executable dependency file and retain an immutable executable copy for the run.

The final editor review is `target/reference-review/editor-source-verified/HalfMeter-noon.png` at 1920x1080. All six injected egui pointer checks passed: Moonstone, Save palette, Reset all, Load palette, Undo, Redo. Expanded finish controls leave the footer accessible. See the adjacent `receipt.txt` for the executable hash. `run-material-studio.cmd` launches this reviewed editor build; use New world → Create Moss & Ember studio, then Inventory → Materials.
