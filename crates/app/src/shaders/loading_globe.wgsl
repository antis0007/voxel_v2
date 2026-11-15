// Loading-screen globe: one orthographic sphere shaded per pixel from six
// cube-face textures (albedo, height in km). Composed after color_shared.wgsl.
//
// Faces follow engine_core::cube_sphere: Face::ALL order, Tangent projection,
// texel (i, j) at face coordinates (-1 + 2i/(n-1), -1 + 2j/(n-1)).
//
// Light: single scattering through a Rayleigh + Mie + ozone shell. Sun
// transmittance at any point uses the Chapman grazing-incidence function, so
// the terminator reddens, the planet's shadow falls across its own air and the
// limb thins into a blue line without a precomputed table.

struct Globe {
    // Rows of the planet-to-view rotation; view x right, y up, z to the eye.
    // view_x.w: sub-texel grain strength (0 while crust moves under it).
    // view_y.w: plate-motion blend between the two history frames, or < 0
    // when the maps simply crossfade.
    view_x: vec4<f32>,
    view_y: vec4<f32>,
    view_z: vec4<f32>,
    // xyz: sun direction in view space; w: seconds.
    sun: vec4<f32>,
    // Centre in framebuffer pixels (y down), radius in pixels, relief exaggeration.
    disc: vec4<f32>,
    // Texels per face edge (current, previous), crossfade weight of current, radius in km.
    maps: vec4<f32>,
    // Cloud cover, cloud rotation, reserved progress values.
    sky: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globe;
@group(0) @binding(1) var current: texture_2d_array<f32>;
@group(0) @binding(2) var previous: texture_2d_array<f32>;
@group(0) @binding(3) var linear_clamp: sampler;
// r: sea ice cover; gba: seam emission as a fraction of GLOW_E.
@group(0) @binding(4) var current_ice: texture_2d_array<f32>;
@group(0) @binding(5) var previous_ice: texture_2d_array<f32>;
// Newer history frame: owning plate per texel, and per plate the scaled-axis
// Euler rotation (radians) that carried its crust from the older frame.
@group(0) @binding(6) var plate_ids: texture_2d_array<u32>;
@group(0) @binding(7) var<uniform> plate_axes: array<vec4<f32>, 64>;
// Newer history frame: margin colour (rgb) and strength (a) on both sides of
// each plate boundary, for the screen-resolution seam line.
@group(0) @binding(8) var seam_tint: texture_2d_array<f32>;

// Per-pixel plate carry, set once in fs_main by globe_carry: lookups into
// the newer map turn by carry_now, into the older map by carry_before.
var<private> carry_now: vec3<f32>;
var<private> carry_before: vec3<f32>;

const GLOBE_PI: f32 = 3.14159265;
// Atmosphere shell top as a fraction of the radius. Scale heights are about
// twice Earth's relative to the radius so the limb reads at screen size; the
// air is a little clearer than Earth's so the terrain map stays legible.
const SHELL: f32 = 0.018;
const H_R: f32 = 0.0026;
const H_M: f32 = 0.0008;
// Vertical optical depth at sea level: Rayleigh (680/550/440 nm, 0.6 of
// Earth's), ozone (Chappuis band, carried on the Rayleigh profile) and Mie.
const TAU_R: vec3<f32> = vec3<f32>(0.028, 0.065, 0.159);
const TAU_O: vec3<f32> = vec3<f32>(0.0098, 0.028, 0.0013);
const TAU_M: f32 = 0.02;
const MIE_EXTINCTION: f32 = 1.11;
const MIE_G: f32 = 0.76;
// Solar irradiance. Lambert ground then shows albedo * 1.35 at the subsolar point.
const SUN_E: vec3<f32> = vec3<f32>(4.24, 4.07, 3.82);
const VIEW_STEPS: i32 = 12;
// Peak seam emission: a lit line on the night side, a fine accent by day.
const GLOW_E: f32 = 0.6;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

// Face index and face coordinates of a unit direction.
fn globe_face(d: vec3<f32>) -> vec3<f32> {
    let a = abs(d);
    var layer = 0.0;
    var u = vec3<f32>(0.0, -1.0, 0.0);
    var n = vec3<f32>(1.0, 0.0, 0.0);
    var v = vec3<f32>(0.0, 0.0, 1.0);
    if a.x >= a.y && a.x >= a.z {
        if d.x < 0.0 {
            layer = 1.0;
            u = vec3<f32>(0.0, 1.0, 0.0);
            n = vec3<f32>(-1.0, 0.0, 0.0);
        }
    } else if a.y >= a.z {
        u = vec3<f32>(1.0, 0.0, 0.0);
        if d.y >= 0.0 {
            layer = 2.0;
            n = vec3<f32>(0.0, 1.0, 0.0);
        } else {
            layer = 3.0;
            n = vec3<f32>(0.0, -1.0, 0.0);
            v = vec3<f32>(0.0, 0.0, -1.0);
        }
    } else {
        u = vec3<f32>(1.0, 0.0, 0.0);
        if d.z >= 0.0 {
            layer = 4.0;
            n = vec3<f32>(0.0, 0.0, 1.0);
            v = vec3<f32>(0.0, -1.0, 0.0);
        } else {
            layer = 5.0;
            n = vec3<f32>(0.0, 0.0, -1.0);
            v = vec3<f32>(0.0, 1.0, 0.0);
        }
    }
    let dn = dot(d, n);
    let s = atan(dot(d, u) / dn) / (GLOBE_PI * 0.25);
    let t = atan(dot(d, v) / dn) / (GLOBE_PI * 0.25);
    return vec3<f32>(layer, clamp(s, -1.0, 1.0), clamp(t, -1.0, 1.0));
}

fn globe_uv(st: vec2<f32>, n: f32) -> vec2<f32> {
    return ((st + 1.0) * 0.5 * (n - 1.0) + 0.5) / n;
}

// Rodrigues rotation of v by a scaled axis (angle = length).
fn globe_rotate(v: vec3<f32>, axis: vec3<f32>) -> vec3<f32> {
    let a = length(axis);
    if a < 1e-7 {
        return v;
    }
    let k = axis / a;
    let c = cos(a);
    return v * c + cross(k, v) * sin(a) + k * dot(k, v) * (1.0 - c);
}

fn globe_plate(d: vec3<f32>) -> u32 {
    let f = globe_face(d);
    let n = max(g.maps.x, 2.0);
    let ij = clamp(vec2<i32>(round((f.yz + 1.0) * 0.5 * (n - 1.0))), vec2<i32>(0), vec2<i32>(i32(n) - 1));
    return min(textureLoad(plate_ids, ij, i32(f.x), 0).r, 63u);
}

// Crust now at d sits on plate k at R_k^(1-b) d in the newer frame and at
// R_k^(-b) d in the older one. Find the plate whose crust lands here with a
// short fixed-point search over the newer frame's ownership.
fn globe_carry(d: vec3<f32>) {
    carry_now = vec3<f32>(0.0);
    carry_before = vec3<f32>(0.0);
    let b = g.view_y.w;
    if b < 0.0 {
        return;
    }
    var plate = globe_plate(d);
    for (var i = 0; i < 3; i = i + 1) {
        let owner = globe_plate(globe_rotate(d, plate_axes[plate].xyz * (1.0 - b)));
        if owner == plate {
            break;
        }
        plate = owner;
    }
    let axis = plate_axes[plate].xyz;
    carry_now = axis * (1.0 - b);
    carry_before = axis * -b;
}

// Plate seam of the newer frame, carried with the crust, as a line of fixed
// screen width: rgb = margin colour scaled by strength, zero off the line.
// The four plate ids around the carried point give a 0/1 "same plate" field;
// its bilinear 0.5 contour is the boundary, and the field's gradient turns
// the distance to it into texels, then pixels, with no derivatives (this runs
// in non-uniform control flow).
fn globe_seam_line(d: vec3<f32>, texel_px: f32) -> vec3<f32> {
    if g.view_y.w < 0.0 {
        return vec3<f32>(0.0);
    }
    let f = globe_face(globe_rotate(d, carry_now));
    let n = max(g.maps.x, 2.0);
    let q = (f.yz + 1.0) * 0.5 * (n - 1.0);
    let hi = vec2<i32>(i32(n) - 1);
    let i0 = clamp(vec2<i32>(floor(q)), vec2<i32>(0), hi);
    let i1 = min(i0 + vec2<i32>(1), hi);
    let fr = clamp(q - vec2<f32>(i0), vec2<f32>(0.0), vec2<f32>(1.0));
    let layer = i32(f.x);
    let p00 = textureLoad(plate_ids, i0, layer, 0).r;
    let p10 = textureLoad(plate_ids, vec2<i32>(i1.x, i0.y), layer, 0).r;
    let p01 = textureLoad(plate_ids, vec2<i32>(i0.x, i1.y), layer, 0).r;
    let p11 = textureLoad(plate_ids, i1, layer, 0).r;
    if p00 == p10 && p00 == p01 && p00 == p11 {
        return vec3<f32>(0.0);
    }
    // Own plate: the nearest corner's.
    let own = select(select(p00, p10, fr.x >= 0.5), select(p01, p11, fr.x >= 0.5), fr.y >= 0.5);
    let c = vec4<f32>(select(0.0, 1.0, p00 == own), select(0.0, 1.0, p10 == own),
        select(0.0, 1.0, p01 == own), select(0.0, 1.0, p11 == own));
    let s = mix(mix(c.x, c.y, fr.x), mix(c.z, c.w, fr.x), fr.y);
    let grad = vec2<f32>(mix(c.y - c.x, c.w - c.z, fr.y), mix(c.z - c.x, c.w - c.y, fr.x));
    let dist_px = abs(s - 0.5) / max(length(grad), 0.25) * texel_px;
    // About 1.6 px across, never wider than a third of a texel.
    let half_px = min(0.8, texel_px * 0.17);
    let line = 1.0 - smoothstep(half_px * 0.5, half_px + 0.6, dist_px);
    let t = textureSampleLevel(seam_tint, linear_clamp, globe_uv(f.yz, n), layer, 0.0);
    return t.rgb * t.a * line;
}

// Albedo and height at a planet direction, crossfading two published maps.
fn globe_texel(d: vec3<f32>) -> vec4<f32> {
    let f = globe_face(globe_rotate(d, carry_now));
    let layer = i32(f.x);
    let now = textureSampleLevel(current, linear_clamp, globe_uv(f.yz, g.maps.x), layer, 0.0);
    if g.maps.z >= 1.0 {
        return now;
    }
    let fb = globe_face(globe_rotate(d, carry_before));
    let before = textureSampleLevel(previous, linear_clamp, globe_uv(fb.yz, g.maps.y), i32(fb.x), 0.0);
    return mix(before, now, g.maps.z);
}

fn globe_ice(d: vec3<f32>) -> f32 {
    let f = globe_face(globe_rotate(d, carry_now));
    let layer = i32(f.x);
    let now = textureSampleLevel(current_ice, linear_clamp, globe_uv(f.yz, g.maps.x), layer, 0.0).r;
    if g.maps.z >= 1.0 {
        return now;
    }
    let fb = globe_face(globe_rotate(d, carry_before));
    let before = textureSampleLevel(previous_ice, linear_clamp, globe_uv(fb.yz, g.maps.y), i32(fb.x), 0.0).r;
    return mix(before, now, g.maps.z);
}

fn globe_glow(d: vec3<f32>) -> vec3<f32> {
    let f = globe_face(globe_rotate(d, carry_now));
    let layer = i32(f.x);
    let now = textureSampleLevel(current_ice, linear_clamp, globe_uv(f.yz, g.maps.x), layer, 0.0).gba;
    if g.maps.z >= 1.0 {
        return now;
    }
    let fb = globe_face(globe_rotate(d, carry_before));
    let before = textureSampleLevel(previous_ice, linear_clamp, globe_uv(fb.yz, g.maps.y), i32(fb.x), 0.0).gba;
    return mix(before, now, g.maps.z);
}

fn globe_view(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(dot(g.view_x.xyz, p), dot(g.view_y.xyz, p), dot(g.view_z.xyz, p));
}

fn globe_planet(v: vec3<f32>) -> vec3<f32> {
    return g.view_x.xyz * v.x + g.view_y.xyz * v.y + g.view_z.xyz * v.z;
}

fn globe_hash(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yxz + 33.33);
    return fract((r.x + r.y) * r.z);
}

