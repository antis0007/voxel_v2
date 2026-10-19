struct AtmosphereLutUniform {
    sun_dir_daylight: vec4<f32>,
    moon_dir_twilight: vec4<f32>,
    sun_color_cloud_cover: vec4<f32>,
    moon_color_reserved: vec4<f32>,
    sky_top_reserved: vec4<f32>,
    sky_horizon_reserved: vec4<f32>,
    fog_color_max_altitude: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: AtmosphereLutUniform;
@group(0) @binding(1) var transmittance_lut: texture_2d<f32>;
@group(0) @binding(2) var out_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(3) var multiscatter_lut: texture_2d<f32>;

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    inv_view_proj: mat4x4<f32>,
    world_params: vec4<f32>,
    prev_view_proj: mat4x4<f32>,
    camera_forward: vec4<f32>,
}
@group(1) @binding(0) var<uniform> camera: CameraUniform;

@group(2) @binding(0) var ap_out_tex: texture_storage_3d<rgba16float, write>;
const PLANET_RADIUS: f32  = 6360000.0;
const ATMOS_RADIUS: f32   = 6420000.0;
const RAYLEIGH_SCALE: f32 = 8000.0;
const MIE_SCALE: f32      = 1200.0;
// Extinction per METER; path integration is in meters as well.
const BETA_R: vec3<f32>   = vec3<f32>(5.8e-6, 13.5e-6, 33.1e-6);
const BETA_M: vec3<f32>   = vec3<f32>(3.0e-6);
// Broad ozone absorption separates blue twilight from the warm horizon.
const BETA_O: vec3<f32>   = vec3<f32>(0.650e-6, 1.881e-6, 0.085e-6);

fn ozone_density(altitude_m: f32) -> f32 {
    return max(1.0 - abs(altitude_m - 25000.0) / 15000.0, 0.0);
}

fn ray_intersect_sphere(ro: vec3<f32>, rd: vec3<f32>, radius: f32) -> vec2<f32> {
    let b = dot(ro, rd);
    // Difference of squared planetary radii loses ground-height precision in f32.
    let h = length(ro) - radius;
    let c = h * (2.0 * radius + h);
    let d = b * b - c;
    if (d < 0.0) { return vec2<f32>(-1.0, -1.0); }
    let sq = sqrt(d);
    return vec2<f32>(-b - sq, -b + sq);
}

fn integrate_sun_transmittance(altitude_m: f32, sun_dir_y: f32) -> vec3<f32> {
    let h = max(altitude_m, 1.0);
    let radius = PLANET_RADIUS + h;
    let horizon = -sqrt(h * (2.0 * PLANET_RADIUS + h)) / radius;
    // Finite solar disk softens the terminator; high clouds remain sunlit longer.
    let visibility = smoothstep(horizon - 0.00465, horizon + 0.00465, sun_dir_y);
    if visibility <= 0.0 { return vec3<f32>(0.0); }
    let mu = clamp(max(sun_dir_y, horizon + 0.00001), -1.0, 1.0);
    let ro = vec3<f32>(0.0, radius, 0.0);
    let rd = vec3<f32>(sqrt(max(1.0 - mu * mu, 0.0)), mu, 0.0);
    let distance = max(ray_intersect_sphere(ro, rd, ATMOS_RADIUS).y, 0.0);
    var tau = vec3<f32>(0.0);
    // Quadratic quadrature concentrates samples in the dense lower atmosphere.
    for (var i = 0u; i < 32u; i += 1u) {
        let u0 = f32(i) / 32.0;
        let u1 = f32(i + 1u) / 32.0;
        let dt = (u1 * u1 - u0 * u0) * distance;
        let p = ro + rd * ((u1 * u1 + u0 * u0) * 0.5 * distance);
        let sample_h = max(length(p) - PLANET_RADIUS, 0.0);
        tau += (BETA_R * exp(-sample_h / RAYLEIGH_SCALE)
            + BETA_M * exp(-sample_h / MIE_SCALE)
            + BETA_O * ozone_density(sample_h)) * dt;
    }
    return exp(-tau) * visibility;
}

