# Texture readability pass

The terrain detail shader was producing a low-contrast olive-brown average. The
meadow ramp mixed a brown boreal colour into cool grass, a warm vigor
multiplier pulled green toward yellow, and soil/rock overlays accumulated on
top. Stone, dirt and leaf branches had their own noise, but the shared result
was close enough in hue and value to read as one muddy surface.

## Changes

- Grass is now green-dominant:
  - saturated lush/bright meadow colours instead of brown-green mixing;
  - cool climates use a deep blue-green boreal colour;
  - dry mixing is limited to genuinely arid climate weights;
  - sides keep a soil edge without replacing the top turf.
- Grass blade-scale value variation was added, plus a small saturation finish.
- Shared natural-material finish raises saturation slightly while preserving
  luminance contrast. This affects grass, dirt, stone, gravel, sand and beach
  layers without changing any material IDs or vertex data.
- Dirt/Mud/Clay now retain more of their authored base hue and gain close-range
  grit instead of a single shared brown overlay.
- Rock bedding contrast is stronger, so stone does not flatten into grey-brown.
- Canopy colours use richer summer and pine greens; autumn remains a distinct
  warm accent through the existing seasonal material tint.
- Built/mined surfaces receive a mild saturation/value finish so wood, mortar,
  bark and ore branches keep their hue separation.

No material IDs, collision, save data or generator geometry changed in this
pass. The material studio can still override family tints and finishes.

## Limits

This was validated by compilation and the `build.rs` Naga WGSL validator only.
No subjective in-game capture was made. The values are authored in the shader's
linear working space; if the art direction needs an entirely different palette,
the constants in `natural_surface_blended` and the leaf/rock branches of
`block_surface` are the tuning points.
