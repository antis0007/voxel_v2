// ============================================================
// world_shader.wgsl
//
// Voxel terrain lit pass: stable surface detail and radiometric composition.
//
// Integrate with the SAME shared WGSL declarations as the supplied shader.
// No new resources or vertex attributes. Internal varyings include interpolated
// natural-material coverage; material IDs are resolved only in the vertex stage.
// Camera depth remains reverse-Z; directional shadow maps remain forward-Z.
// Output stays linear HDR: the shared resolve owns exposure and tone mapping.
// This pass cannot repair missing residency, mismatched LOD topology, or caves
// that are absent from a heightfield. See INTEGRATION.txt for required contracts.
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
    @location(1)       relative_pos : vec3<f32>,
    @location(2)       voxel_pos  : vec3<f32>,
    @location(3)       normal     : vec3<f32>,
    @location(4)       surface    : vec4<f32>,
    @location(5)       ao_strength: f32,
    @location(6) material_tint: vec4<f32>,
    @location(7) material_props: vec4<f32>,
    @location(8) water_optics: vec2<f32>,
    @location(9) terrain_weights: vec4<f32>,
    @location(10) @interpolate(flat) material_family: u32,
    @location(11) gravel_coverage: f32,
    @location(12) model_uv: vec2<f32>,
};

fn terrain_safe_unit3(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let l2 = dot(v, v);
    return select(fallback, v * inverseSqrt(max(l2, 1e-20)), l2 > 1e-20);
}

fn terrain_safe_unit2(v: vec2<f32>, fallback: vec2<f32>) -> vec2<f32> {
    let l2 = dot(v, v);
    return select(fallback, v * inverseSqrt(max(l2, 1e-20)), l2 > 1e-20);
}

fn terrain_pow5(x: f32) -> f32 {
    let x2 = x * x;
    return x2 * x2 * x;
}

/// Ordered 4x4 Bayer threshold in [0,1). World-anchored rather than
/// screen-anchored, so pixel dithering does not crawl when the camera moves.
fn terrain_bayer4(cell: vec2<f32>) -> f32 {
    let x = i32(cell.x) & 3;
    let y = i32(cell.y) & 3;
    let b = array<f32, 16>(
        0.0, 8.0, 2.0, 10.0,
        12.0, 4.0, 14.0, 6.0,
        3.0, 11.0, 1.0, 9.0,
        15.0, 7.0, 13.0, 5.0,
    );
    return b[y * 4 + x] / 16.0;
}

// The one texel density of the world: 20 texels per metre (5 cm pixels).
// Every procedural lattice, hash cell, dither and noise lookup in this file is
// evaluated on this grid, so every pixel of every material is the same size.
// Do not introduce another cells-per-metre constant or a `* 0.5` multi-pixel
// cell: mixed lattices (7, 10, 12, 14, 24 per metre, 2x2 blobs) are what made
// some blocks look like large chunky tiles beside finely grained neighbours.
const PIXEL_RES: f32 = 20.0;
// Crystal facets span this many texels per side: a deliberate wider feature on
// the shared lattice, not a second grid.
const CRYSTAL_FACET_TEXELS: f32 = 8.0;

fn texel_centre(uv: vec2<f32>) -> vec2<f32> {
    return (floor(uv * PIXEL_RES) + vec2<f32>(0.5)) / PIXEL_RES;
}

fn terrain_pixel_uv(p: vec3<f32>, n: vec3<f32>) -> vec2<f32> {
    let a = abs(n);
    if a.y >= a.x && a.y >= a.z { return p.xz; }
    if a.x >= a.z { return p.zy; }
    return p.xy;
}

/// Pixel-art texture without posterization. Keeps a 20-texel/m lattice (5 cm
/// pixels) close to the camera and applies a faint ordered-dither grain on it;
/// tone itself is never quantised, so lighting gradients stay smooth.
fn terrain_pixel_albedo(color: vec3<f32>, p: vec3<f32>, n: vec3<f32>, footprint_m: f32) -> vec3<f32> {
    let uv = terrain_pixel_uv(p, n) * PIXEL_RES;
    let cell = floor(uv);
    // Fade in texels per screen pixel, like every other 5 cm lattice term.
    // The old 0.22-0.65 m (4-13 texels per pixel) window kept the 4x4 dither
    // alive far past Nyquist, where it aliased into broad moire bands on
    // flat canopy and wall faces.
    let mid = surface_detail_weight(footprint_m * PIXEL_RES);
    // No palette quantisation: tone stays continuous so gradients never band.
    // A small ordered-dither lift on the 5 cm lattice keeps the pixel-art
    // texture up close. Multiplicative gain preserves hue.
    let grain = (terrain_bayer4(cell) - 0.5) * 0.07 * mid;
    return color * (1.0 + grain);
}

fn terrain_srgb_to_linear(value: vec3<f32>) -> vec3<f32> {
    let c = clamp(value, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4)),
        c / 12.92, c <= vec3<f32>(0.04045));
}

// Largest singular value of the screen-to-surface Jacobian. Taking only the
// longer derivative misses diagonal stretch by up to sqrt(2), causing aliasing.
fn terrain_pixel_footprint(dx: vec3<f32>, dy: vec3<f32>) -> f32 {
    let a = dot(dx, dx);
    let b = dot(dx, dy);
    let c = dot(dy, dy);
    let discriminant = sqrt(max((a - c) * (a - c) + 4.0 * b * b, 0.0));
    return sqrt(max(0.5 * (a + c + discriminant), 1e-12));
}

fn terrain_is_backdrop(ao: f32) -> bool {
    return ao < 0.0 && ao > -1.5;
}

// Distant voxel columns (render/horizon_columns.rs) tag their vertices with
// ao -1.25, still inside the backdrop range. Every other backdrop mesh uses -1.
fn terrain_is_column(ao: f32) -> bool {
    return ao < -1.125 && ao > -1.375;
}

// Must match the 64 m band in horizon_columns.rs `BANDS`.
const HORIZON_COLUMN_M: f32 = 64.0;

// Per-column value step for distant voxel columns. Natural backdrop materials
// replace vertex colour with their own palette, so a long coast of equal
// columns shaded as one flat stripe. Each column takes one of five value
// steps (a pixel-art ramp, not continuous noise), keyed by its world column
// index. Walls sit on column boundaries, so the sample steps half a metre
// back along the normal into the column that owns the face.
fn horizon_column_value(metres: vec3<f32>, normal: vec3<f32>) -> f32 {
    let inside = metres - normal * 0.5;
    let cell = floor(inside.xz / HORIZON_COLUMN_M);
    let step_index = floor(surface_hash(cell + vec2<f32>(41.0, 113.0)) * 5.0);
    return 1.0 + (min(step_index, 4.0) - 2.0) * 0.045;
}

// Exact edit proxies retain resident optics and AO: tag = -2 - source AO.
// In particular a water surface rim (source AO 1.5) must remain distinguishable.
fn terrain_has_resident_metadata(ao: f32) -> bool {
    return ao >= 0.0 || ao < -1.5;
}

fn terrain_resident_column_owned(world_xz: vec2<f32>, horizontal_distance: f32) -> bool {
    let header = cam.resident_coverage.header;
    if header.w == 0 { return horizontal_distance < max(cam.coverage.w, 0.0); }
    if header.z <= 0 || header.z > 129 { return false; }
    let column = vec2<i32>(floor(world_xz / (32.0 * max(cam.world_params.x, 1e-6))));
    // Keep normal nearby pixels on the dense path, including unowned bits.
    if all(column >= header.xy) {
        let relative = bitcast<vec2<u32>>(column) - bitcast<vec2<u32>>(header.xy);
        if all(relative < vec2<u32>(u32(header.z))) {
            let bit = relative.y * u32(header.z) + relative.x;
            let word = bit >> 5u;
            return (cam.resident_coverage.words[word >> 2u][word & 3u] & (1u << (bit & 31u))) != 0u;
        }
    }
    return terrain_sparse_resident_owned(column);
}

fn terrain_sparse_resident_owned(column: vec2<i32>) -> bool {
    let info = cam.resident_coverage.sparse_header;
    if info.x <= 0 || info.x > 512 { return false; }
    let empty = vec2<i32>(bitcast<i32>(0x80000000u));
    if all(column == empty) { return info.y == 1; }
    let bits = bitcast<vec2<u32>>(column);
    let mixed = (bits.x * 0x9e3779b1u) ^ (bits.y * 0x85ebca77u);
    var slot = (mixed ^ (mixed >> 16u)) & 1023u;
    for (var probe = 0u; probe < 1024u; probe += 1u) {
        let packed = cam.resident_coverage.sparse_keys[slot >> 1u];
        let key = select(packed.xy, packed.zw, (slot & 1u) != 0u);
        if all(key == empty) { return false; }
        if all(key == column) { return true; }
        slot = (slot + 1u) & 1023u;
    }
    return false;
}

fn terrain_water_ownership_weight(world_xz: vec2<f32>, scale: f32) -> f32 {
    if cam.resident_coverage.header.w == 0 { return 1.0; }
    if !terrain_resident_column_owned(world_xz, 0.0) { return 0.0; }
    let span = 32.0 * scale;
    let cell = floor(world_xz / span);
    let width = max(2.0 * scale, 0.24);
    var distance_squared = width * width;
    // Include diagonal holes: four owned sides do not imply an owned corner.
    // Beyond this neighbourhood every boundary is farther than the blend width.
    for (var z = -1; z <= 1; z += 1) {
        for (var x = -1; x <= 1; x += 1) {
            if x == 0 && z == 0 { continue; }
            let low = (cell + vec2<f32>(f32(x), f32(z))) * span;
            if !terrain_resident_column_owned(low + vec2<f32>(span * 0.5), 0.0) {
                let delta = max(max(low - world_xz, world_xz - (low + vec2<f32>(span))), vec2<f32>(0.0));
                distance_squared = min(distance_squared, dot(delta, delta));
            }
        }
    }
    return smoothstep(0.0, width, sqrt(distance_squared));
}

fn terrain_vertex_ao(encoded: f32) -> f32 {
    let decoded = select(encoded, -2.0 - encoded, encoded < -1.5);
    return select(clamp(decoded, 0.0, 1.0), 1.0, terrain_is_backdrop(encoded));
}

fn terrain_backdrop_start() -> f32 {
    let visible = max(cam.world_params.w, 0.0);
    let published = cam.coverage.x;
    if (published >= 0.0) {
        // Never hide backdrop farther than the resident meshes that actually
        // exist. This closes the gap between a lagging stream and the horizon.
        return min(published, visible);
    }
    return visible * 0.55;
}

fn terrain_overlap_width() -> f32 {
    return max(120.0 * max(cam.world_params.x, 1e-6), cam.world_params.w * 0.06);
}

fn terrain_coverage_for_family(family: u32) -> vec4<f32> {
    return vec4<f32>(
        select(0.0, 1.0, family == 0u || family == 1u || family == 7u),
        select(0.0, 1.0, family == 2u),
        select(0.0, 1.0, family == 3u),
        select(0.0, 1.0, family == 4u));
}

fn gerstner_vertex_point(wxz: vec2<f32>, t: f32) -> vec3<f32> {
    let d0 = vec2<f32>(0.88, 0.47);
    let d1 = vec2<f32>(0.52, -0.85);
    let d2 = vec2<f32>(-0.74, 0.67);
    let d3 = vec2<f32>(-0.31, -0.95);

    let ph0 = dot(wxz, d0) * 0.26 - t * 1.45;
    let ph1 = dot(wxz, d1) * 0.43 - t * 1.88 + 1.7;
    let ph2 = dot(wxz, d2) * 0.68 - t * 2.35 + 3.1;
    let ph3 = dot(wxz, d3) * 1.05 - t * 2.90 + 0.8;

    let a0 = 0.057;
    let a1 = 0.034;
    let a2 = 0.019;
    let a3 = 0.010;

    let dy = cos(ph0) * a0 + cos(ph1) * a1 + cos(ph2) * a2 + cos(ph3) * a3;
    let dxz = -d0 * (sin(ph0) * a0 * 0.45)
              - d1 * (sin(ph1) * a1 * 0.42)
              - d2 * (sin(ph2) * a2 * 0.35)
              - d3 * (sin(ph3) * a3 * 0.30);
    return vec3<f32>(dxz.x, dy, dxz.y);
}

// Sun caustics at a point on the bed: light focused by the waves above it. Two
// drifting noise layers cross in bright, thin lines (the web) and a second,
// finer pair adds filaments. Continuous and single channel: the old per-channel
// offsets fringed every line with rainbow colour. The bed point already carries
// the refraction parallax, so the web slides correctly as the view moves.
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

// Mirrors `CAMERA_RELATIVE_FLAG` in mesh/models.rs: 64 * 4096, above any block key.
const CAMERA_RELATIVE_FLAG: f32 = 262144.0;

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let scale     = max(cam.world_params.x, 1e-6);
    // Viewmodel vertices carry CAMERA_RELATIVE_FLAG on top of kind + 64 * block.
    let camera_relative = in.surface.z >= CAMERA_RELATIVE_FLAG;
    let surface_key = select(in.surface.z, in.surface.z - CAMERA_RELATIVE_FLAG, camera_relative);
    let geometry_kind=f32(u32(round(max(surface_key,0.0)))%64u);
    var model_pos = in.pos;
    if geometry_kind == 34.0 {
        model_pos = tuft_wind_position(in.pos, in.surface.y, scale, light.torch_meta.y,
            light.weather_globals[0].zw * light.weather_globals[0].y);
    }
    // Embers carry surface.w = -4; flame sway must not move them.
    let is_spark = in.surface.w < -3.5 && in.surface.w > -4.5;
    if in.surface.w < -1.5 && !is_spark && geometry_kind != 34.0 {
        // The base stays on its mount and the tip moves most. The wave travels
        // up the flame (the tip lags the base) so it licks rather than wobbling
        // as one block. The phase is continuous in position: the old floor()
        // seed moved each voxel as a cube.
        let lift = flame_lift(in.surface.w);
        let sway = lift * lift;
        let phase = dot(in.pos, vec3<f32>(0.73, 1.17, 0.41));
        let flame_time = light.torch_meta.y;
        let wave = sin(flame_time * 6.7 - lift * 2.6 + phase);
        let flutter = sin(flame_time * 10.9 - lift * 3.4 + phase * 1.7);
        // Tip travel is about 3 cm on a 35 cm flame: visible at 1280 px width.
        model_pos.x += (0.034 * wave + 0.012 * flutter) * sway / scale;
        model_pos.z += (0.022 * sin(flame_time * 5.3 - lift * 2.0 + phase * 0.8)
            + 0.008 * flutter) * sway / scale;
        // The flame shortens a little on each flick.
        model_pos.y -= 0.012 * sway * (0.5 + 0.5 * wave) / scale;
    }
    if is_spark {
        // An ember is a crossed pair of quads. Its corner comes from ao_strength
        // ((quad * 4 + corner + 1) / 8) and its seed from surface.x. It rises
        // over its life, drifts, and grows then shrinks to nothing at both ends,
        // so each spark fades by shrinking its own geometry, never by a grid test.
        let corner_id = round(in.ao_strength * 8.0) - 1.0;
        let quad_kind = select(0.0, 1.0, corner_id >= 4.0);
        let corner = corner_id - quad_kind * 4.0;
        let cx = select(-1.0, 1.0, corner == 1.0 || corner == 2.0);
        let cy = select(-1.0, 1.0, corner >= 2.0);
        let half = 0.008 / scale;
        let rest = select(
            vec3<f32>(cx * half, cy * half, 0.0),
            vec3<f32>(0.0, cy * half, cx * half),
            quad_kind > 0.5,
        );
        let seed = in.surface.x;
        let flame_time = light.torch_meta.y;
        let life = fract(flame_time * (0.35 + 0.25 * seed) + seed * 3.0);
        let grow = smoothstep(0.0, 0.2, life) * (1.0 - smoothstep(0.55, 1.0, life));
        let drift = vec3<f32>(sin(flame_time * 1.7 + seed * 17.0), 0.0, cos(flame_time * 1.3 + seed * 11.0))
            * (0.02 * life);
        // Spawned near the tip, so the rise carries the ember out of the flame.
        let rise = vec3<f32>(0.0, 0.24 * life, 0.0);
        model_pos = in.pos + (drift + rise) / scale + rest * (grow - 1.0);
    }
    out.water_optics = vec2<f32>(10.0, 1.0);
    // surface.w = 8 + k + sky/2, k = depth*4 + 256*pinned (see mesh/memory.rs).
    // A pinned corner touches a non-water cell: bank, wall or flow step.
    var water_pinned = 0.0;
    if geometry_kind == 8.0 && in.surface.w >= 8.0 && terrain_has_resident_metadata(in.ao_strength) {
        let k = floor(in.surface.w - 8.0);
        water_pinned = floor(k / 256.0);
        out.water_optics = vec2<f32>((k - 256.0 * water_pinned) * 0.25,
            clamp(fract(in.surface.w)*2.0,0.006,1.0));
    }
    var world_pos = model_pos * scale;
    let horiz_dist = length((world_pos - cam.camera_pos.xyz).xz);
    // A side wall's surface-level vertices ride the same waves as the top face
    // they share a corner with, so the two never split apart. The mesher flags
    // them (ao_strength 1.5). A y-based test cannot tell a full cell's top edge
    // (integer y) from its dry floor edge, which left wall tops flat under moving water.
    let water_surface_side = geometry_kind == 8.0 && abs(in.normal.y) < 0.5
        && (in.ao_strength > 1.25 || in.ao_strength < -3.25);
    if geometry_kind == 8.0 && (in.normal.y > 0.5 || water_surface_side) && terrain_has_resident_metadata(in.ao_strength) {
        // Linear shoaling: wave height grows as the bed shallows, then the
        // displacement is damped in the last few centimetres of run-up so the
        // surface does not intersect the beach. The old gate did the opposite
        // and flattened every wave at exactly the surf zone.
        let depth = out.water_optics.x;
        let shoal = 1.0 + 1.35 * exp(-depth * 1.15) * smoothstep(0.015, 0.55, depth);
        // Open water saturates at 0.6 m, so merged 2x2 quads (always deeper than
        // three voxels) share one amplitude and their straight edges stay exact.
        let surf_fade = smoothstep(0.02, 0.60, depth);
        // Retain the range filter for subpixel displacement. Actual ownership
        // boundaries are handled independently below, including irregular holes.
        let lod_radius = select(cam.coverage.z, cam.world_params.w, cam.coverage.z <= 0.0);
        let dist_atten = (1.0 - smoothstep(96.0 * scale, 240.0 * scale, horiz_dist))
            * (1.0 - smoothstep(0.35 * lod_radius, 0.85 * lod_radius, horiz_dist));
        // Corners that touch a bank, wall or step stay put in all three axes, so
        // the surface meets its neighbours exactly instead of sliding off them.
        let pin = 1.0 - water_pinned;
        if dist_atten > 0.0 && pin > 0.0 && surf_fade > 0.0 {
            let owner_weight = terrain_water_ownership_weight(world_pos.xz, scale);
            if owner_weight > 0.0 {
                let wave_disp = eval_water_vertex_wave(model_pos.xz, scale, light.torch_meta.y);
                // Wave displacement is already in metres. Only its amplitude
                // fades to zero at the shared source surface; no terrain sink.
                world_pos += wave_disp * (shoal * surf_fade * dist_atten * pin * owner_weight);
                let along = dot(model_pos.xz * scale, vec2<f32>(0.54, -0.84));
                let breaker_set = pow(max(sin(along * 0.12 + light.torch_meta.y * 0.72), 0.0), 4.0);
                let breaker_band = (1.0 - smoothstep(0.05, 0.85, depth)) * surf_fade;
                world_pos.y += breaker_set * breaker_band * 0.14 * dist_atten * pin * owner_weight;
            }
        }
    }
    var backdrop_depth_bias = 0.0;
    if terrain_is_backdrop(in.ao_strength) && cam.resident_coverage.header.w == 0 {
        // Same window as the fragment handoff. Bias peaks in the overlap and
        // reaches zero at the outer edge so the surfaces meet without a step.
        let backdrop_start = terrain_backdrop_start();
        let seam = terrain_overlap_width();
        // The backdrop is fragment-discarded inside `backdrop_start`, so the band where
        // it can actually overlap resident voxels is [start, start + seam]. Hold the
        // full sink across the first part of that band, then release it to zero where
        // resident chunks end.
        let underlap = 1.0 - smoothstep(backdrop_start + 0.4 * seam, backdrop_start + seam, horiz_dist);
        // A linear heightfield over-estimates concave, stepped voxel terrain by roughly
        // half a ring cell times the slope, so sink steep ground further. Ring spacing
        // grows with distance (cells span ~1/96 of the ring half-width).
        let ring_cell = max(4.0 * scale, horiz_dist / 96.0);
        let ny = clamp(terrain_safe_unit3(in.normal, vec3<f32>(0.0, 1.0, 0.0)).y, 0.25, 1.0);
        let slope = sqrt(max(1.0 - ny * ny, 0.0)) / ny;
        let relief_sink = clamp(0.5 * ring_cell * slope, 0.0, 3.0 * scale);
        // A compatibility underlap, not a substitute for stitched/morphed meshes.
        // Never lower water, trees, or exact edited fallback geometry.
        if geometry_kind == 0.0 {
            world_pos.y -= underlap * (0.45 * scale + relief_sink);
        }
        // Scale bias by the surface's own reverse-Z depth so perspective
        // hyperbolic depth never saturates or flattens across the overlap zone.
        // Backdrop water sits in the same plane as resident water but carries no
        // waves, so resident troughs dipped below it and let its different colour
        // show through as streaks. Push it back too, less than the terrain it
        // overlies so water still lies over the seabed.
        backdrop_depth_bias = underlap * select(select(0.0, 0.002, geometry_kind == 8.0), 0.0035, geometry_kind == 0.0);
    }
    // Viewmodel vertices (CAMERA_RELATIVE_FLAG) are already camera-relative; going
    // through absolute world space would round them to the f32 grid of the
    // world position.
    let vertex_relative = select(world_pos - cam.camera_pos.xyz, world_pos, camera_relative);
    out.pos        = cam.view_proj * vec4<f32>(vertex_relative, 1.0);
    // Separate exact fallback/resident depth without splitting shared world edges.
    // Per-face normal insets opened sky pinholes on fine voxel silhouettes.
    // Depth is reverse-Z (1 near, 0 far): pushing a surface away scales its depth
    // toward zero by a fraction of its own value, the mirror of the old forward-Z
    // `z += (w - z) * bias`.
    if in.ao_strength < -1.5 && cam.resident_coverage.header.w == 0 {
        out.pos.z -= max(out.pos.z, 0.0) * 0.00015;
    }
    if backdrop_depth_bias > 0.0 {
        out.pos.z -= max(out.pos.z, 0.0) * backdrop_depth_bias;
    }
    // The held hand and wand share the world depth buffer, so pressing against
    // a wall used to slice them. Reverse-Z depth is z/w with 1 at the camera;
    // moving it halfway toward 1 is monotonic, so the pieces still sort against
    // each other, but they now win the depth test against anything beyond a few
    // centimetres of the lens.
    if geometry_kind == 54.0 || geometry_kind == 55.0 {
        out.pos.z += (out.pos.w - out.pos.z) * 0.5;
    }
    // Decode BEFORE interpolation, so gradients and biome averages are linear.
    out.base_color = terrain_srgb_to_linear(in.color);
    out.relative_pos = vertex_relative;
    out.voxel_pos  = in.pos;
    out.normal     = terrain_safe_unit3(in.normal, vec3<f32>(0.0, 1.0, 0.0));
    out.surface    = in.surface;
    out.surface.z = geometry_kind;
    out.material_family = u32(geometry_kind);
    out.ao_strength = in.ao_strength;
    // Resolve IDs at vertices; interpolate colors/finish, never material IDs.
    let far_terrain=in.ao_strength<0.0 && in.ao_strength> -1.5 && surface_key==0.0;
    let key=u32(round(max(select(surface_key,in.surface.w,far_terrain),0.0)));
    // The capped face is a grass renderer variant; preserve its configured
    // tint/response and stable block key while the draw diagnostic reports 36.
    let family=select(key%64u, 1u, geometry_kind == 36.0);
    let block_key=key/64u;
    out.terrain_weights = terrain_coverage_for_family(family);
    out.gravel_coverage = select(0.0, 1.0, family == 33u);
    out.model_uv = select(vec2<f32>(0.0), in.surface.xy, family == 57u || family == 58u || family == 59u || family == 60u || family == 62u || family == 63u);
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
    // Palette reflectance is the dielectric parameter (0.5 -> F0 0.04).
    // An unset palette stores -1 and the mesh channel is the previous
    // empirical specular gain. 0.04, the old fixed dielectric Fresnel,
    // maps to reflectance 0.5; larger authored gains stay ordered.
    // Orbital meshes reserve surface.y > 1.5 for distant-object coverage.
    let orbital_mesh = (in.surface.y == 2.0 || in.surface.y == 3.0) && (geometry_kind == 40.0 || geometry_kind == 41.0 || geometry_kind == 61.0);
    let specular_channel = select(in.surface.y, 0.42, orbital_mesh);
    let mesh_specular = select(clamp(specular_channel, 0.0, 1.0), 0.0, geometry_kind == 34.0);
    let mesh_reflectance = clamp(0.5 * sqrt(mesh_specular / 0.04), 0.0, 1.0);
    // Stone/basalt's mesh gains predate physical reflectance. Unset rock uses
    // the studio's dielectric default; resolved family/individual overrides win.
    let default_reflectance = select(mesh_reflectance, 0.5, family == 3u);
    properties.x=select(default_reflectance, properties.x, properties.x>=0.0);
    out.material_tint=tint;
    out.material_props=properties;
    return out;
}

