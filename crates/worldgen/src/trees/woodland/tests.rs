use super::*;
use crate::WorldType;
use engine_core::VoxelScalePreset;
use std::collections::{HashSet, VecDeque};

#[test]
fn painterly_wood_is_grounded_and_crowns_fit_proxy_bounds() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter] {
        let cfg = WorldSampleConfig {
    planet: None, seed: 2738958700, generator_version: 45,
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

/// Wood and leaf voxels of one profiled tree, checked against its LOD bounds.
fn grow(cfg: &WorldSampleConfig, cell: i32) -> (HashSet<IVec3>, HashSet<IVec3>) {
    let v = cfg.voxel_meters();
    let mut t = TreeProfile { conifer: false, cell_x: cell, cell_z: -3,
        anchor_x: -32, anchor_z: 31, ground_y: 16,
        trunk_h: cfg.blocks_for_span(10.0), canopy_radius: cfg.blocks_for_span(5.0),
        canopy_height: cfg.blocks_for_span(4.0), branch_count: 5, branch_len: 4,
        branch_start_y: 20, lean_x: 0, lean_z: 0, form: TreeForm::Classic };
    let age = hash2(t.cell_x, t.cell_z, cfg.seed.wrapping_add(12011));
    profile(cfg, &mut t, age);
    let radius = crate::trees::crown_reach(cfg, &t);
    let top = crate::trees::crown_top(cfg, &t);
    let base = IVec3::new(t.anchor_x, t.ground_y, t.anchor_z);
    let reach = (radius / v).ceil() as i32 + 1;
    let (mut wood, mut leaves) = (HashSet::new(), HashSet::new());
    for y in 1..=(top / v).ceil() as i32 + 1 {
        for z in -reach..=reach { for x in -reach..=reach {
            let q = IVec3::new(x, y, z);
            match block(cfg, &t, base + q) {
                B::Log => { wood.insert(q); },
                B::Leaves => {
                    assert!((x as f32).hypot(z as f32) * v <= radius, "leaf outside reach");
                    assert!(y as f32 * v <= top, "leaf above top");
                    leaves.insert(q);
                },
                _ => {}
            }
        }}
    }
    (wood, leaves)
}

const SIDES: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];

/// Exposed leaf faces over leaf count^(2/3): 4.8 for a voxel sphere, higher when rough.
fn roughness(wood: &HashSet<IVec3>, leaves: &HashSet<IVec3>) -> f32 {
    let open = leaves.iter()
        .flat_map(|p| SIDES.iter().map(move |d| *p + *d))
        .filter(|n| !leaves.contains(n) && !wood.contains(n))
        .count();
    open as f32 / (leaves.len() as f32).powf(2.0 / 3.0)
}

/// Share of leaf voxels with no 6-connected path through leaves or wood to the ground row.
fn floating(wood: &HashSet<IVec3>, leaves: &HashSet<IVec3>) -> f32 {
    let mut reached: HashSet<IVec3> = wood.iter().chain(leaves).filter(|p| p.y == 1).copied().collect();
    let mut queue: VecDeque<_> = reached.iter().copied().collect();
    while let Some(p) = queue.pop_front() {
        for d in SIDES {
            let n = p + d;
            if (wood.contains(&n) || leaves.contains(&n)) && reached.insert(n) { queue.push_back(n); }
        }
    }
    leaves.iter().filter(|p| !reached.contains(p)).count() as f32 / leaves.len().max(1) as f32
}

/// Interleaved timing of whole-crown sampling, plain against clumped.
#[test]
#[ignore]
fn clumped_crown_sample_cost() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::QuarterMeter] {
        for generator_version in [129, crate::trees::foliage::VERSION, 129, crate::trees::foliage::VERSION] {
            let cfg = WorldSampleConfig {
    planet: None, seed: 2738958700, generator_version,
                world_type: WorldType::Hills, voxel_scale: scale };
            let started = std::time::Instant::now();
            let leaves: usize = (-12..12).map(|cell| grow(&cfg, cell).1.len()).sum();
            println!("{scale:?} v{generator_version}: {leaves} leaves, {:.0} ms", started.elapsed().as_secs_f64() * 1000.0);
        }
    }
}

#[test]
fn clumped_crowns_are_bounded_grounded_and_rougher_only_where_resolvable() {
    for cell in [-7, 0, 11] {
        let mut gain = Vec::new();
        for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
            let [plain, clumped] = [129, crate::trees::foliage::VERSION].map(|generator_version| {
                let cfg = WorldSampleConfig {
    planet: None, seed: 2738958700, generator_version,
                    world_type: WorldType::Hills, voxel_scale: scale };
                grow(&cfg, cell)
            });
            let (wood, leaves) = &clumped;
            assert!(!leaves.is_empty(), "missing crown: {scale:?} cell={cell}");
            let mut reached: HashSet<_> = wood.iter().filter(|p| p.y == 1).copied().collect();
            let mut queue: VecDeque<_> = reached.iter().copied().collect();
            while let Some(p) = queue.pop_front() {
                for dir in SIDES {
                    let next = p + dir;
                    if wood.contains(&next) && reached.insert(next) { queue.push_back(next); }
                }
            }
            assert_eq!(reached.len(), wood.len(), "floating wood: {scale:?} cell={cell}");
            // Clumps keep the leaf mass recognisable at every scale.
            let mass = leaves.len() as f32 / plain.1.len() as f32;            // Saplings with a few dozen leaf voxels swing too far to compare.
            if plain.1.len() >= 200 {
                assert!((0.6..1.5).contains(&mass), "leaf mass x{mass} at {scale:?} cell={cell}");
            }
            let (before, after) = (floating(&plain.0, &plain.1), floating(wood, leaves));
            assert!(after <= before + 0.01, "clumps detached leaves: share {before} -> {after} at {scale:?} cell={cell}");
            gain.push(roughness(wood, leaves) / roughness(&plain.0, &plain.1));
        }
        // Clumps add silhouette at every scale; relief bands add most on the finest grid.
        assert!(gain[2] > 1.15, "quarter-metre crown not rougher: {gain:?} cell={cell}");
        assert!(gain[2] > gain[0] + 0.15, "relief not resolution-gated: {gain:?} cell={cell}");
    }
}
