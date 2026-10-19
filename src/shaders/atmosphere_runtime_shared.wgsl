struct AtmosphereRuntime {
    // x: atmosphere mode
    // y: renderer debug surface
    // z/w: current output surface dimensions
    mode_debug_surface : vec4<u32>,
    // x: max transmittance LUT altitude in meters
    // y: reciprocal altitude scale
    transmittance_params  : vec4<f32>,
};

@group(2) @binding(0) var<uniform> atmosphere_runtime : AtmosphereRuntime;
@group(2) @binding(1) var atmosphere_transmittance_lut : texture_2d<f32>;
@group(2) @binding(2) var atmosphere_sky_view_lut : texture_2d<f32>;
@group(2) @binding(3) var atmosphere_multiscatter_lut : texture_2d<f32>;
@group(2) @binding(4) var atmosphere_aerial_perspective : texture_3d<f32>;
@group(2) @binding(5) var atmosphere_sampler : sampler;

const ATMOSPHERE_MODE_ANALYTIC_FALLBACK : u32 = 0u;
const ATMOSPHERE_MODE_TRANSMITTANCE_LUT : u32 = 1u;
const ATMOSPHERE_MODE_SKY_VIEW_LUT      : u32 = 2u;
const ATMOSPHERE_TAU                    : f32 = 6.28318530718;

const DEBUG_VIEW_NONE                     : u32 = 0u;
const DEBUG_VIEW_WEATHER_CLIP_NEAR        : u32 = 1u;
const DEBUG_VIEW_WEATHER_CLIP_MID         : u32 = 2u;
const DEBUG_VIEW_WEATHER_CLIP_FAR         : u32 = 3u;
const DEBUG_VIEW_WEATHER_COMPOSITE        : u32 = 4u;
const DEBUG_VIEW_WEATHER_SHADOW           : u32 = 5u;
const DEBUG_VIEW_CLOUD_ALPHA              : u32 = 6u;
const DEBUG_VIEW_ATMOSPHERE_TRANSMITTANCE : u32 = 7u;
const DEBUG_VIEW_ATMOSPHERE_SKY_VIEW      : u32 = 8u;
const DEBUG_VIEW_ATMOSPHERE_MULTISCATTER  : u32 = 9u;
const DEBUG_VIEW_CLOUD_LAYER_DENSITY      : u32 = 10u;
const DEBUG_VIEW_CLOUD_LAYER_AABB         : u32 = 11u;
const DEBUG_VIEW_CLOUD_STEP_COUNT         : u32 = 12u;
const DEBUG_VIEW_CLOUD_SHADOW_DENSITY     : u32 = 13u;
const DEBUG_VIEW_CLOUD_WEATHER_DECODE     : u32 = 14u;
const DEBUG_VIEW_CLOUD_TEMPORAL_WEIGHT    : u32 = 15u;

fn atmosphere_mode() -> u32 {
    return atmosphere_runtime.mode_debug_surface.x;
}

fn render_debug_view() -> u32 {
    return atmosphere_runtime.mode_debug_surface.y;
}

fn render_surface_size() -> vec2<f32> {
    return vec2<f32>(
        max(f32(atmosphere_runtime.mode_debug_surface.z), 1.0),
        max(f32(atmosphere_runtime.mode_debug_surface.w), 1.0),
    );
}

fn surface_uv_from_frag_coord(frag_coord: vec2<f32>) -> vec2<f32> {
    return clamp(frag_coord / render_surface_size(), vec2<f32>(0.0), vec2<f32>(1.0));
}

fn atmosphere_normalize_xz_or(dir: vec3<f32>, fallback: vec2<f32>) -> vec2<f32> {
    let projected = vec2<f32>(dir.x, dir.z);
    let len = length(projected);
    if len > 1e-4 {
        return projected / len;
    }
    let fallback_len = max(length(fallback), 1e-4);
    return fallback / fallback_len;
}

