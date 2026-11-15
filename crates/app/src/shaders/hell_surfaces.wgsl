// Hell blocks on the 5 cm texel lattice: Hellstone, umber caps, and, inside
// Hell only, ash drifts and fungal soil. Hellstone and umber caps generate
// nowhere else; ash and fungal soil also occur on the parent planet, so their
// Hell finish waits for `hell_world()` and the planet keeps its natural look.
//
// Outputs beside the albedo, reset by fs_main before each call: `natural_tilt`
// (relief), `surface_finish` (roughness / reflectance scale) and `hell_glow`
// (emission, scaled by fire exposure in fs_main). Light comes from the upper
// left like every sprite. Each motif fades to the block's base colour once it
// falls below a pixel. Palettes are authored sRGB.

// Packed material ids (BlockId + 1); pinned by a const assert in `world/portals/realm.rs`.
const HELL_ASH: u32 = 62u;
const HELL_FUNGAL_SOIL: u32 = 29u;
const HELL_UMBER_CAP: u32 = 87u;
const HELL_HELLSTONE: u32 = 191u;

var<private> hell_glow: vec3<f32>;

// `realm[11].y` is set by `portals::realm::lighting` for Hell worlds.
fn hell_world() -> bool {
    return light.realm[11].y > 0.5;
}

fn is_hell_block(block: u32) -> bool {
    if block == HELL_HELLSTONE || block == HELL_UMBER_CAP { return true; }
    return hell_world() && (block == HELL_ASH || block == HELL_FUNGAL_SOIL);
}

fn hc(r: f32, g: f32, b: f32) -> vec3<f32> {
    return terrain_srgb_to_linear(vec3<f32>(r, g, b) / 255.0);
}

fn hhash(t: vec2<f32>, salt: f32) -> f32 {
    return surface_hash(t + vec2<f32>(salt * 23.17, salt * -11.41));
}

// Slow breathing shared by every crack in a region, so a wall pulses as one
// body instead of glittering. Period 6-11 s, phase from the location.
fn hell_breath(q: vec2<f32>) -> f32 {
    let t = light.torch_meta.y;
    let phase = surface_noise(q / 9.0 + vec2<f32>(5.0, -3.0)) * 6.2832;
    let a = sin(t * 0.71 + phase);
    let b = sin(t * 0.43 + phase * 1.7 + 1.3);
    return 0.78 + 0.14 * a + 0.08 * b;
}

// Distance in texels to the 0.5 level set of a noise field, from its gradient.
fn hvein(x: vec2<f32>, scale_m: f32) -> f32 {
    let g = surface_noise_grad(x);
    let grad_m = max(length(g.yz) / scale_m, 0.05);
    return abs(g.x - 0.5) / grad_m * PIXEL_RES;
}

// Voronoi plates of `cell` metres: x = distance to the nearest border in
// metres, y = own cell hash, z = hash of the cell across that border.
fn hplates(q: vec2<f32>, cell: f32, salt: f32) -> vec3<f32> {
    let g = q / cell;
    let i = floor(g);
    let f = g - i;
    var best = 8.0;
    var mr = vec2<f32>(0.0);
    var mi = vec2<f32>(0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let o = vec2<f32>(f32(x), f32(y));
            let r = o + vec2<f32>(hhash(i + o, salt), hhash(i + o, salt + 1.0)) * 0.7 + 0.15 - f;
            let d = dot(r, r);
            if d < best { best = d; mr = r; mi = i + o; }
        }
    }
    var edge = 8.0;
    var other = 0.0;
    for (var y = -2; y <= 2; y++) {
        for (var x = -2; x <= 2; x++) {
            let c = mi + vec2<f32>(f32(x), f32(y));
            if all(c == mi) { continue; }
            let r = c - i + vec2<f32>(hhash(c, salt), hhash(c, salt + 1.0)) * 0.7 + 0.15 - f;
            let d = dot(0.5 * (mr + r), normalize(r - mr));
            if d < edge { edge = d; other = hhash(c, salt + 2.0); }
        }
    }
    return vec3<f32>(edge * cell, hhash(mi, salt + 2.0), other);
}

