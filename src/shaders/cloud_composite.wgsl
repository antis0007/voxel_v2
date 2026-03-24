@group(0) @binding(0) var<uniform> light : Lighting;

@group(1) @binding(0) var cloud_tex : texture_2d<f32>;
@group(1) @binding(1) var cloud_sampler : sampler;
@group(3) @binding(0) var weather_primary_tex : texture_2d_array<f32>;
@group(3) @binding(1) var weather_secondary_tex : texture_2d_array<f32>;
@group(3) @binding(2) var weather_sampler : sampler;
@group(3) @binding(7) var cloud_shadow_tex : texture_2d_array<f32>;

fn weather_clip_region(level: i32) -> vec4<f32> {
    return light.weather_clip_regions[level];
}

fn weather_uv(world_xz: vec2<f32>, level: i32) -> vec2<f32> {
    let clip = weather_clip_region(level);
    return (world_xz - clip.xy) * clip.w;
}

fn weather_edge_distance(uv: vec2<f32>) -> f32 {
    return min(min(uv.x, uv.y), min(1.0 - uv.x, 1.0 - uv.y));
}

fn weather_level_weight(uv: vec2<f32>, level: i32) -> f32 {
    if level == 2 {
        return 1.0;
    }
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
        return 0.0;
    }
    return smoothstep(0.08, 0.22, weather_edge_distance(uv));
}

fn sample_weather_bilinear(tex: texture_2d_array<f32>, uv: vec2<f32>, level: i32) -> vec4<f32> {
    let dims_u = textureDimensions(tex);
    let dims = vec2<f32>(f32(dims_u.x), f32(dims_u.y));
    let max_xy = vec2<i32>(i32(dims_u.x) - 1, i32(dims_u.y) - 1);
    let p = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * dims - vec2<f32>(0.5);
    let base = clamp(vec2<i32>(floor(p)), vec2<i32>(0), max_xy);
    let next = min(base + vec2<i32>(1, 1), max_xy);
    let frac = fract(p);

    let c00 = textureLoad(tex, base, level, 0);
    let c10 = textureLoad(tex, vec2<i32>(next.x, base.y), level, 0);
    let c01 = textureLoad(tex, vec2<i32>(base.x, next.y), level, 0);
    let c11 = textureLoad(tex, next, level, 0);

    let x0 = mix(c00, c10, frac.x);
    let x1 = mix(c01, c11, frac.x);
    return mix(x0, x1, frac.y);
}

fn debug_world_xz(surface_uv: vec2<f32>) -> vec2<f32> {
    let far_clip = light.weather_clip_regions[2];
    return far_clip.xy + surface_uv * far_clip.z;
}

fn debug_weather_composite(world_xz: vec2<f32>, show_shadow_field: bool) -> vec3<f32> {
    let uv0 = weather_uv(world_xz, 0);
    let uv1 = weather_uv(world_xz, 1);
    let uv2 = weather_uv(world_xz, 2);
    let weight0 = weather_level_weight(uv0, 0);
    let weight1 = weather_level_weight(uv1, 1) * (1.0 - weight0);
    let weight2 = max(1.0 - weight0 - weight1, 0.0);
    var combined = vec4<f32>(0.0);
    if !show_shadow_field {
        combined = sample_weather_bilinear(weather_primary_tex, uv0, 0) * weight0
            + sample_weather_bilinear(weather_primary_tex, uv1, 1) * weight1
            + sample_weather_bilinear(weather_primary_tex, uv2, 2) * weight2;
    }
    if show_shadow_field {
        let shadow = sample_weather_bilinear(cloud_shadow_tex, uv0, 0) * weight0
            + sample_weather_bilinear(cloud_shadow_tex, uv1, 1) * weight1
            + sample_weather_bilinear(cloud_shadow_tex, uv2, 2) * weight2;
        return vec3<f32>(shadow.x, shadow.y * 0.7 + shadow.w * 0.3, shadow.z + shadow.w * 0.25);
    }
    return vec3<f32>(combined.x, combined.z, combined.y);
}