fn atmosphere_transmittance_uv(sun_height: f32, altitude_m: f32) -> vec2<f32> {
    let mu = clamp(sun_height, -1.0, 1.0);
    let sun_u = 0.5 + 0.5 * sign(mu) * sqrt(abs(mu));
    let altitude_v = sqrt(clamp(
        max(altitude_m, 0.0) * atmosphere_runtime.transmittance_params.y,
        0.0,
        1.0,
    ));
    // Generation includes both endpoints. Address their texel centers.
    let dims = vec2<f32>(textureDimensions(atmosphere_transmittance_lut));
    return (vec2<f32>(sun_u, altitude_v) * (dims - vec2<f32>(1.0)) + 0.5) / dims;
}

fn sample_atmosphere_texture_bilinear(
    tex: texture_2d<f32>,
    uv: vec2<f32>,
) -> vec4<f32> {
    let dims_u = textureDimensions(tex);
    let dims = vec2<f32>(f32(dims_u.x), f32(dims_u.y));
    let max_xy = vec2<i32>(i32(dims_u.x) - 1, i32(dims_u.y) - 1);
    let p = clamp(uv * dims - vec2<f32>(0.5), vec2<f32>(0.0), dims - vec2<f32>(1.0));
    let base = clamp(vec2<i32>(floor(p)), vec2<i32>(0), max_xy);
    let next = min(base + vec2<i32>(1, 1), max_xy);
    let frac = fract(p);

    let c00 = textureLoad(tex, base, 0);
    let c10 = textureLoad(tex, vec2<i32>(next.x, base.y), 0);
    let c01 = textureLoad(tex, vec2<i32>(base.x, next.y), 0);
    let c11 = textureLoad(tex, next, 0);

    let x0 = mix(c00, c10, frac.x);
    let x1 = mix(c01, c11, frac.x);
    return mix(x0, x1, frac.y);
}

fn sample_atmosphere_transmittance_lut_at_uv(uv: vec2<f32>) -> vec3<f32> {
    return sample_atmosphere_texture_bilinear(atmosphere_transmittance_lut, uv).rgb;
}

fn sample_atmosphere_transmittance_lut(sun_height: f32, altitude_m: f32) -> vec3<f32> {
    return sample_atmosphere_transmittance_lut_at_uv(
        atmosphere_transmittance_uv(sun_height, altitude_m)
    );
}

fn sample_atmosphere_sky_view_lut_at_uv(uv: vec2<f32>) -> vec3<f32> {
    return sample_atmosphere_texture_bilinear(atmosphere_sky_view_lut, uv).rgb;
}

fn atmosphere_sky_view_uv(view_dir: vec3<f32>, sun_dir: vec3<f32>) -> vec2<f32> {
    let sun_az = atmosphere_normalize_xz_or(sun_dir, vec2<f32>(1.0, 0.0));
    let view_az = atmosphere_normalize_xz_or(view_dir, sun_az);
    let sun_angle = atan2(sun_az.y, sun_az.x);
    let view_angle = atan2(view_az.y, view_az.x);
    let relative_azimuth = view_angle - sun_angle;
    let azimuth_u = fract(relative_azimuth / ATMOSPHERE_TAU + 0.5 + 1.0);
    // Quadratic elevation allocates most texels to the horizon, where radiance
    // changes fastest. Must match generate_sky_view_lut's inverse mapping.
    let elevation_v = 0.5 + 0.5 * sign(view_dir.y) * sqrt(abs(view_dir.y));
    return vec2<f32>(azimuth_u, elevation_v);
}

