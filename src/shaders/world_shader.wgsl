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
    @location(6) material_tint: vec4<f32>,
    @location(7) material_props: vec4<f32>,
    @location(8) water_optics: vec2<f32>,
};

fn gerstner_vertex_point(wxz: vec2<f32>, t: f32) -> vec3<f32> {
    let d0 = vec2<f32>(0.88, 0.47);
    let d1 = vec2<f32>(0.52, -0.85);
    let d2 = vec2<f32>(-0.74, 0.67);
    let d3 = vec2<f32>(-0.31, -0.95);

    let ph0 = dot(wxz, d0) * 0.26 - t * 1.45;
    let ph1 = dot(wxz, d1) * 0.43 - t * 1.88 + 1.7;
    let ph2 = dot(wxz, d2) * 0.68 - t * 2.35 + 3.1;
    let ph3 = dot(wxz, d3) * 1.05 - t * 2.90 + 0.8;

    let a0 = 0.028;
    let a1 = 0.019;
    let a2 = 0.011;
    let a3 = 0.007;

    let dy = cos(ph0) * a0 + cos(ph1) * a1 + cos(ph2) * a2 + cos(ph3) * a3;
    let dxz = -d0 * (sin(ph0) * a0 * 0.45)
              - d1 * (sin(ph1) * a1 * 0.42)
              - d2 * (sin(ph2) * a2 * 0.35)
              - d3 * (sin(ph3) * a3 * 0.30);
    return vec3<f32>(dxz.x, dy, dxz.y);
}

// Bilinear evaluation on the 2-voxel lattice guarantees that 1x1 shoreline
// quads, 2x2 open-water quads, and chunk-boundary vertices share identical
// linear edges without T-junction cracks under 3D wave displacement.
fn eval_water_vertex_wave(voxel_xz: vec2<f32>, scale: f32, t: f32) -> vec3<f32> {
    let grid = voxel_xz * 0.5;
    let cell = floor(grid);
    let f = fract(grid);
    let step = 2.0 * scale;
    let p00 = cell * step;
    let w00 = gerstner_vertex_point(p00, t);
    let w10 = gerstner_vertex_point(p00 + vec2<f32>(step, 0.0), t);
    let w01 = gerstner_vertex_point(p00 + vec2<f32>(0.0, step), t);
    let w11 = gerstner_vertex_point(p00 + vec2<f32>(step, step), t);
    return mix(mix(w00, w10, f.x), mix(w01, w11, f.x), f.y);
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let scale     = cam.world_params.x;
    let geometry_kind=f32(u32(round(max(in.surface.z,0.0)))%64u);
    var model_pos = in.pos;
    if in.surface.w < -1.5 {
        let phase = dot(floor(in.pos), vec3<f32>(0.73, 1.17, 0.41));
        model_pos.x += sin(light.torch_meta.y * 7.1 + phase + in.pos.y * 5.0) * (0.018 / scale);
        model_pos.z += sin(light.torch_meta.y * 5.3 + phase + in.pos.y * 7.0) * (0.012 / scale);
    }
    out.water_optics = vec2<f32>(10.0, 1.0);
    if geometry_kind == 8.0 && in.surface.w >= 8.0 && in.ao_strength >= 0.0 {
        out.water_optics = vec2<f32>(floor(in.surface.w-8.0)*0.25,
            clamp(fract(in.surface.w)*2.0,0.006,1.0));
    }
    var world_pos = model_pos * scale;
    let horiz_dist = length((world_pos - cam.camera_pos.xyz).xz);
    if geometry_kind == 8.0 && in.normal.y > 0.5 && in.ao_strength >= 0.0 {
        let shore_atten = smoothstep(0.55 * scale, 1.45 * scale, out.water_optics.x);
        let dist_atten = 1.0 - smoothstep(96.0 * scale, 240.0 * scale, horiz_dist);
        let wave_disp = eval_water_vertex_wave(model_pos.xz, scale, light.torch_meta.y);
        world_pos += wave_disp * (shore_atten * dist_atten * scale);
    }
    var backdrop_depth_bias = 0.0;
    if in.ao_strength < 0.0 && in.ao_strength > -1.5 {
        let handoff_outer = cam.world_params.w + 16.0 * scale;
        let handoff_inner = max(cam.world_params.w * 0.58, cam.world_params.w - 72.0 * scale);
        let underlap = 1.0 - smoothstep(handoff_inner, handoff_outer, horiz_dist);
        world_pos.y -= underlap * select(0.2 * scale, 0.08 * scale, geometry_kind == 8.0);
        // Scale bias byremaining depth budget (w - z) rather than w so perspective
        // hyperbolic depth never saturates or flattens across the overlap zone.
        backdrop_depth_bias = underlap * 0.0012;
    }
    out.pos        = cam.view_proj * vec4<f32>(world_pos - cam.camera_pos.xyz, 1.0);
    // Separate exact fallback/resident depth without splitting shared world edges.
    // Per-face normal insets opened sky pinholes on fine voxel silhouettes.
    if in.ao_strength < -1.5 {
        out.pos.z += max(out.pos.w - out.pos.z, 0.0) * 0.00015;
    }
    if backdrop_depth_bias > 0.0 {
        out.pos.z += max(out.pos.w - out.pos.z, 0.0) * backdrop_depth_bias;
    }
    out.base_color = in.color;
    out.world_pos  = world_pos;
    out.voxel_pos  = in.pos;
    out.normal     = normalize(in.normal);
    out.surface    = in.surface;
    out.surface.z = geometry_kind;
    out.ao_strength = in.ao_strength;
    // Resolve IDs at vertices; interpolate colors/finish, never material IDs.
    let far_terrain=in.ao_strength<0.0 && in.ao_strength> -1.5 && in.surface.z==0.0;
    let key=u32(round(max(select(in.surface.z,in.surface.w,far_terrain),0.0)));
    let family=key%64u;
    let block_key=key/64u;
    var tint=light.material_tints[family];
    var properties=light.material_properties[family];
    if block_key>0u {
        let individual=min(63u+block_key,255u);
        let t=light.material_tints[individual];
        let p=light.material_properties[individual];
        tint=vec4<f32>(tint.rgb*t.rgb,select(tint.w,t.w,t.w>=0.0));
        properties=vec4<f32>(select(properties.x,p.x,p.x>=0.0),properties.yzw*p.yzw);
    }
    let default_roughness=select(in.surface.x,0.17,geometry_kind==8.0);
    tint.w=select(default_roughness,tint.w,tint.w>=0.0);
    properties.x=select(in.surface.y,properties.x,properties.x>=0.0);
    out.material_tint=tint;
    out.material_props=properties;
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
    let lp = l - n * nl;
    let vp = v - n * nv;
    let lv = lp / max(length(lp), 1e-4);
    let vv = vp / max(length(vp), 1e-4);
    let cos_phi = max(dot(lv, vv), 0.0);
    let alpha   = max(nl, nv);
    let beta    = min(nl, nv);
    // alpha/beta above are COSINES: the larger angle has the smaller cosine.
    let sin_alpha = sqrt(max(1.0 - beta * beta, 0.0));
    let tan_beta = sqrt(max(1.0 - alpha * alpha, 0.0)) / max(alpha, 1e-3);
    return nl * (a + b_coef * cos_phi * sin_alpha * tan_beta);
}

// GGX/Trowbridge-Reitz NDF + Schlick-GGX visibility + Schlick Fresnel.
// Energy-conserving microfacet specular matching modern PBR pipelines.
fn specular_term(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let half_vector = v + l;
    let h = half_vector / max(length(half_vector), 1e-4);
    let ndh = clamp(dot(n, h), 0.0, 1.0);
    let ndv = clamp(dot(n, v), 0.001, 1.0);
    let ndl = clamp(dot(n, l), 0.0, 1.0);
    // Roundoff must not feed a negative base into WGSL pow for Fresnel.
    let vdh = clamp(dot(v, h), 0.0, 1.0);

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

    // BRDF times incident cosine. The old division by N.L without an incident
    // cosine made grazing highlights spike and light back-facing surfaces.
    return D * G * F / max(4.0 * ndv, 1e-4);
}

// ---- Shadow ----

// A filter tap is a different point on the receiver plane. Comparing every
// tap with the centre depth can self-shadow ground under a grazing sun and
// expose cascade footprints as camera-following rectangular patches.
fn shadow_receiver_gradient(matrix: mat4x4<f32>, world_dx: vec3<f32>, world_dy: vec3<f32>) -> vec2<f32> {
    let dx = (matrix * vec4<f32>(world_dx, 0.0)).xyz;
    let dy = (matrix * vec4<f32>(world_dy, 0.0)).xyz;
    let uv_dx = dx.xy * vec2<f32>(0.5, -0.5);
    let uv_dy = dy.xy * vec2<f32>(0.5, -0.5);
    let determinant = uv_dx.x * uv_dy.y - uv_dx.y * uv_dy.x;
    // A receiver edge-on to the light has no invertible texture-space plane.
    if abs(determinant) < 1e-12 { return vec2<f32>(0.0); }
    return vec2<f32>(uv_dy.y * dx.z - uv_dx.y * dy.z,
        uv_dx.x * dy.z - uv_dy.x * dx.z) / determinant;
}

fn pcf_shadow(uv: vec2<f32>, proj_z: f32, bias: f32, radius: f32, cascade_idx: i32, gradient: vec2<f32>) -> f32 {
    let center = textureSampleCompare(shadow_map, shadow_sampler, uv, cascade_idx, proj_z - bias) * 0.25;
    let cross_s =
        textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>( radius, 0.0), cascade_idx, proj_z - bias + gradient.x * radius) +
        textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(-radius, 0.0), cascade_idx, proj_z - bias - gradient.x * radius) +
        textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(0.0,  radius), cascade_idx, proj_z - bias + gradient.y * radius) +
        textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(0.0, -radius), cascade_idx, proj_z - bias - gradient.y * radius);
    let diag_s =
        textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>( radius,  radius), cascade_idx, proj_z - bias + dot(gradient, vec2<f32>( radius, radius))) +
        textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>( radius, -radius), cascade_idx, proj_z - bias + dot(gradient, vec2<f32>( radius,-radius))) +
        textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(-radius,  radius), cascade_idx, proj_z - bias + dot(gradient, vec2<f32>(-radius, radius))) +
        textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(-radius, -radius), cascade_idx, proj_z - bias + dot(gradient, vec2<f32>(-radius,-radius)));

    let filtered = center + cross_s * 0.125 + diag_s * 0.0625;
    return filtered;
}

