// Galaxy band and the planet's atmosphere limb seen from above it.

// Galactic pole; the band lies on the great circle around it.
const CEL_GALAXY_POLE: vec3<f32> = vec3<f32>(-0.2981, 0.4472, 0.8433);
// Toward the bright core, on that circle.
const CEL_GALAXY_CORE: vec3<f32> = vec3<f32>(0.9010, 0.4135, 0.1);

// 0..1 star density across the band, cheap enough for the star field to use.
fn cel_galaxy_band(dir: vec3<f32>) -> f32 {
    let lat = dot(dir, CEL_GALAXY_POLE);
    let core = max(dot(dir, normalize(CEL_GALAXY_CORE)), 0.0);
    return exp(-pow(lat / (0.14 + 0.10 * core * core), 2.0));
}

// Low-frequency warp shared by the band's light and its dust, so lanes bend
// with the star clouds they cut through.
fn cel_galaxy_warp(dir: vec3<f32>) -> vec3<f32> {
    let p = dir * 3.1;
    return vec3<f32>(
        mood_noise3(p + vec3<f32>(4.1, 0.7, 2.2)),
        mood_noise3(p + vec3<f32>(8.3, 5.5, 1.9)),
        mood_noise3(p + vec3<f32>(1.4, 9.6, 6.3))) - vec3<f32>(0.5);
}

// Fraction of light the dust lets through: 1 off the plane, near 0 in a lane.
fn cel_galaxy_dust(dir: vec3<f32>, lat: f32, warp: vec3<f32>) -> f32 {
    let lane_lat = lat - 0.015 + 0.05 * warp.x;
    let near_plane = exp(-pow(lane_lat / (0.06 + 0.04 * warp.y), 2.0));
    let clump = smoothstep(0.42, 0.66, mood_fbm(dir * 8.0 + warp * 2.2 + vec3<f32>(9.2, 3.3, 5.1)));
    return 1.0 - 0.94 * clump * near_plane;
}

// The band's diffuse light, linear. Noise only places structure: star clouds
// in cold silver, an ivory bulge, dust that removes light, and sparse teal,
// violet and rose gas. No per-texel hash, so nothing crawls as the view turns.
fn cel_galaxy(dir: vec3<f32>) -> vec3<f32> {
    let lat = dot(dir, CEL_GALAXY_POLE);
    // Indigo skirt so the band fades into the void instead of ending.
    let skirt = vec3<f32>(0.10, 0.12, 0.26) * exp(-pow(lat / 0.32, 2.0)) * 0.12;
    let band = cel_galaxy_band(dir);
    if band < 0.01 { return skirt * 0.022; }
    let core = max(dot(dir, normalize(CEL_GALAXY_CORE)), 0.0);
    let warp = cel_galaxy_warp(dir);
    let dust = cel_galaxy_dust(dir, lat, warp);
    let clouds = smoothstep(0.32, 0.78, mood_fbm(dir * 4.5 + warp * 1.3 + vec3<f32>(1.3, 7.1, 2.9)));
    let bulge = pow(core, 8.0) * exp(-pow(lat / 0.22, 2.0));
    let light = (band * (0.18 + 1.2 * clouds * clouds) + bulge * 1.5) * dust;
    let tint = mix(vec3<f32>(0.46, 0.60, 0.86), vec3<f32>(0.87, 0.80, 0.67), 0.75 * smoothstep(0.15, 0.9, bulge));
    // Gas: a rare mask times thin ridged filaments, hue varying slowly by region.
    let mask = smoothstep(0.70, 0.86, mood_noise3(dir * 5.0 + warp * 2.0 + vec3<f32>(3.0, 1.0, 7.0)));
    var gas = vec3<f32>(0.0);
    if mask > 0.0 {
        let ridge = 1.0 - abs(2.0 * mood_noise3(dir * 15.0 + warp * 3.0) - 1.0);
        let hue = mood_noise3(dir * 2.0 + vec3<f32>(7.0, 2.0, 5.0));
        let tone = mix(mix(vec3<f32>(0.20, 0.62, 0.70), vec3<f32>(0.42, 0.36, 0.85), smoothstep(0.35, 0.6, hue)),
            vec3<f32>(0.80, 0.42, 0.58), smoothstep(0.62, 0.8, hue));
        gas = tone * mask * pow(ridge, 4.0) * band * (0.3 + 0.7 * dust);
    }
    return (tint * light + gas * 0.9 + skirt) * 0.022;
}

