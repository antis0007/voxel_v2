# Cloud flow integration

The volumetric cloud renderer and orbital shell now compose
`cloud_flow_shared.wgsl`. It contains the existing Karman shedding, gust and jet
warps, Kelvin-Helmholtz lift, and wind-aligned cloud-street mask. Surface and
orbital adapters supply wind and the same sky clock including advection.

| Feature | Surface clouds | Orbital shell |
|---|---|---|
| Karman shedding | Low/mid density coordinates | Coverage and shadow coordinates |
| Gust flow | Low/mid density coordinates | Coverage and shadow coordinates |
| Jet meanders | High cloud coordinates | No separate high deck |
| Kelvin-Helmholtz lift | Low cloud shape and texture | Flat shell cannot show vertical lift |
| Cloud streets | Density and shadows | Coverage and shadows |
| Mammatus / anvils | Low cloud morphology | No volumetric underside/towers |
| Lenticular / cirrus forms | Mid/high density | No distinct mid/high morphology |

The recovered `engine_worldgen::sphere::WeatherSim` is a separate pressure,
Coriolis, temperature and moisture solver. `CloudSky` adds tracked frontal and
tropical storm templates, dry slots, eyes and spiral bands. Concurrent work in
`render/globe/shell_sim.rs` and `shell_worker.rs` connects these to the orbital
bake. The surface clipmap still uses `world/weather/synoptic.rs`; do not describe
the recovered solver as driving both renderers until that adapter exists.

Karman obstacle sites remain procedural hashes, as in the original code; this
does not add terrain-driven island wakes. Orbital chart overlap blends the
flow across cube edges. Lightning, red sprites and auroras retain their existing
rendering paths; this change does not unify their independent event schedules.

The host shell uniform and WGSL binding are both 272 bytes. Build-time Naga
validation covers the composed surface and orbital shaders. Validation is not
visual or performance acceptance.
