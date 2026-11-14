struct PortalComposite {
    view_projection: mat4x4<f32>,
    aperture: array<vec4<f32>, 4>,
    // xy: parent target size; z: seconds; w: veil strength.
    parent_size: vec4<f32>,
    clip_planes: array<vec4<f32>,20>,
    // x: active clip planes; y: 1 when the destination image is missing.
    clip_count: vec4<u32>,
};
@group(0) @binding(0) var<uniform> portal: PortalComposite;
@group(0) @binding(1) var destination: texture_2d<f32>;
@group(0) @binding(2) var destination_sampler: sampler;

// One weave thread every two world texels (PIXEL_RES = 20 texels/m).
const THREADS_PER_M: f32 = 10.0;
// The world's texel grid (`PIXEL_RES` in world_shader.wgsl); the pearl film
// and its sparks snap to it so the membrane shares the voxel pixel size.
const TEXELS_PER_M: f32 = 20.0;
// Below this share of the screen a window is not rendered
// (`views::MIN_WINDOW_SHARE`); the membrane takes over across twice that.
const MIN_WINDOW_SHARE: f32 = 0.0012;
const TAU: f32 = 6.2831853;

struct VertexOut {
    @builtin(position) position:vec4<f32>,
    @location(0) relative:vec3<f32>,
    @location(1) uv:vec2<f32>,
};
@vertex
fn vs_main(@builtin(vertex_index) index:u32)->VertexOut {
    let corner=array<u32,6>(0u,1u,2u,0u,2u,3u)[index];
    let clip=portal.view_projection*portal.aperture[corner];
    // A window closer than the near plane must stay visible until the camera
    // crosses. Clamp only aperture depth; destination geometry keeps its own
    // projection and exit-plane clipping.
    var out:VertexOut;
    out.position=vec4<f32>(clip.xy,clamp(clip.z,0.0,max(clip.w,0.0)),clip.w);
    out.relative=portal.aperture[corner].xyz;
    out.uv=array<vec2<f32>,4>(vec2<f32>(0.0,0.0),vec2<f32>(1.0,0.0),vec2<f32>(1.0,1.0),vec2<f32>(0.0,1.0))[corner];
    return out;
}

fn veil_hash(p: vec2<i32>) -> f32 {
    var h = (u32(p.x) * 0x8da6b343u) ^ (u32(p.y) * 0xd8163841u);
    h = (h ^ (h >> 15u)) * 0x2c1b3c6du;
    h = (h ^ (h >> 12u)) * 0x297a2d39u;
    return f32(h ^ (h >> 15u)) * (1.0 / 4294967296.0);
}

// Quintic value noise and its analytic C2 gradient.
fn veil_sample(p: vec2<f32>) -> vec3<f32> {
    let i = vec2<i32>(floor(p));
    let f = fract(p);
    let s = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let ds = 30.0 * f * f * (f - vec2<f32>(1.0)) * (f - vec2<f32>(1.0));
    let a = veil_hash(i);
    let b = veil_hash(i + vec2<i32>(1, 0));
    let c = veil_hash(i + vec2<i32>(0, 1));
    let d = veil_hash(i + vec2<i32>(1, 1));
    let lower = mix(a, b, s.x);
    let upper = mix(c, d, s.x);
    return vec3<f32>(mix(lower, upper, s.y), mix(b - a, d - c, s.y) * ds.x, (upper - lower) * ds.y);
}

fn veil_noise(p: vec2<f32>) -> f32 {
    return veil_sample(p).x;
}

struct Veil {
    // Surface height in [0,1] and its gradient per metre.
    height: f32,
    slope: vec2<f32>,
    // Distance to the ring in metres, and 0..1 within 45 cm of it.
    edge: f32,
    rim: f32,
    // |cos| between the view ray and the aperture normal.
    facing: f32,
    weave: f32,
    // Aperture size in metres, and the unit direction away from the nearest
    // edge in aperture axes (x right, y up).
    size: vec2<f32>,
    inward: vec2<f32>,
};

fn clip_discard(relative: vec3<f32>) {
    for (var i=0u;i<min(portal.clip_count.x,20u);i++) {
        if dot(portal.clip_planes[i],vec4<f32>(relative,1.0))<0.0 {discard;}
    }
}

