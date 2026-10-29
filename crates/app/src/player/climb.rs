//! Ladder and vine climbing: replaces gravity while the body touches a climbable cell.
use crate::world::{World, player_aabb, speed_blocks};
use engine_core::VoxelScalePreset;
use glam::{IVec3, Vec3};

/// Climb speed, metres per second.
const UP_MPS: f32 = 2.4;
/// Slowest descent while idle on a ladder, metres per second.
const SLIDE_MPS: f32 = 1.5;
/// Reach beyond the body box that still counts as pressing into a ladder, metres.
const GRIP_METRES: f32 = 0.08;

/// Climb controls for one step.
#[derive(Clone, Copy, Debug, Default)]
pub struct Grip {
    pub up: bool,
    pub hold: bool,
}

/// True when the body box, widened by the grip reach, overlaps a ladder or vine.
/// `pos` is local to `origin`, as in `update_player_local`.
pub fn touching(world: &World, pos: Vec3, origin: IVec3, body_scale: f32, crouch_t: f32) -> bool {
    let (lo, hi) = player_aabb(pos, world.voxel_scale, body_scale, crouch_t);
    let reach = Vec3::new(GRIP_METRES, 0.0, GRIP_METRES) / world.voxel_scale.meters();
    let (lo, hi) = ((lo - reach).floor().as_ivec3(), (hi + reach).ceil().as_ivec3());
    for y in lo.y..hi.y {
        for z in lo.z..hi.z {
            for x in lo.x..hi.x {
                if engine_content::thin::is_climbable(world.get(origin + IVec3::new(x, y, z))) {
                    return true;
                }
            }
        }
    }
    false
}

/// Vertical speed in voxels per second after one climbing step: climb, hold or slow slide.
pub fn vertical(vel_y: f32, grip: Grip, scale: VoxelScalePreset) -> f32 {
    if grip.up {
        speed_blocks(scale, UP_MPS)
    } else if grip.hold {
        0.0
    } else {
        vel_y.max(-speed_blocks(scale, SLIDE_MPS))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{BlockId, ChunkData, WorldType};
    use std::collections::HashSet;
    use std::sync::Arc;

    #[test]
    fn climb_rule_rises_holds_and_slides_slowly() {
        for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::QuarterMeter] {
            let m = scale.meters();
            let up = vertical(-20.0, Grip { up: true, hold: false }, scale);
            assert!((up * m - UP_MPS).abs() < 1e-4);
            assert_eq!(vertical(-20.0, Grip { up: false, hold: true }, scale), 0.0);
            let slide = vertical(-20.0 / m, Grip::default(), scale);
            assert!((slide * m + SLIDE_MPS).abs() < 1e-4, "fall capped to a slide");
            // A slower fall or a rise is kept.
            assert_eq!(vertical(-0.1, Grip::default(), scale), -0.1);
            assert_eq!(vertical(3.0, Grip::default(), scale), 3.0);
        }
    }

    #[test]
    fn body_touches_a_ladder_beside_it_but_not_one_away() {
        let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let mut blocks = Box::new([BlockId::Air; 32 * 32 * 32]);
        blocks[ChunkData::local_index(10, 5, 11)] = BlockId::Ladder;
        w.chunks.insert(IVec3::ZERO, Arc::new(ChunkData {
            modified_bounds: None,
            structural_bounds: None,
            blocks,
            light: Box::new([0; 32 * 32 * 32]),
            torches: HashSet::new(),
        }));
        let inside = Vec3::new(10.5, 5.0, 11.5);
        assert!(touching(&w, inside, IVec3::ZERO, 1.0, 0.0));
        // Body box edge 0.04 m short of the ladder cell: the grip reach covers it.
        let half = crate::world::player_width_blocks(w.voxel_scale, 1.0) * 0.5;
        let near = Vec3::new(10.5, 5.0, 11.0 - half - 0.04);
        assert!(touching(&w, near, IVec3::ZERO, 1.0, 0.0));
        let far = Vec3::new(10.5, 5.0, 9.0);
        assert!(!touching(&w, far, IVec3::ZERO, 1.0, 0.0));
    }
}
