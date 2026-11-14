// Gilded Cave blocks (BlockId 255..268, packed material ids 256..269).
// One authored surface per block on the 5 cm texel lattice: banded cave rock,
// a coin hoard, set gems, op-art and zellige tiles, glazed brick, gilded
// relief, pietra dura and fish-scale glaze. Light comes from the upper left
// like every sprite; each motif fades to the block's base colour once its
// feature is smaller than a pixel. Palettes are authored sRGB.
//
// Outputs beside the albedo: `natural_tilt` (relief, applied by fs_main),
// `surface_finish` (roughness / reflectance scale), `gilded_metal` (metallic)
// and `gilded_glow` (emission). fs_main and the art_refiner harness reset
// them before each surface call.

const GILDED_FIRST: u32 = 256u;
const GILDED_GATE: u32 = 256u;
const GILDED_OCHRE: u32 = 257u;
const GILDED_GROTTO: u32 = 258u;
const GILDED_COINS: u32 = 259u;
const GILDED_JEWELED: u32 = 260u;
const GILDED_ZIGZAG: u32 = 261u;
const GILDED_LOZENGE: u32 = 262u;
const GILDED_STAR: u32 = 263u;
const GILDED_LAPIS: u32 = 264u;
const GILDED_RELIEF: u32 = 265u;
const GILDED_MARBLE: u32 = 266u;
const GILDED_CARVED: u32 = 267u;
const GILDED_SCALE: u32 = 268u;
const GILDED_SUNSTONE: u32 = 269u;
const GILDED_LAST: u32 = 269u;

var<private> gilded_metal: f32;
var<private> gilded_glow: vec3<f32>;

fn is_gilded_block(block: u32) -> bool {
    return block >= GILDED_FIRST && block <= GILDED_LAST;
}

fn gc(r: f32, g: f32, b: f32) -> vec3<f32> {
    return terrain_srgb_to_linear(vec3<f32>(r, g, b) / 255.0);
}

fn gmod(a: f32, b: f32) -> f32 {
    return a - b * floor(a / b);
}

fn gmod2(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return a - b * floor(a / b);
}

fn ghash(t: vec2<f32>, salt: f32) -> f32 {
    return surface_hash(t + vec2<f32>(salt * 17.31, salt * -9.73));
}

// Lit bevel for a cell of `size` texels at texel `local` (0..size-1):
// +1 on the upper and left rims, -1 on the lower and right rims.
fn gbevel(local: vec2<f32>, size: vec2<f32>) -> f32 {
    let lit = select(0.0, 1.0, local.x < 0.5 || local.y > size.y - 1.5);
    let dark = select(0.0, 1.0, local.x > size.x - 1.5 || local.y < 0.5);
    return lit - dark;
}

// Relief tilt: `facing` is the direction the facet leans in face uv.
fn gtilt(n: vec3<f32>, facing: vec2<f32>, amount: f32) {
    natural_tilt += natural_tilt_from(n, -facing * amount);
}

// Slow albedo drift across metres so a wall of one block never reads as a grid.
fn gmacro(p: vec3<f32>, fp: f32, salt: f32) -> f32 {
    let q = p.xz + vec2<f32>(p.y * 0.61, -p.y * 0.37) + vec2<f32>(salt * 13.0, salt * 7.0);
    return (surface_noise_filtered(q / 6.5, fp / 6.5) - 0.5) * 0.7
        + (surface_noise_filtered(q / 2.3, fp / 2.3) - 0.5) * 0.3;
}

// Smooth four-stop ramp, x in [0, 1].
fn gramp4(x: f32, a: vec3<f32>, b: vec3<f32>, c: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    let s = clamp(x, 0.0, 1.0) * 3.0;
    if s < 1.0 { return mix(a, b, smoothstep(0.0, 1.0, s)); }
    if s < 2.0 { return mix(b, c, smoothstep(0.0, 1.0, s - 1.0)); }
    return mix(c, d, smoothstep(0.0, 1.0, s - 2.0));
}

// Hand-cut edge chips: a 1-2 texel bite out of the rim of some cells.
fn gchip(local: vec2<f32>, size: vec2<f32>, id: f32) -> f32 {
    if id < 0.62 { return 0.0; }
    let along = floor(fract(id * 7.13) * max(size.x - 3.0, 1.0)) + 1.0;
    let top = local.y > size.y - 1.5 && abs(local.x - along) < 1.5;
    let corner = id > 0.86 && local.x < 1.5 && local.y < 1.5;
    return select(0.0, 1.0, top || corner);
}

// --- Natural rock -----------------------------------------------------------