// Three decorrelated values per lattice point from one hash.
fn globe_hash3(p: vec3<f32>) -> vec3<f32> {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q = q + dot(q, q.yxz + 33.33);
    return fract((q.xxy + q.yxx) * q.zyx);
}

fn globe_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    let w = f * f * (3.0 - 2.0 * f);
    let x00 = mix(globe_hash(i), globe_hash(i + vec3<f32>(1.0, 0.0, 0.0)), w.x);
    let x10 = mix(globe_hash(i + vec3<f32>(0.0, 1.0, 0.0)), globe_hash(i + vec3<f32>(1.0, 1.0, 0.0)), w.x);
    let x01 = mix(globe_hash(i + vec3<f32>(0.0, 0.0, 1.0)), globe_hash(i + vec3<f32>(1.0, 0.0, 1.0)), w.x);
    let x11 = mix(globe_hash(i + vec3<f32>(0.0, 1.0, 1.0)), globe_hash(i + vec3<f32>(1.0, 1.0, 1.0)), w.x);
    return mix(mix(x00, x10, w.y), mix(x01, x11, w.y), w.z);
}

fn globe_noise3(p: vec3<f32>) -> vec3<f32> {
    let i = floor(p);
    let f = p - i;
    let w = f * f * (3.0 - 2.0 * f);
    let x00 = mix(globe_hash3(i), globe_hash3(i + vec3<f32>(1.0, 0.0, 0.0)), w.x);
    let x10 = mix(globe_hash3(i + vec3<f32>(0.0, 1.0, 0.0)), globe_hash3(i + vec3<f32>(1.0, 1.0, 0.0)), w.x);
    let x01 = mix(globe_hash3(i + vec3<f32>(0.0, 0.0, 1.0)), globe_hash3(i + vec3<f32>(1.0, 0.0, 1.0)), w.x);
    let x11 = mix(globe_hash3(i + vec3<f32>(0.0, 1.0, 1.0)), globe_hash3(i + vec3<f32>(1.0, 1.0, 1.0)), w.x);
    return mix(mix(x00, x10, w.y), mix(x01, x11, w.y), w.z);
}

