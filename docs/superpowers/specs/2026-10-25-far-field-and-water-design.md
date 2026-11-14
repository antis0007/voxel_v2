# Far field and water: what Mycopunk's approach transfers

Owner request, 2026-10-25: "take ideas from Mycopunk to improve performance and
appearance at a distance. Water can look really bad even up close." This is an
addendum to the brief [VARIABLE_SCALE_UNDERGROUND.md](../../specs/VARIABLE_SCALE_UNDERGROUND.md)
(Part 1.1 coarse row, Part 1.3 far field, Part 3). The brief stays byte-identical;
its hash is recorded in `docs/specs/README.md`.

Status: design for review. Not an input of record (decision 0022 froze intake;
decision 0023 is the route if the owner adopts it). Master-plan rows touched:
R05 water, R10 far LOD, R12 post. Nothing here changes a generator gate, a save
or a frame until a slice is approved and captured before and after.

Ownership note: a peer session owns water shading on the `visual-*` branches
(memory, 2026-10-25). Slice W1 below must be agreed with that owner first.

## 1. What Mycopunk does

Mycopunk (Pigeons at Play, 1.0 on 2026-10-20) is a marching-cubes co-op shooter.
Documented facts:

| Fact | Source |
|---|---|
| Levels start as a voxel grid and are smoothed with marching cubes. Terrain is built in layers; each layer runs a per-voxel function returning the distance to the nearest surface. Terrain math runs in compute shaders. Generation still takes a while. | Devlog 1 May 2026 (SteamDB mirror) |
| A Render Distance option disables far chunks in procedural levels, with a large gain. Default infinite. | Patch 1.8.1 notes |
| Particle systems use generic shapes fitted around models instead of exact model data, to free VRAM and reduce hitching. | Devlog 1 May 2026 |
| Texture streaming budget (default 2048 MB, 4096 MB when VRAM is unknown). Grass rendering optimised for 1.0. | 1.0 notes |
| Cel-shaded with grungy detailing. Oil paintings shaped the rendering techniques. | CGMagazine review; Space.com interview |

My reading of the 16 store screenshots (2026-10-25, not developer statements):

1. A thin dark ink line sits on every depth and normal edge. It thins with
   distance but never vanishes; far ridges and mushrooms keep their line.
2. Detail collapses in three stages: ink-lined texture near, sparse colour
   patches in the same palette at mid range, one flat tinted mass far.
3. Aerial perspective shifts far masses toward the sky's hue (ochre, violet,
   teal), with saturation kept, not toward grey.
4. Sky and fog gradients carry fine grain; surfaces do not band.
5. Water is a flat body colour with 3 to 6 long white strokes per screen, a
   thin bright rim at the shore, and a soft blurred sky reflection. No dense
   sparkle, no visible refraction detail.
6. The far field is hidden behind atmosphere more than it is drawn: mountains
   at 1 km are 60 to 80% fog colour.

What does not transfer: cel banding (decision: no posterization), grain on
surfaces (one dither only, `PIXEL_ART_DIRECTION.md`), and its scale. A Mycopunk
level is a few hundred metres with an infinite draw distance; our horizon reaches
24,576 m and already has rings, proxies and a sea sheet.

## 2. Where we stand (2026-10-25)

