//! Plate layout and the uplift field it drives.
//!
//! Convergent boundaries raise long belts whose strength varies along strike,
//! so ranges have ends, saddles and passes. Divergent boundaries on land sink
//! into rift basins that the drainage solve turns into lakes.

use super::grid::Grid;
use super::noise::{bump, fbm, smoothstep, unit};
use super::PlanetParams;

pub struct Tectonics {
    /// Uplift per step; negative in rifts.
    pub uplift: Vec<f64>,
    pub land: Vec<bool>,
}

struct Plate {
    site: [f64; 2],
    velocity: [f64; 2],
    continental: bool,
}

const ACROSS: i64 = 5;

fn plates(seed: u32, _p: &PlanetParams) -> Vec<Plate> {
    let cell = super::HOME.face_edge_m / ACROSS as f64;
    let half = super::HOME.face_edge_m * 0.5;
    let mut out = Vec::new();
    for j in -1..=ACROSS {
        for i in -1..=ACROSS {
            let jitter = |salt: u32| 0.15 + 0.7 * unit(i, j, seed.wrapping_add(salt));
            let site = [-half + (i as f64 + jitter(11)) * cell, -half + (j as f64 + jitter(12)) * cell];
            let (vx, vz) = (unit(i, j, seed.wrapping_add(13)) * 2.0 - 1.0, unit(i, j, seed.wrapping_add(14)) * 2.0 - 1.0);
            let len = (vx * vx + vz * vz).sqrt().max(1e-6);
            let speed = 0.4 + 0.6 * unit(i, j, seed.wrapping_add(15));
            out.push(Plate {
                site,
                velocity: [vx / len * speed, vz / len * speed],
                continental: unit(i, j, seed.wrapping_add(16)) < 0.72,
            });
        }
    }
    // The home plate is continental so the spawn region is land.
    let home = nearest_two(&out, [0.0, 0.0]).0;
    out[home].continental = true;
    out
}

fn nearest_two(plates: &[Plate], q: [f64; 2]) -> (usize, usize) {
    let (mut a, mut b) = ((usize::MAX, f64::MAX), (usize::MAX, f64::MAX));
    for (k, pl) in plates.iter().enumerate() {
        let d = (q[0] - pl.site[0]) * (q[0] - pl.site[0]) + (q[1] - pl.site[1]) * (q[1] - pl.site[1]);
        if d < a.1 {
            b = a;
            a = (k, d);
        } else if d < b.1 {
            b = (k, d);
        }
    }
    (a.0, b.0)
}

pub fn build(grid: &Grid, seed: u32, p: &PlanetParams) -> Tectonics {
    let plates = plates(seed, p);
    let half = super::HOME.face_edge_m * 0.5;
    let compression = super::HOME.face_edge_m / p.face_edge_m;
    let mut uplift = vec![0.0; grid.len()];
    let mut land = vec![false; grid.len()];
    for i in 0..grid.len() {
        let [x, z] = grid.at(i);
        let (x, z) = (x * compression, z * compression);
        let warp = |salt: u32| (fbm(x / 7000.0, z / 7000.0, seed.wrapping_add(salt), 3) - 0.5) * 6000.0;
        let q = [(x + warp(21)) * p.continent_scale, (z + warp(22)) * p.continent_scale];
        let (a, b) = nearest_two(&plates, q);
        let (pa, pb) = (&plates[a], &plates[b]);
        let n = [pb.site[0] - pa.site[0], pb.site[1] - pa.site[1]];
        let sep = (n[0] * n[0] + n[1] * n[1]).sqrt().max(1.0);
        let da = (q[0] - pa.site[0]) * (q[0] - pa.site[0]) + (q[1] - pa.site[1]) * (q[1] - pa.site[1]);
        let db = (q[0] - pb.site[0]) * (q[0] - pb.site[0]) + (q[1] - pb.site[1]) * (q[1] - pb.site[1]);
        // Distance from the shared boundary (the bisector of the two sites).
        let d = (db - da) / (2.0 * sep);
        let closing = ((pa.velocity[0] - pb.velocity[0]) * n[0] + (pa.velocity[1] - pb.velocity[1]) * n[1]) / sep;

        let ca = if pa.continental { 1.0 } else { 0.0 };
        let cb = if pb.continental { 1.0 } else { 0.0 };
        let mut crust = (ca + cb) * 0.5 + (ca - (ca + cb) * 0.5) * smoothstep(0.0, 2600.0, d);
        // Two fractal scales give bays, headlands and islands instead of plate chords.
        crust += (fbm(x / 4200.0, z / 4200.0, seed.wrapping_add(23), 5) - 0.5) * 1.1;
        crust += (fbm(x / 900.0, z / 900.0, seed.wrapping_add(26), 3) - 0.5) * 0.3;
        let edge = (half - x.abs()).min(half - z.abs());
        crust -= 1.0 - smoothstep(1200.0, 5200.0, edge);
        land[i] = crust > 0.5 + p.ocean_bias;

        let along = 0.35 + 0.65 * fbm(q[0] / 5600.0, q[1] / 5600.0, seed.wrapping_add(24), 3);
        // Patches of stronger craton uplift become dissected hill country between the ranges.
        let hills = fbm(x / 3800.0, z / 3800.0, seed.wrapping_add(25), 4);
        let mut u = 0.06 + 0.5 * hills * hills * hills;
        if p.face_edge_m <= super::EXPLORATION_HOME.face_edge_m {
            // A finite uplift spur within an ordinary walk of the home craton.
            // Erosion and drainage still carve its passes and tributaries.
            let angle=unit(0,0,seed.wrapping_add(47))*std::f64::consts::TAU;
            let (sin,cos)=angle.sin_cos();
            let (wx,wz)=(x/compression,z/compression);
            let across=wx*cos+wz*sin;
            let along=-wx*sin+wz*cos;
            u += 1.6*bump((across-600.0).abs(),210.0)*bump(along.abs(),850.0);
        }
        if closing > 0.0 {
            // Ocean crust subducting under land raises the arc on the land side.
            let offset = if pa.continental != pb.continental { 900.0 } else { 0.0 };
            u += closing * bump((d - offset).abs(), 2000.0) * along * 1.6;
        } else {
            u += closing * bump(d, 1500.0) * along * 0.25;
        }
        // Spawn sits on stable craton: no rift basin floods it.
        let home = smoothstep(1500.0, 3500.0, (x * x + z * z).sqrt());
        uplift[i] = u.max(-0.12).max(0.06 * (1.0 - home));
    }
    Tectonics { uplift, land }
}
