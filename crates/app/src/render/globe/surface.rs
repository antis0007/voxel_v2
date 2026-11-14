//! Shading fields computed once per completed tile, never per rendered vertex.
use super::{bake::TileData, chart, tile::TILE_SAMPLES};
use engine_core::cube_sphere::Face;
use glam::{DVec2, DVec3};

fn byte(v: f64) -> u8 { (v.clamp(0.0, 1.0) * 255.0).round() as u8 }

fn octahedral(n: DVec3) -> [u8; 2] {
    let p = n / (n.x.abs() + n.y.abs() + n.z.abs());
    let mut q = DVec2::new(p.x, p.y);
    if p.z < 0.0 {
        q = DVec2::new((1.0 - p.y.abs()) * if p.x < 0.0 { -1.0 } else { 1.0 },
            (1.0 - p.x.abs()) * if p.y < 0.0 { -1.0 } else { 1.0 });
    }
    [byte(q.x * 0.5 + 0.5), byte(q.y * 0.5 + 0.5)]
}

pub(super) fn finish(data: &mut TileData, sea: f32) {
    let side = TILE_SAMPLES as usize;
    let last = side - 1;
    let cell = data.id.cell_m();
    let axes = Face::ALL[data.id.face as usize].basis();
    let (u, up, v) = (axes.u.as_dvec3(), axes.n.as_dvec3(), axes.v.as_dvec3());
    let radians_per_m = std::f64::consts::FRAC_PI_4 / chart::HALF_M;
    let height = |i: usize, j: usize| {
        let k = j * side + i;
        let bed = f64::from(data.height_m[k]);
        match data.water[k] { 0 => bed, 1 => bed.max(f64::from(sea)), depth => bed + f64::from(depth - 1) }
    };
    for j in 0..side {
        for i in 0..side {
            let (il, ir, jl, jr) = (i.saturating_sub(1), (i + 1).min(last), j.saturating_sub(1), (j + 1).min(last));
            let (h, hl, hr, hb, ht) = (height(i, j), height(il, j), height(ir, j), height(i, jl), height(i, jr));
            let (dx, dz) = ((hr - hl) / ((ir - il) as f64 * cell), (ht - hb) / ((jr - jl) as f64 * cell));
            let point = data.id.sample_chart(i as u32, j as u32) * radians_per_m;
            let (s, t) = (point.x.tan(), point.y.tan());
            let cube = up + u * s + v * t;
            let inv = cube.length_recip();
            let radial = cube * inv;
            let scale = (chart::RADIUS_M + h) * radians_per_m * inv;
            let along_x = (u - radial * radial.dot(u)) * (scale * (1.0 + s * s)) + radial * dx;
            let along_z = (v - radial * radial.dot(v)) * (scale * (1.0 + t * t)) + radial * dz;
            let k = j * side + i;
            data.normals[k] = octahedral(along_z.cross(along_x).normalize_or(radial));
            // Four local horizon angles approximate lost sky in concavities.
            // Slopes remain sun-lit; this only reduces diffuse sky by <= 18%.
            let horizon = [hl, hr, hb, ht].into_iter()
                .map(|neighbour| ((neighbour - h).max(0.0) / cell).atan()).sum::<f64>() * 0.25;
            data.climate[k][3] = byte(1.0 - (horizon / std::f64::consts::FRAC_PI_2).min(1.0) * 0.18);
        }
    }
}
