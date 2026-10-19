// Analytic local aerosol, bounded in world metres. Zero disables for A/B.
// No shadow map lookups or sample loops in overdrawn terrain fragments.
const MIST_STRENGTH: f32 = 1.0;
const MIST_RANGE_M: f32 = 180.0;
const MIST_NEAR_CLEARANCE_M: f32 = 2.0;
const MIST_BASE_HEIGHT_M: f32 = 12.0;
const MIST_HEIGHT_FALLOFF_M: f32 = 22.0;
struct CascadeData {
    light_view_proj: mat4x4<f32>,
    texel_size: f32,
    base_bias: f32,
    slope_bias: f32,
    far_plane: f32,
};

struct ShadowUniform {
    cascades: array<CascadeData, 4>,
    cascade_count: u32,
    blend_width: f32,
    world_scale: f32,
    _padding: u32,
};

@group(3) @binding(0) var<uniform> shadow       : ShadowUniform;
@group(3) @binding(1) var shadow_map            : texture_depth_2d_array;
@group(3) @binding(2) var shadow_sampler        : sampler_comparison;
@group(3) @binding(3) var cloud_shadow_tex      : texture_2d_array<f32>;
@group(3) @binding(4) var cloud_field_sampler   : sampler;

// Exact mean density along a linear segment through a constant-density bottom
// and exponential atmosphere above it. Stable limits for horizontal rays and
// crossing rays prevent a camera-following slab or a hard height boundary.
fn mist_mean_density(y0: f32, y1: f32) -> f32 {
    let a = (y0 - MIST_BASE_HEIGHT_M) / MIST_HEIGHT_FALLOFF_M;
    let b = (y1 - MIST_BASE_HEIGHT_M) / MIST_HEIGHT_FALLOFF_M;
    if max(a,b) <= 0.0 { return 1.0; }
    if min(a,b) >= 0.0 {
        let delta = abs(b-a);
        if delta < 0.001 { return exp(-(a+b)*0.5); }
        return (exp(-min(a,b)) - exp(-max(a,b))) / delta;
    }
    let high = max(a,b);
    let low = min(a,b);
    return (-low + 1.0 - exp(-high)) / max(high-low,0.001);
}

// This models diffuse lowland haze, not shadowed light shafts. The previous
// 12-step shadow march cost 102 ms in the terrain pass due to overdraw.
fn local_mist(ray: vec3<f32>, distance_m: f32, sky_access: f32, environment: vec3<f32>) -> vec4<f32> {
    let length_m = min(max(distance_m-MIST_NEAR_CLEARANCE_M,0.0),MIST_RANGE_M);
    if MIST_STRENGTH == 0.0 || cam.scene_seed.z != 0u || length_m < 0.1 || cam.camera_pos.w > 0.5 || sky_access < 0.001 {
        return vec4<f32>(0.0,0.0,0.0,1.0);
    }
    let y0 = cam.camera_pos.y+ray.y*MIST_NEAR_CLEARANCE_M;
    let y1 = y0+ray.y*length_m;
    let density = mist_mean_density(y0,y1);
    let sun = normalize(light.sun_dir_intensity.xyz);
    let cover = clamp(light.weather_clip_summaries[0].y,0.0,1.0);
    let low_sun = 1.0-smoothstep(0.08,0.45,sun.y);
    let extinction = 0.00014+cover*0.00085+low_sun*0.00032;
    let optical_depth = min(extinction*length_m*density*sky_access*MIST_STRENGTH,0.22);
    let transmission = exp(-optical_depth);
    // Use the same atmospheric radiance as terrain/sky; no independent gray
    // palette or unshadowed directional term that brightens enclosed spaces.
    // Caller reuses its existing sky radiance. Re-evaluating the atmosphere in
    // this overdrawn geometry path caused a severe low-sun shader-cost spike.
    return vec4<f32>(environment*(1.0-transmission),transmission);
}
