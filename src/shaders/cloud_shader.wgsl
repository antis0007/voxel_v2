// ============================================================
// cloud_shader.wgsl — Volumetric cloud renderer
//
// Techniques: Schneider/Hillaire/Häggström papers
//   - Slab-intersection bounded raymarching (no wasted samples)
//   - Proper step count derived from actual slab path length
//   - Deterministic segment integration (temporal reconstruction optional)
//   - Curl-noise distorted detail erosion for realistic cloud edges
//   - World-space weather cell system: large bilinearly-blended cells,
//     per-cell transformed shape/detail domains
//   - Continuous weather sampled along view and shadow rays
//   - Distance-based detail LOD (full detail near, shape-only far)
//   - Integrated four-tap low/mid cloud self-shadowing
//   - Aerial perspective at each layer's visible mean depth
//   - Wind-sheared precipitation curtains attached to storm banks
//   - Depth-buffer occlusion: clip march at scene geometry
// ============================================================

@group(3) @binding(0) var weather_primary_tex  : texture_2d_array<f32>;
@group(3) @binding(1) var weather_secondary_tex: texture_2d_array<f32>;
@group(3) @binding(2) var cloud_field_sampler  : sampler;
@group(3) @binding(3) var shape_noise_tex      : texture_3d<f32>;
@group(3) @binding(4) var detail_noise_tex     : texture_3d<f32>;
@group(3) @binding(5) var blue_noise_tex       : texture_2d<f32>;
@group(3) @binding(6) var scene_depth_tex      : texture_depth_2d;
@group(3) @binding(7) var cloud_shadow_tex     : texture_2d_array<f32>;

struct VsOut {
    @builtin(position) pos : vec4<f32>,
    @location(0)       ndc : vec2<f32>,
};

struct CloudSample {
    radiance     : vec3<f32>,
    transmittance: f32,
    debug_metrics: vec4<f32>,
    mean_distance: f32,
};

// ============================================================
// WORLD-SPACE CLOUD FIELD
// Density is a pure function of world position + time.
// No camera-relative coordinates. Global noise domain — no cell seams.
// ============================================================

const SHAPE_TEX_SIZE  : f32 = 64.0;
const DETAIL_TEX_SIZE : f32 = 32.0;

// Weather cell size used only for the per-cell seed hash in weather_at_world().
const CLOUD_WEATHER_CELL_M : f32 = 400.0;
const CLOUD_NOISE_STABLE_REGION_M : f32 = 8192.0;
const WEATHER_UV_STABLE_REGION_M  : f32 = 8192.0;


fn stable_world_anchor_xz(world_xz: vec2<f32>, region_m: f32) -> vec2<f32> {
    return floor(world_xz / region_m) * region_m;
}

fn stable_world_anchor_y(world_y: f32, region_m: f32) -> f32 {
    return floor(world_y / region_m) * region_m;
}

fn stable_noise_uvw(
    world_xz : vec2<f32>,
    world_y  : f32,
    freq_xz  : f32,
    freq_y   : f32,
) -> vec3<f32> {
    let anchor_xz = stable_world_anchor_xz(world_xz, CLOUD_NOISE_STABLE_REGION_M);
    let anchor_y  = stable_world_anchor_y(world_y, CLOUD_NOISE_STABLE_REGION_M);

    let local_xz = world_xz - anchor_xz;
    let local_y  = world_y  - anchor_y;

    let phase = vec3<f32>(
        fract(anchor_xz.x * freq_xz),
        fract(anchor_y    * freq_y),
        fract(anchor_xz.y * freq_xz),
    );

    return fract(vec3<f32>(
        local_xz.x * freq_xz,
        local_y    * freq_y,
        local_xz.y * freq_xz
    ) + phase);
}

fn stable_noise_uvw_scaled(
    world_xz : vec2<f32>,
    world_y  : f32,
    freq_xz  : f32,
    freq_y   : f32,
    scale_xz : f32,
    scale_y  : f32,
    offset   : vec3<f32>,
) -> vec3<f32> {
    let anchor_xz = stable_world_anchor_xz(world_xz, CLOUD_NOISE_STABLE_REGION_M);
    let anchor_y  = stable_world_anchor_y(world_y, CLOUD_NOISE_STABLE_REGION_M);

    let local_xz = world_xz - anchor_xz;
    let local_y  = world_y  - anchor_y;

    let phase = vec3<f32>(
        fract(anchor_xz.x * freq_xz * scale_xz + offset.x),
        fract(anchor_y    * freq_y  * scale_y  + offset.y),
        fract(anchor_xz.y * freq_xz * scale_xz + offset.z),
    );

    return fract(vec3<f32>(
        local_xz.x * freq_xz * scale_xz,
        local_y    * freq_y  * scale_y,
        local_xz.y * freq_xz * scale_xz
    ) + phase);
}

struct CellWeather {
    coverage    : f32,
    cloud_type  : f32,
    humidity    : f32,
    convection  : f32,
    storm       : f32,
    low_amount  : f32,
    mid_amount  : f32,
    high_amount : f32,
    anvil_bias  : f32,
    seed        : f32,
};

fn authored_cloud_density() -> f32 {
    return clamp(light.cloud_authoring[0].x, 0.0, 1.0);
}

fn authored_altocumulus() -> f32 {
    return clamp(light.cloud_authoring[0].y, 0.0, 1.0);
}

fn authored_altostratus() -> f32 {
    return clamp(light.cloud_authoring[0].z, 0.0, 1.0);
}

fn authored_cirrus() -> f32 {
    return clamp(light.weather_globals[3].x, 0.0, 1.0);
}

fn authored_storm_height() -> f32 {
    return clamp(light.cloud_authoring[0].w, 0.0, 1.0);
}

fn authored_anvil_spread() -> f32 {
    return clamp(light.cloud_authoring[1].x, 0.0, 1.0);
}

fn cloud_cell_id(world_xz: vec2<f32>, cell_size: f32) -> vec2<i32> {
    let q = floor(world_xz / cell_size);
    return vec2<i32>(i32(q.x), i32(q.y));
}

fn weather_clip_region(level: i32) -> vec4<f32> {
    return light.weather_clip_regions[level];
}

fn weather_uv(world_xz: vec2<f32>, level: i32) -> vec2<f32> {
    let clip = weather_clip_region(level);

    // Important: anchor from the clip's world-space origin, NOT the camera.
    let anchor = stable_world_anchor_xz(clip.xy, WEATHER_UV_STABLE_REGION_M);

    let local_world = world_xz - anchor;
    let local_clip  = clip.xy   - anchor;

    return (local_world - local_clip) * clip.w;
}

