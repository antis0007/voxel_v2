//! Generator 297: faulted rift valleys where continental plates pull apart.
//!
//! The plate province already lowers a divergent margin into a broad trough.
//! This adds the structure of a real continental rift on top of it:
//!
//! - a graben floor dropped between two border faults;
//! - a master fault on one side (steep scarp, uplifted footwall) and a
//!   flexed ramp on the other, so the floor tilts toward the master fault;
//! - the master side flips along strike; where it flips, the floor shallows
//!   into a transfer-zone saddle that divides one basin from the next;
//! - tilted blocks stepping across the floor.
//!
//! The solve mesh cannot hold a 40 m scarp, so there are two versions of the
//! section. `Frame::solve` leaves out the floor blocks and is added to the
//! mesh heights before climate, erosion and drainage, which then see the
//! valley. `Frame::full` is evaluated per column; the caller adds its
//! difference from the interpolated solve term, which restores the sub-grid
//! scarps and is zero at every mesh node.
use super::tectonics::{RIFT_WIDTH, smooth};
use crate::planet::noise_at;
use engine_core::cube_sphere::atlas::RADIUS_M;
use glam::DVec3;

/// Deepest a rift floor is dropped below sea level, metres.
pub(super) const SEA_FLOOR_M: f64 = 14.0;
/// Width of one tilted floor block, radians (about 140 m).
const BLOCK: f64 = 0.0046;
/// Step between neighbouring floor blocks at full extension, metres.
const BLOCK_STEP_M: f64 = 9.0;
/// Scarp width of the master fault and of the flexed side, radians.
const MASTER_SCARP: f64 = 0.0013;
const FLEXED_SCARP: f64 = 0.0065;
/// Distance over which the flank uplift decays away from a border fault.
const FLANK: f64 = 0.014;

/// A point's place in a rift: its signed distance from the plate boundary
/// and the terms that are constant across the valley at this point of strike.
pub(super) struct Frame {
    /// Signed distance from the boundary, radians; the sign is shared by
    /// both plates so the section is continuous across the boundary.
    pub cross: f64,
    /// Projection of the direction on the boundary plane: the along-strike coordinate.
    pub strike: DVec3,
    /// Seed of this plate pair.
    pub key: u32,
    /// How hard the margin is opening through continental crust, 0 to 1.
    pub extension: f64,
    /// The planet's relief scale, metres.
    pub relief: f64,
}

struct Section {
    half: f64,
    centre: f64,
    polarity: f64,
    drop: f64,
    phase: f64,
}

impl Frame {
    fn section(&self) -> Section {
        let n = |f: f64, salt: u32| noise_at(self.strike * f, self.key.wrapping_add(salt));
        // Rift segments a few kilometres long, with weaker ground between them.
        let segment = 0.25 + 0.75 * smooth(0.24, 0.52, n(9.0, 11));
        Section {
            half: 0.011 + 0.009 * n(21.0, 17),
            centre: 0.007 * (n(30.0, 19) - 0.5),
            polarity: 2.0 * smooth(0.40, 0.60, n(12.0, 13)) - 1.0,
            drop: self.extension * segment * (45.0 + 0.16 * self.relief),
            phase: 3.0 * n(40.0, 23),
        }
    }

    fn height(&self, s: &Section, cross: f64, blocks: f64) -> f64 {
        let x = cross - s.centre;
        let side = if x >= 0.0 { 1.0 } else { -1.0 };
        // One on the master fault's side, zero on the flexed side.
        let master = 0.5 + 0.5 * side * s.polarity;
        let scarp = FLEXED_SCARP + (MASTER_SCARP - FLEXED_SCARP) * master;
        let floor = 1.0 - smooth(s.half, s.half + scarp, x.abs());
        let across = (x / s.half).clamp(-1.0, 1.0);
        let depth = s.drop * (0.62 + 0.38 * s.polarity * across);
        let flank = 0.42 * s.drop * (0.45 + 0.55 * master) * (-((x.abs() - s.half).max(0.0) / FLANK)).exp();
        let step = if blocks > 0.0 {
            // Each block rises toward the master fault and drops at its own small scarp.
            let saw = (x / BLOCK + s.phase).rem_euclid(1.0) - 0.5;
            saw * s.polarity * BLOCK_STEP_M * self.extension * blocks
        } else { 0.0 };
        let fade = 1.0 - smooth(0.036, RIFT_WIDTH, cross.abs());
        (flank * (1.0 - floor) + (step - depth) * floor) * fade
    }

