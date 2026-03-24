// ============================================================
// world_shader.wgsl
//
// Voxel terrain lit pass.
//
// Improvements over previous version:
//   • Sun colour uses sky_transmittance_to_sun() so terrain at low sun angles
//     is correctly tinted orange/red — matches cloud illumination tint.
//   • Aerial perspective fog uses atmosphere_color(view_dir) for colour, giving
//     wavelength-correct blue haze at distance.
//   • Cloud shadow attenuation uses per-colour-channel cloud coverage so that
//     overcast scenes stay spectrally neutral.
//   • Fog density has a height-extinction layer and a valley-fog layer so
//     mornings look misty in low areas.
// ============================================================

// World shading keeps terrain shadowing and the shared cloud-shadow product in one
// combined group so the renderer stays within WGPU's four-bind-group limit.
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

struct VsIn {
    @location(0) pos         : vec3<f32>,
    @location(1) color       : vec3<f32>,
    @location(2) normal      : vec3<f32>,
    @location(3) surface     : vec4<f32>,
    @location(4) ao_strength : f32,
};

struct VsOut {
    @builtin(position) pos        : vec4<f32>,
    @location(0)       base_color : vec3<f32>,
    @location(1)       world_pos  : vec3<f32>,
    @location(2)       voxel_pos  : vec3<f32>,
    @location(3)       normal     : vec3<f32>,
    @location(4)       surface    : vec4<f32>,
    @location(5)       ao_strength: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let scale     = cam.world_params.x;
    let world_pos = in.pos * scale;
    out.pos        = cam.view_proj * vec4<f32>(world_pos, 1.0);
    out.base_color = in.color;
    out.world_pos  = world_pos;
    out.voxel_pos  = in.pos;
    out.normal     = normalize(in.normal);
    out.surface    = in.surface;
    out.ao_strength = in.ao_strength;
    return out;
}

// ---- BRDF helpers ----

fn saturate_color(color: vec3<f32>, amount: f32) -> vec3<f32> {
    let luma = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    return mix(vec3<f32>(luma), color, amount);
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

// Oren–Nayar diffuse (physically accurate for rough surfaces).
fn oren_nayar(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let nl = max(dot(n, l), 0.0);
    let nv = max(dot(n, v), 0.0);
    if nl <= 0.0 || nv <= 0.0 {
        return 0.0;
    }
    let sigma2  = roughness * roughness;
    let a       = 1.0 - sigma2 / (2.0 * (sigma2 + 0.33));
    let b_coef  = 0.45 * sigma2 / (sigma2 + 0.09);
    let lv      = normalize(l - n * nl);
    let vv      = normalize(v - n * nv);
    let cos_phi = max(dot(lv, vv), 0.0);
    let alpha   = max(nl, nv);
    let beta    = min(nl, nv);
    let sin_alpha = sqrt(max(1.0 - alpha * alpha, 0.0));
    let tan_beta  = sqrt(max(1.0 - beta  * beta,  0.0)) / max(beta, 1e-3);
    return nl * (a + b_coef * cos_phi * sin_alpha * tan_beta);
}

// GGX/Trowbridge-Reitz NDF + Schlick-GGX visibility + Schlick Fresnel.
// Energy-conserving microfacet specular matching modern PBR pipelines.
fn specular_term(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let h   = normalize(v + l);
    let ndh = max(dot(n, h), 0.0);
    let ndv = max(dot(n, v), 0.001);
    let ndl = max(dot(n, l), 0.0);
    let vdh = max(dot(v, h), 0.0);

    // GGX NDF: D(h) = α² / (π * (ndh² * (α²-1) + 1)²)
    let a  = roughness * roughness;
    let a2 = a * a;
    let d_denom = ndh * ndh * (a2 - 1.0) + 1.0;
    let D = a2 / max(3.14159265 * d_denom * d_denom, 1e-7);

    // Schlick-GGX geometry / visibility term
    let k = (roughness + 1.0) * (roughness + 1.0) / 8.0;
    let G1_v = ndv / (ndv * (1.0 - k) + k);
    let G1_l = ndl / (ndl * (1.0 - k) + k);
    let G = G1_v * G1_l;

    // Schlick Fresnel approximation (F0 = 0.04 for dielectric)
    let F = 0.04 + 0.96 * pow(1.0 - vdh, 5.0);

    // Cook-Torrance: D * G * F / (4 * ndv * ndl)
    return D * G * F / max(4.0 * ndv * ndl, 1e-4);
}

// ---- Shadow ----

fn pcf_shadow(uv: vec2<f32>, proj_z: f32, bias: f32, radius: f32, cascade_idx: i32) -> f32 {
    let center = textureSampleCompare(shadow_map, shadow_sampler, uv, cascade_idx, proj_z - bias) * 0.25;
    let cross_s =
        textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>( radius, 0.0), cascade_idx, proj_z - bias) +
        textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(-radius, 0.0), cascade_idx, proj_z - bias) +
        textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(0.0,  radius), cascade_idx, proj_z - bias) +
        textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(0.0, -radius), cascade_idx, proj_z - bias);
    let diag_s =
        textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>( radius,  radius), cascade_idx, proj_z - bias) +
        textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>( radius, -radius), cascade_idx, proj_z - bias) +
        textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(-radius,  radius), cascade_idx, proj_z - bias) +
        textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(-radius, -radius), cascade_idx, proj_z - bias);

    let filtered = center + cross_s * 0.125 + diag_s * 0.0625;
    return filtered;
}

