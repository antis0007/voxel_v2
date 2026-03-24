// ============================================================
// cloud_shader.wgsl — Volumetric cloud renderer
//
// Techniques: Schneider/Hillaire/Häggström papers
//   - Slab-intersection bounded raymarching (no wasted samples)
//   - Proper step count derived from actual slab path length
//   - Blue-noise temporal jitter with per-frame golden-ratio rotation
//   - Curl-noise distorted detail erosion for realistic cloud edges
//   - World-space weather cell system: large bilinearly-blended cells,
//     per-cell transformed shape/detail domains
//   - Weather cached once per layer (not resampled every step)
//   - Distance-based detail LOD (full detail near, shape-only far)
//   - Cheap 3-step exponential shadow density
//   - Aerial perspective applied once after compositing all layers
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
};

// ============================================================
// WORLD-SPACE CLOUD FIELD
// Density is a pure function of world position + time.
// No camera-relative coordinates. No global periodic tiling.
// ============================================================

const SHAPE_TEX_SIZE  : f32 = 64.0;
const DETAIL_TEX_SIZE : f32 = 32.0;

// Cell sizes scaled down ~5x from the original planet-scale values.
// Weather: 4800 m, Shape: 2400 m, Detail: 1200 m.
// This keeps cloud variety at playable distances while keeping
// the DDA grid fine enough to not alias near cloud boundaries.
const CLOUD_WEATHER_CELL_M : f32 = 2400.0;
const CLOUD_SHAPE_CELL_M   : f32 = 1200.0;
const CLOUD_DETAIL_CELL_M  : f32 = 600.0;

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

fn near_field_cloud_cover() -> f32 {
    return clamp(light.weather_clip_summaries[0].y, 0.0, 1.0);
}

fn layer_activity(cirrus_layer: bool, mid_layer: bool) -> f32 {
    let density = authored_cloud_density();
    let cover = near_field_cloud_cover();
    if cirrus_layer {
        return clamp(authored_cirrus() * density * cover, 0.0, 1.0);
    }
    if mid_layer {
        let authored_mid = max(authored_altocumulus(), authored_altostratus());
        return clamp(authored_mid * density * cover, 0.0, 1.0);
    }
    return clamp(density * cover, 0.0, 1.0);
}

fn rot2(p: vec2<f32>, a: f32) -> vec2<f32> {
    let s = sin(a);
    let c = cos(a);
    return vec2<f32>(c * p.x - s * p.y, s * p.x + c * p.y);
}

fn hash22(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        hash12(p + vec2<f32>( 0.0,  0.0)),
        hash12(p + vec2<f32>(19.3, 73.7))
    );
}

fn cloud_cell_id(world_xz: vec2<f32>, cell_size: f32) -> vec2<i32> {
    let q = floor(world_xz / cell_size);
    return vec2<i32>(i32(q.x), i32(q.y));
}

fn cloud_cell_origin(cell: vec2<i32>, cell_size: f32) -> vec2<f32> {
    return vec2<f32>(f32(cell.x), f32(cell.y)) * cell_size;
}

fn cloud_cell_blend(world_xz: vec2<f32>, cell_size: f32) -> vec2<f32> {
    let f = fract(world_xz / cell_size);
    return f * f * (3.0 - 2.0 * f);
}

fn mix_cell_weather(a: CellWeather, b: CellWeather, t: f32) -> CellWeather {
    var w : CellWeather;
    w.coverage    = mix(a.coverage,    b.coverage,    t);
    w.cloud_type  = mix(a.cloud_type,  b.cloud_type,  t);
    w.humidity    = mix(a.humidity,    b.humidity,    t);
    w.convection  = mix(a.convection,  b.convection,  t);
    w.storm       = mix(a.storm,       b.storm,       t);
    w.low_amount  = mix(a.low_amount,  b.low_amount,  t);
    w.mid_amount  = mix(a.mid_amount,  b.mid_amount,  t);
    w.high_amount = mix(a.high_amount, b.high_amount, t);
    w.anvil_bias  = mix(a.anvil_bias,  b.anvil_bias,  t);
    w.seed        = mix(a.seed,        b.seed,        t);
    return w;
}

fn weather_clip_region(level: i32) -> vec4<f32> {
    return light.weather_clip_regions[level];
}

fn weather_uv(world_xz: vec2<f32>, level: i32) -> vec2<f32> {
    let clip = weather_clip_region(level);
    return (world_xz - clip.xy) * clip.w;
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
    w.anvil_bias  = saturate1(w.storm * 0.72 + w.convection * 0.28);
    w.seed        = hash_cell1(cell, 0x1367);
    return w;
}

// ============================================================
// PER-CELL VERTICAL AABB
//
// Given a cell's weather, compute the Y extent for each layer.
// Cells with negligible amount produce degenerate AABBs (y_min >= y_max).
// ============================================================

fn cell_low_aabb(w: CellWeather) -> CellAABB {
    var aabb: CellAABB;
    let low_fade = smoothstep(0.0, 0.04, w.low_amount);
    if low_fade <= 0.0 {
        aabb.y_min = 0.0;
        aabb.y_max = -1.0;
        return aabb;
    }

    let base_ctl        = clamp(light.weather_globals[1].x, 0.0, 1.0);
    let thick_ctl       = clamp(light.weather_globals[1].y, 0.0, 1.0);
    let storm_ctl       = w.storm;
    let storm_height_ctl = authored_storm_height();

    let base_range = mix(350.0, 2200.0, base_ctl);
    aabb.y_min = base_range * mix(1.0, 0.82, w.convection);

    let fair_thick  = mix(250.0, 1800.0, thick_ctl);
    let storm_thick = mix(900.0, 6000.0, storm_height_ctl);
    let base_thick = fair_thick + storm_thick * storm_ctl;
    let cell_thick = base_thick * mix(0.4, 1.0, w.low_amount) * (1.0 + w.convection * 0.3);
    aabb.y_max = aabb.y_min + cell_thick * low_fade;

    return aabb;
}

fn cell_mid_aabb(w: CellWeather) -> CellAABB {
    var aabb: CellAABB;
    let mid_fade = smoothstep(0.0, 0.04, w.mid_amount);
    if mid_fade <= 0.0 {
        aabb.y_min = 0.0;
        aabb.y_max = -1.0;
        return aabb;
    }

    let mid_base = mix(2500.0, 7500.0, clamp(light.weather_globals[3].y, 0.0, 1.0));
    let altostratus_ctl = authored_altostratus();
    let base_thick = mix(500.0, 2200.0, clamp(light.weather_globals[1].y, 0.0, 1.0))
        + altostratus_ctl * 900.0;
    aabb.y_min = mid_base - mix(250.0, 650.0, altostratus_ctl);
    aabb.y_max = aabb.y_min + base_thick * mix(0.35, 1.0, w.mid_amount) * mid_fade;

    return aabb;
}

fn cell_high_aabb(w: CellWeather) -> CellAABB {
    var aabb: CellAABB;
    let high_fade = smoothstep(0.0, 0.04, w.high_amount);
    if high_fade <= 0.0 {
        aabb.y_min = 0.0;
        aabb.y_max = -1.0;
        return aabb;
    }

    let high_base = mix(6500.0, 12500.0, clamp(light.weather_globals[2].w, 0.0, 1.0));
    let base_thick = mix(350.0, 1400.0, clamp(light.weather_globals[1].y, 0.0, 1.0));
    aabb.y_min = high_base - 250.0;
    aabb.y_max = aabb.y_min + base_thick * mix(0.4, 1.0, w.high_amount) * high_fade;

    return aabb;
}

// ============================================================
// RAY-AABB INTERSECTION (full 3D box: XZ cell + Y from CellAABB)
// Returns (t_enter, t_exit). If t_exit <= t_enter, ray misses.
// ============================================================

