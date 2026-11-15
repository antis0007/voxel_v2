//! Files of the world shader program in composition order.
//! Shared with the `art_refiner` tool, which reads them from disk.
pub const WORLD_SHADER_FILES: &[&str] = &[
    "camera_lighting_shared.wgsl",
    "atmosphere_runtime_shared.wgsl",
    "atmosphere_shared.wgsl",
    "cloud_shared.wgsl",
    "local_mist_shared.wgsl",
    "underwater_shared.wgsl",
    "mood_shared.wgsl",
    "weather_clip_shared.wgsl",
    "cloud_weather_sampling.wgsl",
    "terrain_detail_shared.wgsl",
    "enchantment_shared.wgsl",
    "terrain_surface.wgsl",
    "fluid_surface_shared.wgsl",
    "furnishing_surfaces.wgsl",
    "architectural_surfaces.wgsl",
    "gilded_surfaces.wgsl",
    "hell_surfaces.wgsl",
    "realm_light.wgsl",
    "sky_volume_shared.wgsl",
    "world_shader.wgsl",
];