// History frames are published at a coarse pitch. Bilinear coasts then
// follow texel rows, so warp the lookup by a static fBm sized to that pitch.
// Coastlines break into bays and headlands (fractal, D ~ 1.3); a fine final
// map has a small pitch and the warp shrinks below a texel.
fn globe_warp(d: vec3<f32>) -> vec3<f32> {
    let cell = GLOBE_PI * 0.5 / max(g.maps.x, 2.0);
    var w = vec3<f32>(0.0);
    var amp = 0.9 * cell;
    var x = d / (3.0 * cell);
    for (var o = 0; o < 4; o = o + 1) {
        w = w + (globe_noise3(x + 3.1) - 0.5) * (2.0 * amp);
        x = x * 2.07 + 5.3;
        amp = amp * 0.616;
    }
    return normalize(d + w - d * dot(w, d));
}

// 1 on an unsurveyed texel (dark, matte, height 0; see surface::UNSURVEYED).
// Real ocean is deep or brighter, land is brighter, so neither matches.
fn globe_unseen(t: vec4<f32>) -> f32 {
    let lum = dot(t.xyz, vec3<f32>(0.3, 0.5, 0.2));
    return (1.0 - smoothstep(0.0, 0.25, abs(t.w))) * (1.0 - smoothstep(0.02, 0.06, lum));
}

