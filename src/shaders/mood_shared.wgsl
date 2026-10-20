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

fn mood_stars(dir: vec3<f32>, scale: f32, cutoff: f32, radius: f32, t: f32, seed: f32) -> vec3<f32> {
    let face = mood_dir_face(dir);
    let grid = face.xy * scale;
    let cell = floor(grid);
    let key = vec3<f32>(cell, face.z + seed);
    let h = mood_hash31(key);
    let jitter = vec2<f32>(
        mood_hash31(key + vec3<f32>(3.1, 7.7, 1.3)),
        mood_hash31(key + vec3<f32>(9.2, 2.4, 5.8)));
    let d = length(fract(grid) - (vec2<f32>(0.2) + jitter * 0.6));
    let present = step(cutoff, h);
    let shape = 1.0 - smoothstep(radius * 0.35, radius, d);
    let temp = mood_hash31(key + vec3<f32>(1.7, 4.9, 6.1));
    let tint = mix(vec3<f32>(0.62, 0.72, 1.00), vec3<f32>(1.00, 0.90, 0.72), smoothstep(0.15, 0.9, temp));
    let bright = 0.45 + 0.55 * mood_hash31(key + vec3<f32>(8.8, 0.4, 2.2));
    let twinkle = 0.78 + 0.22 * sin(t * (0.8 + temp * 2.2) + h * 61.0);
    return tint * (present * shape * bright * twinkle);
}

// The inside of a cosmos chamber's sky: a tilted galactic band, nebula, and
// three layers of stars. Stars are fixed in direction, so the room turns
// around you as you turn your head.
fn mood_cosmos_backdrop(dir: vec3<f32>, t: f32) -> vec3<f32> {
    let axis = normalize(vec3<f32>(0.35, 0.62, 0.70));
    let band = exp(-pow(dot(dir, axis) * 3.4, 2.0));
    let p = dir * 2.4 + vec3<f32>(0.0, t * 0.003, 0.0);
    let n1 = mood_fbm(p);
    let n2 = mood_fbm(p * 1.9 + vec3<f32>(11.0, 3.0, 7.0));
    let cloud = smoothstep(0.34, 0.80, n1) * (0.30 + band * 1.10);
    let hue = mix(vec3<f32>(0.34, 0.07, 0.58), vec3<f32>(0.05, 0.40, 0.62), n2);
    let rose = vec3<f32>(0.70, 0.12, 0.34) * smoothstep(0.55, 0.9, n2) * smoothstep(0.5, 0.9, n1);
    var c = vec3<f32>(0.003, 0.004, 0.011) + (hue + rose) * cloud * 0.62;
    c += mood_stars(dir, 46.0, 0.95, 0.20, t, 0.0) * 3.0;
    c += mood_stars(dir, 110.0, 0.965, 0.17, t, 7.0) * 2.4 * (0.6 + band);
    c += mood_stars(dir, 260.0, 0.98, 0.15, t, 19.0) * 1.8 * (0.5 + band * 1.5);
    return c;
}

// Motes of starlight hung in the air of the chamber. They live in world
// space, so they slide past you as you walk and sell the sense of depth.
fn mood_motes(origin: vec3<f32>, dir: vec3<f32>, max_d: f32, t: f32) -> vec3<f32> {
    var acc = vec3<f32>(0.0);
    for (var i = 0; i < 5; i++) {
        let d = 1.8 + f32(i) * 3.1;
        if d > max_d { break; }
        let g = (origin + dir * d) / 1.9 + vec3<f32>(0.0, t * 0.04, 0.0);
        let cell = floor(g);
        let h = mood_hash31(cell);
        if h > 0.94 {
            let c = cell + vec3<f32>(
                mood_hash31(cell + vec3<f32>(1.0, 2.0, 3.0)),
                mood_hash31(cell + vec3<f32>(4.0, 5.0, 6.0)),
                mood_hash31(cell + vec3<f32>(7.0, 8.0, 9.0))) * 0.6 + vec3<f32>(0.2);
            let dd = length(g - c);
            let glow = exp(-dd * dd * 110.0);
            let tint = mix(vec3<f32>(0.5, 0.65, 1.0), vec3<f32>(1.0, 0.8, 0.6), mood_hash31(cell + vec3<f32>(3.3)));
            acc += tint * glow * (0.5 + 0.5 * sin(t * 1.3 + h * 40.0)) * 0.9;
        }
    }
    return acc;
}

