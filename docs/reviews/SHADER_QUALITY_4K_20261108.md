# Shader quality and 4K frame budget — 2026-11-08

Target: native 3840 × 2160 at 200+ FPS on RTX 4090, or less than 5 ms per
frame. This is a target, not a verified whole-game result. Work stays in code:
build validation, numeric GPU readbacks and timestamp profiling; no visual
inspection, screenshots, test suite or hosted CI.

## Accepted changes

- **Pixel centres:** fragment position already addresses the centre of a pixel.
  The resolve added another half texel, averaging fine detail even at native 4K
  and shifting depth sampling. Remove it, including in the contact-shadow path.
  `--benchmark-reference-pixel-centres` retains the faulty coordinate transform
  for numeric comparison only; the default pipeline compiles it out.
- **Independent axes:** use the actual scene/window ratio on each axis. Scaled
  scene dimensions round independently; using the horizontal ratio vertically
  displaced samples in odd-sized windows. The spare uniform component carries
  the vertical ratio without a new allocation or bind group.
- **Coherent contact shadows:** depth now samples the same warped coordinates as
  scene colour during spell lenses and zone swirls. Disabled SSAO returns before
  loading depth. Palette encoding is inside the palette-enabled branch.
- **Rough diffuse:** cancel the tangent lengths analytically in Oren–Nayar.
  Projected light/view vectors already contain the sine factors. Two tangent
  normalizations and two additional square roots are unnecessary. Preserve the
  authored coefficients, grazing denominator, back-face behaviour and roughness.
  GGX, material normal filtering, HDR lighting and colour direction stay intact.

## Evidence and limits

The profilers use the production sources. `shader_quality_profile` alternates
reference/candidate order each pair, with 64 warmup and 64 measured pairs;
`postprocess_capture --no-image` uses 96 warmup and 96 measured frames. Raw byte
comparisons are numeric only. Readback is outside GPU timestamps.

On RTX 4090, Vulkan, driver 617.42, at 3840 × 2160:

| Measurement | Result | Meaning |
|---|---:|---|
| One-pixel stripe contrast, extra-offset reference | 0.001833 codes | Adjacent pixels were effectively averaged away |
| One-pixel stripe contrast, corrected coordinates | 90.351032 codes | Fine detail survives the resolve |
| Rough diffuse, maximum absolute lobe difference | 0.000100493 | Largest difference is around near-normal regularization/roundoff |
| Rough diffuse, mean absolute lobe difference | 0.000000010 | 33,177,600 finite float components, including grazing/back-facing cases |
| Rough diffuse kernel reference / candidate median | 0.172032 / 0.171008 ms | Float32 writes dominate; no meaningful whole-frame gain established |

Stripe measurements disable AA, bloom, vignette and SSAO to isolate sampling;
they retain the production grade and final dither. This is numeric evidence of
pixel placement, not an artistic acceptance claim. Rough-diffuse measurements
include fixture arithmetic and float32 output bandwidth, not a terrain pass.

## Experiment rejected after measurement

A local cubic expansion of the inverse sRGB transfer removed one RGB `pow` from
the final dither. It preserved black, white and the linear toe, with a maximum
linear output difference of 0.000000536 in the float fixture. In the production
4K resolve it changed 243 of 8,294,400 pixels by at most one code, averaging
0.000009766 codes per RGB channel.

It did **not** reliably improve performance. Across three runs in alternating
order, the median of reference medians was 0.422912 ms; candidate 0.430080 ms
(1.7% slower). Keep the existing transfer in production. The rejected candidate
is retained only under `examples/support/dither_candidate.wgsl` for reproduction.
Reducing arithmetic on paper is not sufficient evidence of faster GPU execution.

Initial frozen profiler SHA-256:

- `shader_quality_profile.exe`: `BC8245BAC8B0B10394019FEE274910BF95FC48BCF6207918EE319AEEF0526B0D`
- `postprocess_capture.exe`: `35AE9F94F96976A33866471843EE098FAE58ADA11CAFE57CC142E94C7B3172DE`

Logs and frozen executables are in `target/shader-quality-20261108/`. These initial
executables include the rejected experiment. Production keeps the original
transfer; final verification uses separately named executable copies.

Final builds of the game and all three profiling examples passed. The build
validated 28 composed WGSL programs (`world_fnv1a64=fcba67ae9457b09c`). Existing
compiler warnings remain; no test suite was added or run.

Final production resolve:

| Configuration | Native median / p95 | Separate-pass median | Numeric difference from separate pass |
|---|---:|---:|---|
| 3840 × 2160, standard grade, bloom and AO | 0.417792 / 0.418816 ms | 0.443392 ms | Maximum 1 code; mean 0.020865644 codes/channel |
| 1919 × 1079, 73% scene resolution, spell and zone warp | 0.145408 / 0.146432 ms | 0.144384 ms | Maximum 1 code; mean 0.020571161 codes/channel |

These are single final verification runs, not new whole-frame speedup claims.
Both paths retain half-float intermediate precision; attachment conversion and
explicit half packing are not bit-identical on this device. The final 1279 × 719
art-scale-3 and diagnostic-bypass paths also executed successfully. Final
executables and source SHA-256 values are recorded in `final-hashes.txt` beside
the logs. The retained native-pass fusion predates this audit.

