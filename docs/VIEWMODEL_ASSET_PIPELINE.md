# Wayfarer first-person assets

The active wand and arm come from Blender GLBs. The old `assets/viewmodel/*.ron`
geometry remains as historical/reference data; it is not selected by the held-wand
renderer. Equipment progression, spell slots, mana and cast timing remain owned by
the existing gameplay systems. This pass intentionally provides one hero silhouette.

## Reproduce

Run from the repository root in PowerShell:

```powershell
& 'C:\Program Files\Blender Foundation\Blender 5.0\blender.exe' --background --python tools/blender/export_viewmodels.py
& 'C:\Program Files\Blender Foundation\Blender 5.0\blender.exe' --background --python tools/blender/verify_viewmodels.py
cargo test -p minecraft_style_voxel_prototype --profile release-fast viewmodel -- --nocapture
cargo test -p minecraft_style_voxel_prototype --profile release-fast blender_glbs_have_registered_skin_and_authored_clips -- --nocapture
cargo build -p minecraft_style_voxel_prototype --profile release-fast
```

Blender 5.0.1 was executed during production. Both source files in
`assets/source/blender/` contain the assembled wand, hand, armature, actions,
packed atlas, camera and lights so that grip registration can be edited in context.
The exporter selects the wand or arm separately for the two runtime GLBs. Running
the authoring script rebuilds its generated sources; preserve manual edits in a
separate source revision before rerunning it.

## Geometry and materials

The 41.6 cm focus has a tapered heartwood spine, a shaped 30 mm leather grip,
brass ferrules, a ceramic seat, three bent retaining claws and a seven-sided
turquoise crystal. Geometry uses designed elliptical cross sections and centre
lines. Palm, thenar and wrist volumes are joined in Blender; fingers remain separate
deforming surfaces to prevent accidental welding across the closed grip.

The atlas is 256 by 256, with eight reserved material regions. It contains connected
wood grain, wrap seams, broad skin tones, cloth folds and restrained metal/crystal
planes. It uses nearest sampling and sRGB decoding. The PNG is packed into both
GLBs and source files; the identical RGBA copy uploads once through the native
material pipeline. No texture generation occurs in the frame loop.

## Rig and interchange

There are 19 joints: root, forearm, wrist, shared grip socket, twelve finger joints,
and three thumb joints. At most four normalized weights influence a vertex.
The nine Blender actions are `idle`, `equip`, `windup`, `release`, `recovery`,
`inspect`, `open`, `strong_grip`, and `index_extend`. The runtime selects the first
seven from player state; the last two remain authored pose-library actions.

GLBs are exported by Blender's standard glTF exporter with metres and **Z-up
retained** (`export_yup=False`) to match the established local grip coordinates:
the shaft points along +Y. They are not general-purpose Y-up scene assets.
Both meshes bind to the identical armature and grip reference. The Rust `gltf`
parser reads actual GLB positions, normals, UVs, indices, joints, weights, inverse
binds and clip channels; no hand-written substitute mesh is used. Independent
Blender re-import checks the exported rig and actions.

## Runtime and framing

`mesh/models/wayfarer.rs` decodes each embedded asset once. It caches indexed source
vertices and animation channels, samples compact fixed-size transform arrays, skins
each unique vertex once, then emits the existing scene stream. Scratch storage and
the GPU dynamic vertex buffer retain capacity. There is no per-frame tessellation,
GLB parsing, texture creation or GPU buffer recreation after capacity warmup.
This is bounded CPU skinning, not a GPU skinning implementation. The scene stream
still uploads posed vertices every frame.

`assets/runtime/viewmodels/presentation.ron` controls scale, dominant hand, held
FOV, offsets, axis/roll and pose offsets. Default held FOV is 72 degrees and is
independent of sprint FOV. Mirroring reverses triangle winding. Existing reverse-Z
viewmodel depth treatment keeps the arm from being sliced by nearby world surfaces.
Spell light and ornaments use the animated grip socket.
The forearm joint uses a wrist-preserving elbow constraint so inspect poses do not
reveal the cut end of the sleeve. Its normal transform accounts for axial stretch;
finger and grip transforms remain exactly those sampled from the authored clips.

## Evidence and limits

Blender renders and round-trip receipts are under `target/wayfarer-review/`.
Native captures and executable hashes are under
`target/artificer-20261025/wayfarer-*`. They are actual game captures, not Blender
images presented as gameplay. `wayfarer-before` is the previous native studio view.

The first warm CPU measurement was 26.374 microseconds for the old viewmodel and
125.022 microseconds for the initial replacement. Caching unique skinned vertices
reduced the final replacement to 67.068 microseconds (13,986 emitted vertices,
3,255 unique skinned vertices). All measured warm paths allocated zero times.
These are component measurements,
not claims about overall FPS or latency. Capture runs include readback overhead.

A serial native run without capture readbacks measured 2.558 ms average frame time
before and 2.583 ms after (1280 by 720, RTX 4090 Vulkan, 8-second warmup and sample).
Those runs had different resident terrain draw sets (246 versus 233 draws), so
they are retained as observations, **not an isolated GPU regression verdict**.
See `wayfarer-perf-before` and `wayfarer-perf-after` for full manifests and logs.

Final focused checks: six viewmodel tests pass, including pose bounds, finite unit
normals, warm allocation counts, and matching hand/weapon sockets across casting.
The separate GLB import/clip gate also passes. The optimized native build validates
19 composed WGSL programs. Blender round-trip receipts confirm 19 joints and nine
actions in each GLB. Fifteen Blender review images include clay, textured neutral,
torch light, opposing grip contacts and articulated poses.

Native evidence includes `wayfarer-final-outdoors`, `wayfarer-final-hall`, and the
`wayfarer-native-*` studio/inspect/casting/workshop/night views. Each capture folder
records the actual executable hash and command-line/environment settings. Failed
intermediate runs remain in their own folders and are not acceptance evidence.
`wayfarer-motion` contains 75 native frames spanning casting, inspection and aiming.
The later `wayfarer-cuff-*` views verify the final elbow/cuff constraint refinement.

Close-up limitations: the thumb base and knuckle silhouette remain deliberately
faceted; the arm is a compact stylized asset, not a photoreal character hand.
Finger surfaces meet the palm with overlapping roots rather than a production
retopologized continuous animation mesh. The entire brief's subjective professional
art-quality gate should not be inferred from export, build or test success.
