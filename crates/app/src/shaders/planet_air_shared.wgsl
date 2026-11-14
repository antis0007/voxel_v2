// One air model for the planet's sky limb, globe ground and cloud shell.
// Body-frame metres throughout; the sky composes it before celestial_sky.wgsl
// and the globe before globe_shader.wgsl, so all three agree at every altitude.

// Coefficients are per metre and larger than Earth's, because the planet is
// 1/300 of Earth's size. The shell stays near 15% of the radius so it reads as
// a thin rim; a grazing ray still reaches a blue optical depth near 1, so the
// limb is blue and only the terminator turns red. `ambience::PLANET_AIR`
// mirrors the heights.
const CEL_AIR_TOP_M: f32 = 3000.0;
const CEL_RAYLEIGH_H_M: f32 = 400.0;
const CEL_MIE_H_M: f32 = 120.0;
const CEL_RAYLEIGH: vec3<f32> = vec3<f32>(0.25e-4, 0.59e-4, 1.4e-4);
const CEL_MIE: f32 = 0.3e-4;
const CEL_SUN_E: f32 = 40.0;
// Above the air a camera exposes for the sunlit disc, not for the sky. The
// globe lights ground with unit sun radiance (irradiance pi), so air keyed to
// the sky's 40 outshone the ocean it lies over about tenfold and the planet
// read as a pastel ball in a neon ring. In orbit the air's source term drops
// toward that surface scale; inside the air the sky keeps its own key.
const CEL_SUN_E_ORBIT: f32 = 6.5;
// Chappuis-band absorber in a raised layer, as ozone: it takes green and red
// from long limb paths, which deepens the limb's blue and turns the twilight
// arc violet. Scaled with the air; it scatters nothing.
const CEL_OZONE: vec3<f32> = vec3<f32>(0.8e-5, 2.3e-5, 0.11e-5);
const CEL_OZONE_CENTRE_M: f32 = 1250.0;
const CEL_OZONE_HALF_M: f32 = 750.0;
// Share of the single-scattered source that returns as second and higher
// orders (Hillaire 2020 folds them into one isotropic term). The air is thin,
// so the share is small in daylight; it matters past the terminator, where
// air still lit from above keeps a blue twilight wedge that single
// scattering alone cuts to black.
const CEL_MULTIPLE: f32 = 0.30;
const CEL_TWILIGHT: vec3<f32> = vec3<f32>(0.035, 0.075, 0.17);
const CEL_AIRGLOW_H_M: f32 = 2200.0;
// The system's 1.2 Mm star at the home world's nominal 150 Mm orbit.
const CEL_SOLAR_ANGULAR_RADIUS: f32 = 0.008;

fn cel_air(h: f32) -> vec2<f32> {
    return vec2<f32>(exp(-max(h, 0.0) / CEL_RAYLEIGH_H_M), exp(-max(h, 0.0) / CEL_MIE_H_M));
}

// The hand-over starts at 4 km, where the streamed chunks (which carry their
// own aerial perspective) have shrunk to a small disc, and completes a
// kilometre above where they are gone, so a climb does not hold the washed
// look into orbit.
fn planet_air_sun_e(eye_altitude: f32) -> f32 {
    return mix(CEL_SUN_E, CEL_SUN_E_ORBIT, smoothstep(4000.0, 7000.0, eye_altitude));
}

// Tent profile of the absorbing layer, 0..1.
fn cel_ozone(h: f32) -> f32 {
    return max(1.0 - abs(h - CEL_OZONE_CENTRE_M) / CEL_OZONE_HALF_M, 0.0);
}

// Absorber column toward the sun from height `h`, metres of full density.
// Closed form: the vertical column left above `h`, times an air mass that
// stays finite at the horizon of this small sphere; a sun below the local
// horizon shines through the whole layer on its way in.
fn cel_ozone_sun_column(h: f32, mu: f32) -> f32 {
    let x = clamp((h - CEL_OZONE_CENTRE_M) / CEL_OZONE_HALF_M, -1.0, 1.0);
    // Integral of the tent from x to 1, in half-widths.
    let above = select(0.5 * (1.0 - x) * (1.0 - x), 1.0 - 0.5 * (1.0 + x) * (1.0 + x), x < 0.0);
    let column = select(above, 2.0 - above, mu < 0.0) * CEL_OZONE_HALF_M;
    return column * min(1.0 / (abs(mu) + 0.09), 11.0);
}