// Hellstone: clotted maroon rock cooled into crusted plates. Fissures between
// the plates run hot: a white-yellow core, orange walls and a dark scorched
// lip. Up faces crack into polygonal crust; walls carry drip runnels and
// sulphur bloom gathers in hollows.
fn hell_hellstone(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, uv: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv * PIXEL_RES);
    let q = (t + 0.5) / PIXEL_RES;
    let tw = surface_detail_weight(fp * PIXEL_RES);
    let mw = surface_detail_weight(fp * PIXEL_RES / 3.0);
    let up = n.y > 0.5;
    let wall = abs(n.y) < 0.5;
    // Cooling plates ~1.6 m across, each seam a Voronoi border. About half
    // the seams still run hot; the rest are dark cracks. Fine crazing at 0.5 m.
    let warp = vec2<f32>(surface_noise(q / 3.1 + vec2<f32>(11.0, 2.0)), surface_noise(q / 2.7 + vec2<f32>(-4.0, 17.0))) * 0.35;
    let plate = hplates(q + warp, 1.6, 3.0);
    let craze_cell = hplates(q + warp * 0.5, 0.5, 9.0);
    let seam = plate.x * PIXEL_RES;
    let live = step(0.60, fract((plate.y + plate.z) * 7.31));
    let crack_core = (1.0 - smoothstep(0.2, 0.9, seam)) * live * mw;
    let crack_wall = (1.0 - smoothstep(0.6, 2.4, seam)) * mw;
    let craze = (1.0 - smoothstep(0.0, 0.7, craze_cell.x * PIXEL_RES)) * tw * 0.55;
    // Rock body, relative to the stock stone: oxblood shading, near-black
    // clinker and rust patches.
    let tone = surface_noise_filtered(q / 1.9 + vec2<f32>(7.0, 1.0), fp / 1.9);
    let clinker = smoothstep(0.62, 0.80, surface_noise_filtered(q / 0.7 + vec2<f32>(-13.0, 29.0), fp / 0.7));
    let rust = base * vec3<f32>(1.12, 1.28, 1.05);
    let char_black = base * vec3<f32>(0.24, 0.20, 0.22);
    var col = base * mix(0.80, 1.06, tone);
    col = mix(col, rust, smoothstep(0.70, 0.92, tone) * 0.40);
    col = mix(col, char_black, clinker * 0.65);
    // Vesicles: gas bubbles frozen in the rock, a dark pit with a lit lower rim.
    let g = hhash(t, 1.0);
    let pit = step(0.985, hhash(t, 2.0)) * tw;
    let pit_rim = step(0.985, hhash(t + vec2<f32>(0.0, 1.0), 2.0)) * tw * (1.0 - pit);
    // The stock surface already carries texel grain; add only clusters.
    let cluster = hhash(floor(t / 2.0), 8.0);
    col *= 1.0 + (cluster - 0.5) * 0.14 * tw;
    col = mix(col, char_black * 0.6, pit * 0.8);
    col *= 1.0 + pit_rim * 0.14;
    // Plate relief: each plate domes slightly, cracks sink.
    let dome = surface_noise_grad_filtered((q + warp) / 1.6 + vec2<f32>(plate.y * 31.0, plate.y * 17.0), fp / 1.6);
    natural_tilt += natural_tilt_from(n, -dome.yz * 0.05 * mw);
    col *= 0.92 + plate.y * 0.16;
    if wall {
        // Drip runnels: thin vertical streaks of darker glaze.
        let runnel = smoothstep(0.62, 0.86, surface_noise_filtered(vec2<f32>(q.x * 7.0, q.y * 0.35) + vec2<f32>(2.0, 5.0), fp * 7.0));
        col = mix(col, col * vec3<f32>(0.62, 0.48, 0.46), runnel * 0.55 * mw);
    }
    if up {
        // Sulphur bloom settles in the low parts of the crust.
        let bloom = smoothstep(0.66, 0.84, surface_noise_filtered(q / 1.1 + vec2<f32>(31.0, -8.0), fp / 1.1)) * (1.0 - crack_wall);
        col = mix(col, hc(196.0, 168.0, 70.0) * (0.85 + g * 0.3 * tw), bloom * 0.22);
    }
    // Scorched lip, then hot walls, then the core.
    col = mix(col, char_black, crack_wall * 0.7);
    col = mix(col, char_black * 0.7, craze * 0.6);
    let heat = hell_breath(q);
    let ember = hc(255.0, 120.0, 34.0);
    let core = hc(255.0, 214.0, 120.0);
    let hot = mix(ember, core, crack_core * crack_core);
    col = mix(col, hot * 0.9, crack_core * 0.85);
    // Emission: the core burns, the walls and crazing smoulder. Scaled down at
    // range so a far cliff reads as veined rock, not a lit grid.
    let wall_glow = max(crack_wall - crack_core, 0.0) * live;
    hell_glow = (hot * crack_core * 0.85 + ember * wall_glow * 0.10 + ember * craze * 0.04) * heat;
    surface_finish = vec3<f32>(1.0 - crack_core * 0.25, 1.0, 0.0);
    col *= 1.0 + gmacro(p, fp, 7.0) * 0.18;
    return mix(base, col, max(mw, 0.4));
}

