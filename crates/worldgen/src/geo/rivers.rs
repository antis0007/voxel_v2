//! River channels traced along the planet's receiver network.
//!
//! A river is a chain of quadratic curves through jittered cell nodes, from
//! the midpoint of each link to the next, so it bends smoothly and never
//! follows the grid. Width and depth grow with the square root of discharge.

use super::planet::Planet;
use super::SEA_M;

/// Rainfall-weighted upstream area at which a channel holds water.
pub const SOURCE_AREA_M2: f32 = 2.5e6;

#[derive(Clone, Copy, Debug)]
pub struct Channel {
    /// Metres from the channel centre line.
    pub distance: f64,
    pub half_width: f64,
    pub depth: f64,
    pub water_m: f64,
}

fn level(p: &Planet, i: usize) -> f64 {
    let lake = p.lake[i];
    if lake > f32::MIN { lake as f64 } else { p.height[i] as f64 }
}

fn node(p: &Planet, i: usize) -> [f64; 2] {
    [p.node[i][0] as f64, p.node[i][1] as f64]
}

fn mid(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]
}

fn bezier(a: [f64; 2], c: [f64; 2], b: [f64; 2], t: f64) -> [f64; 2] {
    let s = 1.0 - t;
    [s * s * a[0] + 2.0 * s * t * c[0] + t * t * b[0], s * s * a[1] + 2.0 * s * t * c[1] + t * t * b[1]]
}

/// Closest point on segment `a`-`b`: (squared distance, parameter).
fn project(q: [f64; 2], a: [f64; 2], b: [f64; 2]) -> (f64, f64) {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dz * dz;
    let t = if len2 > 0.0 { (((q[0] - a[0]) * dx + (q[1] - a[1]) * dz) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (px, pz) = (a[0] + dx * t - q[0], a[1] + dz * t - q[1]);
    (px * px + pz * pz, t)
}

const SAMPLES: usize = 8;

/// The nearest channel whose banks reach `(x, z)`, if any.
pub fn near(p: &Planet, x: f64, z: f64) -> Option<Channel> {
    let g = &p.grid;
    let (cx, cz, _, _) = g.cell(x, z);
    let n = g.n as i64;
    let mut best: Option<(f64, Channel)> = None;
    for dz in -2..=2i64 {
        for dx in -2..=2i64 {
            let (a, b) = (cx as i64 + dx, cz as i64 + dz);
            if a < 0 || b < 0 || a >= n || b >= n {
                continue;
            }
            let i = (b * n + a) as usize;
            let area = p.discharge[i];
            let r = p.receiver[i] as usize;
            if area < SOURCE_AREA_M2 || r == i || !p.land[i] {
                continue;
            }
            let rr = p.receiver[r] as usize;
            let (li, mut lr) = (level(p, i), level(p, r));
            lr = lr.min(li);
            let lrr = level(p, rr).min(lr);
            let start = mid(node(p, i), node(p, r));
            let end = if rr == r { node(p, r) } else { mid(node(p, r), node(p, rr)) };
            let ctrl = node(p, r);
            let (mut d2, mut t) = project([x, z], node(p, i), start);
            let mut on_curve = false;
            let mut prev = start;
            for k in 1..=SAMPLES {
                let cur = bezier(start, ctrl, end, k as f64 / SAMPLES as f64);
                let (e2, u) = project([x, z], prev, cur);
                if e2 < d2 {
                    (d2, t, on_curve) = (e2, (k as f64 - 1.0 + u) / SAMPLES as f64, true);
                }
                prev = cur;
            }
            let water = if on_curve {
                let (l0, l1, l2) = ((li + lr) * 0.5, lr, (lr + lrr) * 0.5);
                let s = 1.0 - t;
                s * s * l0 + 2.0 * s * t * l1 + t * t * l2
            } else {
                li + ((li + lr) * 0.5 - li) * t
            };
            let km2 = (area as f64 / 1.0e6).sqrt();
            let half = (1.2 + 0.9 * km2).min(26.0);
            let channel = Channel {
                distance: d2.sqrt(),
                half_width: half,
                depth: (0.6 + 0.25 * km2).min(5.0),
                water_m: (water - 0.35).max(SEA_M),
            };
            let score = channel.distance - half;
            if best.is_none_or(|(s, _)| score < s) {
                best = Some((score, channel));
            }
        }
    }
    best.map(|(_, c)| c).filter(|c| c.distance < c.half_width * 4.0 + 12.0)
}

/// Surface of a lake covering `(x, z)`, if any.
pub fn lake(p: &Planet, x: f64, z: f64) -> Option<f64> {
    let g = &p.grid;
    let (cx, cz, _, _) = g.cell(x, z);
    let i = cz * g.n + cx;
    let top = [i, i + 1, i + g.n, i + g.n + 1].iter().map(|&k| p.lake[k]).fold(f32::MIN, f32::max);
    (top > f32::MIN).then_some(top as f64)
}
