//! Where lightning lands. A strike aimed at a mark resolves at the nearest
//! conductive cell within reach, so a rod, a cable or a metal block placed by
//! the player decides the outcome. One bounded scan per strike; no field solve.
use super::*;
use crate::world::BlockId;

/// S/m. Every metal block and the power cable qualify; stone, soil and
/// crystal do not, so a stormstone charges a creature without grounding it.
pub(super) const MIN_CONDUCTIVITY: f32 = 1.0e6;
/// Cells scanned per axis half-width, whatever the voxel scale; 17³ reads.
const MAX_REACH_CELLS: i32 = 8;

pub(super) fn conductive(block: BlockId) -> bool {
    block.material().sim.electric_conductivity >= MIN_CONDUCTIVITY
}

/// Centre of the conductive cell nearest `mark` within `reach_m`, in metres.
pub(super) fn nearest(world: &World, mark: Vec3, reach_m: f32) -> Option<Vec3> {
    let scale = world.voxel_scale.meters();
    let origin = (mark / scale).floor().as_ivec3();
    let reach = ((reach_m / scale).ceil() as i32).clamp(1, MAX_REACH_CELLS);
    let reach_sq = reach_m * reach_m;
    let mut best: Option<(f32, Vec3)> = None;
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            for dz in -reach..=reach {
                let cell = origin + IVec3::new(dx, dy, dz);
                let centre = (cell.as_vec3() + Vec3::splat(0.5)) * scale;
                let d_sq = centre.distance_squared(mark);
                if d_sq > reach_sq || best.is_some_and(|(b, _)| d_sq >= b) {
                    continue;
                }
                if conductive(world.get(cell)) {
                    best = Some((d_sq, centre));
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

/// Where a strike aimed at `mark` lands: a conductor in reach, else the mark.
pub(super) fn landing(world: &World, mark: Vec3, reach_m: f32) -> Vec3 {
    nearest(world, mark, reach_m).unwrap_or(mark)
}
