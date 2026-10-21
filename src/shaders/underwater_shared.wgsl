// ============================================================
// underwater_shared.wgsl
//
// Environment seen from a submerged camera, shared by the sky pass (empty
// pixels) and the terrain pass (haze and caustics on solid surfaces).
//
// `cam.camera_pos.w` is 0 above water and 1 + surface height (metres) when the
// eye is submerged, so existing `> 0.5` tests keep meaning "underwater".
//
//  * Snell's window: a bright refracted view of the sky inside the 48.6 degree
//    cone overhead, total internal reflection (a dim mirror of the water body)
//    outside it, and shimmer from the moving surface.
//  * Volumetric sun shafts: the refracted sun is marched back to the surface
//    and modulated by the same caustic web that lights the sea floor.
//  * Beer-Lambert absorption that drains red first and darkens with depth.
// ============================================================

const UW_ABSORB: vec3<f32> = vec3<f32>(0.24, 0.075, 0.045);
const UW_REFRACT: f32 = 0.75;      // n_air / n_water
const UW_SHAFT_STEPS: i32 = 14;

fn uw_active() -> bool {
    return cam.camera_pos.w > 0.5;
}

fn uw_surface_y() -> f32 {
    return max(cam.camera_pos.w - 1.0, 0.0);
}

fn uw_hash(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(0.1031, 0.1030));
    q += dot(q, q.yx + vec2<f32>(33.33));
    return fract((q.x + q.y) * q.x);
}

fn uw_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = uw_hash(i);
    let b = uw_hash(i + vec2<f32>(1.0, 0.0));
    let c = uw_hash(i + vec2<f32>(0.0, 1.0));
    let d = uw_hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Bright web where two drifting noise layers cross, like light focused by
// waves. The focused core is quantized into five steps so it reads as
// illuminated pixel art rather than a smooth photographic blob.
fn uw_caustic(p: vec2<f32>, t: f32) -> f32 {
    let r = mat2x2<f32>(0.80, -0.60, 0.60, 0.80);
    let a = uw_noise(p * 0.85 + vec2<f32>(t * 0.31, -t * 0.22));
    let b = uw_noise((r * p) * 1.20 + vec2<f32>(-t * 0.27, t * 0.35));
    let web = smoothstep(0.54, 0.92, 1.0 - abs(a - b) * 2.3);
    let broad_web = smoothstep(0.58, 0.90, 1.0 - abs(
        uw_noise(p * 0.33 + vec2<f32>(-t * 0.11, t * 0.17)) -
        uw_noise((r * p) * 0.41 + vec2<f32>(t * 0.09, -t * 0.13))) * 1.8);
    let focused = max(web, broad_web * 0.72);
    let stepped = floor(focused * 5.0 + 0.5) / 5.0;
    let fine = uw_noise(p * 2.6 + vec2<f32>(-t * 0.5, t * 0.4));
    return mix(focused, stepped, 0.45) * (0.65 + 0.7 * fine);
}

// Smoother web for volumetric shafts, where fine detail would only read as noise.
fn uw_caustic_soft(p: vec2<f32>, t: f32) -> f32 {
    let r = mat2x2<f32>(0.80, -0.60, 0.60, 0.80);
    let a = uw_noise(p * 0.85 + vec2<f32>(t * 0.31, -t * 0.22));
    let b = uw_noise((r * p) * 1.20 + vec2<f32>(-t * 0.27, t * 0.35));
    return smoothstep(0.40, 0.95, 1.0 - abs(a - b) * 2.0);
}

// Direction sunlight travels once inside the water.
fn uw_sun_travel(sun_dir: vec3<f32>) -> vec3<f32> {
    return normalize(refract(-sun_dir, vec3<f32>(0.0, 1.0, 0.0), UW_REFRACT));
}

fn uw_ambient(daylight: f32, depth: f32) -> vec3<f32> {
    let shallow = vec3<f32>(0.020, 0.150, 0.190);
    let deep = vec3<f32>(0.004, 0.032, 0.058);
    let column = 1.0 - exp(-depth * 0.085);
    return mix(shallow, deep, column) * mix(0.16, 1.0, daylight);
}

