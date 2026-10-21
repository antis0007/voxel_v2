# Cloud reconstruction, light transport and voxel particle dissolve

This pass addresses the hard-edged sky mottle in the cloud composite and moves
the volumetric transport closer to current realtime cloud practice. It also
turns the micro-voxel spell particles into Noita-style eroding 3D sprites.

## 1. The mottle: continuous reconstruction

`src/shaders/cloud_composite.wgsl`

The half-resolution cloud march is reconstructed into a full-resolution frame.
The old path chose between a Catmull-Rom cubic and a bilateral 3×3 smoothed
value with:

```wgsl
select(0.0, denoise, filtered_weight > 1e-5)
```

That is a binary per-pixel switch. Together with the `min(corner alpha)`
storm threshold and the hard `alpha <= 1e-4` cutoff, it produced the contour
blotches in the screenshots. The reconstruction now uses:

- `neighbour_confidence = smoothstep(0.0, 1.6, filtered_weight)` so rejected
  depth taps fade between cubic and smoothed instead of switching.
- `deck_alpha = mix(min_alpha, mean_alpha, 0.65)` and a single continuous
  `storm_dense` ramp; no individual corner threshold can own the blend.
- `coverage = smoothstep(0.0, 0.006, alpha)` with premultiplied RGB and alpha
  both scaled by the same coverage. The 1e-4 nesting cutoff is now a knee.
- A smoothly quantised four-band art ramp:

  ```wgsl
  let stepped = (band_i + smoothstep(0.15, 0.85, band_f)) / bands;
  ```

  The flat plateaus survive, but their boundaries are C1-continuous instead of
  a `floor()` contour.

## 2. Cloud transport and colour

`src/shaders/cloud_shader.wgsl`

`CLOUD_AB_LEGACY` is switched to `false`, enabling the modern path that had
already been written but kept dormant:

- view-dependent Beer–powder instead of a constant powder term;
- depth-probability multiple-scattering gain (`ms_full`) for low/mid decks;
- upper altostratus deck shadowing through the cloud-shadow field;
- dusk body hue instead of a pure grey core.

Added in this pass:

- **Chromatic extinction**: one `exp(-shadow_d * extinction_rgb)` per sample
  instead of scalar-only Beer. The storm core drifts cool and deep sunset paths
  keep warmth instead of collapsing into neutral grey.
- **Cheap four-order transmittance**: higher scattering orders are derived from
  `sqrt` and `sqrt(sqrt(...))` of the single chromatic exponential. That
  approximates the Hillaire octave spacing while avoiding three extra `exp`
  calls per march sample.
- **Tinted high-order scattering**: the fourth order is mixed toward the sun
  hue at dusk instead of being pure white, while noon keeps a pale luminous
  core.
- **Chromatic moonlight**: direct moon in-scatter uses the same spectral
  transmission as the sun path.
- Existing Schneider/Hillaire features remain: slab-bounded DDA, blue-noise
  march phase, cone-footprint density LOD, local self-shadow integration,
  powder/silver lining, four-order multiple scattering, aerial perspective and
  temporal reconstruction/clipping.

## 3. Particle dissolve

The particles are already 3D micro-voxel cuboids. The change makes their death
a pixel dissolve instead of a uniform shrink/fade:

- `Model::glow_voxel_with_dissolve` packs a 0..1 dissolve threshold as a
  **negative** specular value, so ordinary glow glyphs and beams are unaffected.
- `Adventure` writes a per-particle dissolve phase from life and particle
  position. This keeps bursts from disappearing as one solid block.
- `world_shader.wgsl` erodes a hashed 3D cell grid (`in.voxel_pos * 40`) and
  blends in a 2-pixel screen-space grain so the pattern remains readable when
  the cells project below one pixel.
- The surviving cells at the erosion front get a bounded additive rim, giving
  the hot, ragged edge of a dissolving Noita sprite.
- Glow particles force `roughness = 1`, `reflectance = 0` so the packed
  dissolve value cannot leak into specular shading.

## Validation

`build.rs` validates every composed WGSL program with naga. The shader sources
were touched and `cargo check --offline --target-dir target` re-ran that
validation successfully. No game launch or visual capture was possible because
an existing game process holds the executable open.

Limits that remain: clouds are still a layered volumetric approximation rather
than a path-traced single medium; overlapping layer composition is ordered by
opacity, not per-sample interleaved transport; there is no screen-space GI or
new probe system. The particle dissolve is a hashed alpha/coverage erosion, not
a sprite-atlas animation.
