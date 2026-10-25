//! Fit a complete room graph to terrain once, before caching or spawning it.
use super::{Complex, WALL};
use crate::{BlockId as B, WorldSampleConfig, architecture::Building, sample_column};
use glam::Vec3;

pub(super) fn fit(cfg: &WorldSampleConfig, mut c: Complex) -> Option<Complex> {
    let (sx, sz, top) = c.shaft;
    let height = |x: f32, z: f32| {
        cfg.world_y(sample_column(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z)).height)
            + cfg.voxel_meters()
    };
    // Gatehouse, its entrance and the grading apron must all fit together.
    for iz in -8..=8 {
        for ix in -8..=8 {
            let (x, z) = (sx + ix as f32 * 3.0, sz + iz as f32 * 3.0);
            let h = height(x, z);
            if (h - top).abs() > 5.0 || h < 16.0 {
                return None;
            }
        }
    }
    for (dx, dz) in [
        (0.0, 0.0),
        (-24.0, -24.0),
        (24.0, -24.0),
        (-24.0, 24.0),
        (24.0, 24.0),
    ] {
        if crate::sites::building_reserved(cfg, sx + dx, sz + dz) {
            return None;
        }
    }
    let mut lower = 0.0f32;
    let mut survey = |x0: f32, z0: f32, x1: f32, z1: f32, roof: f32| {
        let nx = ((x1 - x0) / 3.0).ceil().max(1.0) as usize;
        let nz = ((z1 - z0) / 3.0).ceil().max(1.0) as usize;
        for iz in 0..=nz {
            for ix in 0..=nx {
                let x = x0 + (x1 - x0) * ix as f32 / nx as f32;
                let z = z0 + (z1 - z0) * iz as f32 / nz as f32;
                // The authored valley owns these columns even below ground.
                // Reject the footprint instead of allowing its sampling gate
                // to slice a room or corridor in half.
                if crate::valley::reserves(cfg, x, z) {
                    lower = f32::INFINITY;
                }
                // Six metres of cover also protects against small hollows
                // between survey points. Preserve every room's full vault.
                lower = lower.max(roof + 6.0 - height(x, z));
            }
        }
    };
    for r in &c.rooms {
        survey(
            r.x0 - WALL,
            r.z0 - WALL,
            r.x1 + WALL,
            r.z1 + WALL,
            r.floor + r.top() + WALL,
        );
    }
    for p in &c.corrs {
        survey(
            p.x0 - 2.0,
            p.z0 - 2.0,
            p.x1 + 2.0,
            p.z1 + 2.0,
            p.fa.max(p.fb) + p.h + 3.0,
        );
    }
    let lower = (lower / cfg.voxel_meters()).ceil() * cfg.voxel_meters();
    if lower > 48.0 || c.min.y - lower < cfg.world_y(crate::bedrock::ceiling(cfg)) + 3.0 {
        return None;
    }
    // A single vertical translation keeps all doors, ramps, encounters and
    // puzzle connections aligned. The shaft reads the translated entry floor.
    for r in &mut c.rooms {
        r.floor -= lower;
    }
    for p in &mut c.corrs {
        p.fa -= lower;
        p.fb -= lower;
    }
    for a in &mut c.anchors {
        a.y -= lower;
    }
    c.min.y -= lower;
    c.gatehouse = Building::new(
        c.gatehouse.wings,
        c.gatehouse.curtains,
        Some((-7.0, 7.0, -7.0, 7.0, B::Cobblestone, 10.0)),
    );
    c.min = c.min.min(Vec3::new(sx - 24.0, top - 40.0, sz - 24.0));
    c.max = c.max.max(Vec3::new(sx + 24.0, top + 18.0, sz + 24.0));
    // The index is horizontal; lowering the graph does not invalidate it.
    Some(c)
}
