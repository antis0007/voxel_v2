# First-person and lighting critique triage, 3 November 2026

Source: the pasted 21-section critique (lighting hierarchy, material layers, edge
response, held block, grip profiles, torch, viewmodel exposure, hand anatomy,
contact shadows, atmosphere, sky, snow, trees, macro normals, crafted vs natural,
UI unity, item weight). The screenshot was not attached. This file maps every
section to code so the next session starts from constants and captures, not
prose. Renderer and viewmodel code are unchanged by this triage.

Companion triages: `2026-10-27-visual-audit-triage.md` (veil, fine rings),
`2026-11-03-surface-rendering-review-triage.md` (debug views, globe path).

## Verdict key

- Done: the mechanism the critique asks for already exists; only tuning remains.
- Partial: the mechanism exists for part of the range or as hard-coded constants.
- Gap: nothing in code does this.
- Owner: art direction; needs the owner's call before work.

## Sections 1 to 21

| # | Claim | Verdict | Code | Notes |
|---|---|---|---|---|
| 1 | Whole frame crushed grey-green | Unverified | `render/postprocess.rs` (Rgba16Float scene, tonemap in `postprocess.wgsl`), veil split in `world_shader.wgsl` ~4084 | Pipeline is linear with one encode. No ground-level luma measurement exists; the 10-27 numbers are for the vista band only. Owner rule: no posterization, balanced not overexposed |
| 2 | One noise drives all material layers | Partial | `rock_surface` ~1733: `shape_cells` id tint, chips, cracks, `texel_w` speckle; strata from `macro_normal.y` ~2853 | Three scales exist (strata, chips/cracks, speckle) but all derive from `surface_hash`/`surface_noise` families in one face-uv frame. Macro 2-20 m discoloration and moss distribution are not separate fields |
| 3 | No edge response on stacked cubes | Gap for natural, Done for brick | `architectural_surfaces.wgsl` ~2350 bevel + mortar | Natural voxels have no convex-lighten / concave-darken term. Pixel direction fixes seams at one texel (`PIXEL_RES`), so any edge term is at most one texel wide. One-constant A/B candidate |
| 4 | Held block floats, too big, over-rotated | Confirmed by constants | `mesh/models/held_item.rs`: `REST (0.34,-0.22,0.62)`, block offset `(0.105,-0.065,0.04)`, grip roll `z -0.65 rad` (-37°), cube `x 0.35, y -0.62` (20°, -36°), `CUBE_EDGE 0.14` | Critique asks yaw 10-20°, pitch -5-10°, roll 5-12°. Current cube rotation is the dramatic diagonal it describes. Fingers never touch the cube: rig bones in `assets/viewmodel/rig.ron` are one fixed curl for every item |
| 5 | Separate camera, inertia, bob, grip | Partial | `held_item::frame` = `REST + sway + bob + poke/mine`; `placement` matches on `Art` kind | The chain exists in the critique's order but the per-item grip is a `match`, not data |
| 6 | Torch grip | Partial | `torch_tilt` (y -0.55, z 0.12), `TORCH_SEAT`, `HELD_GRIP_Y 0.43` (lower third), chop zeroed for torch | Placement matches the ask. Sway rate and bob are shared with every item (`viewmodel.rs:111`, rate 16), so the torch has no softer wrist |
| 7 | Torch lights the hand and nearby world | Done | `eval_torch_light` ~1296: falloff `cutoff² / (1 + 0.14 d²)`, held torch uniform `held_torch_pos_radius` (`render.rs:1018`) | Same functional form the critique proposes. The hand is gated by `receives_local_light = terrain_has_resident_metadata(ao_strength)` ~3755; verify with a night `VOXEL_REVIEW_HELD` capture that the fist itself receives it |
| 8 | Viewmodel exposure bias | Done | Neutral fill `0.075` on non-authored parts ~3865; authored kit 0.20 sky access; no fog on viewmodel | Equivalent to a small EV bias, not expressed as one. Making it a single EV constant is a refactor, not a look change |
| 9 | Hand anatomy | Owner | `assets/viewmodel/hand_v6`, `docs/design/VIEWMODEL_RESTART_20261026.md` | Two hands already rejected (10-25, 10-26). Next hand needs owner reference, not a third guess |
| 10 | Contact shadows at three scales | Partial | tiny: vertex `cavity` (floor 0.42, `pow 1.35`) ~3573; large: `macro_occlusion` from horizon cavity beyond 64 m ~2822; no SSAO | The medium scale (rock overhangs, tree clusters, 1-10 m) is the hole: resident terrain within 64 m has zero cavity reach. Trunk-to-ground contact relies on vertex AO only |
| 11 | Atmospheric perspective | Done | Veil split 553670db: hue 0.55, brightness 0.15 | Measured 10-27. Cooler-and-desaturated far field is what the split does; remaining ask is contrast reduction per distance, which the veil does not do |
| 12 | Sky empty | Done | `sky_shader.wgsl`, `atmosphere_*.wgsl` LUTs, `celestial_sky.wgsl`, volumetric `cloud_shader.wgsl` | Overcast at `--benchmark-weather` presumably produced the flat frame. Overcast grading (zenith gradient, sun glow through cover) is a weather-preset tune, not a sky rewrite |
| 13 | Snow is a height line | Done | `worldgen/src/landscape.rs` ~1566: `cold*w + snowline*0.58 + patch*0.32 + sheltered*0.12 - slope*0.18`; geo temperature includes altitude | Already altitude, slope, temperature, shelter. Wind aspect is absent. Any change is a new generator gate |
| 14 | Trees stick + blob | Owner | `render/horizon/vegetation.rs` 12-vertex crowns; grouped far canopy already filed as finding 2 in the 11-03 surface triage | |
| 15 | Hybrid macro normal | Partial | `macro_normal` ~2818 blends toward the horizon detail-map normal only when `is_backdrop or macro_dist > 64` | Inside 64 m natural terrain lights on face normals, exactly the critique's complaint. Extending the blend inward needs a resident-scale macro field (smoothed height or vertex-AO gradient) |
| 16 | Crafted vs natural | Done | `is_natural` branch: `natural_surface_blended` vs `architectural_surfaces` | The rule exists; its strength is a tuning question |
| 17 | UI, hand, world styles differ | Owner | `UI_RULES.md`, `assets/pixel_palette.txt`, `sprites16` | |
| 18 | `GripStyle` / `GripProfile` | Gap | `placement` match in `held_item.rs`; poses in `rig.ron` / `presentation.ron` are per state (rest, aim, inspect, interact, sprint), not per item | Data-driving the match is the enabling refactor for 4, 6 and 19 |
| 19 | Critically damped spring | Done | `viewmodel::Spring::step`: closed-form `(delta + impulse·dt)·e^(-rate·dt)` | Already critically damped. One rate (16) for all items; per-grip stiffness waits on 18 |
| 20 | Mass-driven animation | Gap | No mass on `Item` | Design question; cheap once 18 exists |

