//! Blade clump placement: how many clumps a turf column grows and which forms.
use super::{Blades, Column, Detail, GeneratedChunk, IVec3, Kind, Vec3, rehash};

const MAX_CLUMPS_PER_COLUMN: u32 = 4;

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Tall-grass pockets on the 23 m sward mosaic, about a third of open turf.
fn pocket(c: &Column) -> f32 {
    smoothstep(0.48, 0.72, c.sward)
}

/// Blade clumps per square metre of turf. Contrast carries the read: a thin
/// open field, dense tall pockets, bare soil pockets, thinned shade, a
/// near-bare trunk ring and a lush light gap just outside each crown.
pub(super) fn clump_density(c: &Column, wet: f32, disturbance: f32, steps: f32) -> f32 {
    let open = 0.10 + 0.35 * c.meadow * c.local;
    let tall = pocket(c) * (1.6 + 1.6 * c.local);
    let lit = (open + tall) * (1.0 - c.bare) * (1.0 - 0.78 * c.shade) * (1.0 - 0.92 * c.root);
    (lit + c.edge * 1.4 + wet * 0.6) * (1.0 - disturbance) * (1.0 - 0.35 * steps.min(1.0))
}

fn clump_kind(c: &Column, wet: f32, h: u32) -> Blades {
    let roll = (h >> 20 & 255) as f32 / 255.0;
    let pick = |table: &[(f32, Blades)]| {
        let mut acc = 0.0;
        for &(w, b) in table {
            acc += w;
            if roll < acc {
                return b;
            }
        }
        table[table.len() - 1].1
    };
    use Blades::*;
    if c.shade > 0.45 {
        pick(&[(0.45, Weed), (0.40, Short), (0.15, Dense)])
    } else if c.edge > 0.4 {
        pick(&[(0.35, SeedHead), (0.45, Medium), (0.20, Bent)])
    } else if wet > 0.3 {
        pick(&[(0.6, Dense), (0.4, Medium)])
    } else if pocket(c) > 0.5 {
        let seed = 0.10 + 0.20 * c.dry;
        pick(&[(0.38, Medium), (0.32, Dense), (seed, SeedHead), (1.0, Bent)])
    } else {
        pick(&[(0.50, Short), (0.25, Bent), (0.15, Weed), (0.10, Dense)])
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn plan(
    chunk: &GeneratedChunk,
    out: &mut Vec<Detail>,
    p: IVec3,
    h: u32,
    column: &Column,
    wet: f32,
    disturbance: f32,
    steps: f32,
) {
    let scale = chunk.voxel_meters;
    let expected = clump_density(column, wet, disturbance, steps) * scale * scale;
    let litter = (column.shade * 0.9 + column.root * 1.2 + column.rainforest * 1.1) * scale * scale;
    let fraction = (h >> 4 & 4095) as f32 / 4095.0;
    let count = ((expected + fraction).floor() as u32).min(MAX_CLUMPS_PER_COLUMN);
    let litter_count = u32::from((rehash(h, 77) & 4095) as f32 / 4095.0 < litter.min(0.9));
    for k in 0..count + litter_count {
        let hk = rehash(h, k + 1);
        // Spread clumps over the cell; a single clump keeps clear of the
        // authored plant's usual spot near the cell centre.
        let at = Vec3::new(
            0.08 + (hk & 255) as f32 / 255.0 * 0.84,
            1.0,
            0.08 + (hk >> 8 & 255) as f32 / 255.0 * 0.84,
        );
        let top = p.as_vec3() + at;
        let litter = k >= count;
        if !(litter && column.rainforest > 0.3) && chunk.sky_visibility(top, Vec3::Y) < 0.18 {
            continue;
        }
        let kind = if litter { Blades::Litter } else { clump_kind(column, wet, hk) };
        out.push(Detail {
            p: top,
            hash: hk,
            kind: Kind::Clump(kind),
            color: [0.29, 0.27, 0.22],
            pigment: 0.95 + (hk >> 16 & 255) as f32 / 255.0 * 0.10,
            growth: 1.0,
            dry: column.dry,
        });
    }
}

#[cfg(test)]
pub(super) fn density_at(c: &Column) -> f32 {
    clump_density(c, 0.0, 0.0, 0.0)
}