fn weather_edge_distance(uv: vec2<f32>) -> f32 {
    let edge = min(min(uv.x, uv.y), min(1.0 - uv.x, 1.0 - uv.y));
    return edge;
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


fn weather_at_world(world_xz: vec2<f32>) -> CellWeather {
    let uv0 = weather_uv(world_xz, 0);
    let uv1 = weather_uv(world_xz, 1);
    let uv2 = weather_uv(world_xz, 2);
    let w0 = weather_level_weight(uv0, 0);
    let w1_raw = weather_level_weight(uv1, 1);
    let w1 = w1_raw * (1.0 - w0);
    let w2_raw = weather_level_weight(uv2, 2);
    let w2 = w2_raw * max(1.0 - w0 - w1, 0.0);

    var primary = vec4<f32>(0.0);
    var secondary = vec4<f32>(0.0);
    if w0 > 0.0 {
        primary += sample_weather_bilinear(weather_primary_tex, uv0, 0) * w0;
        secondary += sample_weather_bilinear(weather_secondary_tex, uv0, 0) * w0;
    }
    if w1 > 0.0 {
        primary += sample_weather_bilinear(weather_primary_tex, uv1, 1) * w1;
        secondary += sample_weather_bilinear(weather_secondary_tex, uv1, 1) * w1;
    }
    if w2 > 0.0 {
        primary += sample_weather_bilinear(weather_primary_tex, uv2, 2) * w2;
        secondary += sample_weather_bilinear(weather_secondary_tex, uv2, 2) * w2;
    }
    let cell = cloud_cell_id(world_xz, CLOUD_WEATHER_CELL_M);

    var w : CellWeather;
    w.coverage    = saturate1(primary.x);
    w.cloud_type  = saturate1(primary.y);
    w.humidity    = saturate1(primary.z);
    w.convection  = saturate1(primary.w);
    w.storm       = saturate1(secondary.x);
    w.low_amount  = saturate1(secondary.y);
    w.mid_amount  = saturate1(secondary.z);
    w.high_amount = saturate1(secondary.w);
    // Upper decks are accents; strong low cover already supplies the sky mass.
    w.mid_amount *= 1.0 - smoothstep(0.25, 0.85, w.low_amount) * 0.75;
    w.high_amount *= 1.0 - smoothstep(0.30, 0.90, w.low_amount) * 0.55;
    w.anvil_bias  = saturate1(w.storm * 0.72 + w.convection * 0.28);
    w.seed        = hash_cell1(cell, 0x1367);
    return w;
}

// ============================================================
// PER-CELL VERTICAL AABB
//
// Given a cell's weather, compute the Y extent for each layer.
// Cells with negligible amount produce degenerate AABBs (y_min >= y_max).
//
// CRITICAL: These functions are called at every march step with bilinearly
// interpolated weather values. All inputs MUST vary smoothly (C0-continuous)
// across space. DO NOT use hash functions or noise2() here — they produce
// high-frequency discontinuities that create noisy stripes when evaluated
// per-step. Only use smooth clipmap-driven weather fields.
// ============================================================

fn cell_low_aabb(w: CellWeather) -> CellAABB {
    var aabb: CellAABB;
    // Raise dead zone: only produce geometry when low_amount > 0.07 (was 0.0).
    // This prevents trace-coverage cells from creating invisible AABB volumes
    // that consume march steps and cause faint density walls at cell boundaries.
    let low_fade = smoothstep(0.01, 0.12, w.low_amount);
    if low_fade <= 0.0 {
        aabb.y_min = 0.0;
        aabb.y_max = -1.0;
        return aabb;
    }

    let base_ctl         = clamp(light.weather_globals[1].x, 0.0, 1.0);
    let thick_ctl        = clamp(light.weather_globals[1].y, 0.0, 1.0);
    let storm_ctl        = w.storm;
    let storm_height_ctl = authored_storm_height();

    // Condensation base before the shared altitude offset.
    let base_range = mix(80.0, 280.0, base_ctl);
    // Convection lowers the condensation level; humidity raises it slightly.
    let conv_lower  = mix(1.0, 0.85, w.convection);
    let humid_raise = mix(1.0, 1.06, w.humidity);
    aabb.y_min = base_range * conv_lower * humid_raise;

    // Deep updrafts preserve vertical relief above a shared condensation floor.
    // Give ordinary cumulus enough depth for tall, individually rising
    // towers; thunderheads can span multiple kilometers.
    let fair_thick  = mix(420.0, 980.0, thick_ctl);
    let storm_thick = mix(400.0, 1800.0, storm_height_ctl);
    let base_thick  = fair_thick + storm_thick * storm_ctl;
    let cell_thick  = base_thick * mix(0.60, 1.0, w.humidity)
                    * mix(0.75, 1.35, w.convection);
    aabb.y_max = aabb.y_min + cell_thick * low_fade;

    aabb.y_min += CLOUD_LOW_ALTITUDE_OFFSET_M;
    aabb.y_max += CLOUD_LOW_ALTITUDE_OFFSET_M;

    return aabb;
}

fn cell_mid_aabb(w: CellWeather) -> CellAABB {
    var aabb: CellAABB;
    // Raise dead zone to match low layer — prevents ghost AABBs from trace mid-layer.
    let mid_fade = smoothstep(0.07, 0.22, w.mid_amount);
    if mid_fade <= 0.0 {
        aabb.y_min = 0.0;
        aabb.y_max = -1.0;
        return aabb;
    }

    let mid_base_ctl    = clamp(light.weather_globals[3].y, 0.0, 1.0);
    let altostratus_ctl = authored_altostratus();
    let thick_ctl       = clamp(light.weather_globals[1].y, 0.0, 1.0);

    // Voxel-world: mid clouds at 220-420m.
    let mid_base   = mix(220.0, 420.0, mid_base_ctl)
                   * mix(1.0, 0.94, w.humidity);
    let base_thick = mix(110.0, 320.0, thick_ctl) + altostratus_ctl * 130.0;
    aabb.y_min = mid_base - mix(20.0, 45.0, altostratus_ctl);
    aabb.y_max = aabb.y_min + base_thick * mix(0.45, 1.0, w.mid_amount) * mid_fade;

    aabb.y_min += CLOUD_MID_ALTITUDE_OFFSET_M;
    aabb.y_max += CLOUD_MID_ALTITUDE_OFFSET_M;
    let nominal_min = max(mix(220.0, 420.0, mid_base_ctl) * 0.94
        - mix(20.0, 45.0, altostratus_ctl) - 2.0, 0.0) + CLOUD_MID_ALTITUDE_OFFSET_M;
    let lift = max(cloud_layer_bounds().mid_base - nominal_min, 0.0);
    aabb.y_min += lift;
    aabb.y_max += lift;

    return aabb;
}

fn cell_high_aabb(w: CellWeather) -> CellAABB {
    var aabb: CellAABB;
    // Raise dead zone for cirrus too — thin trace amounts cause faint hazing.
    let high_fade = smoothstep(0.07, 0.22, w.high_amount);
    if high_fade <= 0.0 {
        aabb.y_min = 0.0;
        aabb.y_max = -1.0;
        return aabb;
    }

    // Voxel-world: cirrus at 380-550m (near world ceiling of 512m).
    let high_base  = mix(380.0, 550.0, clamp(light.weather_globals[2].w, 0.0, 1.0))
                   * mix(1.0, 0.97, w.humidity);
    let base_thick = mix(90.0, 240.0, clamp(light.weather_globals[1].y, 0.0, 1.0));
    aabb.y_min = high_base - 20.0;
    aabb.y_max = aabb.y_min + base_thick * mix(0.50, 1.0, w.high_amount) * high_fade;

    aabb.y_min += CLOUD_HIGH_ALTITUDE_OFFSET_M;
    aabb.y_max += CLOUD_HIGH_ALTITUDE_OFFSET_M;

    return aabb;
}

// ============================================================
// CONTINUOUS 3D NOISE SAMPLING
//
// World position used directly as the noise domain.
// Independent volume orientations and frequencies reduce visible repetition.
// Coordinates are transformed before wrapping to preserve continuity.
// Base period at freq_xz=0.0010: 1000m in world space.
// ============================================================

fn sample_shape_global(
    world_xz : vec2<f32>,
    world_y  : f32,
    freq_xz  : f32,
    freq_y   : f32,
    lod      : f32,
) -> vec4<f32> {
    let uvw_a = stable_noise_uvw(
        world_xz,
        world_y,
        freq_xz,
        freq_y
    );

    // Scale into texture space BEFORE rotation, then wrap. Rotating meters
    // first leaked the high vertical cirrus frequency into horizontal axes,
    // creating tightly repeated stripes across an otherwise broad cloud deck.
    let domain = vec3<f32>(world_xz.x * freq_xz, world_y * freq_y, world_xz.y * freq_xz);
    let rotated = vec3<f32>(
        dot(domain, vec3<f32>(0.36, 0.48, -0.8)),
        dot(domain, vec3<f32>(-0.8, 0.60, 0.0)),
        dot(domain, vec3<f32>(0.48, 0.64, 0.60))
    );
    let uvw_b = stable_noise_uvw_scaled(
        rotated.xz,
        rotated.y,
        1.0,
        1.0,
        0.618,
        1.11,
        vec3<f32>(0.371, 0.537, 0.193)
    );

    let s0 = textureSampleLevel(shape_noise_tex, cloud_field_sampler, uvw_a, lod);
    let s1 = textureSampleLevel(shape_noise_tex, cloud_field_sampler, uvw_b, lod + 0.5);
    return clamp(s0 * 0.5 + s1 * 0.5, vec4<f32>(0.0), vec4<f32>(1.0));
}

fn sample_detail_global(
    world_xz : vec2<f32>,
    world_y  : f32,
    freq_xz  : f32,
    freq_y   : f32,
    lod      : f32,
) -> vec4<f32> {
    // Continuous, nonperiodic warp prevents repeated erosion stamps. Keep
    // the deformation broad so distant mips still filter the small detail.
    let warp = vec2<f32>(
        weather_bank_noise(world_xz / 310.0 + vec2<f32>(11.7, 43.1)),
        weather_bank_noise(world_xz / 370.0 + vec2<f32>(-28.4, 7.3))
    ) - vec2<f32>(0.5);
    let uvw = stable_noise_uvw(
        rotate2(world_xz, -0.71) + warp * 48.0,
        world_y,
        freq_xz,
        freq_y
    );
    return textureSampleLevel(detail_noise_tex, cloud_field_sampler, uvw, lod);
}

fn ray_cone_spread(dir: vec3<f32>) -> f32 {
    let ddx = dpdx(dir);
    let ddy = dpdy(dir);
    return max(max(length(ddx), length(ddy)), 1e-5);
}

fn footprint_texel_lod(footprint_m: f32, uv_scale_per_meter: f32, tex_size: f32, max_lod: f32) -> f32 {
    let texels = max(footprint_m * abs(uv_scale_per_meter) * tex_size, 1.0);
    return clamp(log2(texels), 0.0, max_lod);
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var out: VsOut;
    let pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 3.0,  1.0),
    );
    let ndc = pos[vi];
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.ndc = ndc;
    return out;
}

fn sky_dir(ndc: vec2<f32>) -> vec3<f32> {
    return camera_ray(ndc);
}

fn screen_uv(ndc: vec2<f32>) -> vec2<f32> {
    return clamp(vec2<f32>(ndc.x * 0.5 + 0.5, -ndc.y * 0.5 + 0.5), vec2<f32>(0.0), vec2<f32>(1.0));
}

fn clip_to_world_pos(clip: vec4<f32>) -> vec3<f32> {
    let world = cam.inv_view_proj * clip;
    let w     = world.w;
    let inv_w = sign(w) / max(abs(w), 1e-5);
    return world.xyz * inv_w;
}

fn scene_depth_distance(ndc: vec2<f32>, dir: vec3<f32>) -> f32 {
    let dims_u = textureDimensions(scene_depth_tex);
    let dims   = vec2<f32>(f32(dims_u.x), f32(dims_u.y));
    let max_xy = vec2<i32>(i32(dims_u.x) - 1, i32(dims_u.y) - 1);
    let p      = screen_uv(ndc) * dims - vec2<f32>(0.5);
    let base   = clamp(vec2<i32>(floor(p)), vec2<i32>(0), max_xy);
    let next   = min(base + vec2<i32>(1, 1), max_xy);

    let d00 = textureLoad(scene_depth_tex, base, 0);
    let d10 = textureLoad(scene_depth_tex, vec2<i32>(next.x, base.y), 0);
    let d01 = textureLoad(scene_depth_tex, vec2<i32>(base.x, next.y), 0);
    let d11 = textureLoad(scene_depth_tex, next, 0);
    let depth = min(min(d00, d10), min(d01, d11));

    if depth >= 0.9999999 {
        return 1e9;
    }

    // Recover view depth analytically, avoiding subtraction of two distant
    // world positions. The basis w components carry projection coefficients.
    let view_depth = cam.ray_up.w / min(depth + cam.ray_right.w, -1e-8);
    return view_depth / max(dot(dir, cam.camera_forward.xyz), 1e-4);
}

fn slab_intersection(ro: vec3<f32>, rd: vec3<f32>, y0: f32, y1: f32) -> vec2<f32> {
    if abs(rd.y) < 1e-5 {
        if ro.y >= y0 && ro.y <= y1 {
            return vec2<f32>(-1e9, 1e9);
        }
        return vec2<f32>(1e9, -1e9);
    }
    let t0 = (y0 - ro.y) / rd.y;
    let t1 = (y1 - ro.y) / rd.y;
    return vec2<f32>(min(t0, t1), max(t0, t1));
}

