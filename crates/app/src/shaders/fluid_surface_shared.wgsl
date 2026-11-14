// ============================================================
// fluid_surface_shared.wgsl
//
// Surface looks for water, fire, smoke, steam and loose powders. Animated
// terms are pure functions of position and the shared clock.
// Colours are linear reflectance or unit-scale emission, before lighting.
// ============================================================

// Bed light loses red first and keeps jade through the shallows. The separate
// in-scatter ramp supplies the blue of deep water once the bed fades out.
const WATER_ABSORB: vec3<f32> = vec3<f32>(1.05, 0.20, 0.27);
// Reflections of sky and scene read darker than their sources.
const WATER_REFLECTION_GAIN: f32 = 0.80;
const WATER_FOAM: vec3<f32> = vec3<f32>(0.80, 0.83, 0.79);

// Both resident and horizon surfaces use the same sea datum. Use the
// undisplaced height so a wave never changes its own material.
fn water_marine_weight(height: f32, scale: f32) -> f32 {
    let sea_surface = cam.coverage.y + scale;
    return 1.0 - smoothstep(0.15 * scale, 0.65 * scale, abs(height - sea_surface));
}

/// In-scattered colour of the water column above a bed `depth` metres down.
fn water_body_color(depth: f32, turbidity: f32, marine: f32) -> vec3<f32> {
    // Linear-light ramps for the reference brief's jade, teal and navy.
    let shallow = mix(vec3<f32>(0.042, 0.205, 0.156), vec3<f32>(0.085, 0.177, 0.123), turbidity);
    let mid = mix(vec3<f32>(0.026, 0.104, 0.087), vec3<f32>(0.018, 0.116, 0.119), marine);
    let clear_deep = mix(vec3<f32>(0.014, 0.050, 0.059), vec3<f32>(0.006, 0.054, 0.091), marine);
    let deep = mix(clear_deep, vec3<f32>(0.023, 0.037, 0.040), turbidity);
    let body = mix(shallow, mid, smoothstep(0.15, 2.4, depth));
    return mix(body, deep, 1.0 - exp(-max(depth - 2.0, 0.0) * 0.33));
}

/// One swell train: xy slope, z unresolved slope variance, w crest compression.
fn sea_swell_term(xz: vec2<f32>, dir: vec2<f32>, wavelength: f32, slope: f32, t: f32, fp: f32) -> vec4<f32> {
    let k = 6.2831853 / wavelength;
    // Deep-water dispersion, w = sqrt(g k).
    let w = sqrt(9.81 * k);
    let along = dot(xz, dir);
    let across = dot(xz, vec2<f32>(-dir.y, dir.x));
    let packet = k * along * 0.26 - w * t * 0.13 + k * 9.7;
    let flank = k * across * 0.55 + k * 37.0;
    let envelope = 0.45 + 0.38 * cos(packet + 0.65 * sin(flank)) * cos(flank - packet * 0.23);
    let resolved = (1.0 - smoothstep(0.65, 2.0, fp * k))
        * (1.0 - smoothstep(1.5, 8.0, fp));
    let ph = along * k - w * t + 0.90 * sin(flank) + 0.38 * sin(packet);
    // Trochoidal profile: steep crests, wide troughs.
    let crest = cos(ph);
    let s = sin(ph) / (1.0 - 0.3 * crest);
    return vec4<f32>(dir * (s * slope * envelope * resolved),
        0.5 * slope * slope * (1.0 - resolved * resolved), crest * envelope);
}

// A narrow directional band with a slower energy envelope. Phase and group
// velocity differ, so crests grow and die as they move through each packet.
// xy = negative height gradient, z = height, w = horizontal compression.
fn water_wave_packet(
    xz: vec2<f32>, dir: vec2<f32>, k: f32, amplitude: f32, phase_offset: f32,
    t: f32, footprint: f32,
) -> vec4<f32> {
    let omega = sqrt(9.81 * k);
    let across = dot(xz, vec2<f32>(-dir.y, dir.x));
    let along = dot(xz, dir);
    let group = k * along * 0.19 - omega * t * 0.095 + phase_offset;
    let flank = k * across * 0.28 + phase_offset * 1.7;
    let envelope = 0.52 + 0.38 * cos(group) * cos(flank + group * 0.21);
    let phase = k * along - omega * t + phase_offset;
    let resolved = 1.0 - smoothstep(0.9, 2.6, k * footprint);
    let height = amplitude * envelope * resolved;
    let crest = cos(phase);
    let slope = sin(phase) * (height * k);
    return vec4<f32>(dir * slope, height * crest, height * k * crest);
}

