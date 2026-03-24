// ============================================================
// atmosphere_shared.wgsl
//
// Physically-motivated atmosphere model for Hillaire-style sky rendering.
// Replaces the old hand-crafted gradient with:
//   - Rayleigh scattering (wavelength-dependent → blue sky, orange sunsets)
//   - Mie aerosol forward-scatter (sun halo, horizon haze)
//   - Twilight arc with Belt of Venus
//   - sky_transmittance_to_sun() helper for cloud/terrain illumination coupling
//
// All utility functions (hash12, noise2, fbm2, etc.) are kept unchanged
// because cloud_shader.wgsl depends on them.
// ============================================================

// ---- Scalar / vector utilities ----

fn saturate1(x: f32) -> f32 {
    return clamp(x, 0.0, 1.0);
}

fn saturate3(v: vec3<f32>) -> vec3<f32> {
    return clamp(v, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn rotate2(v: vec2<f32>, angle: f32) -> vec2<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return vec2<f32>(c * v.x - s * v.y, s * v.x + c * v.y);
}

fn safe_normalize2(v: vec2<f32>, fallback: vec2<f32>) -> vec2<f32> {
    let len2 = dot(v, v);
    if len2 > 1e-6 {
        return v * inverseSqrt(len2);
    }
    return normalize(fallback);
}

// ---- 2-D noise helpers (used by cloud_shader.wgsl) ----

fn hash12(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453123);
}

// Integer-domain hash for cell IDs. Avoids f32 precision collapse at large world
// coordinates. Uses a Murmur-inspired mix: all arithmetic stays in u32 modular
// space, so output is always uniformly distributed regardless of cell index magnitude.
fn hash_cell1(c: vec2<i32>, salt: i32) -> f32 {
    var h = bitcast<u32>(c.x * 1619 + c.y * 31337 + salt * 6971 + 1013904223);
    h ^= h >> 16u;
    h  = h * 0x45d9f3bu;
    h ^= h >> 16u;
    h  = h * 0xac4d1c7fu;
    h ^= h >> 16u;
    return f32(h) / 4294967296.0;
}

fn hash_cell2(c: vec2<i32>, salt: i32) -> vec2<f32> {
    return vec2<f32>(hash_cell1(c, salt), hash_cell1(c, salt + 7919));
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let a = hash12(i);
    let b = hash12(i + vec2<f32>(1.0, 0.0));
    let c = hash12(i + vec2<f32>(0.0, 1.0));
    let d = hash12(i + vec2<f32>(1.0, 1.0));

    let x0 = mix(a, b, u.x);
    let x1 = mix(c, d, u.x);
    return mix(x0, x1, u.y);
}

fn fbm2(p: vec2<f32>) -> f32 {
    var value = 0.0;
    var amp = 0.55;
    var freq = 1.0;
    for (var i: i32 = 0; i < 3; i = i + 1) {
        value += noise2(p * freq) * amp;
        freq *= 2.07;
        amp *= 0.5;
    }
    return value;
}

fn billow2(p: vec2<f32>) -> f32 {
    return 1.0 - abs(noise2(p) * 2.0 - 1.0);
}

// ============================================================
// ATMOSPHERIC PHYSICS HELPERS
// ============================================================

// Rayleigh scattering phase function: p(θ) = 3/(16π) * (1 + cos²θ).
// Returns a scalar; multiply by wavelength-dependent β to get spectral scatter.
fn rayleigh_phase(cos_theta: f32) -> f32 {
    // 3.0 / (16.0 * π) ≈ 0.059683
    return 0.059683 * (1.0 + cos_theta * cos_theta);
}

// Henyey–Greenstein Mie phase function for aerosol forward scatter.
// g ∈ [0.6, 0.85] for typical atmospheric aerosols.
// Used for the sun corona, haze glow, and cloud self-shadowing.
fn mie_phase_atmo(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let d  = max(1.0 + g2 - 2.0 * g * cos_theta, 0.0001);
    return (1.0 - g2) / (12.56637 * pow(d, 1.5));
}