// ============================================================
// TRUE WORLD-SPACE RAY ORIGIN
// Computes the slab-entry point for this ray, making clouds a
// real volumetric object in world space rather than a billboard.
// ============================================================

struct LayerRayOrigin {
    origin : vec3<f32>,
    valid  : bool,
};

fn compute_layer_ray_origin(
    cam_pos : vec3<f32>,
    ray_dir : vec3<f32>,
    base_y  : f32,
    top_y   : f32,
) -> LayerRayOrigin {
    var result: LayerRayOrigin;

    // Camera inside the slab: start at camera position.
    if cam_pos.y >= base_y && cam_pos.y <= top_y {
        result.origin = cam_pos;
        result.valid  = true;
        return result;
    }

    // Horizontal ray: never enters a horizontal slab.
    if abs(ray_dir.y) < 1e-5 {
        result.origin = cam_pos;
        result.valid  = false;
        return result;
    }

    let t0 = (base_y - cam_pos.y) / ray_dir.y;
    let t1 = (top_y  - cam_pos.y) / ray_dir.y;
    let t_enter = min(t0, t1);
    let t_exit  = max(t0, t1);

    // Ray travels away from the slab.
    if t_exit <= 0.0 {
        result.origin = cam_pos;
        result.valid  = false;
        return result;
    }

    let t_clamped = max(t_enter, 0.0);
    result.origin = cam_pos + ray_dir * t_clamped;
    result.valid  = true;
    return result;
}

fn remap01(x: f32, lo: f32, hi: f32) -> f32 {
    return saturate1((x - lo) / max(hi - lo, 1e-4));
}

// Schneider 2015 erosion remap: shifts the density range so that detail noise
// carves into cloud edges while preserving the gradient slope at the surface.
fn erode_remap(base: f32, detail_threshold: f32) -> f32 {
    return saturate1((base - detail_threshold) / max(1.0 - detail_threshold, 1e-5));
}

// ---- Phase functions ----

fn hg_denom(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let d  = max(1.0 + g2 - 2.0 * g * cos_theta, 1e-3);
    return pow(d, 1.5);
}

fn hg_phase(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    return (1.0 - g2) / (12.5663706 * hg_denom(cos_theta, g));
}

fn cs_phase(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let num = 3.0 * (1.0 - g2) * (1.0 + cos_theta * cos_theta);
    let den = 2.0 * (2.0 + g2) * 12.5663706 * hg_denom(cos_theta, g);
    return num / max(den, 1e-6);
}

// Water droplet phase: forward scatter, backward lobe, broad diffuse, silver lining peak.
fn cloud_phase(cos_theta: f32, forward_g: f32, silver: f32) -> f32 {
    let forward  = cs_phase(cos_theta, clamp(forward_g, 0.55, 0.80));
    let backward = hg_phase(cos_theta, -0.3);
    let broad    = cs_phase(cos_theta, 0.10);
    let silver_peak = hg_phase(cos_theta, 0.97) * silver * 0.06;
    return mix(mix(forward, broad, 0.25), backward, 0.12) + silver_peak;
}

// Ice-crystal phase for cirrus: strong forward diffraction + 22° halo ring.
fn cirrus_phase(cos_theta: f32, sun_elevation: f32) -> f32 {
    let fwd   = cs_phase(cos_theta, 0.96) * 0.72;
    let broad = cs_phase(cos_theta, 0.40) * 0.20;
    let halo_elev_weight = saturate1(1.0 - abs(sun_elevation) * 1.8);
    let halo  = exp(-pow((cos_theta - 0.9272) / 0.040, 2.0)) * 0.08 * halo_elev_weight;
    return fwd + broad + halo;
}

// Physically accurate cloud vertical density profiles.
// All profiles parameterised on h ∈ [0,1] where 0=base, 1=top.
//
// Key physical principles:
//   - Cumulus base is SHARP (thermodynamic LCL boundary) — rapid onset
//   - Cumulus top has an EXPONENTIAL dome — not a flat cut-off
//   - Density peak is at ~25–35% of cloud height, not 50%
//   - Stratus: thin symmetric sheet with extremely sharp base
//   - CB: sharp base, tall tower, sparse glaciated anvil cap at top

fn density_height_profile(y: f32, base_y: f32, top_y: f32,
                           bottom_soft: f32, top_soft: f32) -> f32 {
    let h      = remap01(y, base_y, top_y);
    let bottom = smoothstep(0.0, bottom_soft, h);
    let top    = 1.0 - smoothstep(1.0 - top_soft, 1.0, h);
    return bottom * top;
}

fn stratus_profile(h: f32) -> f32 {
    // Thin symmetric sheet: extremely sharp base (stable thermodynamic
    // inversion lifts condensation abruptly), gradual fibrous top fade.
    // Peak at h=0.30, uniform density through the middle.
    let base = smoothstep(0.0, 0.04, h);
    let top  = 1.0 - smoothstep(0.58, 0.90, h);
    return base * top * 0.90;
}

fn stratocumulus_profile(h: f32) -> f32 {
    // Flat-based (sharp LCL boundary), turreted rounded tops.
    // Density concentrated in the lower third with a smooth exponential dome.
    let base      = smoothstep(0.0, 0.06, h);
    // Density peak at h=0.25, exponential decay toward top
    let peak      = exp(-max(h - 0.25, 0.0) * 5.0);
    // Smooth top boundary — the cells look rounded, not flat-topped
    let top_clamp = 1.0 - smoothstep(0.78, 1.0, h);
    return base * peak * top_clamp;
}

fn cumulus_profile(h: f32) -> f32 {
    // Preserve condensate through the rising turret. Shape-dependent coverage
    // below supplies rounded tops; an exponential height fade flattened them.
    let base = smoothstep(0.0, 0.055, h);
    let core = mix(0.78, 1.0, smoothstep(0.06, 0.28, h));
    return base * core * (1.0 - smoothstep(0.68, 1.0, h));
}

fn cumulonimbus_profile(h: f32) -> f32 {
    // Tall violent tower:
    //   - Extremely sharp base (powerful surface convergence / cold front lifting)
    //   - Near-uniform density through the tower — updrafts maintain a thick core
    //   - At h>0.70: tower density decreases as air overshoots and spreads
    //   - Glaciated anvil at top: secondary density pulse, reduced because ice
    //     crystals have lower mass density than liquid droplets below
    let base      = smoothstep(0.0, 0.03, h);      // sharp convective base
    // Main tower: uniform dense core with slight increase at h=0.40 (max updraft)
    let tower_peak = 1.0 + smoothstep(0.20, 0.42, h) * 0.15;
    let tower_top  = 1.0 - smoothstep(0.68, 0.84, h);
    let tower      = tower_peak * tower_top;
    // Glaciated anvil: ice crystal plume spreads above the overshooting top.
    // Second pulse: weaker and more wispy than the liquid tower below.
    let anvil_rise = smoothstep(0.80, 0.88, h);
    let anvil_fade = 1.0 - smoothstep(0.92, 1.0, h);
    let anvil      = anvil_rise * anvil_fade * 0.48;
    return base * (tower + anvil);
}

fn cloud_type_height_gradient(h: f32, cloud_type: f32) -> f32 {
    // t * 3.0: idx maxes at 2 (for t<1.0), so idx+1 is always in-bounds [0,3].
    // Profiles: 0=stratus, 1=stratocumulus, 2=cumulus, 3=cumulonimbus.
    let t   = saturate1(cloud_type) * 3.0;
    let idx = min(u32(floor(t)), 2u);
    let f   = t - f32(idx);
    let profiles = array<f32, 4>(
        stratus_profile(h),
        stratocumulus_profile(h),
        cumulus_profile(h),
        cumulonimbus_profile(h),
    );
    return mix(profiles[idx], profiles[idx + 1u], f);
}

// ============================================================
// MACRO WEATHER FIELD
// Large-scale (80–400km) smooth coverage variation.
// Uses domain-warped FBM to produce organic cloud streets,
// elongated clusters, and realistic clear-sky gaps. The
// domain warp is computed at a coarser scale so it is
// smooth and stable — no high-frequency jumping.
// ============================================================

// macro_coverage_bias takes world_xz ALREADY advected by the caller at the
// appropriate multiplier (matches the low-layer shape noise advection at 8x).
// The macro field itself does NOT apply additional time advection — this keeps
// macro coverage anchored to the same world-space frame as the shape noise.
// Multi-scale advection drift (macro field vs shape noise) was the primary cause
// of visible coverage flicker as the macro and shape fields drifted out of sync.
fn macro_coverage_bias(world_xz: vec2<f32>, wind: vec2<f32>, time: f32, wspd: f32) -> f32 {
    // Reuse the nonperiodic weather organization instead of eight additional
    // procedural-noise evaluations at every view and shadow density sample.
    return mix(0.35, 1.55, smoothstep(0.18, 0.82, weather_bank(world_xz)));
}

// ============================================================
// DENSITY SAMPLERS
// Accept pre-sampled CellWeather — call weather_at_world once
// before the march loop and pass it to each sample call.
// Slab bounds are enforced by the march loop (not checked here).
// ============================================================

fn sample_low_density(
    p                  : vec3<f32>,
    base_y             : f32,
    top_y              : f32,
    eff_y              : f32,
    dist_fade          : f32,
    dist_t             : f32,
    grazing_f          : f32,
    shadow_lod         : bool,
    sample_footprint_m : f32,
    weather            : CellWeather,
    // macro_bias sampled at the step position (p.xz + wind offset), passed in from
    // the march loop. Evaluated per-step for seamless continuity across cell boundaries.
    macro_bias_precomp : f32,
) -> f32 {
    return photographic_low_density(vec3<f32>(p.x, eff_y, p.z), base_y, top_y,
        sample_footprint_m, weather, macro_bias_precomp, shadow_lod);
}