fn sample_sun_shadow(world_pos: vec3<f32>, linear_depth: f32, ndotl: f32, world_dx: vec3<f32>, world_dy: vec3<f32>) -> f32 {
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
    var shadow_val = pcf_shadow(uv, proj.z, bias, radius, cascade_idx,
        shadow_receiver_gradient(c.light_view_proj, world_dx, world_dy));

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
            let next_shadow = pcf_shadow(next_uv, next_proj.z, next_bias, next_c.texel_size * 1.2, cascade_idx + 1,
                shadow_receiver_gradient(next_c.light_view_proj, world_dx, world_dy));
            let blend_factor = smoothstep(0.0, shadow.blend_width, shadow.blend_width - dist_to_far);
            shadow_val = mix(shadow_val, next_shadow, blend_factor);
        }
    }

    // End shadow coverage in a broad radial band. A view-depth cutoff or the
    // rectangular edge of the last projection reads as an LOD line on terrain.
    let shadow_range = shadow.cascades[3].far_plane;
    let range_fade = smoothstep(shadow_range * 0.78, shadow_range,
        distance(world_pos, cam.camera_pos.xyz));
    return mix(shadow_val, 1.0, range_fade);
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
    return smoothstep(0.04, 0.38, weather_edge_distance(uv));
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

fn shadow_field_at_world(world_xz: vec2<f32>) -> vec4<f32> {
    let uv0 = weather_uv(world_xz, 0);
    let uv1 = weather_uv(world_xz, 1);
    let uv2 = weather_uv(world_xz, 2);
    let weight0 = weather_level_weight(uv0, 0);
    let weight1 = weather_level_weight(uv1, 1) * (1.0 - weight0);
    let weight2 = max(1.0 - weight0 - weight1, 0.0);
    // The outer map is finite too. Clamped samples outside it used to stretch
    // a single dark edge texel into kilometre-long bands across distant hills.
    let outer_coverage = smoothstep(0.0, 0.12, weather_edge_distance(uv2));
    return sample_weather_bilinear(cloud_shadow_tex, uv0, 0) * weight0
        + sample_weather_bilinear(cloud_shadow_tex, uv1, 1) * weight1
        + sample_weather_bilinear(cloud_shadow_tex, uv2, 2) * weight2 * outer_coverage;
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
    let wind = light.weather_globals[0].zw * light.weather_globals[0].x * light.weather_globals[0].y;
    // Project and advect each layer independently, matching the visible field.
    // A single unadvected average height left the terrain's shadows behind.
    let low = shadow_field_at_world(project_shadow_world_xz(world_pos, light_dir, low_center) + wind * 8.0).x;
    let mid = shadow_field_at_world(project_shadow_world_xz(world_pos, light_dir, mid_center) + wind * 6.65).y;
    let high = shadow_field_at_world(project_shadow_world_xz(world_pos, light_dir, high_center) + wind * 12.0).z;
    let optical_depth = low + mid + high;
    return clamp(exp(-optical_depth / max(light_dir.y, 0.15)), 0.04, 1.0);
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
    let d = clamp(1.0 - pow(dist / radius, 4.0), 0.0, 1.0);
    let falloff = d * d / (1.0 + dist2 * 0.12);
    let flicker  = torch_flicker(torch_pos_radius.xyz * 0.23 + vec3<f32>(0.0, 1.7, 0.0), time);
    let intensity = torch_color_strength.w * falloff * flicker;
    let lambert   = max(dot(n, l), 0.0);
    let wrap = max((dot(n, l) + 0.12) / 1.12, 0.0);
    let color     = torch_color_strength.rgb;
    return TorchLightSample(
        color * intensity * wrap * 1.16,
        color * intensity * (0.035 + 0.045 * (1.0 - roughness)),
        color * intensity * specular_term(n, to_eye, l, roughness) * 0.05,
    );
}

// ============================================================
// FRAGMENT SHADER
// ============================================================

// Tile-free, face-aligned surfaces. The footprint fade removes subpixel detail
// before it can shimmer on distant terrain or across greedy/LOD mesh seams.
fn surface_hash(p: vec2<f32>) -> f32 {
    // Integer mixing avoids trigonometric hash precision collapse.
    // Use floor(p) so negative coordinates never fold across zero.
    let cell = vec2<i32>(floor(p));
    var h = bitcast<u32>(cell.x) * 0x9e3779b9u
        ^ bitcast<u32>(cell.y) * 0x85ebca6bu ^ cam.scene_seed.x;
    h = (h ^ (h >> 16u)) * 0x7feb352du;
    h = (h ^ (h >> 15u)) * 0x846ca68bu;
    return f32((h ^ (h >> 16u)) >> 8u) * (1.0 / 16777216.0);
}

fn surface_noise(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let w = f * f * (3.0 - 2.0 * f);
    return mix(mix(surface_hash(cell), surface_hash(cell + vec2<f32>(1.0, 0.0)), w.x),
               mix(surface_hash(cell + vec2<f32>(0.0, 1.0)), surface_hash(cell + vec2<f32>(1.0)), w.x), w.y);
}

fn surface_noise_grad(p: vec2<f32>) -> vec3<f32> {
    let cell = floor(p);
    let f = fract(p);
    let w = f * f * (3.0 - 2.0 * f);
    let dw = 6.0 * f * (1.0 - f);
    let a = surface_hash(cell);
    let b = surface_hash(cell + vec2<f32>(1.0, 0.0));
    let c = surface_hash(cell + vec2<f32>(0.0, 1.0));
    let d = surface_hash(cell + vec2<f32>(1.0, 1.0));
    let k1 = b - a;
    let k2 = c - a;
    let k3 = a - b - c + d;
    let val = a + k1 * w.x + k2 * w.y + k3 * w.x * w.y;
    let grad = dw * vec2<f32>(k1 + k3 * w.y, k2 + k3 * w.x);
    return vec3<f32>(val, grad.x, grad.y);
}

