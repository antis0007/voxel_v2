@group(0) @binding(0) var<uniform> light : Lighting;

@group(1) @binding(0) var cloud_tex : texture_2d<f32>;
@group(1) @binding(1) var cloud_sampler : sampler;
@group(3) @binding(0) var weather_primary_tex : texture_2d_array<f32>;
@group(3) @binding(1) var weather_secondary_tex : texture_2d_array<f32>;
@group(3) @binding(2) var weather_sampler : sampler;
@group(3) @binding(6) var scene_depth_tex : texture_depth_2d;
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
    let p = clamp(uv * dims - vec2<f32>(0.5), vec2<f32>(0.0), dims - vec2<f32>(1.0));
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

// Match the conservative 2x2 depth footprint used by the half-resolution march.
fn cloud_sample_depth(uv: vec2<f32>) -> f32 {
    let dims = vec2<i32>(textureDimensions(scene_depth_tex));
    let base = vec2<i32>(floor(uv * vec2<f32>(dims) - vec2<f32>(0.5)));
    let a = clamp(base, vec2<i32>(0), dims - 1);
    let b = clamp(base + 1, vec2<i32>(0), dims - 1);
    return min(min(textureLoad(scene_depth_tex, a, 0),
        textureLoad(scene_depth_tex, vec2<i32>(b.x, a.y), 0)),
        min(textureLoad(scene_depth_tex, vec2<i32>(a.x, b.y), 0), textureLoad(scene_depth_tex, b, 0)));
}

fn depth_match(a: f32, b: f32) -> f32 {
    if (a >= 0.9999999) != (b >= 0.9999999) { return 0.0; }
    return exp(-abs(a - b) * 24.0 / max(1.0 - a, 1e-7));
}

fn reconstruct_cloud(uv: vec2<f32>) -> vec4<f32> {
    let dims = vec2<f32>(textureDimensions(cloud_tex));
    let p = uv * dims - vec2<f32>(0.5);
    let base = floor(p);
    let f = fract(p);
    var colors: array<vec4<f32>, 4>;
    for (var i = 0; i < 4; i += 1) {
        let corner = vec2<f32>(f32(i % 2), f32(i / 2));
        colors[i] = textureSampleLevel(cloud_tex, cloud_sampler,
            (base + corner + vec2<f32>(0.5)) / dims, 0.0);
    }
    let empty_footprint=max(max(colors[0].a,colors[1].a),max(colors[2].a,colors[3].a))<=0.00001;
    let pixel = clamp(vec2<i32>(uv * vec2<f32>(textureDimensions(scene_depth_tex))),
        vec2<i32>(0), vec2<i32>(textureDimensions(scene_depth_tex)) - 1);
    let depth = textureLoad(scene_depth_tex, pixel, 0);
    var total = vec4<f32>(0.0);
    var weight = 0.0;
    var all_sky = depth >= 0.9999999;
    for (var i = 0; i < 4; i += 1) {
        let corner = vec2<f32>(f32(i % 2), f32(i / 2));
        let tap_uv = (base + corner + vec2<f32>(0.5)) / dims;
        let tap_depth = cloud_sample_depth(tap_uv);
        all_sky = all_sky && tap_depth >= 0.9999999;
        let bilinear = mix(1.0 - f, f, corner);
        let w = bilinear.x * bilinear.y * depth_match(depth, tap_depth);
        total += colors[i] * w;
        weight += w;
    }
    // An empty conservative footprint may straddle geometry: recover sky gaps
    // before taking the transparent fast path, otherwise storm edges glow white.
    if empty_footprint && all_sky {return vec4<f32>(0.0);}
    if !all_sky {
        if weight > 1e-5 { return total / weight; }
        // Thin sky gaps can lose all four conservative march samples. Recover
        // from the closest matching neighbour rather than leave a dark fringe.
        var best = 0.0;
        var fallback = vec4<f32>(0.0);
        for (var y = -1; y <= 2; y += 1) {
            for (var x = -1; x <= 2; x += 1) {
                let q = vec2<f32>(f32(x), f32(y));
                let tap_uv = (base + q + vec2<f32>(0.5)) / dims;
                let w = depth_match(depth, cloud_sample_depth(tap_uv)) / (1.0 + dot(q - f, q - f));
                if w > best {
                    best = w;
                    fallback = textureSampleLevel(cloud_tex, cloud_sampler, tap_uv, 0.0);
                }
            }
        }
        return fallback;
    }
    // Dense storm decks need reconstruction of the stratified integration
    // noise, not a sharpening cubic filter. Keep positive weights and reject
    // geometry neighbours so this cannot bleed clouds across a roof silhouette.
    if light.cloud_authoring[1].z > 0.08 && min(min(colors[0].a,colors[1].a),min(colors[2].a,colors[3].a)) > 0.55 {
        let center=total/max(weight,1e-5);
        var filtered=vec4<f32>(0.0);
        var filtered_weight=0.0;
        for(var y=-1;y<=1;y+=1) {for(var x=-1;x<=1;x+=1) {
            let offset=vec2<f32>(f32(x),f32(y));
            let tap_uv=uv+offset/dims;
            if cloud_sample_depth(tap_uv)<0.9999999 {continue;}
            let tap=textureSampleLevel(cloud_tex,cloud_sampler,tap_uv,0.0);
            let w=exp(-dot(offset,offset)*0.65-abs(tap.a-center.a)*16.0);
            filtered+=tap*w;filtered_weight+=w;
        }}
        return filtered/max(filtered_weight,1e-5);
    }
    // Separable Catmull-Rom, combining its two positive middle weights into
    // one bilinear tap: nine fetches retain detail without another ray march.
    let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    let w3 = f * f * (-0.5 + 0.5 * f);
    let middle = w1 + w2;
    let positions = array<vec2<f32>, 3>(base - 0.5,
        base + 0.5 + w2 / middle, base + 2.5);
    let weights = array<vec2<f32>, 3>(w0, middle, w3);
    var cubic = vec4<f32>(0.0);
    for (var y = 0; y < 3; y += 1) {
        for (var x = 0; x < 3; x += 1) {
            cubic += textureSampleLevel(cloud_tex, cloud_sampler,
                vec2<f32>(positions[x].x, positions[y].y) / dims, 0.0)
                * weights[x].x * weights[y].y;
        }
    }
    // Negative cubic lobes must not invent bright rims or negative opacity.
    return clamp(cubic, min(min(colors[0], colors[1]), min(colors[2], colors[3])),
        max(max(colors[0], colors[1]), max(colors[2], colors[3])));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // This pass writes to the full-resolution surface while the cloud target is
    // rendered at half resolution. Use surface UVs here so the upsampled sample
    // footprint is stable regardless of the cloud texture dimensions.
    let surface_uv = surface_uv_from_frag_coord(in.pos.xy);
    let cloud = reconstruct_cloud(surface_uv);
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

    // Premultiplied linear radiance. Blending tone-mapped cloud RGB over a
    // separately tone-mapped sky made edges dark and sunset colours disagree.
    return vec4<f32>(max(cloud.rgb, vec3<f32>(0.0)), alpha);
}
