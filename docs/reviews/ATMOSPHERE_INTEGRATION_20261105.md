# Planet atmosphere integration

References:

- Hillaire, [A Scalable and Production Ready Sky and Atmosphere Rendering Technique](https://sebh.github.io/publications/egsr2020.pdf), and the author's [reference integrator](https://github.com/sebh/UnrealEngineSkyAtmosphere/blob/master/Resources/RenderSkyRayMarching.hlsl).
- Bruneton, [Precomputed Atmospheric Scattering implementation](https://ebruneton.github.io/precomputed_atmospheric_scattering/atmosphere/functions.glsl.html).

Changes:

- Integrate each constant-medium segment analytically using its extinction;
  retain a small-optical-depth series to avoid cancellation in thin air.
- Concentrate samples on both sides of the ray's closest approach to the
  planet. Ascending, descending and grazing rays keep the full segment covered.
- Attenuate airglow through intervening air and skip direct scattering in
  planetary shadow. Retain existing coefficients and maximum sample counts.
- Compose the spherical sky as scattered light plus transmittance times the
  background, so stars and celestial bodies dim through the limb. This reuses
  the existing march result without extra density samples or LUT reads.
- Use one filtered RG32F depth lookup on `FLOAT32_FILTERABLE` devices. Other
  devices retain four manual loads; endpoints map to the same texel centers.
- Give globe-only rendering the body/eye frame needed by the spherical sky,
  independently of the terrain mesh projection flag. This removes the flat
  blue ground-sky background from orbit. Fade the ground LUT fully at air top.

Both sky/globe filtering variants are covered by the ordinary build-time WGSL
validator (27 composed programs). Native capture evidence lives under
`target/review/atmosphere-integration-20261105/`. The baseline executable predates
other concurrent renderer changes, so whole-frame differences and FPS are not
an isolated measurement of this patch.

Native review builds use the frozen `source/` copy in that evidence directory
and `target/atmosphere-native-20261105` for artifacts. Review-only compile repairs:

- Make `goals_hud::tracker` crate-visible.
- Adapt an unfinished site biome lookup to `landscape::sample` and match the
  homestead cache's `Option` type.
- Give the two new biome variants existing detail profiles. The captures use
  generator 25, not these new biome paths.

Those repairs are confined to the frozen copy; active UI/worldgen files are
not changed by this atmosphere pass.

This remains a single-scattering orbital model with the existing ground LUTs;
it does not claim the complete Hillaire or Bruneton multiple-scattering model.

Final native evidence: `final/{ground,orbit,high-orbit,dusk,night}` all completed
with exit 0 and were visually inspected. The final executable is
`after-final.exe`, SHA-256
`9EDA835B568F1E53F2A6E2FEFD6911BA9E3801863C7A788F967F4992B333DEF7`.
The ground sky remains consistent; orbital views show space behind the blue
limb, a lit/dark transition at dusk, and a dim night-side limb. A previous
high-orbit launch failed during GPU startup; its logs and failure receipt are
preserved, and the unchanged candidate succeeded on retry.

The active planet-air shader, sky shader and shader assembly match the frozen
copy. `cel_limb` also matches; the rest of `celestial_sky.wgsl` has concurrent
Hell-sky edits outside this review. No FPS improvement is claimed: presentation
timeouts and concurrent activity make these capture timings unsuitable.