// Sparse weather: banded by latitude, sheared into fronts, never a uniform veil.
fn globe_clouds(d: vec3<f32>) -> f32 {
    if g.sky.x <= 0.0 {
        return 0.0;
    }
    let c = cos(g.sky.y);
    let s = sin(g.sky.y);
    let q = vec3<f32>(d.x * c + d.z * s, d.y, -d.x * s + d.z * c);
    let warp = globe_noise3(q * 2.1 + 7.0) - 0.5;
    let p = (q + warp * 0.35) * vec3<f32>(3.2, 6.5, 3.2);
    var sum = 0.0;
    var amp = 0.55;
    var x = p;
    for (var o = 0; o < 4; o = o + 1) {
        sum = sum + globe_noise(x) * amp;
        x = x * 2.03 + 11.7;
        amp = amp * 0.5;
    }
    let band = 0.55 + 0.45 * cos(d.y * 7.0);
    let cover = sum * band;
    return smoothstep(0.62 - 0.25 * g.sky.x, 0.86 - 0.2 * g.sky.x, cover) * 0.85;
}

// Chapman function Ch(x, mu) for mu >= 0, x = r / H (Schueler's approximation):
// air column along a slant ray in units of the vertical column above the point.
fn globe_chapman(x: f32, mu: f32) -> f32 {
    let c = sqrt(GLOBE_PI * 0.5 * x);
    return c / ((c - 1.0) * mu + 1.0);
}

// Column of one exponential layer from radius r toward a direction at cosine
// mu from the zenith, in units of the sea-level vertical column.
fn globe_column(r: f32, mu: f32, h: f32) -> f32 {
    let x = r / h;
    let here = exp(-(r - 1.0) / h);
    if mu >= 0.0 {
        return here * globe_chapman(x, mu);
    }
    // Below the horizon: twice the full grazing column through the tangent
    // point, less the part behind the observer.
    let r0 = max(r * sqrt(max(1.0 - mu * mu, 0.0)), 1.0);
    return 2.0 * exp(-(r0 - 1.0) / h) * sqrt(GLOBE_PI * 0.5 * r0 / h) - here * globe_chapman(x, -mu);
}