fn ray_cell_aabb(
    ro: vec3<f32>,
    inv_rd: vec3<f32>,
    cell: vec2<i32>,
    cell_size: f32,
    aabb: CellAABB,
) -> vec2<f32> {
    // Compute cell bounds relative to the ray origin using integer arithmetic
    // to avoid catastrophic f32 cancellation at large world coordinates.
    // cell_min_xz - ro.xz = (cell * cell_size) - ro.xz
    //   = f32(cell.x) * cell_size - ro.x
    // To keep precision: express as offset from the cell the ray origin sits in.
    // ro_cell = floor(ro.xz / cell_size) as i32
    // delta_cell = cell - ro_cell  (small integer, typically -1..24)
    // rel_min = f32(delta_cell) * cell_size  (small magnitude, no cancellation)
    let ro_cell_x = i32(floor(ro.x / cell_size));
    let ro_cell_z = i32(floor(ro.z / cell_size));
    let dx = cell.x - ro_cell_x;
    let dz = cell.y - ro_cell_z;
    // Fractional offset of ro inside its own cell — small, precise
    let frac_x = ro.x - f32(ro_cell_x) * cell_size;
    let frac_z = ro.z - f32(ro_cell_z) * cell_size;

    let rel_min_x = f32(dx) * cell_size - frac_x;
    let rel_max_x = rel_min_x + cell_size;
    let rel_min_z = f32(dz) * cell_size - frac_z;
    let rel_max_z = rel_min_z + cell_size;

    let t1x = rel_min_x * inv_rd.x;
    let t2x = rel_max_x * inv_rd.x;
    let t1z = rel_min_z * inv_rd.z;
    let t2z = rel_max_z * inv_rd.z;
    let t1y = (aabb.y_min - ro.y) * inv_rd.y;
    let t2y = (aabb.y_max - ro.y) * inv_rd.y;

    let t_near = max(max(min(t1x, t2x), min(t1y, t2y)), min(t1z, t2z));
    let t_far  = min(min(max(t1x, t2x), max(t1y, t2y)), max(t1z, t2z));

    return vec2<f32>(max(t_near, 0.0), t_far);
}

