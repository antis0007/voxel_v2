# Sculpted terrain, lighting and streaming increment

## Implemented

- Mesh requests own an `Arc<ResidentChunk>` until construction finishes. Evicting the CPU cache no longer makes an admitted request lose its source. Worker-held arrays stay charged until their last owner releases them.
- Packed directional occupancy masks find exposed faces before patch planning. Fully submerged water interiors bypass meshing; full water surfaces merge into greedy patches. Partial fluid surfaces retain their volume-dependent path.
- Output reservations derive from the planned patches rather than every visible unit face. Planning is serialized under a permit, then output construction runs in parallel. The 24 MiB scratch allowance covers patch-vector growth, masks, shared-corner data and bounded vertex deduplication. Dense torch chunks retain all handles and light positions with compact emissive geometry, rather than exceeding the minimum worker budget.
- Terrain generation has a bounded reservation window taken out of CPU source residency. Pending terrain arrays/activity masks are reported on the CPU side; they are no longer counted as GPU buffers or reported as zero. Edited-region storage and mesh output have separate existing allowances.
- Natural material boundaries use shared relaxed corners derived from the surrounding 3D occupancy. Disconnected solid/air configurations stay pinned. Authored and structural neighbours pin positions and hard normals. Chunk boundaries keep identical canonical edge vertices.
- Natural materials use continuous metre-space shading and the generator's seeded climate field. Smooth slope/climate transitions replace voxel-top turf stripes. Sand ripple amplitude integrates the pixel footprint; stone bedding is continuous. Daytime diffuse sky fill is stronger, and sun occlusion no longer incorrectly suppresses the existing diffuse fill term.
- Nearby screen-space diffuse exchange gathers lit surface colors at half resolution and resolves with depth-aware filtering. Ambient obscurance leaves direct sunlight and emission intact. It uses a 5 m neighbourhood and fades out by 80 m, with early exits for sky and distant pixels. Separate GPU timing includes both passes.
- A persistent **Diffuse bounce and contact shading** control permits disabling that extra GPU work. Preferences format 4 reads formats 1-3 without discarding existing settings.
- The former 64-pixel interleaved-gradient cloud jitter lattice is replaced by a 256-pixel ranked high-pass hash field. Per-segment phases break coherent march patterns. Terrain/backdrop coverage also uses a hash instead of the repeating diagonal lattice. Cloud quality now honors the selected divisor at all resolutions; the former 360-pixel minimum silently made all three presets identical at 720p. High, Balanced and Fast trace 1/4, 1/9 and 1/16 of full-resolution pixels respectively. Concurrent cloud work in the checkout retains its depth-aware resolve and bounded layer budgets.

## Limits and remaining work

The relaxed mesh is a visual surface over the existing voxel simulation. Displacement is bounded to 0.3 voxel; collision, picking and material state still use voxel cells. This is not an arbitrary-resolution signed-distance collision system. Structural/material connectivity, caves and overhangs remain volumetric, but distant LOD simplification is still conservative and surface-dependent. Foliage remains voxel geometry.

The new diffuse exchange is one screen-space bounce, not offscreen or multi-bounce global illumination. Visibility changes can change its contribution. It deliberately has no accumulated temporal history. Its two render targets use about 20.7 MB at 1920x1080 (eight bytes per full-resolution receiver pixel and eight per half-resolution gather pixel), outside world residency. Disabling it skips the passes; the targets remain allocated.

The climate palette softens visible transitions; it does not replace biome ecology, watershed generation or the exploration-site grammar. Large-scale geological variety and more aggressive topology-aware mesh simplification remain substantive work. The distant heightfield backdrop is presentation coverage, not a substitute for resident editable cave/build geometry.

## Verification and review artifact

Local `cargo check --offline --target-dir target` passes, including the build's composed WGSL compilation. No tests or CI were added or run. Diagnostics: `target/sculpted-final-check.log`. The offline optimized build also passes: `target/sculpted-world-release-frozen.log`. The executable is `target/sculpted-world-review/voxel-workshop.exe`, launched from the repository with `run-sculpted-world.cmd`. It was compiled from the unchanged 111-file snapshot under `target/sculpted-world-review/source`; SHA-256 inputs and the binary receipt are stored beside it. The snapshot isolates the delivered version from continued source changes in other chats. The existing running executable was not overwritten.

The already-running game was left alone. This increment has no fresh renderer capture, FPS comparison or low-memory flight acceptance; compilation does not establish those outcomes. Existing review captures predate this increment.