fn debug_weather_level(world_xz: vec2<f32>, level: i32) -> vec3<f32> {
    let uv = weather_uv(world_xz, level);
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
        return vec3<f32>(0.02, 0.02, 0.02);
    }
    let primary = sample_weather_bilinear(weather_primary_tex, uv, level);
    let secondary = sample_weather_bilinear(weather_secondary_tex, uv, level);
    let border = 1.0 - smoothstep(0.02, 0.05, weather_edge_distance(uv));
    let base = vec3<f32>(primary.x, secondary.x, primary.z);
    return mix(base, vec3<f32>(1.0, 0.4, 0.1), border * 0.65);
}

struct VsOut {
    @builtin(position) pos : vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var out: VsOut;
    let pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 3.0,  1.0),
    );
    out.pos = vec4<f32>(pos[vi], 0.0, 1.0);
    return out;
}

fn aces_tonemap(color: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp(
        (color * (a * color + vec3<f32>(b))) / (color * (c * color + vec3<f32>(d)) + vec3<f32>(e)),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );
}

fn saturate_color(color: vec3<f32>, amount: f32) -> vec3<f32> {
    let luma = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    return mix(vec3<f32>(luma), color, amount);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // This pass writes to the full-resolution surface while the cloud target is
    // rendered at half resolution. Use surface UVs here so the upsampled sample
    // footprint is stable regardless of the cloud texture dimensions.
    let surface_uv = surface_uv_from_frag_coord(in.pos.xy);
    let cloud = textureSample(cloud_tex, cloud_sampler, surface_uv);
    let alpha = clamp(cloud.a, 0.0, 0.998);
    let debug_mode = render_debug_view();

    if debug_mode == DEBUG_VIEW_ATMOSPHERE_TRANSMITTANCE {
        return vec4<f32>(atmosphere_debug_transmittance_color(surface_uv), 1.0);
    }

    if debug_mode == DEBUG_VIEW_ATMOSPHERE_SKY_VIEW {
        return vec4<f32>(atmosphere_debug_sky_view_color(surface_uv), 1.0);
    }

    if debug_mode == DEBUG_VIEW_ATMOSPHERE_MULTISCATTER {
        return vec4<f32>(atmosphere_debug_multiscatter_color(surface_uv), 1.0);
    }

    let world_xz = debug_world_xz(surface_uv);
    if debug_mode == DEBUG_VIEW_WEATHER_CLIP_NEAR {
        return vec4<f32>(debug_weather_level(world_xz, 0), 1.0);
    }
    if debug_mode == DEBUG_VIEW_WEATHER_CLIP_MID {
        return vec4<f32>(debug_weather_level(world_xz, 1), 1.0);
    }
    if debug_mode == DEBUG_VIEW_WEATHER_CLIP_FAR {
        return vec4<f32>(debug_weather_level(world_xz, 2), 1.0);
    }
    if debug_mode == DEBUG_VIEW_WEATHER_COMPOSITE {
        return vec4<f32>(debug_weather_composite(world_xz, false), 1.0);
    }
    if debug_mode == DEBUG_VIEW_WEATHER_SHADOW {
        return vec4<f32>(debug_weather_composite(world_xz, true), 1.0);
    }
    if debug_mode != DEBUG_VIEW_NONE {
        return vec4<f32>(cloud.rgb, alpha);
    }

    if alpha <= 0.0001 {
        return vec4<f32>(0.0);
    }

    let exposure = max(light.ambient_exposure.w, 0.01);
    let saturation = clamp(light.sky_top_saturation.w, 0.45, 1.05);
    let linear = max(cloud.rgb / max(alpha, 1e-4), vec3<f32>(0.0));
    let tonemapped = aces_tonemap(linear * exposure);
    let graded = saturate_color(tonemapped, saturation);
    return vec4<f32>(graded * alpha, alpha);
}
