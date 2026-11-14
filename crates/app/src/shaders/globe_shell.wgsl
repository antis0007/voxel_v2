// Cloud shell, cloud shadow and night lights; appended to globe_shader.wgsl.

// Coverage, shadows and storm effects share the same per-face transition.
// Cubic B-spline from four bilinear taps. The bake is coarse next to the
// screen; bilinear reconstruction drew its thresholded texels as squares.
fn globe_shell_bicubic(uv: vec2<f32>, layer: i32, lod: f32) -> f32 {
    let size = vec2<f32>(textureDimensions(shell_coverage).xy) / exp2(floor(lod));
    let st = uv * size - 0.5;
    let i = floor(st);
    let f = st - i;
    let f2 = f * f;
    let f3 = f2 * f;
    let w0 = (1.0 - 3.0 * f + 3.0 * f2 - f3) / 6.0;
    let w1 = (4.0 - 6.0 * f2 + 3.0 * f3) / 6.0;
    let w2 = (1.0 + 3.0 * f + 3.0 * f2 - 3.0 * f3) / 6.0;
    let w3 = f3 / 6.0;
    let g0 = w0 + w1;
    let g1 = w2 + w3;
    let h0 = (i - 0.5 + w1 / g0) / size;
    let h1 = (i + 1.5 + w3 / g1) / size;
    let a = textureSampleLevel(shell_coverage, shell_sampler, vec2<f32>(h0.x, h0.y), layer, lod).r;
    let b = textureSampleLevel(shell_coverage, shell_sampler, vec2<f32>(h1.x, h0.y), layer, lod).r;
    let c = textureSampleLevel(shell_coverage, shell_sampler, vec2<f32>(h0.x, h1.y), layer, lod).r;
    let d = textureSampleLevel(shell_coverage, shell_sampler, vec2<f32>(h1.x, h1.y), layer, lod).r;
    return mix(mix(a, b, g1.x), mix(c, d, g1.x), g1.y);
}

fn globe_shell_sample(uv: vec2<f32>, face: u32, history: u32, lod: f32) -> f32 {
    if all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0)) {
        return globe_shell_bicubic(uv, i32(face + history), lod);
    }
    // Advection can cross a cube edge; clamping to the old face pins a stripe there.
    let axes = globe_axes(face);
    let st = (uv * 2.0 - vec2<f32>(1.0)) * GLOBE_FRAC_PI_4;
    let dir = normalize(axes[1] + tan(st.x) * axes[0] + tan(st.y) * axes[2]);
    let chart = globe_chart_of(dir);
    let target_face = u32(chart.z);
    let face_meta = select(shell_blend.current[target_face], shell_blend.previous[target_face], history != 0u);
    if face_meta.w < 0.5 {
        return textureSampleLevel(shell_coverage, shell_sampler, uv, i32(face + history), lod).r;
    }
    let wrapped_uv = (chart.xy + GLOBE_HALF_M) / (2.0 * GLOBE_HALF_M);
    return textureSampleLevel(shell_coverage, shell_sampler, wrapped_uv, i32(target_face + history), max(lod, face_meta.z)).r;
}

fn globe_shell_weather(uv: vec2<f32>, face: u32, lod: f32) -> f32 {
    let old = shell_blend.previous[face];
    let fresh = shell_blend.current[face];
    if fresh.w < 0.5 { return 0.0; }
    let weight = shell_blend.weights[face / 4u][face % 4u];
    let new_uv = globe_weather_uv(uv, face, 0u, fresh, shell_blend.current_time[face / 4u][face % 4u]);
    if weight >= 1.0 {
        return globe_shell_sample(new_uv, face, 0u, max(lod, fresh.z));
    }
    let old_uv = globe_weather_uv(uv, face, 6u, old, shell_blend.previous_time[face / 4u][face % 4u]);
    let previous = globe_shell_sample(old_uv, face, 6u, max(lod, old.z));
    if weight <= 0.0 { return previous; }
    let current = globe_shell_sample(new_uv, face, 0u, max(lod, fresh.z));
    return mix(previous, current, weight);
}

// The baked GBA channels carry the weather catalogue's body-space wind.
// Advect each snapshot from its own time, rather than sliding cyclones with a
// single flat-world wind. Decoding around 128 leaves still air exactly still.
fn globe_weather_uv(uv: vec2<f32>, face: u32, history: u32, snapshot: vec4<f32>, time: f32) -> vec2<f32> {
    let local_uv = uv + (shell_blend.wind_m.xy - snapshot.xy) / (2.0 * GLOBE_HALF_M);
    let orbital = globe_satellite_blend();
    if orbital <= 0.0 || snapshot.w < 0.5 { return local_uv; }
    let axes = globe_axes(face);
    let st = (uv * 2.0 - 1.0) * GLOBE_FRAC_PI_4;
    let n = normalize(axes[1] + tan(st.x) * axes[0] + tan(st.y) * axes[2]);
    let encoded = textureSampleLevel(shell_coverage, shell_sampler, uv, i32(face + history), snapshot.z).gba;
    let velocity = (encoded * 255.0 - 128.0) * (0.16 / 127.0);
    let elapsed = clamp(shell_blend.wind_m.z - time, 0.0, 24.0) * 0.12;
    let traced = normalize(n - velocity * elapsed);
    let dn = max(dot(traced, axes[1]), 0.1);
    let chart = vec2<f32>(atan(dot(traced, axes[0]) / dn), atan(dot(traced, axes[2]) / dn));
    return mix(local_uv, chart / (2.0 * GLOBE_FRAC_PI_4) + 0.5, orbital);
}

fn globe_weather_wind(n: vec3<f32>) -> vec3<f32> {
    let chart = globe_chart_of(n);
    let face = u32(chart.z);
    let snapshot = shell_blend.current[face];
    if snapshot.w < 0.5 { return vec3<f32>(0.0); }
    let uv = (chart.xy + GLOBE_HALF_M) / (2.0 * GLOBE_HALF_M);
    let encoded = textureSampleLevel(shell_coverage, shell_sampler, uv, i32(face), snapshot.z).gba;
    let v = (encoded * 255.0 - 128.0) * (0.16 / 127.0);
    return v - n * dot(n, v);
}

