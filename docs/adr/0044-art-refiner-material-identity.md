# 0044: Material identity in the surface refiner

Status: accepted, 8 November 2026.

The refiner passes the content registry's material ID alongside texture kind.
Industrial surfaces share kind 12 with masonry but dispatch by material ID.
A 48-byte tile record keeps that ID in the same CPU/WGSL contract.

The refiner adds the existing workspace `engine_gpu` crate as a test-only
dependency so its ignored shader-compilation/render test uses the required
shared adapter and respects `VOXEL_TEST_ADAPTER_NAME`. No new external package
is introduced. Four-band image rows and captions share the same host stride.

This tool is a surface diagnostic. It does not reproduce gameplay shadows,
ambient occlusion, torch illumination, material-studio overrides, or tonemapping.
