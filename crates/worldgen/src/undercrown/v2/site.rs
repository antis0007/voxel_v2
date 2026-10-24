//! Choosing the Undercrown's hill. The complex is authored once at the
//! origin to learn its roof profile: for a grid of points under every
//! room, cavern and passage, how high its open space reaches over the
//! reference level. A candidate's cover is the least ground over that
//! profile; the best candidate wins, and if even it is thin the whole
//! complex sinks until the roof keeps `COVER` metres of rock.
use super::Shape;
use super::layout::{build, surface};
use super::shape::ERODE;
use crate::{WorldSampleConfig, hash2};

/// Rock wanted over the highest open cell, metres.
pub(super) const COVER: f32 = 6.0;
/// Lowest reference level: the deepest room stays well clear of bedrock.
const LOWEST: f32 = 30.0;
/// Profile grid step, metres.
const STEP: f32 = 4.0;

/// Points of a box on the profile grid, with their roof heights.
fn grid(out: &mut Vec<(f32, f32, f32)>, x0: f32, x1: f32, z0: f32, z1: f32, top: impl Fn(f32, f32) -> Option<f32>) {
    let mut x = x0;
    while x <= x1 + STEP * 0.5 {
        let mut z = z0;
        while z <= z1 + STEP * 0.5 {
            let (px, pz) = (x.min(x1), z.min(z1));
            if let Some(t) = top(px, pz) {
                out.push((px, pz, t));
            }
            z += STEP;
        }
        x += STEP;
    }
}

/// (x offset, z offset, top over the reference level) under the roof.
fn profile(cfg: &WorldSampleConfig) -> Vec<(f32, f32, f32)> {
    let (_, _, probe) = build(cfg, 0.0, 0.0, 0.0, 0.0);
    let mut out = Vec::new();
    for sp in &probe.spaces {
        match sp.shape {
            Shape::Hall { x0, x1, z0, z1, floor, h, rise } => {
                let top = floor + h + rise + ERODE;
                grid(&mut out, x0 - ERODE, x1 + ERODE, z0 - ERODE, z1 + ERODE, |_, _| Some(top));
            }
            Shape::Round { x, z, r, floor, h, dome } => grid(&mut out, x - r, x + r, z - r, z + r, |_, _| Some(floor + h + dome)),
            Shape::Cave { c, rad, .. } => {
                // The noisy shell reaches about 1.12 radii; above that, rock.
                let reach = 1.25f32;
                grid(&mut out, c.x - rad.x * 1.12, c.x + rad.x * 1.12, c.z - rad.z * 1.12, c.z + rad.z * 1.12, |x, z| {
                    let q = ((x - c.x) / rad.x).powi(2) + ((z - c.z) / rad.z).powi(2);
                    (q < reach).then(|| c.y + rad.y * (reach - q).sqrt())
                });
            }
            // Shafts are meant to reach the surface.
            Shape::Shaft { .. } => {}
        }
    }
    for t in &probe.tunnels {
        let n = (t.length() / STEP).ceil().max(1.0) as i32;
        for k in 0..=n {
            let p = t.a + (t.b - t.a) * (k as f32 / n as f32);
            out.push((p.x, p.z, p.y + t.h + 0.6));
        }
    }
    out
}

/// Least rock over the profile at a candidate, giving up once it falls
/// under `floor`.
fn cover(cfg: &WorldSampleConfig, profile: &[(f32, f32, f32)], x: f32, z: f32, s: f32, floor: f32) -> f32 {
    let mut least = f32::MAX;
    for &(ox, oz, top) in profile {
        least = least.min(surface(cfg, x + ox, z + oz) - (s + top));
        if least < floor {
            break;
        }
    }
    least
}

/// (anchor x, anchor z, ground at the anchor, reference level).
pub(super) fn site(cfg: &WorldSampleConfig) -> (f32, f32, f32, f32) {
    let profile = profile(cfg);
    let fallback = (160.0, 80.0, surface(cfg, 160.0, 80.0).round());
    let mut best = (f32::MIN, fallback.0, fallback.1, fallback.2);
    for k in 0..64 {
        let a = hash2(k, 1, cfg.seed.wrapping_add(4401)) * std::f32::consts::TAU;
        let r = 220.0 + hash2(k, 2, cfg.seed.wrapping_add(4401)) * 260.0;
        let (x, z) = ((a.cos() * r).round(), (a.sin() * r).round());
        let s = surface(cfg, x, z).round();
        if !(48.0..150.0).contains(&s) || crate::valley::reserves(cfg, x, z) {
            continue;
        }
        let margin = cover(cfg, &profile, x, z, s, best.0);
        if margin > best.0 {
            best = (margin, x, z, s);
        }
        if margin >= COVER {
            break;
        }
    }
    let (margin, x, z, ground) = best;
    let sink = if margin == f32::MIN { 0.0 } else { (COVER - margin).max(0.0) };
    let s = (ground - sink).max(LOWEST.min(ground)).round();
    (x, z, ground, s)
}