fn sample_sun_shadow(world_pos: vec3<f32>, linear_depth: f32, ndotl: f32) -> f32 {
    var cascade_idx = 3;
    for (var i = 0; i < 3; i = i + 1) {
        if linear_depth < shadow.cascades[i].far_plane {
            cascade_idx = i;
            break;
        }
    }
    
    var c = shadow.cascades[cascade_idx];
    var shadow_pos = c.light_view_proj * vec4<f32>(world_pos, 1.0);
    var proj = shadow_pos.xyz / max(shadow_pos.w, 1e-5);
    var uv = proj.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
    
    var edge = min(min(uv.x, uv.y), min(1.0 - uv.x, 1.0 - uv.y));
    if (proj.z <= 0.0 || proj.z >= 1.0 || edge < 0.0) && cascade_idx < 3 {
        cascade_idx = cascade_idx + 1;
        c = shadow.cascades[cascade_idx];
        shadow_pos = c.light_view_proj * vec4<f32>(world_pos, 1.0);
        proj = shadow_pos.xyz / max(shadow_pos.w, 1e-5);
        uv = proj.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
    }

    if proj.z <= 0.0 || proj.z >= 1.0 || any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
        return 1.0;
    }

    let bias = c.base_bias + (1.0 - ndotl) * c.slope_bias;
    let radius = c.texel_size * 1.2;
    var shadow_val = pcf_shadow(uv, proj.z, bias, radius, cascade_idx);
    
    if cascade_idx == 3 {
        let edge_fade = smoothstep(0.015, 0.07, min(min(uv.x, uv.y), min(1.0 - uv.x, 1.0 - uv.y)));
        shadow_val = mix(1.0, shadow_val, edge_fade);
    }

    let dist_to_far = c.far_plane - linear_depth;
    if dist_to_far < shadow.blend_width && cascade_idx < 3 {
        let next_c = shadow.cascades[cascade_idx + 1];
        let next_pos = next_c.light_view_proj * vec4<f32>(world_pos, 1.0);
        let next_proj = next_pos.xyz / max(next_pos.w, 1e-5);
        let next_uv = next_proj.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
        let next_bias = next_c.base_bias + (1.0 - ndotl) * next_c.slope_bias;
        
        if next_proj.z > 0.0 && next_proj.z < 1.0 && all(next_uv >= vec2<f32>(0.0)) && all(next_uv <= vec2<f32>(1.0)) {
            let next_shadow = pcf_shadow(next_uv, next_proj.z, next_bias, next_c.texel_size * 1.2, cascade_idx + 1);
            let blend_factor = smoothstep(0.0, shadow.blend_width, shadow.blend_width - dist_to_far);
            shadow_val = mix(shadow_val, next_shadow, blend_factor);
        }
    }
    
    return shadow_val;
}

