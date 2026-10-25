# Grass, sky and orbital cloud blending

Bounded continuation of the painterly visual plan. Existing unrelated edits,
world generation, saves and controls are preserved.

- Grass tufts use footprint-aware soft boundaries; dry tips and flower centers
  transition smoothly instead of switching at hard distance thresholds.
- Cloud color grading uses less banding and retains more marched light color.
  Grading preserves marched opacity, so thin veils retain their transmission.
- Celestial meshes retain spherical atmospheric extinction but no longer receive
  the additional weather-summary cloud fog. The volumetric pass owns cloud cover.
- Cloud reconstruction classifies surfaces beyond its 16 km march range with
  clear sky. Distant orbital pixels can use neighbouring cloud samples instead
  of rejecting them as foreground depth discontinuities. Near surfaces retain
  the existing depth-aware filtering. The composite receives the current camera
  uniform at creation and resize, within the existing four bind groups.

Local build passed, including 19 composed WGSL programs; log:
`target/grass-sky-station-build.log`. No tests or hosted CI were added or run.
Native visual review artifacts: `target/grass-sky-station-review/`.
The isolated native render completed, and `camera.png` was inspected. No shader
or binding validation error was reported (the optional Vulkan validation layer
was unavailable). Grass and sky render; this static view contains no controlled
station pass and cannot establish temporal stability. Capture timing is not a
matched performance comparison.
This change does not establish flyby temporal acceptance or completion of the
larger engine/world roadmap. Low-resolution subpixel geometry aliasing, objects
inside the cloud range and cameras above cloud layers still need motion review.

## Follow-up: ground cover and reproducible passes

The mesh colour path decodes sRGB at vertices. The old grass blade swatches
were much darker after that conversion than the linear turf palette. Blade
roots now use muted sRGB olive/sage values, with smaller live/dry tip gains.
This palette change does not alter placement, density, geometry, wind, collision
or saves. The shared checkout also received concurrent foliage geometry/density
and flower changes, which are included in the frozen candidate. The current
meadow capture cannot attribute every difference to the palette revision.
Cirrus stretches less along the wind to reduce long perspective streaks.
Marching and reconstruction now share `CLOUD_MAX_DISTANCE_M`.

The existing native capture accepts `--benchmark-orbital-time=<absolute days>`
and optional `--benchmark-orbital-time-end=<absolute days>`. Warmup holds the
start pose. Measurement interpolates to the endpoint over its configured
duration; omitting the endpoint holds the pose. Sun, cloud time and ordinary
gameplay orbital clocks retain their separate existing controls.

For seed 2738958700 near the origin, absolute day 1.4556 places the foundry at
approximately 25 degrees elevation, bearing 204 degrees, 232 km away. A camera
yaw of -2.726 and pitch 0.45 faces that approach. This position was calculated
from the current ephemeris and confirmed by the native approach capture.

R2 build passed, including all 19 composed shaders:
`target/grass-sky-station-r2-build.log`. Frozen candidate and run manifests are
under `target/grass-sky-station-r2/`. The meadow image was inspected. A controlled
8-second partly cloudy foundry approach was recorded at 20 fps in
`foundry-partly-motion/pass.mp4`; its receipt reports 156 real readbacks, three
repeated frames, no late frames and successful encoder finalization. Extracted
frames 1, 4 and 8 show the structure crossing and fading through the cloud veil.
This confirms a real native orbital view rather than only shader compilation;
it does not establish every-frame flicker elimination or every weather/altitude.
The same candidate also completed `foundry-clear-motion/pass.mp4`: 151 real
readbacks, nine repeated frames, no late frames and successful finalization.
Frame 4 of both sequences was inspected: the foundry is darker in the clear
view and blends into the cloud veil in the partly cloudy view. These are the
same orbital path/camera controls, not identical lighting or a performance A/B.
No shader/binding validation error was reported in either run.
Candidate SHA256:
`9093E7BB462786B5D9B9224A5A21516EA0479EAA7714C751C8D0D689E0F95974`.
The earlier `foundry-partly` clip sampled every 0.5 seconds into a 30 fps video
and is time-compressed; use `foundry-partly-motion` for motion review.