// Single scattering along the eye ray through the air shell, plus a thin
// green airglow layer on the night side (planet_air_shared.wgsl).
// `jitter` (0..1 per pixel) offsets the samples so steps never show as rings.
fn cel_limb(eye: vec3<f32>, dir: vec3<f32>, sun: vec3<f32>, radius: f32, jitter: f32) -> PlanetAir {
    return planet_air_segment(eye, dir, sun, radius, 3.0e38, 12, jitter);
}

// The Nether's dome over its frozen plain: near-black indigo overhead, teal
// haze at the horizon and slow violet and teal curtains low around it, in
// the palette the gate's own vista shows from the surface side.
fn cel_realm_sky(dir: vec3<f32>, t: f32) -> vec3<f32> {
    let up = clamp(dir.y, -1.0, 1.0);
    var c = mix(vec3<f32>(0.030, 0.065, 0.085), vec3<f32>(0.004, 0.006, 0.014), smoothstep(-0.05, 0.55, up));
    c += vec3<f32>(0.05, 0.10, 0.12) * exp(-pow(up / 0.06, 2.0)) * 0.6;
    // Curtain rays step in azimuth, like the planet aurora, to match the voxel art.
    let az = atan2(dir.z, dir.x);
    let az_q = floor(az * 96.0) / 96.0;
    let hem = 0.10 + 0.05 * sin(az * 2.0 + t * 0.02) + 0.03 * sin(az * 5.0 - t * 0.035);
    let h = up - hem;
    let curtain = smoothstep(-0.02, 0.015, h) * exp(-max(h, 0.0) * 7.0);
    let rays = 0.35 + 0.65 * mood_noise3(vec3<f32>(az_q * 12.0, t * 0.04, 0.0));
    let violet = clamp(smoothstep(0.0, 0.25, h) * 0.8 + 0.2 * sin(az * 3.0 + t * 0.01), 0.0, 1.0);
    let tint = mix(vec3<f32>(0.10, 0.70, 0.62), vec3<f32>(0.55, 0.22, 0.85), violet);
    return c + tint * curtain * rays * 0.05;
}

fn hell_hash(value: u32) -> u32 {
    var h = value;
    h = (h ^ (h >> 16u)) * 0x7feb352du;
    h = (h ^ (h >> 15u)) * 0x846ca68bu;
    return h ^ (h >> 16u);
}

// Seed-stable 1–3 moons with different orbital periods, inclinations, sizes,
// phases and mineral palettes. Their cratered surfaces share the game's pixel
// grid; molten fractures remain visible on the night hemisphere.
fn cel_hell_moons(dir: vec3<f32>, back: vec3<f32>) -> vec3<f32> {
    let seed = cam.scene_seed.x;
    let count = 1u + hell_hash(seed ^ 0x4d4f4f4eu) % 3u;
    var color = back;
    for (var i = 0u; i < count; i++) {
        let h = hell_hash(seed ^ ((i + 1u) * 0x9e3779b9u));
        let body = cam.planet[5u + i];
        let distance = length(body.xyz);
        // The nearby body has real terrain; its sky disc must not fill the view.
        if body.w <= 0.0 || distance <= body.w + 1250.0 { continue; }
        let center = body.xyz / distance;
        let radius = asin(clamp(body.w / distance, 0.0, 1.0));
        let tint = mix(vec3<f32>(0.22, 0.20, 0.23), vec3<f32>(0.40, 0.30, 0.27), f32(h & 255u) / 255.0);
        let moon = cel_moon(dir, center, cos(radius), light.sun_dir_intensity.xyz, tint, 0.035);
        let disc = cel_disc(dir, center, cos(radius), vec3<f32>(0.0, 1.0, 0.0));
        let q = cel_snap(disc.q, 32.0);
        let rock = mood_noise3(vec3<f32>(q * 5.0, f32(h & 1023u)));
        let fault = mood_noise3(vec3<f32>(q.yx * 3.0 + vec2<f32>(7.1, 2.4), f32(h & 1023u)));
        let cracks = (1.0 - smoothstep(0.013, 0.041, abs(rock - 0.5))) * smoothstep(0.30, 0.60, fault) * disc.cover;
        let air_amount = 1.0 - smoothstep(2000.0, 10000.0, cam.planet[1].w);
        let air = exp(-vec3<f32>(0.045, 0.10, 0.19) * air_amount / max(dir.y + 0.08, 0.025));
        let above = smoothstep(-0.015, 0.025, dir.y);
        color = mix(color, color * (1.0 - moon.a) + (moon.rgb + vec3<f32>(0.26, 0.020, 0.004) * cracks) * air, above);
    }
    return color;
}

