// Title vista: a hills province at dusk. A river winds through a floodplain of
// fields toward a walled hill town; hamlets, forest and banded valley walls
// fill the slopes and far ranges close the horizon. It stands for the regional
// world milestone (city, villages, routes) rather than a generated preview: the
// title must draw instantly without running the generator.
// One voxel column per cell, traced with a 2D grid walk at 640x360.
// clock = (seconds, aspect, quiet band right edge in uv or -1 for even shade, dim 0..1).
// look = (mood 0 dusk 1 night 2 dawn 3 day, pose 0 province 1 riverside 2 high, target width, target height).
struct Scene { clock: vec4<f32>, look: vec4<f32> }
@group(0) @binding(0) var<uniform> scene: Scene;
struct Vertex { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex fn vs_main(@builtin(vertex_index) index: u32) -> Vertex {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return Vertex(vec4<f32>(uv * vec2<f32>(2.0,-2.0) + vec2<f32>(-1.0,1.0),0.0,1.0),uv);
}

const WATER: f32 = 6.0;
const TOWN_Z: f32 = 132.0;
const PLATEAU: f32 = 36.0;
// Kinds: 0 land, 1 water, 2 house, 3 wall or tower, 4 keep, 5 tree, 6 street, 7 field, 8 bridge.
struct Col { top: f32, base: f32, kind: u32, tint: f32 }
struct Hit { t: f32, n: vec3<f32>, cell: vec2<i32>, col: Col, ok: bool }

// One province, four hours. `key` lights the land and aims the sky glow;
// `sun` sets the giant's phase (below the horizon at night, when the giant
// itself is the key light). The banded giant stands over the far ranges.
struct Mood {
    key: vec3<f32>, key_col: vec3<f32>, sun: vec3<f32>,
    amb_up: vec3<f32>, amb_dn: vec3<f32>, fog_a: vec3<f32>, fog_s: vec3<f32>,
    hor_a: vec3<f32>, hor_s: vec3<f32>, mid: vec3<f32>, zen: vec3<f32>,
    cloud_a: vec3<f32>, cloud_s: vec3<f32>,
    stars: f32, giant: f32, veil: f32, disc: f32, mist: f32, windows: f32, birds: f32, exposure: f32,
}
var<private> M: Mood;
fn mood(i: u32) -> Mood {
    switch i {
        case 1u: {
            let g = giant_dir();
            return Mood(g, vec3<f32>(0.46, 0.58, 0.92), normalize(vec3<f32>(-0.35, -0.30, -0.89)),
                vec3<f32>(0.055, 0.085, 0.18), vec3<f32>(0.022, 0.026, 0.045),
                vec3<f32>(0.035, 0.05, 0.10), vec3<f32>(0.09, 0.13, 0.24),
                vec3<f32>(0.05, 0.07, 0.15), vec3<f32>(0.11, 0.16, 0.30),
                vec3<f32>(0.025, 0.04, 0.10), vec3<f32>(0.008, 0.012, 0.04),
                vec3<f32>(0.05, 0.06, 0.11), vec3<f32>(0.24, 0.30, 0.46),
                2.2, 0.62, 0.7, 0.0, 0.8, 2.2, 0.0, 1.3);
        }
        case 2u: {
            let d = normalize(vec3<f32>(-0.78, 0.20, 0.59));
            return Mood(d, vec3<f32>(2.1, 1.25, 0.95), d,
                vec3<f32>(0.17, 0.20, 0.32), vec3<f32>(0.09, 0.075, 0.08),
                vec3<f32>(0.30, 0.27, 0.38), vec3<f32>(0.74, 0.50, 0.46),
                vec3<f32>(0.55, 0.38, 0.46), vec3<f32>(1.25, 0.70, 0.50),
                vec3<f32>(0.22, 0.30, 0.48), vec3<f32>(0.06, 0.10, 0.24),
                vec3<f32>(0.30, 0.22, 0.30), vec3<f32>(1.3, 0.75, 0.62),
                0.35, 0.9, 0.3, 1.0, 1.5, 0.5, 1.0, 1.0);
        }
        case 3u: {
            let d = normalize(vec3<f32>(0.50, 0.74, -0.45));
            return Mood(d, vec3<f32>(1.9, 1.75, 1.5), d,
                vec3<f32>(0.28, 0.38, 0.60), vec3<f32>(0.16, 0.15, 0.13),
                vec3<f32>(0.50, 0.62, 0.82), vec3<f32>(0.70, 0.76, 0.86),
                vec3<f32>(0.62, 0.74, 0.92), vec3<f32>(0.80, 0.86, 0.95),
                vec3<f32>(0.26, 0.46, 0.82), vec3<f32>(0.08, 0.22, 0.58),
                vec3<f32>(0.72, 0.76, 0.84), vec3<f32>(1.15, 1.12, 1.08),
                0.0, 0.4, 0.0, 1.0, 0.45, 0.0, 1.0, 0.9);
        }
        default: {
            let d = normalize(vec3<f32>(0.86, 0.26, 0.44));
            return Mood(d, vec3<f32>(2.5, 1.4, 0.72), d,
                vec3<f32>(0.15, 0.20, 0.36), vec3<f32>(0.10, 0.075, 0.065),
                vec3<f32>(0.19, 0.18, 0.29), vec3<f32>(0.92, 0.50, 0.25),
                vec3<f32>(0.46, 0.25, 0.30), vec3<f32>(1.30, 0.62, 0.25),
                vec3<f32>(0.13, 0.20, 0.36), vec3<f32>(0.02, 0.04, 0.13),
                vec3<f32>(0.20, 0.13, 0.17), vec3<f32>(1.35, 0.66, 0.34),
                1.0, 1.0, 0.5, 1.0, 1.0, 1.0, 1.0, 1.05);
        }
    }
}
fn sun_dir() -> vec3<f32> { return M.key; }
fn giant_dir() -> vec3<f32> { return normalize(vec3<f32>(0.448, 0.150, 0.880)); }
const GIANT_R: f32 = 0.07;
const LIFT_AZ: f32 = 0.16;
const LIFT_HEAD: f32 = 0.16;
fn sky_dir(az: f32, y: f32) -> vec3<f32> {
    let c = sqrt(1.0 - y * y);
    return vec3<f32>(sin(az) * c, y, cos(az) * c);
}
// The ring's plane passes through the lift head and meets the horizon to its left.
fn ring_normal() -> vec3<f32> {
    return normalize(cross(sky_dir(LIFT_AZ, LIFT_HEAD), sky_dir(-0.25, 0.0)));
}

fn mix_u(x: u32) -> u32 {
    var h = x;
    h = h ^ (h >> 16u);
    h = h * 0x7feb352du;
    h = h ^ (h >> 15u);
    h = h * 0x846ca68bu;
    return h ^ (h >> 16u);
}
fn hash2(c: vec2<i32>) -> f32 {
    let h = mix_u((bitcast<u32>(c.x) * 0x9e3779b9u) ^ mix_u(bitcast<u32>(c.y) + 0x632be5abu));
    return f32(h >> 8u) * (1.0 / 16777216.0);
}
fn hash3(c: vec3<i32>) -> f32 { return hash2(vec2<i32>(c.x + c.y * 7919, c.z - c.y * 104729)); }
fn vnoise(p: vec2<f32>) -> f32 {
    let i = vec2<i32>(floor(p));
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash2(i);
    let b = hash2(i + vec2<i32>(1, 0));
    let c = hash2(i + vec2<i32>(0, 1));
    let d = hash2(i + vec2<i32>(1, 1));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}
fn fbm(p: vec2<f32>) -> f32 {
    var s = 0.0;
    var a = 0.5;
    var q = p;
    for (var k = 0; k < 4; k++) {
        s += a * vnoise(q);
        q = vec2<f32>(q.x * 1.9 - q.y * 0.6, q.x * 0.6 + q.y * 1.9) + vec2<f32>(17.0, 9.0);
        a *= 0.5;
    }
    return s / 0.9375;
}

fn river_x(z: f32) -> f32 { return 26.0 * sin(z * 0.017 + 0.4) + 9.0 * sin(z * 0.046 + 2.1); }
fn town() -> vec2<f32> { return vec2<f32>(river_x(TOWN_Z) + 44.0, TOWN_Z); }

fn terrain(p: vec2<f32>) -> f32 {
    let d = abs(p.x - river_x(p.y));
    let hills = 12.0 + 30.0 * fbm(p * 0.011) + 10.0 * fbm(p * 0.031 + 5.0);
    let v = smoothstep(3.0, 62.0, d);
    var h = mix(WATER - 2.5, hills, pow(v, 0.9));
    let r = length(p - town());
    if r < 27.0 { h = PLATEAU; } else { h = max(h, PLATEAU - (r - 27.0) * 0.9); }
    // The river cuts through everything, mound included: a channel, then a
    // low bench beside it that the fields occupy, then the valley walls.
    h = min(h, WATER - 2.5 + max(d - 4.0, 0.0) * 1.6);
    if d > 4.5 { h = max(h, min(WATER + 1.0 + 1.6 * fbm(p * 0.05), WATER - 2.5 + (d - 4.0) * 1.6)); }
    return h;
}

// One gabled house on a 5-cell lot with a street on two sides; top < 0 = none.
fn house(l: vec2<i32>, lot: vec2<i32>, ground: f32, salt: i32) -> vec3<f32> {
    if l.x > 3 || l.y > 3 || hash2(lot + vec2<i32>(91, salt)) > 0.82 { return vec3<f32>(-1.0); }
    let storeys = 3.0 + floor(hash2(lot + vec2<i32>(3, 40 + salt)) * 3.0);
    let k = select(l.y, l.x, hash2(lot + vec2<i32>(salt, 7)) < 0.5);
    let peak = select(0.0, 1.0, k == 1 || k == 2);
    return vec3<f32>(ground + storeys + 1.0 + peak, ground + storeys, hash2(lot + vec2<i32>(11, salt)));
}

fn seg(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ab = b - a;
    return length(p - a - ab * clamp(dot(p - a, ab) / dot(ab, ab), 0.0, 1.0));
}

fn column(c: vec2<i32>) -> Col {
    let p = vec2<f32>(c) + 0.5;
    let g = floor(terrain(p));
    var col = Col(g, g, 0u, 0.0);
    let t = p - town();
    let r = length(t);
    if r < 28.5 {
        // Eight round towers on the curtain; the southern gap is the gate.
        let k = round(atan2(t.y, t.x) / 0.7853982);
        let tc = 25.0 * vec2<f32>(cos(k * 0.7853982), sin(k * 0.7853982));
        if k != -2.0 && length(t - tc) < 2.7 {
            return Col(g + 10.0 + f32((c.x + c.y) & 1), g, 3u, 0.0);
        }
    }
    if r < 27.0 {
        col.kind = 6u;
        let a = vec2<i32>(floor(t));
        let m = max(abs(a.x), abs(a.y));
        if r >= 23.6 && r < 25.6 {
            if !(t.y < 0.0 && abs(t.x) < 2.6) {
                col.top = g + 6.0 + f32((c.x + c.y) & 1);
                col.kind = 3u;
            }
            return col;
        }
        if m <= 4 {
            let crenel = select(0.0, 1.0, m == 4 && ((a.x + a.y) & 1) == 0);
            return Col(g + 20.0 + crenel, g + 20.0, 4u, 0.0);
        }
        if r < 22.0 && m > 6 {
            let lot = vec2<i32>(floor(t / 5.0));
            let h = house(a - lot * 5, lot, g, 0);
            if h.x > 0.0 { return Col(h.x, h.y, 2u, h.z); }
        }
        return col;
    }
    // Hamlets on the floodplain rim; each house stands on its lot's ground.
    var hamlets = array<vec2<f32>, 2>(
        vec2<f32>(river_x(86.0) + 24.0, 86.0),
        vec2<f32>(river_x(214.0) - 22.0, 214.0));
    for (var i = 0; i < 2; i++) {
        let q = p - hamlets[i];
        if length(q) < 11.0 {
            let a = vec2<i32>(floor(q));
            let lot = vec2<i32>(floor(q / 5.0));
            let gc = floor(terrain(hamlets[i] + (vec2<f32>(lot) + 0.4) * 5.0));
            let h = house(a - lot * 5, lot, max(gc, g), 13 + i * 29);
            if h.x > g { return Col(h.x, h.y, 2u, h.z); }
            if length(q) < 8.0 { col.kind = 6u; }
            return col;
        }
    }
    let d = abs(p.x - river_x(p.y));
    // Stone bridge below the first hamlet; the road runs from the town gate
    // through the hamlet, over the bridge and off into the western woods.
    if abs(p.y - 86.0) < 1.6 && d < 9.5 { return Col(WATER + 3.0, WATER + 3.0, 8u, 0.0); }
    if g < WATER { return Col(WATER - 0.15, WATER - 0.15, 1u, 0.0); }
    let bridge = vec2<f32>(river_x(86.0), 86.0);
    if min(min(seg(p, town() + vec2<f32>(0.0, -27.0), hamlets[0]),
        seg(p, hamlets[0], bridge + vec2<f32>(9.0, 0.0))),
        seg(p, bridge - vec2<f32>(9.0, 0.0), vec2<f32>(-70.0, 30.0))) < 1.3 {
        col.kind = 6u;
        return col;
    }
    // Fields: furrowed patches on the bench, one crop per plot.
    let plot = vec2<i32>(floor(p / vec2<f32>(13.0, 9.0)));
    if d > 6.0 && d < 48.0 && g <= WATER + 3.0 && hash2(plot + vec2<i32>(5, 77)) < 0.78 {
        return Col(g, g, 7u, hash2(plot));
    }
    // Woods: crowns on a jittered 5-cell lattice wherever the forest mask is dense.
    if d > 9.0 && r > 31.0 {
        let density = smoothstep(0.40, 0.62, fbm(p * 0.018 + vec2<f32>(40.0, -12.0)));
        let cell = vec2<i32>(floor(p / 5.0));
        let jitter = vec2<f32>(hash2(cell + vec2<i32>(1, 9)), hash2(cell + vec2<i32>(8, 2)));
        let centre = (vec2<f32>(cell) + 0.25 + 0.5 * jitter) * 5.0;
        let dd = length(p - centre);
        if dd < 2.4 && hash2(cell + vec2<i32>(5, 5)) < density {
            let gc = floor(terrain(centre));
            let crown = gc + 5.0 + floor(hash2(cell) * 3.0) - floor(dd * 1.3);
            if crown > g { return Col(crown, gc + 2.0, 5u, hash2(cell + vec2<i32>(2, 2))); }
        }
    }
    col.tint = fbm(p * 0.04 + vec2<f32>(3.0, 1.0));
    return col;
}

fn trace(ro: vec3<f32>, rd: vec3<f32>, steps: i32, tmax: f32) -> Hit {
    var cell = vec2<i32>(floor(ro.xz));
    let dir = rd.xz;
    let stride = vec2<i32>(select(vec2<i32>(-1), vec2<i32>(1), dir > vec2<f32>(0.0)));
    let inv = 1.0 / max(abs(dir), vec2<f32>(1e-6));
    let f = fract(ro.xz);
    var side = select(f, 1.0 - f, dir > vec2<f32>(0.0)) * inv;
    var t0 = 0.0;
    var n = vec3<f32>(0.0, 1.0, 0.0);
    for (var i = 0; i < steps; i++) {
        let col = column(cell);
        let t1 = min(side.x, side.y);
        if i > 0 && ro.y + rd.y * t0 <= col.top { return Hit(t0, n, cell, col, true); }
        if rd.y < 0.0 && ro.y + rd.y * t1 <= col.top {
            return Hit(max((col.top - ro.y) / rd.y, t0), vec3<f32>(0.0, 1.0, 0.0), cell, col, true);
        }
        if t1 > tmax || (rd.y >= 0.0 && ro.y + rd.y * t1 > 72.0) { break; }
        if side.x < side.y {
            t0 = side.x; side.x += inv.x; cell.x += stride.x;
            n = vec3<f32>(-f32(stride.x), 0.0, 0.0);
        } else {
            t0 = side.y; side.y += inv.y; cell.y += stride.y;
            n = vec3<f32>(0.0, 0.0, -f32(stride.y));
        }
    }
    return Hit(t0, n, cell, Col(0.0, 0.0, 0u, 0.0), false);
}

fn sky(rd: vec3<f32>, stars: bool) -> vec3<f32> {
    let s = sun_dir();
    let sd = max(dot(rd, s), 0.0);
    let up = max(rd.y, 0.0);
    // Three bands: ember at the ridge, dusk blue above it, indigo overhead.
    let horizon = mix(M.hor_a, M.hor_s, pow(sd, 3.0));
    var color = mix(horizon, M.mid, smoothstep(0.0, 0.17, up));
    color = mix(color, M.zen, smoothstep(0.12, 0.6, up));
    color += M.key_col * 0.2 * M.disc * pow(sd, 40.0);
    color += M.key_col * 3.6 * M.disc * smoothstep(0.99986, 0.99993, sd);
    if stars && rd.y > 0.06 {
        let sp = vec2<i32>(floor(vec2<f32>(atan2(rd.x, rd.z), rd.y) * 260.0));
        let h = hash2(sp);
        color += vec3<f32>(0.75, 0.8, 1.0) * step(0.9965, h) * (h - 0.9965) * 140.0 * smoothstep(0.05, 0.2, rd.y) * M.stars;
        // Night only: one meteor every nine seconds, drawn in azimuth and height.
        let phase = fract(scene.clock.x / 9.0);
        if M.stars > 1.5 && phase > 0.005 && phase < 0.07 {
            let k = vec2<i32>(i32(floor(scene.clock.x / 9.0)), 3);
            let head = vec2<f32>(-0.1 + 0.7 * hash2(k), 0.2 + 0.1 * hash2(k + vec2<i32>(5, 1)))
                + vec2<f32>(1.0, -0.4) * phase * 1.6;
            let d = seg(vec2<f32>(atan2(rd.x, rd.z), rd.y), head - vec2<f32>(0.05, -0.02), head);
            color += vec3<f32>(0.9, 0.95, 1.0) * smoothstep(0.0022, 0.0006, d) * (1.0 - phase / 0.07);
        }
    }
    // Giant: a sunlit crescent with cloud bands, the night side a shade under
    // the sky. No ring: the only ring in this sky is the built one.
    let g = giant_dir();
    let gd = dot(rd, g);
    if gd > 0.975 {
        let q = (rd - g * gd) / GIANT_R;
        let r2 = dot(q, q);
        let ax = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), g));
        let ay = cross(g, ax);
        let u = dot(q, ax * 0.94 + ay * 0.34);
        let v = dot(q, ay * 0.94 - ax * 0.34);
        if r2 < 1.0 {
            let normal = q - g * sqrt(1.0 - r2);
            let lit = smoothstep(-0.05, 0.3, dot(normal, M.sun));
            let bands = 0.5 + 0.5 * sin(v * 11.0 + 2.5 * fbm(vec2<f32>(u * 2.0, v * 6.0)));
            let tint = mix(vec3<f32>(0.98, 0.80, 0.60), vec3<f32>(0.70, 0.52, 0.50), bands)
                * (0.45 + 0.55 * sqrt(1.0 - r2));
            let disc = color * (1.0 - 0.5 * M.veil) + tint * (0.05 * M.veil + 1.25 * lit) * M.giant;
            color = mix(color, disc, smoothstep(1.0, 0.93, r2));
        }
    }
    // Orbital ring: one sunlit thread on a great circle through the lift head.
    let rn = ring_normal();
    let off = abs(dot(rd, rn));
    if off < 0.003 && rd.y > 0.0 {
        let thread = smoothstep(0.0024, 0.0008, off) * smoothstep(0.02, 0.12, rd.y);
        let spans = 0.5 + 0.5 * step(0.14, fract(atan2(rd.x, rd.z) * 46.0));
        color += M.cloud_s * 0.30 * thread * spans;
    }
    // Banded high cloud catching the last light from below.
    if rd.y > 0.015 {
        let uv = rd.xz / (rd.y + 0.06) * 0.5 + vec2<f32>(scene.clock.x * 0.004, 0.0);
        let cover = smoothstep(0.52, 0.82, fbm(uv * vec2<f32>(0.9, 2.6))) * smoothstep(0.015, 0.11, rd.y);
        let lit = mix(M.cloud_a, M.cloud_s, pow(sd, 3.0) * 0.8 + 0.15);
        color = mix(color, lit, cover * 0.8);
    }
    return color;
}