// Wind-cut sandstone: beds a hand-span thick, each its own tone between cream,
// gold, rose and violet, with dark partings, cross-bedded laminae, flow
// scallops and pits. Floors show the same beds as contour swirls.
fn gilded_ochre(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, uv: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv * PIXEL_RES);
    let q = (t + 0.5) / PIXEL_RES;
    let tw = surface_detail_weight(fp * PIXEL_RES);
    let bed_w = surface_detail_weight(fp * PIXEL_RES / 2.0);
    let up = abs(n.y) > 0.5;
    let warp = surface_noise_filtered(vec2<f32>(q.x / 2.6, q.y / 4.0) + vec2<f32>(3.0, 11.0), fp / 2.6) * 1.5
        + surface_noise_filtered(vec2<f32>(q.x / 0.9, q.y / 1.4) + vec2<f32>(-7.0, 5.0), fp / 0.9) * 0.30;
    var s = q.y + warp;
    if up {
        s = surface_noise_filtered(q / 2.8 + vec2<f32>(19.0, -4.0), fp / 2.8) * 2.2
            + surface_noise_filtered(q / 0.8 + vec2<f32>(-2.0, 31.0), fp / 0.8) * 0.25;
    }
    let bands = s * 4.2;
    let b = floor(bands);
    let f = fract(bands);
    let bed = ghash(vec2<f32>(b, 3.0), 4.0);
    let cream = gc(243.0, 216.0, 166.0);
    let gold = gc(234.0, 164.0, 84.0);
    let orange = gc(222.0, 116.0, 52.0);
    let rose = gc(178.0, 82.0, 62.0);
    let violet = gc(112.0, 60.0, 92.0);
    let drift = abs(fract(s * 0.19 + 0.31) * 2.0 - 1.0);
    let x = clamp(drift + (bed - 0.5) * 0.34, 0.0, 1.0);
    var col = gramp4(x, cream, gold, rose, violet);
    col = mix(col, orange, smoothstep(0.30, 0.55, x) * (1.0 - smoothstep(0.55, 0.80, x)) * 0.55);
    // Dark parting at the base of each bed, a lit lip at its top.
    let parting = select(0.0, 1.0, f < 0.06) * bed_w;
    let lip = select(0.0, 1.0, f > 0.93) * bed_w;
    // Thin laminae inside each bed: a soft value ripple.
    col *= 0.96 + 0.06 * sin(f * 6.2832 * 3.0 + bed * 5.0) * tw;
    col *= (1.0 - parting * 0.28) * (1.0 + lip * 0.07);
    // Cross-bedding: some beds carry inclined laminae.
    if !up && bed > 0.55 {
        let slope = select(-0.9, 0.9, bed > 0.78);
        let lam = fract((q.x * slope + f / 4.2) * 22.0);
        col *= 1.0 - select(0.0, 0.08, lam < 0.22) * tw;
    }
    // Flow scallops: broad hollows that tilt the light.
    let scallop = surface_noise_grad_filtered(vec2<f32>(q.x * 1.4, q.y * 2.2) + vec2<f32>(41.0, 3.0), fp * 2.2);
    col *= 0.90 + scallop.x * 0.20;
    natural_tilt += natural_tilt_from(n, -scallop.yz * vec2<f32>(1.4, 2.2) * 0.035 * bed_w);
    // Quartz grain and weathering pits with a lit lower rim.
    let g = ghash(t, 1.0);
    col *= 1.0 + (g - 0.5) * 0.09 * tw;
    let pit = step(0.994, ghash(t, 2.0)) * tw;
    let pit_rim = step(0.994, ghash(t + vec2<f32>(0.0, 1.0), 2.0)) * tw;
    col = mix(col, col * 0.45, pit);
    col = mix(col, col * 1.12, pit_rim * (1.0 - pit));
    // Wind-blown sand settles on floors.
    if n.y > 0.5 {
        let sand = smoothstep(0.42, 0.68, surface_noise_filtered(q / 1.7 + vec2<f32>(5.0, 9.0), fp / 1.7));
        let ripple = 0.94 + 0.08 * sin((q.x + q.y * 0.3) * 41.0 + surface_noise(q * 2.0) * 4.0) * tw;
        col = mix(col, gc(236.0, 192.0, 124.0) * (0.94 + g * 0.10 * tw) * ripple, sand * 0.7);
    }
    col *= 1.0 + gmacro(p, fp, 1.0) * 0.14;
    surface_finish = vec3<f32>(1.0 + parting * 0.1, 1.0 - pit * 0.5, 0.0);
    return mix(base, col, max(bed_w, 0.35));
}

// Constant-width ridge lines from a noise field: distance to its 0.5 level
// set in texels, estimated from the analytic gradient.
fn gvein(x: vec2<f32>, scale_m: f32, width: f32) -> f32 {
    let g = surface_noise_grad(x);
    let grad_m = max(length(g.yz) / scale_m, 0.05);
    let d = abs(g.x - 0.5) / grad_m * PIXEL_RES;
    return 1.0 - smoothstep(width * 0.5, width, d);
}

// Teal grotto rock: cool mottled limestone, pale calcite veins, flowstone runs
// down the walls and a wet film that tightens the highlight.
fn gilded_grotto(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, uv: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv * PIXEL_RES);
    let q = (t + 0.5) / PIXEL_RES;
    // Rotated domain keeps value-noise lattice lines off the block axes.
    let r = vec2<f32>(q.x * 0.80 - q.y * 0.60, q.x * 0.60 + q.y * 0.80);
    let tw = surface_detail_weight(fp * PIXEL_RES);
    let vein_w = surface_detail_weight(fp * PIXEL_RES / 2.0);
    let deep = gc(14.0, 40.0, 62.0);
    let mid = gc(38.0, 124.0, 142.0);
    let pale = gc(100.0, 182.0, 186.0);
    let mottle = surface_noise_filtered(r / 0.9 + vec2<f32>(7.0, 2.0), fp / 0.9) * 0.6
        + surface_noise_filtered(r / 0.31 + vec2<f32>(-3.0, 8.0), fp / 0.31) * 0.4;
    var col = mix(deep, mid, smoothstep(0.28, 0.62, mottle));
    col = mix(col, pale, smoothstep(0.64, 0.84, mottle) * 0.5);
    let vein = max(gvein(r / 1.3 + vec2<f32>(31.0, 17.0), 1.3, 1.6),
        gvein(vec2<f32>(r.y, -r.x) / 0.5 + vec2<f32>(-13.0, 5.0), 0.5, 1.1) * 0.6);
    col = mix(col, gc(206.0, 240.0, 236.0), vein * vein_w * 0.8);
    var wet = 0.0;
    if abs(n.y) < 0.5 {
        // Flowstone: soft vertical curtains, lighter and glossy.
        let bend = surface_noise_filtered(q * vec2<f32>(1.3, 0.6) + vec2<f32>(5.0, 1.0), fp * 1.3) * 1.6;
        let run = surface_noise_filtered(vec2<f32>(q.x * 3.6 + bend, q.y * 0.45) + vec2<f32>(9.0, 2.0), fp * 3.6);
        let flow = smoothstep(0.56, 0.82, run);
        col = mix(col, gc(148.0, 212.0, 204.0), flow * 0.38);
        wet = flow;
    } else if n.y > 0.5 {
        wet = smoothstep(0.55, 0.75, surface_noise_filtered(r / 1.3 + vec2<f32>(3.0, 3.0), fp / 1.3));
        col *= 1.0 - wet * 0.18;
    }
    let g = ghash(t, 3.0);
    col *= 1.0 + (g - 0.5) * 0.08 * tw;
    col *= 1.0 + gmacro(p, fp, 2.0) * 0.18;
    let relief = surface_noise_grad_filtered(r * 2.4 + vec2<f32>(11.0, -6.0), fp * 2.4);
    natural_tilt += natural_tilt_from(n, -relief.yz * 0.05 * (1.0 - wet));
    surface_finish = vec3<f32>(mix(1.0, 0.32, wet), mix(1.0, 1.5, wet), 0.0);
    return mix(base, col, max(vein_w, 0.4));
}

// --- Hoard ------------------------------------------------------------------

