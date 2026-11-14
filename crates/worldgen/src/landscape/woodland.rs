//! Regional canopy admission uses a fixed terrain survey at every voxel/LOD scale.
use crate::{WorldSampleConfig, geo, smoothstep};

pub const VERSION: u32 = 141;

pub fn active(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= VERSION && crate::basin::solved_face(cfg)
}

/// Concave ground gathers woodland; convex divides open it. Sample the solved
/// surface, never generated columns, so this cannot recurse into tree admission.
/// The 96 m survey stays constant when render footprints or voxel scales change.
pub fn shelter(cfg: &WorldSampleConfig, x: f32, z: f32) -> f32 {
    let p = geo::planet_for(cfg);
    let height = |dx: f64, dz: f64| {
        let (wx, wz) = (f64::from(x) + dx, f64::from(z) + dz);
        crate::vistas::height(cfg, wx as f32, wz as f32, p.grid.bilinear(&p.height, wx, wz)) as f32
    };
    let centre = height(0.0, 0.0);
    let mean = (height(96.0, 0.0) + height(-96.0, 0.0)
        + height(0.0, 96.0) + height(0.0, -96.0)) * 0.25;
    let relative = centre - mean;
    (1.0 - smoothstep(-9.0, -1.0, relative)) * 0.20
        - smoothstep(1.0, 9.0, relative) * 0.24
}
