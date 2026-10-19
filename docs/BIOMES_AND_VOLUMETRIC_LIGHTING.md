# Biomes and local volumetric lighting

## Source tuning (rebuild after editing)

- `src/shaders/local_mist_shared.wgsl`: the `MIST_*` constants control distance, altitude, density and sample cost. Set `MIST_SAMPLES` to zero to disable mist.
- `src/world/landscape.rs`: biome selection and ground cover; `src/world/trees.rs`: editable tree shapes; `src/render/horizon_vegetation.rs`: distant approximations.
- `src/world.rs`: `CURRENT_GENERATOR_VERSION` controls new worlds and save-version acceptance. Preserve historical numeric gates when adding a new version.
- `src/world/adventure.rs`: `swing_duration()` and `swing_impact_time()` jointly drive combat timing and the held-tool animation.
- Melee reach and sweep use the named metre-based constants at the top of `adventure.rs`. Savanna climate thresholds live in `landscape::savanna_climate()`, shared with distant vegetation; change persisted terrain rules only behind a new generator version.

Launch `run-biomes-volumetric.cmd`. The separate executable leaves an already-running game untouched.

New worlds use terrain generator 10. Existing saves retain their generator version and terrain; create a new world to see the new biome regions. Generator versions 1–10 remain loadable.

- Savanna: warm, moderately dry grassland, bare soil patches, sparse flat-crowned trees.
- Rainforest: warm, wet forest, clustered tall broadleaf canopies and muddy patches.
- Highland heath: cool, relatively dry uplands, sparse low trees and gravel patches.
- Lowland fen: moist, gently sloping lowlands, clay beneath mud and grassy tree islands.

These extend the existing climate classification, material selection and tree profiles. Distant vegetation uses corresponding climate-based density and silhouette variations. The distant ecology remains an approximation rather than a duplicate of individual editable trees. Existing spawn clearing rules remain in place.

Local mist integrates Beer–Lambert extinction and directional sun scattering along twelve stratified samples over at most 96 metres. Terrain endpoints stop the march; cascaded terrain shadows occlude sunlight within the medium. The density falls with world altitude and increases with cloud cover and low sun. Sky pixels share the integration, underwater rendering bypasses it, and distant aerial perspective remains separate.

This is a bounded lowland aerosol effect, not terrain-following fog throughout every mountain valley. Cloud attenuation is approximate over the local segment; it is not a second volumetric cloud march. It adds up to twelve shadow comparisons per shaded pixel. GPU cost, visible noise, motion stability and artistic balance have not been measured in-game.

Validation: local candidate build and composed WGSL validation passed using `cargo build --locked --offline --manifest-path target/biome-volume-manifest/Cargo.toml --target-dir target`. Log: `target/biome-volume-candidate.log`. No tests or runtime review were run.