// ---- BRDF helpers ----

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

// Water sun-glint lobe, restored from main. GGX NDF, Schlick-GGX geometry and a
// Schlick Fresnel, returned as D*G*F / (4 N.V) with no N.L. The 0.105 and 0.045
// glint weights below were tuned against this scale, so do not swap in
// terrain_specular, which includes N.L and F0.
fn water_glint_lobe(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let half_vector = v + l;
    let h = half_vector / max(length(half_vector), 1e-4);
    let ndh = clamp(dot(n, h), 0.0, 1.0);
    let ndv = clamp(dot(n, v), 0.001, 1.0);
    let ndl = clamp(dot(n, l), 0.0, 1.0);
    // Roundoff must not feed a negative base into WGSL pow for Fresnel.
    let vdh = clamp(dot(v, h), 0.0, 1.0);

    let a = roughness * roughness;
    let a2 = a * a;
    let d_denom = ndh * ndh * (a2 - 1.0) + 1.0;
    let d = a2 / max(3.14159265 * d_denom * d_denom, 1e-7);

    let k = (roughness + 1.0) * (roughness + 1.0) / 8.0;
    let g1_v = ndv / (ndv * (1.0 - k) + k);
    let g1_l = ndl / (ndl * (1.0 - k) + k);
    let g = g1_v * g1_l;

    let f = 0.04 + 0.96 * pow(1.0 - vdh, 5.0);
    return d * g * f / max(4.0 * ndv, 1e-4);
}

// GGX with height-correlated Smith visibility. Perceptual roughness r maps to
// alpha=r^2; F0 is a linear RGB normal-incidence reflectance. The return value
// includes N.L, so callers must NOT divide/multiply by another incident cosine.
fn terrain_fresnel(f0: vec3<f32>, cos_theta: f32) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * terrain_pow5(1.0 - clamp(cos_theta, 0.0, 1.0));
}

fn terrain_specular(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>,
    roughness: f32, f0: vec3<f32>) -> vec3<f32> {
    let no_l = clamp(dot(n, l), 0.0, 1.0);
    let no_v = clamp(dot(n, v), 0.0, 1.0);
    let sum_dir = v + l;
    if no_l <= 0.0 || no_v <= 0.0 || dot(sum_dir, sum_dir) < 1e-12 {
        return vec3<f32>(0.0);
    }
    let h = terrain_safe_unit3(sum_dir, n);
    let no_h = clamp(dot(n, h), 0.0, 1.0);
    let alpha = pow(clamp(roughness, 0.06, 1.0), 2.0);
    let alpha2 = alpha * alpha;
    // Cross product avoids catastrophic cancellation in 1 - (N.H)^2.
    let nxh = cross(n, h);
    let d = dot(nxh, nxh) + no_h * no_h * alpha2;
    let distribution = alpha2 / max(3.14159265359 * d * d, 1e-12);
    let lambda_v = no_l * sqrt(max(no_v * no_v * (1.0 - alpha2) + alpha2, 0.0));
    let lambda_l = no_v * sqrt(max(no_l * no_l * (1.0 - alpha2) + alpha2, 0.0));
    let visibility = 0.5 / max(lambda_v + lambda_l, 1e-6);
    return terrain_fresnel(f0, dot(v, h)) * (distribution * visibility * no_l);
}

// PILOT: unresolved voxel stairs; macro normals do not encode exact visibility.
fn terrain_axis_facet_specular(macro_n: vec3<f32>, v: vec3<f32>, l: vec3<f32>,
    roughness: f32, f0: vec3<f32>) -> vec3<f32> {
    if !all(abs(macro_n) < vec3<f32>(1e10)) || macro_n.y < 0.0 || dot(macro_n, macro_n) < 1e-12 {
        return terrain_specular(vec3<f32>(0.0, 1.0, 0.0), v, l, roughness, f0);
    }
    let m = terrain_safe_unit3(macro_n, vec3<f32>(0.0, 1.0, 0.0));
    let areas = abs(m);
    var response = vec3<f32>(0.0);
    var projected_area = 0.0;
    let face_x = vec3<f32>(select(-1.0, 1.0, m.x >= 0.0), 0.0, 0.0);
    let weight_x = areas.x * max(dot(face_x, v), 0.0);
    if weight_x > 0.0 {
        projected_area += weight_x;
        if dot(face_x, l) > 0.0 {
            response += weight_x * terrain_specular(face_x, v, l, roughness, f0);
        }
    }
    let face_y = vec3<f32>(0.0, 1.0, 0.0);
    let weight_y = areas.y * max(dot(face_y, v), 0.0);
    if weight_y > 0.0 {
        projected_area += weight_y;
        if dot(face_y, l) > 0.0 {
            response += weight_y * terrain_specular(face_y, v, l, roughness, f0);
        }
    }
    let face_z = vec3<f32>(0.0, 0.0, select(-1.0, 1.0, m.z >= 0.0));
    let weight_z = areas.z * max(dot(face_z, v), 0.0);
    if weight_z > 0.0 {
        projected_area += weight_z;
        if dot(face_z, l) > 0.0 {
            response += weight_z * terrain_specular(face_z, v, l, roughness, f0);
        }
    }
    if projected_area <= 1e-12 { return vec3<f32>(0.0); }
    return response / projected_area;
}

// Shared pilot gate: only continuous ROCK coverage on procedural heightfields.
fn terrain_rock_facet_coverage(ao: f32, kind: i32, rock_coverage: f32) -> f32 {
    if !terrain_is_backdrop(ao) || kind != 0 || !(rock_coverage > 0.0) { return 0.0; }
    return clamp(rock_coverage, 0.0, 1.0);
}

fn terrain_directional_specular(n: vec3<f32>, macro_n: vec3<f32>,
    v: vec3<f32>, l: vec3<f32>, roughness: f32, f0: vec3<f32>,
    ao: f32, kind: i32, rock_coverage: f32) -> vec3<f32> {
    let coverage = terrain_rock_facet_coverage(ao, kind, rock_coverage);
    if coverage <= 0.0 { return terrain_specular(n, v, l, roughness, f0); }
    let facets = terrain_axis_facet_specular(macro_n, v, l, roughness, f0);
    if coverage >= 1.0 { return facets; }
    return mix(terrain_specular(n, v, l, roughness, f0), facets, coverage);
}

// Preserve the existing directional Lambert/Oren blend, including its roughness weight.
fn terrain_diffuse_lobe(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>,
    roughness: f32, oren_weight: f32) -> f32 {
    return mix(max(dot(n, l), 0.0), oren_nayar(n, v, l, roughness), roughness * oren_weight);
}

fn terrain_axis_facet_diffuse(macro_n: vec3<f32>, v: vec3<f32>, l: vec3<f32>,
    roughness: f32, oren_weight: f32) -> f32 {
    if !all(abs(macro_n) < vec3<f32>(1e10)) || macro_n.y < 0.0 || dot(macro_n, macro_n) < 1e-12 {
        return terrain_diffuse_lobe(vec3<f32>(0.0, 1.0, 0.0), v, l, roughness, oren_weight);
    }
    let m = terrain_safe_unit3(macro_n, vec3<f32>(0.0, 1.0, 0.0));
    let areas = abs(m);
    var response = 0.0;
    var projected_area = 0.0;
    let face_x = vec3<f32>(select(-1.0, 1.0, m.x >= 0.0), 0.0, 0.0);
    let weight_x = areas.x * max(dot(face_x, v), 0.0);
    if weight_x > 0.0 {
        projected_area += weight_x;
        if dot(face_x, l) > 0.0 {
            response += weight_x * terrain_diffuse_lobe(face_x, v, l, roughness, oren_weight);
        }
    }
    let face_y = vec3<f32>(0.0, 1.0, 0.0);
    let weight_y = areas.y * max(dot(face_y, v), 0.0);
    if weight_y > 0.0 {
        projected_area += weight_y;
        if dot(face_y, l) > 0.0 {
            response += weight_y * terrain_diffuse_lobe(face_y, v, l, roughness, oren_weight);
        }
    }
    let face_z = vec3<f32>(0.0, 0.0, select(-1.0, 1.0, m.z >= 0.0));
    let weight_z = areas.z * max(dot(face_z, v), 0.0);
    if weight_z > 0.0 {
        projected_area += weight_z;
        if dot(face_z, l) > 0.0 {
            response += weight_z * terrain_diffuse_lobe(face_z, v, l, roughness, oren_weight);
        }
    }
    if projected_area <= 1e-12 { return 0.0; }
    return response / projected_area;
}

fn terrain_directional_diffuse(n: vec3<f32>, macro_n: vec3<f32>,
    v: vec3<f32>, l: vec3<f32>, roughness: f32, oren_weight: f32,
    ao: f32, kind: i32, rock_coverage: f32) -> f32 {
    let coverage = terrain_rock_facet_coverage(ao, kind, rock_coverage);
    if coverage <= 0.0 { return terrain_diffuse_lobe(n, v, l, roughness, oren_weight); }
    let facets = terrain_axis_facet_diffuse(macro_n, v, l, roughness, oren_weight);
    if coverage >= 1.0 { return facets; }
    return mix(terrain_diffuse_lobe(n, v, l, roughness, oren_weight), facets, coverage);
}

// Conservative geometric specular AA, operating on alpha^2 = r^4.
fn terrain_filter_roughness(r: f32, variance: f32) -> f32 {
    let roughness = clamp(r, 0.06, 1.0);
    let kernel = min(2.0 * max(variance, 0.0), 0.08);
    return sqrt(sqrt(clamp(pow(roughness, 4.0) + kernel, 0.00001296, 1.0)));
}

// ---- Shadow ----

// Receiver-plane depth derivatives for directional, orthographic cascades.
// Contract: light_view_proj has a constant homogeneous W of one, as in the
// original renderer. A perspective light needs quotient-rule derivatives at
// the receiver position; this interface does not carry that position.
fn shadow_receiver_gradient(matrix: mat4x4<f32>, world_dx: vec3<f32>, world_dy: vec3<f32>) -> vec2<f32> {
    let dx = (matrix * vec4<f32>(world_dx, 0.0)).xyz;
    let dy = (matrix * vec4<f32>(world_dy, 0.0)).xyz;
    let uv_dx = dx.xy * vec2<f32>(0.5, -0.5);
    let uv_dy = dy.xy * vec2<f32>(0.5, -0.5);
    let determinant = uv_dx.x * uv_dy.y - uv_dx.y * uv_dy.x;
    let area_scale = length(uv_dx) * length(uv_dy);
    // Test conditioning relative to the derivative scale. An absolute
    // determinant threshold changes behaviour with cascade resolution.
    if area_scale <= 1e-20 || abs(determinant) <= area_scale * 1e-4 {
        return vec2<f32>(0.0);
    }
    let gradient = vec2<f32>(
        uv_dy.y * dx.z - uv_dx.y * dy.z,
        uv_dx.x * dy.z - uv_dy.x * dx.z,
    ) / determinant;
    // Gradually stop trusting an almost edge-on plane instead of producing
    // a discontinuous, arbitrarily large correction at the inversion limit.
    let confidence = smoothstep(1e-4, 1e-2, abs(determinant) / area_scale);
    return gradient * confidence;
}

fn pcf_shadow(uv: vec2<f32>, proj_z: f32, bias: f32, radius: f32, cascade_idx: i32, gradient: vec2<f32>) -> f32 {
    // A rotated twelve-tap golden-angle disk. The old axis-aligned 3×3 kernel
    // left the cascade texel grid visible as stair-stepped shadow edges; this
    // distributes the same small penumbra over a disk and breaks the grid
    // direction per pixel. Explicit-level comparison is valid in this
    // fragment-dependent cascade branch.
    let max_correction = radius * (abs(gradient.x) + abs(gradient.y));
    let g = gradient * min(1.0, 0.02 / max(max_correction, 1e-8));
    let z = proj_z - bias;
    // Integer PCG hash of the receiver's shadow-map coordinate. The old
    // fract(sin(large)) hash loses precision at these magnitudes on some GPUs
    // and degrades into visible banding; integer math is exact everywhere.
    // It depends only on the world-stable uv, so the jitter does not crawl.
    var h = bitcast<u32>(uv.x * 4096.0) * 747796405u
        + bitcast<u32>(uv.y * 4096.0) * 2891336453u + 2891336453u;
    h = ((h >> ((h >> 28u) + 4u)) ^ h) * 277803737u;
    h = (h >> 22u) ^ h;
    let rotation = f32(h) * (6.2831853 / 4294967296.0);
    let center = textureSampleCompareLevel(
        shadow_map, shadow_sampler, uv, cascade_idx, clamp(z, 0.0, 1.0));
    // Most pixels are wholly lit or wholly shadowed. Probe the kernel's rim
    // with four taps first and skip the other twelve when they all agree. The
    // map is texel-discrete, so no feature smaller than the rim spacing exists
    // to be missed.
    var probe = center;
    for (var k = 0; k < 4; k = k + 1) {
        let a = rotation + f32(k) * 1.57079633;
        let rim = vec2<f32>(cos(a), sin(a)) * (radius * 0.98);
        probe += textureSampleCompareLevel(
            shadow_map, shadow_sampler, uv + rim, cascade_idx,
            clamp(z + g.x * rim.x + g.y * rim.y, 0.0, 1.0));
    }
    if probe <= 0.0001 { return 0.0; }
    if probe >= 4.9999 { return 1.0; }
    var sum = center * 0.18;
    for (var tap = 0; tap < 12; tap = tap + 1) {
        let angle = rotation + f32(tap) * 2.39996323;
        let tap_radius = radius * sqrt((f32(tap) + 0.5) / 12.0);
        let offset = vec2<f32>(cos(angle), sin(angle)) * tap_radius;
        let gx = g.x * offset.x;
        let gy = g.y * offset.y;
        sum += textureSampleCompareLevel(
            shadow_map, shadow_sampler, uv + offset, cascade_idx,
            clamp(z + gx + gy, 0.0, 1.0)) * 0.0683333;
    }
    return clamp(sum, 0.0, 1.0);
}

fn sample_sun_shadow(world_pos: vec3<f32>, linear_depth: f32, ndotl: f32, world_dx: vec3<f32>, world_dy: vec3<f32>) -> f32 {
    let shadow_range = max(shadow.cascades[4].far_plane, 1e-4);
    let range_fade = smoothstep(shadow_range * 0.78, shadow_range,
        distance(world_pos, cam.camera_pos.xyz));
    if range_fade >= 1.0 { return 1.0; }

    var first_cascade = 4;
    for (var i = 0; i < 4; i = i + 1) {
        if linear_depth < shadow.cascades[i].far_plane {
            first_cascade = i;
            break;
        }
    }

    // Resolve uncovered weight with each successive valid cascade. Interior
    // pixels normally execute one kernel; blend bands normally execute two.
    // A rare overlap of several projection edges can use up to five. This
    // avoids switching abruptly when the immediate next cascade is invalid.
    var remaining = 1.0;
    var visibility = 1.0;
    for (var i = first_cascade; i < 5; i = i + 1) {
        let c = shadow.cascades[i];
        let shadow_pos = c.light_view_proj * vec4<f32>(world_pos, 1.0);
        if shadow_pos.w <= 1e-6 { continue; }
        let proj = shadow_pos.xyz / shadow_pos.w;
        let uv = proj.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
        let edge = min(min(uv.x, uv.y), min(1.0 - uv.x, 1.0 - uv.y));
        let texel = max(c.texel_size, 1e-8);
        let radius = texel * 1.2;
        // Include the bilinear comparison footprint beyond the tap centre.
        let padding = radius + texel * 0.5;
        if proj.z <= 0.0 || proj.z >= 1.0 || edge <= padding { continue; }

        let edge_band = max(texel * 4.0, select(0.035, 0.07, i == 4));
        let edge_coverage = smoothstep(padding, padding + edge_band, edge);
        var depth_coverage = 1.0;
        if i < 4 {
            let width = max(shadow.blend_width, 1e-4);
            depth_coverage = 1.0 - smoothstep(c.far_plane - width, c.far_plane, linear_depth);
        }
        let coverage = min(edge_coverage, depth_coverage);
        let weight = remaining * coverage;
        // Do not spend a full PCF kernel on an imperceptibly small tail.
        // Its omitted weight retains the initialized unshadowed fallback.
        if weight > 1e-4 {
            let bias = c.base_bias + (1.0 - clamp(ndotl, 0.0, 1.0)) * c.slope_bias;
            let sampled = pcf_shadow(uv, proj.z, bias, radius, i,
                shadow_receiver_gradient(c.light_view_proj, world_dx, world_dy));
            visibility += weight * (sampled - 1.0);
        }
        remaining *= 1.0 - coverage;
        if remaining <= 1e-4 { break; }
    }
    return mix(clamp(visibility, 0.0, 1.0), 1.0, range_fade);
}

fn slab_segment_length(ro: vec3<f32>, rd: vec3<f32>, seg_len: f32,
    y0: f32, y1: f32) -> f32 {
    let segment = max(seg_len, 0.0);
    let lo = min(y0, y1);
    let hi = max(y0, y1);
    // Test the endpoint displacement, not a fixed angular epsilon: even a nearly
    // horizontal kilometre ray can cross a shallow layer.
    let dy = rd.y * segment;
    if abs(dy) < 1e-6 {
        return select(0.0, segment, ro.y >= lo && ro.y <= hi);
    }
    let u0 = (lo - ro.y) / dy;
    let u1 = (hi - ro.y) / dy;
    let enter = max(min(u0, u1), 0.0);
    let leave = min(max(u0, u1), 1.0);
    return max(leave - enter, 0.0) * segment;
}

// Integral exp(-k*max(y-base,0)) ds along a straight segment, in world metres.
// Splitting at the layer base prevents overflow for cameras below the layer.
fn terrain_height_integral(y_start: f32, y_end: f32, length_m: f32,
    base: f32, falloff: f32) -> f32 {
    let distance_m = max(length_m, 0.0);
    let k = max(falloff, 0.0);
    let low = min(y_start, y_end) - base;
    let high = max(y_start, y_end) - base;
    let span = high - low;
    if k <= 1e-7 { return distance_m; }
    if high <= 0.0 { return distance_m; }
    if span < 1e-5 { return distance_m * exp(-k * max(0.5 * (low + high), 0.0)); }
    let below_fraction = clamp(-low / span, 0.0, 1.0);
    let upper_start = max(low, 0.0);
    let upper_span = high - upper_start;
    let optical_span = k * upper_span;
    var exponential_mean = 1.0;
    if optical_span < 0.01 {
        exponential_mean = 1.0 - optical_span * 0.5 + optical_span * optical_span / 6.0;
    } else {
        exponential_mean = (1.0 - exp(-optical_span)) / optical_span;
    }
    let above_integral = (upper_span / span) * exp(-k * upper_start) * exponential_mean;
    return distance_m * clamp(below_fraction + above_integral, 0.0, 1.0);
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
    // Avoid twelve texture loads when only one clip level contributes.
    var field = vec4<f32>(0.0);
    if weight0 > 0.0 {
        field += sample_weather_bilinear(cloud_shadow_tex, uv0, 0) * weight0;
    }
    if weight1 > 0.0 {
        field += sample_weather_bilinear(cloud_shadow_tex, uv1, 1) * weight1;
    }
    if weight2 * outer_coverage > 0.0 {
        field += sample_weather_bilinear(cloud_shadow_tex, uv2, 2) * weight2 * outer_coverage;
    }
    return max(field, vec4<f32>(0.0));
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
    let optical_depth = max(low, 0.0) + max(mid, 0.0) + max(high, 0.0);
    let transmittance = exp(-min(optical_depth / max(light_dir.y, 0.15), 80.0));
    return mix(1.0, transmittance, smoothstep(0.02, 0.08, light_dir.y));
}

// ---- Torch lighting ----

struct TorchLightSample {
    diffuse: vec3<f32>,
    fill   : vec3<f32>,
    spec   : vec3<f32>,
};

fn torch_flicker(seed: vec3<f32>, time: f32) -> f32 {
    let phase   = dot(seed, vec3<f32>(0.173, 0.697, 0.357));
    let slow = sin(time * 2.2 + phase);
    let breath = sin(time * 4.6 + phase * 1.3);
    return clamp(0.96 + slow * 0.035 + breath * 0.015, 0.92, 1.04);
}