// Far ranges past the traced grid: two silhouettes by azimuth, hazier behind.
fn backdrop(rd: vec3<f32>) -> vec3<f32> {
    let az = atan2(rd.x, rd.z);
    let near = 0.004 + 0.045 * fbm(vec2<f32>(az * 7.0 + 3.0, 0.5));
    let far = 0.02 + 0.07 * pow(fbm(vec2<f32>(az * 4.0, 2.5)), 1.4);
    let haze = fog(rd);
    if rd.y < near { return haze * vec3<f32>(0.62, 0.62, 0.74); }
    if rd.y < far {
        let snow = smoothstep(far - 0.018, far - 0.006, rd.y) * smoothstep(0.06, 0.08, far);
        return mix(haze * vec3<f32>(0.8, 0.78, 0.88), haze * 1.25, snow);
    }
    var color = sky(rd, true);
    // Lift tether: one pixel of sunlit cable from behind the ranges out of
    // frame, with cars climbing it.
    let da = abs(az - LIFT_AZ);
    if da < 0.0024 && rd.y < LIFT_HEAD {
        let line = smoothstep(0.0024, 0.0008, da);
        color += M.cloud_s * 0.5 * line * (0.10 + 0.55 * smoothstep(0.03, 0.22, rd.y));
        color += vec3<f32>(1.8, 1.1, 0.5) * (0.35 + 0.65 * min(M.windows, 1.0)) * line
            * step(0.955, fract(rd.y * 34.0 - scene.clock.x * 0.04));
    }
    return color;
}

