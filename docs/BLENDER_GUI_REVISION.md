# Wayfarer direct Blender revision

2026-10-26. This is an isolated artist revision, not an accepted runtime asset.

## Source and evidence

- `assets/source/blender/wayfarer_artist_r1.blend`: saved from the live Blender
  5.0.1 interface. Based on the concurrently authored Wayfarer assembly.
- The sleeve was selected in Edit Mode, inspected with X-ray, and its lower
  section moved -15 mm in X and +18 mm in Y. The fitted cuff stayed in place.
  This removes approximately 23 mm of apparent sleeve length.
- `target/wayfarer-artist-r1/player.png`: render from the saved edited scene.
- `target/wayfarer-artist-r1/export_manifest.json`: source and export SHA-256s.
- Both exported GLBs passed `viewmodel_probe`: 19 joints, nine clips,
  embedded texture, indexed triangles, normalized weights, UVs and normals.
  Wand: 868 triangles. Arm: 3,578 triangles.

## Preserve artist edits

Use `tools/blender/export_saved_viewmodel.py` to export an existing source:

```powershell
& 'C:\Program Files\Blender Foundation\Blender 5.0\blender.exe' `
  --background --python-exit-code 1 `
  --python tools/blender/export_saved_viewmodel.py -- `
  assets/source/blender/wayfarer_artist_r1.blend target/wayfarer-artist-r1
```

The exporter reads the saved geometry, materials, weights and actions. It does
not regenerate meshes or write the source document. It checks the source hash
after export. Meshes are tagged `part=wand` or `part=arm`, sharing one armature.
The current Wayfarer runtime retains Blender coordinates, with Y along the
wand; do not enable glTF Y-up conversion without updating the runtime contract.

## Art review: still fails acceptance

The crystal is volumetric and its retaining supports make the head more legible
than the old fork. However, the thumb base still has a visible junction, the
four finger gaps are too regular, and the cloth reads as a smooth tube. The
shortening alone does not solve the sleeve. Skin needs continuous joint topology
and deliberate knuckle/thenar planes. Cloth needs a small number of asymmetric
compression folds near the cuff, with texture clusters following those folds.

This isolated revision has not been integrated or checked in the game. The
shared canonical assets and renderer are being modified in another active chat;
coordinate ownership before promoting this source or replacing their outputs.
