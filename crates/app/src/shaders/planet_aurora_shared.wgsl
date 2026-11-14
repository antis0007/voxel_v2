// One body-space emission field for the globe disc and the sky beyond its limb.
// Heights are scaled to this compact planet, as in planet_air_shared.wgsl.
const PLANET_AURORA_BASE_M: f32 = 650.0;
const PLANET_AURORA_TOP_M: f32 = 1600.0;

fn planet_aurora_emission(p: vec3<f32>, sun: vec3<f32>, radius: f32, t: f32) -> vec3<f32> {
    let r = length(p);
    let up = p / r;
    // The world's north pole is -Z, shared by climate and the globe projection.
    let latitude = -up.z;
    if abs(latitude) < 0.86 { return vec3<f32>(0.0); }
    let night = 1.0 - smoothstep(-0.12, 0.02, dot(up, sun));
    if night <= 0.0 { return vec3<f32>(0.0); }
    let lon = atan2(up.y, up.x);
    let hemisphere = select(0.0, 3.0, latitude < 0.0);
    let lean = dot(up.xy, -sun.xy) / max(length(up.xy) * length(sun.xy), 1e-4);
    // Closed, continuous curtains: no longitude-cell boundaries painted on land.
    let centre = 0.935 - 0.025 * lean
        + 0.008 * sin(3.0 * lon + t * 0.034906585 + hemisphere)
        + 0.004 * sin(7.0 * lon - t * 0.052359878);
    let offset = (abs(latitude) - centre) / 0.012;
    let band = exp(-offset * offset);
    let folds = 0.5 + 0.5 * sin(lon * 48.0 + 2.0 * sin(lon * 7.0) + t * 0.10471976);
    let h = clamp((r - radius - PLANET_AURORA_BASE_M)
        / (PLANET_AURORA_TOP_M - PLANET_AURORA_BASE_M), 0.0, 1.0);
    let envelope = smoothstep(0.0, 0.15, h) * (1.0 - smoothstep(0.45, 1.0, h));
    let tint = mix(vec3<f32>(0.10, 1.0, 0.38), vec3<f32>(0.55, 0.18, 1.0), smoothstep(0.35, 0.90, h));
    return tint * band * (0.25 + 0.75 * folds * folds) * envelope * night;
}

fn planet_aurora_segment(eye: vec3<f32>, ray: vec3<f32>, range: vec2<f32>,
    sun: vec3<f32>, radius: f32, t: f32) -> vec3<f32> {
    if range.y <= range.x { return vec3<f32>(0.0); }
    let a = eye + ray * range.x;
    let b = eye + ray * range.y;
    if max(abs(a.z), abs(b.z)) < (radius + PLANET_AURORA_BASE_M) * 0.86 {
        return vec3<f32>(0.0);
    }
    let step_m = (range.y - range.x) / 6.0;
    var light = vec3<f32>(0.0);
    for (var i = 0u; i < 6u; i += 1u) {
        let p = eye + ray * (range.x + (f32(i) + 0.5) * step_m);
        light += planet_aurora_emission(p, sun, radius, t);
    }
    return light * (step_m * 0.00006);
}

// Integrate only the luminous shell, clipped by the actual foreground surface.
// Splitting at the inner sphere avoids wasting samples through the empty core.
fn planet_aurora_ray(eye: vec3<f32>, ray: vec3<f32>, distance_m: f32,
    sun: vec3<f32>, radius: f32, t: f32) -> vec3<f32> {
    let along = dot(eye, ray);
    let closest = eye - ray * along;
    let impact2 = dot(closest, closest);
    let outer = radius + PLANET_AURORA_TOP_M;
    if impact2 >= outer * outer { return vec3<f32>(0.0); }
    let half_chord = sqrt(max(outer * outer - impact2, 0.0));
    let start = max(-along - half_chord, 0.0);
    let end = min(-along + half_chord, distance_m);
    if end <= start { return vec3<f32>(0.0); }
    let inner = radius + PLANET_AURORA_BASE_M;
    if impact2 >= inner * inner {
        return planet_aurora_segment(eye, ray, vec2<f32>(start, end), sun, radius, t);
    }
    let inner_chord = sqrt(max(inner * inner - impact2, 0.0));
    return planet_aurora_segment(eye, ray, vec2<f32>(start, min(end, -along - inner_chord)), sun, radius, t)
        + planet_aurora_segment(eye, ray, vec2<f32>(max(start, -along + inner_chord), end), sun, radius, t);
}
