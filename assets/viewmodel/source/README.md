# Held kit Blender source

Current direction: Minecraft modded style. The hand and sleeve were restarted
on 2026-10-26 after the stepped hand and long straight sleeve were rejected.

Open `artificer.blend` in Blender 5.0. `SOURCE` contains named editable palm,
finger, thumb, cloth and wand parts. Its child collections are hidden initially.
`GAME` contains the baked meshes and a weighted five-finger armature;
the default kit is assembled for viewing. Reveal `Hand grip skeleton` to pose it.
`STUDIO` contains the lights and camera. Exported RON coordinates stay local;
the game supplies wrist articulation, cast motion and head placement from
`assets/viewmodel/rig.ron`. Hold **I** in-game to inspect the equipped wand;
movement and casting cancel inspection. Existing interaction/cast keys remain.

Rebuild from the repository root:

```powershell
& 'C:\Program Files\Blender Foundation\Blender 5.0\blender.exe' --background --factory-startup --python tools/blender_artificer/build.py
```

This regenerates the Blender file, all fourteen RON parts and a studio render
under `target/artificer-restart-20261026`. Set `VOXEL_ART_EVIDENCE` to a unique
repository-relative directory for each iteration. It overwrites authoring outputs;
preserve manual Blender edits before rebuilding from the scripts.

The hand uses a sloping palm, staggered closed finger chains and a thumb crossing
the pads. Exact Blender unions retain those planes; it is not block-remeshed.
The sleeve has a bent centreline and gathered folds with a short linen cuff.
The `Pixel paint` corner colour attribute contains the same restrained pigment
blocks exported to the engine. Flat planes share world lighting. The renderer
keeps cloth scale uniform instead of stretching it to a camera-space elbow.
The timber body, brass setting and jade focusing stone retain their voxel shapes.
No microscopic skin, thread or engraved surface detail belongs here.
