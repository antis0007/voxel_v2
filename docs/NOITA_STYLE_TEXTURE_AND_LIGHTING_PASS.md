# Noita-style surface, water and lighting pass

## Block palette

- `terrain_pixel_albedo` now keeps a visible palette at **all distances**.
  - Close: a 20-texel/metre lattice with an 8-step luma palette and 4×4 Bayer
    dithering.
  - Far: clean 10-step palette bands with **no dither**, so distant terrain
    reads as authored contours instead of fading into smooth “AI” noise.
  - Value is quantized while hue/chroma are preserved.
- Built materials use hard 20-cell/metre grain; mineral speckle is hard 7/m.
- Natural ground uses hard 10/m cell grain instead of a smooth 6/m noise field.
- Grass now has:
  - 12/m two-tone tip/root pixels,
  - a broader 5.5/m clump layer with hue drift,
  - worn-soil patches,
  - deeper olive tone at full daylight.
- Canopy has hard 10/m leaf pixels, per-pixel hue drift and the retained
  per-leaf tone variation.

This removes the continuous high-frequency multi-octave wash on blocks while
keeping close surfaces visibly pixelated.

## First-person arm and wand

- `kind 54` (skin/leather) and `kind 55` (wand wood) are now explicitly matte:
  roughness `0.96`, reflectance `0`, no environment-reflection lobe.
- A small neutral viewmodel fill keeps hand/sleeve readable in caves.
- The sleeve connector was narrowed from 5 cm to 2 cm and its cloth blocks were
  reduced; the sleeve origin is behind the camera so it enters from the lower
  right instead of crossing the frame as a smooth tube.
- Sleeve cloth is now a muted leather brown rather than a dark green pipe.

## Water

- Water body albedo is snapped to 14 levels; foam is snapped to five hard steps.
- Surface caustics keep chromatic dispersion but quantize focused cores to five
  levels, matching the pixel language of the terrain.
- Underwater caustics now have:
  - a sharper two-scale web,
  - a stronger focused contribution,
  - shallow **caustic spill** onto nearby seabed/rocks/shore shelves.
- Sea level is carried in the spare `cam.coverage.y` lane, allowing shore
  surfaces within ~1.5 m of the waterline to receive a sun-gated caustic bounce
  without a second uniform buffer.

## Shadows and dark areas

- Replaced the axis-aligned 3×3 PCF kernel with a **rotated 12-tap
  golden-angle disk**. This removes the visible cascade-texel stair-step while
  keeping the penumbra small rather than blurring it.
- Cave fill was raised modestly (about 30%) with separate ceiling/floor bounce
  tints. Deep spaces retain dark contrast but no longer collapse to a black
  silhouette.

## Captures

- `target/terrain-style-review/after7/camera.png` — close noon terrain, sleeve
  and grass.
- `target/terrain-style-review/after8/camera.png` — 90 m aerial horizon.
- `target/terrain-style-review/*/camera-800.jpg` — reduced review copies.

The capture runs used seed `2738958700`, Hills, clear weather, noon review
time, and the release executable.

## Validation

- Full suite: **180 passed, 0 failed, 15 ignored**.
- Runtime shader validation succeeds in the benchmark captures.
- Naga WGSL validation via `build.rs` passes.