// Float32 filtering is specialized by shader_sources.rs. Keep explicit loads
// for adapters without the optional feature, at identical texel centers.
fn cel_depth_sample(uv: vec2<f32>) -> vec2<f32> {
    let size = vec2<i32>(textureDimensions(planet_depth_texture));
    let texel = uv * vec2<f32>(size - vec2<i32>(1));
    let lo = vec2<i32>(floor(texel));
    let hi = min(lo + vec2<i32>(1), size - vec2<i32>(1));
    let weight = fract(texel);
    return mix(mix(textureLoad(planet_depth_texture, lo, 0).rg,
        textureLoad(planet_depth_texture, vec2<i32>(hi.x, lo.y), 0).rg, weight.x),
        mix(textureLoad(planet_depth_texture, vec2<i32>(lo.x, hi.y), 0).rg,
        textureLoad(planet_depth_texture, hi, 0).rg, weight.x), weight.y);
}

fn cel_sun_depth(mu: f32, horizon: f32, height: f32) -> vec2<f32> {
    // Same horizon-relative quadratic mapping as engine_gpu::atmosphere.
    // Planet blocking remains analytic so interpolation cannot leak sunlight.
    let uv = vec2<f32>(sqrt(clamp((mu - horizon) / (1.0 - horizon), 0.0, 1.0)),
        sqrt(height / CEL_AIR_TOP_M));
    return cel_depth_sample(uv);
}

// Clip the solar disc against the geometric horizon. Near the terminator,
// sample extinction at the centroid of the visible part of the disc rather
// than switching a point light on or off as its center crosses the horizon.
fn cel_sun_transmittance(r: f32, mu: f32, radius: f32) -> vec3<f32> {
    let radius_ratio = min(radius / max(r, 1.0), 1.0);
    let horizon = -sqrt(max(1.0 - radius_ratio * radius_ratio, 0.0));
    let half_width = max(CEL_SOLAR_ANGULAR_RADIUS * radius_ratio, 1e-6);
    let x = (mu - horizon) / half_width;
    if x <= -1.0 { return vec3<f32>(0.0); }
    var visible = 1.0;
    var sample_mu = mu;
    if x < 1.0 {
        let chord = sqrt(max(1.0 - x * x, 0.0));
        let area = max(acos(-x) + x * chord, 1e-6);
        visible = area * 0.318309886;
        sample_mu += half_width * (2.0 / 3.0) * chord * chord * chord / area;
    }
    // At the top, a sunward ray can still cross air if its direction is inward.
    if r >= radius + CEL_AIR_TOP_M && sample_mu >= 0.0 { return vec3<f32>(visible); }
    let height = clamp(r - radius, 0.0, CEL_AIR_TOP_M);
    let depth = cel_sun_depth(sample_mu, horizon, height);
    return visible * exp(-(CEL_RAYLEIGH * depth.x + vec3<f32>(CEL_MIE * 1.1 * depth.y)
        + CEL_OZONE * cel_ozone_sun_column(height, sample_mu)));
}

// Quadratic spacing on each side of closest approach resolves the dense
// lower atmosphere on ascending, descending and grazing rays alike.
fn planet_air_distance(u: f32, start: f32, end: f32, closest: f32) -> f32 {
    let split = (closest - start) / max(end - start, 1e-4);
    if u < split {
        let q = (split - u) / max(split, 1e-4);
        return closest - (closest - start) * q * q;
    }
    let q = (u - split) / max(1.0 - split, 1e-4);
    return closest + (end - closest) * q * q;
}

// Stable (1-exp(-tau))/tau, including nearly empty high-altitude segments.
fn planet_air_integral(tau: vec3<f32>, segment_transmittance: vec3<f32>) -> vec3<f32> {
    let series = vec3<f32>(1.0) - tau * 0.5 + tau * tau * (1.0 / 6.0);
    return select((vec3<f32>(1.0) - segment_transmittance) / max(tau, vec3<f32>(1e-6)),
        series, tau < vec3<f32>(0.001));
}

