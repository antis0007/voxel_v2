//! Ice-wedge polygon ground on flat permafrost.
//!
//! Frost cracks join into a network of polygons 10-20 m across. Wedges of
//! ground ice grow under the cracks and raise rims beside them; cold wet
//! polygons hold a pond in the centre (low-centred). As the ground warms the
//! wedge tops melt, troughs deepen and flood, and centres stand proud
//! (high-centred). Depths are exaggerated so one-metre voxels show them.

use super::noise::{smoothstep, unit};

const CELL_M: f64 = 14.0;
const WEDGE_DEPTH_M: f64 = 4.0;

/// Static retrogressive thaw scars on wet, seasonally thawing permafrost.
/// Their long axis follows the local downhill gradient; cuts never add peaks.
pub fn slump(p: &super::Planet, seed: u32, x: f64, z: f64) -> f64 {
    const SPACING: f64 = 192.0;
    let (cx, cz) = ((x / SPACING).floor() as i64, (z / SPACING).floor() as i64);
    let mut cut: f64 = 0.0;
    for dz in -1..=1 {
        for dx in -1..=1 {
            let (a, b) = (cx + dx, cz + dz);
            if unit(a, b, seed ^ 0x51a0) < 0.72 { continue; }
            let sx = (a as f64 + unit(a, b, seed ^ 0x51a1)) * SPACING;
            let sz = (b as f64 + unit(a, b, seed ^ 0x51a2)) * SPACING;
            if (x - sx).abs() > 80.0 || (z - sz).abs() > 80.0 { continue; }
            let g = &p.grid;
            let maat = g.bilinear(&p.maat, sx, sz);
            let summer = maat + g.bilinear(&p.season, sx, sz);
            let wet = g.bilinear(&p.wet, sx, sz);
            if maat >= super::PERMAFROST_C || summer <= 1.0 || wet < 0.35 { continue; }
            let d = g.cell_m;
            let gx = (g.bilinear(&p.height, sx + d, sz) - g.bilinear(&p.height, sx - d, sz)) / (2.0 * d);
            let gz = (g.bilinear(&p.height, sx, sz + d) - g.bilinear(&p.height, sx, sz - d)) / (2.0 * d);
            let slope = (gx * gx + gz * gz).sqrt();
            if !(0.04..0.55).contains(&slope) { continue; }
            let along = -((x - sx) * gx + (z - sz) * gz) / slope;
            let across = ((x - sx) * gz - (z - sz) * gx) / slope;
            let u = (along + 20.0) / 90.0;
            if !(0.0..1.0).contains(&u) { continue; }
            let width = 12.0 + 20.0 * (1.0 - u);
            let cross = 1.0 - smoothstep(0.55, 1.0, across.abs() / width);
            let head = smoothstep(0.0, 0.08, u);
            let toe = 1.0 - smoothstep(0.3, 1.0, u);
            cut = cut.min(-(2.0 + 5.0 * wet) * cross * head * toe);
        }
    }
    cut
}

pub struct Polygon {
    /// Ground offset from the smooth surface.
    pub ground: f64,
    /// Pond surface offset from the smooth surface.
    pub water: Option<f64>,
}

fn crack_distance(seed: u32, x: f64, z: f64) -> (f64, i64, i64) {
    let (cx, cz) = ((x / CELL_M).floor() as i64, (z / CELL_M).floor() as i64);
    let mut sites = [(f64::MAX, 0.0, 0.0, 0i64, 0i64); 2];
    for dz in -1..=1 {
        for dx in -1..=1 {
            let (a, b) = (cx + dx, cz + dz);
            let sx = (a as f64 + 0.15 + 0.7 * unit(a, b, seed ^ 0x1ce)) * CELL_M;
            let sz = (b as f64 + 0.15 + 0.7 * unit(a, b, seed ^ 0x1cf)) * CELL_M;
            let d2 = (x - sx) * (x - sx) + (z - sz) * (z - sz);
            if d2 < sites[0].0 {
                sites[1] = sites[0];
                sites[0] = (d2, sx, sz, a, b);
            } else if d2 < sites[1].0 {
                sites[1] = (d2, sx, sz, a, b);
            }
        }
    }
    let (s0, s1) = (sites[0], sites[1]);
    let sep = ((s1.1 - s0.1) * (s1.1 - s0.1) + (s1.2 - s0.2) * (s1.2 - s0.2)).sqrt().max(1e-6);
    ((s1.0 - s0.0) / (2.0 * sep), s0.3, s0.4)
}

/// Microrelief at `(x, z)` given mean annual temperature and wetness in [0, 1).
pub fn polygon(seed: u32, x: f64, z: f64, maat: f64, wet: f64) -> Polygon {
    let (edge, cx, cz) = crack_distance(seed, x, z);
    let thaw = smoothstep(-9.0, -1.5, maat);
    let width = 1.5 + 1.5 * thaw;
    let depth = 0.9 + 1.1 * thaw;
    if edge < width {
        let k = edge / width;
        let ground = -depth * (1.0 - k * k);
        let flooded = thaw > 0.45 && wet > 0.4;
        return Polygon { ground, water: flooded.then_some(-0.4 * depth) };
    }
    let inside = edge - width;
    let rim = 0.6 * (1.0 - thaw) * (1.0 - smoothstep(0.0, 2.5, inside));
    let crown = 0.5 * thaw * smoothstep(1.0, 4.0, inside);
    let pond = thaw < 0.5 && wet > 0.45 && unit(cx, cz, seed ^ 0x1d0) > 0.35 && inside > 2.5;
    if pond {
        return Polygon { ground: -0.3 - 0.9 * smoothstep(2.5, 4.5, inside), water: Some(-0.3) };
    }
    Polygon { ground: rim + crown, water: None }
}

/// Whether ground `depth_m` below the surface is wedge ice.
pub fn wedge_ice(seed: u32, x: f64, z: f64, depth_m: f64, maat: f64) -> bool {
    if !(0.5..WEDGE_DEPTH_M).contains(&depth_m) {
        return false;
    }
    let (edge, _, _) = crack_distance(seed, x, z);
    // Warmer ground has lost the top of its wedges.
    let melted = 0.5 + 1.5 * smoothstep(-9.0, -1.5, maat);
    depth_m > melted && edge < 1.1 * (1.0 - depth_m / WEDGE_DEPTH_M)
}