// Sun light arriving at view-space point `x` (|x| >= 1): air extinction plus
// the planet's own shadow, softened over a few hundred metres of radius.
fn globe_sun_at(x: vec3<f32>, sun: vec3<f32>) -> vec3<f32> {
    let r = max(length(x), 1.0);
    let mu = dot(x, sun) / r;
    let reach = r * sqrt(max(1.0 - mu * mu, 0.0));
    let lit = select(1.0, smoothstep(0.997, 1.003, reach), mu < 0.0);
    if lit <= 0.0 {
        return vec3<f32>(0.0);
    }
    let depth = (TAU_R + TAU_O) * globe_column(r, mu, H_R)
        + vec3<f32>(TAU_M * MIE_EXTINCTION * globe_column(r, mu, H_M));
    return exp(-depth) * lit;
}

struct Air {
    // Light scattered toward the eye.
    inscatter: vec3<f32>,
    // Transmittance from the ray's far end (surface or space) to the eye.
    transmit: vec3<f32>,
};

// Orthographic view ray through the shell at disc point `p`. Samples crowd
// toward the lowest point of the ray (the surface, or the closest approach on
// a limb ray), where the exponential air is densest.
fn globe_air(p: vec2<f32>, far: f32, near: f32, hit: bool, sun: vec3<f32>) -> Air {
    let mu = -sun.z;
    let phase_r = 3.0 / (16.0 * GLOBE_PI) * (1.0 + mu * mu);
    let g2 = MIE_G * MIE_G;
    let phase_m = 3.0 / (8.0 * GLOBE_PI) * (1.0 - g2) * (1.0 + mu * mu)
        / ((2.0 + g2) * pow(1.0 + g2 - 2.0 * MIE_G * mu, 1.5));
    let scatter_r = TAU_R / H_R * phase_r;
    let scatter_m = TAU_M / H_M * phase_m;
    let extinct_r = (TAU_R + TAU_O) / H_R;
    let extinct_m = TAU_M * MIE_EXTINCTION / H_M;
    let bottom = select(0.0, near, hit);
    let span = far - bottom;
    let count = select(2 * VIEW_STEPS, VIEW_STEPS, hit);
    var t = vec3<f32>(1.0);
    var light = vec3<f32>(0.0);
    for (var i = 0; i < count; i = i + 1) {
        // Front half (eye side) runs down to the low point; a limb ray then
        // climbs out through the back half.
        let front = i < VIEW_STEPS;
        let k = f32(select(i - VIEW_STEPS, VIEW_STEPS - 1 - i, front));
        let u = (k + 0.5) / f32(VIEW_STEPS);
        let z = select(-span * u * u, bottom + span * u * u, front);
        let ds = 2.0 * u * span / f32(VIEW_STEPS);
        let x = vec3<f32>(p, z);
        let alt = max(length(x) - 1.0, 0.0);
        let rho_r = exp(-alt / H_R);
        let rho_m = exp(-alt / H_M);
        let step_t = exp(-(extinct_r * rho_r + extinct_m * rho_m) * ds);
        // Integrate across the step against the mean transmittance.
        let source = (scatter_r * rho_r + scatter_m * rho_m) * globe_sun_at(x, sun);
        light = light + t * source * ds * (0.5 + 0.5 * step_t);
        t = t * step_t;
    }
    return Air(light * SUN_E, t);
}

// Ambient sky light on the ground: blue, fading through twilight.
fn globe_skylight(mu: f32, sun_t: vec3<f32>) -> vec3<f32> {
    let day = smoothstep(-0.15, 0.3, mu);
    return SUN_E * vec3<f32>(0.060, 0.085, 0.120) * day * (0.35 + 0.65 * sun_t);
}

// GGX sun reflection off open water, one roughness lobe.
fn globe_glint(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, alpha: f32) -> f32 {
    let h = normalize(l + v);
    let nl = max(dot(n, l), 0.0);
    let nv = max(dot(n, v), 1e-3);
    let nh = max(dot(n, h), 0.0);
    let a2 = alpha * alpha;
    let den = nh * nh * (a2 - 1.0) + 1.0;
    let dist = a2 / (GLOBE_PI * den * den);
    let vis = 0.5 / (nl * sqrt(nv * nv * (1.0 - a2) + a2) + nv * sqrt(nl * nl * (1.0 - a2) + a2) + 1e-4);
    let fresnel = 0.02 + 0.98 * pow(1.0 - max(dot(h, v), 0.0), 5.0);
    return dist * vis * fresnel * nl;
}