// Valley haze: violet away from the sun, amber toward it; no disc or cloud.
fn fog(rd: vec3<f32>) -> vec3<f32> {
    let sd = max(dot(normalize(vec3<f32>(rd.x, 0.0, rd.z)), normalize(vec3<f32>(sun_dir().x, 0.0, sun_dir().z))), 0.0);
    return mix(M.fog_a, M.fog_s, pow(sd, 3.0));
}
// Thin air on the hills, mist pooled on the valley floor.
fn haze(t: f32, y: f32) -> f32 {
    return 1.0 - exp(-t * (0.0011 + 0.0030 * M.mist * exp(-max(y - WATER, 0.0) / 14.0)));
}

struct Surface { albedo: vec3<f32>, emission: vec3<f32> }

fn surface(h: Hit, pos: vec3<f32>) -> Surface {
    let top = h.n.y > 0.5;
    let vy = select(i32(floor(pos.y - 0.001)), i32(h.col.top), top);
    let grain = 0.88 + 0.24 * hash3(vec3<i32>(h.cell.x, vy, h.cell.y));
    let cap = top || pos.y > h.col.base + 0.001;
    let face = select(vec2<f32>(fract(pos.x), fract(pos.y)), vec2<f32>(fract(pos.z), fract(pos.y)), abs(h.n.x) > 0.5);
    var a = vec3<f32>(0.5);
    var e = vec3<f32>(0.0);
    switch h.col.kind {
        case 2u: {
            if cap {
                a = select(vec3<f32>(0.52, 0.22, 0.15), vec3<f32>(0.28, 0.30, 0.34), h.col.tint < 0.35);
            } else {
                a = select(vec3<f32>(0.80, 0.73, 0.60), vec3<f32>(0.44, 0.30, 0.20), i32(h.col.base) - 1 == vy);
                let lit = hash3(vec3<i32>(h.cell.x, vy, h.cell.y) + vec3<i32>(7, 0, 3)) < 0.32 * M.windows;
                let pane = all(abs(face - 0.5) < vec2<f32>(0.26, 0.3));
                if !top && lit && pane && i32(h.col.base) - 1 != vy {
                    let flicker = 0.85 + 0.15 * sin(scene.clock.x * 2.7 + f32(h.cell.x * 13 + vy * 7));
                    e = vec3<f32>(2.4, 1.25, 0.5) * flicker;
                }
            }
        }
        case 3u: {
            a = vec3<f32>(0.56, 0.54, 0.50);
            // Arrow slits: a few lit on the towers and wall walk after dark.
            let slit = all(abs(face - 0.5) < vec2<f32>(0.14, 0.34));
            if !top && slit && vy % 4 == 1 && hash3(vec3<i32>(h.cell.x, vy, h.cell.y)) < 0.10 * M.windows {
                e = vec3<f32>(2.0, 1.0, 0.4);
            }
        }
        case 8u: {
            a = vec3<f32>(0.52, 0.50, 0.47);
            // Three arches: the piers stay stone, the spans show dark water.
            let along = (pos.x - river_x(86.0)) / 6.0;
            let arch = abs(fract(along + 0.5) - 0.5);
            if !top && arch < 0.36 && pos.y < h.col.top - 0.8 - 6.0 * arch * arch {
                a = vec3<f32>(0.05, 0.07, 0.09);
            }
        }
        case 4u: {
            a = select(vec3<f32>(0.47, 0.46, 0.45), vec3<f32>(0.30, 0.31, 0.34), top);
            let pane = all(abs(face - 0.5) < vec2<f32>(0.2, 0.32));
            if !top && pane && hash3(vec3<i32>(h.cell.x, vy, h.cell.y)) < 0.12 * min(M.windows + 0.4, 2.0) && vy % 3 == 0 && M.windows > 0.0 {
                e = vec3<f32>(2.0, 1.1, 0.45);
            }
        }
        case 5u: {
            a = select(vec3<f32>(0.33, 0.22, 0.13),
                mix(vec3<f32>(0.14, 0.27, 0.11), vec3<f32>(0.25, 0.34, 0.12), h.col.tint), cap);
        }
        case 6u: { a = vec3<f32>(0.50, 0.47, 0.42); }
        case 7u: {
            var crops = array<vec3<f32>, 3>(vec3<f32>(0.76, 0.62, 0.30), vec3<f32>(0.40, 0.54, 0.21),
                vec3<f32>(0.44, 0.33, 0.22));
            a = crops[min(u32(h.col.tint * 3.0), 2u)] * select(0.78, 1.0, (h.cell.x & 1) == 0);
        }
        default: {
            if top || h.col.top - pos.y < 1.0 {
                let high = smoothstep(38.0, 48.0, h.col.top + 6.0 * h.col.tint);
                let grass = mix(vec3<f32>(0.42, 0.50, 0.22), vec3<f32>(0.27, 0.45, 0.19), h.col.tint);
                a = mix(grass, vec3<f32>(0.46, 0.43, 0.38), high);
                if h.col.top <= WATER + 1.0 { a = vec3<f32>(0.70, 0.62, 0.44); }
                if !top { a *= 0.82; }
            } else {
                // Exposed valley walls show bedding: alternating sand and mud beds.
                let bed = hash2(vec2<i32>(i32(floor(pos.y / 2.0)), 71));
                a = mix(vec3<f32>(0.47, 0.36, 0.27), vec3<f32>(0.66, 0.56, 0.44), bed);
            }
        }
    }
    return Surface(a * grain, e);
}