fn cel_hell_sky(dir: vec3<f32>, t: f32) -> vec3<f32> {
    let up = max(dir.y, 0.0);
    let solar = max(dot(dir, light.sun_dir_intensity.xyz), 0.0);
    let ash = mood_fbm(dir * vec3<f32>(5.0, 18.0, 5.0) + vec3<f32>(t * 0.006, 0.0, t * 0.003));
    let dome = mix(light.sky_horizon_reserved.xyz, light.sky_top_saturation.xyz, smoothstep(0.0, 0.7, up));
    var physical = dome;
    if atmosphere_runtime.mode_debug_surface.x == ATMOSPHERE_MODE_SKY_VIEW_LUT {
        physical = sample_atmosphere_sky_view_lut(dir, light.sun_dir_intensity.xyz);
        physical *= min(1.0, 0.36 / max(max(physical.r, max(physical.g, physical.b)), 0.001));
    }
    let drift = vec3<f32>(t * 0.012, -t * 0.003, t * 0.005);
    let folds = mood_fbm(dir * vec3<f32>(9.0, 24.0, 9.0) + drift + vec3<f32>(ash * 1.8));
    let smoke = smoothstep(0.42, 0.68, folds) * (1.0 - smoothstep(0.08, 0.72, up));
    let soot = smoothstep(0.39, 0.63, ash);
    var c = mix(dome, physical, 0.30) * (0.45 + soot * 0.52) * (0.38 + light.sun_color_daylight.w * 0.62);
    c += vec3<f32>(0.22, 0.039, 0.009) * pow(solar, 24.0) * exp(-up * 2.0);
    // Pale core, copper aureole: atmospheric transport colours the direct sun.
    let angle = acos(clamp(solar, -1.0, 1.0));
    let aa = max(fwidth(angle), 0.0002);
    let disc = 1.0 - smoothstep(0.012 - aa, 0.012 + aa, angle);
    let trans = sky_transmittance_to_sun_at_altitude(light.sun_dir_intensity.y, max(cam.camera_pos.y, 0.0));
    c += vec3<f32>(1.0, 0.83, 0.58) * (4.0 * disc + 0.12 * exp(-angle / 0.038)) * trans;
    c *= 1.0 - smoke * (1.0 - disc) * 0.67;
    c += vec3<f32>(0.125, 0.024, 0.007) * smoke * soot * exp(-up * 5.5);
    // Sparse windborne sparks: cell-local phase avoids a screen-space overlay.
    let ember_pos = dir * 93.0 + vec3<f32>(-t * 0.083, -t * 0.21, t * 0.042);
    let ember_cell = floor(ember_pos);
    let ember_seed = mood_hash31(ember_cell);
    let ember_center = vec3<f32>(0.25 + 0.5 * ember_seed, 0.45, 0.48);
    let ember_dist = length((fract(ember_pos) - ember_center) * vec3<f32>(1.0, 0.52, 1.0));
    let ember_aa = clamp(length(fwidth(ember_pos)) * 0.42, 0.025, 0.20);
    let spark = (1.0 - smoothstep(0.035, 0.035 + ember_aa, ember_dist))
        * step(0.965, ember_seed) * (0.55 + 0.45 * sin(t * 2.0 + ember_seed * 123.0));
    c += vec3<f32>(0.60, 0.105, 0.014) * spark * (1.0 - smoothstep(0.15, 0.80, up));
    let air_amount = 1.0 - smoothstep(2000.0, 10000.0, cam.planet[1].w);
    c = c * air_amount + star_field(dir, 1.0, 0.0, 0.0, 0.0) * (1.0 - air_amount);
    return cel_hell_moons(dir, c);
}

