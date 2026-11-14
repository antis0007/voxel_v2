# Hand and sleeve restart

The owner rejected the 2026-10-25 hand as stiff and blocky, and the sleeve as too
long and straight. The new Blender hand and sleeve start from fresh construction.
The style remains Minecraft modded fantasy with workshop cloth, timber, brass
and jade. World generation, the palette of the environment and movement physics
are outside this increment.

## Geometry and paint

The palm is an angled wedge with a narrower wrist. Four closed finger chains
have staggered knuckles and pads angled down the grip. The thumb crosses those
pads from a defined saddle. Exact unions join the authored planes; a block
remesher no longer turns the skin into rows of stair steps.

The sleeve follows a bent centreline, with gathered folds near a short turned
cuff. Pixel paint is stored as corner colours in Blender and exported to the
same RON mesh used in-game. The skin uses 8 mm paint cells and the cloth 12 mm;
quiet same-colour coplanar areas are merged. There is no random skin noise.
The existing wand retains its broad voxel construction and material hierarchy.

The source remains editable in `assets/viewmodel/source/artificer.blend` with
named source components, pixel colour layers and a weighted finger armature.
The previous source and meshes are preserved under
`target/artificer-restart-20261026/before`.

## Native iteration

The first native pass exposed a renderer problem hidden by the Blender studio:
the old wrist-to-elbow distance stretched the entire sleeve along one axis.
That elongated the cuff and erased its folds. Sleeve scale is now uniform; the
camera-space elbow guides orientation only.

The second pass retained the cloth shape but exposed the far cut end during
inspection. The third pass directs the forearm below the frame and adjusts its
lower extent. The rest pose shows less forearm. Hand, sleeve and wand continue
to use the existing shared lighting, projection and animation state.

Blender receipts, build logs and frozen executables are in
`target/artificer-restart-20261026`. Native run manifests, screenshots and process
receipts are in `target/artificer-20261025/restart-20261026-*`. Each iteration has
its own label; earlier evidence is retained.

The default authored assembly has 10,247 triangles, versus 10,993 in the rejected
pass. This is geometry accounting, not a whole-game frame-time claim. The native
review covers the default grip; other part combinations retain focused geometry
checks. CPU vertex transformation and streaming remain in place. Captured camera
motion does not establish player-controlled movement feel or reference-game parity.

## Verified candidate

`target/artificer-restart-20261026/restart-r3.exe` is the reviewed build, SHA-256
`4A3CB8D80EBCF678DE42254A736646DA31631CB4F8FF2E87E934B8C171BA7C1C`.
The local release build passed, 19 composed WGSL programs validated, and all five
focused viewmodel checks passed. Warm draw recorded zero allocations; its timing
was 0.234 ms in this run, with no matched whole-game comparison claimed.

Ten successful native reviews cover rest, inspection, aim, windup, release,
interaction, torchlit hall, night, 640 x 360 and the camera motion sequence.
The 50-frame sequence is also encoded as `motion-review.mp4` beside the build;
it is a 12.5 fps review capture. `final-receipt.json` records source hashes and
each native process receipt. One night launch exited before renderer startup
with an empty error log; the identical executable succeeded on retry.
