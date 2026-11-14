//! Flora pack plant shapes and their selection bounds inside the cell.
//! Paddy and wetland plants may rise above their water cell; the cell above stays air.
use crate::BlockId as B;
use glam::{IVec3, Vec3};

/// One plant part in cell units; `color` overrides the block colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub lo: Vec3,
    pub hi: Vec3,
    pub color: Option<[f32; 3]>,
    /// Root, left edge, raised midrib, right edge and tip of a folded leaf.
    pub leaf: Option<[Vec3; 5]>,
    /// Drawn with the Leaves surface so it continues the canopy around it.
    pub canopy: bool,
}

const LEAF: [f32; 3] = [0.24, 0.46, 0.18];
const DARK_LEAF: [f32; 3] = [0.17, 0.34, 0.13];
const STEM: [f32; 3] = [0.30, 0.42, 0.18];
const WOOD: [f32; 3] = [0.42, 0.30, 0.18];

pub fn is_flora(b: B) -> bool {
    (B::Kelp as u16..=B::IronFern as u16).contains(&(b as u16)) && !matches!(b, B::Pumpkin | B::Melon)
        || b == B::CropHarvester
}

/// Plants that occupy a water cell: the mesher keeps the water around and over them.
pub fn is_submerged(b: B) -> bool {
    matches!(b, B::Kelp | B::Seagrass | B::WaterShoot | B::RiceCrop | B::Cattail)
}

/// Plants hung from the block above them.
pub fn hangs(b: B) -> bool {
    matches!(b, B::CaveVine | B::GlowBerries | B::SporeBlossom)
}

/// The item a broken flora cell drops outside a plot: bare sprouts give their seed.
pub fn canonical(b: B) -> B {
    match b {
        B::CaveVine => B::GlowBerries,
        B::BerryBush => B::SweetBerries,
        B::TrellisVine => B::Grapes,
        B::GourdVine => B::Pumpkin,
        B::WaterShoot => B::Seagrass,
        B::FruitBud => B::Leaves,
        _ => b,
    }
}

fn hash(p: IVec3, salt: u32) -> u32 {
    engine_core::hash::hash_u32(
        (p.x as u32).wrapping_mul(0x9e37_79b9)
            ^ (p.y as u32).wrapping_mul(0x85eb_ca6b)
            ^ (p.z as u32).wrapping_mul(0xc2b2_ae35)
            ^ salt.wrapping_mul(0x27d4_eb2f),
    )
}

fn unit(h: u32, bits: u32) -> f32 {
    (h >> bits & 0xFF) as f32 / 255.0
}

fn part(lo: [f32; 3], hi: [f32; 3], color: Option<[f32; 3]>) -> Part {
    Part { lo: Vec3::from_array(lo), hi: Vec3::from_array(hi), color, leaf: None, canopy: false }
}

/// Square column centred on (x, z).
fn post(x: f32, z: f32, w: f32, y0: f32, y1: f32, color: Option<[f32; 3]>) -> Part {
    let h = w * 0.5;
    part([x - h, y0, z - h], [x + h, y1, z + h], color)
}

/// Small cubes scattered over a region, placed by the cell hash.
fn dots(out: &mut Vec<Part>, p: IVec3, salt: u32, n: u32, lo: Vec3, hi: Vec3, size: f32, color: [f32; 3]) {
    for i in 0..n {
        let h = hash(p, salt + i);
        let half = Vec3::splat(size * 0.5);
        let c = (lo + (hi - lo) * Vec3::new(unit(h, 0), unit(h, 8), unit(h, 16))).clamp(half, Vec3::ONE - half);
        out.push(part((c - half).to_array(), (c + half).to_array(), Some(color)));
    }
}

/// Upright blades or stems at hashed spots, `n` of them, heights `h0..h1`.
fn blades(out: &mut Vec<Part>, p: IVec3, salt: u32, n: u32, w: f32, h0: f32, h1: f32, color: Option<[f32; 3]>) -> Vec<(f32, f32, f32)> {
    let mut tops = Vec::with_capacity(n as usize);
    for i in 0..n {
        let h = hash(p, salt + i);
        let x = 0.15 + unit(h, 0) * 0.7;
        let z = 0.15 + unit(h, 8) * 0.7;
        let top = h0 + unit(h, 16) * (h1 - h0);
        out.push(post(x, z, w, 0.0, top, color));
        tops.push((x, z, top));
    }
    tops
}

