---
name: render
description: Renderer, shaders and meshing in crates/app — where things live, the flicker and pixel-grid invariants, how to verify a visual change.
---

# Render

Where: `crates/app/src/render/` (`render.rs` frame step, `horizon/` distant terrain, `shadow.rs`, `postprocess.rs`, `atmosphere.rs`, `streaming/` chunk residency and upload), `crates/app/src/shaders/*.wgsl`, composition in `crates/app/src/shader_sources.rs`, meshing in `crates/app/src/mesh/`.

Shaders: no `#include`; programs are concatenated in `shader_sources.rs` and validated with naga by `crates/app/build.rs`, so a shader error fails the build. Every `.wgsl` must be in the input list (test `every_wgsl_file_is_included_by_shader_sources`). Shared helpers live in `*_shared.wgsl` files; add to those instead of copying a function.

Invariants that prevent flicker we already shipped:
1. Anything glued to the camera (first-person arm, wand) is textured in view space (`is_viewmodel`, `vm_position` in `world_shader.wgsl`). World-anchored patterns swim as you walk.
2. Never discard small moving geometry with the world voxel grid (`fract(voxel_pos)`); shrink the geometry instead (`Model::glow_voxel_with_dissolve`).
3. Ambient motes (`mood_ambience`) are a pure function of world position and the clock: one mote per cell, bounded drift, one native pixel, palette colour.

One pixel size: world textures use `PIXEL_RES` (20 texels/m) for every hash cell, dither and noise lookup; seams are one texel; motes are one screen pixel; wand orbs are `voxel_sphere`s of `WAND_PIXEL` cubes. Palette: `assets/pixel_palette.txt`, mirrored as `MOTE_*` in `mood_shared.wgsl`.

Verify a visual change: `just verify`, then a fixed-pose capture before and after (`docs/design/VISUAL_DIRECTION_EXECUTION.md`; flags in `docs/design/MIRAGE_AND_COSMOS_UPDATE.md`). Evidence goes under `target/`, never overwrite a label.

Pitfalls: the horizon modules are reached only through `render/horizon.rs`; `render.rs` is still 4.8k lines (debt); the loading screen and HUD share `egui_ui`.
