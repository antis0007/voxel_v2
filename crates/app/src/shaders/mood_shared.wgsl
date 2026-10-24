// ============================================================
// mood_shared.wgsl
//
// Cosmos chambers and sky events. Included by the world and sky passes after
// the camera/lighting block, so `light` and `cam` are in scope. Every function
// is prefixed `mood_` to stay clear of the other shared helpers.
//
// light.mood = (cosmos, aurora, meteors, rainbow), each 0..1.
// ============================================================

fn mood_hash31(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += vec3<f32>(dot(q, q.yzx + vec3<f32>(33.33)));
    return fract((q.x + q.y) * q.z);
}

fn mood_noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(
        mix(mood_hash31(i), mood_hash31(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
        mix(mood_hash31(i + vec3<f32>(0.0, 1.0, 0.0)), mood_hash31(i + vec3<f32>(1.0, 1.0, 0.0)), u.x),
        u.y);
    let b = mix(
        mix(mood_hash31(i + vec3<f32>(0.0, 0.0, 1.0)), mood_hash31(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
        mix(mood_hash31(i + vec3<f32>(0.0, 1.0, 1.0)), mood_hash31(i + vec3<f32>(1.0, 1.0, 1.0)), u.x),
        u.y);
    return mix(a, b, u.z);
}

fn mood_fbm(p: vec3<f32>) -> f32 {
    return mood_noise3(p) * 0.55 + mood_noise3(p * 2.03 + vec3<f32>(5.2, 1.3, 8.1)) * 0.30
        + mood_noise3(p * 4.11 + vec3<f32>(2.7, 9.4, 3.6)) * 0.15;
}

// Cube-face parameterisation: near-uniform cells with no polar pinch.
fn mood_dir_face(d: vec3<f32>) -> vec3<f32> {
    let a = abs(d);
    if a.x >= a.y && a.x >= a.z {
        return vec3<f32>(d.y / a.x, d.z / a.x, select(0.0, 1.0, d.x > 0.0));
    }
    if a.y >= a.z {
        return vec3<f32>(d.x / a.y, d.z / a.y, select(2.0, 3.0, d.y > 0.0));
    }
    return vec3<f32>(d.x / a.z, d.y / a.z, select(4.0, 5.0, d.z > 0.0));
}

// Star colour along a rough black-body ramp: 0 = red dwarf, 1 = blue giant.
fn mood_star_colour(temp: f32) -> vec3<f32> {
    let red = vec3<f32>(1.00, 0.52, 0.32);
    let gold = vec3<f32>(1.00, 0.84, 0.60);
    let white = vec3<f32>(0.97, 0.97, 1.00);
    let blue = vec3<f32>(0.62, 0.74, 1.00);
    let a = mix(red, gold, smoothstep(0.0, 0.30, temp));
    let b = mix(a, white, smoothstep(0.25, 0.60, temp));
    return mix(b, blue, smoothstep(0.55, 1.0, temp));
}

// One layer of stars on a cube-face grid. `cutoff` sets how many cells hold a
// star, `radius` the core size in cells. Magnitudes are skewed so most stars
// are faint and a few are bright enough to carry a soft halo. The core never
// shrinks below about a pixel, so fine layers shimmer rather than sparkle.
fn mood_stars(dir: vec3<f32>, scale: f32, cutoff: f32, radius: f32, t: f32, seed: f32) -> vec3<f32> {
    let face = mood_dir_face(dir);
    let grid = face.xy * scale;
    let cell = floor(grid);
    let key = vec3<f32>(cell, face.z * 17.0 + seed);
    let h = mood_hash31(key);
    if h < cutoff {
        return vec3<f32>(0.0);
    }
    let jitter = vec2<f32>(
        mood_hash31(key + vec3<f32>(3.1, 7.7, 1.3)),
        mood_hash31(key + vec3<f32>(9.2, 2.4, 5.8)));
    let off = fract(grid) - (vec2<f32>(0.2) + jitter * 0.6);
    let d2 = dot(off, off);
    let mag = mood_hash31(key + vec3<f32>(8.8, 0.4, 2.2));
    let temp = mood_hash31(key + vec3<f32>(1.7, 4.9, 6.1));
    let sigma = max(radius * (0.18 + 0.22 * mag), 0.0007 * scale);
    let core = exp(-d2 / (2.0 * sigma * sigma));
    let halo = exp(-d2 * 40.0) * 0.12 * mag * mag * mag;
    let bright = mix(0.18, 1.0, mag * mag);
    let rate = 0.9 + temp * 2.6;
    let twinkle = 0.80 + 0.14 * sin(t * rate + h * 61.0) + 0.06 * sin(t * rate * 2.7 + h * 17.0);
    return mood_star_colour(temp) * (bright * twinkle * (core + halo));
}

// Domain-warped nebula: one cheap warp, three octaves of density. Wide
// smoothsteps keep the gradients continuous, never stepped.
fn mood_nebula(dir: vec3<f32>, band: f32, t: f32) -> vec3<f32> {
    let p = dir * 2.2 + vec3<f32>(0.0, t * 0.0020, t * 0.0012);
    let w = vec3<f32>(
        mood_noise3(p * 1.3 + vec3<f32>(1.7, 9.2, 3.1)),
        mood_noise3(p * 1.3 + vec3<f32>(8.3, 2.8, 5.4)),
        mood_noise3(p * 1.3 + vec3<f32>(4.6, 6.1, 0.9)));
    let n = mood_fbm(p + (w - vec3<f32>(0.5)) * 1.7);
    let dens = smoothstep(0.28, 0.86, n);
    let filament = pow(1.0 - abs(2.0 * n - 1.0), 5.0);
    let violet = vec3<f32>(0.30, 0.08, 0.55);
    let teal = vec3<f32>(0.05, 0.36, 0.60);
    let rose = vec3<f32>(0.78, 0.16, 0.36);
    let hue = mix(violet, teal, smoothstep(0.25, 0.75, w.x));
    // Dark dust lanes cut the galactic band so it reads as a disc seen edge-on.
    let dust = 1.0 - 0.65 * smoothstep(0.42, 0.72, w.y) * band;
    let milk = vec3<f32>(0.11, 0.10, 0.16) * band * 0.40;
    return (hue * dens * (0.22 + band * 0.95) + rose * filament * dens * 0.30 * (0.3 + band) + milk) * dust * 0.55;
}

// A distant spiral galaxy: a tilted disc with two logarithmic arms, a bulge,
// and a little dust. `size` is its angular radius in radians.
fn mood_galaxy(dir: vec3<f32>, centre: vec3<f32>, size: f32, spin: f32, squash: f32, tint: vec3<f32>, t: f32) -> vec3<f32> {
    let c = normalize(centre);
    if dot(dir, c) < cos(size * 1.6) {
        return vec3<f32>(0.0);
    }
    let u0 = normalize(cross(c, vec3<f32>(0.0, 1.0, 0.0)));
    let v0 = cross(c, u0);
    let u = u0 * cos(spin) + v0 * sin(spin);
    let v = cross(c, u);
    let xy = vec2<f32>(dot(dir, u), dot(dir, v) / squash) / size;
    let r = length(xy);
    let th = atan2(xy.y, xy.x);
    let arms = pow(0.5 + 0.5 * cos(2.0 * th - log(r + 0.04) * 4.2 - t * 0.004), 2.2);
    let disc = exp(-r * 3.4) * smoothstep(1.6, 0.9, r);
    let bulge = exp(-r * r * 38.0);
    let grain = 0.72 + 0.28 * mood_noise3(vec3<f32>(xy * 7.0, spin * 3.0));
    return tint * disc * (0.20 + 0.80 * arms) * grain * 0.55 + vec3<f32>(1.0, 0.88, 0.70) * bulge * 0.9;
}

fn mood_seg_dist(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ab = b - a;
    let k = clamp(dot(p - a, ab) / max(dot(ab, ab), 1e-6), 0.0, 1.0);
    return length(p - a - ab * k);
}

fn mood_constellation_node(key: vec3<f32>, k: f32) -> vec2<f32> {
    return vec2<f32>(0.14) + vec2<f32>(
        mood_hash31(key + vec3<f32>(1.3, 7.1, 2.9) * (k + 1.0)),
        mood_hash31(key + vec3<f32>(5.7, 3.3, 9.1) * (k + 1.0))) * 0.72;
}

// Constellations: in some coarse cells four or five bright stars joined by a
// faint thread. The whole sphere turns very slowly so they drift overhead.
fn mood_constellations(dir: vec3<f32>, t: f32) -> vec3<f32> {
    let a = t * 0.0015;
    let d = vec3<f32>(dir.x * cos(a) - dir.z * sin(a), dir.y, dir.x * sin(a) + dir.z * cos(a));
    let face = mood_dir_face(d);
    let scale = 3.0;
    let grid = face.xy * scale;
    let key = vec3<f32>(floor(grid), face.z * 13.0 + 41.0);
    let h = mood_hash31(key);
    if h < 0.45 {
        return vec3<f32>(0.0);
    }
    let f = fract(grid);
    let count = 4.0 + floor(mood_hash31(key + vec3<f32>(6.6)) * 2.0);
    var prev = mood_constellation_node(key, 0.0);
    var line = 0.0;
    var nodes = vec3<f32>(0.0);
    let pulse = 0.65 + 0.35 * sin(t * 0.21 + h * 40.0);
    let width = 0.0016 * scale;
    for (var k = 0; k < 5; k++) {
        let fk = f32(k);
        if fk >= count {
            break;
        }
        let p = mood_constellation_node(key, fk);
        let dn = length(f - p);
        let sigma = 0.0012 * scale;
        let temp = mood_hash31(key + vec3<f32>(fk * 2.3, 1.1, 0.7));
        nodes += mood_star_colour(temp) * exp(-dn * dn / (2.0 * sigma * sigma)) * 2.2;
        if k > 0 {
            let ds = mood_seg_dist(f, prev, p);
            // The thread stops short of each star so the star stays a point.
            let gap = smoothstep(0.02, 0.05, length(f - prev)) * smoothstep(0.02, 0.05, dn);
            line = max(line, exp(-ds * ds / (width * width)) * gap);
        }
        prev = p;
    }
    return nodes + vec3<f32>(0.42, 0.58, 1.0) * line * 0.045 * pulse;
}

// Shooting stars. Time is cut into slots; a slot may hold one streak whose
// start, direction and timing come from its hash, so every viewer sees the
// same meteor at the same moment.
fn mood_shooting_stars(dir: vec3<f32>, t: f32) -> vec3<f32> {
    var acc = vec3<f32>(0.0);
    for (var k = 0; k < 2; k++) {
        let fk = f32(k);
        let tt = t / (6.0 + fk * 4.5) + fk * 0.37;
        let s = vec3<f32>(floor(tt), fk * 19.0, 7.7);
        if mood_hash31(s) < 0.30 {
            continue;
        }
        let life = 0.14;
        let start = mood_hash31(s + vec3<f32>(2.0, 0.0, 0.0)) * (1.0 - life);
        let tau = (fract(tt) - start) / life;
        if tau < 0.0 || tau > 1.0 {
            continue;
        }
        let c = normalize(vec3<f32>(
            mood_hash31(s + vec3<f32>(0.0, 3.0, 0.0)) * 2.0 - 1.0,
            mood_hash31(s + vec3<f32>(0.0, 0.0, 4.0)) * 0.9 + 0.15,
            mood_hash31(s + vec3<f32>(5.0, 1.0, 0.0)) * 2.0 - 1.0));
        if dot(dir, c) < 0.85 {
            continue;
        }
        let u0 = normalize(cross(c, vec3<f32>(0.0, 1.0, 0.0)));
        let ang = mood_hash31(s + vec3<f32>(6.0, 2.0, 9.0)) * 6.2832;
        let u = u0 * cos(ang) + cross(c, u0) * sin(ang);
        let v = cross(c, u);
        let x = dot(dir, u);
        let y = dot(dir, v);
        let behind = (tau - 0.5) * 0.36 - x;
        let w = 0.0011;
        let glow = exp(-y * y / (w * w)) * exp(-max(behind, 0.0) / 0.07) * smoothstep(-0.003, 0.0, behind);
        acc += vec3<f32>(0.85, 0.92, 1.0) * glow * sin(tau * 3.14159) * 2.6;
    }
    return acc;
}

// Thin aurora curtains low around the chamber: a bright lower hem fading
// upward, combed into rays by a little noise.
fn mood_cosmos_aurora(dir: vec3<f32>, t: f32) -> vec3<f32> {
    let phi = atan2(dir.z, dir.x);
    let hem = 0.16 + 0.09 * sin(phi * 3.0 + t * 0.03) + 0.04 * sin(phi * 7.0 - t * 0.05);
    let h = dir.y - hem;
    let curtain = smoothstep(-0.03, 0.02, h) * exp(-max(h, 0.0) * 5.5);
    let flat_dir = dir.xz / max(length(dir.xz), 1e-3);
    let rays = 0.55 + 0.45 * mood_noise3(vec3<f32>(flat_dir * 11.0, t * 0.05));
    let colour = mix(vec3<f32>(0.10, 0.85, 0.55), vec3<f32>(0.50, 0.25, 0.95), smoothstep(0.0, 0.35, h));
    return colour * curtain * rays * 0.06;
}

// The inside of a cosmos chamber's sky, all of it fixed in direction (it is
// infinitely far): nebula, a galactic band, two galaxies, four layers of stars,
// constellations, shooting stars and aurora curtains. The near layer of stars
// with real parallax is `mood_motes`, which lives in world metres.
fn mood_cosmos_backdrop(dir: vec3<f32>, t: f32) -> vec3<f32> {
    let axis = normalize(vec3<f32>(0.35, 0.62, 0.70));
    let band = exp(-pow(dot(dir, axis) * 3.0, 2.0));
    var c = vec3<f32>(0.0025, 0.003, 0.009) + vec3<f32>(0.003, 0.0015, 0.006) * (dir.y * 0.5 + 0.5);
    c += mood_nebula(dir, band, t);
    c += mood_galaxy(dir, vec3<f32>(-0.55, 0.42, -0.72), 0.11, 0.6, 0.42, vec3<f32>(0.70, 0.80, 1.00), t);
    c += mood_galaxy(dir, vec3<f32>(0.70, -0.12, 0.62), 0.055, 2.1, 0.75, vec3<f32>(1.00, 0.74, 0.86), t);
    c += mood_stars(dir, 30.0, 0.965, 0.22, t, 0.0) * 3.6;
    c += mood_stars(dir, 80.0, 0.95, 0.18, t, 7.0) * 2.0 * (0.6 + band);
    c += mood_stars(dir, 190.0, 0.95, 0.16, t, 19.0) * 1.3 * (0.4 + band * 1.6);
    c += mood_stars(dir, 420.0, 0.93, 0.20, t, 31.0) * 0.6 * (0.2 + band * 2.0);
    c += mood_constellations(dir, t);
    c += mood_shooting_stars(dir, t);
    c += mood_cosmos_aurora(dir, t);
    // A whisper of fixed per-direction dither so dark gradients never band.
    let n = mood_hash31(floor(dir * 4096.0)) - 0.5;
    return max(c * (1.0 + n * 0.03) + vec3<f32>(n * 0.0006), vec3<f32>(0.0));
}

// Motes of starlight hung in the air of the chamber: the near star layer.
// They live in world metres, so they slide past you as you walk (true
// parallax) and have one density at every voxel scale.
fn mood_motes(origin: vec3<f32>, dir: vec3<f32>, max_d: f32, t: f32) -> vec3<f32> {
    let mpu = cam.world_params.x;
    let o = origin * mpu;
    let reach = max_d * mpu;
    var acc = vec3<f32>(0.0);
    for (var i = 0; i < 6; i++) {
        let d = 1.6 + f32(i) * 2.7;
        if d > reach { break; }
        let g = (o + dir * d) / 1.9 + vec3<f32>(0.0, t * 0.04, 0.0);
        let cell = floor(g);
        let h = mood_hash31(cell);
        if h > 0.94 {
            let c = cell + vec3<f32>(
                mood_hash31(cell + vec3<f32>(1.0, 2.0, 3.0)),
                mood_hash31(cell + vec3<f32>(4.0, 5.0, 6.0)),
                mood_hash31(cell + vec3<f32>(7.0, 8.0, 9.0))) * 0.6 + vec3<f32>(0.2);
            let dd = length(g - c);
            let glow = exp(-dd * dd * 110.0);
            let tint = mood_star_colour(mood_hash31(cell + vec3<f32>(3.3)));
            acc += tint * glow * (0.5 + 0.5 * sin(t * 1.3 + h * 40.0)) * 0.9;
        }
    }
    return acc;
}

// Fog that is the cosmos. The same optical-depth idea as the air it replaces,
// but the in-scattered light is the starfield instead of the sky. Depth is in
// metres so the room reads the same at every voxel scale.
fn mood_cosmos_fog(lit: vec3<f32>, view_dir: vec3<f32>, dist: f32, cosmos: f32, t: f32, origin: vec3<f32>) -> vec3<f32> {
    let fog = (1.0 - exp(-dist * cam.world_params.x / 30.0)) * cosmos;
    let space = mood_cosmos_backdrop(view_dir, t) + mood_motes(origin, view_dir, dist, t);
    // Surfaces keep a hint of their own colour so the room is still readable.
    return mix(lit, space, clamp(fog, 0.0, 0.92));
}

// ---- Sky events ----

fn mood_aurora(dir: vec3<f32>, t: f32) -> vec3<f32> {
    if dir.y < 0.04 { return vec3<f32>(0.0); }
    let p = dir.xz / (dir.y + 0.28);
    var sum = vec3<f32>(0.0);
    for (var k = 0; k < 2; k++) {
        let fk = f32(k);
        let wave = sin(p.x * (1.3 + fk * 0.7) + t * (0.10 + fk * 0.04) + fk * 2.0) * 0.55
            + sin(p.x * (2.9 - fk * 0.5) - t * 0.17) * 0.22;
        let ribbon = exp(-pow((p.y + wave - 1.15 - fk * 0.9) * (2.0 + fk * 0.6), 2.0));
        let rays = 0.55 + 0.45 * sin(p.x * (16.0 + fk * 5.0) + mood_noise3(vec3<f32>(p.x * 1.6, fk * 9.0, t * 0.08)) * 7.0);
        let green = vec3<f32>(0.10, 1.00, 0.46);
        let violet = vec3<f32>(0.55, 0.20, 0.95);
        let colour = mix(green, violet, smoothstep(0.35, 0.95, dir.y + fk * 0.15));
        sum += colour * ribbon * rays * (0.85 - fk * 0.30);
    }
    let fall = smoothstep(0.04, 0.22, dir.y) * (1.0 - smoothstep(0.62, 0.98, dir.y));
    return sum * fall * 0.50;
}

fn mood_meteors(dir: vec3<f32>, t: f32) -> vec3<f32> {
    var acc = vec3<f32>(0.0);
    for (var k = 0; k < 6; k++) {
        let period = 2.6 + f32(k) * 0.83;
        let local = t / period + f32(k) * 0.37;
        let id = floor(local);
        let phase = fract(local);
        let seed = vec3<f32>(id, f32(k), 4.0);
        let sy = 0.35 + mood_hash31(seed + vec3<f32>(1.0)) * 0.55;
        let az = mood_hash31(seed + vec3<f32>(2.0)) * 6.2831853;
        let start = normalize(vec3<f32>(cos(az) * (1.0 - sy * 0.5), sy, sin(az) * (1.0 - sy * 0.5)));
        let heading = mood_hash31(seed + vec3<f32>(3.0)) * 6.2831853;
        let side = normalize(cross(start, vec3<f32>(0.0, 1.0, 0.0)));
        let up = normalize(cross(side, start));
        let tangent = normalize(side * cos(heading) + up * sin(heading));
        let reach = 0.55;
        let head = normalize(start + tangent * phase * reach);
        let tail = normalize(start + tangent * max(phase - 0.14, 0.0) * reach);
        let seg = head - tail;
        let seg_len = max(length(seg), 1e-4);
        let along = clamp(dot(dir - tail, seg) / (seg_len * seg_len), 0.0, 1.0);
        let closest = tail + seg * along;
        let d = length(normalize(closest) - dir);
        let width = 0.0016 + along * 0.0016;
        let streak = exp(-(d * d) / (width * width));
        let life = smoothstep(0.0, 0.08, phase) * (1.0 - smoothstep(0.78, 1.0, phase));
        let on = step(0.55, mood_hash31(seed + vec3<f32>(5.0)));
        let hot = mix(vec3<f32>(0.75, 0.88, 1.0), vec3<f32>(1.0, 0.92, 0.75), along);
        acc += hot * streak * (0.25 + along * 1.1) * life * on;
    }
    return acc * 2.6;
}

fn mood_rainbow(dir: vec3<f32>, sun_dir: vec3<f32>) -> vec3<f32> {
    let cos_a = clamp(dot(dir, -sun_dir), -1.0, 1.0);
    let angle = acos(cos_a);
    // Primary bow: violet at 40.6 deg, red at 42.3 deg from the antisolar
    // point. The old 5 deg band of pure hues read as a cartoon arc.
    let x = (angle - 0.708) / 0.031;
    if x < 0.0 || x > 1.0 { return vec3<f32>(0.0); }
    // Red on the outside, violet inside. Real droplets have a finite size and
    // the sun a finite disk, so the bands overlap and wash toward white.
    let hue = 1.0 - x;
    let pure = clamp(vec3<f32>(
        1.0 - abs(hue * 4.0 - 3.0),
        1.0 - abs(hue * 4.0 - 2.0),
        1.0 - abs(hue * 4.0 - 1.0)), vec3<f32>(0.0), vec3<f32>(1.0));
    let spectrum = mix(vec3<f32>(dot(pure, vec3<f32>(0.3333))), pure, 0.62);
    let fade = smoothstep(0.0, 0.25, x) * (1.0 - smoothstep(0.75, 1.0, x));
    let horizon = smoothstep(-0.015, 0.09, dir.y);
    return spectrum * fade * horizon * 0.10;
}

// ---- Living air ----
// Fireflies, pollen, diamond dust and embers: one mote per world-space lattice
// cell, bobbing inside its own cell, added as in-scatter along the view ray so
// they sit in front of surfaces and slide past with true parallax. A mote is
// exactly one native screen pixel at any distance, in a palette colour: it is
// one more pixel of the same art as the world, not a soft light.
// Every input is a pure function of world position and the simulation clock:
// camera motion may change which motes a ray crosses, never where a mote is.
// Mote palette: the shared pixel palette (docs/PIXEL_ART_DIRECTION.md), in
// linear light. Every mote takes one of these colours; nothing is blended.
const MOTE_HIGHLIGHT: vec3<f32> = vec3<f32>(0.930, 0.768, 0.348);
const MOTE_PAPER: vec3<f32> = vec3<f32>(0.846, 0.753, 0.558);
const MOTE_GOLD: vec3<f32> = vec3<f32>(0.700, 0.418, 0.117);
const MOTE_EMBER: vec3<f32> = vec3<f32>(0.529, 0.133, 0.031);
const MOTE_TEAL: vec3<f32> = vec3<f32>(0.130, 0.472, 0.430);

// `pixel` is the angle one screen pixel subtends, in radians. Every mote is
// drawn as exactly one native screen pixel (or a one-pixel plus sign for a
// glint) whatever its distance. No halo, no soft edge, no size that changes
// with range: it reads as one more pixel of the same art as the world.
fn mood_ambience(origin: vec3<f32>, dir: vec3<f32>, max_d: f32, t: f32, a: vec4<f32>, pixel: f32) -> vec3<f32> {
    let total = a.x + a.y + a.z + a.w;
    if total < 0.002 { return vec3<f32>(0.0); }
    // Sparse on purpose: a mote should be found, not sprinkled across every view.
    let presence = (a.x * 0.10 + a.y * 0.12 + a.z * 0.14 + a.w * 0.10) * 0.5;
    let far = min(max_d, 22.0);
    let pixel_angle = max(pixel, 1e-4);

    // One mote per lattice cell, fixed in world space. The ray walks the cells
    // it crosses (Amanatides-Woo) and takes its closest approach to each mote.
    // Sampling fixed points along the ray made a mote appear only when a sample
    // landed inside it, and warping the lattice by the sample position made a
    // mote hop between cells; this version has neither.
    let cell_size = 2.4;
    var cell = floor(origin / cell_size);
    let step_dir = select(vec3<f32>(-1.0), vec3<f32>(1.0), dir >= vec3<f32>(0.0));
    let safe_dir = select(vec3<f32>(1e-6), abs(dir), abs(dir) > vec3<f32>(1e-6));
    let t_delta = vec3<f32>(cell_size) / safe_dir;
    let next_edge = (cell + max(step_dir, vec3<f32>(0.0))) * cell_size;
    var t_max = (next_edge - origin) * step_dir / safe_dir;

    // Screen-aligned axes: they give the offset from the pixel centre in pixels.
    let up_ref = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(dir.y) > 0.99);
    let axis_r = normalize(cross(dir, up_ref));
    let axis_u = cross(axis_r, dir);

    var acc = vec3<f32>(0.0);
    for (var i = 0; i < 20; i++) {
        let h = mood_hash31(cell);
        if h < presence {
            let jitter = vec3<f32>(
                mood_hash31(cell + vec3<f32>(1.3, 2.1, 3.7)),
                mood_hash31(cell + vec3<f32>(4.4, 5.2, 6.1)),
                mood_hash31(cell + vec3<f32>(7.9, 8.3, 9.5)));
            let ph = mood_hash31(cell + vec3<f32>(3.3, 1.1, 7.7)) * 60.0;
            let pick = fract(h * 37.7) * total;
            // 0 firefly, 1 pollen, 2 diamond dust, 3 ember.
            var kind = 3;
            if pick < a.x + a.y + a.z { kind = 2; }
            if pick < a.x + a.y { kind = 1; }
            if pick < a.x { kind = 0; }
            let rising = kind == 1 || kind == 3;

            // Rest position in the middle half of the cell plus a bounded drift,
            // so the mote never leaves its own cell and cannot pop at a border.
            var offset = (jitter - vec3<f32>(0.5)) * 0.5;
            var drift = vec3<f32>(
                sin(t * 0.21 + ph),
                sin(t * 0.33 + ph * 1.7) * 0.7,
                cos(t * 0.17 + ph * 0.6)) * 0.16;
            // Pollen and embers climb through their cell, then start again from
            // the bottom. `vis` is zero at both ends of the climb, so the loop
            // never shows.
            var vis = 1.0;
            if rising {
                let u = fract(t * select(0.05, 0.11, kind == 3) + ph * 0.137);
                offset.y = 0.0;
                drift.y = (u - 0.5) * 0.9;
                vis = sin(3.14159265 * u);
            }
            let center = (cell + vec3<f32>(0.5) + offset + drift) * cell_size;
            let rel = center - origin;
            let s = dot(rel, dir);
            if s > 0.15 && s < far && vis > 0.05 {
                // Offset from this pixel's ray, measured in pixels at the mote.
                let perp = rel - dir * s;
                let one_pixel = s * pixel_angle;
                let ox = abs(dot(perp, axis_r)) / one_pixel;
                let oy = abs(dot(perp, axis_u)) / one_pixel;

                // Brightness is stepped, like palette-swapped sprite frames.
                var col = vec3<f32>(0.0);
                var glint = false;
                let tone = fract(ph * 0.37);
                if kind == 0 {
                    let breath = 0.5 + 0.5 * sin(t * (0.8 + fract(ph) * 0.9) + ph);
                    let step_level = floor(breath * breath * breath * 3.0 + 0.5) / 3.0;
                    col = select(MOTE_HIGHLIGHT, MOTE_TEAL, tone > 0.78) * step_level * 3.4;
                    glint = step_level >= 0.99;
                } else if kind == 1 {
                    let level = select(0.55, 1.0, sin(t * 1.3 + ph) > 0.0);
                    col = MOTE_PAPER * level * 1.5 * select(0.0, 1.0, vis > 0.4);
                } else if kind == 2 {
                    let flash = pow(max(sin(t * 1.7 + ph), 0.0), 10.0);
                    let level = floor(flash * 3.0 + 0.5) / 3.0;
                    col = select(MOTE_PAPER, MOTE_TEAL, tone > 0.5) * level * 3.6;
                    glint = level >= 0.99;
                } else {
                    // Embers cool through three palette steps as they rise.
                    let step_colour = select(select(MOTE_EMBER, MOTE_GOLD, vis > 0.45), MOTE_HIGHLIGHT, vis > 0.8);
                    col = step_colour * 2.8;
                }
                // The pixel itself, plus a one-pixel plus sign when it glints.
                var shape = select(0.0, 1.0, ox <= 0.5 && oy <= 0.5);
                if glint {
                    let arm_x = ox <= 1.5 && oy <= 0.5;
                    let arm_y = oy <= 1.5 && ox <= 0.5;
                    shape = select(shape, 0.7, (arm_x || arm_y) && shape < 0.5);
                }
                let fade = smoothstep(0.15, 1.2, s) * (1.0 - smoothstep(far * 0.7, far, s));
                acc += col * shape * fade;
            }
        }
        // Step into the next cell along the ray; stop once past the draw range.
        let nearest = min(t_max.x, min(t_max.y, t_max.z));
        if nearest > far { break; }
        if t_max.x <= t_max.y && t_max.x <= t_max.z {
            cell.x += step_dir.x;
            t_max.x += t_delta.x;
        } else if t_max.y <= t_max.z {
            cell.y += step_dir.y;
            t_max.y += t_delta.y;
        } else {
            cell.z += step_dir.z;
            t_max.z += t_delta.z;
        }
    }
    return acc;
}

// Pixel glimmer for crystals and ore. Each voxel face is a grid of texels.
// A texel is a glint candidate only if its hash says so (about one in six),
// and it carries a tilted facet normal that re-rolls on a stepped clock. The
// texel lights for a single frame when that facet's normal lines up with the
// half vector between the eye and the sun, so glints are hard one-texel
// flashes that blink on and off instead of a soft view-locked haze.
// One glint texel: x is the flash level (0 off, else 1/3, 2/3 or 1) and y the
// same level if the flash is bright enough to throw a cross.
//
// Facets are static per texel, so a glint sits still on its texel and the set
// of lit texels sweeps across the surface as the camera moves, like a real cut
// stone. Each candidate texel then twinkles on its own slow cycle in three
// stepped levels. Nothing re-rolls per frame, so there is no strobing.
// `sheen` (0..1) also lights candidates without needing sun alignment, which
// lets a travelling band of sparkle work at night.
fn mood_glint_texel(cell: vec2<f32>, normal: vec3<f32>, half_dir: vec3<f32>, t: f32, sheen: f32) -> vec2<f32> {
    let key = vec3<f32>(cell, dot(normal, vec3<f32>(1.0, 2.0, 3.0)));
    var align = 0.0;
    if mood_hash31(key) >= 0.6 {
        let tilt = vec3<f32>(
            mood_hash31(key + vec3<f32>(3.1, 1.7, 0.0)),
            mood_hash31(key + vec3<f32>(3.1, 4.1, 2.9)),
            mood_hash31(key + vec3<f32>(3.1, 6.3, 5.2))) - vec3<f32>(0.5);
        // Wide facet spread: the sun-eye half vector sits 30 degrees or more
        // off a vertical face at noon, so a narrow tilt never reached it.
        align = max(dot(normalize(normal + tilt * 1.6), half_dir), 0.0);
    }
    let pick = mood_hash31(key + vec3<f32>(5.5, 9.9, 1.1));
    let sheen_on = step(pick, sheen * 0.07);
    let flash = max(step(0.985, align), sheen_on);
    let cross = max(step(0.995, align), sheen_on * step(0.75, mood_hash31(key + vec3<f32>(2.2, 7.7, 3.3))));
    // Triangle envelope on a per-texel phase, quantised to three levels.
    let phase = fract(t * 0.8 + mood_hash31(key + vec3<f32>(8.3, 2.2, 6.1)));
    let env = 1.0 - abs(phase * 2.0 - 1.0);
    let level = select(0.0, ceil(env * 3.0) / 3.0, env > 0.12);
    return vec2<f32>(flash * level, cross * level);
}

// Returns the flash level on a flashing texel and 0.6 of the neighbour's level
// on the four arm texels of a cross, so the brightest glints read as a
// one-texel plus sign.
fn mood_glint(p: vec3<f32>, normal: vec3<f32>, toward_eye: vec3<f32>, sun: vec3<f32>, t: f32, scale: f32, sheen: f32) -> f32 {
    var uv = p.xz;
    if abs(normal.x) > 0.5 { uv = p.zy; }
    else if abs(normal.z) > 0.5 { uv = p.xy; }
    let cell = floor(uv * scale);
    let half_dir = normalize(toward_eye + sun);
    let centre = mood_glint_texel(cell, normal, half_dir, t, sheen).x;
    if centre > 0.0 { return centre; }
    var arm = 0.0;
    arm = max(arm, mood_glint_texel(cell + vec2<f32>(1.0, 0.0), normal, half_dir, t, sheen).y);
    arm = max(arm, mood_glint_texel(cell - vec2<f32>(1.0, 0.0), normal, half_dir, t, sheen).y);
    arm = max(arm, mood_glint_texel(cell + vec2<f32>(0.0, 1.0), normal, half_dir, t, sheen).y);
    arm = max(arm, mood_glint_texel(cell - vec2<f32>(0.0, 1.0), normal, half_dir, t, sheen).y);
    return arm * 0.6;
}

// A slow diagonal band of light that sweeps across crystal in world space,
// one band every five metres, a bit under a metre wide. Smooth, not stepped.
fn crystal_sheen(world_m: vec3<f32>, t: f32) -> f32 {
    let s = dot(world_m, vec3<f32>(0.62, 0.30, 0.72)) - t * 0.55;
    let d = (fract(s / 5.0) - 0.5) * 5.0;
    return exp(-d * d / 0.5);
}