// One layer of loose coins: jittered discs on a 5-texel lattice. Returns
// (coverage, radius 0..1, coin id) and writes the offset from the coin centre.
fn gcoin_layer(t: vec2<f32>, salt: f32, offset: ptr<function, vec2<f32>>) -> vec3<f32> {
    let cell = floor(t / 5.0);
    var best = vec3<f32>(0.0, 2.0, -1.0);
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let c = cell + vec2<f32>(f32(dx), f32(dy));
            let id = ghash(c, salt);
            let centre = c * 5.0 + vec2<f32>(2.5) + (vec2<f32>(ghash(c, salt + 1.0), ghash(c, salt + 2.0)) - 0.5) * 2.6;
            let radius = 3.1 + ghash(c, salt + 3.0) * 0.9;
            // Tilted coins read as ellipses; most lie nearly flat.
            let squash = 0.70 + sqrt(ghash(c, salt + 4.0)) * 0.30;
            let d = t + 0.5 - centre;
            let r = length(vec2<f32>(d.x, d.y / squash)) / radius;
            if r < 1.0 && id > best.z {
                best = vec3<f32>(1.0, r, id);
                *offset = vec2<f32>(d.x, d.y / squash) / radius;
            }
        }
    }
    return best;
}

fn gilded_coins(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, uv: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv * PIXEL_RES);
    let coin_w = surface_detail_weight(fp * PIXEL_RES / 4.0);
    let hi = gc(255.0, 236.0, 140.0);
    let mid = gc(236.0, 168.0, 40.0);
    let lo = gc(156.0, 90.0, 20.0);
    let gap = gc(64.0, 28.0, 10.0);
    var col = gap;
    var metal = 0.30;
    var rough = 1.8;
    var off = vec2<f32>(0.0);
    var coin = gcoin_layer(t, 11.0, &off);
    var shade = 1.0;
    if coin.x < 0.5 {
        coin = gcoin_layer(t + vec2<f32>(2.0, 3.0), 23.0, &off);
        shade = 0.72; // the lower layer sits in the heap's shadow
    }
    if abs(n.y) < 0.5 && coin.x < 0.5 {
        // Gaps on the flank of a heap show coins edge-on: one-texel milled edges.
        let mill = gmod(t.x + t.y * 2.0, 3.0);
        col = mix(lo, mid, 0.4 + ghash(vec2<f32>(floor(t.x / 4.0), t.y), 7.0) * 0.4) * (0.80 + mill * 0.06) * 0.8;
        metal = 1.0;
        rough = 0.7;
    }
    if coin.x > 0.5 {
        let tone = fract(coin.z * 9.7);
        var face = mix(mid, hi, 0.30 + tone * 0.30);
        face = mix(face, face * vec3<f32>(1.03, 1.02, 0.84), select(0.0, 1.0, tone > 0.82));
        face = mix(face, face * vec3<f32>(1.05, 0.84, 0.70), select(0.0, 1.0, tone < 0.10));
        let upper_left = -off.x + off.y;
        if coin.y > 0.80 {
            // Edge: a bright lip toward the upper left, a dark line elsewhere.
            face = select(lo * 0.50, hi * 1.08, upper_left > 0.30);
        } else if coin.y > 0.58 {
            // Raised rim ring around a flat struck field.
            face = mix(face * 1.10, hi, 0.35);
        } else {
            // Flat field a shade deeper than the rim, with a raised boss.
            face *= 0.86;
            if coin.y < 0.24 { face = mix(face, hi, 0.40); }
        }
        col = face * shade;
        metal = 1.0;
        rough = select(0.42, 0.75, coin.y > 0.84);
        gtilt(n, off, select(0.10, 0.26, coin.y > 0.64) * coin_w);
    }
    // Loose gems scattered through the heap.
    let gcell = floor(t / 9.0);
    let gid = ghash(gcell, 41.0);
    if gid > 0.93 {
        let gpos = gcell * 9.0 + vec2<f32>(2.0, 2.0) + floor(vec2<f32>(ghash(gcell, 42.0), ghash(gcell, 43.0)) * 4.0);
        let d = t - gpos;
        if d.x >= 0.0 && d.x <= 2.0 && d.y >= 0.0 && d.y <= 2.0 && !(d.x != 1.0 && d.y != 1.0 && gid < 0.97) {
            let k = fract(gid * 31.0);
            var gem = select(select(gc(36.0, 84.0, 214.0), gc(23.0, 178.0, 106.0), k > 0.33), gc(224.0, 30.0, 56.0), k > 0.66);
            gem *= select(select(0.80, 0.55, d.x > 1.5 && d.y < 0.5), 1.7, d.x < 0.5 && d.y > 1.5);
            col = gem;
            metal = 0.0;
            rough = 0.2;
        }
    }
    col *= 1.0 + gmacro(p, fp, 3.0) * 0.10;
    gilded_metal = mix(0.55, metal * 0.62, coin_w);
    surface_finish = vec3<f32>(mix(1.0, rough, coin_w), mix(1.0, 1.1, coin_w), 0.0);
    return mix(base, col, coin_w);
}

// Repoussé gold sheet set with cabochons in beaded bezels on a staggered grid.
fn gilded_jeweled(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, uv: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv * PIXEL_RES);
    let tw = surface_detail_weight(fp * PIXEL_RES);
    let cell_w = surface_detail_weight(fp * PIXEL_RES / 2.5);
    let hi = gc(255.0, 226.0, 138.0);
    let mid = gc(217.0, 162.0, 27.0);
    let lo = gc(138.0, 90.0, 16.0);
    let row = floor(t.y / 10.0);
    let shift = gmod(row, 2.0) * 5.0;
    let cell = vec2<f32>(floor((t.x + shift) / 10.0), row);
    let local = vec2<f32>(t.x + shift - cell.x * 10.0, t.y - row * 10.0);
    let d = local + 0.5 - vec2<f32>(5.0, 5.0);
    let r = length(d);
    let id = ghash(cell, 51.0);
    // Chased scroll in the sheet between the stones.
    let scroll = sin((local.x + local.y * 0.5) * 0.9 + id * 6.0) * cos(local.y * 0.8 - id * 3.0);
    var col = mix(lo, hi, 0.45 + scroll * 0.25);
    var metal = 1.0;
    var rough = 0.55;
    gtilt(n, vec2<f32>(cos(local.x * 0.9), sin(local.y * 0.8)) * scroll, 0.06 * tw);
    // Beaded bezel: a ring of 1-texel pearls of gold.
    let bezel = r > 3.0 && r < 4.3;
    if bezel {
        let bead = gmod(floor(atan2(d.y, d.x) * 3.2), 2.0);
        col = mix(mid, hi, 0.3 + bead * 0.6) * select(0.85, 1.15, d.x < 0.0 && d.y > 0.0);
        rough = 0.35;
        gtilt(n, d / max(r, 0.5), 0.18 * tw);
    }
    if r <= 3.0 {
        // Cabochon: deep colour, a hot highlight up-left, light pooled low-right.
        let k = fract(id * 13.7);
        var gem = gc(206.0, 24.0, 52.0);
        if k > 0.30 { gem = gc(20.0, 160.0, 96.0); }
        if k > 0.55 { gem = gc(30.0, 70.0, 200.0); }
        if k > 0.78 { gem = gc(128.0, 52.0, 170.0); }
        if k > 0.93 { gem = gc(236.0, 230.0, 214.0); }
        let dome = 1.0 - r / 3.0;
        var c = gem * (0.55 + dome * 0.55);
        let glint = select(0.0, 1.0, d.x < -0.5 && d.x > -2.0 && d.y > 0.5 && d.y < 2.0);
        let pool = select(0.0, 1.0, d.x > 0.5 && d.y < -0.5 && r < 2.6);
        c = mix(c, c * 1.8 + vec3<f32>(0.10), pool * 0.45);
        c = mix(c, vec3<f32>(1.0, 0.97, 0.90), glint * 0.75);
        col = c;
        metal = 0.0;
        rough = 0.12;
        gilded_glow = gem * pool * 0.012;
        gtilt(n, d / 3.0, 0.30 * tw);
    }
    col *= 1.0 + gmacro(p, fp, 4.0) * 0.08;
    gilded_metal = mix(0.55, metal * 0.62, cell_w);
    surface_finish = vec3<f32>(mix(1.0, rough / 0.28, cell_w), 1.0, 0.0);
    return mix(base, col, cell_w);
}