// Umber cap: a leathery brown mushroom skin, darker toward its gills, with
// pale scale warts and a few faintly luminous freckles.
fn hell_umber(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, uv: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv * PIXEL_RES);
    let q = (t + 0.5) / PIXEL_RES;
    let tw = surface_detail_weight(fp * PIXEL_RES);
    let mw = surface_detail_weight(fp * PIXEL_RES / 3.0);
    let skin = surface_noise_filtered(q / 0.9 + vec2<f32>(4.0, 8.0), fp / 0.9);
    var col = base * mix(0.78, 1.08, skin);
    // Underside: radial gills read as fine dark lines.
    if n.y < -0.5 {
        let gill = abs(fract(atan2(q.y - floor(q.y) - 0.5, q.x - floor(q.x) - 0.5) * 9.55) - 0.5);
        col = base * mix(0.55, 0.95, smoothstep(0.1, 0.4, gill) * tw + (1.0 - tw) * 0.5);
    }
    // Scale warts on a jittered 0.25 m lattice.
    let cell = floor(q / 0.25);
    let jitter = vec2<f32>(hhash(cell, 3.0), hhash(cell, 4.0)) * 0.6 + 0.2;
    let d = length(fract(q / 0.25) - jitter) * 0.25 * PIXEL_RES;
    let wart = (1.0 - smoothstep(0.6, 1.4, d)) * step(0.45, hhash(cell, 5.0)) * mw;
    col = mix(col, base * 0.5 + hc(150.0, 132.0, 104.0) * 0.5, wart * 0.75);
    natural_tilt += natural_tilt_from(n, (fract(q / 0.25) - jitter) * -0.4 * wart);
    // Luminous freckles: rare single texels that breathe amber.
    let freckle = step(0.993, hhash(t, 6.0)) * tw;
    let breath = hell_breath(q * 3.0);
    col = mix(col, hc(255.0, 190.0, 96.0), freckle * 0.7);
    hell_glow = hc(255.0, 170.0, 70.0) * freckle * 0.9 * breath;
    surface_finish = vec3<f32>(1.0 - wart * 0.2, 1.0, 0.0);
    return mix(base, col, max(mw, 0.4));
}

// Hell ash: fine grey with soft wind ripples on floors, darker char flecks and
// rare embers still alive in the drift. Walls are packed ash with bedding.
fn hell_ash(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, uv: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv * PIXEL_RES);
    let q = (t + 0.5) / PIXEL_RES;
    let tw = surface_detail_weight(fp * PIXEL_RES);
    let mw = surface_detail_weight(fp * PIXEL_RES / 3.0);
    let g = hhash(t, 11.0);
    var col = base * (0.92 + (surface_noise_filtered(q / 2.4 + vec2<f32>(1.0, 4.0), fp / 2.4) - 0.5) * 0.30);
    if n.y > 0.5 {
        let ripple = sin((q.x * 0.8 + q.y) * 14.0 + surface_noise(q * 0.9) * 5.0);
        col *= 1.0 + ripple * 0.05 * mw;
        natural_tilt += natural_tilt_from(n, vec2<f32>(0.8, 1.0) * cos((q.x * 0.8 + q.y) * 14.0) * 0.03 * mw);
        // Embers buried in the drift wake and fade over a few seconds.
        let cell = floor(q / 0.35);
        let live = step(0.94, hhash(cell, 12.0));
        let centre = (cell + vec2<f32>(hhash(cell, 13.0), hhash(cell, 14.0)) * 0.6 + 0.2) * 0.35;
        let r = length(q - centre) * PIXEL_RES;
        let spot = (1.0 - smoothstep(0.3, 1.2, r)) * live * tw;
        let wake = max(sin(light.torch_meta.y * (0.4 + hhash(cell, 15.0) * 0.6) + hhash(cell, 16.0) * 6.2832), 0.0);
        let ember = hc(255.0, 110.0, 30.0);
        col = mix(col, ember * 0.6, spot * wake * 0.8);
        hell_glow = ember * spot * wake * wake * 0.9;
    } else {
        let bed = fract(q.y * 3.1 + surface_noise(q / 1.6) * 0.8);
        col *= 1.0 - select(0.0, 0.10, bed < 0.08) * mw;
    }
    let fleck = step(0.97, g) * tw;
    col = mix(col, col * 0.55, fleck * 0.5);
    return mix(base, col, max(mw, 0.4));
}

