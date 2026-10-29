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

## Evidence

Captures from `--capture-menu=<dir>` through the production egui/wgpu pipeline.
Before: `target/ui-main-menu-final-review/` (the previous menu review candidate).
After: `target/menu-scene-20261028/`.
