//! Drainage solve fields resampled onto the globe's spawn face.
//!
//! The solve covers one square face with ocean on its border, so it lands on
//! `Face::PosX` (the planet spawn face) and the other faces hold open ocean,
//! or stay unsurveyed on a planet world whose other faces come later.
use engine_core::cube_sphere::{Face, Projection};
use engine_worldgen::geo::{Planet, Preview};

use super::surface::{self, DEEP, Surface, Texel, UNSURVEYED};

/// Texels per face edge; two solve cells per texel at 512 cells.
pub const N: usize = 256;
/// Land relief in km for a normalised preview height of 1.
const PREVIEW_RELIEF_KM: f32 = 0.45;

/// Per-cell fields shared by the preview and the finished solve.
struct Cells<'a> {
    n: usize,
    /// Height above sea in km for land, `None` for sea.
    land_km: Box<dyn Fn(usize) -> Option<f32> + 'a>,
    wet: Option<&'a [f32]>,
    temp_c: Option<&'a [f32]>,
    /// Fresh-water cover of a cell in [0, 1]: rivers and lakes.
    fresh: Box<dyn Fn(usize) -> f32 + 'a>,
    face_km: f32,
    /// Planet world: the other faces belong to the survey, not to this face's sea.
    planet: bool,
}

/// Bilinear sample of `f` at continuous cell coordinates.
fn bilinear(n: usize, x: f32, z: f32, f: &dyn Fn(usize) -> f32) -> f32 {
    let (x, z) = (x.clamp(0.0, (n - 1) as f32), z.clamp(0.0, (n - 1) as f32));
    let (x0, z0) = (x.floor() as usize, z.floor() as usize);
    let (x1, z1) = ((x0 + 1).min(n - 1), (z0 + 1).min(n - 1));
    let (fx, fz) = (x - x0 as f32, z - z0 as f32);
    let row = |z: usize| f(z * n + x0) * (1.0 - fx) + f(z * n + x1) * fx;
    row(z0) * (1.0 - fz) + row(z1) * fz
}

fn build(c: &Cells, others: Texel, key: u64) -> Surface {
    let mut texels = surface::faces(N, others);
    let top = (0..c.n * c.n).filter_map(|i| (c.land_km)(i)).fold(1.0e-3_f32, f32::max);
    // Sea reads as a small negative height so the coast crosses zero between cells.
    let elev = |i: usize| (c.land_km)(i).unwrap_or(-0.05);
    let mut face = vec![Texel::default(); N * N];
    let scale = c.n as f32 / N as f32;
    for j in 0..N {
        for i in 0..N {
            // Cell centres sit at s = -1 + (2x + 1) / n; four taps prefilter the footprint.
            let (s, t) = (surface::grid(i, N) as f32, surface::grid(j, N) as f32);
            let (x, z) = ((s + 1.0) * 0.5 * c.n as f32 - 0.5, (t + 1.0) * 0.5 * c.n as f32 - 0.5);
            let taps = [(-0.25, -0.25), (0.25, -0.25), (-0.25, 0.25), (0.25, 0.25)].map(|(a, b)| (x + a * scale, z + b * scale));
            let avg = |f: &dyn Fn(usize) -> f32| taps.iter().map(|&(x, z)| bilinear(c.n, x, z, f)).sum::<f32>() * 0.25;
            let h = avg(&elev);
            let wet = c.wet.map(|w| avg(&|i| w[i]));
            let temp = c.temp_c.map(|w| avg(&|i| w[i]));
            let fresh = avg(&|i| (c.fresh)(i));
            let k = j * N + i;
            face[k] = Texel { rgb: surface::land_rgb(h / top, wet, temp, fresh), height_km: h, sea_ice: 0.0, glow: [0.0; 3] };
        }
    }
    surface::settle_sea(&mut face, N, c.face_km / (N - 1) as f32, false);
    texels[..N * N].copy_from_slice(&face);
    if !c.planet {
        continue_sea(&face, &mut texels);
    }
    Surface { key, n: N, texels, radius_km: c.face_km * 2.0 / std::f32::consts::PI }
}

/// Carries the spawn face's edge water onto the neighbouring faces and lets
/// it deepen to open ocean, so the face outline never shows as a straight shelf.
fn continue_sea(face: &[Texel], texels: &mut [Texel]) {
    let quarter = std::f32::consts::FRAC_PI_4;
    for (fi, f) in Face::ALL.into_iter().enumerate().skip(1) {
        for k in 0..N * N {
            let d = Projection::Tangent.to_sphere(f, surface::grid(k % N, N), surface::grid(k / N, N));
            if d.x <= 1.0e-3 {
                continue;
            }
            // Spawn-face coordinates extended past the edge: PosX has u = -y, v = +z.
            let s = ((-d.y / d.x).atan() as f32) / quarter;
            let t = ((d.z / d.x).atan() as f32) / quarter;
            let beyond = (s.abs().max(t.abs()) - 1.0).max(0.0);
            let at = |x: f32| (((x.clamp(-1.0, 1.0) + 1.0) * 0.5 * (N - 1) as f32).round() as usize).min(N - 1);
            let edge = face[at(t) * N + at(s)];
            let near = (-edge.height_km).max(0.03);
            let open = (beyond / 0.4).clamp(0.0, 1.0);
            let depth = near + (-DEEP.height_km - near) * open * open * (3.0 - 2.0 * open);
            texels[fi * N * N + k] = Texel { rgb: surface::water_rgb(depth), height_km: -depth, sea_ice: 0.0, glow: [0.0; 3] };
        }
    }
}

fn others(planet: bool) -> Texel {
    if planet { UNSURVEYED } else { DEEP }
}

/// Globe for a solve snapshot; on a planet world the other faces are unsurveyed.
pub fn from_preview(p: &Preview, face_km: f32, planet: bool, key: u64) -> Surface {
    let cells = Cells {
        n: p.n,
        land_km: Box::new(|i| {
            let h = p.height[i];
            (h >= 0.0).then(|| 0.004 + h * (0.3 + 0.7 * h) * PREVIEW_RELIEF_KM)
        }),
        wet: p.wet.as_deref(),
        temp_c: None,
        fresh: Box::new(|i| p.discharge_km2.as_ref().map_or(0.0, |d| ((d[i] - 2.5) / 20.0).clamp(0.0, 1.0))),
        face_km,
        planet,
    };
    build(&cells, others(planet), key)
}

/// Globe for the finished solve: real metres, climate and lakes.
pub fn from_planet(p: &Planet, planet: bool, key: u64) -> Surface {
    let sea = engine_voxel::WATER_LEVEL_METERS;
    let cells = Cells {
        n: p.grid.n,
        land_km: Box::new(|i| p.land[i].then(|| ((p.height[i] - sea) / 1000.0).max(0.004))),
        wet: Some(&p.wet),
        temp_c: Some(&p.maat),
        fresh: Box::new(|i| {
            if p.lake[i] > f32::MIN {
                1.0
            } else {
                ((p.discharge[i] / 1.0e6 - 2.5) / 20.0).clamp(0.0, 1.0)
            }
        }),
        face_km: (p.grid.n as f64 * p.grid.cell_m / 1000.0) as f32,
        planet,
    };
    build(&cells, others(planet), key)
}
