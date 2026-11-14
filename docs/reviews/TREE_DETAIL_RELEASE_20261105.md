# Tree detail release — 2026-11-05

## Root cause

New ordinary worlds selected generator 126. Leaf surface relief required 130; branch-attached woodland foliage required 137. Smaller voxels therefore sampled the old crown more finely rather than revealing the authored new foliage. New customized planets selected 136: leaf relief was available there, but the branch-attached structure was still disabled.

The shared shader already contains leaf-clump surface colour, filtered crown normal relief, authored material finishes, and separate creature/architecture/furnishing treatments. Those are material detail; they cannot supply missing branch geometry, gaps, or silhouette breakup. This pass does not replace them with generic noise or claim that every model has been visually reviewed.

## Release wiring

- Ordinary new worlds select tree-only generator 127. The frozen validation snapshot also selected 127 for default planets.
- Leaf relief and structure retain their original authoring thresholds (130 / 137), with an explicit 127 release alias. Existing 126 worlds retain their old geometry; existing custom 136 worlds retain relief and their old crown anatomy.
- Move the dormant planet authored-site threshold from 127 to 128, alongside the already staged Surveyor content. This keeps the tree-only release from enabling unrelated sites, climate, glaciers, ocean shape, or later terrain features. Supported existing 126 / 136 worlds keep the same site behaviour.
- The tree release selected 137 for new customized planets. The recipe minimum remains 136. Creation, editor preview, save acceptance, benchmark review, and multiplayer join acceptance use the selected generation consistently while retaining support for old 136 worlds. Concurrent planet/portal work subsequently advanced the live planet defaults to 143; that work is preserved and also enables the 137+ tree path. Explicit save/join acceptance retains the first tree-detail planet release (137) when those defaults advance.
- Near foliage, crown bounds, and far proxy lobes use the same feature predicates. Woodland and giant crown padding use the released relief predicate too.
- First matched native inspection showed the released branch skeleton still hidden by merged leaf masses. Reduce per-tip cluster radius from 34% to 24% of tree radius (slender forms 40% to 30%); twig positions, wood, leaf surface relief and coarse-grid leaf floors remain intact. Near sampling, bounds and far lobes all consume the same smaller clusters.

No existing world is migrated. To see the new geometry, create a new world; loading an old save deliberately retains its recorded generator. No unrelated working-tree changes were reverted or staged.

## Verification

**Frozen release build passed**, with default multiplayer enabled and 25 composed WGSL programs validated. Source snapshot: `target/tree-detail-source`; build log: `target/tree-detail-snapshot-final-build.log`. The snapshot prevents concurrent edits from changing the validation target.

**All three native capture cases completed successfully**, using one executable, seed 2738958700, 1280×720, fixed camera and lighting: baseline 126 at 0.25 m, refined 127 at 0.25 m, and refined 127 at 1 m. Each has a PNG sequence and `frames.csv` under `target/tree-detail-accepted-review`. The baseline and both final scale views were visually inspected. The refined quarter-metre crown has rougher edges and visible gaps between twig groups; the one-metre view remains visibly cubic, as expected from that voxel grid. This is a close-tree review, not visual acceptance of every species, every model or distant LOD transitions. CPU/GPU load from concurrent builds prevents a clean performance comparison.

Reviewed executable: `target/release-fast/tree-detail-reviewed.exe`, SHA-256 `AAEBB9A2922E69D8E4469C4517D4D5F18FF3BE6BEB65DE5D637B39AD556FD29D`. The mutable shared executable may be replaced by other sessions. Reproduce with `tools/Review-TreeDetail.ps1 -Output target/tree-detail-accepted-review` after building the intended source.

Startup/build repairs needed to reach the scene: fix RON fixed-array delimiters in `assets/industry.ron` without changing values; move the wind update to the window/headless drivers that own the clock; use the existing `StarGlass` material for the cargo window. The snapshot also incorporates the live workspace's removal of an assertion that wrongly required optional editorial spell descriptions. Their catalogue already logs invalid copy and falls back safely; it must not abort gameplay.

No unit tests or hosted CI were added or run for this pass. Save loading and multiplayer generator acceptance have source/build verification for the frozen snapshot, not a save round trip or two-client runtime acceptance. The final live-tree 137 compatibility additions have source review only; the moving shared workspace as a whole is not claimed to match the frozen binary.