// Match the generator's continuous metre-space climate, independent of mesh
// resolution and biome labels. Adjacent materials share one transition field.
fn climate_hash(p: vec2<i32>, seed: u32) -> f32 {
    var h = bitcast<u32>(p.x)*374761393u ^ bitcast<u32>(p.y)*668265263u ^ seed;
    h = (h ^ (h >> 13u))*1274126177u; h ^= h >> 16u;
    return f32(h & 65535u)/65535.0;
}
fn climate_noise(p: vec2<f32>, seed: u32) -> f32 {
    let cell=vec2<i32>(floor(p));let f=fract(p);let t=f*f*(3.0-2.0*f);
    return mix(mix(climate_hash(cell,seed),climate_hash(cell+vec2<i32>(1,0),seed),t.x),
        mix(climate_hash(cell+vec2<i32>(0,1),seed),climate_hash(cell+vec2<i32>(1,1),seed),t.x),t.y);
}
fn natural_surface(base: vec3<f32>, p: vec3<f32>, normal: vec3<f32>, kind:i32, footprint:f32) -> vec3<f32> {
    let q=p*cam.world_params.x;let fp=footprint*cam.world_params.x;
    let seed=cam.scene_seed.x;
    let warped=q.xz+vec2<f32>(climate_noise(q.xz/420.0,seed+7u)-0.5,
        climate_noise(q.xz/390.0,seed+9u)-0.5)*180.0;
    let climate_gain=select(1.0,1.55,cam.scene_seed.y>=5u);
    let lapse=select(0.0035,0.0018,cam.scene_seed.y>=7u);
    let heat=clamp((climate_noise(warped/700.0,seed+11u)-0.5)*climate_gain+0.5-max(q.y-30.0,0.0)*lapse,0.0,1.0);
    let wet=clamp((climate_noise(warped/520.0,seed+23u)-0.5)*select(1.0,1.4,cam.scene_seed.y>=5u)+0.5,0.0,1.0);
    let arid=smoothstep(0.48,0.70,heat)*(1.0-smoothstep(0.26,0.48,wet));
    // Two continuous projections avoid dominant-axis seams on relaxed geometry.
    let mottle=surface_noise(q.xz/17.0+q.y*vec2<f32>(0.017,0.027));
    let weights=pow(abs(normal),vec3<f32>(4.0));
    let w=weights/max(dot(weights,vec3<f32>(1.0)),0.001);
    var grain=0.0;
    if fp<0.25 {
        // Continuous physical coordinates avoid a painted square grid on turf.
        let pixel=q;
        grain=(surface_noise(pixel.yz*6.0)*w.x+surface_noise(pixel.xz*6.0)*w.y+surface_noise(pixel.xy*6.0)*w.z-0.5)
            *(1.0-smoothstep(0.035,0.25,fp));
    }
    var clumps=0.0;
    if fp<4.0 {clumps=(surface_noise(q.xz*0.65+q.y*vec2<f32>(0.21,0.16))-0.5)*(1.0-smoothstep(1.0,4.0,fp));}
    let up=smoothstep(-0.05,0.65,normal.y);
    let soil=mix(vec3<f32>(0.21,0.155,0.10),vec3<f32>(0.28,0.21,0.135),1.0-wet);
    let green=mix(vec3<f32>(0.048,0.102,0.038),vec3<f32>(0.115,0.150,0.062),1.0-wet);
    let cool=1.0-smoothstep(0.22,0.44,heat);
    let forest=smoothstep(0.48,0.72,wet);
    let meadow=mix(green,vec3<f32>(0.135,0.175,0.076), (1.0-forest)*mottle*0.42);
    let boreal=mix(vec3<f32>(0.18,0.155,0.095),vec3<f32>(0.058,0.105,0.068),forest);
    let turf=mix(mix(meadow,boreal,cool*0.75),vec3<f32>(0.35,0.28,0.12),arid);
    let sand=mix(vec3<f32>(0.54,0.45,0.29),vec3<f32>(0.68,0.57,0.38),heat);
    var color=base;
    if kind==0 || kind==1 || kind==7 {
        let coastal_dune=(1.0-smoothstep(12.2,13.6+mottle*0.6,q.y))*0.28;
        let backdrop_beach=select(0.0,coastal_dune,kind==0);
        let sparse=smoothstep(0.15,0.85,arid)*0.85;
        let top_coat=mix(turf,sand,max(coastal_dune,sparse));
        let turf_overhang=select(0.0,smoothstep(0.48,0.82,fract(p.y)+clumps*0.18),kind==1 || kind==7);
        let side_coat=mix(soil,top_coat,max(backdrop_beach,turf_overhang*0.78));
        color=mix(side_coat,top_coat,up);
        let cold=1.0-smoothstep(0.08,0.37,heat);
        let drift_noise=surface_noise(q.xz/48.0+vec2<f32>(19.3,-27.1))*0.62
                       +surface_noise(q.xz/13.5+vec2<f32>(-41.2,13.7))*0.38;
        let rot_chute = mat2x2<f32>(0.80, -0.60, 0.60, 0.80);
        let chute_val = surface_noise((rot_chute * q.xz) / 22.0 + vec2<f32>(31.4, -19.8));
        // Concave couloir gullies trap drifted snow lower down the mountain flank,
        // while convex rock ribs and steep headwalls shed snow and expose bare crags.
        let couloir_pocket = (1.0 - abs(chute_val * 2.0 - 1.0)) * smoothstep(68.0, 125.0, q.y);
        let windward_drift=clamp(dot(normal.xz,vec2<f32>(-0.65,0.76)),-1.0,1.0)*0.08;
        let slope_hold=smoothstep(0.36,0.78,normal.y+(drift_noise-0.5)*0.20+windward_drift+couloir_pocket*0.14);
        let snow_field=cold*select(0.55,0.18,cam.scene_seed.y>=9u)
                      +smoothstep(94.0,154.0,q.y+(drift_noise-0.5)*24.0+couloir_pocket*16.0)*0.64
                      +drift_noise*0.24;
        let snow=smoothstep(0.46,0.76,snow_field)*slope_hold;
        color=mix(color,vec3<f32>(0.80,0.86,0.91),snow);
        // Weathered rock outcrops, tilted geological strata, and talus scree chutes
        // share the same climate evaluation, including the backdrop (kind zero).
        let scree_noise=surface_noise(q.xz/26.0+vec2<f32>(-14.7,31.9))*0.60
                       +surface_noise(q.xz/7.8+vec2<f32>(23.4,-18.6))*0.40;
        let exposure=1.0-smoothstep(0.46,0.84,normal.y+(scree_noise-0.5)*0.20);
        let dip_coord = q.x * 0.038 + q.z * 0.029 + (drift_noise - 0.5) * 2.2;
        let strata=surface_noise(vec2<f32>(dip_coord, q.y*0.16 + sin(dip_coord * 1.4) * 0.8))*0.65
                  +surface_noise(vec2<f32>(q.x*0.13-q.z*0.09,q.y*0.34+mottle*0.8))*0.35;
        let limestone_band = smoothstep(0.68, 0.86, sin(q.y * 0.24 + dip_coord * 3.2) * 0.5 + 0.5);
        let rock_base = mix(vec3<f32>(0.16,0.18,0.20),vec3<f32>(0.25,0.20,0.15),heat);
        let mineral = mix(rock_base, vec3<f32>(0.28, 0.27, 0.24), limestone_band * 0.45) * (0.78 + strata * 0.44);
        let alpine_rock=smoothstep(52.0,108.0,q.y+(scree_noise-0.5)*20.0)*smoothstep(0.10,0.48,1.0-normal.y+(scree_noise-0.5)*0.16);
        let exposed=clamp(max(exposure*0.48,alpine_rock*0.68),0.0,1.0);
        color=mix(color,mineral*(0.86+mottle*0.28),exposed*(1.0-snow*0.78));
        let heath=smoothstep(0.53,0.76,mottle)*(1.0-forest)*0.30;
        color=mix(color,vec3<f32>(0.21,0.16,0.075),heath*up*(1.0-snow));
        color=mix(color,soil,smoothstep(0.16,0.40,clumps)*0.24*up*(1.0-snow));
        // Climate-driven habitat detail preserves saved terrain boundaries.
        let marsh = smoothstep(0.65,0.88,wet)*(1.0-smoothstep(18.0,30.0,q.y))*smoothstep(14.5,16.5,q.y);
        color=mix(color,vec3<f32>(0.065,0.105,0.043),marsh*up*0.38*(1.0-coastal_dune));
        let heather=cool*(1.0-forest)*smoothstep(0.57,0.77,mottle);
        color=mix(color,vec3<f32>(0.18,0.105,0.14),heather*up*0.24*(1.0-snow));
        if cam.scene_seed.y >= 10u {
            let savanna=smoothstep(0.50,0.65,heat)*(1.0-smoothstep(0.38,0.53,wet));
            let rainforest=smoothstep(0.50,0.65,heat)*smoothstep(0.60,0.78,wet);
            color=mix(color,vec3<f32>(0.32,0.255,0.085)*(0.85+mottle*0.3),savanna*up*0.62*(1.0-coastal_dune));
            color=mix(color,vec3<f32>(0.042,0.105,0.031),rainforest*up*0.38);
        }
    } else if kind==2 {
        color=mix(base,soil,0.55);
    } else if kind==3 {
        // Mineral bedding is continuous, never quantized into contour bands.
        let bedding=surface_noise(vec2<f32>(q.x*0.11+q.z*0.07,q.y*0.42+mottle*0.8));
        color=base*mix(vec3<f32>(0.94,0.99,1.06),vec3<f32>(1.12,1.02,0.87),heat)
            *(0.87+bedding*0.24);
    } else if kind==4 {
        // Climate can weather sand, but cannot replace an editable material
        // with turf. Placed sand and inland dunes must retain their identity.
        let shore_moisture=1.0-smoothstep(11.8,12.95+mottle*0.35,q.y);
        let dry_sand=mix(base,sand,0.48)*(0.98+mottle*0.08);
        let wet_sand=dry_sand*vec3<f32>(0.72,0.69,0.64);
        color=mix(dry_sand,wet_sand,shore_moisture*0.65);
        let ripple_coord=q.xz*vec2<f32>(0.85,0.52)+vec2<f32>(surface_noise(q.xz*0.18)*2.4,mottle*3.2);
        let phase=(ripple_coord.x+ripple_coord.y)*18.0;
        let attenuation=exp(-0.5*pow(fp*18.0,2.0));
        color*=1.0+sin(phase)*0.028*attenuation*(1.0-shore_moisture*0.5);
    } else if kind==16 {
        color=mix(base,vec3<f32>(0.25,0.18,0.33),smoothstep(0.42,0.72,mottle)*0.4);
    }
    return max(color*(0.90+mottle*0.20+grain*0.18+clumps*0.12),vec3<f32>(0.002));
}
fn block_surface(base: vec3<f32>, p: vec3<f32>, normal: vec3<f32>, kind: i32, footprint: f32) -> vec3<f32> {
    if kind==1 || kind==2 || kind==3 || kind==4 || kind==7 || kind==16 { return natural_surface(base,p,normal,kind,footprint); }
    if kind==8 {return base;}
    if (kind==5 || kind==6) && footprint*cam.world_params.x>0.5 {
        return base*(0.88+surface_noise(p.xz*cam.world_params.x/17.0)*0.24);
    }
    var uv = p.xz;
    if abs(normal.x) > 0.5 { uv = p.zy; }
    else if abs(normal.z) > 0.5 { uv = p.xy; }
    let metric_footprint=footprint*cam.world_params.x;
    let fine_fade = 1.0 - smoothstep(0.035, 0.095, metric_footprint);
    let medium_fade = 1.0 - smoothstep(0.08, 0.4, metric_footprint);
    let physical_uv = uv * cam.world_params.x;
    let landscape_p = p.xz * cam.world_params.x;
    // Meter-scale color survives when individual blades and voxel faces no
    // longer resolve. The same continuous field spans every mesh/LOD boundary.
    let broad = surface_noise(landscape_p / 83.0);
    let landscape_patch = surface_noise(landscape_p / 17.0 + vec2<f32>(31.0, -12.0));
    let ridge = surface_noise(landscape_p / 5.2 + vec2<f32>(-73.0, 22.0));
    let patch_keep = 1.0 - smoothstep(4.0, 18.0, metric_footprint);
    let land_tone = mix(broad, broad * 0.50 + landscape_patch * 0.32 + ridge * 0.18, patch_keep);
    // Close surface painting shares one 16-texel/metre grid.
    // Only the near detail snaps; distant colors retain the filtered base.
    uv = mix(physical_uv,(floor(physical_uv*16.0)+0.5)/16.0,fine_fade);
    let grain = (surface_hash(floor(physical_uv * 16.0)) - 0.5) * fine_fade;
    let mottling = (surface_noise(physical_uv * 3.0) - 0.5) * medium_fade;
    let mineral_grain = (surface_noise(physical_uv * 11.0 + vec2<f32>(19.0, 53.0)) - 0.5) * fine_fade;
    var color = base * (1.0 + grain * 0.12 + mottling * 0.14 + mineral_grain * 0.08);
    if kind == 5 {
        if abs(normal.y) > 0.5 {
            let center = fract(uv) - vec2<f32>(0.5);
            let rings = sin(length(center) * 70.0 + surface_noise(uv * 6.0) * 2.0) * fine_fade;
            color = base * vec3<f32>(1.32, 1.22, 1.05) * (1.0 + rings * 0.18 + grain * 0.2);
        } else {
            let bark = surface_noise(vec2<f32>(uv.x * 13.0, uv.y * 2.0));
            let furrow = smoothstep(0.22, 0.46, bark);
            color *= mix(1.0, 0.58 + furrow * 0.57, medium_fade);
        }
    } else if kind == 6 {
        let clusters = surface_noise(uv * 4.0);
        let grove = surface_noise(landscape_p / 34.0 + vec2<f32>(8.0, 61.0));
        let pine = step(base.r, base.b);
        let summer = mix(vec3<f32>(0.038, 0.088, 0.032), vec3<f32>(0.090, 0.145, 0.052), land_tone);
        let autumn = mix(vec3<f32>(0.145, 0.078, 0.031), vec3<f32>(0.205, 0.112, 0.044), smoothstep(0.78, 0.95, grove));
        let broadleaf = mix(summer, autumn, smoothstep(0.68, 0.90, grove) * 0.62);
        let needles = mix(vec3<f32>(0.030, 0.066, 0.046), vec3<f32>(0.064, 0.112, 0.078), land_tone);
        let leaf = fract(uv * 4.0) - 0.5;
        let leaf_shape = 1.0-smoothstep(0.34, 0.48, abs(leaf.x + leaf.y * 0.35) + abs(leaf.y) * 0.65);
        let leaf_light = select(-0.08, 0.10, leaf.x > -leaf.y * 0.35) * leaf_shape;
        // Preserve species and climate tint authored by world generation.
        // Replacing it with a universal dark palette flattened every biome.
        let foliage_palette = mix(mix(broadleaf, needles, pine), base, 0.48);
        color = foliage_palette * (1.08 + (clusters-0.5) * 0.18 * medium_fade + leaf_light * fine_fade + grain * 0.08);
    } else if kind == 9 {
        color *= 1.0 + mottling * 0.2;
    } else if kind == 11 {
        let plank_uv = uv * vec2<f32>(1.0, 4.0);
        let local = fract(plank_uv);
        let border = min(min(local.x, 1.0-local.x), min(local.y, 1.0-local.y));
        let seam = 1.0-smoothstep(0.015, 0.045+metric_footprint*4.0, border);
        let wood = sin(uv.x*80.0 + surface_noise(uv*vec2<f32>(3.0,18.0))*6.0) * fine_fade;
        let board = surface_hash(floor(plank_uv));
        color = base * (0.88+board*0.22+wood*0.075) * (1.0-seam*0.48*medium_fade);
    } else if kind == 12 {
        var brick_uv = uv * vec2<f32>(2.0,4.0);
        brick_uv.x += (floor(brick_uv.y)%2.0)*0.5;
        let local = fract(brick_uv);
        let border = min(min(local.x,1.0-local.x),min(local.y,1.0-local.y));
        let mortar = 1.0-smoothstep(0.025,0.065+metric_footprint*3.0,border);
        let variation = surface_hash(floor(brick_uv));
        color = base*(0.82+variation*0.32+grain*0.22);
        color = mix(color, vec3<f32>(0.09,0.085,0.075), mortar*medium_fade*0.8);
    } else if kind == 13 {
        let q = uv*4.0;
        let cell = floor(q); let local = fract(q);
        var nearest = 8.0; var second = 8.0;
        for (var j = -1; j <= 1; j++) { for (var i = -1; i <= 1; i++) {
            let offset = vec2<f32>(f32(i),f32(j));
            let stone = offset + vec2<f32>(surface_hash(cell+offset),surface_hash(cell+offset+vec2<f32>(19.0)))*0.6+0.2;
            let d = length(stone-local);
            second = min(second,max(nearest,d)); nearest = min(nearest,d);
        }}
        let joint = 1.0-smoothstep(0.02,0.12+metric_footprint*3.0,second-nearest);
        color *= (1.0-joint*0.58*medium_fade)*(1.0+mottling*0.4);
    } else if kind == 14 || kind == 15 || kind == 20 || kind == 21 {
        let deposit = surface_noise(uv * vec2<f32>(4.0, 2.0));
        let fleck = surface_hash(floor(uv * 12.0));
        let vein = smoothstep(0.53, 0.66, deposit);
        let ore = vein * smoothstep(0.22, 0.42, fleck) * medium_fade;
        let ore_light = 0.83 + fleck * 0.38;
        var ore_color = select(vec3<f32>(0.018,0.02,0.022),vec3<f32>(0.34,0.16,0.07),kind==15);
        if kind == 20 { ore_color = vec3<f32>(0.46,0.22,0.075); }
        if kind == 21 { ore_color = vec3<f32>(0.42,0.55,0.62); }
                color = mix(color,ore_color*ore_light,ore*0.92);
        let rim=smoothstep(0.47,0.56,deposit)*medium_fade*(1.0-ore);
        color*=1.0-rim*0.22;
    } else if kind == 17 {
        color *= 0.8 + surface_noise(vec2<f32>(uv.x*22.0,uv.y*2.0))*0.35;
    } else if kind == 18 || kind == 19 {
        let spot_cell=floor(uv*4.0);
        let spot_pos=fract(uv*4.0)-0.5;
        let spots=(1.0-smoothstep(0.2,0.32,length(spot_pos)))*step(0.3,surface_hash(spot_cell));
        color = mix(color,vec3<f32>(0.84,0.79,0.61),spots*medium_fade);
        if normal.y < -0.5 {
            let gills=step(0.48,fract((uv.x+uv.y)*14.0));
            color=base*mix(0.6,1.2,gills)* (1.0+grain*0.15);
        }
    } else if kind == 22 {
        color *= 0.6+surface_noise(floor(uv*4.0))*0.8;
    } else if kind >= 23 && kind <= 25 {
        let panel=fract(uv);
        let rim=1.0-smoothstep(0.025,0.07,min(min(panel.x,panel.y),min(1.0-panel.x,1.0-panel.y)));
        color*=1.0-rim*0.55*medium_fade;
        if kind == 23 {
            // Salvage cases: timber slats, iron straps and a brass keeper.
            let slat=1.0-smoothstep(0.015,0.035,min(fract(panel.y*4.0),1.0-fract(panel.y*4.0)));
            color*=1.0-slat*0.22*medium_fade;
            let straps=1.0-smoothstep(0.035,0.052,min(abs(panel.x-0.18),abs(panel.x-0.82)));
            color=mix(color,vec3<f32>(0.095,0.12,0.13),straps*medium_fade);
            let studs=1.0-smoothstep(0.018,0.031,length(abs(panel-0.5)-vec2<f32>(0.32,0.39)));
            color=mix(color,vec3<f32>(0.44,0.47,0.45),studs*medium_fade);
            let keeper=1.0-smoothstep(0.10,0.115,abs(panel.x-0.5)+abs(panel.y-0.5));
            color=mix(color,vec3<f32>(0.48,0.28,0.075),keeper*medium_fade);
        }
        if kind == 25 {
            let vent=step(0.18,panel.x)*step(panel.x,0.82)*step(0.22,panel.y)*step(panel.y,0.72);
            color=mix(color,vec3<f32>(0.13,0.055,0.018),vent*medium_fade);
        }
    }
    if kind >= 26 && kind <= 32 {
        // Assemblies retain metre-sized panels even at quarter-metre voxels.
        let panel = fract(physical_uv);
        let edge = min(min(panel.x,panel.y),min(1.0-panel.x,1.0-panel.y));
        let frame = 1.0-smoothstep(0.035,0.075,edge);
        let bolts = 1.0-smoothstep(0.027,0.046,length(abs(panel-0.5)-vec2<f32>(0.39)));
        color = mix(color,vec3<f32>(0.035,0.045,0.055),frame*medium_fade);
        color = mix(color,vec3<f32>(0.46),bolts*medium_fade);
        if kind == 30 || kind == 31 {
            // Copper transport junctions and teal electrical junctions are
            // readable independently of the inventory icons.
            let port = 1.0-smoothstep(0.24,0.29,length(panel-0.5));
            let inner = 1.0-smoothstep(0.17,0.20,length(panel-0.5));
            let stripe = step(0.46,panel.x)*step(panel.x,0.54);
            let accent = select(vec3<f32>(0.52,0.22,0.045),vec3<f32>(0.055,0.48,0.50),kind==31);
            color = mix(color,accent,max(port,stripe)*medium_fade);
            color = mix(color,vec3<f32>(0.018,0.023,0.027),inner*medium_fade);
        } else {
            let screen = step(0.20,panel.x)*step(panel.x,0.80)*step(0.37,panel.y)*step(panel.y,0.78);
            let slats = step(0.46,fract(panel.y*14.0));
            var accent = vec3<f32>(0.07,0.09,0.12);
            if kind==27 { accent=vec3<f32>(0.36,0.24,0.07)*select(0.35,1.0,slats>0.5); }
            if kind==28 { accent=vec3<f32>(0.20,0.12,0.07)*select(0.35,1.0,slats>0.5); }
            if kind==32 { accent=vec3<f32>(0.045,0.24,0.15)*(0.65+surface_noise(physical_uv*9.0)*0.5); }
            color=mix(color,accent,screen*medium_fade);
            let band=step(0.12,panel.y)*step(panel.y,0.22)*step(0.12,panel.x)*step(panel.x,0.88);
            let warning=select(vec3<f32>(0.045),vec3<f32>(0.58,0.37,0.07),fract((panel.x+panel.y)*7.0)>0.5);
            color=mix(color,warning,band*medium_fade);
        }
    }
    if kind==41 {
        // Brushed enamel, edge wear and small pits on moving machinery.
        let brush=surface_hash(floor(physical_uv*vec2<f32>(12.0,128.0)))-0.5;
        let scratches=step(0.95,surface_hash(floor(physical_uv*vec2<f32>(64.0,8.0))));
        color*=1.0+(brush*0.12+scratches*0.14)*fine_fade;
        let patina=smoothstep(0.64,0.86,surface_noise(physical_uv*5.0));
        let pits=step(0.985,surface_hash(floor(physical_uv*64.0)));
        color=mix(color,color*vec3<f32>(0.65,0.78,0.73),patina*0.25*medium_fade);
        color*=1.0-pits*0.25*fine_fade;
    }
    return max(color, vec3<f32>(0.0));
}

