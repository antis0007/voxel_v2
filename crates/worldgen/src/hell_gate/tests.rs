use super::*;
use engine_core::VoxelScalePreset;

fn cfg(version: u32) -> WorldSampleConfig {
    WorldSampleConfig { planet: None, generator_version: version, seed: crate::test_world::seed(), world_type: WorldType::Hills, voxel_scale: VoxelScalePreset::HalfMeter }
}

fn cell(cfg: &WorldSampleConfig, m: Vec3) -> IVec3 {
    (m / cfg.voxel_meters()).floor().as_ivec3()
}

/// Every cell of the ruins, as (cell, block) pairs.
fn site(cfg: &WorldSampleConfig) -> (Vec3, Vec<(IVec3, B)>) {
    let c = center(cfg, Vec3::new(240.0, 0.0, -64.0));
    let vm = cfg.voxel_meters();
    let reach = (approach::REACH / vm).ceil() as i32;
    let o = cell(cfg, c);
    let mut out = Vec::new();
    for z in -reach..=reach {
        for x in -reach..=reach {
            let column = crate::sample_column(cfg, o.x + x, o.z + z);
            let top = column.height;
            for y in top - 4..top + (12.0 / vm) as i32 {
                let p = IVec3::new(o.x + x, y, o.z + z);
                if let Some(b) = block(cfg, p, column) {
                    out.push((p, b));
                }
            }
        }
    }
    (c, out)
}

#[test]
fn approach_ruins_exist_only_from_their_gate() {
    let (_, old) = site(&cfg(approach::VERSION - 1));
    let (_, new) = site(&cfg(approach::VERSION));
    let count = |b: B| new.iter().filter(|(_, x)| *x == b).count();
    assert!(count(B::Limestone) > 100 && count(B::DressedStone) > 100 && count(B::Crystal) > 0,
        "gate 89 columns, arches and ring: {} {} {}", count(B::Limestone), count(B::DressedStone), count(B::Crystal));
    assert!(!old.iter().any(|(_, b)| matches!(b, B::Limestone | B::DressedStone)), "v88 has no ruins");
}

#[test]
fn every_approach_torch_hangs_on_stone() {
    let cfg = cfg(approach::VERSION);
    let (_, cells) = site(&cfg);
    let torches: Vec<_> = cells.iter().filter(|(_, b)| b.torch_support_offset().is_some()).collect();
    assert!(torches.len() >= 4, "doorway and arch torches: {}", torches.len());
    for (p, b) in torches {
        let support = *p + b.torch_support_offset().unwrap_or_default();
        assert!(crate::sample_block(&cfg, support).is_solid(), "{b:?} at {p} hangs on air");
    }
}

#[test]
fn the_road_centre_is_walkable_to_the_tower() {
    let cfg = cfg(approach::VERSION);
    let (c, cells) = site(&cfg);
    let vm = cfg.voxel_meters();
    let offset = |p: IVec3| (p.as_vec3() + 0.5) * vm - c;
    // The paved axis is the cardinal direction with the most paving on it.
    let axes = [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z];
    let on = |axis: Vec3, p: IVec3| {
        let d = offset(p);
        let along = d.x * axis.x + d.z * axis.z;
        let across = d.x * axis.z - d.z * axis.x;
        (16.0..54.0).contains(&along) && across.abs() < 0.6
    };
    let paved = |axis: Vec3| cells.iter()
        .filter(|(p, b)| matches!(b, B::StoneBricks | B::WeatheredBricks) && on(axis, *p)).count();
    let axis = axes.into_iter().max_by_key(|a| paved(*a)).unwrap_or(Vec3::X);
    assert!(paved(axis) > 20, "road paving found: {}", paved(axis));
    let blocked: Vec<_> = cells.iter().filter(|(p, b)| {
        let h = (p.y - crate::sample_column(&cfg, p.x, p.z).height) as f32 * vm;
        b.is_solid() && on(axis, *p) && h > 0.0 && h < 2.0
    }).collect();
    assert!(blocked.is_empty(), "road obstructed at {:?}", &blocked[..blocked.len().min(6)]);
}

#[test]
fn threshold_stays_clear_and_intact() {
    let cfg = cfg(approach::VERSION);
    let c = center(&cfg, Vec3::new(240.0, 0.0, -64.0));
    let base = floor(&cfg, c);
    for (x, y) in [(-2.8, 2.0), (2.8, 2.0), (0.0, 5.4)] {
        let p = cell(&cfg, c + Vec3::new(x, base + y, 0.0));
        assert_eq!(crate::sample_block(&cfg, p), B::Basalt, "arch stone at {x},{y}");
    }
    for z in [-3.0, -1.0, 0.0, 1.0] {
        for y in [0.25, 1.0, 1.75] {
            let p = cell(&cfg, c + Vec3::new(0.0, base + y, z));
            assert!(!crate::sample_block(&cfg, p).is_solid(), "threshold blocked at z {z} y {y}");
        }
    }
}

/// Capture poses for `tools/Review-Portals.ps1`:
/// `cargo test -p engine_worldgen review_poses -- --ignored --nocapture`.
#[test]
#[ignore]
fn review_poses() {
    let cfg = WorldSampleConfig { voxel_scale: VoxelScalePreset::OneMeter, ..cfg(approach::VERSION) };
    let c = center(&cfg, Vec3::new(240.0, 0.0, -64.0));
    let a = approach::road_axis(&cfg, c);
    println!("centre {:.1} {:.1} floor {:.1} road ({}, {})", c.x, c.z, floor(&cfg, c), a.x, a.y);
    let yaw = |f: glam::Vec2| f.x.atan2(-f.y);
    let far = glam::Vec2::new(c.x, c.z) + a * 62.0;
    println!("road view: {:.1},{:.1} yaw {:.3}", far.x, far.y, yaw(-a));
}