fn orchard_bough(out: &mut Vec<Part>, p: IVec3, fruit: B) {
    // Orchard trees all grow ordinary Leaves around their fruiting cells. The
    // upper half is a Leaves-textured slab that meets the neighbouring leaf
    // cubes (a hair inside the cell so coplanar faces never fight); the fruit
    // hangs in the open lower half.
    let canopy = [0.30, 0.43, 0.25];
    let e = 0.004;
    let slab = |lo, hi, color| Part { canopy: true, ..part(lo, hi, Some(color)) };
    out.push(slab([e, 0.48, e], [1.0 - e, 1.0 - e, 1.0 - e], canopy));
    out.push(slab([0.13, 0.35, 0.17], [0.85, 0.49, 0.83], canopy.map(|c| c * 0.89)));

    for (i, (x, z)) in [(0.23, 0.28), (0.74, 0.36), (0.49, 0.76)].into_iter().enumerate() {
        let h = hash(p, 0xc1 + i as u32);
        let x = x + (unit(h, 0) - 0.5) * 0.08;
        let y = 0.21 + unit(h, 8) * 0.065;
        out.push(post(x, z, 0.035, y, 0.40, Some(WOOD)));
        match fruit {
            B::Apple => {
                let skin = [0.73, 0.12 + unit(h, 16) * 0.13, 0.07];
                out.push(post(x, z, 0.15, y - 0.14, y - 0.09, Some(skin.map(|c| c * 0.78))));
                out.push(post(x, z, 0.22, y - 0.095, y + 0.035, Some(skin)));
                out.push(post(x - 0.019, z, 0.17, y + 0.03, y + 0.065, Some(skin.map(|c| c * 1.13))));
            }
            B::Cherry => {
                let fork = Vec3::new(x, y + 0.058, z);
                for (dx, dy, skin) in [(-0.055, 0.0, [0.64, 0.055, 0.12]), (0.053, -0.037, [0.48, 0.035, 0.09])] {
                    let tip = Vec3::new(x + dx, y + dy, z);
                    leaf(out, fork, tip, 0.023, 0.0, STEM);
                    out.push(post(tip.x, z, 0.092, tip.y - 0.066, tip.y + 0.006, Some(skin)));
                    out.push(post(tip.x, z, 0.064, tip.y - 0.083, tip.y - 0.06, Some(skin)));
                }
            }
            B::Fig => {
                out.push(post(x, z, 0.066, y + 0.025, y + 0.098, Some([0.37, 0.38, 0.19])));
                out.push(post(x, z, 0.113, y - 0.016, y + 0.037, Some([0.41, 0.25, 0.34])));
                out.push(post(x, z, 0.16, y - 0.095, y - 0.008, Some([0.34, 0.18, 0.30])));
                out.push(post(x, z, 0.106, y - 0.128, y - 0.088, Some([0.27, 0.13, 0.25])));
            }
            _ => {
                let centre = Vec3::new(x, y, z);
                for petal in 0..5 {
                    let ray = shoot_direction(h, petal as f32 * std::f32::consts::TAU / 5.0);
                    leaf(out, centre, centre + ray * 0.072 - Vec3::Y * 0.018, 0.055, 0.012, [0.94, 0.79, 0.82]);
                }
                out.push(post(x, z, 0.035, y - 0.023, y + 0.008, Some([0.93, 0.73, 0.32])));
            }
        }
    }
}