fn water_surface_packets(xz: vec2<f32>, t: f32, footprint: f32) -> vec4<f32> {
    // One continuous world-space warp breaks the four trains' repeated cross
    // pattern. Shared by displacement and normals, with no per-tile UV reset.
    // It replaces eight nested sine evaluations in the packet phase/envelope.
    let drift = vec2<f32>(0.37, -0.23) * t;
    let bend = vec2<f32>(surface_noise((xz - drift) * 0.013),
        surface_noise((xz - drift) * 0.019 + vec2<f32>(31.7, -19.3))) - 0.5;
    let q = xz + bend * 5.0;
    return water_wave_packet(q, vec2<f32>(0.88, 0.475), 0.24, 0.31, 0.3, t, footprint)
        + water_wave_packet(q, vec2<f32>(0.31, -0.951), 0.37, 0.20, 1.8, t, footprint)
        + water_wave_packet(q, vec2<f32>(-0.69, 0.724), 0.59, 0.115, 3.2, t, footprint)
        + water_wave_packet(q, vec2<f32>(-0.54, -0.842), 0.91, 0.057, 4.9, t, footprint);
}

/// Caustic gain on a submerged bed, with a bright core and softer shoulders.
fn water_bed_caustic(bed_xz: vec2<f32>, t: f32, depth: f32, footprint_m: f32) -> f32 {
    let shallow = exp(-depth * 0.55) * smoothstep(0.02, 0.15, depth);
    let resolved = fluid_pixel_weight(footprint_m);
    // Unresolved lines keep their average lift instead of shimmering.
    let mean = 0.045;
    if resolved <= 0.0 { return mean * shallow; }
    let q = texel_centre(bed_xz);
    let r = mat2x2<f32>(0.80, -0.60, 0.60, 0.80);
    let a = uw_noise(q * 2.2 + vec2<f32>(t * 0.31, -t * 0.22));
    let b = uw_noise((r * q) * 2.9 + vec2<f32>(-t * 0.27, t * 0.35));
    let separation = abs(a - b);
    let shoulder = 1.0 - smoothstep(0.025, 0.14, separation);
    let core = 1.0 - smoothstep(0.012, 0.052, separation);
    return mix(mean, shoulder * 0.16 + core * 0.52, resolved) * shallow;
}

// Depth steers the fronts into the bank. Integrating the shallow-water
// 1/sqrt(depth) travel time over a local beach slope bunches them in the
// shallows. World-space modulation bends and interrupts adjacent fronts.
// Returns crest phase and group strength for both displacement and foam.
fn water_breaker(xz: vec2<f32>, depth: f32, t: f32) -> vec2<f32> {
    let drift = vec2<f32>(0.27, -0.19) * t;
    let group = surface_noise((xz - drift) * 0.094);
    let bend = surface_noise(xz * 0.165 + vec2<f32>(0.015, 0.027) * t);
    let travel = (sqrt(max(depth, 0.0) + 0.09) - 0.3) * 0.98;
    let clock = t * 0.29 + 0.14 * sin(t * 0.67 + group * 4.0);
    let phase = fract(travel + clock + (bend - 0.5) * 0.55 + (group - 0.5) * 0.45);
    return vec2<f32>(phase, smoothstep(0.24, 0.76, group));
}

fn water_breaker_band(depth: f32) -> f32 {
    return smoothstep(0.04, 0.22, depth) * (1.0 - smoothstep(0.55, 1.50, depth));
}

fn water_breaker_crest(phase: f32) -> f32 {
    return smoothstep(0.32, 0.54, phase) * (1.0 - smoothstep(0.54, 0.69, phase));
}

// Reconstruct a tiny displacement of the interpolated bed colour. The offset
// is bounded to one pixel because the mesh carries only local bed information.
fn water_refract_bed(
    bed: vec3<f32>, offset: vec2<f32>, chart_dx: vec2<f32>, chart_dy: vec2<f32>,
    bed_dx: vec3<f32>, bed_dy: vec3<f32>,
) -> vec3<f32> {
    let det = chart_dx.x * chart_dy.y - chart_dx.y * chart_dy.x;
    if abs(det) < 1e-8 { return bed; }
    let pixels = vec2<f32>(
        offset.x * chart_dy.y - offset.y * chart_dy.x,
        chart_dx.x * offset.y - chart_dx.y * offset.x,
    ) / det;
    let step_px = pixels / max(1.0, length(pixels));
    return clamp(bed + bed_dx * step_px.x + bed_dy * step_px.y, vec3<f32>(0.0), vec3<f32>(1.0));
}