// --- Tiles ------------------------------------------------------------------

// Glazed 0.5 m tiles: returns texel position inside the tile, the tile id and
// writes the grout mask, a chip mask and a per-tile glaze tilt.
fn gtile(t: vec2<f32>, size: f32, n: vec3<f32>, tw: f32, grout: ptr<function, f32>) -> vec3<f32> {
    let cell = floor(t / size);
    let local = t - cell * size;
    let id = ghash(cell, 61.0);
    let g = select(0.0, 1.0, local.x < 0.5 || local.y < 0.5);
    *grout = max(g, gchip(local, vec2<f32>(size), id) * 0.8);
    // Hand-set tiles never sit quite flat; each catches light at its own angle.
    gtilt(n, vec2<f32>(ghash(cell, 62.0), ghash(cell, 63.0)) - 0.5, 0.05 * tw);
    return vec3<f32>(local, id);
}

fn gglaze(col: vec3<f32>, t: vec2<f32>, local: vec2<f32>, size: f32, tw: f32) -> vec3<f32> {
    // Glaze pools thicker toward the lower edge; fine crackle in the field.
    let pool = 1.0 - local.y / size;
    let crackle = step(0.93, ghash(floor(t / vec2<f32>(3.0, 2.0)) + t * 0.0, 71.0))
        * step(0.5, ghash(t, 72.0));
    let bev = gbevel(local - vec2<f32>(1.0, 1.0), vec2<f32>(size - 1.0));
    return col * (0.94 + pool * 0.08) * (1.0 - crackle * 0.10 * tw) * (1.0 + bev * 0.06 * tw);
}

// Op-art chevrons: ink and paper bands that zigzag across the tile, with a gilt
// pin line every fifth band. Alternate tiles flip so the floor shimmers.
fn gilded_zigzag(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, uv: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv * PIXEL_RES);
    let tw = surface_detail_weight(fp * PIXEL_RES);
    let band_w = surface_detail_weight(fp * PIXEL_RES / 2.0);
    var grout = 0.0;
    let tile = gtile(t, 10.0, n, tw, &grout);
    let ink = gc(22.0, 20.0, 26.0);
    let paper = gc(242.0, 234.0, 214.0);
    let gilt = gc(214.0, 160.0, 40.0);
    let flip = gmod(floor(t.x / 10.0) + floor(t.y / 10.0), 2.0);
    let zig = abs(gmod(t.x, 8.0) - 4.0);
    let y = t.y + select(zig, -zig, flip > 0.5);
    let band = floor(y / 2.0);
    var col = select(paper, ink, gmod(band, 2.0) > 0.5);
    let pin = gmod(band, 5.0) == 0.0 && gmod(y, 2.0) < 1.0;
    col = select(col, gilt, pin);
    // Paper yellows a little per tile; ink fades toward slate where worn.
    col *= mix(vec3<f32>(1.0), vec3<f32>(1.02, 0.99, 0.92), tile.z * 0.6);
    let worn = smoothstep(0.62, 0.85, surface_noise_filtered(p.xz / 1.4 + vec2<f32>(p.y), fp / 1.4));
    col = mix(col, col * 0.86 + vec3<f32>(0.035), worn * 0.4);
    col = gglaze(col, t, tile.xy, 10.0, tw);
    col = mix(col, gc(58.0, 50.0, 44.0), grout);
    gilded_metal = select(0.0, 0.85, pin) * band_w;
    surface_finish = vec3<f32>(mix(1.0, select(0.75, 1.6, grout > 0.5), band_w), 1.0, 0.0);
    return mix(base, col, band_w);
}

// Concentric lozenges, inverted on alternate tiles, with a lapis heart and a
// gilt stud: the floor appears to fold.
fn gilded_lozenge(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, uv: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv * PIXEL_RES);
    let tw = surface_detail_weight(fp * PIXEL_RES);
    let ring_w = surface_detail_weight(fp * PIXEL_RES / 2.0);
    var grout = 0.0;
    let tile = gtile(t, 20.0, n, tw, &grout);
    let ink = gc(20.0, 22.0, 34.0);
    let paper = gc(240.0, 236.0, 222.0);
    let lapis = gc(31.0, 63.0, 148.0);
    let gilt = gc(226.0, 176.0, 52.0);
    let local = tile.xy - vec2<f32>(10.0, 10.0) + 0.5;
    let d = abs(local.x) + abs(local.y);
    let ring = floor(d / 2.0);
    let invert = gmod(floor(t.x / 20.0) + floor(t.y / 20.0), 2.0) > 0.5;
    let odd = gmod(ring, 2.0) > 0.5;
    var col = select(paper, ink, odd != invert);
    if d < 3.0 { col = lapis; }
    if d < 1.0 { col = gilt; }
    // Corner quarter-diamonds in lapis tie four tiles into a larger lozenge.
    if d > 17.0 { col = select(lapis, gilt, d > 18.6); }
    col *= mix(vec3<f32>(1.0), vec3<f32>(1.02, 1.0, 0.95), tile.z * 0.5);
    col = gglaze(col, t, tile.xy, 20.0, tw);
    col = mix(col, gc(54.0, 48.0, 46.0), grout);
    gilded_metal = select(0.0, 0.85, d < 1.0 || d > 18.6) * ring_w;
    surface_finish = vec3<f32>(mix(1.0, select(0.75, 1.6, grout > 0.5), ring_w), 1.0, 0.0);
    return mix(base, col, ring_w);
}