fn sample_atmosphere_sky_view_lut(view_dir: vec3<f32>, sun_dir: vec3<f32>) -> vec3<f32> {
    let uv = atmosphere_sky_view_uv(view_dir, sun_dir);
    let dims = vec2<i32>(textureDimensions(atmosphere_sky_view_lut));
    let p = uv * vec2<f32>(dims) - vec2<f32>(0.5);
    let base = vec2<i32>(floor(p));
    let x0 = (base.x + dims.x) % dims.x;
    let x1 = (x0 + 1) % dims.x;
    let y0 = clamp(base.y, 0, dims.y - 1);
    let y1 = clamp(base.y + 1, 0, dims.y - 1);
    let f = fract(p);
    return mix(mix(textureLoad(atmosphere_sky_view_lut, vec2<i32>(x0, y0), 0).rgb,
                   textureLoad(atmosphere_sky_view_lut, vec2<i32>(x1, y0), 0).rgb, f.x),
               mix(textureLoad(atmosphere_sky_view_lut, vec2<i32>(x0, y1), 0).rgb,
                   textureLoad(atmosphere_sky_view_lut, vec2<i32>(x1, y1), 0).rgb, f.x), f.y);
}

fn atmosphere_multiscatter_uv(sun_height: f32, altitude_m: f32) -> vec2<f32> {
    let mu = clamp(sun_height, -1.0, 1.0);
    let uv = vec2<f32>(0.5 + 0.5 * sign(mu) * sqrt(abs(mu)),
        sqrt(clamp(max(altitude_m, 0.0) * atmosphere_runtime.transmittance_params.y, 0.0, 1.0)));
    let dims = vec2<f32>(textureDimensions(atmosphere_multiscatter_lut));
    return (uv * (dims - vec2<f32>(1.0)) + 0.5) / dims;
}

fn sample_atmosphere_multiscatter_lut_at_uv(uv: vec2<f32>) -> vec3<f32> {
    return sample_atmosphere_texture_bilinear(atmosphere_multiscatter_lut, uv).rgb;
}

fn sample_atmosphere_multiscatter_lut(sun_height: f32, altitude_m: f32) -> vec3<f32> {
    return sample_atmosphere_multiscatter_lut_at_uv(
        atmosphere_multiscatter_uv(sun_height, altitude_m)
    );
}

fn sample_atmosphere_aerial_perspective(view_dir: vec3<f32>, distance_m: f32, max_dist_m: f32) -> vec4<f32> {
    let azimuth = atan2(view_dir.x, view_dir.z);
    
    let u = fract(azimuth / ATMOSPHERE_TAU + 0.5 + 1.0);
    let v = view_dir.y * 0.5 + 0.5;
    // Clouds extend beyond the terrain draw distance. Keep this domain in sync
    // with generate_aerial_perspective; the first depth slice is exactly zero.
    let depth = sqrt(clamp(distance_m / max(max_dist_m, 16000.0), 0.0, 1.0));
    let slices = f32(textureDimensions(atmosphere_aerial_perspective).z);
    let w = (depth * (slices - 1.0) + 0.5) / slices;
    
    return textureSampleLevel(atmosphere_aerial_perspective, atmosphere_sampler, vec3<f32>(u, v, w), 0.0);
}

fn atmosphere_debug_transmittance_color(surface_uv: vec2<f32>) -> vec3<f32> {
    let lut_uv = vec2<f32>(
        clamp(surface_uv.x, 0.0, 1.0),
        clamp(1.0 - surface_uv.y, 0.0, 1.0),
    );
    return clamp(
        sample_atmosphere_transmittance_lut_at_uv(lut_uv),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );
}

fn atmosphere_debug_sky_view_color(surface_uv: vec2<f32>) -> vec3<f32> {
    let lut_uv = vec2<f32>(
        clamp(surface_uv.x, 0.0, 1.0),
        clamp(1.0 - surface_uv.y, 0.0, 1.0),
    );
    return max(sample_atmosphere_sky_view_lut_at_uv(lut_uv), vec3<f32>(0.0));
}

fn atmosphere_debug_multiscatter_color(surface_uv: vec2<f32>) -> vec3<f32> {
    let lut_uv = vec2<f32>(
        clamp(surface_uv.x, 0.0, 1.0),
        clamp(1.0 - surface_uv.y, 0.0, 1.0),
    );
    return max(sample_atmosphere_multiscatter_lut_at_uv(lut_uv), vec3<f32>(0.0));
}