fn globe_shell_coverage(uv: vec2<f32>, face: u32, lod: f32) -> f32 {
    let chart = (uv * 2.0 - vec2<f32>(1.0)) * GLOBE_HALF_M;
    let dir = globe_dir(face, chart.x, chart.y);
    let largest = max(max(abs(dir.x), abs(dir.y)), abs(dir.z));
    var coverage = 0.0;
    var total = 0.0;
    // Overlap charts exactly as canonical_sample() does for weather. A vortex
    // can straddle a cube edge without switching abruptly to another street.
    for (var f = 0u; f < 6u; f++) {
        let axes = globe_axes(f);
        let normal = dot(dir, axes[1]);
        let weight = smoothstep(0.85, 1.0, normal / largest);
        if weight <= 0.0 { continue; }
        let local = vec2<f32>(atan(dot(dir, axes[0]) / normal), atan(dot(dir, axes[2]) / normal))
            / GLOBE_FRAC_PI_4 * GLOBE_HALF_M;
        let world_xz = local + vec2<f32>(f32(f) * GLOBE_PITCH_M, 0.0);
        let wind = shell_blend.flow.xy;
        let speed = shell_blend.flow.z;
        let time = shell_blend.flow.w;
        var flow = vec3<f32>(0.0);
        if shell_blend.flow_shape.w > 0.5 {
            flow = cloud_flow_drift_at(world_xz, 0.5, 0, wind, speed, time, shell_blend.flow_shape.x)
                + vec3<f32>(globe_vortex_streets(world_xz, wind, speed * 8.0 * 1.65 * 0.85, time)
                    * cloud_street_weight(0, speed), 0.0);
        } else {
            flow = cloud_flow_warp_at(world_xz, 0.5, 0, wind, speed, time,
                shell_blend.flow_shape.x, speed * 8.0 * 1.65);
        }
        let warped_uv = (local + flow.xy + GLOBE_HALF_M) / (2.0 * GLOBE_HALF_M);
        let streets = cloud_street_rolls_at(world_xz + flow.xy + shell_blend.wind_m.xy,
            wind, clamp(shell_blend.flow_shape.y, 0.0, 1.0), speed);
        coverage += clamp(globe_shell_weather(warped_uv, f, lod) * streets, 0.0, 1.0) * weight;
        total += weight;
    }
    coverage /= max(total, 1e-4);
    if coverage <= 0.0 || coverage >= 1.0 || globe_satellite_blend() >= 0.999 {
        return clamp(coverage, 0.0, 1.0);
    }
    let structure = globe_cloud_structure(dir);
    // Preserve clear sky and solid cores; carve the broad transition into
    // broken filaments. Shadows sample this same field, including cube edges.
    return clamp(coverage - structure.x * 4.0 * coverage * (1.0 - coverage)
        * (1.0 - globe_satellite_blend()), 0.0, 1.0);
}

fn globe_cloud_lod(dir: vec3<f32>) -> f32 {
    let distance_m = distance(dir * (globe.radius_m + GLOBE_SHELL_M), globe.eye_body.xyz);
    let footprint = distance_m / max(globe.focal_px, 1.0) / (2.0 * GLOBE_HALF_M);
    return max(log2(max(footprint * f32(textureDimensions(shell_coverage).x), 1.0)), 0.0);
}

// Projection::Tangent::from_sphere: face whose normal is nearest, then face-local chart metres.
fn globe_chart_of(dir: vec3<f32>) -> vec3<f32> {
    var face = 0u;
    var best = -2.0;
    for (var f = 0u; f < 6u; f++) {
        let dn = dot(dir, globe_axes(f)[1]);
        if dn > best {
            best = dn;
            face = f;
        }
    }
    let a = globe_axes(face);
    let dn = max(best, 1e-6);
    let s = atan(dot(dir, a[0]) / dn) / GLOBE_FRAC_PI_4;
    let t = atan(dot(dir, a[2]) / dn) / GLOBE_FRAC_PI_4;
    return vec3<f32>(clamp(s, -1.0, 1.0) * GLOBE_HALF_M, clamp(t, -1.0, 1.0) * GLOBE_HALF_M, f32(face));
}

// Worker filters occupied area times column opacity, preserving thin coverage.
fn globe_cover(c: f32) -> f32 {
    let types = shell_blend.flow_shape.y + shell_blend.flow_shape.z
        + dot(shell_blend.layers, vec4<f32>(1.0));
    if shell_blend.appearance.x <= 0.0 || types <= 0.0 { return 0.0; }
    return clamp(c, 0.0, 1.0);
}

fn globe_shell_alpha(dir: vec3<f32>) -> f32 {
    if globe_cover(1.0) <= 0.0 { return 0.0; }
    return globe_cover(globe_zonal_coverage(dir, globe_cloud_lod(dir), globe_zonal_amount()));
}

// Obstacle lattice window; render/globe/shell_obstacles.rs fills it.
const GLOBE_OBSTACLE_X0: i32 = -4;
const GLOBE_OBSTACLE_Z0: i32 = -4;
const GLOBE_OBSTACLE_COLUMNS: i32 = 45;
const GLOBE_OBSTACLE_ROWS: i32 = 8;

// Street strength of lattice cell `id` from peaks and islands; zero is no obstacle.
fn globe_obstacle_weight(id: vec2<i32>) -> f32 {
    let c = id - vec2<i32>(GLOBE_OBSTACLE_X0, GLOBE_OBSTACLE_Z0);
    if any(c < vec2<i32>(0)) || c.x >= GLOBE_OBSTACLE_COLUMNS || c.y >= GLOBE_OBSTACLE_ROWS { return 0.0; }
    let k = u32(c.x * GLOBE_OBSTACLE_ROWS + c.y);
    return shell_blend.obstacles[k / 4u][k % 4u];
}