// Zellige: an eight-pointed star per metre in cut glazed pieces, a white
// ring band, ochre crosses where four tiles meet, dark grout between pieces.
// Each piece is hand cut: its own tilt and glaze value.
fn gilded_star(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, uv: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv * PIXEL_RES);
    let tw = surface_detail_weight(fp * PIXEL_RES);
    let piece_w = surface_detail_weight(fp * PIXEL_RES / 3.0);
    let cell = floor(t / 20.0);
    let local = t - cell * 20.0 + 0.5 - vec2<f32>(10.0);
    let a = abs(local);
    // Two overlapping squares: axis-aligned and diagonal.
    let sq = max(a.x, a.y);
    let dia = (a.x + a.y) * 0.7071;
    // Union of the two squares is the eight-pointed star; the ring band
    // follows the same outline two texels further out.
    let star = min(sq, dia) / 5.0;
    let ring = min(sq, dia) / 6.7;
    let id = ghash(cell, 81.0);
    let cobalt = gc(30.0, 68.0, 160.0);
    let turq = gc(32.0, 160.0, 160.0);
    let green = gc(28.0, 112.0, 80.0);
    let white = gc(240.0, 236.0, 222.0);
    let ochre = gc(200.0, 146.0, 46.0);
    let grout_c = gc(40.0, 36.0, 34.0);
    var piece = 0.0;
    var col = ochre;
    var edge = 0.0;
    if star < 1.0 {
        // The star splits into eight kite pieces around a small centre.
        let k = select(select(cobalt, turq, id > 0.40), green, id > 0.78);
        let sector = floor((atan2(local.y, local.x) + 3.14159) / 0.7854);
        piece = 1.0 + sector;
        col = select(k, white, star < 0.22);
        edge = select(0.0, 1.0, star > 0.86 || abs(fract((atan2(local.y, local.x) + 3.14159) / 0.7854) - 0.5) > 0.47 && star > 0.30);
    } else if ring < 1.0 {
        col = white;
        piece = 20.0 + floor((atan2(local.y, local.x) + 3.14159) / 0.7854);
        edge = select(0.0, 1.0, ring > 0.90 || star < 1.10);
    } else {
        // Corner infill where four cells meet: ochre cross with black points.
        let corner = vec2<f32>(10.0) - a;
        piece = 40.0 + select(0.0, 1.0, local.x > 0.0) + select(0.0, 2.0, local.y > 0.0);
        let arm = min(corner.x, corner.y) < 1.2 && max(corner.x, corner.y) < 3.2;
        col = select(ochre, gc(30.0, 70.0, 150.0), arm);
        edge = select(0.0, 1.0, abs(min(corner.x, corner.y) - 1.2) < 0.5 && max(corner.x, corner.y) < 3.7);
    }
    let pid = ghash(cell * 64.0 + vec2<f32>(piece, 0.0), 82.0);
    col *= 0.90 + pid * 0.18;
    gtilt(n, vec2<f32>(ghash(vec2<f32>(pid, piece), 83.0), ghash(vec2<f32>(piece, pid), 84.0)) - 0.5, 0.09 * tw);
    // Glaze crawl: tiny pale pinholes, a zellige tell.
    col = mix(col, col * 1.25 + vec3<f32>(0.04), step(0.985, ghash(t, 85.0)) * tw);
    col = mix(col, grout_c, edge * tw);
    col *= 1.0 + gmacro(p, fp, 5.0) * 0.06;
    surface_finish = vec3<f32>(mix(1.0, select(0.7, 1.8, edge > 0.5), piece_w), mix(1.0, 1.15, piece_w), 0.0);
    return mix(base, col, piece_w);
}

// Glazed lapis brick in running bond with gilt joints; a few turquoise headers.
fn gilded_lapis(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, uv: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv * PIXEL_RES);
    let tw = surface_detail_weight(fp * PIXEL_RES);
    let brick_w = surface_detail_weight(fp * PIXEL_RES / 3.0);
    let row = floor(t.y / 4.0);
    let shift = gmod(row, 2.0) * 5.0;
    let bx = floor((t.x + shift) / 10.0);
    let local = vec2<f32>(t.x + shift - bx * 10.0, t.y - row * 4.0);
    let id = ghash(vec2<f32>(bx, row), 91.0);
    let joint = local.x < 0.5 || local.y < 0.5;
    var col = mix(gc(26.0, 52.0, 140.0), gc(40.0, 70.0, 170.0), id);
    if id > 0.93 { col = gc(30.0, 150.0, 158.0); }
    if id < 0.05 { col = gc(18.0, 34.0, 96.0); }
    // Glaze: a lit top edge, a darker pooled lower edge, mottled depth.
    let mottle = surface_noise_filtered((t + 0.5) / 3.0 + vec2<f32>(id * 40.0), fp * PIXEL_RES / 3.0);
    col *= 0.88 + mottle * 0.22;
    col *= select(1.0, 1.14, local.y > 2.5) * select(1.0, 0.86, local.y < 1.5);
    gtilt(n, vec2<f32>(0.0, (local.y - 2.0) / 2.0), 0.08 * tw);
    let worn = smoothstep(0.7, 0.9, surface_noise_filtered(p.xz / 1.1 + vec2<f32>(p.y * 0.7), fp / 1.1));
    let gilt = mix(gc(200.0, 146.0, 46.0), gc(110.0, 84.0, 50.0), worn);
    col = select(col, gilt, joint);
    gilded_metal = select(0.0, 0.8 * (1.0 - worn), joint) * tw;
    surface_finish = vec3<f32>(mix(1.0, select(0.6, 1.2, joint), brick_w), mix(1.0, 1.2, brick_w), 0.0);
    return mix(base, col, brick_w);
}

// Gilded strapwork: an interlaced medallion per metre (a ring knotted with four
// lobes around an eight-petal rosette), quarter medallions linking the
// corners, raised in gold over a lapis field. Straps catch the lamp on their
// upper-left shoulder and cast a short shadow down-right. The gilt wears
// through to red bole on the high points.
fn gstrap(d: f32, width: f32) -> f32 {
    return 1.0 - step(width, abs(d));
}