fn light(h: Hit, pos: vec3<f32>, rd: vec3<f32>, primary: bool) -> vec3<f32> {
    let s = sun_dir();
    let sf = surface(h, pos);
    var ao = 1.0;
    var shadow = 0.0;
    let ndl = max(dot(h.n, s), 0.0);
    if primary {
        if h.n.y > 0.5 {
            var higher = 0.0;
            var around = array<vec2<i32>, 4>(vec2<i32>(1, 0), vec2<i32>(-1, 0), vec2<i32>(0, 1), vec2<i32>(0, -1));
            for (var k = 0; k < 4; k++) {
                higher += select(0.0, 1.0, column(h.cell + around[k]).top > h.col.top);
            }
            ao = 1.0 - 0.11 * higher;
        } else {
            let next = column(h.cell + vec2<i32>(i32(h.n.x), i32(h.n.z)));
            ao = mix(0.55, 1.0, clamp((pos.y - next.top) / 2.5, 0.0, 1.0));
        }
        if ndl > 0.0 && !trace(pos + h.n * 0.02 + vec3<f32>(0.0, 0.01, 0.0), s, 72, 160.0).ok {
            shadow = 1.0;
        }
    } else {
        shadow = 0.6;
    }
    let ambient = mix(M.amb_dn, M.amb_up, h.n.y * 0.5 + 0.5);
    let sun = M.key_col * ndl * shadow;
    var color = srgb_decode(sf.albedo) * (sun + ambient * ao) + sf.emission;
    return mix(color, fog(rd), haze(h.t, pos.y));
}