// Karman streets shed by real high ground, at the march's lattice sites.
fn globe_vortex_streets(xz: vec2<f32>, wind: vec2<f32>, drift: f32, time: f32) -> vec2<f32> {
    let cell0 = vec2<i32>(floor(xz / FLOW_CELL_M));
    var d = vec2<f32>(0.0);
    for (var z = -1; z <= 1; z += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let id = cell0 + vec2<i32>(x, z);
            let w = globe_obstacle_weight(id);
            if w <= 0.0 { continue; }
            d += cloud_vortex_wake(xz, id, wind, drift, time) * w;
        }
    }
    return d;
}

fn globe_coverage_at(dir: vec3<f32>, lod: f32) -> f32 {
    let chart = globe_chart_of(dir);
    let uv = (chart.xy + GLOBE_HALF_M) / (2.0 * GLOBE_HALF_M);
    return globe_shell_coverage(uv, u32(chart.z), lod);
}

// Height of the cloud top above the shell for a coverage value. Thin margins
// stay low; cores and storm towers climb, so the deck has relief from orbit.
const GLOBE_SHELL_TOP_M: f32 = 280.0;
// Zonal flow map period; two phases half a period apart hide the reset.
const GLOBE_ZONAL_PERIOD_S: f32 = 1200.0;

// A kilometre-high deck is a large fraction of this compact planet's radius.
// Share the compact flight-through altitude, and taper relief after the
// volumetric handoff. Keep a shallow 3D top even in the satellite view.
fn globe_satellite_blend() -> f32 {
    return smoothstep(GLOBE_HANDOFF_HIGH_M, 10000.0,
        length(globe.eye_body.xyz) - globe.radius_m);
}

fn globe_shell_relief_m() -> f32 {
    return GLOBE_SHELL_TOP_M * mix(1.0, 0.30, globe_satellite_blend());
}

fn globe_cloud_top_m(c: f32) -> f32 {
    return globe_shell_relief_m() * c * sqrt(c);
}

// Trade easterlies, mid-latitude westerlies and weak polar easterlies,
// relative to one global wind, in units of the layer speed.
fn globe_zonal_speed(n: vec3<f32>) -> f32 {
    let s = abs(dot(n, GLOBE_NORTH));
    let trades = 1.0 - smoothstep(0.30, 0.50, s);
    let westerlies = smoothstep(0.35, 0.55, s) * (1.0 - smoothstep(0.75, 0.90, s));
    let polar = smoothstep(0.85, 0.95, s);
    return westerlies * 1.4 - trades - polar * 0.4;
}

// The march has one uniform wind, so belts shear only well above the handoff.
fn globe_zonal_amount() -> f32 {
    let altitude = length(globe.eye_body.xyz) - globe.radius_m;
    return smoothstep(GLOBE_HANDOFF_HIGH_M, 25000.0, altitude) * (1.0 - globe_satellite_blend());
}

fn globe_zonal_dir(n: vec3<f32>, displacement_m: f32) -> vec3<f32> {
    let east = cross(GLOBE_NORTH, n);
    let len = length(east);
    if len < 1e-4 { return n; }
    let angle = displacement_m / (globe.radius_m + GLOBE_SHELL_M);
    return normalize(n * cos(angle) + (east / len) * sin(angle));
}

// Two-phase flow map: x, y phase displacement in metres, z weight of phase a.
fn globe_zonal_phases(n: vec3<f32>, amount: f32) -> vec3<f32> {
    let speed = shell_blend.flow.z * 8.0 * 1.65 * 0.35 * globe_zonal_speed(n) * amount;
    let t = shell_blend.flow.w / GLOBE_ZONAL_PERIOD_S;
    let a = fract(t);
    let b = fract(t + 0.5);
    return vec3<f32>((a - 0.5) * speed * GLOBE_ZONAL_PERIOD_S,
        (b - 0.5) * speed * GLOBE_ZONAL_PERIOD_S, 1.0 - abs(2.0 * a - 1.0));
}

fn globe_zonal_coverage(n: vec3<f32>, lod: f32, amount: f32) -> f32 {
    // The weather texture already contains zonal circulation and storm winds.
    // A second global flow-map would split each front into two ghost systems.
    if amount <= 0.0 || globe_satellite_blend() >= 0.999 { return globe_coverage_at(n, lod); }
    let ph = globe_zonal_phases(n, amount);
    let ca = globe_coverage_at(globe_zonal_dir(n, ph.x), lod);
    let cb = globe_coverage_at(globe_zonal_dir(n, ph.y), lod);
    return mix(cb, ca, ph.z);
}

// Unwarped single-face lookup for parallax and self-shadow taps.
fn globe_coverage_fast(n: vec3<f32>, lod: f32, amount: f32) -> f32 {
    var dir = n;
    if amount > 0.0 {
        let ph = globe_zonal_phases(n, amount);
        dir = globe_zonal_dir(n, select(ph.y, ph.x, ph.z >= 0.5));
    }
    let chart = globe_chart_of(dir);
    let uv = (chart.xy + GLOBE_HALF_M) / (2.0 * GLOBE_HALF_M);
    return clamp(globe_shell_weather(uv, u32(chart.z), lod), 0.0, 1.0);
}

// Follow an outward ray to the next concentric shell. The rationalized root
// stays accurate for small rises and bounds the shift at grazing angles.
fn globe_shell_ray_dir(n: vec3<f32>, ray: vec3<f32>, radius: f32, rise: f32) -> vec3<f32> {
    let radial = radius * max(dot(n, ray), 0.0);
    let radius_sq_delta = rise * (2.0 * radius + rise);
    let travel = radius_sq_delta / max(sqrt(radial * radial + radius_sq_delta) + radial, 1e-4);
    return normalize(n * radius + ray * travel);
}

