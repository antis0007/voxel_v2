//! Runs the isolated physical solver invariants even when unrelated legacy
//! worldgen unit-test modules fail to compile. Includes the production source.
#![allow(dead_code)]
#[path = "../src/geo/grid.rs"]
mod grid;
#[path = "../src/geo/noise.rs"]
mod noise;
#[path = "../src/geo/erosion.rs"]
mod erosion;
#[path = "../src/geo/mountain.rs"]
mod mountain;

#[test]
fn historical_recipe_is_unchanged_and_new_relief_is_scale_independent() {
    use engine_core::VoxelScalePreset::{OneMeter, HalfMeter, QuarterMeter};
    use engine_worldgen::{geo, WorldSampleConfig, WorldType};
    let legacy = WorldSampleConfig { generator_version: 241, seed: 2738958700,
        world_type: WorldType::Hills, voxel_scale: OneMeter, planet: None };
    // Recorded from the unmodified production build, including drainage and climate.
    assert_eq!(geo::planet_for(&legacy).checksum, 0xf1da_d00e_ad29_7448);
    let candidate = WorldSampleConfig { generator_version: geo::MOUNTAIN_EROSION_GENERATOR, ..legacy };
    // The faster routing/transport implementation must preserve 252 bit for bit.
    assert_eq!(geo::planet_for(&candidate).checksum, 0xf762_7e4a_b2a9_741b);
    let mut first_hash = None;
    for scale in [OneMeter, HalfMeter, QuarterMeter] {
        let cfg = WorldSampleConfig { voxel_scale: scale, ..candidate };
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for z in -8..=8 { for x in -8..=8 {
            let (x,z) = (x as f32 * 128.0, z as f32 * 128.0);
            let h = geo::ground(&cfg, x, z, 1.0).top_m;
            hash = (hash ^ u64::from(h.to_bits())).wrapping_mul(0x0100_0000_01b3);
            let a = geo::ground(&cfg, x, z, 31.999).top_m;
            let b = geo::ground(&cfg, x, z, 32.001).top_m;
            assert!((a-b).abs() < 0.001, "LOD step at ({x},{z}): {a} -> {b}");
            let left = geo::ground(&cfg, x-0.001, z, 1.0).top_m;
            let right = geo::ground(&cfg, x+0.001, z, 1.0).top_m;
            assert!((left-right).abs() < 0.02, "spatial seam at ({x},{z})");
        }}
        assert_eq!(*first_hash.get_or_insert(hash), hash, "physical shape changed at {scale:?}");
    }
    assert_ne!(geo::planet_for(&legacy).checksum, geo::planet_for(&candidate).checksum);
}

#[test]
fn compact_route_preserves_drainage_and_accumulation() {
    let grid = grid::Grid { n: 37, cell_m: 12.0, min_m: [-222.0; 2] };
    for scenario in 0..4 {
        let fixed: Vec<_> = (0..grid.len()).map(|i| scenario == 3 || grid.on_border(i)
            || (i % grid.n < 12 && i / grid.n < 25)).collect();
        let h: Vec<_> = (0..grid.len()).map(|i| if fixed[i] { -3.0 }
            else { match scenario { 0 => 20.0, 1 => ((i * 7919) % 31) as f64,
                _ => 0.03 * (i % grid.n).pow(2) as f64 } }).collect();
        let rain: Vec<_> = (0..grid.len()).map(|i| ((i * 37) % 19) as f64 / 13.0).collect();
        let old = erosion::route(&grid, &h, &fixed, &rain);
        let new = erosion::route_compact(&grid, &h, &fixed, &rain);
        assert_eq!(old.receiver, new.receiver);
        assert_eq!(old.spill, new.spill);
        assert_eq!(old.area, new.area);
        let mut rank = vec![0; grid.len()];
        for (r, &i) in new.order.iter().enumerate() { rank[i as usize] = r; }
        assert_eq!(new.order.len(), grid.len());
        for i in 0..grid.len() {
            assert!(fixed[i] || rank[new.receiver[i] as usize] < rank[i]);
        }
    }
}

