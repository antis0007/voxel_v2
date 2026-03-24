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
const BETA_R: vec3<f32>   = vec3<f32>(0.00529, 0.01235, 0.03022);
const BETA_M: vec3<f32>   = vec3<f32>(0.00100, 0.00100, 0.00100);

fn ray_intersect_sphere(ro: vec3<f32>, rd: vec3<f32>, radius: f32) -> vec2<f32> {
    let b = dot(ro, rd);
    let c = dot(ro, ro) - radius * radius;
    let d = b * b - c;
    if (d < 0.0) { return vec2<f32>(-1.0, -1.0); }
    let sq = sqrt(d);
    return vec2<f32>(-b - sq, -b + sq);
}

fn get_sun_transmittance(altitude_m: f32, sun_dir_y: f32) -> vec3<f32> {
    let path = min(1.0 / max(sun_dir_y + 0.08, 0.015), 38.0);
    let rayleigh_density = exp(-max(altitude_m, 0.0) / RAYLEIGH_SCALE);
    let mie_density = exp(-max(altitude_m, 0.0) / MIE_SCALE);
    let tau = (BETA_R * rayleigh_density + BETA_M * mie_density) * path;
    return exp(-tau);
}

fn rayleigh_phase(cos_theta: f32) -> f32 {
    return 0.059683 * (1.0 + cos_theta * cos_theta);
}

fn mie_phase(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let d = max(1.0 + g2 - 2.0 * g * cos_theta, 1e-4);
    return (1.0 - g2) / (12.56637 * pow(d, 1.5));
}

@compute @workgroup_size(8, 8, 1)
fn generate_transmittance_lut(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = vec2<f32>(textureDimensions(out_tex));
    if (f32(id.x) >= dims.x || f32(id.y) >= dims.y) { return; }

    // Calculate altitude_t (normalized altitude from 0 to 1)
    // Equivalent to normalized_coordinate(y, TRANSMITTANCE_LUT_HEIGHT)
    let altitude_t = f32(id.y) / (dims.y - 1.0);
    let altitude_m = altitude_t * params.fog_color_max_altitude.w; // TRANSMITTANCE_LUT_MAX_ALTITUDE_M

    // Calculate sun_height (normalized sun angle from -1 to 1)
    // Equivalent to normalized_coordinate(x, TRANSMITTANCE_LUT_WIDTH) * 2.0 - 1.0
    let sun_height = (f32(id.x) / (dims.x - 1.0)) * 2.0 - 1.0;

    let transmittance = get_sun_transmittance(altitude_m, sun_height);
    textureStore(out_tex, id.xy, vec4<f32>(transmittance, 1.0));
}

@compute @workgroup_size(8, 8, 1)
fn generate_sky_view_lut(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = vec2<f32>(textureDimensions(out_tex));
    if (f32(id.x) >= dims.x || f32(id.y) >= dims.y) { return; }
    
    // UV to perfectly align with `atan2(x, z)` spherical mapping in active path
    let uv = (vec2<f32>(id.xy) + 0.5) / dims;
    let azimuth = (uv.x - 0.5) * 6.2831853;
    let elevation = (uv.y * 2.0 - 1.0) * 1.5707963;
    let dir = vec3<f32>(cos(elevation)*sin(azimuth), sin(elevation), cos(elevation)*cos(azimuth));
    
    let sun_dir = normalize(params.sun_dir_daylight.xyz);
    let daylight = params.sun_dir_daylight.w;
    
    let cam_alt = max(camera.camera_pos.y, 0.0);
    let ro = vec3<f32>(0.0, PLANET_RADIUS + cam_alt, 0.0);
    
    let hit_ground = ray_intersect_sphere(ro, dir, PLANET_RADIUS);
    let hit_atmos  = ray_intersect_sphere(ro, dir, ATMOS_RADIUS);
    
    var max_dist = hit_atmos.y;
    if (hit_ground.y > 0.0) {
        max_dist = hit_ground.x;
    }
    max_dist = max(max_dist, 0.0);

    let steps = 32.0;
    let dt = max_dist / steps;
    var t = dt * 0.5;

    var total_od_r = 0.0;
    var total_od_m = 0.0;
    var in_scatter = vec3<f32>(0.0);

    let cos_theta = dot(dir, sun_dir);
    let p_r = rayleigh_phase(cos_theta);
    let p_m = mie_phase(cos_theta, 0.76);

    let ambient_ms = params.sky_horizon_reserved.rgb * 0.12; 

    for (var i = 0u; i < 32u; i = i + 1u) {
        let p = ro + dir * t;
        let h = length(p) - PLANET_RADIUS;
        
        let rho_r = exp(-h / RAYLEIGH_SCALE);
        let rho_m = exp(-h / MIE_SCALE);

        total_od_r += rho_r * dt;
        total_od_m += rho_m * dt;

        let trans = exp(-(BETA_R * total_od_r + BETA_M * total_od_m));
        let sun_trans = get_sun_transmittance(h, sun_dir.y);
        
        let scatter_r = BETA_R * rho_r;
        let scatter_m = BETA_M * rho_m;
        
        let s_scatter = (scatter_r * p_r + scatter_m * p_m) * sun_trans * daylight;
        let m_scatter = (scatter_r + scatter_m) * ambient_ms;
        
        let step_scatter = (s_scatter + m_scatter) * params.sun_color_cloud_cover.rgb;
        
        in_scatter += trans * step_scatter * dt;
        t += dt;
    }
    
    textureStore(out_tex, id.xy, vec4<f32>(in_scatter, 1.0));
}

