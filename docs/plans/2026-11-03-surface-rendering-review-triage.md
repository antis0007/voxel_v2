# Surface rendering review triage, 3 November 2026

Source: the pasted "harsh verdict" surface review (eight sections, screenshot not
attached). This file maps each claim to code and to captures, so the next session
starts from measurements. Renderer code is unchanged.

## Which frame

The review describes an elevated oblique view: mountains, woodland as fine green
stipple, bright angular snow fragments, dark branching valley lines, cloudy patches,
an amber cast, a pale hand. The pose was not supplied. Two candidate poses were
captured with the release-fast exe of 2026-11-03 11:23, seed 2738958700:

| Label | Flags | Match |
|---|---|---|
| `vista-golden` | hills, `--benchmark-terrain-camera=96,0,45,3.46,-0.10`, time 0.715 | No: voxel foreground, no stipple, no fractures |
| `planet-600-sunset` | planet, `0,0,600,0.6,-0.55`, time 0.72 | No: one-pixel grass noise, snow blobs |
| `planet-3000-sunset` | planet, `0,0,3000,0.0,-0.5`, time 0.75, partly | Closest: clouds, snow facets, stipple, valley lines |

Evidence: `target/reviews/visual-audit-20261103/<label>/<view>/bench-001.png`.
Runner: `audit-views.ps1` in the session scratchpad (same flags, `-Views`, `-World`,
`-Camera`, `-ReviewTime`).

## Finding 1: the diagnostic views do not reach the planet ground

Views `albedo`, `geometric-normal`, `direct-light`, `ambient-diffuse`, `ao`,
`material-id`, `terrain-shadow` and `sky-access` recoloured only the hand and torch
at the 3000 m pose. The ground kept its final shading. Only `unfogged` changed the
terrain (it removes clouds and haze in the composite).

Cause: at that altitude the visible ground is the globe tile renderer
(`shaders/globe_shader.wgsl`, `render/globe/`), which has no `render_debug_view()`
branch. The debug switch lives in `world_shader.wgsl` (`fs_main`, debug modes 11 to
33) and covers resident chunks and the horizon backdrop only.

Pass 1 of the review (neutral grey, unlit albedo, direct only, sky only, AO only, fog
transmittance, material ids, mip colours) is therefore not runnable on the frame the
review is about until the globe shader takes the same uniform. The globe shading
model is small: per-material flat albedo (`globe_albedo`), N dot L sun with cloud
shell shadow, an air-path haze term. Candidate first debug views for it: material
id, vertex normal, N dot L, haze transmittance.

## Finding 2: the globe path has no vegetation representation

`globe_albedo` paints a forest material as a flat colour per height texel, so
woodland reads as green stipple with no canopy height, shadow or clearings. The
horizon tree meshes (`render/horizon/vegetation.rs`, 12-vertex crowns, byte budget
with `omitted` count) stop at the backdrop range. The review's section 7 (grouped
far canopy, coverage and height preserved) is a new stage between the two, not a
tuning of either.

## Claims that the code already answers

| Review claim | Code | Verdict |
|---|---|---|
| Possible gamma error | Scene target `Rgba16Float` (`render/postprocess.rs`), tonemap in `postprocess.wgsl`, one `srgb_encode` in `present.wgsl` with 8-bit dither | Linear pipeline, single encode. Debug views skip the grade (`params.resolve.w`). No gamma defect found |
| AO multiplies the whole result | `world_shader.wgsl` ambient block: `cavity` (floor 0.42, `pow(raw_ao, 1.35)`) and `macro_occlusion` scale ambient only; direct sun is separate | Direct light is not occluded by AO. Ambient floor and curve are tunable |
| Face orientation decides cover | `natural_surface_blended` takes `macro_normal`; strata and steep rock use `macro_normal.y` | Already macro-normal driven for natural cover |
| Fracture darkening dominates rock | `rock_surface`: fracture darkens 0.32, gated by `texel_w` (fades with footprint); far strata ledges darken 0.20 | Near rock only; neither reaches the globe path. A/B of 0.32 is a one-constant change |
| Amber cast | Distance veil: hue weight 0.55, brightness 0.15 toward the dome radiance (553670db, 2026-10-27 triage) | Sunset cast at 0.75 persists on the globe path through its own haze term, not the veil |
| Hand composited over the world | Viewmodel gets its own neutral fill, no fog, 0.20 sky access for authored kits | By design; shared sun direction. Hand brightness vs ground is a grading decision |
| Fog model | Non-LUT path integrates height density over the ray; LUT path uses aerial perspective | Both are transmittance plus in-scatter as the review asks. Nine fog paths remain to consolidate (M5) |

## Also seen

`planet-600-sunset`: grass at 600 m is one-pixel noise over the whole frame and
snow lies in flat white blobs. This is the resident and backdrop materials sampled
at a footprint where `surface_detail_weight` should already have faded detail;
check `fp_m` on the planet path before tuning any material.

## Globe debug views (landed in the tree, uncommitted)

`GlobeView.debug_view` carries `RenderDebugView::as_u32` into bits 8-15 of
`GlobeUniform.flags` (struct size unchanged, 128 bytes). `globe_shader.wgsl`
answers the shared ids: 7 haze transmittance, 18 albedo, 19 material id, 20/21
normal, 23 direct, 24 ambient, 26 unfogged; 11/28/29 read as open sky since the
globe bakes no occlusion. Other ids are magenta.

Captures `planet-3000-sunset-globedbg/<view>/` (exe built from a D: snapshot of
the tree at 15:11, since the shared tree did not compile: peer edits in
`engine_content` and `adventure/conductors.rs`):

- `material-id`: one id covers most of the ground; leaves and stone appear as
  patches. Forest is a material texel, not geometry, as finding 2 says.
- `geometric-normal`: nearly uniform up over the plains; relief only in the
  mountain belt at right. The dark valley lines of the final frame are not in
  the normals.
- `direct-light`: N dot L is near 1.0 over the whole ground at review time 0.75
  (sun on the horizon in the sky pass). Lead: the globe sun direction does not
  match the sky's sun at sunset, which would explain midday-bright ground under a
  dusk sky. Unverified; compare `view.sun_dir` with the sky pass at that time.

## Fracture A/B (blocked on a pose)

`peak-noon-fracture-{032,010}/none/`: hills peak pose, noon, fracture darkening
0.32 vs 0.10. Pixel difference 301 of 1.44 M, all inside the hand. The nearest
rock is past the footprint where `texel_w` reaches 0, so the experiment needs a
pose within a few metres of exposed stone. Both exes are under
`visual-audit-20261103/exe/`.

## Next

1. Near-rock pose for the fracture A/B.
2. Confirm or clear the globe sun-direction lead at review time 0.75.
3. Decide owner for a grouped far-canopy stage between horizon trees and globe
   texels.