fn sample_cirrus_density(
    p                  : vec3<f32>,
    base_y             : f32,
    top_y              : f32,
    eff_y              : f32,
    sample_footprint_m : f32,
    weather            : CellWeather,
) -> f32 {
    let cirrus_amt = weather.high_amount;
    if cirrus_amt <= 0.001 { return 0.0; }

    let wind = safe_normalize2(light.weather_globals[0].zw, vec2<f32>(0.96, -0.12));
    let side = vec2<f32>(-wind.y, wind.x);
    let time = light.weather_globals[0].x;
    let wspd = max(light.weather_globals[0].y, 0.0);

    let advected_xz = p.xz + wind * time * wspd * 12.0;

    // Compress the along-wind domain, so fibers stretch ALONG the wind.
    let along_wind  = dot(advected_xz, wind);
    let perp_wind   = dot(advected_xz, side);
    let fallstreak = sin(perp_wind * 0.004 + along_wind * 0.0008) * 26.0;
    let stretched_xz = (along_wind * 0.167 + fallstreak) * wind + perp_wind * side;

    let h_frac = remap01(eff_y, base_y, top_y);

    let lod_shape  = clamp(log2(max(sample_footprint_m * 0.00060 * SHAPE_TEX_SIZE,  1.0)), 0.0, 7.0);
    let lod_detail = clamp(log2(max(sample_footprint_m * 0.0018  * DETAIL_TEX_SIZE, 1.0)), 0.0, 6.0);

    // Large-scale fiber shape on the anisotropic domain.
    // Very compressed Y to keep the sheet extremely thin.
    let s1 = sample_shape_global(
        stretched_xz,
        h_frac * 60.0 + time * wspd * 0.025,
        0.00045,
        0.040,
        max(1.0, lod_shape)
    ).r;

    // Fine-scale fiber at a higher frequency, offset to break repetition.
    let s2 = sample_shape_global(
        stretched_xz + vec2<f32>(83.0, -47.0),
        h_frac * 90.0 + time * wspd * 0.032,
        0.0018,
        0.080,
        max(0.5, lod_shape)
    ).r;

    let fine_keep  = 1.0 - smoothstep(14.0, 36.0, sample_footprint_m);
    let detail_tex = sample_detail_global(stretched_xz, h_frac * 120.0, 0.0015, 0.13, lod_detail);
    let detail_fbm = detail_tex.r * 0.625 + detail_tex.g * 0.25 + detail_tex.b * 0.125;
    let curl_str   = detail_tex.a * 2.0 - 1.0;
    let s3         = saturate1(detail_fbm - curl_str * 0.15);

    // Very thin vertical profile for ice-crystal sheets (top_soft=0.05 vs 0.20 for cumulus).
    let height_shape = density_height_profile(eff_y, base_y, top_y, 0.08, 0.05);
    let density_ctl  = mix(0.45, 1.35, authored_cloud_density());
    let fiber        = s1 * 0.50 + s2 * 0.25 + mix(0.5, s3, fine_keep) * 0.25;
    let effective_cirrus_amt = cirrus_amt * height_shape;
    let thresh = mix(0.46, 0.24, cirrus_amt);

    return smoothstep(thresh, thresh + 0.16, fiber) * effective_cirrus_amt * density_ctl * 0.90;
}

fn sample_mid_density(
    p                  : vec3<f32>,
    base_y             : f32,
    top_y              : f32,
    eff_y              : f32,
    sample_footprint_m : f32,
    weather            : CellWeather,
) -> f32 {
    let wind = safe_normalize2(light.weather_globals[0].zw, vec2<f32>(0.9, -0.4));
    let side = vec2<f32>(-wind.y, wind.x);
    let time = light.weather_globals[0].x;
    let wspd = max(light.weather_globals[0].y, 0.0) * 0.7;

    let advected_xz = p.xz + wind * time * wspd * 9.5;

    let ac_bias = authored_altocumulus() * (0.45 + weather.convection * 0.55)
                + weather.convection * 0.20;
    let as_bias = authored_altostratus() * (0.55 + weather.humidity * 0.45)
                + (1.0 - weather.convection) * 0.10;
    let mid_bias_sum = max(ac_bias + as_bias, 1e-4);
    let ac_amt = weather.mid_amount * ac_bias / mid_bias_sum;
    let as_amt = weather.mid_amount * as_bias / mid_bias_sum;
    if ac_amt + as_amt <= 0.001 { return 0.0; }

    let mid_lod = clamp(
        log2(max(sample_footprint_m * 0.0007 * SHAPE_TEX_SIZE, 1.0)),
        0.0, 7.0
    );

    let billow = sample_shape_global(
        advected_xz,
        eff_y + time * wspd * 0.018,
        0.00070,
        0.0014,
        max(0.5, mid_lod)
    ).r;

    let cell = sample_shape_global(
        advected_xz + vec2<f32>(180.0, -90.0),
        eff_y + time * wspd * 0.024,
        0.0010,
        0.0022,
        max(0.5, mid_lod)
    ).g;

    let h = remap01(eff_y, base_y, top_y);

    let density_ctl = mix(0.35, 1.40, authored_cloud_density());
    let height_ac = smoothstep(0.0, 0.12, h) * (1.0 - smoothstep(0.72, 1.0, h));
    let effective_ac_amt = ac_amt * height_ac;
    let ac_base   = billow * 0.55 + cell * 0.45;
    let ac_thresh = mix(0.42, 0.24, ac_amt) + smoothstep(0.40, 1.0, h) * 0.14;
    var ac_body = smoothstep(ac_thresh, ac_thresh + 0.13, ac_base);

    let fine_keep = 1.0 - smoothstep(10.0, 28.0, sample_footprint_m);
    if ac_body > 0.001 && fine_keep > 0.01 {
        let detail_tex = sample_detail_global(
            advected_xz,
            eff_y,
            0.0028,
            0.0028,
            mid_lod
        );
        let detail_fbm = detail_tex.r * 0.625 + detail_tex.g * 0.25 + detail_tex.b * 0.125;
        ac_body = erode_remap(ac_body, detail_fbm * 0.26 * fine_keep);
    }
    // Erode shape before applying the amount. Eroding the small final density
    // deleted every delicate patch in a low-coverage evening sky.
    let ac_density = ac_body * effective_ac_amt * density_ctl;

    let sheet_a = sample_shape_global(
        advected_xz,
        eff_y,
        0.00035,
        0.0010,
        max(2.0, mid_lod)
    ).r;
    let sheet_b = sample_shape_global(
        advected_xz + vec2<f32>(600.0, -370.0),
        eff_y * 0.55,
        0.00020,
        0.00055,
        max(2.5, mid_lod + 0.5)
    ).r;
    let sheet = saturate1(sheet_a * 0.35 + sheet_b * 0.65);
    let height_as  = smoothstep(0.0, 0.08, h) * (1.0 - smoothstep(0.88, 1.0, h));
    let effective_as_amt = as_amt * height_as;
    let veil = smoothstep(0.12, 0.42, sheet);
    let as_density = veil * effective_as_amt * density_ctl * 1.05;

    return max(ac_density, as_density);
}

fn sample_mid_density_shadow(
    p: vec3<f32>,
    base_y: f32,
    top_y: f32,
    eff_y: f32,
    weather: CellWeather,
) -> f32 {
    return sample_mid_density(p, base_y, top_y, eff_y, 24.0, weather);
}

fn shadow_field_at_world(world_xz: vec2<f32>) -> vec4<f32> {
    let uv0 = weather_uv(world_xz, 0);
    let uv1 = weather_uv(world_xz, 1);
    let uv2 = weather_uv(world_xz, 2);
    let w0 = weather_level_weight(uv0, 0);
    let w1 = weather_level_weight(uv1, 1) * (1.0 - w0);
    let w2 = max(1.0 - w0 - w1, 0.0);
    return sample_weather_bilinear(cloud_shadow_tex, uv0, 0) * w0
        + sample_weather_bilinear(cloud_shadow_tex, uv1, 1) * w1
        + sample_weather_bilinear(cloud_shadow_tex, uv2, 2) * w2;
}

fn layer_wind_advection_offset(layer_id: i32) -> vec2<f32> {
    let wind = safe_normalize2(light.weather_globals[0].zw, vec2<f32>(0.9, -0.4));
    let time = light.weather_globals[0].x;
    let wind_spd = max(light.weather_globals[0].y, 0.0);
    // Must match actual density sampler multipliers including per-layer speed factors.
    // Low: wspd * 8.0, Mid: wspd * 0.7 * 9.5 = 6.65, High: wspd * 12.0
    let mult = select(select(8.0, 6.65, layer_id == 1), 12.0, layer_id == 2);
    return wind * time * wind_spd * mult;
}

fn macro_shadow_optical_depth(world_xz: vec2<f32>, layer_id: i32) -> f32 {
    let shadow = shadow_field_at_world(world_xz);
    return select(
        select(shadow.x + shadow.w * 0.12, shadow.y + shadow.w * 0.08, layer_id == 1),
        shadow.z + shadow.w * 0.04,
        layer_id == 2,
    );
}

struct ShadowTapContext {
    weather: CellWeather,
    aabb: CellAABB,
    valid: bool,
};

fn sample_shadow_tap_context(world_pos: vec3<f32>, layer_id: i32) -> ShadowTapContext {
    var ctx: ShadowTapContext;
    let weather = weather_at_world(world_pos.xz + layer_wind_advection_offset(layer_id));
    let aabb = cell_aabb_for_layer(weather, layer_id);
    ctx.weather = weather;
    ctx.aabb = aabb;
    ctx.valid = cell_aabb_valid(aabb) && world_pos.y >= aabb.y_min && world_pos.y <= aabb.y_max;
    return ctx;
}