fn get_sun_transmittance(altitude_m: f32, sun_dir_y: f32) -> vec3<f32> {
    // Same nonlinear coordinates as the runtime lookup. Interpolate the
    // preintegrated table instead of nesting a sun march in each view sample.
    let mu = clamp(sun_dir_y, -1.0, 1.0);
    let uv = vec2<f32>(0.5 + 0.5 * sign(mu) * sqrt(abs(mu)),
        sqrt(clamp(altitude_m / params.fog_color_max_altitude.w, 0.0, 1.0)));
    let dims = vec2<i32>(textureDimensions(transmittance_lut));
    let p = uv * vec2<f32>(dims - vec2<i32>(1));
    let lo = vec2<i32>(floor(p));
    let hi = min(lo + vec2<i32>(1), dims - vec2<i32>(1));
    let f = fract(p);
    return mix(mix(textureLoad(transmittance_lut, lo, 0).rgb,
                    textureLoad(transmittance_lut, vec2<i32>(hi.x, lo.y), 0).rgb, f.x),
               mix(textureLoad(transmittance_lut, vec2<i32>(lo.x, hi.y), 0).rgb,
                    textureLoad(transmittance_lut, hi, 0).rgb, f.x), f.y);
}

fn rayleigh_phase(cos_theta: f32) -> f32 {
    return 0.059683 * (1.0 + cos_theta * cos_theta);
}

fn mie_phase(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let d = max(1.0 + g2 - 2.0 * g * cos_theta, 1e-4);
    return (1.0 - g2) / (12.56637 * pow(d, 1.5));
}

fn get_multiple_scattering(h: f32, sun_mu: f32) -> vec3<f32> {
    let uv = vec2<f32>(0.5 + 0.5 * sign(sun_mu) * sqrt(abs(sun_mu)),
        sqrt(clamp(h / params.fog_color_max_altitude.w, 0.0, 1.0)));
    let dims = vec2<i32>(textureDimensions(multiscatter_lut));
    let p = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * vec2<f32>(dims - vec2<i32>(1));
    let lo = vec2<i32>(floor(p));
    let hi = min(lo + vec2<i32>(1), dims - vec2<i32>(1));
    let f = fract(p);
    return mix(mix(textureLoad(multiscatter_lut, lo, 0).rgb,
                   textureLoad(multiscatter_lut, vec2<i32>(hi.x, lo.y), 0).rgb, f.x),
               mix(textureLoad(multiscatter_lut, vec2<i32>(lo.x, hi.y), 0).rgb,
                   textureLoad(multiscatter_lut, hi, 0).rgb, f.x), f.y);
}

@compute @workgroup_size(8, 8, 1)
fn generate_transmittance_lut(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = vec2<f32>(textureDimensions(out_tex));
    if (f32(id.x) >= dims.x || f32(id.y) >= dims.y) { return; }

    // Calculate altitude_t (normalized altitude from 0 to 1)
    // Equivalent to normalized_coordinate(y, TRANSMITTANCE_LUT_HEIGHT)
    let altitude_t = f32(id.y) / (dims.y - 1.0);
    let altitude_m = altitude_t * altitude_t * params.fog_color_max_altitude.w;

    // Calculate sun_height (normalized sun angle from -1 to 1)
    // Equivalent to normalized_coordinate(x, TRANSMITTANCE_LUT_WIDTH) * 2.0 - 1.0
    let signed_sun = (f32(id.x) / (dims.x - 1.0)) * 2.0 - 1.0;
    let sun_height = signed_sun * abs(signed_sun);

    let transmittance = integrate_sun_transmittance(altitude_m, sun_height);
    textureStore(out_tex, id.xy, vec4<f32>(transmittance, 1.0));
}