// Fog that is the cosmos. The same optical-depth idea as the air it replaces,
// but the in-scattered light is the starfield instead of the sky.
fn mood_cosmos_fog(lit: vec3<f32>, view_dir: vec3<f32>, dist: f32, cosmos: f32, t: f32, origin: vec3<f32>) -> vec3<f32> {
    let fog = (1.0 - exp(-dist / 30.0)) * cosmos;
    let space = mood_cosmos_backdrop(view_dir, t) + mood_motes(origin, view_dir, dist, t);
    // Surfaces keep a hint of their own colour so the room is still readable.
    return mix(lit, space, clamp(fog, 0.0, 0.92)) + lit * 0.0;
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
// Fireflies, pollen, diamond dust and embers: motes hung on a slowly drifting
// world-space lattice and added as in-scatter along the view ray, so they sit
// in front of surfaces and slide past with true parallax. Apparent size is
// roughly constant on screen, which keeps every mote a crisp point of light.
fn mood_ambience(origin: vec3<f32>, dir: vec3<f32>, max_d: f32, t: f32, a: vec4<f32>) -> vec3<f32> {
    let total = a.x + a.y + a.z + a.w;
    if total < 0.002 { return vec3<f32>(0.0); }
    var acc = vec3<f32>(0.0);
    let presence = a.x * 0.10 + a.y * 0.12 + a.z * 0.14 + a.w * 0.10;
    for (var i = 0; i < 8; i++) {
        let d = 2.0 + f32(i) * 3.4;
        if d > max_d { break; }
        let p = origin + dir * d;
        let g = (p + vec3<f32>(sin(t * 0.21 + p.z * 0.07) * 0.9, t * 0.12, cos(t * 0.17 + p.x * 0.07) * 0.9)) / 2.4;
        let cell = floor(g);
        let h = mood_hash31(cell);
        if h < presence {
            let c = cell + vec3<f32>(
                mood_hash31(cell + vec3<f32>(1.3, 2.1, 3.7)),
                mood_hash31(cell + vec3<f32>(4.4, 5.2, 6.1)),
                mood_hash31(cell + vec3<f32>(7.9, 8.3, 9.5))) * 0.6 + vec3<f32>(0.2);
            // Pixel particles: a hard square whose side is about two screen
            // pixels at this distance (rad scales with d). No soft falloff, so
            // every mote is a crisp block like the voxels around it.
            let rad = max(0.05, d * 0.0032);
            let box = abs(g - c) * 2.4;
            let edge = max(box.x, max(box.y, box.z));
            let glow = select(0.0, 1.35, edge < rad * 0.9);
            let pick = fract(h * 37.7) * total;
            let ph = mood_hash31(cell + vec3<f32>(3.3, 1.1, 7.7)) * 60.0;
            var col = vec3<f32>(0.0);
            if pick < a.x {
                // Fireflies breathe slowly, mostly dark, flaring warm green-gold.
                // Stepped flicker: four brightness frames, like sprite animation.
                let breath = floor(pow(0.5 + 0.5 * sin(t * (0.8 + fract(ph) * 0.9) + ph), 3.0) * 4.0) / 4.0;
                col = vec3<f32>(0.75, 1.0, 0.26) * (0.15 + 0.85 * breath) * 3.2;
            } else if pick < a.x + a.y {
                col = vec3<f32>(1.0, 0.92, 0.60) * 1.1;
            } else if pick < a.x + a.y + a.z {
                // Diamond dust: a brief, hard glint rather than a steady light.
                col = vec3<f32>(0.86, 0.95, 1.0) * pow(max(sin(t * 1.7 + ph), 0.0), 10.0) * 3.0;
            } else {
                col = vec3<f32>(1.0, 0.42, 0.14) * (0.6 + 0.4 * sin(t * 6.0 + ph)) * 2.2;
            }
            acc += col * glow * (1.0 - smoothstep(16.0, 28.0, d));
        }
    }
    return acc;
}

// Facet glints for crystals and ore: a sparse set of points on each face that
// flare as the view angle sweeps past, like light catching a gem.
fn mood_glint(p: vec3<f32>, normal: vec3<f32>, view: vec3<f32>, t: f32, scale: f32) -> f32 {
    var uv = p.xz;
    if abs(normal.x) > 0.5 { uv = p.zy; }
    else if abs(normal.z) > 0.5 { uv = p.xy; }
    let q = uv * scale;
    let cell = floor(q);
    let h = mood_hash31(vec3<f32>(cell, dot(normal, vec3<f32>(1.0, 2.0, 3.0))));
    let j = vec2<f32>(mood_hash31(vec3<f32>(cell, 5.0)), mood_hash31(vec3<f32>(cell, 9.0)));
    let d = length(fract(q) - (vec2<f32>(0.2) + j * 0.6));
    let spark = 1.0 - smoothstep(0.04, 0.14, d);
    // View-locked and soft. A clock-driven spike made standing ore glitter.
    let sweep = pow(clamp(abs(dot(view, normal)), 0.0, 1.0), 2.0) * (0.97 + 0.03 * sin(t * 0.2));
    return spark * sweep * step(0.82, h) * 0.35;
}