// Approximate spectral transmittance from a given altitude to the sun.
//
// Uses Beer–Lambert extinction with Rayleigh cross-sections scaled so the
// visible sky has calibrated brightness.  The Rayleigh cross-sections are
// proportional to λ⁻⁴ (5.8 : 13.5 : 33.1 for R : G : B), so low-elevation
// paths (large air-mass) attenuate blue more than red → orange/red sunsets.
//
// sun_height: sin(sun elevation angle).  Negative = below horizon.
// Returns a vec3 colour tint to apply to direct illumination.
fn analytic_sky_transmittance_to_sun_at_altitude(
    sun_height: f32,
    altitude_m: f32,
) -> vec3<f32> {
    // Game-scale normalised Rayleigh cross-sections (R, G, B).
    // Ratios are physical; overall magnitude is tuned so direct sun appears
    // white at zenith and deep orange near the horizon.
    let rayleigh_density = exp(-max(altitude_m, 0.0) / 8000.0);
    let mie_density = exp(-max(altitude_m, 0.0) / 1200.0);
    let beta_r = vec3<f32>(0.00529, 0.01235, 0.03022) * rayleigh_density;
    // Mie (white aerosol) adds a uniform extinction component.
    let beta_m = 0.00100 * mie_density;

    // Optical path length ~ 1 / sin(elevation), clamped to prevent singularity.
    // At sun_height = 0 (horizon) path ≈ 12.5; at 90° (zenith) path ≈ 1.
    let path = min(1.0 / max(sun_height + 0.08, 0.015), 38.0);

    let tau = (beta_r + vec3<f32>(beta_m)) * path;
    return exp(-tau);
}

fn sky_transmittance_to_sun_at_altitude(
    sun_height: f32,
    altitude_m: f32,
) -> vec3<f32> {
    // Transmittance is the first atmosphere stage with explicit runtime
    // ownership. The analytic path remains available as a named fallback while
    // the rest of the LUT stack is still being migrated.
    if atmosphere_mode() != ATMOSPHERE_MODE_ANALYTIC_FALLBACK {
        return sample_atmosphere_transmittance_lut(sun_height, altitude_m);
    }
    return analytic_sky_transmittance_to_sun_at_altitude(sun_height, altitude_m);
}

fn sky_transmittance_to_sun(sun_height: f32) -> vec3<f32> {
    return sky_transmittance_to_sun_at_altitude(sun_height, 0.0);
}
// Atmospheric density factor at camera altitude.
// Models exponential atmosphere falloff: less air above = darker, more saturated sky.
// Scale height ~3000m (compressed from real 8500m for more visible effect in-game).
fn altitude_atmosphere_factor() -> f32 {
    let scale = max(cam.world_params.x, 1e-5);
    let cam_h = max(cam.camera_pos.y, 0.0);
    return exp(-cam_h / (3000.0 * scale));
}
// ============================================================
// Aerial perspective: in-scattered radiance along a view ray segment.
// Used by cloud_shader to add atmospheric haze between camera and cloud layer.
// This is the key Hillaire technique that makes distant clouds look natural.
//
// Returns (in_scatter, transmittance) where in_scatter is the light added
// by the atmosphere segment and transmittance is what fraction of the cloud
// radiance survives to the camera.
// ============================================================
fn aerial_perspective(
    view_dir: vec3<f32>,
    dist: f32,
    scale: f32,
    sky_col: vec3<f32>,
) -> vec4<f32> {
    if atmosphere_mode() == ATMOSPHERE_MODE_ANALYTIC_FALLBACK {
        let visible_world = max(cam.world_params.w, 1.0);
        let fog_end       = max(light.fog_params.y, visible_world * 0.82);

        let horizon = saturate1(1.0 - abs(view_dir.y));

        let beta_r = vec3<f32>(0.00529, 0.01235, 0.03022);
        let beta_m = 0.00100;
        let beta_total = beta_r + vec3<f32>(beta_m);

        let norm_dist = clamp(dist / max(fog_end * 1.35, 1.0), 0.0, 3.0);
        let optical_depth = beta_total * norm_dist * mix(5.0, 12.0, horizon * horizon);
        let view_trans = exp(-optical_depth);
        let avg_trans = clamp(dot(view_trans, vec3<f32>(0.3333333)), 0.0, 1.0);

        let fog_col = mix(light.fog_color_density.rgb, sky_col, 0.86);

        let in_scatter =
            fog_col * (vec3<f32>(1.0) - view_trans) * mix(0.55, 1.0, horizon);

        return vec4<f32>(max(in_scatter, vec3<f32>(0.0)), avg_trans);
    }

    let azimuth = atan2(view_dir.x, view_dir.z);
    let elevation = asin(clamp(view_dir.y, -1.0, 1.0));
    let u = azimuth / 6.2831853 + 0.5;
    let v = (elevation / 1.5707963 + 1.0) * 0.5;
    
    let max_dist = max(cam.world_params.w * 0.82, 1.0);
    let z_frac = sqrt(clamp(dist / max_dist, 0.0, 1.0));
    let uvw = vec3<f32>(u, v, z_frac);
    
    return textureSampleLevel(atmosphere_aerial_perspective, atmosphere_sampler, uvw, 0.0);
}