| Item | State |
|---|---|
| Resident ring | 32³ chunks, greedy CPU mesh, LOD 0..3 by chunk distance (`streaming/visibility.rs`). 28 chunks at 1 m is 896 m. |
| Backdrop start | `min(published, visible)` or 0.55 × visible; overlap `max(120 m, 6% visible)`. |
| Far rings | Heightfield from `distant_surface` to 24,576 m; 8 m fine grid inside the 3,072 m square; 64 m cube columns to 6,144 m, opt-in (`VOXEL_HORIZON_CUBES=1`) and judged wrong beside smooth terrain; tree proxies; one water sheet at sea level only. |
| Agreement | ADR 0012: 8 m interpolation p95 1.75 voxels, 17 to 19 m at 64 m; divergence 93% explained by features. |
| Build | Horizon build about 24 s, 14 s of it in columns/fine/reference apply (memory). |
| GPU, 1080p, 28 chunks, 4090 | Terrain 2.9 to 4.9 ms, shadows 6.3 to 7.8 ms with 50 to 105 ms refresh hitches (1,000 to 5,500 shadow draws), clouds 7.1 to 7.7 ms, 1,020 draws. |
| iGPU (UHD 770) | Terrain 29.9 ms with the lean tier; fog and aerial 11 ms; clouds unusable. |
| Water pass | Opaque, in the terrain pass. Bed colour is a vertex attribute; depth comes from the mesher pack; no scene depth or colour is bound. |
| Water shading | Gerstner plus fbm normals, GGX glint, Beer-Lambert on a vertex bed, foam from interpolated pinned-corner contact, mint edge. Rivers have every corner pinned, so no vertex waves. |
| Postprocess | Edge term is a block-scale luminance-contrast contour (`style.w`); SSAO reads depth; no normal target. |

Captures inspected: `fluids-pass-20261024/after/water-review-final.png` (pools:
foam lace covers roughly a third of the near pool as white sheets, body is flat
opaque teal, bed invisible, walls flat), `rivers-fit-gen53-eye` (river reads as a
grey slab with hard stepped banks and no shore transition),
`rivers-after-overview` (rivers vanish past residency, sea is one flat tint).

## 3. Goals, non-goals, anti-goals

Goals: far masses that read as the same world as the near voxels (silhouette,
water bodies, biome colour) at lower build and shadow cost than today; water
that reads as liquid at every distance, with a visible bed in the shallows and a
thin shore line.

Non-goals: simulated far water, a new generator version for appearance, a
change to the pixel direction or the palette, cloud cost (own track).

Anti-goals: banded shading, 64 m towers, foam sheets, noise shimmer, speed
gained by dropping geometry (`.claude/rules/render.md`).

## 4. Far field

### F1. Silhouette ink that persists

Replace the contrast contour with a depth-and-normal edge: `|Δdepth| / depth`
over a 1-block kernel at the art scale, plus the normal dot product. Width is
one art pixel at every distance. Strength follows aerial transmittance T but is
floored: `line = style.w × max(T, 0.35)`. Needs a normal target or depth
reconstruction in `postprocess.wgsl` (render contract change, both sides in one
patch). Motion captures must show no crawl on static edges.

### F2. Detail collapse as a contract

| Stage | Range (1 m voxels) | Surface | Lighting |
|---|---|---|---|
| Detail | 0 to 60 m | `PIXEL_RES` lattice, dither, micro-noise | full |
| Patch | 60 m to backdrop start | `surface_detail_weight` fade already does this; add family-colour patches of 2 to 4 m from a hashed field | full, torches culled by distance |
| Mass | beyond backdrop start | one colour per coarse cell from the coarse field's material fraction | sun and sky lobes only |

Patch and mass boundaries scale with `cam.world_params.x` like everything else.

### F3. Atmosphere

Keep the physical aerial perspective. Add a per-biome far tint from the climate
field already sampled by worldgen (temperature, moisture): far masses mix toward
`sky_probe` colour weighted by the biome tint, saturation preserved (Mycopunk
reading 3). Grain stays where it is now: dither on smooth gradients only.

### F4. Coarse field, the layer stack

Mycopunk's layered distance functions map onto our pure worldgen. Add
`engine_worldgen::coarse_field(cfg, cell, level)` for 2, 4, 8 and 16 m cells
returning: signed distance to the surface, majority material family, water level
(sea, lake planner, drainage reach of width ≥ 4 m), and tree cover fraction.
It samples the same planners the chunks use, through the same caches (memory:
plan cache re-entrancy). A pure read: outputs change only with a generator gate.

Rings replace the heightfield inside 6,144 m. Cell size is the outer radius over
384, so a cell is at most 3.6 px at its outer edge (1440p, 60° vertical field of
view assumed):

