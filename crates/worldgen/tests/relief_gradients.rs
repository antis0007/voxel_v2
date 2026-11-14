#[path = "../src/planetary/slope_relief/gradient.rs"]
mod gradient;
// Transport unit tests run in the library, where their private module paths resolve.

#[test]
fn historical_headwaters_and_interleaved_cache_remain_identical() {
    use engine_core::VoxelScalePreset::OneMeter;
    use engine_worldgen::{WorldSampleConfig, WorldType, planetary};
    use glam::DVec2;
    let old=WorldSampleConfig { planet:None, seed:74, generator_version:266,
        world_type:WorldType::Planet, voxel_scale:OneMeter };
    let new=WorldSampleConfig { generator_version:planetary::DRAINAGE_RELIEF_GENERATOR,..old };
    let origin=DVec2::new(343580.71852828487,20453.256840943533);
    let across=DVec2::new(0.9996345010145292,0.02703450353591003);
    let down=DVec2::new(across.y,-across.x);
    // Recorded 4 m samples from the frozen, unmodified generator 266.
    for (index,bits) in [(100,0x43d4041e),(500,0x43bf485d),(900,0x43287913)] {
        let p=origin+(across+down)*(2.0+4.0*f64::from(index));
        let sample=|cfg:&WorldSampleConfig| planetary::ground(cfg,p.x as f32,p.y as f32,4.0);
        assert_eq!(sample(&old).top_m.to_bits(),bits);
        let candidate=sample(&new);
        assert!(candidate.top_m.is_finite() && candidate.slope.is_finite());
        assert_eq!(sample(&old).top_m.to_bits(),bits);
        assert_eq!(sample(&new).top_m.to_bits(),candidate.top_m.to_bits());
    }
}

#[test]
fn relief_matches_physical_scales_and_crosses_tile_and_lod_boundaries() {
    use engine_core::VoxelScalePreset::{OneMeter,HalfMeter,QuarterMeter};
    use engine_worldgen::{WorldSampleConfig, WorldType, planetary};
    let base=WorldSampleConfig { planet:None, seed:74,
        generator_version:planetary::DRAINAGE_RELIEF_GENERATOR,
        world_type:WorldType::Planet, voxel_scale:OneMeter };
    for tx in 36..=42 {
        // Exact 448 m tile boundary in the NegZ chart, including a four-tile corner.
        let x=327680.0+tx as f32*448.0;
        let z=42.0*448.0;
        for footprint in [8.0,16.0,32.0,36.0,96.0] {
            let sample=|cfg:&WorldSampleConfig,x,z,fp| planetary::ground(cfg,x,z,fp).top_m;
            let h=sample(&base,x,z,footprint);
            for scale in [HalfMeter,QuarterMeter] {
                assert_eq!(h.to_bits(),sample(&WorldSampleConfig {voxel_scale:scale,..base},x,z,footprint).to_bits());
            }
            assert!((sample(&base,x-0.0625,z,footprint)-sample(&base,x+0.0625,z,footprint)).abs()<0.5);
            assert!((sample(&base,x,z-0.0625,footprint)-sample(&base,x,z+0.0625,footprint)).abs()<0.5);
            assert!((sample(&base,x,z,footprint-0.001)-sample(&base,x,z,footprint+0.001)).abs()<0.01);
        }
    }
}