// Flame height above its base, 0..1. The mesher packs it into surface.w as
// -2 - lift, so every existing `surface.w < -1.5` flame test still matches.
fn flame_lift(surface_w: f32) -> f32 {
    return clamp(-surface_w - 2.0, 0.0, 1.0);
}

// Black-body ramp for fire voxels and torch flames, at unit emission scale:
// deep orange edges, orange body and a small warm core. The body keeps little
// green: at 0.31 with a full white core, the hottest cells summed to near-white
// and read as pale salmon.
fn fire_ramp(heat: f32) -> vec3<f32> {
    let edge = vec3<f32>(0.75, 0.055, 0.006);
    let body = vec3<f32>(1.0, 0.18, 0.015);
    let core = vec3<f32>(1.0, 0.60, 0.25);
    return mix(edge, body, heat) + core * (heat * heat * heat * 0.5 / 3.1);
}

// Light cast by a torch: a slow sway plus a quicker flutter, +/-8% around the
// same 0.96 mean as torch_flicker, so the average brightness of lights is kept.
fn torch_light_flicker(seed: vec3<f32>, time: f32) -> f32 {
    let phase = dot(seed, vec3<f32>(0.173, 0.697, 0.357));
    let sway = sin(time * 6.1 + phase);
    let flutter = sin(time * 11.3 + phase * 1.6);
    return clamp(0.96 + sway * 0.05 + flutter * 0.03, 0.88, 1.04);
}

fn eval_torch_light(
    torch_pos_radius: vec4<f32>, torch_color_strength: vec4<f32>,
    time: f32, world_pos: vec3<f32>, n: vec3<f32>, to_eye: vec3<f32>,
    roughness: f32, f0: vec3<f32>, metallic: f32, foliage: bool, moving: bool,
) -> TorchLightSample {
    let radius = max(torch_pos_radius.w, 0.001);
    let to_torch = torch_pos_radius.xyz - world_pos;
    let dist2 = dot(to_torch, to_torch);
    if dist2 >= radius * radius {
        return TorchLightSample(vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0));
    }
    let distance_m = sqrt(max(dist2, 1e-8));
    let l = terrain_safe_unit3(to_torch, n);
    let cutoff = clamp(1.0 - pow(distance_m / radius, 4.0), 0.0, 1.0);
    let falloff = cutoff * cutoff / (1.0 + dist2 * 0.045);
    let flicker_seed = select(torch_pos_radius.xyz * 0.23, vec3<f32>(0.0), moving);
    let flicker = torch_light_flicker(flicker_seed + vec3<f32>(0.0, 1.7, 0.0), time);
    let radiance = max(torch_color_strength.rgb, vec3<f32>(0.0))
        * (max(torch_color_strength.w, 0.0) * falloff * flicker);
    let no_l = clamp(dot(n, l), 0.0, 1.0);
    let wrapped = clamp((dot(n, l) + 0.35) / 1.35, 0.0, 1.0);
    let h = terrain_safe_unit3(to_eye + l, n);
    let energy = (vec3<f32>(1.0) - terrain_fresnel(f0, dot(to_eye, h))) * (1.0 - metallic);
    return TorchLightSample(
        radiance * energy * (select(no_l, wrapped, foliage) * 0.318309886),
        radiance * (vec3<f32>(1.0) - f0) * (0.025 * (1.0 - metallic)),
        radiance * terrain_specular(n, to_eye, l, roughness, f0),
    );
}

// ============================================================
// FRAGMENT SHADER
// ============================================================

// Procedural surface filters receive their footprint from fs_main. There are
// no implicit derivatives here, so material branches remain valid WGSL.
fn surface_hash(p: vec2<f32>) -> f32 {
    let cell = vec2<i32>(floor(p));
    var h = (bitcast<u32>(cell.x) * 0x9e3779b9u)
        ^ (bitcast<u32>(cell.y) * 0x85ebca6bu) ^ cam.scene_seed.x;
    h = (h ^ (h >> 16u)) * 0x7feb352du;
    h = (h ^ (h >> 15u)) * 0x846ca68bu;
    return f32((h ^ (h >> 16u)) >> 8u) * (1.0 / 16777216.0);
}

fn surface_noise_grad(p: vec2<f32>) -> vec3<f32> {
    let cell = floor(p);
    let f = fract(p);
    // Quintic reconstruction is C2 at cell boundaries. The analytic gradient
    // matches the scalar field exactly, including at negative coordinates.
    let w = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let dw = 30.0 * f * f * (f - 1.0) * (f - 1.0);
    let a = surface_hash(cell);
    let b = surface_hash(cell + vec2<f32>(1.0, 0.0));
    let c = surface_hash(cell + vec2<f32>(0.0, 1.0));
    let d = surface_hash(cell + vec2<f32>(1.0));
    let k1 = b - a;
    let k2 = c - a;
    let k3 = a - b - c + d;
    let value = a + k1 * w.x + k2 * w.y + k3 * w.x * w.y;
    let gradient = dw * vec2<f32>(k1 + k3 * w.y, k2 + k3 * w.x);
    return vec3<f32>(value, gradient);
}

fn surface_noise(p: vec2<f32>) -> f32 {
    return surface_noise_grad(p).x;
}

// Conservative octave clamping, not an exact integral of the complete shader.
// A noise cell loses contrast between five and two pixels across and converges
// to its statistical mean. Fixed wavelengths never change with camera distance.
// Means of the per-texel leaf tint/tone ramps (surface_hash is uniform on
// [0,1)), so the footprint fade keeps a canopy's average colour.
const LEAF_TINT_MEAN: vec3<f32> = vec3<f32>(0.985, 1.01, 0.89);
const LEAF_TONE_MEAN: vec3<f32> = vec3<f32>(0.97, 0.995, 0.825);

fn surface_detail_weight(footprint_in_cells: f32) -> f32 {
    return 1.0 - smoothstep(0.20, 0.50, max(footprint_in_cells, 0.0));
}

fn surface_noise_filtered(p: vec2<f32>, footprint_in_cells: f32) -> f32 {
    let weight = surface_detail_weight(footprint_in_cells);
    if weight <= 0.0 { return 0.5; }
    return 0.5 + (surface_noise(p) - 0.5) * weight;
}

fn surface_noise_grad_filtered(p: vec2<f32>, footprint_in_cells: f32) -> vec3<f32> {
    let weight = surface_detail_weight(footprint_in_cells);
    if weight <= 0.0 { return vec3<f32>(0.5, 0.0, 0.0); }
    let sample = surface_noise_grad(p);
    return vec3<f32>(0.5 + (sample.x - 0.5) * weight, sample.yz * weight);
}

// Small shading relief from one fixed world field, shared by resident terrain
// and the horizon mesh. This does not reconstruct missing geometric relief.
// Projecting the gradient into the tangent plane keeps the normal in the same
// hemisphere and avoids flattening cliffs toward the world-up axis.
fn far_shade(n: vec3<f32>, xz: vec2<f32>, footprint_m: f32) -> vec3<f32> {
    let coarse = surface_noise_grad_filtered(xz / 48.0 + vec2<f32>(13.7, -29.4), footprint_m / 48.0);
    let fine = surface_noise_grad_filtered(xz / 12.0 + vec2<f32>(-17.3, 8.1), footprint_m / 12.0);
    let g = coarse.yz * 0.045 + fine.yz * 0.014;
    let gradient = vec3<f32>(g.x, 0.0, g.y);
    let tangent_gradient = gradient - n * dot(n, gradient);
    return normalize(n - tangent_gradient);
}

// Distant-terrain detail maps (render/horizon_detail.rs). One RGBA16F layer per
// level: height (m), normal x, normal z, cavity. Level i has texels of
// params.w * 2^i metres, 512 wide, centred on params.xy; params.z is 1 when a
// published map is bound. Texels are world anchored, so nothing swims when
// the horizon recentres, and every lookup blends levels continuously.
@group(3) @binding(5) var horizon_detail_tex: texture_2d_array<f32>;
@group(3) @binding(6) var<uniform> horizon_detail_params: vec4<f32>;
const HORIZON_DETAIL_LEVELS: i32 = 4;
const HORIZON_DETAIL_SIZE: f32 = 512.0;

fn horizon_detail_on() -> bool {
    return horizon_detail_params.z > 0.5;
}

fn horizon_detail_texel(level: i32) -> f32 {
    return horizon_detail_params.w * exp2(f32(level));
}

// 0 well inside a level, rising to 1 near its outer edge.
fn horizon_detail_edge(xz_m: vec2<f32>, level: i32) -> f32 {
    let half = 0.5 * HORIZON_DETAIL_SIZE * horizon_detail_texel(level);
    let d = abs(xz_m - horizon_detail_params.xy);
    return smoothstep(0.88, 0.97, max(d.x, d.y) / half);
}

fn horizon_detail_fetch(xz_m: vec2<f32>, level: i32) -> vec4<f32> {
    let uv = (xz_m - horizon_detail_params.xy)
        / (HORIZON_DETAIL_SIZE * horizon_detail_texel(level)) + vec2<f32>(0.5);
    let inset = vec2<f32>(0.5 / HORIZON_DETAIL_SIZE);
    return textureSampleLevel(horizon_detail_tex, cloud_field_sampler,
        clamp(uv, inset, vec2<f32>(1.0) - inset), level, 0.0);
}

// Continuous-level lookup. `lod` is the finest level wanted (fractional); a
// level's outer edge pushes the lookup to the next coarser one, so there is no
// visible boundary between levels. `weight` is the coverage:
// 0 when the map is off or past the coarsest level's edge.
struct HorizonDetail {
    value: vec4<f32>,
    weight: f32,
}

fn horizon_detail_sample(xz_m: vec2<f32>, lod: f32) -> HorizonDetail {
    var out: HorizonDetail;
    out.value = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    out.weight = 0.0;
    if !horizon_detail_on() { return out; }
    let top = HORIZON_DETAIL_LEVELS - 1;
    let l = clamp(lod, 0.0, f32(top));
    var level = i32(floor(l));
    var blend = l - floor(l);
    loop {
        if level >= top {
            level = top;
            blend = 0.0;
            break;
        }
        let edge = horizon_detail_edge(xz_m, level);
        if edge < 1.0 {
            blend = max(blend, edge);
            break;
        }
        level = level + 1;
        blend = 0.0;
    }
    out.value = horizon_detail_fetch(xz_m, level);
    if blend > 0.0 && level < top {
        out.value = mix(out.value, horizon_detail_fetch(xz_m, level + 1), blend);
    }
    out.weight = 1.0 - horizon_detail_edge(xz_m, top);
    return out;
}

// The finest level whose texel is at least ~1.5 screen pixels, so normals
// never alias into shimmer.
fn horizon_detail_lod(footprint_m: f32) -> f32 {
    return log2(max(footprint_m * 1.5, 1e-3) / horizon_detail_params.w);
}

fn horizon_detail_normal(value: vec4<f32>) -> vec3<f32> {
    let xz = value.yz;
    return normalize(vec3<f32>(xz.x, sqrt(max(1.0 - dot(xz, xz), 0.0)), xz.y));
}

// Broad terrain occlusion from the map's cavity term: valleys and hollows
// lose sky, crests gain a little. Smooth, never a step.
fn horizon_detail_occlusion(cavity: f32) -> f32 {
    return clamp(1.0 - 0.42 * max(cavity, 0.0) + 0.10 * max(-cavity, 0.0), 0.62, 1.06);
}

// Sun visibility from distant terrain: march the height field toward the sun
// in growing steps (32 m to ~17 km) and keep the least clearance. Ranges then
// shade valleys and each other at low sun, far past the shadow cascades. The
// start height is never below the map, so filtered relief cannot shadow the
// surface it was sampled from.
fn horizon_detail_sun_visibility(p_m: vec3<f32>, sun: vec3<f32>) -> f32 {
    if !horizon_detail_on() || sun.y <= 0.0 { return 1.0; }
    let across = length(sun.xz);
    if across < 1e-4 { return 1.0; }
    let tan_sun = sun.y / across;
    // Steeper than any filtered slope in the map: nothing can occlude it.
    if tan_sun > 1.6 { return 1.0; }
    let dir = sun.xz / across;
    let here = horizon_detail_sample(p_m.xz, 0.0);
    if here.weight <= 0.0 { return 1.0; }
    let start = max(p_m.y, here.value.x);
    var t = 2.0 * horizon_detail_params.w;
    var clearance = 1e9;
    for (var i = 0; i < 18; i = i + 1) {
        let q = p_m.xz + dir * t;
        // Step length sets the level: about four texels per step.
        let lod = log2(max(t * 0.25, 1e-3) / horizon_detail_params.w);
        let s = horizon_detail_sample(q, lod);
        if s.weight <= 0.0 { break; }
        let texel = horizon_detail_texel(clamp(i32(ceil(lod)), 0, HORIZON_DETAIL_LEVELS - 1));
        // Clearance in units of a soft penumbra that widens with the texel
        // seen from here, so coarse levels give soft, stable edges.
        let rise = (s.value.x - start) / t;
        let width = 0.010 + 0.5 * texel / t;
        clearance = min(clearance, (tan_sun - rise) / width * s.weight + (1.0 - s.weight) * 1e3);
        t = t * 1.45;
    }
    return smoothstep(-1.0, 1.0, clearance);
}

// Preserve the generator's seed mixing and cubic climate interpolation.
// World-space micro relief for opaque voxel materials. The height field is
// evaluated in metres, projected into the shading tangent plane, and filtered
// by the pixel footprint. This is the difference between a pile of perfectly
// milled cubes and stone/wood/soil that catches a torch at close range.
fn surface_relief(
    n: vec3<f32>,
    world_m: vec3<f32>,
    footprint_m: f32,
    strength: f32,
) -> vec3<f32> {
    let drift = vec2<f32>(world_m.y * 0.37, -world_m.y * 0.29);
    let coarse = surface_noise_grad_filtered(
        world_m.xz / 0.58 + drift,
        footprint_m / 0.58,
    );
    let fine = surface_noise_grad_filtered(
        world_m.xz / 0.17 - drift * 1.6 + vec2<f32>(37.0, -19.0),
        footprint_m / 0.17,
    );
    let gradient2 = coarse.yz * 0.060 + fine.yz * 0.012;
    let gradient = vec3<f32>(gradient2.x, 0.0, gradient2.y);
    let tangent_gradient = gradient - n * dot(n, gradient);
    return terrain_safe_unit3(n - tangent_gradient * strength, n);
}

fn climate_hash(p: vec2<i32>, seed: u32) -> f32 {
    var h = (bitcast<u32>(p.x) * 374761393u) ^ (bitcast<u32>(p.y) * 668265263u) ^ seed;
    h = (h ^ (h >> 13u))*1274126177u; h ^= h >> 16u;
    return f32(h & 65535u)/65535.0;
}
fn climate_noise(p: vec2<f32>, seed: u32) -> f32 {
    let cell=vec2<i32>(floor(p));let f=fract(p);let t=f*f*(3.0-2.0*f);
    return mix(mix(climate_hash(cell,seed),climate_hash(cell+vec2<i32>(1,0),seed),t.x),
        mix(climate_hash(cell+vec2<i32>(0,1),seed),climate_hash(cell+vec2<i32>(1,1),seed),t.x),t.y);
}
fn climate_noise_filtered(p: vec2<f32>, seed: u32, footprint_in_cells: f32) -> f32 {
    let weight = surface_detail_weight(footprint_in_cells);
    if weight <= 0.0 { return 0.5; }
    return 0.5 + (climate_noise(p, seed) - 0.5) * weight;
}

// One-texel speck on the world pixel grid. A texel is on when its hash falls
// in the top `density` fraction. It fades out once a texel is smaller than a
// screen pixel, so distant ground stays calm. `cells_per_m` is kept so callers
// need no change, but it no longer picks a resolution: there is only PIXEL_RES.
fn pixel_blob(uv: vec2<f32>, cells_per_m: f32, seed: vec2<f32>, density: f32, footprint_m: f32) -> f32 {
    let anchor = floor(uv * PIXEL_RES);
    let on = step(1.0 - density, surface_hash(anchor + seed));
    return on * surface_detail_weight(footprint_m * PIXEL_RES);
}

// Planar coordinates on the dominant face axis, for detail that must not
// stretch down a vertical cut (native-cell grain on cut banks and cliffs).
fn natural_face_uv(q: vec3<f32>, normal: vec3<f32>) -> vec2<f32> {
    let a = abs(normal);
    if a.x > a.y && a.x > a.z { return q.zy; }
    if a.z > a.y { return q.xy; }
    return q.xz;
}

// ---- Terrain material shapes -------------------------------------------
// Grass, soil, gravel and rock are built from shape cells on the one world
// texel lattice: a Voronoi of seeds placed every N texels, measured from
// texel centres. N picks how many texels form one blade tuft, clod, pebble
// or rock plane; it never changes the pixel size. Neighbouring texels agree
// on their parent cell, which is what turns per-texel noise into readable
// forms. Tunables (in texels): GRASS_TUFT_TEXELS, SOIL_CLOD_TEXELS,
// GRAVEL_STONE_TEXELS, ROCK_PLANE_TEXELS.
const GRASS_TUFT_TEXELS: f32 = 4.0;
const SOIL_CLOD_TEXELS: f32 = 4.0;
const GRAVEL_STONE_TEXELS: f32 = 3.0;
const ROCK_PLANE_TEXELS: f32 = 9.0;
// Shallowest turf fringe on a grass block side, in texels from the block top.
// Each column adds 0-4 more, and a few drop 3 further. About 0.55 of a
// one-metre riser is turf: terraces on gentle slopes are mostly green, and a
// mostly-soil riser drew brown bands across hillsides.
const GRASS_FRINGE_TEXELS: f32 = 7.0;

// World-space shading tilt written by the natural material functions for the
// fragment being shaded: the tangent-plane gradient of the material's own
// height (pebble domes, clod tops, rock planes). fs_main subtracts it from the
// normal, so relief follows the painted shapes rather than a separate field.
var<private> natural_tilt: vec3<f32>;

struct ShapeCell {
    d1: f32,          // texel distance to the nearest seed
    d2: f32,          // texel distance to the second seed
    id: f32,          // hash of the nearest cell, in [0, 1)
    to_seed: vec2<f32>, // texel offset from the nearest seed to this texel
}

// `t` is a texel-centre coordinate in texel units (floor(uv * PIXEL_RES) + 0.5).
fn shape_cells(t: vec2<f32>, cell_texels: f32, seed: vec2<f32>) -> ShapeCell {
    let g = floor(t / cell_texels);
    var out = ShapeCell(1e8, 1e8, 0.0, vec2<f32>(0.0));
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let c = g + vec2<f32>(f32(i), f32(j));
            let jitter = vec2<f32>(surface_hash(c + seed), surface_hash(c + seed + vec2<f32>(57.0, 113.0)));
            let site = (c + vec2<f32>(0.15) + jitter * 0.7) * cell_texels;
            let off = t - site;
            // Compare squared distances: only the two final radii need sqrt.
            let d = dot(off, off);
            if d < out.d1 {
                out.d2 = out.d1;
                out.d1 = d;
                out.id = surface_hash(c + seed + vec2<f32>(-91.0, 37.0));
                out.to_seed = off;
            } else if d < out.d2 {
                out.d2 = d;
            }
        }
    }
    out.d1 = sqrt(out.d1);
    out.d2 = sqrt(out.d2);
    return out;
}

// World axes of the face's planar mapping, matching natural_face_uv.
fn natural_face_axes(normal: vec3<f32>) -> mat2x3<f32> {
    let a = abs(normal);
    if a.x > a.y && a.x > a.z { return mat2x3<f32>(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 0.0)); }
    if a.z > a.y { return mat2x3<f32>(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0)); }
    return mat2x3<f32>(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0));
}

fn natural_tilt_from(normal: vec3<f32>, slope_texels: vec2<f32>) -> vec3<f32> {
    let axes = natural_face_axes(normal);
    return axes[0] * slope_texels.x + axes[1] * slope_texels.y;
}

// Gravel: separate small stones in a bed of darker compacted fines. Stones are
// 2-4 texels across, never touch (a fines gap keeps them from reading as
// masonry), take one of four mineral tones, and shade as low domes.
fn gravel_surface(base: vec3<f32>, q: vec3<f32>, normal: vec3<f32>, fp: f32) -> vec3<f32> {
    let t = natural_face_uv(q, normal) * PIXEL_RES;
    let shape_w = surface_detail_weight(fp * PIXEL_RES / GRAVEL_STONE_TEXELS);
    let mean = mix(base * 0.66, base * 1.01, 0.34);
    if shape_w <= 0.0 { return mean; }
    let c = shape_cells(t, GRAVEL_STONE_TEXELS, vec2<f32>(211.0, -67.0));
    let texel_w = surface_detail_weight(fp * PIXEL_RES);
    let radius = GRAVEL_STONE_TEXELS * (0.34 + c.id * 0.28);
    let stone = step(c.d1, radius) * step(0.75, c.d2 - c.d1);
    let pick = floor(c.id * 4.0);
    var tone = vec3<f32>(1.0);
    if pick == 1.0 { tone = vec3<f32>(1.10, 1.00, 0.86); }
    if pick == 2.0 { tone = vec3<f32>(0.80, 0.82, 0.86); }
    if pick == 3.0 { tone = vec3<f32>(1.16, 1.14, 1.08); }
    let fleck = (surface_hash(floor(t) + vec2<f32>(3.0, 91.0)) - 0.5) * texel_w;
    let fines = base * (0.66 + fleck * 0.08);
    // Crevice: fines texels hugging a stone sit in its shadow.
    let crevice = step(c.d1, radius + 0.9) * (1.0 - stone);
    let detailed = mix(fines * (1.0 - crevice * 0.16), base * tone * (0.98 + fleck * 0.05), stone);
    natural_tilt = natural_tilt_from(normal, -c.to_seed / max(radius, 1.0) * 0.34 * stone * shape_w);
    return mix(mean, detailed, shape_w);
}

// Soil: compacted clods 3-5 texels across with broken dark seams, sparse
// embedded stones. `earth` is the authored Dirt / Mud / Clay swatch.
fn soil_surface(earth: vec3<f32>, q: vec3<f32>, normal: vec3<f32>, fp: f32) -> vec3<f32> {
    let t = natural_face_uv(q, normal) * PIXEL_RES;
    let shape_w = surface_detail_weight(fp * PIXEL_RES / SOIL_CLOD_TEXELS);
    if shape_w <= 0.0 { return earth; }
    let c = shape_cells(t, SOIL_CLOD_TEXELS, vec2<f32>(13.0, 71.0));
    let texel_w = surface_detail_weight(fp * PIXEL_RES);
    let grain = (surface_hash(floor(t) + vec2<f32>(9.0, -3.0)) - 0.5) * texel_w;
    var col = earth * (0.84 + c.id * 0.30 + grain * 0.07);
    // Seams between clods, broken so they never outline every clod.
    let seam = step(c.d2 - c.d1, 0.6) * step(0.45, surface_hash(floor(t / 2.0) + vec2<f32>(5.0, 17.0))) * texel_w;
    col *= 1.0 - seam * 0.20;
    let pebble = step(0.93, c.id) * step(c.d1, 1.7);
    col = mix(col, vec3<f32>(0.42, 0.40, 0.37) * (0.92 + grain * 0.1), pebble);
    natural_tilt = natural_tilt_from(normal,
        -c.to_seed / SOIL_CLOD_TEXELS * mix(0.16, 0.34, pebble) * (1.0 - seam) * shape_w);
    return mix(earth, col, shape_w);
}

