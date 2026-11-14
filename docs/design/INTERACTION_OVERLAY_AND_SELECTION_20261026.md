# Magical interaction overlay and rainbow selection

Two effects, both original implementations, inspired by the pixel-dither
treatment in Mycopunk's interaction and selection materials. Extracted reference
material and the parameter facts they were read from are recorded in
`downloads/mycopunk-20261026/` and in `docs/SHADER_AND_NETCODE_STUDY_20261026.md`
covers the unrelated engine study.

## What the reference actually does

Its shaders are stripped from the build, so this is read from material property
schemas only. Three things are visible in that data:

- The interaction material is a visual-effect quad carrying an explicit pixel
  grid (`_PixelCount` 256 by 144) with posterise, scanline and warp controls,
  and the game writes that grid from script.
- The readout material has no texture at all. Its whole pattern comes from a
  band lattice parameter, so the dither is procedural rather than sampled.
- The rainbow selection material exposes five palette slots and a noise block
  (`_NoiseScale`, `_NoiseSpeed`, `_NoiseGlow`, `_NoiseStep`) rather than a hue
  wheel, so its colour comes from stepping between authored colours.

Nothing was copied. The look that follows is rebuilt from the technique: fixed
pixel lattice, ordered-threshold dither, posterised colour, vertical gradient.

## Interaction overlay

`crates/app/src/shaders/outline_shader.wgsl`. This module draws every
`build_outline_vertices` caller in `crates/app/src/main.rs`: the placement
preview, the out-of-range marker and the centre-cell marker. Those three are the
whole population of both line streams, so rewriting the fragment stage changes
the interaction overlay and nothing else. No Rust change was needed and no new
shader file was registered.

Behaviour:

- Dither cells ride the engine's 5 cm world lattice, so the pattern is anchored
  to the world and does not crawl as the camera moves.
- Coverage rises with screen height and along the horizontal edges of the drawn
  cell, so the lower half breaks into sparse dashes while the top stays close to
  continuous. That is the top-to-bottom variation.
- The magical palette is taken from our own reference rules: cream core, warm
  bloom, violet deep, mint haze, gold motes. The caller's colour still shows
  through, so amber keeps meaning out of range.
- Hue is keyed to world position so neighbouring cells differ; mint motes sit on
  a second, sparser lattice.
- Glow gathers upward and fades between 16 m and 40 m so a distant cell does not
  glitter.

Limits: the line pass binds only `CameraUniform`, which carries no time value, so
this overlay varies in space rather than in time. Adding one field to that
uniform would allow drift, and is the follow-up if the overlay should animate.

## Rainbow selection

`rainbow_selection` in `crates/app/src/egui_ui/pixel_art.rs`, called from
`crates/app/src/egui_ui/inventory.rs` for the selected slot.

- Cells are two pixels on the art lattice, walked around the perimeter, so the
  frame is made of pixels rather than a stroke.
- Hue travels twice around the perimeter and drifts with the egui clock, so the
  colour moves through the slot while it is selected.
- About a third of the cells drop out to an ordered threshold, densest at the top
  and thinnest at the bottom, matching the world overlay's gradient.
- Three faint rows under the top edge give an inner halo.

## Verification

`cargo check -p minecraft_style_voxel_prototype` reports 19 composed WGSL
programs validated and compiles clean.

No native capture was taken. The release executable is held open by a concurrent
session, and both effects need specific state to be visible: the overlay needs a
cell under the crosshair and the selection needs the inventory panel open, which
the benchmark routes do not exercise. Appearance is therefore unverified until
someone captures it; the shader and the Rust are verified.