fn sample_local_self_shadow(
    p: vec3<f32>,
    light_dir: vec3<f32>,
    layer_id: i32,
    sample_footprint_m: f32,
    layer_thickness_m: f32,
) -> f32 {
    // Integrate density * distance (meters), not average density. A thick
    // storm must block more light than a small puff with the same density.
    // Quadratic spacing resolves nearby lobes while reaching the far cloud edge.
    let bounds = cloud_layer_bounds();
    let y0 = select(select(bounds.low_base, bounds.mid_base, layer_id == 1), bounds.high_base, layer_id == 2);
    let y1 = select(select(bounds.low_top, bounds.mid_top, layer_id == 1), bounds.high_top, layer_id == 2);
    let exit_distance = max(slab_intersection(p, light_dir, y0, y1).y, 0.0);
    let reach = min(exit_distance, clamp(layer_thickness_m * 3.0, 60.0, 900.0));
    // Changing quadrature at a camera-distance threshold creates a visible
    // lighting seam across an otherwise continuous storm deck.
    let tap_count = select(3, 2, layer_id == LAYER_HIGH);
    let sigma = select(select(0.022, 0.016, layer_id == 1), 0.010, layer_id == 2);
    // A shared midpoint locks shadow quadrature to the condensation plane,
    // producing horizontal lighting contours even when opacity is smooth.
    // Vary the phase continuously in world space so moving the camera cannot
    // make the shadow pattern crawl, and retain the full interval weights.
    let shadow_phase = mix(0.22, 0.78, weather_bank_noise(p.xz / 137.0 + vec2<f32>(8.3, -2.1)));
    var optical_depth = 0.0;
    for (var i: i32 = 0; i < tap_count; i = i + 1) {
        let u0 = f32(i) / f32(tap_count);
        let u1 = f32(i + 1) / f32(tap_count);
        let segment_start = u0 * u0 * reach;
        let segment_end = u1 * u1 * reach;
        let midpoint = (segment_start + segment_end) * 0.5;
        let tap = sample_shadow_tap_context(p + light_dir * midpoint, layer_id);
        if !cell_aabb_valid(tap.aabb) { continue; }
        // Integrate the local slab overlap instead of switching an entire
        // light tap on/off at a layer boundary (visible as a hard shadow plane).
        let hit = slab_intersection(p, light_dir, tap.aabb.y_min, tap.aabb.y_max);
        let start = max(segment_start, hit.x);
        let end = min(segment_end, hit.y);
        let segment = max(end - start, 0.0);
        if segment <= 0.0 { continue; }
        let t = mix(start, end, shadow_phase);
        let q = p + light_dir * t;
        // Match filtering to the light segment without increasing tap count.
        let shadow_footprint = max(sample_footprint_m, segment * 0.5);

        // Branch explicitly: select() evaluates all three density functions.
        var d: f32;
        if layer_id == LAYER_HIGH {
            d = sample_cirrus_density(q, tap.aabb.y_min, tap.aabb.y_max, q.y,
                max(shadow_footprint, 24.0), tap.weather);
        } else if layer_id == LAYER_MID {
            d = sample_mid_density(q, tap.aabb.y_min, tap.aabb.y_max, q.y,
                max(shadow_footprint, 24.0), tap.weather);
        } else {
            // Reuse the continuous macro field at the actual light sample; a
            // fixed coverage bias casts shadows from clouds that do not exist.
            let offset = layer_wind_advection_offset(LAYER_LOW);
            let bias = macro_coverage_bias(q.xz + offset, vec2<f32>(0.0), 0.0, 0.0);
            d = sample_low_density(q, tap.aabb.y_min, tap.aabb.y_max, q.y,
                1.0, 0.0, 0.0, true, shadow_footprint, tap.weather, bias);
        }
        optical_depth += d * segment * sigma;
        // All segments are needed: higher scattering orders still carry
        // exp(-0.18 * depth) energy at depth=8. Early termination there made
        // entire planes of a storm jump between two illumination values.
    }
    return optical_depth;
}

// ============================================================
// MAIN RAY MARCH — FLAT SLAB MARCH WITH ADAPTIVE AABB CULLING
//
// Marches directly through the global layer slab (no DDA).
// At each step, weather is sampled at the exact step position
// and used to compute a per-step vertical AABB. Steps outside
// any cloud AABB are skipped cheaply. This produces seamless
// coverage — no cell-boundary artifacts anywhere.
//
// Empty-space skipping: per-step AABB cull + coverage early-out
// inside the density samplers (no explicit large-step mechanism
// needed since both are already O(1) cheap before the texture taps).
// ============================================================

// Layer ID constants for selecting the correct per-step AABB.
const LAYER_LOW  : i32 = 0;
const LAYER_MID  : i32 = 1;
const LAYER_HIGH : i32 = 2;

fn cell_aabb_for_layer(w: CellWeather, layer_id: i32) -> CellAABB {
    if layer_id == LAYER_MID {
        return cell_mid_aabb(w);
    } else if layer_id == LAYER_HIGH {
        return cell_high_aabb(w);
    }
    return cell_low_aabb(w);
}

// Density-only quadrature taps. Shading still runs once per segment; resolving
// the volume along a long grazing segment is cheaper than adding shadow rays.
fn segment_density(p: vec3<f32>, layer_id: i32, footprint: f32,
    wind_offset: vec2<f32>, weather: CellWeather) -> f32 {
    let bounds = cell_aabb_for_layer(weather, layer_id);
    if !cell_aabb_valid(bounds) || p.y < bounds.y_min || p.y > bounds.y_max {
        return 0.0;
    }
    if layer_id == LAYER_HIGH {
        return sample_cirrus_density(p, bounds.y_min, bounds.y_max, p.y, footprint, weather);
    }
    if layer_id == LAYER_MID {
        return sample_mid_density(p, bounds.y_min, bounds.y_max, p.y, footprint, weather);
    }
    var bias = 1.0;
    if weather.coverage > 0.02 || weather.storm > 0.04 {
        bias = macro_coverage_bias(p.xz + wind_offset, vec2<f32>(0.0), 0.0, 0.0);
    }
    return sample_low_density(p, bounds.y_min, bounds.y_max, p.y,
        1.0, 0.0, 0.0, false, footprint, weather, bias);
}