    /// The part of the valley the solve mesh carries, metres.
    pub fn solve(&self) -> f64 {
        self.height(&self.section(), self.cross, 0.0)
    }

    /// The whole section and its slope (rise over run) for a column whose
    /// sample covers `footprint` metres.
    pub fn full(&self, footprint: f64) -> (f64, f64) {
        let s = self.section();
        let block_m = BLOCK * RADIUS_M;
        let blocks = 1.0 - smooth(block_m * 0.25, block_m, footprint);
        let reach = footprint.max(1.5);
        let d = reach / RADIUS_M;
        let slope = (self.height(&s, self.cross + d, blocks) - self.height(&s, self.cross - d, blocks)).abs() / (2.0 * reach);
        (self.height(&s, self.cross, blocks), slope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WorldSampleConfig, WorldType};
    use engine_core::VoxelScalePreset;

    /// Review dump: how much of the planet rifts, and one section across the
    /// strongest dry valley. `RV_SEED` picks the seed; run with
    /// `VOXEL_HISTORY_MYR=20` for a quick planet.
    #[test]
    #[ignore = "review dump"]
    fn dump_rift_valleys() {
        let seed = std::env::var("RV_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(74);
        let cfg = WorldSampleConfig { planet: None, generator_version: super::super::RIFT_VALLEY_GENERATOR, seed,
            world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter };
        let world = super::super::get(&cfg);
        let mesh = &world.mesh;
        let (mut nodes, mut land, mut lake) = (0, 0, 0);
        let mut best: Option<(f64, usize)> = None;
        for (i, &dir) in mesh.directions.iter().enumerate() {
            let Some(frame) = world.plates.rift_frame(dir) else { continue };
            nodes += 1;
            if world.height[i] <= super::super::SEA { continue; }
            land += 1;
            lake += usize::from(world.flow.spill[i] > world.height[i] + 0.75);
            let drop = -frame.solve();
            let inland = world.height[i] > super::super::SEA + 60.0;
            if inland && best.is_none_or(|(d, _)| drop > d) { best = Some((drop, i)); }
        }
        println!("seed {seed}: rift nodes {nodes} of {}, on land {land}, lake nodes {lake}", mesh.directions.len());
        let Some((drop, i)) = best else { println!("no dry rift valley"); return };
        let dir = mesh.directions[i];
        let at = engine_core::cube_sphere::atlas::from_body(dir * RADIUS_M);
        println!("deepest floor: solve drop {drop:.1} m at x={:.0} z={:.0}", at.x, at.z);
        // Walk across the valley along the cross-boundary gradient.
        let frame = world.plates.rift_frame(dir).expect("picked from a frame");
        let e = 1e-4;
        let east = dir.cross(DVec3::Y).normalize_or(DVec3::X);
        let north = dir.cross(east).normalize();
        let cross = |d: DVec3| world.plates.rift_frame(d.normalize()).map_or(frame.cross, |f| f.cross);
        let grad = (east * (cross(dir + east * e) - cross(dir - east * e))
            + north * (cross(dir + north * e) - cross(dir - north * e))).normalize_or(east);
        let (full, _) = frame.full(1.0);
        println!("node height {:.1} sea {:.1} applied {:.1} kept {:.2} cross {:.4} full {full:.1} extension {:.2}",
            world.height[i], super::super::SEA, world.rift_valley[i], world.rift_scale[i], frame.cross, frame.extension);
        println!("  metres  ground  water  slope");
        for k in -30..=30 {
            let offset = f64::from(k) * 60.0;
            let d = (dir + grad * (offset / RADIUS_M)).normalize();
            let g = world.ground(&cfg, d, 1.0);
            println!("  {offset:6.0}  {:6.1}  {}  {:.2}", g.top_m,
                g.water_m.map_or("     -".into(), |w| format!("{w:6.1}")), g.slope);
        }
    }
}
