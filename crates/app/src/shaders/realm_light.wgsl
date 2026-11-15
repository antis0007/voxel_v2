// ============================================================
// realm_light.wgsl
//
// Light rig of a sunless realm (world.rs `LightingState::realm`, written by
// `world/portals/realm.rs`). All words are zero outside such a realm, and
// every function here then returns zero, so other worlds are unchanged.
//
//  * Glow pools: still water that emits its own light. Seen from above, the
//    bed shows through clear water lit by a moving caustic web; the same web
//    is thrown up onto the walls and ceiling above the pool and onto anything
//    submerged. Pools are a disk or a box on xz with a surface height.
//  * Fill: a sky-free ambient. Underground the sky ambient is gated by sky
//    access and reads zero, so a realm brings its own.
//  * Air: in-scatter toward the realm's air colour over the realm's range.
//  * Shaft: a vertical beam of light with a mist halo, and the pool of light
//    it puts on whatever it falls on.
//
// Caustic webs reuse `uw_caustic` and snap to the shared world texel lattice
// (`texel_centre`), so they keep the one-pixel size of every world pattern.
// ============================================================

const REALM_SHAFT_COLOR: vec3<f32> = vec3<f32>(0.78, 0.93, 1.0);
const REALM_WATER_ABSORB: vec3<f32> = vec3<f32>(0.34, 0.075, 0.040);

fn realm_rig_on() -> bool {
    return light.realm[0].x > 0.5;
}

// Linear colour of a packed 0xRRGGBB sRGB layout colour (Layout fields).
fn realm_unpack(packed: f32) -> vec3<f32> {
    let v = u32(max(packed, 0.0));
    let c = vec3<f32>(f32((v >> 16u) & 255u), f32((v >> 8u) & 255u), f32(v & 255u)) / 255.0;
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

// Water glow colour of a pool, from its packed layout colour (word z).
fn realm_pool_tint(packed: f32) -> vec3<f32> {
    return realm_unpack(packed);
}

// Pool flags in word w: bit 0 round, bit 1 a coin bed.
fn realm_pool_flag(w: f32, bit: u32) -> bool {
    return (u32(max(w, 0.0)) & bit) != 0u;
}

// Colour of the shaft of light; a zero word falls back to the authored white.
fn realm_shaft_color() -> vec3<f32> {
    let packed = light.realm[10].z;
    return select(REALM_SHAFT_COLOR, realm_unpack(packed), packed > 0.5);
}

// Signed horizontal distance to pool `i` in metres; negative inside.
fn realm_pool_sdf(i: u32, xz: vec2<f32>) -> f32 {
    let a = light.realm[3u + i * 2u];
    let d = xz - a.xy;
    if realm_pool_flag(light.realm[4u + i * 2u].w, 1u) {
        return length(d) - a.z;
    }
    let q = abs(d) - a.zw;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0);
}

// Index of the pool whose water holds `p`, or -1.
fn realm_pool_at(p: vec3<f32>) -> i32 {
    for (var i = 0u; i < 3u; i += 1u) {
        let b = light.realm[4u + i * 2u];
        if b.y > 0.0 && p.y < b.x + 0.6 && realm_pool_sdf(i, p.xz) < 0.6 {
            return i32(i);
        }
    }
    return -1;
}

// Caustic web on the world texel lattice. `blur` 0..1 fades it to its mean
// where the light has spread far from the water; unresolved texels hold the mean.
fn realm_caustic(p: vec2<f32>, t: f32, blur: f32, fp_m: f32) -> f32 {
    let web = uw_caustic(texel_centre(p) * 2.1, t * 0.9);
    return mix(0.36, web, fluid_pixel_weight(fp_m) * (1.0 - blur));
}