// Steep parallax through the cloud-top height field: x, y, z hit direction,
// w height of the hit above the shell.
fn globe_shell_parallax(n: vec3<f32>, to_eye: vec3<f32>, lod: f32, amount: f32) -> vec4<f32> {
    let radius = globe.radius_m + GLOBE_SHELL_M;
    // Linear refinement between the last miss and the first hit; stopping at
    // the step drew the tops as stacked terraces.
    let relief_m = globe_shell_relief_m();
    var prev_h = relief_m;
    var prev_gap = -relief_m;
    for (var k = 0; k < 6; k++) {
        let h = relief_m * (1.0 - (f32(k) + 1.0) / 6.0);
        let dir = globe_shell_ray_dir(n, to_eye, radius, h);
        let gap = globe_cloud_top_m(globe_coverage_fast(dir, lod, amount)) - h;
        if gap >= 0.0 {
            let t = prev_gap / min(prev_gap - gap, -1e-3);
            let hit_h = mix(prev_h, h, clamp(t, 0.0, 1.0));
            return vec4<f32>(globe_shell_ray_dir(n, to_eye, radius, hit_h), hit_h);
        }
        prev_h = h;
        prev_gap = gap;
    }
    return vec4<f32>(n, 0.0);
}

// Cloud tops toward the sun shade the deck beside them; long at the terminator.
fn globe_shell_self_shadow(n: vec3<f32>, h: f32, sun: vec3<f32>, lod: f32, amount: f32) -> f32 {
    let radius = globe.radius_m + GLOBE_SHELL_M + h;
    let step_m = globe_shell_relief_m() * 0.25;
    var occlusion = 0.0;
    for (var k = 1; k <= 3; k++) {
        let rise = step_m * f32(k);
        let dir = globe_shell_ray_dir(n, sun, radius, rise);
        let top = globe_cloud_top_m(globe_coverage_fast(dir, lod + 1.0, amount));
        occlusion += smoothstep(0.0, step_m, top - h - rise);
    }
    return exp(-occlusion * 0.75);
}

fn globe_shell_hg(mu: f32, g: f32) -> f32 {
    let d = 1.0 + g * g - 2.0 * g * mu;
    return (1.0 - g * g) / (12.566371 * d * sqrt(d));
}

// Above the deck, view extinction comes directly from the atmosphere's
// spherical optical-depth table. The thin air above 1600 m needs only one
// density-weighted solar sample; lower flight keeps the full air integral.
fn globe_orbital_cloud_air(p: vec3<f32>, to_eye: vec3<f32>, sun: vec3<f32>) -> PlanetAir {
    let r = length(p);
    let n = p / r;
    let mu = max(dot(n, to_eye), 0.0);
    let horizon = -sqrt(max(1.0 - pow(globe.radius_m / r, 2.0), 0.0));
    let depth = cel_sun_depth(mu, horizon, r - globe.radius_m);
    let tau = CEL_RAYLEIGH * depth.x + vec3<f32>(CEL_MIE * 1.1 * depth.y);
    let transmittance = exp(-tau);
    let phase_mu = dot(-to_eye, sun);
    let phase_r = 0.0597 * (1.0 + phase_mu * phase_mu);
    let g = 0.76;
    let phase_m = 0.1194 * (1.0 - g * g) * (1.0 + phase_mu * phase_mu)
        / ((2.0 + g * g) * pow(1.0 + g * g - 2.0 * g * phase_mu, 1.5));
    let sample_dir = globe_shell_ray_dir(n, to_eye, r, CEL_RAYLEIGH_H_M);
    let sample_r = r + CEL_RAYLEIGH_H_M;
    let sun_through = cel_sun_transmittance(sample_r, dot(sample_dir, sun), globe.radius_m);
    let scattering = CEL_RAYLEIGH * depth.x * phase_r + vec3<f32>(CEL_MIE * depth.y * phase_m);
    let inscatter = planet_air_sun_e(length(globe.eye_body.xyz) - globe.radius_m)
        * planet_air_integral(tau, transmittance) * scattering * sun_through;
    // Integrate the same high-altitude Gaussian airglow, using the ray's
    // incidence at the layer rather than the horizon's flat-plane secant.
    let glow_r = globe.radius_m + CEL_AIRGLOW_H_M;
    let impact2 = r * r * (1.0 - mu * mu);
    let glow_mu = sqrt(max(1.0 - impact2 / (glow_r * glow_r), 0.01));
    let glow_dir = globe_shell_ray_dir(n, to_eye, r, max(glow_r - r, 0.0));
    let dark = 1.0 - smoothstep(-0.1, 0.15, dot(glow_dir, sun));
    let airglow = vec3<f32>(0.22, 0.48, 0.62) * (212.694 / glow_mu) * dark * 1.5e-6 * sqrt(transmittance);
    return PlanetAir(inscatter, transmittance, airglow);
}

// Land fraction under the fragment from its own tile, so the mask never
// crosses a tile edge.
fn globe_shell_land(slot: u32, chart_min: vec2<f32>, cell_m: f32, chart: vec2<f32>) -> f32 {
    let g = clamp((chart - chart_min) / cell_m, vec2<f32>(0.0), vec2<f32>(f32(GLOBE_LAST)));
    let i = vec2<u32>(min(floor(g), vec2<f32>(f32(GLOBE_LAST - 1u))));
    let f = g - vec2<f32>(i);
    var land = vec4<f32>(0.0);
    for (var k = 0u; k < 4u; k++) {
        let q = i + vec2<u32>(k & 1u, k >> 1u);
        let raw = textureLoad(height_atlas, globe_texel(slot, q.x, q.y), 0).y;
        land[k] = select(1.0, 0.0, (raw & GLOBE_WATER_BIT) != 0u);
    }
    return mix(mix(land.x, land.y, f.x), mix(land.z, land.w, f.x), f.y);
}