/// Foam coverage from the interpolated pinned-corner weight (1 on a solid).
fn water_shore_foam(xz: vec2<f32>, contact: f32, current: vec2<f32>, t: f32, footprint_m: f32) -> f32 {
    // The widest swell and noise perturbation still cannot reach the band here.
    if contact <= 0.35 { return 0.0; }
    let resolved = fluid_pixel_weight(footprint_m);
    let mean = smoothstep(0.35, 1.0, contact) * 0.35;
    if resolved <= 0.0 { return mean; }
    let q = texel_centre(xz);
    let phase = fract(t * 0.25);
    // At 5 cycles/m a noise cell was about three texels, so the thresholded
    // lace broke into isolated one-texel white specks. A lower frequency
    // keeps the foam in connected strands several texels wide.
    let a = surface_noise((q - current * (phase * 4.0)) * 1.7);
    let b = surface_noise((q - current * (fract(phase + 0.5) * 4.0)) * 1.7);
    let lace = mix(a, b, smoothstep(0.0, 1.0, abs(phase * 2.0 - 1.0)));
    let rim = smoothstep(0.65, 0.96, contact + (lace - 0.5) * 0.27);
    let coverage = rim * smoothstep(0.30, 0.62, lace);
    return mix(mean, coverage, resolved);
}

// ---- Fire -----------------------------------------------------------------

// Hard one-texel patterns (flame pixels, gas dither) keep full contrast until a
// texel is under about 1.5 screen pixels, later than surface_detail_weight's
// smooth-noise fade; below one pixel they alias, so they settle to the mean.
fn fluid_pixel_weight(footprint_m: f32) -> f32 {
    return 1.0 - smoothstep(0.65, 1.1, max(footprint_m * PIXEL_RES, 0.0));
}

/// Pixel fire palette at unit emission: red edge, #F07A1A..#FFB23A body, #FFE9A0 core.
fn pixel_fire_ramp(heat: f32) -> vec3<f32> {
    let edge = vec3<f32>(0.50, 0.045, 0.008);
    let body_lo = vec3<f32>(0.87, 0.19, 0.010);
    let body_hi = vec3<f32>(1.0, 0.44, 0.042);
    let core = vec3<f32>(1.0, 0.81, 0.35);
    var c = mix(edge, body_lo, smoothstep(0.18, 0.45, heat));
    c = mix(c, body_hi, smoothstep(0.45, 0.72, heat));
    return mix(c, core, smoothstep(0.72, 0.95, heat));
}

// Face-plane axes in voxel units: x across, y up the face (world up on walls).
fn fluid_face_uv(p: vec3<f32>, n: vec3<f32>) -> vec2<f32> {
    let a = abs(n);
    if a.x > a.y && a.x > a.z { return p.zy; }
    if a.z > a.y { return p.xy; }
    return p.xz;
}

/// Fire heat on one texel of a fire voxel face; below 0.2 the texel is empty.
fn pixel_fire_heat(voxel_pos: vec3<f32>, n: vec3<f32>, scale: f32, t: f32, footprint_m: f32) -> f32 {
    let texel = floor(fluid_face_uv(voxel_pos, n) * scale * PIXEL_RES);
    // Texel centre inside its voxel, 0..1 on both face axes.
    let local = fract((texel + vec2<f32>(0.5)) / (scale * PIXEL_RES));
    let side = abs(n.y) < 0.5;
    // Tongues rise: the noise scrolls up the face, a few texels per second.
    let rise = surface_noise(vec2<f32>(texel.x * 0.33, texel.y * 0.26 - t * 2.6));
    let lick = surface_noise(vec2<f32>(texel.x * 0.61 + 17.0, texel.y * 0.47 - t * 4.1));
    // Crackle: each texel re-rolls ten times a second, never per frame.
    let crackle = surface_hash(texel + vec2<f32>(floor(t * 10.0) * 7.0, 31.0)) - 0.5;
    var shape: f32;
    if side {
        let across = abs(local.x - 0.5) * 2.0;
        shape = 1.18 - local.y * 0.95 - across * across * 0.75;
    } else {
        // Seen from above a flame is its tongue tips, not a solid disc.
        shape = 0.80 - length(local - vec2<f32>(0.5)) * 1.3;
    }
    let up = select(0.5, local.y, side);
    // Sub-pixel texels fall back to the bare flame shape, so distant fire
    // does not shimmer as its texels alias.
    let resolved = fluid_pixel_weight(footprint_m);
    let detail = (rise - 0.5) * (0.55 + 0.35 * up) + (lick - 0.5) * 0.25 + crackle * 0.16;
    return clamp(shape + detail * resolved, 0.0, 1.0);
}

// ---- Smoke and steam -------------------------------------------------------

struct GasPuff {
    normal: vec3<f32>,
    coverage: f32,
    shade: f32,   // 0 underside .. 1 lit top
    density: f32, // per-texel value variation around 1
}