fn gilded_relief(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, uv: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv * PIXEL_RES);
    let tw = surface_detail_weight(fp * PIXEL_RES);
    let strap_w = surface_detail_weight(fp * PIXEL_RES / 2.5);
    let cell = floor(t / 20.0);
    let l = t - cell * 20.0 + 0.5 - vec2<f32>(10.0);
    let field = mix(gc(22.0, 38.0, 108.0), gc(38.0, 60.0, 146.0),
        surface_noise_filtered((t + 0.5) / 7.0, fp * PIXEL_RES / 7.0));
    let hi = gc(255.0, 230.0, 146.0);
    let mid = gc(220.0, 162.0, 34.0);
    let lo = gc(118.0, 76.0, 18.0);
    let bole = gc(140.0, 54.0, 36.0);
    // Signed distances (texels) to each strap centre line, with the outward
    // direction of that strap for shading.
    var best = 99.0;
    var dir = vec2<f32>(0.0);
    let r = length(l);
    let ring = r - 5.4;
    if abs(ring) < abs(best) { best = ring; dir = l / max(r, 0.5); }
    for (var k = 0; k < 4; k++) {
        let a = f32(k) * 1.5708;
        let c = vec2<f32>(cos(a), sin(a)) * 5.4;
        let lr = length(l - c);
        let lobe = lr - 2.3;
        if abs(lobe) < abs(best) { best = lobe; dir = (l - c) / max(lr, 0.5); }
    }
    // Quarter medallions centred on the cell corners.
    let corner = vec2<f32>(select(-10.0, 10.0, l.x > 0.0), select(-10.0, 10.0, l.y > 0.0));
    let cr = length(l - corner);
    var h = gstrap(best, 1.05);
    // Corner rosettes: four petals and a boss where four panels meet.
    let corner_petal = 2.4 * (0.45 + 0.55 * abs(cos(atan2(l.y - corner.y, l.x - corner.x) * 2.0)));
    let corner_rose = cr < corner_petal;
    if corner_rose { h = 1.0; best = 0.0; dir = (l - corner) / max(cr, 0.5); }
    // Rosette: eight petals around a boss.
    let petal = 2.6 * (0.62 + 0.38 * abs(cos(atan2(l.y, l.x) * 4.0)));
    let rosette = r < petal;
    if rosette { h = 1.0; dir = l / max(r, 0.5); }
    // Interlace: where the ring crosses a lobe, the strap that passes under
    // breaks for a texel either side, alternating around the medallion.
    let quadrant = floor((atan2(l.y, l.x) + 3.1416 + 0.3927) / 0.7854);
    let under_ring = gmod(quadrant, 2.0) > 0.5;
    var gap = false;
    if abs(ring) < 1.05 {
        for (var k = 0; k < 4; k++) {
            let a = f32(k) * 1.5708;
            let c = vec2<f32>(cos(a), sin(a)) * 5.4;
            let lobe = length(l - c) - 2.3;
            if abs(lobe) < 2.2 && abs(lobe) > 1.05 && under_ring { gap = true; }
        }
    }
    if gap { h = 0.0; }
    var col = field;
    var metal = 0.0;
    var rough = 1.0;
    if h > 0.5 {
        // Across the strap: the half toward the upper left is lit.
        let across = select(dir, -dir, best < 0.0);
        let facing = dot(across, vec2<f32>(-0.7071, 0.7071));
        var g = mix(mid, hi, 0.30 + select(0.0, 0.45, facing > 0.3 && abs(best) > 0.35));
        g = select(g, mix(g, lo, 0.50), facing < -0.3 && abs(best) > 0.35);
        if rosette { g = mix(mid, hi, select(0.35, 0.85, r < 1.2 || (l.x < 0.0 && l.y > 0.0))); }
        if corner_rose { g = mix(mid, hi, select(0.30, 0.80, cr < 1.0 || (l.x < corner.x && l.y > corner.y))); }
        let wear = smoothstep(0.68, 0.88, surface_noise_filtered((t + 0.5) / 4.0 + cell * 7.0, fp * PIXEL_RES / 4.0));
        col = mix(g, bole, wear * 0.7);
        metal = 1.0 - wear * 0.8;
        rough = 0.45 + wear;
        gtilt(n, across * select(0.0, 1.0, abs(best) > 0.35), 0.13 * tw);
    } else {
        // Cast shadow: the strap one texel up-left of this field texel.
        let s = l + vec2<f32>(-1.0, 1.0);
        let sr = length(s);
        var sb = abs(sr - 5.4);
        for (var k = 0; k < 4; k++) {
            let a = f32(k) * 1.5708;
            sb = min(sb, abs(length(s - vec2<f32>(cos(a), sin(a)) * 5.4) - 3.0));
        }
        if sb < 1.05 || sr < 2.6 { col = field * 0.62; }
    }
    col *= 1.0 + gmacro(p, fp, 6.0) * 0.08;
    gilded_metal = metal * 0.62 * strap_w;
    surface_finish = vec3<f32>(mix(1.0, rough, strap_w), 1.0, 0.0);
    return mix(base, col, strap_w);
}