// Irradiance a solid surface takes from the glowing pools, pre-exposure.
// Above the water: the light sheet below, rippled with caustics that soften
// with distance. Under it: caustics focused down from the moving surface.
fn realm_pool_light(p: vec3<f32>, n: vec3<f32>, t: f32, fp_m: f32) -> vec3<f32> {
    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < 3u; i += 1u) {
        let b = light.realm[4u + i * 2u];
        if b.y <= 0.0 { continue; }
        let sdf = realm_pool_sdf(i, p.xz);
        let outside = max(sdf, 0.0);
        let h = p.y - b.x;
        if outside > 40.0 || h > 50.0 { continue; }
        let tint = realm_pool_tint(b.z) * b.y;
        if h >= 0.0 {
            // Toward the nearest point of the sheet.
            let e = 0.05;
            let grad = vec2<f32>(
                realm_pool_sdf(i, p.xz + vec2<f32>(e, 0.0)) - realm_pool_sdf(i, p.xz - vec2<f32>(e, 0.0)),
                realm_pool_sdf(i, p.xz + vec2<f32>(0.0, e)) - realm_pool_sdf(i, p.xz - vec2<f32>(0.0, e))) / (2.0 * e);
            let toward = normalize(vec3<f32>(-grad.x * outside, -h - 0.25, -grad.y * outside));
            let facing = clamp(dot(n, toward) * 0.7 + 0.3, 0.0, 1.0);
            let reach = 1.0 / (1.0 + (h * h + outside * outside) / 70.0);
            // Ceilings take the web on xz; walls on their own plane, so the
            // bands run along the wall and climb it instead of smearing.
            // Walls: the web is drawn out upward from the water, as light
            // thrown by moving ripples streaks the rock above a pool.
            var plane = p.xz;
            if abs(n.y) < 0.6 {
                let along = normalize(vec2<f32>(-n.z, n.x) + vec2<f32>(1e-4, 0.0));
                plane = vec2<f32>(dot(p.xz, along), p.y * 0.42);
            }
            let blur = smoothstep(1.0, 12.0, h + outside);
            let web = realm_caustic(plane, t, blur, fp_m);
            // Light leaving the water is whiter than the water body.
            let light_col = mix(tint, vec3<f32>(b.y), 0.25);
            sum += light_col * reach * facing * (0.20 + 1.6 * web);
        } else if sdf < 0.3 {
            let up = clamp(n.y * 0.6 + 0.4, 0.0, 1.0);
            let web = realm_caustic(p.xz, t, 0.0, fp_m);
            sum += mix(tint, vec3<f32>(b.y), 0.45) * up * exp(-0.10 * -h) * (0.25 + 2.2 * web);
        }
    }
    return sum * 0.60;
}

// Emission of a glowing pool's surface seen from above: the bed through clear
// water lit by the caustic web, the luminous water column, and bright filaments
// where the moving surface focuses. `bed` is the mesh's substrate colour.
fn realm_water_glow(p: vec3<f32>, depth: f32, bed: vec3<f32>, view: vec3<f32>, n: vec3<f32>, t: f32, fp_m: f32) -> vec3<f32> {
    let pool = realm_pool_at(p);
    if pool < 0 { return vec3<f32>(0.0); }
    let b = light.realm[4u + u32(pool) * 2u];
    let tint = realm_pool_tint(b.z) * b.y;
    var ray = refract(view, n, 0.75);
    if dot(ray, ray) < 0.5 { ray = view; }
    let path = max(depth, 0.05) / max(-ray.y, 0.25);
    let bed_xz = p.xz + ray.xz * path;
    let web = realm_caustic(bed_xz, t, 0.0, fp_m);
    // Palace pools hold gold coins: a bright yellow bed under white light read
    // as green slime, so their bed sits deeper in the blue body.
    let palace = realm_pool_flag(b.w, 2u);
    let trans = exp(-REALM_WATER_ABSORB * path * select(1.0, 2.2, palace));
    let bed_light = mix(tint, vec3<f32>(b.y), select(0.60, 0.25, palace)) * (0.40 + 2.6 * web);
    let lit_bed = max(bed, vec3<f32>(0.03)) * bed_light * select(1.0, 0.55, palace);
    let column = tint * select(0.20, 0.34, palace) * (1.0 - exp(-path * select(0.45, 1.1, palace)));
    let film = realm_caustic(p.xz * 1.7 + vec2<f32>(3.1, -1.7), t * 1.3, 0.0, fp_m);
    let filaments = mix(tint, vec3<f32>(1.0), 0.5) * film * film * film * 0.30 * b.y;
    return lit_bed * trans + column + filaments;
}

