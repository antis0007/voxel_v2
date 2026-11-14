use super::*;
use super::geom;
use crate::{WorldSampleConfig, WorldType, sample_block};
use engine_core::VoxelScalePreset;
use glam::{IVec3, Vec3};

fn cfg(kind: WorldType) -> WorldSampleConfig {
    WorldSampleConfig {
        planet: None,
        generator_version: VERSION,
        seed: crate::test_world::seed(),
        world_type: kind,
        voxel_scale: VoxelScalePreset::OneMeter,
    }
}

fn cell(m: Vec3) -> IVec3 {
    m.floor().as_ivec3()
}

#[test]
fn chart_is_walkable_and_marked() {
    let cfg = cfg(WorldType::Hall);
    for p in geom::route_samples() {
        let floor = cell(Vec3::new(p.x, p.y - 0.5, p.z));
        let head = cell(Vec3::new(p.x, p.y + 1.2, p.z));
        let under = sample_block(&cfg, floor);
        let above = sample_block(&cfg, head);
        assert!(under.is_solid(), "no floor at {p:?} ({under:?})");
        assert!(!above.is_solid() && above != crate::BlockId::Lava, "blocked at {p:?} ({above:?})");
        let marked = (-2..=2).any(|dx| (-2..=2).any(|dz| {
            sample_block(&cfg, floor + IVec3::new(dx, 0, dz)) == crate::BlockId::Crystal
        }));
        assert!(marked, "chart missing near {p:?}");
    }
}

#[test]
fn three_routes_keep_their_hazards() {
    let cfg = cfg(WorldType::Hall);
    let at = |x, y, z| sample_block(&cfg, IVec3::new(x, y, z));
    assert_eq!(at(-40, 48, 19), crate::BlockId::Lava);
    assert_eq!(at(-40, 48, 23), crate::BlockId::Water);
    assert_eq!(at(40, 32, 30), crate::BlockId::Water);
    assert!(ring_cells(&cfg).iter().any(|p| sample_block(&cfg, *p) == crate::BlockId::StarGlass));
    assert_eq!(at(0, 73, 95), crate::BlockId::CopperBlock);
    assert_eq!(at(-9, 48, 22), crate::BlockId::Workbench);
}

#[test]
fn seeded_hazards_follow_the_seed_and_leave_the_route() {
    let mut cfg = cfg(WorldType::Hall);
    cfg.generator_version = crate::SEEDED_REALMS;
    let mut differed = false;
    for seed in [1_u32, 9, 44, 90] {
        cfg.seed = seed;
        for p in geom::route_samples() {
            let floor = cell(Vec3::new(p.x, p.y - 0.5, p.z));
            let head = cell(Vec3::new(p.x, p.y + 1.2, p.z));
            assert!(sample_block(&cfg, floor).is_solid(), "seed {seed} floor {p:?}");
            let above = sample_block(&cfg, head);
            assert!(!above.is_solid() && above != crate::BlockId::Lava, "seed {seed} blocked {p:?}");
        }
        let classic = {
            let mut old = cfg;
            old.generator_version = VERSION;
            sample_block(&old, IVec3::new(-40, 48, 19))
        };
        if sample_block(&cfg, IVec3::new(-40, 48, 19)) != classic {
            differed = true;
        }
    }
    assert!(differed, "seed never moved the foundry lava");
}

#[test]
fn warden_floors_match_the_column() {
    let cfg = cfg(WorldType::Hall);
    for (x, z, y) in [(0, 4, 48.0), (-40, 30, 48.0), (0, 68, 64.0), (30, 30, 32.0)] {
        let column = column(&cfg, x, z);
        assert_eq!(column.height, y as i32 - 1, "column {x},{z}");
        assert!(sample_block(&cfg, IVec3::new(x, column.height, z)).is_solid());
        assert_eq!(sample_block(&cfg, IVec3::new(x, column.height + 1, z)), crate::BlockId::Air);
        assert!(crate::tree_profile_uncached(&cfg, x, z).is_none());
    }
    let stand = spawn(&cfg);
    assert!(stand.y > 40.0);
}

#[test]
fn departure_arch_is_versioned() {
    let mut hills = cfg(WorldType::Hills);
    let feet = departure_metres(&hills);
    assert_eq!(standing(&hills, feet + Vec3::Y * 0.2), Some(Chart::Hall));
    let floor = cell(feet - Vec3::Y * 0.5);
    assert!(sample_block(&hills, floor).is_solid());
    assert_eq!(sample_block(&hills, cell(feet + Vec3::Y * 0.5)), crate::BlockId::Air);
    hills.generator_version = VERSION - 1;
    assert!(standing(&hills, feet + Vec3::Y * 0.2).is_none());
}

#[test]
fn throne_arches_name_the_other_realms() {
    let hall = cfg(WorldType::Hall);
    assert_eq!(standing(&hall, Vec3::new(0.0, geom::COURT_Y, 2.0)), Some(Chart::Hills));
    assert_eq!(standing(&hall, Vec3::new(-6.0, geom::THRONE_Y, 94.0)), Some(Chart::Nether));
    assert_eq!(standing(&hall, Vec3::new(6.0, geom::THRONE_Y, 94.0)), Some(Chart::Manyfold));
    assert_eq!(encounters(Vec3::new(0.0, geom::COURT_Y, 8.0)).len(), 2);
    for chart in [Chart::Hall, Chart::Nether, Chart::Manyfold] {
        let kind = match chart {
            Chart::Hall => WorldType::Hall,
            Chart::Nether => WorldType::Nether,
            Chart::Manyfold => WorldType::Manyfold,
            Chart::Hills => WorldType::Hills,
        };
        let there = cfg(kind);
        let feet = arrival_voxels(&hall, chart);
        let body = feet.floor().as_ivec3();
        assert!(!sample_block(&there, body).is_solid(), "{chart:?} body");
        if chart != Chart::Manyfold {
            let support = (feet - Vec3::Y * 0.1).floor().as_ivec3();
            assert!(sample_block(&there, support).is_solid(), "{chart:?} footing");
        }
    }
}