// Pietra dura: polished white marble with grey-blue veins; an inset black
// band frames a five-petal flower of carnelian, lapis and malachite.
fn gilded_marble(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, uv: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv * PIXEL_RES);
    let q = (t + 0.5) / PIXEL_RES;
    let inlay_w = surface_detail_weight(fp * PIXEL_RES / 2.0);
    let r2 = vec2<f32>(q.x * 0.8 + q.y * 0.6, -q.x * 0.6 + q.y * 0.8);
    let warp = surface_noise_filtered(r2 / 0.8 + vec2<f32>(4.0, 1.0), fp / 0.8);
    var col = gc(238.0, 233.0, 222.0) * (0.95 + warp * 0.07);
    let vein = gvein(r2 / 1.5 + vec2<f32>(warp * 0.9, -warp * 0.6), 1.5, 1.4);
    col = mix(col, gc(146.0, 152.0, 166.0), vein * inlay_w * 0.6);
    let l = vec2<f32>(gmod(t.x, 20.0), gmod(t.y, 20.0)) + 0.5 - vec2<f32>(10.0);
    let a = abs(l);
    let edge = max(a.x, a.y);
    var inlay = false;
    var stone = 0.0;
    if edge > 8.0 && edge < 9.0 {
        col = gc(24.0, 24.0, 28.0);
        inlay = true;
    }
    let r = length(l);
    let ang = atan2(l.y, l.x) + 1.5708;
    // Nearest of five petals, measured in that petal's frame.
    let k = floor(ang / 1.2566 + 0.5);
    let pa = k * 1.2566 - 1.5708;
    let pf = vec2<f32>(dot(l, vec2<f32>(cos(pa), sin(pa))), dot(l, vec2<f32>(-sin(pa), cos(pa))));
    let e = length(vec2<f32>((pf.x - 3.6) / 2.7, pf.y / 1.6));
    if e < 1.0 {
        let kk = gmod(k + 5.0, 3.0);
        col = select(select(gc(188.0, 72.0, 42.0), gc(36.0, 70.0, 156.0), kk > 0.5), gc(28.0, 124.0, 84.0), kk > 1.5);
        // Stones are cut with grain and a lit upper-left shoulder.
        col *= 0.80 + (1.0 - e) * 0.30 + select(0.0, 0.12, pf.y > 0.5);
        if e > 0.80 { col *= 0.6; }
        inlay = true;
        stone = 1.0;
    }
    if r < 1.5 {
        col = gc(224.0, 172.0, 64.0) * select(0.85, 1.15, l.x < 0.0 && l.y > 0.0);
        inlay = true;
    }
    // Corner leaves pointing in, in malachite.
    let c = a - vec2<f32>(6.0);
    if length(vec2<f32>(c.x + c.y, (c.x - c.y) * 1.8)) < 1.8 && edge < 8.0 {
        col = gc(30.0, 122.0, 85.0) * select(0.9, 1.1, c.x + c.y > 0.0);
        inlay = true;
        stone = 1.0;
    }
    col *= 1.0 + gmacro(p, fp, 7.0) * 0.05;
    gilded_metal = select(0.0, 0.8, r < 1.5) * inlay_w;
    surface_finish = vec3<f32>(mix(1.0, select(0.7, 0.9, stone > 0.5), inlay_w), mix(1.0, 1.25, inlay_w), 0.0);
    return mix(base, col, inlay_w);
}

// Pink sandstone ashlar, tooled with fine diagonal chisel lines and chipped
// arrises; a carved dentil-and-rosette course runs at fixed heights.
fn gilded_carved(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, uv: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv * PIXEL_RES);
    let tw = surface_detail_weight(fp * PIXEL_RES);
    let course_w = surface_detail_weight(fp * PIXEL_RES / 3.0);
    let row = floor(t.y / 10.0);
    let shift = gmod(row, 2.0) * 10.0;
    let bx = floor((t.x + shift) / 20.0);
    let local = vec2<f32>(t.x + shift - bx * 20.0, t.y - row * 10.0);
    let id = ghash(vec2<f32>(bx, row), 101.0);
    var col = mix(gc(222.0, 160.0, 128.0), gc(236.0, 184.0, 150.0), id);
    col = mix(col, gc(206.0, 132.0, 108.0), smoothstep(0.7, 1.0, id) * 0.5);
    // Tooling: diagonal chisel strokes two texels apart.
    let tool = select(0.0, 1.0, gmod(local.x + local.y + floor(id * 4.0), 3.0) < 0.5);
    col *= 1.0 - tool * 0.05 * tw;
    col *= 1.0 + (ghash(t, 102.0) - 0.5) * 0.08 * tw;
    // Faint bedding in each block and grime gathered along its lower edge.
    col *= 0.97 + 0.05 * sin(t.y * 1.25 + id * 9.0 + surface_noise(vec2<f32>(t.x * 0.15, id * 7.0)) * 2.0) * tw;
    col *= 1.0 - select(0.0, 0.07, local.y < 2.5) * tw;
    let mortar = local.x < 0.5 || local.y < 0.5;
    let chip = gchip(local, vec2<f32>(20.0, 10.0), id);
    let bev = gbevel(local - vec2<f32>(1.0, 1.0), vec2<f32>(19.0, 9.0));
    col *= 1.0 + bev * 0.06 * tw;
    // Carved course on walls: one row in four, from world height.
    let carved = abs(n.y) < 0.5 && gmod(row, 4.0) == 3.0;
    if carved {
        let rc = vec2<f32>(gmod(t.x, 10.0) - 4.5, local.y - 4.5);
        let rose = length(rc);
        let petal = abs(cos(atan2(rc.y, rc.x) * 3.0));
        if rose < 3.4 * (0.6 + 0.4 * petal) {
            col *= select(1.08, 0.92, rc.x > 0.0 || rc.y < 0.0);
            gtilt(n, rc / max(rose, 0.5), 0.16 * tw);
        } else if local.y < 2.5 || local.y > 7.5 {
            // Dentils above and below the rosettes.
            col *= select(1.04, 0.70, gmod(t.x, 3.0) < 1.0);
        } else {
            col *= 0.80;
        }
    }
    col = mix(col, col * 0.62, chip * tw);
    col = select(col, gc(150.0, 110.0, 92.0), mortar);
    col *= 1.0 + gmacro(p, fp, 8.0) * 0.12;
    surface_finish = vec3<f32>(1.0 + chip * 0.2, 1.0, 0.0);
    return mix(base, col, course_w);
}

// Fish-scale glaze for domes: overlapping scallops 40 cm wide, each lighter at
// its crown and edged by a dark rim on its lower arc; a few lapis and gilt
// scales break the field. Rows overlap downward like roof tiles.
fn gscale_cell(t: vec2<f32>, row: f32) -> vec4<f32> {
    let shift = gmod(row, 2.0) * 4.0;
    let sx = floor((t.x + shift) / 8.0);
    let local = vec2<f32>(t.x + shift - sx * 8.0 + 0.5 - 4.0, t.y - row * 5.0 + 0.5);
    // Lower arc of a circle centred at the top of the row.
    let r = length(vec2<f32>(local.x, (local.y - 5.5) * 0.80)) / 4.3;
    return vec4<f32>(local, r, ghash(vec2<f32>(sx, row), 111.0));
}

