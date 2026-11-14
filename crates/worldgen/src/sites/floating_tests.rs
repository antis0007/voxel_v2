//! Homesteads and surface sites: no roof, wall or trim may float free of the
//! ground chain. Islands name the first cells so a failing shape is easy to find.
use super::*;
use crate::floating_check;
use engine_core::VoxelScalePreset;

fn cfg(scale: VoxelScalePreset) -> WorldSampleConfig {
    crate::test_world::World::with_seed(WorldType::Hills, scale, crate::test_world::seed()).sample_config()
}

fn check(cfg: &WorldSampleConfig, label: &str, x: f32, z: f32, ground: f32, reach: f32) {
    let v = cfg.voxel_meters();
    let centre = (Vec3::new(x, ground, z) / v).floor().as_ivec3();
    let half = IVec3::new((reach / v) as i32, (14.0 / v) as i32, (reach / v) as i32);
    let found = floating_check::islands(cfg, centre - half, centre + half);
    assert!(
        found.is_empty(),
        "{label}: {} floating islands\n{}",
        found.len(),
        floating_check::describe(&found)
    );
}

#[test]
fn homesteads_have_no_floating_cells() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let cfg = cfg(scale);
        // A quarter-metre homestead is ten million samples; one is the budget.
        let limit = if scale == VoxelScalePreset::QuarterMeter { 1 } else { 6 };
        let mut found = 0;
        for ix in -6..=6 {
            for iz in -6..=6 {
                let Some(h) = homestead(&cfg, ix, iz) else { continue };
                found += 1;
                check(&cfg, &format!("{scale:?} homestead {ix},{iz}"), h.x, h.z, h.ground, h.reach + 4.0);
                if found >= limit {
                    break;
                }
            }
        }
        assert!(found >= limit.min(3), "only {found} homesteads at {scale:?}");
    }
}

/// `FLOAT_CELL=x,y,z cargo test dump_cell -- --ignored --nocapture` prints the
/// blocks around one cell of the homestead seed.
#[test]
#[ignore = "prints a neighbourhood for a given cell"]
fn dump_cell() {
    let scale = match std::env::var("FLOAT_SCALE").as_deref() {
        Ok("half") => VoxelScalePreset::HalfMeter,
        Ok("quarter") => VoxelScalePreset::QuarterMeter,
        _ => VoxelScalePreset::OneMeter,
    };
    let cfg = cfg(scale);
    let spec = std::env::var("FLOAT_CELL").unwrap_or_default();
    let n: Vec<i32> = spec.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    let c = IVec3::new(n[0], n[1], n[2]);
    if let Ok(home) = std::env::var("FLOAT_HOME") {
        let h: Vec<i32> = home.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        if let Some(h) = homestead(&cfg, h[0], h[1]) {
            let wings: Vec<_> = h.building.wings.iter().map(|w| (w.x0, w.z0, w.nx, w.nz, w.storeys, w.h)).collect();
            println!("homestead at ({}, {}) ground {} turn {} reach {} wings {wings:?} curtains {}", h.x, h.z, h.ground, h.turn, h.reach, h.building.curtains.len());
        }
    }
    for y in (c.y - 4..=c.y + 4).rev() {
        for z in c.z - 3..=c.z + 3 {
            let row: Vec<String> = (c.x - 3..=c.x + 3)
                .map(|x| format!("{:?}", crate::sample_block(&cfg, IVec3::new(x, y, z))))
                .collect();
            println!("y={y} z={z}: {}", row.join(" "));
        }
    }
}

#[test]
fn surface_sites_have_no_floating_cells() {
    let cfg = cfg(VoxelScalePreset::OneMeter);
    let mut seen = [false; 6];
    for ring in 0i32..=6 {
        for rz in -ring..=ring {
            for rx in -ring..=ring {
                if rx.abs().max(rz.abs()) != ring {
                    continue;
                }
                let Some(s) = site(&cfg, rx, rz) else { continue };
                if seen[s.kind as usize] {
                    continue;
                }
                seen[s.kind as usize] = true;
                check(&cfg, &format!("site kind {} at {rx},{rz}", s.kind), s.x, s.z, s.ground, 22.0);
            }
        }
    }
    assert!(seen.iter().filter(|s| **s).count() >= 3, "too few site kinds found: {seen:?}");
}
