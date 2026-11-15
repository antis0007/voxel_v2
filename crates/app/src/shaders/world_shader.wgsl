// ============================================================
// world_shader.wgsl
//
// Voxel terrain lit pass: stable surface detail and radiometric composition.
//
// Integrate with the SAME shared WGSL declarations as the supplied shader.
// Group 1 binding 1 is the held-kit atlas; vertex attributes remain unchanged.
// Internal varyings include interpolated
// natural-material coverage; material IDs are resolved only in the vertex stage.
// Camera depth remains reverse-Z; directional shadow maps remain forward-Z.
// Output stays linear HDR: the shared resolve owns exposure and tone mapping.
// This pass cannot repair missing residency, mismatched LOD topology, or caves
// that are absent from a heightfield. See INTEGRATION.txt for required contracts.
// ============================================================

// World shading keeps terrain shadowing and the shared cloud-shadow product in one
// combined group so the renderer stays within WGPU's four-bind-group limit.
@group(1) @binding(1) var held_kit_atlas: texture_2d<f32>;

// One bit per voxel in a window around the camera. origin.w is the edge
// length; zero means the window has not been uploaded yet.
struct TorchBlockage {
    origin: vec4<i32>,
    words: array<u32>,
};
@group(1) @binding(2) var<storage, read> torch_block: TorchBlockage;

struct VsIn {
    @location(0) pos         : vec3<f32>,
    @location(1) color       : vec3<f32>,
    @location(2) normal      : vec3<f32>,
    @location(3) surface     : vec4<f32>,
    @location(4) ao_strength : f32,
    // Per-draw transform at vertex slot 1 (render/origin.rs DrawTransform).
    // offset.w > 0.5: planet chunk. xyz is origin-frame metres, axes are
    // metres per voxel, and axis.w is the chart voxel offset.
    @location(5) draw_offset : vec4<f32>,
    @location(6) axis_x : vec4<f32>,
    @location(7) axis_y : vec4<f32>,
    @location(8) axis_z : vec4<f32>,
    // Corner corrections (DrawTransform::seam): x, y, z pairs, corners 0-3 then 4-7.
    @location(9) seam_x0 : vec4<f32>,
    @location(10) seam_x1 : vec4<f32>,
    @location(11) seam_y0 : vec4<f32>,
    @location(12) seam_y1 : vec4<f32>,
    @location(13) seam_z0 : vec4<f32>,
    @location(14) seam_z1 : vec4<f32>,
};

// Trilinear blend of a planet chunk's corner corrections at chunk voxel `p`.
fn world_planet_position(p: vec3<f32>, in: VsIn) -> vec3<f32> {
    let t = clamp(p / 32.0, vec3<f32>(0.0), vec3<f32>(1.0));
    let face = vec4<f32>((1.0 - t.x) * (1.0 - t.y), t.x * (1.0 - t.y), (1.0 - t.x) * t.y, t.x * t.y);
    let lo = face * (1.0 - t.z);
    let hi = face * t.z;
    return vec3<f32>(
        dot(in.seam_x0, lo) + dot(in.seam_x1, hi),
        dot(in.seam_y0, lo) + dot(in.seam_y1, hi),
        dot(in.seam_z0, lo) + dot(in.seam_z1, hi),
    );
}

struct VsOut {
    @builtin(position) pos        : vec4<f32>,
    @location(0)       base_color : vec3<f32>,
    @location(1)       relative_pos : vec3<f32>,
    // Absolute voxels, except face-local on a planet face (see vs_main).
    @location(2)       voxel_pos  : vec3<f32>,
    @location(3)       normal     : vec3<f32>,
    @location(4)       surface    : vec4<f32>,
    @location(5)       ao_strength: f32,
    @location(6) material_tint: vec4<f32>,
    @location(7) material_props: vec4<f32>,
    // Depth, sky access, and decoded surface current (m/s).
    @location(8) water_optics: vec4<f32>,
    @location(9) terrain_weights: vec4<f32>,
    @location(10) @interpolate(flat) material_family: u32,
    @location(11) gravel_coverage: f32,
    @location(12) model_uv: vec2<f32>,
    // Bit 0: viewmodel. Bits 1-3: atlas face that `voxel_pos` is local to.
    @location(13) @interpolate(flat) camera_relative: u32,
    @location(14) climate: vec2<f32>,
    // Voxel-to-render rotation of a bent planet chunk; zero means identity.
    @location(15) @interpolate(flat) frame: vec4<f32>,
};

// Rotation matrix columns to a unit quaternion (xyz, w).
fn frame_quat(ax: vec3<f32>, ay: vec3<f32>, az: vec3<f32>) -> vec4<f32> {
    let trace = ax.x + ay.y + az.z;
    if trace > 0.0 {
        let s = sqrt(trace + 1.0) * 2.0;
        return vec4<f32>((ay.z - az.y) / s, (az.x - ax.z) / s, (ax.y - ay.x) / s, 0.25 * s);
    }
    if ax.x > ay.y && ax.x > az.z {
        let s = sqrt(1.0 + ax.x - ay.y - az.z) * 2.0;
        return vec4<f32>(0.25 * s, (ay.x + ax.y) / s, (az.x + ax.z) / s, (ay.z - az.y) / s);
    }
    if ay.y > az.z {
        let s = sqrt(1.0 + ay.y - ax.x - az.z) * 2.0;
        return vec4<f32>((ay.x + ax.y) / s, 0.25 * s, (az.y + ay.z) / s, (az.x - ax.z) / s);
    }
    let s = sqrt(1.0 + az.z - ax.x - ay.y) * 2.0;
    return vec4<f32>((az.x + ax.z) / s, (az.y + ay.z) / s, 0.25 * s, (ax.y - ay.x) / s);
}

fn frame_rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    if dot(q, q) < 0.25 { return v; }
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

// Integrated-GPU shading tier (render::lean_shading). Desktop pipelines keep
// the default, so every lean branch is folded away there.
override LEAN_SHADING: bool = false;
override TERRAIN_BACKSTOP: bool = false;
// Same-binary cave-water comparison; enabled during ordinary play.
override CAVE_WATER_FAST: bool = true;

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

// Terrain, furniture and lichen share 20 texels/metre (5 cm pixels).
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
    // Fade in texels per screen pixel, like every other 5 cm lattice term.
    // The old 0.22-0.65 m (4-13 texels per pixel) window kept the 4x4 dither
    // alive far past Nyquist, where it aliased into broad moire bands on
    // flat canopy and wall faces.
    let mid = surface_detail_weight(footprint_m * PIXEL_RES);
    if mid <= 0.0 { return color; }
    let cell = floor(terrain_pixel_uv(p, n) * PIXEL_RES);
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

// Distance from a point in a resident column to the nearest unowned column,
// capped at the requested width. Diagonal neighbours can expose corners.
fn terrain_unowned_distance_squared(world_xz: vec2<f32>, span: f32, width: f32) -> f32 {
    let cell = floor(world_xz / span);
    let in_cell = world_xz - cell * span;
    let nearest_edge = min(in_cell, vec2<f32>(span) - in_cell);
    var distance_squared = width * width;
    if min(nearest_edge.x, nearest_edge.y) >= width { return distance_squared; }
    if width <= span * 0.5 {
        // Only the nearest edge on each axis, and their shared corner,
        // can be inside the search radius.
        let direction = select(vec2<f32>(-1.0), vec2<f32>(1.0), in_cell >= vec2<f32>(span * 0.5));
        let x_step = vec2<f32>(direction.x, 0.0);
        let z_step = vec2<f32>(0.0, direction.y);
        let x2 = nearest_edge.x * nearest_edge.x;
        let z2 = nearest_edge.y * nearest_edge.y;
        let half_cell = vec2<f32>(0.5);
        if x2 < distance_squared && !terrain_resident_column_owned((cell + x_step + half_cell) * span, 0.0) {
            distance_squared = x2;
        }
        if z2 < distance_squared && !terrain_resident_column_owned((cell + z_step + half_cell) * span, 0.0) {
            distance_squared = z2;
        }
        if x2 + z2 < distance_squared && !terrain_resident_column_owned((cell + direction + half_cell) * span, 0.0) {
            distance_squared = x2 + z2;
        }
        return distance_squared;
    }
    for (var z = -1; z <= 1; z += 1) {
        for (var x = -1; x <= 1; x += 1) {
            if x == 0 && z == 0 { continue; }
            let low = (cell + vec2<f32>(f32(x), f32(z))) * span;
            let delta = max(max(low - world_xz, world_xz - (low + vec2<f32>(span))), vec2<f32>(0.0));
            let d2 = dot(delta, delta);
            // A neighbour that cannot lower the minimum skips its hash lookup.
            if d2 < distance_squared && !terrain_resident_column_owned(low + vec2<f32>(span * 0.5), 0.0) {
                distance_squared = d2;
            }
        }
    }
    return distance_squared;
}

fn terrain_arrival_phase(world_xz: vec2<f32>, scale: f32) -> u32 {
    if cam.terrain_arrival.count.x == 0u { return 0u; }
    let column = vec2<i32>(floor(world_xz / (32.0 * scale)));
    let key = bitcast<vec2<u32>>(column);
    let mixed = (key.x * 0x9e3779b1u) ^ (key.y * 0x85ebca77u);
    var slot = (mixed ^ (mixed >> 16u)) & 255u;
    for (var i = 0u; i < 256u; i++) {
        let candidate = cam.terrain_arrival.slots[slot];
        if candidate.z == 0 { return 0u; }
        if all(candidate.xy == column) { return u32(candidate.z); }
        slot = (slot + 1u) & 255u;
    }
    return 0u;
}

fn terrain_dither(pixel: vec2<u32>) -> u32 {
    let x = pixel.x ^ pixel.y;
    let y = pixel.y;
    return ((x & 1u) << 5u) | ((y & 1u) << 4u)
        | ((x & 2u) << 2u) | ((y & 2u) << 1u)
        | ((x & 4u) >> 1u) | ((y & 4u) >> 2u);
}

fn terrain_backdrop_column_hidden(world_xz: vec2<f32>, scale: f32) -> bool {
    if cam.resident_coverage.header.w == 0 { return true; }
    // Keep the parent at exposed edges only. Drawing it across a whole
    // perimeter column lets its interpolated surface cut through resident voxels.
    let width = min(4.0 * scale, 4.0);
    return terrain_unowned_distance_squared(world_xz, 32.0 * scale, width) >= width * width;
}

