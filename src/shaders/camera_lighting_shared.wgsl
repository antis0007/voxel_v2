struct Camera {
    view_proj : mat4x4<f32>,
    camera_pos : vec4<f32>,
    inv_view_proj : mat4x4<f32>,
    world_params : vec4<f32>,
    prev_view_proj : mat4x4<f32>,
    camera_forward : vec4<f32>,
};

@group(0) @binding(0) var<uniform> cam : Camera;

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
    torch_positions : array<vec4<f32>, 16>,
    torch_colors : array<vec4<f32>, 16>,
};

@group(1) @binding(0) var<uniform> light : Lighting;
