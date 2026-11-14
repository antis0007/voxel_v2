# Rendering, interface and infrastructure review · 7 November 2026

The visual target is a well-lit modern renderer with the legibility and deliberate
composition of 1990s management games. Nostalgia belongs in the interface, material
language, silhouettes and practical systems. It is not a reason to lower cloud
resolution, crush the lighting palette or obscure distant detail with a screen pattern.

## Dither: measured work, not a performance claim about noise

Native rendering previously resolved HDR lighting into another full-resolution
RGBA16F texture, then sampled that texture in a second fullscreen pass to add one-code
blue-noise dither before the final surface write. The native path now combines those
operations. The original two-pass path remains available for scaled rendering and
`--benchmark-reference-resolve` comparisons. Lighting remains HDR; the optional palette
and contour effects are unchanged. Dither itself does not save rendering work.

The production postprocessor was measured with `postprocess_capture` on an RTX 4090
(Vulkan, driver 617.42), 96 warmup and 96 measured GPU frames per run. Three interleaved
runs per path used the same scene, effects and resolution. Median of run medians:

| Resolution | Two-pass GPU time | Combined GPU time | Saving |
| --- | ---: | ---: | ---: |
| 1920 × 1080 | 0.113664 ms | 0.107520 ms | 5.4% |
| 3840 × 2160 | 0.442368 ms | 0.407552 ms | 7.9% |

The fixture includes smooth sky/HDR gradients, one-pixel features, emissive colours
and depth transitions. At 1080p the maximum output difference was one 8-bit code;
mean RGB difference was 0.0204 codes. With the existing magical presentation effect
enabled, maximum difference was also one code (mean 0.0206). Debug output matched
exactly. These are postprocessing savings, not a measured whole-game FPS increase.
GPU frame totals now include this pass, which the old counters omitted.

Evidence: `target/dither-audit-20261107/resolve-*.log`, `compare-*.log`, and the paired
PNGs. The production build and the offscreen capture executable were compiled locally;
no automated tests or hosted CI were run for this review.

## Orbital clouds

The original 1,600 m shell and 460 m relief are conspicuous on the compact planet.
At orbital distances the relief must read as a thin weather layer rather than
raised terrain. Sampling now intersects the smooth sphere and feathers coverage
into the atmospheric limb; near-cloud flight retains the volumetric handoff.

Broad density and local wind share the weather catalogue. Cached RGBA faces contain
coverage and body-space velocity; each temporal snapshot is advected from its own
time. A second zonal displacement no longer creates duplicate orbital fronts.
Clearing highs rotate opposite the lows. Fine, filtered, advected detail is evaluated
at screen resolution rather than enlarging coarse weather texels. Terrain-obstacle
wakes remain connected to the shared Kármán street field.

This is an efficient procedural weather representation, not a global numerical
weather prediction model. The pressure/moisture solver supplies broad anomalies;
tracked systems supply fronts and circulation. The latest changes received source
and build validation only: the user requested that capture/visual review stop.

Validation state: `build-code-final.log` records a successful release-fast build
before the last coverage normalization. `build-final-validation.log` validates all
27 composed shader programs, then stops in concurrent geology work: missing
`planetary/drift/episodes.rs` and unresolved `surface_step_resolved`. That work was
left intact. The latest Rust changes therefore do not yet have a completed final
application build. No tests or further native inspections were run after the user
requested code-only work.

The upper layer used to sit near 7.5 km on a planet with a 3 km atmosphere. Planet
weather now preserves heights below 1.6 km and compresses the higher layers into the
atmosphere. Both global march bounds and per-cell bounds use the same mapping.
The volume remains active through the complete 3.2–6 km handoff; silhouette clipping
starts above the actual highest cloud, so it no longer erases cirrus overhead.

Above the orbital transition, the existing optical-depth table and a single
density-weighted sunlight evaluation replace the six/ten-step air integral on each
cloud fragment. Near flight retains that integral. Detail uses a cached 256 KiB
volume, with footprint filtering and no lower-resolution orbital render target.
Identical weather uploads no longer restart an unnecessary two-snapshot blend.
Final cloud performance and appearance are not claimed as measured acceptance.

### Settings and definition

The settings are a global artistic baseline, with regional weather and terrain
modulation; they are not a brush limited to the player's immediate surroundings.
Orbit integrates the local layers into a representative thin cloud column.