// Hell fungal soil: dark peat threaded with mycelium that glows faint amber,
// brighter where threads cross. Matches the Fungal Hollows spores.
fn hell_fungal(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, uv: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv * PIXEL_RES);
    let q = (t + 0.5) / PIXEL_RES;
    let tw = surface_detail_weight(fp * PIXEL_RES);
    let mw = surface_detail_weight(fp * PIXEL_RES / 3.0);
    let g = hhash(t, 21.0);
    var col = base * (0.86 + (surface_noise_filtered(q / 1.3 + vec2<f32>(9.0, 2.0), fp / 1.3) - 0.5) * 0.35);
    let a = hvein(q / 0.6 + vec2<f32>(2.0, 7.0), 0.6);
    let b = hvein(q / 0.85 + vec2<f32>(-8.0, 3.0), 0.85);
    // Threads break up along their length so they read as mycelium, not scratches.
    let broken = smoothstep(0.35, 0.6, surface_noise(q / 0.3 + vec2<f32>(5.0, 1.0)));
    let thread = (1.0 - smoothstep(0.0, 0.5, min(a, b))) * tw * broken;
    let knot = (1.0 - smoothstep(0.0, 1.1, max(a, b))) * mw;
    let amber = hc(240.0, 176.0, 84.0);
    col = mix(col, hc(150.0, 132.0, 104.0), thread * 0.35);
    col = mix(col, amber * 0.8, knot * 0.45);
    let breath = hell_breath(q * 2.0);
    hell_glow = amber * (thread * 0.04 + knot * 0.30) * breath * select(0.5, 1.0, n.y > 0.5);
    col *= 1.0 + (g - 0.5) * 0.06 * tw;
    return mix(base, col, max(mw, 0.4));
}

// Dispatch from block_surface. `p` is in metres; `fp` is the metric footprint.
fn hell_surface(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, block: u32, fp: f32) -> vec3<f32> {
    var uv = p.xz;
    if abs(n.x) > 0.5 { uv = p.zy; }
    else if abs(n.z) > 0.5 { uv = p.xy; }
    switch block {
        case HELL_HELLSTONE: { return hell_hellstone(base, p, n, uv, fp); }
        case HELL_UMBER_CAP: { return hell_umber(base, p, n, uv, fp); }
        case HELL_ASH: { return hell_ash(base, p, n, uv, fp); }
        case HELL_FUNGAL_SOIL: { return hell_fungal(base, p, n, uv, fp); }
        default: { return base; }
    }
}

// Squall haze: a short-range veil of the storm's colour, strength in
// `realm[11].z` (`portals::realm::hell_weather`). The lighting fog cannot do
// this: its range is held past most of the view distance. `air` is the ground
// fog radiance; the veil keeps its brightness and takes the fog's hue.
fn hell_squall_air(lit: vec3<f32>, dist: f32, air: vec3<f32>) -> vec3<f32> {
    let s = light.realm[11].z;
    if s <= 0.0 { return lit; }
    let fog = light.fog_color_density.rgb;
    let hue = fog / max(dot(fog, vec3<f32>(0.2126, 0.7152, 0.0722)), 1e-4);
    let veil = hue * dot(air, vec3<f32>(0.2126, 0.7152, 0.0722)) * 1.35;
    let reach = mix(900.0, 85.0, s);
    let t = (1.0 - exp(-dist / reach)) * s * 0.80;
    return mix(lit, veil, t);
}