// Runtime sky-view mode. Sky-view and multiscatter now come from explicit
// runtime-generated LUT passes, but the current solve is still approximate.
fn runtime_sky_view_atmosphere_color(
    dir: vec3<f32>,
    sun_dir: vec3<f32>,
    daylight: f32,
) -> vec3<f32> {
    let altitude_m = max(cam.camera_pos.y, 0.0);
    let up = saturate1(dir.y * 0.5 + 0.5);
    let down = saturate1(-dir.y);
    let horizon_w = exp(-abs(dir.y) * 8.5);
    let alt_factor = altitude_atmosphere_factor();
    let cos_theta = dot(dir, sun_dir);

    let base_sky = sample_atmosphere_sky_view_lut(dir, sun_dir);
    let multiscatter = sample_atmosphere_multiscatter_lut(cos_theta, altitude_m);
    let sun_trans = sky_transmittance_to_sun_at_altitude(sun_dir.y, altitude_m);

    var sky = base_sky;
    sky += multiscatter * mix(0.45, 0.92, horizon_w);
    sky *= mix(0.88, 1.02, alt_factor);
    sky += light.sky_horizon_reserved.rgb * horizon_w * daylight * 0.03;
    sky += light.sky_top_saturation.rgb * up * 0.02;
    sky += light.fog_color_density.rgb * down * 0.04 * alt_factor;
    sky += light.sun_color_daylight.rgb
        * light.sun_dir_intensity.w
        * sun_trans
        * vec3<f32>(mie_phase_atmo(cos_theta, 0.76) * 0.004)
        * daylight
        * (0.28 + 0.72 * (1.0 - up));

    return max(sky, vec3<f32>(0.0));
}
// ============================================================
// atmosphere_color(dir) — full sky radiance for view direction `dir`.
//
// Called from:
//   • sky_shader.wgsl  — sky background colour
//   • world_shader.wgsl — fog / aerial perspective colour
//
// Signature is UNCHANGED from previous version so no call-site edits needed.
// ============================================================
fn atmosphere_color(dir: vec3<f32>) -> vec3<f32> {
    let sun_dir    = normalize(light.sun_dir_intensity.xyz);
    let moon_dir   = normalize(light.moon_dir_intensity.xyz);
    let daylight   = saturate1(light.sun_color_daylight.w);
    let twilight   = saturate1(light.moon_color_twilight.w);
    let night      = 1.0 - daylight;
    let sun_height = sun_dir.y;

    if atmosphere_mode() == ATMOSPHERE_MODE_SKY_VIEW_LUT {
        return runtime_sky_view_atmosphere_color(dir, sun_dir, daylight);
    }

    let alt_factor   = altitude_atmosphere_factor();
    let haze_atten   = mix(0.15, 1.0, alt_factor);

    let cos_theta   = dot(dir, sun_dir);
    let cos_theta_m = dot(dir, moon_dir);

    // Use the true view direction elevation — no artificial horizon_drop offset.
    // The old horizon_drop_from_eye_height() shift was meant to hide the chunk
    // loading edge at altitude but caused a visible dark horizontal band in the
    // sky as the player flew upward.  Fog in world_shader handles the terrain
    // edge fade; the sky itself should be geometrically correct.
    let elev      = dir.y;
    let up        = saturate1(elev * 0.5 + 0.5);
    let down      = saturate1(-elev);
    let horizon_w = exp(-abs(elev) * 8.5);

    let top     = light.sky_top_saturation.rgb;
    let horizon = light.sky_horizon_reserved.rgb;
    let fog_col = light.fog_color_density.rgb;

    let alt_top = mix(top, top * 0.72, 1.0 - alt_factor);

    // Upper hemisphere: same atmosphere family as before.
    let upper_sky = mix(
        horizon * haze_atten + alt_top * (1.0 - haze_atten),
        alt_top,
        pow(up, 0.42),
    );

    // Lower hemisphere: continuous atmospheric continuation, not a separate blank skybox.
    let lower_horizon = mix(
        horizon * 0.58 + fog_col * 0.42,
        horizon * 0.36 + fog_col * 0.64,
        down,
    );
    let lower_nadir = fog_col * 0.76 + horizon * 0.18 + alt_top * 0.06;
    let lower_sky = mix(lower_horizon, lower_nadir, smoothstep(0.0, 1.0, down));

    // Continuous full-sphere blend through the horizon. No seam weld.
    let hemisphere_mix = smoothstep(-0.30, 0.16, elev);
    var sky = mix(lower_sky, upper_sky, hemisphere_mix);

    // Directional Rayleigh modulation.
    let r_phase      = rayleigh_phase(cos_theta);
    let r_phase_perp = 0.11937;
    let phase_mod    = r_phase / r_phase_perp;
    sky *= mix(1.0, phase_mod, 0.22 * daylight);

    // Mie forward scatter from sun and moon.
    let mie_g_atmo  = 0.76;
    let m_phase_sun = mie_phase_atmo(cos_theta, mie_g_atmo);
    let sun_trans   = sky_transmittance_to_sun_at_altitude(sun_height, cam.camera_pos.y);
    let sun_irr     = light.sun_color_daylight.rgb * light.sun_dir_intensity.w;

    sky += sun_irr * sun_trans * vec3<f32>(m_phase_sun * 0.012)
        * daylight
        * (0.24 + 0.76 * (1.0 - up))
        * mix(haze_atten, 1.0, horizon_w);

    let m_phase_moon = mie_phase_atmo(cos_theta_m, 0.60);
    let moon_irr     = light.moon_color_twilight.rgb * light.moon_dir_intensity.w;
    sky += moon_irr * vec3<f32>(m_phase_moon * 0.007) * night * (0.55 + 0.45 * horizon_w);

    let sun_az_raw = vec2<f32>(sun_dir.x, sun_dir.z);
    let sun_az_len = length(sun_az_raw);
    let sun_az     = select(
        vec2<f32>(1.0, 0.0),
        sun_az_raw / max(sun_az_len, 1e-5),
        sun_az_len > 1e-4,
    );

    let view_az_raw = vec2<f32>(dir.x, dir.z);
    let view_az_len = length(view_az_raw);
    let view_az     = select(
        sun_az,
        view_az_raw / max(view_az_len, 1e-5),
        view_az_len > 1e-4,
    );

    let az_align = dot(sun_az, view_az);

    let tw_vert  = exp(-abs(elev) * 5.0);
    let tw_solar = saturate1(az_align * 0.5 + 0.5);
    let tw_angle = tw_solar * tw_solar;

    sky += vec3<f32>(1.00, 0.40, 0.06)
        * twilight
        * tw_vert
        * tw_angle
        * 0.50
        * (1.0 - daylight * 0.60)
        * mix(haze_atten, 1.0, horizon_w);

    let bov_vert = exp(-abs(elev) * 6.0) * saturate1(0.65 + elev * 7.5);
    let bov_az   = saturate1(-az_align * 0.5 + 0.5) * (1.0 - saturate1(az_align));
    sky += vec3<f32>(0.62, 0.28, 0.48) * twilight * bov_vert * bov_az * 0.20 * haze_atten;

    let lower_warm = vec3<f32>(0.82, 0.48, 0.20)
        * twilight
        * tw_angle
        * exp(-down * 4.2)
        * 0.14;
    sky += lower_warm;

    let horizon_boost = daylight * 0.08 * horizon_w * haze_atten;
    sky += horizon * horizon_boost;

    let night_zenith  = top * up * 0.08;
    let night_horizon = horizon * horizon_w * 0.05;
    let night_lower   = mix(horizon, fog_col, 0.55) * down * 0.08;
    sky += (night_zenith + night_horizon + night_lower) * night;

    return max(sky, vec3<f32>(0.0));
}
