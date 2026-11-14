# Irregular volumetric cloud forms — 2026-11-08

The reported problem was regular, smooth ellipsoid silhouettes. The low-cloud
shader built a smooth union of broad ellipsoids and smaller, periodically lobed
thermals. The broad field could override gaps in the smaller field; fine erosion
then decorated the surviving inflated outline. Distance-dependent noise scale
and fading the entire erosion term away reinforced that outline at range.

## Changes

- Removed the broad-bank and thermal-cell geometry from `cloud_morphology.wgsl`.
  Low clouds now use a continuous, advected 3D condensate field, a nonperiodic
  bank warp and smaller-scale entrainment. Cloud coverage still comes from the
  shared weather field. No additional raymarch steps or resolution reduction.
- Kept the condensation floor, convection/storm growth, anvil, weather controls,
  wind, vortex flow and existing lighting. View and light rays use the same new
  low-cloud density function. Existing layer bounds still enclose the medium.
- Kept detail frequency fixed in world space. Distance filters detail instead of
  moving its physical scale. Coarse condensate remains resolved when fine detail
  disappears; erosion retains an average contribution instead of falling to zero.
- The detail sampler requested up to mip 5, but its 32³ texture had only mip 0.
  It now receives a full 6-level chain, adding 18,724 bytes. Shape noise remains
  64³ with its existing 7 levels. Shared generation/upload code moved into
  `render/cloud_noise.rs`, preserving the generated base-level data.
- No exposure, palette, postprocessing, lighting or orbital-shell redesign.
  The shell already has a separate continuous weather/noise representation;
  this change specifically removes the ellipsoid construction in the volume
  used for ground and upper-atmosphere views.

## Numeric density profiling

`cloud_density_profile` samples the production density and generated noise on a
128 × 128 × 32 grid spanning 8 km and an 800 m layer. It covers fair, broken,
overcast and storm inputs. The saved original shader uses its original mip-0-only
detail view; the candidate uses the complete chain. Both use the same inputs,
positions and generated noise. Readbacks contain numbers, not images.

RTX 4090, Vulkan, driver 617.42. Each measurement has 64 warmup and 64 measured
reference/candidate pairs, reversing their order on alternate pairs:

| Sampling case | Original median | Candidate median | Reduction |
|---|---:|---:|---:|
| Near, 2 m footprint | 0.267040 ms | 0.253888 ms | 4.9% |
| Planet horizontal scale, 12 m footprint | 0.245664 ms | 0.185344 ms | 24.6% |
| Far, 64 m footprint | 0.230400 ms | 0.092160 ms | 60.0% |

These are density-kernel timings, not complete cloud passes or game frame rates.
The planet probe isolates the horizontal-scale setting, retaining the same
800 m sampling layer; it is not an orbital image or a full planet scene.

At the near sampling footprint:

| Weather | Original mean density | Candidate mean density | Original/candidate column coverage above density 0.05 |
|---|---:|---:|---:|
| Fair | 0.006685 | 0.008405 | 4.0% / 14.7% |
| Broken | 0.055054 | 0.052258 | 25.5% / 47.5% |
| Overcast | 0.375976 | 0.377729 | 100% / 100% |
| Storm | 0.195269 | 0.230696 | 66.8% / 84.3% |

The broken-cloud case retains about 95% of its integrated density and spreads
it over more irregular regions. Overcast remains a connected deck; storm and
fair-weather material have not been removed to obtain a faster result. All
sampled outputs are finite. The shapes intentionally differ, so pixel/density
equality to the ellipsoid reference is not an acceptance criterion.

The first prototype used two coarse texture samples and substantially increased
cloud mass. It was about 10% slower in the near density profile. The retained
version uses the existing nonperiodic warp with one coarse sample and a revised
condensation threshold. Both results remain in the evidence directory.

Evidence: `target/cloud-form-20261108/`, including the original morphology,
frozen executables, source hashes, component logs and game timing logs. Work is
code-only: no captures or visual inspections, and no tests or hosted CI. Numeric
profiling verifies cost, density and filtering behaviour; it does not establish
artistic acceptance of the rendered result.

## Build and whole-game execution

The final `release-fast` game build passed and validated 28 composed WGSL
programs. The frozen `game-after.exe` was checked for the refined density source
before launch. It ran the 4K hills fixture through completion without a shader
validation error: seed 74, generator 53, partly cloudy, static camera, 12 chunks,
8 s warmup and 12 s measurement. Both runs reported cloud divisor 4 and temporal
history disabled; no quality reduction was requested.

The raw cloud-pass averages were 8.814 ms before and 5.702 ms after. However, the
whole-game A/B is **not an isolated performance comparison**:

- A separate `axiom-lod-streaming-284.exe` renderer benchmark was active during
  the candidate measurement, sharing the GPU.
- Unrelated world-shader work also landed in the shared checkout: the build's
  world fingerprint changed from `fcba67ae9457b09c` to `f1addbc96769ceb8`.
- Terrain averaged 11.068 versus 21.139 ms and total FPS 43.94 versus 29.08.
  Those figures cannot establish this cloud change's whole-frame effect.

Use the paired density profiles above for the isolated cost claim; retain the
game logs as successful runtime execution and explicitly confounded timing data.
SHA-256 values for both frozen games and the refined density profiler are in
`candidate-hashes.txt`. The original cloud source is preserved separately.