/// Billowed gas on a voxel face: a rising world-anchored billow field that runs
/// across neighbouring voxels, thinned toward the voxel rim by ordered dither.
fn gas_puff(voxel_pos: vec3<f32>, n: vec3<f32>, scale: f32, t: f32, footprint_m: f32) -> GasPuff {
    let face = fluid_face_uv(voxel_pos, n);
    let texel = floor(face * scale * PIXEL_RES);
    let uv = fract((texel + vec2<f32>(0.5)) / (scale * PIXEL_RES)) - vec2<f32>(0.5);
    let side = abs(n.y) < 0.5;
    // Billows climb walls slowly; on tops they drift.
    let climb = select(vec2<f32>(t * 0.05, -t * 0.04), vec2<f32>(0.0, -t * 0.40), side);
    let broad = surface_noise_grad(texel * 0.07 + climb);
    let fine = surface_noise(texel * 0.17 + climb * 2.0 + vec2<f32>(13.0, 5.0));
    let field = broad.x * 0.7 + fine * 0.3;
    // Superellipse distance: 1 on face edges, about 1.19 in the corners.
    let q = abs(uv) * 2.0;
    let q2 = q * q;
    let r = sqrt(sqrt(q2.x * q2.x + q2.y * q2.y));
    let body = 1.0 - smoothstep(0.66, 1.12, r + (field - 0.5) * 0.70);
    // Dense cores stay solid; thin rims dissolve into an ordered pixel dither.
    let mass = body * (0.50 + field * 0.95);
    // The dither threshold settles to its mean once a texel is sub-pixel.
    let resolved = fluid_pixel_weight(footprint_m);
    let coverage = step(mix(0.5, terrain_bayer4(texel) * 0.9 + 0.05, resolved), mass);
    let a = abs(n);
    var across = vec3<f32>(1.0, 0.0, 0.0);
    var along = vec3<f32>(0.0, 1.0, 0.0);
    if a.x > a.y && a.x > a.z { across = vec3<f32>(0.0, 0.0, 1.0); }
    else if a.z <= a.y { along = vec3<f32>(0.0, 0.0, 1.0); }
    // Each voxel bulges, and the broad billows tilt toward their crests.
    let tilt = uv * 1.0 - broad.yz * 0.07 * 6.0;
    let normal = normalize(n + across * tilt.x + along * tilt.y);
    let shade = clamp((normal.y * 0.5 + 0.5) * 0.75 + field * 0.40 - 0.08, 0.0, 1.0);
    let density = 0.93 + 0.06 * surface_hash(texel + vec2<f32>(3.0, 77.0)) + 0.06 * fine;
    return GasPuff(normal, coverage, shade, density);
}

// ---- Powders ---------------------------------------------------------------

/// Per-grain tint for one-texel sand grains: value, warm grains, dark minerals.
fn sand_grain_tint(texel: vec2<f32>) -> vec3<f32> {
    let g = surface_hash(texel + vec2<f32>(37.0, -11.0));
    let pick = surface_hash(texel + vec2<f32>(-5.0, 83.0));
    var tint = vec3<f32>(0.93 + g * 0.14);
    tint *= select(vec3<f32>(1.0), vec3<f32>(1.07, 0.97, 0.84), pick > 0.74);
    tint *= select(vec3<f32>(1.0), vec3<f32>(1.04, 1.04, 1.06), pick < 0.10);
    return select(tint, vec3<f32>(0.64, 0.61, 0.62), pick > 0.972);
}

/// Loose ash: fine grey powder with pale flakes and char specks on the texel grid.
fn ash_surface(base: vec3<f32>, q: vec3<f32>, normal: vec3<f32>, fp: f32) -> vec3<f32> {
    // The authored swatch is near charcoal; loose ash reads as a pale warm grey.
    let grey = mix(vec3<f32>(0.20, 0.192, 0.182), base * 3.0, 0.25);
    let drift = surface_noise_filtered(q.xz * 1.3 + q.y * vec2<f32>(0.21, 0.17), fp * 1.3);
    let mean = grey * (0.94 + drift * 0.12);
    let resolved = surface_detail_weight(fp * PIXEL_RES);
    if resolved <= 0.0 { return mean; }
    let texel = floor(fluid_face_uv(q, normal) * PIXEL_RES);
    let g = surface_hash(texel + vec2<f32>(13.0, 29.0));
    let pick = surface_hash(texel + vec2<f32>(-71.0, 5.0));
    var col = mean * (0.90 + g * 0.20);
    col = select(col, mean * vec3<f32>(1.55, 1.55, 1.50), pick > 0.955);
    col = select(col, mean * 0.45, pick < 0.035);
    natural_tilt = natural_tilt_from(normal, (vec2<f32>(g, pick) - vec2<f32>(0.5)) * 0.08 * resolved);
    return mix(mean, col, resolved);
}
