use super::*;
use crate::{WorldType, planet, sample_column};
use engine_core::VoxelScalePreset;
use engine_core::cube_sphere::atlas;

fn cfg(generator_version: u32) -> WorldSampleConfig {
    WorldSampleConfig { planet: None, generator_version, seed: crate::test_world::seed(), world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter }
}

/// Face 1; face 0 is the solved continent and runs the hills block path.
const FACE: i32 = atlas::PITCH_M as i32;

/// Counts blocks in columns on a coarse grid, from the surface down 140 m.
fn census(c: &WorldSampleConfig) -> Vec<(BlockId, u32)> {
    let mut counts: Vec<(BlockId, u32)> = Vec::new();
    for gz in -12..12 {
        for gx in -12..12 {
            let (x, z) = (FACE + gx * 997 + 5, gz * 997 - 3);
            let column = sample_column(c, x, z);
            if column.height <= c.water_level() {
                continue;
            }
            for depth in 0..140 {
                let b = planet::block(c, IVec3::new(x, column.height - depth, z), column);
                match counts.iter_mut().find(|e| e.0 == b) {
                    Some(e) => e.1 += 1,
                    None => counts.push((b, 1)),
                }
            }
        }
    }
    counts
}

#[test]
fn deep_planets_hold_hazards_zones_and_lodes() {
    let counts = census(&cfg(VERSION));
    let has = |b: BlockId| counts.iter().any(|e| e.0 == b);
    for b in [BlockId::Lava, BlockId::Basalt, BlockId::Crystal, BlockId::GoldQuartz, BlockId::IronOre, BlockId::CoalOre] {
        assert!(has(b), "no {b:?} in {counts:?}");
    }
    let old = census(&cfg(VERSION - 1));
    assert!(!old.iter().any(|e| e.0 == BlockId::Lava), "lava before the gate: {old:?}");
}

#[test]
fn lodes_get_richer_with_depth() {
    assert_eq!(lode_ore(10.0, 0.1), BlockId::CoalOre);
    assert_eq!(lode_ore(120.0, 0.1), BlockId::GoldQuartz);
    for d in [10.0, 40.0, 80.0, 120.0] {
        for pick in [0.0, 0.5, 0.99] {
            assert!(crate::ore_geology::is_scattered_ore(lode_ore(d, pick)) || lode_ore(d, pick) == BlockId::GoldQuartz);
        }
    }
}

#[test]
fn shallow_lodes_mark_the_surface() {
    let c = cfg(VERSION);
    let mut tells = 0;
    for gz in -40..40 {
        for gx in -40..40 {
            let (x, z) = (FACE + gx * 37, gz * 37);
            let column = sample_column(&c, x, z);
            if column.height > c.water_level() && surface_tell(&c, x, z, &column).is_some() {
                tells += 1;
            }
        }
    }
    assert!(tells > 0, "no surface tells in 6400 columns");
}

#[test]
fn every_zone_occurs() {
    let c = cfg(VERSION);
    let mut seen = Vec::new();
    for gz in -20..20 {
        for gx in -20..20 {
            let (x, z) = (FACE + gx * 613, gz * 613);
            let column = sample_column(&c, x, z);
            for depth in (10..130).step_by(6) {
                let zone = zone(&c, IVec3::new(x, column.height - depth, z), &column);
                if !seen.contains(&zone) {
                    seen.push(zone);
                }
            }
        }
    }
    assert!(seen.len() >= 4, "zones {seen:?}");
}

#[test]
fn the_solved_spawn_face_gets_the_same_zones() {
    let c = cfg(VERSION);
    let mut lava_or_crystal = false;
    let mut lodes = 0;
    'scan: for gz in -6..6 {
        for gx in -6..6 {
            let (x, z) = (gx * 1_400 + 9, gz * 1_400 - 11);
            if !crate::basin::covers(&c, x, z) {
                continue;
            }
            let column = sample_column(&c, x, z);
            for depth in 4..140 {
                let b = crate::sample_block(&c, IVec3::new(x, column.height - depth, z));
                lava_or_crystal |= matches!(b, BlockId::Lava | BlockId::Crystal | BlockId::Basalt);
                lodes += u32::from(matches!(b, BlockId::GoldQuartz | BlockId::IronOre | BlockId::CopperOre));
                if lava_or_crystal && lodes > 3 {
                    break 'scan;
                }
            }
        }
    }
    assert!(lava_or_crystal && lodes > 3, "spawn face: zones {lava_or_crystal}, lode cells {lodes}");
}

/// Finds one open cave cell per zone near spawn for review poses.
/// `cargo test -p engine_worldgen find_cave_poses -- --ignored --nocapture`
#[test]
#[ignore]
fn find_cave_poses() {
    let c = cfg(VERSION);
    let mut found: Vec<Zone> = Vec::new();
    for r in 0..60 {
        for (gx, gz) in [(r, 0), (-r, 0), (0, r), (0, -r), (r, r), (-r, -r), (r, -r), (-r, r)] {
            let (x, z) = (gx * 97 + 300, gz * 97 + 300);
            let column = sample_column(&c, x, z);
            for y in (1..column.height - 6).rev() {
                let p = IVec3::new(x, y, z);
                let open = (0..3).all(|k| crate::sample_block(&c, p + IVec3::Y * k) == BlockId::Air);
                if !open {
                    continue;
                }
                let zone = zone(&c, p, &column);
                if !found.contains(&zone) {
                    found.push(zone);
                    eprintln!("{zone:?}: x={x} z={z} y={y} surface={}", column.height);
                }
                break;
            }
            if found.len() == 5 {
                return;
            }
        }
    }
}

/// Chunk generation cost before and after the gate, interleaved, best of three.
/// `cargo test --release -p engine_worldgen deep_generation_cost -- --ignored --nocapture`
#[test]
#[ignore]
fn deep_generation_cost() {
    let chunks: Vec<IVec3> = (0..12).map(|i| IVec3::new(FACE / 32 + i * 7, -1 + i % 3, i * 5 - 30)).collect();
    let mut best = [f64::MAX; 2];
    for _ in 0..3 {
        for (k, v) in [VERSION - 1, VERSION].into_iter().enumerate() {
            let c = cfg(v);
            let t = std::time::Instant::now();
            for &ch in &chunks {
                std::hint::black_box(crate::generate(&c, ch));
            }
            best[k] = best[k].min(t.elapsed().as_secs_f64() * 1000.0 / chunks.len() as f64);
        }
    }
    eprintln!("ms per chunk: v{} {:.2}, v{} {:.2}", VERSION - 1, best[0], VERSION, best[1]);
}
