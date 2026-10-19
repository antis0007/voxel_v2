# Distant terrain visual review

This receipt describes the frozen `horizon-final.exe`. The subsequent screenshot
repair removes terrain and backdrop replacement dithering: current source keeps
opaque coverage and swaps the backdrop atomically. Its visual evidence is
recorded separately in `GRAPHICS_WORLD_REVIEW.md`.

Reviewed on 2026-10-18 in `target/release/horizon-final.exe`.

The original captures exposed interpolated material IDs producing bright bands,
angular coastlines, and complementary voxel/backdrop dithering leaving holes.
The revised backdrop has 45,058 triangles, independent flat water, continuous
land colours, world-anchored samples, matching boundary heights/normals/colours,
distance haze and filtered surface variation. Backdrop coverage remains underneath
fading voxels. Rebuilt meshes crossfade for 1.5 seconds; a bounded sample cache
reuses terrain columns between builds.

Run from the repository directory:

```powershell
.\target\release\horizon-final.exe --benchmark-horizon-sweep --benchmark-capture-frames --benchmark-weather=clear --benchmark-width=1280 --benchmark-height=720 --benchmark-warmup=20 --benchmark-duration=100
```

The camera holds for 20 seconds, then travels at 12 m/s and turns once per minute.
Final run covered 1.2 km and more than a full rotation; 32 captures are preserved
in `target/horizon-review/final`. Inspected north/east/south/west views and close
frame pairs near the first recenter. Bright bands and open-water pinholes were
absent in those final captures. Initial and first-refinement captures are kept in
the adjacent `before` and `first-refinement` directories.

`target/horizon-final-run.log` records the run; stderr in
`target/horizon-final-error.log` was empty. Its legacy cloud-review phase labels
say “16 km” after phase 6, but the logged camera positions describe this sweep:
it does not travel 16 km. Release compilation succeeded. A one-line closure fix
in `src/mesh.rs` resolved a concurrent compile blocker (`quad.map(Vec3::to_array)`).

Measured on RTX 4090/Vulkan at 1280x720: 131.60 average FPS, 28.872 ms p99,
585.856 ms worst frame. Captures and concurrent desktop work were active;
these figures do not establish hitch-free performance or a speedup.
First backdrop generation took 17.846 s; subsequent builds took 854 and 992 ms.

Remaining limits: initial cold-build delay, visible foliage dithering at the
voxel boundary, and no distant player edits, structures, or tree geometry.
Visual acceptance here covers the clear-noon Hills seed 2738958700 at one-metre
voxel scale, not every weather preset, terrain seed, altitude, or scale.
