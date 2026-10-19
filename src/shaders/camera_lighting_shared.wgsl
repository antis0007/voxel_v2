struct Camera {
    view_proj : mat4x4<f32>,
    camera_pos : vec4<f32>,
    inv_view_proj : mat4x4<f32>,
    world_params : vec4<f32>,
    prev_view_proj : mat4x4<f32>,
    camera_forward : vec4<f32>,
    ray_right : vec4<f32>,
    ray_up : vec4<f32>,
    previous_camera_pos : vec4<f32>,
    scene_seed : vec4<u32>,
};

@group(0) @binding(0) var<uniform> cam : Camera;

// Projection-scaled basis vectors never contain world translation.
fn camera_ray(ndc: vec2<f32>) -> vec3<f32> {
    return normalize(cam.ray_right.xyz * ndc.x + cam.ray_up.xyz * ndc.y + cam.camera_forward.xyz);
}

struct Lighting {
    sun_dir_intensity : vec4<f32>,
    moon_dir_intensity : vec4<f32>,
    sun_color_daylight : vec4<f32>,
    moon_color_twilight : vec4<f32>,
    ambient_exposure : vec4<f32>,
    sky_top_saturation : vec4<f32>,
    sky_horizon_reserved : vec4<f32>,
    fog_color_density : vec4<f32>,
    fog_params : vec4<f32>,
    weather_globals : array<vec4<f32>, 4>,
    cloud_authoring : array<vec4<f32>, 2>,
    weather_clip_regions : array<vec4<f32>, 3>,
    weather_clip_summaries : array<vec4<f32>, 3>,
    held_torch_pos_radius : vec4<f32>,
    held_torch_color_strength : vec4<f32>,
    torch_meta : vec4<f32>,
    torch_positions : array<vec4<f32>, 64>,
    torch_colors : array<vec4<f32>, 64>,
    material_tints: array<vec4<f32>,256>,
    material_properties: array<vec4<f32>,256>,
    surface_weather: vec4<f32>,
};

@group(1) @binding(0) var<uniform> light : Lighting;

// Shared by opaque terrain and clouds: fading terrain must also release its
// occlusion of the background volume, not merely change its surface colour.
fn terrain_streaming_fade(horizontal_distance: f32) -> f32 {
    if cam.previous_camera_pos.w > 0.5 {
        return smoothstep(12000.0, 16000.0, horizontal_distance);
    }
    let radius = max(cam.world_params.w - 48.0 * cam.world_params.x,
        cam.world_params.w * 0.5);
    return smoothstep(radius * 0.70, radius, horizontal_distance);
}
