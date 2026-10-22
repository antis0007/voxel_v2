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
    let mid = 1.0 - smoothstep(0.22, 0.65, footprint_m);
    // No palette quantisation: tone stays continuous so gradients never band.
    // A small ordered-dither lift on the 5 cm lattice keeps the pixel-art
    // texture up close, and fades out as the lattice shrinks below a pixel so
    // distant ground does not shimmer. Multiplicative gain preserves hue.
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
    let scale     = max(cam.world_params.x, 1e-6);
    let geometry_kind=f32(u32(round(max(in.surface.z,0.0)))%64u);
    var model_pos = in.pos;
    if in.surface.w < -1.5 {
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
    out.water_optics = vec2<f32>(10.0, 1.0);
    if geometry_kind == 8.0 && in.surface.w >= 8.0 && in.ao_strength >= 0.0 {
        out.water_optics = vec2<f32>(floor(in.surface.w-8.0)*0.25,
            clamp(fract(in.surface.w)*2.0,0.006,1.0));
    }
    var world_pos = model_pos * scale;
    let horiz_dist = length((world_pos - cam.camera_pos.xyz).xz);
    // A side wall's surface-level vertices (fractional y) ride the same waves as
    // the top face they share a corner with, so the two never split apart.
    let water_surface_side = geometry_kind == 8.0 && abs(in.normal.y) < 0.5
        && in.ao_strength >= 0.0 && fract(in.pos.y) > 0.002;
    if geometry_kind == 8.0 && (in.normal.y > 0.5 || water_surface_side) && in.ao_strength >= 0.0 {
        // Linear shoaling: wave height grows as the bed shallows, then the
        // displacement is damped in the last few centimetres of run-up so the
        // surface does not intersect the beach. The old gate did the opposite
        // and flattened every wave at exactly the surf zone.
        let depth = out.water_optics.x;
        let shoal = 1.0 + 1.35 * exp(-depth * 1.15) * smoothstep(0.015, 0.55, depth);
        let surf_fade = smoothstep(0.015, 0.20, depth);
        let dist_atten = 1.0 - smoothstep(96.0 * scale, 240.0 * scale, horiz_dist);
        let wave_disp = eval_water_vertex_wave(model_pos.xz, scale, light.torch_meta.y);
        // `eval_water_vertex_wave` receives metre-space sample spacing and
        // returns metres. Do not multiply by voxel scale again: doing so made
        // the same sea four times calmer in a quarter-metre world.
        world_pos += wave_disp * (shoal * surf_fade * dist_atten);
        // A slower set envelope lifts occasional breaking crests near shore.
        let along = dot(world_pos.xz, vec2<f32>(0.54, -0.84));
        let breaker_set = pow(max(sin(along * 0.12 + light.torch_meta.y * 0.72), 0.0), 4.0);
        let breaker_band = (1.0 - smoothstep(0.05, 0.85, depth)) * surf_fade;
        world_pos.y += breaker_set * breaker_band * 0.14;
    }
    var backdrop_depth_bias = 0.0;
    if terrain_is_backdrop(in.ao_strength) {
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
        backdrop_depth_bias = underlap * select(0.0, 0.0035, geometry_kind == 0.0);
    }
    out.pos        = cam.view_proj * vec4<f32>(world_pos - cam.camera_pos.xyz, 1.0);
    // Separate exact fallback/resident depth without splitting shared world edges.
    // Per-face normal insets opened sky pinholes on fine voxel silhouettes.
    // Depth is reverse-Z (1 near, 0 far): pushing a surface away scales its depth
    // toward zero by a fraction of its own value, the mirror of the old forward-Z
    // `z += (w - z) * bias`.
    if in.ao_strength < -1.5 {
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
    out.relative_pos = world_pos - cam.camera_pos.xyz;
    out.voxel_pos  = in.pos;
    out.normal     = terrain_safe_unit3(in.normal, vec3<f32>(0.0, 1.0, 0.0));
    out.surface    = in.surface;
    out.surface.z = geometry_kind;
    out.ao_strength = in.ao_strength;
    // Resolve IDs at vertices; interpolate colors/finish, never material IDs.
    let far_terrain=in.ao_strength<0.0 && in.ao_strength> -1.5 && in.surface.z==0.0;
    let key=u32(round(max(select(in.surface.z,in.surface.w,far_terrain),0.0)));
    let family=key%64u;
    let block_key=key/64u;
    out.terrain_weights = terrain_coverage_for_family(family);
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
    let mesh_specular = clamp(in.surface.y, 0.0, 1.0);
    let mesh_reflectance = clamp(0.5 * sqrt(mesh_specular / 0.04), 0.0, 1.0);
    properties.x=select(mesh_reflectance, properties.x, properties.x>=0.0);
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
    let shadow_range = max(shadow.cascades[3].far_plane, 1e-4);
    let range_fade = smoothstep(shadow_range * 0.78, shadow_range,
        distance(world_pos, cam.camera_pos.xyz));
    if range_fade >= 1.0 { return 1.0; }

    var first_cascade = 3;
    for (var i = 0; i < 3; i = i + 1) {
        if linear_depth < shadow.cascades[i].far_plane {
            first_cascade = i;
            break;
        }
    }

    // Resolve uncovered weight with each successive valid cascade. Interior
    // pixels normally execute one kernel; blend bands normally execute two.
    // A rare overlap of several projection edges can use up to four. This
    // avoids switching abruptly when the immediate next cascade is invalid.
    var remaining = 1.0;
    var visibility = 1.0;
    for (var i = first_cascade; i < 4; i = i + 1) {
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

        let edge_band = max(texel * 4.0, select(0.035, 0.07, i == 3));
        let edge_coverage = smoothstep(padding, padding + edge_band, edge);
        var depth_coverage = 1.0;
        if i < 3 {
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
    roughness: f32, f0: vec3<f32>, metallic: f32, foliage: bool,
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
    let flicker = torch_light_flicker(torch_pos_radius.xyz * 0.23 + vec3<f32>(0.0, 1.7, 0.0), time);
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

// Interpolated coverage, not interpolated numeric material IDs. Missing coverage
// retains the authored base color. Use unscaled voxel coordinates: texture scale
// must not move physical climate boundaries, shorelines, or snow altitude.
fn natural_surface_blended(
    base: vec3<f32>, p: vec3<f32>, normal: vec3<f32>,
    terrain_weights: vec4<f32>, footprint: f32,
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
    // One-texel grain on the world pixel grid, not a smooth multi-octave field.
    let grain_cell = floor(natural_face_uv(q, normal) * PIXEL_RES);
    let grain = (surface_hash(grain_cell + vec2<f32>(11.3, -4.7)) - 0.5)
        * surface_detail_weight(fp * PIXEL_RES);
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
        // The mean of smoothstep(0.48, 0.82, fract(y)) is 0.35. Preserve it
        // as individual block-top turf strips disappear; do not leave an alias.
        let turf_overhang = mix(0.35,
            smoothstep(0.48, 0.82, fract(p.y) + clumps * 0.18), surface_detail_weight(footprint));
        let side_coat = mix(soil, top_coat, max(coastal_dune, turf_overhang * 0.78));
        var turf_color = mix(side_coat, top_coat, up);
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
        let exposed = clamp(max(exposure * 0.36, alpine_rock * 0.68), 0.0, 1.0);
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
        // A crisp blade-scale lift and shadow pair gives the turf texture
        // without desaturating it. Large-scale colour is handled by the meadow
        // ramp above; this is the part that disappears into mud when absent.
        let blade = surface_noise_filtered(
            q.xz * vec2<f32>(3.7, 2.9) + q.y * vec2<f32>(0.13, -0.17),
            fp * 3.7,
        ) - 0.5;
        let blade_weight = surface_detail_weight(fp * 3.7) * up;
        turf_color *= 1.0 + blade * 0.10 * blade_weight + grain * 0.10 * up;
        // Hard grass pixels on the world texel grid. Unlike the filtered blade
        // field this does not blur toward the average: each texel picks a
        // tip/root tone, which is what makes the turf read as pixel art. The y
        // term only decorrelates stacked faces; it never changes the cell size.
        let grass_cell = floor(q.xz * PIXEL_RES) + floor(q.y * PIXEL_RES) * vec2<f32>(7.0, -5.0);
        let grass_px = surface_hash(grass_cell);
        let grass_px2 = surface_hash(grass_cell + vec2<f32>(17.3, -9.1));
        let pixel_weight = surface_detail_weight(fp * PIXEL_RES) * up * (1.0 - snow);
        turf_color *= 1.0 + (grass_px - 0.5) * 0.62 * pixel_weight;
        let pixel_tip = step(0.58, grass_px2) * pixel_weight;
        let pixel_root = step(grass_px2, 0.24) * pixel_weight;
        // Tips and roots change value inside the olive, not the hue. The
        // stronger two-tone split reads as authored 8 cm grass pixels rather
        // than flat green paint.
        turf_color *= 1.0 + pixel_tip * 0.22 - pixel_root * 0.24;
        // A second per-texel value layer, independent of the first. It used to
        // be an 18 cm lattice (3-4 texels), which drew pale squares several
        // pixels wide beside one-pixel grain. Hue shifts stay inside the
        // olive/yellow-green ramp.
        let grass_macro_cell = floor(q.xz * PIXEL_RES) + vec2<f32>(-13.0, 29.0);
        let grass_macro = (surface_hash(grass_macro_cell) - 0.5)
            * surface_detail_weight(fp * PIXEL_RES) * up * (1.0 - snow);
        let grass_hue = surface_hash(grass_cell + vec2<f32>(41.7, -17.2));
        let grass_tint = mix(
            vec3<f32>(0.92, 1.03, 0.86),
            vec3<f32>(1.07, 1.01, 0.85),
            grass_hue,
        );
        turf_color *= (1.0 + grass_macro * 0.14)
            * mix(vec3<f32>(1.0), grass_tint, pixel_weight * 0.55);
        // Meadow flowers in patches. A low-frequency field decides where they
        // grow and a 12/m pixel hash decides which pixels bloom, so each patch
        // is one continuous cluster. Both fade out with the footprint, so distant
        // ground does not sparkle.
        let flower_patch = smoothstep(0.56, 0.68,
            surface_noise_filtered(q.xz / 9.0 + vec2<f32>(-23.0, 17.0), fp / 9.0));
        let flower_bloom = pixel_blob(q.xz, 12.0, vec2<f32>(53.1, -21.4), 0.07, fp)
            * flower_patch * up * (1.0 - snow) * (1.0 - forest * 0.5);
        let flower_hue = surface_hash(floor(q.xz * PIXEL_RES) + vec2<f32>(7.9, 61.3));
        let flower_col = select(
            select(vec3<f32>(0.93, 0.90, 0.80), vec3<f32>(0.95, 0.78, 0.18), flower_hue > 0.33),
            vec3<f32>(0.58, 0.36, 0.80),
            flower_hue > 0.66);
        turf_color = mix(turf_color, flower_col, flower_bloom);
        turf_color *= 1.0 + blade * 0.11 * blade_weight * (1.0 - snow);
        // Deepen the meadow's authored olive at full daylight. Snow and dry
        // coastal sand keep their own values; this only removes the pale mint
        // wash from living turf.
        turf_color *= mix(vec3<f32>(0.80, 0.92, 0.74), vec3<f32>(1.0), snow);
        // Limited sprite palette: the turf albedo steps onto four value bands.
        // Hue still comes from the climate ramp; each 8 cm pixel picks its band
        // from a hash. Only the albedo is stepped, so lighting stays continuous.
        let turf_luma = max(dot(turf_color, vec3<f32>(0.2126, 0.7152, 0.0722)), 1e-3);
        let turf_chroma = clamp(turf_color / turf_luma, vec3<f32>(0.0), vec3<f32>(2.0));
        let band_pick = surface_hash(grass_cell + vec2<f32>(71.3, 19.7));
        let band_t = saturate1((turf_luma - 0.10) / 0.30);
        let band = floor(clamp(band_t * 4.0 + (band_pick - 0.5) * 0.9, 0.0, 3.999));
        turf_color = turf_chroma * (0.15 + band * 0.085);
        color += turf_color * weights.x;
    }
    if weights.y > 0.0 {
        // Keep the authored Dirt / Mud / Clay hue instead of replacing it with
        // one shared brown, and add a close-range grit break-up.
        let earth_cell = floor(natural_face_uv(q, normal) * PIXEL_RES) + vec2<f32>(9.0, -3.0);
        let earth_grain = (surface_hash(earth_cell) - 0.5)
            * surface_detail_weight(fp * PIXEL_RES);
        var earth = mix(base, soil, 0.46)
            * (1.0 + earth_grain * 0.14);
        // Loose pebbles: sparse grey stones on a 4/m face grid, filtered away
        // once a cell is smaller than a pixel.
        let face_q = natural_face_uv(q, normal);
        let pebble = pixel_blob(face_q, 20.0, vec2<f32>(29.0, 3.0), 0.06, fp);
        let pebble_tone = floor(surface_hash(floor(face_q * PIXEL_RES) + vec2<f32>(5.0, 2.0)) * 3.0) / 3.0;
        earth = mix(earth, vec3<f32>(0.46, 0.44, 0.40) * (0.80 + pebble_tone * 0.35), pebble);
        color += earth * weights.y;
    }
    if weights.z > 0.0 {
        let bedding = surface_noise_filtered(vec2<f32>(q.x * 0.11 + q.z * 0.07, q.y * 0.42 + mottle * 0.8), fp * 0.60);
        var rock = base * mix(vec3<f32>(0.98, 1.00, 1.03), vec3<f32>(1.06, 1.02, 0.96), heat)
            * (0.96 + bedding * 0.28);
        // Hairline cracks: the zero set of a low-frequency field, drawn at a
        // width that widens with the footprint so a crack never aliases.
        // Cracks are one-pixel lines on the 20/m lattice: the zero set of a
        // smooth field, sampled per pixel so the edges step like sprite art.
        let rock_face = natural_face_uv(q, normal);
        let crack_cell = floor(rock_face * PIXEL_RES);
        let crack_field = surface_noise(crack_cell * 0.11 + vec2<f32>(3.0, -7.0)) - 0.5;
        let crack = step(abs(crack_field), 0.018) * surface_detail_weight(fp * 20.0);
        rock = mix(rock, rock * 0.56, crack);
        // Lichen: hard pixel blobs in pale grey-green, on sunward and upper faces.
        let lichen_field = surface_noise_filtered(q.xz / 2.7 + vec2<f32>(44.0, -9.0), fp / 2.7);
        let lichen_zone = smoothstep(0.62, 0.70, lichen_field)
            * step(-0.2, normal.y + 0.25 * (heat - 0.5));
        let lichen = pixel_blob(rock_face, PIXEL_RES, vec2<f32>(61.0, 17.0), 0.10, fp) * lichen_zone;
        rock = mix(rock, vec3<f32>(0.58, 0.62, 0.42) * (0.92 + bedding * 0.12), lichen);
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
    let modulation = 0.90 + mottle * 0.20 + grain * 0.18 + clumps * 0.12;
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
    if kind == 16 {
        // Snapped to texel centres: every noise below is flat inside a texel.
    let q = (floor(p * cam.world_params.x * PIXEL_RES) + vec3<f32>(0.5)) / PIXEL_RES;
        let mottle = surface_noise_filtered(q.xz / 17.0 + q.y * vec2<f32>(0.017, 0.027), footprint * cam.world_params.x * 0.068);
        return max(mix(base, vec3<f32>(0.25, 0.18, 0.33), smoothstep(0.42, 0.72, mottle) * 0.4)
            * (0.90 + mottle * 0.20), vec3<f32>(0.002));
    }
    let weights = vec4<f32>(
        select(0.0, 1.0, kind == 0 || kind == 1 || kind == 7),
        select(0.0, 1.0, kind == 2), select(0.0, 1.0, kind == 3), select(0.0, 1.0, kind == 4));
    return natural_surface_blended(base, p, normal, weights, footprint);
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
    var d1 = 8.0;
    var d2 = 8.0;
    var id = cell;
    var toward = vec2<f32>(0.0);
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let o = vec2<f32>(f32(i), f32(j));
            let seed = o + vec2<f32>(surface_hash(cell + o), surface_hash(cell + o + vec2<f32>(19.0))) * 0.8 + 0.1;
            let d = length(seed - local);
            if d < d1 {
                d2 = d1;
                d1 = d;
                id = cell + o;
                toward = seed - local;
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    let tone = surface_hash(id + vec2<f32>(4.4, 9.1));
    // Light falls across each shard from its upper-left to its lower-right.
    let gradient = clamp(0.5 - dot(toward, vec2<f32>(0.6, 0.8)) * 0.9, 0.0, 1.0);
    // One texel wide: the gap d2 - d1 is twice the distance to the shared edge.
    let edge = step(d2 - d1, 0.9 / CRYSTAL_FACET_TEXELS);
    return vec4<f32>(tone, gradient, edge, 0.5 + 0.5 * surface_hash(id + vec2<f32>(7.7, 1.3)));
}

fn block_surface(base: vec3<f32>, p: vec3<f32>, normal: vec3<f32>, kind: i32, footprint: f32) -> vec3<f32> {
    if kind==0 || kind==1 || kind==2 || kind==3 || kind==4 || kind==7 || kind==16 { return natural_surface(base,p,normal,kind,footprint); }
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
        color = mix(broadleaf, needles, pine) * (1.04 + (clusters-0.5) * 0.16 * medium_fade
            + leaf_light * fine_fade + grain * 0.10 + leaf_px * 0.30)
            * mix(vec3<f32>(1.0), leaf_tint,
                surface_detail_weight(metric_footprint * PIXEL_RES) * 0.45);
        // Per-leaf hue drift: some leaves run yellow-green, some cool and dark.
        // Stays within the canopy's hue family and fades with the footprint.
        let leaf_drift = surface_hash(leaf_cell + vec2<f32>(13.1, -5.3));
        let leaf_tone = mix(vec3<f32>(1.10, 1.03, 0.72), vec3<f32>(0.84, 0.96, 0.93), leaf_drift);
        color *= mix(vec3<f32>(1.0), leaf_tone, surface_detail_weight(metric_footprint * PIXEL_RES) * 0.8);
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
        color *= (1.0-joint*0.58*medium_fade)*(1.0+mottling*0.4);
    } else if kind == 14 || kind == 15 || kind == 20 || kind == 21 {
        let deposit = surface_noise(uv * vec2<f32>(4.0, 2.0));
        let fleck = surface_hash(floor(uv * PIXEL_RES));
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
        color *= 0.8 + surface_noise_filtered(vec2<f32>(uv.x*12.0,uv.y*2.0), metric_footprint*12.0)*0.35;
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
        let f = crystal_facets(uv);
        let shade = clamp(0.10 + f.x * 0.30 + (f.y - 0.5) * 0.45, 0.0, 1.0);
        let shadow = color * vec3<f32>(0.34, 0.26, 0.90) * 0.55;
        let light = mix(color, vec3<f32>(0.90, 0.68, 1.0), 0.30);
        var shard = mix(shadow, light, shade);
        shard = mix(shard, vec3<f32>(0.66, 0.60, 1.0), f.z * f.w * f.w * 0.42);
        color = mix(color, shard, medium_fade);
    } else if kind == 52 {
        // Void stone: nearly black, with a violet marbling that only shows in torchlight.
        let marble = surface_noise_filtered(physical_uv * 1.7 + vec2<f32>(7.0, 3.0), metric_footprint * 1.7);
        color = mix(base * 0.78, vec3<f32>(0.26, 0.18, 0.28), marble * 0.42);
    } else if kind == 53 {
        // Star glass: faceted tiles with a cold sheen that slides across them.
        let tile = fract(physical_uv * 2.0);
        let edge = min(min(tile.x, tile.y), min(1.0 - tile.x, 1.0 - tile.y));
        let seam = 1.0 - smoothstep(0.02, 0.07, edge);
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
    var albedo = clamp(in.base_color, vec3<f32>(0.0), vec3<f32>(1.0));
    let kind = i32(round(in.surface.z));
    let footprint = fp_m / scale;
    let material_tint=in.material_tint;
    let material_props=in.material_props;
    var roughness = clamp(select(in.surface.x,material_tint.w,material_tint.w>=0.0), 0.08, 1.0);
    var reflectance = clamp(material_props.x,0.0,1.0);
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
        in.surface.w >= 4.0 && kind != 40 && in.ao_strength >= 0.0);
    if kind == 8 && in.surface.w >= 8.0 && in.ao_strength >= 0.0 {
        sky_access = in.water_optics.y;
    }

    let geometric_normal = terrain_safe_unit3(in.normal, vec3<f32>(0.0, 1.0, 0.0));
    var n = geometric_normal;
    var filtered_slope_variance = 0.0;
    let is_backdrop = terrain_is_backdrop(in.ao_strength);
    let is_natural = kind == 0 || kind == 1 || kind == 2 || kind == 3
        || kind == 4 || kind == 7 || kind == 16;
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
    if is_backdrop && kind == 0 {
        // Coverage is continuous over a coarse triangle. No interpolated ID is
        // rounded into an unrelated material halfway across a mountain.
        detail_color = natural_surface_blended(albedo, in.voxel_pos,
            geometric_normal, in.terrain_weights, footprint);
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
    if kind != 8 && kind != 9 && kind != 40 && kind != 42 && !is_torch_material && !is_viewmodel {
        albedo = terrain_pixel_albedo(albedo, world_pos, geometric_normal, fp_m);
    }
    if is_natural {
        let relief = far_shade(geometric_normal, in.voxel_pos.xz * scale, fp_m);
        n = terrain_safe_unit3(mix(geometric_normal, relief, min(detail_strength, 1.0)), geometric_normal);
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
    if kind != 6 && kind != 8 && kind != 9 && !is_backdrop && !is_viewmodel
        && relief_strength > 0.02
    {
        n = surface_relief(n, world_pos, fp_m, relief_strength);
        filtered_slope_variance += 0.0016 * relief_strength * relief_strength;
        if kind == 22 {
            let shimmer = vec3<f32>(
                sin(world_pos.y * 11.0 + light.torch_meta.y * 1.7),
                cos(world_pos.x * 9.0 - light.torch_meta.y * 1.3),
                sin(world_pos.z * 13.0 + light.torch_meta.y * 1.9),
            ) * 0.10;
            n = terrain_safe_unit3(n + shimmer, geometric_normal);
        }
    }
    if !is_backdrop && sky_access < 0.80 {
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
    if kind != 8 && kind != 9 && kind != 54 && kind != 55 {
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
    let water_side = kind == 8 && n.y <= 0.5 && in.surface.w >= 8.0 && in.ao_strength >= 0.0;
    if kind == 8 && (n.y > 0.5 || water_side) {
        let t = light.torch_meta.y;
        let p = world_pos.xz;
        let fp = footprint * cam.world_params.x;

        // The far water lattice carries bed depth in [0,3], separate from
        // resident sky-access metadata [4,5].
        let is_far = in.ao_strength < 0.0 && in.ao_strength > -1.5;
        // Fade details that the proxy cannot represent before the shared handoff.
        // This does not substitute for equal bed depths from the terrain source.
        let handoff_start = max(terrain_backdrop_start() - terrain_overlap_width() * 0.50, 0.0);
        let handoff_end = max(terrain_backdrop_start(), handoff_start + scale);
        let local_water = select(1.0, 0.0, is_far)
            * (1.0 - smoothstep(handoff_start, handoff_end, length(relative_pos.xz)));
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

        let gradient = gerstner_grad + fbm_grad;
        n = normalize(n + select(vec3<f32>(gradient.x, 0.0, gradient.y), vec3<f32>(0.0), water_side));

        let jac_compress = (0.34 * cos(ph0) + 0.31 * cos(ph1)) * w0
                         + (0.27 * cos(ph2) + 0.24 * cos(ph3)) * w1;
        water_jacobian = clamp(1.0 - jac_compress * 0.55 * shoal_boost, 0.35, 1.65);

        // Snell's Law refraction (n_air / n_water = 1.0 / 1.333 = 0.75) shifts
        // the apparent seabed position and drives refracted sand ripple detail.
        let v_inc = terrain_safe_unit3(relative_pos, vec3<f32>(0.0, -1.0, 0.0));
        let refracted = refract(v_inc, n, 0.75);
        let refract_depth = min(depth, 4.2);
        let parallax_xz = (refracted.xz - v_inc.xz) * (refract_depth / max(-refracted.y, 0.24)) * local_water;
        let bed_p = p + parallax_xz;

        let turbidity = surface_noise_filtered(p / 340.0, fp / 340.0);
        let sand_bed = mix(vec3<f32>(0.185, 0.168, 0.108), vec3<f32>(0.105, 0.178, 0.148), turbidity);
        let turquoise_mid = mix(vec3<f32>(0.06, 0.16, 0.20), vec3<f32>(0.05, 0.14, 0.19), turbidity);
        let deep_ocean = mix(vec3<f32>(0.020, 0.055, 0.080), vec3<f32>(0.016, 0.045, 0.070), turbidity);
        let shallow_t = 1.0 - exp(-depth * 0.62);
        let deep_t = 1.0 - exp(-max(depth - 1.2, 0.0) * 0.18);
        var water_col = mix(sand_bed, turquoise_mid, shallow_t);
        water_col = mix(water_col, deep_ocean, deep_t);

        if local_water > 0.0 {
            // Refracted seabed ripples & mineral grain visible through clear shallows.
            let bed_ripple = sin(dot(bed_p, vec2<f32>(0.82, 0.57)) * 3.6 + surface_noise(bed_p * 0.9) * 3.2)
                * surface_detail_weight(fp * 3.0);
            let bed_grain = surface_noise_filtered(bed_p * 4.2, fp * 6.5);
            // Bed and caustic patterns are functions of xz only: on a wall they would
            // smear into vertical streaks, so walls keep the flat body colour.
            let bed_detail = select((0.91 + 0.09 * bed_ripple + 0.06 * (bed_grain - 0.5)) * (0.94 + 0.08 * cusp_x * w2), 1.0, water_side);
            let refracted_bed = albedo * bed_detail;

            // Physical Beer-Lambert spectral absorption along the refracted ray + solar path.
            let optical_path = depth * (1.0 / max(-refracted.y, 0.22) + 0.45 / max(light.sun_dir_intensity.y + 0.28, 0.25));
            let transmission = exp(-vec3<f32>(0.46, 0.24, 0.13) * optical_path);
            water_col = mix(water_col, water_col * (vec3<f32>(1.0) - transmission) + refracted_bed * transmission, local_water);
        }

        // Dual-layer refracted sun caustics. One luminance web tinted by a fixed
        // sun colour: per-channel offsets drew rainbow fringes along every
        // caustic edge, which real shallow water does not show at this scale.
        if !water_side {
            let shallow_vis = exp(-depth * 0.52) * w2;
            let c_uv = bed_p * 1.52 + n.xz * (refract_depth * 0.85);
            let ca = surface_noise(c_uv + vec2<f32>(t * 0.44, -t * 0.32));
            let cb = surface_noise((rot1 * c_uv) * 1.14 + vec2<f32>(-t * 0.38, t * 0.49));
            let caustic = smoothstep(0.58, 0.93, 1.0 - abs(ca - cb) * 2.25);
            water_col += caustic * vec3<f32>(0.105, 0.145, 0.125) * shallow_vis;
        }

        // Cellular perforated shoreline swash foam & Jacobian-driven breaking whitecaps.
        let bubble_fine = surface_noise_filtered(bed_p * 5.8 + n1.yz * 1.1 + vec2<f32>(t * 0.25, -t * 0.19), fp * 12.0);
        let bubble_mid  = surface_noise_filtered((rot2 * bed_p) * 2.7 - n0.yz * 0.9, fp * 6.0);
        let foam_lace = smoothstep(0.28, 0.76, bubble_fine * 0.62 + bubble_mid * 0.38);

        // A few wave sets arrive on a slow envelope. The depth contours act as
        // bathymetry, so a moving break band reads as a wave front travelling
        // shoreward even though the surface mesh is only displaced per vertex.
        let surf_dir = vec2<f32>(0.54, -0.84);
        let alongshore = dot(p, surf_dir);
        let set_env = pow(0.5 + 0.5 * sin(alongshore * 0.055 + t * 0.62), 3.0);
        let front0 = 1.0 - smoothstep(0.0, 0.11,
            abs(depth - (0.20 + 0.09 * set_env)));
        let front1 = 1.0 - smoothstep(0.0, 0.18,
            abs(depth - (0.58 + 0.16 * sin(alongshore * 0.043 - t * 0.51))));
        let break_front = max(front0, front1 * 0.72) * (0.35 + 0.65 * set_env);

        // Wind Waker-inspired shore contact: a saturated shallow turquoise shelf
        // followed by a darker waterline and crisp, breathing foam lace. This is
        // driven by bed depth, so it hugs islands and voxel coastlines without a
        // second depth buffer or a per-object contact pass.
        let turquoise_shelf = (1.0 - smoothstep(0.04, 0.68, depth)) * local_water;
        water_col = mix(
            water_col,
            vec3<f32>(0.14, 0.32, 0.38),
            turquoise_shelf * 0.24,
        );
        let contact_edge = (1.0 - smoothstep(0.012, 0.11, depth)) * local_water;
        let contact_ripple = 0.5 + 0.5 * sin(depth * 26.0 - t * 3.6 + n2.y * 4.0);
        let contact_lace = smoothstep(
            0.10,
            0.88,
            foam_lace * 0.55 + contact_ripple * 0.45,
        );
        let contact_foam = contact_edge * (0.44 + 0.56 * contact_lace);
        let waterline = (1.0 - smoothstep(0.005, 0.055, depth)) * local_water;
        water_col = mix(
            water_col,
            vec3<f32>(0.04, 0.10, 0.14),
            waterline * 0.35,
        );

        // Swash foam hugs the contact line (about 40 cm of depth), not every shallow.
        let shore_band = (1.0 - smoothstep(0.04, 0.40, depth)) * mix(0.30, 1.0, local_water);
        let surf_ripple = sin(depth * 14.0 - t * 3.2 + alongshore * 0.18 + n0.x * 5.0 + n1.x * 3.2);
        let swash_crest = smoothstep(-0.12, 0.58, surf_ripple + (n2.x - 0.45) * 0.85);
        let run_up = 0.5 + 0.5 * sin(alongshore * 0.085 + t * 0.88 + n2.y * 1.8);
        let lip_contact = (1.0 - smoothstep(0.02, 0.08 + 0.18 * run_up, depth)) * local_water;
        let shore_foam = (
            shore_band * swash_crest * (0.46 + 0.54 * foam_lace)
            + lip_contact * (0.48 + 0.52 * foam_lace)
            + contact_foam * 0.58
            + break_front * (0.36 + 0.64 * foam_lace)
        ) * (0.45 + 0.55 * w1);

        let crest_strain = max(1.0 - water_jacobian, 0.0) + length(gradient) * 2.7;
        let crest_whitecap = smoothstep(0.34, 0.62, crest_strain)
            * (0.38 + 0.62 * foam_lace) * (1.0 - shore_band) * 0.44 * w1;
        let whitecap = crest_whitecap
            + break_front * (0.34 + 0.66 * foam_lace) * 0.55 * w1;
        // The water body keeps its continuous optical colour: posterizing it
        // produced visible contour bands across smooth depth gradients.
        // Continuous foam: a smoothstep keeps the lace crisp without the five hard
        // coverage steps, which showed as contour bands across shallow water.
        // Walls read as a water body, not a flat slab: deeper toward the bed,
        // a bright meniscus under the surface. voxel_pos is in voxel units, so
        // the fraction is the height inside one voxel at any voxel scale.
        if water_side {
            let wall_h = fract(in.voxel_pos.y);
            let body_fade = mix(0.58, 1.0, smoothstep(0.0, 0.9, wall_h));
            let meniscus = smoothstep(0.80, 0.99, wall_h);
            water_col = water_col * body_fade;
            water_col = mix(water_col, vec3<f32>(0.66, 0.84, 0.84), meniscus * 0.55);
        }
        water_foam = select(0.90 * smoothstep(0.10, 0.90, shore_foam * 0.62 + whitecap), 0.0, water_side);
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
    roughness = terrain_filter_roughness(roughness, normal_variance);
    // Distant water uses the same finish as resident water. There is no blanket
    // backdrop roughness floor or extra (1-r)^2 specular suppression.
    let metallic = select(0.0, 1.0, kind >= 47 && kind <= 51);
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

    let dist = length(relative_pos);
    let horizontal_dist = length(relative_pos.xz);
    // The heightfield is a distant fallback, not a replacement. It is sunk
    // below the detailed surface and depth-tested, so beyond the reliable local
    // stream it safely fills a missing resident chunk with continuous ground
    // instead of exposing an X-ray hole straight to sky. Never draw it in the
    // near field: a heightfield cannot represent the local excavation, and
    // doing so produced a giant backdrop wall beside the player.
    let backdrop_start = terrain_backdrop_start();
    let backdrop_near_clear = 96.0 * cam.world_params.x;
    if is_backdrop && kind != 5 && kind != 6
        && horizontal_dist < backdrop_near_clear { discard; }
    // Foliage proxies still start near the handoff so they do not double the
    // resident tree crowns.
    if is_backdrop && (kind == 5 || kind == 6)
        && horizontal_dist < backdrop_start - 48.0 * cam.world_params.x { discard; }
    // Resident terrain is never discarded past the handoff. The old outer-band
    // discard was visible as translucent ground whenever streaming, a remesh,
    // or an eviction lagged behind the camera.
    // Depth selects detailed chunks over the lowered backdrop without a
    // stippled material transition.

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
    let foliage = kind == 6;
    let sun_lambert  = max(dot(n, sun_dir), 0.0);
    let moon_lambert = max(dot(n, moon_dir), 0.0);
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
    }

    if debug_mode == 11 { return debug_surface_output(vec4<f32>(vec3<f32>(shadow_visibility), 1.0)); }

    let sun_shadow = mix(1.0, shadow_visibility, sun_shadow_strength);

    // Retain geometric horizon visibility when small shading normals bend past
    // a face. Without it, relief-perturbed normals light slopes that face away
    // from the sun, which shows up beyond the shadow range. Foliage wraps its
    // light on purpose, so it is exempt.
    let sun_horizon = smoothstep(-0.015, 0.015, dot(geometric_normal, sun_dir));
    let moon_horizon = smoothstep(-0.015, 0.015, dot(geometric_normal, moon_dir));
    // Oren–Nayar for rough surfaces, Lambertian for smooth.
    let sun_diffuse = select(mix(sun_lambert,
                          oren_nayar(n, to_eye, sun_dir, roughness),
                          roughness * 0.22) * sun_horizon, foliage_sun, foliage)
                      * light.sun_dir_intensity.w * sun_trans * sun_shadow;
    let moon_diffuse = select(mix(moon_lambert,
                           oren_nayar(n, to_eye, moon_dir, roughness),
                           roughness * 0.16) * moon_horizon, foliage_moon, foliage)
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
    let raw_ao = select(clamp(in.ao_strength, 0.0, 1.0), 1.0, in.ao_strength < 0.0);
    let shaped_ao   = pow(raw_ao, 1.35);
    let cavity      = mix(mix(0.32, 1.0, shaped_ao), 1.0, hemi * 0.08);
    let ambient     = (
        light.ambient_exposure.rgb * mix(vec3<f32>(0.38), vec3<f32>(1.0), hemi * hemi)
        + mix(terrain_sky_ambient,
              vec3<f32>(dot(terrain_sky_ambient, vec3<f32>(0.2126, 0.7152, 0.0722))),
              cloudiness * 0.18) * sky_fill_strength
        + ground_col * ground_amt + dusk_sky_fill
    ) * cavity * mix(1.0, 0.86, cloudiness * daylight * 0.55);
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
    if kind == 42 {
        // Black-body ramp: deep orange edges, orange body and a small warm
        // core. Keeping the ramp in HDR preserves dark contrast between flame
        // cells. The body keeps little green: at 0.31 with a full white core,
        // the hottest voxels summed to near-white and read as pale salmon.
        let edge = vec3<f32>(0.75, 0.055, 0.006);
        let body = vec3<f32>(1.0, 0.18, 0.015);
        let core = vec3<f32>(1.0, 0.60, 0.25);
        let heat2 = fire_heat * fire_heat;
        let heat3 = heat2 * fire_heat;
        emissive += (mix(edge, body, fire_heat) * 3.1 + core * heat3 * 0.5) * fire_exposure;
        albedo = mix(vec3<f32>(0.16, 0.028, 0.004), vec3<f32>(0.95, 0.36, 0.05), heat2);
    }
    if kind == 43 { emissive += albedo * 0.10 * fire_exposure; }
    if kind == 44 { emissive += albedo * 0.18 * fire_exposure; }
    if kind == 51 { emissive += albedo * 0.42; }
    // Gems and ore catch the light: a faint sweep of facet glints as you move.
    if kind == 22 || kind == 14 || kind == 15 || kind == 20 || kind == 21 || (kind >= 47 && kind <= 51) {
        let sheen = select(0.0, crystal_sheen(world_pos, light.torch_meta.y), kind == 22);
        let glint = mood_glint(in.voxel_pos / material_scale, geometric_normal, -view_dir, sun_dir, light.torch_meta.y, select(7.0, PIXEL_RES, kind == 22), sheen);
        let gem = select(vec3<f32>(1.0, 0.95, 0.85), vec3<f32>(0.85, 0.65, 1.0), kind == 22);
        // Glints run toward white-hot: a gem tint at full gain tonemaps to a
        // pale wash against its own body colour.
        emissive += mix(gem, vec3<f32>(1.0), 0.5) * glint * select(0.7, 1.1, kind == 22) * fire_exposure;
        // The same band passing over crystal also lifts a soft inner gleam.
        emissive += gem * sheen * 0.08 * fire_exposure;
    }
    if kind == 52 || kind == 53 {
        let stars = void_star_mask(in.voxel_pos / material_scale, geometric_normal, kind, light.torch_meta.y);
        let warmth = surface_hash(floor(in.voxel_pos.xz * 0.5));
        emissive += mix(vec3<f32>(0.55, 0.68, 1.0), vec3<f32>(1.0, 0.84, 0.62), warmth) * stars
            * select(1.3, 2.4, kind == 53) * fire_exposure;
        // A faint inner light so the room reads as a place, not just a backdrop.
        emissive += albedo * select(0.55, 1.6, kind == 53) * fire_exposure;
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
        // base_color is already linear (decoded in the vertex stage).
        emissive += max(in.base_color, vec3<f32>(0.0)) * select(0.65, 3.2, flame) * flame_wave * fire_exposure;
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
    let torch_count = select(0u, u32(clamp(light.torch_meta.x, 0.0, 64.0)), receives_local_light);
    for (var i: u32 = 0u; i < 64u; i = i + 1u) {
        if i >= torch_count { break; }
        let ts = eval_torch_light(
            light.torch_positions[i], light.torch_colors[i],
            light.torch_meta.y, world_pos, n, to_eye, roughness, f0, metallic, foliage,
        );
        torch_diffuse += ts.diffuse;
        torch_fill    += ts.fill;
        torch_spec    += ts.spec;
    }
    if receives_local_light && light.held_torch_pos_radius.w > 0.0 && light.held_torch_color_strength.w > 0.0 {
        let ht = eval_torch_light(
            light.held_torch_pos_radius, light.held_torch_color_strength,
            light.torch_meta.y, world_pos, n, to_eye, roughness, f0, metallic, foliage,
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
    // Real inter-ridge occlusion still requires distant shadow casters.
    let direct_specular = sun_radiance * terrain_specular(n, to_eye, sun_dir, roughness, f0)
            * max(light.sun_dir_intensity.w, 0.0) * sun_trans * sun_shadow * sun_horizon
        + moon_radiance * terrain_specular(n, to_eye, moon_dir, roughness, f0)
            * max(light.moon_dir_intensity.w, 0.0) * moon_trans * moon_horizon * sky_access
        + torch_spec * fire_exposure;

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

    // Water caustic bounce on the immediate shore. The seabed already receives
    // caustics inside the water pass; this carries the same pixel-quantized web
    // onto low banks, wet sand and nearby rocks without requiring a second
    // scene buffer. It is deliberately dim and sun-gated.
    let sea_level = cam.coverage.y;
    if kind != 8 && kind != 9 && kind != 40 && sea_level > -64.0 {
        let delta = world_pos.y - sea_level;
        let above_water = smoothstep(-0.02, 0.18, delta);
        let near_shore = 1.0 - smoothstep(0.25, 1.55, abs(delta));
        if near_shore > 0.002 {
            let t = light.torch_meta.y;
            let web_a = surface_noise(world_pos.xz * 0.72 + vec2<f32>(t * 0.17, -t * 0.13));
            let web_b = surface_noise(world_pos.xz * 0.93 + vec2<f32>(-t * 0.11, t * 0.19));
            let web = smoothstep(0.56, 0.90, 1.0 - abs(web_a - web_b) * 2.2);
            lit += albedo * sun_radiance * web
                * near_shore * above_water * sun_shadow * sun_trans * 0.10;
        }
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
    if kind == 22 {
        // Crystal facets catch the torch at a different phase per face and
        // leak a small violet halo into the surrounding cavern.
        let facet = 0.5 + 0.5 * sin(
            dot(world_pos, vec3<f32>(0.9, 1.3, 0.7)) + light.torch_meta.y * 0.9
        );
        // Thin crystal glows from inside and brightens at grazing view. A sharp
        // sun specular on the facets gives the glint that makes it read as gem.
        let view_fresnel = terrain_pow5(1.0 - clamp(dot(n, to_eye), 0.0, 1.0));
        let iris = mix(albedo, albedo.brg, 0.55);
        let sparkle = min(terrain_specular(n, to_eye, sun_dir, 0.08, vec3<f32>(0.06)), vec3<f32>(6.0))
            * sun_radiance * sun_shadow * sun_trans;
        lit += (iris * (0.10 + facet * 0.14 + view_fresnel * 0.38)
            + vec3<f32>(0.006, 0.004, 0.012) * facet) * fire_exposure
            + sparkle * vec3<f32>(1.0, 0.92, 1.0) * 0.35;
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
            * sun_trans * sun_shadow * max(light.sun_dir_intensity.w, 0.0)
            * sky_access
            * (1.0 - water_foam * 0.75);
        lit += sss_tint * light.sun_color_daylight.rgb * sun_spectral * sss_energy * 0.17;

        // Sky radiance already follows time of day. Dimming it again made
        // night water read as black cutout shadows against illuminated banks.
        // No reflection floor or cap: Schlick approaches one at grazing angles.
        lit = mix(lit, reflection * sky_access, clamp(fresnel, 0.0, 1.0));
        lit += direct_specular * (1.0 - water_foam * 0.65);
    }

    if kind == 10 {
        let crust = surface_noise_filtered(world_pos.xz * 2.8, fp_m * 2.8);
        let molten = smoothstep(0.38, 0.72, crust);
        lit += mix(vec3<f32>(0.55, 0.035, 0.003), vec3<f32>(2.8, 0.65, 0.035), molten) * fire_exposure;
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
    let cloud_fog_radiance = max(mix(sky_horizon, sky_up, 0.50), vec3<f32>(0.0));
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
    let edge_fade = terrain_streaming_fade(horizontal_dist);
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

