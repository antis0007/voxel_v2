//! Terrain generation cost at the current generator version. Replaces the
//! ignored wall-clock ratio test that compared generator v15 and v16; the
//! perf gate (`just bench-check`) now compares against `baselines.json`.
use criterion::{Criterion, criterion_group, criterion_main};
use engine_content::BlockId;
use engine_core::VoxelScalePreset;
use engine_voxel::{CURRENT_GENERATOR_VERSION, WorldSampleConfig, WorldType};
use glam::IVec3;
use std::sync::atomic::AtomicBool;

fn terrain(c: &mut Criterion) {
    let cfg = WorldSampleConfig {
        planet: None,
        generator_version: CURRENT_GENERATOR_VERSION,
        seed: 424242,
        world_type: WorldType::Hills,
        voxel_scale: VoxelScalePreset::OneMeter,
    };
    let never = AtomicBool::new(false);
    // A 2×2×2 block on the hillside the old test used: surface, caves and sites.
    c.bench_function("terrain_generate_8_chunks", |b| {
        b.iter(|| {
            let mut solid = 0usize;
            for x in 6..8 {
                for z in -1..1 {
                    for y in -1..1 {
                        let chunk = engine_worldgen::generate_padded(&cfg, IVec3::new(x, y, z), &never)
                            .expect("generation is never cancelled here");
                        solid += chunk.blocks.iter().filter(|b| **b != BlockId::Air).count();
                    }
                }
            }
            std::hint::black_box(solid)
        })
    });
}

criterion_group! {
    name = benches;
    // Short runs: the gate takes the median of three whole runs instead.
    config = Criterion::default().sample_size(10).warm_up_time(std::time::Duration::from_millis(500))
        .measurement_time(std::time::Duration::from_secs(2));
    targets = terrain
}
criterion_main!(benches);