struct PlanetAir {
    // Sunlight scattered toward the eye, in the sky's radiance units.
    inscatter: vec3<f32>,
    // What survives of the light behind the segment.
    transmittance: vec3<f32>,
    // Night-side airglow column, already in radiance units.
    airglow: vec3<f32>,
};

// Single scattering on the eye ray from the eye to `t_end` metres (or the air's
// far side), clipped to the air shell and the ground. `jitter` (0..1 per
// pixel) offsets the samples so steps never show as rings.
fn planet_air_segment(eye: vec3<f32>, dir: vec3<f32>, sun: vec3<f32>, radius: f32, t_end: f32, steps: i32, jitter: f32) -> PlanetAir {
    var out = PlanetAir(vec3<f32>(0.0), vec3<f32>(1.0), vec3<f32>(0.0));
    let top = radius + CEL_AIR_TOP_M;
    let b = dot(eye, dir);
    // Closest-point form avoids subtracting two orbital-distance squares.
    let closest_point = eye - dir * b;
    let miss2 = dot(closest_point, closest_point);
    let h_top = top * top - miss2;
    if h_top <= 0.0 { return out; }
    let t0 = max(-b - sqrt(h_top), 0.0);
    var t1 = min(-b + sqrt(h_top), t_end);
    let h_ground = radius * radius - miss2;
    if h_ground > 0.0 && b < 0.0 && dot(eye, eye) >= radius * radius {
        t1 = min(t1, max(-b - sqrt(h_ground), 0.0));
    }
    if t1 <= t0 { return out; }
    let mu = dot(dir, sun);
    let phase_r = 0.0597 * (1.0 + mu * mu);
    let g = 0.76;
    let phase_m = 0.1194 * (1.0 - g * g) * (1.0 + mu * mu) / ((2.0 + g * g) * pow(1.0 + g * g - 2.0 * g * mu, 1.5));
    let closest = clamp(-b, t0, t1);
    let count = max(steps, 1);
    let sun_e = planet_air_sun_e(length(eye) - radius);
    var start = t0;
    for (var i = 0; i < count; i++) {
        let end = planet_air_distance(f32(i + 1) / f32(count), t0, t1, closest);
        let ds = end - start;
        let p = eye + dir * mix(start, end, clamp(jitter, 0.0, 1.0));
        let r = length(p);
        let h = r - radius;
        let air = cel_air(h);
        let extinction = CEL_RAYLEIGH * air.x + vec3<f32>(CEL_MIE * 1.1 * air.y) + CEL_OZONE * cel_ozone(h);
        let tau = extinction * ds;
        let segment_transmittance = exp(-tau);
        // Hillaire's analytic constant-medium segment integral preserves
        // energy with coarse steps, instead of a midpoint attenuation guess.
        let integral = out.transmittance * planet_air_integral(tau, segment_transmittance) * ds;
        let sun_mu = dot(p, sun) / max(r, 1.0);
        let sun_through = cel_sun_transmittance(r, sun_mu, radius);
        let scattering = CEL_RAYLEIGH * air.x * phase_r + vec3<f32>(CEL_MIE * air.y * phase_m);
        // Isotropic higher orders: lit by the sun where it shines, and by the
        // sunlit sky overhead for a few degrees past the terminator.
        let diffuse_light = max(sun_through, CEL_TWILIGHT * smoothstep(-0.24, 0.02, sun_mu));
        let multiple = (CEL_RAYLEIGH * air.x + vec3<f32>(CEL_MIE * air.y)) * (0.0796 * CEL_MULTIPLE) * diffuse_light;
        out.inscatter += sun_e * integral * (scattering * sun_through + multiple);
        let dark = 1.0 - smoothstep(-0.1, 0.15, sun_mu);
        let glow_height = (h - CEL_AIRGLOW_H_M) / 120.0;
        out.airglow += vec3<f32>(0.22, 0.48, 0.62) * exp(-glow_height * glow_height) * dark * integral * 1.5e-6;
        out.transmittance *= segment_transmittance;
        start = end;
    }
    return out;
}