| Ring | Outer radius | Cell | Mesh |
|---|---|---|---|
| 1 | 1,536 m | 4 m (2 m inside 512 m when the budget allows) | blocky voxel tiles, majority material |
| 2 | 3,072 m | 8 m | surface nets, smooth |
| 3 | 6,144 m | 16 m | surface nets, smooth |
| 4 | 24,576 m | heightfield | unchanged |

Ring 1 blocky is the ADR 0012 recommendation (tiles from the same data as the
chunks). Whether blocky or smooth is right beside 1 m voxels is the owner's call
from a same-pose capture; the design defaults to blocky inside 768 m.

Overhangs, cave mouths, lake basins and river channels survive in rings 1 to 3;
a heightfield cannot carry them.

### F5. GPU meshing of coarse tiles

The CPU samples the field (64 times fewer samples than 1 m at 4 m cells); a
WGSL compute pass meshes a tile (greedy quads for ring 1, surface nets for
rings 2 and 3) into a vertex buffer. The `gpu` crate's shared adapter tests
CPU and GPU meshers for identical output, as `sim_gpu` does. Target: the full
ring set builds in under 4 s on H1 and recentres without a frame over 4 ms.

### F6. Proxies

Mycopunk's generic shapes, extended from the tree proxies we have: sites,
homesteads, dungeon entrances and anchor districts as proxy boxes with the
plan's roof and wall families beyond the backdrop start; mobs beyond 96 m as 1
to 3 boxes; particles outside residency collide with the coarse field.

### F7. Far cost

- A far pipeline variant for rings 1 to 3 (the `LEAN_SHADING` override pattern):
  sun and sky lobes, no torch loop, no caustics, no micro-noise.
- Shadow cascades 2 and 3 draw coarse tiles only and refresh every 4th frame;
  this is the hitch source in the 2026-10-22 baseline.
- Budget: rings 1 to 4 together at or under 1.5 ms GPU at 1440p on the 4090
  reference and 6 ms on the UHD 770; shadow draws at a cascade refresh under 600.

## 5. Water

### W1. Own pass with scene depth

Split water faces into their own index range and draw them after the opaque
pass with the opaque depth copied and bound. Per-pixel depth is then scene depth
behind minus surface depth, so shallows fade to the bed, films read as films,
and the vertex bed colour stops acting as paint. Full tier: copy the scene
colour at half resolution before the pass and refract it. Lean tier: depth fade
only, bed from the vertex colour as today. The water pass stays in HDR before
`postprocess.wgsl`.

### W2. Shore line and strokes (Mycopunk reading 5)

- Shore line: one `PIXEL_RES` texel where per-pixel depth is under 0.06 m.
- Second line at 0.25 to 0.35 m, broken to 30 to 50% coverage, advancing and
  retreating on a 4 to 6 s period. These two lines replace the lace mask.
- Open water: 2 to 4 strokes per 10 m², 1 to 3 m long, one texel wide, 2 to
  3 s life, a pure function of position and the clock.
- Flow steps keep a 2 to 3 texel run-off line at the step only.
- Coverage cap: foam and stroke pixels at or under 8% of water pixels in the
  fluids fixture (the current capture is near a third on the closest pool).

### W3. Body and bed

Keep `water_body_color` and `WATER_ABSORB`, driven by the per-pixel depth from
W1. A 0.05 m film shows at least 80% bed colour; 2 m shows at most 10%. The
0.70 wet-bed darkening is the builder's to retune under that test.

### W4. Reflection

Soft sky reflection from the probe at roughness 0.10 to 0.15 and one soft sun
lobe. The dense glint sparkle goes. Screen-space reflection at half resolution,
16 steps, is a separate optional slice for the full tier, off on lean.

### W5. Rivers

Pack flow direction (8 directions plus speed, one byte) into the water vertex.
Ripples scroll along flow at 0.4 to 1.0 m/s as normal detail; the Gerstner set
runs only on open water with fetch (`surf_fade`). Pinned corners keep their
height pinned and keep their normal detail; today pinning zeroes both.

### W6. Far water

