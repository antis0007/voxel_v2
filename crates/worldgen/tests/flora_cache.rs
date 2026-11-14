//! Fingerprints recorded before placement caching; includes blocks, water and skylight.
use engine_core::{CHUNK_SIZE, VoxelScalePreset};
use engine_worldgen::{WorldSampleConfig, WorldType};
use glam::IVec3;
use std::sync::atomic::AtomicBool;
fn hash(t: &engine_worldgen::PaddedTerrain) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    let mut add = |v: u64| {
        h = (h ^ v).wrapping_mul(0x100000001b3);
    };
    for b in &t.blocks {
        add(b.as_u32() as u64);
    }
    for y in &t.bed_height {
        add(*y as u32 as u64);
    }
    for y in &t.bed_frac {
        add(y.to_bits() as u64);
    }
    for (p, a) in &t.partial_water {
        for v in p.to_array() {
            add(v as u32 as u64);
        }
        add(*a);
    }
    for y in &t.sky_ceiling {
        add(*y as u32 as u64);
    }
    h
}
#[test]
fn cached_flora_preserves_padded_terrain_fingerprints() {
    let expected: Vec<_> = include_str!("fixtures/flora_cache_terrain.txt")
        .lines()
        .collect();
    let mut row = 0;
    for (case, (kind, seed, version, scale)) in [
        (WorldType::Hills, 7, 209, VoxelScalePreset::OneMeter),
        (WorldType::Hills, 7, 241, VoxelScalePreset::OneMeter),
        (WorldType::Planet, 7, 241, VoxelScalePreset::OneMeter),
        (
            WorldType::Hills,
            2738958700,
            241,
            VoxelScalePreset::HalfMeter,
        ),
        (
            WorldType::Ridged,
            424242,
            241,
            VoxelScalePreset::QuarterMeter,
        ),
        (
            WorldType::Hills,
            2738958700,
            252,
            VoxelScalePreset::OneMeter,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let cfg = WorldSampleConfig {
            planet: None,
            generator_version: version,
            seed,
            world_type: kind,
            voxel_scale: scale,
        };
        for (x, z) in [(-5, 4), (2, -3)] {
            let surface = engine_worldgen::sample_column(&cfg, x * CHUNK_SIZE, z * CHUNK_SIZE)
                .height
                .div_euclid(CHUNK_SIZE);
            for y in [surface, surface + 1, surface + 3] {
                let t = engine_worldgen::generate_padded(
                    &cfg,
                    IVec3::new(x, y, z),
                    &AtomicBool::new(false),
                )
                .unwrap();
                let got = format!("{case},{x},{y},{z},{:016x}", hash(&t));
                assert_eq!(got, expected[row], "terrain, water or sky data changed");
                row += 1;
            }
        }
    }
    assert_eq!(row, expected.len());
}
