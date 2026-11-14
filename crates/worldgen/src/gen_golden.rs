//! Bit-exactness guard for generation speed-ups. Prints one FNV-1a digest per
//! (version, seed, scale) over every voxel of a spread of padded chunks; run it
//! on a baseline tree and on the optimised tree and diff the output:
//! `cargo test gen_golden -- --ignored --nocapture --test-threads=1`
use super::*;

fn digest(cfg: &WorldSampleConfig, coords: &[IVec3]) -> u64 {
    let never = std::sync::atomic::AtomicBool::new(false);
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &c in coords {
        let chunk = generate_padded(cfg, c, &never).unwrap();
        for b in chunk.blocks.iter() {
            h = (h ^ *b as u64).wrapping_mul(0x100_0000_01b3);
        }
        for v in chunk.sky_ceiling.iter().chain(chunk.bed_height.iter()) {
            h = (h ^ *v as u32 as u64).wrapping_mul(0x100_0000_01b3);
        }
    }
    h
}

#[test]
#[ignore]
fn gen_golden() {
    use engine_core::VoxelScalePreset as S;
    // Surface, deep, sites (starter ruin near z=-42), shore, caves, dungeons.
    let mut coords = Vec::new();
    for cx in [-9, -3, 0, 2, 6, 11] {
        for cz in [-6, -1, 0, 3, 8] {
            for cy in [-3, -1, 0, 1, 2] {
                coords.push(IVec3::new(cx, cy, cz));
            }
        }
    }
    for version in [2u32, 9, 12, 15, 16, 20, 31, 32, 33, 34, CURRENT_GENERATOR_VERSION] {
        for (seed, world_type) in [(424242u32, WorldType::Hills), (7, WorldType::Hills)] {
            for scale in [S::OneMeter, S::HalfMeter] {
                let cfg = WorldSampleConfig {
                    planet: None,
                    generator_version: version,
                    seed,
                    world_type,
                    voxel_scale: scale,
                };
                println!(
                    "golden v{version} seed{seed} {:?} {:016x}",
                    scale,
                    digest(&cfg, &coords)
                );
            }
        }
    }
}