// Rock: broad mineral planes ~45 cm across, each tilted a little so it catches
// the light as one plane; sparse broken fractures along some plane borders and
// rare pale inclusions. No per-texel crack noise.
fn rock_surface(rock: vec3<f32>, q: vec3<f32>, normal: vec3<f32>, fp: f32) -> vec3<f32> {
    let t = natural_face_uv(q, normal) * PIXEL_RES;
    let shape_w = surface_detail_weight(fp * PIXEL_RES / ROCK_PLANE_TEXELS);
    if shape_w <= 0.0 { return rock; }
    let c = shape_cells(t, ROCK_PLANE_TEXELS, vec2<f32>(-44.0, 19.0));
    let texel_w = surface_detail_weight(fp * PIXEL_RES);
    var col = rock * (0.92 + c.id * 0.14);
    let run = surface_noise(t * 0.07 + vec2<f32>(c.id * 9.0, 3.0));
    let fracture = step(c.d2 - c.d1, 0.55) * step(0.52, run) * texel_w;
    col *= 1.0 - fracture * 0.32;
    let inclusion = step(0.988, surface_hash(floor(t) + vec2<f32>(61.0, -5.0))) * texel_w;
    col = mix(col, rock * 1.22, inclusion);
    let a = c.id * 6.2831853;
    natural_tilt = natural_tilt_from(normal, vec2<f32>(cos(a), sin(a)) * 0.10 * shape_w);
    return mix(rock, col, shape_w);
}

// Grass top: short blade tufts on a quieter matted floor. Each tuft is 3-6
// texels across with a lean direction: texels on the lean side are lit tips,
// the far side is the root, and texels just behind it fall in its shade.
// `turf` is the field colour already carrying climate and broad patchiness.
// `bloom` (0-1, a patch field) lets a few tufts carry one flower at their heart.
fn grass_top(turf: vec3<f32>, q: vec3<f32>, fp: f32, dry: f32, bloom: f32) -> vec3<f32> {
    let t = q.xz * PIXEL_RES;
    let shape_w = surface_detail_weight(fp * PIXEL_RES / GRASS_TUFT_TEXELS);
    if shape_w <= 0.0 { return turf * 0.90; }
    let c = shape_cells(t, GRASS_TUFT_TEXELS, vec2<f32>(31.0, -17.0));
    let texel_w = surface_detail_weight(fp * PIXEL_RES);
    let a = c.id * 6.2831853;
    let lean = vec2<f32>(cos(a), sin(a));
    let r = 1.3 + fract(c.id * 7.31) * 1.4;
    let along = dot(c.to_seed, lean);
    let across = dot(c.to_seed, vec2<f32>(-lean.y, lean.x));
    let in_tuft = step(c.d1, r);
    // Blade columns inside a tuft: a run of texels across the lean is on.
    let blade = mix(0.68, step(0.32, surface_hash(vec2<f32>(floor(across + 0.5), c.id * 977.0))), texel_w);
    let rise = saturate1(along / r * 0.5 + 0.5);
    let quiet = (surface_hash(floor(t) + vec2<f32>(-13.0, 29.0)) - 0.5) * texel_w;
    var value = 0.81 + quiet * 0.04;
    // Variation is pigment/root occlusion, not a painted directional highlight.
    // Live direct and local lights illuminate the material-shaped normal below.
    value = mix(value, mix(0.87, 1.02, blade), in_tuft);
    var col = turf * value;
    let dry_tip = dry * in_tuft * blade * step(0.55, rise);
    col = mix(col, turf * vec3<f32>(1.20, 1.10, 0.62), dry_tip * 0.75);
    let flower_pick = fract(c.id * 53.7);
    let flower = step(1.0 - bloom * 0.08, flower_pick) * step(c.d1, 0.75) * texel_w;
    let petal = select(select(vec3<f32>(0.86, 0.84, 0.74), vec3<f32>(0.92, 0.74, 0.16), flower_pick > 0.93),
        vec3<f32>(0.55, 0.36, 0.78), flower_pick > 0.97);
    col = mix(col, petal, flower);
    natural_tilt = vec3<f32>(lean.x, 0.0, lean.y) * 0.05 * in_tuft * shape_w;
    return mix(turf * 0.90, col, shape_w);
}

// Grass block side: soil with an irregular turf fringe 7-14 texels deep along
// the block's top edge, and a few root texels just beneath it. Bottom faces
// are plain soil. Rows count down from the block top so the fringe follows the
// face orientation on all four sides.
fn grass_side(soil_col: vec3<f32>, turf: vec3<f32>, p: vec3<f32>, q: vec3<f32>, normal: vec3<f32>, fp: f32) -> vec3<f32> {
    if normal.y < -0.5 { return soil_col; }
    let scale = cam.world_params.x;
    let top_m = (floor(p.y) + 1.0) * scale;
    let row = floor(max(top_m - q.y, 0.0) * PIXEL_RES);
    let col_i = floor(natural_face_uv(q, normal).x * PIXEL_RES);
    var depth = GRASS_FRINGE_TEXELS + floor(surface_hash(vec2<f32>(floor(col_i / 2.0), floor(p.y)) + vec2<f32>(7.0, 3.0)) * 5.0);
    depth += step(0.86, surface_hash(vec2<f32>(col_i, floor(p.y)) + vec2<f32>(-29.0, 11.0))) * 3.0;
    let texel_w = surface_detail_weight(fp * PIXEL_RES);
    let fringe = step(row, depth - 0.5);
    let lip = step(depth - 1.5, row) * fringe;
    let root = step(depth - 0.5, row) * step(row, depth + 2.5)
        * step(0.84, surface_hash(vec2<f32>(col_i, row) + vec2<f32>(41.0, 3.0)));
    var col = mix(soil_col, turf * (0.90 + surface_hash(vec2<f32>(col_i, row)) * 0.12) * (1.0 - lip * 0.18), fringe);
    col = mix(col, soil_col * vec3<f32>(0.72, 0.66, 0.60), root * (1.0 - fringe));
    // Keep the fringe's mean coverage (about 11 of 20 rows) when texels blur.
    let mean = mix(soil_col, turf, clamp(0.55 / scale, 0.0, 1.0));
    if fringe > 0.5 { natural_tilt = vec3<f32>(0.0); }
    return mix(mean, col, texel_w);
}

// Interpolated coverage, not interpolated numeric material IDs. Missing coverage
// retains the authored base color. Use unscaled voxel coordinates: texture scale
// must not move physical climate boundaries, shorelines, or snow altitude.
fn natural_surface_blended(
    base: vec3<f32>, p: vec3<f32>, normal: vec3<f32>,
    terrain_weights: vec4<f32>, footprint: f32, turf_exposed: bool,
) -> vec3<f32> {
    let positive_weights = max(terrain_weights, vec4<f32>(0.0));
    let weights = positive_weights / max(dot(positive_weights, vec4<f32>(1.0)), 1.0);
    let coverage = dot(weights, vec4<f32>(1.0));
    if coverage <= 0.00001 { return base; }
    // Snapped to texel centres: every noise below is flat inside a texel.
    let q = (floor(p * cam.world_params.x * PIXEL_RES) + vec3<f32>(0.5)) / PIXEL_RES;
    let fp = max(footprint * cam.world_params.x, 0.0);
    let seed = cam.scene_seed.x;
    let warped = q.xz + vec2<f32>(
        climate_noise_filtered(q.xz / 420.0, seed + 7u, fp / 420.0) - 0.5,
        climate_noise_filtered(q.xz / 390.0, seed + 9u, fp / 390.0) - 0.5) * 180.0;
    let climate_gain = select(1.0, 1.55, cam.scene_seed.y >= 5u);
    let lapse = select(0.0035, 0.0018, cam.scene_seed.y >= 7u);
    // 2.5 bounds the original domain warp's local magnification.
    let heat = clamp((climate_noise_filtered(warped / 700.0, seed + 11u, fp * (2.5 / 700.0)) - 0.5)
        * climate_gain + 0.5 - max(q.y - 30.0, 0.0) * lapse, 0.0, 1.0);
    let wet = clamp((climate_noise_filtered(warped / 520.0, seed + 23u, fp * (2.5 / 520.0)) - 0.5)
        * select(1.0, 1.4, cam.scene_seed.y >= 5u) + 0.5, 0.0, 1.0);
    let arid = smoothstep(0.48, 0.70, heat) * (1.0 - smoothstep(0.26, 0.48, wet));
    let mottle = surface_noise_filtered(q.xz / 17.0 + q.y * vec2<f32>(0.017, 0.027), fp * 0.068);
    let clumps = surface_noise_filtered(q.xz * 0.65 + q.y * vec2<f32>(0.21, 0.16), fp * 0.71) - 0.5;
    let up = smoothstep(-0.05, 0.65, normal.y);
    let soil = mix(vec3<f32>(0.36, 0.25, 0.15), vec3<f32>(0.48, 0.35, 0.21), 1.0 - wet);
    let sand = mix(vec3<f32>(0.54, 0.45, 0.29), vec3<f32>(0.68, 0.57, 0.38), heat);
    var color = vec3<f32>(0.0);

    if weights.x > 0.0 {
        // Expedition olive. `base` is the authored grass swatch. Climate may
        // only walk toward leaf, sand, or soil, the same hues as the blocks.
        let leaf = vec3<f32>(0.30, 0.43, 0.25);
        let cool = 1.0 - smoothstep(0.22, 0.44, heat);
        let forest = smoothstep(0.48, 0.72, wet);
        let vigor = clamp(0.5
            + (surface_noise_filtered(q.xz / 310.0 + vec2<f32>(7.0, -3.0), fp / 310.0) - 0.5) * 1.1
            + (surface_noise_filtered(q.xz / 105.0 + vec2<f32>(-19.0, 41.0), fp / 105.0) - 0.5) * 0.55, 0.0, 1.0);
        // Saturated lush ramp (the pre-olive grass that read best in ground
        // captures), with vigour tint for patchy health.
        let meadow = mix(mix(vec3<f32>(0.055, 0.145, 0.038), vec3<f32>(0.132, 0.198, 0.058), 1.0 - wet),
            vec3<f32>(0.135, 0.175, 0.076), (1.0 - forest) * mottle * 0.42);
        let base_turf = mix(meadow, sand, arid * 0.18)
            * mix(vec3<f32>(1.14, 1.02, 0.78), vec3<f32>(0.90, 1.12, 0.86), vigor);
        let turf = mix(base_turf, vec3<f32>(0.30, 0.285, 0.075), arid * 0.26);
        let coastal_dune = (1.0 - smoothstep(12.2, 13.6 + mottle * 0.6, q.y)) * 0.24;
        let sparse = smoothstep(0.34, 0.88, arid) * 0.30;
        let top_coat = mix(turf, sand, max(coastal_dune, sparse));
        // Climate overlays below run on the field colour; the top/side texture
        // (blade tufts, or a soil bank under a turf fringe) is applied last.
        var turf_color = top_coat;
        let cold = 1.0 - smoothstep(0.08, 0.37, heat);
        let drift_noise = surface_noise_filtered(q.xz / 48.0 + vec2<f32>(19.3, -27.1), fp / 48.0) * 0.62
            + surface_noise_filtered(q.xz / 13.5 + vec2<f32>(-41.2, 13.7), fp / 13.5) * 0.38;
        let rot_chute = mat2x2<f32>(0.80, -0.60, 0.60, 0.80);
        let chute_val = surface_noise_filtered((rot_chute * q.xz) / 22.0 + vec2<f32>(31.4, -19.8), fp / 22.0);
        // Artistic drift variation is only a proxy. True concavity and exposure
        // require terrain data; noise is not a reconstruction of a couloir.
        let drift_pocket = (1.0 - abs(chute_val * 2.0 - 1.0)) * smoothstep(68.0, 125.0, q.y);
        let windward_drift = clamp(dot(normal.xz, vec2<f32>(-0.65, 0.76)), -1.0, 1.0) * 0.08;
        let slope_hold = smoothstep(0.36, 0.78, normal.y + (drift_noise - 0.5) * 0.20
            + windward_drift + drift_pocket * 0.14);
        let snow_field = cold * select(0.55, 0.18, cam.scene_seed.y >= 9u)
            + smoothstep(94.0, 154.0, q.y + (drift_noise - 0.5) * 24.0 + drift_pocket * 16.0) * 0.64
            + drift_noise * 0.24;
        let snow = smoothstep(0.46, 0.76, snow_field) * slope_hold;
        turf_color = mix(turf_color, vec3<f32>(0.80, 0.86, 0.91), snow);
        let scree_noise = surface_noise_filtered(q.xz / 26.0 + vec2<f32>(-14.7, 31.9), fp / 26.0) * 0.60
            + surface_noise_filtered(q.xz / 7.8 + vec2<f32>(23.4, -18.6), fp / 7.8) * 0.40;
        let exposure = 1.0 - smoothstep(0.46, 0.84, normal.y + (scree_noise - 0.5) * 0.20);
        let dip_coord = q.x * 0.038 + q.z * 0.029 + (drift_noise - 0.5) * 2.2;
        // Bounds include the derivatives of the warped coordinates.
        let strata = surface_noise_filtered(vec2<f32>(dip_coord, q.y * 0.16 + sin(dip_coord * 1.4) * 0.8), fp * 0.60) * 0.65
            + surface_noise_filtered(vec2<f32>(q.x * 0.13 - q.z * 0.09, q.y * 0.34 + mottle * 0.8), fp * 0.54) * 0.35;
        let limestone_raw = smoothstep(0.62, 0.92,
            sin(q.y * 0.085 + dip_coord * 1.1 + (drift_noise - 0.5) * 2.4) * 0.5 + 0.5);
        // The thresholded periodic band contains harmonics; filter the whole
        // response to a representative mean, not only the sine's base frequency.
        let limestone_band = mix(0.3156, limestone_raw, surface_detail_weight(fp * 0.32));
        let rock_base = mix(vec3<f32>(0.44, 0.46, 0.47), vec3<f32>(0.50, 0.44, 0.36), heat);
        let mineral = mix(rock_base, vec3<f32>(0.28, 0.27, 0.24), limestone_band * 0.20) * (0.90 + strata * 0.20);
        let alpine_rock = smoothstep(52.0, 108.0, q.y + (scree_noise - 0.5) * 20.0)
            * smoothstep(0.10, 0.48, 1.0 - normal.y + (scree_noise - 0.5) * 0.16);
        // Every 1-voxel terrace riser is a vertical face. Below the foothills the
        // generator places real rock where ground is steep, so the shader must not
        // also grey every riser: that drew tan bands across lowland slopes.
        let riser_rock = exposure * 0.36 * smoothstep(26.0, 64.0, q.y);
        let exposed = clamp(max(riser_rock, alpine_rock * 0.68), 0.0, 1.0);
        turf_color = mix(turf_color, mineral * (0.86 + mottle * 0.28), exposed * (1.0 - snow * 0.78));
        let heath = smoothstep(0.60, 0.82, mottle) * (1.0 - forest) * 0.22;
        turf_color = mix(turf_color, vec3<f32>(0.30, 0.43, 0.25) * 0.86, heath * up * (1.0 - snow));
        turf_color = mix(turf_color, soil, smoothstep(0.20, 0.46, clumps) * 0.20 * up * (1.0 - snow));
        let marsh = smoothstep(0.65, 0.88, wet) * (1.0 - smoothstep(18.0, 30.0, q.y)) * smoothstep(14.5, 16.5, q.y);
        turf_color = mix(turf_color, vec3<f32>(0.30, 0.43, 0.25) * 0.70, marsh * up * 0.34 * (1.0 - coastal_dune));
        let heather = cool * (1.0 - forest) * smoothstep(0.62, 0.82, mottle);
        turf_color = mix(turf_color, vec3<f32>(0.42, 0.32, 0.40), heather * up * 0.16 * (1.0 - snow));
        if cam.scene_seed.y >= 10u {
            let savanna = smoothstep(0.50, 0.65, heat) * (1.0 - smoothstep(0.38, 0.53, wet));
            let rainforest = smoothstep(0.50, 0.65, heat) * smoothstep(0.60, 0.78, wet);
            turf_color = mix(turf_color, sand * 0.82, savanna * up * 0.24 * (1.0 - coastal_dune));
            turf_color = mix(turf_color, vec3<f32>(0.30, 0.43, 0.25) * 0.82, rainforest * up * 0.42);
        }
        // Break the meadow with low-frequency worn soil so the ground has an
        // authored patch language instead of one continuous pastel green.
        let worn = smoothstep(
            0.68,
            0.90,
            surface_noise_filtered(q.xz * 1.7 + vec2<f32>(23.0, -11.0), fp * 1.7),
        ) * up * (1.0 - snow);
        turf_color = mix(turf_color, soil, worn * 0.16);
        // Broad dry patches (several blocks) choose where tuft tips straw.
        let dry_field = smoothstep(0.58, 0.78,
            surface_noise_filtered(q.xz / 6.5 + vec2<f32>(17.0, 5.0), fp / 6.5)) * (1.0 - snow);
        // Meadow flowers in patches. A low-frequency field decides where they
        // grow; inside a patch a few grass tufts carry one bloom at their heart
        // (grass_top), so flowers belong to plants instead of speckling texels.
        let flower_patch = smoothstep(0.56, 0.68,
            surface_noise_filtered(q.xz / 9.0 + vec2<f32>(-23.0, 17.0), fp / 9.0));
        // Deepen the meadow's authored olive at full daylight. Snow and dry
        // coastal sand keep their own values.
        turf_color *= mix(vec3<f32>(0.80, 0.92, 0.74), vec3<f32>(1.0), snow);
        if normal.y > 0.5 && turf_exposed {
            turf_color = grass_top(turf_color, q, fp, dry_field * (0.35 + arid * 0.65),
                flower_patch * (1.0 - snow) * (1.0 - forest * 0.5));
        } else {
            let bank = soil_surface(mix(soil * 0.86, top_coat, coastal_dune), q, normal, fp);
            if turf_exposed {
                turf_color = grass_side(bank, turf_color, p, q, normal, fp);
            } else { turf_color = bank; }
        }
        color += turf_color * weights.x;
    }
    if weights.y > 0.0 {
        // Keep the authored Dirt / Mud / Clay hue instead of replacing it with
        // one shared brown, and add a close-range grit break-up.
        let earth = soil_surface(mix(base, soil, 0.46), q, normal, fp);
        color += earth * weights.y;
    }
    if weights.z > 0.0 {
        let bedding = surface_noise_filtered(vec2<f32>(q.x * 0.11 + q.z * 0.07, q.y * 0.42 + mottle * 0.8), fp * 0.60);
        var rock = base * mix(vec3<f32>(0.98, 1.00, 1.03), vec3<f32>(1.06, 1.02, 0.96), heat)
            * (0.96 + bedding * 0.28);
        rock = rock_surface(rock, q, normal, fp);
        let rock_face = natural_face_uv(q, normal);
        // Lichen: hard pixel blobs in pale grey-green, on sunward and upper faces.
        let lichen_field = surface_noise_filtered(q.xz / 2.7 + vec2<f32>(44.0, -9.0), fp / 2.7);
        let lichen_zone = smoothstep(0.62, 0.70, lichen_field)
            * step(-0.2, normal.y + 0.25 * (heat - 0.5));
        let lichen = pixel_blob(rock_face, PIXEL_RES, vec2<f32>(61.0, 17.0), 0.10, fp) * lichen_zone;
        rock = mix(rock, vec3<f32>(0.50, 0.54, 0.40) * (0.92 + bedding * 0.12), lichen * 0.55);
        color += rock * weights.z;
    }
    if weights.w > 0.0 {
        let legacy_moisture = 1.0 - smoothstep(11.8, 12.95 + mottle * 0.35, q.y);
        let t = light.torch_meta.y;
        // Generator 21 beaches carry a live swash band: a run-up line breathes
        // over the intertidal sand instead of a fixed wet/dry threshold.
        let alongshore = dot(q.xz, vec2<f32>(0.42, 0.28));
        let run_up = 0.5 + 0.5 * sin(alongshore * 0.095 + t * 0.82 + mottle * 2.1);
        let swash_line = 12.0 + 0.12 + 0.62 * run_up + (mottle - 0.5) * 0.22;
        let swash = 1.0 - smoothstep(swash_line - 0.08, swash_line + 0.58, q.y);
        let shore_moisture = select(
            legacy_moisture,
            clamp(max(legacy_moisture * 0.72, swash), 0.0, 1.0),
            cam.scene_seed.y >= 22u,
        );
        let cusp = 0.5 + 0.5 * sin(dot(q.xz, vec2<f32>(0.83, -0.41)) * 0.42 + mottle * 3.0);
        let dry_sand = mix(base, sand, 0.48) * (0.98 + mottle * 0.08 + (cusp - 0.5) * 0.035);
        let wet_sand = dry_sand * vec3<f32>(0.68, 0.66, 0.61);
        var sand_color = mix(dry_sand, wet_sand, shore_moisture * 0.78);
        // Bound the warped ripple phase before evaluating the expensive detail.
        let ripple_weight = surface_detail_weight(fp * 8.0);
        if ripple_weight > 0.0 {
            let ripple_coord = q.xz * vec2<f32>(0.85, 0.52)
                + vec2<f32>(surface_noise_filtered(q.xz * 0.18, fp * 0.18) * 2.4, mottle * 3.2);
            let phase = (ripple_coord.x + ripple_coord.y) * 18.0;
            sand_color *= 1.0 + sin(phase) * 0.028 * ripple_weight * (1.0 - shore_moisture * 0.72);
        }
        color += sand_color * weights.w;
    }
    let modulation = 0.92 + mottle * 0.16 + clumps * 0.06;
    let mixed = max(base * (1.0 - coverage) + color * modulation, vec3<f32>(0.0));
    // Shared natural-material finish: lift saturation and preserve contrast.
    // Without this the independently mixed dirt, stone and sand layers drift
    // toward the same grey-brown average at grazing angles.
    let natural_luma = dot(mixed, vec3<f32>(0.2126, 0.7152, 0.0722));
    // A slightly stronger hue separation keeps the Noita-like material ramps
    // from averaging into a pale mint under high daytime exposure.
    return max(mix(vec3<f32>(natural_luma), mixed, 1.09) * 0.985, vec3<f32>(0.002));
}