// Mesoscale forms readable from orbit: open cells (rings around clear
// centres) in broken ocean decks, closed cells (bright centres, dark rims)
// in solid decks, popcorn cumulus over land, smooth anvils with overshooting
// tops over storms. x carves coverage, y shifts relief, z raises the top.
fn globe_cloud_forms(n: vec3<f32>, c: f32, land: f32, world_footprint: f32) -> vec3<f32> {
    let footprint = world_footprint / globe_cloud_element_scale();
    let resolved = (1.0 - smoothstep(400.0, 1400.0, footprint))
        * clamp(shell_blend.appearance.z, 0.0, 1.0);
    if resolved <= 0.0 { return vec3<f32>(0.0); }
    let q = globe_cloud_coordinates(n);
    let cell = globe_cloud_noise(q / 1450.0 + vec3<f32>(3.1, 17.2, -5.4));
    let ring = 1.0 - abs(2.0 * cell - 1.0);
    let broken = 4.0 * c * (1.0 - c);
    let ocean = 1.0 - land;
    let open_w = broken * ocean * smoothstep(0.25, 0.55, 1.0 - c);
    let closed_w = smoothstep(0.55, 0.80, c) * ocean * (1.0 - smoothstep(0.88, 0.95, c));
    let storm = smoothstep(0.84, 0.95, c);
    var carve = open_w * (0.55 - smoothstep(0.30, 0.85, ring)) * 0.9;
    var relief = closed_w * (0.35 - ring) * 0.6;
    var lift = 0.0;
    // Small clouds form around the margins of a weather system. Leave dry
    // air clear and keep thick frontal decks continuous.
    let field = smoothstep(0.055, 0.19, c) * (1.0 - smoothstep(0.44, 0.73, c));
    let puffs = smoothstep(0.36, 0.69, globe_cloud_noise(q / 365.0 + vec3<f32>(12.7, -6.4, 1.7)));
    carve += (0.31 - 0.84 * puffs) * field * (1.0 - smoothstep(140.0, 475.0, footprint));
    let convective = clamp(shell_blend.flow_shape.y, 0.0, 1.0);
    let day = smoothstep(0.0, 0.25, dot(n, normalize(globe.sun_dir_body.xyz)));
    if land > 0.01 && footprint < 700.0 {
        let pop = globe_cloud_noise(q / 330.0 + vec3<f32>(-8.3, 2.2, 11.9));
        let w = land * broken * day * mix(0.4, 1.0, convective) * (1.0 - smoothstep(250.0, 700.0, footprint));
        carve += (0.5 - smoothstep(0.45, 0.75, pop)) * w * 0.8;
        relief += (pop - 0.5) * w * 0.5;
    }
    if storm > 0.0 {
        let top = smoothstep(0.72, 0.90, globe_cloud_noise(q / 900.0 + vec3<f32>(5.5, -1.7, 2.9)));
        carve *= 1.0 - storm;
        relief = mix(relief, 0.25 + top * 0.35, storm);
        lift = top * storm * 0.35;
    }
    return vec3<f32>(carve * mix(0.35, 1.0, shell_blend.appearance.y), relief, lift) * resolved;
}

fn globe_hash(a: u32, b: u32, c: u32) -> f32 {
    var h = (a * 0x9e3779b1u) ^ ((b + 0x7f4a7c15u) * 0x85ebca77u) ^ ((c + 0x165667b1u) * 0xc2b2ae3du);
    h ^= h >> 15u;
    h *= 0x2c1b3c6du;
    h ^= h >> 12u;
    h *= 0x297a2d39u;
    h ^= h >> 15u;
    return f32(h) * (1.0 / 4294967295.0);
}

fn globe_cloud_noise(p: vec3<f32>) -> f32 {
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    return textureSampleLevel(cloud_detail_lattice, cloud_detail_sampler, (floor(p) + u + 0.5) / 64.0, 0.0).r;
}

// Rotate the detail lattice away from the cube-face axes. Weather still uses
// the shared advected field, while small billows no longer resemble square tiles.
// Match the compact world's surface element scale, including the size dial.
fn globe_cloud_element_scale() -> f32 {
    let world_scale = select(1.0, 0.58, (globe.flags & 64u) != 0u);
    return world_scale * cloud_size_ratio(shell_blend.appearance.w);
}

fn globe_cloud_coordinates(dir: vec3<f32>) -> vec3<f32> {
    let p = dir * (globe.radius_m + GLOBE_SHELL_M)
        + vec3<f32>(shell_blend.wind_m.x, 0.0, shell_blend.wind_m.y);
    return mat3x3<f32>(vec3<f32>(0.36, 0.48, -0.80),
        vec3<f32>(-0.80, 0.60, 0.0), vec3<f32>(0.48, 0.64, 0.60)) * p / globe_cloud_element_scale();
}

// Continuous body-space erosion crosses cube faces without a texture seam.
// Baked weather banks provide the shape; the cached volume supplies the
// missing billow scales, filtered by pixel size.
fn globe_cloud_structure(dir: vec3<f32>) -> vec2<f32> {
    if globe_satellite_blend() >= 0.999 { return globe_orbital_structure(dir); }
    let p = dir * (globe.radius_m + GLOBE_SHELL_M);
    let q = globe_cloud_coordinates(dir);
    let footprint = distance(p, globe.eye_body.xyz) / max(globe.focal_px, 1.0) / globe_cloud_element_scale();
    let resolved = 1.0 - smoothstep(180.0, 600.0, footprint);
    if resolved <= 0.0 { return vec2<f32>(0.0, 0.5); }
    let broad = globe_cloud_noise(q / 305.0);
    var fine = 0.5;
    if footprint < 150.0 {
        fine = mix(0.5, globe_cloud_noise(q / 98.0 + vec3<f32>(19.3, -7.1, 31.7)),
            1.0 - smoothstep(35.0, 150.0, footprint));
    }
    let billows = mix(broad, fine, 0.20);
    let local = vec2<f32>((billows - 0.46) * 0.44 * resolved, mix(0.5, billows, resolved));
    let satellite = globe_satellite_blend();
    if satellite <= 0.0 { return local; }
    return mix(local, globe_orbital_structure(dir), satellite);
}

