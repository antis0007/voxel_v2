# Main menu scene, 28 October 2026

The title screen was a flat charcoal field with a centred column. Nothing on it
said what the game is. The artificers' instrument room (`shaders/menu_scene.wgsl`,
`egui_ui/menu_scene.rs`) had been written for this screen, with a reserved quiet
band for type, but no module declaration or caller ever landed, so it never drew.

## Changes

- The room is installed with the other egui callbacks and painted behind the
  title stack and the loading screen. Reduced motion freezes the clock at zero.
- Wide windows (900 points and up) anchor the column at 8% from the left so the
  bench, furnace and crystal stay visible. The host passes the column's right edge
  to the shader, which shades the room to 16% behind the column and lifts it over
  the next 30% of the width. Narrower windows keep the centred stack over an even
  42% shade. Loading dims the room a further 36%.
- Shader: a dithered six-step march of sunlit air inside the room volume, three
  short occlusion rays per hit, stepped furnace flicker, one-pixel embers rising
  in a plane before the furnace mouth (one ember per 0.1 m cell, pure function of
  cell and a tenth-second clock), analytic in-scatter halos around the furnace and
  crystal, a slow lateral camera sway, and a 4x4 Bayer dither before the 8-bit
  store. A first pass with sky stars and a stronger, unphased haze read as grey
  fog with grey blobs in the sky; both were removed before the final capture.
- Uniform contract: `clock = (seconds, aspect, quiet edge in uv or -1, dim)`;
  16 bytes as before.

## 29 October: loading, flame and dusk

- The world-load screen no longer dims the room or floats its label over the
  bench. It paints the same shot as the title with the stage label and bar in
  the title column (`menu_scene::column`, shared with `main_menu.rs`), so title
  to loading is one continuous view. The sweep advances in whole pixels.
- The focused-control flame (`ui_prism.wgsl`) stood on a base at 38% of the
  control height and shed sparks downward, so it filled only the upper half.
  It now stands on the icon's foot (control centre plus 10 px), reaches the top
  edge, and two in three sparks rise from the tongue while one in three settles
  as ash below. The host rect is unchanged.
- Scene: dusk sky (warm band at the ridge, teal, indigo zenith), a low warm sun
  against cool sky ambient, warm haze, darker distance fog, and a slow breath on
  the crystal's light. The previous grey-teal sky washed the right half.
- Captures: `target/menu-scene-20261029` against `target/menu-scene-20261028`.
  Flame crops at 4x: `crop-title-focused*.png` in the same folder.

## Evidence

Captures from `--capture-menu=<dir>` through the production egui/wgpu pipeline.
Before: `target/ui-main-menu-final-review/` (the previous menu review candidate).
After: `target/menu-scene-20261028/`.