| Setting | Orbital treatment |
| --- | --- |
| Coverage | Changes occupied area within the regional fronts; zero removes that area. |
| Density, thickness, storm thickness | Beer–Lambert column opacity, evaluated during the cached bake. |
| Cloud-type amounts | Column opacity; stratus preserves sheets, cumulus allows stronger breakup, ice-rich columns stretch into fibres. |
| Erosion, detail, noise scale | Native-pixel edge erosion and detail; dense interiors remain connected. |
| Wind, shear, advection | Shared local drift and obstacle wakes; catalogue circulation also moves systems and detail in body space. |
| Layer heights/separation | Traversable volume geometry; orbital rendering uses a representative integrated shell rather than marching each layer. |

The definition fix normalizes coverage once for both consumers. Previously the
surface interpreted a density biased by 0.58 as coverage, while orbit thresholded
it again. Surface cloud amount and shadows now follow the same normalized front.
Orbital opacity also respects density and layer amounts, which were previously
largely absent from the orbital appearance. These changes do not turn the artistic
controls into meteorological inputs to a full atmosphere simulation.

### Water repetition

The globe now derives its wave footprint from projected surface derivatives,
including the grazing limb. Unresolved coherent wave normals transfer their energy
to reflection roughness; the distant path skips four wave trains and fine foam
noise. Close water uses one continuous world-space warp for its shared displacement
and normal packets, replacing nested periodic phase warps without a tile UV reset.