fn slab_segment_length(
    ro: vec3<f32>,
    rd: vec3<f32>,
    seg_len: f32,
    y0: f32,
    y1: f32,
) -> f32 {
    if abs(rd.y) < 1e-5 {
        return 0.0;
    }

    let t0 = (y0 - ro.y) / rd.y;
    let t1 = (y1 - ro.y) / rd.y;

    let enter = max(min(t0, t1), 0.0);
    let exit  = min(max(t0, t1), seg_len);

    return max(exit - enter, 0.0);
}

fn cloud_fog_amount(view_dir: vec3<f32>, seg_len: f32) -> f32 {
    let scale     = cam.world_params.x;
    let summary   = light.weather_clip_summaries[0];
    let coverage  = saturate1(summary.y);
    let density   = 1.0;
    let cirrus    = saturate1(summary.y * 0.45);
    let storm     = saturate1(summary.z);

    // Use cloud_layer_bounds() — the single source of truth for cloud heights.
    // Previous code hardcoded mix(900, 2200, ...) * scale which disagreed with
    // cloud_shared.wgsl's mix(450, 1400, ...) and produced terrain fog at
    // completely different altitudes from the visible cloud layer.
    let b = cloud_layer_bounds();

    let low_len = slab_segment_length(
        cam.camera_pos.xyz, view_dir, seg_len,
        b.low_base, b.low_top,
    );
    let high_len = slab_segment_length(
        cam.camera_pos.xyz, view_dir, seg_len,
        b.high_base, b.high_top,
    );

    let low_span  = max(b.low_top - b.low_base, 1.0);
    let high_span = max(b.high_top - b.high_base, 1.0);

    let low_optical  = (low_len / low_span) * coverage * density * (1.28 + storm * 0.52);
    let high_optical = (high_len / high_span) * cirrus * 0.18;

    let low_t  = 1.0 - exp(-low_optical * 1.24);
    let high_t = 1.0 - exp(-high_optical * 0.92);

    let horizon = saturate1(1.0 - abs(view_dir.y));
    let horizon_boost = mix(1.0, 1.18, horizon * horizon);

    let seg_factor = smoothstep(220.0 * scale, 1600.0 * scale, seg_len);
    let path_t = (low_t + high_t * (1.0 - low_t)) * horizon_boost * seg_factor;

    return clamp(path_t, 0.0, 0.82);
}


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

fn shadow_field_at_world(world_xz: vec2<f32>) -> vec4<f32> {
    let uv0 = weather_uv(world_xz, 0);
    let uv1 = weather_uv(world_xz, 1);
    let uv2 = weather_uv(world_xz, 2);
    let weight0 = weather_level_weight(uv0, 0);
    let weight1 = weather_level_weight(uv1, 1) * (1.0 - weight0);
    let weight2 = max(1.0 - weight0 - weight1, 0.0);
    return sample_weather_bilinear(cloud_shadow_tex, uv0, 0) * weight0
        + sample_weather_bilinear(cloud_shadow_tex, uv1, 1) * weight1
        + sample_weather_bilinear(cloud_shadow_tex, uv2, 2) * weight2;
}

fn project_shadow_world_xz(world_pos: vec3<f32>, light_dir: vec3<f32>, cloud_height: f32) -> vec2<f32> {
    let t = max((cloud_height - world_pos.y) / max(light_dir.y, 1e-4), 0.0);
    return world_pos.xz + light_dir.xz * min(t, light.weather_clip_regions[2].z);
}