#[test]
fn fracture_history_records_actual_transport_and_only_wet_incision() {
    let grid = grid::Grid { n: 41, cell_m: 12.0, min_m: [-246.0; 2] };
    let fixed: Vec<_> = (0..grid.len()).map(|i| grid.on_border(i)).collect();
    let base: Vec<_> = (0..grid.len()).map(|i| {
        let [x,z] = grid.at(i); 20.0 + 0.15 * (z + 246.0) + 0.3 * x.abs()
    }).collect();
    let mut wet = base.clone();
    let mut dry = base.clone();
    let damage = vec![1.0; grid.len()];
    let record = mountain::weather(&grid, &mut wet, &fixed, &vec![1.0; grid.len()], 1.0, Some(&damage)).unwrap();
    let dry_record = mountain::weather(&grid, &mut dry, &fixed, &vec![0.0; grid.len()], 1.0, Some(&damage)).unwrap();
    assert_eq!(dry, base);
    assert_eq!(dry_record.fluvial_removal_m3, 0.0);
    assert!(record.fluvial_removal_m3 > 0.0);
    assert_eq!(record.steps, 8);
    let volume_loss = base.iter().zip(&wet).map(|(a,b)| a-b).sum::<f64>() * grid.cell_m.powi(2);
    assert!((volume_loss - record.fluvial_removal_m3 - record.boundary_export_m3).abs() < 1e-6);
    for i in 0..grid.len() {
        let reconstructed = record.before_m[i] + record.hillslope_net_m[i] - record.fluvial_incision_m[i];
        assert!((f64::from(reconstructed) - wet[i]).abs() < 2e-5);
        assert!(!fixed[i] || wet[i] == base[i]);
    }
}

#[test]
fn history_is_explicit_scale_independent_and_not_invented_for_old_worlds() {
    use engine_core::VoxelScalePreset::{OneMeter, HalfMeter, QuarterMeter};
    use engine_worldgen::{geo, WorldSampleConfig, WorldType};
    let cfg = WorldSampleConfig { generator_version: geo::RAVINE_GENERATOR, seed: 7,
        world_type: WorldType::Ridged, voxel_scale: OneMeter, planet: None };
    assert!(geo::history::sample(&WorldSampleConfig { generator_version: 252, ..cfg }, 0.0, 0.0).is_none());
    assert!(geo::history::sample(&WorldSampleConfig { world_type: WorldType::Planet, ..cfg }, 0.0, 0.0).is_none());
    assert!(geo::history::sample(&cfg, f64::NAN, 0.0).is_none());
    assert!(geo::history::sample(&cfg, 1e10, 0.0).is_none());
    let first = geo::history::sample(&cfg, -352.0, -288.0).unwrap();
    assert_eq!(first.effective_erosion_steps, 8);
    assert!(first.fracture_damage >= 0.0 && first.fracture_damage <= 1.0);
    for scale in [HalfMeter, QuarterMeter] {
        let scaled = WorldSampleConfig { voxel_scale: scale, ..cfg };
        assert_eq!(Some(first), geo::history::sample(&scaled, -352.0, -288.0));
        for z in -8..=8 { for x in -8..=8 {
            let (x,z) = (x as f32 * 128.0, z as f32 * 128.0);
            assert_eq!(geo::ground(&cfg,x,z,1.0).top_m, geo::ground(&scaled,x,z,1.0).top_m);
            let a = geo::ground(&scaled,x,z,31.999).top_m;
            let b = geo::ground(&scaled,x,z,32.001).top_m;
            assert!((a-b).abs() < 0.001);
        }}
    }
    let budget = geo::history::budget(&cfg).unwrap();
    assert!(budget.fluvial_removal_m3 > 0.0);
    assert!(budget.hillslope_boundary_export_m3 >= 0.0);
}
