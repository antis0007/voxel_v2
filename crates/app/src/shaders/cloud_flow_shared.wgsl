// All cloud renderers interpret the authored size dial as the same metre ratio.
// Keep its existing surface direction so saved weather does not change size.
fn cloud_size_ratio(noise_scale: f32) -> f32 {
    return mix(1120.0, 640.0, clamp(noise_scale, 0.0, 1.0)) / 880.0;
}

// Shared cloud dynamics; explicit inputs keep surface and orbit on one field.
fn flow_hash_cell1(c: vec2<i32>, salt: i32) -> f32 {
    var h = bitcast<u32>(c.x * 1619 + c.y * 31337 + salt * 6971 + 1013904223);
    h ^= h >> 16u;
    h  = h * 0x45d9f3bu;
    h ^= h >> 16u;
    h  = h * 0xac4d1c7fu;
    h ^= h >> 16u;
    return f32(h) / 4294967296.0;
}

fn flow_hash_cell2(c: vec2<i32>, salt: i32) -> vec2<f32> {
    return vec2<f32>(flow_hash_cell1(c, salt), flow_hash_cell1(c, salt + 7919));
}

fn flow_rotate2(v: vec2<f32>, angle: f32) -> vec2<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return vec2<f32>(c * v.x - s * v.y, s * v.x + c * v.y);
}

// Continuous kilometer-scale organization shared by cloud banks and rain.
// Hash noise avoids a repeating volume-texture period across the horizon.
fn weather_bank_noise(p: vec2<f32>) -> f32 {
    let cell = vec2<i32>(floor(p));
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    return mix(mix(flow_hash_cell1(cell, 4127), flow_hash_cell1(cell + vec2<i32>(1, 0), 4127), u.x),
               mix(flow_hash_cell1(cell + vec2<i32>(0, 1), 4127),
                   flow_hash_cell1(cell + vec2<i32>(1, 1), 4127), u.x), u.y);
}

// Cheap flow dynamics applied to the cloud texture coordinates. Weather cells
// keep the plain advection offset (the CPU shell mirrors it); only the density
// samplers see this warp, so it can bend shapes without moving coverage.
const FLOW_CELL_M: f32 = 9000.0;
const FLOW_SPACING_M: f32 = 1500.0;
const FLOW_CORE_M: f32 = 430.0;
const FLOW_ROW_M: f32 = 380.0;
const FLOW_MAX_TURN: f32 = 2.4;
const FLOW_STREET_VORTICES: f32 = 6.0;

// Karman street shed by sparse hashed obstacles. Vortices leave each obstacle
// at 0.85 of the layer speed, alternate rows and turn the sampled texture
// about their core with a Gaussian angle that fades in and decays downstream.
fn cloud_vortex_street(xz: vec2<f32>, wind: vec2<f32>, drift: f32, time: f32) -> vec2<f32> {
    let cell0 = vec2<i32>(floor(xz / FLOW_CELL_M));
    var d = vec2<f32>(0.0);
    for (var z = -1; z <= 1; z += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let id = cell0 + vec2<i32>(x, z);
            if flow_hash_cell1(id, 7411) > 0.30 { continue; }
            d += cloud_vortex_wake(xz, id, wind, drift, time);
        }
    }
    return d;
}

// Obstacle candidate of lattice cell `id`; render/globe/shell_obstacles.rs mirrors it.
fn cloud_vortex_site(id: vec2<i32>) -> vec2<f32> {
    return (vec2<f32>(id) + flow_hash_cell2(id, 7412)) * FLOW_CELL_M;
}