@compute @workgroup_size(8, 8, 1)
fn generate_multiscatter_lut(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = vec2<f32>(textureDimensions(out_tex));
    if (f32(id.x) >= dims.x || f32(id.y) >= dims.y) { return; }
    
    let uv = (vec2<f32>(id.xy) + 0.5) / dims;
    let cos_sun_zenith = uv.x * 2.0 - 1.0;
    
    let h = uv.y * 12000.0; 
    let rayleigh_density = exp(-h / RAYLEIGH_SCALE);
    
    // Analytical approximation of multiscatter energy to save integration loops
    let ms_energy = BETA_R * rayleigh_density * max(cos_sun_zenith + 0.2, 0.0) * 0.5;
    
    textureStore(out_tex, id.xy, vec4<f32>(ms_energy, 1.0));
}

@compute @workgroup_size(8, 8, 4)
fn generate_aerial_perspective(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = vec3<f32>(textureDimensions(ap_out_tex));
    if (f32(id.x) >= dims.x || f32(id.y) >= dims.y || f32(id.z) >= dims.z) { return; }
    
    let uv = (vec2<f32>(id.xy) + 0.5) / dims.xy;
    let azimuth = (uv.x - 0.5) * 6.2831853;
    let elevation = (uv.y * 2.0 - 1.0) * 1.5707963;
    let dir = vec3<f32>(cos(elevation)*sin(azimuth), sin(elevation), cos(elevation)*cos(azimuth));
    
    let z_frac = (f32(id.z) + 0.5) / dims.z;
    let max_dist = max(camera.world_params.w * 0.82, 1.0);
    let target_dist = max_dist * (z_frac * z_frac);
    
    let sun_dir = normalize(params.sun_dir_daylight.xyz);
    let daylight = params.sun_dir_daylight.w;
    
    let cam_alt = max(camera.camera_pos.y, 0.0);
    let ro = vec3<f32>(0.0, PLANET_RADIUS + cam_alt, 0.0);
    
    let hit_ground = ray_intersect_sphere(ro, dir, PLANET_RADIUS);
    let hit_atmos  = ray_intersect_sphere(ro, dir, ATMOS_RADIUS);
    
    var d_max = hit_atmos.y;
    if (hit_ground.y > 0.0) {
        d_max = hit_ground.x;
    }
    d_max = max(d_max, 0.0);
    let march_dist = min(target_dist, d_max);

    let steps = 16.0;
    let dt = march_dist / steps;
    var t = dt * 0.5;

    var total_od_r = 0.0;
    var total_od_m = 0.0;
    var in_scatter = vec3<f32>(0.0);

    let cos_theta = dot(dir, sun_dir);
    let p_r = rayleigh_phase(cos_theta);
    let p_m = mie_phase(cos_theta, 0.76);

    let fog_density = 0.0001;
    var total_fog_od = 0.0;

    for (var i = 0u; i < 16u; i = i + 1u) {
        let p = ro + dir * t;
        let h = length(p) - PLANET_RADIUS;
        
        let rho_r = exp(-h / RAYLEIGH_SCALE);
        let rho_m = exp(-h / MIE_SCALE);

        total_od_r += rho_r * dt;
        total_od_m += rho_m * dt;
        total_fog_od += fog_density * dt * exp(-h / 2000.0);

        let trans = exp(-(BETA_R * total_od_r + BETA_M * total_od_m + vec3<f32>(total_fog_od)));
        let sun_trans = get_sun_transmittance(h, sun_dir.y);
        
        let scatter_r = BETA_R * rho_r;
        let scatter_m = BETA_M * rho_m;
        let scatter_fog = params.fog_color_max_altitude.rgb * fog_density * exp(-h / 2000.0);
        
        let s_scatter = (scatter_r * p_r + scatter_m * p_m) * sun_trans * daylight;
        
        // Mix physically based scatter with localized fog color bounds
        let step_scatter = (s_scatter + scatter_fog) * params.sun_color_cloud_cover.rgb;
        in_scatter += trans * step_scatter * dt;
        
        t += dt;
    }

    let final_trans = exp(-(BETA_R * total_od_r + BETA_M * total_od_m + vec3<f32>(total_fog_od)));
    let avg_trans = clamp((final_trans.x + final_trans.y + final_trans.z) / 3.0, 0.0, 1.0);

    textureStore(ap_out_tex, id, vec4<f32>(in_scatter, avg_trans));
}