// Sky-free fill plus pool and shaft light for a solid surface, pre-exposure.
fn realm_surface_light(p: vec3<f32>, n: vec3<f32>, t: f32, fp_m: f32, cavity: f32, hemi: f32) -> vec3<f32> {
    let fill = light.realm[1].rgb * (0.55 + 0.45 * cavity) * (0.75 + 0.25 * hemi);
    return fill + realm_pool_light(p, n, t, fp_m) + realm_shaft_light(p, n);
}

// The pool of light the shaft lays on the summit, with a faint halo.
fn realm_shaft_light(p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    let s = light.realm[9];
    let y = light.realm[10];
    if s.w <= 0.0 || p.y < y.x - 1.5 || p.y > y.y { return vec3<f32>(0.0); }
    let d = length(p.xz - s.xy) / max(s.z, 0.1);
    let up = max(n.y, 0.0);
    let spot = (1.0 - smoothstep(0.7, 1.5, d)) * up * 1.6;
    let halo = (1.0 - smoothstep(1.0, 4.0, d)) * 0.10 * (0.5 + 0.5 * up);
    return realm_shaft_color() * s.w * (spot + halo);
}

// Path length of the view ray inside a vertical cylinder, weighted toward the
// axis so the beam has a soft edge.
fn realm_cylinder_path(eye: vec3<f32>, dir: vec3<f32>, dist: f32, c: vec2<f32>, r: f32, y0: f32, y1: f32) -> f32 {
    let o = eye.xz - c;
    let d = dir.xz;
    let a = dot(d, d);
    var t0 = 0.0;
    var t1 = dist;
    var closest = length(o) / r;
    if a > 1e-6 {
        let b = dot(o, d);
        let cc = dot(o, o) - r * r;
        let disc = b * b - a * cc;
        if disc <= 0.0 { return 0.0; }
        let sq = sqrt(disc);
        t0 = max(t0, (-b - sq) / a);
        t1 = min(t1, (-b + sq) / a);
        closest = sqrt(max(dot(o, o) - b * b / a, 0.0)) / r;
    } else if closest >= 1.0 {
        return 0.0;
    }
    if abs(dir.y) > 1e-5 {
        let ta = (y0 - eye.y) / dir.y;
        let tb = (y1 - eye.y) / dir.y;
        t0 = max(t0, min(ta, tb));
        t1 = min(t1, max(ta, tb));
    } else if eye.y < y0 || eye.y > y1 {
        return 0.0;
    }
    return max(t1 - t0, 0.0) * (1.0 - closest * closest);
}

// Light scattered toward the eye by the shaft and its mist halo, pre-exposure.
fn realm_shaft_air(eye: vec3<f32>, dir: vec3<f32>, dist: f32) -> vec3<f32> {
    let s = light.realm[9];
    if s.w <= 0.0 { return vec3<f32>(0.0); }
    let y = light.realm[10];
    let core = realm_cylinder_path(eye, dir, dist, s.xy, s.z, y.x, y.y);
    let halo = realm_cylinder_path(eye, dir, dist, s.xy, s.z * 3.2, y.x - 3.0, y.y);
    return realm_shaft_color() * s.w * (core * 0.26 + halo * 0.03);
}

// Realm air, shaft, and the glowing water seen by a submerged eye, applied
// to a finished surface colour (pre-exposure scale `gain`).
fn realm_air(lit: vec3<f32>, eye: vec3<f32>, dir: vec3<f32>, dist: f32, gain: f32) -> vec3<f32> {
    let air_t = 1.0 - exp(-dist / max(light.realm[2].w, 1.0));
    var out = mix(lit, light.realm[2].rgb * gain, air_t);
    let pool = realm_pool_at(eye);
    if uw_active() && pool >= 0 {
        // Clear glowing water: reds drain first and distance glows the pool colour.
        let b = light.realm[4u + u32(pool) * 2u];
        let tint = realm_pool_tint(b.z) * b.y;
        let trans = exp(-REALM_WATER_ABSORB * 0.8 * dist);
        out = out * trans + tint * 0.16 * gain * (vec3<f32>(1.0) - trans);
    }
    return out + realm_shaft_air(eye, dir, dist) * gain;
}