// Displacement at `xz` from the wake of the obstacle in cell `id`.
fn cloud_vortex_wake(xz: vec2<f32>, id: vec2<i32>, wind: vec2<f32>, drift: f32, time: f32) -> vec2<f32> {
    let side = vec2<f32>(-wind.y, wind.x);
    let travelled = time * drift / FLOW_SPACING_M;
    let whole = floor(travelled);
    let phase = travelled - whole;
    let rel = xz - cloud_vortex_site(id);
    let s = dot(rel, wind);
    let q = dot(rel, side);
    if s < -FLOW_CORE_M || s > (FLOW_STREET_VORTICES + 1.0) * FLOW_SPACING_M
        || abs(q) > FLOW_ROW_M + 2.4 * FLOW_CORE_M { return vec2<f32>(0.0); }
    var d = vec2<f32>(0.0);
    let n0 = floor(s / FLOW_SPACING_M - phase);
    for (var k = 0; k < 2; k += 1) {
        let n = n0 + f32(k);
        let age = n + phase;
        if age < 0.0 || age > FLOW_STREET_VORTICES { continue; }
        let sgn = select(-1.0, 1.0, ((i32(n) + i32(whole)) & 1) == 0);
        let r = vec2<f32>(s - age * FLOW_SPACING_M, q - sgn * FLOW_ROW_M);
        let strength = smoothstep(0.0, 0.9, age) * exp(-age * 0.33);
        let turn = -sgn * FLOW_MAX_TURN * strength * exp(-dot(r, r) / (FLOW_CORE_M * FLOW_CORE_M));
        let moved = flow_rotate2(r, turn) - r;
        d += moved.x * wind + moved.y * side;
    }
    return d;
}

// Horizontal warp (xz) and vertical shift (y) for one density tap.
// Streets need moderate wind and fade out in a calm or a gale.
fn cloud_flow_warp_at(xz: vec2<f32>, h: f32, layer_id: i32, wind: vec2<f32>, wspd: f32, time: f32, shear: f32, speed: f32) -> vec3<f32> {
    let street_weight = cloud_street_weight(layer_id, wspd);
    var warp = vec2<f32>(0.0);
    if street_weight > 0.01 {
        warp += cloud_vortex_street(xz, wind, speed * 0.85, time) * street_weight;
    }
    return cloud_flow_drift_at(xz, h, layer_id, wind, wspd, time, shear) + vec3<f32>(warp, 0.0);
}

fn cloud_street_weight(layer_id: i32, wspd: f32) -> f32 {
    return select(select(1.0, 0.45, layer_id == 1), 0.0, layer_id == 2)
        * smoothstep(0.05, 0.30, wspd) * (1.0 - smoothstep(1.6, 3.0, wspd));
}

// Gusts and billows without the vortex streets.
fn cloud_flow_drift_at(xz: vec2<f32>, h: f32, layer_id: i32, wind: vec2<f32>, wspd: f32, time: f32, shear: f32) -> vec3<f32> {
    var warp = vec2<f32>(0.0);
    // Gust fronts: a slowly evolving divergence-free-looking wobble. Jet
    // meanders on the high deck are broader and larger.
    let jet = layer_id == 2;
    let scale = select(2600.0, 7000.0, jet);
    let phase = vec2<f32>(time * wspd * 0.0006, -time * wspd * 0.0004);
    let gust = vec2<f32>(
        weather_bank_noise(xz / scale + phase + vec2<f32>(5.1, 17.9)),
        weather_bank_noise(xz / scale - phase + vec2<f32>(-23.4, 9.6))) - vec2<f32>(0.5);
    warp += gust * select(select(150.0, 110.0, layer_id == 1), 520.0, jet);
    // Kelvin-Helmholtz billows ride the sheared top of the low deck.
    var lift = 0.0;
    if layer_id == 0 {
        let along = dot(xz, wind);
        let wave = sin(along * 0.0085 - time * wspd * 0.012 + dot(xz, vec2<f32>(-wind.y, wind.x)) * 0.0012);
        lift = wave * smoothstep(0.55, 1.0, h) * (0.15 + shear) * 22.0;
    }
    return vec3<f32>(warp, lift);
}

// Cloud streets: wind-aligned rolls that thin the low deck into rows.
fn cloud_street_rolls_at(xz: vec2<f32>, wind: vec2<f32>, convection: f32, wspd: f32) -> f32 {
    let region = smoothstep(0.50, 0.68, weather_bank_noise(xz / 13000.0 + vec2<f32>(-9.4, 21.6)));
    let amount = region * smoothstep(0.10, 0.35, wspd) * smoothstep(0.10, 0.40, convection)
        * (1.0 - smoothstep(0.60, 0.90, convection));
    if amount <= 0.01 { return 1.0; }
    let perp = dot(xz, vec2<f32>(-wind.y, wind.x));
    let wobble = weather_bank_noise(vec2<f32>(dot(xz, wind) / 4200.0, perp / 9000.0)) * 2.4;
    let rows = 0.5 + 0.5 * sin(perp * 6.2831853 / 2300.0 + wobble);
    return mix(1.0, 0.30 + 0.95 * smoothstep(0.25, 0.80, rows), amount * 0.75);
}