fn leaf(out: &mut Vec<Part>, root: Vec3, tip: Vec3, width: f32, rise: f32, color: [f32; 3]) {
    let across = Vec3::Y.cross(tip - root).normalize_or(Vec3::X);
    let mid = root.lerp(tip, 0.45) + Vec3::Y * rise;
    let ridge = (tip - root).cross(across).normalize_or(Vec3::Y) * (width * 0.16);
    let points = [root, mid - across * width * 0.5, mid + ridge, mid + across * width * 0.5, tip];
    let (lo, hi) = points.iter().fold((root, root), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    let margin = Vec3::splat(0.012);
    let (lo, hi) = ((lo - margin).max(Vec3::ZERO), (hi + margin).min(Vec3::ONE));
    out.push(Part { leaf: Some(points), ..part(lo.to_array(), hi.to_array(), Some(color)) });
}

fn shoot_direction(h: u32, turn: f32) -> Vec3 {
    let angle = unit(h, 0) * std::f32::consts::TAU + turn;
    Vec3::new(angle.cos(), 0.0, angle.sin())
}

fn plant_root(h: u32) -> Vec3 {
    Vec3::new(0.44 + unit(h, 8) * 0.12, 0.012, 0.44 + unit(h, 16) * 0.12)
}

fn berry_bush(out: &mut Vec<Part>, p: IVec3, ripe: bool) {
    let h = hash(p, 0xbe);
    let root = plant_root(h);
    let spreading = h & 0x1000 != 0;
    let count = 3 + h % 3;
    for i in 0..count {
        let pick = hash(p, 0x87 + i);
        let dir = shoot_direction(h, std::f32::consts::TAU * i as f32 / count as f32);
        let reach = if spreading { 0.21 } else { 0.13 } + unit(pick, 8) * 0.065;
        let height = if spreading { 0.23 } else { 0.37 } + unit(pick, 16) * 0.24;
        let tip = root + dir * reach + Vec3::Y * height;
        leaf(out, root, tip, 0.045, 0.0, WOOD);
        for (along, side) in [(0.48, 0.85), (0.72, -0.63), (0.88, 0.26)] {
            let base = root.lerp(tip, along);
            let limb = (dir * 0.66 + Vec3::Y.cross(dir) * side) * 0.105;
            let pigment = 0.80 + 0.25 * unit(pick, 24) + along * 0.1;
            leaf(out, base, base + limb + Vec3::Y * 0.064, 0.14, 0.02, LEAF.map(|c| c * pigment));
        }
        if ripe && (pick & 3 != 0 || i == 0) {
            let berry = root.lerp(tip, 0.68) + dir * 0.054 - Vec3::Y * 0.038;
            out.push(post(berry.x, berry.z, 0.068, berry.y, berry.y + 0.055, Some([0.68, 0.10, 0.13])));
        }
    }
}

fn lavender(out: &mut Vec<Part>, p: IVec3) {
    let h = hash(p, 0x1a);
    let root = plant_root(h);
    let spreading = h & 0x1000 != 0;
    let count = 3 + h % 4;
    for i in 0..count {
        let pick = hash(p, 0x2e + i);
        let spacing = std::f32::consts::TAU * i as f32 / count as f32;
        let dir = shoot_direction(h, spacing + (unit(pick, 8) - 0.5) * 0.44);
        let cross = Vec3::Y.cross(dir);
        let base = root + dir * if spreading { 0.042 } else { 0.014 };
        let reach = if spreading { 0.20 } else { 0.10 } + unit(pick, 8) * 0.065;
        let height = if spreading { 0.23 } else { 0.38 } + unit(pick, 16) * 0.24;
        let tip = base + dir * reach + Vec3::Y * height;
        leaf(out, base, tip, 0.035, 0.0, [0.33, 0.42, 0.28]);
        for (along, side) in [(0.15, -0.60), (0.29, 0.80), (0.48, -0.34)] {
            let node = base.lerp(tip, along);
            let reach = (0.145 - along * 0.1) * (0.86 + unit(pick, 24) * 0.23);
            let end = node + (dir * 0.85 + cross * side) * reach + Vec3::Y * 0.08;
            leaf(out, node, end, 0.065 - along * 0.025, 0.017, [0.43, 0.51, 0.38]);
        }
        if pick & 7 != 0 || i == 0 {
            let bloom = [0.52, 0.42, 0.65].map(|c| c * (0.83 + 0.26 * unit(pick, 24)));
            let crown = tip + dir * 0.041 + Vec3::Y * (0.085 + 0.07 * unit(pick, 0));
            leaf(out, tip - Vec3::Y * 0.027, crown, 0.073, 0.012, bloom);
            leaf(out, tip + cross * 0.032, crown - cross * 0.020, 0.047, 0.0, bloom.map(|c| c * 0.86));
        }
    }
}

fn agave(out: &mut Vec<Part>, p: IVec3) {
    let h = hash(p, 0xa9a);
    let root = plant_root(h);
    let compact = h & 0x8000 != 0;
    let count = 6 + h % 4;
    for i in 0..count {
        let pick = hash(p, 0x8e + i);
        let dir = shoot_direction(h, i as f32 * 2.40);
        let rank = i as f32 / count as f32;
        let reach = (0.16 + 0.20 * rank + 0.045 * unit(pick, 8))
            * if compact { 0.81 } else { 1.0 };
        let height = (0.52 - 0.35 * rank + 0.17 * unit(pick, 16))
            * if compact { 1.0 } else { 0.78 };
        let pigment = 0.77 + 0.30 * unit(pick, 24);
        let tint = if compact { [0.32, 0.50, 0.39] } else { [0.38, 0.50, 0.31] };
        leaf(out, root + Vec3::Y * 0.028, root + dir * reach + Vec3::Y * height,
            0.088 + 0.085 * rank, 0.048 + 0.065 * rank, tint.map(|c| c * pigment));
    }
}

fn orchid(out: &mut Vec<Part>, p: IVec3) {
    let h = hash(p, 0x04c);
    let root = plant_root(h);
    for i in 0..4 {
        let pick = hash(p, 0xc0 + i);
        let dir = shoot_direction(h, i as f32 * 1.65);
        leaf(out, root, root + dir * (0.21 + 0.10 * unit(pick, 8)) + Vec3::Y * (0.11 + 0.10 * unit(pick, 16)),
            0.11, 0.045, LEAF);
    }
    let dir = shoot_direction(h, 0.4);
    let tip = root + dir * 0.11 + Vec3::Y * (0.41 + 0.20 * unit(h, 0));
    leaf(out, root, tip, 0.027, 0.02, STEM);
    for i in 0..5 {
        let ray = shoot_direction(h, i as f32 * 1.25);
        let pale = 0.80 + 0.17 * unit(h, 16);
        let color = [pale, 0.48 + 0.12 * unit(h, 8), 0.68 + 0.14 * unit(h, 24)];
        leaf(out, tip, tip + ray * (0.09 + 0.035 * (i % 2) as f32) + Vec3::Y * 0.05, 0.083, 0.018, color);
    }
    out.push(post(tip.x, tip.z, 0.047, tip.y, tip.y + 0.043, Some([0.94, 0.72, 0.35])));
}

mod grapevine;

/// Parts for a flora block at `p`, or `None` for any other block.
pub fn parts(b: B, p: IVec3) -> Option<Vec<Part>> {
    if !is_flora(b) {
        return None;
    }
    let mut out = Vec::with_capacity(16);
    let o = &mut out;
    match b {
        B::Kelp => {
            o.push(post(0.5, 0.5, 0.1, 0.0, 1.0, None));
            for i in 0..3u32 {
                let y = 0.1 + i as f32 * 0.32;
                let (a, c) = if (hash(p, i) & 1) == 0 { (0.1, 0.46) } else { (0.54, 0.9) };
                o.push(part([a, y, 0.44], [c, y + 0.2, 0.56], None));
            }
        }
        B::Seagrass => {
            blades(o, p, 0x5e, 6, 0.06, 0.4, 0.95, None);
        }
        B::WaterShoot => {
            o.push(post(0.5, 0.5, 0.06, 0.0, 0.3, None));
            o.push(part([0.3, 0.2, 0.46], [0.7, 0.28, 0.54], None));
        }
        B::LilyPad => {
            o.push(part([0.1, 0.0, 0.1], [0.9, 0.04, 0.9], None));
            if hash(p, 0x11) % 3 == 0 {
                o.push(part([0.42, 0.04, 0.42], [0.58, 0.14, 0.58], Some([0.92, 0.84, 0.88])));
            }
        }
        B::Cattail => {
            for (x, z, top) in blades(o, p, 0xca, 4, 0.05, 1.3, 1.8, Some(STEM)) {
                o.push(post(x, z, 0.1, top - 0.36, top - 0.08, Some([0.40, 0.26, 0.13])));
            }
            blades(o, p, 0xcb, 3, 0.04, 1.0, 1.5, Some(LEAF));
        }
        B::SeaOats => {
            for (x, z, top) in blades(o, p, 0x0a, 5, 0.04, 0.6, 1.0, Some([0.62, 0.62, 0.36])) {
                o.push(post(x, z, 0.08, top - 0.22, top, None));
            }
        }
        B::MangrovePropagule => {
            o.push(post(0.5, 0.5, 0.08, 0.0, 0.62, Some([0.40, 0.48, 0.22])));
            o.push(part([0.3, 0.55, 0.46], [0.7, 0.62, 0.54], Some(LEAF)));
            o.push(part([0.46, 0.55, 0.3], [0.54, 0.62, 0.7], Some(LEAF)));
        }
        B::MangroveRoots => {
            let h = hash(p, 0x40);
            for (i, (x, z)) in [(0.2, 0.2), (0.8, 0.25), (0.25, 0.8), (0.78, 0.78)].into_iter().enumerate() {
                if (h >> i) & 1 == 0 || i == 0 {
                    o.push(post(x, z, 0.12, 0.0, 1.0, None));
                }
            }
            o.push(part([0.2, 0.84, 0.44], [0.8, 0.96, 0.56], None));
            o.push(part([0.44, 0.6, 0.2], [0.56, 0.72, 0.8], None));
        }
        B::BerryBush | B::SweetBerries => berry_bush(o, p, b == B::SweetBerries),
        B::FruitBud | B::Apple | B::Cherry | B::Fig => orchard_bough(o, p, b),
        B::CocoaPod => {
            o.push(post(0.5, 0.5, 0.05, 0.7, 1.0, Some(WOOD)));
            o.push(post(0.5, 0.5, 0.3, 0.24, 0.72, None));
        }
        B::TrellisVine | B::Grapes => grapevine::build(o, p, b == B::Grapes),
        B::Tomato => {
            o.push(post(0.5, 0.5, 0.06, 0.0, 1.0, Some(WOOD)));
            o.push(part([0.22, 0.1, 0.22], [0.78, 0.86, 0.78], Some(LEAF)));
            dots(o, p, 0x70, 5, Vec3::new(0.2, 0.18, 0.2), Vec3::new(0.8, 0.8, 0.8), 0.14, [0.86, 0.16, 0.10]);
        }
        B::Potato => {
            o.push(part([0.2, 0.0, 0.2], [0.8, 0.36, 0.8], None));
            o.push(part([0.3, 0.36, 0.3], [0.7, 0.5, 0.7], Some(DARK_LEAF)));
            dots(o, p, 0x90, 3, Vec3::new(0.3, 0.5, 0.3), Vec3::new(0.7, 0.54, 0.7), 0.07, [0.92, 0.90, 0.84]);
        }
        B::Onion => {
            o.push(post(0.5, 0.5, 0.26, 0.0, 0.16, Some([0.80, 0.66, 0.46])));
            blades(o, p, 0x01, 5, 0.05, 0.5, 0.8, None);
        }
        B::Cabbage => {
            o.push(part([0.18, 0.0, 0.18], [0.82, 0.5, 0.82], None));
            o.push(part([0.06, 0.0, 0.3], [0.94, 0.22, 0.7], Some([0.40, 0.60, 0.28])));
            o.push(part([0.3, 0.0, 0.06], [0.7, 0.22, 0.94], Some([0.40, 0.60, 0.28])));
        }
        B::GourdVine => {
            o.push(part([0.1, 0.0, 0.46], [0.9, 0.06, 0.54], None));
            for (x, z) in [(0.22, 0.34), (0.5, 0.62), (0.76, 0.36)] {
                o.push(part([x - 0.12, 0.04, z - 0.12], [x + 0.12, 0.2, z + 0.12], Some(LEAF)));
            }
        }
        B::RiceCrop => {
            for (x, z, top) in blades(o, p, 0x21, 7, 0.04, 1.1, 1.5, Some([0.46, 0.60, 0.26])) {
                o.push(post(x + 0.02, z, 0.07, top - 0.2, top, None));
            }
        }
        B::Lavender => lavender(o, p),
        B::Orchid => orchid(o, p),
        B::Cactus => {
            o.push(post(0.5, 0.5, 0.56, 0.0, 1.0, None));
            let h = hash(IVec3::new(p.x, 0, p.z), 0xcc);
            if h % 3 == 0 && p.y.rem_euclid(2) == 0 {
                let side = if h & 8 == 0 { (0.0, 0.22) } else { (0.78, 1.0) };
                o.push(part([side.0, 0.34, 0.42], [side.1, 0.5, 0.58], None));
                let x = (side.0 + side.1) * 0.5;
                o.push(post(x, 0.5, 0.16, 0.34, 0.86, None));
            }
        }
        B::Agave => agave(o, p),
        B::CaveVine | B::GlowBerries => {
            for i in 0..3u32 {
                let h = hash(IVec3::new(p.x, 0, p.z), i);
                let x = 0.2 + i as f32 * 0.3 + (unit(h, 0) - 0.5) * 0.1;
                let z = 0.3 + unit(h, 8) * 0.4;
                o.push(post(x, z, 0.06, 0.0, 1.0, Some(STEM)));
                o.push(part([x - 0.1, 0.4 + unit(h, 16) * 0.3, z - 0.04], [x + 0.1, 0.5 + unit(h, 16) * 0.3, z + 0.04], Some(LEAF)));
            }
            if b == B::GlowBerries {
                dots(o, p, 0x6b, 5, Vec3::new(0.18, 0.1, 0.25), Vec3::new(0.82, 0.9, 0.75), 0.11, [1.0, 0.70, 0.22]);
            }
        }
        B::Dripleaf => {
            o.push(post(0.5, 0.5, 0.07, 0.0, 0.84, Some(STEM)));
            o.push(part([0.06, 0.82, 0.06], [0.94, 0.88, 0.94], None));
        }
        B::SporeBlossom => {
            o.push(part([0.2, 0.9, 0.2], [0.8, 1.0, 0.8], Some(LEAF)));
            for (lo, hi) in [([0.3, 0.76, 0.42], [0.7, 0.9, 0.58]), ([0.42, 0.76, 0.3], [0.58, 0.9, 0.7])] {
                o.push(part(lo, hi, None));
            }
            o.push(post(0.5, 0.5, 0.1, 0.68, 0.78, Some([0.96, 0.82, 0.86])));
        }
        B::ManaFlower => {
            o.push(post(0.5, 0.5, 0.05, 0.0, 0.6, Some(STEM)));
            o.push(part([0.28, 0.54, 0.42], [0.72, 0.66, 0.58], None));
            o.push(part([0.42, 0.54, 0.28], [0.58, 0.66, 0.72], None));
            o.push(post(0.5, 0.5, 0.1, 0.6, 0.72, Some([0.86, 0.94, 1.0])));
        }
        B::CollectorFlower => {
            o.push(post(0.5, 0.5, 0.05, 0.0, 0.5, Some(STEM)));
            o.push(part([0.26, 0.5, 0.26], [0.74, 0.58, 0.74], None));
            o.push(part([0.3, 0.58, 0.3], [0.7, 0.74, 0.7], None));
            o.push(post(0.5, 0.5, 0.16, 0.56, 0.66, Some([0.86, 0.42, 0.10])));
        }
        B::IronFern => {
            for (x, z, top) in blades(o, p, 0x1f, 5, 0.05, 0.5, 0.9, None) {
                let dx = if x < 0.5 { 0.12 } else { -0.12 };
                o.push(part([x.min(x + dx), top - 0.3, z - 0.03], [x.max(x + dx), top - 0.24, z + 0.03], None));
                o.push(post(x, z, 0.07, top - 0.08, top, Some([0.56, 0.34, 0.22])));
            }
        }
        B::CropHarvester => {
            // Post, cross arm and a hanging blade: reads as a tool, not a plant.
            o.push(post(0.5, 0.5, 0.14, 0.0, 0.9, Some(WOOD)));
            o.push(part([0.1, 0.78, 0.44], [0.9, 0.88, 0.56], Some(WOOD)));
            o.push(part([0.1, 0.48, 0.47], [0.18, 0.78, 0.53], None));
            o.push(part([0.82, 0.48, 0.47], [0.9, 0.78, 0.53], None));
            o.push(post(0.5, 0.5, 0.3, 0.0, 0.08, Some([0.34, 0.34, 0.36])));
        }
        _ => return None,
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_flora_block_has_parts_inside_its_column() {
        for &b in crate::ALL_BLOCK_IDS.iter().filter(|&&b| is_flora(b)) {
            for p in [IVec3::ZERO, IVec3::new(-77, -5, 1 << 20), IVec3::new(1 << 20, 300, -(1 << 20))] {
                let parts = parts(b, p).unwrap_or_default();
                assert!(!parts.is_empty(), "{b:?}");
                for part in parts {
                    assert!(part.lo.cmplt(part.hi).all(), "{b:?} {part:?}");
                    assert!(part.lo.cmpge(Vec3::splat(-1e-4)).all(), "{b:?} {part:?}");
                    let top = if matches!(b, B::Cattail | B::RiceCrop) { 2.0 } else { 1.0 };
                    assert!(part.hi.x <= 1.0001 && part.hi.z <= 1.0001 && part.hi.y <= top, "{b:?} {part:?}");
                }
            }
        }
        assert!(!is_flora(B::Pumpkin) && !is_flora(B::Melon) && !is_flora(B::SulfuricAcid));
    }
}