fn natural_surface(base: vec3<f32>, p: vec3<f32>, normal: vec3<f32>, kind: i32, footprint: f32) -> vec3<f32> {
    if kind == 33 {
        let q = (floor(p * cam.world_params.x * PIXEL_RES) + vec3<f32>(0.5)) / PIXEL_RES;
        return max(gravel_surface(base, q, normal, max(footprint * cam.world_params.x, 0.0)), vec3<f32>(0.002));
    }
    if kind == 16 {
        // Snapped to texel centres: every noise below is flat inside a texel.
    let q = (floor(p * cam.world_params.x * PIXEL_RES) + vec3<f32>(0.5)) / PIXEL_RES;
        let mottle = surface_noise_filtered(q.xz / 17.0 + q.y * vec2<f32>(0.017, 0.027), footprint * cam.world_params.x * 0.068);
        return max(mix(base, vec3<f32>(0.25, 0.18, 0.33), smoothstep(0.42, 0.72, mottle) * 0.4)
            * (0.90 + mottle * 0.20), vec3<f32>(0.002));
    }
    let weights = vec4<f32>(
        select(0.0, 1.0, kind == 0 || kind == 1 || kind == 7 || kind == 36),
        select(0.0, 1.0, kind == 2), select(0.0, 1.0, kind == 3), select(0.0, 1.0, kind == 4));
    return natural_surface_blended(base, p, normal, weights, footprint, kind != 36);
}

// Sparse twinkling pinpricks for void stone (52) and star glass (53). Lives in
// metres, so the sky of a cosmos chamber has one density at every voxel scale.
fn void_star_mask(p: vec3<f32>, normal: vec3<f32>, kind: i32, t: f32) -> f32 {
    var uv = p.xz;
    if abs(normal.x) > 0.5 { uv = p.zy; }
    else if abs(normal.z) > 0.5 { uv = p.xy; }
    let q = uv * cam.world_params.x * select(5.0, 8.0, kind == 53);
    let cell = floor(q);
    let jitter = vec2<f32>(surface_hash(cell + vec2<f32>(3.7, 1.3)), surface_hash(cell + vec2<f32>(11.1, 7.9)));
    let d = length(fract(q) - (vec2<f32>(0.25) + jitter * 0.5));
    let present = step(select(0.84, 0.60, kind == 53), surface_hash(cell));
    let rate = 0.35 + surface_hash(cell + vec2<f32>(5.0)) * 0.55;
    let twinkle = 0.88 + 0.12 * sin(t * rate + surface_hash(cell) * 40.0);
    return present * (1.0 - smoothstep(0.03, 0.11, d)) * twinkle;
}

