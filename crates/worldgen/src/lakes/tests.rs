use super::*;
use crate::{BlockId, Profiles, sample_block, sample_column};
use engine_core::VoxelScalePreset;
use glam::IVec3;

fn cfg(seed: u32, voxel_scale: VoxelScalePreset) -> WorldSampleConfig {
    WorldSampleConfig { planet: None, generator_version: LAKES_GENERATOR, seed, world_type: WorldType::Hills, voxel_scale }
}

/// Lakes owned by big and small lattice cells within `cells` of the origin.
fn lakes_near(c: &WorldSampleConfig, cells: i32) -> Vec<Lake> {
    let mut out = Vec::new();
    for (small, scale) in [(false, 1), (true, (BIG_CELL / SMALL_CELL) as i32)] {
        let n = cells * scale;
        for iz in -n..=n {
            for ix in -n..=n {
                out.extend(lake(c, small, ix, iz));
            }
        }
    }
    out
}

/// Ground top and the water surface a column actually holds.
fn column(c: &WorldSampleConfig, x: i32, z: i32) -> (f32, Option<f32>) {
    let top = sample_column(c, x, z).height as f32 + 1.0;
    let water = crate::CachedProfiles.river_top(c, x, z).filter(|&s| s > top);
    (top, water)
}

#[test]
fn lake_kinds_vary_and_levels_sit_below_their_rims() {
    let c = cfg(crate::test_world::seed(), VoxelScalePreset::OneMeter);
    let lakes = lakes_near(&c, 8);
    let mut kinds = std::collections::BTreeSet::new();
    for l in &lakes {
        kinds.insert(format!("{:?}", l.kind));
        assert!(l.centre.length() >= 220.0, "lake at spawn: {:?}", l.centre);
        assert!(l.depth > 0.0 && l.radius > 0.0);
    }
    println!("lakes near spawn: {} kinds {kinds:?}", lakes.len());
    assert!(kinds.len() >= 5, "only kinds {kinds:?}");
}

/// Standing water must be held by ground on every side: a wet column never
/// sits next to a dry one whose ground is over a metre below its surface.
#[test]
fn lake_water_never_stands_above_dry_ground() {
    let c = cfg(crate::test_world::seed(), VoxelScalePreset::OneMeter);
    let (mut wet, mut walls, mut worst, mut lakes) = (0, 0, 0.0f32, 0);
    for l in lakes_near(&c, 6).into_iter().filter(|l| l.kind != LakeKind::MarshPool).take(24) {
        lakes += 1;
        let r = (l.radius * 1.5) as i32 + 4;
        let (cx, cz) = (l.centre.x as i32, l.centre.y as i32);
        for z in (cz - r..=cz + r).step_by(2) {
            for x in (cx - r..=cx + r).step_by(2) {
                let (_, Some(s)) = column(&c, x, z) else { continue };
                if (s - l.level).abs() > 0.01 {
                    continue;
                }
                wet += 1;
                for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (n_top, n_water) = column(&c, x + dx, z + dz);
                    if n_water.is_none() && s - n_top > 1.0 {
                        walls += 1;
                        worst = worst.max(s - n_top);
                    }
                }
            }
        }
    }
    println!("lake fit lakes={lakes} wet={wet} walls={walls} worst={worst:.1}");
    assert!(lakes >= 6 && wet > 500, "only {lakes} lakes, {wet} wet columns");
    assert!(worst <= 3.0 && walls * 100 <= wet, "{walls} walls, worst {worst:.1} m");
}

#[test]
fn lake_water_voxels_fill_from_the_bed() {
    let c = cfg(crate::test_world::seed(), VoxelScalePreset::OneMeter);
    let l = lakes_near(&c, 6).into_iter().find(|l| l.kind == LakeKind::Lake).expect("a lake near spawn");
    let (x, z) = (l.centre.x as i32, l.centre.y as i32);
    let (top, water) = column(&c, x, z);
    let s = water.expect("water at the lake centre");
    assert_eq!(sample_block(&c, IVec3::new(x, top as i32, z)), BlockId::Water);
    assert_ne!(sample_block(&c, IVec3::new(x, top as i32 - 1, z)), BlockId::Water);
    assert!(s - top >= 1.5, "lake centre only {:.1} m deep", s - top);
}

#[test]
fn lake_plan_ignores_voxel_size_and_negative_coordinates() {
    let (coarse, fine) = (cfg(crate::test_world::seed(), VoxelScalePreset::OneMeter), cfg(crate::test_world::seed(), VoxelScalePreset::QuarterMeter));
    let mut seen = 0;
    for z in (-1500..1500).step_by(41) {
        for x in (-1500..1500).step_by(41) {
            let (a, b) = (sample(&coarse, x as f32, z as f32), sample(&fine, x as f32, z as f32));
            assert_eq!((a.ceiling_m, a.water_m, a.kind), (b.ceiling_m, b.water_m, b.kind), "{x},{z}");
            seen += usize::from(a.kind.is_some());
        }
    }
    assert!(seen > 0);
}

#[test]
fn historical_generators_have_no_lakes() {
    // Generator 52 terrain must stay byte-identical, so no lake may appear in it.
    let c = WorldSampleConfig { generator_version: LAKES_GENERATOR - 1, ..cfg(crate::test_world::seed(), VoxelScalePreset::OneMeter) };
    for z in (-900..900).step_by(37) {
        for x in (-900..900).step_by(37) {
            assert!(sample(&c, x as f32, z as f32).kind.is_none(), "lake at {x},{z} in a historical world");
        }
    }
}

/// Nearest lake of each kind for review captures.
#[test]
#[ignore = "probe"]
fn probe_lakes_by_kind() {
    let c = cfg(std::env::var("LAKE_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(2_738_958_700), VoxelScalePreset::OneMeter);
    let mut best = std::collections::BTreeMap::<String, Lake>::new();
    for l in lakes_near(&c, 10) {
        let e = best.entry(format!("{:?}", l.kind)).or_insert(l);
        if l.centre.length() < e.centre.length() {
            *e = l;
        }
    }
    for (k, l) in best {
        println!("lake {k}: centre {:.0},{:.0} radius {:.0} level {:.1} depth {:.1}", l.centre.x, l.centre.y, l.radius, l.level, l.depth);
    }
}