Horizon water sheets per coarse cell at the cell's water level (sea, lakes from
generator 53, reaches of width ≥ 4 m), clipped to the coarse bed. The existing
`is_far` shading branch takes depth from the coarse field. Rivers stay visible
at 300 m in the overview pose.

Budget for W1 to W6: at or under 0.4 ms added at 1440p on the 4090, 1.5 ms on
the UHD 770 (lean).

## 6. Acceptance

| Check | Where |
|---|---|
| `coarse_field_matches_columns` | worldgen: for 4,096 columns in 4 quadrants with negative and large coordinates, the 4 m tile equals the majority of its 1 m cells; lake cells carry the planner level |
| `horizon_agreement` | existing test, thresholds re-pinned per ring |
| `coarse_mesh_gpu_equals_cpu` | gpu crate: byte-identical tile meshes |
| `far_water_sheets` | a lake at 1 km has a sheet; a 4 m reach has one |
| `water_depth_fade` | capture analysis: film ≥ 80% bed, 2 m ≤ 10% bed |
| `water_foam_coverage` | capture analysis on the fluids fixture: ≤ 8% |
| `water_depth_probe` | existing tests keep passing after the pass split |
| Captures | fluids pose, river eye pose `-150,120,3,0.5,-0.18`, overview pose, 120 s horizon sweep, each before and after, same seed 2738958700 |
| Perf | p95 at 1440p and 28 chunks against the 2026-10-22 baseline; shadow draws at refresh; horizon build time; interleaved A/B |

## 7. Order of work

1. W1 and W2: pass split, per-pixel depth, shore line, strokes. No worldgen
   touch. Gate: fluids and river captures, `water_depth_probe`, cost.
2. F4 water part and W6: coarse field with water level; far sheets. Gate:
   overview capture, `far_water_sheets`.
3. F1: silhouette ink. Gate: sweep and motion captures, no crawl.
4. F4 rings 1 to 3 and F5: coarse tiles, GPU meshing, heightfield beyond.
   Gate: `horizon_agreement`, build time, owner's blocky-or-smooth call.
5. F7 and F6: far pipeline, far cascades, proxies. Gate: shadow draws, p95.
6. W4 reflection and W5 river flow. Gate: captures and cost.

Each slice is one branch with its own A/B evidence under `target/`.

## 8. Decided here, and left to the builder

Decided (my calls unless marked owner): ring radii and the radius/384 cell rule;
blocky ring 1 inside 768 m pending the owner's capture call; no cel banding;
heightfield kept beyond 6,144 m; water drawn in its own pass after opaque; foam
cap 8%; two shore lines and sparse strokes instead of lace; sun and sky lobes
only in the far tier; far cascades every 4th frame.

Open for the owner (answers change the work): ring 1 blocky or smooth; scene
colour copy for refraction on the full tier or depth fade only; ink strength at
distance (persist, as Mycopunk, or fade as today); whether this track runs before
or after step 2 of `docs/plans/2026-10-25-variable-scale-and-underground.md`;
screen-space reflection at all.

Builder's: depth copy mechanics, vertex packing of flow, tile sizes for the
compute mesher, the edge kernel, biome tint weights, bed darkening, stroke
hashing, proxy box authoring per site type.

## 9. Sources

- Devlog 1 May 2026, Procedural Terrain: https://steamdb.info/patchnotes/23052561/
- Mycopunk news (1.8.1 render distance, 1.0 notes, 2.0.2): https://steamcommunity.com/app/3247750/allnews/
- CGMagazine review: https://www.cgmagonline.com/review/game/mycopunk-pc/
- Space.com interview: https://www.space.com/entertainment/space-games/mycopunk-is-what-happens-when-borderlands-gets-a-nasty-fungal-infection-interview
- Store screenshots (16, inspected 2026-10-25): https://store.steampowered.com/app/3247750/Mycopunk/
- ADR 0012 horizon measurement; `docs/design/PIXEL_ART_DIRECTION.md`;
  `docs/design/PAINTERLY_20261024_CHECKPOINT.md`; perf baseline memory 2026-10-22.