@compute @workgroup_size(8, 8, 1)
fn generate_sky_view_lut(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = vec2<f32>(textureDimensions(out_tex));
    if (f32(id.x) >= dims.x || f32(id.y) >= dims.y) { return; }

    let sun_dir = normalize(params.sun_dir_daylight.xyz);
    let daylight = params.sun_dir_daylight.w;

    // Build a sun-relative azimuth frame so the LUT origin aligns with the
    // runtime atmosphere_sky_view_uv() which stores view angle relative to sun.
    // sun_az: normalized sun XZ direction (world-space).
    let sun_az_raw = vec2<f32>(sun_dir.x, sun_dir.z);
    let sun_az_len = length(sun_az_raw);
    let sun_az = select(vec2<f32>(1.0, 0.0), sun_az_raw / max(sun_az_len, 1e-5), sun_az_len > 1e-4);
    let sun_angle = atan2(sun_az.y, sun_az.x);  // world azimuth of the sun

    // UV mapping: u=0.5 => directly toward sun, u=0.0/1.0 => opposite sun.
    // This matches fract(relative_azimuth / TAU + 0.5 + 1.0) in the runtime sampler.
    let uv = (vec2<f32>(id.xy) + 0.5) / dims;
    // relative_azimuth such that fract(relative_azimuth/TAU + 0.5) == uv.x
    let relative_azimuth = (uv.x - 0.5) * 6.2831853;
    let world_azimuth = sun_angle + relative_azimuth;

    // Signed quadratic elevation concentrates samples around the horizon.
    let elevation_v = uv.y;  // 0=down, 0.5=horizon, 1=up
    let signed_elev = elevation_v * 2.0 - 1.0;
    let sin_elev = signed_elev * abs(signed_elev);
    let cos_elev = sqrt(max(1.0 - sin_elev * sin_elev, 0.0));
    let dir = vec3<f32>(cos_elev * cos(world_azimuth), sin_elev, cos_elev * sin(world_azimuth));
    
    let cam_alt = max(camera.camera_pos.y, 1.0);
    let ro = vec3<f32>(0.0, PLANET_RADIUS + cam_alt, 0.0);
    
    let hit_ground = ray_intersect_sphere(ro, dir, PLANET_RADIUS);
    let hit_atmos  = ray_intersect_sphere(ro, dir, ATMOS_RADIUS);
    
    var max_dist = hit_atmos.y;
    if (hit_ground.y > 0.0) {
        max_dist = hit_ground.x;
    }
    max_dist = max(max_dist, 0.0);

    var view_trans = vec3<f32>(1.0);
    var in_scatter = vec3<f32>(0.0);

    let cos_theta = dot(dir, sun_dir);
    let p_r = rayleigh_phase(cos_theta);
    let p_m = mie_phase(cos_theta, 0.76);

    for (var i = 0u; i < 32u; i = i + 1u) {
        let u0 = f32(i) / 32.0;
        let u1 = f32(i + 1u) / 32.0;
        let dt = (u1 * u1 - u0 * u0) * max_dist;
        let t = (u1 * u1 + u0 * u0) * 0.5 * max_dist;
        let p = ro + dir * t;
        let h = max(length(p) - PLANET_RADIUS, 0.0);
        
        let rho_r = exp(-h / RAYLEIGH_SCALE);
        let rho_m = exp(-h / MIE_SCALE);

        let local_up = normalize(p);
        let sun_trans = get_sun_transmittance(h, dot(local_up, sun_dir));
        
        let scatter_r = BETA_R * rho_r;
        let scatter_m = BETA_M * rho_m;
        
        let s_scatter = (scatter_r * p_r + scatter_m * p_m) * sun_trans * daylight;
        let m_scatter = (scatter_r + scatter_m)
            * get_multiple_scattering(h, dot(local_up, sun_dir)) * daylight;
        
        // The transmittance solve already reddens sunlight. Its incident
        // spectrum must not also carry the CPU's legacy sunset color grade.
        let step_scatter = (s_scatter + m_scatter) * vec3<f32>(1.0, 0.98, 0.96);
        let extinction = scatter_r + scatter_m + BETA_O * ozone_density(h);
        let segment_trans = exp(-extinction * dt);
        in_scatter += view_trans * step_scatter
            * (vec3<f32>(1.0) - segment_trans) / max(extinction, vec3<f32>(1e-9));
        view_trans *= segment_trans;
    }
    
    // The lower hemisphere terminates on the ground. Returning scattering alone
    // made every downward ray nearly black/empty, especially at flight altitude.
    let ground_trans = view_trans;
    var ground = vec3<f32>(0.0);
    if hit_ground.x > 0.0 {
        let up = normalize(ro + dir * max_dist);
        let irradiance = vec3<f32>(1.0, 0.98, 0.96) * get_sun_transmittance(0.0, dot(up, sun_dir))
            * max(dot(up, sun_dir), 0.0) * daylight;
        let ambient = params.sky_horizon_reserved.rgb * 0.16 + params.sky_top_reserved.rgb * 0.08;
        ground = vec3<f32>(0.16, 0.19, 0.12) * (irradiance + ambient) * ground_trans;
    }
    let night_sky = mix(params.sky_horizon_reserved.rgb, params.sky_top_reserved.rgb, max(dir.y, 0.0)) * (1.0 - daylight) * 0.35;
    textureStore(out_tex, id.xy, vec4<f32>(in_scatter * 8.0 + ground + night_sky, 1.0));
}