fn gilded_scale(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, uv: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv * PIXEL_RES);
    let tw = surface_detail_weight(fp * PIXEL_RES);
    let scale_w = surface_detail_weight(fp * PIXEL_RES / 4.0);
    let row = floor(t.y / 5.0);
    var c = gscale_cell(t, row);
    if c.z > 1.0 {
        // Outside this scallop the scale of the row below shows.
        c = gscale_cell(t, row - 1.0);
    }
    let id = c.w;
    var glaze = mix(gc(26.0, 142.0, 144.0), gc(58.0, 190.0, 178.0), fract(id * 5.3));
    var metal = 0.0;
    if id > 0.93 { glaze = gc(30.0, 62.0, 150.0); }
    if id < 0.05 { glaze = gc(226.0, 174.0, 56.0); metal = 0.6; }
    // Crown highlight up-left, deeper glaze pooled toward the rim.
    let crown = clamp(1.0 - c.z, 0.0, 1.0);
    var col = glaze * (0.78 + crown * 0.34 + select(0.0, 0.08, c.x < 0.0));
    let rim = c.z > 0.80;
    let lit_rim = rim && c.x < -1.0 && c.y > 2.0;
    col = select(col, select(col * 0.45, col * 1.18, lit_rim), rim);
    // Glaze crawl pinholes.
    col = mix(col, col * 1.3, step(0.988, ghash(t, 112.0)) * tw);
    gtilt(n, vec2<f32>(c.x / 4.0, (c.y - 5.5) / 5.0), -0.14 * tw);
    col *= 1.0 + gmacro(p, fp, 9.0) * 0.08;
    gilded_metal = metal * scale_w;
    surface_finish = vec3<f32>(mix(1.0, select(0.6, 1.5, rim), scale_w), mix(1.0, 1.25, scale_w), 0.0);
    return mix(base, col, scale_w);
}

// Sunstone: warm crystal in broad facets with a bright core; it glows.
fn gilded_sunstone(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, uv: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv * PIXEL_RES);
    let facet_w = surface_detail_weight(fp * PIXEL_RES / 6.0);
    let cell = floor(t / 6.0);
    var best = 99.0;
    var second = 99.0;
    var bid = 0.0;
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let c = cell + vec2<f32>(f32(dx), f32(dy));
            let s = c * 6.0 + vec2<f32>(ghash(c, 121.0), ghash(c, 122.0)) * 6.0;
            let d = length(t + 0.5 - s);
            if d < best { second = best; best = d; bid = ghash(c, 123.0); }
            else if d < second { second = d; }
        }
    }
    let ridge = select(0.0, 1.0, second - best < 1.0);
    var col = mix(gc(236.0, 168.0, 88.0), gc(255.0, 240.0, 200.0), bid);
    col *= 0.78 + bid * 0.30 - ridge * 0.30;
    gtilt(n, vec2<f32>(ghash(vec2<f32>(bid, 1.0), 124.0), ghash(vec2<f32>(bid, 2.0), 125.0)) - 0.5, 0.3 * facet_w);
    gilded_glow = mix(gc(255.0, 214.0, 140.0), col, facet_w) * (0.30 + bid * 0.20 * facet_w);
    surface_finish = vec3<f32>(0.6, 1.3, 0.0);
    return mix(base, col, facet_w);
}

// The gate: a gold frame with a beaded fillet around a lapis field; an
// eight-pointed star of turquoise lines, and a glowing heart stone.
fn gilded_gate(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, uv: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv * PIXEL_RES);
    let tw = surface_detail_weight(fp * PIXEL_RES);
    let w = surface_detail_weight(fp * PIXEL_RES / 2.0);
    let cell = floor(t / 20.0);
    let l = t - cell * 20.0 + 0.5 - vec2<f32>(10.0);
    let a = abs(l);
    let edge = max(a.x, a.y);
    let hi = gc(255.0, 226.0, 138.0);
    let mid = gc(217.0, 162.0, 27.0);
    let lo = gc(120.0, 78.0, 16.0);
    var col = mix(gc(24.0, 42.0, 120.0), gc(32.0, 58.0, 150.0), ghash(t, 131.0) * 0.5 + 0.25);
    var metal = 0.0;
    var glow = 0.0;
    if edge > 7.0 {
        let bev = select(select(0.0, -1.0, l.x > 7.0 || l.y < -7.0), 1.0, l.x < -7.0 || l.y > 7.0);
        col = mix(mid, select(hi, lo, bev < 0.0), abs(bev) * 0.5);
        if edge > 7.5 && edge < 8.5 && gmod(floor((l.x + l.y) * 0.5), 2.0) > 0.5 { col = hi; }
        metal = 1.0;
        gtilt(n, select(vec2<f32>(sign(l.x), 0.0), vec2<f32>(0.0, sign(l.y)), a.y > a.x), 0.15 * tw);
    } else {
        let sq = max(a.x, a.y);
        let dia = (a.x + a.y) * 0.7071;
        let star = min(sq, dia) / 4.6;
        if abs(star - 1.0) < 0.16 {
            col = gc(40.0, 200.0, 190.0);
            glow = 0.4;
        }
        let r = length(l);
        if r < 2.0 {
            col = mix(gc(255.0, 214.0, 120.0), gc(255.0, 250.0, 220.0), 1.0 - r / 2.0);
            glow = 1.0;
            metal = 0.0;
        }
    }
    gilded_glow = col * glow * 0.6 * w;
    gilded_metal = metal * 0.62 * w;
    surface_finish = vec3<f32>(mix(1.0, select(1.4, 0.6, metal > 0.5), w), 1.0, 0.0);
    return mix(base, col, w);
}

// Dispatch from block_surface. `p` is in metres; `fp` is the metric footprint.
fn gilded_surface(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, block: u32, fp: f32) -> vec3<f32> {
    var uv = p.xz;
    if abs(n.x) > 0.5 { uv = p.zy; }
    else if abs(n.z) > 0.5 { uv = p.xy; }
    switch block {
        case GILDED_GATE: { return gilded_gate(base, p, n, uv, fp); }
        case GILDED_OCHRE: { return gilded_ochre(base, p, n, uv, fp); }
        case GILDED_GROTTO: { return gilded_grotto(base, p, n, uv, fp); }
        case GILDED_COINS: { return gilded_coins(base, p, n, uv, fp); }
        case GILDED_JEWELED: { return gilded_jeweled(base, p, n, uv, fp); }
        case GILDED_ZIGZAG: { return gilded_zigzag(base, p, n, uv, fp); }
        case GILDED_LOZENGE: { return gilded_lozenge(base, p, n, uv, fp); }
        case GILDED_STAR: { return gilded_star(base, p, n, uv, fp); }
        case GILDED_LAPIS: { return gilded_lapis(base, p, n, uv, fp); }
        case GILDED_RELIEF: { return gilded_relief(base, p, n, uv, fp); }
        case GILDED_MARBLE: { return gilded_marble(base, p, n, uv, fp); }
        case GILDED_CARVED: { return gilded_carved(base, p, n, uv, fp); }
        case GILDED_SCALE: { return gilded_scale(base, p, n, uv, fp); }
        case GILDED_SUNSTONE: { return gilded_sunstone(base, p, n, uv, fp); }
        default: { return base; }
    }
}
