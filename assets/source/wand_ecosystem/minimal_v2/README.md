# Minimal wand and hand review — 26 October 2026

These are three comparison assets within the twelve-family ecosystem direction. They do not replace the normal game kit or establish the final art standard.

## Source and reproduction

`ecosystem_items_r2.blend` contains the editable geometry, UVs, packed textures, named functional components, muzzle markers, shared grip rig and inherited actions. The hand uses an authored quad cage with broad palm planes, wrapped finger sections and an opposing thumb. It is not the decimated Quaternius study.

From the repository root, run Blender with `--background --python` and these scripts in order:

1. `tools/blender/author_ecosystem_items.py` creates the preserved first study and original 32px icons.
2. `tools/blender/refine_ecosystem_items.py` creates this second study and compatible GLBs. This regenerates the second source; preserve any subsequent artist edits separately before running it.
3. `tools/blender/capture_ecosystem_items.py` reads the saved second source and renders front, side, three-quarter, 96px silhouette and 72-degree projection views.
4. `tools/pixel_kit/ecosystem_material_masks.py` writes original ID/metal/emission-eligibility masks. These masks are authoring data; the current renderer does not consume them.

The first study's `icon_sources.json` stores integer-grid polygons and palettes. `texture_sources.json` stores this pass's authored marks and hand ramps. No generated concept image was resized into a finished sprite.

## Engine review

`VOXEL_REVIEW_ITEM` accepts only `bone_needle`, `runic_key`, or `cinder_rod`, and only with benchmark arguments. `tools/Review-Artificer.ps1 -Item` selects these through the normal WGPU viewmodel path. The selector changes the review hand too. Normal gameplay continues loading its existing embedded kit.

The review uses a 512×256 atlas: hand on the left, wand on the right. UV remapping occurs once during asset loading. This avoids the first pass's shared-atlas cuff recoloring. The normal 256×256 atlas is unchanged. Review saves and captures use fresh isolated directories.

| Asset | Triangles | Exported vertices | Length |
| --- | ---: | ---: | ---: |
| Bone needle | 152 | 288 | 47.1 cm |
| Runic key | 128 | 240 | 42.3 cm |
| Cinder rod | 132 | 242 | 46.6 cm |
| Hand and sleeve | 848 | 1532 | — |

The hand source has 440 vertices; flat normals and UV seams split exported vertices. All four GLBs pass the existing Rust asset validator with 19 skin joints. The nine actions are inherited compatibility clips, not newly authored or accepted animation.

## Native evidence and critique

`target/artificer-20261025/ecosystem-{bone_needle,runic_key,cinder_rod}-{outdoors,hall}-v3/` contains six native 1280×720 captures, isolated saves, executable hashes and logs. `ecosystem-key-motion-v3/` contains 75 frames of the existing pose cycle. The final cleanup removes duplicate imported actions; it does not change the geometry or textures shown in these captures.

The cleaned final build was captured again in `ecosystem-bone-hall-v4/` and `ecosystem-key-motion-v4/`. Both runs completed; the latter contains 70 frames. Logs confirm nine clips on the selected wand and hand. `target/ecosystem-minimal-v2/artifact-manifest.json` records the source/export/executable hashes; `asset-validation.log` records the four final GLB validations.

- **Bone needle:** strongest simple silhouette and the clearest natural-material identity. The first pass was too long; it was shortened after native review. Painted notches read more like bands than shallow carving and need more deliberate UV placement.
- **Runic key:** recognizable utility focus with a genuine open bow. It is still rather flat from the default camera; the teeth and bow need a more convincing forged cross-section before becoming hero equipment.
- **Cinder rod:** sufficiently distinct from the other two, but the dark shaft and two bright seams are too plain. The seams currently use ordinary albedo. State-driven heat, ember shedding and mixed-spell feedback are not implemented by this pass.
- **Hand:** cleaner planes and clearer grip contact than the previous surface, with less isolated finger-tip geometry. Fingers still read as parallel bars at some angles. Inspect exposes the long straight sleeve and should not be considered accepted.
- **Presentation:** sampled idle/inspect/aim frames keep the item attached and do not show near-plane clipping. This is not a complete walk/sprint/rapid-fire/continuous-cast acceptance run. Projectile origin/collision behavior was not changed or claimed verified; only the held light marker follows the authored candidate muzzle.

No shader validation errors or crashes were reported in the six still runs or pose-cycle run. Build and asset validation passed. GPU performance is unmeasured: captures use readbacks and other work was active on the machine. The lower triangle count is not an FPS claim.

Blender 5.0.1 executed the production scripts and saved the source files. Desktop activity interrupted this turn's GUI inspection; the user explicitly directed continuation through Blender's Python API. Earlier GUI edits belong to the Copper study, not these assets.

The generated twelve-family and botanical concept sheets remain visual references. Sprite/proxy comparison, dropped-item identity, spell programming/discovery integration, material-mask shaders and new casting animation remain separate production work.