fn globe_tonemap(c: vec3<f32>) -> vec3<f32> {
    // Soft shoulder: keeps glint and snow below white without flattening the land.
    return srgb_encode(1.0 - exp(-c * 1.15));
}

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let radius = g.disc.z;
    let p = vec2<f32>(frag.x - g.disc.x, g.disc.y - frag.y) / radius;
    let b = length(p);
    let outer = 1.0 + SHELL;
    // Everything below is per-pixel work; the corners of the callback rect
    // and the space beyond the shell skip it entirely.
    if b >= outer {
        discard;
    }
    let near = sqrt(max(1.0 - b * b, 0.0));
    let far = sqrt(max(outer * outer - b * b, 0.0));
    let hit = b < 1.0;
    let sun = normalize(g.sun.xyz);
    let eye = vec3<f32>(0.0, 0.0, 1.0);
    let air = globe_air(p, far, near, hit, sun);

    var surface = vec3<f32>(0.0);
    if hit {
        let n_view = vec3<f32>(p, near);
        let d = normalize(globe_planet(n_view));
        globe_carry(d);
        // Surface lookups use the warped direction; light and clouds keep `d`.
        // Where a face is not surveyed yet (one-face previews, flat worlds) the
        // warp would ripple the edge of the surveyed patch, so it fades out there.
        let dw = globe_warp(d);
        let unseen = max(globe_unseen(globe_texel(d)), globe_unseen(globe_texel(dw)));
        let ds = normalize(mix(dw, d, unseen));
        let texel = globe_texel(ds);
        let raw_albedo = texel.xyz;
        let h = texel.w;

        // Relief normal from cross-face height differences along two tangents.
        let helper = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(d.y) > 0.9);
        let t1 = normalize(cross(helper, d));
        let t2 = cross(d, t1);
        let eps = 0.85 * GLOBE_PI * 0.5 / max(g.maps.x, 2.0);
        let hx0 = globe_texel(normalize(ds - t1 * eps)).w;
        let hx1 = globe_texel(normalize(ds + t1 * eps)).w;
        let hy0 = globe_texel(normalize(ds - t2 * eps)).w;
        let hy1 = globe_texel(normalize(ds + t2 * eps)).w;
        let gx = max(hx1, 0.0) - max(hx0, 0.0);
        let gy = max(hy1, 0.0) - max(hy0, 0.0);
        let k = g.disc.w / (2.0 * eps * g.maps.w);
        // Sub-texel detail: micro-relief and albedo grain below the map pitch,
        // stronger on steep ground, fading out once a texel is under a few
        // pixels so a small globe never shimmers.
        let texel_px = eps / 0.85 * radius * max(near, 0.12);
        let detail = smoothstep(2.0, 6.0, texel_px) * clamp(g.view_x.w, 0.0, 1.0);
        let grain = globe_noise3(d * (5.0 / eps)) - 0.5 + 0.5 * (globe_noise3(d * (11.0 / eps) + 9.1) - 0.5);
        let rough = (0.3 + clamp(length(vec2<f32>(gx, gy)) * 5.0, 0.0, 1.0)) * detail;
        let bump = (t1 * grain.x + t2 * grain.y) * 0.45 * rough;
        let n_land = normalize(globe_view(normalize(d - (t1 * gx + t2 * gy) * k - bump)));
        // History endpoints bake their seams into the albedo and emission.
        // Refine the moving contour at screen resolution within the interval.
        // The detailed boundary belongs to the incoming ownership map.
        // Ease it in and out so replacing that map cannot snap the contour.
        let phase = g.view_y.w;
        let detail_weight = smoothstep(0.0, 0.30, phase) * (1.0 - smoothstep(0.70, 1.0, phase));
        let seam_line = globe_seam_line(ds, texel_px) * detail_weight;
        let seam_line_peak = max(seam_line.r, max(seam_line.g, seam_line.b));
        let seam_albedo = seam_line / max(seam_line_peak, 1e-3) * 0.8;
        let albedo = mix(raw_albedo * (1.0 + grain.z * 0.22 * detail * step(0.0, h)), seam_albedo,
            0.45 * min(seam_line_peak * 1.4, 1.0));

        // Sea-level crossing filtered at screen resolution: the signed height
        // change across one pixel, from the same taps (foreshortened toward
        // the limb), so the coast neither blurs nor aliases as the globe turns.
        let slope = length(vec2<f32>(hx1 - hx0, hy1 - hy0)) / (2.0 * eps);
        let footprint = 1.0 / (radius * max(near, 0.12));
        let coast = max(0.00012, 0.65 * slope * footprint);
        let land = smoothstep(-coast, coast, h);

        let mu = dot(n_view, sun);
        let sun_t = globe_sun_at(n_view * 1.0002, sun);
        let sky = globe_skylight(mu, sun_t);
        let direct = SUN_E * sun_t / GLOBE_PI;

        // Land: Lambert on the relief normal; the terminator mask stops relief
        // from catching light that the sphere itself cannot receive.
        let lambert = max(dot(n_land, sun), 0.0) * smoothstep(-0.04, 0.10, mu);
        let ground = albedo * (direct * lambert + sky / GLOBE_PI + 0.012);

        // Water: body colour under a Fresnel sky reflection and a broad sunglint;
        // at globe scale the sea is rough, so there is no hot calm-water core.
        let fresnel = 0.02 + 0.98 * pow(1.0 - near, 5.0);
        // Seafloor relief read through the water, as on a bathymetric chart:
        // ridges catch light on their sunward flanks, trenches fall dark.
        let sgx = min(hx1, 0.0) - min(hx0, 0.0);
        let sgy = min(hy1, 0.0) - min(hy0, 0.0);
        let n_floor = normalize(globe_view(normalize(d - (t1 * sgx + t2 * sgy) * k * 0.35)));
        let floor_light = clamp(dot(n_floor, sun) / max(mu, 0.08), 0.55, 1.45);
        let floor_shade = mix(1.0, floor_light, 0.45 * smoothstep(0.0, 0.15, mu));
        let body = albedo * floor_shade * 1.2 * (direct * max(mu, 0.0) + sky / GLOBE_PI) * (1.0 - fresnel);
        let reflect_sky = vec3<f32>(0.10, 0.17, 0.30) * smoothstep(-0.12, 0.25, mu) * fresnel;
        // Wave glitter: the glint breaks into brighter and darker sea patches.
        let glitter = 0.55 + 0.9 * globe_noise(d * (9.0 / eps) + 3.3);
        let glint = 0.6 * globe_glint(n_view, eye, sun, 0.22) * glitter;
        let sea = body + reflect_sky + SUN_E * sun_t * glint + albedo * 0.003;
        // Pack ice: matte, lit like snow on flat ground.
        let ice = albedo * (direct * max(mu, 0.0) + sky / GLOBE_PI + 0.012);
        // Pack ice keeps a hard, broken floe edge instead of a bilinear smear.
        let floes = globe_ice(ds) + (globe_noise(d * (3.0 / eps)) - 0.5) * 0.35;
        surface = mix(mix(sea, ice, smoothstep(0.35, 0.6, floes)), ground, land);
        // Plate seams glow: faint against daylight, a lit line across the night.
        let night = 1.0 - smoothstep(-0.08, 0.22, mu);
        // The bilinear tail of a one-texel line is cut back so the seam stays thin.
        let seam = globe_glow(ds);
        let seam_peak = max(seam.r, max(seam.g, seam.b));
        surface += (seam * smoothstep(0.1, 0.45, seam_peak) * (1.0 - detail_weight) + seam_line) * GLOW_E * (0.25 + 0.75 * night);

        // Cloud shell: lit tops over a soft cast shadow offset toward the sun.
        let cover = globe_clouds(d);
        if cover > 0.0 {
            let shadow = globe_clouds(normalize(d + globe_planet(sun) * 0.012));
            surface = surface * (1.0 - 0.45 * shadow);
            let cloud_lit = 0.8 * direct * max(mu, 0.0) * 1.3 + sky / GLOBE_PI + vec3<f32>(0.004);
            surface = mix(surface, cloud_lit, cover);
        }
    }

    // Work progress belongs to the adjacent stage display. An illuminated
    // scan stripe across the surface reads as a geographic seam.
    // One-pixel edge coverage keeps the limb smooth at any size.
    let disc = clamp((1.0 - b) * radius + 0.5, 0.0, 1.0);
    let radiance = surface * air.transmit * disc + air.inscatter;
    // Half an 8-bit step of dither: the air gradients would otherwise band.
    let dither = (globe_hash(vec3<f32>(frag.xy, 7.0)) - 0.5) / 255.0;
    let color = max(globe_tonemap(radiance) + dither, vec3<f32>(0.0));
    // Premultiplied: stars behind the shell dim by the air's transmittance.
    let veil = 1.0 - dot(air.transmit, vec3<f32>(0.3, 0.4, 0.3));
    return vec4<f32>(color, mix(clamp(veil, 0.0, 1.0), 1.0, disc));
}

