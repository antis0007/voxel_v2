//! Stage-isolated generation timing for the v15/v16 terrain cost regression.
//!
//! Each stage runs as its own whole pass over the same 80 chunks as
//! `terrain_generation_cost_stays_close_to_the_previous_generator`, in the
//! real call order and with no per-voxel timers, so one stage cannot warm a
//! cache for another. Run:
//! `cargo test gen_probe -- --ignored --nocapture --test-threads=1`
use super::*;
use std::time::Instant;

fn chunks() -> impl Iterator<Item = IVec3> {
    (6..10).flat_map(|cx| (-2..2).flat_map(move |cz| (-2..3).map(move |cy| IVec3::new(cx, cy, cz))))
}

fn cfg(version: u32) -> WorldSampleConfig {
    WorldSampleConfig {
        generator_version: version,
        seed: 424242,
        world_type: WorldType::Hills,
        voxel_scale: crate::fluid::VoxelScalePreset::OneMeter,
    }
}

/// Warm once, then best of three.
fn best(mut pass: impl FnMut() -> usize) -> (f64, usize) {
    let sink = pass();
    let mut best = f64::MAX;
    for _ in 0..3 {
        let t = Instant::now();
        std::hint::black_box(pass());
        best = best.min(t.elapsed().as_secs_f64());
    }
    (best, sink)
}

fn each_voxel(c: &WorldSampleConfig, mut f: impl FnMut(IVec3, ColumnSample, &RegionProfiles) -> bool) -> usize {
    let size = crate::mesh::PADDED_CHUNK_SIZE;
    let mut n = 0;
    for coord in chunks() {
        let origin = coord * CHUNK_SIZE - IVec3::ONE;
        let profiles = RegionProfiles::new(c, origin.x, origin.z, size);
        for z in 0..size {
            for x in 0..size {
                let column = sample_column(c, origin.x + x, origin.z + z);
                for y in 0..size {
                    n += f(origin + IVec3::new(x, y, z), column, &profiles) as usize;
                }
            }
        }
    }
    n
}

#[test]
#[ignore = "profiling probe; run explicitly"]
fn gen_probe_stage_costs() {
    let never = std::sync::atomic::AtomicBool::new(false);
    let order: Vec<u32> = std::env::var("GEN_PROBE_ORDER").map(|v| v.split(',').map(|x| x.parse().unwrap()).collect()).unwrap_or(vec![15, 16]);
    for version in order {
        let c = cfg(version);
        let full = best(|| {
            chunks()
                .map(|coord| {
                    generate_padded(&c, coord, &never)
                        .unwrap()
                        .blocks
                        .iter()
                        .filter(|b| **b != BlockId::Air)
                        .count()
                })
                .sum()
        });
        let columns = best(|| each_voxel(&c, |_, _, _| false));
        let chain = best(|| {
            each_voxel(&c, |p, col, _| {
                undercrown::block(&c, p, col.height)
                    .or_else(|| sites::block(&c, p, col.height))
                    .or_else(|| sites::cavern(&c, p, col.height))
                    .is_some()
            })
        });
        let undercrown_only = best(|| each_voxel(&c, |p, col, _| undercrown::block(&c, p, col.height).is_some()));
        let sites_block = best(|| each_voxel(&c, |p, col, _| sites::block(&c, p, col.height).is_some()));
        let cavern = best(|| each_voxel(&c, |p, col, _| sites::cavern(&c, p, col.height).is_some()));
        let structure = best(|| {
            each_voxel(&c, |p, col, prof| p.y > col.height && structure_block(&c, p, col, prof) != BlockId::Air)
        });
        let caves = best(|| each_voxel(&c, |p, col, _| p.y <= col.height && cave_carves(&c, p, &col)));
        let per_voxel = best(|| {
            each_voxel(&c, |p, col, prof| sample_block_with(&c, p, col, prof) != BlockId::Air)
        });
        println!(
            "v{version}: generate_padded {:.3}s | columns-only {:.3}s | sample_block_with {:.3}s | \
             chain {:.3}s (undercrown {:.3}s, sites::block {:.3}s, cavern {:.3}s) | \
             structure_block {:.3}s | cave_carves {:.3}s | solid {}",
            full.0, columns.0, per_voxel.0, chain.0, undercrown_only.0, sites_block.0, cavern.0,
            structure.0, caves.0, full.1
        );
    }
}