/// Slow domain-warped swell in aperture metres. Motion periods are 9 to 14 s,
/// so the surface breathes rather than flickers.
fn veil_surface(uv: vec2<f32>, relative: vec3<f32>, weave_footprint: f32) -> Veil {
    let ex = portal.aperture[1].xyz - portal.aperture[0].xyz;
    let ey = portal.aperture[3].xyz - portal.aperture[0].xyz;
    let size = vec2<f32>(length(ex), length(ey));
    let p = uv * size;
    let t = portal.parent_size.z;
    let wx = veil_sample(p * 0.55 + vec2<f32>(t * 0.071, 3.1));
    let wy = veil_sample(p * 0.55 + vec2<f32>(7.7, -t * 0.083));
    let warp = vec2<f32>(wx.x, wy.x) * 1.4;
    let q = p * 1.2 + warp + vec2<f32>(0.0, t * 0.11);
    let sample = veil_sample(q);
    let h = sample.x;
    // Chain rule includes the domain warp. Analytic C2 gradients avoid the
    // directional bias and extra height evaluations of a stencil.
    let gradient = vec2<f32>(
        sample.y * (1.2 + wx.y * 0.77) + sample.z * wy.y * 0.77,
        sample.y * wx.z * 0.77 + sample.z * (1.2 + wy.z * 0.77));
    let slope = gradient / (1.0 + length(gradient) * 0.35);
    var v: Veil;
    v.height = h;
    v.slope = slope;
    v.edge = min(min(p.x, size.x - p.x), min(p.y, size.y - p.y));
    v.rim = 1.0 - smoothstep(0.0, 0.45, v.edge);
    v.size = size;
    let horizontal = min(p.x, size.x - p.x);
    let vertical = min(p.y, size.y - p.y);
    v.inward = select(vec2<f32>(0.0, select(-1.0, 1.0, p.y < size.y - p.y)),
        vec2<f32>(select(-1.0, 1.0, p.x < size.x - p.x), 0.0), horizontal < vertical);
    let n = normalize(cross(ex, ey));
    v.facing = abs(dot(relative / max(length(relative), 0.0001), n));
    // Over-under weave: threads bow with the swell; whichever family is on top
    // alternates per crossing. Faded out before a thread is under ~2 pixels.
    let w = p * THREADS_PER_M + vec2<f32>(h, -h) * 0.45;
    let warp_thread = pow(0.5 + 0.5 * cos(TAU * w.x), 6.0);
    let weft_thread = pow(0.5 + 0.5 * cos(TAU * w.y), 6.0);
    let cell = vec2<i32>(floor(w + 0.5));
    let on_top = select(warp_thread, weft_thread, ((cell.x + cell.y) & 1) == 0);
    v.weave = (max(warp_thread, weft_thread) * 0.55 + on_top * 0.45)
        * (1.0 - smoothstep(0.22, 0.5, weave_footprint * (1.0 + 0.045 * length(gradient))));
    return v;
}

/// Interference colour for a film of relative thickness `x`: rose, gold,
/// mint, sky and lilac in turn, never saturated to a primary.
fn spectral(x: f32) -> vec3<f32> {
    return 0.56 + 0.44 * cos(TAU * (vec3<f32>(0.0, 0.33, 0.67) + x));
}