Reference targets: [NASA's Pacific Northwest cyclone](https://science.nasa.gov/earth/earth-observatory/extratropical-cyclone-whips-over-the-pacific-northwest-153605/)
for comma heads, dry slots and fine cloud texture, and
[NASA's nighttime swirls](https://science.nasa.gov/earth/earth-observatory/nighttime-swirls-145471/)
for alternating island wakes. Orbital clouds should not share a pixel-art filter with
the interface.

## Interface findings

| Priority | Finding | Treatment |
| --- | --- | --- |
| High | Orbital and Statistics tabs used the same resource icon. | Distinct orbit and chart glyphs, drawn with the existing pixel-icon language. |
| High | Nearby production scope can hide global rates and power behind an empty local state. | Added an All Rates control; global readings keep their global scope. Invalid stored outpost selections return to Nearby. |
| Medium | Stock amounts are embedded in prose-like rows. | Added an icon/name ledger with right-aligned quantities; retained filtering. |
| Medium | Machine state, input/output, units and bottleneck access should be consistent across screens. | Keep input left, progress centre, output right; show actual capacity and rates. Use existing UI_RULES.md rather than a second theme. |
| Medium | Route information needs destination and service state. | Expose real infrastructure records before adding decorative route overlays. |
| Low | Some postprocess sampling uses a half-pixel offset in addition to the fragment centre. | Separate sampling-alignment work from the matched dither optimisation; changing it would invalidate that comparison. |

Preserve the existing inventory animation, magical effects, lighting and palettes.
Keep idle HUD content restrained. Use small bevels, distinct glyphs, aligned numbers,
concise state labels and contextual inspection. Dense information belongs in a useful
management panel, not over the landscape. The relevant inspiration is the practical
readability of [Chris Sawyer's games](https://www.chrissawyergames.com/features.htm),
with modern contrast, scaling and input behaviour.

## Public utilities and the palimpsest

Every generated utility needs a purpose, endpoints, usable access and a continuous
route before abandonment is applied. Damage must explain a lost connection. A
destination bearing, a random broken girder or a pipe vanishing at a parcel boundary
does not satisfy that contract. The same rule applies to an occupied medieval
settlement, a planned new city and an abandoned high-tech district.

The existing ruins line selector advertised a downstream tree while allowing an
upstream fallback; it selected unsurveyed district candidates. Its rendered track
stopped at the district boundary, and the route hint extended only 600 m beyond it.
That is a bearing hint, not a physical regional railway. Boarding shelters were also
conditional on terracing, although access stairs already existed elsewhere.

A valid local service between actual stations is preferable to representing an
unbuilt regional connection as operational. Regional routes require shared endpoint
records, a surveyed alignment, grades/clearance, bridge or tunnel decisions, station
access and surviving right-of-way. The civilization implementation brief remains a
plan for the wider simulation; decorative generation must not claim that simulation
already exists.

Implemented: two stations on rebuilt local viaducts and A-frame routes, outboard
boarding platforms with clear running rails, stair landings, reserved paved links
to real streets, and buffer stops at the outer termini. Interior damage keeps its
failed-span evidence. Station endpoints are exposed from the same geometry kit.
The phantom 600 m railway bonus now uses only actual surveyed street approaches.

Medieval home navigation and generated access use the same `AccessWay` endpoints;
work paths reach field edges without paving over their interiors. Streets and door
paths survive abandonment alongside foundations. This is shared physical-access
geometry, not a second owner for the ongoing civilization simulation.

Still required for regional rail service: a surveyed inter-settlement alignment,
grade/clearance checks, bridge/tunnel decisions and continuity records. No new
regional railway, power network or water network is claimed by this change. Already
persisted voxel chunks are not regenerated by these source changes.

## Further pipeline and streaming costs

- Terrain weather filtering now shares the cloud sampler's hardware path on
  adapters with `FLOAT32_FILTERABLE`. Each bilinear lookup uses one filtered
  sample instead of four explicit loads and interpolation arithmetic. Float32
  fields, texel-centre clamping and the unsupported-device fallback remain.
  Both world shader variants have independent build/runtime source receipts.
- Held items no longer execute three voxel shadow walks only to discard their
  result through `select`. Unplated orbital geometry also skips the unused
  plated-surface calculation. Lighting equations and actual world shadows stay
  the same.
- The final resolve retains its input bindings until scene/depth resize and
  writes settings only when they change. Acquiring another display image does
  not invalidate those bindings.
- Background simulation discovery scans at most 64 visible entries per frame,
  using a rotating cursor and constant-time pending-job lookup. Previously a
  settled view scanned the entire resident set each frame. Discovery still
  reaches every eligible chunk; edits, cancellation and world reset keep their
  existing ownership rules.
- Clouds Off also stops new orbital weather bakes and texture uploads. Already
  running jobs drain, and the shared worker continues settlement night lights.
- Default and Balanced voxel range increase from 8 to 12 chunks: 256 to 384 m
  with metre voxels. Full detail remains through ring 5; ring 6–8 retains 2 m
  detail; the added outer rings use 4 m cells. Existing hysteresis, complementary
  transitions, complete-column publication and memory limits remain. Saved
  custom preferences and the integrated/minimum presets are preserved.

The headless `weather_filter_profile` example measures the exact production
sampler with four field lookups per pixel, alternating both variants at the
same resolution. It reports GPU timestamps and numeric float differences;
it opens no window and saves no images. This isolates sampling cost, not total
terrain cost or game FPS. `--benchmark-reference-weather-filtering` selects the
four-tap world shader for a future matched whole-game comparison.

Measured on RTX 4090, Vulkan, driver 617.42: three runs, each with 64 alternating
warmup pairs and 64 measured pairs. Median of the three run medians:

| Output | Four explicit taps | Hardware filtering | Component saving |
| --- | ---: | ---: | ---: |
| 1920 × 1080 | 0.037792 ms | 0.029696 ms | 21.4% |
| 3840 × 2160 | 0.177152 ms | 0.168960 ms | 4.6% |

Maximum absolute output difference was 0.000125408; mean was about 0.00001208.
The fixture writes RGBA32F, so target bandwidth is substantial at 4K. Its 4K
tail timings were noisy; no tail-latency or whole-game FPS improvement is claimed.
Evidence: `target/pipeline-streaming-20261107/weather-*.log`; frozen profiler
SHA256 `BD1D3211B9116A798667CBBD1FB9C141635CE913D82F06736DE1AD3498F47AE6`.

The application build in `build.log` passed, including all 28 composed shader
variants. A single `f64` closure-argument annotation repaired an unrelated
compile error in the concurrently edited tectonics code. The subsequent build
in `build-profile.log` built the profiler and again validated all shaders, but
the game target stopped on a newer unrelated `DriftTexel.winter_c` reference in
`egui_ui/world_loader/geology.rs:89`. No tests, hosted CI or visual inspections
were run. Streaming scan bounds and the longer default range are source/build
verified; their overall frame-rate and travel/loading effects are not measured.