fn terrain_water_ownership_weight(world_xz: vec2<f32>, scale: f32) -> f32 {
    if cam.resident_coverage.header.w == 0 { return 1.0; }
    if !terrain_resident_column_owned(world_xz, 0.0) { return 0.0; }
    let width = max(2.0 * scale, 0.24);
    let distance_squared = terrain_unowned_distance_squared(world_xz, 32.0 * scale, width);
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

fn water_vertex_point(xz: vec2<f32>, t: f32, footprint: f32) -> vec3<f32> {
    let wave = water_surface_packets(xz, t, footprint);
    return vec3<f32>(-wave.x * 1.45, wave.z, -wave.y * 1.45);
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
    let w00 = water_vertex_point(p00, t, step);
    if f.x == 0.0 && f.y == 0.0 { return w00; }
    if f.x == 0.0 {
        return mix(w00, water_vertex_point(p00 + vec2<f32>(0.0, step), t, step), f.y);
    }
    let w10 = water_vertex_point(p00 + vec2<f32>(step, 0.0), t, step);
    if f.y == 0.0 { return mix(w00, w10, f.x); }
    let w01 = water_vertex_point(p00 + vec2<f32>(0.0, step), t, step);
    let w11 = water_vertex_point(p00 + vec2<f32>(step, step), t, step);
    return mix(mix(w00, w10, f.x), mix(w01, w11, f.x), f.y);
}

// Mirrors `CAMERA_RELATIVE_FLAG` in mesh/models.rs: 64 * 4096, above any block key.
const CAMERA_RELATIVE_FLAG: f32 = 262144.0;

// Atlas face holding chart x (metres); 0 off a planet.
fn world_planet_face(chart_x_m: f32) -> u32 {
    if cam.planet[0].w <= 0.5 { return 0u; }
    let pitch = cam.planet[0].z;
    return u32(clamp(floor((chart_x_m + pitch * 0.5) / pitch), 0.0, 5.0));
}

// Chart voxel x of a face centre. Multiplies by the reciprocal scale, which
// WGSL rounds exactly, so both stages derive the same integer.
fn world_face_anchor(face: u32) -> i32 {
    return i32(round(f32(face) * cam.planet[0].z * cam.world_params.y));
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let scale     = max(cam.world_params.x, 1e-6);
    // Viewmodel vertices carry CAMERA_RELATIVE_FLAG on top of kind + 64 * block.
    let camera_relative = in.surface.z >= CAMERA_RELATIVE_FLAG;
    let surface_key = select(in.surface.z, in.surface.z - CAMERA_RELATIVE_FLAG, camera_relative);
    let geometry_kind=f32(u32(round(max(surface_key,0.0)))%64u);
    // `local` is origin-relative chart voxels. A bent chunk keeps that offset
    // in the axis `.w` lanes; `offset.xyz` is metres and must not be added here.
    // Viewmodel vertices are camera-relative already and take no offset.
    let bent = !camera_relative && (u32(in.draw_offset.w) & 1u) != 0u;
    let chart_offset = select(in.draw_offset.xyz, vec3<f32>(in.axis_x.w, in.axis_y.w, in.axis_z.w), bent);
    let local = select(in.pos + chart_offset, in.pos, camera_relative);
    // Phases and patterns read the absolute voxel position, bit-identical to
    // the raw vertex position while the origin is zero.
    let absolute = select(vec3<f32>(cam.render_origin.xyz) + local, in.pos, camera_relative);
    // Planet faces sit PITCH metres apart on chart x, so a far face is hundreds
    // of km out where one f32 step outgrows half a texel. Patterns and phases
    // read face-local voxels; the integer anchor is subtracted before the cast.
    let pattern_face = select(world_planet_face(absolute.x * scale), 0u, camera_relative);
    let face_anchor = world_face_anchor(pattern_face);
    let pattern = select(vec3<f32>(f32(cam.render_origin.x - face_anchor), f32(cam.render_origin.y),
        f32(cam.render_origin.z)) + local, absolute, camera_relative || face_anchor == 0);
    // Displacements are deltas so the absolute and origin frames share them:
    // `disp` in voxels, `disp_m` in metres.
    var disp = vec3<f32>(0.0);
    var disp_m = vec3<f32>(0.0);
    if geometry_kind == 34.0 {
        disp = tuft_wind_offset(pattern, in.surface.y, scale, light.torch_meta.y,
            local_surface_wind());
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
        let phase = select(dot(pattern, vec3<f32>(0.73, 1.17, 0.41)), 0.0, camera_relative);
        let flame_time = light.torch_meta.y;
        let wave = sin(flame_time * 6.7 - lift * 2.6 + phase);
        let flutter = sin(flame_time * 10.9 - lift * 3.4 + phase * 1.7);
        // Tip travel is about 3 cm on a 35 cm flame: visible at 1280 px width.
        disp.x += (0.034 * wave + 0.012 * flutter) * sway / scale;
        disp.z += (0.022 * sin(flame_time * 5.3 - lift * 2.0 + phase * 0.8)
            + 0.008 * flutter) * sway / scale;
        // The flame shortens a little on each flick.
        disp.y -= 0.012 * sway * (0.5 + 0.5 * wave) / scale;
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
        // Inside the shell the tip points at the axis, not world up.
        var rise = vec3<f32>(0.0, 0.24 * life, 0.0);
        if light.fog_color_density.w > 2.0 {
            let radial = vec3<f32>(absolute.x, absolute.y - light.fog_color_density.w / scale, 0.0);
            if dot(radial, radial) > 1.0 {
                rise = -normalize(radial) * 0.24 * life;
            }
        }
        disp = (drift + rise) / scale + rest * (grow - 1.0);
    }
    out.water_optics = vec4<f32>(10.0, 1.0, 0.0, 0.0);
    let liquid_current = (geometry_kind == 8.0 || geometry_kind == 10.0) && in.surface.y >= 8388608.0;
    if liquid_current {
        // Decode before interpolation: packed integer bits cannot interpolate.
        let packed = in.surface.y - 8388608.0;
        let step = floor(packed / 16384.0);
        let rest = packed - step * 16384.0;
        let x = floor(rest / 128.0);
        let signed_flow = (vec2<f32>(x, rest - x * 128.0) - vec2<f32>(64.0)) / 63.0;
        out.water_optics = vec4<f32>(out.water_optics.xy, signed_flow * abs(signed_flow) * 2.5);
        if geometry_kind == 8.0 {
            let interval = max(0.08 * scale, 1.0 / 30.0);
            // Linear travel keeps consecutive level updates from repeatedly
            // braking the surface. The upload clock still bounds the step.
            let blend = clamp((light.torch_meta.y - in.surface.x) / interval, 0.0, 1.0);
            disp.y += (step - 127.0) / 127.0 * (1.0 - blend);
        }
    }
    // surface.w = 8 + depth*64 + 4096*pinned + 8192*shore + 16384*direction + sky/2.
    // Water steps pin the mesh, while solid contact supplies the foam.
    var water_pinned = 0.0;
    var water_shore = 0.0;
    var breaker_direction = 0u;
    let resident_water = geometry_kind == 8.0 && in.surface.w >= 8.0 && terrain_has_resident_metadata(in.ao_strength);
    if resident_water {
        let packed_all = floor(in.surface.w - 8.0);
        breaker_direction = u32(packed_all / 16384.0);
        let packed = packed_all - f32(breaker_direction) * 16384.0;
        water_shore = floor(packed / 8192.0);
        let k = packed - water_shore * 8192.0;
        water_pinned = floor(k / 4096.0);
        out.water_optics = vec4<f32>((k - 4096.0 * water_pinned) / 64.0,
            clamp(fract(in.surface.w)*2.0,0.006,1.0), out.water_optics.zw);
    }
    let model_pos = absolute + disp;
    // Absolute metres before the metre displacements: ownership, coverage and
    // distance tests stay in the frame they were tuned in.
    let world_base = model_pos * scale;
    let horiz_dist = length((world_base - cam.camera_pos.xyz).xz);
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
        let marine = water_marine_weight(world_base.y, scale);
        let current = smoothstep(0.035, 0.38, length(out.water_optics.zw));
        let wind = smoothstep(0.20, 0.82, out.water_optics.y)
            * mix(0.50, 1.0, marine) * (1.0 - current)
            * smoothstep(0.0, 7.0, length(local_surface_wind()));
        if dist_atten > 0.0 && pin > 0.0 && surf_fade > 0.0 && wind > 0.0 {
            let owner_weight = terrain_water_ownership_weight(world_base.xz, scale);
            if owner_weight > 0.0 {
                let wave_xz = pattern.xz + disp.xz;
                let wave_disp = eval_water_vertex_wave(wave_xz, scale, light.torch_meta.y);
                // Wave displacement is already in metres. Only its amplitude
                // fades to zero at the shared source surface; no terrain sink.
                disp_m += wave_disp * (shoal * surf_fade * dist_atten * pin * owner_weight * wind);
                let breaker_energy = water_breaker_band(depth) * dist_atten * pin * owner_weight * wind;
                if breaker_direction != 0u && breaker_energy > 0.0 {
                    let breaker = water_breaker(wave_xz * scale, depth, light.torch_meta.y);
                    let crest = water_breaker_crest(breaker.x) * breaker.y;
                    let shoreward = array<vec2<f32>, 4>(
                        vec2<f32>(1.0, 0.0), vec2<f32>(-1.0, 0.0),
                        vec2<f32>(0.0, 1.0), vec2<f32>(0.0, -1.0),
                    )[min(breaker_direction - 1u, 3u)];
                    // Lift the crest, then throw its lip into the shallows.
                    let plunge = smoothstep(0.43, 0.61, breaker.x);
                    disp_m.y += crest * min(depth * 0.64, 0.36) * breaker_energy;
                    let roll = shoreward * (min(depth * 0.8, 0.38) * crest * plunge * breaker_energy);
                    disp_m += vec3<f32>(roll.x, 0.0, roll.y);
                }
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
            disp_m.y -= underlap * (0.45 * scale + relief_sink);
        }
        // Scale bias by the surface's own reverse-Z depth so perspective
        // hyperbolic depth never saturates or flattens across the overlap zone.
        // Backdrop water sits in the same plane as resident water but carries no
        // waves, so resident troughs dipped below it and let its different colour
        // show through as streaks. Push it back too, less than the terrain it
        // overlies so water still lies over the seabed.
        backdrop_depth_bias = underlap * select(select(0.0, 0.002, geometry_kind == 8.0), 0.0035, geometry_kind == 0.0);
    }
    let world_pos = world_base + disp_m;
    // Origin-frame vertices meet the eye in small exact numbers. Viewmodel
    // vertices (CAMERA_RELATIVE_FLAG) are already camera-relative.
    let origin_relative = (local + disp) * scale + disp_m - cam.eye_from_origin.xyz;
    var vertex_relative = select(origin_relative, world_pos, camera_relative);
    var planet_face = 0u;
    var planet_coord = world_pos;
    // ao >= 4 is the tangent-chart celestial layer (skyhooks, stations, meteors).
    // Those longitudes sit outside the six atlas faces; tan() then wraps and
    // the cables leave the planet as rays. The chart frame is already camera-safe.
    // Resident chunks carry a frame (offset.w). All negative-AO chart meshes,
    // including exact proxies and seam curtains, share the cover projection.
    // Sending their shared edge through the props path splits it from the ground.
    let cover = cam.planet[0].w > 0.5 && !camera_relative && in.ao_strength < 0.0;
    if bent {
        let p = in.pos + disp;
        let delta = p - clamp(in.pos, vec3<f32>(0.0), vec3<f32>(32.0));
        var metres = world_planet_position(in.pos, in)
            + in.axis_x.xyz * delta.x + in.axis_y.xyz * delta.y + in.axis_z.xyz * delta.z;
        metres += terrain_safe_unit3(in.axis_x.xyz, vec3<f32>(1.0, 0.0, 0.0)) * disp_m.x
            + terrain_safe_unit3(in.axis_y.xyz, vec3<f32>(0.0, 1.0, 0.0)) * disp_m.y
            + terrain_safe_unit3(in.axis_z.xyz, vec3<f32>(0.0, 0.0, 1.0)) * disp_m.z;
        vertex_relative = metres - cam.eye_from_origin.xyz;
    } else if cover {
        planet_face = u32(clamp(floor((world_pos.x + cam.planet[0].z * 0.5) / cam.planet[0].z),0.0,5.0));
        let face_origin = i32(f32(planet_face) * cam.planet[0].z / scale);
        let origin_on_face = vec3<f32>(f32(cam.render_origin.x-face_origin),f32(cam.render_origin.y),f32(cam.render_origin.z));
        planet_coord = (origin_on_face + local + disp) * scale + disp_m;
        vertex_relative = planet_project(planet_face, planet_coord);
    } else if cam.planet[0].w > 0.5 && !camera_relative && in.ao_strength < 4.0 {
        // Attached geometry uses the same chart projection as its voxel shaft.
        // A distance blend rotated tall props away from their ground attachment
        // even nearby (the lateral error grows with the elevator's height).
        let face = u32(clamp(floor((world_pos.x + cam.planet[0].z * 0.5) / cam.planet[0].z), 0.0, 5.0));
        let face_origin = i32(f32(face) * cam.planet[0].z / scale);
        let origin_on_face = vec3<f32>(f32(cam.render_origin.x - face_origin), f32(cam.render_origin.y), f32(cam.render_origin.z));
        let coord = (origin_on_face + local + disp) * scale + disp_m;
        vertex_relative = planet_project(face, coord);
    }
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
    if terrain_is_backdrop(in.ao_strength) && cam.resident_coverage.header.w != 0
        && terrain_resident_column_owned(world_base.xz, horiz_dist) {
        // Depth-only separation leaves the shared geometric edge in place.
        out.pos.z -= max(out.pos.z, 0.0) * 0.00015;
    }
    // A planet-cover facet is hundreds of metres across, so its chord cuts
    // the voxel surface along a residency edge. Bias it behind that surface.
    if (cam.planet[0].w > 0.5 && in.ao_strength < 0.0 && in.ao_strength > -1.5) {
        out.pos.z -= max(out.pos.z, 0.0) * 0.0015;
    }
    // The held hand and wand share the world depth buffer, so pressing against
    // a wall used to slice them. Reverse-Z depth is z/w with 1 at the camera;
    // moving it halfway toward 1 is monotonic, so the pieces still sort against
    // each other, but they now win the depth test against anything beyond a few
    // centimetres of the lens.
    if camera_relative {
        // Fixed 72-degree vertical viewmodel projection, independent of sprint
        // FOV. Keep aspect and world camera roll; depth remains monotonic.
        let forward=terrain_safe_unit3(cam.camera_forward.xyz, vec3<f32>(0.0,0.0,-1.0));
        let right=terrain_safe_unit3(cam.ray_right.xyz, vec3<f32>(1.0,0.0,0.0));
        let up=terrain_safe_unit3(cam.ray_up.xyz, vec3<f32>(0.0,1.0,0.0));
        let aspect=length(cam.ray_right.xyz)/max(length(cam.ray_up.xyz),0.001);
        let depth=max(dot(vertex_relative,forward),0.03);
        let focal=1.376382;
        out.pos.x=dot(vertex_relative,right)*focal/aspect;
        out.pos.y=dot(vertex_relative,up)*focal;
        out.pos.w=depth;
        out.pos.z += (out.pos.w - out.pos.z) * 0.5;
        // The held item belongs to the primary eye, not a portal's virtual one.
        if portal_view() { out.pos = vec4<f32>(2.0, 2.0, 0.5, 1.0); }
    }
    // Decode BEFORE interpolation, so gradients and biome averages are linear.
    out.base_color = terrain_srgb_to_linear(in.color);
    out.relative_pos = vertex_relative;
    out.voxel_pos  = pattern;
    out.normal     = terrain_safe_unit3(in.normal, vec3<f32>(0.0, 1.0, 0.0));
    out.frame      = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    if bent {
        let fx = terrain_safe_unit3(in.axis_x.xyz, vec3<f32>(1.0, 0.0, 0.0));
        let fy = terrain_safe_unit3(in.axis_y.xyz, vec3<f32>(0.0, 1.0, 0.0));
        let fz = terrain_safe_unit3(in.axis_z.xyz, vec3<f32>(0.0, 0.0, 1.0));
        out.normal = normalize(fx * in.normal.x + fy * in.normal.y + fz * in.normal.z);
        out.frame = normalize(frame_quat(fx, fy, fz));
    } else if cover {
        // Step back into the face on its +x/+z border: the projection clamps
        // there, a forward step has zero length and the normal became NaN,
        // which drew every face edge as a black line.
        let sx = select(1.0, -1.0, planet_coord.x > 0.0);
        let sz = select(1.0, -1.0, planet_coord.z > 0.0);
        let north = normalize((planet_project(planet_face, planet_coord + vec3<f32>(0.0, 0.0, sz)) - vertex_relative) * sz);
        let east = normalize((planet_project(planet_face, planet_coord + vec3<f32>(sx, 0.0, 0.0)) - vertex_relative) * sx);
        let up = normalize(cross(north,east));
        out.normal = normalize(east * in.normal.x + up * in.normal.y + north * in.normal.z);
        if in.ao_strength < -1.5 {
            out.frame = normalize(frame_quat(east, up, north));
        }
    }
    out.surface    = in.surface;
    if resident_water { out.surface.w = 8.0 + water_shore; }
    out.surface.z = geometry_kind;
    out.material_family = u32(geometry_kind);
    let transition = u32(in.draw_offset.w) >> 1u;
    out.camera_relative = select(0u, 1u, camera_relative) | (pattern_face << 1u) | (transition << 4u);
    // Model range fade (Model::fade_since): normal length 2 - coverage. Decoded
    // per vertex, before interpolation shortens it, into the arrival bits so
    // the fragment stage drops pixels with the chunk-arrival dither.
    let fade_length = length(in.normal);
    if !camera_relative && (out.camera_relative >> 12u) == 0u && fade_length > 1.004 {
        let coverage = clamp(2.0 - fade_length, 0.0, 1.0);
        out.camera_relative |= (1u + u32(round(coverage * 64.0))) << 12u;
    }
    out.ao_strength = in.ao_strength;
    // Resolve IDs at vertices; interpolate colors/finish, never material IDs.
    let far_terrain=in.ao_strength<0.0 && in.ao_strength> -1.5 && surface_key==0.0;
    let key=u32(round(max(select(surface_key,in.surface.w,far_terrain),0.0)));
    // The capped face is a grass renderer variant; preserve its configured
    // tint/response and stable block key while the draw diagnostic reports 36.
    let family=select(key%64u, 1u, geometry_kind == 36.0);
    let block_key=key/64u;
    // Packed voxel keys carry a stable block ID; authored models use bare families.
    // Keep architectural voxel families distinct from rigid model finishes 54..58.
    let architecture = block_key > 0u && family >= 54u && family <= 58u;
    let creature = block_key == 0u && geometry_kind == 39.0 && in.surface.w >= 69.0 && in.surface.w <= 72.0;
    out.material_family = select(select(u32(geometry_kind), family + 10u, architecture), u32(max(in.surface.w,0.0)), creature);
    out.material_family |= (min(block_key, 0xffffffu) << 8u);
    out.terrain_weights = terrain_coverage_for_family(family);
    out.climate = vec2<f32>(0.5);
    if out.terrain_weights.x > 0.0 || family == 6u { out.climate = terrain_surface_climate(world_base, 0.0); }
    out.gravel_coverage = select(0.0, 1.0, family == 33u);
    out.model_uv = select(vec2<f32>(0.0), in.surface.xy,
        creature || family == 57u || family == 58u || family == 59u || family == 60u || family == 62u || family == 63u
        || (camera_relative && (family == 54u || family == 55u)));
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
    let specular_channel = select(in.surface.y, select(0.42, 0.48, geometry_kind == 10.0),
        orbital_mesh || liquid_current);
    let mesh_specular = select(clamp(specular_channel, 0.0, 1.0), 0.0, geometry_kind == 34.0);
    let mesh_reflectance = clamp(0.5 * sqrt(mesh_specular / 0.04), 0.0, 1.0);
    // Stone/basalt's mesh gains predate physical reflectance. Unset rock uses
    // the studio's dielectric default; resolved family/individual overrides win.
    let default_reflectance = select(mesh_reflectance, 0.5, family == 3u);
    properties.x=select(default_reflectance, properties.x, properties.x>=0.0);
    out.material_tint=tint;
    out.material_props=properties;
    if !camera_relative && in.surface.w < 0.0 && in.surface.y <= -1024.0 {
        let packed = i32(-1024.0 - in.surface.y);
        let offset = vec3<i32>(packed & 15, (packed >> 4) & 15, packed >> 8) - vec3<i32>(8);
        let owner = cam.render_origin.xyz + vec3<i32>(floor(local)) + offset;
        if placed_torch_extinguished(owner) {
            out.pos = vec4<f32>(2.0, 2.0, 0.5, 1.0);
        }
    }
    return out;
}

// Word offsets in `torch_block.words`, sized for the largest window
// (render/torch_occlusion.rs `MAX_DIM`): voxel bits, one bit per 4-voxel
// block, then the sorted extinguished cell list.
const TORCH_COARSE_WORD = 192u * 192u * 192u / 32u;
const TORCH_EXTINGUISHED_WORD = TORCH_COARSE_WORD + 48u * 48u * 48u / 32u;

fn placed_torch_extinguished(cell: vec3<i32>) -> bool {
    const header = TORCH_EXTINGUISHED_WORD;
    var lo = 0u;
    var hi = torch_block.words[header];
    while lo < hi {
        let mid = lo + (hi - lo) / 2u;
        let base = header + 1u + mid * 3u;
        let other = vec3<i32>(bitcast<i32>(torch_block.words[base]),
            bitcast<i32>(torch_block.words[base + 1u]), bitcast<i32>(torch_block.words[base + 2u]));
        if all(other == cell) { return true; }
        if other.x < cell.x || (other.x == cell.x &&
            (other.y < cell.y || (other.y == cell.y && other.z < cell.z))) {
            lo = mid + 1u;
        } else {
            hi = mid;
        }
    }
    return false;
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
    // Unit N/V/L: the tangent lengths in cos(phi) cancel sin(alpha) and
    // sin(beta). Keep the projected dot instead of normalizing two tangents
    // and reconstructing their lengths with two more square roots. This also
    // stays defined when either tangent vanishes at normal incidence.
    let azimuth = max(dot(lp, vp), 0.0);
    return nl * (a + b_coef * azimuth / max(max(nl, nv), 1e-3));
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

// Unit-radius offsets for the fixed 0.39-radian rotation. Keep the four
// evenly spaced rim probes and the twelve golden-angle interior taps.
const PCF_RIM_OFFSETS = array<vec2<f32>, 4>(
    vec2<f32>(0.906410879, 0.372584647),
    vec2<f32>(-0.372584650, 0.906410877),
    vec2<f32>(-0.906410876, -0.372584653),
    vec2<f32>(0.372584656, -0.906410875),
);
const PCF_DISK_OFFSETS = array<vec2<f32>, 12>(
    vec2<f32>(0.188796271, 0.077605635),
    vec2<f32>(-0.331920427, 0.121773683),
    vec2<f32>(0.209774788, -0.405373743),
    vec2<f32>(0.140973825, 0.521337748),
    vec2<f32>(-0.517178200, -0.327912656),
    vec2<f32>(0.666478986, -0.118907927),
    vec2<f32>(-0.446934246, 0.584736220),
    vec2<f32>(-0.070281201, -0.787439237),
    vec2<f32>(0.621428819, 0.567591011),
    vec2<f32>(-0.889755547, 0.001316399),
    vec2<f32>(0.688809828, -0.632883102),
    vec2<f32>(-0.084142076, 0.975322226),
);

fn pcf_shadow(uv: vec2<f32>, proj_z: f32, bias: f32, radius: f32, cascade_idx: i32, gradient: vec2<f32>) -> f32 {
    // A fixed, off-axis golden-angle disk keeps the soft edge while preserving
    // tap positions relative to the receiver as the light projection moves.
    // Hashing the bits of projected UV re-rolled the disk even for sub-texel motion.
    let max_correction = radius * (abs(gradient.x) + abs(gradient.y));
    let g = gradient * min(1.0, 0.02 / max(max_correction, 1e-8));
    let z = proj_z - bias;
    let center = textureSampleCompareLevel(
        shadow_map, shadow_sampler, uv, cascade_idx, clamp(z, 0.0, 1.0));
    // Narrow near-cascade kernels can usually resolve with four rim taps.
    // Thin silhouettes can fall between the wider far-cascade probes, so
    // sample the full disk there.
    if cascade_idx < 4 {
        var probe = center;
        for (var k = 0; k < 4; k = k + 1) {
            let rim = PCF_RIM_OFFSETS[k] * radius;
            probe += textureSampleCompareLevel(
                shadow_map, shadow_sampler, uv + rim, cascade_idx,
                clamp(z + g.x * rim.x + g.y * rim.y, 0.0, 1.0));
        }
        if probe <= 0.0001 { return 0.0; }
        if probe >= 4.9999 { return 1.0; }
    }
    var sum = center * 0.18;
    for (var tap = 0; tap < 12; tap = tap + 1) {
        let offset = PCF_DISK_OFFSETS[tap] * radius;
        let gx = g.x * offset.x;
        let gy = g.y * offset.y;
        sum += textureSampleCompareLevel(
            shadow_map, shadow_sampler, uv + offset, cascade_idx,
            clamp(z + gx + gy, 0.0, 1.0)) * 0.0683333;
    }
    return clamp(sum, 0.0, 1.0);
}

// `receiver` is in render-origin metres, the frame of every cascade matrix.
fn sample_sun_shadow(receiver: vec3<f32>, linear_depth: f32, ndotl: f32, world_dx: vec3<f32>, world_dy: vec3<f32>) -> f32 {
    let shadow_range = max(shadow.cascades[4].far_plane, 1e-4);
    let range_fade = smoothstep(shadow_range * 0.78, shadow_range,
        distance(receiver, cam.eye_from_origin.xyz));
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
        let shadow_pos = c.light_view_proj * vec4<f32>(receiver, 1.0);
        if shadow_pos.w <= 1e-6 { continue; }
        let proj = shadow_pos.xyz / shadow_pos.w;
        let uv = proj.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
        let edge = min(min(uv.x, uv.y), min(1.0 - uv.x, 1.0 - uv.y));
        let texel = max(c.texel_size, 1e-8);
        let radius = texel * select(1.2, 2.0, i == 4);
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
    if coverage <= 0.0 {
        return 0.0;
    }

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
    if weather_clip_region(0).w == 0.0 {
        return vec4<f32>(0.0);
    }
    let uv0 = weather_uv(world_xz, 0);
    let weight0 = weather_level_weight(uv0, 0);
    if weight0 == 1.0 {
        return max(sample_weather_bilinear(cloud_shadow_tex, uv0, 0), vec4<f32>(0.0));
    }
    let uv1 = weather_uv(world_xz, 1);
    let weight1 = weather_level_weight(uv1, 1) * (1.0 - weight0);
    let weight2 = max(1.0 - weight0 - weight1, 0.0);
    // Avoid twelve texture loads when only one clip level contributes.
    var field = vec4<f32>(0.0);
    if weight0 > 0.0 {
        field += sample_weather_bilinear(cloud_shadow_tex, uv0, 0) * weight0;
    }
    if weight1 > 0.0 {
        field += sample_weather_bilinear(cloud_shadow_tex, uv1, 1) * weight1;
    }
    if weight2 > 0.0 {
        // Fade the finite outer map before its clamped edge can stretch across distant hills.
        let uv2 = weather_uv(world_xz, 2);
        let outer_coverage = smoothstep(0.0, 0.12, weather_edge_distance(uv2));
        if weight2 * outer_coverage > 0.0 {
            field += sample_weather_bilinear(cloud_shadow_tex, uv2, 2) * weight2 * outer_coverage;
        }
    }
    return max(field, vec4<f32>(0.0));
}

fn project_shadow_world_xz(world_pos: vec3<f32>, light_dir: vec3<f32>, cloud_height: f32) -> vec2<f32> {
    let t = max((cloud_height - world_pos.y) / max(light_dir.y, 1e-4), 0.0);
    return world_pos.xz + light_dir.xz * min(t, light.weather_clip_regions[2].z);
}

fn sample_shared_cloud_shadow_transmittance(world_pos: vec3<f32>, light_dir: vec3<f32>) -> f32 {
    if light_dir.y <= 0.02 || weather_clip_region(0).w == 0.0 {
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

// Flame height above its base, 0..1. The mesher packs it into surface.w as
// -2 - lift, so every existing `surface.w < -1.5` flame test still matches.
fn flame_lift(surface_w: f32) -> f32 {
    return clamp(-surface_w - 2.0, 0.0, 1.0);
}

// Light cast by a torch: a slow sway plus a quicker flutter, +/-8% around the
// same 0.96 mean as the torch body glow, so the average brightness of lights is kept.
fn torch_light_flicker(seed: vec3<f32>, time: f32) -> f32 {
    let phase = dot(seed, vec3<f32>(0.173, 0.697, 0.357));
    let sway = sin(time * 6.1 + phase);
    let flutter = sin(time * 11.3 + phase * 1.6);
    return clamp(0.96 + sway * 0.05 + flutter * 0.03, 0.88, 1.04);
}

// `d` is a voxel relative to the window origin.
fn blockage_solid(d: vec3<i32>) -> bool {
    let dim = torch_block.origin.w;
    if any(d < vec3<i32>(0)) || any(d >= vec3<i32>(dim)) { return false; }
    let span = u32(dim);
    let i = u32(d.x) + span * (u32(d.z) + span * u32(d.y));
    return (torch_block.words[i >> 5u] & (1u << (i & 31u))) != 0u;
}

// `b` is a 4-voxel block relative to the window origin. Blocks outside the
// window hold nothing.
fn blockage_block(b: vec3<i32>) -> bool {
    let dim = torch_block.origin.w / 4;
    if any(b < vec3<i32>(0)) || any(b >= vec3<i32>(dim)) { return false; }
    let span = u32(dim);
    let i = u32(b.x) + span * (u32(b.z) + span * u32(b.y));
    return (torch_block.words[TORCH_COARSE_WORD + (i >> 5u)] & (1u << (i & 31u))) != 0u;
}

fn axis_step(rd: f32, origin: f32, map: i32) -> vec3<f32> {
    if rd > 1e-6 {
        let inv = 1.0 / rd;
        return vec3<f32>(1.0, (f32(map) + 1.0 - origin) * inv, inv);
    }
    if rd < -1e-6 {
        let inv = 1.0 / rd;
        return vec3<f32>(-1.0, (f32(map) - origin) * inv, -inv);
    }
    return vec3<f32>(0.0, 1e30, 1e30);
}

// Same cells as render/torch_occlusion.rs `blocked`: skip the air cell in front
// of the face and the flame's own cell. An opaque voxel on the way stops the ray.
// A 4-voxel block with no opaque voxel is crossed in one step, so a ray through
// open air costs a quarter of the reads.
fn ray_hits_opaque(from_w: vec3<f32>, to_w: vec3<f32>) -> bool {
    if torch_block.origin.w <= 0 { return false; }
    // Window-local voxels. The origin is whole, so the subtraction is exact and
    // the cells are unchanged; the block exit below needs the small magnitudes.
    let window = vec3<f32>(torch_block.origin.xyz);
    let from_v = from_w - window;
    let to_v = to_w - window;
    let delta = to_v - from_v;
    let len = length(delta);
    if len < 0.5 { return false; }
    let rd = delta / len;
    var map = vec3<i32>(floor(from_v));
    let end = vec3<i32>(floor(to_v));
    if all(map == end) { return false; }
    let sx = axis_step(rd.x, from_v.x, map.x);
    let sy = axis_step(rd.y, from_v.y, map.y);
    let sz = axis_step(rd.z, from_v.z, map.z);
    var tmax = vec3<f32>(sx.y, sy.y, sz.y);
    let step = vec3<i32>(i32(sx.x), i32(sy.x), i32(sz.x));
    let tdelta = vec3<f32>(sx.z, sy.z, sz.z);
    let ahead = vec3<f32>(rd > vec3<f32>(0.0)) * 4.0;
    var first = true;
    var occupied = vec3<i32>(-1073741824);
    // 160 steps: torch_occlusion.rs `STEPS`. The walk ends at `len` first
    // unless the light is far in voxels, as at 0.25 m.
    for (var i = 0; i < 160; i++) {
        if all(map == end) { return false; }
        let block = map >> vec3<u32>(2u);
        if any(block != occupied) {
            if blockage_block(block) {
                occupied = block;
            } else {
                // Leave through the nearest far face of this clear block.
                let face = vec3<f32>(block * 4) + ahead;
                var leave = 1e30;
                if abs(rd.x) > 1e-6 { leave = min(leave, (face.x - from_v.x) / rd.x); }
                if abs(rd.y) > 1e-6 { leave = min(leave, (face.y - from_v.y) / rd.y); }
                if abs(rd.z) > 1e-6 { leave = min(leave, (face.z - from_v.z) / rd.z); }
                leave += 1e-3;
                if leave >= len { return false; }
                map = vec3<i32>(floor(from_v + rd * leave));
                tmax = vec3<f32>(axis_step(rd.x, from_v.x, map.x).y,
                    axis_step(rd.y, from_v.y, map.y).y, axis_step(rd.z, from_v.z, map.z).y);
                first = false;
                continue;
            }
        }
        if !first && blockage_solid(map) { return true; }
        first = false;
        if tmax.x <= tmax.y && tmax.x <= tmax.z {
            if tmax.x > len { return false; }
            map.x += step.x;
            tmax.x += tdelta.x;
        } else if tmax.y <= tmax.z {
            if tmax.y > len { return false; }
            map.y += step.y;
            tmax.y += tdelta.y;
        } else {
            if tmax.z > len { return false; }
            map.z += step.z;
            tmax.z += tdelta.z;
        }
    }
    return false;
}

// `frame` maps voxel axes to the render axes `geom_n` is in.
fn torch_reach(from_m: vec3<f32>, to_m: vec3<f32>, geom_n_render: vec3<f32>, frame: vec4<f32>) -> f32 {
    let geom_n = frame_rotate(vec4<f32>(-frame.xyz, frame.w), geom_n_render);
    let inv = cam.world_params.y;
    let to_v = to_m * inv;
    // The walk already skips its starting voxel. A half-cell normal offset
    // moves shallow-water receivers above nearby banks and through thin walls.
    let from_v = from_m * inv + geom_n * 0.03;
    // Sample the flame rather than sliding the receiver around corners.
    // Receiver offsets leaked light and drew curved lobes beside block edges.
    let flame_half_height = vec3<f32>(0.0, 0.12 * inv, 0.0);
    var visibility = 0.0;
    if !ray_hits_opaque(from_v, to_v) { visibility += 0.5; }
    if !ray_hits_opaque(from_v, to_v + flame_half_height) { visibility += 0.25; }
    if !ray_hits_opaque(from_v, to_v - flame_half_height) { visibility += 0.25; }
    return visibility;
}

fn eval_torch_light(
    torch_pos_radius: vec4<f32>, torch_color_strength: vec4<f32>,
    time: f32, world_pos: vec3<f32>, n: vec3<f32>, to_eye: vec3<f32>,
    roughness: f32, f0: vec3<f32>, metallic: f32, foliage: bool, moving: bool, frame: vec4<f32>,
) -> TorchLightSample {
    let radius = max(torch_pos_radius.w, 0.001);
    // Positions are voxel frame; `n` and `to_eye` are render frame.
    let to_torch = frame_rotate(frame, torch_pos_radius.xyz - world_pos);
    let dist2 = dot(to_torch, to_torch);
    let radius2 = radius * radius;
    if dist2 >= radius2 || torch_color_strength.w <= 0.0 {
        return TorchLightSample(vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0));
    }
    // (d / r)^4 from squared distances, without pow or sqrt.
    let fraction2 = dist2 / radius2;
    let cutoff = clamp(1.0 - fraction2 * fraction2, 0.0, 1.0);
    // Keep a readable pool around the source without summing a hall full of
    // distant flames into a uniform amber wash. Distances are physical metres;
    // the soft core preserves nearby surfaces and the finite cutoff is smooth.
    let falloff = cutoff * cutoff / (1.0 + dist2 * 0.14);
    let flicker_seed = select(torch_pos_radius.xyz * 0.23, vec3<f32>(0.0), moving);
    let flicker = torch_light_flicker(flicker_seed + vec3<f32>(0.0, 1.7, 0.0), time);
    let radiance = max(torch_color_strength.rgb, vec3<f32>(0.0))
        * (max(torch_color_strength.w, 0.0) * falloff * flicker);
    // Reflect an elongated flame instead of a mathematical point, which
    // produced a round pinprick on stone and sharp curves on nearby leather.
    let flame_axis = frame_rotate(frame, vec3<f32>(0.0, 0.12, 0.0));
    let source_variance = 0.0144 / (dist2 + 0.0144);
    let flame_roughness = clamp(sqrt(roughness * roughness + source_variance), 0.045, 1.0);
    var diffuse = vec3<f32>(0.0);
    var specular = vec3<f32>(0.0);
    for (var sample = 0; sample < 3; sample += 1) {
        let weight = select(0.25, 0.5, sample == 1);
        let direction = terrain_safe_unit3(to_torch + flame_axis * f32(sample - 1), n);
        let no_l = clamp(dot(n, direction), 0.0, 1.0);
        let wrapped = clamp((dot(n, direction) + 0.35) / 1.35, 0.0, 1.0);
        let h = terrain_safe_unit3(to_eye + direction, n);
        let energy = (vec3<f32>(1.0) - terrain_fresnel(f0, dot(to_eye, h))) * (1.0 - metallic);
        diffuse += energy * (select(no_l, wrapped, foliage) * 0.318309886 * weight);
        specular += terrain_specular(n, to_eye, direction, flame_roughness, f0) * weight;
    }
    return TorchLightSample(
        radiance * diffuse,
        radiance * (vec3<f32>(1.0) - f0) * (0.025 * (1.0 - metallic)),
        radiance * specular,
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

// Octave clamping, not an exact integral of the complete shader. A noise cell
// keeps full contrast down to about three pixels across and reaches its
// statistical mean near one pixel. The old 5-to-2 pixel window erased the 5 cm
// lattice a few metres out, flattening nearby walls and floors. Fixed
// wavelengths never change with camera distance.
fn surface_detail_weight(footprint_in_cells: f32) -> f32 {
    return 1.0 - smoothstep(0.30, 0.90, max(footprint_in_cells, 0.0));
}

// Box-filtered periodic mortar. Its integral retains thin-joint coverage
// after the 5 cm lattice is unresolved, without widening the authored joint.
fn surface_periodic_band(coordinate: f32, period: f32, width: f32, footprint: f32) -> f32 {
    let span = max(footprint, 0.0001);
    let phase = fract(coordinate / period) * period;
    let lo = phase - span * 0.5;
    let hi = phase + span * 0.5;
    let a = floor(lo / period) * width + min(fract(lo / period) * period, width);
    let b = floor(hi / period) * width + min(fract(hi / period) * period, width);
    return clamp((b - a) / span, 0.0, 1.0);
}

// Leaf clump value at one lattice scale, mean near 1. `t` is in lattice cells.
fn leaf_clump_value(t: vec2<f32>, pine: f32, salt: f32) -> f32 {
    let tex = floor(t);
    let cell = floor(t / 3.0);
    let id = surface_hash(cell + vec2<f32>(41.0, -13.0 + salt));
    let centre = cell * 3.0 + vec2<f32>(1.5) + (vec2<f32>(
        surface_hash(cell + vec2<f32>(7.0, 3.0 + salt)),
        surface_hash(cell + vec2<f32>(-2.0, 29.0 + salt))) - 0.5) * 1.4;
    let d = length((tex + 0.5 - centre) * vec2<f32>(1.0, 1.25));
    let gap = step(1.75 - pine * 0.35, d) * step(0.22, id);
    let rim = step(0.0, tex.y + 0.5 - centre.y) * step(0.8, d) * (1.0 - gap);
    var v = 0.90 + surface_hash(tex + vec2<f32>(5.0, 17.0 + salt)) * 0.12 + (id - 0.5) * 0.16;
    v = mix(v, 0.46, gap);
    v = mix(v, v * 1.14, rim);
    return v / 0.93;
}

// Whether a leaf lattice layer is drawn at this texel. Hand-over between layers
// is an ordered grid, not a blend: each lattice cell switches whole at its
// world-anchored Bayer threshold (like the chunk arrival dither), so crowns never
// pass through a blurred mean. The window holds until a cell nears one screen
// pixel (2.2 -> 1.05 px per cell); `offset` decorrelates the layers' grids.
fn leaf_layer_on(t: vec2<f32>, footprint_in_cells: f32, offset: vec2<f32>) -> bool {
    let weight = 1.0 - smoothstep(0.45, 0.95, footprint_in_cells);
    return terrain_bayer4(floor(t) + offset) < weight;
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

// Distant-terrain detail maps (render/horizon_detail.rs). One RGBA16F layer per
// level: height (m), normal x, normal z, cavity. Level i has texels of
// params.w * 2^i metres, 512 wide, centred on params.xy; params.z is 1 when a
// published map is bound. Texels are world anchored, so nothing swims when
// the horizon recentres, and every lookup blends levels continuously.
@group(3) @binding(5) var horizon_detail_tex: texture_2d_array<f32>;
@group(3) @binding(6) var<uniform> horizon_detail_params: vec4<f32>;
@group(3) @binding(7) var terrain_backstop_depth: texture_depth_2d;
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
    let top_edge = horizon_detail_edge(xz_m, top);
    // Outside the cache, a clamped texture read has zero coverage anyway.
    if top_edge >= 1.0 { return out; }
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
    out.weight = 1.0 - top_edge;
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

// Must match `fine::HALF_M` in render/horizon/fine.rs. Inside this square,
// centred like the detail map, the backdrop is the fine rings, whose vertex
// normals come from 4-16 m knots; the map's 16-128 m normals would blur them.
const FINE_RING_HALF_M: f32 = 6144.0;

fn horizon_detail_normal_weight(xz_m: vec2<f32>) -> f32 {
    let d = abs(xz_m - horizon_detail_params.xy);
    return smoothstep(FINE_RING_HALF_M - 512.0, FINE_RING_HALF_M, max(d.x, d.y));
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
        // The minimum only falls, and smoothstep(-1, 1, x) is 0 from here on.
        if clearance <= -1.0 { return 0.0; }
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
// Per-texel finish written beside the albedo: x scales roughness, y scales
// reflectance. (1, 1, 0) leaves the material's authored finish untouched; z is
// reserved. Reset before each surface call; fs_main applies it before F0.
var<private> surface_finish: vec3<f32>;

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
    // Quiet mineral washes carry the bank; individual pebbles are subordinate.
    // This is pigment variation, not a second shoreline/sediment simulation.
    let wash = surface_noise_filtered(q.xz / 18.0 + q.y * vec2<f32>(0.017, 0.023), fp * 0.07);
    let mineral = mix(base * vec3<f32>(0.84, 0.86, 0.82), vec3<f32>(0.16, 0.145, 0.105),
        0.20 + wash * 0.25);
    let mean = mineral * 0.80;
    if shape_w <= 0.0 { return mean; }
    let c = shape_cells(t, GRAVEL_STONE_TEXELS, vec2<f32>(211.0, -67.0));
    let texel_w = surface_detail_weight(fp * PIXEL_RES);
    // Squashed ellipses: round pebbles read as dots, flat ones as chips.
    let squash = 0.75 + fract(c.id * 13.7) * 0.5;
    let off = vec2<f32>(c.to_seed.x * squash, c.to_seed.y / squash);
    let radius = GRAVEL_STONE_TEXELS * (0.36 + c.id * 0.24);
    let reach = length(off) / radius;
    let stone = step(reach, 1.0) * step(0.75, c.d2 - c.d1);
    let pick = floor(c.id * 4.0);
    var tone = vec3<f32>(1.0);
    if pick == 1.0 { tone = vec3<f32>(1.22, 1.06, 0.82); }
    if pick == 2.0 { tone = vec3<f32>(0.74, 0.78, 0.86); }
    if pick == 3.0 { tone = vec3<f32>(1.30, 1.28, 1.20); }
    let fleck = (surface_hash(floor(t) + vec2<f32>(3.0, 91.0)) - 0.5) * texel_w;
    // Keep mineral chips distinct without outlining every one in near-black
    // fines. Actual surface normals carry most of the directional lighting;
    // the painted value step is subordinate to the terrain's larger forms.
    let lit = dot(off / radius, vec2<f32>(-0.6, 0.8));
    let value = (0.82 + fract(c.id * 5.3) * 0.22) * (1.0 + lit * 0.10);
    let fines = mineral * (0.68 + fleck * 0.05);
    let crevice = step(reach, 1.55) * (1.0 - stone);
    let detailed = mix(fines * (1.0 - crevice * 0.10), mineral * mix(vec3<f32>(1.0), tone, 0.55) * (value + fleck * 0.04), stone);
    natural_tilt = natural_tilt_from(normal, -c.to_seed / max(radius, 1.0) * 0.16 * stone * shape_w);
    // Water-worn pebbles are smoother and glint; the fines between them are dry and matte.
    let pebble_finish = vec3<f32>(0.58 + fract(c.id * 3.1) * 0.14, 1.35, 0.0);
    surface_finish = mix(vec3<f32>(1.0, 1.0, 0.0), mix(vec3<f32>(1.20, 0.55, 0.0), pebble_finish, stone), shape_w * texel_w);
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
    // Pebbles sit a step paler than their soil; a fixed grey read as white
    // specks across every dirt bank and beach terrace.
    let pebble = step(0.95, c.id) * step(c.d1, 1.7);
    let pebble_tone = mix(earth * 1.45, vec3<f32>(0.42, 0.40, 0.37), 0.35);
    col = mix(col, pebble_tone * (0.92 + grain * 0.1), pebble);
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
    let texel_w = surface_detail_weight(fp * PIXEL_RES);
    let luma = dot(rock, vec3<f32>(0.2126, 0.7152, 0.0722));
    // Dark rock is basalt: it cools into columns, so joints replace strata.
    let columnar = 1.0 - smoothstep(0.04, 0.07, luma);
    let side = step(abs(normal.y), 0.5);
    let cell_texels = mix(ROCK_PLANE_TEXELS, 5.0, columnar);
    let c = shape_cells(t, cell_texels, vec2<f32>(-44.0, 19.0));
    var col = rock * (0.92 + c.id * 0.14);
    let tx = floor(t);
    // Bedding: horizontal layers 4-7 texels thick on vertical faces, each with
    // its own value, split by a broken one-texel seam.
    let layer_h = 5.0 + floor(surface_hash(vec2<f32>(floor(tx.x / 40.0), 5.0)) * 3.0) - 1.0;
    let layer = floor(tx.y / layer_h);
    let layer_id = surface_hash(vec2<f32>(layer, floor(tx.x / 40.0) + 3.0));
    let strata = side * (1.0 - columnar) * texel_w;
    col *= 1.0 + (layer_id - 0.5) * 0.16 * strata;
    let on_seam = step(tx.y - layer * layer_h, 0.5) * step(0.40, surface_hash(vec2<f32>(layer, floor(tx.x / 9.0) + 71.0)));
    col *= 1.0 - on_seam * 0.30 * strata;
    // Fractures: runs along plane borders, one texel wide.
    let run = surface_noise(t * 0.07 + vec2<f32>(c.id * 9.0, 3.0));
    let fracture = step(c.d2 - c.d1, 0.85) * step(0.46, run) * texel_w * (1.0 - columnar);
    col *= 1.0 - fracture * 0.42;
    // Basalt columns: vertical joints on walls, cell joints on tops.
    let col_w = 5.0 + floor(surface_hash(vec2<f32>(floor(tx.x / 30.0), 9.0)) * 3.0);
    let col_i = floor(tx.x / col_w);
    let wall_joint = step(tx.x - col_i * col_w, 0.5) * step(0.25, surface_hash(vec2<f32>(col_i, floor(tx.y / 22.0))));
    let top_joint = step(c.d2 - c.d1, 0.9);
    let joint = mix(top_joint, wall_joint, side) * columnar * texel_w;
    let col_value = 0.90 + surface_hash(vec2<f32>(col_i, 31.0)) * 0.22;
    col *= mix(1.0, mix(col_value * 1.1, c.id * 0.5 + 0.8, 1.0 - side), columnar * texel_w);
    col *= 1.0 - joint * 0.55;
    // Mineral flecks: pale inclusions and a few dark pits, one texel each.
    let h = surface_hash(tx + vec2<f32>(61.0, -5.0));
    col = mix(col, rock * 1.30 + vec3<f32>(0.014) * columnar, step(0.978, h) * texel_w);
    col += vec3<f32>(0.010) * columnar * step(0.60, c.id) * texel_w;
    col = mix(col, rock * 0.60, step(h, 0.012) * texel_w);
    let a = c.id * 6.2831853;
    natural_tilt = natural_tilt_from(normal, vec2<f32>(cos(a), sin(a)) * 0.10 * shape_w);
    // Fractures and seams are rough; mineral flecks are smooth and glint;
    // cooled basalt tops are glassier than cut or weathered faces.
    let pale = step(0.978, h);
    let pit = step(h, 0.012);
    let glassy = columnar * (1.0 - side);
    let rock_finish = vec3<f32>(
        (1.0 + fracture * 0.25 + on_seam * 0.12 * strata) * mix(1.0, 0.82, glassy) * mix(1.0, 0.55, pale) * mix(1.0, 1.3, pit),
        mix(1.0, 0.6, fracture) * mix(1.0, 1.8, pale) * mix(1.0, 1.3, glassy),
        0.0);
    surface_finish = mix(vec3<f32>(1.0, 1.0, 0.0), rock_finish, shape_w * texel_w);
    return mix(rock, col, shape_w);
}

// Grass top: short blade tufts on a quieter matted floor. Each tuft is 3-6
// texels across with a lean direction: texels on the lean side are lit tips,
// the far side is the root, and texels just behind it fall in its shade.
// `turf` is the field colour already carrying climate and broad patchiness.
// Blossoms are meshed above the soil by the support-owned ground-detail pass.
fn grass_top(turf: vec3<f32>, q: vec3<f32>, fp: f32, dry: f32) -> vec3<f32> {
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
    let edge_width = max(fp * PIXEL_RES, 0.65);
    let in_tuft = 1.0 - smoothstep(r - edge_width, r + edge_width, c.d1);
    // Blade columns inside a tuft: a run of texels across the lean is on.
    let blade = mix(0.68, step(0.32, surface_hash(vec2<f32>(floor(across + 0.5), c.id * 977.0))), texel_w);
    let rise = saturate1(along / r * 0.5 + 0.5);
    let quiet = (surface_hash(floor(t) + vec2<f32>(-13.0, 29.0)) - 0.5) * texel_w;
    var value = 0.87 + quiet * 0.018;
    // Variation is pigment/root occlusion, not a painted directional highlight.
    // Live direct and local lights illuminate the material-shaped normal below.
    value = mix(value, mix(0.89, 0.98, blade), in_tuft);
    // Separate connected roots and blades around the existing distant tone.
    // More independent texel noise would sharpen grain, not the tuft shapes.
    value = 0.90 + (value - 0.90) * 1.55;
    var col = turf * value;
    let dry_tip = dry * in_tuft * blade * smoothstep(0.45, 0.75, rise);
    col = mix(col, turf * vec3<f32>(1.20, 1.10, 0.62), dry_tip * 0.75);
    // Flowers are supported mesh geometry; never bright confetti on flat soil.
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
    // Soil texels: grain, 2x2 pebbles and clods, and the turf lip's shadow
    // on the first soil rows. Mean stays near the soil colour.
    let clod_cell = floor(vec2<f32>(col_i, row) / 2.0);
    let clod = surface_hash(clod_cell + vec2<f32>(13.0, floor(p.y) * 7.0));
    let pebble = step(0.91, clod);
    let dark_clod = step(clod, 0.07);
    var soil_v = 0.91 + surface_hash(vec2<f32>(col_i, row) + vec2<f32>(3.0, -71.0)) * 0.16;
    soil_v *= 1.0 - step(depth - 0.5, row) * step(row, depth + 1.5) * 0.22;
    var soil_tex = soil_col * soil_v;
    soil_tex = mix(soil_tex, mix(soil_col, vec3<f32>(dot(soil_col, vec3<f32>(0.33))), 0.55) * 1.28, pebble);
    soil_tex = mix(soil_tex, soil_col * vec3<f32>(0.66, 0.60, 0.56), dark_clod);
    var col = mix(soil_tex, turf * (0.90 + surface_hash(vec2<f32>(col_i, row)) * 0.12) * (1.0 - lip * 0.18), fringe);
    col = mix(col, soil_col * vec3<f32>(0.72, 0.66, 0.60), root * (1.0 - fringe));
    // Keep the fringe's mean coverage (about 11 of 20 rows) when texels blur.
    let mean = mix(soil_col, turf, clamp(0.55 / scale, 0.0, 1.0));
    if fringe > 0.5 { natural_tilt = vec3<f32>(0.0); }
    return mix(mean, col, texel_w);
}

// Keep the overhead pattern on gentle terrain. Blend in vertical projections
// on steep faces, where an XZ sample can smear up the whole cliff.
fn backdrop_projection_weights(normal: vec3<f32>) -> vec3<f32> {
    let axes = max(abs(normal) - vec3<f32>(0.45), vec3<f32>(0.0));
    let weights = axes * axes;
    return weights / dot(weights, vec3<f32>(1.0));
}

fn backdrop_projected_noise(p: vec3<f32>, top_uv: vec2<f32>, offset: vec2<f32>,
    fp: f32, projection: vec3<f32>) -> f32 {
    let detail = surface_detail_weight(fp);
    if detail <= 0.0 { return 0.5; }
    var value = 0.0;
    if projection.x > 0.0 {
        value += projection.x * surface_noise(p.zy + offset + vec2<f32>(17.0, 31.0));
    }
    if projection.y > 0.0 {
        value += projection.y * surface_noise(top_uv + offset);
    }
    if projection.z > 0.0 {
        value += projection.z * surface_noise(p.xy + offset + vec2<f32>(-23.0, 11.0));
    }
    return 0.5 + (value - 0.5) * detail;
}

// Height-blend of the backdrop's vertex coverage. Linear coverage across a
// coarse lattice cell drew sand, rock and snow as soft blobs with stepped
// outlines, like an upscaled map. Each present channel gets its own
// footprint-filtered relief, and the highest wins over a transition width set
// by the screen footprint, so borders stay crisp and irregular up close and
// fall back to the plain mean once the relief is below a pixel. A channel
// absent from the whole triangle never appears.
fn backdrop_coverage(w: vec4<f32>, p_m: vec3<f32>, fp_m: f32, w_fw: vec4<f32>,
    projection: vec3<f32>) -> vec4<f32> {
    let total = dot(w, vec4<f32>(1.0));
    if total <= 1e-5 { return w; }
    let present = step(vec4<f32>(1e-4), w);
    let detail = surface_detail_weight(fp_m / 29.0);
    if dot(present, vec4<f32>(1.0)) <= 1.0 || detail <= 0.0 { return w; }
    var h = vec4<f32>(0.0);
    for (var i = 0; i < 4; i += 1) {
        if present[i] > 0.0 {
            let o = vec2<f32>(37.1, -19.7) * f32(i + 1);
            h[i] = (backdrop_projected_noise(p_m / 7.0, p_m.xz / 7.0, o, fp_m / 7.0, projection) - 0.5) * 0.30
                + (backdrop_projected_noise(p_m / 29.0, p_m.xz / 29.0, -o, fp_m / 29.0, projection) - 0.5) * 0.45;
        }
    }
    let s = w / total + h;
    let peak = max(max(s.x * present.x, s.y * present.y), max(s.z * present.z, s.w * present.w));
    let d = clamp(length(w_fw) / total * 1.5, 0.06, 0.6);
    let b = max(s - vec4<f32>(peak - d), vec4<f32>(0.0)) * present;
    return mix(w, b / max(dot(b, vec4<f32>(1.0)), 1e-5) * total, detail);
}

// Interpolated coverage, not interpolated numeric material IDs. Missing coverage
// retains the authored base color. Use unscaled voxel coordinates: texture scale
// must not move physical climate boundaries, shorelines, or snow altitude.
// Climate is interpolated; retain the established material art.
var<private> interpolated_climate: vec2<f32>;
fn natural_surface_blended(
    base: vec3<f32>, p: vec3<f32>, normal: vec3<f32>,
    terrain_weights: vec4<f32>, footprint: f32, turf_exposed: bool, projection: vec3<f32>,
) -> vec3<f32> {
    let positive_weights = max(terrain_weights, vec4<f32>(0.0));
    let weights = positive_weights / max(dot(positive_weights, vec4<f32>(1.0)), 1.0);
    let coverage = dot(weights, vec4<f32>(1.0));
    if coverage <= 0.00001 { return base; }
    // Snapped to texel centres: every noise below is flat inside a texel.
    let q = (floor(p * cam.world_params.x * PIXEL_RES) + vec3<f32>(0.5)) / PIXEL_RES;
    let fp = max(footprint * cam.world_params.x, 0.0);
    let climate = interpolated_climate;
    let heat = climate.x;
    let wet = climate.y;
    natural_relief_strength = vec4<f32>(0.0);
    let arid = smoothstep(0.48, 0.70, heat) * (1.0 - smoothstep(0.26, 0.48, wet));
    let mottle = backdrop_projected_noise(q / 17.0, q.xz / 17.0 + q.y * vec2<f32>(0.017, 0.027),
        vec2<f32>(0.0), fp * 0.068, projection);
    let clumps = backdrop_projected_noise(q * 0.65, q.xz * 0.65 + q.y * vec2<f32>(0.21, 0.16),
        vec2<f32>(0.0), fp * 0.71, projection) - 0.5;
    // On the shell, the floor faces the axis. World +Y is only the floor at the bottom.
    var face = normal.y;
    if light.fog_color_density.w > 2.0 {
        let axis_y = light.fog_color_density.w / max(cam.world_params.x, 1e-6);
        let inward = vec3<f32>(-p.x, axis_y - p.y, 0.0);
        face = dot(normal, inward / max(length(inward), 0.001));
    }
    let up = smoothstep(-0.05, 0.65, face);
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
        // Reference meadow: related olive/sage masses, with color accents left
        // to living flowers. This is linear reflectance, not a display grade.
        let meadow = mix(mix(vec3<f32>(0.060, 0.105, 0.052), vec3<f32>(0.150, 0.167, 0.081), 1.0 - wet),
            vec3<f32>(0.130, 0.151, 0.084), (1.0 - forest) * mottle * 0.42);
        // Broad moisture and exposure differences should read across a valley,
        // with warm dry grass and darker green in sheltered, vigorous stands.
        let base_turf = mix(meadow, sand, arid * 0.18)
            * mix(vec3<f32>(1.12, 1.035, 0.86), vec3<f32>(0.88, 1.02, 0.96), vigor);
        let turf = mix(base_turf, vec3<f32>(0.30, 0.285, 0.075), arid * 0.26);
        let coastal_dune = (1.0 - smoothstep(12.2, 13.6 + mottle * 0.6, q.y)) * 0.24;
        let sparse = smoothstep(0.34, 0.88, arid) * 0.30;
        let top_coat = mix(turf, sand, max(coastal_dune, sparse));
        // Climate overlays below run on the field colour; the top/side texture
        // (blade tufts, or a soil bank under a turf fringe) is applied last.
        var turf_color = top_coat;
        let drift_noise = surface_noise_filtered(q.xz / 48.0 + vec2<f32>(19.3, -27.1), fp / 48.0) * 0.62
            + surface_noise_filtered(q.xz / 13.5 + vec2<f32>(-41.2, 13.7), fp / 13.5) * 0.38;
        // Physical snow blocks own snow cover; altitude cannot whiten warm grass.
        let scree_noise = surface_noise_filtered(q.xz / 26.0 + vec2<f32>(-14.7, 31.9), fp / 26.0) * 0.60
            + surface_noise_filtered(q.xz / 7.8 + vec2<f32>(23.4, -18.6), fp / 7.8) * 0.40;
        let exposure = 1.0 - smoothstep(0.46, 0.84, face + (scree_noise - 0.5) * 0.20);
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
            * smoothstep(0.10, 0.48, 1.0 - face + (scree_noise - 0.5) * 0.16);
        // Every 1-voxel terrace riser is a vertical face. Below the foothills the
        // generator places real rock where ground is steep, so the shader must not
        // also grey every riser: that drew tan bands across lowland slopes.
        let riser_rock = exposure * 0.36 * smoothstep(26.0, 64.0, q.y);
        let exposed = clamp(max(riser_rock, alpine_rock * 0.68), 0.0, 1.0);
        turf_color = mix(turf_color, mineral * (0.86 + mottle * 0.28), exposed);
        let heath = smoothstep(0.60, 0.82, mottle) * (1.0 - forest) * 0.22;
        turf_color = mix(turf_color, vec3<f32>(0.30, 0.43, 0.25) * 0.86, heath * up);
        turf_color = mix(turf_color, soil, smoothstep(0.20, 0.46, clumps) * 0.20 * up);
        let marsh = smoothstep(0.65, 0.88, wet) * (1.0 - smoothstep(18.0, 30.0, q.y)) * smoothstep(14.5, 16.5, q.y);
        turf_color = mix(turf_color, vec3<f32>(0.30, 0.43, 0.25) * 0.70, marsh * up * 0.34 * (1.0 - coastal_dune));
        let heather = cool * (1.0 - forest) * smoothstep(0.62, 0.82, mottle);
        turf_color = mix(turf_color, vec3<f32>(0.42, 0.32, 0.40), heather * up * 0.16);
        if cam.scene_seed.y >= 10u {
            // Biome floors. Weights mirror the classify_landscape thresholds in
            // landscape.rs on the same heat/wet fields, softened into ecotones.
            // Each biome owns a matted-floor reflectance and one or two patch
            // materials; tufts, scree and wear are still applied after.
            // Ramps straddle the worldgen thresholds (0.56 warm, 0.24 polar,
            // 0.49 / 0.68 / 0.42 / 0.58 / 0.78 / 0.55 moisture, 0.44 heath heat).
            let warm = smoothstep(0.53, 0.60, heat);
            let polar = 1.0 - smoothstep(0.21, 0.28, heat);
            let temperate = (1.0 - polar) * (1.0 - warm);
            let b_savanna = warm * (1.0 - smoothstep(0.46, 0.53, wet)) * (1.0 - coastal_dune);
            let b_rainforest = warm * smoothstep(0.65, 0.72, wet);
            let b_taiga = polar * smoothstep(0.39, 0.46, wet);
            let b_tundra = polar * (1.0 - smoothstep(0.39, 0.46, wet));
            let b_heath = temperate * (1.0 - smoothstep(0.42, 0.47, heat))
                * (1.0 - smoothstep(0.55, 0.61, wet)) * smoothstep(30.0, 36.0, q.y);
            let b_fen = smoothstep(0.75, 0.81, wet) * smoothstep(15.5, 17.5, q.y) * (1.0 - smoothstep(23.0, 26.0, q.y));
            let b_forest = temperate * smoothstep(0.52, 0.58, wet) * (1.0 - b_heath) * (1.0 - b_fen);
            let floor_patch = surface_noise_filtered(q.xz / 9.0 + vec2<f32>(-31.0, 57.0), fp / 9.0);
            let grove = surface_noise_filtered(q.xz / 41.0 + vec2<f32>(8.0, 61.0), fp / 41.0);
            let speck = pixel_blob(natural_face_uv(q, normal), PIXEL_RES, vec2<f32>(77.0, -23.0), 0.07, fp);
            // Savanna: straw gold, greener near groves, bare dust in patches.
            var savanna_col = mix(vec3<f32>(0.260, 0.205, 0.075), vec3<f32>(0.120, 0.150, 0.052), smoothstep(0.52, 0.72, grove) * 0.7);
            savanna_col = mix(savanna_col, vec3<f32>(0.310, 0.235, 0.130), smoothstep(0.62, 0.80, floor_patch) * 0.6);
            // Cool moss, warm leaf litter and small amber flecks separate the
            // wet floor from the canopy. Reuse the filtered world texel lattice.
            var rain_col = mix(vec3<f32>(0.025, 0.083, 0.060), vec3<f32>(0.078, 0.150, 0.047), smoothstep(0.30, 0.80, mottle));
            let litter = smoothstep(0.49, 0.69, floor_patch) * smoothstep(0.20, 0.46, clumps);
            rain_col = mix(rain_col, vec3<f32>(0.100, 0.061, 0.031), litter * 0.76);
            rain_col = mix(rain_col, vec3<f32>(0.170, 0.105, 0.038), litter * speck * 0.45);
            // Taiga: blue-green moss floor, rust needle litter under crowns, pale lichen specks.
            var taiga_col = mix(vec3<f32>(0.040, 0.072, 0.050), vec3<f32>(0.170, 0.090, 0.040), smoothstep(0.50, 0.68, floor_patch) * 0.85);
            taiga_col = mix(taiga_col, vec3<f32>(0.26, 0.28, 0.24), speck * 0.6);
            // Tundra: grey-ochre lichen mat, rust moss cushions, grey lichen specks.
            var tundra_col = mix(vec3<f32>(0.165, 0.160, 0.100), vec3<f32>(0.210, 0.120, 0.050), smoothstep(0.55, 0.75, floor_patch) * 0.75);
            tundra_col = mix(tundra_col, vec3<f32>(0.28, 0.29, 0.26), speck * 0.8);
            // Heath: straw grass with heather in broad purple-brown runs and dark wet hollows.
            var heath_col = mix(vec3<f32>(0.190, 0.165, 0.075), vec3<f32>(0.190, 0.090, 0.150), smoothstep(0.48, 0.66, mottle) * 0.85);
            heath_col = mix(heath_col, vec3<f32>(0.070, 0.080, 0.040), smoothstep(0.20, 0.42, -clumps) * 0.5);
            // Fen: deep wet green over peat hollows, rush straw specks.
            var fen_col = mix(vec3<f32>(0.035, 0.090, 0.045), vec3<f32>(0.065, 0.045, 0.028), smoothstep(0.15, 0.40, -clumps) * 0.7);
            fen_col = mix(fen_col, vec3<f32>(0.200, 0.180, 0.080), speck * 0.5);
            // Temperate forest: a cooler, shaded floor than the open meadow.
            let forest_col = vec3<f32>(0.060, 0.100, 0.048);
            var floor_col = turf_color;
            floor_col = mix(floor_col, forest_col, b_forest * 0.55);
            floor_col = mix(floor_col, heath_col, b_heath);
            floor_col = mix(floor_col, fen_col, b_fen);
            floor_col = mix(floor_col, savanna_col, b_savanna);
            floor_col = mix(floor_col, rain_col, b_rainforest);
            floor_col = mix(floor_col, taiga_col, b_taiga);
            floor_col = mix(floor_col, tundra_col, b_tundra);
            turf_color = floor_col;
        }
        // Broad wear patches leave quiet intervals between detailed plants.
        let worn = smoothstep(
            0.68,
            0.90,
            surface_noise_filtered(q.xz / 4.5 + vec2<f32>(23.0, -11.0), fp / 4.5),
        ) * up;
        turf_color = mix(turf_color, soil, worn * 0.16);
        // Broad dry patches (several blocks) choose where tuft tips straw.
        let dry_field = smoothstep(0.58, 0.78,
            surface_noise_filtered(q.xz / 6.5 + vec2<f32>(17.0, 5.0), fp / 6.5));
        // Deepen the meadow's authored olive at full daylight. Snow and dry
        // coastal sand keep their own values.
        turf_color *= vec3<f32>(0.89, 0.90, 0.88);
        if face > 0.5 && turf_exposed {
            turf_color = grass_top(turf_color, q, fp, dry_field * (0.35 + arid * 0.65));
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
        surface_finish = mix(vec3<f32>(1.0, 1.0, 0.0), surface_finish, weights.z);
        let rock_face = natural_face_uv(p * cam.world_params.x, normal);
        // Fine connected colonies retain the existing broad habitat mask.
        let lichen_field = surface_noise_filtered(q.xz / 2.7 + vec2<f32>(44.0, -9.0), fp / 2.7);
        let lichen_zone = smoothstep(0.62, 0.70, lichen_field)
            * smoothstep(-0.35, 0.45, normal.y + 0.25 * (heat - 0.5));
        let colony = lichen_colony(rock_face, fp);
        let lichen = colony.x * lichen_zone;
        let rock_luma = dot(rock, vec3<f32>(0.2126, 0.7152, 0.0722));
        let lichen_tone = mix(rock * 1.25, vec3<f32>(0.34, 0.39, 0.25), smoothstep(0.08, 0.30, rock_luma))
            * vec3<f32>(0.96, 1.04, 0.88) * (0.92 + bedding * 0.12);
        rock = mix(rock, lichen_tone * colony.y, lichen * 0.65);
        color += rock * weights.z;
    }
    if weights.w > 0.0 {
        let legacy_moisture = 1.0 - smoothstep(11.8, 12.95 + mottle * 0.35, q.y);
        let t = light.torch_meta.y;
        // Generator 21 beaches carry a live swash band: a run-up line breathes
        // over the intertidal sand instead of a fixed wet/dry threshold.
        var shore_moisture = legacy_moisture;
        if cam.scene_seed.y >= 22u && q.y > 11.9 && q.y < 13.45 {
            let breaker = water_breaker(q.xz, 0.0, t);
            let shore_phase = breaker.x;
            let run_up = smoothstep(0.44, 0.61, shore_phase)
                * (1.0 - smoothstep(0.65, 0.99, shore_phase));
            let swash_line = 12.12 + 0.62 * run_up * breaker.y + (mottle - 0.5) * 0.22;
            let swash = 1.0 - smoothstep(swash_line - 0.08, swash_line + 0.58, q.y);
            shore_moisture = max(legacy_moisture * 0.72, swash);
        }
        let cusp = 0.5 + 0.5 * sin(dot(q.xz, vec2<f32>(0.83, -0.41)) * 0.42 + mottle * 3.0);
        let coastal = 1.0 - smoothstep(2.0, 8.0, abs(q.y - 12.0));
        // Oyster cream on dry bars, honey where the broad sediment patches show
        // through. Existing sky fill supplies the cooler shaded side.
        let beach_sand = mix(vec3<f32>(0.78, 0.65, 0.45), vec3<f32>(0.66, 0.47, 0.26),
            smoothstep(0.32, 0.68, mottle));
        let dry_sand = mix(mix(base, sand, 0.48), beach_sand, coastal * 0.80)
            * (0.98 + mottle * 0.08 + (cusp - 0.5) * 0.035);
        let wet_sand = dry_sand * mix(vec3<f32>(0.68, 0.66, 0.61), vec3<f32>(0.60, 0.53, 0.45), coastal);
        var sand_color = mix(dry_sand, wet_sand, shore_moisture * 0.78);
        // Bound the warped ripple phase before evaluating the expensive detail.
        let ripple_weight = surface_detail_weight(fp * 8.0);
        if ripple_weight > 0.0 {
            let ripple_coord = q.xz * vec2<f32>(0.85, 0.52)
                + vec2<f32>(surface_noise_filtered(q.xz * 0.18, fp * 0.18) * 2.4, mottle * 3.2);
            let phase = (ripple_coord.x + ripple_coord.y) * 18.0;
            sand_color *= 1.0 + sin(phase) * 0.028 * ripple_weight * (1.0 - shore_moisture * 0.72);
        }
        // One-texel grains: value spread, warm grains and rare dark minerals,
        // each with a small tilt so it catches the light on its own.
        let grain_weight = surface_detail_weight(fp * PIXEL_RES);
        if grain_weight > 0.0 {
            let texel = floor(natural_face_uv(q, normal) * PIXEL_RES);
            sand_color *= mix(vec3<f32>(1.0), sand_grain_tint(texel), grain_weight);
            let lean = vec2<f32>(surface_hash(texel + vec2<f32>(3.0, 61.0)),
                surface_hash(texel + vec2<f32>(47.0, -9.0))) - vec2<f32>(0.5);
            natural_tilt += natural_tilt_from(normal, lean * 0.07 * grain_weight) * weights.w;
        }
        color += sand_color * weights.w;
    }
    // Broad value swings read as cloud shadow; 17 m mottle stays mostly in hue.
    let modulation = 0.97 + mottle * 0.06 + clumps * 0.06;
    let mixed = max(base * (1.0 - coverage) + color * modulation, vec3<f32>(0.0));
    // Preserve the authored reflectance; exposure and display grading have
    // their own owners. A second saturation boost made quiet greens acidic.
    return max(mixed, vec3<f32>(0.002));
}

// Cobblestone cells in metres: `edge` is twice the distance to the nearest
// stone border in cell units, `to_seed` the offset from the stone centre.
const COBBLE_CELL_M: f32 = 0.30;
const COBBLE_GAP: f32 = 0.13;

struct CobbleCell {
    edge: f32,
    id: f32,
    to_seed: vec2<f32>,
}

fn cobble_cells(metres: vec2<f32>) -> CobbleCell {
    let q = metres / COBBLE_CELL_M;
    let g = floor(q);
    var d1 = 1e8; var d2 = 1e8;
    var out = CobbleCell(0.0, 0.0, vec2<f32>(0.0));
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let c = g + vec2<f32>(f32(i), f32(j));
            let site = c + vec2<f32>(surface_hash(c + vec2<f32>(5.0, 71.0)), surface_hash(c + vec2<f32>(83.0, 19.0))) * 0.7 + 0.15;
            let off = q - site;
            let d = dot(off, off);
            if d < d1 {
                d2 = d1; d1 = d;
                out.id = surface_hash(c + vec2<f32>(-37.0, 29.0));
                out.to_seed = off;
            } else if d < d2 { d2 = d; }
        }
    }
    out.edge = sqrt(d2) - sqrt(d1);
    return out;
}

// Lighting-normal lean of one cobble: each stone domes away from its centre.
fn cobble_tilt(uv: vec2<f32>, normal: vec3<f32>, fp_m: f32) -> vec3<f32> {
    let cell = cobble_cells(texel_centre(uv));
    let dome = smoothstep(COBBLE_GAP, 0.95, cell.edge);
    return natural_tilt_from(normal, -cell.to_seed * (1.0 - dome) * 0.45)
        * surface_detail_weight(fp_m * PIXEL_RES);
}

fn natural_surface(base: vec3<f32>, p: vec3<f32>, normal: vec3<f32>, kind: i32, footprint: f32) -> vec3<f32> {
    if kind == 33 || kind == 38 {
        let q = (floor(p * cam.world_params.x * PIXEL_RES) + vec3<f32>(0.5)) / PIXEL_RES;
        let fp = max(footprint * cam.world_params.x, 0.0);
        if kind == 38 { return max(ash_surface(base, q, normal, fp), vec3<f32>(0.002)); }
        return max(gravel_surface(base, q, normal, fp), vec3<f32>(0.002));
    }
    if kind == 16 {
        // Snapped to texel centres: every noise below is flat inside a texel.
    let q = (floor(p * cam.world_params.x * PIXEL_RES) + vec3<f32>(0.5)) / PIXEL_RES;
        let mottle = surface_noise_filtered(q.xz / 17.0 + q.y * vec2<f32>(0.017, 0.027), footprint * cam.world_params.x * 0.068);
        let fungal = mix(base, vec3<f32>(0.25, 0.18, 0.33), smoothstep(0.42, 0.72, mottle) * 0.4)
            * (0.90 + mottle * 0.20);
        // Grove moss shares this family with fungal soil. The purple mottle
        // turned green moss into a flat grey slab, so a green-dominant base
        // takes cushions instead: half-metre clumps, sunlit yellow-green
        // tips and one-texel grain, all fading to their mean with distance.
        let mossy = smoothstep(0.0, 0.025, base.g - max(base.r, base.b));
        if mossy <= 0.0 {
            // Mycelium: dark crumbly soil laced with pale hyphae. A main
            // network of one-texel threads, a finer branching net in patches,
            // dark crumbs and, on tops, a scatter of pale spore specks. Near
            // the camera only; far away the threads settle into the mean tone.
            let fp = max(footprint * cam.world_params.x, 0.0);
            let texel_w = surface_detail_weight(fp * PIXEL_RES);
            let pale = vec3<f32>(0.47, 0.44, 0.55);
            let far = mix(fungal, pale, 0.05);
            if texel_w <= 0.0 { return max(far, vec3<f32>(0.002)); }
            var fuv = q.xz;
            if abs(normal.x) > 0.5 { fuv = q.zy; } else if abs(normal.z) > 0.5 { fuv = q.xy; }
            let cell = floor(fuv * PIXEL_RES);
            let crumb = surface_hash(cell + vec2<f32>(7.0, 29.0));
            let clump = surface_noise(fuv * 3.1 + vec2<f32>(-19.0, 44.0));
            var soil = fungal * (0.86 + crumb * 0.18) * (0.90 + clump * 0.16);
            soil *= select(1.0, 0.68, crumb < 0.07);
            let main_thread = texel_thread(fuv, 1.7, vec2<f32>(13.0, -41.0), 0.42);
            let fine_patch = smoothstep(0.48, 0.62, surface_noise(fuv * 1.2 + vec2<f32>(3.0, 8.0)));
            let fine_thread = texel_thread(fuv, 4.6, vec2<f32>(-29.0, 7.0), 0.38) * fine_patch;
            let spore = step(0.978, surface_hash(cell + vec2<f32>(61.0, -3.0)))
                * select(0.35, 0.85, normal.y > 0.5);
            // Hyphae fade in and out along their length and miss a texel here
            // and there, so the net reads as growth, not as a drawn maze.
            let reach = smoothstep(0.30, 0.62, surface_noise(fuv * 2.6 + vec2<f32>(-37.0, 15.0)));
            let gap = select(1.0, 0.35, surface_hash(cell + vec2<f32>(-43.0, 17.0)) < 0.22);
            let hypha = max(max(main_thread * 0.42 * reach * gap, fine_thread * 0.24 * gap), spore * 0.8);
            soil = mix(soil, pale * (0.88 + crumb * 0.22), hypha);
            return max(mix(far, soil, texel_w), vec3<f32>(0.002));
        }
        let fp = max(footprint * cam.world_params.x, 0.0);
        var uv = q.xz;
        if abs(normal.x) > 0.5 { uv = q.zy; } else if abs(normal.z) > 0.5 { uv = q.xy; }
        let cushion = smoothstep(0.34, 0.70, surface_noise_filtered(uv * 1.9 + vec2<f32>(41.0, -13.0), fp * 1.9));
        let tips = smoothstep(0.60, 0.86, surface_noise_filtered(uv * 4.3 + vec2<f32>(-7.0, 57.0), fp * 4.3));
        let grain = (surface_hash(floor(uv * PIXEL_RES) + vec2<f32>(19.0, 83.0)) - 0.5)
            * surface_detail_weight(fp * PIXEL_RES);
        var moss = base * mix(0.74, 1.16, cushion) * (1.0 + grain * 0.16);
        moss = mix(moss, base * vec3<f32>(1.34, 1.30, 0.72), tips * cushion * 0.38);
        moss *= 0.93 + mottle * 0.14;
        return max(mix(fungal, moss, mossy), vec3<f32>(0.002));
    }
    let weights = vec4<f32>(
        select(0.0, 1.0, kind == 0 || kind == 1 || kind == 7 || kind == 36),
        select(0.0, 1.0, kind == 2), select(0.0, 1.0, kind == 3), select(0.0, 1.0, kind == 4));
    return natural_surface_blended(base, p, normal, weights, footprint, kind != 36, vec3<f32>(0.0, 1.0, 0.0));
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

// Mana hedron interior, seen through its faces. `entry` is the texel-centred
// entry point in the hedron's frame (x/z from the spine, y in world metres) and
// `ray` the refracted view direction. Six taps, no texture reads: three
// cleavage sheet families split per channel (dispersion fringes), a lit spine
// and Beer-Lambert falloff, so the depth parallaxes correctly from any angle.
// `resolved` fades the sheets to their mean once a texel is sub-pixel.
fn hedron_interior(entry: vec3<f32>, ray: vec3<f32>, tint: vec3<f32>, t: f32, resolved: f32) -> vec3<f32> {
    let k0 = vec3<f32>(0.78, 0.45, 0.43) * 1.7;
    let k1 = vec3<f32>(-0.36, 0.62, -0.70) * 1.35;
    let k2 = vec3<f32>(0.20, -0.30, 0.93) * 1.15;
    let fringe = vec3<f32>(-0.016, 0.0, 0.016);
    let pale = mix(tint, vec3<f32>(1.0), 0.55);
    var sum = vec3<f32>(0.0);
    var carry = 1.0;
    for (var i = 0; i < 6; i++) {
        let d = (f32(i) + 0.55) * 0.24;
        var q = entry + ray * d;
        // A slow warp: the inner lattice swims a few centimetres, never the faces.
        q += 0.07 * sin(q.yzx * 2.1 + vec3<f32>(t * 0.71, t * 0.53, t * 0.61));
        let spine = exp(-dot(q.xz, q.xz) * 1.6);
        let s0 = abs(fract(vec3<f32>(dot(q, k0)) + fringe) - 0.5);
        let s1 = abs(fract(vec3<f32>(dot(q, k1) + 0.37) + fringe) - 0.5);
        let s2 = abs(fract(vec3<f32>(dot(q, k2) + 0.71) + fringe) - 0.5);
        let sheet = max(max(1.0 - smoothstep(vec3<f32>(0.0), vec3<f32>(0.04), s0),
            1.0 - smoothstep(vec3<f32>(0.0), vec3<f32>(0.03), s1)),
            1.0 - smoothstep(vec3<f32>(0.0), vec3<f32>(0.03), s2));
        let sheets = mix(vec3<f32>(0.12), sheet, resolved);
        sum += carry * (tint * (0.10 + spine * 0.85) + pale * sheets * 0.40);
        carry *= 0.78;
    }
    return sum * 0.30;
}

// Shared by colour and BRDF: only exposed mineral catches light, never its host rock.
fn ore_inclusion(uv_m: vec2<f32>, footprint_m: f32) -> f32 {
    // Preserve average mineral coverage when individual pixels cannot resolve.
    let resolved = surface_detail_weight(footprint_m * PIXEL_RES);
    if resolved <= 0.0 { return ORE_MEAN_COVER; }
    let inside = select(0.0, 1.0, ore_nugget_at(floor(uv_m * PIXEL_RES)));
    return mix(ORE_MEAN_COVER, inside, resolved);
}

// Ore nuggets on the 5 cm lattice, scattered like cellular noise. Each 6x6
// texel cell may hold one rounded, slightly elongated cluster of about 4-14
// texels whose centre lies anywhere in the cell, so clusters cross cell edges
// and never line up in rows or columns. A cluster reaches under 3 texels (half
// a cell) from its centre, so the 2x2 cells around a texel are all that can
// cover it. A broad deposit field sets how many cells carry one: rich seams
// and barren patches, never stripes.
const ORE_CELL: f32 = 6.0;
const ORE_MEAN_COVER: f32 = 0.20;

fn ore_nugget_in(cell: vec2<f32>, p: vec2<f32>, jitter: f32) -> bool {
    let deposit = surface_noise(cell * (ORE_CELL / PIXEL_RES) * 1.4 + vec2<f32>(11.0, -5.0));
    let chance = mix(0.45, 1.0, smoothstep(0.28, 0.66, deposit));
    if surface_hash(cell + vec2<f32>(71.0, 29.0)) > chance { return false; }
    let radius = 1.5 + surface_hash(cell + vec2<f32>(-17.0, 5.0)) * 1.0;
    let centre = (cell + vec2<f32>(surface_hash(cell + vec2<f32>(3.0, 91.0)),
        surface_hash(cell + vec2<f32>(47.0, 13.0)))) * ORE_CELL;
    let angle = surface_hash(cell + vec2<f32>(29.0, -61.0)) * 3.14159;
    let axis = vec2<f32>(cos(angle), sin(angle));
    let d = p - centre;
    let squeezed = vec2<f32>(dot(d, axis), dot(d, vec2<f32>(-axis.y, axis.x)) * 1.35);
    return length(squeezed) + jitter < radius;
}

// The cell whose cluster covers texel `t` in xy, and 1 in z when one does.
fn ore_nugget_cell(t: vec2<f32>) -> vec3<f32> {
    let p = t + vec2<f32>(0.5);
    let base = floor(p / ORE_CELL - vec2<f32>(0.5));
    let jitter = (surface_hash(t + vec2<f32>(23.0, 61.0)) - 0.5) * 0.7;
    for (var i = 0u; i < 4u; i++) {
        let cell = base + vec2<f32>(f32(i & 1u), f32(i >> 1u));
        if ore_nugget_in(cell, p, jitter) { return vec3<f32>(cell, 1.0); }
    }
    return vec3<f32>(0.0);
}

fn ore_nugget_at(t: vec2<f32>) -> bool { return ore_nugget_cell(t).z > 0.5; }

// 1 on texels that the 0.5 contour of a noise field crosses: a wandering line
// about one texel wide, for hyphae and veins. Sampled at texel centres.
fn texel_thread(uv_m: vec2<f32>, freq: f32, offset: vec2<f32>, half_width: f32) -> f32 {
    let g = surface_noise_grad(texel_centre(uv_m) * freq + offset);
    let slope = max(length(g.yz) * freq, 0.001);
    return 1.0 - step(half_width / PIXEL_RES, abs(g.x - 0.5) / slope);
}

// Material ids (BlockId + 1) of the industrial blocks; the content ABI assertion pins them.
const BLOCK_CONCRETE: u32 = 192u;
// Surface keys use BlockId + 1, shared by resident and proxy foliage.
const BLOCK_PINE_LEAVES: u32 = 28u;
const BLOCK_STEEL_PLATE: u32 = 193u;
const BLOCK_RUSTED_STEEL: u32 = 194u;
const BLOCK_HAZARD_PAINT: u32 = 195u;
const BLOCK_WINDOW_GLASS: u32 = 196u;

fn is_industrial_block(block: u32) -> bool {
    return block >= BLOCK_CONCRETE && block <= BLOCK_WINDOW_GLASS;
}

fn is_factory_panel(kind: i32) -> bool {
    return kind >= 26 && kind <= 32;
}

// One-texel seam lines at a metre period, faded once they fall below a pixel.
fn industrial_seam(uv: vec2<f32>, period: vec2<f32>, width: f32) -> f32 {
    let local = fract(uv / period) * period;
    let edge = min(min(local.x, period.x - local.x), min(local.y, period.y - local.y));
    return 1.0 - step(width, edge);
}

// Abandoned structural finishes, authored in metres: board-formed concrete,
// riveted plate, corroded sheet, worn hazard striping and gridded glazing.
// Vertical faces get rain and rust run-off; up faces gather grime and moss.
fn industrial_surface(base: vec3<f32>, uv: vec2<f32>, normal: vec3<f32>, block: u32, fp: f32, land: vec2<f32>) -> vec3<f32> {
    let fine = surface_detail_weight(fp * PIXEL_RES);
    let medium = surface_detail_weight(fp * 4.0);
    let vertical = abs(normal.y) < 0.5;
    let up = select(0.0, 1.0, normal.y > 0.5);
    let grime = surface_noise_filtered(land / 9.0 + vec2<f32>(13.0, -41.0), fp / 9.0);
    let texel = (surface_hash(floor(uv * PIXEL_RES)) - 0.5) * fine;
    // Long vertical streaks: low frequency along the face, high across it.
    let streak = select(0.0, smoothstep(0.55, 0.85, surface_noise_filtered(uv * vec2<f32>(3.1, 0.18), fp * 3.1)), vertical);
    let moss_patch = smoothstep(0.52, 0.74, surface_noise_filtered(uv * 0.7 + vec2<f32>(-7.0, 29.0), fp * 0.7)) * up;
    let moss = vec3<f32>(0.20, 0.27, 0.12) * (0.85 + texel * 0.6);
    var color = base;
    if block == BLOCK_CONCRETE {
        // Formwork lifts 2.4 m x 1.2 m; each pour has its own tone.
        let panel = floor(uv / vec2<f32>(2.4, 1.2));
        let pour = (surface_hash(panel + vec2<f32>(7.0, 3.0)) - 0.5) * 0.10;
        color = base * (1.0 + pour * medium + texel * 0.10 + (grime - 0.5) * 0.16);
        let seam = industrial_seam(uv, select(vec2<f32>(3.0, 3.0), vec2<f32>(2.4, 1.2), vertical), 0.05) * medium;
        color *= 1.0 - seam * 0.30;
        // Tie holes on a 0.6 m grid inside each lift.
        let tie = fract(uv / vec2<f32>(0.6, 0.6)) - vec2<f32>(0.5);
        let hole = (1.0 - step(0.045, length(tie))) * select(0.0, 1.0, vertical) * fine;
        color *= 1.0 - hole * 0.45;
        color = mix(color, color * vec3<f32>(0.62, 0.60, 0.56), streak * 0.55);
        let spall = smoothstep(0.80, 0.90, surface_noise_filtered(uv * 1.6 + vec2<f32>(51.0, 9.0), fp * 1.6)) * medium;
        color = mix(color, base * vec3<f32>(0.74, 0.70, 0.64), spall * 0.6);
    } else if block == BLOCK_STEEL_PLATE {
        let seam = industrial_seam(uv, vec2<f32>(1.5, 0.75), 0.05) * medium;
        // Rivet rows 7.5 cm inside each seam, 15 cm apart.
        let cell = fract(uv / vec2<f32>(1.5, 0.75)) * vec2<f32>(1.5, 0.75);
        let rivet_y = min(abs(cell.y - 0.075), abs(cell.y - 0.675));
        let rivet_x = abs(fract(uv.x / 0.15) - 0.5) * 0.15;
        let rivet = (1.0 - step(0.03, max(rivet_y, rivet_x))) * fine;
        let paint = (surface_hash(floor(uv / vec2<f32>(1.5, 0.75))) - 0.5) * 0.08;
        color = furnishing_iron(base, uv, fp) * (1.0 + paint * medium + (grime - 0.5) * 0.12);
        color = mix(color, vec3<f32>(0.04, 0.045, 0.05), seam * 0.75);
        color = mix(color, base * 1.45, rivet * 0.6);
        surface_finish = vec3<f32>(1.0 + seam * 0.25 - rivet * 0.35, 1.0, 0.0);
        // Chipped paint exposes rust, which bleeds downward on walls.
        let chip = smoothstep(0.70, 0.82, surface_noise_filtered(uv * 2.3 + vec2<f32>(5.0, 17.0), fp * 2.3));
        let rust = vec3<f32>(0.36, 0.17, 0.08) * (0.9 + texel * 0.8);
        color = mix(color, rust, clamp(chip * 0.85 + streak * 0.45, 0.0, 1.0) * medium);
    } else if block == BLOCK_RUSTED_STEEL {
        // Corrugated sheet: 15 cm ribs read through shade bands.
        let rib = select(0.0, sin(uv.x * 6.2831853 / 0.15), vertical) * fine;
        let scale_rust = surface_noise_filtered(uv * 3.7 + vec2<f32>(-3.0, 8.0), fp * 3.7);
        color = mix(base * vec3<f32>(0.75, 0.70, 0.66), base * vec3<f32>(1.22, 1.10, 0.92), scale_rust);
        color *= 1.0 + rib * 0.09 + texel * 0.16;
        let pit = smoothstep(0.78, 0.88, surface_noise_filtered(uv * 9.0, fp * 9.0)) * fine;
        color = mix(color, vec3<f32>(0.09, 0.05, 0.03), pit * 0.7);
        color = mix(color, color * vec3<f32>(0.66, 0.55, 0.48), streak * 0.5);
    } else if block == BLOCK_HAZARD_PAINT {
        // 40 cm diagonal bands, scuffed through to dark primer.
        let band = step(0.5, fract((uv.x + uv.y) / 0.4));
        let yellow = base * (1.0 + texel * 0.12);
        let black = vec3<f32>(0.035, 0.035, 0.03);
        color = mix(mix(yellow, black, band), base * 0.55, 1.0 - medium);
        let wear = smoothstep(0.62, 0.80, surface_noise_filtered(uv * 1.9 + vec2<f32>(33.0, 2.0), fp * 1.9));
        color = mix(color, vec3<f32>(0.20, 0.17, 0.13) * (0.9 + texel), wear * 0.8 * medium);
        color = mix(color, color * vec3<f32>(0.6, 0.52, 0.44), streak * 0.4);
    } else if block == BLOCK_WINDOW_GLASS {
        // Curtain-wall grid, 1.2 m x 1.5 m, with steel mullions. A few panes are
        // gone (black) and some are fogged with grime from the sill upward.
        let pane = floor(uv / vec2<f32>(1.2, 1.5));
        let mullion = industrial_seam(uv, vec2<f32>(1.2, 1.5), 0.07) * medium;
        let roll = surface_hash(pane + vec2<f32>(91.0, 17.0));
        let sill = fract(uv.y / 1.5);
        let fog = (1.0 - smoothstep(0.0, 0.45, sill)) * select(0.0, 0.5, roll > 0.55);
        color = base * (0.85 + roll * 0.3 + texel * 0.06);
        color = mix(color, vec3<f32>(0.26, 0.27, 0.24), fog * medium);
        color = select(color, vec3<f32>(0.012, 0.014, 0.016), roll < 0.12 && medium > 0.5);
        color = mix(color, vec3<f32>(0.10, 0.11, 0.12), mullion);
        return max(color, vec3<f32>(0.002));
    }
    color = mix(color, moss, moss_patch * select(0.85, 0.55, block == BLOCK_HAZARD_PAINT));
    return max(color, vec3<f32>(0.002));
}

fn block_surface(base: vec3<f32>, p: vec3<f32>, normal: vec3<f32>, kind: i32, block: u32, footprint: f32) -> vec3<f32> {
    if kind == 12 && is_gilded_block(block) {
        return gilded_surface(base, p * cam.world_params.x, normal, block, footprint * cam.world_params.x);
    }
    if kind == 12 && is_industrial_block(block) {
        var face = p.xz;
        if abs(normal.x) > 0.5 { face = p.zy; }
        else if abs(normal.z) > 0.5 { face = p.xy; }
        return industrial_surface(base, texel_centre(face * cam.world_params.x), normal, block,
            footprint * cam.world_params.x, p.xz * cam.world_params.x);
    }
    if kind==0 || kind==1 || kind==2 || kind==3 || kind==4 || kind==7 || kind==16 || kind==33 || kind==38 { return natural_surface(base,p,normal,kind,footprint); }
    if kind == 8 || kind == 61 { return base; }
    let landscape_p = p.xz * cam.world_params.x;
    var uv = p.xz;
    if abs(normal.x) > 0.5 { uv = p.zy; }
    else if abs(normal.z) > 0.5 { uv = p.xy; }
    let metric_footprint=footprint*cam.world_params.x;
    let fine_fade = surface_detail_weight(metric_footprint * 8.0);
    let medium_fade = surface_detail_weight(metric_footprint * 4.0);
    let continuous_uv = uv * cam.world_params.x;
    let physical_uv = texel_centre(continuous_uv);
    if kind == 41 { return furnishing_iron(base, continuous_uv, metric_footprint); }
if kind >= 64 && kind <= 68 {
    return architecture_surface(base, p * cam.world_params.x, normal, kind, metric_footprint);
}
    if kind == 45 {
        return bookcase_surface(base, continuous_uv, normal, metric_footprint);
    }
    if kind == 23 { return chest_surface(base, continuous_uv, normal, metric_footprint); }
    // Meter-scale color survives when individual blades and voxel faces no
    // longer resolve. The same continuous field spans every mesh/LOD boundary.
    let broad = surface_noise_filtered(landscape_p / 83.0, metric_footprint / 83.0);
    let landscape_patch = surface_noise_filtered(landscape_p / 17.0 + vec2<f32>(31.0, -12.0), metric_footprint / 17.0);
    let ridge = surface_noise_filtered(landscape_p / 5.2 + vec2<f32>(-73.0, 22.0), metric_footprint / 5.2);
    let land_tone = broad * 0.50 + landscape_patch * 0.32 + ridge * 0.18;
    // Default detail stays pixel-aligned; masonry and boards use continuous_uv
    // for narrow joints, independently of their snapped grain coordinates.
    uv = physical_uv;
    // Default 20-cell/metre grain: one pixel per 5 cm. Masonry and boards
    // override this with finer 32-cell grain below.
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
        // Bark is drawn as plates on the pixel grid: columns 3-4 texels wide,
        // plates 10-17 texels tall staggered per column, a one-texel dark
        // furrow between columns and plates, and a one-texel lit edge on the
        // left of each plate (light from the upper left, as every sprite).
        // Cut ends show rings and a pale sapwood band. Logs also build
        // houses, so the wood keeps part of its warm base colour.
        let luma = dot(base, vec3<f32>(0.2126, 0.7152, 0.0722));
        let olive = vec3<f32>(luma) * vec3<f32>(1.30, 1.28, 1.00);
        let moss_tone = vec3<f32>(luma) * vec3<f32>(1.20, 1.46, 0.62);
        let moss_patch = surface_noise_filtered(physical_uv * 1.9 + vec2<f32>(5.0, -23.0), metric_footprint * 1.9);
        let texel_w = surface_detail_weight(metric_footprint * PIXEL_RES);
        if abs(normal.y) > 0.5 {
            let center = fract(uv) - vec2<f32>(0.5);
            let radial = length(center) * 2.0;
            let wobble = surface_noise(uv * 5.0 + vec2<f32>(floor(p.y) * 3.1, 0.0)) - 0.5;
            let ring_w = surface_detail_weight(metric_footprint * 14.0);
            let rings = (sin((radial + wobble * 0.12) * 42.0) * 0.5 + 0.5) * ring_w;
            let heart = vec3<f32>(luma) * vec3<f32>(1.34, 1.04, 0.74);
            let sapwood = vec3<f32>(luma) * vec3<f32>(1.92, 1.64, 1.16);
            var cut = mix(heart, sapwood, smoothstep(0.35, 0.95, radial)) * (0.88 + rings * 0.22 + grain * 0.14);
            let rim = smoothstep(0.88, 0.97, max(abs(center.x), abs(center.y)) * 2.0);
            cut = mix(cut, base * 0.70, rim);
            // Moss settles on some upper ends; cut faces still read as wood.
            let moss = smoothstep(0.44, 0.68, moss_patch) * select(0.0, 0.62, normal.y > 0.0);
            color = mix(cut, moss_tone * (0.90 + mottling * 0.4), moss);
        } else {
            let t = physical_uv * PIXEL_RES;
            let col_i = floor(t.x / 3.5);
            let col_x = t.x - col_i * 3.5;
            let col_hash = surface_hash(vec2<f32>(col_i, 17.0));
            let plate_h = 10.0 + floor(surface_hash(vec2<f32>(col_i, 41.0)) * 8.0);
            let plate_i = floor((t.y + col_hash * 23.0) / plate_h);
            let plate_y = t.y + col_hash * 23.0 - plate_i * plate_h;
            let plate_id = surface_hash(vec2<f32>(col_i, plate_i) + vec2<f32>(5.0, 61.0));
            let age = surface_noise_filtered(physical_uv / 2.7 + vec2<f32>(37.0, -13.0), metric_footprint / 2.7);
            // Some column seams are deep furrows; the rest are hairline.
            let deep = step(0.72, col_hash);
            let furrow = max(step(col_x, 1.0) * mix(0.70, 0.46, deep), step(plate_y, 1.0) * 0.78);
            let lit_edge = step(1.0, col_x) * step(col_x, 2.0) * step(1.0, plate_y) * 0.09;
            let plate_value = 0.84 + plate_id * 0.30;
            var bark_color = mix(base, olive, 0.26 + smoothstep(0.40, 0.80, age) * 0.24)
                * mix(1.0, plate_value * (1.0 - furrow) + lit_edge, texel_w)
                * (1.0 + grain * 0.10);
            let colony = lichen_colony(continuous_uv + vec2<f32>(-9.0,31.0), metric_footprint);
            let lichen = colony.x * smoothstep(0.52,0.72,age) * step(0.5,plate_id);
            let lichen_tone = vec3<f32>(luma) * vec3<f32>(1.62, 1.66, 1.30);
            bark_color = mix(bark_color, lichen_tone * colony.y * (1.0 - furrow * 0.5), lichen * 0.45);
            // Moss creeps up the shaded, older runs and sits in the furrows.
            let creep = smoothstep(0.58, 0.82, age) * smoothstep(0.40, 0.64, moss_patch);
            color = mix(bark_color, moss_tone * (1.0 - furrow * 0.4), creep * 0.38);
        }
    } else if kind == 6 {
        // Three painted tones per crown lobe, chosen by face orientation:
        // sunlit tops lean yellow-green, sides keep the mid green and
        // undersides take the cool sky bounce instead of sinking to dark
        // green. Every term is a smooth metric field, so the tone survives
        // when the detail filters away and no motif repeats per voxel.
        // Species is geometry metadata, not a side effect of its palette.
        // Only legacy proxies without a block key retain the colour fallback.
        let pine = select(select(0.0, 1.0, block == BLOCK_PINE_LEAVES),
            smoothstep(-0.02, 0.02, base.b - base.r), block == 0u);
        let luma = dot(base, vec3<f32>(0.2126, 0.7152, 0.0722));
        let grove = surface_noise_filtered(landscape_p / 34.0 + vec2<f32>(8.0, 61.0), metric_footprint / 34.0);
        let crown_uv = (p.xz + p.y * vec2<f32>(0.41, -0.23)) * cam.world_params.x;
        // Two lobe scales: 4.8 m masses and 2.2 m clumps inside them, so a
        // crown reads as clustered lobes with shaded hollows, not one slab.
        let crown_mass = surface_noise_filtered(crown_uv / 4.8, metric_footprint * 0.27)
            + (surface_noise_filtered(crown_uv / 2.2 + vec2<f32>(13.0, -41.0), metric_footprint * 0.45) - 0.5) * 0.6;
        // Neighbouring groves drift between warm and cool greens.
        var mid = mix(base, vec3<f32>(luma), 0.16)
            * mix(vec3<f32>(0.80, 0.86, 0.80), vec3<f32>(0.90, 0.90, 0.70), grove);
        var tropical = 0.0;
        if cam.scene_seed.y >= 10u {
            // Crowns follow the climate their floor does: savanna khaki,
            // rainforest deep emerald, cold stands grey-green.
            let climate = interpolated_climate;
            let warm = smoothstep(0.53, 0.60, climate.x);
            let polar = 1.0 - smoothstep(0.21, 0.30, climate.x);
            let savanna = warm * (1.0 - smoothstep(0.46, 0.53, climate.y));
            tropical = warm * smoothstep(0.65, 0.72, climate.y) * (1.0 - pine);
            mid = mix(mid, vec3<f32>(luma) * vec3<f32>(1.30, 1.16, 0.54), savanna * 0.60 * (1.0 - pine));
            mid = mix(mid, vec3<f32>(luma) * vec3<f32>(0.54, 1.14, 0.68), tropical * 0.68);
            mid = mix(mid, vec3<f32>(luma) * vec3<f32>(0.84, 0.98, 0.86), polar * 0.40);
        }
        var lobe_top = mid * vec3<f32>(1.62, 1.46, 0.56);
        var lobe_under = mix(mid, vec3<f32>(luma) * vec3<f32>(0.80, 0.98, 1.18), 0.66);
        var lobe_side = mid * (0.80 + crown_mass * 0.34);
        // A painted three-tone ramp follows crown lobes in world space. The
        // filtered mass blends out before individual lobe bands become pixels.
        let band_weight = tropical * surface_detail_weight(metric_footprint / 2.2);
        let emerald = smoothstep(0.33, 0.42, crown_mass);
        let lime = smoothstep(0.62, 0.71, crown_mass);
        let crown_top = vec3<f32>(0.48, 0.94, 0.69)
            + emerald * vec3<f32>(0.26, 0.27, -0.10) + lime * vec3<f32>(0.45, 0.22, -0.18);
        let crown_side = mix(vec3<f32>(0.32, 0.70, 0.55), vec3<f32>(0.55, 0.98, 0.61), emerald);
        lobe_top = mix(lobe_top, luma * crown_top, band_weight);
        lobe_side = mix(lobe_side, luma * crown_side, band_weight);
        lobe_under = mix(lobe_under, luma * vec3<f32>(0.36, 0.67, 0.64), tropical * 0.73);
        // Rare autumn-gold stands: one regional field, soft at its edges.
        let gold = smoothstep(0.85, 0.91, surface_noise_filtered(landscape_p / 260.0
            + vec2<f32>(23.0, -57.0), metric_footprint / 260.0)) * (1.0 - pine);
        lobe_top = mix(lobe_top, vec3<f32>(luma) * vec3<f32>(2.30, 1.62, 0.42), gold);
        lobe_side = mix(lobe_side, vec3<f32>(luma) * vec3<f32>(1.86, 1.10, 0.30) * (0.80 + crown_mass * 0.34), gold);
        lobe_under = mix(lobe_under, vec3<f32>(luma) * vec3<f32>(1.10, 0.86, 0.80), gold);
        // Broadleaf middle planes retain pigment in shade; the sunlit tops
        // and darker conifers keep their existing range. This is reflectance,
        // still gated by the actual sky, canopy and local light below.
        lobe_side *= 1.12;
        lobe_under *= 1.12;
        let up = smoothstep(0.30, 0.90, normal.y);
        let down = smoothstep(0.30, 0.90, -normal.y);
        // Lobe tops also carry the mass shading, so the sunlit plane of a
        // crown is a field of lighter and darker clumps rather than flat.
        let broadleaf = mix(mix(lobe_side, lobe_top * (0.88 + crown_mass * 0.20), up), lobe_under, down);
        // Conifers stay dark value anchors with a narrower tonal range, but
        // the same clump field keeps a tall flame from reading as one slab.
        let needles = base * mix(0.86, 1.02, land_tone) * (0.90 + crown_mass * 0.20)
            * mix(vec3<f32>(1.0), vec3<f32>(1.16, 1.12, 0.84), up * 0.6)
            * mix(vec3<f32>(1.0), vec3<f32>(0.92, 1.0, 1.14), down * 0.6);
        // Leaf clumps break flat voxel planes; filtered out before they alias.
        let leaf_cluster = surface_noise_filtered(texel_centre(continuous_uv) * 2.2
            + vec2<f32>(17.0, -9.0), metric_footprint * 2.8);
        // Texel leaves: jittered 3-texel clumps with dark gaps between them
        // and a lit upper rim, so a crown reads as layered leaves, not a
        // painted cube. Faded by texel footprint before it can alias.
        let lt = physical_uv * PIXEL_RES;
        // A 4x coarser clump lattice carries the read out to mid-distance
        // crowns once the texel layer has dithered away.
        let fp_texels = metric_footprint * PIXEL_RES;
        let fine_on = leaf_layer_on(lt, fp_texels, vec2<f32>(0.0));
        let coarse_on = leaf_layer_on(lt * 0.25, fp_texels * 0.25, vec2<f32>(2.0, 1.0));
        let coarse_texel = select(1.0, mix(1.0, leaf_clump_value(lt * 0.25, pine, 37.0), 0.85), coarse_on);
        let leaf_texel = select(coarse_texel, leaf_clump_value(lt, pine, 0.0), fine_on);
        color = mix(broadleaf, needles, pine) * (0.93 + leaf_cluster * 0.14) * leaf_texel;
    } else if kind == 9 {
        color *= 1.0 + mottling * 0.2;
    } else if kind == 46 {
        // Fired clay: staggered overlapping courses, connected chips and a
        // broad hue per tile. Fine seams fade before minification aliases.
        let t = floor(physical_uv * PIXEL_RES);
        let row = floor(t.y / 7.0);
        let sx = t.x + (row - floor(row / 2.0) * 2.0) * 4.0;
        let col = floor(sx / 8.0);
        let local = vec2<f32>(sx - col * 8.0, t.y - row * 7.0);
        let id = surface_hash(vec2<f32>(col, row));
        let seam = max(1.0 - step(1.0, local.x), 1.0 - step(1.0, local.y));
        let lip = step(5.0, local.y) * (1.0 - seam);
        let shade = mix(vec3<f32>(0.78, 0.88, 0.94), vec3<f32>(1.10, 1.03, 0.92), id);
        let tile = base * shade * (1.0 - seam * 0.35 + lip * 0.13);
        color = mix(base, tile, surface_detail_weight(metric_footprint * PIXEL_RES));
    } else if kind == 11 {
        // Boards 5 texels (25 cm) tall and 1.5-2.5 m long with joints
        // staggered per row, so a wall never repeats per block. One-texel
        // dark seams, a one-texel lit top edge, long grain streaks and the
        // occasional knot; each board has its own warm or cool value.
        let t = physical_uv * PIXEL_RES;
        let row = floor(t.y / 5.0);
        let row_shift = surface_hash(vec2<f32>(row, 3.0)) * 40.0;
        let len = 30.0 + floor(surface_hash(vec2<f32>(row, 11.0)) * 3.0) * 10.0;
        let seg = floor((t.x + row_shift) / len);
        let id = surface_hash(vec2<f32>(seg, row) + vec2<f32>(3.3, 9.1));
        let local_x = t.x + row_shift - seg * len;
        let local_y = t.y - row * 5.0;
        let texel_w = surface_detail_weight(metric_footprint * PIXEL_RES);
        let streak = surface_noise(vec2<f32>(t.x * 0.045 + id * 13.0, floor(local_y) * 1.7 + id * 5.0)) - 0.5;
        let knot_x = 4.0 + surface_hash(vec2<f32>(seg, row) + vec2<f32>(23.0, 7.0)) * (len - 8.0);
        let knot_d = length(vec2<f32>(local_x - knot_x, (local_y - 2.5) * 1.6));
        let has_knot = step(0.80, surface_hash(vec2<f32>(seg, row) + vec2<f32>(-5.0, 19.0)));
        let knot_ring = has_knot * step(1.1, knot_d) * step(knot_d, 2.0);
        let knot_core = has_knot * step(knot_d, 1.1);
        let seam = max(step(local_y, 1.0), step(local_x, 1.0));
        let top_light = step(4.0, local_y) * (1.0 - seam);
        let board_value = 0.86 + id * 0.24;
        var value = board_value * (1.0 + streak * 0.16 * medium_fade);
        value = mix(value, value * 0.60, knot_ring * texel_w);
        value = mix(value, value * 1.10, knot_core * texel_w);
        value = mix(value, value * 0.52, seam * texel_w);
        value = mix(value, value * 1.07, top_light * texel_w);
        let warm = (id - 0.5) * 2.0;
        let tint = vec3<f32>(1.0 + warm * 0.06, 1.0 + warm * 0.01, 1.0 - warm * 0.08);
        color = base * mix(vec3<f32>(0.96), vec3<f32>(value) * tint, medium_fade);
    } else if kind == 12 {
        // Cut stone has 1.2 x 0.6 m courses and 5 cm grain. Filter each at its
        // own wavelength; a grazing floor must not lose whole stones with grit.
        let texel_w = surface_detail_weight(metric_footprint * PIXEL_RES);
        let course_w = surface_detail_weight(metric_footprint / 0.60);
        let weathered = smoothstep(0.045, 0.11, base.g - base.r);
        let moss = vec3<f32>(0.30, 0.38, 0.22);
        color = mix(base * vec3<f32>(0.93, 0.93, 0.92), moss, weathered * 0.04);
        surface_finish = vec3<f32>(1.0, 1.0, 0.0);
        // Do not evaluate invisible grain, bevels or mortar in the far field.
        if course_w > 0.0 {
            let t = mix(continuous_uv * PIXEL_RES, floor(physical_uv * PIXEL_RES), texel_w);
            let row = floor(t.y / 12.0);
            let sx = t.x + (row - 2.0 * floor(row * 0.5)) * 12.0;
            let bx = floor(sx / 24.0);
            let id = surface_hash(vec2<f32>(bx, row) + vec2<f32>(17.0, 5.0));
            let id2 = surface_hash(vec2<f32>(bx, row) + vec2<f32>(-23.0, 61.0));
            let tint = mix(vec3<f32>(0.95, 0.97, 1.00), vec3<f32>(1.04, 1.01, 0.96), id2);
            let mortar_col = mix(base * vec3<f32>(0.52, 0.55, 0.58), moss * 0.55, weathered * (0.3 + id * 0.3));
            if texel_w < 1.0 {
                let span = metric_footprint * PIXEL_RES;
                let joint_x = surface_periodic_band(sx, 24.0, 1.0, span);
                let joint_y = surface_periodic_band(t.y, 12.0, 1.0, span);
                let mortar = 1.0 - (1.0 - joint_x) * (1.0 - joint_y);
                let course = mix(base * tint * (0.84 + id * 0.30), mortar_col, mortar);
                color = mix(color, course, course_w);
                let finish = vec3<f32>(mix(0.92 + id * 0.16, 1.25, mortar), mix(1.0, 0.4, mortar), 0.0);
                surface_finish = mix(surface_finish, finish, course_w);
            }
            if texel_w > 0.0 {
                let local = vec2<f32>(sx - bx * 24.0, t.y - row * 12.0);
                let chip = id > 0.72 && local.x >= 21.0 && local.y >= 10.0;
                let joint = local.x < 1.0 || local.y < 1.0 || chip;
                let lit_edge = (local.y >= 11.0 && local.x < 23.0) || (local.x >= 1.0 && local.x < 2.0 && local.y < 11.0);
                let shade_edge = (local.y >= 1.0 && local.y < 2.0) || (local.x >= 23.0 && local.y < 11.0);
                let crack = id2 > 0.80 && abs(local.x - (5.0 + id2 * 6.0 + floor(local.y * 0.5))) < 0.6 && local.y > 2.0 && local.y < 10.0;
                let grit = (surface_hash(t + vec2<f32>(7.0, 29.0)) - 0.5) * 0.07;
                var value = 0.84 + id * 0.30 + grit;
                value += select(0.0, 0.07, lit_edge) - select(0.0, 0.09, shade_edge) - select(0.0, 0.22, crack);
                var block = base * tint * value;
                let stain = smoothstep(0.58, 0.86, surface_noise_filtered(physical_uv / 4.0, metric_footprint / 4.0));
                block = mix(block, moss * value, stain * weathered * 0.22);
                color = mix(color, select(block, mortar_col, joint), texel_w);
                let face_rough = (0.92 + id * 0.16) * select(1.0, 0.80, lit_edge) * select(1.0, 1.3, crack);
                surface_finish = mix(surface_finish, vec3<f32>(select(face_rough, 1.25, joint), select(1.0, 0.4, joint), 0.0), texel_w);
            }
        }
    } else if kind == 13 {
        // Cobbles: rounded stones ~30 cm across, each its own value and cool or
        // warm lean, domed so the lit upper-left of every stone is paler, set in
        // dark packed earth one texel wide. Cells sample the 5 cm texel lattice.
        let texel_w = surface_detail_weight(metric_footprint * PIXEL_RES);
        let cell = cobble_cells(physical_uv);
        let gap = 1.0 - smoothstep(COBBLE_GAP, COBBLE_GAP + 0.05, cell.edge);
        let dome = smoothstep(COBBLE_GAP, 0.95, cell.edge);
        let lean = normalize(vec2<f32>(-0.6, 0.8));
        let lit = dot(cell.to_seed / max(length(cell.to_seed), 0.05), lean) * (1.0 - dome) * 0.5;
        let tone = vec3<f32>(1.0 + (cell.id - 0.5) * 0.10, 1.0, 1.0 - (cell.id - 0.5) * 0.12);
        let value = 0.72 + cell.id * 0.46 + dome * 0.16 + lit * 0.14;
        let grit = (surface_hash(floor(physical_uv * PIXEL_RES)) - 0.5) * 0.06 * texel_w;
        let stone = base * tone * (value + grit);
        let earth = base * vec3<f32>(0.34, 0.31, 0.27);
        let detailed = mix(stone, earth, gap);
        let mean = base * vec3<f32>(0.80, 0.79, 0.78);
        color = mix(mean, detailed, medium_fade) * (1.0 + mottling * 0.25);
        // Stone tops are worn smooth toward their centres; the earth between is dry and rough.
        let worn = mix(0.70, 1.0, 1.0 - dome) * (0.9 + cell.id * 0.2);
        surface_finish = mix(vec3<f32>(1.0, 1.0, 0.0), vec3<f32>(mix(worn, 1.25, gap), mix(1.15, 0.45, gap), 0.0), medium_fade);
    } else if kind == 14 || kind == 15 || kind == 20 || kind == 21 {
        // Ore is stone with nuggets in it. The host keeps the stone's plates
        // and fractures; each nugget has a lit upper-left rim, a shaded
        // lower-right rim and drops a one-texel shadow on the host, as every
        // sprite. Far away it settles to the host blended with the mean cover.
        let q = (floor(p * cam.world_params.x * PIXEL_RES) + vec3<f32>(0.5)) / PIXEL_RES;
        let host = rock_surface(base, q, normal, metric_footprint);
        let texel_w = surface_detail_weight(metric_footprint * PIXEL_RES);
        var ore_color = select(vec3<f32>(0.018,0.02,0.022),vec3<f32>(0.34,0.16,0.07),kind==15);
        if kind == 20 { ore_color = vec3<f32>(0.46,0.22,0.075); }
        if kind == 21 { ore_color = vec3<f32>(0.42,0.55,0.62); }
        let mean = mix(host, ore_color, ORE_MEAN_COVER * 0.92);
        if texel_w <= 0.0 {
            color = mean;
        } else {
            let t = floor(physical_uv * PIXEL_RES);
            let covering = ore_nugget_cell(t);
            let inside = covering.z > 0.5;
            let lit_side = ore_nugget_at(t + vec2<f32>(-1.0, 1.0));
            let shade_side = ore_nugget_at(t + vec2<f32>(1.0, -1.0));
            let fleck = surface_hash(t + vec2<f32>(5.0, 37.0));
            var nugget = ore_color * (0.90 + fleck * 0.20);
            // About a third of copper nuggets have weathered to verdigris.
            let weathered = kind == 20 && surface_hash(covering.xy + vec2<f32>(-5.0, 77.0)) < 0.35;
            nugget = select(nugget, vec3<f32>(0.17, 0.36, 0.29) * (0.90 + fleck * 0.20), weathered);
            // Coal is too dark to brighten by a factor; its rim is a glossy grey.
            let rim_lit = select(nugget * 1.55, vec3<f32>(0.085, 0.088, 0.096), kind == 14);
            nugget = select(nugget, rim_lit, !lit_side);
            nugget = select(nugget, nugget * 0.62, lit_side && !shade_side);
            let shadowed = !inside && lit_side;
            let detailed = select(host * select(1.0, 0.60, shadowed), nugget, inside);
            color = mix(mean, detailed, texel_w);
        }
    } else if kind >= 47 && kind <= 51 {
        // Plated metal on the 5 cm lattice: 10x10 texel plates with a dark
        // groove, a lit top-left bevel, a shaded bottom-right bevel, four corner
        // rivets and brushed rows that run along each plate. Value, not hue,
        // carries the read so metals stay distinct by base colour.
        let t = floor(physical_uv * PIXEL_RES);
        let plate = floor(t / 10.0);
        let local = t - plate * 10.0;
        let id = surface_hash(plate + vec2<f32>(53.0, 11.0));
        let texel_w = surface_detail_weight(metric_footprint * PIXEL_RES);
        let groove = local.x < 1.0 || local.y < 1.0;
        let lit_bevel = (local.y >= 9.0 && local.x >= 1.0) || (local.x >= 1.0 && local.x < 2.0 && local.y >= 1.0);
        let shade_bevel = (local.y >= 1.0 && local.y < 2.0 && local.x >= 2.0) || (local.x >= 9.0 && local.y >= 1.0);
        let rivet_at = vec2<f32>(select(2.0, 7.0, local.x > 4.5), select(2.0, 7.0, local.y > 4.5));
        let rivet = all(abs(local - rivet_at) < vec2<f32>(0.5)) && !groove;
        let rivet_shade = all(abs(local - rivet_at - vec2<f32>(1.0, -1.0)) < vec2<f32>(0.5)) && !groove;
        let brush = (surface_hash(vec2<f32>(t.y, plate.x + 7.0)) - 0.5) * 0.10;
        let scratch = step(0.985, surface_hash(t + vec2<f32>(19.0, 3.0)));
        var value = 0.95 + (id - 0.5) * 0.10 + brush;
        value += select(0.0, 0.16, lit_bevel) - select(0.0, 0.14, shade_bevel);
        value += select(0.0, 0.30, rivet) - select(0.0, 0.20, rivet_shade) + scratch * 0.18;
        value = select(value, 0.52, groove);
        color = mix(base * 0.95, base * value, texel_w);
        // Brushed rows vary in sheen, scratches and rivet heads are polished, grooves hold grime.
        var sheen = (1.0 + brush * 1.5) * select(1.0, 0.8, lit_bevel);
        sheen = select(sheen, sheen * 0.45, scratch > 0.5 || rivet);
        sheen = select(sheen, 1.5, groove);
        surface_finish = mix(vec3<f32>(1.0, 1.0, 0.0), vec3<f32>(sheen, 1.0, 0.0), texel_w);
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
        let clouded = mix(base * 0.78, vec3<f32>(0.26, 0.18, 0.28), marble * 0.42);
        // Near the camera the marbling gathers into thin violet veins on the
        // texel grid, a fainter branch net inside the cloudy patches and a
        // one-texel grain, so the stone reads as polished rock, not a blur.
        let texel_w = surface_detail_weight(metric_footprint * PIXEL_RES);
        if texel_w <= 0.0 {
            color = clouded;
        } else {
            let grain = surface_hash(floor(physical_uv * PIXEL_RES) + vec2<f32>(61.0, 17.0));
            let vein = texel_thread(physical_uv, 1.6, vec2<f32>(-23.0, 9.0), 0.45);
            let branch = texel_thread(physical_uv, 4.1, vec2<f32>(-11.0, 5.0), 0.40) * smoothstep(0.45, 0.65, marble);
            var stone = clouded * (0.86 + grain * 0.24);
            stone = mix(stone, vec3<f32>(0.34, 0.22, 0.44), max(vein * 0.48, branch * 0.26));
            color = mix(clouded * 1.02, stone, texel_w);
        }
    } else if kind == 53 {
        let tile = fract(continuous_uv / 0.5);
        let seam = industrial_seam(continuous_uv, vec2<f32>(0.5), 0.025);
        let facet = select(0.97, 1.04, tile.x > tile.y);
        color = base * mix(1.0, facet * (1.0 - seam * 0.28), medium_fade);
    } else if kind == 24 || kind == 25 {
        let panel=fract(uv);
        let rim=1.0-smoothstep(0.025,0.07,min(min(panel.x,panel.y),min(1.0-panel.x,1.0-panel.y)));
        color*=1.0-rim*0.55*medium_fade;
        if kind == 25 {
            let vent=step(0.18,panel.x)*step(panel.x,0.82)*step(0.22,panel.y)*step(panel.y,0.72);
            color=mix(color,vec3<f32>(0.13,0.055,0.018),vent*medium_fade);
        }
    }
    if is_factory_panel(kind) {
        // Assemblies retain metre-sized panels even at quarter-metre voxels.
        color = furnishing_iron(base, continuous_uv, metric_footprint);
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
// The same material language at a ground console and on a celestial hull.
// Structural panels use a 10 cm grid. A finer directional grain is reserved
// for brushed alloy, while ceramic and radiator faces keep their own finish.
fn orbital_pixel_surface(base:vec3<f32>,uv:vec2<f32>,finish:f32,fp:f32)->vec3<f32> {
    let t=floor(uv*20.0);
    let coarse=floor((t+vec2<f32>(32.0))/2.0);
    let plate=floor(coarse/32.0);
    let pixel=coarse-plate*32.0;
    let bay=surface_hash(plate);
    let body=select(select(0.98,1.0,bay>0.22),1.018,bay>0.84);
    let alloy=finish>0.25 && finish<0.6;
    let brush=(surface_hash(vec2<f32>(t.y,plate.x))-0.5)*0.012;
    let seam=pixel.x==0.0 || pixel.y==0.0;
    let rim=pixel.x==1.0 || pixel.y==1.0;
    let fastener=pixel.x>=3.0 && pixel.x<=4.0 && pixel.y>=3.0 && pixel.y<=4.0;
    var shade=body;
    if seam {shade=0.81;} else if rim {shade=body+0.025;}
    if fastener && alloy {shade=0.91;}
    if finish>0.75 {
        // Thermal banks: ordered channels and cross ties, never random speckle.
        let channel=coarse.x-floor(coarse.x/4.0)*4.0;
        shade=select(0.94,0.70,channel<1.0);
        if pixel.y==0.0 {shade=1.025;}
    }
    if finish>0.6 && finish<0.75 {
        shade=select(1.0,0.86,seam);
    }
    let fine_plate=surface_detail_weight(fp/0.1);
    let large=surface_detail_weight(fp/3.2);
    // Plate-scale variation survives after individual pixels become subpixel.
    // Integrate narrow joints over raster support instead of dropping the
    // entire 3.2 m plate when its 0.1 m edge can no longer resolve.
    let distance=abs(fract((uv+vec2<f32>(1.55))/3.2+vec2<f32>(0.5))-vec2<f32>(0.5))*3.2;
    let coverage=clamp((vec2<f32>(0.05+fp*0.5)-distance)/max(fp,0.0001),vec2<f32>(0.0),vec2<f32>(1.0));
    let joint=max(coverage.x,coverage.y);
    var broad=body-joint*0.17;
    if finish>0.75 {broad=mix(0.88,shade,surface_detail_weight(fp/0.4));}
    if finish>0.6 && finish<0.75 {broad=1.0-joint*0.14;}
    shade=mix(broad,shade,fine_plate);
    // Fade to the material mean, not white or a different far-distance shader.
    let mean=select(select(0.99,0.88,finish>0.75),0.991,finish>0.6 && finish<0.75);
    let grain=select(0.0,brush*surface_detail_weight(fp*20.0),alloy);
    return base*mix(mean,shade,large)*(1.0+grain);
}

fn world_planet_fragment(in: VsOut) -> bool {
    return cam.planet[0].w > 0.5 && (in.camera_relative & 1u) == 0u;
}

// Metres that material patterns read; face-local on a planet.
fn world_pattern_pos(in: VsOut) -> vec3<f32> {
    if world_planet_fragment(in) { return in.voxel_pos * cam.world_params.x; }
    return cam.camera_pos.xyz + in.relative_pos;
}

// Absolute chart metres, the frame of lights, weather and ownership tests.
fn world_chart_pos(in: VsOut) -> vec3<f32> {
    if world_planet_fragment(in) {
        let anchor = vec3<f32>(f32(world_face_anchor((in.camera_relative >> 1u) & 7u)), 0.0, 0.0);
        return (in.voxel_pos + anchor) * cam.world_params.x;
    }
    return cam.camera_pos.xyz + in.relative_pos;
}

@fragment
fn fs_main(in: VsOut) -> WorldOutput {
    // Evaluate derivatives before material-dependent control flow/discards.
    interpolated_climate = in.climate;
    let relative_pos = in.relative_pos;
    let world_pos = world_chart_pos(in);
    let pattern_pos = world_pattern_pos(in);
    let water_chart_dx = dpdx(pattern_pos.xz);
    let water_chart_dy = dpdy(pattern_pos.xz);
    let water_bed_dx = dpdx(in.base_color);
    let water_bed_dy = dpdy(in.base_color);
    let world_dx = dpdx(relative_pos);
    let world_dy = dpdy(relative_pos);
    let coverage_fw = fwidth(in.terrain_weights);
    // Undo perspective interpolation before taking angular derivatives.
    // A helper lane beyond a grazing triangle's horizon can have negative
    // reciprocal W: normalizing relative_pos alone flips that lane's ray and
    // turns a one-pixel firefly into a screen-wide bright strip. Multiplying
    // by reciprocal W recovers the continuous, affine camera ray instead.
    let view_ray = normalize(relative_pos);
    let screen_ray = normalize(relative_pos * in.pos.w);
    let ambience_pixel = max(length(dpdx(screen_ray)), length(dpdy(screen_ray)));
    let scale = max(cam.world_params.x, 1e-6);
    let fp_m = terrain_pixel_footprint(world_dx, world_dy);
    let far_water_depth = terrain_is_backdrop(in.ao_strength);
    let surface_water_depth = select(in.water_optics.x, in.surface.w * 16.0, far_water_depth);
    let water_depth_change = length(vec2<f32>(dpdx(surface_water_depth), dpdy(surface_water_depth)));
    let model_fp = max(length(dpdx(in.model_uv)), length(dpdy(in.model_uv))) * PIXEL_RES;
    // Between a portal's virtual eye and its exit plane lies the far side of
    // the exit wall; it must not occlude the destination.
    let transition = (in.camera_relative >> 4u) & 255u;
    let arrival = in.camera_relative >> 12u;
    if arrival != 0u && !portal_view()
        && terrain_dither(vec2<u32>(in.pos.xy)) >= arrival - 1u { discard; }
    if transition != 0u {
        // Complementary 8x8 Bayer coverage. Both representations use the
        // same screen pixel, so coincident surfaces cannot fight over depth.
        let threshold = terrain_dither(vec2<u32>(in.pos.xy));
        let order = select(threshold, 63u - threshold, (transition & 128u) != 0u);
        if (order < (transition & 63u)) != ((transition & 64u) != 0u) { discard; }
    }
    if portal_view() && dot(cam.portal_clip.xyz, relative_pos) + cam.portal_clip.w < 0.0 { discard; }
    var albedo = clamp(in.base_color, vec3<f32>(0.0), vec3<f32>(1.0));
    let kind = i32(in.material_family & 255u);
    let block = in.material_family >> 8u;
    let is_viewmodel = (in.camera_relative & 1u) != 0u;
    // Family 39 is shared by authored models and generic voxel surfaces.
    let authored_model = kind == 39 && block == 0u;
    // Celestial bodies are projected from f64 planet-centred ephemerides into
    // the observer tangent frame. They do not belong to terrain coverage or
    // ground shadow cascades. Positive AO >= 4 carries their eclipse fraction.
    if in.ao_strength >= 4.0 {
        // Celestial geometry bypasses ordinary material fog below.
        if uw_active() || lava_eye_active() { discard; }
        // Same compressed spherical datum as orbital::megastructures.
        let radius = 1592750.0;
        // Preserve the small altitude before adding the planetary radius.
        let observer_altitude = max(cam.camera_pos.y, 0.0);
        let observer_radius = radius + observer_altitude;
        let distance = length(relative_pos);
        let b = observer_radius * view_ray.y;
        let ground_c = observer_altitude * (2.0*radius + observer_altitude);
        let discriminant = b*b - ground_c;
        if b < 0.0 && discriminant >= 0.0 {
            // The direct near root subtracts nearly equal planet-scale values.
            let hit = ground_c / max(-b + sqrt(discriminant), 1e-6);
            if hit >= 0.0 && hit < distance { discard; }
        }
        let sunlight = clamp(in.ao_strength-4.0,0.0,1.0);
        let geometric_normal = normalize(in.normal);
        let lat = -cam.camera_pos.z/radius;
        let lon = cam.camera_pos.x/radius;
        let reference_sun = normalize(light.sun_dir_intensity.xyz);
        let east = vec3<f32>(cos(lon),-sin(lon),0.0);
        let up = vec3<f32>(sin(lon)*cos(lat),cos(lon)*cos(lat),-sin(lat));
        let south = vec3<f32>(sin(lon)*sin(lat),cos(lon)*sin(lat),cos(lat));
        let sun = normalize(vec3<f32>(dot(reference_sun,east),dot(reference_sun,up),dot(reference_sun,south)));
        let plated = kind == 62 || kind == 63;
        var base = albedo;
        if plated { base = orbital_pixel_surface(albedo,in.model_uv,in.surface.w,model_fp/PIXEL_RES); }
        let normal=geometric_normal;
        let ndl = max(dot(normal,sun),0.0);
        // Earthshine keeps unlit hulls readable. Alloy, ceramic and thermal
        // banks retain different, bounded highlights on the celestial path.
        // A sunlit planet subtends almost a hemisphere in these low shells.
        // Its reflected daylight is substantial; a tiny constant left every
        // ground-facing surface an unreadable blue-black silhouette at noon.
        let earthshine = 0.18*max(sun.y,0.0)*max(dot(normal,vec3<f32>(0.0,-1.0,0.0)),0.0);
        let cavity = 1.0;
        var orbital_light = base*((0.018+earthshine)*cavity+sunlight*ndl*1.65);
        let half_vector = normalize(sun-view_ray);
        let finish = in.surface.w;
        let reflective = plated && finish > 0.25 && finish < 0.75;
        let specular = pow(max(dot(normal,half_vector),0.0),select(28.0,110.0,finish>0.6));
        let facing = max(dot(normal,-view_ray),0.0);
        let fresnel = 0.04+0.96*pow(1.0-facing,5.0);
        let specular_tint = select(vec3<f32>(1.0,0.94,0.83),mix(vec3<f32>(0.8),albedo,0.45),reflective);
        orbital_light += specular_tint*specular*sunlight*ndl*select(0.05,0.6,reflective)*(0.35+fresnel)*cavity;
        if kind == 40 { orbital_light = albedo*max(in.surface.w,0.0)*0.35; }
        // Integrate only the atmosphere crossed by the ray, not hundreds of
        // kilometres of fictitious ground fog. Spherical density falls with
        // actual radial altitude. A finite top-of-air intersection bounds work.
        let top_c = (observer_altitude-120000.0)*(2.0*radius+observer_altitude+120000.0);
        let top_d = b*b-top_c;
        let top_root = sqrt(max(top_d,0.0));
        let air_exit = select(-b+top_root, -top_c/max(b+top_root,1e-6), b>=0.0);
        let air_end = min(distance,max(air_exit,0.0));
        var rayleigh_column = 0.0;
        var aerosol_column = 0.0;
        // Quadratic intervals resolve dense air near a surface observer.
        for (var i=0u; i<16u; i=i+1u) {
            let lo = f32(i)/16.0; let hi = f32(i+1u)/16.0;
            let a = air_end*lo*lo; let z = air_end*hi*hi;
            let t = (a+z)*0.5;
            let radial_delta = ground_c+t*(2.0*b+t);
            let altitude = max(radial_delta/(sqrt(max(radius*radius+radial_delta,0.0))+radius),0.0);
            rayleigh_column += exp(-altitude/8000.0)*(z-a);
            aerosol_column += exp(-altitude/1200.0)*(z-a);
        }
        let transmission = exp(-vec3<f32>(5.8e-6,13.5e-6,33.1e-6)*rayleigh_column
            -vec3<f32>(3.0e-6)*aerosol_column);
        let sky = max(atmosphere_color(view_ray),vec3<f32>(0.0));
        let through_air = orbital_light*transmission+sky*(vec3<f32>(1.0)-transmission);
        // Foreground clouds are integrated once by the volumetric composite.
        // The weather-summary fog approximation here double-occluded stations
        // and changed their contrast independently of the visible cloud edge.
        // Widened raster support must retain physical cable coverage here too;
        // this early return previously bypassed the regular material path.
        let coverage = select(1.0,clamp(in.surface.w,0.0,1.0),kind==61);
        return WorldOutput(vec4<f32>(mix(sky,through_air,coverage),1.0),vec4<f32>(0.0));
    }
    // Authored rigid bodies share the vertex-colour/UV path, with their own finish.
    let architecture_material = kind >= 64 && kind <= 68;
    let cut_crystal = kind == 56 || kind == 57 || kind == 58;
    // Mana hedrons: cut crystal with surface.w = 2 + glow and surface.xy the
    // spine's x/z offset from this fragment in metres (see hedrons.rs).
    let hedron = kind == 56 && in.surface.w >= 1.5 && in.surface.w < 4.0;
    let enemy_metal = kind == 59 || kind == 60;
    let creature_material = kind >= 69 && kind <= 72;
    let authored_kit = is_viewmodel && (kind == 54 || kind == 55);
    // UV.x >= 2 identifies the authored atlas without changing the vertex ABI.
    // textureLoad preserves integer clusters and performs sRGB-to-linear decoding.
    let pixel_kit = authored_kit && in.model_uv.x >= 2.0;
    if pixel_kit {
        let uv = in.model_uv - vec2<f32>(2.0, 0.0);
        let dimensions = vec2<i32>(512,256);
        let texel = clamp(vec2<i32>(floor(uv * vec2<f32>(dimensions))), vec2<i32>(0), dimensions - vec2<i32>(1));
        let paint = textureLoad(held_kit_atlas, texel, 0).rgb;
        if kind == 55 && in.surface.w > 0.5 {
            // The mineral retains its authored colour at rest. Only its stored
            // energy takes the installed spell's hue during a cast.
            let energy = clamp((in.surface.w - 0.60) / 0.35, 0.0, 1.0);
            let value = dot(paint, vec3<f32>(0.2126, 0.7152, 0.0722));
            albedo = mix(paint, in.base_color * value * 1.8, energy);
        } else {
            albedo *= paint;
        }
    }
    let kit_metal = authored_kit && kind == 55 && in.surface.w > 0.3 && in.surface.w < 0.5;
    let kit_glass = authored_kit && kind == 55 && in.surface.w > 0.5;
    let rigid_material = authored_model || cut_crystal || enemy_metal || authored_kit || creature_material;
    // Orbital rigid parts: baked local UVs; w selects ceramic/alloy/radiator.
    let orbital_part = kind == 62 || kind == 63;
    let plated_block = kind >= 47 && kind <= 51;
    let industrial_block = kind == 12 && is_industrial_block(block);
    // Gilded Cave blocks author their own finish, relief and metal per texel.
    let gilded_block = kind == 12 && is_gilded_block(block);
    // Hell rock, caps and drifts author their own relief and ember glow.
    let hell_block = is_hell_block(block) && !is_viewmodel && !terrain_is_backdrop(in.ao_strength);
    let manufactured_surface = orbital_part || plated_block || industrial_block || gilded_block || is_factory_panel(kind)
        || kind == 23 || kind == 41 || kind == 53 || kind == 61;
    let authored_finish = rigid_material || manufactured_surface;
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
    if hedron { roughness = 0.09; reflectance = 0.53; }
    if kind == 58 { roughness = 0.82; reflectance = 0.24; }
    if authored_model { roughness = 0.82; reflectance = 0.35; }
    if creature_material { roughness = 0.92; reflectance = 0.24; }
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
    // Authored surfaces carry their own finish; the sleeve remains dry cloth.
    if authored_kit && kind == 55 {
        roughness = select(0.86, 0.52, kit_metal);
        roughness = select(roughness, 0.24, kit_glass);
        reflectance = select(0.16, 0.38, kit_metal || kit_glass);
    }
    if authored_kit && kind == 54 {
        roughness=select(0.88,0.96,in.surface.w > 0.1);
        reflectance=0.16;
    }
    if kind == 40 {
        // Glow particles use surface.y as a dissolve threshold, not as a
        // specular gain. Keep their shading emissive and non-metallic.
        roughness = 1.0;
        reflectance = 0.0;
    }
    let is_torch_material = in.surface.w < 0.0;
    if authored_kit && is_torch_material {
        // Fuel is dry charcoal; fire emits light and has no reflective surface lobe.
        roughness = 1.0;
        reflectance = 0.0;
    }
    // Regular resident vertices encode cached overhead visibility in [4,5].
    // Preserve dedicated torch/fire metadata and unencoded horizon geometry.
    // Resident water writes 8 or 9 at the vertex. Leave room for rasterizer
    // interpolation error below 8 on clear-water triangles.
    let resident_water = kind == 8 && in.surface.w >= 7.5 && terrain_has_resident_metadata(in.ao_strength);
    var sky_access = select(1.0, clamp(in.surface.w - 4.0, 0.006, 1.0),
        in.surface.w >= 4.0 && kind != 40 && !creature_material
            && terrain_has_resident_metadata(in.ao_strength));
    if resident_water {
        sky_access = in.water_optics.y;
    }
    if authored_kit && in.ao_strength >= 2.0 {
        sky_access=clamp(in.ao_strength-2.0,0.006,1.0);
    }

    let geometric_normal = terrain_safe_unit3(in.normal, vec3<f32>(0.0, 1.0, 0.0));
    var n = geometric_normal;
    var filtered_slope_variance = 0.0;
    let is_backdrop = terrain_is_backdrop(in.ao_strength);
    // Ambient sky visibility. Near the camera the traced probe volume replaces
    // the overhead-only column term; direct light keeps `sky_access`. Planet
    // faces carry face-local voxels the volume does not index, so they keep it.
    var local_sky = LocalSky(1.0, 0.0);
    if dot(in.frame.xyz, in.frame.xyz) < 1e-6 && !is_backdrop && !is_viewmodel && !creature_material
        && kind != 9 && kind != 40 && terrain_has_resident_metadata(in.ao_strength) {
        local_sky = local_sky_visibility(in.voxel_pos + geometric_normal * 0.5, geometric_normal);
    }
    let ambient_sky = mix(sky_access, local_sky.visibility, local_sky.weight);
    let is_natural = kind == 0 || kind == 1 || kind == 2 || kind == 3
        || kind == 4 || kind == 7 || kind == 16 || kind == 33 || kind == 36 || kind == 38;
    let material_scale = max(material_props.y, 0.0625);
    // The held hand and wand are rigid with the camera, but their mesh is
    // rebuilt in world space every frame. World-anchored procedural patterns
    // therefore crawl across them as the player moves, and the planar-UV axis
    // choice flips mid-face as the world normal rotates past 45 degrees, which
    // pops whole triangles. Evaluate their patterns in camera space instead,
    // using both a camera-space position and a camera-space normal.
    var pattern_position = in.voxel_pos;
    var pattern_normal = geometric_normal;
    var vm_right = vec3<f32>(1.0, 0.0, 0.0);
    var vm_up = vec3<f32>(0.0, 1.0, 0.0);
    if is_viewmodel || kind == 56 {
        vm_right = terrain_safe_unit3(cam.ray_right.xyz, vm_right);
        vm_up = terrain_safe_unit3(cam.ray_up.xyz, vm_up);
    }
    if is_viewmodel {
        let vm_forward = terrain_safe_unit3(cam.camera_forward.xyz, vec3<f32>(0.0, 0.0, -1.0));
        pattern_position = vec3<f32>(dot(relative_pos, vm_right), dot(relative_pos, vm_up),
            dot(relative_pos, vm_forward)) / scale;
        pattern_normal = vec3<f32>(dot(geometric_normal, vm_right), dot(geometric_normal, vm_up),
            dot(geometric_normal, vm_forward));
    } else if !is_industrial_block(block) && (kind == 12 || kind == 13)
        && light.fog_color_density.w > 2.0 {
        let radial = in.voxel_pos.xy - vec2<f32>(0.0, light.fog_color_density.w / scale);
        let radius = length(radial);
        if radius > 1.0 {
            let inward = -radial / radius;
            let tangent = vec2<f32>(inward.y, -inward.x);
            // Unroll the shell before choosing a face projection. The brick
            // courses stay in the shell's frame as the local up vector rolls.
            pattern_position = vec3<f32>(atan2(radial.x, -radial.y) * (160.0 / scale), -radius, in.voxel_pos.z);
            let local_normal = vec3<f32>(dot(geometric_normal.xy, tangent), dot(geometric_normal.xy, inward), geometric_normal.z);
            let n_abs = abs(local_normal);
            pattern_normal = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), n_abs.x > n_abs.y);
            if n_abs.z > max(n_abs.x, n_abs.y) { pattern_normal = vec3<f32>(0.0, 0.0, 1.0); }
        }
    }
    let detail_strength = clamp(max(material_props.z, select(0.0, 0.8, creature_material)), 0.0, 2.0);
    var detail_color = albedo;
    natural_tilt = vec3<f32>(0.0);
    surface_finish = vec3<f32>(1.0, 1.0, 0.0);
    gilded_metal = 0.0;
    gilded_glow = vec3<f32>(0.0);
    hell_glow = vec3<f32>(0.0);
    // Fallback for gravel and natural materials outside the four-way blend.
    natural_relief_strength = vec4<f32>(0.045, 0.030, 0.060, 0.025);
    // Distant terrain shades with the detail map's texel normal (16-128 m)
    // instead of the coarse lattice normal, so ridges, spurs and gullies read
    // at any range. Resident terrain keeps its own geometry normals; both get
    // the same broad cavity term, faded in with distance, so the hand-over
    // between them shows no brightness step.
    var macro_normal = geometric_normal;
    var macro_occlusion = 1.0;
    let macro_dist = length(relative_pos);
    // Resident terrain within 64 m has zero cavity reach, so it skips the sample.
    if is_natural && !is_viewmodel && (is_backdrop || macro_dist > 64.0) {
        let detail = horizon_detail_sample(world_pos.xz, horizon_detail_lod(fp_m));
        if detail.weight > 0.0 {
            if is_backdrop {
                macro_normal = terrain_safe_unit3(mix(geometric_normal,
                    horizon_detail_normal(detail.value),
                    detail.weight * horizon_detail_normal_weight(world_pos.xz)), geometric_normal);
            }
            let reach = smoothstep(64.0, 256.0, macro_dist);
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
        let projection = backdrop_projection_weights(macro_normal);
        let coverage = backdrop_coverage(in.terrain_weights, world_pos, fp_m, coverage_fw, projection);
        detail_color = natural_surface_blended(albedo, in.voxel_pos,
            macro_normal, coverage / max(1.0 - gravel, 0.00001), footprint, true, projection);
        if gravel > 0.0 {
            let q = (floor(in.voxel_pos * scale * PIXEL_RES) + vec3<f32>(0.5)) / PIXEL_RES;
            detail_color = mix(detail_color, gravel_surface(albedo, q, geometric_normal, fp_m), gravel);
        }
        if terrain_is_column(in.ao_strength) {
            // Keep the big voxel shapes legible without a checkerboard of
            // unrelated colours across a continuous hillside.
            detail_color *= mix(1.0, horizon_column_value(in.voxel_pos * scale, geometric_normal), 0.35);
        }
        // Shared material strata already describe the rock. Preserve the
        // solved relief normal: horizon-only seven-metre treads invented
        // terraces and produced horizontal bands absent from the geometry.
    } else if orbital_part {
        detail_color=orbital_pixel_surface(albedo,in.model_uv,in.surface.w,model_fp/PIXEL_RES);
    } else if kind == 34 || kind == 35 || rigid_material {
        // Ground-detail tufts and loose stones (mesh/ground_detail.rs) carry
        // their finished colour in the vertex. Painting them with the turf
        // pipeline gave blades the soil/grass-side bank colour.
        detail_color = albedo;
        if authored_kit && !pixel_kit {
            // Brush marks follow the authored part, including bent finger joints.
            // Low contrast keeps painted detail subordinate to the sculpted form.
            let uv = (floor(in.model_uv * 80.0)+vec2<f32>(0.5))/80.0;
            let fade = surface_detail_weight(model_fp * 80.0 / PIXEL_RES);
            if kind == 55 && in.surface.w >= 0.0 && in.surface.w < 0.05 {
                let cluster = floor(uv * vec2<f32>(80.0, 20.0));
                let tone = select(0.92, 1.04, surface_hash(cluster) > 0.35);
                detail_color *= mix(1.0, tone, fade);
            } else if kind == 55 && in.surface.w >= 0.05 && in.surface.w < 0.15 {
                let seam = 1.0 - smoothstep(0.05, 0.16, abs(fract(uv.y * 60.0 + uv.x * 10.0) - 0.5));
                detail_color *= 1.0 - seam * 0.12 * fade;
            } else if kit_metal {
                detail_color = furnishing_iron(detail_color, uv * 4.0, model_fp / PIXEL_RES * 4.0);
            }
        }
        if creature_material {
            // Fixed five-centimetre lattice, baked before joint rotation. Broad
            // clusters replace independent per-pixel noise and never swim.
            let pixel = floor(in.model_uv * 20.0);
            let cluster = floor(pixel / vec2<f32>(3.0, 2.0));
            let h = surface_hash(cluster);
            let shadow = select(0.91, 0.86, kind == 69);
            let tone = select(select(shadow, 1.0, h > 0.24), 1.08, h > 0.82);
            let seam = select(0.0, 0.045, kind == 69 && i32(pixel.y) % 3 == 0 && h < 0.46);
            let rib = select(0.0, 0.04, kind == 71 && i32(pixel.y) % 4 == 0);
            let weave = select(0.0, 0.025, kind == 72 && i32(pixel.x+pixel.y) % 4 == 0);
            // model_fp is a footprint in texels, not metres.
            let fade = 1.0-smoothstep(0.7,1.4,model_fp);
            detail_color *= mix(1.0,tone-seam-rib-weave,fade);
        }
        if kind == 57 || kind == 58 || enemy_metal {
            let cell = floor(in.model_uv * PIXEL_RES);
            let h = surface_hash(select(cell, floor(cell / vec2<f32>(3.0, 2.0)), enemy_metal));
            let grain = select(select(select(0.75, 0.94, h > 0.24), 1.18, h > 0.73), select(0.95, 1.04, h > 0.60), enemy_metal);
            let fade = 1.0 - smoothstep(0.7, 1.4, model_fp);
            detail_color *= mix(1.0, grain, fade);
        }
    } else if hell_block {
        // Hell detail rides on the stock natural surface, so value and grain
        // match the rock and drifts the planet already shows.
        let stock = natural_surface(albedo, in.voxel_pos, geometric_normal, kind, footprint);
        detail_color = hell_surface(stock, in.voxel_pos * cam.world_params.x, geometric_normal, block,
            footprint * cam.world_params.x);
    } else if is_natural {
        // Climate stays in world coordinates: texture scale must not move the
        // snowline, beaches, or biome transitions.
        detail_color = natural_surface(albedo, in.voxel_pos,
            geometric_normal, kind, footprint);
    } else {
        detail_color = block_surface(albedo, pattern_position / material_scale,
            pattern_normal, kind, block, footprint / material_scale);
    }
    albedo = max(mix(albedo, detail_color, detail_strength), vec3<f32>(0.0));
    surface_finish = mix(vec3<f32>(1.0, 1.0, 0.0), surface_finish, min(detail_strength, 1.0));
    // The shared dither is a fallback for surfaces without an authored finish.
    if kind != 8 && kind != 9 && kind != 40 && kind != 42 && !is_torch_material && !is_viewmodel
        && !is_natural && !architecture_material && kind != 5 && kind != 6 && kind != 45
        && kind != 34 && kind != 35 && !authored_finish {
        albedo = terrain_pixel_albedo(albedo, pattern_pos, geometric_normal, fp_m);
    }
    // Diagnostic albedo excludes cached roof visibility and lighting-derived
    // darkening. Apply the configured material tint exactly once, as in the
    // production path below; retain all procedural surface colour and dither.
    let diagnostic_albedo = max(albedo * material_tint.rgb, vec3<f32>(0.0));
    if is_natural {
        let relief = terrain_surface_relief(macro_normal, in.voxel_pos * scale, fp_m);
        n = terrain_safe_unit3(mix(macro_normal, relief.normal, min(detail_strength, 1.0)), macro_normal);
        // Material-shaped relief (pebble domes, clods, rock planes), already
        // faded by footprint in the material functions.
        let tilt = natural_tilt * min(detail_strength, 1.0);
        n = terrain_safe_unit3(n - (tilt - n * dot(n, tilt)), n);
        // Approximate lost slope variance prevents filtered relief from turning
        // into an unnaturally perfect mirror. This is not a measured normal map.
        filtered_slope_variance = relief.variance * min(detail_strength * detail_strength, 1.0);
    }
    if kind == 13 && !is_viewmodel {
        let uv = natural_face_uv(pattern_position * scale / material_scale, pattern_normal);
        let tilt = cobble_tilt(uv, geometric_normal, fp_m / material_scale) * min(detail_strength, 1.0);
        n = terrain_safe_unit3(n - (tilt - n * dot(n, tilt)), n);
    }
    if (gilded_block || hell_block) && !is_viewmodel {
        // Coin domes, tile bevels and relief written by gilded_surface and
        // hell_surface.
        let tilt = natural_tilt * min(detail_strength, 1.0);
        n = terrain_safe_unit3(n - (tilt - n * dot(n, tilt)), n);
    }
    if kind == 12 && !industrial_block && !gilded_block && !is_viewmodel {
        // Cut-stone edges catch the actual lamp; broad face interiors stay quiet.
        let uv = natural_face_uv(pattern_position * scale / material_scale, pattern_normal);
        let row = floor(uv.y / 0.6);
        let stagger = (row - 2.0 * floor(row * 0.5)) * 0.5;
        let local = fract(uv / vec2<f32>(1.2, 0.6) + vec2<f32>(stagger, 0.0));
        let edge = (vec2<f32>(1.0) - smoothstep(vec2<f32>(0.04), vec2<f32>(0.14), local))
            - smoothstep(vec2<f32>(0.86), vec2<f32>(0.96), local);
        let tilt = natural_tilt_from(geometric_normal, edge * 0.16)
            * surface_detail_weight(fp_m / material_scale * PIXEL_RES) * min(detail_strength, 1.0);
        n = terrain_safe_unit3(n - tilt, n);
    }

    let cave_amount = 1.0 - smoothstep(0.03, 0.62, ambient_sky);
    if kind == 6 {
        // Broad, world-fixed leaf-cluster relief gives large voxel planes a
        // restrained roll in the light. Geometry, silhouette and shadow-bias
        // normals stay authoritative; unresolved relief fades continuously.
        let shear = vec2<f32>(0.41, -0.23);
        let crown_uv = (pattern_pos.xz + pattern_pos.y * shear) / 4.8;
        // The gradient is filtered by the same footprint that fades the tilt.
        // Unfiltered, a 4.8 m cell's analytic gradient tilts a side face about
        // twenty degrees, and past forty on strong samples, which crosses the
        // foliage wrap lobe's boundary and mottles mid-distance crowns between
        // lit and unlit. Ground relief uses 0.060 for the same job at a 0.58 m
        // wavelength; crown shape belongs to the clump colour field, so this
        // stays a restrained roll in the light.
        let gradient = surface_noise_grad_filtered(crown_uv, fp_m / 4.8).yz;
        let g = vec3<f32>(gradient.x, dot(gradient, shear), gradient.y);
        let tangent = g - n * dot(n, g);
        n = terrain_safe_unit3(n - tangent * 0.08 * surface_detail_weight(fp_m / 4.8), n);
    }
    // Natural ground already has the shared far-field relief. Built and mined
    // materials previously had no normal detail at all; natural cave surfaces
    // get an additional pass only below the skylight threshold.
    let relief_strength = select(
        mix(0.44, 0.92, cave_amount),
        0.62 * cave_amount,
        is_natural,
    );
    if kind != 6 && kind != 8 && kind != 9 && kind != 34 && !is_backdrop && !is_viewmodel
        && relief_strength > 0.02 && !authored_finish && !architecture_material && kind != 22
    {
        n = surface_relief(n, pattern_pos, fp_m, relief_strength);
        filtered_slope_variance += 0.0016 * relief_strength * relief_strength;
    }
    // Kept for the crystal's film colour in the lighting below.
    var crystal_facet = vec4<f32>(0.5);
    var crystal_resolved = 0.0;
    if kind == 22 {
        // Solid facets do not wobble with the clock. Their normals share the
        // colour pattern and fade to the geometric face when no longer resolved.
        let uv = enchantment_face_uv(pattern_position * scale / material_scale, pattern_normal);
        let facet = crystal_facets(texel_centre(uv));
        let resolved = surface_detail_weight(fp_m / material_scale * PIXEL_RES / CRYSTAL_FACET_TEXELS);
        crystal_facet = facet;
        crystal_resolved = resolved;
        let tangent = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(geometric_normal.x) > 0.5);
        let bitangent = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(geometric_normal.y) > 0.5);
        n = terrain_safe_unit3(geometric_normal + (tangent * (facet.x - 0.5)
            + bitangent * (facet.w - 0.75)) * 0.40 * resolved, geometric_normal);
        filtered_slope_variance += 0.013 * (1.0 - resolved * resolved);
    }
    if hedron {
        // Faceted skin on the texel grid: each shard tilts its own way, so the
        // interior refracts in discrete crystalline cells as the view turns.
        let facet = crystal_facets(texel_centre(enchantment_face_uv(in.voxel_pos * scale, geometric_normal)));
        let resolved = surface_detail_weight(fp_m * PIXEL_RES / CRYSTAL_FACET_TEXELS);
        crystal_facet = facet;
        crystal_resolved = resolved;
        let tangent = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(geometric_normal.x) > 0.5);
        let bitangent = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(geometric_normal.y) > 0.5);
        n = terrain_safe_unit3(geometric_normal + (tangent * (facet.x - 0.5)
            + bitangent * (facet.w - 0.75)) * 0.32 * resolved, geometric_normal);
        // The skin is thin and dark; the colour lives inside.
        albedo *= 0.30;
    }
    if cave_amount > 0.0 && !is_backdrop && !authored_finish && !is_viewmodel && kind != 8 && kind != 9 {
        let grime = surface_noise_filtered(
            pattern_pos.xz / 0.90 + vec2<f32>(pattern_pos.y * 0.27, -pattern_pos.y * 0.19),
            fp_m / 0.90,
        );
        albedo *= 1.0 - cave_amount * (0.10 + (grime - 0.5) * 0.10);
        // Damp mineral streaks catch torchlight on natural rock faces. Reuse
        // the existing grime field so the sheen and colour describe one patch.
        if kind == 3 || kind == 7 {
            let damp = smoothstep(0.46, 0.79, grime) * cave_amount
                * (1.0 - abs(geometric_normal.y) * 0.65);
            albedo *= mix(vec3<f32>(1.0), vec3<f32>(0.76, 0.88, 0.92), damp);
            roughness = mix(roughness, min(roughness, 0.46), damp);
        }
    }
    // Smoke and steam: billowed masses whose normal is settled here, so the
    // shared lighting brightens their tops and leaves the underside dim.
    var gas = GasPuff(n, 1.0, 1.0, 1.0);
    if kind == 43 || kind == 44 {
        gas = gas_puff(in.voxel_pos, geometric_normal, scale, light.torch_meta.y, fp_m);
        n = gas.normal;
        filtered_slope_variance += 0.004;
        let under = select(vec3<f32>(0.090, 0.095, 0.110), vec3<f32>(0.34, 0.39, 0.46), kind == 44);
        let top = select(vec3<f32>(0.30, 0.29, 0.27), vec3<f32>(0.78, 0.79, 0.80), kind == 44);
        // Industry gas keeps its species hue: the vertex colour relative to the
        // material's own base. Terrain smoke and steam carry that base, so
        // their ratio is 1 and they look exactly as before.
        let base_srgb = select(vec3<f32>(0.25, 0.26, 0.28), vec3<f32>(0.80, 0.85, 0.89), kind == 44);
        let base_lin = terrain_srgb_to_linear(base_srgb);
        let hue = (in.base_color / max(dot(in.base_color, vec3<f32>(0.2126, 0.7152, 0.0722)), 1e-4))
            / (base_lin / max(dot(base_lin, vec3<f32>(0.2126, 0.7152, 0.0722)), 1e-4));
        albedo = mix(under, top, smoothstep(0.15, 0.85, gas.shade)) * gas.density
            * clamp(hue, vec3<f32>(0.5), vec3<f32>(1.8));
    }
    // Ropy dark crust over narrow molten seams. Side faces advect downwards;
    // the metre-space pixel grid keeps a consistent scale across voxel sizes.
    var lava_emission = vec3<f32>(0.0);
    if kind == 10 {
        let acid = block == 206u; // SulfuricAcid, packed material ID.
        let t = light.torch_meta.y;
        let wall = abs(geometric_normal.y) < 0.5;
        let uv = natural_face_uv(pattern_pos, geometric_normal);
        let p = mix(uv, (floor(uv * PIXEL_RES) + 0.5) / PIXEL_RES, surface_detail_weight(fp_m * PIXEL_RES));
        let wall_speed = select(-0.26, -0.56, acid);
        let current = select(in.water_optics.zw, vec2<f32>(0.0, wall_speed), wall);
        let stretch = select(vec2<f32>(1.65, 2.5), vec2<f32>(3.4, 1.35), wall);
        let phase = fract(t * 0.25);
        let a = (p - current * (phase * 4.0)) * stretch;
        let b = (p - current * (fract(phase + 0.5) * 4.0)) * stretch;
        let weight = abs(phase * 2.0 - 1.0);
        let fold = mix(surface_noise_filtered(a * 0.55, fp_m * 1.9),
            surface_noise_filtered(b * 0.55, fp_m * 1.9), weight);
        let grain = mix(surface_noise_filtered(a * 3.4, fp_m * 11.6),
            surface_noise_filtered(b * 3.4, fp_m * 11.6), weight);
        let warp = vec2<f32>(fold * 1.45, 0.0);
        let seam = abs(mix(surface_noise_filtered(a + warp, fp_m * 3.4),
            surface_noise_filtered(b + warp, fp_m * 3.4), weight) - 0.5);
        let aa = clamp(fp_m * 0.8, 0.025, 0.15);
        // Filter seam coverage toward its average. Filtering the noise alone
        // converges to 0.5, which would turn distant lava entirely molten.
        let unresolved = smoothstep(0.08, 0.36, fp_m);
        let melt = mix(1.0 - smoothstep(0.018, 0.068 + aa, seam), 0.24, unresolved);
        let core = mix((1.0 - smoothstep(0.0, 0.033 + aa, seam)) * smoothstep(0.26, 0.67, grain),
            0.04, unresolved);
        let crust = mix(vec3<f32>(0.016, 0.008, 0.006), vec3<f32>(0.061, 0.019, 0.010), grain);
        if acid {
            let streak = mix(melt * 0.35, melt, clamp(length(current) * 1.8, 0.0, 1.0));
            let fizz = smoothstep(0.72, 0.87, grain) * (0.55 + 0.45 * sin(t * 3.8 + fold * 24.0));
            albedo = mix(vec3<f32>(0.095, 0.17, 0.023), vec3<f32>(0.44, 0.57, 0.08), streak * 0.8 + fizz * 0.2);
            roughness = mix(0.20, 0.37, fizz);
        } else {
            albedo = mix(crust, vec3<f32>(0.19, 0.035, 0.005), melt);
            roughness = mix(0.87, 0.34, melt);
            let heat_pulse = 0.94 + 0.06 * sin(t * 1.3 + fold * 9.0);
            lava_emission = vec3<f32>(0.028, 0.002, 0.0005) * grain
                + vec3<f32>(0.77, 0.085, 0.004) * melt * melt * heat_pulse
                + vec3<f32>(0.42, 0.19, 0.012) * core;
        }
    }
    if kind != 8 && kind != 9 && !is_viewmodel && !authored_finish {
        let micro = surface_noise_filtered(
            pattern_pos.xz / 0.22 + vec2<f32>(pattern_pos.y * 0.41, -pattern_pos.y * 0.33),
            fp_m / 0.22,
        );
        roughness = clamp(roughness * (0.86 + micro * 0.28), 0.08, 1.0);
    }

    var water_depth = 10.0;
    var water_foam = 0.0;
    // Emission of a realm glow pool (realm_light.wgsl), pre-exposure.
    var realm_water_emit = vec3<f32>(0.0);
    var water_jacobian = 1.0;
    // Walls of a pool or a flowing step get the same water body colour as the
    // surface, without waves or foam. They used to be a flat teal albedo.
    let water_side = resident_water && geometric_normal.y <= 0.5;
    if kind == 8 && (n.y > 0.5 || water_side) {
        let t = light.torch_meta.y;
        let fp = footprint * cam.world_params.x;
        // Water normals read continuous coordinates. Ground and bank foam
        // keep their authored pixel patterns.
        let flow_plane = select(pattern_pos.xz, vec2<f32>(dot(pattern_pos.xz,
            vec2<f32>(abs(geometric_normal.z), abs(geometric_normal.x))), pattern_pos.y), water_side);
        let p = flow_plane;
        let current = in.water_optics.zw;
        let current_speed = length(current);
        let current_strength = min(current_speed, 1.0);
        let stream = smoothstep(0.035, 0.38, current_speed);
        let surface_height = in.voxel_pos.y * scale;
        let marine = water_marine_weight(surface_height, scale) * (1.0 - stream);
        let current_detail = smoothstep(0.015, 0.30, current_speed)
            * (1.0 - smoothstep(0.16, 0.65, fp));
        var current_ripple = vec3<f32>(0.5, 0.0, 0.0);
        var current_gradient = vec2<f32>(0.0);
        if current_detail > 0.0 {
            // Two overlapping four-second phases avoid unbounded stretching
            // where neighbouring vertices carry different current velocities.
            // Sample upstream so the surface features travel downstream at
            // the encoded speed. Favor slopes across the current so the
            // reflected highlights read along the stream.
            // Ripples ride at twice the encoded current: at 1x, rivers read as
            // creeping even on steep reaches.
            let phase = fract(t * 0.25);
            let drift = current * 2.0;
            let a = surface_noise_grad((p - drift * (phase * 4.0)) * 1.35);
            let b = surface_noise_grad((p - drift * (fract(phase + 0.5) * 4.0)) * 1.35);
            current_ripple = mix(a, b, smoothstep(0.0, 1.0, abs(phase * 2.0 - 1.0)));
            let downstream = current / current_speed;
            let slope = current_ripple.yz;
            current_gradient = (slope - downstream * dot(slope, downstream) * 0.55) * 0.085;
        }

        // The far water lattice carries bed depth in [0,3], separate from
        // resident sky-access metadata [4,5].
        // Both owners carry source bed colour/depth. Filter the same refraction
        // field by distance; an irregular ownership edge must not switch it off.
        // This does not substitute for equal bed depths from the terrain source.
        let handoff_start = max(terrain_backdrop_start() - terrain_overlap_width() * 0.50, 0.0);
        let handoff_end = max(terrain_backdrop_start(), handoff_start + scale);
        let local_water = 1.0 - smoothstep(handoff_start, handoff_end, length(relative_pos.xz));
        let depth = max(surface_water_depth, 0.001);
        water_depth = depth;
        let contact = select(0.0, clamp(in.surface.w - 8.0, 0.0, 1.0), resident_water);

        // The same dispersive bands move the mesh and its reflections.
        // Smaller capillary detail fades into roughness as it becomes subpixel.
        let surface_wind = local_surface_wind();
        let wind = smoothstep(0.12, 0.65, ambient_sky)
            * smoothstep(0.0, 7.0, length(surface_wind));
        let wind_energy = wind * mix(0.50, 1.0, marine) * (1.0 - stream);
        let resolved = 1.0 - smoothstep(0.16, 1.40, fp);
        let wet_surface = smoothstep(0.01, 0.15, depth);
        let wave_xz = in.voxel_pos.xz * scale;
        var gradient = vec2<f32>(0.0);
        if !water_side && wet_surface > 0.0 && current_detail < 1.0 {
            let energy = wind_energy * wet_surface;
            if fp < 11.0 && energy > 0.0 {
                let packets = water_surface_packets(wave_xz, t, fp);
                gradient = packets.xy * energy;
                water_jacobian = 1.0 - clamp(packets.w * energy, -0.3, 0.3);
            }
            let fine_energy = mix(0.004, 0.030, energy) * wet_surface;
            filtered_slope_variance = 0.45 * fine_energy * fine_energy * (1.0 - resolved * resolved);
            if resolved > 0.0 {
                let a = water_wave_packet(wave_xz, vec2<f32>(0.60, 0.80), 1.9, 0.36, 2.4, t, fp);
                let b = water_wave_packet(wave_xz, vec2<f32>(-0.86, 0.51), 3.3, 0.19, 5.2, t, fp);
                gradient += (a.xy + b.xy) * fine_energy * resolved;
            }
        }
        var sea_whitecap = 0.0;
        let sea_open = smoothstep(1.0, 8.0, depth) * wind * marine;
        if !water_side && sea_open > 0.0 {
            let sea_xz = pattern_pos.xz;
            let gust_warp = surface_noise_filtered(sea_xz / 2300.0, fp / 2300.0);
            let gust = surface_noise_filtered(sea_xz / 700.0 + vec2<f32>(t * 0.0021, gust_warp * 1.7), fp / 700.0);
            // Stretch slicks downwind using the local weather heading.
            let along = surface_wind / max(length(surface_wind), 0.0001);
            let across = vec2<f32>(-along.y, along.x);
            let streak = surface_noise_filtered(vec2<f32>(dot(sea_xz, along) * 0.0037,
                dot(sea_xz, across) * 0.0148), fp * 0.0148);
            let rough_field = smoothstep(0.22, 0.78, gust * 0.72 + streak * 0.28);
            let sea_energy = sea_open * mix(0.35, 1.25, rough_field);
            let swell_xz = sea_xz + vec2<f32>(gust_warp - 0.5, gust - 0.5) * 46.0;
            let crest_bend = (surface_noise_filtered(sea_xz / 72.0, fp / 72.0) - 0.5) * 8.0;
            let short_swell_xz = swell_xz + vec2<f32>(crest_bend, -crest_bend * 0.6);
            let s0 = sea_swell_term(swell_xz, vec2<f32>(0.83, 0.56), 110.0, 0.035, t, fp);
            let s1 = sea_swell_term(swell_xz, vec2<f32>(0.47, -0.883), 62.0, 0.040, t, fp);
            let s2 = sea_swell_term(swell_xz, vec2<f32>(-0.56, 0.828), 34.0, 0.045, t, fp);
            let s3 = sea_swell_term(short_swell_xz, vec2<f32>(-0.22, -0.976), 19.0, 0.032, t, fp);
            gradient += (s0.xy + s1.xy + s2.xy + s3.xy) * sea_energy;
            // Cox-Munk wind-sea slope variance (~0.02 at 7 m/s) arrives as the
            // centimetre layers stop resolving, so the sun path widens to a glitter patch.
            let unresolved_wind = 0.012 * smoothstep(0.5, 6.0, fp);
            filtered_slope_variance += (s0.z + s1.z + s2.z + s3.z + unresolved_wind) * sea_energy;
            // Foam forms on crossing swell crests in rough patches. The patch
            // noise breaks up the fronts but does not move independently of them.
            let cap_resolved = 1.0 - smoothstep(0.8, 4.0, fp);
            var cap_shape = 0.0;
            if cap_resolved > 0.0 && s3.w > 0.51 && s2.w > 0.16 {
                let cap_noise = surface_noise_filtered(short_swell_xz / 7.0, fp / 7.0);
                cap_shape = smoothstep(0.51, 0.80, s3.w) * smoothstep(0.16, 0.57, s2.w)
                    * smoothstep(0.52, 0.80, cap_noise);
            }
            let cap_light = mix(0.12, 1.0, smoothstep(-0.12, 0.12, light.sun_dir_intensity.y));
            sea_whitecap = mix(0.008, cap_shape * 0.46, cap_resolved) * rough_field * rough_field * sea_open * cap_light;
        }
        if water_side && current_detail > 0.0 {
            let wall_tangent = vec3<f32>(abs(geometric_normal.z), 0.0, abs(geometric_normal.x));
            n = normalize(n + (wall_tangent * current_gradient.x
                + vec3<f32>(0.0, current_gradient.y, 0.0)) * current_detail);
        } else if !water_side {
            gradient = mix(gradient, current_gradient, current_detail);
            let wind_slope_weight = 1.0 - current_detail;
            filtered_slope_variance *= wind_slope_weight * wind_slope_weight;
            n = normalize(n + vec3<f32>(gradient.x, 0.0, gradient.y));
        }

        // Current and broad variation make lowland channels more sediment-rich
        // while still pools and springs stay clearer.
        let sediment = surface_noise_filtered(pattern_pos.xz / 340.0, fp / 340.0);
        let lowland = 1.0 - smoothstep(4.0, 40.0, surface_height - cam.coverage.y);
        let stream_sediment = mix(0.10 + sediment * 0.35, 0.48 + sediment * 0.48, lowland);
        let turbidity = mix(sediment * mix(0.48, 0.80, marine), stream_sediment, stream);
        // Resident and distant water share the same optical-depth palette.
        // The former resident-only cyan ramp changed colour with residency.
        var water_col = water_body_color(depth, turbidity, marine);
        // Sheltered freshwater reads blue/slate, not the sunlit mint of an
        // outdoor sandbank. This remains reflectance, never cave emission.
        let cave_water = mix(vec3<f32>(0.025, 0.065, 0.105), vec3<f32>(0.008, 0.025, 0.052),
            1.0 - exp(-depth * 0.45));
        water_col = mix(water_col, cave_water, cave_amount);

        // Keep the bed visible in clear shallows. Beyond them, water colour
        // carries the depth gradient. This fades both coarse bed tints and the
        // fallback colour used when the bed lies beyond a resident chunk.
        let bed_visibility = (1.0 - smoothstep(0.8, 6.0, fp))
            * (1.0 - smoothstep(4.0, 12.0, depth)) * (1.0 - turbidity * 0.40);
        if bed_visibility > 0.0 {
            // The mesh supplies the authoritative sampled substrate colour.
            // Do not paint imaginary ripples/minerals over an edited bottom;
            // caustics are light on that bed, not pigment.
            let v_inc = terrain_safe_unit3(relative_pos, vec3<f32>(0.0, -1.0, 0.0));
            // Snell's law at the mean horizontal surface. Keeping the
            // texel-stepped wave normal out of absorption smooths the bed tint.
            // With eta = 0.75, the squared refracted cosine is 1 - eta² sin²(i).
            var view_path = inverseSqrt(0.4375 + 0.5625 * v_inc.y * v_inc.y);
            var bed = albedo;
            var bed_xz = pattern_pos.xz;
            if water_side {
                let wall_tangent = vec3<f32>(-n.z, 0.0, n.x);
                let wall_normal = normalize(n + wall_tangent * (gradient.x * 1.6)
                    + vec3<f32>(0.0, gradient.y * 1.6, 0.0));
                let wall_ray = refract(v_inc, wall_normal, 0.75);
                view_path = 1.0 / max(-wall_ray.y, 0.22);
            } else {
                let ray = refract(v_inc, n, 0.75);
                bed_xz += ray.xz * (depth / max(-ray.y, 0.22));
                // Suppress the colour warp near banks and sharp depth steps.
                let clearance = smoothstep(0.04, 0.22, depth) * (1.0 - smoothstep(0.25, 0.85, contact))
                    * (1.0 - smoothstep(0.04, 0.25, water_depth_change));
                if clearance > 0.0 {
                    let flat_offset = v_inc.xz * (0.75 * depth * view_path);
                    let ripple_offset = (bed_xz - pattern_pos.xz) - flat_offset;
                    bed = water_refract_bed(bed, ripple_offset * clearance, water_chart_dx,
                        water_chart_dy, water_bed_dx, water_bed_dy);
                }
            }
            let sun_dir = terrain_safe_unit3(light.sun_dir_intensity.xyz, vec3<f32>(0.0, 1.0, 0.0));
            let caustic_gain = smoothstep(0.05, 0.30, sun_dir.y) * smoothstep(0.03, 0.16, light.sun_dir_intensity.w)
                * ambient_sky * (1.0 - turbidity) * select(1.0, 0.0, water_side);
            var caustic = 0.0;
            if caustic_gain > 0.0 && (!CAVE_WATER_FAST || caustic_gain > 0.001) {
                let sun_ray = uw_sun_travel(sun_dir);
                let light_xz = bed_xz - sun_ray.xz * (depth / max(-sun_ray.y, 0.22));
                caustic = water_bed_caustic(light_xz, t, depth, fp_m) * caustic_gain;
            }
            // A submerged bed is wet, so darker than the same block on land.
            let bed_luma = dot(bed, vec3<f32>(0.2126, 0.7152, 0.0722));
            let sheltered_bed = mix(bed, bed_luma * vec3<f32>(0.72, 0.88, 1.0), cave_amount * 0.70);
            let refracted_bed = sheltered_bed * 0.70 * (1.0 + caustic);

            // Beer-Lambert along the refracted view ray plus the solar path down.
            let optical_path = depth * (view_path + 0.45 / max(light.sun_dir_intensity.y + 0.28, 0.25));
            // Broad suspended sediment changes the depth of visible bed detail
            // as well as the open-water hue. Reuse the continuous turbidity.
            let transmission = exp(-WATER_ABSORB * (optical_path * (0.85 + 0.40 * turbidity)));
            water_col = mix(water_col, water_col * (vec3<f32>(1.0) - transmission) + refracted_bed * transmission, bed_visibility);
        }

        // Foam is lace where the surface meets a solid bank or boulder.
        // Depth contours alone are not contact. fract(voxel_pos.y) is not a
        // waterline: it repeated a bright meniscus at every cell boundary.
        water_foam = 0.0;
        if resident_water && !water_side {
            // A rain film a few centimetres deep has no surf to throw foam.
            // Compression already follows the travelling wave crests. Let
            // those crests lift the foam before the lace ebbs back.
            let wash = clamp(1.0 + (1.0 - water_jacobian) * 1.6, 0.64, 1.28);
            // Stream current can drive a thin ribbon of foam against a bank
            // even where the wind spectrum is weak.
            let current_wash = current_strength * (0.28 + (current_ripple.x - 0.5) * 0.22);
            let bank_energy = max(wind * wash, current_wash);
            let foam_energy = local_water * smoothstep(0.03, 0.10, depth) * bank_energy;
            if foam_energy > 0.0 {
                water_foam = clamp(water_shore_foam(pattern_pos.xz, contact, current, t, fp_m)
                    * foam_energy, 0.0, 1.0);
            }
        }
        // A sloped shallow bed supports a travelling breaker front. Advancing
        // the phase moves its crest toward lower depths, following the coast
        // through the resident and horizon meshes alike.
        if !water_side && wind_energy > 0.0 && depth < 1.50 {
            let slope = water_depth_change / max(fp_m, 0.002);
            let shore = max(contact, smoothstep(0.035, 0.18, slope));
            let surf_zone = water_breaker_band(depth) * shore * wind_energy
                * (1.0 - smoothstep(0.35, 1.8, water_depth_change));
            if surf_zone > 0.0 {
                let breaker = water_breaker(wave_xz, depth, t);
                let phase = breaker.x;
                // Skip both texture scales between waves. Once the crest
                // passes, the foam widens and thins in place behind it.
                if phase > 0.34 && phase < 0.98 {
                    let age = smoothstep(0.52, 0.90, phase);
                    let patch_field = surface_noise_filtered(p * 0.43, fp * 0.43);
                    let spread = smoothstep(0.46 - age * 0.12, 0.70 - age * 0.10, patch_field);
                    let arrival = smoothstep(0.34 + (1.0 - patch_field) * 0.10, 0.53, phase);
                    let decay = 1.0 - smoothstep(0.56, 0.98, phase);
                    let patch_foam = spread * arrival * decay * breaker.y;
                    if patch_foam > 0.0 {
                        let q = p - current * (age * 0.55);
                        let lace = surface_noise_filtered(q * 2.8 + vec2<f32>(age * 0.21, -age * 0.13), fp * 2.8);
                        let holes = smoothstep(0.26 + age * 0.22, 0.64 + age * 0.16, lace);
                        let foam = patch_foam * mix(0.24, 0.90, holes);
                        water_foam = max(water_foam, foam * surf_zone);
                    }
                }
            }
        }
        if water_side && current.y < -0.1 {
            let aeration = smoothstep(0.40, 0.78, current_ripple.x);
            water_foam = aeration * current_detail * 0.36;
        }
        water_foam = max(water_foam, sea_whitecap);
        if realm_rig_on() && !water_side {
            // A realm glow pool lights its own bed; `albedo` is still the
            // substrate here. The body keeps a little reflectance for lamp glints.
            realm_water_emit = realm_water_glow(world_pos, depth, albedo,
                terrain_safe_unit3(relative_pos, vec3<f32>(0.0, -1.0, 0.0)), n, t, fp_m);
            if dot(realm_water_emit, realm_water_emit) > 0.0 {
                water_col = water_col * 0.25;
            }
        }
        albedo = mix(water_col, WATER_FOAM, water_foam);
        let water_roughness = 0.045 + 0.025 * marine + 0.025 * stream;
        roughness = mix(select(water_roughness, material_tint.w, material_tint.w >= 0.0), 0.48, water_foam);
    }
    albedo *= material_tint.rgb;
    // Wet films accumulate/dry in real time, with cached roof visibility.
    let exposure_to_rain=smoothstep(0.2,0.85,sky_access)*smoothstep(-0.15,0.65,geometric_normal.y);
    let wet=clamp(light.surface_weather.x*material_props.w*exposure_to_rain,0.0,1.0)
        *select(1.0,0.0,kind==8 || kind==9 || kind==10 || is_torch_material || kind>=40);
    if wet > 0.0 {
        let wet_patch=0.75+0.25*surface_noise_filtered(pattern_pos.xz*0.32,fp_m*0.32);
        albedo*=1.0-wet*wet_patch*0.32;
        roughness=mix(roughness,min(roughness,0.20),wet*0.80);
        reflectance=mix(reflectance,max(reflectance,0.55),wet);
    }

    var animated_emission = vec3<f32>(0.0);
    var hedron_glow = vec3<f32>(0.0);
    if hedron {
        let p_m = in.voxel_pos * scale;
        // Snap the entry point to the face's texel centre (the normal axis keeps
        // its exact plane), so the inner depth reads as one pixel per texel.
        let snapped = (floor(p_m * PIXEL_RES) + vec3<f32>(0.5)) / PIXEL_RES;
        let entry_m = select(snapped, p_m, abs(geometric_normal) > vec3<f32>(0.5));
        let spine = in.surface.xy + (p_m.xz - entry_m.xz);
        let entry = vec3<f32>(-spine.x, entry_m.y, -spine.y);
        let view = terrain_safe_unit3(relative_pos + (entry_m - p_m), -geometric_normal);
        let facing = select(n, -n, dot(n, view) > 0.0);
        var ray = refract(view, facing, 1.0 / 1.55);
        if dot(ray, ray) < 0.5 { ray = view; }
        let fresnel = 0.046 + 0.954 * terrain_pow5(1.0 - clamp(dot(-view, facing), 0.0, 1.0));
        let glow = in.surface.w - 2.0;
        let tint = clamp(in.base_color, vec3<f32>(0.0), vec3<f32>(1.0));
        let inner = hedron_interior(entry, ray, tint, light.torch_meta.y, crystal_resolved);
        // Cut edges catch a thin internal reflection, brightest at grazing view.
        let edge = crystal_facet.z * crystal_resolved * (0.15 + 0.6 * fresnel);
        hedron_glow = (inner * (1.0 - fresnel) + mix(tint, vec3<f32>(1.0), 0.6) * edge * 0.08)
            * (0.55 + glow * 1.6);
    } else if kind == 56 {
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
    if kind == 6 && terrain_has_resident_metadata(in.ao_strength) {
        // Let leaf lighting flutter while the solid crown's edges stay sealed.
        let wind = capped_surface_wind(24.0);
        let phase = dot(pattern_pos, vec3<f32>(0.55, 0.31, 0.74));
        let flutter = sin(light.torch_meta.y * 2.1 - phase) * 0.009
            * (1.0 - smoothstep(0.08, 0.45, fp_m));
        n = normalize(n + vec3<f32>(wind.x, 0.0, wind.y) * flutter);
    }
    let normal_dx = dpdx(n);
    let normal_dy = dpdy(n);
    // Evaluate outside material branches and before any fragment discard.
    let spell_current_footprint = abs(dpdx(in.surface.y)) + abs(dpdy(in.surface.y));
    let horizontal_dist = length(relative_pos.xz);
    // Main and diagnostic views must show the same owner. Keep this after all
    // derivative evaluations, but before any material/normal/depth debug return.
    // Planet cover yields to resident voxels at every altitude; a high-altitude
    // gate let the coarse cover show through streamed terrain.
    if TERRAIN_BACKSTOP {
        if !terrain_is_backdrop(in.ao_strength) || (in.surface.z != 0.0 && kind != 8) {
            discard;
        }
        // Planet fallback repairs only the footprint replaced by residents.
        // Its original parent must not cover the adjusted unowned collar strip.
        if cam.planet[0].w > 0.5
            && !terrain_resident_column_owned(world_pos.xz, horizontal_dist) {
            discard;
        }
        // Only fill pixels left clear by the protected opaque geometry. Preserve
        // real depth so the nearest parent wins and clouds stop at its surface.
        if textureLoad(terrain_backstop_depth, vec2<i32>(in.pos.xy), 0) > 0.0 {
            discard;
        }
    }
    if in.ao_strength < 0.0 && !TERRAIN_BACKSTOP {
        // Exact edited proxies still yield to the precise owner. Only the
        // procedural parent retains an opaque perimeter beneath replacement.
        var hidden = terrain_resident_column_owned(world_pos.xz, horizontal_dist);
        if hidden {
            let phase = select(terrain_arrival_phase(world_pos.xz, scale), 0u, portal_view());
            let opaque_backdrop = in.surface.z == 0.0 && terrain_is_backdrop(in.ao_strength);
            if phase != 0u {
                // The parent and resident surfaces can project to different pixels
                // or columns on slopes. Keep the ground filled as resident terrain fades in.
                hidden = !opaque_backdrop && terrain_dither(vec2<u32>(in.pos.xy)) < phase - 1u;
            } else if opaque_backdrop {
                // Keep an opaque terrain skirt under the resident edge. The
                // two water sheets share an elevation and need a single owner
                // after the complementary arrival fade, or both beds can show.
                hidden = terrain_backdrop_column_hidden(world_pos.xz, scale);
            }
        }
        if hidden { discard; }
    }
    let normal_variance = 0.5 * (dot(normal_dx, normal_dx) + dot(normal_dy, normal_dy)) + filtered_slope_variance;
    var fire_heat = 0.0;
    if kind == 42 {
        // Pixel fire: every 5 cm texel of the face carries its own heat, from
        // tongues scrolled up the face and a 10 Hz crackle, both pure functions
        // of world position and the clock. Cool texels are empty, so the
        // silhouette is a pixel edge that licks upward.
        fire_heat = pixel_fire_heat(in.voxel_pos, geometric_normal, scale, light.torch_meta.y, fp_m);
        if fire_heat < 0.20 { discard; }
    }
    if (kind == 43 || kind == 44) && gas.coverage < 0.5 { discard; }
    // A cable thinner than a pixel still writes depth, so the cloud composite
    // clipped the deck behind it and drew a clear-sky streak to the zenith.
    // Below a quarter-pixel of coverage it yields the pixel to sky and clouds;
    // a fixed screen dither ramps the hand-over instead of popping.
    if kind == 61 {
        let ign = fract(52.982918 * fract(dot(in.pos.xy, vec2<f32>(0.06711056, 0.00583715))));
        if clamp(in.surface.w, 0.0, 1.0) * 4.0 < ign { discard; }
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
    if kind == 12 && block == BLOCK_WINDOW_GLASS { roughness = 0.10; reflectance = 0.55; }
    roughness = clamp(roughness * surface_finish.x, 0.06, 1.0);
    reflectance = clamp(reflectance * surface_finish.y, 0.0, 1.0);
    roughness = terrain_filter_roughness(roughness, normal_variance);
    // Distant water uses the same finish as resident water. There is no blanket
    // backdrop roughness floor or extra (1-r)^2 specular suppression.
    let generic_metallic = max(select(select(mineral_metallic, 1.0, plated_block), 0.72, enemy_metal),
        select(0.0, gilded_metal * min(detail_strength, 1.0), gilded_block));
    var metallic = select(generic_metallic,
        select(select(0.0, 0.82, in.surface.w > 0.25), 0.35, in.surface.w > 0.75), orbital_part);
    if authored_kit { metallic = select(0.0, 0.78, kit_metal); }
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
    if debug_mode == 32 {
        // Texture-free massing diagnostic, fixed broad light and geometric
        // normals. Deliberately independent of materials and scene lighting.
        let key = max(dot(geometric_normal, normalize(vec3<f32>(0.4, 0.8, -0.3))), 0.0);
        let clay = (0.22 + 0.42 * key) * (0.75 + 0.25 * terrain_vertex_ao(in.ao_strength));
        return debug_surface_output(vec4<f32>(vec3<f32>(clay), 1.0));
    }
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
    // Ambient sky visibility; blue where only the column term applies.
    if debug_mode == 36 {
        return debug_surface_output(vec4<f32>(vec3<f32>(ambient_sky)
            * mix(vec3<f32>(0.55, 0.70, 1.0), vec3<f32>(1.0), local_sky.weight), 1.0));
    }
    if debug_mode == 30 {
        if TERRAIN_BACKSTOP { return debug_surface_output(vec4<f32>(0.9, 0.0, 0.9, 1.0)); }
        let color = select(vec3<f32>(0.12, 0.75, 0.2), vec3<f32>(0.12, 0.3, 0.9), is_backdrop);
        return debug_surface_output(vec4<f32>(select(color, vec3<f32>(0.9, 0.4, 0.1), in.ao_strength < -1.5), 1.0));
    }
    if debug_mode == 37 {
        // Actual rendered eye-to-surface distance, shared by resident and parent
        // draws. Chart coordinates would conceal depth jumps on a curved planet.
        let distance_m = length(relative_pos);
        let band = u32(distance_m >= 128.0) + u32(distance_m >= 256.0)
            + u32(distance_m >= 512.0) + u32(distance_m >= 1024.0)
            + u32(distance_m >= 2048.0) + u32(distance_m >= 4096.0);
        let colors = array<vec3<f32>, 7>(
            vec3<f32>(0.9, 0.0, 0.0), vec3<f32>(0.9, 0.3, 0.0),
            vec3<f32>(0.9, 0.9, 0.0), vec3<f32>(0.0, 0.9, 0.0),
            vec3<f32>(0.0, 0.9, 0.9), vec3<f32>(0.0, 0.0, 0.9),
            vec3<f32>(0.9, 0.0, 0.9));
        return debug_surface_output(vec4<f32>(colors[band], 1.0));
    }
    if debug_mode == 19 {
        // Known families are 0..38, 40..44, 47..56. Unsupported keys are magenta.
        let supported = kind <= 38 || (kind >= 40 && kind <= 68);
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
    let sun_spectral  = sky_probe_term(SKY_PROBE_SUN_TRANSMITTANCE);
    let moon_spectral = sky_probe_term(SKY_PROBE_MOON_TRANSMITTANCE);

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
    let foliage_sun_n = saturate1((dot(n, sun_dir) + 0.08) / 1.08);
    let foliage_moon_n = saturate1((dot(n, moon_dir) + 0.08) / 1.08);
    let foliage_sun = foliage_sun_n * mix(0.50, 1.0, smoothstep(0.16, 0.72, foliage_sun_n));
    let foliage_moon = foliage_moon_n * mix(0.50, 1.0, smoothstep(0.16, 0.72, foliage_moon_n));

    // PCF shadow
    let sun_shadow_strength = 0.94 * smoothstep(0.03, 0.16, light.sun_dir_intensity.w);
    var shadow_visibility = 1.0;
    var cascade_visibility = 1.0;
    var range_visibility = 1.0;
    if cam.scene_seed.w == 0u && (sun_shadow_strength > 0.0 || debug_mode == 11 || debug_mode == 33) {
        // Real resident, exact fallback, and procedural receivers share CSM
        // wherever it has coverage; N.L already handles unlit slope orientations.
        // Origin-frame receiver: eye_from_origin plus relative_pos is exact
        // near the eye, where absolute f32 metres lose centimetres far from spawn.
        shadow_visibility = sample_sun_shadow(cam.eye_from_origin.xyz + relative_pos, linear_depth,
            clamp(dot(geometric_normal, sun_dir), 0.0, 1.0), world_dx, world_dy);
        cascade_visibility = shadow_visibility;
        // Distant ranges as casters: the cascades end within a few hundred
        // metres, the detail map reaches the far plane. Skipped where the
        // surface already faces away from the sun or has no sky, and where the
        // cascades already give full shadow, unless view 33 shows both terms.
        if sun_shadow_strength > 0.0 && !is_viewmodel && sky_access > 0.05
            && dot(macro_normal, sun_dir) > 0.0
            && (shadow_visibility > 0.0 || debug_mode == 33) {
            range_visibility = horizon_detail_sun_visibility(world_pos, sun_dir);
            shadow_visibility = min(shadow_visibility, range_visibility);
        }
    }

    if debug_mode == 11 { return debug_surface_output(vec4<f32>(vec3<f32>(shadow_visibility), 1.0)); }
    // Red: lit by cascades only. Green: lit by distant ranges only. Yellow:
    // both agree lit; black: both shadowed; pure red or green: they disagree.
    if debug_mode == 33 {
        return debug_surface_output(vec4<f32>(cascade_visibility, range_visibility, 0.0, 1.0));
    }

    let sun_shadow = mix(1.0, shadow_visibility, mix(sun_shadow_strength, 1.0, cave_amount));

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
    let sky_up      = sky_probe_term(SKY_PROBE_ZENITH);
    // Diffuse illumination belongs to the world, not the camera. Key the
    // horizon sample to the sun azimuth so a pan cannot relight a hillside.
    // Sky irradiance approximation: radiance of the horizon band the face
    // looks at. A face turned to the sun sees the warm solar sky; one turned
    // away sees the cooler anti-solar sky. Sampling only the solar side gave
    // every shaded face the same warm fill and erased the light direction.
    let sun_az = safe_normalize2(sun_dir.xz, vec2<f32>(1.0, 0.0));
    let face_toward_sun = saturate1(dot(n.xz, sun_az) * 0.5 + 0.5);
    let solar_horizon = sky_probe_term(SKY_PROBE_SOLAR_HORIZON);
    let antisolar_horizon = sky_probe_term(SKY_PROBE_ANTISOLAR_HORIZON);
    let sky_horizon = mix(antisolar_horizon, solar_horizon, face_toward_sun);
    let clear_sky_ambient = mix(sky_horizon * 0.78, sky_up, pow(hemi, 0.72));
    let overcast_sky_ambient = mix(
        sky_horizon * 0.62,
        sky_horizon * 0.54 + sky_up * 0.46,
        pow(hemi, 0.85),
    );
    let sky_ambient = mix(clear_sky_ambient, overcast_sky_ambient, cloudiness * 0.65);
    let twilight_fill = 1.0 - smoothstep(-0.10, 0.22, sun_dir.y);
    let low_solar_sky = sky_probe_term(SKY_PROBE_LOW_SOLAR);
    let low_horizon_sky = mix(antisolar_horizon, low_solar_sky, face_toward_sun);
    let terrain_sky_ambient = mix(sky_ambient, mix(sky_ambient, low_horizon_sky, 0.38), twilight_fill);
    // Dense clouds remove the direct sun, but should replace it with a broad
    // bright sky dome rather than turning the terrain nearly black. Keep the
    // clear-day level restrained and let the local cloud field lift overcast.
    // Direct/ambient captures showed the clear-day fill reproducing almost
    // the whole scene by itself. Keep overcast and twilight illumination, but
    // let a visible sun establish the primary value structure.
    // 0.14 left canopy shade and trunks near black against the reference
    // paintings, whose shadows stay lifted and sky-blue on clear days.
    let sky_fill_strength = mix(mix(0.24, 0.95, cloudiness * daylight), 0.66, twilight_fill);
    let clear_day = daylight * (1.0 - twilight_fill) * (1.0 - cloudiness);
    // Clear-day shade read dark teal: the raw zenith probe is saturated blue
    // and was the only fill. Real shade also receives sunlit-ground and
    // foliage bounce, so pull part of the fill toward a warm neutral of the
    // same luminance. Twilight and overcast keep the cool sky colour.
    let fill_luma = dot(terrain_sky_ambient, vec3<f32>(0.2126, 0.7152, 0.0722));
    let warm_neutral_fill = vec3<f32>(fill_luma) * vec3<f32>(1.06, 1.02, 0.90);
    let balanced_sky_ambient = mix(terrain_sky_ambient, warm_neutral_fill, clear_day * 0.45);

    // A low sun still leaves a broad cool hemisphere above shaded ground.
    // Sampling only the bright solar horizon made that fill too warm while
    // the terrain night grade crushed the opposite-facing canopy. This is
    // world-oriented, restricted to twilight, and still gated by sky access.
    let dusk_window = smoothstep(-0.16, -0.02, sun_dir.y)
        * (1.0 - smoothstep(0.10, 0.28, sun_dir.y));
    var dusk_sky_fill = vec3<f32>(0.0);
    if dusk_window > 0.001 {
        let opposite_sky = sky_probe_term(SKY_PROBE_OPPOSITE_SUN);
        let cool_environment = mix(opposite_sky, sky_up, 0.50);
        // The authored sky irradiance is a bounded fallback for an extremely
        // dim LUT at blue hour; it is not emissive material light.
        let sky_floor = light.ambient_exposure.rgb * vec3<f32>(0.65, 0.85, 1.15);
        dusk_sky_fill = min(max(cool_environment, sky_floor), vec3<f32>(0.32))
            * dusk_window * (0.55 + 0.45 * hemi) * 0.65;
    }

    // Ground bounce contribution for downward-facing surfaces
    let ground_irr  = light.sun_color_daylight.rgb * sun_spectral
                    * saturate1(sun_dir.y) * light.sun_dir_intensity.w * 0.05 * sun_trans;
    let ground_col  = mix(vec3<f32>(0.018, 0.025, 0.042), ground_irr, daylight);
    // Side faces see sunlit ground too (half the lower hemisphere), not only
    // downward faces; green-tinted for the dominant grass/moss floor.
    let ground_amt  = saturate1(1.0 - hemi) * 0.56 + clear_day * 0.10;
    let ground_tint = ground_col * mix(vec3<f32>(1.0), vec3<f32>(0.92, 1.08, 0.80), clear_day);
    // Strengthen local cavity & canopy AO so tree trunks under foliage,
    // voxel ledges, and rock crevices have rich three-dimensional depth.
    let raw_ao = terrain_vertex_ao(in.ao_strength);
    let shaped_ao   = pow(raw_ao, 1.35);
    // Leaf clusters transmit some ambient sky light through their canopy.
    // Keep rock/soil contact shading and enclosed canopy shadows intact.
    let cavity_floor = mix(0.42, 0.60, f32(foliage) * ambient_sky);
    // Occlusion is contact detail. Through ground fog and the far veil the
    // in-scattered air in front of a slope is lit evenly, so cavity and macro
    // occlusion must fade with the same path the fog stage integrates; kept at
    // full strength they printed dark blotches on fogged ranges. This mirrors
    // the fog optical depth below without its height integral (an upper bound,
    // which only fades occlusion sooner) and leaves cave interiors untouched.
    let ao_cave = clamp(light.sky_art[1].w, 0.0, 1.0);
    let ao_fog_start = max(light.fog_params.x, 0.0);
    var ao_fog_range = max(light.fog_params.y - ao_fog_start, 1.0);
    let ao_planet_air = select(0.0, (1.0 - ao_cave)
        * (1.0 - smoothstep(1500.0, 8000.0, max(cam.planet[1].w, 0.0))),
        cam.planet[0].w > 0.5 && cam.planet[0].w < 1.5);
    ao_fog_range = mix(ao_fog_range, max(ao_fog_range, 6000.0 - ao_fog_start),
        ao_planet_air * daylight * (1.0 - smoothstep(0.15, 0.65, heavy_cloud)));
    let ao_fog_density = mix(0.22, 0.10, daylight) + heavy_cloud * 0.10;
    let ao_fog_depth = (1.35 + ao_fog_density * 3.2)
        * max(dist - ao_fog_start, 0.0) / ao_fog_range;
    let ao_veil = (1.0 - exp(-max(dist - 65.0, 0.0) / 1850.0)) * 0.78;
    let ao_keep = mix(exp(-clamp(ao_fog_depth, 0.0, 80.0)) * (1.0 - ao_veil), 1.0, ao_cave);
    let cavity      = mix(1.0, mix(mix(cavity_floor, 1.0, shaped_ao), 1.0, hemi * 0.08), ao_keep);
    let fog_macro_occlusion = mix(1.0, macro_occlusion, ao_keep);
    let ambient     = (
        light.ambient_exposure.rgb * mix(vec3<f32>(0.38), vec3<f32>(1.0), hemi * hemi)
            * mix(1.0, 0.55, clear_day)
        + mix(balanced_sky_ambient,
              vec3<f32>(fill_luma),
              cloudiness * 0.18) * sky_fill_strength
        + ground_tint * ground_amt + dusk_sky_fill
    ) * cavity * fog_macro_occlusion * mix(1.0, 0.86, cloudiness * daylight * 0.55);
    // A small non-sky fill keeps deep-cave silhouettes readable without making
    // the underground look like an overlit interior. It follows the cavity and
    // face hemisphere so micro-relief still has shape.
    // Cave fill is deliberately not one flat grey lift. Deep rock receives a
    // cool green-blue bounce, upward faces keep more of it, and downward faces
    // retain a small warm floor bounce. This makes tunnels read as carved
    // spaces with a ceiling and floor rather than an evenly exposed interior.
    let cave_depth = 1.0 - smoothstep(0.0, 0.48, ambient_sky);
    // Openings borrow a little cool daylight on sheltered faces. It falls off
    // with the sky probe, leaving the deeper room to local lamps and cave fill.
    let opening_bounce = ambient_sky * cave_amount * daylight;
    var cave_fill = (
        vec3<f32>(0.019, 0.024, 0.033)
        + vec3<f32>(0.013, 0.018, 0.015) * cave_depth
        + vec3<f32>(0.008, 0.006, 0.0035) * (1.0 - hemi) * 0.7
    ) * cave_amount * (0.35 + 0.65 * cavity) * (0.62 + 0.38 * hemi)
        * select(1.0, 0.0, kind == 9);
    cave_fill += vec3<f32>(0.025, 0.056, 0.085) * opening_bounce * cavity
        * select(1.0, 0.0, kind == 9);

    // --- Explicit ember / flame emission ---
    // Fire retains its luminance when the environment is darkened for night.
    // Cancel both the shared exposure and the terrain-only night grade below.
    let fire_exposure = 0.9 / exposure;
    // The Manyfold has no sky. A linear add is crushed when resolve exposure
    // drops, so this fill is pre-divided by exposure, same as the lamps.
    // w > 2 is the shell axis in metres. Lamps remain separate sources.
    if light.fog_color_density.w > 0.5 {
        let axis_y = select(36.0, light.fog_color_density.w, light.fog_color_density.w > 2.0);
        let inward = vec3<f32>(-world_pos.x, axis_y - world_pos.y, 0.0);
        let radial_up = inward / max(length(inward), 0.001);
        let facing_hollow = max(dot(n, radial_up), 0.0);
        cave_fill += vec3<f32>(0.22, 0.28, 0.40) * fire_exposure
            * (0.78 + 0.22 * facing_hollow) * (0.70 + 0.30 * cavity)
            * select(0.0, 1.0, kind != 8 && kind != 9);
    }
    if realm_rig_on() && kind != 8 && kind != 9 && !is_viewmodel {
        cave_fill += realm_surface_light(world_pos, n, light.torch_meta.y, fp_m, cavity, hemi) * fire_exposure;
    }
    var emissive = animated_emission + lava_emission * fire_exposure
        + realm_water_emit * fire_exposure
        + hedron_glow * mix(1.0, fire_exposure, 0.35)
        + select(vec3<f32>(0.0), gilded_glow * fire_exposure, gilded_block)
        + select(vec3<f32>(0.0), hell_glow * fire_exposure * min(detail_strength, 1.0), hell_block);
    // Keep the authored body albedo. Only the moving ribbons emit, in the
    // selected spell's colour carried by the wand mesh, never a universal hue.
    if (kind == 55 && !authored_kit) || kind == 22 || kind == 51 {
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
    if pixel_kit && kit_glass {
        // The selected spell drives colour; release energy lights only the core.
        // The authored pixel clusters stay fixed through the cast and camera motion.
        let release_energy = clamp((in.surface.w - 0.60) / 0.35, 0.0, 1.0);
        emissive += in.base_color * release_energy * (0.6 + albedo * 1.8);
    } else if authored_kit && kind == 55 && in.surface.w > 0.7 {
        emissive += albedo * 0.16;
    }
    if kind == 42 {
        // HDR so bloom carries the core; the edge stays a dim red so flame
        // pixels keep contrast against each other.
        let heat2 = fire_heat * fire_heat;
        emissive += pixel_fire_ramp(fire_heat) * (1.6 + 4.2 * heat2) * fire_exposure;
        albedo = mix(vec3<f32>(0.16, 0.028, 0.004), vec3<f32>(0.95, 0.36, 0.05), heat2);
    }
    // A faint self-glow keeps gas readable at night; the lit top comes from the sun.
    if kind == 43 { emissive += albedo * 0.05 * fire_exposure; }
    if kind == 44 { emissive += albedo * 0.10 * fire_exposure; }
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
        // Plasma glow (after Mycopunk's PlasmaBullet / Inner Glow look): faces
        // toward the eye run white-hot, the silhouette keeps the full spell
        // hue. The core only lifts the non-peak channels toward the peak, so
        // the brightest channel (and the bloom threshold test) barely moves.
        let facing = clamp(abs(dot(n, to_eye)), 0.0, 1.0);
        let rim = 1.0 - facing;
        let translucent = in.surface.x < 0.0;
        let glow = max(in.surface.w, 0.0);
        let peak = max(albedo.r, max(albedo.g, albedo.b));
        let hue = albedo / max(peak, 1e-4);
        let core_t = smoothstep(0.55, 0.98, facing) * select(0.35, 0.60, translucent);
        let core = mix(albedo, vec3<f32>(peak) * mix(hue, vec3<f32>(1.0), 0.75), core_t);
        // A slow plasma breath and rare fine flicker on translucent pieces;
        // both are world-anchored and clock-driven, never frame-driven.
        let t = light.torch_meta.y;
        let seed = dot(floor(pattern_pos * 6.0), vec3<f32>(0.131, 0.217, 0.173));
        let flicker = select(1.0,
            0.93 + 0.07 * sin(t * 9.0 + seed * 40.0) + 0.04 * sin(t * 23.0 + seed * 17.0),
            translucent && in.surface.y >= 0.0);
        let rim_gain = select(1.0 + 0.18 * rim, 1.0 + 0.55 * rim * rim, translucent);
        var current = 1.0;
        var emission_color = core;
        if in.surface.y <= -2.0 {
            // Local arc length comes from the spell mesh, not the world grid.
            // A stable luminous body remains between the travelling pulses.
            let along = max(-in.surface.y - 2.0, 0.0);
            let phase = fract(along * 1.35 - t * 3.6);
            let footprint = spell_current_footprint * 1.35;
            let edge = clamp(footprint * 0.5, 0.0, 0.12);
            let pulse = smoothstep(0.54 - edge, 0.67 + edge, phase)
                * (1.0 - smoothstep(0.76 - edge, 0.90 + edge, phase));
            let resolved = 1.0 - smoothstep(0.10, 0.52, footprint);
            current = mix(0.93, 0.72 + pulse * 0.95, resolved);
        } else if in.surface.y < -1.0 {
            // Shards carry a stable heat gradient along the piece itself.
            // A narrow pale tip leaves enough coloured body to read the spell.
            let heat = smoothstep(0.15, 1.0, (-in.surface.y - 1.1) / 0.8);
            emission_color = mix(albedo, vec3<f32>(peak), 0.53 * heat * heat);
            current = mix(0.50, 1.30, heat);
        }
        emissive += emission_color * rim_gain * flicker * glow * current * mix(1.0, fire_exposure, 0.35);
        if in.surface.y < 0.0 && in.surface.y >= -1.0 {
            // Only surviving microvoxels at the particle's erosion front
            // carry this tag. Keep the accent in the original spell hue.
            let erosion_edge = smoothstep(0.15, 0.95, -in.surface.y);
            let edge_light = 0.20 + 0.80 * rim * rim;
            emissive += core * glow * erosion_edge * edge_light * 0.65;
        }
    }
    if is_torch_material {
        let flame = in.surface.w < -1.5;
        let flame_time = light.torch_meta.y;
        // One flicker for the whole flame. A per-voxel seed made each cube pulse alone.
        let lift = flame_lift(in.surface.w);
        let lick_phase = select(dot(in.voxel_pos, vec3<f32>(3.1, 2.2, 2.7)), 0.0, is_viewmodel);
        let lick = 0.6 * sin(flame_time * 6.7 - lift * 2.6 + lick_phase)
            + 0.4 * sin(flame_time * 10.9 - lift * 3.4 - 1.7 * lick_phase);
        // Continuous over the mesh for the flame and the torch body alike. A per-voxel
        // floor() seed made each cube pulse on its own. 0.96 keeps the average brightness;
        // a higher mean pushes the saturated orange toward pale peach after tonemap.
        let flame_wave = 0.96 + 0.14 * lick;
        // base_color is already linear (decoded in the vertex stage). Each face
        // keeps that flat colour: a noise field turned the low-poly shell into
        // a brown volume and the blended billboard left no depth for the sky.
        // Same flat colours as a placed torch. Coal on the held fuel turned that cap black.
        let flame_color = max(in.base_color, vec3<f32>(0.0));
        emissive += flame_color * select(0.65, 1.35, flame) * flame_wave * fire_exposure;
        if flame {
            albedo = flame_color;
        } else {
            albedo *= 0.5;
        }
    }

    // --- Sun/moon fill (bounced indirect) ---
    let sun_fill = light.sun_color_daylight.rgb * sun_spectral
                   * light.sun_dir_intensity.w * sun_trans
                   * (0.02 + 0.025 * daylight) * (0.55 + 0.45 * hemi)
                   * mix(1.0, 0.45, clear_day);
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
    for (var group = 0u; group < 8u; group += 1u) {
        let first = group*8u;
        if first >= torch_count { break; }
        if any(world_pos < light.torch_group_bounds[group*2u].xyz)
            || any(world_pos > light.torch_group_bounds[group*2u+1u].xyz) { continue; }
        for (var i=first; i<min(first+8u,torch_count); i+=1u) {
            // Reject out-of-radius lights before the shadow ray: `select` evaluates
            // both arms, and eval_torch_light would return zero for them anyway.
            let to_torch = light.torch_positions[i].xyz - world_pos;
            let radius = light.torch_positions[i].w;
            if dot(to_torch, to_torch) >= radius * radius || light.torch_colors[i].w <= 0.0 { continue; }
            // Shade first: the three occlusion rays cost far more, up to 160
            // cells each at fine voxel scales, and a light that adds nothing
            // here (facing away, faded out) needs none of them.
            let ts = eval_torch_light(light.torch_positions[i], light.torch_colors[i],
                light.torch_meta.y,world_pos,n,to_eye,roughness,f0,metallic,foliage,i<moving_count,in.frame);
            let ts_sum = ts.diffuse + ts.fill + ts.spec;
            if max(ts_sum.x, max(ts_sum.y, ts_sum.z)) <= 1e-6 { continue; }
            var reach = 1.0;
            if !is_viewmodel {
                reach = torch_reach(world_pos, light.torch_positions[i].xyz, geometric_normal, in.frame);
            }
            if reach <= 0.0 { continue; }
            torch_diffuse += ts.diffuse * reach;
            torch_fill += ts.fill * reach;
            torch_spec += ts.spec * reach;
        }
    }
    let held_to = light.held_torch_pos_radius.xyz - world_pos;
    let held_r = light.held_torch_pos_radius.w;
    var held_reach = 0.0;
    if receives_local_light && held_r > 0.0 && light.held_torch_color_strength.w > 0.0
        && dot(held_to, held_to) < held_r * held_r {
        held_reach = 1.0;
        if !is_viewmodel {
            held_reach = torch_reach(world_pos, light.held_torch_pos_radius.xyz, geometric_normal, in.frame);
        }
    }
    if held_reach > 0.0 {
        let ht = eval_torch_light(
            light.held_torch_pos_radius, light.held_torch_color_strength,
            light.torch_meta.y, world_pos, n, to_eye, roughness, f0, metallic, foliage, true, in.frame,
        );
        torch_diffuse += ht.diffuse * held_reach;
        torch_fill    += ht.fill * held_reach;
        torch_spec    += ht.spec * held_reach;
    }

    // --- Linear HDR composition ---
    let sun_radiance = max(light.sun_color_daylight.rgb, vec3<f32>(0.0)) * sun_spectral;
    let moon_radiance = max(light.moon_color_twilight.rgb, vec3<f32>(0.0)) * moon_spectral;
    let sun_h = terrain_safe_unit3(to_eye + sun_dir, n);
    let moon_h = terrain_safe_unit3(to_eye + moon_dir, n);
    let sun_diffuse_energy = (vec3<f32>(1.0) - terrain_fresnel(f0, dot(to_eye, sun_h))) * (1.0 - metallic);
    let moon_diffuse_energy = (vec3<f32>(1.0) - terrain_fresnel(f0, dot(to_eye, moon_h))) * (1.0 - metallic);
    // Material response, before atmosphere/tonemapping: vegetation groups warm
    // solar planes against cooler sky-lit planes, as in the landscape studies.
    // Local lamps and emissive colour keep their authored spectra.
    let botanical = select(select(0.0, 0.45, is_natural && in.terrain_weights.x > 0.5), 1.0, foliage);
    // Loose powders take a cool lavender shadow side (sand, gravel, ash; earth
    // at reduced strength), warm in direct light.
    let powder = select(select(select(0.0, 0.55, kind == 2 || (is_natural && in.terrain_weights.y > 0.5)),
        1.0, kind == 33 || kind == 38 || (is_natural && in.terrain_weights.w > 0.5)), 0.0, is_backdrop);
    let sun_albedo = albedo * mix(vec3<f32>(1.0), vec3<f32>(1.10, 1.025, 0.76), botanical)
        * mix(vec3<f32>(1.0), vec3<f32>(1.03, 1.0, 0.95), powder);
    // Blue-lifted green read as teal on sky-lit canopy sides and far proxies;
    // the references keep shaded leaves a deep, slightly cool green.
    let fill_albedo = albedo * mix(vec3<f32>(1.0), vec3<f32>(0.86, 0.99, 0.90), botanical)
        * mix(vec3<f32>(1.0), vec3<f32>(0.90, 0.89, 1.12), powder);
    var lit = albedo * (
        diffuse_energy * cave_fill + (torch_diffuse + torch_fill) * fire_exposure)
        + fill_albedo * diffuse_energy * (ambient + sun_fill + moon_fill) * ambient_sky
        + sun_albedo * sun_radiance * sun_diffuse_energy * sun_diffuse * 0.318309886
        + albedo * moon_radiance * moon_diffuse_energy * moon_diffuse * 0.318309886 * sky_access;
    // Inter-ridge occlusion comes from the detail-map sun march in `sun_shadow`.
    var sun_specular = vec3<f32>(0.0);
    if light.sun_dir_intensity.w > 0.0 && sun_horizon > 0.0 {
        sun_specular = sun_radiance * terrain_directional_specular(n, macro_normal, to_eye, sun_dir,
            roughness, f0, in.ao_strength, kind, in.terrain_weights.z)
            * light.sun_dir_intensity.w * sun_trans * sun_shadow * sun_horizon * sky_access;
    }
    var moon_specular = vec3<f32>(0.0);
    if light.moon_dir_intensity.w > 0.0 && moon_horizon > 0.0 {
        moon_specular = moon_radiance * terrain_directional_specular(n, macro_normal, to_eye, moon_dir,
            roughness, f0, in.ao_strength, kind, in.terrain_weights.z)
            * light.moon_dir_intensity.w * moon_trans * moon_horizon * sky_access;
    }
    let direct_specular = sun_specular + moon_specular + torch_spec * fire_exposure;

    if debug_mode == 23 { // Directional diffuse + specular, linear HDR.
        let direct = sun_albedo * sun_radiance * sun_diffuse_energy * sun_diffuse * 0.318309886
            + albedo * moon_radiance * moon_diffuse_energy * moon_diffuse * 0.318309886 * sky_access
            + direct_specular - torch_spec * fire_exposure;
        return debug_surface_output(vec4<f32>(max(direct, vec3<f32>(0.0)), 1.0));
    }
    if debug_mode == 24 { // Existing ambient/bounce diffuse; excludes environment specular.
        return debug_surface_output(vec4<f32>(fill_albedo * diffuse_energy
            * (ambient + sun_fill + moon_fill) * ambient_sky + albedo * diffuse_energy * cave_fill, 1.0));
    }
    if debug_mode == 25 { // All local-light diffuse/fill/specular contributions.
        return debug_surface_output(vec4<f32>((albedo * (torch_diffuse + torch_fill) + torch_spec) * fire_exposure, 1.0));
    }

    // A bounded sky-environment approximation for opaque materials. It supplies
    // a broad response for metals and wet rock; a prefiltered environment/LUT
    // from the renderer is needed for accurate rough reflections of the scene.
    if kind != 8 && kind != 9 && kind != 54 && (kind != 55 || authored_kit) {
        let reflection_dir = reflect(-to_eye, n);
        let sky_direction = terrain_safe_unit3(vec3<f32>(reflection_dir.x,
            max(reflection_dir.y, 0.02), reflection_dir.z), vec3<f32>(0.0, 1.0, 0.0));
        let environment_mean = mix(sky_horizon, sky_up, 0.55);
        // Lean: the per-frame horizon and zenith terms stand in for a sky lookup.
        var reflected_sky = mix(sky_horizon, sky_up, sqrt(saturate1(sky_direction.y)));
        if !LEAN_SHADING {
            reflected_sky = atmosphere_color(sky_direction);
        }
        // A rough lobe straddles the horizon, so part of it sees lit ground
        // rather than blue sky. Without this, shaded grass seen at a grazing
        // angle reflected pure sky and read teal.
        let environment_luma = dot(environment_mean, vec3<f32>(0.2126, 0.7152, 0.0722));
        let rough_environment = mix(environment_mean,
            vec3<f32>(environment_luma) * vec3<f32>(1.0, 1.0, 0.88), 0.5);
        let environment = mix(reflected_sky, rough_environment, roughness * roughness);
        // Schlick-roughness without a DFG term over-reflects rough dielectrics
        // at grazing view; squaring tracks the split-sum response closer.
        let smooth_term = max(1.0 - roughness, 0.0);
        let grazing = vec3<f32>(smooth_term * smooth_term);
        let environment_fresnel = f0 + (max(grazing, f0) - f0)
            * terrain_pow5(1.0 - clamp(dot(n, to_eye), 0.0, 1.0));
        // F0 suppresses diffuse above, so this response is not layered on a
        // full-strength diffuse reflector. Screen-space or probe GI stays external.
        lit += max(environment, vec3<f32>(0.0)) * environment_fresnel
            * select(sky_access, select(0.0, 0.20 * sky_access, authored_kit), is_viewmodel)
            * cavity * mix(0.35, 1.0, metallic);
        lit += direct_specular;
    }

    // A small neutral first-person fill keeps the hand and sleeve readable in
    // deep caves without turning them into shiny world objects. It is applied
    // to every camera-relative part, including a metal tool head and its haft.
    if is_viewmodel && !authored_kit {
        lit += albedo * vec3<f32>(0.075, 0.078, 0.082)
            * (0.55 + 0.45 * hemi);
    }

    lit += emissive;
    if foliage {
        // Thin-leaf transmission is driven by incoming light and shadow;
        // it cannot illuminate a canopy at night or through solid terrain.
        let backlight = pow(max(dot(-to_eye, sun_dir), 0.0), 2.5);
        let top_sheen = smoothstep(0.15, 0.85, n.y) * (0.28 + 0.72 * sun_shadow);
        let transmission = (0.015 + backlight * 0.18 + top_sheen * 0.035) * sun_shadow * sun_trans
            * light.sun_dir_intensity.w * (0.45 + 0.55 * cavity);
        let leaf_warmth = mix(albedo, albedo * vec3<f32>(1.18, 1.24, 0.72), 0.55);
        lit += leaf_warmth * light.sun_color_daylight.rgb * sun_spectral * transmission;
        // Undersides and inward faces take light bounced off the ground and
        // the lower crown. Without it a crown's belly went black-green while
        // the turf under it stayed lit.
        let under = saturate1(-n.y * 0.5 + 0.5);
        lit += albedo * (ambient + sun_fill) * ambient_sky * under * under * 0.55;
    }
    if kind == 18 {
        // Glowcaps breathe as a colony, not as a constant green sticker.
        let pulse = 0.62 + 0.38 * sin(
            pattern_pos.x * 5.2 + pattern_pos.z * 4.7 + light.torch_meta.y * 2.4
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
        // Thin-film iridescence: each shard has its own film thickness and the
        // hue walks with the viewing angle, so turning the camera rolls rose,
        // gold and mint across fixed shards. Re-hues the light already received
        // at equal luminance, so it never glows in the dark; violet face-on.
        let grazing = 1.0 - clamp(dot(n, to_eye), 0.0, 1.0);
        let film = 0.56 + 0.44 * cos(6.2831853 * (vec3<f32>(0.0, 0.33, 0.67)
            + 0.3 + crystal_facet.x * 1.3 + grazing * 1.4));
        let luma_weights = vec3<f32>(0.2126, 0.7152, 0.0722);
        let tinted = film * dot(lit, luma_weights) / max(dot(film, luma_weights), 0.05);
        let iridescence = crystal_resolved * (0.16 + 0.5 * grazing * grazing)
            * (0.75 + 0.25 * crystal_facet.w);
        lit = mix(lit, tinted, iridescence);
    }
    if kind == 8 || kind == 9 {
        let reflected = reflect(-to_eye, n);
        let ndotv = clamp(dot(n, to_eye), 0.0, 1.0);
        // Voxel water steps expose thin vertical faces under a continuous
        // sheet. From above, grazing Fresnel on those faces draws a bright
        // outline at each step. Keep side reflections when seen face-on.
        let wall_reflection = select(1.0, 1.0 - smoothstep(0.06, 0.52, to_eye.y), water_side);
        // Physical dielectric Schlick Fresnel for water (n = 1.333 -> F0 = 0.0204).
        let reflection_f0 = select(clamp(dot(f0, vec3<f32>(0.333333333)), 0.0, 1.0), 0.0204, kind == 8);
        let fresnel = (reflection_f0 + (1.0 - reflection_f0) * terrain_pow5(1.0 - ndotv))
            * (1.0 - water_foam * 0.68) * wall_reflection;
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
        if kind == 8 {
            // The water lobe below already reflects the sun. Keep the
            // atmospheric aureole from adding a second smooth white disc.
            let sunward = smoothstep(0.94, 0.998, dot(refl_dir, sun_dir));
            let peak = max(reflection.r, max(reflection.g, reflection.b));
            reflection /= 1.0 + sunward * peak;
        }

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
        // A reflection reads 15-25% darker than its source; the screen-space
        // trace in indirect_gather.wgsl applies the same factor to scene hits.
        lit = mix(lit, reflection * WATER_REFLECTION_GAIN, clamp(fresnel * ambient_sky, 0.0, 1.0));
        // Combine the tight core, broad sheen and direct highlight before
        // limiting the water response so the reflected sun retains wave detail.
        let sun_glint_sharp = water_glint_lobe(n, to_eye, sun_dir, max(roughness * 0.72, 0.058));
        let sun_glint_broad = water_glint_lobe(n, to_eye, sun_dir, max(roughness * 1.60, 0.17));
        // Wave-advected breakup of the sharp core. Far filtering widens the lobe
        // until the sun path is a smooth column; real glitter is broken up by
        // swell. Mean-preserving (0.4..1.6, mean 1.0), filtered by pixel footprint
        // so it fades instead of aliasing, and it moves only with the wave clock.
        let glint_t = light.torch_meta.y;
        let glint_breakup = 0.4 + 1.2 * surface_noise_filtered(
            pattern_pos.xz * 0.45 + vec2<f32>(glint_t * 0.30, -glint_t * 0.21), fp_m * 0.45);
        let water_sheen = light.sun_color_daylight.rgb * sun_spectral
             * (sun_glint_sharp * glint_breakup * 0.105 + sun_glint_broad * 0.045)
             // Fully gated: an 18% floor let the glint shine through
             // terrain shadow and cloud, doubling the path under overcast.
             * light.sun_dir_intensity.w * sun_trans * sun_shadow
             * sky_access;
        let surface_glint = direct_specular + water_sheen;
        let glint_peak = max(surface_glint.r, max(surface_glint.g, surface_glint.b));
        let glint_rolloff = select(1.0, 1.0 + 1.6 * glint_peak, kind == 8);
        lit += surface_glint / glint_rolloff * (1.0 - water_foam * 0.65) * wall_reflection;
        if geometric_normal.y > 0.5 && ambient_sky > 0.12 && terrain_has_resident_metadata(in.ao_strength) {
            // Sun glitter: each 5 cm texel is a tiny facet with a fixed random tilt
            // added to the wave normal. It lights when that facet mirrors the sun
            // to the eye, so glints are hard texels that drift and fade as the
            // waves move, with no per-frame re-roll. Fades out with the texel size.
            let cell = floor(pattern_pos.xz * PIXEL_RES);
            let tilt = vec2<f32>(mood_hash31(vec3<f32>(cell, 1.3)), mood_hash31(vec3<f32>(cell, 7.9))) - vec2<f32>(0.5);
            let facet = normalize(n + vec3<f32>(tilt.x, 0.0, tilt.y) * 0.55);
            let half_vec = normalize(to_eye + sun_dir);
            let align = clamp((dot(facet, half_vec) - 0.9965) / 0.0035, 0.0, 1.0);
            let glint = align * align * surface_detail_weight(fp_m * PIXEL_RES) * (1.0 - water_foam)
                * smoothstep(0.02, 0.25, sun_dir.y);
            lit += sun_radiance * sun_shadow * sun_trans * glint
                * (0.5 * smoothstep(0.12, 0.65, ambient_sky));
        }
    }

    // Keep the normal exposure/tone resolve for this diagnostic, while removing
    // aerial perspective, cloud-layer fog, streaming veil and local mist. This
    // exposes geometry/lighting discontinuities instead of hiding them in haze.
    if debug_mode == 26 {
        return debug_surface_output(vec4<f32>(max(lit, vec3<f32>(0.0)), 1.0));
    }

    // Underground draw-distance gaps end in darkness, never outdoor fog.
    let camera_cave = clamp(light.sky_art[1].w, 0.0, 1.0);
    if camera_cave >= 0.999 && light.mood.x < 0.001 && realm_rig_on() {
        // A sunless realm has its own air, shaft of light and glowing water;
        // the draw distance ends at its fog range, not 80 m.
        var realm_lit = max(lit, vec3<f32>(0.0));
        if uw_active() && kind == 8 && world_pos.y > cam.camera_pos.y {
            // The glowing surface seen from below, rippled by the same web.
            let pool = realm_pool_at(cam.camera_pos.xyz);
            if pool >= 0 {
                let b = light.realm[4u + u32(pool) * 2u];
                let web = realm_caustic(world_pos.xz * 1.3, light.torch_meta.y, 0.0, fp_m);
                realm_lit = realm_pool_tint(b.z) * b.y * (0.30 + 0.9 * web) * fire_exposure;
            }
        }
        realm_lit = realm_air(realm_lit, cam.camera_pos.xyz, view_dir, dist, fire_exposure);
        let fade = 1.0 - smoothstep(light.fog_params.y * 0.85, max(light.fog_params.y, 1.0), dist);
        return WorldOutput(vec4<f32>(realm_lit * fade, 1.0), vec4<f32>(0.0));
    }
    if camera_cave >= 0.999 && light.mood.x < 0.001 {
        let fade = 1.0-smoothstep(24.0, max(32.0, min(light.fog_params.y, 80.0)), dist);
        return WorldOutput(vec4<f32>(max(lit,vec3<f32>(0.0))*fade,1.0),vec4<f32>(0.0));
    }

    // ============================================================
    // AERIAL PERSPECTIVE
    // Shared contract: aerial_perspective returns (in-scattered radiance,
    // transmittance), i.e. RGB is already premultiplied by path extinction.
    // ============================================================
    var raw_sky_radiance: vec3<f32>;
    if LEAN_SHADING {
        // Per-frame horizon terms toward or away from the sun, then zenith.
        let toward_sun = saturate1(dot(safe_normalize2(view_dir.xz, sun_az), sun_az) * 0.5 + 0.5);
        raw_sky_radiance = max(mix(mix(antisolar_horizon, solar_horizon, toward_sun), sky_up,
            smoothstep(0.0, 0.6, view_dir.y)), vec3<f32>(0.0));
    } else {
        raw_sky_radiance = max(atmosphere_color(view_dir), vec3<f32>(0.0));
    }
    let sky_luma = dot(raw_sky_radiance, vec3<f32>(0.2126, 0.7152, 0.0722));
    let weather_sky_radiance = mix(raw_sky_radiance, vec3<f32>(sky_luma), heavy_cloud * 0.55)
        * mix(1.0, 0.78, heavy_cloud * daylight);
    let realm_haze = light.realm[11].x > 0.5;
    let ground_fog_radiance = select(weather_sky_radiance, light.fog_color_density.rgb, realm_haze);
    // Integrate the local weather layer in both atmosphere modes. The aerial
    // LUT models planetary air and does not contain this ground fog.
    let fog_start = max(light.fog_params.x, 0.0);
    var ground_transmittance = 1.0;
    if dist > fog_start {
        let path = dist - fog_start;
        var fog_range = max(light.fog_params.y - fog_start, 1.0);
        // The short flat-world weather range made even clear Planet valleys
        // almost opaque after 1-2 km. Keep a readable ground-air layer over the
        // continuous surface; dense weather, caves and the orbital handoff retain
        // their existing range. Local mist and planetary scattering stay separate.
        let planet_ground_air = select(0.0, (1.0 - camera_cave)
            * (1.0 - smoothstep(1500.0, 8000.0, max(cam.planet[1].w, 0.0))),
            cam.planet[0].w > 0.5 && cam.planet[0].w < 1.5);
        let clear_ground_air = daylight * (1.0 - smoothstep(0.15, 0.65, heavy_cloud));
        fog_range = mix(fog_range, max(fog_range, 6000.0 - fog_start),
            planet_ground_air * clear_ground_air);
        let layer_base = light.fog_params.z * scale;
        let layer_k = max(light.fog_params.w, 0.0);
        let start_y = cam.camera_pos.y + view_dir.y * fog_start;
        let broad_column = terrain_height_integral(start_y, world_pos.y, path,
            layer_base, layer_k);
        let valley_column = terrain_height_integral(start_y, world_pos.y, path,
            layer_base - 18.0 * scale, layer_k * 2.0);
        let density = mix(0.22, 0.10, daylight) + heavy_cloud * 0.10;
        let optical_depth = (1.35 + density * 3.2)
            * (0.72 * broad_column + 0.28 * valley_column) / fog_range;
        ground_transmittance = exp(-clamp(optical_depth, 0.0, 80.0));
    }
    var atmosphere_transmittance = ground_transmittance;
    var atmosphere_scatter = ground_fog_radiance * (1.0 - ground_transmittance);
    if !realm_haze && atmosphere_mode() >= ATMOSPHERE_MODE_SKY_VIEW_LUT {
        let ap = aerial_perspective(view_dir, dist, scale, vec3<f32>(0.0));
        atmosphere_transmittance *= clamp(ap.a, 0.0, 1.0);
        atmosphere_scatter += max(ap.rgb, vec3<f32>(0.0)) * ground_transmittance;
    }
    // Cloud-layer fog remains the supplied homogeneous-layer approximation.
    // Give it its own radiance; do not inflate already integrated air scattering
    // by the cloud opacity or apply another solar lobe on top of the sky LUT.
    let cloud_fog = cloud_fog_amount(view_dir, dist)
        * mix(1.0, 0.25, camera_cave);
    let cloud_transmittance = 1.0 - cloud_fog;
    // Fog radiance is a property of the air, not of the surface normal.
    let cloud_fog_radiance = max(select(mix(weather_sky_radiance, sky_up, 0.35),
        ground_fog_radiance, realm_haze), vec3<f32>(0.0));
    lit = lit * cloud_transmittance + cloud_fog_radiance * (1.0 - cloud_transmittance);
    // The observer's air can be enclosed even when looking toward a lit exit.
    // A shaded receiver outdoors still has the same in-scatter in front of it.
    atmosphere_scatter *= mix(1.0, 0.28, camera_cave);
    lit = lit * atmosphere_transmittance + atmosphere_scatter;
    lit = hell_squall_air(lit, dist, ground_fog_radiance);
    // Earth-scale Rayleigh barely tints land inside the 6 km coverage, so far
    // forest stayed saturated up to the streaming edge. Fade it toward the sky
    // the viewer sees behind it, as the reference backdrops do. The target was
    // the horizon probe nine degrees up, which at sunset is the solar aureole:
    // brighter than the horizon itself, so far ground outshone the sky it
    // stood against by half again and the backdrop edge drew a bright line.
    let far_limit = max(select(cam.world_params.w, light.sky_horizon_reserved.w,
        light.sky_horizon_reserved.w > 0.0), 1.0);
    // Only finite flat-world backdrops end at this range. Planet has continuous
    // surface coverage beyond it; forcing that land to sky erases whole valleys
    // and exposes bright wedges where resident terrain meets the distant mesh.
    let far_close = select(smoothstep(far_limit * 0.75, far_limit, dist)
        * (1.0 - camera_cave), 0.0, cam.planet[0].w > 0.5);
    var veil_sky = raw_sky_radiance;
    if LEAN_SHADING && far_close > 0.0 {
        veil_sky = max(atmosphere_color(view_dir), vec3<f32>(0.0));
    }
    let veil = select((1.0 - exp(-max(dist - 65.0, 0.0) / 1850.0)) * 0.78 * (1.0 - camera_cave),
        0.0, realm_haze);
    // Take the horizon's hue at the surface's own brightness, and only part of
    // the horizon's brightness. A plain mix toward a low sun's horizon gave lit
    // and shaded faces one value, so every far range read as the same beige.
    let veil_luma = max(dot(veil_sky, vec3<f32>(0.2126, 0.7152, 0.0722)), 1e-4);
    let veil_hue = veil_sky * (max(dot(lit, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.0) / veil_luma);
    lit = mix(mix(lit, veil_hue, veil * 0.55), veil_sky, veil * 0.15);
    // Air in front of a dark range scatters skylight toward the eye, so far
    // shaded slopes rise toward the sky while lit faces keep their value.
    // Without it backlit ranges at 2-4 km stayed darker than the plain in
    // front of them and every ridge line read as one flat silhouette. A low
    // sun's sky outshines all ground, so the lift fades out there.
    let veil_lift = veil * veil * 0.6 * smoothstep(0.30, 0.60, sun_dir.y);
    lit = mix(lit, max(lit, veil_sky * 0.85), veil_lift);
    lit = mix(lit, veil_sky, far_close);
    // Broad, low-contrast cave air separates distant silhouettes without
    // washing out near walls or requiring a volumetric pass.
    let cave_air = smoothstep(7.0, 46.0, dist) * camera_cave * 0.20;
    let cave_air_color = vec3<f32>(0.020, 0.034, 0.045)
        + vec3<f32>(0.025, 0.046, 0.068) * opening_bounce;
    lit = mix(lit, cave_air_color, cave_air);
    let total_fog_t = 1.0 - atmosphere_transmittance * cloud_transmittance
        * (1.0 - veil) * (1.0 - far_close) * (1.0 - cave_air);

    if uw_active() {
        // Dry land seen past the edge of Snell's window is hidden by total internal
        // reflection; the diver sees the mirrored water body there, not the shore.
        let beyond_window = world_pos.y > uw_surface_y() + 0.05 && view_dir.y < UW_WINDOW_COS;
        if (kind == 8 && world_pos.y > cam.camera_pos.y) || beyond_window {
            // The surface above a diver is the same Snell's-window sky the sky pass paints
            // behind it, so the two meet without seams.
            let jitter = fract(dot(floor(in.pos.xy), vec2<f32>(0.754877666, 0.569840296)));
            lit = uw_backdrop(view_dir, jitter);
        } else {
            // Absorption, caustic light and a haze that matches the sky pass's underwater backdrop.
            lit = uw_apply_fragment(lit, albedo, world_pos, n, view_dir, dist, sun_shadow * sun_trans);
            // `uw_haze` is a cheap stand-in; far fragments fade into the real backdrop
            // so the visible range ends in the same water the sky pass paints.
            let far = smoothstep(22.0, 55.0, dist);
            if far > 0.0 {
                let jitter = fract(dot(floor(in.pos.xy), vec2<f32>(0.754877666, 0.569840296)));
                lit = mix(lit, uw_backdrop(view_dir, jitter), far);
            }
        }
    }
    if lava_eye_active() && !is_viewmodel {
        let melt = lava_backdrop(view_dir);
        let transmission = exp(-max(dist, 0.0) * vec3<f32>(3.2, 5.7, 9.0));
        let contact = lit * vec3<f32>(0.95, 0.40, 0.16) + albedo * melt * 0.6;
        lit = mix(melt, contact, transmission);
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
        var edge_sky = raw_sky_radiance;
        if LEAN_SHADING {
            edge_sky = max(atmosphere_color(view_dir), vec3<f32>(0.0));
        }
        lit = mix(lit, edge_sky, edge_fade);
    }

    // Nearby mist sits in front of the graded surface and distant haze, just
    // as it does in the sky pass. Later lighting must use its transmittance too.
    let mist = local_mist(view_dir, dist, 1.0 - camera_cave, ground_fog_radiance);
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
        let reflection_visibility = surface_visibility * ambient_sky * smoothstep(0.03, 0.30, water_depth);
        let refractable = water_depth < 1.4 && water_foam < 0.50 && reflection_visibility > 0.05;
        receiver=vec4<f32>(n.x,n.z,reflection_visibility + select(0.0, 2.0, refractable),-max(f,0.001));
    } else if kind == 56 && !hedron {
        // Optical receiver: view-space normal xy, visibility, class -2.
        // Kept distinct from water (-1..0) and diffuse receivers (>=0).
        let optical_n = select(n, -n, dot(n, to_eye) < 0.0);
        let view_right = terrain_safe_unit3(cam.ray_right.xyz, vec3<f32>(1.0, 0.0, 0.0));
        let view_up = terrain_safe_unit3(cam.ray_up.xyz, vec3<f32>(0.0, 1.0, 0.0));
        receiver = vec4<f32>(dot(optical_n, view_right), dot(optical_n, view_up), surface_visibility, -2.0);
    } else if kind != 8 {
        receiver = vec4<f32>(albedo * surface_visibility, max(dot(ambient * ambient_sky, vec3<f32>(0.2126,0.7152,0.0722)),0.0));
    }
    if cam.planet[0].w > 0.5 && in.ao_strength < 0.0 {
        let nrm = normalize(in.normal);
        let sun = normalize(select(cam.planet[5].xyz, light.sun_dir_intensity.xyz, cam.planet[0].w > 1.5));
        let ndl = max(dot(nrm, sun), 0.0);
        let altitude = max(cam.planet[1].w, 0.0);
        // Near the ground the cover takes the resident terrain's full shading,
        // so the residency edge shows no brightness step. From high up that
        // shading shows per-triangle material tints as a checker, so the cover
        // hands over to a plain lit colour. In orbit that crushes the disc, so
        // the same cover gets a night side, a terminator and a rim.
        let space = smoothstep(12000.0, 40000.0, altitude);
        // Cover water carries its seabed colour for the ground view; from above it is open sea.
        let ocean = kind == 8 || (in.base_color.b > in.base_color.g && in.base_color.b > in.base_color.r);
        let surface_color = select(in.base_color, terrain_srgb_to_linear(vec3<f32>(0.07, 0.25, 0.38)), kind == 8);
        let air = exp(-max(altitude - 150.0, 0.0) / 1400.0);
        let haze = (1.0 - exp(-dist / 12000.0)) * air * 0.35;
        let aloft = mix(surface_color * (0.06 + ndl * 1.2), vec3<f32>(0.13, 0.24, 0.38), haze);
        let grounded = mix(max(lit, vec3<f32>(0.0)), aloft, smoothstep(1500.0, 8000.0, altitude));
        let day = smoothstep(-0.06, 0.28, dot(nrm, sun));
        // Day brightness is unchanged; the night side drops to a cool moonlit
        // floor, low enough to keep the terminator, high enough to read terrain.
        let moonlit = vec3<f32>(0.07, 0.085, 0.13);
        var body = surface_color * (moonlit * (1.0 - day) + day * (0.45 + ndl * 1.15));
        let spec = pow(max(dot(nrm, normalize(sun + to_eye)), 0.0), 48.0) * ndl;
        body += select(vec3<f32>(0.0), vec3<f32>(0.42, 0.50, 0.48) * spec, ocean);
        let fres = pow(1.0 - clamp(dot(nrm, to_eye), 0.0, 1.0), 2.6);
        // Weather from orbit. Body-frame directions pin it to the planet.
        let t = light.torch_meta.y;
        let b = planet_wx_body(nrm);
        let sun_b = planet_wx_body(sun);
        let cloud = planet_wx_cloud(b, t);
        let storm = planet_wx_storm(b, t);
        // Shadow is the deck sampled one cell toward the sun.
        let shade = planet_wx_cloud(normalize(b + sun_b * 0.012), t);
        body *= 1.0 - 0.55 * shade * day;
        let sun_dot = dot(nrm, sun);
        let deck = vec3<f32>(0.92, 0.94, 0.97) * (0.022 + day * (0.30 + 0.70 * max(sun_dot, 0.0)))
            * (1.0 - 0.35 * storm);
        body = mix(body, deck, cloud * 0.92);
        // Rayleigh tint thickens toward the limb; sunset light rims the terminator.
        let path = clamp(fres * 1.4, 0.0, 1.0);
        body = mix(body, vec3<f32>(0.22, 0.46, 0.92) * (0.03 + 0.95 * day), path * 0.7);
        let dusk = exp(-pow((sun_dot - 0.02) / 0.10, 2.0));
        body += vec3<f32>(0.85, 0.38, 0.12) * dusk * (0.18 + 0.5 * cloud) * (0.4 + 0.6 * path);
        let night = 1.0 - day;
        body += vec3<f32>(0.70, 0.80, 1.0) * planet_wx_lightning(b, t, storm) * night * 0.5;
        body += vec3<f32>(1.0, 0.22, 0.32) * planet_wx_sprite(b, t, storm) * night * 0.5;
        // Aurora only during an aurora event; a permanent floor read as a green ring.
        body += planet_wx_aurora(b, t, 0.3) * night * light.mood.y * 0.55;
        return WorldOutput(vec4<f32>(mix(grounded, body, space), 1.0), vec4<f32>(0.0));
    }
    return WorldOutput(vec4<f32>(max(lit, vec3<f32>(0.0)), 1.0), receiver);
}

