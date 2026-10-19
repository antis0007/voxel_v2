# Atmosphere and first-person art refinement

## Diagnosed causes

- `src/shaders/cloud_morphology.wgsl::cumulus_envelope`: the main thermal
  ellipsoid was centred on the condensation floor, so its lower half extended
  outside the cloud layer. The final altitude mask cut it into a flat shelf.
  Strong positive noise inflation made this worse. Cores and shoulders now
  close above their local floor; cell-scale height offsets and restrained noise
  preserve rounded silhouettes. Stratiform weather retains its own deck mask.
- `src/shaders/atmosphere_shared.wgsl::runtime_sky_view_atmosphere_color`:
  the planetary LUT's downward ground intersection formed a dark rim behind
  the local landscape. A smooth continuation of tangent sky radiance now covers
  the shallow lower hemisphere, shared by the sky and terrain fog. The deep
  lower hemisphere is retained. `world_shader.wgsl` strengthens distant
  extinction at twilight without changing nearby surface contrast.
- `src/render/horizon.rs::build`: distant trees were smooth-normal octahedra,
  producing pointed diamonds. They now have trunks and stepped, overlapping
  voxel crowns, with separate broadleaf and conifer silhouettes. Their base
  follows the local slope when their position is jittered. `proxy_box` uses
  flat face normals. The shader keeps these proxies beyond 512 metres or
  1.15 times the detailed-terrain radius, whichever is larger.
- `src/egui_ui.rs::draw_held_tool_overlay`: the hand used a large plain arm box
  and bright blue sleeve. It now has a compact expedition glove, distinct
  thumb and knuckles, leather seams, a copper cuff buckle and a muted teal
  sleeve. Its placement preserves the sight line and hotbar; its colour follows
  the day/night cycle. This is still a projected UI model, not a world-lit mesh.

## Runtime evidence

First release: `target/art-refinement/v1/voxel-art.exe`, with successful composed
WGSL validation. Inspected fresh 1600x900 noon and moving dusk captures, Hills
seed 2738958700, partly cloudy. Both runs exited with empty error logs.

The dusk view replaces the dark horizon stripe with layered atmospheric depth.
The distant trees have broadleaf/conifer silhouettes rather than diamonds.
The first glove pass exposed a projected-polygon stroke miter defect: thin,
edge-on seams generated long lines into the scene. Solid faces now use explicit
triangles without polygon strokes. The glove was also reduced another 15 percent
and rotated to expose its thumb and stitched palm. The first cloud pass removed
the clipped undersides but retained squat shapes; narrower thermal radii and a
taller height distribution are now awaiting the second capture review.

The runtime screenshots are under `v1/noon/hills-detail/None/` and
`v1/dusk/horizon-review/None/`. The dusk filename says `noon` because that is the
route label; the log records the actual solar time of 0.72. These are visual
reviews, not broad performance acceptance. No automated tests or CI were run.

## Reproducible second candidate

`target/art-refinement/v2/source` is a copy of the successfully profiled
`target/measured-review/candidate-source`, with the three atmosphere/world/cloud
shader files and the held-hand rendering function updated. It retains cubic
terrain, scale-aware tools, the faster backdrop sampler and the new proxy trees.
This isolates visual review from concurrent unfinished simulation changes.
The release executable and WGSL validation built successfully; binary and source
hash receipts are stored in `target/art-refinement/v2`. Later live-source cave,
torch and gameplay changes are outside that frozen build and remain untouched.

## Final visual review

The second release was run and inspected at noon and during a moving dusk
flyover (solar time 0.72), at 1600x900 with the same Hills seed. Both processes
exited successfully and both error logs were empty. Reviewed frames:

- `target/art-refinement/v2/noon/hills-detail/None/Hills-01-PartlyCloudy-noon-3.png`
- `target/art-refinement/v2/dusk/horizon-review/None/Hills-01-PartlyCloudy-noon-3.png`

The cumulus volumes are taller and have rounded, separated undersides instead
of the previous broad clipped shelves. The dusk horizon has layered haze with
no dark horizontal rim in the reviewed view. Far trees show stepped crowns and
trunks rather than diamonds. The glove occupies less of the view, shows its
thumb/knuckles/palm stitching, darkens at dusk, and has no stray seam lines.

`run-atmosphere-repair.cmd` launches this inspected snapshot. Acceptance is
limited to these outdoor views: the glove still uses approximate day/night
lighting, far trees remain simplified proxies, and other weather presets were
not part of this final capture pair. No performance improvement is claimed from
these art captures; the tree proxies increase backdrop geometry.