// Native-pixel detail, independent of the coarse weather atlas. Differential
// advection stretches small cloud elements around lows and along fronts.
// Two overlapping phases avoid endlessly stretched texture or periodic pops.
fn globe_orbital_grain(n: vec3<f32>, velocity: vec3<f32>, elapsed: f32, footprint: f32) -> f32 {
    let mid = normalize(n - velocity * elapsed * 0.06);
    let flow = globe_weather_wind(mid);
    let traced = normalize(n - flow * elapsed * 0.12);
    let rotation = mat3x3<f32>(vec3<f32>(0.36, 0.48, -0.80),
        vec3<f32>(-0.80, 0.60, 0.0), vec3<f32>(0.48, 0.64, 0.60));
    let scale = globe_cloud_element_scale();
    var q = rotation * traced * ((globe.radius_m + GLOBE_SHELL_M) / scale);
    let liquid = shell_blend.flow_shape.y + shell_blend.flow_shape.z
        + shell_blend.layers.x + shell_blend.layers.y + shell_blend.layers.z;
    let ice = shell_blend.layers.w / max(liquid + shell_blend.layers.w, 0.001);
    // Ice-rich columns stretch the same advected lattice into fine fibres.
    // Use a fixed body frame: a basis from coarse wind texels would form seams.
    let fibre_scale = mix(1.0, 5.0, ice * ice);
    q.z *= fibre_scale;
    let filtered_footprint = footprint / scale * fibre_scale;
    let warp = vec3<f32>(globe_cloud_noise(q / 910.0),
        globe_cloud_noise(q / 910.0 + vec3<f32>(13.7, -4.2, 9.1)),
        globe_cloud_noise(q / 910.0 + vec3<f32>(-5.4, 17.6, 2.3))) - 0.5;
    q += warp * 570.0;
    // Differential backtracing supplies the stretch. Projecting absolute
    // coordinates onto a changing wind axis amplified coarse flow texels.
    let large = globe_cloud_noise(q / 210.0);
    var grain = large * 0.65 + 0.175;
    if filtered_footprint < 90.0 {
        grain += (globe_cloud_noise(q / 73.0 + vec3<f32>(7.2, 3.7, -9.1)) - 0.5)
            * 0.25 * (1.0 - smoothstep(24.0, 90.0, filtered_footprint));
    }
    if filtered_footprint < 30.0 {
        grain += (globe_cloud_noise(q / 27.0 + vec3<f32>(-3.7, 11.4, 6.2)) - 0.5)
            * 0.10 * (1.0 - smoothstep(9.0, 30.0, filtered_footprint));
    }
    return grain;
}

fn globe_orbital_structure(n: vec3<f32>) -> vec2<f32> {
    let footprint = distance(n * (globe.radius_m + GLOBE_SHELL_M), globe.eye_body.xyz) / max(globe.focal_px, 1.0);
    let element_footprint = footprint / globe_cloud_element_scale();
    let authored_detail = clamp(shell_blend.appearance.z, 0.0, 1.0);
    if element_footprint >= 400.0 || authored_detail <= 0.0 { return vec2<f32>(0.0, 0.5); }
    let velocity = globe_weather_wind(n);
    let cycle = shell_blend.wind_m.z / 48.0;
    let a = fract(cycle);
    let b = fract(cycle + 0.5);
    let weight = 1.0 - abs(2.0 * a - 1.0);
    let detail = mix(globe_orbital_grain(n, velocity, b * 48.0, footprint),
        globe_orbital_grain(n, velocity, a * 48.0, footprint), weight);
    let resolved = (1.0 - smoothstep(120.0, 400.0, element_footprint)) * authored_detail;
    let stratiform = clamp(shell_blend.layers.x + shell_blend.layers.z * 0.65, 0.0, 1.0);
    let erosion = clamp(shell_blend.appearance.y, 0.0, 1.0);
    // Sheets retain connected opaque interiors; broken cumulus has stronger
    // margins. This uses existing samples, not another noise or lighting pass.
    let carve = mix(0.22, 0.88, erosion) * mix(1.0, 0.35, stratiform);
    return vec2<f32>((0.5 - detail) * carve * resolved, mix(0.5, detail, resolved));
}

fn globe_clock() -> f32 {
    return globe.sun_dir_body.w;
}

// Body-frame north pole: the home face is +y with north toward chart -z (worldgen geo).
const GLOBE_NORTH: vec3<f32> = vec3<f32>(0.0, 0.0, -1.0);
// Rare flashes inside storm cores; one chance per cell per 4 s slot. A 0.6 s
// slot at 12% strobed the whole night side.
fn globe_lightning(uv: vec2<f32>, face: u32, c: f32) -> f32 {
    let storm = smoothstep(0.78, 0.95, c);
    if storm <= 0.0 {
        return 0.0;
    }
    let g = uv * 64.0;
    let cell = bitcast<vec2<u32>>(vec2<i32>(floor(g)));
    let slot_f = globe_clock() / 4.0 + globe_hash(cell.x, cell.y, face);
    let slot = u32(floor(slot_f));
    // Seconds into the slot, so the flash keeps its ~0.2 s length.
    let phase = fract(slot_f) * 4.0;
    let key = slot * 7u + face;
    if globe_hash(cell.x ^ 0x51u, cell.y, key) > 0.025 * storm {
        return 0.0;
    }
    let centre = vec2<f32>(globe_hash(cell.x, key, 1u), globe_hash(cell.y, key, 2u)) * 0.6 + 0.2;
    let d = fract(g) - centre;
    let flash = exp(-phase * 14.0) + 0.4 * exp(-abs(phase - 0.15) * 40.0);
    return flash * exp(-dot(d, d) * 14.0);
}

// Rare red sprites above the densest storms; much rarer and briefer than lightning.
fn globe_sprite(uv: vec2<f32>, face: u32) -> f32 {
    let g = uv * 12.0;
    let cell = bitcast<vec2<u32>>(vec2<i32>(floor(g)));
    let slot_f = globe_clock() / 10.0 + globe_hash(cell.x, cell.y, face + 40u);
    let slot = u32(floor(slot_f));
    let phase = fract(slot_f);
    if phase > 0.036 {
        return 0.0;
    }
    let key = slot * 7u + face;
    if globe_hash(cell.x ^ 0xa3u, cell.y, key) > 0.015 {
        return 0.0;
    }
    let centre = vec2<f32>(globe_hash(cell.x, key, 3u), globe_hash(cell.y, key, 4u)) * 0.5 + 0.25;
    let core = globe_shell_coverage((floor(g) + centre) / 12.0, face, 0.0);
    let d = (fract(g) - centre) / 0.16;
    let r2 = dot(d, d);
    let tendrils = 0.6 + 0.4 * sin(atan2(d.y, d.x) * 9.0);
    let shape = exp(-r2 * 3.0) + 0.5 * tendrils * exp(-r2 * 0.9);
    return smoothstep(0.85, 0.95, core) * smoothstep(0.0, 0.003, phase) * exp(-phase * 100.0) * shape;
}