fn march_layer_core(
    ray_origin      : vec3<f32>,
    ray_dir         : vec3<f32>,
    cam_dist_offset : f32,
    sun_dir         : vec3<f32>,
    moon_dir     : vec3<f32>,
    frag_coord   : vec2<f32>,
    base_y       : f32,
    top_y        : f32,
    max_dist     : f32,
    extinction   : f32,
    ambient_gain : f32,
    forward_g    : f32,
    cirrus_layer : bool,
    mid_layer    : bool,
    cirrus_transmittance_atten : f32,
    cone_spread  : f32,
) -> CloudSample {
    // ---- Coarse early-out using global envelope ----
    // When the camera is inside the slab, slab_intersection returns (1e9, -1e9) for
    // near-horizontal rays. Bypass the global envelope check in that case.
    let env_slab  = slab_intersection(ray_origin, ray_dir, base_y, top_y);
    var env_enter = max(env_slab.x, 0.0);
    var env_exit  = min(env_slab.y, max_dist);
    
    let cam_inside_slab = ray_origin.y >= base_y && ray_origin.y <= top_y;
    if cam_inside_slab {
        env_enter = 0.0;
        env_exit = min(max(env_slab.y, 0.0), max_dist);
    } else {
        if env_exit <= env_enter || env_enter >= max_dist {
            return CloudSample(vec3<f32>(0.0), 1.0, vec4<f32>(0.0), 0.0);
        }
    }
    
    let ray_length_in_slab = max(env_exit - env_enter, 0.0);

    // ---- Layer ID for per-cell AABB dispatch ----
    var layer_id = LAYER_LOW;
    if mid_layer    { layer_id = LAYER_MID; }
    if cirrus_layer { layer_id = LAYER_HIGH; }

    if cirrus_layer && authored_cirrus() * authored_cloud_density() <= 0.001 {
        return CloudSample(vec3<f32>(0.0), 1.0, vec4<f32>(0.0), 0.0);
    }
    if mid_layer && max(authored_altocumulus(), authored_altostratus()) * authored_cloud_density() <= 0.001 {
        return CloudSample(vec3<f32>(0.0), 1.0, vec4<f32>(0.0), 0.0);
    }

    // ---- Pre-compute frame constants (shared across all cells) ----
    let grazing_f  = 1.0 - saturate1(abs(ray_dir.y) * 2.5);
    // Break coherent horizontal integration bands with a small, frame-stable
    // blue-noise phase. Stratified density taps remain inside their interval.
    // No animated noise or Bayer grid.
    let noise_size = vec2<i32>(textureDimensions(blue_noise_tex));
    let noise_pixel = vec2<i32>(frag_coord) % noise_size;
    let phase = textureLoad(blue_noise_tex, noise_pixel, 0).r;
    // A separate low-discrepancy phase for each segment prevents the same
    // quadrature lattice from drawing coherent stripes through distant decks.

    let wind     = safe_normalize2(light.weather_globals[0].zw, vec2<f32>(0.9, -0.4));
    let time     = light.weather_globals[0].x;
    let wind_spd = max(light.weather_globals[0].y, 0.0);
    // Match density sampler wind multipliers: low=8, mid=6.65 (0.7*9.5), high=12
    let wx_mult  = select(select(8.0, 6.65, mid_layer), 12.0, cirrus_layer);
    let wx_offset_march = wind * time * wind_spd * wx_mult;

    let daylight = saturate1(light.sun_color_daylight.w);
    let night    = 1.0 - daylight;
    let silver   = saturate1(light.weather_globals[2].z);

    let cos_theta_sun  = dot(ray_dir, sun_dir);
    let cos_theta_moon = dot(ray_dir, moon_dir);

    let phase_sun  = select(
        cloud_phase(cos_theta_sun, forward_g, silver),
        cirrus_phase(cos_theta_sun, sun_dir.y),
        cirrus_layer
    );
    let phase_moon = cs_phase(cos_theta_moon, 0.45);

    // Interpolate bottom/top sunlight: raised turrets can remain peach-lit
    // after the low deck has entered the planet's shadow.
    let sun_trans_base = photographic_cloud_sun(sun_dir.y, base_y);
    let sun_trans_top = photographic_cloud_sun(sun_dir.y, top_y);
    let twilight_light = photographic_twilight(sun_dir.y);
    // The terrain sun is already dimmed by global cloud cover and daylight.
    // A cloud samples its own solar path and occluders; applying that dimmed
    // terrain value plus another daylight factor erased sunset illumination.
    let cloud_solar_irradiance = 5.5;
    let moon_light = light.moon_color_twilight.rgb * light.moon_dir_intensity.w * night;

    // Horizon elevation fade is constant per ray (only depends on ray_dir.y).
    // Pre-compute once here to avoid the smoothstep per step.
    let horizon_elev_fade_precomp = smoothstep(0.015, 0.070, abs(ray_dir.y));

    // Two approximate higher scattering orders with decreasing energy and
    // reduced extinction preserve luminous cores behind a strongly lit edge.
    let ms_phase_iso  = 0.07957747; // 1/(4*pi) — pure isotropic scatter
    let ms_scatter = select(mix(0.55, 0.42, twilight_light), 0.30, cirrus_layer);
    let shadow_dir = select(sun_dir, moon_dir, night > 0.7);
    let ground_base  = light.sun_color_daylight.rgb * sun_trans_base
                     * daylight * max(sun_dir.y, 0.12) * 0.06
                     * vec3<f32>(0.15, 0.18, 0.12) * 1.20;

    // max_dist is measured from the slab entry, but density LOD and distance
    // fading use distances from the camera. Mixing those spaces made a raised
    // layer fade out sooner as the view approached the horizon.
    let camera_max_dist = cam_dist_offset + max_dist;
    // Clouds fade smoothly approaching the max draw distance.
    let fade_dist = max(
        max(light.fog_params.y * 1.2, cam.world_params.w * 1.0),
        camera_max_dist * 0.90
    );

    // ---- Hoist per-ray constants outside the DDA loop ----
    // atmosphere_color() routes through the runtime sky-view LUT which involves
    // texture sampling — calling it per step is wasteful. Sky radiance is constant
    // per ray direction; only cloud depth (h_frac) modulates how it is blended.
    // Pre-compute the three hemisphere directions and their sky radiance here.
    // The per-step code then just does a weighted blend of these three constants.
    let sky_zenith_precomp  = atmosphere_color(vec3<f32>(0.0, 1.0, 0.0));
    // Illumination is independent of the camera direction. Opposed sky samples
    // keep a cloud's shadow color stable while the player looks around it.
    let sun_az = safe_normalize2(sun_dir.xz, vec2<f32>(1.0, 0.0));
    let sky_upper_precomp = (atmosphere_color(vec3<f32>(sun_az.x * 0.8, 0.6, sun_az.y * 0.8))
        + atmosphere_color(vec3<f32>(-sun_az.x * 0.8, 0.6, -sun_az.y * 0.8))) * 0.5;
    let sky_horizon_precomp = (atmosphere_color(vec3<f32>(sun_az.x, 0.0, sun_az.y))
        + atmosphere_color(vec3<f32>(-sun_az.x, 0.0, -sun_az.y))) * 0.5;

    // ---- Accumulated march state ----
    var trans          = 1.0;
    var radiance       = vec3<f32>(0.0);
    var distance_moment = 0.0;
    var total_steps    = 0;             // steps taken (for debug step-count view)
    var debug_density_max = 0.0;
    var debug_any_aabb    = 0.0;
    var debug_shadow_max  = 0.0;

    // ---- Step budget: sufficient for the shorter march distances.
    let global_step_budget = select(select(192, 72, mid_layer), 32, cirrus_layer);

    // Step size: smaller for the voxel-world scale (clouds are 30-350m thick).
    let min_dt  = ray_length_in_slab / max(f32(global_step_budget), 1.0);
    // Resolve thin cloud silhouettes with several samples through their depth.
    // Sample spacing must not change when the camera rotates.
    let layer_dt_max = select(select(8.0, 12.0, mid_layer), 14.0, cirrus_layer);
    let layer_dt_min = select(select(3.5, 5.0, mid_layer), 6.0, cirrus_layer);
    let base_dt = mix(layer_dt_min, layer_dt_max, grazing_f);
    let dt      = max(base_dt, min_dt);

    for (var i: i32 = 0; i < global_step_budget; i = i + 1) {
        if trans <= 0.005 { break; }

        let segment_start = env_enter + f32(i) * dt;
        if segment_start >= env_exit { break; }
        let segment_length = min(dt, env_exit - segment_start);
        let jitter = 0.05 + fract(phase + f32(i) * 0.618033989) * 0.90;
        var t_cur = segment_start + segment_length * jitter;
        var p = ray_origin + ray_dir * t_cur;

        // ---- PER-STEP: CONTINUOUS WEATHER + AABB ----
        // Weather sampled at the exact step position — no cell grid, no seams.
        let step_weather = weather_at_world(p.xz + wx_offset_march);
        let step_aabb    = cell_aabb_for_layer(step_weather, layer_id);

        if !cell_aabb_valid(step_aabb) {
            total_steps += 1;
            continue;
        }
        // Integrate the intersection of this march segment and the local
        // vertical volume. A point-only cull skipped entire thin layers when
        // sample spacing exceeded their depth, making clouds switch off.
        var integration_length = segment_length;
        if abs(ray_dir.y) > 1e-5 {
            let local_hit = slab_intersection(ray_origin, ray_dir, step_aabb.y_min, step_aabb.y_max);
            let local_start = max(segment_start, local_hit.x);
            let local_end = min(segment_start + segment_length, local_hit.y);
            integration_length = max(local_end - local_start, 0.0);
            if integration_length <= 0.0 { total_steps += 1; continue; }
            t_cur = local_start + integration_length * jitter;
            p = ray_origin + ray_dir * t_cur;
        } else if p.y < step_aabb.y_min || p.y > step_aabb.y_max {
            total_steps += 1;
            continue;
        }

        let step_base_y = step_aabb.y_min;
        let step_top_y  = step_aabb.y_max;

        let t_from_cam         = cam_dist_offset + t_cur;
        let cone_footprint_m   = max(t_from_cam * cone_spread, 0.0);
        // A long ray segment is not a wide pixel. Using its full length for
        // every texture axis erased all visible detail at shallow elevations.
        let sample_footprint_m = max(max(min(integration_length * 0.35, 3.0), cone_footprint_m), 1.0);
        let dist_t = saturate1(t_from_cam / max(camera_max_dist, 1.0));

        debug_any_aabb = 1.0;

        let abs_dist        = t_from_cam / fade_dist;
        let dist_fade_base  = 1.0 - smoothstep(0.55, 1.0, abs_dist);
        let elev_blend_dist = smoothstep(0.40, 0.80, abs_dist);
        let dist_fade       = dist_fade_base * mix(1.0, horizon_elev_fade_precomp, elev_blend_dist);

        // Stratify the full interval rather than moving a fixed pair of Gauss
        // taps together. The latter leaves correlated gaps which show up as
        // horizontal shelves through thin, distant storm condensation floors.
        let quadrature = smoothstep(8.0, 22.0, integration_length);
        var raw_density = 0.0;
        if quadrature < 1.0 {
            raw_density = segment_density(p, layer_id, sample_footprint_m, wx_offset_march, step_weather);
        }
        if quadrature > 0.0 {
            let start_p=p-ray_dir*integration_length*jitter;
            let stratum=fract(phase + f32(i)*0.754877666);
            let q0=start_p+ray_dir*integration_length*(stratum*0.5);
            let q1=start_p+ray_dir*integration_length*(0.5+fract(stratum+0.5)*0.5);
            let integrated=(segment_density(q0,layer_id,sample_footprint_m,wx_offset_march,
                    weather_at_world(q0.xz+wx_offset_march))
                +segment_density(q1,layer_id,sample_footprint_m,wx_offset_march,
                    weather_at_world(q1.xz+wx_offset_march)))*0.5;
            raw_density=mix(raw_density,integrated,quadrature);
        }
        let sample_density = raw_density * dist_fade;
        debug_density_max = max(debug_density_max, sample_density);

        let step_alpha_cheap = 1.0 - exp(-sample_density * extinction * integration_length);
        let pre_step_trans   = trans;

        if sample_density > 0.0001 {
            trans          *= 1.0 - step_alpha_cheap;
            distance_moment += pre_step_trans * step_alpha_cheap * t_from_cam;
        }

        total_steps += 1;

        if sample_density > 0.0005 && step_alpha_cheap > 0.0002 {
            let layer_thickness_m = max(step_top_y - step_base_y, 1.0);
            let layer_mid_y = (step_base_y + step_top_y) * 0.5;
            let project_t = max((layer_mid_y - p.y) / max(shadow_dir.y, 0.08), 0.0);
            let macro_sample_xz = p.xz + shadow_dir.xz * min(project_t, 2000.0);
            let macro_blend = max(
                smoothstep(26.0, 120.0, sample_footprint_m),
                smoothstep(0.25, 0.80, dist_t),
            );
            var macro_shadow_od = 0.0;
            if macro_blend > 0.0 {
                macro_shadow_od = macro_shadow_optical_depth(macro_sample_xz, layer_id);
            }
            var local_shadow_od = 0.0;
            // At the fully coarse LOD this term has zero contribution. Preserve
            // continuous blending while avoiding four discarded density taps.
            if macro_blend < 1.0 {
                local_shadow_od = sample_local_self_shadow(
                    p, shadow_dir, layer_id, sample_footprint_m, layer_thickness_m,
                );
            }
            let local_scale = select(select(0.95, 0.80, mid_layer), 0.58, cirrus_layer);
            let macro_scale = select(select(0.24, 0.18, mid_layer), 0.12, cirrus_layer);
            // The coarse weather shadow is a distant fallback, not a second
            // occluder laid over the same nearby cloud.
            let shadow_d = mix(local_shadow_od * local_scale,
                macro_shadow_od * macro_scale, macro_blend);
            debug_shadow_max = max(debug_shadow_max, shadow_d);

            let h_frac     = saturate1(remap01(p.y, step_base_y, step_top_y));
            let sun_spectral_trans = mix(sun_trans_base, sun_trans_top,
                saturate1((p.y - base_y) / max(top_y - base_y, 1.0)));
            let cell_thick = max(step_top_y - step_base_y, 1.0);
            // Optical depth from sun entry at top to current altitude.
            // Decoupled from march extinction so height shading stays consistent.
            let od_top_coeff    = select(select(0.030, 0.020, mid_layer), 0.010, cirrus_layer);
            let top_atten_coeff = od_top_coeff * cell_thick * 0.006;
            let sun_from_top    = exp(-max(1.0 - h_frac, 0.0) * top_atten_coeff);
            let base_dark       = mix(0.32, 1.0, smoothstep(0.0, 0.35, h_frac))
                                * select(1.0, 0.85, !cirrus_layer);
            // At low solar elevation light enters from the side, so the old
            // top-down gradient incorrectly darkened even exposed sunset rims.
            let vertical_gradient = (sun_from_top * 0.55 + 0.45) * base_dark
                * (1.0 - smoothstep(0.88, 1.0, h_frac) * 0.08);
            let height_gradient = mix(vertical_gradient, 1.0, twilight_light * 0.75);

            // Local lighting must not depend on view-step length/history.
            // Ambient light enters through nearby cloud surfaces. Scaling its
            // occlusion by the entire kilometer-deep storm column crushed the
            // visible underside to black, leaving aerial haze to fill it in.
            let interior_od = sample_density * extinction * min(cell_thick * 0.5, 220.0);
            // Diffuse sky reaches the visible boundary from a hemisphere of
            // directions, including nearby sides. A full vertical-column AO
            // crushed that cool fill and left every sunset shadow brown.
            let ambient_path = mix(90.0, 220.0, step_weather.storm);
            let ambient_od = sample_density * extinction * min(cell_thick * 0.5, ambient_path);
            let depth_ao = exp(-ambient_od * select(0.35, 0.14, cirrus_layer));
            let light_trans = exp(-shadow_d);
            let moon_trans  = light_trans;
            let powder = 1.0 - exp(-interior_od * 2.0);
            let powder_gain = 1.0 + powder * (1.0 - max(cos_theta_sun, 0.0)) * 0.35;

            let light_trans_attenuated = light_trans * cirrus_transmittance_atten;
            // Golden light is concentrated on exposed faces and thin rims;
            // increasing every scattering order would wash out cool interiors.
            // Preserve a cool cloud core while letting low-angle, atmosphere-
            // filtered sunlight paint brighter warm edges and thin crowns.
            let sunset_key_gain = 1.0 + twilight_light * 0.95;
            let sun_lit = light.sun_color_daylight.rgb
                        * sun_spectral_trans * cloud_solar_irradiance
                        * light_trans_attenuated * phase_sun * height_gradient * sunset_key_gain;
            let moon_lit = moon_light * moon_trans * phase_moon * 0.40 * height_gradient;

            // Hillaire energy-preserving MS: second path sees reduced extinction (sqrt approx).
            let ms_ext_factor = exp(-shadow_d * 0.45) * cirrus_transmittance_atten;
            let ms_sun_base   = light.sun_color_daylight.rgb * sun_spectral_trans
                              * cloud_solar_irradiance * ms_phase_iso * ms_ext_factor * ms_scatter
                              * height_gradient;
            // Deep interior clouds whiten due to higher-order scattering cascade.
            let ms_sun = ms_sun_base + light.sun_color_daylight.rgb * sun_spectral_trans
                * cloud_solar_irradiance * ms_phase_iso
                * ms_scatter * ms_scatter * exp(-shadow_d * 0.18)
                * cirrus_transmittance_atten * height_gradient;
            let ms_moon  = moon_light * pow(max(moon_trans, 0.001), 0.5)
                         * ms_scatter * 0.30 * night * height_gradient;

            // Pre-computed hemisphere sky samples (hoisted outside march loop).
            let sky_zenith  = sky_zenith_precomp;
            let sky_upper   = sky_upper_precomp;
            let sky_horizon = sky_horizon_precomp;
            let w_zenith  = saturate1(h_frac * 1.4);
            let w_upper   = 1.0 - abs(h_frac - 0.55) * 1.6;
            let w_horizon = saturate1(1.0 - h_frac * 2.0);
            let w_sum     = max(w_zenith + max(w_upper, 0.0) + w_horizon, 1e-4);
            let sky_hemi  = (sky_zenith * w_zenith
                          + sky_upper  * max(w_upper, 0.0)
                          + sky_horizon * w_horizon) / w_sum;
            // Diffuse skylight retains the cool upper hemisphere even when
            // the narrow sunset horizon is strongly warm.
            let diffuse_sky = mix(sky_hemi, sky_zenith, 0.45);
            let sky_luma = dot(diffuse_sky, vec3<f32>(0.2126, 0.7152, 0.0722));
            // Cool hemisphere light stays in shadowed pockets while exposed
            // material receives the altitude-dependent warm solar spectrum.
            let shadow_sky = cloud_shadow_chroma(sun_dir.y, h_frac) * sky_luma;
            let sky_above = mix(diffuse_sky, shadow_sky, twilight_light * 0.78)
                * (0.16 + ambient_gain * 0.60);

            let ground_dist   = saturate1(1.0 - h_frac * 1.4);
            let ground_bounce = ground_base * ground_dist * 1.8;
            let ambient       = (sky_above + ground_bounce
                               + vec3<f32>(0.008, 0.012, 0.028) * night) * depth_ao;

            let interior_depth = 1.0 - depth_ao;
            let tint_strength  = mix(0.025, 0.010, saturate1(interior_od * 0.25))
                               * select(1.0, 0.0, cirrus_layer);
            let blue_tint         = vec3<f32>(0.85, 0.91, 1.0) * tint_strength * interior_depth;
            let ambient_with_tint = ambient + blue_tint;

            let edge_density = smoothstep(0.0, 0.030, sample_density)
                             * (1.0 - smoothstep(0.030, 0.14, sample_density));
            let silver_angle        = pow(max(cos_theta_sun * 0.5 + 0.5, 0.0), 2.5);
            let silver_density_gate = 1.0 / (1.0 + sample_density * 4.0);
            let sun_elev_silver     = mix(1.0, 0.55, saturate1(sun_dir.y * 2.0));
            let silver_light = light.sun_color_daylight.rgb * sun_spectral_trans
                             * light_trans
                             * edge_density * silver * silver_angle * cloud_solar_irradiance * 0.28
                             * silver_density_gate * sun_elev_silver * sunset_key_gain;

            // Direct light is attenuated once. The old beer-powder mix applied
            // Beer extinction twice, crushing the sculpted middle tones.
            let scatter = (sun_lit + moon_lit) * powder_gain
                + ambient_with_tint * mix(1.0, 0.65, step_weather.storm)
                + ms_sun + ms_moon + silver_light;

            radiance += pre_step_trans * scatter * step_alpha_cheap;
        }
    }

    return CloudSample(
        radiance,
        trans,
        vec4<f32>(
            debug_density_max,
            debug_any_aabb,
            f32(total_steps) / max(f32(global_step_budget), 1.0),
            debug_shadow_max,
        ),
        distance_moment / max(1.0 - trans, 1e-5),
    );
}