fn sample_shared_cloud_shadow_transmittance(world_pos: vec3<f32>, light_dir: vec3<f32>) -> f32 {
    if light_dir.y <= 0.02 {
        return 1.0;
    }

    let b = cloud_layer_bounds();
    let low_center  = (b.low_base + b.low_top) * 0.5;
    let mid_center  = (b.mid_base + b.mid_top) * 0.5;
    let high_center = (b.high_base + b.high_top) * 0.5;
    let summary = light.weather_clip_summaries[0];
    let low_weight = saturate1(summary.y * 0.85);
    let mid_weight = saturate1(0.30);
    let high_weight = saturate1(0.20);
    let total_weight = max(low_weight + mid_weight + high_weight, 1e-4);
    let sample_height = (
        low_center * low_weight +
        mid_center * mid_weight +
        high_center * high_weight
    ) / total_weight;

    let shadow = shadow_field_at_world(
        project_shadow_world_xz(world_pos, light_dir, sample_height),
    );
    let optical_depth = shadow.x * 1.35
                      + shadow.y * 1.00
                      + shadow.z * 0.55;

    return clamp(exp(-optical_depth * 1.35), 0.04, 1.0);
}

// ---- Torch lighting ----

struct TorchLightSample {
    diffuse: vec3<f32>,
    fill   : vec3<f32>,
    spec   : vec3<f32>,
};

fn torch_flicker(seed: vec3<f32>, time: f32) -> f32 {
    let phase   = dot(seed, vec3<f32>(0.173, 0.697, 0.357));
    let slow    = sin(time * 6.3  + phase);
    let fast    = sin(time * 13.7 + phase * 1.71);
    let flutter = sin(time * 21.9 + phase * 2.43);
    return clamp(0.92 + slow * 0.08 + fast * 0.05 + flutter * 0.03, 0.78, 1.18);
}

fn eval_torch_light(
    torch_pos_radius   : vec4<f32>,
    torch_color_strength: vec4<f32>,
    time               : f32,
    world_pos          : vec3<f32>,
    n                  : vec3<f32>,
    to_eye             : vec3<f32>,
    roughness          : f32,
) -> TorchLightSample {
    let radius  = max(torch_pos_radius.w, 0.001);
    let to_torch = torch_pos_radius.xyz - world_pos;
    let dist2   = dot(to_torch, to_torch);
    let radius2 = radius * radius;
    if dist2 >= radius2 {
        return TorchLightSample(vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0));
    }
    let dist = sqrt(max(dist2, 1e-6));
    let l    = to_torch / dist;
    let d    = clamp(1.0 - dist / radius, 0.0, 1.0);
    let falloff = d * d;
    let flicker  = torch_flicker(torch_pos_radius.xyz * 0.23 + vec3<f32>(0.0, 1.7, 0.0), time);
    let intensity = torch_color_strength.w * falloff * flicker;
    let lambert   = max(dot(n, l), 0.0);
    let wrap      = 0.22 + 0.78 * lambert;
    let color     = torch_color_strength.rgb;
    return TorchLightSample(
        color * intensity * wrap * 1.16,
        color * intensity * (0.14 + 0.18 * (1.0 - roughness)),
        color * intensity * specular_term(n, to_eye, l, roughness) * 0.05,
    );
}

