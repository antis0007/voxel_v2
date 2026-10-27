# Visual quality audit triage, 27 October 2026

Source: the pasted "Axiom full visual quality audit" (15 sections, one screenshot not
attached). This file maps each claim to evidence from the diagnostic views, so later
sessions start from measurements instead of the prose.

## Reproduction

Pose reproducing the audit's vista (temperate forest looking at the alpine range,
sun low and ahead-right):

```
--benchmark-world=hills --benchmark-seed=2738958700 --benchmark-generator-version=65
--benchmark-terrain-camera=96,0,45,3.46,-0.10 --benchmark-review-time=0.715
--benchmark-weather=partly --benchmark-view-chunks=8 --benchmark-sky-time=12
--benchmark-cloud-divisor=3 --benchmark-width=1600 --benchmark-height=900
```

Review time maps to sun height `sin((t - 0.25) * 2pi)`: 0.5 noon, 0.75 sunset, 0.715
is about 12 degrees. `tools/Review-HorizonContinuity.ps1` caps the generator at 64 and
pins noon, so these runs used a scratch runner with the same flags.

Evidence: `target/reviews/visual-audit-20261027/`.
- `vista-golden/<view>/bench-000.png`: generator 65, dirty-tree exe built 01:49
  (`audit.exe`), views none, clay, mesh-kind, geometric-normal, shading-normal,
  terrain-shadow, unfogged, albedo, sky-access.
- `ab53-{golden,noon}-veil-{on,off}/`: HEAD 304b9fe5 (generator 53), veil weight
  0.78 vs 0.0, otherwise identical (`veil-on.exe`, `veil-off.exe`).

## Findings by audit section

| Audit item | Verdict | Evidence |
|---|---|---|
| 1 Holes, black streaks | Not reproduced at this pose | `clay` and `mesh-kind` show continuous coverage; resident/backdrop boundary at ~256 m is clean. Lead only: `geometric-normal` shows violet texels on several peak tips (normal facing +z, away from the camera), possibly back faces at lattice spikes. Needs the audit's actual pose or a route sweep. |
| 2 Mountain shapes | Confirmed, two causes | `clay`: smooth cones with single-vertex crowns. Backdrop lattice cells are 128-512 m beyond 6 km (`render/horizon/detail.rs`), so a peak is one vertex. `shading-normal` is smoother than `geometric-normal`: the 16-128 m detail maps add no mid-scale relief. Shape itself comes from `worldgen/src/landscape.rs` (ridged multifractal, pyramidal peaks); changing it needs a new generator gate. |
| 3 Atmospheric wash | Confirmed, cause measured | Distance veil added in 2d5532fa (`world_shader.wgsl`, `let veil = ...`): mixes everything past 80 m toward `solar_horizon`, weight `0.78 * (1 - exp(-(d - 80) / 2400))`, about 0.43 at 2 km and 0.62 at 4 km. Mountain band, veil on vs off: golden hour mean abs diff 46.4/255, luma std 35.8 vs 42.1; noon 6.4/255, 30.0 vs 36.6. `unfogged` keeps grey granite vs tan rock and `terrain-shadow` has shadowed faces; the veil erases both at low sun. |
| 4 Terrain stripes | Partly seen | `albedo` shows smeared low-frequency blotches on mid-distance backdrop hills. Not diagnosed further. |
| 6 Shadows | No unexplained marks at this pose | `terrain-shadow`: cascades clean near; far ranges shade each other via the height-field march. |
| 5, 7-13 Trees, sky, elevator, water, viewmodel, HUD, pixel identity, places | Art direction | Owner decisions. Known constraints: machine float-greeble look rejected, authored voxel assets on the fine sub-grid, wand art from `assets/wands.ron`, HUD text under `UI_RULES.md`. |

## Veil split (item 3, landed)

The veil now pulls hue toward the horizon at the surface's own brightness (weight
`0.55 * veil`) and brightness toward the horizon at `0.15 * veil`; the distance curve
is unchanged. Same base (304b9fe5), generator 53, pose above; regions are 1600x900
pixel boxes defined in `veil_metrics.py` (next to the captures, with the
`audit-views.ps1` runner) (cone = centre tan range, rock separation =
brightness-normalised colour distance between the left grey range and the cone,
forest = far forest band right of centre).

| Variant | Golden cone luma | Golden rock separation | Noon far-forest saturation |
|---|---|---|---|
| veil off | 121 | 0.212 | 0.388 |
| veil 0.78 (2d5532fa) | 192 | 0.052 | 0.319 |
| hue 1.0, brightness 0.5 | 176 | 0.069 | 0.327 |
| hue 0.55, brightness 0.25 | 154 | 0.103 | 0.353 |
| hue 0.55, brightness 0.15 (landed) | 142 | 0.123 | 0.360 |

Captures: `ab53-{golden,noon}-{veil-off,veil-on,split50,h55b25,h55b15}/none/`.
The landed variant keeps about half of 2d5532fa's noon desaturation of far forest.
The veil still feeds `total_fog_t` at full weight; reflection visibility is unchanged.
Merging it into the M5 fog consolidation (`2026-10-26-axiom-brief-reality-check.md`,
nine fog paths) remains open.

## Recommendations

1. Mountains (item 2): grayscale `clay` acceptance first. Geometry work splits into a
   worldgen gate (ridge/valley structure) and backdrop resolution near peaks (lattice
   refinement already targets 6-24 km ranges, `render/horizon/refinement.rs`).
2. Holes (item 1): fafbee28 seals coarse LOD seams against sky holes; rerun the nine views at the owner's screenshot pose; the
   `--benchmark-horizon-sweep` route covers LOD transitions.
