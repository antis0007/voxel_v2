# Cloud sampling quality

## Colour and solar-lighting rework

### Final integrated receipt

The current reviewed release is
`target/cloud-quality-review/colour-integrated/voxel-clouds.exe`, SHA256
`EB055A63E4F1B456BDB79005E0A16D7000BA45691C4293482896FD433EB7A853`.
Launch it with `target/cloud-quality-review/run-colour-build.cmd`; the adjacent
`watch-sunset.cmd` starts a released-mouse continuous sunset demonstration.
Golden hour/Sunset/Afterglow controls are included in this executable.

The integrated release compiled in 91 seconds with WGSL validation, then
completed the previously failing 43-second continuous sunset and a 149-second
five-preset noon/sunset review. All 52 fresh native captures were inspected;
40 preset frames and 12 changing-lighting frames, all 1600x900, all capture
events with released mouse input. No renderer error or full cloud dropout
appeared in those finite captures. The continuous sequence keeps PartlyCloudy
fixed while time advances from 0.71 to 0.765; its camera translates and sweeps
over real Hills terrain near X=16384 m, Z=-8192 m. It is not a colour crossfade
between still images. Logs, dimensions and hashes accompany both runs.

The integrated candidate also includes coordinated noise-domain rotation and
deformation, variable cloud growth, a lower 256/96 view-step budget, three-tap
low/mid self-shadowing, continuous world-anchored shadow quadrature variation,
bounded adjacent-pixel anti-aliasing, and the streaming owner's unmapped GPU
upload/error-scope repair. All frozen cloud/atmosphere/postprocess hashes matched
the source after review. No previous checkpoint was overwritten.

RTX 4090/Vulkan, 1600x900 preset matrix: 69.60 average FPS, cloud pass 8.949 ms,
composite 0.019 ms, p99 frame 35.695 ms. Continuous sunset: cloud pass 8.463 ms.
These are current-run measurements with concurrent desktop activity, not an
isolated speed comparison. Fine distant bands and approximate volume/lighting
transport remain limitations; photographic equivalence is not established.

### Implementation

Cloud illumination now uses its own incident solar energy, atmospheric
transmittance and local self-shadowing. The terrain sunlight value already
included global weather attenuation and daylight; applying it to clouds with
another daylight factor suppressed sunset light twice. The new response
separates gold/peach/coral direct light from cooler blue/violet sky fill, with
altitude-dependent transitions. This is an art-directed RGB approximation,
not a full spectral transport solver.

The sun follows a constant-speed tilted circle. Previously normalizing an
ellipse accelerated its apparent motion near the horizon by roughly threefold,
compressing the strongest colour transitions. High ice clouds now sit near
7.5 km, allowing them to remain illuminated after lower clouds enter shadow.
The settings shortcuts target Golden hour 0.728, Sunset 0.746 and Afterglow 0.754.