// ---- Per-layer march wrappers (signatures unchanged) ----

fn march_low_layer(
    ray_origin: vec3<f32>,
    ray_dir   : vec3<f32>,
    cam_dist_offset: f32,
    sun_dir   : vec3<f32>,
    moon_dir  : vec3<f32>,
    frag_coord: vec2<f32>,
    base_y    : f32,
    top_y     : f32,
    max_dist  : f32,
    cirrus_transmittance_atten: f32,
    cone_spread: f32,
) -> CloudSample {
    // extinction=0.022: calibrated for ~12-22m step sizes. density=0.5, dt=15m →
    // per-step alpha=0.154. Over 15 steps (225m cloud) → ~90% opaque. Realistic.
    return march_layer_core(
        ray_origin, ray_dir, cam_dist_offset, sun_dir, moon_dir, frag_coord,
        base_y, top_y, max_dist, 0.022, 0.72, 0.74,
        false, false, cirrus_transmittance_atten, cone_spread
    );
}

fn march_mid_layer(
    ray_origin: vec3<f32>,
    ray_dir   : vec3<f32>,
    cam_dist_offset: f32,
    sun_dir   : vec3<f32>,
    moon_dir  : vec3<f32>,
    frag_coord: vec2<f32>,
    base_y    : f32,
    top_y     : f32,
    max_dist  : f32,
    cirrus_transmittance_atten: f32,
    cone_spread: f32,
) -> CloudSample {
    // extinction=0.016: mid clouds are thinner/less dense than low cumulus.
    return march_layer_core(
        ray_origin, ray_dir, cam_dist_offset, sun_dir, moon_dir, frag_coord,
        base_y, top_y, max_dist, 0.016, 0.75, 0.70,
        false, true, cirrus_transmittance_atten, cone_spread
    );
}

fn march_high_layer(
    ray_origin: vec3<f32>,
    ray_dir   : vec3<f32>,
    cam_dist_offset: f32,
    sun_dir   : vec3<f32>,
    moon_dir  : vec3<f32>,
    frag_coord: vec2<f32>,
    base_y    : f32,
    top_y     : f32,
    max_dist  : f32,
    cone_spread: f32,
) -> CloudSample {
    // extinction=0.010: cirrus ice crystals have lower volumetric density than water droplets.
    return march_layer_core(
        ray_origin, ray_dir, cam_dist_offset, sun_dir, moon_dir, frag_coord,
        base_y, top_y, max_dist, 0.010, 0.75, 0.86,
        true, false, 1.0, cone_spread
    );
}

// Approximate attenuation through the cirrus layer for rays heading upward.
fn approx_cirrus_transmittance(dir: vec3<f32>) -> f32 {
    if authored_cirrus() <= 0.010 { return 1.0; }
    if dir.y < 0.04   { return 1.0; }

    let b          = cloud_layer_bounds();
    let mid_height = (b.high_base + b.high_top) * 0.5;
    let t_to_mid   = max((mid_height - cam.camera_pos.y) / dir.y, 0.1);
    let p_mid      = cam.camera_pos.xyz + dir * t_to_mid;

    let wind = safe_normalize2(light.weather_globals[0].zw, vec2<f32>(0.9, -0.4));
    let time = light.weather_globals[0].x;
    let wspd = max(light.weather_globals[0].y, 0.0);
    let wx   = weather_at_world(p_mid.xz + wind * time * wspd * 12.0);

    let local_aabb = cell_aabb_for_layer(wx, LAYER_HIGH);
    let d_cirrus = sample_cirrus_density(p_mid, local_aabb.y_min, local_aabb.y_max, p_mid.y, 16.0, wx);
    return exp(-d_cirrus * 0.045 * 20.0);
}