fn aces(x: vec3<f32>) -> vec3<f32> {
    return clamp(x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment fn fs_main(v: Vertex) -> @location(0) vec4<f32> {
    let uv = v.uv;
    let pixel = uv * scene.look.zw;
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let t = scene.clock.x;
    M = mood(u32(scene.look.x));
    // Three standpoints: the province from a ridge, the riverside below the
    // hamlet looking up at the walls, and a high pass over the whole valley.
    var ro = vec3<f32>(8.0 + 3.0 * sin(t * 0.045), 70.0, 2.0 + 2.0 * sin(t * 0.03));
    var aim = vec3<f32>(58.0, 36.0, 210.0);
    var lens = 0.42;
    let pose = u32(scene.look.y);
    if pose == 1u {
        ro = vec3<f32>(38.0 + 1.5 * sin(t * 0.05), 29.0 + 0.5 * sin(t * 0.04), 20.0);
        aim = vec3<f32>(60.0, 40.0, 132.0);
        lens = 0.50;
    } else if pose == 2u {
        ro = vec3<f32>(-14.0 + 4.0 * sin(t * 0.04), 88.0, -4.0 + 3.0 * sin(t * 0.028));
        aim = vec3<f32>(62.0, 57.0, 170.0);
        lens = 0.40;
    }
    let forward = normalize(aim - ro);
    let right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), forward));
    let up = cross(forward, right);
    let rd = normalize(forward + right * ndc.x * scene.clock.y * lens + up * ndc.y * lens);
    var color: vec3<f32>;
    let hit = trace(ro, rd, 440, 320.0);
    if !hit.ok {
        color = backdrop(rd);
    } else if hit.col.kind == 1u {
        let pos = ro + rd * hit.t;
        // Ripples run downstream; reflection sees the town, hills and sky.
        let w = pos.xz * vec2<f32>(0.9, 0.35) + vec2<f32>(0.0, -t * 0.6);
        let ripple = vec2<f32>(sin(w.x * 2.1 + sin(w.y * 1.3)), sin(w.y * 2.7 + w.x * 0.4)) * 0.035;
        let n = normalize(vec3<f32>(ripple.x, 1.0, ripple.y));
        let rr = reflect(rd, n);
        let fres = 0.03 + 0.97 * pow(1.0 - max(dot(-rd, n), 0.0), 5.0);
        let back = trace(pos + vec3<f32>(0.0, 0.05, 0.0), rr, 120, 220.0);
        var mirror = backdrop(rr);
        if back.ok { mirror = light(back, pos + rr * back.t, rr, false); }
        color = mix(vec3<f32>(0.02, 0.045, 0.05), mirror, max(fres, 0.35));
        color = mix(color, fog(rd), haze(hit.t, pos.y));
    } else {
        color = light(hit, ro + rd * hit.t, rd, true);
    }
    // Hearth smoke: puffs climb a bent column over five chimneys and thin out.
    var depth = 1e5;
    if hit.ok { depth = hit.t; }
    let h0 = vec2<f32>(river_x(86.0) + 24.0, 86.0);
    let h1 = vec2<f32>(river_x(214.0) - 22.0, 214.0);
    var chimneys = array<vec3<f32>, 5>(
        vec3<f32>(town().x - 12.0, PLATEAU + 7.0, TOWN_Z - 10.0),
        vec3<f32>(town().x + 9.0, PLATEAU + 6.0, TOWN_Z - 14.0),
        vec3<f32>(town().x + 14.0, PLATEAU + 7.0, TOWN_Z + 6.0),
        vec3<f32>(h0.x + 2.0, terrain(h0) + 6.0, h0.y + 1.0),
        vec3<f32>(h1.x - 2.0, terrain(h1) + 6.0, h1.y));
    var smoke = 0.0;
    for (var c = 0; c < 5; c++) {
        for (var k = 0; k < 10; k++) {
            let rise = fract(f32(k) / 10.0 + t * 0.045 + f32(c) * 0.37);
            let hgt = rise * 16.0;
            let puff = chimneys[c] + vec3<f32>(0.035 * hgt * hgt + 0.4 * sin(hgt * 0.9 + f32(c)), hgt, 0.0);
            let along = dot(puff - ro, rd);
            if along > 0.0 && along < depth {
                let radius = 0.45 + 0.10 * hgt;
                let miss = length(puff - ro - rd * along);
                smoke += smoothstep(radius, 0.0, miss) * (1.0 - rise) * smoothstep(0.0, 0.08, rise) * 0.6;
            }
        }
    }
    color = mix(color, (M.fog_a + M.fog_s) * 0.45 + M.key_col * 0.06, min(smoke, 1.0) * 0.42);
    // A skein of five birds crossing the upper frame, one pixel each with a wingbeat.
    if M.birds > 0.0 {
        let lead = vec2<f32>(fract(0.60 + t * 0.006), 0.17 + 0.012 * sin(t * 0.21));
        for (var k = 0; k < 5; k++) {
            let o = f32(k - 2);
            let at = vec2<i32>(floor((lead + vec2<f32>(-abs(o) * 0.011, o * 0.014)) * scene.look.zw));
            let dp = vec2<i32>(floor(pixel)) - at;
            let beat = select(0, -1, fract(t * 1.4 + f32(k) * 0.37) < 0.5);
            if (dp.x == 0 && dp.y == 0) || (abs(dp.x) == 1 && dp.y == beat) {
                color = mix(color, M.amb_dn * 0.5, 0.85 * M.birds);
            }
        }
    }
    color = srgb_encode(aces(color * M.exposure));
    // A quiet band keeps type legible without glass UI cards; the host passes
    // where the menu column ends, or -1 for an even shade behind a centred stack.
    var shade = 0.42;
    if scene.clock.z >= 0.0 { shade = mix(0.42, 1.0, smoothstep(scene.clock.z * 0.3, scene.clock.z + 0.16, uv.x)); }
    let vignette = 1.0 - 0.55 * dot(uv - vec2<f32>(0.5), uv - vec2<f32>(0.5));
    color *= shade * vignette * (1.0 - 0.65 * scene.clock.w);
    color += (hash2(vec2<i32>(pixel)) - 0.5) / 255.0;
    return vec4<f32>(color, 1.0);
}