fn bilerp_vec4(a: vec4<f32>, b: vec4<f32>, c: vec4<f32>, d: vec4<f32>, u: vec2<f32>) -> vec4<f32> {
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// salt_i: a small integer differentiating calls — stays in u32 arithmetic, no f32 blow-up.
fn cell_angle(cell: vec2<i32>, salt_i: i32) -> f32 {
    return (hash_cell1(cell, salt_i) * 2.0 - 1.0) * 3.14159265;
}

fn cell_scale(cell: vec2<i32>, salt_i: i32, lo: f32, hi: f32) -> f32 {
    return mix(lo, hi, hash_cell1(cell, salt_i));
}

fn cell_offset(cell: vec2<i32>, salt_i: i32, span_m: f32) -> vec2<f32> {
    return (hash_cell2(cell, salt_i) * 2.0 - 1.0) * span_m;
}

fn sample_shape_cell(
    world_xz : vec2<f32>,
    world_y  : f32,
    cell     : vec2<i32>,
    cell_size: f32,
    freq_xz  : f32,
    freq_y   : f32,
    lod      : f32,
) -> vec4<f32> {
    let origin = cloud_cell_origin(cell, cell_size);
    let local  = world_xz - origin;

    // Integer salts encode the world seed without any float arithmetic blow-up.
    let seed_i = 0x1367;

    let angle0 = cell_angle(cell,  seed_i + 10);
    let scale0 = cell_scale(cell,  seed_i + 11, 0.82, 1.22);
    let offs0  = cell_offset(cell, seed_i + 12, cell_size * 0.30);

    let angle1 = cell_angle(cell,  seed_i + 20);
    let scale1 = cell_scale(cell,  seed_i + 21, 0.74, 1.35);
    let offs1  = cell_offset(cell, seed_i + 22, cell_size * 0.42);

    let p0 = rot2((local + offs0) * scale0, angle0);
    let p1 = rot2((local + offs1) * scale1, angle1);

    // Y-domain offset per cell: integer hash, no float cell index arithmetic.
    let y0 = world_y * freq_y + hash_cell1(cell, seed_i + 30) * 4.0;
    let y1 = world_y * freq_y * 1.11 + hash_cell1(cell, seed_i + 31) * 4.0;

    let uvw0 = fract(vec3<f32>(p0.x * freq_xz, y0, p0.y * freq_xz));
    let uvw1 = fract(vec3<f32>(p1.x * freq_xz * 0.73, y1, p1.y * freq_xz * 0.73));

    let s0 = textureSampleLevel(shape_noise_tex, cloud_field_sampler, uvw0, lod);
    let s1 = textureSampleLevel(shape_noise_tex, cloud_field_sampler, uvw1, lod + 0.5);

    return clamp(s0 * 0.62 + s1 * 0.38, vec4<f32>(0.0), vec4<f32>(1.0));
}

fn sample_detail_cell(
    world_xz : vec2<f32>,
    world_y  : f32,
    cell     : vec2<i32>,
    cell_size: f32,
    freq_xz  : f32,
    freq_y   : f32,
    lod      : f32,
) -> vec4<f32> {
    let origin = cloud_cell_origin(cell, cell_size);
    let local  = world_xz - origin;

    let seed_i = 0x1367;
    let angle0 = cell_angle(cell,  seed_i + 40);
    let scale0 = cell_scale(cell,  seed_i + 41, 0.88, 1.30);
    let offs0  = cell_offset(cell, seed_i + 42, cell_size * 0.36);

    let p0 = rot2((local + offs0) * scale0, angle0);
    let y0 = world_y * freq_y + hash_cell1(cell, seed_i + 43) * 4.0;

    let uvw0 = fract(vec3<f32>(p0.x * freq_xz, y0, p0.y * freq_xz));
    return textureSampleLevel(detail_noise_tex, cloud_field_sampler, uvw0, lod);
}

fn sample_shape_field(
    world_xz : vec2<f32>,
    world_y  : f32,
    cell_size: f32,
    freq_xz  : f32,
    freq_y   : f32,
    lod      : f32,
) -> vec4<f32> {
    let base = cloud_cell_id(world_xz, cell_size);
    let u    = cloud_cell_blend(world_xz, cell_size);

    let s00 = sample_shape_cell(world_xz, world_y, base + vec2<i32>(0, 0), cell_size, freq_xz, freq_y, lod);
    let s10 = sample_shape_cell(world_xz, world_y, base + vec2<i32>(1, 0), cell_size, freq_xz, freq_y, lod);
    let s01 = sample_shape_cell(world_xz, world_y, base + vec2<i32>(0, 1), cell_size, freq_xz, freq_y, lod);
    let s11 = sample_shape_cell(world_xz, world_y, base + vec2<i32>(1, 1), cell_size, freq_xz, freq_y, lod);

    return bilerp_vec4(s00, s10, s01, s11, u);
}

fn sample_detail_field(
    world_xz : vec2<f32>,
    world_y  : f32,
    cell_size: f32,
    freq_xz  : f32,
    freq_y   : f32,
    lod      : f32,
) -> vec4<f32> {
    let base = cloud_cell_id(world_xz, cell_size);
    let u    = cloud_cell_blend(world_xz, cell_size);

    let s00 = sample_detail_cell(world_xz, world_y, base + vec2<i32>(0, 0), cell_size, freq_xz, freq_y, lod);
    let s10 = sample_detail_cell(world_xz, world_y, base + vec2<i32>(1, 0), cell_size, freq_xz, freq_y, lod);
    let s01 = sample_detail_cell(world_xz, world_y, base + vec2<i32>(0, 1), cell_size, freq_xz, freq_y, lod);
    let s11 = sample_detail_cell(world_xz, world_y, base + vec2<i32>(1, 1), cell_size, freq_xz, freq_y, lod);

    return bilerp_vec4(s00, s10, s01, s11, u);
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
    let clip  = vec4<f32>(ndc, 1.0, 1.0);
    let world = cam.inv_view_proj * clip;
    let w     = world.w;
    let inv_w = sign(w) / max(abs(w), 1e-5);
    return normalize(world.xyz * inv_w - cam.camera_pos.xyz);
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

    if depth >= 0.99999 {
        return 1e9;
    }

    let scene_world = clip_to_world_pos(vec4<f32>(ndc, depth, 1.0));
    let delta       = scene_world - cam.camera_pos.xyz;
    return max(dot(delta, dir), 0.0);
}

fn slab_intersection(ro: vec3<f32>, rd: vec3<f32>, y0: f32, y1: f32) -> vec2<f32> {
    if abs(rd.y) < 1e-5 {
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

fn camera_inside_layer(cam_y: f32, base_y: f32, top_y: f32) -> bool {
    return cam_y >= base_y && cam_y <= top_y;
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

// ---- Height profiles (Schneider 2015 cloud-type gradient) ----

fn density_height_profile(y: f32, base_y: f32, top_y: f32,
                           bottom_soft: f32, top_soft: f32) -> f32 {
    let h      = remap01(y, base_y, top_y);
    let bottom = smoothstep(0.0, bottom_soft, h);
    let top    = 1.0 - smoothstep(1.0 - top_soft, 1.0, h);
    return bottom * top;
}

fn stratus_profile(h: f32) -> f32 {
    return smoothstep(0.0, 0.04, h) * (1.0 - smoothstep(0.65, 0.90, h)) * 0.80;
}

fn stratocumulus_profile(h: f32) -> f32 {
    let base = smoothstep(0.0, 0.10, h);
    let top_dense = smoothstep(0.55, 0.80, h) * (1.0 - smoothstep(0.88, 1.0, h));
    return base * (0.30 + top_dense * 0.70);
}

fn cumulus_profile(h: f32) -> f32 {
    let base = smoothstep(0.0, 0.07, h);
    let top  = 1.0 - smoothstep(0.60, 1.0, h);
    let peak = max(0.0, 1.0 - abs(h - 0.35) * 2.4);
    return base * top * max(0.30, peak);
}

fn cumulonimbus_profile(h: f32) -> f32 {
    let base  = smoothstep(0.0, 0.05, h);
    let tower = 1.0 - smoothstep(0.70, 0.83, h);
    let anvil = smoothstep(0.80, 0.92, h) * (1.0 - smoothstep(0.93, 1.0, h)) * 0.65;
    return base * tower + anvil;
}

fn cloud_type_height_gradient(h: f32, cloud_type: f32) -> f32 {
    // t * 3.0: idx maxes at 2 (for t<1.0), so idx+1 is always in-bounds [0,3].
    // Profiles: 0=stratus, 1=stratocumulus, 2=cumulus, 3=cumulonimbus.
    let t   = saturate1(cloud_type) * 3.0;
    let idx = min(u32(floor(t)), 2u);
    let f   = fract(t);
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
// ============================================================

fn macro_coverage_bias(world_xz: vec2<f32>, wind: vec2<f32>, time: f32, wspd: f32) -> f32 {
    let macro_xz = world_xz + wind * time * wspd * 2.0;

    let q0 = macro_xz * 0.0000025;
    let q1 = macro_xz * 0.0000056 + vec2<f32>(17.3, 5.1);
    let q2 = macro_xz * 0.0000125 + vec2<f32>(3.7, 11.5);
    let q3 = macro_xz * 0.000028  + vec2<f32>(9.1, 21.3);

    let n = noise2(q0) * 0.40 + noise2(q1) * 0.30 + noise2(q2) * 0.20 + noise2(q3) * 0.10;
    return mix(0.20, 1.65, smoothstep(0.15, 0.85, n));
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
) -> f32 {
    let wind       = safe_normalize2(light.weather_globals[0].zw, vec2<f32>(0.9, -0.4));
    let side       = vec2<f32>(-wind.y, wind.x);
    let time       = light.weather_globals[0].x;
    let wind_speed = max(light.weather_globals[0].y, 0.05);

    let coverage      = weather.coverage;
    let cloud_type    = weather.cloud_type;
    let is_stratus    = smoothstep(0.30, 0.05, cloud_type);
    let cumulus_ctl   = weather.convection;
    let storm         = weather.storm;
    let anvil_ctl     = weather.anvil_bias;
    let density_ctl   = mix(0.35, 1.55, authored_cloud_density());
    let storm_height_ctl = authored_storm_height();
    let authored_anvil = authored_anvil_spread();
    
    let thickness_ctl = light.weather_globals[1].y;
    let shear_ctl     = light.weather_globals[1].z;
    let erosion_ctl   = light.weather_globals[1].w;
    let detail_ctl    = light.weather_globals[2].y;

    let advected_xz = p.xz + wind * time * wind_speed * 40.0;
    let h           = remap01(eff_y, base_y, top_y);

    let shear = side * (h - 0.5) * wind_speed * 180.0 * shear_ctl;

    let noise_scale_ctl = saturate1(light.weather_globals[2].x);
    let freq_mod = mix(0.70, 1.50, noise_scale_ctl);
    let shape_freq_xz = mix(0.00015, 0.00045, weather.humidity) * freq_mod;
    let shape_freq_y  = mix(0.00025, 0.00065, weather.convection) * freq_mod;

    let s_levels = max(textureNumLevels(shape_noise_tex), 1u);
    let max_slod = f32(s_levels - 1u);

    let shape_uv_scale = max(shape_freq_xz, shape_freq_y);
    let shape_lod_footprint = footprint_texel_lod(
        sample_footprint_m,
        shape_uv_scale,
        SHAPE_TEX_SIZE,
        max_slod
    );

    let shape_lod = select(
        max(shape_lod_footprint, clamp(dist_t * 0.9 + grazing_f * 0.15, 0.0, max_slod)),
        1.5,
        shadow_lod
    );

    let shape_pos_xz = advected_xz + shear;

    let shape_tex = sample_shape_field(
        shape_pos_xz,
        eff_y,
        CLOUD_SHAPE_CELL_M,
        shape_freq_xz,
        shape_freq_y,
        shape_lod
    );

    let shape_base = shape_tex.r;
    let worley_fbm = shape_tex.g * 0.625 + shape_tex.b * 0.25 + shape_tex.a * 0.125;
    let shape_erosion = worley_fbm * mix(1.0, 0.2, coverage);
    let base_shape = remap01(shape_base, shape_erosion, 1.0);

    let threshold  = max(mix(0.55, -0.10, coverage), 0.0);
    let lod_soften = saturate1(shape_lod * 0.04);
    let softness   = mix(0.16, 0.06, coverage) + lod_soften;
    let body       = smoothstep(threshold - softness * 0.4, threshold + softness, base_shape);

    var eroded_body = body;
    if !shadow_lod {
        let d_levels = max(textureNumLevels(detail_noise_tex), 1u);
        let max_dlod = f32(d_levels - 1u);

        let detail_freq_xz = shape_freq_xz * (2.7 + shear_ctl * 0.8);
        let detail_freq_y  = shape_freq_y  * (2.2 + shear_ctl * 0.6);

        let detail_lod = footprint_texel_lod(
            sample_footprint_m,
            max(detail_freq_xz, detail_freq_y),
            DETAIL_TEX_SIZE,
            max_dlod
        );

        let footprint_detail_keep = 1.0 - smoothstep(12.0, 40.0, sample_footprint_m);
        let detail_keep = footprint_detail_keep
                        * smoothstep(0.02, 0.20, dist_fade)
                        * (1.0 - smoothstep(0.0, 0.65, dist_t))
                        * mix(1.0, 0.65, grazing_f);

        if detail_keep > 0.01 && body > 0.005 {
            let detail_tex = sample_detail_field(
                shape_pos_xz,
                eff_y,
                CLOUD_DETAIL_CELL_M,
                detail_freq_xz,
                detail_freq_y,
                detail_lod
            );

            let detail_fbm = detail_tex.r * 0.625 + detail_tex.g * 0.25 + detail_tex.b * 0.125;
            let curl_str   = detail_tex.a * 2.0 - 1.0;
            let eroded_detail = saturate1(detail_fbm - curl_str * 0.25);

            let edge = 1.0 - smoothstep(0.0, 0.55, body);
            let near_factor = saturate1(1.0 - dist_t * 2.5);

            let erosion_strength =
                erosion_ctl * detail_ctl
                * mix(0.25, 0.75, detail_keep)
                * mix(0.50, 1.0, edge)
                * (1.0 + near_factor * 0.65);

            eroded_body = erode_remap(body, eroded_detail * erosion_strength);
        }
    }

    let stratus_body = remap01(base_shape, saturate1(1.0 - coverage * 0.80), 1.0);
    let typed_body   = mix(eroded_body, stratus_body, is_stratus);

    let regional_thickness = mix(0.55, 1.0, weather.humidity);
    let type_gradient  = cloud_type_height_gradient(h, cloud_type);
    let height_shape   = density_height_profile(
        eff_y, base_y, top_y,
        mix(0.10, 0.26, is_stratus),
        mix(0.20, 0.38 + thickness_ctl * 0.14, cumulus_ctl + storm * 0.30),
    );

    let final_height = mix(height_shape, type_gradient, saturate1(cloud_type * 1.2))
                     * regional_thickness;

    let anvil    = smoothstep(0.68, 1.0, h)
                 * storm
                 * (0.10 + 0.30 * anvil_ctl + 0.55 * authored_anvil)
                 * weather.anvil_bias
                 * (0.85 + storm_height_ctl * 0.55);
    let type_mix = saturate1(cumulus_ctl * 0.85 + cloud_type * 0.45 - is_stratus * 0.25);
    let type_mod = mix(0.82, 1.0, type_mix);
    let amount_fade = smoothstep(0.0, 0.05, weather.low_amount);

    return typed_body * final_height * type_mod * density_ctl * (1.0 + anvil) * amount_fade;
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
    let wspd = max(light.weather_globals[0].y, 0.05);

    let advected_xz = p.xz + wind * time * wspd * 60.0;

    let q     = advected_xz * 0.000022;
    let along = dot(q, wind);
    let perp  = dot(q, side);

    let wp = vec2<f32>(along * 3.0, perp * 10.0);
    let w1 = (hash12(floor(wp) + vec2<f32>(3.7, 1.1)) * 2.0 - 1.0) * 0.16;
    let w2 = (hash12(floor(wp * 2.1) + vec2<f32>(7.2, 9.4)) * 2.0 - 1.0) * 0.07;
    let warped_xz = advected_xz + side * ((w1 + w2) * 45454.5);

    let h_frac = remap01(eff_y, base_y, top_y);

    let cirrus_lod_shape = clamp(
        log2(max(sample_footprint_m * 0.00015 * SHAPE_TEX_SIZE, 1.0)),
        0.0, 7.0
    );
    let cirrus_lod_detail = clamp(
        log2(max(sample_footprint_m * 0.00050 * DETAIL_TEX_SIZE, 1.0)),
        0.0, 6.0
    );

    let s1 = sample_shape_field(
        warped_xz,
        h_frac * 1000.0,
        CLOUD_SHAPE_CELL_M,
        0.000100,
        0.0012,
        max(1.5, cirrus_lod_shape)
    ).r;

    let s2 = sample_shape_field(
        warped_xz + vec2<f32>(1400.0, 800.0),
        h_frac * 1700.0,
        CLOUD_SHAPE_CELL_M,
        0.000150,
        0.0018,
        max(1.0, cirrus_lod_shape)
    ).r;

    let fine_keep = 1.0 - smoothstep(14.0, 36.0, sample_footprint_m);

    let detail_tex = sample_detail_field(
        warped_xz,
        h_frac * 2500.0,
        CLOUD_DETAIL_CELL_M,
        0.00030,
        0.0026,
        cirrus_lod_detail
    );
    let detail_fbm = detail_tex.r * 0.625 + detail_tex.g * 0.25 + detail_tex.b * 0.125;
    let curl_str   = detail_tex.a * 2.0 - 1.0;
    let s3         = saturate1(detail_fbm - curl_str * 0.15);

    let density_ctl = mix(0.45, 1.35, authored_cloud_density());
    let fiber       = s1 * 0.42 + s2 * 0.33 + s3 * (0.25 * fine_keep);
    let height_shape = density_height_profile(eff_y, base_y, top_y, 0.12, 0.18);
    let thresh      = mix(0.08, 0.18, 1.0 - cirrus_amt);

    return smoothstep(thresh, thresh + 0.16, fiber) * height_shape * cirrus_amt * density_ctl * 1.55;
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
    let wspd = max(light.weather_globals[0].y, 0.05) * 0.7;

    let advected_xz = p.xz + wind * time * wspd * 47.5;

    let ac_bias = authored_altocumulus() * (0.45 + weather.convection * 0.55)
                + weather.convection * 0.20;
    let as_bias = authored_altostratus() * (0.55 + weather.humidity * 0.45)
                + (1.0 - weather.convection) * 0.10;
    let mid_bias_sum = max(ac_bias + as_bias, 1e-4);
    let ac_amt = weather.mid_amount * ac_bias / mid_bias_sum;
    let as_amt = weather.mid_amount * as_bias / mid_bias_sum;
    if ac_amt + as_amt <= 0.001 { return 0.0; }

    let mid_lod = clamp(
        log2(max(sample_footprint_m * 0.00012 * SHAPE_TEX_SIZE, 1.0)),
        0.0, 7.0
    );

    let billow = sample_shape_field(
        advected_xz,
        eff_y,
        CLOUD_SHAPE_CELL_M,
        0.00012,
        0.00024,
        max(1.5, mid_lod)
    ).r;

    let cell = sample_shape_field(
        advected_xz + vec2<f32>(2000.0, -1000.0),
        eff_y,
        CLOUD_SHAPE_CELL_M,
        0.00018,
        0.00040,
        max(1.0, mid_lod)
    ).g;

    let h = remap01(eff_y, base_y, top_y);

    let density_ctl = mix(0.35, 1.40, authored_cloud_density());
    let height_ac = smoothstep(0.0, 0.12, h) * (1.0 - smoothstep(0.72, 1.0, h));
    let ac_base   = billow * 0.55 + cell * 0.45;
    let ac_thresh = mix(0.15, 0.35, 1.0 - ac_amt);
    var ac_density = smoothstep(ac_thresh, ac_thresh + 0.18, ac_base)
                   * height_ac * ac_amt * density_ctl;

    let fine_keep = 1.0 - smoothstep(10.0, 28.0, sample_footprint_m);
    if ac_density > 0.001 && fine_keep > 0.01 {
        let detail_tex = sample_detail_field(
            advected_xz,
            eff_y,
            CLOUD_DETAIL_CELL_M,
            0.00050,
            0.00050,
            mid_lod
        );
        let detail_fbm = detail_tex.r * 0.625 + detail_tex.g * 0.25 + detail_tex.b * 0.125;
        ac_density = erode_remap(ac_density, detail_fbm * 0.4 * fine_keep);
    }

    let sheet_a = sample_shape_field(
        advected_xz,
        eff_y,
        CLOUD_SHAPE_CELL_M,
        0.00006,
        0.00018,
        max(2.0, mid_lod)
    ).r;
    let sheet_b = sample_shape_field(
        advected_xz + vec2<f32>(6800.0, -4200.0),
        eff_y * 0.55,
        CLOUD_SHAPE_CELL_M,
        0.000035,
        0.00010,
        max(2.5, mid_lod + 0.5)
    ).r;
    let sheet = saturate1(sheet_a * 0.35 + sheet_b * 0.65);

    let height_as  = smoothstep(0.0, 0.08, h) * (1.0 - smoothstep(0.88, 1.0, h));
    let veil = smoothstep(0.12, 0.42, sheet);
    let as_density = veil * height_as * as_amt * density_ctl * 1.05;

    return ac_density + as_density;
}

fn sample_density_shadow(p: vec3<f32>, base_y: f32, top_y: f32, eff_y: f32,
                          weather: CellWeather) -> f32 {
    return sample_low_density(p, base_y, top_y, eff_y, 1.0, 0.5, 0.8, true, 8.0, weather);
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
    let wind_spd = max(light.weather_globals[0].y, 0.05);
    let mult = select(select(40.0, 33.25, layer_id == 1), 60.0, layer_id == 2);
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
    let min_spacing = select(select(90.0, 140.0, layer_id == 1), 220.0, layer_id == 2);
    let max_spacing = select(select(520.0, 900.0, layer_id == 1), 1400.0, layer_id == 2);
    let spacing = clamp(
        max(sample_footprint_m * 0.75, layer_thickness_m * 0.08),
        min_spacing,
        max_spacing,
    );
    let tap_count = select(3, 2, layer_id == 2);

    var optical_depth = 0.0;
    var norm = 0.0;
    for (var i: i32 = 0; i < tap_count; i = i + 1) {
        let t = spacing * (f32(i) + 0.75);
        let q = p + light_dir * t;
        let tap = sample_shadow_tap_context(q, layer_id);
        if !tap.valid {
            continue;
        }

        let d = select(
            select(
                sample_density_shadow(q, tap.aabb.y_min, tap.aabb.y_max, q.y, tap.weather),
                sample_mid_density_shadow(q, tap.aabb.y_min, tap.aabb.y_max, q.y, tap.weather),
                layer_id == 1,
            ),
            sample_cirrus_density(
                q,
                tap.aabb.y_min,
                tap.aabb.y_max,
                q.y,
                max(sample_footprint_m, 24.0),
                tap.weather,
            ),
            layer_id == 2,
        );
        let w = 1.0 / (1.0 + t * 0.00012);
        optical_depth += d * spacing * w;
        norm += spacing * w;
    }

    return optical_depth / max(norm, 1e-4);
}

// ---- Blue-noise temporal jitter ----
// frag_coord is in cloud-render-target pixels (half-resolution when CLOUD_RENDER_SCALE=2).
// The 64-texel blue-noise tile is designed to tile against full-resolution screen pixels.
// Scale frag_coord by CLOUD_RENDER_SCALE so that adjacent cloud pixels map to
// distinct noise texels, restoring the intended high-frequency dithering pattern.
const CLOUD_RENDER_SCALE_F: f32 = 2.0; // Must match CLOUD_RENDER_SCALE in render.rs

fn blue_noise_jitter(frag_coord: vec2<f32>) -> f32 {
    let sz = 64;
    // Reconstruct the full-resolution pixel coordinate for correct tile mapping.
    let full_res_coord = frag_coord * CLOUD_RENDER_SCALE_F;
    let ip = vec2<i32>(full_res_coord);
    let pixel = ip % vec2<i32>(sz, sz);
    let base  = textureLoad(blue_noise_tex, pixel, 0).r;
    let frame_offset = fract(floor(light.weather_globals[0].x * 60.0) * 0.61803398875);
    return fract(base + frame_offset);
}

// ============================================================
// MAIN RAY MARCH — DDA CELL-GRID TRAVERSAL
//
// Instead of marching between global slab planes, we walk the
// 2D XZ weather cell grid (24km cells) using Amanatides & Woo DDA.
// For each cell the ray enters:
//   1. Sample that cell's weather
//   2. Compute the cell's vertical AABB from its weather parameters
//   3. Intersect the ray with the 3D cell box (XZ cell + Y AABB)
//   4. March only within that intersection segment
//
// This eliminates hard global clip planes, naturally handles
// above/below/inside views, and keeps marching tightly bounded.
// ============================================================

// Layer ID constants for selecting the correct per-cell AABB.
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

// Safe reciprocal for ray direction components (avoids div-by-zero).
// Uses larger epsilon to catch near-zero rays (important for horizontal/grazing rays).
fn safe_inv_rd(rd: vec3<f32>) -> vec3<f32> {
    let eps = 1e-6;
    let big = 1e12;
    var r: vec3<f32>;
    if abs(rd.x) < eps { r.x = big; } else { r.x = 1.0 / rd.x; }
    if abs(rd.y) < eps { r.y = big; } else { r.y = 1.0 / rd.y; }
    if abs(rd.z) < eps { r.z = big; } else { r.z = 1.0 / rd.z; }
    return r;
}

// Optimized per-cell AABB with position-aware sampling.
// Uses weather_at_world (which already does bilinear blending across neighbors)
// sampled at the actual ray position. This avoids expensive 4-corner sampling
// while still providing smooth interpolation.
//
// The key insight: weather_at_world() already blends across the 2×2 cell neighborhood.
// So sampling at ray position inherently gives us smooth position-dependent AABB.
fn cell_aabb_for_position(
    world_xz: vec2<f32>,
    cell: vec2<i32>,
    cell_size: f32,
    layer_id: i32
) -> CellAABB {
    let wind = safe_normalize2(light.weather_globals[0].zw, vec2<f32>(0.9, -0.4));
    let time = light.weather_globals[0].x;
    let wind_spd = max(light.weather_globals[0].y, 0.05);

    // Match the wind advection multiplier used by each layer's density sampler exactly.
    // Low  layer: sample_low_density   uses wind_speed * 80.0
    // Mid  layer: sample_mid_density   uses wspd * 0.7 * 95.0  = wind_speed * 66.5
    // High layer: sample_cirrus_density uses wspd * 120.0
    // The AABB that bounds marching must enclose the advected density field,
    // so they must use the same advection offset. Previously the AABB used 12/13/15
    // which caused the AABB to bracket empty space while the real cloud was elsewhere.
    let wx_mult = select(
        select(40.0, 33.25, layer_id == LAYER_MID),
        60.0,
        layer_id == LAYER_HIGH
    );
    let wx_offset = wind * time * wind_spd * wx_mult;

    let w = weather_at_world(world_xz + wx_offset);
    return cell_aabb_for_layer(w, layer_id);
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
    storm_bias   : f32,
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
            return CloudSample(vec3<f32>(0.0), 1.0, vec4<f32>(0.0));
        }
    }
    
    let ray_length_in_slab = max(env_exit - env_enter, 0.0);

    // ---- Layer ID for per-cell AABB dispatch ----
    var layer_id = LAYER_LOW;
    if mid_layer    { layer_id = LAYER_MID; }
    if cirrus_layer { layer_id = LAYER_HIGH; }

    let activity = layer_activity(cirrus_layer, mid_layer);
    if cirrus_layer && activity <= 0.010 {
        return CloudSample(vec3<f32>(0.0), 1.0, vec4<f32>(0.0));
    }
    if mid_layer && activity <= 0.012 {
        return CloudSample(vec3<f32>(0.0), 1.0, vec4<f32>(0.0));
    }

    // ---- Pre-compute frame constants (shared across all cells) ----
    let grazing_f  = 1.0 - saturate1(abs(ray_dir.y) * 2.5);
    let motion_mod = mix(1.0, 0.78, saturate1(cam.world_params.z));
    let jitter     = blue_noise_jitter(frag_coord);
    let inv_rd     = safe_inv_rd(ray_dir);

    let wind     = safe_normalize2(light.weather_globals[0].zw, vec2<f32>(0.9, -0.4));
    let time     = light.weather_globals[0].x;
    let wind_spd = max(light.weather_globals[0].y, 0.05);
    // Match density sampler wind multipliers exactly: low=80, mid=66.5, high=120
    let wx_mult  = select(select(40.0, 33.25, mid_layer), 60.0, cirrus_layer);
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

    let sun_spectral_trans = sky_transmittance_to_sun(sun_dir.y);
    let moon_light = light.moon_color_twilight.rgb * light.moon_dir_intensity.w * night;

    let ms_phase_iso = cs_phase(cos_theta_sun, 0.05);
    let ms_a         = select(0.50, 0.25, cirrus_layer);
    let ms_geometric = ms_a / (1.0 - ms_a);
    let shadow_dir = select(sun_dir, moon_dir, night > 0.7);
    let ground_base  = light.sun_color_daylight.rgb * sun_spectral_trans
                     * daylight * max(sun_dir.y, 0.12) * 0.06
                     * vec3<f32>(0.15, 0.18, 0.12) * 1.20;

    let fade_dist = max(
        max(light.fog_params.y * 1.5, cam.world_params.w * 1.3),
        max_dist * 1.2
    );

    // ---- Accumulated state across all cells ----
    var trans          = 1.0;
    var radiance       = vec3<f32>(0.0);
    var accumulated_od = 0.0;
    var first_hit_t    = 1e9;           // nearest t where density was found
    var total_steps    = 0;             // global step budget across all cells this march
    var debug_density_max = 0.0;
    var debug_any_aabb    = 0.0;
    var debug_shadow_max  = 0.0;

    // ---- 2D DDA setup on the XZ weather cell grid ----
    let cell_size = CLOUD_WEATHER_CELL_M;       // 24km
    let ro_xz     = vec2<f32>(ray_origin.x, ray_origin.z);
    let rd_xz     = vec2<f32>(ray_dir.x, ray_dir.z);

    // Starting cell based on ray origin position
    var cell = cloud_cell_id(ro_xz, cell_size);

    // DDA step direction (+1 or -1 per axis)
    var step_x = -1;
    if ray_dir.x >= 0.0 { step_x = 1; }
    var step_z = -1;
    if ray_dir.z >= 0.0 { step_z = 1; }

    // Compute distance along ray to next cell boundary on each axis.
    // Use frac() of the position within its cell to avoid large-float subtraction.
    // frac_x = ro.x - floor(ro.x / cell_size) * cell_size  (position within cell, 0..cell_size)
    let frac_start_x = ro_xz.x - f32(cell.x) * cell_size;
    let frac_start_z = ro_xz.y - f32(cell.y) * cell_size;

    // Distance from ray origin to the leading XZ boundary of its starting cell
    let dist_to_x_wall = select(frac_start_x, cell_size - frac_start_x, step_x > 0);
    let dist_to_z_wall = select(frac_start_z, cell_size - frac_start_z, step_z > 0);

    let next_x = dist_to_x_wall * abs(inv_rd.x);
    let next_z = dist_to_z_wall * abs(inv_rd.z);

    var tMax_x = max(next_x, 0.0);
    var tMax_z = max(next_z, 0.0);

    // How far along the ray for one full cell crossing on each axis
    let tDelta_x = abs(cell_size * inv_rd.x);
    let tDelta_z = abs(cell_size * inv_rd.z);

    // ---- DDA loop: traverse cells ----
    let max_cells = 96;  // Safely covers up to 120km at 4.8km cell size
    var ray_t = 0.0;        // current position along the ray

    // Global step budget: caps total work regardless of how many cells have clouds.
    // Low layer: 256 max, mid/high: 128 max.  Prevents frame-rate collapse at cloud level.
    let secondary_activity = smoothstep(0.015, 0.30, activity);
    let secondary_budget = i32(round(mix(40.0, 160.0, secondary_activity)));
    let global_step_budget = select(secondary_budget, 320, !cirrus_layer && !mid_layer);
    
    // Dynamically guarantee we can reach the end of the slab within budget
    let min_dt  = ray_length_in_slab / max(f32(global_step_budget), 1.0);
    let layer_dt_max = select(select(14.0, 18.0, mid_layer), 24.0, cirrus_layer);
    let base_dt = mix(6.0, layer_dt_max, grazing_f) / max(motion_mod, 0.1);
    let dt      = max(base_dt, min_dt);
    let global_offset = dt * jitter;

    for (var ci: i32 = 0; ci < max_cells; ci = ci + 1) {
        if trans <= 0.003 { break; }
        if ray_t >= max_dist { break; }
        if total_steps >= global_step_budget { break; }

        // Distance to the exit of this cell column (XZ boundary)
        let cell_exit_t = min(tMax_x, tMax_z);

        // We use the global slab bounds because the weather texture resolution (250m) is 
        // much finer than the DDA cell size (4.8km). Evaluating only at cell boundaries 
        // will incorrectly cull clouds. The inner loop correctly and cheaply culls 
        // empty space at the exact step position.
        var aabb: CellAABB;
        aabb.y_min = base_y;
        aabb.y_max = top_y;

        if cell_aabb_valid(aabb) {
            // Intersect ray with the 3D cell box
            let hit = ray_cell_aabb(ray_origin, inv_rd, cell, cell_size, aabb);
            let t_enter = max(hit.x, ray_t);
            let t_exit  = min(hit.y, min(cell_exit_t, max_dist));

            if t_exit > t_enter {
                // ---- March within this cell's AABB segment ----
                
                // Evaluate continuous step grid. Prevent `ceil()` accumulation
                // error by indexing against the absolute global step count.
                let start_k = max(0, i32(ceil((t_enter - global_offset) / dt)));
                let end_k   = max(0, i32(ceil((t_exit - global_offset) / dt)));
                let steps   = max(0, end_k - start_k);
                let actual_steps = min(steps, global_step_budget - total_steps);

                if actual_steps > 0 {
                    let t_start = f32(start_k) * dt + global_offset;
                    total_steps += actual_steps;
                    
                for (var i: i32 = 0; i < actual_steps; i = i + 1) {
                    if trans <= 0.003 { break; }

                    let t_cur = t_start + f32(i) * dt;
                    if t_cur >= t_exit { break; }

                    let p = ray_origin + ray_dir * t_cur;

                    // Evaluate exact weather at the sample position (standard AAA practice)
                    let weather = weather_at_world(p.xz + wx_offset_march);
                    let local_aabb = cell_aabb_for_layer(weather, layer_id);
                    let cell_base_y = local_aabb.y_min;
                    let cell_top_y  = local_aabb.y_max;

                    // Strictly cull empty space at the exact step position to save expensive 3D noise fetches
                    if cell_top_y <= cell_base_y || p.y < cell_base_y || p.y > cell_top_y {
                        continue;
                    }
                    debug_any_aabb = 1.0;

                    let t_from_cam         = cam_dist_offset + t_cur;
                    let cone_footprint_m   = max(t_from_cam * cone_spread * 1.5, 0.0);
                    let sample_footprint_m = max(max(dt, cone_footprint_m), 1.0);

                    let abs_dist  = t_from_cam / fade_dist;
                    let dist_fade = 1.0 - smoothstep(0.85, 1.0, abs_dist);
                    let dist_t    = saturate1(t_from_cam / max(max_dist, 1.0));

                    var raw_density: f32;
                    if cirrus_layer {
                        raw_density = sample_cirrus_density(
                            p, cell_base_y, cell_top_y, p.y, sample_footprint_m, weather
                        );
                    } else if mid_layer {
                        raw_density = sample_mid_density(
                            p, cell_base_y, cell_top_y, p.y, sample_footprint_m, weather
                        );
                    } else {
                        raw_density = sample_low_density(
                            p, cell_base_y, cell_top_y, p.y, dist_fade, dist_t, grazing_f,
                            false, sample_footprint_m, weather
                        );
                    }
                    let sample_density = raw_density * dist_fade;
                    debug_density_max = max(debug_density_max, sample_density);

                    let step_alpha_cheap = 1.0 - exp(-sample_density * extinction * dt);
                    let pre_step_trans   = trans;

                    if sample_density > 0.0001 {
                        trans          *= 1.0 - step_alpha_cheap;
                        accumulated_od += sample_density * dt * 0.40;
                        if first_hit_t > 1e8 {
                            first_hit_t = t_cur;
                        }
                    }

                    if sample_density > 0.0005 && step_alpha_cheap > 0.0002 {
                        let layer_thickness_m = max(cell_top_y - cell_base_y, 1.0);
                        let layer_mid_y = (cell_base_y + cell_top_y) * 0.5;
                        let project_t = max((layer_mid_y - p.y) / max(shadow_dir.y, 0.08), 0.0);
                        let macro_sample_xz = p.xz + shadow_dir.xz * min(project_t, light.weather_clip_regions[2].z);
                        let macro_shadow_od = macro_shadow_optical_depth(macro_sample_xz, layer_id);
                        let local_shadow_od = sample_local_self_shadow(
                            p,
                            shadow_dir,
                            layer_id,
                            sample_footprint_m,
                            layer_thickness_m,
                        );
                        let macro_blend = max(
                            smoothstep(26.0, 120.0, sample_footprint_m),
                            smoothstep(0.25, 0.80, dist_t),
                        );
                        let local_scale = select(select(0.95, 0.80, mid_layer), 0.58, cirrus_layer);
                        let macro_scale = select(select(0.24, 0.18, mid_layer), 0.12, cirrus_layer);
                        let shadow_d = mix(
                            local_shadow_od * local_scale + macro_shadow_od * macro_scale * 0.45,
                            macro_shadow_od * macro_scale,
                            macro_blend,
                        );
                        debug_shadow_max = max(debug_shadow_max, shadow_d);

                        // Height gradient within this cell's vertical extent.
                        let h_frac      = saturate1(remap01(p.y, cell_base_y, cell_top_y));
                        let cell_thick  = max(cell_top_y - cell_base_y, 1.0);
                        let od_from_top = (1.0 - h_frac) * extinction * cell_thick * 0.0003;
                        let top_bright  = exp(-od_from_top * 1.8) * 0.60 + 0.40;
                        let height_gradient = top_bright * (1.0 - smoothstep(0.92, 1.0, h_frac) * 0.12);

                        let depth_ao    = exp(-accumulated_od * select(0.35, 0.14, cirrus_layer));
                        let od          = sample_density * dt * select(0.48, 0.20, cirrus_layer);
                        let beer        = exp(-od);
                        let powder      = 1.0 - exp(-od * 2.0);
                        let sun_facing  = saturate1(cos_theta_sun * 0.5 + 0.5);
                        let beer_powder = beer * (1.0 + powder * sun_facing * 0.85);

                        let ext_light   = select(select(5.0, 4.2, mid_layer), 2.8, cirrus_layer);
                        let light_trans = exp(-shadow_d * ext_light);
                        let moon_trans  = exp(-shadow_d * ext_light * 0.45);

                        let light_trans_attenuated = light_trans * cirrus_transmittance_atten;
                        let sun_lit = light.sun_color_daylight.rgb
                                    * sun_spectral_trans * light.sun_dir_intensity.w
                                    * light_trans_attenuated * phase_sun * daylight * height_gradient;
                        let moon_lit = moon_light * moon_trans * phase_moon * 0.40 * height_gradient;

                        let ms_trans_factor = pow(max(light_trans_attenuated, 0.001), 0.4);
                        let ms_sun_base = light.sun_color_daylight.rgb * sun_spectral_trans
                                        * ms_phase_iso * ms_trans_factor * ms_geometric
                                        * daylight * height_gradient;
                        let ms_depth = saturate1(accumulated_od * 0.20);
                        let ms_white = vec3<f32>(dot(ms_sun_base, vec3<f32>(0.2126, 0.7152, 0.0722)));
                        let ms_sun   = mix(ms_sun_base, vec3<f32>(ms_white), ms_depth * 0.65);
                        let ms_moon  = moon_light * pow(max(moon_trans, 0.001), 0.4)
                                     * ms_geometric * 0.25 * night * height_gradient;

                        let sky_zenith  = light.sky_top_saturation.rgb;
                        let sky_horizon = light.sky_horizon_reserved.rgb;
                        let sky_above_blend = saturate1(h_frac * 1.2 + 0.15);
                        let sky_above   = mix(sky_horizon, sky_zenith, sky_above_blend)
                                        * (0.18 + ambient_gain * 0.55);
                        let ground_dist   = saturate1(1.0 - h_frac * 1.6);
                        let ground_bounce = ground_base * ground_dist * 1.5;
                        let ambient       = (sky_above + ground_bounce
                                           + vec3<f32>(0.010, 0.015, 0.030) * night) * depth_ao;

                        let interior_depth    = 1.0 - depth_ao;
                        let blue_tint         = vec3<f32>(0.88, 0.92, 1.0) * 0.04 * interior_depth
                                              * select(1.0, 0.0, cirrus_layer);
                        let ambient_with_tint = ambient + blue_tint;

                        let edge_density = smoothstep(0.0, 0.035, sample_density)
                                         * (1.0 - smoothstep(0.035, 0.16, sample_density));
                        let silver_angle       = pow(max(cos_theta_sun * 0.5 + 0.5, 0.0), 2.5);
                        let silver_density_gate = 1.0 / (1.0 + sample_density * 10.0);
                        let silver_light = light.sun_color_daylight.rgb * sun_spectral_trans
                                         * light_trans
                                         * edge_density * silver * silver_angle * 2.2 * daylight
                                         * silver_density_gate;

                        let scatter = mix(
                                ambient_with_tint + ms_sun + ms_moon,
                                sun_lit + moon_lit + ambient_with_tint + ms_sun + ms_moon,
                                beer_powder
                            ) * mix(1.0, 0.72, storm_bias)
                            + silver_light;

                        radiance += pre_step_trans * scatter * step_alpha_cheap;
                    }
                }
                }
            }
        }

        // ---- DDA advance to next cell ----
        if tMax_x < tMax_z {
            ray_t  = tMax_x;
            cell.x += step_x;
            tMax_x += tDelta_x;
        } else {
            ray_t  = tMax_z;
            cell.y += step_z;
            tMax_z += tDelta_z;
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
    storm_bias: f32,
    cirrus_transmittance_atten: f32,
    cone_spread: f32,
) -> CloudSample {
    return march_layer_core(
        ray_origin, ray_dir, cam_dist_offset, sun_dir, moon_dir, frag_coord,
        base_y, top_y, max_dist, 0.18, 0.72, 0.74,
        false, false, storm_bias, cirrus_transmittance_atten, cone_spread
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
    return march_layer_core(
        ray_origin, ray_dir, cam_dist_offset, sun_dir, moon_dir, frag_coord,
        base_y, top_y, max_dist, 0.075, 0.75, 0.70,
        false, true, 0.0, cirrus_transmittance_atten, cone_spread
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
    return march_layer_core(
        ray_origin, ray_dir, cam_dist_offset, sun_dir, moon_dir, frag_coord,
        base_y, top_y, max_dist, 0.045, 0.75, 0.86,
        true, false, 0.0, 1.0, cone_spread
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
    let wspd = max(light.weather_globals[0].y, 0.05);
    let wx   = weather_at_world(p_mid.xz + wind * time * wspd * 60.0);

    let local_aabb = cell_aabb_for_layer(wx, LAYER_HIGH);
    let d_cirrus = sample_cirrus_density(p_mid, local_aabb.y_min, local_aabb.y_max, p_mid.y, 32.0, wx);
    return exp(-d_cirrus * 0.045 * 50.0);
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

    let summary  = light.weather_clip_summaries[0];
    let storm    = summary.z;

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

    // cam.world_params.w already holds the visible terrain extent in meters.
    // Clamp between 2000 and 60000 meters so dist_t and dist_fade are active across
    // the visible range, LOD kicks in at moderate distance, and distant clouds fade cleanly.
    // The old 200000 floor made dist_t ≈ 0 everywhere, breaking all distance-based quality.
    let raw_visible = cam.world_params.w;
    let unclamped_max_dist = 120000.0;
    let scene_dist = scene_depth_distance(in.ndc, dir);
    let max_dist = min(unclamped_max_dist, max(scene_dist - scale * 2.0, 0.0));
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
        cirrus_trans = approx_cirrus_transmittance(dir);
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
            storm * 0.45, cirrus_trans, cone_spread,
        );
    } else {
        low = CloudSample(vec3<f32>(0.0), 1.0, vec4<f32>(0.0));
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
        mid = CloudSample(vec3<f32>(0.0), 1.0, vec4<f32>(0.0));
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
        high = CloudSample(vec3<f32>(0.0), 1.0, vec4<f32>(0.0));
    }

    // ---- Depth Sort & Composite ----
    var layers: array<CloudSample, 3>;
    layers[0] = low;
    layers[1] = mid;
    layers[2] = high;
    
    var dists: array<f32, 3>;
    dists[0] = select(1e9, low_offset, low_ro.valid);
    dists[1] = select(1e9, mid_offset, mid_ro.valid);
    dists[2] = select(1e9, high_offset, high_ro.valid);

    for (var i = 0; i < 2; i = i + 1) {
        if dists[0] > dists[1] {
            let td = dists[0]; dists[0] = dists[1]; dists[1] = td;
            let tl = layers[0]; layers[0] = layers[1]; layers[1] = tl;
        }
        if dists[1] > dists[2] {
            let td = dists[1]; dists[1] = dists[2]; dists[2] = td;
            let tl = layers[1]; layers[1] = layers[2]; layers[2] = tl;
        }
    }

    let after_0_rgb   = layers[0].radiance + layers[0].transmittance * layers[1].radiance;
    let after_0_trans = layers[0].transmittance * layers[1].transmittance;
    let combined_rgb  = after_0_rgb   + after_0_trans * layers[2].radiance;
    let combined_trans= after_0_trans * layers[2].transmittance;
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

    let sky_col = atmosphere_color(dir);

    // ---- Aerial perspective ----
    // Use the global envelope slab intersection as a conservative distance
    // estimate for atmospheric haze. This is cheap and sufficient since
    // aerial perspective is a smooth low-frequency effect.
    let slab_low  = slab_intersection(cam.camera_pos.xyz, dir, bounds.low_base,  bounds.low_top);
    let slab_mid  = slab_intersection(cam.camera_pos.xyz, dir, bounds.mid_base,  bounds.mid_top);
    let slab_high = slab_intersection(cam.camera_pos.xyz, dir, bounds.high_base, bounds.high_top);

    let has_low  = slab_low.y  > 0.0 && (1.0 - low.transmittance)  > 0.001;
    let has_mid  = slab_mid.y  > 0.0 && (1.0 - mid.transmittance)  > 0.001;
    let has_high = slab_high.y > 0.0 && (1.0 - high.transmittance) > 0.001;

    let t_low  = select(1e9, max(slab_low.x,  0.0), has_low);
    let t_mid  = select(1e9, max(slab_mid.x,  0.0), has_mid);
    let t_high = select(1e9, max(slab_high.x, 0.0), has_high);
    let cloud_dist = min(min(t_low, t_mid), t_high);
    let ap_dist    = select(0.0, cloud_dist, cloud_dist < 1e8);

    let ap = aerial_perspective(dir, ap_dist, scale, sky_col);

    // Aerial Perspective adds in-scatter. Background also adds in-scatter.
    // Multiplying ap.rgb by combined_alpha preserves the exact atmospheric energy 
    // without double-darkening the cloud's extinction.
    var out_rgb = combined_rgb * ap.a + ap.rgb * combined_alpha;

    // ---- Interior cloud ambient overlay ----
    // When the camera is inside a cloud layer, sample density at the camera
    // position once and blend a milky-white fog term into the result.
    // This gives physical presence when flying through clouds.
    let inside_low  = camera_inside_layer(cam.camera_pos.y, bounds.low_base,  bounds.low_top);
    let inside_mid  = camera_inside_layer(cam.camera_pos.y, bounds.mid_base,  bounds.mid_top);
    let inside_high = camera_inside_layer(cam.camera_pos.y, bounds.high_base, bounds.high_top);

    if inside_low || inside_mid || inside_high {
        let i_wind     = safe_normalize2(light.weather_globals[0].zw, vec2<f32>(0.9, -0.4));
        let i_time     = light.weather_globals[0].x;
        let i_wind_spd = max(light.weather_globals[0].y, 0.05);

        var local_density = 0.0;

        if inside_low {
            let wx_off = i_wind * i_time * i_wind_spd * 40.0;
            let wx = weather_at_world(cam.camera_pos.xz + wx_off);
            let local_aabb = cell_aabb_for_layer(wx, LAYER_LOW);
            local_density = max(local_density,
                sample_low_density(cam.camera_pos.xyz,
                    local_aabb.y_min, local_aabb.y_max, cam.camera_pos.y,
                    1.0, 0.0, 0.0, false, 8.0, wx));
        }
        if inside_mid {
            let wx_off = i_wind * i_time * i_wind_spd * 33.25;
            let wx = weather_at_world(cam.camera_pos.xz + wx_off);
            let local_aabb = cell_aabb_for_layer(wx, LAYER_MID);
            local_density = max(local_density,
                sample_mid_density(cam.camera_pos.xyz,
                    local_aabb.y_min, local_aabb.y_max, cam.camera_pos.y,
                    8.0, wx));
        }
        if inside_high {
            let wx_off = i_wind * i_time * i_wind_spd * 60.0;
            let wx = weather_at_world(cam.camera_pos.xz + wx_off);
            let local_aabb = cell_aabb_for_layer(wx, LAYER_HIGH);
            local_density = max(local_density,
                sample_cirrus_density(cam.camera_pos.xyz,
                    local_aabb.y_min, local_aabb.y_max, cam.camera_pos.y,
                    8.0, wx));
        }

        if local_density > 0.001 {
            let sky_zenith  = light.sky_top_saturation.rgb;
            let sky_horizon = light.sky_horizon_reserved.rgb;
            let fog_col = mix(sky_horizon, sky_zenith, 0.5) * 1.1
                        + vec3<f32>(0.03, 0.03, 0.04);
            let milky = mix(fog_col, vec3<f32>(0.85, 0.88, 0.92), 0.65);
            let fog_alpha = smoothstep(0.0, 0.10, local_density) * 0.45;
            out_rgb = out_rgb * (1.0 - fog_alpha) + milky * fog_alpha;
            combined_alpha = combined_alpha + fog_alpha * (1.0 - combined_alpha);
        }
    }

    let weld = exp(-abs(dir.y) * 18.0)
             * smoothstep(light.fog_params.y * 0.70, light.fog_params.y * 1.10, ap_dist);
    out_rgb = mix(out_rgb, sky_col, weld * 0.10 * combined_alpha);

    return vec4<f32>(max(out_rgb, vec3<f32>(0.0)), combined_alpha);
}