The final display transform uses the
[Khronos PBR Neutral tone mapper](https://github.com/KhronosGroup/ToneMapping/blob/main/PBR_Neutral/pbrNeutral.glsl),
ported to WGSL with its Apache-2.0 notice. Its peak compression retains more
highlight hue than the prior per-channel curve. Convective clouds also gained
broader continuous layout deformation and varied radii, aspect ratios and
turret placement. Subsequent coordinated noise/anti-aliasing improvements are
separate from the first frozen colour executable below.

First colour release: `target/cloud-quality-review/colour-release/voxel-clouds.exe`,
SHA256 `D40E14B93FF45A9D236DBECF7BE8FBCD104A82D666B9AE690C5031741C7A1FF1`.
It completed the five-preset noon/sunset matrix at 16 km: 40 fresh native frames,
all with released mouse input. Five frames were 1600x900 and 35 were 2560x1346
after the review window resized; individual sizes are recorded in metadata.
No full cloud dropout appeared in the inspected matrix. The later continuous
time sweep hit a chunk vertex-buffer mapping failure before completion, so
that first executable is not a completed continuous-transition acceptance run.
The streaming owner is addressing that failure before the integrated replay.

Automated review supports `--benchmark-review-time-end` and a fixed weather
override for a genuine continuous sunset, and creates its window inactive.
Existing game sessions retain their compiled shaders; restart into the new
executable to see this implementation.

## Earlier sampling increment and V4 evidence

The renderer keeps the cloud volume in world space and evaluates it at half
resolution. This increment improves reconstruction and long view rays while
preserving deterministic sampling and released-mouse automated review.

- **Distance spaces:** each layer's march budget starts at its slab entry.
  Density fading and LOD now add that entry distance back before comparing
  camera-space distances. Previously a distant layer could fade out simply
  because its entry point consumed much of the remaining march budget.
- **Long segments:** two Gauss-Legendre density taps replace a lone midpoint
  over long grazing segments. The transition is continuous from 12 to 28 m.
  Lighting and self-shadowing still execute once per segment. This costs extra
  density sampling, not another complete lighting evaluation.
- **Reconstruction:** bounded nine-tap Catmull-Rom preserves more cloud detail
  than bilinear enlargement. Terrain edges use depth-matched reconstruction,
  with a nearby matching sample as fallback for thin sky gaps. Bounding cubic
  output by the local range prevents invented bright rims or negative opacity.
- **Display gradients:** fixed zero-mean dither below one display code value
  reduces 8-bit sunset/haze contouring without animated grain. Debug views
  bypass it. This addresses display quantization, not volume integration errors.

Atmosphere, cloud illumination and precipitation changes are coordinated with
the atmosphere rendering chat. They share the final executable and native GPU
captures, so a build receipt alone is never counted as visual verification.
The combined candidate includes warm gold/peach illumination, cooler cloud
shadows, directional rose afterglow and conservative stratiform water-content
variation. A more aggressive sheet-shape experiment was rejected after it
exposed angular holes at noon; its geometry changes are absent from the final
candidate.

## Runtime evidence

The final release candidate is `target/cloud-quality-review/quality-v4/voxel-clouds.exe`.
Use `target/cloud-quality-review/run-reviewed-game.cmd` to launch it from the
correct asset directory. The ordinary release executable was rebuilt too.
The source snapshot, build receipt and full review outputs are preserved with
this candidate; later workspace edits are separate from that frozen executable.

Release build: 50.17 seconds, including built-in WGSL validation. Executable
SHA256: `64EA3742EDA20E196D0ADF4A0A9155D8FE4C5CF5D9B0AC43FBD67EB6A2D28E95`.
The 149-second native run completed over Hills near X=16384 m, Z=-8192 m,
with terrain-following translation and yaw/pitch sweeps. All five presets ran
at noon and sunset. Forty fresh 1600x900 captures were visually inspected in
paired sky/terrain views; all forty capture events recorded released mouse
input. No full cloud dropout or renderer error appeared in this finite review.
Cloud/atmosphere/postprocess shader hashes remained unchanged through the run.

Evidence: `quality-v4/verification.json`, `review.log`, `errors.log`,
`all-40-frames.png` and four `comparison-*.png` sheets under the review folder.
No test suite or hosted CI was used.

RTX 4090/Vulkan: 66.04 average FPS, 15.142 ms average frame, 28.399 ms p99;
cloud pass 12.718 ms and composite 0.020 ms. These figures describe this run,
not an isolated before/after performance comparison. Extra density integration
and the corrected distant-cloud reach are not free; this is still a costly
cloud renderer rather than a demonstrated low-cost solution.

The frozen executable predates the separate UI-only Golden hour/Sunset/
Afterglow button addition. Its existing time control can show the reviewed
sunset at 17:56; those new button labels are not part of this build receipt.

The remaining fine horizontal strips near distant storm bases are visible in
native captures. Dither addresses display contouring, and quadrature improves
integration, but neither is evidence that every remaining stripe is resolved.
Overcast retains a soft sheet appearance; its varied illumination is still an
approximation. Distant terrain and handheld shading are separate limitations.