// ---------------------------------------------------------------------------
// Backdrop: deep space behind the globe, drawn by a second pipeline over the
// whole screen before the globe. It reads only its own uniform (group 1), so
// it draws before the first surface map has arrived.

struct Backdrop {
    // Globe centre in framebuffer pixels (y down), radius in pixels, seconds.
    frame: vec4<f32>,
    // Sun direction in the globe's view space, then 1 while stars may twinkle.
    light: vec4<f32>,
    // Framebuffer size in pixels, pixels per point, unused.
    screen: vec4<f32>,
};

@group(1) @binding(0) var<uniform> bd: Backdrop;

// One layer of stars on a jittered grid: few bright, many faint (power-law
// magnitudes), colour from blue-white to orange by a temperature hash.
fn backdrop_stars(px: vec2<f32>, cell: f32, density: f32, seed: f32) -> vec3<f32> {
    let q = px / cell;
    let i = floor(q);
    let h = globe_hash3(vec3<f32>(i, seed));
    if h.x > density {
        return vec3<f32>(0.0);
    }
    let centre = (i + 0.2 + 0.6 * h.yz) * cell;
    let mag = pow(globe_hash(vec3<f32>(i, seed + 3.0)), 7.0);
    let size = 0.55 + 1.3 * mag;
    let r2 = dot(px - centre, px - centre) / (size * size);
    let temp = globe_hash(vec3<f32>(i, seed + 7.0));
    let tint = mix(vec3<f32>(0.62, 0.74, 1.0), vec3<f32>(1.0, 0.80, 0.58), temp * temp);
    let twinkle = 1.0 + 0.3 * bd.light.w * sin(bd.frame.w * (1.3 + 3.0 * h.y) + h.z * 40.0);
    // Core plus a faint wide skirt, as a lens spreads a point source.
    return tint * (exp(-r2 * 1.6) + 0.04 * exp(-r2 * 0.12)) * (0.05 + 2.6 * mag) * twinkle;
}

