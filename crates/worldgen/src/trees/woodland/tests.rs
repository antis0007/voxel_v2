use super::*;
use crate::WorldType;
use engine_core::VoxelScalePreset;
use std::collections::{HashSet, VecDeque};

#[test]
fn painterly_wood_is_grounded_and_crowns_fit_proxy_bounds() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter] {
        let cfg = WorldSampleConfig { seed: 2738958700, generator_version: 45,
            world_type: WorldType::Hills, voxel_scale: scale };
        let v = cfg.voxel_meters();
        for cell in [-7, 0, 11] {
            let mut t = TreeProfile { conifer: false, cell_x: cell, cell_z: -3,
                anchor_x: -32, anchor_z: 31, ground_y: 16,
                trunk_h: cfg.blocks_for_span(10.0), canopy_radius: cfg.blocks_for_span(5.0),
                canopy_height: cfg.blocks_for_span(4.0), branch_count: 5, branch_len: 4,
                branch_start_y: 20, lean_x: 0, lean_z: 0, form: TreeForm::Classic };
            let age = hash2(t.cell_x, t.cell_z, cfg.seed.wrapping_add(12011));
            profile(&cfg, &mut t, age);
            let radius = crate::trees::crown_reach(&cfg, &t);
            let top = crate::trees::crown_top(&cfg, &t);
            let base = IVec3::new(t.anchor_x, t.ground_y, t.anchor_z);
            let reach = (radius / v).ceil() as i32 + 1;
            let mut wood = HashSet::new();
            let mut leaves = 0;
            for y in 1..=(top / v).ceil() as i32 + 1 {
                for z in -reach..=reach { for x in -reach..=reach {
                    let q = IVec3::new(x, y, z);
                    match block(&cfg, &t, base + q) {
                        B::Log => { wood.insert(q); },
                        B::Leaves => {
                            leaves += 1;
                            assert!((x as f32).hypot(z as f32) * v <= radius);
                            assert!(y as f32 * v <= top);
                        },
                        _ => {}
                    }
                }}
            }
            assert!(leaves > 0, "missing crown: {scale:?} cell={cell}");
            let mut reached: HashSet<_> = wood.iter().filter(|p| p.y == 1).copied().collect();
            let mut queue: VecDeque<_> = reached.iter().copied().collect();
            while let Some(p) = queue.pop_front() {
                for dir in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] {
                    let next = p + dir;
                    if wood.contains(&next) && reached.insert(next) { queue.push_back(next); }
                }
            }
            assert_eq!(reached.len(), wood.len(), "floating wood: {scale:?} cell={cell}");
        }
    }
}
