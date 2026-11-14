# Orbital renderer foundations

Implementation: 2026-11-05. Local compile and runtime receipts are recorded below.

## Engine facilities and game consumers

| Engine facility | Consumer in the game |
| --- | --- |
| engine_core::work: resumable cursor and cooperative work slices | Planetary weather coverage raster; work resumes without repeating finished samples, yields between turns, and observes cancellation |
| engine_core::lod: projected sphere size and hysteretic texture resolution | Orbital cloud bakes select 64–512 samples per face (256 maximum on lean quality) |
| engine_gpu::image: scalar mip chains and array-layer uploads | Opacity-filtered cloud snapshots, cloud shadows, and distant storm coverage |
| engine_gpu::transfer: byte admission | Cloud mip uploads share the existing frame allowance with terrain tiles |
| engine_gpu::spatial: influence bounds and conservative spherical visibility | Settlement and factory night lighting is culled at the horizon and grouped before GPU evaluation; world torch groups share the same influence-bounds utility |
| engine_gpu::atmosphere: spherical optical-depth tables | The sky limb, globe surface and cloud deck use the same sun-path table |
| Core child budgets bounded by their parent | Nested bounded work cannot extend the caller's deadline |

## Planetary weather presentation

Weather snapshots remain eight seconds apart. Wind moves their sample coordinates on the GPU each frame; each snapshot records its own wind origin. Both temporal layers have complete mip chains. Different bake resolutions occupy the matching texture mip instead of reallocating GPU resources. Screen-size hysteresis prevents quality oscillation.

The weather worker checks its one-millisecond work slice every 32 samples, yields for one millisecond between turns, and cancels when dropped. An individual sample and final mip construction are atomic units, so this is cooperative pacing rather than a hard real-time deadline. It coalesces repeated face and light requests. The driver permits one outstanding bake per face and one face upload per frame. New snapshots wait for the previous transition to finish, avoiding CPU reconstruction on the render thread.

Opacity is thresholded before downsampling to preserve fractional cloud coverage at a distance. Ready faces render independently. World changes clear temporal history and readiness; old texture layers are ignored until their new face is ready.

## Industrial and settlement lights

Factory node counts still produce the existing industrial lights. Spherical horizon culling retains influences crossing the limb. Conservative spatial groups reject whole sets before per-light distance, street pattern and falloff work. The per-fragment street hash is evaluated once and reused across lights. The list refreshes after significant orbital travel (at most once per second), independently of the weather cadence, so horizon culling does not leave a new hemisphere dark for thirty seconds.

## Atmospheric transport

A 128 by 64 RG32F table stores Rayleigh and Mie density-weighted metres. It is integrated once using 64 samples per texel. Its horizon-relative quadratic coordinates spend more samples near low altitude and grazing sun directions. Planet blocking remains an analytic intersection, preventing interpolation across the day/night occlusion boundary. The sky, ground and clouds retain the existing coefficients and scattering model.

## Observability

GlobeStats exposes cloud worker CPU milliseconds, pending face count, selected bake resolution, and night-light count. CpuFrameStats carries the cloud values into benchmark history and summary output. Worker CPU time is distinct from render-thread time and GPU timing.

## Verification

- engine_core and engine_gpu compile checks passed, including their final use by the application check.
- All 25 composed WGSL programs passed the existing current-source validator.
- Full application check reached the application and reported no errors in this renderer/engine slice. The first attempt stopped on a concurrent Express Lift tuple change; after that call was corrected in the working tree, the second stopped on an unrelated bed::sleep call missing its new fourth argument at main.rs:4467.
- Check command: cargo check --offline -p minecraft_style_voxel_prototype --bin minecraft_style_voxel_prototype --target-dir target/orbital-foundation-check -j 2.
- Latest shader validation receipt: 25 programs; world_fnv1a64=777b81da3d5c8a27. The working tree has concurrent writers, so this receipt describes that check rather than a frozen release.
- Runtime visuals and frame-time comparison: pending a successful application build. No executable performance gain or visual acceptance is claimed.
- No tests or hosted CI added or run.

## Next foundation work identified

- Share generation/revision invalidation across all derived-data workers; cloud cancellation is currently owned by its renderer adapter.
- Add common scheduling telemetry for terrain, weather, geometry and lighting, with queue age and worst work-unit duration.
- Use measured GPU and transfer pressure to govern distant field updates independently from simulation and gameplay quality.
- Unify the other streamed scalar masks on the mip/upload API after their filtering semantics have been checked.