struct WorldOutput {
    @location(0) color: vec4<f32>,
    @location(1) receiver: vec4<f32>,
}
fn debug_surface_output(color: vec4<f32>) -> WorldOutput {
    return WorldOutput(color, vec4<f32>(0.0));
}
@fragment
fn fs_main(in: VsOut) -> WorldOutput {
    // Evaluate derivatives before material-dependent control flow/discards.
    let world_dx = dpdx(in.world_pos);
    let world_dy = dpdy(in.world_pos);
    var albedo = pow(clamp(in.base_color, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(2.2));
    let kind = i32(round(in.surface.z));
    let footprint = max(length(dpdx(in.voxel_pos)), length(dpdy(in.voxel_pos)));
    let material_tint=in.material_tint;
    let material_props=in.material_props;
    var roughness = clamp(select(in.surface.x,material_tint.w,material_tint.w>=0.0), 0.08, 1.0);
    var reflectance = clamp(material_props.x,0.0,1.0);
    let is_torch_material = in.surface.w < 0.0;
    // Regular resident vertices encode cached overhead visibility in [4,5].
    // Preserve dedicated torch/fire metadata and unencoded horizon geometry.
    var sky_access = select(1.0, clamp(in.surface.w - 4.0, 0.006, 1.0),
        in.surface.w >= 4.0 && kind != 40 && in.ao_strength >= 0.0);
    if kind == 8 && in.surface.w >= 8.0 && in.ao_strength >= 0.0 {
        sky_access = in.water_optics.y;
    }

    let geometric_normal = normalize(in.normal);
    var n = geometric_normal;
    if (kind == 1 || kind == 7) && n.y > 0.15 {
        n = normalize(mix(n, vec3<f32>(0.0,1.0,0.0), 0.30));
    }
    if in.ao_strength >= 0.0 || in.ao_strength < -1.5 || kind != 0 {
        let detail_color=block_surface(albedo, in.voxel_pos/max(material_props.y,0.0625), n, kind, footprint/max(material_props.y,0.0625));
        albedo=max(mix(albedo,detail_color,clamp(material_props.z,0.0,2.0)),vec3<f32>(0.0));
    } else {
        let q_far = in.voxel_pos * cam.world_params.x;
        let fp_far = footprint * cam.world_params.x;
        // Evaluate material zones using the unperturbed macro normal so snow
        // and scree boundaries never chatter with high-frequency normal noise.
        albedo = natural_surface(albedo, in.voxel_pos, n, 0, footprint);
        // Apply multi-scale geological relief and ridged arête facet shading after
        // surface coloration so distant mountains have crisp pyramidal crags & couloirs.
        let crag_weight = clamp((1.0 - geometric_normal.y) * 1.65 + smoothstep(42.0, 115.0, q_far.y) * 0.58, 0.0, 1.0);
        let filter_macro = exp(-0.5 * pow(fp_far * 0.022, 2.0));
        let filter_meso  = exp(-0.5 * pow(fp_far * 0.078, 2.0));
        let filter_micro = exp(-0.5 * pow(fp_far * 0.22, 2.0));
        let rot_far = mat2x2<f32>(0.80, -0.60, 0.60, 0.80);
        let g_macro = surface_noise_grad(q_far.xz * 0.018 + vec2<f32>(13.7, -29.4));
        let g_meso  = surface_noise_grad((rot_far * q_far.xz) * 0.058 + vec2<f32>(-41.2, 17.8));
        let g_micro = surface_noise_grad(q_far.xz * 0.165 + vec2<f32>(27.1, 53.6));
        // Ridged derivative: d/dx (1 - |2n - 1|) flips sign across arête crests,
        // producing sharp angular mountain facets rather than round pillowy bumps.
        let arete_macro = -tanh((g_macro.x - 0.5) * 6.0) * g_macro.yz;
        let arete_meso  = -tanh((g_meso.x - 0.5) * 6.0) * (rot_far * g_meso.yz);
        let ridge_grad = mix(g_macro.yz, arete_macro, crag_weight * 0.65) * (0.16 * filter_macro)
                       + mix(rot_far * g_meso.yz, arete_meso, crag_weight * 0.72) * (0.12 * filter_meso * (0.35 + 0.65 * crag_weight))
                       + g_micro.yz * (0.075 * filter_micro * crag_weight);
        n = normalize(n + vec3<f32>(ridge_grad.x, 0.0, ridge_grad.y));
        // Couloir crevice occlusion and sunlit arête crest highlight.
        let ridge_crest = 1.0 - abs(g_macro.x * 2.0 - 1.0);
        let couloir_groove = 1.0 - abs(g_meso.x * 2.0 - 1.0);
        let gully_ao = mix(0.80, 1.10, clamp(ridge_crest * 0.62 + (1.0 - couloir_groove) * 0.38, 0.0, 1.0));
        albedo *= mix(1.0, gully_ao, (0.42 + 0.58 * crag_weight) * filter_macro);
    }
    var water_depth = 10.0;
    var water_foam = 0.0;
    var water_jacobian = 1.0;
    if kind == 8 && n.y > 0.5 {
        let t = light.torch_meta.y;
        let p = in.world_pos.xz;
        let fp = footprint * cam.world_params.x;

        // The far water lattice carries bed depth in [0,3], separate from
        // resident sky-access metadata [4,5].
        let is_far = in.ao_strength < 0.0 && in.ao_strength > -1.5;
        let resident_depth = in.water_optics.x;
        let depth = max(select(resident_depth, in.surface.w * 16.0, is_far), 0.04);
        water_depth = depth;

        // Domain-warped organic flow field bends wave fronts around shoals and
        // prevents repeating rectilinear grid patterns across oceans & lakes.
        let flow_a = surface_noise_grad(p * 0.024 + vec2<f32>(t * 0.018, -t * 0.013));
        let flow_b = surface_noise_grad(p * 0.061 + vec2<f32>(-t * 0.025, t * 0.021));
        let domain_warp = (vec2<f32>(flow_a.x, flow_b.x) - vec2<f32>(0.5)) * 8.5
                        + (flow_a.yz - flow_b.yz) * 2.6;
        let wp = p + domain_warp;

        let rot1 = mat2x2<f32>(0.80, -0.60, 0.60, 0.80);
        let rot2 = mat2x2<f32>(0.60,  0.80, -0.80, 0.60);
        let p0 = wp * 0.16 + vec2<f32>(t * 0.21, -t * 0.13);
        let p1 = (rot1 * wp) * 0.42 + vec2<f32>(-t * 0.34, t * 0.26) + flow_a.yz * 1.4;
        let p2 = (rot2 * p) * 1.08 + vec2<f32>(t * 0.58, t * 0.43) + flow_b.yz * 0.9;
        let p3 = (rot1 * p) * 2.65 + vec2<f32>(-t * 0.92, -t * 0.67);

        let w0 = exp(-0.5 * pow(fp * 0.26, 2.0));
        let w1 = exp(-0.5 * pow(fp * 0.72, 2.0));
        let w2 = exp(-0.5 * pow(fp * 1.75, 2.0));
        let w3 = exp(-0.5 * pow(fp * 4.20, 2.0));

        let n0 = surface_noise_grad(p0);
        let n1 = surface_noise_grad(p1);
        let n2 = surface_noise_grad(p2);
        let n3 = surface_noise_grad(p3);

        // Shallow-water dispersion & wave shoaling: omega(k) = sqrt(g*k*tanh(k*h)).
        // As depth decreases toward shore, wave phase slows and crests steepen.
        let shoal_boost = 1.0 + 0.42 * exp(-depth * 0.95) * smoothstep(0.06, 0.42, depth);
        let shoal_lag = (1.0 - tanh(min(depth * 0.85, 3.0))) * 2.1;

        let d0 = vec2<f32>(0.88, 0.47);
        let d1 = vec2<f32>(0.52, -0.85);
        let d2 = vec2<f32>(-0.74, 0.67);
        let d3 = vec2<f32>(-0.31, -0.95);
        let d4 = vec2<f32>(0.96, -0.28);

        let ph0 = dot(wp, d0) * 0.26 - t * 1.45 + shoal_lag + n0.x * 2.8;
        let ph1 = dot(wp, d1) * 0.43 - t * 1.88 + shoal_lag * 1.3 + n1.x * 2.4;
        let ph2 = dot(wp, d2) * 0.68 - t * 2.35 + n0.y * 2.1;
        let ph3 = dot(p,  d3) * 1.15 - t * 3.05 + n2.x * 1.9;
        let ph4 = dot(p,  d4) * 1.85 - t * 3.85 + n1.y * 1.6;

        // Trochoidal / Gerstner wave slopes: sin(ph) / (1 - Q*cos(ph)) sharpens crests
        // and widens troughs, while the Jacobian J tracks crest convergence.
        let s0 = sin(ph0) / (1.0 - 0.34 * cos(ph0));
        let s1 = sin(ph1) / (1.0 - 0.31 * cos(ph1));
        let s2 = sin(ph2) / (1.0 - 0.27 * cos(ph2));
        let s3 = sin(ph3) / (1.0 - 0.24 * cos(ph3));
        let s4 = sin(ph4) / (1.0 - 0.22 * cos(ph4));

        let gerstner_grad = (d0 * (s0 * 0.032) + d1 * (s1 * 0.026)) * w0 * shoal_boost
                          + (d2 * (s2 * 0.021) + d3 * (s3 * 0.016)) * w1 * shoal_boost
                          + d4 * (s4 * 0.012 * w2);

        // High-frequency capillary cusp ripples & diamond interference facets
        // give close-up water a crisp liquid surface texture.
        let cap_a = surface_noise_grad(p2 * 1.35 + n1.yz * 0.65);
        let cap_b = surface_noise_grad(p3 * 1.25 - n2.yz * 0.55);
        let cusp_x = (1.0 - abs(cap_a.x * 2.0 - 1.0)) * (1.0 - abs(cap_b.x * 2.0 - 1.0));
        let fbm_grad = n0.yz * (0.046 * w0)
                     + (rot1 * n1.yz) * (0.040 * w1)
                     + (rot2 * (n2.yz + cap_a.yz * 0.55)) * (0.032 * w2)
                     + (rot1 * (n3.yz + cap_b.yz * 0.65)) * (0.021 * w3);

        let gradient = gerstner_grad + fbm_grad;
        n = normalize(n + vec3<f32>(gradient.x, 0.0, gradient.y));

        let jac_compress = (0.34 * cos(ph0) + 0.31 * cos(ph1)) * w0
                         + (0.27 * cos(ph2) + 0.24 * cos(ph3)) * w1;
        water_jacobian = clamp(1.0 - jac_compress * 0.55 * shoal_boost, 0.35, 1.65);

        // Snell's Law refraction (n_air / n_water = 1.0 / 1.333 = 0.75) shifts
        // the apparent seabed position and drives refracted sand ripple detail.
        let v_inc = normalize(in.world_pos - cam.camera_pos.xyz);
        let refracted = refract(v_inc, n, 0.75);
        let refract_depth = min(depth, 4.2);
        let parallax_xz = (refracted.xz - v_inc.xz) * (refract_depth / max(-refracted.y, 0.24)) * (1.0 - f32(is_far));
        let bed_p = p + parallax_xz;

        let turbidity = surface_noise(p / 340.0);
        let sand_bed = mix(vec3<f32>(0.185, 0.168, 0.108), vec3<f32>(0.105, 0.178, 0.148), turbidity);
        let turquoise_mid = mix(vec3<f32>(0.022, 0.118, 0.114), vec3<f32>(0.016, 0.104, 0.128), turbidity);
        let deep_ocean = mix(vec3<f32>(0.007, 0.039, 0.056), vec3<f32>(0.005, 0.033, 0.062), turbidity);
        let shallow_t = 1.0 - exp(-depth * 0.62);
        let deep_t = 1.0 - exp(-max(depth - 1.2, 0.0) * 0.18);
        var water_col = mix(sand_bed, turquoise_mid, shallow_t);
        water_col = mix(water_col, deep_ocean, deep_t);

        if !is_far {
            // Refracted seabed ripples & mineral grain visible through clear shallows.
            let bed_ripple = sin(dot(bed_p, vec2<f32>(0.82, 0.57)) * 3.6 + surface_noise(bed_p * 0.9) * 3.2);
            let bed_grain = surface_noise(bed_p * 4.2);
            let bed_detail = (0.91 + 0.09 * bed_ripple + 0.06 * (bed_grain - 0.5)) * (0.94 + 0.08 * cusp_x * w2);
            let refracted_bed = albedo * bed_detail;

            // Physical Beer-Lambert spectral absorption along the refracted ray + solar path.
            let optical_path = depth * (1.0 / max(-refracted.y, 0.22) + 0.45 / max(light.sun_dir_intensity.y + 0.28, 0.25));
            let transmission = exp(-vec3<f32>(0.46, 0.24, 0.13) * optical_path);
            water_col = water_col * (vec3<f32>(1.0) - transmission) + refracted_bed * transmission;
        }

        // Dual-layer refracted sun caustics with subtle R/G/B chromatic dispersion.
        let shallow_vis = exp(-depth * 0.52) * w2;
        let c_uv = bed_p * 1.52 + n.xz * (refract_depth * 0.85);
        let ca_r = surface_noise(c_uv + vec2<f32>(t * 0.44 + 0.035, -t * 0.32));
        let cb_r = surface_noise((rot1 * c_uv) * 1.14 + vec2<f32>(-t * 0.38, t * 0.49 - 0.035));
        let ca_g = surface_noise(c_uv + vec2<f32>(t * 0.44, -t * 0.32));
        let cb_g = surface_noise((rot1 * c_uv) * 1.14 + vec2<f32>(-t * 0.38, t * 0.49));
        let ca_b = surface_noise(c_uv + vec2<f32>(t * 0.44 - 0.035, -t * 0.32 + 0.025));
        let cb_b = surface_noise((rot1 * c_uv) * 1.14 + vec2<f32>(-t * 0.38 - 0.025, t * 0.49 + 0.035));
        let caustic_rgb = vec3<f32>(
            smoothstep(0.60, 0.93, 1.0 - abs(ca_r - cb_r) * 2.25),
            smoothstep(0.60, 0.93, 1.0 - abs(ca_g - cb_g) * 2.25),
            smoothstep(0.60, 0.93, 1.0 - abs(ca_b - cb_b) * 2.25)
        );
        water_col += caustic_rgb * vec3<f32>(0.075, 0.105, 0.092) * shallow_vis;

        // Cellular perforated shoreline swash foam & Jacobian-driven breaking whitecaps.
        let bubble_fine = surface_noise(bed_p * 5.8 + n1.yz * 1.1 + vec2<f32>(t * 0.25, -t * 0.19));
        let bubble_mid  = surface_noise((rot2 * bed_p) * 2.7 - n0.yz * 0.9);
        let foam_lace = smoothstep(0.28, 0.76, bubble_fine * 0.62 + bubble_mid * 0.38);

        let shore_band = (1.0 - smoothstep(0.06, 0.82, depth)) * (1.0 - f32(is_far) * 0.7);
        let surf_ripple = sin(depth * 11.2 - t * 2.75 + n0.x * 5.0 + n1.x * 3.2);
        let swash_crest = smoothstep(-0.12, 0.62, surf_ripple + (n2.x - 0.45) * 0.85);
        let lip_contact = (1.0 - smoothstep(0.05, 0.26, depth)) * (1.0 - f32(is_far));
        let shore_foam = (shore_band * swash_crest * (0.42 + 0.58 * foam_lace) + lip_contact * (0.35 + 0.45 * foam_lace)) * (0.45 + 0.55 * w1);

        let crest_strain = max(1.0 - water_jacobian, 0.0) + length(gradient) * 2.4;
        let whitecap = smoothstep(0.24, 0.42, crest_strain) * (0.38 + 0.62 * foam_lace) * (1.0 - shore_band) * 0.38 * w1;
        water_foam = clamp(shore_foam * 0.58 + whitecap, 0.0, 0.72);
        albedo = mix(water_col, vec3<f32>(0.56, 0.61, 0.63), water_foam);
        roughness = mix(select(0.075, material_tint.w, material_tint.w >= 0.0), 0.44, water_foam);
    }
    albedo *= material_tint.rgb;
    // Wet films accumulate/dry in real time, with cached roof visibility.
    let exposure_to_rain=smoothstep(0.2,0.85,sky_access)*smoothstep(-0.15,0.65,geometric_normal.y);
    let wet=clamp(light.surface_weather.x*material_props.w*exposure_to_rain,0.0,1.0)
        *select(1.0,0.0,kind==8 || kind==9 || kind==10 || is_torch_material || kind>=40);
    let wet_patch=0.75+0.25*surface_noise(in.world_pos.xz*0.32);
    albedo*=1.0-wet*wet_patch*0.32;
    roughness=mix(roughness,min(roughness,0.20),wet*0.80);
    reflectance=mix(reflectance,max(reflectance,0.55),wet);

    // Normal variance widens the specular lobe as ripples become subpixel.
    let normal_variance = dot(dpdx(n), dpdx(n)) + dot(dpdy(n), dpdy(n));
    if kind >= 42 && kind <= 44 {
        let threshold = fract(dot(floor(in.pos.xy), vec2<f32>(0.754877666, 0.569840296)));
        // Rounded, feathered puffs suppress the opaque-looking voxel box edges.
        // World-anchored face coordinates stay continuous across chunk boundaries.
        let axis = abs(in.normal);
        let cell = fract(in.voxel_pos) - vec3<f32>(0.5);
        var uv = cell.xz;
        if axis.x > axis.y && axis.x > axis.z { uv = cell.zy; }
        else if axis.z > axis.y { uv = cell.xy; }
        let radius = length(uv * vec2<f32>(1.0, select(1.0, 0.8, kind == 42)));
        let feather = 1.0 - smoothstep(0.18, 0.50, radius);
        if threshold > select(0.30, 0.72, kind == 42) * feather { discard; }
    }
    if kind == 40 && in.surface.x < 0.0 {
        let alpha = clamp(-in.surface.x, 0.06, 1.0);
        let dither = fract(dot(floor(in.pos.xy), vec2<f32>(0.754877666, 0.569840296))
            + fract(dot(floor(in.voxel_pos * 16.0), vec3<f32>(0.31, 0.53, 0.79))));
        if dither > alpha { discard; }
    }
    roughness = clamp(sqrt(roughness * roughness + min(normal_variance, 0.25)), 0.08, 1.0);
    let specular_weight=reflectance*pow(1.0-roughness,2.0);
    let to_eye  = normalize(cam.camera_pos.xyz - in.world_pos);
    let view_dir = normalize(in.world_pos - cam.camera_pos.xyz);
    let sun_dir  = normalize(light.sun_dir_intensity.xyz);
    let moon_dir = normalize(light.moon_dir_intensity.xyz);

    let daylight   = saturate1(light.sun_color_daylight.w);
    let exposure   = max(light.ambient_exposure.w, 0.01);
    let saturation = clamp(light.sky_top_saturation.w, 0.45, 1.05);

    let linear_depth = dot(in.world_pos - cam.camera_pos.xyz, cam.camera_forward.xyz);

    let debug_mode = i32(render_debug_view());
    if debug_mode == 16 { // Shadow Cascade Index
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
        return debug_surface_output(vec4<f32>(colors[cascade_idx], 1.0));
    }

    if debug_mode == 17 { // Shadow Texel Density / Utilization
        var cascade_idx = 3;
        for (var i = 0; i < 3; i = i + 1) {
            if linear_depth < shadow.cascades[i].far_plane {
                cascade_idx = i;
                break;
            }
        }

        let cascade = shadow.cascades[cascade_idx];
        let matrix = cascade.light_view_proj;
        let projection_scale = length(vec3<f32>(matrix[0].x, matrix[1].x, matrix[2].x));
        let texel_size = 2.0 * cascade.texel_size / max(projection_scale, 1e-6);

        // Map 0 -> 2.5 world meters per texel into a 0.0 -> 1.0 scale
        // Blue means highly detailed (tight projection), Red means loose/blocky projection
        let t = clamp(texel_size / 2.5, 0.0, 1.0);

        let r = smoothstep(0.5, 1.0, t);
        let g = smoothstep(0.0, 0.5, t) - smoothstep(0.5, 1.0, t);
        let b = 1.0 - smoothstep(0.0, 0.5, t);

        return debug_surface_output(vec4<f32>(vec3<f32>(r, g, b), 1.0));
    }

    let dist = distance(in.world_pos, cam.camera_pos.xyz);
    let horizontal_dist = length((in.world_pos - cam.camera_pos.xyz).xz);
    // A heightfield cannot represent caves or excavations. Depth alone cannot
    // hide it where the detailed mesh deliberately has an opening. Keep ALL
    // backdrop geometry outside the playable inner residency, including water.
    // Outer chunks can still be streaming while moving. Exclude the
    // prioritized local coverage rather than the entire requested draw radius.
    let backdrop_start = min(256.0 * cam.world_params.x, cam.world_params.w * 0.70);
    if in.ao_strength < 0.0 && in.ao_strength > -1.5 && kind != 5 && kind != 6
        && horizontal_dist < backdrop_start { discard; }
    // Foliage proxies start where the solid heightfield does, with a short
    // overlap so voxel trees and distant crowns do not leave a bare ring.
    if in.ao_strength < 0.0 && in.ao_strength > -1.5 && (kind == 5 || kind == 6)
        && horizontal_dist < backdrop_start - 36.0 * cam.world_params.x { discard; }
    // Coarse resident cells past the handoff are full of caves and borders.
    // The heightfield covers that band; edited fallback faces still draw.
    if in.ao_strength >= 0.0 && horizontal_dist > backdrop_start + 36.0 * cam.world_params.x { discard; }
    // In the outer overlap, depth selects detailed chunks over the lowered
    // backdrop without a stippled material transition.

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
    // Diffuse weather attenuation is local too: a camera-centred clipmap
    // average must not change the fill on a stationary surface during travel.
    let local_weather = shadow_field_at_world(in.world_pos.xz);
    let heavy_cloud  = saturate1(1.0 - exp(-dot(local_weather.xyz, vec3<f32>(1.0))));
    // Preserve directional sky-dome modeling through cloud cover so overcast
    // days still cast visible tree/terrain shadows and 3D ridge definition.
    let sun_trans    = max(local_cloud_trans, mix(0.28, 0.16, heavy_cloud) * daylight);
    let moon_trans   = mix(1.0, local_cloud_trans, 0.35) * (1.0 - cloudiness * 0.10);

    // --- Direct diffuse ---
    let foliage = kind == 6;
    let sun_lambert  = max(dot(n, sun_dir), 0.0);
    let moon_lambert = max(dot(n, moon_dir), 0.0);
    let foliage_sun = max((dot(n, sun_dir) + 0.35) / 1.35, 0.0);
    let foliage_moon = max((dot(n, moon_dir) + 0.35) / 1.35, 0.0);

    // PCF shadow
    let sun_shadow_strength = 0.94 * smoothstep(0.03, 0.16, light.sun_dir_intensity.w);
    var shadow_visibility = 1.0;
    if in.ao_strength >= 0.0 && cam.scene_seed.w == 0u
        && (sun_shadow_strength > 0.0 || debug_mode == 11) {
        shadow_visibility = sample_sun_shadow(in.world_pos, linear_depth, sun_lambert, world_dx, world_dy);
    } else if in.ao_strength < 0.0 && in.ao_strength > -1.5 {
        // Distant horizon slopes self-shadow when facing away from the sun.
        shadow_visibility = smoothstep(-0.08, 0.22, dot(geometric_normal, sun_dir));
    }
    if debug_mode == 11 { return debug_surface_output(vec4<f32>(vec3<f32>(shadow_visibility), 1.0)); }

    let sun_shadow = mix(1.0, shadow_visibility, sun_shadow_strength);

    // Oren–Nayar for rough surfaces, Lambertian for smooth.
    let sun_diffuse = select(mix(sun_lambert,
                          oren_nayar(n, to_eye, sun_dir, roughness),
                          roughness * 0.22), foliage_sun, foliage)
                      * light.sun_dir_intensity.w * sun_trans * sun_shadow * 1.18;
    let moon_diffuse = select(mix(moon_lambert,
                           oren_nayar(n, to_eye, moon_dir, roughness),
                           roughness * 0.16), foliage_moon, foliage)
                       * light.moon_dir_intensity.w * moon_trans;

    // --- Ambient / sky fill (hemisphere integral approximation) ---
    // Normal-weighted sky sampling: upward-facing surfaces get top sky,
    // side-facing get horizon, downward-facing get ground bounce.
    let hemi        = saturate1(n.y * 0.5 + 0.5);
    let sky_up      = atmosphere_color(vec3<f32>(0.0, 1.0, 0.0));
    // Diffuse illumination belongs to the world, not the camera. Key the
    // horizon sample to the sun azimuth so a pan cannot relight a hillside.
    let sky_horizon = atmosphere_color(normalize(vec3<f32>(sun_dir.x, 0.16, sun_dir.z)));
    let clear_sky_ambient = mix(sky_horizon * 0.78, sky_up, pow(hemi, 0.72));
    let overcast_sky_ambient = mix(
        sky_horizon * 0.62,
        sky_horizon * 0.54 + sky_up * 0.46,
        pow(hemi, 0.85),
    );
    let sky_ambient = mix(clear_sky_ambient, overcast_sky_ambient, cloudiness * 0.65);
    let twilight_fill = 1.0 - smoothstep(-0.10, 0.22, sun_dir.y);
    let low_horizon_sky = atmosphere_color(normalize(vec3<f32>(sun_dir.x, 0.035, sun_dir.z)));
    let terrain_sky_ambient = mix(sky_ambient, mix(sky_ambient, low_horizon_sky, 0.38), twilight_fill);
    // Dense clouds remove the direct sun, but should replace it with a broad
    // bright sky dome rather than turning the terrain nearly black. Keep the
    // clear-day level restrained and let the local cloud field lift overcast.
    let sky_fill_strength = mix(mix(0.30, 0.95, cloudiness * daylight), 0.66, twilight_fill);

    // A low sun still leaves a broad cool hemisphere above shaded ground.
    // Sampling only the bright solar horizon made that fill too warm while
    // the terrain night grade crushed the opposite-facing canopy. This is
    // world-oriented, restricted to twilight, and still gated by sky access.
    let dusk_window = smoothstep(-0.16, -0.02, sun_dir.y)
        * (1.0 - smoothstep(0.10, 0.28, sun_dir.y));
    var dusk_sky_fill = vec3<f32>(0.0);
    if dusk_window > 0.001 {
        let opposite_sky = atmosphere_color(normalize(vec3<f32>(-sun_dir.x, 0.55, -sun_dir.z)));
        let cool_environment = mix(opposite_sky, sky_up, 0.50);
        // The authored sky irradiance is a bounded fallback for an extremely
        // dim LUT at blue hour; it is not emissive material light.
        let sky_floor = light.ambient_exposure.rgb * vec3<f32>(0.65, 0.85, 1.15);
        dusk_sky_fill = min(max(cool_environment, sky_floor), vec3<f32>(0.32))
            * dusk_window * (0.55 + 0.45 * hemi) * 0.65;
    }

    // Ground bounce contribution for downward-facing surfaces
    let ground_irr  = light.sun_color_daylight.rgb * sky_transmittance_to_sun(sun_dir.y)
                    * saturate1(sun_dir.y) * light.sun_dir_intensity.w * 0.05 * sun_trans;
    let ground_col  = mix(vec3<f32>(0.018, 0.025, 0.042), ground_irr, daylight);
    let ground_amt  = saturate1(1.0 - hemi) * 0.56;
    // Strengthen local cavity & canopy AO so tree trunks under foliage,
    // voxel ledges, and rock crevices have rich three-dimensional depth.
    let raw_ao      = clamp(abs(in.ao_strength), 0.0, 1.0);
    let shaped_ao   = pow(raw_ao, 1.35);
    let cavity      = mix(mix(0.32, 1.0, shaped_ao), 1.0, hemi * 0.08);
    let ambient     = (
        light.ambient_exposure.rgb * mix(vec3<f32>(0.38), vec3<f32>(1.0), hemi * hemi)
        + mix(terrain_sky_ambient,
              vec3<f32>(dot(terrain_sky_ambient, vec3<f32>(0.2126, 0.7152, 0.0722))),
              cloudiness * 0.18) * sky_fill_strength
        + ground_col * ground_amt + dusk_sky_fill
    ) * cavity * mix(1.0, 0.86, cloudiness * daylight * 0.55);

    // --- Explicit ember / flame emission ---
    // Fire retains its luminance when the environment is darkened for night.
    // Cancel both the shared exposure and the terrain-only night grade below.
    let fire_exposure = 0.9 / (exposure * mix(0.40, 1.0, daylight));
    var emissive = vec3<f32>(0.0);
    if kind == 42 { emissive += albedo * 3.5; }
    if kind == 51 { emissive += albedo * 0.42; }
    if kind >= 47 && kind <= 51 {
        var face = in.voxel_pos.xz;
        if abs(n.x) > 0.5 { face = in.voxel_pos.zy; }
        else if abs(n.z) > 0.5 { face = in.voxel_pos.xy; }
        let brush = surface_noise(face * vec2<f32>(48.0, 6.0)) - 0.5;
        let flake = surface_noise(face * 22.0) - 0.5;
        n = normalize(n + vec3<f32>(brush, flake * 0.35, brush * 0.4));
        roughness = select(0.30, select(0.12, select(0.16, select(0.15, 0.08, kind == 51), kind == 50), kind == 49), kind == 48);
        reflectance = select(0.82, 0.96, kind == 48 || kind == 49);
        albedo *= 1.0 + brush * 0.08;
    }
    if kind == 46 {
        let phase = light.torch_meta.y * 3.0 + in.world_pos.y * 8.0;
        let warp = vec3<f32>(sin(phase), cos(phase * 1.3), sin(phase * 0.7));
        n = normalize(n + warp * 0.7);
        let rim = pow(1.0 - clamp(abs(dot(n, to_eye)), 0.0, 1.0), 1.35);
        emissive += mix(albedo, albedo.brg, 0.55) * (1.1 + rim * 3.4);
        albedo *= 0.12;
    }
    if kind == 40 {
        let rim = 1.0 - clamp(abs(dot(n, to_eye)), 0.0, 1.0);
        let rim_boost = select(1.0, 0.85 + 1.45 * rim * rim, in.surface.x < 0.0);
        let hot_core = select(vec3<f32>(0.0), vec3<f32>(0.18, 0.16, 0.22) * (1.0 - rim), in.surface.x < 0.0 && in.surface.w > 2.2);
        emissive += (albedo * rim_boost + hot_core) * max(in.surface.w, 0.0) * mix(1.0, fire_exposure, 0.52);
    }
    if is_torch_material {
        let flame_wave = torch_flicker(floor(in.voxel_pos), light.torch_meta.y);
        let flame = in.surface.w < -1.5;
        // Model colours are authored in sRGB like the rest of the mesh. Feeding
        // them directly into HDR washed orange flames into pale pink/white.
        emissive += pow(in.base_color, vec3<f32>(2.2)) * select(0.65, 3.2, flame) * flame_wave * fire_exposure;
        albedo *= select(0.5, 0.03, flame);
    }

    // --- Sun/moon fill (bounced indirect) ---
    let sun_fill = light.sun_color_daylight.rgb * sun_spectral
                   * light.sun_dir_intensity.w * sun_trans
                   * (0.02 + 0.025 * daylight) * (0.55 + 0.45 * hemi);
    let moon_fill = light.moon_color_twilight.rgb * moon_spectral
                    * light.moon_dir_intensity.w * moon_trans
                    * 0.02 * (0.55 + 0.45 * hemi);

    // --- Torch lights ---
    var torch_diffuse = vec3<f32>(0.0);
    var torch_fill    = vec3<f32>(0.0);
    var torch_spec    = vec3<f32>(0.0);
    // Exact edited backdrop surfaces must receive the same local lights as
    // their resident counterparts; only the procedural far terrain skips them.
    let receives_local_light = in.ao_strength >= 0.0 || in.ao_strength < -1.5;
    let torch_count = select(0u, min(u32(light.torch_meta.x), 64u), receives_local_light);
    for (var i: u32 = 0u; i < 64u; i = i + 1u) {
        if i >= torch_count { break; }
        let ts = eval_torch_light(
            light.torch_positions[i], light.torch_colors[i],
            light.torch_meta.y, in.world_pos, n, to_eye, roughness,
        );
        torch_diffuse += ts.diffuse;
        torch_fill    += ts.fill;
        torch_spec    += ts.spec;
    }
    if receives_local_light && light.held_torch_pos_radius.w > 0.0 && light.held_torch_color_strength.w > 0.0 {
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
        (ambient + sun_fill + moon_fill) * sky_access
        + light.sun_color_daylight.rgb  * sun_spectral  * sun_diffuse  * 0.31830989
        + light.moon_color_twilight.rgb * moon_spectral * moon_diffuse * 0.34 * sky_access
        + (torch_diffuse + torch_fill) * fire_exposure
    );

    // Specular highlights
    lit += light.sun_color_daylight.rgb * sun_spectral
           * specular_term(n, to_eye, sun_dir, roughness)
           * light.sun_dir_intensity.w * sun_trans * sun_shadow
           * specular_weight * 0.022;
    lit += light.moon_color_twilight.rgb * moon_spectral
           * specular_term(n, to_eye, moon_dir, roughness)
           * light.moon_dir_intensity.w * moon_trans
           * specular_weight * 0.006 * sky_access;
    lit += torch_spec * specular_weight * fire_exposure;
    // Broad sky sheen remains visible under overcast lighting; no sharp fake mirror.
    let film_fresnel=0.025+0.25*pow(1.0-clamp(dot(n,to_eye),0.0,1.0),5.0);
    let film_sky=mix(sky_horizon,sky_up,0.6)*mix(0.12,0.55,daylight);
    lit=mix(lit,film_sky,wet*sky_access*film_fresnel*(1.0-roughness*0.6));
    lit += emissive;
    if foliage {
        // Thin-leaf transmission is driven by incoming light and shadow;
        // it cannot illuminate a canopy at night or through solid terrain.
        let backlight = pow(max(dot(-to_eye, sun_dir), 0.0), 2.5);
        let top_sheen = smoothstep(0.15, 0.85, n.y) * (0.28 + 0.72 * sun_shadow);
        let transmission = (0.055 + backlight * 0.22 + top_sheen * 0.08) * sun_shadow * sun_trans
            * light.sun_dir_intensity.w * (0.45 + 0.55 * cavity);
        let leaf_warmth = mix(albedo, albedo * vec3<f32>(1.18, 1.24, 0.72), 0.55);
        lit += leaf_warmth * light.sun_color_daylight.rgb * sun_spectral * transmission;
    }
    if kind == 18 {lit += albedo*0.22 + vec3<f32>(0.003,0.018,0.014);}
    if kind == 22 {lit += albedo*0.18;}
    if kind == 8 || kind == 9 {
        let reflected = reflect(-to_eye, n);
        let ndotv = clamp(dot(n, to_eye), 0.0, 1.0);
        // Physical dielectric Schlick Fresnel for water (n = 1.333 -> F0 = 0.0204).
        let fresnel = (0.0204 + 0.9796 * pow(1.0 - ndotv, 5.0)) * (1.0 - water_foam * 0.68);
        let refl_dir = normalize(vec3<f32>(reflected.x, max(reflected.y, 0.025), reflected.z));
        var reflection = atmosphere_color(refl_dir);
        // Preserve zenith-vs-horizon sky dome gradient and warm sun-azimuth sheen
        // even under cloudy skies so water reflections never flatten into grey paint.
        let refl_elev = clamp(refl_dir.y, 0.0, 1.0);
        let sun_align = pow(max(dot(normalize(refl_dir.xz), normalize(sun_dir.xz + vec2<f32>(1e-5))), 0.0), 3.0);
        let cloud_sky_dome = mix(
            sky_horizon * (1.08 + sun_align * 0.28 * sun_trans),
            sky_up * 0.88,
            smoothstep(0.02, 0.55, refl_elev)
        );
        let cloud_blend = clamp(max(cloudiness * 0.72, heavy_cloud * 0.68), 0.0, 0.78);
        reflection = mix(reflection, cloud_sky_dome, cloud_blend);

        // Subsurface wave-crest scatter: trochoidal crests with low Jacobian (pinched ridges)
        // tilted toward the sun transmit rich emerald-aquamarine light.
        let wave_tilt = length(n.xz);
        let crest_pinch = clamp(1.35 - water_jacobian, 0.15, 1.25);
        let forward_sun = pow(max(dot(-to_eye, sun_dir), 0.0), 2.2) * max(sun_dir.y + 0.15, 0.0);
        let sss_tint = mix(vec3<f32>(0.018, 0.25, 0.23), vec3<f32>(0.065, 0.36, 0.27), exp(-water_depth * 0.35));
        let sss_energy = (0.16 + forward_sun * 0.68 + wave_tilt * 1.9) * crest_pinch
            * (0.35 + 0.65 * sun_trans * sun_shadow)
            * mix(0.22, 1.0, daylight)
            * sky_access
            * (1.0 - water_foam * 0.75);
        lit += sss_tint * light.sun_color_daylight.rgb * sun_spectral * sss_energy * 0.17;

        // Sky radiance already follows time of day. Dimming it again made
        // night water read as black cutout shadows against illuminated banks.
        lit = mix(lit, reflection * sky_access, clamp(fresnel, 0.03, 0.88));
        let sun_glint_sharp = specular_term(n, to_eye, sun_dir, max(roughness * 0.72, 0.058));
        let sun_glint_broad = specular_term(n, to_eye, sun_dir, max(roughness * 1.60, 0.17));
        lit += light.sun_color_daylight.rgb * sun_spectral
             * (sun_glint_sharp * 0.105 + sun_glint_broad * 0.045)
             * light.sun_dir_intensity.w * (0.18 + 0.82 * sun_trans * sun_shadow) * sky_access * (1.0 - water_foam * 0.65);
    }
    if kind == 10 {
        let crust = surface_noise(in.world_pos.xz * 2.8);
        let molten = smoothstep(0.38, 0.72, crust);
        lit += mix(vec3<f32>(0.55, 0.035, 0.003), vec3<f32>(2.8, 0.65, 0.035), molten);
    }

    // ============================================================
    // AERIAL PERSPECTIVE / FOG
    //
    // Uses atmosphere_color(view_dir) for the fog target colour so distant
    // terrain inherits exactly the same sky hue as the sky pass.  This is the
    // primary cause of coherent blue distance haze.
    // ============================================================
    var fog_target = vec3<f32>(0.0);
    var total_fog_t = 0.0;

    if atmosphere_mode() >= ATMOSPHERE_MODE_SKY_VIEW_LUT {
        // Share the sky/cloud radiance response so twilight haze does not
        // retain the old brown palette around otherwise cool distant terrain.
        let ap = aerial_perspective(view_dir, dist, cam.world_params.x, vec3<f32>(0.0));

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

    // Overcast fog inherits the neutral, dimmed cloud illumination rather than
    // filling the world with bright clear-sky blue beneath a grey ceiling.
    let fog_luma = dot(fog_target, vec3<f32>(0.2126, 0.7152, 0.0722));
    fog_target = mix(fog_target, vec3<f32>(fog_luma), cloudiness * 0.65)
        * mix(1.0, 0.74, cloudiness * daylight);
    // Air between the camera and a shaded cliff still scatters light. Using
    // the endpoint's overhead visibility here erased distant haze precisely
    // on dark slopes, leaving black cutout bands against the landscape.
    lit = mix(lit, fog_target, total_fog_t);
    if cam.camera_pos.w > 0.5 {
        // Wavelength-dependent absorption gives short red visibility and a blue-green distance haze.
        let transmission = exp(-vec3<f32>(0.24, 0.075, 0.045) * dist);
        let underwater_color = vec3<f32>(0.012, 0.085, 0.11) * mix(0.18, 1.0, daylight);
        lit = lit * transmission + underwater_color * (vec3<f32>(1.0)-transmission);
    }
    lit *= mix(0.40, 1.0, daylight);
    // Streaming-edge haze must also cover downward rays while flying.
    // Kilometre-scale extinction preserves layered silhouettes instead of leaving
    // remote slopes as bright and contrasty as the foreground.
    let horizon_haze = (1.0 - exp(-max(horizontal_dist - 480.0, 0.0)
        / mix(4200.0, 7200.0, daylight))) * sky_access * 0.72;
    let raw_sky_radiance = atmosphere_color(view_dir);
    let sky_luma = dot(raw_sky_radiance, vec3<f32>(0.2126, 0.7152, 0.0722));
    let weather_sky_radiance = mix(raw_sky_radiance, vec3<f32>(sky_luma), cloudiness * 0.55)
        * mix(1.0, 0.78, cloudiness * daylight);
    lit = mix(lit, mix(fog_target, weather_sky_radiance, 0.65), horizon_haze);
    // Residency is centred on a snapped chunk, not the exact camera. Reserve
    // a chunk diagonal so the closest missing column is already fully veiled.
    let edge_fade = terrain_streaming_fade(horizontal_dist);
    if edge_fade > 0.0 {
        // This must equal the sky pass in HDR at the outer boundary.
        lit = mix(lit, raw_sky_radiance, edge_fade);
    }

    // Nearby mist sits in front of the graded surface and distant haze, just
    // as it does in the sky pass. Later lighting must use its transmittance too.
    let mist = local_mist(view_dir, dist, sky_access, weather_sky_radiance);
    lit = lit * mist.a + mist.rgb;
    let surface_visibility = (1.0-total_fog_t) * (1.0-edge_fade) * (1.0-horizon_haze) * mist.a;

    // Sky, terrain and premultiplied clouds share one HDR resolve.
    var receiver = vec4<f32>(0.0);
    if kind == 8 && geometric_normal.y>0.5 {
        // Negative alpha tags reflection receivers. Reuse the existing HDR
        // receiver target: signed normal x/z, visibility, and Fresnel weight.
        let f=0.025+0.975*pow(1.0-clamp(dot(n,to_eye),0.0,1.0),5.0);
        receiver=vec4<f32>(n.x,n.z,surface_visibility*sky_access,-max(f,0.025));
    } else if kind != 8 {
        receiver = vec4<f32>(albedo * surface_visibility, max(dot(ambient * sky_access, vec3<f32>(0.2126,0.7152,0.0722)),0.0));
    }
    return WorldOutput(vec4<f32>(max(lit, vec3<f32>(0.0)), 1.0), receiver);
}