/// How many voxels get past `cave_carves`' cheap exits into its 3D noise, and
/// the column statistics that decide it.
#[test]
#[ignore = "profiling probe; run explicitly"]
fn gen_probe_cave_noise_reach() {
    for version in [15u32, 16] {
        let c = cfg(version);
        let (mut below, mut noise, mut cols, mut haw_sum, mut depth_sum) = (0u64, 0u64, 0u64, 0f64, 0f64);
        let size = crate::mesh::PADDED_CHUNK_SIZE;
        for coord in chunks() {
            let origin = coord * CHUNK_SIZE - IVec3::ONE;
            for z in 0..size {
                for x in 0..size {
                    let col = sample_column(&c, origin.x + x, origin.z + z);
                    cols += 1;
                    haw_sum += col.cave.height_above_water as f64;
                    for y in 0..size {
                        let p = origin + IVec3::new(x, y, z);
                        if p.y > col.height || p.y <= c.min_world_y() + c.blocks_for_span(0.75) {
                            continue;
                        }
                        below += 1;
                        let depth = (col.height - p.y) as f32 * c.voxel_meters();
                        let haw = col.cave.height_above_water;
                        if (haw < 9.0 && depth < 5.5) || (haw < 18.0 && depth < 2.5) {
                            continue;
                        }
                        noise += 1;
                        depth_sum += depth as f64;
                    }
                }
            }
        }
        println!(
            "v{version}: below-surface voxels {below}, reach cave noise {noise} ({:.1}%), mean height_above_water {:.2} m, mean noise depth {:.1} m",
            100.0 * noise as f64 / below.max(1) as f64,
            haw_sum / cols as f64,
            depth_sum / noise.max(1) as f64
        );
    }
}

/// Time `generate_padded` for one version, best of three after a warm-up.
fn time_generation(version: u32) -> f64 {
    let never = std::sync::atomic::AtomicBool::new(false);
    let c = cfg(version);
    best(|| {
        chunks()
            .map(|coord| generate_padded(&c, coord, &never).unwrap().blocks.len())
            .sum()
    })
    .0
}

/// Separate thread-local cache state from global state: the same sequence of
/// versions run on one thread, then each version on its own fresh thread.
#[test]
#[ignore = "profiling probe; run explicitly"]
fn gen_probe_thread_state() {
    let same: Vec<f64> = [15u32, 16, 15, 16].iter().map(|&v| time_generation(v)).collect();
    println!("same thread   v15 {:.3} v16 {:.3} v15 {:.3} v16 {:.3}", same[0], same[1], same[2], same[3]);
    let fresh: Vec<f64> = [15u32, 16, 15, 16]
        .iter()
        .map(|&v| std::thread::spawn(move || time_generation(v)).join().unwrap())
        .collect();
    println!("fresh threads v15 {:.3} v16 {:.3} v15 {:.3} v16 {:.3}", fresh[0], fresh[1], fresh[2], fresh[3]);
}
    #[test]
    #[ignore = "historical HEAD timing method, full workload"]
    fn gen_probe_original_order() {
        use super::BlockId as B;
        use std::sync::atomic::AtomicBool;
        let never = AtomicBool::new(false);
        let time = |version: u32| {
            let c = WorldSampleConfig {
                generator_version: version,
                seed: 424242,
                world_type: WorldType::Hills,
                voxel_scale: VoxelScalePreset::OneMeter,
            };
            let start = std::time::Instant::now();
            let mut solid = 0usize;
            for cx in 6..10 {
                for cz in -2..2 {
                    for cy in -2..3 {
                        let chunk =
                            crate::world::generate_padded(&c, glam::IVec3::new(cx, cy, cz), &never)
                                .unwrap();
                        solid += chunk.blocks.iter().filter(|b| **b != B::Air).count();
                    }
                }
            }
            (start.elapsed().as_secs_f64(), solid)
        };
        // Each version is timed in its own block. Alternating versions made them
        // evict each other from the shared landscape tile cache, which measured
        // cache thrash instead of generator cost. Best of three per version
        // keeps background load from favouring either side.
        let best = |version| {
            let _ = time(version);
            (0..3).map(|_| time(version).0).fold(f64::MAX, f64::min)
        };
        let old = best(15);
        let new = best(16);
        println!(
            "terrain generation: v15 {old:.3}s, v16 {new:.3}s ({:.2}x)",
            new / old
        );
        assert!(
            new < old * 1.35 + 0.05,
            "v16 generation is {:.2}x slower than v15",
            new / old
        );
    }