fn cloud_with_aerial_perspective(cloud: CloudSample, dir: vec3<f32>) -> CloudSample {
    if cloud.transmittance >= 0.9999 { return cloud; }
    var result = cloud;
    let ap = aerial_perspective(dir, cloud.mean_distance, cam.world_params.x, atmosphere_color(dir));
    result.radiance = cloud.radiance * ap.a + ap.rgb * (1.0 - cloud.transmittance);
    return result;
}

// ============================================================
// FRAGMENT SHADER
//
// Marches all three layers via DDA cell traversal,
// composites front-to-back, applies aerial perspective.
// ============================================================

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let dir         = sky_dir(in.ndc);
    let cone_spread = ray_cone_spread(dir);
    let debug_mode  = i32(render_debug_view());

    let scale    = cam.world_params.x;
    let sun_dir  = normalize(light.sun_dir_intensity.xyz);
    let moon_dir = normalize(light.moon_dir_intensity.xyz);

    let bounds = cloud_layer_bounds();

    if debug_mode == i32(DEBUG_VIEW_ATMOSPHERE_TRANSMITTANCE)
        || debug_mode == i32(DEBUG_VIEW_ATMOSPHERE_SKY_VIEW)
        || debug_mode == i32(DEBUG_VIEW_ATMOSPHERE_MULTISCATTER)
        || debug_mode == i32(DEBUG_VIEW_WEATHER_CLIP_NEAR)
        || debug_mode == i32(DEBUG_VIEW_WEATHER_CLIP_MID)
        || debug_mode == i32(DEBUG_VIEW_WEATHER_CLIP_FAR)
        || debug_mode == i32(DEBUG_VIEW_WEATHER_COMPOSITE)
        || debug_mode == i32(DEBUG_VIEW_WEATHER_SHADOW)
    {
        return vec4<f32>(0.0);
    }

    // Clouds occupy independent atmospheric altitudes above the voxel terrain.
    let unclamped_max_dist = 16000.0;
    let scene_dist = scene_depth_distance(in.ndc, dir);
    let terrain_fade = terrain_streaming_fade(scene_dist * length(dir.xz));
    // Far terrain is blended into the background only at the outer horizon boundary.
    // Do not march clouds behind solid mountains until terrain_fade is substantial.
    let max_dist = select(min(unclamped_max_dist, max(scene_dist - scale * 2.0, 0.0)),
        unclamped_max_dist, terrain_fade > 0.25);
    let pixel_jitter = in.pos.xy;

    if max_dist <= 0.0 {
        return vec4<f32>(0.0);
    }

    // ---- Compute per-layer world-space slab-entry ray origins ----
    // Each layer now has its own true world-space ray origin: the point where
    // the ray enters that layer's slab. When the camera is inside a layer,
    // the origin is the camera itself. This makes clouds real volumetric
    // objects rather than a screen-space billboard illusion.
    let low_ro  = compute_layer_ray_origin(cam.camera_pos.xyz, dir, bounds.low_base,  bounds.low_top);
    let mid_ro  = compute_layer_ray_origin(cam.camera_pos.xyz, dir, bounds.mid_base,  bounds.mid_top);
    let high_ro = compute_layer_ray_origin(cam.camera_pos.xyz, dir, bounds.high_base, bounds.high_top);

    // Distance from camera to each slab-entry point (0 when camera is inside the layer).
    let low_offset  = length(low_ro.origin  - cam.camera_pos.xyz);
    let mid_offset  = length(mid_ro.origin  - cam.camera_pos.xyz);
    let high_offset = length(high_ro.origin - cam.camera_pos.xyz);
    var cirrus_trans = 1.0;
    if low_ro.valid || mid_ro.valid {
        cirrus_trans = approx_cirrus_transmittance(sun_dir);
    }

    // ---- March each layer via DDA ----
    // max_dist is a budget from the camera. Subtract the camera-to-slab-entry
    // offset so the march doesn't extend beyond the visibility horizon.
    var low: CloudSample;
    if low_ro.valid {
        let low_max_dist = max(max_dist - low_offset, 0.0);
        low = march_low_layer(
            low_ro.origin, dir, low_offset, sun_dir, moon_dir, pixel_jitter,
            bounds.low_base, bounds.low_top, low_max_dist,
            cirrus_trans, cone_spread,
        );
    } else {
        low = CloudSample(vec3<f32>(0.0), 1.0, vec4<f32>(0.0), 0.0);
    }

    var mid: CloudSample;
    if mid_ro.valid {
        let mid_max_dist = max(max_dist - mid_offset, 0.0);
        mid = march_mid_layer(
            mid_ro.origin, dir, mid_offset, sun_dir, moon_dir, pixel_jitter,
            bounds.mid_base, bounds.mid_top, mid_max_dist,
            cirrus_trans, cone_spread,
        );
    } else {
        mid = CloudSample(vec3<f32>(0.0), 1.0, vec4<f32>(0.0), 0.0);
    }

    var high: CloudSample;
    if high_ro.valid {
        let high_max_dist = max(max_dist - high_offset, 0.0);
        high = march_high_layer(
            high_ro.origin, dir, high_offset, sun_dir, moon_dir, pixel_jitter,
            bounds.high_base, bounds.high_top, high_max_dist,
            cone_spread,
        );
    } else {
        high = CloudSample(vec3<f32>(0.0), 1.0, vec4<f32>(0.0), 0.0);
    }

    // ---- Depth Sort & Composite ----
    let rain = march_rain_volume(dir, max_dist, cone_spread, in.pos.xy);
    var layers: array<CloudSample, 4>;
    layers[0] = cloud_with_aerial_perspective(low, dir);
    layers[1] = cloud_with_aerial_perspective(mid, dir);
    layers[2] = cloud_with_aerial_perspective(high, dir);
    layers[3] = cloud_with_aerial_perspective(rain, dir);
    
    var dists: array<f32, 4>;
    dists[0] = select(1e9, low.mean_distance, low.transmittance < 0.9999);
    dists[1] = select(1e9, mid.mean_distance, mid.transmittance < 0.9999);
    dists[2] = select(1e9, high.mean_distance, high.transmittance < 0.9999);
    dists[3] = select(1e9, rain.mean_distance, rain.transmittance < 0.9999);

    // Keep foreground weather fully visible; release background-layer coverage
    // continuously with the same world-space fade used for the terrain colour.
    for (var i = 0; i < 4; i += 1) {
        if dists[i] > scene_dist {
            layers[i].radiance *= terrain_fade;
            layers[i].transmittance = mix(1.0, layers[i].transmittance, terrain_fade);
        }
    }

    for (var i = 0; i < 3; i += 1) {
        for (var j = 0; j < 3 - i; j += 1) {
            if dists[j] > dists[j + 1] {
                let td = dists[j]; dists[j] = dists[j + 1]; dists[j + 1] = td;
                let tl = layers[j]; layers[j] = layers[j + 1]; layers[j + 1] = tl;
            }
        }
    }

    var combined_rgb = vec3<f32>(0.0);
    var combined_trans = 1.0;
    for (var i = 0; i < 4; i += 1) {
        combined_rgb += combined_trans * layers[i].radiance;
        combined_trans *= layers[i].transmittance;
    }
    var combined_alpha = clamp(1.0 - combined_trans, 0.0, 0.998);

    if debug_mode == i32(DEBUG_VIEW_CLOUD_ALPHA) {
        return vec4<f32>(vec3<f32>(combined_alpha), 1.0);
    }

    if debug_mode == i32(DEBUG_VIEW_CLOUD_LAYER_DENSITY) {
        return vec4<f32>(
            min(
                vec3<f32>(
                    low.debug_metrics.x,
                    mid.debug_metrics.x,
                    high.debug_metrics.x
                ) * 1.8,
                vec3<f32>(1.0)
            ),
            1.0,
        );
    }

    if debug_mode == i32(DEBUG_VIEW_CLOUD_LAYER_AABB) {
        return vec4<f32>(
            vec3<f32>(
                low.debug_metrics.y,
                mid.debug_metrics.y,
                high.debug_metrics.y
            ),
            1.0,
        );
    }

    if debug_mode == i32(DEBUG_VIEW_CLOUD_STEP_COUNT) {
        return vec4<f32>(
            vec3<f32>(
                low.debug_metrics.z,
                mid.debug_metrics.z,
                high.debug_metrics.z
            ),
            1.0,
        );
    }

    if debug_mode == i32(DEBUG_VIEW_CLOUD_SHADOW_DENSITY) {
        return vec4<f32>(
            min(
                vec3<f32>(
                    low.debug_metrics.w,
                    mid.debug_metrics.w,
                    high.debug_metrics.w
                ) * 0.12,
                vec3<f32>(1.0)
            ),
            1.0,
        );
    }

    if debug_mode == i32(DEBUG_VIEW_CLOUD_WEATHER_DECODE) {
        var debug_dist = 1e9;
        var debug_pos = vec3<f32>(0.0);
        if low_ro.valid && low_offset < debug_dist {
            debug_dist = low_offset;
            debug_pos = low_ro.origin;
        }
        if mid_ro.valid && mid_offset < debug_dist {
            debug_dist = mid_offset;
            debug_pos = mid_ro.origin;
        }
        if high_ro.valid && high_offset < debug_dist {
            debug_dist = high_offset;
            debug_pos = high_ro.origin;
        }
        if debug_dist < 1e8 {
            let debug_weather = weather_at_world(debug_pos.xz);
            return vec4<f32>(
                vec3<f32>(debug_weather.coverage, debug_weather.humidity, debug_weather.storm),
                1.0,
            );
        }
        return vec4<f32>(0.0);
    }

    if combined_alpha <= 0.001 {
        return vec4<f32>(0.0);
    }

    // The volume march also handles a camera inside the cloud. Adding a white
    // overlay here made storm interiors glow independently of the sky lighting.
    return vec4<f32>(max(combined_rgb, vec3<f32>(0.0)), combined_alpha);
}