@compute @workgroup_size(8, 8, 1)
fn generate_multiscatter_lut(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = vec2<f32>(textureDimensions(out_tex));
    if (f32(id.x) >= dims.x || f32(id.y) >= dims.y) { return; }
    
    let uv = vec2<f32>(id.xy) / (dims - vec2<f32>(1.0));
    let signed_mu = uv.x * 2.0 - 1.0;
    let mu = signed_mu * abs(signed_mu);
    let h = clamp(uv.y * uv.y * params.fog_color_max_altitude.w, 1.0, 59999.0);
    let sun_dir = vec3<f32>(sqrt(max(1.0 - mu * mu, 0.0)), mu, 0.0);
    let ro = vec3<f32>(0.0, PLANET_RADIUS + h, 0.0);
    var incident = vec3<f32>(0.0);
    var feedback = vec3<f32>(0.0);
    // Equal-solid-angle Fibonacci quadrature. Higher orders approach an
    // isotropic field; accumulate the response to unit isotropic illumination
    // alongside sun scattering, then sum its geometric series L/(1-F).
    // This is a low-order transport approximation, not a full spectral solve.
    for (var ray = 0u; ray < 32u; ray += 1u) {
        let y = 1.0 - 2.0 * (f32(ray) + 0.5) / 32.0;
        let angle = f32(ray) * 2.39996323;
        let r = sqrt(max(1.0 - y * y, 0.0));
        let dir = vec3<f32>(r * cos(angle), y, r * sin(angle));
        let ground_hit = ray_intersect_sphere(ro, dir, PLANET_RADIUS);
        var distance = max(ray_intersect_sphere(ro, dir, ATMOS_RADIUS).y, 0.0);
        if ground_hit.x > 0.0 { distance = min(distance, ground_hit.x); }
        var trans = vec3<f32>(1.0);
        for (var step = 0u; step < 20u; step += 1u) {
            let u0 = f32(step) / 20.0;
            let u1 = f32(step + 1u) / 20.0;
            let dt = (u1 * u1 - u0 * u0) * distance;
            let p = ro + dir * ((u0 * u0 + u1 * u1) * 0.5 * distance);
            let altitude = max(length(p) - PLANET_RADIUS, 0.0);
            let scattering = BETA_R * exp(-altitude / RAYLEIGH_SCALE)
                + BETA_M * exp(-altitude / MIE_SCALE);
            let extinction = scattering + BETA_O * ozone_density(altitude);
            let segment_trans = exp(-extinction * dt);
            let integral = trans * (vec3<f32>(1.0) - segment_trans)
                / max(extinction, vec3<f32>(1e-9));
            let sun_trans = get_sun_transmittance(altitude, dot(normalize(p), sun_dir));
            incident += integral * scattering * sun_trans * 0.07957747;
            feedback += integral * scattering;
            trans *= segment_trans;
        }
        if ground_hit.x > 0.0 {
            let up = normalize(ro + dir * distance);
            let ground_mu = dot(up, sun_dir);
            let albedo = vec3<f32>(0.16, 0.19, 0.12);
            incident += trans * albedo * get_sun_transmittance(0.0, ground_mu)
                * max(ground_mu, 0.0) / 3.14159265;
            feedback += trans * albedo;
        }
    }
    incident /= 32.0;
    feedback /= 32.0;
    let ms_energy = incident / max(vec3<f32>(1.0) - feedback, vec3<f32>(0.02));
    textureStore(out_tex, id.xy, vec4<f32>(ms_energy, 1.0));
}

