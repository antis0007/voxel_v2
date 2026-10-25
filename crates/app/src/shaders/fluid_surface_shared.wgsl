// ============================================================
// fluid_surface_shared.wgsl
//
// Fragment-only looks for water, fire, smoke, steam and loose powders. Every
// lattice here is the world PIXEL_RES grid declared in world_shader.wgsl, and
// every animated term is a pure function of position and the shared clock.
// Colours are linear reflectance or unit-scale emission, before lighting.
// ============================================================

// Per-metre absorption: red goes first, green next, blue last, so a sand bed
// reads jade in the shallows, teal at 1-3 m and teal-navy beyond.
const WATER_ABSORB: vec3<f32> = vec3<f32>(1.6, 0.21, 0.17);
// Reflections of sky and scene read darker than their sources.
const WATER_REFLECTION_GAIN: f32 = 0.80;
const WATER_FOAM: vec3<f32> = vec3<f32>(0.80, 0.83, 0.79);
// Thin edge water (#7FE3C2 at display scale).
const WATER_MINT_EDGE: vec3<f32> = vec3<f32>(0.060, 0.220, 0.155);

/// In-scattered colour of the water column above a bed `depth` metres down.
fn water_body_color(depth: f32, turbidity: f32) -> vec3<f32> {
    let shallow = vec3<f32>(0.018, 0.150, 0.112);
    let mid = vec3<f32>(0.008, 0.090, 0.088);
    let deep = mix(vec3<f32>(0.0055, 0.040, 0.064), vec3<f32>(0.004, 0.028, 0.054), turbidity);
    let body = mix(shallow, mid, smoothstep(0.2, 2.0, depth));
    return mix(body, deep, 1.0 - exp(-max(depth - 1.5, 0.0) * 0.30));
}

/// Caustic gain on a submerged bed: one-texel wobbly lines, 1.3x bed value.
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
    let line = smoothstep(0.74, 0.90, 1.0 - abs(a - b) * 1.7);
    return mix(mean, line * 0.32, resolved) * shallow;
}

/// Foam coverage from the interpolated pinned-corner weight (1 on a solid).
fn water_shore_foam(xz: vec2<f32>, contact: f32, t: f32, footprint_m: f32) -> f32 {
    if contact <= 0.01 { return 0.0; }
    let resolved = fluid_pixel_weight(footprint_m);
    let mean = smoothstep(0.35, 1.0, contact) * 0.35;
    if resolved <= 0.0 { return mean; }
    let texel = floor(xz * PIXEL_RES);
    // The lace breathes: its edge advances and retreats along the shore.
    let swell = 0.07 * sin(t * 1.3 + dot(texel, vec2<f32>(0.037, 0.029)));
    let lace = surface_noise(texel * 0.29 + vec2<f32>(t * 0.30, -t * 0.21));
    let s = contact + swell + (lace - 0.5) * 0.16;
    // Even the contact line is lace, so a film pinned on every corner does
    // not turn into a solid white sheet.
    let rim = step(0.84, s) * step(0.50, surface_noise(texel * 0.61 + vec2<f32>(t * 0.22, 7.0)));
    let holes = step(0.55, surface_noise(texel * 0.47 + vec2<f32>(-t * 0.18, t * 0.26)));
    let band = step(0.56, s) * (1.0 - step(0.74, s)) * holes;
    return mix(mean, max(rim, band), resolved);
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
