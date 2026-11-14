//! Coarse, whole-sphere snapshots of the same fields the generator solves.
use engine_core::cube_sphere::{Face, Projection, atlas};
use glam::DVec3;

use super::{SEA, World};
use crate::WorldSampleConfig;

pub struct SolveAtlas {
    pub revision: u8,
    pub n: usize,
    pub texels: Vec<SolveTexel>,
}

#[derive(Clone, Copy)]
pub struct SolveTexel {
    pub height_m: f32,
    pub wet: f32,
    pub annual_c: f32,
    pub winter_c: f32,
    pub slope: f32,
    pub inland_depth_m: f32,
}

impl SolveAtlas {
    const SIDE: usize = 65;

    pub(super) fn sample(revision: u8, field: impl Fn(DVec3) -> SolveTexel) -> Self {
        let n = Self::SIDE;
        let mut texels = Vec::with_capacity(6 * n * n);
        let axis = |i| 2.0 * i as f64 / (n - 1) as f64 - 1.0;
        for face in Face::ALL {
            for z in 0..n {
                for x in 0..n {
                    texels.push(field(Projection::Tangent.to_sphere(face, axis(x), axis(z))));
                }
            }
        }
        Self { revision, n, texels }
    }

    pub(super) fn finished(world: &World, cfg: &WorldSampleConfig) -> Self {
        let footprint = (2.0 * atlas::HALF_M / (Self::SIDE - 1) as f64) as f32;
        Self::sample(2, |dir| {
            let g = world.ground(cfg, dir, footprint);
            let head = g.water_m.unwrap_or(g.top_m);
            SolveTexel {
                height_m: g.top_m - SEA as f32,
                wet: g.moisture,
                annual_c: g.maat_c,
                winter_c: world.climate(dir).winter_c as f32,
                slope: g.slope,
                inland_depth_m: if head > SEA as f32 { (head - g.top_m).max(0.0) } else { 0.0 },
            }
        })
    }
}
