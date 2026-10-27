# Blender viewmodel interchange

Use `gltf` 1.4 with `utils` and `names` to validate and read Blender's GLB 2.0
exports, including indexed meshes, skin joints, inverse bind matrices and
animation channels. The application supplies embedded buffer and texture data;
the import/image feature is unnecessary because the renderer already uses the
`image` crate. This avoids a bespoke GLB parser. Editable Blender scenes are the
source of truth; export scripts must not regenerate or overwrite artist edits.