/// `c` rescaled to unit luminance, so a tint changes hue and keeps brightness.
fn unit_luma(c: vec3<f32>) -> vec3<f32> {
    return c / max(dot(c, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.05);
}

/// Terrace level of the nacre field: whole numbers are plate boundaries.
fn nacre_level(q: vec2<f32>, height: f32, t: f32) -> f32 {
    return (veil_noise(q * 0.7 + vec2<f32>(t * 0.017, -t * 0.023)) * 2.6
        + veil_noise(q * 2.1 + vec2<f32>(5.3, 1.9)) * 0.5 + height * 0.7) * 2.5;
}

struct Pearl {
    film: vec3<f32>,
    // One-texel growth line where one nacre terrace meets the next.
    growth: f32,
    // 1 where an aperture texel spans over 1.5 screen pixels, 0 under one.
    resolved: f32,
    spark: f32,
};

/// Nacre on the membrane: a slow film whose thickness drifts across terraces
/// like stacked aragonite plates and walks with the viewing angle, so colour
/// rolls across the surface as the camera moves; plates show as growth lines. Sampled at aperture
/// texel centres; fades to its average once a texel is under a pixel.
fn pearl(v: Veil, uv: vec2<f32>, texels_per_pixel: f32) -> Pearl {
    let p = uv * v.size;
    let t = portal.parent_size.z;
    let resolved = 1.0 - smoothstep(0.65, 1.5, texels_per_pixel);
    let cell = floor(p * TEXELS_PER_M);
    let q = mix(p, (cell + 0.5) / TEXELS_PER_M, resolved);
    let level = nacre_level(q, v.height, t);
    let plate = floor(level);
    var o: Pearl;
    // Thickness follows the field continuously: per-plate hues stepped into
    // flat bands with hard edges that read as tearing up close.
    o.film = spectral(0.35 + 0.11 * level + 1.15 * (1.0 - v.facing) + 0.03 * sin(t * 0.21));
    // A texel-wide growth line where the texel below sits on another plate.
    let below = floor(nacre_level(q - vec2<f32>(0.0, 1.0 / TEXELS_PER_M), v.height, t));
    o.growth = select(0.0, 1.0, below != plate) * resolved;
    o.resolved = resolved;
    // Sparks: rare texels that flare in three stepped levels on their own slow
    // cycle, densest at the ring. Pure functions of texel and clock.
    let h = veil_hash(vec2<i32>(cell));
    let cycle = t * (0.35 + 0.5 * h) + h * 17.0;
    let slot = i32(floor(cycle));
    let chance = 0.0004 + 0.006 * exp(-v.edge * 4.0);
    let lit = step(1.0 - chance, veil_hash(vec2<i32>(cell) + vec2<i32>(slot * 7, 911)));
    let envelope = 1.0 - abs(fract(cycle) * 2.0 - 1.0);
    o.spark = lit * select(0.0, ceil(envelope * 3.0) / 3.0, envelope > 0.15) * resolved;
    return o;
}

// Gate kinds, matching composite.rs: 0 window, 1 mist, 2 vista, 3 realm mist.
// Crystal gates take the violet of resonant crystal, realm gates the cold blue
// of the underworld lamp.
fn gate_tint(kind: u32) -> vec3<f32> {
    if kind == 4u { return vec3<f32>(1.0, 0.22, 0.045); }
    return select(vec3<f32>(0.62, 0.48, 1.0), vec3<f32>(0.32, 0.58, 1.0), kind >= 2u);
}

/// Light held in the inside of the ring: a hairline where the membrane meets
/// the frame and a soft halo with bands creeping around the opening. The
/// bands carry the pearl's interference colours over the gate's own tint.
fn rim_light(v: Veil, uv: vec2<f32>, kind: u32) -> vec3<f32> {
    let t = portal.parent_size.z;
    let bands = max(round((v.size.x + v.size.y) * 1.4), 3.0);
    let angle = atan2(uv.y - 0.5, uv.x - 0.5);
    let phase = angle * bands - t * 0.8 + v.height * 3.0;
    let crawl = 0.5 + 0.5 * sin(phase);
    let hue = mix(gate_tint(kind), unit_luma(spectral(angle / TAU * 2.0 - t * 0.04)) * 0.62, 0.45);
    let line = exp(-v.edge / 0.025);
    let halo = exp(-v.edge / 0.3) * (0.35 + 0.65 * crawl);
    return mix(gate_tint(kind), hue, crawl) * (0.7 * line + 0.13 * halo) * portal.parent_size.w;
}

/// 1 when the eye is about to pass through the opening, 0 beyond 1.5 m.
fn crossing(v: Veil) -> f32 {
    let ex = portal.aperture[1].xyz - portal.aperture[0].xyz;
    let ey = portal.aperture[3].xyz - portal.aperture[0].xyz;
    let n = normalize(cross(ex, ey));
    let plane = abs(dot(portal.aperture[0].xyz, n));
    // Only while the eye is also inside the opening's outline.
    let local = -portal.aperture[0].xyz;
    let inside = step(0.0, dot(local, ex)) * step(dot(local, ex), dot(ex, ex))
        * step(0.0, dot(local, ey)) * step(dot(local, ey), dot(ey, ey));
    return (1.0 - smoothstep(0.15, 1.5, plane)) * inside;
}

/// The membrane as premultiplied colour and coverage, for openings with no
/// rendered window and for windows too small to be worth one.
fn membrane(v: Veil, pl: Pearl, uv: vec2<f32>, kind: u32) -> vec4<f32> {
    let glow = rim_light(v, uv, kind);
    if kind >= 2u {
        // A realm threshold seen from its far side: dark and mostly opaque,
        // with slow plumes of cold mist rising through it.
        let t = portal.parent_size.z;
        let p = uv * v.size;
        let plume = veil_noise(p * vec2<f32>(0.9, 0.45) + vec2<f32>(t * 0.05, -t * 0.16));
        let base = select(vec3<f32>(0.012, 0.02, 0.034), vec3<f32>(0.055, 0.006, 0.012), kind == 4u);
        let mist = base
            + mix(gate_tint(kind), pl.film, 0.35) * 0.05 * plume * plume * (1.0 - uv.y * 0.6);
        let alpha = clamp(0.86 + 0.1 * v.rim, 0.0, 0.97);
        return vec4<f32>(mist * alpha + glow + pl.film * pl.spark * 0.25, alpha);
    }
    // Mother of pearl: a pale milky body whose plates catch interference
    // colour, brighter edge-on, with a growth line on each plate.
    let body = vec3<f32>(0.50, 0.49, 0.56) * (0.78 + 0.22 * v.height);
    let nacre = body * mix(vec3<f32>(1.0), unit_luma(pl.film), 0.3 + 0.25 * pl.resolved);
    let alpha = clamp(0.42 + 0.3 * v.rim + 0.25 * (1.0 - v.facing), 0.0, 0.92);
    let lines = pl.film * pl.growth * 0.08;
    let spark = mix(vec3<f32>(1.0), pl.film, 0.5) * pl.spark * 0.9;
    return vec4<f32>((nacre + lines) * alpha + glow + spark, alpha);
}

/// Screen offset in pixels of a ripple that runs from the ring inward and dies
/// out within about a metre, so the middle of a large opening stays still.
fn rim_ripple(v: Veil) -> vec2<f32> {
    let wave = sin(TAU * (v.edge * 1.3 - portal.parent_size.z * 0.27)) * exp(-v.edge * 1.8);
    return vec2<f32>(v.inward.x, -v.inward.y) * wave * 2.2 * portal.parent_size.w;
}

fn veil_fresnel(v: Veil) -> f32 {
    let k = 1.0 - v.facing;
    return 0.02 + 0.98 * k * k * k * k * k;
}

// Underworld stand-in seen through a gate whose realm is not loaded. Gate
// frame: x right, y up, z into the realm, metres; the threshold floor is at
// y = -2.45. Proportions follow the owner's concept: a carved mass hanging from
// the roof over a flat plain, ridges either side, cold mist, one blue lamp.
fn under_ridges(p: vec3<f32>) -> f32 {
    let half_width = 34.0 + p.z * 0.16;
    let side = abs(p.x) - half_width;
    let crag = veil_noise(vec2<f32>(p.z * 0.045, sign(p.x) * 9.0)) * 38.0
        + veil_noise(vec2<f32>(p.z * 0.17, p.x * 0.03)) * 11.0;
    let height = max(side, 0.0) * 1.35 + crag * smoothstep(-8.0, 30.0, side) - 6.0;
    return (p.y - height) * 0.45;
}

fn under_mass(p: vec3<f32>) -> f32 {
    let c = p - vec3<f32>(0.0, 128.0, 270.0);
    let body = length(c / vec3<f32>(230.0, 95.0, 70.0)) - 1.0;
    var d = body * 65.0;
    // Two hollow sockets.
    let socket = vec3<f32>(abs(c.x) - 72.0, c.y + 8.0, c.z + 55.0);
    d = max(d, -(length(socket / vec3<f32>(38.0, 26.0, 34.0)) - 1.0) * 26.0);
    // Fluted columns hang from the lower lip, longest at the centre.
    let cell = 13.0;
    let x = c.x - cell * round(c.x / cell);
    let idx = round(c.x / cell);
    let reach = 118.0 - abs(idx) * 11.0 + veil_hash(vec2<i32>(i32(idx), 3)) * 22.0;
    let column_y = clamp(c.y, -reach, -50.0);
    let column = length(vec3<f32>(x, c.y - column_y, c.z + 30.0 - abs(idx) * 3.0)) - 4.6;
    if abs(c.x) < 105.0 { d = min(d, column); }
    return d;
}

fn under_scene(p: vec3<f32>) -> vec2<f32> {
    let plain = p.y + 2.45;
    let roof = 235.0 - p.y - veil_noise(p.xz * 0.01) * 50.0;
    var best = vec2<f32>(plain, 0.0);
    let ridge = under_ridges(p);
    if ridge < best.x { best = vec2<f32>(ridge, 1.0); }
    let mass = under_mass(p);
    if mass < best.x { best = vec2<f32>(mass, 2.0); }
    if roof < best.x { best = vec2<f32>(roof, 2.0); }
    return best;
}

fn underworld(origin: vec3<f32>, dir: vec3<f32>) -> vec3<f32> {
    let mist = vec3<f32>(0.016, 0.026, 0.04);
    var t = 0.5;
    var hit = -1.0;
    // Cone-relaxed march: the hit tolerance and minimum step grow with
    // distance, where a pixel covers metres, so 72 steps reach the far roof.
    for (var i = 0; i < 72; i++) {
        let s = under_scene(origin + dir * t);
        if s.x < 0.003 * t { hit = s.y; break; }
        t += max(s.x * 0.9, 0.025 * t + 0.05);
        if t > 1400.0 { break; }
    }
    var color = mist * 0.8;
    if hit >= 0.0 {
        let p = origin + dir * t;
        let e = vec2<f32>(0.01 * t + 0.02, 0.0);
        let n = normalize(vec3<f32>(
            under_scene(p + e.xyy).x - under_scene(p - e.xyy).x,
            under_scene(p + e.yxy).x - under_scene(p - e.yxy).x,
            under_scene(p + e.yyx).x - under_scene(p - e.yyx).x));
        // Cold light from above the far end, wrapped so carving reads.
        let light = normalize(vec3<f32>(0.5, 0.42, -0.75));
        let wrap = clamp(dot(n, light) * 0.6 + 0.4, 0.0, 1.0);
        var albedo = vec3<f32>(0.07, 0.09, 0.12);
        if hit < 0.5 {
            // Flat plain broken into plates by dark cracks.
            let q = p.xz * vec2<f32>(0.35, 0.12);
            let crack = smoothstep(0.0, 0.06, abs(veil_noise(q) - 0.5)) * smoothstep(0.0, 0.05, abs(veil_noise(q * 2.3 + 4.0) - 0.5));
            albedo = vec3<f32>(0.03, 0.04, 0.055) * (0.3 + 0.7 * crack);
        } else if hit > 1.5 {
            albedo = vec3<f32>(0.05, 0.065, 0.085);
        }
        // Pale edges on dark carving: a sharp term over a dim wrap. The plain
        // stays dark; the mist carries its light.
        let edge = select(2.4 * pow(wrap, 6.0), 0.0, hit < 0.5);
        color = albedo * (0.1 + 0.6 * wrap + edge) * vec3<f32>(0.55, 0.72, 0.95);
        // Distance mist, thicker near the plain.
        let low = exp(-max(p.y + 2.45, 0.0) * 0.03);
        let fog = 1.0 - exp(-t * (0.0011 + 0.012 * low));
        color = mix(color, mist, clamp(fog, 0.0, 0.97));
    }
    // A single lamp carried far out on the plain.
    let lamp = vec3<f32>(1.4, -1.2, 46.0) - origin;
    let along = max(dot(lamp, dir), 0.0);
    let miss = length(lamp - dir * along);
    if along < t || hit < 0.0 {
        color += vec3<f32>(0.25, 0.55, 1.0) * (exp(-miss * 9.0) * 0.9 + exp(-miss * 0.9) * 0.025);
    }
    return color;
}

/// Eye ray into the gate frame. The forward axis points away from the viewer.
fn underworld_view(relative: vec3<f32>) -> vec3<f32> {
    let c = (portal.aperture[0].xyz + portal.aperture[2].xyz) * 0.5;
    let right = normalize(portal.aperture[1].xyz - portal.aperture[0].xyz);
    let up = normalize(portal.aperture[3].xyz - portal.aperture[0].xyz);
    var forward = normalize(cross(right, up));
    if dot(forward, c) < 0.0 { forward = -forward; }
    let side = normalize(cross(up, forward));
    let to_local = transpose(mat3x3<f32>(side, up, forward));
    let ray = to_local * relative;
    let direction = select(vec3<f32>(0.0, 0.0, 1.0), ray / max(length(ray), 0.0001), dot(ray, ray) > 0.00000001);
    return underworld(to_local * (-c), direction);
}

@fragment
fn fs_main(in:VertexOut)->@location(0) vec4<f32> {
    // Derivatives before any discard.
    let metres = in.uv * vec2<f32>(length(portal.aperture[1].xyz - portal.aperture[0].xyz),
        length(portal.aperture[3].xyz - portal.aperture[0].xyz));
    let footprint = length(fwidth(metres * THREADS_PER_M));
    let texels_per_pixel = length(fwidth(metres * TEXELS_PER_M));
    let opening_pixels = 1.0 / max(fwidth(in.uv.x) * fwidth(in.uv.y), 1e-9);
    clip_discard(in.relative);
    let v = veil_surface(in.uv, in.relative, footprint);
    let pl = pearl(v, in.uv, texels_per_pixel);
    let near = crossing(v);
    let strength = portal.parent_size.w;
    // Refraction in pixels: under one at the centre facing the opening, a few
    // at the ring and at grazing angles where the membrane is most visible.
    // Stepping through it swells every term, like pushing through soap film.
    let amplitude = strength * (0.6 + 2.0 * v.rim * v.rim + 1.6 * (1.0 - v.facing) + 5.0 * near);
    let offset = (vec2<f32>(v.slope.x, -v.slope.y) * amplitude + rim_ripple(v) * (1.0 + 2.0 * near))
        / portal.parent_size.xy;
    // Screen-space sampling keeps correct parallax; quad UVs would flatten it.
    let base_uv = in.position.xy / portal.parent_size.xy + offset;
    let split = normalize(v.slope + vec2<f32>(1e-4, 0.0))
        * (strength * (0.7 * v.rim * v.rim + 2.5 * near)) / portal.parent_size.xy;
    var color: vec3<f32>;
    if portal.clip_count.y == 2u {
        // Bend the eye ray by the same swell that offsets the image samples.
        let bend = vec3<f32>(v.slope.x, v.slope.y, 0.0) * (0.0025 * amplitude * length(in.relative));
        color = underworld_view(in.relative + bend);
    } else {
        color = vec3<f32>(
            textureSampleLevel(destination, destination_sampler, base_uv + split, 0.0).r,
            textureSampleLevel(destination, destination_sampler, base_uv, 0.0).g,
            textureSampleLevel(destination, destination_sampler, base_uv - split, 0.0).b);
    }
    let luma = max(dot(color, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.0);
    // Weave lifts and darkens by about one percent facing the opening, more
    // where the membrane is seen edge-on.
    let weave_gain = strength * (0.012 + 0.05 * (1.0 - v.facing) * (1.0 - v.facing) + 0.025 * v.rim);
    color *= 1.0 + weave_gain * (v.weave - 0.3);
    // Pearl: the destination's own light, re-hued by the nacre film. Faint
    // facing the opening, strong edge-on and while crossing.
    let grazing = 1.0 - v.facing;
    let pearl_mix = strength * clamp(0.03 + 0.32 * grazing * grazing + 0.14 * v.rim + 0.35 * near, 0.0, 0.7);
    color = mix(color, luma * mix(vec3<f32>(1.0), unit_luma(pl.film), 0.45), pearl_mix);
    // Silvering: reflectance tinted by the film, lit by the destination itself
    // so it never glows brighter than the world behind it.
    let sheen = pl.film * veil_fresnel(v) * (0.45 * luma + 0.01) * strength;
    color += sheen * (0.6 + 0.4 * v.height) + pl.film * pl.growth * (0.03 * luma + 0.002) * strength;
    color += mix(vec3<f32>(1.0), pl.film, 0.35) * pl.spark * 0.4 * strength;
    // Contact seam where the membrane meets the ring, then the ring's light.
    color *= mix(0.88, 1.0, smoothstep(0.0, 0.05, v.edge));
    color += rim_light(v, in.uv, portal.clip_count.y);
    // A window about to be dropped for the budget hands over to the membrane
    // the planner will draw instead, so the swap never pops.
    if portal.clip_count.y == 0u {
        let share = opening_pixels / (portal.parent_size.x * portal.parent_size.y);
        let small = 1.0 - smoothstep(MIN_WINDOW_SHARE, MIN_WINDOW_SHARE * 2.0, share);
        let m = membrane(v, pl, in.uv, 1u);
        color = mix(color, color * (1.0 - m.a) + m.rgb, small);
    }
    return vec4<f32>(color, 1.0);
}

/// Opening whose destination is not resident: a translucent membrane over the
/// scene behind the ring, premultiplied alpha.
@fragment
fn fs_veil(in:VertexOut)->@location(0) vec4<f32> {
    let footprint = length(fwidth(in.uv * THREADS_PER_M
        * vec2<f32>(length(portal.aperture[1].xyz - portal.aperture[0].xyz),
                    length(portal.aperture[3].xyz - portal.aperture[0].xyz))));
    clip_discard(in.relative);
    let v = veil_surface(in.uv, in.relative, footprint);
    let texels_per_pixel = footprint * TEXELS_PER_M / THREADS_PER_M;
    return membrane(v, pearl(v, in.uv, texels_per_pixel), in.uv, portal.clip_count.y);
}