// ============================================================
// FRAGMENT SHADER
// ============================================================

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var albedo = pow(clamp(in.base_color, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(2.2));
    let roughness      = clamp(in.surface.x, 0.04, 1.0);
    let specular_weight = clamp(in.surface.y, 0.0, 1.0) * pow(1.0 - roughness, 2.0);
    let is_torch_material = in.surface.w < 0.0;

    let n       = normalize(in.normal);
    let to_eye  = normalize(cam.camera_pos.xyz - in.world_pos);
    let view_dir = normalize(in.world_pos - cam.camera_pos.xyz);
    let sun_dir  = normalize(light.sun_dir_intensity.xyz);
    let moon_dir = normalize(light.moon_dir_intensity.xyz);

    let daylight   = saturate1(light.sun_color_daylight.w);
    let exposure   = max(light.ambient_exposure.w, 0.01);
    let saturation = clamp(light.sky_top_saturation.w, 0.45, 1.05);

    let linear_depth = dot(in.world_pos - cam.camera_pos.xyz, cam.camera_forward.xyz);

    let debug_mode = i32(render_debug_view());
    if debug_mode == 99 { // Reserved local debug value; renderer-wide enum now uses 10+ for cloud diagnostics.
        var cascade_idx = 3;
        for (var i = 0; i < 3; i = i + 1) {
            if linear_depth < shadow.cascades[i].far_plane {
                cascade_idx = i;
                break;
            }
        }
        let colors = array<vec3<f32>, 4>(
            vec3<f32>(1.0, 0.2, 0.2), vec3<f32>(0.2, 1.0, 0.2),
            vec3<f32>(0.2, 0.2, 1.0), vec3<f32>(1.0, 1.0, 0.2)
        );
        return vec4<f32>(colors[cascade_idx], 1.0);
    }

    // --- Spectral sun/moon tinting from atmosphere transmittance ---
    // Matches the cloud illumination so sky, clouds, and terrain are tonally
    // consistent at all times of day, especially at sunset.
    let sun_spectral  = sky_transmittance_to_sun(sun_dir.y);
    let moon_spectral = sky_transmittance_to_sun(moon_dir.y);

    // --- World-space cloud transmittance above this surface point ---
    // This makes terrain lighting react to the actual cloud field overhead,
    // instead of just a global weather scalar.
    let local_cloud_trans = sample_shared_cloud_shadow_transmittance(
        in.world_pos + n * (2.0 * cam.world_params.x),
        sun_dir,
    );
    let cloudiness   = 1.0 - local_cloud_trans;
    let summary      = light.weather_clip_summaries[0];
    let heavy_cloud  = saturate1(summary.y * 0.65 + summary.z);
    let sun_trans    = local_cloud_trans;
    let moon_trans   = mix(1.0, local_cloud_trans, 0.35) * (1.0 - cloudiness * 0.10);

    // --- Direct diffuse ---
    let sun_lambert  = max(dot(n, sun_dir), 0.0);
    let moon_lambert = max(dot(n, moon_dir), 0.0);

    // PCF shadow
    let shadow_visibility = sample_sun_shadow(in.world_pos, linear_depth, sun_lambert);
    if debug_mode == 11 { return vec4<f32>(vec3<f32>(shadow_visibility), 1.0); }

    let sun_shadow_strength = 0.94 * smoothstep(0.03, 0.16, light.sun_dir_intensity.w);
    let sun_shadow = mix(1.0, shadow_visibility, sun_shadow_strength);

    // Oren–Nayar for rough surfaces, Lambertian for smooth.
    let sun_diffuse = mix(sun_lambert,
                          oren_nayar(n, to_eye, sun_dir, roughness),
                          roughness * 0.22)
                      * light.sun_dir_intensity.w * sun_trans * sun_shadow;
    let moon_diffuse = mix(moon_lambert,
                           oren_nayar(n, to_eye, moon_dir, roughness),
                           roughness * 0.16)
                       * light.moon_dir_intensity.w * moon_trans;

    // --- Ambient / sky fill (hemisphere integral approximation) ---
    // Normal-weighted sky sampling: upward-facing surfaces get top sky,
    // side-facing get horizon, downward-facing get ground bounce.
    let hemi        = saturate1(n.y * 0.5 + 0.5);
    let sky_up      = light.sky_top_saturation.rgb;
    let sky_side    = light.sky_horizon_reserved.rgb;
    let clear_sky_ambient = mix(sky_side, sky_up, pow(hemi, 0.6));
    let overcast_sky_ambient = mix(
        light.sky_horizon_reserved.rgb * 0.88,
        light.sky_horizon_reserved.rgb * 0.72 + light.sky_top_saturation.rgb * 0.28,
        hemi,
    );
    let sky_ambient = mix(clear_sky_ambient, overcast_sky_ambient, cloudiness * 0.65);

    // Ground bounce contribution for downward-facing surfaces
    let ground_irr  = light.sun_color_daylight.rgb * sky_transmittance_to_sun(sun_dir.y)
                    * saturate1(sun_dir.y) * light.sun_dir_intensity.w * 0.03 * sun_trans;
    let ground_col  = mix(vec3<f32>(0.12, 0.14, 0.10), ground_irr, daylight);
    let ground_amt  = saturate1(1.0 - hemi) * 0.5;
    let cavity      = mix(clamp(in.ao_strength, 0.0, 1.0), 1.0, hemi * 0.22);
    let ambient     = (
        light.ambient_exposure.rgb * mix(vec3<f32>(0.76), vec3<f32>(1.08), hemi)
        + sky_ambient * mix(0.24, 0.10, daylight)
        + ground_col * ground_amt
    ) * cavity * mix(1.0, 0.88, cloudiness * daylight * 0.55);

    // --- Torch emissive (unchanged) ---
    var emissive = vec3<f32>(0.0);
    if is_torch_material {
        let local      = fract(in.voxel_pos);
        let core       = distance(local.xz, vec2<f32>(0.5));
        let ember_mask = smoothstep(0.42, 0.0, core) * smoothstep(0.46, 0.98, local.y);
        let flame_mask = smoothstep(0.30, 0.0, core) * smoothstep(0.62, 1.0, local.y);
        let char_mask  = smoothstep(0.18, 0.44, core) * smoothstep(0.56, 0.96, local.y);
        let flame_wave = torch_flicker(in.voxel_pos + vec3<f32>(0.0, 2.3, 0.0), light.torch_meta.y);
        let wood_color  = vec3<f32>(0.26, 0.15, 0.05)
                          + vec3<f32>(0.14, 0.10, 0.04) * clamp(local.y, 0.0, 1.0);
        let ember_color = mix(vec3<f32>(0.92, 0.14, 0.03), vec3<f32>(1.00, 0.42, 0.10),
                              0.5 + flame_wave * 0.25);
        let flame_color = mix(vec3<f32>(1.00, 0.30, 0.08), vec3<f32>(1.00, 0.70, 0.26),
                              0.45 + flame_wave * 0.30);
        albedo = mix(albedo, wood_color, 0.86);
        albedo = mix(albedo, ember_color, ember_mask * 0.72);
        albedo *= 1.0 - char_mask * 0.16;
        emissive += flame_color * flame_mask * (2.3 + flame_wave * 1.6);
        emissive += ember_color * ember_mask * (0.32 + flame_wave * 0.24);
    }

    // --- Sun/moon fill (bounced indirect) ---
    let sun_fill = light.sun_color_daylight.rgb * sun_spectral
                   * light.sun_dir_intensity.w * sun_trans * sun_shadow
                   * (0.04 + 0.05 * daylight) * (0.55 + 0.45 * hemi);
    let moon_fill = light.moon_color_twilight.rgb * moon_spectral
                    * light.moon_dir_intensity.w * moon_trans
                    * 0.02 * (0.55 + 0.45 * hemi);

    // --- Torch lights ---
    var torch_diffuse = vec3<f32>(0.0);
    var torch_fill    = vec3<f32>(0.0);
    var torch_spec    = vec3<f32>(0.0);
    let torch_count   = min(u32(light.torch_meta.x), 16u);
    for (var i: u32 = 0u; i < 16u; i = i + 1u) {
        if i >= torch_count { break; }
        let ts = eval_torch_light(
            light.torch_positions[i], light.torch_colors[i],
            light.torch_meta.y, in.world_pos, n, to_eye, roughness,
        );
        torch_diffuse += ts.diffuse;
        torch_fill    += ts.fill;
        torch_spec    += ts.spec;
    }
    if light.held_torch_pos_radius.w > 0.0 && light.held_torch_color_strength.w > 0.0 {
        let ht = eval_torch_light(
            light.held_torch_pos_radius, light.held_torch_color_strength,
            light.torch_meta.y, in.world_pos, n, to_eye, roughness,
        );
        torch_diffuse += ht.diffuse;
        torch_fill    += ht.fill;
        torch_spec    += ht.spec;
    }

    // --- Composite lit colour ---
    // Use spectral sun/moon colours so that terrain takes on the correct
    // warm/cool tones matching the sky and clouds at all times of day.
    var lit = albedo * (
        ambient
        + light.sun_color_daylight.rgb  * sun_spectral  * sun_diffuse  * 0.62
        + light.moon_color_twilight.rgb * moon_spectral * moon_diffuse * 0.34
        + sun_fill
        + moon_fill
        + torch_diffuse
        + torch_fill
    );

    // Specular highlights
    lit += light.sun_color_daylight.rgb * sun_spectral
           * specular_term(n, to_eye, sun_dir, roughness)
           * light.sun_dir_intensity.w * sun_trans * sun_shadow
           * specular_weight * 0.022;
    lit += light.moon_color_twilight.rgb * moon_spectral
           * specular_term(n, to_eye, moon_dir, roughness)
           * light.moon_dir_intensity.w * moon_trans
           * specular_weight * 0.006;
    lit += torch_spec * specular_weight;
    lit += emissive;

    // ============================================================
    // AERIAL PERSPECTIVE / FOG
    //
    // Uses atmosphere_color(view_dir) for the fog target colour so distant
    // terrain inherits exactly the same sky hue as the sky pass.  This is the
    // primary cause of coherent blue distance haze.
    // ============================================================
    let dist     = distance(in.world_pos, cam.camera_pos.xyz);
    var fog_target = vec3<f32>(0.0);
    var total_fog_t = 0.0;

    if atmosphere_mode() >= ATMOSPHERE_MODE_SKY_VIEW_LUT {
        let max_dist = max(cam.world_params.w * 0.82, 1.0);
        let ap = sample_atmosphere_aerial_perspective(view_dir, dist, max_dist);
        
        let fog_t = clamp(1.0 - ap.a, 0.0, 1.0);
        let ap_fog_color = ap.rgb / max(fog_t, 1e-5);
        
        let sun_forward = pow(max(dot(view_dir, sun_dir), 0.0), 10.0)
                        * light.sun_dir_intensity.w * sun_trans;
        fog_target = ap_fog_color
                   + light.sun_color_daylight.rgb * sky_transmittance_to_sun(sun_dir.y)
                   * sun_forward * 0.016;
                   
        let cloud_fog_t = cloud_fog_amount(view_dir, dist);
        total_fog_t = clamp(1.0 - (1.0 - fog_t) * (1.0 - cloud_fog_t), 0.0, 1.0);
    } else {
        let fog_lin  = clamp(
            (dist - light.fog_params.x) / max(light.fog_params.y - light.fog_params.x, 1.0),
            0.0, 1.0,
        );
        let horizon      = saturate1(1.0 - abs(view_dir.y));
        let cloud_fog    = cloudiness * (0.05 + heavy_cloud * 0.05);
        let fog_density  = mix(0.22, 0.10, daylight) + cloud_fog;

        let world_fog_center = light.fog_params.z * cam.world_params.x;
        let height_fog   = exp(-max(in.world_pos.y - world_fog_center, 0.0) * light.fog_params.w);
        let valley_fog   = exp(-max(world_fog_center - in.world_pos.y, 0.0) * light.fog_params.w * 0.52);

        let fog_shape = (0.34 + 0.66 * horizon) * (0.38 * height_fog + 0.62 * valley_fog);
        let fog_t = clamp(
            1.0 - exp2(-fog_lin * fog_lin * (2.8 + 5.4 * fog_density) * fog_shape),
            0.0, 1.0
        );

        let atmospheric_sky = atmosphere_color(view_dir);
        let sun_forward = pow(max(dot(view_dir, sun_dir), 0.0), 10.0)
                        * light.sun_dir_intensity.w * sun_trans;

        let ap = aerial_perspective(view_dir, dist, cam.world_params.x, atmospheric_sky);
        let ap_fog_color = ap.rgb / max(1.0 - ap.a, 1e-5);
        
        fog_target  = ap_fog_color
                    + light.sun_color_daylight.rgb * sky_transmittance_to_sun(sun_dir.y)
                        * sun_forward * 0.016;

        let cloud_fog_t = cloud_fog_amount(view_dir, dist);
        total_fog_t = clamp(1.0 - (1.0 - fog_t) * (1.0 - cloud_fog_t), 0.0, 1.0);
    }
    
    lit = mix(lit, fog_target, total_fog_t);

    // Night darkening
    lit *= mix(0.40, 1.0, daylight);

    // Tonemap + saturation grade
    let tonemapped = aces_tonemap(max(lit, vec3<f32>(0.0)) * exposure);
    let graded     = saturate_color(tonemapped, saturation * mix(0.72, 1.0, daylight));
    return vec4<f32>(graded, 1.0);
}