// Crystal shards: a Voronoi of ~30 cm facets sampled at texel centres, so every
// boundary lands on the 5 cm pixel grid. Returns (facet tone, light gradient
// across the facet, one-texel edge flag, per-facet edge strength). A coherent
// shard reads as cut stone; per-texel random tones read as static.
fn crystal_facets(uv: vec2<f32>) -> vec4<f32> {
    let q = uv * PIXEL_RES / CRYSTAL_FACET_TEXELS;
    let cell = floor(q);
    let local = fract(q);
    // Distance order is unchanged by squaring. Resolve only the two winning
    // distances below: two square roots instead of nine per crystal fragment.
    var d1 = 64.0;
    var d2 = 64.0;
    var id = cell;
    var toward = vec2<f32>(0.0);
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let o = vec2<f32>(f32(i), f32(j));
            let seed = o + vec2<f32>(surface_hash(cell + o), surface_hash(cell + o + vec2<f32>(19.0))) * 0.8 + 0.1;
            let delta = seed - local;
            let d = dot(delta, delta);
            if d < d1 {
                d2 = d1;
                d1 = d;
                id = cell + o;
                toward = delta;
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    let tone = surface_hash(id + vec2<f32>(4.4, 9.1));
    // Light falls across each shard from its upper-left to its lower-right.
    let gradient = clamp(0.5 - dot(toward, vec2<f32>(0.6, 0.8)) * 0.9, 0.0, 1.0);
    // Recover the Euclidean gap to retain the existing cut-edge threshold.
    let edge = step(sqrt(d2) - sqrt(d1), 0.9 / CRYSTAL_FACET_TEXELS);
    return vec4<f32>(tone, gradient, edge, 0.5 + 0.5 * surface_hash(id + vec2<f32>(7.7, 1.3)));
}

// Shared by colour and BRDF: only exposed mineral catches light, never its host rock.
fn ore_inclusion(uv_m: vec2<f32>, footprint_m: f32) -> f32 {
    let uv = texel_centre(uv_m);
    let deposit = surface_noise(uv * vec2<f32>(4.0, 2.0));
    let fleck = surface_hash(floor(uv * PIXEL_RES));
    let resolved = surface_detail_weight(footprint_m * PIXEL_RES);
    // Preserve average mineral coverage when individual pixels cannot resolve.
    return mix(0.16, smoothstep(0.53, 0.66, deposit)
        * smoothstep(0.22, 0.42, fleck), resolved);
}

fn block_surface(base: vec3<f32>, p: vec3<f32>, normal: vec3<f32>, kind: i32, footprint: f32) -> vec3<f32> {
    if kind==0 || kind==1 || kind==2 || kind==3 || kind==4 || kind==7 || kind==16 || kind==33 { return natural_surface(base,p,normal,kind,footprint); }
    if kind==8 {return base;}
    let landscape_p = p.xz * cam.world_params.x;
    var uv = p.xz;
    if abs(normal.x) > 0.5 { uv = p.zy; }
    else if abs(normal.z) > 0.5 { uv = p.xy; }
    let metric_footprint=footprint*cam.world_params.x;
    let fine_fade = surface_detail_weight(metric_footprint * 8.0);
    let medium_fade = surface_detail_weight(metric_footprint * 4.0);
    let physical_uv = texel_centre(uv * cam.world_params.x);
    // Meter-scale color survives when individual blades and voxel faces no
    // longer resolve. The same continuous field spans every mesh/LOD boundary.
    let broad = surface_noise_filtered(landscape_p / 83.0, metric_footprint / 83.0);
    let landscape_patch = surface_noise_filtered(landscape_p / 17.0 + vec2<f32>(31.0, -12.0), metric_footprint / 17.0);
    let ridge = surface_noise_filtered(landscape_p / 5.2 + vec2<f32>(-73.0, 22.0), metric_footprint / 5.2);
    let land_tone = broad * 0.50 + landscape_patch * 0.32 + ridge * 0.18;
    // Keep continuous metre coordinates up close. Snapping onto a 16-sample
    // grid flattened mortar, grain and leaf shapes at every playable distance.
    uv = physical_uv;
    // Hard 20-cell/metre grain. This is one pixel per 5 cm, shared by every
    // built material, rather than a smooth 8-cell noise field that softens
    // into a high-resolution haze when the camera moves.
    let grain_cell = floor(physical_uv * PIXEL_RES);
    let grain = (surface_hash(grain_cell) - 0.5)
        * surface_detail_weight(metric_footprint * 20.0);
    let mottling = (surface_noise_filtered(physical_uv * 3.0, metric_footprint * 3.0) - 0.5);
    // A second, independent per-texel hash. This used to be a 7-per-metre
    // lattice: blocks three texels wide beside one-texel grain.
    let mineral_cell = floor(physical_uv * PIXEL_RES) + vec2<f32>(19.0, 53.0);
    let mineral_grain = (surface_hash(mineral_cell) - 0.5)
        * surface_detail_weight(metric_footprint * PIXEL_RES);
    var color = base * (1.0 + grain * 0.24 + mottling * 0.06 + mineral_grain * 0.09);
    if kind == 5 {
        if abs(normal.y) > 0.5 {
            let center = fract(uv) - vec2<f32>(0.5);
            let rings = sin(length(center) * 70.0 + surface_noise(uv * 6.0) * 2.0) * fine_fade;
            color = base * vec3<f32>(1.32, 1.22, 1.05) * (1.0 + rings * 0.18 + grain * 0.2);
        } else {
            let bark = surface_noise(vec2<f32>(uv.x * 13.0, uv.y * 2.0));
            let furrow = smoothstep(0.22, 0.46, bark);
            color *= mix(1.0, 0.58 + furrow * 0.57, surface_detail_weight(metric_footprint * 13.0));
        }
    } else if kind == 6 {
        let clusters = surface_noise(uv * 4.0);
        let grove = surface_noise_filtered(landscape_p / 34.0 + vec2<f32>(8.0, 61.0), metric_footprint / 34.0);
        let pine = smoothstep(-0.02, 0.02, base.b - base.r);
        let autumn = vec3<f32>(0.55, 0.34, 0.18);
        let broadleaf = mix(base, autumn, smoothstep(0.68, 0.90, grove) * 0.40);
        let needles = base * mix(0.86, 1.02, land_tone);
        let leaf = fract(uv * 4.0) - 0.5;
        let leaf_shape = 1.0-smoothstep(0.34, 0.48, abs(leaf.x + leaf.y * 0.35) + abs(leaf.y) * 0.65);
        let leaf_light = select(-0.08, 0.10, leaf.x > -leaf.y * 0.35) * leaf_shape;
        // Hard one-texel leaf pixels give the canopy the same authored
        // speckle language as ground cover, without the old smooth noise wash.
        let leaf_cell = floor(physical_uv * PIXEL_RES) + vec2<f32>(5.0, -3.0);
        let leaf_px = (surface_hash(leaf_cell) - 0.5)
            * surface_detail_weight(metric_footprint * PIXEL_RES);
        let leaf_pixel_hue = surface_hash(leaf_cell + vec2<f32>(13.2, -8.4));
        let leaf_tint = mix(
            vec3<f32>(0.92, 1.02, 0.90),
            vec3<f32>(1.05, 1.00, 0.88),
            leaf_pixel_hue,
        );
        // Canopy stays on the authored leaf hue. Autumn is the copper swatch.
        // Per-texel tint and tone fade toward their own means, not toward 1:
        // fading to 1 shifted unresolved faces about 18% bluer than resolved
        // faces, so the detail fade showed up as a hue seam across a canopy.
        let leaf_resolved = surface_detail_weight(metric_footprint * PIXEL_RES);
        color = mix(broadleaf, needles, pine) * (1.04 + (clusters-0.5) * 0.16 * medium_fade
            + leaf_light * fine_fade + grain * 0.10 + leaf_px * 0.16)
            * mix(vec3<f32>(1.0), mix(LEAF_TINT_MEAN, leaf_tint, leaf_resolved), 0.45);
        // Per-leaf hue drift: some leaves run yellow-green, some cool and dark.
        // Stays within the canopy's hue family and fades with the footprint.
        let leaf_drift = surface_hash(leaf_cell + vec2<f32>(13.1, -5.3));
        let leaf_tone = mix(vec3<f32>(1.10, 1.03, 0.72), vec3<f32>(0.84, 0.96, 0.93), leaf_drift);
        color *= mix(vec3<f32>(1.0), mix(LEAF_TONE_MEAN, leaf_tone, leaf_resolved), 0.8);
    } else if kind == 9 {
        color *= 1.0 + mottling * 0.2;
    } else if kind == 11 {
        let plank_uv = uv * vec2<f32>(1.0, 4.0);
        let local = fract(plank_uv);
        // Seams are exactly one texel: a plank is 5 texels tall and 20 long.
        let seam_x = 1.0 - step(0.04, min(local.x, 1.0 - local.x));
        let seam_y = 1.0 - step(0.12, min(local.y, 1.0 - local.y));
        let seam = max(seam_x, seam_y);
        let wood = sin(uv.x*80.0 + surface_noise(uv*vec2<f32>(3.0,18.0))*6.0) * fine_fade;
        let board = mix(0.5, surface_hash(floor(plank_uv)), medium_fade);
        color = base * (0.88+board*0.22+wood*0.075) * (1.0-seam*0.48*medium_fade);
        // Each board takes its own warm/cool tint, so a run of planks does not
        // read as one repeated texture. Scaled by the same fade as the grain.
        let board_tint = (surface_hash(floor(plank_uv) + vec2<f32>(3.3, 9.1)) - 0.5) * 2.0;
        color *= vec3<f32>(1.0 + board_tint * 0.05 * medium_fade, 1.0, 1.0 - board_tint * 0.06 * medium_fade);
    } else if kind == 12 {
        // Cut stone: two bricks per metre, four courses, running bond.
        // Joints are measured in continuous metres so they stay open next to the camera.
        let courses = uv.y * 4.0;
        let row = floor(courses);
        let row_phase = row - 2.0 * floor(row * 0.5);
        let brick_uv = vec2<f32>(uv.x * 2.0 + row_phase * 0.5, courses);
        let cell = floor(brick_uv);
        let local = fract(brick_uv);
        let edge = min(min(local.x, 1.0 - local.x), min(local.y, 1.0 - local.y));
        let joint = 0.12;
        let mortar = 1.0 - smoothstep(joint, joint + 0.028, edge);
        let bevel = smoothstep(0.0, 0.2, edge);
        let id = surface_hash(cell);
        let speckle = surface_noise(uv * vec2<f32>(19.0, 14.0) + id * 11.0);
        let chip = smoothstep(0.76, 0.92, speckle);
        let crack = smoothstep(0.84, 0.93, surface_noise(vec2<f32>(local.x * 2.4 + id * 5.0, local.y * 0.35)));
        let weathered = smoothstep(0.045, 0.11, base.g - base.r);
        let tint = mix(vec3<f32>(0.84, 0.82, 0.76), vec3<f32>(1.12, 1.07, 0.98), id);
        let moss = vec3<f32>(0.30, 0.38, 0.22);
        color = base * tint * (0.70 + bevel * 0.36 + (speckle - 0.5) * 0.08);
        color = mix(color, color * vec3<f32>(0.58, 0.54, 0.50), (chip * 0.55 + crack * 0.22) * (1.0 - mortar));
        let mortar_col = mix(base * vec3<f32>(0.46, 0.42, 0.37), moss, weathered * (0.45 + id * 0.35));
        let joint_keep = 1.0 - smoothstep(0.16, 0.6, metric_footprint);
        color = mix(color, mortar_col, mortar * joint_keep);
        let stain = smoothstep(0.58, 0.86, surface_noise(cell * 0.41 + uv * 1.3));
        color = mix(color, moss, stain * weathered * (1.0 - mortar) * 0.34);
        // Filter the complete brick response: cell IDs, chips, cracks, and
        // bevels also alias after mortar becomes subpixel. This mean is an
        // authored approximation, pending a baked material mip chain.
        let stone_mean = mix(base * vec3<f32>(0.84, 0.82, 0.76), moss, weathered * 0.08);
        color = mix(stone_mean, color, surface_detail_weight(metric_footprint * 4.0));
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
        color *= (1.0-joint*0.45*medium_fade)*(1.0+mottling*0.4);
    } else if kind == 14 || kind == 15 || kind == 20 || kind == 21 {
        let deposit = surface_noise(uv * vec2<f32>(4.0, 2.0));
        let fleck = surface_hash(floor(uv * PIXEL_RES));
        let ore = ore_inclusion(physical_uv, metric_footprint);
        let ore_light = 1.02 + (fleck - 0.5) * 0.38 * surface_detail_weight(metric_footprint * PIXEL_RES);
        var ore_color = select(vec3<f32>(0.018,0.02,0.022),vec3<f32>(0.34,0.16,0.07),kind==15);
        if kind == 20 { ore_color = vec3<f32>(0.46,0.22,0.075); }
        if kind == 21 { ore_color = vec3<f32>(0.42,0.55,0.62); }
        color = mix(color,ore_color*ore_light,ore*0.92);
        let rim=smoothstep(0.47,0.56,deposit)*medium_fade*(1.0-ore);
        color*=1.0-rim*0.22;
    } else if kind == 17 {
        color *= 0.8 + surface_noise_filtered(vec2<f32>(uv.x*12.0,uv.y*2.0), metric_footprint*12.0)*0.35;
    } else if kind == 37 {
        // Amber gills: fine diagonal lamellae, no cap spots.
        let lamella = step(0.5, fract((uv.x + uv.y) * 10.0));
        color *= mix(0.9, 1.08, lamella * surface_detail_weight(metric_footprint * 10.0));
    } else if kind == 18 || kind == 19 {
        let spot_cell=floor(uv*4.0);
        let spot_pos=fract(uv*4.0)-0.5;
        let spots=(1.0-smoothstep(0.2,0.32,length(spot_pos)))*step(0.3,surface_hash(spot_cell));
        color = mix(color,vec3<f32>(0.84,0.79,0.61),spots*medium_fade);
        if normal.y < -0.5 {
            let gills=step(0.48,fract((uv.x+uv.y)*14.0));
            color=base*mix(0.9,mix(0.6,1.2,gills),surface_detail_weight(metric_footprint*12.0))* (1.0+grain*0.15);
        }
    } else if kind == 22 {
        // Pixel crystal: coherent shards with a deep-indigo to pale-lilac ramp,
        // light running across each shard and a bright one-texel cut edge.
        // Filtered out with distance, so far crystals keep their average colour.
        if medium_fade > 0.0 {
            let f = crystal_facets(uv);
            let shade = clamp(0.10 + f.x * 0.30 + (f.y - 0.5) * 0.45, 0.0, 1.0);
            let shadow = color * vec3<f32>(0.34, 0.26, 0.90) * 0.55;
            let light = mix(color, vec3<f32>(0.90, 0.68, 1.0), 0.30);
            var shard = mix(shadow, light, shade);
            // A one-texel cut edge disappears before the broad facet does.
            // Keeping both on the facet-scale filter made thin lilac edges
            // flicker at grazing angles and overpowered the underlying stone.
            let edge_fade = surface_detail_weight(metric_footprint * PIXEL_RES);
            shard = mix(shard, vec3<f32>(0.66, 0.60, 1.0), f.z * f.w * f.w * 0.42 * edge_fade);
            color = mix(color, shard, medium_fade);
        }
    } else if kind == 52 {
        // Void stone: nearly black, with a violet marbling that only shows in torchlight.
        let marble = surface_noise_filtered(physical_uv * 1.7 + vec2<f32>(7.0, 3.0), metric_footprint * 1.7);
        color = mix(base * 0.78, vec3<f32>(0.26, 0.18, 0.28), marble * 0.42);
    } else if kind == 53 {
        // Star glass: faceted tiles with a cold sheen that slides across them.
        let cell = floor(physical_uv * PIXEL_RES);
        let tile = cell - floor(cell / 10.0) * 10.0;
        let seam = max(1.0 - step(1.0, tile.x), 1.0 - step(1.0, tile.y));
        let sheen = surface_noise_filtered(physical_uv * 3.0 + vec2<f32>(2.0, 9.0), metric_footprint * 3.0);
        color = base * (0.7 + sheen * 1.2) * (1.0 - seam * 0.6 * medium_fade);
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
    if kind == 55 {
        // First-person wand body: wood/leather/crystal pixels with a low-frequency
        // grain and a dark band every few centimetres. The grain is quantized to
        // five steps so it reads as authored pixel art instead of smooth noise.
        // Solid material, not the glow-particle path, so it keeps directional
        // shading and texture.
        let wand_step = round((surface_hash(floor(physical_uv * PIXEL_RES)) - 0.5) * 4.0) * 0.25;
        let band = 1.0 - smoothstep(0.06, 0.16, abs(fract(physical_uv.y * 9.0) - 0.5));
        color *= 1.0 + wand_step * 0.18 * medium_fade;
        color = mix(color, color * vec3<f32>(0.70, 0.58, 0.46), band * 0.28 * medium_fade);
    }
    if kind == 54 {
        // Warm skin/leather for the first-person hand. A quantized 14-cell/m
        // grain and a dark crease grid keep it in the same pixel language as
        // voxels and the shared six-role UI sprites.
        let skin_cell = floor(physical_uv * PIXEL_RES);
        let skin_step = round((surface_hash(skin_cell) - 0.5) * 4.0) * 0.25;
        let crease = 1.0 - smoothstep(0.035, 0.085, min(fract(physical_uv * 7.0).x,
            fract(physical_uv * 7.0).y));
        color *= 1.0 + skin_step * 0.16 * medium_fade;
        color = mix(color, color * vec3<f32>(0.72, 0.58, 0.48), crease * 0.22 * medium_fade);
    }
    if kind==41 {
        // Brushed enamel, edge wear and small pits on moving machinery.
        let brush=surface_hash(floor(physical_uv*vec2<f32>(10.0,48.0)))-0.5;
        let scratches=step(0.95,surface_hash(floor(physical_uv*vec2<f32>(32.0,8.0))));
        color*=1.0+brush*0.14*surface_detail_weight(metric_footprint*48.0)
            +(scratches-0.05)*0.16*surface_detail_weight(metric_footprint*32.0)+0.007;
        let patina=smoothstep(0.64,0.86,surface_noise(physical_uv*5.0));
        let pits=step(0.985,surface_hash(floor(physical_uv*PIXEL_RES)));
        color=mix(color,color*vec3<f32>(0.65,0.78,0.73),patina*0.25*medium_fade);
        color*=1.0-0.00375-(pits-0.015)*0.28*surface_detail_weight(metric_footprint*32.0);
    }
    // Keep authored hue separation on built and mined surfaces too. The
    // texture branches above each add their own value noise; this shared finish
    // prevents low-luminance branches (grain, mortar, bark, ore rims) from
    // averaging everything into the same dull brown-grey.
    let block_luma = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    color = mix(vec3<f32>(block_luma), color, 1.05) * 1.018;
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
    let relative_pos = in.relative_pos;
    let world_pos = cam.camera_pos.xyz + relative_pos;
    let world_dx = dpdx(relative_pos);
    let world_dy = dpdy(relative_pos);
    // Angle one screen pixel subtends, for pixel-exact ambient motes.
    let view_ray = normalize(relative_pos);
    let ambience_pixel = max(length(dpdx(view_ray)), length(dpdy(view_ray)));
    let scale = max(cam.world_params.x, 1e-6);
    let fp_m = terrain_pixel_footprint(world_dx, world_dy);
    let model_fp = max(length(dpdx(in.model_uv)), length(dpdy(in.model_uv))) * PIXEL_RES;
    var albedo = clamp(in.base_color, vec3<f32>(0.0), vec3<f32>(1.0));
    let kind = i32(in.material_family);
    // Celestial bodies are projected from f64 planet-centred ephemerides into
    // the observer tangent frame. They do not belong to terrain coverage or
    // ground shadow cascades. Positive AO >= 4 carries their eclipse fraction.
    if in.ao_strength >= 4.0 {
        let radius = 6371000.0;
        let observer_radius = radius + max(cam.camera_pos.y, 0.0);
        let distance = length(relative_pos);
        let b = observer_radius * view_ray.y;
        let discriminant = b*b - (observer_radius-radius)*(observer_radius+radius);
        if b < 0.0 && discriminant >= 0.0 {
            let hit = -b-sqrt(discriminant);
            if hit >= 0.0 && hit < distance { discard; }
        }
        let sunlight = clamp(in.ao_strength-4.0,0.0,1.0);
        let normal = normalize(in.normal);
        let lat = -cam.camera_pos.z/radius;
        let lon = cam.camera_pos.x/radius;
        let reference_sun = normalize(light.sun_dir_intensity.xyz);
        let east = vec3<f32>(cos(lon),-sin(lon),0.0);
        let up = vec3<f32>(sin(lon)*cos(lat),cos(lon)*cos(lat),-sin(lat));
        let south = vec3<f32>(sin(lon)*sin(lat),cos(lon)*sin(lat),cos(lat));
        let sun = normalize(vec3<f32>(dot(reference_sun,east),dot(reference_sun,up),dot(reference_sun,south)));
        let grain = (surface_hash(floor(in.model_uv*PIXEL_RES))-0.5)*0.07;
        let base = albedo*(1.0+grain*surface_detail_weight(model_fp));
        var orbital_light = base*(0.006 + sunlight*max(dot(normal,sun),0.0)*2.5);
        if kind == 40 { orbital_light = albedo*max(in.surface.w,0.0)*0.35; }
        // Integrate only the atmosphere crossed by the ray, not hundreds of
        // kilometres of fictitious ground fog. Spherical density falls with
        // actual radial altitude. A finite top-of-air intersection bounds work.
        let top_radius = radius+120000.0;
        let top_d = b*b + top_radius*top_radius-observer_radius*observer_radius;
        let air_end = min(distance,max(-b+sqrt(max(top_d,0.0)),0.0));
        var rayleigh_column = 0.0;
        var aerosol_column = 0.0;
        // Quadratic intervals resolve dense air near a surface observer.
        for (var i=0u; i<16u; i=i+1u) {
            let lo = f32(i)/16.0; let hi = f32(i+1u)/16.0;
            let a = air_end*lo*lo; let z = air_end*hi*hi;
            let point = vec3<f32>(0.0,observer_radius,0.0)+view_ray*((a+z)*0.5);
            let altitude = max(length(point)-radius,0.0);
            rayleigh_column += exp(-altitude/8000.0)*(z-a);
            aerosol_column += exp(-altitude/1200.0)*(z-a);
        }
        let transmission = exp(-vec3<f32>(5.8e-6,13.5e-6,33.1e-6)*rayleigh_column
            -vec3<f32>(3.0e-6)*aerosol_column);
        let sky = max(atmosphere_color(view_ray),vec3<f32>(0.0));
        let through_air = orbital_light*transmission+sky*(vec3<f32>(1.0)-transmission);
        let cloud = clamp(cloud_fog_amount(view_ray,distance),0.0,1.0);
        return WorldOutput(vec4<f32>(mix(through_air,sky,cloud),1.0),vec4<f32>(0.0));
    }
    // Authored rigid bodies share the vertex-colour/UV path, with their own finish.
    let cut_crystal = kind == 56 || kind == 57 || kind == 58;
    let enemy_metal = kind == 59 || kind == 60;
    // Orbital rigid parts: baked local UVs; w selects ceramic/alloy/radiator.
    let orbital_part = kind == 62 || kind == 63;
    let footprint = fp_m / scale;
    let material_tint=in.material_tint;
    let material_props=in.material_props;
    var roughness = clamp(select(in.surface.x,material_tint.w,material_tint.w>=0.0), 0.08, 1.0);
    var reflectance = clamp(material_props.x,0.0,1.0);
    if orbital_part {
        roughness = select(select(0.74, 0.35, in.surface.w > 0.25), 0.48, in.surface.w > 0.75);
        reflectance = 0.5;
    }
    if cut_crystal { roughness = select(0.24, 0.84, kind == 57); reflectance = 0.5; }
    if kind == 58 { roughness = 0.82; reflectance = 0.24; }
    if enemy_metal { roughness = select(0.68, 0.46, kind == 60); reflectance = 0.38; }
    // First-person skin/leather and wand wood are deliberately matte. Their
    // default material family has no PBR override, so leaving them in the
    // generic environment path produced a view-dependent sky sheen that read as
    // a wet or metallic arm in caves. Keep the authored diffuse colour and
    // torch response, but suppress the environment lobe entirely.
    if kind == 54 || kind == 55 {
        roughness = 0.96;
        reflectance = 0.0;
    }
    if kind == 40 {
        // Glow particles use surface.y as a dissolve threshold, not as a
        // specular gain. Keep their shading emissive and non-metallic.
        roughness = 1.0;
        reflectance = 0.0;
    }
    let is_torch_material = in.surface.w < 0.0;
    // Regular resident vertices encode cached overhead visibility in [4,5].
    // Preserve dedicated torch/fire metadata and unencoded horizon geometry.
    var sky_access = select(1.0, clamp(in.surface.w - 4.0, 0.006, 1.0),
        in.surface.w >= 4.0 && kind != 40
            && terrain_has_resident_metadata(in.ao_strength));
    if kind == 8 && in.surface.w >= 8.0 && terrain_has_resident_metadata(in.ao_strength) {
        sky_access = in.water_optics.y;
    }

    let geometric_normal = terrain_safe_unit3(in.normal, vec3<f32>(0.0, 1.0, 0.0));
    var n = geometric_normal;
    var filtered_slope_variance = 0.0;
    let is_backdrop = terrain_is_backdrop(in.ao_strength);
    let is_natural = kind == 0 || kind == 1 || kind == 2 || kind == 3
        || kind == 4 || kind == 7 || kind == 16 || kind == 33 || kind == 36;
    let material_scale = max(material_props.y, 0.0625);
    // The held hand and wand are rigid with the camera, but their mesh is
    // rebuilt in world space every frame. World-anchored procedural patterns
    // therefore crawl across them as the player moves, and the planar-UV axis
    // choice flips mid-face as the world normal rotates past 45 degrees, which
    // pops whole triangles. Evaluate their patterns in camera space instead,
    // using both a camera-space position and a camera-space normal.
    let is_viewmodel = kind == 54 || kind == 55;
    let vm_right = terrain_safe_unit3(cam.ray_right.xyz, vec3<f32>(1.0, 0.0, 0.0));
    let vm_up = terrain_safe_unit3(cam.ray_up.xyz, vec3<f32>(0.0, 1.0, 0.0));
    let vm_forward = terrain_safe_unit3(cam.camera_forward.xyz, vec3<f32>(0.0, 0.0, -1.0));
    let vm_position = vec3<f32>(dot(relative_pos, vm_right), dot(relative_pos, vm_up),
        dot(relative_pos, vm_forward)) / scale;
    let vm_normal = vec3<f32>(dot(geometric_normal, vm_right), dot(geometric_normal, vm_up),
        dot(geometric_normal, vm_forward));
    let pattern_position = select(in.voxel_pos, vm_position, is_viewmodel);
    let pattern_normal = select(geometric_normal, vm_normal, is_viewmodel);
    let detail_strength = clamp(material_props.z, 0.0, 2.0);
    var detail_color = albedo;
    natural_tilt = vec3<f32>(0.0);
    // Distant terrain shades with the detail map's texel normal (16-128 m)
    // instead of the coarse lattice normal, so ridges, spurs and gullies read
    // at any range. Resident terrain keeps its own geometry normals; both get
    // the same broad cavity term, faded in with distance, so the hand-over
    // between them shows no brightness step.
    var macro_normal = geometric_normal;
    var macro_occlusion = 1.0;
    if is_natural && !is_viewmodel {
        let detail = horizon_detail_sample(world_pos.xz, horizon_detail_lod(fp_m));
        if detail.weight > 0.0 {
            if is_backdrop {
                macro_normal = terrain_safe_unit3(mix(geometric_normal,
                    horizon_detail_normal(detail.value), detail.weight), geometric_normal);
            }
            let reach = smoothstep(64.0, 256.0, length(relative_pos));
            macro_occlusion = mix(1.0, horizon_detail_occlusion(detail.value.w), detail.weight * reach);
        }
    }
    if is_backdrop && kind == 0 {
        // Coverage is continuous over a coarse triangle. No interpolated ID is
        // rounded into an unrelated material halfway across a mountain.
        // Gravel is a fifth continuous coverage channel, never an interpolated
        // numeric ID. Preserve its filtered aggregate mean in the proxy instead
        // of falling back to unmodulated base color (a ~28% value jump).
        let gravel = clamp(in.gravel_coverage, 0.0, 1.0);
        detail_color = natural_surface_blended(albedo, in.voxel_pos,
            macro_normal, in.terrain_weights / max(1.0 - gravel, 0.00001), footprint, true);
        if gravel > 0.0 {
            let q = (floor(in.voxel_pos * scale * PIXEL_RES) + vec3<f32>(0.5)) / PIXEL_RES;
            detail_color = mix(detail_color, gravel_surface(albedo, q, geometric_normal, fp_m), gravel);
        }
        if terrain_is_column(in.ao_strength) {
            detail_color *= horizon_column_value(in.voxel_pos * scale, geometric_normal);
        }
    } else if orbital_part {
        // A single 5 cm lattice follows the rigid part through every rotation.
        // Filter unresolved pixels to their mean; no world-space crawling grain.
        let cell = floor(in.model_uv * PIXEL_RES);
        let fade = surface_detail_weight(model_fp);
        let grain = (surface_hash(cell) - 0.5) * 0.07;
        let tile = cell - floor(cell / 20.0) * 20.0;
        let joint = max(1.0 - step(1.0, tile.x), 1.0 - step(1.0, tile.y));
        let radiator = in.surface.w > 0.75;
        let channel = 1.0 - step(1.0, cell.x - floor(cell.x / 10.0) * 10.0);
        let pattern = select(joint * 0.18, channel * 0.32, radiator);
        let pattern_mean = select(0.0975 * 0.18, 0.1 * 0.32, radiator);
        detail_color = albedo * (1.0 - pattern_mean + (grain - pattern + pattern_mean) * fade);
    } else if kind == 34 || kind == 35 || cut_crystal || enemy_metal {
        // Ground-detail tufts and loose stones (mesh/ground_detail.rs) carry
        // their finished colour in the vertex. Painting them with the turf
        // pipeline gave blades the soil/grass-side bank colour.
        detail_color = albedo;
        if kind == 57 || kind == 58 || enemy_metal {
            let cell = floor(in.model_uv * PIXEL_RES);
            let h = surface_hash(cell);
            let grain = select(select(select(0.75, 0.94, h > 0.24), 1.18, h > 0.73), select(0.87, 1.08, h > 0.60), enemy_metal);
            let fade = 1.0 - smoothstep(0.7, 1.4, model_fp);
            detail_color *= mix(1.0, grain, fade);
        }
    } else if is_natural {
        // Climate stays in world coordinates: texture scale must not move the
        // snowline, beaches, or biome transitions.
        detail_color = natural_surface(albedo, in.voxel_pos,
            geometric_normal, kind, footprint);
    } else {
        detail_color = block_surface(albedo, pattern_position / material_scale,
            pattern_normal, kind, footprint / material_scale);
    }
    albedo = max(mix(albedo, detail_color, detail_strength), vec3<f32>(0.0));
    // One shared pixel-art albedo lattice for natural and built surfaces.
    // Water, fire and glow particles keep their own treatment; everything else
    // gets the same visibly chunky dither instead of 32-128 texel/m noise.
    // Natural ground skips it: its shape cells already own the texel detail,
    // and a regular 4x4 dither over them reads as a grid on open turf.
    if kind != 8 && kind != 9 && kind != 40 && kind != 42 && !is_torch_material && !is_viewmodel
        && !is_natural && kind != 34 && kind != 35 && !cut_crystal && !orbital_part {
        albedo = terrain_pixel_albedo(albedo, world_pos, geometric_normal, fp_m);
    }
    // Diagnostic albedo excludes cached roof visibility and lighting-derived
    // darkening. Apply the configured material tint exactly once, as in the
    // production path below; retain all procedural surface colour and dither.
    let diagnostic_albedo = max(albedo * material_tint.rgb, vec3<f32>(0.0));
    if is_natural {
        let relief = far_shade(macro_normal, in.voxel_pos.xz * scale, fp_m);
        n = terrain_safe_unit3(mix(macro_normal, relief, min(detail_strength, 1.0)), macro_normal);
        // Material-shaped relief (pebble domes, clods, rock planes), already
        // faded by footprint in the material functions.
        let tilt = natural_tilt * min(detail_strength, 1.0);
        n = terrain_safe_unit3(n - (tilt - n * dot(n, tilt)), n);
        let coarse_weight = surface_detail_weight(fp_m / 48.0);
        let fine_weight = surface_detail_weight(fp_m / 12.0);
        // Approximate lost slope variance prevents filtered relief from turning
        // into an unnaturally perfect mirror. This is not a measured normal map.
        filtered_slope_variance = 0.20 * min(detail_strength * detail_strength, 1.0)
            * (0.055 * 0.055 * (1.0 - coarse_weight * coarse_weight)
                + 0.030 * 0.030 * (1.0 - fine_weight * fine_weight));
    }

    let cave_amount = 1.0 - smoothstep(0.03, 0.62, sky_access);
    // Natural ground already has the shared far-field relief. Built and mined
    // materials previously had no normal detail at all; natural cave surfaces
    // get an additional pass only below the skylight threshold.
    let relief_strength = select(
        mix(0.44, 0.92, cave_amount),
        0.62 * cave_amount,
        is_natural,
    );
    if kind != 6 && kind != 8 && kind != 9 && kind != 34 && !is_backdrop && !is_viewmodel
        && relief_strength > 0.02 && !cut_crystal && kind != 22
    {
        n = surface_relief(n, world_pos, fp_m, relief_strength);
        filtered_slope_variance += 0.0016 * relief_strength * relief_strength;
    }
    if kind == 22 {
        // Solid facets do not wobble with the clock. Their normals share the
        // colour pattern and fade to the geometric face when no longer resolved.
        let uv = enchantment_face_uv(pattern_position * scale / material_scale, pattern_normal);
        let facet = crystal_facets(texel_centre(uv));
        let resolved = surface_detail_weight(fp_m / material_scale * PIXEL_RES / CRYSTAL_FACET_TEXELS);
        let tangent = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(geometric_normal.x) > 0.5);
        let bitangent = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(geometric_normal.y) > 0.5);
        n = terrain_safe_unit3(geometric_normal + (tangent * (facet.x - 0.5)
            + bitangent * (facet.w - 0.75)) * 0.40 * resolved, geometric_normal);
        filtered_slope_variance += 0.013 * (1.0 - resolved * resolved);
    }
    if !is_backdrop && sky_access < 0.80 && !cut_crystal {
        let grime = surface_noise_filtered(
            world_pos.xz / 0.90 + vec2<f32>(world_pos.y * 0.27, -world_pos.y * 0.19),
            fp_m / 0.90,
        );
        albedo *= 1.0 - cave_amount * (0.10 + (grime - 0.5) * 0.10);
    }
    // Lava is a moving thermal surface, not a flat orange sticker. A bounded
    // heat shimmer bends its lighting normal without touching screen space or
    // requiring another pass.
    if kind == 10 && !is_backdrop {
        let heat_time = light.torch_meta.y;
        let heat = vec3<f32>(
            sin(world_pos.x * 4.7 + world_pos.z * 2.3 + heat_time * 5.1),
            0.0,
            cos(world_pos.z * 5.3 - world_pos.y * 3.1 + heat_time * 4.3),
        ) * 0.16;
        n = terrain_safe_unit3(n + heat, geometric_normal);
        filtered_slope_variance += 0.0025;
    }
    if kind != 8 && kind != 9 && kind != 54 && kind != 55 && !cut_crystal {
        let micro = surface_noise_filtered(
            world_pos.xz / 0.22 + vec2<f32>(world_pos.y * 0.41, -world_pos.y * 0.33),
            fp_m / 0.22,
        );
        roughness = clamp(roughness * (0.86 + micro * 0.28), 0.08, 1.0);
    }

    var water_depth = 10.0;
    var water_foam = 0.0;
    var water_jacobian = 1.0;
    // Walls of a pool or a flowing step get the same water body colour as the
    // surface, without waves or foam. They used to be a flat teal albedo.
    let water_side = kind == 8 && n.y <= 0.5 && in.surface.w >= 8.0 && terrain_has_resident_metadata(in.ao_strength);
    if kind == 8 && (n.y > 0.5 || water_side) {
        let t = light.torch_meta.y;
        let fp = footprint * cam.world_params.x;
        // Up close the wave field is sampled at 5 cm texel centres, the same
        // lattice as the ground, so ripples and foam are crisp pixels beside
        // pixel terrain. The weight is the ground's own detail fade, so it
        // hands over to smooth sampling once a texel drops below a screen pixel.
        let p = select(world_pos.xz, texel_centre(world_pos.xz), surface_detail_weight(fp * PIXEL_RES) > 0.5);

        // The far water lattice carries bed depth in [0,3], separate from
        // resident sky-access metadata [4,5].
        let is_far = in.ao_strength < 0.0 && in.ao_strength > -1.5;
        // Both owners carry source bed colour/depth. Filter the same refraction
        // field by distance; an irregular ownership edge must not switch it off.
        // This does not substitute for equal bed depths from the terrain source.
        let handoff_start = max(terrain_backdrop_start() - terrain_overlap_width() * 0.50, 0.0);
        let handoff_end = max(terrain_backdrop_start(), handoff_start + scale);
        let local_water = 1.0 - smoothstep(handoff_start, handoff_end, length(relative_pos.xz));
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
        filtered_slope_variance = 0.0012 * (1.0 - w0 * w0)
            + 0.0010 * (1.0 - w1 * w1) + 0.0006 * (1.0 - w2 * w2)
            + 0.00025 * (1.0 - w3 * w3);

        let n0 = surface_noise_grad(p0);
        let n1 = surface_noise_grad(p1);
        let n2 = surface_noise_grad(p2);
        let n3 = surface_noise_grad(p3);

        // Artistic shallow-water shoaling proxy. The renderer does not supply
        // a bathymetry/flow simulation, so this is not a dispersion solver.
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

        let gradient = (gerstner_grad + fbm_grad) * 0.40;
        n = normalize(n + select(vec3<f32>(gradient.x, 0.0, gradient.y), vec3<f32>(0.0), water_side));

        let jac_compress = (0.34 * cos(ph0) + 0.31 * cos(ph1)) * w0
                         + (0.27 * cos(ph2) + 0.24 * cos(ph3)) * w1;
        water_jacobian = clamp(1.0 - jac_compress * 0.55 * shoal_boost, 0.35, 1.65);

        // Snell's Law refraction (n_air / n_water = 1.0 / 1.333 = 0.75) shifts
        // the apparent seabed position and drives refracted sand ripple detail.
        let v_inc = terrain_safe_unit3(relative_pos, vec3<f32>(0.0, -1.0, 0.0));
        // A wall is a window into the water volume. Its normal is wobbled by the
        // surface ripples, and `depth` (the water thickness below this fragment,
        // interpolated from the vertices) is how far the refracted ray travels
        // before it meets the bed, so the bed and its caustics move with parallax.
        let wall_tangent = vec3<f32>(-n.z, 0.0, n.x);
        let refract_n = select(
            n,
            normalize(n + wall_tangent * (gradient.x * 1.6) + vec3<f32>(0.0, gradient.y * 1.6, 0.0)),
            water_side);
        let refracted = refract(v_inc, refract_n, 0.75);
        let refract_depth = min(depth, 4.2);
        let ray_xz = select(refracted.xz - v_inc.xz, refracted.xz, water_side);
        let parallax_xz = ray_xz * (refract_depth / max(-refracted.y, 0.24)) * local_water;
        let bed_p = p + parallax_xz;

        let turbidity = surface_noise_filtered(p / 340.0, fp / 340.0);
        let sand_bed = mix(vec3<f32>(0.185, 0.168, 0.108), vec3<f32>(0.105, 0.178, 0.148), turbidity);
        let turquoise_mid = mix(vec3<f32>(0.06, 0.16, 0.20), vec3<f32>(0.05, 0.14, 0.19), turbidity);
        let deep_ocean = mix(vec3<f32>(0.020, 0.055, 0.080), vec3<f32>(0.016, 0.045, 0.070), turbidity);
        let shallow_t = 1.0 - exp(-depth * 0.62);
        let deep_t = 1.0 - exp(-max(depth - 1.2, 0.0) * 0.18);
        var water_col = mix(sand_bed, turquoise_mid, shallow_t);
        water_col = mix(water_col, deep_ocean, deep_t);
        // Resident art direction (expedition design §6, Wind Waker): three bands
        // readable at a glance. Shallow celadon under ~0.4 m, sea in the middle,
        // ink in the trough. Far water keeps the depth ramp above.
        let wind_shallow = vec3<f32>(0.42, 0.62, 0.48);
        let wind_sea = vec3<f32>(0.05, 0.38, 0.48);
        let wind_ink = vec3<f32>(0.02, 0.07, 0.14);
        // Depth reaches the shader in 0.25 m steps (water_optics is packed as
        // k = depth*4). The transitions below span many steps, so each step
        // moves the colour a little and no contour reads as a hard band.
        let wind_shallow_t = 1.0 - smoothstep(0.25, 2.5, depth);
        let wind_ink_t = smoothstep(2.5, 9.0, depth);
        var wind_col = mix(wind_sea, wind_shallow, wind_shallow_t);
        wind_col = mix(wind_col, wind_ink, wind_ink_t);
        water_col = mix(water_col, wind_col, local_water);

        if local_water > 0.0 {
            // Refracted seabed ripples & mineral grain visible through clear shallows.
            let bed_ripple = sin(dot(bed_p, vec2<f32>(0.82, 0.57)) * 3.6 + surface_noise(bed_p * 0.9) * 3.2)
                * surface_detail_weight(fp * 3.0);
            let bed_grain = surface_noise_filtered(bed_p * 4.2, fp * 6.5);
            let bed_detail = (0.91 + 0.09 * bed_ripple + 0.06 * (bed_grain - 0.5)) * (0.94 + 0.08 * cusp_x * w2);
            let refracted_bed = albedo * bed_detail;

            // Physical Beer-Lambert spectral absorption along the refracted ray + solar path.
            let optical_path = depth * (1.0 / max(-refracted.y, 0.22) + 0.45 / max(light.sun_dir_intensity.y + 0.28, 0.25));
            let transmission = exp(-vec3<f32>(0.46, 0.24, 0.13) * optical_path);
            water_col = mix(water_col, water_col * (vec3<f32>(1.0) - transmission) + refracted_bed * transmission, local_water);
        }

        // Caustics require submerged receivers; foam requires contact/agitation.
        // Depth contours alone provide neither. fract(voxel_pos.y) is not a
        // waterline: it repeated a bright meniscus at every cell boundary.
        water_foam = 0.0;
        albedo = mix(water_col, vec3<f32>(0.60, 0.66, 0.67), water_foam);
        roughness = mix(select(0.075, material_tint.w, material_tint.w >= 0.0), 0.48, water_foam);
    }
    albedo *= material_tint.rgb;
    // Wet films accumulate/dry in real time, with cached roof visibility.
    let exposure_to_rain=smoothstep(0.2,0.85,sky_access)*smoothstep(-0.15,0.65,geometric_normal.y);
    let wet=clamp(light.surface_weather.x*material_props.w*exposure_to_rain,0.0,1.0)
        *select(1.0,0.0,kind==8 || kind==9 || kind==10 || is_torch_material || kind>=40);
    let wet_patch=0.75+0.25*surface_noise_filtered(world_pos.xz*0.32,fp_m*0.32);
    albedo*=1.0-wet*wet_patch*0.32;
    roughness=mix(roughness,min(roughness,0.20),wet*0.80);
    reflectance=mix(reflectance,max(reflectance,0.55),wet);

    var animated_emission = vec3<f32>(0.0);
    if kind == 56 {
        // Calm inner scatter follows the authored facets, with no world-space
        // patina, moving noise or time-varying normal. Core and eyes remain brighter.
        let rim = pow(1.0 - clamp(abs(dot(n, terrain_safe_unit3(-relative_pos, geometric_normal))), 0.0, 1.0), 3.0);
        animated_emission = albedo * (0.045 + rim * 0.12);
    }
    if kind >= 47 && kind <= 51 {
        var face = in.voxel_pos.xz;
        if abs(n.x) > 0.5 { face = in.voxel_pos.zy; }
        else if abs(n.z) > 0.5 { face = in.voxel_pos.xy; }
        let brush = surface_noise_filtered(face * vec2<f32>(48.0, 6.0), footprint * 48.0) - 0.5;
        let flake = surface_noise_filtered(face * 22.0, footprint * 22.0) - 0.5;
        n = normalize(n + vec3<f32>(brush, flake * 0.35, brush * 0.4));
        let finish_default = select(0.30, select(0.12, select(0.16, select(0.15, 0.08, kind == 51), kind == 50), kind == 49), kind == 48);
        roughness = select(finish_default, material_tint.w, material_tint.w >= 0.0);
        reflectance = select(0.82, 0.96, kind == 48 || kind == 49);
        albedo *= 1.0 + brush * 0.08;
    }
    if kind == 46 {
        let phase = light.torch_meta.y * 0.45 + world_pos.y * 1.6;
        let warp = vec3<f32>(sin(phase), cos(phase * 0.7), sin(phase * 0.4));
        n = terrain_safe_unit3(n + warp * 0.12, geometric_normal);
        let rim = pow(1.0 - clamp(abs(dot(n, terrain_safe_unit3(-relative_pos, geometric_normal))), 0.0, 1.0), 1.8);
        animated_emission = albedo * (0.35 + rim * 0.25);
        albedo *= 0.42;
    }

    // All material normals are settled before derivatives. No material branch
    // or fragment discard may execute a derivative internally.
    // Normal variance widens the specular lobe as ripples become subpixel.
    let normal_dx = dpdx(n);
    let normal_dy = dpdy(n);
    let horizontal_dist = length(relative_pos.xz);
    // Main and diagnostic views must show the same owner. Keep this after all
    // derivative evaluations, but before any material/normal/depth debug return.
    if in.ao_strength < 0.0 && terrain_resident_column_owned(world_pos.xz, horizontal_dist) { discard; }
    let normal_variance = 0.5 * (dot(normal_dx, normal_dx) + dot(normal_dy, normal_dy)) + filtered_slope_variance;
    var fire_heat = 0.0;
    if kind >= 42 && kind <= 44 {
        // Stuck to the voxel, not the screen. A screen hash reshuffles the
        // silhouette every frame and reads as sparkle.
        let threshold = fract(dot(floor(in.voxel_pos * 6.0), vec3<f32>(0.754877666, 0.569840296, 0.414213562)));
        // Rounded, feathered puffs suppress the opaque-looking voxel box edges.
        // World-anchored face coordinates stay continuous across chunk boundaries.
        let axis = abs(in.normal);
        let cell = fract(in.voxel_pos) - vec3<f32>(0.5);
        var uv = cell.xz;
        if axis.x > axis.y && axis.x > axis.z { uv = cell.zy; }
        else if axis.z > axis.y { uv = cell.xy; }
        let radius = length(uv * vec2<f32>(1.0, select(1.0, 0.8, kind == 42)));
        var feather = 1.0 - smoothstep(0.18, 0.50, radius);
        if kind == 42 {
            // Fire is a vertically advected, licking volume rather than a
            // static orange disc. The noise is anchored in voxel space and
            // scrolled on the simulation clock; it does not touch derivatives.
            let t = light.torch_meta.y;
            let tongue = surface_noise_filtered(
                vec2<f32>(
                    dot(in.voxel_pos.xz, vec2<f32>(2.7, 1.9)) + in.voxel_pos.y * 0.37,
                    in.voxel_pos.y * 4.6 - t * 2.25,
                ),
                footprint * 3.4,
            );
            let flare = surface_noise_filtered(
                in.voxel_pos.xz * 5.3 + vec2<f32>(t * 0.8, -t * 1.1),
                footprint * 5.3,
            );
            let core = 1.0 - smoothstep(0.06, 0.46, radius);
            fire_heat = clamp(core * (1.05 + (flare - 0.5) * 0.36)
                + (tongue - 0.5) * 0.42, 0.0, 1.0);
            feather = 1.0 - smoothstep(
                0.12,
                0.50,
                radius + (tongue - 0.5) * 0.22 + (flare - 0.5) * 0.10,
            );
            if threshold > 0.78 * feather { discard; }
        } else {
            // Smoke and steam drift as rounded puffs; the silhouette breathes
            // slowly instead of being one fixed circle per voxel face.
            let drift = surface_noise_filtered(
                in.voxel_pos.xz * 1.9 + vec2<f32>(light.torch_meta.y * 0.17, -light.torch_meta.y * 0.11),
                footprint * 1.9,
            );
            feather = 1.0 - smoothstep(0.16, 0.50, radius + (drift - 0.5) * 0.20);
            if threshold > 0.34 * feather { discard; }
        }
    }
    // Translucent glow pieces (wand head, spell motes, glyphs) are not clipped
    // here. This used to discard fragments whose position inside the *world*
    // voxel cell was far from the cell centre. That suits a voxel-sized puff
    // but not a 2 cm mote: whether the whole piece survived depended on where
    // it sat in the grid, so anything carried or flying strobed in and out as
    // it moved. Fade-out is done in geometry instead (Model::glow_voxel_with_dissolve).
    var mineral_metallic = 0.0;
    if kind == 15 || kind == 20 || kind == 21 {
        let mineral_uv = enchantment_face_uv(pattern_position * scale / material_scale, pattern_normal);
        let inclusion = ore_inclusion(mineral_uv, fp_m / material_scale);
        roughness = mix(0.90, select(select(0.38, 0.30, kind == 20), 0.26, kind == 21), inclusion);
        mineral_metallic = inclusion * 0.78;
    }
    if kind == 22 { roughness = 0.26; reflectance = 0.5; }
    if kind == 53 { roughness = 0.20; reflectance = 0.5; }
    roughness = terrain_filter_roughness(roughness, normal_variance);
    // Distant water uses the same finish as resident water. There is no blanket
    // backdrop roughness floor or extra (1-r)^2 specular suppression.
    let generic_metallic = select(select(mineral_metallic, 1.0, kind >= 47 && kind <= 51), 0.72, enemy_metal);
    let metallic = select(generic_metallic,
        select(select(0.0, 0.82, in.surface.w > 0.25), 0.35, in.surface.w > 0.75), orbital_part);
    albedo = clamp(albedo, vec3<f32>(0.0), vec3<f32>(1.0));
    // Reflectance convention: 0.5 -> dielectric F0 0.04.
    // Explicit palette values already use that parameter. The vertex stage
    // converts only the unset mesh-specular fallback. Host data is not F0.
    let dielectric_f0 = vec3<f32>(0.16 * reflectance * reflectance);
    let f0 = select(mix(dielectric_f0, albedo, metallic), vec3<f32>(0.0204), kind == 8);
    let diffuse_energy = (vec3<f32>(1.0) - f0) * (1.0 - metallic);

    let to_eye = terrain_safe_unit3(-relative_pos, geometric_normal);
    let view_dir = -to_eye;
    let sun_dir = terrain_safe_unit3(light.sun_dir_intensity.xyz, vec3<f32>(0.0, 1.0, 0.0));
    let moon_dir = terrain_safe_unit3(light.moon_dir_intensity.xyz, vec3<f32>(0.0, -1.0, 0.0));

    let daylight   = saturate1(light.sun_color_daylight.w);
    let exposure   = max(light.ambient_exposure.w, 0.01);
    let saturation = clamp(light.sky_top_saturation.w, 0.45, 1.05);

    let linear_depth = dot(relative_pos, cam.camera_forward.xyz);

    let debug_mode = i32(render_debug_view());
    if debug_mode == 31 {
        // Optical depth in metres, linear grayscale: white = 32 m.
        // Non-water is black; use the same ownership as the beauty pass.
        let depth_color = select(0.0, clamp(water_depth / 32.0, 0.0, 1.0), kind == 8);
        return debug_surface_output(vec4<f32>(vec3<f32>(depth_color), 1.0));
    }
    if debug_mode == 27 {
        let uv = terrain_pixel_uv(pattern_position * scale / material_scale, pattern_normal);
        let cell = floor(uv * PIXEL_RES);
        let checker = (cell.x + cell.y) - 2.0 * floor((cell.x + cell.y) * 0.5);
        return debug_surface_output(vec4<f32>(vec3<f32>(0.15 + checker * 0.65), 1.0));
    }
    if debug_mode == 28 {
        let ao = terrain_vertex_ao(in.ao_strength);
        return debug_surface_output(vec4<f32>(vec3<f32>(ao), 1.0));
    }
    if debug_mode == 29 { return debug_surface_output(vec4<f32>(vec3<f32>(sky_access), 1.0)); }
    if debug_mode == 30 {
        let color = select(vec3<f32>(0.12, 0.75, 0.2), vec3<f32>(0.12, 0.3, 0.9), is_backdrop);
        return debug_surface_output(vec4<f32>(select(color, vec3<f32>(0.9, 0.4, 0.1), in.ao_strength < -1.5), 1.0));
    }
    if debug_mode == 19 {
        // Known families are 0..37, 40..44, 47..56. Unsupported keys are magenta.
        let supported = kind <= 37 || (kind >= 40 && kind <= 60);
        let id_color = 0.15 + 0.75 * fract(f32(kind + 1) * vec3<f32>(0.618034, 0.381966, 0.236068));
        return debug_surface_output(vec4<f32>(select(vec3<f32>(1.0, 0.0, 1.0), id_color, supported), 1.0));
    }
    if debug_mode == 20 {
        // The mesh normal may already be smoothed across horizon triangles.
        // Show actual rasterized face orientation so that smoothing cannot
        // conceal a geometric discontinuity in this diagnostic.
        let face_normal = terrain_safe_unit3(cross(world_dx, world_dy), geometric_normal);
        let oriented_face = select(face_normal, -face_normal, dot(face_normal, geometric_normal) < 0.0);
        return debug_surface_output(vec4<f32>(oriented_face * 0.5 + 0.5, 1.0));
    }
    if debug_mode == 21 { return debug_surface_output(vec4<f32>(n * 0.5 + 0.5, 1.0)); }
    if debug_mode == 22 { return debug_surface_output(vec4<f32>(vec3<f32>(roughness), 1.0)); }
    if debug_mode == 18 { // Material-only linear albedo; excludes roof/cavity darkening.
        return debug_surface_output(vec4<f32>(diagnostic_albedo, 1.0));
    }
    if debug_mode == 16 { // Shadow Cascade Index
        var cascade_idx = 4;
        for (var i = 0; i < 4; i = i + 1) {
            if linear_depth < shadow.cascades[i].far_plane {
                cascade_idx = i;
                break;
            }
        }
        let colors = array<vec3<f32>, 5>(
            vec3<f32>(1.0, 0.2, 0.2), vec3<f32>(0.2, 1.0, 0.2),
            vec3<f32>(0.2, 0.2, 1.0), vec3<f32>(1.0, 1.0, 0.2),
            vec3<f32>(0.8, 0.2, 1.0)
        );
        return debug_surface_output(vec4<f32>(colors[cascade_idx], 1.0));
    }

    if debug_mode == 17 { // Shadow Texel Density / Utilization
        var cascade_idx = 4;
        for (var i = 0; i < 4; i = i + 1) {
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

    let dist = length(relative_pos);

    // --- Spectral sun/moon tinting from atmosphere transmittance ---
    // Matches the cloud illumination so sky, clouds, and terrain are tonally
    // consistent at all times of day, especially at sunset.
    let sun_spectral  = sky_transmittance_to_sun(sun_dir.y);
    let moon_spectral = sky_transmittance_to_sun(moon_dir.y);

    // --- World-space cloud transmittance above this surface point ---
    // This makes terrain lighting react to the actual cloud field overhead,
    // instead of just a global weather scalar.
    let local_cloud_trans = sample_shared_cloud_shadow_transmittance(
        world_pos + n * (2.0 * cam.world_params.x),
        sun_dir,
    );
    let cloudiness   = 1.0 - local_cloud_trans;
    // Diffuse weather attenuation is local too: a camera-centred clipmap
    // average must not change the fill on a stationary surface during travel.
    let local_weather = shadow_field_at_world(world_pos.xz);
    let heavy_cloud  = saturate1(1.0 - exp(-dot(local_weather.xyz, vec3<f32>(1.0))));
    // Preserve directional sky-dome modeling through cloud cover so overcast
    // days still cast visible tree/terrain shadows and 3D ridge definition.
    let sun_trans = local_cloud_trans;
    var moon_trans = 1.0;
    if light.moon_dir_intensity.w > 0.0 && moon_dir.y > 0.02 {
        moon_trans = sample_shared_cloud_shadow_transmittance(world_pos, moon_dir);
    }

    // --- Direct diffuse ---
    let foliage = kind == 6 || kind == 34;
    let foliage_sun = max((dot(n, sun_dir) + 0.35) / 1.35, 0.0);
    let foliage_moon = max((dot(n, moon_dir) + 0.35) / 1.35, 0.0);

    // PCF shadow
    let sun_shadow_strength = 0.94 * smoothstep(0.03, 0.16, light.sun_dir_intensity.w);
    var shadow_visibility = 1.0;
    if cam.scene_seed.w == 0u && (sun_shadow_strength > 0.0 || debug_mode == 11) {
        // Real resident, exact fallback, and procedural receivers share CSM
        // wherever it has coverage; N.L already handles unlit slope orientations.
        shadow_visibility = sample_sun_shadow(world_pos, linear_depth,
            clamp(dot(geometric_normal, sun_dir), 0.0, 1.0), world_dx, world_dy);
        // Distant ranges as casters: the cascades end within a few hundred
        // metres, the detail map reaches the far plane. Skipped where the
        // surface already faces away from the sun or has no sky.
        if sun_shadow_strength > 0.0 && !is_viewmodel && sky_access > 0.05
            && dot(macro_normal, sun_dir) > 0.0 {
            shadow_visibility = min(shadow_visibility, horizon_detail_sun_visibility(world_pos, sun_dir));
        }
    }

    if debug_mode == 11 { return debug_surface_output(vec4<f32>(vec3<f32>(shadow_visibility), 1.0)); }

    let sun_shadow = mix(1.0, shadow_visibility, sun_shadow_strength);

    // Retain geometric horizon visibility when small shading normals bend past
    // a face. Without it, relief-perturbed normals light slopes that face away
    // from the sun, which shows up beyond the shadow range. Foliage wraps its
    // light on purpose, so it is exempt.
    let sun_horizon = smoothstep(-0.015, 0.015, dot(macro_normal, sun_dir));
    let moon_horizon = smoothstep(-0.015, 0.015, dot(macro_normal, moon_dir));
    // Oren–Nayar for rough surfaces, Lambertian for smooth.
    let sun_diffuse = select(terrain_directional_diffuse(n, macro_normal, to_eye, sun_dir,
                          roughness, 0.22, in.ao_strength, kind, in.terrain_weights.z) * sun_horizon, foliage_sun, foliage)
                      * light.sun_dir_intensity.w * sun_trans * sun_shadow * sky_access;
    let moon_diffuse = select(terrain_directional_diffuse(n, macro_normal, to_eye, moon_dir,
                          roughness, 0.16, in.ao_strength, kind, in.terrain_weights.z) * moon_horizon, foliage_moon, foliage)
                       * light.moon_dir_intensity.w * moon_trans;

    // --- Ambient / sky fill (hemisphere integral approximation) ---
    // Normal-weighted sky sampling: upward-facing surfaces get top sky,
    // side-facing get horizon, downward-facing get ground bounce.
    let hemi        = saturate1(n.y * 0.5 + 0.5);
    let sky_up      = atmosphere_color(vec3<f32>(0.0, 1.0, 0.0));
    // Diffuse illumination belongs to the world, not the camera. Key the
    // horizon sample to the sun azimuth so a pan cannot relight a hillside.
    // Sky irradiance approximation: radiance of the horizon band the face
    // looks at. A face turned to the sun sees the warm solar sky; one turned
    // away sees the cooler anti-solar sky. Sampling only the solar side gave
    // every shaded face the same warm fill and erased the light direction.
    let sun_az = safe_normalize2(sun_dir.xz, vec2<f32>(1.0, 0.0));
    let face_toward_sun = saturate1(dot(n.xz, sun_az) * 0.5 + 0.5);
    let solar_horizon = atmosphere_color(normalize(vec3<f32>(sun_az.x, 0.16, sun_az.y)));
    let antisolar_horizon = atmosphere_color(normalize(vec3<f32>(-sun_az.x, 0.16, -sun_az.y)));
    let sky_horizon = mix(antisolar_horizon, solar_horizon, face_toward_sun);
    let clear_sky_ambient = mix(sky_horizon * 0.78, sky_up, pow(hemi, 0.72));
    let overcast_sky_ambient = mix(
        sky_horizon * 0.62,
        sky_horizon * 0.54 + sky_up * 0.46,
        pow(hemi, 0.85),
    );
    let sky_ambient = mix(clear_sky_ambient, overcast_sky_ambient, cloudiness * 0.65);
    let twilight_fill = 1.0 - smoothstep(-0.10, 0.22, sun_dir.y);
    let low_solar_sky = atmosphere_color(normalize(vec3<f32>(sun_az.x, 0.035, sun_az.y)));
    let low_horizon_sky = mix(antisolar_horizon, low_solar_sky, face_toward_sun);
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
    let raw_ao = terrain_vertex_ao(in.ao_strength);
    let shaped_ao   = pow(raw_ao, 1.35);
    let cavity      = mix(mix(0.32, 1.0, shaped_ao), 1.0, hemi * 0.08);
    let ambient     = (
        light.ambient_exposure.rgb * mix(vec3<f32>(0.38), vec3<f32>(1.0), hemi * hemi)
        + mix(terrain_sky_ambient,
              vec3<f32>(dot(terrain_sky_ambient, vec3<f32>(0.2126, 0.7152, 0.0722))),
              cloudiness * 0.18) * sky_fill_strength
        + ground_col * ground_amt + dusk_sky_fill
    ) * cavity * macro_occlusion * mix(1.0, 0.86, cloudiness * daylight * 0.55);
    // A small non-sky fill keeps deep-cave silhouettes readable without making
    // the underground look like an overlit interior. It follows the cavity and
    // face hemisphere so micro-relief still has shape.
    // Cave fill is deliberately not one flat grey lift. Deep rock receives a
    // cool green-blue bounce, upward faces keep more of it, and downward faces
    // retain a small warm floor bounce. This makes tunnels read as carved
    // spaces with a ceiling and floor rather than an evenly exposed interior.
    let cave_depth = 1.0 - smoothstep(0.0, 0.48, sky_access);
    let cave_fill = (
        vec3<f32>(0.019, 0.024, 0.033)
        + vec3<f32>(0.013, 0.018, 0.015) * cave_depth
        + vec3<f32>(0.008, 0.006, 0.0035) * (1.0 - hemi) * 0.7
    ) * cave_amount * (0.35 + 0.65 * cavity) * (0.62 + 0.38 * hemi)
        * select(0.0, 1.0, kind != 8 && kind != 9);

    // --- Explicit ember / flame emission ---
    // Fire retains its luminance when the environment is darkened for night.
    // Cancel both the shared exposure and the terrain-only night grade below.
    let fire_exposure = 0.9 / exposure;
    var emissive = animated_emission;
    // Keep the authored body albedo. Only the moving ribbons emit, in the
    // selected spell's colour carried by the wand mesh, never a universal hue.
    if kind == 55 || kind == 22 || kind == 51 {
        let magic_uv = enchantment_face_uv(pattern_position * scale, pattern_normal);
        if kind == 55 {
            let magic_bands = enchantment_bands(magic_uv, fp_m, light.torch_meta.y);
            let spell_color = terrain_srgb_to_linear(clamp(
                vec3<f32>(in.surface.xy, in.surface.w), vec3<f32>(0.0), vec3<f32>(1.0)));
            emissive += spell_color * (magic_bands.x * 0.12 + magic_bands.y * 0.06);
        } else if kind == 51 {
            let magic_bands = enchantment_bands(magic_uv, fp_m, light.torch_meta.y);
            // Mithril carries a quieter pearl-and-cyan travelling sheen.
            emissive += (vec3<f32>(0.10, 0.34, 0.39) * magic_bands.x
                + vec3<f32>(0.27, 0.22, 0.38) * magic_bands.y) * 0.30;
        } else {
            // A broad resonance breath and rare facet sweep, rather than
            // adding more tiny random sparkles to the existing crystal glints.
            let resonance = 0.5 + 0.5 * sin(light.torch_meta.y * light.magic[3].y * 1.15);
            let ribbon = enchantment_primary(magic_uv, fp_m, light.torch_meta.y);
            emissive += vec3<f32>(0.14, 0.07, 0.25) * (ribbon * 0.28 + resonance * 0.10);
        }
    }
    if kind == 42 {
        // Black-body ramp: deep orange edges, orange body and a small warm
        // core. Keeping the ramp in HDR preserves dark contrast between flame
        // cells. The body keeps little green: at 0.31 with a full white core,
        // the hottest voxels summed to near-white and read as pale salmon.
        let heat2 = fire_heat * fire_heat;
        emissive += fire_ramp(fire_heat) * 3.1 * fire_exposure;
        albedo = mix(vec3<f32>(0.16, 0.028, 0.004), vec3<f32>(0.95, 0.36, 0.05), heat2);
    }
    if kind == 43 { emissive += albedo * 0.10 * fire_exposure; }
    if kind == 44 { emissive += albedo * 0.18 * fire_exposure; }
    // Mithril remains a reflective metal; its restrained ribbons supply magic.
    // Crystal shine comes from the shared sun/torch BRDF, not emissive flashes.
    if kind == 52 || kind == 53 {
        let stars = void_star_mask(in.voxel_pos / material_scale, geometric_normal, kind, light.torch_meta.y);
        let warmth = surface_hash(floor(in.voxel_pos.xz * 0.5));
        emissive += mix(vec3<f32>(0.55, 0.68, 1.0), vec3<f32>(1.0, 0.84, 0.62), warmth) * stars
            * select(1.3, 2.4, kind == 53) * fire_exposure;
        // A faint inner light so the room reads as a place, not just a backdrop.
        emissive += albedo * select(0.10, 0.18, kind == 53) * fire_exposure;
    }
    if kind == 60 {
        // Corrupted gear uses attached native texels, never a world-space noise mask.
        let cell = vec2<i32>(floor(in.model_uv * PIXEL_RES));
        let x = abs(cell.x) % 4;
        let y = abs(cell.y) % 7;
        let stroke = select(0.0, 1.0, x == 0 || (y == 2 && x <= 1));
        let charge = clamp(in.surface.w, 0.0, 1.0);
        let tint = mix(vec3<f32>(0.38,0.025,0.045), vec3<f32>(1.0,0.24,0.075), charge);
        let fade = 1.0 - smoothstep(0.8, 1.6, model_fp);
        emissive += tint * stroke * (0.12 + charge * 1.35) * fade;
    }
    if kind == 40 {
        let rim = 1.0 - clamp(abs(dot(n, to_eye)), 0.0, 1.0);
        let rim_boost = select(1.0, 1.0 + 0.18 * rim, in.surface.x < 0.0);
        emissive += albedo * rim_boost * max(in.surface.w, 0.0) * mix(1.0, fire_exposure, 0.35);
    }
    if is_torch_material {
        let flame = in.surface.w < -1.5;
        let flame_time = light.torch_meta.y;
        // Flame emission rides a continuous field over the flame. The old
        // floor() seed held each voxel at one brightness, so the flame stepped
        // in cube-sized chunks and barely moved. Placed torches are world-fixed,
        // so a world-anchored field does not swim.
        // Same wave as the vertex sway, so the brightness climbs with the motion.
        let lift = flame_lift(in.surface.w);
        let lick_phase = dot(in.voxel_pos, vec3<f32>(3.1, 2.2, 2.7));
        let lick = 0.6 * sin(flame_time * 6.7 - lift * 2.6 + lick_phase)
            + 0.4 * sin(flame_time * 10.9 - lift * 3.4 - 1.7 * lick_phase);
        // 0.96 is torch_flicker's mean, so the flame keeps its average brightness;
        // a higher mean pushes the saturated orange toward pale peach after tonemap.
        let flame_wave = select(torch_flicker(floor(in.voxel_pos), flame_time), 0.96 + 0.14 * lick, flame);
        // base_color is already linear (decoded in the vertex stage), so the
        // HDR emission keeps its saturated orange instead of washing toward white.
        var flame_color = max(in.base_color, vec3<f32>(0.0));
        if flame {
            // Shadertoy-style flame field: noise scrolled up the flame and shaped
            // by height and the view-facing rim, so the silhouette cools to red
            // and hot tongues climb from the core. Coordinates are the mesh's
            // own lift and normal, never world cells, so swaying geometry keeps
            // its pattern (render invariant 2).
            let around = in.normal.xz * 1.7;
            let tongue = surface_noise_filtered(
                vec2<f32>(around.x + lift * 0.4, lift * 3.4 - flame_time * 2.6),
                footprint * 3.4,
            );
            let flick = surface_noise_filtered(
                vec2<f32>(around.y - lift * 0.3, lift * 5.6 - flame_time * 3.9),
                footprint * 5.6,
            );
            let facing = clamp(abs(dot(n, to_eye)), 0.0, 1.0);
            let heat = clamp(1.12 - lift * 0.9 - (1.0 - facing) * 0.4
                + ((tongue - 0.5) * 0.6 + (flick - 0.5) * 0.3) * (0.35 + lift), 0.0, 1.0);
            // Same palette as fire voxels, blended with the mesher's ring colour
            // so the yellow core -> red tip authoring still leads; the hot core
            // keeps most of its authored yellow. The heat gain averages near 1,
            // keeping mean brightness.
            flame_color = mix(flame_color, fire_ramp(heat), 0.6 - 0.45 * heat) * (0.55 + 0.75 * heat);
        }
        emissive += flame_color * select(0.65, 3.2, flame) * flame_wave * fire_exposure;
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
    let receives_local_light = terrain_has_resident_metadata(in.ao_strength);
    let torch_count = select(0u, u32(clamp(light.torch_meta.x, 0.0, 64.0)), receives_local_light);
    // Moving lights occupy the leading slots (render.rs sorts them first), so
    // their flicker is time-seeded and does not re-roll as the player walks.
    let moving_count = u32(clamp(light.torch_meta.z, 0.0, 64.0));
    for (var i: u32 = 0u; i < 64u; i = i + 1u) {
        if i >= torch_count { break; }
        let ts = eval_torch_light(
            light.torch_positions[i], light.torch_colors[i],
            light.torch_meta.y, world_pos, n, to_eye, roughness, f0, metallic, foliage, i < moving_count,
        );
        torch_diffuse += ts.diffuse;
        torch_fill    += ts.fill;
        torch_spec    += ts.spec;
    }
    if receives_local_light && light.held_torch_pos_radius.w > 0.0 && light.held_torch_color_strength.w > 0.0 {
        let ht = eval_torch_light(
            light.held_torch_pos_radius, light.held_torch_color_strength,
            light.torch_meta.y, world_pos, n, to_eye, roughness, f0, metallic, foliage, true,
        );
        torch_diffuse += ht.diffuse;
        torch_fill    += ht.fill;
        torch_spec    += ht.spec;
    }

    // --- Linear HDR composition ---
    let sun_radiance = max(light.sun_color_daylight.rgb, vec3<f32>(0.0)) * sun_spectral;
    let moon_radiance = max(light.moon_color_twilight.rgb, vec3<f32>(0.0)) * moon_spectral;
    let sun_h = terrain_safe_unit3(to_eye + sun_dir, n);
    let moon_h = terrain_safe_unit3(to_eye + moon_dir, n);
    let sun_diffuse_energy = (vec3<f32>(1.0) - terrain_fresnel(f0, dot(to_eye, sun_h))) * (1.0 - metallic);
    let moon_diffuse_energy = (vec3<f32>(1.0) - terrain_fresnel(f0, dot(to_eye, moon_h))) * (1.0 - metallic);
    var lit = albedo * (
        diffuse_energy * ((ambient + sun_fill + moon_fill) * sky_access + cave_fill)
        + sun_radiance * sun_diffuse_energy * sun_diffuse * 0.318309886
        + moon_radiance * moon_diffuse_energy * moon_diffuse * 0.318309886 * sky_access
        + (torch_diffuse + torch_fill) * fire_exposure
    );
    // Inter-ridge occlusion comes from the detail-map sun march in `sun_shadow`.
    let direct_specular = sun_radiance * terrain_directional_specular(n, macro_normal, to_eye, sun_dir,
            roughness, f0, in.ao_strength, kind, in.terrain_weights.z)
            * max(light.sun_dir_intensity.w, 0.0) * sun_trans * sun_shadow * sun_horizon * sky_access
        + moon_radiance * terrain_directional_specular(n, macro_normal, to_eye, moon_dir,
            roughness, f0, in.ao_strength, kind, in.terrain_weights.z)
            * max(light.moon_dir_intensity.w, 0.0) * moon_trans * moon_horizon * sky_access
        + torch_spec * fire_exposure;

    if debug_mode == 23 { // Directional diffuse + specular, linear HDR.
        let direct = albedo * (sun_radiance * sun_diffuse_energy * sun_diffuse * 0.318309886
            + moon_radiance * moon_diffuse_energy * moon_diffuse * 0.318309886 * sky_access)
            + direct_specular - torch_spec * fire_exposure;
        return debug_surface_output(vec4<f32>(max(direct, vec3<f32>(0.0)), 1.0));
    }
    if debug_mode == 24 { // Existing ambient/bounce diffuse; excludes environment specular.
        return debug_surface_output(vec4<f32>(albedo * diffuse_energy
            * ((ambient + sun_fill + moon_fill) * sky_access + cave_fill), 1.0));
    }
    if debug_mode == 25 { // All local-light diffuse/fill/specular contributions.
        return debug_surface_output(vec4<f32>((albedo * (torch_diffuse + torch_fill) + torch_spec) * fire_exposure, 1.0));
    }

    // A bounded sky-environment approximation for opaque materials. It supplies
    // a broad response for metals and wet rock; a prefiltered environment/LUT
    // from the renderer is needed for accurate rough reflections of the scene.
    if kind != 8 && kind != 9 && kind != 54 && kind != 55 {
        let reflection_dir = reflect(-to_eye, n);
        let sky_direction = terrain_safe_unit3(vec3<f32>(reflection_dir.x,
            max(reflection_dir.y, 0.02), reflection_dir.z), vec3<f32>(0.0, 1.0, 0.0));
        let reflected_sky = atmosphere_color(sky_direction);
        let environment_mean = mix(sky_horizon, sky_up, 0.55);
        let environment = mix(reflected_sky, environment_mean, roughness * roughness);
        let grazing = vec3<f32>(max(1.0 - roughness, 0.0));
        let environment_fresnel = f0 + (max(grazing, f0) - f0)
            * terrain_pow5(1.0 - clamp(dot(n, to_eye), 0.0, 1.0));
        // F0 suppresses diffuse above, so this response is not layered on a
        // full-strength diffuse reflector. Screen-space or probe GI stays external.
        lit += max(environment, vec3<f32>(0.0)) * environment_fresnel
            * sky_access * cavity * mix(0.35, 1.0, metallic);
        lit += direct_specular;
    }

    // A small neutral first-person fill keeps the hand and sleeve readable in
    // deep caves without turning them into shiny world objects. It is applied
    // only to the authored viewmodel materials.
    if kind == 54 || kind == 55 {
        lit += albedo * vec3<f32>(0.075, 0.078, 0.082)
            * (0.55 + 0.45 * hemi);
    }

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
    if kind == 18 {
        // Glowcaps breathe as a colony, not as a constant green sticker.
        let pulse = 0.62 + 0.38 * sin(
            world_pos.x * 5.2 + world_pos.z * 4.7 + light.torch_meta.y * 2.4
        );
        let cap = mix(albedo, albedo.brg, 0.28);
        // Emissive, so it keeps its authored luminance at night like fire does.
        lit += (cap * (0.24 + 0.28 * pulse) + vec3<f32>(0.004, 0.022, 0.014) * pulse) * fire_exposure;
    }
    if kind == 37 {
        // Amber gills: a steady warm glow, strongest on the shaded undersides.
        // The ground around them is lit by the indexed local light, not by this.
        let under = select(0.55, 1.0, geometric_normal.y < -0.5);
        let gill = albedo * vec3<f32>(1.0, 0.82, 0.62);
        lit += gill * (0.20 + 0.16 * under) * fire_exposure;
    }
    if kind == 22 {
        // A quiet inner rim preserves the violet identity in caves. Direct
        // highlights already come from the shared BRDF; do not stack a second lobe.
        let view_fresnel = terrain_pow5(1.0 - clamp(dot(n, to_eye), 0.0, 1.0));
        lit += albedo * (0.07 + view_fresnel * 0.16) * fire_exposure;
    }
    if kind == 8 || kind == 9 {
        let reflected = reflect(-to_eye, n);
        let ndotv = clamp(dot(n, to_eye), 0.0, 1.0);
        // Physical dielectric Schlick Fresnel for water (n = 1.333 -> F0 = 0.0204).
        let reflection_f0 = select(clamp(dot(f0, vec3<f32>(0.333333333)), 0.0, 1.0), 0.0204, kind == 8);
        let fresnel = (reflection_f0 + (1.0 - reflection_f0) * terrain_pow5(1.0 - ndotv))
            * (1.0 - water_foam * 0.68);
        let refl_dir = normalize(vec3<f32>(reflected.x, max(reflected.y, 0.025), reflected.z));
        var reflection = atmosphere_color(refl_dir);
        // Preserve zenith-vs-horizon sky dome gradient and warm sun-azimuth sheen
        // even under cloudy skies so water reflections never flatten into grey paint.
        let refl_elev = clamp(refl_dir.y, 0.0, 1.0);
        let sun_align = pow(max(dot(terrain_safe_unit2(refl_dir.xz, vec2<f32>(0.0, 1.0)), terrain_safe_unit2(sun_dir.xz, vec2<f32>(0.0, 1.0))), 0.0), 3.0);
        // The cloud-softened dome comes from the reflected direction's own sky.
        // It used the solar horizon for every azimuth, so water facing away
        // from a sunset mirrored the sunset side as a flat bright sheet.
        let reflected_luma = dot(reflection, vec3<f32>(0.2126, 0.7152, 0.0722));
        let cloud_sky_dome = mix(
            mix(reflection, vec3<f32>(reflected_luma), 0.35) * (1.0 + sun_align * 0.28 * sun_trans),
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
            * select(1.0, 0.0, water_side)
            * sun_trans * sun_shadow * max(light.sun_dir_intensity.w, 0.0)
            * sky_access
            * (1.0 - water_foam * 0.75);
        lit += sss_tint * light.sun_color_daylight.rgb * sun_spectral * sss_energy * 0.17;

        // Sky radiance already follows time of day. Dimming it again made
        // night water read as black cutout shadows against illuminated banks.
        // No reflection floor or cap: Schlick approaches one at grazing angles.
        lit = mix(lit, reflection * sky_access, clamp(fresnel, 0.0, 1.0));
        lit += direct_specular * (1.0 - water_foam * 0.65);
        // Sun glint restored from main: a tight core and a broad sheen on top of
        // the GGX highlight and the texel glitter below.
        let sun_glint_sharp = water_glint_lobe(n, to_eye, sun_dir, max(roughness * 0.72, 0.058));
        let sun_glint_broad = water_glint_lobe(n, to_eye, sun_dir, max(roughness * 1.60, 0.17));
        // Wave-advected breakup of the sharp core. Far filtering widens the lobe
        // until the sun path is a smooth column; real glitter is broken up by
        // swell. Mean-preserving (0.4..1.6, mean 1.0), filtered by pixel footprint
        // so it fades instead of aliasing, and it moves only with the wave clock.
        let glint_t = light.torch_meta.y;
        let glint_breakup = 0.4 + 1.2 * surface_noise_filtered(
            world_pos.xz * 0.45 + vec2<f32>(glint_t * 0.30, -glint_t * 0.21), fp_m * 0.45);
        lit += light.sun_color_daylight.rgb * sun_spectral
             * (sun_glint_sharp * glint_breakup * 0.105 + sun_glint_broad * 0.045)
             // Fully gated: an 18% floor let the glint shine through
             // terrain shadow and cloud, doubling the path under overcast.
             * light.sun_dir_intensity.w * sun_trans * sun_shadow
             * sky_access * (1.0 - water_foam * 0.65);
        if geometric_normal.y > 0.5 && terrain_has_resident_metadata(in.ao_strength) {
            // Sun glitter: each 5 cm texel is a tiny facet with a fixed random tilt
            // added to the wave normal. It lights when that facet mirrors the sun
            // to the eye, so glints are hard texels that drift and fade as the
            // waves move, with no per-frame re-roll. Fades out with the texel size.
            let cell = floor(world_pos.xz * PIXEL_RES);
            let tilt = vec2<f32>(mood_hash31(vec3<f32>(cell, 1.3)), mood_hash31(vec3<f32>(cell, 7.9))) - vec2<f32>(0.5);
            let facet = normalize(n + vec3<f32>(tilt.x, 0.0, tilt.y) * 0.55);
            let half_vec = normalize(to_eye + sun_dir);
            let align = clamp((dot(facet, half_vec) - 0.9965) / 0.0035, 0.0, 1.0);
            let glint = align * align * surface_detail_weight(fp_m * PIXEL_RES) * (1.0 - water_foam)
                * smoothstep(0.02, 0.25, sun_dir.y);
            lit += sun_radiance * sun_shadow * sun_trans * glint * 0.5;
        }
    }

    if kind == 10 {
        let crust = surface_noise_filtered(world_pos.xz * 2.8, fp_m * 2.8);
        let molten = smoothstep(0.38, 0.72, crust);
        lit += mix(vec3<f32>(0.55, 0.035, 0.003), vec3<f32>(2.8, 0.65, 0.035), molten) * fire_exposure;
    }

    // Keep the normal exposure/tone resolve for this diagnostic, while removing
    // aerial perspective, cloud-layer fog, streaming veil and local mist. This
    // exposes geometry/lighting discontinuities instead of hiding them in haze.
    if debug_mode == 26 {
        return debug_surface_output(vec4<f32>(max(lit, vec3<f32>(0.0)), 1.0));
    }

    // ============================================================
    // AERIAL PERSPECTIVE
    // Shared contract: aerial_perspective returns (in-scattered radiance,
    // transmittance), i.e. RGB is already premultiplied by path extinction.
    // ============================================================
    let raw_sky_radiance = max(atmosphere_color(view_dir), vec3<f32>(0.0));
    let sky_luma = dot(raw_sky_radiance, vec3<f32>(0.2126, 0.7152, 0.0722));
    let weather_sky_radiance = mix(raw_sky_radiance, vec3<f32>(sky_luma), heavy_cloud * 0.55)
        * mix(1.0, 0.78, heavy_cloud * daylight);
    var atmosphere_transmittance = 1.0;
    var atmosphere_scatter = vec3<f32>(0.0);
    if atmosphere_mode() >= ATMOSPHERE_MODE_SKY_VIEW_LUT {
        let ap = aerial_perspective(view_dir, dist, scale, vec3<f32>(0.0));
        atmosphere_transmittance = clamp(ap.a, 0.0, 1.0);
        atmosphere_scatter = max(ap.rgb, vec3<f32>(0.0));
    } else {
        // Integrate density over the ray, including horizontal and layer-crossing
        // views. Endpoint height alone made haze change when looking up/down.
        let fog_start = max(light.fog_params.x, 0.0);
        let path = max(dist - fog_start, 0.0);
        let fog_range = max(light.fog_params.y - fog_start, 1.0);
        let layer_base = light.fog_params.z * scale;
        let layer_k = max(light.fog_params.w, 0.0);
        let start_y = cam.camera_pos.y + view_dir.y * min(fog_start, dist);
        let broad_column = terrain_height_integral(start_y, world_pos.y, path,
            layer_base, layer_k);
        let valley_column = terrain_height_integral(start_y, world_pos.y, path,
            layer_base - 18.0 * scale, layer_k * 2.0);
        let density = mix(0.22, 0.10, daylight) + heavy_cloud * 0.10;
        let optical_depth = (1.35 + density * 3.2)
            * (0.72 * broad_column + 0.28 * valley_column) / fog_range;
        atmosphere_transmittance = exp(-clamp(optical_depth, 0.0, 80.0));
        atmosphere_scatter = weather_sky_radiance * (1.0 - atmosphere_transmittance);
    }
    // Cloud-layer fog remains the supplied homogeneous-layer approximation.
    // Give it its own radiance; do not inflate already integrated air scattering
    // by the cloud opacity or apply another solar lobe on top of the sky LUT.
    let cloud_fog = cloud_fog_amount(view_dir, dist)
        * mix(1.0, 0.25, cave_amount);
    let cloud_transmittance = 1.0 - cloud_fog;
    // Fog radiance is a property of the air, not of the surface normal.
    let cloud_fog_radiance = max(mix((solar_horizon + antisolar_horizon) * 0.5, sky_up, 0.50), vec3<f32>(0.0));
    lit = lit * cloud_transmittance + cloud_fog_radiance * (1.0 - cloud_transmittance);
    // Sky in-scatter should not flood a roofed tunnel with daylight haze.
    atmosphere_scatter *= mix(1.0, 0.28, cave_amount);
    lit = lit * atmosphere_transmittance + atmosphere_scatter;
    let total_fog_t = 1.0 - atmosphere_transmittance * cloud_transmittance;

    if uw_active() {
        if kind == 8 && world_pos.y > cam.camera_pos.y {
            // The surface above a diver is the same Snell's-window sky the sky pass paints
            // behind it, so the two meet without seams.
            let jitter = fract(dot(floor(in.pos.xy), vec2<f32>(0.754877666, 0.569840296)));
            lit = uw_backdrop(view_dir, jitter);
        } else {
            // Absorption, caustic light and a haze that matches the sky pass's underwater backdrop.
            lit = uw_apply_fragment(lit, albedo, world_pos, n, view_dir, dist, sun_shadow * sun_trans);
        }
    }
    // Aerial perspective already accounts for distance haze. Streaming fade
    // below is a separate coverage contract, not a second atmospheric model.

    // Residency is centred on a snapped chunk, not the exact camera. Reserve
    // a chunk diagonal so the closest missing column is already fully veiled.
    // Orbital infrastructure is independent of resident terrain coverage.
    // Keep the normal atmosphere/cloud occlusion and veil the bounded corridor
    // mesh range before its CPU visibility limit, instead of clipping at chunks.
    let corridor_fade = select(terrain_streaming_fade(horizontal_dist),
        smoothstep(5000.0, 6000.0, horizontal_dist),
        kind == 62 || (in.surface.y == 2.0 && (kind == 40 || kind == 41 || kind == 61)));
    // Immutable megastructures use their own bounded horizon coverage. Keep
    // local UVs and the normal physical atmosphere/cloud path intact.
    let coverage_fade = select(corridor_fade, smoothstep(16000.0, 20000.0, horizontal_dist),
        kind == 63 || (in.surface.y == 3.0 && (kind == 40 || kind == 41 || kind == 61)));
    let cable_coverage = select(1.0, clamp(in.surface.w, 0.0, 1.0), kind == 61);
    let edge_fade = 1.0 - (1.0 - coverage_fade) * cable_coverage;
    if edge_fade > 0.0 {
        // This must equal the sky pass in HDR at the outer boundary.
        lit = mix(lit, raw_sky_radiance, edge_fade);
    }

    // Nearby mist sits in front of the graded surface and distant haze, just
    // as it does in the sky pass. Later lighting must use its transmittance too.
    let mist = local_mist(view_dir, dist, sky_access, weather_sky_radiance);
    lit = lit * clamp(mist.a, 0.0, 1.0) + max(mist.rgb, vec3<f32>(0.0));
    // Inside a cosmos chamber the fog is the night sky of another universe.
    if light.mood.x > 0.001 {
        lit = mood_cosmos_fog(lit, view_dir, dist, light.mood.x, light.torch_meta.y, cam.camera_pos.xyz);
    }
    lit += mood_ambience(cam.camera_pos.xyz, view_dir, dist, light.torch_meta.y, light.ambience, ambience_pixel);
    let surface_visibility = (1.0-total_fog_t) * (1.0-edge_fade) * clamp(mist.a,0.0,1.0);

    // Sky, terrain and premultiplied clouds share one HDR resolve.
    var receiver = vec4<f32>(0.0);
    if kind == 8 && geometric_normal.y>0.5 {
        // Negative alpha tags reflection receivers. Reuse the existing HDR
        // receiver target: signed normal x/z, visibility, and Fresnel weight.
        let f=(0.0204+0.9796*terrain_pow5(1.0-clamp(dot(n,to_eye),0.0,1.0))) * (1.0-water_foam*0.68);
        receiver=vec4<f32>(n.x,n.z,surface_visibility*sky_access,-max(f,0.001));
    } else if kind != 8 {
        receiver = vec4<f32>(albedo * surface_visibility, max(dot(ambient * sky_access, vec3<f32>(0.2126,0.7152,0.0722)),0.0));
    }
    return WorldOutput(vec4<f32>(max(lit, vec3<f32>(0.0)), 1.0), receiver);
}