// Coverage where the ray from `p` toward the sun crosses the shell.
fn globe_shell_alpha_along(p: vec3<f32>, sun: vec3<f32>) -> f32 {
    let r = globe.radius_m + GLOBE_SHELL_M;
    let b = dot(p, sun);
    let c = dot(p, p) - r * r;
    let disc = b * b - c;
    if disc < 0.0 {
        return 0.0;
    }
    let t = -b + sqrt(disc);
    if t < 0.0 {
        return 0.0;
    }
    return globe_shell_alpha(normalize(p + sun * t));
}

fn globe_city_glow(p: vec3<f32>) -> vec3<f32> {
    var glow = 0.0;
    let count = min(u32(globe.eye_body.w), arrayLength(&night_lights));
    let cell = bitcast<vec3<u32>>(vec3<i32>(floor(p / 180.0)));
    let street = globe_hash(cell.x, cell.y, cell.z);
    for (var group = 0u; group < (count + 15u) / 16u; group++) {
        let low = globe_light_groups.bounds[group * 2u];
        let high = globe_light_groups.bounds[group * 2u + 1u];
        if any(p < low.xyz) || any(p > high.xyz) { continue; }
        let first = u32(low.w);
        for (var i = first; i < min(first + u32(high.w), count); i++) {
            let light = night_lights[i];
            let t = distance(p, light.body) / light.radius_m;
            if t >= 1.0 { continue; }
            let lit = step(0.55 + 0.4 * t, street);
            let edge = 1.0 - t;
            glow += light.intensity * edge * (exp(-t * t * 30.0) * 1.5 + 0.12 * edge + lit * 0.9);
        }
    }
    return GLOBE_CITY * glow * GLOBE_CITY_GAIN;
}

struct GlobeShellOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) dir: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) face: u32,
    @location(3) @interpolate(flat) slot: u32,
    // xy tile chart_min, z tile cell_m, for the land mask under the fragment.
    @location(4) @interpolate(flat) tile: vec3<f32>,
};

