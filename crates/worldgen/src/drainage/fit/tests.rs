use super::super::{RIVER_FIT_GENERATOR, sample};
use super::*;
use crate::{BlockId, WorldType, sample_block, sample_column};
use glam::IVec3;

fn cfg(voxel_scale: VoxelScalePreset) -> WorldSampleConfig {
    WorldSampleConfig { generator_version: RIVER_FIT_GENERATOR, seed: 2738958700, world_type: WorldType::Hills, voxel_scale }
}

/// Ground top and the water surface a column actually holds.
fn column(cfg: &WorldSampleConfig, x: i32, z: i32) -> (f32, Option<f32>) {
    let top = sample_column(cfg, x, z).height as f32 + 1.0;
    (top, sample(cfg, x as f32, z as f32).water_m.filter(|&s| s > top))
}

/// Generator 51 left 244 dry columns more than 1 m below a neighbouring water
/// surface in this area (worst 11.9 m) and 90% of wet columns one voxel deep.
#[test]
fn river_fit_water_never_stands_above_dry_ground() {
    let cfg = cfg(VoxelScalePreset::OneMeter);
    let (mut wet, mut deep, mut walls, mut worst) = (0, 0, 0, 0.0f32);
    for z in (380..700).step_by(2) {
        for x in (150..460).step_by(2) {
            let (top, Some(s)) = column(&cfg, x, z) else { continue };
            wet += 1;
            deep += usize::from(s.ceil() - top >= 2.0);
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (n_top, n_water) = column(&cfg, x + dx, z + dz);
                if n_water.is_none() && s - n_top > 1.0 {
                    walls += 1;
                    worst = worst.max(s - n_top);
                }
            }
        }
    }
    println!("river fit wet={wet} deep={deep} walls={walls} worst={worst:.1}");
    assert!(wet > 500, "only {wet} wet columns");
    assert!(deep * 2 > wet, "{deep} of {wet} wet columns are two voxels deep");
    assert!(worst <= 3.0 && walls * 100 <= wet, "{walls} walls, worst {worst:.1} m");
}

#[test]
fn river_fit_places_water_voxels_where_the_plan_says() {
    let cfg = cfg(VoxelScalePreset::OneMeter);
    let mut checked = 0;
    for z in (380..700).step_by(7) {
        for x in (150..460).step_by(7) {
            let (top, Some(s)) = column(&cfg, x, z) else { continue };
            let y = top as i32;
            assert_eq!(sample_block(&cfg, IVec3::new(x, y, z)), BlockId::Water, "{x},{y},{z} surface {s:.2}");
            assert_ne!(sample_block(&cfg, IVec3::new(x, y - 1, z)), BlockId::Water, "{x},{z} water under the bed");
            checked += 1;
        }
    }
    assert!(checked > 20, "only {checked} wet samples");
}

#[test]
fn river_fit_plan_ignores_voxel_size_and_negative_coordinates() {
    let (coarse, fine) = (cfg(VoxelScalePreset::OneMeter), cfg(VoxelScalePreset::QuarterMeter));
    let mut wet = 0;
    for z in (-900..900).step_by(37) {
        for x in (-900..900).step_by(37) {
            let (a, b) = (sample(&coarse, x as f32, z as f32), sample(&fine, x as f32, z as f32));
            assert_eq!((a.bed_m, a.water_m, a.levee_m), (b.bed_m, b.water_m, b.levee_m), "{x},{z}");
            wet += usize::from(a.water_m.is_some());
        }
    }
    assert!(wet > 0);
}

/// Every reach starts at its source's level and ends at its target's, so the
/// surfaces of the reaches meeting at a node agree there.
#[test]
fn river_fit_reaches_join_at_node_levels() {
    let cfg = cfg(VoxelScalePreset::OneMeter);
    let (mut joins, mut steps) = (0, 0);
    for z in -6..=6 {
        for x in -6..=6 {
            let Outlet::Downhill(c, d) = outlet(&cfg, x, z) else { continue };
            assert_eq!(reach_surface(&cfg, (x, z), (c, d), 0.0), level(&cfg, x, z), "{x},{z} start");
            let end = reach_surface(&cfg, (x, z), (c, d), 1.0);
            joins += 1;
            if (end - level(&cfg, c, d)).abs() > 0.01 {
                steps += 1;
                assert!(end < level(&cfg, c, d), "{x},{z} ends above its target");
            }
        }
    }
    println!("river fit joins={joins} steps={steps}");
    assert!(steps * 10 <= joins, "{steps} of {joins} reaches end off their target level");
}

#[test]
fn catchment_grows_downstream_and_is_zero_before_fit() {
    let c = cfg(VoxelScalePreset::OneMeter);
    let mut trunk = 0;
    let mut headwater = u32::MAX;
    for z in -8..=8 {
        for x in -8..=8 {
            let n = crate::drainage::catchment_at(&c, x as f32 * 192.0, z as f32 * 192.0);
            assert!(n >= 1, "node {x},{z} drains nothing");
            if let Outlet::Downhill(a, b) = outlet(&c, x, z) {
                let down = flow(&c, a, b);
                assert!(down >= n, "flow falls downstream at {x},{z}: {n} -> {down}");
            }
            trunk = trunk.max(n);
            headwater = headwater.min(n);
        }
    }
    assert_eq!(headwater, 1);
    assert!(trunk >= 8, "largest catchment only {trunk} nodes");
    let old = WorldSampleConfig { generator_version: RIVER_FIT_GENERATOR - 1, ..c };
    assert_eq!(crate::drainage::catchment_at(&old, 0.0, 0.0), 0);
}