### Whole-scene diagnostic sample

The final game executable (`93B1871FB6B9800EFBD303D32F741699ECD407D10B1732839618AC5DD276391D`)
ran the fixed hills regression fixture: seed 74, generator 53, partly cloudy,
static camera, native 3840 × 2160, view distance 12 chunks, 8 s warmup and 12 s
measurement. It used a scratch save directory, hidden launch and no captures.
Generator 53 is the established benchmark fixture; the current generator default
is 241. This does not represent every current world or the orbital view.

- 557 frames, average **46.26 FPS**; median **21.530 ms**, p95 **22.850 ms**.
- Average GPU total **21.004 ms**: terrain **10.501**, cloud raymarch **8.362**,
  shadows **1.248**, postprocess **0.388**, sky **0.197**, cloud composite **0.192**.
- Cloud divisor 4, temporal history disabled; no adaptive-quality change was
  requested. The scene was fully resident: 1,442 visible chunks, no missing or
  pending chunks, 4,107,569 terrain triangles and 16 torches.
- Terrain and cloud raymarch account for about **90%** of measured GPU time.
  A faster final resolve cannot close the gap to 5 ms. The terrain timestamp
  includes geometry, material and lighting work; it does not isolate one cause.

The requested 4K/200 target is **not achieved** by this patch. These are diagnostic
samples of the final executable, not a before/after FPS comparison. Logs and
per-frame numeric data are under `frame-sample/` beside the component results.

An additional albedo diagnostic run is retained under `frame-albedo/`, but is
**not a valid lighting-only comparison**: shadow draws changed from 523 to 964,
average shadow cost from 1.248 to 8.134 ms, and frame p95 rose to 70.034 ms. It
changes more than lighting output and has a much noisier tail. Do not infer a
material-versus-lighting cost split from it or use its 21.23 FPS as a regression
against the standard-grade run. A dedicated isolated pass profile is needed for
that split.

## Three directions to choose from

All three retain the documented broad voxel shapes, restrained material detail,
smooth atmospheric gradients, warm local lights, and charcoal/paper/brass/teal UI.
Nostalgia comes from readable construction and useful controls, not a coarse
full-frame filter or a fixed palette over clouds and water.

1. **Crisp simulation — recommended foundation.** Prioritize stable native detail,
   consistent material response and readable structures at distance. The 4K
   evidence makes terrain the first budget target: separate geometry/coverage,
   procedural material and light visibility cost, then cache the repeated work
   with invalidation tied to world edits. Avoid replacing detail with a broad
   distance blur or merely moving the same cost into an update pass. Extend the
   current material filtering and lighting work only where numeric profiling or
   debug output identifies a defect. Keep the 90s influence in silhouettes,
   authored material clusters and compact utility panels. Lowest additional GPU
   and memory risk; less spectacle than adding another lighting effect.
2. **Atmospheric world.** Prioritize defined cloud bodies and coherent weather
   from ground to orbit. Investigate cached sun transmittance and conservative
   empty-space bounds before adding raymarch samples. Cache refresh cost, motion
   invalidation, thin-wisp preservation and close-cloud detail are acceptance
   conditions. This deserves a bounded experiment, not an automatic reduction
   in cloud resolution whenever the rest of the frame is slow.
3. **Lived-in settlements.** Prioritize warm workshops, shaded interiors and wet
   streets. Existing grouped light bounds already reject irrelevant torches;
   evaluate spatial light lists and bounded ambient-light caches only for dense
   settlements where those groups overlap. Edits must invalidate cached lighting
   and prevent light leaking through walls. Highest integration cost; supplies
   useful lighting and material depth where players build and inhabit the world.

The current cloud governor's 14 ms over-budget / 8 ms recovery thresholds do not
represent a 5 ms target. Do not silently retune them to trade away the requested
cloud quality. Establish complete scene timings and a quality floor before making
the governor target-aware. A whole frame also includes terrain, shadows, clouds,
water, UI, streaming and simulation; isolated resolve timings cannot establish
200 FPS. CPU and GPU overlap, so their timings must not simply be added.

## Technical choices checked against current sources

- The workspace already uses [wgpu 30.0.1](https://github.com/gfx-rs/wgpu/releases/tag/v30.0.1),
  the current stable release checked for this audit. An upgrade is not a performance technique.
- [WGSL fragment position](https://www.w3.org/TR/WGSL/#position-builtin-value)
  defines the input coordinate used by the corrected resolve.
- [Filament's material model](https://google.github.io/filament/main/materials.html)
  supports bounded specular antialiasing; this renderer already has roughness and
  normal-variance filtering. Adding another global blur would duplicate it.
- [Guerrilla's Nubis³](https://www.guerrilla-games.com/read/nubis-cubed) motivates
  the cloud acceleration experiments, not a claim that this renderer reproduces
  its image quality or performance.
- wgpu's [ray-query](https://github.com/gfx-rs/wgpu/blob/v30.0.1/docs/api-specs/ray_tracing.md)
  and [mesh-shader](https://github.com/gfx-rs/wgpu/blob/v30.0.1/docs/api-specs/mesh_shading.md)
  work remains experimental. Neither is an appropriate default prerequisite for
  this 5 ms goal without scene-level evidence.
