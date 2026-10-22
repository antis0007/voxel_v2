# Visual direction execution, 21 October 2026

Scope: execute the supplied nine prompts in order, preserving the dirty checkout.
The five cited images were not attached; exact screenshot matching remains unavailable.
The existing seed does include a house, material installations and woodland.

## Review contract

`tools/Review-VisualDirection.ps1` launches the actual rebuilt game, without loading
or autosaving a player's world. Use a unique `-Label`; evidence is never overwritten.
Cases: daylight, clouds, water, materials, canopy, cave and motion. Camera bookmarks
are physical metres, with a fixed seed (2738958700), noon, partly cloudy weather,
6 resident chunks, integer world art sampling and native-resolution HUD. The cave
and motion cases use the existing native review routes, rather than substitute renders.
Water/material cases install ordinary world blocks through the existing review hooks.
The cave is the existing torch review, not the missing user's exact cave.

Budget declared before changes: High at 2560x1440, p95 <= 16.67 ms; p99 and hitches
reported separately. 120 FPS is an additional target, not a claim. Reviews use uncapped
presentation. Cloud quality is fixed in benchmark mode; adaptive quality remains
available in play. Explicit `--benchmark-cloud-divisor=3` and
`--benchmark-sky-time=12` prevent cost/animation differences from altering comparisons.
`frames.csv` contains completed benchmark samples. GPU timings are asynchronous
selected-pass measurements, not a complete GPU frame; unavailable timings are tagged.
Process working/private peaks exclude driver-owned VRAM. PNG capture readbacks can
cause hitches; inspect them separately from steady state.

## Colour ownership (locked WGPU 27.0.1)

* Material palette and mesh vertex colours are authored as display/sRGB values.
  `world_shader.wgsl::terrain_srgb_to_linear` decodes in the vertex stage **before**
  interpolation. Procedural shading, illumination and atmosphere operate in linear light.
* Scene/receiver, cloud current/resolved/history and postprocess intermediates use
  `Rgba16Float`; HDR radiance survives until the shared resolve. Cloud RGB is
  premultiplied radiance, blended with ONE / ONE_MINUS_SRC_ALPHA in linear HDR.
* Weather fields use `Rgba8Unorm` numerical data, cloud noise and blue noise use
  numerical UNORM data; depth and normals are numerical. They are not sRGB decoded.
* `postprocess.wgsl` owns exposure, the neutral tone mapper, grading and artistic
  palette finishing. Palette quantization's encode/decode round trip is deliberate;
  it does not encode the HDR scene twice. Diagnostic modes bypass this grade.
* `present.wgsl` uses integer texel loads for world-art upscaling, adds final 8-bit
  dither in display space, then decodes back to linear. The sRGB surface store
  performs the ONE final hardware encoding. The current Vulkan adapter supplies
  an sRGB surface. The non-sRGB fallback is not claimed validated.
* egui is composited last, on the surface's non-sRGB view, using its own display-space
  premultiplied colours. World exposure, bloom and art scale do not affect this HUD.
* `--benchmark-debug=albedo` exposes material albedo before light/fog/exposure;
  the sky is still the real sky. It is a material diagnostic, not an unlit sky renderer.

No demonstrated double colour conversion was found on the current Vulkan path.
No dependencies, material IDs or save formats are changed by the review tooling.

## Progress and evidence

Exploratory baseline: `target/visual-direction/before/clouds-1280-720`.
Actual NVIDIA GeForce RTX 4090 / Vulkan, 2607 measured frames over 32 seconds,
p50 10.774 ms, p95 15.750 ms, p99 17.445 ms, max 450.380 ms.
Cloud governor changed quality during this run, so **exclude it from controlled
before/after performance claims**. Seven actual PNGs were produced and the last
was inspected. No renderer failure occurred. The initial runner incorrectly treated
Windows PowerShell's null process exit code as failure; completion logs prove this
run finished. The runner now requires a completed benchmark summary.

The first build failed renaming an LTO archive under the sandbox TEMP directory.
Building with TEMP/TMP set to `target/compiler-tmp` succeeded (2m09s).

Controlled pre-fix baseline: `controlled-before/daylight-2560-1440`, divisor 3,
1050+ measured frames (use the log for the exact count), p50 26.481 ms,
p95 32.298 ms, p99 33.836 ms. Selected GPU passes average 26.802 ms;
cloud march 23.591 ms. Budget fails. Shadows were disabled in the loaded
render settings (zero shadow draws); this is the current-settings baseline,
not acceptance of a final High preset. The baseline overlapped a CPU release
build, so repeat without compiler activity before performance comparisons.

Cloud cause identified in `cloud_morphology.wgsl::broad_cloud_mass`: a
`seed > odds` branch removed an entire positive signed dome while `odds`
varied continuously over space/time. The replacement continuously moves the
signed dome out of the density range. It preserves negative distances instead
of multiplying them toward zero. This fixes a demonstrated discontinuity;
matching the absent annotated screenshot's exact edge remains unproven.

Cloud target dimensions and copies use the same ceil division at odd sizes.
The resolution floor is now respected even after the governor bump; the old
minimum divisor of two violated its promised 360-row minimum at small sizes.
Targets are cleared each frame; history remains disabled in production.

Prompt 1 is in progress. Prompts 2–9 have not passed their acceptance gates.