// Radiance along `dir` where nothing solid is drawn. `jitter` in [0,1) breaks
// up the shaft sampling pattern.
fn uw_backdrop(dir: vec3<f32>, jitter: f32) -> vec3<f32> {
    let daylight = clamp(light.sun_color_daylight.w, 0.0, 1.0);
    let surface = uw_surface_y();
    let depth = max(surface - cam.camera_pos.y, 0.0);
    let t = light.torch_meta.y;
    let sun = normalize(light.sun_dir_intensity.xyz);
    let sun_col = light.sun_color_daylight.rgb * sky_transmittance_to_sun(sun.y)
        * light.sun_dir_intensity.w;

    var col = uw_ambient(daylight, depth);
    // The view down into the deep is darker than the view across.
    col *= mix(1.0, 0.45, clamp(-dir.y, 0.0, 1.0));

    if dir.y > 0.02 {
        let cos_t = dir.y;
        let sin_i = sqrt(max(1.0 - cos_t * cos_t, 0.0));
        let sin_o = min(sin_i / UW_REFRACT, 1.0);
        let cos_o = sqrt(max(1.0 - sin_o * sin_o, 0.0));
        let h = normalize(dir.xz + vec2<f32>(1e-5));
        let out_dir = normalize(vec3<f32>(h.x * sin_o, cos_o, h.y * sin_o));

        // Where this ray meets the moving surface, for shimmer.
        let hit = cam.camera_pos.xz + dir.xz * (depth / max(cos_t, 0.05));
        let ripple = 0.62 + 0.76 * uw_noise(hit * 0.65 + vec2<f32>(t * 0.45, -t * 0.33))
            + 0.25 * (uw_noise(hit * 2.1 - vec2<f32>(t * 0.9, t * 0.7)) - 0.5);

        // Inside the window (cos(critical) = 0.661) the sky shines through.
        let window = smoothstep(0.60, 0.72, cos_t);
        let path = depth / max(cos_t, 0.2);
        let trans = exp(-UW_ABSORB * path);
        let fresnel = 0.0204 + 0.9796 * pow(1.0 - cos_o, 5.0);
        var sky = atmosphere_color(out_dir);
        sky += sun_col * pow(max(dot(out_dir, sun), 0.0), 420.0) * 3.0;
        let through = sky * trans * (1.0 - fresnel) * ripple * 0.9;

        // Beyond it the surface is a mirror of the dim water body.
        let mirror_web = 0.85 + 0.40 * uw_caustic(hit * 0.5, t);
        let mirror = uw_ambient(daylight, depth) * 1.25 * mirror_web;
        col = mix(col, mix(mirror, through, window), smoothstep(0.02, 0.18, cos_t));
    }

    // Sun shafts: march the view ray, project each sample up the refracted sun
    // direction to the surface and read the caustic web there.
    if daylight > 0.02 && sun.y > 0.05 {
        let travel = uw_sun_travel(sun);
        let to_surface = -travel;
        let max_dist = 36.0;
        let step_len = max_dist / f32(UW_SHAFT_STEPS);
        var shafts = 0.0;
        for (var i = 0; i < UW_SHAFT_STEPS; i = i + 1) {
            let d = (f32(i) + jitter) * step_len;
            let p = cam.camera_pos.xyz + dir * d;
            let h = surface - p.y;
            if h < 0.0 {
                continue;
            }
            let s = p.xz + to_surface.xz * (h / max(to_surface.y, 0.25));
            shafts += uw_caustic_soft(s * 0.45, t) * exp(-0.075 * h) * exp(-0.035 * d);
        }
        let forward = 0.45 + 0.55 * pow(max(dot(dir, travel), 0.0), 2.0);
        col += sun_col * shafts * step_len * 0.0065 * forward * daylight;
    }
    return col;
}

// Cheap stand-in for the backdrop colour, used as the haze target on solid fragments.
fn uw_haze(dir: vec3<f32>) -> vec3<f32> {
    let daylight = clamp(light.sun_color_daylight.w, 0.0, 1.0);
    let depth = max(uw_surface_y() - cam.camera_pos.y, 0.0);
    var col = uw_ambient(daylight, depth);
    col *= mix(1.0, 0.45, clamp(-dir.y, 0.0, 1.0));
    col *= 1.0 + 0.9 * clamp(dir.y, 0.0, 1.0) * daylight;
    return col;
}

// Haze, absorption and caustic light for a solid fragment.
fn uw_apply_fragment(
    lit: vec3<f32>,
    albedo: vec3<f32>,
    world_pos: vec3<f32>,
    normal: vec3<f32>,
    view_dir: vec3<f32>,
    dist: f32,
    sun_visibility: f32,
) -> vec3<f32> {
    let daylight = clamp(light.sun_color_daylight.w, 0.0, 1.0);
    let surface = uw_surface_y();
    let fragment_depth = max(surface - world_pos.y, 0.0);
    let t = light.torch_meta.y;
    let sun = normalize(light.sun_dir_intensity.xyz);
    let sun_col = light.sun_color_daylight.rgb * sky_transmittance_to_sun(sun.y)
        * light.sun_dir_intensity.w;

    var colour = lit;
    // Sunlight and skylight that reached this depth lost its reds on the way down.
    colour *= exp(-UW_ABSORB * fragment_depth * 0.32);

    // Caustics: focused sunlight on faces that look toward the surface.
    if sun.y > 0.05 && fragment_depth > 0.0 {
        let travel = uw_sun_travel(sun);
        let s = world_pos.xz + (-travel.xz) * (fragment_depth / max(-travel.y, 0.25));
        let facing = max(dot(normal, -travel), 0.0);
        let fade = exp(-0.11 * fragment_depth) * smoothstep(0.0, 1.2, fragment_depth);
        let web = uw_caustic(s * 0.55, t);
        colour += albedo * sun_col * web * facing * fade * sun_visibility * daylight * 0.68;
        // Shallow ledges and the seabed near the waterline sit in the caustic
        // spill even where the depth fade above is still near zero. This is the
        // light that visibly wraps onto surrounding rocks and shore shelves.
        let spill = (1.0 - smoothstep(0.0, 1.35, fragment_depth))
            * smoothstep(0.0, 0.18, fragment_depth);
        let spill_web = uw_caustic(world_pos.xz * 0.42 + vec2<f32>(t * 0.05, -t * 0.04), t);
        colour += albedo * sun_col * spill_web * facing * spill
            * sun_visibility * daylight * 0.30;
    }

    // Distance haze takes its colour from what the sky pass shows in that direction.
    let transmission = exp(-UW_ABSORB * dist);
    let haze = uw_haze(view_dir);
    return colour * transmission + haze * (vec3<f32>(1.0) - transmission);
}