## What the critique's priority order means here

1. Exposure: measure first. Run `audit-views.ps1` (session scratchpad from the
   11-03 surface triage) at a ground pose holding a block, overcast, and record
   luma histograms of foreground, midground and sky bands. No tuning before that.
2. Material readability: separate the macro 2-20 m field from `surface_hash`
   (section 2) and extend `macro_normal` inside 64 m (section 15). Both are
   `world_shader.wgsl` changes with A/B captures at a near-rock pose.
3. Held block: four constants in `held_item.rs` (section 4). A/B via
   `VOXEL_REVIEW_HELD`. Finger contact needs a second bone curl in `rig.ron`
   selected by `Art::Block`, which is the first slice of section 18.
4. Grip profiles (18): move the `placement` match into a RON table keyed by
   `Art` kind with offset, rotation, sway rate, bob weight, bone curl.
5. Spring: done; per-grip rate lands with 4.
6. Medium AO (10): the real gap. Options are a resident cavity reach within
   64 m or a short-range screen-space term; both need a perf receipt
   (`just bench`) because the shader is already the frame cost.
7. Snow: done, wind aspect only, gate required.
8. Trees, 11 veil, 12 sky: done or owner.
9. Macro normals: with step 2.
10. UI unity: owner.

## Not done here

No shader or viewmodel constant was changed: every item is a look change and the
owner rejects look changes without same-pose before/after captures. The
screenshot pose is unknown; the nearest known held-item recipe is
`VOXEL_REVIEW_HELD` (146807a1).