fn backdrop_fbm(p: vec3<f32>) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var x = p;
    for (var o = 0; o < 3; o = o + 1) {
        sum = sum + globe_noise(x) * amp;
        x = x * 2.13 + 4.7;
        amp = amp * 0.5;
    }
    return sum / 0.875;
}

@fragment
fn fs_backdrop(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let size = bd.screen.xy;
    // Resolution-independent units: one unit is a 900-pixel-tall screen.
    let scale = size.y / 900.0;
    let px = frag.xy / scale;
    let uv = (frag.xy - 0.5 * size) / size.y;

    // Galactic band: a tilted strip of unresolved stars with dust lanes.
    let normal = normalize(vec2<f32>(0.42, 1.0));
    let across = dot(uv, normal) - 0.08;
    let along = dot(uv, vec2<f32>(normal.y, -normal.x));
    let band = exp(-across * across / 0.018);
    let glow = backdrop_fbm(vec3<f32>(uv * 3.2, 1.7));
    let dust = smoothstep(0.42, 0.72, backdrop_fbm(vec3<f32>(uv * 7.5 + vec2<f32>(along * 2.0, 0.0), 5.3)));
    let milky = band * (0.35 + 0.65 * glow) * (1.0 - 0.75 * dust * band);
    var sky = vec3<f32>(0.0035, 0.0045, 0.0085)
        + milky * mix(vec3<f32>(0.020, 0.024, 0.040), vec3<f32>(0.046, 0.038, 0.030), band);

    // Stars, denser along the band.
    let crowd = 0.5 + 0.9 * band;
    sky = sky + backdrop_stars(px, 9.0, 0.10 * crowd, 1.0) * 0.6;
    sky = sky + backdrop_stars(px, 23.0, 0.35 * crowd, 2.0);
    sky = sky + backdrop_stars(px + 11.0, 61.0, 0.45, 3.0);

    // Light scattered off the lit air and glare around the limb: blue, wide
    // and stronger on the day side. The globe itself covers the disc.
    let to = (frag.xy - bd.frame.xy) * vec2<f32>(1.0, -1.0);
    let r = length(to) / max(bd.frame.z, 1.0);
    let side = 0.25 + 0.75 * clamp(dot(normalize(to + vec2<f32>(1e-4)), normalize(bd.light.xy + vec2<f32>(1e-4))) * 0.5 + 0.5, 0.0, 1.0);
    let halo = (exp(-max(r - 1.0, 0.0) * 9.0) * 0.07 + exp(-max(r - 1.0, 0.0) * 2.2) * 0.012) * side;
    sky = sky + vec3<f32>(0.30, 0.55, 1.0) * halo * step(1.0, r);

    // Vignette toward the corners keeps the eye on the planet and the panel.
    let vignette = 1.0 - 0.45 * dot(uv, uv);
    let dither = (globe_hash(vec3<f32>(frag.xy, 3.0)) - 0.5) / 255.0;
    return vec4<f32>(max(globe_tonemap(sky * vignette) + dither, vec3<f32>(0.0)), 1.0);
}