// Level-0 root tiles at a constant height; instances follow the opaque tiles.
@vertex
fn vs_shell(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> GlobeShellOut {
    let tile = tiles[instance];
    let x = tile.chart_min.x + f32(vertex % GLOBE_SAMPLES) * tile.cell_m;
    let z = tile.chart_min.y + f32(vertex / GLOBE_SAMPLES) * tile.cell_m;
    let local = globe_fold(tile.face, x, z, GLOBE_SHELL_M) - globe_fold(tile.face, tile.centre.x, tile.centre.y, 0.0);
    var out: GlobeShellOut;
    out.clip = globe.view_proj * vec4<f32>(local + tile.origin_body.xyz, 1.0);
    out.dir = globe_dir(tile.face, x, z);
    out.uv = (vec2<f32>(x, z) + GLOBE_HALF_M) / (2.0 * GLOBE_HALF_M);
    out.face = tile.face;
    out.slot = tile.slot;
    out.tile = vec3<f32>(tile.chart_min, tile.cell_m);
    return out;
}

@fragment
fn fs_shell(in: GlobeShellOut) -> GlobeOutput {
    let altitude = length(globe.eye_body.xyz) - globe.radius_m;
    if (globe.flags & 8u) != 0u || altitude <= GLOBE_HANDOFF_LOW_M || globe_cover(1.0) <= 0.0 {
        return GlobeOutput(vec4<f32>(0.0), vec4<f32>(0.0));
    }
    let eye = globe.eye_body.xyz;
    let radius = globe.radius_m + GLOBE_SHELL_M;
    // Interpolated directions lie on flat triangles. Intersect the actual ray
    // with the smooth sphere before sampling weather to avoid faceted patches.
    let ray = normalize(in.dir * radius - eye);
    let along = dot(eye, ray);
    let discriminant = along * along - dot(eye, eye) + radius * radius;
    let p_shell = eye + ray * (-along - sqrt(max(discriminant, 0.0)));
    let n = normalize(p_shell);
    let sun = normalize(globe.sun_dir_body.xyz);
    let footprint = max(length(dpdx(in.uv)), length(dpdy(in.uv)));
    let lod = max(log2(max(footprint * f32(textureDimensions(shell_coverage).x), 1.0)), 0.0);
    let amount = globe_zonal_amount();
    // Above the tops the view ray walks the cloud-top height field, so towers
    // lean and overlap toward the limb instead of lying painted on a sphere.
    let shell_to_eye = normalize(eye - p_shell);
    let detailed = altitude > GLOBE_SHELL_M + GLOBE_SHELL_TOP_M && dot(shell_to_eye, n) > 0.0;
    var hit = vec4<f32>(n, 0.0);
    let raised_tops = detailed && globe_satellite_blend() < 0.999;
    if raised_tops { hit = globe_shell_parallax(n, shell_to_eye, lod + 0.35, amount); }
    let hn = hit.xyz;
    let chart_frag = in.uv * (2.0 * GLOBE_HALF_M) - vec2<f32>(GLOBE_HALF_M);
    var land = 0.0;
    if detailed { land = globe_shell_land(in.slot, in.tile.xy, in.tile.z, chart_frag); }
    let base_c = globe_zonal_coverage(hn, lod, amount);
    let satellite = globe_satellite_blend();
    var forms = vec3<f32>(0.0);
    if satellite < 0.999 {
        forms = globe_cloud_forms(hn, base_c, land, distance(p_shell, eye) / max(globe.focal_px, 1.0)) * (1.0 - satellite);
    }
    var structure = vec2<f32>(0.0, 0.5);
    if base_c > 0.0 { structure = globe_cloud_structure(hn); }
    // The volumetric deck fades out over this band (cloud_composite.wgsl) while
    // the shell fades in, so there is one deck at every altitude.
    let handoff = smoothstep(GLOBE_HANDOFF_LOW_M, GLOBE_HANDOFF_HIGH_M, altitude);
    let margin = 4.0 * base_c * (1.0 - base_c)
        * (1.0 - smoothstep(0.55, 0.90, base_c));
    let c = select(0.0, clamp(base_c - forms.x * mix(0.35, 0.68, handoff)
        - structure.x * margin * satellite, 0.0, 1.0), base_c > 0.0);
    let p = hn * (globe.radius_m + GLOBE_SHELL_M + hit.w);
    let to_eye = normalize(eye - p);
    // A grazing ray crosses more cloud than a vertical ray. Bound the path
    // length for the finite curved deck instead of forming an opaque rim.
    let path_length = inverseSqrt(max(abs(dot(hn, to_eye)), 0.10));
    let vertical_alpha = min(globe_cover(c), 0.998);
    // Feather the compact world's over-large shell into the atmospheric limb,
    // rather than drawing detached opaque plates against black space. This is
    // an orbital projection treatment; the traversable cloud deck is unchanged.
    let impact = length(cross(eye, ray));
    let limb_width = max(globe.radius_m * 0.006,
        2.0 * distance(p_shell, eye) / max(globe.focal_px, 1.0));
    let limb = 1.0 - smoothstep(globe.radius_m, globe.radius_m + limb_width, impact);
    let alpha = (1.0 - pow(1.0 - vertical_alpha, path_length * handoff))
        * mix(1.0, limb, globe_satellite_blend());
    let sun_angle = dot(hn, sun);
    let day = smoothstep(-0.12, 0.08, sun_angle);
    let night = 1.0 - smoothstep(-0.1, 0.02, sun_angle);
    // Sprites may cover clear sky. Only evaluate the storm lookup at night.
    var sprite = 0.0;
    if night > 0.0 { sprite = globe_sprite(in.uv, in.face) * night * handoff; }
    let cover = max(alpha, min(sprite, 1.0));
    let sprite_rgb = vec3<f32>(1.0, 0.15, 0.22) * 0.8 * sprite;
    if alpha <= 0.0 {
        return GlobeOutput(vec4<f32>(sprite_rgb / max(cover, 1e-4), cover), vec4<f32>(0.0));
    }
    // No probe is bound to the globe; GLOBE_SKY stands in for the probe zenith.
    // Water-droplet albedo and uneven optical depth keep sunlit banks below
    // flat white, with cool creases between their illuminated crowns.
    let relief = smoothstep(0.20, 0.80, structure.y + forms.y);
    // Coverage stands in for thickness: thin margins are grey and translucent,
    // dense cores brighter, so the deck has volume instead of one flat white.
    let thick = smoothstep(0.25, 0.92, c);
    var shadow = 1.0;
    var sunward = 0.0;
    if raised_tops && sun_angle > 0.0 {
        let top = globe_cloud_top_m(c) + forms.z * globe_shell_relief_m();
        shadow = globe_shell_self_shadow(hn, top, sun, lod, amount);
    } else if detailed && sun_angle > 0.0 {
        // Satellite view: the deck is too thin to march, but its thickness
        // still shades it. One tap a kilometre toward the sun: thicker cloud
        // there puts this point in its shadow, thinner cloud leaves a lit
        // edge. Shadows lengthen toward the terminator.
        let sun_t = sun - hn * sun_angle;
        let reach = mix(2200.0, 700.0, smoothstep(0.05, 0.7, sun_angle)) / (globe.radius_m + GLOBE_SHELL_M);
        let toward = normalize(hn + sun_t / max(length(sun_t), 1e-3) * reach);
        let rise = globe_coverage_fast(toward, lod + 0.5, amount) - base_c;
        shadow = exp(-max(rise, 0.0) * mix(7.0, 3.0, smoothstep(0.05, 0.7, sun_angle)));
        sunward = clamp(-rise * 2.5, 0.0, 1.0);
    }
    var sun_rgb = vec3<f32>(0.0);
    if sun_angle > 0.0 { sun_rgb = GLOBE_SHELL_SUN * globe_sun_through(p, sun) * shadow; }
    let direct = sun_rgb * max(sun_angle, 0.0);
    // Thick cores scatter more of their light back out; thin margins seen
    // toward the sun glow with forward scattering.
    let powder = 1.0 - exp(-3.0 * thick);
    let rim = globe_shell_hg(dot(-sun, to_eye), 0.55) * (1.0 - thick) * 0.8;
    let lit = GLOBE_CLOUD_ALBEDO * (direct * mix(0.64, 1.0, relief) * (0.80 + 0.20 * powder) * (1.0 + 0.16 * sunward)
        + sun_rgb * rim + GLOBE_SKY * 0.19 * (0.75 + 0.25 * thick));
    // Moonlit tops are brighter than the ground below them, as the march draws
    // them; with the moon down only a faint sky floor remains.
    let moonlit = GLOBE_CLOUD_ALBEDO * (GLOBE_MOON * 0.15 + globe_moonlight(n));
    // Thin margins lose brightness to the ground showing through, which the
    // alpha already draws; dimming their colour as well left grey smears on
    // a dark sea where sunlit cloud is the brightest thing in the frame.
    var color = mix(moonlit, lit * (0.88 + 0.12 * thick), day);
    if night > 0.0 {
        color += vec3<f32>(0.75, 0.82, 1.0) * 0.6 * night * globe_lightning(in.uv, in.face, c);
    }
    let rel = p - globe.eye_body.xyz;
    let jitter = fract(dot(floor(in.clip.xy), vec2<f32>(0.754877666, 0.569840296)));
    var air: PlanetAir;
    if satellite >= 0.999 && (globe.flags & 32u) == 0u {
        air = globe_orbital_cloud_air(p, to_eye, sun);
    } else { air = globe_air(rel, jitter); }
    color = color * air.transmittance + air.inscatter + air.airglow;
    let premul = color * alpha + sprite_rgb;
    return GlobeOutput(vec4<f32>(premul / max(cover, 1e-4), cover), vec4<f32>(0.0));
}