@compute @workgroup_size(8, 8, 4)
fn generate_aerial_perspective(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = vec3<f32>(textureDimensions(ap_out_tex));
    if (f32(id.x) >= dims.x || f32(id.y) >= dims.y || f32(id.z) >= dims.z) { return; }
    
    let uv = (vec2<f32>(id.xy) + 0.5) / dims.xy;
    // u maps azimuth via fract(azimuth/TAU + 0.5 + 1.0) — must match runtime sampler exactly.
    // Reconstruct: azimuth = (uv.x - 0.5) * TAU
    let azimuth = (uv.x - 0.5) * 6.2831853;
    // v maps view_dir.y linearly: v = dir.y * 0.5 + 0.5 — must match runtime sampler exactly.
    // Reconstruct: dir.y = (uv.y - 0.5) * 2.0
    let dir_y = clamp(uv.y * 2.0 - 1.0, -1.0, 1.0);
    let dir_xz_len = sqrt(max(1.0 - dir_y * dir_y, 0.0));
    let dir = vec3<f32>(dir_xz_len * sin(azimuth), dir_y, dir_xz_len * cos(azimuth));
    
    let z_frac = f32(id.z) / max(dims.z - 1.0, 1.0);
    let max_dist = max(camera.world_params.w * 0.82, 16000.0);
    let target_dist = max_dist * (z_frac * z_frac);
    
    let sun_dir = normalize(params.sun_dir_daylight.xyz);
    let daylight = params.sun_dir_daylight.w;
    
    let cam_alt = max(camera.camera_pos.y, 1.0);
    let ro = vec3<f32>(0.0, PLANET_RADIUS + cam_alt, 0.0);
    
    let hit_atmos  = ray_intersect_sphere(ro, dir, ATMOS_RADIUS);
    
    // Do NOT clamp d_max to hit_ground.x (the sea-level planet sphere):
    // mountains and valleys sit above or below dir.y = 0, and truncating the
    // aerial perspective LUT at the smooth sea-level horizon created a sharp
    // horizontal fog discontinuity across distant mountain slopes at dir.y = 0.
    let d_max = max(hit_atmos.y, 0.0);
    let march_dist = min(target_dist, d_max);

    let steps = 16.0;
    let dt = march_dist / steps;
    var t = dt * 0.5;

    var view_trans = vec3<f32>(1.0);
    var in_scatter = vec3<f32>(0.0);

    let cos_theta = dot(dir, sun_dir);
    let p_r = rayleigh_phase(cos_theta);
    let p_m = mie_phase(cos_theta, 0.76);

    // Clear air retains blue depth; humid overcast carries a denser veil.
    let fog_density = mix(0.000018, 0.000085, params.sun_color_cloud_cover.w);

    for (var i = 0u; i < 16u; i = i + 1u) {
        let p = ro + dir * t;
        let h = max(length(p) - PLANET_RADIUS, 0.0);
        
        let rho_r = exp(-h / RAYLEIGH_SCALE);
        let rho_m = exp(-h / MIE_SCALE);

        let fog_extinction = fog_density * exp(-h / 2000.0);
        let sun_trans = get_sun_transmittance(h, dot(normalize(p), sun_dir));
        
        let scatter_r = BETA_R * rho_r;
        let scatter_m = BETA_M * rho_m;
        let scatter_fog = params.fog_color_max_altitude.rgb * fog_extinction;
        
        let s_scatter = (scatter_r * p_r + scatter_m * p_m) * sun_trans * daylight;
        
        // Fog already contains the sky's illumination; multiplying it by the
        // warm sun again made cool storm haze uniformly orange at dusk.
        let m_scatter = (scatter_r + scatter_m)
            * get_multiple_scattering(h, dot(normalize(p), sun_dir)) * daylight;
        let step_scatter = (s_scatter + m_scatter) * vec3<f32>(1.0, 0.98, 0.96) * 8.0 + scatter_fog;
        let extinction = scatter_r + scatter_m + BETA_O * ozone_density(h) + vec3<f32>(fog_extinction);
        let segment_trans = exp(-extinction * dt);
        in_scatter += view_trans * step_scatter
            * (vec3<f32>(1.0) - segment_trans) / max(extinction, vec3<f32>(1e-9));
        view_trans *= segment_trans;
        
        t += dt;
    }

    let avg_trans = clamp(dot(view_trans, vec3<f32>(0.3333333)), 0.0, 1.0);

    textureStore(ap_out_tex, id, vec4<f32>(in_scatter, avg_trans));
}
