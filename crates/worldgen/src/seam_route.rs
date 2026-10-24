//! Chunk-seam route: a fixed walk of chunk coordinates, negative and far from
//! the origin, checking that the padded halo, single-voxel sampling and the
//! unpadded chunk agree on every shared cell. Ore bodies and geodes are placed
//! by position, so a body that crosses a chunk face must look the same from
//! both sides. This is the automated half of the `seam_route` check
//! (`just seam-route`); the GPU half is `tools/Review-HorizonContinuity.ps1`.
use super::*;
use engine_core::VoxelScalePreset;
use std::sync::atomic::AtomicBool;

/// (chunk x, chunk z): across the origin, deep negative, and ~96 000 voxels out.
const ROUTE: [(i32, i32); 8] = [(-3000, -2999), (-33, 17), (-2, -1), (-1, -1), (0, 0), (1, 0), (7, -5), (3000, 2999)];
/// Underground and surface bands where strata, ores and caves meet.
const LEVELS: [i32; 4] = [-1, 0, 1, 2];

fn check_chunk(cfg: &WorldSampleConfig, coord: IVec3) {
    let size = PADDED_CHUNK_SIZE;
    let padded = generate_padded(cfg, coord, &AtomicBool::new(false)).unwrap();
    let chunk = generate(cfg, coord);
    let at = |x: i32, y: i32, z: i32| padded.blocks[((y * size + z) * size + x) as usize];
    let ctx = format!("v{} {:?} chunk {coord}", cfg.generator_version, cfg.voxel_scale);
    for z in 0..size {
        for y in 0..size {
            for x in 0..size {
                let on_face = [x, y, z].iter().any(|&v| v == 0 || v == size - 1);
                let p = padded.origin + IVec3::new(x, y, z);
                if on_face {
                    assert_eq!(at(x, y, z), sample_block(cfg, p), "{ctx}: halo cell {p} differs from direct sampling");
                } else {
                    let local = ChunkData::local_index(x - 1, y - 1, z - 1);
                    assert_eq!(at(x, y, z), chunk.blocks[local], "{ctx}: padded interior {p} differs from generate()");
                }
            }
        }
    }
}

#[test]
fn halo_sampling_and_chunk_generation_agree_along_the_route() {
    for version in [33u32, CURRENT_GENERATOR_VERSION] {
        for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter] {
            let cfg = WorldSampleConfig { generator_version: version, seed: 424242, world_type: WorldType::Hills, voxel_scale: scale };
            for &(cx, cz) in &ROUTE {
                for cy in LEVELS {
                    check_chunk(&cfg, IVec3::new(cx, cy, cz));
                }
            }
        }
    }
}

/// Padded neighbours overlap by two cells: a's local x 32..=33 are b's 0..=1.
/// An ore body or geode split by a chunk border must read the same from both.
#[test]
fn neighbouring_chunks_share_their_overlap_cells() {
    let cfg = WorldSampleConfig {
        generator_version: CURRENT_GENERATOR_VERSION,
        seed: 7,
        world_type: WorldType::Hills,
        voxel_scale: VoxelScalePreset::OneMeter,
    };
    let never = AtomicBool::new(false);
    let size = PADDED_CHUNK_SIZE;
    let index = |x: i32, y: i32, z: i32| ((y * size + z) * size + x) as usize;
    for &(cx, cz) in &ROUTE {
        for cy in LEVELS {
            let a = IVec3::new(cx, cy, cz);
            let pa = generate_padded(&cfg, a, &never).unwrap();
            let pb = generate_padded(&cfg, a + IVec3::X, &never).unwrap();
            for z in 0..size {
                for y in 0..size {
                    for k in 0..2 {
                        assert_eq!(pa.blocks[index(32 + k, y, z)], pb.blocks[index(k, y, z)], "x seam after chunk {a}");
                    }
                }
            }
        }
    }
}
